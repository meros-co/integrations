#!/usr/bin/env python3
"""Generate the commands of specs/vmix.yaml, and the command-to-function table
crates/core/src/modules/vmix_functions.rs, from vMix's Shortcut Function
Reference, so every command is one of its functions.

    python tools/generate_vmix.py [path/to/ShortcutFunctionReference.html]

Without a path the page is downloaded from vmix.com. It is not stored here.
Everything in the spec outside `commands:` and the model's `supports` is kept.

A command is the function in snake_case; its parameters are the function's
(Input, Value, Duration, SelectedName, SelectedIndex, Mix, Channel) in
snake_case, all optional, since vMix applies its own defaults and refuses what
it cannot use (FUNCTION ER). Function names do not convert back from
snake_case reliably (PTZMoveUp), so the module looks them up in the table.
"""
import html
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "vmix.yaml"
TABLE = ROOT / "crates" / "core" / "src" / "modules" / "vmix_functions.rs"
URL = "https://www.vmix.com/help29/ShortcutFunctionReference.html"

PARAMS = {
    "Input": ('input', "{ type: string, max_length: 256 }"),
    "Value": ('value', "{ type: string, max_length: 4096 }"),
    "Duration": ('duration', "{ type: int, min: 0, max: 600000 }"),
    "SelectedName": ('selected_name', "{ type: string, max_length: 256 }"),
    "SelectedIndex": ('selected_index', "{ type: int, min: 0 }"),
    "Mix": ('mix', "{ type: int, min: 0, max: 16 }"),
    "Channel": ('channel', "{ type: string, max_length: 16 }"),
}

