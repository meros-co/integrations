RVS = "ross-video-server"
# Ross video servers (Tria, Mira, Kiva) RossTalk receive commands (Ross online help; V9.2.0 release notes): text
# ended by CR LF, no reply.


def _rvs(command, input, wire, **extra):
    text(RVS, command, input, wire, **extra)


_rvs("cue", {"channel": "D", "clip": "crime-report"}, "CUE D:crime-report\r\n", expect_result={"ok": {"kind": "unverified"}})
_rvs("cue", {"channel": "A", "clip": "SPORTS/HOCKEY/Finals"}, "CUE A:SPORTS/HOCKEY/Finals\r\n", file="cue-folder")
_rvs("cue_at", {"channel": "A", "clip": "goal-highlight", "hours": 0, "minutes": 0, "seconds": 15, "frames": 0},
     "CUE A:goal-highlight:00,00,15,00\r\n")
_rvs("play", {"channel": "B"}, "PLAY B\r\n")
_rvs("play_clip", {"channel": "B", "clip": "storm"}, "PLAY B:storm\r\n")
_rvs("play_clip_at", {"channel": "B", "clip": "storm", "hours": 0, "minutes": 0, "seconds": 3, "frames": 10},
     "PLAY B:storm:00,00,03,10\r\n")
_rvs("stop", {"channel": "F"}, "STOP F\r\n")
_rvs("goto", {"channel": "A", "hours": 0, "minutes": 0, "seconds": 0, "frames": 20}, "GOTO A:00,00,00,20\r\n")
_rvs("jog", {"channel": "C", "frames": -20}, "JOG C:-20f\r\n")
_rvs("jog", {"channel": "C", "frames": 5}, "JOG C:5f\r\n", file="jog-forward")
_rvs("loop", {"channel": "B", "mode": "loop"}, "LOOP B:loop\r\n")
_rvs("loop", {"channel": "B", "mode": "ping_pong_to"}, "LOOP B:ping-pong-to\r\n", file="loop-ping-pong-to")
_rvs("angle", {"channel": "A", "angle": 3}, "ANGLE A:3\r\n")
_rvs("eject", {"channel": "A"}, "EJECT A\r\n")
_rvs("load_playlist", {"channel": "D", "playlist": "Good Morning Show"}, "LOAD_PLAYLIST D:Good Morning Show\r\n")
_rvs("load_playlist_pair", {"preview": "B", "program": "A", "playlist": "Sports_Show"}, "LOAD_PLAYLIST B:A:Sports_Show\r\n")
_rvs("loop_playlist", {"channel": "A", "enabled": True}, "LOOP_PLAYLIST A:loop\r\n")
_rvs("loop_playlist", {"channel": "A", "enabled": False}, "LOOP_PLAYLIST A:off\r\n", file="loop_playlist-off")
_rvs("take", {"channel": "G"}, "TAKE G\r\n", model="mira")
_rvs("up_one_item", {"channel": "B"}, "UP_ONE_ITEM B\r\n")
_rvs("down_one_item", {"channel": "B"}, "DOWN_ONE_ITEM B\r\n")
_rvs("recue_to_first_item", {"channel": "B"}, "RECUE_TO_FIRST_ITEM B\r\n")
_rvs("eject_playlist", {"channel": "A"}, "EJECT_PLAYLIST A\r\n")
