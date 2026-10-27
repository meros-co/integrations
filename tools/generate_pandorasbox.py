#!/usr/bin/env python3
"""Generate the commands of specs/christie-pandorasbox.yaml, and the command
table crates/core/src/modules/pandorasbox_commands.rs, from the Pandoras Box
Automation command database (db.json of the PandorasBoxSDK/pbauto-generator
repository, MIT), so every command is one of the SDK's functions with its
code and argument types.

    python tools/generate_pandorasbox.py [path/to/db.json]

Without a path the database is downloaded from GitHub. It is not stored here.
Everything in the spec outside `commands:` and the model's `supports` is kept.

A command is the function's name in snake_case and takes the function's
arguments, in snake_case, in their order; it returns what the function
receives. Labels and descriptions are written here, not taken from the
database.
"""
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "christie-pandorasbox.yaml"
TABLE = ROOT / "crates" / "core" / "src" / "modules" / "pandorasbox_commands.rs"
URL = "https://raw.githubusercontent.com/PandorasBoxSDK/pbauto-generator/HEAD/db.json"

# Argument names whose snake_case would read badly.
RENAME = {
    "playlistdmxFileId": "playlist_dmx_file_id", "newdmxFileId": "new_dmx_file_id",
    "setdmxFileIds": "set_dmx_ids", "pFilename": "filename", "pNote": "note", "Ip": "ip",
    "FontFamily": "font_family", "fileID": "file_id", "folderID": "folder_id",
    "isContentInconsistent": "is_content_inconsistent",
}

# The database's enumerations, by the type id that carries them.
ENUM_TYPES = {"101": "param_kind", "103": "clx_hardware", "104": "consistency", "105": "selection_mode",
              "106": "watch_folder_property", "107": "transport_mode", "108": "sequence_smpte_mode",
              "109": "sequence_smpte_stop_mode"}


def snake(name):
    if name in RENAME:
        return RENAME[name]
    s = re.sub(r"(?<=[a-z0-9])([A-Z])", r"_\1", name)
    s = re.sub(r"(?<=[A-Z])([A-Z][a-z])", r"_\1", s)
    return s.lower()


def words(name):
    s = re.sub(r"(?<=[a-z0-9])([A-Z])", r" \1", name)
    s = re.sub(r"(?<=[A-Z])([A-Z][a-z])", r" \1", s)
    s = s.replace("Dmx", "DMX").replace("Clx", "CLX").replace("SMTPE", "SMPTE").replace("ASIO", "ASIO")
    first, *rest = s.split(" ")
    return " ".join([first] + [w if w.isupper() and len(w) > 1 else w.lower() for w in rest])


# What each argument means, by its snake_case name: (label, description).
TIME = {"hours": "hours", "minutes": "minutes", "seconds": "seconds", "frames": "frames"}