# Labels and descriptions. Value means something different in almost every
# function; the reference ends each description with "Value = <what it is>",
# which picks the text here, refined by the function where the same words
# mean different things.
VALUE = {
    "Alpha 0-255": ("Alpha", "Transparency from 0 (transparent) to 255 (opaque)."),
    "Animation": ("Animation", "Name of the title's animation to begin, as named in the title."),
    "AudioSource": ("Audio source", "Audio source as named in the replay's dropdown, such as Camera1."),
    "Balance -1-1": ("Balance", "Audio balance from -1 (left) to 1 (right), 0 centred."),
    "Bus": ("Bus", "Audio bus: M for master, or A to G."),
    "Bus,PluginNumber": ("Bus and plugin", "Bus (M or A to G) and plugin number from 1, separated by a comma, such as A,1."),
    "Bus,Volume 0-100": ("Bus and volume", "Bus (M or A to G) and volume from 0 to 100, separated by a comma, such as A,80."),
    "Camera": ("Camera", "Camera angle, numbered from 1 (1 to 8)."),
    "Camera,Text": ("Camera and text", "Camera angle (1 to 4) and the text, separated by a comma, such as 3,angle3text."),
    "Category": ("Category", "Category to show: All, Red, Green, Orange, Purple, Aqua, Blue, Custom1 to Custom16, or Search."),
    "Channel,Volume 0-100": ("Channel and volume", "Sub channel (1 to 16) and volume from 0 to 100, separated by a comma, such as 2,80."),
    "Code": ("Code", "Script code to run."),
    "Color": ("Colour", "Colour in HTML #xxxxxxxx format."),
    "Colour": ("Colour", "Text colour in HTML #xxxxxx format."),
    "Command": ("Command", "Command to send to the NDI source."),
    "Count -10-10": ("Count", "Events to move through the list, from -10 (back) to 10 (forward)."),
    "Duration 00:00:00": ("Duration", "Countdown duration as hh:mm:ss, such as 00:05:00."),
    "Duration MS": ("Duration", "Duration in milliseconds."),
    "Event List 0-19": ("Event list", "Event list to copy or move the event to, numbered from 0 (0 to 19)."),
    "Event Number where 0 is bottom of list 0-1000": ("Event", "Event number, where 0 is the bottom of the list (0 to 1000)."),
    "Fader 0-255": ("T-bar", "T-bar position from 0 to 255; 255 cuts to preview."),
    "Filename": ("File name", "Full path of the file on the vMix computer."),
    "Folder": ("Folder", "Folder on the vMix computer to export to."),
    "Frames": ("Frames", "Number of frames."),
    "FromIndex,ToIndex": ("From and to", "Two indexes from 1 separated by a comma: 1,2 moves the first to the second."),
    "FromIndex,ToIndex,DurationMilliseconds": ("Layers and duration", "Two layer indexes from 1 and a time in milliseconds, comma-separated: 1,2,1000 swaps Layer 1 and Layer 2 over 1000 ms."),
    "Gain dB 0-24": ("Gain", "Gain in dB, from 0 to 24."),
    "Index": ("Index", "Index, numbered from 1."),
    "Index 0-100": ("Index", "Source index in the list, numbered from 0 (0 to 100)."),
    "Index,Input": ("Layer and input", "Layer index from 1 and an input, separated by a comma: 1,2 puts Input 2 in Layer 1."),
    "Index,Input,DurationMilliseconds": ("Layer, input and time", "Layer index from 1, input and time in milliseconds, comma-separated: 1,2,1000 puts Input 2 in Layer 1, animated over 1000 ms."),
    "Input": ("Input", "Input number or name to use as the dynamic input."),
    "Key": ("Key", "Key to press, as vMix names it."),
    "Keys": ("Keys", "Keys to send to the active window."),
    "List of IDs": ("Event IDs", "List of event IDs; the reference does not give its format further."),
    "MeetingID,Password": ("Meeting and password", "Zoom meeting ID and password, separated by a comma."),
    "Milliseconds": ("Position", "Playback position in milliseconds from the start."),
    "Name": ("Name", "Name, as vMix shows it."),
    "Name,Password": ("Name and password", "Name and password for the call, separated by a comma."),
    "Name,Table": ("Data source and table", "Data source name and optional table name, separated by a comma, such as Excel/CSV,Sheet1."),
    "Name,Table,Index": ("Data source, table and row", "Data source name, optional table name and row index from 0, comma-separated, such as Excel/CSV,Sheet1,5."),
    "Number": ("Position", "Input number to move the input to."),
    "Output": ("Output", "SRT output number from 0; left out, only Output 1."),
    "Output, Preview, MultiView, Replay, Mix, Input": ("Source", "What the output shows: Output, Preview, MultiView, Replay, Mix or Input."),
    "PlayList": ("Playlist", "Name of the playlist to open."),
    "Plugin Number": ("Plugin", "Audio plugin number, from 1."),
    "Preset Index": ("Preset", "Title preset index, as vMix numbers it."),
    "Preset Name": ("Preset", "Name of the channel matrix preset to apply."),
    "Script Name": ("Script", "Script name, as in vMix's script list."),
    "Seconds": ("Seconds", "Time in seconds."),
    "Speed": ("Speed", "Speed; the reference gives no range."),
    "Speed 0-1": ("Speed", "Speed from 0 to 1."),
    "Speed 0-1000": ("Speed", "Ticker speed from 0 to 1000."),
    "Speed 0.1-4": ("Rate", "Playback rate from 0.1 to 4, where 0.5 is 50%, 1 is 100% and 2 is 200%."),
    "Stream": ("Stream", "Stream number from 0; left out, every stream."),
    "Tag Text": ("Tag", "Tag text written to the log with the duration; optional."),
    "Text": ("Text", "Text for the event."),
    "Time 00:00:00": ("Time", "New countdown time as hh:mm:ss, such as 00:05:00."),
    "Timecode": ("Timecode", "Timecode as yyyy-MM-ddTHH:mm:ss.fff."),
    "Transition": ("Transition", "Transition effect name, such as Fade, Zoom or Wipe."),
    "Type|Filename": ("Type and file", "Input type and path separated by |, such as Video|c:\\path\\to\\video.avi; other types are Image, Photos, Title, VideoList, Colour, AudioFile, Flash and PowerPoint."),
    "URL": ("URL", "Web address to open."),
    "Value": ("Value", "Value to set."),
    "Value -1-1": ("Level", "Level from -1 to 1, 0 the original."),
    "Value 0-1": ("Strength", "Effect strength from 0 to 1."),
    "Value 0-2": ("Gain", "Gain from 0 to 2, 1 the original."),
    "Volume 0-100": ("Volume", "Volume from 0 to 100."),
    "Volume 0-100,Milliseconds": ("Volume and time", "Target volume from 0 to 100 and fade time in milliseconds, separated by a comma, such as 50,2000."),
    "X,Y,Width,Height": ("Rectangle", "X, Y, width and height in pixels, separated by commas."),
    "X1 0-1": ("Crop X1", "Crop X1 from 0 (no crop) to 1 (full crop)."),
    "X2 0-1": ("Crop X2", "Crop X2 from 1 (no crop) to 0 (full crop)."),
    "Y1 0-1": ("Crop Y1", "Crop Y1 from 0 (no crop) to 1 (full crop)."),
    "Y2 0-1": ("Crop Y2", "Crop Y2 from 1 (no crop) to 0 (full crop)."),
    "X1,Y1,X2,Y2": ("Crop", "Crop edges X1, Y1, X2 and Y2 between 0 and 1, separated by commas."),
    "Zoom 0-5": ("Zoom", "Zoom from 0 to 5, where 1 is 100%, 0.5 is 50% and 2 is 200%."),
}

