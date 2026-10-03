#!/usr/bin/env python3
"""Generate the commands of specs/obs-studio.yaml from obs-websocket's own
machine-readable protocol description (docs/generated/protocol.json), so every
command is one of its requests.

    python tools/generate_obs.py [path/to/protocol.json]

Without a path the description is downloaded from the obs-websocket
repository. It is read as documentation; no code is used, and it is not stored
here. Everything outside `commands:` and the model's `supports` is kept as
written.

Naming is mechanical and reversible, and the native module relies on it: a
command is the request type in snake_case (GetSceneList -> get_scene_list),
and a parameter is the request field in snake_case (sceneName -> scene_name).
"""
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "obs-studio.yaml"
URL = "https://raw.githubusercontent.com/obsproject/obs-websocket/master/docs/generated/protocol.json"


def load() -> dict:
    if len(sys.argv) > 1:
        return json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    return json.loads(urllib.request.urlopen(URL, timeout=30).read().decode("utf-8"))


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def camel(name: str, upper: bool) -> str:
    s = "".join(p[:1].upper() + p[1:] for p in name.split("_"))
    return s if upper else s[:1].lower() + s[1:]


def yq(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def bounds(restriction: str) -> dict:
    out = {}
    for part in (restriction or "").split(","):
        m = re.match(r"\s*(>=|<=)\s*(-?[0-9.]+)\s*$", part)
        if m:
            out["min" if m.group(1) == ">=" else "max"] = m.group(2)
    return out


# Labels are the field name in words; these read better spelled out.
LABELS = {
    "input_volume_db": "Volume (dB)",
    "input_volume_mul": "Volume (multiplier)",
    "input_audio_balance": "Audio balance",
    "input_audio_sync_offset": "Audio sync offset",
    "input_audio_tracks": "Audio tracks",
    "input_deinterlace_mode": "Deinterlace mode",
    "input_deinterlace_field_order": "Deinterlace field order",
    "input_muted": "Mute",
    "monitor_type": "Monitor type",
    "position": "T-bar position",
    "studio_mode_enabled": "Studio mode",
    "scene_item_enabled": "Enabled",
    "scene_item_locked": "Locked",
    "filter_enabled": "Enabled",
    "sleep_millis": "Sleep time (ms)",
}

# The protocol's own field descriptions, rewritten where they are more than
# one sentence, or say less than the person choosing a value needs.
DESCRIPTIONS = {
    "imageCompressionQuality": "Compression quality from 0 (high compression) to 100 (uncompressed), or -1 for the default.",
    "imageFilePath": "Path on the OBS computer to save the screenshot to, such as C:\\Users\\user\\Desktop\\screenshot.png.",
    "imageFormat": "Image format, one of those get_version lists as supported, such as png.",
    "keyId": "OBS key ID, such as OBS_KEY_A, as listed in libobs/obs-hotkeys.h.",
    "mediaAction": "ObsMediaInputAction value, such as OBS_WEBSOCKET_MEDIA_INPUT_ACTION_PLAY.",
    "mediaCursor": "New cursor position in milliseconds from the start of the media.",
    "mediaCursorOffset": "Milliseconds to move the cursor by; negative moves it back.",
    "monitorIndex": "Monitor index as get_monitor_list gives it.",
    "monitorType": "Audio monitor type: OBS_MONITORING_TYPE_NONE, OBS_MONITORING_TYPE_MONITOR_ONLY or OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT.",
    "overlay": "On applies the settings on top of the existing ones, off resets to the defaults first and then applies them.",
    "parameterValue": "Value of the parameter to set.",
    "projectorGeometry": "Size and position of a windowed projector in Qt Base64 format; not used with a monitor index.",
    "realm": "Data realm: OBS_WEBSOCKET_DATA_REALM_GLOBAL or OBS_WEBSOCKET_DATA_REALM_PROFILE.",
    "release": "Whether to release the T-bar; set it off only when another position update follows.",
    "searchOffset": "Number of matches to skip, searching forward from the first; -1 gives the last (top) item.",
    "streamServiceType": "Type of stream service, such as rtmp_common or rtmp_custom.",
    "studioModeEnabled": "On enables studio mode, off disables it.",
    "transitionSettings": "Settings object to apply to the transition; it can be {}.",
    "unversioned": "On returns every kind without its version suffix, off returns them with version suffixes where there are any.",
    "inputVolumeMul": "Volume as a linear multiplier, 1.0 for unchanged; give this or the volume in dB.",
    "inputVolumeDb": "Volume in dB, 0 for unchanged; give this or the volume as a multiplier.",
    "inputAudioBalance": "Audio balance from 0.0 (left) to 1.0 (right), 0.5 centred.",
    "inputMuted": "On mutes the input, off unmutes it.",
    "filterEnabled": "On enables the filter, off disables it.",
    "position": "New T-bar position from 0.0 to 1.0.",
    "outputSettings": "Settings object to apply to the output.",
    "recordDirectory": "Directory on the OBS computer that recordings are saved to.",
    "requestData": "Request data object for the vendor's request.",
    "videoMixType": "Type of mix to open, such as OBS_WEBSOCKET_VIDEO_MIX_TYPE_PREVIEW, _PROGRAM or _MULTIVIEW.",
    "sceneItemBlendMode": "New blend mode, such as OBS_BLEND_NORMAL.",
    ("CreateScene", "canvasUuid"): "UUID of the canvas to create the new scene in; leave it out for the main canvas.",
    ("SetCurrentSceneTransitionDuration", "transitionDuration"): "Transition duration in milliseconds.",
    ("SetSceneSceneTransitionOverride", "transitionDuration"): "Duration in milliseconds for the overriding transition.",
    ("SetSceneSceneTransitionOverride", "transitionName"): "Name of the scene transition to use as the override.",
}


def label(pname: str) -> str:
    if pname in LABELS:
        return LABELS[pname]
    words = {"uuid": "UUID", "id": "ID", "db": "dB", "fps": "FPS", "tbar": "T-bar"}
    parts = [words.get(w, w) for w in pname.split("_")]
    parts[0] = parts[0][:1].upper() + parts[0][1:]
    return " ".join(parts)


def plain(s: str) -> str:
    return s if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ()'.+-]*", s) else yq(s)


def describe(request_type: str, f: dict) -> str:
    d = DESCRIPTIONS.get((request_type, f["valueName"])) or DESCRIPTIONS.get(f["valueName"])
    if d is None:
        d = " ".join(f["valueDescription"].replace("`", "").split()).rstrip(".")
        assert ". " not in d, (request_type, f["valueName"], d)
        # Field names in the text are the request's; the spec's are snake_case.
        d = re.sub(r"\b[a-z]+[A-Z][A-Za-z]*\b", lambda m: snake(m.group(0)), d)
        d = d.replace(" to created", " to be created")
        d = d[:1].upper() + d[1:] + "."
    return d


protocol = load()
commands = []
names = []
for r in protocol["requests"]:
    if r.get("deprecated"):
        continue
    request_type = r["requestType"]
    name = snake(request_type)
    assert camel(name, True) == request_type, request_type
    names.append(name)
    summary = " ".join(r["description"].split())
    if r.get("initialVersion") and r["initialVersion"] != "5.0.0":
        summary += f" (obs-websocket {r['initialVersion']} and later.)"
    lines = [f"  {name}:", f"    summary: {yq(summary)}"]
    fields = r["requestFields"]
    # Nested fields (keyModifiers.shift) describe an object parameter: it is
    # one json parameter, and its fields are named in the summary.
    nested = {}
    for f in fields:
        if "." in f["valueName"]:
            parent, child = f["valueName"].split(".", 1)
            nested.setdefault(parent, []).append(child)
    if nested:
        described = "; ".join(
            f"{snake(p)} is an object with {', '.join(c)}" for p, c in nested.items()
        )
        lines[1] = f"    summary: {yq(summary + ' ' + described + '.')}"
    if fields:
        lines.append("    params:")
        for f in fields:
            field = f["valueName"]
            if "." in field:
                continue
            pname = snake(field)
            assert camel(pname, False) == field, field
            kind = {"String": "string", "Boolean": "bool", "Number": "float"}.get(
                f["valueType"], "json")
            spec = [f"type: {kind}"]
            for k, v in bounds(f.get("valueRestrictions")).items():
                spec.append(f"{k}: {v}")
            if not f.get("valueOptional"):
                spec.append("required: true")
            spec.append(f"label: {plain(label(pname))}")
            spec.append(f"description: {yq(describe(request_type, f))}")
            lines.append(f"      {pname}: {{ {', '.join(spec)} }}")
    lines.append(f"    returns: {'value' if r['responseFields'] else 'ack'}")
    commands.append("\n".join(lines))

text = SPEC.read_text(encoding="utf-8")
head, rest = text.split("\ncommands:\n", 1)
tail = rest[rest.index("\nstate:\n"):]

supports, line = [], "    supports: ["
for i, n in enumerate(names):
    piece = n + (", " if i < len(names) - 1 else "]")
    if len(line) + len(piece) > 88:
        supports.append(line.rstrip())
        line = "               "
    line += piece
supports.append(line)
head = re.sub(r"    supports: \[.*?\]\n", "\n".join(supports) + "\n", head, count=1, flags=re.S)

SPEC.write_text(head + "\ncommands:\n" + "\n\n".join(commands) + "\n" + tail,
                encoding="utf-8", newline="\n")
print(f"{len(commands)} commands")
