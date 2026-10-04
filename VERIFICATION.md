# Hardware verification

Every integration here was written from the manufacturer's documentation and
tested against simulated devices. Documentation is often incomplete, sometimes
contradicts itself, and doesn't always match the firmware, so some behaviour
had to be inferred or chosen. This page lists what needs checking on a real
device.

We can't test every device ourselves, so results from anyone with access to
the hardware are very welcome, including a single item on a single model.

## How to help

1. Find your device below. Each item says what to check and links to the
   spec line that describes the current behaviour.
2. Try it on the device and note the model, firmware version and what you
   saw. The exact bytes sent and received are the most useful thing you can
   include, such as a terminal session or a Wireshark capture.
3. Report it in whichever way suits you:
   - **Open an issue** with the
     [hardware verification form](https://github.com/meros-co/integrations/issues/new?template=hardware-verification.yml).
     You don't need to change any code.
   - **Open a pull request** that fixes the spec if the device differs, adds
     a recorded conformance vector (see
     [CONTRIBUTING.md](CONTRIBUTING.md#verification)), and ticks the item
     here or removes it once it is fully answered.

A result that confirms the current behaviour is as useful as one that
corrects it. Things not listed here are welcome too: wrong ranges, missing
commands, odd replies.

Line links point to where each note was when this list was written, so they
can drift as specs change. The quirk text in the spec is the reference.

## aes70 — AES70 / OCA devices

- [ ] Check the device's OCP.1 TCP port (AES70 assigns none) and that it accepts OCP.1 on plain TCP, not only over TLS, UDP or WebSocket. ([specs/aes70.yaml:279](specs/aes70.yaml#L279))
- [ ] Check whether the device sends OcaDelay's delay time (OcaTimeInterval) as a 32-bit or a 64-bit float, and that set_delay written at that width is accepted. ([specs/aes70.yaml:291](specs/aes70.yaml#L291))
- [ ] Confirm the device answers the controller's keep-alive with keep-alives of its own at that interval, so a device opened for commands only is reported connected within one interval, and whether it accepts the millisecond form for an interval that is not whole seconds. ([specs/aes70.yaml:297](specs/aes70.yaml#L297))
- [ ] Confirm the device sends keep-alives while idle, so it is not dropped after three intervals of quiet, and that it closes the connection itself after three intervals without the controller's. ([specs/aes70.yaml:305](specs/aes70.yaml#L305))
- [ ] Confirm AddSubscription (3.1 on object 4) for an object's PropertyChanged event, with subscriber method 1.1 of object 4096, an empty context, delivery mode 1 and an empty destination, is accepted, and that changes then arrive as notifications carrying the property id, value and change type (record which PDU type, 2 or 5). ([specs/aes70.yaml:312](specs/aes70.yaml#L312))
- [ ] Check that GetMembers (3.5) answers on every block, how long a full walk takes on a large device (a DS100 or an amplifier with many channels), and whether 32 requests in flight at once is too many for it. ([specs/aes70.yaml:321](specs/aes70.yaml#L321))
- [ ] Check that roles are unique within each block and contain no '/', so role paths are unambiguous. ([specs/aes70.yaml:329](specs/aes70.yaml#L329))
- [ ] Confirm that a role path lookup when opened for commands only (GetMembers and GetRole level by level) works, and that GetClassIdentification (1.1) answers for an object addressed by number. ([specs/aes70.yaml:335](specs/aes70.yaml#L335))
- [ ] Check what a setter answers for a value out of range (ParameterOutOfRange, or a clamped value notified) and for a locked object, and record the status codes seen. ([specs/aes70.yaml:343](specs/aes70.yaml#L343))
- [ ] Confirm the bounded getters (OcaGain GetGain, OcaSwitch GetPosition, OcaDelay GetDelayTime, the numeric basic actuators, OcaLevelSensor GetReading) return the value, minimum and maximum in that order. ([specs/aes70.yaml:238](specs/aes70.yaml#L238))
- [ ] Check level sensor readings: the unit (dBFS or dBu) and range, and whether polling every sensor once a second is acceptable on a device with many meters. ([specs/aes70.yaml:352](specs/aes70.yaml#L352))
- [ ] Confirm the device manager getters: ModelGUID (3.2) as 8 bytes, ModelDescription (3.6) as three strings, State (3.13) as a 16-bit set, and which optional ones (DeviceRevisionID 3.20) the device implements. ([specs/aes70.yaml:262](specs/aes70.yaml#L262))
- [ ] Check a role or label with non-ASCII characters: OCP.1 string lengths are taken to count Unicode characters, not bytes. ([crates/core/src/modules/aes70_codec.rs:244](crates/core/src/modules/aes70_codec.rs#L244))
- [ ] Record the class ids of a vendor's proprietary objects (d&b, Powersoft, stagebox preamps) and whether they derive from the standard classes, so they are read and followed. ([specs/aes70.yaml:319](specs/aes70.yaml#L319))

## aja-kipro — AJA Ki Pro

- [ ] Confirm that an event connection id expires when no wait_for_config_events is made within a minute, and that the extension's back-to-back waits keep it alive over a long session. ([specs/aja-kipro.yaml:1326](specs/aja-kipro.yaml#L1326))
- [ ] Check how the unit answers wait_for_config_events with an expired connection id (status code and body), since AJA does not document it and the extension reconnects on anything that is not a 2xx JSON array. ([specs/aja-kipro.yaml:1341](specs/aja-kipro.yaml#L1341))
- [ ] Confirm which URL opens the event connection on each model and firmware, /json (AJA examples) or /config (2021 guide). ([specs/aja-kipro.yaml:1338](specs/aja-kipro.yaml#L1338))
- [ ] On a Ki Pro GO and Ultra, check desc.json for eParamID_TransportState, eParamID_CurrentClip, eParamID_DisplayTimecode and eParamID_MediaState, and whether the 30-second poll errors on them. ([specs/aja-kipro.yaml:1356](specs/aja-kipro.yaml#L1356))
- [ ] On Ultra models, confirm the accepted range and format of eParamID_TransportRequestedSpeed (three decimals such as -1.500) for variable speed play. ([specs/aja-kipro.yaml:1403](specs/aja-kipro.yaml#L1403))
- [ ] On Ultra Plus and 12G, confirm that set_sdi_monitor_channel works when sent with paramid= (AJA's chapter 6 URL omits it). ([specs/aja-kipro.yaml:1431](specs/aja-kipro.yaml#L1431))
- [ ] On Ki Pro, Mini and Rack, check whether the deprecated /clips listing still uses unquoted JSON keys. ([specs/aja-kipro.yaml:1393](specs/aja-kipro.yaml#L1393))

## aja-kumo — AJA KUMO

- [ ] Check what a salvo crosspoint of source 0 does: turns the output off, or leaves the destination unchanged. ([specs/aja-kumo.yaml:1461](specs/aja-kumo.yaml#L1461))
- [ ] Confirm that take_salvo sending the salvo number as the eParamID_TakeSalvo value takes salvos 2-8 (AJA's own TakeSalvo always sends 1). ([specs/aja-kumo.yaml:1468](specs/aja-kumo.yaml#L1468))
- [ ] Confirm the lock parameter eParamID_XPT_Destination{n}_Locked (taken from the Companion module) exists and works, and whether lock changes arrive as events. ([specs/aja-kumo.yaml:1392](specs/aja-kumo.yaml#L1392))
- [ ] Check that string and integer values in KUMO event elements are told apart correctly, since KUMO sends no param_type and a non-empty str_value with no or zero int_value is taken as a string. ([crates/core/src/modules/aja_config_events.rs:172](crates/core/src/modules/aja_config_events.rs#L172))
- [ ] Record what a salvo change looks like on the event connection, since AJA does not document its event form. ([specs/aja-kumo.yaml:1412](specs/aja-kumo.yaml#L1412))
- [ ] Check how the router answers wait_for_config_events with an expired connection id, and that /config (not /json) gives the connection id. ([specs/aja-kumo.yaml:1414](specs/aja-kumo.yaml#L1414))
- [ ] Check whether the firmware on hand answers eParamID_XPT_DestinationAll_Status through /options, or {} as older firmware does. ([specs/aja-kumo.yaml:1424](specs/aja-kumo.yaml#L1424))
- [ ] Confirm that a number above the router's count, and a switch to a locked destination, fail with HTTP 403 (or note the actual status). ([specs/aja-kumo.yaml:1384](specs/aja-kumo.yaml#L1384))
- [ ] Confirm the router handles back-to-back requests without the pauses AJA's examples add. ([specs/aja-kumo.yaml:1433](specs/aja-kumo.yaml#L1433))
- [ ] Confirm button colour writes in the {'classes': 'color_N'} form are accepted. ([specs/aja-kumo.yaml:1476](specs/aja-kumo.yaml#L1476))
- [ ] Confirm eParamID_LED_Identify turns Identify on and off (it comes from a generic AJA example, not the manual). ([specs/aja-kumo.yaml:1485](specs/aja-kumo.yaml#L1485))

## allenheath-ahm — Allen & Heath AHM

- [ ] Confirm the login encoding on the TLS port 51327: profile sent as one byte followed by the password's UTF-8 bytes, with no ", " separator or terminator, answered by AuthOK. ([specs/allenheath-ahm.yaml:227](specs/allenheath-ahm.yaml#L227))
- [ ] Check whether the processor transmits changes other than preset recall, source selection and room combination (mutes, levels) when they are made on it. ([specs/allenheath-ahm.yaml:240](specs/allenheath-ahm.yaml#L240))
- [ ] Confirm that -48 dB sent as 01 sets -48 dB, and that received levels read back as the expected dB. ([specs/allenheath-ahm.yaml:245](specs/allenheath-ahm.yaml#L245))
- [ ] Check trim (-24 to +24 dB) and preamp gain (+5 to +60 dB) at several points, since only their ends are documented and the spec spaces them evenly. ([specs/allenheath-ahm.yaml:248](specs/allenheath-ahm.yaml#L248))
- [ ] Confirm the source selector get is answered when sent with N = 1, and that its reply is told apart from the message sent on a selection. ([specs/allenheath-ahm.yaml:253](specs/allenheath-ahm.yaml#L253))
- [ ] Confirm room numbers in the room combiner message are 00-0F for rooms 1-16. ([specs/allenheath-ahm.yaml:257](specs/allenheath-ahm.yaml#L257))

## allenheath-cq — Allen & Heath CQ

- [ ] Confirm the pan centre value 40 00 and level values between the p.15 table points. ([specs/allenheath-cq.yaml:199](specs/allenheath-cq.yaml#L199))
- [ ] Check that a smaller CQ model ignores input and output numbers it lacks rather than erroring. ([specs/allenheath-cq.yaml:207](specs/allenheath-cq.yaml#L207))

## allenheath-dlive — Allen & Heath dLive / Avantis

- [ ] Confirm the login encoding on the TLS ports 51327 and 51329: profile sent as one byte followed by the password's UTF-8 bytes, with no separator or terminator, answered by AuthOK. ([specs/allenheath-dlive.yaml:375](specs/allenheath-dlive.yaml#L375))
- [ ] Check how the login profile number 00-1F maps to the console's user profiles. ([specs/allenheath-dlive.yaml:379](specs/allenheath-dlive.yaml#L379))
- [ ] On Avantis, find the TLS port (not documented) and confirm tls works with it set. ([specs/allenheath-dlive.yaml:366](specs/allenheath-dlive.yaml#L366))
- [ ] Check that a plain TCP connection to a console in Secure mode gets no answer (so the warning fires) rather than being dropped. ([specs/allenheath-dlive.yaml:368](specs/allenheath-dlive.yaml#L368))
- [ ] Check whether the console transmits changes other than scene recall (mutes, faders, names) when made on the surface. ([specs/allenheath-dlive.yaml:397](specs/allenheath-dlive.yaml#L397))
- [ ] Confirm fader and send level rounding: 6B reads 0 dB, +5 dB is 75, and on Avantis the hex column (not the decimal column) matches the console. ([specs/allenheath-dlive.yaml:404](specs/allenheath-dlive.yaml#L404))
- [ ] Confirm preamp gain follows the p.9 table (+30 dB is 3A) and that the preamp get works with the socket number MP in place of CH, including DX sockets. ([specs/allenheath-dlive.yaml:412](specs/allenheath-dlive.yaml#L412))
- [ ] Confirm that group and aux assignment off, sent as 3F, unassigns. ([specs/allenheath-dlive.yaml:419](specs/allenheath-dlive.yaml#L419))
- [ ] Check whether dLive accepts the UFX chromatic scale (02), which is offered on Avantis only. ([specs/allenheath-dlive.yaml:426](specs/allenheath-dlive.yaml#L426))
- [ ] Check how dLive handles a name longer than it allows (presumably truncated). ([specs/allenheath-dlive.yaml:431](specs/allenheath-dlive.yaml#L431))
- [ ] Opened for commands only, check that the console answers the liveness probe when nothing else has been sent, since that answer is what reports it connected. ([crates/core/src/modules/allenheath.rs:41](crates/core/src/modules/allenheath.rs#L41))

## allenheath-qu — Allen & Heath Qu

- [ ] Confirm the mixer sends Active Sensing (FE) about every 300 ms and keeps the connection with the core's FE every second. ([specs/allenheath-qu.yaml:274](specs/allenheath-qu.yaml#L274))
- [ ] Check pan and trim at several points, since only their ends are documented and the spec spaces them evenly. ([specs/allenheath-qu.yaml:288](specs/allenheath-qu.yaml#L288))
- [ ] Confirm the DCA assign values address DCAs 1-4 (the document repeats the mute group label). ([specs/allenheath-qu.yaml:295](specs/allenheath-qu.yaml#L295))
- [ ] Confirm stereo mixes, groups and matrices answer on their odd number and the LR destination is index 07. ([specs/allenheath-qu.yaml:294](specs/allenheath-qu.yaml#L294))
- [ ] Confirm remote shutdown works when sent on MIDI channel 1 (B0) whatever the mixer's channel. ([specs/allenheath-qu.yaml:301](specs/allenheath-qu.yaml#L301))

## allenheath-sq — Allen & Heath SQ

- [ ] Confirm the linear taper table values (not the disagreeing p.13 examples) set the expected dB, and check a few points of the "approximate" audio taper table. ([specs/allenheath-sq.yaml:325](specs/allenheath-sq.yaml#L325))
- [ ] Confirm LR mute is 00 44 (the p.18 example uses 00 00). ([specs/allenheath-sq.yaml:335](specs/allenheath-sq.yaml#L335))
- [ ] On SQ-5/6/7, check whether matrices 4-6 exist over MIDI (the firmware V1.6 article lists them). ([specs/allenheath-sq.yaml:340](specs/allenheath-sq.yaml#L340))
- [ ] On Qu-5/6/7 under this spec, confirm USB to LR uses 40 24 and the group-to-matrix assignment values. ([specs/allenheath-sq.yaml:353](specs/allenheath-sq.yaml#L353))

## analogway-alta4k — Analog Way Alta 4K

- [ ] Confirm the auxiliary screen T-bar read path DeviceObject/transition/$auxiliaryScreen/@items/<n>/status/@props/transition, which is inferred. ([specs/analogway-alta4k.yaml:657](specs/analogway-alta4k.yaml#L657))
- [ ] Confirm the resolved guide ambiguities: '$auxillaryScreen' as printed, audio modes with underscores, followScreen/@props/source for follow-screen writes, audio/$lineOut, and $preset in the background status read. ([specs/analogway-alta4k.yaml:663](specs/analogway-alta4k.yaml#L663))
- [ ] Confirm that errors arrive in message order, so matching each to the oldest unanswered message is right. ([specs/analogway-alta4k.yaml:651](specs/analogway-alta4k.yaml#L651))

## analogway-livecore — Analog Way LiveCore

- [ ] Confirm master preset screen enabling uses PSose (not PSsse), GPits takes a value, and layers 1-24 are accepted. ([specs/analogway-livecore.yaml:619](specs/analogway-livecore.yaml#L619))
- [ ] Check that an out-of-range value is answered with the previous value followed by an E10-E13 error. ([specs/analogway-livecore.yaml:613](specs/analogway-livecore.yaml#L613))
- [ ] Check how often unsolicited register pushes arrive while a command waits, which makes that command fail. ([specs/analogway-livecore.yaml:606](specs/analogway-livecore.yaml#L606))

## analogway-livepremier — Analog Way LivePremier / Aquilon

- [ ] Confirm the auxiliary screen T-bar read path DeviceObject/$screenAuxGroup/@items/A<n>/status/@props/transition, which is inferred. ([specs/analogway-livepremier.yaml:904](specs/analogway-livepremier.yaml#L904))
- [ ] Confirm freeze_input can write the input's freeze property (the guide documents only reading it). ([specs/analogway-livepremier.yaml:910](specs/analogway-livepremier.yaml#L910))
- [ ] Check whether reboot and shutdown work on DeviceObject/system/shutdown/cmd or need the long v4.0 path. ([specs/analogway-livepremier.yaml:916](specs/analogway-livepremier.yaml#L916))
- [ ] Confirm that reading an object returns the paths beneath it, as read_on_connect assumes. ([specs/analogway-livepremier.yaml:942](specs/analogway-livepremier.yaml#L942))
- [ ] Confirm errors arrive in message order, so matching each to the oldest unanswered message is right. ([specs/analogway-livepremier.yaml:896](specs/analogway-livepremier.yaml#L896))

## analogway-midra — Analog Way Midra

- [ ] Confirm TAKE ALL is accepted as "1GCtal" (the guide also shows "1,GCtal"). ([specs/analogway-midra.yaml:495](specs/analogway-midra.yaml#L495))
- [ ] Confirm LF alone is accepted as the command terminator. ([specs/analogway-midra.yaml:497](specs/analogway-midra.yaml#L497))
- [ ] Check what the device answers to a TAKE sent while TAKEAVA is 0. ([specs/analogway-midra.yaml:483](specs/analogway-midra.yaml#L483))

## analogway-midra4k — Analog Way Midra 4K

- [ ] Confirm the auxiliary screen T-bar read path, which is inferred from the auxiliary xTake. ([specs/analogway-midra4k.yaml:772](specs/analogway-midra4k.yaml#L772))
- [ ] Confirm the resolved guide ambiguities: '$auxillaryScreen', underscored audio modes, followScreen/@props/source, audio/$lineOut, and $preset in the background status read. ([specs/analogway-midra4k.yaml:778](specs/analogway-midra4k.yaml#L778))
- [ ] Check whether screen 3 exists (the REST guide gives 1-2, the multiviewer source list shows SCREEN_PRGM_3). ([specs/analogway-midra4k.yaml:806](specs/analogway-midra4k.yaml#L806))

## analogway-picturall — Analog Way Picturall

- [ ] Check whether control status messages separate controls with the literal string "\n" or with real line breaks. ([specs/analogway-picturall.yaml:240](specs/analogway-picturall.yaml#L240))
- [ ] Confirm unquoted set values are accepted. ([specs/analogway-picturall.yaml:246](specs/analogway-picturall.yaml#L246))
- [ ] Check what media_end_action values mean and which playback commands besides go exist. ([specs/analogway-picturall.yaml:251](specs/analogway-picturall.yaml#L251))

## avolites-titan — Avolites Titan

- [ ] Check the status code and body Titan returns for an invalid request (unknown handle, bad parameter), since any 200 counts as success here. ([specs/avolites-titan.yaml:1353](specs/avolites-titan.yaml#L1353))
- [ ] Confirm the handle record field names on Titan 19 (Active or active, Legend or legend, userNumber.hashCode) and that `active` reflects a running playback. ([specs/avolites-titan.yaml:1377](specs/avolites-titan.yaml#L1377))
- [ ] Confirm that times sent with two decimals (fadeTime=5.00) and levels with three (level_level=1.000) are accepted like the documentation's whole numbers. ([specs/avolites-titan.yaml:1385](specs/avolites-titan.yaml#L1385))
- [ ] Check the grand master value range (0-100 as sent here, or 0-1). ([specs/avolites-titan.yaml:1387](specs/avolites-titan.yaml#L1387))
- [ ] Check that an empty panelTimeStamp works for tap tempo and an empty oldValue for the grand master. ([specs/avolites-titan.yaml:1395](specs/avolites-titan.yaml#L1395))
- [ ] Check which licences answer the WebAPI (consoles, Titan PC with T2/T3/TNP, Titan Go) and that T1 and Titan One do not. ([specs/avolites-titan.yaml:1345](specs/avolites-titan.yaml#L1345))
- [ ] Measure the /titan/handles reply size and time on a large show, to judge the 5 s poll. ([specs/avolites-titan.yaml:1369](specs/avolites-titan.yaml#L1369))
- [ ] Find the handle group name of the playback faders for set_group_page (Playbacks, PlaybackWindow or other). ([specs/avolites-titan.yaml:1210](specs/avolites-titan.yaml#L1210))

## avstumpfl-pixera — AV Stumpfl PIXERA

- [ ] Confirm that replies and pushed monEvent messages on a JSON/TCP (dl) access point end with 0xPX, and what setMonitoringHasDelimiter changes (26.1 lists it without a description). ([specs/avstumpfl-pixera.yaml:690](specs/avstumpfl-pixera.yaml#L690))
- [ ] Check how a PIXERA before 26.1 answers setMonitoringHasDelimiter (an error reply is assumed) and that monitoring still works there. ([specs/avstumpfl-pixera.yaml:691](specs/avstumpfl-pixera.yaml#L691))
- [ ] Confirm that setMonitoringEventMode all pushes timelineTransport, timelinePositions and timelineCountdowns with the same entry shapes as pollMonitoring, with id -1, and how often positions arrive. ([specs/avstumpfl-pixera.yaml:682](specs/avstumpfl-pixera.yaml#L682))
- [ ] Check that a reply carries the request's id (1) as "id":1, so that pushed messages (id -1) are not taken as replies. ([specs/avstumpfl-pixera.yaml:94](specs/avstumpfl-pixera.yaml#L94))
- [ ] Check the error object PIXERA sends for an unknown method, wrong parameter names, an unknown timeline or cue name, and an invalid handle. ([specs/avstumpfl-pixera.yaml:139](specs/avstumpfl-pixera.yaml#L139))
- [ ] Check what a name or path containing the text 0xPX does to framing. ([specs/avstumpfl-pixera.yaml:668](specs/avstumpfl-pixera.yaml#L668))
- [ ] Confirm the default API port (1400 assumed from AV Stumpfl's examples; none is documented) and whether a disabled API (25.2 default) refuses the connection or answers -32602. ([specs/avstumpfl-pixera.yaml:660](specs/avstumpfl-pixera.yaml#L660))
- [ ] Check whether handles stay valid after reloading the project or restarting PIXERA. ([specs/avstumpfl-pixera.yaml:685](specs/avstumpfl-pixera.yaml#L685))
- [ ] Confirm the Compound time methods take "name" in 26.1 and "timelineName" before it, and which revision changed it. ([specs/avstumpfl-pixera.yaml:69](specs/avstumpfl-pixera.yaml#L69))
- [ ] Check whether load_project discards unsaved changes silently. ([specs/avstumpfl-pixera.yaml:696](specs/avstumpfl-pixera.yaml#L696))
- [ ] Check the format of getCurrentHMSFOfTimeline (hh:mm:ss:ff assumed) and what getCurrentCountdownOfTimeline returns when no cue is ahead. ([specs/avstumpfl-pixera.yaml:305](specs/avstumpfl-pixera.yaml#L305))

## barco-eventmaster — Barco Event Master

- [ ] Confirm JSON-RPC requests are accepted at the root path / on port 9999, since the documents give no request path. ([specs/barco-eventmaster.yaml:1257](specs/barco-eventmaster.yaml#L1257))
- [ ] Check which key activateDestGroup by name accepts, destGrpName (sent) or destGrName. ([specs/barco-eventmaster.yaml:1307](specs/barco-eventmaster.yaml#L1307))
- [ ] Confirm the user key list method is listUserKeys (not listUserKey), what it returns, and that recallUserKey accepts the key userkeyName. ([specs/barco-eventmaster.yaml:1314](specs/barco-eventmaster.yaml#L1314))
- [ ] Record the actual reply shape of powerStatus and of listCues, which the documents leave open. ([specs/barco-eventmaster.yaml:1323](specs/barco-eventmaster.yaml#L1323))
- [ ] Check which test pattern numbers exist beyond 0 (off), 3 (colour bars) and 5 (grid). ([specs/barco-eventmaster.yaml:1326](specs/barco-eventmaster.yaml#L1326))
- [ ] Check what cue_transport (pause or stop with type alone and no cue id) applies to. ([specs/barco-eventmaster.yaml:1409](specs/barco-eventmaster.yaml#L1409))
- [ ] On Encore3, check what the ApiExtControl flag in getFrameSettings means and whether it blocks API calls. ([specs/barco-eventmaster.yaml:1380](specs/barco-eventmaster.yaml#L1380))
- [ ] Check what error codes and messages come back in result.success and the JSON-RPC error member, since none are documented. ([specs/barco-eventmaster.yaml:1273](specs/barco-eventmaster.yaml#L1273))
- [ ] Confirm the subscribe command is accepted with the port sent as a string. ([specs/barco-eventmaster.yaml:1359](specs/barco-eventmaster.yaml#L1359))

## behringer-wing — Behringer WING

- [ ] Confirm that solo, talkback on, monitor level, solo dim and mono, input gain and phantom ($-prefixed parameters) can be set over OSC. ([specs/behringer-wing.yaml:2467](specs/behringer-wing.yaml#L2467))
- [ ] Check what gain and phantom sets do on a strip whose input source has no preamp. ([specs/behringer-wing.yaml:2478](specs/behringer-wing.yaml#L2478))
- [ ] Find the separator between tags in a strip's tag string, so DCA assignment does not damage other tags. ([specs/behringer-wing.yaml:2494](specs/behringer-wing.yaml#L2494))
- [ ] Find the tag names used for mute group assignment, which are not documented. ([specs/behringer-wing.yaml:2496](specs/behringer-wing.yaml#L2496))
- [ ] Check what GONEXT and GOPREV do, since they are listed without description. ([specs/behringer-wing.yaml:2528](specs/behringer-wing.yaml#L2528))
- [ ] Check whether a set sent while /$ctl/OSC/ronly is on returns an error. ([specs/behringer-wing.yaml:2537](specs/behringer-wing.yaml#L2537))
- [ ] Check which models allow set_monitor_level (settable on Compact and Rack, read-only on the full-size WING). ([specs/behringer-wing.yaml:2517](specs/behringer-wing.yaml#L2517))
- [ ] On WING Rack, check whether main sends 5-8 (the headphone outputs) are reachable. ([specs/behringer-wing.yaml:2508](specs/behringer-wing.yaml#L2508))
- [ ] Confirm the 9-second /*s renewal keeps the subscription alive and the connection open, and that another subscribing client takes changes away as described. ([specs/behringer-wing.yaml:2418](specs/behringer-wing.yaml#L2418))

## behringer-x32 — Behringer X32 / Midas M32

- [ ] Check dB rounding on get: set a range of fader and send levels in dB, read them back, and record how far the console's rounding to its nearest step moves them. ([specs/behringer-x32.yaml:3284](specs/behringer-x32.yaml#L3284))
- [ ] Check whether recall_scene, recall_snippet and recall_cue produce any reply. ([specs/behringer-x32.yaml:3239](specs/behringer-x32.yaml#L3239))
- [ ] Check how long after the /load reply the load actually completes. ([specs/behringer-x32.yaml:3240](specs/behringer-x32.yaml#L3240))
- [ ] Check what the console does when an empty scene, snippet or cue slot is recalled over OSC. ([specs/behringer-x32.yaml:3245](specs/behringer-x32.yaml#L3245))
- [ ] Confirm a recall produces /xremote updates for every changed parameter. ([specs/behringer-x32.yaml:3244](specs/behringer-x32.yaml#L3244))
- [ ] Check how many local headamps each model has within the 1-32 range. ([specs/behringer-x32.yaml:3303](specs/behringer-x32.yaml#L3303))
- [ ] Check what a type above 33 sent to effect slots 5-8 does (passes validation, presumably ignored). ([specs/behringer-x32.yaml:3320](specs/behringer-x32.yaml#L3320))
- [ ] Find which bit of the 18-bit talkback destination bitmap is which destination. ([specs/behringer-x32.yaml:3350](specs/behringer-x32.yaml#L3350))
- [ ] Check the user-control button numbering 5-12 on Compact and Producer. ([specs/behringer-x32.yaml:3334](specs/behringer-x32.yaml#L3334))

## behringer-xair — Behringer X AIR

- [ ] Confirm the mute sense of mix/on (0 muted, 1 passing audio), which is taken from the X32. ([specs/behringer-xair.yaml:3832](specs/behringer-xair.yaml#L3832))
- [ ] Check whether the mixer echoes a set back to the client that sent it. ([specs/behringer-xair.yaml:3859](specs/behringer-xair.yaml#L3859))
- [ ] Confirm the snapshot node is /-snap/ (not /snap/ as the manufacturer document writes it), and whether /-snap/load replies. ([specs/behringer-xair.yaml:3849](specs/behringer-xair.yaml#L3849))
- [ ] Confirm the fader taper is the X32's four segments (0.75 = 0 dB), which the X AIR document does not state. ([specs/behringer-xair.yaml:3901](specs/behringer-xair.yaml#L3901))
- [ ] Confirm headamp gain range is -12 to +60 dB (the community list says -12 to +20 dB). ([specs/behringer-xair.yaml:3914](specs/behringer-xair.yaml#L3914))
- [ ] Find how line inputs and the stereo aux input are addressed for gain, which is not documented. ([specs/behringer-xair.yaml:3911](specs/behringer-xair.yaml#L3911))
- [ ] Find which /ch/NN/config/insrc value is OFF, if any, within 0-15. ([specs/behringer-xair.yaml:3941](specs/behringer-xair.yaml#L3941))
- [ ] Find the order of gate modes 0-4 (GATE, EXP2, EXP3, EXP4, DUCK or the X32's EXP2, EXP3, EXP4, GATE, DUCK). ([specs/behringer-xair.yaml:3949](specs/behringer-xair.yaml#L3949))
- [ ] Confirm the copy-error resolutions: bus pan and insert ranges, bus compressor filter type 0-8, /bus/N/grp, USB routing 1-18. ([specs/behringer-xair.yaml:3955](specs/behringer-xair.yaml#L3955))
- [ ] Check the main LR insert address, FX return EQ on/off, the GEQ indexing and whether FX parameters are int or float, which are not covered because sources disagree. ([specs/behringer-xair.yaml:3967](specs/behringer-xair.yaml#L3967))
- [ ] Check the order of /xinfo's reply strings. ([specs/behringer-xair.yaml:3991](specs/behringer-xair.yaml#L3991))
- [ ] Check whether /-action/setclock is ignored on XR18, X18 and MR18. ([specs/behringer-xair.yaml:3933](specs/behringer-xair.yaml#L3933))
- [ ] Check the low-cut frequency mapping of 0-1 (the community list gives 20 to 200 Hz). ([specs/behringer-xair.yaml:2255](specs/behringer-xair.yaml#L2255))
- [ ] Check the automix weight range (-24 to +24 or -12 to +12). ([specs/behringer-xair.yaml:2295](specs/behringer-xair.yaml#L2295))

## biamp-tesira — Biamp Tesira

- [ ] Capture the login prompts of a protected system over Telnet (the module looks for a waiting "login:", "username:", "user name:", "user:" or "password:") and what follows a wrong password. ([specs/biamp-tesira.yaml:443](specs/biamp-tesira.yaml#L443))
- [ ] Check whether an unprotected system asks for a login over Telnet at all (the manual says a login prompt appears; Biamp's negotiation example goes straight to the banner). ([specs/biamp-tesira.yaml:443](specs/biamp-tesira.yaml#L443))
- [ ] Check that lines sent with LF alone (no CR) are accepted, including the user name and password at the login prompts. ([specs/biamp-tesira.yaml:443](specs/biamp-tesira.yaml#L443))
- [ ] Check whether a subscription's first publication and its +OK arrive on separate lines (the manual) or on one line (the knowledge-base examples); the module accepts both. ([specs/biamp-tesira.yaml:480](specs/biamp-tesira.yaml#L480))
- [ ] Check that subscription labels of the form meros1, meros2 are accepted unquoted, and that subscribing again with the same label replaces the subscription rather than adding one. ([specs/biamp-tesira.yaml:480](specs/biamp-tesira.yaml#L480))
- [ ] Check that DEVICE and SESSION work with the module's tag handling, and that quoted tags with spaces work for every command. ([specs/biamp-tesira.yaml:457](specs/biamp-tesira.yaml#L457))
- [ ] Check what recallPresetByName and savePresetByName answer for an unknown name, and the value recallPresetShowFailures returns. ([specs/biamp-tesira.yaml:318](specs/biamp-tesira.yaml#L318))
- [ ] Check which Tesira amplifier models host a TTP server. ([specs/biamp-tesira.yaml:132](specs/biamp-tesira.yaml#L132))
- [ ] Check that SESSION set verbose true is answered +OK on every firmware the module may meet. ([specs/biamp-tesira.yaml:496](specs/biamp-tesira.yaml#L496))

## birddog — BirdDog

- [ ] On 2.0 cameras, check whether a single-key POST is accepted or the whole object must be sent. ([specs/birddog.yaml:3178](specs/birddog.yaml#L3178))
- [ ] Confirm a POST reply does or does not show the new value, so it is clear whether a following poll is needed. ([specs/birddog.yaml:3163](specs/birddog.yaml#L3163))
- [ ] Check whether presets above 9 are accepted over REST on 2.1 cameras (2.0 gives 1-9, 2.1 gives no range). ([specs/birddog.yaml:3154](specs/birddog.yaml#L3154))
- [ ] Confirm the document inconsistency resolutions: TWODNR key on X1, the Sil2 ModeSel values, /sil2codec and /sil2enc paths, /RestImage, NDIGrpName and NDIOffSnSrc spelling, Wi-Fi and DHCP values On/Off and dhcp/static. ([specs/birddog.yaml:3229](specs/birddog.yaml#L3229))
- [ ] Check the ExpCompLvl range on P200 A2/A3 (0-14 or -128 to 127). ([specs/birddog.yaml:3245](specs/birddog.yaml#L3245))
- [ ] Check whether AnalogAudiooutputselect exists on P200/A200/A300, P100/PF120 and P4K/P400, and whether P4K/P400 accept the videooutputinterface modes. ([specs/birddog.yaml:3247](specs/birddog.yaml#L3247))
- [ ] Check MAKI Ultra tuning and WBSensitivity value forms (numbers or names such as "Middle"). ([specs/birddog.yaml:3242](specs/birddog.yaml#L3242))
- [ ] Confirm the X1 pan POST range -12996 to 12208. ([specs/birddog.yaml:3212](specs/birddog.yaml#L3212))
- [ ] Confirm the API needs no authentication on the firmware in use. ([specs/birddog.yaml:3143](specs/birddog.yaml#L3143))
- [ ] On X5 Ultra and X4 Ultra, check the firmware version and which 2.1 endpoints it lacks. ([specs/birddog.yaml:3271](specs/birddog.yaml#L3271))

## blackmagic-atem — Blackmagic ATEM

- [ ] Check how many simultaneous connections the switcher accepts and how long a dropped connection holds its place. ([specs/blackmagic-atem.yaml:1150](specs/blackmagic-atem.yaml#L1150))
- [ ] Confirm camera commands are acknowledged with no camera connected and that cameras.* reflects what a connected camera applied. ([specs/blackmagic-atem.yaml:1204](specs/blackmagic-atem.yaml#L1204))

## blackmagic-camera — Blackmagic cameras

- [ ] Find the event websocket URL, which the document does not give, so telemetry can use pushes instead of polling. ([specs/blackmagic-camera.yaml:2814](specs/blackmagic-camera.yaml#L2814))
- [ ] Confirm the login scheme with "Enabled with security" (Basic sent, Digest answered if challenged). ([specs/blackmagic-camera.yaml:2780](specs/blackmagic-camera.yaml#L2780))
- [ ] Check which API groups each camera model implements and whether missing ones answer 501 or 404. ([specs/blackmagic-camera.yaml:2786](specs/blackmagic-camera.yaml#L2786))
- [ ] Check that 0.0-1.0 is the range for normalised values beyond audio level. ([specs/blackmagic-camera.yaml:2804](specs/blackmagic-camera.yaml#L2804))
- [ ] Check the ranges the camera accepts for colour correction, playback speed, focus distance and aperture number, which are not limited here. ([specs/blackmagic-camera.yaml:2801](specs/blackmagic-camera.yaml#L2801))
- [ ] Confirm the format filesystem path spelling (doformatSupportedFilesystems or doFormatSupportedFilesystems). ([specs/blackmagic-camera.yaml:2839](specs/blackmagic-camera.yaml#L2839))
- [ ] Check what body PUT /monitoring/{displayName}/focusAssist accepts ({enabled} or mode, color and intensity). ([specs/blackmagic-camera.yaml:2848](specs/blackmagic-camera.yaml#L2848))
- [ ] Check whether a clip path in a folder must be sent with "/" encoded as %2F. ([specs/blackmagic-camera.yaml:2861](specs/blackmagic-camera.yaml#L2861))
- [ ] Check whether preset names for save_preset and delete_preset include the .cset extension. ([specs/blackmagic-camera.yaml:2862](specs/blackmagic-camera.yaml#L2862))
- [ ] Confirm set_custom_platform is accepted with application/xml and the XML on one line. ([specs/blackmagic-camera.yaml:2871](specs/blackmagic-camera.yaml#L2871))

## blackmagic-hyperdeck — Blackmagic HyperDeck

- [ ] Record the reply shape that carries the format token after format_prepare. ([specs/blackmagic-hyperdeck.yaml:2206](specs/blackmagic-hyperdeck.yaml#L2206))
- [ ] Record the fields of the asynchronous notifications documented only by their notify switch (dropped frames, display timecode, timeline position, playrange, cache, dynamic range, slate, device info, nas). ([specs/blackmagic-hyperdeck.yaml:2145](specs/blackmagic-hyperdeck.yaml#L2145))
- [ ] Check how protocol secure mode is reached, so credentials need not travel in plain text on 9993. ([specs/blackmagic-hyperdeck.yaml:2231](specs/blackmagic-hyperdeck.yaml#L2231))
- [ ] Check which dynamic range spelling the deck accepts and reports (ST2084 or ST2048). ([specs/blackmagic-hyperdeck.yaml:2254](specs/blackmagic-hyperdeck.yaml#L2254))
- [ ] Check whether "slate clips:" needs the colon (the table prints it without). ([specs/blackmagic-hyperdeck.yaml:2220](specs/blackmagic-hyperdeck.yaml#L2220))
- [ ] Check what the combined goto forms with relative offsets are relative to. ([specs/blackmagic-hyperdeck.yaml:2169](specs/blackmagic-hyperdeck.yaml#L2169))
- [ ] On protocol 1.8 and 1.11 decks, check which other documented commands work. ([specs/blackmagic-hyperdeck.yaml:2261](specs/blackmagic-hyperdeck.yaml#L2261))
- [ ] Check how many clients may connect at once. ([specs/blackmagic-hyperdeck.yaml:2190](specs/blackmagic-hyperdeck.yaml#L2190))

## blackmagic-multiview — Blackmagic MultiView 16 / MultiView 4

- [ ] On MultiView 4, check which routing output is the solo source (wire 4 assumed) and whether its solo source can be set over Ethernet at all. ([specs/blackmagic-multiview.yaml:420](specs/blackmagic-multiview.yaml#L420))
- [ ] Check that every CONFIGURATION line can be set by sending the block (the manual shows only Solo enabled being sent), and the boolean case the unit reports. ([specs/blackmagic-multiview.yaml:439](specs/blackmagic-multiview.yaml#L439))
- [ ] Check how Output format values map on MultiView 4. ([specs/blackmagic-multiview.yaml:451](specs/blackmagic-multiview.yaml#L451))
- [ ] Check whether the MultiView accepts F (force unlock) on VIDEO OUTPUT LOCKS. ([specs/blackmagic-multiview.yaml:460](specs/blackmagic-multiview.yaml#L460))
- [ ] Confirm the MULTIVIEW DEVICE output count (16 in the manual's example, with 18 routing outputs). ([specs/blackmagic-multiview.yaml:393](specs/blackmagic-multiview.yaml#L393))

## blackmagic-smartview — Blackmagic SmartView / SmartScope

- [ ] Check that the monitor answers commands with ACK/NAK, answers header-only status requests and PING, as the Videohub protocol does. ([specs/blackmagic-smartview.yaml:389](specs/blackmagic-smartview.yaml#L389))
- [ ] Check how the name is set (Name in the SMARTVIEW DEVICE block assumed). ([specs/blackmagic-smartview.yaml:389](specs/blackmagic-smartview.yaml#L389))
- [ ] Record how WidescreenSD and Border are reported (case, ON/OFF or true/false). ([specs/blackmagic-smartview.yaml:418](specs/blackmagic-smartview.yaml#L418))
- [ ] Check how a single-monitor model answers a MONITOR B block. ([specs/blackmagic-smartview.yaml:426](specs/blackmagic-smartview.yaml#L426))
- [ ] On SmartView 4K, check MonitorInput (community-observed), and whether SmartView 4K G3 has it. ([specs/blackmagic-smartview.yaml:439](specs/blackmagic-smartview.yaml#L439))
- [ ] Check whether SmartView 4K accepts contrast and saturation. ([specs/blackmagic-smartview.yaml:447](specs/blackmagic-smartview.yaml#L447))

## blackmagic-streaming — Blackmagic Web Presenter / Streaming

- [ ] Check whether the device accepts a Streaming XML file sent on a single line. ([specs/blackmagic-streaming.yaml:681](specs/blackmagic-streaming.yaml#L681))
- [ ] Confirm the character encoding is UTF-8 (the document names none). ([specs/blackmagic-streaming.yaml:695](specs/blackmagic-streaming.yaml#L695))
- [ ] Check whether a unit can report more than two network interfaces. ([specs/blackmagic-streaming.yaml:657](specs/blackmagic-streaming.yaml#L657))
- [ ] Check the length limits for label, stream key, password and URL. ([specs/blackmagic-streaming.yaml:715](specs/blackmagic-streaming.yaml#L715))
- [ ] Check which firmware release introduced protocol 1.2 (and what earlier units report). ([specs/blackmagic-streaming.yaml:706](specs/blackmagic-streaming.yaml#L706))

## blackmagic-videohub — Blackmagic Videohub

- [ ] Check how the router answers a port number beyond its count. ([specs/blackmagic-videohub.yaml:1053](specs/blackmagic-videohub.yaml#L1053))
- [ ] Record the exact spelling of the device block's needs-update value. ([specs/blackmagic-videohub.yaml:1063](specs/blackmagic-videohub.yaml#L1063))
- [ ] Check whether SERIAL PORT DIRECTIONS can be set by sending the block. ([specs/blackmagic-videohub.yaml:1095](specs/blackmagic-videohub.yaml#L1095))
- [ ] On Workgroup Videohub, confirm what the two numbers in PROCESSING UNIT ROUTING and FRAME BUFFER ROUTING index. ([specs/blackmagic-videohub.yaml:1104](specs/blackmagic-videohub.yaml#L1104))
- [ ] On 12G routers, check whether CONFIGURATION (Take Mode) and TAKE MODE can be set by a client and how the two relate. ([specs/blackmagic-videohub.yaml:1113](specs/blackmagic-videohub.yaml#L1113))
- [ ] Check whether a 12G router reports more than one network interface. ([specs/blackmagic-videohub.yaml:1124](specs/blackmagic-videohub.yaml#L1124))
- [ ] On Universal Videohubs, check whether ALARM STATUS is sent and what names it uses. ([specs/blackmagic-videohub.yaml:1131](specs/blackmagic-videohub.yaml#L1131))
- [ ] Check that routers documented only for v2.3 do or do not send the v2.8 blocks. ([specs/blackmagic-videohub.yaml:1074](specs/blackmagic-videohub.yaml#L1074))

## boinx-mimolive — Boinx mimoLive

- [ ] Confirm that writes with Content-Type application/vnd.api+json are accepted (Boinx's reference requires it; the manual's examples send application/json). ([specs/boinx-mimolive.yaml:1240](specs/boinx-mimolive.yaml#L1240))
- [ ] Check the body shape PUT /documents/(DocumentID) takes for programOutputMasterVolume (a plain object assumed, as for layers). ([specs/boinx-mimolive.yaml:168](specs/boinx-mimolive.yaml#L168))
- [ ] Check which body POST /documents/(DocumentID)/layers takes: the manual's layer-identifier, name and index, or the reference's data.attributes.composition-id. ([specs/boinx-mimolive.yaml:1251](specs/boinx-mimolive.yaml#L1251))
- [ ] Confirm that GET /documents?include=layers (and sources, output-destinations, layer-sets) sideloads every document's objects in included, as the poll reads them. ([specs/boinx-mimolive.yaml:1052](specs/boinx-mimolive.yaml#L1052))
- [ ] Record the websocket's added, changed and removed messages: that data is the whole object as the HTTP API returns it, and that a removed message carries type and id at the top level. ([specs/boinx-mimolive.yaml:1187](specs/boinx-mimolive.yaml#L1187))
- [ ] Confirm that the websocket closes a client that sends nothing for 15 seconds and that {"event":"ping"} every 5 seconds keeps it open. ([specs/boinx-mimolive.yaml:1044](specs/boinx-mimolive.yaml#L1044))
- [ ] With a remote control password set, confirm that the X-MimoLive-Password-SHA256 header is accepted on the websocket's opening request, and that a wrong key answers 401 on HTTP. ([specs/boinx-mimolive.yaml:88](specs/boinx-mimolive.yaml#L88))
- [ ] Check that output destination PATCH bodies are accepted with type and id, and layer set PATCH bodies without them. ([specs/boinx-mimolive.yaml:680](specs/boinx-mimolive.yaml#L680))
- [ ] Check that layer set PATCH and POST bodies take the JSON:API data.attributes form shown in the manual. ([specs/boinx-mimolive.yaml:777](specs/boinx-mimolive.yaml#L777))
- [ ] Check that a number input accepts four decimals and that set_layer_flag's JSON true and false are taken for a bool input. ([specs/boinx-mimolive.yaml:307](specs/boinx-mimolive.yaml#L307))
- [ ] Confirm the built-in outputs record, stream, playout and fullscreen in the document's outputs attribute, and their live-state values. ([specs/boinx-mimolive.yaml:177](specs/boinx-mimolive.yaml#L177))
- [ ] Check that comments/new takes its parameters in the query of a POST. ([specs/boinx-mimolive.yaml:883](specs/boinx-mimolive.yaml#L883))
- [ ] Check what a layer's live-variant relationship holds while the layer is off (null assumed, which leaves live_variant as it was). ([specs/boinx-mimolive.yaml:1009](specs/boinx-mimolive.yaml#L1009))

## brompton-tessera — Brompton Tessera

- [ ] Confirm that GET /api/ answers the whole tree under an "api" key, with groups, input ports, cable loops and frame remapping frames keyed "1", "2", ... (ports and frames numbered from 1), as the poll reads it. ([specs/brompton-tessera.yaml:3315](specs/brompton-tessera.yaml#L3315))
- [ ] Confirm that a section read (GET /api/override) answers {"override": {...}}, and that every single-endpoint read and write answers {"<last path segment>": value}. ([specs/brompton-tessera.yaml:4707](specs/brompton-tessera.yaml#L4707))
- [ ] Record the HTTP status of a failed request ({"response-code": "..."}), such as a write out of range, a path that does not exist and a read with no project loaded. ([specs/brompton-tessera.yaml:5465](specs/brompton-tessera.yaml#L5465))
- [ ] Measure how long GET /api/ takes on a large wall (thousands of panels) and whether reading it every 5 seconds affects the processor. ([specs/brompton-tessera.yaml:3327](specs/brompton-tessera.yaml#L3327))
- [ ] On each model, list which genlock, ShutterSync, frame remapping, hidden marker, failover and cable redundancy endpoints answer "Not supported". ([specs/brompton-tessera.yaml:5484](specs/brompton-tessera.yaml#L5484))
- [ ] Confirm each model's input ports (SX40 and S8: HDMI and SDI; S4 and T1: DVI; M2: DVI and two SDI) and that the port number starts at 1. ([specs/brompton-tessera.yaml:85](specs/brompton-tessera.yaml#L85))
- [ ] Check what a brightness above an active brightness limit does: refused, or held at the limit. ([specs/brompton-tessera.yaml:5551](specs/brompton-tessera.yaml#L5551))
- [ ] Check reboot and shutdown with and without a processor password, and what a wrong password answers. ([specs/brompton-tessera.yaml:5529](specs/brompton-tessera.yaml#L5529))
- [ ] Confirm that set_input_source (GET ?set=1&port-type=...&port-number=...) switches the input in one request on 3.5. ([specs/brompton-tessera.yaml:3306](specs/brompton-tessera.yaml#L3306))
- [ ] Check how the tree reports the test pattern type after a frame store user number was set (a number or a string). ([specs/brompton-tessera.yaml:5560](specs/brompton-tessera.yaml#L5560))
- [ ] Check that request_failover with an empty string hands over to the partner, and what it answers with no partner present. ([specs/brompton-tessera.yaml:1556](specs/brompton-tessera.yaml#L1556))
- [ ] On 3.6 or later, confirm the TrueLight endpoints and their ranges. ([specs/brompton-tessera.yaml:5584](specs/brompton-tessera.yaml#L5584))

## bss-london — BSS Soundweb London

- [ ] Confirm that nothing (no ACK or NAK byte) is sent back over TCP, and whether the device expects an ACK from the controller over Ethernet before it stops resending a subscription's message (the FAQ describes 1-second resends on serial). ([specs/bss-london.yaml:168](specs/bss-london.yaml#L168))
- [ ] Check that subscribing again to a subscribed state variable makes the device send the current value again (the read and the liveness check rely on it). ([specs/bss-london.yaml:103](specs/bss-london.yaml#L103))
- [ ] Check the SUBSCRIBE rate field's unit (ms, 50 ms steps) and what 0 means for a meter. ([specs/bss-london.yaml:120](specs/bss-london.yaml#L120))
- [ ] Check that venue and parameter preset recall bodies carry the 32-bit preset ID with no node address. ([specs/bss-london.yaml:147](specs/bss-london.yaml#L147))
- [ ] Check the SET STRING length field (string bytes plus the terminator) and the answer to a subscribed string SV. ([specs/bss-london.yaml:94](specs/bss-london.yaml#L94))
- [ ] Confirm which current London models (BLU-806, BLU-806DA and later) speak the same protocol unchanged. ([specs/bss-london.yaml:55](specs/bss-london.yaml#L55))

## canon-ptz — Canon PTZ and pro video cameras (XC protocol)

- [ ] Confirm every control.cgi key and value against a camera: they come from the Companion module, not from Canon's gated XC specification. ([specs/canon-ptz.yaml:1226](specs/canon-ptz.yaml#L1226))
- [ ] Record what control.cgi, standby.cgi, preset/set and trace/control answer on success and on a refused value (status 200 assumed for success). ([specs/canon-ptz.yaml:219](specs/canon-ptz.yaml#L219))
- [ ] Record a full info.cgi reply: the key:=value line format, line endings, and the values of f.standby, f.tally/f.tally.mode, shutter, iris, gain and ND. ([specs/canon-ptz.yaml:1129](specs/canon-ptz.yaml#L1129))
- [ ] Check whether info.cgi reports program and preview tally separately, or only the last-set tally with its mode. ([specs/canon-ptz.yaml:1141](specs/canon-ptz.yaml#L1141))
- [ ] On CR-N100, CR-N300, CR-N350 and CR-N400, check whether the iris value key is c.1.me.diaphragm (sent here) or me.diaphragm (sent by the Companion module). ([specs/canon-ptz.yaml:1265](specs/canon-ptz.yaml#L1265))
- [ ] Check the pan/tilt speed scale for pan.speed.dir and tilt.speed.dir (10-10000 assumed) and zoom.speed.dir (0-127). ([specs/canon-ptz.yaml:341](specs/canon-ptz.yaml#L341))
- [ ] Check the units and ranges of the absolute pan, tilt and zoom positions in pan_tilt_zoom_to and zoom_to. ([specs/canon-ptz.yaml:452](specs/canon-ptz.yaml#L452))
- [ ] Check that p.ptztime is in milliseconds (2000-99000) and p.ptzspeed 1-100. ([specs/canon-ptz.yaml:960](specs/canon-ptz.yaml#L960))
- [ ] Check save_preset_selective's option keys (ptz, focus, exp, wb, is, cp) and what cp saves. ([specs/canon-ptz.yaml:1003](specs/canon-ptz.yaml#L1003))
- [ ] Check whether set_exposure_mode needs manual shooting first, and the per-model value lists (shutter, iris, gain, ND, kelvin, digital magnification). ([specs/canon-ptz.yaml:648](specs/canon-ptz.yaml#L648))
- [ ] Check that on1 is the only "on" value of c.1.is. ([specs/canon-ptz.yaml:317](specs/canon-ptz.yaml#L317))
- [ ] Check Digest authentication with an administrator account and with guest access, on firmware with HTTPS enabled. ([specs/canon-ptz.yaml:57](specs/canon-ptz.yaml#L57))
- [ ] On the EOS C80 and XF605, confirm the XC protocol is enabled over the network the same way, and which commands they accept. ([specs/canon-ptz.yaml:305](specs/canon-ptz.yaml#L305))

## chamsys-magicq — ChamSys MagicQ (OSC)

- [ ] Check the blackout sense: whether /dbo 0 turns blackout on (manual) or off (Companion). ([specs/chamsys-magicq.yaml:1104](specs/chamsys-magicq.yaml#L1104))
- [ ] Record the feedback message addresses and arguments (/pb/<n>, /exec/<page>/<item>, /pb/<n>/flash), which come from Companion rather than ChamSys. ([specs/chamsys-magicq.yaml:1070](specs/chamsys-magicq.yaml#L1070))
- [ ] Check whether /rpc needs the Ethernet Remote Protocol set to an rx mode, and whether it is refused in net session mode. ([specs/chamsys-magicq.yaml:1112](specs/chamsys-magicq.yaml#L1112))
- [ ] Check the highest playback number the remote protocol accepts through /rpc (1-202, or 34 on consoles and 10 on PC). ([specs/chamsys-magicq.yaml:1122](specs/chamsys-magicq.yaml#L1122))
- [ ] Check the 10Scene zone range for /10Scene (1-100 assumed; the remote protocol documents 1-20). ([specs/chamsys-magicq.yaml:1132](specs/chamsys-magicq.yaml#L1132))
- [ ] Check the bounds chosen where the manual is silent: attribute values, times, cue stack ids and cue ids. ([specs/chamsys-magicq.yaml:1130](specs/chamsys-magicq.yaml#L1130))
- [ ] Check that exec level as a float and pb level as an int 0-100 behave as expected. ([specs/chamsys-magicq.yaml:1096](specs/chamsys-magicq.yaml#L1096))
- [ ] Check whether QuickQ DIN accepts OSC. ([specs/chamsys-magicq.yaml:1182](specs/chamsys-magicq.yaml#L1182))

## chamsys-magicq-udp — ChamSys MagicQ (UDP remote)

- [ ] Check where the "rx echo" mode sends its echo and whether a header is added. ([specs/chamsys-magicq-udp.yaml:781](specs/chamsys-magicq-udp.yaml#L781))
- [ ] Confirm commands sent to the console's address (not broadcast) are received. ([specs/chamsys-magicq-udp.yaml:791](specs/chamsys-magicq-udp.yaml#L791))
- [ ] Check the highest playback number the console accepts over the remote protocol (1-202, or 34 on consoles and 10 on PC). ([specs/chamsys-magicq-udp.yaml:806](specs/chamsys-magicq-udp.yaml#L806))
- [ ] Check the bounds chosen where the manual is silent: attribute values 0-65535, times 0-3600 s, cue stack ids 1-10000, cue ids 1-65536. ([specs/chamsys-magicq-udp.yaml:814](specs/chamsys-magicq-udp.yaml#L814))
- [ ] On QuickQ, check the jump cue form (whole numbers only) and the X zone range 0-10. ([specs/chamsys-magicq-udp.yaml:836](specs/chamsys-magicq-udp.yaml#L836))
- [ ] Check whether QuickQ DIN accepts remote control. ([specs/chamsys-magicq-udp.yaml:852](specs/chamsys-magicq-udp.yaml#L852))
- [ ] On MQ40 and MQ40N, confirm the remote protocol works. ([specs/chamsys-magicq-udp.yaml:830](specs/chamsys-magicq-udp.yaml#L830))

## christie-spyder — Christie Spyder X20 / X80

- [ ] Confirm that answers come back to the sending port with no "spyder" header, the result code first. ([specs/christie-spyder.yaml:46](specs/christie-spyder.yaml#L46))
- [ ] Record the exact answers to RLC, RSN, RBL, RRL, RLK and RCS (spacing, trailing characters), which the get_ commands return as text. ([specs/christie-spyder.yaml:731](specs/christie-spyder.yaml#L731))
- [ ] Check that Spyder decodes percent-encoding other than %20 in names (such as %28 for a parenthesis). ([specs/christie-spyder.yaml:846](specs/christie-spyder.yaml#L846))
- [ ] Check what learn_command_key answers (command key ID and script ID) on X20 and X80. ([specs/christie-spyder.yaml:146](specs/christie-spyder.yaml#L146))
- [ ] Check whether an X20 accepts ILA with the fifth (gamma) argument, which only the X80 reference lists. ([specs/christie-spyder.yaml:384](specs/christie-spyder.yaml#L384))
- [ ] Check how often UDP commands or answers are lost on a busy network, and whether a repeated command is harmless. ([specs/christie-spyder.yaml:890](specs/christie-spyder.yaml#L890))

## cockos-reaper — Cockos REAPER (OSC)

- [ ] Confirm REAPER's default Local listen port (8000) and Device port (9000) when a new OSC control surface is added. ([specs/cockos-reaper.yaml:59](specs/cockos-reaper.yaml#L59))
- [ ] Confirm that /device/track/count, /device/marker/count and /device/region/count sent on connecting take effect, and whether REAPER then sends the new bank's state at once. ([specs/cockos-reaper.yaml:79](specs/cockos-reaper.yaml#L79))
- [ ] Record the OSC types REAPER sends feedback in for b and t patterns (float 1.0 and 0.0 or integers); the state accepts both. ([specs/cockos-reaper.yaml:597](specs/cockos-reaper.yaml#L597))
- [ ] Confirm that REAPER takes binary values as OSC integers, as reaper-osc.js sends them, and floats where the pattern is n or f. ([specs/cockos-reaper.yaml:153](specs/cockos-reaper.yaml#L153))
- [ ] Confirm the inverted FX bypass (0 bypasses, 1 makes active) in both directions. ([specs/cockos-reaper.yaml:382](specs/cockos-reaper.yaml#L382))
- [ ] Record the track monitor values REAPER takes and sends (0 off, 1 on, 2 tape-auto, from reaper-osc.js) and the OSC type of the feedback, which the state reads as an integer. ([specs/cockos-reaper.yaml:652](specs/cockos-reaper.yaml#L652))
- [ ] Check whether /time, /tempo/raw and /playrate/raw sent to REAPER move the cursor and set the tempo and rate, and the tempo range REAPER accepts. ([specs/cockos-reaper.yaml:181](specs/cockos-reaper.yaml#L181))
- [ ] Check the lowest and highest dB /track/@/volume/db accepts (REAPER's fader-range preference assumed for the top). ([specs/cockos-reaper.yaml:226](specs/cockos-reaper.yaml#L226))
- [ ] Confirm that /action/str runs a named command ID (_SWS_..., a script's ID). ([specs/cockos-reaper.yaml:452](specs/cockos-reaper.yaml#L452))

## dataton-watchout6 — Dataton WATCHOUT 6

- [ ] Confirm that every ID-tagged command ([m]) gets a tagged reply on WATCHOUT 6 (production and display) as the guide says, including an empty [m] for a successful action; WATCHOUT 7 documents the empty tag, the 6.x guide does not. ([specs/dataton-watchout6.yaml:489](specs/dataton-watchout6.yaml#L489))
- [ ] Check the reply to authenticate 1 on a production computer and a display cluster, and which Error 8 sub-codes a refused login gives (1, 4, 5 and 6 are taken as refusals). ([specs/dataton-watchout6.yaml:81](specs/dataton-watchout6.yaml#L81))
- [ ] Confirm the field order of the general Status push after getStatus 1 (taken from Bitfocus's Companion module) and how often it is sent. ([specs/dataton-watchout6.yaml:533](specs/dataton-watchout6.yaml#L533))
- [ ] Check that the timeline Status line for a timeline in a folder has a space before :mItemList, as the knowledge base article shows. ([specs/dataton-watchout6.yaml:537](specs/dataton-watchout6.yaml#L537))
- [ ] Check the exact keyword of the information message (Information is assumed from the guide's heading). ([specs/dataton-watchout6.yaml:445](specs/dataton-watchout6.yaml#L445))
- [ ] Check the syntax and reply of powerDown on a WATCHOUT 6 display cluster; only WATCHOUT 7's compatibility page gives it. ([specs/dataton-watchout6.yaml:507](specs/dataton-watchout6.yaml#L507))
- [ ] Check whether getAuxTimelines, getControlCues and getInputs exist on WATCHOUT 6 (documented only for WATCHOUT 7's compatibility layer), and their JSON. ([specs/dataton-watchout6.yaml:544](specs/dataton-watchout6.yaml#L544))
- [ ] Check how load_display reports progress and completion: the command takes the first tagged reply, which may be a Busy line. ([specs/dataton-watchout6.yaml:501](specs/dataton-watchout6.yaml#L501))
- [ ] On WATCHOUT 7, check the show name in the getStatus reply (the show file's name, or its id as an older page says). ([specs/dataton-watchout6.yaml:542](specs/dataton-watchout6.yaml#L542))
- [ ] Confirm that spontaneous messages go only to the most recently connected controller. ([specs/dataton-watchout6.yaml:493](specs/dataton-watchout6.yaml#L493))

## dataton-watchout7 — Dataton WATCHOUT 7

- [ ] Capture /api-docs/openapi.json from a node and check success statuses (200 assumed), error bodies, and the response shapes of /info, /v0/state, /v0/timelines, /v0/cues and /v0/inputs. ([specs/dataton-watchout7.yaml:347](specs/dataton-watchout7.yaml#L347))
- [ ] Confirm the /v2/sse event shape {"kind", "value"} without event names (from Bitfocus's Companion module), and the playbackState, timelineCountdowns and showRevision value shapes. ([specs/dataton-watchout7.yaml:355](specs/dataton-watchout7.yaml#L355))
- [ ] Check whether /v0/play needs a JSON body (current guide) or plays every timeline with none (older page). ([specs/dataton-watchout7.yaml:154](specs/dataton-watchout7.yaml#L154))
- [ ] Check the node restart path on port 3017: /v0/services/restart (Dataton) or /v0/restart (Companion module). ([specs/dataton-watchout7.yaml:33](specs/dataton-watchout7.yaml#L33))
- [ ] Check the type of showRevision's value (a string is assumed). ([specs/dataton-watchout7.yaml:322](specs/dataton-watchout7.yaml#L322))
- [ ] Check how names with spaces or non-ASCII characters are encoded in /v0/cue-group-state/by-name/{group}/{variant}. ([specs/dataton-watchout7.yaml:237](specs/dataton-watchout7.yaml#L237))

## digico-sd — DiGiCo SD and Quantum consoles

- [ ] Confirm that current SD and Quantum software still accepts the 2014 Other OSC list's /sd/ addresses, on an SD-series console and on a Quantum (225, 338, 5, 7 or 852). ([specs/digico-sd.yaml:18](specs/digico-sd.yaml#L18))
- [ ] Check whether the console sends anything back to an Other OSC device (changes made on the surface, or echoes of received messages), and on which port. ([specs/digico-sd.yaml:2684](specs/digico-sd.yaml#L2684))
- [ ] Record the fader position for 0 dB and a few other levels, and what the 0.0-1.0 range covers for trim, analogue gain, EQ frequency and gain, and delay. ([specs/digico-sd.yaml:2692](specs/digico-sd.yaml#L2692))
- [ ] Check what a message for a strip the session does not have does (ignored, or something else), and the real strip limits per console. ([specs/digico-sd.yaml:2701](specs/digico-sd.yaml#L2701))
- [ ] Confirm press_macro sends macro 1 as 0 (the list's 0-255 range and the Companion module), not 1. ([specs/digico-sd.yaml:2709](specs/digico-sd.yaml#L2709))
- [ ] Check that fire_snapshot takes the snapshot number shown on the console (and what happens for a number with no snapshot, or a decimal-numbered snapshot). ([specs/digico-sd.yaml:2639](specs/digico-sd.yaml#L2639))
- [ ] Check the meaning of the Int ranges taken as given: input phase 0-3, EQ curve 1-4, compressor knee 0-2, dynamic EQ over/under 0-1. ([specs/digico-sd.yaml:776](specs/digico-sd.yaml#L776))
- [ ] Check the longest channel name the console accepts, and what it does with a longer one. ([specs/digico-sd.yaml:832](specs/digico-sd.yaml#L832))
- [ ] Check the addresses left out (Channel_Input/main/alt_in, CGs_level, CGs_mute, Matrix_Inputs sends, Multis) and what they do. ([specs/digico-sd.yaml:2722](specs/digico-sd.yaml#L2722))

## disguise — disguise Designer

- [ ] Confirm the Live Update subscription to one Python dictionary property of transportmanager:<name> (player.playing, player.tRender, player.playMode.state, player.track.description, engaged, volume, brightness) and the valuesChanged messages it produces. ([specs/disguise.yaml:608](specs/disguise.yaml#L608))
- [ ] Check that a Locator with only a name (no uid key) is accepted by every transport and failover command. ([specs/disguise.yaml:573](specs/disguise.yaml#L573))
- [ ] Check that gototime takes seconds and gotoframe a frame count, and the units of track length and annotation times. ([specs/disguise.yaml:582](specs/disguise.yaml#L582))
- [ ] Check the status.code values Designer returns (4000 seen for a rejected play mode) and the shape of status.details. ([specs/disguise.yaml:568](specs/disguise.yaml#L568))
- [ ] Check whether volume and brightness outside 0 to 1 are ignored, as reported for r34. ([specs/disguise.yaml:584](specs/disguise.yaml#L584))
- [ ] Check which Designer release added each endpoint used here; the Swagger documents carry no version markers. ([specs/disguise.yaml:107](specs/disguise.yaml#L107))
- [ ] Check the health severity values (ready in the Swagger document, ok in the monitoring guide). ([specs/disguise.yaml:540](specs/disguise.yaml#L540))

## emberplus — Ember+

- [ ] Check the device's Ember+ port, since the module's default of 9000 is a common choice, not a standard. ([specs/emberplus.yaml:199](specs/emberplus.yaml#L199))
- [ ] Check whether the provider repeats the DTD and application bytes in each packet of a multi-packet message, and that it accepts the module's header in every packet. ([specs/emberplus.yaml:234](specs/emberplus.yaml#L234))
- [ ] Confirm the provider accepts DTD version 2.50 sent as 0x32 0x02. ([specs/emberplus.yaml:233](specs/emberplus.yaml#L233))
- [ ] Confirm the module's rule for knowing a GetDirectory has been answered holds with the provider in use, especially one that splits a directory across messages. ([specs/emberplus.yaml:248](specs/emberplus.yaml#L248))
- [ ] Check what value a trigger parameter expects. ([specs/emberplus.yaml:269](specs/emberplus.yaml#L269))
- [ ] Check whether connect and disconnect matrix operations work on 1:N matrices, and which disposition values the provider returns. ([specs/emberplus.yaml:274](specs/emberplus.yaml#L274))
- [ ] Confirm the FieldFlags values (prose) and connections 5 are accepted. ([specs/emberplus.yaml:258](specs/emberplus.yaml#L258))
- [ ] Confirm StreamCollection and StreamEntry tags and the templateReference tag match the provider. ([specs/emberplus.yaml:224](specs/emberplus.yaml#L224))
- [ ] Opened for commands only, check that an Unsubscribe on a looked-up node (or as a bare command at the root) ends the value subscription its GetDirectory created. ([crates/core/src/modules/emberplus.rs:35](crates/core/src/modules/emberplus.rs#L35))

## etc-eos — ETC Eos

- [ ] Check whether /eos/param levels take 0-100 or the parameter's own units (pan 270). ([specs/etc-eos.yaml:4712](specs/etc-eos.yaml#L4712))
- [ ] Confirm wheel and switch ticks are accepted as floats. ([specs/etc-eos.yaml:4722](specs/etc-eos.yaml#L4722))
- [ ] Check which fader level output address the console sends (/eos/out/fader/... or /eos/fader/...), and whether a label string ever arrives on the level address. ([specs/etc-eos.yaml:4738](specs/etc-eos.yaml#L4738))
- [ ] Check the argument shape of /eos/out/active/chan (one string, or int plus string). ([specs/etc-eos.yaml:4747](specs/etc-eos.yaml#L4747))
- [ ] Check relay event sense (0 = on or 0 = off). ([specs/etc-eos.yaml:4755](specs/etc-eos.yaml#L4755))
- [ ] Confirm OSC Get by index and UID uses the path form /eos/get/<type>/index/<n>, and which address /eos/get/setup replies on. ([specs/etc-eos.yaml:4762](specs/etc-eos.yaml#L4762))
- [ ] Record the reply shape of get_session, which is not documented. ([specs/etc-eos.yaml:4817](specs/etc-eos.yaml#L4817))
- [ ] Confirm macro text arrives on /eos/out/get/macro/<n>/text/list/... ([specs/etc-eos.yaml:4807](specs/etc-eos.yaml#L4807))
- [ ] Confirm the console uses the default /eos cue OSC strings, or note the configured ones. ([specs/etc-eos.yaml:4801](specs/etc-eos.yaml#L4801))

## etc-paradigm — ETC Paradigm (PSAP)

- [ ] Check whether action commands get any reply over UDP (they report unverified here). ([specs/etc-paradigm.yaml:1017](specs/etc-paradigm.yaml#L1017))
- [ ] Check whether a fade time without a space name works: the guide's example "grp int:128 Group 1, 3" contradicts its rule that a fade time needs a space name. ([specs/etc-paradigm.yaml:1032](specs/etc-paradigm.yaml#L1032))
- [ ] Confirm that a get reply always includes the space name and levels as 0-255, and record the reply to a get for an unknown name. ([specs/etc-paradigm.yaml:1025](specs/etc-paradigm.yaml#L1025))
- [ ] Check where PSAP trigger output is sent (the destination set in LightDesigner) and that it reaches feedback_port. ([specs/etc-paradigm.yaml:1018](specs/etc-paradigm.yaml#L1018))
- [ ] Confirm that fade times with one decimal (2.5) and the sequence rate with two (1.50) are accepted. ([specs/etc-paradigm.yaml:732](specs/etc-paradigm.yaml#L732))

## evertz-quartz — Evertz Quartz protocol routers

- [ ] Confirm the control port of each product (23 assumed per AN65; EQT, EMR and 7700R use 3737-3740, the 7700R also 2000). ([specs/evertz-quartz.yaml:97](specs/evertz-quartz.yaml#L97))
- [ ] Check that a route (.S) is answered only by a .U update, and that .E comes only for commands the router cannot parse. ([specs/evertz-quartz.yaml:414](specs/evertz-quartz.yaml#L414))
- [ ] Check what an interrogate of a missing destination returns on EQX and MAGNUM (AN65 says nothing). ([specs/evertz-quartz.yaml:436](specs/evertz-quartz.yaml#L436))
- [ ] Check that numbers without leading zeros are accepted, and that replies use three digits. ([specs/evertz-quartz.yaml:457](specs/evertz-quartz.yaml#L457))
- [ ] Check whether lock and name changes made elsewhere are pushed as .BA or .RA. ([specs/evertz-quartz.yaml:484](specs/evertz-quartz.yaml#L484))
- [ ] Check which commands MAGNUM implements beyond routing, interrogate, locks, salvos and names. ([specs/evertz-quartz.yaml:484](specs/evertz-quartz.yaml#L484))
- [ ] Check whether name replies carry the number on current firmware. ([specs/evertz-quartz.yaml:473](specs/evertz-quartz.yaml#L473))
- [ ] Record the tieline source encoding (source in the lower 12 bits, level above) on a system with tielines. ([specs/evertz-quartz.yaml:448](specs/evertz-quartz.yaml#L448))

## extron-matrix — Extron SIS matrix switchers

- [ ] Confirm that a CR after a simple command (1*2!) is ignored. ([specs/extron-matrix.yaml:931](specs/extron-matrix.yaml#L931))
- [ ] Record what a wrong password produces (the prompt again assumed) and whether the unit sends Telnet option bytes before the banner. ([specs/extron-matrix.yaml:941](specs/extron-matrix.yaml#L941))
- [ ] Check how often a change report arrives while a command waits, and whether replies and reports can be told apart. ([specs/extron-matrix.yaml:916](specs/extron-matrix.yaml#L916))
- [ ] Record the Esc VM mute reply on DXP and XTP II (with and without the "Mut00" prefix). ([specs/extron-matrix.yaml:955](specs/extron-matrix.yaml#L955))
- [ ] On DXP HD 4K PLUS, check set_input_attenuation_db at 0 dB (1*0G). ([specs/extron-matrix.yaml:971](specs/extron-matrix.yaml#L971))
- [ ] On SMX, confirm two-digit plane numbers in commands and the plane-first reply form. ([specs/extron-matrix.yaml:985](specs/extron-matrix.yaml#L985))
- [ ] On XTP II, confirm the Exec form of executive-mode reports. ([specs/extron-matrix.yaml:978](specs/extron-matrix.yaml#L978))

## extron-switcher — Extron IN-series scaling presentation switchers

- [ ] Record the tagged reply forms in verbose mode 3 (In X!*1 All, VnamI, Inf00, GrpmD padding). ([specs/extron-switcher.yaml:1031](specs/extron-switcher.yaml#L1031))
- [ ] Confirm that a CR after a simple command (1*3!, 2B, 1X) is ignored. ([specs/extron-switcher.yaml:1025](specs/extron-switcher.yaml#L1025))
- [ ] Check how often a change report arrives while a command waits. ([specs/extron-switcher.yaml:1009](specs/extron-switcher.yaml#L1009))
- [ ] On IN1606/IN1608, check whether the unsolicited signal message (IN00 ...) is sent, as on IN1808. ([specs/extron-switcher.yaml:1063](specs/extron-switcher.yaml#L1063))
- [ ] Confirm the HDCP status codes on each family (1 and 2 are documented the opposite way). ([specs/extron-switcher.yaml:1070](specs/extron-switcher.yaml#L1070))
- [ ] Record what a wrong password produces (the prompt again assumed). ([specs/extron-switcher.yaml:1018](specs/extron-switcher.yaml#L1018))

## freeshow — FreeShow

- [ ] Record the status code and body FreeShow's REST API answers to an action (204 assumed) and to a get_ query (200 with JSON assumed), and to an unknown action. ([specs/freeshow.yaml:1489](specs/freeshow.yaml#L1489))
- [ ] Check whether positions (index_select_slide, index_select_project, index_select_project_item, index_select_overlay) count from 1 or from 0. ([specs/freeshow.yaml:1497](specs/freeshow.yaml#L1497))
- [ ] Check that a get_cleared POST is harmless and answered while nothing is on output, since it is the liveness probe. ([specs/freeshow.yaml:65](specs/freeshow.yaml#L65))
- [ ] With an API password set, confirm a wrong or missing Bearer token gets 401 on REST, and that versions before 1.6.6-beta.4 ignore the header. ([specs/freeshow.yaml:1481](specs/freeshow.yaml#L1481))
- [ ] Check that numbers sent as JSON numbers are accepted (the Companion module sends every number and boolean as a string). ([specs/freeshow.yaml:321](specs/freeshow.yaml#L321))
- [ ] Check the change_volume range (0-1 assumed, from the Companion module) and the transition types and easing names. ([specs/freeshow.yaml:768](specs/freeshow.yaml#L768))
- [ ] Check that set_plain_text and create_show take line breaks in the text and split slides on a blank line. ([specs/freeshow.yaml:215](specs/freeshow.yaml#L215))
- [ ] Record what get_shows, get_output, get_slide, get_timers and get_cleared return, so state rules can be written for them. ([specs/freeshow.yaml:1205](specs/freeshow.yaml#L1205))
- [ ] Confirm that FreeShow's Socket.IO server on 5505 (Engine.IO 4 assumed) answers the variable request {"isVariable": true, "keys": [...]} on the "data" event with {isVariable: true, values: {...}}, as the Companion module receives it. ([specs/freeshow.yaml:1697](specs/freeshow.yaml#L1697))
- [ ] Record the type each variable arrives as: whether slide_number and layout_slides are numbers, and whether output_windows_active, outputs_locked and log_song_usage are true/false or text. ([specs/freeshow.yaml:1752](specs/freeshow.yaml#L1752))
- [ ] Check what slide_number counts from (1 assumed for the slide shown) and what it holds with nothing on output. ([specs/freeshow.yaml:1660](specs/freeshow.yaml#L1660))
- [ ] With an API password set, confirm the socket takes it as the handshake auth {"token": key}, refuses the namespace without it, and accepts {"token": ""} when no password is set. ([specs/freeshow.yaml:1693](specs/freeshow.yaml#L1693))
- [ ] Check whether FreeShow pushes changes unasked on the socket, which would let the once-a-second variable request go slower. ([specs/freeshow.yaml:1695](specs/freeshow.yaml#L1695))

## grandma2 — MA Lighting grandMA2

- [ ] Record exactly what the console sends over telnet after a failed Login, since a failed login cannot be detected now and every later command is silently discarded. ([specs/grandma2.yaml:1164](specs/grandma2.yaml#L1164))
- [ ] Check what the console greets with before login (it claims the guest user) and whether that text differs from a logged-in session. ([specs/grandma2.yaml:1161](specs/grandma2.yaml#L1161))
- [ ] Check whether Login "user" "" is accepted for a user without a password. ([specs/grandma2.yaml:1170](specs/grandma2.yaml#L1170))
- [ ] Confirm the username is case sensitive over telnet ("administrator" against "Administrator"). ([specs/grandma2.yaml:1167](specs/grandma2.yaml#L1167))
- [ ] Check whether At accepts a fractional percentage. ([specs/grandma2.yaml:1235](specs/grandma2.yaml#L1235))
- [ ] Check the upper bounds of pool numbers and fixture and channel IDs (9999 and 99999 assumed). ([specs/grandma2.yaml:1226](specs/grandma2.yaml#L1226))
- [ ] Check the units and ranges of Rate, Speed and MasterFade values. ([specs/grandma2.yaml:1237](specs/grandma2.yaml#L1237))

## grandma3 — MA Lighting grandMA3

- [ ] Confirm the OSC port default of 8000 on a fresh configuration line. ([specs/grandma3.yaml:1727](specs/grandma3.yaml#L1727))
- [ ] Check where sequences live in the object tree on the software in use (/13.13.1.6.X or 13.13.<pool>.7.X after 2.5), and whether sequence feedback still matches. ([specs/grandma3.yaml:1764](specs/grandma3.yaml#L1764))
- [ ] Record the feedback formats for groups, presets, sounds, worlds, plugin components, screen configurations, timers and sii relative encoder feedback, which MA does not give. ([specs/grandma3.yaml:1744](specs/grandma3.yaml#L1744))
- [ ] Check whether the sequence feedback text uses ";" between name and cue number on the version in use. ([specs/grandma3.yaml:1755](specs/grandma3.yaml#L1755))
- [ ] Check which user's rights apply to OSC /cmd commands. ([specs/grandma3.yaml:1775](specs/grandma3.yaml#L1775))
- [ ] Confirm unpaged /FaderX and /KeyX work (the SendOSC page says a page must be given). ([specs/grandma3.yaml:1792](specs/grandma3.yaml#L1792))
- [ ] Confirm the encoder address is /EncoderX (the menu calls it ExecutorKnob). ([specs/grandma3.yaml:1800](specs/grandma3.yaml#L1800))
- [ ] Confirm key and fader function names other than Flash, Black and FaderMaster (Go+, Swap, FaderCrossFade and others) are accepted over OSC. ([specs/grandma3.yaml:1816](specs/grandma3.yaml#L1816))
- [ ] Confirm object_fader's second argument 3 and object_fader_relative's handle type values. ([specs/grandma3.yaml:1819](specs/grandma3.yaml#L1819))
- [ ] Check the command-line forms composed from syntax rather than examples: Page X.Y with Goto, Load and On/Off, "FaderMaster Master 2.1 At", "Group N At", and the Rate/Speed targets. ([specs/grandma3.yaml:1826](specs/grandma3.yaml#L1826))
- [ ] Check the fader_fade fade argument (two integers) and any upper bound on fade time. ([specs/grandma3.yaml:1807](specs/grandma3.yaml#L1807))
- [ ] Check the upper bounds of cue and pool numbers (four digits, 9999, 99999 assumed). ([specs/grandma3.yaml:1831](specs/grandma3.yaml#L1831))

## greenhippo-hippotizer — Green Hippo Hippotizer

- [ ] Check whether mix, layer and timeline indexes start at 0 or 1 (mix and layer from 1, timeline by iD are assumed). ([specs/greenhippo-hippotizer.yaml:656](specs/greenhippo-hippotizer.yaml#L656))
- [ ] Check the single preset number of load_mix_preset and load_layer_preset (bank * 256 + slot per the Companion module). ([specs/greenhippo-hippotizer.yaml:654](specs/greenhippo-hippotizer.yaml#L654))
- [ ] Check the body of a level reply (a bare integer is assumed) and of the action endpoints. ([specs/greenhippo-hippotizer.yaml:240](specs/greenhippo-hippotizer.yaml#L240))
- [ ] Capture the JSON wrapper of GET /timelines, which the Swagger document leaves empty. ([specs/greenhippo-hippotizer.yaml:672](specs/greenhippo-hippotizer.yaml#L672))
- [ ] Check that the Web Callbacks websocket on 40513 accepts the subscription array sent on opening, with the path /, and whether it acknowledges it. ([specs/greenhippo-hippotizer.yaml:492](specs/greenhippo-hippotizer.yaml#L492))
- [ ] Check whether the presets-reset event is PRESETS_RESET or PRESES_RESET (the manual's example). ([specs/greenhippo-hippotizer.yaml:685](specs/greenhippo-hippotizer.yaml#L685))
- [ ] Check that DELETE /media/delete/{id}, DELETE /media/deletemapentry/{index} and PUT /media/addmapentry/... behave as their GET forms. ([specs/greenhippo-hippotizer.yaml:626](specs/greenhippo-hippotizer.yaml#L626))
- [ ] Check how a pin name containing an underscore is written in a REST pin path. ([specs/greenhippo-hippotizer.yaml:649](specs/greenhippo-hippotizer.yaml#L649))
- [ ] Check whether the media id and preset id path forms are told apart from map indexes and preset numbers by the segment being a number. ([specs/greenhippo-hippotizer.yaml:663](specs/greenhippo-hippotizer.yaml#L663))
- [ ] Capture the 4.9.x REST additions (media encoding settings, strata folder, user name and colour) from the API help served at port 40512. ([specs/greenhippo-hippotizer.yaml:57](specs/greenhippo-hippotizer.yaml#L57))

## h2r-graphics — H2R Graphics

- [ ] Record the HTTP status of a successful request (200 assumed) and of an error reply from v3.4 onward. ([specs/h2r-graphics.yaml:352](specs/h2r-graphics.yaml#L352))
- [ ] Confirm the text variable id form text.N in updateVariableText. ([specs/h2r-graphics.yaml:391](specs/h2r-graphics.yaml#L391))
- [ ] Check the update_score team and level ranges. ([specs/h2r-graphics.yaml:398](specs/h2r-graphics.yaml#L398))
- [ ] Check whether select_list_row counts rows from 1 or 0. ([specs/h2r-graphics.yaml:401](specs/h2r-graphics.yaml#L401))
- [ ] On version 2, check how an older version answers commands it predates. ([specs/h2r-graphics.yaml:383](specs/h2r-graphics.yaml#L383))

## highend-hog4 — High End Systems Hog 4 / Hog OS

- [ ] Confirm that playback commands take the object number as the argument (/hog/playback/go/0 with 15), as the manual says, and not in the address as the Companion module sends it. ([specs/highend-hog4.yaml:507](specs/highend-hog4.yaml#L507))
- [ ] Check how goto_cue's float list.cue is read (15.2 sent as 15.19999981; cue 2 versus 20; point cues). ([specs/highend-hog4.yaml:516](specs/highend-hog4.yaml#L516))
- [ ] Record the argument types of the status outputs (LED value, LED colour, command line, encoder and H-key labels, chat lines). ([specs/highend-hog4.yaml:497](specs/highend-hog4.yaml#L497))
- [ ] Find the address the console uses for fader level status after consolefaderrefresh. ([specs/highend-hog4.yaml:491](specs/highend-hog4.yaml#L491))
- [ ] Check the real ranges of H keys, U keys, encoder wheels and the trackball values per model. ([specs/highend-hog4.yaml:524](specs/highend-hog4.yaml#L524))
- [ ] Check which keys of the Hog OS 5 table work on Hog 4 OS 3.x and 4.x. ([specs/highend-hog4.yaml:534](specs/highend-hog4.yaml#L534))
- [ ] Confirm that refreshall makes the console send every status, and whether repeating it every 60 s is noticeable on the console. ([specs/highend-hog4.yaml:489](specs/highend-hog4.yaml#L489))
- [ ] Check whether the HPU acts on front panel keys, encoders and faders sent over OSC. ([specs/highend-hog4.yaml:556](specs/highend-hog4.yaml#L556))

## kramer-p3000 — Kramer Protocol 3000

- [ ] Check whether ROUTE with source 0 disconnects, as the legacy commands do. ([specs/kramer-p3000.yaml:1135](specs/kramer-p3000.yaml#L1135))
- [ ] Check whether AUD-LVL's channel means the Audio Channel table or an input/output number on the product in use. ([specs/kramer-p3000.yaml:1164](specs/kramer-p3000.yaml#L1164))
- [ ] Confirm the documentation inconsistencies taken as written: the LABEL query with only a port number, NAME? echoed with the question mark, the #INFO-PRST? query form, and PRST-STO/PRST-RCL replies without "nn@". ([specs/kramer-p3000.yaml:1151](specs/kramer-p3000.yaml#L1151))
- [ ] Find the LOGIN level argument's wire spelling, so login can be modelled. ([specs/kramer-p3000.yaml:1176](specs/kramer-p3000.yaml#L1176))
- [ ] Record the exact X-SIGNAL and X-AFV reply forms, which are matched leniently because the guide has typing errors. ([specs/kramer-p3000.yaml:1187](specs/kramer-p3000.yaml#L1187))
- [ ] Confirm VMUTE flag 2 (blank picture) is unsupported. ([specs/kramer-p3000.yaml:1162](specs/kramer-p3000.yaml#L1162))

## lightware-lw2 — Lightware LW2 (bracket protocol) matrices and switchers

- [ ] Confirm that a CR LF after the closing bracket is accepted (Companion's module sends it; the manuals give no terminator). ([specs/lightware-lw2.yaml:463](specs/lightware-lw2.yaml#L463))
- [ ] Record the error answers ("(ERR04)" and others) and whether a failed command is answered at all. ([specs/lightware-lw2.yaml:463](specs/lightware-lw2.yaml#L463))
- [ ] Check whether front panel changes are reported unasked on TCP 10001 (none documented; the crosspoint is polled). ([specs/lightware-lw2.yaml:470](specs/lightware-lw2.yaml#L470))
- [ ] Confirm the EDID learn order ({location>output}, as the manual's example). ([specs/lightware-lw2.yaml:496](specs/lightware-lw2.yaml#L496))
- [ ] Check how the UMX answers {VC} without a layer. ([specs/lightware-lw2.yaml:470](specs/lightware-lw2.yaml#L470))

## lightware-lw3 — Lightware LW3 matrices, switchers and extenders

- [ ] Check whether "OPEN <node>/*" also subscribes to grandchild nodes. ([specs/lightware-lw3.yaml:2541](specs/lightware-lw3.yaml#L2541))
- [ ] Record the answers to OPEN, GET and CALL on each tree, including the error lines for another tree's paths. ([specs/lightware-lw3.yaml:2530](specs/lightware-lw3.yaml#L2530))
- [ ] On MX2, confirm the preset, device label and muteSource syntax where the manual contradicts itself, and the /MEDIA/NAMES/VIDEO port names. ([specs/lightware-lw3.yaml:2582](specs/lightware-lw3.yaml#L2582))
- [ ] On MMX2, check whether method replies are mO (the manual prints m0 in places). ([specs/lightware-lw3.yaml:2594](specs/lightware-lw3.yaml#L2594))
- [ ] On UMX-HDMI-140-Plus, record the answer to a wrong Cleartext login password. ([specs/lightware-lw3.yaml:2615](specs/lightware-lw3.yaml#L2615))
- [ ] Record the MX2 and UMX port status letters and bytes on a live port. ([specs/lightware-lw3.yaml:2564](specs/lightware-lw3.yaml#L2564))

## megapixel-helios — Megapixel HELIOS

- [ ] Confirm that the JSON-RPC websocket answers {"jsonrpc":"2.0","id":1,"method":"state"} with the whole tree under result, then pushes updates without a subscription, and that it takes the web application's credentials when authentication is on. ([specs/megapixel-helios.yaml:520](specs/megapixel-helios.yaml#L520))
- [ ] Check that basic access is accepted when authentication is on, or that the processor's digest challenge is answered. ([specs/megapixel-helios.yaml:83](specs/megapixel-helios.yaml#L83))
- [ ] Confirm that a PATCH out of range answers 200 with the unchanged value, as the document says, on current firmware. ([specs/megapixel-helios.yaml:2827](specs/megapixel-helios.yaml#L2827))
- [ ] Record the ranges of display gains, output adjustment gain, gamma, lift, offset and saturation, which the document does not give. ([specs/megapixel-helios.yaml:199](specs/megapixel-helios.yaml#L199))
- [ ] Confirm that a group's gains are keyed r, g, b and i, its mask l, t, b and r, and its test pattern colour r, g, b and a, and record their ranges (0-1 or 0-255). ([specs/megapixel-helios.yaml:362](specs/megapixel-helios.yaml#L362))
- [ ] Confirm that blackClipping can be written (it is in the processor's data and fixtures, not the API tables). ([specs/megapixel-helios.yaml:191](specs/megapixel-helios.yaml#L191))
- [ ] Check whether redundancy mode accepts manual and single, which sys.fixtures lists beside none, failover and seamless. ([specs/megapixel-helios.yaml:286](specs/megapixel-helios.yaml#L286))
- [ ] List the test pattern type names the processor accepts. ([specs/megapixel-helios.yaml:2900](specs/megapixel-helios.yaml#L2900))
- [ ] Check that a receiver's x, y and groupId can be written by MAC address. ([specs/megapixel-helios.yaml:426](specs/megapixel-helios.yaml#L426))
- [ ] Record the status of a successful preset apply (200 is assumed) and of hide_still (204 is assumed, as for show). ([specs/megapixel-helios.yaml:458](specs/megapixel-helios.yaml#L458))

## millumin — Millumin

- [ ] Confirm the default OSC input port of Millumin 5 (5000 in the documentation; a V5 screenshot shows 8000). ([specs/millumin.yaml:722](specs/millumin.yaml#L722))
- [ ] Check that changes made through the OSC API send no feedback, and which side effects (a column's media starting) still do. ([specs/millumin.yaml:689](specs/millumin.yaml#L689))
- [ ] Check where /? and /ping answers go (the configured senders are assumed) and what /ping sends. ([specs/millumin.yaml:692](specs/millumin.yaml#L692))
- [ ] Check the light intensity scale on input (0-255 in the documentation's example) and feedback (0 to 1), and the master levels' range. ([specs/millumin.yaml:709](specs/millumin.yaml#L709))
- [ ] Check the rate of /millumin/layer:<name>/media/time feedback and whether it is sent for every layer. ([specs/millumin.yaml:638](specs/millumin.yaml#L638))
- [ ] Check whether Millumin 5 still has a global API feedback switch beside each sender's send feedback. ([specs/millumin.yaml:682](specs/millumin.yaml#L682))
- [ ] Check which of the dashboard and the edited timeline play, pause and go_to_time act on. ([specs/millumin.yaml:703](specs/millumin.yaml#L703))
- [ ] Check that stop_column takes no column in Millumin 5. ([specs/millumin.yaml:705](specs/millumin.yaml#L705))
- [ ] Check that floats are accepted where the documentation shows whole numbers (positions, rotation, media time). ([specs/millumin.yaml:398](specs/millumin.yaml#L398))

## newtek-tricaster — NewTek TriCaster

- [ ] Record the response body of a shortcut request, since whether the shortcut applied is not confirmed by the 200. ([specs/newtek-tricaster.yaml:269](specs/newtek-tricaster.yaml#L269))
- [ ] Confirm the login scheme (Basic sent, Digest answered if challenged). ([specs/newtek-tricaster.yaml:262](specs/newtek-tricaster.yaml#L262))

## novastar-central-control — NovaStar COEX central control protocol

- [ ] Record a refused request's answer (a non-zero ACK byte is assumed) and whether its checksum covers the aa 55 header as the documented answer's does. ([specs/novastar-central-control.yaml:153](specs/novastar-central-control.yaml#L153))
- [ ] Check whether the controller keeps an idle TCP connection open, and whether it answers anything that could serve as a liveness check. ([specs/novastar-central-control.yaml:146](specs/novastar-central-control.yaml#L146))
- [ ] Confirm that output card 255 addresses every card on single-card controllers and that card numbers start from 1 on MX6000 Pro and MX2000 Pro. ([specs/novastar-central-control.yaml:169](specs/novastar-central-control.yaml#L169))
- [ ] Confirm the card-based MX6000 Pro and MX2000 Pro answer the same frames as the single-card controllers. ([specs/novastar-central-control.yaml:52](specs/novastar-central-control.yaml#L52))

## novastar-coex — NovaStar COEX

- [ ] Check which API generation each model and firmware answers: the current OpenAPI paths (/api/v1/screen/..., /api/v1/preset/...) or the 2023 manual's (/api/v1/device/screen/..., /api/v1/device/currentpreset). ([specs/novastar-coex.yaml:854](specs/novastar-coex.yaml#L854))
- [ ] Confirm that the endpoints the OpenAPI pages list without /api/v1 (internal source, colour correction, 3D emitter, sync source, identify, controller name, no-signal image, thermal amount) answer with the prefix. ([specs/novastar-coex.yaml:1024](specs/novastar-coex.yaml#L1024))
- [ ] Record the displayMode numbers the display state read reports for blackout and freeze. ([specs/novastar-coex.yaml:1032](specs/novastar-coex.yaml#L1032))
- [ ] Check whether requests need the Device-Key header shown in the OpenAPI examples. ([specs/novastar-coex.yaml:1064](specs/novastar-coex.yaml#L1064))
- [ ] Record the screenID format (the examples here assume a braced GUID-like string) and that a one-element screenIdList acts on that screen only. ([specs/novastar-coex.yaml:234](specs/novastar-coex.yaml#L234))
- [ ] Check switch_layer_source in send-only and all-in-one modes, and what layer IDs each mode uses. ([specs/novastar-coex.yaml:381](specs/novastar-coex.yaml#L381))
- [ ] Record the colour component range of set_test_pattern (0-4095 in the 2023 example, gray 0-255 in OpenAPI) and how to return from a test pattern to the input. ([specs/novastar-coex.yaml:489](specs/novastar-coex.yaml#L489))
- [ ] Confirm identify_controller takes a JSON body (the OpenAPI page names application/xml). ([specs/novastar-coex.yaml:625](specs/novastar-coex.yaml#L625))
- [ ] Check whether set_working_mode to all-in-one (3) works through the API. ([specs/novastar-coex.yaml:700](specs/novastar-coex.yaml#L700))
- [ ] Confirm MX30 and MX20 answer the same API as the MX40 Pro. ([specs/novastar-coex.yaml:65](specs/novastar-coex.yaml#L65))

## novastar-h — NovaStar H series

- [ ] Confirm that unencrypted requests signed with Base64(md5(timeStamp + pId)) are accepted with encryption off for the requestor, and how far the time may drift before error 11. ([specs/novastar-h.yaml:768](specs/novastar-h.yaml#L768))
- [ ] Confirm the OpenAPI listens on port 8000 (the documentation's example address) on every model. ([specs/novastar-h.yaml:47](specs/novastar-h.yaml#L47))
- [ ] Check which deviceId to send (0 in most examples, 1 on the brightness pages). ([specs/novastar-h.yaml:793](specs/novastar-h.yaml#L793))
- [ ] Record whether the splicer pushes notifications (the change history mentions a websocket for layer z-order) and on which address. ([specs/novastar-h.yaml:801](specs/novastar-h.yaml#L801))
- [ ] Check set_screen_bkg's enable sense (documented 0 on, 1 off). ([specs/novastar-h.yaml:152](specs/novastar-h.yaml#L152))
- [ ] Check update_input_crop with the documented field spelling heigth. ([specs/novastar-h.yaml:280](specs/novastar-h.yaml#L280))

## obsidian-onyx — Obsidian ONYX (Telnet)

- [ ] Record ONYX's own Telnet server's replies (banner, success, error) for GQL, SQL and an unknown command, which Obsidian does not document. ([specs/obsidian-onyx.yaml:296](specs/obsidian-onyx.yaml#L296))
- [ ] Check which ONYX Manager queries ONYX's own server answers (QLList, QLActive, IsMxRun, WhoIAm) and its default port. ([specs/obsidian-onyx.yaml:305](specs/obsidian-onyx.yaml#L305))
- [ ] Check whether action commands (GQL, RQL) reply with "200 Ok" then ".", or something else, and whether 300 ms is enough to wait for a ".". ([specs/obsidian-onyx.yaml:299](specs/obsidian-onyx.yaml#L299))
- [ ] Check whether QLActive lines have the same form as QLList lines on ONYX Manager 4.x. ([specs/obsidian-onyx.yaml:304](specs/obsidian-onyx.yaml#L304))
- [ ] Check whether QLName answers on current versions. ([specs/obsidian-onyx.yaml:322](specs/obsidian-onyx.yaml#L322))

## obsidian-onyx-osc — Obsidian ONYX (OSC)

- [ ] Find ONYX's default OSC ports (the port ONYX listens on and the device's incoming port). ([specs/obsidian-onyx-osc.yaml:1027](specs/obsidian-onyx-osc.yaml#L1027))
- [ ] Confirm that PF GROUP 2 is /Mx/button/5702 (the mapping prints 5701 twice). ([specs/obsidian-onyx-osc.yaml:1057](specs/obsidian-onyx-osc.yaml#L1057))
- [ ] Record the OSC types ONYX sends for LED colour, blink and fader updates. ([specs/obsidian-onyx-osc.yaml:1066](specs/obsidian-onyx-osc.yaml#L1066))
- [ ] Check whether playback page actions need a release (0) after the key down. ([specs/obsidian-onyx-osc.yaml:1050](specs/obsidian-onyx-osc.yaml#L1050))
- [ ] Check what the belt execute address /Mx/belt/<id>/ does. ([specs/obsidian-onyx-osc.yaml:1060](specs/obsidian-onyx-osc.yaml#L1060))
- [ ] Check which commands work without a licence (FREE/NOVA modes and the playback licence note). ([specs/obsidian-onyx-osc.yaml:1034](specs/obsidian-onyx-osc.yaml#L1034))

## openlp — OpenLP

- [ ] Confirm that POST endpoints take their arguments as a JSON body with Content-Type application/json. ([specs/openlp.yaml:88](specs/openlp.yaml#L88))
- [ ] Confirm that the login token goes in the Authorization header with nothing before it, whether it stays valid across OpenLP restarts, and that a missing or wrong token answers 401. ([specs/openlp.yaml:49](specs/openlp.yaml#L49))
- [ ] Confirm that the websocket on 4317 sends its state in binary frames, sends the current state when a client connects, and needs no message from the client. ([specs/openlp.yaml:373](specs/openlp.yaml#L373))
- [ ] Confirm that the websocket's slide field is the selected slide's index from 0 (its documentation repeats the service field's words). ([specs/openlp.yaml:343](specs/openlp.yaml#L343))
- [ ] Check what the websocket's display, theme and blank fields hold for each display mode (desktop assumed for display). ([specs/openlp.yaml:383](specs/openlp.yaml#L383))
- [ ] Check how /service/show tells a position from an id, and whether positions count from 0 or 1. ([specs/openlp.yaml:313](specs/openlp.yaml#L313))
- [ ] Check which method /controller/clear/<controller> takes (POST assumed). ([specs/openlp.yaml:160](specs/openlp.yaml#L160))
- [ ] Check whether plugin item ids sent as JSON strings are accepted for songs, whose search results may give numbers. ([specs/openlp.yaml:253](specs/openlp.yaml#L253))
- [ ] Check what set_theme answers (200 with the name assumed from the documentation) and the bible search option names set_search_option takes. ([specs/openlp.yaml:151](specs/openlp.yaml#L151))

## panasonic-ptz — Panasonic PTZ

- [ ] Confirm the menu keys need DUP:1 (the AW-UE usage examples send DUP). ([specs/panasonic-ptz.yaml:4325](specs/panasonic-ptz.yaml#L4325))
- [ ] Confirm colour correction Yl_Yl_G uses OSD:1C/OSD:1D, not OSJ. ([specs/panasonic-ptz.yaml:4331](specs/panasonic-ptz.yaml#L4331))
- [ ] Confirm the AWB A/B numbering difference between control (1, 2) and query (2, 3), and get_scene 0-3 for scenes 1-4. ([specs/panasonic-ptz.yaml:4310](specs/panasonic-ptz.yaml#L4310))

## pjlink — PJLink

- [ ] With authentication auto, check how a projector that predates 2.10 answers "PJLINK 2", and whether it counts it as a failed login. ([specs/pjlink.yaml:213](specs/pjlink.yaml#L213))
- [ ] Check whether AVMT ? ever returns 10 or 20, and how they read. ([specs/pjlink.yaml:234](specs/pjlink.yaml#L234))
- [ ] Confirm the power-on response is "%1POWR=OK". ([specs/pjlink.yaml:240](specs/pjlink.yaml#L240))
- [ ] On a Class 2 projector, find how the controller address for status notifications is registered. ([specs/pjlink.yaml:254](specs/pjlink.yaml#L254))

## probel-swp08 — Probel / Grass Valley SW-P-08 routers

- [ ] Confirm the TCP port in use (2008 assumed; SW-P-08 over IP leaves it to the controller's configuration). ([specs/probel-swp08.yaml:442](specs/probel-swp08.yaml#L442))
- [ ] Confirm matrix and level 1 are wire 0 on each controller family, and 1 on System 2. ([specs/probel-swp08.yaml:391](specs/probel-swp08.yaml#L391))
- [ ] Check what a controller sends after a connect to a protected destination (nothing is assumed, so it times out). ([specs/probel-swp08.yaml:398](specs/probel-swp08.yaml#L398))
- [ ] Check that a controller acknowledges DUAL CONTROLLER STATUS REQUEST (08) with DLE ACK even when it does not implement it, since that ACK is the liveness check. ([specs/probel-swp08.yaml:414](specs/probel-swp08.yaml#L414))
- [ ] Check whether a controller NAKs commands it does not implement, answers INVALID MESSAGE (99), or stays silent. ([specs/probel-swp08.yaml:425](specs/probel-swp08.yaml#L425))
- [ ] Check which device numbers a remote client may protect with, and that PROTECT CONNECTED reports state 3 (OEM) with that device. ([specs/probel-swp08.yaml:455](specs/probel-swp08.yaml#L455))
- [ ] Confirm the protect tally dump request (19) layout: matrix/level byte then a two-byte first destination. ([specs/probel-swp08.yaml:503](specs/probel-swp08.yaml#L503))
- [ ] Confirm the tie-line connect (111) bytes 7 and 8 are the source association number. ([specs/probel-swp08.yaml:503](specs/probel-swp08.yaml#L503))
- [ ] Check whether CONNECTED messages follow a salvo go on XD and Eclipse routers (Issue 30 says none; go-acp reports controllers that expect them). ([specs/probel-swp08.yaml:476](specs/probel-swp08.yaml#L476))
- [ ] On dual controllers over IP, record the unsolicited DUAL CONTROLLER STATUS RESPONSE on a changeover. ([specs/probel-swp08.yaml:448](specs/probel-swp08.yaml#L448))
- [ ] Record name responses with name counts below 16, where a community parser reports the names starting one byte later. ([specs/probel-swp08.yaml:494](specs/probel-swp08.yaml#L494))

## propresenter — ProPresenter

- [ ] Confirm the network API port in use (50001 assumed). ([specs/propresenter.yaml:1534](specs/propresenter.yaml#L1534))

## ptzoptics — PTZOptics

- [ ] Check whether the camera asks for digest authentication (the document says all endpoints use it, the examples send none). ([specs/ptzoptics.yaml:1681](specs/ptzoptics.yaml#L1681))
- [ ] On G2 cameras, record the reply bodies, which the 2021 sheet does not document. ([specs/ptzoptics.yaml:1709](specs/ptzoptics.yaml#L1709))
- [ ] On G2 cameras, check what speed 0 does on zoom_in, zoom_out, focus_in and focus_out. ([specs/ptzoptics.yaml:1718](specs/ptzoptics.yaml#L1718))
- [ ] Check the value conflicts: backlight 1-3 or 2-3, focus_mode values, iris 0-8 or 0-12 on Move 4K and Link 4K, and tally_mode set as 2/3 but reported as 1. ([specs/ptzoptics.yaml:1752](specs/ptzoptics.yaml#L1752))
- [ ] Confirm video endpoints accept profile=highprofile (key=value) and Focus Limit works with GET. ([specs/ptzoptics.yaml:1764](specs/ptzoptics.yaml#L1764))
- [ ] Check the shutter index meanings per model. ([specs/ptzoptics.yaml:1772](specs/ptzoptics.yaml#L1772))
- [ ] On models other than Move 4K and Link 4K, check whether the query endpoints answer, and whether replies come as name="value" lines or JSON. ([specs/ptzoptics.yaml:1789](specs/ptzoptics.yaml#L1789))
- [ ] Check whether the camera decodes percent-encoded names, stream keys and SRT settings. ([specs/ptzoptics.yaml:1801](specs/ptzoptics.yaml#L1801))
- [ ] Record the values of the streaming, standby and privacy flags and the formats of the network and camera information replies, which are not documented. ([specs/ptzoptics.yaml:1397](specs/ptzoptics.yaml#L1397))

## qlab — QLab

- [ ] Check whether a cue that completes on its own produces a stop event, and how pause, panic, stop-all and reset-all events are addressed. ([specs/qlab.yaml:3954](specs/qlab.yaml#L3954))
- [ ] Confirm the /updates message with no argument when a cue list's playhead is unset. ([specs/qlab.yaml:3948](specs/qlab.yaml#L3948))

## qsys — Q-SYS

- [ ] Record how the Core answers a refused Logon (error object shape, with and without an id). ([specs/qsys.yaml:443](specs/qsys.yaml#L443))
- [ ] Check whether the Core's replies end with a NUL. ([specs/qsys.yaml:452](specs/qsys.yaml#L452))
- [ ] Record how each automatic poll after ChangeGroup.AutoPoll is framed (a result with the AutoPoll id or a ChangeGroup.Poll notification). ([specs/qsys.yaml:457](specs/qsys.yaml#L457))
- [ ] Confirm StatusGet with "params": 0 and Component.GetComponents with {} are accepted. ([specs/qsys.yaml:478](specs/qsys.yaml#L478))
- [ ] Check whether a Component.Get reply carries an id. ([specs/qsys.yaml:480](specs/qsys.yaml#L480))
- [ ] Find the unit of Mixer.SetCrossPointDelay's value. ([specs/qsys.yaml:491](specs/qsys.yaml#L491))
- [ ] Confirm LoopPlayer.Start accepts RefID and Seek at the top level. ([specs/qsys.yaml:496](specs/qsys.yaml#L496))
- [ ] On Designer emulation, check whether PA Router paging and Loop Player playback behave as on a Core. ([specs/qsys.yaml:540](specs/qsys.yaml#L540))

## qsys-ecp — Q-SYS (External Control Protocol)

- [ ] Check that the three-line subscription (cgc 1, cgsna 1 <ms>, sg sent in one write) is accepted, and that cgc and cgsna are silent on success. ([specs/qsys-ecp.yaml:378](specs/qsys-ecp.yaml#L378))
- [ ] Check login's answers (login_success, login_failed, and the socket closed after login_failed) and that a Core without Access Control answers login with an error rather than ignoring it. ([specs/qsys-ecp.yaml:77](specs/qsys-ecp.yaml#L77))
- [ ] Check that quoting every control name (cg "gain1") is accepted for names without spaces. ([specs/qsys-ecp.yaml:116](specs/qsys-ecp.yaml#L116))
- [ ] Check csvvr's form: the command reference shows it without the value count that csvv takes. ([specs/qsys-ecp.yaml:199](specs/qsys-ecp.yaml#L199))
- [ ] Check what ssl answers on success and on an unknown bank. ([specs/qsys-ecp.yaml:330](specs/qsys-ecp.yaml#L330))
- [ ] Check the cvv layout for a meter (count, strings, count, values, count, positions). ([specs/qsys-ecp.yaml:399](specs/qsys-ecp.yaml#L399))
- [ ] Check that an sg every 30 s keeps the connection open while a scheduled change group is pushing. ([specs/qsys-ecp.yaml:385](specs/qsys-ecp.yaml#L385))

## renewedvision-pvp — Renewed Vision PVP

- [ ] Confirm the API port in use (8080 assumed). ([specs/renewedvision-pvp.yaml:874](specs/renewedvision-pvp.yaml#L874))
- [ ] Confirm every successful command answers 200, not another 2xx status. ([specs/renewedvision-pvp.yaml:912](specs/renewedvision-pvp.yaml#L912))
- [ ] Check whether Layer Blending is at /blend/layer/{id} (document) or /layerBlend/layer/{id} (Companion). ([specs/renewedvision-pvp.yaml:919](specs/renewedvision-pvp.yaml#L919))
- [ ] Record the opacity GET reply shape and the Layer Blending GET reply shape. ([specs/renewedvision-pvp.yaml:928](specs/renewedvision-pvp.yaml#L928))
- [ ] Check which body the transition POST accepts ({"transition": {...}} or {"value": uuid}). ([specs/renewedvision-pvp.yaml:930](specs/renewedvision-pvp.yaml#L930))
- [ ] Confirm the opacity range is 0.0 to 1.0. ([specs/renewedvision-pvp.yaml:934](specs/renewedvision-pvp.yaml#L934))
- [ ] On the PVP version in use, check whether negative layer or cue indexes still crash it. ([specs/renewedvision-pvp.yaml:903](specs/renewedvision-pvp.yaml#L903))

## resolume — Resolume Arena / Avenue

- [ ] Record the shape of the composition message pushed on the websocket (bare object or wrapped as {"type", "value"}). ([specs/resolume.yaml:964](specs/resolume.yaml#L964))
- [ ] Confirm that layer, column, deck and group positions reported in state match the order in the composition list Resolume sends. ([specs/resolume.yaml:875](specs/resolume.yaml#L875))
- [ ] Confirm that setters sending only the changed property leave other properties unchanged. ([specs/resolume.yaml:911](specs/resolume.yaml#L911))
- [ ] Check what Resolume does with a speed, crossfader phase or tempo value outside the parameter's min and max. ([specs/resolume.yaml:921](specs/resolume.yaml#L921))
- [ ] Confirm opacity and master levels are 0.0 to 1.0 over REST. ([specs/resolume.yaml:918](specs/resolume.yaml#L918))
- [ ] Confirm a deck switch, deck open and deck close bring no new composition on the websocket (so the REST read is needed). ([specs/resolume.yaml:954](specs/resolume.yaml#L954))
- [ ] On Avenue, check whether get_deck, replace_deck and deck delete and duplicate answer 402. ([specs/resolume.yaml:996](specs/resolume.yaml#L996))

## roland-p20hd — Roland P-20HD

- [ ] Confirm the login exchange: USR sent on connecting with no prompt, ACK after USR and after PSS, NAK on refusal. ([specs/roland-p20hd.yaml:572](specs/roland-p20hd.yaml#L572))
- [ ] Confirm the session closes after five minutes without commands and that ACS every ten seconds of silence keeps it open. ([specs/roland-p20hd.yaml:583](specs/roland-p20hd.yaml#L583))

## roland-v160hd — Roland V-160HD, V-80HD, VR-120HD, VR-6HD, V-1-4K

- [ ] Confirm commands ended with CR LF are accepted (Roland documents only Telnet). ([specs/roland-v160hd.yaml:84](specs/roland-v160hd.yaml#L84))
- [ ] Confirm the password prompt and reply texts ("Enter password:", "Welcome to ...", "Authentication error", "Wait a moment"), which are community-sourced. ([specs/roland-v160hd.yaml:2811](specs/roland-v160hd.yaml#L2811))
- [ ] Check whether a query's value frame and its ACK; arrive on one line or two. ([specs/roland-v160hd.yaml:2827](specs/roland-v160hd.yaml#L2827))
- [ ] Check whether GRCH, AMCH, SPCH, AUXCH, QSRTIST and QBSY are followed by ACK;. ([specs/roland-v160hd.yaml:2829](specs/roland-v160hd.yaml#L2829))
- [ ] Confirm a poll of five queries a second does not lock the panel. ([specs/roland-v160hd.yaml:2850](specs/roland-v160hd.yaml#L2850))
- [ ] Confirm the wire spellings chosen: test tone levels OFF, -20, -10, 0 and frequencies 500, 1k, 2k, and values without the space before the comma. ([specs/roland-v160hd.yaml:2874](specs/roland-v160hd.yaml#L2874))
- [ ] Confirm the AUXLV and AUXCH replies are QAUXLV and QAUXCH, and QTIM names DSK on V-1-4K. ([specs/roland-v160hd.yaml:2880](specs/roland-v160hd.yaml#L2880))
- [ ] Confirm the minimum firmware version per model for the Basic commands. ([specs/roland-v160hd.yaml:2886](specs/roland-v160hd.yaml#L2886))
- [ ] Check whether a second control session receives tally frames after TALLY AUTO SEND. ([specs/roland-v160hd.yaml:2900](specs/roland-v160hd.yaml#L2900))
- [ ] Check whether RQH sizes other than 000001 are answered. ([specs/roland-v160hd.yaml:2898](specs/roland-v160hd.yaml#L2898))

## roland-v600uhd — Roland V-600UHD

- [ ] Confirm the password prompt "Enter password:" and greeting "Welcome to Roland V-600UHD.", and record what a wrong password produces. ([specs/roland-v600uhd.yaml:260](specs/roland-v600uhd.yaml#L260))
- [ ] Find which input audio numbers lie between HDMI IN1 (0) and TEST TONE (7). ([specs/roland-v600uhd.yaml:280](specs/roland-v600uhd.yaml#L280))

## roland-v60hd — Roland V-60HD

- [ ] Confirm the QAL numbering (which of 11, 12 and 13 is MASTER OUT, AUX and all). ([specs/roland-v60hd.yaml:463](specs/roland-v60hd.yaml#L463))
- [ ] Check whether the QPL frame for PANEL INFORMATION is also sent over LAN. ([specs/roland-v60hd.yaml:476](specs/roland-v60hd.yaml#L476))

## roland-vr400uhd — Roland VR-400UHD

- [ ] Confirm the password prompt (a line containing "Enter") and greeting, and record what a wrong password produces. ([specs/roland-vr400uhd.yaml:443](specs/roland-vr400uhd.yaml#L443))
- [ ] Check whether get is answered with an ack line as well as the set line. ([specs/roland-vr400uhd.yaml:454](specs/roland-vr400uhd.yaml#L454))
- [ ] Check the dB scale of levels 0-127. ([specs/roland-vr400uhd.yaml:465](specs/roland-vr400uhd.yaml#L465))

## roland-xs42h — Roland XS-42H / VP-42H

- [ ] Check whether the unit prompts before taking the password, and record the login exchange and what a wrong password produces. ([specs/roland-xs42h.yaml:341](specs/roland-xs42h.yaml#L341))
- [ ] Check the dB scale of levels 0-127. ([specs/roland-xs42h.yaml:361](specs/roland-xs42h.yaml#L361))

## roland-xs62s — Roland XS-62S

- [ ] Confirm the forms the manual leaves open: VOS with only the bus, IL2 with both a and b, the two TRS values, and VER's empty model field. ([specs/roland-xs62s.yaml:605](specs/roland-xs62s.yaml#L605))

## roland-xs80h — Roland XS-82H/83H/84H

- [ ] Check whether any line ending follows ";" over LAN. ([specs/roland-xs80h.yaml:543](specs/roland-xs80h.yaml#L543))
- [ ] Check whether ITS, OTS, CTS, KLS and VER are followed by ACK;. ([specs/roland-xs80h.yaml:544](specs/roland-xs80h.yaml#L544))
- [ ] Check whether a setting enables LAN control and whether a password applies. ([specs/roland-xs80h.yaml:558](specs/roland-xs80h.yaml#L558))

## ross-xpression — Ross XPression (TCP)

- [ ] Confirm take IDs are read padded or unpadded, including UNCUE and UPNEXT in the padded form. ([specs/ross-xpression.yaml:348](specs/ross-xpression.yaml#L348))
- [ ] Check the take ID, layer, framebuffer, GPI and channel ranges. ([specs/ross-xpression.yaml:354](specs/ross-xpression.yaml#L354))
- [ ] Check the syntax for RESUME with a tessera source name. ([specs/ross-xpression.yaml:399](specs/ross-xpression.yaml#L399))

## ross-xpression-udp — Ross XPression (UDP)

- [ ] Confirm the UDP port configured on the board (7788 assumed; Ross states no default). ([specs/ross-xpression-udp.yaml:310](specs/ross-xpression-udp.yaml#L310))
- [ ] Confirm one command per datagram, ended with CR LF, is accepted. ([specs/ross-xpression-udp.yaml:319](specs/ross-xpression-udp.yaml#L319))
- [ ] Confirm take IDs are read padded or unpadded, including UNCUE and UPNEXT in the padded form. ([specs/ross-xpression-udp.yaml:372](specs/ross-xpression-udp.yaml#L372))
- [ ] Check the take ID, layer, framebuffer, GPI and channel ranges. ([specs/ross-xpression-udp.yaml:378](specs/ross-xpression-udp.yaml#L378))
- [ ] Check the syntax for RESUME with a tessera source name. ([specs/ross-xpression-udp.yaml:423](specs/ross-xpression-udp.yaml#L423))

## rosstalk — RossTalk (switchers)

- [ ] Check how memory numbers are written for banks of two digits. ([specs/rosstalk.yaml:1102](specs/rosstalk.yaml#L1102))
- [ ] Confirm CC and GPI are accepted with a colon and two-digit number as in the examples. ([specs/rosstalk.yaml:1085](specs/rosstalk.yaml#L1085))
- [ ] Confirm MVBOX's I/O MultiViewer token (IO or OP) and that NTP SET (not NET SET) is accepted. ([specs/rosstalk.yaml:1161](specs/rosstalk.yaml#L1161))
- [ ] Confirm Media-Store IDs zero-padded to three digits on Carbonite and unpadded on Acuity. ([specs/rosstalk.yaml:1151](specs/rosstalk.yaml#L1151))
- [ ] On Acuity, Ultrix Acuity and Vision, check what the RTalk-IN "Cmd Response" option does. ([specs/rosstalk.yaml:1182](specs/rosstalk.yaml#L1182))
- [ ] On Vision, check whether command words such as MECUT and MEAUTO must use MLE instead of ME. ([specs/rosstalk.yaml:1202](specs/rosstalk.yaml#L1202))
- [ ] On Acuity, record the reply CAPTURE sends when the capture completes. ([specs/rosstalk.yaml:1221](specs/rosstalk.yaml#L1221))

## sennheiser-digital-6000 — Sennheiser Digital 6000

- [ ] Confirm the SSC port is 45 (the Companion module defaults to 6970). ([specs/sennheiser-digital-6000.yaml:110](specs/sennheiser-digital-6000.yaml#L110))
- [ ] Confirm that subscriptions lapse after 20 s and that renewal every 6.7 s keeps telemetry flowing. ([specs/sennheiser-digital-6000.yaml:102](specs/sennheiser-digital-6000.yaml#L102))
- [ ] Confirm out-of-range set_frequency and set_af_out values are adapted and replied with the value applied. ([specs/sennheiser-digital-6000.yaml:116](specs/sennheiser-digital-6000.yaml#L116))
- [ ] Opened for commands only, confirm the receiver answers an unsubscribed `device.name` read and echoes its `osc.xid`, the liveness check. ([crates/core/src/modules/sennheiser_d6000.rs:129](crates/core/src/modules/sennheiser_d6000.rs#L129))

## sennheiser-ew-dx — Sennheiser EW-DX

- [ ] Check whether gain, frequency, identify and network settings have SSCv2 paths on the firmware in use. ([specs/sennheiser-ew-dx.yaml:96](specs/sennheiser-ew-dx.yaml#L96))
- [ ] On an EM 4 Dante, confirm subscriptions and mute work on channels 3 and 4 (only channels 1 and 2 have been tested on hardware). ([specs/sennheiser-ew-dx.yaml:118](specs/sennheiser-ew-dx.yaml#L118))
- [ ] Confirm a 3 s silence check against /api/ssc/version does not lose a healthy connection. ([specs/sennheiser-ew-dx.yaml:104](specs/sennheiser-ew-dx.yaml#L104))

## sennheiser-ew-g3-g4 — Sennheiser ew G3 / G4

- [ ] Check which bit of States is TX mute against a real receiver, using mute_flags, since the document's examples disagree with its table. ([specs/sennheiser-ew-g3-g4.yaml:187](specs/sennheiser-ew-g3-g4.yaml#L187))
- [ ] On a G3 receiver, confirm squelch, AF out, equalizer, RfConfig and FirmwareRevision behave as on G4 (no G3 document has been found). ([specs/sennheiser-ew-g3-g4.yaml:205](specs/sennheiser-ew-g3-g4.yaml#L205))
- [ ] Opened for commands only, confirm a receiver with no Push subscription answers a bare `Name`, the liveness check. ([crates/core/src/modules/sennheiser_mcp.rs:31](crates/core/src/modules/sennheiser_mcp.rs#L31))

## shure-ani — Shure ANI4IN, ANI4OUT, ANIUSB-MATRIX, ANI22

- [ ] Check that the ANI4IN and ANI4OUT accept two-digit channel numbers (01), which the session sends, though their documents write one digit. ([specs/shure-ani.yaml:443](specs/shure-ani.yaml#L443))
- [ ] Check that < REP ERR > is the answer to an invalid command and nothing else. ([specs/shure-ani.yaml:60](specs/shure-ani.yaml#L60))
- [ ] Check that GET 00 ALL is accepted by every ANI (the ANIUSB-MATRIX document limits GET ALL to V1 units) and that the answers end. ([specs/shure-ani.yaml:766](specs/shure-ani.yaml#L766))
- [ ] Check that the analog gain is sent and reported as two digits (00-51) and in 3 dB steps. ([specs/shure-ani.yaml:466](specs/shure-ani.yaml#L466))
- [ ] Confirm that CHAN_LED_IN_STATE is not answered on the ANI4IN and whether it is on the ANI22. ([specs/shure-ani.yaml:544](specs/shure-ani.yaml#L544))
- [ ] Check LED_BRIGHTNESS's range on current firmware (0-2 documented). ([specs/shure-ani.yaml:577](specs/shure-ani.yaml#L577))

## shure-imx-room — Shure IntelliMix Room

- [ ] Check that GET 00 ALL is answered by IntelliMix Room and that its answers end. ([specs/shure-imx-room.yaml:398](specs/shure-imx-room.yaml#L398))
- [ ] Check that the gain step keywords are accepted in lower case (inc, dec), as the document writes them. ([specs/shure-imx-room.yaml:185](specs/shure-imx-room.yaml#L185))
- [ ] Check how licence values are padded (LIC_TYPE is documented as 10 characters). ([specs/shure-imx-room.yaml:260](specs/shure-imx-room.yaml#L260))
- [ ] Check the matrix gain REP's spacing (the document shows the gain run into the output number in one example). ([specs/shure-imx-room.yaml:343](specs/shure-imx-room.yaml#L343))

## shure-mxa — Shure Microflex Advance arrays

- [ ] Check whether the MXA910 and MXA310 accept lower-case inc and dec (their documents write INC and DEC). ([specs/shure-mxa.yaml:1077](specs/shure-mxa.yaml#L1077))
- [ ] Check that GET 00 ALL is accepted on every model and firmware (some documents write GET 0 ALL or GET ALL). ([specs/shure-mxa.yaml:1834](specs/shure-mxa.yaml#L1834))
- [ ] Check PRESET_NAME's answer form (< REP PRESET_NAME nn name >) on the MXA910 and MXA310, whose documents use PRESET1-PRESET10 instead. ([specs/shure-mxa.yaml:972](specs/shure-mxa.yaml#L972))
- [ ] Check the speech gating and noise filter values each model accepts (Off/Low/Medium/High on the MXA920, ON/OFF on the MXA902 and MXA901) and their case in REP. ([specs/shure-mxa.yaml:1232](specs/shure-mxa.yaml#L1232))
- [ ] Check LED_BRIGHTNESS's range per model and firmware (0-2 or 0-5). ([specs/shure-mxa.yaml:1337](specs/shure-mxa.yaml#L1337))
- [ ] Check that lobe X and Y are accepted as four digits (0000-3048) on SET. ([specs/shure-mxa.yaml:1472](specs/shure-mxa.yaml#L1472))
- [ ] Check the MXA310 polar pattern names (SUPER, HYPER, BIDIRECTION as documented). ([specs/shure-mxa.yaml:1731](specs/shure-mxa.yaml#L1731))

## shure-mxn5 — Shure MXN5-C

- [ ] Check what an MXN5-C that is not V1 answers to GET 00 ALL. ([specs/shure-mxn5.yaml:459](specs/shure-mxn5.yaml#L459))
- [ ] Check that the gain step keywords are accepted in lower case. ([specs/shure-mxn5.yaml:267](specs/shure-mxn5.yaml#L267))
- [ ] Check that SIG_GEN on channel 03 starts and stops the generator and what the 00 index does. ([specs/shure-mxn5.yaml:403](specs/shure-mxn5.yaml#L403))
- [ ] Check the PRESET_NAME answer for an empty preset ({empty}). ([specs/shure-mxn5.yaml:162](specs/shure-mxn5.yaml#L162))

## shure-p300 — Shure P300

- [ ] Check that a SET answered by the REP of the new value is never interleaved with a change report for the same parameter on another channel, or note how often it happens. ([specs/shure-p300.yaml:40](specs/shure-p300.yaml#L40))
- [ ] Check that recall_preset accepts the two-digit form (SET PRESET 03). ([specs/shure-p300.yaml:261](specs/shure-p300.yaml#L261))
- [ ] Check that INC and DEC steps are in tenths of a dB as documented, and how a step beyond the range is answered. ([specs/shure-p300.yaml:366](specs/shure-p300.yaml#L366))
- [ ] Check the AEC reference values on firmware 4.1 and later (Dante outputs 3-8 added). ([specs/shure-p300.yaml:554](specs/shure-p300.yaml#L554))
- [ ] Check what GATE_INHIBIT answers on firmware 4.1 and later, where it is documented not to work. ([specs/shure-p300.yaml:681](specs/shure-p300.yaml#L681))
- [ ] Check the matrix gain REP's spacing (the document shows < REP xx MATRIX_MXR_GAIN yyzzzz > once). ([specs/shure-p300.yaml:991](specs/shure-p300.yaml#L991))

## shure-wireless — Shure wireless receivers

- [ ] On AD4Q with frequency diversity combined (FD-C), check the second RF section in SAMPLE and that the first is the right one to report. ([specs/shure-wireless.yaml:199](specs/shure-wireless.yaml#L199))
- [ ] Confirm the PSM1000 answers REPORT with string values without braces. ([specs/shure-wireless.yaml:223](specs/shure-wireless.yaml#L223))
- [ ] Opened for commands only, confirm a receiver that is not metering answers `GET DEVICE_ID` (`GET DEVICE_NAME` on the PSM1000) promptly enough for the 5 s liveness check. ([crates/core/src/modules/shure.rs:48](crates/core/src/modules/shure.rs#L48))

## softouch-easyworship — Softouch EasyWorship

- [ ] Confirm the whole exchange against EasyWorship 7.3 or later: the pairing request and its paired and notPaired answers, a heartbeat after each message, and status messages. ([specs/softouch-easyworship.yaml:130](specs/softouch-easyworship.yaml#L130))
- [ ] Check whether EasyWorship sends paired on the same connection once the pairing request is approved, or only on the next request (sent again every 15 seconds). ([crates/core/src/modules/easyworship.rs:39](crates/core/src/modules/easyworship.rs#L39))
- [ ] Check whether the pairing request is accepted with any uid text (meros-<name> by default), and whether repeating it while EasyWorship is asking shows a second request. ([specs/softouch-easyworship.yaml:39](specs/softouch-easyworship.yaml#L39))
- [ ] Record whether the advertised port stays the same across EasyWorship restarts. ([specs/softouch-easyworship.yaml:44](specs/softouch-easyworship.yaml#L44))
- [ ] Check what a status sent with only the logo, black and clear flags (before EasyWorship's first status) does. ([specs/softouch-easyworship.yaml:101](specs/softouch-easyworship.yaml#L101))
- [ ] Check whether gotoSlide and gotoSchedule count from 1, and what pres_no, slide_no and rectype hold. ([specs/softouch-easyworship.yaml:134](specs/softouch-easyworship.yaml#L134))
- [ ] Check whether UTF-8 is accepted where the Companion module sends Latin-1 (only the controller name can hold non-ASCII text, and it is limited to ASCII here). ([specs/softouch-easyworship.yaml:62](specs/softouch-easyworship.yaml#L62))

## sony-camera — Sony cameras

- [ ] Check user base look numbering on each body: whether properties.D0C7 lists the plain number 1-16 or the user-LUT form (0x0100 plus the number), and that select, delete and PPLUT commands hit the intended look. ([specs/sony-camera.yaml:2515](specs/sony-camera.yaml#L2515))
- [ ] Check zoom and focus preset slot numbering: that slot 0 is the first preset, matching save_zoom_focus_position, before relying on set_preset_zoom_only. ([specs/sony-camera.yaml:2509](specs/sony-camera.yaml#L2509))
- [ ] On ILME-FR7 and BRC-AM7, check whether pan/tilt preset numbers for preset_recall, preset_set and preset_clear start at 0 or 1 within pan_tilt.preset_slots. ([specs/sony-camera.yaml:2365](specs/sony-camera.yaml#L2365))
- [ ] Check the display-list mapping: that base look, AE level offset, scene file, creative look and stream destination names come from the list types the module uses and match what the camera shows. ([crates/core/src/modules/sony_camera_info.rs:92](crates/core/src/modules/sony_camera_info.rs#L92))
- [ ] Check shutter speed encoding: that 1/x values and long exposures (sent as tenths of a second, such as 2.5") round-trip through the 16/16 and 32/32 numerator-over-denominator properties and match the camera's listed values. ([crates/core/src/modules/sony_camera_props.rs:28](crates/core/src/modules/sony_camera_props.rs#L28))
- [ ] On a Camera Control PTP 2 body (ILCE-7M3), check which way a positive exposure_step moves for iris, shutter speed, ISO and exposure and flash compensation. ([specs/sony-camera.yaml:2411](specs/sony-camera.yaml#L2411))
- [ ] On ILME-FR7 and BRC-AM7, confirm pan is positive anticlockwise from above and tilt positive upward, both desk- and ceiling-mounted. ([specs/sony-camera.yaml:2360](specs/sony-camera.yaml#L2360))
- [ ] On ILCE-7CM2, ILCE-7CR and ILX-LR1 firmware 1.00, check focus indication from AF status events and whether focal distance reads sensibly. ([specs/sony-camera.yaml:2371](specs/sony-camera.yaml#L2371))
- [ ] Opened for commands only, confirm the camera tolerates GetDeviceInfo once a second as the liveness check. ([crates/core/src/modules/sony_camera.rs:48](crates/core/src/modules/sony_camera.rs#L48))

## symetrix-composer — Symetrix Composer DSPs

- [ ] Check that pushed GSYSS strings (with "Enable System String Pushing") do not start with "#" lines that look like controller pushes, and what they look like; they are currently ignored, or taken as an answer. ([specs/symetrix-composer.yaml:44](specs/symetrix-composer.yaml#L44))
- [ ] Check that EH 0 and SQ 1 are answered ACK on a unit already in quiet mode with echo off, and what they answer when echo was on. ([specs/symetrix-composer.yaml:51](specs/symetrix-composer.yaml#L51))
- [ ] Check whether a GS2 answer pads the controller number, and whether GS answers are padded. ([specs/symetrix-composer.yaml:138](specs/symetrix-composer.yaml#L138))
- [ ] Check what a GSB2 block read answers for a block within range (no ACK is expected after the lines). ([specs/symetrix-composer.yaml:138](specs/symetrix-composer.yaml#L138))
- [ ] Check that the fader conversion (-72 to +12 dB over 0-65535) matches Composer's display on a typical volume fader. ([specs/symetrix-composer.yaml:145](specs/symetrix-composer.yaml#L145))
- [ ] Check what GSYSS answers for an empty string, and that NAK is its failure answer. ([specs/symetrix-composer.yaml:225](specs/symetrix-composer.yaml#L225))
- [ ] Check whether CMV Toggle needs the trailing value the document's example carries ("CMV Toggle 0.1.OMute.O2 1"), and whether the documented feature names (CPGain, IGain, OGain) or the examples' (InGain, OutGain, CPVol) are the ones accepted. ([specs/symetrix-composer.yaml:393](specs/symetrix-composer.yaml#L393))
- [ ] Check that PU 1 and PUR on connecting send every push-enabled value to this TCP session. ([specs/symetrix-composer.yaml:417](specs/symetrix-composer.yaml#L417))
- [ ] Confirm the model list: which current products (Radius NX, Prism, Edge, Solus NX) speak this protocol revision unchanged. ([specs/symetrix-composer.yaml:475](specs/symetrix-composer.yaml#L475))

## symetrix-jupiter — Symetrix Jupiter

- [ ] Check that Jupiter accepts GPR D as the liveness probe at any time and answers PrstD=nnnn. ([specs/symetrix-jupiter.yaml:132](specs/symetrix-jupiter.yaml#L132))
- [ ] Check that PU 1, PUE and PUR on connecting make every controller's value arrive at this session's UDP port. ([specs/symetrix-jupiter.yaml:212](specs/symetrix-jupiter.yaml#L212))
- [ ] Check which buttons of each Jupiter app use negative logic. ([specs/symetrix-jupiter.yaml:113](specs/symetrix-jupiter.yaml#L113))
- [ ] Check that answers never start with "#", which is how pushes are told apart. ([specs/symetrix-jupiter.yaml:38](specs/symetrix-jupiter.yaml#L38))

## tsl-umd-listener — TSL UMD tally received from a switcher

- [ ] On a Ross Carbonite sending TSLUMD_1.0 over TCP, confirm the core receives each 18-byte V3.1 packet back to back with nothing between them, and record the tally and text for program, preview and a key. ([specs/tsl-umd-listener.yaml:46](specs/tsl-umd-listener.yaml#L46))
- [ ] On a Carbonite, confirm tally 1 is preview and tally 2 program on the TSL feed, and what ShowUMDId and ShowBusName add to the text. ([specs/tsl-umd-listener.yaml:94](specs/tsl-umd-listener.yaml#L94))

## tvone-coriomaster — tvONE CORIOmaster

- [ ] Record how a failed command answers (an "!Error" line is assumed) and what a wrong login answers. ([specs/tvone-coriomaster.yaml:689](specs/tvone-coriomaster.yaml#L689))
- [ ] Confirm that the unit greets a connection with a caret prompt that needs no reply, and that login() before any other command is enough. ([specs/tvone-coriomaster.yaml:55](specs/tvone-coriomaster.yaml#L55))
- [ ] Confirm that every command, including methods and StartBatch/EndBatch, ends with exactly one "!Done" line after its value lines. ([specs/tvone-coriomaster.yaml:46](specs/tvone-coriomaster.yaml#L46))
- [ ] Record the exact form of the WINDOW, PRESET, STBD and CANVAS event lines (spaces after the commas vary in the reference). ([specs/tvone-coriomaster.yaml:567](specs/tvone-coriomaster.yaml#L567))
- [ ] Check what happens when a second client (CORIOgrapher) connects while this one is connected. ([specs/tvone-coriomaster.yaml:681](specs/tvone-coriomaster.yaml#L681))
- [ ] Confirm that On and Off are accepted for HFlip, VFlip, SCFTB and the shrink animations, and whether Yes and No are also accepted. ([specs/tvone-coriomaster.yaml:199](specs/tvone-coriomaster.yaml#L199))

## visca — VISCA over IP

- [ ] On Sony VISCA over IP, confirm the sequence number the camera expects after RESET (the core sends 1). ([specs/visca.yaml:912](specs/visca.yaml#L912))
- [ ] On PTZOptics, check whether UDP 1259 expects Sony's 8-byte header. ([specs/visca.yaml:959](specs/visca.yaml#L959))
- [ ] On PTZOptics, confirm the shutter, gain, bright, focus mode and focus position codes the core uses (0A/4A, 4C, 4D, 04 38, 04 48) against the 2026 list's conflicting codes. ([specs/visca.yaml:966](specs/visca.yaml#L966))
- [ ] On SRG-X400, check which pan direction is positive. ([specs/visca.yaml:953](specs/visca.yaml#L953))
- [ ] On BirdDog, confirm 32-bit sequence numbers are accepted, and the P400 white balance codes (04 ATW, 05/06 manual). ([specs/visca.yaml:979](specs/visca.yaml#L979))
- [ ] On Lumens and Marshall, confirm preset 1 is wire 00. ([specs/visca.yaml:985](specs/visca.yaml#L985))
- [ ] On AVer, confirm presets are numbered from 0 on the wire (preset n is wire n-1). ([specs/visca.yaml:992](specs/visca.yaml#L992))
- [ ] On Canon, confirm replies go to UDP 52381 with the default response port setting. ([specs/visca.yaml:1004](specs/visca.yaml#L1004))
- [ ] On Datavideo, find the IP framing and port, which the document does not give (52381 with Sony framing assumed). ([specs/visca.yaml:1012](specs/visca.yaml#L1012))

## yamaha-cl-ql — Yamaha CL / QL

- [ ] Confirm the command set beyond scene recall and input fader, ON and pan (get and its reply, OK/OKm acknowledgements, ERROR codes, NOTIFY set pushes, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-cl-ql.yaml:409](specs/yamaha-cl-ql.yaml#L409))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-cl-ql.yaml:451](specs/yamaha-cl-ql.yaml#L451))
- [ ] Confirm the sscurrent_ex reply form "OK sscurrent_ex MIXER:Lib/Scene <number> [modified|unmodified]", taken by analogy with the DME7. ([specs/yamaha-cl-ql.yaml:485](specs/yamaha-cl-ql.yaml#L485))
- [ ] Confirm an out-of-range set is clamped and answered OKm. ([specs/yamaha-cl-ql.yaml:433](specs/yamaha-cl-ql.yaml#L433))

## yamaha-dm3 — Yamaha DM3

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-dm3.yaml:4432](specs/yamaha-dm3.yaml#L4432))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-dm3.yaml:4474](specs/yamaha-dm3.yaml#L4474))
- [ ] Record the sscurrent_ex reply form, which is matched leniently, and whether the query for the inactive scene list is answered with an error. ([specs/yamaha-dm3.yaml:4505](specs/yamaha-dm3.yaml#L4505))
- [ ] Check how stereo input and FX return sends to mix, FX and matrix choose their destination (no Y dimension documented). ([specs/yamaha-dm3.yaml:4517](specs/yamaha-dm3.yaml#L4517))
- [ ] Check what send pre/post 0 and 1 mean. ([specs/yamaha-dm3.yaml:4520](specs/yamaha-dm3.yaml#L4520))
- [ ] Confirm local head-amp gain is whole dB 0 to 64. ([specs/yamaha-dm3.yaml:4521](specs/yamaha-dm3.yaml#L4521))

## yamaha-dm7 — Yamaha DM7

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-dm7.yaml:7575](specs/yamaha-dm7.yaml#L7575))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-dm7.yaml:7618](specs/yamaha-dm7.yaml#L7618))
- [ ] Record the sscurrentt_ex reply form, which is matched leniently, and check whether ssrecallt_ex and sscurrentt_ex need scpmode sstype "text" first (the Companion module sends it). ([specs/yamaha-dm7.yaml:7650](specs/yamaha-dm7.yaml#L7650))
- [ ] Check what send pre/post 0 and 1 mean (RIVAGE gives 0 Post, 1 Pre). ([specs/yamaha-dm7.yaml:7662](specs/yamaha-dm7.yaml#L7662))
- [ ] Check the PEQ band gain scaling (10 or 100). ([specs/yamaha-dm7.yaml:7664](specs/yamaha-dm7.yaml#L7664))

## yamaha-dme7 — Yamaha DME7

- [ ] Check whether Remote Control Setup List indexes start at 0 or 1. ([specs/yamaha-dme7.yaml:877](specs/yamaha-dme7.yaml#L877))
- [ ] Record which reply forms the device actually sends where the document shows two (setr, sscurrent_ex, ssnum_ex, ssinfo_ex, mtrinfo, devinfo port quoting, devstatus Power1/power1). ([specs/yamaha-dme7.yaml:947](specs/yamaha-dme7.yaml#L947))
- [ ] Check whether AudioPlayerGetStatus and listitemnum need the trailing "" from the syntax tables. ([specs/yamaha-dme7.yaml:957](specs/yamaha-dme7.yaml#L957))
- [ ] Check the scheduler time format beyond "9:00". ([specs/yamaha-dme7.yaml:962](specs/yamaha-dme7.yaml#L962))

## yamaha-rivage — Yamaha RIVAGE PM

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-rivage.yaml:9668](specs/yamaha-rivage.yaml#L9668))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-rivage.yaml:9710](specs/yamaha-rivage.yaml#L9710))
- [ ] Find which address answers RCP on TCP 49280: the DSP engine, the console's Network PC port, or both. ([specs/yamaha-rivage.yaml:9736](specs/yamaha-rivage.yaml#L9736))
- [ ] Confirm get_current_scene (sscurrentt_ex) and its reply form, taken from the Companion module, and whether scpmode sstype "text" is needed first. ([specs/yamaha-rivage.yaml:9743](specs/yamaha-rivage.yaml#L9743))
- [ ] Check the Dyna1/Dyna2 Attack and Ratio scaling for COMP260 and other types, and the -720 threshold for gate and ducking. ([specs/yamaha-rivage.yaml:9755](specs/yamaha-rivage.yaml#L9755))
- [ ] Confirm the surround monitor channel switches are 0 ON, 1 MUTE. ([specs/yamaha-rivage.yaml:9762](specs/yamaha-rivage.yaml#L9762))

## yamaha-rm — Yamaha RM series

- [ ] Record what a rejected command returns, since the RM document does not describe error replies. ([specs/yamaha-rm.yaml:5795](specs/yamaha-rm.yaml#L5795))
- [ ] Confirm replies arrive in order, since get and set replies are matched on AccessID only and not on the sub-address. ([specs/yamaha-rm.yaml:5798](specs/yamaha-rm.yaml#L5798))
- [ ] On RM-CR, RM-CG and RM-TT, confirm sett, setr and gett work as the parameter tables list them. ([specs/yamaha-rm.yaml:5804](specs/yamaha-rm.yaml#L5804))
- [ ] On RM-CG and RM-TT, check the sense of NeOut_Mute/Ch/On (tabled as 0 ON, 1 OFF). ([specs/yamaha-rm.yaml:5836](specs/yamaha-rm.yaml#L5836))
- [ ] Check which models answer the SIP, Bluetooth, DECT pairing and accessory identify events, which were assigned by function. ([specs/yamaha-rm.yaml:5849](specs/yamaha-rm.yaml#L5849))
- [ ] On RM-CR, record the NOTIFY ssupdate_ex form and check Dante input patch values 17-21. ([specs/yamaha-rm.yaml:5857](specs/yamaha-rm.yaml#L5857))
- [ ] Confirm frequencies use the parameter tables' 0.1 Hz units (200 = 20 Hz). ([specs/yamaha-rm.yaml:5868](specs/yamaha-rm.yaml#L5868))
- [ ] Record the reply forms of the GetTimeZone, DST and NTP events, taken to repeat the set form, and the identify range. ([specs/yamaha-rm.yaml:5875](specs/yamaha-rm.yaml#L5875))

## yamaha-tf — Yamaha TF

- [ ] Confirm the command set beyond scene recall and input fader, ON and pan (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-tf.yaml:388](specs/yamaha-tf.yaml#L388))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-tf.yaml:430](specs/yamaha-tf.yaml#L430))
- [ ] Confirm get_current_scene (sscurrent_ex scene_a|scene_b) and its reply, and that the inactive list's query is answered with an error. ([specs/yamaha-tf.yaml:461](specs/yamaha-tf.yaml#L461))
- [ ] Confirm the address uses "InCh" (the QLab guide prints "Inch"). ([specs/yamaha-tf.yaml:476](specs/yamaha-tf.yaml#L476))

## Integrations with no open items

Nothing specific is listed for these yet, but test results are still welcome: generic-http, generic-osc, generic-tcp-udp, http-snapshot, obs-studio, tsl-umd-display, vmix.
