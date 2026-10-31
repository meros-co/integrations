//! `transport.pacing` in the engine (SPEC.md §2, Pacing), on a fake clock:
//! each `Cx` is a moment, and the pacing timer is fired by hand.

use std::net::Ipv4Addr;

use crate::json;

use super::*;
use crate::module::Action;

const SPEC: &str = r##"
spec: 1
id: pace-test
name: Pace test
vendor: Test
category: camera
source: [{ title: test }]
transport:
  type: line-tcp
  port: 9990
  framing: terminated
  terminator: cr
  pacing:
    min_interval_ms: 100
    groups:
      pan_tilt: { match: "^#P", min_interval_ms: 300, by_model: [{ models: [fast], min_interval_ms: 150 }] }
      power: { match: "^POWR", after_ms: 5000 }
ports:
  - { port: 9990, protocol: tcp, role: control }
models:
  - { id: m, name: M, supports: [pan, ask, power, both], verification: none }
  - { id: fast, name: Fast, supports: [pan, ask, power, both], verification: none }
commands:
  pan: { params: { speed: { type: int, min: 1, max: 99, label: Speed, description: Speed. } }, send: "#P{speed}" }
  ask: { send: "QID" }
  power: { send: "POWR1" }
  both: { send: ["#P50", "#T50"], returns: none }
telemetry:
  poll: { every_ms: 10000, send: ["QPW", "QSV"] }
  updates: []
"##;

fn open(model: &str, settings: Value, monitor: bool) -> SpecEngine {
    let spec: DeviceSpec = crate::yaml::from_str(SPEC).unwrap();
    let mut e = SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: model.into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        },
    )
    .unwrap();
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    e.tcp(&mut cx, SOCKET, TcpInput::Connected);
    e
}

fn sent(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::TcpSend { data, .. } | Action::UdpSend { data, .. } => {
                Some(String::from_utf8_lossy(data).trim_end().to_string())
            }
            _ => None,
        })
        .collect()
}

fn pace_timer(actions: &[Action]) -> Option<Millis> {
    actions.iter().rev().find_map(|a| match a {
        Action::SetTimer { key: PACE, after } => Some(*after),
        _ => None,
    })
}

fn speed(n: u64) -> Params {
    json!({ "speed": n }).as_object().unwrap().clone()
}

/// Run `f` at `now`, returning what it did.
fn at(now: Millis, f: impl FnOnce(&mut Cx)) -> Vec<Action> {
    let mut cx = Cx::new(now);
    f(&mut cx);
    cx.take()
}

fn reply(e: &mut SpecEngine, now: Millis) -> Vec<Action> {
    at(now, |cx| {
        e.tcp(cx, SOCKET, TcpInput::Data(b"OK\r".to_vec()))
    })
}

#[test]
fn a_message_too_soon_waits_its_turn_and_nothing_overtakes_it() {
    let mut e = open("m", json!({}), false);
    let a = at(0, |cx| e.command(cx, 1, "pan", &speed(1)));
    assert_eq!(sent(&a), ["#P1"]);
    reply(&mut e, 10);
    // 100 ms after the last message, whatever its kind.
    let a = at(20, |cx| e.command(cx, 2, "ask", &Params::new()));
    assert!(sent(&a).is_empty());
    assert_eq!(pace_timer(&a), Some(80));
    // A pan-tilt command behind it stays behind it.
    let a = at(30, |cx| e.command(cx, 3, "pan", &speed(2)));
    assert!(sent(&a).is_empty());
    // Too early: the timer is armed again.
    let a = at(60, |cx| e.timer(cx, PACE));
    assert!(sent(&a).is_empty());
    assert_eq!(pace_timer(&a), Some(40));
    let a = at(100, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["QID"]);
    // The pan-tilt group's 300 ms run from the first pan-tilt message.
    let a = reply(&mut e, 120);
    assert!(sent(&a).is_empty());
    assert_eq!(pace_timer(&a), Some(180));
    let a = at(300, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["#P2"]);
    assert_eq!(completed_ids(&reply(&mut e, 310)), [3]);
}

