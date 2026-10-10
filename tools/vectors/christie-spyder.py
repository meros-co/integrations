CSP = "christie-spyder"
# Christie Spyder external control: an ASCII command in one UDP datagram to
# port 11116, preceded by the 10-byte header "spyder" + four 0x00 bytes, with
# no space after the header (X20 User Manual 020-000916-01 pp. 111-128; X80
# Serial Commands Technical Reference 020-102207-01). Expected strings are
# typed from the documents' syntax lines; the answer is a datagram whose first
# argument is the result code (0 success).
_SPH = "spyder\x00\x00\x00\x00"
_SP_OK = {"ok": {"kind": "ack"}}


def _sp(command, input, wire, **extra):
    # The answer is one datagram, given as hex.
    if "device_reply" in extra:
        extra["device_reply_hex"] = extra.pop("device_reply").encode("ascii").hex()
    text(CSP, command, input, _SPH + wire, **extra)


# Presets, scripts, keys
_sp("recall_basic_preset", {"preset": 5}, "BPR 5", device_reply="0", expect_result=_SP_OK)
_sp("recall_basic_preset_duration", {"preset": 5, "duration": 30}, "BPR 5 30")
_sp("learn_basic_preset", {"preset": 7}, "BPL 7")
_sp("learn_basic_preset_duration", {"preset": 7, "duration": 60}, "BPL 7 60")
_sp("recall_script_cue", {"script": 10, "cue": 1}, "RSC 10 1 S", device_reply="0", expect_result=_SP_OK)
# X20 UM: register 2 on page 3 is 3002.
_sp("recall_register_cue", {"register": 3002, "cue": 0}, "RSC 3002 0 R",
    device_reply="4", expect_result={"error": {"error": "device_error"}})
_sp("function_key_recall", {"key": 3}, "FKR 3")
_sp("function_key_recall_layers", {"key": 3, "layers": "2 3"}, "FKR 3 2 3")
_sp("function_key_recall_register", {"register": 1002}, "FKR 1002 R")
_sp("learn_command_key", {"name": "Look 1", "register": 4}, "LCK 0 Look%201 4 3 0",
    device_reply="0 12 10", expect_result={"ok": {"kind": "value", "value": "12 10"}})
_sp("delete_command_key", {"script": 10}, "DCK 10 S")
_sp("delete_command_key_register", {"register": 4}, "DCK 4 R")
_sp("apply_register", {"type": 6, "register": 2, "layers": "2 4"}, "ARL 6 2 2 4")
# Layers
_sp("transition_layers", {"mix_on": True, "duration": 30, "layers": "2 3 4"}, "TRN 1 30 2 3 4",
    device_reply="0", expect_result=_SP_OK)
_sp("freeze_layers", {"frozen": False, "layers": "<DVCE2:PGM>"}, "FRZ 0 <DVCE2:PGM>")
_sp("source_apply", {"source": "Camera 1", "layers": "2"}, "SRA Camera%201 2")
_sp("layer_assign_pixelspace", {"pixelspace": 1, "visible": True, "layers": "5"}, "LAP 1 1 5")
# X20 UM: "KSZ 800 <DVCE0:PVW>".
_sp("layer_size", {"width": 800, "layers": "<DVCE0:PVW>"}, "KSZ 800 <DVCE0:PVW>")
_sp("layer_position", {"relative": True, "x": -10, "y": 20, "layers": "2"}, "KPS 1 -10 20 2")
_sp("layer_size_position", {"x": 0, "y": 0, "width": 1920, "layers": "2 3"}, "LSP 0 0 0 1920 2 3")
_sp("layer_crop", {"left": 0.1, "right": 0, "top": 0.25, "bottom": 0, "layers": "2"}, "CRP 0.100 0.000 0.250 0.000 2")
_sp("layer_aspect_ratio", {"type": "t", "ratio": 1.778, "layers": "2"}, "ARO t 1.778 2")
_sp("layer_border", {"layer": 2, "thickness": -8}, "KBD 2 -8")
_sp("layer_border_full", {"layer": 2, "thickness": 4, "red": 255, "green": 0, "blue": 0, "h_bevel": 0,
                          "v_bevel": 0, "inside_softness": 10}, "KBD 2 4 255 0 0 0 0 10")
