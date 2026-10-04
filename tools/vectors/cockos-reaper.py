# Cockos REAPER (cockos-reaper): OSC over UDP to REAPER's local listen port,
# every address from REAPER's Default.ReaperOSC pattern config with '@' as the
# target's number. Argument types follow the config's flags: n and f are OSC
# floats, b and i integers (1 and 0 for b, as the reaper-osc.js library sends
# them), s strings, and t (trigger or toggle) carries no argument. FX bypass
# is inverted on the wire: 0 bypasses.
RP = "cockos-reaper"


def rp(command, input, address, *args, **extra):
    binary(RP, command, input, osc(address, *args), **extra)


# ── Transport ────────────────────────────────────────────────────────────
for _c, _a in [("play", "/play"), ("stop", "/stop"), ("pause", "/pause"), ("record", "/record"),
               ("toggle_repeat", "/repeat"), ("toggle_metronome", "/click"), ("toggle_replace", "/replace"),
               ("toggle_auto_record_arm", "/autorecarm"), ("reset_solos", "/soloreset")]:
    rp(_c, {}, _a)
rp("rewind", {"held": True}, "/rewind", ("i", 1))
rp("forward", {"held": False}, "/forward", ("i", 0))
rp("go_to_marker", {"marker": 3}, "/marker", ("i", 3))
rp("go_to_region", {"region": 12}, "/region", ("i", 12))
rp("set_time", {"seconds": 62.5}, "/time", ("f", 62.5))
# The config's own example: /tempo/raw 100.351.
rp("set_tempo", {"bpm": 100.351}, "/tempo/raw", ("f", 100.351))
rp("set_playrate", {"rate": 1.5}, "/playrate/raw", ("f", 1.5))
# ── Master ───────────────────────────────────────────────────────────────
rp("set_master_volume", {"position": 0.75}, "/master/volume", ("f", 0.75))
rp("set_master_pan", {"position": 0.5}, "/master/pan", ("f", 0.5))
# ── Tracks ───────────────────────────────────────────────────────────────
# The config's example: /track/3/volume 0.5.
rp("set_track_volume", {"track": 3, "position": 0.5}, "/track/3/volume", ("f", 0.5))
rp("set_track_volume_db", {"track": 3, "db": -6.0}, "/track/3/volume/db", ("f", -6.0))
rp("set_track_pan", {"track": 2, "position": 0.25}, "/track/2/pan", ("f", 0.25))
rp("set_track_pan2", {"track": 2, "position": 1.0}, "/track/2/pan2", ("f", 1.0))
# The config's example: /track/3/mute 1 mutes track 3.
rp("set_track_mute", {"track": 3, "enabled": True}, "/track/3/mute", ("i", 1))
rp("toggle_track_mute", {"track": 3}, "/track/3/mute/toggle")
rp("set_track_solo", {"track": 4, "enabled": False}, "/track/4/solo", ("i", 0))
rp("toggle_track_solo", {"track": 4}, "/track/4/solo/toggle")
rp("set_track_arm", {"track": 1, "enabled": True}, "/track/1/recarm", ("i", 1))
rp("toggle_track_arm", {"track": 1}, "/track/1/recarm/toggle")
rp("set_track_monitor", {"track": 5, "mode": 2}, "/track/5/monitor", ("i", 2))
rp("set_track_selected", {"track": 6, "selected": True}, "/track/6/select", ("i", 1))
# The config's example: /track/3/name "vox".
rp("set_track_name", {"track": 3, "name": "vox"}, "/track/3/name", ("s", "vox"))
for _m in ["trim", "read", "latch", "touch", "write"]:
    rp(f"set_track_automation_{_m}", {"track": 7}, f"/track/7/auto{_m}")
rp("set_track_send_volume", {"track": 2, "send": 1, "position": 0.6}, "/track/2/send/1/volume", ("f", 0.6))
rp("set_track_send_pan", {"track": 2, "send": 1, "position": 0.4}, "/track/2/send/1/pan", ("f", 0.4))
rp("set_track_receive_volume", {"track": 8, "receive": 2, "position": 0.3}, "/track/8/recv/2/volume", ("f", 0.3))
# ── Track effects ────────────────────────────────────────────────────────
rp("set_fx_bypass", {"track": 3, "fx": 1, "bypassed": True}, "/track/3/fx/1/bypass", ("i", 0))
rp("set_fx_ui_open", {"track": 3, "fx": 1, "open": True}, "/track/3/fx/1/openui", ("i", 1))
rp("next_fx_preset", {"track": 3, "fx": 2}, "/track/3/fx/2/preset+")
rp("previous_fx_preset", {"track": 3, "fx": 2}, "/track/3/fx/2/preset-")
rp("set_fx_preset", {"track": 3, "fx": 2, "preset": "Vocal"}, "/track/3/fx/2/preset", ("s", "Vocal"))
rp("set_fx_wet_dry", {"track": 3, "fx": 2, "position": 0.8}, "/track/3/fx/2/wetdry", ("f", 0.8))
# The config's example targets /track/3/fx/1,2,5/fxparam/6,7,7/value; one parameter here.
rp("set_fx_param", {"track": 3, "fx": 1, "param": 6, "position": 0.25}, "/track/3/fx/1/fxparam/6/value", ("f", 0.25))
# ── Actions ──────────────────────────────────────────────────────────────
# The config's example: /action 40757 splits items at the edit cursor.
rp("run_action", {"command": 40757}, "/action", ("i", 40757))
rp("run_named_action", {"command": "_SWS_ABOUT"}, "/action/str", ("s", "_SWS_ABOUT"))
rp("run_midi_editor_action", {"command": 40001}, "/midiaction", ("i", 40001))
# ── The device's view ────────────────────────────────────────────────────
rp("select_track_bank", {"bank": 2}, "/device/track/bank/select", ("i", 2))
rp("select_device_track", {"track": 4}, "/device/track/select", ("i", 4))
rp("set_device_track_count", {"count": 64}, "/device/track/count", ("i", 64))
rp("set_device_marker_count", {"count": 8}, "/device/marker/count", ("i", 8))
rp("set_device_region_count", {"count": 0}, "/device/region/count", ("i", 0))
# ── Virtual MIDI keyboard ────────────────────────────────────────────────
rp("vkb_note", {"channel": 0, "note": 60, "velocity": 100}, "/vkb_midi/0/note/60", ("i", 100))
rp("vkb_cc", {"channel": 1, "controller": 7, "value": 127}, "/vkb_midi/1/cc/7", ("i", 127))
rp("vkb_program", {"channel": 0, "program": 5}, "/vkb_midi/0/program", ("i", 5))

