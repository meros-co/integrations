# Vizrt Viz Trio (vizrt-trio): one vector per command. Each line is the macro
# command as the Viz Trio 4.6 Overview of Commands writes it, group:command
# then the arguments separated by spaces, ended by a line feed. The example
# values below are the guide's own (page 1000, channel A, show variable
# MyShowVar) where it gives them. Commands with no result wait for no reply.
VT = "vizrt-trio"
_VT_EX = {"page": "1000", "channel": "A", "action": "take", "clear": True, "name": "MyShowVar", "value": "Hello World!",
          "description": "Election night", "effect": "", "status": "FINISHED", "start": 2000, "end": 2010,
          "number": 3, "property": "text", "table": "1", "column": "3", "row": 2, "count": 1, "rows": "2",
          "path": "/shows/Election", "concept": "Sport", "variant": "Lower", "variable": "platform",
          "playlist": "/playlists/News", "looping": "true", "manual": True, "profile": "Studio A",
          "hosts": "viz1,viz2", "clock": "Clock1", "command": "RENDERER*MAIN_LAYER SET_OBJECT SCENE*Election/Lower",
          "enabled": True}


def _vt_vectors():
    import re as _re
    doc = yaml.safe_load((ROOT / "specs" / "vizrt-trio.yaml").read_text(encoding="utf-8"))
    for name, c in doc["commands"].items():
        params = c.get("params") or {}
        inp = {}
        for p, s in params.items():
            v = _VT_EX.get(p)
            if s["type"] == "int":
                v = int(v) if v is not None else 1
                if name in ("page_read", "page_save_as", "page_set_status"):
                    v = 1000
            elif s["type"] == "string" and isinstance(v, (int, bool)):
                v = str(v)
            if name in ("show_page_exists",) and p == "page":
                v = "1000"
            if name in ("call_macro",):
                v = "Open Show"
            if name == "tabfield_tab_to_name":
                v = "Title"
            if p == "name" and name.startswith("page_") and "property" in name:
                v = "01"
            inp[p] = v

        def render(m):
            p, d = m.group(1), m.group(2)
            v = inp[p]
            if d == "upper":
                return str(v).upper()
            if isinstance(v, bool):
                return "true" if v else "false"
            return str(v)
        line = _re.sub(r"\{([a-z_]+)(?::([a-z]+))?\}", render, c["send"]) + "\n"
        extra = {}
        if c["returns"] == "value":
            extra = {"device_reply": "Election\n", "expect_result": {"ok": {"kind": "value", "value": "Election"}}}
        else:
            extra = {"expect_result": {"ok": {"kind": "unverified"}}}
        text(VT, name, inp, line, **extra)


_vt_vectors()
# An error answer to a get command (Overview: errors are ERROR: <message>).
text(VT, "show_get_variable", {"name": "Missing"}, "show:get_variable Missing\n",
     device_reply="ERROR: variable Missing not found\n", expect_result={"error": {"error": "device_error"}},
     file="show_get_variable-error")
# The guide's own example: show:get_variable MyShowVar -> Hello World!
text(VT, "show_get_variable", {"name": "MyShowVar"}, "show:get_variable MyShowVar\n",
     device_reply="Hello World!\n", expect_result={"ok": {"kind": "value", "value": "Hello World!"}},
     file="show_get_variable-guide")

# Telemetry: the polled reads, each tied to its request.
telemetry(VT, "channels-status", request="trio:check_channels_status", inbound="true\n",
          expect_state={"channels": {"connected": True}})
telemetry(VT, "show-name", request="show:get_show_name", inbound="Election 2026\n",
          expect_state={"show": {"name": "Election 2026"}})
telemetry(VT, "profile", request="channelcontrol:get_active_profile", inbound="Studio A\n",
          expect_state={"channels": {"profile": "Studio A"}})
telemetry(VT, "version", request="main:get_version", inbound="4.6.0.12345\n",
          expect_state={"system": {"version": "4.6.0.12345"}})
telemetry(VT, "error-is-no-name", request="show:get_show_name", inbound="ERROR: no show\n", expect_state={})
telemetry(VT, "pages", request="show:get_pages", inbound="1000 1010 2000\n",
          state_before={"show": {"pages": {"3": "3000"}}},
          expect_state={"show": {"pages": {"0": "1000", "1": "1010", "2": "2000"}}})
telemetry(VT, "templates", request="show:get_templates", inbound="5000,5010\n",
          expect_state={"show": {"templates": {"0": "5000", "1": "5010"}}})
telemetry(VT, "viz-channels", request="channelcontrol:list_viz_channels", inbound="PGM, PVW\n",
          expect_state={"channels": {"viz": {"0": "PGM", "1": "PVW"}}})
telemetry(VT, "pages-error", request="show:get_pages", inbound="ERROR: no show\n",
          state_before={"show": {"pages": {"0": "1000"}}}, expect_state={"show": {"pages": {"0": "1000"}}})