_sp("layer_shadow", {"layer": 3, "x": 10, "y": 10, "size": 128, "transparency": 64, "softness": 20}, "KSH 3 10 10 128 64 20")
_sp("layer_zoom_pan", {"zoom": 2, "h_pan": -100, "v_pan": 0, "layer": 2}, "ZPA 0 2.000 -100.000 0.000 2")
_sp("layer_clone", {"layer": 2, "mode": 2}, "LCC 2 2")
_sp("layer_clone_offset", {"layer": 2, "mode": 1, "offset": 0.5}, "LCC 2 1 0.500")
_sp("layer_alignment", {"effect": 24, "duration": 30, "layers": "2 3"}, "LAC 24 30 2 3")
_sp("treatment_recall", {"treatment": 4, "layers": "2 3"}, "KTR 4 2 3")
_sp("treatment_learn", {"treatment": -1, "layer": 2}, "KTL -1 2")
# Input
_sp("input_levels", {"brightness": 1, "contrast": 1.2, "hue": 0, "saturation": 1, "layers": "2"},
    "ILA 1.000 1.200 0.000 1.000 2")
_sp("input_levels_gamma", {"brightness": 1, "contrast": 1, "hue": -10, "saturation": 1, "gamma": 2.2, "layers": "2"},
    "ILA 1.000 1.000 -10.000 1.000 2.200 2", model="x80")
_sp("luminance_key", {"enabled": False, "layers": "4"}, "ILK 0 0 0 4")
_sp("color_key", {"red": 0, "green": 255, "blue": 0, "range_red": 40, "range_green": 40, "range_blue": 40,
                  "gain": 256, "layers": "4 5"}, "ICK 1 0 255 0 40 40 40 256 4 5")
_sp("input_config_recall", {"config": 3, "layer": 2}, "ICR 3 2")
_sp("input_auto_sync", {"layer": 2, "connector": 2}, "ICR -1 2 2")
_sp("input_config_learn", {"config": 3, "layer": 2}, "ICL 3 2")
_sp("input_raster", {"layer": 2, "edge": "L", "pixels": -4}, "IRA 2 L -4")
_sp("input_auto_raster", {"layer": 2}, "IRA 2 A")
# Stills and background
_sp("still_load_layers", {"file": "logo.png", "layers": "2"}, "SLD logo.png 2")
_sp("still_clear_layers", {"layers": "2 3"}, "SCL 2 3")
_sp("background_load", {"file": "Stage BG.jpg", "pixelspace": 0}, "BLD Stage%20BG.jpg 0 0")
_sp("background_transition", {}, "BTR")
_sp("background_transition_duration", {"duration": 45}, "BTR 45")
_sp("still_load_output", {"file": "grid.bmp", "output": 1}, "LSO grid.bmp 1")
_sp("still_load_output_channel", {"file": "grid.bmp", "output": 1, "channel": 3}, "LSO grid.bmp 1 3")
_sp("still_clear_output", {"output": 1}, "CSO 1")
_sp("still_clear_output_channel", {"output": 1, "channel": 0}, "CSO 1 0")
# Mixers
_sp("device_mixer_transition", {"duration": 1, "devices": "0 1"}, "DMT 1 0 1")
_sp("device_mixer_bus", {"duration": 30, "bus": "PGM", "devices": "2"}, "DMB 30 PGM 2")
# Outputs
_sp("freeze_outputs", {"outputs": "0 1"}, "OFZ 1 0 1")
_sp("output_format", {"output": 0, "width": 1920, "height": 1080, "rate": 59.94}, "OCF 0 1920 1080 59.94 0")
_sp("output_format_timing", {"output": 0, "width": 1920, "height": 1200, "rate": 60}, "OCF 0 1920 1200 60.00 0 1")
_sp("output_mode_normal", {"output": 2}, "OCM 2 Normal")
_sp("output_mode_normal_start", {"output": 2, "h_start": 1920, "v_start": 0}, "OCM 2 Normal 1920 0")
_sp("output_mode_opmon", {"output": 3, "pixelspace": 1}, "OCM 3 OpMon 1")
_sp("output_mode_scaled", {"output": 3, "pixelspace": 0}, "OCM 3 Scaled 0")
_sp("output_rotation", {"output": 0, "angle": "90"}, "OCR 0 90")
_sp("output_blend", {"output": 0, "edge": "R", "width": 256, "mode": "Gamma", "curve1": 0.5, "curve2": 0.25},
    "OCB 0 R 1 256 Gamma 0.500 0.250")
_sp("output_blend_enabled", {"output": 0, "edge": "L", "enabled": False}, "OCB 0 L 0")
_sp("output_save", {"output": 0}, "OCS 0")
# Routers
_sp("router_switch", {"router": 0, "output": 3, "input": 7}, "RCR 0 L 3 7")
_sp("router_switch_physical", {"router": 0, "output": 3, "input": 7}, "RCR 0 P 3 7")
_sp("router_query", {"router": 0}, "QRC 0", device_reply="0 0 0:3 1:-1",
    expect_result={"ok": {"kind": "value", "value": "0 0:3 1:-1"}})
