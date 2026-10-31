SUI = "soundcraft-ui"
# Soundcraft Ui24R / Ui16 / Ui12: text messages on ws://<mixer>/, each prefixed
# 3:::, as the MIT-licensed soundcraft-ui library (mixer-connection.ts and the
# facades, commit 2f54ace6) writes them: SETD^<path>^<number>, SETS^<path>^<text>,
# channels numbered from 0 on the wire.


def sui(command, input, message, **extra):
    text(SUI, command, input, "3:::" + message, **extra)


sui("set_value", {"path": "i.0.mix", "value": 0.5}, "SETD^i.0.mix^0.500000")
sui("set_text", {"path": "var.mtk.session", "text": "Show 1"}, "SETS^var.mtk.session^Show 1")
sui("set_fader", {"strip": "input", "channel": 3, "level": 0.75}, "SETD^i.2.mix^0.7500")
# 0 dB lies between the fader law's points 0.75 (-0.55 dB) and 0.775 (+0.38 dB):
# 0.75 + 0.025 × 0.55 / 0.93 = 0.76478...
sui("set_fader_db", {"strip": "aux", "channel": 2, "level_db": 0.0}, "SETD^a.1.mix^0.7648")
sui("set_fader_db", {"strip": "vca", "channel": 1, "level_db": 10.0}, "SETD^v.0.mix^1.0000",
    file="set_fader_db-top")
sui("set_mute", {"strip": "fx", "channel": 4, "muted": True}, "SETD^f.3.mute^1")
sui("set_solo", {"strip": "sub", "channel": 1, "soloed": False}, "SETD^s.0.solo^0")
sui("set_pan", {"strip": "line", "channel": 2, "pan": 0.25}, "SETD^l.1.pan^0.2500")
sui("set_name", {"strip": "player", "channel": 1, "name": "PLAYBACK"}, "SETS^p.0.name^PLAYBACK")
sui("set_delay", {"strip": "aux", "channel": 5, "delay_ms": 120.0}, "SETD^a.4.delay^0.1200")
sui("set_aux_send", {"source": "input", "channel": 1, "aux": 3, "level": 0.6}, "SETD^i.0.aux.2.value^0.6000")
sui("set_aux_send_db", {"source": "fx", "channel": 1, "aux": 1, "level_db": 10.0}, "SETD^f.0.aux.0.value^1.0000")
sui("set_aux_send_mute", {"source": "line", "channel": 1, "aux": 10, "muted": True}, "SETD^l.0.aux.9.mute^1")
sui("set_aux_send_pan", {"source": "player", "channel": 2, "aux": 1, "pan": 1.0}, "SETD^p.1.aux.0.pan^1.0000")
sui("set_aux_send_post", {"source": "input", "channel": 24, "aux": 4, "post": True}, "SETD^i.23.aux.3.post^1")
sui("set_aux_send_post_proc", {"source": "input", "channel": 2, "aux": 1, "post_proc": False},
    "SETD^i.1.aux.0.postproc^0")
sui("set_fx_send", {"source": "sub", "channel": 2, "fx": 1, "level": 0.5}, "SETD^s.1.fx.0.value^0.5000")
sui("set_fx_send_db", {"source": "input", "channel": 1, "fx": 4, "level_db": -100.0}, "SETD^i.0.fx.3.value^0.0000")
sui("set_fx_send_mute", {"source": "player", "channel": 1, "fx": 2, "muted": False}, "SETD^p.0.fx.1.mute^0")
sui("set_fx_send_post", {"source": "line", "channel": 2, "fx": 3, "post": True}, "SETD^l.1.fx.2.post^1")
sui("set_matrix_send", {"source": "aux", "channel": 1, "matrix": 9, "level": 0.8}, "SETD^a.0.mtx.8.value^0.8000")
sui("set_matrix_send_db", {"source": "sub", "channel": 3, "matrix": 1, "level_db": 10.0},
    "SETD^s.2.mtx.0.value^1.0000")
sui("set_matrix_send_mute", {"source": "sub", "channel": 1, "matrix": 2, "muted": True}, "SETD^s.0.mtx.1.mute^1")
sui("set_matrix_send_pan", {"source": "aux", "channel": 2, "matrix": 2, "pan": 0.5}, "SETD^a.1.mtx.1.pan^0.5000")
sui("set_matrix_send_post_proc", {"source": "aux", "channel": 1, "matrix": 1, "post_proc": True},
    "SETD^a.0.mtx.0.postproc^1")
