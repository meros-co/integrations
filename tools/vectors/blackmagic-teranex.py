# Blackmagic Teranex: the Teranex Ethernet Protocol on TCP 9800, "Name: value"
# blocks ended by a blank line (Teranex Processors manual, June 2017,
# Developer Information p.101-117). The wire forms follow the manual's own
# examples: "VIDEO OUTPUT:\nVideo mode: 1080i5994" (p.101), the header-only
# status request "VIDEO OUTPUT:" (p.102), "PING:" (p.102), "MODE3D:\n3D mode:
# 3DModeAlign" (p.103), "ALIGN:\nAlign pos X left: -20" (p.103), "MODE3D:\n3D
# roll left: 20" (p.110), "Recall: 1" and "Save: 1" (p.112), and every other
# block takes the "Field: value" line its table gives. ACK is followed by a
# blank line (p.102-103).
TX = "blackmagic-teranex"
AV = {"model": "teranex-av"}
EXP = {"model": "teranex-express"}
D2 = {"model": "teranex-2d"}
D3 = {"model": "teranex-3d"}


def tx(command, input, header, line, **extra):
    text(TX, command, input, f"{header}:\n{line}\n\n", **extra)


# Device (p.113).
tx("set_panel_lock", {"value": True}, "TERANEX DEVICE", "Panel lock: true",
   device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
tx("set_remote_lock", {"value": False}, "TERANEX DEVICE", "Remote lock: false",
   device_reply="NACK\n\n", expect_result={"error": {"error": "device_error"}})

# Video input (p.115-116).
tx("set_video_source", {"value": "SDI2"}, "VIDEO INPUT", "Video source: SDI2", **AV)
tx("set_audio_source", {"value": "AES"}, "VIDEO INPUT", "Audio source: AES")
tx("set_wide_sd_aspect", {"value": True}, "VIDEO INPUT", "Wide SD aspect: true")

# Video output (p.101, p.116-117).
tx("set_output_video_mode", {"value": "1080i5994"}, "VIDEO OUTPUT", "Video mode: 1080i5994",
   device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
tx("set_aspect_ratio", {"value": "CentreCut"}, "VIDEO OUTPUT", "Aspect ratio: CentreCut")
tx("set_demux_mode", {"value": "QuadLink"}, "VIDEO OUTPUT", "Video demux mode: QuadLink", **EXP)
tx("set_output_sdi_mode", {"value": "LevelB"}, "VIDEO OUTPUT", "Output SDI mode: LevelB")
tx("set_output_pixel_format", {"value": "RGB444"}, "VIDEO OUTPUT", "Video pixel format: RGB444", **D3)
tx("set_analog_output", {"value": "Component"}, "VIDEO OUTPUT", "Analog output: Component")
tx("set_hdmi_output", {"value": "RGB444"}, "VIDEO OUTPUT", "HDMI Output: RGB444", **AV)
tx("set_output_option", {"value": "Freeze"}, "VIDEO OUTPUT", "Output option: Freeze", **AV)
tx("set_quad_ancillary_replication", {"value": True}, "VIDEO OUTPUT", "Quad ancillary replication: true")
tx("set_quad_sdi_output", {"value": "QuadHDSplit"}, "VIDEO OUTPUT", "Quad SDI output: QuadHDSplit", **AV)
tx("set_transition_setting", {"value": 50}, "VIDEO OUTPUT", "Transition setting: 50", **AV)

# Video adjust (p.114-115).
tx("set_adjust_red", {"value": -200}, "VIDEO ADJUST", "Red: -200")
tx("set_adjust_green", {"value": 25}, "VIDEO ADJUST", "Green: 25")
tx("set_adjust_blue", {"value": 200}, "VIDEO ADJUST", "Blue: 200")
tx("set_adjust_luma_low", {"value": 4}, "VIDEO ADJUST", "Luma low: 4")
tx("set_adjust_luma_high", {"value": 1019}, "VIDEO ADJUST", "Luma high: 1019")
tx("set_adjust_chroma_low", {"value": 16}, "VIDEO ADJUST", "Chroma low: 16")
tx("set_adjust_chroma_high", {"value": 1000}, "VIDEO ADJUST", "Chroma high: 1000")
tx("set_adjust_aspect_fill_luma", {"value": 64}, "VIDEO ADJUST", "Aspect fill luma: 64")
tx("set_adjust_aspect_fill_cb", {"value": 512}, "VIDEO ADJUST", "Aspect fill Cb: 512")
tx("set_adjust_aspect_fill_cr", {"value": 960}, "VIDEO ADJUST", "Aspect fill Cr: 960")

# Video proc amp (p.117).
tx("set_proc_amp_gain", {"value": -60}, "VIDEO PROC AMP", "Gain: -60")
tx("set_proc_amp_black", {"value": 30}, "VIDEO PROC AMP", "Black: 30")
tx("set_proc_amp_saturation", {"value": 10}, "VIDEO PROC AMP", "Saturation: 10")
tx("set_proc_amp_hue", {"value": -179}, "VIDEO PROC AMP", "Hue: -179")
tx("set_proc_amp_ry", {"value": 5}, "VIDEO PROC AMP", "RY: 5")
tx("set_proc_amp_by", {"value": -5}, "VIDEO PROC AMP", "BY: -5")
tx("set_proc_amp_sharp", {"value": 50}, "VIDEO PROC AMP", "Sharp: 50")

# Video advanced (p.115).
tx("set_clean_cadence", {"value": True}, "VIDEO ADVANCED", "Clean cadence: true")
tx("set_scenecut_detect", {"value": False}, "VIDEO ADVANCED", "Scenecut detect: false")
tx("set_source_type", {"value": "Film"}, "VIDEO ADVANCED", "Source type: Film")
tx("set_frc_aperture", {"value": 3}, "VIDEO ADVANCED", "FRC aperture: 3")
tx("set_processing", {"value": "Highest Quality"}, "VIDEO ADVANCED", "Processing: Highest Quality", **AV)

# Noise reduction (p.112).
tx("set_noise_reduction", {"value": True}, "NOISE REDUCTION", "Enabled: true")
tx("set_noise_reduction_bias", {"value": -3}, "NOISE REDUCTION", "Bias: -3")
tx("set_noise_reduction_split_screen", {"value": True}, "NOISE REDUCTION", "Split screen: true")
tx("set_noise_reduction_red_overlay", {"value": False}, "NOISE REDUCTION", "Red overlay: false")

# Genlock (p.109).
tx("set_genlock_reference", {"value": "TriLevel"}, "GENLOCK", "Gen reference: TriLevel", **AV)
tx("set_genlock_type", {"value": "External"}, "GENLOCK", "Type: External")
tx("set_genlock_line_offset", {"value": 1}, "GENLOCK", "Line offset: 1")
tx("set_genlock_pixel_offset", {"value": 2639}, "GENLOCK", "Pixel offset: 2639")

# Test pattern (p.113).
tx("set_test_pattern", {"value": "SMPTEBars"}, "TEST PATTERN", "Output: SMPTEBars")
tx("set_no_signal_output", {"value": "Bars"}, "TEST PATTERN", "No signal: Bars")
tx("set_test_tone", {"value": "Tone1500Hz"}, "TEST PATTERN", "Test tone: Tone1500Hz", **AV)
tx("set_test_pattern_motion", {"value": True}, "TEST PATTERN", "Motion: true", **AV)
tx("set_test_pattern_rate", {"value": -2}, "TEST PATTERN", "Horizontal rate: -2", **AV)

# Presets (p.112): "Recall: 1", "Save: 1"; PresetName0 is preset 1.
tx("recall_preset", {"preset": 1}, "PRESET", "Recall: 1", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
tx("save_preset", {"preset": 6}, "PRESET", "Save: 6")
tx("set_preset_name", {"preset": 1, "name": "Stage 4K"}, "PRESET", "PresetName0: Stage 4K")

# Variable aspect ratio (p.113-114).
tx("set_variable_aspect", {"control": "pos X left", "value": -20}, "VARIABLE ASPECT RATIO",
   "Variable Aspect Ratio pos X left: -20")
tx("set_zoom_crop", {"value": True}, "VARIABLE ASPECT RATIO", "Variable Aspect Ratio zoom/crop: true")

# Ancillary data (p.105-106).
tx("set_cc_enabled", {"value": True}, "ANCILLARY DATA", "CC enabled: true")
tx("set_cc_input_line", {"value": 21}, "ANCILLARY DATA", "CC input line: 21")
tx("set_cc_output_line", {"value": 22}, "ANCILLARY DATA", "CC output line: 22")
tx("set_cc_service2_source", {"value": "CC3"}, "ANCILLARY DATA", "CC service2 source: CC3")
tx("set_cc_service1_language", {"value": "French"}, "ANCILLARY DATA", "CC service1 language: French")
tx("set_cc_service2_language", {"value": "Spanish"}, "ANCILLARY DATA", "CC service2 language: Spanish")
tx("set_timecode_mode", {"value": "JamSync"}, "ANCILLARY DATA", "Timecode mode: JamSync")
tx("set_timecode_input_line", {"value": 0}, "ANCILLARY DATA", "Timecode input line: 0")
tx("set_timecode_output_line", {"value": 14}, "ANCILLARY DATA", "Timecode output line: 14")
tx("set_timecode_drop_frame", {"value": "NDF"}, "ANCILLARY DATA", "Timecode drop frame mode: NDF")
tx("set_timecode_source", {"value": "LTC"}, "ANCILLARY DATA", "Timecode source: LTC", **D3)
tx("set_timecode_generate_value", {"value": "10:00:00:00"}, "ANCILLARY DATA", "Timecode generate value: 10:00:00:00")
tx("set_timecode_jam_sync_value", {"value": "01:02:03:04"}, "ANCILLARY DATA", "Timecode jam sync value: 01:02:03:04")
tx("set_timecode_start_source", {"value": "User"}, "ANCILLARY DATA", "Timecode start source: User")
tx("set_index_reaction", {"value": "On"}, "ANCILLARY DATA", "Index reaction: On")
tx("set_afd_insert_type", {"value": "1010"}, "ANCILLARY DATA", "AFD insert type: 1010")
tx("set_afd_output_line", {"value": 11}, "ANCILLARY DATA", "AFD output line: 11")

# Audio (p.106-109); AudioInLevel0, AudioOut0, AudioEncode0 and AudioInPair0 are the first.
tx("set_aes_output", {"value": True}, "AUDIO", "AES output select: true", **D3)
tx("set_analog_input_ref_level", {"value": 8}, "AUDIO", "Analog input ref level: 8", **D2)
tx("set_audio_meter_channels", {"value": "MeterChan3&4"}, "AUDIO", "Audio meter channels: MeterChan3&4", **AV)
tx("set_audio_delay", {"value": -28}, "AUDIO", "AudioUserDelay0: -28", **D2)
tx("set_audio_gain_all", {"value": 40}, "AUDIO", "AudioInLevel0: 40")
tx("set_audio_gain", {"channel": 16, "gain": -320}, "AUDIO", "AudioInLevel15: -320", **D3)
tx("set_audio_output_source", {"output": 1, "source": "TT1500"}, "AUDIO", "AudioOut0: TT1500")
tx("set_audio_encoder_source", {"channel": 8, "source": "AudioIn16"}, "AUDIO", "AudioEncode7: AudioIn16", **D3)
tx("set_audio_input_pair", {"pair": 2, "source": "AESPair1"}, "AUDIO", "AudioInPair1: AESPair1", **D2)
tx("set_metadata_channel_mode", {"value": "20"}, "AUDIO", "Metadata channel mode: 20", **D3)
tx("set_metadata_lfe", {"value": True}, "AUDIO", "Metadata lfe select: true", **D3)

# Network config (p.111).
tx("set_friendly_name", {"value": "Teranex Truck A"}, "NETWORK CONFIG", "Friendly name: Teranex Truck A")
tx("set_dhcp", {"value": True}, "NETWORK CONFIG", "DHCP enabled: true")

# Teranex 3D align and 3D modes (p.103-104, p.110-111).
tx("set_align", {"control": "pos X left", "value": -20}, "ALIGN", "Align pos X left: -20", **D3)
tx("set_3d_mode", {"value": "3DModeAlign"}, "MODE3D", "3D mode: 3DModeAlign", **D3)
tx("set_2d3d_intensity", {"value": 15}, "MODE3D", "2D3D intensity: 15", **D3)
tx("set_2d3d_depth", {"value": -12}, "MODE3D", "2D3D depth: -12", **D3)
tx("set_3d_output", {"value": "3DSideBySide"}, "MODE3D", "3D output: 3DSideBySide", **D3)
tx("set_3d_input", {"value": "3DTopBottom"}, "MODE3D", "3D input: 3DTopBottom", **D3)
tx("set_3d_rotation", {"axis": "roll", "eye": "left", "value": 20}, "MODE3D", "3D roll left: 20", **D3)
tx("set_3d_flip", {"eye": "right", "flip": "Both"}, "MODE3D", "3D flip right: Both", **D3)

# Status request and PING (p.102): header alone, then a blank line.
text(TX, "request_status", {"block": "VIDEO OUTPUT"}, "VIDEO OUTPUT:\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(TX, "ping", {}, "PING:\n\n", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})


# ── Telemetry ──
telemetry(TX, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 3.18\n\n",
          expect_state={"device": {"protocol_version": "3.18"}})
telemetry(TX, "device", inbound="TERANEX DEVICE:\nModel name: Teranex AV\nSoftware Version: 1A2B3C4D\n"
          "FPGA Version: 12\nPanel lock: false\nRemote lock: true\n\n",
          expect_state={"device": {"model": "Teranex AV", "software_version": "1A2B3C4D", "fpga_version": "12",
                                   "panel_lock": False, "remote_lock": True}})
# The p.103 status dump example.
telemetry(TX, "video-output", inbound="VIDEO OUTPUT:\nVideo mode: 1080i5994\nAspect ratio: Anamorphic\n"
          "Video demux mode: SingleLink\nVideo pixel format: YCbCr422\nAnalog output: Component\n\n",
          expect_state={"video_output": {"video_mode": "1080i5994", "aspect_ratio": "Anamorphic",
                                         "demux_mode": "SingleLink", "pixel_format": "YCbCr422",
                                         "analog_output": "Component"}})
telemetry(TX, "video-input", inbound="VIDEO INPUT:\nVideo source: SDI1\nAudio source: Embedded\n"
          "Video mode: 2160p59.94\nSignal present: true\nTimecode present: Detected\nWide SD aspect: false\n\n",
          expect_state={"video_input": {"video_source": "SDI1", "audio_source": "Embedded",
                                        "video_mode": "2160p59.94", "signal_present": True,
                                        "timecode_present": "Detected", "wide_sd_aspect": False}})
# The p.102 example blocks following a change.
telemetry(TX, "video-adjust", inbound="VIDEO ADJUST:\nLuma low: 4\nLuma high: 1019\n\n",
          expect_state={"video_adjust": {"luma_low": 4, "luma_high": 1019}})
telemetry(TX, "ancillary", inbound="ANCILLARY DATA:\nAFD output line: 11\nCC enabled: false\n\n",
          expect_state={"ancillary": {"afd_output_line": 11, "cc_enabled": False}})
telemetry(TX, "variable-aspect", inbound="VARIABLE ASPECT RATIO:\nVariable Aspect Ratio size X left: 0.000000\n"
          "Variable Aspect Ratio size X right: 0.000000\n\n",
          expect_state={"variable_aspect": {"size_x_left": 0.0, "size_x_right": 0.0}})
# Community-observed ON/OFF form (Companion module).
telemetry(TX, "noise-reduction", inbound="NOISE REDUCTION:\nEnabled: ON\nBias: 2\nSplit screen: OFF\n\n",
          expect_state={"noise_reduction": {"enabled": True, "bias": 2.0, "split_screen": False}})
telemetry(TX, "genlock", inbound="GENLOCK:\nGen reference: Blackburst\nLine offset: 1\nPixel offset: 0\n"
          "Signal locked: true\nType: External\n\n",
          expect_state={"genlock": {"reference": "Blackburst", "line_offset": 1, "pixel_offset": 0,
                                    "signal_locked": True, "type": "External"}})
telemetry(TX, "preset", inbound="PRESET:\nPresetName0: Stage 4K\nPresetName5: Spare\n\n",
          expect_state={"presets": {"1": {"name": "Stage 4K"}, "6": {"name": "Spare"}}})
telemetry(TX, "audio", inbound="AUDIO:\nAudioInLevel0: 40\nAudioOut0: AudioIn1\nAudioUserDelay0: 20\n\n",
          expect_state={"audio": {"inputs": {"1": {"level": 40}}, "outputs": {"1": {"source": "AudioIn1"}},
                                  "delay_ms": 20}})
# The p.103 align example.
telemetry(TX, "align", inbound="ALIGN:\nAlign pos X left: -20.000000\nAlign pos X right: 0.000000\n\n",
          expect_state={"align": {"pos_x_left": -20.0, "pos_x_right": 0.0}})
telemetry(TX, "mode3d", inbound="MODE3D:\n3D mode: 3DModeAlign\n3D roll left: 20\n3D flip right: Both\n\n",
          expect_state={"mode3d": {"mode": "3DModeAlign", "roll_left": 20, "flip_right": "Both"}})
