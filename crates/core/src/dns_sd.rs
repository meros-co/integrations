//! Finding NMOS registries by unicast DNS-SD (DNS-Based Service Discovery,
//! RFC 6763, over ordinary unicast DNS).
//!
//! AMWA IS-04 (v1.3, "Discovery - Registered Operation") has a registry's
//! Query API advertised as `_nmos-query._tcp` in the site's DNS domain, as well
//! as by mDNS (`crate::mdns`): a PTR from `_nmos-query._tcp.<domain>` to each
//! instance, an SRV giving its host and port, a TXT carrying `api_proto`,
//! `api_ver`, `api_auth` and `pri`, and the host's A record. A network that
//! runs NMOS across routers, where multicast does not reach, advertises its
//! registries this way.
//!
//! Rules:
//!
//! - The DNS servers and domains asked are the request's (`dns_servers`,
//!   `dns_domains`), or else the host's own: on Windows each interface that
//!   is up gives its DNS servers, its connection-specific suffix and its
//!   suffix search list; on Unix `/etc/resolv.conf` gives `nameserver`,
//!   `domain` and `search`. A host with neither (Android, iOS, or nothing
//!   configured) is told so in a `discovery` message, and a scan asks
//!   nothing. At most 4 servers and 8 domains are asked.
//! - A scan asks the first server that answers, over UDP 53 with recursion
//!   desired, for the PTR records of `_nmos-query._tcp.<domain>` in each
//!   domain; then for the SRV and TXT of each instance and the A record of
//!   each SRV target that the answers did not already give. Each question
//!   waits 2 s and is asked once more of the next server before it is given
//!   up; an answer whose id or question does not match is ignored. A
//!   truncated answer (TC) is read as far as it goes.
//! - A registry is reported once per address and port for each scan, with
//!   `api_proto`, `api_ver`, `api_auth` and `pri` in its evidence. Its models
//!   are the integration's one model. No connection is made to it.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::events::{Event, EventQueue};
use crate::mdns::{self, Data, Name};
use crate::session::Services;

pub(crate) const PROTOCOL: &str = "dns-sd";
const SERVICE: &str = "_nmos-query._tcp";
const SPEC: &str = "nmos-registry";
const SOURCE: &str = "AMWA IS-04 v1.3, Discovery - Registered Operation: a Query API advertised in unicast DNS as _nmos-query._tcp, its port in SRV, TXT api_proto, api_ver, api_auth and pri";
const WAIT: Duration = Duration::from_secs(2);
const MAX_SERVERS: usize = 4;
const MAX_DOMAINS: usize = 8;
const MAX_INSTANCES: usize = 256;
const MAX_MESSAGE: usize = 65_535;

/// Where to ask: DNS servers and domains.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Config {
    pub(crate) servers: Vec<IpAddr>,
    pub(crate) domains: Vec<String>,
}

impl Config {
    fn push_server(&mut self, ip: IpAddr) {
        if !ip.is_unspecified() && !self.servers.contains(&ip) {
            self.servers.push(ip);
        }
    }

    fn push_domain(&mut self, domain: &str) {
        let d = domain.trim().trim_end_matches('.').to_ascii_lowercase();
        if !d.is_empty() && d != "local" && !self.domains.contains(&d) {
            self.domains.push(d);
        }
    }
}

/// `/etc/resolv.conf`'s `nameserver`, `domain` and `search` lines.
pub(crate) fn parse_resolv_conf(text: &str) -> Config {
    let mut c = Config::default();
    for line in text.lines() {
        let line = line.split(['#', ';']).next().unwrap_or("");
        let mut words = line.split_whitespace();
        match words.next() {
            Some("nameserver") => {
                // A link-local IPv6 server may carry a zone (`fe80::1%eth0`).
                if let Some(ip) = words.next().and_then(|w| w.split('%').next()?.parse().ok()) {
                    c.push_server(ip);
                }
            }
            Some("domain") | Some("search") => {
                for d in words {
                    c.push_domain(d);
                }
            }
            _ => {}
        }
    }
    c
}

/// The host's own DNS servers and domains.
#[cfg(unix)]
fn system_config() -> Config {
    std::fs::read_to_string("/etc/resolv.conf")
        .map(|t| parse_resolv_conf(&t))
        .unwrap_or_default()
}