_sp("router_query_output", {"router": 0, "output": 1}, "QRC 0 1")
# System
_sp("save", {}, "SAV")
_sp("restart_server", {}, "SDN 1")
_sp("power_off", {}, "SDN 0")
# Queries
_sp("get_layer_count", {}, "RLC", device_reply="0 18", expect_result={"ok": {"kind": "value", "value": "18"}})
_sp("get_source_names", {}, "RSN", device_reply="0 Camera%201 PC", expect_result={"ok": {"kind": "value", "value": "Camera%201 PC"}})
_sp("get_basic_presets", {}, "RBL", device_reply="0 2 1 Opening 2 Keynote",
    expect_result={"ok": {"kind": "value", "value": "2 1 Opening 2 Keynote"}})
_sp("get_register_count", {"type": 4}, "RRC 4")
_sp("get_registers", {"type": 10}, "RRL 10 -1")
_sp("get_script_cue", {"script": 10}, "SCR 10 S", device_reply="0 -1", expect_result={"ok": {"kind": "value", "value": "-1"}})
_sp("get_register_cue", {"register": 4}, "SCR 4 R")
_sp("get_layer_keyframe", {"layer": 2}, "RLK 2")
# An empty layer answers the Empty code.
_sp("get_layer_source", {"layer": 5}, "RLS 5", device_reply="1", expect_result={"error": {"error": "device_error"}})
_sp("get_connection_status", {"layer": 2}, "RCS 2", device_reply="0 2 1 1", expect_result={"ok": {"kind": "value", "value": "2 1 1"}})
_sp("get_pixelspaces", {}, "RPD")
_sp("get_io_status", {}, "RPS")
_sp("get_aspect_ratio", {"source": "Camera%201"}, "RAR Camera%201")

# State: Spyder never pushes and its answers do not name what they answer, so
# each answer is read with the query it answers (`request`, header included).
_KF = ("0 0.5 0.25 100 50 960 540 4 255 0 0 2 2 10 0 f 6 6 128 20 64 1 0.5 0.1 0 0.25 0 1 "
       "0 2 -100 0 1 0")
_KF_STATE = {
    "h_position_relative": 0.5, "v_position_relative": 0.25, "x": 100.0, "y": 50.0,
    "width": 960.0, "height": 540.0,
    "border": {"thickness": 4.0, "red": 255.0, "green": 0.0, "blue": 0.0, "h_bevel": 2.0, "v_bevel": 2.0,
               "inside_softness": 10.0, "outside_softness": 0.0, "outside_edges": "f"},
    "shadow": {"h_offset": 6.0, "v_offset": 6.0, "size": 128.0, "softness": 20.0, "transparency": 64.0},
    "clone": {"mode": "offset", "offset": 0.5},
    "crop": {"left": 0.1, "right": 0.0, "top": 0.25, "bottom": 0.0, "anchor": "window_center"},
    "aspect_offset": 0.0, "zoom": 2.0, "h_pan": -100.0, "v_pan": 0.0, "pixelspace": 1,
    "transparency": 0.0}


def _spt(name, inbound, request, **extra):
    telemetry(CSP, name, inbound=inbound, request=_SPH + request, **extra)


# RLK: the keyframe of an existing layer, then the next layer, and this
# layer's source and aspect ratio (X80 TR p.32).
_spt("layer-keyframe", _KF, "RLK 2", expect_state={"layers": {"2": _KF_STATE}},
     expect_then_send=[_SPH + "RLK 3", _SPH + "RLS 2", _SPH + "RAR 2"])
# Values appended by a later version are ignored.
_spt("layer-keyframe-longer", _KF + " 7 8", "RLK 4", expect_state={"layers": {"4": _KF_STATE}},
     expect_then_send=[_SPH + "RLK 5", _SPH + "RLS 4", _SPH + "RAR 4"])
# Past the last layer the answer is an error code: the walk ends.
_spt("layer-keyframe-past-the-end", "4", "RLK 20", expect_state={}, expect_then_send=[])
_spt("layer-source", "0 Camera%201 12", "RLS 2",
     expect_state={"layers": {"2": {"source": "Camera 1", "source_register": 12}}})
# X80 TR p.33: an empty layer answers the Empty code with no parameters.
_spt("layer-source-empty", "1", "RLS 3",
     state_before={"layers": {"3": {"source": "PC", "source_register": 4, "x": 0.0}}},
     expect_state={"layers": {"3": {"x": 0.0}}})
