# Grass Valley Kaleido gateway (grassvalley-kaleido): XML commands ended by a
# carriage return on TCP 13000, as the Kaleido Remote Control Protocol
# (Gateway) User's Guide (M770-0900-116) writes them; answers <ack/>, <nack/>
# or a reply element. The session is opened with <openID> on connecting.
GVK = "grassvalley-kaleido"
_GVK_ACK = {"device_reply": "<ack/>\r", "expect_result": {"ok": {"kind": "ack"}}}


def gvk(command, input, wire, **extra):
    text(GVK, command, input, wire, **extra)


gvk("get_software_version", {}, '<getParameterInfo>get key="softwareVersion"</getParameterInfo>\r',
    device_reply='<kParameterInfo>softwareVersion="7.20 build 5401"</kParameterInfo>\r',
    expect_result={"ok": {"kind": "value", "value": "7.20 build 5401"}})
gvk("get_system_name", {}, '<getParameterInfo>get key="systemName"</getParameterInfo>\r',
    device_reply='<kParameterInfo>systemName="Cougar-X"</kParameterInfo>\r',
    expect_result={"ok": {"kind": "value", "value": "Cougar-X"}})
gvk("get_current_layout", {}, "<getKCurrentLayout/>\r",
    device_reply='<kCurrentLayout>name="MAIN.kg2"</kCurrentLayout>\r',
    expect_result={"ok": {"kind": "value", "value": "MAIN.kg2"}})
gvk("set_current_layout", {"layout": "Room1/Layout1.kg2"},
    "<setKCurrentLayout>set Room1/Layout1.kg2</setKCurrentLayout>\r", **_GVK_ACK)
gvk("set_current_layout", {"layout": "Missing.kg2"}, "<setKCurrentLayout>set Missing.kg2</setKCurrentLayout>\r",
    device_reply="<nack/>\r", expect_result={"error": {"error": "device_error"}}, file="set_current_layout-nack")
gvk("get_layout_list", {}, "<getKLayoutList/>\r",
    device_reply="<kLayoutList>Room1/MAIN.kg2 Room1/BACKUP1.kg2</kLayoutList>\r",
    expect_result={"ok": {"kind": "value", "value": "Room1/MAIN.kg2 Room1/BACKUP1.kg2"}})
gvk("get_room", {"room": "Room1"}, "<getKRoom>Room1</getKRoom>\r")
gvk("get_room_list", {}, "<getKRoomList/>\r")
gvk("set_status_message", {"id": "my_alarm", "status": "MINOR"},
    '<setKStatusMessage>set id="my_alarm" status="MINOR" message=""</setKStatusMessage>\r', **_GVK_ACK)
gvk("unlatch_channel", {"channel": "/Input A/Channel 01"},
    '<setKUnlatch>set channelName="/Input A/Channel 01"</setKUnlatch>\r', **_GVK_ACK)
gvk("unlatch_alarm", {"uri": "miranda.monako.daq://local/data/master/3/card/av1/DTVCCData#dtvccService1Presence"},
    '<setKUnlatch>set uri="miranda.monako.daq://local/data/master/3/card/av1/DTVCCData#dtvccService1Presence"'
    '</setKUnlatch>\r')
gvk("unlatch_gateway_alarm", {"id": "my_gateway_alarm_1"}, '<setKUnlatch>set id="my_gateway_alarm_1"</setKUnlatch>\r')
gvk("get_channel", {"monitor": "Room1/composite42"}, '<getKChannel>set monitor="Room1/composite42"</getKChannel>\r',
    device_reply='<kChannel>channelname="/Input A/Channel 1"</kChannel>\r',
    expect_result={"ok": {"kind": "value", "value": "/Input A/Channel 1"}})
gvk("set_channel", {"channel": "/Input A/Channel 1", "monitor": "Room1/composite42"},
    '<setKChannel>set channelname="/Input A/Channel 1" monitor="Room1/composite42"</setKChannel>\r', **_GVK_ACK)
gvk("set_dynamic_text", {"address": "12", "text": "Camera 1"},
    '<setKDynamicText>set address="12" text="Camera 1"</setKDynamicText>\r', **_GVK_ACK)
gvk("get_dynamic_text", {"address": "12"}, '<getKDynamicText>set address="12"</getKDynamicText>\r',
    device_reply="<kDynamicText>Camera 1</kDynamicText>\r",
    expect_result={"ok": {"kind": "value", "value": "Camera 1"}})
