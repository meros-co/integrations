//! An Ember+ provider simulated on real TCP, driven through the public API:
//! S101 framing (escaping, CRC, the EmBER packet header) around BER-encoded
//! Glow (Lawo, Ember+ Specification 2.50). The simulator has its own small
//! BER and S101 code, written separately from the module's.
//!
//! Its tree: node 1 "Device" holding parameter 1.1 "gain" (real, -128 to 15,
//! read/write), parameter 1.3 "level" (integer, stream identifier 7), matrix
//! 1.4 "router" (1:N, linear 4x4) and function 1.5 "add" (two integers, one
//! integer result). It answers GetDirectory, value changes with the value it
//! now holds, matrix connects with the connection's new state, Subscribe on
//! the level with a StreamCollection, and Invoke with an InvocationResult.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// --- BER, as the simulator writes it -----------------------------------------

fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = content.len();
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0x100 {
        out.extend([0x81, n as u8]);
    } else {
        out.extend([0x82, (n >> 8) as u8, n as u8]);
    }
    out.extend_from_slice(content);
    out
}

fn int(v: i64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let mut start = 0;
    while start < 7
        && ((b[start] == 0 && b[start + 1] & 0x80 == 0)
            || (b[start] == 0xFF && b[start + 1] & 0x80 != 0))
    {
        start += 1;
    }
    tlv(0x02, &b[start..])
}

/// A REAL whose value is an integer or a half-integer: mantissa, exponent 0
/// or -1, base 2.
fn real(v: f64) -> Vec<u8> {
    let (m, e) = if v.fract() == 0.0 {
        (v.abs() as u64, 0u8)
    } else {
        ((v.abs() * 2.0) as u64, 0xFF)
    };
    let sign = if v < 0.0 { 0x40 } else { 0 };
    let mut c = vec![0x80 | sign, e];
    let mb = m.to_be_bytes();
    let skip = mb.iter().take_while(|&&x| x == 0).count().min(7);
    c.extend_from_slice(&mb[skip..]);
    tlv(0x09, &c)
}

fn utf8(s: &str) -> Vec<u8> {
    tlv(0x0C, s.as_bytes())
}

fn roid(path: &[u8]) -> Vec<u8> {
    tlv(0x0D, path)
}

fn ctx(n: u8, inner: Vec<u8>) -> Vec<u8> {
    tlv(0xA0 | n, &inner)
}

fn app(n: u8, parts: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x60 | n, &parts.concat())
}

fn set(parts: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x31, &parts.concat())
}

fn seq(parts: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &parts.concat())
}

/// Root { RootElementCollection { [0] element... } }
fn root_elements(elements: &[Vec<u8>]) -> Vec<u8> {
    let items: Vec<Vec<u8>> = elements.iter().map(|e| ctx(0, e.clone())).collect();
    app(0, &[app(11, &items)])
}

fn children(elements: &[Vec<u8>]) -> Vec<u8> {
    let items: Vec<Vec<u8>> = elements.iter().map(|e| ctx(0, e.clone())).collect();
    ctx(2, app(4, &items))
}

// --- S101 --------------------------------------------------------------------

fn crc(data: &[u8]) -> u16 {
    let mut c: u16 = 0xFFFF;
    for &b in data {
        c ^= u16::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x8408
            } else {
                c >> 1
            };
        }
    }
    c
}

fn frame(ber: &[u8]) -> Vec<u8> {
    let mut data = vec![0x00, 0x0E, 0x00, 0x01, 0xC0, 0x01, 0x02, 0x32, 0x02];
    data.extend_from_slice(ber);
    let c = !crc(&data);
    data.extend([c as u8, (c >> 8) as u8]);
    let mut out = vec![0xFE];
    for b in data {
        if b >= 0xF8 {
            out.extend([0xFD, b ^ 0x20]);
        } else {
            out.push(b);
        }
    }
    out.push(0xFF);
    out
}

/// The data of each complete frame in `buf`, removed from it.
fn deframe(buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let (Some(start), Some(end)) = (
        buf.iter().position(|&b| b == 0xFE),
        buf.iter().position(|&b| b == 0xFF),
    ) {
        if end < start {
            buf.drain(..=end);
            continue;
        }
        let mut data = Vec::new();
        let mut escape = false;
        for &b in &buf[start + 1..end] {
            if escape {
                data.push(b ^ 0x20);
                escape = false;
            } else if b == 0xFD {
                escape = true;
            } else {
                data.push(b);
            }
        }
        buf.drain(..=end);
        assert_eq!(crc(&data), 0xF0B8, "the core's CRC is good");
        data.truncate(data.len() - 2);
        out.push(data);
    }
    out
}

