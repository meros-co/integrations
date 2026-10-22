K = "nec-projector-ascii"
# Sharp NEC Common ASCII Control Command (VDT24007): command name, a space,
# parameters, CR; responses ">" ... ended by the same line end; ">ok" for a
# setting (§1.1, §1.2, §2, §3).
OK = {"ok": {"kind": "ack"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


text(K, "set_power", {"on": True}, "power on\r", device_reply=">ok\r", expect_result=OK)
text(K, "set_power", {"on": False}, "power off\r", device_reply=">ok\r", expect_result=OK, file="set_power-off")
# The same on the PC CONTROL port (NP-P627UL manual p.139).
text(K, "set_power", {"on": True}, "power on\r", device_reply=">ok\r", expect_result=OK,
     settings={"link": "serial"}, file="set_power-serial")
text(K, "get_power", {}, "power\r", device_reply=">power warming\r", expect_result=val("warming"))
text(K, "get_power_info", {}, "power ?\r", device_reply=">power cur=off,sel=on|off\r",
     expect_result=val("cur=off,sel=on|off"))
text(K, "set_input", {"input": "video"}, "input video\r", device_reply=">ok\r", expect_result=OK)
text(K, "get_input", {}, "input\r", device_reply=">input video\r", expect_result=val("video"))
text(K, "get_input_info", {}, "input ?\r", device_reply=">input cur=hdmi1,sel=hdmi1|hdmi2|video\r",
     expect_result=val("cur=hdmi1,sel=hdmi1|hdmi2|video"))
text(K, "set_av_mute", {"target": "both", "muted": True}, "avmute on\r", device_reply=">ok\r", expect_result=OK)
text(K, "set_av_mute", {"target": "video", "muted": False}, "avmute video off\r", device_reply=">ok\r",
     expect_result=OK, file="set_av_mute-video")
text(K, "get_av_mute", {"target": "audio"}, "avmute audio\r", device_reply=">avmute audio on\r",
     expect_result=val("on"))
text(K, "get_av_mute_info", {"target": "both"}, "avmute ?\r", device_reply=">avmute cur=off,sel=on|off\r",
     expect_result=val("cur=off,sel=on|off"))
text(K, "get_serial_number", {}, "product serial\r", device_reply=">product serial 1234567AB\r",
     expect_result=val("1234567AB"))
text(K, "get_model_name", {}, "product model\r", device_reply=">product model NP-P627UL\r",
     expect_result=val("NP-P627UL"))
text(K, "get_firmware_version", {}, "product firmware1\r", device_reply=">product firmware1 1.02.031\r",
     expect_result=val("1.02.031"))
text(K, "get_product", {"item": "serial"}, "product serial\r", device_reply=">product serial 1234567AB\r",
     expect_result=val("serial 1234567AB"))
text(K, "get_light_hours", {}, "usage light hours\r", device_reply=">usage light hours 1520\r",
     expect_result=val("1520"))
text(K, "get_light_remaining", {}, "usage light remains\r", device_reply=">usage light remains 92\r",
     expect_result=val("92"))
text(K, "get_filter_hours", {}, "usage filter hours\r", device_reply=">usage filter hours 300\r",
     expect_result=val("300"))
text(K, "get_status", {}, "status\r", device_reply=">status standby;error:cover&filter;warning:light\r",
     expect_result=val("standby;error:cover&filter;warning:light"))
text(K, "get_status_info", {}, "status ?\r",
     device_reply=">status cur=standby,sel=cover|temp|fan|light|system|filter\r",
     expect_result=val("cur=standby,sel=cover|temp|fan|light|system|filter"))
text(K, "set_adjustment", {"item": "volume", "value": 10}, "volume 10\r", device_reply=">ok\r", expect_result=OK)
text(K, "set_adjustment", {"item": "light_adjust", "value": -5}, "light adjust -5\r", device_reply=">ok\r",
     expect_result=OK, file="set_adjustment-light")
text(K, "set_adjustment", {"item": "brightness", "value": 99}, "brightness 99\r",
     device_reply=">error:parameter\r", expect_result={"error": {"error": "device_error"}}, file="set_adjustment-error")
text(K, "step_adjustment", {"item": "volume", "direction": "up", "steps": 3}, "volume ++3\r",
     device_reply=">ok\r", expect_result=OK)
text(K, "step_adjustment", {"item": "hue", "direction": "down", "steps": 1}, "hue --1\r",
     device_reply=">ok\r", expect_result=OK, file="step_adjustment-down")
text(K, "get_adjustment", {"item": "volume"}, "volume\r", device_reply=">volume 25\r", expect_result=val("25"))
text(K, "get_adjustment_info", {"item": "volume"}, "volume ?\r",
     device_reply=">volume cur=25,def=10,min=0,max=30,step=1\r",
     expect_result=val("cur=25,def=10,min=0,max=30,step=1"))
text(K, "set_tile_matrix", {"enabled": True}, "tilematrix enable\r", device_reply=">ok\r", expect_result=OK)
text(K, "set_tile_matrix_value", {"item": "h_monitor", "value": "2"}, "tilematrix h_monitor 2\r",
     device_reply=">ok\r", expect_result=OK)
text(K, "get_tile_matrix", {"item": "position"}, "tilematrix position\r",
     device_reply=">tilematrix position 2\r", expect_result=val("2"))
text(K, "get_tile_matrix_info", {"item": "enabled"}, "tilematrix ?\r",
     device_reply=">tilematrix cur=disable,sel=enable|disable\r", expect_result=val("cur=disable,sel=enable|disable"))
text(K, "set_lens_position", {"target": "zoom", "position": 1000}, "lens zoom 1000\r", device_reply=">ok\r",
     expect_result=OK)
text(K, "move_lens_for", {"target": "focus", "seconds": -1.5}, "lens focus time -1.5\r", device_reply=">ok\r",
     expect_result=OK)
text(K, "start_lens", {"target": "h_shift", "direction": "+"}, "lens h_shift start+\r", device_reply=">ok\r",
     expect_result=OK)
text(K, "stop_lens", {"target": "h_shift"}, "lens h_shift stop\r", device_reply=">ok\r", expect_result=OK)
text(K, "lens_home", {}, "lens home\r", device_reply=">ok\r", expect_result=OK)
text(K, "get_lens_position", {"target": "v_shift"}, "lens v_shift\r", device_reply=">lens v_shift -120\r",
     expect_result=val("-120"))
text(K, "get_lens_info", {"target": "zoom"}, "lens zoom ?\r",
     device_reply=">lens zoom cur=1000,def=0,min=0,max=4095,step=1\r",
     expect_result=val("cur=1000,def=0,min=0,max=4095,step=1"))
text(K, "set_shutter", {"closed": True}, "shutter close\r", device_reply=">ok\r", expect_result=OK)
text(K, "get_shutter", {}, "shutter\r", device_reply=">shutter open\r", expect_result=val("open"))
text(K, "get_shutter_info", {}, "shutter ?\r", device_reply=">shutter cur=open,sel=open|close\r",
     expect_result=val("cur=open,sel=open|close"))
text(K, "auto_adjust", {}, "autoadj exec\r", device_reply=">error:busy\r",
     expect_result={"error": {"error": "device_error"}})
text(K, "set_freeze", {"frozen": True}, "freeze on\r", device_reply=">ok\r", expect_result=OK)
text(K, "get_freeze", {}, "freeze\r", device_reply=">freeze off\r", expect_result=val("off"))
text(K, "get_freeze_info", {}, "freeze ?\r", device_reply=">freeze cur=off,sel=on|off\r",
     expect_result=val("cur=off,sel=on|off"))
text(K, "help", {}, "help\r", device_reply=">power|input|avmute|status|help\r",
     expect_result=val("power|input|avmute|status|help"))
text(K, "get_command_help", {"command": "power"}, "power help\r", device_reply=">power[on|off|?]\r",
     expect_result=val("power[on|off|?]"))

telemetry(K, "power", inbound=">power on\r", expect_state={"power": "on"})
telemetry(K, "status", inbound=">status running;error:temp&fan;warning:filter\r",
          expect_state={"status": {"power": "running", "errors": "temp&fan", "warnings": "filter"}})
telemetry(K, "status-plain", inbound=">status standby\r", expect_state={"status": {"power": "standby"}})
telemetry(K, "input", inbound=">input hdmi2\r", expect_state={"input": "hdmi2"})
telemetry(K, "avmute", inbound=">avmute on\r", expect_state={"av_mute": "on"})
telemetry(K, "avmute-video", inbound=">avmute video off\r", expect_state={"av_mute_video": "off"})
telemetry(K, "avmute-audio", inbound=">avmute audio on\r", expect_state={"av_mute_audio": "on"})
telemetry(K, "freeze", inbound=">freeze on\r", expect_state={"freeze": "on"})
telemetry(K, "shutter", inbound=">shutter close\r", expect_state={"shutter": "close"})
telemetry(K, "adjustment", inbound=">contrast -3\r", expect_state={"adjustments": {"contrast": -3}})
telemetry(K, "light-adjust", inbound=">light adjust 70\r", expect_state={"adjustments": {"light_adjust": 70}})
telemetry(K, "lens", inbound=">lens zoom 1000\r", expect_state={"lens": {"zoom": 1000}})
telemetry(K, "serial", inbound=">product serial 1234567AB\r", expect_state={"info": {"serial_number": "1234567AB"}})
telemetry(K, "model", inbound=">product model NP-P627UL\r", expect_state={"info": {"model_name": "NP-P627UL"}})
telemetry(K, "firmware", inbound=">product firmware1 1.02.031\r",
          expect_state={"info": {"firmware_version": "1.02.031"}})
telemetry(K, "light-hours", inbound=">usage light hours 1520\r", expect_state={"usage": {"light_hours": 1520}})
telemetry(K, "light-remaining", inbound=">usage light remains 92\r", expect_state={"usage": {"light_remaining": 92}})
telemetry(K, "filter-hours", inbound=">usage filter hours 300\r", expect_state={"usage": {"filter_hours": 300}})
telemetry(K, "tilematrix", inbound=">tilematrix enable\r", expect_state={"tile_matrix": {"enabled": "enable"}})
