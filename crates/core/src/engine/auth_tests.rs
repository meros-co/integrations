//! Sessions, cookies and credentials computed per request in the engine
//! (SPEC.md §2: Sessions, Cookies, Computed request credentials, AWS
//! Signature Version 4, WS-Security UsernameToken), with the real specs that
//! use them and loopback addresses only: nothing is ever sent.

use std::net::Ipv4Addr;

use serde_json::json;

use super::*;
use crate::catalog::Catalog;
use crate::module::Action;

fn open(id: &str, model: &str, settings: Value, monitor: bool) -> SpecEngine {
    let spec = Catalog::source_tree().device(id).unwrap().clone();
    SpecEngine::new(
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
    .unwrap()
}

fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Http { id, request } => Some((*id, request.clone())),
            _ => None,
        })
        .collect()
}

fn header<'a>(r: &'a HttpRequest, name: &str) -> Option<&'a str> {
    r.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn reply(body: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status: 200,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: body.as_bytes().to_vec(),
    })
}

fn completed(actions: &[Action]) -> Vec<(CommandId, crate::module::CommandResult)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Complete { id, result } => Some((*id, result.clone())),
            _ => None,
        })
        .collect()
}

fn pushed(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::WsSend { socket: PUSH, text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Start the device for commands only and answer its probe.
fn started(e: &mut SpecEngine) {
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    for (id, _) in requests(&cx.take()) {
        let mut cx = Cx::new(1);
        e.http_response(&mut cx, id, reply(r#"{"result":"ok"}"#, &[]));
    }
}

#[test]
fn a_json_token_goes_inside_a_header_of_the_spec_and_is_renewed_on_its_code() {
    // Kiloview MG300: {"token": ...} in the app header, a new login on
    // {"result":"error","msg":"2"}.
    let mut e = open(
        "kiloview-mg300",
        "mg300-v2",
        json!({"username": "admin", "password": "admin"}),
        false,
    );
    started(&mut e);
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "list_users", &Params::new());
    let (login, request) = requests(&cx.take()).remove(0);
    assert!(request.url.ends_with("/api/users/login"), "{}", request.url);
    assert_eq!(header(&request, "app"), None);
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        login,
        reply(
            r#"{"result":"ok","data":{"token":"T1","username":"admin"}}"#,
            &[],
        ),
    );
    let (first, request) = requests(&cx.take()).remove(0);
    assert!(request.url.ends_with("/api/users/list"), "{}", request.url);
    assert_eq!(header(&request, "app"), Some(r#"{"token":"T1"}"#));
    // No header carries the session as a whole value.
    assert!(request.headers.iter().all(|(n, _)| !n.is_empty()));

    // Code 2, invalid token: a fresh login, and the request once more.
    let mut cx = Cx::new(4);
    e.http_response(
        &mut cx,
        first,
        reply(r#"{"result":"error","msg":"2"}"#, &[]),
    );
    let (relogin, request) = requests(&cx.take()).remove(0);
    assert!(request.url.ends_with("/api/users/login"));
    let mut cx = Cx::new(5);
    e.http_response(
        &mut cx,
        relogin,
        reply(r#"{"result":"ok","data":{"token":"T2"}}"#, &[]),
    );
    let (again, request) = requests(&cx.take()).remove(0);
    assert_eq!(header(&request, "app"), Some(r#"{"token":"T2"}"#));
    let mut cx = Cx::new(6);
    e.http_response(&mut cx, again, reply(r#"{"result":"ok","data":[]}"#, &[]));
    assert!(matches!(&completed(&cx.take())[..], [(1, Ok(_))]));

    // A wrong password (302) is the terminal refusal.
    let mut e = open(
        "kiloview-mg300",
        "mg300-v2",
        json!({"username": "admin", "password": "wrong"}),
        false,
    );
    started(&mut e);
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "list_users", &Params::new());
    let (login, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        login,
        reply(r#"{"result":"error","msg":"302"}"#, &[]),
    );
    let actions = cx.take();
    assert!(matches!(
        &completed(&actions)[..],
        [(1, Err(CommandError::Auth { .. }))]
    ));
}

#[test]
fn a_login_answer_can_hold_more_values_for_other_headers() {
    // Kiloview NDI encoders (Version 1.0): the session ID and the token, in
    // API-Session and API-Token.
    let mut e = open(
        "kiloview-ndi",
        "n4",
        json!({"username": "api", "password": "pw"}),
        false,
    );
    started(&mut e);
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "get_mode", &Params::new());
    let (login, request) = requests(&cx.take()).remove(0);
    assert!(request.url.ends_with("/api/v1/user/authorize"));
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        login,
        reply(r#"{"result":"ok","data":{"session":"S","token":"T"}}"#, &[]),
    );
    let (_, request) = requests(&cx.take()).remove(0);
    assert!(request.url.ends_with("/api/v1/mode/get"), "{}", request.url);
    assert_eq!(header(&request, "API-Session"), Some("S"));
    assert_eq!(header(&request, "API-Token"), Some("T"));
    assert_eq!(header(&request, "Authorization"), None);
}

#[test]
fn a_renewed_token_replaces_the_session_and_push_messages_carry_it() {
    // Pharos: a token in each answer, and in each WebSocket message.
    let mut e = open(
        "pharos-designer",
        "lpc",
        json!({"username": "op", "password": "pw"}),
        true,
    );
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let actions = cx.take();
    // The polls go first, and need the session: the login goes before them.
    let (login, request) = requests(&actions).remove(0);
    assert!(request.url.ends_with("/authenticate"), "{}", request.url);
    assert_eq!(
        request.body.as_deref(),
        Some(&br#"{"username":"op","password":"pw"}"#[..])
    );
    // The WebSocket opens: its messages wait for the session, and no second
    // login goes.
    let mut cx = Cx::new(1);
    e.ws(&mut cx, PUSH, WsInput::Opened);
    let actions = cx.take();
    assert!(pushed(&actions).is_empty());
    assert!(requests(&actions).is_empty());
    let mut cx = Cx::new(3);
    e.http_response(&mut cx, login, reply(r#"{"token":"A"}"#, &[]));
    let actions = cx.take();
    let messages = pushed(&actions);
    assert!(
        messages.contains(&r#"{"subscribe":"scene","token":"A"}"#.to_string()),
        "{messages:?}"
    );
    assert!(messages.contains(&r#"{"token":"A"}"#.to_string()));
    assert!(!messages.contains(&r#"{"subscribe":"scene"}"#.to_string()));
    let (poll, request) = requests(&actions).remove(0);
    assert_eq!(header(&request, "Authorization"), Some("Bearer A"));
    // The poll's answer holds a new token: it replaces the old one.
    let mut cx = Cx::new(4);
    e.http_response(&mut cx, poll, reply(r#"{"scenes":[],"token":"B"}"#, &[]));
    let next = requests(&cx.take());
    assert_eq!(header(&next[0].1, "Authorization"), Some("Bearer B"));
    let mut cx = Cx::new(5);
    e.timer(&mut cx, PUSH_RENEW);
    assert!(pushed(&cx.take()).contains(&r#"{"token":"B"}"#.to_string()));

    // Without security: the plain messages, no login.
    let mut e = open("pharos-designer", "lpc", json!({}), true);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    cx.take();
    let mut cx = Cx::new(1);
    e.ws(&mut cx, PUSH, WsInput::Opened);
    let actions = cx.take();
    let messages = pushed(&actions);
    assert!(
        messages.contains(&r#"{"subscribe":"scene"}"#.to_string()),
        "{messages:?}"
    );
    assert!(messages.contains(&"{}".to_string()));
    assert!(messages.iter().all(|m| !m.contains("token")));
    assert!(requests(&actions).is_empty());
}

#[test]
fn cookies_a_device_sets_go_back_on_later_requests() {
    // An Axis camera's PTZ control queue.
    let mut e = open(
        "axis-ptz",
        "axis-ptz",
        json!({"username": "root", "password": "pw", "camera": 1}),
        false,
    );
    started(&mut e);
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "queue_request", &Params::new());
    let actions = cx.take();
    assert!(!requests(&actions).is_empty(), "{actions:?}");
    let (queue, request) = requests(&actions).remove(0);
    assert_eq!(header(&request, "Cookie"), None);
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        queue,
        reply(
            r#"<a name="1"></a><a name="60"></a><a name="5"></a>"#,
            &[("set-cookie", "ptz_ctl_id=7f3a; path=/")],
        ),
    );
    cx.take();
    let mut cx = Cx::new(4);
    e.command(&mut cx, 2, "queue_query", &Params::new());
    let (_, request) = requests(&cx.take()).remove(0);
    assert_eq!(header(&request, "Cookie"), Some("ptz_ctl_id=7f3a"));
}

#[test]
fn computed_credentials_use_the_wall_clock_as_each_request_goes() {
    // AWS Elemental Live with user authentication on.
    let mut e = open(
        "aws-elemental-live",
        "live",
        json!({"username": "admin", "api_key": "k3y"}),
        false,
    );
    let mut cx = Cx::at(0, 1_700_000_000_000);
    e.start(&mut cx);
    let (_, request) = requests(&cx.take()).remove(0);
    let md5 = |s: &str| {
        use md5::Digest;
        md5::Md5::digest(s.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let expires = "1700000030";
    let key = md5(&format!(
        "k3y{}",
        md5(&format!("/system_status.jsonadmink3y{expires}"))
    ));
    assert_eq!(header(&request, "X-Auth-Expires"), Some(expires));
    assert_eq!(header(&request, "X-Auth-Key"), Some(key.as_str()));
    // Without the key, no header.
    let mut e = open("aws-elemental-live", "live", json!({}), false);
    let mut cx = Cx::at(0, 1_700_000_000_000);
    e.start(&mut cx);
    let (_, request) = requests(&cx.take()).remove(0);
    assert_eq!(header(&request, "X-Auth-Key"), None);
}

#[test]
fn a_push_websocket_can_open_with_a_token_fetched_just_before() {
    // CamStreamer: a token from ws_authorization.cgi, sent as the first
    // message, for each opening.
    let mut e = open(
        "camstreamer",
        "app-6",
        json!({"username": "root", "password": "pw", "scheme": "http", "auth": "digest"}),
        true,
    );
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let actions = cx.take();
    assert!(!actions
        .iter()
        .any(|a| matches!(a, Action::WsOpen { socket: PUSH, .. })));
    let (token, request) = requests(&actions)
        .into_iter()
        .find(|(_, r)| r.url.contains("ws_authorization"))
        .expect("the token request");
    assert!(request
        .url
        .ends_with("/local/camstreamer/api/ws_authorization.cgi"));
    assert!(request.digest.is_some(), "with the camera's credential");
    let mut cx = Cx::new(1);
    e.http_response(
        &mut cx,
        token,
        reply(
            r#"{"status":200,"message":"ok","data":"9lZ2rs9W8xeRZkTQTfsq"}"#,
            &[],
        ),
    );
    let actions = cx.take();
    let opened = actions.iter().find_map(|a| match a {
        Action::WsOpen {
            socket: PUSH,
            request,
        } => Some(request.clone()),
        _ => None,
    });
    let opened = opened.expect("the websocket opens");
    assert_eq!(opened.url, "ws://127.0.0.1:80/local/camstreamer/events");
    // The token's answer is never state.
    assert!(!actions.iter().any(|a| matches!(a, Action::State(_))));
    let mut cx = Cx::new(2);
    e.ws(&mut cx, PUSH, WsInput::Opened);
    assert_eq!(
        pushed(&cx.take()),
        vec![r#"{"authorization":"9lZ2rs9W8xeRZkTQTfsq"}"#.to_string()]
    );
    // Closed: a fresh token before it opens again.
    let mut cx = Cx::new(3);
    e.ws(
        &mut cx,
        PUSH,
        WsInput::Closed {
            code: None,
            reason: "closed".into(),
        },
    );
    cx.take();
    let mut cx = Cx::new(4);
    e.timer(&mut cx, PUSH_RECONNECT);
    let actions = cx.take();
    assert!(requests(&actions)
        .iter()
        .any(|(_, r)| r.url.contains("ws_authorization")));
    // A model without a websocket opens none.
    let mut e = open(
        "camstreamer",
        "camscripter",
        json!({"scheme": "http"}),
        true,
    );
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let actions = cx.take();
    assert!(!requests(&actions)
        .iter()
        .any(|(_, r)| r.url.contains("ws_authorization")));
}
