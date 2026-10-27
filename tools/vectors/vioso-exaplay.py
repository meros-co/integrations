# VIOSO EXAPLAY (vioso-exaplay): command lines + CR LF on the TCP port of
# config.xml's <tcp_control>, each answered with one line, as the EXAPLAY
# Remote Control page's table gives them (play 3, seek 10, set vol 50 -> OK;
# get cue -> play,1,562,18.733,50.000,30.00; get vol -> 100; ver ->
# Exaplay,1.1.0.0).
EX = "vioso-exaplay"
OK = {"ok": {"kind": "ack"}}

text(EX, "hello", {}, "hello\r\n", device_reply="hallo\r\n", expect_result={"ok": {"kind": "value", "value": "hallo"}})
text(EX, "get_version", {}, "ver\r\n", device_reply="Exaplay,1.1.0.0\r\n",
     expect_result={"ok": {"kind": "value", "value": "Exaplay,1.1.0.0"}})
text(EX, "play", {}, "play\r\n", device_reply="OK\r\n", expect_result=OK)
text(EX, "play_cue", {"cue": 3}, "play 3\r\n", device_reply="OK\r\n", expect_result=OK)
text(EX, "pause", {}, "pause\r\n", device_reply="OK\r\n", expect_result=OK)
text(EX, "stop", {}, "stop\r\n", device_reply="OK\r\n", expect_result=OK)
text(EX, "seek", {"seconds": 10.0}, "seek 10.000\r\n", device_reply="OK\r\n", expect_result=OK)
text(EX, "get_cue", {}, "get cue\r\n", device_reply="play,1,562,18.733,50.000,30.00\r\n",
     expect_result={"ok": {"kind": "value", "value": "play,1,562,18.733,50.000,30.00"}})
text(EX, "get_volume", {}, "get vol\r\n", device_reply="100\r\n", expect_result={"ok": {"kind": "value", "value": "100"}})
text(EX, "set_volume", {"volume": 50}, "set vol 50\r\n", device_reply="OK\r\n", expect_result=OK)

telemetry(EX, "cue", inbound="play,1,562,18.733,50.000,30.00\r\n", expect_state={"playback": {
    "status": "play", "cue": 1, "frame": 562, "time": 18.733, "duration": 50.0, "fps": 30.0}})
telemetry(EX, "cue-paused", inbound="pause,2,0,0.000,12.500,25.00\r\n", expect_state={"playback": {
    "status": "pause", "cue": 2, "frame": 0, "time": 0.0, "duration": 12.5, "fps": 25.0}})
telemetry(EX, "volume", inbound="80\r\n", request="get vol", expect_state={"audio": {"volume": 80}})
telemetry(EX, "version", inbound="Exaplay,1.1.0.0\r\n", request="ver", expect_state={"device": {"version": "Exaplay,1.1.0.0"}})