# Where the same Value words mean different things, by function name prefix
# or exact function, in order: the first match wins.
VALUE_BY_FUNCTION = [
    (r"Set(Layer\d+|LayerDynamic)?PanX$", ("Pan X", "Horizontal pan from -2 (100% to the left) to 2 (100% to the right), 0 centred.")),
    (r"Set(Layer\d+|LayerDynamic)?PanY$", ("Pan Y", "Vertical pan from -2 (100% to the bottom) to 2 (100% to the top), 0 centred.")),
    (r"Set(Layer\d+|LayerDynamic)X$", ("X position", "X position in pixels at the preset resolution, from -4096 to 4096.")),
    (r"Set(Layer\d+|LayerDynamic)Y$", ("Y position", "Y position in pixels at the preset resolution, from -4096 to 4096.")),
    (r"Set(Layer\d+|LayerDynamic)Width$", ("Width", "Width in pixels at the preset resolution, from -4096 to 4096.")),
    (r"Set(Layer\d+|LayerDynamic)Height$", ("Height", "Height in pixels at the preset resolution, from -4096 to 4096.")),
    (r"SetCCHue$", ("Hue", "Hue from -1 to 1, 0 the original.")),
    (r"SetCCSaturation$", ("Saturation", "Saturation from -1 (greyscale) to 1, 0 the original.")),
    (r"SetCCGamma", ("Gamma", "Gamma from -1 to 1, 0 the original.")),
    (r"SetCCLift", ("Lift", "Lift from -1 to 1, 0 the original.")),
    (r"SetDynamicValue\d$", ("Value", "Text to use wherever Dynamic1 to Dynamic4 is given as a shortcut value.")),
    (r"Snapshot", ("File name", "File name to save to, which can include a date such as mysnapshot {0:dd MMM yyyy}.jpg; left out, a save window appears.")),
    (r"Layer(On|Off|OnOff)$", ("Layer", "Layer index, numbered from 1.")),
    (r"ListRemove$", ("Item", "List item index, numbered from 1.")),
    (r"MultiViewOverlay", ("Overlay", "MultiView overlay index, numbered from 1.")),
    (r"SelectIndex$", ("Index", "List item, virtual set preset or title page, numbered from 1.")),
    (r"PTZMoveToVirtualInputPositionByIndex$", ("Index", "Which PTZ virtual input of this input, numbered from 0 in the order found.")),
    (r"SetInputName$", ("Name", "Display name for the input.")),
    (r"ZoomSelectParticipantByName$", ("Participant", "Zoom participant name.")),
    (r"NDISelectSourceByName$", ("Source", "NDI source name.")),
    (r"OMTSelectSourceByName$", ("Source", "OMT source name.")),
    (r"DataSource(Play|Pause|PlayPause)$", ("Data source", "Data source name, such as Excel/CSV.")),
    (r"ListAdd$", ("File name", "Full path of the file on the vMix computer to add to the list.")),
    (r"ListExport$", ("File name", "Full path on the vMix computer to export the list to as M3U.")),
    (r"SetImage$", ("File name", "Full path of the image on the vMix computer; empty clears it.")),
    (r"OpenPreset$", ("File name", "Full path of the preset file on the vMix computer to load.")),
    (r"SavePreset$", ("File name", "Full path on the vMix computer to save the preset to.")),
    (r"SetFrameDelay$", ("Delay", "Delay in frames.")),
    (r"ReplayJumpFrames$", ("Frames", "Frames to jump, or seconds when fast jumping is on; negative jumps back.")),
    (r"ReplayMoveSelected(In|Out)Point$", ("Frames", "Frames to move the point by.")),
    (r"AdjustCountdown$", ("Seconds", "Seconds to add to the countdown; negative subtracts.")),
    (r"SetPictureTransition$", ("Time", "Time between photos or slides, in seconds.")),
    (r"ReplayMarkInOutLiveFuture$", ("Seconds", "Seconds into the future to use for the new event.")),
    (r"ReplayMarkInOut", ("Seconds", "Seconds back from now to use for the new event.")),
    (r"ReplayFast(Forward|Backward)$", ("Speed", "Speed multiple, from 1 to 30x.")),
    (r"ReplayChangeSpeed$", ("Speed change", "Amount to change the replay speed by; the reference gives no range.")),
    (r"Replay(SetSpeed|UpdateSelectedSpeedFromValue)$", ("Speed", "Slow motion speed from 0 to 1, where 0.5 is 50% and 1 is 100%.")),
    (r"SetRateSlowMotion$", ("Speed", "Slow motion speed from 0 to 1, where 0.5 is 50% and 1 is 100%.")),
    (r"PTZ", ("Speed", "Speed of the move, zoom or focus, from 0 to 1.")),
    (r"SetTransitionDuration\d$", ("Duration", "Transition duration in milliseconds.")),
    (r"SetPictureEffectDuration$", ("Duration", "Duration of the photo transition effect in milliseconds.")),
    (r"SetPictureEffect$", ("Transition", "Transition effect between photos or slides, such as Fade or Zoom.")),
    (r"SetText$", ("Text", "Text to show in the title's text field.")),
    (r"VideoCallAudioSource$", ("Audio source", "Master, Headphones, or BusA to BusG.")),
    (r"VideoCallVideoSource$", ("Video source", "Output1, Output2, Output3 or Output4.")),
    (r"StreamingSetKey$", ("Stream key", "Stream key, optionally preceded by a stream number from 0 and a comma, such as 0,mystreamkey.")),
    (r"StreamingSetPassword$", ("Password", "Password, optionally preceded by a stream number from 0 and a comma, such as 0,password.")),
    (r"StreamingSetURL$", ("URL", "URL, optionally preceded by a stream number from 0 and a comma, such as 0,rtmp://myurl/.")),
    (r"StreamingSetUsername$", ("Username", "Username, optionally preceded by a stream number from 0 and a comma, such as 0,username.")),
    (r"BrowserNavigate$", ("URL", "Web address to open in the browser input.")),
    (r"SetVolumeChannel[12]$", ("Volume", "Channel volume from 0 to 100, for an audio input using SeparateMono.")),
]

