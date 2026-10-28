# Atlona AT-UHD-SW-510W REST API (atlona-sw510w-rest): GET /API?method=... as
# the API document's examples (p.29-33) write them.
ASR = "atlona-sw510w-rest"

V.append({"spec": ASR, "command": "get_wifi_fence", "input": {},
          "expect_request": {"method": "GET", "target": "/API?method=Net%3AWiFiFence%3AGet"},
          "http_reply": {"status": 200, "body": '{"result":{"duration":2,"period":5000,"success":true,"enable":false,"timethreshold":60000,"threshold":-75}}'},
          "expect_result": {"ok": {"kind": "value", "value": {"duration": 2, "period": 5000, "success": True, "enable": False, "timethreshold": 60000, "threshold": -75}}}})
V.append({"spec": ASR, "command": "set_wifi_fence",
          "input": {"enable": False, "period": 5000, "threshold": -20, "duration": 2, "timethreshold": 60000},
          "expect_request": {"method": "GET", "target": "/API?method=Net%3AWiFiFence%3ASet&enable=false&period=5000&threshold=-20&duration=2&timethreshold=60000"}})
V.append({"spec": ASR, "command": "kick_wifi_station", "input": {"mac": "30:59:b7:0f:d6:cc", "timeout": 10},
          "expect_request": {"method": "GET", "target": "/API?method=Net%3AWiFiStations%3AKick&mac=30%3A59%3Ab7%3A0f%3Ad6%3Acc&timeout=10"}})
V.append({"spec": ASR, "command": "get_maintenance_time", "input": {},
          "expect_request": {"method": "GET", "target": "/API?method=System%3AMaintenanceTime%3AGet"}})
V.append({"spec": ASR, "command": "set_maintenance_time",
          "input": {"enable": True, "hour": 0, "minute": 0, "timezone": "UTC", "monday": True},
          "expect_request": {"method": "GET", "target": "/API?method=System%3AMaintenanceTime%3ASet&enable=true&hour=0&minute=0&timezone=UTC&monday=true&tuesday=false&wednesday=false&thursday=false&friday=false&saturday=false&sunday=false"}})
telemetry(ASR, "wifi-fence", inbound_http={"path": "/API?method=Net%3AWiFiFence%3AGet",
          "body": '{"result":{"duration":2,"period":5000,"success":true,"enable":false,"timethreshold":60000,"threshold":-75}}'},
          expect_state={"wifi_fence": {"enabled": False, "period_ms": 5000, "threshold_db": -75, "duration_min": 2, "time_threshold_ms": 60000}})
telemetry(ASR, "maintenance", inbound_http={"path": "/API?method=System%3AMaintenanceTime%3AGet",
          "body": '{"result":{"sunday":true,"saturday":false,"tuesday":false,"enable":false,"timezone":"America/Los_Angeles","next_maintenance":"2019-03-10T00:00:00.521-08:00","wednesday":false,"thursday":false,"friday":false,"time":"00:00","monday":false}}'},
          expect_state={"maintenance": {"enabled": False, "time": "00:00", "timezone": "America/Los_Angeles",
                                        "next": "2019-03-10T00:00:00.521-08:00",
                                        "days": {"monday": False, "tuesday": False, "wednesday": False, "thursday": False,
                                                 "friday": False, "saturday": False, "sunday": True}}})