def describe(n):
    fixed = {
        "site_id": ("Site", "The site's ID: a manager, player or server of the project, as the Devices tab shows it."),
        "device_id": ("Device", "The device's ID on its site: a layer, camera, output or DMX device."),
        "source_device_id": ("Source device", "The ID of the device on the same site to assign from."),
        "parameter_name": ("Parameter", "The parameter's name, as Pandoras Box shows it in the device's inspector, such as Opacity or X Position."),
        "parameter_kind_id": ("Parameter kind", "The parameter, by its kind, the faster form of its name."),
        "parameter_value": ("Value", "The parameter's value, in its own units."),
        "do_silent": ("Silent", "Change the value without making the parameter active."),
        "do_direct": ("Direct", "Change the value at once, without the parameter's smoothing."),
        "smoothing_time": ("Smoothing", "How long the parameter fades to a new value, in milliseconds; 0 for none."),
        "tuple_dimension": ("Tuple size", "The number of elements of the tuple."),
        "tuple_data": ("Tuple data", "The tuple's bytes, as hexadecimal digits."),
        "sequence_id": ("Sequence", "The sequence's ID, from 1."),
        "cue_id": ("Cue", "The cue's ID in the sequence."),
        "cue_name": ("Cue name", "The cue's name."),
        "cue_kind_id": ("Cue kind", "The kind of cue, as Pandoras Box numbers it."),
        "cue_btn_id": ("Cue button", "The CLX controller's cue button."),
        "fader_id": ("Fader", "The CLX controller's fader."),
        "transport_mode": ("Transport", "play, stop or pause."),
        "time_code_mode": ("SMPTE mode", "no, send or receive timecode."),
        "stop_action": ("Stop action", "What the sequence does when incoming timecode stops: no, stop, pause or continue."),
        "transparency": ("Transparency", "The sequence's transparency, as Pandoras Box scales it."),
        "is_next": ("Direction", "1 for the next one, 0 for the previous one."),
        "play_mode": ("Play mode", "The cue's play mode, as Pandoras Box numbers it."),
        "do_ignore": ("Ignore", "1 to ignore the next cue, 0 not to."),
        "jump_count": ("Jump count", "How many times the cue jumps."),
        "selection_index": ("Selection index", "The device's place in the selection, from 0."),
        "selection_mode": ("Selection", "set_selection, add_selection, unselect or unselect_all."),
        "group_index": ("Group", "The device group's index."),
        "is_selected": ("Selected", "Whether the device is selected."),
        "dmx_folder_id": ("DMX folder", "The resource's DMX folder ID."),
        "dmx_file_id": ("DMX file", "The resource's DMX file ID."),
        "new_dmx_folder_id": ("New DMX folder", "The DMX folder ID to give the new resource."),
        "new_dmx_file_id": ("New DMX file", "The DMX file ID to give the new resource."),
        "resource_dmx_folder_id": ("Resource DMX folder", "The DMX folder ID of the resource to use."),
        "resource_dmx_file_id": ("Resource DMX file", "The DMX file ID of the resource to use."),
        "canvas_dmx_folder_id": ("Canvas DMX folder", "The canvas's DMX folder ID."),
        "canvas_dmx_file_id": ("Canvas DMX file", "The canvas's DMX file ID."),
        "playlist_dmx_folder_id": ("Playlist DMX folder", "The playlist's DMX folder ID."),
        "playlist_dmx_file_id": ("Playlist DMX file", "The playlist's DMX file ID."),
        "folder_id": ("DMX folder", "The resource's DMX folder ID."),
        "file_id": ("DMX file", "The resource's DMX file ID."),
        "do_set_dmx_ids": ("Set DMX IDs", "Give the new resource the DMX IDs below."),
        "set_dmx_ids": ("Set DMX IDs", "Give the new resource the DMX IDs below."),
        "auto_increment_dmx_id": ("Next free DMX ID", "Use the next free DMX ID when the one given is taken."),
        "for_mesh": ("Mesh", "Assign it as the device's mesh rather than its media."),
        "is_media": ("Media", "Read the media parameter rather than the mesh."),
        "resource_path": ("Resource", "The resource's path in the project, such as Project/Media/Clip.mov."),
        "live_input_resource_path": ("Live input", "The live input's resource path in the project."),
        "canvas_resource_path": ("Canvas", "The canvas's resource path in the project."),
        "playlist_path": ("Playlist", "The playlist's resource path in the project."),
        "project_path": ("Project folder", "A folder of the project, such as Project/Media."),
        "source_project_path": ("Source folder", "The project folder whose content fills the playlist."),
        "file_path": ("File", "A file's path on the computer running the master, or as the command names it."),
        "folder_path": ("Folder", "A folder's path on the computer running the master."),
        "folder_path_to_project": ("Project folder", "The folder holding the project, on the master's computer."),
        "project_xml_file_name": ("Project file", "The project's XML file name."),
        "folder_name": ("Folder name", "The new folder's name."),
        "bundle_path": ("Bundle folder", "The folder the bundle is written to."),
        "bundle_name": ("Bundle name", "The bundle's name."),
        "new_resource_name": ("New name", "The resource's new name."),
        "tree_item_index": ("Project item", "The item's index in the project tree."),
        "tree_items_media_index": ("Media item", "The media's index among the project's media."),
        "canvas_item_index": ("Canvas item", "The canvas's index in the project tree."),
        "playlist_item_index": ("Playlist item", "The playlist's index in the project tree."),
        "folder_item_index": ("Folder item", "The folder's index in the project tree."),
        "source_folder_item_id": ("Source folder item", "The index of the folder whose content fills the playlist."),
        "resource_item_id": ("Resource item", "The resource's index in the project tree."),
        "item_id": ("Item", "The item's index in the project tree."),
        "item_id_from": ("Item", "The index of the item to move."),
        "item_id_to": ("Target item", "The index of the folder item to move it into."),
        "index": ("Index", "The index, from 0."),
        "new_index": ("New index", "The new index, from 0."),
        "playlist_entry_index": ("Entry", "The playlist entry's index, from 0."),
        "all_equally_named": ("All of that name", "Remove every resource of that name."),
        "remove_folder": ("Folders too", "Remove the project's folders as well."),
        "frame_blended": ("Frame blending", "Blend frames."),
        "deinterlacer": ("Deinterlacer", "The deinterlacing method, as Pandoras Box numbers it."),
        "use_filtering": ("Anisotropic filtering", "Use anisotropic filtering."),
        "use_underscan": ("Underscan", "Use underscan."),
        "use_mpeg_color_space": ("MPEG color space", "Use the MPEG color space."),
        "use_alpha_channel": ("Alpha channel", "Use the alpha channel."),
        "text": ("Text", "The text."),
        "is_formatted": ("Formatted", "The text carries formatting."),
        "pending_only": ("Pending only", "Clear only the text not shown yet."),
        "width": ("Width", "The width in pixels."),
        "height": ("Height", "The height in pixels."),
        "set_dims": ("Set size", "Give the canvas the width and height below."),
        "texture_format": ("Texture format", "The texture format, as Pandoras Box numbers it."),
        "size": ("Size", "The font size."),
        "style": ("Style", "The font style, as Pandoras Box numbers it."),
        "alignment": ("Alignment", "The text alignment, as Pandoras Box numbers it."),
        "color_red": ("Red", "The red part of the color, 0-255."),
        "color_green": ("Green", "The green part of the color, 0-255."),
        "color_blue": ("Blue", "The blue part of the color, 0-255."),
        "center_on_texture": ("Center", "1 centers the text on its texture, 0 does not."),
        "font_family": ("Font", "The font family."),
        "save_existing": ("Save current", "1 saves the open project first, 0 does not."),
        "save": ("Save", "1 saves the project first, 0 does not."),
        "accept_dmx": ("Accept DMX", "1 to accept DMX, 0 not to."),
        "id1": ("DMX universe", "The first part of the DMX address."),
        "id2": ("DMX channel", "The second part of the DMX address."),
        "is_fullscreen": ("Fullscreen", "1 for fullscreen, 0 for windowed."),
        "ip_address": ("IP address", "The site's IP address."),
        "ip": ("IP address", "The IP address to give the site."),
        "is_graphic_layer": ("Graphic layer", "A graphic layer rather than a video layer."),
        "layer_id": ("Layer", "The layer's ID on its site."),
        "enable": ("Enabled", "On or off."),
        "view_id": ("View", "The view's ID."),
        "spare_from_spread": ("Spare from spread", "Leave the site out of spreading."),
        "for_jog_shuttle": ("Hardware", "fader_extension or jog_shuttle."),
        "mouse_event_type": ("Mouse event", "The mouse event, as Pandoras Box numbers it."),
        "screen_pos_x": ("X", "The horizontal screen position, in pixels."),
        "screen_pos_y": ("Y", "The vertical screen position, in pixels."),
        "first_pass": ("First pass", "Handle the input in the first pass."),
        "scroll_value": ("Scroll", "The scroll amount."),
        "touch_id": ("Touch", "The touch point's ID."),
        "touch_type": ("Touch event", "The touch event, as Pandoras Box numbers it."),
        "keyboard_event_type": ("Key event", "The key event, as Pandoras Box numbers it."),
        "key_code": ("Key", "The key code."),
        "show_cursor": ("Show cursor", "Show the mouse cursor in fullscreen."),
        "is_master": ("Audio clock master", "The site's node is the audio clock master."),
        "encryption_key": ("Key", "The encryption key."),
        "encryption_policy": ("Policy", "The encryption policy."),
        "enable_input_routing": ("Route to layer", "Route the input to the layer."),
        "enable_input_automation": ("Route to automation", "Route the input to automation."),
        "output_id": ("Output", "The output's ID."),
        "enable_input_picking": ("Picking", "Use the output for picking."),
        "asio_volume": ("Volume", "The ASIO master volume."),
        "fade_fx_id": ("Transition", "The transition's ID."),
        "note": ("Note", "The entry's note."),
        "filename": ("File name", "The file name to record or export to."),
        "encoding_preset_name": ("Encoding preset", "The encoding preset's name."),
        "encoding_preset": ("Encoding preset", "The encoding preset's name."),
        "overwrite_existing": ("Overwrite", "Replace an existing file."),
        "cmd": ("Command", "The canvas drawing command."),
        "cmd_contains_resource_path": ("Has a resource path", "The command names a resource path."),
        "do_add_to_stack": ("Add to stack", "Add the canvas commands to the stack."),
        "sublayer_id": ("Sublayer", "The sublayer's ID."),
        "action": ("Action", "The sublayer action, as Pandoras Box numbers it."),
        "data": ("Data", "The action's data."),
        "fps": ("Frame rate", "Frames per second."),
        "watch_folder_property": ("Property", "include_subdirectories, delete_in_project or delete_in_clients."),
        "in_radians": ("Radians", "Angles in radians rather than degrees."),
        "yaw": ("Yaw", "The yaw angle."),
        "pitch": ("Pitch", "The pitch angle."),
        "roll": ("Roll", "The roll angle."),
        "is_warped": ("Warped", "The soft edge is warped."),
        "forward": ("Forward", "Jump to the next entry; otherwise the previous."),
    }
    if n in fixed:
        return fixed[n]
    m = re.match(r"^(duration|fade_out|start|end)_(hours?|minutes?|seconds?|frames?)$", n)
    if m:
        what = {"duration": "duration", "fade_out": "fade-out time", "start": "start time", "end": "end time"}[m.group(1)]
        part = TIME.get(m.group(2), TIME.get(m.group(2) + "s", m.group(2)))
        return (f"{what.capitalize()} {part}", f"The {what}'s {part}.")
    if n in TIME:
        return (n.capitalize(), f"The time's {n}.")
    raise SystemExit(f"no description for argument {n!r}")


