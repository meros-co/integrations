# AI-Media / EEG iCap (aimedia-icap): one vector per command, over the iCap
# REST API v1.0. Paths and JSON bodies are written by hand from the "EEG /
# Ai-Media iCap REST API Documentation v1.0" (company acme, user encoder1,
# access code 1234 are made up); query values are percent-encoded.
IC = "aimedia-icap"
_IC_S = {"company": "acme", "username": "admin", "password": "secret"}
_B = "/api/1.0/acme"


def _ic(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": IC, "command": command, "input": input, "settings": _IC_S, "expect_request": request, **extra})


_ic("list_users", {}, "GET", _B + "/users",
    http_reply={"status": 200, "body": '{"users":[{"username":"encoder1","usertype":"encoder","online":true}]}'},
    expect_result={"ok": {"kind": "value", "value": {"users": [{"username": "encoder1", "usertype": "encoder", "online": True}]}}})
_ic("list_users_page", {"limit": 50, "sort": "username", "direction": "desc", "filter": "enc"}, "GET",
    _B + "/users?limit=50&offset=0&sort=username&sortdir=desc&filter=enc")
_ic("get_user", {"user": "encoder1"}, "GET", _B + "/users/encoder1")
_ic("create_user", {"user": "encoder2", "password": "p4ss", "type": "encoder"}, "POST", _B + "/users",
    '{"username":"encoder2","password":"p4ss","usertype":"encoder"}',
    http_reply={"status": 201, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
_ic("delete_user", {"user": "encoder2", "cascade": True}, "DELETE", _B + "/users/encoder2?cascade=true")
_ic("list_access_codes", {}, "GET", _B + "/accesscodes")
_ic("list_access_codes_page", {"limit": 100}, "GET", _B + "/accesscodes?limit=100&offset=0&sort=online&sortdir=asc&filter=")
_ic("get_access_code", {"code": "1234"}, "GET", _B + "/accesscodes/1234")
_AC = {"code": "1234", "broadcaster": "acme", "encoder": "encoder1", "service": 1, "shared_with": ["captionco"],
       "secondary": [{"broadcaster": "acme", "encoder": "encoder2"}]}
_AC_BODY = ('{"accesscode":"1234","broadcaster":"acme","primaryencoder":"encoder1","service":1,'
            '"sharedwith":["captionco"],"secondaryencoder":[{"broadcaster":"acme","encoder":"encoder2"}]}')
_ic("create_access_code", _AC, "POST", _B + "/accesscodes", _AC_BODY)
_ic("update_access_code", _AC, "PUT", _B + "/accesscodes", _AC_BODY)
_ic("delete_access_code", {"code": "1234"}, "DELETE", _B + "/accesscodes/1234")
_ic("list_encoder_permissions", {}, "GET", _B + "/encoderpermissions")
_ic("get_encoder_permissions", {"encoder": "encoder1"}, "GET", _B + "/encoderpermissions/encoder1")
_ic("share_encoder", {"encoder": "encoder1", "agencies": ["captionco"]}, "POST", _B + "/encoderpermissions",
    '{"encoder":"encoder1","agencies":["captionco"]}')
_ic("unshare_encoder", {"encoder": "encoder1", "agency": "captionco"}, "DELETE", _B + "/encoderpermissions/encoder1/captionco")
_ic("get_encoder_activity", {"encoder": "encoder1"}, "GET", _B + "/encoderactivity/encoder1")
_ic("list_shared_encoders", {}, "GET", _B + "/sharedencoders")
_ic("list_captioncasts", {}, "GET", _B + "/captioncasts")
_ic("get_captioncast", {"event": "town-hall"}, "GET", _B + "/captioncasts/town-hall")
_ic("create_captioncast", {"event": "town-hall", "code": "1234", "banner": "Town hall meeting"}, "POST", _B + "/captioncasts",
    '{"eventcode":"town-hall","accesscode":"1234","banner":"Town hall meeting"}')
_ic("delete_captioncast", {"event": "town-hall"}, "DELETE", _B + "/captioncasts/town-hall")
_ic("get_logs", {"log": "eventlog", "by": "username", "value": "encoder1", "seconds": 86400, "captioning_only": True}, "GET",
    _B + "/logs/eventlog/username/encoder1/86400?captioningonly=1")
_ic("get_logs_page", {"log": "captionlog", "by": "username", "value": "encoder1", "seconds": 604800, "limit": 1000, "offset": 1000},
    "GET", _B + "/logs/captionlog/username/encoder1/604800?limit=1000&offset=1000&captioningonly=0")
_ic("list_active_encoders", {}, "GET", _B + "/active-encoders")
_ic("list_active_captioners", {}, "GET", _B + "/active-captioners")
_ic("get_sessions", {"user": "encoder1", "start": 1790000000, "end": 1790086400}, "GET",
    _B + "/sessions/encoder1/1790000000/1790086400")
_ic("get_uptime", {"user": "encoder1", "start": 1790000000, "end": 1792592000}, "GET",
    _B + "/uptime/encoder1/1790000000/1792592000")
_ic("list_providers", {}, "GET", "/api/1.0/providers")
_ic("list_relay_servers", {}, "GET", "/api/1.0/relaynames")
_ic("get_health", {}, "GET", "/api/1.0/health/overview")
_ic("list_email_subscriptions", {}, "GET", _B + "/emailnotices/subscriptions")
_ic("list_email_subscriptions_for", {"email": "ops@example.com"}, "GET", _B + "/emailnotices/subscriptions/ops%40example.com")
_ic("subscribe_email", {"email": "ops@example.com", "type": "AllEncoders"}, "POST", _B + "/emailnotices/subscriptions",
    '{"emailaddr":"ops@example.com","subscriptiontype":"AllEncoders"}')
_ic("subscribe_email_user", {"email": "ops@example.com", "user": "encoder1"}, "POST", _B + "/emailnotices/subscriptions",
    '{"emailaddr":"ops@example.com","subscriptiontype":"UserAccount","useraccount":"encoder1"}')
_ic("unsubscribe_email", {"email": "ops@example.com", "type": "DailyCaptionLog"}, "DELETE", _B + "/emailnotices/subscriptions",
    '{"emailaddr":"ops@example.com","subscriptiontype":"DailyCaptionLog"}')
_ic("unsubscribe_email_user", {"email": "ops@example.com", "user": "encoder1"}, "DELETE", _B + "/emailnotices/subscriptions",
    '{"emailaddr":"ops@example.com","subscriptiontype":"UserAccount","useraccount":"encoder1"}')
