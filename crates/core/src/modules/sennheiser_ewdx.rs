//! Sennheiser EW-DX over SSCv2: HTTPS requests plus a server-sent event stream.
//!
//! Protocol from Sennheiser's SSCv2 specification. Resource paths are the ones
//! RFDeck exercised on real EW-DX receivers (OpenAPI 1.7): /api/channel/{id},
//! its signalQualityIndicator, level and warnings, /api/rf/channels/{id} and
//! /api/transmitters/{id}/battery. Only mute is writable: see the spec's quirks.
//! The SSCv2 session itself (probe, authentication, subscriptions, liveness)
//! is the shared client in [`super::sscv2`].
//!
//! Opened for commands only, it opens no subscription stream (so takes none of
//! the device's subscription sessions) and reads nothing on connecting: the
//! version request that finds the device, repeated whenever it has been quiet
//! for 3 s, is the liveness check.

use std::net::IpAddr;

use serde_json::{json, Map, Value};

use super::sscv2::{object, Call, SscDevice, Sscv2};
use crate::catalog::Params;
use crate::module::{CommandError, Cx, Level, Millis, OpenContext};

const MUTE_TIMEOUT: Millis = 3_000;

/// The EW-DX receiver's resources, state and commands.
pub(crate) struct EwdxDevice {
    channels: u32,
}

pub(crate) type Ewdx = Sscv2<EwdxDevice>;

impl Sscv2<EwdxDevice> {
    pub(crate) fn from_context(ctx: OpenContext) -> Ewdx {
        let password = ctx
            .settings
            .get("password")
            .and_then(Value::as_str)
            .unwrap_or("");
        let port = ctx.port.unwrap_or(443);
        let mut d = Ewdx::for_device(ctx.host, port, password, ctx.channels.unwrap_or(2));
        d.monitor = ctx.monitor;
        d
    }

    fn for_device(host: IpAddr, port: u16, password: &str, channels: u32) -> Ewdx {
        Sscv2::new(host, port, password, EwdxDevice { channels })
    }
}

impl SscDevice for EwdxDevice {
    fn resources(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for id in 0..self.channels {
            paths.push(format!("/api/channel/{id}"));
            paths.push(format!("/api/channel/{id}/signalQualityIndicator"));
            paths.push(format!("/api/channel/{id}/level"));
            paths.push(format!("/api/channel/{id}/warnings"));
            paths.push(format!("/api/rf/channels/{id}"));
            paths.push(format!("/api/transmitters/{id}/battery"));
        }
        paths
    }

    fn initial_reads(&self) -> Vec<String> {
        self.resources()
            .into_iter()
            .filter(|p| !p.ends_with("/warnings"))
            .collect()
    }

    fn apply_resource(&self, path: &str, value: &Value, patch: &mut Map<String, Value>) {
        apply_resource(path, value, patch);
    }

    fn read_failed(&self, cx: &mut Cx, path: &str, status: u16) {
        match status {
            // No transmitter linked: the battery resource answers 422.
            422 if path.contains("/transmitters/") => {
                let id: u32 = path
                    .split('/')
                    .nth(3)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                cx.state(json!({"channels": {(id + 1).to_string(): {"transmitter": null}}}));
            }
            404 if path.starts_with("/api/channel/") => cx.log(
                Level::Warning,
                format!("{path} does not exist; the device has fewer channels than this model"),
            ),
            _ => {}
        }
    }

