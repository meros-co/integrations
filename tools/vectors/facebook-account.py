# Facebook account (facebook-account): one vector per command, and telemetry
# vectors for the user's id and name and for a Page list, which must leave
# state untouched. Targets are the Graph API paths as Meta's Graph API
# reference gives them, on the default version v26.0, with query values
# percent-encoded by urllib (RFC 3986 unreserved characters kept). Reply
# bodies follow the reference's shapes; ids, cursors and tokens are made up.
from urllib.parse import quote as _q

FA = "facebook-account"
_FAS = {"token": "EAAGusertoken"}
_FAV = "/v26.0"
_PAGES_T = "fields=" + _q("id,name,category,tasks,access_token", safe="")
_PAGES_N = "fields=" + _q("id,name,category,tasks", safe="")
_CURSOR = "QVFIUmxtcWZAhNGZAGUjZAQ"
_PAGE = {"id": "1093482904102", "name": "Grace Church", "category": "Church",
         "tasks": ["ANALYZE", "ADVERTISE", "MODERATE", "CREATE_CONTENT", "MANAGE"],
         "access_token": "EAAGpagetoken"}
_LIST = {"data": [_PAGE], "paging": {"cursors": {"before": "QVFIUjBef", "after": _CURSOR}}}


def _fa(command, input, target, **extra):
    V.append({"spec": FA, "command": command, "input": input, "settings": _FAS,
              "expect_request": {"method": "GET", "target": target}, **extra})


_fa("get_me", {}, _FAV + "/me?fields=" + _q("id,name", safe=""),
    http_reply={"status": 200, "body": json.dumps({"id": "10229384756", "name": "Sam Operator"})},
    expect_result={"ok": {"kind": "value", "value": {"id": "10229384756", "name": "Sam Operator"}}})
_fa("get_permissions", {}, _FAV + "/me/permissions",
    http_reply={"status": 200, "body": json.dumps({"data": [
        {"permission": "pages_show_list", "status": "granted"},
        {"permission": "pages_manage_posts", "status": "declined"}]})},
    expect_result={"ok": {"kind": "value", "value": [
        {"permission": "pages_show_list", "status": "granted"},
        {"permission": "pages_manage_posts", "status": "declined"}]}})
_fa("list_pages", {}, _FAV + "/me/accounts?" + _PAGES_T + "&limit=25",
    http_reply={"status": 200, "body": json.dumps(_LIST)},
    expect_result={"ok": {"kind": "value", "value": _LIST}})
_fa("list_pages_after", {"after": _CURSOR, "limit": 50},
    _FAV + "/me/accounts?" + _PAGES_T + "&limit=50&after=" + _CURSOR)
_fa("list_page_names", {"limit": 100}, _FAV + "/me/accounts?" + _PAGES_N + "&limit=100")
_fa("get_page_token", {"page_id": "1093482904102"},
    _FAV + "/1093482904102?fields=" + _q("id,name,access_token", safe=""),
    http_reply={"status": 200, "body": json.dumps({"id": "1093482904102", "name": "Grace Church",
                                                  "access_token": "EAAGpagetoken"})},
    expect_result={"ok": {"kind": "value", "value": {"id": "1093482904102", "name": "Grace Church",
                                                     "access_token": "EAAGpagetoken"}}})
# An expired user token: code 190 under HTTP 400 is the terminal refusal.
_fa("list_page_names_after", {"after": _CURSOR},
    _FAV + "/me/accounts?" + _PAGES_N + "&limit=25&after=" + _CURSOR,
    http_reply={"status": 400, "body": json.dumps({"error": {
        "message": "Error validating access token: Session has expired.", "type": "OAuthException",
        "code": 190, "error_subcode": 463, "fbtrace_id": "AbCdEf"}})},
    expect_result={"error": {"error": "auth"}})

# Telemetry: the probe's answer gives the user; a Page list gives nothing.
telemetry(FA, "me", settings=_FAS, inbound_http={
    "path": _FAV + "/me?fields=" + _q("id,name", safe=""),
    "body": json.dumps({"id": "10229384756", "name": "Sam Operator"})},
    expect_state={"user": {"id": "10229384756", "name": "Sam Operator"}})
telemetry(FA, "pages-not-in-state", settings=_FAS, inbound_http={
    "path": _FAV + "/me/accounts?" + _PAGES_T + "&limit=25", "body": json.dumps(_LIST)},
    expect_state={})
telemetry(FA, "page-token-not-in-state", settings=_FAS, inbound_http={
    "path": _FAV + "/1093482904102?fields=" + _q("id,name,access_token", safe=""),
    "body": json.dumps({"id": "1093482904102", "name": "Grace Church", "access_token": "EAAGpagetoken"})},
    expect_state={})