gvk("get_metadata", {"name": "XDS1"}, "<getKMetaData>get dataname=XDS1</getKMetaData>\r", model="kaleido-k2")
gvk("set_timer", {"id": "T1", "preset": "00:05:00"},
    '<setKTimer>set id="T1" preset="00:05:00" direction="DOWN" loop="OFF" </setKTimer>\r', **_GVK_ACK)
gvk("set_timer2", {"name": "Show", "preset": "00:30:00:00", "mode": "down"},
    '<setKTimer2>set TimerName="Show" StartTime="NOW" PresetTime="00:30:00:00" TimerMode="down" EndMode="STOP"'
    '</setKTimer2>\r', **_GVK_ACK)
gvk("trigger_timer", {"id": "T1", "trigger": "START"},
    '<setKTimerTrigger>set id="T1" trigger="START"</setKTimerTrigger>\r', **_GVK_ACK)
gvk("fire_action", {"name": "Go Live"}, '<setKFireAction>set name="Go Live"</setKFireAction>\r', **_GVK_ACK)
gvk("get_action_list", {}, "<getKActionList/>\r",
    device_reply="<kActionList><action>Action1</action><action>Action2</action></kActionList>\r",
    expect_result={"ok": {"kind": "value", "value": "<action>Action1</action><action>Action2</action>"}})
gvk("enable_alarm_group", {"name": "Group1"}, '<setKEnableAlarmGroup>set name="Group1"</setKEnableAlarmGroup>\r')
gvk("disable_alarm_group", {"name": "Group1"}, '<setKDisableAlarmGroup>set name="Group1"</setKDisableAlarmGroup>\r')
gvk("save_layout", {"name": "Backup"}, '<setKSaveLayout>set name="Backup"</setKSaveLayout>\r', model="kaleido-k2")
gvk("get_audio_out", {}, "<getKAudioOut/>\r", device_reply='<kAudioOut>Type="NONE"</kAudioOut>\r',
    expect_result={"ok": {"kind": "value", "value": 'Type="NONE"'}})
gvk("monitor_no_audio", {}, ['<setKAudioOut>set Type="NONE"</setKAudioOut>\r', "<getKAudioOut/>\r"],
    device_reply="<ack/>\r")
gvk("monitor_stream", {"ip": "10.0.0.20", "feed": 3},
    ['<setKAudioOut>set Type="STREAMING" IP="10.0.0.20" FeedID="3"</setKAudioOut>\r', "<getKAudioOut/>\r"],
    device_reply="<ack/>\r")
gvk("monitor_audio_card", {"input": 65},
    ['<setKAudioOut>set Type="AUDIOCARD" Input="65"</setKAudioOut>\r', "<getKAudioOut/>\r"], device_reply="<ack/>\r")
gvk("monitor_embedded", {"input": 4, "group": 2, "aes": 1},
    ['<setKAudioOut>set Type="EMBEDDED" Input="4" Group="2" AES="1"</setKAudioOut>\r', "<getKAudioOut/>\r"],
    device_reply="<ack/>\r")
gvk("get_audio_volume", {}, "<getKAudioOutVolume/>\r", device_reply='<kAudioOutVolume>volume="-20"</kAudioOutVolume>\r',
    expect_result={"ok": {"kind": "value", "value": "-20"}})
gvk("set_audio_volume", {"db": -12.5},
    ['<setKAudioOutVolume>set volume="-12.5"</setKAudioOutVolume>\r', "<getKAudioOutVolume/>\r"],
    device_reply="<ack/>\r")
gvk("get_audio_mode", {}, "<getKAudioOutMode/>\r", device_reply='<kAudioOutMode>mode="MUTE"</kAudioOutMode>\r',
    expect_result={"ok": {"kind": "value", "value": "MUTE"}})
gvk("set_audio_mode", {"mode": "minus_20_db"},
    ['<setKAudioOutMode>set mode="-20 dB"</setKAudioOutMode>\r', "<getKAudioOutMode/>\r"], device_reply="<ack/>\r")
gvk("set_vertical_offset", {"lines": 88}, '<setKVerticalOffset>set offset="88"</setKVerticalOffset>\r',
    model="kaleido-alto-quad")
gvk("set_icontrol_mode", {"enabled": True}, '<setKIcontrolMode>set mode="1"</setKIcontrolMode>\r',
    model="kaleido-alto-quad")
gvk("set_mouse_color", {"slot": "B", "color": "ff00ff"},
    '<setKMouseColorB>set mouseColorB="FFFF00FF"</setKMouseColorB>\r', model="kaleido-alto-quad")

