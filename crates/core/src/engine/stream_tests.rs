//! Replies tied to their requests on a websocket transport (SPEC.md §8,
//! Replies and what they answer), on a fake clock: each `Cx` is a moment.

use std::net::Ipv4Addr;

use serde_json::json;

use super::*;
use crate::module::Action;

pub(super) fn open_yaml(yaml: &str, settings: Value, monitor: bool) -> SpecEngine {
    let spec: DeviceSpec = crate::yaml::from_str(yaml).unwrap();
    let model = spec.models[0].id.clone();
    SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model,
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        },
    )
    .unwrap()
}

/// Run `f` at `now`, returning what it did.
pub(super) fn at(now: Millis, f: impl FnOnce(&mut Cx)) -> Vec<Action> {
    let mut cx = Cx::new(now);
    f(&mut cx);
    cx.take()
}

pub(super) fn ws_sent(actions: &[Action], socket: Key) -> Vec<String> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::WsSend { socket: s, text } if *s == socket => Some(text.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn states(actions: &[Action]) -> Vec<Value> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::State(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}

const WS_SPEC: &str = r##"
spec: 1
id: ws-test
name: WS test
vendor: Test
category: lighting
source: [{ title: test }]
transport:
  type: ws
  port: 9999
  path: /ws
  timeout_ms: 1000
  reply_match: "^API\\|"
ports: [{ port: 9999, protocol: ws, role: control }]
models: [{ id: m, name: M, supports: [status], verification: none }]
commands:
  status:
    params: { id: { type: int, min: 0, required: true, label: Id, description: Id. } }
    send: "API|getStatus|{id}"
    expect: { matches: "^API\\|getStatus\\|(.*)$" }
    returns: value
state:
  items.*.name: { type: string, description: x }
  items.*.status: { type: string, description: x }
  level: { type: int, description: x }
telemetry:
  poll: { every_ms: 10000, send: ["API|getList"] }
  updates:
    - match: "^API\\|getList\\|"
      match_each: '(\d+)\|([^|]*)(?:\||$)'
      state: { "items.{1}.name": "{2}" }
      then_send: ["API|getStatus|{1}"]
    - match: "^API\\|getStatus\\|(.*)$"
      request_match: "^API\\|getStatus\\|(\\d+)$"
      state: { "items.{2}.status": "{1}" }
    - match: "^LEVEL\\|(\\d+)$"
      state: { level: "{1}" }
"##;

#[test]
fn a_websocket_reply_names_the_request_it_answers() {
    let mut e = open_yaml(WS_SPEC, json!({}), true);
    at(0, |cx| e.start(cx));
    // Opened: the poll goes through the queue, awaiting its reply.
    let a = at(1, |cx| e.ws(cx, SOCKET, WsInput::Opened));
    assert_eq!(ws_sent(&a, SOCKET), ["API|getList"]);

    // A push meanwhile is no reply: it goes to the rules alone.
    let a = at(2, |cx| e.ws(cx, SOCKET, WsInput::Text("LEVEL|7".into())));
    assert_eq!(states(&a), [json!({"level": 7})]);
    assert!(ws_sent(&a, SOCKET).is_empty());

    // The list's reply queues a status query for each item, one at a time.
    let a = at(3, |cx| {
        e.ws(
            cx,
            SOCKET,
            WsInput::Text("API|getList|4|Front|9|Back|".into()),
        )
    });
    assert_eq!(
        states(&a),
        [json!({"items": {"4": {"name": "Front"}, "9": {"name": "Back"}}})]
    );
    assert_eq!(ws_sent(&a, SOCKET), ["API|getStatus|4"]);

    // Each reply is the status of the item its request named.
    let a = at(4, |cx| {
        e.ws(cx, SOCKET, WsInput::Text("API|getStatus|Running".into()))
    });
    assert_eq!(states(&a), [json!({"items": {"4": {"status": "Running"}}})]);
    assert_eq!(ws_sent(&a, SOCKET), ["API|getStatus|9"]);
    let a = at(5, |cx| {
        e.ws(cx, SOCKET, WsInput::Text("API|getStatus|Stopped".into()))
    });
    assert_eq!(states(&a), [json!({"items": {"9": {"status": "Stopped"}}})]);

    // A command's reply is tied to its request too, and still answers it.
    let params = json!({"id": 4}).as_object().unwrap().clone();
    let a = at(6, |cx| e.command(cx, 1, "status", &params));
    assert_eq!(ws_sent(&a, SOCKET), ["API|getStatus|4"]);
    let a = at(7, |cx| {
        e.ws(cx, SOCKET, WsInput::Text("API|getStatus|Stopped".into()))
    });
    assert_eq!(states(&a), [json!({"items": {"4": {"status": "Stopped"}}})]);
    assert!(a.contains(&Action::Complete {
        id: 1,
        result: Ok(Outcome::Value {
            value: json!("Stopped")
        })
    }));
}

#[test]
fn a_reply_with_nothing_asked_is_read_without_its_request() {
    let mut e = open_yaml(WS_SPEC, json!({}), false);
    at(0, |cx| e.start(cx));
    // Opened for commands only: nothing is polled.
    let a = at(1, |cx| e.ws(cx, SOCKET, WsInput::Opened));
    assert!(ws_sent(&a, SOCKET).is_empty());
    // A status with no request in flight names no item: nothing is written.
    let a = at(2, |cx| {
        e.ws(cx, SOCKET, WsInput::Text("API|getStatus|Running".into()))
    });
    assert!(states(&a).is_empty());
}

const ACK_SPEC: &str = r##"
spec: 1
id: ack-test
name: Ack test
vendor: Test
category: playback
source: [{ title: test }]
transport:
  type: line-tcp
  port: 9030
  framing: terminated
  terminator: cr
  timeout_ms: 1000
  reply_framing: terminated
  reply_terminators: ["\r", "\u0006", "\u0015"]
  reply_match: '^(\x06|\x15|@0)'
  error_match: '^\x15'
  acknowledge: { match: "^@0", send: "\u0006", framed: false, answers: '^@0\?' }
ports: [{ port: 9030, protocol: tcp, role: control }]
models: [{ id: m, name: M, supports: [play, status], verification: none }]
commands:
  play: { send: "@02353", expect: { reply_contains: "\u0006" }, returns: ack }
  status: { send: "@0?ST", expect: { reply_contains: "@0ST", matches: "^@0ST(.*)$" }, returns: value }
state:
  status: { type: string, description: x }
telemetry:
  updates:
    - match: '^@0ST(\w+)'
      state: { status: "{1}" }
"##;

fn tcp_sent(actions: &[Action]) -> Vec<Vec<u8>> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::TcpSend { data, .. } => Some(data.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_notification_is_acknowledged_even_for_commands_only() {
    let mut e = open_yaml(ACK_SPEC, json!({}), false);
    at(0, |cx| e.start(cx));
    at(1, |cx| e.tcp(cx, SOCKET, TcpInput::Connected));
    // A status the device sends by itself: state, and the ACK byte alone.
    let a = at(2, |cx| {
        e.tcp(cx, SOCKET, TcpInput::Data(b"@0STPL\r".to_vec()))
    });
    assert_eq!(tcp_sent(&a), [vec![6u8]]);
    assert_eq!(states(&a), [json!({"status": "PL"})]);

    // A command's ACK is not answered.
    let a = at(3, |cx| e.command(cx, 1, "play", &Params::new()));
    assert_eq!(tcp_sent(&a), [b"@02353\r".to_vec()]);
    let a = at(4, |cx| e.tcp(cx, SOCKET, TcpInput::Data(vec![6])));
    assert!(tcp_sent(&a).is_empty());

    // A status request: its ACK, then the status it asked for, which is no
    // notification; a notification meanwhile still is.
    let a = at(5, |cx| e.command(cx, 2, "status", &Params::new()));
    assert_eq!(tcp_sent(&a), [b"@0?ST\r".to_vec()]);
    let a = at(6, |cx| e.tcp(cx, SOCKET, TcpInput::Data(vec![6])));
    assert!(tcp_sent(&a).is_empty());
    let a = at(7, |cx| {
        e.tcp(cx, SOCKET, TcpInput::Data(b"@0STPP\r".to_vec()))
    });
    assert!(tcp_sent(&a).is_empty());
    assert!(a.contains(&Action::Complete {
        id: 2,
        result: Ok(Outcome::Value { value: json!("PP") })
    }));
    let a = at(8, |cx| {
        e.tcp(cx, SOCKET, TcpInput::Data(b"@0STPL\r".to_vec()))
    });
    assert_eq!(tcp_sent(&a), [vec![6u8]]);
}

const ECHO_SPEC: &str = r##"
spec: 1
id: echo-test
name: Echo test
vendor: Test
category: camera
source: [{ title: test }]
transport:
  type: ws
  port: 9998
  path: /
  acknowledge:
    - json_match: { "$.type": "^rcp_session$", "$.status": "^open$" }
      json: { data: "$.data" }
      send: '{"type":"rcp_session","data":{data}}'
ports: [{ port: 9998, protocol: ws, role: control }]
models: [{ id: m, name: M, supports: [], verification: none }]
commands: {}
"##;

#[test]
fn a_session_message_is_echoed_even_for_commands_only() {
    let mut e = open_yaml(ECHO_SPEC, json!({}), false);
    at(0, |cx| e.start(cx));
    at(1, |cx| e.ws(cx, SOCKET, WsInput::Opened));
    let a = at(2, |cx| {
        e.ws(
            cx,
            SOCKET,
            WsInput::Text(r#"{"type":"rcp_session","status":"open","data":"k7"}"#.into()),
        )
    });
    assert_eq!(
        ws_sent(&a, SOCKET),
        [r#"{"type":"rcp_session","data":"k7"}"#]
    );
    // Whatever the data's type, it goes back as it came.
    let a = at(3, |cx| {
        e.ws(
            cx,
            SOCKET,
            WsInput::Text(r#"{"type":"rcp_session","status":"open","data":{"n":[1,2]}}"#.into()),
        )
    });
    assert_eq!(
        ws_sent(&a, SOCKET),
        [r#"{"type":"rcp_session","data":{"n":[1,2]}}"#]
    );
    let a = at(3, |cx| {
        e.ws(
            cx,
            SOCKET,
            WsInput::Text(r#"{"type":"rcp_cur_int"}"#.into()),
        )
    });
    assert!(ws_sent(&a, SOCKET).is_empty());
}

const TOKEN_SPEC: &str = r##"
spec: 1
id: token-test
name: Token test
vendor: Test
category: camera
source: [{ title: test }]
transport:
  type: http
  port: 80
  auth: basic
  timeout_ms: 1000
settings:
  username: { type: string, default: root, label: User, description: x }
  password: { type: string, secret: true, default: pw, label: Password, description: x }
ports:
  - { port: 80, protocol: http, role: control }
  - { port: 80, protocol: ws, role: push }
models: [{ id: m, name: M, supports: [], verification: none }]
commands: {}
state:
  streams.*.on: { type: bool, description: x }
  leaked: { type: string, description: x }
telemetry:
  websocket:
    path: /events
    subprotocol: events
    token:
      request: { method: GET, path: /api/ws_authorization.cgi }
      json_path: "$.data"
    send: ['{"authorization":{token:json}}']
  updates:
    - json_match: { "$.type": "^StreamState$", "$.streamID": '^(\w+)$' }
      json: { on: "$.isStreaming" }
      state: { "streams.{1}.on": { value: "{on}", map: { "0": false, "1": true } } }
    - path: "^/api/ws_authorization"
      json: { t: "$.data" }
      state: { leaked: "{t}" }
"##;

fn http_requests(actions: &[Action]) -> Vec<(RequestId, String)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Http { id, request } => Some((*id, request.url.clone())),
            _ => None,
        })
        .collect()
}

fn push_opened(actions: &[Action]) -> Option<WsRequest> {
    actions.iter().find_map(|a| match a {
        Action::WsOpen {
            socket: PUSH,
            request,
        } => Some(request.clone()),
        _ => None,
    })
}

fn answer(status: u16, body: &str) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    })
}

#[test]
fn a_push_websocket_opens_with_a_token_asked_for_first() {
    let mut e = open_yaml(TOKEN_SPEC, json!({}), true);
    let a = at(0, |cx| e.start(cx));
    // The token is asked for first; the websocket waits for it.
    assert!(push_opened(&a).is_none());
    let asked = http_requests(&a);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, "http://127.0.0.1:80/api/ws_authorization.cgi");
    let a = at(10, |cx| {
        e.http_response(
            cx,
            asked[0].0,
            answer(200, r#"{"status":200,"data":"tk1"}"#),
        )
    });
    // Never offered to the rules: the token is a secret.
    assert!(states(&a).is_empty());
    let request = push_opened(&a).expect("the websocket opens");
    assert_eq!(request.url, "ws://127.0.0.1:80/events");
    // Opened: its first message carries the token.
    let a = at(20, |cx| e.ws(cx, PUSH, WsInput::Opened));
    assert_eq!(ws_sent(&a, PUSH), [r#"{"authorization":"tk1"}"#]);
    let a = at(30, |cx| {
        e.ws(
            cx,
            PUSH,
            WsInput::Text(r#"{"type":"StreamState","streamID":"117930","isStreaming":1}"#.into()),
        )
    });
    assert_eq!(states(&a), [json!({"streams": {"117930": {"on": true}}})]);

    // Closed: reopened after the backoff, with a token asked for afresh.
    let a = at(40, |cx| {
        e.ws(
            cx,
            PUSH,
            WsInput::Closed {
                reason: "gone".into(),
                code: None,
            },
        )
    });
    assert!(a.contains(&Action::SetTimer {
        key: PUSH_RECONNECT,
        after: RECONNECT_MIN
    }));
    let a = at(1040, |cx| e.timer(cx, PUSH_RECONNECT));
    let asked = http_requests(&a);
    assert_eq!(asked.len(), 1);
    // An answer without a token is asked again later, and opens nothing.
    let a = at(1050, |cx| {
        e.http_response(cx, asked[0].0, answer(200, "{}"))
    });
    assert!(push_opened(&a).is_none());
    assert!(a.iter().any(|x| matches!(
        x,
        Action::SetTimer {
            key: PUSH_RECONNECT,
            ..
        }
    )));
}

#[test]
fn a_refused_token_request_is_the_credential_refused() {
    let mut e = open_yaml(TOKEN_SPEC, json!({}), true);
    let a = at(0, |cx| e.start(cx));
    let asked = http_requests(&a);
    let a = at(10, |cx| e.http_response(cx, asked[0].0, answer(401, "")));
    assert!(push_opened(&a).is_none());
    assert!(a
        .iter()
        .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
}

#[test]
fn no_token_is_asked_for_commands_only() {
    let mut e = open_yaml(TOKEN_SPEC, json!({}), false);
    let a = at(0, |cx| e.start(cx));
    assert!(http_requests(&a)
        .iter()
        .all(|(_, url)| !url.contains("ws_authorization")));
    assert!(push_opened(&a).is_none());
}

const SESSION_PUSH_SPEC: &str = r##"
spec: 1
id: session-push-test
name: Session push test
vendor: Test
category: lighting
source: [{ title: test }]
transport:
  type: http
  port: 80
  timeout_ms: 1000
  session:
    when_set: username
    login: { method: POST, path: /authenticate, body: "{settings.username}" }
    capture: { cookie: token }
settings:
  username: { type: string, label: User, description: x }
ports:
  - { port: 80, protocol: http, role: control }
  - { port: 80, protocol: ws, role: push }
models: [{ id: m, name: M, supports: [], verification: none }]
commands: {}
state:
  level: { type: int, description: x }
telemetry:
  websocket:
    path: /query
    send:
      - { unless_set: username, message: '{"subscribe":"scene"}' }
      - { when_set: username, message: '{"subscribe":"scene","token":{session.id:json}}' }
    every_ms: 10000
  poll: { every_ms: 2000, send: [{ method: GET, path: /api/scene }] }
  updates: []
"##;

#[test]
fn push_messages_follow_a_setting_and_wait_for_the_session() {
    // No user: the message without a token.
    let mut e = open_yaml(SESSION_PUSH_SPEC, json!({}), true);
    at(0, |cx| e.start(cx));
    let a = at(1, |cx| e.ws(cx, PUSH, WsInput::Opened));
    assert_eq!(ws_sent(&a, PUSH), [r#"{"subscribe":"scene"}"#]);

    // A user: the message waits for the session, then carries its token.
    let mut e = open_yaml(SESSION_PUSH_SPEC, json!({"username": "op"}), true);
    let a = at(0, |cx| e.start(cx));
    let login = http_requests(&a);
    assert!(login[0].1.ends_with("/authenticate"));
    let a = at(1, |cx| e.ws(cx, PUSH, WsInput::Opened));
    assert!(ws_sent(&a, PUSH).is_empty());
    at(2, |cx| {
        e.http_response(
            cx,
            login[0].0,
            Ok(HttpResponse {
                status: 200,
                headers: vec![("set-cookie".into(), "token=abc; Path=/".into())],
                body: Vec::new(),
            }),
        )
    });
    let a = at(10_001, |cx| e.timer(cx, PUSH_RENEW));
    assert_eq!(
        ws_sent(&a, PUSH),
        [r#"{"subscribe":"scene","token":"abc"}"#]
    );
}
