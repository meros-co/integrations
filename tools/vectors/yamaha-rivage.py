YPM = "yamaha-rivage"
# Yamaha RIVAGE PM over RCP (TCP 49280, LF-terminated). Addresses and ranges from Yamaha's RIVAGE PM
# Series OSC Specifications V1.0.2 (p.3-15); the RCP line syntax and replies from Yamaha's DME7 RCP
# specification V1.1.0. The OSC examples number X and Y from 1; on the RCP wire they start at 0.


def _ypm_vectors():
    S = YPM
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('cue_active', 'MIXER:Current/Cue/ActiveCue', ('cue', 2), None, ('enum', ('NONE', 'OTHERS', 'INPUT', 'DCA', 'OUTPUT')), 'cue.{x}.active', False),
        ('cue_mode', 'MIXER:Current/Cue/CueMode', ('cue', 2), None, ('enum', ('MIX', 'LAST')), 'cue.{x}.mode', False),
        ('dca_cue_on', 'MIXER:Current/Cue/DCA/On', ('dca', 24), ('cue', 2), ('bool',), 'dcas.{x}.cue.{y}', False),
        ('cue_dca_point', 'MIXER:Current/Cue/DCA/Point', ('cue', 2), None, ('enum', ('PRE PAN', 'POST PAN')), 'cue.{x}.dca_point', False),
        ('cue_dca_unity', 'MIXER:Current/Cue/DCA/Unity', ('cue', 2), None, ('bool',), 'cue.{x}.dca_unity', False),
        ('input_cue_on', 'MIXER:Current/Cue/InCh/On', ('channel', 288), ('cue', 2), ('bool',), 'inputs.{x}.cue.{y}', False),
        ('cue_input_point', 'MIXER:Current/Cue/InCh/Point', ('cue', 2), None, ('enum', ('PFL PRE FILTER', 'PFL PRE FADER', 'AFL', 'POST PAN')), 'cue.{x}.input_point', False),
        ('mix_cue_on', 'MIXER:Current/Cue/Mix/On', ('mix', 72), ('cue', 2), ('bool',), 'mixes.{x}.cue.{y}', False),
        ('matrix_cue_on', 'MIXER:Current/Cue/Mtrx/On', ('matrix', 36), ('cue', 2), ('bool',), 'matrices.{x}.cue.{y}', False),
        ('cue_output_level', 'MIXER:Current/Cue/Out/Level', ('cue', 2), None, ('level', 1000), 'cue.{x}.output_level', False),
        ('cue_output_on', 'MIXER:Current/Cue/Out/On', ('cue', 2), None, ('bool',), 'cue.{x}.output_on', False),
        ('cue_output_point', 'MIXER:Current/Cue/OutCh/Point', ('cue', 2), None, ('enum', ('PFL', 'AFL')), 'cue.{x}.output_point', False),
        ('stereo_cue_on', 'MIXER:Current/Cue/St/On', ('stereo', 4), ('cue', 2), ('bool',), 'stereo.{x}.cue.{y}', False),
        ('cue_surround_mode', 'MIXER:Current/Cue/Surr/CueMode', None, None, ('enum', ('MIX', 'LAST')), 'cue.surround_mode', False),
        ('dca_fader_level', 'MIXER:Current/DCA/Fader/Level', ('dca', 24), None, ('level', 1000), 'dcas.{x}.fader_level', False),
        ('dca_on', 'MIXER:Current/DCA/Fader/On', ('dca', 24), None, ('bool',), 'dcas.{x}.on', False),
        ('dca_name', 'MIXER:Current/DCA/Label/Name', ('dca', 24), None, ('name', 8), 'dcas.{x}.name', False),
        ('dca_color', 'MIXER:Current/DCA/Label/Color', ('dca', 24), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'LtGreen', 'White', 'OFF')), 'dcas.{x}.color', False),
        ('dca_icon', 'MIXER:Current/DCA/Label/Icon', ('dca', 24), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'DrumKit', 'Perc.', 'E.Bass', 'A.Guitar', 'E.Guitar', 'BassAmp', 'GuitarAmp', 'A.Bass', 'Strings', 'DirectBox', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Piano', 'Organ', 'Keyboard', 'Mallets', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'Foh', 'Speaker', 'Subwoofer', 'Wedge', 'Video', 'In-Ear', 'Monitor', 'Effector', 'Media1', 'Media2', 'Media3', 'Mixer', 'PC', 'Processor', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'dcas.{x}.icon', False),
        ('digital_out_patch', 'MIXER:Current/DigitalOutPort/Patch', ('console', 2), ('output', 8), ('text', 32, 'NONE, CUE A [L|R], CUE B [L|R], MONI A/B/AB [L|R|C], S CUE/S MON/S METER [LFE|Ls|Rs|L|R|C], TB [n], ST A [L|R], ST B [L|R], MIX[n], MTRX[n] or DIR OUT[n] (Table 4)'), 'consoles.{x}.digital_out.{y}.patch', False),
        ('omni_out_patch', 'MIXER:Current/OmniOutPort/Patch', ('console', 2), ('output', 8), ('text', 32, 'NONE, HY1-HY4 [n], DSP MY1 [n], CUE A/B [L|R], MONI A/B/AB [L|R|C], S CUE/S MON/S METER [...], TB [n], ST A/B [L|R], MIX[n], MTRX[n] or DIR OUT[n] (Table 9)'), 'consoles.{x}.omni_out.{y}.patch', False),
        ('input_dca_assign', 'MIXER:Current/InCh/DCA/Assign', ('channel', 288), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'inputs.{x}.dcas.{y}', False),
        ('input_delay_on', 'MIXER:Current/InCh/Delay/On', ('channel', 288), None, ('bool',), 'inputs.{x}.delay.on', False),
        ('input_delay_time', 'MIXER:Current/InCh/Delay/Time', ('channel', 288), None, ('int', 'time', 0, 1000000, 'thousandths of a ms, 0 to 1000 ms'), 'inputs.{x}.delay.time', False),
        ('input_dyn1_attack', 'MIXER:Current/InCh/Dyna1/Attack', ('channel', 288), None, ('int', 'attack', 0, 80000, 'ms for most types (0-120); thousandths of a ms for COMP260 (10-80000)'), 'inputs.{x}.dynamics.1.attack', False),
        ('input_dyn1_decay', 'MIXER:Current/InCh/Dyna1/Decay', ('channel', 288), None, ('int', 'decay', 3340, 42700000, 'thousandths of a ms, 3.34 ms to 42.7 s'), 'inputs.{x}.dynamics.1.decay', False),
        ('input_dyn1_deesser_type', 'MIXER:Current/InCh/Dyna1/DeEsserType', ('channel', 288), None, ('enum', ('BELL', 'HIGH SHELF')), 'inputs.{x}.dynamics.1.deesser_type', False),
        ('input_dyn1_freq', 'MIXER:Current/InCh/Dyna1/Freq', ('channel', 288), None, ('int', 'freq', 8000, 160000, 'tenths of a Hz, 800 Hz to 16 kHz'), 'inputs.{x}.dynamics.1.freq', False),
        ('input_dyn1_gain', 'MIXER:Current/InCh/Dyna1/Gain', ('channel', 288), None, ('int', 'gain', -200, 400, 'tenths of a dB, -20 to +40 dB'), 'inputs.{x}.dynamics.1.gain', False),
        ('input_dyn1_hold', 'MIXER:Current/InCh/Dyna1/Hold', ('channel', 288), None, ('int', 'hold', 20, 1960000, 'thousandths of a ms, 0.02 ms to 1.96 s'), 'inputs.{x}.dynamics.1.hold', False),
        ('input_dyn1_knee', 'MIXER:Current/InCh/Dyna1/Knee', ('channel', 288), None, ('enum', ('HARD', 'SOFT-1', 'SOFT-2', 'SOFT-3', 'SOFT-4', 'SOFT-5')), 'inputs.{x}.dynamics.1.knee', False),
        ('input_dyn1_on', 'MIXER:Current/InCh/Dyna1/On', ('channel', 288), None, ('bool',), 'inputs.{x}.dynamics.1.on', False),
        ('input_dyn1_q', 'MIXER:Current/InCh/Dyna1/Q', ('channel', 288), None, ('int', 'q', 500, 25000, 'thousandths, 0.5 to 25.0'), 'inputs.{x}.dynamics.1.q', False),
        ('input_dyn1_range', 'MIXER:Current/InCh/Dyna1/Range', ('channel', 288), None, ('int', 'range', -32768, 0, 'hundredths of a dB, -inf to 0 dB'), 'inputs.{x}.dynamics.1.range', False),
        ('input_dyn1_ratio', 'MIXER:Current/InCh/Dyna1/Ratio', ('channel', 288), None, ('int', 'ratio', 10, 65535, '10-255 in tenths for most types; 100-65535 in hundredths for COMP260 (Table 18)'), 'inputs.{x}.dynamics.1.ratio', False),
        ('input_dyn1_release', 'MIXER:Current/InCh/Dyna1/Release', ('channel', 288), None, ('int', 'release', 3340, 42700000, 'thousandths of a ms; COMP260 6.2 to 999 ms'), 'inputs.{x}.dynamics.1.release', False),
        ('input_dyn1_threshold', 'MIXER:Current/InCh/Dyna1/Threshold', ('channel', 288), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -60 to 0 dB; GATE and DUCKING to -72 dB'), 'inputs.{x}.dynamics.1.threshold', False),
        ('input_dyn1_type', 'MIXER:Current/InCh/Dyna1/Type', ('channel', 288), None, ('enum', ('PM Comp', 'Classic Comp', 'GATE', 'DE-ESSER', 'EXPANDER', 'DUCKING')), 'inputs.{x}.dynamics.1.type', False),
        ('input_dyn2_attack', 'MIXER:Current/InCh/Dyna2/Attack', ('channel', 288), None, ('int', 'attack', 0, 80000, 'ms for most types (0-120); thousandths of a ms for COMP260 (10-80000)'), 'inputs.{x}.dynamics.2.attack', False),
        ('input_dyn2_decay', 'MIXER:Current/InCh/Dyna2/Decay', ('channel', 288), None, ('int', 'decay', 3340, 42700000, 'thousandths of a ms, 3.34 ms to 42.7 s'), 'inputs.{x}.dynamics.2.decay', False),
        ('input_dyn2_deesser_type', 'MIXER:Current/InCh/Dyna2/DeEsserType', ('channel', 288), None, ('enum', ('BELL', 'HIGH SHELF')), 'inputs.{x}.dynamics.2.deesser_type', False),
        ('input_dyn2_freq', 'MIXER:Current/InCh/Dyna2/Freq', ('channel', 288), None, ('int', 'freq', 8000, 160000, 'tenths of a Hz, 800 Hz to 16 kHz'), 'inputs.{x}.dynamics.2.freq', False),
        ('input_dyn2_gain', 'MIXER:Current/InCh/Dyna2/Gain', ('channel', 288), None, ('int', 'gain', -200, 400, 'tenths of a dB, -20 to +40 dB'), 'inputs.{x}.dynamics.2.gain', False),
        ('input_dyn2_hold', 'MIXER:Current/InCh/Dyna2/Hold', ('channel', 288), None, ('int', 'hold', 20, 1960000, 'thousandths of a ms, 0.02 ms to 1.96 s'), 'inputs.{x}.dynamics.2.hold', False),
        ('input_dyn2_knee', 'MIXER:Current/InCh/Dyna2/Knee', ('channel', 288), None, ('enum', ('HARD', 'SOFT-1', 'SOFT-2', 'SOFT-3', 'SOFT-4', 'SOFT-5')), 'inputs.{x}.dynamics.2.knee', False),
        ('input_dyn2_on', 'MIXER:Current/InCh/Dyna2/On', ('channel', 288), None, ('bool',), 'inputs.{x}.dynamics.2.on', False),
        ('input_dyn2_q', 'MIXER:Current/InCh/Dyna2/Q', ('channel', 288), None, ('int', 'q', 500, 25000, 'thousandths, 0.5 to 25.0'), 'inputs.{x}.dynamics.2.q', False),
        ('input_dyn2_range', 'MIXER:Current/InCh/Dyna2/Range', ('channel', 288), None, ('int', 'range', -32768, 0, 'hundredths of a dB, -inf to 0 dB'), 'inputs.{x}.dynamics.2.range', False),
        ('input_dyn2_ratio', 'MIXER:Current/InCh/Dyna2/Ratio', ('channel', 288), None, ('int', 'ratio', 10, 65535, '10-255 in tenths for most types; 100-65535 in hundredths for COMP260 (Table 18)'), 'inputs.{x}.dynamics.2.ratio', False),
        ('input_dyn2_release', 'MIXER:Current/InCh/Dyna2/Release', ('channel', 288), None, ('int', 'release', 3340, 42700000, 'thousandths of a ms; COMP260 6.2 to 999 ms'), 'inputs.{x}.dynamics.2.release', False),
        ('input_dyn2_threshold', 'MIXER:Current/InCh/Dyna2/Threshold', ('channel', 288), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -60 to 0 dB; GATE and DUCKING to -72 dB'), 'inputs.{x}.dynamics.2.threshold', False),
        ('input_dyn2_type', 'MIXER:Current/InCh/Dyna2/Type', ('channel', 288), None, ('enum', ('PM Comp', 'Classic Comp', 'GATE', 'DE-ESSER', 'EXPANDER', 'DUCKING')), 'inputs.{x}.dynamics.2.type', False),
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 288), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 288), None, ('bool',), 'inputs.{x}.on', False),
        ('input_hpf_on', 'MIXER:Current/InCh/HPF/On', ('channel', 288), ('bank', 2), ('bool',), 'inputs.{x}.hpf.banks.{y}.on', False),
        ('input_hpf_freq', 'MIXER:Current/InCh/HPF/Freq', ('channel', 288), ('bank', 2), ('int', 'freq', 200, 20000, 'tenths of a Hz'), 'inputs.{x}.hpf.banks.{y}.freq', False),
        ('input_hpf_slope', 'MIXER:Current/InCh/HPF/Slope', ('channel', 288), ('bank', 2), ('int', 'slope', 6, 24, 'dB/oct'), 'inputs.{x}.hpf.banks.{y}.slope', False),
        ('input_lpf_on', 'MIXER:Current/InCh/LPF/On', ('channel', 288), ('bank', 2), ('bool',), 'inputs.{x}.lpf.banks.{y}.on', False),
        ('input_lpf_freq', 'MIXER:Current/InCh/LPF/Freq', ('channel', 288), ('bank', 2), ('int', 'freq', 200, 200000, 'tenths of a Hz, 20 Hz to 20 kHz'), 'inputs.{x}.lpf.banks.{y}.freq', False),
        ('input_lpf_slope', 'MIXER:Current/InCh/LPF/Slope', ('channel', 288), ('bank', 2), ('int', 'slope', 6, 12, 'dB/oct'), 'inputs.{x}.lpf.banks.{y}.slope', False),
        ('input_name', 'MIXER:Current/InCh/Label/Name', ('channel', 288), None, ('name', 8), 'inputs.{x}.name', False),
        ('input_color', 'MIXER:Current/InCh/Label/Color', ('channel', 288), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'LtGreen', 'White', 'OFF')), 'inputs.{x}.color', False),
        ('input_icon', 'MIXER:Current/InCh/Label/Icon', ('channel', 288), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'DrumKit', 'Perc.', 'E.Bass', 'A.Guitar', 'E.Guitar', 'BassAmp', 'GuitarAmp', 'A.Bass', 'Strings', 'DirectBox', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Piano', 'Organ', 'Keyboard', 'Mallets', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'Foh', 'Speaker', 'Subwoofer', 'Wedge', 'Video', 'In-Ear', 'Monitor', 'Effector', 'Media1', 'Media2', 'Media3', 'Mixer', 'PC', 'Processor', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'inputs.{x}.icon', False),
        ('input_pan_mode', 'MIXER:Current/InCh/PanMode', ('channel', 288), None, ('enum', ('PAN', 'BALANCE')), 'inputs.{x}.pan_mode', False),
        ('input_patch', 'MIXER:Current/InCh/Patch', ('channel', 288), ('bank', 2), ('text', 32, 'for example CS1 MY1 [n], DSP MY2 [n], CS1 OMNI[n], CS1 AES[n], CS1 PB [L|R], ST A [L|R], MIX[n], MTRX[n] (Table 8)'), 'inputs.{x}.patch.{y}', False),
        ('input_patch_select', 'MIXER:Current/InCh/PatchSelect', ('channel', 288), None, ('int', 'patch', 0, 1, '0 patch A, 1 patch B'), 'inputs.{x}.patch_select', False),
        ('input_eq_band_bypass', 'MIXER:Current/InCh/PEQ/Band/Bypass', ('channel', 288), ('band', 4), ('flag', 'bypassed', '1 bypassed'), 'inputs.{x}.eq.bands.{y}.bypass', False),
        ('input_eq_band_freq', 'MIXER:Current/InCh/PEQ/Band/Freq', ('channel', 288), ('band', 4), ('int', 'freq', 200, 200000, 'tenths of a Hz, 20 Hz to 20 kHz'), 'inputs.{x}.eq.bands.{y}.freq', False),
        ('input_eq_band_gain', 'MIXER:Current/InCh/PEQ/Band/Gain', ('channel', 288), ('band', 4), ('int', 'gain', -1800, 1800, 'hundredths of a dB, -18 to +18 dB'), 'inputs.{x}.eq.bands.{y}.gain', False),
        ('input_eq_band_q', 'MIXER:Current/InCh/PEQ/Band/Q', ('channel', 288), ('band', 4), ('int', 'q', 100, 16000, 'thousandths, 0.1 to 16.0'), 'inputs.{x}.eq.bands.{y}.q', False),
        ('input_eq_on', 'MIXER:Current/InCh/PEQ/On', ('channel', 288), ('bank', 2), ('bool',), 'inputs.{x}.eq.banks.{y}.on', False),
        ('input_eq_type', 'MIXER:Current/InCh/PEQ/Type', ('channel', 288), ('bank', 2), ('enum', ('PRECISE', 'AGGRESSIVE', 'SMOOTH', 'LEGACY')), 'inputs.{x}.eq.banks.{y}.type', False),
        ('input_ha_gain', 'MIXER:Current/InCh/Port/HA/Gain', ('channel', 288), None, ('int', 'gain', -600, 6600, 'hundredths of a dB, -6 to +66 dB'), 'inputs.{x}.ha_gain', False),
        ('input_role', 'MIXER:Current/InCh/Role', ('channel', 288), None, ('text', 16, 'Mono, StereoL, StereoR, SurrL, SurrR, SurrC, SurrLFE, SurrLs or SurrRs (Tables 10-13)'), 'inputs.{x}.role', False),
        ('input_surround_lr_pan', 'MIXER:Current/InCh/Surr/LRPan', ('channel', 288), None, ('pan',), 'inputs.{x}.surround.lr_pan', False),
        ('input_surround_fr_pan', 'MIXER:Current/InCh/Surr/FRPan', ('channel', 288), None, ('int', 'pan', -63, 63, '-63 to 63 (the document labels the ends R63 and F63)'), 'inputs.{x}.surround.fr_pan', False),
        ('input_surround_divergence', 'MIXER:Current/InCh/Surr/Div', ('channel', 288), None, ('int', 'divergence', 0, 100, '0 to 100'), 'inputs.{x}.surround.divergence', False),
        ('input_surround_l_on', 'MIXER:Current/InCh/Surr/LOn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.l_on', False),
        ('input_surround_r_on', 'MIXER:Current/InCh/Surr/ROn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.r_on', False),
        ('input_surround_c_on', 'MIXER:Current/InCh/Surr/COn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.c_on', False),
        ('input_surround_lfe_on', 'MIXER:Current/InCh/Surr/LFEOn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.lfe_on', False),
        ('input_surround_ls_on', 'MIXER:Current/InCh/Surr/LsOn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.ls_on', False),
        ('input_surround_rs_on', 'MIXER:Current/InCh/Surr/RsOn', ('channel', 288), None, ('bool',), 'inputs.{x}.surround.rs_on', False),
        ('input_surround_lfe_level', 'MIXER:Current/InCh/Surr/LFELevel', ('channel', 288), None, ('level', 1000), 'inputs.{x}.surround.lfe_level', False),
        ('input_mix_send_level', 'MIXER:Current/InCh/ToMix/Level', ('channel', 288), ('mix', 72), ('level', 1000), 'inputs.{x}.mix_sends.{y}.level', False),
        ('input_mix_send_on', 'MIXER:Current/InCh/ToMix/On', ('channel', 288), ('mix', 72), ('bool',), 'inputs.{x}.mix_sends.{y}.on', False),
        ('input_mix_send_pan', 'MIXER:Current/InCh/ToMix/Pan', ('channel', 288), ('mix', 72), ('pan',), 'inputs.{x}.mix_sends.{y}.pan', False),
        ('input_mix_send_pre', 'MIXER:Current/InCh/ToMix/PrePost', ('channel', 288), ('mix', 72), ('flag', 'pre', '1 Pre, 0 Post'), 'inputs.{x}.mix_sends.{y}.pre', False),
        ('input_matrix_send_level', 'MIXER:Current/InCh/ToMtrx/Level', ('channel', 288), ('matrix', 36), ('level', 1000), 'inputs.{x}.matrix_sends.{y}.level', False),
        ('input_matrix_send_on', 'MIXER:Current/InCh/ToMtrx/On', ('channel', 288), ('matrix', 36), ('bool',), 'inputs.{x}.matrix_sends.{y}.on', False),
        ('input_matrix_send_pan', 'MIXER:Current/InCh/ToMtrx/Pan', ('channel', 288), ('matrix', 36), ('pan',), 'inputs.{x}.matrix_sends.{y}.pan', False),
        ('input_matrix_send_pre', 'MIXER:Current/InCh/ToMtrx/PrePost', ('channel', 288), ('matrix', 36), ('flag', 'pre', '1 Pre, 0 Post'), 'inputs.{x}.matrix_sends.{y}.pre', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 288), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('input_insert_on', 'MIXER:Current/InCh/Insert/On', ('channel', 288), ('insert', 2), ('bool',), 'inputs.{x}.inserts.{y}.on', False),
        ('input_link_group', 'MIXER:Current/InputChLink/InCh/Assign', ('channel', 288), None, ('int', 'group', 0, 52, '0 none, 1 A ... 26 Z, 27 a ... 52 z'), 'inputs.{x}.link_group', False),
        ('input_link_channel_on', 'MIXER:Current/InputChLink/LinkParams/ChOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.channel_on', False),
        ('input_link_dca', 'MIXER:Current/InputChLink/LinkParams/DCA', ('link_group', 52), None, ('bool',), 'input_links.{x}.dca', False),
        ('input_link_delay', 'MIXER:Current/InputChLink/LinkParams/Delay', ('link_group', 52), None, ('bool',), 'input_links.{x}.delay', False),
        ('input_link_digital_gain', 'MIXER:Current/InputChLink/LinkParams/DigitalGain', ('link_group', 52), None, ('bool',), 'input_links.{x}.digital_gain', False),
        ('input_link_direct_out', 'MIXER:Current/InputChLink/LinkParams/DirectOut', ('link_group', 52), None, ('bool',), 'input_links.{x}.direct_out', False),
        ('input_link_dynamics1', 'MIXER:Current/InputChLink/LinkParams/Dyna1', ('link_group', 52), None, ('bool',), 'input_links.{x}.dynamics1', False),
        ('input_link_dynamics2', 'MIXER:Current/InputChLink/LinkParams/Dyna2', ('link_group', 52), None, ('bool',), 'input_links.{x}.dynamics2', False),
        ('input_link_eq', 'MIXER:Current/InputChLink/LinkParams/EQ', ('link_group', 52), None, ('bool',), 'input_links.{x}.eq', False),
        ('input_link_fader', 'MIXER:Current/InputChLink/LinkParams/Fader', ('link_group', 52), None, ('bool',), 'input_links.{x}.fader', False),
        ('input_link_ha', 'MIXER:Current/InputChLink/LinkParams/HA', ('link_group', 52), None, ('bool',), 'input_links.{x}.ha', False),
        ('input_link_hpf', 'MIXER:Current/InputChLink/LinkParams/HPF', ('link_group', 52), None, ('bool',), 'input_links.{x}.hpf', False),
        ('input_link_input_patch', 'MIXER:Current/InputChLink/LinkParams/InPatch', ('link_group', 52), None, ('bool',), 'input_links.{x}.input_patch', False),
        ('input_link_insert', 'MIXER:Current/InputChLink/LinkParams/Insert', ('link_group', 52), ('insert', 2), ('bool',), 'input_links.{x}.insert.{y}', False),
        ('input_link_matrix_send_alt', 'MIXER:Current/InputChLink/LinkParams/MatrixSend', ('link_group', 52), None, ('bool',), 'input_links.{x}.matrix_send_alt', False),
        ('input_link_mix_send', 'MIXER:Current/InputChLink/LinkParams/MixSend', ('link_group', 52), None, ('bool',), 'input_links.{x}.mix_send', False),
        ('input_link_mix_send_on', 'MIXER:Current/InputChLink/LinkParams/MixSendOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.mix_send_on', False),
        ('input_link_matrix_send', 'MIXER:Current/InputChLink/LinkParams/MtrxSend', ('link_group', 52), None, ('bool',), 'input_links.{x}.matrix_send', False),
        ('input_link_matrix_send_on', 'MIXER:Current/InputChLink/LinkParams/MtrxSendOn', ('link_group', 52), None, ('bool',), 'input_links.{x}.matrix_send_on', False),
        ('input_link_mute', 'MIXER:Current/InputChLink/LinkParams/Mute', ('link_group', 52), None, ('bool',), 'input_links.{x}.mute', False),
        ('input_link_silk', 'MIXER:Current/InputChLink/LinkParams/Silk', ('link_group', 52), None, ('bool',), 'input_links.{x}.silk', False),
        ('input_link_to_stereo', 'MIXER:Current/InputChLink/LinkParams/ToSt', ('link_group', 52), None, ('bool',), 'input_links.{x}.to_stereo', False),
        ('input_link_send_to_mix', 'MIXER:Current/InputChLink/SendParams/ToMix', ('link_group', 52), ('mix', 72), ('bool',), 'input_links.{x}.send_to_mix.{y}', False),
        ('input_link_send_to_matrix', 'MIXER:Current/InputChLink/SendParams/ToMtrx', ('link_group', 52), ('matrix', 36), ('bool',), 'input_links.{x}.send_to_matrix.{y}', False),
        ('mix_bus_type', 'MIXER:Current/Mix/BusType', ('mix', 72), None, ('enum', ('VARI', 'FIXED', 'MIXMINUS')), 'mixes.{x}.bus_type', False),
        ('mix_dca_assign', 'MIXER:Current/Mix/DCA/Assign', ('mix', 72), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'mixes.{x}.dcas.{y}', False),
        ('mix_delay_on', 'MIXER:Current/Mix/Delay/On', ('mix', 72), None, ('bool',), 'mixes.{x}.delay.on', False),
        ('mix_delay_time', 'MIXER:Current/Mix/Delay/Time', ('mix', 72), None, ('int', 'time', 0, 1000000, 'thousandths of a ms, 0 to 1000 ms'), 'mixes.{x}.delay.time', False),
        ('mix_dyn1_threshold', 'MIXER:Current/Mix/Dyna1/Threshold', ('mix', 72), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -60 to 0 dB; GATE and DUCKING to -72 dB'), 'mixes.{x}.dynamics.1.threshold', False),
        ('mix_dyn1_type', 'MIXER:Current/Mix/Dyna1/Type', ('mix', 72), ('bank', 2), ('enum', ('PM Comp', 'Classic Comp', 'GATE', 'DE-ESSER', 'EXPANDER', 'DUCKING')), 'mixes.{x}.dynamics.1.banks.{y}.type', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 72), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 72), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_name', 'MIXER:Current/Mix/Label/Name', ('mix', 72), None, ('name', 8), 'mixes.{x}.name', False),
        ('mix_color', 'MIXER:Current/Mix/Label/Color', ('mix', 72), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'LtGreen', 'White', 'OFF')), 'mixes.{x}.color', False),
        ('mix_icon', 'MIXER:Current/Mix/Label/Icon', ('mix', 72), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'DrumKit', 'Perc.', 'E.Bass', 'A.Guitar', 'E.Guitar', 'BassAmp', 'GuitarAmp', 'A.Bass', 'Strings', 'DirectBox', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Piano', 'Organ', 'Keyboard', 'Mallets', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'Foh', 'Speaker', 'Subwoofer', 'Wedge', 'Video', 'In-Ear', 'Monitor', 'Effector', 'Media1', 'Media2', 'Media3', 'Mixer', 'PC', 'Processor', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'mixes.{x}.icon', False),
        ('mix_minus_owner', 'MIXER:Current/Mix/MixMinus/Owner/InputChannel', ('mix', 72), ('channel', 288), ('flag', 'owner', '1 the input channel owns the mix-minus'), 'mixes.{x}.mix_minus_owner.{y}', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 72), None, ('pan',), 'mixes.{x}.balance', False),
        ('mix_pan_link', 'MIXER:Current/Mix/PanLink', ('mix', 72), None, ('bool',), 'mixes.{x}.pan_link', False),
        ('mix_pan_mode', 'MIXER:Current/Mix/PanMode', ('mix', 72), None, ('enum', ('PAN', 'BALANCE')), 'mixes.{x}.pan_mode', False),
        ('mix_role', 'MIXER:Current/Mix/Role', ('mix', 72), None, ('text', 16, 'Mono, StereoL, StereoR, SurrL, SurrR, SurrC, SurrLFE, SurrLs or SurrRs (Tables 10-13)'), 'mixes.{x}.role', False),
        ('mix_matrix_send_level', 'MIXER:Current/Mix/ToMtrx/Level', ('mix', 72), ('matrix', 36), ('level', 1000), 'mixes.{x}.matrix_sends.{y}.level', False),
        ('mix_matrix_send_on', 'MIXER:Current/Mix/ToMtrx/On', ('mix', 72), ('matrix', 36), ('bool',), 'mixes.{x}.matrix_sends.{y}.on', False),
        ('mix_matrix_send_pan', 'MIXER:Current/Mix/ToMtrx/Pan', ('mix', 72), ('matrix', 36), ('pan',), 'mixes.{x}.matrix_sends.{y}.pan', False),
        ('mix_matrix_send_pre', 'MIXER:Current/Mix/ToMtrx/PrePost', ('mix', 72), ('matrix', 36), ('flag', 'pre', '1 Pre, 0 Post'), 'mixes.{x}.matrix_sends.{y}.pre', False),
        ('mix_stereo_pan', 'MIXER:Current/Mix/ToSt/Pan', ('mix', 72), None, ('pan',), 'mixes.{x}.stereo_pan', False),
        ('mix_insert_on', 'MIXER:Current/Mix/Insert/On', ('mix', 72), ('insert', 2), ('bool',), 'mixes.{x}.inserts.{y}.on', False),
        ('mix_link_group', 'MIXER:Current/OutputChLink/Mix/Assign', ('mix', 72), None, ('int', 'group', 0, 52, '0 none, 1 A ... 52 z (Table 14)'), 'mixes.{x}.link_group', False),
        ('monitor_cue_interruption', 'MIXER:Current/Monitor/CueInterruption', ('monitor', 2), None, ('bool',), 'monitors.{x}.cue_interruption', False),
        ('monitor_dimmer_on', 'MIXER:Current/Monitor/DimmerOn', None, None, ('bool',), 'monitor.dimmer', False),
        ('monitor_downmix_l_to_l_on', 'MIXER:Current/Monitor/DownMix/LToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.l_to_l_on', False),
        ('monitor_downmix_l_to_r_on', 'MIXER:Current/Monitor/DownMix/LToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.l_to_r_on', False),
        ('monitor_downmix_l_to_lr_level', 'MIXER:Current/Monitor/DownMix/LToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.l_to_lr_level', False),
        ('monitor_downmix_r_to_l_on', 'MIXER:Current/Monitor/DownMix/RToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.r_to_l_on', False),
        ('monitor_downmix_r_to_r_on', 'MIXER:Current/Monitor/DownMix/RToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.r_to_r_on', False),
        ('monitor_downmix_r_to_lr_level', 'MIXER:Current/Monitor/DownMix/RToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.r_to_lr_level', False),
        ('monitor_downmix_c_to_l_on', 'MIXER:Current/Monitor/DownMix/CToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.c_to_l_on', False),
        ('monitor_downmix_c_to_r_on', 'MIXER:Current/Monitor/DownMix/CToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.c_to_r_on', False),
        ('monitor_downmix_c_to_lr_level', 'MIXER:Current/Monitor/DownMix/CToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.c_to_lr_level', False),
        ('monitor_downmix_lfe_to_l_on', 'MIXER:Current/Monitor/DownMix/LFEToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.lfe_to_l_on', False),
        ('monitor_downmix_lfe_to_r_on', 'MIXER:Current/Monitor/DownMix/LFEToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.lfe_to_r_on', False),
        ('monitor_downmix_lfe_to_lr_level', 'MIXER:Current/Monitor/DownMix/LFEToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.lfe_to_lr_level', False),
        ('monitor_downmix_ls_to_l_on', 'MIXER:Current/Monitor/DownMix/LsToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.ls_to_l_on', False),
        ('monitor_downmix_ls_to_r_on', 'MIXER:Current/Monitor/DownMix/LsToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.ls_to_r_on', False),
        ('monitor_downmix_ls_to_lr_level', 'MIXER:Current/Monitor/DownMix/LsToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.ls_to_lr_level', False),
        ('monitor_downmix_rs_to_l_on', 'MIXER:Current/Monitor/DownMix/RsToLOn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.rs_to_l_on', False),
        ('monitor_downmix_rs_to_r_on', 'MIXER:Current/Monitor/DownMix/RsToROn', ('surround', 2), None, ('bool',), 'monitor.downmix.{x}.rs_to_r_on', False),
        ('monitor_downmix_rs_to_lr_level', 'MIXER:Current/Monitor/DownMix/RsToLRLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.rs_to_lr_level', False),
        ('monitor_downmix_master_level', 'MIXER:Current/Monitor/DownMix/MasterLevel', ('surround', 2), None, ('level', 0), 'monitor.downmix.{x}.master_level', False),
        ('monitor_downmix_type', 'MIXER:Current/Monitor/DownMix/Type', None, None, ('enum', ('Stereo', 'Mono')), 'monitor.downmix_type', False),
        ('monitor_fader_level', 'MIXER:Current/Monitor/Fader/Level', ('monitor', 2), None, ('level', 1000), 'monitors.{x}.fader_level', False),
        ('monitor_on', 'MIXER:Current/Monitor/On', ('monitor', 2), None, ('bool',), 'monitors.{x}.on', False),
        ('monitor_phones_level_link', 'MIXER:Current/Monitor/PhonesLevelLink', ('phones', 2), None, ('bool',), 'monitors.{x}.phones_level_link', False),
        ('monitor_source', 'MIXER:Current/Monitor/St/SourceSelect', ('monitor', 2), None, ('int', 'source', 0, 7, 'define group 1-8 (the document gives 0-7)'), 'monitors.{x}.source', False),
        ('monitor_surround_c_mute', 'MIXER:Current/Monitor/Surr/COn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.c_mute', False),
        ('monitor_surround_lfe_mute', 'MIXER:Current/Monitor/Surr/LFEOn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.lfe_mute', False),
        ('monitor_surround_l_mute', 'MIXER:Current/Monitor/Surr/LOn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.l_mute', False),
        ('monitor_surround_ls_mute', 'MIXER:Current/Monitor/Surr/LsOn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.ls_mute', False),
        ('monitor_surround_r_mute', 'MIXER:Current/Monitor/Surr/ROn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.r_mute', False),
        ('monitor_surround_rs_mute', 'MIXER:Current/Monitor/Surr/RsOn', None, None, ('flag', 'muted', '0 ON, 1 MUTE as the document states'), 'monitor.surround.rs_mute', False),
        ('monitor_surround_solo_mode', 'MIXER:Current/Monitor/Surr/SoloMode', None, None, ('bool',), 'monitor.surround.solo_mode', False),
        ('monitor_surround_source', 'MIXER:Current/Monitor/Surr/SourceSelect', None, None, ('enum', ('Surr A', 'Surr B', 'DownMix A', 'DownMix B', 'Ext Surr5.1 1', 'Ext Surr5.1 2', 'Ext Surr5.1 3', 'Ext Surr5.1 4', 'Ext St 1', 'Ext St 2', 'Ext St 3', 'Ext St 4', 'Define 1', 'Define 2', 'Define 3', 'Define 4', 'Define 5', 'Define 6', 'Define 7', 'Define 8')), 'monitor.surround.source', False),
        ('matrix_dca_assign', 'MIXER:Current/Mtrx/DCA/Assign', ('matrix', 36), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'matrices.{x}.dcas.{y}', False),
        ('matrix_delay_on', 'MIXER:Current/Mtrx/Delay/On', ('matrix', 36), None, ('bool',), 'matrices.{x}.delay.on', False),
        ('matrix_delay_time', 'MIXER:Current/Mtrx/Delay/Time', ('matrix', 36), None, ('int', 'time', 0, 1000000, 'thousandths of a ms, 0 to 1000 ms'), 'matrices.{x}.delay.time', False),
        ('matrix_dyn1_threshold', 'MIXER:Current/Mtrx/Dyna1/Threshold', ('matrix', 36), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -60 to 0 dB; GATE and DUCKING to -72 dB'), 'matrices.{x}.dynamics.1.threshold', False),
        ('matrix_dyn1_type', 'MIXER:Current/Mtrx/Dyna1/Type', ('matrix', 36), None, ('enum', ('PM Comp', 'Classic Comp', 'GATE', 'DE-ESSER', 'EXPANDER', 'DUCKING')), 'matrices.{x}.dynamics.1.type', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 36), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 36), None, ('bool',), 'matrices.{x}.on', False),
        ('matrix_name', 'MIXER:Current/Mtrx/Label/Name', ('matrix', 36), None, ('name', 8), 'matrices.{x}.name', False),
        ('matrix_color', 'MIXER:Current/Mtrx/Label/Color', ('matrix', 36), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'LtGreen', 'White', 'OFF')), 'matrices.{x}.color', False),
        ('matrix_icon', 'MIXER:Current/Mtrx/Label/Icon', ('matrix', 36), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'DrumKit', 'Perc.', 'E.Bass', 'A.Guitar', 'E.Guitar', 'BassAmp', 'GuitarAmp', 'A.Bass', 'Strings', 'DirectBox', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Piano', 'Organ', 'Keyboard', 'Mallets', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'Foh', 'Speaker', 'Subwoofer', 'Wedge', 'Video', 'In-Ear', 'Monitor', 'Effector', 'Media1', 'Media2', 'Media3', 'Mixer', 'PC', 'Processor', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'matrices.{x}.icon', False),
        ('matrix_balance', 'MIXER:Current/Mtrx/Out/Balance', ('matrix', 36), None, ('pan',), 'matrices.{x}.balance', False),
        ('matrix_pan_link', 'MIXER:Current/Mtrx/PanLink', ('matrix', 36), None, ('bool',), 'matrices.{x}.pan_link', False),
        ('matrix_role', 'MIXER:Current/Mtrx/Role', ('matrix', 36), None, ('text', 16, 'Mono, StereoL, StereoR, SurrL, SurrR, SurrC, SurrLFE, SurrLs or SurrRs (Tables 10-13)'), 'matrices.{x}.role', False),
        ('matrix_insert_on', 'MIXER:Current/Mtrx/Insert/On', ('matrix', 36), ('insert', 2), ('bool',), 'matrices.{x}.inserts.{y}.on', False),
        ('matrix_link_group', 'MIXER:Current/OutputChLink/Mtrx/Assign', ('matrix', 36), None, ('int', 'group', 0, 52, '0 none, 1 A ... 52 z (Table 14)'), 'matrices.{x}.link_group', False),
        ('mute_group_name', 'MIXER:Current/MuteMaster/Label/Name', ('mute_group', 12), None, ('name', 8), 'mute_groups.{x}.name', False),
        ('mute_group_on', 'MIXER:Current/MuteMaster/On', ('mute_group', 12), None, ('bool',), 'mute_groups.{x}.on', False),
        ('output_link_channel_on', 'MIXER:Current/OutputChLink/LinkParams/ChOn', ('link_group', 52), None, ('bool',), 'output_links.{x}.channel_on', False),
        ('output_link_dca', 'MIXER:Current/OutputChLink/LinkParams/DCA', ('link_group', 52), None, ('bool',), 'output_links.{x}.dca', False),
        ('output_link_delay', 'MIXER:Current/OutputChLink/LinkParams/Delay', ('link_group', 52), None, ('bool',), 'output_links.{x}.delay', False),
        ('output_link_dynamics1', 'MIXER:Current/OutputChLink/LinkParams/Dyna1', ('link_group', 52), None, ('bool',), 'output_links.{x}.dynamics1', False),
        ('output_link_eq', 'MIXER:Current/OutputChLink/LinkParams/EQ', ('link_group', 52), None, ('bool',), 'output_links.{x}.eq', False),
        ('output_link_fader', 'MIXER:Current/OutputChLink/LinkParams/Fader', ('link_group', 52), None, ('bool',), 'output_links.{x}.fader', False),
        ('output_link_filters', 'MIXER:Current/OutputChLink/LinkParams/Filters', ('link_group', 52), None, ('bool',), 'output_links.{x}.filters', False),
        ('output_link_insert', 'MIXER:Current/OutputChLink/LinkParams/Insert', ('link_group', 52), ('insert', 2), ('bool',), 'output_links.{x}.insert.{y}', False),
        ('output_link_matrix_send_alt', 'MIXER:Current/OutputChLink/LinkParams/MatrixSend', ('link_group', 52), None, ('bool',), 'output_links.{x}.matrix_send_alt', False),
        ('output_link_matrix_send', 'MIXER:Current/OutputChLink/LinkParams/MtrxSend', ('link_group', 52), None, ('bool',), 'output_links.{x}.matrix_send', False),
        ('output_link_matrix_send_on', 'MIXER:Current/OutputChLink/LinkParams/MtrxSendOn', ('link_group', 52), None, ('bool',), 'output_links.{x}.matrix_send_on', False),
        ('output_link_mute', 'MIXER:Current/OutputChLink/LinkParams/Mute', ('link_group', 52), None, ('bool',), 'output_links.{x}.mute', False),
        ('output_link_to_stereo', 'MIXER:Current/OutputChLink/LinkParams/ToSt', ('link_group', 52), None, ('bool',), 'output_links.{x}.to_stereo', False),
        ('output_link_send_to_matrix', 'MIXER:Current/OutputChLink/SendParams/ToMtrx', ('link_group', 52), ('matrix', 36), ('bool',), 'output_links.{x}.send_to_matrix.{y}', False),
        ('stereo_dca_assign', 'MIXER:Current/St/DCA/Assign', ('stereo', 4), ('dca', 24), ('flag', 'assigned', '1 assigned'), 'stereo.{x}.dcas.{y}', False),
        ('stereo_delay_on', 'MIXER:Current/St/Delay/On', ('stereo', 4), None, ('bool',), 'stereo.{x}.delay.on', False),
        ('stereo_delay_time', 'MIXER:Current/St/Delay/Time', ('stereo', 4), None, ('int', 'time', 0, 1000000, 'thousandths of a ms, 0 to 1000 ms'), 'stereo.{x}.delay.time', False),
        ('stereo_dyn1_threshold', 'MIXER:Current/St/Dyna1/Threshold', ('stereo', 4), None, ('int', 'threshold', -720, 0, 'tenths of a dB, -60 to 0 dB; GATE and DUCKING to -72 dB'), 'stereo.{x}.dynamics.1.threshold', False),
        ('stereo_dyn1_type', 'MIXER:Current/St/Dyna1/Type', ('stereo', 4), None, ('enum', ('PM Comp', 'Classic Comp', 'GATE', 'DE-ESSER', 'EXPANDER', 'DUCKING')), 'stereo.{x}.dynamics.1.type', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 4), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 4), None, ('bool',), 'stereo.{x}.on', False),
        ('stereo_name', 'MIXER:Current/St/Label/Name', ('stereo', 4), None, ('name', 8), 'stereo.{x}.name', False),
        ('stereo_color', 'MIXER:Current/St/Label/Color', ('stereo', 4), None, ('enum', ('Blue', 'Orange', 'Yellow', 'Purple', 'SkyBlue', 'Pink', 'Red', 'Green', 'LtGreen', 'White', 'OFF')), 'stereo.{x}.color', False),
        ('stereo_icon', 'MIXER:Current/St/Label/Icon', ('stereo', 4), None, ('enum', ('Kick', 'Snare', 'Hi-Hat', 'TomTom', 'FloorTom', 'Cymbal', 'DrumKit', 'Perc.', 'E.Bass', 'A.Guitar', 'E.Guitar', 'BassAmp', 'GuitarAmp', 'A.Bass', 'Strings', 'DirectBox', 'Trumpet', 'Trombone', 'Saxophone', 'Flute', 'Piano', 'Organ', 'Keyboard', 'Mallets', 'Male', 'Female', 'Choir', 'DynamicMic', 'CondenserMic', 'InstMic', 'WirelessMic', 'Headset', 'SpeechMic', 'Foh', 'Speaker', 'Subwoofer', 'Wedge', 'Video', 'In-Ear', 'Monitor', 'Effector', 'Media1', 'Media2', 'Media3', 'Mixer', 'PC', 'Processor', 'Audience', 'ArrowLeft', 'ArrowRight', 'Exclamation', 'Smile', 'Money', 'Star1', 'Star2', 'Blank')), 'stereo.{x}.icon', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 4), None, ('pan',), 'stereo.{x}.balance', False),
        ('stereo_role', 'MIXER:Current/St/Role', ('stereo', 4), None, ('text', 16, 'Mono, StereoL, StereoR, SurrL, SurrR, SurrC, SurrLFE, SurrLs or SurrRs (Tables 10-13)'), 'stereo.{x}.role', False),
        ('stereo_matrix_send_level', 'MIXER:Current/St/ToMtrx/Level', ('stereo', 4), ('matrix', 36), ('level', 1000), 'stereo.{x}.matrix_sends.{y}.level', False),
        ('stereo_matrix_send_on', 'MIXER:Current/St/ToMtrx/On', ('stereo', 4), ('matrix', 36), ('bool',), 'stereo.{x}.matrix_sends.{y}.on', False),
        ('stereo_matrix_send_pan', 'MIXER:Current/St/ToMtrx/Pan', ('stereo', 4), ('matrix', 36), ('pan',), 'stereo.{x}.matrix_sends.{y}.pan', False),
        ('stereo_matrix_send_pre', 'MIXER:Current/St/ToMtrx/PrePost', ('stereo', 4), ('matrix', 36), ('flag', 'pre', '1 Pre, 0 Post'), 'stereo.{x}.matrix_sends.{y}.pre', False),
        ('surround_mode', 'MIXER:Current/SurrMode', None, None, ('flag', 'surround', '0 stereo mode, 1 surround mode'), 'surround_mode', False),
        ('monitor_mix_pin', 'MIXER:Setup/MonitorMix/Password', None, None, ('text', 24, 'up to 24 characters'), None, True),
        ('oscillator_level', 'MIXER:Setup/Oscillator/Level', ('oscillator', 2), None, ('int', 'level', -9600, 0, 'hundredths of a dB, -96 to 0 dB'), 'oscillator.{x}.level', False),
        ('oscillator_on', 'MIXER:Setup/Oscillator/On', None, None, ('bool',), 'oscillator.on', False),
        ('oscillator_type', 'MIXER:Setup/Oscillator/Type', None, None, ('enum', ('SINE 1CH', 'SINE 2CH', 'PINK NOISE', 'BURST NOISE')), 'oscillator.type', False),
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

    done = set(_ypm_EXPLICIT)
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
    for args, kw in _ypm_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_ypm_EXPLICIT = {}


