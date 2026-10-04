# Planning Center Services LIVE (planningcenter-services): one vector per
# command, and telemetry vectors for the polled resources. Targets are the
# Services API v2 paths of Planning Center's documentation graph
# (api.planningcenteronline.com/services/v2/documentation/2018-11-01), with
# the LIVE actions under /service_types/{id}/plans/{id}/live/. Query values
# are percent-encoded by hand (a comma is %2C). Reply bodies follow the
# JSON:API shapes of the documentation's examples; ids are made up.
PC = "planningcenter-services"
_PCS = {"auth": "bearer", "token": "pco_tok_x", "service_type_id": "1234", "plan_id": "5678"}
_PCP = "/services/v2/service_types/1234/plans/5678"
_INC = "?include=current_item_time%2Cnext_item_time"


def _pc(command, input, method, target, **extra):
    http(PC, command, input, method, target, settings=_PCS, **extra)


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
# An expired OAuth token: 401 is terminal, reported as auth.
_pc("get_live", {}, "GET", _PCP + "/live" + _INC,
    http_reply={"status": 401, "body": '{"errors":[{"status":"401","title":"Unauthorized"}]}'},
    expect_result={"error": {"error": "auth"}})

# Reading.
_pc("get_organization", {}, "GET", "/services/v2",
    http_reply={"status": 200, "body": '{"data":{"type":"Organization","id":"1","attributes":{"name":"Grace Church"}}}'},
    expect_result={"ok": {"kind": "value", "value": {"type": "Organization", "id": "1",
                                                     "attributes": {"name": "Grace Church"}}}})
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
telemetry(PC, "live", settings=_PCS,
          inbound_http={"path": _PCP + "/live" + _INC, "body": json.dumps(_LIVE)},
          expect_state={"plans": {"5678": _LIVE_STATE}, "item_times": _ITEM_TIMES})
telemetry(PC, "live-action", settings=_PCS,
          inbound_http={"path": _PCP + "/live/go_to_next_item" + _INC, "body": json.dumps(_LIVE)},
          expect_state={"plans": {"5678": _LIVE_STATE}, "item_times": _ITEM_TIMES})
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
telemetry(PC, "items", settings=_PCS, inbound_http={"path": _PCP + "/items?per_page=100", "body": json.dumps({"data": [
    {"type": "Item", "id": "11", "attributes": {"title": "Welcome", "sequence": 1, "length": 300,
                                                "item_type": "item", "service_position": "during",
                                                "description": "Host on stage", "key_name": None}},
    {"type": "Item", "id": "12", "attributes": {"title": "Amazing Grace", "sequence": 2, "length": 240,
                                                "item_type": "song", "service_position": "during",
                                                "description": None, "key_name": "G"}}]})},
    expect_state={"items": {
        "11": {"plan_id": "5678", "title": "Welcome", "sequence": 1, "length": 300, "item_type": "item",
               "service_position": "during", "description": "Host on stage"},
        "12": {"plan_id": "5678", "title": "Amazing Grace", "sequence": 2, "length": 240, "item_type": "song",
               "service_position": "during", "key_name": "G"}}})
telemetry(PC, "plan-times", settings=_PCS, inbound_http={"path": _PCP + "/plan_times?per_page=100", "body": json.dumps({"data": [
    {"type": "PlanTime", "id": "77", "attributes": {
        "name": "9:00", "time_type": "service", "starts_at": "2026-10-04T16:00:00Z",
        "ends_at": "2026-10-04T17:15:00Z", "live_starts_at": "2026-10-04T16:01:12Z", "live_ends_at": None}}]})},
    expect_state={"plan_times": {"77": {
        "plan_id": "5678", "name": "9:00", "time_type": "service", "starts_at": "2026-10-04T16:00:00Z",
        "ends_at": "2026-10-04T17:15:00Z", "live_starts_at": "2026-10-04T16:01:12Z"}}})
telemetry(PC, "plans", settings=_PCS, inbound_http={
    "path": "/services/v2/service_types/1234/plans?filter=future&order=sort_date&per_page=25",
    "body": json.dumps({"data": [{"type": "Plan", "id": "5679", "attributes": {
        "title": "Evening", "series_title": None, "dates": "October 4, 2026", "short_dates": "Oct 4",
        "sort_date": "2026-10-04T23:00:00Z", "last_time_at": "2026-10-05T00:00:00Z", "total_length": 3600,
        "items_count": 9, "planning_center_url": "https://services.planningcenteronline.com/plans/5679"}}]})},
    expect_state={"plans": {"5679": {
        "service_type_id": "1234", "title": "Evening", "dates": "October 4, 2026",
        "short_dates": "Oct 4", "sort_date": "2026-10-04T23:00:00Z", "last_time_at": "2026-10-05T00:00:00Z",
        "total_length": 3600, "items_count": 9,
        "planning_center_url": "https://services.planningcenteronline.com/plans/5679"}}})
telemetry(PC, "service-types", settings=_PCS, inbound_http={
    "path": "/services/v2/service_types?per_page=100",
    "body": '{"data":[{"type":"ServiceType","id":"1234","attributes":{"name":"Sunday Morning"}}]}'},
    expect_state={"service_types": {"1234": {"name": "Sunday Morning"}}})