sui("set_master_matrix_send", {"matrix": 3, "level": 0.25}, "SETD^m.mtx.2.value^0.2500")
sui("set_master_matrix_send_db", {"matrix": 1, "level_db": -100.0}, "SETD^m.mtx.0.value^0.0000")
sui("set_master_matrix_send_mute", {"matrix": 1, "muted": True}, "SETD^m.mtx.0.mute^1")
sui("set_master_matrix_send_pan", {"matrix": 2, "pan": 0.0}, "SETD^m.mtx.1.pan^0.0000")
sui("set_master_matrix_send_post_proc", {"matrix": 2, "post_proc": False}, "SETD^m.mtx.1.postproc^0")
sui("set_aux_matrix_mode", {"aux": 9, "matrix": True}, "SETD^a.8.matrix^1")
sui("set_master_fader", {"level": 0.7}, "SETD^m.mix^0.7000")
sui("set_master_fader_db", {"level_db": -100.0}, "SETD^m.mix^0.0000")
sui("set_master_pan", {"pan": 0.5}, "SETD^m.pan^0.5000")
sui("set_master_dim", {"dimmed": True}, "SETD^m.dim^1")
sui("set_master_delay", {"side": "right", "delay_ms": 500.0}, "SETD^m.delayR^0.5000")
# The Ui24R's preamps are hw.<n>; the Ui16's and Ui12's are the input channels.
sui("set_input_gain", {"input": 1, "gain": 0.5}, "SETD^hw.0.gain^0.5000")
V.append({"spec": SUI, "model": "ui16", "command": "set_input_gain", "input": {"input": 12, "gain": 0.25},
          "expect_wire": "3:::SETD^i.11.gain^0.2500", "file": "set_input_gain-ui16"})
# Ui24R -6 to 57 dB: 25.5 dB is the middle. Ui12 -40 to 50 dB: 5 dB is the middle.
sui("set_input_gain_db", {"input": 2, "gain_db": 25.5}, "SETD^hw.1.gain^0.5000")
V.append({"spec": SUI, "model": "ui12", "command": "set_input_gain_db", "input": {"input": 8, "gain_db": 5.0},
          "expect_wire": "3:::SETD^i.7.gain^0.5000", "file": "set_input_gain_db-ui12"})
sui("set_phantom", {"input": 20, "enabled": True}, "SETD^hw.19.phantom^1")
V.append({"spec": SUI, "model": "ui12", "command": "set_phantom", "input": {"input": 1, "enabled": False},
          "expect_wire": "3:::SETD^i.0.phantom^0", "file": "set_phantom-ui12"})