// --- Reading what the consumer sends -------------------------------------------

#[derive(Debug, Clone)]
struct T {
    tag: u8,
    content: Vec<u8>,
    children: Vec<T>,
}

fn parse(buf: &[u8], at: &mut usize) -> T {
    let tag = buf[*at];
    *at += 1;
    let mut len = usize::from(buf[*at]);
    *at += 1;
    if len & 0x80 != 0 {
        let n = len & 0x7F;
        len = buf[*at..*at + n]
            .iter()
            .fold(0, |a, &b| (a << 8) | usize::from(b));
        *at += n;
    }
    let end = *at + len;
    let mut t = T {
        tag,
        content: Vec::new(),
        children: Vec::new(),
    };
    if tag & 0x20 != 0 {
        while *at < end {
            t.children.push(parse(buf, at));
        }
    } else {
        t.content = buf[*at..end].to_vec();
        *at = end;
    }
    t
}

impl T {
    fn field(&self, n: u8) -> Option<&T> {
        self.children
            .iter()
            .find(|c| c.tag == 0xA0 | n)
            .map(|c| &c.children[0])
    }
    fn items(&self) -> Vec<&T> {
        self.children.iter().map(|c| &c.children[0]).collect()
    }
    fn int(&self) -> i64 {
        let mut v: i64 = if self.content[0] & 0x80 != 0 { -1 } else { 0 };
        for &b in &self.content {
            v = (v << 8) | i64::from(b);
        }
        v
    }
    fn real(&self) -> f64 {
        let c = &self.content;
        if c.is_empty() {
            return 0.0;
        }
        let elen = usize::from(c[0] & 0x03) + 1;
        let e = T {
            tag: 2,
            content: c[1..1 + elen].to_vec(),
            children: vec![],
        }
        .int();
        let m = c[1 + elen..]
            .iter()
            .fold(0u64, |a, &b| (a << 8) | u64::from(b));
        let v = m as f64 * 2f64.powi(e as i32);
        if c[0] & 0x40 != 0 {
            -v
        } else {
            v
        }
    }
    fn commands(&self) -> Vec<&T> {
        self.field(2)
            .map(|c| c.items().into_iter().filter(|e| e.tag == 0x62).collect())
            .unwrap_or_default()
    }
}

// --- The provider --------------------------------------------------------------

struct Provider {
    gain: f64,
    connections: [Vec<u8>; 4],
}

fn param(number: i64, contents: &[Vec<u8>]) -> Vec<u8> {
    app(1, &[ctx(0, int(number)), ctx(1, set(contents))])
}

fn matrix_connections(p: &Provider, targets: &[usize], disposition: Option<i64>) -> Vec<u8> {
    let conns: Vec<Vec<u8>> = targets
        .iter()
        .map(|&t| {
            let mut f = vec![ctx(0, int(t as i64)), ctx(1, roid(&p.connections[t]))];
            if let Some(d) = disposition {
                f.push(ctx(3, int(d)));
            }
            ctx(0, app(16, &f))
        })
        .collect();
    root_elements(&[app(17, &[ctx(0, roid(&[1, 4])), ctx(5, seq(&conns))])])
}

