YTF = "yamaha-tf"
# Yamaha TF over RCP (TCP 49280, LF-terminated). Wire formats from Yamaha's QLab Setup Guide for
# CL/QL/TF and its Python Script Template V1.00 (command_list.pdf, recall_a.py, TFxQLab/MultiCh.py,
# TFxQLab/recallb1.py); replies from the RCP grammar of Yamaha's DME7 specification.


def _ytf_vectors():
    S = YTF
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 40), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 40), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 40), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('stereo_input_fader_level', 'MIXER:Current/StInCh/Fader/Level', ('stereo_input', 4), None, ('level', 1000), 'stereo_inputs.{x}.fader_level', False),
        ('stereo_input_on', 'MIXER:Current/StInCh/Fader/On', ('stereo_input', 4), None, ('bool',), 'stereo_inputs.{x}.on', False),
        ('stereo_input_stereo_pan', 'MIXER:Current/StInCh/ToSt/Pan', ('stereo_input', 4), None, ('pan',), 'stereo_inputs.{x}.stereo_pan', False),
        ('fx_return_fader_level', 'MIXER:Current/FxRtnCh/Fader/Level', ('fx_return', 4), None, ('level', 1000), 'fx_returns.{x}.fader_level', False),
        ('fx_return_on', 'MIXER:Current/FxRtnCh/Fader/On', ('fx_return', 4), None, ('bool',), 'fx_returns.{x}.on', False),
        ('fx_return_stereo_pan', 'MIXER:Current/FxRtnCh/ToSt/Pan', ('fx_return', 4), None, ('pan',), 'fx_returns.{x}.stereo_pan', False),
        ('dca_fader_level', 'MIXER:Current/DCA/Fader/Level', ('dca', 8), None, ('level', 1000), 'dcas.{x}.fader_level', False),
        ('dca_on', 'MIXER:Current/DCA/Fader/On', ('dca', 8), None, ('bool',), 'dcas.{x}.on', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 20), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 20), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 20), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'mixes.{x}.balance', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 4), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 4), None, ('bool',), 'matrices.{x}.on', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 2), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 2), None, ('bool',), 'stereo.{x}.on', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 2), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'stereo.{x}.balance', False),
        ('mix_pan_link', 'MIXER:Current/Mix/PanLink', ('mix', 20), None, ('bool',), 'mixes.{x}.pan_link', False),
        ('mono_fader_level', 'MIXER:Current/Mono/Fader/Level', None, None, ('level', 1000), 'mono.fader_level', False),
        ('mono_on', 'MIXER:Current/Mono/Fader/On', None, None, ('bool',), 'mono.on', False),
        ('mute_group_on', 'MIXER:Current/MuteMaster/On', ('mute_group', 6), None, ('bool',), 'mute_groups.{x}.on', False),
        ('input_name', 'MIXER:Current/InCh/Label/Name', ('channel', 40), None, ('name', 64), 'inputs.{x}.name', False),
        ('input_color', 'MIXER:Current/InCh/Label/Color', ('channel', 40), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'inputs.{x}.color', False),
        ('input_icon', 'MIXER:Current/InCh/Label/Icon', ('channel', 40), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'inputs.{x}.icon', False),
        ('input_category', 'MIXER:Current/InCh/Label/Category', ('channel', 40), None, ('text',), 'inputs.{x}.category', False),
        ('stereo_input_name', 'MIXER:Current/StInCh/Label/Name', ('stereo_input', 4), None, ('name', 64), 'stereo_inputs.{x}.name', False),
        ('stereo_input_color', 'MIXER:Current/StInCh/Label/Color', ('stereo_input', 4), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'stereo_inputs.{x}.color', False),
        ('stereo_input_icon', 'MIXER:Current/StInCh/Label/Icon', ('stereo_input', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'stereo_inputs.{x}.icon', False),
        ('stereo_input_category', 'MIXER:Current/StInCh/Label/Category', ('stereo_input', 4), None, ('text',), 'stereo_inputs.{x}.category', False),
        ('fx_return_name', 'MIXER:Current/FxRtnCh/Label/Name', ('fx_return', 4), None, ('name', 64), 'fx_returns.{x}.name', False),
        ('fx_return_color', 'MIXER:Current/FxRtnCh/Label/Color', ('fx_return', 4), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'fx_returns.{x}.color', False),
        ('fx_return_icon', 'MIXER:Current/FxRtnCh/Label/Icon', ('fx_return', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'fx_returns.{x}.icon', False),
        ('fx_return_category', 'MIXER:Current/FxRtnCh/Label/Category', ('fx_return', 4), None, ('text',), 'fx_returns.{x}.category', False),
        ('dca_name', 'MIXER:Current/DCA/Label/Name', ('dca', 8), None, ('name', 64), 'dcas.{x}.name', False),
        ('dca_color', 'MIXER:Current/DCA/Label/Color', ('dca', 8), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'dcas.{x}.color', False),
        ('dca_icon', 'MIXER:Current/DCA/Label/Icon', ('dca', 8), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'dcas.{x}.icon', False),
        ('dca_category', 'MIXER:Current/DCA/Label/Category', ('dca', 8), None, ('text',), 'dcas.{x}.category', False),
        ('mix_name', 'MIXER:Current/Mix/Label/Name', ('mix', 20), None, ('name', 64), 'mixes.{x}.name', False),
        ('mix_color', 'MIXER:Current/Mix/Label/Color', ('mix', 20), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'mixes.{x}.color', False),
        ('mix_icon', 'MIXER:Current/Mix/Label/Icon', ('mix', 20), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'mixes.{x}.icon', False),
        ('mix_category', 'MIXER:Current/Mix/Label/Category', ('mix', 20), None, ('text',), 'mixes.{x}.category', False),
        ('matrix_name', 'MIXER:Current/Mtrx/Label/Name', ('matrix', 4), None, ('name', 64), 'matrices.{x}.name', False),
        ('matrix_color', 'MIXER:Current/Mtrx/Label/Color', ('matrix', 4), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'matrices.{x}.color', False),
        ('matrix_icon', 'MIXER:Current/Mtrx/Label/Icon', ('matrix', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'matrices.{x}.icon', False),
        ('matrix_category', 'MIXER:Current/Mtrx/Label/Category', ('matrix', 4), None, ('text',), 'matrices.{x}.category', False),
        ('stereo_name', 'MIXER:Current/St/Label/Name', ('stereo', 2), None, ('name', 64), 'stereo.{x}.name', False),
        ('stereo_color', 'MIXER:Current/St/Label/Color', ('stereo', 2), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'stereo.{x}.color', False),
        ('stereo_icon', 'MIXER:Current/St/Label/Icon', ('stereo', 2), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'stereo.{x}.icon', False),
        ('stereo_category', 'MIXER:Current/St/Label/Category', ('stereo', 2), None, ('text',), 'stereo.{x}.category', False),
        ('mono_name', 'MIXER:Current/Mono/Label/Name', None, None, ('name', 64), 'mono.name', False),
        ('mono_color', 'MIXER:Current/Mono/Label/Color', None, None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'Off')), 'mono.color', False),
        ('mono_icon', 'MIXER:Current/Mono/Label/Icon', None, None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'mono.icon', False),
        ('mono_category', 'MIXER:Current/Mono/Label/Category', None, None, ('text',), 'mono.category', False),
        ('mute_group_name', 'MIXER:Current/MuteMaster/Label/Name', ('mute_group', 6), None, ('name', 8), 'mute_groups.{x}.name', False),
        ('input_mix_send_level', 'MIXER:Current/InCh/ToMix/Level', ('channel', 40), ('mix', 20), ('level', 1000), 'inputs.{x}.mix_sends.{y}.level', False),
        ('input_mix_send_on', 'MIXER:Current/InCh/ToMix/On', ('channel', 40), ('mix', 20), ('bool',), 'inputs.{x}.mix_sends.{y}.on', False),
        ('input_mix_send_pan', 'MIXER:Current/InCh/ToMix/Pan', ('channel', 40), ('mix', 20), ('pan',), 'inputs.{x}.mix_sends.{y}.pan', False),
        ('input_mix_send_pre', 'MIXER:Current/InCh/ToMix/PrePost', ('channel', 40), ('mix', 20), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'inputs.{x}.mix_sends.{y}.pre', False),
        ('input_fx_send_level', 'MIXER:Current/InCh/ToFx/Level', ('channel', 40), ('fx', 2), ('level', 1000), 'inputs.{x}.fx_sends.{y}.level', False),
        ('input_fx_send_on', 'MIXER:Current/InCh/ToFx/On', ('channel', 40), ('fx', 2), ('bool',), 'inputs.{x}.fx_sends.{y}.on', False),
        ('input_fx_send_pre', 'MIXER:Current/InCh/ToFx/PrePost', ('channel', 40), ('fx', 2), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'inputs.{x}.fx_sends.{y}.pre', False),
        ('input_mono_send_level', 'MIXER:Current/InCh/ToMono/Level', ('channel', 40), None, ('level', 1000), 'inputs.{x}.mono_send.level', False),
        ('input_mono_send_on', 'MIXER:Current/InCh/ToMono/On', ('channel', 40), None, ('bool',), 'inputs.{x}.mono_send.on', False),
        ('stereo_input_mix_send_level', 'MIXER:Current/StInCh/ToMix/Level', ('stereo_input', 4), ('mix', 20), ('level', 1000), 'stereo_inputs.{x}.mix_sends.{y}.level', False),
        ('stereo_input_mix_send_on', 'MIXER:Current/StInCh/ToMix/On', ('stereo_input', 4), ('mix', 20), ('bool',), 'stereo_inputs.{x}.mix_sends.{y}.on', False),
        ('stereo_input_mix_send_pan', 'MIXER:Current/StInCh/ToMix/Pan', ('stereo_input', 4), ('mix', 20), ('pan',), 'stereo_inputs.{x}.mix_sends.{y}.pan', False),
        ('stereo_input_mix_send_pre', 'MIXER:Current/StInCh/ToMix/PrePost', ('stereo_input', 4), ('mix', 20), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'stereo_inputs.{x}.mix_sends.{y}.pre', False),
        ('stereo_input_fx_send_level', 'MIXER:Current/StInCh/ToFx/Level', ('stereo_input', 4), ('fx', 2), ('level', 1000), 'stereo_inputs.{x}.fx_sends.{y}.level', False),
        ('stereo_input_fx_send_on', 'MIXER:Current/StInCh/ToFx/On', ('stereo_input', 4), ('fx', 2), ('bool',), 'stereo_inputs.{x}.fx_sends.{y}.on', False),
        ('stereo_input_fx_send_pre', 'MIXER:Current/StInCh/ToFx/PrePost', ('stereo_input', 4), ('fx', 2), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'stereo_inputs.{x}.fx_sends.{y}.pre', False),
        ('stereo_input_mono_send_level', 'MIXER:Current/StInCh/ToMono/Level', ('stereo_input', 4), None, ('level', 1000), 'stereo_inputs.{x}.mono_send.level', False),
        ('stereo_input_mono_send_on', 'MIXER:Current/StInCh/ToMono/On', ('stereo_input', 4), None, ('bool',), 'stereo_inputs.{x}.mono_send.on', False),
        ('fx_return_mix_send_level', 'MIXER:Current/FxRtnCh/ToMix/Level', ('fx_return', 4), ('mix', 20), ('level', 1000), 'fx_returns.{x}.mix_sends.{y}.level', False),
        ('fx_return_mix_send_on', 'MIXER:Current/FxRtnCh/ToMix/On', ('fx_return', 4), ('mix', 20), ('bool',), 'fx_returns.{x}.mix_sends.{y}.on', False),
        ('fx_return_mix_send_pan', 'MIXER:Current/FxRtnCh/ToMix/Pan', ('fx_return', 4), ('mix', 20), ('pan',), 'fx_returns.{x}.mix_sends.{y}.pan', False),
        ('fx_return_mix_send_pre', 'MIXER:Current/FxRtnCh/ToMix/PrePost', ('fx_return', 4), ('mix', 20), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'fx_returns.{x}.mix_sends.{y}.pre', False),
        ('fx_return_mono_send_level', 'MIXER:Current/FxRtnCh/ToMono/Level', ('fx_return', 4), None, ('level', 1000), 'fx_returns.{x}.mono_send.level', False),
        ('fx_return_mono_send_on', 'MIXER:Current/FxRtnCh/ToMono/On', ('fx_return', 4), None, ('bool',), 'fx_returns.{x}.mono_send.on', False),
        ('mix_matrix_send_level', 'MIXER:Current/Mix/ToMtrx/Level', ('mix', 20), ('matrix', 4), ('level', 1000), 'mixes.{x}.matrix_sends.{y}.level', False),
        ('mix_matrix_send_on', 'MIXER:Current/Mix/ToMtrx/On', ('mix', 20), ('matrix', 4), ('bool',), 'mixes.{x}.matrix_sends.{y}.on', False),
        ('stereo_matrix_send_level', 'MIXER:Current/St/ToMtrx/Level', ('stereo', 2), ('matrix', 4), ('level', 1000), 'stereo.{x}.matrix_sends.{y}.level', False),
        ('stereo_matrix_send_on', 'MIXER:Current/St/ToMtrx/On', ('stereo', 2), ('matrix', 4), ('bool',), 'stereo.{x}.matrix_sends.{y}.on', False),
        ('mono_matrix_send_level', 'MIXER:Current/Mono/ToMtrx/Level', None, ('matrix', 4), ('level', 1000), 'mono.matrix_sends.{y}.level', False),
        ('mono_matrix_send_on', 'MIXER:Current/Mono/ToMtrx/On', None, ('matrix', 4), ('bool',), 'mono.matrix_sends.{y}.on', False),
    ]

    # Expected wire stated independently of the spec's templates: X and Y go
    # on the wire numbered from 0, a parameter without X or Y sends 0, strings
    # are double-quoted with \\ and \" escapes, every line ends with LF.
    def sample(kind):
        """(input value, wire text, state value)"""
        k = kind[0]
        if k == "level":
            return -1000, "-1000", -1000
        if k in ("bool", "flag"):
            return True, "1", True
        if k == "int":
            return kind[3], str(kind[3]), kind[3]
        if k == "pan":
            return -35, "-35", -35
        if k == "enum":
            v = kind[1][-1]
            return v, '"' + v + '"', v
        if k == "name":
            return 'Lead "V"', '"Lead \\"V\\""', None
        return "MIX1", '"MIX1"', "MIX1"

    def pname(kind):
        return {"level": "level", "bool": "enabled", "pan": "pan", "enum": "value", "name": "name",
                "text": "value"}.get(kind[0]) or kind[1]

    def nest(path, value):
        out = cur = {}
        parts = path.split(".")
        for p in parts[:-1]:
            cur[p] = {}
            cur = cur[p]
        cur[parts[-1]] = value
        return out

    done = set(_ytf_EXPLICIT)
    for base, addr, x, y, kind, state, set_only in rows:
        inp, xw, yw, xn, yn = {}, 0, 0, None, None
        if x:
            xn = min(3, x[1]); inp[x[0]] = xn; xw = xn - 1
        if y:
            yn = min(2, y[1]); inp[y[0]] = yn; yw = yn - 1
        val, wire, sval = sample(kind)
        if "set_" + base not in done:
            text(S, "set_" + base, dict(inp, **{pname(kind): val}), f"set {addr} {xw} {yw} {wire}\n",
                 device_reply=f"OK set {addr} {xw} {yw} {wire}\n", expect_result={"ok": {"kind": "ack"}})
        if not set_only and "get_" + base not in done:
            plain = wire[1:-1] if wire.startswith('"') else wire
            text(S, "get_" + base, inp, f"get {addr} {xw} {yw}\n",
                 device_reply=f"OK get {addr} {xw} {yw} {wire}\n",
                 expect_result={"ok": {"kind": "value", "value": plain}})
        if state:
            if kind[0] == "name":
                wire, sval = '"Vox 1"', "Vox 1"
            path = state.replace("{x}", str(xn)).replace("{y}", str(yn))
            tail = "" if wire.startswith('"') else f' "{wire}"'
            telemetry(S, base, inbound=f"NOTIFY set {addr} {xw} {yw} {wire}{tail}\n", expect_state=nest(path, sval))
    for args, kw in _ytf_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_ytf_EXPLICIT = {}


