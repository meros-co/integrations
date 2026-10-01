# Analog Way LiveCore TPP (LiveCore TPP Programmer's Guide for v04.02.23 and
# the TPP variables document): ASCII command + LF on TCP 10600, answered with
# the register name, indexes and value + CR LF. Expected lines are written
# from the guide's syntax lines ("<scrn>,1SPCtkLF", "1PMloaLF", ...) with the
# 0-based indexes the guide states. A command sending several registers
# waits for each answer; the vector states the first line on the wire and
# feeds every answer, the full sequence being checked by
# crates/core/tests/analogway_session.rs.
LC = "analogway-livecore"
ACK = {"ok": {"kind": "ack"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


text(LC, "take", {"screen": 1}, "0,1SPCtk\n", device_reply="SPCtk0,1\r\n", expect_result=ACK)  # guide 3.6
text(LC, "take_all", {}, "1SPtsl\n", device_reply="SPtsl1\r\n", expect_result=ACK)                 # guide 3.7
text(LC, "set_take_list", {"screen": 4, "included": False}, "3,0SPscl\n",
     device_reply="SPscl3,0\r\n", expect_result=ACK)
text(LC, "set_take_list_on_master_load", {"enabled": True}, "1SPslu\n",
     device_reply="SPslu1\r\n", expect_result=ACK)
text(LC, "set_tbar", {"screen": 2, "position": 32768}, "1,32768SPCtb\n",
     device_reply="SPCtb1,32768\r\n", expect_result=ACK)

# Preset recalls (guide 3.2, 3.3): filter, scale, memory - 1, screen - 1,
# 0 program / 1 preview, then the load.
text(LC, "recall_preset", {"memory": 4, "screen": 2}, "4095PMcat\n",
     device_reply="PMcat4095\r\nPMlse0\r\nPMmet3\r\nPMscf1\r\nPMprf1\r\nPMloa1\r\n", expect_result=ACK)
text(LC, "recall_preset_and_take", {"memory": 1, "screen": 1, "preview": False, "filter": 4094, "scale": True},
     "4094PMcat\n",
     device_reply="PMcat4094\r\nPMlse1\r\nPMmet0\r\nPMscf0\r\nPMprf0\r\nPMlot1\r\n", expect_result=ACK)
text(LC, "recall_master_preset", {"memory": 10}, "4095PMcat\n",
     device_reply="PMcat4095\r\nPMlse0\r\nPSmet9\r\nPSprf1\r\nPSloa1\r\n", expect_result=ACK)
text(LC, "recall_master_preset_and_take", {"memory": 2, "preview": False}, "4095PMcat\n",
     device_reply="PMcat4095\r\nPMlse0\r\nPSmet1\r\nPSprf0\r\nPSlot1\r\n", expect_result=ACK)
text(LC, "set_master_preset_screen", {"screen": 3, "enabled": False}, "2,0PSose\n",
     device_reply="PSose2,0\r\n", expect_result=ACK)

# Layers (guide 3.4, 3.5): screen - 1, 0 program / 1 preview, layer - 1, source.
text(LC, "set_layer_source", {"screen": 1, "layer": 2, "source": 4}, "0,1,1,4SPPEi\n",
     device_reply="SPPEi0,1,1,4\r\n", expect_result=ACK)
text(LC, "set_background", {"screen": 2, "set": 3}, "1,1,3SPPNi\n",
     device_reply="SPPNi1,1,3\r\n", expect_result=ACK)

text(LC, "freeze_input", {"input": 5, "frozen": True}, "4,1INfrz\n", device_reply="INfrz4,1\r\n",
     expect_result=ACK)
text(LC, "black_input", {"input": 12, "black": False}, "11,0INbla\n", device_reply="INbla11,0\r\n",
     expect_result=ACK)

# Monitoring (guide 3.8-3.10): device - 1.
text(LC, "monitoring_fullscreen", {"source": 40}, "0,0MLupd\n",
     device_reply="MLupd0,0\r\nMLfen0,1\r\nMLfes0,40\r\nMLupd0,1\r\n", expect_result=ACK)
text(LC, "monitoring_window_source", {"window": 3, "source": 3}, "0,0MLupd\n",
     device_reply="MLupd0,0\r\nMLfen0,0\r\nMLces0,2,3\r\nMLupd0,1\r\n", expect_result=ACK)
text(LC, "recall_monitoring_preset", {"memory": 3, "device": 2}, "1,0MLupd\n",
     device_reply="MLupd1,0\r\nMMloa2,1,1\r\nMLupd1,1\r\n", expect_result=ACK)
text(LC, "recall_confidence_preset", {"memory": 16, "screen": 8}, "15,7,1CMloa\n",
     device_reply="CMloa15,7,1\r\n", expect_result=ACK)

# GPIO (guide 3.16, 3.17): GPI and GPO indexes from 0.
text(LC, "set_gpi_mode", {"gpi": 1, "take": True}, "0,1GPimo\n", device_reply="GPimo0,1\r\n",
     expect_result=ACK)
text(LC, "set_gpi_take_screen", {"gpi": 1, "screen": 4}, "0,3,1GPits\n", device_reply="GPits0,3,1\r\n",
     expect_result=ACK)
text(LC, "set_gpo_mode", {"gpo": 2, "mode": 0}, "1,0GPomo\n", device_reply="GPomo1,0\r\n",
     expect_result=ACK)
text(LC, "set_gpo", {"gpo": 2, "active": True}, "1,1GPofa\n", device_reply="GPofa1,1\r\n",
     expect_result=ACK)

# System (guide 3.14).
text(LC, "reboot", {}, "0,1PCreb\n", device_reply="PCreb0,1\r\n", expect_result=ACK)
text(LC, "shutdown", {"device": 2}, "1,1PCsht\n", device_reply="PCsht1,1\r\n", expect_result=ACK)
text(LC, "shutdown_wake_on_lan", {}, "0,2PCsht\n", device_reply="PCsht0,2\r\n", expect_result=ACK)

# Reads.
text(LC, "get_device_type", {}, "?\n", device_reply="DEV113\r\n", expect_result=val("113"))   # guide 3.1
text(LC, "get_device_state", {}, "PCdgs\n", device_reply="PCdgs255\r\n", expect_result=val("255"))
text(LC, "get_firmware", {}, "0,VEupd\n", device_reply="VEupd0,67239971\r\n",
     expect_result=val("67239971"))                                    # variables example: v4.02.23
text(LC, "get_command_set_version", {}, "0,TPver\n", device_reply="TPver0,2\r\n",
     expect_result=val("2"))                                           # guide 3.1
text(LC, "get_take_available", {"screen": 3}, "2,GCava\n", device_reply="GCava2,1\r\n",
     expect_result=val("1"))                                           # guide 3.6
text(LC, "get_layer_source", {"screen": 1, "layer": 1}, "0,1,0,SPPEi\n", device_reply="SPPEi0,1,0,7\r\n",
     expect_result=val("7"))
text(LC, "get_background", {"screen": 1, "preview": False}, "0,0,SPPNi\n", device_reply="SPPNi0,0,2\r\n",
     expect_result=val("2"))
text(LC, "get_tbar", {"screen": 1}, "0,SPCtb\n", device_reply="SPCtb0,65535\r\n", expect_result=val("65535"))
text(LC, "get_input_frozen", {"input": 1}, "0,INfrz\n", device_reply="INfrz0,0\r\n", expect_result=val("0"))
text(LC, "get_tally_program", {"source": 4}, "4,TAopr\n", device_reply="TAopr4,1\r\n",
     expect_result=val("1"))                                           # guide 3.18
text(LC, "get_tally_preview", {"source": 25}, "25,TAopw\n", device_reply="TAopw25,0\r\n",
     expect_result=val("0"))
text(LC, "get_gpi", {"gpi": 2}, "1,GPist\n", device_reply="GPist1,1\r\n", expect_result=val("1"))
text(LC, "read_back", {}, "3TPdie\n", device_reply="TPdie3\r\n", expect_result=ACK)  # guide 3.1
# An error answer (guide 2.4.5) fails the command.
text(LC, "send_raw", {"command": "BADXX"}, "BADXX\n", model="nextage-16", device_reply="E10\r\n",
     expect_result={"error": {"error": "device_error"}})

# ── Telemetry ──
telemetry(LC, "device-type", inbound="DEV108\r\n", expect_state={"device": {"type": 108}})
telemetry(LC, "device-state", inbound="PCdgs255\r\n", expect_state={"device": {"state": 255}})
telemetry(LC, "firmware", inbound="VEupd0,67239971\r\n", expect_state={"device": {"firmware": 67239971}})
telemetry(LC, "command-set", inbound="TPver0,2\r\n", expect_state={"device": {"command_set_version": 2}})
telemetry(LC, "controllers", inbound="TPcon0,2\r\n", expect_state={"device": {"tpp_controllers": 2}})
telemetry(LC, "temperature", inbound="TEdal0,1\r\n", expect_state={"device": {"temperature_alarm": 1}})
telemetry(LC, "fan", inbound="FAalm0,1\r\n", expect_state={"device": {"fan_alarm": True}})
telemetry(LC, "screen-enabled", inbound="SPise2,1\r\n", expect_state={"screens": {"3": {"enabled": True}}})
telemetry(LC, "taking", inbound="SPCtk0,0\r\n", expect_state={"screens": {"1": {"taking": False}}})
telemetry(LC, "take-available", inbound="GCava0,1\r\n",
          expect_state={"screens": {"1": {"take_available": True}}})
telemetry(LC, "take-list", inbound="SPscl1,1\r\n", expect_state={"screens": {"2": {"in_take_list": True}}})
telemetry(LC, "tbar", inbound="SPCtb0,65535\r\n", expect_state={"screens": {"1": {"tbar": 65535}}})
telemetry(LC, "layer-program", inbound="SPPEi0,0,1,4\r\n",
          expect_state={"screens": {"1": {"program": {"layers": {"2": {"source": 4}}}}}})
telemetry(LC, "layer-preview", inbound="SPPEi1,1,0,41\r\n",
          expect_state={"screens": {"2": {"preview": {"layers": {"1": {"source": 41}}}}}})
telemetry(LC, "background-program", inbound="SPPNi0,0,2\r\n",
          expect_state={"screens": {"1": {"program": {"background": 2}}}})
telemetry(LC, "background-preview", inbound="SPPNi0,1,0\r\n",
          expect_state={"screens": {"1": {"preview": {"background": 0}}}})
telemetry(LC, "input-available", inbound="INava3,1\r\n", expect_state={"inputs": {"4": {"available": True}}})
telemetry(LC, "input-frozen", inbound="INfrz0,1\r\n", expect_state={"inputs": {"1": {"frozen": True}}})
telemetry(LC, "input-black", inbound="INbla0,0\r\n", expect_state={"inputs": {"1": {"black": False}}})
telemetry(LC, "input-signal", inbound="ISsva0,4,1\r\n",
          expect_state={"inputs": {"1": {"plugs": {"4": {"signal": True}}}}})
telemetry(LC, "tally-program", inbound="TAopr4,1\r\n", expect_state={"tally": {"4": {"program": True}}})
telemetry(LC, "tally-preview", inbound="TAopw4,0\r\n", expect_state={"tally": {"4": {"preview": False}}})
telemetry(LC, "gpi", inbound="GPist0,1\r\n", expect_state={"gpi": {"1": {"active": True}}})
telemetry(LC, "gpo", inbound="GPofa1,1\r\n", expect_state={"gpo": {"2": {"active": True}}})
telemetry(LC, "monitoring-fullscreen", inbound="MLfen0,1\r\n",
          expect_state={"monitoring": {"1": {"fullscreen": True}}})
telemetry(LC, "monitoring-source", inbound="MLfes0,40\r\n",
          expect_state={"monitoring": {"1": {"fullscreen_source": 40}}})
telemetry(LC, "monitoring-window", inbound="MLces0,2,3\r\n",
          expect_state={"monitoring": {"1": {"windows": {"3": {"source": 3}}}}})
telemetry(LC, "preset-loading", inbound="PMloa0\r\n", expect_state={"presets": {"loading": False}})
telemetry(LC, "master-loading", inbound="PSloa1\r\n", expect_state={"master_presets": {"loading": True}})
telemetry(LC, "read-back", inbound="TPdie0\r\n", expect_state={"device": {"reading_back": False}})