# ── Telemetry: REAPER's feedback ─────────────────────────────────────────
# On connecting, the bank size and marker and region counts from the
# settings' defaults (32, 16, 16).
telemetry(RP, "track-mute",
          expect_connect_wire_hex=[hexs(osc("/device/track/count", ("i", 32))),
                                   hexs(osc("/device/marker/count", ("i", 16))),
                                   hexs(osc("/device/region/count", ("i", 16)))],
          inbound_hex=hexs(osc("/track/3/mute", ("f", 1.0))),
          expect_state={"tracks": {"3": {"mute": True}}})
telemetry(RP, "track-name", inbound_hex=hexs(osc("/track/3/name", ("s", "vox"))),
          expect_state={"tracks": {"3": {"name": "vox"}}})
telemetry(RP, "track-volume", inbound_hex=hexs(osc("/track/3/volume", ("f", 0.5))),
          expect_state={"tracks": {"3": {"volume": 0.5}}})
telemetry(RP, "track-volume-db", inbound_hex=hexs(osc("/track/3/volume/db", ("f", -6.0))),
          expect_state={"tracks": {"3": {"volume_db": -6.0}}})
telemetry(RP, "track-arm-int", inbound_hex=hexs(osc("/track/1/recarm", ("i", 0))),
          expect_state={"tracks": {"1": {"armed": False}}})
telemetry(RP, "track-monitor", inbound_hex=hexs(osc("/track/5/monitor", ("i", 2))),
          expect_state={"tracks": {"5": {"monitor": 2}}})
telemetry(RP, "fx-bypassed", inbound_hex=hexs(osc("/track/3/fx/1/bypass", ("f", 0.0))),
          expect_state={"tracks": {"3": {"fx": {"1": {"active": False}}}}})
telemetry(RP, "fx-name", inbound_hex=hexs(osc("/track/3/fx/1/name", ("s", "ReaEQ"))),
          expect_state={"tracks": {"3": {"fx": {"1": {"name": "ReaEQ"}}}}})
telemetry(RP, "send-volume", inbound_hex=hexs(osc("/track/2/send/1/volume/str", ("s", "-3.0dB"))),
          expect_state={"tracks": {"2": {"sends": {"1": {"volume_text": "-3.0dB"}}}}})
telemetry(RP, "play", inbound_hex=hexs(osc("/play", ("f", 1.0))), expect_state={"transport": {"playing": True}})
telemetry(RP, "record-off", inbound_hex=hexs(osc("/record", ("f", 0.0))), expect_state={"transport": {"recording": False}})
# The config's example: REAPER sends /tempo/raw 120.
telemetry(RP, "tempo", inbound_hex=hexs(osc("/tempo/raw", ("f", 120.0))), expect_state={"transport": {"tempo": 120.0}})
telemetry(RP, "time", inbound_hex=hexs(osc("/time", ("f", 12.5))), expect_state={"transport": {"time": 12.5}})
telemetry(RP, "beat", inbound_hex=hexs(osc("/beat/str", ("s", "5.2.00"))), expect_state={"transport": {"beat": "5.2.00"}})
telemetry(RP, "master-volume", inbound_hex=hexs(osc("/master/volume/str", ("s", "+0.00dB"))),
          expect_state={"master": {"volume_text": "+0.00dB"}})
telemetry(RP, "marker-name", inbound_hex=hexs(osc("/marker/2/name", ("s", "Chorus"))),
          expect_state={"markers": {"2": {"name": "Chorus"}}})
telemetry(RP, "region-length", inbound_hex=hexs(osc("/region/1/length", ("f", 30.0))),
          expect_state={"regions": {"1": {"length": 30.0}}})
telemetry(RP, "last-marker", inbound_hex=hexs(osc("/lastmarker/name", ("s", "Bridge"))),
          expect_state={"last_marker": {"name": "Bridge"}})
telemetry(RP, "selected-track", inbound_hex=hexs(osc("/track/name", ("s", "Lead"))),
          expect_state={"selected_track": {"name": "Lead"}})