def _ytf_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _ytf_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (MultiCh.py, recalla0.py, recallb1.py, Readme_EN.pdf).
_ytf_explicit("text", "set_input_fader_level", {"channel": 5, "level": 500}, "set MIXER:Current/InCh/Fader/Level 4 0 500\n",
              device_reply="OK set MIXER:Current/InCh/Fader/Level 4 0 500 \"5.00\"\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "set_input_on", {"channel": 2, "enabled": False}, "set MIXER:Current/InCh/Fader/On 1 0 0\n",
              device_reply="OK set MIXER:Current/InCh/Fader/On 1 0 0\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "recall_scene", {"bank": "a", "scene": 0}, "ssrecall_ex scene_a 0\n",
              device_reply="OK ssrecall_ex scene_a 0\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "get_current_scene", {"bank": "b"}, "sscurrent_ex scene_b\n",
              device_reply="ERROR sscurrent_ex InvalidArgument\n", expect_result={"error": {"error": "device_error"}})
_ytf_explicit("telemetry", "scene-current", inbound="NOTIFY sscurrent_ex scene_b 1\n", expect_state={"scenes": {"b": {"current": 1}}})
_ytf_explicit("telemetry", "scene-modified", inbound="OK sscurrent_ex scene_a 3 unmodified\n",
              expect_state={"scenes": {"a": {"current": 3, "modified": False}}})
_ytf_explicit("telemetry", "scene-recalled", inbound="OK ssrecall_ex scene_a 5\n", expect_state={"scenes": {"a": {"current": 5}}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_ytf_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_ytf_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_ytf_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "TF5"\n',
     expect_result={"ok": {"kind": "value", "value": "TF5"}})
_ytf_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_ytf_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_ytf_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "TF5"\n', expect_state={"device": {"product_name": "TF5"}})
_ytf_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_ytf_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_ytf_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})


_ytf_vectors()
_ytf_EXPLICIT.clear()
