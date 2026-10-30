//! Settings chosen after signing in (`select`, SPEC.md §2) and the time each
//! poll group was last answered (`polled.<group>`, SPEC.md §8), with a spec
//! of their own and loopback addresses only: nothing is ever sent.

use std::net::Ipv4Addr;

use crate::json;

use super::*;
use crate::module::Action;

const SPEC: &str = r#"
spec: 1
id: test-select
name: Test
vendor: Test
category: other
source: [{ title: Test }]
transport:
  type: http
  port: 443
  scheme: https
  probe:
    - { method: GET, path: "/things/{settings.thing}" }
    - { method: GET, path: /me }
ports: [{ port: 443, protocol: https, role: control }]
settings:
  thing: { type: string, label: Thing, description: "The thing followed.",
           select: { command: list_things, id: $.id, label: $.name } }
  part: { type: string, label: Part, description: "A part of the thing.",
          select: { command: list_parts, id: $.id, params: { thing: thing } } }
models:
  - { id: m, name: M, supports: [list_things, list_parts, start_thing], verification: none }
commands:
  list_things:
    send: { method: GET, path: /things }
    expect: { status: 200, json_path: "$" }
    returns: value
  list_parts:
    params: { thing: { type: string, required: true, label: Thing, description: "The thing." } }
    send: { method: GET, path: "/things/{thing}/parts" }
    expect: { status: 200, json_path: "$" }
    returns: value
  start_thing:
    send: { method: POST, path: "/things/{settings.thing}/start" }
    expect: { status: 200 }
    returns: ack
state:
  things.*.name: { type: string, description: "A thing's name" }
  thing.status: { type: string, description: "The followed thing's status" }
telemetry:
  poll:
    - { name: account, every_ms: 60000, send: [{ method: GET, path: /things }] }
    - { every_ms: 5000, send: [{ method: GET, path: "/things/{settings.thing}" }] }
  updates:
    - { path: "^/things$", json_each: "$", json: { id: "$.id", name: "$.name" },
        state: { "things.{id}.name": "{name}" } }
    - { path: "^/things/[^/]+$", json: { s: "$.status" }, state: { thing.status: "{s}" } }
"#;

fn spec() -> DeviceSpec {
    DeviceSpec::from_yaml(SPEC).unwrap()
}

fn open(settings: Value, monitor: bool) -> SpecEngine {
    let spec = spec();
    let settings = crate::catalog::validate(&spec.settings, settings.as_object().unwrap()).unwrap();
    SpecEngine::new(
        Arc::new(spec),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "m".into(),
            channels: None,
            settings,
            monitor,
        },
    )
    .unwrap()
}

fn requests(actions: &[Action]) -> Vec<(RequestId, String, String)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Http { id, request } => Some((
                *id,
                request.method.to_string(),
                request
                    .url
                    .splitn(4, '/')
                    .nth(3)
                    .map(|p| format!("/{p}"))
                    .unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

fn answer(status: u16, body: &str) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    })
}

