# Planning Center Services LIVE (planningcenter-services): one vector per
# command, and telemetry vectors for the polled resources. Targets are the
# Services API v2 paths of Planning Center's documentation graph
# (api.planningcenteronline.com/services/v2/documentation/2018-11-01), with
# the LIVE actions under /service_types/{id}/plans/{id}/live/. Query values
# are percent-encoded by hand (a comma is %2C). Reply bodies follow the
# JSON:API shapes of the documentation's examples; ids are made up.
PC = "planningcenter-services"
_PCS = {"auth": "oauth2", "access_token": "pco_tok_x", "service_type_id": "1234", "plan_id": "5678"}
_PCP = "/services/v2/service_types/1234/plans/5678"
_INC = "?include=current_item_time%2Cnext_item_time"


def _pc(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": PC, "command": command, "input": input, "expect_request": request,
              "settings": _PCS, **extra})


_LIVE = {
    "data": {
        "type": "Live", "id": "5678",
        "attributes": {"can_chat": True, "can_control": True, "can_control_video_feed": False,
                       "can_take_control": True, "chat_room_channel": "chat-5678", "dates": "October 4, 2026",
                       "live_channel": "services-live-5678", "series_title": "Advent", "title": "Sunday Service"},
        "relationships": {
            "current_item_time": {"data": {"type": "ItemTime", "id": "901"}},
            "next_item_time": {"data": {"type": "ItemTime", "id": "902"}},
            "controller": {"data": {"type": "Person", "id": "42"}}}},
    "included": [
        {"type": "ItemTime", "id": "901",
         "attributes": {"exclude": False, "length": 300, "length_offset": 0,
                        "live_start_at": "2026-10-04T16:05:00Z", "live_end_at": None},
         "relationships": {"item": {"data": {"type": "Item", "id": "11"}},
                           "plan_time": {"data": {"type": "PlanTime", "id": "77"}},
                           "plan": {"data": {"type": "Plan", "id": "5678"}}}},
        {"type": "ItemTime", "id": "902",
         "attributes": {"exclude": False, "length": 240, "length_offset": 0,
                        "live_start_at": None, "live_end_at": None},
         "relationships": {"item": {"data": {"type": "Item", "id": "12"}},
                           "plan_time": {"data": {"type": "PlanTime", "id": "77"}},
                           "plan": {"data": {"type": "Plan", "id": "5678"}}}}]}
_LIVE_STATE = {"live": {
    "title": "Sunday Service", "series_title": "Advent", "dates": "October 4, 2026",
    "live_channel": "services-live-5678", "chat_room_channel": "chat-5678", "can_control": True,
    "can_take_control": True, "can_control_video_feed": False, "can_chat": True,
    "current_item_time_id": "901", "next_item_time_id": "902", "controller_id": "42"}}
_ITEM_TIMES = {
    "901": {"item_id": "11", "plan_time_id": "77", "live_start_at": "2026-10-04T16:05:00Z",
            "length": 300, "length_offset": 0, "exclude": False},
    "902": {"item_id": "12", "plan_time_id": "77",
            "length": 240, "length_offset": 0, "exclude": False}}

# LIVE actions: POST, answered with the Live resource.
_pc("go_to_next_item", {}, "POST", _PCP + "/live/go_to_next_item" + _INC,
    http_reply={"status": 200, "body": json.dumps(_LIVE)},
    expect_result={"ok": {"kind": "value", "value": _LIVE["data"]}})
# Someone else has control: Planning Center refuses the move (403), an
# ordinary failure, not a refused credential.
_pc("go_to_previous_item", {}, "POST", _PCP + "/live/go_to_previous_item" + _INC,
    http_reply={"status": 403, "body": '{"errors":[{"status":"403","title":"Forbidden"}]}'},
    expect_result={"error": {"error": "device_error", "code": "403"}})