/// The host's own DNS servers and domains: each interface that is up.
#[cfg(windows)]
fn system_config() -> Config {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
        GAA_FLAG_SKIP_UNICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6,
    };

    fn wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: a NUL-terminated string the API returned, read up to its NUL.
        unsafe {
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
        }
    }

    fn address(sa: *const SOCKADDR) -> Option<IpAddr> {
        if sa.is_null() {
            return None;
        }
        // SAFETY: the API gives each address with its family, which says
        // which structure it is.
        unsafe {
            match (*sa).sa_family {
                AF_INET => {
                    let v4 = &*(sa as *const SOCKADDR_IN);
                    Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(
                        v4.sin_addr.S_un.S_addr,
                    ))))
                }
                AF_INET6 => {
                    let v6 = &*(sa as *const SOCKADDR_IN6);
                    Some(IpAddr::V6(std::net::Ipv6Addr::from(v6.sin6_addr.u.Byte)))
                }
                _ => None,
            }
        }
    }

    let mut c = Config::default();
    let flags = GAA_FLAG_SKIP_UNICAST | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size: u32 = 16 * 1024;
    for _ in 0..3 {
        // u64 elements keep the buffer aligned for the structures in it.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let head = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        // SAFETY: the buffer is `size` bytes, as the call is told.
        let rc = unsafe {
            GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), head, &mut size)
        };
        // ERROR_BUFFER_OVERFLOW: `size` now says what is needed.
        if rc == 111 {
            continue;
        }
        if rc != 0 {
            return c;
        }
        let mut adapter = head as *const IP_ADAPTER_ADDRESSES_LH;
        // SAFETY: a list the API built in the buffer, which outlives the walk.
        unsafe {
            while !adapter.is_null() {
                let a = &*adapter;
                if a.OperStatus == IfOperStatusUp {
                    let mut server = a.FirstDnsServerAddress;
                    while !server.is_null() {
                        if let Some(ip) = address((*server).Address.lpSockaddr) {
                            // Windows lists the deprecated site-local
                            // defaults (fec0:0:0:ffff::1) on interfaces
                            // with no IPv6 DNS server.
                            let site_local =
                                matches!(ip, IpAddr::V6(v6) if v6.segments()[0] == 0xfec0);
                            if !site_local {
                                c.push_server(ip);
                            }
                        }
                        server = (*server).Next;
                    }
                    c.push_domain(&wide(a.DnsSuffix));
                    let mut suffix = a.FirstDnsSuffix;
                    while !suffix.is_null() {
                        c.push_domain(&wide((*suffix).String.as_ptr()));
                        suffix = (*suffix).Next;
                    }
                }
                adapter = a.Next;
            }
        }
        break;
    }
    c
}

#[cfg(not(any(unix, windows)))]
fn system_config() -> Config {
    Config::default()
}

/// One DNS question, with recursion desired and the given id.
fn question(id: u16, name: &Name, qtype: u16) -> Option<Vec<u8>> {
    let (mut q, held) = mdns::query(&[(name.clone(), qtype)], 512);
    if held != 1 {
        return None;
    }
    q[0..2].copy_from_slice(&id.to_be_bytes());
    q[2] |= 0x01; // RD
    Some(q)
}

/// What a scan has heard.
#[derive(Debug, Default)]
struct Heard {
    /// Instance name key to the instance's name.
    instances: BTreeMap<String, Name>,
    srv: BTreeMap<String, (u16, Name)>,
    txt: BTreeMap<String, Vec<Vec<u8>>>,
    a: BTreeMap<String, Vec<Ipv4Addr>>,
}

impl Heard {
    fn absorb(&mut self, records: &[mdns::Record], service: &Name) {
        for r in records {
            let key = r.name.key();
            match &r.data {
                Data::Ptr(target)
                    if key == service.key() && self.instances.len() < MAX_INSTANCES =>
                {
                    self.instances.insert(target.key(), target.clone());
                }
                Data::Srv { port, target } => {
                    self.srv.insert(key, (*port, target.clone()));
                }
                Data::Txt(strings) => {
                    self.txt.insert(key, strings.clone());
                }
                Data::A(ip) => {
                    let list = self.a.entry(key).or_default();
                    if !list.contains(ip) {
                        list.push(*ip);
                    }
                }
                _ => {}
            }
        }
    }

