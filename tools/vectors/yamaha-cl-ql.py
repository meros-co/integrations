YCLQL = "yamaha-cl-ql"
# Yamaha CL/QL over RCP (TCP 49280, LF-terminated). Wire formats from Yamaha's QLab Setup Guide
# for CL/QL/TF and its Python Script Template V1.00 (command_list.pdf, command.py, recall.py);
# replies from the RCP grammar of Yamaha's DME7 specification (see the spec's quirks).


def _yclql_vectors():
    S = YCLQL
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 72), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 72), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 72), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('stereo_input_fader_level', 'MIXER:Current/StInCh/Fader/Level', ('stereo_input', 16), None, ('level', 1000), 'stereo_inputs.{x}.fader_level', False),
        ('stereo_input_on', 'MIXER:Current/StInCh/Fader/On', ('stereo_input', 16), None, ('bool',), 'stereo_inputs.{x}.on', False),
        ('stereo_input_stereo_pan', 'MIXER:Current/StInCh/ToSt/Pan', ('stereo_input', 16), None, ('pan',), 'stereo_inputs.{x}.stereo_pan', False),
        ('stereo_input_balance', 'MIXER:Current/StInCh/Out/Balance', ('stereo_input', 16), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'stereo_inputs.{x}.balance', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 24), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 24), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_stereo_pan', 'MIXER:Current/Mix/ToSt/Pan', ('mix', 24), None, ('pan',), 'mixes.{x}.stereo_pan', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 24), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'mixes.{x}.balance', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 8), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 8), None, ('bool',), 'matrices.{x}.on', False),
        ('matrix_balance', 'MIXER:Current/Mtrx/Out/Balance', ('matrix', 8), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'matrices.{x}.balance', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 3), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 3), None, ('bool',), 'stereo.{x}.on', False),
        ('dca_fader_level', 'MIXER:Current/DCA/Fader/Level', ('dca', 16), None, ('level', 1000), 'dcas.{x}.fader_level', False),
        ('dca_on', 'MIXER:Current/DCA/Fader/On', ('dca', 16), None, ('bool',), 'dcas.{x}.on', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 2), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'stereo.{x}.balance', False),
        ('mute_group_on', 'MIXER:Current/MuteMaster/On', ('mute_group', 8), None, ('bool',), 'mute_groups.{x}.on', False),
        ('input_name', 'MIXER:Current/InCh/Label/Name', ('channel', 72), None, ('name', 8), 'inputs.{x}.name', False),
        ('input_color', 'MIXER:Current/InCh/Label/Color', ('channel', 72), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'inputs.{x}.color', False),
        ('input_icon', 'MIXER:Current/InCh/Label/Icon', ('channel', 72), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'inputs.{x}.icon', False),
        ('stereo_input_name', 'MIXER:Current/StInCh/Label/Name', ('stereo_input', 16), None, ('name', 8), 'stereo_inputs.{x}.name', False),
        ('stereo_input_color', 'MIXER:Current/StInCh/Label/Color', ('stereo_input', 16), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'stereo_inputs.{x}.color', False),
        ('stereo_input_icon', 'MIXER:Current/StInCh/Label/Icon', ('stereo_input', 16), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'stereo_inputs.{x}.icon', False),
        ('mix_name', 'MIXER:Current/Mix/Label/Name', ('mix', 24), None, ('name', 8), 'mixes.{x}.name', False),
        ('mix_color', 'MIXER:Current/Mix/Label/Color', ('mix', 24), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'mixes.{x}.color', False),
        ('mix_icon', 'MIXER:Current/Mix/Label/Icon', ('mix', 24), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'mixes.{x}.icon', False),
        ('matrix_name', 'MIXER:Current/Mtrx/Label/Name', ('matrix', 8), None, ('name', 8), 'matrices.{x}.name', False),
        ('matrix_color', 'MIXER:Current/Mtrx/Label/Color', ('matrix', 8), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'matrices.{x}.color', False),
        ('matrix_icon', 'MIXER:Current/Mtrx/Label/Icon', ('matrix', 8), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'matrices.{x}.icon', False),
        ('stereo_name', 'MIXER:Current/St/Label/Name', ('stereo', 3), None, ('name', 8), 'stereo.{x}.name', False),
        ('stereo_color', 'MIXER:Current/St/Label/Color', ('stereo', 3), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'stereo.{x}.color', False),
        ('stereo_icon', 'MIXER:Current/St/Label/Icon', ('stereo', 3), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'FloorTom', 'Drumkit', 'Perc.', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'WirelessMic', 'SpeechMic', 'Speaker', 'Wedge', 'In-Ear', 'Effect', 'Processor', 'Media1', 'Media2', 'Video', 'Mixer', 'PC', 'Audience', 'Star1', 'Star2', 'Blank')), 'stereo.{x}.icon', False),
        ('dca_name', 'MIXER:Current/DCA/Label/Name', ('dca', 16), None, ('name', 8), 'dcas.{x}.name', False),
        ('dca_color', 'MIXER:Current/DCA/Label/Color', ('dca', 16), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'Cyan', 'Magenta', 'Red', 'Green', 'Off')), 'dcas.{x}.color', False),
        ('mute_group_name', 'MIXER:Current/MuteMaster/Label/Name', ('mute_group', 8), None, ('name', 8), 'mute_groups.{x}.name', False),
        ('input_mix_send_level', 'MIXER:Current/InCh/ToMix/Level', ('channel', 72), ('mix', 24), ('level', 1000), 'inputs.{x}.mix_sends.{y}.level', False),
        ('input_mix_send_on', 'MIXER:Current/InCh/ToMix/On', ('channel', 72), ('mix', 24), ('bool',), 'inputs.{x}.mix_sends.{y}.on', False),
        ('input_mix_send_pan', 'MIXER:Current/InCh/ToMix/Pan', ('channel', 72), ('mix', 24), ('pan',), 'inputs.{x}.mix_sends.{y}.pan', False),
        ('input_mix_send_pre', 'MIXER:Current/InCh/ToMix/PrePost', ('channel', 72), ('mix', 24), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'inputs.{x}.mix_sends.{y}.pre', False),
        ('input_matrix_send_level', 'MIXER:Current/InCh/ToMtrx/Level', ('channel', 72), ('matrix', 8), ('level', 1000), 'inputs.{x}.matrix_sends.{y}.level', False),
        ('input_matrix_send_on', 'MIXER:Current/InCh/ToMtrx/On', ('channel', 72), ('matrix', 8), ('bool',), 'inputs.{x}.matrix_sends.{y}.on', False),
        ('input_matrix_send_pan', 'MIXER:Current/InCh/ToMtrx/Pan', ('channel', 72), ('matrix', 8), ('pan',), 'inputs.{x}.matrix_sends.{y}.pan', False),
        ('input_matrix_send_pre', 'MIXER:Current/InCh/ToMtrx/PrePost', ('channel', 72), ('matrix', 8), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'inputs.{x}.matrix_sends.{y}.pre', False),
        ('stereo_input_mix_send_level', 'MIXER:Current/StInCh/ToMix/Level', ('stereo_input', 16), ('mix', 24), ('level', 1000), 'stereo_inputs.{x}.mix_sends.{y}.level', False),
        ('stereo_input_mix_send_on', 'MIXER:Current/StInCh/ToMix/On', ('stereo_input', 16), ('mix', 24), ('bool',), 'stereo_inputs.{x}.mix_sends.{y}.on', False),
        ('stereo_input_mix_send_pan', 'MIXER:Current/StInCh/ToMix/Pan', ('stereo_input', 16), ('mix', 24), ('pan',), 'stereo_inputs.{x}.mix_sends.{y}.pan', False),
        ('stereo_input_mix_send_pre', 'MIXER:Current/StInCh/ToMix/PrePost', ('stereo_input', 16), ('mix', 24), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'stereo_inputs.{x}.mix_sends.{y}.pre', False),
        ('stereo_input_matrix_send_level', 'MIXER:Current/StInCh/ToMtrx/Level', ('stereo_input', 16), ('matrix', 8), ('level', 1000), 'stereo_inputs.{x}.matrix_sends.{y}.level', False),
        ('stereo_input_matrix_send_on', 'MIXER:Current/StInCh/ToMtrx/On', ('stereo_input', 16), ('matrix', 8), ('bool',), 'stereo_inputs.{x}.matrix_sends.{y}.on', False),
        ('stereo_input_matrix_send_pan', 'MIXER:Current/StInCh/ToMtrx/Pan', ('stereo_input', 16), ('matrix', 8), ('pan',), 'stereo_inputs.{x}.matrix_sends.{y}.pan', False),
        ('stereo_input_matrix_send_pre', 'MIXER:Current/StInCh/ToMtrx/PrePost', ('stereo_input', 16), ('matrix', 8), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'stereo_inputs.{x}.matrix_sends.{y}.pre', False),
        ('mix_matrix_send_level', 'MIXER:Current/Mix/ToMtrx/Level', ('mix', 24), ('matrix', 8), ('level', 1000), 'mixes.{x}.matrix_sends.{y}.level', False),
        ('mix_matrix_send_on', 'MIXER:Current/Mix/ToMtrx/On', ('mix', 24), ('matrix', 8), ('bool',), 'mixes.{x}.matrix_sends.{y}.on', False),
        ('mix_matrix_send_pan', 'MIXER:Current/Mix/ToMtrx/Pan', ('mix', 24), ('matrix', 8), ('pan',), 'mixes.{x}.matrix_sends.{y}.pan', False),
        ('mix_matrix_send_pre', 'MIXER:Current/Mix/ToMtrx/PrePost', ('mix', 24), ('matrix', 8), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'mixes.{x}.matrix_sends.{y}.pre', False),
        ('stereo_matrix_send_level', 'MIXER:Current/St/ToMtrx/Level', ('stereo', 3), ('matrix', 8), ('level', 1000), 'stereo.{x}.matrix_sends.{y}.level', False),
        ('stereo_matrix_send_on', 'MIXER:Current/St/ToMtrx/On', ('stereo', 3), ('matrix', 8), ('bool',), 'stereo.{x}.matrix_sends.{y}.on', False),
        ('stereo_matrix_send_pan', 'MIXER:Current/St/ToMtrx/Pan', ('stereo', 3), ('matrix', 8), ('pan',), 'stereo.{x}.matrix_sends.{y}.pan', False),
        ('stereo_matrix_send_pre', 'MIXER:Current/St/ToMtrx/PrePost', ('stereo', 3), ('matrix', 8), ('flag', 'pre', 'true = 1 (PRE), false = 0 (POST)'), 'stereo.{x}.matrix_sends.{y}.pre', False),
        ('input_ha_gain', 'MIXER:Current/InCh/Port/HA/Gain', ('channel', 72), None, ('int', 'gain', -600, 6600, 'hundredths of a dB, -600 (-6 dB) to 6600 (+66 dB)'), 'inputs.{x}.ha_gain', False),
        ('stereo_input_ha_gain', 'MIXER:Current/StInCh/Port/HA/Gain', ('stereo_input', 16), None, ('int', 'gain', -600, 6600, 'hundredths of a dB, -600 (-6 dB) to 6600 (+66 dB)'), 'stereo_inputs.{x}.ha_gain', False),
        ('input_dynamics1_threshold', 'MIXER:Current/InCh/Dyna1/Threshold', ('channel', 72), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -720 (-72 dB) to 0'), 'inputs.{x}.dynamics.1.threshold', False),
        ('input_dynamics2_threshold', 'MIXER:Current/InCh/Dyna2/Threshold', ('channel', 72), None, ('int', 'threshold', -540, 0, 'tenths of a dB, -540 (-54 dB) to 0'), 'inputs.{x}.dynamics.2.threshold', False),
        ('stereo_input_dynamics1_threshold', 'MIXER:Current/StInCh/Dyna1/Threshold', ('stereo_input', 16), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -720 (-72 dB) to 0'), 'stereo_inputs.{x}.dynamics.1.threshold', False),
        ('stereo_input_dynamics2_threshold', 'MIXER:Current/StInCh/Dyna2/Threshold', ('stereo_input', 16), None, ('int', 'threshold', -540, 0, 'tenths of a dB, -540 (-54 dB) to 0'), 'stereo_inputs.{x}.dynamics.2.threshold', False),
        ('mix_dynamics1_threshold', 'MIXER:Current/Mix/Dyna1/Threshold', ('mix', 24), None, ('int', 'threshold', -540, 0, 'tenths of a dB, -540 (-54 dB) to 0'), 'mixes.{x}.dynamics.1.threshold', False),
        ('matrix_dynamics1_threshold', 'MIXER:Current/Mtrx/Dyna1/Threshold', ('matrix', 8), None, ('int', 'threshold', -540, 0, 'tenths of a dB, -540 (-54 dB) to 0'), 'matrices.{x}.dynamics.1.threshold', False),
        ('stereo_dynamics1_threshold', 'MIXER:Current/St/Dyna1/Threshold', ('stereo', 3), None, ('int', 'threshold', -540, 0, 'tenths of a dB, -540 (-54 dB) to 0'), 'stereo.{x}.dynamics.1.threshold', False),
        ('input_dca_assign', 'MIXER:Current/InCh/DCA/Assign', ('channel', 72), ('dca', 16), ('flag', 'assigned', 'true = 1 (assigned), false = 0 (not assigned)'), 'inputs.{x}.dcas.{y}', False),
        ('stereo_input_dca_assign', 'MIXER:Current/StInCh/DCA/Assign', ('stereo_input', 16), ('dca', 16), ('flag', 'assigned', 'true = 1 (assigned), false = 0 (not assigned)'), 'stereo_inputs.{x}.dcas.{y}', False),
        ('mix_dca_assign', 'MIXER:Current/Mix/DCA/Assign', ('mix', 24), ('dca', 16), ('flag', 'assigned', 'true = 1 (assigned), false = 0 (not assigned)'), 'mixes.{x}.dcas.{y}', False),
        ('matrix_dca_assign', 'MIXER:Current/Mtrx/DCA/Assign', ('matrix', 8), ('dca', 16), ('flag', 'assigned', 'true = 1 (assigned), false = 0 (not assigned)'), 'matrices.{x}.dcas.{y}', False),
        ('stereo_dca_assign', 'MIXER:Current/St/DCA/Assign', ('stereo', 3), ('dca', 16), ('flag', 'assigned', 'true = 1 (assigned), false = 0 (not assigned)'), 'stereo.{x}.dcas.{y}', False),
        ('input_cue', 'MIXER:Current/Cue/InCh/On', ('channel', 72), None, ('bool',), 'inputs.{x}.cue', False),
        ('stereo_input_cue', 'MIXER:Current/Cue/StInCh/On', ('stereo_input', 16), None, ('bool',), 'stereo_inputs.{x}.cue', False),
        ('mix_cue', 'MIXER:Current/Cue/Mix/On', ('mix', 24), None, ('bool',), 'mixes.{x}.cue', False),
        ('matrix_cue', 'MIXER:Current/Cue/Mtrx/On', ('matrix', 8), None, ('bool',), 'matrices.{x}.cue', False),
        ('stereo_cue', 'MIXER:Current/Cue/St/On', ('stereo', 3), None, ('bool',), 'stereo.{x}.cue', False),
        ('dca_cue', 'MIXER:Current/Cue/DCA/On', ('dca', 16), None, ('bool',), 'dcas.{x}.cue', False),
        ('monitor_on', 'MIXER:Current/Monitor/On', None, None, ('bool',), 'monitor.on', False),
        ('monitor_dimmer', 'MIXER:Current/Monitor/DimmerOn', None, None, ('bool',), 'monitor.dimmer', False),
        ('monitor_cue_interruption', 'MIXER:Current/Monitor/CueInterruption', None, None, ('bool',), 'monitor.cue_interruption', False),
        ('monitor_fader_level', 'MIXER:Current/Monitor/Fader/Level', None, None, ('level', 1000), 'monitor.fader_level', False),
        ('cue_output', 'MIXER:Current/Cue/Output', None, None, ('bool',), 'cue.output_on', False),
        ('cue_mode', 'MIXER:Current/Cue/CueMode', None, None, ('text',), 'cue.mode', False),
        ('cue_fader_release', 'MIXER:Current/Cue/FaderCueRelease', None, None, ('bool',), 'cue.fader_cue_release', False),
        ('cue_output_level', 'MIXER:Current/Cue/OutputLevel', None, None, ('level', 1000), 'cue.output_level', False),
        ('cue_active', 'MIXER:Current/Cue/ActiveCue', None, None, ('text',), 'cue.active', 'get'),
        ('input_patch', 'MIXER:Current/InCh/Patch', ('channel', 72), None, ('text',), 'inputs.{x}.patch', 'get'),
        ('stereo_input_patch', 'MIXER:Current/StInCh/Patch', ('stereo_input', 16), None, ('text',), 'stereo_inputs.{x}.patch', 'get'),
        ('dante_output_patch', 'MIXER:Current/DanteOutPort/Patch', ('output', 64), None, ('text',), 'dante_outputs.{x}.patch', False),
        ('omni_output_patch', 'MIXER:Current/OmniOutPort/Patch', ('output', 16), None, ('text',), 'omni_outputs.{x}.patch', False),
        ('digital_output_patch', 'MIXER:Current/DigitalOutPort/Patch', ('output', 2), None, ('text',), 'digital_outputs.{x}.patch', False),
        ('slot1_output_patch', 'MIXER:Current/SlotOut1Port/Patch', None, ('output', 16), ('text',), 'slots.1.outputs.{y}.patch', False),
        ('slot2_output_patch', 'MIXER:Current/SlotOut2Port/Patch', None, ('output', 16), ('text',), 'slots.2.outputs.{y}.patch', False),
        ('slot3_output_patch', 'MIXER:Current/SlotOut3Port/Patch', None, ('output', 16), ('text',), 'slots.3.outputs.{y}.patch', False),
        ('recorder_patch', 'MIXER:Current/Recorder/Patch', None, ('input', 2), ('text',), 'recorder.inputs.{y}.patch', False),
        ('cl_custom_fader_bank_a', 'CL:Current/CustomFaderBank/A/SourceCh', ('layer', 2), ('strip', 16), ('text',), 'fader_banks.custom.a.{x}.{y}', False),
        ('cl_custom_fader_bank_b_centralogic_out', 'CL:Current/CustomFaderBank/B/CentraOut/SourceCh', None, ('strip', 8), ('text',), 'fader_banks.custom.b_out.{y}', False),
        ('cl_custom_fader_bank_c', 'CL:Current/CustomFaderBank/C/SourceCh', ('layer', 6), ('strip', 8), ('text',), 'fader_banks.custom.c.{x}.{y}', False),
        ('cl_custom_fader_bank_master', 'CL:Current/CustomFaderBank/Master/SourceCh', None, ('strip', 2), ('text',), 'fader_banks.custom.master.{y}', False),
        ('cl_fader_bank_a', 'CL:Current/FaderBank/A/Recall', None, None, ('int', 'bank', 0, 8, "0 to 8, the bank as the console's parameter list numbers it"), 'fader_banks.a', False),
        ('cl_fader_bank_b_centralogic_select', 'CL:Current/FaderBank/B/Centralogic/Select', None, None, ('int', 'selection', 0, 1, "0 or 1, as the console's parameter list numbers it"), 'fader_banks.b_centralogic', False),
        ('cl_fader_bank_b_centralogic_in', 'CL:Current/FaderBank/B/CentraIn/Recall', None, None, ('int', 'bank', 0, 8, "0 to 8, the bank as the console's parameter list numbers it"), 'fader_banks.b_in', False),
        ('cl_fader_bank_b_centralogic_out', 'CL:Current/FaderBank/B/CentraOut/Recall', None, None, ('int', 'bank', 0, 8, "0 to 8, the bank as the console's parameter list numbers it"), 'fader_banks.b_out', False),
        ('cl_fader_bank_c', 'CL:Current/FaderBank/C/Recall', None, None, ('int', 'bank', 0, 8, "0 to 8, the bank as the console's parameter list numbers it"), 'fader_banks.c', False),
        ('ql_custom_fader_bank', 'QL:Current/CustomFaderBank/SourceCh', ('layer', 4), ('strip', 32), ('text',), 'fader_banks.custom.layers.{x}.{y}', False),
        ('ql_custom_fader_bank_master', 'QL:Current/CustomFaderBank/Master/SourceCh', ('layer', 4), ('strip', 2), ('text',), 'fader_banks.custom.masters.{x}.{y}', False),
        ('ql_fader_bank_select', 'QL:Current/FaderBank/Select', None, None, ('int', 'selection', 0, 1, "0 or 1, as the console's parameter list numbers it"), 'fader_banks.select', False),
        ('ql_fader_bank_recall', 'QL:Current/FaderBank/Bank/Recall', None, ('section', 3), ('int', 'bank', 0, 8, "0 to 8, the bank as the console's parameter list numbers it"), 'fader_banks.sections.{y}', False),
        ('ql_fader_bank_toggle', 'QL:Current/FaderBank/Bank/Toggle', None, None, ('int', 'bank', 0, 8, "0 to 8, as the console's parameter list numbers it"), 'fader_banks.toggle', False),
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

    done = set(_yclql_EXPLICIT)
    for base, addr, x, y, kind, state, set_only in rows:
        inp, xw, yw, xn, yn = {}, 0, 0, None, None
        if x:
            xn = min(3, x[1]); inp[x[0]] = xn; xw = xn - 1
        if y:
            yn = min(2, y[1]); inp[y[0]] = yn; yw = yn - 1
        val, wire, sval = sample(kind)
        if set_only != "get" and "set_" + base not in done:
            text(S, "set_" + base, dict(inp, **{pname(kind): val}), f"set {addr} {xw} {yw} {wire}\n",
                 device_reply=f"OK set {addr} {xw} {yw} {wire}\n", expect_result={"ok": {"kind": "ack"}})
        if set_only is not True and "get_" + base not in done:
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
    for args, kw in _yclql_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_yclql_EXPLICIT = {}


def _yclql_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _yclql_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (QLab guide p.12-15, recall.py, command_list.pdf).
_yclql_explicit("text", "set_input_on", {"channel": 1, "enabled": True}, "set MIXER:Current/InCh/Fader/On 0 0 1\n",
                device_reply="OK set MIXER:Current/InCh/Fader/On 0 0 1 \"ON\"\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_input_stereo_pan", {"channel": 1, "pan": -63}, "set MIXER:Current/InCh/ToSt/Pan 0 0 -63\n",
                device_reply="OK set MIXER:Current/InCh/ToSt/Pan 0 0 -63 \"L63\"\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_input_fader_level", {"channel": 2, "level": -1000}, "set MIXER:Current/InCh/Fader/Level 1 0 -1000\n",
                device_reply="ERROR set InvalidArgument\n", expect_result={"error": {"error": "device_error"}})
_yclql_explicit("text", "recall_scene", {"scene": 1}, "ssrecall_ex MIXER:Lib/Scene 1\n",
                device_reply="OK ssrecall_ex MIXER:Lib/Scene 1\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "get_current_scene", {}, "sscurrent_ex MIXER:Lib/Scene\n",
                device_reply="OK sscurrent_ex MIXER:Lib/Scene 12 modified\n", expect_result={"ok": {"kind": "value", "value": "12"}})
_yclql_explicit("telemetry", "scene-current", inbound="NOTIFY sscurrent_ex MIXER:Lib/Scene 12\n", expect_state={"scene": {"current": 12}})
_yclql_explicit("telemetry", "scene-modified", inbound="OK sscurrent_ex MIXER:Lib/Scene 12 modified\n",
                expect_state={"scene": {"current": 12, "modified": True}})
_yclql_explicit("telemetry", "scene-recalled", inbound="OK ssrecall_ex MIXER:Lib/Scene 7\n", expect_state={"scene": {"current": 7}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_yclql_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_yclql_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_yclql_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "CL5"\n',
     expect_result={"ok": {"kind": "value", "value": "CL5"}})
_yclql_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_yclql_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_yclql_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "CL5"\n', expect_state={"device": {"product_name": "CL5"}})
_yclql_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_yclql_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_yclql_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})
_yclql_explicit("text", "get_error_status", {}, "devstatus error\n", device_reply='OK devstatus error "none"\n',
     expect_result={"ok": {"kind": "value", "value": "none"}})
_yclql_explicit("telemetry", "error-status", inbound='NOTIFY devstatus error "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"\n',
     expect_state={"device": {"error": "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"}})


_yclql_vectors()
_yclql_EXPLICIT.clear()
