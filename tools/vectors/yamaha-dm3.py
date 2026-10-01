YDM3 = "yamaha-dm3"
# Yamaha DM3 over RCP (TCP 49280, LF-terminated). Addresses and ranges from Yamaha's DM3 Series OSC
# Specifications V1.0.0 (p.3-11); the RCP line syntax and replies from Yamaha's DME7 RCP specification
# V1.1.0. The OSC examples number X and Y from 1; on the RCP wire they start at 0.


def _ydm3_vectors():
    S = YDM3
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 16), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 16), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 16), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('input_stereo_on', 'MIXER:Current/InCh/ToSt/On', ('channel', 16), ('stereo_bus', 1), ('bool',), 'inputs.{x}.stereo_sends.{y}.on', False),
        ('input_name', 'MIXER:Current/InCh/Label/Name', ('channel', 16), None, ('name', 8), 'inputs.{x}.name', False),
        ('input_color', 'MIXER:Current/InCh/Label/Color', ('channel', 16), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'inputs.{x}.color', False),
        ('input_icon', 'MIXER:Current/InCh/Label/Icon', ('channel', 16), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'inputs.{x}.icon', False),
        ('input_category', 'MIXER:Current/InCh/Label/Category', ('channel', 16), None, ('enum', ('----', 'Vocal', 'Drums', 'Guitars', 'Keys', 'Horns', 'Strings', 'Spoken', 'FX RTN', 'Others')), 'inputs.{x}.category', False),
        ('input_mix_send_level', 'MIXER:Current/InCh/ToMix/Level', ('channel', 16), ('mix', 6), ('level', 1000), 'inputs.{x}.mix_sends.{y}.level', False),
        ('input_mix_send_on', 'MIXER:Current/InCh/ToMix/On', ('channel', 16), ('mix', 6), ('bool',), 'inputs.{x}.mix_sends.{y}.on', False),
        ('input_mix_send_pan', 'MIXER:Current/InCh/ToMix/Pan', ('channel', 16), ('mix', 6), ('pan',), 'inputs.{x}.mix_sends.{y}.pan', False),
        ('input_mix_send_pre', 'MIXER:Current/InCh/ToMix/PrePost', ('channel', 16), ('mix', 6), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'inputs.{x}.mix_sends.{y}.pre', False),
        ('input_fx_send_level', 'MIXER:Current/InCh/ToFx/Level', ('channel', 16), ('fx', 2), ('level', 1000), 'inputs.{x}.fx_sends.{y}.level', False),
        ('input_fx_send_on', 'MIXER:Current/InCh/ToFx/On', ('channel', 16), ('fx', 2), ('bool',), 'inputs.{x}.fx_sends.{y}.on', False),
        ('input_fx_send_pre', 'MIXER:Current/InCh/ToFx/PrePost', ('channel', 16), ('fx', 2), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'inputs.{x}.fx_sends.{y}.pre', False),
        ('input_matrix_send_level', 'MIXER:Current/InCh/ToMtrx/Level', ('channel', 16), ('matrix', 2), ('level', 1000), 'inputs.{x}.matrix_sends.{y}.level', False),
        ('input_matrix_send_on', 'MIXER:Current/InCh/ToMtrx/On', ('channel', 16), ('matrix', 2), ('bool',), 'inputs.{x}.matrix_sends.{y}.on', False),
        ('input_matrix_send_pan', 'MIXER:Current/InCh/ToMtrx/Pan', ('channel', 16), ('matrix', 2), ('pan',), 'inputs.{x}.matrix_sends.{y}.pan', False),
        ('input_matrix_send_pre', 'MIXER:Current/InCh/ToMtrx/PrePost', ('channel', 16), ('matrix', 2), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'inputs.{x}.matrix_sends.{y}.pre', False),
        ('input_link_group', 'MIXER:Current/InputChLink/InCh/Assign', ('channel', 16), None, ('int', 'group', 0, 9, '0 none, 1 A ... 9 I'), 'inputs.{x}.link_group', False),
        ('stereo_input_fader_level', 'MIXER:Current/StInCh/Fader/Level', ('stereo_input', 2), None, ('level', 1000), 'stereo_inputs.{x}.fader_level', False),
        ('stereo_input_on', 'MIXER:Current/StInCh/Fader/On', ('stereo_input', 2), None, ('bool',), 'stereo_inputs.{x}.on', False),
        ('stereo_input_stereo_pan', 'MIXER:Current/StInCh/ToSt/Pan', ('stereo_input', 2), None, ('pan',), 'stereo_inputs.{x}.stereo_pan', False),
        ('stereo_input_stereo_on', 'MIXER:Current/StInCh/ToSt/On', ('stereo_input', 2), ('stereo_bus', 1), ('bool',), 'stereo_inputs.{x}.stereo_sends.{y}.on', False),
        ('stereo_input_name', 'MIXER:Current/StInCh/Label/Name', ('stereo_input', 2), None, ('name', 8), 'stereo_inputs.{x}.name', False),
        ('stereo_input_color', 'MIXER:Current/StInCh/Label/Color', ('stereo_input', 2), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'stereo_inputs.{x}.color', False),
        ('stereo_input_icon', 'MIXER:Current/StInCh/Label/Icon', ('stereo_input', 2), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'stereo_inputs.{x}.icon', False),
        ('stereo_input_category', 'MIXER:Current/StInCh/Label/Category', ('stereo_input', 2), None, ('enum', ('----', 'Vocal', 'Drums', 'Guitars', 'Keys', 'Horns', 'Strings', 'Spoken', 'FX RTN', 'Others')), 'stereo_inputs.{x}.category', False),
        ('stereo_input_link_group', 'MIXER:Current/InputChLink/StInCh/Assign', ('stereo_input', 2), None, ('int', 'group', 0, 9, '0 none, 1 A ... 9 I'), 'stereo_inputs.{x}.link_group', False),
        ('fx_return_fader_level', 'MIXER:Current/FxRtnCh/Fader/Level', ('fx_return', 4), None, ('level', 1000), 'fx_returns.{x}.fader_level', False),
        ('fx_return_on', 'MIXER:Current/FxRtnCh/Fader/On', ('fx_return', 4), None, ('bool',), 'fx_returns.{x}.on', False),
        ('fx_return_stereo_pan', 'MIXER:Current/FxRtnCh/ToSt/Pan', ('fx_return', 4), None, ('pan',), 'fx_returns.{x}.stereo_pan', False),
        ('fx_return_stereo_on', 'MIXER:Current/FxRtnCh/ToSt/On', ('fx_return', 4), ('stereo_bus', 1), ('bool',), 'fx_returns.{x}.stereo_sends.{y}.on', False),
        ('fx_return_name', 'MIXER:Current/FxRtnCh/Label/Name', ('fx_return', 4), None, ('name', 8), 'fx_returns.{x}.name', False),
        ('fx_return_color', 'MIXER:Current/FxRtnCh/Label/Color', ('fx_return', 4), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'fx_returns.{x}.color', False),
        ('fx_return_icon', 'MIXER:Current/FxRtnCh/Label/Icon', ('fx_return', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'fx_returns.{x}.icon', False),
        ('fx_return_category', 'MIXER:Current/FxRtnCh/Label/Category', ('fx_return', 4), None, ('enum', ('FX RTN',)), 'fx_returns.{x}.category', False),
        ('fx_return_link_group', 'MIXER:Current/InputChLink/FxRtnCh/Assign', ('fx_return', 4), None, ('int', 'group', 0, 9, '0 none, 1 A ... 9 I'), 'fx_returns.{x}.link_group', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 6), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 6), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_pan_link', 'MIXER:Current/Mix/PanLink', ('mix', 6), None, ('bool',), 'mixes.{x}.pan_link', False),
        ('mix_stereo_pan', 'MIXER:Current/Mix/ToSt/Pan', ('mix', 6), None, ('pan',), 'mixes.{x}.stereo_pan', False),
        ('mix_stereo_on', 'MIXER:Current/Mix/ToSt/On', ('mix', 6), ('stereo_bus', 1), ('bool',), 'mixes.{x}.stereo_sends.{y}.on', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 6), None, ('pan',), 'mixes.{x}.balance', False),
        ('mix_name', 'MIXER:Current/Mix/Label/Name', ('mix', 6), None, ('name', 8), 'mixes.{x}.name', False),
        ('mix_color', 'MIXER:Current/Mix/Label/Color', ('mix', 6), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'mixes.{x}.color', False),
        ('mix_icon', 'MIXER:Current/Mix/Label/Icon', ('mix', 6), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'mixes.{x}.icon', False),
        ('mix_category', 'MIXER:Current/Mix/Label/Category', ('mix', 6), None, ('enum', ('----', 'FX', 'Output', 'Others')), 'mixes.{x}.category', False),
        ('mix_matrix_send_level', 'MIXER:Current/Mix/ToMtrx/Level', ('mix', 6), ('matrix', 2), ('level', 1000), 'mixes.{x}.matrix_sends.{y}.level', False),
        ('mix_matrix_send_on', 'MIXER:Current/Mix/ToMtrx/On', ('mix', 6), ('matrix', 2), ('bool',), 'mixes.{x}.matrix_sends.{y}.on', False),
        ('mix_matrix_send_pan', 'MIXER:Current/Mix/ToMtrx/Pan', ('mix', 6), ('matrix', 2), ('pan',), 'mixes.{x}.matrix_sends.{y}.pan', False),
        ('mix_matrix_send_pre', 'MIXER:Current/Mix/ToMtrx/PrePost', ('mix', 6), ('matrix', 2), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'mixes.{x}.matrix_sends.{y}.pre', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 2), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 2), None, ('bool',), 'stereo.{x}.on', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 2), None, ('pan',), 'stereo.{x}.balance', False),
        ('stereo_name', 'MIXER:Current/St/Label/Name', ('stereo', 2), None, ('name', 8), 'stereo.{x}.name', False),
        ('stereo_color', 'MIXER:Current/St/Label/Color', ('stereo', 2), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'stereo.{x}.color', False),
        ('stereo_icon', 'MIXER:Current/St/Label/Icon', ('stereo', 2), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'stereo.{x}.icon', False),
        ('stereo_category', 'MIXER:Current/St/Label/Category', ('stereo', 2), None, ('enum', ('----', 'FX', 'Output', 'Others')), 'stereo.{x}.category', False),
        ('stereo_matrix_send_level', 'MIXER:Current/St/ToMtrx/Level', ('stereo', 2), ('matrix', 2), ('level', 1000), 'stereo.{x}.matrix_sends.{y}.level', False),
        ('stereo_matrix_send_on', 'MIXER:Current/St/ToMtrx/On', ('stereo', 2), ('matrix', 2), ('bool',), 'stereo.{x}.matrix_sends.{y}.on', False),
        ('stereo_matrix_send_pan', 'MIXER:Current/St/ToMtrx/Pan', ('stereo', 2), ('matrix', 2), ('pan',), 'stereo.{x}.matrix_sends.{y}.pan', False),
        ('stereo_matrix_send_pre', 'MIXER:Current/St/ToMtrx/PrePost', ('stereo', 2), ('matrix', 2), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'stereo.{x}.matrix_sends.{y}.pre', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 2), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 2), None, ('bool',), 'matrices.{x}.on', False),
        ('matrix_name', 'MIXER:Current/Mtrx/Label/Name', ('matrix', 2), None, ('name', 8), 'matrices.{x}.name', False),
        ('matrix_color', 'MIXER:Current/Mtrx/Label/Color', ('matrix', 2), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'matrices.{x}.color', False),
        ('matrix_icon', 'MIXER:Current/Mtrx/Label/Icon', ('matrix', 2), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'matrices.{x}.icon', False),
        ('matrix_category', 'MIXER:Current/Mtrx/Label/Category', ('matrix', 2), None, ('enum', ('----', 'FX', 'Output', 'Others')), 'matrices.{x}.category', False),
        ('matrix_pan_link', 'MIXER:Current/Mtrx/PanLink', ('matrix', 2), None, ('bool',), 'matrices.{x}.pan_link', False),
        ('matrix_balance', 'MIXER:Current/Mtrx/Out/Balance', ('matrix', 2), None, ('pan',), 'matrices.{x}.balance', False),
        ('fx_fader_level', 'MIXER:Current/Fx/Fader/Level', ('fx', 2), None, ('level', 1000), 'fx.{x}.fader_level', False),
        ('fx_on', 'MIXER:Current/Fx/Fader/On', ('fx', 2), None, ('bool',), 'fx.{x}.on', False),
        ('fx_name', 'MIXER:Current/Fx/Label/Name', ('fx', 2), None, ('name', 8), 'fx.{x}.name', False),
        ('fx_color', 'MIXER:Current/Fx/Label/Color', ('fx', 2), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'fx.{x}.color', False),
        ('fx_icon', 'MIXER:Current/Fx/Label/Icon', ('fx', 2), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'fx.{x}.icon', False),
        ('fx_category', 'MIXER:Current/Fx/Label/Category', ('fx', 2), None, ('enum', ('----', 'FX', 'Output', 'Others')), 'fx.{x}.category', False),
        ('mute_group_on', 'MIXER:Current/MuteGrpCtrl/On', ('mute_group', 6), None, ('bool',), 'mute_groups.{x}.on', False),
        ('mute_group_name', 'MIXER:Current/MuteGrpCtrl/Label/Name', ('mute_group', 6), None, ('name', 8), 'mute_groups.{x}.name', False),
        ('input_link_eq', 'MIXER:Current/InputChLink/LinkParams/EQ', ('link_group', 9), None, ('bool',), 'input_links.{x}.eq', False),
        ('input_link_fader', 'MIXER:Current/InputChLink/LinkParams/Fader', ('link_group', 9), None, ('bool',), 'input_links.{x}.fader', False),
        ('input_link_channel_on', 'MIXER:Current/InputChLink/LinkParams/ChOn', ('link_group', 9), None, ('bool',), 'input_links.{x}.channel_on', False),
        ('input_link_to_stereo', 'MIXER:Current/InputChLink/LinkParams/ToSt', ('link_group', 9), None, ('bool',), 'input_links.{x}.to_stereo', False),
        ('input_link_mute', 'MIXER:Current/InputChLink/LinkParams/Mute', ('link_group', 9), None, ('bool',), 'input_links.{x}.mute', False),
        ('input_link_matrix_send', 'MIXER:Current/InputChLink/LinkParams/MtrxSend', ('link_group', 9), None, ('bool',), 'input_links.{x}.matrix_send', False),
        ('input_link_matrix_send_on', 'MIXER:Current/InputChLink/LinkParams/MtrxSendOn', ('link_group', 9), None, ('bool',), 'input_links.{x}.matrix_send_on', False),
        ('input_link_fx_send', 'MIXER:Current/InputChLink/LinkParams/FxSend', ('link_group', 9), None, ('bool',), 'input_links.{x}.fx_send', False),
        ('input_link_fx_send_on', 'MIXER:Current/InputChLink/LinkParams/FxSendOn', ('link_group', 9), None, ('bool',), 'input_links.{x}.fx_send_on', False),
        ('input_link_ha', 'MIXER:Current/InputChLink/LinkParams/HA', ('link_group', 9), None, ('bool',), 'input_links.{x}.ha', False),
        ('input_link_hpf', 'MIXER:Current/InputChLink/LinkParams/HPF', ('link_group', 9), None, ('bool',), 'input_links.{x}.hpf', False),
        ('input_link_digital_gain', 'MIXER:Current/InputChLink/LinkParams/DigitalGain', ('link_group', 9), None, ('bool',), 'input_links.{x}.digital_gain', False),
        ('input_link_mix_send', 'MIXER:Current/InputChLink/LinkParams/MixSend', ('link_group', 9), None, ('bool',), 'input_links.{x}.mix_send', False),
        ('input_link_mix_send_on', 'MIXER:Current/InputChLink/LinkParams/MixSendOn', ('link_group', 9), None, ('bool',), 'input_links.{x}.mix_send_on', False),
        ('input_link_delay', 'MIXER:Current/InputChLink/LinkParams/Delay', ('link_group', 9), None, ('bool',), 'input_links.{x}.delay', False),
        ('input_link_dynamics1', 'MIXER:Current/InputChLink/LinkParams/Dyna1', ('link_group', 9), None, ('bool',), 'input_links.{x}.dynamics1', False),
        ('input_link_dynamics2', 'MIXER:Current/InputChLink/LinkParams/Dyna2', ('link_group', 9), None, ('bool',), 'input_links.{x}.dynamics2', False),
        ('input_link_send_to_mix', 'MIXER:Current/InputChLink/SendParams/ToMix', ('link_group', 9), ('mix', 6), ('bool',), 'input_links.{x}.send_to_mix.{y}', False),
        ('input_link_send_to_matrix', 'MIXER:Current/InputChLink/SendParams/ToMtrx', ('link_group', 9), ('matrix', 2), ('bool',), 'input_links.{x}.send_to_matrix.{y}', False),
        ('input_link_send_to_fx', 'MIXER:Current/InputChLink/SendParams/ToFx', ('link_group', 9), ('fx', 2), ('bool',), 'input_links.{x}.send_to_fx.{y}', False),
        ('local_input_ha_gain', 'IO:Current/InCh/HAGain', ('input', 16), None, ('int', 'gain', 0, 64, 'dB, 0 to +64 (Table 7)'), 'local_inputs.{x}.ha_gain', False),
        ('local_input_phantom', 'IO:Current/InCh/48VOn', ('input', 16), None, ('bool',), 'local_inputs.{x}.phantom', False),
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

    done = set(_ydm3_EXPLICIT)
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
    for args, kw in _ydm3_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_ydm3_EXPLICIT = {}


