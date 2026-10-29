//! Paged lists, saving an answer to a host file and per-command timeouts in
//! the engine (SPEC.md §4, "Paged lists", "Saving a response to a file",
//! "Timeouts"), on a fake clock: each `Cx` is a moment.

use std::net::Ipv4Addr;

use serde_json::json;

use super::*;
use crate::module::{Action, CommandResult};

const SPEC: &str = r##"
spec: 1
id: transfer-test
name: Transfer test
vendor: Test
category: other
source: [{ title: test }]
transport: { type: http, port: 80, timeout_ms: 2000, probe: { method: GET, path: /info } }
ports: [{ port: 80, protocol: http, role: control }]
models: [{ id: m, name: M, supports: [list, download, slow, upload, fetch_file, export], verification: none }]
commands:
  list:
    send:
      method: GET
      path: /items
      query: { per_page: "2" }
      pages: { items: "$.data", next: "$.links.next" }
    expect: { status: 200, json_path: "$.data" }
    returns: value
  download:
    params:
      id: { type: int, min: 1, required: true }
      to: { type: save, required: true }
    send: { method: GET, path: "/recordings/{id}/file", save: to }
    expect: { status: 200 }
    returns: file
  fetch_file:
    params:
      to: { type: save, required: true }
    send: { method: GET, path: /log, save: to }
    expect: { status: 200 }
    returns: ack
  export:
    params:
      to: { type: save, required: true }
    send:
      - { method: GET, path: /export/start }
      - { method: GET, path: /export/file.bin, save: to }
    expect: { status: 200 }
    returns: file
  slow:
    timeout_ms: 600000
    send: { method: POST, path: /firmware/apply }
    expect: { status: 200 }
    returns: ack
  upload:
    params:
      file: { type: binary, required: true }
    send: { method: POST, path: /upload, body: { binary: file }, content_type: application/octet-stream }
    expect: { status: 200 }
    returns: ack
state:
  items.*.name: { type: string, description: "x" }
telemetry:
  poll:
    every_ms: 10000
    send:
      - method: GET
        path: /items
        query: { per_page: "2" }
        pages: { items: "$.data", next: "$.links.next" }
  updates:
    - path: "^/items\\?per_page=2$"
      json_each: "$.data"
      json: { id: "$.id", name: "$.name" }
      replace: items
      state: { "items.{id}.name": "{name}" }
"##;

fn engine(monitor: bool) -> SpecEngine {
    let spec: DeviceSpec = crate::yaml::from_str(SPEC).unwrap();
    SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "m".into(),
            channels: None,
            settings: Params::new(),
            monitor,
        },
    )
    .unwrap()
}

fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest, Option<std::path::PathBuf>)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Http { id, request } => Some((*id, request.clone(), None)),
            Action::HttpSave { id, request, to } => Some((*id, request.clone(), Some(to.clone()))),
            _ => None,
        })
        .collect()
}

fn ok(body: &str) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status: 200,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.as_bytes().to_vec(),
    })
}

fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Complete { id, result } => Some((*id, result.clone())),
            _ => None,
        })
        .collect()
}

fn reply_timer(actions: &[Action]) -> Option<Millis> {
    actions.iter().find_map(|a| match a {
        Action::SetTimer { key: REPLY, after } => Some(*after),
        _ => None,
    })
}

