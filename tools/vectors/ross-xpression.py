XP = "ross-xpression"
# XPression RossTalk (Ross Video online help, RossTalk > XPression Commands,
# help build 2026013010): ASCII command + CR LF on TCP 7788, never
# acknowledged. Framebuffers are 1-based in XPression and 0-based on the wire.
# Expected lines are written from the guide's syntax lines and examples.
XU = {"ok": {"kind": "unverified"}}

text(XP, "focus_story_channel", {"channel": 2, "rundown": "0001", "story": "Opening"},
     "CHANFOCUSSTORY 2:0001:Opening\r\n", expect_result=XU)                    # guide example
text(XP, "clear_framebuffer", {"framebuffer": 1}, "CLFB 0\r\n", expect_result=XU)  # guide: CLFB 0000 clears fb 1
text(XP, "clear_layer", {"framebuffer": 1, "layer": 2}, "CLFB 0:2\r\n")         # guide: CLFB 0000:2
text(XP, "clear_all", {}, "CLRA\r\n")
text(XP, "cue", {"take_id": 3, "framebuffer": 3, "layer": -5}, "CUE 3:2:-5\r\n")  # guide example
text(XP, "set_datalinq_key", {"take_id": 2, "key": "Names", "value": "John"},
     "DATALINQKEY 0002:Names:John\r\n")                                        # guide example
text(XP, "sequencer_down", {}, "DOWN\r\n")
text(XP, "focus_story", {"rundown": "0003", "story": "Halftime"}, "FOCUSSTORY 0003:Halftime\r\n")  # guide example
text(XP, "focus", {"take_id": 5}, "FOCUS 0005\r\n")                             # guide example
text(XP, "game_state", {}, "GAMESTATE\r\n")
text(XP, "gpi", {"number": 5}, "GPI 5\r\n")                                     # guide example
text(XP, "layer_off", {"framebuffer": 1, "layer": 2}, "LAYEROFF 0:2\r\n")      # guide: LAYEROFF 0000:2
text(XP, "next", {}, "NEXT\r\n")
text(XP, "read", {}, "READ\r\n")
text(XP, "resume_framebuffer", {"framebuffer": 1}, "RESUME 0\r\n")             # guide: RESUME 0000
text(XP, "resume_layer", {"framebuffer": 1, "layer": 2}, "RESUME 0:2\r\n")     # guide: RESUME 0000:2
text(XP, "route_ip_input", {"input": 1, "address": "192.168.10.20", "port": 5000},
     "ROUTEIPIN 1:192.168.10.20:5000\r\n")
text(XP, "route_ip_input_audio", {"input": 1, "address": "192.168.10.20", "port": 5000,
                                  "audio_address": "192.168.10.21", "audio_port": 5002},
     "ROUTEIPIN 1:192.168.10.20:5000:AUDIO:192.168.10.21:5002\r\n")            # guide example
text(XP, "take_online", {"take_id": 5, "layer": 7}, "SEQI 0005:7\r\n")          # guide example
text(XP, "take_offline", {"take_id": 5}, "SEQO 0005\r\n")                       # guide example
text(XP, "swap", {"framebuffer": 1}, "SWAP 0\r\n")                              # guide example
text(XP, "swap_all", {}, "SWAP\r\n")
text(XP, "take", {"take_id": 5, "framebuffer": 1, "layer": 7}, "TAKE 5:0:7\r\n")  # guide example
text(XP, "set_template_data", {"take_id": 2, "object": "Text1", "property": "Visibility", "value": "0"},
     "TEMPLATEDATA 0002:Text1:Visibility:0\r\n")                               # guide example
text(XP, "uncue_all", {}, "UNCUEALL\r\n")
text(XP, "uncue", {"take_id": 12}, "UNCUE 0012\r\n")
text(XP, "sequencer_up", {}, "UP\r\n")
text(XP, "up_next", {"take_id": 1001}, "UPNEXT 1001\r\n")
