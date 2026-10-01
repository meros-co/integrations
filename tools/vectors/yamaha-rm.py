RM = "yamaha-rm"
# Yamaha RM series Remote Control Protocol (RM Series Remote Control Protocol
# Specifications V3.0.0): LF-terminated ASCII lines; replies "OK ...",
# "OKm ..." (clamped) or "ERROR <cmd> <code>"; pushes "NOTIFY ...". Examples
# below are the document's own where it gives one. Typed parameter commands
# number sub-addresses from 1 (the wire's X/Y are 0-based); the wire strings in
# the second half are written from the section 7 tables (AccessID, X/Y range,
# value range), one set and one get per parameter.
RCR = {"model": "rm-cr"}
RCG = {"model": "rm-cg"}
RTT = {"model": "rm-tt"}
RW16 = {"model": "rm-wap-16"}
OK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


# Device status and protocol mode (p.9-11)
text(RM, "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n', expect_result=val("normal"))
text(RM, "get_error_status", {}, "devstatus error\n", device_reply='OK devstatus error "fault"\n', expect_result=val("fault"))
text(RM, "set_encoding", {"encoding": "utf8"}, "scpmode encoding utf8\n", device_reply="OK scpmode encoding utf8\n", expect_result=OK)
text(RM, "set_value_type", {"value_type": "normalized"}, "scpmode valuetype normalized\n",
     device_reply="OK scpmode valuetype normalized\n", expect_result=OK)
text(RM, "set_normalized_resolution", {"resolution": 128}, "scpmode resolution 128\n",
     device_reply="OK scpmode resolution 128\n", expect_result=OK)
text(RM, "set_keepalive", {"interval_ms": 2000}, "scpmode keepalive 2000\n", device_reply="OK scpmode keepalive 2000\n", expect_result=OK)

# Product information (p.14-17)
text(RM, "get_protocol_version", {}, "devinfo protocolver\n", device_reply='OK devinfo protocolver "1.0.0"\n', expect_result=val("1.0.0"))
text(RM, "get_parameter_set_version", {}, "devinfo paramsetver\n", device_reply='OK devinfo paramsetver "RM:1.0.0"\n', expect_result=val("RM:1.0.0"))
text(RM, "get_firmware_version", {}, "devinfo version\n", device_reply='OK devinfo version "1.0.0"\n', expect_result=val("1.0.0"))
text(RM, "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "RM-CR"\n', expect_result=val("RM-CR"))
text(RM, "get_serial_number", {}, "devinfo serialno\n", device_reply='OK devinfo serialno "S7A001001"\n', expect_result=val("S7A001001"))
text(RM, "get_category", {}, "devinfo category\n", device_reply='OK devinfo category "processor"\n', expect_result=val("processor"))
text(RM, "get_device_id", {}, "devinfo deviceid\n", device_reply='OK devinfo deviceid "001"\n', expect_result=val("001"))
text(RM, "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "Y001-Yamaha-RM-CR-061281"\n',
     expect_result=val("Y001-Yamaha-RM-CR-061281"))
text(RM, "get_manufacturer", {}, "devinfo manufacturer\n", device_reply='OK devinfo manufacturer "Yamaha Corporation"\n',
     expect_result=val("Yamaha Corporation"))
text(RM, "get_parameter_count", {}, "prmnum\n", device_reply="OK prmnum 114\n", expect_result=val("114"))
text(RM, "get_meter_count", {}, "mtrnum\n", device_reply="OK mtrnum 16\n", expect_result=val("16"))
text(RM, "get_meter_info", {"index": 1}, "mtrinfo 1\n")
text(RM, "identify", {"seconds": 10}, "identify 10\n", device_reply="OK identify 10\n", expect_result=OK)

# Generic parameter access (p.11-13), the document's FarEnd fader examples
text(RM, "set_parameter", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0, "value": -7760},
     "set RM:FeIn_Fader/Ch/Level 0 0 -7760\n", device_reply='OK set RM:FeIn_Fader/Ch/Level 0 0 -7760 "-77.60"\n', expect_result=OK)
text(RM, "set_parameter_normalized", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0, "value": 35},
     "setn RM:FeIn_Fader/Ch/Level 0 0 35\n", device_reply='OK setn RM:FeIn_Fader/Ch/Level 0 0 35 "-77.60"\n', expect_result=OK)
text(RM, "set_parameter_text", {"address": "RM:FeIn_Fader/Ch/Level", "x": 1, "y": 0, "text": 'say "hi" \\'},
     'sett RM:FeIn_Fader/Ch/Level 1 0 "say \\"hi\\" \\\\"\n', **RCR)
text(RM, "set_parameter_relative", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0, "steps": -3},
     "setr RM:FeIn_Fader/Ch/Level 0 0 -3\n", device_reply="OKm setr RM:FeIn_Fader/Ch/Level 0 0 -32768\n", expect_result=OK, **RCR)
text(RM, "get_parameter", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0}, "get RM:FeIn_Fader/Ch/Level 0 0\n",
     device_reply="OK get RM:FeIn_Fader/Ch/Level 0 0 -7760\n", expect_result=val("-7760"))
text(RM, "get_parameter_normalized", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0}, "getn RM:FeIn_Fader/Ch/Level 0 0\n",
     device_reply="OK getn RM:FeIn_Fader/Ch/Level 0 0 35\n", expect_result=val("35"))
text(RM, "get_parameter_text", {"address": "RM:FeIn_Fader/Ch/Level", "x": 0, "y": 0}, "gett RM:FeIn_Fader/Ch/Level 0 0\n",
     device_reply='OK gett RM:FeIn_Fader/Ch/Level 0 0 "-77.60"\n', expect_result=val("-77.60"), **RCR)

# Meters (p.13-14, section 8)
text(RM, "start_meter", {"meter": "RM:FeInPostFader", "interval_ms": 1000}, "mtrstart RM:FeInPostFader 1000\n",
     device_reply="OK mtrstart RM:FeInPostFader\n", expect_result=OK)
text(RM, "stop_meter", {"meter": "RM:FeInPostFader"}, "mtrstop RM:FeInPostFader\n", device_reply="OK mtrstop RM:FeInPostFader\n",
     expect_result=OK)

# Snapshots (RM-CR, p.67)
text(RM, "get_current_snapshot", {}, "sscurrent_ex config\n", device_reply="OK sscurrent_ex config 1 modified\n",
     expect_result=val("1"), **RCR)
text(RM, "recall_snapshot", {"snapshot": 1}, "ssrecall_ex config 1\n", device_reply="OK ssrecall_ex config 1\n", expect_result=OK, **RCR)
text(RM, "get_snapshot_count", {}, "ssnum_ex config\n", device_reply="OK ssnum_ex config 11\n", expect_result=val("11"), **RCR)
text(RM, "get_snapshot_info", {"snapshot": 1}, "ssinfo_ex config 1\n", **RCR)

# Events (Event List, p.20-21)
text(RM, "firmware_update", {"source": "tftp://192.168.0.10/rm.bin"}, 'event RM:FirmwareUpdate "tftp://192.168.0.10/rm.bin"\n')
text(RM, "set_time_zone", {"zone": 12}, 'event RM:SetTimeZone "12"\n', device_reply='OK event RM:SetTimeZone "12"\n', expect_result=OK)
text(RM, "get_time_zone", {}, 'event RM:GetTimeZone ""\n', device_reply='OK event RM:GetTimeZone "12"\n', expect_result=val("12"))
text(RM, "set_dst_enabled", {"state": "Enable"}, 'event RM:SetDstEnable "Enable"\n')
text(RM, "get_dst_enabled", {}, 'event RM:GetDstEnable ""\n', device_reply='OK event RM:GetDstEnable "Disable"\n', expect_result=val("Disable"))
text(RM, "set_dst_start", {"month": 3, "week": 5, "day": 0, "hour": 2}, 'event RM:SetDstStartTime "month=3,week=5,day=0,hour=2"\n')
text(RM, "get_dst_start", {}, 'event RM:GetDstStartTime ""\n')
text(RM, "set_dst_end", {"month": 10, "week": 5, "day": 0, "hour": 3}, 'event RM:SetDstEndTime "month=10,week=5,day=0,hour=3"\n')
text(RM, "get_dst_end", {}, 'event RM:GetDstEndTime ""\n')
text(RM, "set_ntp_enabled", {"state": "Disable"}, 'event RM:SetNtpEnable "Disable"\n')
text(RM, "get_ntp_enabled", {}, 'event RM:GetNtpEnable ""\n')
text(RM, "set_ntp_server", {"server": 2, "address": "pool.ntp.org"}, 'event RM:SetNtpServer2 "pool.ntp.org"\n',
     device_reply='OK event RM:SetNtpServer2 "pool.ntp.org"\n', expect_result=OK)
text(RM, "get_ntp_server", {"server": 4}, 'event RM:GetNtpServer4 ""\n', device_reply='OK event RM:GetNtpServer4 "10.0.0.1"\n',
     expect_result=val("10.0.0.1"))
text(RM, "provisioning_import", {"path": "ftp://user:pass@10.0.0.2/rm/config.xml"},
     'event rm:provisioningimport "ftp://user:pass@10.0.0.2/rm/config.xml"\n')
text(RM, "call_dial", {"line": "sip1", "number": "1234"}, 'event rm:callaction "dial=sip1:1234"\n', **RCR)
text(RM, "call_answer", {"line": "bt"}, 'event rm:callaction "offhook=bt"\n', **RCR)
text(RM, "call_hold_or_resume", {"line": "aux"}, 'event rm:callaction "holdorresume=aux"\n', **RCR)
text(RM, "call_hangup", {"line": "usb"}, 'event rm:callaction "hangup=usb"\n', device_reply='OK event rm:callaction "hangup=usb"\n',
     expect_result=OK, **RCR)
text(RM, "call_dtmf", {"line": "sip2", "digit": "#"}, 'event rm:callaction "dtmf=sip2:#"\n', **RCR)
text(RM, "call_join_or_split", {"line": "sip2"}, 'event rm:callaction "joinorsplit=sip2"\n', **RCR)
text(RM, "call_split_all", {}, 'event rm:callaction "splitall"\n', **RCR)
text(RM, "call_hold_or_resume_conference", {}, 'event rm:callaction "holdorresumeconf"\n', **RCR)
text(RM, "call_hangup_conference", {}, 'event rm:callaction "hangupconf"\n', **RCR)
text(RM, "get_call_status", {}, 'event rm:getcallstatus ""\n', **RCR)
text(RM, "set_call_mute", {"state": "on"}, 'event rm:setcallconfig "mute=on"\n', **RCR)
text(RM, "set_call_volume", {"volume": 19}, 'event rm:setcallconfig "vol=19"\n', **RCR)
text(RM, "set_call_do_not_disturb", {"state": "off"}, 'event rm:setcallconfig "dnd=off"\n', **RCR)
text(RM, "get_call_config", {}, 'event rm:getcallconfig ""\n', **RCR)
text(RM, "get_latest_call_recent_index", {}, 'event rm:getlatestcallrecentindex ""\n', **RCR)
text(RM, "get_call_recent", {"index": 5}, 'event rm:getcallrecents "5"\n', **RCR)
text(RM, "get_call_contact", {"index": 0}, 'event rm:getcallcontacts "0"\n', **RCR)
text(RM, "set_bluetooth_pairing", {"action": "start"}, 'event rm:bluetoothpairing "start"\n', **RCR)
text(RM, "get_bluetooth_status", {}, 'event rm:getbluetoothstatus ""\n', device_reply='OK event rm:getbluetoothstatus "connected"\n',
     expect_result=val("connected"), **RCR)
text(RM, "identify_accessory", {"seconds": 10, "id": "0123456789"}, 'event rm:accessoryidentify "duration=10,id=0123456789"\n', **RW16)
text(RM, "set_dect_pairing", {"action": "stop"}, 'event rm:dectpairing "stop"\n', **RW16)
text(RM, "get_dect_status", {}, 'event rm:getdectstatus ""\n', device_reply='OK event rm:getdectstatus "idle"\n',
     expect_result=val("idle"), **RW16)

# Error replies (RCP "ERROR <command> <code>")
text(RM, "get_parameter_info", {"index": 999}, "prminfo 999\n", device_reply="ERROR prminfo InvalidArgument\n", expect_result=ERR)

# Typed parameter commands: one set (last sub-address, lowest value) and
# one get (first sub-address) per section 7 entry.
text(RM, 'set_far_end_input_eq_on', {'channel': 8, 'enabled': True}, 'set RM:FeIn_EQ/Ch/On/On 7 0 1\n',
     device_reply='OK set RM:FeIn_EQ/Ch/On/On 7 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_on', {'channel': 1}, 'get RM:FeIn_EQ/Ch/On/On 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/On/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_input_eq_band_bypass', {'channel': 8, 'band': 3, 'bypass': True}, 'set RM:FeIn_EQ/Ch/Band/Bypass 7 2 1\n',
     device_reply='OK set RM:FeIn_EQ/Ch/Band/Bypass 7 2 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_band_bypass', {'channel': 1, 'band': 1}, 'get RM:FeIn_EQ/Ch/Band/Bypass 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/Band/Bypass 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_input_eq_band_frequency', {'channel': 8, 'band': 3, 'frequency': 200}, 'set RM:FeIn_EQ/Ch/Band/Frequency 7 2 200\n',
     device_reply='OK set RM:FeIn_EQ/Ch/Band/Frequency 7 2 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_band_frequency', {'channel': 1, 'band': 1}, 'get RM:FeIn_EQ/Ch/Band/Frequency 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/Band/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_far_end_input_eq_band_gain', {'channel': 8, 'band': 3, 'gain': -1800}, 'set RM:FeIn_EQ/Ch/Band/Gain 7 2 -1800\n',
     device_reply='OK set RM:FeIn_EQ/Ch/Band/Gain 7 2 -1800 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_band_gain', {'channel': 1, 'band': 1}, 'get RM:FeIn_EQ/Ch/Band/Gain 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/Band/Gain 0 0 1800\n', expect_result=val('1800'), **RCR)
text(RM, 'set_far_end_input_eq_band_q', {'channel': 8, 'band': 3, 'q': 100}, 'set RM:FeIn_EQ/Ch/Band/Q 7 2 100\n',
     device_reply='OK set RM:FeIn_EQ/Ch/Band/Q 7 2 100 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_band_q', {'channel': 1, 'band': 1}, 'get RM:FeIn_EQ/Ch/Band/Q 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/Band/Q 0 0 16000\n', expect_result=val('16000'), **RCR)
text(RM, 'set_far_end_input_eq_band_type', {'channel': 8, 'band': 3, 'type': 0}, 'set RM:FeIn_EQ/Ch/Band/Type 7 2 0\n',
     device_reply='OK set RM:FeIn_EQ/Ch/Band/Type 7 2 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_eq_band_type', {'channel': 1, 'band': 1}, 'get RM:FeIn_EQ/Ch/Band/Type 0 0\n',
     device_reply='OK get RM:FeIn_EQ/Ch/Band/Type 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_far_end_input_agc_on', {'channel': 8, 'enabled': True}, 'set RM:FeIn_AGC/Ch/On 7 0 1\n',
     device_reply='OK set RM:FeIn_AGC/Ch/On 7 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_agc_on', {'channel': 1}, 'get RM:FeIn_AGC/Ch/On 0 0\n',
     device_reply='OK get RM:FeIn_AGC/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_input_agc_target_level', {'channel': 8, 'level': -4000}, 'set RM:FeIn_AGC/Ch/TargetLevel 7 0 -4000\n',
     device_reply='OK set RM:FeIn_AGC/Ch/TargetLevel 7 0 -4000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_agc_target_level', {'channel': 1}, 'get RM:FeIn_AGC/Ch/TargetLevel 0 0\n',
     device_reply='OK get RM:FeIn_AGC/Ch/TargetLevel 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_far_end_input_agc_max_gain', {'channel': 8, 'gain': 0}, 'set RM:FeIn_AGC/Ch/MaxGain 7 0 0\n',
     device_reply='OK set RM:FeIn_AGC/Ch/MaxGain 7 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_agc_max_gain', {'channel': 1}, 'get RM:FeIn_AGC/Ch/MaxGain 0 0\n',
     device_reply='OK get RM:FeIn_AGC/Ch/MaxGain 0 0 2000\n', expect_result=val('2000'), **RCR)
text(RM, 'set_far_end_input_agc_min_gain', {'channel': 8, 'gain': -2000}, 'set RM:FeIn_AGC/Ch/MinGain 7 0 -2000\n',
     device_reply='OK set RM:FeIn_AGC/Ch/MinGain 7 0 -2000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_agc_min_gain', {'channel': 1}, 'get RM:FeIn_AGC/Ch/MinGain 0 0\n',
     device_reply='OK get RM:FeIn_AGC/Ch/MinGain 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_far_end_input_agc_noise_gate_on', {'channel': 8, 'enabled': True}, 'set RM:FeIn_AGC/Ch/NoiseGateOn 7 0 1\n',
     device_reply='OK set RM:FeIn_AGC/Ch/NoiseGateOn 7 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_agc_noise_gate_on', {'channel': 1}, 'get RM:FeIn_AGC/Ch/NoiseGateOn 0 0\n',
     device_reply='OK get RM:FeIn_AGC/Ch/NoiseGateOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_input_fader_on', {'channel': 8, 'enabled': True}, 'set RM:FeIn_Fader/Ch/On 7 0 1\n',
     device_reply='OK set RM:FeIn_Fader/Ch/On 7 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_fader_on', {'channel': 1}, 'get RM:FeIn_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:FeIn_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_input_fader_level', {'channel': 8, 'level': -32768}, 'set RM:FeIn_Fader/Ch/Level 7 0 -32768\n',
     device_reply='OK set RM:FeIn_Fader/Ch/Level 7 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_input_fader_level', {'channel': 1}, 'get RM:FeIn_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:FeIn_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_mic_input_eq_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_EQ/Ch/On/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/On/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_on', {'channel': 1}, 'get RM:ExtMic_EQ/Ch/On/On 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/On/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_eq_band_bypass', {'channel': 2, 'band': 5, 'bypass': True}, 'set RM:ExtMic_EQ/Ch/Band/Bypass 1 4 1\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/Band/Bypass 1 4 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_band_bypass', {'channel': 1, 'band': 1}, 'get RM:ExtMic_EQ/Ch/Band/Bypass 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/Band/Bypass 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_eq_band_frequency', {'channel': 2, 'band': 5, 'frequency': 200}, 'set RM:ExtMic_EQ/Ch/Band/Frequency 1 4 200\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/Band/Frequency 1 4 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_band_frequency', {'channel': 1, 'band': 1}, 'get RM:ExtMic_EQ/Ch/Band/Frequency 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/Band/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_mic_input_eq_band_gain', {'channel': 2, 'band': 5, 'gain': -1800}, 'set RM:ExtMic_EQ/Ch/Band/Gain 1 4 -1800\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/Band/Gain 1 4 -1800 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_band_gain', {'channel': 1, 'band': 1}, 'get RM:ExtMic_EQ/Ch/Band/Gain 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/Band/Gain 0 0 1800\n', expect_result=val('1800'), **RCR)
