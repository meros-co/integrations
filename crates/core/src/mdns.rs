//! Finding Blackmagic Design devices over Multicast DNS and DNS-SD (Bonjour).
//!
//! Multicast DNS (RFC 6762) carries ordinary DNS messages to 224.0.0.251 on
//! UDP 5353; DNS-SD (RFC 6763) names each advertised service instance
//! `<Instance>.<_service>.<_proto>.local`, with a PTR from the service type to
//! each instance, an SRV giving the instance's host and port, a TXT of
//! `key=value` strings, and the host's A and AAAA records.
//!
//! Blackmagic does not document what its products advertise. What this
//! module asks for and how it reads the answer rests on public MIT-licensed
//! sources, listed against each rule in [`RULES`]:
//!
//! - Bitfocus Companion's module manifests (`companion/manifest.json`,
//!   `bonjourQueries`) browse `_blackmagic._tcp` and keep the instances whose
//!   TXT `class` is `AtemSwitcher` (companion-module-bmd-atem), `Videohub`
//!   (-bmd-videohub), `SmartView` (-bmd-smartview) or `MultiView`
//!   (-bmd-multiview4, which also requires TXT `name` to be "Blackmagic
//!   MultiView 4"); -bmd-atem also browses `_switcher_ctrl._udp` and
//!   -bmd-hyperdeck browses `_hyperdeck_ctrl._tcp`, each with no TXT filter.
//! - smartview-client (Jan Schär) finds SmartView and SmartScope monitors by
//!   browsing `_smartview._tcp`.
//!
//! What is inferred, and listed in VERIFICATION.md: that TXT `class` names the
//! product family exactly as Companion filters on it, and that TXT `name` is
//! the product name (Companion's MultiView 4 filter can only work if it is),
//! not a label the user sets. No public source names the service type or
//! class of the Web Presenter and Streaming Encoders, Blackmagic cameras or
//! Teranex converters, so they are not identified; a `_blackmagic._tcp`
//! instance of another class is reported as a `Discovery` message naming the
//! class, so a user can tell us what their device advertises.
//!
//! Rules, in the spirit of SSDP discovery's (a venue's network is not ours):
//!
//! - UDP 5353 is shared with the host's own mDNS responder (mDNSResponder,
//!   Avahi, Windows' DNS client). The listener binds it with address (and on
//!   Unix port) reuse and never assumes it is alone; if the port cannot be
//!   had, that is reported and scans still work, because a query sent from
//!   another port is answered by unicast to that port (RFC 6762 §6.7). On Unix
//!   the listener binds the group address itself; Windows cannot bind a
//!   multicast address, so it binds the wildcard. It joins the group on every
//!   usable interface.
//! - A scan sends one query, a PTR question for each service type of a built
//!   and selected integration, per usable interface (multicast, IP TTL 255,
//!   RFC 6762 §11) and by unicast to UDP 5353 of each hint (at most 32; §5.5
//!   has a responder answer such a query by unicast), at 0, 1 and 3 s, then
//!   listens 2 s more: at most 3 x (interfaces + 32) datagrams in five
//!   seconds. Questions after the first round also ask for the SRV and TXT of
//!   instances heard without them and the A record of SRV targets heard
//!   without one, as many as fit in 1200 bytes. A scan requested while one is
//!   running is ignored.
//! - Only responses (QR set, opcode and rcode 0) are read; a query, ours or
//!   anyone's, is not. A malformed message (a label or record running past the
//!   end, a compression pointer that does not point backwards, a name over
//!   255 bytes) is dropped whole.
//! - Only records of the service types asked for are kept: their instances'
//!   SRV and TXT, and the addresses of the hosts those SRVs name. At most 256
//!   instances and 256 hosts are remembered until discovery stops.
//! - An instance is identified once it has an SRV and, for `_blackmagic._tcp`,
//!   a TXT carrying a known `class`. Its address is the SRV target's A
//!   record, or the address the answer came from when no A record was given.
//!   The port reported is the integration's control port; the SRV port is
//!   evidence only, since no source says what it is for `_blackmagic._tcp`.
//!   Each (address, integration) is reported once, and again only when its
//!   models or name change.
//! - `models` is the one catalogue model whose name is TXT `name` (with or
//!   without the "Blackmagic " prefix, ignoring case), or every model of the
//!   integration when TXT `name` is absent or names none of them.
//! - A goodbye (a PTR with TTL 0, RFC 6762 §10.1) is reported as a
//!   `Discovery` message; there is no removal event. TTLs are not otherwise
//!   tracked.
//! - No connection is made to a found device: Videohub, SmartView and
//!   MultiView accept few clients, and opening the device reads its identity.

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Map, Value};
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::catalog::Catalog;
use crate::events::{Event, EventQueue};
use crate::session::Services;

pub(crate) const PROTOCOL: &str = "mdns";
const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const PORT: u16 = 5353;

/// The integrations mDNS discovery can find, as listed in
/// `crate::discovery::PROTOCOLS`.
pub(crate) const SPECS: &[&str] = &[
    "blackmagic-atem",
    "blackmagic-hyperdeck",
    "blackmagic-multiview",
    "blackmagic-smartview",
    "blackmagic-videohub",
];

/// One way to recognise a device: instances of `service` (whose TXT `class`
/// is `class`, when given) are devices of `spec`. `source` is what the rule
/// rests on.
#[derive(Debug, PartialEq)]
struct Rule {
    service: &'static str,
    class: Option<&'static str>,
    spec: &'static str,
    source: &'static str,
}

const BLACKMAGIC: &str = "_blackmagic._tcp.local";

const RULES: &[Rule] = &[
    Rule {
        service: BLACKMAGIC,
        class: Some("AtemSwitcher"),
        spec: "blackmagic-atem",
        source: "companion-module-bmd-atem: _blackmagic._tcp with TXT class=AtemSwitcher",
    },
    Rule {
        service: "_switcher_ctrl._udp.local",
        class: None,
        spec: "blackmagic-atem",
        source: "companion-module-bmd-atem: _switcher_ctrl._udp, any TXT",
    },
    Rule {
        service: BLACKMAGIC,
        class: Some("Videohub"),
        spec: "blackmagic-videohub",
        source: "companion-module-bmd-videohub: _blackmagic._tcp with TXT class=Videohub",
    },
    Rule {
        service: "_hyperdeck_ctrl._tcp.local",
        class: None,
        spec: "blackmagic-hyperdeck",
        source: "companion-module-bmd-hyperdeck: _hyperdeck_ctrl._tcp, any TXT",
    },
    Rule {
        service: BLACKMAGIC,
        class: Some("SmartView"),
        spec: "blackmagic-smartview",
        source: "companion-module-bmd-smartview: _blackmagic._tcp with TXT class=SmartView",
    },
    Rule {
        service: "_smartview._tcp.local",
        class: None,
        spec: "blackmagic-smartview",
        source: "smartview-client (MIT) README: browses _smartview._tcp for SmartView and SmartScope",
    },
    Rule {
        service: BLACKMAGIC,
        class: Some("MultiView"),
        spec: "blackmagic-multiview",
        source: "companion-module-bmd-multiview4: _blackmagic._tcp with TXT class=MultiView and name=Blackmagic MultiView 4",
    },
];