# ── Telemetry ──
telemetry(GVK, "session", expect_connect_wire=["<openID></openID>\r"], inbound="<ack/>\r", expect_state={})
telemetry(GVK, "software-version", inbound='<kParameterInfo>softwareVersion="7.20 build 5401"</kParameterInfo>\r',
          expect_state={"software_version": "7.20 build 5401"})
telemetry(GVK, "system-name", inbound='<kParameterInfo>systemName="Cougar-X"</kParameterInfo>\r',
          expect_state={"system_name": "Cougar-X"})
telemetry(GVK, "current-layout", inbound='<kCurrentLayout>name="CurrentLayout.kg2"</kCurrentLayout>\r',
          expect_state={"current_layout": "CurrentLayout.kg2"})
telemetry(GVK, "current-layout-alto", inbound="<kCurrentLayout>Currentlayout.xml</kCurrentLayout>\r",
          expect_state={"current_layout": "Currentlayout.xml"})
telemetry(GVK, "layouts", inbound="<kLayoutList>Room1/MAIN.kg2 Room1/BACKUP1.kg2 Room2/MAIN.kg2</kLayoutList>\r",
          state_before={"layouts": {"3": "Old.kg2"}},
          expect_state={"layouts": {"0": "Room1/MAIN.kg2", "1": "Room1/BACKUP1.kg2", "2": "Room2/MAIN.kg2"}})
telemetry(GVK, "rooms", inbound="<kRoomList><room>Room A</room><room>Room B</room></kRoomList>\r",
          expect_state={"rooms": {"0": "Room A", "1": "Room B"}})
telemetry(GVK, "actions", inbound="<kActionList><action>Action1</action><action>Action2</action></kActionList>\r",
          expect_state={"actions": {"0": "Action1", "1": "Action2"}})
telemetry(GVK, "channel", request='<getKChannel>set monitor="Room1/composite42"</getKChannel>',
          inbound='<kChannel>channelname="/Input A/Channel 1"</kChannel>\r',
          expect_state={"monitors": {"Room1/composite42": {"channel": "/Input A/Channel 1"}}})
telemetry(GVK, "dynamic-text", request='<getKDynamicText>set address="12"</getKDynamicText>',
          inbound="<kDynamicText>Camera 1</kDynamicText>\r", expect_state={"dynamic_text": {"12": "Camera 1"}})
telemetry(GVK, "audio-out-embedded", inbound='<kAudioOut>Type="EMBEDDED" Input="4" Group="2" AES="1"</kAudioOut>\r',
          expect_state={"audio_out": {"type": "EMBEDDED", "ip": "", "feed": "", "input": "4", "group": "2", "aes": "1"}})
telemetry(GVK, "audio-out-stream", inbound='<kAudioOut>Type="STREAMING" IP="10.0.0.20" FeedID="3"</kAudioOut>\r',
          expect_state={"audio_out": {"type": "STREAMING", "ip": "10.0.0.20", "feed": "3", "input": "", "group": "",
                                      "aes": ""}})
telemetry(GVK, "audio-volume", inbound='<kAudioOutVolume>volume="-20.5"</kAudioOutVolume>\r',
          expect_state={"audio_volume_db": -20.5})
telemetry(GVK, "audio-mode", inbound='<kAudioOutMode>mode="-20 dB"</kAudioOutMode>\r',
          expect_state={"audio_mode": "minus_20_db"})
telemetry(GVK, "layout-loaded", request="<setKCurrentLayout>set Room1/Layout1.kg2</setKCurrentLayout>",
          inbound="<ack/>\r", expect_state={"current_layout": "Room1/Layout1.kg2"})
telemetry(GVK, "channel-set", request='<setKChannel>set channelname="/Input B/Channel 2" monitor="M1"</setKChannel>',
          inbound="<ack/>\r", expect_state={"monitors": {"M1": {"channel": "/Input B/Channel 2"}}})
telemetry(GVK, "dynamic-text-set", request='<setKDynamicText>set address="7" text="VT 1"</setKDynamicText>',
          inbound="<ack/>\r", expect_state={"dynamic_text": {"7": "VT 1"}})
telemetry(GVK, "alarm-set", request='<setKStatusMessage>set id="7" status="CRITICAL" message=""</setKStatusMessage>',
          inbound="<ack/>\r", expect_state={"alarms": {"7": "CRITICAL"}})