text(RM, 'set_mic_input_eq_band_q', {'channel': 2, 'band': 5, 'q': 100}, 'set RM:ExtMic_EQ/Ch/Band/Q 1 4 100\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/Band/Q 1 4 100 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_band_q', {'channel': 1, 'band': 1}, 'get RM:ExtMic_EQ/Ch/Band/Q 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/Band/Q 0 0 16000\n', expect_result=val('16000'), **RCR)
text(RM, 'set_mic_input_eq_band_type', {'channel': 2, 'band': 5, 'type': 0}, 'set RM:ExtMic_EQ/Ch/Band/Type 1 4 0\n',
     device_reply='OK set RM:ExtMic_EQ/Ch/Band/Type 1 4 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_eq_band_type', {'channel': 1, 'band': 1}, 'get RM:ExtMic_EQ/Ch/Band/Type 0 0\n',
     device_reply='OK get RM:ExtMic_EQ/Ch/Band/Type 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_mic_input_gate_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_Gate/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_on', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_gate_threshold', {'channel': 2, 'threshold': -7200}, 'set RM:ExtMic_Gate/Ch/Threshold 1 0 -7200\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/Threshold 1 0 -7200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_threshold', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/Threshold 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/Threshold 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mic_input_gate_range', {'channel': 2, 'range': -7000}, 'set RM:ExtMic_Gate/Ch/Range 1 0 -7000\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/Range 1 0 -7000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_range', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/Range 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/Range 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mic_input_gate_attack', {'channel': 2, 'attack': 0}, 'set RM:ExtMic_Gate/Ch/Attack 1 0 0\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/Attack 1 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_attack', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/Attack 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/Attack 0 0 120\n', expect_result=val('120'), **RCR)