def main():
    db = json.loads(Path(sys.argv[1]).read_text("utf-8") if len(sys.argv) > 1
                    else urllib.request.urlopen(URL, timeout=30).read().decode("utf-8"))
    types, chunks = db["types"], db["chunks"]
    enums = {e["name"]: [(v["key"], v["val"]) for v in e["values"]] for e in db["enums"]}

    # One function per code: the database repeats a few codes, the later
    # entry with its arguments.
    by_code = {}
    for c in db["commands"]:
        if c["code"] not in by_code or len(c["send"]) >= len(by_code[c["code"]]["send"]):
            by_code[c["code"]] = c
    funcs = sorted(by_code.values(), key=lambda c: c["code"])
    names = {}
    for c in funcs:
        n = snake(c["name"])
        if n in names.values():
            # Two functions of one name (EncodeFileByDmxId at 230 and 231):
            # the later one writes to a project path.
            n = n + "_to_path"
        names[c["code"]] = n

    def arg(cid):
        ch = chunks[str(cid)]
        tid = ch["type_id"]
        return snake(ch["name"]), tid

    spec_cmds, rust_cmds = [], []
    used_enums = set()
    for c in funcs:
        name = names[c["code"]]
        params = []
        seen = set()
        for cid in c["send"]:
            n, tid = arg(cid)
            assert n not in seen, (c["name"], n)
            seen.add(n)
            label, desc = describe(n)
            t = types[tid]["name"]
            if tid in ENUM_TYPES:
                used_enums.add(ENUM_TYPES[tid])
                vals = [k for k, _ in enums[ENUM_TYPES[tid]]]
                p = f"{{ type: enum, values: [{', '.join(json.dumps(v) for v in vals)}], required: true"
            elif t == "bool":
                default = ", default: false" if n in ("do_silent", "do_direct") else ", required: true"
                p = "{ type: bool" + default
            elif t == "byte":
                p = "{ type: int, min: 0, max: 255, required: true"
            elif t == "short":
                p = "{ type: int, min: -32768, max: 32767, required: true"
            elif t == "int":
                lo = 0 if re.search(r"(hours?|minutes?|seconds?|frames?|width|height|index|_id|id[12])$", n) else -2147483648
                p = f"{{ type: int, min: {lo}, max: 2147483647, required: true"
            elif t == "double":
                p = "{ type: float, required: true"
            elif t == "string_narrow":
                p = "{ type: string, pattern: '^[\\x20-\\x7E]*$', max_length: 32767, required: true"
            elif t == "string_wide":
                p = "{ type: string, max_length: 32767, required: true"
            elif t == "byte_buffer":
                p = "{ type: string, pattern: '^([0-9A-Fa-f]{2})*$', max_length: 65534, required: true"
            else:
                raise SystemExit(f"{c['name']}: cannot send {t}")
            p += f", label: {json.dumps(label)}, description: {json.dumps(desc)} }}"
            params.append(f"      {n}: {p}")
        returns = "value" if c["recv"] else "ack"
        summary = f"{words(c['name'])} (code {c['code']})"
        if not c["recv"]:
            summary += "; Pandoras Box does not answer it"
        block = f"  {name}:\n    summary: {json.dumps(summary)}\n"
        if params:
            block += "    params:\n" + "\n".join(params) + "\n"
        block += f"    returns: {returns}"
        spec_cmds.append(block)

        def rust_args(ids):
            out = []
            for cid in ids:
                n, tid = arg(cid)
                t = types[tid]["name"]
                if tid in ENUM_TYPES:
                    used_enums.add(ENUM_TYPES[tid])
                    ty = ("ByteEnum" if t == "byte" else "IntEnum") + f"({ENUM_TYPES[tid].upper()})"
                else:
                    ty = {"byte": "Byte", "short": "Short", "int": "Int", "double": "Double",
                          "string_narrow": "Narrow", "string_wide": "Wide", "bool": "Bool", "int_buffer": "IntBuffer",
                          "byte_buffer": "ByteBuffer"}[t]
                out.append(f'Arg {{ name: "{n}", ty: Ty::{ty} }}')
            return "&[" + ", ".join(out) + "]"

        rust_cmds.append(f'    Cmd {{ name: "{name}", code: {c["code"]}, send: {rust_args(c["send"])}, '
                         f'recv: {rust_args(c["recv"])} }},')

    enum_consts = []
    for e in sorted(used_enums):
        vals = ", ".join(f'("{k}", {v})' for k, v in enums[e])
        enum_consts.append(f"pub(crate) const {e.upper()}: &[(&str, i32)] = &[{vals}];")

    TABLE.write_text(f'''//! Generated by tools/generate_pandorasbox.py from the Pandoras Box
//! Automation command database (PandorasBoxSDK/pbauto-generator, db.json,
//! revision {db["revision"]}, MIT). Do not edit: run the generator.

use super::christie_pandorasbox::{{Arg, Cmd, Ty}};

{chr(10).join(enum_consts)}

pub(crate) const COMMANDS: &[Cmd] = &[
{chr(10).join(rust_cmds)}
];
''', encoding="utf-8", newline="\n")
    subprocess.run(["rustfmt", "--edition", "2021", str(TABLE)], check=True)

    spec = SPEC.read_text("utf-8")
    head, rest = spec.split("\ncommands:\n", 1)
    tail = rest[rest.index("\nstate:\n"):]
    supports = ", ".join(names[c["code"]] for c in funcs)
    head = re.sub(r"(    supports: )\[[^\]]*\]", lambda m: m.group(1) + "[" + supports + "]", head, flags=re.S)
    SPEC.write_text(head + "\ncommands:\n" + "\n".join(spec_cmds) + "\n" + tail, encoding="utf-8", newline="\n")
    print(f"{len(funcs)} commands")


main()