_pc("toggle_control", {}, "POST", _PCP + "/live/toggle_control" + _INC)
# A plain OAuth access token (no refresh token) refused: 401 is terminal, reported as auth.
_pc("get_live", {}, "GET", _PCP + "/live" + _INC,
    http_reply={"status": 401, "body": '{"errors":[{"status":"401","title":"Unauthorized"}]}'},
    expect_result={"error": {"error": "auth"}})

# Reading.
_pc("get_organization", {}, "GET", "/services/v2",
    http_reply={"status": 200, "body": '{"data":{"type":"Organization","id":"1","attributes":{"name":"Grace Church"}}}'},
    expect_result={"ok": {"kind": "value", "value": {"type": "Organization", "id": "1",
                                                     "attributes": {"name": "Grace Church"}}}})
# The person the credential acts as: /me of the Services API (the
# documentation overview's Current User), a Person resource.
_ME = {"data": {"type": "Person", "id": "321",
                "attributes": {"first_name": "Ada", "last_name": "Lovelace", "full_name": "Ada Lovelace",
                               "photo_thumbnail_url": "https://example.com/ada.jpg",
                               "plan_permissions": "Editor", "song_permissions": "Viewer",
                               "media_permissions": "Viewer", "can_view_all_people": True,
                               "can_edit_all_people": False}}}
_pc("get_me", {}, "GET", "/services/v2/me",
    http_reply={"status": 200, "body": json.dumps(_ME)},
    expect_result={"ok": {"kind": "value", "value": _ME["data"]}})
_pc("list_service_types", {}, "GET", "/services/v2/service_types?per_page=100")
_pc("list_live_controllers", {"service_type_id": "1234"}, "GET",
    "/services/v2/service_types/1234/live_controllers?per_page=100")
_pc("list_plans", {"service_type_id": "1234"}, "GET",
    "/services/v2/service_types/1234/plans?filter=future&order=sort_date&per_page=25")
_pc("get_plan", {"service_type_id": "1234", "plan_id": "5679"}, "GET", "/services/v2/service_types/1234/plans/5679")
_pc("get_next_plan", {"service_type_id": "1234", "plan_id": "5678"}, "GET", _PCP + "/next_plan")
_pc("get_previous_plan", {"service_type_id": "1234", "plan_id": "5678"}, "GET", _PCP + "/previous_plan")
_pc("list_items", {"service_type_id": "1234", "plan_id": "5678"}, "GET", _PCP + "/items?per_page=100")
_pc("get_item", {"service_type_id": "1234", "plan_id": "5678", "item_id": "11"}, "GET", _PCP + "/items/11")
_pc("list_item_notes", {"service_type_id": "1234", "plan_id": "5678", "item_id": "11"}, "GET",
    _PCP + "/items/11/item_notes?per_page=100")
_pc("list_plan_times", {"service_type_id": "1234", "plan_id": "5678"}, "GET", _PCP + "/plan_times?per_page=100")

# Telemetry.
# Each Live reply replaces the plan's item times: one that was current before
# (900) leaves the state.
telemetry(PC, "live", settings=_PCS,
          inbound_http={"path": _PCP + "/live" + _INC, "body": json.dumps(_LIVE)},
          state_before={"plans": {"5678": {"item_times": {"900": {"item_id": "10", "length": 120}}}}},
          expect_state={"plans": {"5678": {**_LIVE_STATE, "item_times": _ITEM_TIMES}}})
telemetry(PC, "live-action", settings=_PCS,
          inbound_http={"path": _PCP + "/live/go_to_next_item" + _INC, "body": json.dumps(_LIVE)},
          expect_state={"plans": {"5678": {**_LIVE_STATE, "item_times": _ITEM_TIMES}}})
# LIVE not running: no item times included, and the last ones leave.
telemetry(PC, "live-idle", settings=_PCS,
          inbound_http={"path": _PCP + "/live" + _INC, "body": json.dumps({
              "data": {"type": "Live", "id": "5678", "attributes": {"title": "Sunday Service"},
                       "relationships": {}}, "included": []})},
          state_before={"plans": {"5678": {"item_times": dict(_ITEM_TIMES)}}},
          expect_state={"plans": {"5678": {"live": {"title": "Sunday Service"}}}})
