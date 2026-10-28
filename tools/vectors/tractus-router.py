# Tractus Router for NDI: the routes on its product page
# (tractusevents.com/tools/ndi-router, "API Documentation"). Tractus gives no
# port, method or answer: requests are GETs on the port the router is opened
# with (the runner's stand-in), and the lists are kept whole as JSON text.
TR = "tractus-router"


def _tr(command, input, target, **extra):
    V.append({"spec": TR, "command": command, "input": input,
              "expect_request": {"method": "GET", "target": target}, **extra})


_tr("get_slots", {}, "/slots")
_tr("get_sources", {}, "/sources")
_tr("set_slot_source", {"slot": "Stage Left", "source": "STUDIO-PC (Camera 1)"},
    "/slot/Stage%20Left/set/STUDIO-PC%20%28Camera%201%29")
_tr("clear_slot", {"slot": "Stage Left"}, "/slot/Stage%20Left/clear")
_tr("add_slot", {"slot": "Confidence"}, "/slots/add/Confidence")
_tr("add_slot_with_source", {"slot": "Confidence", "source": "MIXER (Program)"}, "/slots/add/Confidence/MIXER%20%28Program%29")
_tr("delete_slot", {"slot": "Confidence"}, "/slots/delete/Confidence")
_tr("rename_slot", {"slot": "Confidence", "new_name": "Confidence 2"}, "/slot/Confidence/rename/Confidence%202")
_tr("reset", {}, "/reset", http_reply={"status": 500, "body": ""},
    expect_result={"error": {"error": "device_error", "code": "500"}})

telemetry(TR, "slots", inbound_http={"path": "/slots", "body": '[{"name":"Stage Left","source":"CAM 1"}]'},
          expect_state={"slots": '[{"name":"Stage Left","source":"CAM 1"}]'})
telemetry(TR, "sources", inbound_http={"path": "/sources", "body": '["CAM 1","CAM 2"]'},
          expect_state={"sources": '["CAM 1","CAM 2"]'})