def _ydm3_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _ydm3_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (OSC spec 1.4, p.3), restated for the RCP wire.
_ydm3_explicit("text", "set_input_fader_level", {"channel": 1, "level": -32768}, "set MIXER:Current/InCh/Fader/Level 0 0 -32768\n",
               device_reply="OK set MIXER:Current/InCh/Fader/Level 0 0 -32768 \"-Inf\"\n", expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("text", "set_mix_color", {"mix": 1, "value": "Pink"}, 'set MIXER:Current/Mix/Label/Color 0 0 "Pink"\n',
               device_reply='OK set MIXER:Current/Mix/Label/Color 0 0 "Pink"\n', expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("text", "recall_scene", {"bank": "a", "scene": 5}, "ssrecall_ex scene_a 5\n",
               device_reply="OK ssrecall_ex scene_a 5\n", expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("text", "get_current_scene", {"bank": "a"}, "sscurrent_ex scene_a\n",
               device_reply="OK sscurrent_ex scene_a 5 modified\n", expect_result={"ok": {"kind": "value", "value": "5"}})
_ydm3_explicit("text", "set_local_input_ha_gain", {"input": 16, "gain": 64}, "set IO:Current/InCh/HAGain 15 0 64\n",
               device_reply="OK set IO:Current/InCh/HAGain 15 0 64 \"+64\"\n", expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("telemetry", "scene-current", inbound="NOTIFY sscurrent_ex scene_a 5\n", expect_state={"scenes": {"a": {"current": 5}}})
_ydm3_explicit("telemetry", "scene-modified", inbound="OK sscurrent_ex scene_b 2 modified\n",
               expect_state={"scenes": {"b": {"current": 2, "modified": True}}})
_ydm3_explicit("telemetry", "scene-recalled", inbound="OK ssrecall_ex scene_a 9\n", expect_state={"scenes": {"a": {"current": 9}}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_ydm3_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_ydm3_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_ydm3_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "DM3"\n',
     expect_result={"ok": {"kind": "value", "value": "DM3"}})
_ydm3_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_ydm3_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_ydm3_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_ydm3_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "DM3"\n', expect_state={"device": {"product_name": "DM3"}})
_ydm3_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_ydm3_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_ydm3_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})


_ydm3_vectors()
_ydm3_EXPLICIT.clear()