telemetry(PC, "plan", settings=_PCS, inbound_http={"path": _PCP, "body": json.dumps({"data": {
    "type": "Plan", "id": "5678",
    "attributes": {"title": "Sunday Service", "series_title": "Advent", "dates": "October 4, 2026",
                   "short_dates": "Oct 4", "sort_date": "2026-10-04T16:00:00Z",
                   "last_time_at": "2026-10-04T18:00:00Z", "total_length": 4500, "items_count": 14,
                   "planning_center_url": "https://services.planningcenteronline.com/plans/5678"}}})},
    expect_state={"plans": {"5678": {
        "service_type_id": "1234", "title": "Sunday Service", "series_title": "Advent",
        "dates": "October 4, 2026", "short_dates": "Oct 4", "sort_date": "2026-10-04T16:00:00Z",
        "last_time_at": "2026-10-04T18:00:00Z", "total_length": 4500, "items_count": 14,
        "planning_center_url": "https://services.planningcenteronline.com/plans/5678"}}})
_ITEMS = [
    {"type": "Item", "id": "11", "attributes": {"title": "Welcome", "sequence": 1, "length": 300,
                                                "item_type": "item", "service_position": "during",
                                                "description": "Host on stage", "key_name": None}},
    {"type": "Item", "id": "12", "attributes": {"title": "Amazing Grace", "sequence": 2, "length": 240,
                                                "item_type": "song", "service_position": "during",
                                                "description": None, "key_name": "G"}}]
_ITEMS_STATE = {
    "11": {"title": "Welcome", "sequence": 1, "length": 300, "item_type": "item",
           "service_position": "during", "description": "Host on stage"},
    "12": {"title": "Amazing Grace", "sequence": 2, "length": 240, "item_type": "song",
           "service_position": "during", "key_name": "G"}}
_SELF = "https://api.planningcenteronline.com" + _PCP
# The whole list (top-level links without next): item 10, deleted from the
# plan, leaves the state, and the plan's other values stay.
telemetry(PC, "items", settings=_PCS, inbound_http={"path": _PCP + "/items?per_page=100", "body": json.dumps({
    "links": {"self": _SELF + "/items?per_page=100"}, "data": _ITEMS,
    "meta": {"total_count": 2, "count": 2}})},
    state_before={"plans": {"5678": {"title": "Sunday Service",
                                     "items": {"10": {"title": "Deleted", "sequence": 1}}}}},
    expect_state={"plans": {"5678": {"title": "Sunday Service", "items": _ITEMS_STATE}}})
# The plan has emptied: the list is complete with no item, and every item leaves.
telemetry(PC, "items-empty", settings=_PCS, inbound_http={"path": _PCP + "/items?per_page=100", "body": json.dumps({
    "links": {"self": _SELF + "/items?per_page=100"}, "data": [], "meta": {"total_count": 0, "count": 0}})},
    state_before={"plans": {"5678": {"title": "Sunday Service", "items": dict(_ITEMS_STATE)}}},
    expect_state={"plans": {"5678": {"title": "Sunday Service"}}})
# One page of several (a next link): its items are added, nothing is replaced.
telemetry(PC, "items-paged", settings=_PCS, inbound_http={"path": _PCP + "/items?per_page=100", "body": json.dumps({
    "links": {"self": _SELF + "/items?per_page=100", "next": _SELF + "/items?offset=100&per_page=100"},
    "data": _ITEMS, "meta": {"total_count": 102, "count": 100, "next": {"offset": 100}}})},
    state_before={"plans": {"5678": {"items": {"10": {"title": "Later page", "sequence": 101}}}}},
    expect_state={"plans": {"5678": {"items": {"10": {"title": "Later page", "sequence": 101},
                                               **_ITEMS_STATE}}}})