_spt("layer-aspect-ratio", "0 1.778", "RAR 2", expect_state={"layers": {"2": {"aspect_ratio": 1.778}}})
# RCS answers its layer: the next layer is asked in turn.
_spt("connection-status", "0 2 1 1", "RCS 2",
     expect_state={"layers": {"2": {"connector": "dvi", "connection": "connected"}}},
     expect_then_send=[_SPH + "RCS 3"])
_spt("layer-count", "0 18", "RLC", expect_state={"layer_count": 18})
_spt("basic-presets", "0 2 1 Opening%20Look 2 Keynote", "RBL",
     state_before={"basic_presets": {"9": {"name": "Old"}}},
     expect_state={"basic_preset_count": 2,
                   "basic_presets": {"1": {"name": "Opening Look"}, "2": {"name": "Keynote"}}})
_spt("basic-presets-empty", "0 0", "RBL", state_before={"basic_presets": {"9": {"name": "Old"}}},
     expect_state={"basic_preset_count": 0})
# Command keys: each register's script cue is asked for.
_spt("registers-command-keys", "0 2 1 Look%201 1002 Wide", "RRL 4 -1",
     expect_state={"registers": {"4": {"count": 2, "1": {"name": "Look 1"}, "1002": {"name": "Wide"}}}},
     expect_then_send=[_SPH + "SCR 1 R", _SPH + "SCR 1002 R"])
_spt("registers-sources", "0 1 3 Camera%201", "RRL 6 -1",
     state_before={"registers": {"6": {"count": 4, "9": {"name": "Gone"}}, "5": {"count": 1}}},
     expect_state={"registers": {"6": {"count": 1, "3": {"name": "Camera 1"}}, "5": {"count": 1}}})
_spt("register-cue", "0 3", "SCR 1002 R", expect_state={"registers": {"4": {"1002": {"cue": 3}}}})
_spt("pixelspaces", "0 2 0 Main bg.png next.png 0 0 1920 1080 1 1 Side bg2.png bg3.png 1920 0 1280 720 1", "RPD",
     expect_state={"pixelspace_count": 2, "pixelspaces": {
         "0": {"name": "Main", "current_background": "bg.png", "next_background": "next.png",
               "x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0, "renewal_group": 1},
         "1": {"name": "Side", "current_background": "bg2.png", "next_background": "bg3.png",
               "x": 1920.0, "y": 0.0, "width": 1280.0, "height": 720.0, "renewal_group": 1}}})
_spt("source-names", "0 Camera%201 PC", "RSN", state_before={"source_names": {"2": "Old"}},
     expect_state={"source_names": {"0": "Camera 1", "1": "PC"}})
# X80 TR p.34: idle answers 0 with an empty message, or 101 and Ready.
_spt("io-status", "0 42 Loading%20still", "RPS", expect_state={"io": {"progress": 42, "status": "Loading still"}})
_spt("io-status-idle", "0 101 Ready", "RPS", expect_state={"io": {"progress": 101, "status": "Ready"}})
_spt("router-crosspoints", "0 0 0:3 1:-1", "QRC 0",
     expect_state={"routers": {"0": {"outputs": {"0": {"input": 3}, "1": {"input": -1}}}}})
# Writes are read back: layer changes walk the layers again, others re-read
# their own list.
_spt("reread-after-source-apply", "0", "SRA Camera%201 2 3", expect_state={},
     expect_then_send=[_SPH + "RLK 2"])
_spt("reread-after-preset-learn", "0", "BPL 7", expect_state={}, expect_then_send=[_SPH + "RBL"])
_spt("reread-after-command-key-learn", "0 12 10", "LCK 0 Look%201 4 3 0", expect_state={},
     expect_then_send=[_SPH + "RRL 4 -1"])
_spt("reread-after-cue-recall", "0", "RSC 10 1 S", expect_state={},
     expect_then_send=[_SPH + "RLK 2", _SPH + "RRL 4 -1"])
_spt("reread-after-treatment-learn", "0", "KTL -1 2", expect_state={}, expect_then_send=[_SPH + "RRL 5 -1"])
_spt("reread-after-background-load", "0", "BLD bg.png 0 0", expect_state={},
     expect_then_send=[_SPH + "RPD", _SPH + "RPS"])
_spt("reread-after-router-switch", "0", "RCR 2 L 3 7", expect_state={}, expect_then_send=[_SPH + "QRC 2"])
# A failed write is not read back.
_spt("no-reread-after-failure", "4", "SRA Nope 2", expect_state={}, expect_then_send=[])
