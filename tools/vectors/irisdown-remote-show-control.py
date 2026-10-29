# Irisdown Remote Show Control (irisdown-remote-show-control): one vector
# per command, one UDP datagram each, written by hand from the "Remote Show
# Control v1.2.4" manual's syntax and examples (UTF-8 text, no line ending
# documented, none sent).
RSC = "irisdown-remote-show-control"
text(RSC, "open", {"file": "test.pptx"}, 'OPEN "test.pptx"')
text(RSC, "close", {"file": r"c:\users\user\documents\test.pptx"}, r'CLOSE "c:\users\user\documents\test.pptx"')
text(RSC, "close_all", {}, "CLOSE")
text(RSC, "run", {"file": "test.pptx"}, 'RUN "test.pptx"')
text(RSC, "run_active", {}, "RUN")
text(RSC, "run_current", {}, "RUNCURRENT")
text(RSC, "stop", {"file": "test.pptx"}, 'STOP "test.pptx"')
text(RSC, "stop_all", {}, "STOP")
text(RSC, "next", {}, "NEXT")
text(RSC, "previous", {}, "PREV")
text(RSC, "go_to_slide", {"slide": 23}, "GO 23")
text(RSC, "go_to_section", {"section": "Introduction"}, 'GO "Introduction"')
text(RSC, "set_background", {}, "SETBG")

# FIND answers the folder's presentations, one per line, or "none" (manual,
# FIND); taken as one datagram back to the sender.
_FOUND = "c:\\presentations\\presentation1.pptx\r\nc:\\presentations\\presentation2.pptx\r\n"
text(RSC, "find", {"path": r"c:\presentations"}, r'FIND "c:\presentations"',
     device_reply_hex=hexs(_FOUND.encode()),
     expect_result={"ok": {"kind": "value", "value": _FOUND.rstrip("\r\n")}})
telemetry(RSC, "find-list", request=r'FIND "c:\presentations"', inbound=_FOUND,
          expect_state={"folders": {r"c:\presentations": {"files": {
              "0": r"c:\presentations\presentation1.pptx", "1": r"c:\presentations\presentation2.pptx"}}}})
telemetry(RSC, "find-none", request=r'FIND "d:\empty"', inbound="none\r\n",
          state_before={"folders": {r"d:\empty": {"files": {"0": r"d:\empty\old.pptx"}}}},
          expect_state={"folders": {r"d:\empty": {}}})
