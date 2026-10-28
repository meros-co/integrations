# Sofie Core User Actions API (sofie-core): one request per operation of the project's OpenAPI document,
# the path under /api/v1.0 and the JSON body as the document gives them.
SOF = "sofie-core"

V.append({"spec": SOF, "command": "get_version", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/"}})
V.append({"spec": SOF, "command": "get_health", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/health"}})
V.append({"spec": SOF, "command": "assign_system_blueprint", "input": {"blueprint_id": "blueprint_id0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/system/blueprint", "body": "{\"blueprintId\":\"blueprint_id0\"}"}})
V.append({"spec": SOF, "command": "unassign_system_blueprint", "input": {}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/system/blueprint"}})
V.append({"spec": SOF, "command": "get_pending_migrations", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/system/migrations"}})
V.append({"spec": SOF, "command": "apply_pending_migrations", "input": {"body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/system/migrations", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_devices", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/devices"}})
V.append({"spec": SOF, "command": "get_device", "input": {"device": "device0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/devices/device0"}})
V.append({"spec": SOF, "command": "device_action", "input": {"device": "device0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/devices/device0/action", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_blueprints", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/blueprints"}})
V.append({"spec": SOF, "command": "get_blueprint", "input": {"blueprint": "blueprint0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/blueprints/blueprint0"}})
V.append({"spec": SOF, "command": "get_playlists", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/playlists"}})
V.append({"spec": SOF, "command": "activate_playlist", "input": {"playlist": "playlist0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/activate", "body": "{\"rehearsal\":false}"}})
V.append({"spec": SOF, "command": "deactivate_playlist", "input": {"playlist": "playlist0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/deactivate"}})
V.append({"spec": SOF, "command": "activate_adlib_testing", "input": {"playlist": "playlist0", "rundown": "rundown0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/rundowns/rundown0/activate-adlib-testing"}})
V.append({"spec": SOF, "command": "execute_adlib", "input": {"playlist": "playlist0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/execute-adlib", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "execute_bucket_adlib", "input": {"playlist": "playlist0", "bucket_id": "bucket_id0", "external_id": "external_id0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/execute-bucket-adlib", "body": "{\"bucketId\":\"bucket_id0\",\"externalId\":\"external_id0\"}"}})
V.append({"spec": SOF, "command": "execute_bucket_adlib_with_action_type", "input": {"playlist": "playlist0", "bucket_id": "bucket_id0", "external_id": "external_id0", "action_type": "action_type0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/execute-bucket-adlib", "body": "{\"bucketId\":\"bucket_id0\",\"externalId\":\"external_id0\",\"actionType\":\"action_type0\"}"}})
V.append({"spec": SOF, "command": "move_next_part", "input": {"playlist": "playlist0", "delta": 1}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/move-next-part", "body": "{\"delta\":1}"}})
V.append({"spec": SOF, "command": "move_next_segment", "input": {"playlist": "playlist0", "delta": 1}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/move-next-segment", "body": "{\"delta\":1,\"ignoreQuickLoop\":false}"}})
V.append({"spec": SOF, "command": "reload_playlist", "input": {"playlist": "playlist0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/reload-playlist"}})
V.append({"spec": SOF, "command": "reset_playlist", "input": {"playlist": "playlist0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/reset-playlist"}})
V.append({"spec": SOF, "command": "set_next_part", "input": {"playlist": "playlist0", "part_id": "part_id0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/set-next-part", "body": "{\"partId\":\"part_id0\"}"}})
V.append({"spec": SOF, "command": "set_next_segment", "input": {"playlist": "playlist0", "segment_id": "segment_id0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/set-next-segment", "body": "{\"segmentId\":\"segment_id0\"}"}})
V.append({"spec": SOF, "command": "queue_next_segment", "input": {"playlist": "playlist0", "segment_id": "segment_id0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/queue-next-segment", "body": "{\"segmentId\":\"segment_id0\"}"}})
V.append({"spec": SOF, "command": "clear_source_layers", "input": {"playlist": "playlist0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/playlists/playlist0/clear-sourcelayers", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "take", "input": {"playlist": "playlist0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/take", "body": "{}"}})
V.append({"spec": SOF, "command": "take_with_from_part_instance_id", "input": {"playlist": "playlist0", "from_part_instance_id": "from_part_instance_id0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/take", "body": "{\"fromPartInstanceId\":\"from_part_instance_id0\"}"}})
V.append({"spec": SOF, "command": "clear_source_layer", "input": {"playlist": "playlist0", "source_layer": "source_layer0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/playlists/playlist0/sourceLayer/source_layer0"}})
V.append({"spec": SOF, "command": "recall_sticky", "input": {"playlist": "playlist0", "source_layer": "source_layer0"}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/sourceLayer/source_layer0/sticky"}})
V.append({"spec": SOF, "command": "start_timer_countdown", "input": {"playlist": "playlist0", "timer": 2, "duration": 1}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/countdown", "body": "{\"duration\":1,\"stopAtZero\":false,\"startPaused\":false}"}})
V.append({"spec": SOF, "command": "start_timer_free_run", "input": {"playlist": "playlist0", "timer": 2}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/free-run", "body": "{\"startPaused\":false}"}})
V.append({"spec": SOF, "command": "pause_timer", "input": {"playlist": "playlist0", "timer": 2}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/pause"}})
V.append({"spec": SOF, "command": "resume_timer", "input": {"playlist": "playlist0", "timer": 2}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/resume"}})
V.append({"spec": SOF, "command": "restart_timer", "input": {"playlist": "playlist0", "timer": 2}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/restart"}})
V.append({"spec": SOF, "command": "clear_timer_projection", "input": {"playlist": "playlist0", "timer": 2}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/projected/clear"}})
V.append({"spec": SOF, "command": "set_timer_projection_anchor", "input": {"playlist": "playlist0", "timer": 2, "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/projected/anchor-part", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "set_timer_projection_time", "input": {"playlist": "playlist0", "timer": 2, "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/projected/time", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "set_timer_projection_duration", "input": {"playlist": "playlist0", "timer": 2, "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/playlists/playlist0/t-timers/2/projected/duration", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_studios", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/studios"}})
V.append({"spec": SOF, "command": "add_studio", "input": {"body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/studios", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_studio", "input": {"studio": "studio0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/studios/studio0"}})
V.append({"spec": SOF, "command": "put_studio", "input": {"studio": "studio0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/studios/studio0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "delete_studio", "input": {"studio": "studio0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/studios/studio0"}})
V.append({"spec": SOF, "command": "get_studio_config", "input": {"studio": "studio0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/studios/studio0/config"}})
V.append({"spec": SOF, "command": "update_studio_config", "input": {"studio": "studio0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/studios/studio0/config", "body": "{\"developerMode\":false}"}})
V.append({"spec": SOF, "command": "switch_route_set", "input": {"studio": "studio0", "route_set_id": "route_set_id0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/studios/studio0/switch-route-set", "body": "{\"routeSetId\":\"route_set_id0\",\"active\":false}"}})
V.append({"spec": SOF, "command": "get_studio_devices", "input": {"studio": "studio0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/studios/studio0/devices"}})
V.append({"spec": SOF, "command": "attach_device", "input": {"studio": "studio0", "device_id": "device_id0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/studios/studio0/devices", "body": "{\"deviceId\":\"device_id0\"}"}})
V.append({"spec": SOF, "command": "attach_device_with_config_id", "input": {"studio": "studio0", "device_id": "device_id0", "config_id": "config_id0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/studios/studio0/devices", "body": "{\"deviceId\":\"device_id0\",\"configId\":\"config_id0\"}"}})
V.append({"spec": SOF, "command": "detach_device", "input": {"studio": "studio0", "device": "device0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/studios/studio0/devices/device0"}})
V.append({"spec": SOF, "command": "studio_action", "input": {"studio": "studio0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/studios/studio0/action", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_show_styles", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/showstyles"}})
V.append({"spec": SOF, "command": "add_show_style", "input": {"body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/showstyles", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_show_style", "input": {"show_style": "show_style0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/showstyles/show_style0"}})
V.append({"spec": SOF, "command": "put_show_style", "input": {"show_style": "show_style0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/showstyles/show_style0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "delete_show_style", "input": {"show_style": "show_style0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/showstyles/show_style0"}})
V.append({"spec": SOF, "command": "get_show_style_config", "input": {"show_style": "show_style0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/showstyles/show_style0/config"}})
V.append({"spec": SOF, "command": "update_show_style_config", "input": {"show_style": "show_style0"}, "expect_request": {"method": "PUT", "target": "/api/v1.0/showstyles/show_style0/config", "body": "{\"developerMode\":false}"}})
V.append({"spec": SOF, "command": "get_show_style_variants", "input": {"show_style": "show_style0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/showstyles/show_style0/variants"}})
V.append({"spec": SOF, "command": "add_show_style_variant", "input": {"show_style": "show_style0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/showstyles/show_style0/variants", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_show_style_variant", "input": {"show_style": "show_style0", "variant": "variant0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/showstyles/show_style0/variants/variant0"}})
V.append({"spec": SOF, "command": "put_show_style_variant", "input": {"show_style": "show_style0", "variant": "variant0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/showstyles/show_style0/variants/variant0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "delete_show_style_variant", "input": {"show_style": "show_style0", "variant": "variant0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/showstyles/show_style0/variants/variant0"}})
V.append({"spec": SOF, "command": "show_style_action", "input": {"show_style": "show_style0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/showstyles/show_style0/action", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_buckets", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1.0/buckets"}})
V.append({"spec": SOF, "command": "add_bucket", "input": {"body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/buckets", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "get_bucket", "input": {"bucket": "bucket0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/buckets/bucket0"}})
V.append({"spec": SOF, "command": "delete_bucket", "input": {"bucket": "bucket0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/buckets/bucket0"}})
V.append({"spec": SOF, "command": "import_bucket_adlib", "input": {"bucket": "bucket0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/buckets/bucket0/adlibs", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "delete_bucket_adlibs", "input": {"bucket": "bucket0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/buckets/bucket0/adlibs"}})
V.append({"spec": SOF, "command": "delete_bucket_adlib", "input": {"bucket": "bucket0", "external_id": "external_id0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/buckets/bucket0/adlibs/external_id0"}})
V.append({"spec": SOF, "command": "store_snapshot", "input": {"idempotency_key": "faf1ba52-a1b5-4958-9baa-b91727ace097", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/snapshots", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_get_playlists", "input": {"studio": "studio0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists"}})
V.append({"spec": SOF, "command": "ingest_delete_playlists", "input": {"studio": "studio0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists"}})
V.append({"spec": SOF, "command": "ingest_get_playlist", "input": {"studio": "studio0", "playlist": "playlist0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0"}})
V.append({"spec": SOF, "command": "ingest_delete_playlist", "input": {"studio": "studio0", "playlist": "playlist0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0"}})
V.append({"spec": SOF, "command": "ingest_post_rundown_in_studio", "input": {"studio": "studio0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/ingest/studio0/rundowns", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_get_rundowns", "input": {"studio": "studio0", "playlist": "playlist0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns"}})
V.append({"spec": SOF, "command": "ingest_post_rundown", "input": {"studio": "studio0", "playlist": "playlist0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_put_rundowns", "input": {"studio": "studio0", "playlist": "playlist0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_rundowns", "input": {"studio": "studio0", "playlist": "playlist0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns"}})
V.append({"spec": SOF, "command": "ingest_get_rundown", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0"}})
V.append({"spec": SOF, "command": "ingest_put_rundown", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_rundown", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0"}})
V.append({"spec": SOF, "command": "ingest_get_segments", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments"}})
V.append({"spec": SOF, "command": "ingest_post_segment", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_put_segments", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_segments", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments"}})
V.append({"spec": SOF, "command": "ingest_get_segment", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0"}})
V.append({"spec": SOF, "command": "ingest_put_segment", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_segment", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0"}})
V.append({"spec": SOF, "command": "ingest_get_parts", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts"}})
V.append({"spec": SOF, "command": "ingest_post_part", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "body": {"example": True}}, "expect_request": {"method": "POST", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_put_parts", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_parts", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts"}})
V.append({"spec": SOF, "command": "ingest_get_part", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "part": "part0"}, "expect_request": {"method": "GET", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts/part0"}})
V.append({"spec": SOF, "command": "ingest_put_part", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "part": "part0", "body": {"example": True}}, "expect_request": {"method": "PUT", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts/part0", "body": "{\"example\":true}"}})
V.append({"spec": SOF, "command": "ingest_delete_part", "input": {"studio": "studio0", "playlist": "playlist0", "rundown": "rundown0", "segment": "segment0", "part": "part0"}, "expect_request": {"method": "DELETE", "target": "/api/v1.0/ingest/studio0/playlists/playlist0/rundowns/rundown0/segments/segment0/parts/part0"}})

# ── Telemetry: REST reads, and the Live Status Gateway's events ──
telemetry(SOF, "version", inbound_http={"path": "/api/v1.0/", "body": json.dumps({"status": 200, "result": {"version": "1.52.0"}})},
          expect_state={"version": "1.52.0"})
telemetry(SOF, "health", inbound_http={"path": "/api/v1.0/health", "body": json.dumps({
    "name": "Sofie Automation system", "updated": "2026-10-28T10:00:00.000Z", "status": "WARNING", "version": "1.52.0",
    "blueprintsVersion": "1.0.60", "statusMessage": "Playout gateway: Disconnected",
    "components": [{"name": "Playout Gateway", "updated": "2026-10-28T10:00:00.000Z", "status": "FAIL", "version": "1.52.0"}]})},
    expect_state={"version": "1.52.0", "health": {"status": "WARNING", "message": "Playout gateway: Disconnected",
                  "updated": "2026-10-28T10:00:00.000Z", "blueprints_version": "1.0.60",
                  "components": {"0": {"name": "Playout Gateway", "status": "FAIL", "version": "1.52.0"}}}})
telemetry(SOF, "playlist-ids", inbound_http={"path": "/api/v1.0/playlists", "body": json.dumps({
    "status": 200, "result": [{"id": "OKAgZmZ0Buc99lE_2uPPSKVbMrQ_", "externalId": "1ZIY"}]})},
    state_before={"playlist_ids": {"0": "a", "1": "b"}}, expect_state={"playlist_ids": {"0": "OKAgZmZ0Buc99lE_2uPPSKVbMrQ_"}})
telemetry(SOF, "buckets", inbound_http={"path": "/api/v1.0/buckets", "body": json.dumps({
    "status": 200, "result": [{"id": "6jZ6NvpoikxuXqcm4", "name": "My Bucket", "studioId": "studio0"}]})},
    expect_state={"buckets": {"6jZ6NvpoikxuXqcm4": {"name": "My Bucket", "studio": "studio0"}}})
telemetry(SOF, "studio", inbound_ws=json.dumps({"event": "studio", "id": "B0avqzSM41UJDpbyf3U28", "name": "Studio 0",
    "playlists": [{"id": "OKAgZmZ0Buc99lE_2uPPSKVbMrQ_", "externalId": "1ZIY", "name": "Playlist 0", "activationStatus": "rehearsal"}]}),
    expect_connect_ws=['{"event":"subscribe","reqid":1,"subscription":{"name":"studio"}}',
                       '{"event":"subscribe","reqid":2,"subscription":{"name":"activePlaylist"}}',
                       '{"event":"subscribe","reqid":3,"subscription":{"name":"activePieces"}}',
                       '{"event":"subscribe","reqid":4,"subscription":{"name":"segments"}}'],
    expect_state={"studio": {"id": "B0avqzSM41UJDpbyf3U28", "name": "Studio 0"},
                  "playlists": {"OKAgZmZ0Buc99lE_2uPPSKVbMrQ_": {"name": "Playlist 0", "external_id": "1ZIY", "activation": "rehearsal"}}})
telemetry(SOF, "active-playlist", inbound_ws=json.dumps({
    "event": "activePlaylist", "id": "OKAg", "externalId": "1ZIY", "name": "Playlist 0", "rundownIds": ["y9Ha"],
    "currentPart": {"id": "H5CB", "name": "Intro", "autoNext": False, "createdByAdLib": False, "segmentId": "n1mO",
                    "pieces": [], "timing": {"startTime": 1600000060000, "expectedDurationMs": 15000, "projectedEndTime": 1600000075000}},
    "currentSegment": {"id": "n1mO", "name": "Segment 0", "timing": {"expectedDurationMs": 15000}, "parts": []},
    "nextPart": {"id": "Q3vU", "name": "Interview", "autoNext": False, "createdByAdLib": False, "segmentId": "n1mO", "pieces": []},
    "timing": {"timingMode": "forward-time", "expectedStart": 1728895750727, "expectedDurationMs": 180000},
    "quickLoop": {"locked": False, "running": True},
    "tTimers": [{"index": 1, "label": "Segment Timer", "configured": True, "mode": {"type": "countdown", "duration": 120000, "stopAtZero": True},
                 "state": {"paused": False, "zeroTime": 1706371920000, "pauseTime": None}, "projected": None, "anchorPartId": None}]}),
    state_before={"active_playlist": {"id": "old", "next_part": {"id": "gone"}}},
    expect_state={"active_playlist": {
        "id": "OKAg", "name": "Playlist 0", "external_id": "1ZIY",
        "current_part": {"id": "H5CB", "name": "Intro", "segment": "n1mO", "auto_next": False, "start_ms": 1600000060000,
                         "duration_ms": 15000, "end_ms": 1600000075000},
        "next_part": {"id": "Q3vU", "name": "Interview", "segment": "n1mO"},
        "current_segment": {"id": "n1mO", "name": "Segment 0"},
        "timing": {"mode": "forward-time", "expected_start_ms": 1728895750727, "expected_duration_ms": 180000},
        "quick_loop": {"running": True, "locked": False},
        "timers": {"1": {"label": "Segment Timer", "configured": True, "mode": "countdown", "paused": False,
                         "zero_time_ms": 1706371920000}}}})
telemetry(SOF, "no-active-playlist", inbound_ws=json.dumps({
    "event": "activePlaylist", "id": None, "externalId": None, "name": "", "rundownIds": [], "currentPart": None,
    "currentSegment": None, "nextPart": None, "timing": {"timingMode": "none"}, "tTimers": []}),
    state_before={"active_playlist": {"id": "OKAg", "current_part": {"id": "H5CB"}}},
    expect_state={"active_playlist": {"name": "", "timing": {"mode": "none"}}})
telemetry(SOF, "active-pieces", inbound_ws=json.dumps({"event": "activePieces", "rundownPlaylistId": "OKAg", "activePieces": [
    {"id": "H5CB", "name": "Camera 1", "sourceLayer": "Camera", "outputLayer": "PGM", "tags": ["camera"]}]}),
    expect_state={"active_pieces": {"H5CB": {"name": "Camera 1", "source_layer": "Camera", "output_layer": "PGM"}}})
telemetry(SOF, "segments", inbound_ws=json.dumps({"event": "segments", "rundownPlaylistId": "OKAg", "segments": [
    {"id": "n1mO", "identifier": "Segment 0 identifier", "rundownId": "y9Ha", "name": "Segment 0", "timing": {"expectedDurationMs": 15000}}]}),
    expect_state={"segments": {"n1mO": {"name": "Segment 0", "identifier": "Segment 0 identifier", "rundown": "y9Ha", "duration_ms": 15000}}})