/// Answers to one element of a request.
fn answer(p: &mut Provider, e: &T) -> Vec<Vec<u8>> {
    let path = e.field(0).map(|f| f.content.clone()).unwrap_or_default();
    let commands = e.commands();
    let command = commands.first().map(|c| c.field(0).unwrap().int());
    match (e.tag, path.as_slice(), command) {
        // GetDirectory at the root.
        (0x62, _, _) if e.field(0).unwrap().int() == 32 => vec![root_elements(&[app(
            3,
            &[ctx(0, int(1)), ctx(1, set(&[ctx(0, utf8("Device"))]))],
        )])],
        (0x6A, [1], Some(32)) => vec![root_elements(&[app(
            10,
            &[
                ctx(0, roid(&[1])),
                children(&[
                    param(
                        1,
                        &[
                            ctx(0, utf8("gain")),
                            ctx(2, real(p.gain)),
                            ctx(3, real(-128.0)),
                            ctx(4, real(15.0)),
                            ctx(5, int(3)),
                        ],
                    ),
                    param(
                        3,
                        &[
                            ctx(0, utf8("level")),
                            ctx(5, int(1)),
                            ctx(13, int(1)),
                            ctx(14, int(7)),
                        ],
                    ),
                    app(
                        13,
                        &[
                            ctx(0, int(4)),
                            ctx(
                                1,
                                set(&[ctx(0, utf8("router")), ctx(4, int(4)), ctx(5, int(4))]),
                            ),
                        ],
                    ),
                    app(
                        19,
                        &[
                            ctx(0, int(5)),
                            ctx(
                                1,
                                set(&[
                                    ctx(0, utf8("add")),
                                    ctx(
                                        2,
                                        seq(&[
                                            ctx(0, app(21, &[ctx(0, int(1)), ctx(1, utf8("a"))])),
                                            ctx(0, app(21, &[ctx(0, int(1)), ctx(1, utf8("b"))])),
                                        ]),
                                    ),
                                    ctx(3, seq(&[ctx(0, app(21, &[ctx(0, int(1))]))])),
                                ]),
                            ),
                        ],
                    ),
                ]),
            ],
        )])],
        (0x71, [1, 4], Some(32)) => vec![matrix_connections(p, &[0, 1, 2, 3], None)],
        (0x71, [1, 4], None) => {
            let mut changed = Vec::new();
            for c in e.field(5).unwrap().items() {
                let target = c.field(0).unwrap().int() as usize;
                let sources = c.field(1).unwrap().content.clone();
                let op = c.field(2).map_or(0, T::int);
                let current = &mut p.connections[target];
                match op {
                    1 => current.extend(
                        sources
                            .iter()
                            .filter(|s| !current.contains(s))
                            .collect::<Vec<_>>(),
                    ),
                    2 => current.retain(|s| !sources.contains(s)),
                    _ => *current = sources,
                }
                changed.push(target);
            }
            vec![matrix_connections(p, &changed, Some(1))]
        }
        (0x69, [1, 1], None) => {
            let v = e.field(1).unwrap().field(2).unwrap().real();
            // Within range it is taken; the provider answers with what it holds.
            if (-128.0..=15.0).contains(&v) {
                p.gain = v;
            }
            vec![root_elements(&[app(
                9,
                &[ctx(0, roid(&[1, 1])), ctx(1, set(&[ctx(2, real(p.gain))]))],
            )])]
        }
        // Subscribe to the level: stream it.
        (0x69, [1, 3], Some(30)) => vec![app(
            0,
            &[app(
                6,
                &[ctx(0, app(5, &[ctx(0, int(7)), ctx(1, int(-42))]))],
            )],
        )],
        (0x74, [1, 5], Some(33)) => {
            let invocation = commands[0].field(2).unwrap();
            let id = invocation.field(0).unwrap().int();
            let sum: i64 = invocation
                .field(1)
                .unwrap()
                .items()
                .iter()
                .map(|v| v.int())
                .sum();
            vec![app(
                0,
                &[app(
                    23,
                    &[
                        ctx(0, int(id)),
                        ctx(1, tlv(0x01, &[0xFF])),
                        ctx(2, seq(&[ctx(0, int(sum))])),
                    ],
                )],
            )]
        }
        _ => vec![],
    }
}

type Log = Arc<Mutex<Vec<(u8, Vec<u8>, Option<i64>)>>>;

async fn simulate() -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut p = Provider {
            gain: -10.0,
            connections: [vec![1], vec![], vec![], vec![]],
        };
        let mut pending = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            let mut reply = Vec::new();
            for data in deframe(&mut pending) {
                // Keep-alive request: answer it.
                if data[2] == 0x01 {
                    reply.extend(frame_raw(&[0x00, 0x0E, 0x02, 0x01]));
                    continue;
                }
                assert_eq!(
                    &data[..9],
                    &[0x00, 0x0E, 0x00, 0x01, 0xC0, 0x01, 0x02, 50, 0x02]
                );
                let root = parse(&data[9..], &mut 0);
                assert_eq!(root.tag, 0x60);
                let collection = &root.children[0];
                assert_eq!(collection.tag, 0x6B);
                for e in collection.items() {
                    let path = e.field(0).map(|f| f.content.clone()).unwrap_or_default();
                    let command = e.commands().first().map(|c| c.field(0).unwrap().int());
                    record.lock().unwrap().push((e.tag, path, command));
                    for out in answer(&mut p, e) {
                        reply.extend(frame(&out));
                    }
                }
            }
            if !reply.is_empty() {
                stream.write_all(&reply).await.unwrap();
            }
        }
    });
    (port, log)
}

