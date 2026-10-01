YDM7 = "yamaha-dm7"
# Yamaha DM7 over RCP (TCP 49280, LF-terminated). Addresses and ranges from Yamaha's DM7 Series OSC
# Specifications V1.1.0 (p.3-19); the RCP line syntax and replies from Yamaha's DME7 RCP
# specification V1.1.0 (see the spec's quirks). The OSC examples number X and Y from 1; on the RCP
# wire they are numbered from 0.


def _ydm7_vectors():
    S = YDM7
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 120), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 120), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 120), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('input_stereo_on', 'MIXER:Current/InCh/ToSt/On', ('channel', 120), ('stereo_bus', 2), ('bool',), 'inputs.{x}.stereo_sends.{y}.on', False),
        ('input_ha_gain', 'MIXER:Current/InCh/Port/HA/Gain', ('channel', 120), None, ('int', 'gain', -600, 6600, 'hundredths of a dB, -6 to +66 dB in 1 dB steps (Table 5)'), 'inputs.{x}.ha_gain', False),
        ('input_name', 'MIXER:Current/InCh/Label/Name', ('channel', 120), None, ('name', 8), 'inputs.{x}.name', False),
        ('input_color', 'MIXER:Current/InCh/Label/Color', ('channel', 120), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'inputs.{x}.color', False),
        ('input_icon', 'MIXER:Current/InCh/Label/Icon', ('channel', 120), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'inputs.{x}.icon', False),
        ('input_hpf_on', 'MIXER:Current/InCh/HPF/On', ('channel', 120), None, ('bool',), 'inputs.{x}.hpf.on', False),
        ('input_hpf_freq', 'MIXER:Current/InCh/HPF/Freq', ('channel', 120), None, ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'inputs.{x}.hpf.freq', False),
        ('input_hpf_slope', 'MIXER:Current/InCh/HPF/Slope', ('channel', 120), None, ('int', 'slope', 6, 24, 'dB/oct'), 'inputs.{x}.hpf.slope', False),
        ('input_lpf_on', 'MIXER:Current/InCh/LPF/On', ('channel', 120), None, ('bool',), 'inputs.{x}.lpf.on', False),
        ('input_lpf_freq', 'MIXER:Current/InCh/LPF/Freq', ('channel', 120), None, ('int', 'freq', 200, 200000, 'tenths of a Hz, 20 Hz to 20 kHz'), 'inputs.{x}.lpf.freq', False),
        ('input_lpf_slope', 'MIXER:Current/InCh/LPF/Slope', ('channel', 120), None, ('int', 'slope', 6, 12, 'dB/oct'), 'inputs.{x}.lpf.slope', False),
        ('input_eq_bank', 'MIXER:Current/InCh/PEQ/BankSelect', ('channel', 120), None, ('int', 'bank', 0, 1, '0 Bank A, 1 Bank B'), 'inputs.{x}.eq.bank', False),
        ('input_eq_on', 'MIXER:Current/InCh/PEQ/On', ('channel', 120), None, ('bool',), 'inputs.{x}.eq.on', False),
        ('input_eq_type', 'MIXER:Current/InCh/PEQ/Type', ('channel', 120), None, ('enum', ('PRECISE', 'AGGRESSIVE', 'SMOOTH', 'LEGACY')), 'inputs.{x}.eq.type', False),
        ('input_eq_band_bypass', 'MIXER:Current/InCh/PEQ/Band/Bypass', ('channel', 120), ('band', 4), ('flag', 'bypassed', '1 bypassed'), 'inputs.{x}.eq.bands.{y}.bypass', False),
        ('input_eq_band_freq', 'MIXER:Current/InCh/PEQ/Band/Freq', ('channel', 120), ('band', 4), ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'inputs.{x}.eq.bands.{y}.freq', False),
        ('input_eq_band_gain', 'MIXER:Current/InCh/PEQ/Band/Gain', ('channel', 120), ('band', 4), ('int', 'gain', -1800, 1800, 'raw value; the document gives scaling 10'), 'inputs.{x}.eq.bands.{y}.gain', False),
        ('input_eq_band_q', 'MIXER:Current/InCh/PEQ/Band/Q', ('channel', 120), ('band', 4), ('int', 'q', 100, 16000, 'thousandths, 0.1 to 16.0'), 'inputs.{x}.eq.bands.{y}.q', False),
        ('input_eq_high_shelf_on', 'MIXER:Current/InCh/PEQ/HighShelving/On', ('channel', 120), None, ('bool',), 'inputs.{x}.eq.high_shelf', False),
        ('input_eq_low_shelf_on', 'MIXER:Current/InCh/PEQ/LowShelving/On', ('channel', 120), None, ('bool',), 'inputs.{x}.eq.low_shelf', False),
        ('input_mix_send_level', 'MIXER:Current/InCh/ToMix/Level', ('channel', 120), ('mix', 48), ('level', 1000), 'inputs.{x}.mix_sends.{y}.level', False),
        ('input_mix_send_on', 'MIXER:Current/InCh/ToMix/On', ('channel', 120), ('mix', 48), ('bool',), 'inputs.{x}.mix_sends.{y}.on', False),
        ('input_mix_send_pan', 'MIXER:Current/InCh/ToMix/Pan', ('channel', 120), ('mix', 48), ('pan',), 'inputs.{x}.mix_sends.{y}.pan', False),
        ('input_mix_send_pre', 'MIXER:Current/InCh/ToMix/PrePost', ('channel', 120), ('mix', 48), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'inputs.{x}.mix_sends.{y}.pre', False),
        ('input_matrix_send_level', 'MIXER:Current/InCh/ToMtrx/Level', ('channel', 120), ('matrix', 12), ('level', 1000), 'inputs.{x}.matrix_sends.{y}.level', False),
        ('input_matrix_send_on', 'MIXER:Current/InCh/ToMtrx/On', ('channel', 120), ('matrix', 12), ('bool',), 'inputs.{x}.matrix_sends.{y}.on', False),
        ('input_matrix_send_pan', 'MIXER:Current/InCh/ToMtrx/Pan', ('channel', 120), ('matrix', 12), ('pan',), 'inputs.{x}.matrix_sends.{y}.pan', False),
        ('input_matrix_send_pre', 'MIXER:Current/InCh/ToMtrx/PrePost', ('channel', 120), ('matrix', 12), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'inputs.{x}.matrix_sends.{y}.pre', False),
        ('input_dca_assign', 'MIXER:Current/InCh/DCA/Assign', ('channel', 120), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'inputs.{x}.dcas.{y}', False),
        ('input_patch_select', 'MIXER:Current/InCh/PatchSelect', ('channel', 120), None, ('int', 'patch', 0, 1, '0 patch A, 1 patch B'), 'inputs.{x}.patch_select', False),
        ('input_surround_lr_pan', 'MIXER:Current/InCh/Surr/LRPan', ('channel', 120), None, ('pan',), 'inputs.{x}.surround.lr_pan', False),
        ('input_surround_fr_pan', 'MIXER:Current/InCh/Surr/FRPan', ('channel', 120), None, ('int', 'pan', -63, 63, '-63 to 63'), 'inputs.{x}.surround.fr_pan', False),
        ('input_surround_divergence', 'MIXER:Current/InCh/Surr/Div', ('channel', 120), None, ('int', 'divergence', 0, 100, '0 to 100'), 'inputs.{x}.surround.divergence', False),
        ('input_surround_l_on', 'MIXER:Current/InCh/Surr/LOn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.l_on', False),
        ('input_surround_r_on', 'MIXER:Current/InCh/Surr/ROn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.r_on', False),
        ('input_surround_c_on', 'MIXER:Current/InCh/Surr/COn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.c_on', False),
        ('input_surround_lfe_on', 'MIXER:Current/InCh/Surr/LFEOn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.lfe_on', False),
        ('input_surround_ls_on', 'MIXER:Current/InCh/Surr/LsOn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.ls_on', False),
        ('input_surround_rs_on', 'MIXER:Current/InCh/Surr/RsOn', ('channel', 120), None, ('bool',), 'inputs.{x}.surround.rs_on', False),
        ('input_surround_lfe_level', 'MIXER:Current/InCh/Surr/LFELevel', ('channel', 120), None, ('level', 1000), 'inputs.{x}.surround.lfe_level', False),
        ('input_cue_on', 'MIXER:Current/Cue/InCh/On', ('channel', 120), ('cue', 2), ('bool',), 'inputs.{x}.cue.{y}', False),
        ('input_link_group', 'MIXER:Current/InputChLink/InCh/Assign', ('channel', 120), None, ('int', 'group', 0, 52, '0 none, 1 A ... 26 Z, 27 a ... 52 z'), 'inputs.{x}.link_group', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 48), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 48), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_pan_link', 'MIXER:Current/Mix/PanLink', ('mix', 48), None, ('bool',), 'mixes.{x}.pan_link', False),
        ('mix_stereo_pan', 'MIXER:Current/Mix/ToSt/Pan', ('mix', 48), None, ('pan',), 'mixes.{x}.stereo_pan', False),
        ('mix_stereo_on', 'MIXER:Current/Mix/ToSt/On', ('mix', 48), ('stereo_bus', 2), ('bool',), 'mixes.{x}.stereo_sends.{y}.on', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 48), None, ('pan',), 'mixes.{x}.balance', False),
        ('mix_name', 'MIXER:Current/Mix/Label/Name', ('mix', 48), None, ('name', 8), 'mixes.{x}.name', False),
        ('mix_color', 'MIXER:Current/Mix/Label/Color', ('mix', 48), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'mixes.{x}.color', False),
        ('mix_icon', 'MIXER:Current/Mix/Label/Icon', ('mix', 48), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'mixes.{x}.icon', False),
        ('mix_hpf_on', 'MIXER:Current/Mix/HPF/On', ('mix', 48), None, ('bool',), 'mixes.{x}.hpf.on', False),
        ('mix_hpf_freq', 'MIXER:Current/Mix/HPF/Freq', ('mix', 48), None, ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'mixes.{x}.hpf.freq', False),
        ('mix_hpf_slope', 'MIXER:Current/Mix/HPF/Slope', ('mix', 48), None, ('int', 'slope', 6, 24, 'dB/oct'), 'mixes.{x}.hpf.slope', False),
        ('mix_lpf_on', 'MIXER:Current/Mix/LPF/On', ('mix', 48), None, ('bool',), 'mixes.{x}.lpf.on', False),
        ('mix_lpf_freq', 'MIXER:Current/Mix/LPF/Freq', ('mix', 48), None, ('int', 'freq', 200, 200000, 'tenths of a Hz, 20 Hz to 20 kHz'), 'mixes.{x}.lpf.freq', False),
        ('mix_lpf_slope', 'MIXER:Current/Mix/LPF/Slope', ('mix', 48), None, ('int', 'slope', 6, 12, 'dB/oct'), 'mixes.{x}.lpf.slope', False),
        ('mix_eq_bank', 'MIXER:Current/Mix/PEQ/BankSelect', ('mix', 48), None, ('int', 'bank', 0, 1, '0 Bank A, 1 Bank B'), 'mixes.{x}.eq.bank', False),
        ('mix_eq_on', 'MIXER:Current/Mix/PEQ/On', ('mix', 48), None, ('bool',), 'mixes.{x}.eq.on', False),
        ('mix_eq_type', 'MIXER:Current/Mix/PEQ/Type', ('mix', 48), None, ('enum', ('PRECISE', 'AGGRESSIVE', 'SMOOTH', 'LEGACY')), 'mixes.{x}.eq.type', False),
        ('mix_eq_band_bypass', 'MIXER:Current/Mix/PEQ/Band/Bypass', ('mix', 48), ('band', 8), ('flag', 'bypassed', '1 bypassed'), 'mixes.{x}.eq.bands.{y}.bypass', False),
        ('mix_eq_band_freq', 'MIXER:Current/Mix/PEQ/Band/Freq', ('mix', 48), ('band', 8), ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'mixes.{x}.eq.bands.{y}.freq', False),
        ('mix_eq_band_gain', 'MIXER:Current/Mix/PEQ/Band/Gain', ('mix', 48), ('band', 8), ('int', 'gain', -1800, 1800, 'raw value; the document gives scaling 10'), 'mixes.{x}.eq.bands.{y}.gain', False),
        ('mix_eq_band_q', 'MIXER:Current/Mix/PEQ/Band/Q', ('mix', 48), ('band', 8), ('int', 'q', 100, 16000, 'thousandths, 0.1 to 16.0'), 'mixes.{x}.eq.bands.{y}.q', False),
        ('mix_eq_high_shelf_on', 'MIXER:Current/Mix/PEQ/HighShelving/On', ('mix', 48), None, ('bool',), 'mixes.{x}.eq.high_shelf', False),
        ('mix_eq_low_shelf_on', 'MIXER:Current/Mix/PEQ/LowShelving/On', ('mix', 48), None, ('bool',), 'mixes.{x}.eq.low_shelf', False),
        ('mix_minus_owner', 'MIXER:Current/Mix/MixMinus/Owner/InputChannel', ('mix', 48), ('channel', 120), ('flag', 'owner', '1 the input channel owns the mix-minus'), 'mixes.{x}.mix_minus_owner.{y}', False),
        ('mix_matrix_send_level', 'MIXER:Current/Mix/ToMtrx/Level', ('mix', 48), ('matrix', 12), ('level', 1000), 'mixes.{x}.matrix_sends.{y}.level', False),
        ('mix_matrix_send_on', 'MIXER:Current/Mix/ToMtrx/On', ('mix', 48), ('matrix', 12), ('bool',), 'mixes.{x}.matrix_sends.{y}.on', False),
        ('mix_matrix_send_pan', 'MIXER:Current/Mix/ToMtrx/Pan', ('mix', 48), ('matrix', 12), ('pan',), 'mixes.{x}.matrix_sends.{y}.pan', False),
        ('mix_matrix_send_pre', 'MIXER:Current/Mix/ToMtrx/PrePost', ('mix', 48), ('matrix', 12), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'mixes.{x}.matrix_sends.{y}.pre', False),
        ('mix_dca_assign', 'MIXER:Current/Mix/DCA/Assign', ('mix', 48), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'mixes.{x}.dcas.{y}', False),
        ('mix_cue_on', 'MIXER:Current/Cue/Mix/On', ('mix', 48), ('cue', 2), ('bool',), 'mixes.{x}.cue.{y}', False),
        ('mix_link_group', 'MIXER:Current/OutputChLink/Mix/Assign', ('mix', 48), None, ('int', 'group', 0, 52, '0 none, 1 A ... 52 z'), 'mixes.{x}.link_group', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 4), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 4), None, ('bool',), 'stereo.{x}.on', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 4), None, ('pan',), 'stereo.{x}.balance', False),
        ('stereo_name', 'MIXER:Current/St/Label/Name', ('stereo', 4), None, ('name', 8), 'stereo.{x}.name', False),
        ('stereo_color', 'MIXER:Current/St/Label/Color', ('stereo', 4), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'stereo.{x}.color', False),
        ('stereo_icon', 'MIXER:Current/St/Label/Icon', ('stereo', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'stereo.{x}.icon', False),
        ('stereo_matrix_send_level', 'MIXER:Current/St/ToMtrx/Level', ('stereo', 4), ('matrix', 12), ('level', 1000), 'stereo.{x}.matrix_sends.{y}.level', False),
        ('stereo_matrix_send_on', 'MIXER:Current/St/ToMtrx/On', ('stereo', 4), ('matrix', 12), ('bool',), 'stereo.{x}.matrix_sends.{y}.on', False),
        ('stereo_matrix_send_pan', 'MIXER:Current/St/ToMtrx/Pan', ('stereo', 4), ('matrix', 12), ('pan',), 'stereo.{x}.matrix_sends.{y}.pan', False),
        ('stereo_matrix_send_pre', 'MIXER:Current/St/ToMtrx/PrePost', ('stereo', 4), ('matrix', 12), ('flag', 'pre', '1 ON, 0 OFF as the document states'), 'stereo.{x}.matrix_sends.{y}.pre', False),
        ('stereo_dca_assign', 'MIXER:Current/St/DCA/Assign', ('stereo', 4), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'stereo.{x}.dcas.{y}', False),
        ('stereo_cue_on', 'MIXER:Current/Cue/St/On', ('stereo', 4), ('cue', 2), ('bool',), 'stereo.{x}.cue.{y}', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 12), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.on', False),
        ('matrix_name', 'MIXER:Current/Mtrx/Label/Name', ('matrix', 12), None, ('name', 8), 'matrices.{x}.name', False),
        ('matrix_color', 'MIXER:Current/Mtrx/Label/Color', ('matrix', 12), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'matrices.{x}.color', False),
        ('matrix_icon', 'MIXER:Current/Mtrx/Label/Icon', ('matrix', 12), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'matrices.{x}.icon', False),
        ('matrix_hpf_on', 'MIXER:Current/Mtrx/HPF/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.hpf.on', False),
        ('matrix_hpf_freq', 'MIXER:Current/Mtrx/HPF/Freq', ('matrix', 12), None, ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'matrices.{x}.hpf.freq', False),
        ('matrix_hpf_slope', 'MIXER:Current/Mtrx/HPF/Slope', ('matrix', 12), None, ('int', 'slope', 6, 24, 'dB/oct'), 'matrices.{x}.hpf.slope', False),
        ('matrix_lpf_on', 'MIXER:Current/Mtrx/LPF/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.lpf.on', False),
        ('matrix_lpf_freq', 'MIXER:Current/Mtrx/LPF/Freq', ('matrix', 12), None, ('int', 'freq', 200, 200000, 'tenths of a Hz, 20 Hz to 20 kHz'), 'matrices.{x}.lpf.freq', False),
        ('matrix_lpf_slope', 'MIXER:Current/Mtrx/LPF/Slope', ('matrix', 12), None, ('int', 'slope', 6, 12, 'dB/oct'), 'matrices.{x}.lpf.slope', False),
        ('matrix_eq_bank', 'MIXER:Current/Mtrx/PEQ/BankSelect', ('matrix', 12), None, ('int', 'bank', 0, 1, '0 Bank A, 1 Bank B'), 'matrices.{x}.eq.bank', False),
        ('matrix_eq_on', 'MIXER:Current/Mtrx/PEQ/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.eq.on', False),
        ('matrix_eq_type', 'MIXER:Current/Mtrx/PEQ/Type', ('matrix', 12), None, ('enum', ('PRECISE', 'AGGRESSIVE', 'SMOOTH', 'LEGACY')), 'matrices.{x}.eq.type', False),
        ('matrix_eq_band_bypass', 'MIXER:Current/Mtrx/PEQ/Band/Bypass', ('matrix', 12), ('band', 8), ('flag', 'bypassed', '1 bypassed'), 'matrices.{x}.eq.bands.{y}.bypass', False),
        ('matrix_eq_band_freq', 'MIXER:Current/Mtrx/PEQ/Band/Freq', ('matrix', 12), ('band', 8), ('int', 'freq', 200, 200000, 'tenths of a Hz'), 'matrices.{x}.eq.bands.{y}.freq', False),
        ('matrix_eq_band_gain', 'MIXER:Current/Mtrx/PEQ/Band/Gain', ('matrix', 12), ('band', 8), ('int', 'gain', -1800, 1800, 'raw value; the document gives scaling 10'), 'matrices.{x}.eq.bands.{y}.gain', False),
        ('matrix_eq_band_q', 'MIXER:Current/Mtrx/PEQ/Band/Q', ('matrix', 12), ('band', 8), ('int', 'q', 100, 16000, 'thousandths, 0.1 to 16.0'), 'matrices.{x}.eq.bands.{y}.q', False),
        ('matrix_eq_high_shelf_on', 'MIXER:Current/Mtrx/PEQ/HighShelving/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.eq.high_shelf', False),
        ('matrix_eq_low_shelf_on', 'MIXER:Current/Mtrx/PEQ/LowShelving/On', ('matrix', 12), None, ('bool',), 'matrices.{x}.eq.low_shelf', False),
        ('matrix_pan_link', 'MIXER:Current/Mtrx/PanLink', ('matrix', 12), None, ('bool',), 'matrices.{x}.pan_link', False),
        ('matrix_balance', 'MIXER:Current/Mtrx/Out/Balance', ('matrix', 12), None, ('pan',), 'matrices.{x}.balance', False),
        ('matrix_dca_assign', 'MIXER:Current/Mtrx/DCA/Assign', ('matrix', 12), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'matrices.{x}.dcas.{y}', False),
        ('matrix_cue_on', 'MIXER:Current/Cue/Mtrx/On', ('matrix', 12), ('cue', 2), ('bool',), 'matrices.{x}.cue.{y}', False),
        ('matrix_link_group', 'MIXER:Current/OutputChLink/Mtrx/Assign', ('matrix', 12), None, ('int', 'group', 0, 52, '0 none, 1 A ... 52 z'), 'matrices.{x}.link_group', False),
        ('dca_fader_level', 'MIXER:Current/DCA/Fader/Level', ('dca', 24), None, ('level', 1000), 'dcas.{x}.fader_level', False),
        ('dca_on', 'MIXER:Current/DCA/Fader/On', ('dca', 24), None, ('bool',), 'dcas.{x}.on', False),
        ('dca_name', 'MIXER:Current/DCA/Label/Name', ('dca', 24), None, ('name', 8), 'dcas.{x}.name', False),
        ('dca_color', 'MIXER:Current/DCA/Label/Color', ('dca', 24), None, ('enum', ('Blue', 'Green', 'Orange', 'Pink', 'Purple', 'Red', 'SkyBlue', 'Yellow', 'Cyan', 'Magenta', 'Off')), 'dcas.{x}.color', False),
        ('dca_icon', 'MIXER:Current/DCA/Label/Icon', ('dca', 24), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'Drumkit', 'Perc.', 'Mallets', 'A.Bass', 'E.Bass', 'BassAmp', 'A.Guitar', 'E.Guitar', 'GuitarAmp', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Strings', 'Piano', 'Organ', 'Keyboard', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'DirectBox', 'Foh', 'Speaker', 'SubWoofer', 'Wedge', 'In-Ear', 'Monitor', 'Effect', 'Processor', 'Media1', 'Media2', 'Media3', 'Video', 'Mixer', 'PC', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'dcas.{x}.icon', False),
        ('dca_cue_on', 'MIXER:Current/Cue/DCA/On', ('dca', 24), ('cue', 2), ('bool',), 'dcas.{x}.cue.{y}', False),
        ('mute_group_on', 'MIXER:Current/MuteGrpCtrl/On', ('mute_group', 12), None, ('bool',), 'mute_groups.{x}.on', False),
        ('mute_group_name', 'MIXER:Current/MuteGrpCtrl/Label/Name', ('mute_group', 12), None, ('name', 8), 'mute_groups.{x}.name', False),
        ('input_link_eq', 'MIXER:Current/InputChLink/LinkParams/EQ', ('link_group', 52), None, ('bool',), 'input_links.{x}.eq', False),
        ('input_link_insert', 'MIXER:Current/InputChLink/LinkParams/Insert', ('link_group', 52), None, ('bool',), 'input_links.{x}.insert', False),
        ('input_link_fader', 'MIXER:Current/InputChLink/LinkParams/Fader', ('link_group', 52), None, ('bool',), 'input_links.{x}.fader', False),
        ('input_link_channel_on', 'MIXER:Current/InputChLink/LinkParams/ChOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.channel_on', False),
        ('input_link_to_stereo', 'MIXER:Current/InputChLink/LinkParams/ToSt', ('link_group', 52), None, ('bool',), 'input_links.{x}.to_stereo', False),
        ('input_link_dca', 'MIXER:Current/InputChLink/LinkParams/DCA', ('link_group', 52), None, ('bool',), 'input_links.{x}.dca', False),
        ('input_link_mute', 'MIXER:Current/InputChLink/LinkParams/Mute', ('link_group', 52), None, ('bool',), 'input_links.{x}.mute', False),
        ('input_link_matrix_send', 'MIXER:Current/InputChLink/LinkParams/MtrxSend', ('link_group', 52), None, ('bool',), 'input_links.{x}.matrix_send', False),
        ('input_link_matrix_send_on', 'MIXER:Current/InputChLink/LinkParams/MtrxSendOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.matrix_send_on', False),
        ('input_link_ha', 'MIXER:Current/InputChLink/LinkParams/HA', ('link_group', 52), None, ('bool',), 'input_links.{x}.ha', False),
        ('input_link_hpf', 'MIXER:Current/InputChLink/LinkParams/HPF', ('link_group', 52), None, ('bool',), 'input_links.{x}.hpf', False),
        ('input_link_digital_gain', 'MIXER:Current/InputChLink/LinkParams/DigitalGain', ('link_group', 52), None, ('bool',), 'input_links.{x}.digital_gain', False),
        ('input_link_direct_out', 'MIXER:Current/InputChLink/LinkParams/DirectOut', ('link_group', 52), None, ('bool',), 'input_links.{x}.direct_out', False),
        ('input_link_mix_send', 'MIXER:Current/InputChLink/LinkParams/MixSend', ('link_group', 52), None, ('bool',), 'input_links.{x}.mix_send', False),
        ('input_link_mix_send_on', 'MIXER:Current/InputChLink/LinkParams/MixSendOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.mix_send_on', False),
        ('input_link_delay', 'MIXER:Current/InputChLink/LinkParams/Delay', ('link_group', 52), None, ('bool',), 'input_links.{x}.delay', False),
        ('input_link_dynamics1', 'MIXER:Current/InputChLink/LinkParams/Dyna1', ('link_group', 52), None, ('bool',), 'input_links.{x}.dynamics1', False),
        ('input_link_dynamics2', 'MIXER:Current/InputChLink/LinkParams/Dyna2', ('link_group', 52), None, ('bool',), 'input_links.{x}.dynamics2', False),
        ('input_link_send_to_mix', 'MIXER:Current/InputChLink/SendParams/ToMix', ('link_group', 52), ('mix', 48), ('bool',), 'input_links.{x}.send_to_mix.{y}', False),
        ('input_link_send_to_matrix', 'MIXER:Current/InputChLink/SendParams/ToMtrx', ('link_group', 52), ('matrix', 12), ('bool',), 'input_links.{x}.send_to_matrix.{y}', False),
        ('output_link_eq', 'MIXER:Current/OutputChLink/LinkParams/EQ', ('link_group', 52), None, ('bool',), 'output_links.{x}.eq', False),
        ('output_link_insert', 'MIXER:Current/OutputChLink/LinkParams/Insert', ('link_group', 52), None, ('bool',), 'output_links.{x}.insert', False),
        ('output_link_fader', 'MIXER:Current/OutputChLink/LinkParams/Fader', ('link_group', 52), None, ('bool',), 'output_links.{x}.fader', False),
        ('output_link_channel_on', 'MIXER:Current/OutputChLink/LinkParams/ChOn', ('link_group', 52), None, ('bool',), 'output_links.{x}.channel_on', False),
        ('output_link_to_stereo', 'MIXER:Current/OutputChLink/LinkParams/ToSt', ('link_group', 52), None, ('bool',), 'output_links.{x}.to_stereo', False),
        ('output_link_dca', 'MIXER:Current/OutputChLink/LinkParams/DCA', ('link_group', 52), None, ('bool',), 'output_links.{x}.dca', False),
        ('output_link_mute', 'MIXER:Current/OutputChLink/LinkParams/Mute', ('link_group', 52), None, ('bool',), 'output_links.{x}.mute', False),
        ('output_link_matrix_send', 'MIXER:Current/OutputChLink/LinkParams/MtrxSend', ('link_group', 52), None, ('bool',), 'output_links.{x}.matrix_send', False),
        ('output_link_matrix_send_on', 'MIXER:Current/OutputChLink/LinkParams/MtrxSendOn', ('link_group', 52), None, ('bool',), 'output_links.{x}.matrix_send_on', False),
        ('output_link_dynamics1', 'MIXER:Current/OutputChLink/LinkParams/Dyna1', ('link_group', 52), None, ('bool',), 'output_links.{x}.dynamics1', False),
        ('output_link_send_to_matrix', 'MIXER:Current/OutputChLink/SendParams/ToMtrx', ('link_group', 52), ('matrix', 12), ('bool',), 'output_links.{x}.send_to_matrix.{y}', False),
        ('monitor_surround_solo_mode', 'MIXER:Current/Monitor/Surr/SoloMode', None, None, ('bool',), 'monitor.surround.solo_mode', False),
        ('monitor_surround_l_on', 'MIXER:Current/Monitor/Surr/LOn', None, None, ('bool',), 'monitor.surround.l_on', False),
        ('monitor_surround_r_on', 'MIXER:Current/Monitor/Surr/ROn', None, None, ('bool',), 'monitor.surround.r_on', False),
        ('monitor_surround_c_on', 'MIXER:Current/Monitor/Surr/COn', None, None, ('bool',), 'monitor.surround.c_on', False),
        ('monitor_surround_lfe_on', 'MIXER:Current/Monitor/Surr/LFEOn', None, None, ('bool',), 'monitor.surround.lfe_on', False),
        ('monitor_surround_ls_on', 'MIXER:Current/Monitor/Surr/LsOn', None, None, ('bool',), 'monitor.surround.ls_on', False),
        ('monitor_surround_rs_on', 'MIXER:Current/Monitor/Surr/RsOn', None, None, ('bool',), 'monitor.surround.rs_on', False),
        ('monitor_surround_source', 'MIXER:Current/Monitor/Surr/SourceSelect', None, None, ('enum', ('Surr A', 'Surr B', 'Downmix A', 'Downmix B', 'Ext Surr5.1 1', 'Ext Surr5.1 2', 'Ext Surr5.1 3', 'Ext Surr5.1 4', 'Ext St 1', 'Ext St 2', 'Ext St 3', 'Ext St 4', 'Define')), 'monitor.surround.source', False),
        ('monitor_downmix_l_to_l_on', 'MIXER:Current/Monitor/DownMix/LToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.l_to_l_on', False),
        ('monitor_downmix_l_to_r_on', 'MIXER:Current/Monitor/DownMix/LToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.l_to_r_on', False),
        ('monitor_downmix_l_to_lr_level', 'MIXER:Current/Monitor/DownMix/LToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.l_to_lr_level', False),
        ('monitor_downmix_r_to_l_on', 'MIXER:Current/Monitor/DownMix/RToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.r_to_l_on', False),
        ('monitor_downmix_r_to_r_on', 'MIXER:Current/Monitor/DownMix/RToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.r_to_r_on', False),
        ('monitor_downmix_r_to_lr_level', 'MIXER:Current/Monitor/DownMix/RToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.r_to_lr_level', False),
        ('monitor_downmix_c_to_l_on', 'MIXER:Current/Monitor/DownMix/CToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.c_to_l_on', False),
        ('monitor_downmix_c_to_r_on', 'MIXER:Current/Monitor/DownMix/CToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.c_to_r_on', False),
        ('monitor_downmix_c_to_lr_level', 'MIXER:Current/Monitor/DownMix/CToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.c_to_lr_level', False),
        ('monitor_downmix_lfe_to_l_on', 'MIXER:Current/Monitor/DownMix/LFEToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.lfe_to_l_on', False),
        ('monitor_downmix_lfe_to_r_on', 'MIXER:Current/Monitor/DownMix/LFEToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.lfe_to_r_on', False),
        ('monitor_downmix_lfe_to_lr_level', 'MIXER:Current/Monitor/DownMix/LFEToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.lfe_to_lr_level', False),
        ('monitor_downmix_ls_to_l_on', 'MIXER:Current/Monitor/DownMix/LsToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.ls_to_l_on', False),
        ('monitor_downmix_ls_to_r_on', 'MIXER:Current/Monitor/DownMix/LsToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.ls_to_r_on', False),
        ('monitor_downmix_ls_to_lr_level', 'MIXER:Current/Monitor/DownMix/LsToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.ls_to_lr_level', False),
        ('monitor_downmix_rs_to_l_on', 'MIXER:Current/Monitor/DownMix/RsToLOn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.rs_to_l_on', False),
        ('monitor_downmix_rs_to_r_on', 'MIXER:Current/Monitor/DownMix/RsToROn', ('downmix', 2), None, ('bool',), 'monitor.downmix.{x}.rs_to_r_on', False),
        ('monitor_downmix_rs_to_lr_level', 'MIXER:Current/Monitor/DownMix/RsToLRLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.rs_to_lr_level', False),
        ('monitor_downmix_main_level', 'MIXER:Current/Monitor/DownMix/MainLevel', ('downmix', 2), None, ('level', 0), 'monitor.downmix.{x}.main_level', False),
        ('monitor_downmix_type', 'MIXER:Current/Monitor/DownMix/Type', None, None, ('enum', ('Stereo', 'Mono')), 'monitor.downmix_type', False),
        ('monitor_on', 'MIXER:Current/Monitor/On', ('monitor', 2), None, ('bool',), 'monitors.{x}.on', False),
        ('monitor_dimmer_on', 'MIXER:Current/Monitor/DimmerOn', ('dimmer', 2), None, ('bool',), 'monitors.{x}.dimmer', False),
        ('monitor_phones_level_link', 'MIXER:Current/Monitor/PhonesLevelLink', ('monitor', 2), None, ('bool',), 'monitors.{x}.phones_level_link', False),
        ('monitor_cue_interruption', 'MIXER:Current/Monitor/CueInterruption', ('monitor', 2), None, ('bool',), 'monitors.{x}.cue_interruption', False),
        ('monitor_fader_level', 'MIXER:Current/Monitor/Fader/Level', ('monitor', 2), None, ('level', 0), 'monitors.{x}.fader_level', False),
        ('monitor_source', 'MIXER:Current/Monitor/St/SourceSelect', ('monitor', 2), None, ('int', 'source', 0, 7, 'monitor source number'), 'monitors.{x}.source', False),
        ('cue_surround_mode', 'MIXER:Current/Cue/Surr/CueMode', None, None, ('enum', ('LAST', 'MIX')), 'cue.surround_mode', False),
        ('cue_output_on', 'MIXER:Current/Cue/Out/On', ('cue', 2), None, ('bool',), 'cue.{x}.output_on', False),
        ('cue_mode', 'MIXER:Current/Cue/CueMode', ('cue', 2), None, ('enum', ('LAST', 'MIX')), 'cue.{x}.mode', False),
        ('cue_input_point', 'MIXER:Current/Cue/InCh/Point', ('cue', 2), None, ('enum', ('PFL PRE FILTER', 'PFL PRE FADER', 'AFL', 'POST PAN')), 'cue.{x}.input_point', False),
        ('cue_output_point', 'MIXER:Current/Cue/OutCh/Point', ('cue', 2), None, ('enum', ('PFL', 'AFL')), 'cue.{x}.output_point', False),
        ('cue_dca_point', 'MIXER:Current/Cue/DCA/Point', ('cue', 2), None, ('enum', ('PRE PAN', 'POST PAN')), 'cue.{x}.dca_point', False),
        ('cue_dca_unity', 'MIXER:Current/Cue/DCA/Unity', ('cue', 2), None, ('bool',), 'cue.{x}.dca_unity', False),
        ('cue_output_level', 'MIXER:Current/Cue/Out/Level', ('cue', 2), None, ('level', 0), 'cue.{x}.output_level', False),
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

    done = set(_ydm7_EXPLICIT)
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
    for args, kw in _ydm7_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_ydm7_EXPLICIT = {}


def _ydm7_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _ydm7_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (OSC spec 1.4, p.3-8), restated for the RCP wire.
_ydm7_explicit("text", "set_input_fader_level", {"channel": 61, "level": -2000}, "set MIXER:Current/InCh/Fader/Level 60 0 -2000\n",
               device_reply="OK set MIXER:Current/InCh/Fader/Level 60 0 -2000 \"-20.00\"\n", expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "set_input_name", {"channel": 25, "name": "LeadVox"}, 'set MIXER:Current/InCh/Label/Name 24 0 "LeadVox"\n',
               device_reply='OK set MIXER:Current/InCh/Label/Name 24 0 "LeadVox"\n', expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "set_input_color", {"channel": 8, "value": "Green"}, 'set MIXER:Current/InCh/Label/Color 7 0 "Green"\n')
_ydm7_explicit("text", "set_input_mix_send_on", {"channel": 8, "mix": 10, "enabled": True}, "set MIXER:Current/InCh/ToMix/On 7 9 1\n",
               device_reply="OK set MIXER:Current/InCh/ToMix/On 7 9 1\n", expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "set_input_mix_send_pan", {"channel": 71, "mix": 41, "pan": 63}, "set MIXER:Current/InCh/ToMix/Pan 70 40 63\n")
_ydm7_explicit("text", "set_input_matrix_send_level", {"channel": 45, "matrix": 2, "level": -32768},
               "set MIXER:Current/InCh/ToMtrx/Level 44 1 -32768\n")
_ydm7_explicit("text", "set_input_dca_assign", {"channel": 17, "dca": 8, "assigned": False}, "set MIXER:Current/InCh/DCA/Assign 16 7 0\n",
               device_reply="ERROR set InvalidArgument\n", expect_result={"error": {"error": "device_error"}})
_ydm7_explicit("text", "set_matrix_name", {"matrix": 7, "name": "SUB"}, 'set MIXER:Current/Mtrx/Label/Name 6 0 "SUB"\n')
_ydm7_explicit("text", "set_dca_on", {"dca": 12, "enabled": True}, "set MIXER:Current/DCA/Fader/On 11 0 1\n")
_ydm7_explicit("text", "recall_scene", {"bank": "a", "scene": "4.00"}, 'ssrecallt_ex scene_a "4.00"\n',
               device_reply='OK ssrecallt_ex scene_a "4.00"\n', expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "get_current_scene", {"bank": "a"}, "sscurrentt_ex scene_a\n",
               device_reply='OK sscurrentt_ex scene_a "12.50" modified\n', expect_result={"ok": {"kind": "value", "value": "12.50"}})
_ydm7_explicit("text", "recall_next_scene", {"bank": "a"}, "event MIXER:Lib/Scene/RecallInc scene_a\n",
               device_reply="OK event MIXER:Lib/Scene/RecallInc scene_a\n", expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "recall_previous_scene", {"bank": "b"}, "event MIXER:Lib/Scene/RecallDec scene_b\n",
               device_reply="ERROR event AccessDenied\n", expect_result={"error": {"error": "device_error"}})
_ydm7_explicit("telemetry", "scene-current", inbound='NOTIFY sscurrentt_ex scene_a "12.50"\n',
               expect_state={"scenes": {"a": {"current": "12.50"}}})
_ydm7_explicit("telemetry", "scene-modified", inbound='OK sscurrentt_ex scene_a "12.50" modified\n',
               expect_state={"scenes": {"a": {"current": "12.50", "modified": True}}})
_ydm7_explicit("telemetry", "scene-recalled", inbound='OK ssrecallt_ex scene_b "3.00"\n', expect_state={"scenes": {"b": {"current": "3.00"}}})
_ydm7_explicit("telemetry", "input_fader_level", inbound='NOTIFY set MIXER:Current/InCh/Fader/Level 60 0 -2000 "-20.00"\n',
               expect_state={"inputs": {"61": {"fader_level": -2000}}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_ydm7_explicit("text", "set_parameter", {"address": "MIXER:Current/Mix/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/Mix/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/Mix/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_ydm7_explicit("text", "get_parameter", {"address": "MIXER:Current/Mix/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/Mix/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/Mix/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_ydm7_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "DM7"\n',
     expect_result={"ok": {"kind": "value", "value": "DM7"}})
_ydm7_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_ydm7_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_ydm7_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_ydm7_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "DM7"\n', expect_state={"device": {"product_name": "DM7"}})
_ydm7_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_ydm7_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_ydm7_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})


_ydm7_vectors()
_ydm7_EXPLICIT.clear()
