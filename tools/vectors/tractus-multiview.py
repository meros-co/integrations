# Tractus Multiview for NDI: the HTTP API on port 8901 (Tractus, HTTP API
# Details, multiviewforndi.com/documentation/webapi). Routes are the page's;
# viewers and viewports count from 0 (Caption/UMD and Expressions). The page
# names the answer fields of /viewers, /receivers and /current but shows no
# answer, so the replies below are built from those names.
TM = "tractus-multiview"
_OK = {"ok": {"kind": "ack"}}


def _tm(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": TM, "command": command, "input": input, "expect_request": req, **extra})


_tm("get_sources", {}, "GET", "/sources")
_tm("refresh_sources", {}, "GET", "/sources?forcereload=true")
_tm("get_viewers", {}, "GET", "/viewers")
_tm("get_receivers", {}, "GET", "/receivers")
_tm("get_current", {}, "GET", "/current",
    http_reply={"status": 200, "body": '{"Version":"2.4.1","BuildType":"production"}'},
    expect_result={"ok": {"kind": "value", "value": {"Version": "2.4.1", "BuildType": "production"}}})
_tm("get_layouts", {}, "GET", "/layouts")
_tm("create_viewer", {}, "GET", "/viewers/create")
_tm("set_viewer_count", {"count": 2}, "GET", "/viewers/set/2")
_tm("shutdown_viewer", {"viewer": 1}, "GET", "/viewer/1/shutdown")
_tm("rename_viewer", {"viewer": 0, "name": "Gallery A"}, "GET", "/viewer/0/rename/Gallery%20A")
_tm("set_layout", {"viewer": 0, "layout": "4X4"}, "GET", "/viewer/0/layout/4X4")
_tm("set_output_source", {"viewer": 0, "output": 3, "source": "STUDIO-PC (Camera 1)"}, "GET",
    "/viewer/0/output/3/sourcename/STUDIO-PC%20%28Camera%201%29")
_tm("set_output_source", {"viewer": 0, "output": 3, "source": "STUDIO-PC (Camera 1)"}, "GET",
    "/viewer/0/output/3/sourcename/STUDIO-PC%20%28Camera%201%29", file="set_output_source.error",
    http_reply={"status": 404, "body": ""}, expect_result={"error": {"error": "device_error", "code": "404"}})
_tm("clear_output", {"viewer": 0, "output": 3}, "GET", "/viewer/0/output/3/clear")
_tm("set_ndi_output", {"viewer": 0, "enabled": True}, "GET", "/viewer/0/ndi/true")
_tm("save_viewer_preset", {"viewer": 0, "bank": "1"}, "GET", "/viewer/0/savepreset/1")
_tm("load_viewer_preset", {"viewer": 0, "bank": "1"}, "GET", "/viewer/0/loadpreset/1")
_tm("save_output_preset", {"viewer": 0, "output": 2, "bank": "4"}, "GET", "/viewer/0/output/2/savepreset/4")
_tm("load_output_preset", {"viewer": 0, "output": 2, "bank": "4"}, "GET", "/viewer/0/output/2/loadpreset/4")
_tm("listen_viewport", {"viewer": 0, "viewport": 5}, "GET", "/viewer/0/listen/viewport/5")
_tm("listen_source", {"viewer": 0, "source": "MIXER (Program)"}, "GET", "/viewer/0/listen/source/MIXER%20%28Program%29")
_tm("stop_listening", {"viewer": 0}, "GET", "/viewer/0/listen/stop")
_tm("project_viewport", {"viewer": 1, "viewport": 0}, "GET", "/viewer/1/project/viewport/0")
_tm("project_source", {"viewer": 1, "source": "MIXER (Program)"}, "GET", "/viewer/1/project/source/MIXER%20%28Program%29")
_tm("stop_projecting", {"viewer": 1}, "GET", "/viewer/1/project/stop")
_tm("focus_viewport", {"viewer": 0, "viewport": 2}, "GET", "/viewer/0/focus/2")
_tm("set_caption_text", {"viewer": 0, "code": "umd1", "text": "CAM 1"}, "GET", "/viewer/0/caption/update/umd1/CAM%201",
    http_reply={"status": 200, "body": ""}, expect_result=_OK)
_tm("set_caption_background", {"viewer": 0, "code": "umd1", "rgba": "#FF000080"}, "GET",
    "/viewer/0/caption/updatebg/umd1/%23FF000080")
_tm("set_caption_position", {"viewer": 0, "code": "umd1", "x": 100, "y": 980}, "GET",
    "/viewer/0/caption/updatepos/umd1/100/980")
_tm("hide_caption", {"viewer": 0, "code": "umd1"}, "GET", "/viewer/0/caption/hide/umd1")
_tm("show_caption", {"viewer": 0, "code": "umd1"}, "GET", "/viewer/0/caption/show/umd1")
_tm("remove_caption", {"viewer": 0, "code": "umd1"}, "DELETE", "/viewer/0/caption/remove/umd1")
_tm("clear_captions", {"viewer": 0}, "GET", "/viewer/0/caption/clearall")
_tm("update_caption", {"viewer": 0, "caption": {"Code": "umd1", "Text": "CAM 1"}}, "POST", "/viewer/0/caption/update",
    '{"Code":"umd1","Text":"CAM 1"}')
_tm("export_layout", {"viewer": 0}, "GET", "/viewer/0/exportlayout")
_tm("import_layout", {"viewer": 0, "layout": {"Viewports": []}}, "POST", "/viewer/0/importlayout", '{"Viewports":[]}')
_tm("apply_layout_template", {"viewer": 0, "template": {"Code": "3X3"}}, "POST", "/viewer/0/layout", '{"Code":"3X3"}')
_tm("shutdown", {}, "GET", "/shutdown")
_tm("get_expressions", {}, "GET", "/expressions")
_tm("get_launch_options", {}, "GET", "/debug/launchoptions")
_tm("get_diagnostics", {}, "GET", "/debug/diagnostics")
_tm("get_debug", {}, "GET", "/debug",
    http_reply={"status": 200, "body": "Iteration: 4.1 ms"}, expect_result={"ok": {"kind": "value", "value": "Iteration: 4.1 ms"}})


def _tel(name, path, body, expect_state, **extra):
    telemetry(TM, name, inbound_http={"path": path, "body": json.dumps(body, separators=(",", ":"))},
              expect_state=expect_state, **extra)


_VIEWER = {"Name": "Gallery A", "ViewerIndex": 0, "NdiStatus": True, "LayoutModeCode": "2L8S", "Layout": None,
           "AudioChannels": 2, "AudioSampleRate": 48000, "AudioPeakdBFS": [-18.5, -19.0],
           "AudioMonitorSourceName": "MIXER (Program)",
           "ProjectorSource": {"NdiSource": "", "UseLowBandwidth": False, "IsHDR": False},
           "Outputs": [{"NdiSource": "STUDIO-PC (Camera 1)", "Name": "CAM 1", "UseLowBandwidth": True,
                        "IsHDR": False, "VideoWatchdogTripped": False}],
           "FocusedViewportIndex": 0}
_tel("viewers", "/viewers", [_VIEWER],
     {"viewers": {"0": {"name": "Gallery A", "ndi_output": True, "layout": "2L8S", "audio_channels": 2,
                        "audio_sample_rate": 48000, "audio_peak_dbfs": "[-18.5,-19.0]",
                        "audio_monitor_source": "MIXER (Program)", "projector_source": "", "projector_hdr": False,
                        "focused_viewport": 0,
                        "outputs": {"0": {"source": "STUDIO-PC (Camera 1)", "name": "CAM 1", "low_bandwidth": True,
                                          "hdr": False, "video_watchdog_tripped": False}}}}},
     state_before={"viewers": {"1": {"name": "Gone"}}})
# The same fields in camelCase.
_tel("viewers-camel", "/viewers", [{"name": "Gallery B", "viewerIndex": 1, "ndiStatus": False, "layoutModeCode": "3X3",
                                    "outputs": [{"ndiSource": "CAM 2", "name": "Two", "isHDR": True}]}],
     {"viewers": {"1": {"name": "Gallery B", "ndi_output": False, "layout": "3X3",
                        "outputs": {"0": {"source": "CAM 2", "name": "Two", "hdr": True}}}}})
_tel("receivers", "/receivers",
     [{"SourceName": "STUDIO-PC (Camera 1)", "SubscribedRenderers": 2, "PreviewTally": False, "ProgramTally": True,
       "IsHDR": False, "LowBandwidth": True, "ReceiverWatchdogTimeout": 5000, "SupportsPtz": True, "SupportsKvm": False}],
     {"receivers": {"STUDIO-PC (Camera 1)": {"subscribed_renderers": 2, "preview_tally": False, "program_tally": True,
                                             "hdr": False, "low_bandwidth": True, "watchdog_timeout": "5000",
                                             "supports_ptz": True, "supports_kvm": False}}})
_tel("current", "/current", {"Version": "2.4.1", "InstanceName": "MV-1", "BuildType": "production",
                             "DefaultInstances": 1, "NdiSenderType": "Normal", "Sources": [], "Viewers": []},
     {"app": {"version": "2.4.1", "instance_name": "MV-1", "build_type": "production", "default_instances": "1",
              "ndi_sender_type": "Normal"}})
_tel("sources", "/sources", [{"Name": "STUDIO-PC (Camera 1)"}], {"sources": '[{"Name":"STUDIO-PC (Camera 1)"}]'})
_tel("layouts", "/layouts", [{"Code": "2X2"}], {"layouts": '[{"Code":"2X2"}]'})
