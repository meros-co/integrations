VPC = "visualproductions-cuecore"
# Visual Productions UDP API: one ASCII string per datagram to port 7000,
# nothing appended, "core" the default prefix (CueCore3 Manual rev. 8,
# Appendix E; CueCore2 Manual rev. 13, Appendix D; B-Station2, IoCore2,
# TimeCore and KioscTouch2 manuals). Expected strings are typed from the
# manuals' tables, not taken from the spec.


def vpc(command, input, wire, **extra):
    text(VPC, command, input, wire, **extra)


vpc("playback_go_forward", {"playback": 1}, "core-pb-1-go+", expect_result={"ok": {"kind": "unverified"}})
vpc("playback_go_back", {"playback": 16}, "core-pb-16-go-")
vpc("playback_jump", {"playback": 1, "cue": 3}, "core-pb-1-jump=3")
vpc("playback_release", {"playback": 2}, "core-pb-2-release")
vpc("playback_intensity", {"playback": 1, "level": 0.55}, "core-pb-1-intensity=0.550")
vpc("playback_rate", {"playback": 1, "rate": -0.5}, "core-pb-1-rate=-0.500")
vpc("release_all_playbacks", {}, "core-pb-release")
vpc("master_intensity", {"level": 1.0}, "core-pb-intensity=1.000")
vpc("master_rate", {"rate": 0.55}, "core-pb-rate=0.550")
vpc("master_fade", {"fade": "3s"}, "core-pb-fade=3s")
vpc("solo_playback", {"playback": 3}, "core-pb-solo=3")
vpc("set_blackout", {"enabled": True}, "core-pb-blackout=true", model="b-station2")
vpc("select_track", {"track": 128}, "core-tr-select=128")
vpc("erase_selected_track", {}, "core-tr-erase")
vpc("record_selected_track", {}, "core-tr-record")
vpc("stop_recording", {}, "core-tr-stop")
vpc("erase_track", {"track": 1}, "core-tr-1-erase")
vpc("record_track", {"track": 1}, "core-tr-1-record")
vpc("snapshot_dmx", {}, "core-tr-snapshot-dmx")
vpc("snapshot_artnet", {}, "core-tr-snapshot-artnet")
vpc("snapshot_sacn", {}, "core-tr-snapshot-sacn")
vpc("timecode_start", {}, "core-tc-start")
vpc("timecode_stop", {}, "core-tc-stop")
vpc("timecode_restart", {}, "core-tc-restart")
vpc("timecode_pause", {}, "core-tc-pause")
# TimeCore D.2: "For example 23:59:59.24".
vpc("timecode_set", {"timecode": "23:59:59.24"}, "core-tc-set=23:59:59.24")
vpc("execute_action", {"action_list": 1, "action": 2, "value": "true"}, "core-al-1-2-execute=true")
vpc("set_action_list_enabled", {"action_list": 1, "enabled": False}, "core-al-1-enable=false")
vpc("timer_start", {"timer": 1}, "core-tm-1-start")
vpc("timer_stop", {"timer": 4}, "core-tm-4-stop")
vpc("timer_restart", {"timer": 2}, "core-tm-2-restart")
vpc("timer_pause", {"timer": 3}, "core-tm-3-pause")
vpc("timer_set", {"timer": 1, "time": "01:02:03"}, "core-tm-1-set=01:02:03")
vpc("set_variable", {"variable": 1, "value": 42}, "core-va-1-set=42")
vpc("refresh_variable", {"variable": 1}, "core-va-1-refresh")
vpc("refresh_variables", {}, "core-va-refresh")
vpc("set_dmx_channel", {"channel": 1, "value": 255}, "core-dmx-1=255")
vpc("set_button", {"button": 1, "pressed": True}, "core-bu-1=true", model="b-station2")
vpc("set_led_intensity", {"button": 1, "level": 0.7}, "core-ld-1=0.700", model="b-station2")
# B-Station2 D.2: "An example for setting LED #3 to red is core-ld-3=#ff0000.
# To set all LEDs to blue send core-ld=#0000ff."
vpc("set_led_color", {"button": 3, "color": "#ff0000"}, "core-ld-3=#ff0000", model="b-station2")
vpc("set_all_leds_intensity", {"level": 0.5}, "core-ld=0.500", model="b-station2")
vpc("set_all_leds_color", {"color": "#0000ff"}, "core-ld=#0000ff", model="b-station2")
vpc("set_gpo", {"gpo": 1, "closed": True}, "core-gpo-1=1", model="iocore2")
vpc("select_page", {"page": 2}, "kiosc-page-select=2", model="kiosctouch2", settings={"prefix": "kiosc"})
vpc("blink", {}, "core-blink")
vpc("hello", {}, "core-hello", device_reply_hex=b"core-hello".hex(),
    expect_result={"ok": {"kind": "value", "value": "core-hello"}})
vpc("goodbye", {}, "core-goodbye")

# Feedback (CueCore3 Appendix E.5; IoCore2 D.3: "core-gpi-<pin>=ON/OFF" and "=50%").
telemetry(VPC, "playback-intensity", inbound_hex=b"core-pb-2-intensity=0.75".hex(),
          expect_state={"playbacks": {"2": {"intensity": 0.75}}})
telemetry(VPC, "playback-rate", inbound_hex=b"core-pb-1-rate=-0.25".hex(),
          expect_state={"playbacks": {"1": {"rate": -0.25}}})
telemetry(VPC, "playback-active", inbound_hex=b"core-pb-3-active=1".hex(),
          expect_state={"playbacks": {"3": {"active": True}}})
telemetry(VPC, "playback-cue", inbound_hex=b"core-pb-3-cue=7".hex(),
          expect_state={"playbacks": {"3": {"cue": 7}}})
telemetry(VPC, "master-intensity", inbound_hex=b"core-pb-intensity=0.5".hex(),
          expect_state={"master": {"intensity": 0.5}})
telemetry(VPC, "master-rate", inbound_hex=b"core-pb-rate=1".hex(),
          expect_state={"master": {"rate": 1.0}})
telemetry(VPC, "master-fade", inbound_hex=b"core-pb-fade=3s".hex(),
          expect_state={"master": {"fade": "3s"}})
telemetry(VPC, "action-list-enable", inbound_hex=b"core-al-1-enable=false".hex(),
          expect_state={"action_lists": {"1": {"enabled": False}}})
telemetry(VPC, "gpi-digital", inbound_hex=b"core-gpi-4=ON".hex(), model="iocore2",
          expect_state={"gpi": {"4": {"closed": True}}})
telemetry(VPC, "gpi-analog", inbound_hex=b"core-gpi-2=50%".hex(), model="iocore2",
          expect_state={"gpi": {"2": {"level": 50}}})
telemetry(VPC, "button", inbound_hex=b"core-bu-5=true".hex(), model="b-station2",
          expect_state={"buttons": {"5": {"pressed": True}}})