fn states(actions: &[Action]) -> Vec<Value> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::State(patch) => Some(patch.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_catalogue_lists_what_each_command_needs_and_the_poll_stamps() {
    let spec = spec();
    assert_eq!(spec.commands["start_thing"].needs, ["thing"]);
    assert!(spec.commands["list_things"].needs.is_empty());
    let select = spec.settings["part"].select.as_ref().unwrap();
    assert_eq!(select.command, "list_parts");
    assert_eq!(select.params["thing"], "thing");
    assert_eq!(spec.state["polled.account"].kind, "int");
    assert_eq!(spec.state["polled.2"].unit.as_deref(), Some("s"));
    assert!(spec.state["polled.2"].description.contains("every 5000 ms"));
    // Served with the setting.
    let served = crate::value::to_value(&spec).unwrap();
    assert_eq!(served["settings"]["thing"]["select"]["id"], "$.id");
    assert_eq!(served["commands"]["start_thing"]["needs"], json!(["thing"]));
}

#[test]
fn a_selection_is_neither_required_nor_defaulted_and_its_lister_does_not_name_it() {
    let required = SPEC.replace(
        "thing: { type: string, label: Thing,",
        "thing: { type: string, required: true, label: Thing,",
    );
    assert!(DeviceSpec::from_yaml(&required)
        .unwrap_err()
        .contains("neither required"));
    let circular = SPEC.replace("path: /things }", "path: \"/things/{settings.thing}\" }");
    assert!(DeviceSpec::from_yaml(&circular)
        .unwrap_err()
        .contains("names the setting"));
    let unknown = SPEC.replace("params: { thing: thing }", "params: { other: thing }");
    assert!(DeviceSpec::from_yaml(&unknown)
        .unwrap_err()
        .contains("no parameter 'other'"));
    let declared = SPEC.replace(
        "state:\n",
        "state:\n  polled.account: { type: int, description: x }\n",
    );
    assert!(DeviceSpec::from_yaml(&declared).is_err());
}

#[test]
fn opened_without_its_selection_a_device_reads_only_what_needs_none() {
    let mut e = open(json!({}), true);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let sent = requests(&cx.take());
    let paths: Vec<&str> = sent.iter().map(|(_, _, p)| p.as_str()).collect();
    // The account's list, then the probe that needs no choice.
    assert_eq!(paths, ["/things"]);
    let (poll, ..) = sent[0];
    let mut cx = Cx::at(10, 1_700_000_000_250);
    e.http_response(&mut cx, poll, answer(200, r#"[{"id":"a","name":"A"}]"#));
    let a = cx.take();
    let patches = states(&a);
    assert!(
        patches.contains(&json!({"things": {"a": {"name": "A"}}})),
        "{patches:?}"
    );
    assert!(
        patches.contains(&json!({"polled": {"account": 1_700_000_000u64}})),
        "{patches:?}"
    );
    assert_eq!(
        requests(&a)
            .iter()
            .map(|(_, _, p)| p.as_str())
            .collect::<Vec<_>>(),
        ["/me"],
        "the account-level probe"
    );

    // A command naming the selection fails at once, saying how to choose.
    let mut cx = Cx::new(20);
    e.command(&mut cx, 7, "start_thing", &Params::new());
    let a = cx.take();
    assert!(requests(&a).is_empty());
    let failed = a.iter().find_map(|x| match x {
        Action::Complete {
            id: 7,
            result: Err(CommandError::InvalidParams { message }),
        } => Some(message.clone()),
        _ => None,
    });
    let message = failed.expect("invalid_params");
    assert!(
        message.contains("'thing'") && message.contains("list_things"),
        "{message}"
    );

    // The lister needs nothing and goes.
    let mut cx = Cx::new(30);
    e.command(&mut cx, 8, "list_things", &Params::new());
    e.pump(&mut cx);
    let _ = cx.take();

    // The next round still leaves the chosen thing's poll out.
    let mut cx = Cx::new(5000);
    e.timer(&mut cx, POLLS[1]);
    assert!(requests(&cx.take()).is_empty());
}

#[test]
fn choosing_the_selection_reads_it_at_once() {
    let mut e = open(json!({}), true);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let (poll, ..) = requests(&cx.take())[0];
    let mut cx = Cx::new(5);
    e.http_response(&mut cx, poll, answer(200, "[]"));
    let (probe, ..) = requests(&cx.take())[0];
    let mut cx = Cx::new(6);
    e.http_response(&mut cx, probe, answer(200, "{}"));
    cx.take();

    let spec = spec();
    let chosen =
        crate::catalog::validate(&spec.settings, json!({"thing": "a"}).as_object().unwrap())
            .unwrap();
    let mut cx = Cx::new(100);
    assert!(matches!(
        e.update_settings(&mut cx, &chosen),
        SettingsUpdate::Applied
    ));
    let sent = requests(&cx.take());
    assert_eq!(sent[0].2, "/things", "every group goes again: {sent:?}");
    // One at a time: answer the account list, then the chosen thing goes.
    let mut cx = Cx::new(110);
    e.http_response(&mut cx, sent[0].0, answer(200, "[]"));
    let next = requests(&cx.take());
    assert_eq!(next[0].2, "/things/a");
    // And the probe asks for the chosen thing now.
    assert_eq!(
        e.probe.as_ref().unwrap()["path"],
        "/things/{settings.thing}"
    );

    // Commands needing it go.
    let mut cx = Cx::new(120);
    e.command(&mut cx, 9, "start_thing", &Params::new());
    assert!(!cx.take().iter().any(|a| matches!(
        a,
        Action::Complete {
            id: 9,
            result: Err(_)
        }
    )));
}

#[test]
fn a_poll_group_is_stamped_once_a_second_and_only_when_answered() {
    let mut e = open(json!({"thing": "a"}), true);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let sent = requests(&cx.take());
    assert_eq!(sent[0].2, "/things");
    let mut cx = Cx::at(1, 1_700_000_000_100);
    e.http_response(&mut cx, sent[0].0, answer(200, "[]"));
    let a = cx.take();
    assert!(states(&a).contains(&json!({"polled": {"account": 1_700_000_000u64}})));
    // The second group: an error answer leaves its stamp unset.
    let second = requests(&a).remove(0);
    assert_eq!(second.2, "/things/a");
    let mut cx = Cx::at(2, 1_700_000_000_200);
    e.http_response(&mut cx, second.0, answer(500, "{}"));
    let a = cx.take();
    assert!(
        !states(&a).iter().any(|p| p.get("polled").is_some()),
        "{a:?}"
    );

    // Answered again within the same second: no patch.
    let mut cx = Cx::new(3);
    e.timer(&mut cx, POLLS[0]);
    e.pump(&mut cx);
    let again = requests(&cx.take());
    let account = again.iter().find(|r| r.2 == "/things").unwrap().0;
    let mut cx = Cx::at(4, 1_700_000_000_900);
    e.http_response(&mut cx, account, answer(200, "[]"));
    assert!(!states(&cx.take()).iter().any(|p| p.get("polled").is_some()));

    // A later second: the stamp moves on.
    let mut cx = Cx::new(5);
    e.timer(&mut cx, POLLS[0]);
    e.pump(&mut cx);
    let again = requests(&cx.take());
    if let Some(account) = again.iter().find(|r| r.2 == "/things") {
        let mut cx = Cx::at(6, 1_700_000_061_000);
        e.http_response(&mut cx, account.0, answer(200, "[]"));
        assert!(states(&cx.take()).contains(&json!({"polled": {"account": 1_700_000_061u64}})));
    }
}

#[test]
fn a_device_opened_for_commands_only_has_no_poll_stamps() {
    let mut e = open(json!({"thing": "a"}), false);
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    let a = cx.take();
    assert!(states(&a).is_empty());
    let sent = requests(&a);
    assert_eq!(sent.len(), 1, "the probe alone: {sent:?}");
    let mut cx = Cx::at(1, 1_700_000_000_000);
    e.http_response(&mut cx, sent[0].0, answer(200, "{}"));
    assert!(!states(&cx.take()).iter().any(|p| p.get("polled").is_some()));
}

#[test]
fn a_new_choice_clears_what_belonged_to_the_last() {
    let yaml = SPEC.replace(
        "select: { command: list_things, id: $.id, label: $.name }",
        "select: { command: list_things, id: $.id, label: $.name, clears: [thing] }",
    );
    let spec = DeviceSpec::from_yaml(&yaml).unwrap();
    let settings =
        crate::catalog::validate(&spec.settings, json!({"thing": "a"}).as_object().unwrap())
            .unwrap();
    let mut e = SpecEngine::new(
        Arc::new(spec.clone()),
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "m".into(),
            channels: None,
            settings,
            monitor: true,
        },
    )
    .unwrap();
    let mut cx = Cx::new(0);
    e.start(&mut cx);
    cx.take();
    let other =
        crate::catalog::validate(&spec.settings, json!({"thing": "b"}).as_object().unwrap())
            .unwrap();
    let mut cx = Cx::new(10);
    e.update_settings(&mut cx, &other);
    assert!(states(&cx.take()).contains(&json!({"thing": null})));
    // An undeclared path is refused.
    let bad = SPEC.replace(
        "select: { command: list_things, id: $.id, label: $.name }",
        "select: { command: list_things, id: $.id, clears: [nothing] }",
    );
    assert!(DeviceSpec::from_yaml(&bad)
        .unwrap_err()
        .contains("no declared state"));
}