OTHER = {
    "input": ("Input", "Input by number, exact title (case-sensitive) or key; left out, vMix uses its own default."),
    "mix": ("Mix", "Mix as vMix's API numbers it, 0 or left out for the main mix."),
    "channel": ("Replay channel", "Replay channel, such as A or B; left out, vMix uses its own default."),
    "duration": ("Duration", "Duration in milliseconds."),
    "selected_name": ("Selected name", "Name of the title field or object to change."),
    "selected_index": ("Selected index", "Index of the title field or object to change."),
}
DURATION = {
    "Cut": "Transition duration in milliseconds.",
    "Fade": "Transition duration in milliseconds.",
    "SaveVideoDelay": "Length of the clip to save, in milliseconds.",
    "WaitForCompletion": "Duration in milliseconds, as the reference gives it.",
}


def value_text(fn: str, desc: str) -> tuple[str, str]:
    for pattern, text in VALUE_BY_FUNCTION:
        if re.search("^" + pattern if not pattern.startswith("^") else pattern, fn):
            return text
    if "Value = " not in desc:
        raise SystemExit(f"{fn}: no 'Value =' in its description")
    return VALUE[desc.rsplit("Value = ", 1)[1]]


def param_text(fn: str, desc: str, pname: str) -> tuple[str, str]:
    if pname == "value":
        return value_text(fn, desc)
    if pname == "duration" and fn in DURATION:
        return ("Duration", DURATION[fn])
    return OTHER[pname]


