TVU = "teradek-vidiu"
# Teradek VidiU Go web API, as the MIT-licensed Companion module (index.js,
# commit e4d75b7c) uses it: GET /cgi-bin/system.cgi and /cgi-bin/json.cgi
# with the serenity-session cookie of the api.cgi login.


def tvu(command, input, target, **extra):
    http(TVU, command, input, "GET", target, **extra)


STATUS = ('{"result":"success","status":{"System-Power":"Battery:76:0:Charging","Video-Input":"HDMI:1080p30",'
          '"Network-Uplink":"Wired","Broadcast-State":"Live","Codec-State":"Running","Broadcast-Error":"none",'
          '"Record-Status":"1:00:10:12"}}')
tvu("get_status", {}, "/cgi-bin/system.cgi?command=status",
    http_reply={"status": 200, "body": '{"result":"success","status":{"Broadcast-State":"Ready"}}'},
    expect_result={"ok": {"kind": "value", "value": {"Broadcast-State": "Ready"}}})
tvu("get_product", {}, "/cgi-bin/json.cgi?command=geti&q=System.Info.Product",
    http_reply={"status": 200, "body": '{"System":{"Info":{"Product":{"productname":"VidiU Go"}}}}'},
    expect_result={"ok": {"kind": "value", "value": {"productname": "VidiU Go"}}})
tvu("get_setting", {"key": "Codec.Status"}, "/cgi-bin/json.cgi?command=geti&q=Codec.Status")
tvu("start_broadcast", {}, "/cgi-bin/system.cgi?command=broadcast&action=start",
    http_reply={"status": 200, "body": '{"result":"success"}'}, expect_result={"ok": {"kind": "ack"}})
tvu("stop_broadcast", {}, "/cgi-bin/system.cgi?command=broadcast&action=stop")
tvu("start_recording", {}, "/cgi-bin/system.cgi?command=recording&action=start")
tvu("stop_recording", {}, "/cgi-bin/system.cgi?command=recording&action=stop",
    http_reply={"status": 200, "body": '{"result":"failed"}'}, expect_result={"error": {"error": "device_error"}},
    file="stop_recording-failed")

telemetry(TVU, "status", inbound_http={"path": "/cgi-bin/system.cgi?command=status", "body": STATUS},
          expect_state={"power": {"source": "Battery", "battery_percent": 76, "charging": "Charging"},
                        "video": {"input": "HDMI", "state": "1080p30"},
                        "network": {"uplink": "Wired"},
                        "broadcast": {"state": "Live", "live": True, "error": "none"},
                        "codec": {"state": "Running"},
                        "recording": {"active": True}})
telemetry(TVU, "product", inbound_http={"path": "/cgi-bin/json.cgi?command=geti&q=System.Info.Product", "body":
          '{"System":{"Info":{"Product":{"productname":"VidiU Go","serialnumber":"123456","productversion":"3.0.8"}}}}'},
          expect_state={"product": {"name": "VidiU Go", "serial": "123456", "firmware": "3.0.8"}})