/// Seconds to wait after each round of queries: queries go out at 0, 1 and
/// 3 s (RFC 6762 §5.2: at least a second apart, the gap at least doubling),
/// and answers are heard for 2 s after the last.
const ROUND_WAITS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(2),
];
const MAX_HINTS: usize = 32;
/// A query never exceeds this, so it is never fragmented (RFC 6762 §17).
const MAX_QUERY: usize = 1200;
/// The largest mDNS message (RFC 6762 §17).
const MAX_MESSAGE: usize = 9000;
const MAX_KNOWN: usize = 256;
const MAX_ADDRESSES: usize = 4;
/// RFC 6762 §11.
const TTL: u32 = 255;

// --- DNS messages (RFC 1035 §4, RFC 6762 §18) -------------------------------

const TYPE_A: u16 = 1;
const TYPE_PTR: u16 = 12;
const TYPE_TXT: u16 = 16;
const TYPE_AAAA: u16 = 28;
const TYPE_SRV: u16 = 33;
const CLASS_IN: u16 = 1;
/// The top bit of a record's class is the cache-flush bit (RFC 6762 §10.2).
const CLASS_MASK: u16 = 0x7fff;
const MAX_NAME: usize = 255;

/// A domain name as its labels. Labels are bytes: an instance label may hold
/// any UTF-8, dots included (RFC 6763 §4.3).
#[derive(Debug, Clone, PartialEq)]
struct Name(Vec<Vec<u8>>);

impl Name {
    /// A name with no dot inside a label, such as a service type.
    fn dotted(text: &str) -> Name {
        Name(
            text.split('.')
                .filter(|l| !l.is_empty())
                .map(|l| l.as_bytes().to_vec())
                .collect(),
        )
    }

    /// Dotted text, a '.' or '\' inside a label escaped with '\'.
    fn text(&self) -> String {
        let mut out = String::new();
        for (i, label) in self.0.iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            for c in String::from_utf8_lossy(label).chars() {
                if c == '.' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
        }
        out
    }

    /// For comparing: names are equal without regard to ASCII case (§16).
    fn key(&self) -> String {
        self.text().to_ascii_lowercase()
    }

    fn parent(&self) -> Name {
        Name(self.0.iter().skip(1).cloned().collect())
    }

    /// The first label: an instance's own name.
    fn first(&self) -> String {
        self.0
            .first()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .unwrap_or_default()
    }

    /// Uncompressed wire form; `None` for a label over 63 bytes or a name
    /// over 255.
    fn encode(&self, out: &mut Vec<u8>) -> Option<()> {
        let length: usize = self.0.iter().map(|l| l.len() + 1).sum::<usize>() + 1;
        if length > MAX_NAME || self.0.iter().any(|l| l.is_empty() || l.len() > 63) {
            return None;
        }
        for label in &self.0 {
            out.push(label.len() as u8);
            out.extend_from_slice(label);
        }
        out.push(0);
        Some(())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Data {
    Ptr(Name),
    Srv { port: u16, target: Name },
    Txt(Vec<Vec<u8>>),
    A(Ipv4Addr),
    Aaaa(Ipv6Addr),
}

#[derive(Debug, Clone, PartialEq)]
struct Record {
    name: Name,
    ttl: u32,
    data: Data,
}

#[derive(Debug, PartialEq)]
struct Message {
    response: bool,
    /// Answers, authority and additional records alike: mDNS answers often
    /// arrive as additional records (RFC 6763 §12).
    records: Vec<Record>,
}

fn u16_at(packet: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*packet.get(at)?, *packet.get(at + 1)?]))
}

fn u32_at(packet: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(packet.get(at..at + 4)?.try_into().ok()?))
}

/// The name at `start`, following compression pointers (RFC 1035 §4.1.4),
/// and where the record stream continues after it. A pointer must point
/// before every place this name has been read from, so a name cannot loop.
fn read_name(packet: &[u8], start: usize) -> Option<(Name, usize)> {
    let mut labels = Vec::new();
    let mut at = start;
    let mut lowest = start;
    let mut resume = None;
    let mut length = 1;
    loop {
        let len = *packet.get(at)? as usize;
        match len & 0xc0 {
            0x00 if len == 0 => return Some((Name(labels), resume.unwrap_or(at + 1))),
            0x00 => {
                length += len + 1;
                if length > MAX_NAME {
                    return None;
                }
                labels.push(packet.get(at + 1..at + 1 + len)?.to_vec());
                at += 1 + len;
            }
            0xc0 => {
                let target = ((len & 0x3f) << 8) | *packet.get(at + 1)? as usize;
                resume.get_or_insert(at + 2);
                if target >= lowest {
                    return None;
                }
                lowest = target;
                at = target;
            }
            // 0x40 and 0x80 are extended label types no responder sends.
            _ => return None,
        }
    }
}

/// A DNS message's records of the types discovery reads, or `None` for a
/// message that is malformed or carries an opcode or rcode other than 0
/// (RFC 6762 §18.3, §18.11).
fn parse(packet: &[u8]) -> Option<Message> {
    let flags = u16_at(packet, 2)?;
    let count = |at| u16_at(packet, at).map(usize::from);
    let (questions, answers, authority, additional) = (count(4)?, count(6)?, count(8)?, count(10)?);
    if (flags >> 11) & 0xf != 0 || flags & 0xf != 0 {
        return None;
    }
    let mut at = 12;
    for _ in 0..questions {
        let (_, next) = read_name(packet, at)?;
        at = next + 4;
        if at > packet.len() {
            return None;
        }
    }
    let mut records = Vec::new();
    for _ in 0..answers + authority + additional {
        let (name, next) = read_name(packet, at)?;
        let rtype = u16_at(packet, next)?;
        let class = u16_at(packet, next + 2)? & CLASS_MASK;
        let ttl = u32_at(packet, next + 4)?;
        let start = next + 10;
        let end = start + usize::from(u16_at(packet, next + 8)?);
        let rdata = packet.get(start..end)?;
        at = end;
        if class != CLASS_IN {
            continue;
        }
        let data = match rtype {
            TYPE_A => Data::A(<[u8; 4]>::try_from(rdata).ok()?.into()),
            TYPE_AAAA => Data::Aaaa(<[u8; 16]>::try_from(rdata).ok()?.into()),
            TYPE_PTR => {
                let (target, after) = read_name(packet, start)?;
                if after != end {
                    return None;
                }
                Data::Ptr(target)
            }
            TYPE_SRV => {
                // priority, weight, port, target (RFC 2782).
                let port = u16_at(rdata, 4)?;
                let (target, after) = read_name(packet, start + 6)?;
                if after != end {
                    return None;
                }
                Data::Srv { port, target }
            }
            TYPE_TXT => {
                let mut strings = Vec::new();
                let mut i = 0;
                while i < rdata.len() {
                    let len = rdata[i] as usize;
                    strings.push(rdata.get(i + 1..i + 1 + len)?.to_vec());
                    i += 1 + len;
                }
                Data::Txt(strings)
            }
            _ => continue,
        };
        records.push(Record { name, ttl, data });
    }
    Some(Message {
        response: flags & 0x8000 != 0,
        records,
    })
}

/// A query (ID 0, RFC 6762 §18.1; QU clear, so answers are multicast, or
/// unicast to a query from another port) with as many of `questions` as fit
/// in `limit` bytes. Returns the message and how many questions it holds.
fn query(questions: &[(Name, u16)], limit: usize) -> (Vec<u8>, usize) {
    let mut out = vec![0u8; 12];
    let mut held = 0u16;
    for (name, qtype) in questions {
        let mut q = Vec::new();
        if name.encode(&mut q).is_none() {
            continue;
        }
        q.extend_from_slice(&qtype.to_be_bytes());
        q.extend_from_slice(&CLASS_IN.to_be_bytes());
        if out.len() + q.len() > limit {
            break;
        }
        out.extend_from_slice(&q);
        held += 1;
    }
    out[4..6].copy_from_slice(&held.to_be_bytes());
    (out, usize::from(held))
}