def _ypm_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _ypm_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (OSC spec 1.4, p.3), restated for the RCP wire.
_ypm_explicit("text", "set_input_fader_level", {"channel": 1, "level": -32768}, "set MIXER:Current/InCh/Fader/Level 0 0 -32768\n",
              device_reply="OK set MIXER:Current/InCh/Fader/Level 0 0 -32768 \"-Inf\"\n", expect_result={"ok": {"kind": "ack"}})
_ypm_explicit("text", "set_mix_color", {"mix": 1, "value": "Pink"}, 'set MIXER:Current/Mix/Label/Color 0 0 "Pink"\n',
              device_reply='OK set MIXER:Current/Mix/Label/Color 0 0 "Pink"\n', expect_result={"ok": {"kind": "ack"}})
_ypm_explicit("text", "recall_scene", {"scene": "5.00"}, 'ssrecallt_ex MIXER:Lib/Scene "5.00"\n',
              device_reply='OK ssrecallt_ex MIXER:Lib/Scene "5.00"\n', expect_result={"ok": {"kind": "ack"}})
_ypm_explicit("text", "get_current_scene", {}, "sscurrentt_ex MIXER:Lib/Scene\n",
              device_reply='OK sscurrentt_ex MIXER:Lib/Scene "5.00" unmodified\n', expect_result={"ok": {"kind": "value", "value": "5.00"}})
