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