text(RM, 'set_mic_input_gate_decay', {'channel': 2, 'decay': 3340}, 'set RM:ExtMic_Gate/Ch/Decay 1 0 3340\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/Decay 1 0 3340 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_decay', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/Decay 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/Decay 0 0 42700000\n', expect_result=val('42700000'), **RCR)
text(RM, 'set_mic_input_gate_hold', {'channel': 2, 'hold': 20}, 'set RM:ExtMic_Gate/Ch/Hold 1 0 20\n',
     device_reply='OK set RM:ExtMic_Gate/Ch/Hold 1 0 20 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_gate_hold', {'channel': 1}, 'get RM:ExtMic_Gate/Ch/Hold 0 0\n',
     device_reply='OK get RM:ExtMic_Gate/Ch/Hold 0 0 1960000\n', expect_result=val('1960000'), **RCR)
text(RM, 'set_mic_input_comp_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_Comp/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_on', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_comp_threshold', {'channel': 2, 'threshold': -5400}, 'set RM:ExtMic_Comp/Ch/Threshold 1 0 -5400\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Threshold 1 0 -5400 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_threshold', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Threshold 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Threshold 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mic_input_comp_ratio', {'channel': 2, 'ratio': 10}, 'set RM:ExtMic_Comp/Ch/Ratio 1 0 10\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Ratio 1 0 10 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_ratio', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Ratio 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Ratio 0 0 201\n', expect_result=val('201'), **RCR)
text(RM, 'set_mic_input_comp_knee', {'channel': 2, 'knee': 0}, 'set RM:ExtMic_Comp/Ch/Knee 1 0 0\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Knee 1 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_knee', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Knee 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Knee 0 0 5\n', expect_result=val('5'), **RCR)
text(RM, 'set_mic_input_comp_attack', {'channel': 2, 'attack': 0}, 'set RM:ExtMic_Comp/Ch/Attack 1 0 0\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Attack 1 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_attack', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Attack 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Attack 0 0 120\n', expect_result=val('120'), **RCR)
text(RM, 'set_mic_input_comp_release', {'channel': 2, 'release': 3340}, 'set RM:ExtMic_Comp/Ch/Release 1 0 3340\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Release 1 0 3340 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_release', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Release 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Release 0 0 42700000\n', expect_result=val('42700000'), **RCR)
text(RM, 'set_mic_input_comp_gain', {'channel': 2, 'gain': 0}, 'set RM:ExtMic_Comp/Ch/Gain 1 0 0\n',
     device_reply='OK set RM:ExtMic_Comp/Ch/Gain 1 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_comp_gain', {'channel': 1}, 'get RM:ExtMic_Comp/Ch/Gain 0 0\n',
     device_reply='OK get RM:ExtMic_Comp/Ch/Gain 0 0 1800\n', expect_result=val('1800'), **RCR)
text(RM, 'set_mic_input_fbs_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_FBS/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_FBS/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_fbs_on', {'channel': 1}, 'get RM:ExtMic_FBS/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_FBS/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_agc_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_AGC/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_AGC/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_agc_on', {'channel': 1}, 'get RM:ExtMic_AGC/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_AGC/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_agc_target_level', {'channel': 2, 'level': -4000}, 'set RM:ExtMic_AGC/Ch/TargetLevel 1 0 -4000\n',
     device_reply='OK set RM:ExtMic_AGC/Ch/TargetLevel 1 0 -4000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_agc_target_level', {'channel': 1}, 'get RM:ExtMic_AGC/Ch/TargetLevel 0 0\n',
     device_reply='OK get RM:ExtMic_AGC/Ch/TargetLevel 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mic_input_agc_max_gain', {'channel': 2, 'gain': 0}, 'set RM:ExtMic_AGC/Ch/MaxGain 1 0 0\n',
     device_reply='OK set RM:ExtMic_AGC/Ch/MaxGain 1 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_agc_max_gain', {'channel': 1}, 'get RM:ExtMic_AGC/Ch/MaxGain 0 0\n',
     device_reply='OK get RM:ExtMic_AGC/Ch/MaxGain 0 0 2000\n', expect_result=val('2000'), **RCR)
text(RM, 'set_mic_input_agc_min_gain', {'channel': 2, 'gain': -2000}, 'set RM:ExtMic_AGC/Ch/MinGain 1 0 -2000\n',
     device_reply='OK set RM:ExtMic_AGC/Ch/MinGain 1 0 -2000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_agc_min_gain', {'channel': 1}, 'get RM:ExtMic_AGC/Ch/MinGain 0 0\n',
     device_reply='OK get RM:ExtMic_AGC/Ch/MinGain 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mic_input_agc_noise_gate_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_AGC/Ch/NoiseGateOn 1 0 1\n',
     device_reply='OK set RM:ExtMic_AGC/Ch/NoiseGateOn 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_agc_noise_gate_on', {'channel': 1}, 'get RM:ExtMic_AGC/Ch/NoiseGateOn 0 0\n',
     device_reply='OK get RM:ExtMic_AGC/Ch/NoiseGateOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_fader_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_Fader/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_Fader/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_fader_on', {'channel': 1}, 'get RM:ExtMic_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_input_fader_level', {'channel': 2, 'level': -32768}, 'set RM:ExtMic_Fader/Ch/Level 1 0 -32768\n',
     device_reply='OK set RM:ExtMic_Fader/Ch/Level 1 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_fader_level', {'channel': 1}, 'get RM:ExtMic_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:ExtMic_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_mic_input_echo_suppressor_on', {'channel': 2, 'enabled': True}, 'set RM:ExtMic_ES/Ch/On 1 0 1\n',
     device_reply='OK set RM:ExtMic_ES/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mic_input_echo_suppressor_on', {'channel': 1}, 'get RM:ExtMic_ES/Ch/On 0 0\n',
     device_reply='OK get RM:ExtMic_ES/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_input_fader_on', {'channel': 16, 'enabled': True}, 'set RM:NeIn_Fader/Ch/On 15 0 1\n',
     device_reply='OK set RM:NeIn_Fader/Ch/On 15 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_fader_on', {'channel': 1}, 'get RM:NeIn_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:NeIn_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_input_fader_level', {'channel': 16, 'level': -32768}, 'set RM:NeIn_Fader/Ch/Level 15 0 -32768\n',
     device_reply='OK set RM:NeIn_Fader/Ch/Level 15 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_fader_level', {'channel': 1}, 'get RM:NeIn_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:NeIn_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_automix_type', {'type': 0}, 'set RM:Automix/Type/Type 0 0 0\n',
     device_reply='OK set RM:Automix/Type/Type 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_automix_type', {}, 'get RM:Automix/Type/Type 0 0\n',
     device_reply='OK get RM:Automix/Type/Type 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_gating_automix_last_mic_on', {'enabled': True}, 'set RM:GatingAutomix/Settings/LastMicOn 0 0 1\n',
     device_reply='OK set RM:GatingAutomix/Settings/LastMicOn 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_last_mic_on', {}, 'get RM:GatingAutomix/Settings/LastMicOn 0 0\n',
     device_reply='OK get RM:GatingAutomix/Settings/LastMicOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_gating_automix_open_mics', {'count': 1}, 'set RM:GatingAutomix/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:GatingAutomix/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_open_mics', {}, 'get RM:GatingAutomix/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:GatingAutomix/Settings/NumOfOpenMic 0 0 16\n', expect_result=val('16'), **RCR)
text(RM, 'set_gating_automix_threshold', {'threshold': -7200}, 'set RM:GatingAutomix/Settings/Threshold 0 0 -7200\n',
     device_reply='OK set RM:GatingAutomix/Settings/Threshold 0 0 -7200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_threshold', {}, 'get RM:GatingAutomix/Settings/Threshold 0 0\n',
     device_reply='OK get RM:GatingAutomix/Settings/Threshold 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_gating_automix_range', {'range': -7000}, 'set RM:GatingAutomix/Settings/Range 0 0 -7000\n',
     device_reply='OK set RM:GatingAutomix/Settings/Range 0 0 -7000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_range', {}, 'get RM:GatingAutomix/Settings/Range 0 0\n',
     device_reply='OK get RM:GatingAutomix/Settings/Range 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_gating_automix_hold', {'hold': 20}, 'set RM:GatingAutomix/Settings/Hold 0 0 20\n',
     device_reply='OK set RM:GatingAutomix/Settings/Hold 0 0 20 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_hold', {}, 'get RM:GatingAutomix/Settings/Hold 0 0\n',
     device_reply='OK get RM:GatingAutomix/Settings/Hold 0 0 1960000\n', expect_result=val('1960000'), **RCR)
text(RM, 'set_gating_automix_priority_mic', {'channel': 16, 'enabled': True}, 'set RM:GatingAutomix/Ch/PriorityMic 15 0 1\n',
     device_reply='OK set RM:GatingAutomix/Ch/PriorityMic 15 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gating_automix_priority_mic', {'channel': 1}, 'get RM:GatingAutomix/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:GatingAutomix/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_gain_sharing_automix_open_mics', {'count': 1}, 'set RM:GainSharingAutomix/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:GainSharingAutomix/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gain_sharing_automix_open_mics', {}, 'get RM:GainSharingAutomix/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:GainSharingAutomix/Settings/NumOfOpenMic 0 0 16\n', expect_result=val('16'), **RCR)
text(RM, 'set_gain_sharing_automix_priority_mic', {'channel': 16, 'enabled': True}, 'set RM:GainSharingAutomix/Ch/PriorityMic 15 0 1\n',
     device_reply='OK set RM:GainSharingAutomix/Ch/PriorityMic 15 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gain_sharing_automix_priority_mic', {'channel': 1}, 'get RM:GainSharingAutomix/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:GainSharingAutomix/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_input_agc_on', {'enabled': True}, 'set RM:NeIn_AGC/Ch/On 0 0 1\n',
     device_reply='OK set RM:NeIn_AGC/Ch/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_agc_on', {}, 'get RM:NeIn_AGC/Ch/On 0 0\n',
     device_reply='OK get RM:NeIn_AGC/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_input_agc_target_level', {'level': -4000}, 'set RM:NeIn_AGC/Ch/TargetLevel 0 0 -4000\n',
     device_reply='OK set RM:NeIn_AGC/Ch/TargetLevel 0 0 -4000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_agc_target_level', {}, 'get RM:NeIn_AGC/Ch/TargetLevel 0 0\n',
     device_reply='OK get RM:NeIn_AGC/Ch/TargetLevel 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_near_end_input_agc_max_gain', {'gain': 0}, 'set RM:NeIn_AGC/Ch/MaxGain 0 0 0\n',
     device_reply='OK set RM:NeIn_AGC/Ch/MaxGain 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_agc_max_gain', {}, 'get RM:NeIn_AGC/Ch/MaxGain 0 0\n',
     device_reply='OK get RM:NeIn_AGC/Ch/MaxGain 0 0 2000\n', expect_result=val('2000'), **RCR)
text(RM, 'set_near_end_input_agc_min_gain', {'gain': -2000}, 'set RM:NeIn_AGC/Ch/MinGain 0 0 -2000\n',
     device_reply='OK set RM:NeIn_AGC/Ch/MinGain 0 0 -2000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_agc_min_gain', {}, 'get RM:NeIn_AGC/Ch/MinGain 0 0\n',
     device_reply='OK get RM:NeIn_AGC/Ch/MinGain 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_near_end_input_agc_noise_gate_on', {'enabled': True}, 'set RM:NeIn_AGC/Ch/NoiseGateOn 0 0 1\n',
     device_reply='OK set RM:NeIn_AGC/Ch/NoiseGateOn 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_agc_noise_gate_on', {}, 'get RM:NeIn_AGC/Ch/NoiseGateOn 0 0\n',
     device_reply='OK get RM:NeIn_AGC/Ch/NoiseGateOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_input_ducker_on', {'enabled': True}, 'set RM:NeIn_Ducker/Ch/On 0 0 1\n',
     device_reply='OK set RM:NeIn_Ducker/Ch/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_input_ducker_on', {}, 'get RM:NeIn_Ducker/Ch/On 0 0\n',
     device_reply='OK get RM:NeIn_Ducker/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mix_bus_on', {'input': 11, 'output': 10, 'enabled': True}, 'set RM:MixBus/Input/Output/On 10 9 1\n',
     device_reply='OK set RM:MixBus/Input/Output/On 10 9 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mix_bus_on', {'input': 1, 'output': 1}, 'get RM:MixBus/Input/Output/On 0 0\n',
     device_reply='OK get RM:MixBus/Input/Output/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mix_bus_level', {'input': 11, 'output': 10, 'level': -32768}, 'set RM:MixBus/Input/Output/Level 10 9 -32768\n',
     device_reply='OK set RM:MixBus/Input/Output/Level 10 9 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mix_bus_level', {'input': 1, 'output': 1}, 'get RM:MixBus/Input/Output/Level 0 0\n',
     device_reply='OK get RM:MixBus/Input/Output/Level 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_mix_bus_delay_on', {'input': 11, 'output': 10, 'enabled': True}, 'set RM:MixBus/Input/Output/DelayOn 10 9 1\n',
     device_reply='OK set RM:MixBus/Input/Output/DelayOn 10 9 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mix_bus_delay_on', {'input': 1, 'output': 1}, 'get RM:MixBus/Input/Output/DelayOn 0 0\n',
     device_reply='OK get RM:MixBus/Input/Output/DelayOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mix_bus_delay_time', {'input': 11, 'output': 10, 'time': 0}, 'set RM:MixBus/Input/Output/DelayTime 10 9 0\n',
     device_reply='OK set RM:MixBus/Input/Output/DelayTime 10 9 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mix_bus_delay_time', {'input': 1, 'output': 1}, 'get RM:MixBus/Input/Output/DelayTime 0 0\n',
     device_reply='OK get RM:MixBus/Input/Output/DelayTime 0 0 500000\n', expect_result=val('500000'), **RCR)
text(RM, 'set_far_end_output_fader_on', {'channel': 8, 'enabled': True}, 'set RM:FeOut_Fader/Ch/On 7 0 1\n',
     device_reply='OK set RM:FeOut_Fader/Ch/On 7 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_output_fader_on', {'channel': 1}, 'get RM:FeOut_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:FeOut_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_far_end_output_fader_level', {'channel': 8, 'level': -32768}, 'set RM:FeOut_Fader/Ch/Level 7 0 -32768\n',
     device_reply='OK set RM:FeOut_Fader/Ch/Level 7 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_output_fader_level', {'channel': 1}, 'get RM:FeOut_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:FeOut_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_room_eq_on', {'channel': 2, 'enabled': True}, 'set RM:RoomEQ/Ch/On/On 1 0 1\n',
     device_reply='OK set RM:RoomEQ/Ch/On/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_on', {'channel': 1}, 'get RM:RoomEQ/Ch/On/On 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/On/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_room_eq_band_bypass', {'channel': 2, 'band': 6, 'bypass': True}, 'set RM:RoomEQ/Ch/Band/Bypass 1 5 1\n',
     device_reply='OK set RM:RoomEQ/Ch/Band/Bypass 1 5 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_band_bypass', {'channel': 1, 'band': 1}, 'get RM:RoomEQ/Ch/Band/Bypass 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/Band/Bypass 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_room_eq_band_frequency', {'channel': 2, 'band': 6, 'frequency': 200}, 'set RM:RoomEQ/Ch/Band/Frequency 1 5 200\n',
     device_reply='OK set RM:RoomEQ/Ch/Band/Frequency 1 5 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_band_frequency', {'channel': 1, 'band': 1}, 'get RM:RoomEQ/Ch/Band/Frequency 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/Band/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_room_eq_band_gain', {'channel': 2, 'band': 6, 'gain': -1800}, 'set RM:RoomEQ/Ch/Band/Gain 1 5 -1800\n',
     device_reply='OK set RM:RoomEQ/Ch/Band/Gain 1 5 -1800 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_band_gain', {'channel': 1, 'band': 1}, 'get RM:RoomEQ/Ch/Band/Gain 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/Band/Gain 0 0 1800\n', expect_result=val('1800'), **RCR)
text(RM, 'set_room_eq_band_q', {'channel': 2, 'band': 6, 'q': 100}, 'set RM:RoomEQ/Ch/Band/Q 1 5 100\n',
     device_reply='OK set RM:RoomEQ/Ch/Band/Q 1 5 100 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_band_q', {'channel': 1, 'band': 1}, 'get RM:RoomEQ/Ch/Band/Q 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/Band/Q 0 0 16000\n', expect_result=val('16000'), **RCR)
text(RM, 'set_room_eq_band_type', {'channel': 2, 'band': 6, 'type': 0}, 'set RM:RoomEQ/Ch/Band/Type 1 5 0\n',
     device_reply='OK set RM:RoomEQ/Ch/Band/Type 1 5 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_band_type', {'channel': 1, 'band': 1}, 'get RM:RoomEQ/Ch/Band/Type 0 0\n',
     device_reply='OK get RM:RoomEQ/Ch/Band/Type 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_speaker_processor_input_level', {'channel': 4, 'level': -32768}, 'set RM:SpeakerProcessor/Ch/Input/Level 3 0 -32768\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/Input/Level 3 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_input_level', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/Input/Level 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/Input/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_speaker_processor_delay_on', {'channel': 4, 'enabled': True}, 'set RM:SpeakerProcessor/Ch/Delay/On 3 0 1\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/Delay/On 3 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_delay_on', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/Delay/On 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/Delay/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_speaker_processor_delay_time', {'channel': 4, 'time': 0}, 'set RM:SpeakerProcessor/Ch/Delay/Time 3 0 0\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/Delay/Time 3 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_delay_time', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/Delay/Time 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/Delay/Time 0 0 500000\n', expect_result=val('500000'), **RCR)
text(RM, 'set_speaker_processor_xover_hpf_frequency', {'channel': 4, 'frequency': 200}, 'set RM:SpeakerProcessor/Ch/XOverHpf/Frequency 3 0 200\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverHpf/Frequency 3 0 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_hpf_frequency', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverHpf/Frequency 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverHpf/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_speaker_processor_xover_hpf_gc', {'channel': 4, 'gain': -6}, 'set RM:SpeakerProcessor/Ch/XOverHpf/Gc 3 0 -6\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverHpf/Gc 3 0 -6 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_hpf_gc', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverHpf/Gc 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverHpf/Gc 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_speaker_processor_xover_hpf_type', {'channel': 4, 'type': 0}, 'set RM:SpeakerProcessor/Ch/XOverHpf/Type 3 0 0\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverHpf/Type 3 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_hpf_type', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverHpf/Type 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverHpf/Type 0 0 19\n', expect_result=val('19'), **RCR)
text(RM, 'set_speaker_processor_xover_lpf_frequency', {'channel': 4, 'frequency': 200}, 'set RM:SpeakerProcessor/Ch/XOverLpf/Frequency 3 0 200\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverLpf/Frequency 3 0 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_lpf_frequency', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverLpf/Frequency 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverLpf/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_speaker_processor_xover_lpf_gc', {'channel': 4, 'gain': -6}, 'set RM:SpeakerProcessor/Ch/XOverLpf/Gc 3 0 -6\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverLpf/Gc 3 0 -6 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_lpf_gc', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverLpf/Gc 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverLpf/Gc 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_speaker_processor_xover_lpf_type', {'channel': 4, 'type': 0}, 'set RM:SpeakerProcessor/Ch/XOverLpf/Type 3 0 0\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/XOverLpf/Type 3 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_xover_lpf_type', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/XOverLpf/Type 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/XOverLpf/Type 0 0 19\n', expect_result=val('19'), **RCR)
text(RM, 'set_speaker_processor_peq_on', {'channel': 4, 'enabled': True}, 'set RM:SpeakerProcessor/Ch/PEQOn/On 3 0 1\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQOn/On 3 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_on', {'channel': 1}, 'get RM:SpeakerProcessor/Ch/PEQOn/On 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQOn/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_speaker_processor_peq_band_bypass', {'channel': 4, 'band': 6, 'bypass': True}, 'set RM:SpeakerProcessor/Ch/PEQBand/Bypass 3 5 1\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQBand/Bypass 3 5 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_band_bypass', {'channel': 1, 'band': 1}, 'get RM:SpeakerProcessor/Ch/PEQBand/Bypass 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQBand/Bypass 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_speaker_processor_peq_band_frequency', {'channel': 4, 'band': 6, 'frequency': 200}, 'set RM:SpeakerProcessor/Ch/PEQBand/Frequency 3 5 200\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQBand/Frequency 3 5 200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_band_frequency', {'channel': 1, 'band': 1}, 'get RM:SpeakerProcessor/Ch/PEQBand/Frequency 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQBand/Frequency 0 0 200000\n', expect_result=val('200000'), **RCR)
text(RM, 'set_speaker_processor_peq_band_gain', {'channel': 4, 'band': 6, 'gain': -1800}, 'set RM:SpeakerProcessor/Ch/PEQBand/Gain 3 5 -1800\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQBand/Gain 3 5 -1800 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_band_gain', {'channel': 1, 'band': 1}, 'get RM:SpeakerProcessor/Ch/PEQBand/Gain 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQBand/Gain 0 0 1800\n', expect_result=val('1800'), **RCR)
text(RM, 'set_speaker_processor_peq_band_q', {'channel': 4, 'band': 6, 'q': 100}, 'set RM:SpeakerProcessor/Ch/PEQBand/Q 3 5 100\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQBand/Q 3 5 100 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_band_q', {'channel': 1, 'band': 1}, 'get RM:SpeakerProcessor/Ch/PEQBand/Q 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQBand/Q 0 0 16000\n', expect_result=val('16000'), **RCR)
text(RM, 'set_speaker_processor_peq_band_type', {'channel': 4, 'band': 6, 'type': 0}, 'set RM:SpeakerProcessor/Ch/PEQBand/Type 3 5 0\n',
     device_reply='OK set RM:SpeakerProcessor/Ch/PEQBand/Type 3 5 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_speaker_processor_peq_band_type', {'channel': 1, 'band': 1}, 'get RM:SpeakerProcessor/Ch/PEQBand/Type 0 0\n',
     device_reply='OK get RM:SpeakerProcessor/Ch/PEQBand/Type 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_near_end_output_fader_on', {'channel': 5, 'enabled': True}, 'set RM:NeOut_Fader/Ch/On 4 0 1\n',
     device_reply='OK set RM:NeOut_Fader/Ch/On 4 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_output_fader_on', {'channel': 1}, 'get RM:NeOut_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:NeOut_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_near_end_output_fader_level', {'channel': 5, 'level': -32768}, 'set RM:NeOut_Fader/Ch/Level 4 0 -32768\n',
     device_reply='OK set RM:NeOut_Fader/Ch/Level 4 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_output_fader_level', {'channel': 1}, 'get RM:NeOut_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:NeOut_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_far_end_sip_tone_level', {'level': -32768}, 'set RM:SipToneFe_Fader/Ch/Level 0 0 -32768\n',
     device_reply='OK set RM:SipToneFe_Fader/Ch/Level 0 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_far_end_sip_tone_level', {}, 'get RM:SipToneFe_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:SipToneFe_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_near_end_sip_tone_level', {'level': -32768}, 'set RM:SipToneNe_Fader/Ch/Level 0 0 -32768\n',
     device_reply='OK set RM:SipToneNe_Fader/Ch/Level 0 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_near_end_sip_tone_level', {}, 'get RM:SipToneNe_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:SipToneNe_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_ai_denoiser_on', {'enabled': True}, 'set RM:DSPTOP/SubprocMode/On 0 0 1\n',
     device_reply='OK set RM:DSPTOP/SubprocMode/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_ai_denoiser_on', {}, 'get RM:DSPTOP/SubprocMode/On 0 0\n',
     device_reply='OK get RM:DSPTOP/SubprocMode/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_ai_denoiser_type', {'type': 0}, 'set RM:AIDenoiser/Type/Type 0 0 0\n',
     device_reply='OK set RM:AIDenoiser/Type/Type 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_ai_denoiser_type', {}, 'get RM:AIDenoiser/Type/Type 0 0\n',
     device_reply='OK get RM:AIDenoiser/Type/Type 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_room_eq_output_on', {'channel': 2, 'enabled': True}, 'set RM:RoomEQ_Fader/Ch/On 1 0 1\n',
     device_reply='OK set RM:RoomEQ_Fader/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_output_on', {'channel': 1}, 'get RM:RoomEQ_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:RoomEQ_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_room_eq_output_level', {'channel': 2, 'level': -5000}, 'set RM:RoomEQ_Fader/Ch/Level 1 0 -5000\n',
     device_reply='OK set RM:RoomEQ_Fader/Ch/Level 1 0 -5000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_room_eq_output_level', {'channel': 1}, 'get RM:RoomEQ_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:RoomEQ_Fader/Ch/Level 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_dante_input_patch', {'output': 22, 'source': 0}, 'set RM:DanteIn_Patch/Output/Input 21 0 0\n',
     device_reply='OK set RM:DanteIn_Patch/Output/Input 21 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_dante_input_patch', {'output': 1}, 'get RM:DanteIn_Patch/Output/Input 0 0\n',
     device_reply='OK get RM:DanteIn_Patch/Output/Input 0 0 16\n', expect_result=val('16'), **RCR)
text(RM, 'set_voice_lift_input_fader_on', {'channel': 6, 'enabled': True}, 'set RM:VLIn_Fader/Ch/On 5 0 1\n',
     device_reply='OK set RM:VLIn_Fader/Ch/On 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_input_fader_on', {'channel': 1}, 'get RM:VLIn_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:VLIn_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_input_fader_level', {'channel': 6, 'level': -32768}, 'set RM:VLIn_Fader/Ch/Level 5 0 -32768\n',
     device_reply='OK set RM:VLIn_Fader/Ch/Level 5 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_input_fader_level', {'channel': 1}, 'get RM:VLIn_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:VLIn_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_voice_lift_align_fader_on', {'channel': 6, 'enabled': True}, 'set RM:VLAlign_Fader/Ch/On 5 0 1\n',
     device_reply='OK set RM:VLAlign_Fader/Ch/On 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_align_fader_on', {'channel': 1}, 'get RM:VLAlign_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:VLAlign_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_align_fader_level', {'channel': 6, 'level': -32768}, 'set RM:VLAlign_Fader/Ch/Level 5 0 -32768\n',
     device_reply='OK set RM:VLAlign_Fader/Ch/Level 5 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_align_fader_level', {'channel': 1}, 'get RM:VLAlign_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:VLAlign_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_voice_lift_link_fader_on', {'enabled': True}, 'set RM:VLLink_Fader/Ch/On 0 0 1\n',
     device_reply='OK set RM:VLLink_Fader/Ch/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_link_fader_on', {}, 'get RM:VLLink_Fader/Ch/On 0 0\n',
     device_reply='OK get RM:VLLink_Fader/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_link_fader_level', {'level': -32768}, 'set RM:VLLink_Fader/Ch/Level 0 0 -32768\n',
     device_reply='OK set RM:VLLink_Fader/Ch/Level 0 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_link_fader_level', {}, 'get RM:VLLink_Fader/Ch/Level 0 0\n',
     device_reply='OK get RM:VLLink_Fader/Ch/Level 0 0 1000\n', expect_result=val('1000'), **RCR)
text(RM, 'set_voice_lift_automix_type', {'type': 0}, 'set RM:VL_Automix/Type/Type 0 0 0\n',
     device_reply='OK set RM:VL_Automix/Type/Type 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_automix_type', {}, 'get RM:VL_Automix/Type/Type 0 0\n',
     device_reply='OK get RM:VL_Automix/Type/Type 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_gating_automix_last_mic_on', {'enabled': True}, 'set RM:VL_GatingAutomix/Settings/LastMicOn 0 0 1\n',
     device_reply='OK set RM:VL_GatingAutomix/Settings/LastMicOn 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_last_mic_on', {}, 'get RM:VL_GatingAutomix/Settings/LastMicOn 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Settings/LastMicOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_gating_automix_open_mics', {'count': 1}, 'set RM:VL_GatingAutomix/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:VL_GatingAutomix/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_open_mics', {}, 'get RM:VL_GatingAutomix/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Settings/NumOfOpenMic 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_voice_lift_gating_automix_threshold', {'threshold': -7200}, 'set RM:VL_GatingAutomix/Settings/Threshold 0 0 -7200\n',
     device_reply='OK set RM:VL_GatingAutomix/Settings/Threshold 0 0 -7200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_threshold', {}, 'get RM:VL_GatingAutomix/Settings/Threshold 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Settings/Threshold 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_voice_lift_gating_automix_range', {'range': -7000}, 'set RM:VL_GatingAutomix/Settings/Range 0 0 -7000\n',
     device_reply='OK set RM:VL_GatingAutomix/Settings/Range 0 0 -7000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_range', {}, 'get RM:VL_GatingAutomix/Settings/Range 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Settings/Range 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_voice_lift_gating_automix_hold', {'hold': 20}, 'set RM:VL_GatingAutomix/Settings/Hold 0 0 20\n',
     device_reply='OK set RM:VL_GatingAutomix/Settings/Hold 0 0 20 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_hold', {}, 'get RM:VL_GatingAutomix/Settings/Hold 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Settings/Hold 0 0 1960000\n', expect_result=val('1960000'), **RCR)
text(RM, 'set_voice_lift_gating_automix_priority_mic', {'channel': 6, 'enabled': True}, 'set RM:VL_GatingAutomix/Ch/PriorityMic 5 0 1\n',
     device_reply='OK set RM:VL_GatingAutomix/Ch/PriorityMic 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gating_automix_priority_mic', {'channel': 1}, 'get RM:VL_GatingAutomix/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:VL_GatingAutomix/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_gain_sharing_automix_open_mics', {'count': 1}, 'set RM:VL_GainSharingAutomix/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:VL_GainSharingAutomix/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gain_sharing_automix_open_mics', {}, 'get RM:VL_GainSharingAutomix/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:VL_GainSharingAutomix/Settings/NumOfOpenMic 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_voice_lift_gain_sharing_automix_priority_mic', {'channel': 6, 'enabled': True}, 'set RM:VL_GainSharingAutomix/Ch/PriorityMic 5 0 1\n',
     device_reply='OK set RM:VL_GainSharingAutomix/Ch/PriorityMic 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gain_sharing_automix_priority_mic', {'channel': 1}, 'get RM:VL_GainSharingAutomix/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:VL_GainSharingAutomix/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_gain_sharing_automix_weight', {'channel': 6, 'weight': -3000}, 'set RM:VL_GainSharingAutomix/Ch/Weight 5 0 -3000\n',
     device_reply='OK set RM:VL_GainSharingAutomix/Ch/Weight 5 0 -3000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gain_sharing_automix_weight', {'channel': 1}, 'get RM:VL_GainSharingAutomix/Ch/Weight 0 0\n',
     device_reply='OK get RM:VL_GainSharingAutomix/Ch/Weight 0 0 1500\n', expect_result=val('1500'), **RCR)
text(RM, 'set_voice_lift_gate_on', {'channel': 6, 'enabled': True}, 'set RM:VL_Gate/Ch/On 5 0 1\n',
     device_reply='OK set RM:VL_Gate/Ch/On 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_on', {'channel': 1}, 'get RM:VL_Gate/Ch/On 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_gate_threshold', {'channel': 6, 'threshold': -7200}, 'set RM:VL_Gate/Ch/Threshold 5 0 -7200\n',
     device_reply='OK set RM:VL_Gate/Ch/Threshold 5 0 -7200 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_threshold', {'channel': 1}, 'get RM:VL_Gate/Ch/Threshold 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/Threshold 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_voice_lift_gate_range', {'channel': 6, 'range': -7000}, 'set RM:VL_Gate/Ch/Range 5 0 -7000\n',
     device_reply='OK set RM:VL_Gate/Ch/Range 5 0 -7000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_range', {'channel': 1}, 'get RM:VL_Gate/Ch/Range 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/Range 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_voice_lift_gate_attack', {'channel': 6, 'attack': 0}, 'set RM:VL_Gate/Ch/Attack 5 0 0\n',
     device_reply='OK set RM:VL_Gate/Ch/Attack 5 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_attack', {'channel': 1}, 'get RM:VL_Gate/Ch/Attack 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/Attack 0 0 120\n', expect_result=val('120'), **RCR)
text(RM, 'set_voice_lift_gate_decay', {'channel': 6, 'decay': 3340}, 'set RM:VL_Gate/Ch/Decay 5 0 3340\n',
     device_reply='OK set RM:VL_Gate/Ch/Decay 5 0 3340 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_decay', {'channel': 1}, 'get RM:VL_Gate/Ch/Decay 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/Decay 0 0 42700000\n', expect_result=val('42700000'), **RCR)
text(RM, 'set_voice_lift_gate_hold', {'channel': 6, 'hold': 20}, 'set RM:VL_Gate/Ch/Hold 5 0 20\n',
     device_reply='OK set RM:VL_Gate/Ch/Hold 5 0 20 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_gate_hold', {'channel': 1}, 'get RM:VL_Gate/Ch/Hold 0 0\n',
     device_reply='OK get RM:VL_Gate/Ch/Hold 0 0 1960000\n', expect_result=val('1960000'), **RCR)
text(RM, 'set_voice_lift_fbs_on', {'channel': 6, 'enabled': True}, 'set RM:VL_FBS/Ch/On 5 0 1\n',
     device_reply='OK set RM:VL_FBS/Ch/On 5 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_fbs_on', {'channel': 1}, 'get RM:VL_FBS/Ch/On 0 0\n',
     device_reply='OK get RM:VL_FBS/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_fbs_suppression_level', {'channel': 6, 'level': 0}, 'set RM:VL_FBS/Ch/SuppressionLevel 5 0 0\n',
     device_reply='OK set RM:VL_FBS/Ch/SuppressionLevel 5 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_fbs_suppression_level', {'channel': 1}, 'get RM:VL_FBS/Ch/SuppressionLevel 0 0\n',
     device_reply='OK get RM:VL_FBS/Ch/SuppressionLevel 0 0 9\n', expect_result=val('9'), **RCR)
text(RM, 'set_voice_lift_auto_mute_on', {'enabled': True}, 'set RM:VL_AutoMute/Ch/On 0 0 1\n',
     device_reply='OK set RM:VL_AutoMute/Ch/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_auto_mute_on', {}, 'get RM:VL_AutoMute/Ch/On 0 0\n',
     device_reply='OK get RM:VL_AutoMute/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_input_patch', {'source': 0}, 'set RM:LG_Patch/Output/Input 0 0 0\n',
     device_reply='OK set RM:LG_Patch/Output/Input 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_input_patch', {}, 'get RM:LG_Patch/Output/Input 0 0\n',
     device_reply='OK get RM:LG_Patch/Output/Input 0 0 6\n', expect_result=val('6'), **RCR)
text(RM, 'set_voice_lift_oscillator_on', {'enabled': True}, 'set RM:VL_Manualtune/Ch/On 0 0 1\n',
     device_reply='OK set RM:VL_Manualtune/Ch/On 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_oscillator_on', {}, 'get RM:VL_Manualtune/Ch/On 0 0\n',
     device_reply='OK get RM:VL_Manualtune/Ch/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_oscillator_level', {'level': -32768}, 'set RM:VL_Manualtune/Ch/Level 0 0 -32768\n',
     device_reply='OK set RM:VL_Manualtune/Ch/Level 0 0 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_oscillator_level', {}, 'get RM:VL_Manualtune/Ch/Level 0 0\n',
     device_reply='OK get RM:VL_Manualtune/Ch/Level 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_voice_lift_loop_gain_start_time', {'time': 0}, 'set RM:VL_Manualtune/Ch/StartTime 0 0 0\n',
     device_reply='OK set RM:VL_Manualtune/Ch/StartTime 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_loop_gain_start_time', {}, 'get RM:VL_Manualtune/Ch/StartTime 0 0\n',
     device_reply='OK get RM:VL_Manualtune/Ch/StartTime 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_voice_lift_output_patch', {'output': 6, 'source': 0}, 'set RM:VLOut_Patch/Output/Input 5 0 0\n',
     device_reply='OK set RM:VLOut_Patch/Output/Input 5 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_output_patch', {'output': 1}, 'get RM:VLOut_Patch/Output/Input 0 0\n',
     device_reply='OK get RM:VLOut_Patch/Output/Input 0 0 7\n', expect_result=val('7'), **RCR)
text(RM, 'set_dante_output_matrix_on', {'input': 10, 'output': 16, 'enabled': True}, 'set RM:NeOut_Matrix/Input/Output/On 9 14 1\n',
     device_reply='OK set RM:NeOut_Matrix/Input/Output/On 9 14 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_dante_output_matrix_on', {'input': 1, 'output': 2}, 'get RM:NeOut_Matrix/Input/Output/On 0 0\n',
     device_reply='OK get RM:NeOut_Matrix/Input/Output/On 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_dante_output_matrix_level', {'input': 10, 'output': 16, 'level': -32768}, 'set RM:NeOut_Matrix/Input/Output/Level 9 14 -32768\n',
     device_reply='OK set RM:NeOut_Matrix/Input/Output/Level 9 14 -32768 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_dante_output_matrix_level', {'input': 1, 'output': 2}, 'get RM:NeOut_Matrix/Input/Output/Level 0 0\n',
     device_reply='OK get RM:NeOut_Matrix/Input/Output/Level 0 0 0\n', expect_result=val('0'), **RCR)
text(RM, 'set_dante_output_matrix_delay_on', {'input': 10, 'output': 16, 'enabled': True}, 'set RM:NeOut_Matrix/Input/Output/DelayOn 9 14 1\n',
     device_reply='OK set RM:NeOut_Matrix/Input/Output/DelayOn 9 14 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_dante_output_matrix_delay_on', {'input': 1, 'output': 2}, 'get RM:NeOut_Matrix/Input/Output/DelayOn 0 0\n',
     device_reply='OK get RM:NeOut_Matrix/Input/Output/DelayOn 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_dante_output_matrix_delay_time', {'input': 10, 'output': 16, 'time': 0}, 'set RM:NeOut_Matrix/Input/Output/DelayTime 9 14 0\n',
     device_reply='OK set RM:NeOut_Matrix/Input/Output/DelayTime 9 14 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_dante_output_matrix_delay_time', {'input': 1, 'output': 2}, 'get RM:NeOut_Matrix/Input/Output/DelayTime 0 0\n',
     device_reply='OK get RM:NeOut_Matrix/Input/Output/DelayTime 0 0 500000\n', expect_result=val('500000'), **RCR)
text(RM, 'set_voice_lift_fine_tune', {'value': 0}, 'set RM:AutoMicEq/FineTune 0 0 0\n',
     device_reply='OK set RM:AutoMicEq/FineTune 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_voice_lift_fine_tune', {}, 'get RM:AutoMicEq/FineTune 0 0\n',
     device_reply='OK get RM:AutoMicEq/FineTune 0 0 10\n', expect_result=val('10'), **RCR)
text(RM, 'set_mute_all', {'muted': True}, 'set RM:MicMute/All 0 0 1\n',
     device_reply='OK set RM:MicMute/All 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mute_all', {}, 'get RM:MicMute/All 0 0\n',
     device_reply='OK get RM:MicMute/All 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mute_group', {'group': 8, 'muted': True}, 'set RM:MicMute/Group 8 0 1\n',
     device_reply='OK set RM:MicMute/Group 8 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mute_group', {'group': 1}, 'get RM:MicMute/Group 1 0\n',
     device_reply='OK get RM:MicMute/Group 1 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mute_force_all_individual', {'muted': True}, 'set RM:MicMute/ForceAllIndividual 0 0 1\n',
     device_reply='OK set RM:MicMute/ForceAllIndividual 0 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_mute_force_all_individual', {}, 'get RM:MicMute/ForceAllIndividual 0 0\n',
     device_reply='OK get RM:MicMute/ForceAllIndividual 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_led_brightness', {'brightness': 0}, 'set RM:Led/Brightness 0 0 0\n',
     device_reply='OK set RM:Led/Brightness 0 0 0 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_led_brightness', {}, 'get RM:Led/Brightness 0 0\n',
     device_reply='OK get RM:Led/Brightness 0 0 3\n', expect_result=val('3'), **RCR)
text(RM, 'set_gain_sharing_automix_weight', {'channel': 16, 'weight': -3000}, 'set RM:GainSharingAutomix/Ch/Weight 15 0 -3000\n',
     device_reply='OK set RM:GainSharingAutomix/Ch/Weight 15 0 -3000 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_gain_sharing_automix_weight', {'channel': 1}, 'get RM:GainSharingAutomix/Ch/Weight 0 0\n',
     device_reply='OK get RM:GainSharingAutomix/Ch/Weight 0 0 1500\n', expect_result=val('1500'), **RCR)
text(RM, 'set_control_set_execute', {'control': 10, 'execute': True}, 'set RM:ControlSets/Execution 9 0 1\n',
     device_reply='OK set RM:ControlSets/Execution 9 0 1 "x"\n', expect_result=OK, **RCR)
text(RM, 'get_control_set_execute', {'control': 1}, 'get RM:ControlSets/Execution 0 0\n',
     device_reply='OK get RM:ControlSets/Execution 0 0 1\n', expect_result=val('1'), **RCR)
text(RM, 'set_mic_gain_type', {'type': 0}, 'set RM:DSPTOP/MicGaintype 0 0 0\n',
     device_reply='OK set RM:DSPTOP/MicGaintype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_mic_gain_type', {}, 'get RM:DSPTOP/MicGaintype 0 0\n',
     device_reply='OK get RM:DSPTOP/MicGaintype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_low_latency_mic_gain_type', {'type': 0}, 'set RM:DSPTOP/LlMicGaintype 0 0 0\n',
     device_reply='OK set RM:DSPTOP/LlMicGaintype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_low_latency_mic_gain_type', {}, 'get RM:DSPTOP/LlMicGaintype 0 0\n',
     device_reply='OK get RM:DSPTOP/LlMicGaintype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_output2_mode', {'mode': 0}, 'set RM:DSPTOP/Output2Mode 0 0 0\n',
     device_reply='OK set RM:DSPTOP/Output2Mode 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_output2_mode', {}, 'get RM:DSPTOP/Output2Mode 0 0\n',
     device_reply='OK get RM:DSPTOP/Output2Mode 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_beam_max_speed', {'speed': 0}, 'set RM:DSPTOP/MaxBeamSpeed 0 0 0\n',
     device_reply='OK set RM:DSPTOP/MaxBeamSpeed 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_max_speed', {}, 'get RM:DSPTOP/MaxBeamSpeed 0 0\n',
     device_reply='OK get RM:DSPTOP/MaxBeamSpeed 0 0 2\n', expect_result=val('2'), **RCG)
text(RM, 'set_beam_limit_on', {'enabled': True}, 'set RM:Mic_Beam/LimitOn 0 0 1\n',
     device_reply='OK set RM:Mic_Beam/LimitOn 0 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_limit_on', {}, 'get RM:Mic_Beam/LimitOn 0 0\n',
     device_reply='OK get RM:Mic_Beam/LimitOn 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_beam_limit_top', {'value': -39}, 'set RM:Mic_Beam/Top 0 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Top 0 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_limit_top', {}, 'get RM:Mic_Beam/Top 0 0\n',
     device_reply='OK get RM:Mic_Beam/Top 0 0 40\n', expect_result=val('40'), **RCG)
text(RM, 'set_beam_limit_bottom', {'value': -40}, 'set RM:Mic_Beam/Bottom 0 0 -40\n',
     device_reply='OK set RM:Mic_Beam/Bottom 0 0 -40 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_limit_bottom', {}, 'get RM:Mic_Beam/Bottom 0 0\n',
     device_reply='OK get RM:Mic_Beam/Bottom 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_limit_left', {'value': -40}, 'set RM:Mic_Beam/Left 0 0 -40\n',
     device_reply='OK set RM:Mic_Beam/Left 0 0 -40 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_limit_left', {}, 'get RM:Mic_Beam/Left 0 0\n',
     device_reply='OK get RM:Mic_Beam/Left 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_limit_right', {'value': -39}, 'set RM:Mic_Beam/Right 0 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Right 0 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_limit_right', {}, 'get RM:Mic_Beam/Right 0 0\n',
     device_reply='OK get RM:Mic_Beam/Right 0 0 40\n', expect_result=val('40'), **RCG)
text(RM, 'set_beam_floor_to_talker_height', {'value': 0}, 'set RM:Mic_Beam/Height_FlrToTalk 0 0 0\n',
     device_reply='OK set RM:Mic_Beam/Height_FlrToTalk 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_floor_to_talker_height', {}, 'get RM:Mic_Beam/Height_FlrToTalk 0 0\n',
     device_reply='OK get RM:Mic_Beam/Height_FlrToTalk 0 0 30\n', expect_result=val('30'), **RCG)
text(RM, 'set_beam_floor_to_mic_height', {'value': 20}, 'set RM:Mic_Beam/Height_FlrToMic 0 0 20\n',
     device_reply='OK set RM:Mic_Beam/Height_FlrToMic 0 0 20 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_floor_to_mic_height', {}, 'get RM:Mic_Beam/Height_FlrToMic 0 0\n',
     device_reply='OK get RM:Mic_Beam/Height_FlrToMic 0 0 60\n', expect_result=val('60'), **RCG)
text(RM, 'set_beam_speed', {'speed': 0}, 'set RM:Mic_Beam/Speed 0 0 0\n',
     device_reply='OK set RM:Mic_Beam/Speed 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_speed', {}, 'get RM:Mic_Beam/Speed 0 0\n',
     device_reply='OK get RM:Mic_Beam/Speed 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_echo_cancellation_level', {'level': 0}, 'set RM:Mic_Dsp/Aectype 0 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Aectype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_echo_cancellation_level', {}, 'get RM:Mic_Dsp/Aectype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Aectype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_noise_reduction_level', {'level': 0}, 'set RM:Mic_Dsp/Nrtype 0 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Nrtype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_noise_reduction_level', {}, 'get RM:Mic_Dsp/Nrtype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Nrtype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_dereverb_level', {'level': 0}, 'set RM:Mic_Dsp/Derevtype 0 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Derevtype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_dereverb_level', {}, 'get RM:Mic_Dsp/Derevtype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Derevtype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_near_end_output_eq_on', {'channel': 2, 'enabled': True}, 'set RM:NeOut_EQ/Ch/On/On 1 0 1\n',
     device_reply='OK set RM:NeOut_EQ/Ch/On/On 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_on', {'channel': 1}, 'get RM:NeOut_EQ/Ch/On/On 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/On/On 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_near_end_output_eq_band_bypass', {'channel': 2, 'band': 6, 'bypass': True}, 'set RM:NeOut_EQ/Ch/Band/Bypass 1 5 1\n',
     device_reply='OK set RM:NeOut_EQ/Ch/Band/Bypass 1 5 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_band_bypass', {'channel': 1, 'band': 1}, 'get RM:NeOut_EQ/Ch/Band/Bypass 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/Band/Bypass 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_near_end_output_eq_band_frequency', {'channel': 2, 'band': 6, 'frequency': 200}, 'set RM:NeOut_EQ/Ch/Band/Frequency 1 5 200\n',
     device_reply='OK set RM:NeOut_EQ/Ch/Band/Frequency 1 5 200 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_band_frequency', {'channel': 1, 'band': 1}, 'get RM:NeOut_EQ/Ch/Band/Frequency 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/Band/Frequency 0 0 200000\n', expect_result=val('200000'), **RCG)
text(RM, 'set_near_end_output_eq_band_gain', {'channel': 2, 'band': 6, 'gain': -1800}, 'set RM:NeOut_EQ/Ch/Band/Gain 1 5 -1800\n',
     device_reply='OK set RM:NeOut_EQ/Ch/Band/Gain 1 5 -1800 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_band_gain', {'channel': 1, 'band': 1}, 'get RM:NeOut_EQ/Ch/Band/Gain 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/Band/Gain 0 0 1800\n', expect_result=val('1800'), **RCG)
text(RM, 'set_near_end_output_eq_band_q', {'channel': 2, 'band': 6, 'q': 100}, 'set RM:NeOut_EQ/Ch/Band/Q 1 5 100\n',
     device_reply='OK set RM:NeOut_EQ/Ch/Band/Q 1 5 100 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_band_q', {'channel': 1, 'band': 1}, 'get RM:NeOut_EQ/Ch/Band/Q 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/Band/Q 0 0 16000\n', expect_result=val('16000'), **RCG)
text(RM, 'set_near_end_output_eq_band_type', {'channel': 2, 'band': 6, 'type': 0}, 'set RM:NeOut_EQ/Ch/Band/Type 1 5 0\n',
     device_reply='OK set RM:NeOut_EQ/Ch/Band/Type 1 5 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_eq_band_type', {'channel': 1, 'band': 1}, 'get RM:NeOut_EQ/Ch/Band/Type 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Ch/Band/Type 0 0 6\n', expect_result=val('6'), **RCG)
text(RM, 'set_mic_automix_type', {'channel': 2, 'type': 0}, 'set RM:Automix/Mixtype 1 0 0\n',
     device_reply='OK set RM:Automix/Mixtype 1 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_mic_automix_type', {'channel': 1}, 'get RM:Automix/Mixtype 0 0\n',
     device_reply='OK get RM:Automix/Mixtype 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_mic_agc_type', {'type': 0}, 'set RM:Mic_Agc/Agctype 0 0 0\n',
     device_reply='OK set RM:Mic_Agc/Agctype 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_mic_agc_type', {}, 'get RM:Mic_Agc/Agctype 0 0\n',
     device_reply='OK get RM:Mic_Agc/Agctype 0 0 2\n', expect_result=val('2'), **RCG)
text(RM, 'set_mic_agc_speed', {'speed': 0}, 'set RM:Mic_Agc/AgcSpeed 0 0 0\n',
     device_reply='OK set RM:Mic_Agc/AgcSpeed 0 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_mic_agc_speed', {}, 'get RM:Mic_Agc/AgcSpeed 0 0\n',
     device_reply='OK get RM:Mic_Agc/AgcSpeed 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_near_end_output_mute', {'channel': 2, 'muted': True}, 'set RM:NeOut_Mute/Ch/On 1 0 0\n',
     device_reply='OK set RM:NeOut_Mute/Ch/On 1 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_near_end_output_mute', {'channel': 1}, 'get RM:NeOut_Mute/Ch/On 0 0\n',
     device_reply='OK get RM:NeOut_Mute/Ch/On 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_mute_link_enable', {'channel': 32, 'enabled': True}, 'set RM:MicMute/Link/Enable 31 0 1\n',
     device_reply='OK set RM:MicMute/Link/Enable 31 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_mute_link_enable', {'channel': 1}, 'get RM:MicMute/Link/Enable 0 0\n',
     device_reply='OK get RM:MicMute/Link/Enable 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_dante_output_patch', {'output': 2, 'source': 0}, 'set RM:DanteOut_Patch/Output/Input 1 0 0\n',
     device_reply='OK set RM:DanteOut_Patch/Output/Input 1 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_dante_output_patch', {'output': 1}, 'get RM:DanteOut_Patch/Output/Input 0 0\n',
     device_reply='OK get RM:DanteOut_Patch/Output/Input 0 0 2\n', expect_result=val('2'), **RCG)
text(RM, 'set_beam_focus_on', {'area': 2, 'enabled': True}, 'set RM:Mic_Beam/Focus/Ch/On 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Focus/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_focus_on', {'area': 1}, 'get RM:Mic_Beam/Focus/Ch/On 0 0\n',
     device_reply='OK get RM:Mic_Beam/Focus/Ch/On 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_beam_focus_x', {'area': 2, 'value': -39}, 'set RM:Mic_Beam/Focus/Ch/X 1 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Focus/Ch/X 1 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_focus_x', {'area': 1}, 'get RM:Mic_Beam/Focus/Ch/X 0 0\n',
     device_reply='OK get RM:Mic_Beam/Focus/Ch/X 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_focus_y', {'area': 2, 'value': -39}, 'set RM:Mic_Beam/Focus/Ch/Y 1 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Focus/Ch/Y 1 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_focus_y', {'area': 1}, 'get RM:Mic_Beam/Focus/Ch/Y 0 0\n',
     device_reply='OK get RM:Mic_Beam/Focus/Ch/Y 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_focus_width', {'area': 2, 'value': 1}, 'set RM:Mic_Beam/Focus/Ch/Width 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Focus/Ch/Width 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_focus_width', {'area': 1}, 'get RM:Mic_Beam/Focus/Ch/Width 0 0\n',
     device_reply='OK get RM:Mic_Beam/Focus/Ch/Width 0 0 80\n', expect_result=val('80'), **RCG)
text(RM, 'set_beam_focus_depth', {'area': 2, 'value': 1}, 'set RM:Mic_Beam/Focus/Ch/Depth 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Focus/Ch/Depth 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_focus_depth', {'area': 1}, 'get RM:Mic_Beam/Focus/Ch/Depth 0 0\n',
     device_reply='OK get RM:Mic_Beam/Focus/Ch/Depth 0 0 80\n', expect_result=val('80'), **RCG)
text(RM, 'set_beam_exclusion_on', {'area': 2, 'enabled': True}, 'set RM:Mic_Beam/Exclusion/Ch/On 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Exclusion/Ch/On 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_exclusion_on', {'area': 1}, 'get RM:Mic_Beam/Exclusion/Ch/On 0 0\n',
     device_reply='OK get RM:Mic_Beam/Exclusion/Ch/On 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_beam_exclusion_x', {'area': 2, 'value': -39}, 'set RM:Mic_Beam/Exclusion/Ch/X 1 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Exclusion/Ch/X 1 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_exclusion_x', {'area': 1}, 'get RM:Mic_Beam/Exclusion/Ch/X 0 0\n',
     device_reply='OK get RM:Mic_Beam/Exclusion/Ch/X 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_exclusion_y', {'area': 2, 'value': -39}, 'set RM:Mic_Beam/Exclusion/Ch/Y 1 0 -39\n',
     device_reply='OK set RM:Mic_Beam/Exclusion/Ch/Y 1 0 -39 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_exclusion_y', {'area': 1}, 'get RM:Mic_Beam/Exclusion/Ch/Y 0 0\n',
     device_reply='OK get RM:Mic_Beam/Exclusion/Ch/Y 0 0 39\n', expect_result=val('39'), **RCG)
text(RM, 'set_beam_exclusion_width', {'area': 2, 'value': 1}, 'set RM:Mic_Beam/Exclusion/Ch/Width 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Exclusion/Ch/Width 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_exclusion_width', {'area': 1}, 'get RM:Mic_Beam/Exclusion/Ch/Width 0 0\n',
     device_reply='OK get RM:Mic_Beam/Exclusion/Ch/Width 0 0 80\n', expect_result=val('80'), **RCG)
text(RM, 'set_beam_exclusion_depth', {'area': 2, 'value': 1}, 'set RM:Mic_Beam/Exclusion/Ch/Depth 1 0 1\n',
     device_reply='OK set RM:Mic_Beam/Exclusion/Ch/Depth 1 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_beam_exclusion_depth', {'area': 1}, 'get RM:Mic_Beam/Exclusion/Ch/Depth 0 0\n',
     device_reply='OK get RM:Mic_Beam/Exclusion/Ch/Depth 0 0 80\n', expect_result=val('80'), **RCG)
text(RM, 'set_led_mute_pattern', {'state': 2, 'pattern': 0}, 'set RM:Led/Config/Pattern 1 0 0\n',
     device_reply='OK set RM:Led/Config/Pattern 1 0 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_led_mute_pattern', {'state': 1}, 'get RM:Led/Config/Pattern 0 0\n',
     device_reply='OK get RM:Led/Config/Pattern 0 0 3\n', expect_result=val('3'), **RCG)
text(RM, 'set_led_mute_color_red', {'state': 2, 'led': 2, 'value': 0}, 'set RM:Led/Config/Color/R 1 1 0\n',
     device_reply='OK set RM:Led/Config/Color/R 1 1 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_led_mute_color_red', {'state': 1, 'led': 1}, 'get RM:Led/Config/Color/R 0 0\n',
     device_reply='OK get RM:Led/Config/Color/R 0 0 255\n', expect_result=val('255'), **RCG)
text(RM, 'set_led_mute_color_green', {'state': 2, 'led': 2, 'value': 0}, 'set RM:Led/Config/Color/G 1 1 0\n',
     device_reply='OK set RM:Led/Config/Color/G 1 1 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_led_mute_color_green', {'state': 1, 'led': 1}, 'get RM:Led/Config/Color/G 0 0\n',
     device_reply='OK get RM:Led/Config/Color/G 0 0 255\n', expect_result=val('255'), **RCG)
text(RM, 'set_led_mute_color_blue', {'state': 2, 'led': 2, 'value': 0}, 'set RM:Led/Config/Color/B 1 1 0\n',
     device_reply='OK set RM:Led/Config/Color/B 1 1 0 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_led_mute_color_blue', {'state': 1, 'led': 1}, 'get RM:Led/Config/Color/B 0 0\n',
     device_reply='OK get RM:Led/Config/Color/B 0 0 255\n', expect_result=val('255'), **RCG)
text(RM, 'set_voice_lift_reset', {'reset': True}, 'set RM:AutoMicEq/Reset_oneshot 0 0 1\n',
     device_reply='OK set RM:AutoMicEq/Reset_oneshot 0 0 1 "x"\n', expect_result=OK, **RCG)
text(RM, 'get_voice_lift_reset', {}, 'get RM:AutoMicEq/Reset_oneshot 0 0\n',
     device_reply='OK get RM:AutoMicEq/Reset_oneshot 0 0 1\n', expect_result=val('1'), **RCG)
text(RM, 'set_directivity_mode', {'mode': 0}, 'set RM:Mic_Direcctl/Mode 0 0 0\n',
     device_reply='OK set RM:Mic_Direcctl/Mode 0 0 0 "x"\n', expect_result=OK, **RTT)
text(RM, 'get_directivity_mode', {}, 'get RM:Mic_Direcctl/Mode 0 0\n',
     device_reply='OK get RM:Mic_Direcctl/Mode 0 0 6\n', expect_result=val('6'), **RTT)
text(RM, 'set_directivity_angle', {'mic': 4, 'angle': -180}, 'set RM:Mic_Direcctl/Ch/Angle 3 0 -180\n',
     device_reply='OK set RM:Mic_Direcctl/Ch/Angle 3 0 -180 "x"\n', expect_result=OK, **RTT)
text(RM, 'get_directivity_angle', {'mic': 1}, 'get RM:Mic_Direcctl/Ch/Angle 0 0\n',
     device_reply='OK get RM:Mic_Direcctl/Ch/Angle 0 0 180\n', expect_result=val('180'), **RTT)
text(RM, 'set_directivity_mic_on', {'mic': 4, 'enabled': True}, 'set RM:Mic_Direcctl/Ch/On 3 0 1\n',
     device_reply='OK set RM:Mic_Direcctl/Ch/On 3 0 1 "x"\n', expect_result=OK, **RTT)
text(RM, 'get_directivity_mic_on', {'mic': 1}, 'get RM:Mic_Direcctl/Ch/On 0 0\n',
     device_reply='OK get RM:Mic_Direcctl/Ch/On 0 0 1\n', expect_result=val('1'), **RTT)
text(RM, 'set_wap_unified_comms_lpf_type', {'unit': 32, 'type': 0}, 'set RM:Mic_Dsp/Id/LpfType 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/LpfType 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_unified_comms_lpf_type', {'unit': 1}, 'get RM:Mic_Dsp/Id/LpfType 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/LpfType 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_unified_comms_hpf_type', {'unit': 32, 'type': 0}, 'set RM:Mic_Dsp/Id/HpfType 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/HpfType 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_unified_comms_hpf_type', {'unit': 1}, 'get RM:Mic_Dsp/Id/HpfType 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/HpfType 0 0 4\n', expect_result=val('4'), **RW16)
text(RM, 'set_wap_unified_comms_gain', {'unit': 32, 'gain': -128}, 'set RM:NeOut_Gain/Id/Level 31 0 -128\n',
     device_reply='OK set RM:NeOut_Gain/Id/Level 31 0 -128 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_unified_comms_gain', {'unit': 1}, 'get RM:NeOut_Gain/Id/Level 0 0\n',
     device_reply='OK get RM:NeOut_Gain/Id/Level 0 0 12\n', expect_result=val('12'), **RW16)
text(RM, 'set_wap_low_latency_lpf_type', {'unit': 32, 'type': 0}, 'set RM:MicLL_Dsp/Id/LpfType 31 0 0\n',
     device_reply='OK set RM:MicLL_Dsp/Id/LpfType 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_low_latency_lpf_type', {'unit': 1}, 'get RM:MicLL_Dsp/Id/LpfType 0 0\n',
     device_reply='OK get RM:MicLL_Dsp/Id/LpfType 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_low_latency_hpf_type', {'unit': 32, 'type': 0}, 'set RM:MicLL_Dsp/Id/HpfType 31 0 0\n',
     device_reply='OK set RM:MicLL_Dsp/Id/HpfType 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_low_latency_hpf_type', {'unit': 1}, 'get RM:MicLL_Dsp/Id/HpfType 0 0\n',
     device_reply='OK get RM:MicLL_Dsp/Id/HpfType 0 0 4\n', expect_result=val('4'), **RW16)
text(RM, 'set_wap_low_latency_gain', {'unit': 32, 'gain': -128}, 'set RM:NeOutLL_Gain/Id/Level 31 0 -128\n',
     device_reply='OK set RM:NeOutLL_Gain/Id/Level 31 0 -128 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_low_latency_gain', {'unit': 1}, 'get RM:NeOutLL_Gain/Id/Level 0 0\n',
     device_reply='OK get RM:NeOutLL_Gain/Id/Level 0 0 12\n', expect_result=val('12'), **RW16)
text(RM, 'set_wap_dante_output_patch', {'output': 16, 'source': 0}, 'set RM:DanteOut_Patch/Output/Id 15 0 0\n',
     device_reply='OK set RM:DanteOut_Patch/Output/Id 15 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_dante_output_patch', {'output': 1}, 'get RM:DanteOut_Patch/Output/Id 0 0\n',
     device_reply='OK get RM:DanteOut_Patch/Output/Id 0 0 66\n', expect_result=val('66'), **RW16)
text(RM, 'set_wap_input_selector', {'mic': 32, 'enabled': True}, 'set RM:In_Selector/Input/Id 31 0 1\n',
     device_reply='OK set RM:In_Selector/Input/Id 31 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_input_selector', {'mic': 1}, 'get RM:In_Selector/Input/Id 0 0\n',
     device_reply='OK get RM:In_Selector/Input/Id 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_gating_automix_ll_type', {'type': 0}, 'set RM:AutomixLL/Type/Type 0 0 0\n',
     device_reply='OK set RM:AutomixLL/Type/Type 0 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_type', {}, 'get RM:AutomixLL/Type/Type 0 0\n',
     device_reply='OK get RM:AutomixLL/Type/Type 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_gating_automix_ll_last_mic_on', {'enabled': True}, 'set RM:GatingAutomixLL/Settings/LastMicOn 0 0 1\n',
     device_reply='OK set RM:GatingAutomixLL/Settings/LastMicOn 0 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_last_mic_on', {}, 'get RM:GatingAutomixLL/Settings/LastMicOn 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Settings/LastMicOn 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_gating_automix_ll_open_mics', {'count': 1}, 'set RM:GatingAutomixLL/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:GatingAutomixLL/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_open_mics', {}, 'get RM:GatingAutomixLL/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Settings/NumOfOpenMic 0 0 16\n', expect_result=val('16'), **RW16)
text(RM, 'set_gating_automix_ll_threshold', {'threshold': -7200}, 'set RM:GatingAutomixLL/Settings/Threshold 0 0 -7200\n',
     device_reply='OK set RM:GatingAutomixLL/Settings/Threshold 0 0 -7200 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_threshold', {}, 'get RM:GatingAutomixLL/Settings/Threshold 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Settings/Threshold 0 0 0\n', expect_result=val('0'), **RW16)
text(RM, 'set_gating_automix_ll_range', {'range': -7000}, 'set RM:GatingAutomixLL/Settings/Range 0 0 -7000\n',
     device_reply='OK set RM:GatingAutomixLL/Settings/Range 0 0 -7000 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_range', {}, 'get RM:GatingAutomixLL/Settings/Range 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Settings/Range 0 0 0\n', expect_result=val('0'), **RW16)
text(RM, 'set_gating_automix_ll_hold', {'hold': 20}, 'set RM:GatingAutomixLL/Settings/Hold 0 0 20\n',
     device_reply='OK set RM:GatingAutomixLL/Settings/Hold 0 0 20 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_hold', {}, 'get RM:GatingAutomixLL/Settings/Hold 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Settings/Hold 0 0 1960000\n', expect_result=val('1960000'), **RW16)
text(RM, 'set_gating_automix_ll_priority_mic', {'channel': 16, 'enabled': True}, 'set RM:GatingAutomixLL/Ch/PriorityMic 15 0 1\n',
     device_reply='OK set RM:GatingAutomixLL/Ch/PriorityMic 15 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gating_automix_ll_priority_mic', {'channel': 1}, 'get RM:GatingAutomixLL/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:GatingAutomixLL/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_gain_sharing_automix_ll_open_mics', {'count': 1}, 'set RM:GainSharingAutomixLL/Settings/NumOfOpenMic 0 0 1\n',
     device_reply='OK set RM:GainSharingAutomixLL/Settings/NumOfOpenMic 0 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gain_sharing_automix_ll_open_mics', {}, 'get RM:GainSharingAutomixLL/Settings/NumOfOpenMic 0 0\n',
     device_reply='OK get RM:GainSharingAutomixLL/Settings/NumOfOpenMic 0 0 16\n', expect_result=val('16'), **RW16)
text(RM, 'set_gain_sharing_automix_ll_priority_mic', {'channel': 16, 'enabled': True}, 'set RM:GainSharingAutomixLL/Ch/PriorityMic 15 0 1\n',
     device_reply='OK set RM:GainSharingAutomixLL/Ch/PriorityMic 15 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gain_sharing_automix_ll_priority_mic', {'channel': 1}, 'get RM:GainSharingAutomixLL/Ch/PriorityMic 0 0\n',
     device_reply='OK get RM:GainSharingAutomixLL/Ch/PriorityMic 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_gain_sharing_automix_ll_weight', {'channel': 16, 'weight': -3000}, 'set RM:GainSharingAutomixLL/Ch/Weight 15 0 -3000\n',
     device_reply='OK set RM:GainSharingAutomixLL/Ch/Weight 15 0 -3000 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_gain_sharing_automix_ll_weight', {'channel': 1}, 'get RM:GainSharingAutomixLL/Ch/Weight 0 0\n',
     device_reply='OK get RM:GainSharingAutomixLL/Ch/Weight 0 0 1500\n', expect_result=val('1500'), **RW16)
text(RM, 'set_mute_link_ll_enable', {'unit': 32, 'enabled': True}, 'set RM:MicMuteLL/Link/Enable 31 0 1\n',
     device_reply='OK set RM:MicMuteLL/Link/Enable 31 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_mute_link_ll_enable', {'unit': 1}, 'get RM:MicMuteLL/Link/Enable 0 0\n',
     device_reply='OK get RM:MicMuteLL/Link/Enable 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_wap_near_end_mute', {'unit': 32, 'muted': True}, 'set RM:NeOut_Mute/Id/On 31 0 1\n',
     device_reply='OK set RM:NeOut_Mute/Id/On 31 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_mute', {'unit': 1}, 'get RM:NeOut_Mute/Id/On 0 0\n',
     device_reply='OK get RM:NeOut_Mute/Id/On 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'get_wap_mic_battery_level', {'unit': 1}, 'get RM:Mic_Battery/Id/Level 0 0\n',
     device_reply='OK get RM:Mic_Battery/Id/Level 0 0 100\n', expect_result=val('100'), **RW16)
text(RM, 'get_wap_mic_packet_error_count', {'unit': 1}, 'get RM:Mic_LinkQuality/Id/PacketErrCount 0 0\n',
     device_reply='OK get RM:Mic_LinkQuality/Id/PacketErrCount 0 0 2147483647\n', expect_result=val('2147483647'), **RW16)
text(RM, 'set_wap_mic_gain_type', {'unit': 32, 'type': 0}, 'set RM:Mic_Dsp/Id/MicGaintype 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/MicGaintype 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_mic_gain_type', {'unit': 1}, 'get RM:Mic_Dsp/Id/MicGaintype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/MicGaintype 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_echo_cancellation_level', {'unit': 32, 'level': 0}, 'set RM:Mic_Dsp/Id/Aectype 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/Aectype 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_echo_cancellation_level', {'unit': 1}, 'get RM:Mic_Dsp/Id/Aectype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/Aectype 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_noise_reduction_level', {'unit': 32, 'level': 0}, 'set RM:Mic_Dsp/Id/Nrtype 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/Nrtype 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_noise_reduction_level', {'unit': 1}, 'get RM:Mic_Dsp/Id/Nrtype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/Nrtype 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_dereverb_level', {'unit': 32, 'level': 0}, 'set RM:Mic_Dsp/Id/Derevtype 31 0 0\n',
     device_reply='OK set RM:Mic_Dsp/Id/Derevtype 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_dereverb_level', {'unit': 1}, 'get RM:Mic_Dsp/Id/Derevtype 0 0\n',
     device_reply='OK get RM:Mic_Dsp/Id/Derevtype 0 0 3\n', expect_result=val('3'), **RW16)
text(RM, 'set_wap_near_end_eq_on', {'unit': 32, 'enabled': True}, 'set RM:NeOut_EQ/Id/On 31 0 1\n',
     device_reply='OK set RM:NeOut_EQ/Id/On 31 0 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_on', {'unit': 1}, 'get RM:NeOut_EQ/Id/On 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/On 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_wap_near_end_eq_band_bypass', {'unit': 32, 'band': 6, 'bypass': True}, 'set RM:NeOut_EQ/Id/Band/Bypass 31 5 1\n',
     device_reply='OK set RM:NeOut_EQ/Id/Band/Bypass 31 5 1 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_band_bypass', {'unit': 1, 'band': 1}, 'get RM:NeOut_EQ/Id/Band/Bypass 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/Band/Bypass 0 0 1\n', expect_result=val('1'), **RW16)
text(RM, 'set_wap_near_end_eq_band_frequency', {'unit': 32, 'band': 6, 'frequency': 200}, 'set RM:NeOut_EQ/Id/Band/Frequency 31 5 200\n',
     device_reply='OK set RM:NeOut_EQ/Id/Band/Frequency 31 5 200 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_band_frequency', {'unit': 1, 'band': 1}, 'get RM:NeOut_EQ/Id/Band/Frequency 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/Band/Frequency 0 0 200000\n', expect_result=val('200000'), **RW16)
text(RM, 'set_wap_near_end_eq_band_gain', {'unit': 32, 'band': 6, 'gain': -1800}, 'set RM:NeOut_EQ/Id/Band/Gain 31 5 -1800\n',
     device_reply='OK set RM:NeOut_EQ/Id/Band/Gain 31 5 -1800 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_band_gain', {'unit': 1, 'band': 1}, 'get RM:NeOut_EQ/Id/Band/Gain 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/Band/Gain 0 0 1800\n', expect_result=val('1800'), **RW16)
text(RM, 'set_wap_near_end_eq_band_q', {'unit': 32, 'band': 6, 'q': 100}, 'set RM:NeOut_EQ/Id/Band/Q 31 5 100\n',
     device_reply='OK set RM:NeOut_EQ/Id/Band/Q 31 5 100 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_band_q', {'unit': 1, 'band': 1}, 'get RM:NeOut_EQ/Id/Band/Q 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/Band/Q 0 0 16000\n', expect_result=val('16000'), **RW16)
text(RM, 'set_wap_near_end_eq_band_type', {'unit': 32, 'band': 6, 'type': 0}, 'set RM:NeOut_EQ/Id/Band/Type 31 5 0\n',
     device_reply='OK set RM:NeOut_EQ/Id/Band/Type 31 5 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_near_end_eq_band_type', {'unit': 1, 'band': 1}, 'get RM:NeOut_EQ/Id/Band/Type 0 0\n',
     device_reply='OK get RM:NeOut_EQ/Id/Band/Type 0 0 6\n', expect_result=val('6'), **RW16)
text(RM, 'set_wap_agc_type', {'unit': 32, 'type': 0}, 'set RM:Mic_Agc/Id/Agctype 31 0 0\n',
     device_reply='OK set RM:Mic_Agc/Id/Agctype 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_agc_type', {'unit': 1}, 'get RM:Mic_Agc/Id/Agctype 0 0\n',
     device_reply='OK get RM:Mic_Agc/Id/Agctype 0 0 2\n', expect_result=val('2'), **RW16)
text(RM, 'set_wap_agc_speed', {'unit': 32, 'speed': 0}, 'set RM:Mic_Agc/Id/AgcSpeed 31 0 0\n',
     device_reply='OK set RM:Mic_Agc/Id/AgcSpeed 31 0 0 "x"\n', expect_result=OK, **RW16)
text(RM, 'get_wap_agc_speed', {'unit': 1}, 'get RM:Mic_Agc/Id/AgcSpeed 0 0\n',
     device_reply='OK get RM:Mic_Agc/Id/AgcSpeed 0 0 1\n', expect_result=val('1'), **RW16)

# Telemetry: device status, events, meters and snapshots
telemetry(RM, 'run-mode', inbound='NOTIFY devstatus runmode "normal"\n', expect_state={'device': {'run_mode': 'normal'}}, expect_connect_wire=['devstatus runmode\n'])
telemetry(RM, 'run-mode-reply', inbound='OK devstatus runmode "update"\n', expect_state={'device': {'run_mode': 'update'}})
telemetry(RM, 'error-status', inbound='NOTIFY devstatus error "fault"\n', expect_state={'device': {'error': 'fault'}})
telemetry(RM, 'protocol-version', inbound='OK devinfo protocolver "1.0.0"\n', expect_state={'device': {'protocol_version': '1.0.0'}})
telemetry(RM, 'parameter-set-version', inbound='OK devinfo paramsetver "RM:1.0.0"\n', expect_state={'device': {'parameter_set_version': 'RM:1.0.0'}})
telemetry(RM, 'firmware', inbound='OK devinfo version "3.0.0"\n', expect_state={'device': {'firmware': '3.0.0'}})
telemetry(RM, 'product-name', inbound='OK devinfo productname "RM-CR"\n', expect_state={'device': {'product_name': 'RM-CR'}})
telemetry(RM, 'serial', inbound='OK devinfo serialno "S7A001001"\n', expect_state={'device': {'serial': 'S7A001001'}})
telemetry(RM, 'category', inbound='OK devinfo category "microphone"\n', expect_state={'device': {'category': 'microphone'}})
telemetry(RM, 'device-id', inbound='OK devinfo deviceid "001"\n', expect_state={'device': {'device_id': '001'}})
telemetry(RM, 'device-name', inbound='OK devinfo devicename "Y001-Yamaha-RM-CR-061281"\n', expect_state={'device': {'name': 'Y001-Yamaha-RM-CR-061281'}})
telemetry(RM, 'manufacturer', inbound='OK devinfo manufacturer "Yamaha Corporation"\n', expect_state={'device': {'manufacturer': 'Yamaha Corporation'}})
telemetry(RM, 'parameter-count', inbound='OK prmnum 114\n', expect_state={'device': {'parameter_count': 114}})
telemetry(RM, 'meter-count', inbound='OK mtrnum 16\n', expect_state={'device': {'meter_count': 16}})
telemetry(RM, 'alert', inbound='NOTIFY event RM:Alert "01:SYSTEM ERROR,fault"\n', expect_state={'alerts': {'last_number': '01', 'last_message': 'SYSTEM ERROR', 'last_severity': 'fault'}})
telemetry(RM, 'firmware-update-started', inbound='NOTIFY event RM:FirmwareUpdateStarted\n', expect_state={'firmware_update': {'started': True}})
telemetry(RM, 'firmware-update-finished', inbound='NOTIFY event RM:FirmwareUpdateFinished "success"\n', expect_state={'firmware_update': {'result': 'success'}})
telemetry(RM, 'time-zone', inbound='OK event RM:GetTimeZone "12"\n', expect_state={'clock': {'time_zone': 12}})
telemetry(RM, 'dst-enabled', inbound='OK event RM:SetDstEnable "Enable"\n', expect_state={'clock': {'dst_enabled': True}})
telemetry(RM, 'dst-start', inbound='OK event RM:GetDstStartTime "month=3,week=5,day=0,hour=2"\n', expect_state={'clock': {'dst_start': 'month=3,week=5,day=0,hour=2'}})
telemetry(RM, 'dst-end', inbound='OK event RM:SetDstEndTime "month=10,week=5,day=0,hour=3"\n', expect_state={'clock': {'dst_end': 'month=10,week=5,day=0,hour=3'}})
telemetry(RM, 'ntp-enabled', inbound='OK event RM:GetNtpEnable "Disable"\n', expect_state={'clock': {'ntp_enabled': False}})
telemetry(RM, 'ntp-server', inbound='OK event RM:GetNtpServer2 "pool.ntp.org"\n', expect_state={'clock': {'ntp_servers': {'2': 'pool.ntp.org'}}})
telemetry(RM, 'meter', inbound='NOTIFY mtr RM:FeInPostFader level 00 00 2d 2e 00 00 00 00\n', expect_state={'meters': {'FeInPostFader': {'type': 'level', 'values': '00 00 2d 2e 00 00 00 00'}}})
telemetry(RM, 'meter-untyped', inbound='NOTIFY mtr RM:InputPort 00 00 00 3f 54 22\n', expect_state={'meters': {'InputPort': {'values': '00 00 00 3f 54 22'}}})
telemetry(RM, 'snapshot-current', inbound='OK sscurrent_ex config 1 modified\n', expect_state={'snapshots': {'current': 1, 'modified': True}})
telemetry(RM, 'snapshot-current-push', inbound='NOTIFY sscurrent_ex config 3\n', expect_state={'snapshots': {'current': 3}})
telemetry(RM, 'snapshot-recalled', inbound='NOTIFY ssrecall_ex config 1\n', expect_state={'snapshots': {'last_recalled': 1}})
telemetry(RM, 'snapshot-updated', inbound='NOTIFY ssupdate_ex config 1\n', expect_state={'snapshots': {'last_updated': 1}})
telemetry(RM, 'snapshot-count', inbound='OK ssnum_ex config 11\n', expect_state={'snapshots': {'count': 11}})
telemetry(RM, 'snapshot-info', inbound='OK ssinfo_ex config 1 "1" "Meeting" "" user\n', expect_state={'snapshots': {'1': {'number_text': '1', 'title': 'Meeting', 'attribute': 'user'}}})
telemetry(RM, 'call-status', inbound='NOTIFY event rm:changedcallstatus "sip1=active"\n', expect_state={'calls': {'status': {'sip1': 'active'}}})
telemetry(RM, 'call-config', inbound='NOTIFY event rm:changedcallconfig "vol=10"\n', expect_state={'calls': {'config': {'vol': '10'}}})
telemetry(RM, 'call-recent', inbound='NOTIFY event rm: callrecent "3=number=1234,type=out"\n', expect_state={'calls': {'recents': {'3': 'number=1234,type=out'}}})
telemetry(RM, 'call-contact', inbound='NOTIFY event rm: changedcallcontacts "1=name=Desk,mobile=1234"\n', expect_state={'calls': {'contacts': {'1': 'name=Desk,mobile=1234'}}})
telemetry(RM, 'bluetooth-status', inbound='NOTIFY event rm: bluetoothstatus "pairing"\n', expect_state={'bluetooth': {'status': 'pairing'}})
telemetry(RM, 'dect-status', inbound='NOTIFY event rm:dectstatus "idle"\n', expect_state={'dect': {'status': 'idle'}})
telemetry(RM, 'error-not-state', inbound='ERROR set UnknownAddress\n', expect_state={})

# Telemetry: one NOTIFY set per section 7 parameter (the pushed form; OK/OKm
# set and OK get replies carry the same data)
telemetry(RM, 'far-end-input-eq-on', inbound='NOTIFY set RM:FeIn_EQ/Ch/On/On 1 0 1 "x"\n', expect_state={'far_end_inputs': {'2': {'eq_on': True}}})
telemetry(RM, 'far-end-input-eq-band-bypass', inbound='NOTIFY set RM:FeIn_EQ/Ch/Band/Bypass 1 1 1 "x"\n', expect_state={'far_end_inputs': {'2': {'eq': {'2': {'bypass': True}}}}})
telemetry(RM, 'far-end-input-eq-band-frequency', inbound='NOTIFY set RM:FeIn_EQ/Ch/Band/Frequency 1 1 201 "x"\n', expect_state={'far_end_inputs': {'2': {'eq': {'2': {'frequency': 201}}}}})
telemetry(RM, 'far-end-input-eq-band-gain', inbound='NOTIFY set RM:FeIn_EQ/Ch/Band/Gain 1 1 -1799 "x"\n', expect_state={'far_end_inputs': {'2': {'eq': {'2': {'gain': -1799}}}}})
telemetry(RM, 'far-end-input-eq-band-q', inbound='NOTIFY set RM:FeIn_EQ/Ch/Band/Q 1 1 101 "x"\n', expect_state={'far_end_inputs': {'2': {'eq': {'2': {'q': 101}}}}})
telemetry(RM, 'far-end-input-eq-band-type', inbound='NOTIFY set RM:FeIn_EQ/Ch/Band/Type 1 1 1 "x"\n', expect_state={'far_end_inputs': {'2': {'eq': {'2': {'type': 1}}}}})
telemetry(RM, 'far-end-input-agc-on', inbound='NOTIFY set RM:FeIn_AGC/Ch/On 1 0 1 "x"\n', expect_state={'far_end_inputs': {'2': {'agc_on': True}}})
telemetry(RM, 'far-end-input-agc-target-level', inbound='NOTIFY set RM:FeIn_AGC/Ch/TargetLevel 1 0 -3999 "x"\n', expect_state={'far_end_inputs': {'2': {'agc_target_level': -3999}}})
telemetry(RM, 'far-end-input-agc-max-gain', inbound='NOTIFY set RM:FeIn_AGC/Ch/MaxGain 1 0 1 "x"\n', expect_state={'far_end_inputs': {'2': {'agc_max_gain': 1}}})
telemetry(RM, 'far-end-input-agc-min-gain', inbound='NOTIFY set RM:FeIn_AGC/Ch/MinGain 1 0 -1999 "x"\n', expect_state={'far_end_inputs': {'2': {'agc_min_gain': -1999}}})
telemetry(RM, 'far-end-input-agc-noise-gate-on', inbound='NOTIFY set RM:FeIn_AGC/Ch/NoiseGateOn 1 0 1 "x"\n', expect_state={'far_end_inputs': {'2': {'agc_noise_gate_on': True}}})
telemetry(RM, 'far-end-input-fader-on', inbound='NOTIFY set RM:FeIn_Fader/Ch/On 1 0 1 "x"\n', expect_state={'far_end_inputs': {'2': {'on': True}}})
telemetry(RM, 'far-end-input-fader-level', inbound='NOTIFY set RM:FeIn_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'far_end_inputs': {'2': {'level': -32767}}})
telemetry(RM, 'mic-input-eq-on', inbound='NOTIFY set RM:ExtMic_EQ/Ch/On/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'eq_on': True}}})
telemetry(RM, 'mic-input-eq-band-bypass', inbound='NOTIFY set RM:ExtMic_EQ/Ch/Band/Bypass 1 1 1 "x"\n', expect_state={'mic_inputs': {'2': {'eq': {'2': {'bypass': True}}}}})
telemetry(RM, 'mic-input-eq-band-frequency', inbound='NOTIFY set RM:ExtMic_EQ/Ch/Band/Frequency 1 1 201 "x"\n', expect_state={'mic_inputs': {'2': {'eq': {'2': {'frequency': 201}}}}})
telemetry(RM, 'mic-input-eq-band-gain', inbound='NOTIFY set RM:ExtMic_EQ/Ch/Band/Gain 1 1 -1799 "x"\n', expect_state={'mic_inputs': {'2': {'eq': {'2': {'gain': -1799}}}}})
telemetry(RM, 'mic-input-eq-band-q', inbound='NOTIFY set RM:ExtMic_EQ/Ch/Band/Q 1 1 101 "x"\n', expect_state={'mic_inputs': {'2': {'eq': {'2': {'q': 101}}}}})
telemetry(RM, 'mic-input-eq-band-type', inbound='NOTIFY set RM:ExtMic_EQ/Ch/Band/Type 1 1 1 "x"\n', expect_state={'mic_inputs': {'2': {'eq': {'2': {'type': 1}}}}})
telemetry(RM, 'mic-input-gate-on', inbound='NOTIFY set RM:ExtMic_Gate/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'gate_on': True}}})
telemetry(RM, 'mic-input-gate-threshold', inbound='NOTIFY set RM:ExtMic_Gate/Ch/Threshold 1 0 -7199 "x"\n', expect_state={'mic_inputs': {'2': {'gate_threshold': -7199}}})
telemetry(RM, 'mic-input-gate-range', inbound='NOTIFY set RM:ExtMic_Gate/Ch/Range 1 0 -6999 "x"\n', expect_state={'mic_inputs': {'2': {'gate_range': -6999}}})
telemetry(RM, 'mic-input-gate-attack', inbound='NOTIFY set RM:ExtMic_Gate/Ch/Attack 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'gate_attack': 1}}})
telemetry(RM, 'mic-input-gate-decay', inbound='NOTIFY set RM:ExtMic_Gate/Ch/Decay 1 0 3341 "x"\n', expect_state={'mic_inputs': {'2': {'gate_decay': 3341}}})
telemetry(RM, 'mic-input-gate-hold', inbound='NOTIFY set RM:ExtMic_Gate/Ch/Hold 1 0 21 "x"\n', expect_state={'mic_inputs': {'2': {'gate_hold': 21}}})
telemetry(RM, 'mic-input-comp-on', inbound='NOTIFY set RM:ExtMic_Comp/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'comp_on': True}}})
telemetry(RM, 'mic-input-comp-threshold', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Threshold 1 0 -5399 "x"\n', expect_state={'mic_inputs': {'2': {'comp_threshold': -5399}}})
telemetry(RM, 'mic-input-comp-ratio', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Ratio 1 0 11 "x"\n', expect_state={'mic_inputs': {'2': {'comp_ratio': 11}}})
telemetry(RM, 'mic-input-comp-knee', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Knee 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'comp_knee': 1}}})
telemetry(RM, 'mic-input-comp-attack', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Attack 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'comp_attack': 1}}})
telemetry(RM, 'mic-input-comp-release', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Release 1 0 3341 "x"\n', expect_state={'mic_inputs': {'2': {'comp_release': 3341}}})
telemetry(RM, 'mic-input-comp-gain', inbound='NOTIFY set RM:ExtMic_Comp/Ch/Gain 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'comp_gain': 1}}})
telemetry(RM, 'mic-input-fbs-on', inbound='NOTIFY set RM:ExtMic_FBS/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'fbs_on': True}}})
telemetry(RM, 'mic-input-agc-on', inbound='NOTIFY set RM:ExtMic_AGC/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'agc_on': True}}})
telemetry(RM, 'mic-input-agc-target-level', inbound='NOTIFY set RM:ExtMic_AGC/Ch/TargetLevel 1 0 -3999 "x"\n', expect_state={'mic_inputs': {'2': {'agc_target_level': -3999}}})
telemetry(RM, 'mic-input-agc-max-gain', inbound='NOTIFY set RM:ExtMic_AGC/Ch/MaxGain 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'agc_max_gain': 1}}})
telemetry(RM, 'mic-input-agc-min-gain', inbound='NOTIFY set RM:ExtMic_AGC/Ch/MinGain 1 0 -1999 "x"\n', expect_state={'mic_inputs': {'2': {'agc_min_gain': -1999}}})
telemetry(RM, 'mic-input-agc-noise-gate-on', inbound='NOTIFY set RM:ExtMic_AGC/Ch/NoiseGateOn 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'agc_noise_gate_on': True}}})
telemetry(RM, 'mic-input-fader-on', inbound='NOTIFY set RM:ExtMic_Fader/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'on': True}}})
telemetry(RM, 'mic-input-fader-level', inbound='NOTIFY set RM:ExtMic_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'mic_inputs': {'2': {'level': -32767}}})
telemetry(RM, 'mic-input-echo-suppressor-on', inbound='NOTIFY set RM:ExtMic_ES/Ch/On 1 0 1 "x"\n', expect_state={'mic_inputs': {'2': {'echo_suppressor_on': True}}})
telemetry(RM, 'near-end-input-fader-on', inbound='NOTIFY set RM:NeIn_Fader/Ch/On 1 0 1 "x"\n', expect_state={'near_end_inputs': {'2': {'on': True}}})
telemetry(RM, 'near-end-input-fader-level', inbound='NOTIFY set RM:NeIn_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'near_end_inputs': {'2': {'level': -32767}}})
telemetry(RM, 'automix-type', inbound='NOTIFY set RM:Automix/Type/Type 0 0 1 "x"\n', expect_state={'automix': {'type': 1}})
telemetry(RM, 'gating-automix-last-mic-on', inbound='NOTIFY set RM:GatingAutomix/Settings/LastMicOn 0 0 1 "x"\n', expect_state={'automix': {'gating': {'last_mic_on': True}}})
telemetry(RM, 'gating-automix-open-mics', inbound='NOTIFY set RM:GatingAutomix/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'automix': {'gating': {'open_mics': 2}}})
telemetry(RM, 'gating-automix-threshold', inbound='NOTIFY set RM:GatingAutomix/Settings/Threshold 0 0 -7199 "x"\n', expect_state={'automix': {'gating': {'threshold': -7199}}})
telemetry(RM, 'gating-automix-range', inbound='NOTIFY set RM:GatingAutomix/Settings/Range 0 0 -6999 "x"\n', expect_state={'automix': {'gating': {'range': -6999}}})
telemetry(RM, 'gating-automix-hold', inbound='NOTIFY set RM:GatingAutomix/Settings/Hold 0 0 21 "x"\n', expect_state={'automix': {'gating': {'hold': 21}}})
telemetry(RM, 'gating-automix-priority-mic', inbound='NOTIFY set RM:GatingAutomix/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'automix': {'gating': {'channels': {'2': {'priority': True}}}}})
telemetry(RM, 'gain-sharing-automix-open-mics', inbound='NOTIFY set RM:GainSharingAutomix/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'automix': {'gain_sharing': {'open_mics': 2}}})
telemetry(RM, 'gain-sharing-automix-priority-mic', inbound='NOTIFY set RM:GainSharingAutomix/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'automix': {'gain_sharing': {'channels': {'2': {'priority': True}}}}})
telemetry(RM, 'near-end-input-agc-on', inbound='NOTIFY set RM:NeIn_AGC/Ch/On 0 0 1 "x"\n', expect_state={'near_end_input': {'agc_on': True}})
telemetry(RM, 'near-end-input-agc-target-level', inbound='NOTIFY set RM:NeIn_AGC/Ch/TargetLevel 0 0 -3999 "x"\n', expect_state={'near_end_input': {'agc_target_level': -3999}})
telemetry(RM, 'near-end-input-agc-max-gain', inbound='NOTIFY set RM:NeIn_AGC/Ch/MaxGain 0 0 1 "x"\n', expect_state={'near_end_input': {'agc_max_gain': 1}})
telemetry(RM, 'near-end-input-agc-min-gain', inbound='NOTIFY set RM:NeIn_AGC/Ch/MinGain 0 0 -1999 "x"\n', expect_state={'near_end_input': {'agc_min_gain': -1999}})
telemetry(RM, 'near-end-input-agc-noise-gate-on', inbound='NOTIFY set RM:NeIn_AGC/Ch/NoiseGateOn 0 0 1 "x"\n', expect_state={'near_end_input': {'agc_noise_gate_on': True}})
telemetry(RM, 'near-end-input-ducker-on', inbound='NOTIFY set RM:NeIn_Ducker/Ch/On 0 0 1 "x"\n', expect_state={'near_end_input': {'ducker_on': True}})
telemetry(RM, 'mix-bus-on', inbound='NOTIFY set RM:MixBus/Input/Output/On 1 1 1 "x"\n', expect_state={'mix_bus': {'2': {'2': {'on': True}}}})
telemetry(RM, 'mix-bus-level', inbound='NOTIFY set RM:MixBus/Input/Output/Level 1 1 -32767 "x"\n', expect_state={'mix_bus': {'2': {'2': {'level': -32767}}}})
telemetry(RM, 'mix-bus-delay-on', inbound='NOTIFY set RM:MixBus/Input/Output/DelayOn 1 1 1 "x"\n', expect_state={'mix_bus': {'2': {'2': {'delay_on': True}}}})
telemetry(RM, 'mix-bus-delay-time', inbound='NOTIFY set RM:MixBus/Input/Output/DelayTime 1 1 1 "x"\n', expect_state={'mix_bus': {'2': {'2': {'delay_time': 1}}}})
telemetry(RM, 'far-end-output-fader-on', inbound='NOTIFY set RM:FeOut_Fader/Ch/On 1 0 1 "x"\n', expect_state={'far_end_outputs': {'2': {'on': True}}})
telemetry(RM, 'far-end-output-fader-level', inbound='NOTIFY set RM:FeOut_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'far_end_outputs': {'2': {'level': -32767}}})
telemetry(RM, 'room-eq-on', inbound='NOTIFY set RM:RoomEQ/Ch/On/On 1 0 1 "x"\n', expect_state={'room_eq': {'2': {'on': True}}})
telemetry(RM, 'room-eq-band-bypass', inbound='NOTIFY set RM:RoomEQ/Ch/Band/Bypass 1 1 1 "x"\n', expect_state={'room_eq': {'2': {'bands': {'2': {'bypass': True}}}}})
telemetry(RM, 'room-eq-band-frequency', inbound='NOTIFY set RM:RoomEQ/Ch/Band/Frequency 1 1 201 "x"\n', expect_state={'room_eq': {'2': {'bands': {'2': {'frequency': 201}}}}})
telemetry(RM, 'room-eq-band-gain', inbound='NOTIFY set RM:RoomEQ/Ch/Band/Gain 1 1 -1799 "x"\n', expect_state={'room_eq': {'2': {'bands': {'2': {'gain': -1799}}}}})
telemetry(RM, 'room-eq-band-q', inbound='NOTIFY set RM:RoomEQ/Ch/Band/Q 1 1 101 "x"\n', expect_state={'room_eq': {'2': {'bands': {'2': {'q': 101}}}}})
telemetry(RM, 'room-eq-band-type', inbound='NOTIFY set RM:RoomEQ/Ch/Band/Type 1 1 1 "x"\n', expect_state={'room_eq': {'2': {'bands': {'2': {'type': 1}}}}})
telemetry(RM, 'speaker-processor-input-level', inbound='NOTIFY set RM:SpeakerProcessor/Ch/Input/Level 1 0 -32767 "x"\n', expect_state={'speaker_processor': {'2': {'input_level': -32767}}})
telemetry(RM, 'speaker-processor-delay-on', inbound='NOTIFY set RM:SpeakerProcessor/Ch/Delay/On 1 0 1 "x"\n', expect_state={'speaker_processor': {'2': {'delay_on': True}}})
telemetry(RM, 'speaker-processor-delay-time', inbound='NOTIFY set RM:SpeakerProcessor/Ch/Delay/Time 1 0 1 "x"\n', expect_state={'speaker_processor': {'2': {'delay_time': 1}}})
telemetry(RM, 'speaker-processor-xover-hpf-frequency', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverHpf/Frequency 1 0 201 "x"\n', expect_state={'speaker_processor': {'2': {'xover_hpf_frequency': 201}}})
telemetry(RM, 'speaker-processor-xover-hpf-gc', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverHpf/Gc 1 0 -5 "x"\n', expect_state={'speaker_processor': {'2': {'xover_hpf_gc': -5}}})
telemetry(RM, 'speaker-processor-xover-hpf-type', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverHpf/Type 1 0 1 "x"\n', expect_state={'speaker_processor': {'2': {'xover_hpf_type': 1}}})
telemetry(RM, 'speaker-processor-xover-lpf-frequency', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverLpf/Frequency 1 0 201 "x"\n', expect_state={'speaker_processor': {'2': {'xover_lpf_frequency': 201}}})
telemetry(RM, 'speaker-processor-xover-lpf-gc', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverLpf/Gc 1 0 -5 "x"\n', expect_state={'speaker_processor': {'2': {'xover_lpf_gc': -5}}})
telemetry(RM, 'speaker-processor-xover-lpf-type', inbound='NOTIFY set RM:SpeakerProcessor/Ch/XOverLpf/Type 1 0 1 "x"\n', expect_state={'speaker_processor': {'2': {'xover_lpf_type': 1}}})
telemetry(RM, 'speaker-processor-peq-on', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQOn/On 1 0 1 "x"\n', expect_state={'speaker_processor': {'2': {'peq_on': True}}})
telemetry(RM, 'speaker-processor-peq-band-bypass', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQBand/Bypass 1 1 1 "x"\n', expect_state={'speaker_processor': {'2': {'peq': {'2': {'bypass': True}}}}})
telemetry(RM, 'speaker-processor-peq-band-frequency', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQBand/Frequency 1 1 201 "x"\n', expect_state={'speaker_processor': {'2': {'peq': {'2': {'frequency': 201}}}}})
telemetry(RM, 'speaker-processor-peq-band-gain', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQBand/Gain 1 1 -1799 "x"\n', expect_state={'speaker_processor': {'2': {'peq': {'2': {'gain': -1799}}}}})
telemetry(RM, 'speaker-processor-peq-band-q', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQBand/Q 1 1 101 "x"\n', expect_state={'speaker_processor': {'2': {'peq': {'2': {'q': 101}}}}})
telemetry(RM, 'speaker-processor-peq-band-type', inbound='NOTIFY set RM:SpeakerProcessor/Ch/PEQBand/Type 1 1 1 "x"\n', expect_state={'speaker_processor': {'2': {'peq': {'2': {'type': 1}}}}})
telemetry(RM, 'near-end-output-fader-on', inbound='NOTIFY set RM:NeOut_Fader/Ch/On 1 0 1 "x"\n', expect_state={'near_end_outputs': {'2': {'on': True}}})
telemetry(RM, 'near-end-output-fader-level', inbound='NOTIFY set RM:NeOut_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'near_end_outputs': {'2': {'level': -32767}}})
telemetry(RM, 'far-end-sip-tone-level', inbound='NOTIFY set RM:SipToneFe_Fader/Ch/Level 0 0 -32767 "x"\n', expect_state={'sip_tone': {'far_end_level': -32767}})
telemetry(RM, 'near-end-sip-tone-level', inbound='NOTIFY set RM:SipToneNe_Fader/Ch/Level 0 0 -32767 "x"\n', expect_state={'sip_tone': {'near_end_level': -32767}})
telemetry(RM, 'ai-denoiser-on', inbound='NOTIFY set RM:DSPTOP/SubprocMode/On 0 0 1 "x"\n', expect_state={'ai_denoiser': {'on': True}})
telemetry(RM, 'ai-denoiser-type', inbound='NOTIFY set RM:AIDenoiser/Type/Type 0 0 1 "x"\n', expect_state={'ai_denoiser': {'type': 1}})
telemetry(RM, 'room-eq-output-on', inbound='NOTIFY set RM:RoomEQ_Fader/Ch/On 1 0 1 "x"\n', expect_state={'room_eq_outputs': {'2': {'on': True}}})
telemetry(RM, 'room-eq-output-level', inbound='NOTIFY set RM:RoomEQ_Fader/Ch/Level 1 0 -4999 "x"\n', expect_state={'room_eq_outputs': {'2': {'level': -4999}}})
telemetry(RM, 'dante-input-patch', inbound='NOTIFY set RM:DanteIn_Patch/Output/Input 1 0 1 "x"\n', expect_state={'dante_input_patch': {'2': 1}})
telemetry(RM, 'voice-lift-input-fader-on', inbound='NOTIFY set RM:VLIn_Fader/Ch/On 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'on': True}}}})
telemetry(RM, 'voice-lift-input-fader-level', inbound='NOTIFY set RM:VLIn_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'level': -32767}}}})
telemetry(RM, 'voice-lift-align-fader-on', inbound='NOTIFY set RM:VLAlign_Fader/Ch/On 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'align_on': True}}}})
telemetry(RM, 'voice-lift-align-fader-level', inbound='NOTIFY set RM:VLAlign_Fader/Ch/Level 1 0 -32767 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'align_level': -32767}}}})
telemetry(RM, 'voice-lift-link-fader-on', inbound='NOTIFY set RM:VLLink_Fader/Ch/On 0 0 1 "x"\n', expect_state={'voice_lift': {'link_on': True}})
telemetry(RM, 'voice-lift-link-fader-level', inbound='NOTIFY set RM:VLLink_Fader/Ch/Level 0 0 -32767 "x"\n', expect_state={'voice_lift': {'link_level': -32767}})
telemetry(RM, 'voice-lift-automix-type', inbound='NOTIFY set RM:VL_Automix/Type/Type 0 0 1 "x"\n', expect_state={'voice_lift': {'automix': {'type': 1}}})
telemetry(RM, 'voice-lift-gating-automix-last-mic-on', inbound='NOTIFY set RM:VL_GatingAutomix/Settings/LastMicOn 0 0 1 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'last_mic_on': True}}}})
telemetry(RM, 'voice-lift-gating-automix-open-mics', inbound='NOTIFY set RM:VL_GatingAutomix/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'open_mics': 2}}}})
telemetry(RM, 'voice-lift-gating-automix-threshold', inbound='NOTIFY set RM:VL_GatingAutomix/Settings/Threshold 0 0 -7199 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'threshold': -7199}}}})
telemetry(RM, 'voice-lift-gating-automix-range', inbound='NOTIFY set RM:VL_GatingAutomix/Settings/Range 0 0 -6999 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'range': -6999}}}})
telemetry(RM, 'voice-lift-gating-automix-hold', inbound='NOTIFY set RM:VL_GatingAutomix/Settings/Hold 0 0 21 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'hold': 21}}}})
telemetry(RM, 'voice-lift-gating-automix-priority-mic', inbound='NOTIFY set RM:VL_GatingAutomix/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'voice_lift': {'automix': {'gating': {'channels': {'2': {'priority': True}}}}}})
telemetry(RM, 'voice-lift-gain-sharing-automix-open-mics', inbound='NOTIFY set RM:VL_GainSharingAutomix/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'voice_lift': {'automix': {'gain_sharing': {'open_mics': 2}}}})
telemetry(RM, 'voice-lift-gain-sharing-automix-priority-mic', inbound='NOTIFY set RM:VL_GainSharingAutomix/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'voice_lift': {'automix': {'gain_sharing': {'channels': {'2': {'priority': True}}}}}})
telemetry(RM, 'voice-lift-gain-sharing-automix-weight', inbound='NOTIFY set RM:VL_GainSharingAutomix/Ch/Weight 1 0 -2999 "x"\n', expect_state={'voice_lift': {'automix': {'gain_sharing': {'channels': {'2': {'weight': -2999}}}}}})
telemetry(RM, 'voice-lift-gate-on', inbound='NOTIFY set RM:VL_Gate/Ch/On 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_on': True}}}})
telemetry(RM, 'voice-lift-gate-threshold', inbound='NOTIFY set RM:VL_Gate/Ch/Threshold 1 0 -7199 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_threshold': -7199}}}})
telemetry(RM, 'voice-lift-gate-range', inbound='NOTIFY set RM:VL_Gate/Ch/Range 1 0 -6999 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_range': -6999}}}})
telemetry(RM, 'voice-lift-gate-attack', inbound='NOTIFY set RM:VL_Gate/Ch/Attack 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_attack': 1}}}})
telemetry(RM, 'voice-lift-gate-decay', inbound='NOTIFY set RM:VL_Gate/Ch/Decay 1 0 3341 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_decay': 3341}}}})
telemetry(RM, 'voice-lift-gate-hold', inbound='NOTIFY set RM:VL_Gate/Ch/Hold 1 0 21 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'gate_hold': 21}}}})
telemetry(RM, 'voice-lift-fbs-on', inbound='NOTIFY set RM:VL_FBS/Ch/On 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'fbs_on': True}}}})
telemetry(RM, 'voice-lift-fbs-suppression-level', inbound='NOTIFY set RM:VL_FBS/Ch/SuppressionLevel 1 0 1 "x"\n', expect_state={'voice_lift': {'inputs': {'2': {'fbs_suppression_level': 1}}}})
telemetry(RM, 'voice-lift-auto-mute-on', inbound='NOTIFY set RM:VL_AutoMute/Ch/On 0 0 1 "x"\n', expect_state={'voice_lift': {'auto_mute_on': True}})
telemetry(RM, 'voice-lift-input-patch', inbound='NOTIFY set RM:LG_Patch/Output/Input 0 0 1 "x"\n', expect_state={'voice_lift': {'input_patch': 1}})
telemetry(RM, 'voice-lift-oscillator-on', inbound='NOTIFY set RM:VL_Manualtune/Ch/On 0 0 1 "x"\n', expect_state={'voice_lift': {'oscillator_on': True}})
telemetry(RM, 'voice-lift-oscillator-level', inbound='NOTIFY set RM:VL_Manualtune/Ch/Level 0 0 -32767 "x"\n', expect_state={'voice_lift': {'oscillator_level': -32767}})
telemetry(RM, 'voice-lift-loop-gain-start-time', inbound='NOTIFY set RM:VL_Manualtune/Ch/StartTime 0 0 1 "x"\n', expect_state={'voice_lift': {'loop_gain_start_time': 1}})
telemetry(RM, 'voice-lift-output-patch', inbound='NOTIFY set RM:VLOut_Patch/Output/Input 1 0 1 "x"\n', expect_state={'voice_lift': {'output_patch': {'2': 1}}})
telemetry(RM, 'dante-output-matrix-on', inbound='NOTIFY set RM:NeOut_Matrix/Input/Output/On 1 1 1 "x"\n', expect_state={'dante_output_matrix': {'2': {'3': {'on': True}}}})
telemetry(RM, 'dante-output-matrix-level', inbound='NOTIFY set RM:NeOut_Matrix/Input/Output/Level 1 1 -32767 "x"\n', expect_state={'dante_output_matrix': {'2': {'3': {'level': -32767}}}})
telemetry(RM, 'dante-output-matrix-delay-on', inbound='NOTIFY set RM:NeOut_Matrix/Input/Output/DelayOn 1 1 1 "x"\n', expect_state={'dante_output_matrix': {'2': {'3': {'delay_on': True}}}})
telemetry(RM, 'dante-output-matrix-delay-time', inbound='NOTIFY set RM:NeOut_Matrix/Input/Output/DelayTime 1 1 1 "x"\n', expect_state={'dante_output_matrix': {'2': {'3': {'delay_time': 1}}}})
telemetry(RM, 'voice-lift-fine-tune', inbound='NOTIFY set RM:AutoMicEq/FineTune 0 0 1 "x"\n', expect_state={'voice_lift': {'fine_tune': 1}})
telemetry(RM, 'mute-all', inbound='NOTIFY set RM:MicMute/All 0 0 1 "x"\n', expect_state={'mute': {'all': True}})
telemetry(RM, 'mute-group', inbound='NOTIFY set RM:MicMute/Group 2 0 1 "x"\n', expect_state={'mute': {'groups': {'2': True}}})
telemetry(RM, 'mute-force-all-individual', inbound='NOTIFY set RM:MicMute/ForceAllIndividual 0 0 1 "x"\n', expect_state={'mute': {'force_all_individual': True}})
telemetry(RM, 'led-brightness', inbound='NOTIFY set RM:Led/Brightness 0 0 1 "x"\n', expect_state={'led': {'brightness': 1}})
telemetry(RM, 'gain-sharing-automix-weight', inbound='NOTIFY set RM:GainSharingAutomix/Ch/Weight 1 0 -2999 "x"\n', expect_state={'automix': {'gain_sharing': {'channels': {'2': {'weight': -2999}}}}})
telemetry(RM, 'control-set-execute', inbound='NOTIFY set RM:ControlSets/Execution 1 0 1 "x"\n', expect_state={'control_sets': {'2': {'execution': True}}})
telemetry(RM, 'mic-gain-type', inbound='NOTIFY set RM:DSPTOP/MicGaintype 0 0 1 "x"\n', expect_state={'mic': {'gain_type': 1}})
telemetry(RM, 'low-latency-mic-gain-type', inbound='NOTIFY set RM:DSPTOP/LlMicGaintype 0 0 1 "x"\n', expect_state={'mic': {'low_latency_gain_type': 1}})
telemetry(RM, 'output2-mode', inbound='NOTIFY set RM:DSPTOP/Output2Mode 0 0 1 "x"\n', expect_state={'mic': {'output2_mode': 1}})
telemetry(RM, 'beam-max-speed', inbound='NOTIFY set RM:DSPTOP/MaxBeamSpeed 0 0 1 "x"\n', expect_state={'beam': {'max_speed': 1}})
telemetry(RM, 'beam-limit-on', inbound='NOTIFY set RM:Mic_Beam/LimitOn 0 0 1 "x"\n', expect_state={'beam': {'limit_on': True}})
telemetry(RM, 'beam-limit-top', inbound='NOTIFY set RM:Mic_Beam/Top 0 0 -38 "x"\n', expect_state={'beam': {'limit_top': -38}})
telemetry(RM, 'beam-limit-bottom', inbound='NOTIFY set RM:Mic_Beam/Bottom 0 0 -39 "x"\n', expect_state={'beam': {'limit_bottom': -39}})
telemetry(RM, 'beam-limit-left', inbound='NOTIFY set RM:Mic_Beam/Left 0 0 -39 "x"\n', expect_state={'beam': {'limit_left': -39}})
telemetry(RM, 'beam-limit-right', inbound='NOTIFY set RM:Mic_Beam/Right 0 0 -38 "x"\n', expect_state={'beam': {'limit_right': -38}})
telemetry(RM, 'beam-floor-to-talker-height', inbound='NOTIFY set RM:Mic_Beam/Height_FlrToTalk 0 0 1 "x"\n', expect_state={'beam': {'floor_to_talker': 1}})
telemetry(RM, 'beam-floor-to-mic-height', inbound='NOTIFY set RM:Mic_Beam/Height_FlrToMic 0 0 21 "x"\n', expect_state={'beam': {'floor_to_mic': 21}})
telemetry(RM, 'beam-speed', inbound='NOTIFY set RM:Mic_Beam/Speed 0 0 1 "x"\n', expect_state={'beam': {'speed': 1}})
telemetry(RM, 'echo-cancellation-level', inbound='NOTIFY set RM:Mic_Dsp/Aectype 0 0 1 "x"\n', expect_state={'mic': {'aec_level': 1}})
telemetry(RM, 'noise-reduction-level', inbound='NOTIFY set RM:Mic_Dsp/Nrtype 0 0 1 "x"\n', expect_state={'mic': {'nr_level': 1}})
telemetry(RM, 'dereverb-level', inbound='NOTIFY set RM:Mic_Dsp/Derevtype 0 0 1 "x"\n', expect_state={'mic': {'dereverb_level': 1}})
telemetry(RM, 'near-end-output-eq-on', inbound='NOTIFY set RM:NeOut_EQ/Ch/On/On 1 0 1 "x"\n', expect_state={'near_end_outputs': {'2': {'eq_on': True}}})
telemetry(RM, 'near-end-output-eq-band-bypass', inbound='NOTIFY set RM:NeOut_EQ/Ch/Band/Bypass 1 1 1 "x"\n', expect_state={'near_end_outputs': {'2': {'eq': {'2': {'bypass': True}}}}})
telemetry(RM, 'near-end-output-eq-band-frequency', inbound='NOTIFY set RM:NeOut_EQ/Ch/Band/Frequency 1 1 201 "x"\n', expect_state={'near_end_outputs': {'2': {'eq': {'2': {'frequency': 201}}}}})
telemetry(RM, 'near-end-output-eq-band-gain', inbound='NOTIFY set RM:NeOut_EQ/Ch/Band/Gain 1 1 -1799 "x"\n', expect_state={'near_end_outputs': {'2': {'eq': {'2': {'gain': -1799}}}}})
telemetry(RM, 'near-end-output-eq-band-q', inbound='NOTIFY set RM:NeOut_EQ/Ch/Band/Q 1 1 101 "x"\n', expect_state={'near_end_outputs': {'2': {'eq': {'2': {'q': 101}}}}})
telemetry(RM, 'near-end-output-eq-band-type', inbound='NOTIFY set RM:NeOut_EQ/Ch/Band/Type 1 1 1 "x"\n', expect_state={'near_end_outputs': {'2': {'eq': {'2': {'type': 1}}}}})
telemetry(RM, 'mic-automix-type', inbound='NOTIFY set RM:Automix/Mixtype 1 0 1 "x"\n', expect_state={'mic': {'automix_type': {'2': 1}}})
telemetry(RM, 'mic-agc-type', inbound='NOTIFY set RM:Mic_Agc/Agctype 0 0 1 "x"\n', expect_state={'mic': {'agc_type': 1}})
telemetry(RM, 'mic-agc-speed', inbound='NOTIFY set RM:Mic_Agc/AgcSpeed 0 0 1 "x"\n', expect_state={'mic': {'agc_speed': 1}})
telemetry(RM, 'near-end-output-mute', inbound='NOTIFY set RM:NeOut_Mute/Ch/On 1 0 1 "x"\n', expect_state={'near_end_outputs': {'2': {'muted': False}}})
telemetry(RM, 'mute-link-enable', inbound='NOTIFY set RM:MicMute/Link/Enable 1 0 1 "x"\n', expect_state={'mute': {'link': {'2': True}}})
telemetry(RM, 'dante-output-patch', inbound='NOTIFY set RM:DanteOut_Patch/Output/Input 1 0 1 "x"\n', expect_state={'dante_output_patch': {'2': 1}})
telemetry(RM, 'beam-focus-on', inbound='NOTIFY set RM:Mic_Beam/Focus/Ch/On 1 0 1 "x"\n', expect_state={'beam': {'focus': {'2': {'on': True}}}})
telemetry(RM, 'beam-focus-x', inbound='NOTIFY set RM:Mic_Beam/Focus/Ch/X 1 0 -38 "x"\n', expect_state={'beam': {'focus': {'2': {'x': -38}}}})
telemetry(RM, 'beam-focus-y', inbound='NOTIFY set RM:Mic_Beam/Focus/Ch/Y 1 0 -38 "x"\n', expect_state={'beam': {'focus': {'2': {'y': -38}}}})
telemetry(RM, 'beam-focus-width', inbound='NOTIFY set RM:Mic_Beam/Focus/Ch/Width 1 0 2 "x"\n', expect_state={'beam': {'focus': {'2': {'width': 2}}}})
telemetry(RM, 'beam-focus-depth', inbound='NOTIFY set RM:Mic_Beam/Focus/Ch/Depth 1 0 2 "x"\n', expect_state={'beam': {'focus': {'2': {'depth': 2}}}})
telemetry(RM, 'beam-exclusion-on', inbound='NOTIFY set RM:Mic_Beam/Exclusion/Ch/On 1 0 1 "x"\n', expect_state={'beam': {'exclusion': {'2': {'on': True}}}})
telemetry(RM, 'beam-exclusion-x', inbound='NOTIFY set RM:Mic_Beam/Exclusion/Ch/X 1 0 -38 "x"\n', expect_state={'beam': {'exclusion': {'2': {'x': -38}}}})
telemetry(RM, 'beam-exclusion-y', inbound='NOTIFY set RM:Mic_Beam/Exclusion/Ch/Y 1 0 -38 "x"\n', expect_state={'beam': {'exclusion': {'2': {'y': -38}}}})
telemetry(RM, 'beam-exclusion-width', inbound='NOTIFY set RM:Mic_Beam/Exclusion/Ch/Width 1 0 2 "x"\n', expect_state={'beam': {'exclusion': {'2': {'width': 2}}}})
telemetry(RM, 'beam-exclusion-depth', inbound='NOTIFY set RM:Mic_Beam/Exclusion/Ch/Depth 1 0 2 "x"\n', expect_state={'beam': {'exclusion': {'2': {'depth': 2}}}})
telemetry(RM, 'led-mute-pattern', inbound='NOTIFY set RM:Led/Config/Pattern 1 0 1 "x"\n', expect_state={'led': {'mute_states': {'2': {'pattern': 1}}}})
telemetry(RM, 'led-mute-color-red', inbound='NOTIFY set RM:Led/Config/Color/R 1 1 1 "x"\n', expect_state={'led': {'mute_states': {'2': {'leds': {'2': {'red': 1}}}}}})
telemetry(RM, 'led-mute-color-green', inbound='NOTIFY set RM:Led/Config/Color/G 1 1 1 "x"\n', expect_state={'led': {'mute_states': {'2': {'leds': {'2': {'green': 1}}}}}})
telemetry(RM, 'led-mute-color-blue', inbound='NOTIFY set RM:Led/Config/Color/B 1 1 1 "x"\n', expect_state={'led': {'mute_states': {'2': {'leds': {'2': {'blue': 1}}}}}})
telemetry(RM, 'voice-lift-reset', inbound='NOTIFY set RM:AutoMicEq/Reset_oneshot 0 0 1 "x"\n', expect_state={'voice_lift': {'reset': True}})
telemetry(RM, 'directivity-mode', inbound='NOTIFY set RM:Mic_Direcctl/Mode 0 0 1 "x"\n', expect_state={'directivity': {'mode': 1}})
telemetry(RM, 'directivity-angle', inbound='NOTIFY set RM:Mic_Direcctl/Ch/Angle 1 0 -179 "x"\n', expect_state={'directivity': {'mics': {'2': {'angle': -179}}}})
telemetry(RM, 'directivity-mic-on', inbound='NOTIFY set RM:Mic_Direcctl/Ch/On 1 0 1 "x"\n', expect_state={'directivity': {'mics': {'2': {'on': True}}}})
telemetry(RM, 'wap-unified-comms-lpf-type', inbound='NOTIFY set RM:Mic_Dsp/Id/LpfType 1 0 1 "x"\n', expect_state={'units': {'2': {'uc_lpf_type': 1}}})
telemetry(RM, 'wap-unified-comms-hpf-type', inbound='NOTIFY set RM:Mic_Dsp/Id/HpfType 1 0 1 "x"\n', expect_state={'units': {'2': {'uc_hpf_type': 1}}})
telemetry(RM, 'wap-unified-comms-gain', inbound='NOTIFY set RM:NeOut_Gain/Id/Level 1 0 -127 "x"\n', expect_state={'units': {'2': {'uc_gain': -127}}})
telemetry(RM, 'wap-low-latency-lpf-type', inbound='NOTIFY set RM:MicLL_Dsp/Id/LpfType 1 0 1 "x"\n', expect_state={'units': {'2': {'ll_lpf_type': 1}}})
telemetry(RM, 'wap-low-latency-hpf-type', inbound='NOTIFY set RM:MicLL_Dsp/Id/HpfType 1 0 1 "x"\n', expect_state={'units': {'2': {'ll_hpf_type': 1}}})
telemetry(RM, 'wap-low-latency-gain', inbound='NOTIFY set RM:NeOutLL_Gain/Id/Level 1 0 -127 "x"\n', expect_state={'units': {'2': {'ll_gain': -127}}})
telemetry(RM, 'wap-dante-output-patch', inbound='NOTIFY set RM:DanteOut_Patch/Output/Id 1 0 1 "x"\n', expect_state={'wap': {'dante_outputs': {'2': {'source': 1}}}})
telemetry(RM, 'wap-input-selector', inbound='NOTIFY set RM:In_Selector/Input/Id 1 0 1 "x"\n', expect_state={'wap': {'input_selector': {'2': True}}})
telemetry(RM, 'gating-automix-ll-type', inbound='NOTIFY set RM:AutomixLL/Type/Type 0 0 1 "x"\n', expect_state={'automix_ll': {'type': 1}})
telemetry(RM, 'gating-automix-ll-last-mic-on', inbound='NOTIFY set RM:GatingAutomixLL/Settings/LastMicOn 0 0 1 "x"\n', expect_state={'automix_ll': {'gating': {'last_mic_on': True}}})
telemetry(RM, 'gating-automix-ll-open-mics', inbound='NOTIFY set RM:GatingAutomixLL/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'automix_ll': {'gating': {'open_mics': 2}}})
telemetry(RM, 'gating-automix-ll-threshold', inbound='NOTIFY set RM:GatingAutomixLL/Settings/Threshold 0 0 -7199 "x"\n', expect_state={'automix_ll': {'gating': {'threshold': -7199}}})
telemetry(RM, 'gating-automix-ll-range', inbound='NOTIFY set RM:GatingAutomixLL/Settings/Range 0 0 -6999 "x"\n', expect_state={'automix_ll': {'gating': {'range': -6999}}})
telemetry(RM, 'gating-automix-ll-hold', inbound='NOTIFY set RM:GatingAutomixLL/Settings/Hold 0 0 21 "x"\n', expect_state={'automix_ll': {'gating': {'hold': 21}}})
telemetry(RM, 'gating-automix-ll-priority-mic', inbound='NOTIFY set RM:GatingAutomixLL/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'automix_ll': {'gating': {'channels': {'2': {'priority': True}}}}})
telemetry(RM, 'gain-sharing-automix-ll-open-mics', inbound='NOTIFY set RM:GainSharingAutomixLL/Settings/NumOfOpenMic 0 0 2 "x"\n', expect_state={'automix_ll': {'gain_sharing': {'open_mics': 2}}})
telemetry(RM, 'gain-sharing-automix-ll-priority-mic', inbound='NOTIFY set RM:GainSharingAutomixLL/Ch/PriorityMic 1 0 1 "x"\n', expect_state={'automix_ll': {'gain_sharing': {'channels': {'2': {'priority': True}}}}})
telemetry(RM, 'gain-sharing-automix-ll-weight', inbound='NOTIFY set RM:GainSharingAutomixLL/Ch/Weight 1 0 -2999 "x"\n', expect_state={'automix_ll': {'gain_sharing': {'channels': {'2': {'weight': -2999}}}}})
telemetry(RM, 'mute-link-ll-enable', inbound='NOTIFY set RM:MicMuteLL/Link/Enable 1 0 1 "x"\n', expect_state={'mute': {'link_ll': {'2': True}}})
telemetry(RM, 'wap-near-end-mute', inbound='NOTIFY set RM:NeOut_Mute/Id/On 1 0 1 "x"\n', expect_state={'units': {'2': {'near_end_muted': True}}})
telemetry(RM, 'wap-mic-battery-level', inbound='NOTIFY set RM:Mic_Battery/Id/Level 1 0 1 "x"\n', expect_state={'units': {'2': {'battery': 1}}})
telemetry(RM, 'wap-mic-packet-error-count', inbound='NOTIFY set RM:Mic_LinkQuality/Id/PacketErrCount 1 0 1 "x"\n', expect_state={'units': {'2': {'packet_errors': 1}}})
telemetry(RM, 'wap-mic-gain-type', inbound='NOTIFY set RM:Mic_Dsp/Id/MicGaintype 1 0 1 "x"\n', expect_state={'units': {'2': {'mic_gain_type': 1}}})
telemetry(RM, 'wap-echo-cancellation-level', inbound='NOTIFY set RM:Mic_Dsp/Id/Aectype 1 0 1 "x"\n', expect_state={'units': {'2': {'aec_level': 1}}})
telemetry(RM, 'wap-noise-reduction-level', inbound='NOTIFY set RM:Mic_Dsp/Id/Nrtype 1 0 1 "x"\n', expect_state={'units': {'2': {'nr_level': 1}}})
telemetry(RM, 'wap-dereverb-level', inbound='NOTIFY set RM:Mic_Dsp/Id/Derevtype 1 0 1 "x"\n', expect_state={'units': {'2': {'dereverb_level': 1}}})
telemetry(RM, 'wap-near-end-eq-on', inbound='NOTIFY set RM:NeOut_EQ/Id/On 1 0 1 "x"\n', expect_state={'units': {'2': {'eq_on': True}}})
telemetry(RM, 'wap-near-end-eq-band-bypass', inbound='NOTIFY set RM:NeOut_EQ/Id/Band/Bypass 1 1 1 "x"\n', expect_state={'units': {'2': {'eq': {'2': {'bypass': True}}}}})
telemetry(RM, 'wap-near-end-eq-band-frequency', inbound='NOTIFY set RM:NeOut_EQ/Id/Band/Frequency 1 1 201 "x"\n', expect_state={'units': {'2': {'eq': {'2': {'frequency': 201}}}}})
telemetry(RM, 'wap-near-end-eq-band-gain', inbound='NOTIFY set RM:NeOut_EQ/Id/Band/Gain 1 1 -1799 "x"\n', expect_state={'units': {'2': {'eq': {'2': {'gain': -1799}}}}})
telemetry(RM, 'wap-near-end-eq-band-q', inbound='NOTIFY set RM:NeOut_EQ/Id/Band/Q 1 1 101 "x"\n', expect_state={'units': {'2': {'eq': {'2': {'q': 101}}}}})
telemetry(RM, 'wap-near-end-eq-band-type', inbound='NOTIFY set RM:NeOut_EQ/Id/Band/Type 1 1 1 "x"\n', expect_state={'units': {'2': {'eq': {'2': {'type': 1}}}}})
telemetry(RM, 'wap-agc-type', inbound='NOTIFY set RM:Mic_Agc/Id/Agctype 1 0 1 "x"\n', expect_state={'units': {'2': {'agc_type': 1}}})
telemetry(RM, 'wap-agc-speed', inbound='NOTIFY set RM:Mic_Agc/Id/AgcSpeed 1 0 1 "x"\n', expect_state={'units': {'2': {'agc_speed': 1}}})
