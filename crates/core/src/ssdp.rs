//! Finding Sony cameras over UPnP SSDP.
//!
//! Sony's Camera Control PTP 3 Reference ("Device Discovery") has a camera
//! announce itself with SSDP NOTIFYs to 239.255.255.250:1900, NT
//! `urn:schemas-sony-com:service:DigitalImaging:1`, and answer an M-SEARCH for
//! that service type. LOCATION points at a UPnP device description (dd.xml)
//! whose DigitalImaging service has an SCPDURL to DigitalImagingDesc.xml,
//! which says the model (`X_ModelName`), the PTP versions spoken
//! (`X_PTP_Versions`; 3.00 is Camera Control PTP 3, anything else PTP 2),
//! whether pairing is necessary and whether SSH is supported.
//!
//! Rules, in the spirit of MCP discovery's (a venue's network is not ours):
//!
//! - UDP 1900 is shared with every other UPnP stack on the host (Windows'
//!   SSDP service, media servers, browsers). The listener binds it with
//!   address reuse and never assumes it is alone; if the port cannot be had,
//!   that is reported and scans still work, because M-SEARCH responses come
//!   back to the search socket's own ephemeral port. On Unix the listener
//!   binds the group address itself, which BSD/macOS treat as shareable under
//!   SO_REUSEADDR and which keeps unrelated unicast traffic off the socket;
//!   Windows cannot bind a multicast address, so it binds the wildcard.
//! - A scan sends one M-SEARCH (MX 2, multicast TTL 2) per usable interface,
//!   three times one second apart, and one unicast M-SEARCH per hint (at most
//!   32) on the same schedule, then listens MX + 1 s for late answers: at most
//!   3 x (interfaces + 32) datagrams in about five seconds. A scan requested
//!   while one is running is ignored.
//! - Anything that is not the Sony DigitalImaging service (by NT/ST or USN) is
//!   dropped before any other work.
//! - Each device (USN) is described once: dd.xml, then DigitalImagingDesc.xml,
//!   each with a 3 s timeout and a 64 KiB cap, at most four devices described
//!   at a time. Repeated NOTIFYs and answers do not refetch; a changed
//!   LOCATION does (the camera moved), and a failed description is retried
//!   no sooner than a minute later. At most 256 devices are remembered.
//! - LOCATION and the SCPDURL must be plain HTTP on the address the datagram
//!   came from, and redirects are not followed, so a forged datagram cannot
//!   make the core fetch from anywhere else.
//! - `ssdp:byebye` is reported as a `Discovery` message; there is no removal
//!   event. CACHE-CONTROL max-age is not tracked.

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use crate::events::{Event, EventQueue};
use crate::session::Services;

pub(crate) const PROTOCOL: &str = "ssdp";
const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const PORT: u16 = 1900;
/// The service type Sony cameras announce and answer for.
const SONY_URN: &str = "urn:schemas-sony-com:service:DigitalImaging:1";
const SONY_SPEC: &str = "sony-camera";
/// PTP-IP, where a found camera is opened.
const PTP_IP_PORT: u16 = 15740;

/// Seconds a device may wait before answering an M-SEARCH (UDA 1.1: 1 to 5).
const MX: u64 = 2;
const SEARCH_REPEATS: usize = 3;
const SEARCH_SPACING: Duration = Duration::from_secs(1);
/// UDA 1.1's recommended default: the local network and one router.
const SEARCH_TTL: u32 = 2;
const MAX_HINTS: usize = 32;

const FETCHES_IN_FLIGHT: usize = 4;
const FETCH_TIMEOUT: Duration = Duration::from_secs(3);
const FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BODY: usize = 64 * 1024;
const RETRY_AFTER: Duration = Duration::from_secs(60);
const MAX_KNOWN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// NOTIFY ssdp:alive (or ssdp:update).
    Alive,
    /// NOTIFY ssdp:byebye.
    Byebye,
    /// HTTP/1.1 200 answering an M-SEARCH.
    Response,
}

/// One SSDP datagram about a Sony camera.
#[derive(Debug, PartialEq)]
struct Advert {
    kind: Kind,
    usn: String,
    location: Option<String>,
    server: Option<String>,
}