/// Open the device for commands only and answer its probe.
fn connected() -> SpecEngine {
    let mut e = engine(false);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let (probe, _, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(1);
    e.http_response(&mut cx, probe, ok("{}"));
    e
}

#[test]
fn a_list_command_follows_every_page_and_returns_every_item() {
    let mut e = connected();
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "list", &Params::new());
    let (id, first, _) = requests(&cx.take()).remove(0);
    assert_eq!(first.url, "http://127.0.0.1:80/items?per_page=2");

    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        id,
        ok(r#"{"data":[{"id":"a"},{"id":"b"}],"links":{"next":"https://elsewhere.test/items?offset=2&per_page=2"}}"#),
    );
    let a = cx.take();
    assert!(completed(&a).is_empty(), "the next page first");
    let (id, second, _) = requests(&a).remove(0);
    // The next page goes to the same server, whatever host the link names.
    assert_eq!(second.url, "http://127.0.0.1:80/items?offset=2&per_page=2");
    assert_eq!(reply_timer(&a), Some(2000));

    let mut cx = Cx::new(4);
    e.http_response(&mut cx, id, ok(r#"{"data":[{"id":"c"}],"links":{}}"#));
    assert_eq!(
        completed(&cx.take()),
        vec![(
            1,
            Ok(Outcome::Value {
                value: json!([{"id": "a"}, {"id": "b"}, {"id": "c"}])
            })
        )]
    );
}

#[test]
fn a_paged_poll_replaces_its_list_once_with_every_page() {
    let mut e = engine(true);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let (poll, _, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(1);
    e.http_response(
        &mut cx,
        poll,
        ok(r#"{"data":[{"id":"a","name":"A"},{"id":"b","name":"B"}],"links":{"next":"/items?offset=2&per_page=2"}}"#),
    );
    let a = cx.take();
    assert!(
        !a.iter().any(|x| matches!(x, Action::State(_))),
        "nothing is written until the last page: {a:?}"
    );
    let (next, _, _) = requests(&a).remove(0);
    let mut cx = Cx::new(2);
    e.http_response(
        &mut cx,
        next,
        ok(r#"{"data":[{"id":"c","name":"C"}],"links":{}}"#),
    );
    let states: Vec<Value> = cx
        .take()
        .into_iter()
        .filter_map(|x| match x {
            // The poll group's `polled` stamp aside (select_tests).
            Action::State(s) if s.get("polled").is_none() => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(
        states.last().unwrap(),
        &json!({"items": {"a": {"name": "A"}, "b": {"name": "B"}, "c": {"name": "C"}}})
    );
}

#[test]
fn a_failed_page_fails_the_command_and_writes_nothing() {
    let mut e = connected();
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "list", &Params::new());
    let (id, _, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        id,
        ok(r#"{"data":[{"id":"a"}],"links":{"next":"/items?offset=1"}}"#),
    );
    let (id, _, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(4);
    e.http_response(
        &mut cx,
        id,
        Ok(HttpResponse {
            status: 500,
            headers: vec![],
            body: b"oops".to_vec(),
        }),
    );
    let a = cx.take();
    assert!(matches!(
        &completed(&a)[..],
        [(1, Err(CommandError::DeviceError { .. }))]
    ));
    assert!(!a.iter().any(|x| matches!(x, Action::State(_))));
}

#[test]
fn a_save_parameter_has_the_answer_saved_and_returns_the_file() {
    let dir = std::env::temp_dir();
    let to = dir.join(format!("meros-save-{}.bin", std::process::id()));
    let mut e = connected();
    let mut cx = Cx::new(2);
    let params = json!({"id": 7, "to": {"file": to.to_str().unwrap()}});
    e.command(&mut cx, 1, "download", params.as_object().unwrap());
    let a = cx.take();
    let (id, request, saved_to) = requests(&a).remove(0);
    assert_eq!(request.url, "http://127.0.0.1:80/recordings/7/file");
    assert_eq!(saved_to.as_deref(), Some(to.as_path()));
    // The HTTP client times a saved answer by its progress: no engine timer.
    assert_eq!(reply_timer(&a), None);
    assert!(a.contains(&Action::CancelTimer { key: REPLY }));

    // The client wrote the file; the answer carries no body.
    std::fs::write(&to, b"0123456789").unwrap();
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        id,
        Ok(HttpResponse {
            status: 200,
            headers: vec![("content-type".into(), "video/mp4".into())],
            body: Vec::new(),
        }),
    );
    let done = completed(&cx.take());
    let _ = std::fs::remove_file(&to);
    assert_eq!(
        done,
        vec![(
            1,
            Ok(Outcome::Value {
                value: json!({"file": to.to_str().unwrap(), "size": 10, "content_type": "video/mp4"})
            })
        )]
    );

    // A refusal is the command's error, and nothing was saved.
    let mut cx = Cx::new(4);
    e.command(&mut cx, 2, "download", params.as_object().unwrap());
    let (id, _, _) = requests(&cx.take()).remove(0);
    let mut cx = Cx::new(5);
    e.http_response(
        &mut cx,
        id,
        Ok(HttpResponse {
            status: 404,
            headers: vec![],
            body: b"no such recording".to_vec(),
        }),
    );
    assert!(matches!(
        &completed(&cx.take())[..],
        [(2, Err(CommandError::DeviceError { .. }))]
    ));
}

#[test]
fn a_save_parameter_must_name_a_file_in_a_folder_that_exists() {
    let mut e = connected();
    let missing = std::env::temp_dir()
        .join("meros-no-such-folder-x")
        .join("a.bin");
    for to in [
        json!({"file": missing.to_str().unwrap()}),
        json!({"file": std::env::temp_dir().to_str().unwrap()}),
        json!("a.bin"),
        json!({"file": ""}),
    ] {
        let mut cx = Cx::new(2);
        e.command(
            &mut cx,
            1,
            "download",
            json!({"id": 1, "to": to}).as_object().unwrap(),
        );
        let a = cx.take();
        assert!(requests(&a).is_empty(), "{to}");
    }
}

#[test]
fn a_command_timeout_replaces_the_transports() {
    let mut e = connected();
    let mut cx = Cx::new(2);
    e.command(&mut cx, 1, "slow", &Params::new());
    let a = cx.take();
    let (_, request, _) = requests(&a).remove(0);
    assert_eq!(request.timeout, Some(600_000));
    assert_eq!(reply_timer(&a), Some(600_000));

    // Others keep the transport's.
    let mut cx = Cx::new(3);
    e.timer(&mut cx, REPLY);
    let mut cx = Cx::new(4);
    e.command(&mut cx, 2, "list", &Params::new());
    assert_eq!(reply_timer(&cx.take()), Some(2000));
}

#[test]
fn a_streamed_upload_is_timed_by_its_progress() {
    let path = std::env::temp_dir().join(format!("meros-upload-{}.bin", std::process::id()));
    std::fs::write(&path, b"abc").unwrap();
    let mut e = connected();
    let mut cx = Cx::new(2);
    e.command(
        &mut cx,
        1,
        "upload",
        json!({"file": {"file": path.to_str().unwrap()}})
            .as_object()
            .unwrap(),
    );
    let a = cx.take();
    let _ = std::fs::remove_file(&path);
    let (id, request, _) = requests(&a).remove(0);
    assert!(request.body_stream.is_some());
    assert_eq!(request.timeout, Some(2000));
    assert_eq!(reply_timer(&a), None);

    // The client's report that nothing moved is the command's timeout.
    let mut cx = Cx::new(3);
    e.http_response(
        &mut cx,
        id,
        Err(format!("{} 2000 ms", crate::http::STALLED)),
    );
    assert_eq!(completed(&cx.take()), vec![(1, Err(CommandError::Timeout))]);
}

#[test]
fn a_file_fetched_after_an_earlier_message_is_the_commands_result() {
    let to = std::env::temp_dir().join(format!("meros-export-{}.bin", std::process::id()));
    let mut e = connected();
    let mut cx = Cx::new(2);
    let params = json!({"to": {"file": to.to_str().unwrap()}});
    e.command(&mut cx, 1, "export", params.as_object().unwrap());
    let (id, first, saved) = requests(&cx.take()).remove(0);
    assert_eq!(
        (first.url.as_str(), saved),
        ("http://127.0.0.1:80/export/start", None)
    );
    // The export's acknowledgement lets the file's request go.
    let mut cx = Cx::new(3);
    e.http_response(&mut cx, id, ok("<result>0</result>"));
    let a = cx.take();
    assert!(completed(&a).is_empty());
    let (id, second, saved) = requests(&a).remove(0);
    assert_eq!(second.url, "http://127.0.0.1:80/export/file.bin");
    assert_eq!(saved.as_deref(), Some(to.as_path()));
    std::fs::write(&to, b"abc").unwrap();
    let mut cx = Cx::new(4);
    e.http_response(
        &mut cx,
        id,
        Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: Vec::new(),
        }),
    );
    let done = completed(&cx.take());
    let _ = std::fs::remove_file(&to);
    assert_eq!(
        done,
        vec![(
            1,
            Ok(Outcome::Value {
                value: json!({"file": to.to_str().unwrap(), "size": 3, "content_type": null})
            })
        )]
    );
}