# One item read adds to the plan's items and removes none.
telemetry(PC, "item", settings=_PCS, inbound_http={"path": _PCP + "/items/12", "body": json.dumps({
    "data": _ITEMS[1]})},
    state_before={"plans": {"5678": {"items": {"11": {"title": "Welcome"}}}}},
    expect_state={"plans": {"5678": {"items": {"11": {"title": "Welcome"}, "12": _ITEMS_STATE["12"]}}}},
    # A read of one item, like an update's reply on the same path, reads the
    # plan's items again.
    expect_then_send=[{"method": "GET", "target": _PCP + "/items?per_page=100&include=song%2Carrangement%2Ckey"}])
telemetry(PC, "plan-times", settings=_PCS, inbound_http={"path": _PCP + "/plan_times?per_page=100", "body": json.dumps({
    "links": {"self": _SELF + "/plan_times?per_page=100"},
    "data": [
        {"type": "PlanTime", "id": "77", "attributes": {
            "name": "9:00", "time_type": "service", "starts_at": "2026-10-04T16:00:00Z",
            "ends_at": "2026-10-04T17:15:00Z", "live_starts_at": "2026-10-04T16:01:12Z", "live_ends_at": None}}]})},
    state_before={"plans": {"5678": {"plan_times": {"76": {"name": "Rehearsal", "time_type": "rehearsal"}}}}},
    expect_state={"plans": {"5678": {"plan_times": {"77": {
        "name": "9:00", "time_type": "service", "starts_at": "2026-10-04T16:00:00Z",
        "ends_at": "2026-10-04T17:15:00Z", "live_starts_at": "2026-10-04T16:01:12Z"}}}}})
# A list of plans adds to plans and replaces nothing: it is filtered and paged.
telemetry(PC, "plans", settings=_PCS, inbound_http={
    "path": "/services/v2/service_types/1234/plans?filter=future&order=sort_date&per_page=25",
    "body": json.dumps({"links": {"self": "https://api.planningcenteronline.com/services/v2/service_types/1234/plans"},
                        "data": [{"type": "Plan", "id": "5679", "attributes": {
                            "title": "Evening", "series_title": None, "dates": "October 4, 2026",
                            "short_dates": "Oct 4", "sort_date": "2026-10-04T23:00:00Z",
                            "last_time_at": "2026-10-05T00:00:00Z", "total_length": 3600, "items_count": 9,
                            "planning_center_url": "https://services.planningcenteronline.com/plans/5679"}}]})},
    state_before={"plans": {"5678": {"title": "Sunday Service"}}},
    expect_state={"plans": {"5678": {"title": "Sunday Service"}, "5679": {
        "service_type_id": "1234", "title": "Evening", "dates": "October 4, 2026",
        "short_dates": "Oct 4", "sort_date": "2026-10-04T23:00:00Z", "last_time_at": "2026-10-05T00:00:00Z",
        "total_length": 3600, "items_count": 9,
        "planning_center_url": "https://services.planningcenteronline.com/plans/5679"}}})
# The organization and the signed-in person (the account poll).
telemetry(PC, "organization", settings=_PCS, inbound_http={"path": "/services/v2", "body": json.dumps({
    "data": {"type": "Organization", "id": "42",
             "attributes": {"name": "Grace Church", "owner_name": "Pat Smith", "time_zone": "America/Chicago",
                            "twenty_four_hour_time": False, "date_format": 1, "ccli": "123456"}}})},
    expect_state={"organization": {"id": "42", "name": "Grace Church", "owner_name": "Pat Smith",
                                   "time_zone": "America/Chicago", "twenty_four_hour_time": False,
                                   "date_format": 1, "ccli": "123456"}})
telemetry(PC, "me", settings=_PCS, inbound_http={"path": "/services/v2/me", "body": json.dumps(_ME)},
    expect_state={"me": {"id": "321", "first_name": "Ada", "last_name": "Lovelace", "full_name": "Ada Lovelace",
                         "photo_thumbnail_url": "https://example.com/ada.jpg", "plan_permissions": "Editor",
                         "song_permissions": "Viewer", "media_permissions": "Viewer",
                         "can_view_all_people": True, "can_edit_all_people": False}})
