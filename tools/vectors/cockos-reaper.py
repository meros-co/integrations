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
rp("set_track_receive_pan", {"track": 8, "receive": 2, "position": 0.5}, "/track/8/recv/2/pan", ("f", 0.5))
rp("set_master_send_volume", {"send": 1, "position": 0.75}, "/master/send/1/volume", ("f", 0.75))
rp("set_master_send_pan", {"send": 1, "position": 0.25}, "/master/send/1/pan", ("f", 0.25))
# ── Track effects ────────────────────────────────────────────────────────
rp("set_fx_bypass", {"track": 3, "fx": 1, "bypassed": True}, "/track/3/fx/1/bypass", ("i", 0))
rp("set_fx_ui_open", {"track": 3, "fx": 1, "open": True}, "/track/3/fx/1/openui", ("i", 1))
rp("next_fx_preset", {"track": 3, "fx": 2}, "/track/3/fx/2/preset+")
rp("previous_fx_preset", {"track": 3, "fx": 2}, "/track/3/fx/2/preset-")
rp("set_fx_preset", {"track": 3, "fx": 2, "preset": "Vocal"}, "/track/3/fx/2/preset", ("s", "Vocal"))
rp("set_fx_wet_dry", {"track": 3, "fx": 2, "position": 0.8}, "/track/3/fx/2/wetdry", ("f", 0.8))
# The config's example targets /track/3/fx/1,2,5/fxparam/6,7,7/value; one parameter here.
rp("set_fx_param", {"track": 3, "fx": 1, "param": 6, "position": 0.25}, "/track/3/fx/1/fxparam/6/value", ("f", 0.25))
# ── Track instruments (FX_INST_*), bypass inverted as for effects ─────────
rp("set_instrument_bypass", {"track": 4, "bypassed": True}, "/track/4/fxinst/bypass", ("i", 0))
rp("set_instrument_ui_open", {"track": 4, "open": False}, "/track/4/fxinst/openui", ("i", 0))
rp("next_instrument_preset", {"track": 4}, "/track/4/fxinst/preset+")
rp("previous_instrument_preset", {"track": 4}, "/track/4/fxinst/preset-")
rp("set_instrument_preset", {"track": 4, "preset": "Pad"}, "/track/4/fxinst/preset", ("s", "Pad"))
rp("set_instrument_param", {"track": 4, "param": 2, "position": 0.5}, "/track/4/fxinstparam/2/value", ("f", 0.5))
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
rp("set_device_send_count", {"count": 8}, "/device/send/count", ("i", 8))
rp("set_device_receive_count", {"count": 8}, "/device/receive/count", ("i", 8))
rp("set_device_fx_count", {"count": 16}, "/device/fx/count", ("i", 16))
rp("set_device_fx_param_count", {"count": 32}, "/device/fxparam/count", ("i", 32))
rp("set_device_fx_inst_param_count", {"count": 32}, "/device/fxinstparam/count", ("i", 32))
rp("select_device_fx", {"fx": 2}, "/device/fx/select", ("i", 2))
rp("select_fx_param_bank", {"bank": 2}, "/device/fxparam/bank/select", ("i", 2))
# ── Virtual MIDI keyboard ────────────────────────────────────────────────
rp("vkb_note", {"channel": 0, "note": 60, "velocity": 100}, "/vkb_midi/0/note/60", ("i", 100))
rp("vkb_cc", {"channel": 1, "controller": 7, "value": 127}, "/vkb_midi/1/cc/7", ("i", 127))
rp("vkb_program", {"channel": 0, "program": 5}, "/vkb_midi/0/program", ("i", 5))