def plain(s: str) -> str:
    return s if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ()'.+-]*", s) else yq(s)


def load() -> str:
    if len(sys.argv) > 1:
        return Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    request = urllib.request.Request(URL, headers={"User-Agent": "Mozilla/5.0"})
    return urllib.request.urlopen(request, timeout=30).read().decode("utf-8", "replace")


def snake(name: str) -> str:
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    return s.lower()


def yq(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def cells(row: str) -> list[str]:
    return [html.unescape(re.sub(r"<[^>]+>", "", c)).strip()
            for c in re.findall(r"<t[dh][^>]*>(.*?)</t[dh]>", row, re.S)]


page = load()
functions = []  # (command, function, section, description, params)
section = ""
for row in re.findall(r"<tr[^>]*>(.*?)</tr>", page, re.S)[1:]:
    c = cells(row)
    if len(c) != 3:
        continue
    name, description, params = c
    if not description and not params:
        section = name
        continue
    params = [p.strip() for p in params.split(",") if p.strip() and p.strip() != "None"]
    unknown = [p for p in params if p not in PARAMS]
    if unknown:
        raise SystemExit(f"{name}: unknown parameters {unknown}")
    functions.append((snake(name), name, section, " ".join(description.split()), params))

# Transitions are functions named as in vMix's interface; the page names Cut
# and Fade, and says the rest follow the interface.
extra = [
    ("cut", "Cut", "Transition", "Cut to the input, or from preview to program", ["Input", "Mix"]),
    ("fade", "Fade", "Transition", "Fade to the input, or from preview to program", ["Duration", "Input", "Mix"]),
]
seen = {f[0] for f in functions}
for e in extra:
    if e[0] not in seen:
        functions.append(e)
seen = {}
for cmd, fn, *_ in functions:
    if cmd in seen and seen[cmd] != fn:
        raise SystemExit(f"{fn} and {seen[cmd]} both become {cmd}")
    seen[cmd] = fn

commands = []
for cmd, fn, sec, desc, params in functions:
    summary = f"{fn} ({sec})" + (f": {desc}" if desc else "")
    lines = [f"  {cmd}:", f"    summary: {yq(summary)}"]
    if params:
        lines.append("    params:")
        for p in params:
            pname, ptype = PARAMS[p]
            label, text = param_text(fn, desc, pname)
            ptype = ptype[:-2] + f", label: {plain(label)}, description: {yq(text)} }}"
            lines.append(f"      {pname}: {ptype}")
    lines.append("    returns: ack")
    commands.append("\n".join(lines))
commands.append("""  run_transition:
    summary: "Run a transition by its name in vMix's interface (Fade, Zoom, Wipe, Merge, Stinger1 and so on)"
    params:
      name:     { type: string, pattern: "^[A-Za-z0-9]+$", max_length: 64, required: true, label: Transition, description: "Transition name as in vMix's interface, such as Fade, Zoom, Wipe, Merge or Stinger1." }
      duration: { type: int, min: 0, max: 600000, label: Duration, description: "Transition duration in milliseconds." }
      input:    { type: string, max_length: 256, label: Input, description: "Input by number, exact title (case-sensitive) or key; left out, vMix uses its own default." }
      mix:      { type: int, min: 0, max: 16, label: Mix, description: "Mix as vMix's API numbers it, 0 or left out for the main mix." }
    returns: ack""")

names = [f[0] for f in functions] + ["run_transition"]
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

rows = "\n".join(f'    ("{cmd}", "{fn}"),' for cmd, fn, *_ in sorted(functions))
TABLE.write_text(f'''//! Generated by tools/generate_vmix.py from vMix's Shortcut Function
//! Reference. Do not edit by hand.

/// Command name to vMix function name, sorted by command for binary search.
pub(crate) const FUNCTIONS: &[(&str, &str)] = &[
{rows}
];
''', encoding="utf-8", newline="\n")
# Formatted as `cargo fmt` would, so regenerating leaves no diff to tidy.
subprocess.run(["rustfmt", "--edition", "2021", str(TABLE)], check=True)
print(f"{len(names)} commands")