    /// The registries fully known: one event each.
    fn events(&self, domain: &str, server: IpAddr) -> Vec<Event> {
        let mut out = Vec::new();
        for (key, name) in &self.instances {
            let Some((port, target)) = self.srv.get(key) else {
                continue;
            };
            let Some(addresses) = self.a.get(&target.key()) else {
                continue;
            };
            let txt = self.txt.get(key).map(|s| mdns::txt_pairs(s));
            let mut txt_json = Map::new();
            for (k, v) in txt.iter().flatten() {
                txt_json.insert(k.clone(), v.as_ref().map_or(Value::Null, |v| json!(v)));
            }
            for address in addresses {
                out.push(Event::Discovered {
                    protocol: PROTOCOL.into(),
                    address: address.to_string(),
                    port: *port,
                    device: SPEC.into(),
                    models: vec!["registry".into()],
                    name: Some(name.first()),
                    evidence: json!({
                        "protocol": format!("answered a unicast DNS query to {server} (DNS-SD browse of {SERVICE}.{domain})"),
                        "service": format!("{SERVICE}.{domain}"),
                        "instance": name.text(),
                        "txt": if txt.is_some() { Value::Object(txt_json.clone()) } else { Value::Null },
                        "srv": {"port": port, "target": target.text()},
                        "address": format!("the A record of the SRV target {}", target.text()),
                        "port": "the SRV record's port, which IS-04 documents as the Query API's",
                        "identification": format!("{SERVICE} is {SPEC} ({SOURCE})"),
                        "model": "an NMOS registry: the integration's one model",
                        "name": "the DNS-SD instance name, which the site's DNS configures, so weak evidence of identity",
                    }),
                });
            }
        }
        out
    }
}

/// Ask one question of the servers in turn, each once; the answer's records.
async fn ask(
    socket: &UdpSocket,
    servers: &[SocketAddr],
    id: &mut u16,
    name: &Name,
    qtype: u16,
) -> Option<Vec<mdns::Record>> {
    let mut buf = vec![0u8; MAX_MESSAGE];
    for server in servers {
        *id = id.wrapping_add(1);
        let q = question(*id, name, qtype)?;
        if socket.send_to(&q, *server).await.is_err() {
            continue;
        }
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let Ok(Ok((n, from))) = tokio::time::timeout(left, socket.recv_from(&mut buf)).await
            else {
                break;
            };
            let answer = &buf[..n];
            // Only the server asked, with the id asked and as a response.
            if from != *server
                || answer.get(0..2) != Some(&id.to_be_bytes()[..])
                || answer.get(2).is_none_or(|f| f & 0x80 == 0)
            {
                continue;
            }
            // NXDOMAIN or another error: the server answered, with nothing.
            return Some(mdns::parse(answer).map(|m| m.records).unwrap_or_default());
        }
    }
    None
}

/// Ask each domain of the servers, at their DNS port (53 but in tests).
async fn scan(
    events: Arc<EventQueue>,
    bind: IpAddr,
    servers: Vec<SocketAddr>,
    domains: Vec<String>,
) {
    let log = |text: String| {
        events.push(Event::Discovery {
            protocol: PROTOCOL.into(),
            message: text,
        })
    };
    let wildcard: IpAddr = match servers.first() {
        Some(SocketAddr::V6(_)) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
        _ if bind.is_unspecified() || bind.is_ipv6() => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        _ => bind,
    };
    let socket = match UdpSocket::bind(SocketAddr::new(wildcard, 0)).await {
        Ok(s) => s,
        Err(e) => return log(format!("cannot open a UDP socket for DNS queries ({e})")),
    };
    // A socket of one family reaches only servers of that family.
    let servers: Vec<SocketAddr> = servers
        .into_iter()
        .filter(|s| s.is_ipv6() == wildcard.is_ipv6())
        .collect();
    let mut id: u16 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        & 0xffff) as u16;
    for domain in &domains {
        let service = Name::dotted(&format!("{SERVICE}.{domain}"));
        let mut heard = Heard::default();
        let Some(records) = ask(&socket, &servers, &mut id, &service, mdns::TYPE_PTR).await else {
            log(format!("no DNS server answered for {SERVICE}.{domain}"));
            continue;
        };
        heard.absorb(&records, &service);
        let instances: Vec<(String, Name)> = heard
            .instances
            .iter()
            .map(|(k, n)| (k.clone(), n.clone()))
            .collect();
        for (key, name) in &instances {
            if !heard.srv.contains_key(key) {
                if let Some(r) = ask(&socket, &servers, &mut id, name, mdns::TYPE_SRV).await {
                    heard.absorb(&r, &service);
                }
            }
            if !heard.txt.contains_key(key) {
                if let Some(r) = ask(&socket, &servers, &mut id, name, mdns::TYPE_TXT).await {
                    heard.absorb(&r, &service);
                }
            }
            let target = heard.srv.get(key).map(|(_, t)| t.clone());
            if let Some(target) = target {
                if !heard.a.contains_key(&target.key()) {
                    if let Some(r) = ask(&socket, &servers, &mut id, &target, mdns::TYPE_A).await {
                        heard.absorb(&r, &service);
                    }
                }
            }
        }
        let server = servers.first().map_or(wildcard, SocketAddr::ip);
        for e in heard.events(domain, server) {
            events.push(e);
        }
    }
}