    fn command(&self, name: &str, params: &Params) -> Result<Call, CommandError> {
        match name {
            "mute" => {
                let channel = params.get("channel").and_then(Value::as_i64).unwrap_or(1);
                if channel > self.channels as i64 {
                    return Err(CommandError::InvalidParams {
                        message: format!("this model has {} channels", self.channels),
                    });
                }
                let muted = params.get("muted").and_then(Value::as_bool).unwrap_or(true);
                // "Per SSCv2, a write is a PUT of that resource carrying only the
                // properties to change" (RFDeck, verified on OpenAPI 1.7).
                Ok(Call::put(
                    format!("/api/channel/{}", channel - 1),
                    json!({"mute": muted}),
                    MUTE_TIMEOUT,
                ))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

/// Map one resource onto the state tree. Channels are 1-based in the state and
/// 0-based in the API.
fn apply_resource(path: &str, value: &Value, patch: &mut Map<String, Value>) {
    if path == "/api/device/identity" {
        object(patch, "device").insert("identity".into(), value.clone());
        return;
    }
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let (id, field): (Option<u32>, Option<&str>) = match parts.as_slice() {
        ["api", "channel", id] => (id.parse().ok(), None),
        ["api", "channel", id, field] => (id.parse().ok(), Some(field)),
        ["api", "rf", "channels", id] => (id.parse().ok(), Some("rf")),
        ["api", "transmitters", id, "battery"] => (id.parse().ok(), Some("battery")),
        _ => (None, None),
    };
    let Some(id) = id else { return };
    let channel = object(object(patch, "channels"), &(id + 1).to_string());

    match field {
        None => {
            if let Some(v) = value.get("name").and_then(Value::as_str) {
                channel.insert("name".into(), json!(v));
            }
            if let Some(v) = value.get("mute").and_then(Value::as_bool) {
                channel.insert("mute".into(), json!(v));
            }
        }
        Some("signalQualityIndicator") => {
            if let Some(v) = value.get("value").filter(|v| v.is_number()) {
                channel.insert("rf".into(), json!({"quality_pct": v}));
            }
        }
        Some("level") => {
            if let Some(v) = value.get("value").filter(|v| v.is_number()) {
                channel.insert("af".into(), json!({"level_dbfs": v}));
            }
        }
        Some("warnings") => {
            channel.insert("warnings".into(), value.clone());
        }
        Some("rf") => {
            if let Some(v) = value.get("frequency").filter(|v| v.is_number()) {
                channel.insert("frequency_khz".into(), v.clone());
            }
        }
        Some("battery") => {
            if let Some(v) = value.get("gauge").filter(|v| v.is_number()) {
                channel.insert("transmitter".into(), json!({"battery_percent": v}));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::super::sscv2::{LIVENESS, QUIET_AFTER, RETRY, RETRY_AFTER, STREAM};
    use super::*;
    use crate::module::{
        Action, CommandId, CommandResult, Connection, HttpRequest, HttpResponse, Module, Outcome,
        RequestId, SseInput,
    };
    use crate::sse::SseEvent;
    use std::net::Ipv4Addr;

    fn device() -> Ewdx {
        Ewdx::for_device(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 443, "secret", 2)
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

    fn ok(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            body: body.to_string().into_bytes(),
        })
    }

    fn status(code: u16) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: code,
            body: Vec::new(),
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

    /// Start, answer the probe, open the stream and deliver the session id.
    fn streaming() -> (Ewdx, Vec<Action>) {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(
            &mut cx,
            probe,
            ok(json!({"protocol": "2.0", "schema": "1.5"})),
        );
        cx.take();
        let mut cx = Cx::new(20);
        d.sse(&mut cx, STREAM, SseInput::Opened);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Event(SseEvent {
                event: "open".into(),
                data: json!({"path": "/api/ssc/state/subscriptions/abc", "sessionUUID": "abc"})
                    .to_string(),
            }),
        );
        let actions = cx.take();
        (d, actions)
    }

    #[test]
    fn requests_are_authenticated_as_api() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let (_, probe) = requests(&cx.take()).remove(0);
        assert_eq!(probe.url, "https://10.0.0.5:443/api/ssc/version");
        // base64("api:secret")
        assert!(probe
            .headers
            .contains(&("Authorization".into(), "Basic YXBpOnNlY3JldA==".into())));
        assert!(probe.accept_invalid_certs);
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn unauthorized(actions: &[Action]) -> bool {
        actions
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. })))
    }

    fn sends_anything(actions: &[Action]) -> bool {
        actions.iter().any(|a| {
            matches!(
                a,
                Action::Http { .. } | Action::SseOpen { .. } | Action::SetTimer { .. }
            )
        })
    }

    /// Commands after a refusal fail with Auth and send nothing.
    fn assert_refused(d: &mut Ewdx) {
        let mut cx = Cx::new(90_000);
        d.command(
            &mut cx,
            7,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let a = cx.take();
        assert!(!sends_anything(&a));
        assert!(matches!(
            completed(&a)[0],
            (7, Err(CommandError::Auth { .. }))
        ));
    }

    #[test]
    fn a_rejected_password_is_terminal() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, status(401));
        let a = cx.take();
        assert!(unauthorized(&a));
        // No retry on any schedule: repeated failures can lock the device's
        // third-party access (RFDeck review item O).
        assert!(!sends_anything(&a));
        assert!(a.contains(&Action::CancelTimer { key: RETRY }));
        // A stray retry timer does nothing either.
        let mut cx = Cx::new(60_000);
        d.timer(&mut cx, RETRY);
        assert!(!sends_anything(&cx.take()));
        assert_refused(&mut d);
    }