fn completed_ids(actions: &[Action]) -> Vec<CommandId> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Complete { id, .. } => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_model_and_a_group_keep_their_own_gaps() {
    let mut e = open("fast", json!({}), false);
    at(0, |cx| e.command(cx, 1, "pan", &speed(1)));
    reply(&mut e, 5);
    let a = at(10, |cx| e.command(cx, 2, "pan", &speed(2)));
    assert_eq!(pace_timer(&a), Some(140));
    // After power, everything waits 5 s.
    let mut e = open("m", json!({}), false);
    at(0, |cx| e.command(cx, 1, "power", &Params::new()));
    reply(&mut e, 5);
    let a = at(10, |cx| e.command(cx, 2, "ask", &Params::new()));
    assert_eq!(pace_timer(&a), Some(4990));
}

#[test]
fn each_message_of_a_command_is_paced() {
    let mut e = open("m", json!({}), false);
    let a = at(0, |cx| e.command(cx, 1, "both", &Params::new()));
    assert_eq!(sent(&a), ["#P50"]);
    assert!(completed_ids(&a).is_empty());
    assert_eq!(pace_timer(&a), Some(100));
    let a = at(100, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["#T50"]);
    assert_eq!(completed_ids(&a), [1]);
}

#[test]
fn polls_are_paced_like_commands() {
    let mut e = open("m", json!({}), true);
    // The first poll item went on connecting; the second waits its turn.
    let a = reply(&mut e, 5);
    assert!(sent(&a).is_empty());
    assert_eq!(pace_timer(&a), Some(95));
    let a = at(100, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["QSV"]);
}

#[test]
fn a_lost_connection_drops_what_waited() {
    let mut e = open("m", json!({}), false);
    at(0, |cx| e.command(cx, 1, "pan", &speed(1)));
    reply(&mut e, 5);
    at(10, |cx| e.command(cx, 2, "ask", &Params::new()));
    let a = at(20, |cx| {
        e.tcp(
            cx,
            SOCKET,
            TcpInput::Closed {
                reason: "gone".into(),
            },
        )
    });
    assert_eq!(completed_ids(&a), [2]);
    assert!(a.contains(&Action::CancelTimer { key: PACE }));
    assert!(sent(&at(100, |cx| e.timer(cx, PACE))).is_empty());
}

#[test]
fn messages_awaiting_no_reply_keep_their_order() {
    let mut spec: DeviceSpec = crate::yaml::from_str(SPEC).unwrap();
    spec.transport = Some(json!({
        "type": "line-udp", "port": 9990, "terminator": "cr", "reply": "none",
        "pacing": {"min_interval_ms": 100},
    }));
    spec.on_connect = vec![json!("HELLO"), json!("SUBSCRIBE")];
    let mut e = SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "m".into(),
            channels: None,
            settings: Params::new(),
            monitor: false,
        },
    )
    .unwrap();
    let a = at(0, |cx| e.start(cx));
    assert_eq!(sent(&a), ["HELLO"]);
    let a = at(10, |cx| e.command(cx, 1, "ask", &Params::new()));
    assert!(sent(&a).is_empty());
    let a = at(100, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["SUBSCRIBE"]);
    assert_eq!(pace_timer(&a), Some(100));
    let a = at(200, |cx| e.timer(cx, PACE));
    assert_eq!(sent(&a), ["QID"]);
    assert_eq!(completed_ids(&a), [1]);
}

#[test]
fn a_malformed_pacing_block_is_refused_on_opening() {
    let mut spec: DeviceSpec = crate::yaml::from_str(SPEC).unwrap();
    let t = spec.transport.as_mut().unwrap();
    t["pacing"] = json!({"min_interval_ms": "soon"});
    let e = SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "m".into(),
            channels: None,
            settings: Params::new(),
            monitor: false,
        },
    );
    assert!(e.err().unwrap().contains("min_interval_ms"));
}
