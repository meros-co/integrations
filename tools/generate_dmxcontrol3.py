#!/usr/bin/env python3
"""Generate DMXControl 3's protobuf schema, the commands and state of
specs/dmxcontrol3.yaml, and the module's tables, from the .proto files of
DMXControl's official network interface repository.

    python tools/generate_dmxcontrol3.py path/to/DMXControl3-Network-Interface
    cargo fmt --all

The repository (github.com/DMXControl/DMXControl3-Network-Interface, one
commit per DMXControl release) is read as documentation: the field names,
numbers and types of its messages and the methods of its services are
protocol facts, written here into tables of our own. No file of it is
copied or stored in this repository.

It writes:
- crates/core/src/modules/dmxcontrol3_schema.rs: every message and enum, as
  `crate::protobuf` tables; every method a command calls; and the state
  collections the module keeps current;
- the `commands:` and `state:` sections of specs/dmxcontrol3.yaml and the
  model's `supports`, keeping everything else in the spec as written.

Naming is mechanical, and the module relies on it: a command is its method
in snake_case (GetCuelists -> get_cuelists) unless RENAMES names it, a
parameter is its request field in snake_case (cuelistId -> cuelist_id), and
JSON keys in values and state are field names in snake_case.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "dmxcontrol3.yaml"
SCHEMA = ROOT / "crates" / "core" / "src" / "modules" / "dmxcontrol3_schema.rs"

# ---------------------------------------------------------------------------
# A proto3 parser for what the files use: packages, imports, options,
# messages (nested), enums, oneofs, maps, repeated fields, and services with
# streaming methods.

TOKEN = re.compile(r'//[^\n]*|/\*.*?\*/|"(?:[^"\\]|\\.)*"|[A-Za-z_.][A-Za-z0-9_.]*|-?\d+|[{}()<>;=,\[\]]', re.S)
SCALARS = {"double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32",
           "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes"}


class Protos:
    def __init__(self):
        self.messages = {}
        self.enums = {}
        self.services = {}

    def parse(self, path):
        text = Path(path).read_text(encoding="utf-8-sig")
        self.t = [m.group(0) for m in TOKEN.finditer(text) if not m.group(0).startswith(("//", "/*"))]
        self.i = 0
        package = ""
        while self.i < len(self.t):
            t = self.next()
            if t == "package":
                package = self.next()
                self.expect(";")
            elif t in ("syntax", "import", "option"):
                self.skip(";")
            elif t == "message":
                self.message(package)
            elif t == "enum":
                self.enum(package)
            elif t == "service":
                self.service(package)
            elif t != ";":
                raise SyntaxError(f"{path}: unexpected {t}")

    def next(self):
        self.i += 1
        return self.t[self.i - 1]

    def peek(self):
        return self.t[self.i]

    def expect(self, want):
        got = self.next()
        if got != want:
            raise SyntaxError(f"expected {want}, got {got}")

    def skip(self, end):
        while self.next() != end:
            pass

    def enum(self, scope):
        full = f"{scope}.{self.next()}"
        self.expect("{")
        values = []
        while self.peek() != "}":
            t = self.next()
            if t == "option":
                self.skip(";")
            elif t != ";":
                self.expect("=")
                values.append((t, int(self.next())))
                if self.peek() == "[":
                    self.skip("]")
                self.expect(";")
        self.next()
        self.enums[full] = values

    def message(self, scope):
        full = f"{scope}.{self.next()}"
        self.expect("{")
        fields = []
        self.messages[full] = fields
        oneof = None
        while True:
            t = self.next()
            if t == "}":
                if oneof:
                    oneof = None
                    continue
                return
            if t == ";":
                continue
            if t == "message":
                self.message(full)
            elif t == "enum":
                self.enum(full)
            elif t in ("reserved", "option"):
                self.skip(";")
            elif t == "oneof":
                oneof = self.next()
                self.expect("{")
            else:
                label = "single"
                if t in ("repeated", "optional"):
                    label = "repeated" if t == "repeated" else "single"
                    t = self.next()
                key = None
                if t == "map":
                    self.expect("<")
                    key = self.next()
                    self.expect(",")
                    t = self.next()
                    self.expect(">")
                    label = "map"
                name = self.next()
                self.expect("=")
                number = int(self.next())
                if self.peek() == "[":
                    self.skip("]")
                self.expect(";")
                fields.append({"name": name, "number": number, "type": t, "label": label, "key": key, "oneof": oneof})

    def streamed(self):
        t = self.next()
        if t == "stream":
            return True, self.next()
        if t.startswith("stream."):
            return True, t[len("stream"):]
        return False, t

    def service(self, scope):
        full = f"{scope}.{self.next()}"
        self.expect("{")
        methods = []
        while self.peek() != "}":
            t = self.next()
            if t == "option":
                self.skip(";")
            elif t == "rpc":
                name = self.next()
                self.expect("(")
                cs, tin = self.streamed()
                self.expect(")")
                self.expect("returns")
                self.expect("(")
                ss, tout = self.streamed()
                self.expect(")")
                if self.peek() == "{":
                    self.skip("}")
                methods.append([name, tin, tout, cs, ss])
        self.next()
        self.services[full] = methods

    def resolve(self, name, scope):
        if name in SCALARS:
            return name
        if name.startswith("."):
            if name[1:] in self.messages or name[1:] in self.enums:
                return name[1:]
            raise KeyError(name)
        parts = scope.split(".")
        while True:
            cand = ".".join(parts + [name])
            if cand in self.messages or cand in self.enums:
                return cand
            if not parts:
                raise KeyError(f"{name} in {scope}")
            parts.pop()

    def link(self):
        for full, fields in self.messages.items():
            for f in fields:
                f["type"] = self.resolve(f["type"], full)
        for full, methods in self.services.items():
            package = full.rsplit(".", 1)[0]
            for m in methods:
                m[1] = self.resolve(m[1], package)
                m[2] = self.resolve(m[2], package)


def snake(name: str) -> str:
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name)
    s = re.sub(r"(?<=[A-Z])(?=[A-Z][a-z])", "_", s)
    return re.sub(r"_+", "_", s).lower().strip("_")


def short(full: str) -> str:
    return full.rsplit(".", 1)[-1]


# ---------------------------------------------------------------------------
# Which methods are commands.

# Services a client calls on Umbra. ResourceService and the Server package
# are the Kernel's side.
CLIENT_SERVICES = ("LumosProtobufClient.", "LumosProtobufCommon.ResourceClient",
                   "LumosProtobufCommon.ConnectedClientService", "LumosProtobufCommon.DMXCNetService")

# Methods the module runs itself: the session (login, ready, ping, logoff),
# and the change streams that keep state current.
MANAGED = {
    "ConnectedClientService.ReportReadyToWork", "ConnectedClientService.Ping",
    "ConnectedClientService.SendBroadcast", "ConnectedClientService.ProcessClientControls",
    "CuelistClient.ReceiveCuelistProgressChanges", "DeviceClient.ReceiveDevicePropertyChanges",
    "DMXClient.GetDmxChanges", "InputClient.ReceiveConnectionSetProcessedMessages",
    # A GUI's: Umbra asks the clients that open these to confirm a project
    # change and waits for their answers.
    "ProjectClient.ReceiveProjectChangeConfirmationRequests",
    "ProjectClient.ReceiveProjectChangeInformationRequests",
    "ProjectClient.ReceiveProjectDirtyRequests", "ProjectClient.ReceiveProjectWarningRequests",
    # A monitor of every message between the programs, for debugging.
    "MonitoringClient.ReceiveTransferMessages",
}

RENAMES = {
    "AttachableClient.CanAttachTo": "can_attach_attachable_to",
    "PresetClient.CanAttachTo": "can_attach_preset_to",
    "UserClient.Bind": "bind_user_context",
    "UserClient.Destroy": "destroy_user_context",
    "RDMClient.Discovery": "rdm_discovery",
    "RDMClient.RequestDeviceInfo": "rdm_request_device_info",
    "RDMClient.RequestSensors": "rdm_request_sensors",
    "RDMClient.RequestParameters": "rdm_request_parameters",
    "ProgrammerClient.GetSnapshots": "get_programmer_snapshots",
    "DMXInterfaceManagementClient.GetComment": "get_dmx_interface_comment",
    "DMXInterfaceManagementClient.SetComment": "set_dmx_interface_comment",
    "DMXInterfaceManagementClient.GetAvailableInterfaces": "get_available_dmx_interfaces",
    "DMXInterfaceManagementClient.GetUsedInterfaces": "get_used_dmx_interfaces",
    "DMXInterfaceManagementClient.GetInterfaceEnabled": "get_dmx_interface_enabled",
    "DMXInterfaceManagementClient.SetInterfaceEnabled": "set_dmx_interface_enabled",
    "DMXInterfaceManagementClient.GetInterfaceAddress": "get_dmx_interface_address",
    "DMXInterfaceManagementClient.SetInterfaceAddress": "set_dmx_interface_address",
    "DMXInterfaceManagementClient.IsInterfaceAvailable": "is_dmx_interface_available",
    "DMXInterfaceManagementClient.AddInterface": "add_dmx_interface",
    "DMXInterfaceManagementClient.DeleteInterface": "delete_dmx_interface",
    "DMXInterfaceManagementClient.GetInterfaceParameters": "get_dmx_interface_parameters",
    "DMXInterfaceManagementClient.GetInterfaceParameter": "get_dmx_interface_parameter",
    "DMXInterfaceManagementClient.SetInterfaceParameter": "set_dmx_interface_parameter",
    "DMXInterfaceManagementClient.TestInterfaceParameter": "test_dmx_interface_parameter",
    "ResourceClient.ExistsResource": "resource_exists",
    "ResourceClient.StreamData": "stream_resource_data",
    "ResourceClient.RequestAllResources": "get_resources",
    "DMXCNetService.SetClientParameter": "set_client_network_property",
    "DMXCNetService.InformClientExists": "inform_client_exists",
    "ConnectedClientService.ControlClient": "control_client",
    "DMXClient.SetDMXValues": "set_dmx_values",
    "DMXClient.ClearSetDMXValues": "clear_dmx_values",
    "DMXClient.GetDmxValues": "get_dmx_values",
    "DMXClient.GetDMXIn2OutMapping": "get_dmx_in2out_mapping",
    "DMXClient.SetDMXIn2OutMapping": "set_dmx_in2out_mapping",
    "MonitoringClient.GetDMXStatistics": "get_dmx_statistics",
    "MonitoringClient.PerformEnd2EndLatency": "perform_end_to_end_latency",
    "DeviceClient.GetSetGlobalFunction": "global_function",
    "LogClient.GetLogData": "get_log_data",
}

# ---------------------------------------------------------------------------
# What each command does: one line each.

SUMMARIES = {
    # Affinity
    "get_affinity_classes": "List the affinity classes: the kinds of objects (such as colors or gobos) DMXControl keeps a preference order for",
    "get_affinity_class_reference_list": "List the objects of one affinity class that an affinity list can refer to",
    "get_affinity_list": "Read an affinity list: an affinity class's objects in order, with their affinity values, for one reference object",
    "set_user_defined_affinity_list": "Use an item list as the user-defined affinity list of a reference object of an affinity class",
    "get_user_defined_affinity_lists": "List the reference objects of an affinity class that have a user-defined affinity list, and those lists",
    # Attachables (effects and other things attached to a property)
    "get_attachables": "List the attachables (effects, generators and other items attachable to a device property), or those whose IDs are given",
    "can_attach_attachable_to": "Ask which properties of a device or group an attachable can be attached to",
    "modify_attachable_preset": "Add, delete or change a saved attachable preset (an attachable's parameter set)",
    "get_attachable_preset": "Read a saved attachable preset's parameters",
    "get_effect_visualization": "Compute the visualization of an effect over time, for a new effect or one in a programmer",
    "get_effect_visualization_vectors": "Compute an effect's visualization vectors for one property in a programmer",
    # Audio
    "get_player_threads": "List the audio players and their playback state",
    "request_all_audio_metadata": "List the audio files' metadata (duration, channels, sample rate, waveform and spectrum analyses)",
    "request_all_sound_output_infos": "List the sound outputs (audio devices) an audio player can play on",
    "control_player_thread": "Play, stop, pause or buffer an audio file on a player, and set its volume, balance, position and sound output",
    # Background tasks
    "manage_background_task": "Pause or continue a background task",
    # Beat
    "get_manual_beat": "Read the manual beat's interval and whether it is on",
    "set_manual_beat": "Set the manual beat's interval and turn it on or off",
    "get_manual_beat_factor": "Read the manual beat's factor",
    "set_manual_beat_factor": "Set the manual beat's factor, which multiplies its speed",
    "learn_manual_beat": "Tap the manual beat: each tap's time teaches it the interval",
    "sync_manual_beat": "Restart the manual beat's count at the given time",
    # Command line
    "get_current_command_line": "Read a user context's current command line",
    "get_command_line_history": "Read a user context's command line history",
    "execute_or_modify_command_line_command": "Replace, extend or run the command line of a user context, as typing at DMXControl's command line does; the answer holds the result, an error or completions",
    # Cues
    "set_cue_value": "Change a cue: its name, enabled, auto-prepare, trigger, comment, take fades, number, or its delay and fade times (one per call)",
    "set_cue_entry_value": "Change a cue entry's delay or fade, a device's own delay or fade in it, or clear one of them (one per call)",
    "get_cue_states": "Read the values, delays or fades a cue sets, by group and device",
    "delete_cue": "Delete a cue from its cuelist",
    "move_cue": "Move a cue to another place in its cuelist",
    "copy_cue": "Copy a cue to a place in the same or another cuelist",
    "get_cue_triggers": "List the cue triggers a cue can wait for (such as a time, a timecode or a beat), with the format each takes",
    # Cuelists
    "get_cuelists": "List the cuelists with their cues, or those whose IDs are given",
    "create_cuelist": "Create a cuelist, empty or as a copy of another",
    "delete_cuelist": "Delete a cuelist; when something depends on it, the answer lists the dependencies to resolve and send back",
    "set_cuelist_value": "Set a cuelist's intensity, fade factor, speed factor or temporary fader (one per call)",
    "cuelist_action": "Play, pause, stop, go, go back, go to a cue, go to the next cue, load a cue or renumber the cues of a cuelist",
    "insert_special_cue": "Insert a special cue into a cuelist: a preset, audio, another cuelist, a sum or psum, a macro or a timecode",
    "add_programmer_cue": "Store the programmer into a cuelist as a cue: added, inserted, replacing, merged, appended or subtracted",
    "get_property_value_sources": "List where the values of a cue in a cuelist come from (presets, other cues)",
    "get_cuelist_progress": "Read a cuelist's state and the progress of its running cues",
    "all_cuelist_action": "Stop every cuelist",
    # Cuelist groups
    "get_cuelist_groups": "List the cuelist groups, or those whose IDs are given",
    "create_cuelist_group": "Create a cuelist group, empty or as a copy of another",
    "delete_cuelist_group": "Delete a cuelist group; when something depends on it, the answer lists the dependencies",
    "modify_cuelist_group": "Add cuelists to a cuelist group, remove them, or move them within it",
    # DMX
    "get_dmx_values": "Read the DMX output and input values of a range of channels",
    "set_dmx_values": "Set DMX channels to values directly, as a named source that overrides the output",
    "clear_dmx_values": "Clear the values a named source set with set_dmx_values",
    "get_dmx_in2out_mapping": "Read the mapping of DMX input channels to output channels",
    "set_dmx_in2out_mapping": "Set the mapping of DMX input channels to output channels",
    # DMX interfaces
    "get_available_dmx_interfaces": "List the DMX interfaces DMXControl can use (found or configured), with their ports",
    "get_used_dmx_interfaces": "List the DMX interfaces the project uses, with their ports and status",
    "get_dmx_interface_enabled": "Read whether a DMX interface and each of its ports are enabled",
    "set_dmx_interface_enabled": "Enable or disable a DMX interface, or one of its input or output ports",
    "get_dmx_interface_address": "Read the universe address of a DMX interface's port",
    "set_dmx_interface_address": "Set the universe address of a DMX interface's port",
    "get_dmx_interface_comment": "Read the comment on a DMX interface's port",
    "set_dmx_interface_comment": "Set the comment on a DMX interface's port",
    "is_dmx_interface_available": "Ask whether a DMX interface is connected and available",
    "add_dmx_interface": "Add a DMX interface to the project",
    "delete_dmx_interface": "Remove a DMX interface from the project",
    "get_dmx_interface_parameters": "List a DMX interface's configuration parameters",
    "get_dmx_interface_parameter": "Read one configuration parameter of a DMX interface",
    "set_dmx_interface_parameter": "Set one configuration parameter of a DMX interface",
    "test_dmx_interface_parameter": "Check whether a value is valid for a DMX interface's parameter, without setting it",
    # Devices
    "get_device_property_current_value": "Read a device's or group's property value: its output, its value, or the programmer's",
    "get_device_properties_current_values": "Read several device or group property values at once",
    "get_descriptor": "Read the descriptor of a device property, subproperty, group or property type: its type, bounds and values",
    "get_available_devices": "List the device definitions (DDFs) in DMXControl's library",
    "get_devices": "List the devices with their beams, properties and procedures, or those whose IDs are given",
    "create_devices": "Patch new devices from a device definition, at a DMX start address, into a new or existing group",
    "create_proxy_device": "Create a proxy device, which takes values from DMXControl without a DMX patch of its own",
    "replace_device": "Replace a device by another, keeping what refers to it",
    "delete_device": "Delete a device; when something depends on it, the answer lists the dependencies",
    "get_device_groups": "List the device groups with their devices and properties, or those whose IDs are given",
    "create_device_group": "Create a device group, empty or as a copy of another",
    "get_temporary_device_group": "Make a temporary device group of the given devices and groups, for a user context",
    "delete_device_group": "Delete a device group; when something depends on it, the answer lists the dependencies",
    "modify_device_group": "Add devices or groups to a device group, remove them, or move them within it",
    "global_function": "Read or set a global function: blackout, freeze, fog state or highlight",
    "execute_procedure": "Run a device procedure (such as lamp on, lamp off or reset) on devices or groups",
    # Electricity
    "get_power_sources": "List the power sources (circuits) with their consumers, or those whose IDs are given",
    "get_power_source_usages": "Read the current drawn from each power source, in amperes",
    "create_power_source": "Create a power source",
    "delete_power_source": "Delete a power source",
    "modify_power_source": "Add or remove sub-sources and consumers (devices) of a power source",
    # Executors
    "get_executors": "List the executors with their faders and buttons, or those whose IDs are given",
    "get_executor_pages": "List the executor pages, or those whose IDs are given",
    "get_executor_member_wrappers": "List the objects that can be put on an executor (cuelists, masters and others)",
    "create_executor_page": "Create an executor page, empty or as a copy of another",
    "delete_executor_page": "Delete an executor page",
    "modify_executor": "Put an object on an executor, or remove the one it holds",
    "set_executor_values": "Press or release an executor's buttons and move its fader, absolutely or by an increment",
    # Fanned values
    "get_fanned_values": "List the fanning containers, operators or container types a fanned value can use",
    "is_valid_parametered_fanned_value": "Check whether a fanned property value is valid",
    # Input
    "create_bank": "Create an input bank, empty or as a copy of another",
    "delete_bank": "Delete an input bank",
    "create_connection_set": "Create an input connection set (a graph linking input sources to sinks), in a bank",
    "delete_connection_set": "Delete input connection sets",
    "move_connection_set": "Move input connection sets to another bank",
    "modify_connection_set": "Edit an input connection set: create, delete or copy nodes, and create or delete edges between ports",
    "create_or_replace_port": "Add a source or sink node to a connection set, or replace one",
    "create_or_update_or_delete_sink_proxy": "Add, change or remove an input sink this client provides (a proxy for an output it handles)",
    "create_or_update_or_delete_source_proxy": "Add, change or remove an input source this client provides, or send its value",
    "get_available_nodes": "List the node types a connection set can use",
    "get_input_class_providers": "List the input class providers and the sources and sinks they offer",
    "get_banks": "List the input banks, or those whose IDs are given",
    "get_connection_sets": "List the input connection sets with their nodes, or those whose IDs are given",
    "get_input_sinks": "List the input sinks (what inputs can control), or those whose IDs are given",
    "get_input_sources": "List the input sources (MIDI, DMX in, OSC and other inputs), or those whose IDs are given",
    "get_connection_set_preview_values": "Read the current values on a connection set's ports",
    # Item lists
    "get_item_lists": "List the item lists, or those whose IDs are given",
    "get_available_item_list_types": "List the kinds of item list that can be created",
    "get_possible_items": "List the items an item list type can hold",
    "create_item_list": "Create an item list of a type, empty or as a copy of another",
    "delete_item_list": "Delete an item list",
    "modify_item_list": "Add, remove, move or clear the items of an item list, or redefine it",
    # Log
    "get_log_data": "Read the most recent log entries of a kernel",
    # Macro boards
    "get_macro_boards": "List the macro boards (button grids such as a Stream Deck), or those whose IDs are given",
    "get_macro_board_profiles": "List the macro board profiles with their buttons, or those whose IDs are given",
    "get_macro_board_profile_templates": "List the macro board profile templates",
    "create_macro_board_profile": "Create a macro board profile, as a copy, from a template, for a board or of a size",
    "delete_macro_board_profile": "Delete a macro board profile",
    "change_macro_board_profile": "Rename or renumber a macro board profile, or press and release its buttons",
    "create_or_update_or_delete_macro_board_proxy": "Add, change or remove a macro board this client provides, or send its button presses",
    # Macros
    "get_macros": "List the macros with their faders and buttons, or those whose IDs are given",
    "create_macro": "Create a macro",
    "set_macro_button_state": "Press or release a macro's button",
    "set_macro_fader_state": "Move a macro's fader, absolutely or by an increment",
    "delete_macro": "Delete a macro",
    # Main switches
    "get_main_switches": "List the main switches and whether each is on",
    "set_main_switch": "Turn a main switch on or off, or toggle it",
    # Masters
    "get_masters": "List the masters (grand, group, device, special, parameter, color, position and speed masters) with their values",
    "master_action": "Run a master's action: double or halve its time, learn, sync, full or black",
    "set_master_value": "Move a master's fader or press its button",
    "create_master": "Create a master of a type, or a copy of another",
    "delete_master": "Delete a master; when something depends on it, the answer lists the dependencies",
    # Monitoring
    "get_monitor_slots": "Read the monitoring slots: DMXControl's performance counters",
    "get_dmx_statistics": "Read how many DMX values per second DMXControl sends, and its DMX frame rate",
    "get_transfer_statistics": "Read the network traffic between DMXControl's programs, by service",
    "perform_end_to_end_latency": "Send test data through Umbra and the kernel and back, to measure latency",
    # Parameters
    "get_parameters": "List the parameters of an object (a device, an interface, a plugin), with their values when asked",
    "get_parameter_value": "Read one parameter of an object",
    "set_parameter_value": "Set one parameter of an object",
    "test_parameter_value": "Check whether a value is valid for an object's parameter, without setting it",
    # Plugins
    "get_plugins": "List the plugins, or those whose IDs are given",
    "is_plugin_enabled": "Read whether a plugin is enabled",
    "set_plugin_enabled": "Enable or disable a plugin",
    # Position mapping
    "start_preview_position_mapping": "Start previewing positions and intensities on devices, for a user context",
    "stop_preview_position_mapping": "Stop the position preview of a user context",
    "set_preview_position_mapping_intensity": "Set the intensities of devices in the position preview",
    "set_preview_position_mapping": "Set the positions of devices in the position preview",
    "get_status_position_mapping": "Read whether a user context's position preview is on, and its devices",
    # Presets
    "get_presets": "List the presets with their entries, or those whose IDs are given",
    "can_attach_preset_to": "Ask which properties of a device or group a preset can be applied to",
    "invoke_preset": "Apply a preset to a device or group (in the programmer of a user context)",
    "edit_preset_in_programmer": "Load a preset into a user context's programmer for editing, live or blind",
    "delete_preset": "Delete a preset; when something depends on it, the answer lists the dependencies",
    "create_preset": "Create a preset from the programmer, or as a copy of another",
    "merge_append_programmer": "Merge or append the programmer into a preset",
    "replace_subtract_programmer": "Replace a preset's content with the programmer, or subtract the programmer from it",
    # Programmer
    "get_programmer_value": "Read a property value in a user context's programmer",
    "set_programmer_value": "Set a property value of a device or group in a user context's programmer",
    "set_programmer_attachable": "Attach an attachable (an effect) to a property in the programmer",
    "delete_programmer_attachable": "Remove an attachable from the programmer",
    "delete_programmer_value": "Delete, clear or undo values in the programmer, for devices, groups, entries or properties",
    "get_programmer_state": "Read what a user context's programmer (or a cue) holds: values, delays or fades, by group and device",
    "get_attachable_state": "Read which properties of the given devices have attachables in the programmer",
    "set_programmer_output": "Set what the programmer outputs: all, filtered by a predicate, or hidden",
    "edit_cue_in_programmer": "Load a cue into a user context's programmer for editing, live or blind",
    "mod_programmer_snapshot": "Write, load or clear a programmer snapshot",
    "get_programmer_snapshots": "List a user context's programmer snapshots",
    "copy_programmer_values": "Copy programmer values from one user context or group to another",
    "get_programmer_property": "Read a property of a user context's programmer",
    "update_origin_of_programmer": "Write the programmer's values back to where they came from (presets, cues)",
    # Projects
    "get_projects": "List the projects and their versions, and the version loaded",
    "create_project": "Create a project, or a copy of a project version",
    "delete_project": "Delete a project version, or every version of a project",
    "modify_project": "Change a project version's name, description, author, delete lock or favorite, and its tags",
    "import_project": "Import a project file that is already in Umbra's temporary resources; the answer is every progress message up to the result",
    "export_project": "Export a project version to a file in Umbra's temporary resources; the answer is every progress message up to the file name",
    "get_project_todos": "List the project's to-dos, or those whose IDs are given",
    "create_project_todo": "Create a project to-do",
    "modify_project_todo": "Change a project to-do: name, text, category, priority or done (one per call)",
    "delete_project_todo": "Delete project to-dos",
    "get_project_state": "Read whether a project is loaded, closing or closed, and its file version",
    "get_project_dirty": "Ask every program whether the project has unsaved changes, and where",
    "get_project_warning": "Ask every program whether the project has warnings, and where",
    "close_project_version": "Close the loaded project; the answer is every status message up to the end",
    "load_project_version": "Load a project version; the answer is every status message up to the end",
    "save_project_version": "Save the loaded project as a new version; the answer is every status message up to the end",
    # RDM
    "get_rdm_devices": "List the RDM devices found, with their information and sensors",
    "rdm_discovery": "Start an RDM discovery",
    "rdm_request_device_info": "Ask an RDM device for its information",
    "rdm_request_sensors": "Ask an RDM device for its sensor values",
    "rdm_request_parameters": "Ask an RDM device for its parameters",
    # Settings
    "get_default_configuration": "Read the default configurations of a name, for the application or the project",
    "get_registered_settings": "List the registered settings with their descriptions, ranges and values",
    "register_setting": "Register a new setting with its default value",
    "get_setting": "Read a setting's value",
    "get_setting_type": "Read a setting's value type",
    "get_dynamic_enum_values": "List the values a setting with a dynamic list of choices can take",
    "set_setting": "Set a setting's value",
    "reset_application_settings": "Reset the application settings to their defaults",
    # Stage views
    "get_stage_views": "List the stage views with the devices and groups on them, or those whose IDs are given",
    "create_stage_view": "Create a stage view, empty or as a copy of another",
    "delete_stage_view": "Delete a stage view",
    "modify_stage_view": "Change a stage view: move it, set its transformation, or add, remove, place and size its devices and groups",
    # Timecode
    "get_timecodes": "List the timecode shows, or those whose IDs are given",
    "timecode_action": "Play, stop, pause, record or clear a timecode show",
    "create_timecode": "Create a timecode show, empty or as a copy of another",
    "delete_timecode": "Delete a timecode show",
    "get_tracks": "List the timecode tracks with their actions, or those whose IDs are given",
    "create_track": "Create a track in a timecode show",
    "delete_track": "Delete a timecode track",
    "add_children_to_track": "Add children (cuelists, executors, sounds) to a timecode track at a time",
    "edit_track": "Add, change, remove, copy, cut or paste a timecode track's actions",
    "get_markers": "List the timecode markers, or those whose IDs are given",
    "create_marker": "Create a marker in a timecode show",
    "delete_marker": "Delete a timecode marker",
    "edit_marker": "Change a timecode marker's parameter",
    "get_snips": "List the sound snips on timecode tracks, or those whose IDs are given",
    "create_snip": "Create a sound snip on a timecode track",
    "delete_snip": "Delete a sound snip",
    # Users
    "get_user_context": "Read a user context: its selected devices, groups, executor page, executor, timecode and track",
    "bind_user_context": "Get a user context for a user name and password hash, for commands that act in one",
    "destroy_user_context": "End a user context",
    "select_devices_and_groups": "Select devices and groups in a user context",
    "select_executor_page": "Select the executor page of a user context",
    "select_executor": "Select an executor in a user context",
    "fix_executors": "Fix executors in a user context so they stay when the page changes, or unfix them",
    "select_timecode": "Select a timecode show in a user context",
    "select_track": "Select a timecode track in a user context",
    # Umbra
    "get_client_program_info": "Read a program's information (name, version, state) by its runtime ID",
    "control_client": "Ask another program in the DMXControl network to shut down, show or hide itself",
    "inform_client_exists": "Tell Umbra this client exists; the answer may ask it to change its network ID or name",
    "set_client_network_property": "Set another program's network ID or client name",
    "get_umbra_network_info": "Read Umbra's information and the programs connected to it and known to it",
    # Resources
    "get_resources": "List the resources (icons, device images, pictures, device definitions, symbols, sounds, logs, files) of a kind",
    "resource_exists": "Ask whether a resource exists",
    "load_resource": "Read a resource: its data, or the ID of a stream to read it with stream_resource_data",
    "save_resource": "Write a resource: its data, a stream's ID, or a managed tree; or remove its reference from the project",
    "stream_resource_data": "Read or write a block of a resource stream that load_resource or save_resource opened",
}

# ---------------------------------------------------------------------------
# Parameters: labels and one-sentence descriptions, by field name, or by
# (request message, field) where a name means different things.

CONTEXT = ("The user context (one operator's selection, programmer and pages) the command acts in, "
           "as bind_user_context returns it; empty for the one Umbra assigns.")

FIELDS = {
    "absolut": ("Position", "The fader's new position, absolute; give this or an increment."),
    "action": ("Action", "What to do."),
    "active": ("Active", "Whether the button is pressed (true) or released (false)."),
    "addFeedback": ("Add feedback", "Whether to also connect the sink's feedback back to the source."),
    "addToDevices": ("Parent devices", "JSON array of device IDs to add the new devices to as subdevices; empty for none."),
    "address": ("Address", "The port's universe address, as DMXControl numbers universes."),
    "affinityList": ("Affinity list", "JSON array of the class's objects (IEnumObjectData), the order to read; empty to read it."),
    "affinityValues": ("Affinity values", "JSON array of numbers, one per object of the affinity list; empty to read them."),
    "append": ("Append", "Whether to append the programmer to the preset (true) or merge it in (false)."),
    "appendthis": ("Append text", "Text to append to the command line."),
    "attachable": ("Attachable", "JSON object of the attachable (AttachableData: id, type, name and the rest), as get_attachables lists it."),
    "attachableId": ("Attachable ID", "The ID of the attachable in the programmer."),
    "audio": ("Audio file", "The audio file the special cue plays."),
    "audioFile": ("Audio file", "The audio file to play, by its resource name."),
    "autoPositionData": ("Auto position", "JSON object with startPosition {x, y} and maxWidth, to place the devices added automatically."),
    "autoprepareEnabled": ("Auto-prepare", "Whether the cue's devices move into position before it fades in."),
    "backgroundTaskId": ("Background task ID", "The background task's ID, as background_tasks in state keys it."),
    "bag": ("Parameters", "JSON object of the attachable's parameter bag: source (the attachable) and parameters (name to value)."),
    "balance": ("Balance", "The player's balance, from left to right; used when balance_set is true."),
    "balanceSet": ("Set balance", "Whether to set the balance."),
    "bankId": ("Bank ID", "The input bank's ID, as banks in state keys it."),
    "bankNameTemplate": ("Name", "The new bank's name, or a template for it."),
    "blackout": ("Blackout", "The blackout global function: set it on or off with set_value, or name it to read it."),
    "blind": ("Blind", "Whether to edit blind, without changing the output."),
    "boolValue": ("Boolean value", "The setting's new value, for a Boolean setting."),
    "button": ("Button", "JSON object of the button to press or release: masterId, name and ButtonValue (true pressed)."),
    "button1": ("Button 1", "Press (True) or release (False) the executor's first button; NullBoolValue leaves it."),
    "button2": ("Button 2", "Press (True) or release (False) the executor's second button; NullBoolValue leaves it."),
    "button3": ("Button 3", "Press (True) or release (False) the executor's third button; NullBoolValue leaves it."),
    "button4": ("Button 4", "Press (True) or release (False) the executor's fourth button; NullBoolValue leaves it."),
    "buttonNumber": ("Button", "The macro button's number, as the macro lists its buttons."),
    "buttonPressed": ("Button presses", "JSON array of button presses, each {x, y, pressed}: the button's column and row from 0, and pressed or released."),
    "bytesToRead": ("Bytes to read", "How many bytes of the stream to read; 0 to only write."),
    "category": ("Category", "The to-do's category."),
    "changeTags": ("Change tags", "Whether to replace the version's tags with tags_to_set."),
    "changeType": ("Change", "Added, Changed or Removed: what to do with the proxy."),
    "changes": ("Changes", "JSON array of attribute changes, each one of projectName, description, author, isDeleteLocked or isFavorite."),
    "childrenIds": ("Children", "JSON array of the IDs of the children to add."),
    "className": ("Affinity class", "The affinity class, as get_affinity_classes names it."),
    "clear": ("Clear", "Which value of the entry to clear: Delay, Fade, DeviceDelay or DeviceFade."),
    "clearProgrammer": ("Clear programmer", "Whether to clear the programmer afterwards."),
    "clientReferences": ("Client resources", "JSON array of resources this client wants saved with the project, each {type, name, source}."),
    "clientRuntimeId": ("Program", "The runtime ID of the program to change, as clients in state keys it."),
    "closeInBackend": ("Close", "Whether to close the stream after this block."),
    "comment": ("Comment", "The new comment."),
    "compareContent": ("Compare content", "Whether to compare the resources' content, not only their dates and sizes."),
    "connectionSetId": ("Connection set ID", "The connection set's ID, as connection_sets in state keys it."),
    "connectionSetIds": ("Connection sets", "JSON array of connection set IDs."),
    "connectionSetNameTemplate": ("Name", "The new connection set's name, or a template for it."),
    "consumers": ("Consumers", "JSON array of device IDs that draw from the power source."),
    "contentType": ("Content type", "RAW for a file's bytes, MANAGED_TREE for a structured resource."),
    "continue": ("Continue", "Continue the background task; give this or pause."),
    "controlClientRuntimeId": ("Program", "The runtime ID of the program to control, as clients in state keys it."),
    "copyAllVersions": ("Copy all versions", "Whether to copy every version of the project, not only the one named."),
    "copyFrom": ("Copy from", "JSON object of the project version to copy (ProjectVersionID: guid, major, minor, revision); none for a new empty project."),
    "copyFromBank": ("Copy from", "The ID of a bank to copy; empty for an empty bank."),
    "copyFromConnectionSet": ("Copy from", "The ID of a connection set to copy; empty for an empty one."),
    "copyFromCuelist": ("Copy from", "The ID of a cuelist to copy; empty for an empty cuelist."),
    "copyFromGroup": ("Copy from", "The ID of a group to copy; empty for an empty group."),
    "copyFromItemList": ("Copy from", "The ID of an item list to copy; empty for an empty list."),
    "copyFromMaster": ("Copy from", "The ID of a master to copy; empty for a new master of the type."),
    "copyFromPage": ("Copy from", "The ID of an executor page to copy; empty for an empty page."),
    "copyFromPreset": ("Copy from", "The ID of a preset to copy, in place of storing the programmer."),
    "copyFromProfile": ("Copy from", "The ID of a profile to copy."),
    "copyFromStageView": ("Copy from", "The ID of a stage view to copy; empty for an empty one."),
    "copyFromTemplate": ("Template", "The ID of a profile template to start from."),
    "copyFromTimecode": ("Copy from", "The ID of a timecode show to copy; empty for an empty one."),
    "copyNameTemplate": ("Name", "The copy's name, or a template for it."),
    "crcValue": ("CRC", "The CRC of the data written, to check it."),
    "createNewOnDuplicate": ("New project on duplicate", "Whether to import as a new project when the project already exists."),
    "cueEntryId": ("Cue entry ID", "The cue entry's ID, as its cue lists its entries."),
    "cueId": ("Cue ID", "The cue's ID, as its cuelist lists its cues."),
    "cueNumber": ("Cue number", "JSON object {number: [major, minor, ...]}: the cue's number, its parts in order."),
    "cuelistGroupId": ("Cuelist group ID", "The cuelist group's ID, as cuelist_groups in state keys it."),
    "cuelistId": ("Cuelist ID", "The cuelist's ID, as cuelists in state keys it."),
    "cuelistIds": ("Cuelists", "JSON array of cuelist IDs."),
    "cuelistNameTemplate": ("Name", "The new cuelist's name, or a template for it."),
    "customProfileSize": ("Size", "JSON object {width, height}: the new profile's size in buttons."),
    "dataToWrite": ("Data", "The bytes to write, as base64; empty to only read."),
    "dataType": ("Resource kind", "The kind of resource: ICON, DEVICE_IMAGE, PICTURE, DDF, SYMBOL, SOUND, LOG or BINARY; give this or a resource type."),
    "delay": ("Delay", "JSON object of the fanned delay value (FannedPropertyValue: operator, fannedValues and the rest)."),
    "delayDown": ("Delay down", "JSON object of the fanned delay for falling values (FannedPropertyValue)."),
    "deleteAllVersions": ("All versions", "Whether to delete every version of the project, not only the one named."),
    "dependencies": ("Dependencies", "JSON object of the dependencies, as a refused deletion answered them, each with its solution chosen; none for the first try."),
    "device": ("Device", "JSON object of the device definition (DeviceMetadataData), as get_available_devices lists it."),
    "deviceAndGroupIDs": ("Devices and groups", "JSON array of device and group IDs."),
    "deviceAndGroupIds": ("Devices and groups", "JSON array of device and group IDs, in the group's order."),
    "deviceDelay": ("Device delay", "JSON object {deviceId, value, valueSet}: one device's delay in the entry, in milliseconds."),
    "deviceFade": ("Device fade", "JSON object {deviceId, value, valueSet}: one device's fade in the entry, in milliseconds."),
    "deviceGroup": ("Device group", "JSON object of the group change (StageViewDeviceGroupModifyData)."),
    "deviceGroupId": ("Group ID", "The device group's ID, as device_groups in state keys it."),
    "deviceGroupIds": ("Groups", "JSON array of device group IDs."),
    "deviceId": ("Device ID", "The device's ID, as devices in state keys it."),
    "deviceIds": ("Devices", "JSON array of device IDs."),
    "deviceIntensities": ("Intensities", "JSON array of {deviceId, intensity}."),
    "deviceOrGroupId": ("Device or group ID", "The ID of the device or group."),
    "devicePositions": ("Positions", "JSON array of {deviceId, position: {pan, tilt, panset, tiltset}}."),
    "devices": ("Devices", "JSON array of {deviceId, intensity, position: {pan, tilt, panset, tiltset}}."),
    "dmxChannels": ("Channels", "JSON array of the DMX channels to set, as DMXControl numbers them."),
    "dmxInStart": ("First input channel", "The first DMX input channel to read."),
    "dmxInStop": ("Last input channel", "The last DMX input channel to read."),
    "dmxOutStart": ("First output channel", "The first DMX output channel to read."),
    "dmxOutStop": ("Last output channel", "The last DMX output channel to read."),
    "dmxValues": ("Values", "The channels' values, one byte each in the order of channels, as base64."),
    "done": ("Done", "Whether the to-do is done."),
    "doubleValue": ("Number value", "The setting's new value, for a number setting."),
    "dpv": ("Value", "JSON object of the property value (DevicePropertyValue: doubleValue, color, position and the other kinds)."),
    "edgeToCreate": ("Edge to create", "JSON object {from, to}: the two ports to connect."),
    "edgeToDelete": ("Edge to delete", "JSON object {from, to}: the two ports to disconnect."),
    "editMessages": ("Changes", "JSON array of the changes, each {id, parameter, ticks} and one of newValueBool, newValueInt, newValueLong, newValueFloat or newValueString."),
    "editType": ("Edit", "ADD, CHANGE, REMOVE, COPY, CUT or PASTE."),
    "enabled": ("Enabled", "Whether it is on."),
    "enabledSet": ("Set enabled", "Whether to set enabled."),
    "enabledState": ("On", "Whether the main switch is on; give this or toggle."),
    "endTrim": ("End trim", "How much to trim off the sound's end, in ticks."),
    "entryId": ("Entry ID", "The ID of the programmer entry."),
    "executionSource": ("Source", "A name for what runs the procedure, as the log shows it."),
    "executionType": ("Execution", "No (only edit the command line), DryRun (check it) or Run."),
    "executorId": ("Executor ID", "The executor's ID, as executors in state keys it."),
    "executorPageId": ("Executor page ID", "The executor page's ID, as executor_pages in state keys it."),
    "existingGroupId": ("Existing group", "The ID of a group to add the new devices to; give this or a group name."),
    "exportEverything": ("Export everything", "Whether to export every resource the project uses."),
    "factor": ("Factor", "The manual beat's factor, multiplying its speed."),
    "fade": ("Fade", "JSON object of the fanned fade value (FannedPropertyValue)."),
    "fadeDown": ("Fade down", "JSON object of the fanned fade for falling values (FannedPropertyValue)."),
    "fadeFactor": ("Fade factor", "The cuelist's fade factor, multiplying its fade times."),
    "fader": ("Fader", "JSON object of the fader to move: masterId, name and increment or absolut."),
    "faderAbsolut": ("Fader position", "The executor fader's new position; give this or an increment."),
    "faderIncrement": ("Fader increment", "How far to move the executor fader, negative for down; give this or a position."),
    "faderNumber": ("Fader", "The macro fader's number, as the macro lists its faders."),
    "fanningGroupSize": ("Fanning group size", "How many devices share a fanned value."),
    "fogState": ("Fog", "The fog global function's state."),
    "forceInvokeAll": ("Apply everything", "Whether to apply every value of the preset, even those the device already has."),
    "fpv": ("Fanned value", "JSON object of the fanned property value (FannedPropertyValue)."),
    "freeze": ("Freeze", "The freeze global function: set it on or off with set_value, or name it to read it."),
    "fromGroupId": ("From group", "The group to copy from."),
    "fromProgrammer": ("From programmer", "JSON object {userContextId, groupId, propertyId, effectId}: the effect in a programmer to visualize."),
    "fromUserContextId": ("From user context", "The user context to copy from."),
    "groupHandling": ("Group handling", "How a group's devices take the value: CONCAT_GROUPS, GROUP_IS_DEVICE or PARALLEL_GROUPS."),
    "groupId": ("Group ID", "The ID of the device group."),
    "groupNameTemplate": ("Group name", "The name of a new group for the devices, or a template for it; give this or an existing group."),
    "highlight": ("Highlight", "JSON object of the highlight: userContextId, id, and enabled or itemType."),
    "id": ("ID", "The ID of the object."),
    "idFilter": ("IDs", "JSON array of the IDs to read; empty for all."),
    "idsToFix": ("Fix", "JSON array of the executor IDs to fix."),
    "idsToUnfix": ("Unfix", "JSON array of the executor IDs to unfix."),
    "includedDevices": ("Devices", "JSON array of the device IDs to read."),
    "includingValues": ("With values", "Whether to read the parameters' values too."),
    "increment": ("Increment", "How far to move the fader, negative for down; give this or a position."),
    "incrementVersion": ("Version", "Which part of the version number to increase: ESaveProjectVersionIncrement_None, _Minor or _Major."),
    "index": ("Index", "The cue's place in the cuelist, from 0."),
    "info": ("Program information", "JSON object of this client's program information (ClientProgramInfo)."),
    "initialData": ("Project", "JSON object of the new project (ProjectGlobalData: projectName, author, projectDescription, tags and the rest)."),
    "inputSinkId": ("Sink ID", "The input sink's ID."),
    "inputSourceId": ("Source ID", "The input source's ID."),
    "intValue": ("Whole number value", "The setting's new value, for a whole-number setting."),
    "intensity": ("Intensity", "The cuelist's intensity."),
    "interface": ("Interface", "JSON object of the interface (DMXInterfaceMetadata), as get_available_dmx_interfaces lists it."),
    "interfaceId": ("Interface ID", "The DMX interface's ID, as dmx_interfaces in state keys it."),
    "interval": ("Interval", "The manual beat's interval, in milliseconds."),
    "ioFlags": ("I/O flags", "The access wanted, as DMXControl's resource flags."),
    "isAutosave": ("Autosave", "Whether to save as an autosave."),
    "itemListId": ("Item list ID", "The item list's ID, as item_lists in state keys it."),
    "itemListNameTemplate": ("Name", "The new item list's name, or a template for it."),
    "kernelRuntimeId": ("Kernel", "The runtime ID of the kernel whose log to read, as clients in state keys it."),
    "key": ("Key", "The executor's key to act on, as its fader_key and button keys name them; empty for the executor itself."),
    "listType": ("List type", "The kind of item list, as get_available_item_list_types names it."),
    "logCount": ("Entries", "How many log entries to read."),
    "macro": ("Macro", "JSON object {macroId, elementIndex}: the macro and its element the special cue runs."),
    "macroBoard": ("Macro board", "JSON object of the macro board {id, width, height}."),
    "macroBoardId": ("Macro board ID", "The macro board's ID."),
    "macroId": ("Macro ID", "The macro's ID, as macros in state keys it."),
    "mainSwitchId": ("Main switch ID", "The main switch's ID, as main_switches in state keys it."),
    "managedItem": ("Managed data", "JSON object of the managed tree item to save (name, values, children)."),
    "mapping": ("Mapping", "JSON array of mappings, each {inAddress, outAddress, mappingMode}."),
    "markerId": ("Marker ID", "The marker's ID, as markers in state keys it."),
    "markerType": ("Marker type", "The kind of marker, as DMXControl numbers them."),
    "masterId": ("Master ID", "The master's ID, as masters in state keys it."),
    "masterNameTemplate": ("Name", "The new master's name, or a template for it."),
    "memberId": ("Object", "The ID of the object to put on the executor, as executor_member_wrappers lists it."),
    "message": ("Message", "JSON object of the confirmation to send back."),
    "messageType": ("Message type", "The broadcast's type, which receivers use to tell broadcasts apart."),
    "mode": ("Mode", "What to read: VALUE, DELAY or FADE."),
    "moveFrom": ("Move from", "The place of the item to move, from 0."),
    "moveTo": ("Move to", "Where to move it, from 0."),
    "name": ("Name", "The name."),
    "nameFilter": ("Names", "JSON array of the parameter names to read; empty for all."),
    "nameTemplate": ("Name", "The new to-do's name, or a template for it."),
    "newEffect": ("New effect", "JSON object {meta, parameters}: the attachable and its parameter bag."),
    "newName": ("New name", "The preset's new name."),
    "newValueBool": ("Boolean value", "The parameter's new value, when it is a Boolean."),
    "newValueFloat": ("Number value", "The parameter's new value, when it is a number."),
    "newValueInt": ("Whole number value", "The parameter's new value, when it is a 32-bit whole number."),
    "newValueLong": ("Long value", "The parameter's new value, when it is a 64-bit whole number."),
    "newValueString": ("Text value", "The parameter's new value, when it is text."),
    "nodeToCreateTypeId": ("Node type", "The type of node to create, as get_available_nodes lists it."),
    "nodeToDelete": ("Node to delete", "JSON object {graphId, nodeId}: the node to delete."),
    "nodesToCopy": ("Nodes to copy", "JSON array of {graphId, nodeId}: the nodes to copy."),
    "notOnFreeze": ("Not while frozen", "Whether not to run the procedure while freeze is on."),
    "number": ("Number", "The new number."),
    "numberOfDevices": ("Count", "How many devices to create."),
    "onlyDiff": ("Only differences", "Whether to list only resources that differ from those given in result."),
    "originalNameOfImportedFile": ("Original name", "The file's original name, as the project list shows it."),
    "otherCuelistId": ("Cuelist", "The ID of the cuelist the special cue runs."),
    "pageNameTemplate": ("Name", "The new page's name, or a template for it."),
    "parameter": ("Parameter", "JSON object of the parameter (GenericParameterData), as get_parameters lists it."),
    "parameterName": ("Parameter", "The parameter's name, as get_dmx_interface_parameters lists it."),
    "params": ("Effect", "JSON object {userContextId, groupId, propertyId, effectId}: the effect in a programmer."),
    "parentContainerId": ("Cuelist ID", "The ID of the cuelist the cue is in."),
    "parentEntryContainerId": ("Cue ID", "The ID of the cue the entry is in."),
    "parentParentContainerId": ("Cuelist ID", "The ID of the cuelist that cue is in."),
    "parentTimecodeId": ("Timecode ID", "The ID of the timecode show."),
    "parentTrackId": ("Track ID", "The ID of the track."),
    "passwordHash": ("Password hash", "The user's password, hashed as DMXControl's user management stores it."),
    "path": ("Setting", "The setting's path, as get_registered_settings lists it."),
    "pause": ("Pause", "Pause the background task; give this or continue."),
    "playMode": ("Play mode", "Play, Stop, Pause or Buffer; None leaves the playback as it is."),
    "playerThreadId": ("Player ID", "The audio player's ID, as audio_players in state keys it; empty for a new player."),
    "pluginId": ("Plugin ID", "The plugin's ID, as plugins in state keys it."),
    "pluginIds": ("Plugins", "JSON array of plugin IDs to read; empty for all."),
    "port": ("Port", "The interface's port, numbered as get_used_dmx_interfaces lists them."),
    "portType": ("Port type", "INPUTPORT or OUTPUTPORT."),
    "positionToAdd": ("Position", "JSON object {x, y}: where to place the node."),
    "powerSourceId": ("Power source ID", "The power source's ID, as power_sources in state keys it."),
    "powerSourceNameTemplate": ("Name", "The new power source's name, or a template for it."),
    "predicate": ("Filter", "JSON object of the programmer filter: userContextId, includedDeviceIds, includedGroupIds, includedPropertyIds and destinationSources."),
    "presetId": ("Preset ID", "The preset's ID, as presets in state keys it."),
    "presetNameTemplate": ("Name", "The new preset's name, or a template for it."),
    "priority": ("Priority", "The to-do's priority."),
    "procedure": ("Procedure", "JSON object of the procedure (ProcedureMetadataData), as the device lists it in procedures."),
    "profileId": ("Profile ID", "The macro board profile's ID, as macro_board_profiles in state keys it."),
    "profileNameTemplate": ("Name", "The new profile's name, or a template for it."),
    "project": ("Project", "Whether the setting is the project's (true) or the application's (false)."),
    "propertyId": ("Property ID", "The ID of the device property."),
    "propertyIds": ("Properties", "JSON array of property IDs."),
    "propertyName": ("Property", "The programmer property's name."),
    "propertyPreset": ("Property preset", "Whether the preset holds property values for any device of a type, not for particular devices."),
    "propertyTypes": ("Property types", "JSON array of property type numbers."),
    "psum": ("Psum", "The psum the special cue sets."),
    "quickResponse": ("Quick", "Whether to answer at once from what each program last reported."),
    "rawContent": ("Raw content", "The broadcast's content as bytes, in base64; give this or text."),
    "rawData": ("Data", "The resource's bytes, as base64."),
    "receiverHostnames": ("Receivers", "JSON array of host names to send to; empty for every program."),
    "receiverType": ("Receiver type", "The kind of program to send to; EClientType_Unknown for every kind."),
    "referenceObjectId": ("Reference object", "The ID of the reference object of the affinity class."),
    "registered": ("Registered", "Whether the setting is a registered one."),
    "relative": ("Relative", "Whether the value is relative to the current one."),
    "removeReferenceInProject": ("Remove reference", "Remove the resource's reference from the project."),
    "replacementId": ("Replacement", "The ID of the device definition (metadataId) to replace it with."),
    "requests": ("Requests", "JSON array of property value requests, each {deviceOrGroupId, propertyId, userContextId, type}."),
    "resetDirty": ("Reset unsaved changes", "Whether saving clears the unsaved-changes flag: True, False or NullBoolValue for the default."),
    "resetFactor": ("Reset factor", "Whether to reset the beat factor to 1."),
    "resource": ("Resource", "JSON object of the resource (ResourceData)."),
    "resourceType": ("Resource store", "The store the resource is in: APPLICATION, PROJECT, TEMP, KERNEL_TEMP or CONFIG; give this or a resource kind."),
    "result": ("Known resources", "JSON array of the resources the client has, to compare with."),
    "runtimeId": ("Runtime ID", "This client's runtime ID, as session.runtime_id in state holds it."),
    "selected": ("Selected", "Whether the snip is selected."),
    "senderName": ("Sender", "The name to send under."),
    "setValue": ("Set", "Whether to set the function (true) or only read it (false)."),
    "setting": ("Setting", "JSON object of the setting's description (SettingsMetadataData)."),
    "showHideAction": ("Show or hide", "EShowHideProcess_SHOW or EShowHideProcess_HIDE; give this or a shutdown action."),
    "shutdownAction": ("Shutdown", "EClientShutdownAction_NOW, or EClientShutdownAction_NO_OTHER_CLIENT to shut down only when no other program is connected."),
    "sink": ("Sink", "JSON object of the sink (InputSinkDescriptor: element and providesFeedback)."),
    "sinkIdAsReplacement": ("Replacement sink", "The ID of a sink to replace the node with."),
    "sinkOrElementId": ("Sink", "The ID of the sink (or sink element) to connect."),
    "sinkOrElementIdToCreate": ("Sink to create", "The ID of the sink (or sink element) of the new node."),
    "snapshots": ("Snapshots", "JSON array of snapshot names."),
    "snipId": ("Snip ID", "The snip's ID, as snips in state keys it."),
    "soundFileName": ("Sound file", "The sound file, by its resource name."),
    "soundOutputInfo": ("Sound output", "JSON object of the sound output {guid, description, moduleName}, as sound_outputs lists it."),
    "soundOutputInfoSet": ("Set sound output", "Whether to set the sound output."),
    "source": ("Source", "The name of the source, which clear_dmx_values clears."),
    "sourceIdAsReplacement": ("Replacement source", "The ID of a source to replace the node with."),
    "sourceOrElementId": ("Source", "The ID of the source (or source element) to connect."),
    "sourceOrElementIdToCreate": ("Source to create", "The ID of the source (or source element) of the new node."),
    "sourceValue": ("Value", "JSON object of the source's current value (ObjectData)."),
    "space": ("Spacing", "How many DMX channels to leave between the devices."),
    "specialEnum": ("Option", "An option of the special cue, numbered as DMXControl numbers that kind's options."),
    "specialValue": ("Value", "JSON object of the special cue's value (ObjectData), where its kind takes one."),
    "speedFactor": ("Speed factor", "The cuelist's speed factor, multiplying its speed."),
    "stageViewId": ("Stage view ID", "The stage view's ID, as stage_views in state keys it."),
    "stageViewNameTemplate": ("Name", "The new stage view's name, or a template for it."),
    "startAddress": ("Start address", "JSON object of the DMX start address (DMXAddressData)."),
    "startTrim": ("Start trim", "How much to trim off the sound's start, in ticks."),
    "state": ("State", "JSON object {readyToWork, receiveAndDisplayMessages}."),
    "steps": ("Steps", "How many steps of the effect to compute."),
    "streamId": ("Stream ID", "The stream's ID, as load_resource or save_resource gave it."),
    "stringContent": ("Text", "The broadcast's content as text; give this or raw content."),
    "stringValue": ("Text value", "The setting's new value, for a text setting."),
    "subSources": ("Sub-sources", "JSON array of power source IDs that draw from this one."),
    "subtract": ("Subtract", "Whether to subtract the programmer from the preset (true) or replace the preset with it (false)."),
    "sum": ("Sum", "The sum the special cue sets."),
    "tagsToSet": ("Tags", "JSON array of the version's new tags."),
    "takeFades": ("Take fades", "Whether the cue takes the fade times of its entries."),
    "takecurrentcontext": ("Take context", "Fill the command line from the current context."),
    "takethis": ("Command line", "Text to put on the command line in place of what is there."),
    "target": ("Target", "INTERFACE, INPUTPORT or OUTPUTPORT: what to enable or disable."),
    "targetBankId": ("Bank", "The ID of the bank to put it in."),
    "targetClientName": ("Client name", "The program's new client name; give this or a network ID."),
    "targetConnectionSetForCopyNodes": ("Copy nodes to", "The ID of the connection set to copy the nodes to."),
    "targetDeviceGroupIds": ("Groups", "JSON array of the group IDs to run the procedure on."),
    "targetDeviceIds": ("Devices", "JSON array of the device IDs to run the procedure on."),
    "targetGroupId": ("To group", "The group to copy to."),
    "targetIndex": ("Target index", "The place to move to, from 0."),
    "targetNetworkId": ("Network ID", "The program's new network ID; give this or a client name."),
    "targetParentContainerId": ("Target cuelist", "The ID of the cuelist to copy into."),
    "targetUserContextId": ("To user context", "The user context to copy to."),
    "tempFader": ("Temporary fader", "The cuelist's temporary fader."),
    "tempFileName": ("File", "The project file's name in Umbra's temporary resources."),
    "testData": ("Test data", "Bytes to send through and back, as base64."),
    "text": ("Text", "The to-do's text."),
    "tick": ("Position", "Where the snip starts, in ticks."),
    "ticks": ("Position", "Where, in ticks."),
    "ticksEnd": ("End", "Where the marker ends, in ticks."),
    "time": ("Time", "The player's position, in milliseconds; used when time_set is true."),
    "timeSet": ("Set time", "Whether to set the position."),
    "timecodeId": ("Timecode ID", "The timecode show's ID, as timecodes in state keys it."),
    "timecodeNameTemplate": ("Name", "The new timecode show's name, or a template for it."),
    "toAdd": ("Items to add", "JSON array of items (IEnumObjectData) to add."),
    "toExport": ("Version", "JSON object of the project version to export (ProjectVersionID: guid, major, minor, revision)."),
    "toRemove": ("Items to remove", "JSON array of the places of the items to remove, from 0."),
    "toReplace": ("Node", "JSON object {graphId, nodeId}: the node to replace."),
    "todoId": ("To-do ID", "The to-do's ID, as project_todos in state keys it."),
    "toggle": ("Toggle", "Toggle the main switch; give this or on."),
    "trackId": ("Track ID", "The track's ID, as tracks in state keys it."),
    "trackType": ("Track type", "The kind of track, as DMXControl numbers them."),
    "transformationMatrix": ("Transformation", "JSON array of the numbers of the stage view's transformation matrix."),
    "triggerValue": ("Trigger", "JSON object {trigger, value}: the trigger, as get_cue_triggers names it, and its value."),
    "type": ("Type", "The kind."),
    "uiValueType": ("Value type", "The kind of value, as DMXControl numbers them."),
    "userContextId": ("User context", CONTEXT),
    "userImported": ("Imported", "Whether the user imported the resource."),
    "username": ("User", "The user's name in DMXControl's user management."),
    "value": ("Value", "JSON object of the value (ObjectData), one of stringValue, doubleValue, intValue, boolValue and the other kinds."),
    "versionToDelete": ("Version", "JSON object of the project version to delete (ProjectVersionID)."),
    "versionToLoad": ("Version", "JSON object of the project version to load (ProjectVersionID), as projects lists it."),
    "versionToModify": ("Version", "JSON object of the project version to change (ProjectVersionID)."),
    "viewMode": ("Mode", "What to read: VALUE, DELAY or FADE."),
    "volume": ("Volume", "The player's volume; used when volume_set is true."),
    "volumeSet": ("Set volume", "Whether to set the volume."),
    # Where a name means something else in one request.
    ("AllCuelistActionRequest", "action"): ("Action", "STOP: stop every cuelist."),
    ("CuelistActionRequest", "action"): ("Action", "PLAY, PAUSE, STOP, GO, GO_BACK, GO_TO (to the cue at index), GO_NEXT, LOAD (the cue at index) or REASSIGN_SCENE_NUMBERS."),
    ("CuelistActionRequest", "index"): ("Cue index", "For GO_TO and LOAD, the cue's place in the cuelist, from 0."),
    ("MasterActionRequest", "action"): ("Action", "DOUBLE_TIMES, HALF_TIMES, LEARN, SYNC, FULL or BLACK."),
    ("TimecodeActionRequest", "action"): ("Action", "PLAY, STOP, PAUSE, RECORD or CLEAR."),
    ("CueAddFromProgrammerRequest", "type"): ("How", "ADD, INSERT, REPLACE, MERGE, APPEND or SUBTRACT."),
    ("CueAddFromProgrammerRequest", "index"): ("Index", "The place in the cuelist, from 0."),
    ("SpecialCueInsertRequest", "index"): ("Index", "The place in the cuelist, from 0."),
    ("SpecialCueInsertRequest", "presetId"): ("Preset", "The ID of the preset the special cue applies."),
    ("SpecialCueInsertRequest", "timecodeId"): ("Timecode", "The ID of the timecode show the special cue runs."),
    ("GetPropertyValueSourceRequest", "index"): ("Cue index", "The cue's place in the cuelist, from 0."),
    ("CuelistGroupModifyRequest", "type"): ("Change", "ADD, REMOVE, MOVE or MOVE_ALL."),
    ("DeviceGroupModifyRequest", "type"): ("Change", "ADD, REMOVE, MOVE or MOVE_ALL."),
    ("DeviceGroupModifyRequest", "deviceIds"): ("Devices", "JSON array of device IDs to add, remove or move."),
    ("ExecutorModifyRequest", "type"): ("Change", "SET (put the object on the executor) or REMOVE."),
    ("ItemListModifyRequest", "type"): ("Change", "ADD, REMOVE, MOVE, CLEAR or REDEFINE."),
    ("ModifyAttachablePresetRequest", "type"): ("Change", "ADD, DELETE or CHANGE."),
    ("ModifyAttachablePresetRequest", "id"): ("Preset ID", "The saved attachable preset's ID; empty when adding."),
    ("StageViewModifyRequest", "type"): ("Change", "MOVE, TRANSFORMATION_MATRIX, DEVICE or DEVICE_GROUP: what the request changes."),
    ("StageViewModifyRequest", "device"): ("Device", "JSON object of the device change (StageViewDeviceModifyData: type, deviceId, aliasId, position, size, displayType)."),
    ("PowerSourceModificationRequest", "type"): ("Change", "Added to add the sub-sources and consumers, Removed to remove them."),
    ("ProgrammerSnapshotsRequest", "type"): ("Action", "WRITE, LOAD or CLEAR."),
    ("ProgrammerSnapshotsRequest", "name"): ("Snapshot", "The snapshot's name."),
    ("MasterCreationRequest", "type"): ("Master type", "GRAND, GROUP, DEVICE, SPECIAL, PARAMETER, COLOR, POSITION or SPEED."),
    ("ItemListCreationRequest", "listType"): ("List type", "The kind of item list, as get_available_item_list_types names it."),
    ("TrackCreationRequest", "name"): ("Name", "The track's name."),
    ("MarkerCreationRequest", "id"): ("Marker ID", "An ID for the new marker; empty to let DMXControl choose."),
    ("MarkerCreationRequest", "name"): ("Name", "The marker's name."),
    ("MarkerEditRequest", "parameter"): ("Parameter", "The marker's parameter to change, by name."),
    ("SnipCreationRequest", "id"): ("Snip ID", "An ID for the new snip; empty to let DMXControl choose."),
    ("SnipCreationRequest", "name"): ("Name", "The snip's name."),
    ("ProjectTodoModifyRequest", "name"): ("Name", "The to-do's new name."),
    ("ProjectTodoDeletionRequest", "todoId"): ("To-dos", "JSON array of the IDs of the to-dos to delete."),
    ("MacroBoardProfileChangeRequest", "name"): ("Name", "The profile's new name."),
    ("MacroBoardProfileChangeRequest", "number"): ("Number", "The profile's new number."),
    ("MacroBoardProxyRequest", "changeType"): ("Change", "Added, Changed or Removed: what to do with the macro board."),
    ("MacroBoardProxyRequest", "runtimeId"): ("Runtime ID", "This client's runtime ID, as session.runtime_id in state holds it."),
    ("MacroSetButtonStateRequest", "active"): ("Pressed", "Whether the button is pressed (true) or released (false)."),
    ("SetCueValueRequest", "name"): ("Name", "The cue's new name."),
    ("SetCueValueRequest", "enabled"): ("Enabled", "Whether the cue is enabled."),
    ("SetCueValueRequest", "comment"): ("Comment", "The cue's new comment."),
    ("SetCueValueRequest", "delay"): ("Delay", "JSON object of the cue's fanned delay (FannedPropertyValue)."),
    ("SetCueValueRequest", "fade"): ("Fade", "JSON object of the cue's fanned fade (FannedPropertyValue)."),
    ("GetCueStateRequest", "cueId"): ("Cue ID", "The cue's ID."),
    ("CueStateRequest", "cueId"): ("Cue ID", "The ID of a cue to read in place of a programmer; give this or a user context."),
    ("CueStateRequest", "userContextId"): ("User context", "The user context whose programmer to read; give this or a cue."),
    ("CueCopyRequest", "targetIndex"): ("Target index", "The place in the target cuelist, from 0."),
    ("CueMoveRequest", "targetIndex"): ("Target index", "The cue's new place in its cuelist, from 0."),
    ("ExportProjectRequest", "exportEverything"): ("Export everything", "Whether to export every resource the project uses."),
    ("DeleteProgrammerValueRequest", "Mode"): ("Mode", "DELETE, CLEAR, UNDO, CLEAR_SELECTED or CLEAR_GROUP."),
    ("SetProgrammerOutputRequest", "Mode"): ("Output", "ALL, FILTER or HIDDEN; give this or a filter."),
    ("SetProgrammerValueRequest", "groupId"): ("Device or group ID", "The ID of the device or group whose property to set."),
    ("SetProgrammerAttachableRequestResponse", "groupId"): ("Group ID", "The ID of the group; give this or a device."),
    ("SetProgrammerAttachableRequestResponse", "deviceId"): ("Device ID", "The ID of the device; give this or a group."),
    ("InvokePresetRequest", "deviceId"): ("Device ID", "The device to apply the preset to; give this or a group."),
    ("InvokePresetRequest", "deviceGroupId"): ("Group ID", "The group to apply the preset to; give this or a device."),
    ("InvokePresetRequest", "propertyId"): ("Property ID", "The property to apply the preset to; empty for every property it holds."),
    ("CanAttachAttachableToRequest", "deviceId"): ("Device ID", "The device to check; give this or a group."),
    ("CanAttachAttachableToRequest", "deviceGroupId"): ("Group ID", "The group to check; give this or a device."),
    ("CanAttachAttachableToRequest", "propertyId"): ("Property ID", "A property to check; empty for every property."),
    ("CanAttachPresetToRequest", "deviceId"): ("Device ID", "The device to check; give this or a group."),
    ("CanAttachPresetToRequest", "deviceGroupId"): ("Group ID", "The group to check; give this or a device."),
    ("DeviceCreationRequest", "nameTemplate"): ("Name", "The new devices' name, or a template for it."),
    ("DeviceDeletionRequest", "parentDeviceId"): ("Parent device", "The ID of the device the device is a subdevice of; empty for none."),
    ("DescriptorRequest", "groupId"): ("Group ID", "The group to describe, or whose property to describe."),
    ("DescriptorRequest", "type"): ("What", "PROPERTY, SUBPROPERTY, GROUP, TYPE or SUBTYPE: what to describe."),
    ("DevicePropertyValueRequest", "type"): ("Value", "CURRENT_OUTPUT, CURRENT_PROPERTYVALUE, CURRENT_PROPERTYVALUE_VALUE or PROGRAMMER: which value to read."),
    ("EnableDisableDMXInterfaceRequest", "enabled"): ("Enabled", "Whether to enable (true) or disable (false) it."),
    ("EnableDisableDMXInterfaceRequest", "port"): ("Port", "The port to enable or disable, for INPUTPORT and OUTPUTPORT."),
    ("GetSetDMXInterfaceCommentRequestResponse", "comment"): ("Comment", "The port's new comment; empty when reading."),
    ("GetSetDMXInterfaceAddressRequestResponse", "address"): ("Address", "The port's new universe address; 0 when reading."),
    ("GetSetTestInterfaceParameterValueRequestResponse", "value"): ("Value", "JSON object of the parameter's value (ObjectData); none when reading."),
    ("BackgroundTaskManagementRequest", "pause"): ("Pause", "Pause the background task; give this or continue."),
    ("ControlPlayerThreadRequest", "balance"): ("Balance", "The player's balance, from left to right; used when balance_set is true."),
    ("ManualBeatIntervalRequestResponse", "enabled"): ("On", "Whether the manual beat is on; used when enabled_set is true."),
    ("ManualBeatLearnRequest", "time"): ("Tap time", "The time of the tap, as DMXControl counts time."),
    ("ManualBeatLearnRequest", "runtimeId"): ("Runtime ID", "The runtime ID of the program tapping, as session.runtime_id in state holds it."),
    ("ManualBeatSyncRequest", "time"): ("Time", "The time to restart the count at, as DMXControl counts time."),
    ("GetDefaultConfigurationRequest", "name"): ("Configuration", "The default configuration's name; empty for every one."),
    ("GetDefaultConfigurationRequest", "project"): ("Project", "Whether to read the project's (true) or the application's (false)."),
    ("GetSettingRequest", "type"): ("Value type", "String, Int, Bool or Double: the value type to read it as."),
    ("SetSettingRequest", "path"): ("Setting", "The setting's path, as get_registered_settings lists it."),
    ("GetParametersRequest", "type"): ("Object type", "The kind of object whose parameters to read."),
    ("GetParametersRequest", "id"): ("Object ID", "The ID of the object whose parameters to read."),
    ("GetParameterValueRequest", "id"): ("Object ID", "The ID of the object."),
    ("SetTestParameterValueRequest", "id"): ("Object ID", "The ID of the object."),
    ("SetTestParameterValueRequest", "value"): ("Value", "JSON object of the parameter's new value (ObjectData)."),
    ("EnableDisablePluginRequestResponse", "enabled"): ("Enabled", "Whether to enable (true) or disable (false) the plugin; ignored when reading."),
    ("AllResourcesRequestResponse", "dataType"): ("Resource kind", "The kind of resource to list: ICON, DEVICE_IMAGE, PICTURE, DDF, SYMBOL, SOUND, LOG or BINARY; give this or a resource store."),
    ("AllResourcesRequestResponse", "resourceType"): ("Resource store", "The store to list: APPLICATION, PROJECT, TEMP, KERNEL_TEMP or CONFIG; give this or a resource kind."),
    ("ResourceExistsRequest", "name"): ("Name", "The resource's name."),
    ("ResourceLoadRequest", "name"): ("Name", "The resource's name."),
    ("ResourceLoadRequest", "source"): ("Source", "DEFAULT, PREFER_APPLICATION_STORE or KERNEL: where to look."),
    ("ResourceSaveRequest", "name"): ("Name", "The resource's name."),
    ("BroadcastMessage", "receiverType"): ("Receiver type", "The kind of program to send to."),
    ("ClientControlRequest", "source"): ("Source", "JSON object of this client's information (ClientInfo); filled in when left out."),
    ("SetNetworkPropertyRequest", "source"): ("Source", "JSON object of this client's information (ClientInfo); filled in when left out."),
    ("GetLogDataRequest", "logCount"): ("Entries", "How many of the most recent log entries to read."),
    ("FannedValuesRequest", "type"): ("What", "CONTAINER, OPERATOR or CONTAINERTYPE: what to list."),
    ("ValidParameteredFannedValueRequest", "fpv"): ("Fanned value", "JSON object of the fanned property value to check (FannedPropertyValue)."),
    ("ProjectCreationRequest", "copyFrom"): ("Copy from", "JSON object of the project version to copy (ProjectVersionID); none for a new empty project."),
    ("SaveProjectVersionRequest", "isAutosave"): ("Autosave", "Whether to save as an autosave."),
    ("UserContextDestructionRequest", "userContextId"): ("User context", "The user context to end, as bind_user_context returned it."),
    ("TrackAddChildrenRequest", "ticks"): ("Position", "Where to add them on the track, in ticks."),
    ("PresetCreationRequest", "predicate"): ("Filter", "JSON object of the programmer filter for what to store; give this or a preset to copy."),
    ("PresetReplaceSubtractRequest", "dependencies"): ("Dependencies", "JSON object of the dependencies, as a refused replacement answered them, each with its solution chosen; none for the first try."),
    ("DMXIn2OutMappingRequestResponse", "mapping"): ("Mapping", "JSON array of mappings, each {inAddress, outAddress, mappingMode}: the whole mapping, replacing the one there."),
    ("ConnectionSetCreationRequest", "targetBankId"): ("Bank", "The ID of the bank to create it in."),
    ("ConnectionSetMoveRequest", "targetBankId"): ("Bank", "The ID of the bank to move them to."),
    ("InputSinkProxyRequest", "changeType"): ("Change", "Added, Changed or Removed: what to do with the sink."),
    ("InputSourceProxyRequest", "changeType"): ("Change", "Added, Changed or Removed: what to do with the source; Changed with a value sends the value."),
    ("InputSourceProxyRequest", "source"): ("Source", "JSON object of the source (InputSourceDescriptor: element and receivesFeedback)."),
    ("InputSourceProxyRequest", "runtimeId"): ("Runtime ID", "This client's runtime ID, as session.runtime_id in state holds it."),
    ("InputSinkProxyRequest", "runtimeId"): ("Runtime ID", "This client's runtime ID, as session.runtime_id in state holds it."),
    ("MacroBoardProfileCreationRequest", "macroBoard"): ("Macro board", "JSON object of the board {id, width, height} to make a profile for."),
    ("CopyProgrammerValuesRequest", "fromGroupId"): ("From group", "The group whose values to copy; empty for every group."),
    ("CopyProgrammerValuesRequest", "targetGroupId"): ("To group", "The group to copy them to; empty for the same group."),
    ("DeleteProgrammerValueRequest", "groupId"): ("Group ID", "The group whose values to delete; empty for none."),
    ("DeleteProgrammerValueRequest", "deviceId"): ("Device ID", "The device whose values to delete; empty for none."),
    ("DeleteProgrammerValueRequest", "entryId"): ("Entry ID", "The programmer entry to delete; empty for none."),
    ("DeleteProgrammerValueRequest", "propertyIds"): ("Properties", "JSON array of the property IDs whose values to delete; empty for every property."),
    ("DeleteProgrammerValueRequest", "propertyTypes"): ("Property types", "JSON array of the property type numbers whose values to delete; empty for every type."),
    ("DeleteProgrammerAttachableRequest", "attachableId"): ("Attachable ID", "The ID of the attachable to remove from the programmer."),
    ("ExecuteOrModifyCommandLineCommandRequest", "executionType"): ("Execution", "No (only edit the command line), DryRun (check it) or Run (run it)."),
    ("StartPreviewPositionMappingRequest", "devices"): ("Devices", "JSON array of {deviceId, intensity, position: {pan, tilt, panset, tiltset}} to preview."),
    ("ItemListDeletionRequest", "itemListId"): ("Item list ID", "The ID of the item list to delete."),
    ("SetUserDefinedAffinityListRequest", "itemListId"): ("Item list", "The ID of the item list to use as the affinity list; empty to remove it."),
    ("GetAffinityListRequestResponse", "referenceObjectId"): ("Reference object", "The ID of the reference object whose affinity list to read."),
    ("EditCueInProgrammerRequest", "cuelistId"): ("Cuelist ID", "The ID of the cuelist the cue is in."),
    ("UpdateOriginOfProgrammerRequest", "dependencies"): ("Dependencies", "JSON object of the dependencies, as a refused update answered them, each with its solution chosen; none for the first try."),
    ("StreamDataRequest", "streamId"): ("Stream ID", "The stream's ID, as load_resource or save_resource answered it."),
}

# Get-style requests take the object they read in requestId (the reference
# client reads a program by its runtime ID that way); the command's `id`
# parameter fills it, described per command.
GET_REQUEST_IDS = {
    "get_affinity_class_reference_list": ("Affinity class", "The affinity class, as get_affinity_classes names it."),
    "get_user_defined_affinity_lists": ("Affinity class", "The affinity class, as get_affinity_classes names it."),
    "get_attachable_preset": ("Preset ID", "The saved attachable preset's ID."),
    "get_dmx_interface_enabled": ("Interface ID", "The DMX interface's ID."),
    "is_dmx_interface_available": ("Interface ID", "The DMX interface's ID."),
    "get_dmx_interface_parameters": ("Interface ID", "The DMX interface's ID."),
    "get_possible_items": ("List type", "The item list type, as get_available_item_list_types names it."),
    "get_connection_set_preview_values": ("Connection set ID", "The connection set's ID."),
    "get_cuelist_progress": ("Cuelist ID", "The cuelist's ID."),
    "rdm_discovery": ("Interface ID", "The DMX interface to discover on; empty for every one."),
    "rdm_request_device_info": ("RDM device", "The RDM device's UID, as rdm_devices in state keys it."),
    "rdm_request_sensors": ("RDM device", "The RDM device's UID, as rdm_devices in state keys it."),
    "rdm_request_parameters": ("RDM device", "The RDM device's UID, as rdm_devices in state keys it."),
    "get_client_program_info": ("Runtime ID", "The program's runtime ID, as clients in state keys it."),
}

# Get-style requests that need no ID: the parameter is left out.
GET_REQUEST_NO_ID = {
    "get_affinity_classes", "get_manual_beat", "get_manual_beat_factor", "get_current_command_line",
    "get_command_line_history", "get_dmx_in2out_mapping", "get_available_dmx_interfaces", "get_available_devices",
    "get_available_nodes", "get_available_item_list_types", "get_monitor_slots", "get_dmx_statistics",
    "get_transfer_statistics", "get_project_state", "get_registered_settings", "get_umbra_network_info",
    "reset_application_settings",
}

ACRONYMS = {"id": "ID", "ids": "IDs", "dmx": "DMX", "rdm": "RDM", "ui": "UI", "utc": "UTC", "crc": "CRC",
            "ttl": "TTL", "aq": "AQ", "tc": "TC", "dpv": "DPV", "fpv": "FPV", "hal": "HAL", "ddf": "DDF"}


def label_of(pname: str) -> str:
    parts = [ACRONYMS.get(w, w) for w in pname.split("_")]
    parts[0] = parts[0][:1].upper() + parts[0][1:]
    return " ".join(parts)


INT_RANGES = {"int32": (-2147483648, 2147483647), "sint32": (-2147483648, 2147483647),
              "sfixed32": (-2147483648, 2147483647), "uint32": (0, 4294967295), "fixed32": (0, 4294967295),
              "uint64": (0, None), "fixed64": (0, None)}


def yq(s: str) -> str:
    import json
    return json.dumps(s, ensure_ascii=False)


def plain(s: str) -> str:
    ok = re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ()'.+-]*", s) and s.lower() not in YAML_WORDS
    return s if ok else yq(s)


# ---------------------------------------------------------------------------
# State collections the module keeps current: each object of a list, keyed
# by its ID, read on connecting and kept current from its change stream.
#   key: state key; list: method reading the whole list, and the response's
#   list field; id: the JSON path of each item's ID; changes: the change
#   stream, the message's ID, data and change-type fields (None for none),
#   or "reread" when the message carries no data and the list is read again.
COLLECTIONS = [
    ("attachables", "AttachableClient.GetAttachables", "attachables", ["id"],
     ("AttachableClient.ReceiveAttachableChanges", None, None, None)),
    ("audio_players", "AudioClient.GetPlayerThreads", "playerThreads", ["id"],
     ("AudioClient.ReceivePlayerThreadChanges", "playerThreadId", "PlayerThreadData", "changeType")),
    ("background_tasks", None, None, ["id"],
     ("BackgroundTaskClient.ReceiveBackgroundTaskChanges", "id", "backgroundTaskData", "changeType")),
    ("banks", "InputClient.GetBanks", "banks", ["id"],
     ("InputClient.ReceiveBankChanges", "bankId", "bank", "changeType")),
    ("connection_sets", "InputClient.GetConnectionSets", "connectionSets", ["id"],
     ("InputClient.ReceiveConnectionSetChanges", "connectionSetId", "set", "changeType")),
    ("cue_triggers", "CueTriggerClient.GetCueTriggers", "triggers", ["name"], None),
    ("cuelist_groups", "CuelistGroupClient.GetCuelistGroups", "cuelistgroups", ["id"],
     ("CuelistGroupClient.ReceiveCuelistGroupChanges", "cuelistGroupId", "cuelistGroupData", "changeType")),
    ("cuelists", "CuelistClient.GetCuelists", "cuelists", ["id"],
     ("CuelistClient.ReceiveCuelistChanges", "cuelistId", "cuelistData", "changeType")),
    ("default_configurations", "SettingsClient.GetDefaultConfiguration", "requestedData", ["id"],
     ("SettingsClient.ReceiveDefaultConfigurationChanges", "defaultConfigurationId", "defaultConfigurationData", "type")),
    ("device_groups", "DeviceClient.GetDeviceGroups", "deviceGroups", ["id"],
     ("DeviceClient.ReceiveDeviceGroupChanges", "deviceGroupId", "deviceGroupData", "changeType")),
    ("devices", "DeviceClient.GetDevices", "devices", ["id"],
     ("DeviceClient.ReceiveDeviceChanges", "deviceId", "deviceData", "changeType")),
    ("dmx_interfaces", "DMXInterfaceManagementClient.GetUsedInterfaces", "interfaces", ["interface_data", "interface_id"], None),
    ("dmx_interfaces_available", "DMXInterfaceManagementClient.GetAvailableInterfaces", "interfaces", ["interface_data", "interface_id"], None),
    ("executor_member_wrappers", "ExecutorClient.GetExecutorMemberWrappers", "executorMemberWrappers", ["id"],
     ("ExecutorClient.ReceiveExecutorMemberWrapperChanges", "executorMemberWrapperId", "executorMemberWrapperData", "changeType")),
    ("executor_pages", "ExecutorClient.GetExecutorPages", "executorPages", ["id"],
     ("ExecutorClient.ReceiveExecutorPageChanges", "executorPageId", "executorPageData", "changeType")),
    ("executors", "ExecutorClient.GetExecutors", "executors", ["id"],
     ("ExecutorClient.ReceiveExecutorChanges", "executorId", "executorData", "changeType")),
    ("input_class_providers", "InputClient.GetInputClassProviders", "providers", ["id"],
     ("InputClient.ReceiveInputClassProviderChanges", "inputClassProviderId", "provider", "changeType")),
    ("input_nodes_available", "InputClient.GetAvailableNodes", "nodes", ["type_id"], None),
    ("input_sinks", "InputClient.GetInputSinks", "sinks", ["element", "id"],
     ("InputClient.ReceiveInputSinkChanges", "inputSinkId", "sink", "changeType")),
    ("input_sources", "InputClient.GetInputSources", "sources", ["element", "id"],
     ("InputClient.ReceiveInputSourceChanges", "inputSourceId", "source", "changeType")),
    ("item_lists", "ItemListClient.GetItemLists", "itemLists", ["id"],
     ("ItemListClient.ReceiveItemListChanges", "itemListId", "itemListData", "changeType")),
    ("macro_board_profile_templates", "MacroBoardClient.GetMacroBoardProfileTemplates", "profiles", ["id"], None),
    ("macro_board_profiles", "MacroBoardClient.GetMacroBoardProfiles", "profiles", ["id"],
     ("MacroBoardClient.ReceiveMacroBoardProfileChanges", "macroBoardProfileId", "macroBoardProfileData", "changeType")),
    ("macro_boards", "MacroBoardClient.GetMacroBoards", "macroBoards", ["id"],
     ("MacroBoardClient.ReceiveMacroBoardChanges", "macroBoardId", "macroBoardData", "changeType")),
    ("macros", "MacroClient.GetMacros", "macros", ["id"],
     ("MacroClient.ReceiveMacroChanges", "macroId", "macroData", "changeType")),
    ("main_switches", "MainSwitchClient.GetMainSwitches", "mainSwitches", ["main_switch_id"], None),
    ("markers", "TimecodeClient.GetMarkers", "markers", ["id"],
     ("TimecodeClient.ReceiveMarkerChanges", "markerId", "markerData", "changeType")),
    ("masters", "MasterClient.GetMasters", "masters", ["master_id"],
     ("MasterClient.ReceiveMasterChanges", "masterId", "masterData", "changeType")),
    ("plugins", "PluginManagementClient.GetPlugins", "plugins", ["id"], None),
    ("power_sources", "ElectricityClient.GetPowerSources", "powerSources", ["id"],
     ("ElectricityClient.ReceivePowerSourceChanges", "powerSourceId", "powerSourceData", "changeType")),
    ("presets", "PresetClient.GetPresets", "presets", ["preset_id"],
     ("PresetClient.ReceivePresetChanges", "presetId", "presetData", "changeType")),
    ("project_todos", "ProjectClient.GetProjectTodos", "todos", ["id"],
     ("ProjectClient.ReceiveProjectTodoChanges", "todoId", "todo", "changeType")),
    ("projects", "ProjectClient.GetProjects", "projects", ["global", "guid"],
     ("ProjectClient.ReceiveProjectChanges", None, None, None)),
    ("rdm_devices", "RDMClient.GetRDMDevices", "rdmDevices", ["uid"],
     ("RDMClient.ReceiveRDMDeviceChanges", "rdmDeviceId", "rdmDeviceData", "changeType")),
    ("registered_settings", "SettingsClient.GetRegisteredSettings", "settings", ["path"], None),
    ("snips", "TimecodeClient.GetSnips", "snips", ["id"],
     ("TimecodeClient.ReceiveSnipChanged", "snipId", "snipData", "changeType")),
    ("sound_outputs", "AudioClient.RequestAllSoundOutputInfos", "soundOutputInfos", ["guid"], None),
    ("stage_views", "StageViewClient.GetStageViews", "stageViews", ["id"],
     ("StageViewClient.ReceiveStageViewChanges", "stageViewId", "stageViewData", "changeType")),
    ("timecodes", "TimecodeClient.GetTimecodes", "timecodes", ["id"],
     ("TimecodeClient.ReceiveTimecodeChanges", "timecodeId", "timecodeData", "changeType")),
    ("tracks", "TimecodeClient.GetTracks", "tracks", ["id"],
     ("TimecodeClient.ReceiveTrackChanges", "trackId", "trackData", "changeType")),
]

# What each collection holds, for its state description.
COLLECTION_TEXT = {
    "attachables": "attachable (an effect or other item attachable to a device property)",
    "audio_players": "audio player",
    "background_tasks": "background task (an import, a save, an analysis), as it reports changes",
    "banks": "input bank",
    "connection_sets": "input connection set",
    "cue_triggers": "cue trigger kind, keyed by name",
    "cuelist_groups": "cuelist group",
    "cuelists": "cuelist, with its cues",
    "default_configurations": "default configuration",
    "device_groups": "device group",
    "devices": "device, with its beams, properties and procedures",
    "dmx_interfaces": "DMX interface the project uses",
    "dmx_interfaces_available": "DMX interface DMXControl can use",
    "executor_member_wrappers": "object an executor can hold",
    "executor_pages": "executor page",
    "executors": "executor",
    "input_class_providers": "input class provider",
    "input_nodes_available": "node type a connection set can use, keyed by type ID",
    "input_sinks": "input sink",
    "input_sources": "input source",
    "item_lists": "item list",
    "macro_board_profile_templates": "macro board profile template",
    "macro_board_profiles": "macro board profile",
    "macro_boards": "macro board",
    "macros": "macro",
    "main_switches": "main switch",
    "markers": "timecode marker",
    "masters": "master",
    "plugins": "plugin",
    "power_sources": "power source",
    "presets": "preset",
    "project_todos": "project to-do",
    "projects": "project, keyed by its GUID, with its versions",
    "rdm_devices": "RDM device, keyed by UID",
    "registered_settings": "registered setting, keyed by path",
    "snips": "sound snip",
    "sound_outputs": "sound output, keyed by GUID",
    "stage_views": "stage view",
    "timecodes": "timecode show",
    "tracks": "timecode track",
}


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    protos = Protos()
    files = sorted(Path(sys.argv[1]).rglob("*.proto"))
    if not files:
        print(f"no .proto files under {sys.argv[1]}")
        return 1
    for f in files:
        protos.parse(f)
    protos.link()

    # Methods by "Service.Method".
    methods = {}
    for full, ms in protos.services.items():
        for name, tin, tout, cs, ss in ms:
            methods[f"{short(full)}.{name}"] = (full, name, tin, tout, cs, ss)

    # Every message's fields with their JSON names, checked unique.
    for full, fields in protos.messages.items():
        names = [snake(f["name"]) for f in fields]
        assert len(names) == len(set(names)), (full, names)

    commands = []
    for full, ms in sorted(protos.services.items()):
        if not full.startswith(CLIENT_SERVICES):
            continue
        for name, tin, tout, cs, ss in ms:
            key = f"{short(full)}.{name}"
            if key in MANAGED or name.startswith(("Receive", "Recieve")) or key.startswith("ClientService."):
                continue
            command = RENAMES.get(key, snake(name))
            commands.append((command, key, full, name, tin, tout, cs, ss))
    names = [c[0] for c in commands]
    assert len(names) == len(set(names)), sorted(n for n in names if names.count(n) > 1)
    missing = [n for n in names if n not in SUMMARIES]
    assert not missing, f"no summary for {missing}"
    extra = [n for n in SUMMARIES if n not in names]
    assert not extra, f"summary for no command: {extra}"
    commands.sort()

    for key, list_method, list_field, id_path, changes in COLLECTIONS:
        if list_method:
            assert list_method in methods, list_method
        if changes:
            assert changes[0] in methods, changes[0]

    write_schema(protos, methods, commands)
    write_spec(protos, methods, commands)
    print(f"{len(commands)} commands, {len(protos.messages)} messages, {len(protos.enums)} enums")
    return 0


KINDS = {"double": "Double", "float": "Float", "int32": "Int32", "int64": "Int64", "uint32": "Uint32",
         "uint64": "Uint64", "sint32": "Sint32", "sint64": "Sint64", "fixed32": "Fixed32", "fixed64": "Fixed64",
         "sfixed32": "Sfixed32", "sfixed64": "Sfixed64", "bool": "Bool", "string": "String", "bytes": "Bytes"}


def kind(protos, t: str) -> str:
    if t in KINDS:
        return f"Kind::{KINDS[t]}"
    if t in protos.enums:
        return f'Kind::Enum("{t}")'
    return f'Kind::Message("{t}")'


def write_schema(protos, methods, commands):
    out = [
        "//! DMXControl 3's messages, enums, command methods and state collections,",
        "//! generated by tools/generate_dmxcontrol3.py from the .proto files of",
        "//! DMXControl's official network interface repository",
        "//! (github.com/DMXControl/DMXControl3-Network-Interface). Do not edit.",
        "",
        "use super::dmxcontrol3::{Changes, Collection, Method};",
        "use crate::protobuf::{EnumDef, Field, Kind, Label, MessageDef, Schema};",
        "",
        "pub(crate) const SCHEMA: Schema = Schema {",
        "    messages: MESSAGES,",
        "    enums: ENUMS,",
        "};",
        "",
        "static MESSAGES: &[MessageDef] = &[",
    ]
    for full in sorted(protos.messages):
        fields = protos.messages[full]
        out.append("    MessageDef {")
        out.append(f'        name: "{full}",')
        if not fields:
            out.append("        fields: &[],")
        else:
            out.append("        fields: &[")
            for f in fields:
                if f["label"] == "map":
                    label = f"Label::Map({kind(protos, f['key'])})"
                elif f["label"] == "repeated":
                    label = "Label::Repeated"
                else:
                    label = "Label::Single"
                oneof = f'Some("{f["oneof"]}")' if f["oneof"] else "None"
                out.append(
                    f'            Field {{ name: "{f["name"]}", json: "{snake(f["name"])}", number: {f["number"]}, '
                    f'kind: {kind(protos, f["type"])}, label: {label}, oneof: {oneof} }},')
            out.append("        ],")
        out.append("    },")
    out.append("];")
    out.append("")
    out.append("static ENUMS: &[EnumDef] = &[")
    for full in sorted(protos.enums):
        values = ", ".join(f'("{n}", {v})' for n, v in protos.enums[full])
        out.append(f'    EnumDef {{ name: "{full}", values: &[{values}] }},')
    out.append("];")
    out.append("")
    out.append("/// Every command that calls a method, sorted by command.")
    out.append("pub(crate) static METHODS: &[Method] = &[")
    for command, key, full, name, tin, tout, cs, ss in commands:
        out.append(
            f'    Method {{ command: "{command}", path: "/{full}/{name}", request: "{tin}", response: "{tout}", '
            f'client_streaming: {str(cs).lower()}, server_streaming: {str(ss).lower()} }},')
    out.append("];")
    out.append("")
    out.append("/// The state collections, sorted by key.")
    out.append("pub(crate) static COLLECTIONS: &[Collection] = &[")
    for key, list_method, list_field, id_path, changes in COLLECTIONS:
        if list_method:
            full, name, tin, tout, _, _ = methods[list_method]
            list_text = f'Some(("/{full}/{name}", "{tin}", "{tout}", "{snake(list_field)}"))'
        else:
            list_text = "None"
        if changes:
            full, name, tin, tout, _, _ = methods[changes[0]]
            path = f"/{full}/{name}"
            if changes[1] is None:
                changes_text = f'Some(Changes::Reread {{ path: "{path}", message: "{tout}" }})'
            else:
                change_type = f'Some("{snake(changes[3])}")' if changes[3] else "None"
                changes_text = (f'Some(Changes::Data {{ path: "{path}", message: "{tout}", id: "{snake(changes[1])}", '
                                f'data: "{snake(changes[2])}", change_type: {change_type} }})')
        else:
            changes_text = "None"
        ids = ", ".join(f'"{p}"' for p in id_path)
        out.append(f'    Collection {{ key: "{key}", list: {list_text}, id: &[{ids}], changes: {changes_text} }},')
    out.append("];")
    SCHEMA.write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")


# Words a YAML 1.1 reader takes for something other than text.
YAML_WORDS = {"true", "false", "yes", "no", "on", "off", "none", "null", "y", "n"}

# The fields naming the one object a command acts on: required wherever
# they are not a choice (a oneof) and the command neither reads a list nor
# creates.
TARGET_IDS = {
    "backgroundTaskId", "bankId", "className", "connectionSetId", "cueEntryId", "cueId", "cuelistGroupId",
    "cuelistId", "deviceGroupId", "deviceId", "executorId", "executorPageId", "interfaceId", "itemListId",
    "macroBoardId", "macroId", "mainSwitchId", "markerId", "masterId", "path", "pluginId", "powerSourceId",
    "presetId", "profileId", "snipId", "stageViewId", "streamId", "timecodeId", "todoId", "trackId",
}
OPTIONAL_TARGETS = {
    ("DeleteProgrammerValueRequest", "deviceId"), ("CuelistProgressStateChangeRequest", "cuelistIds"),
}

# Fields of a request-and-response message that only the answer fills.
ANSWER_ONLY = {
    ("ProgrammerPropertyRequestResponse", "notFound"), ("ProgrammerPropertyRequestResponse", "value"),
    ("GetProgrammerSnapshotsRequestResponse", "snapshots"),
}


def describe_field(message: str, field: str):
    entry = FIELDS.get((short(message), field)) or FIELDS.get(field)
    if entry is None:
        raise KeyError(f"no description for {short(message)}.{field}")
    return entry


def write_spec(protos, methods, commands):
    lines = []
    for command, key, full, name, tin, tout, cs, ss in commands:
        fields = protos.messages[tin]
        get_style = short(tin) in ("GetRequest", "GetMultipleRequest", "ActionRequest")
        params = []
        for f in fields:
            pname = snake(f["name"])
            if f["name"] == "requestId":
                if not get_style or command in GET_REQUEST_NO_ID:
                    continue
                if command not in GET_REQUEST_IDS:
                    continue
                label, desc = GET_REQUEST_IDS[command]
                pname = "id"
                spec = ["type: string"]
            elif (short(tin), f["name"]) in ANSWER_ONLY:
                continue
            else:
                label, desc = describe_field(tin, f["name"])
                t = f["type"]
                if f["label"] != "single":
                    spec = ["type: json"]
                elif t in protos.enums:
                    values = ", ".join(n if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", n) and n.lower() not in YAML_WORDS else yq(n) for n, _ in protos.enums[t])
                    spec = ["type: enum", f"values: [{values}]"]
                elif t in protos.messages:
                    spec = ["type: json"]
                elif t == "bool":
                    spec = ["type: bool"]
                elif t in ("double", "float"):
                    spec = ["type: float"]
                elif t == "string":
                    spec = ["type: string"]
                elif t == "bytes":
                    spec = ["type: string", "pattern: '^[A-Za-z0-9+/]*={0,2}$'"]
                else:
                    spec = ["type: int"]
                    lo, hi = INT_RANGES.get(t, (None, None))
                    if lo is not None:
                        spec.append(f"min: {lo}")
                    if hi is not None:
                        spec.append(f"max: {hi}")
            if (f["name"] in TARGET_IDS and not f["oneof"] and f["label"] == "single"
                    and (short(tin), f["name"]) not in OPTIONAL_TARGETS and not command.startswith(("get_", "create_"))):
                spec.append("required: true")
            spec.append(f"label: {plain(label)}")
            spec.append(f"description: {yq(desc)}")
            params.append(f"      {pname}: {{ {', '.join(spec)} }}")
        returns = "ack" if short(tout) == "ConfirmedResponse" else "value"
        summary = SUMMARIES[command]
        if ss and not cs:
            summary += " (a server-streaming call)"
        elif cs:
            summary += " (one request on a bidirectional call; the answer is every message the server sends back)"
        block = [f"  {command}:", f"    summary: {yq(summary)}"]
        if params:
            block.append("    params:")
            block.extend(params)
        block.append(f"    returns: {returns}")
        lines.append("\n".join(block))

    # Commands the module answers itself.
    lines.append("\n".join([
        "  send_broadcast:",
        '    summary: "Send a broadcast to the programs of the DMXControl network, on the session\'s broadcast call (sent; nothing confirms it)"',
        "    params:",
        '      message_type: { type: string, max_length: 1024, required: true, label: Message type, description: "The broadcast\'s type, which receivers use to tell broadcasts apart." }',
        '      string_content: { type: string, multiline: true, default: "", label: Text, description: "The broadcast\'s content as text; empty sends none." }',
        "      receiver_hostnames: { type: json, default: [], label: Receivers, description: \"JSON array of the host names to send to; empty for every program.\" }",
        "      receiver_type: { type: enum, values: [EClientType_Unknown, Kernel, GUI, DMXInterfaceBroker, ExternalTool, Umbra], default: EClientType_Unknown, label: Receiver type, description: \"The kind of program to send to; EClientType_Unknown for every kind.\" }",
        "    returns: none",
    ]))
    names = sorted([c[0] for c in commands] + ["send_broadcast"])

    text = SPEC.read_text(encoding="utf-8")
    head, rest = text.split("\ncommands:\n", 1)
    tail = rest[rest.index("\nstate:\n"):]
    # The generated state goes after the marker; what is above it is kept.
    marker = "  # Collections (generated by tools/generate_dmxcontrol3.py)\n"
    tail = tail[: tail.index(marker)] + marker + "\n".join(state_lines(protos, methods)) + "\n"

    supports, line = [], "    supports: ["
    for i, n in enumerate(names):
        piece = n + (", " if i < len(names) - 1 else "]")
        if len(line) + len(piece) > 110:
            supports.append(line.rstrip())
            line = "               "
        line += piece
    supports.append(line)
    head = re.sub(r"    supports: \[.*?\]\n", lambda _: "\n".join(supports) + "\n", head, count=1, flags=re.S)
    lines.sort(key=lambda b: b.split(":", 1)[0])
    SPEC.write_text(head + "\ncommands:\n" + "\n".join(lines) + "\n" + tail, encoding="utf-8", newline="\n")


def state_type(protos, f) -> str:
    if f["label"] == "repeated":
        return "array"
    if f["label"] == "map" or f["type"] in protos.messages:
        return "object"
    if f["type"] in protos.enums or f["type"] in ("string", "bytes"):
        return "string"
    if f["type"] == "bool":
        return "bool"
    if f["type"] in ("double", "float"):
        return "float"
    return "int"


def state_lines(protos, methods):
    out = []
    for key, list_method, list_field, id_path, changes in COLLECTIONS:
        if list_method:
            tout = methods[list_method][3]
            item = next(f["type"] for f in protos.messages[tout] if snake(f["name"]) == snake(list_field))
        else:
            tout = methods[changes[0]][3]
            item = next(f["type"] for f in protos.messages[tout] if f["name"] == changes[2])
        what = COLLECTION_TEXT[key]
        idtext = ".".join(id_path)
        how = []
        if list_method:
            how.append(f"read with {snake(list_method.split('.')[1])} on connecting")
        if changes:
            how.append(f"kept current from {changes[0].split('.')[1]}")
        out.append(f"  {key}.*: {{ type: object, description: {yq(f'Each {what}, keyed by its {idtext}: {short(item)} with its fields in snake_case; ' + ', '.join(how))} }}")
        for f in protos.messages[item]:
            t = state_type(protos, f)
            fname = snake(f["name"])
            note = ""
            if f["type"] in protos.enums:
                values = ", ".join(n for n, _ in protos.enums[f["type"]])
                note = f": one of {values}"
            elif f["type"] in protos.messages:
                note = f" ({short(f['type'])})"
            if f["oneof"]:
                note += f"; present only when it is the {f['oneof']} given"
            singular = what.split(" (")[0].split(",")[0]
            out.append(f"  {key}.*.{fname}: {{ type: {t}, description: {yq(f'The {fname} field of each {singular}{note}')} }}")
    return out


if __name__ == "__main__":
    raise SystemExit(main())