/// A TXT record's strings as keys and values (RFC 6763 §6): keys without
/// regard to case, the first of a repeated key kept, a string without '='
/// a key with no value, an empty key dropped.
fn txt_pairs(strings: &[Vec<u8>]) -> Vec<(String, Option<String>)> {
    let mut pairs: Vec<(String, Option<String>)> = Vec::new();
    for s in strings {
        let (key, value) = match s.iter().position(|&b| b == b'=') {
            Some(i) => (&s[..i], Some(&s[i + 1..])),
            None => (&s[..], None),
        };
        let key = String::from_utf8_lossy(key).into_owned();
        if key.is_empty() || pairs.iter().any(|(k, _)| k.eq_ignore_ascii_case(&key)) {
            continue;
        }
        pairs.push((key, value.map(|v| String::from_utf8_lossy(v).into_owned())));
    }
    pairs
}

fn txt_get<'a>(pairs: &'a [(String, Option<String>)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_deref())
}

// --- What a found instance is ------------------------------------------------

#[derive(Debug, PartialEq)]
enum Identity {
    Found(&'static Rule),
    /// The service needs a TXT `class` and no TXT has been heard.
    NeedsTxt,
    /// A `class` (or none) no rule knows.
    Unknown(Option<String>),
}

fn identify(service: &str, txt: Option<&[(String, Option<String>)]>) -> Identity {
    let rules: Vec<&'static Rule> = RULES.iter().filter(|r| r.service == service).collect();
    if let Some(rule) = rules.iter().find(|r| r.class.is_none()) {
        return Identity::Found(rule);
    }
    let Some(txt) = txt else {
        return Identity::NeedsTxt;
    };
    let class = txt_get(txt, "class");
    match rules.iter().find(|r| r.class.is_some() && r.class == class) {
        Some(rule) => Identity::Found(rule),
        None => Identity::Unknown(class.map(String::from)),
    }
}

/// The catalogue model named by `product` (TXT `name`): the one model whose
/// name is it, with or without the "Blackmagic " prefix, ignoring case.
fn model_named(models: &[(String, String)], product: &str) -> Option<String> {
    fn bare(s: &str) -> &str {
        let s = s.trim();
        match s.get(..11) {
            Some(p) if p.eq_ignore_ascii_case("Blackmagic ") => s[11..].trim_start(),
            _ => s,
        }
    }
    let wanted = bare(product);
    if wanted.is_empty() {
        return None;
    }
    let mut found = models
        .iter()
        .filter(|(_, name)| bare(name).eq_ignore_ascii_case(wanted));
    match (found.next(), found.next()) {
        (Some((id, _)), None) => Some(id.clone()),
        _ => None,
    }
}

/// What discovery needs of one integration in the core's catalogue.
#[derive(Debug)]
struct SpecInfo {
    port: u16,
    models: Vec<(String, String)>,
}

/// The integrations this core finds, and the service types to ask for.
#[derive(Debug)]
pub(crate) struct Table {
    specs: HashMap<&'static str, SpecInfo>,
    services: Vec<&'static str>,
}

impl Table {
    pub(crate) fn new(catalog: &Catalog) -> Table {
        let mut specs = HashMap::new();
        for id in SPECS {
            let Some(spec) = catalog.device(id) else {
                continue;
            };
            let port = spec
                .ports
                .iter()
                .find(|p| p.role == "control")
                .and_then(|p| p.port)
                .unwrap_or(0);
            let models = spec
                .models
                .iter()
                .map(|m| (m.id.clone(), m.name.clone()))
                .collect();
            specs.insert(*id, SpecInfo { port, models });
        }
        let mut services = Vec::new();
        for rule in RULES {
            if specs.contains_key(rule.spec) && !services.contains(&rule.service) {
                services.push(rule.service);
            }
        }
        Table { specs, services }
    }

    fn service(&self, key: &str) -> Option<&'static str> {
        self.services.iter().copied().find(|s| *s == key)
    }
}

// --- What has been heard -----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Heard {
    /// On the scan's socket: an answer to our query.
    Scan,
    /// On UDP 5353: an announcement, or an answer multicast to everyone.
    Listener,
}

struct Instance {
    name: Name,
    service: &'static str,
    srv: Option<(u16, Name)>,
    txt: Option<Vec<(String, Option<String>)>>,
    /// Where its records last came from.
    source: IpAddr,
    unknown_reported: bool,
    reported: Option<(IpAddr, &'static str)>,
}

#[derive(Default)]
struct Host {
    v4: Vec<Ipv4Addr>,
    v6: Vec<Ipv6Addr>,
}

/// One discovery session's memory (listen to stop), and what it reports.
struct Cache {
    table: Arc<Table>,
    instances: HashMap<String, Instance>,
    hosts: HashMap<String, Host>,
    reported: HashMap<(IpAddr, &'static str), (Vec<String>, String)>,
    full_reported: bool,
}

fn message(text: String) -> Event {
    Event::Discovery {
        protocol: PROTOCOL.into(),
        message: text,
    }
}

impl Cache {
    fn new(table: Arc<Table>) -> Cache {
        Cache {
            table,
            instances: HashMap::new(),
            hosts: HashMap::new(),
            reported: HashMap::new(),
            full_reported: false,
        }
    }

    fn full(&mut self, events: &mut Vec<Event>) {
        if !self.full_reported {
            self.full_reported = true;
            events.push(message(format!(
                "more than {MAX_KNOWN} services or hosts answered; further ones are ignored \
                 until discovery is stopped"
            )));
        }
    }

    /// The instance `name` of `service`, created if there is room.
    fn instance(
        &mut self,
        name: &Name,
        service: &'static str,
        from: IpAddr,
        events: &mut Vec<Event>,
    ) -> Option<&mut Instance> {
        let key = name.key();
        if !self.instances.contains_key(&key) {
            if self.instances.len() >= MAX_KNOWN {
                self.full(events);
                return None;
            }
            self.instances.insert(
                key.clone(),
                Instance {
                    name: name.clone(),
                    service,
                    srv: None,
                    txt: None,
                    source: from,
                    unknown_reported: false,
                    reported: None,
                },
            );
        }
        let instance = self.instances.get_mut(&key)?;
        instance.source = from;
        Some(instance)
    }

