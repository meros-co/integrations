# AWS Elemental MediaLive (aws-elemental-medialive): one vector per command,
# its request worked out here from the spec's operation (the method and path
# of the API reference, IDs put into the path with RFC 3986 encoding, the
# query in the order the spec lists it, a JSON body as json.dumps writes it),
# and, for a few, the AWS Signature Version 4 headers worked out here
# independently with hmac and hashlib (the vector runner's clock reads the
# Unix epoch; the host is the vector device's 192.0.2.10). The credentials are
# AWS's documented example keys, not real ones. Telemetry vectors follow the
# API reference's response members.
import hashlib as _ml_hashlib
import hmac as _ml_hmac
from urllib.parse import quote as _ml_quote

ML = "aws-elemental-medialive"
_ML_SET = {"region": "us-east-1", "access_key_id": "AKIDEXAMPLE",
           "secret_access_key": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY"}
_ML_EXAMPLES = {"channel_id": "1234567", "input_id": "7654321", "input_device_id": "hd-123456789abcdef",
                "input_security_group_id": "123456", "multiplex_id": "3456789", "program_name": "program 1",
                "offering_id": "87654321", "reservation_id": "1234567",
                "resource_arn": "arn:aws:medialive:us-east-1:123456789012:channel:1234567",
                "identifier": "aw-0123456789abcdef", "cluster_id": "1234567", "node_id": "2345678",
                "network_id": "3456789", "channel_placement_group_id": "4567890", "sdi_source_id": "5678901",
                "tag_key": "team", "next_token": "abc+def/=="}
_ML_BODY = {"name": "Studio A", "tags": {"team": "live"}}


def _ml_canonical_query(query):
    pairs = sorted((_ml_quote(k, safe="-._~"), _ml_quote(v, safe="-._~")) for k, v in query)
    return "&".join(f"{k}={v}" for k, v in pairs)


def _ml_auth(method, path, query, body, content_type):
    """The Authorization header AWS Signature Version 4 gives this request."""
    date, day = "19700101T000000Z", "19700101"
    headers = {"host": "192.0.2.10", "x-amz-date": date}
    if content_type:
        headers["content-type"] = content_type
    names = sorted(headers)
    canonical = "\n".join([
        method,
        _ml_quote(path, safe="/-._~"),
        _ml_canonical_query(query),
        "".join(f"{n}:{headers[n]}\n" for n in names),
        ";".join(names),
        _ml_hashlib.sha256((body or "").encode()).hexdigest(),
    ])
    scope = f"{day}/us-east-1/medialive/aws4_request"
    to_sign = "\n".join(["AWS4-HMAC-SHA256", date, scope, _ml_hashlib.sha256(canonical.encode()).hexdigest()])
    key = ("AWS4" + _ML_SET["secret_access_key"]).encode()
    for part in (day, "us-east-1", "medialive", "aws4_request"):
        key = _ml_hmac.new(key, part.encode(), _ml_hashlib.sha256).digest()
    signature = _ml_hmac.new(key, to_sign.encode(), _ml_hashlib.sha256).hexdigest()
    return f"AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/{scope}, SignedHeaders={';'.join(names)}, Signature={signature}"


def _ml_vectors():
    doc = yaml.safe_load((ROOT / "specs" / f"{ML}.yaml").read_text(encoding="utf-8"))
    signed = {"start_channel", "list_channels_page", "create_tags", "describe_thumbnails", "batch_start"}
    for name, command in doc["commands"].items():
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            if p["type"] == "json":
                values[pname] = _ML_BODY
            elif pname in _ML_EXAMPLES:
                values[pname] = _ML_EXAMPLES[pname]
            elif "default" in p:
                values[pname] = p["default"]
            else:
                raise SystemExit(f"{ML}: no example for {name}.{pname}")
        send = command["send"]
        path = send["path"]
        for pname, v in values.items():
            path = path.replace("{" + pname + "}", _ml_quote(str(v), safe="-._~"))
        query = []
        for k, t in (send.get("query") or {}).items():
            query.append((k, str(values[t.strip("{}")])))
        target = path + ("?" + "&".join(f"{_ml_quote(k, safe='-._~')}={_ml_quote(v, safe='-._~')}" for k, v in query)
                         if query else "")
        request = {"method": send["method"], "target": target}
        body = None
        if "body" in send:
            body = json.dumps(values["body"], separators=(",", ":"))
            request["body"] = body
            request["content_type"] = "application/json"
        vector = {"spec": ML, "command": name, "input": values, "settings": dict(_ML_SET),
                  "expect_request": request}
        if name in signed:
            request["headers"] = {"Host": "192.0.2.10", "X-Amz-Date": "19700101T000000Z",
                                  "Authorization": _ml_auth(send["method"], path, query, body,
                                                            request.get("content_type"))}
        V.append(vector)


_ml_vectors()

# Temporary credentials: the session token goes as X-Amz-Security-Token and
# is signed.
V.append({"spec": ML, "command": "describe_channel", "input": {"channel_id": "1234567"}, "file": "describe_channel-session-token",
          "settings": {**_ML_SET, "session_token": "FQoGZXIvYXdzEXAMPLE"},
          "expect_request": {"method": "GET", "target": "/prod/channels/1234567",
                             "headers": {"X-Amz-Security-Token": "FQoGZXIvYXdzEXAMPLE"}}})
# Answers: the channel as DescribeChannel gives it.
V.append({"spec": ML, "command": "describe_channel", "input": {"channel_id": "1234567"}, "file": "describe_channel-reply",
          "settings": dict(_ML_SET),
          "expect_request": {"method": "GET", "target": "/prod/channels/1234567"},
          "http_reply": {"status": 200, "body": '{"id":"1234567","name":"Studio A","state":"IDLE"}'},
          "expect_result": {"ok": {"kind": "value", "value": {"id": "1234567", "name": "Studio A", "state": "IDLE"}}}})
# An error answer fails the command.
V.append({"spec": ML, "command": "start_channel", "input": {"channel_id": "1234567"}, "file": "start_channel-conflict",
          "settings": dict(_ML_SET),
          "expect_request": {"method": "POST", "target": "/prod/channels/1234567/start"},
          "http_reply": {"status": 409, "body": '{"message":"Channel is not in a state that can be started"}'},
          "expect_result": {"error": {"error": "device_error", "code": "409"}}})


def _ml_t(name, path, body, state, **extra):
    telemetry(ML, name, settings=dict(_ML_SET), inbound_http={"path": path, "body": json.dumps(body)},
              expect_state=state, **extra)


_ml_t("channels", "/prod/channels?maxResults=1000", {"channels": [
    {"arn": "arn:aws:medialive:us-east-1:123456789012:channel:1234567", "channelClass": "STANDARD",
     "destinations": [{"id": "dest1", "settings": [{"url": "rtmp://a/live"}]}], "id": "1234567",
     "inputAttachments": [{"inputId": "7654321", "inputAttachmentName": "cam"}], "logLevel": "INFO",
     "name": "Studio A", "pipelinesRunningCount": 2, "state": "RUNNING",
     "channelEngineVersion": {"version": "1.0.0"}}]},
    {"channels": {"1234567": {
        "name": "Studio A", "state": "RUNNING", "arn": "arn:aws:medialive:us-east-1:123456789012:channel:1234567",
        "channel_class": "STANDARD", "pipelines_running": 2, "log_level": "INFO", "engine_version": "1.0.0",
        "input_attachments": '[{"inputId":"7654321","inputAttachmentName":"cam"}]',
        "destinations": '[{"id":"dest1","settings":[{"url":"rtmp://a/live"}]}]'}}},
    state_before={"channels": {"999": {"name": "Deleted", "state": "IDLE"}}})
_ml_t("channel-described", "/prod/channels/1234567/stop",
      {"id": "1234567", "name": "Studio A", "state": "STOPPING", "pipelinesRunningCount": 2},
      {"channels": {"1234567": {"name": "Studio A", "state": "STOPPING", "pipelines_running": 2}}})
_ml_t("channel-created", "/prod/channels",
      {"channel": {"id": "2345678", "name": "Studio B", "state": "CREATING", "channelClass": "SINGLE_PIPELINE"}},
      {"channels": {"2345678": {"name": "Studio B", "state": "CREATING", "channel_class": "SINGLE_PIPELINE"}}})
_ml_t("inputs", "/prod/inputs?maxResults=1000", {"inputs": [
    {"arn": "arn:aws:medialive:us-east-1:123456789012:input:7654321", "attachedChannels": ["1234567"],
     "destinations": [{"ip": "203.0.113.5", "port": "1935", "url": "rtmp://203.0.113.5:1935/live/a"}],
     "id": "7654321", "inputClass": "STANDARD", "inputSourceType": "STATIC", "name": "cam", "securityGroups": ["123456"],
     "sources": [], "state": "ATTACHED", "type": "RTMP_PUSH"}]},
    {"inputs": {"7654321": {
        "name": "cam", "state": "ATTACHED", "arn": "arn:aws:medialive:us-east-1:123456789012:input:7654321",
        "type": "RTMP_PUSH", "input_class": "STANDARD", "source_type": "STATIC", "attached_channels": '["1234567"]',
        "destinations": '[{"ip":"203.0.113.5","port":"1935","url":"rtmp://203.0.113.5:1935/live/a"}]',
        "sources": "[]", "security_groups": '["123456"]'}}})
_ml_t("multiplexes", "/prod/multiplexes?maxResults=1000", {"multiplexes": [
    {"arn": "arn:aws:medialive:us-east-1:123456789012:multiplex:3456789", "availabilityZones": ["us-east-1a", "us-east-1b"],
     "id": "3456789", "name": "mux", "pipelinesRunningCount": 0, "programCount": 3, "state": "IDLE"}]},
    {"multiplexes": {"3456789": {"name": "mux", "state": "IDLE",
                                  "arn": "arn:aws:medialive:us-east-1:123456789012:multiplex:3456789",
                                  "pipelines_running": 0, "program_count": 3,
                                  "availability_zones": '["us-east-1a","us-east-1b"]'}}})
_ml_t("input-devices", "/prod/inputDevices?maxResults=1000", {"inputDevices": [
    {"arn": "arn:aws:medialive:us-east-1:123456789012:inputDevice:hd-123456789abcdef", "connectionState": "CONNECTED",
     "deviceSettingsSyncState": "SYNCED", "deviceUpdateStatus": "UP_TO_DATE", "id": "hd-123456789abcdef",
     "macAddress": "00:11:22:33:44:55", "name": "Link 1", "serialNumber": "AB1234", "type": "HD",
     "availabilityZone": "us-east-1a", "outputType": "MEDIALIVE_INPUT"}]},
    {"input_devices": {"hd-123456789abcdef": {
        "name": "Link 1", "connection_state": "CONNECTED", "type": "HD",
        "arn": "arn:aws:medialive:us-east-1:123456789012:inputDevice:hd-123456789abcdef",
        "device_update_status": "UP_TO_DATE", "settings_sync_state": "SYNCED", "serial_number": "AB1234",
        "mac_address": "00:11:22:33:44:55", "availability_zone": "us-east-1a", "output_type": "MEDIALIVE_INPUT"}}})
_ml_t("input-security-groups", "/prod/inputSecurityGroups?maxResults=1000", {"inputSecurityGroups": [
    {"arn": "arn:aws:medialive:us-east-1:123456789012:inputSecurityGroup:123456", "id": "123456",
     "inputs": ["7654321"], "state": "IN_USE", "whitelistRules": [{"cidr": "203.0.113.0/24"}]}]},
    {"input_security_groups": {"123456": {
        "state": "IN_USE", "arn": "arn:aws:medialive:us-east-1:123456789012:inputSecurityGroup:123456",
        "whitelist_rules": '[{"cidr":"203.0.113.0/24"}]', "inputs": '["7654321"]'}}})
_ml_t("security-group-updated", "/prod/inputSecurityGroups/123456",
      {"securityGroup": {"id": "123456", "state": "UPDATING", "whitelistRules": []}},
      {"input_security_groups": {"123456": {"state": "UPDATING", "whitelist_rules": "[]"}}})
_ml_t("account", "/prod/accountConfiguration", {"accountConfiguration": {"kmsKeyId": "alias/medialive"}},
      {"account": {"kms_key_id": "alias/medialive"}})