/// The Sony DigitalImaging advertisement or answer in `data`, or `None` for
/// anything else (an M-SEARCH, another device type, not SSDP).
fn parse_datagram(data: &[u8]) -> Option<Advert> {
    let text = String::from_utf8_lossy(data);
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    let start = lines.next()?.trim();
    let notify = start
        .split_whitespace()
        .next()
        .is_some_and(|m| m.eq_ignore_ascii_case("NOTIFY"));
    let response = {
        let mut parts = start.split_whitespace();
        parts
            .next()
            .is_some_and(|p| p.to_ascii_uppercase().starts_with("HTTP/1."))
            && parts.next() == Some("200")
    };
    if !notify && !response {
        return None;
    }
    let mut headers: HashMap<String, String> = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers
                .entry(name.trim().to_ascii_lowercase())
                .or_insert_with(|| value.trim().to_string());
        }
    }
    let target = headers.get(if notify { "nt" } else { "st" });
    let usn = headers.get("usn")?.clone();
    let sony = target.is_some_and(|t| t.eq_ignore_ascii_case(SONY_URN))
        || usn
            .to_ascii_lowercase()
            .ends_with(&format!("::{}", SONY_URN.to_ascii_lowercase()));
    if !sony || usn.is_empty() {
        return None;
    }
    let kind = if notify {
        match headers.get("nts")?.to_ascii_lowercase().as_str() {
            "ssdp:alive" | "ssdp:update" => Kind::Alive,
            "ssdp:byebye" => Kind::Byebye,
            _ => return None,
        }
    } else {
        Kind::Response
    };
    let location = headers.get("location").filter(|l| !l.is_empty()).cloned();
    if kind != Kind::Byebye && location.is_none() {
        return None;
    }
    Some(Advert {
        kind,
        usn,
        location,
        server: headers.get("server").cloned(),
    })
}

/// An M-SEARCH for Sony cameras. Multicast carries MX; unicast must not
/// (UDA 1.1, 1.3.2).
fn m_search(host: SocketAddr, mx: Option<u64>) -> String {
    let mut m = format!("M-SEARCH * HTTP/1.1\r\nHOST: {host}\r\nMAN: \"ssdp:discover\"\r\n");
    if let Some(mx) = mx {
        m.push_str(&format!("MX: {mx}\r\n"));
    }
    m.push_str(&format!("ST: {SONY_URN}\r\n\r\n"));
    m
}

/// What dd.xml says. Tags are found by local name, so namespaces and
/// prefixes do not matter.
#[derive(Debug, Default, PartialEq)]
struct DeviceDescription {
    friendly_name: Option<String>,
    manufacturer: Option<String>,
    model_name: Option<String>,
    udn: Option<String>,
    url_base: Option<String>,
    /// The DigitalImaging service's SCPDURL.
    scpd_url: Option<String>,
}

/// What DigitalImagingDesc.xml says.
#[derive(Debug, Default, PartialEq)]
struct ImagingDescription {
    model_name: Option<String>,
    ptp_versions: Vec<String>,
    pairing: Option<bool>,
    ssh: Option<bool>,
}

impl ImagingDescription {
    fn ptp3(&self) -> bool {
        self.ptp_versions
            .iter()
            .any(|v| v.parse::<f64>().is_ok_and(|n| n.trunc() == 3.0))
    }
}

fn parse_xml(xml: &str) -> Result<roxmltree::Document<'_>, String> {
    roxmltree::Document::parse(xml.trim_start_matches('\u{feff}').trim_start())
        .map_err(|e| format!("not XML: {e}"))
}

fn is(node: &roxmltree::Node, local: &str) -> bool {
    node.is_element() && node.tag_name().name().eq_ignore_ascii_case(local)
}

