# Tractus FullView for NDI: the HTTP API on port 9901 (Tractus Tools for NDI,
# HTTP API). The page's own example: GET /display/0/source/ALPHAVII%20%28Test%20Pattern%29.
TF = "tractus-fullview"

V.append({"spec": TF, "command": "set_display_source", "input": {"display": 0, "source": "ALPHAVII (Test Pattern)"},
          "expect_request": {"method": "GET", "target": "/display/0/source/ALPHAVII%20%28Test%20Pattern%29"},
          })
V.append({"spec": TF, "command": "get_displays", "input": {},
          "expect_request": {"method": "GET", "target": "/displays"}})
V.append({"spec": TF, "command": "get_debug", "input": {}, "expect_request": {"method": "GET", "target": "/debug"},
          "http_reply": {"status": 200, "body": "SdlEvents: 0.2\nBlitRender: 6.1\nRenderLoop: 16.6"},
          "expect_result": {"ok": {"kind": "value", "value": "SdlEvents: 0.2\nBlitRender: 6.1\nRenderLoop: 16.6"}}})
telemetry(TF, "displays", inbound_http={"path": "/displays", "body": '[{"index":0},{"index":1}]'},
          expect_state={"displays": '[{"index":0},{"index":1}]'})