    /// Take in one message from `from`; returns the events it gives rise to.
    fn absorb(&mut self, from: IpAddr, message: &Message, heard: Heard) -> Vec<Event> {
        let mut events = Vec::new();
        if !message.response {
            return events;
        }
        let mut touched: Vec<String> = Vec::new();
        // PTRs first: instances of the service types asked for, and goodbyes.
        for r in &message.records {
            let Data::Ptr(target) = &r.data else {
                continue;
            };
            let owner = r.name.key();
            let Some(service) = self.table.service(&owner) else {
                continue;
            };
            // An instance is <Instance>.<Service> (RFC 6763 §4.1).
            if target.0.len() != r.name.0.len() + 1 || target.parent().key() != owner {
                continue;
            }
            if r.ttl == 0 {
                events.extend(self.goodbye(&target.key()));
                continue;
            }
            if self.instance(target, service, from, &mut events).is_some() {
                touched.push(target.key());
            }
        }
        // The SRV and TXT of instances of those service types.
        for r in &message.records {
            if r.ttl == 0 || !matches!(r.data, Data::Srv { .. } | Data::Txt(_)) {
                continue;
            }
            let Some(service) = self.table.service(&r.name.parent().key()) else {
                continue;
            };
            let Some(instance) = self.instance(&r.name, service, from, &mut events) else {
                continue;
            };
            match &r.data {
                Data::Srv { port, target } => instance.srv = Some((*port, target.clone())),
                Data::Txt(strings) => instance.txt = Some(txt_pairs(strings)),
                _ => {}
            }
            touched.push(r.name.key());
        }
        // The addresses of the hosts those SRVs name, and nothing else.
        let targets: BTreeSet<String> = self
            .instances
            .values()
            .filter_map(|i| i.srv.as_ref().map(|(_, t)| t.key()))
            .collect();
        for r in &message.records {
            if r.ttl == 0 || !matches!(r.data, Data::A(_) | Data::Aaaa(_)) {
                continue;
            }
            let key = r.name.key();
            if !targets.contains(&key) {
                continue;
            }
            if !self.hosts.contains_key(&key) && self.hosts.len() >= MAX_KNOWN {
                self.full(&mut events);
                continue;
            }
            let host = self.hosts.entry(key.clone()).or_default();
            match r.data {
                Data::A(a) if !host.v4.contains(&a) && host.v4.len() < MAX_ADDRESSES => {
                    host.v4.push(a)
                }
                Data::Aaaa(a) if !host.v6.contains(&a) && host.v6.len() < MAX_ADDRESSES => {
                    host.v6.push(a)
                }
                _ => {}
            }
            for (k, i) in &self.instances {
                if i.srv.as_ref().is_some_and(|(_, t)| t.key() == key) {
                    touched.push(k.clone());
                }
            }
        }
        let mut seen = BTreeSet::new();
        for key in touched {
            if seen.insert(key.clone()) {
                events.extend(self.evaluate(&key, heard));
            }
        }
        events
    }

    fn goodbye(&mut self, key: &str) -> Option<Event> {
        let instance = self.instances.remove(key)?;
        let reported = instance.reported?;
        if !self
            .instances
            .values()
            .any(|i| i.reported == Some(reported))
        {
            self.reported.remove(&reported);
        }
        Some(message(format!(
            "{} ({}) at {} announced it is leaving (mDNS goodbye)",
            instance.name.first(),
            instance.service,
            reported.0
        )))
    }

    /// The event for an instance whose identification or address is now
    /// complete or has changed.
    fn evaluate(&mut self, key: &str, heard: Heard) -> Option<Event> {
        let table = self.table.clone();
        let instance = self.instances.get_mut(key)?;
        let rule = match identify(instance.service, instance.txt.as_deref()) {
            Identity::Found(rule) => rule,
            Identity::NeedsTxt => return None,
            Identity::Unknown(class) => {
                if instance.unknown_reported {
                    return None;
                }
                instance.unknown_reported = true;
                return Some(message(format!(
                    "{} at {} advertises {} with TXT class {}, which no integration here is \
                     known to serve; it is not identified",
                    instance.name.first(),
                    instance.source,
                    instance.service,
                    class.map_or("(none)".into(), |c| format!("'{c}'")),
                )));
            }
        };
        let spec = table.specs.get(rule.spec)?;
        let (srv_port, target) = instance.srv.clone()?;
        let instance_name = instance.name.clone();
        let source = instance.source;
        let heard_txt = instance.txt.is_some();
        let txt = instance.txt.clone().unwrap_or_default();
        let (address, address_basis) =
            match self.hosts.get(&target.key()).and_then(|h| h.v4.first()) {
                Some(a) => (
                    IpAddr::V4(*a),
                    format!("the A record of the SRV target {}", target.text()),
                ),
                None => (
                    source,
                    format!(
                    "the address the answer came from: no A record for the SRV target {} was given",
                    target.text()
                ),
                ),
            };
        let product = txt_get(&txt, "name");
        let named = product.and_then(|p| model_named(&spec.models, p));
        let models: Vec<String> = match &named {
            Some(id) => vec![id.clone()],
            None => spec.models.iter().map(|(id, _)| id.clone()).collect(),
        };
        let name = instance_name.first();
        let reported_key = (address, rule.spec);
        if self.reported.get(&reported_key) == Some(&(models.clone(), name.clone())) {
            return None;
        }
        if !self.reported.contains_key(&reported_key) && self.reported.len() >= MAX_KNOWN {
            return None;
        }
        self.reported
            .insert(reported_key, (models.clone(), name.clone()));
        if let Some(instance) = self.instances.get_mut(key) {
            instance.reported = Some(reported_key);
        }
        let host = self.hosts.get(&target.key());
        let model_basis = match (product, &named) {
            (Some(p), Some(id)) => format!(
                "TXT name '{p}' is the name of catalogue model {id} (TXT name taken to be the \
                 product name, which Companion's MultiView 4 filter relies on)"
            ),
            (Some(p), None) => format!(
                "TXT name '{p}' is not the name of one catalogue model; every model of {} is possible",
                rule.spec
            ),
            (None, _) => format!(
                "the advertisement does not name the model; every model of {} is possible",
                rule.spec
            ),
        };
        let identification = match rule.class {
            Some(c) => format!(
                "{} with TXT class {c} is {} ({})",
                rule.service, rule.spec, rule.source
            ),
            None => format!("{} is {} ({})", rule.service, rule.spec, rule.source),
        };
        let mut txt_json = Map::new();
        for (k, v) in &txt {
            txt_json.insert(k.clone(), v.as_ref().map_or(Value::Null, |v| json!(v)));
        }
        Some(Event::Discovered {
            protocol: PROTOCOL.into(),
            address: address.to_string(),
            port: spec.port,
            device: rule.spec.into(),
            models,
            name: Some(name),
            evidence: json!({
                "protocol": match heard {
                    Heard::Scan => "answered an mDNS query (DNS-SD browse)",
                    Heard::Listener => "heard on UDP 5353: an mDNS announcement or multicast answer",
                },
                "service": rule.service,
                "instance": instance_name.text(),
                "txt": if heard_txt { Value::Object(txt_json) } else { Value::Null },
                "srv": {"port": srv_port, "target": target.text()},
                "ipv4": host.map(|h| h.v4.iter().map(|a| a.to_string()).collect::<Vec<_>>()),
                "ipv6": host.map(|h| h.v6.iter().map(|a| a.to_string()).collect::<Vec<_>>()),
                "address": address_basis,
                "port": "the integration's control port; the SRV port is evidence only",
                "identification": identification,
                "model": model_basis,
                "name": "the DNS-SD instance name, which can usually be changed on the device, so weak evidence of identity",
            }),
        })
    }

    /// Questions for what instances still lack: SRV, TXT where the class is
    /// needed, and the A record of SRV targets without one.
    fn follow_ups(&self) -> Vec<(Name, u16)> {
        let mut keys: Vec<&String> = self.instances.keys().collect();
        keys.sort();
        let mut questions = Vec::new();
        for key in keys {
            let i = &self.instances[key];
            if matches!(identify(i.service, i.txt.as_deref()), Identity::Unknown(_)) {
                continue;
            }
            if i.srv.is_none() {
                questions.push((i.name.clone(), TYPE_SRV));
            }
            if identify(i.service, i.txt.as_deref()) == Identity::NeedsTxt {
                questions.push((i.name.clone(), TYPE_TXT));
            }
            if let Some((_, target)) = &i.srv {
                let resolved = self
                    .hosts
                    .get(&target.key())
                    .is_some_and(|h| !h.v4.is_empty());
                let question = (target.clone(), TYPE_A);
                if !resolved && !questions.contains(&question) {
                    questions.push(question);
                }
            }
        }
        questions
    }
}

// --- Sockets -----------------------------------------------------------------

/// What the listener, scans and their sockets share in one session.
struct Shared {
    events: Arc<EventQueue>,
    cache: Mutex<Cache>,
}

impl Shared {
    fn handle(&self, from: IpAddr, data: &[u8], heard: Heard) {
        let Some(message) = parse(data) else {
            return;
        };
        let events = self.cache.lock().unwrap().absorb(from, &message, heard);
        for e in events {
            self.events.push(e);
        }
    }