# Opened before a plan is chosen: the account's telemetry alone.
telemetry(PC, "service-types-unchosen", settings={"auth": "oauth2", "access_token": "pco_tok_x"}, inbound_http={
    "path": "/services/v2/service_types?per_page=100",
    "body": '{"links":{},"data":[{"type":"ServiceType","id":"1234","attributes":{"name":"Sunday Morning"}}]}'},
    expect_state={"service_types": {"1234": {"name": "Sunday Morning"}}})
# The whole list of service types replaces the last: 999 leaves.
telemetry(PC, "service-types", settings=_PCS, inbound_http={
    "path": "/services/v2/service_types?per_page=100",
    "body": '{"links":{"self":"https://api.planningcenteronline.com/services/v2/service_types?per_page=100"},'
            '"data":[{"type":"ServiceType","id":"1234","attributes":{"name":"Sunday Morning"}}]}'},
    state_before={"service_types": {"999": {"name": "Retired"}}},
    expect_state={"service_types": {"1234": {"name": "Sunday Morning"}}})

# Editing (JSON:API bodies, as the documentation graph's writable attributes).
_ST = {"service_type_id": "1234"}
_SP = {"service_type_id": "1234", "plan_id": "5678"}
_SPI = {**_SP, "item_id": "11"}
_ITEMS_RELOAD = {"method": "GET", "target": _PCP + "/items?per_page=100&include=song%2Carrangement%2Ckey"}
_pc("create_plan", {**_ST, "attributes": {"title": "Christmas Eve"}}, "POST", "/services/v2/service_types/1234/plans",
    body='{"data":{"type":"Plan","attributes":{"title":"Christmas Eve"}}}')
_pc("update_plan", {**_SP, "attributes": {"public": True}}, "PATCH", _PCP,
    body='{"data":{"type":"Plan","attributes":{"public":true}}}')
_pc("set_plan_title", {**_SP, "title": "Sunday \"Service\""}, "PATCH", _PCP,
    body='{"data":{"type":"Plan","attributes":{"title":"Sunday \\"Service\\""}}}')
