BDC = "birddog-central"
# BirdDog Central 2.0: POST /?cmd=<cmd>&subcmd=<subcmd>&<params> on port 8080, as
# the MIT-licensed Companion module (index.js, actions.js, commit 740dcfae)
# sends them; values percent-encoded here.


def bdc(command, input, query, model=None, **extra):
    v = {"spec": BDC, "command": command, "input": input,
         "expect_request": {"method": "POST", "target": "/?" + query}, **extra}
    if model:
        v["model"] = model
    V.append(v)


V.append({"spec": BDC, "command": "get_version", "input": {}, "expect_request": {"method": "POST", "target": "/version"},
          "http_reply": {"status": 200, "body": '"2.0.1"'}, "expect_result": {"ok": {"kind": "value", "value": "2.0.1"}}})
bdc("get_server_info", {}, "cmd=server&subcmd=info",
    http_reply={"status": 200, "body": '{"version":"2.0.1","access":0}'},
    expect_result={"ok": {"kind": "value", "value": {"version": "2.0.1", "access": 0}}})
bdc("list_sources", {}, "cmd=source&subcmd=list")
bdc("list_destinations", {}, "cmd=dest&subcmd=list")
bdc("connect_source", {"host_name": "BIRDDOG-0B710", "format": "CAM", "destinations": ["10.0.0.21", "10.0.0.22"]},
    "cmd=source&subcmd=connect&hname=BIRDDOG-0B710&format=CAM"
    "&dest=%7B%22destinations%22%3A%5B%2210.0.0.21%22%2C%2210.0.0.22%22%5D%7D",
    http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
bdc("list_generators", {}, "cmd=gen&subcmd=list")
bdc("get_generator", {"generator": "Walk in"}, "cmd=gen&subcmd=info&gname=Walk%20in")
bdc("add_generator", {"generator": "Walk in"}, "cmd=gen&subcmd=add_fav_grp&gname=Walk%20in")
bdc("delete_generator", {"generator": "Walk in"}, "cmd=gen&subcmd=del_fav_grp&gname=Walk%20in")
bdc("list_generator_media", {}, "cmd=gen&subcmd=srs_list")
bdc("add_generator_media", {"generator": "Walk in", "media": "loop.mp4"},
    "cmd=gen&subcmd=add_srs&gname=Walk%20in&name=loop.mp4")
bdc("remove_generator_media", {"generator": "Walk in", "media": "loop.mp4"},
    "cmd=gen&subcmd=del_srs&gname=Walk%20in&name=loop.mp4")
bdc("generator_play", {"generator": "Walk in"}, "cmd=gen&subcmd=play&gname=Walk%20in")
bdc("generator_stop", {"generator": "Walk in"}, "cmd=gen&subcmd=stop&gname=Walk%20in")
bdc("generator_pause", {"generator": "Walk in"}, "cmd=gen&subcmd=pause&gname=Walk%20in")
bdc("generator_skip", {"generator": "Walk in"}, "cmd=gen&subcmd=skip&gname=Walk%20in")
bdc("set_generator_loop", {"generator": "Walk in", "loop": True}, "cmd=gen&subcmd=loop&gname=Walk%20in&loop=TRUE")
PRO = "central-pro"
bdc("connect_source_to_destination_group", {"group": "Stage", "host_name": "BIRDDOG-0B710", "format": "CAM"},
    "cmd=source&subcmd=connect&gname=Stage&hname=BIRDDOG-0B710&format=CAM", model=PRO)
bdc("list_source_groups", {}, "cmd=source&subcmd=fav_gnames", model=PRO)
bdc("get_source_group", {"group": "Cameras"}, "cmd=source&subcmd=fav_glist&gname=Cameras", model=PRO)
bdc("add_source_group", {"group": "Cameras"}, "cmd=source&subcmd=add_fav_grp&gname=Cameras", model=PRO)
bdc("delete_source_group", {"group": "Cameras"}, "cmd=source&subcmd=del_fav_grp&gname=Cameras", model=PRO)
bdc("add_source_to_group", {"group": "Cameras", "host_name": "BIRDDOG-0B710", "format": "CAM"},
    "cmd=source&subcmd=add_srs&gname=Cameras&hname=BIRDDOG-0B710&format=CAM", model=PRO)
bdc("remove_source_from_group", {"group": "Cameras", "host_name": "BIRDDOG-0B710", "format": "CAM"},
    "cmd=source&subcmd=del_srs&gname=Cameras&hname=BIRDDOG-0B710&format=CAM", model=PRO)
bdc("list_destination_groups", {}, "cmd=dest&subcmd=fav_gnames", model=PRO)
bdc("get_destination_group", {"group": "Stage"}, "cmd=dest&subcmd=fav_glist&gname=Stage", model=PRO)
bdc("add_destination_group", {"group": "Stage"}, "cmd=dest&subcmd=add_fav_grp&gname=Stage", model=PRO)
bdc("delete_destination_group", {"group": "Stage"}, "cmd=dest&subcmd=del_fav_grp&gname=Stage", model=PRO)
bdc("add_destination_to_group", {"group": "Stage", "host_name": "PLAY-1"},
    "cmd=dest&subcmd=add_srs&gname=Stage&hname=PLAY-1&format=", model=PRO)
bdc("remove_destination_from_group", {"group": "Stage", "host_name": "PLAY-1", "format": "DEC"},
    "cmd=dest&subcmd=del_srs&gname=Stage&hname=PLAY-1&format=DEC", model=PRO)
bdc("list_routers", {}, "cmd=router&subcmd=list", model=PRO)
bdc("get_router", {"router": "R1"}, "cmd=router&subcmd=info&name=R1", model=PRO)
bdc("add_router", {"router": "R1"}, "cmd=router&subcmd=add&name=R1", model=PRO)
bdc("delete_router", {"router": "R1"}, "cmd=router&subcmd=del&name=R1", model=PRO)
bdc("link_router", {"group": "Cameras", "router": "R1"}, "cmd=router&subcmd=link&gname=Cameras&name=R1", model=PRO)
bdc("router_connect_source", {"router": "R1", "host_name": "BIRDDOG-0B710", "format": "CAM"},
    "cmd=router&subcmd=sconnect&hname=BIRDDOG-0B710&format=CAM&name=R1", model=PRO)
bdc("router_connect_destinations", {"router": "R1", "destinations": ["10.0.0.21"]},
    "cmd=router&subcmd=dconnect&name=R1&dest=%7B%22destinations%22%3A%5B%2210.0.0.21%22%5D%7D", model=PRO)
bdc("list_retransmitters", {}, "cmd=retransmtr&subcmd=list", model=PRO)
bdc("get_retransmitter", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=info&name=RT1", model=PRO)
bdc("add_retransmitter", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=add&name=RT1", model=PRO)
bdc("delete_retransmitter", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=delete&name=RT1", model=PRO)
bdc("retransmitter_video_source", {"retransmitter": "RT1", "host_name": "BIRDDOG-0B710", "format": "CAM"},
    "cmd=retransmtr&subcmd=vconnect&hname=BIRDDOG-0B710&format=CAM&name=RT1", model=PRO)
bdc("retransmitter_audio_source", {"retransmitter": "RT1", "host_name": "MIXER", "format": "PGM"},
    "cmd=retransmtr&subcmd=aconnect&hname=MIXER&format=PGM&name=RT1", model=PRO)
bdc("retransmitter_play", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=play&name=RT1", model=PRO)
bdc("retransmitter_stop", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=stop&name=RT1", model=PRO)
bdc("retransmitter_stop_audio", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=astop&name=RT1", model=PRO)
bdc("retransmitter_stop_video", {"retransmitter": "RT1"}, "cmd=retransmtr&subcmd=vstop&name=RT1", model=PRO)

telemetry(BDC, "server", inbound_http={"path": "/?cmd=server&subcmd=info", "body": '{"version":"2.0.1","access":0}'},
          expect_state={"server": {"version": "2.0.1", "access": 0}})
telemetry(BDC, "sources", inbound_http={"path": "/?cmd=source&subcmd=list", "body":
          '{"ndi_find_results":[{"sources":[{"host_name":"BIRDDOG-0B710","format":"CAM"},'
          '{"host_name":"MIXER","format":"PGM"}]}]}'},
          state_before={"sources": {"2": {"host_name": "OLD", "format": "GONE"}}},
          expect_state={"sources": {"0": {"host_name": "BIRDDOG-0B710", "format": "CAM"},
                                    "1": {"host_name": "MIXER", "format": "PGM"}}})
telemetry(BDC, "destinations", inbound_http={"path": "/?cmd=dest&subcmd=list", "body":
          '{"mdns_find_results":[{"host_name":"PLAY-1","ip":"10.0.0.21"}]}'},
          expect_state={"destinations": {"PLAY-1": {"ip": "10.0.0.21"}}})
telemetry(BDC, "source-groups", inbound_http={"path": "/?cmd=source&subcmd=fav_gnames", "body": '{"group_names":["Cameras"]}'},
          expect_then_send=[{"method": "POST", "target": "/?cmd=source&subcmd=fav_glist&gname=Cameras"}],
          expect_state={"source_groups": {"0": {"name": "Cameras"}}})
telemetry(BDC, "source-group", inbound_http={"path": "/?cmd=source&subcmd=fav_glist&gname=Cameras", "body":
          '{"favourite_source_groups":[{"group_name":"Cameras","sources":[{"host_name":"BIRDDOG-0B710","format":"CAM"}]}]}'},
          expect_state={"source_groups_members": {"Cameras": {"0": {"host_name": "BIRDDOG-0B710", "format": "CAM"}}}})
telemetry(BDC, "destination-group", inbound_http={"path": "/?cmd=dest&subcmd=fav_glist&gname=Stage", "body":
          '{"favourite_destination_groups":[{"group_name":"Stage","destinations":[{"host_name":"PLAY-1"}]}]}'},
          expect_state={"destination_groups_members": {"Stage": {"0": {"host_name": "PLAY-1"}}}})
telemetry(BDC, "routers", inbound_http={"path": "/?cmd=router&subcmd=list", "body": '{"routers":[{"name":"R1"}]}'},
          expect_then_send=[{"method": "POST", "target": "/?cmd=router&subcmd=info&name=R1"}],
          expect_state={"router_names": {"0": "R1"}})
telemetry(BDC, "router", inbound_http={"path": "/?cmd=router&subcmd=info&name=R1", "body":
          '{"router":"R1","name":"BIRDDOG-0B710 (CAM)","src_group":"Cameras","url":"ndi://R1"}'},
          expect_state={"routers": {"R1": {"source": "BIRDDOG-0B710 (CAM)", "source_group": "Cameras", "url": "ndi://R1"}}})
telemetry(BDC, "generators", inbound_http={"path": "/?cmd=gen&subcmd=list", "body": '{"generators":[{"group_name":"Walk in"}]}'},
          expect_then_send=[{"method": "POST", "target": "/?cmd=gen&subcmd=info&gname=Walk%20in"}],
          expect_state={"generator_names": {"0": "Walk in"}})
telemetry(BDC, "generator", inbound_http={"path": "/?cmd=gen&subcmd=info&gname=Walk%20in", "body":
          '{"generators":[{"group_name":"Walk in","loop":"TRUE","playlist":["loop.mp4"],"status":"Playing"}]}'},
          expect_state={"generators": {"Walk in": {"status": "Playing", "loop": True, "playlist": '["loop.mp4"]'}}})
telemetry(BDC, "generator-media", inbound_http={"path": "/?cmd=gen&subcmd=srs_list", "body": '{"gen_sources":["loop.mp4","bars.png"]}'},
          expect_state={"generator_media": {"0": "loop.mp4", "1": "bars.png"}})
telemetry(BDC, "retransmitter", inbound_http={"path": "/?cmd=retransmtr&subcmd=info&name=RT1", "body":
          '{"retransmitters":[{"Name":"RT1","AudioNDIname":"MIXER (PGM)","AudioPlayStatus":"Playing","AudioURL":"",'
          '"VideoNDIname":"BIRDDOG-0B710 (CAM)","VideoPlayStatus":"Playing","VideoURL":""}]}'},
          expect_state={"retransmitters": {"RT1": {"audio_source": "MIXER (PGM)", "audio_status": "Playing", "audio_url": "",
                                                   "video_source": "BIRDDOG-0B710 (CAM)", "video_status": "Playing",
                                                   "video_url": ""}}})