    fn log(&self, text: String) {
        self.events.push(message(text));
    }
}

/// UDP 5353, shared with the host's own mDNS responder.
fn bind_listener() -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    // mDNSResponder binds 5353 with SO_REUSEPORT, so sharing it needs it too.
    #[cfg(all(
        unix,
        not(any(target_os = "solaris", target_os = "illumos", target_os = "cygwin"))
    ))]
    let _ = socket.set_reuse_port(true);
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
    socket.set_multicast_ttl_v4(TTL)?;
    socket.set_ttl(TTL)?;
    socket.set_nonblocking(true)?;
    let local = match bind {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
    };
    socket.bind(&SocketAddr::new(IpAddr::V4(local), 0).into())?;
    UdpSocket::from_std(socket.into())
}

async fn receive(socket: &UdpSocket, shared: &Shared, window: Duration, buf: &mut [u8]) {
    let until = tokio::time::Instant::now() + window;
    loop {
        match tokio::time::timeout_at(until, socket.recv_from(buf)).await {
            Err(_) => return,
            Ok(Ok((n, from))) => shared.handle(from.ip(), &buf[..n], Heard::Scan),
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
    let browse: Vec<(Name, u16)> = {
        let cache = shared.cache.lock().unwrap();
        cache
            .table
            .services
            .iter()
            .map(|s| (Name::dotted(s), TYPE_PTR))
            .collect()
    };
    let mut buf = vec![0u8; MAX_MESSAGE];
    for wait in ROUND_WAITS {
        let mut questions = browse.clone();
        questions.extend(shared.cache.lock().unwrap().follow_ups());
        let (packet, _) = query(&questions, MAX_QUERY);
        for ip in &interfaces {
            if SockRef::from(&socket).set_multicast_if_v4(ip).is_ok() {
                let _ = socket.send_to(&packet, group).await;
            }
        }
        for hint in &hints {
            let _ = socket
                .send_to(&packet, SocketAddr::new(IpAddr::V4(*hint), PORT))
                .await;
        }
        receive(&socket, &shared, wait, &mut buf).await;
    }
}

#[derive(Default)]
struct State {
    shared: Option<Arc<Shared>>,
    listener: Option<(Arc<UdpSocket>, JoinHandle<()>)>,
    joined: BTreeSet<Ipv4Addr>,
    scan: Option<JoinHandle<()>>,
}

pub(crate) struct Mdns {
    table: Arc<Table>,
    state: Mutex<State>,
}

impl Mdns {
    pub(crate) fn new(catalog: &Catalog) -> Mdns {
        Mdns {
            table: Arc::new(Table::new(catalog)),
            state: Mutex::default(),
        }
    }

    fn shared(&self, state: &mut State, services: &Services) -> Arc<Shared> {
        state
            .shared
            .get_or_insert_with(|| {
                Arc::new(Shared {
                    events: services.events.clone(),
                    cache: Mutex::new(Cache::new(self.table.clone())),
                })
            })
            .clone()
    }

    /// Hear announcements and multicast answers. If UDP 5353 cannot be had,
    /// that is reported and not an error: scans still work. Interfaces that
    /// appeared since are joined on every call.
    pub(crate) fn listen(&self, services: &Arc<Services>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let shared = self.shared(&mut state, services);
        if state.listener.is_none() {
            let socket = match bind_listener() {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    shared.log(format!(
                        "cannot listen for mDNS announcements on UDP {PORT} ({e}); another \
                         program may hold it exclusively. Scans still find devices"
                    ));
                    return Ok(());
                }
            };
            let task = {
                let socket = socket.clone();
                let shared = shared.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; MAX_MESSAGE];
                    loop {
                        match socket.recv_from(&mut buf).await {
                            Ok((n, from)) => shared.handle(from.ip(), &buf[..n], Heard::Listener),
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
                    "cannot join the mDNS group on {ip} ({e}); announcements there will not be heard"
                )),
            }
        }
        Ok(())
    }

    pub(crate) fn scan(&self, services: &Arc<Services>, hints: &[Ipv4Addr]) {
        let mut state = self.state.lock().unwrap();
        if state.scan.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let shared = self.shared(&mut state, services);
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
                "{} hints given; only the first {MAX_HINTS} are asked by unicast query",
                unique.len()
            ));
            unique.truncate(MAX_HINTS);
        }
        if interfaces.is_empty() && unique.is_empty() {
            shared.log("no IPv4 interface to search on, and no hints".into());
            return;
        }
        state.scan = Some(tokio::spawn(search(
            shared,
            services.bind_address,
            interfaces,
            unique,
        )));
    }

    /// Stop listening and scanning, and forget every device, so the next
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
        state.shared = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
    const DEVICE: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 20));

    fn table() -> Arc<Table> {
        Arc::new(Table::new(&Catalog::source_tree()))
    }

    // Wire pieces, byte for byte from RFC 1035 §4.1 and RFC 6762 §18.

    /// `_blackmagic._tcp.local`, uncompressed: 24 bytes.
    const BLACKMAGIC_NAME: &[u8] = b"\x0b_blackmagic\x04_tcp\x05local\x00";

    /// A response header: ID 0, QR and AA set (0x8400), no questions.
    fn header(answers: u16, additional: u16) -> Vec<u8> {
        let mut h = vec![0x00, 0x00, 0x84, 0x00, 0x00, 0x00];
        h.extend_from_slice(&answers.to_be_bytes());
        h.extend_from_slice(&[0x00, 0x00]);
        h.extend_from_slice(&additional.to_be_bytes());
        h
    }

    /// Type, class (IN, with the cache-flush bit when `flush`), TTL and the
    /// rdata with its length.
    fn rr(out: &mut Vec<u8>, rtype: u16, flush: bool, ttl: u32, rdata: &[u8]) {
        out.extend_from_slice(&rtype.to_be_bytes());
        out.extend_from_slice(&(if flush { 0x8001u16 } else { 0x0001 }).to_be_bytes());
        out.extend_from_slice(&ttl.to_be_bytes());
        out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        out.extend_from_slice(rdata);
    }

    fn txt_rdata(strings: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in strings {
            out.push(s.len() as u8);
            out.extend_from_slice(s.as_bytes());
        }
        out
    }

    /// An ATEM's full answer, as a responder compresses it: the PTR answer,
    /// then SRV, TXT, A and AAAA as additional records.
    ///
    /// ```text
    ///  0 header (12)
    /// 12 _blackmagic._tcp.local            PTR  ttl 4500
    /// 46   -> "\x0dATEM Mini Pro" + ptr 12  (instance at offset 46)
    /// 62 ptr 46                            SRV  flush ttl 120  0 0 9910 "\x0cATEM-7c2e0d" + ptr 29 (".local")
    ///    ptr 46                            TXT  flush ttl 4500
    ///    "\x0cATEM-7c2e0d" target via ptr  A    flush ttl 120  10.0.0.20
    ///    target via ptr                    AAAA flush ttl 120  fe80::1
    /// ```
    fn atem_response(txt: &[&str]) -> Vec<u8> {
        let mut p = header(1, 4);
        assert_eq!(p.len(), 12);
        p.extend_from_slice(BLACKMAGIC_NAME); // 12..36; "_tcp" at 24, "local" at 29
        let mut instance = b"\x0dATEM Mini Pro".to_vec();
        instance.extend_from_slice(&[0xc0, 12]);
        rr(&mut p, TYPE_PTR, false, 4500, &instance); // rdata at 46
        assert_eq!(&p[46..60], b"\x0dATEM Mini Pro");
        // SRV owner: pointer to the instance name at 46.
        p.extend_from_slice(&[0xc0, 46]);
        let mut srv = vec![0, 0, 0, 0, 0x26, 0xb6]; // priority 0, weight 0, port 9910
        let target_at = p.len() + 10 + srv.len();
        srv.extend_from_slice(b"\x0bATEM-7c2e0d");
        srv.extend_from_slice(&[0xc0, 29]);
        rr(&mut p, TYPE_SRV, true, 120, &srv);
        p.extend_from_slice(&[0xc0, 46]);
        rr(&mut p, TYPE_TXT, true, 4500, &txt_rdata(txt));
        p.extend_from_slice(&[0xc0, target_at as u8]);
        rr(&mut p, TYPE_A, true, 120, &[10, 0, 0, 20]);
        p.extend_from_slice(&[0xc0, target_at as u8]);
        let mut v6 = [0u8; 16];
        v6[0] = 0xfe;
        v6[1] = 0x80;
        v6[15] = 1;
        rr(&mut p, TYPE_AAAA, true, 120, &v6);
        p
    }

    fn atem_txt() -> Vec<&'static str> {
        vec![
            "txtvers=1",
            "class=AtemSwitcher",
            "name=Blackmagic ATEM Mini Pro",
            "protocol version=2.30",
            "unique id=7c2e0d16505d",
        ]
    }

    #[test]
    fn discovery_lists_what_this_protocol_reports() {
        assert!(crate::discovery::PROTOCOLS.contains(&(PROTOCOL, SPECS)));
        for rule in RULES {
            assert!(SPECS.contains(&rule.spec), "{}", rule.spec);
        }
        for spec in SPECS {
            assert!(RULES.iter().any(|r| r.spec == *spec), "{spec}");
        }
    }

    #[test]
    fn a_query_is_byte_exact() {
        let (packet, held) = query(&[(Name::dotted(BLACKMAGIC), TYPE_PTR)], MAX_QUERY);
        let mut expected = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        expected.extend_from_slice(BLACKMAGIC_NAME);
        expected.extend_from_slice(&[0x00, 0x0c, 0x00, 0x01]);
        assert_eq!(held, 1);
        assert_eq!(packet, expected);
        // Our own query, echoed back to the listener, is not a response.
        assert!(!parse(&packet).unwrap().response);

        // Questions beyond the limit are left out, and the count says so.
        let many: Vec<(Name, u16)> = (0..100)
            .map(|n| (Name::dotted(&format!("host-{n}.local")), TYPE_A))
            .collect();
        let (packet, held) = query(&many, MAX_QUERY);
        assert!(packet.len() <= MAX_QUERY);
        assert!(held < 100);
        assert_eq!(u16_at(&packet, 4), Some(held as u16));
        // A label over 63 bytes cannot be asked.
        let long = Name(vec![vec![b'a'; 64], b"local".to_vec()]);
        assert_eq!(query(&[(long, TYPE_SRV)], MAX_QUERY).1, 0);
    }

    #[test]
    fn a_compressed_response_is_parsed() {
        let packet = atem_response(&atem_txt());
        let m = parse(&packet).unwrap();
        assert!(m.response);
        let instance = Name(vec![
            b"ATEM Mini Pro".to_vec(),
            b"_blackmagic".to_vec(),
            b"_tcp".to_vec(),
            b"local".to_vec(),
        ]);
        let target = Name::dotted("ATEM-7c2e0d.local");
        assert_eq!(m.records.len(), 5);
        assert_eq!(m.records[0].name, Name::dotted(BLACKMAGIC));
        assert_eq!(m.records[0].ttl, 4500);
        assert_eq!(m.records[0].data, Data::Ptr(instance.clone()));
        assert_eq!(m.records[1].name, instance);
        assert_eq!(
            m.records[1].data,
            Data::Srv {
                port: 9910,
                target: target.clone()
            }
        );
        let Data::Txt(strings) = &m.records[2].data else {
            panic!("TXT");
        };
        assert_eq!(strings[1], b"class=AtemSwitcher");
        assert_eq!(m.records[3].name, target);
        assert_eq!(m.records[3].data, Data::A(Ipv4Addr::new(10, 0, 0, 20)));
        assert_eq!(m.records[4].data, Data::Aaaa("fe80::1".parse().unwrap()));
        assert_eq!(instance.text(), "ATEM Mini Pro._blackmagic._tcp.local");
    }

    #[test]
    fn malformed_messages_are_dropped_whole() {
        let good = atem_response(&atem_txt());
        // Every truncation fails cleanly.
        for n in 0..good.len() {
            assert!(parse(&good[..n]).is_none(), "truncated at {n}");
        }
        // A pointer to itself, or forwards, would loop or read ahead.
        let mut p = header(1, 0);
        p.extend_from_slice(&[0xc0, 12]);
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0, 1]);
        assert!(parse(&p).is_none(), "self pointer");
        let mut p = header(1, 0);
        p.extend_from_slice(&[0xc0, 40]);
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0, 1]);
        assert!(parse(&p).is_none(), "forward pointer");
        // Two names pointing at each other.
        let mut p = header(2, 0);
        p.extend_from_slice(b"\x01a\xc0\x1b"); // 12: "a" then -> 27
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0, 1]); // 16..30
        assert_eq!(p.len(), 30);
        let mut q = p.clone();
        q.truncate(27);
        q.extend_from_slice(b"\x01b\xc0\x0c");
        assert!(parse(&q).is_none(), "pointer loop");
        // A name over 255 bytes.
        let mut p = header(1, 0);
        for _ in 0..5 {
            p.push(63);
            p.extend_from_slice(&[b'x'; 63]);
        }
        p.push(0);
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0, 1]);
        assert!(parse(&p).is_none(), "long name");
        // Extended label types.
        let mut p = header(1, 0);
        p.extend_from_slice(&[0x41, 0x00]);
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0, 1]);
        assert!(parse(&p).is_none(), "label type 01");
        // A PTR whose name runs past its rdata length.
        let mut p = header(1, 0);
        p.extend_from_slice(BLACKMAGIC_NAME);
        rr(&mut p, TYPE_PTR, false, 120, b"\x01a\xc0\x0c\x00");
        assert!(parse(&p).is_none(), "PTR rdata longer than its name");
        // An A record of the wrong length.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x01h\x05local\x00");
        rr(&mut p, TYPE_A, false, 120, &[10, 0, 0]);
        assert!(parse(&p).is_none(), "short A");
        // A TXT string running past the record.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x01h\x05local\x00");
        rr(&mut p, TYPE_TXT, false, 120, b"\x05ab");
        assert!(parse(&p).is_none(), "TXT string overrun");
        // A non-zero rcode or opcode is ignored (RFC 6762 §18.3, §18.11).
        let mut p = good.clone();
        p[3] = 0x03;
        assert!(parse(&p).is_none(), "rcode");
        let mut p = good.clone();
        p[2] |= 0x08;
        assert!(parse(&p).is_none(), "opcode");
        assert!(parse(b"\xff\xfe").is_none());
    }

    #[test]
    fn txt_keys_follow_rfc_6763() {
        let strings: Vec<Vec<u8>> = [
            &b"class=AtemSwitcher"[..],
            b"CLASS=Videohub",
            b"flag",
            b"=nokey",
            b"empty=",
            b"eq=a=b",
        ]
        .iter()
        .map(|s| s.to_vec())
        .collect();
        let pairs = txt_pairs(&strings);
        assert_eq!(
            txt_get(&pairs, "Class"),
            Some("AtemSwitcher"),
            "first wins, any case"
        );
        assert_eq!(txt_get(&pairs, "flag"), None);
        assert!(pairs.iter().any(|(k, v)| k == "flag" && v.is_none()));
        assert_eq!(txt_get(&pairs, "empty"), Some(""));
        assert_eq!(txt_get(&pairs, "eq"), Some("a=b"));
        assert_eq!(pairs.len(), 4, "no empty key, no repeated key");
    }

    #[test]
    fn the_table_maps_service_types_and_classes_to_integrations() {
        let txt = |class: &str| vec![("class".to_string(), Some(class.to_string()))];
        let found = |service: &str, class: &str| match identify(service, Some(&txt(class))) {
            Identity::Found(r) => Some(r.spec),
            _ => None,
        };
        assert_eq!(found(BLACKMAGIC, "AtemSwitcher"), Some("blackmagic-atem"));
        assert_eq!(found(BLACKMAGIC, "Videohub"), Some("blackmagic-videohub"));
        assert_eq!(found(BLACKMAGIC, "SmartView"), Some("blackmagic-smartview"));
        assert_eq!(found(BLACKMAGIC, "MultiView"), Some("blackmagic-multiview"));
        assert_eq!(
            found("_switcher_ctrl._udp.local", ""),
            Some("blackmagic-atem")
        );
        assert_eq!(
            found("_hyperdeck_ctrl._tcp.local", ""),
            Some("blackmagic-hyperdeck")
        );
        assert_eq!(
            found("_smartview._tcp.local", ""),
            Some("blackmagic-smartview")
        );
        // The class is compared exactly, as Companion's filter does.
        assert_eq!(
            identify(BLACKMAGIC, Some(&txt("atemswitcher"))),
            Identity::Unknown(Some("atemswitcher".into()))
        );
        assert_eq!(
            identify(BLACKMAGIC, Some(&txt("Teranex"))),
            Identity::Unknown(Some("Teranex".into()))
        );
        assert_eq!(identify(BLACKMAGIC, Some(&[])), Identity::Unknown(None));
        assert_eq!(identify(BLACKMAGIC, None), Identity::NeedsTxt);
        // Without a class-less rule, a class is needed; with one, it is not.
        assert!(matches!(
            identify("_hyperdeck_ctrl._tcp.local", None),
            Identity::Found(_)
        ));
    }

    #[test]
    fn txt_name_narrows_models_only_when_it_names_one() {
        let t = table();
        let atem = &t.specs["blackmagic-atem"].models;
        assert_eq!(
            model_named(atem, "Blackmagic ATEM Mini Pro").as_deref(),
            Some("atem-mini-pro")
        );
        assert_eq!(
            model_named(atem, "ATEM Mini Pro ISO").as_deref(),
            Some("atem-mini-pro-iso")
        );
        assert_eq!(model_named(atem, "atem mini").as_deref(), Some("atem-mini"));
        assert_eq!(model_named(atem, "ATEM Mini Pro Stage Left"), None);
        assert_eq!(model_named(atem, ""), None);
        let multiview = &t.specs["blackmagic-multiview"].models;
        assert_eq!(
            model_named(multiview, "Blackmagic MultiView 4").as_deref(),
            Some("multiview-4")
        );
        assert_eq!(t.specs["blackmagic-atem"].port, 9910);
        assert_eq!(t.specs["blackmagic-hyperdeck"].port, 9993);
    }

    #[test]
    fn a_core_asks_only_for_the_services_of_its_integrations() {
        let all = table();
        assert_eq!(
            all.services,
            [
                BLACKMAGIC,
                "_switcher_ctrl._udp.local",
                "_hyperdeck_ctrl._tcp.local",
                "_smartview._tcp.local"
            ]
        );
        let catalog = Catalog::source_tree();
        let only = |ids: &[&str]| Catalog {
            devices: ids
                .iter()
                .map(|id| (id.to_string(), catalog.device(id).unwrap().clone()))
                .collect(),
        };
        let decks = Table::new(&only(&["blackmagic-hyperdeck"]));
        assert_eq!(decks.services, ["_hyperdeck_ctrl._tcp.local"]);
        // A Videohub answering a core without the Videohub integration is
        // not reported.
        let switchers = Arc::new(Table::new(&only(&["blackmagic-atem"])));
        let mut cache = Cache::new(switchers);
        let hub = atem_response(&["class=Videohub", "name=Blackmagic Videohub 40x40 12G"]);
        assert!(cache
            .absorb(DEVICE, &parse(&hub).unwrap(), Heard::Scan)
            .is_empty());
    }

    fn discovered(e: &Event) -> (&str, &str, u16, &str, &[String], &Value) {
        let Event::Discovered {
            protocol,
            address,
            port,
            device,
            models,
            evidence,
            ..
        } = e
        else {
            panic!("expected Discovered, got {e:?}");
        };
        (protocol, address, *port, device, models, evidence)
    }

    #[test]
    fn an_atem_is_reported_once_with_its_evidence() {
        let mut cache = Cache::new(table());
        let packet = parse(&atem_response(&atem_txt())).unwrap();
        let events = cache.absorb(LOCAL, &packet, Heard::Scan);
        assert_eq!(events.len(), 1);
        let (protocol, address, port, device, models, evidence) = discovered(&events[0]);
        assert_eq!(
            (protocol, address, port, device),
            ("mdns", "10.0.0.20", 9910, "blackmagic-atem")
        );
        assert_eq!(models, ["atem-mini-pro"]);
        let Event::Discovered { name, .. } = &events[0] else {
            unreachable!()
        };
        assert_eq!(name.as_deref(), Some("ATEM Mini Pro"));
        assert_eq!(evidence["service"], "_blackmagic._tcp.local");
        assert_eq!(evidence["instance"], "ATEM Mini Pro._blackmagic._tcp.local");
        assert_eq!(evidence["txt"]["class"], "AtemSwitcher");
        assert_eq!(evidence["txt"]["unique id"], "7c2e0d16505d");
        assert_eq!(
            evidence["srv"],
            json!({"port": 9910, "target": "ATEM-7c2e0d.local"})
        );
        assert_eq!(evidence["ipv4"], json!(["10.0.0.20"]));
        assert_eq!(evidence["ipv6"], json!(["fe80::1"]));
        assert!(evidence["address"]
            .as_str()
            .unwrap()
            .starts_with("the A record"));
        assert!(evidence["identification"]
            .as_str()
            .unwrap()
            .contains("companion-module-bmd-atem"));

        // The same answer again, or heard by the listener, reports nothing.
        assert!(cache.absorb(LOCAL, &packet, Heard::Scan).is_empty());
        assert!(cache.absorb(LOCAL, &packet, Heard::Listener).is_empty());

        // A TXT that no longer names one model widens the models: reported again.
        let renamed = parse(&atem_response(&["class=AtemSwitcher"])).unwrap();
        let events = cache.absorb(LOCAL, &renamed, Heard::Listener);
        let (.., models, evidence) = discovered(&events[0]);
        assert_eq!(models.len(), table().specs["blackmagic-atem"].models.len());
        assert!(evidence["model"].as_str().unwrap().contains("every model"));

        // A goodbye is a message, and the device is reported again when it returns.
        let mut bye = header(1, 0);
        bye.extend_from_slice(BLACKMAGIC_NAME);
        let mut instance = b"\x0dATEM Mini Pro".to_vec();
        instance.extend_from_slice(&[0xc0, 12]);
        rr(&mut bye, TYPE_PTR, false, 0, &instance);
        let events = cache.absorb(LOCAL, &parse(&bye).unwrap(), Heard::Listener);
        let [Event::Discovery { message, .. }] = &events[..] else {
            panic!("{events:?}");
        };
        assert!(
            message.contains("ATEM Mini Pro") && message.contains("leaving"),
            "{message}"
        );
        assert_eq!(cache.absorb(LOCAL, &packet, Heard::Scan).len(), 1);
    }

    #[test]
    fn a_ptr_alone_asks_for_the_rest() {
        let mut cache = Cache::new(table());
        // HyperDeck answers the browse with the PTR only.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x0f_hyperdeck_ctrl\x04_tcp\x05local\x00");
        rr(&mut p, TYPE_PTR, false, 4500, b"\x0aHyperDeck1\xc0\x0c");
        assert!(cache
            .absorb(DEVICE, &parse(&p).unwrap(), Heard::Scan)
            .is_empty());
        let deck = Name::dotted("HyperDeck1._hyperdeck_ctrl._tcp.local");
        assert_eq!(cache.follow_ups(), [(deck.clone(), TYPE_SRV)]);

        // Its SRV, without an A record: reported at the answer's address,
        // and the target's A record is asked for.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x0aHyperDeck1\x0f_hyperdeck_ctrl\x04_tcp\x05local\x00");
        rr(
            &mut p,
            TYPE_SRV,
            true,
            120,
            b"\x00\x00\x00\x00\x27\x09\x09HyperDeck\x05local\x00",
        );
        let events = cache.absorb(DEVICE, &parse(&p).unwrap(), Heard::Scan);
        let (_, address, port, device, models, evidence) = discovered(&events[0]);
        assert_eq!(
            (address, port, device),
            ("10.0.0.20", 9993, "blackmagic-hyperdeck")
        );
        assert_eq!(
            models.len(),
            table().specs["blackmagic-hyperdeck"].models.len()
        );
        assert!(evidence["address"]
            .as_str()
            .unwrap()
            .starts_with("the address the answer came from"));
        assert_eq!(evidence["txt"], Value::Null);
        assert_eq!(
            cache.follow_ups(),
            [(Name::dotted("HyperDeck.local"), TYPE_A)]
        );

        // The A record at the same address changes nothing reported.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x09HyperDeck\x05local\x00");
        rr(&mut p, TYPE_A, true, 120, &[10, 0, 0, 20]);
        assert!(cache
            .absorb(DEVICE, &parse(&p).unwrap(), Heard::Scan)
            .is_empty());
        assert!(cache.follow_ups().is_empty());

        // An address for a host no instance names is not kept.
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x05other\x05local\x00");
        rr(&mut p, TYPE_A, true, 120, &[10, 0, 0, 99]);
        cache.absorb(DEVICE, &parse(&p).unwrap(), Heard::Listener);
        assert!(!cache.hosts.contains_key("other.local"));
    }

    #[test]
    fn a_blackmagic_instance_needs_its_class_and_an_unknown_class_is_told_once() {
        let mut cache = Cache::new(table());
        let mut p = header(1, 0);
        p.extend_from_slice(BLACKMAGIC_NAME);
        rr(&mut p, TYPE_PTR, false, 4500, b"\x0bWeb Present\xc0\x0c");
        let m = parse(&p).unwrap();
        assert!(cache.absorb(DEVICE, &m, Heard::Scan).is_empty());
        let instance = Name::dotted("Web Present._blackmagic._tcp.local");
        assert_eq!(
            cache.follow_ups(),
            [(instance.clone(), TYPE_SRV), (instance, TYPE_TXT)]
        );
        let unknown = parse(&atem_response(&["class=WebPresenter"])).unwrap();
        let events = cache.absorb(DEVICE, &unknown, Heard::Scan);
        let [Event::Discovery { message, .. }] = &events[..] else {
            panic!("{events:?}");
        };
        assert!(message.contains("'WebPresenter'"), "{message}");
        assert!(cache.absorb(DEVICE, &unknown, Heard::Scan).is_empty());
        // Nothing more is asked about a device whose class is unknown.
        assert!(!cache
            .follow_ups()
            .iter()
            .any(|(n, _)| n.first() == "ATEM Mini Pro"));
    }

    #[test]
    fn instances_of_other_services_and_queries_are_ignored() {
        let mut cache = Cache::new(table());
        let mut p = header(1, 0);
        p.extend_from_slice(b"\x05_http\x04_tcp\x05local\x00");
        rr(&mut p, TYPE_PTR, false, 4500, b"\x07Printer\xc0\x0c");
        assert!(cache
            .absorb(DEVICE, &parse(&p).unwrap(), Heard::Listener)
            .is_empty());
        assert!(cache.instances.is_empty());
        // A PTR to a name outside the service is not an instance of it.
        let mut p = header(1, 0);
        p.extend_from_slice(BLACKMAGIC_NAME);
        rr(
            &mut p,
            TYPE_PTR,
            false,
            4500,
            b"\x07Printer\x05_http\x04_tcp\x05local\x00",
        );
        assert!(cache
            .absorb(DEVICE, &parse(&p).unwrap(), Heard::Listener)
            .is_empty());
        assert!(cache.instances.is_empty());
        // A query carrying known answers is not a response.
        let mut q = atem_response(&atem_txt());
        q[2] = 0x00;
        assert!(cache
            .absorb(DEVICE, &parse(&q).unwrap(), Heard::Listener)
            .is_empty());
    }

    #[test]
    fn memory_is_bounded() {
        let mut cache = Cache::new(table());
        let mut full = 0;
        for n in 0..MAX_KNOWN + 10 {
            let mut p = header(1, 0);
            p.extend_from_slice(BLACKMAGIC_NAME);
            let label = format!("dev{n}");
            let mut rdata = vec![label.len() as u8];
            rdata.extend_from_slice(label.as_bytes());
            rdata.extend_from_slice(&[0xc0, 12]);
            rr(&mut p, TYPE_PTR, false, 4500, &rdata);
            full += cache
                .absorb(DEVICE, &parse(&p).unwrap(), Heard::Listener)
                .len();
        }
        assert_eq!(cache.instances.len(), MAX_KNOWN);
        assert_eq!(full, 1, "being full is said once");
        // The follow-ups still fit one datagram.
        let (packet, _) = query(&cache.follow_ups(), MAX_QUERY);
        assert!(packet.len() <= MAX_QUERY);
    }

    #[test]
    fn a_scan_is_bounded() {
        let total: Duration = ROUND_WAITS.iter().sum();
        assert_eq!(total, Duration::from_secs(5));
        assert_eq!(ROUND_WAITS[0], Duration::from_secs(1));
        assert!(
            ROUND_WAITS[1] >= ROUND_WAITS[0] * 2,
            "RFC 6762 §5.2 spacing"
        );
        assert!(ROUND_WAITS.len() * MAX_HINTS <= 96);
        const { assert!(MAX_QUERY < 1460) };
    }
}