sui("set_eq_bypass", {"strip": "sub", "channel": 1, "bypassed": True}, "SETD^s.0.eq.bypass^1")
sui("set_eq_band_frequency", {"strip": "input", "channel": 1, "band": 2, "position": 0.5}, "SETD^i.0.eq.b2.freq^0.5000")
sui("set_eq_band_q", {"strip": "fx", "channel": 1, "band": 4, "position": 0.3}, "SETD^f.0.eq.b4.q^0.3000")
# -20 to 20 dB linear: +5 dB is 0.625.
sui("set_eq_band_gain", {"strip": "line", "channel": 1, "band": 1, "gain_db": 5.0}, "SETD^l.0.eq.b1.gain^0.6250")
sui("set_hpf_frequency", {"strip": "input", "channel": 4, "position": 0.2}, "SETD^i.3.eq.hpf.freq^0.2000")
sui("set_hpf_slope", {"strip": "player", "channel": 1, "slope": "24"}, "SETD^p.0.eq.hpf.slope^1")
sui("set_lpf_frequency", {"channel": 1, "position": 0.9}, "SETD^i.0.eq.lpf.freq^0.9000")
sui("set_lpf_slope", {"channel": 2, "slope": "36"}, "SETD^i.1.eq.lpf.slope^2")
sui("set_fx_bpm", {"fx": 2, "bpm": 120}, "SETD^f.1.bpm^120")
sui("set_fx_param", {"fx": 1, "param": 6, "value": 0.1}, "SETD^f.0.par6^0.1000")
sui("set_mute_groups", {"mask": 5}, "SETD^mgmask^5")
sui("set_automix_group", {"channel": 5, "group": "none"}, "SETD^i.4.amixgroup^-1")
sui("set_automix_weight", {"channel": 1, "weight_db": 6.0}, "SETD^i.0.amix^0.7500")
sui("set_automix_enabled", {"group": "b", "enabled": True}, "SETD^automix.b.on^1")
sui("set_automix_response_time", {"position": 0.5}, "SETD^automix.time^0.5000")
sui("set_multitrack_arm", {"strip": "line", "channel": 1, "armed": True}, "SETD^l.0.mtkrec^1")
sui("set_solo_level", {"level": 0.6}, "SETD^settings.solovol^0.6000")
sui("set_solo_level_db", {"level_db": 10.0}, "SETD^settings.solovol^1.0000")
sui("set_headphone_level", {"headphone": 2, "level": 0.4}, "SETD^settings.hpvol.1^0.4000")
sui("set_headphone_level_db", {"headphone": 1, "level_db": -100.0}, "SETD^settings.hpvol.0^0.0000")
sui("player_play", {}, "MEDIA_PLAY")
sui("player_pause", {}, "MEDIA_PAUSE")
sui("player_stop", {}, "MEDIA_STOP")
sui("player_next", {}, "MEDIA_NEXT")
sui("player_previous", {}, "MEDIA_PREV")
sui("load_playlist", {"playlist": "Walk-in"}, "MEDIA_SWITCH_PLIST^Walk-in")
sui("load_track", {"playlist": "Walk-in", "track": "01 Intro.mp3"}, "MEDIA_SWITCH_TRACK^Walk-in^01 Intro.mp3")
sui("set_player_shuffle", {"enabled": False}, "SETD^settings.shuffle^0")
sui("set_player_mode", {"mode": "auto"}, "SETD^settings.playMode^3")
sui("record_toggle", {}, "RECTOGGLE")
sui("multitrack_play", {}, "MTK_PLAY")
sui("multitrack_pause", {}, "MTK_PAUSE")
sui("multitrack_stop", {}, "MTK_STOP")
sui("multitrack_record_toggle", {}, "MTK_REC_TOGGLE")
sui("set_multitrack_soundcheck", {"enabled": True}, "SETD^var.mtk.soundcheck^1")
sui("load_show", {"show": "Sunday"}, "LOADSHOW^Sunday")
sui("load_snapshot", {"show": "Sunday", "snapshot": "Band"}, "LOADSNAPSHOT^Sunday^Band")
sui("load_cue", {"show": "Sunday", "cue": "Sermon"}, "LOADCUE^Sunday^Sermon")
sui("save_snapshot", {"show": "Sunday", "snapshot": "Band"}, "SAVESNAPSHOT^Sunday^Band")
sui("save_cue", {"show": "Sunday", "cue": "Sermon"}, "SAVECUE^Sunday^Sermon")
sui("get_shows", {}, "SHOWLIST", device_reply="3:::SHOWLIST^Default^Sunday^",
    expect_result={"ok": {"kind": "value", "value": "Default^Sunday^"}})
sui("get_snapshots", {"show": "Sunday"}, "SNAPSHOTLIST^Sunday", device_reply="3:::SNAPSHOTLIST^Sunday^Band^Speech^",
    expect_result={"ok": {"kind": "value", "value": "Band^Speech^"}})
sui("get_cues", {"show": "Sunday"}, "CUELIST^Sunday", device_reply="3:::CUELIST^Sunday^Sermon^",
    expect_result={"ok": {"kind": "value", "value": "Sermon^"}})
sui("get_playlists", {}, "MEDIA_GET_PLISTS", device_reply="3:::PLISTS^Walk-in^Break^",
    expect_result={"ok": {"kind": "value", "value": "Walk-in^Break^"}})
sui("get_playlist_tracks", {"playlist": "Break"}, "MEDIA_GET_PLIST_TRACKS^Break",
    device_reply="3:::PLIST_TRACKS^Break^a.mp3^b.wav^",
    expect_result={"ok": {"kind": "value", "value": "a.mp3^b.wav^"}})
sui("select_channel", {"index": 38}, "BMSG^SYNC^SYNC_ID^38")

# The parameter feed: the dump on connecting holds many lines in one message.
telemetry(SUI, "dump", inbound_ws="3:::SETD^i.0.mix^0.75\nSETD^i.0.mute^1\nSETS^i.0.name^KICK\n"
          "SETD^i.0.pan^0.5\nSETS^model^ui24\nSETS^firmware^1.0.7720\nSETD^m.mix^1\nSETD^m.dim^0",
          expect_state={
              "values": {"i.0.mix": 0.75, "i.0.mute": 1.0, "i.0.pan": 0.5, "m.mix": 1.0, "m.dim": 0.0},
              "texts": {"i.0.name": "KICK", "model": "ui24", "firmware": "1.0.7720"},
              "device": {"model": "ui24", "firmware": "1.0.7720"},
              "inputs": {"1": {"fader": 0.75, "fader_db": -0.55, "mute": True, "name": "KICK", "pan": 0.5}},
              "master": {"fader": 1.0, "fader_db": 10.0, "dim": False}})