_pc("delete_plan", _SP, "DELETE", _PCP, http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_pc("create_item", {**_SP, "attributes": {"title": "Offering", "length": 180}}, "POST", _PCP + "/items",
    body='{"data":{"type":"Item","attributes":{"title":"Offering","length":180}}}')
_pc("create_header", {**_SP, "title": "Worship"}, "POST", _PCP + "/items",
    body='{"data":{"type":"Item","attributes":{"title":"Worship","item_type":"header"}}}')
_pc("update_item", {**_SPI, "attributes": {"song_id": "301", "arrangement_id": "401"}}, "PATCH", _PCP + "/items/11",
    body='{"data":{"type":"Item","attributes":{"song_id":"301","arrangement_id":"401"}}}')
_pc("set_item_title", {**_SPI, "title": "Welcome"}, "PATCH", _PCP + "/items/11",
    body='{"data":{"type":"Item","attributes":{"title":"Welcome"}}}')
_pc("set_item_length", {**_SPI, "length": 240}, "PATCH", _PCP + "/items/11",
    body='{"data":{"type":"Item","attributes":{"length":240}}}')
_pc("set_item_description", {**_SPI, "description": "Host on stage"}, "PATCH", _PCP + "/items/11",
    body='{"data":{"type":"Item","attributes":{"description":"Host on stage"}}}')
_pc("delete_item", _SPI, "DELETE", _PCP + "/items/11")
_pc("reorder_items", {**_SP, "sequence": ["12", "11"]}, "POST", _PCP + "/item_reorder",
    body='{"data":{"type":"PlanItemReorder","attributes":{"sequence":["12","11"]}}}',
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_pc("list_item_note_categories", _ST, "GET", "/services/v2/service_types/1234/item_note_categories?per_page=100")
_pc("create_item_note", {**_SPI, "category_id": "5", "content": "Spot on the pulpit"}, "POST",
    _PCP + "/items/11/item_notes",
    body='{"data":{"type":"ItemNote","attributes":{"content":"Spot on the pulpit"},'
         '"relationships":{"item_note_category":{"data":{"type":"ItemNoteCategory","id":"5"}}}}}')
_pc("update_item_note", {**_SPI, "note_id": "81", "content": "Spot and haze"}, "PATCH", _PCP + "/items/11/item_notes/81",
    body='{"data":{"type":"ItemNote","attributes":{"content":"Spot and haze"}}}')
_pc("delete_item_note", {**_SPI, "note_id": "81"}, "DELETE", _PCP + "/items/11/item_notes/81")
_pc("list_plan_note_categories", _ST, "GET", "/services/v2/service_types/1234/plan_note_categories?per_page=100")
_pc("list_plan_notes", _SP, "GET", _PCP + "/notes?per_page=100")
_pc("create_plan_note", {**_SP, "category_id": "6", "content": "Doors at 8:30"}, "POST", _PCP + "/notes",
    body='{"data":{"type":"PlanNote","attributes":{"content":"Doors at 8:30"},'
         '"relationships":{"plan_note_category":{"data":{"type":"PlanNoteCategory","id":"6"}}}}}')
_pc("update_plan_note", {**_SP, "note_id": "91", "content": "Doors at 8:45"}, "PATCH", _PCP + "/notes/91",
    body='{"data":{"type":"PlanNote","attributes":{"content":"Doors at 8:45"}}}')
_pc("delete_plan_note", {**_SP, "note_id": "91"}, "DELETE", _PCP + "/notes/91")
_pc("create_plan_time", {**_SP, "name": "11:00", "starts_at": "2026-10-18T11:00:00-05:00",
                         "ends_at": "2026-10-18T12:15:00-05:00"}, "POST", _PCP + "/plan_times",
    body='{"data":{"type":"PlanTime","attributes":{"name":"11:00","time_type":"service",'
         '"starts_at":"2026-10-18T11:00:00-05:00","ends_at":"2026-10-18T12:15:00-05:00"}}}')
_pc("update_plan_time", {**_SP, "plan_time_id": "77", "attributes": {"name": "9:30"}}, "PATCH", _PCP + "/plan_times/77",
    body='{"data":{"type":"PlanTime","attributes":{"name":"9:30"}}}')
_pc("delete_plan_time", {**_SP, "plan_time_id": "77"}, "DELETE", _PCP + "/plan_times/77")

# Songs, arrangements, media and attachments.
_pc("list_songs", {}, "GET", "/services/v2/songs?order=title&per_page=25&offset=0")
_pc("search_songs", {"title": "Amazing Grace"}, "GET", "/services/v2/songs?where%5Btitle%5D=Amazing%20Grace&per_page=100")
_pc("get_song", {"song_id": "301"}, "GET", "/services/v2/songs/301")
_pc("list_arrangements", {"song_id": "301"}, "GET", "/services/v2/songs/301/arrangements?per_page=100")
_pc("get_arrangement", {"song_id": "301", "arrangement_id": "401"}, "GET", "/services/v2/songs/301/arrangements/401?include=keys")
_pc("list_arrangement_keys", {"song_id": "301", "arrangement_id": "401"}, "GET",
    "/services/v2/songs/301/arrangements/401/keys?per_page=100")
_pc("list_arrangement_sections", {"song_id": "301", "arrangement_id": "401"}, "GET",
    "/services/v2/songs/301/arrangements/401/sections")
_pc("list_media", {"order": "-created_at"}, "GET", "/services/v2/media?order=-created_at&per_page=25&offset=0")
_pc("get_media", {"media_id": "601"}, "GET", "/services/v2/media/601")
_pc("list_media_attachments", {"media_id": "601"}, "GET", "/services/v2/media/601/attachments?per_page=100")
_pc("get_item_media", _SPI, "GET", _PCP + "/items/11/media")
_pc("list_item_attachments", _SPI, "GET", _PCP + "/items/11/attachments?per_page=100")
_pc("list_plan_attachments", _SP, "GET", _PCP + "/all_attachments?per_page=100")
_pc("list_song_attachments", {"song_id": "301"}, "GET", "/services/v2/songs/301/attachments?per_page=100")
_OPENED = {"data": {"type": "AttachmentActivity", "id": "1",
                    "attributes": {"attachment_url": "https://services.planningcenteronline.com/files/abc"}}}
_pc("open_media_attachment", {"media_id": "601", "attachment_id": "701"}, "POST",
    "/services/v2/media/601/attachments/701/open",
    http_reply={"status": 200, "body": json.dumps(_OPENED)},
    expect_result={"ok": {"kind": "value", "value": "https://services.planningcenteronline.com/files/abc"}})
_pc("open_item_attachment", {**_SPI, "attachment_id": "701"}, "POST", _PCP + "/items/11/attachments/701/open")

# Telemetry: the poll's items with their songs, arrangements and keys.
_SONG_ITEMS = [{**_ITEMS[1], "attributes": {**_ITEMS[1]["attributes"], "custom_arrangement_sequence": []},
                "relationships": {"song": {"data": {"type": "Song", "id": "301"}},
                                  "arrangement": {"data": {"type": "Arrangement", "id": "401"}},
                                  "key": {"data": {"type": "Key", "id": "501"}}}}]
_INCLUDED = [
    {"type": "Song", "id": "301", "attributes": {"title": "Amazing Grace", "author": "John Newton",
                                                 "ccli_number": 22025, "copyright": "Public Domain"}},
    {"type": "Arrangement", "id": "401", "attributes": {
        "name": "Default", "bpm": 72.0, "meter": "3/4", "length": 240,
        "sequence": ["Verse 1", "Verse 2", "Verse 3"], "chord_chart_key": "G", "has_chords": True,
        "lyrics": "Amazing grace, how sweet the sound"}},
    {"type": "Key", "id": "501", "attributes": {"name": "Default", "starting_key": "G", "ending_key": "G"}}]
telemetry(PC, "items-with-songs", settings=_PCS, inbound_http={
    "path": _PCP + "/items?per_page=100&include=song%2Carrangement%2Ckey", "body": json.dumps({
        "links": {"self": _SELF + "/items?per_page=100"}, "data": _SONG_ITEMS, "included": _INCLUDED,
        "meta": {"total_count": 1, "count": 1}})},
    state_before={"plans": {"5678": {"songs": {"300": {"title": "No longer in the plan"}}}}},
    expect_state={"plans": {"5678": {
        "items": {"12": {**_ITEMS_STATE["12"], "song_id": "301", "arrangement_id": "401", "key_id": "501",
                         "custom_arrangement_sequence": "[]"}},
        "songs": {"301": {"title": "Amazing Grace", "author": "John Newton", "ccli_number": 22025,
                          "copyright": "Public Domain"}},
        "arrangements": {"401": {"name": "Default", "bpm": 72.0, "meter": "3/4", "length": 240,
                                 "sequence": '["Verse 1","Verse 2","Verse 3"]', "chord_chart_key": "G",
                                 "has_chords": True, "lyrics": "Amazing grace, how sweet the sound"}},
        "keys": {"501": {"name": "Default", "starting_key": "G", "ending_key": "G"}}}}})
telemetry(PC, "item-created", settings=_PCS, inbound_http={"path": _PCP + "/items", "body": json.dumps({
    "data": {"type": "Item", "id": "13", "attributes": {"title": "Offering", "sequence": 3, "length": 180,
                                                         "item_type": "item", "service_position": "during"}}})},
    state_before={"plans": {"5678": {"items": {"11": {"title": "Welcome"}}}}},
    expect_state={"plans": {"5678": {"items": {"11": {"title": "Welcome"}, "13": {
        "title": "Offering", "sequence": 3, "length": 180, "item_type": "item", "service_position": "during"}}}}},
    expect_then_send=[_ITEMS_RELOAD])
telemetry(PC, "items-reordered", settings=_PCS, inbound_http={"path": _PCP + "/item_reorder", "body": ""},
          expect_state={}, expect_then_send=[_ITEMS_RELOAD])
telemetry(PC, "plan-time-created", settings=_PCS, inbound_http={"path": _PCP + "/plan_times", "body": json.dumps({
    "data": {"type": "PlanTime", "id": "78", "attributes": {
        "name": "11:00", "time_type": "service", "starts_at": "2026-10-18T16:00:00Z", "ends_at": "2026-10-18T17:15:00Z",
        "live_starts_at": None, "live_ends_at": None}}})},
    expect_state={"plans": {"5678": {"plan_times": {"78": {
        "name": "11:00", "time_type": "service", "starts_at": "2026-10-18T16:00:00Z", "ends_at": "2026-10-18T17:15:00Z"}}}}})
telemetry(PC, "plan-time-deleted", settings=_PCS, inbound_http={"path": _PCP + "/plan_times/77", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _PCP + "/plan_times?per_page=100"}])
telemetry(PC, "plan-created", settings=_PCS, inbound_http={
    "path": "/services/v2/service_types/1234/plans", "body": json.dumps({"data": {
        "type": "Plan", "id": "5690", "attributes": {"title": "Christmas Eve", "series_title": None,
                                                     "dates": "No dates",
                                                     "planning_center_url": "https://services.planningcenteronline.com/plans/5690"}}})},
    expect_state={"plans": {"5690": {"service_type_id": "1234", "title": "Christmas Eve", "dates": "No dates",
                                     "planning_center_url": "https://services.planningcenteronline.com/plans/5690"}}})
_NOTE = {"type": "ItemNote", "id": "81", "attributes": {"category_name": "Lighting", "content": "Spot on the pulpit"}}
telemetry(PC, "item-notes", settings=_PCS, inbound_http={
    "path": _PCP + "/items/11/item_notes?per_page=100",
    "body": json.dumps({"links": {"self": _SELF + "/items/11/item_notes?per_page=100"}, "data": [_NOTE]})},
    state_before={"plans": {"5678": {"item_notes": {"11": {"80": {"content": "Deleted"}}, "12": {"85": {"content": "kept"}}}}}},
    expect_state={"plans": {"5678": {"item_notes": {"11": {"81": {"category": "Lighting", "content": "Spot on the pulpit"}},
                                                    "12": {"85": {"content": "kept"}}}}}})
telemetry(PC, "item-note-created", settings=_PCS, inbound_http={"path": _PCP + "/items/11/item_notes",
                                                                  "body": json.dumps({"data": _NOTE})},
          expect_state={"plans": {"5678": {"item_notes": {"11": {"81": {"category": "Lighting",
                                                                         "content": "Spot on the pulpit"}}}}}})
telemetry(PC, "item-note-deleted", settings=_PCS, inbound_http={"path": _PCP + "/items/11/item_notes/81", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _PCP + "/items/11/item_notes?per_page=100"}])
_PNOTE = {"type": "PlanNote", "id": "91", "attributes": {"category_name": "Production", "content": "Doors at 8:30"}}
telemetry(PC, "plan-notes", settings=_PCS, inbound_http={
    "path": _PCP + "/notes?per_page=100", "body": json.dumps({"data": [_PNOTE]})},
    state_before={"plans": {"5678": {"notes": {"90": {"content": "Deleted"}}}}},
    expect_state={"plans": {"5678": {"notes": {"91": {"category": "Production", "content": "Doors at 8:30"}}}}})
telemetry(PC, "plan-note-updated", settings=_PCS, inbound_http={"path": _PCP + "/notes/91", "body": json.dumps({
    "data": {**_PNOTE, "attributes": {"category_name": "Production", "content": "Doors at 8:45"}}})},
    expect_state={"plans": {"5678": {"notes": {"91": {"category": "Production", "content": "Doors at 8:45"}}}}},
    expect_then_send=[{"method": "GET", "target": _PCP + "/notes?per_page=100"}])
