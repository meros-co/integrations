DME = "yamaha-dme7"
# Yamaha DME7 Remote Control Protocol (DME7 Remote Control Protocol
# Specifications V1.1.0 rev2): LF-terminated ASCII lines; replies "OK ...",
# "OKm ..." (value adjusted) or "ERROR <command> <code>"; pushes "NOTIFY ...".
# Parameters are PROC:Remote/<index> on the Remote Control Setup List, with X
# and Y as on the wire. Examples are the document's own where it gives one.
OK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


# Device status and run mode (p.13-14)
text(DME, "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n', expect_result=val("normal"))
text(DME, "get_error_status", {}, "devstatus error\n",
     device_reply='OK devstatus error "err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"\n',
     expect_result=val("err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"))
text(DME, "get_power_supply_status", {"unit": "power2"}, "devstatus power2\n", device_reply='OK devstatus Power2 "down"\n',
     expect_result=val("down"))
text(DME, "set_run_mode", {"mode": "emergency"}, "devmode emergency\n", device_reply="OK devmode emergency\n", expect_result=OK)

# Protocol mode (p.14-15)
text(DME, "set_encoding", {"encoding": "utf8"}, "scpmode encoding utf8\n", device_reply="OK scpmode encoding utf8\n", expect_result=OK)
text(DME, "set_value_type", {"value_type": "normalized"}, "scpmode valuetype normalized\n",
     device_reply="OK scpmode valuetype normalized\n", expect_result=OK)
text(DME, "set_normalized_resolution", {"resolution": 128}, "scpmode resolution 128\n",
     device_reply="OK scpmode resolution 128\n", expect_result=OK)
text(DME, "set_keepalive", {"interval_ms": 2000}, "scpmode keepalive 2000\n", device_reply="OK scpmode keepalive 2000\n",
     expect_result=OK)

# Product information (p.24-28)
text(DME, "get_protocol_version", {}, "devinfo protocolver\n", device_reply='OK devinfo protocolver "1.0.0"\n', expect_result=val("1.0.0"))
text(DME, "get_parameter_set_version", {}, "devinfo paramsetver\n", device_reply='OK devinfo paramsetver "PROC:1.0.0"\n',
     expect_result=val("PROC:1.0.0"))
text(DME, "get_firmware_version", {}, "devinfo version\n", device_reply='OK devinfo version "1.0.0"\n', expect_result=val("1.0.0"))
text(DME, "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "DME7"\n', expect_result=val("DME7"))
text(DME, "get_manufacturer", {}, "devinfo manufacturer\n", device_reply='OK devinfo manufacturer "Yamaha Corporation"\n',
     expect_result=val("Yamaha Corporation"))
text(DME, "get_serial_number", {}, "devinfo serialno\n", device_reply='OK devinfo serialno "ZA37640CHNET101001"\n',
     expect_result=val("ZA37640CHNET101001"))
text(DME, "get_category", {}, "devinfo category\n", device_reply='OK devinfo category "processor"\n', expect_result=val("processor"))
text(DME, "get_device_id", {}, "devinfo deviceid\n", device_reply='OK devinfo deviceid "001"\n', expect_result=val("001"))
text(DME, "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "DME7xxx"\n', expect_result=val("DME7xxx"))
text(DME, "get_input_port_count", {}, "devinfo inputport\n", device_reply='OK devinfo inputport "64"\n', expect_result=val("64"))
text(DME, "get_output_port_count", {}, "devinfo outputport\n", device_reply="OK devinfo outputport 64\n", expect_result=val("64"))
text(DME, "get_gpi_count", {}, "devinfo gpi\n", device_reply="OK devinfo gpi 16\n", expect_result=val("16"))
text(DME, "get_gpo_count", {}, "devinfo gpo\n", device_reply="OK devinfo gpo 8\n", expect_result=val("8"))

# Remote Control Setup List information (p.28-30)
text(DME, "get_parameter_count", {}, "prmnum\n", device_reply="OK prmnum 1000\n", expect_result=val("1000"))
text(DME, "get_parameter_info", {"index": 5}, "prminfo 5\n", device_reply="ERROR prminfo InvalidArgument\n", expect_result=ERR)
text(DME, "get_meter_count", {}, "mtrnum\n", device_reply="OK mtrnum 1000\n", expect_result=val("1000"))
text(DME, "get_meter_info", {"index": 1}, "mtrinfo 1\n")
text(DME, "identify", {"seconds": 3}, "identify 3\n", device_reply="OK identify 3\n", expect_result=OK)

# Parameters (p.16-21): Ch 2 level of a Fader component at index 1
text(DME, "set_parameter", {"index": 1, "x": 2, "y": 0, "value": -7760}, "set PROC:Remote/1 2 0 -7760\n",
     device_reply='OK set PROC:Remote/1 2 0 -7760 "-77.60"\n', expect_result=OK)
text(DME, "set_parameter_normalized", {"index": 1, "value": 408}, "setn PROC:Remote/1 0 0 408\n",
     device_reply='OKm setn PROC:Remote/1 0 0 408 "-21.50"\n', expect_result=OK)
text(DME, "set_parameter_text", {"index": 1, "text": "10.0"}, 'sett PROC:Remote/1 0 0 "10.0"\n',
     device_reply='OK sett PROC:Remote/1 0 0 "10.0"\n', expect_result=OK)
text(DME, "set_parameter_relative", {"index": 1, "steps": 100}, "setr PROC:Remote/1 0 0 100\n",
     device_reply="OK setr PROC:Remote/1 0 0 -1900\n", expect_result=OK)
text(DME, "get_parameter", {"index": 1}, "get PROC:Remote/1 0 0\n", device_reply="OK get PROC:Remote/1 0 0 -7760\n",
     expect_result=val("-7760"))
text(DME, "get_parameter_normalized", {"index": 1}, "getn PROC:Remote/1 0 0\n", device_reply="OK getn PROC:Remote/1 0 0 408\n",
     expect_result=val("408"))
text(DME, "get_parameter_text", {"index": 1}, "gett PROC:Remote/1 0 0\n", device_reply='OK gett PROC:Remote/1 0 0 "10.0"\n',
     expect_result=val("10.0"))
text(DME, "get_parameter_all_x", {"index": 1, "y": 0}, "get PROC:Remote/1 all 0\n",
     device_reply="OK get PROC:Remote/1 all 0 -5000 -6000 -7000 -7760\n", expect_result=val("-5000 -6000 -7000 -7760"))
text(DME, "get_parameter_all_y", {"index": 3, "x": 1}, "get PROC:Remote/3 1 all\n")
text(DME, "get_parameter_normalized_all_x", {"index": 1, "y": 0}, "getn PROC:Remote/1 all 0\n",
     device_reply="OK getn PROC:Remote/1 all 0 408 500\n", expect_result=val("408 500"))
text(DME, "get_parameter_normalized_all_y", {"index": 3, "x": 0}, "getn PROC:Remote/3 0 all\n")
text(DME, "get_parameter_text_all_x", {"index": 1, "y": 0}, "gett PROC:Remote/1 all 0\n",
     device_reply='OK gett PROC:Remote/1 all 0 "-50.00" "-60.00"\n', expect_result=val('"-50.00" "-60.00"'))
text(DME, "get_parameter_text_all_y", {"index": 3, "x": 2}, "gett PROC:Remote/3 2 all\n")

# Meters (p.22)
text(DME, "start_meter", {"index": 2, "interval_ms": 1000}, "mtrstart PROC:Remote/2 1000\n", device_reply="OK mtrstart PROC:Remote/2\n",
     expect_result=OK)
text(DME, "start_meter_peak_hold", {"index": 2, "interval_ms": 1000}, "mtrstart PROC:Remote/2>PeakHold 1000\n",
     device_reply="OK mtrstart PROC:Remote/2>PeakHold\n", expect_result=OK)
text(DME, "stop_meter", {"index": 2}, "mtrstop PROC:Remote/2\n", device_reply="OK mtrstop PROC:Remote/2\n", expect_result=OK)
text(DME, "stop_meter_peak_hold", {"index": 2}, "mtrstop PROC:Remote/2>PeakHold\n", device_reply="OK mtrstop PROC:Remote/2>PeakHold\n",
     expect_result=OK)

# Snapshots (p.23, p.31)
text(DME, "get_current_parameter_set", {}, "sscurrent_ex parameter_set\n", device_reply="OK sscurrent_ex 2 unmodified\n",
     expect_result=val("2"))
text(DME, "get_current_snapshot", {"parameter_set": 2}, "sscurrent_ex 2\n", device_reply="OK sscurrent 2 10 modified\n",
     expect_result=val("10"))
text(DME, "recall_snapshot", {"parameter_set": 5000, "snapshot": 10}, "ssrecall_ex 5000 10\n", device_reply="OK ssrecall_ex 5000 10\n",
     expect_result=OK)
text(DME, "get_snapshot_count", {"parameter_set": 5000}, "ssnum_ex 5000\n", device_reply="OK ssnum_ex 100\n", expect_result=val("100"))
text(DME, "get_snapshot_info", {"parameter_set": 5000, "snapshot": 10}, "ssinfo_ex 5000 10\n")

# Audio Player (p.32-37)
text(DME, "set_audio_player_type", {"index": 1, "type": "1song"}, 'event PROC:AudioPlayerSetType "index=1" "type=1song"\n',
     device_reply='OK event PROC:AudioPlayerSetType "index=1" "type=1song"\n', expect_result=OK)
text(DME, "set_audio_player_path", {"index": 1, "path": "song.wav"}, 'event PROC:AudioPlayerSetPath "index=1" "path=song.wav"\n',
     device_reply='OK event PROC:AudioPlayerSetPath "index=1" "path=song.wav"\n', expect_result=OK)
text(DME, "set_audio_player_play_mode", {"index": 1, "mode": "shuffleRepeat"},
     'event PROC:AudioPlayerSetPlayMode "index=1" "mode=shuffleRepeat"\n')
text(DME, "set_audio_player_go_to_top", {"index": 1, "go_to_top": "off"}, 'event PROC:AudioPlayerSetGoToTheTop "index=1" "goToTheTop=off"\n',
     device_reply='OK event PROC:AudioPlayerSetGoToTheTop "index=1" "goToTheTop=off"\n', expect_result=OK)
text(DME, "set_audio_player_interval", {"index": 1, "interval": 3.0}, 'event PROC:AudioPlayerSetInterval "index=1" "interval=3.0"\n',
     device_reply='OK event PROC:AudioPlayerSetInterval "index=1" "interval=3.0"\n', expect_result=OK)
text(DME, "get_audio_player_status", {}, 'event PROC:AudioPlayerGetStatus ""\n',
     device_reply='OKm event PROC:AudioPlayerGetStatus "sdcard is not inserted"\n', expect_result=ERR)
text(DME, "audio_player_play", {}, 'event PROC:AudioPlayerTransport "operation=play"\n',
     device_reply='OKm event PROC:AudioPlayerTransport "song is not set up"\n', expect_result=ERR)
text(DME, "audio_player_stop", {}, 'event PROC:AudioPlayerTransport "operation=stop"\n',
     device_reply='OK event PROC:AudioPlayerTransport "operation=stop"\n', expect_result=OK)
text(DME, "audio_player_previous", {}, 'event PROC:AudioPlayerTransport "operation=prev"\n')
text(DME, "audio_player_next", {}, 'event PROC:AudioPlayerTransport "operation=next"\n')
text(DME, "get_audio_player_current_song", {}, 'event PROC:AudioPlayerGetCurrentSong ""\n',
     device_reply='OK event PROC:AudioPlayerSetCurrentSong "index=1"\n', expect_result=val("1"))
text(DME, "set_audio_player_current_song", {"index": 2}, 'event PROC:AudioPlayerSetCurrentSong "index=2"\n',
     device_reply='OK event PROC:AudioPlayerSetCurrentSong "index=2"\n', expect_result=OK)
text(DME, "get_audio_player_item_count", {}, 'listitemnum PROC:AudioPlayer ""\n',
     device_reply="OK listitemnum PROC:AudioPlayer 100 1 100\n", expect_result=val("100"))
text(DME, "get_audio_player_item", {"index": 1}, "listitem PROC:AudioPlayer 1\n",
     device_reply='OK listitem PROC:AudioPlayer 1 "1Song Normal On 1.0 xxx.wav"\n', expect_result=val("1Song Normal On 1.0 xxx.wav"))

# Display font and Scheduler (p.35-38)
text(DME, "set_display_font", {"font": "type1"}, 'event PROC:Language "type1"\n', device_reply='OK event PROC:Language "type1"\n',
     expect_result=OK)
text(DME, "set_scheduler_event_enabled", {"index": 2, "enabled": True}, 'event PROC:SchedulerSetEnable "index=2" "enable=true"\n',
     device_reply='OK event PROC:SchedulerSetEnable "index=2" "enable=true"\n', expect_result=OK)
text(DME, "set_scheduler_event_time", {"index": 3, "time": "9:00"}, 'event PROC:SchedulerSetTime "index=3" "time=9:00"\n',
     device_reply='OKm event PROC:SchedulerSetTime "scheduler is not placed"\n', expect_result=ERR)
text(DME, "get_scheduler_event_count", {}, 'listitemnum PROC:Scheduler ""\n', device_reply="OK listitemnum PROC:Scheduler 15 1 15\n",
     expect_result=val("15"))
text(DME, "get_scheduler_event", {"index": 1}, "listitem PROC:Scheduler 1\n",
     device_reply='OK listitem PROC:Scheduler 1 "enable 9:00 MorningChime"\n', expect_result=val("enable 9:00 MorningChime"))

# Telemetry. Asked on connecting, then pushed (3.2, p.9-12) or replied.
telemetry(DME, "run-mode", expect_connect_wire=["devstatus runmode\n"],
    inbound='NOTIFY devstatus runmode "booting"\n', expect_state={"device": {"run_mode": "booting"}})
telemetry(DME, "devmode", inbound="OK devmode emergency\n", expect_state={"device": {"run_mode": "emergency"}})
telemetry(DME, "error-status", inbound='NOTIFY devstatus error "err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"\n',
          expect_state={"device": {"error": "err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"}})
telemetry(DME, "power-supply", inbound='NOTIFY devstatus power1 "fine"\n', expect_state={"power_supplies": {"1": {"ok": True}}})
telemetry(DME, "power-supply-reply", inbound='OK devstatus Power2 "down"\n', expect_state={"power_supplies": {"2": {"ok": False}}})
telemetry(DME, "protocol-version", inbound='OK devinfo protocolver "1.0.0"\n', expect_state={"device": {"protocol_version": "1.0.0"}})
telemetry(DME, "parameter-set-version", inbound='OK devinfo paramsetver "PROC:1.0.0"\n',
          expect_state={"device": {"parameter_set_version": "PROC:1.0.0"}})
telemetry(DME, "firmware", inbound='OK devinfo version "1.1.0"\n', expect_state={"device": {"firmware": "1.1.0"}})
telemetry(DME, "product-name", inbound='OK devinfo productname "DME7"\n', expect_state={"device": {"product_name": "DME7"}})
telemetry(DME, "manufacturer", inbound='OK devinfo manufacturer "Yamaha Corporation"\n',
          expect_state={"device": {"manufacturer": "Yamaha Corporation"}})
telemetry(DME, "serial", inbound='OK devinfo serialno "ZA37640CHNET101001"\n', expect_state={"device": {"serial": "ZA37640CHNET101001"}})
telemetry(DME, "category", inbound='OK devinfo category "processor"\n', expect_state={"device": {"category": "processor"}})
telemetry(DME, "device-id", inbound='OK devinfo deviceid "001"\n', expect_state={"device": {"device_id": "001"}})
telemetry(DME, "device-name", inbound='OK devinfo devicename "DME7 Hall"\n', expect_state={"device": {"name": "DME7 Hall"}})
telemetry(DME, "inputs", inbound='OK devinfo inputport "64"\n', expect_state={"device": {"inputs": 64}})
telemetry(DME, "outputs", inbound="OK devinfo outputport 64\n", expect_state={"device": {"outputs": 64}})
telemetry(DME, "gpi", inbound="OK devinfo gpi 16\n", expect_state={"device": {"gpi": 16}})
telemetry(DME, "gpo", inbound="OK devinfo gpo 8\n", expect_state={"device": {"gpo": 8}})
telemetry(DME, "parameter-count", inbound="OK prmnum 1000\n", expect_state={"device": {"parameter_count": 1000}})
telemetry(DME, "meter-count", inbound="OK mtrnum 1000\n", expect_state={"device": {"meter_count": 1000}})
telemetry(DME, "parameter-notify", inbound='NOTIFY set PROC:Remote/3 0 0 -7760 "-77.60"\n',
          expect_state={"parameters": {"3": {"0": {"0": {"raw": -7760, "text": "-77.60"}}}}})
telemetry(DME, "parameter-get", inbound="OK get PROC:Remote/1 2 0 -7760\n", expect_state={"parameters": {"1": {"2": {"0": {"raw": -7760}}}}})
telemetry(DME, "parameter-relative", inbound="OK setr PROC:Remote/1 0 0 -1900\n",
          expect_state={"parameters": {"1": {"0": {"0": {"raw": -1900}}}}})
telemetry(DME, "parameter-normalized", inbound='OKm setn PROC:Remote/1 0 0 408 "-21.50"\n',
          expect_state={"parameters": {"1": {"0": {"0": {"normalized": 408, "text": "-21.50"}}}}})
telemetry(DME, "parameter-getn", inbound="OK getn PROC:Remote/1 0 0 408\n", expect_state={"parameters": {"1": {"0": {"0": {"normalized": 408}}}}})
telemetry(DME, "parameter-text", inbound='OK gett PROC:Remote/1 0 0 "10.0"\n', expect_state={"parameters": {"1": {"0": {"0": {"text": "10.0"}}}}})
telemetry(DME, "parameter-get-all", inbound="OK get PROC:Remote/1 all 0 -5000 -6000\n", expect_state={})
telemetry(DME, "meter", inbound="NOTIFY mtr PROC:Remote/2 level 71 71 71 71 71 71 69 68\n",
          expect_state={"meters": {"2": {"type": "level", "values": "71 71 71 71 71 71 69 68"}}})
telemetry(DME, "meter-peak-hold", inbound="NOTIFY mtr PROC:Remote/2>PeakHold level 71 71 69 68\n",
          expect_state={"meters": {"2": {"peak_hold_values": "71 71 69 68"}}})
telemetry(DME, "current-parameter-set", inbound="OK sscurrent_ex 2 unmodified\n", expect_state={"snapshots": {"current_parameter_set": 2}})
telemetry(DME, "current-snapshot", inbound="OK sscurrent 2 10 modified\n",
          expect_state={"snapshots": {"current_parameter_set": 2}, "parameter_sets": {"2": {"current_snapshot": 10, "modified": True}}})
telemetry(DME, "snapshot-changed", inbound="NOTIFY sscurrent_ex 5000 10\n",
          expect_state={"snapshots": {"current_parameter_set": 5000}, "parameter_sets": {"5000": {"current_snapshot": 10}}})
telemetry(DME, "snapshot-recall", inbound="NOTIFY ssrecall_ex 5000 10\n",
          expect_state={"snapshots": {"last_recalled_parameter_set": 5000, "last_recalled": 10}})
telemetry(DME, "snapshot-count", inbound="OK ssnum_ex 50 100\n", expect_state={"parameter_sets": {"50": {"snapshot_count": 100}}})
telemetry(DME, "snapshot-info", inbound='OK ssinfo_ex 50 1 "001" "open time snapshot" "" user\n',
          expect_state={"parameter_sets": {"50": {"snapshots": {"1": {"number_text": "001", "title": "open time snapshot", "attribute": "user"}}}}})
telemetry(DME, "synchronization", inbound='NOTIFY event PROC:SynchronizationSetStatus "active"\n', expect_state={"device": {"synchronizing": True}})
telemetry(DME, "media", inbound='NOTIFY event PROC:Media "sdcard=extracted"\n', expect_state={"media": {"sd_card_inserted": False}})
telemetry(DME, "media-missing", inbound='OKm event PROC:AudioPlayerTransport "sdcard is not inserted"\n',
          expect_state={"media": {"sd_card_inserted": False}})
telemetry(DME, "player-type", inbound='NOTIFY event PROC:AudioPlayerSetType "index=1" "type=1song"\n',
          expect_state={"audio_player": {"items": {"1": {"type": "1song"}}}})
telemetry(DME, "player-path", inbound='NOTIFY event PROC:AudioPlayerSetPath "index=1" "path=song.wav"\n',
          expect_state={"audio_player": {"items": {"1": {"path": "song.wav"}}}})
telemetry(DME, "player-mode", inbound='NOTIFY event PROC:AudioPlayerSetPlayMode "index=1" "mode=normal"\n',
          expect_state={"audio_player": {"items": {"1": {"play_mode": "normal"}}}})
telemetry(DME, "player-go-to-top", inbound='NOTIFY event PROC:AudioPlayerSetGoToTheTop "index=1" "goToTheTop=off"\n',
          expect_state={"audio_player": {"items": {"1": {"go_to_top": False}}}})
telemetry(DME, "player-interval", inbound='NOTIFY event PROC:AudioPlayerSetInterval "index=1" "interval=3.0"\n',
          expect_state={"audio_player": {"items": {"1": {"interval": 3.0}}}})
telemetry(DME, "player-transport", inbound='NOTIFY event PROC:AudioPlayerTransport "operation=pause"\n',
          expect_state={"audio_player": {"status": "pause"}})
telemetry(DME, "player-status", inbound='OK event PROC:AudioPlayerGetStatus "status=stop"\n', expect_state={"audio_player": {"status": "stop"}})
telemetry(DME, "player-current-song", inbound='NOTIFY event PROC:AudioPlayerSetCurrentSong "index=1"\n',
          expect_state={"audio_player": {"current_song": 1}})
telemetry(DME, "player-item-count", inbound="OK listitemnum PROC:AudioPlayer 100 1 100\n",
          expect_state={"audio_player": {"item_count": 100, "first_index": 1, "last_index": 100}})
telemetry(DME, "player-item", inbound='OK listitem PROC:AudioPlayer 1 "1Song Normal On 1.0 xxx.wav"\n',
          expect_state={"audio_player": {"items": {"1": {"type": "1Song", "play_mode": "Normal", "go_to_top": True, "interval": 1.0, "name": "xxx.wav"}}}})
telemetry(DME, "display-font", inbound='OK event PROC:Language "type2"\n', expect_state={"device": {"display_font": "type2"}})
telemetry(DME, "scheduler-enable", inbound='OK event PROC:SchedulerSetEnable "index=2" "enable=true"\n',
          expect_state={"scheduler": {"events": {"2": {"enabled": True}}}})
telemetry(DME, "scheduler-time", inbound='OK event PROC:SchedulerSetTime "index=3" "time=9:00"\n',
          expect_state={"scheduler": {"events": {"3": {"time": "9:00"}}}})
telemetry(DME, "scheduler-count", inbound="OK listitemnum PROC:Scheduler 15 1 15\n",
          expect_state={"scheduler": {"event_count": 15, "first_index": 1, "last_index": 15}})
telemetry(DME, "scheduler-event", inbound='OK listitem PROC:Scheduler 1 "enable 9:00 MorningChime"\n',
          expect_state={"scheduler": {"events": {"1": {"enabled": True, "time": "9:00", "name": "MorningChime"}}}})