telemetry(SUI, "aux-solo", inbound_ws="3:::SETD^a.9.solo^1",
          expect_state={"values": {"a.9.solo": 1.0}, "auxes": {"10": {"solo": True}}})
telemetry(SUI, "vca-fader-down", inbound_ws="3:::SETD^v.5.mix^0",
          expect_state={"values": {"v.5.mix": 0.0}, "vcas": {"6": {"fader": 0.0, "fader_db": -100.0}}})
# mgmask: groups 1 and 3, and mute all (1 + 4 + 8388608).
telemetry(SUI, "mute-groups", inbound_ws="3:::SETD^mgmask^8388613",
          expect_state={"values": {"mgmask": 8388613.0},
                        "mute_groups": {"mask": 8388613, "1": {"muted": True}, "2": {"muted": False},
                                        "3": {"muted": True}, "4": {"muted": False}, "5": {"muted": False},
                                        "6": {"muted": False}, "fx": False, "all": True}})
telemetry(SUI, "player", inbound_ws="3:::SETD^var.currentState^2\nSETS^var.currentPlaylist^Walk-in\n"
          "SETS^var.currentTrack^01 Intro.mp3\nSETD^var.currentLength^184\nSETD^var.currentTrackPos^0.25\n"
          "SETD^settings.shuffle^1\nSETD^settings.playMode^3",
          expect_state={
              "values": {"var.currentState": 2.0, "var.currentLength": 184.0, "var.currentTrackPos": 0.25,
                         "settings.shuffle": 1.0, "settings.playMode": 3.0},
              "texts": {"var.currentPlaylist": "Walk-in", "var.currentTrack": "01 Intro.mp3"},
              "player": {"state": "playing", "playlist": "Walk-in", "track": "01 Intro.mp3", "length_s": 184.0,
                         "position": 0.25, "shuffle": True, "mode": "auto"}})
telemetry(SUI, "multitrack", inbound_ws="3:::SETD^var.mtk.currentState^1\nSETD^var.mtk.rec.currentState^1\n"
          "SETD^var.mtk.rec.time^12.5\nSETD^var.isRecording^0\nSETS^var.currentShow^Sunday",
          expect_state={
              "values": {"var.mtk.currentState": 1.0, "var.mtk.rec.currentState": 1.0, "var.mtk.rec.time": 12.5,
                         "var.isRecording": 0.0},
              "texts": {"var.currentShow": "Sunday"},
              "multitrack": {"state": "paused", "recording": True, "record_time_s": 12.5},
              "recorder": {"recording": False},
              "shows": {"current": "Sunday"}})
telemetry(SUI, "master-delay", inbound_ws="3:::SETD^m.delayL^0.25",
          expect_state={"values": {"m.delayL": 0.25}, "master": {"delay_left_ms": 250.0}})
telemetry(SUI, "sync", inbound_ws="3:::BMSG^SYNC^SYNC_ID^38", expect_state={"sync": {"SYNC_ID": {"selected_channel": 38}}})
# The lists: each replaces what was there, and each item has more read.
telemetry(SUI, "show-list", inbound_ws="3:::SHOWLIST^Default^Sunday^",
          state_before={"shows": {"names": {"0": "Default", "1": "Old", "2": "Deleted"}}},
          expect_then_send=["3:::SNAPSHOTLIST^Default", "3:::CUELIST^Default",
                            "3:::SNAPSHOTLIST^Sunday", "3:::CUELIST^Sunday"],
          expect_state={"shows": {"names": {"0": "Default", "1": "Sunday"}}})
telemetry(SUI, "snapshot-list", inbound_ws="3:::SNAPSHOTLIST^Sunday^Band^Speech^",
          expect_state={"shows": {"snapshots": {"Sunday": {"0": "Band", "1": "Speech"}}}})
telemetry(SUI, "cue-list", inbound_ws="3:::CUELIST^Sunday^Sermon^",
          expect_state={"shows": {"cues": {"Sunday": {"0": "Sermon"}}}})
telemetry(SUI, "playlists", inbound_ws="3:::PLISTS^Walk-in^",
          expect_then_send=["3:::MEDIA_GET_PLIST_TRACKS^Walk-in"],
          expect_state={"playlists": {"names": {"0": "Walk-in"}}})
telemetry(SUI, "playlist-tracks", inbound_ws="3:::PLIST_TRACKS^Walk-in^01 Intro.mp3^02.wav^",
          expect_state={"playlists": {"tracks": {"Walk-in": {"0": "01 Intro.mp3", "1": "02.wav"}}}})