# ── Telemetry: REAPER's feedback ─────────────────────────────────────────
# On connecting, the bank size, the marker and region counts, and the send,
# receive, FX, FX parameter and instrument parameter counts from the
# settings' defaults (32, 16, 16, then REAPER's own 4, 4, 8, 16, 16).
telemetry(RP, "track-mute",
          expect_connect_wire_hex=[hexs(osc("/device/track/count", ("i", 32))),
                                   hexs(osc("/device/marker/count", ("i", 16))),
                                   hexs(osc("/device/region/count", ("i", 16))),
                                   hexs(osc("/device/send/count", ("i", 4))),
                                   hexs(osc("/device/receive/count", ("i", 4))),
                                   hexs(osc("/device/fx/count", ("i", 8))),
                                   hexs(osc("/device/fxparam/count", ("i", 16))),
                                   hexs(osc("/device/fxinstparam/count", ("i", 16)))],
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
# FX parameter values for any track (FX_PARAM_VALUE n/track/@/fx/@/fxparam/@/value).
telemetry(RP, "fx-param-value", inbound_hex=hexs(osc("/track/3/fx/1/fxparam/6/value", ("f", 0.25))),
          expect_state={"tracks": {"3": {"fx": {"1": {"params": {"6": {"value": 0.25}}}}}}})
telemetry(RP, "fx-number", inbound_hex=hexs(osc("/track/3/fx/2/number/str", ("s", "2"))),
          expect_state={"tracks": {"3": {"fx": {"2": {"number": "2"}}}}})
# Names and displayed values only for the selected track (s/fx/@/fxparam/@/name).
telemetry(RP, "selected-track-fx-param-name", inbound_hex=hexs(osc("/fx/1/fxparam/6/name", ("s", "Gain"))),
          expect_state={"selected_track": {"fx": {"1": {"params": {"6": {"name": "Gain"}}}}}})
telemetry(RP, "selected-track-fx-param-text", inbound_hex=hexs(osc("/fx/1/fxparam/6/value/str", ("s", "-6.0"))),
          expect_state={"selected_track": {"fx": {"1": {"params": {"6": {"value_text": "-6.0"}}}}}})
telemetry(RP, "selected-track-fx-wet-text", inbound_hex=hexs(osc("/fx/2/wetdry/str", ("s", "80%"))),
          expect_state={"selected_track": {"fx": {"2": {"wet_text": "80%"}}}})
telemetry(RP, "selected-fx-name", inbound_hex=hexs(osc("/fx/name", ("s", "ReaComp"))),
          expect_state={"selected_fx": {"name": "ReaComp"}})
telemetry(RP, "selected-fx-bypassed", inbound_hex=hexs(osc("/fx/bypass", ("f", 0.0))),
          expect_state={"selected_fx": {"active": False}})
telemetry(RP, "selected-fx-param-value", inbound_hex=hexs(osc("/fxparam/3/value", ("f", 0.5))),
          expect_state={"selected_fx": {"params": {"3": {"value": 0.5}}}})
telemetry(RP, "selected-fx-param-name", inbound_hex=hexs(osc("/fxparam/3/name", ("s", "Ratio"))),
          expect_state={"selected_fx": {"params": {"3": {"name": "Ratio"}}}})
telemetry(RP, "selected-fx-param-bank", inbound_hex=hexs(osc("/device/fxparam/bank/str", ("s", "1-16"))),
          expect_state={"selected_fx": {"param_bank": "1-16"}})
telemetry(RP, "last-touched-fx-param", inbound_hex=hexs(osc("/fxparam/last_touched/value", ("f", 0.75))),
          expect_state={"last_touched_fx": {"param_value": 0.75}})
telemetry(RP, "last-touched-fx-track", inbound_hex=hexs(osc("/fx/last_touched/track/name", ("s", "Vox"))),
          expect_state={"last_touched_fx": {"track_name": "Vox"}})
# Receives (TRACK_RECV_*).
telemetry(RP, "receive-name", inbound_hex=hexs(osc("/track/8/recv/2/name", ("s", "Drums"))),
          expect_state={"tracks": {"8": {"receives": {"2": {"name": "Drums"}}}}})
telemetry(RP, "receive-volume", inbound_hex=hexs(osc("/track/8/recv/2/volume", ("f", 0.25))),
          expect_state={"tracks": {"8": {"receives": {"2": {"volume": 0.25}}}}})
telemetry(RP, "receive-volume-text", inbound_hex=hexs(osc("/track/8/recv/2/volume/str", ("s", "-12.0dB"))),
          expect_state={"tracks": {"8": {"receives": {"2": {"volume_text": "-12.0dB"}}}}})
telemetry(RP, "receive-pan", inbound_hex=hexs(osc("/track/8/recv/2/pan", ("f", 0.5))),
          expect_state={"tracks": {"8": {"receives": {"2": {"pan": 0.5}}}}})
telemetry(RP, "receive-pan-text", inbound_hex=hexs(osc("/track/8/recv/2/pan/str", ("s", "center"))),
          expect_state={"tracks": {"8": {"receives": {"2": {"pan_text": "center"}}}}})
telemetry(RP, "send-pan-text", inbound_hex=hexs(osc("/track/2/send/1/pan/str", ("s", "25%L"))),
          expect_state={"tracks": {"2": {"sends": {"1": {"pan_text": "25%L"}}}}})
telemetry(RP, "track-pan2-text", inbound_hex=hexs(osc("/track/2/pan2/str", ("s", "100%W"))),
          expect_state={"tracks": {"2": {"pan2_text": "100%W"}}})
telemetry(RP, "track-vu-left", inbound_hex=hexs(osc("/track/2/vu/L", ("f", 0.5))),
          expect_state={"tracks": {"2": {"vu_left": 0.5}}})
telemetry(RP, "track-vu-right", inbound_hex=hexs(osc("/track/2/vu/R", ("f", 0.25))),
          expect_state={"tracks": {"2": {"vu_right": 0.25}}})
# Instruments (FX_INST_*).
telemetry(RP, "instrument-name", inbound_hex=hexs(osc("/track/4/fxinst/name", ("s", "ReaSynth"))),
          expect_state={"tracks": {"4": {"instrument": {"name": "ReaSynth"}}}})
telemetry(RP, "instrument-bypassed", inbound_hex=hexs(osc("/track/4/fxinst/bypass", ("f", 0.0))),
          expect_state={"tracks": {"4": {"instrument": {"active": False}}}})
telemetry(RP, "instrument-ui", inbound_hex=hexs(osc("/track/4/fxinst/openui", ("f", 1.0))),
          expect_state={"tracks": {"4": {"instrument": {"ui_open": True}}}})
telemetry(RP, "instrument-preset", inbound_hex=hexs(osc("/track/4/fxinst/preset", ("s", "Pad"))),
          expect_state={"tracks": {"4": {"instrument": {"preset": "Pad"}}}})
telemetry(RP, "instrument-param", inbound_hex=hexs(osc("/track/4/fxinstparam/2/value", ("f", 0.5))),
          expect_state={"tracks": {"4": {"instrument": {"params": {"2": {"value": 0.5}}}}}})
telemetry(RP, "selected-instrument-param-name", inbound_hex=hexs(osc("/fxinstparam/2/name", ("s", "Attack"))),
          expect_state={"selected_track": {"instrument": {"params": {"2": {"name": "Attack"}}}}})
telemetry(RP, "selected-instrument-name", inbound_hex=hexs(osc("/fxinst/name", ("s", "ReaSynth"))),
          expect_state={"selected_track": {"instrument": {"name": "ReaSynth"}}})
# The master track's meters and sends.
telemetry(RP, "master-vu", inbound_hex=hexs(osc("/master/vu", ("f", 0.5))),
          expect_state={"master": {"vu": 0.5}})
telemetry(RP, "master-vu-left", inbound_hex=hexs(osc("/master/vu/L", ("f", 0.5))),
          expect_state={"master": {"vu_left": 0.5}})
telemetry(RP, "master-send-name", inbound_hex=hexs(osc("/master/send/1/name", ("s", "Stream"))),
          expect_state={"master": {"sends": {"1": {"name": "Stream"}}}})
telemetry(RP, "master-send-volume", inbound_hex=hexs(osc("/master/send/1/volume", ("f", 0.75))),
          expect_state={"master": {"sends": {"1": {"volume": 0.75}}}})
telemetry(RP, "master-send-pan-text", inbound_hex=hexs(osc("/master/send/1/pan/str", ("s", "center"))),
          expect_state={"master": {"sends": {"1": {"pan_text": "center"}}}})