    #[test]
    fn a_refusal_on_the_stream_is_terminal() {
        for code in [401, 403] {
            let (mut d, _) = streaming();
            let mut cx = Cx::new(30);
            d.sse(
                &mut cx,
                STREAM,
                SseInput::Closed {
                    status: Some(code),
                    reason: format!("HTTP {code}"),
                },
            );
            let a = cx.take();
            assert!(unauthorized(&a), "HTTP {code}");
            assert!(!sends_anything(&a), "HTTP {code}");
            assert_refused(&mut d);
        }
    }

    #[test]
    fn a_refused_liveness_check_is_not_proof_of_life() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(20 + QUIET_AFTER + 1);
        d.timer(&mut cx, LIVENESS);
        let (check, _) = requests(&cx.take())
            .into_iter()
            .find(|(_, r)| r.url.ends_with("/api/ssc/version"))
            .unwrap();
        let mut cx = Cx::new(20 + QUIET_AFTER + 2);
        d.http_response(&mut cx, check, status(401));
        let a = cx.take();
        assert!(!a.contains(&Action::Alive));
        assert!(unauthorized(&a));
        assert!(!sends_anything(&a));
        assert_refused(&mut d);
    }

    #[test]
    fn a_refused_subscription_or_fetch_is_terminal() {
        let (mut d, actions) = streaming();
        let reqs = requests(&actions);
        let (subscribe, _) = reqs.iter().find(|(_, r)| r.method == "PUT").unwrap();
        let (fetch, _) = reqs.iter().find(|(_, r)| r.method == "GET").unwrap();
        let mut cx = Cx::new(30);
        d.http_response(&mut cx, *subscribe, status(403));
        let a = cx.take();
        assert!(unauthorized(&a));
        assert!(a.contains(&Action::SseClose { stream: STREAM }));
        assert!(!sends_anything(&a));
        // Responses still in flight report nothing further.
        let mut cx = Cx::new(31);
        d.http_response(&mut cx, *fetch, status(401));
        assert!(cx.take().is_empty());
        assert_refused(&mut d);
    }

    #[test]
    fn a_mute_refused_mid_session_fails_with_auth_and_stops() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(
            &mut cx,
            3,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let (put, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, put, status(401));
        let a = cx.take();
        assert!(matches!(
            completed(&a)[0],
            (3, Err(CommandError::Auth { .. }))
        ));
        assert!(unauthorized(&a));
        assert_refused(&mut d);
    }

    #[test]
    fn subscribes_every_channel_in_batches_after_the_session_id() {
        let (mut d, actions) = streaming();
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let reqs = requests(&actions);
        let (first_id, first) = &reqs[0];
        assert_eq!(first.method, "PUT");
        assert_eq!(
            first.url,
            "https://10.0.0.5:443/api/ssc/state/subscriptions/abc/add"
        );
        let body: Value = serde_json::from_slice(first.body.as_ref().unwrap()).unwrap();
        assert_eq!(body.as_array().unwrap().len(), 4);

        // The next batch follows the first's success, until all twelve paths
        // (six per channel, two channels) are subscribed.
        let mut id = *first_id;
        let mut total = 4;
        loop {
            let mut cx = Cx::new(30);
            d.http_response(&mut cx, id, status(200));
            let next: Vec<_> = requests(&cx.take())
                .into_iter()
                .filter(|(_, r)| r.method == "PUT")
                .collect();
            let Some((next_id, req)) = next.into_iter().next() else {
                break;
            };
            let body: Value = serde_json::from_slice(req.body.as_ref().unwrap()).unwrap();
            total += body.as_array().unwrap().len();
            id = next_id;
        }
        assert_eq!(total, 12);
    }

    #[test]
    fn notifications_map_onto_channels() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({
                "/api/channel/1": {"name": "Vox", "mute": false},
                "/api/channel/1/signalQualityIndicator": {"value": 87},
                "/api/channel/1/level": {"value": -42.5},
                "/api/rf/channels/1": {"frequency": 606500},
                "/api/transmitters/1/battery": {"gauge": 65},
            }),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        let ch = &patch.unwrap()["channels"]["2"];
        assert_eq!(ch["name"], "Vox");
        assert_eq!(ch["rf"]["quality_pct"], 87);
        assert_eq!(ch["af"]["level_dbfs"], -42.5);
        assert_eq!(ch["frequency_khz"], 606500);
        // A percentage, per EW-DX SSC §8.106 (RFDeck review item A).
        assert_eq!(ch["transmitter"]["battery_percent"], 65);
    }

    #[test]
    fn path_value_notifications_are_accepted() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({"path": "/api/channel/0", "value": {"mute": true}}),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        assert_eq!(patch.unwrap()["channels"]["1"]["mute"], true);
    }

    #[test]
    fn mute_puts_the_channel_resource() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        let params = json!({"channel": 2, "muted": true})
            .as_object()
            .unwrap()
            .clone();
        d.command(&mut cx, 9, "mute", &params);
        let (id, req) = requests(&cx.take()).remove(0);
        assert_eq!(req.method, "PUT");
        assert_eq!(req.url, "https://10.0.0.5:443/api/channel/1");
        assert_eq!(req.body.as_deref(), Some(br#"{"mute":true}"#.as_slice()));

        let mut cx = Cx::new(60);
        d.http_response(&mut cx, id, status(200));
        assert_eq!(completed(&cx.take()), [(9, Ok(Outcome::Ack))]);
    }

    #[test]
    fn mute_failures_are_classified() {
        let (mut d, _) = streaming();
        let params = json!({"channel": 1}).as_object().unwrap().clone();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, "mute", &params);
        d.command(&mut cx, 2, "mute", &params);
        let ids: Vec<_> = requests(&cx.take()).into_iter().map(|(i, _)| i).collect();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, ids[0], status(404));
        d.http_response(&mut cx, ids[1], Err("connection reset".into()));
        let done = completed(&cx.take());
        assert!(
            matches!(&done[0].1, Err(CommandError::DeviceError { code: Some(c), .. }) if c == "404")
        );
        assert!(matches!(&done[1].1, Err(CommandError::Transport { .. })));
    }

    #[test]
    fn a_quiet_stream_is_probed_and_dropped_after_two_failures() {
        let (mut d, _) = streaming();
        let mut failures = 0;
        for now in [3_100, 6_200] {
            let mut cx = Cx::new(now);
            d.timer(&mut cx, LIVENESS);
            let (id, req) = requests(&cx.take()).remove(0);
            assert!(req.url.ends_with("/api/ssc/version"));
            let mut cx = Cx::new(now + 10);
            d.http_response(&mut cx, id, Err("timed out".into()));
            failures += 1;
            let a = cx.take();
            let dropped = a.contains(&Action::SseClose { stream: STREAM });
            assert_eq!(dropped, failures == 2);
        }
    }

    #[test]
    fn opened_for_commands_only_it_opens_no_stream() {
        let mut d = device();
        d.monitor = false;
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.0"})));
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // No stream, no subscription, no identity or state read.
        assert!(!a.iter().any(|x| matches!(x, Action::SseOpen { .. })));
        assert!(requests(&a).is_empty());

        // Quiet for 3 s: the version is asked as the liveness check.
        let mut cx = Cx::new(10 + QUIET_AFTER);
        d.timer(&mut cx, LIVENESS);
        let (check, req) = requests(&cx.take()).remove(0);
        assert!(req.url.ends_with("/api/ssc/version"));
        let mut cx = Cx::new(10 + QUIET_AFTER + 8);
        d.http_response(&mut cx, check, ok(json!({"protocol": "2.0"})));
        let a = cx.take();
        assert!(a.contains(&Action::Alive));
        assert!(a.contains(&Action::RoundTrip(8)));

        // Commands work.
        let mut cx = Cx::new(5_000);
        d.command(
            &mut cx,
            4,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let (put, req) = requests(&cx.take()).remove(0);
        assert_eq!(req.url, "https://10.0.0.5:443/api/channel/0");
        let mut cx = Cx::new(5_030);
        d.http_response(&mut cx, put, status(200));
        let a = cx.take();
        assert_eq!(completed(&a), [(4, Ok(Outcome::Ack))]);
        assert!(a.contains(&Action::RoundTrip(30)));

        // Losing it closes no stream, since none was opened.
        let mut cx = Cx::new(9_000);
        d.lost(&mut cx, "gone".into());
        assert!(!cx.take().contains(&Action::SseClose { stream: STREAM }));
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut d = device();
        let mut cx = Cx::new(100);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(140);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.0"})));
        assert!(cx.take().contains(&Action::RoundTrip(40)));

        // A request that fails in transport was not answered.
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, "mute", &params(json!({"channel": 1})));
        let (id, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(2_000);
        d.http_response(&mut cx, id, Err("timed out".into()));
        assert!(!cx.take().iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }

    #[test]
    fn a_closed_stream_reconnects() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(70);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Closed {
                status: None,
                reason: "stream ended".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_AFTER
        }));
        // The last known state stays; the connection status says it is stale.
        assert!(!a.iter().any(|x| matches!(x, Action::State(_))));
    }
}