_ypm_explicit("text", "set_input_patch", {"channel": 3, "bank": 2, "value": "CS1 MY1 3"}, 'set MIXER:Current/InCh/Patch 2 1 "CS1 MY1 3"\n')
_ypm_explicit("text", "set_monitor_mix_pin", {"value": "1234"}, 'set MIXER:Setup/MonitorMix/Password 0 0 "1234"\n',
              device_reply="ERROR set ReadOnly\n", expect_result={"error": {"error": "device_error"}})
_ypm_explicit("telemetry", "scene-current", inbound='NOTIFY sscurrentt_ex MIXER:Lib/Scene "5.00"\n', expect_state={"scene": {"current": "5.00"}})
_ypm_explicit("telemetry", "scene-modified", inbound='OK sscurrentt_ex MIXER:Lib/Scene "5.00" modified\n',
              expect_state={"scene": {"current": "5.00", "modified": True}})
_ypm_explicit("telemetry", "scene-recalled", inbound='OK ssrecallt_ex MIXER:Lib/Scene "7.50"\n', expect_state={"scene": {"current": "7.50"}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_ypm_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_ypm_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_ypm_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_ypm_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "RIVAGE PM10"\n',
     expect_result={"ok": {"kind": "value", "value": "RIVAGE PM10"}})
_ypm_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_ypm_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_ypm_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_ypm_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "RIVAGE PM10"\n', expect_state={"device": {"product_name": "RIVAGE PM10"}})
_ypm_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_ypm_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_ypm_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})
_ypm_explicit("text", "get_error_status", {}, "devstatus error\n", device_reply='OK devstatus error "none"\n',
     expect_result={"ok": {"kind": "value", "value": "none"}})
_ypm_explicit("telemetry", "error-status", inbound='NOTIFY devstatus error "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"\n',
     expect_state={"device": {"error": "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"}})


_ypm_vectors()
_ypm_EXPLICIT.clear()
