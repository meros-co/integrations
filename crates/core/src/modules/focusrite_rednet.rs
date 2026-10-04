//! Focusrite RedNet interfaces over AES70 (OCP.1 on TCP): the AES70 module,
//! with commands named for the controls Focusrite's object lists give.
//!
//! Focusrite publishes its RedNet AES70 objects only through the OCA
//! Alliance: the RedNet8 (MP8R) implementation chart and the RedNet virtual
//! device's object lists for RedNet 1, 2, 4 and 8. Every control there is a
//! standard class (OcaSwitch, OcaGain, OcaLevelSensor) directly in the root
//! block, with a role naming the control and its channel ("48V-1",
//! "Gain-3"). The chart and the virtual device number the objects
//! differently but agree on the roles, so the commands here address objects
//! by role and let the AES70 module look up their numbers.
//!
//! Each named command becomes one of the AES70 module's commands
//! (`set_switch` or `set_gain` on the role), which does the rest: the
//! session, the walk, subscriptions, meters and commands-only opening are
//! the AES70 module's (see its documentation). Opened for commands only, a
//! command naming a role asks the root block for its members and their
//! roles once, then remembers them.

use serde_json::{json, Value};

use super::aes70::Aes70;
use crate::catalog::Params;
use crate::module::{CommandError, CommandId, Cx, Key, Module, OpenContext, TcpInput};

pub(crate) struct RedNet {
    inner: Aes70,
}

impl RedNet {
    pub(crate) fn new(ctx: OpenContext, port: u16) -> RedNet {
        RedNet {
            inner: Aes70::new(ctx, port),
        }
    }
}

/// A switch position on a role: the object addressed by role path.
fn switch(role: String, position: i64) -> (&'static str, Params) {
    let mut p = Params::new();
    p.insert("object".into(), json!(role));
    p.insert("position".into(), json!(position));
    ("set_switch", p)
}

fn channel(params: &Params) -> Result<i64, CommandError> {
    params
        .get("channel")
        .and_then(Value::as_i64)
        .ok_or_else(|| CommandError::InvalidParams {
            message: "'channel' is required".into(),
        })
}

fn on(params: &Params, name: &str) -> i64 {
    params.get(name).and_then(Value::as_bool).unwrap_or(true) as i64
}

/// The AES70 command a named RedNet command becomes, or None for a command
/// the AES70 module takes as it is.
fn translate(name: &str, params: &Params) -> Result<Option<(&'static str, Params)>, CommandError> {
    let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or("");
    let invalid = |message: String| CommandError::InvalidParams { message };
    Ok(Some(match name {
        "set_phantom_power" => switch(format!("48V-{}", channel(params)?), on(params, "enabled")),
        "set_high_pass_filter" => {
            switch(format!("HPF-{}", channel(params)?), on(params, "enabled"))
        }
        "set_phase_invert" => switch(
            format!("Phase-{}", channel(params)?),
            on(params, "inverted"),
        ),
        "set_pad" => switch(format!("Pad-{}", channel(params)?), on(params, "enabled")),
        "set_gain_compensation" => {
            switch(format!("GComp-{}", channel(params)?), on(params, "enabled"))
        }
        "set_impedance" => {
            let position = match text("impedance") {
                "10k" => 0,
                "2k4" => 1,
                other => return Err(invalid(format!("unknown impedance '{other}'"))),
            };
            switch(format!("Impedance-{}", channel(params)?), position)
        }
        "set_input_source" => {
            let position = match text("source") {
                "mic" => 0,
                "line" => 1,
                "di" => 2,
                other => return Err(invalid(format!("unknown source '{other}'"))),
            };
            switch(format!("Source-{}", channel(params)?), position)
        }
        "set_channel_link" => switch(format!("Link{}", text("pair")), on(params, "enabled")),
        "set_fan" => switch("Fan".into(), on(params, "enabled")),
        "identify" => switch("Identify".into(), on(params, "enabled")),
        "set_front_panel_lock" => switch("FP Lock".into(), on(params, "enabled")),
        "set_preferred_leader" => switch("Preferred Master".into(), on(params, "enabled")),
        "set_preamp_gain" => {
            let gain = params
                .get("gain_db")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("'gain_db' is required".into()))?;
            let mut p = Params::new();
            p.insert("object".into(), json!(format!("Gain-{}", channel(params)?)));
            p.insert("gain_db".into(), json!(gain));
            ("set_gain", p)
        }
        _ => return Ok(None),
    }))
}

impl Module for RedNet {
    fn start(&mut self, cx: &mut Cx) {
        self.inner.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match translate(name, params) {
            Ok(Some((aes70, p))) => self.inner.command(cx, id, aes70, &p),
            Ok(None) => self.inner.command(cx, id, name, params),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.inner.tcp(cx, socket, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        self.inner.timer(cx, key);
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.inner.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn named_commands_become_switches_and_gains_on_their_roles() {
        let t = |name: &str, p: Value| translate(name, &params(p)).unwrap().unwrap();
        assert_eq!(
            t("set_phantom_power", json!({"channel": 3, "enabled": true})),
            (
                "set_switch",
                params(json!({"object": "48V-3", "position": 1}))
            )
        );
        assert_eq!(
            t("set_impedance", json!({"channel": 8, "impedance": "2k4"})),
            (
                "set_switch",
                params(json!({"object": "Impedance-8", "position": 1}))
            )
        );
        assert_eq!(
            t("set_input_source", json!({"channel": 2, "source": "di"})),
            (
                "set_switch",
                params(json!({"object": "Source-2", "position": 2}))
            )
        );
        assert_eq!(
            t("set_channel_link", json!({"pair": "3-4", "enabled": false})),
            (
                "set_switch",
                params(json!({"object": "Link3-4", "position": 0}))
            )
        );
        assert_eq!(
            t("set_preferred_leader", json!({"enabled": true})),
            (
                "set_switch",
                params(json!({"object": "Preferred Master", "position": 1}))
            )
        );
        assert_eq!(
            t("set_preamp_gain", json!({"channel": 1, "gain_db": 42.0})),
            (
                "set_gain",
                params(json!({"object": "Gain-1", "gain_db": 42.0}))
            )
        );
        // The AES70 module's own commands pass through.
        assert!(
            translate("set_switch", &params(json!({"ono": 4097, "position": 1})))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_named_command_goes_out_as_the_aes70_request() {
        use crate::module::Action;
        use std::net::{IpAddr, Ipv4Addr};
        let mut m = RedNet::new(
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 71)),
                host_name: None,
                port: Some(65000),
                model: "rednet-mp8r".into(),
                channels: Some(8),
                settings: Params::new(),
                monitor: false,
            },
            65000,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { to, .. } if to.port() == 65000)));
        // Not connected yet: the AES70 module answers, so the command reached it.
        let mut cx = Cx::new(1);
        m.command(&mut cx, 9, "set_pad", &params(json!({"channel": 2})));
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Complete { id: 9, .. })));
        // Connected and opened for commands only: the role is looked up
        // (the root block's members), not refused.
        let mut cx = Cx::new(2);
        m.tcp(&mut cx, "ocp1", TcpInput::Connected);
        cx.take();
        let mut cx = Cx::new(3);
        m.command(&mut cx, 10, "set_pad", &params(json!({"channel": 2})));
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(x, Action::TcpSend { .. })));
        assert!(!a.iter().any(|x| matches!(x, Action::Complete { .. })));
    }
}