fn frame_raw(data: &[u8]) -> Vec<u8> {
    let mut d = data.to_vec();
    let c = !crc(data);
    d.extend([c as u8, (c >> 8) as u8]);
    let mut out = vec![0xFE];
    for b in d {
        if b >= 0xF8 {
            out.extend([0xFD, b ^ 0x20]);
        } else {
            out.push(b);
        }
    }
    out.push(0xFF);
    out
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("state within 10 s: {}", core.snapshot(id).unwrap().state))
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn ember_plus_end_to_end() {
    let (port, log) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "emberplus".into(),
            model: "provider".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
        })
        .unwrap();

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let events = core.next_events(64).await;
            if events.iter().any(|e| {
                matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
            }) {
                return;
            }
        }
    })
    .await
    .expect("connected");

    // The walk: root, node 1, then the router's connections.
    wait_for_state(&core, id, |s| s["walk"]["complete"] == true).await;
    let st = core.snapshot(id).unwrap().state;
    assert_eq!(st["identifiers"]["Device/gain"], "1.1");
    assert_eq!(st["elements"]["1.1"]["value"], -10.0);
    assert_eq!(st["elements"]["1.1"]["access"], "read_write");
    assert_eq!(st["elements"]["1.3"]["stream_identifier"], 7);
    assert_eq!(
        st["elements"]["1.4"]["connections"]["0"]["sources"],
        json!([1])
    );
    assert_eq!(
        st["elements"]["1.5"]["result"],
        json!([{"type": "integer", "name": null}])
    );

    // A value change, answered with the value the provider now holds.
    assert_eq!(
        core.execute(
            id,
            "set_parameter",
            params(json!({"path": "Device/gain", "value": -6.5}))
        )
        .await,
        Ok(Outcome::Value { value: json!(-6.5) })
    );
    wait_for_state(&core, id, |s| s["elements"]["1.1"]["value"] == -6.5).await;

    // A matrix connect.
    assert_eq!(
        core.execute(
            id,
            "matrix_connect",
            params(json!({"path": "Device/router", "target": 2, "sources": [3]}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"target": 2, "sources": [3], "disposition": "modified"})
        })
    );
    wait_for_state(&core, id, |s| {
        s["elements"]["1.4"]["connections"]["2"]["sources"] == json!([3])
    })
    .await;

    // A stream, after subscribing.
    assert_eq!(
        core.execute(id, "subscribe", params(json!({"path": "Device/level"})))
            .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| s["elements"]["1.3"]["value"] == -42).await;

    // A function.
    assert_eq!(
        core.execute(
            id,
            "invoke_function",
            params(json!({"path": "1.5", "arguments": [2, 40]}))
        )
        .await,
        Ok(Outcome::Value { value: json!([42]) })
    );

    let seen = log.lock().unwrap().clone();
    let summary: Vec<(u8, Vec<u8>, Option<i64>)> = seen.into_iter().take(7).collect();
    assert_eq!(
        summary,
        vec![
            (0x62, vec![32], None),
            (0x6A, vec![1], Some(32)),
            (0x71, vec![1, 4], Some(32)),
            (0x69, vec![1, 1], None),
            (0x71, vec![1, 4], None),
            (0x69, vec![1, 3], Some(30)),
            (0x74, vec![1, 5], Some(33)),
        ]
    );
    core.close(id).await;
}

/// Lazy walk: only the root on connecting; a value change by identifier path
/// asks for node 1's directory and nothing else, then goes out.
#[tokio::test(flavor = "multi_thread")]
async fn ember_plus_lazy_walk() {
    let (port, log) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "emberplus".into(),
            model: "provider".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"walk": "lazy"})),
        })
        .unwrap();

    wait_for_state(&core, id, |s| s["walk"]["complete"] == true).await;
    let st = core.snapshot(id).unwrap().state;
    assert_eq!(st["walk"]["mode"], "lazy");
    assert_eq!(st["identifiers"]["Device"], "1");
    assert_eq!(st["elements"]["1.1"], Value::Null);
    assert_eq!(log.lock().unwrap().len(), 1, "only the root is asked for");

    assert_eq!(
        core.execute(
            id,
            "set_parameter",
            params(json!({"path": "Device/gain", "value": -6.5}))
        )
        .await,
        Ok(Outcome::Value { value: json!(-6.5) })
    );
    let st = core.snapshot(id).unwrap().state;
    assert_eq!(st["identifiers"]["Device/router"], "1.4");
    // The router was found but not walked: no connections asked for.
    assert_eq!(st["elements"]["1.4"]["connections"], Value::Null);

    // A matrix's connections come from get_directory on it.
    assert_eq!(
        core.execute(
            id,
            "get_directory",
            params(json!({"path": "Device/router"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["elements"]["1.4"]["connections"]["0"]["sources"] == json!([1])
    })
    .await;

    let seen = log.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![
            (0x62, vec![32], None),
            (0x6A, vec![1], Some(32)),
            (0x69, vec![1, 1], None),
            (0x71, vec![1, 4], Some(32)),
        ]
    );
    core.close(id).await;
}