fn text(node: roxmltree::Node) -> Option<String> {
    let t: String = node
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// The text of the first element named `local`, anywhere under `node`.
fn find(node: roxmltree::Node, local: &str) -> Option<String> {
    node.descendants().find(|n| is(n, local)).and_then(text)
}

fn parse_device_description(xml: &str) -> Result<DeviceDescription, String> {
    let doc = parse_xml(xml)?;
    let root = doc.root();
    let services: Vec<(Option<String>, Option<String>)> = root
        .descendants()
        .filter(|n| is(n, "service"))
        .map(|s| (find(s, "serviceType"), find(s, "SCPDURL")))
        .collect();
    let scpd_url = services
        .iter()
        .find(|(t, u)| u.is_some() && t.as_deref().is_some_and(|t| t.contains("DigitalImaging")))
        .or_else(|| {
            services.iter().find(|(_, u)| {
                u.as_deref()
                    .is_some_and(|u| u.contains("DigitalImagingDesc"))
            })
        })
        .and_then(|(_, u)| u.clone());
    Ok(DeviceDescription {
        friendly_name: find(root, "friendlyName"),
        manufacturer: find(root, "manufacturer"),
        model_name: find(root, "modelName"),
        udn: find(root, "UDN"),
        url_base: find(root, "URLBase"),
        scpd_url,
    })
}

fn parse_imaging_description(xml: &str) -> Result<ImagingDescription, String> {
    let doc = parse_xml(xml)?;
    let root = doc.root();
    Ok(ImagingDescription {
        model_name: find(root, "X_ModelName"),
        ptp_versions: find(root, "X_PTP_Versions")
            .map(|v| {
                v.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        pairing: find(root, "X_PTP_PairingNecessity").and_then(|v| flag(&v)),
        ssh: find(root, "X_SSH_Support").and_then(|v| flag(&v)),
    })
}

/// Enable/Disable, read leniently.
fn flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "enable" | "enabled" | "true" | "yes" | "on" | "1" | "supported" | "necessary" => {
            Some(true)
        }
        "disable" | "disabled" | "false" | "no" | "off" | "0" | "unsupported" | "unnecessary" => {
            Some(false)
        }
        _ => None,
    }
}

/// The sony-camera models in the embedded catalogue, as (id, name). Read once.
fn sony_models() -> &'static [(String, String)] {
    static MODELS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    MODELS.get_or_init(|| {
        crate::catalog::Catalog::embedded()
            .device(SONY_SPEC)
            .map(|s| {
                s.models
                    .iter()
                    .map(|m| (m.id.clone(), m.name.clone()))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The catalogue model a camera's `X_ModelName` names: the model whose id is
/// that name, or whose name has it as a whole token (`ILCE-7M4` in "Sony
/// Alpha 7 IV (ILCE-7M4)"; whole tokens, so ILCE-7RM4 never matches
/// ILCE-7RM4A). Sony's documents call the PXW-Z380 the PXW-Z300.
fn match_model(model_name: &str, models: &[(String, String)]) -> Option<String> {
    let wanted = model_name.trim();
    if wanted.is_empty() {
        return None;
    }
    let mut names = vec![wanted.to_string()];
    if wanted.eq_ignore_ascii_case("PXW-Z300") {
        names.push("PXW-Z380".into());
    }
    let tokens = |name: &str| -> Vec<String> {
        name.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter(|t| !t.is_empty())
            .map(String::from)
            .collect()
    };
    for wanted in &names {
        if let Some((id, _)) = models
            .iter()
            .find(|(id, _)| id.eq_ignore_ascii_case(wanted))
        {
            return Some(id.clone());
        }
        if let Some((id, _)) = models
            .iter()
            .find(|(_, name)| tokens(name).iter().any(|t| t.eq_ignore_ascii_case(wanted)))
        {
            return Some(id.clone());
        }
    }
    None
}

/// `url` if it is plain HTTP on `from`, the address the datagram came from.
fn same_host(url: &str, from: IpAddr) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?.trim_matches(['[', ']']);
    (url.scheme() == "http" && host.parse::<IpAddr>().ok() == Some(from)).then_some(url)
}

enum Known {
    Fetching {
        location: String,
    },
    Found {
        location: String,
        address: IpAddr,
        name: Option<String>,
    },
    Failed {
        location: String,
        at: Instant,
    },
}

/// What one discovery session (listen to stop) shares between the listener,
/// scans and description fetches.
struct Shared {
    events: Arc<EventQueue>,
    client: reqwest::Client,
    known: Mutex<HashMap<String, Known>>,
    fetches: Semaphore,
    stopped: AtomicBool,
    full_reported: AtomicBool,
}

impl Shared {
    fn new(events: Arc<EventQueue>) -> Result<Shared, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(FETCH_CONNECT_TIMEOUT)
            .timeout(FETCH_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .http1_only()
            .build()
            .map_err(|e| format!("cannot build the SSDP HTTP client: {e}"))?;
        Ok(Shared {
            events,
            client,
            known: Mutex::default(),
            fetches: Semaphore::new(FETCHES_IN_FLIGHT),
            stopped: AtomicBool::new(false),
            full_reported: AtomicBool::new(false),
        })
    }

    fn log(&self, message: String) {
        self.events.push(Event::Discovery {
            protocol: PROTOCOL.into(),
            message,
        });
    }

    /// A datagram from the listener or a scan.
    fn handle(self: &Arc<Self>, from: IpAddr, data: &[u8]) {
        let Some(advert) = parse_datagram(data) else {
            return;
        };
        let mut known = self.known.lock().unwrap();
        if advert.kind == Kind::Byebye {
            if let Some(Known::Found { address, name, .. }) = known.remove(&advert.usn) {
                drop(known);
                self.log(format!(
                    "{} at {address} announced it is leaving (ssdp:byebye)",
                    name.as_deref().unwrap_or(&advert.usn)
                ));
            }
            return;
        }
        let Some(location) = advert.location.clone() else {
            return;
        };
        let Some(url) = same_host(&location, from) else {
            return;
        };
        match known.get(&advert.usn) {
            Some(Known::Fetching { .. }) => return,
            Some(Known::Found { location: l, .. }) if *l == location => return,
            Some(Known::Failed { location: l, at })
                if *l == location && at.elapsed() < RETRY_AFTER =>
            {
                return
            }
            None if known.len() >= MAX_KNOWN => {
                if !self.full_reported.swap(true, Ordering::Relaxed) {
                    drop(known);
                    self.log(format!(
                        "more than {MAX_KNOWN} cameras announced; further ones are ignored until discovery is stopped"
                    ));
                }
                return;
            }
            _ => {}
        }
        known.insert(
            advert.usn.clone(),
            Known::Fetching {
                location: location.clone(),
            },
        );
        drop(known);
        let shared = self.clone();
        tokio::spawn(async move { shared.describe(from, advert, location, url).await });
    }

    async fn describe(
        self: Arc<Self>,
        from: IpAddr,
        advert: Advert,
        location: String,
        url: reqwest::Url,
    ) {
        let Ok(_permit) = self.fetches.acquire().await else {
            return;
        };
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let result = self.fetch_descriptions(from, &url).await;
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let mut known = self.known.lock().unwrap();
        // A byebye while fetching removed the entry: the camera has gone.
        if !matches!(known.get(&advert.usn), Some(Known::Fetching { location: l }) if *l == location)
        {
            return;
        }
        match result {
            Ok((device, imaging, imaging_error)) => {
                known.insert(
                    advert.usn.clone(),
                    Known::Found {
                        location: location.clone(),
                        address: from,
                        name: device.friendly_name.clone(),
                    },
                );
                drop(known);
                self.events.push(discovered(
                    from,
                    &advert,
                    &location,
                    &device,
                    &imaging,
                    imaging_error,
                ));
            }
            Err(e) => {
                known.insert(
                    advert.usn.clone(),
                    Known::Failed {
                        location: location.clone(),
                        at: Instant::now(),
                    },
                );
                drop(known);
                self.log(format!(
                    "{from} announces a Sony camera ({}) but its description at {location} could not be read: {e}",
                    advert.usn
                ));
            }
        }
    }

    /// dd.xml, then DigitalImagingDesc.xml. A camera whose second document
    /// cannot be read is still reported, with the reason.
    async fn fetch_descriptions(
        &self,
        from: IpAddr,
        location: &reqwest::Url,
    ) -> Result<(DeviceDescription, ImagingDescription, Option<String>), String> {
        let device = parse_device_description(&self.get(location).await?)?;
        let imaging = async {
            let scpd = device
                .scpd_url
                .as_deref()
                .ok_or("dd.xml names no DigitalImaging service")?;
            let base = match &device.url_base {
                Some(b) => reqwest::Url::parse(b).map_err(|e| format!("URLBase: {e}"))?,
                None => location.clone(),
            };
            let url = base
                .join(scpd)
                .map_err(|e| format!("SCPDURL {scpd}: {e}"))?;
            let url = same_host(url.as_str(), from)
                .ok_or_else(|| format!("SCPDURL {url} is not plain HTTP on {from}"))?;
            parse_imaging_description(&self.get(&url).await?)
        }
        .await;
        Ok(match imaging {
            Ok(i) => (device, i, None),
            Err(e) => (device, ImagingDescription::default(), Some(e)),
        })
    }

    async fn get(&self, url: &reqwest::Url) -> Result<String, String> {
        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| format!("{url}: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("{url}: HTTP {}", response.status().as_u16()));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| format!("{url}: {e}"))? {
            body.extend_from_slice(&chunk);
            if body.len() > MAX_BODY {
                return Err(format!("{url}: larger than {MAX_BODY} bytes"));
            }
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

fn discovered(
    address: IpAddr,
    advert: &Advert,
    location: &str,
    device: &DeviceDescription,
    imaging: &ImagingDescription,
    imaging_error: Option<String>,
) -> Event {
    let model_name = imaging.model_name.clone().or(device.model_name.clone());
    let model = model_name
        .as_deref()
        .and_then(|n| match_model(n, sony_models()));
    let ptp3 = imaging.ptp3();
    let suggested = if imaging.ssh == Some(true) {
        "ssh"
    } else if imaging.pairing == Some(true) {
        "pairing"
    } else {
        "plain"
    };
    let identification = match (&model_name, &model) {
        (Some(n), Some(m)) => format!("model name {n} is catalogue model {m}"),
        (Some(n), None) => format!("model name {n} is not in the sony-camera catalogue"),
        (None, _) => "the camera did not say its model".into(),
    };
    let ptp = if ptp3 {
        "Camera Control PTP 3 (X_PTP_Versions has 3.00)"
    } else if imaging.ptp_versions.is_empty() {
        "unknown: X_PTP_Versions was not read"
    } else {
        "PTP 2 only: X_PTP_Versions lacks 3.00; the module drives it with Camera Control PTP 2 (protocol ptp2, or auto for ILCE-7M3)"
    };
    let mut evidence = json!({
        "protocol": match advert.kind {
            Kind::Response => "answered an SSDP M-SEARCH for the Sony DigitalImaging service",
            _ => "announced the Sony DigitalImaging service over SSDP",
        },
        "usn": advert.usn,
        "location": location,
        "server": advert.server,
        "udn": device.udn,
        "manufacturer": device.manufacturer,
        "model_name": model_name,
        "model": identification,
        "ptp_versions": imaging.ptp_versions,
        "ptp3": ptp3,
        "ptp": ptp,
        "pairing": imaging.pairing,
        "ssh": imaging.ssh,
        "suggested_connection": suggested,
        "name": "friendlyName from dd.xml; can be set on the camera, so weak evidence of identity",
    });
    if let Some(e) = imaging_error {
        evidence["description_error"] = json!(e);
    }
    Event::Discovered {
        protocol: PROTOCOL.into(),
        address: address.to_string(),
        port: PTP_IP_PORT,
        device: SONY_SPEC.into(),
        models: model.into_iter().collect(),
        name: device.friendly_name.clone(),
        evidence,
    }
}

/// The socket that hears NOTIFYs: UDP 1900, shared with other software.
fn bind_listener() -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    #[cfg(unix)]
    let address = IpAddr::V4(GROUP);
    #[cfg(not(unix))]
    let address = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    socket.bind(&SocketAddr::new(address, PORT).into())?;
    UdpSocket::from_std(socket.into())
}

/// The socket a scan sends from and hears answers on: an ephemeral port on
/// the core's bind address.
fn bind_search(bind: IpAddr) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_ttl_v4(SEARCH_TTL)?;
    socket.set_nonblocking(true)?;
    let local = match bind {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
    };
    socket.bind(&SocketAddr::new(IpAddr::V4(local), 0).into())?;
    UdpSocket::from_std(socket.into())
}

async fn receive(socket: &UdpSocket, shared: &Arc<Shared>, window: Duration, buf: &mut [u8]) {
    let until = tokio::time::Instant::now() + window;
    loop {
        match tokio::time::timeout_at(until, socket.recv_from(buf)).await {
            Err(_) => return,
            Ok(Ok((n, from))) => shared.handle(from.ip(), &buf[..n]),
            // Windows reports an ICMP port unreachable from an earlier send.
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
            Ok(Err(_)) => return,
        }
    }
}

async fn search(
    shared: Arc<Shared>,
    bind: IpAddr,
    interfaces: Vec<Ipv4Addr>,
    hints: Vec<Ipv4Addr>,
) {
    let socket = match bind_search(bind) {
        Ok(s) => s,
        Err(e) => {
            shared.log(format!("cannot open a socket to search from: {e}"));
            return;
        }
    };
    let group = SocketAddr::new(IpAddr::V4(GROUP), PORT);
    let multicast = m_search(group, Some(MX));
    let mut buf = vec![0u8; 8192];
    for round in 0..SEARCH_REPEATS {
        for ip in &interfaces {
            if SockRef::from(&socket).set_multicast_if_v4(ip).is_ok() {
                let _ = socket.send_to(multicast.as_bytes(), group).await;
            }
        }
        for hint in &hints {
            let to = SocketAddr::new(IpAddr::V4(*hint), PORT);
            let _ = socket.send_to(m_search(to, None).as_bytes(), to).await;
        }
        let window = if round + 1 < SEARCH_REPEATS {
            SEARCH_SPACING
        } else {
            Duration::from_secs(MX + 1)
        };
        receive(&socket, &shared, window, &mut buf).await;
    }
}

#[derive(Default)]
struct State {
    shared: Option<Arc<Shared>>,
    listener: Option<(Arc<UdpSocket>, JoinHandle<()>)>,
    joined: BTreeSet<Ipv4Addr>,
    scan: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub(crate) struct Ssdp {
    state: Mutex<State>,
}

impl Ssdp {
    fn shared(state: &mut State, services: &Services) -> Result<Arc<Shared>, String> {
        if let Some(s) = &state.shared {
            return Ok(s.clone());
        }
        let shared = Arc::new(Shared::new(services.events.clone())?);
        state.shared = Some(shared.clone());
        Ok(shared)
    }

    /// Hear announcements. If UDP 1900 cannot be had, that is reported and
    /// not an error: scans still work. Interfaces that appeared since are
    /// joined on every call, so a scan picks up a newly connected network.
    pub(crate) fn listen(&self, services: &Arc<Services>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let shared = Self::shared(&mut state, services)?;
        if state.listener.is_none() {
            let socket = match bind_listener() {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    shared.log(format!(
                        "cannot listen for camera announcements on UDP {PORT} ({e}); another \
                         program may hold it exclusively. Scans still find cameras"
                    ));
                    return Ok(());
                }
            };
            let task = {
                let socket = socket.clone();
                let shared = shared.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    loop {
                        match socket.recv_from(&mut buf).await {
                            Ok((n, from)) => shared.handle(from.ip(), &buf[..n]),
                            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                            Err(_) => return,
                        }
                    }
                })
            };
            state.listener = Some((socket, task));
            state.joined.clear();
        }
        let socket = state.listener.as_ref().map(|(s, _)| s.clone()).unwrap();
        for (ip, _) in crate::discovery::interfaces(services.bind_address) {
            if state.joined.contains(&ip) {
                continue;
            }
            match SockRef::from(&*socket).join_multicast_v4(&GROUP, &ip) {
                Ok(()) => {
                    state.joined.insert(ip);
                }
                Err(e) => shared.log(format!(
                    "cannot join the SSDP group on {ip} ({e}); announcements there will not be heard"
                )),
            }
        }
        Ok(())
    }

    pub(crate) fn scan(&self, services: &Arc<Services>, hints: &[Ipv4Addr]) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if state.scan.as_ref().is_some_and(|t| !t.is_finished()) {
            return Ok(());
        }
        let shared = Self::shared(&mut state, services)?;
        let interfaces: Vec<Ipv4Addr> = crate::discovery::interfaces(services.bind_address)
            .into_iter()
            .map(|(ip, _)| ip)
            .collect();
        let mut unique: Vec<Ipv4Addr> = Vec::new();
        for h in hints {
            if !unique.contains(h) && !interfaces.contains(h) {
                unique.push(*h);
            }
        }
        if unique.len() > MAX_HINTS {
            shared.log(format!(
                "{} hints given; only the first {MAX_HINTS} are asked by unicast M-SEARCH",
                unique.len()
            ));
            unique.truncate(MAX_HINTS);
        }
        if interfaces.is_empty() && unique.is_empty() {
            shared.log("no IPv4 interface to search on, and no hints".into());
            return Ok(());
        }
        state.scan = Some(tokio::spawn(search(
            shared,
            services.bind_address,
            interfaces,
            unique,
        )));
        Ok(())
    }

    /// Stop listening and scanning, and forget every camera, so the next
    /// session reports them again.
    pub(crate) fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(t) = state.scan.take() {
            t.abort();
        }
        if let Some((_, t)) = state.listener.take() {
            t.abort();
        }
        state.joined.clear();
        if let Some(s) = state.shared.take() {
            s.stopped.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    fn notify(nts: &str, location: &str) -> String {
        format!(
            "NOTIFY * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nCACHE-CONTROL: max-age=1800\r\n\
             LOCATION: {location}\r\nNT: {SONY_URN}\r\nNTS: {nts}\r\n\
             SERVER: UPnP/1.0 SonyImagingDevice/1.0\r\n\
             USN: uuid:00000000-0000-0010-8000-1234567890ab::{SONY_URN}\r\n\r\n"
        )
    }

    fn dd_xml() -> String {
        r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0" xmlns:av="urn:schemas-sony-com:av">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>
    <friendlyName>ILCE-7M4 Stage Left</friendlyName>
    <manufacturer>Sony Corporation</manufacturer>
    <modelName>ILCE-7M4</modelName>
    <UDN>uuid:00000000-0000-0010-8000-1234567890ab</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <SCPDURL>/cm.xml</SCPDURL>
      </service>
      <service>
        <serviceType>urn:schemas-sony-com:service:DigitalImaging:1</serviceType>
        <serviceId>urn:schemas-sony-com:serviceId:DigitalImaging</serviceId>
        <SCPDURL>/DigitalImagingDesc.xml</SCPDURL>
        <controlURL>/upnp/control/DigitalImaging</controlURL>
      </service>
    </serviceList>
  </device>
</root>"#
            .into()
    }

    fn imaging_xml(versions: &str, pairing: &str, ssh: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0" xmlns:di="urn:schemas-sony-com:di">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <di:X_DigitalImagingInfo>
    <di:X_ModelName>ILCE-7M4</di:X_ModelName>
    <di:X_PTP_Versions>{versions}</di:X_PTP_Versions>
    <di:X_PTP_PairingNecessity>{pairing}</di:X_PTP_PairingNecessity>
    <di:X_SSH_Support>{ssh}</di:X_SSH_Support>
  </di:X_DigitalImagingInfo>
</scpd>"#
        )
    }

    #[test]
    fn notifies_and_answers_for_sony_cameras_are_parsed() {
        let alive =
            parse_datagram(notify("ssdp:alive", "http://10.0.0.9:80/dd.xml").as_bytes()).unwrap();
        assert_eq!(alive.kind, Kind::Alive);
        assert_eq!(
            alive.usn,
            format!("uuid:00000000-0000-0010-8000-1234567890ab::{SONY_URN}")
        );
        assert_eq!(alive.location.as_deref(), Some("http://10.0.0.9:80/dd.xml"));
        assert_eq!(
            alive.server.as_deref(),
            Some("UPnP/1.0 SonyImagingDevice/1.0")
        );

        let bye = parse_datagram(notify("ssdp:byebye", "").as_bytes()).unwrap();
        assert_eq!(bye.kind, Kind::Byebye);
        assert_eq!(bye.location, None);

        // Lower-case headers and bare LF, as some stacks send.
        let answer = format!(
            "HTTP/1.1 200 OK\ncache-control: max-age=1800\next:\nlocation: http://10.0.0.9:80/dd.xml\n\
             server: UPnP/1.0 SonyImagingDevice/1.0\nst: {SONY_URN}\n\
             usn: uuid:abc::{SONY_URN}\n\n"
        );
        let answer = parse_datagram(answer.as_bytes()).unwrap();
        assert_eq!(answer.kind, Kind::Response);
        assert_eq!(answer.usn, format!("uuid:abc::{SONY_URN}"));

        // Our own (or anyone's) M-SEARCH, other device types and noise are not cameras.
        let search = m_search(SocketAddr::new(IpAddr::V4(GROUP), PORT), Some(MX));
        assert!(search.contains("MX: 2\r\n") && search.contains(SONY_URN));
        assert_eq!(parse_datagram(search.as_bytes()), None);
        let other = notify("ssdp:alive", "http://10.0.0.9/x.xml")
            .replace(SONY_URN, "urn:schemas-upnp-org:device:MediaRenderer:1");
        assert_eq!(parse_datagram(other.as_bytes()), None);
        assert_eq!(parse_datagram(b"HTTP/1.1 404 Not Found\r\n\r\n"), None);
        assert_eq!(parse_datagram(b"\xff\xfe"), None);
        // Unicast M-SEARCH carries no MX.
        assert!(!m_search(SocketAddr::new(LOCAL, PORT), None).contains("MX"));
    }

    #[test]
    fn the_descriptions_are_read_by_local_name() {
        let dd = parse_device_description(&dd_xml()).unwrap();
        assert_eq!(dd.friendly_name.as_deref(), Some("ILCE-7M4 Stage Left"));
        assert_eq!(dd.manufacturer.as_deref(), Some("Sony Corporation"));
        assert_eq!(
            dd.udn.as_deref(),
            Some("uuid:00000000-0000-0010-8000-1234567890ab")
        );
        assert_eq!(dd.scpd_url.as_deref(), Some("/DigitalImagingDesc.xml"));

        let ptp3 = parse_imaging_description(&imaging_xml("3.00", "Enable", "Disable")).unwrap();
        assert_eq!(ptp3.model_name.as_deref(), Some("ILCE-7M4"));
        assert_eq!(ptp3.ptp_versions, ["3.00"]);
        assert!(ptp3.ptp3());
        assert_eq!((ptp3.pairing, ptp3.ssh), (Some(true), Some(false)));

        let ptp2 = parse_imaging_description(&imaging_xml("2.00", "Disable", "Enable")).unwrap();
        assert!(!ptp2.ptp3());
        assert_eq!((ptp2.pairing, ptp2.ssh), (Some(false), Some(true)));

        let both = parse_imaging_description(&imaging_xml("2.00, 3.00", "", "")).unwrap();
        assert!(both.ptp3());
        assert_eq!((both.pairing, both.ssh), (None, None));

        assert!(parse_device_description("not xml").is_err());
    }

    #[test]
    fn model_names_map_to_catalogue_models() {
        let models: Vec<(String, String)> = [
            ("ilce-7m4", "Sony Alpha 7 IV (ILCE-7M4)"),
            ("ilce-7rm4a", "Sony Alpha 7R IVA (ILCE-7RM4A)"),
            ("ilce-7rm4", "Sony Alpha 7R IV (ILCE-7RM4)"),
            ("ilme-fx3", "Sony FX3 and FX3A (ILME-FX3, ILME-FX3A)"),
            ("pxw-z380", "Sony PXW-Z380"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let m = |n: &str| match_model(n, &models);
        assert_eq!(m("ILCE-7M4").as_deref(), Some("ilce-7m4"));
        assert_eq!(m("ILCE-7RM4").as_deref(), Some("ilce-7rm4"));
        assert_eq!(m("ILCE-7RM4A").as_deref(), Some("ilce-7rm4a"));
        assert_eq!(m("ILME-FX3A").as_deref(), Some("ilme-fx3"));
        assert_eq!(m("PXW-Z300").as_deref(), Some("pxw-z380"));
        assert_eq!(m("ILCE-7M3"), None);
        assert_eq!(m(""), None);
        // And against the embedded catalogue.
        assert_eq!(
            match_model("ILCE-7M4", sony_models()).as_deref(),
            Some("ilce-7m4")
        );
    }

    #[test]
    fn descriptions_are_fetched_only_from_the_announcing_address() {
        let from = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        assert!(same_host("http://10.0.0.9:80/dd.xml", from).is_some());
        assert!(same_host("http://10.0.0.10/dd.xml", from).is_none());
        assert!(same_host("https://10.0.0.9/dd.xml", from).is_none());
        assert!(same_host("http://camera.local/dd.xml", from).is_none());
    }

    /// Serves dd.xml and DigitalImagingDesc.xml on 127.0.0.1, counting
    /// requests.
    async fn camera_http(imaging: String) -> (u16, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = requests.clone();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                counter.fetch_add(1, Ordering::SeqCst);
                let imaging = imaging.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let mut got = Vec::new();
                    while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                        let Ok(n) = stream.read(&mut buf).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        got.extend_from_slice(&buf[..n]);
                    }
                    let head = String::from_utf8_lossy(&got);
                    let body = if head.starts_with("GET /dd.xml") {
                        dd_xml()
                    } else {
                        imaging
                    };
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/xml\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(reply.as_bytes()).await;
                });
            }
        });
        (port, requests)
    }

    async fn next_event(events: &EventQueue) -> Event {
        tokio::time::timeout(Duration::from_secs(10), events.next(1))
            .await
            .expect("an event within 10 s")
            .remove(0)
    }

    #[tokio::test]
    async fn a_notify_is_described_once_and_reported() {
        let (port, requests) = camera_http(imaging_xml("3.00", "Enable", "Disable")).await;
        let events = Arc::new(EventQueue::new(64));
        let shared = Arc::new(Shared::new(events.clone()).unwrap());
        let location = format!("http://127.0.0.1:{port}/dd.xml");
        let alive = notify("ssdp:alive", &location);
        shared.handle(LOCAL, alive.as_bytes());
        shared.handle(LOCAL, alive.as_bytes());

        let Event::Discovered {
            protocol,
            address,
            port: ptp_port,
            device,
            models,
            name,
            evidence,
        } = next_event(&events).await
        else {
            panic!("expected Discovered");
        };
        assert_eq!(protocol, "ssdp");
        assert_eq!(address, "127.0.0.1");
        assert_eq!(ptp_port, 15740);
        assert_eq!(device, "sony-camera");
        assert_eq!(models, ["ilce-7m4"]);
        assert_eq!(name.as_deref(), Some("ILCE-7M4 Stage Left"));
        assert_eq!(evidence["location"], location);
        assert_eq!(evidence["model_name"], "ILCE-7M4");
        assert_eq!(evidence["ptp_versions"], json!(["3.00"]));
        assert_eq!(evidence["ptp3"], true);
        assert_eq!(evidence["pairing"], true);
        assert_eq!(evidence["ssh"], false);
        assert_eq!(evidence["suggested_connection"], "pairing");
        assert!(evidence.get("description_error").is_none());

        // Repeated announcements do not refetch: two documents, once.
        for _ in 0..5 {
            shared.handle(LOCAL, alive.as_bytes());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert!(events.drain(16).is_empty(), "nothing more is reported");

        // A forged LOCATION elsewhere is ignored.
        let forged = notify("ssdp:alive", "http://10.9.9.9/dd.xml").replace("1234567890ab", "ffff");
        shared.handle(LOCAL, forged.as_bytes());

        shared.handle(LOCAL, notify("ssdp:byebye", "").as_bytes());
        let Event::Discovery { protocol, message } = next_event(&events).await else {
            panic!("expected Discovery");
        };
        assert_eq!(protocol, "ssdp");
        assert!(message.contains("ssdp:byebye"), "{message}");
        assert!(message.contains("ILCE-7M4 Stage Left"), "{message}");
    }

    #[tokio::test]
    async fn a_ptp2_body_with_ssh_is_still_reported() {
        let (port, _) = camera_http(imaging_xml("2.00", "Disable", "Enable")).await;
        let events = Arc::new(EventQueue::new(64));
        let shared = Arc::new(Shared::new(events.clone()).unwrap());
        let answer = format!(
            "HTTP/1.1 200 OK\r\nLOCATION: http://127.0.0.1:{port}/dd.xml\r\nST: {SONY_URN}\r\n\
             USN: uuid:ptp2::{SONY_URN}\r\n\r\n"
        );
        shared.handle(LOCAL, answer.as_bytes());
        let Event::Discovered { evidence, .. } = next_event(&events).await else {
            panic!("expected Discovered");
        };
        assert_eq!(evidence["ptp3"], false);
        assert_eq!(evidence["suggested_connection"], "ssh");
        assert!(evidence["ptp"].as_str().unwrap().starts_with("PTP 2"));
    }

    #[test]
    fn a_scan_is_bounded() {
        // Per scan: three rounds of one datagram per interface and hint.
        let per_target = SEARCH_REPEATS;
        assert!(per_target * MAX_HINTS <= 96);
        let duration = SEARCH_SPACING * (SEARCH_REPEATS as u32 - 1) + Duration::from_secs(MX + 1);
        assert_eq!(duration, Duration::from_secs(5));
        // A device's two documents are fetched one after the other.
        const { assert!(FETCHES_IN_FLIGHT <= 4, "at most four requests in flight") };
    }
}