#[derive(Default)]
pub(crate) struct DnsSd {
    scan: Mutex<Option<JoinHandle<()>>>,
}

impl DnsSd {
    /// Unicast DNS has no announcements to hear: listening is a no-op.
    pub(crate) fn listen(&self, _services: &Arc<Services>) -> Result<(), String> {
        Ok(())
    }

    /// Ask the request's DNS servers and domains, or the host's own. A scan
    /// requested while one runs is ignored.
    pub(crate) fn scan(&self, services: &Arc<Services>, servers: &[IpAddr], domains: &[String]) {
        let mut task = self.scan.lock().unwrap();
        if task.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let system = system_config();
        let mut config = Config::default();
        for s in if servers.is_empty() {
            &system.servers
        } else {
            servers
        } {
            config.push_server(*s);
        }
        for d in if domains.is_empty() {
            &system.domains
        } else {
            domains
        } {
            config.push_domain(d);
        }
        let events = services.events.clone();
        if config.servers.is_empty() || config.domains.is_empty() {
            events.push(Event::Discovery {
                protocol: PROTOCOL.into(),
                message: format!(
                    "no {} to ask for {SERVICE}: give dns_servers and dns_domains in the request",
                    if config.servers.is_empty() {
                        "DNS server"
                    } else {
                        "DNS domain"
                    }
                ),
            });
            return;
        }
        if config.servers.len() > MAX_SERVERS || config.domains.len() > MAX_DOMAINS {
            events.push(Event::Discovery {
                protocol: PROTOCOL.into(),
                message: format!(
                    "only the first {MAX_SERVERS} DNS servers and {MAX_DOMAINS} domains are asked"
                ),
            });
            config.servers.truncate(MAX_SERVERS);
            config.domains.truncate(MAX_DOMAINS);
        }
        let servers = config
            .servers
            .iter()
            .map(|ip| SocketAddr::new(*ip, 53))
            .collect();
        *task = Some(tokio::spawn(scan(
            events,
            services.bind_address,
            servers,
            config.domains,
        )));
    }

    pub(crate) fn stop(&self) {
        if let Some(t) = self.scan.lock().unwrap().take() {
            t.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolv_conf_gives_servers_and_domains() {
        let c = parse_resolv_conf(
            "# comment\nnameserver 10.0.0.2\nnameserver fe80::1%eth0\ndomain studio.example\n\
             search studio.example nmos.example. local ; tail\nnameserver 10.0.0.2\noptions ndots:1\n",
        );
        assert_eq!(
            c.servers,
            [
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
                "fe80::1".parse::<IpAddr>().unwrap()
            ]
        );
        assert_eq!(c.domains, ["studio.example", "nmos.example"]);
    }

    #[test]
    fn a_question_asks_for_recursion_with_its_id() {
        let q = question(
            0x1234,
            &Name::dotted("_nmos-query._tcp.studio.example"),
            mdns::TYPE_PTR,
        )
        .unwrap();
        assert_eq!(&q[..4], &[0x12, 0x34, 0x01, 0x00]);
        assert_eq!(&q[4..6], &[0, 1]);
        assert_eq!(&q[q.len() - 4..], &[0, 12, 0, 1]);
    }

    /// A DNS answer: header, the question, then records.
    fn answer(id: u16, records: &[(&[&str], u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = id.to_be_bytes().to_vec();
        out.extend_from_slice(&[0x81, 0x80, 0, 0]);
        out.extend_from_slice(&(records.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        for (name, rtype, rdata) in records {
            for l in *name {
                out.push(l.len() as u8);
                out.extend_from_slice(l.as_bytes());
            }
            out.push(0);
            out.extend_from_slice(&rtype.to_be_bytes());
            out.extend_from_slice(&[0, 1, 0, 0, 0, 60]);
            out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
            out.extend_from_slice(rdata);
        }
        out
    }

    fn labels(parts: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for p in parts {
            out.push(p.len() as u8);
            out.extend_from_slice(p.as_bytes());
        }
        out.push(0);
        out
    }

    #[test]
    fn a_registry_is_reported_from_its_records() {
        let service = Name::dotted("_nmos-query._tcp.studio.example");
        let instance = ["reg1", "_nmos-query", "_tcp", "studio", "example"];
        let mut srv = vec![0, 0, 0, 0, 0x1f, 0x90];
        srv.extend(labels(&["reg", "studio", "example"]));
        let mut txt = Vec::new();
        for s in [
            "api_proto=http",
            "api_ver=v1.2,v1.3",
            "api_auth=false",
            "pri=10",
        ] {
            txt.push(s.len() as u8);
            txt.extend_from_slice(s.as_bytes());
        }
        let message = answer(
            7,
            &[
                (
                    &["_nmos-query", "_tcp", "studio", "example"],
                    mdns::TYPE_PTR,
                    labels(&instance),
                ),
                (&instance, mdns::TYPE_SRV, srv),
                (&instance, mdns::TYPE_TXT, txt),
                (
                    &["reg", "studio", "example"],
                    mdns::TYPE_A,
                    vec![10, 0, 0, 40],
                ),
            ],
        );
        let records = mdns::parse(&message).unwrap().records;
        let mut heard = Heard::default();
        heard.absorb(&records, &service);
        let events = heard.events("studio.example", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)));
        assert_eq!(events.len(), 1);
        let Event::Discovered {
            protocol,
            address,
            port,
            device,
            name,
            evidence,
            ..
        } = &events[0]
        else {
            panic!("{:?}", events[0]);
        };
        assert_eq!(
            (
                protocol.as_str(),
                address.as_str(),
                *port,
                device.as_str(),
                name.as_deref()
            ),
            ("dns-sd", "10.0.0.40", 8080, "nmos-registry", Some("reg1"))
        );
        assert_eq!(evidence["txt"]["pri"], "10");
        assert_eq!(evidence["txt"]["api_ver"], "v1.2,v1.3");
    }

    #[test]
    fn an_instance_without_its_address_is_not_reported() {
        let service = Name::dotted("_nmos-query._tcp.studio.example");
        let instance = ["reg1", "_nmos-query", "_tcp", "studio", "example"];
        let message = answer(
            1,
            &[(
                &["_nmos-query", "_tcp", "studio", "example"],
                mdns::TYPE_PTR,
                labels(&instance),
            )],
        );
        let mut heard = Heard::default();
        heard.absorb(&mdns::parse(&message).unwrap().records, &service);
        assert_eq!(heard.instances.len(), 1);
        assert!(heard
            .events("studio.example", IpAddr::V4(Ipv4Addr::LOCALHOST))
            .is_empty());
    }

    #[tokio::test]
    async fn a_scan_asks_a_loopback_server_and_reports_the_registry() {
        // A fake DNS server on loopback: the PTR answered alone, so the scan
        // asks for the SRV, the TXT and the A record in turn.
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let instance = ["r", "_nmos-query", "_tcp", "lab", "test"];
        let fake = tokio::spawn(async move {
            let mut asked = Vec::new();
            for _ in 0..4 {
                let mut buf = [0u8; 512];
                let (n, from) = server.recv_from(&mut buf).await.unwrap();
                let id = u16::from_be_bytes([buf[0], buf[1]]);
                assert_eq!(buf[2] & 0x01, 1, "recursion desired");
                let qtype = u16::from_be_bytes([buf[n - 4], buf[n - 3]]);
                asked.push(qtype);
                let reply = match qtype {
                    mdns::TYPE_PTR => answer(
                        id,
                        &[(
                            &["_nmos-query", "_tcp", "lab", "test"],
                            qtype,
                            labels(&instance),
                        )],
                    ),
                    mdns::TYPE_SRV => {
                        let mut srv = vec![0, 0, 0, 0, 0x0c, 0x8c];
                        srv.extend(labels(&["host", "lab", "test"]));
                        answer(id, &[(&instance, qtype, srv)])
                    }
                    mdns::TYPE_TXT => {
                        answer(id, &[(&instance, qtype, b"\x0eapi_proto=http".to_vec())])
                    }
                    _ => answer(
                        id,
                        &[(&["host", "lab", "test"], mdns::TYPE_A, vec![127, 0, 0, 9])],
                    ),
                };
                // A stray answer with another id first: ignored.
                server
                    .send_to(&answer(id.wrapping_add(9), &[]), from)
                    .await
                    .unwrap();
                server.send_to(&reply, from).await.unwrap();
            }
            asked
        });
        let events = Arc::new(EventQueue::new(100));
        scan(
            events.clone(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            vec![address],
            vec!["lab.test".into()],
        )
        .await;
        assert_eq!(
            fake.await.unwrap(),
            [mdns::TYPE_PTR, mdns::TYPE_SRV, mdns::TYPE_TXT, mdns::TYPE_A]
        );
        let found = events.drain(100);
        assert!(
            matches!(&found[..], [Event::Discovered { address, port: 3212, evidence, .. }]
                if address == "127.0.0.9" && evidence["txt"]["api_proto"] == "http"),
            "{found:?}"
        );
    }
}
