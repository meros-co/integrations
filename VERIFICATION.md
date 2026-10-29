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

## 7thsense-delta — 7thSense Delta Media Server

- [ ] Confirm names in double quotes are taken where the reference writes 'name' in single quotes (LOAD, GOTOMARKER, SEQUENCE). ([specs/7thsense-delta.yaml:637](specs/7thsense-delta.yaml#L637))
- [ ] Confirm the OK or BAD line comes after a command's data lines (STATUS, LISTSHOWS, GLOBALVARGET), and how GLOBALVARGET and SEQUENCESTATUS answer. ([specs/7thsense-delta.yaml:618](specs/7thsense-delta.yaml#L618))
- [ ] Check the play and boolean words GOTOFRAME/GOTOMARKER take (play), CLIPSEARCH and MARKERSEARCH take (play=true/false), and PAUSESEQUENCE, MEDIAENABLE, ARTNET and SETTIMELINEVISIBILITY take (y/n). ([specs/7thsense-delta.yaml:610](specs/7thsense-delta.yaml#L610))
- [ ] Check the separator spacing of the STATUS lines on current versions (the example shows SERVER  :, FILE   : and CURRENTFRAME:), and whether the label is LICENCE or LICENSE and OTHERDRIVE or OTHERDRIVES. ([specs/7thsense-delta.yaml:661](specs/7thsense-delta.yaml#L661))
- [ ] Confirm what a follower answers to the leader-only commands (PLAY, CUE, GOTOFRAME...). ([specs/7thsense-delta.yaml:624](specs/7thsense-delta.yaml#L624))

## ableton-live — Ableton Live (AbletonOSC)

- [ ] Confirm that AbletonOSC sends replies and listener updates to the last sender's address on UDP 11001, so commands sent from local port 11001 get their replies, and that a second client moves the updates away. ([specs/ableton-live.yaml:3007](specs/ableton-live.yaml#L3007))
- [ ] Check what happens to registered listeners when a track or scene is inserted or deleted, and that resync registers them again under the new indexes. ([specs/ableton-live.yaml:3015](specs/ableton-live.yaml#L3015))
- [ ] Check which properties Live really lets AbletonOSC listen to (start_listen) on tracks, clip slots, clips, scenes and devices, and which answer /live/error instead (the spec follows the Live Object Model's observe flags). ([specs/ableton-live.yaml:3027](specs/ableton-live.yaml#L3027))
- [ ] Confirm each walk ends with a /live/error reply past the last index (is_foldable, is_empty, is_group_slot, class_name, send), and how long a walk of a large Set takes. ([specs/ableton-live.yaml:3037](specs/ableton-live.yaml#L3037))
- [ ] Check the "*" track index on start_listen for color, color_index, fired_slot_index, mute, name, panning, playing_slot_index, solo and volume covers every track, including group tracks, without one track's error stopping the rest. ([specs/ableton-live.yaml:3037](specs/ableton-live.yaml#L3037))
- [ ] Check watch_device_parameters on a large device (a rack, a plug-in) and the load listening to every parameter puts on Live. ([specs/ableton-live.yaml:3046](specs/ableton-live.yaml#L3046))
- [ ] Confirm Live accepts integer 1 and 0 for boolean properties (mute, solo, arm, loop, warping...) and for a note's mute in /live/clip/add/notes, and that it reports booleans as OSC true and false. ([specs/ableton-live.yaml:3065](specs/ableton-live.yaml#L3065))
- [ ] Confirm fired_slot_index reports -2 when the track's stop button is fired and -1 when nothing is fired, and that playing_slot_index follows clips started from Live. ([specs/ableton-live.yaml:3071](specs/ableton-live.yaml#L3071))
- [ ] Check that arm and current_monitoring_state are readable on every track that can be armed, and fold_state on every group track, and that they answer /live/error elsewhere. ([specs/ableton-live.yaml:3078](specs/ableton-live.yaml#L3078))
- [ ] Check current_monitoring_state's values (0 In, 1 Auto, 2 Off) and the device type values (1, 2, 4 in the README; 0, 1, 2 in a source comment). ([specs/ableton-live.yaml:3104](specs/ableton-live.yaml#L3104))
- [ ] Check the value ranges Live accepts for tempo, song and scene signatures, clip transposition and detune, groove amount (the Live Object Model says 0 to 1), and what Live does with a value outside them. ([specs/ableton-live.yaml:3097](specs/ableton-live.yaml#L3097))
- [ ] Confirm /live/startup is sent when Live loads another Set, so the rule registering everything again runs then. ([specs/ableton-live.yaml:3112](specs/ableton-live.yaml#L3112))
- [ ] Record Live's dB for several mixer volume values (0.85 is taken to be 0 dB) so a dB law can be stated. ([specs/ableton-live.yaml:2475](specs/ableton-live.yaml#L2475))
- [ ] Record the dB Live's meters show for several output meter values (0.0 to 1.0) so their scale can be stated. ([specs/ableton-live.yaml:2468](specs/ableton-live.yaml#L2468))

## adder-aim — Adder AdderLink Infinity Manager (AIM API)

- [ ] Check what the AIM answers a request with an ended or unknown token: the integration logs in again only when the error's message names the login, the token or the session, since appendix K documents no code for it. ([specs/adder-aim.yaml:314](specs/adder-aim.yaml#L314))
- [ ] Check on AIM 5.10 that get_devices for receivers lists a current connection with an empty con_end_time, as the example shows, so receivers.<id>.connected is right, and that pages past 1000 devices come as total_devices says. ([specs/adder-aim.yaml:309](specs/adder-aim.yaml#L309))

## adder-xdip — Adder XDIP receivers

- [ ] Check that a switch with an expired token (after a day) is answered 401, which logs in again, and that a receiver without an Access password answers the access login with 401. ([specs/adder-xdip.yaml:49](specs/adder-xdip.yaml#L49))

## aes67-sap — AES67 stream discovery (SAP and SDP)

- [ ] Check that Dante devices in AES67 mode, RAVENNA devices and other AES67 senders announce on 239.255.255.255:9875 and appear as streams, with their names, channels, multicast addresses and PTP grandmaster. ([specs/aes67-sap.yaml:58](specs/aes67-sap.yaml#L58))
- [ ] Check how often real AES67 announcers repeat (about 30 s is commonly seen, not taken from a read standard), and that Dante Controller and a RAVENNA receiver keep a stream this integration announces every 30 s. ([specs/aes67-sap.yaml:60](specs/aes67-sap.yaml#L60))
- [ ] Check whether Dante and RAVENNA devices send a SAP deletion when a stream is removed, and that it removes the stream here; and how long a stream lingers in state when its device is unplugged. ([specs/aes67-sap.yaml:140](specs/aes67-sap.yaml#L140))
- [ ] Check the a=ts-refclk forms real devices write (RFC 7273's domain-nmbr= or the bare domain number in Audinate's sample) and that both read into grandmaster and domain. ([specs/aes67-sap.yaml:116](specs/aes67-sap.yaml#L116))
- [ ] Check whether current announcers still send a=framecount, and the encodings (L16, L24, AM824) and packet times they announce. ([specs/aes67-sap.yaml:122](specs/aes67-sap.yaml#L122))
- [ ] Check on a host with several network interfaces that the group is joined on each, and which interface this integration's announcements leave by. ([specs/aes67-sap.yaml:129](specs/aes67-sap.yaml#L129))

## aes70 — AES70 / OCA devices

- [ ] Check the device's OCP.1 port (AES70 assigns none) and which transports it accepts: TCP, UDP, WebSocket (and its path), or only secure OCP.1. ([specs/aes70.yaml:401](specs/aes70.yaml#L401))
- [ ] Check whether the device sends OcaDelay's delay time (OcaTimeInterval) as a 32-bit or a 64-bit float, and that set_delay written at that width is accepted. ([specs/aes70.yaml:460](specs/aes70.yaml#L460))
- [ ] Confirm the device answers the controller's keep-alive with keep-alives of its own at that interval, so a device opened for commands only is reported connected within one interval, and whether it accepts the millisecond form for an interval that is not whole seconds. ([specs/aes70.yaml:466](specs/aes70.yaml#L466))
- [ ] Confirm the device sends keep-alives while idle, so it is not dropped after three intervals of quiet, and that it closes the connection itself after three intervals without the controller's. ([specs/aes70.yaml:474](specs/aes70.yaml#L474))
- [ ] Confirm AddSubscription (3.1 on object 4) for an object's PropertyChanged event, with subscriber method 1.1 of object 4096, an empty context, delivery mode 1 and an empty destination, is accepted, and that changes then arrive as notifications carrying the property id, value and change type (record which PDU type, 2 or 5). ([specs/aes70.yaml:481](specs/aes70.yaml#L481))
- [ ] Check that GetMembers (3.5) answers on every block, how long a full walk takes on a large device (a DS100 or an amplifier with many channels), and whether 32 requests in flight at once is too many for it. ([specs/aes70.yaml:490](specs/aes70.yaml#L490))
- [ ] Check that roles are unique within each block and contain no '/', so role paths are unambiguous. ([specs/aes70.yaml:498](specs/aes70.yaml#L498))
- [ ] Confirm that a role path lookup when opened for commands only (GetMembers and GetRole level by level) works, and that GetClassIdentification (1.1) answers for an object addressed by number. ([specs/aes70.yaml:504](specs/aes70.yaml#L504))
- [ ] Check what a setter answers for a value out of range (ParameterOutOfRange, or a clamped value notified) and for a locked object, and record the status codes seen. ([specs/aes70.yaml:512](specs/aes70.yaml#L512))
- [ ] Confirm the bounded getters (OcaGain GetGain, OcaSwitch GetPosition, OcaDelay GetDelayTime, the numeric basic actuators, OcaLevelSensor GetReading) return the value, minimum and maximum in that order. ([specs/aes70.yaml:359](specs/aes70.yaml#L359))
- [ ] Confirm the basic sensors (OcaBooleanSensor to OcaStringSensor) answer GetReading (5.1) with the reading, and the numeric ones also the minimum and maximum, as the basic actuators' GetSetting does; Focusrite's RedNet8 chart has OcaInt8Sensor status objects. ([specs/aes70.yaml:376](specs/aes70.yaml#L376))
- [ ] Check level sensor readings: the unit (dBFS or dBu) and range, and whether polling every sensor once a second is acceptable on a device with many meters. ([specs/aes70.yaml:521](specs/aes70.yaml#L521))
- [ ] Confirm the device manager getters: ModelGUID (3.2) as 8 bytes, ModelDescription (3.6) as three strings, State (3.13) as a 16-bit set, and which optional ones (DeviceRevisionID 3.20) the device implements. ([specs/aes70.yaml:386](specs/aes70.yaml#L386))
- [ ] Check a role or label with non-ASCII characters: OCP.1 string lengths are taken to count Unicode characters, not bytes. ([crates/core/src/modules/aes70_codec.rs:244](crates/core/src/modules/aes70_codec.rs#L244))
- [ ] Record the class ids of a vendor's proprietary objects (d&b, Powersoft, stagebox preamps) and whether they derive from the standard classes, so they are read and followed. ([specs/aes70.yaml:358](specs/aes70.yaml#L358))
- [ ] On a device that speaks OCP.1 over UDP, confirm it ignores nothing after the controller's first keep-alive, answers in one PDU per datagram to the controller's address and port, sends its notifications there too, and accepts PDUs of up to 1452 bytes. ([specs/aes70.yaml:408](specs/aes70.yaml#L408))
- [ ] On a device that speaks OCP.1 over a WebSocket, note its path and port, confirm it accepts the AES70-OCP.1 subprotocol (or needs ws_subprotocol off), answers keep-alives sent over the WebSocket, and, over wss, what certificate it presents. ([specs/aes70.yaml:418](specs/aes70.yaml#L418))
- [ ] On a device that offers secure OCP.1, note its port and transport (_ocasec._tcp or _ocasec._udp), confirm it negotiates TLS_DHE_PSK_WITH_AES_128_CBC_SHA over TLS 1.2 (or DTLS 1.2) and sends the identity hint OCA-PSK, which identity it expects (OCA-PSK or its own), and how its pre-shared key is set and shown (hex, and its length). ([specs/aes70.yaml:427](specs/aes70.yaml#L427))
- [ ] Record the Diffie-Hellman group size a secure device offers (at least 2048 bits is required; a 1024-bit group is refused) and whether it agrees to the extended master secret and encrypt-then-MAC. ([specs/aes70.yaml:427](specs/aes70.yaml#L427))
- [ ] Confirm that a wrong key or identity shows as a refused handshake (and which alert the device sends), and that the device answers the keep-alive sent first inside the secure session as it does over TCP. ([specs/aes70.yaml:427](specs/aes70.yaml#L427))
- [ ] On a device that speaks secure OCP.1 over DTLS, confirm it takes one OCP.1 PDU per record and datagram and answers the same way, whether it asks for a cookie, and that it sends its flight again when the controller's is lost. ([specs/aes70.yaml:444](specs/aes70.yaml#L444))

## aimedia-icap — AI-Media / EEG iCap

- [ ] Record the JSON replies of the list and get calls (users, access codes, active encoders and captioners, health), so the live encoders and captioners can be kept in state. ([specs/aimedia-icap.yaml:375](specs/aimedia-icap.yaml#L375))
- [ ] Confirm the path of the access code PUT (the collection, as the spec sends it, or accesscodes/<code>). ([specs/aimedia-icap.yaml:384](specs/aimedia-icap.yaml#L384))
- [ ] Record the body of a 401 for wrong credentials and for a CaptionCast without a paid account, so the two can be told apart. ([specs/aimedia-icap.yaml:384](specs/aimedia-icap.yaml#L384))
- [ ] Record the success statuses of POST and DELETE calls (200, 201 or 204 accepted). ([specs/aimedia-icap.yaml:106](specs/aimedia-icap.yaml#L106))

## aja-bridge-live — AJA BRIDGE LIVE

- [ ] Confirm the unit's remote API address: https on 443 with a self-signed certificate (as the API document's examples), and whether http on 8080 is reachable from the network or only on the unit (the manual's examples). ([specs/aja-bridge-live.yaml:51](specs/aja-bridge-live.yaml#L51))
- [ ] Compare the unit's current on-box API documentation (v1.20) with the public v1.16 PDF this spec follows, and note any endpoint or field that changed. ([specs/aja-bridge-live.yaml:8](specs/aja-bridge-live.yaml#L8))
- [ ] Confirm that the replies to PUT /api/pipeline/{id} and DELETE /api/pipeline/{id} have an empty body (the rule dropping a pipeline's runtime state relies on it), and what the replies to POST /api/pipelines and PUT .../start and .../stop carry. ([specs/aja-bridge-live.yaml:437](specs/aja-bridge-live.yaml#L437))
- [ ] Check what GET /api/pipeline/{id}/state returns for a pipeline that is not started (the document describes it for a running pipeline). ([specs/aja-bridge-live.yaml:293](specs/aja-bridge-live.yaml#L293))
- [ ] Confirm that elementaryStreams appear in the state of every input, and that the order is stable between reads, since streams are keyed by their place. ([specs/aja-bridge-live.yaml:384](specs/aja-bridge-live.yaml#L384))
- [ ] Confirm that ndiGroup in GET /api/system/ndi-devices takes a comma-separated list as sent percent-encoded. ([specs/aja-bridge-live.yaml:494](specs/aja-bridge-live.yaml#L494))

## aja-helo — AJA HELO and HELO Plus

- [ ] Confirm what eParamID_ReplicatorCommand 5 (Shutdown) shuts down on HELO and HELO Plus (the replicator engine or the whole unit) and how the unit comes back from it. ([specs/aja-helo.yaml:761](specs/aja-helo.yaml#L761))
- [ ] Confirm that a request without the session cookie, and one with an expired session, is answered with the HTML 404 page, so a 404 is the right signal to log in again. ([specs/aja-helo.yaml:104](specs/aja-helo.yaml#L104))
- [ ] Check how /authenticator/login answers a wrong password (status, body, whether a session cookie is set), since AJA documents only {"login":"success"}. ([specs/aja-helo.yaml:802](specs/aja-helo.yaml#L802))
- [ ] Confirm the HELO event elements carry param_type, int_value and str_value as on Ki Pro (AJA's HELO example reads only param_id), and that the connection id expires as on Ki Pro. ([specs/aja-helo.yaml:818](specs/aja-helo.yaml#L818))
- [ ] Record the names and numbers of eParamID_ReplicatorStreamState (only 2, streaming, is documented) and of eParamID_RegisterRecallResult (only 2, Complete, is documented; Recalling appears in AJA's example). ([specs/aja-helo.yaml:832](specs/aja-helo.yaml#L832))
- [ ] Confirm that eParamID_RegisterRecallResult arrives as an event when a recall completes (AJA's example waits for it), so the configuration re-read is triggered. ([specs/aja-helo.yaml:727](specs/aja-helo.yaml#L727))
- [ ] Confirm that eParamID_RegisterRecall and eParamID_RegisterSave are reported as events with the preset number when another client recalls or saves a preset. ([specs/aja-helo.yaml:716](specs/aja-helo.yaml#L716))
- [ ] Confirm that eParamID_StreamingVideoBitrate_1 is the first streaming profile's video bit rate, find its unit (kbit/s assumed), and list the ids of the other profile instances. ([specs/aja-helo.yaml:843](specs/aja-helo.yaml#L843))
- [ ] Confirm that eParamID_RecordingProfileSel and eParamID_StreamingProfileSel 0 to 9 are the web UI's profiles 1 to 10 in order. ([specs/aja-helo.yaml:838](specs/aja-helo.yaml#L838))
- [ ] Confirm that eParamID_FilenamePrefix follows the selected recording profile (a mapped parameter), as the HELO manual places the prefix in the recording profile. ([specs/aja-helo.yaml:741](specs/aja-helo.yaml#L741))
- [ ] On a HELO Plus, check desc.json for eParamID_ReplicatorCommand, eParamID_ReplicatorRecordState, eParamID_ReplicatorStreamState, eParamID_FilenamePrefix, eParamID_MediaState, eParamID_Authentication and the preset parameters, and whether it keeps eParamID_RecordingProfileSel, eParamID_StreamingProfileSel and eParamID_VideoInSelect. ([specs/aja-helo.yaml:877](specs/aja-helo.yaml#L877))
- [ ] Confirm that eParamID_VideoInSelect on current HELO firmware still has 0 SDI, 1 HDMI and 2 Test Pattern. ([specs/aja-helo.yaml:546](specs/aja-helo.yaml#L546))
- [ ] Confirm that a recall's immediate read of eParamID_RegisterRecallResult (the recall reply's re-read) gives the in-progress value rather than the previous recall's result. ([specs/aja-helo.yaml:777](specs/aja-helo.yaml#L777))

## aja-kipro — AJA Ki Pro

- [ ] Confirm that an event connection id expires when no wait_for_config_events is made within a minute, and that the extension's back-to-back waits keep it alive over a long session. ([specs/aja-kipro.yaml:1482](specs/aja-kipro.yaml#L1482))
- [ ] Check how the unit answers wait_for_config_events with an expired connection id (status code and body), since AJA does not document it and the extension reconnects on anything that is not a 2xx JSON array. ([specs/aja-kipro.yaml:1498](specs/aja-kipro.yaml#L1498))
- [ ] Confirm which URL opens the event connection on each model and firmware, /json (AJA examples) or /config (2021 guide). ([specs/aja-kipro.yaml:1494](specs/aja-kipro.yaml#L1494))
- [ ] On a Ki Pro GO and Ultra, check desc.json for eParamID_TransportState, eParamID_CurrentClip, eParamID_DisplayTimecode and eParamID_MediaState, and whether the 30-second poll errors on them. ([specs/aja-kipro.yaml:1525](specs/aja-kipro.yaml#L1525))
- [ ] On Ultra models, confirm the accepted range and format of eParamID_TransportRequestedSpeed (three decimals such as -1.500) for variable speed play. ([specs/aja-kipro.yaml:1578](specs/aja-kipro.yaml#L1578))
- [ ] On Ultra Plus and 12G, confirm that set_sdi_monitor_channel works when sent with paramid= (AJA's chapter 6 URL omits it). ([specs/aja-kipro.yaml:1606](specs/aja-kipro.yaml#L1606))
- [ ] On Ki Pro, Mini and Rack, check whether the deprecated /clips listing still uses unquoted JSON keys. ([specs/aja-kipro.yaml:1562](specs/aja-kipro.yaml#L1562))
- [ ] Check that a get of each input and monitor parameter answers its value as the descriptor's number, and that a change made on the unit (front panel, preset recall) arrives as an event for eParamID_VideoInput_1, eParamID_SDIMonitorChannel, eParamID_HDMIOutChannel and the others now kept in state. ([specs/aja-kipro.yaml:1286](specs/aja-kipro.yaml#L1286))
- [ ] On a GO, find the ids of the channel 2 to 4 inputs in desc.json (only eParamID_VideoInput_1 and eParamID_AudioInput_1 are documented), and on Ultra Plus and 12G the values of eParamID_EncodeChannels other than 1. ([specs/aja-kipro.yaml:1391](specs/aja-kipro.yaml#L1391))
- [ ] Check whether reading eParamID_RegisterRecall answers the last preset recalled, which AJA does not document. ([specs/aja-kipro.yaml:1508](specs/aja-kipro.yaml#L1508))
- [ ] Confirm upload_clip's PUT /media answers 200 within the 60 s the command waits after the last byte (the guide shows the answer, not when it comes). ([specs/aja-kipro.yaml:911](specs/aja-kipro.yaml#L911))
- [ ] Confirm download_clip's GET /media/<file> works on the original Ki Pro, Ki Pro Mini and Ki Pro Rack (the guide documents it for the Ki Pro GO and Ultra models), and what any model answers outside Data - LAN mode. ([specs/aja-kipro.yaml:897](specs/aja-kipro.yaml#L897))
- [ ] Confirm /clips on Ki Pro, Mini and Rack answers its clips in the order and form api.txt shows (clipname, timestamp, fourcc, width, height, framecount, framerate, interlace), with the file name URL encoded. ([specs/aja-kipro.yaml:1567](specs/aja-kipro.yaml#L1567))

## aja-kumo — AJA KUMO

- [ ] Check what a salvo crosspoint of source 0 does: turns the output off, or leaves the destination unchanged. ([specs/aja-kumo.yaml:1665](specs/aja-kumo.yaml#L1665))
- [ ] Confirm that take_salvo sending the salvo number as the eParamID_TakeSalvo value takes salvos 2-8 (AJA's own TakeSalvo always sends 1). ([specs/aja-kumo.yaml:1672](specs/aja-kumo.yaml#L1672))
- [ ] Confirm the lock parameter eParamID_XPT_Destination{n}_Locked (taken from the Companion module) exists and works, and whether lock changes arrive as events. ([specs/aja-kumo.yaml:1586](specs/aja-kumo.yaml#L1586))
- [ ] Check that string and integer values in KUMO event elements are told apart correctly, since KUMO sends no param_type and a non-empty str_value with no or zero int_value is taken as a string. ([crates/core/src/modules/aja_config_events.rs:172](crates/core/src/modules/aja_config_events.rs#L172))
- [ ] Record what a salvo change looks like on the event connection, since AJA does not document its event form. ([specs/aja-kumo.yaml:1606](specs/aja-kumo.yaml#L1606))
- [ ] Check how the router answers wait_for_config_events with an expired connection id, and that /config (not /json) gives the connection id. ([specs/aja-kumo.yaml:1608](specs/aja-kumo.yaml#L1608))
- [ ] Check whether the firmware on hand answers eParamID_XPT_DestinationAll_Status through /options, or {} as older firmware does. ([specs/aja-kumo.yaml:1618](specs/aja-kumo.yaml#L1618))
- [ ] Confirm that a number above the router's count, and a switch to a locked destination, fail with HTTP 403 (or note the actual status). ([specs/aja-kumo.yaml:1578](specs/aja-kumo.yaml#L1578))
- [ ] Check whether a burst of back-to-back requests (several routes or a salvo set at once, a full read on connecting) overloads the router's queue without the 0.1-0.25 s pauses AJA's examples add; the core sends one request at a time and paces nothing, since AJA documents no required gap. ([specs/aja-kumo.yaml:1633](specs/aja-kumo.yaml#L1633))
- [ ] Confirm button colour writes in the {'classes': 'color_N'} form are accepted. ([specs/aja-kumo.yaml:1680](specs/aja-kumo.yaml#L1680))
- [ ] Read a button colour (eParamID_Button_Settings_{n}) and its event, and check the value holds color_N as written, for a source and a destination in each bank. ([specs/aja-kumo.yaml:1690](specs/aja-kumo.yaml#L1690))
- [ ] Record the values and names eParamID_SuppressPSAlarm reads, and the ids of the switching mode, reference and temperature alarm suppression from desc.json. ([specs/aja-kumo.yaml:1715](specs/aja-kumo.yaml#L1715))
- [ ] Check what /authenticator/login answers on a unit with User Authentication Disabled (whether it still sets a session cookie). ([specs/aja-kumo.yaml:1640](specs/aja-kumo.yaml#L1640))
- [ ] Confirm eParamID_LED_Identify turns Identify on and off (it comes from a generic AJA example, not the manual). ([specs/aja-kumo.yaml:1689](specs/aja-kumo.yaml#L1689))
- [ ] With User Authentication set to Login, confirm a form POST of password_provided to /authenticator/login answers with a session cookie named session, that every /config request (and the event connection's) is accepted with it, and record how the router answers a wrong password and an expired session, which the documents do not say. ([specs/aja-kumo.yaml:84](specs/aja-kumo.yaml#L84))

## allenheath-ahm — Allen & Heath AHM

- [ ] Confirm the login encoding on the TLS port 51327: profile sent as one byte followed by the password's UTF-8 bytes, with no ", " separator or terminator, answered by AuthOK. ([specs/allenheath-ahm.yaml:243](specs/allenheath-ahm.yaml#L243))
- [ ] Check whether the processor transmits changes other than preset recall, source selection and room combination (mutes, levels) when they are made on it. ([specs/allenheath-ahm.yaml:256](specs/allenheath-ahm.yaml#L256))
- [ ] Confirm that -48 dB sent as 01 sets -48 dB, and that received levels read back as the expected dB. ([specs/allenheath-ahm.yaml:261](specs/allenheath-ahm.yaml#L261))
- [ ] Check trim (-24 to +24 dB) and preamp gain (+5 to +60 dB) at several points, since only their ends are documented and the spec spaces them evenly. ([specs/allenheath-ahm.yaml:264](specs/allenheath-ahm.yaml#L264))
- [ ] Confirm the source selector get is answered when sent with N = 1, and that its reply is told apart from the message sent on a selection. ([specs/allenheath-ahm.yaml:269](specs/allenheath-ahm.yaml#L269))
- [ ] Confirm room numbers in the room combiner message are 00-0F for rooms 1-16. ([specs/allenheath-ahm.yaml:273](specs/allenheath-ahm.yaml#L273))
- [ ] Confirm that playback channel 02 plays on the stereo playback channel (V1.5), on firmware V1.5x and V1.6x. ([specs/allenheath-ahm.yaml:232](specs/allenheath-ahm.yaml#L232))
- [ ] On firmware V1.60 or later, check that a name longer than 8 characters comes back whole in the name reply. ([specs/allenheath-ahm.yaml:352](specs/allenheath-ahm.yaml#L352))

## allenheath-cq — Allen & Heath CQ

- [ ] Confirm the pan centre value 40 00 and level values between the p.15 table points. ([specs/allenheath-cq.yaml:199](specs/allenheath-cq.yaml#L199))
- [ ] Check that a smaller CQ model ignores input and output numbers it lacks rather than erroring. ([specs/allenheath-cq.yaml:207](specs/allenheath-cq.yaml#L207))

## allenheath-dlive — Allen & Heath dLive / Avantis

- [ ] Confirm the login encoding on the TLS ports 51327 and 51329: profile sent as one byte followed by the password's UTF-8 bytes, with no separator or terminator, answered by AuthOK. ([specs/allenheath-dlive.yaml:394](specs/allenheath-dlive.yaml#L394))
- [ ] Check how the login profile number 00-1F maps to the console's user profiles. ([specs/allenheath-dlive.yaml:398](specs/allenheath-dlive.yaml#L398))
- [ ] On Avantis, find the TLS port (not documented) and confirm tls works with it set. ([specs/allenheath-dlive.yaml:385](specs/allenheath-dlive.yaml#L385))
- [ ] Check that a plain TCP connection to a console in Secure mode gets no answer (so the warning fires) rather than being dropped. ([specs/allenheath-dlive.yaml:387](specs/allenheath-dlive.yaml#L387))
- [ ] Check whether the console transmits changes other than scene recall (mutes, faders, names) when made on the surface. ([specs/allenheath-dlive.yaml:416](specs/allenheath-dlive.yaml#L416))
- [ ] Confirm fader and send level rounding: 6B reads 0 dB, +5 dB is 75, and on Avantis the hex column (not the decimal column) matches the console. ([specs/allenheath-dlive.yaml:423](specs/allenheath-dlive.yaml#L423))
- [ ] Confirm preamp gain follows the p.9 table (+30 dB is 3A) and that the preamp get works with the socket number MP in place of CH, including DX sockets. ([specs/allenheath-dlive.yaml:431](specs/allenheath-dlive.yaml#L431))
- [ ] Confirm that group and aux assignment off, sent as 3F, unassigns. ([specs/allenheath-dlive.yaml:438](specs/allenheath-dlive.yaml#L438))
- [ ] Check whether dLive accepts the UFX chromatic scale (02), which is offered on Avantis only. ([specs/allenheath-dlive.yaml:445](specs/allenheath-dlive.yaml#L445))
- [ ] Check how dLive handles a name longer than it allows (presumably truncated). ([specs/allenheath-dlive.yaml:450](specs/allenheath-dlive.yaml#L450))
- [ ] On a Surface, confirm Go, Next and Previous respond to their assigned Control Change sent on the base MIDI channel N, which CC numbers are reserved, and whether the Surface transmits the CC when Go is pressed. ([specs/allenheath-dlive.yaml:477](specs/allenheath-dlive.yaml#L477))
- [ ] Opened for commands only, check that the console answers the liveness probe when nothing else has been sent, since that answer is what reports it connected. ([crates/core/src/modules/allenheath.rs:41](crates/core/src/modules/allenheath.rs#L41))

## allenheath-qu — Allen & Heath Qu

- [ ] Confirm the mixer sends Active Sensing (FE) about every 300 ms and keeps the connection with the core's FE every second. ([specs/allenheath-qu.yaml:278](specs/allenheath-qu.yaml#L278))
- [ ] Check pan and trim at several points, since only their ends are documented and the spec spaces them evenly. ([specs/allenheath-qu.yaml:292](specs/allenheath-qu.yaml#L292))
- [ ] Confirm the DCA assign values address DCAs 1-4 (the document repeats the mute group label). ([specs/allenheath-qu.yaml:299](specs/allenheath-qu.yaml#L299))
- [ ] Confirm stereo mixes, groups and matrices answer on their odd number and the LR destination is index 07. ([specs/allenheath-qu.yaml:298](specs/allenheath-qu.yaml#L298))
- [ ] Confirm remote shutdown works when sent on MIDI channel 1 (B0) whatever the mixer's channel. ([specs/allenheath-qu.yaml:305](specs/allenheath-qu.yaml#L305))
- [ ] With meters on, record how often the mixer sends meter data, whether it stops when the connection closes, and that each value is big-endian. ([specs/allenheath-qu.yaml:323](specs/allenheath-qu.yaml#L323))
- [ ] Check the meter block order on each model: that the first column of the p.12 table is the Qu-16's, and the Qu-24's 180 unused meters and two group blocks against the Qu-32's 20 unused meters, inputs 25-32 and four group blocks. ([specs/allenheath-qu.yaml:324](specs/allenheath-qu.yaml#L324))

## allenheath-sq — Allen & Heath SQ

- [ ] Confirm the linear taper table values (not the disagreeing p.13 examples) set the expected dB, and check a few points of the "approximate" audio taper table. ([specs/allenheath-sq.yaml:325](specs/allenheath-sq.yaml#L325))
- [ ] Confirm LR mute is 00 44 (the p.18 example uses 00 00). ([specs/allenheath-sq.yaml:335](specs/allenheath-sq.yaml#L335))
- [ ] On SQ-5/6/7, check whether matrices 4-6 exist over MIDI (the firmware V1.6 article lists them). ([specs/allenheath-sq.yaml:340](specs/allenheath-sq.yaml#L340))
- [ ] On Qu-5/6/7 under this spec, confirm USB to LR uses 40 24 and the group-to-matrix assignment values. ([specs/allenheath-sq.yaml:353](specs/allenheath-sq.yaml#L353))

## analogway-alta4k — Analog Way Alta 4K

- [ ] Check which of followScreen/@props/source (the guide's write) and followScreen/@props/screen (its read) the device uses; state reads the second. ([specs/analogway-alta4k.yaml:1028](specs/analogway-alta4k.yaml#L1028))
- [ ] Check that subscribing to DeviceObject/$output, DeviceObject/audio and DeviceObject/preconfig pushes audio, output and screen-enable changes. ([specs/analogway-alta4k.yaml:1057](specs/analogway-alta4k.yaml#L1057))
- [ ] Confirm the auxiliary screen T-bar read path DeviceObject/transition/$auxiliaryScreen/@items/<n>/status/@props/transition, which is inferred. ([specs/analogway-alta4k.yaml:1020](specs/analogway-alta4k.yaml#L1020))
- [ ] Confirm the resolved guide ambiguities: '$auxillaryScreen' as printed, audio modes with underscores, followScreen/@props/source for follow-screen writes, audio/$lineOut, and $preset in the background status read. ([specs/analogway-alta4k.yaml:1026](specs/analogway-alta4k.yaml#L1026))
- [ ] Confirm that errors arrive in message order, so matching each to the oldest unanswered message is right. ([specs/analogway-alta4k.yaml:1013](specs/analogway-alta4k.yaml#L1013))

## analogway-livecore — Analog Way LiveCore

- [ ] Confirm master preset screen enabling uses PSose (not PSsse), GPits takes a value, and layers 1-24 are accepted. ([specs/analogway-livecore.yaml:967](specs/analogway-livecore.yaml#L967))
- [ ] Check that an out-of-range value is answered with the previous value followed by an E10-E13 error. ([specs/analogway-livecore.yaml:961](specs/analogway-livecore.yaml#L961))
- [ ] Check how often unsolicited register pushes arrive while a command waits, which makes that command fail. ([specs/analogway-livecore.yaml:954](specs/analogway-livecore.yaml#L954))
- [ ] Confirm the index order of MMsav (device, then memory) and CMsav (screen, then memory), the reverse of the loads, and that writing 1 saves the current layout. ([specs/analogway-livecore.yaml:611](specs/analogway-livecore.yaml#L611))
- [ ] Check that COupd applies confidence layout and source changes, as MLupd does for monitoring. ([specs/analogway-livecore.yaml:604](specs/analogway-livecore.yaml#L604))
- [ ] Check that a plug the input card lacks is refused by INplg with an error. ([specs/analogway-livecore.yaml:530](specs/analogway-livecore.yaml#L530))

## analogway-livepremier — Analog Way LivePremier / Aquilon

- [ ] Check that subscribing to DeviceObject/audio, DeviceObject/$still, DeviceObject/preconfig/backgrounds and DeviceObject/backup pushes their changes, and how often isLevelDetected changes are pushed. ([specs/analogway-livepremier.yaml:1061](specs/analogway-livepremier.yaml#L1061))
- [ ] Check that a TCP client cannot set layer position, size or opacity, transitions or output settings over AWJ, or find the paths: the guide documents none. ([specs/analogway-livepremier.yaml:1075](specs/analogway-livepremier.yaml#L1075))
- [ ] Confirm the auxiliary screen T-bar read path DeviceObject/$screenAuxGroup/@items/A<n>/status/@props/transition, which is inferred. ([specs/analogway-livepremier.yaml:1025](specs/analogway-livepremier.yaml#L1025))
- [ ] Confirm freeze_input can write the input's freeze property (the guide documents only reading it). ([specs/analogway-livepremier.yaml:1031](specs/analogway-livepremier.yaml#L1031))
- [ ] Check whether reboot and shutdown work on DeviceObject/system/shutdown/cmd or need the long v4.0 path. ([specs/analogway-livepremier.yaml:1037](specs/analogway-livepremier.yaml#L1037))
- [ ] Confirm that reading an object returns the paths beneath it, as read_on_connect assumes. ([specs/analogway-livepremier.yaml:1063](specs/analogway-livepremier.yaml#L1063))
- [ ] Confirm errors arrive in message order, so matching each to the oldest unanswered message is right. ([specs/analogway-livepremier.yaml:1017](specs/analogway-livepremier.yaml#L1017))

## analogway-midra — Analog Way Midra

- [ ] Confirm TAKE ALL is accepted as "1GCtal" (the guide also shows "1,GCtal"). ([specs/analogway-midra.yaml:884](specs/analogway-midra.yaml#L884))
- [ ] Confirm LF alone is accepted as the command terminator. ([specs/analogway-midra.yaml:886](specs/analogway-midra.yaml#L886))
- [ ] Check what the device answers to a TAKE sent while TAKEAVA is 0. ([specs/analogway-midra.yaml:872](specs/analogway-midra.yaml#L872))
- [ ] Check that PRbst and PRbal take the same indexes as PRinp (screen, 0 program or 1 preview, layer) and are applied by PUscu. ([specs/analogway-midra.yaml:300](specs/analogway-midra.yaml#L300))
- [ ] Check the clock is set by writing RTyea to RTsec and then 2RTreq, and that RTmon counts months from 0. ([specs/analogway-midra.yaml:452](specs/analogway-midra.yaml#L452))

## analogway-midra4k — Analog Way Midra 4K

- [ ] Check which of followScreen/@props/source (the guide's write) and followScreen/@props/screen (its read) the device uses; state reads the second. ([specs/analogway-midra4k.yaml:887](specs/analogway-midra4k.yaml#L887))
- [ ] Check that subscribing to DeviceObject/$output, DeviceObject/audio and DeviceObject/preconfig pushes audio, output and screen-enable changes. ([specs/analogway-midra4k.yaml:922](specs/analogway-midra4k.yaml#L922))
- [ ] Confirm the auxiliary screen T-bar read path, which is inferred from the auxiliary xTake. ([specs/analogway-midra4k.yaml:879](specs/analogway-midra4k.yaml#L879))
- [ ] Confirm the resolved guide ambiguities: '$auxillaryScreen', underscored audio modes, followScreen/@props/source, audio/$lineOut, and $preset in the background status read. ([specs/analogway-midra4k.yaml:885](specs/analogway-midra4k.yaml#L885))
- [ ] Check whether screen 3 exists (the REST guide gives 1-2, the multiviewer source list shows SCREEN_PRGM_3). ([specs/analogway-midra4k.yaml:913](specs/analogway-midra4k.yaml#L913))

## analogway-picturall — Analog Way Picturall

- [ ] Check whether control status messages separate controls with the literal string "\n" or with real line breaks, and that they end with "\n)" as the manual's example does (the per-control state reads them so). ([specs/analogway-picturall.yaml:527](specs/analogway-picturall.yaml#L527))
- [ ] Confirm unquoted set values are accepted. ([specs/analogway-picturall.yaml:533](specs/analogway-picturall.yaml#L533))
- [ ] Check what media_end_action values mean and which playback commands besides go exist. ([specs/analogway-picturall.yaml:538](specs/analogway-picturall.yaml#L538))
- [ ] Check that the server sends a control status message for every object on ctrl_status, so objects.*.controls.* fills for all of them, and how large it is on a big show. ([specs/analogway-picturall.yaml:443](specs/analogway-picturall.yaml#L443))
- [ ] Confirm the object list message ends each entry with a colon (id:name:) as the D.e.e example shows. ([specs/analogway-picturall.yaml:449](specs/analogway-picturall.yaml#L449))
- [ ] Check that lua with backslash-escaped quotes runs set_pda_value_by_index, connect and send_ui_event as sent, and what the server answers when Lua raises an error. ([specs/analogway-picturall.yaml:554](specs/analogway-picturall.yaml#L554))
- [ ] Find the parameter names of a layer's transform, colour, crop and draw mode with object_info on a server, to give them typed commands. ([specs/analogway-picturall.yaml:576](specs/analogway-picturall.yaml#L576))

## apc-rack-pdu — APC Rack PDU (SNMP)

- [ ] Confirm which models implement rPDU2 (AP84xx, AP86xx, AP88xx, AP89xx, APDU9xxx) and which only rPDU (AP78xx, AP79xx); the FAQ names only AP86XX/AP89XX for rPDU2. ([specs/apc-rack-pdu.yaml:37](specs/apc-rack-pdu.yaml#L37))
- [ ] Walk a 2G PDU and check every table is keyed by a single integer index, as the MIB declares, and that a metered PDU lists no switched outlet rows. ([specs/apc-rack-pdu.yaml:41](specs/apc-rack-pdu.yaml#L41))
- [ ] Check an outlet's control command reads back as immediate_on or immediate_off after a SET, and when a delayed action's result shows in the switched status table. ([specs/apc-rack-pdu.yaml:1052](specs/apc-rack-pdu.yaml#L1052))
- [ ] Check the units taken from the MIB's descriptions (tenths of amps, hundredths of kilowatts, tenths of kilowatt-hours) against the PDU's display. ([specs/apc-rack-pdu.yaml:1047](specs/apc-rack-pdu.yaml#L1047))
- [ ] Receive a v1 trap from the PDU (an outlet turned off) and check its snmpTrapOID is enterprises.318.0.n and is named. ([specs/apc-rack-pdu.yaml:1059](specs/apc-rack-pdu.yaml#L1059))
- [ ] Check a first-generation PDU (AP79xx) answers GETBULK walks of the rPDU group over v2c, or needs v1. ([specs/apc-rack-pdu.yaml:44](specs/apc-rack-pdu.yaml#L44))

## apc-ups — APC UPS (SNMP)

- [ ] Walk a Smart-UPS with a Network Management Card 2 or 3 and check which groups it fills, and that every table is keyed by single integer indexes as the MIB declares. ([specs/apc-ups.yaml:36](specs/apc-ups.yaml#L36))
- [ ] Check that upsAdvControlUpsOff and the other control objects read back their idle value (no_turn_ups_off) after a SET, and when a graceful turn-off shows in upsBasicOutputStatus. ([specs/apc-ups.yaml:1629](specs/apc-ups.yaml#L1629))
- [ ] Check the units taken from the MIB's descriptions (tenths of volts in the high-precision objects, TimeTicks runtimes) against the UPS's display. ([specs/apc-ups.yaml:1624](specs/apc-ups.yaml#L1624))
- [ ] Receive a v1 trap (on battery, then back on line) and check it is named and that the poll after it shows the change. ([specs/apc-ups.yaml:1636](specs/apc-ups.yaml#L1636))

## arkaos-mediamaster — ArKaos MediaMaster

- [ ] Confirm the Remote Command Service listens on TCP 45678 and takes each command as a line ended by CR LF; the user guide does not say. ([specs/arkaos-mediamaster.yaml:49](specs/arkaos-mediamaster.yaml#L49))
- [ ] Compare the command texts with those MediaMaster's TELNET tab shows (Cue:<n>, Controller:Play, Fullscreen:On, Properties:Mixing, View:Remote Player...), on MediaMaster 6 and later. ([specs/arkaos-mediamaster.yaml:68](specs/arkaos-mediamaster.yaml#L68))
- [ ] Check whether the service ever answers (an acknowledgement or an error), and whether the cue player's state can be read in any way. ([specs/arkaos-mediamaster.yaml:12](specs/arkaos-mediamaster.yaml#L12))
- [ ] Check what Controller:Final15 and Controller:Final30 do. ([specs/arkaos-mediamaster.yaml:164](specs/arkaos-mediamaster.yaml#L164))

## artnet — Art-Net 4 network

- [ ] Check that nodes ignore ArtAddress fields sent as 0x7F (no change) and act on those with bit 7 set, and that 0 resets a field to the physical switch. ([specs/artnet.yaml:433](specs/artnet.yaml#L433))
- [ ] Check which universes a node expects ArtDmx for when a port's SwIn and SwOut differ, and that nodes answer an ArtPoll unicast to them without targeted mode. ([specs/artnet.yaml:433](specs/artnet.yaml#L433))
- [ ] Check that ArtCommand receivers read the ESTA code high byte first (SwoutText, SwinText with 0xFFFF are unaffected). ([specs/artnet.yaml:433](specs/artnet.yaml#L433))
- [ ] Check that consoles and DMX-Workshop list the core's own ArtPollReply (OEM 0x00FF, IP 0.0.0.0 without a bind address) without complaint. ([specs/artnet.yaml:410](specs/artnet.yaml#L410))
- [ ] On a node, check that set_node_input_enabled, node_command and the set_node_* commands are each answered with an ArtPollReply within 3 s. ([specs/artnet.yaml:394](specs/artnet.yaml#L394))

## atlona-matrix — Atlona matrix switchers and switchers

- [ ] Record the Telnet login prompts (Username:, Password:) and what a wrong login answers. ([specs/atlona-matrix.yaml:295](specs/atlona-matrix.yaml#L295))
- [ ] Confirm the RS-232 factory default of PRO3 matrices and the AT-HDR-H2H-88MA (115200 8N1 assumed from the CSpara example). ([specs/atlona-matrix.yaml:275](specs/atlona-matrix.yaml#L275))
- [ ] Confirm CSpara and RS232para write parity as 0 none, 1 odd, 2 even. ([specs/atlona-matrix.yaml:308](specs/atlona-matrix.yaml#L308))
- [ ] Check whether PRO3 matrices and the H2H need a gap between commands (none is documented; Omega asks for 500 ms). ([specs/atlona-matrix.yaml:264](specs/atlona-matrix.yaml#L264))
- [ ] Confirm the PS62 sets the repeat count with RepeatCmd n, as its document writes RepCmdTime. ([specs/atlona-matrix.yaml:1453](specs/atlona-matrix.yaml#L1453))
- [ ] Record Status on the PS62 and SW32: xINAVxOUT or xINVxOUT as their examples show. ([specs/atlona-matrix.yaml:2638](specs/atlona-matrix.yaml#L2638))
- [ ] Record what a PRO3 answers to VOUTn + and VOUTn - (the new level, or the command echoed). ([specs/atlona-matrix.yaml:751](specs/atlona-matrix.yaml#L751))
- [ ] Record the MH21 InputFormat answer: the format alone or prefixed with InputFormatN. ([specs/atlona-matrix.yaml:944](specs/atlona-matrix.yaml#L944))
- [ ] Confirm the MH21 takes HDCPSet with a space before the input (HDCPSet 1 on). ([specs/atlona-matrix.yaml:992](specs/atlona-matrix.yaml#L992))
- [ ] Confirm Unlock is answered Unlock (the PRO3 document shows Lock for both). ([specs/atlona-matrix.yaml:1128](specs/atlona-matrix.yaml#L1128))
- [ ] Confirm show_host_name answers the bare host name. ([specs/atlona-matrix.yaml:1637](specs/atlona-matrix.yaml#L1637))
- [ ] Record what Broadcast on sends when routing changes in the web GUI (assumed the same text as the echoes). ([specs/atlona-matrix.yaml:1389](specs/atlona-matrix.yaml#L1389))
- [ ] Confirm IPChangePass takes user, old and new password (the document lists a fourth, the confirmation, but its example has three). ([specs/atlona-matrix.yaml:1610](specs/atlona-matrix.yaml#L1610))
- [ ] Check whether TCP 9000 on Omega switchers needs a login and answers like Telnet. ([specs/atlona-matrix.yaml:85](specs/atlona-matrix.yaml#L85))
- [ ] Confirm All# is answered All# on PRO3 matrices and with the new routes on the H2H and PS62. ([specs/atlona-matrix.yaml:634](specs/atlona-matrix.yaml#L634))
- [ ] AT-HDR-H2H-44M: confirm HDCPSet takes a space before the input (HDCPSet 1 on), as its document writes it, unlike the 44MA and 88MA. ([specs/atlona-matrix.yaml:407](specs/atlona-matrix.yaml#L407))
- [ ] AT-HDR-H2H-44M and -44MA: record what EDIDOut's one number is (an output or an EDID memory), so save_output_edid can be offered. ([specs/atlona-matrix.yaml:427](specs/atlona-matrix.yaml#L427))
- [ ] AT-HDR-H2H-44MA: confirm the longest name InNameSet, OutNameSet and PreNameSet take, whether a name may hold spaces, and that the 44MA needs 500 ms between commands as its document says. ([specs/atlona-matrix.yaml:1015](specs/atlona-matrix.yaml#L1015))
- [ ] AT-OME-ST31: the RS-232 control port runs at 9600 8N1, as on the ST31A (its document gives no line settings) ([specs/atlona-matrix.yaml:277](specs/atlona-matrix.yaml#L277))
- [ ] AT-OME-SR21: HDMIAUD, RelayAct, USBHostRoute and VOUT sta answer in the form of the document's examples (HDMIAUD on, RelayAct1 close, USBHostRoute local, VOUT -30) ([specs/atlona-matrix.yaml:2756](specs/atlona-matrix.yaml#L2756))
- [ ] AT-OME-SR21: Status answers x1AVx1 or x2AVx1 (the document's example reads x3Vx1 on a two-input unit) ([specs/atlona-matrix.yaml:2981](specs/atlona-matrix.yaml#L2981))
- [ ] AT-UHD-CLSO-840: an audio route (x6Ax2) is answered x6Ax2, not x6AVx2 as the document's example shows ([specs/atlona-matrix.yaml:646](specs/atlona-matrix.yaml#L646))
- [ ] AT-UHD-CLSO-840: VIN + and - are answered with the new level, and HDCPSet takes the input before the mode (HDCPSet 1 on) ([specs/atlona-matrix.yaml:812](specs/atlona-matrix.yaml#L812))
- [ ] AT-UHD-CLSO-840: Status lists the audio routes (x1Ax1) after the video routes on one line ([specs/atlona-matrix.yaml:2641](specs/atlona-matrix.yaml#L2641))
- [ ] AT-UHD-CLSO-601: PWSTA is answered ON or OFF, not PWON or PWOFF ([specs/atlona-matrix.yaml:1098](specs/atlona-matrix.yaml#L1098))
- [ ] AT-UHD-CLSO-601: microphone ducking is turned on and off with MICon and MICoff and read with MICsta ([specs/atlona-matrix.yaml:1942](specs/atlona-matrix.yaml#L1942))
- [ ] AT-UHD-CLSO-601: the output resolution command is VFmtRes (the syntax line says VidOutRes) and the aspect values are full, overscan, underscan, letterbox, panscan and follow (the example answers fill) ([specs/atlona-matrix.yaml:1716](specs/atlona-matrix.yaml#L1716))
- [ ] AT-UHD-CLSO-601: InputStatus without an input answers one digit per input, and VersionMCU and VersionVSRX answer V followed by the version ([specs/atlona-matrix.yaml:1666](specs/atlona-matrix.yaml#L1666))
- [ ] AT-UHD-CLSO-601 and AT-UHD-CLSO-840: the RS-232 control port's factory line is 115200 8N1 (the documents give the CSpara example only) ([specs/atlona-matrix.yaml:272](specs/atlona-matrix.yaml#L272))

## atlona-ms52w — Atlona AT-OME-MS52W and AT-UHD-SW-510W switchers (Telnet JSON API)

- [ ] Confirm the Telnet port (23) takes the commands with no login prompt, and that OutputMode j makes every JSON answer one line. ([specs/atlona-ms52w.yaml:66](specs/atlona-ms52w.yaml#L66))
- [ ] Record the plain-text answers of Display:Minimal:Get, GetBroadcast and SetBroadcast, and whether Platform:Restart, Reset and Shutdown answer at all. ([specs/atlona-ms52w.yaml:758](specs/atlona-ms52w.yaml#L758))
- [ ] Record what the unit broadcasts on a change (routing, input, volume) besides the moderator event, so rules can follow it. ([specs/atlona-ms52w.yaml:714](specs/atlona-ms52w.yaml#L714))
- [ ] Confirm the parity numbering (0 none, 1 odd, 2 even) and which relay state is closed. ([specs/atlona-ms52w.yaml:766](specs/atlona-ms52w.yaml#L766))
- [ ] AT-UHD-SW-510W: confirm the Telnet port (23), that OutputMode j is taken, and how SetVol sta answers ([specs/atlona-ms52w.yaml:741](specs/atlona-ms52w.yaml#L741))
- [ ] AT-UHD-SW-510W: record what Input:Set and Display:Input:Set answer (the examples set input 1 and answer activeinput 0) ([specs/atlona-ms52w.yaml:270](specs/atlona-ms52w.yaml#L270))

## atlona-sw510w-rest — Atlona AT-UHD-SW-510W (REST API)

- [ ] Confirm the REST API is on port 80 at /API with no login, and that the colons in method= may be percent-encoded. ([specs/atlona-sw510w-rest.yaml:30](specs/atlona-sw510w-rest.yaml#L30))
- [ ] Record which parameters System:MaintenanceTime:Set takes (its table lists hour, minute and day; its example sends one boolean per day and a timezone) and that the Set commands answer as the Get commands do. ([specs/atlona-sw510w-rest.yaml:80](specs/atlona-sw510w-rest.yaml#L80))
- [ ] Record the accepted ranges of the Wi-Fi fence threshold, duration and time threshold, and of the station kick timeout. ([specs/atlona-sw510w-rest.yaml:51](specs/atlona-sw510w-rest.yaml#L51))

## avolites-titan — Avolites Titan

- [ ] Check the status code and body Titan returns for an invalid request (unknown handle, bad parameter), since any 200 counts as success here. ([specs/avolites-titan.yaml:1779](specs/avolites-titan.yaml#L1779))
- [ ] Confirm the handle record field names on Titan 19 (Active or active, Legend or legend, userNumber.hashCode) and that `active` reflects a running playback. ([specs/avolites-titan.yaml:1806](specs/avolites-titan.yaml#L1806))
- [ ] Confirm that times sent with two decimals (fadeTime=5.00) and levels with three (level_level=1.000) are accepted like the documentation's whole numbers. ([specs/avolites-titan.yaml:1814](specs/avolites-titan.yaml#L1814))
- [ ] Check the grand master value range (0-100 as sent here, or 0-1). ([specs/avolites-titan.yaml:1816](specs/avolites-titan.yaml#L1816))
- [ ] Check that an empty panelTimeStamp works for tap tempo and an empty oldValue for the grand master. ([specs/avolites-titan.yaml:1824](specs/avolites-titan.yaml#L1824))
- [ ] Check which licences answer the WebAPI (consoles, Titan PC with T2/T3/TNP, Titan Go) and that T1 and Titan One do not. ([specs/avolites-titan.yaml:1771](specs/avolites-titan.yaml#L1771))
- [ ] Measure the /titan/handles reply size and time on a large show, to judge the 5 s poll. ([specs/avolites-titan.yaml:1801](specs/avolites-titan.yaml#L1801))
- [ ] Find the handle group name of the playback faders for set_group_page (Playbacks, PlaybackWindow or other). ([specs/avolites-titan.yaml:1271](specs/avolites-titan.yaml#L1271))
- [ ] Confirm /titan/handles lists every programmed handle on the console in one answer: each poll replaces the handles state. ([specs/avolites-titan.yaml:2014](specs/avolites-titan.yaml#L2014))
- [ ] Find the wire form of AcwHSI for set_selected_colour_hsi (hsi_hue, hsi_saturation and hsi_intensity assumed) and the ranges of hue, saturation and intensity. ([specs/avolites-titan.yaml:1839](specs/avolites-titan.yaml#L1839))
- [ ] Find the values AcwRecordMask and AcwShapeType take in a query (names, comma-separated names or a number) for the masked clears and clear_effects. ([specs/avolites-titan.yaml:1842](specs/avolites-titan.yaml#L1842))
- [ ] Find the MenuEventTypes names handle_key_action and handle_fader_action take (Press, Release, Move assumed), and whether an empty timeStamp is accepted. ([specs/avolites-titan.yaml:1844](specs/avolites-titan.yaml#L1844))
- [ ] Find the text form of TimecodeTime (hh:mm:ss:ff assumed) for set_timer_start_time and timeline_play_from_time. ([specs/avolites-titan.yaml:1845](specs/avolites-titan.yaml#L1845))
- [ ] Confirm the AcwTimecodeSource names (Internal, UsbExpert, Winamp, System, Smpte) against Titan 19. ([specs/avolites-titan.yaml:1847](specs/avolites-titan.yaml#L1847))
- [ ] Check whether the WebAPI has any read of a playback's or master's current level, for per-handle levels in state. ([specs/avolites-titan.yaml:1868](specs/avolites-titan.yaml#L1868))
- [ ] Confirm a property set takes its value as the POST body text (the introduction's False), for set_palette_number, set_palette_name and the other set_ commands, and the record types set_record_type takes. ([specs/avolites-titan.yaml:1429](specs/avolites-titan.yaml#L1429))
- [ ] Check that store_cue's group and index are a handle's group and handleLocation.index, and what the Int32 it returns is. ([specs/avolites-titan.yaml:1857](specs/avolites-titan.yaml#L1857))

## avproedge-matrix — AVPro Edge AUHD matrices

- [ ] Record the answers to SET and GET commands (assumed to be the command without SET or GET, as GET DHCP answers DHCP 0). ([specs/avproedge-matrix.yaml:74](specs/avproedge-matrix.yaml#L74))
- [ ] Record the lines GET STA prints and confirm it starts with ADDR. ([specs/avproedge-matrix.yaml:130](specs/avproedge-matrix.yaml#L130))
- [ ] Record what GET OUT0 VS and GET IN0 SIG STA answer (assumed a line per output or input). ([specs/avproedge-matrix.yaml:619](specs/avproedge-matrix.yaml#L619))
- [ ] Confirm the CR terminator alone is accepted on Telnet (the manual says to add a return; its examples show CR LF). ([specs/avproedge-matrix.yaml:71](specs/avproedge-matrix.yaml#L71))
- [ ] Check whether the matrix sends status lines unasked after a front-panel change. ([specs/avproedge-matrix.yaml:703](specs/avproedge-matrix.yaml#L703))
- [ ] Confirm the 4x4 GEN2 matrix takes the same command list as the 88. ([specs/avproedge-matrix.yaml:94](specs/avproedge-matrix.yaml#L94))

## avstumpfl-pixera — AV Stumpfl PIXERA

- [ ] Confirm that replies and pushed monEvent messages on a JSON/TCP (dl) access point end with 0xPX, and what setMonitoringHasDelimiter changes (26.1 lists it without a description). ([specs/avstumpfl-pixera.yaml:6725](specs/avstumpfl-pixera.yaml#L6725))
- [ ] Check how a PIXERA before 26.1 answers setMonitoringHasDelimiter (an error reply is assumed) and that monitoring still works there. ([specs/avstumpfl-pixera.yaml:6726](specs/avstumpfl-pixera.yaml#L6726))
- [ ] Confirm that setMonitoringEventMode all pushes timelineTransport, timelinePositions and timelineCountdowns with the same entry shapes as pollMonitoring, with id -1, and how often positions arrive. ([specs/avstumpfl-pixera.yaml:6718](specs/avstumpfl-pixera.yaml#L6718))
- [ ] Check that a reply carries the request's id (1) as "id":1, so that pushed messages (id -1) are not taken as replies. ([specs/avstumpfl-pixera.yaml:104](specs/avstumpfl-pixera.yaml#L104))
- [ ] Check the error object PIXERA sends for an unknown method, wrong parameter names, an unknown timeline or cue name, and an invalid handle. ([specs/avstumpfl-pixera.yaml:427](specs/avstumpfl-pixera.yaml#L427))
- [ ] Check what a name or path containing the text 0xPX does to framing. ([specs/avstumpfl-pixera.yaml:6703](specs/avstumpfl-pixera.yaml#L6703))
- [ ] Confirm the default API port (1400 assumed from AV Stumpfl's examples; none is documented) and whether a disabled API (25.2 default) refuses the connection or answers -32602. ([specs/avstumpfl-pixera.yaml:6695](specs/avstumpfl-pixera.yaml#L6695))
- [ ] Check whether handles stay valid after reloading the project or restarting PIXERA. ([specs/avstumpfl-pixera.yaml:6720](specs/avstumpfl-pixera.yaml#L6720))
- [ ] Confirm the Compound time methods take "name" in 26.1 and "timelineName" before it, and which revision changed it. ([specs/avstumpfl-pixera.yaml:79](specs/avstumpfl-pixera.yaml#L79))
- [ ] Check whether load_project discards unsaved changes silently. ([specs/avstumpfl-pixera.yaml:6731](specs/avstumpfl-pixera.yaml#L6731))
- [ ] Check the format of getCurrentHMSFOfTimeline (hh:mm:ss:ff assumed) and what getCurrentCountdownOfTimeline returns when no cue is ahead. ([specs/avstumpfl-pixera.yaml:651](specs/avstumpfl-pixera.yaml#L651))
- [ ] Confirm the handle-based requests answer as the 26.1 reference shows, and how a setter whose arguments the reference marks optional treats each one left out. ([specs/avstumpfl-pixera.yaml:6778](specs/avstumpfl-pixera.yaml#L6778))
- [ ] Record the units of the arguments the reference does not state (positions, rotations, layer opacity and volume, cue wait and blend durations, clip times) against PIXERA's user interface. ([specs/avstumpfl-pixera.yaml:6782](specs/avstumpfl-pixera.yaml#L6782))
- [ ] Check which handle-based, Session, Resources and LiveSystems methods PIXERA before 26.1 has, to list them for pixera-legacy. ([specs/avstumpfl-pixera.yaml:6791](specs/avstumpfl-pixera.yaml#L6791))
- [ ] Record the words getLiveSystemState, getRemoteSystemState and LiveSystem.getState answer, and the numbers getBackupRole answers. ([specs/avstumpfl-pixera.yaml:6808](specs/avstumpfl-pixera.yaml#L6808))
- [ ] Check how audio master channels are numbered (0 or 1 for the master) for live_system_get_audio_master_volume and mute. ([specs/avstumpfl-pixera.yaml:6805](specs/avstumpfl-pixera.yaml#L6805))
- [ ] Check that replies to polled getters come back compact or spaced as the poll rules read them, with strings escaped as JSON. ([specs/avstumpfl-pixera.yaml:6802](specs/avstumpfl-pixera.yaml#L6802))
- [ ] Confirm what utility_run_js_script answers for a script that throws, and whether it is subject to the allowlist. ([specs/avstumpfl-pixera.yaml:6743](specs/avstumpfl-pixera.yaml#L6743))

## aws-elemental-live — AWS Elemental Live

- [ ] Confirm the REST interface answers over HTTPS on 443 (the guide's examples) and record whether plain HTTP on 80 is served too. ([specs/aws-elemental-live.yaml:66](specs/aws-elemental-live.yaml#L66))
- [ ] Confirm list and detail XML carry an <id> child for each live event and device (the guide says GET XML holds id tags; its list example shows only href). ([specs/aws-elemental-live.yaml:1420](specs/aws-elemental-live.yaml#L1420))
- [ ] Confirm GET /live_events with filter=active lists running, preprocessing and postprocessing events, and that each status document's errors and warnings hold code and message (warnings assumed in the same shape as errors). ([specs/aws-elemental-live.yaml:1457](specs/aws-elemental-live.yaml#L1457))
- [ ] Record what GET /live_events/<id>/priority answers (a <priority> element assumed) and the priority range. ([specs/aws-elemental-live.yaml:1462](specs/aws-elemental-live.yaml#L1462))
- [ ] Record what the action endpoints (start, stop, cancel, archive, reset, mute_audio, adjust_audio_gain, the output and group actions) answer, and their status on success and on refusal. ([specs/aws-elemental-live.yaml:240](specs/aws-elemental-live.yaml#L240))
- [ ] Confirm one-line XML documents (with an XML declaration or without) are accepted by create and update. ([specs/aws-elemental-live.yaml:1347](specs/aws-elemental-live.yaml#L1347))
- [ ] Confirm MPTS start takes an empty <mux></mux> body (the guide gives no body for POST /mpts/<id>/mux). ([specs/aws-elemental-live.yaml:972](specs/aws-elemental-live.yaml#L972))
- [ ] Confirm system_status.json is served under /api like the other endpoints, and that cpu.pct may be a string ("1.46" in the guide's example). ([specs/aws-elemental-live.yaml:74](specs/aws-elemental-live.yaml#L74))
- [ ] Confirm the time signal descriptors element may be empty when there are no descriptors. ([specs/aws-elemental-live.yaml:531](specs/aws-elemental-live.yaml#L531))
- [ ] With user authentication on, check that the X-Auth-Key url is the request path without /api (the hashing page says "without any API version prefix"), and that a request is refused when the encoder's clock and the host's differ by more than the 30 seconds X-Auth-Expires allows. ([specs/aws-elemental-live.yaml:84](specs/aws-elemental-live.yaml#L84))
- [ ] Check how the encoder refuses a wrong or expired X-Auth-Key (401 is assumed, the terminal refusal). ([specs/aws-elemental-live.yaml:90](specs/aws-elemental-live.yaml#L90))
- [ ] Confirm a page of live events holds its live_event elements directly inside the document's root element, and that a page past the last is empty or short, so the core can join the pages. ([specs/aws-elemental-live.yaml:1413](specs/aws-elemental-live.yaml#L1413))

## aws-elemental-medialive — AWS Elemental MediaLive

- [ ] Check a signed request against the live API from the host (not from tests): the signature over the host without :443, content-type, x-amz-date and x-amz-security-token, and a path with an ARN encoded twice in the canonical request. ([specs/aws-elemental-medialive.yaml:62](specs/aws-elemental-medialive.yaml#L62))
- [ ] Confirm that a refused credential is HTTP 403 with a message the refusal_status_body pattern matches, and that a missing permission is a 403 whose message it does not (AccessDeniedException), which fails that command only. ([specs/aws-elemental-medialive.yaml:69](specs/aws-elemental-medialive.yaml#L69))
- [ ] Check the polled lists' shapes against live answers (channels, inputs, multiplexes, inputDevices, inputSecurityGroups) and that the list replaces drop a deleted channel. ([specs/aws-elemental-medialive.yaml:1460](specs/aws-elemental-medialive.yaml#L1460))
- [ ] Confirm that describe, start, stop and delete answer the channel at the top level, and create and update wrapped in channel, as the service model gives them. ([specs/aws-elemental-medialive.yaml:1483](specs/aws-elemental-medialive.yaml#L1483))

## axia-lwrp — Axia Livewire devices

- [ ] Confirm that the client that makes a change also receives its indication (the specification says changes go to every client). ([specs/axia-lwrp.yaml:62](specs/axia-lwrp.yaml#L62))
- [ ] Check what LOGIN without a password does on a device with a password (assumed: read-only, no error) before the password is sent. ([specs/axia-lwrp.yaml:78](specs/axia-lwrp.yaml#L78))
- [ ] Confirm INGN is in tenths of a dB (the specification's text says "10dB units" while its example is -100) and record each node's gain range. ([specs/axia-lwrp.yaml:221](specs/axia-lwrp.yaml#L221))
- [ ] Check that NCHN can be set on SRC and DST, and the channel counts each device accepts. ([specs/axia-lwrp.yaml:235](specs/axia-lwrp.yaml#L235))
- [ ] Confirm that CFG GPO with only a port number is answered with its CFG GPO indication. ([specs/axia-lwrp.yaml:364](specs/axia-lwrp.yaml#L364))
- [ ] Record the IP indication's layout (assumed keyword and value pairs as in the command). ([specs/axia-lwrp.yaml:529](specs/axia-lwrp.yaml#L529))
- [ ] Record the xNode mixer's gain range and the MIX indication sent after a change (assumed the mix's whole point list). ([specs/axia-lwrp.yaml:402](specs/axia-lwrp.yaml#L402))
- [ ] Record a VER indication from current xNodes, the IP-Audio Driver and a GPIO node (quoting of DEVN and SYSV, tags beyond 2.0.2). ([specs/axia-lwrp.yaml:513](specs/axia-lwrp.yaml#L513))

## aximmetry — Aximmetry Composer

- [ ] Record the default HTTP Control port (21463 is the documents' example) and the editions that offer HTTP Control. ([specs/aximmetry.yaml:31](specs/aximmetry.yaml#L31))
- [ ] Check that an empty Document attribute is taken as the running compound, as the documents say of an omitted or empty one. ([specs/aximmetry.yaml:300](specs/aximmetry.yaml#L300))
- [ ] Record the HTTP status of an error response (non-zero Code) and the error codes Aximmetry uses. ([specs/aximmetry.yaml:293](specs/aximmetry.yaml#L293))
- [ ] Check whether XML character references (&amp;, &quot;) are decoded in attribute values, so values holding them could be sent. ([specs/aximmetry.yaml:298](specs/aximmetry.yaml#L298))

## axis-ptz — Axis PTZ cameras and video encoders (VAPIX PTZ API)

- [ ] Check which answer sets the control queue's cookie (queue_request is assumed), its name, that ptz.cgi and ptzqueue.cgi take it back as a Cookie header, and how they answer a client without it. ([specs/axis-ptz.yaml:60](specs/axis-ptz.yaml#L60))
- [ ] Record the reply to ptz.cgi?query=mode on a Panopsis dome (the document does not show its form). ([specs/axis-ptz.yaml:813](specs/axis-ptz.yaml#L813))
- [ ] Confirm that a ptz.cgi argument the channel's driver does not support is answered 200 with "Error:" rather than 204. ([specs/axis-ptz.yaml:772](specs/axis-ptz.yaml#L772))
- [ ] Record a query=presetposcam reply with no presets, and one on a multichannel encoder, to confirm the "Preset Positions for camera N" heading and presetposnoN=name lines. ([specs/axis-ptz.yaml:706](specs/axis-ptz.yaml#L706))
- [ ] Confirm that param.cgi?action=list&group=PTZ answers root.PTZ.<name>=<value> lines, and which UserAdv.U# group number a channel's MoveSpeed and dome settings use. ([specs/axis-ptz.yaml:721](specs/axis-ptz.yaml#L721))

## barco-encore — Barco Encore Presentation System

- [ ] Confirm the original Encore controller takes ASCII control on TCP port 3000, with commands ended by a line feed, and record whatever it answers. ([specs/barco-encore.yaml:88](specs/barco-encore.yaml#L88))
- [ ] Check the unit of the ATRN argument (frames or tenths of a second), whether ATRN alone is accepted, and the range of PRESET R preset numbers. ([specs/barco-encore.yaml:99](specs/barco-encore.yaml#L99))
- [ ] Find the syntax of the functions third-party software exposes beyond ATRN and PRESET R (manual transition, preset save and delete, preset names, current preset), and whether a Barco protocol document exists. ([specs/barco-encore.yaml:88](specs/barco-encore.yaml#L88))
- [ ] Check whether the controller accepts more than one TCP session at a time. ([specs/barco-encore.yaml:107](specs/barco-encore.yaml#L107))

## barco-eventmaster — Barco Event Master

- [ ] Confirm JSON-RPC requests are accepted at the root path / on port 9999, since the documents give no request path. ([specs/barco-eventmaster.yaml:1396](specs/barco-eventmaster.yaml#L1396))
- [ ] Check which key activateDestGroup by name accepts, destGrpName (sent) or destGrName. ([specs/barco-eventmaster.yaml:1446](specs/barco-eventmaster.yaml#L1446))
- [ ] Confirm the user key list method is listUserKeys (not listUserKey), what it returns, and that recallUserKey accepts the key userkeyName. ([specs/barco-eventmaster.yaml:1453](specs/barco-eventmaster.yaml#L1453))
- [ ] Record the actual reply shape of powerStatus and of listCues, which the documents leave open. ([specs/barco-eventmaster.yaml:1462](specs/barco-eventmaster.yaml#L1462))
- [ ] Check which test pattern numbers exist beyond 0 (off), 3 (colour bars) and 5 (grid). ([specs/barco-eventmaster.yaml:1465](specs/barco-eventmaster.yaml#L1465))
- [ ] Check what cue_transport (pause or stop with type alone and no cue id) applies to. ([specs/barco-eventmaster.yaml:1550](specs/barco-eventmaster.yaml#L1550))
- [ ] On Encore3, check what the ApiExtControl flag in getFrameSettings means and whether it blocks API calls. ([specs/barco-eventmaster.yaml:1521](specs/barco-eventmaster.yaml#L1521))
- [ ] Check what error codes and messages come back in result.success and the JSON-RPC error member, since none are documented. ([specs/barco-eventmaster.yaml:1412](specs/barco-eventmaster.yaml#L1412))
- [ ] Confirm the subscribe command is accepted with the port sent as a string. ([specs/barco-eventmaster.yaml:1500](specs/barco-eventmaster.yaml#L1500))
- [ ] Confirm getFrameSettings lists every slot of the frame, and listSources, listInputs, listStill and listOperators with no parameters list every one: each such answer replaces cards, sources, inputs, stills or operators. ([specs/barco-eventmaster.yaml:1295](specs/barco-eventmaster.yaml#L1295))
- [ ] Confirm reading listContent for every screen destination every 10 seconds (after listDestinations) is acceptable on a processor with many screens, and that listContent's Layers, BGLyr and Transition arrays match the Doc 8.2 example on E2, S3, EX and Encore3. ([specs/barco-eventmaster.yaml:1175](specs/barco-eventmaster.yaml#L1175))
- [ ] Record a listCues reply: the documents give no cue object, and the cue's id and Name are read as the other lists hold them. ([specs/barco-eventmaster.yaml:1248](specs/barco-eventmaster.yaml#L1248))

## barco-eventmaster-telnet — Barco Event Master (ASCII over Telnet)

- [ ] Record what the processor sends on port 9878 (the prompt, any answer to ATRN and PRESET, any error text), which Barco does not document. ([specs/barco-eventmaster-telnet.yaml:130](specs/barco-eventmaster-telnet.yaml#L130))
- [ ] Confirm commands ended with CR LF are accepted (and whether CR or LF alone is too). ([specs/barco-eventmaster-telnet.yaml:140](specs/barco-eventmaster-telnet.yaml#L140))
- [ ] Check how PRESET numbers 1-1000 map to the JSON API's preset ids and serial numbers, and whether PRESET -s overwrites an existing preset. ([specs/barco-eventmaster-telnet.yaml:148](specs/barco-eventmaster-telnet.yaml#L148))
- [ ] Check which software versions accept PRESET -a. ([specs/barco-eventmaster-telnet.yaml:156](specs/barco-eventmaster-telnet.yaml#L156))
- [ ] Confirm the Encore3 opens port 9878 and accepts ATRN and PRESET. ([specs/barco-eventmaster-telnet.yaml:165](specs/barco-eventmaster-telnet.yaml#L165))

## barco-projection-protocol — Barco projectors (Barco Projection Protocol)

- [ ] The serial baud rate default (9600 assumed; the catalogs leave it to the owner's manual) on HDX, PGWU and older models. ([specs/barco-projection-protocol.yaml:59](specs/barco-projection-protocol.yaml#L59))
- [ ] Default serial address 1, and that the address byte is escaped like the rest of the message (addresses 0x80, 0xFE, 0xFF). ([specs/barco-projection-protocol.yaml:55](specs/barco-projection-protocol.yaml#L55))
- [ ] A projector keeps an idle TCP connection on 43680 open, with the 10 s status read as its only traffic. ([specs/barco-projection-protocol.yaml:71](specs/barco-projection-protocol.yaml#L71))
- [ ] Read responses repeat the request's bytes, data included, before the values; read projector status (0x67) does not repeat its optional mask. ([specs/barco-projection-protocol.yaml:1168](specs/barco-projection-protocol.yaml#L1168))
- [ ] A read a family refuses (NACK) stays refused, so dropping it from polling is right. ([specs/barco-projection-protocol.yaml:2783](specs/barco-projection-protocol.yaml#L2783))
- [ ] HDX "increment blanking top" is 0x22 0x4C (sent), not the catalog's 0x22 0x45. ([specs/barco-projection-protocol.yaml:2253](specs/barco-projection-protocol.yaml#L2253))
- [ ] HDX warp floats are IEEE-754 big-endian. ([specs/barco-projection-protocol.yaml:2124](specs/barco-projection-protocol.yaml#L2124))
- [ ] HDX warp area reads answer with the area echoed, and firmware before 1.6 refuses the area byte. ([specs/barco-projection-protocol.yaml:1261](specs/barco-projection-protocol.yaml#L1261))
- [ ] HDX write DMX universe uses 0x57 0x45 (the same bytes as the read) as the catalog gives it. ([specs/barco-projection-protocol.yaml:1708](specs/barco-projection-protocol.yaml#L1708))
- [ ] HDX write network/wifi configuration takes the MAC address field the catalog lays out. ([specs/barco-projection-protocol.yaml:1886](specs/barco-projection-protocol.yaml#L1886))
- [ ] HDX warp status enable bit field: which bit is which kind. ([specs/barco-projection-protocol.yaml:2178](specs/barco-projection-protocol.yaml#L2178))
- [ ] HDX get source extended (0x34 0x00 0x05) answers six bytes: source, module mode, type, configurations 1-3. ([specs/barco-projection-protocol.yaml:1226](specs/barco-projection-protocol.yaml#L1226))
- [ ] HDX get output window parameters 0x21 0x8D 0x00 0x06 answers four words, x, y, width and height. ([specs/barco-projection-protocol.yaml:1099](specs/barco-projection-protocol.yaml#L1099))
- [ ] HDX no-signal shutdown delay response is 0x21 0x9A 0x02 0x02 then a double word. ([specs/barco-projection-protocol.yaml:1088](specs/barco-projection-protocol.yaml#L1088))
- [ ] HDX get lock with 0xFD answers the mode and the manual refresh rate in 1/10000 Hz. ([specs/barco-projection-protocol.yaml:1073](specs/barco-projection-protocol.yaml#L1073))
- [ ] HDX scan read: orientation in the upper nibble, auto front/rear in the lower. ([specs/barco-projection-protocol.yaml:1096](specs/barco-projection-protocol.yaml#L1096))
- [ ] HDX set color temperature by byte (0x00 custom, 0x01 projector-white, 0x32, 0x54, 0x65, 0x93). ([specs/barco-projection-protocol.yaml:1641](specs/barco-projection-protocol.yaml#L1641))
- [ ] P7 coordinates are words of value x 10000. ([specs/barco-projection-protocol.yaml:1947](specs/barco-projection-protocol.yaml#L1947))
- [ ] The PGWX-61B and PGXG-61B speak the PGWU-61B catalog. ([specs/barco-projection-protocol.yaml:260](specs/barco-projection-protocol.yaml#L260))
- [ ] PGWU power on/off is 0x58 0x03 / 0x58 0x00. ([specs/barco-projection-protocol.yaml:1380](specs/barco-projection-protocol.yaml#L1380))
- [ ] PGWU about info layout: pixel clock and horizontal rate as an integer byte plus 256ths, firmware as six bytes. ([specs/barco-projection-protocol.yaml:731](specs/barco-projection-protocol.yaml#L731))
- [ ] PGWU get versions data indexes (the catalog's from/to tables disagree). ([specs/barco-projection-protocol.yaml:1241](specs/barco-projection-protocol.yaml#L1241))
- [ ] Older models: vertical refresh write synchronous 0x26 0x61 and asynchronous 0x27 0x61 (pages) rather than the appendix's reverse. ([specs/barco-projection-protocol.yaml:2109](specs/barco-projection-protocol.yaml#L2109))
- [ ] Older models: which models take a 1-byte phase and which a word. ([specs/barco-projection-protocol.yaml:1958](specs/barco-projection-protocol.yaml#L1958))
- [ ] Lamp history run time is a little-endian word after each C-string serial. ([specs/barco-projection-protocol.yaml:982](specs/barco-projection-protocol.yaml#L982))
- [ ] DP100/XLM diagnosis 3 for indexes 0-87 in one request, temperatures as signed bytes. ([specs/barco-projection-protocol.yaml:926](specs/barco-projection-protocol.yaml#L926))
- [ ] DP100/XLM main version is read with version read 0x60 0x00 0x01. ([specs/barco-projection-protocol.yaml:1070](specs/barco-projection-protocol.yaml#L1070))
- [ ] Lamp power and nominal current: length and unit. ([specs/barco-projection-protocol.yaml:1004](specs/barco-projection-protocol.yaml#L1004))
- [ ] Older models' color balance read with a preset byte echoes the preset. ([specs/barco-projection-protocol.yaml:815](specs/barco-projection-protocol.yaml#L815))
- [ ] Which older RS-232 commands each product line (Galaxy, Helios, ELM, SLM, BarcoData/Graphics/Reality/Vision) answers; the catalog does not say per product. ([specs/barco-projection-protocol.yaml:304](specs/barco-projection-protocol.yaml#L304))
- [ ] Lens commands move one step per message. ([specs/barco-projection-protocol.yaml:1331](specs/barco-projection-protocol.yaml#L1331))

## barco-pulse — Barco Pulse projectors

- [ ] Messages arrive as bare JSON objects with no line ending or separator, and a property.changed or signal.callback push can arrive between a request and its reply. ([specs/barco-pulse.yaml:133](specs/barco-pulse.yaml#L133))
- [ ] Every reply holds "result" or "error" and no push holds either key, so a push is never taken as a command's reply. ([specs/barco-pulse.yaml:133](specs/barco-pulse.yaml#L133))
- [ ] Replies come back in request order when every request carries "id":1 and requests go one at a time. ([specs/barco-pulse.yaml:134](specs/barco-pulse.yaml#L134))
- [ ] The Pulse API answers on TLS port 9091 with a self-signed certificate, from which firmware version, and with the same JSON-RPC stream and access level as 9090. ([specs/barco-pulse.yaml:126](specs/barco-pulse.yaml#L126))
- [ ] On Pulse 2.10 and later, port 9090 refuses connections until protocols.jsonrpc.port-raw.enable is turned on (LCD/OSD, Prospector or the API over 9091), and a network-domain factory reset turns it off again. ([specs/barco-pulse.yaml:4926](specs/barco-pulse.yaml#L4926))
- [ ] authenticate with a correct PIN code answers result true, and what a wrong code answers (false, or a JSON-RPC error). ([specs/barco-pulse.yaml:156](specs/barco-pulse.yaml#L156))
- [ ] An unauthenticated end-user connection on 9090 or 9091 can read, set and call everything this spec uses, on Pulse 2.10 with user.authenticationrequired on. ([specs/barco-pulse.yaml:4949](specs/barco-pulse.yaml#L4949))
- [ ] property.get of one property name (string form) answers the bare value in "result" for strings, numbers, booleans and objects. ([specs/barco-pulse.yaml:6303](specs/barco-pulse.yaml#L6303))
- [ ] property.subscribe with an array that names a property missing or unavailable in the current state either subscribes the rest or fails as a whole. ([specs/barco-pulse.yaml:5094](specs/barco-pulse.yaml#L5094))
- [ ] Subscriptions survive a system.state change (standby to on) or must be made again; this spec re-subscribes on every state change and on modelupdated. ([specs/barco-pulse.yaml:6370](specs/barco-pulse.yaml#L6370))
- [ ] property.changed lists each changed property as a single-member object in params.property, several in one push included. ([specs/barco-pulse.yaml:6298](specs/barco-pulse.yaml#L6298))
- [ ] Properties the catalogs type as {base int or float, hints} (optics.*.target, illumination.sources.*.power, image.stereo.darktime, image.processing.transportdelay.desired) are set and pushed as plain numbers. ([specs/barco-pulse.yaml:4985](specs/barco-pulse.yaml#L4985))
- [ ] A motor target outside optics.<axis>.minposition..maxposition is refused with an error rather than clamped. ([specs/barco-pulse.yaml:2642](specs/barco-pulse.yaml#L2642))
- [ ] environment.getcontrolblocks Sensor/Temperature and Sensor/Speed answer a dictionary whose keys start with "environment." on every family. ([specs/barco-pulse.yaml:6353](specs/barco-pulse.yaml#L6353))
- [ ] statistics.listcounters answers objects of name, plain-integer value and unit. ([specs/barco-pulse.yaml:6359](specs/barco-pulse.yaml#L6359))
- [ ] The arguments the notification.emitted/dismissed and profile.profileactivated signals carry (the catalogs do not document them). ([specs/barco-pulse.yaml:6364](specs/barco-pulse.yaml#L6364))
- [ ] Introspecting image confirms there is no freeze property or method on any family. ([specs/barco-pulse.yaml:4962](specs/barco-pulse.yaml#L4962))
- [ ] Which properties and methods answer -32000 in standby (shutter, optics, illumination), and that the re-reads after the state change fill them in. ([specs/barco-pulse.yaml:4973](specs/barco-pulse.yaml#L4973))
- [ ] The connector and source object names the catalogs list (hdmi, displayport1, l1hdmi, l2displayporta ...) match real units of each family. ([specs/barco-pulse.yaml:4993](specs/barco-pulse.yaml#L4993))
- [ ] power_on does nothing from ECO over the network, and the API stops answering in ECO. ([specs/barco-pulse.yaml:5010](specs/barco-pulse.yaml#L5010))
- [ ] The RS232 port speaks the same undelimited JSON-RPC stream as TCP 9090 (link serial). ([specs/barco-pulse.yaml:5017](specs/barco-pulse.yaml#L5017))
- [ ] GET /api/image/processing/warp/file/transfer answers the current warp grid XML, and which HTTP authentication Pulse 2.10 asks for on the file endpoints. ([specs/barco-pulse.yaml:5027](specs/barco-pulse.yaml#L5027))
- [ ] show_test_pattern: image.testpattern.selected takes the pattern id from image.testpattern.list and image.testpattern.show true then shows it. ([specs/barco-pulse.yaml:3647](specs/barco-pulse.yaml#L3647))
- [ ] activate_profile and activate_preset apply the stored settings live, and their result is true on success. ([specs/barco-pulse.yaml:4493](specs/barco-pulse.yaml#L4493))
- [ ] reset_all answers the list of domains it resets, and whether it resets the network and user domains, which can cut the connection. ([specs/barco-pulse.yaml:3544](specs/barco-pulse.yaml#L3544))
- [ ] Confirm the UDM, F70, F80, F90, HDX 4K and XDL serve the file endpoints the I600 and UDX catalogs list (logs, EDID, black level, blend, warp and test pattern files, license registration, LCD and OSD captures, splash screen upload), and which of them also serve the UDX's source image and camera downloads. ([specs/barco-pulse.yaml:4759](specs/barco-pulse.yaml#L4759))
- [ ] Confirm what the file endpoints answer to a multipart upload (status and body), and that the part's file name may be any name with the right extension. ([specs/barco-pulse.yaml:4850](specs/barco-pulse.yaml#L4850))

## barco-rlm-w14 — Barco RLM-W14

- [ ] The echo is lower case and ends at CR, and answers come as "OP <COMMAND> = <value>" lines (line ending unknown). ([specs/barco-rlm-w14.yaml:29](specs/barco-rlm-w14.yaml#L29))
- [ ] Commands the table capitalizes (Bright, Tint, Lens.center, Osdlite, CS_cust_*, DarkTm.sel) are accepted in lower case. ([specs/barco-rlm-w14.yaml:1087](specs/barco-rlm-w14.yaml#L1087))
- [ ] A step (+/-) answers with the new value. ([specs/barco-rlm-w14.yaml:1810](specs/barco-rlm-w14.yaml#L1810))
- [ ] Execute functions answer "OP <COMMAND>". ([specs/barco-rlm-w14.yaml:154](specs/barco-rlm-w14.yaml#L154))
- [ ] What motor commands answer (OP ZOOMIO, OP FOCUS, OP VERT.OFFSET ...). ([specs/barco-rlm-w14.yaml:138](specs/barco-rlm-w14.yaml#L138))
- [ ] What lens memory commands answer. ([specs/barco-rlm-w14.yaml:852](specs/barco-rlm-w14.yaml#L852))
- [ ] P7 custom coordinates must be sent as three digits. ([specs/barco-rlm-w14.yaml:1382](specs/barco-rlm-w14.yaml#L1382))
- [ ] Blend top/bottom 0-500 and left/right 0-800 (ranges reconstructed from the table layout). ([specs/barco-rlm-w14.yaml:1059](specs/barco-rlm-w14.yaml#L1059))
- [ ] Black level width top/bottom in steps of 8 and left/right in steps of 4 (reconstructed from the table layout). ([specs/barco-rlm-w14.yaml:1003](specs/barco-rlm-w14.yaml#L1003))
- [ ] The scen.blue range (0-32) is assumed, and scen.all reads NA when red, green and blue differ. ([specs/barco-rlm-w14.yaml:954](specs/barco-rlm-w14.yaml#L954))
- [ ] pip.sel uses 1-based source numbers, unlike input.sel. ([specs/barco-rlm-w14.yaml:1452](specs/barco-rlm-w14.yaml#L1452))
- [ ] The text format of lamp hours, pixel clock, refresh rates and run time. ([specs/barco-rlm-w14.yaml:458](specs/barco-rlm-w14.yaml#L458))
- [ ] Error codes 39 and 41-50 (the catalog elides them). ([specs/barco-rlm-w14.yaml:378](specs/barco-rlm-w14.yaml#L378))
- [ ] What scen.adl and w2.recover do. ([specs/barco-rlm-w14.yaml:1515](specs/barco-rlm-w14.yaml#L1515))
- [ ] Unit of warp rotation -20 to +20. ([specs/barco-rlm-w14.yaml:1642](specs/barco-rlm-w14.yaml#L1642))
- [ ] The effect of control_port network (marked obsolete) and eco_network_power on RS-232 control. ([specs/barco-rlm-w14.yaml:1130](specs/barco-rlm-w14.yaml#L1130))
- [ ] Whether the projector has a network (TCP) form of these commands. ([specs/barco-rlm-w14.yaml:2563](specs/barco-rlm-w14.yaml#L2563))

## behringer-wing — Behringer WING

- [ ] Confirm that solo, talkback on, monitor level, solo dim and mono, input gain and phantom ($-prefixed parameters) can be set over OSC. ([specs/behringer-wing.yaml:8509](specs/behringer-wing.yaml#L8509))
- [ ] Check what gain and phantom sets do on a strip whose input source has no preamp. ([specs/behringer-wing.yaml:8516](specs/behringer-wing.yaml#L8516))
- [ ] Find the separator between tags in a strip's tag string, so DCA assignment does not damage other tags. ([specs/behringer-wing.yaml:8533](specs/behringer-wing.yaml#L8533))
- [ ] Find the tag names used for mute group assignment, which are not documented. ([specs/behringer-wing.yaml:8536](specs/behringer-wing.yaml#L8536))
- [ ] Record the reply of /$ctl/lib ,s ?, /play ,s ? and /cards/wlive/1/$stat ,s ? with several scenes, songs and sessions, to see how a list is laid out in a node description (for a rule that keeps the whole list). ([specs/behringer-wing.yaml:3739](specs/behringer-wing.yaml#L3739))
- [ ] Check that the node queries ,s ?, ,s # and ,s * are answered on the node's own address, as the /fx/1 examples show. ([specs/behringer-wing.yaml:3739](specs/behringer-wing.yaml#L3739))
- [ ] Check what GONEXT and GOPREV do, since they are listed without description. ([specs/behringer-wing.yaml:8577](specs/behringer-wing.yaml#L8577))
- [ ] Check whether a set sent while /$ctl/OSC/ronly is on returns an error. ([specs/behringer-wing.yaml:8595](specs/behringer-wing.yaml#L8595))
- [ ] Check which models allow set_monitor_level (settable on Compact and Rack, read-only on the full-size WING). ([specs/behringer-wing.yaml:8564](specs/behringer-wing.yaml#L8564))
- [ ] On WING Rack, check whether main sends 5-8 (the headphone outputs) are reachable. ([specs/behringer-wing.yaml:8557](specs/behringer-wing.yaml#L8557))
- [ ] Confirm the 9-second /*s renewal keeps the subscription alive and the connection open, and that another subscribing client takes changes away as described. ([specs/behringer-wing.yaml:8459](specs/behringer-wing.yaml#L8459))
- [ ] Confirm set_node is answered on the node's address followed by * (/* for the root) with OK or one of the documented error strings, and that get_node's reply lists the node's children. ([specs/behringer-wing.yaml:8633](specs/behringer-wing.yaml#L8633))
- [ ] Check on which address the console answers a query sent with a native hash (/#f50f69f8), which decides the key the params state uses for it, and whether a set of a [RO] parameter returns an error. ([specs/behringer-wing.yaml:8618](specs/behringer-wing.yaml#L8618))
- [ ] Check what the console does with STD-model EQ addresses (lf, 1q, heq ...) while another EQ model is loaded: ignored, an error, or applied to the hidden STD settings. ([specs/behringer-wing.yaml:8645](specs/behringer-wing.yaml#L8645))
- [ ] Confirm the EQ mix range per strip (the document gives 0..125 for channels, auxes and matrices and 0..100 with 126 steps for buses and mains). ([specs/behringer-wing.yaml:8643](specs/behringer-wing.yaml#L8643))
- [ ] Confirm the compressor ratio's type per strip: a list of texts on channels, a float 1.1-100 on buses, mains and matrices, as the document gives them. ([specs/behringer-wing.yaml:8660](specs/behringer-wing.yaml#L8660))
- [ ] Confirm the gate side-chain source's first value is SELF (the document's table says SHELF). ([specs/behringer-wing.yaml:8671](specs/behringer-wing.yaml#L8671))
- [ ] Confirm the aux input dynamics parameters (thr -36..12, depth, fast, peak, ingain, cpeak, cmode) and that an aux has no dynamics model choice. ([specs/behringer-wing.yaml:8665](specs/behringer-wing.yaml#L8665))
- [ ] Check that a numbered effect parameter takes a float in its own unit and an enumeration by its text, and how the console answers a number out of range. ([specs/behringer-wing.yaml:8679](specs/behringer-wing.yaml#L8679))
- [ ] Check what slots 9-16 do with a model only slots 1-8 take. ([specs/behringer-wing.yaml:8685](specs/behringer-wing.yaml#L8685))
- [ ] Check how the console keeps a delay value outside the range of the selected delay unit (for example 400 with mode M, whose range is 0-150 m). ([specs/behringer-wing.yaml:8694](specs/behringer-wing.yaml#L8694))
- [ ] Confirm the preamp remote parameters' types, read from misaligned table columns: rmt OFF/AES A/AES B/AES C, rcvc 0/1, $ha an int, $ract 0/1, $rdest text. ([specs/behringer-wing.yaml:8704](specs/behringer-wing.yaml#L8704))
- [ ] Check how the console answers a query or set for an input or output number past its group's size, and whether CRD and MOD inputs and outputs answer with no card or module fitted (each unanswered query costs the timeout on connecting). ([specs/behringer-wing.yaml:8704](specs/behringer-wing.yaml#L8704))
- [ ] Check whether patching an output with set_output_source (group, then number) is heard as a glitch between the two messages. ([specs/behringer-wing.yaml:8722](specs/behringer-wing.yaml#L8722))
- [ ] Confirm that user inputs 25-56 are the user patches the console numbers 1-32, and that a user signal's tap and side apply only to inputs 1-24. ([specs/behringer-wing.yaml:8722](specs/behringer-wing.yaml#L8722))
- [ ] Confirm the monitor bus dim and PFL dim ranges are -40 to 0 dB (the document writes 40..0) and that the monitor delay is in metres only. ([specs/behringer-wing.yaml:8740](specs/behringer-wing.yaml#L8740))
- [ ] Confirm the source solo enable values (/cfg/solo/srcsolo: OFF, CH39, AUX7 as documented) and the source solo group numbering 1-13 in input-group order. ([specs/behringer-wing.yaml:8740](specs/behringer-wing.yaml#L8740))
- [ ] Confirm $sololed is a 0/1 int on every strip type and /$stat/sip a 0/1 int (the document's columns are misaligned for aux, bus, main and matrix). ([specs/behringer-wing.yaml:8740](specs/behringer-wing.yaml#L8740))
- [ ] Check what the RTA and EQ-RTA range does with a value other than 30 or 60 dB. ([specs/behringer-wing.yaml:8740](specs/behringer-wing.yaml#L8740))
- [ ] Confirm that apply_network_settings ($ipapply ,i 1) applies the network mode, address, mask and gateway, and what the console does with a set of ipN, mskN or gwN before it. ([specs/behringer-wing.yaml:8757](specs/behringer-wing.yaml#L8757))
- [ ] Confirm the clock rate is set as the strings 48000.0 / 44100.0 (the ae_globals snapshot shows a number). ([specs/behringer-wing.yaml:8768](specs/behringer-wing.yaml#L8768))
- [ ] Confirm save_console_data ($savenow ,i 1) saves at once, and that $noautosave 0 means autosave on. ([specs/behringer-wing.yaml:8777](specs/behringer-wing.yaml#L8777))
- [ ] Confirm muteovr and autosel_* live under /$ctl/cfg (the table writes /$globals/muteovr and /$ctl/cgf/autosel_*), and that the EQ RTA size value is spelled Off rather than OFF. ([specs/behringer-wing.yaml:8788](specs/behringer-wing.yaml#L8788))
- [ ] Confirm /$ctl/$stat/selidx is reported 0-75 while set 1-76, and what /$ctl/$stat/sof reports. ([specs/behringer-wing.yaml:8788](specs/behringer-wing.yaml#L8788))
- [ ] Check whether /$syscfg/tcplock also blocks OSC sets over UDP. ([specs/behringer-wing.yaml:8805](specs/behringer-wing.yaml#L8805))
- [ ] Check how often the subscription pushes /$stat/time and /$stat/date. ([specs/behringer-wing.yaml:8805](specs/behringer-wing.yaml#L8805))
- [ ] Confirm the USB player's position and length are seconds and that usb_player_seek ($pos) moves the play position. ([specs/behringer-wing.yaml:8822](specs/behringer-wing.yaml#L8822))
- [ ] Confirm that /play/$actionidx selects a playlist entry, and whether playing it needs a following PLAY. ([specs/behringer-wing.yaml:8822](specs/behringer-wing.yaml#L8822))
- [ ] Check whether a W-Live card's sessionpos, markerpos, start and stop are settable (the document does not mark them read-only) and what setting them does. ([specs/behringer-wing.yaml:8822](specs/behringer-wing.yaml#L8822))
- [ ] Confirm the W-Live time values (etime, sdfree, sessionlen, start, stop) are milliseconds. ([specs/behringer-wing.yaml:8822](specs/behringer-wing.yaml#L8822))
- [ ] Confirm that once /$ctl/OSC/ronly is on, an OSC set cannot switch it off again. ([specs/behringer-wing.yaml:8841](specs/behringer-wing.yaml#L8841))
- [ ] Check whether the console answers a query for an address the model or a control's function lacks (custom-control parameters, the other models' layer sections and pages); an unanswered query costs the timeout on connecting. ([specs/behringer-wing.yaml:8849](specs/behringer-wing.yaml#L8849))
- [ ] Confirm the custom-control page select is set 1-16 and reported 0-15, and that the Compact's pages U1-U4, MM and D1-D4 have no lower row or encoder. ([specs/behringer-wing.yaml:8849](specs/behringer-wing.yaml#L8849))
- [ ] Confirm the layer counts (left 7, centre and right 9, Compact 9, Rack 5), which layers are User 1 and User 2 on each, and whether the Rack has user layers. ([specs/behringer-wing.yaml:8867](specs/behringer-wing.yaml#L8867))
- [ ] Check that a user layer's fader position takes type, index and destination as set_layer_strip sends them, and what select_layer does with 8 and 9. ([specs/behringer-wing.yaml:8867](specs/behringer-wing.yaml#L8867))
- [ ] Check what set_daw_preset does: record the name only, or load the DAW preset. ([specs/behringer-wing.yaml:8884](specs/behringer-wing.yaml#L8884))
- [ ] Confirm GPIO gpstate is the switchable state and $state the read state, and that set_gpio_state drives an output GPIO. ([specs/behringer-wing.yaml:8884](specs/behringer-wing.yaml#L8884))
- [ ] Capture the /$ctl/lib ,s ? reply with a show open and check that $scenes is given as `$scenes list [name, name, ...]`, what separates the node's entries (the document prints it as ~), and how a scene name holding a comma or a bracket is written. ([specs/behringer-wing.yaml:34225](specs/behringer-wing.yaml#L34225))
- [ ] Capture the /play ,s ? reply with a playlist open and check that $songs is listed as `$songs list [...]`. ([specs/behringer-wing.yaml:35242](specs/behringer-wing.yaml#L35242))
- [ ] Capture a W-Live card's /cards/wlive/N/$stat ,s ? reply and check that sessionlist and snamelist are listed as `list [...]`, in the same order. ([specs/behringer-wing.yaml:35332](specs/behringer-wing.yaml#L35332))
- [ ] Check how many meter words each channel, aux, bus, main and matrix sends (eight are assumed: input and output left and right, gate key and gain, dynamics key and gain) and each DCA (four), and their order. ([specs/behringer-wing.yaml:8445](specs/behringer-wing.yaml#L8445))
- [ ] Check that the console sends meter blocks to the UDP port named in the request from its own address, so a shared port routed by source address receives them, and that renewing the report id every 4 s keeps them coming over a TCP connection otherwise idle. ([specs/behringer-wing.yaml:42](specs/behringer-wing.yaml#L42))
- [ ] Check the scale of the gate and dynamics gain meters: the document says 1.0 (256) is 20 dB of reduction for most models and 60 dB for the standard gate. ([specs/behringer-wing.yaml:8936](specs/behringer-wing.yaml#L8936))

## behringer-x32 — Behringer X32 / Midas M32

- [ ] Check dB rounding on get: set a range of fader and send levels in dB, read them back, and record how far the console's rounding to its nearest step moves them. ([specs/behringer-x32.yaml:17389](specs/behringer-x32.yaml#L17389))
- [ ] Check whether recall_scene, recall_snippet and recall_cue produce any reply. ([specs/behringer-x32.yaml:17343](specs/behringer-x32.yaml#L17343))
- [ ] Check how long after the /load reply the load actually completes. ([specs/behringer-x32.yaml:17347](specs/behringer-x32.yaml#L17347))
- [ ] Check what the console does when an empty scene, snippet or cue slot is recalled over OSC. ([specs/behringer-x32.yaml:17351](specs/behringer-x32.yaml#L17351))
- [ ] Confirm a recall produces /xremote updates for every changed parameter. ([specs/behringer-x32.yaml:17350](specs/behringer-x32.yaml#L17350))
- [ ] Check how many local headamps each model has within the 1-32 range. ([specs/behringer-x32.yaml:17409](specs/behringer-x32.yaml#L17409))
- [ ] Check that /-ha/NN/index answers a query (it is read-only), whether /xremote pushes it when routing changes, and that it reports -1 for a card or other source without a headamp. ([specs/behringer-x32.yaml:15920](specs/behringer-x32.yaml#L15920))
- [ ] Check what a type above 33 sent to effect slots 5-8 does (passes validation, presumably ignored). ([specs/behringer-x32.yaml:17430](specs/behringer-x32.yaml#L17430))
- [ ] Find which bit of the 18-bit talkback destination bitmap is which destination. ([specs/behringer-x32.yaml:17460](specs/behringer-x32.yaml#L17460))
- [ ] Check the user-control button numbering 5-12 on Compact and Producer. ([specs/behringer-x32.yaml:17444](specs/behringer-x32.yaml#L17444))
- [ ] Confirm /node is answered on the address node (without the leading /) and that a / set is echoed back to the client that sent it, as get_node and set_node expect. ([specs/behringer-x32.yaml:17480](specs/behringer-x32.yaml#L17480))
- [ ] Check whether /xremote pushes any parameter with more than one argument, which the params rules do not keep. ([specs/behringer-x32.yaml:17466](specs/behringer-x32.yaml#L17466))
- [ ] Confirm an EQ band Q of 0.0 on the wire is Q 10 (narrow) and 1.0 is Q 0.3 (wide), the document giving the range as [10.000, 0.3, 72]. ([specs/behringer-x32.yaml:17494](specs/behringer-x32.yaml#L17494))
- [ ] Check what a matrix or main EQ does with bands 2 and 5 when a crossover type (6-13) is selected, which the document says can make them unavailable. ([specs/behringer-x32.yaml:17500](specs/behringer-x32.yaml#L17500))
- [ ] Confirm the X32 gate mode order 0 EXP2, 1 EXP3, 2 EXP4, 3 GATE, 4 DUCK as the document lists it (the X AIR community list orders them differently). ([specs/behringer-x32.yaml:4401](specs/behringer-x32.yaml#L4401))
- [ ] Confirm the dynamics and gate wire ranges (threshold, knee, makeup gain, attack, hold, release, mix, gate range) map as the document's linear and logarithmic laws state. ([specs/behringer-x32.yaml:17505](specs/behringer-x32.yaml#L17505))
- [ ] Confirm /ch/NN/preamp/hpon switches the channel's low-cut filter, not phantom power as the document's description of it says. ([specs/behringer-x32.yaml:17533](specs/behringer-x32.yaml#L17533))
- [ ] Check that FX return sends to bus 01 have pan, type and panFollow like the other inputs (the document's list starts at 03). ([specs/behringer-x32.yaml:17521](specs/behringer-x32.yaml#L17521))
- [ ] Check what the console does with pan, type or panFollow sent to an even send number, which the commands refuse. ([specs/behringer-x32.yaml:17518](specs/behringer-x32.yaml#L17518))
- [ ] Measure how long the connect-time reads take on a console, and whether replies are lost under the load. ([specs/behringer-x32.yaml:17541](specs/behringer-x32.yaml#L17541))
- [ ] Confirm the source-block indices of /config/routing/OUT for blocks 5-8 and 13-16 follow the second list on p.23. ([specs/behringer-x32.yaml:17554](specs/behringer-x32.yaml#L17554))
- [ ] Confirm the console pushes the routswitch and PLAY block changes it makes itself during USB card playback. ([specs/behringer-x32.yaml:17559](specs/behringer-x32.yaml#L17559))
- [ ] Find what /config/solo/masterctrl does on the console. ([specs/behringer-x32.yaml:17567](specs/behringer-x32.yaml#L17567))
- [ ] Check what /config/dp48/broadcast does and whether it answers 0. ([specs/behringer-x32.yaml:17570](specs/behringer-x32.yaml#L17570))
- [ ] Confirm that a preset saved at /save position N is listed under /-libs at N+1. ([specs/behringer-x32.yaml:17584](specs/behringer-x32.yaml#L17584))
- [ ] Check whether /save over an occupied position asks for confirmation on the console (the Overwrite confirmation preference) or overwrites at once. ([specs/behringer-x32.yaml:17576](specs/behringer-x32.yaml#L17576))
- [ ] Check that setting /-stat/tape/state controls the USB recorder, and the units of its elapsed and remaining times. ([specs/behringer-x32.yaml:17622](specs/behringer-x32.yaml#L17622))
- [ ] Check that an effect's enumeration parameters take ,i (set_fx_parameter_int) and its continuous ones ,f. ([specs/behringer-x32.yaml:17597](specs/behringer-x32.yaml#L17597))
- [ ] Confirm /-stat/selidx takes the strip index 0-71 when set, as it reports it. ([specs/behringer-x32.yaml:17606](specs/behringer-x32.yaml#L17606))
- [ ] Check that the console takes `/meters ,siii /meters/N 0 0 <time factor>` for banks 1-3 and answers on /meters/1-3 at the time factor's interval, and that resending it every 9 s keeps the meters coming. ([specs/behringer-x32.yaml:6224](specs/behringer-x32.yaml#L6224))
- [ ] Check what the gate and dynamics gain-reduction values of /meters/1 and /meters/2 mean: they are read as linear levels (1.0 = 0 dB) like the level meters. ([specs/behringer-x32.yaml:15798](specs/behringer-x32.yaml#L15798))
- [ ] Check that /meters/3's first six values are the aux sends (rear aux outputs 1-6) and the last eight the effects returns 1L-4R. ([specs/behringer-x32.yaml:15831](specs/behringer-x32.yaml#L15831))

## behringer-xair — Behringer X AIR

- [ ] Confirm the mute sense of mix/on (0 muted, 1 passing audio), which is taken from the X32. ([specs/behringer-xair.yaml:4951](specs/behringer-xair.yaml#L4951))
- [ ] Check whether the mixer echoes a set back to the client that sent it. ([specs/behringer-xair.yaml:4975](specs/behringer-xair.yaml#L4975))
- [ ] Confirm the snapshot node is /-snap/ (not /snap/ as the manufacturer document writes it), and whether /-snap/load replies. ([specs/behringer-xair.yaml:4968](specs/behringer-xair.yaml#L4968))
- [ ] Confirm the fader taper is the X32's four segments (0.75 = 0 dB), which the X AIR document does not state. ([specs/behringer-xair.yaml:5016](specs/behringer-xair.yaml#L5016))
- [ ] Confirm headamp gain range is -12 to +60 dB (the community list says -12 to +20 dB). ([specs/behringer-xair.yaml:5033](specs/behringer-xair.yaml#L5033))
- [ ] Find how line inputs and the stereo aux input are addressed for gain, which is not documented. ([specs/behringer-xair.yaml:5030](specs/behringer-xair.yaml#L5030))
- [ ] Find which /ch/NN/config/insrc value is OFF, if any, within 0-15. ([specs/behringer-xair.yaml:5058](specs/behringer-xair.yaml#L5058))
- [ ] Find the order of gate modes 0-4 (GATE, EXP2, EXP3, EXP4, DUCK or the X32's EXP2, EXP3, EXP4, GATE, DUCK). ([specs/behringer-xair.yaml:5066](specs/behringer-xair.yaml#L5066))
- [ ] Confirm the copy-error resolutions: bus pan and insert ranges, bus compressor filter type 0-8, /bus/N/grp, USB routing 1-18. ([specs/behringer-xair.yaml:5074](specs/behringer-xair.yaml#L5074))
- [ ] Confirm the main LR insert is /lr/insert/on and /lr/insert/fxslot (0-8), as on the buses, not /lr/dyn/insert/... as the community list prints it. ([specs/behringer-xair.yaml:5088](specs/behringer-xair.yaml#L5088))
- [ ] Confirm FX return EQ on/off is /rtn/N/eq/on (the community list gives eq/on for the aux return only). ([specs/behringer-xair.yaml:5090](specs/behringer-xair.yaml#L5090))
- [ ] Confirm the bus and main graphic EQ bands are /bus/N/geq/<frequency> and /lr/geq/<frequency> (20, 25, 31.5 ... 1k25 ... 20k), not indexed /geq/0-30, that they are 0-1 floats over -15 to +15 dB, and whether TEQ mode uses the same bands. ([specs/behringer-xair.yaml:5093](specs/behringer-xair.yaml#L5093))
- [ ] Find which effect parameters take a float and which an int (the community list types every /fx/N/par/NN as int; this spec follows the X32's per-parameter types), and how many parameters a slot has. ([specs/behringer-xair.yaml:5100](specs/behringer-xair.yaml#L5100))
- [ ] Check the order of /xinfo's reply strings. ([specs/behringer-xair.yaml:5124](specs/behringer-xair.yaml#L5124))
- [ ] Check whether /-action/setclock is ignored on XR18, X18 and MR18. ([specs/behringer-xair.yaml:5052](specs/behringer-xair.yaml#L5052))
- [ ] Check the low-cut frequency mapping of 0-1 (the community list gives 20 to 200 Hz). ([specs/behringer-xair.yaml:2980](specs/behringer-xair.yaml#L2980))
- [ ] Check the automix weight range (-24 to +24 or -12 to +12). ([specs/behringer-xair.yaml:3036](specs/behringer-xair.yaml#L3036))
- [ ] Check whether the mixer answers /node and the / text set as the X32 does, which the manufacturer document does not describe and this spec does not offer. ([specs/behringer-xair.yaml:5131](specs/behringer-xair.yaml#L5131))
- [ ] Check that the mixer takes `/meters ,s /meters/N` without a time factor, sends meters for 10 s and keeps sending them when the request is repeated every 9 s. ([specs/behringer-xair.yaml:4070](specs/behringer-xair.yaml#L4070))
- [ ] Check the byte order of the meter blob's size and 16-bit values: little-endian is assumed, as for the X32's meters; the document does not say. ([specs/behringer-xair.yaml:4232](specs/behringer-xair.yaml#L4232))
- [ ] Check which stereo return each pair of /meters/1's '5x2 fx/aux' values is (aux first or the effects returns first). ([specs/behringer-xair.yaml:4236](specs/behringer-xair.yaml#L4236))

## biamp-tesira — Biamp Tesira

- [ ] Capture the login prompts of a protected system over Telnet (the module looks for a waiting "login:", "username:", "user name:", "user:" or "password:") and what follows a wrong password. ([specs/biamp-tesira.yaml:444](specs/biamp-tesira.yaml#L444))
- [ ] Check whether an unprotected system asks for a login over Telnet at all (the manual says a login prompt appears; Biamp's negotiation example goes straight to the banner). ([specs/biamp-tesira.yaml:444](specs/biamp-tesira.yaml#L444))
- [ ] Check that lines sent with LF alone (no CR) are accepted, including the user name and password at the login prompts. ([specs/biamp-tesira.yaml:444](specs/biamp-tesira.yaml#L444))
- [ ] Check whether a subscription's first publication and its +OK arrive on separate lines (the manual) or on one line (the knowledge-base examples); the module accepts both. ([specs/biamp-tesira.yaml:481](specs/biamp-tesira.yaml#L481))
- [ ] Check that subscription labels of the form meros1, meros2 are accepted unquoted, and that subscribing again with the same label replaces the subscription rather than adding one. ([specs/biamp-tesira.yaml:481](specs/biamp-tesira.yaml#L481))
- [ ] Check that DEVICE and SESSION work with the module's tag handling, and that quoted tags with spaces work for every command. ([specs/biamp-tesira.yaml:458](specs/biamp-tesira.yaml#L458))
- [ ] Check what recallPresetByName and savePresetByName answer for an unknown name, and the value recallPresetShowFailures returns. ([specs/biamp-tesira.yaml:318](specs/biamp-tesira.yaml#L318))
- [ ] Check which Tesira amplifier models host a TTP server. ([specs/biamp-tesira.yaml:132](specs/biamp-tesira.yaml#L132))
- [ ] Check that SESSION set verbose true is answered +OK on every firmware the module may meet. ([specs/biamp-tesira.yaml:497](specs/biamp-tesira.yaml#L497))

## birddog — BirdDog

- [ ] On 2.0 cameras, check whether a single-key POST is accepted or the whole object must be sent. ([specs/birddog.yaml:4322](specs/birddog.yaml#L4322))
- [ ] Confirm a POST reply does or does not show the new value, so it is clear whether a following poll is needed. ([specs/birddog.yaml:4307](specs/birddog.yaml#L4307))
- [ ] Check whether presets above 9 are accepted over REST on 2.1 cameras (2.0 gives 1-9, 2.1 gives no range). ([specs/birddog.yaml:4291](specs/birddog.yaml#L4291))
- [ ] Check that a PanSpeed, TiltSpeed or ZoomSpeed set through /birddogptzsetup applies to the next REST move at once, and whether it changes a move already running. ([specs/birddog.yaml:4295](specs/birddog.yaml#L4295))
- [ ] Confirm the document inconsistency resolutions: TWODNR key on X1, the Sil2 ModeSel values, /sil2codec and /sil2enc paths, /RestImage, NDIGrpName and NDIOffSnSrc spelling, Wi-Fi and DHCP values On/Off and dhcp/static. ([specs/birddog.yaml:4375](specs/birddog.yaml#L4375))
- [ ] Check the ExpCompLvl range on P200 A2/A3 (0-14 or -128 to 127). ([specs/birddog.yaml:4392](specs/birddog.yaml#L4392))
- [ ] Check whether AnalogAudiooutputselect exists on P200/A200/A300, P100/PF120 and P4K/P400, and whether P4K/P400 accept the videooutputinterface modes. ([specs/birddog.yaml:4393](specs/birddog.yaml#L4393))
- [ ] Check MAKI Ultra tuning and WBSensitivity value forms (numbers or names such as "Middle"). ([specs/birddog.yaml:4388](specs/birddog.yaml#L4388))
- [ ] Confirm the X1 pan POST range -12996 to 12208. ([specs/birddog.yaml:4358](specs/birddog.yaml#L4358))
- [ ] Confirm the API needs no authentication on the firmware in use. ([specs/birddog.yaml:4282](specs/birddog.yaml#L4282))
- [ ] On X5 Ultra and X4 Ultra, check the firmware version and which 2.1 endpoints it lacks. ([specs/birddog.yaml:4441](specs/birddog.yaml#L4441))
- [ ] Confirm the 30-second reads of every endpoint a camera serves (up to 16 requests) and the 5-second polls do not disturb the camera, and that each model serves the endpoints its request list names. ([specs/birddog.yaml:3473](specs/birddog.yaml#L3473))
- [ ] Confirm the 2.0 cameras answer GET /sil2codec per streaming protocol (HX, SRT, RTSP, RTMP, DISABLE) with each protocol's preset and Custom values, as the 2.0 example data shows. ([specs/birddog.yaml:3821](specs/birddog.yaml#L3821))
- [ ] Confirm the 2.0 /sil2enc reply's value forms: Encryption as true/false (or 0/1), AuthEnable as 0/1, pbkeylen 16, 24 or 32. ([specs/birddog.yaml:3865](specs/birddog.yaml#L3865))
- [ ] Confirm the 2.1 /secondary_protocol reply's encryption and authentication values are enable and disable, and record what it reports for unset fields (the document's POST reply shows %!s(<nil>)). ([specs/birddog.yaml:3896](specs/birddog.yaml#L3896))
- [ ] Record the Wi-Fi Enable and DhcpEnable values the cameras report (documented On/Off and dhcp/static, the example shows 1; 1 is read as on and dhcp). ([specs/birddog.yaml:3934](specs/birddog.yaml#L3934))
- [ ] Record what GET /Genlock_Failover_Source returns (the collection gives its keys but no example reply). ([specs/birddog.yaml:3951](specs/birddog.yaml#L3951))
- [ ] Record the GET /connectTo reply per location once it is complete on the cameras (sourceHostname, sourceStreamName, sourceIP, sourcePort), and its status in encode mode. ([specs/birddog.yaml:3964](specs/birddog.yaml#L3964))
- [ ] Confirm /operationmode answers exactly encode, decode or dual, and the P400/P4K /videooutputinterface exactly LowLatency or NormalMode, with nothing around them (read as plain text by their words). ([specs/birddog.yaml:3973](specs/birddog.yaml#L3973))
- [ ] Confirm that reading an endpoint again straight after a POST reports the new value, and how long a camera takes to apply a change (some only after a reboot). ([specs/birddog.yaml:3998](specs/birddog.yaml#L3998))
- [ ] Confirm a preset recall in Camera or Birddog preset mode restores the exposure, picture, white balance and colour settings that are read again after it. ([specs/birddog.yaml:4013](specs/birddog.yaml#L4013))
- [ ] Check GET /List's shape on a camera that sees sources: each member a source's NDI name, its value the address and port; a member whose value holds no digit (the documented {"None":"None"}) is not taken for a source, and a reset or refresh is followed by a read of the list. ([specs/birddog.yaml:3639](specs/birddog.yaml#L3639))
- [ ] Record the plain-text GET replies of /version, /hostname, /NDIGrpName (quoted names separated by commas assumed, from BirdDog's example) and /NDIOffSnSrc (addresses separated by commas), and what a successful POST to /NDIGrpName and /NDIOffSnSrc answers, which must not be taken for a value. ([specs/birddog.yaml:3982](specs/birddog.yaml#L3982))

## birddog-converters — BirdDog converters and decoders

- [ ] Which converter firmware first serves API 2.0, and whether Studio and Mini units on older firmware answer /about with API 1.0's {"Version":"1.0"}. ([specs/birddog-converters.yaml:1091](specs/birddog-converters.yaml#L1091))
- [ ] Check /about's Format value on each converter (Studio, Mini, Flex Encode, Flex Decode, WP Encode, WP Decode, 4KHDMI/SDI, QUAD, PLAY) and what Flex 4K BACKPACK and Pod report. ([specs/birddog-converters.yaml:1098](specs/birddog-converters.yaml#L1098))
- [ ] Confirm POST /connectTo with only {"sourceName": ...} switches the decoder on every converter, and whether ChNum in the query selects the channel on QUAD and dual-channel 4K HDMI/SDI. ([specs/birddog-converters.yaml:1081](specs/birddog-converters.yaml#L1081))
- [ ] Check which keys /decodestatus returns (the document's example and parameter table differ), and its reply in encode mode. ([specs/birddog-converters.yaml:1074](specs/birddog-converters.yaml#L1074))
- [ ] Check the odd DEVICE SUPPORT cells: /encodeTransport on Flex Decode and QUAD, decode NDIAudio on Wallplate Output and 4K HDMI/SDI, StreamName and NDIGroup on Wallplate Input. ([specs/birddog-converters.yaml:1071](specs/birddog-converters.yaml#L1071))
- [ ] Check whether single-key POST bodies with ChNum are accepted on /encodesetup and /decodesetup, or the whole object must be sent. ([specs/birddog-converters.yaml:1082](specs/birddog-converters.yaml#L1082))
- [ ] Check /operationmode's values on 4K QUAD with the 2x2 mode. ([specs/birddog-converters.yaml:1106](specs/birddog-converters.yaml#L1106))

## blackmagic-atem — Blackmagic ATEM

- [ ] Check how many simultaneous connections the switcher accepts and how long a dropped connection holds its place. ([specs/blackmagic-atem.yaml:4021](specs/blackmagic-atem.yaml#L4021))
- [ ] Confirm camera commands are acknowledged with no camera connected and that cameras.* reflects what a connected camera applied. ([specs/blackmagic-atem.yaml:4089](specs/blackmagic-atem.yaml#L4089))
- [ ] Check KeFS's layout: Sofie reads the at-keyframe bits and the infinite direction at bytes 6 and 7, LibAtem's sample has them at 4 and 5. Record which a switcher sends after usk_fly_to and correct mes.*.usk.*.fly if needed. ([specs/blackmagic-atem.yaml:3716](specs/blackmagic-atem.yaml#L3716))
- [ ] Confirm the keyer units: DVE size 1000 raw is full frame; positions and mask edges are ±16000 and ±9000 raw at a 16:9 frame's edges (and what they are on SD 4:3); DVE positions to ±1000 are accepted; rotation past ±360 degrees turns more than once. ([specs/blackmagic-atem.yaml:4111](specs/blackmagic-atem.yaml#L4111))
- [ ] Record which models have the advanced chroma keyer and which the older chroma keyer, and whether a switcher acknowledges and ignores the other keyer's commands. ([specs/blackmagic-atem.yaml:4120](specs/blackmagic-atem.yaml#L4120))
- [ ] Confirm the chroma sample scaling (cursor in thousandths, cursor size in hundredths, Y, Cb and Cr in ten-thousandths) and the range of Cb and Cr. ([specs/blackmagic-atem.yaml:1859](specs/blackmagic-atem.yaml#L1859))
- [ ] Confirm RFlK carries mask 2 only when running to infinite, as Sofie sends it, and that runs to A, B and full ignore the direction byte. ([specs/blackmagic-atem.yaml:1762](specs/blackmagic-atem.yaml#L1762))
- [ ] Confirm the Fairlight dynamics and EQ ranges the spec accepts (threshold, ratio, attack, hold, release, range, EQ frequency, gain and Q) against ATEM Software Control, and what the switcher does with a value outside a band's or source's own range. ([specs/blackmagic-atem.yaml:4146](specs/blackmagic-atem.yaml#L4146))
- [ ] Record what the EQ band frequency_range values (bits 1, 2, 4, 8) mean on a real switcher, so they can be named. ([specs/blackmagic-atem.yaml:3852](specs/blackmagic-atem.yaml#L3852))
- [ ] Settle which DVE transition rate comes first: set the DVE rate and the logo wipe rate to different values in ATEM Software Control and record which TDvP byte (1 or 2) and which CTDv byte and mask bit (3 and bit 0, or 4 and bit 1) each one takes. atem-connection and LibAtem's samples put the transition rate first; bmdwire's documentation puts the logo rate first. If bmdwire is right, swap rate and logo_rate in the module. ([specs/blackmagic-atem.yaml:4316](specs/blackmagic-atem.yaml#L4316))
- [ ] Check the audio routing internal port names (atem-connection marks most as unverified) and the channel pair encoding of routing ids on a Constellation. ([specs/blackmagic-atem.yaml:3903](specs/blackmagic-atem.yaml#L3903))
- [ ] Confirm the pre-2.30 RCA-to-XLR switch maps to pro_line and microphone as atem-connection reads it, and that CFIP before 2.30 takes the switch at byte 4. ([specs/blackmagic-atem.yaml:4162](specs/blackmagic-atem.yaml#L4162))
- [ ] Confirm the Fairlight monitor's "on" bytes are the inverse of muted (atem-connection writes them so) and whether sidetone has a mute. ([specs/blackmagic-atem.yaml:2602](specs/blackmagic-atem.yaml#L2602))
- [ ] Confirm reset_fairlight_peaks needs the extra bytes atem-connection sends (1 at byte 1 with all, 4 at byte 3 with master). ([specs/blackmagic-atem.yaml:2563](specs/blackmagic-atem.yaml#L2563))
- [ ] Check MvPr's layout from protocol 2.28: atem-connection reads program/preview swapped at byte 2, LibAtem's older sample a safe-area flag at 2 and swapped at 3; and that CMvP's mask is bit 0 layout, bit 1 swapped. ([specs/blackmagic-atem.yaml:3941](specs/blackmagic-atem.yaml#L3941))
- [ ] Confirm set_clip's SMPC layout (3 at byte 0, name from 2, frames at 66) names a clip without an upload, and the longest name the switcher keeps. ([specs/blackmagic-atem.yaml:2833](specs/blackmagic-atem.yaml#L2833))
- [ ] Confirm STAB, SaMw and ISOi are accepted as commands under the same names the switcher reports them by, as atem-connection sends them. ([specs/blackmagic-atem.yaml:3073](specs/blackmagic-atem.yaml#L3073))
- [ ] Confirm CRSS bit 3 sets both video bitrates at 1092 and 1096, and the units (bits per second). ([specs/blackmagic-atem.yaml:3957](specs/blackmagic-atem.yaml#L3957))
- [ ] Check RTMD's status bits (idle, unformatted, active, recording, bit 5 removed) on a recording model with two disks. ([specs/blackmagic-atem.yaml:3955](specs/blackmagic-atem.yaml#L3955))
- [ ] Confirm the camera colour wheel ranges (lift ±2, gamma ±4, gain 0 to 16, offset ±8), the iris aperture value scale, the ND filter in stops, and what camera_record does on a camera without media. ([specs/blackmagic-atem.yaml:3468](specs/blackmagic-atem.yaml#L3468))
- [ ] Confirm colour bars on is the SINT8 30 camera-control sends (seconds of bars) and how the camera reports it back. ([specs/blackmagic-atem.yaml:3427](specs/blackmagic-atem.yaml#L3427))
- [ ] Confirm the switcher advertises `_blackmagic._tcp` with TXT `class=AtemSwitcher` ([crates/core/src/mdns.rs:118](crates/core/src/mdns.rs#L118)) and `_switcher_ctrl._udp` ([crates/core/src/mdns.rs:123](crates/core/src/mdns.rs#L123)), as Companion's ATEM module browses for, and from which firmware on; record which models (Mini, Television Studio, Constellation) do.
- [ ] Record what the unit advertises over mDNS (`dns-sd -B _blackmagic._tcp` and `dns-sd -L <instance> _blackmagic._tcp`, or `avahi-browse -rt _blackmagic._tcp`): every service type, the SRV port and the TXT keys and values. Only the service type and TXT `class` come from a public source (Bitfocus Companion's module manifests); the SRV port and the other TXT keys are not documented anywhere. ([crates/core/src/mdns.rs:115](crates/core/src/mdns.rs#L115))
- [ ] Confirm TXT `name` is the product name (as Companion's MultiView 4 filter relies on), not a label the user sets: discovery narrows `models` to the catalogue model with that name. ([crates/core/src/mdns.rs:471](crates/core/src/mdns.rs#L471))
- [ ] On a classic-mixer switcher (such as an ATEM Television Studio HD or 1 M/E), capture AMLv after SALN: that the source ids follow the monitor levels at 36 and are padded to 4 bytes, that the monitor has four levels at 20 to 35 (SKAARHOJ lists one and twelve unknown bytes), and the levels' full scale and whether they are linear, so they can be given in dBFS. ([specs/blackmagic-atem.yaml:3793](specs/blackmagic-atem.yaml#L3793))
- [ ] Check that SALN starts and stops the classic mixer's AMLv levels about ten times a second, and that a new session needs it sent again, as SFLN does. ([specs/blackmagic-atem.yaml:2267](specs/blackmagic-atem.yaml#L2267))
- [ ] Confirm the lock commands as atem-connection lays them out (LOCK u16 store, u8 state; LKOB u16 store; LKST u16 store, u8 locked), that the stills are store 0 and clip n store n, and that a client keeps its lock from LKOB to its own release whatever LKST announces in between. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Check that a LOCK sent while another client holds the store is queued and granted later (LKOB with no LKST), and that withdrawing it with LOCK state 0 before the grant does nothing, so the late grant has to be released. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Record what FTDE codes 1, 2, 5 and 6 mean on a switcher (atem-connection: 1 retry, 2 not found, 5 "maybe" not locked; bmdwire: 6 another client won the store, its loser still holding its lock), and whether any other code appears. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Confirm FTDC goes to every connected client with the owner's transfer id, and that FTDa goes only to the client that asked. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Check that the 60 ms wait after a release and the wait for a store announced held (up to 10 s) let ATEM Software Control load its thumbnails while this client transfers stills back to back. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Confirm the switcher takes 1,392-byte chunks when its FTCD offers 1,396, as atem-connection sends whole 8-byte blocks, and that a chunk ending before a run marker rather than inside the run is required. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Check how long the switcher keeps a dead session's lock (bmdwire measured about 5 s) and that a new session of the same client is then granted it. ([specs/blackmagic-atem.yaml:4218](specs/blackmagic-atem.yaml#L4218))
- [ ] Confirm the FTSD and FTSU layouts (id, store, index at 6, size or type at 8, mode at 12) and the modes and types: 1 for a frame, 0x100 for clip audio, 0x300 for a macro, 0x201 for a label; 0xF9 to download a frame, 0x3FE a macro; macros and labels in store 0xFFFF with no lock. ([specs/blackmagic-atem.yaml:4204](specs/blackmagic-atem.yaml#L4204))
- [ ] Confirm FTFD's layout (name at 2, 64 bytes; description at 66, 128 bytes; MD5 at 194), the longest still name and description the switcher keeps, and that MPfe then reports the MD5 of the uncompressed 10-bit YUVA frame as the still's hash. ([specs/blackmagic-atem.yaml:3926](specs/blackmagic-atem.yaml#L3926))
- [ ] Check the picture conversion against ATEM Software Control: video-range luma and alpha (64 to 940), chroma around 512, BT.709 from 720 lines and BT.601 below, a pair of pixels sharing one chroma sample (Cb in the first word, Cr in the second). ([specs/blackmagic-atem.yaml:4250](specs/blackmagic-atem.yaml#L4250))
- [ ] Confirm the frame sizes the switcher expects in each video mode, in particular 640 by 480 for 525-line SD (atem-connection's size) and 7680 by 4320 for 8K (atem-connection writes 4260 in one place). ([specs/blackmagic-atem.yaml:4250](specs/blackmagic-atem.yaml#L4250))
- [ ] Check upload_clip end to end: CMPC clears the clip before frame 1, every frame goes up under the clip's lock, and SMPC (3, clip, name, frame count) makes it playable; and that clips 3 and 4 of a Constellation take frames in stores 3 and 4 (atem-connection only goes to clip 2). ([specs/blackmagic-atem.yaml:2909](specs/blackmagic-atem.yaml#L2909))
- [ ] Check download_clip on a clip: FTSU to store clip n with the frame index and type 0xF9. ([specs/blackmagic-atem.yaml:2930](specs/blackmagic-atem.yaml#L2930))
- [ ] Confirm clip audio's format: 48 kHz 24-bit stereo, big-endian samples with every 4-byte word reversed on switchers from the Production Studio 4K on (atem-connection), and what the switcher does with an odd number of sample pairs (one silent pair is added here). ([specs/blackmagic-atem.yaml:2949](specs/blackmagic-atem.yaml#L2949))
- [ ] Confirm a macro round trip: download_macro gives the switcher's own macro data, upload_macro of that file restores it, the FTFD name becomes the macro's name (MPrp), and whether a description in FTFD is kept. ([specs/blackmagic-atem.yaml:2966](specs/blackmagic-atem.yaml#L2966))
- [ ] Check upload_multiviewer_label on a switcher before the ATEM Mini: the label bitmap's size and layout per multiviewer resolution, and that bytes 254 and 255 are refused here because they upset the switcher (atem-connection). ([specs/blackmagic-atem.yaml:2997](specs/blackmagic-atem.yaml#L2997))
- [ ] Record what the switcher does when a client stops sending chunks or acknowledging them and releases its lock in the middle of a transfer (cancel_transfer): whether it ends the transfer, and what it leaves in the slot. ([specs/blackmagic-atem.yaml:3030](specs/blackmagic-atem.yaml#L3030))
- [ ] Check clear_still and clear_clip under the lock: on a 1 M/E Production Studio 4K (where bmdwire saw a bare CSTL ignored) and on a newer model, that CSTL sent while holding the stills lock empties the slot (MPfe not in use) and CMPC under clip n's lock empties the clip, that the lock released in the next packet does not undo or race the clear, and whether a bare CSTL or CMPC is taken on models while no other client holds the lock, as the SDK manual allows. ([specs/blackmagic-atem.yaml:4279](specs/blackmagic-atem.yaml#L4279))
- [ ] Check set_hyperdeck against ATEM Software Control's HyperDeck settings: CXMS's mask bits (1 address, 2 input, 4 auto roll, 8 frame delay, inferred from the field order), that the address goes in written order (192.168.1.50 as C0 A8 01 32) rather than least significant byte first as the SDK's packed form is, that 0.0.0.0 stops the switcher connecting, and RXMS's fields (status at byte 14, storage media count at 16, remote access at 18). ([specs/blackmagic-atem.yaml:4332](specs/blackmagic-atem.yaml#L4332))
- [ ] Record RXMS's status values with a HyperDeck connecting, connected and on old firmware: the core names 0 to 3 not_connected, connecting, connected and incompatible in the SDK manual's order. ([specs/blackmagic-atem.yaml:4332](specs/blackmagic-atem.yaml#L4332))
- [ ] With a HyperDeck connected to the switcher, capture RXCP, RXSS, RXCC and RXCS while it is stopped, playing, recording and shuttling, and with media in and out, and capture ATEM Software Control's CXCP and CXSS for play, stop, record, shuttle, jog, loop, single clip, clip and media selection: the player and storage states' values are what HyperDeck transport through the switcher needs (unconnected slots report player state 1). ([specs/blackmagic-atem.yaml:4332](specs/blackmagic-atem.yaml#L4332))
- [ ] Confirm V3sl and C3sl's values: 1 level A, 0 level B as bmdwire documents them (the SDK manual lists A first), and that bytes 1 to 3 carry nothing. ([specs/blackmagic-atem.yaml:4350](specs/blackmagic-atem.yaml#L4350))
- [ ] Confirm set_fade_to_black_enabled: that the switcher takes FEna from a client with the enable at byte 1 (no source gives the command's bytes), that one setting covers every M/E, what byte 0 means if anything (captures show 0, 4 and 0x69), and from which firmware each model sends FEna. ([specs/blackmagic-atem.yaml:4359](specs/blackmagic-atem.yaml#L4359))
- [ ] Confirm WhoI's layout (id 32 characters at 0, address at 32, host name at 48, name at 112, 176 bytes), from which firmware it is sent, and whether name follows the name set in ATEM Setup. ([specs/blackmagic-atem.yaml:4373](specs/blackmagic-atem.yaml#L4373))
- [ ] Look for the messages ATEM Software Control uses to download clip audio and to rename a still, which Blackmagic's SDK offers (DownloadAudio, SetName) but no public protocol source describes. ([specs/blackmagic-atem.yaml:4267](specs/blackmagic-atem.yaml#L4267))
- [ ] Confirm the goodbye: the hello packet with the session's id and byte 12 set to 4 ends the session, and the switcher then frees its place and media pool locks within about 10 ms (bmdwire) and answers nothing, or record what it answers. ([specs/blackmagic-atem.yaml:4030](specs/blackmagic-atem.yaml#L4030))
- [ ] Confirm that MPfe reports clip n's frames in pool n (atem-connection reads pools 1 and 2 as clips 1 and 2), including clips 3 and 4 of a Constellation, and whether a valid frame always carries a non-zero hash. ([specs/blackmagic-atem.yaml:3768](specs/blackmagic-atem.yaml#L3768))
- [ ] Compare a label drawn by set_multiviewer_label with ATEM Software Control's on a switcher before the ATEM Mini, at each multiviewer resolution: box size and position, the palette (atem-connection's coverage mapping), border and corners, and which resolutions the switcher expects stacked in the buffer. ([specs/blackmagic-atem.yaml:4240](specs/blackmagic-atem.yaml#L4240))
- [ ] Check that the multiviewer modes in _VMC (one bit per video mode, at byte 4 of each entry) are the resolutions the switcher wants labels for, and what it does with a label buffer of the wrong size. ([specs/blackmagic-atem.yaml:4240](specs/blackmagic-atem.yaml#L4240))

## blackmagic-audio-monitor — Blackmagic Audio Monitor 12G G3

- [ ] Check that the monitor answers a header-only block (such as `AUDIO OUTPUT:` alone) with ACK and that block, as the Videohub protocol does. ([specs/blackmagic-audio-monitor.yaml:166](specs/blackmagic-audio-monitor.yaml#L166))
- [ ] Record the reply to a refused change (NAK, or something else) and confirm the ACK: block repeats the fields set as the manual shows. ([specs/blackmagic-audio-monitor.yaml:223](specs/blackmagic-audio-monitor.yaml#L223))
- [ ] Check the longest label the monitor takes (64 characters assumed). ([specs/blackmagic-audio-monitor.yaml:54](specs/blackmagic-audio-monitor.yaml#L54))
- [ ] Check which input routings a monitor refuses (SDI pairs beyond those the signal carries, ST2110 pairs without a 2110 stream). ([specs/blackmagic-audio-monitor.yaml:84](specs/blackmagic-audio-monitor.yaml#L84))
- [ ] Record the SDI Output Level values the ST2110 block reports besides Auto. ([specs/blackmagic-audio-monitor.yaml:245](specs/blackmagic-audio-monitor.yaml#L245))
- [ ] Record the audio delay fields with a delay set in frames, and the units' other value (Frames assumed). ([specs/blackmagic-audio-monitor.yaml:257](specs/blackmagic-audio-monitor.yaml#L257))

## blackmagic-camera — Blackmagic cameras

- [ ] Confirm the notification websocket's path, /control/api/v1/event/websocket, against the camera's /control/documentation.html: the PDF does not give it. ([specs/blackmagic-camera.yaml:2395](specs/blackmagic-camera.yaml#L2395))
- [ ] Confirm the camera accepts the subscribe request with type "request" (the PDF's Publish table repeats the Response table, type "response") and that every property subscribed is subscribable. ([specs/blackmagic-camera.yaml:2401](specs/blackmagic-camera.yaml#L2401))
- [ ] Check that over HTTPS the websocket is served as wss on the same port, and whether it needs the same login. ([specs/blackmagic-camera.yaml:2396](specs/blackmagic-camera.yaml#L2396))
- [ ] Check that a pushed property names its path without the /control/api/v1 base, as the document's tables show, so the value and the re-read go to the right path. ([specs/blackmagic-camera.yaml:2399](specs/blackmagic-camera.yaml#L2399))
- [ ] Confirm a propertyValueChanged event carries the property's value in the format its REST GET returns (applied as that reply), and whether any event names only the property (then read again). ([specs/blackmagic-camera.yaml:2473](specs/blackmagic-camera.yaml#L2473))
- [ ] Confirm the login scheme with "Enabled with security" (Basic sent, Digest answered if challenged). ([specs/blackmagic-camera.yaml:2859](specs/blackmagic-camera.yaml#L2859))
- [ ] Check which API groups each camera model implements and whether missing ones answer 501 or 404. ([specs/blackmagic-camera.yaml:2865](specs/blackmagic-camera.yaml#L2865))
- [ ] Check that 0.0-1.0 is the range for normalised values beyond audio level. ([specs/blackmagic-camera.yaml:2883](specs/blackmagic-camera.yaml#L2883))
- [ ] Check the ranges the camera accepts for colour correction, playback speed, focus distance and aperture number, which are not limited here. ([specs/blackmagic-camera.yaml:2880](specs/blackmagic-camera.yaml#L2880))
- [ ] Confirm the format filesystem path spelling (doformatSupportedFilesystems or doFormatSupportedFilesystems). ([specs/blackmagic-camera.yaml:2926](specs/blackmagic-camera.yaml#L2926))
- [ ] Check what body PUT /monitoring/{displayName}/focusAssist accepts ({enabled} or mode, color and intensity). ([specs/blackmagic-camera.yaml:2935](specs/blackmagic-camera.yaml#L2935))
- [ ] Check whether a clip path in a folder must be sent with "/" encoded as %2F. ([specs/blackmagic-camera.yaml:2948](specs/blackmagic-camera.yaml#L2948))
- [ ] Check whether preset names for save_preset and delete_preset include the .cset extension. ([specs/blackmagic-camera.yaml:2949](specs/blackmagic-camera.yaml#L2949))
- [ ] Confirm set_custom_platform is accepted with application/xml and the XML on one line. ([specs/blackmagic-camera.yaml:2961](specs/blackmagic-camera.yaml#L2961))
- [ ] Record what the camera advertises over mDNS besides its `<name>.local` host name (service types, TXT `class`). No public source names it, so mDNS discovery does not identify cameras. ([crates/core/src/mdns.rs:751](crates/core/src/mdns.rs#L751))
- [ ] Confirm GET /media/workingset lists every slot, an empty one as null, so a card or drive taken out leaves media.devices at the next poll. ([specs/blackmagic-camera.yaml:2630](specs/blackmagic-camera.yaml#L2630))
- [ ] Confirm POST /presets takes the .cset file as the request body with a Content-Disposition header naming it (the document only implies this through its 400 answer), not a multipart/form-data part, and answers 200 with presetAdded. ([specs/blackmagic-camera.yaml:1697](specs/blackmagic-camera.yaml#L1697))

## blackmagic-hyperdeck — Blackmagic HyperDeck

- [ ] Record the reply shape that carries the format token after format_prepare. ([specs/blackmagic-hyperdeck.yaml:3473](specs/blackmagic-hyperdeck.yaml#L3473))
- [ ] Record the codes and fields of the notifications documented only by their notify switch (dropped frames, display timecode, timeline position, playrange, cache, dynamic range, slate, device info, nas); state reads them by the field names the switches use. ([specs/blackmagic-hyperdeck.yaml:3390](specs/blackmagic-hyperdeck.yaml#L3390))
- [ ] Check how protocol secure mode is reached, so credentials need not travel in plain text on 9993. ([specs/blackmagic-hyperdeck.yaml:3502](specs/blackmagic-hyperdeck.yaml#L3502))
- [ ] Check which dynamic range spelling the deck accepts and reports (ST2084 or ST2048). ([specs/blackmagic-hyperdeck.yaml:3525](specs/blackmagic-hyperdeck.yaml#L3525))
- [ ] Check whether "slate clips:" needs the colon (the table prints it without). ([specs/blackmagic-hyperdeck.yaml:3489](specs/blackmagic-hyperdeck.yaml#L3489))
- [ ] Check what the combined goto forms with relative offsets are relative to. ([specs/blackmagic-hyperdeck.yaml:3437](specs/blackmagic-hyperdeck.yaml#L3437))
- [ ] On protocol 1.8 and 1.11 decks, check which other documented commands work. ([specs/blackmagic-hyperdeck.yaml:3532](specs/blackmagic-hyperdeck.yaml#L3532))
- [ ] Check how many clients may connect at once. hyperdeckwire (MIT) corroborates, on an HD Mini, that a client past the limit is answered "120 connection failed" and the socket closed, without giving the number. ([specs/blackmagic-hyperdeck.yaml:3461](specs/blackmagic-hyperdeck.yaml#L3461))
- [ ] Confirm the deck advertises `_hyperdeck_ctrl._tcp` (Companion's HyperDeck module browses it), that its SRV port is 9993, and whether it also advertises `_blackmagic._tcp` and with which TXT `class`. ([crates/core/src/mdns.rs:135](crates/core/src/mdns.rs#L135))
- [ ] Record the TXT keys `_hyperdeck_ctrl._tcp` carries; if one names the model, discovery could narrow `models`, which it now never does for a HyperDeck. ([crates/core/src/mdns.rs:471](crates/core/src/mdns.rs#L471))
- [ ] Confirm the display timecode, timeline position and dropped frames notifications carry fields named "display timecode", "timeline position" and "dropped frames", and that timeline position and dropped frames are whole numbers. ([specs/blackmagic-hyperdeck.yaml:2752](specs/blackmagic-hyperdeck.yaml#L2752))
- [ ] Confirm every slot info report lists slot id before its other fields, as the document prints it; a field before it is not read. ([specs/blackmagic-hyperdeck.yaml:2759](specs/blackmagic-hyperdeck.yaml#L2759))
- [ ] Check how a deck answers "slot info: slot id: n" for a slot it lacks (slots 1 to 4 are asked for on connecting). ([specs/blackmagic-hyperdeck.yaml:2679](specs/blackmagic-hyperdeck.yaml#L2679))
- [ ] Confirm the playrange reply (219) and push (515) codes and their "timeline in" and "timeline out" fields, and what a clip or timecode playrange and a cleared one report. ([specs/blackmagic-hyperdeck.yaml:2895](specs/blackmagic-hyperdeck.yaml#L2895))
- [ ] Confirm the reply codes and fields of play on startup (218: enable, single clip) and play option (220: stop mode). ([specs/blackmagic-hyperdeck.yaml:2900](specs/blackmagic-hyperdeck.yaml#L2900))
- [ ] Confirm the cache info reply (221: status, transferring slot id, recording time) and the code and header text of the cache notification. ([specs/blackmagic-hyperdeck.yaml:2909](specs/blackmagic-hyperdeck.yaml#L2909))
- [ ] Confirm the field names of the dynamic range reply (playback override, record override) and of the slate replies and notifications (the multiline command's parameter names). ([specs/blackmagic-hyperdeck.yaml:2916](specs/blackmagic-hyperdeck.yaml#L2916))
- [ ] Record the reply shapes of nas list, nas selected, external drive list and external drive selected, kept as text. ([specs/blackmagic-hyperdeck.yaml:2941](specs/blackmagic-hyperdeck.yaml#L2941))
- [ ] Check that a timeline clips notification is 519 (p.8) and whether an addition and a snapshot can be told apart. ([specs/blackmagic-hyperdeck.yaml:2861](specs/blackmagic-hyperdeck.yaml#L2861))
- [ ] Check that clips get on an empty timeline answers 107 timeline empty rather than a 205 with clip count 0. ([specs/blackmagic-hyperdeck.yaml:2855](specs/blackmagic-hyperdeck.yaml#L2855))
- [ ] Check that version 1 clips get lines are "id: name start duration" with the name able to hold spaces. hyperdeckwire (MIT) corroborates this layout and names with spaces on an HD Mini on 8.1.1. ([specs/blackmagic-hyperdeck.yaml:2841](specs/blackmagic-hyperdeck.yaml#L2841))
- [ ] Capture a 206 disk list reply in response versions 1 and 2 and confirm the slot id line follows the header and each clip line has the documented field order (version 1 name first, version 2 file name last), so the clip lists key by slot. hyperdeckwire (MIT) corroborates the version 1 order, with names holding spaces, on an HD Mini on 8.1.1. ([specs/blackmagic-hyperdeck.yaml:2871](specs/blackmagic-hyperdeck.yaml#L2871))
- [ ] Confirm a 520 disk list info notification is sent when clips are added to or removed from the active disk, and record its body (the document does not give it), so it can update the list without a re-read. ([specs/blackmagic-hyperdeck.yaml:2887](specs/blackmagic-hyperdeck.yaml#L2887))
- [ ] Confirm which notify switches and queries protocol 1.8 and 1.11 decks accept (transport, slot, remote and configuration notifications; device info, transport info, disk list, configuration, clips get, and playrange on 1.11), since the old decks are now sent only those. ([specs/blackmagic-hyperdeck.yaml:2655](specs/blackmagic-hyperdeck.yaml#L2655))
- [ ] Confirm the configuration API (`/admin/api/v1` on port 80) on the HyperDeck Studio HD Plus, HD Pro and 4K Pro, the Extreme models and the Shuttle, and record the first software that serves it (the HD Mini's 8.1.1 had port 80 closed); the paths and replies come from an HD Mini on 9.0.2 only. ([specs/blackmagic-hyperdeck.yaml:3541](specs/blackmagic-hyperdeck.yaml#L3541))
- [ ] Record the status and body each configuration write answers (200 with `{"success": true}`, or 204; 201 for a user or signing request), and whether a 2xx answer can carry `"success": false` with an `errorMessage`. ([specs/blackmagic-hyperdeck.yaml:3625](specs/blackmagic-hyperdeck.yaml#L3625))
- [ ] With utility administration set to USB only, confirm reads still answer and PUT is refused with 401, and check POST and DELETE; with it allowed over the network, check whether network access changes are still refused over Ethernet, as the manual's p.53 suggests. ([specs/blackmagic-hyperdeck.yaml:3554](specs/blackmagic-hyperdeck.yaml#L3554))
- [ ] Check what the web media manager set to Disabled, and to SecureOnly, does to the configuration API on port 80 (closed, moved to HTTPS on 443, or unchanged), and how long a network access change needing a reboot keeps answering the old states. ([specs/blackmagic-hyperdeck.yaml:3594](specs/blackmagic-hyperdeck.yaml#L3594))
- [ ] Check whether set_ethernet_protocol Disabled closes connections already open on TCP 9993 or only refuses new ones, and what SecureOnly asks of a client. ([specs/blackmagic-hyperdeck.yaml:3570](specs/blackmagic-hyperdeck.yaml#L3570))
- [ ] Confirm the interface PUT must be whole (and whether a partial one is accepted), that the deck answers it before moving to a new address, and whether interface ids other than 0 exist on any model. ([specs/blackmagic-hyperdeck.yaml:3581](specs/blackmagic-hyperdeck.yaml#L3581))
- [ ] Check that the reboot key is single-use, how long it stays valid, and what the PUT answers before the deck goes down. ([specs/blackmagic-hyperdeck.yaml:3607](specs/blackmagic-hyperdeck.yaml#L3607))
- [ ] Record the certificate summary's validity field names with a certificate installed; state keeps the summary as JSON text until they are known. ([specs/blackmagic-hyperdeck.yaml:3155](specs/blackmagic-hyperdeck.yaml#L3155))
- [ ] Confirm a signed certificate is imported as PEM text in `{"certificate": ...}`, record what the signing request POST answers and the id it gives, and what the `.csr` download returns. ([specs/blackmagic-hyperdeck.yaml:2382](specs/blackmagic-hyperdeck.yaml#L2382))
- [ ] Confirm a user PUT may carry only the fields it changes, that DELETE takes the user id in a JSON body, the format of user ids, and the shape of `sharedFolderPermissions` once a share is granted. ([specs/blackmagic-hyperdeck.yaml:2439](specs/blackmagic-hyperdeck.yaml#L2439))
- [ ] Check how the configuration API takes an administrator login once `adminRequired` is true. ([specs/blackmagic-hyperdeck.yaml:3639](specs/blackmagic-hyperdeck.yaml#L3639))
- [ ] Confirm the date and time PUT takes the time as a string of Unix seconds, what it does with NTP on, and the time zone offsets the deck accepts (-720 to 840 minutes assumed). ([specs/blackmagic-hyperdeck.yaml:2486](specs/blackmagic-hyperdeck.yaml#L2486))
- [ ] Check set_language with each code the language list gives. ([specs/blackmagic-hyperdeck.yaml:2221](specs/blackmagic-hyperdeck.yaml#L2221))
- [ ] Record the network access protocol ids on models other than the HD Mini (FTP and HTTP there); set_network_access sends those two. ([specs/blackmagic-hyperdeck.yaml:2323](specs/blackmagic-hyperdeck.yaml#L2323))
- [ ] Check that PUT and DELETE on `setupBasic/identify` start and stop the front panel flashing. ([specs/blackmagic-hyperdeck.yaml:2231](specs/blackmagic-hyperdeck.yaml#L2231))
- [ ] Check that set_device_name changes the host name and invalidates a certificate issued for the old one. ([specs/blackmagic-hyperdeck.yaml:3617](specs/blackmagic-hyperdeck.yaml#L3617))
- [ ] Record the FTP root's folder names on each model and software version (sd1, ssd1, cfast1, usb, nas, or slot numbers), and that STOR at the root is refused. ([specs/blackmagic-hyperdeck.yaml:3687](specs/blackmagic-hyperdeck.yaml#L3687))
- [ ] Confirm the FTP server's anonymous login (USER anonymous, PASS anonymous@) on each model, and what it asks for with secure login settings set. ([specs/blackmagic-hyperdeck.yaml:3687](specs/blackmagic-hyperdeck.yaml#L3687))
- [ ] Record whether the FTP server answers MLSD, and the LIST format it uses otherwise, from a capture. ([specs/blackmagic-hyperdeck.yaml:3723](specs/blackmagic-hyperdeck.yaml#L3723))
- [ ] Check what the deck's PASV reply names (its own address, 0.0.0.0, a private address), whether it answers EPSV, and its passive port range. ([specs/blackmagic-hyperdeck.yaml:3723](specs/blackmagic-hyperdeck.yaml#L3723))
- [ ] Check that the FTP server answers SIZE (213 for a file, 550 for none) in binary mode; uploads and renames without overwrite fail where it does not. ([specs/blackmagic-hyperdeck.yaml:3671](specs/blackmagic-hyperdeck.yaml#L3671))
- [ ] Check DELE, RNFR and RNTO (including a move between slots and RNTO onto an existing file), MKD and ABOR on the deck. ([specs/blackmagic-hyperdeck.yaml:2590](specs/blackmagic-hyperdeck.yaml#L2590))
- [ ] Check whether the deck takes two FTP sessions at once (a transfer and a listing), and how many. ([specs/blackmagic-hyperdeck.yaml:3705](specs/blackmagic-hyperdeck.yaml#L3705))
- [ ] Check what the deck does to a clip deleted, renamed or replaced over FTP while it is cued, playing or being recorded, and to a partial upload left by a cancel. ([specs/blackmagic-hyperdeck.yaml:3671](specs/blackmagic-hyperdeck.yaml#L3671))
- [ ] Check that a clip uploaded over FTP plays, and that disk list shows it at once on each model. ([specs/blackmagic-hyperdeck.yaml:2569](specs/blackmagic-hyperdeck.yaml#L2569))
- [ ] Check which FTP over TLS a deck with SecureOnly for FTP offers (explicit AUTH TLS or implicit), and which models offer it. ([specs/blackmagic-hyperdeck.yaml:3737](specs/blackmagic-hyperdeck.yaml#L3737))

## blackmagic-multiview — Blackmagic MultiView 16 / MultiView 4

- [ ] On MultiView 4, check which routing output is the solo source (wire 4 assumed) and whether its solo source can be set over Ethernet at all. ([specs/blackmagic-multiview.yaml:421](specs/blackmagic-multiview.yaml#L421))
- [ ] Check that every CONFIGURATION line can be set by sending the block (the manual shows only Solo enabled being sent), and the boolean case the unit reports. ([specs/blackmagic-multiview.yaml:440](specs/blackmagic-multiview.yaml#L440))
- [ ] Check how Output format values map on MultiView 4. ([specs/blackmagic-multiview.yaml:452](specs/blackmagic-multiview.yaml#L452))
- [ ] Check whether the MultiView accepts F (force unlock) on VIDEO OUTPUT LOCKS. ([specs/blackmagic-multiview.yaml:461](specs/blackmagic-multiview.yaml#L461))
- [ ] Confirm the MULTIVIEW DEVICE output count (16 in the manual's example, with 18 routing outputs). ([specs/blackmagic-multiview.yaml:394](specs/blackmagic-multiview.yaml#L394))
- [ ] Confirm MultiView 4 advertises `_blackmagic._tcp` with TXT `class=MultiView` and `name=Blackmagic MultiView 4` (Companion's MultiView 4 module), and whether MultiView 16 uses the same class with `name=Blackmagic MultiView 16`. ([crates/core/src/mdns.rs:154](crates/core/src/mdns.rs#L154))
- [ ] Record what the unit advertises over mDNS (`dns-sd -B _blackmagic._tcp` and `dns-sd -L <instance> _blackmagic._tcp`, or `avahi-browse -rt _blackmagic._tcp`): every service type, the SRV port and the TXT keys and values. Only the service type and TXT `class` come from a public source (Bitfocus Companion's module manifests); the SRV port and the other TXT keys are not documented anywhere. ([crates/core/src/mdns.rs:115](crates/core/src/mdns.rs#L115))

## blackmagic-smartview — Blackmagic SmartView / SmartScope

- [ ] Check that the monitor answers commands with ACK/NAK, answers header-only status requests and PING, as the Videohub protocol does. ([specs/blackmagic-smartview.yaml:390](specs/blackmagic-smartview.yaml#L390))
- [ ] Check how the name is set (Name in the SMARTVIEW DEVICE block assumed). ([specs/blackmagic-smartview.yaml:390](specs/blackmagic-smartview.yaml#L390))
- [ ] Record how WidescreenSD and Border are reported (case, ON/OFF or true/false). ([specs/blackmagic-smartview.yaml:419](specs/blackmagic-smartview.yaml#L419))
- [ ] Check how a single-monitor model answers a MONITOR B block. ([specs/blackmagic-smartview.yaml:427](specs/blackmagic-smartview.yaml#L427))
- [ ] On SmartView 4K, check MonitorInput (community-observed), and whether SmartView 4K G3 has it. ([specs/blackmagic-smartview.yaml:440](specs/blackmagic-smartview.yaml#L440))
- [ ] Check whether SmartView 4K accepts contrast and saturation. ([specs/blackmagic-smartview.yaml:448](specs/blackmagic-smartview.yaml#L448))
- [ ] Confirm current monitors advertise `_blackmagic._tcp` with TXT `class=SmartView` (Companion's SmartView module) ([crates/core/src/mdns.rs:142](crates/core/src/mdns.rs#L142)), whether SmartScope uses the same class, and whether older firmware advertises `_smartview._tcp` (smartview-client browses it) ([crates/core/src/mdns.rs:147](crates/core/src/mdns.rs#L147)).
- [ ] Record what the unit advertises over mDNS (`dns-sd -B _blackmagic._tcp` and `dns-sd -L <instance> _blackmagic._tcp`, or `avahi-browse -rt _blackmagic._tcp`): every service type, the SRV port and the TXT keys and values. Only the service type and TXT `class` come from a public source (Bitfocus Companion's module manifests); the SRV port and the other TXT keys are not documented anywhere. ([crates/core/src/mdns.rs:115](crates/core/src/mdns.rs#L115))
- [ ] Confirm TXT `name` is the product name (as Companion's MultiView 4 filter relies on), not a label the user sets: discovery narrows `models` to the catalogue model with that name. ([crates/core/src/mdns.rs:471](crates/core/src/mdns.rs#L471))

## blackmagic-streaming — Blackmagic Web Presenter / Streaming

- [ ] Check whether the device accepts a Streaming XML file sent on a single line. ([specs/blackmagic-streaming.yaml:685](specs/blackmagic-streaming.yaml#L685))
- [ ] Confirm the character encoding is UTF-8 (the document names none). ([specs/blackmagic-streaming.yaml:699](specs/blackmagic-streaming.yaml#L699))
- [ ] Check whether a unit can report more than two network interfaces. ([specs/blackmagic-streaming.yaml:657](specs/blackmagic-streaming.yaml#L657))
- [ ] Check the length limits for label, stream key, password and URL. ([specs/blackmagic-streaming.yaml:719](specs/blackmagic-streaming.yaml#L719))
- [ ] Check which firmware release introduced protocol 1.2 (and what earlier units report). ([specs/blackmagic-streaming.yaml:710](specs/blackmagic-streaming.yaml#L710))
- [ ] Record what Web Presenter and Streaming Encoder units advertise over mDNS (service types, and TXT `class` under `_blackmagic._tcp`). No public source names it, so mDNS discovery does not identify them; an unknown class is reported as a discovery message naming it. ([crates/core/src/mdns.rs:751](crates/core/src/mdns.rs#L751))

## blackmagic-teranex — Blackmagic Teranex

- [ ] Check which output format text set_output_video_mode needs: 1080i5994 (the Command Syntax example) or 1080i59.94 (the table and the Companion module). ([specs/blackmagic-teranex.yaml:1691](specs/blackmagic-teranex.yaml#L1691))
- [ ] Confirm the AUDIO and NETWORK CONFIG block headers, which are inferred from the section titles. ([specs/blackmagic-teranex.yaml:1704](specs/blackmagic-teranex.yaml#L1704))
- [ ] Check whether the refusal is NACK (as documented) or NAK, and that ACK is followed by a blank line on Teranex AV and Express. ([specs/blackmagic-teranex.yaml:1684](specs/blackmagic-teranex.yaml#L1684))
- [ ] Check whether on/off settings are accepted as true/false (sent here) and how they are reported (true/false or ON/OFF). ([specs/blackmagic-teranex.yaml:1711](specs/blackmagic-teranex.yaml#L1711))
- [ ] Check the ranges the unit accepts for genlock line and pixel offsets, Variable Aspect Ratio and camera align values, and that whole numbers are accepted where it reports decimals. ([specs/blackmagic-teranex.yaml:1718](specs/blackmagic-teranex.yaml#L1718))

## blackmagic-ultimatte — Blackmagic Ultimatte 12

- [ ] Confirm the Controls table is read correctly with its ranges two rows below their controls (for example Matte Correct Horizontal Size 0-6 and Vertical Size 0-3, Transition Rate 1-120, Output Offset -1500 to +1500). ([specs/blackmagic-ultimatte.yaml:1529](specs/blackmagic-ultimatte.yaml#L1529))
- [ ] Check how the unit answers a control it does not have (HD and HD Mini) and a value out of range. ([specs/blackmagic-ultimatte.yaml:1547](specs/blackmagic-ultimatte.yaml#L1547))
- [ ] Check that PING: is answered with ACK. ([specs/blackmagic-ultimatte.yaml:1561](specs/blackmagic-ultimatte.yaml#L1561))
- [ ] Confirm BG 2 Frame Buffer Index and Enable, and the command names of the layer and matte frame buffers. ([specs/blackmagic-ultimatte.yaml:1563](specs/blackmagic-ultimatte.yaml#L1563))
- [ ] Check whether Input Source and Output Enable are set in an IP VIDEO block or a CONTROL block, and in which case (SDI/IP2110, On/Off, or ip2110, on). ([specs/blackmagic-ultimatte.yaml:1571](specs/blackmagic-ultimatte.yaml#L1571))
- [ ] Check that Quickload n: On and Quicksave n: On load and save the quick memories (the Companion module writes Quick Load n). ([specs/blackmagic-ultimatte.yaml:1554](specs/blackmagic-ultimatte.yaml#L1554))

- [ ] Confirm that a VIDEO FORMATS, FILE LIST or GPI LIST block always carries the whole list (an update replaces the list kept), that list lines never hold a colon, and whether an IMAGE LIST update can carry Capacity or Available alone. ([specs/blackmagic-ultimatte.yaml:1084](specs/blackmagic-ultimatte.yaml#L1084))
- [ ] Record the GPI LIST block's event lines (the file of each event, in event order, assumed). ([specs/blackmagic-ultimatte.yaml:1092](specs/blackmagic-ultimatte.yaml#L1092))
- [ ] Check which name the unit sends and takes for the DHCP field: DynamicIP (the document's NETWORK INTERFACE example) or Dynamic IP (seen by bmdwire on protocol 2.1); both are read, and the commands write DynamicIP. ([specs/blackmagic-ultimatte.yaml:622](specs/blackmagic-ultimatte.yaml#L622))
- [ ] Check that a NETWORK INTERFACE 0 block with Static Addresses: <address>/<netmask> and Static Gateway changes the interface, that the unit answers ACK and echoes the fields sent, and that the header alone answers the whole block. ([specs/blackmagic-ultimatte.yaml:1505](specs/blackmagic-ultimatte.yaml#L1505))
- [ ] Check whether the unit pushes NETWORK INTERFACE 0 when the interface has reapplied (Current Addresses reads 0.0.0.0/255.255.0.0 for about a second after an address change). ([specs/blackmagic-ultimatte.yaml:1510](specs/blackmagic-ultimatte.yaml#L1510))
- [ ] Check whether the connection drops on a new address only, or on a netmask, gateway or DHCP change too, and how long the unit takes to answer again. ([specs/blackmagic-ultimatte.yaml:1452](specs/blackmagic-ultimatte.yaml#L1452))
- [ ] Check how Static DNS Servers is written: separated by a comma and a space (the document's listing, sent here) or by spaces (bmdwire), and that an empty value clears the list. ([specs/blackmagic-ultimatte.yaml:654](specs/blackmagic-ultimatte.yaml#L654))
- [ ] Find out what the NETWORK INTERFACE 0 Priority line means (0 in the document and in bmdwire's capture), and whether it can be set. ([specs/blackmagic-ultimatte.yaml:757](specs/blackmagic-ultimatte.yaml#L757))
- [ ] Confirm that CONTROL DEFAULT lines are Name: value as in CONTROL, and that its values never change while connected. ([specs/blackmagic-ultimatte.yaml:1103](specs/blackmagic-ultimatte.yaml#L1103))
- [ ] Check that a TCP 9996 read (opcode 0x0000 with a FILE LIST name, 0x0003 with GPISettings or SavedSettings) answers a big-endian length and that many bytes, then closes, on every model (only an Ultimatte 12 4K was captured). ([specs/blackmagic-ultimatte.yaml:1473](specs/blackmagic-ultimatte.yaml#L1473))
- [ ] Check that a TCP 9996 write (opcode 0x0100 for a preset, 0x0103 for GPISettings or SavedSettings) needs the trailing zero byte and is acknowledged with zero bytes, how the unit answers a refused write, and which parts of the name field it reads (the vendor app's macOS path is sent). ([specs/blackmagic-ultimatte.yaml:1479](specs/blackmagic-ultimatte.yaml#L1479))
- [ ] Check that a Smart Remote 4 Restore accepts an archive export_presets writes (members deflated, the layout of Archive All) and that import_presets takes a Smart Remote Archive All zip. ([specs/blackmagic-ultimatte.yaml:1488](specs/blackmagic-ultimatte.yaml#L1488))
- [ ] Check whether writing a preset under a name not in FILE LIST adds it, and whether the unit sends FILE LIST or CURRENT FILE after import_presets; and whether SavedSettings changes the live key at once. ([specs/blackmagic-ultimatte.yaml:1460](specs/blackmagic-ultimatte.yaml#L1460))
- [ ] Find out whether the Media Pool images and the quick-save memories have their own 9996 opcodes (images/ and quickfiles/ are written empty here), what a Smart Remote archive of a unit holding images puts in them, and whether the HD Mini has GPISettings. ([specs/blackmagic-ultimatte.yaml:1493](specs/blackmagic-ultimatte.yaml#L1493))
- [ ] Check that a cold unit refuses or drops the first 9996 connection and answers a second one 250 ms later. ([specs/blackmagic-ultimatte.yaml:1482](specs/blackmagic-ultimatte.yaml#L1482))

## blackmagic-videohub — Blackmagic Videohub

- [ ] Check how the router answers a port number beyond its count. ([specs/blackmagic-videohub.yaml:1153](specs/blackmagic-videohub.yaml#L1153))
- [ ] Record the exact spelling of the device block's needs-update value. ([specs/blackmagic-videohub.yaml:1164](specs/blackmagic-videohub.yaml#L1164))
- [ ] Check whether SERIAL PORT DIRECTIONS can be set by sending the block. ([specs/blackmagic-videohub.yaml:1206](specs/blackmagic-videohub.yaml#L1206))
- [ ] On Workgroup Videohub, confirm what the two numbers in PROCESSING UNIT ROUTING and FRAME BUFFER ROUTING index. ([specs/blackmagic-videohub.yaml:1215](specs/blackmagic-videohub.yaml#L1215))
- [ ] On 12G routers, check whether CONFIGURATION (Take Mode) and TAKE MODE can be set by a client and how the two relate. ([specs/blackmagic-videohub.yaml:1224](specs/blackmagic-videohub.yaml#L1224))
- [ ] Check whether a 12G router reports more than one network interface. ([specs/blackmagic-videohub.yaml:1235](specs/blackmagic-videohub.yaml#L1235))
- [ ] On Universal Videohubs, check whether ALARM STATUS is sent and what names it uses. ([specs/blackmagic-videohub.yaml:1256](specs/blackmagic-videohub.yaml#L1256))
- [ ] Check that routers documented only for v2.3 do or do not send the v2.8 blocks. ([specs/blackmagic-videohub.yaml:1175](specs/blackmagic-videohub.yaml#L1175))
- [ ] Find each router's label length limit (64 characters assumed from a community client's Smart Videohub 40x40 and 80x80) and whether a longer label is truncated, refused with NAK or kept; check monitoring output, serial port and frame labels as well as input and output labels. ([specs/blackmagic-videohub.yaml:1191](specs/blackmagic-videohub.yaml#L1191))
- [ ] Find which firmware ends the initial status dump without END PRELUDE, and check that no router pauses for a second or more within its dump (Videohub Server computers serving routers over USB included), which would mark device.prelude_complete early. ([specs/blackmagic-videohub.yaml:1243](specs/blackmagic-videohub.yaml#L1243))
- [ ] Check how the router answers a route, label or lock change to a port locked by another client (NAK for a route, per a community client), and whether its own client's lock (O) blocks other clients' label changes as well as routes. ([specs/blackmagic-videohub.yaml:1130](specs/blackmagic-videohub.yaml#L1130))
- [ ] Check that an empty label clears it, and that the router then reports the port number alone ("3 ", or "3" with no space). ([specs/blackmagic-videohub.yaml:1191](specs/blackmagic-videohub.yaml#L1191))
- [ ] Confirm routers advertise `_blackmagic._tcp` with TXT `class=Videohub` (Companion's Videohub module) ([crates/core/src/mdns.rs:130](crates/core/src/mdns.rs#L130)), and whether any also advertise `_videohub._tcp`, which no public source confirms and discovery does not ask for.
- [ ] Record what the unit advertises over mDNS (`dns-sd -B _blackmagic._tcp` and `dns-sd -L <instance> _blackmagic._tcp`, or `avahi-browse -rt _blackmagic._tcp`): every service type, the SRV port and the TXT keys and values. Only the service type and TXT `class` come from a public source (Bitfocus Companion's module manifests); the SRV port and the other TXT keys are not documented anywhere. ([crates/core/src/mdns.rs:115](crates/core/src/mdns.rs#L115))
- [ ] Confirm TXT `name` is the product name (as Companion's MultiView 4 filter relies on), not a label the user sets: discovery narrows `models` to the catalogue model with that name. ([crates/core/src/mdns.rs:471](crates/core/src/mdns.rs#L471))

## blackmagic-videohub-leitch — Blackmagic Videohub (RS-422, Leitch protocol)

- [ ] Confirm the ">" prompt is the only answer to every command, comes after the S: or V: lines a query returns, and that an unrecognized or out-of-range command gets no other answer (Blackmagic documents none). ([specs/blackmagic-videohub-leitch.yaml:58](specs/blackmagic-videohub-leitch.yaml#L58))
- [ ] Confirm the bare CR on connecting is answered by ">" and that nothing else (a banner) is sent. ([specs/blackmagic-videohub-leitch.yaml:66](specs/blackmagic-videohub-leitch.yaml#L66))
- [ ] Record the S: and V: notifications as sent: the hex digits' case, any zero padding, and whether a route change made by this connection is notified with status reporting off. ([specs/blackmagic-videohub-leitch.yaml:284](specs/blackmagic-videohub-leitch.yaml#L284))
- [ ] Confirm the RS-422 port needs no flow control, which Blackmagic leaves unstated. ([specs/blackmagic-videohub-leitch.yaml:107](specs/blackmagic-videohub-leitch.yaml#L107))
- [ ] Confirm which models have an RS-422 router control port that answers in Leitch server mode (Smart Videohub 12x12/20x20/40x40, CleanSwitch 12x12, Universal Videohub 72/288 from the manuals; older Smart Videohub, Broadcast and Studio Videohub not checked) and the connector each uses. ([specs/blackmagic-videohub-leitch.yaml:112](specs/blackmagic-videohub-leitch.yaml#L112))
- [ ] Confirm a read-back right after a route (`@ X?`) reports the new source, and that @ B:R behaves as @ B:C. ([specs/blackmagic-videohub-leitch.yaml:160](specs/blackmagic-videohub-leitch.yaml#L160))

## blustream-matrix — Blustream matrix switchers

- [ ] Record what each model answers to a command (Blustream documents none; a [SUCCESS] line and a prompt are reported). ([specs/blustream-matrix.yaml:17](specs/blustream-matrix.yaml#L17))
- [ ] Record the STATUS report of each model other than the PLA88CS, and check the table rules read it. ([specs/blustream-matrix.yaml:1449](specs/blustream-matrix.yaml#L1449))
- [ ] Confirm RESET followed at once by Yes resets the matrix (the help text asks for Yes to confirm). ([specs/blustream-matrix.yaml:554](specs/blustream-matrix.yaml#L554))
- [ ] Confirm the HDBaseT models accept OUTxx FR yy with a space before FR, as their help text writes it, and the CMX models OUT xx FR yy. ([specs/blustream-matrix.yaml:327](specs/blustream-matrix.yaml#L327))
- [ ] Confirm the HMX and HMXL models take MUTE On/Off and AUDOUT xx On/Off in that case. ([specs/blustream-matrix.yaml:691](specs/blustream-matrix.yaml#L691))
- [ ] Confirm the RS-232 factory default of the CMX, MX and Pro models (57600 assumed from the HDBaseT models and the CMX88CS-V2). ([specs/blustream-matrix.yaml:92](specs/blustream-matrix.yaml#L92))
- [ ] Check the CECUSERCMD form with a space before the closing bracket, as the help text example writes it. ([specs/blustream-matrix.yaml:1173](specs/blustream-matrix.yaml#L1173))
- [ ] Record what the CTRLSTA report looks like on models other than the PLA88CS. ([specs/blustream-matrix.yaml:1292](specs/blustream-matrix.yaml#L1292))
- [ ] Confirm the [SUCCESS] routing line text on current firmware. ([specs/blustream-matrix.yaml:1505](specs/blustream-matrix.yaml#L1505))

## boinx-mimolive — Boinx mimoLive

- [ ] Confirm that writes with Content-Type application/vnd.api+json are accepted (Boinx's reference requires it; the manual's examples send application/json). ([specs/boinx-mimolive.yaml:2091](specs/boinx-mimolive.yaml#L2091))
- [ ] Check the body shape PUT /documents/(DocumentID) takes for programOutputMasterVolume (a plain object assumed, as for layers). ([specs/boinx-mimolive.yaml:208](specs/boinx-mimolive.yaml#L208))
- [ ] Check which body POST /documents/(DocumentID)/layers takes: the manual's layer-identifier, name and index, or the reference's data.attributes.composition-id. ([specs/boinx-mimolive.yaml:2107](specs/boinx-mimolive.yaml#L2107))
- [ ] Confirm that GET /documents?include=layers (and sources, output-destinations, layer-sets) sideloads every document's objects in included, as the poll reads them. ([specs/boinx-mimolive.yaml:1598](specs/boinx-mimolive.yaml#L1598))
- [ ] Record the websocket's added, changed and removed messages: that data is the whole object as the HTTP API returns it, and that a removed message carries type and id at the top level. ([specs/boinx-mimolive.yaml:2015](specs/boinx-mimolive.yaml#L2015))
- [ ] Check whether closing a document also pushes the removal of its layers, variants and sources; only what mimoLive reports removed leaves state. ([specs/boinx-mimolive.yaml:2015](specs/boinx-mimolive.yaml#L2015))
- [ ] Confirm that the websocket closes a client that sends nothing for 15 seconds and that {"event":"ping"} every 5 seconds keeps it open. ([specs/boinx-mimolive.yaml:1587](specs/boinx-mimolive.yaml#L1587))
- [ ] With a remote control password set, confirm that the X-MimoLive-Password-SHA256 header is accepted on the websocket's opening request, and that a wrong key answers 401 on HTTP. ([specs/boinx-mimolive.yaml:102](specs/boinx-mimolive.yaml#L102))
- [ ] Check that output destination PATCH bodies are accepted with type and id, and layer set PATCH bodies without them. ([specs/boinx-mimolive.yaml:953](specs/boinx-mimolive.yaml#L953))
- [ ] Check that layer set PATCH and POST bodies take the JSON:API data.attributes form shown in the manual. ([specs/boinx-mimolive.yaml:1106](specs/boinx-mimolive.yaml#L1106))
- [ ] Check that a number input accepts four decimals and that set_layer_flag's JSON true and false are taken for a bool input. ([specs/boinx-mimolive.yaml:438](specs/boinx-mimolive.yaml#L438))
- [ ] Confirm the built-in outputs record, stream, playout and fullscreen in the document's outputs attribute, and their live-state values. ([specs/boinx-mimolive.yaml:308](specs/boinx-mimolive.yaml#L308))
- [ ] Check that comments/new takes its parameters in the query of a POST. ([specs/boinx-mimolive.yaml:1280](specs/boinx-mimolive.yaml#L1280))
- [ ] Check what a layer's live-variant relationship holds while the layer is off (null assumed, which leaves live_variant as it was). ([specs/boinx-mimolive.yaml:1481](specs/boinx-mimolive.yaml#L1481))
- [ ] Check whether GET /documents/(DocumentID)/layer-sets and the poll's include=layer-sets carry each set's layers attribute (and recall-on-show-start and recall-on-show-end), or only a GET of one set does; a set answered without layers keeps the list it had. ([specs/boinx-mimolive.yaml:1845](specs/boinx-mimolive.yaml#L1845))
- [ ] Record the websocket's added and changed messages for a layer set: whether data carries layers, and whether it carries relationships.document, which the re-read of the set needs. ([specs/boinx-mimolive.yaml:2010](specs/boinx-mimolive.yaml#L2010))
- [ ] Check whether one layer can appear twice in a layer set's layers, and in what order mimoLive returns the entries; state keeps them by position as returned. ([specs/boinx-mimolive.yaml:1545](specs/boinx-mimolive.yaml#L1545))
- [ ] Record what PATCH .../layer-sets/(LayerSetID) answers to a layers change (status 200 assumed; the body is not needed, since the set is read back after any 2xx answer), and whether mimoLive keeps the list as written. ([specs/boinx-mimolive.yaml:1116](specs/boinx-mimolive.yaml#L1116))
- [ ] Check that PUT /documents/(DocumentID) with {"name": ...} renames the document, and whether it renames the .tvshow file or only the name mimoLive shows. ([specs/boinx-mimolive.yaml:217](specs/boinx-mimolive.yaml#L217))
- [ ] Check that the show metadata is written as {"metadata": {...}} with only the changed fields (a PUT naming title alone leaves author, comments and the rest as they were), as Boinx's reference lists metadata among the modifiable attributes. ([specs/boinx-mimolive.yaml:226](specs/boinx-mimolive.yaml#L226))
- [ ] Check that width, height, framerate and samplerate in the metadata can be written, which values mimoLive accepts (a frame rate with three decimals, such as 29.970), and what happens to a running show, recording or stream when they change. ([specs/boinx-mimolive.yaml:271](specs/boinx-mimolive.yaml#L271))
- [ ] Check that POST /documents/(DocumentID)/layers applies input-values given with layer-identifier, index and name, as the manual says initial input values can be set. ([specs/boinx-mimolive.yaml:512](specs/boinx-mimolive.yaml#L512))
- [ ] Check that the mimoCall attributes video-codec, prefers-high-quality-audio, partner-sees and partner-hears are taken by PATCH with a plain body, as the manual's example sends them. ([specs/boinx-mimolive.yaml:738](specs/boinx-mimolive.yaml#L738))
- [ ] Record which body POST /documents/(DocumentID)/sources and POST .../output-destinations take: the manual's plain fields (source-type, name, output-destination-type, index) or the reference's data.attributes form with settings; the *_with_attributes commands send the object given. ([specs/boinx-mimolive.yaml:858](specs/boinx-mimolive.yaml#L858))
- [ ] Check that settings.publicurl can be written by PATCH (the manual lists it among the readable settings and says the settings' contents are modifiable), and that it reads back unobfuscated. ([specs/boinx-mimolive.yaml:1014](specs/boinx-mimolive.yaml#L1014))
- [ ] Check that a PATCH with settings.location or settings.filename null resets it to mimoLive's default, as the Data Types page says. ([specs/boinx-mimolive.yaml:1024](specs/boinx-mimolive.yaml#L1024))
- [ ] Check that comments/new takes date as an ISO 8601 timestamp with a Z or offset, favorite as true, and userimageurl percent-encoded in the query of a POST. ([specs/boinx-mimolive.yaml:1290](specs/boinx-mimolive.yaml#L1290))
- [ ] Check that zoom/join takes webinartoken without a passcode, and virtualcamera=false to join without sending the program out. ([specs/boinx-mimolive.yaml:1324](specs/boinx-mimolive.yaml#L1324))
- [ ] Check that input-values and output-values come in list replies, sideloaded objects and websocket pushes, not only in a GET of one object, and how often output-values are pushed while a layer renders. ([specs/boinx-mimolive.yaml:1482](specs/boinx-mimolive.yaml#L1482))
- [ ] Record the types of the source attributes is-hidden, is-static, filepath, zoom-userid, zoom-userselectiontype and zoom-videoresolution (a string assumed for the resolution), and on which source types each appears. ([specs/boinx-mimolive.yaml:1511](specs/boinx-mimolive.yaml#L1511))
- [ ] Check whether mimoLive pushes added, changed and removed messages for filters on the websocket with type filters; without them a filter's state follows list_filters, get_source and get_filter replies only. ([specs/boinx-mimolive.yaml:1521](specs/boinx-mimolive.yaml#L1521))
- [ ] Check that GET /devices lists every device with connected, video, audio, device-type, tally-state and, for audio devices, input-channels, and record the shape of device IDs (each is kept as one state key, dots included). ([specs/boinx-mimolive.yaml:1554](specs/boinx-mimolive.yaml#L1554))
- [ ] Check what GET /accounts answers before mimoLive 6.19 (a 404 assumed, which leaves the accounts empty) and whether it lists accounts of every service. ([specs/boinx-mimolive.yaml:1603](specs/boinx-mimolive.yaml#L1603))
- [ ] Check that zoom/participants answers {"data": []} with no meeting joined (not 409), that each participant's id is a number, and how soon after zoom/join the participants are listed. ([specs/boinx-mimolive.yaml:1568](specs/boinx-mimolive.yaml#L1568))
- [ ] With mlController installed, check its API on port 8990: the status object, that start, stop, restart, open and select answer 200, that /api/open opens a .tvshow by its full path, and that /api/select takes an empty path for the system's default; with its password set, that it answers 401 to every route. ([specs/boinx-mimolive.yaml:108](specs/boinx-mimolive.yaml#L108))
- [ ] Record what GET /layertypes, /sourcetypes and /outputdestinationtypes answer: a JSON:API data array whose elements carry the type's identifier as id and its name as attributes.name is assumed (only the id is documented), and whether the list carries each type's parameters and settings or only a GET of one type does. ([specs/boinx-mimolive.yaml:1918](specs/boinx-mimolive.yaml#L1918))
- [ ] Record what GET /layertypes/(TypeID) answers: a JSON:API object with the type's identifier as id and its inputs as a layer reports them (input-descriptions, and input-values holding the defaults) is assumed, since Boinx's reference says only that it includes the input parameters. ([specs/boinx-mimolive.yaml:1250](specs/boinx-mimolive.yaml#L1250))
- [ ] Check that input-descriptions comes in list replies, sideloaded objects and websocket pushes of layers, variants, sources and filters, not only in a GET of one object, and in the shape the Data Types page gives (type, label, hidden, index, value-min, value-max, value-step, value-unit, value-list). ([specs/boinx-mimolive.yaml:1483](specs/boinx-mimolive.yaml#L1483))
- [ ] Check whether a camera or device source's GET, list entry, sideloaded object or websocket push reports video-device-id, video-device-name, audio-device-id, audio-device-name or channel-map (none is documented as reported, so state does not hold them); if they are, add them to the source state. ([specs/boinx-mimolive.yaml:2164](specs/boinx-mimolive.yaml#L2164))

## boxcast — BoxCast

- [ ] Confirm the client credentials grant (HTTP Basic client id and secret to auth.boxcast.com/oauth2/token, grant_type client_credentials) answers an access token, how long it lasts in steady use, and that an expired or revoked token answers 401 (not 403). ([specs/boxcast.yaml:66](specs/boxcast.yaml#L66))
- [ ] Confirm JSON request bodies are accepted for every POST and PUT, and record the status Create calls answer (the reference shows 200 with the resource; uploads 201). ([specs/boxcast.yaml:228](specs/boxcast.yaml#L228))
- [ ] Confirm the poll's search q=timeframe:preroll timeframe:current answers both kinds (space as OR) and that list items carry timeframe, which the reference's summary examples leave out. ([specs/boxcast.yaml:205](specs/boxcast.yaml#L205))
- [ ] Confirm s=name and l=100 are accepted on the BoxCaster, channel and RTMP source lists, and record whether those lists page (X-Pagination) beyond a default size. ([specs/boxcast.yaml:438](specs/boxcast.yaml#L438))
- [ ] Confirm stops_at now in Update a Broadcast ends a live broadcast at once (the now keyword is documented for Schedule a Broadcast only), and likewise starts_at now. ([specs/boxcast.yaml:332](specs/boxcast.yaml#L332))
- [ ] Confirm DELETE of a broadcast, highlight and RTMP source answers 200 with an empty body (the state rules rely on it). ([specs/boxcast.yaml:1094](specs/boxcast.yaml#L1094))
- [ ] Record whether broadcast answers include audio_source and recording_id (Get Broadcast Detail names recording_id only in the download guide). ([specs/boxcast.yaml:1070](specs/boxcast.yaml#L1070))
- [ ] Confirm the download preparation takes audio_only as JSON and answers 201, and the download_status values (processing:N). ([specs/boxcast.yaml:414](specs/boxcast.yaml#L414))
- [ ] Confirm /account/viewer_analytics/{id} serves broadcasts and channels alike with since, until and include_viewer_timeseries as query parameters (the reference's examples join them with &). ([specs/boxcast.yaml:743](specs/boxcast.yaml#L743))
- [ ] Confirm Update a Channel takes description (not documented) and whether channel lists include description. ([specs/boxcast.yaml:854](specs/boxcast.yaml#L854))

## brightsign-dws — BrightSign players (Local DWS)

- [ ] Confirm a path with slashes in /api/v1/files/<path>/ (each segment percent-encoded) lists, reads, renames and deletes nested files. ([specs/brightsign-dws.yaml:671](specs/brightsign-dws.yaml#L671))
- [ ] Confirm an upload as one multipart part named file, with the file's name as its filename, lands in the folder given in the path. ([specs/brightsign-dws.yaml:244](specs/brightsign-dws.yaml#L244))
- [ ] Check whether the telnet and ssh answers come wrapped in data.result or bare, as the documentation shows them. ([specs/brightsign-dws.yaml:684](specs/brightsign-dws.yaml#L684))
- [ ] Check the display power setting values beyond on and standby, the video output names, and the brightness, contrast, volume and white-balance ranges on a Moka display. ([specs/brightsign-dws.yaml:607](specs/brightsign-dws.yaml#L607))
- [ ] Confirm the file systems format_storage takes (exfat by default) and the supervisor logging level range. ([specs/brightsign-dws.yaml:264](specs/brightsign-dws.yaml#L264))

## brightsign-udp — BrightSign players (UDP)

- [ ] Confirm the default UDP Receiver Port of a new BrightSign Author presentation (5000 is assumed). ([specs/brightsign-udp.yaml:62](specs/brightsign-udp.yaml#L62))
- [ ] Confirm the sample control script's command texts (PLAY, PLAYCL, LOOP, LOOPS, STOP, STOPCL, PAUSE, RESUME, SEARCH, VOLUME, REBOOT) on its current version. ([specs/brightsign-udp.yaml:108](specs/brightsign-udp.yaml#L108))
- [ ] Confirm name:value and name:value!!name:value set user variables through an Input-specifies-variable UDP event. ([specs/brightsign-udp.yaml:101](specs/brightsign-udp.yaml#L101))
- [ ] Confirm that BrightSign's current sample control script still broadcasts its responses to UDP 5001 with the v3.01E codes (PLAY, STOP, ENDP, READY...) and STATUS texts read here. ([specs/brightsign-udp.yaml:49](specs/brightsign-udp.yaml#L49))
- [ ] Check that a response broadcast to 255.255.255.255:5001 comes from the player's own address, so it is taken by the device opened at that address. ([specs/brightsign-udp.yaml:194](specs/brightsign-udp.yaml#L194))

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

## camstreamer — CamStreamer apps for Axis cameras

- [ ] Confirm the 6.x stream list's path (/local/camstreamer/stream_list.cgi; the page also writes /local/camstreamer/stream/stream_list.cgi for one stream) and whether its answer is the bare {"streamList": [...]} or wrapped in {status, message, data}. ([specs/camstreamer.yaml:744](specs/camstreamer.yaml#L744))
- [ ] Confirm set_stream_enabled.cgi takes stream_id and enabled (the page writes stream_id=?stream_id=), on 6.x and 4.x. ([specs/camstreamer.yaml:164](specs/camstreamer.yaml#L164))
- [ ] Confirm set_stream's body is one stream object (the page shows only the whole streamList). ([specs/camstreamer.yaml:155](specs/camstreamer.yaml#L155))
- [ ] Record how an unlicensed app answers its API calls (status and body). ([specs/camstreamer.yaml:531](specs/camstreamer.yaml#L531))
- [ ] Confirm param.cgi lists CamOverlay.Enabled and camswitcher.masterAudio as root.CamOverlay.Enabled= and root.camswitcher.masterAudio= lines (the group names' case as the page writes them). ([specs/camstreamer.yaml:797](specs/camstreamer.yaml#L797))
- [ ] Confirm CamScripter's PackageConfigurations parameter is camscripter.PackageConfigurations in param.cgi, and network_camera_list.cgi's path under /local/camscripter/. ([specs/camstreamer.yaml:499](specs/camstreamer.yaml#L499))
- [ ] Record the shape of playlist_queue_list.cgi's and clips.cgi's answers, to keep the queue in state. ([specs/camstreamer.yaml:397](specs/camstreamer.yaml#L397))
- [ ] Confirm upload_custom_image takes the picture as the raw POST body (as curl --data-binary sends it). ([specs/camstreamer.yaml:359](specs/camstreamer.yaml#L359))
- [ ] Check that the apps' events websockets answer on the camera's HTTP port as ws (the document gives wss only) and take the subprotocol events. ([specs/camstreamer.yaml:750](specs/camstreamer.yaml#L750))
- [ ] Confirm that ws_authorization.cgi takes the camera's Digest credential and answers valid JSON with the token in data (CamStreamer 6.x, CamSwitcher) or message (CamStreamer 4.x, CamOverlay; the document's 4.x and CamOverlay examples have a trailing comma). ([specs/camstreamer.yaml:759](specs/camstreamer.yaml#L759))
- [ ] Check that the first message {"authorization": token} is answered {"type":"authorization","state":"OK"} and followed by the init messages on every app, and that CamStreamer's StreamState values are 0/1 numbers. ([specs/camstreamer.yaml:754](specs/camstreamer.yaml#L754))
- [ ] Record whether CamSwitcher sends an event when it stops switching (none is documented, so switcher.started is never set false). ([specs/camstreamer.yaml:669](specs/camstreamer.yaml#L669))

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

## casparcg — CasparCG Server

- [ ] Confirm the reply framing on 2.3 and later: 200 followed by lines and a blank line, 201 and 400 by exactly one line, 202 and the other errors alone, including for a 200 that lists nothing. ([specs/casparcg.yaml:1450](specs/casparcg.yaml#L1450))
- [ ] Check that LOADBG, LOAD and PLAY accept the transition sent in full when none is wanted (CUT 0 linear RIGHT) and SEEK 0, and that LOOP and AUTO may sit where they are sent. ([specs/casparcg.yaml:1466](specs/casparcg.yaml#L1466))
- [ ] Confirm that 2.3 and later send OSC to each AMCP client's address on the configured default-port (6250), with no predefined client needed. ([specs/casparcg.yaml:1474](specs/casparcg.yaml#L1474))
- [ ] Record the OSC addresses a 2.3 or later server sends for a layer playing a file, an image and an HTML template (with or without a foreground/ segment, and whether background, paused, loop, host and profiler messages still exist), and fix the rules to match. ([specs/casparcg.yaml:1486](specs/casparcg.yaml#L1486))
- [ ] Check where DEFER goes in a MIXER command with a tween (after the tween, as sent here) and what a read answers for a deferred change before MIXER COMMIT. ([specs/casparcg.yaml:1508](specs/casparcg.yaml#L1508))
- [ ] Check the code and layout of MIXER CHROMA's read answer (201 and nine values, or the example's 202) and of the other MIXER reads. ([specs/casparcg.yaml:1513](specs/casparcg.yaml#L1513))
- [ ] Check what CALL LOOP, CALL SEEK, START and LENGTH answer (201 CALL OK and a value, or 202). ([specs/casparcg.yaml:1572](specs/casparcg.yaml#L1572))
- [ ] Check PING's answer on 2.3 and later (PONG alone, or with a code). ([specs/casparcg.yaml:1579](specs/casparcg.yaml#L1579))
- [ ] Record TLS, FLS and DATA LIST answers on 2.3 and later, so their lines can be read into fields. ([specs/casparcg.yaml:1523](specs/casparcg.yaml#L1523))
- [ ] Confirm CG ADD, PLAY, STOP, REMOVE and CLEAR answer 202 CG OK, LOADBG 202 LOADBG OK and a bare PLAY 202 PLAY OK, so the template and background state is kept. ([specs/casparcg.yaml:1530](specs/casparcg.yaml#L1530))
- [ ] Record the XML CG INFO and INFO <channel>-<layer> answer on 2.3 and later, so templates loaded by other clients can be read into state. ([specs/casparcg.yaml:1530](specs/casparcg.yaml#L1530))

## castr — Castr

- [ ] Confirm HTTP Basic with the access token's ID and secret key is accepted, and that a wrong key answers 401 (not 403). ([specs/castr.yaml:98](specs/castr.yaml#L98))
- [ ] Confirm GET /v2/live_streams/{id} as the idle probe is cheap and record any rate limits Castr applies. ([specs/castr.yaml:103](specs/castr.yaml#L103))
- [ ] Confirm PATCH /v2/live_streams/{id} with a body of enabled alone changes only that field, and that enabled false cuts the encoder off at once. ([specs/castr.yaml:245](specs/castr.yaml#L245))
- [ ] Confirm a PATCH whose settings object holds one field (cloud_recording, abr, low_latency_playback, chat_enabled) keeps the stream's other settings, despite the HTTP Verbs page calling PATCH a replacement. ([specs/castr.yaml:257](specs/castr.yaml#L257))
- [ ] Confirm PATCH .../platforms/{platform_id} with metadata alone changes a linked Facebook or YouTube title and description (the guide's example also sends name and enabled false). ([specs/castr.yaml:421](specs/castr.yaml#L421))
- [ ] Confirm PATCH .../platforms/{platform_id} with enabled alone pauses and resumes a target while live, and that it answers the whole stream. ([specs/castr.yaml:406](specs/castr.yaml#L406))
- [ ] Record the status codes Create, Add Platform and the platform PATCH answer (the reference says 200; the guide's create example does not say) and confirm Create answers the stream with _id. ([specs/castr.yaml:191](specs/castr.yaml#L191))
- [ ] Confirm DELETE of a stream and of a platform answers 200 with success true. ([specs/castr.yaml:365](specs/castr.yaml#L365))
- [ ] Confirm GET .../stats answers 404 while the stream is offline, and record its bitrate's unit (kbps assumed) and whether opened_at is Unix milliseconds. ([specs/castr.yaml:462](specs/castr.yaml#L462))
- [ ] Confirm broadcasting_status is reported for the stream and each platform in GET /v2/live_streams/{id}, and how soon it follows the encoder starting and stopping. ([specs/castr.yaml:1071](specs/castr.yaml#L1071))
- [ ] Confirm the add_platform body (template custom, name, server, key, enabled) is accepted and the target's id is a 24-character hexadecimal id. ([specs/castr.yaml:373](specs/castr.yaml#L373))
- [ ] Record what Convert Live-to-VOD answers, and confirm from takes an ISO 8601 UTC time inside the recording. ([specs/castr.yaml:473](specs/castr.yaml#L473))
- [ ] Confirm list_streams' page and limit parameters and the largest limit Castr accepts (100 assumed). ([specs/castr.yaml:147](specs/castr.yaml#L147))
- [ ] Confirm the stream's platforms list holds every multistream target, and that adding or changing a platform answers the whole stream: each such answer replaces the stream's platforms. ([specs/castr.yaml:1070](specs/castr.yaml#L1070))
- [ ] Confirm varying_rewind is accepted as a JSON number from the documented set, and that the stream answers it, the country and domain lists and ll_embed_url in settings and playback. ([specs/castr.yaml:305](specs/castr.yaml#L305))
- [ ] Confirm an empty embed_password removes the player password. ([specs/castr.yaml:317](specs/castr.yaml#L317))
- [ ] Confirm the country codes country_block and country_whitelist take (ISO 3166-1 alpha-2 assumed), and which wins when both are set. ([specs/castr.yaml:329](specs/castr.yaml#L329))
- [ ] Confirm GET /v2/sub_second_streams answers a plain array with broadcasting_status, and that limit 100 is accepted. ([specs/castr.yaml:1029](specs/castr.yaml#L1029))
- [ ] Confirm sub-second stream create, update and get answer the stream object, and delete answers success true. ([specs/castr.yaml:536](specs/castr.yaml#L536))
- [ ] Record the shape of List folders' answer: an object with total, page, pages and docs, or that object inside an array as the reference shows. ([specs/castr.yaml:603](specs/castr.yaml#L603))
- [ ] Confirm Update folder and Create a folder answer the folder with _id, and Delete folder success true; record whether deleting a folder deletes its videos. ([specs/castr.yaml:660](specs/castr.yaml#L660))
- [ ] Record what Create direct upload URL answers (url and fields expected, as in the guide) and that customFileName names the file. ([specs/castr.yaml:719](specs/castr.yaml#L719))
- [ ] Confirm Move content to subfolder takes subfolder_id null to move a video back to the root. ([specs/castr.yaml:709](specs/castr.yaml#L709))
- [ ] Confirm List subfolders answers a plain array, and that Rename answers success true and Delete removed and removedVideos. ([specs/castr.yaml:737](specs/castr.yaml#L737))
- [ ] Record what the activity logs and events endpoints answer, whether they take the Basic credential or need the auth-token header, and how a video's id differs from a stream's on the same path. ([specs/castr.yaml:833](specs/castr.yaml#L833))

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
- [ ] Confirm commands sent to the console's address (not broadcast) are received. ([specs/chamsys-magicq-udp.yaml:798](specs/chamsys-magicq-udp.yaml#L798))
- [ ] Check the highest playback number the console accepts over the remote protocol (1-202, or 34 on consoles and 10 on PC). ([specs/chamsys-magicq-udp.yaml:813](specs/chamsys-magicq-udp.yaml#L813))
- [ ] Check the bounds chosen where the manual is silent: attribute values 0-65535, times 0-3600 s, cue stack ids 1-10000, cue ids 1-65536. ([specs/chamsys-magicq-udp.yaml:821](specs/chamsys-magicq-udp.yaml#L821))
- [ ] On QuickQ, check the jump cue form (whole numbers only) and the X zone range 0-10. ([specs/chamsys-magicq-udp.yaml:843](specs/chamsys-magicq-udp.yaml#L843))
- [ ] Check whether QuickQ DIN accepts remote control. ([specs/chamsys-magicq-udp.yaml:859](specs/chamsys-magicq-udp.yaml#L859))
- [ ] On MQ40 and MQ40N, confirm the remote protocol works. ([specs/chamsys-magicq-udp.yaml:837](specs/chamsys-magicq-udp.yaml#L837))

## characterworks — CharacterWorks

- [ ] Confirm the request path (/) and content type (application/json) the Remote Control Server accepts. ([specs/characterworks.yaml:348](specs/characterworks.yaml#L348))
- [ ] Record what a command does without a channel, and whether other channel names (more live channels) exist. ([specs/characterworks.yaml:339](specs/characterworks.yaml#L339))
- [ ] Confirm the row selection action is select_data_source_rows (the document's examples write source). ([specs/characterworks.yaml:344](specs/characterworks.yaml#L344))
- [ ] Record the HTTP status of a failure reply (the spec reads the result field, whatever the status). ([specs/characterworks.yaml:329](specs/characterworks.yaml#L329))

## christie-1dlp — Christie 1DLP projectors

- [ ] Check that a set sent with the "#" prefix is answered by its echo over TCP, and whether the echo keeps the "#" ("(#PWR 1)") or drops it ("(PWR 1)"). ([specs/christie-1dlp.yaml:479](specs/christie-1dlp.yaml#L479))
- [ ] Check that the echo repeats the value as sent (for example "(BRT 50)" for "#BRT 50") and not reformatted or zero-padded. ([specs/christie-1dlp.yaml:703](specs/christie-1dlp.yaml#L703))
- [ ] Check how long the "#PWR 1" and "#PWR 0" acknowledgments take (immediately or after warm-up/cool-down), against the 5 s timeout. ([specs/christie-1dlp.yaml:71](specs/christie-1dlp.yaml#L71))
- [ ] Check the request reply format "(PWR!001 \"On\")": zero padding, the quoted text, and the 10 (cooling) and 11 (warming up) values. ([specs/christie-1dlp.yaml:486](specs/christie-1dlp.yaml#L486))
- [ ] Check that codes with no documented request answer the general form (FRZ?, LPP?, LPM?, SIN+MAIN?, BRT?) and in what format. ([specs/christie-1dlp.yaml:646](specs/christie-1dlp.yaml#L646))
- [ ] Check SIN+MAIN? on the DLP families and the E Series and SIN? on the Captiva, and that the reply is "(SIN+MAIN!003 ...)" or "(SIN!003 ...)". ([specs/christie-1dlp.yaml:606](specs/christie-1dlp.yaml#L606))
- [ ] Record the SST? reply on the laser families: one message or several "(SST+<group>!nnn ...)" items, and their format. ([specs/christie-1dlp.yaml:498](specs/christie-1dlp.yaml#L498))
- [ ] Check the SST+<group> item shape this spec assumes from code for Christie's larger projectors. ([specs/christie-1dlp.yaml:3178](specs/christie-1dlp.yaml#L3178))
- [ ] Check the E Series SST? reply items "(SST!017 \"10 Hours\" \"Lamp 1 Hours\")" as documented, and whether the laser families answer the same way. ([specs/christie-1dlp.yaml:3393](specs/christie-1dlp.yaml#L3393))
- [ ] Record the ILI? reply format (projector and laser diode hours) so the hours can be parsed. ([specs/christie-1dlp.yaml:503](specs/christie-1dlp.yaml#L503))
- [ ] Record the FVI? reply format (firmware versions). ([specs/christie-1dlp.yaml:508](specs/christie-1dlp.yaml#L508))
- [ ] Record the FAN+<item>? reply format and which fan items exist per family. ([specs/christie-1dlp.yaml:548](specs/christie-1dlp.yaml#L548))
- [ ] Record the LDI+LDnn? reply format (voltage, current, temperature) and the number of diode banks per family. ([specs/christie-1dlp.yaml:555](specs/christie-1dlp.yaml#L555))
- [ ] Check that PIF+MDLN? and PIF+SNUM? are refused outside service mode and answered in it. ([specs/christie-1dlp.yaml:576](specs/christie-1dlp.yaml#L576))
- [ ] Check the error reply format "(65535 00000 ERR00005 \"...\")" over TCP, and collect other ERR codes. ([specs/christie-1dlp.yaml:74](specs/christie-1dlp.yaml#L74))
- [ ] Check that the E Series (DWU670-E) accepts these commands on TCP port 3002 (its reference names an ETHERNET port but no number). ([specs/christie-1dlp.yaml:431](specs/christie-1dlp.yaml#L431))
- [ ] Check that the Captiva DWU500S accepts these commands on TCP port 3002 (its document names no port). ([specs/christie-1dlp.yaml:459](specs/christie-1dlp.yaml#L459))
- [ ] Check that the D13/D16/D20-HS take the commands on TCP port 3002 (their document leaves the port blank). ([specs/christie-1dlp.yaml:137](specs/christie-1dlp.yaml#L137))
- [ ] Check whether the projector sends unsolicited messages (network messages, NET+SHOW 1) and what they look like. ([specs/christie-1dlp.yaml:2315](specs/christie-1dlp.yaml#L2315))
- [ ] Check that the CCA reset subcodes take "1" on the Inspire, D13/D16/D20 and 4K7/4K10 (their documents show no value). ([specs/christie-1dlp.yaml:1013](specs/christie-1dlp.yaml#L1013))
- [ ] Check the PST+USER value on the D13/D16/D20 and 4K7/4K10 (documented as "<value>" with no range; "1" is sent). ([specs/christie-1dlp.yaml:691](specs/christie-1dlp.yaml#L691))
- [ ] Check SNS+SRC: the quoted name on the Inspire, D13/D16/D20 and 4K7/4K10 (and 4K7/4K10 numbering 0 to B), and the E Series form with the input in the subcode ("SNS+SRC1 \"WUXGA\""). ([specs/christie-1dlp.yaml:1759](specs/christie-1dlp.yaml#L1759))
- [ ] Check which SD#+EVWR form the projector takes: "<event>, <time>, <function>" in quotes or the example's "01 11 28 31 42". ([specs/christie-1dlp.yaml:2237](specs/christie-1dlp.yaml#L2237))
- [ ] Check the UID service-mode syntax UID "service,<password>" (the E Series reference documents UID "username,password") against the HS example UID "service" "<password>". ([specs/christie-1dlp.yaml:2075](specs/christie-1dlp.yaml#L2075))
- [ ] Check the values the Korus GUP+ACME, ATFE, FRZE, SCHE and SHUT take (0/1 assumed) and that GUP+RSET takes no value. ([specs/christie-1dlp.yaml:2427](specs/christie-1dlp.yaml#L2427))
- [ ] Check the Captiva SOR values 0 and 1 (only 2 = rear and 3 = front ceiling are documented). ([specs/christie-1dlp.yaml:1945](specs/christie-1dlp.yaml#L1945))
- [ ] Check that ACT is taken as "ACTxy" with no space (for example ACT54). ([specs/christie-1dlp.yaml:1148](specs/christie-1dlp.yaml#L1148))
- [ ] Check that the HS Series 2K (DWU15/19/23-HS) takes DDM and DDY (its reference prints both as DDD). ([specs/christie-1dlp.yaml:2121](specs/christie-1dlp.yaml#L2121))
- [ ] Check LOC+LANG on the DLP families and the E Series and LOC on the Captiva, and the D13/D16/D20, 4K7/4K10 and E Series language order. ([specs/christie-1dlp.yaml:1896](specs/christie-1dlp.yaml#L1896))
- [ ] Check the inverted MBE+USER on the D13/D16/D20 (0 shows message boxes), as documented. ([specs/christie-1dlp.yaml:1889](specs/christie-1dlp.yaml#L1889))
- [ ] Check the inverted NET+DHCP on the D13/D16/D20 and 4K7/4K10 (0 = DHCP on). ([specs/christie-1dlp.yaml:2252](specs/christie-1dlp.yaml#L2252))
- [ ] Check the inverted HDR+HDRE on the 4K7/4K10 (0 = auto, 1 = off). ([specs/christie-1dlp.yaml:1598](specs/christie-1dlp.yaml#L1598))
- [ ] Check the KEY code tables per family (HS/GS/Jazz/Korus differ; E Series 0 to 72 per its Appendix 2; Captiva 1 to 10). ([specs/christie-1dlp.yaml:2053](specs/christie-1dlp.yaml#L2053))
- [ ] Check the 4K7/4K10 edge blend width limits (right edge up to 6000 as printed, left 1600, top and bottom 1000). ([specs/christie-1dlp.yaml:1403](specs/christie-1dlp.yaml#L1403))
- [ ] Check the backup input numbering on the HS Series 2K (0 = HDMI1 to 3 = HDBaseT) against the other families (3 = HDMI1 to 8 = HDBaseT). ([specs/christie-1dlp.yaml:1738](specs/christie-1dlp.yaml#L1738))
- [ ] Check whether FCT+SERN (set serial number) is accepted outside service mode. ([specs/christie-1dlp.yaml:2083](specs/christie-1dlp.yaml#L2083))
- [ ] Check BAC+REST and BAC+SAVE with no record on the D13/D16/D20 (as their document shows). ([specs/christie-1dlp.yaml:2104](specs/christie-1dlp.yaml#L2104))
- [ ] Check that LPM on the E Series sets lamp power (0 = 280 W to 10 = 330 W) and what LPM? answers. ([specs/christie-1dlp.yaml:973](specs/christie-1dlp.yaml#L973))
- [ ] Check the E Series lamp 2 hour reset subcode (LPC+LMP2 in the subcode list, LPC+LAMP2 in the example; LMP2 is sent). ([specs/christie-1dlp.yaml:2611](specs/christie-1dlp.yaml#L2611))
- [ ] Check the E Series LIF+<item>? replies as plain numbers ("(LIF+LP1H!534)"), including the LP1R and LP2R reset counts. ([specs/christie-1dlp.yaml:2597](specs/christie-1dlp.yaml#L2597))
- [ ] Check the E Series upper limits of LSF+TIME and LPL (none documented; 99999 is allowed). ([specs/christie-1dlp.yaml:2590](specs/christie-1dlp.yaml#L2590))
- [ ] Check the inverted E Series ESH (0 = number keys select sources, 1 = off). ([specs/christie-1dlp.yaml:2549](specs/christie-1dlp.yaml#L2549))
- [ ] Check the E Series BCL values beyond 0 and 1 (the reference text mentions 1 to 10 as a wish). ([specs/christie-1dlp.yaml:2528](specs/christie-1dlp.yaml#L2528))
- [ ] Check whether the E Series SKS (marked READ ONLY but with values 0 to 2) accepts a set. ([specs/christie-1dlp.yaml:1724](specs/christie-1dlp.yaml#L1724))

## christie-pandorasbox — Christie Pandoras Box

- [ ] Confirm doubles go little-endian (as the vendor SDKs send them), not in network order as the protocol page says, with set_param_double on a known parameter. ([specs/christie-pandorasbox.yaml:2344](specs/christie-pandorasbox.yaml#L2344))
- [ ] Confirm functions without return values are never answered, and how a refused one is signalled. ([specs/christie-pandorasbox.yaml:2337](specs/christie-pandorasbox.yaml#L2337))
- [ ] Confirm a refused function answers a negative code followed by an int error number, and what the numbers mean. ([specs/christie-pandorasbox.yaml:2337](specs/christie-pandorasbox.yaml#L2337))
- [ ] Check the int buffer of GetSiteIds and GetPlaylistEntryIndices* carries an int count first (the C# SDK reads none). ([specs/christie-pandorasbox.yaml:2321](specs/christie-pandorasbox.yaml#L2321))
- [ ] Check which Pandoras Box versions answer GetCurrentTimeCueInfo (295); older ones are taken to refuse it. ([specs/christie-pandorasbox.yaml:2352](specs/christie-pandorasbox.yaml#L2352))

## christie-spyder — Christie Spyder X20 / X80

- [ ] Confirm that answers come back to the sending port with no "spyder" header, the result code first. ([specs/christie-spyder.yaml:46](specs/christie-spyder.yaml#L46))
- [ ] Record the exact answers to RLC, RSN, RBL, RRL, RLK and RCS (spacing, trailing characters), which the get_ commands return as text. ([specs/christie-spyder.yaml:731](specs/christie-spyder.yaml#L731))
- [ ] Check that Spyder decodes percent-encoding other than %20 in names (such as %28 for a parenthesis). ([specs/christie-spyder.yaml:1090](specs/christie-spyder.yaml#L1090))
- [ ] Check what learn_command_key answers (command key ID and script ID) on X20 and X80. ([specs/christie-spyder.yaml:146](specs/christie-spyder.yaml#L146))
- [ ] Check whether an X20 accepts ILA with the fifth (gamma) argument, which only the X80 reference lists. ([specs/christie-spyder.yaml:384](specs/christie-spyder.yaml#L384))
- [ ] Check how often UDP commands or answers are lost on a busy network, and whether a repeated command is harmless. ([specs/christie-spyder.yaml:1134](specs/christie-spyder.yaml#L1134))
- [ ] Record an RLK answer: whether its values are integers or decimals, how Outside Edges is written in hexadecimal (f, 0xF), and that the 33 values come in the documented order. ([specs/christie-spyder.yaml:915](specs/christie-spyder.yaml#L915))
- [ ] Check that RLK answers an error code (not 0) for a layer past the last one, which ends the layer walk, and never for an existing layer. ([specs/christie-spyder.yaml:916](specs/christie-spyder.yaml#L916))
- [ ] Check that clone mode and crop anchor in RLK are written 0, 1, 2 (not 0.0), as the state maps them. ([specs/christie-spyder.yaml:938](specs/christie-spyder.yaml#L938))
- [ ] Check that an empty layer answers RLS with the code 1 alone, and what a layer with no source register answers. ([specs/christie-spyder.yaml:961](specs/christie-spyder.yaml#L961))
- [ ] Check that "more than once per second" for RCS is per input, so walking every layer's RCS once each 10-second poll does not degrade the system, and that a layer with no input answers 0 rather than an error. ([specs/christie-spyder.yaml:969](specs/christie-spyder.yaml#L969))
- [ ] Check that RBL and RRL ... -1 without start and count arguments return the whole list in one datagram, even for long lists. ([specs/christie-spyder.yaml:983](specs/christie-spyder.yaml#L983))
- [ ] Record an RPD answer: whether an empty background is sent as a word (which keeps the nine values per PixelSpace) or left out, which would misread the list. ([specs/christie-spyder.yaml:1007](specs/christie-spyder.yaml#L1007))
- [ ] Check that router IDs are small numbers from 0, as the poll of routers 0 to 3 assumes, and that QRC for a router that does not exist answers an error. ([specs/christie-spyder.yaml:893](specs/christie-spyder.yaml#L893))
- [ ] Check that the layer state an RLK walk reads after a recall or transition shows the new layout once the command's answer arrives, rather than only after the transition's duration. ([specs/christie-spyder.yaml:1034](specs/christie-spyder.yaml#L1034))

## christie-trulife — Christie TruLife and TruLife+ projectors

- [ ] The "#" full acknowledgment echoes a set as "(CODE value)" or "(#CODE value)" with the value as sent, so set commands complete on the echo. ([specs/christie-trulife.yaml:441](specs/christie-trulife.yaml#L441))
- [ ] A 30 s reply timeout covers the power on and off acknowledgments during the documented 20 s or longer warm-up; note which commands (lens calibration, CAV+MODE, BST, HFR+ENBL, RBT) still time out. ([specs/christie-trulife.yaml:62](specs/christie-trulife.yaml#L62))
- [ ] Error replies carry a nonzero ERR code and the asynchronous status message is ERR00000, so error_match fails only real errors. ([specs/christie-trulife.yaml:66](specs/christie-trulife.yaml#L66))
- [ ] With EME 1 the asynchronous FYI/ERR messages reach the TCP 3002 connection, and a status change names its item as "(SST+GROUP?nnn)", with bare or escaped parentheses. ([specs/christie-trulife.yaml:2409](specs/christie-trulife.yaml#L2409))
- [ ] How the projector sends parentheses inside quoted text (bare or \( \)), and that the framing keeps a status item's whole description and the start of event and message texts. ([specs/christie-trulife.yaml:2385](specs/christie-trulife.yaml#L2385))
- [ ] The general-form queries SIN? and SIN+PORT? are answered with "(SIN!nnn \"name\")" and "(SIN+PORT!nnn \"name\")". ([specs/christie-trulife.yaml:492](specs/christie-trulife.yaml#L492))
- [ ] The general-form queries FRZ?, ITP? and TMD+DATE? are answered on every family. ([specs/christie-trulife.yaml:689](specs/christie-trulife.yaml#L689))
- [ ] CHA? answers "(CHA!600 \"One-Port HDMI0\")" as the automation section shows, and CHA exists only on TruLife+. ([specs/christie-trulife.yaml:468](specs/christie-trulife.yaml#L468))
- [ ] In list replies "(CODE!L001 001 00600 \"name\")" the first field is enabled/available; find out what the second field means. ([specs/christie-trulife.yaml:1989](specs/christie-trulife.yaml#L1989))
- [ ] Reply format of NET+ETH0?, NET+SUB0?, NET+GATE? and NET+MAC0? (quoted or not). ([specs/christie-trulife.yaml:1528](specs/christie-trulife.yaml#L1528))
- [ ] Reply format of OTR? (text in quotes). ([specs/christie-trulife.yaml:919](specs/christie-trulife.yaml#L919))
- [ ] Reply format of UID?. ([specs/christie-trulife.yaml:1797](specs/christie-trulife.yaml#L1797))
- [ ] Reply format of LAS+CSRX?, LAS+CSRY? and LAS+CSRZ?. ([specs/christie-trulife.yaml:1066](specs/christie-trulife.yaml#L1066))
- [ ] LPP?m answers "(LPP!M<min> <max>)" like the lens ranges. ([specs/christie-trulife.yaml:1195](specs/christie-trulife.yaml#L1195))
- [ ] TMC+MODE? answers with a number, a text or both ("(TMC+MODE! \"Standard\")" is the only example). ([specs/christie-trulife.yaml:1136](specs/christie-trulife.yaml#L1136))
- [ ] Status replies keep the "(SST+GROUP!idx state \"value\" \"description\")" form on the 4K40-RGB, Mirage SST and Boxer families, and the item names match Projector Model, Projector S/N, Output Resolution, Projector Build Date, Main Control Board SW Version, Projector Hours, and Laser On/Standby or Lamp Hours. ([specs/christie-trulife.yaml:2399](specs/christie-trulife.yaml#L2399))
- [ ] Which status groups a Mirage SST returns to SST? (its status system reference was not available). ([specs/christie-trulife.yaml:2830](specs/christie-trulife.yaml#L2830))
- [ ] GIO+TRIG (documented as "GPIO+TRIG", unquoted) is accepted as GIO+TRIG "pattern". ([specs/christie-trulife.yaml:1753](specs/christie-trulife.yaml#L1753))
- [ ] GIO+CNFG takes O for an output (its examples show 0), and its reply reads GIO+CNFG! or GIO+CONFG!. ([specs/christie-trulife.yaml:1729](specs/christie-trulife.yaml#L1729))
- [ ] BLO+REDO and BLO+BLUO adjust the DMD their names say (their descriptions are swapped). ([specs/christie-trulife.yaml:892](specs/christie-trulife.yaml#L892))
- [ ] The range of BLO+MAXO (not documented; 0 to 100 assumed). ([specs/christie-trulife.yaml:900](specs/christie-trulife.yaml#L900))
- [ ] LCB+HOME with no value is accepted on every family. ([specs/christie-trulife.yaml:1218](specs/christie-trulife.yaml#L1218))
- [ ] Lens positions stay within ±65535, and whether FCS?, LHO?, LVO? or ZOM? report the current position. ([specs/christie-trulife.yaml:1243](specs/christie-trulife.yaml#L1243))
- [ ] The unit and range of TDD ("TDD 2000" is documented as 20.00 ms). ([specs/christie-trulife.yaml:1403](specs/christie-trulife.yaml#L1403))
- [ ] How many numbers a HIS entry holds (the examples show five where six fields are documented). ([specs/christie-trulife.yaml:2201](specs/christie-trulife.yaml#L2201))
- [ ] Whether CUC+H measurements take decimals (Mirage SST documents 0 to 99999.99 and 0 to 0.9999) or whole numbers (TruLife+ examples). ([specs/christie-trulife.yaml:999](specs/christie-trulife.yaml#L999))
- [ ] BST and BST+TEST sent without "#" answer with result lines, and a full suite finishes within 30 s. ([specs/christie-trulife.yaml:1865](specs/christie-trulife.yaml#L1865))
- [ ] EVT replies and where they are cut at parentheses. ([specs/christie-trulife.yaml:1838](specs/christie-trulife.yaml#L1838))
- [ ] Whether HFR+ENBL and RBT 111 echo before the projector restarts. ([specs/christie-trulife.yaml:1446](specs/christie-trulife.yaml#L1446))
- [ ] The units of ITP+FREQ per family (2300 to 50000 on TruLife+ and 4K40-RGB, 24 to 500 on Mirage SST and Boxer). ([specs/christie-trulife.yaml:711](specs/christie-trulife.yaml#L711))
- [ ] BGC numbering per family (0 is auto detect on TruLife+ and 4K40-RGB, sRGB on Mirage SST and Boxer). ([specs/christie-trulife.yaml:788](specs/christie-trulife.yaml#L788))
- [ ] LAS+MODE works on the Sapphire 4K40-RGBH (its "Only applies to" names M RGB and Griffyn, but the values list a Sapphire default). ([specs/christie-trulife.yaml:1036](specs/christie-trulife.yaml#L1036))
- [ ] The number of profiles per family (20 TruLife+, 10 Mirage SST and Boxer, 4 4K40-RGB). ([specs/christie-trulife.yaml:1452](specs/christie-trulife.yaml#L1452))
- [ ] The EDO rates each family accepts (48, 120, 240, 360 and 480 are family-specific). ([specs/christie-trulife.yaml:570](specs/christie-trulife.yaml#L570))
- [ ] FIB+SLTB on the 4K40-RGB and Boxer 4K (only SLTA is documented there). ([specs/christie-trulife.yaml:521](specs/christie-trulife.yaml#L521))
- [ ] Which SNMP traps each family accepts (LAMP and LIFE are not on Mirage SST; LIFE is not on the 4K40-RGB). ([specs/christie-trulife.yaml:1581](specs/christie-trulife.yaml#L1581))
- [ ] The UID login echo, and whether it repeats the password. ([specs/christie-trulife.yaml:1784](specs/christie-trulife.yaml#L1784))
- [ ] Whether LAS+EBLU/EGRN/ERED (and LAS+EPOW on Mirage SST) are safe on the 4K40-RGB and Mirage SST, where they are still documented (not offered). ([specs/christie-trulife.yaml:2732](specs/christie-trulife.yaml#L2732))
- [ ] The RS232 IN data bits, parity, stop bits and connector (not in these references). ([specs/christie-trulife.yaml:2739](specs/christie-trulife.yaml#L2739))
- [ ] Measure how long power on and off, lens calibration, the CAV+MODE reload, the BST self tests and the rebooting commands take to acknowledge, against the three minutes those commands wait. ([specs/christie-trulife.yaml:2659](specs/christie-trulife.yaml#L2659))

## chyron-prime — Chyron PRIME (Intelligent Interface)

- [ ] Record the default serial line settings of an Intelligent Interface COM connection (9600 8N1, no flow control assumed). ([specs/chyron-prime.yaml:772](specs/chyron-prime.yaml#L772))
- [ ] Confirm each reply ends with CR LF, and record the replies to the B commands (AN, AV, QS, QL, TH, SL, SA, SX, MX, CO, CS, SI, QE); the spec takes a reply to start with * or an eight-digit error code. ([specs/chyron-prime.yaml:62](specs/chyron-prime.yaml#L62))
- [ ] Record what PRIME answers to the default heartbeat (two backslashes). ([specs/chyron-prime.yaml:39](specs/chyron-prime.yaml#L39))
- [ ] Check that a buffer of * and a layer expression of * are accepted wherever the spec sends them (play_all with channel *, clear with layer *). ([specs/chyron-prime.yaml:192](specs/chyron-prime.yaml#L192))
- [ ] Record the External Update X command PRIME sends, so it can be read into state. ([specs/chyron-prime.yaml:63](specs/chyron-prime.yaml#L63))
- [ ] Check that the UTF-8 encoding setting of the connection carries non-ASCII text correctly. ([specs/chyron-prime.yaml:769](specs/chyron-prime.yaml#L769))
- [ ] Confirm SCENE_STATE answers *P\SCENE_STATE\<state>\\ for a scene name as SCENE_LIST gives it, without its project path. ([specs/chyron-prime.yaml:790](specs/chyron-prime.yaml#L790))

## clock-8001 — Clock-8001

- [ ] Confirm the default command and feedback port 1245 and that feedback is broadcast as one bundle on every change. ([specs/clock-8001.yaml:57](specs/clock-8001.yaml#L57))
- [ ] Record the source mode numbers and the icon names the state messages carry. ([specs/clock-8001.yaml:399](specs/clock-8001.yaml#L399))
- [ ] Confirm the cue commands take an empty UUID string from an external sender. ([specs/clock-8001.yaml:405](specs/clock-8001.yaml#L405))
- [ ] Check whether /clock/automation and /clock/cue/blank take OSC T/F booleans, as sent. ([specs/clock-8001.yaml:314](specs/clock-8001.yaml#L314))

## cockos-reaper — Cockos REAPER (OSC)

- [ ] Confirm REAPER's default Local listen port (8000) and Device port (9000) when a new OSC control surface is added. ([specs/cockos-reaper.yaml:59](specs/cockos-reaper.yaml#L59))
- [ ] Confirm that /device/track/count, /device/marker/count and /device/region/count sent on connecting take effect, and whether REAPER then sends the new bank's state at once. ([specs/cockos-reaper.yaml:94](specs/cockos-reaper.yaml#L94))
- [ ] Record the OSC types REAPER sends feedback in for b and t patterns (float 1.0 and 0.0 or integers); the state accepts both. ([specs/cockos-reaper.yaml:810](specs/cockos-reaper.yaml#L810))
- [ ] Confirm that REAPER takes binary values as OSC integers, as reaper-osc.js sends them, and floats where the pattern is n or f. ([specs/cockos-reaper.yaml:179](specs/cockos-reaper.yaml#L179))
- [ ] Confirm the inverted FX bypass (0 bypasses, 1 makes active) in both directions. ([specs/cockos-reaper.yaml:433](specs/cockos-reaper.yaml#L433))
- [ ] Record the track monitor values REAPER takes and sends (0 off, 1 on, 2 tape-auto, from reaper-osc.js) and the OSC type of the feedback, which the state reads as an integer. ([specs/cockos-reaper.yaml:865](specs/cockos-reaper.yaml#L865))
- [ ] Check whether /time, /tempo/raw and /playrate/raw sent to REAPER move the cursor and set the tempo and rate, and the tempo range REAPER accepts. ([specs/cockos-reaper.yaml:207](specs/cockos-reaper.yaml#L207))
- [ ] Check the lowest and highest dB /track/@/volume/db accepts (REAPER's fader-range preference assumed for the top). ([specs/cockos-reaper.yaml:252](specs/cockos-reaper.yaml#L252))
- [ ] Confirm that /action/str runs a named command ID (_SWS_..., a script's ID). ([specs/cockos-reaper.yaml:551](specs/cockos-reaper.yaml#L551))
- [ ] Confirm that /device/send/count, /device/receive/count, /device/fx/count, /device/fxparam/count and /device/fxinstparam/count sent on connecting take effect. ([specs/cockos-reaper.yaml:94](specs/cockos-reaper.yaml#L94))
- [ ] Confirm that REAPER sends /track/@/fx/@/fxparam/@/value for every track in the bank, numbering parameters from 1, and not only for the device's selected track. ([specs/cockos-reaper.yaml:930](specs/cockos-reaper.yaml#L930))
- [ ] Record what /track/@/recv/@/name carries (the source track's name assumed). ([specs/cockos-reaper.yaml:912](specs/cockos-reaper.yaml#L912))
- [ ] Confirm that an instrument's bypass (/track/@/fxinst/bypass) is inverted like an effect's, in both directions. ([specs/cockos-reaper.yaml:934](specs/cockos-reaper.yaml#L934))
- [ ] Check that /fx/@/fxparam/@/... address the device's selected track's effects and /fxparam/@/... its selected effect, numbered within the selected parameter bank. ([specs/cockos-reaper.yaml:1011](specs/cockos-reaper.yaml#L1011))
- [ ] Check that /device/fxparam/bank/select numbers banks from 1, as track banks are. ([specs/cockos-reaper.yaml:643](specs/cockos-reaper.yaml#L643))
- [ ] Check that REAPER feeds back /track/@/autotrim ... /autowrite as 1 for the mode in force and 0 for the others, and record what /track/auto sends for each mode. ([specs/cockos-reaper.yaml:870](specs/cockos-reaper.yaml#L870))
- [ ] Record the command ID of "Control surface: Refresh all surfaces" in REAPER's Actions list, and whether it is the same across REAPER versions. ([specs/cockos-reaper.yaml:1105](specs/cockos-reaper.yaml#L1105))

## cockos-reaper-web — Cockos REAPER (web interface)

- [ ] Confirm that the web interface's Username:password field makes REAPER answer 401 with HTTP Basic (realm reaper_www) and accept the user and password as Basic credentials, and that requests carrying a Basic header are accepted while the field is empty. ([spec](specs/cockos-reaper-web.yaml#L105))
- [ ] Confirm the default port REAPER proposes for a new Web browser interface (8080, the User Guide's example). ([spec](specs/cockos-reaper-web.yaml#L128))
- [ ] Confirm what actions 40667 and 40668 do (taken as stop saving all recorded media and stop deleting it, from REAPER's pages' stop and abort buttons). ([spec](specs/cockos-reaper-web.yaml#L176))
- [ ] Confirm the action IDs read from REAPER's pages for snap (1157), next and previous marker (40173, 40172), add marker (40157), and undo and redo (40029, 40030). ([spec](specs/cockos-reaper-web.yaml#L194))
- [ ] Check that a command and the read-back chained after it in one request (`;TRANSPORT`, `;TRACK`, `;GET/...`) answer with the value after the change, including for actions that REAPER may run deferred. ([spec](specs/cockos-reaper-web.yaml#L152))
- [ ] Check that SET/POS_STR decodes a percent-encoded position and takes m1, r1, M1 and R1 as main.js describes. ([spec](specs/cockos-reaper-web.yaml#L258))
- [ ] Check the order of the volume, pan and width suffixes this spec sends (e for releasing the touch, then g for ignoring grouping, as "0.500000eg"), and that REAPER reads both. ([spec](specs/cockos-reaper-web.yaml#L331))
- [ ] Check that a relative volume change with its suffixes ("+1.500000e", "-3.000000g") moves the volume by that many dB, as main.js describes. ([spec](specs/cockos-reaper-web.yaml#L364))
- [ ] Check that a relative pan or width change is on the same -1 to 1 scale as an absolute one (main.js says only that the adjustment is relative), and that the result stays within -1 to 1. ([spec](specs/cockos-reaper-web.yaml#L375))
- [ ] Record the RECMON value REAPER takes for monitoring off (0 assumed; main.js names only 1 on, 2 auto and < 0 to cycle). ([spec](specs/cockos-reaper-web.yaml#L458))
- [ ] Check that a SET/TRACK/n/<attribute>/<value> value, a P_NAME, extended state sections, keys and values, and undo messages sent percent-encoded (/ as %2F, ; as %3B) are decoded by REAPER before use. ([spec](specs/cockos-reaper-web.yaml#L501))
- [ ] Check the send index of GET/TRACK/n/SEND/i and SET/TRACK/n/SEND/i/...: hardware outputs first from 0, then sends (inferred from more_me.html), and receives from -1. ([spec](specs/cockos-reaper-web.yaml#L521))
- [ ] Check the send volume and pan suffixes e (end of a drag) and E (instant edit), and what a value with neither does. ([spec](specs/cockos-reaper-web.yaml#L549))
- [ ] Check that OSC/<address> takes a percent-encoded address (%2F for /), that :<float> and :s<text> carry the argument, and that the message reaches Default.ReaperOSC with no OSC control surface configured. ([spec](specs/cockos-reaper-web.yaml#L686))
- [ ] Check that REAPER ends reply lines with LF alone (the rules do not expect CR), and that a request of several commands answers each command's lines in order in one body. ([spec](specs/cockos-reaper-web.yaml#L775))
- [ ] Record a TRANSPORT reply: the repeat field as 0 and 1 (main.js says only nonzero when on), and the position formats. ([spec](specs/cockos-reaper-web.yaml#L786))
- [ ] Record a BEATPOS reply: whether the measure count is a whole number and the beats-in-measure a decimal, as read. ([spec](specs/cockos-reaper-web.yaml#L799))
- [ ] Record a TRACK reply: meter peak and position as integers, the width and pan mode fields, the color as 0xaarrggbb text or 0, and whether a track name holding a tab is escaped. ([spec](specs/cockos-reaper-web.yaml#L809))
- [ ] Record MARKER and REGION replies with and without the color field, and check that markers and regions are listed in timeline order (state keys them by place from 1). ([spec](specs/cockos-reaper-web.yaml#L835))
- [ ] Record a SEND reply's flags, volume and pan formats, and its first two fields for a receive (the track and the negative index asked for). ([spec](specs/cockos-reaper-web.yaml#L865))
- [ ] Record a LYRICS reply: beat and lyric pairs after the track number. ([spec](specs/cockos-reaper-web.yaml#L871))
- [ ] Check that the EXTSTATE and PROJEXTSTATE replies give the section and key decoded, as sent before encoding. ([spec](specs/cockos-reaper-web.yaml#L878))

## cuetimer — CueTimer

- [ ] Confirm CueTimer sends its state document on connecting and every 200 ms, each ended by $ with nothing between documents. ([specs/cuetimer.yaml:49](specs/cuetimer.yaml#L49))
- [ ] Record what Pause does (pause, or toggle pause and resume) and whether Undo$ (sent by the Companion module) exists. ([specs/cuetimer.yaml:347](specs/cuetimer.yaml#L347))
- [ ] Check whether CueTimer answers a command or a malformed command at all. ([specs/cuetimer.yaml:51](specs/cuetimer.yaml#L51))
- [ ] Confirm h, m and s are the running timer's remaining time and how overtime is shown in them. ([specs/cuetimer.yaml:265](specs/cuetimer.yaml#L265))

## cuez-automator — Cuez Automator

- [ ] Record the bodies the triggering and timer endpoints answer (documented only as 200 OK) and whether an unknown id answers an error status. ([specs/cuez-automator.yaml:431](specs/cuez-automator.yaml#L431))
- [ ] Record the device-state socket (/ws/device-state) and the Automator's WebSocket messages, so state can be pushed instead of polled. ([specs/cuez-automator.yaml:437](specs/cuez-automator.yaml#L437))
- [ ] Confirm a step index counts the trigger itself as 0, and what /api/trigger/step/<n> answers past the last step. ([specs/cuez-automator.yaml:104](specs/cuez-automator.yaml#L104))
- [ ] Record the shapes of /api/timer, /api/macro/ and /api/trigger/current so timers, macros and the current trigger can be kept in state. ([specs/cuez-automator.yaml:241](specs/cuez-automator.yaml#L241))

## cyberpower-ups — CyberPower UPS (SNMP)

- [ ] Walk a UPS through an RMCARD205 and an RMCARD305 and check which groups each fills, and that every table is keyed by single integer indexes. ([specs/cyberpower-ups.yaml:33](specs/cyberpower-ups.yaml#L33))
- [ ] Check that the UPS control objects read back their idle value after a SET, and when a turn-off or sleep shows in the output status. ([specs/cyberpower-ups.yaml:697](specs/cyberpower-ups.yaml#L697))
- [ ] Check the units taken from the MIB's descriptions against the UPS's display. ([specs/cyberpower-ups.yaml:692](specs/cyberpower-ups.yaml#L692))
- [ ] Receive a v1 trap (on battery, then back on line) and check it is named and that the poll after it shows the change. ([specs/cyberpower-ups.yaml:703](specs/cyberpower-ups.yaml#L703))

## dahua-ptz — Dahua and Amcrest PTZ cameras (HTTP CGI, public subset)

- [ ] Check whether ptz.cgi numbers the first channel 1 (as the public integration sends) or 0. ([specs/dahua-ptz.yaml:43](specs/dahua-ptz.yaml#L43))
- [ ] Check which of arg1 and arg2 carry the vertical and horizontal speeds for each move code, and the speed range. ([specs/dahua-ptz.yaml:61](specs/dahua-ptz.yaml#L61))
- [ ] Check the stop, zoom, focus and GotoPreset argument forms, and what the camera answers on success and on an unknown preset. ([specs/dahua-ptz.yaml:112](specs/dahua-ptz.yaml#L112))

## dandugan-model-e — Dan Dugan Model E

- [ ] Confirm the Model E takes the commands on TCP 23 (the guide tests with Telnet but names no port) and whether it sends a Telnet greeting. ([specs/dandugan-model-e.yaml:36](specs/dandugan-model-e.yaml#L36))
- [ ] Record the reply forms (no spaces, value after a comma) for a name holding spaces and for every query. ([specs/dandugan-model-e.yaml:86](specs/dandugan-model-e.yaml#L86))

## dashare-multiplay — MultiPlay 3 (OSC control)

- [ ] Confirm MultiPlay takes OSC over UDP on its Control port, and which port a fresh install uses. ([specs/dashare-multiplay.yaml:59](specs/dashare-multiplay.yaml#L59))
- [ ] Confirm the cue actions and /Stopwatch are matched without case: the help writes Go and /Stopwatch/Start, the Companion module sends go and /stopwatch/start. ([specs/dashare-multiplay.yaml:109](specs/dashare-multiplay.yaml#L109))
- [ ] Record the OSC type of the status booleans (/status/go and the others): OSC true/false, the strings TRUE/FALSE, or ints. ([specs/dashare-multiplay.yaml:375](specs/dashare-multiplay.yaml#L375))
- [ ] Confirm /status/refresh sends every status message, and that MultiPlay sends status only to the Remote target. ([specs/dashare-multiplay.yaml:369](specs/dashare-multiplay.yaml#L369))
- [ ] Confirm the int and float argument types of Pan, Speed, Volume and their /- and /+ forms, and that Position and the jumps take a float. ([specs/dashare-multiplay.yaml:221](specs/dashare-multiplay.yaml#L221))
- [ ] Confirm the lowest volume level bounds Volume and Volume/- (-60 dB unless changed). ([specs/dashare-multiplay.yaml:292](specs/dashare-multiplay.yaml#L292))

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

- [ ] Capture /api-docs/openapi.json from a node and check success statuses (200 assumed), error bodies, and the response shapes of /info, /v0/state, /v0/timelines, /v0/cues and /v0/inputs. ([specs/dataton-watchout7.yaml:467](specs/dataton-watchout7.yaml#L467))
- [ ] Confirm the /v2/sse event shape {"kind", "value"} without event names (from Bitfocus's Companion module), and the playbackState, timelineCountdowns and showRevision value shapes. ([specs/dataton-watchout7.yaml:475](specs/dataton-watchout7.yaml#L475))
- [ ] Check whether /v0/play needs a JSON body (current guide) or plays every timeline with none (older page). ([specs/dataton-watchout7.yaml:174](specs/dataton-watchout7.yaml#L174))
- [ ] Check the node restart path on port 3017: /v0/services/restart (Dataton) or /v0/restart (Companion module). ([specs/dataton-watchout7.yaml:332](specs/dataton-watchout7.yaml#L332))
- [ ] Record what the Process Manager on port 3017 answers to /v0/shutdown, /v0/reboot and /v0/services/restart (HTTP 200 assumed, no body documented), and whether it answers before the node goes down. ([specs/dataton-watchout7.yaml:338](specs/dataton-watchout7.yaml#L338))
- [ ] Confirm the Process Manager on port 3017 takes no authentication and is served on every node, including a node running the Director, and from which WATCHOUT 7 release (assumed for the pre-7.8 model too). ([specs/dataton-watchout7.yaml:70](specs/dataton-watchout7.yaml#L70))
- [ ] Check what restart_node_services restarts (the Operative, the Director, or every WATCHOUT service) and how long playback stops. ([specs/dataton-watchout7.yaml:498](specs/dataton-watchout7.yaml#L498))
- [ ] Check the type of showRevision's value (a string is assumed). ([specs/dataton-watchout7.yaml:444](specs/dataton-watchout7.yaml#L444))
- [ ] Check how names with spaces or non-ASCII characters are encoded in /v0/cue-group-state/by-name/{group}/{variant}. ([specs/dataton-watchout7.yaml:257](specs/dataton-watchout7.yaml#L257))
- [ ] Record the shape of GET /v0/state ({clockTime, timelines: [{id, playbackStatus}]} bare, or under value as in the event stream, are both read) and confirm it lists every timeline of the show, stopped ones included: each read replaces the timelines state. ([specs/dataton-watchout7.yaml:365](specs/dataton-watchout7.yaml#L365))
- [ ] Confirm showRevision is sent when a timeline is added to or removed from the show (it triggers the re-read of /v0/state), and whether /v2/sse sends the whole playback state when it opens. ([specs/dataton-watchout7.yaml:408](specs/dataton-watchout7.yaml#L408))
- [ ] Confirm POST /v0/showfile takes the .watch file as the raw request body with Content-Type application/octet-stream, answers 200, and that showName names the loaded show; and record whether WATCHOUT 7 before 7.8 has it. ([specs/dataton-watchout7.yaml:318](specs/dataton-watchout7.yaml#L318))
- [ ] Record /v0/show's shape (show.timelines keyed by id with name and cues keyed by id with name and start, showName, show.hosts) against the node's OpenAPI document, and whether a show revision event carries the revision bare or as value.revision. ([specs/dataton-watchout7.yaml:481](specs/dataton-watchout7.yaml#L481))

## datavideo-switcher — Datavideo switchers (DVIP and RS-232)

- [ ] Confirm port 5009's answer (a real-time port, 0xaa55ffff when none is free) and that the pair it names is held while connected; check which pair the RMC-260 panel takes on the SE-1200MU. ([specs/datavideo-switcher.yaml:859](specs/datavideo-switcher.yaml#L859))
- [ ] Confirm the real-time port's first packet: whether it lists every control or only the non-zero ones (state is rebuilt against zeros), that later packets carry only changes, and that answering each with a null packet keeps it flowing. ([specs/datavideo-switcher.yaml:867](specs/datavideo-switcher.yaml#L867))
- [ ] Confirm a GET_CONTROL reply on the command port: pairs in the order asked, and the two trailing zero words of 5.3's example; and that SET_CONTROL is answered OK even for a read-only control. ([specs/datavideo-switcher.yaml:867](specs/datavideo-switcher.yaml#L867))
- [ ] Record the source codes each bus takes on each model (program, keyers, aux, multiviewer), the SE-4000's inputs (8) and the SE-1200MU's (the document's tables list 1-4), against the documents' conflicting tables. ([specs/datavideo-switcher.yaml:877](specs/datavideo-switcher.yaml#L877))
- [ ] Confirm the SE-3200/SE-4000 bus tally numbering 30-43 and free space 44 (the documents repeat 23-30), and the Ethernet input numbering (freeze mode 6) over the RS-232 supplement's (5). ([specs/datavideo-switcher.yaml:884](specs/datavideo-switcher.yaml#L884))
- [ ] Record the types and values of the audio section, GPI, output mode and system preference controls, which the documents leave undescribed, and how memory.flags reads. ([specs/datavideo-switcher.yaml:890](specs/datavideo-switcher.yaml#L890))
- [ ] Confirm names are UTF-16LE, GET_INPUT_NAME's input numbering (from 0 assumed), and the file transfer: GET_FILE_DATA chunk size and end, STORE_FILE_DATA's chunk numbering and size word, FILE_EVENT; and the mini picture's layout. ([specs/datavideo-switcher.yaml:900](specs/datavideo-switcher.yaml#L900))
- [ ] Confirm the RS-232 70/F0 control IDs and values are little-endian, the SE-1200MU's speed (19200 in 2019, 57600 in 2017) and the SE-4000's status byte bits. ([specs/datavideo-switcher.yaml:908](specs/datavideo-switcher.yaml#L908))
- [ ] Confirm the HS-1300 takes the SE-1200MU's Ethernet protocol and the HS-4000 the SE-4000's, and their control tables. ([specs/datavideo-switcher.yaml:920](specs/datavideo-switcher.yaml#L920))
- [ ] Confirm the SE-2200's DVIP board takes the serial blocks over TCP 5002 inside DVIP's length header and answers the same way; the T-bar's range; status byte 4 bit 4 as PinP 2 preview; and the 'ignored' status outside PC CTRL mode. ([specs/datavideo-switcher.yaml:925](specs/datavideo-switcher.yaml#L925))
- [ ] Confirm DVIP UDP packets' length counts the whole packet (or leaves out the type byte), replies come back to the sender's port, vendor and product IDs are big-endian, and which switchers answer the UDP management commands and presets. ([specs/datavideo-switcher.yaml:934](specs/datavideo-switcher.yaml#L934))
- [ ] Check whether the SE-650 (and the HS-1300 over DVIP TCP) takes switching commands over DVIP and where Datavideo documents them. ([specs/datavideo-switcher.yaml:941](specs/datavideo-switcher.yaml#L941))

## dbaudio — d&b audiotechnik amplifiers

- [ ] On each model (especially the D25, 25D, D90 and 5DM, which d&b's Q-SYS plug-in added last), confirm the named objects answer at the numbers built from the plug-in's tables, and record the class GetClassIdentification reports for each. ([specs/dbaudio.yaml:638](specs/dbaudio.yaml#L638))
- [ ] Confirm the OCP.1 port on each family (30013 on the D80, D20, 30D and 10D, 50014 on the others) and on older firmware, and whether any amplifier also speaks OCP.1 over UDP or a WebSocket. ([specs/dbaudio.yaml:647](specs/dbaudio.yaml#L647))
- [ ] Read device.model_guid on each model and check the manufacturer code and model byte against the list in the quirk. ([specs/dbaudio.yaml:655](specs/dbaudio.yaml#L655))
- [ ] Check that subscriptions to the named objects are accepted and changes made in R1 or on the front panel arrive by notification, and whether a walk of the root block finds the same objects, and under which roles. ([specs/dbaudio.yaml:662](specs/dbaudio.yaml#L662))
- [ ] Load an AmpPreset with load_amp_preset and backup_and_load_amp_preset and confirm the backup lands in 13 to 15; check what AmpPreset/Selected, AmpPreset/Modified and AmpPreset <n>/Stored report, and that the preset names can be written. ([specs/dbaudio.yaml:670](specs/dbaudio.yaml#L670))
- [ ] Confirm identify (method 5.2 with 0x0200, then 0) makes the amplifier wink and stop, and that the Wink object's Active (4.1) follows it. ([specs/dbaudio.yaml:678](specs/dbaudio.yaml#L678))
- [ ] In each 2-way active and mix top/sub output mode, check which channel's objects the pair answers to and what writing the second channel's does. ([specs/dbaudio.yaml:684](specs/dbaudio.yaml#L684))
- [ ] Check the delay's reported minimum and maximum, and what Filter1, Filter2 and Filter3 do and accept with a few loudspeaker setups. ([specs/dbaudio.yaml:690](specs/dbaudio.yaml#L690))
- [ ] On a D80 and D20, confirm the input mode positions 1 to 5 and which routing switches answer in each. ([specs/dbaudio.yaml:696](specs/dbaudio.yaml#L696))
- [ ] Confirm switching load or input monitoring off and on again clears its errors, and what DeviceErrorText and ErrorText hold. ([specs/dbaudio.yaml:703](specs/dbaudio.yaml#L703))
- [ ] Check the Headroom meter's unit and range, and how often the ISP, GR and OVL sensors notify under programme. ([specs/dbaudio.yaml:710](specs/dbaudio.yaml#L710))
- [ ] On a DS10 and DS100, find the OCP.1 port and walk the tree: record its roles and classes so they can be named here. ([specs/dbaudio.yaml:716](specs/dbaudio.yaml#L716))
- [ ] Confirm set_device_time (SetDeviceTimeNTP, 3.2, on object 9 or 10 by family) sets the amplifier's clock. ([specs/dbaudio.yaml:579](specs/dbaudio.yaml#L579))

## denon-dn500av — Denon Professional DN-500AV

- [ ] Confirm IP control on TCP 23 needs no login, and how many connections the unit takes. ([specs/denon-dn500av.yaml:26](specs/denon-dn500av.yaml#L26))
- [ ] Confirm PWON works only over IP control, as the guide marks it. ([specs/denon-dn500av.yaml:37](specs/denon-dn500av.yaml#L37))
- [ ] Confirm the video processing mode is asked with VSVPM ? (the guide prints VSVPN ?). ([specs/denon-dn500av.yaml:328](specs/denon-dn500av.yaml#L328))
- [ ] Confirm the HDMI setup and GUI requests are SSHOS ? and SSOSD ? (printed HOS ? and OSD ?), and that SSHOSALS ON disables auto lip sync and SSOSDSCR ON turns the screen saver off. ([specs/denon-dn500av.yaml:853](specs/denon-dn500av.yaml#L853))
- [ ] Confirm Cinema EQ is set with PSHTEQ and asked with "PSCINEMA EQ. ?". ([specs/denon-dn500av.yaml:357](specs/denon-dn500av.yaml#L357))
- [ ] Check whether the unit sends half-dB volume steps (three digits, MV805) or MVMAX lines, which the spec does not read. ([specs/denon-dn500av.yaml:967](specs/denon-dn500av.yaml#L967))
- [ ] Confirm the on-screen list's line form (NSE0 to NSE8, a flag byte on lines 1-7, text up to a Null) and whether it is sent unasked. ([specs/denon-dn500av.yaml:1064](specs/denon-dn500av.yaml#L1064))
- [ ] Confirm the effect level's range (00-99 is the guide's form). ([specs/denon-dn500av.yaml:492](specs/denon-dn500av.yaml#L492))
- [ ] Confirm the LFE level's form: 00 for 0 dB to 10 for -10 dB. ([specs/denon-dn500av.yaml:473](specs/denon-dn500av.yaml#L473))

## denon-dn500bd — Denon Professional DN-500BD / DN-500BD MKII

- [ ] Confirm IP control on TCP 9030 for the DN-500BD MKII (its guide documents RS-232C only; the port is the DN-500BD guide's and the Companion module's). ([specs/denon-dn500bd.yaml:48](specs/denon-dn500bd.yaml#L48))
- [ ] Confirm ACK and NACK are the bare bytes 06H and 15H with no CR, on TCP as on RS-232C. ([specs/denon-dn500bd.yaml:56](specs/denon-dn500bd.yaml#L56))
- [ ] Confirm the host's ACK is the byte 06H alone (no CR after it), that each status the player notifies by itself is answered within 30 ms by it and not sent again, and which statuses it sends by itself. ([specs/denon-dn500bd.yaml:62](specs/denon-dn500bd.yaml#L62))
- [ ] Confirm the RS-232C speed's factory setting (115200 assumed). ([specs/denon-dn500bd.yaml:71](specs/denon-dn500bd.yaml#L71))
- [ ] Confirm the total group and group answers carry the "?" (@0?PCTG0003) as the guide prints them. ([specs/denon-dn500bd.yaml:779](specs/denon-dn500bd.yaml#L779))
- [ ] Confirm ?DVANGL is answered @0DVANGL and the angle's number. ([specs/denon-dn500bd.yaml:340](specs/denon-dn500bd.yaml#L340))
- [ ] Confirm the speaker setting's level form (a sign and two digits; the guide writes vvv and vvvv) and the delay's unit and range. ([specs/denon-dn500bd.yaml:560](specs/denon-dn500bd.yaml#L560))
- [ ] Confirm the output volume answers: a sign and two digits, 000, inf and FX. ([specs/denon-dn500bd.yaml:615](specs/denon-dn500bd.yaml#L615))
- [ ] Confirm the speaker configurations 4.1, 5.1 and 6.1 (the guide gives LR, ST, 2.1, 3.1 and 7.1 as examples) and whether ?DVSCCh reads it back. ([specs/denon-dn500bd.yaml:552](specs/denon-dn500bd.yaml#L552))
- [ ] Confirm the DN-500BD MKII's rewind (PCSLSR) and that its guide's fast forward code (PCGPPV, the previous title's) is a misprint. ([specs/denon-dn500bd.yaml:403](specs/denon-dn500bd.yaml#L403))
- [ ] Check whether the DN-500BD MKII answers the menu settings' requests (?PCAP and the others) as the DN-500BD does; its guide lists none, so the spec does not ask it. ([specs/denon-dn500bd.yaml:857](specs/denon-dn500bd.yaml#L857))
- [ ] Confirm DHCP is set with @0IpAUTO and eight zeros, and the IP settings' twelve-digit form. ([specs/denon-dn500bd.yaml:666](specs/denon-dn500bd.yaml#L666))
- [ ] Confirm a status request's own answer wants no ACK (the guide's status request flow shows none), that ?PW is answered ACK alone, and that BUSY (@0BDERBUSY) wants none. ([specs/denon-dn500bd.yaml:62](specs/denon-dn500bd.yaml#L62))

## denon-dn500cb — Denon Professional DN-500CB

- [ ] Confirm ACK and NACK are the bare bytes 06H and 15H with no CR, and statuses end in CR. ([specs/denon-dn500cb.yaml:33](specs/denon-dn500cb.yaml#L33))
- [ ] Confirm the host's ACK is the byte 06H alone (no CR after it), that each status the player notifies by itself is answered by it and not sent again, and that a status request's own answer, ?PW's ACK alone and BUSY want none. ([specs/denon-dn500cb.yaml:39](specs/denon-dn500cb.yaml#L39))
- [ ] Confirm the RS-232C speed's factory setting (115200 assumed). ([specs/denon-dn500cb.yaml:50](specs/denon-dn500cb.yaml#L50))
- [ ] Confirm track selection is @0Tr and four digits with no space, and the artist and album answers' form (the guide shows a space after at and al). ([specs/denon-dn500cb.yaml:129](specs/denon-dn500cb.yaml#L129))
- [ ] Confirm the track time answer is MMMSS (five digits) on this model. ([specs/denon-dn500cb.yaml:268](specs/denon-dn500cb.yaml#L268))

## denon-dn500r — Denon Professional DN-500R

- [ ] Confirm ACK and NACK are the bare bytes 06H and 15H with no CR, and statuses end in CR. ([specs/denon-dn500r.yaml:44](specs/denon-dn500r.yaml#L44))
- [ ] Confirm the host's ACK is the byte 06H alone (no CR after it), that each status the recorder notifies by itself is answered by it and not sent again, and that a status request's own answer and BUSY want none. ([specs/denon-dn500r.yaml:50](specs/denon-dn500r.yaml#L50))
- [ ] Confirm the RS-232C speed's factory setting (9600 assumed). ([specs/denon-dn500r.yaml:61](specs/denon-dn500r.yaml#L61))
- [ ] Check random's and program list playback's codes: set 00 for on, but answered 01 for on as the guide prints it. ([specs/denon-dn500r.yaml:1321](specs/denon-dn500r.yaml#L1321))
- [ ] Confirm rename_file (@0RN, a four-digit file number and a name) is not taken for random (@0RN00 or @0RN01). ([specs/denon-dn500r.yaml:791](specs/denon-dn500r.yaml#L791))
- [ ] Confirm ?FE answers each media's free space and its unit; the media size request (?SF in the guide) is the folder name's. ([specs/denon-dn500r.yaml:885](specs/denon-dn500r.yaml#L885))
- [ ] Confirm the track change the recorder sends by itself: "@Tr NNNN" as printed, or @0Tr. ([specs/denon-dn500r.yaml:1924](specs/denon-dn500r.yaml#L1924))
- [ ] Confirm the three-digit hot start assignment @0HS, and the four-digit one's code (printed @hs without the ID). ([specs/denon-dn500r.yaml:705](specs/denon-dn500r.yaml#L705))
- [ ] Confirm the codes printed with a space (@0PO XX, @0IN XX and others) take no space. ([specs/denon-dn500r.yaml:1090](specs/denon-dn500r.yaml#L1090))
- [ ] Confirm the fade in answer's code: FL as printed, or FI. ([specs/denon-dn500r.yaml:1363](specs/denon-dn500r.yaml#L1363))
- [ ] Confirm the recording sample rate is asked with ?FS (the guide prints @0FSNN as the request). ([specs/denon-dn500r.yaml:1228](specs/denon-dn500r.yaml#L1228))
- [ ] Confirm the recording volume answer's form: two characters per channel, signed or not. ([specs/denon-dn500r.yaml:918](specs/denon-dn500r.yaml#L918))
- [ ] Confirm the hot start information's time is HHHmmssttt (ten digits) after the colon. ([specs/denon-dn500r.yaml:1966](specs/denon-dn500r.yaml#L1966))
- [ ] Confirm the timer answers carry no timer number (the spec takes it from the request) and their field order. ([specs/denon-dn500r.yaml:1895](specs/denon-dn500r.yaml#L1895))

## dhd-aes67 — DHD audio AES67 / RAVENNA interfaces (Control API)

- [ ] Check the module path /modules/1/api/ws on an AES67 interface itself, and the module ID Toolbox gives an XC3 52-7010 AES67 module. ([specs/dhd-aes67.yaml:40](specs/dhd-aes67.yaml#L40))
- [ ] Check that the PTP, stream and route values whose names do not begin with _ can be set: DHD's AES67 OpenAPI document gives none of them a set parameter. ([specs/dhd-aes67.yaml:138](specs/dhd-aes67.yaml#L138))
- [ ] Check that the current value sent after a subscribe arrives as an update rooted at the tree's root, as on cores. ([specs/dhd-aes67.yaml:770](specs/dhd-aes67.yaml#L770))
- [ ] Check the auth answer and error codes 1280 and 1281 on an AES67 module, as on cores. ([specs/dhd-aes67.yaml:52](specs/dhd-aes67.yaml#L52))

## dhd-control-api — DHD audio XC3 / XD3 / XS3 / RM1 cores (Control API)

- [ ] Check that the current value sent after a subscribe arrives as an update message rooted at the tree's root (`{"method":"update","payload":{"audio":...}}`), as later changes do, and not only inside the subscribe answer. ([specs/dhd-control-api.yaml:3808](specs/dhd-control-api.yaml#L3808))
- [ ] Check that every answer, including RPC answers and errors, returns the msgID the request carried as a string, which is how the core ties each command to its answer. ([specs/dhd-control-api.yaml:319](specs/dhd-control-api.yaml#L319))
- [ ] Check the auth answer: success given as the string "true" (as DHD's example shows) or a boolean, error code 1281 for a wrong token and 1280 for none; and that the subscriptions sent once auth is answered are accepted. ([specs/dhd-control-api.yaml:70](specs/dhd-control-api.yaml#L70))
- [ ] Check that an access group's access, brightnessconsole, brightnessdisplay and colorscheme can be set: their names do not begin with _, but DHD's OpenAPI document gives them no set parameter. ([specs/dhd-control-api.yaml:2736](specs/dhd-control-api.yaml#L2736))
- [ ] Check the path of the "channel on key sets fader" option: DHD's document gives /control/mixers/<mixer>/faderoptionschonkeysetsfader, which looks like faderoptions/chonkeysetsfader with a slash missing. ([specs/dhd-control-api.yaml:2872](specs/dhd-control-api.yaml#L2872))
- [ ] Check that fadetovalue takes its fade time as time (as DHD's example request and answer do) rather than fadetime (as its parameter list says). ([specs/dhd-control-api.yaml:485](specs/dhd-control-api.yaml#L485))
- [ ] Check that a reverb's decay/min is read only like its _max and _step (it is named without the underscore) and that eq q is a whole number in flex processings and a decimal on faders, as the document's types say. ([specs/dhd-control-api.yaml:2963](specs/dhd-control-api.yaml#L2963))
- [ ] Check that a level detect's _name and _mode are text, as their descriptions say, though DHD's document types them as numbers. ([specs/dhd-control-api.yaml:2971](specs/dhd-control-api.yaml#L2971))
- [ ] Check that decimals sent with three places (fader -6.500) are accepted, and the nearest-value clamping answer of a set out of range. ([specs/dhd-control-api.yaml:3818](specs/dhd-control-api.yaml#L3818))

## dhd-ecp — DHD audio ECP (RM4200D, Series 52 cores)

- [ ] Check that an answer to a request addressed by fader channel (FaderHi 0x10) repeats 0x10 and the channel, for get_fader_level and get_pfl, rather than naming the logic fader the channel sits on; the core matches the answer by those bytes. ([specs/dhd-ecp.yaml:567](specs/dhd-ecp.yaml#L567))
- [ ] Check that a set followed at once by its request is answered with the new value (set_route, set_logic, set_fader_level, set_label, set_io_gain), not the value before the set was applied. ([specs/dhd-ecp.yaml:521](specs/dhd-ecp.yaml#L521))
- [ ] Check whether the device sends 0x1102 on/off blocks unasked when a fader or channel is switched on or off on the console; the protocol documents no request for it, and the on state is read only from such blocks. ([specs/dhd-ecp.yaml:488](specs/dhd-ecp.yaml#L488))
- [ ] Check whether the device answers a fader level request for a logic fader whose virtual mixer or fader is not configured, or stays silent (the core times out after 2 s). ([specs/dhd-ecp.yaml:521](specs/dhd-ecp.yaml#L521))
- [ ] Check the firmware 05 access status answer (0x1104, length 7): which access group D6 numbers (0-5 assumed, reported 1-6), and whether one answer comes per group. ([specs/dhd-ecp.yaml:508](specs/dhd-ecp.yaml#L508))
- [ ] Check that the meter correlation byte is signed (-100 to 100 as a two's complement byte) as read. ([specs/dhd-ecp.yaml:515](specs/dhd-ecp.yaml#L515))
- [ ] Check that Load Save Setups (0x111C) loads and saves snapshots on firmware 06 and later, which DHD documents only under firmware 05 but lists for every firmware line. ([specs/dhd-ecp.yaml:538](specs/dhd-ecp.yaml#L538))
- [ ] Check that the channel label request (0x1114, length 3) is answered with length 8 for the display asked, and how a central operating panel module (0x8001-0x800C) answers it. ([specs/dhd-ecp.yaml:363](specs/dhd-ecp.yaml#L363))

## digico-sd — DiGiCo SD and Quantum consoles

- [ ] Confirm that current SD and Quantum software still accepts the 2014 Other OSC list's /sd/ addresses, on an SD-series console and on a Quantum (225, 338, 5, 7 or 852). ([specs/digico-sd.yaml:18](specs/digico-sd.yaml#L18))
- [ ] Check whether the console sends anything back to an Other OSC device (changes made on the surface, or echoes of received messages), and on which port. ([specs/digico-sd.yaml:4037](specs/digico-sd.yaml#L4037))
- [ ] Record the fader position for 0 dB and a few other levels, and what the 0.0-1.0 range covers for trim, analogue gain, EQ frequency and gain, and delay. ([specs/digico-sd.yaml:4045](specs/digico-sd.yaml#L4045))
- [ ] Check what a message for a strip the session does not have does (ignored, or something else), and the real strip limits per console. ([specs/digico-sd.yaml:4054](specs/digico-sd.yaml#L4054))
- [ ] Confirm press_macro sends macro 1 as 0 (the list's 0-255 range and the Companion module), not 1. ([specs/digico-sd.yaml:4062](specs/digico-sd.yaml#L4062))
- [ ] Check that fire_snapshot takes the snapshot number shown on the console (and what happens for a number with no snapshot, or a decimal-numbered snapshot). ([specs/digico-sd.yaml:3764](specs/digico-sd.yaml#L3764))
- [ ] Check the meaning of the Int ranges taken as given: input phase 0-3, EQ curve 1-4, compressor knee 0-2, dynamic EQ over/under 0-1. ([specs/digico-sd.yaml:837](specs/digico-sd.yaml#L837))
- [ ] Check the longest channel name the console accepts, and what it does with a longer one. ([specs/digico-sd.yaml:921](specs/digico-sd.yaml#L921))
- [ ] Confirm /sd/Input_Channels/N/Channel_Input/main/alt_in is the address (the table's row is garbled) and that 1 selects the alternate input and 0 the main input. ([specs/digico-sd.yaml:3792](specs/digico-sd.yaml#L3792))
- [ ] Check what CGs_level and CGs_mute do on input channels and aux, group and matrix outputs (taken as the level and mute a strip takes from its control groups), and what a CGs_level position means in dB. ([specs/digico-sd.yaml:3804](specs/digico-sd.yaml#L3804))
- [ ] Confirm Matrix_Inputs/N/Matrix_Send/M sends matrix input N to matrix output M, both numbered from 1, and how many matrix inputs each console has. ([specs/digico-sd.yaml:3900](specs/digico-sd.yaml#L3900))
- [ ] Find what a multi is on current SD and Quantum software, how many exist, and whether Multis/N solo, fader, mute and name act on them as the list's rows read. ([specs/digico-sd.yaml:3930](specs/digico-sd.yaml#L3930))

## digitalprojection-dvision — Digital Projection dVision 35 and dVision Scope (ASCII protocol)

- [ ] Confirm commands without a value in the document (MENU, NVUP, PRST, RD65, FCRE, SINF) are sent as the mnemonic alone and acknowledged. ([specs/digitalprojection-dvision.yaml:978](specs/digitalprojection-dvision.yaml#L978))
- [ ] Confirm the meaning of the table rows the document prints one row off: PMUT 0/1, SABS's value table, CMTV 3200-9300, RD65 set-only, DSCR's table. ([specs/digitalprojection-dvision.yaml:130](specs/digitalprojection-dvision.yaml#L130))
- [ ] Record each setting's range with ?N and ?M (brightness, contrast, lamp power and the others the document gives no range for). ([specs/digitalprojection-dvision.yaml:1099](specs/digitalprojection-dvision.yaml#L1099))
- [ ] Confirm a negative value is acknowledged as - and five digits, and how a string value (SERI) is answered. ([specs/digitalprojection-dvision.yaml:1438](specs/digitalprojection-dvision.yaml#L1438))
- [ ] Confirm the 5 s pause after 20 commands is needed on the LAN as well as on RS-232, and whether TCP 1025 takes several commands on one connection. ([specs/digitalprojection-dvision.yaml:39](specs/digitalprojection-dvision.yaml#L39))
- [ ] Confirm TATB, EDIR and EDIT take the value before the target (":TATB 1 3"). ([specs/digitalprojection-dvision.yaml:1025](specs/digitalprojection-dvision.yaml#L1025))
- [ ] Confirm LPW1 is the LED power on LED models and lamp commands answer !00002 there. ([specs/digitalprojection-dvision.yaml:649](specs/digitalprojection-dvision.yaml#L649))

## digitalprojection-evision-6500 — Digital Projection E-Vision Laser 6500 II (VXX command set)

- [ ] TCP 23 accepts the V commands as a raw TCP connection, with no Telnet negotiation or prompt. ([specs/digitalprojection-evision-6500.yaml:22](specs/digitalprojection-evision-6500.yaml#L22))
- [ ] A read is answered "P" followed directly by the value ("P50"), a set by "P", a failure by "F", each ended by CR. ([specs/digitalprojection-evision-6500.yaml:548](specs/digitalprojection-evision-6500.yaml#L548))
- [ ] "op power.on" powers the projector on and what it answers. ([specs/digitalprojection-evision-6500.yaml:61](specs/digitalprojection-evision-6500.yaml#L61))
- [ ] Projector ID 99 is answered by a projector set to another ID. ([specs/digitalprojection-evision-6500.yaml:18](specs/digitalprojection-evision-6500.yaml#L18))
- [ ] VXXS0317 and VXXG0319/VXXS0319 are the codes for 3D format and light mode (the hexadecimal column shows 0316 and 0318). ([specs/digitalprojection-evision-6500.yaml:581](specs/digitalprojection-evision-6500.yaml#L581))
- [ ] get_firmware_version's answer format. ([specs/digitalprojection-evision-6500.yaml:379](specs/digitalprojection-evision-6500.yaml#L379))

## digitalprojection-evision-op — Digital Projection E-Vision projectors (op command set)

- [ ] TCP 23 accepts the op commands as a raw TCP connection, with no Telnet option negotiation or prompt that the core would have to answer. ([specs/digitalprojection-evision-op.yaml:50](specs/digitalprojection-evision-op.yaml#L50))
- [ ] A get, a set and a step are each answered "OP <KEYWORD> = <value>" in capitals on one CR-ended line. ([specs/digitalprojection-evision-op.yaml:56](specs/digitalprojection-evision-op.yaml#L56))
- [ ] An execute command (power_on, reset_all) is answered with a line starting "OP" (the documents' example is garbled). ([specs/digitalprojection-evision-op.yaml:1068](specs/digitalprojection-evision-op.yaml#L1068))
- [ ] "OP <KEYWORD> = NA" and "*Illegal format#" are the only failure answers. ([specs/digitalprojection-evision-op.yaml:1904](specs/digitalprojection-evision-op.yaml#L1904))
- [ ] Commands longer than 26 bytes (set_net_ipaddr with a long address) are accepted, and no delay between commands is needed on TCP 23. ([specs/digitalprojection-evision-op.yaml:1006](specs/digitalprojection-evision-op.yaml#L1006))
- [ ] Keywords printed with capitals or a space ("Hsg.b. hue") are sent as hsg.b.hue and accepted. ([specs/digitalprojection-evision-op.yaml:579](specs/digitalprojection-evision-op.yaml#L579))
- [ ] The E-Vision 7010 and 6110 accept the 5900 and 5100 values, and answer NA to inputs and modes they lack. ([specs/digitalprojection-evision-op.yaml:1930](specs/digitalprojection-evision-op.yaml#L1930))
- [ ] Input numbering per model (input.sel starts at 1). ([specs/digitalprojection-evision-op.yaml:740](specs/digitalprojection-evision-op.yaml#L740))
- [ ] Lens memories 1 to 8 on the E-Vision Laser 9000 and 9100. ([specs/digitalprojection-evision-op.yaml:827](specs/digitalprojection-evision-op.yaml#L827))
- [ ] Confirm the 200 ms between commands the manuals ask of "RS232 by Telnet" holds when counted from each answer, and that the RS-232 port itself needs no gap (none is applied there). ([specs/digitalprojection-evision-op.yaml:59](specs/digitalprojection-evision-op.yaml#L59))

## digitalprojection-insight-laser — Digital Projection INSIGHT 4K Laser (binary protocol)

- [ ] Confirm Get Current Title is 00 85 00 00 01 02 88 (the guide prints 02 as its first byte). ([specs/digitalprojection-insight-laser.yaml:153](specs/digitalprojection-insight-laser.yaml#L153))
- [ ] Record the reply status byte (C0h in every example) for a refused or unsupported request, and whether a reply's length can exceed 255. ([specs/digitalprojection-insight-laser.yaml:158](specs/digitalprojection-insight-laser.yaml#L158))
- [ ] Confirm the running status's field order (external control, power, cooling, power processing, mode at bytes 6 to 10; light status and on/off at 14 and 15). ([specs/digitalprojection-insight-laser.yaml:129](specs/digitalprojection-insight-laser.yaml#L129))
- [ ] Confirm the light power level is at byte 12 of the Get Light Power Level reply. ([specs/digitalprojection-insight-laser.yaml:137](specs/digitalprojection-insight-laser.yaml#L137))
- [ ] Confirm TCP 43728 takes several requests on one connection and needs no delay between them. ([specs/digitalprojection-insight-laser.yaml:39](specs/digitalprojection-insight-laser.yaml#L39))

## digitalprojection-projector — Digital Projection projectors (ASCII control protocol)

- [ ] A set ("*brightness = 10") is answered "ack brightness = 10" on one CR-ended line, echoing the keyword and the value now in effect, on every series. ([specs/digitalprojection-projector.yaml:96](specs/digitalprojection-projector.yaml#L96))
- [ ] Which series answer "ACK"/"NAK" in capitals and which "ack"/"nack", and that a refusal is one line starting nack or NAK. ([specs/digitalprojection-projector.yaml:96](specs/digitalprojection-projector.yaml#L96))
- [ ] Nothing arrives on connecting to TCP 7000 (no banner or prompt) and replies end in CR only. ([specs/digitalprojection-projector.yaml:87](specs/digitalprojection-projector.yaml#L87))
- [ ] The E-Vision 8000i/10000i and 16000i answer the "*power ?" probe with a nack line rather than not at all. ([specs/digitalprojection-projector.yaml:99](specs/digitalprojection-projector.yaml#L99))
- [ ] A value outside the series' own range is refused with nack (each command takes the union of the documented ranges). ([specs/digitalprojection-projector.yaml:6671](specs/digitalprojection-projector.yaml#L6671))
- [ ] Polling settings in standby or without a source is answered nack, not left unanswered, and the connection stays usable. ([specs/digitalprojection-projector.yaml:6679](specs/digitalprojection-projector.yaml#L6679))
- [ ] The older series (TITAN 930/Quad/LED, HIGHlite 660/740, INSIGHT) accept on/off, open/close and the 3D words in lowercase. ([specs/digitalprojection-projector.yaml:6686](specs/digitalprojection-projector.yaml#L6686))
- [ ] The documented misspellings work as printed: altiutude.info (M-Vision Laser 18K, E-Vision Laser 15000/13000/10K/7500/8500) and user2.target2.cx/cy (E-Vision Laser 11000 4K). ([specs/digitalprojection-projector.yaml:6703](specs/digitalprojection-projector.yaml#L6703))
- [ ] E-Vision 8000i/10000i and 16000i auto.poweroff takes 0 to 4 for Off, 5, 10, 15 and 20 minutes (the document lists the values without numbers). ([specs/digitalprojection-projector.yaml:1347](specs/digitalprojection-projector.yaml#L1347))
- [ ] color.mode and color.max, listed with no operator for TITAN Laser, E-Vision Laser 11000 4K/4K-UHD and E-Vision 8000i/10000i/16000i, can be set and read. ([specs/digitalprojection-projector.yaml:1585](specs/digitalprojection-projector.yaml#L1585))
- [ ] The date format each series takes and reports (YYYY:MM:DD, yyyy/MM/dd or dd:MM:yyyy, DD:MM:YYYY). ([specs/digitalprojection-projector.yaml:1778](specs/digitalprojection-projector.yaml#L1778))
- [ ] The schedule day mask is sent as two hexadecimal digits (TITAN Laser, 8000i/10000i, 16000i, 11000 4K, 4K-UHD) or eight binary digits (other series). ([specs/digitalprojection-projector.yaml:3705](specs/digitalprojection-projector.yaml#L3705))
- [ ] lan.mac can be set, as the tables mark it. ([specs/digitalprojection-projector.yaml:2849](specs/digitalprojection-projector.yaml#L2849))
- [ ] Lens memories are 1 to 10 on most series and 0 to 9 on the E-Vision 6900, and "lens.save ?" returns the used-slot mask where documented. ([specs/digitalprojection-projector.yaml:2976](specs/digitalprojection-projector.yaml#L2976))
- [ ] HIGHlite Laser 4K lens.type takes 1 to 10 as printed. ([specs/digitalprojection-projector.yaml:3028](specs/digitalprojection-projector.yaml#L3028))
- [ ] remote.test and keypad.test keycode reports arrive as lines that do not start with ack, and do not disturb the next command. ([specs/digitalprojection-projector.yaml:3629](specs/digitalprojection-projector.yaml#L3629))
- [ ] E-Vision 6900 and Mercury Quad lan.dhcp, lan.power and lan.amx are 0 for on and 1 for off as documented. ([specs/digitalprojection-projector.yaml:6748](specs/digitalprojection-projector.yaml#L6748))
- [ ] INSIGHT power takes on and off (2017 guide) and whether "standby" (2016 guide) is accepted; INSIGHT "pic.mute = on" closes the picture. ([specs/digitalprojection-projector.yaml:6756](specs/digitalprojection-projector.yaml#L6756))
- [ ] INSIGHT "+x", "-x" and "#" operators (step_by, reset_setting). ([specs/digitalprojection-projector.yaml:4511](specs/digitalprojection-projector.yaml#L4511))
- [ ] A keyword sent with no operator sets that setting to its default on the series other than INSIGHT. ([specs/digitalprojection-projector.yaml:4520](specs/digitalprojection-projector.yaml#L4520))
- [ ] HIGHlite Laser II 3D supports h.keystone, v.keystone, ir.enable, osd.lang, blank.screen and the lens memories although the Rev H table leaves its column empty. ([specs/digitalprojection-projector.yaml:6775](specs/digitalprojection-projector.yaml#L6775))
- [ ] lamp.power on TITAN 930/Quad/LED and HIGHlite 660/740 below 80 or 85 is taken as that minimum, and lamp hours read as HH:MM. ([specs/digitalprojection-projector.yaml:6767](specs/digitalprojection-projector.yaml#L6767))
- [ ] The format of the information replies (temperatures, fan speeds as "a / b / c", hours, refresh rates) and that each is one line. ([specs/digitalprojection-projector.yaml:4530](specs/digitalprojection-projector.yaml#L4530))
- [ ] 3D format words on the older series (auto, seq, fpack, tab, sbs; INSIGHT also off, dplr, dpew). ([specs/digitalprojection-projector.yaml:937](specs/digitalprojection-projector.yaml#L937))
- [ ] E-Vision Laser 15000 test.pattern numbering, which the document lists with 8 and 9 twice. ([specs/digitalprojection-projector.yaml:3939](specs/digitalprojection-projector.yaml#L3939))
- [ ] Mercury Quad and E-Vision 6900 lamp.mode, lamp.power (E-Vision 6900 0-26 for 80-100%) and lamp.pow (Mercury Quad 77-100). ([specs/digitalprojection-projector.yaml:2789](specs/digitalprojection-projector.yaml#L2789))
- [ ] 3 s is long enough for the ack to power, lens, calibration and memory commands. ([specs/digitalprojection-projector.yaml:92](specs/digitalprojection-projector.yaml#L92))

## disguise — disguise Designer

- [ ] Confirm the Live Update subscription to one Python dictionary property of transportmanager:<name> (player.playing, player.tRender, player.playMode.state, player.track.description, engaged, volume, brightness) and the valuesChanged messages it produces. ([specs/disguise.yaml:1348](specs/disguise.yaml#L1348))
- [ ] Check that a Locator with only a name (no uid key) is accepted by every command that names a resource. ([specs/disguise.yaml:2459](specs/disguise.yaml#L2459))
- [ ] Check that gototime takes seconds and gotoframe a frame count, and the units of track length and annotation times. ([specs/disguise.yaml:2469](specs/disguise.yaml#L2469))
- [ ] Check the status.code values Designer returns (4000 seen for a rejected play mode) and the shape of status.details. ([specs/disguise.yaml:2454](specs/disguise.yaml#L2454))
- [ ] Check whether volume and brightness outside 0 to 1 are ignored, as reported for r34. ([specs/disguise.yaml:2470](specs/disguise.yaml#L2470))
- [ ] Check which Designer release added each endpoint used here; the Swagger documents carry no version markers. ([specs/disguise.yaml:183](specs/disguise.yaml#L183))
- [ ] Check the health severity values (ready in the Swagger document, ok in the monitoring guide). ([specs/disguise.yaml:2247](specs/disguise.yaml#L2247))
- [ ] Confirm /api/session/transport/transports lists every transport and /api/session/status/health every machine of the session: each poll replaces transports and machines. ([specs/disguise.yaml:1356](specs/disguise.yaml#L1356))
- [ ] Confirm a successful request answers with status.code 0 written out (not left out as a proto3 default), and that empty lists (multitransports, actors, states) come as [] rather than being left out: every read-back and list replacement keys on them. ([specs/disguise.yaml:1437](specs/disguise.yaml#L1437))
- [ ] Check how failover/understudytargets keys its understudies map (by machine name or uid), kept whole as JSON text. ([specs/disguise.yaml:1555](specs/disguise.yaml#L1555))
- [ ] Confirm that runningAsMachine names the machine an understudy has taken over after a failover, and the machine itself otherwise. ([specs/disguise.yaml:2240](specs/disguise.yaml#L2240))
- [ ] Confirm that transport/annotations accepts a track's uid in the query (uid=...), as the current track's annotations are read that way. ([specs/disguise.yaml:1377](specs/disguise.yaml#L1377))
- [ ] Confirm that sequencing/indirectionresources accepts the indirection's uid in the query, and what resourceType holds (a type name such as VideoClip is assumed). ([specs/disguise.yaml:1583](specs/disguise.yaml#L1583))
- [ ] Check whether changeindirections takes effect on every machine at once, as assumed. ([specs/disguise.yaml:2524](specs/disguise.yaml#L2524))
- [ ] Confirm that renderstream/layerstatus and layerconfig accept a layer's uid in the query, as each layer is read that way. ([specs/disguise.yaml:1606](specs/disguise.yaml#L1606))
- [ ] Check the words Designer uses for a workload instance's state and a stream's statusString, and the clock tNow, tLastDropped and tLastError are on. ([specs/disguise.yaml:2278](specs/disguise.yaml#L2278))
- [ ] Check what renderstream/failover does (taken as failing a render node over to an understudy of its pool) and what failoverpool changes, as the Swagger summaries are brief. ([specs/disguise.yaml:784](specs/disguise.yaml#L784))
- [ ] Confirm every HTTP Sockpuppet patch has a non-empty uid (patches are keyed by it) and what address looks like. ([specs/disguise.yaml:1720](specs/disguise.yaml#L1720))
- [ ] Check that an empty easingFunction is accepted (Designer's default easing) for a float change, and the field type words (float, string, resource assumed). ([specs/disguise.yaml:832](specs/disguise.yaml#L832))
- [ ] Check that floatValue.value is the target and currentValue the value now during a timed change, and whether a set_live_ command persists in the project or is live only. ([specs/disguise.yaml:2314](specs/disguise.yaml#L2314))
- [ ] Confirm that a CDL's slope, offset and power x, y and z are red, green and blue, and that a CDL Locator by name alone (uid absent) is accepted inside colour/cdl. ([specs/disguise.yaml:2553](specs/disguise.yaml#L2553))
- [ ] Confirm that POST note creates a note when none has the name ("Create/Update"), that notes with no start or count returns every note, and what makes a note "named". ([specs/disguise.yaml:2562](specs/disguise.yaml#L2562))
- [ ] Confirm that shotrecorder/record takes take as a decimal string (int64 in proto3 JSON) and whether stopping with engage false uses the slate and take sent. ([specs/disguise.yaml:2570](specs/disguise.yaml#L2570))
- [ ] Check what execute_python answers when the script raises (status.code, pythonLog), whether a registered module of the same name is replaced, and where pythonApiExecutionTimeout is set. ([specs/disguise.yaml:2578](specs/disguise.yaml#L2578))
- [ ] Check whether selectcamera with an empty cameraOverride clears the override, and that observation uids are sent as decimal strings. ([specs/disguise.yaml:2597](specs/disguise.yaml#L2597))
- [ ] Find the units of a QuickCal lineup position (normalised 0 to 1, or pixels). ([specs/disguise.yaml:2608](specs/disguise.yaml#L2608))
- [ ] Find the values of the OmniCal RequestPlansFilter, the cameradiscovery discovery states, and which Designer release added the OmniCal endpoints (not in the Swagger document). ([specs/disguise.yaml:2625](specs/disguise.yaml#L2625))
- [ ] Confirm the form of media/remove's path: an array in the Swagger document, a single string on the Media page and guide; an array is sent. ([specs/disguise.yaml:1293](specs/disguise.yaml#L1293))
- [ ] Check the size, creationDate and lastWriteDate units of media/list (uint64 strings; the epoch of the dates is not documented), and whether provision overwrites a file already at the destination. ([specs/disguise.yaml:2413](specs/disguise.yaml#L2413))
- [ ] Confirm the proof of play subscription to one dictionary of subsystem:ProofOfPlaySubsystem (getRecords, called if it is a method, and getRecentRecords(n)), that it is available without enabling anything, and which release added it. ([specs/disguise.yaml:1350](specs/disguise.yaml#L1350))

## dtvideolabs-playbackpro-plus — PlaybackPro Plus

- [ ] Check the TCP port on PlaybackPro Plus 3.8 (7000 in its guide) and on Plus X (4647), and whether the UDP port takes the same commands. ([specs/dtvideolabs-playbackpro-plus.yaml:416](specs/dtvideolabs-playbackpro-plus.yaml#L416))
- [ ] Record what PlaybackPro Plus sends back for an inquiry (PS, TE, AN) and for a command that changes something: the line ending, if any, and whether commands are answered at all; and whether a command sent with a CR or CRLF after it is still accepted. ([specs/dtvideolabs-playbackpro-plus.yaml:425](specs/dtvideolabs-playbackpro-plus.yaml#L425))
- [ ] Check what SF answers for an unnamed show and VU/GU for a clip with no number (nothing, an empty line, or N/A). ([specs/dtvideolabs-playbackpro-plus.yaml:431](specs/dtvideolabs-playbackpro-plus.yaml#L431))
- [ ] Check that GO, GT, SI and TI take a number without leading zeros (GO7) and the largest number they accept. ([specs/dtvideolabs-playbackpro-plus.yaml:437](specs/dtvideolabs-playbackpro-plus.yaml#L437))
- [ ] Record the ID inquiry's answer, to read each clip's number and file name apart. ([specs/dtvideolabs-playbackpro-plus.yaml:453](specs/dtvideolabs-playbackpro-plus.yaml#L453))

## dsan-limitimer — DSAN Limitimer

- [ ] Confirm the TP-2000XLT takes commands ended by a carriage return alone and sends its messages with the > prefix and a CR (the PDF writes /x0D and \x0D). ([specs/dsan-limitimer.yaml:195](specs/dsan-limitimer.yaml#L195))
- [ ] Record the time strings in hours mode and over 99 minutes, and in overtime. ([specs/dsan-limitimer.yaml:199](specs/dsan-limitimer.yaml#L199))
- [ ] Check whether the TP-2000XLT sends the LED states on connecting or only on change, and whether flow control is needed. ([specs/dsan-limitimer.yaml:188](specs/dsan-limitimer.yaml#L188))
- [ ] Confirm INCMIN and INCHOUR (struck through in the PDF) are no longer taken. ([specs/dsan-limitimer.yaml:203](specs/dsan-limitimer.yaml#L203))
- [ ] Record what a Global Cache iTach (or other gateway) adds between messages (the Companion module skips a byte after each CR). ([specs/dsan-limitimer.yaml:41](specs/dsan-limitimer.yaml#L41))

## dsan-perfectcue — DSAN PerfectCue

- [ ] Confirm the TP-2000X-PC takes commands ended by a carriage return alone and sends its LED messages with the > prefix and a CR. ([specs/dsan-perfectcue.yaml:103](specs/dsan-perfectcue.yaml#L103))
- [ ] Check whether the LED states are sent on connecting or only on change, and whether flow control is needed. ([specs/dsan-perfectcue.yaml:96](specs/dsan-perfectcue.yaml#L96))

## eeg-smart-encoder — EEG / AI-Media caption encoders

- [ ] Record what the encoder answers to a command and to the status commands (?, O, +, A, b, S, e), so replies can be checked and status read into state. ([specs/eeg-smart-encoder.yaml:250](specs/eeg-smart-encoder.yaml#L250))
- [ ] Record the HD492 serial ports' stop bits and flow control (1 stop bit and none assumed from the HD1492 manual). ([specs/eeg-smart-encoder.yaml:264](specs/eeg-smart-encoder.yaml#L264))
- [ ] Check whether Telnet needs a login and which port the Telnet page offers by default. ([specs/eeg-smart-encoder.yaml:261](specs/eeg-smart-encoder.yaml#L261))
- [ ] Check whether a CR after CTRL+C is harmful or needed when ending a mode (the spec sends CTRL+C alone). ([specs/eeg-smart-encoder.yaml:123](specs/eeg-smart-encoder.yaml#L123))
- [ ] Define the HD492 PassThru pairing argument, and the message command parameters (Title, Channel, Repeat, K/D, O/H, N/L). ([specs/eeg-smart-encoder.yaml:272](specs/eeg-smart-encoder.yaml#L272))

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

## epiphan-pearl — Epiphan Pearl-2, Pearl Mini, Pearl Nano and Pearl Nexus

- [ ] Confirm the default user name (admin), that an operator account may use the REST API for the commands an operator has in the Admin panel, and whether a factory-fresh unit on firmware 4.14.2 or later accepts any request before passwords are set. ([specs/epiphan-pearl.yaml:66](specs/epiphan-pearl.yaml#L66))
- [ ] Check what the Pearl answers for a wrong password (401 expected) and for a role without the right (403 or other): only 401 is taken as a refused credential. ([specs/epiphan-pearl.yaml:60](specs/epiphan-pearl.yaml#L60))
- [ ] Confirm the REST API is served on HTTPS port 443 once HTTPS is enabled, and whether its certificate is self-signed by default (the spec does not check it). ([specs/epiphan-pearl.yaml:61](specs/epiphan-pearl.yaml#L61))
- [ ] Confirm GET /system/firmware answers without delay on every model and firmware, as the liveness probe. ([specs/epiphan-pearl.yaml:63](specs/epiphan-pearl.yaml#L63))
- [ ] Check the Pearl Nano and Pearl Nexus command sets: which inputs, outputs, Single Touch and storage commands each has (the document names models only in input examples). ([specs/epiphan-pearl.yaml:149](specs/epiphan-pearl.yaml#L149))
- [ ] Check whether input requests take the input list's long id (e.g. D2P496187.hdmi-a) or only the short form the document's examples use (hdmi-a); settings reads are queued with the list's id. ([specs/epiphan-pearl.yaml:1798](specs/epiphan-pearl.yaml#L1798))
- [ ] Check which inputs answer GET /inputs/{sid}/settings: the document shows SRT and analog inputs answering and HDMI, SDI, USB, NDI and web graphics refusing with 405; RTSP is read on the assumption it answers, and analog inputs are recognized by an id ending in analog, analog-a or analog-b with type embedded. ([specs/epiphan-pearl.yaml:1798](specs/epiphan-pearl.yaml#L1798))
- [ ] Check that PATCH of input settings works for SRT and analog audio inputs and is refused for the others, and that PUT works for HDMI, SDI and USB inputs. ([specs/epiphan-pearl.yaml:1215](specs/epiphan-pearl.yaml#L1215))
- [ ] Find the maximum analog capture gain per input (the document gives a minimum of 0 only; the spec allows 0 to 100). ([specs/epiphan-pearl.yaml:644](specs/epiphan-pearl.yaml#L644))
- [ ] Confirm that set_usb_camera_settings (a PUT with only the usb section) changes the USB camera and leaves the input's other settings alone. ([specs/epiphan-pearl.yaml:791](specs/epiphan-pearl.yaml#L791))
- [ ] Find the No Signal timeout's maximum (the document gives none; the spec allows 0 to 3600 seconds). ([specs/epiphan-pearl.yaml:715](specs/epiphan-pearl.yaml#L715))
- [ ] Confirm a typed SRT input PATCH needs the input's current mode in the srt section, and what a PATCH naming another mode does. ([specs/epiphan-pearl.yaml:726](specs/epiphan-pearl.yaml#L726))
- [ ] Confirm a publisher PATCH of one section field (disable_audio, url, encryption) needs no type or mode beside it, as the document's examples show. ([specs/epiphan-pearl.yaml:395](specs/epiphan-pearl.yaml#L395))
- [ ] Confirm the PATCH replies' flattened shape (started, single-touch, disable-audio...) on current firmware; the spec reads settings back with a GET instead. ([specs/epiphan-pearl.yaml:1206](specs/epiphan-pearl.yaml#L1206))
- [ ] Find the units of the SRT statistics send_rate, estimated_bandwidth and rtt (the document's example only), and whether other publisher types report statistics. ([specs/epiphan-pearl.yaml:1312](specs/epiphan-pearl.yaml#L1312))
- [ ] Check the shape of a publisher's warnings (an array of {text}) as the poll reads it. ([specs/epiphan-pearl.yaml:1613](specs/epiphan-pearl.yaml#L1613))
- [ ] Confirm the active layout's sources object holds video and audio arrays of {id, name}, as the document's example shows (its schema leaves it open). ([specs/epiphan-pearl.yaml:1281](specs/epiphan-pearl.yaml#L1281))
- [ ] Check that GET /channels honors ids= together with the include flags (set_active_layout's read-back). ([specs/epiphan-pearl.yaml:1735](specs/epiphan-pearl.yaml#L1735))
- [ ] Confirm the output source can only be set (no read) and that the source query takes input ids, channel ids, multiview, deviceinfo and console. ([specs/epiphan-pearl.yaml:818](specs/epiphan-pearl.yaml#L818))
- [ ] Confirm the recorders ids filter takes a comma-separated list sent unencoded (ids=1,2). ([specs/epiphan-pearl.yaml:878](specs/epiphan-pearl.yaml#L878))
- [ ] Check the ad-hoc session replies: the document gives the session object without a status member, unlike every other reply, and does not say what GET answers with no session. ([specs/epiphan-pearl.yaml:986](specs/epiphan-pearl.yaml#L986))
- [ ] Check that a created ad-hoc event's reply (201) holds the event with its id, and how Panopto and Opencast events appear before the schedule is refreshed. ([specs/epiphan-pearl.yaml:934](specs/epiphan-pearl.yaml#L934))
- [ ] Confirm that only the external storage reports transfer progress (main answers 405; maintenance is not documented). ([specs/epiphan-pearl.yaml:2126](specs/epiphan-pearl.yaml#L2126))
- [ ] Confirm the Single Touch control's id is 0 on every model. ([specs/epiphan-pearl.yaml:1095](specs/epiphan-pearl.yaml#L1095))
- [ ] Check how long run_speedtest takes and whether the Pearl answers before the 5-second request timeout for short durations. ([specs/epiphan-pearl.yaml:1029](specs/epiphan-pearl.yaml#L1029))
- [ ] Check whether apply_preset with no body applies every section, as the document's optional body suggests. ([specs/epiphan-pearl.yaml:1113](specs/epiphan-pearl.yaml#L1113))

## epson-projector — Epson projectors (ESC/VP21)

- [ ] Confirm the projector's answer to the ESC/VP.net connect header (its first ten bytes "ESC/VP.net" are all the module checks), and how a projector set to Protected or with a Monitor Password answers it. ([specs/epson-projector.yaml:4075](specs/epson-projector.yaml#L4075))
- [ ] Check whether TCP 3629 keeps an idle connection open, and how the projector answers a second connection. ([specs/epson-projector.yaml:4075](specs/epson-projector.yaml#L4075))
- [ ] Confirm the reply's colon comes at the start of a line, after CR, for every query, including QC? (four lines) and SOURCELIST?. ([specs/epson-projector.yaml:2554](specs/epson-projector.yaml#L2554))
- [ ] Find which serial speeds each model offers (SPEED lists 9600-57600 on the models that have it). ([specs/epson-projector.yaml:56](specs/epson-projector.yaml#L56))
- [ ] Measure power on and power off answer times on laser models (110 s is allowed). ([specs/epson-projector.yaml:2490](specs/epson-projector.yaml#L2490))
- [ ] Check how LAMP? answers on two-lamp models (one number per lamp is assumed). ([specs/epson-projector.yaml:3835](specs/epson-projector.yaml#L3835))
- [ ] Check whether the sheets spelling PLUG-IN take PLUGIN, and that TRACKING and GAING are the keywords the workbook prints as TRACKIOK and GAIOK. ([specs/epson-projector.yaml:3162](specs/epson-projector.yaml#L3162))
- [ ] Confirm each family's SOURCE codes against its sheet, and which commands answer ERR in standby with the network on. ([specs/epson-projector.yaml:2726](specs/epson-projector.yaml#L2726))

## etc-echo — ETC Echo

- [ ] Check whether set commands (pst act, off, seq act, zone int, chan int, ras, low, lok, ulok) get any reply over UDP (they report unverified here). ([spec](specs/etc-echo.yaml#L380))
- [ ] Record the reply to E$pst get on an Integration Interface and an EchoTouch: E>pst act: space, preset assumed, with 0 for no active preset. ([spec](specs/etc-echo.yaml#L302))
- [ ] Record the reply to E$off get: E>space off: space, 0 or 1 assumed. ([spec](specs/etc-echo.yaml#L312))
- [ ] Record the reply to E$seq get: E>seq act: space, sequence assumed, with 0 for no running sequence, and what it says with two sequences running. ([spec](specs/etc-echo.yaml#L322))
- [ ] Record the reply to E$zone int get: E>zone int: space, zone, level assumed, all of a space's zones in one datagram, with or without an EOM between them. ([spec](specs/etc-echo.yaml#L466))
- [ ] On an EchoTouch, record the reply to E$lok get and the lock status a subscriber receives (E>lok: space, 0 or 1 assumed). ([spec](specs/etc-echo.yaml#L469))
- [ ] On an EchoTouch with a Channel-format subscriber, record the reply to E$chan int get and the channel updates (E>chan int: channel, level assumed), and whether the reply is one datagram or many. ([spec](specs/etc-echo.yaml#L472))
- [ ] Record the preset altered state message an EchoTouch sends subscribers (CSCON-49497), which no state rule reads yet. ([spec](specs/etc-echo.yaml#L391))
- [ ] Record what E$sync get: 0 returns (which messages, how many datagrams), and whether gets from a non-subscriber are answered with the device in All Incoming mode. ([spec](specs/etc-echo.yaml#L350))
- [ ] Confirm that this host must be a subscriber, with feedback_port as its port, for its commands to be accepted in Subscribers Only mode, and that subscriber updates arrive on that port. ([spec](specs/etc-echo.yaml#L373))
- [ ] Confirm that a time of 99.0 makes the output products use their default timing, as ETC documents for any time outside 0-25.4. ([spec](specs/etc-echo.yaml#L145))
- [ ] Confirm that times with one decimal (3.5, 0.0) are accepted. ([spec](specs/etc-echo.yaml#L133))
- [ ] Confirm that E$chan int get: with nothing after the colon (as documented) is answered. ([spec](specs/etc-echo.yaml#L345))
- [ ] Check whether a 10-second E$sync get: 0 poll is light enough on a full 16-space system, and that it catches a space off becoming false, which subscribers are not told (EEB-47104). ([spec](specs/etc-echo.yaml#L454))

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
- [ ] Check that a route (.S) is answered only by a .U update, and that .E comes only for commands the router cannot parse. ([specs/evertz-quartz.yaml:465](specs/evertz-quartz.yaml#L465))
- [ ] Check what an interrogate of a missing destination returns on EQX and MAGNUM (AN65 says nothing). ([specs/evertz-quartz.yaml:487](specs/evertz-quartz.yaml#L487))
- [ ] Check that numbers without leading zeros are accepted, and that replies use three digits. ([specs/evertz-quartz.yaml:508](specs/evertz-quartz.yaml#L508))
- [ ] Check whether lock and name changes made elsewhere are pushed as .BA or .RA. ([specs/evertz-quartz.yaml:535](specs/evertz-quartz.yaml#L535))
- [ ] Check which commands MAGNUM implements beyond routing, interrogate, locks, salvos and names. ([specs/evertz-quartz.yaml:535](specs/evertz-quartz.yaml#L535))
- [ ] Check whether name replies carry the number on current firmware. ([specs/evertz-quartz.yaml:524](specs/evertz-quartz.yaml#L524))
- [ ] Record the tieline source encoding (source in the lower 12 bits, level above) on a system with tielines. ([specs/evertz-quartz.yaml:499](specs/evertz-quartz.yaml#L499))
- [ ] Confirm a name write (.WD, .WS, .WL) is answered with the .RA name update and then .A, in that order, that a space within a name is accepted, and record what an invalid number or a name over eight characters answers. ([specs/evertz-quartz.yaml:401](specs/evertz-quartz.yaml#L401))

## extron-ipl — Extron IP Link control interfaces (IPL 250, IPL T S, IPL T SFI244, IPL T CR48, IPL T PCS4)

- [ ] On an IPL T S, record the copyright banner, the date line and the password prompt on a Telnet connection (not printed in its guide; taken as the IPL 250's), and on both models what a wrong password produces. ([spec](specs/extron-ipl.yaml#L1118))
- [ ] Confirm the contact input query is the input number followed by the ] character (03]), and record its tagged reply in verbose mode 3. ([spec](specs/extron-ipl.yaml#L504))
- [ ] Record the tagged forms of the relay, serial port, flow control, receive timeout and pass-through views in verbose mode 3 (Cpn02 Rly1, Cpn02 Ccp9600,N,8,1 assumed). ([spec](specs/extron-ipl.yaml#L1101))
- [ ] Check whether a relay or contact input change, from a program, a touchpanel or the input itself, sends any message on a verbose Telnet connection; if one does, the 10-second poll can be replaced by it. ([spec](specs/extron-ipl.yaml#L1101))
- [ ] Check whether the CR the core adds after send_serial_data's data is forwarded to the attached device, and how the device's answer arrives on the Telnet session (one line, ending CR LF assumed). ([spec](specs/extron-ipl.yaml#L252))
- [ ] On an IPL T S, confirm its serial ports are numbered 1 to 4 in SIS commands (the guide's key names another product's ports). ([spec](specs/extron-ipl.yaml#L1118))
- [ ] On an IPL T S, record the date and time set format (the guide's key gives none; MM/DD/YY-HH:MM:SS assumed as on the IPL 250). ([spec](specs/extron-ipl.yaml#L580))
- [ ] Record the GMT offset as set and read (-05:00 or -05.00), and the daylight saving values on the IPL T S (0-2 assumed as on the IPL 250). ([spec](specs/extron-ipl.yaml#L596))
- [ ] Record the IR reply's padding (Irs01,03,001,00 assumed) and what playing a function missing from a loaded file answers (E13 or E28). ([spec](specs/extron-ipl.yaml#L447))
- [ ] Record the receive timeout view's separator (the IPL 250 prints commas, the IPL T S an asterisk after the first field). ([spec](specs/extron-ipl.yaml#L395))
- [ ] On an IPL 250, record what a port type other than 0 (Esc 01*1CY) answers. ([spec](specs/extron-ipl.yaml#L322))
- [ ] Record the port map replies' padding (Pmt00023) and the bare views, and confirm a remapped Telnet port takes effect at once. ([spec](specs/extron-ipl.yaml#L773))
- [ ] Record the Q reply's tagged form in verbose mode 3 (Ver01*1.15 assumed) and the 0Q reply's. ([spec](specs/extron-ipl.yaml#L515))
- [ ] On an IPL T PCS4, record the tagged forms of the receptacle replies in verbose mode 3 (Cpn1 Ppc1, Pgp1120, Pdt6, Ptf2, Exe1, Ast1*2*1*61*1*0, Set2*2*1*450 assumed from the guide) and the untagged PS, TH and SS answers. ([spec](specs/extron-ipl.yaml#L823))
- [ ] On an IPL T PCS4, record which alarm mode (MA) field is which (the guide prints the symbol X70* twice) and what set_alarm_mode 0*0*0*1*4 answers. ([spec](specs/extron-ipl.yaml#L933))
- [ ] On an IPL T PCS4, check whether a receptacle, current sense, alarm or front panel change sends any message on a verbose Telnet connection; if one does, the 10-second poll can be replaced by it. ([spec](specs/extron-ipl.yaml#L1103))
- [ ] On an IPL T PCS4, confirm the alarm relay answers the relay commands as relay 1 (Cpn1 Rly1) and the view of a receptacle's alarm (Esc 1SA) has five fields. ([spec](specs/extron-ipl.yaml#L926))
- [ ] On an IPL T SFI244, record the Flex I/O replies in verbose mode 3 (Cpn02 Iom1, Cpn02 Sio1) and the view of an analog input (n] answering 0-4095). ([spec](specs/extron-ipl.yaml#L968))
- [ ] On an IPL T CR48 and SFI244, record the copyright banner (in capitals, no comma after EXTRON ELECTRONICS in the guides) and the contact input view in analog use. ([spec](specs/extron-ipl.yaml#L1310))
- [ ] On an IPL T CR48, confirm the eight relays answer as Cpn01-Cpn08 Rly and daylight saving 3 (Brazil) is accepted. ([spec](specs/extron-ipl.yaml#L215))

## extron-matrix — Extron SIS matrix switchers

- [ ] Confirm that a CR after a simple command (1*2!) is ignored. ([spec](specs/extron-matrix.yaml#L1949))
- [ ] Record what a wrong password produces (the prompt again assumed) and whether the unit sends Telnet option bytes before the banner. ([spec](specs/extron-matrix.yaml#L1959))
- [ ] Check how often a change report arrives while a command waits, and whether replies and reports can be told apart. ([spec](specs/extron-matrix.yaml#L1934))
- [ ] Record the Esc VM mute reply on DXP and XTP II (with and without the "Mut00" prefix). ([spec](specs/extron-matrix.yaml#L510))
- [ ] On DXP HD 4K PLUS, check set_input_attenuation_db at 0 dB (1*0G). ([spec](specs/extron-matrix.yaml#L1989))
- [ ] On SMX, confirm two-digit plane numbers in commands and the plane-first reply form. ([spec](specs/extron-matrix.yaml#L2003))
- [ ] On XTP II, confirm the Exec form of executive-mode reports. ([spec](specs/extron-matrix.yaml#L1997))
- [ ] On DXP HD 4K PLUS, record the EDID view replies (EdidR hex, EdidN native resolution) and the EDID import and export tags, which the guide prints as EIX5@ and EEX5@. ([spec](specs/extron-matrix.yaml#L1235))
- [ ] On XTP II, record the remote power (POEC) replies and pushes, and the all-ports forms. ([spec](specs/extron-matrix.yaml#L1339))
- [ ] On XTP II, check ETIE: which type value (1 or 2) selects video only, and the receiver form (Rev. Q). ([spec](specs/extron-matrix.yaml#L1408))
- [ ] Check which IP, DHCP and password changes end the Telnet session on each family (only DXP DVI documents it). ([spec](specs/extron-matrix.yaml#L2011))
- [ ] Check the unit name length limit on each family (63 on DXP HD 4K PLUS, 24 elsewhere) and the CT read format (SMX MM/DD/YY, others Ddd, DD Mmm YYYY). ([spec](specs/extron-matrix.yaml#L1657))
- [ ] On SMX, check the serial port (CP) syntax, which the guide prints as Esc *port,...CP, and which port numbers are the rear ports on each family. ([spec](specs/extron-matrix.yaml#L1707))
- [ ] On SMX, record a virtual plane definition and its read-back (Mpv), and whether Esc ZXXX clears virtual planes. ([spec](specs/extron-matrix.yaml#L1767))
- [ ] Check what a DXP HD 4K PLUS answers for global presets 17-32, and how DXP DVI and SMX treat names longer than 12 characters. ([spec](specs/extron-matrix.yaml#L2040))
- [ ] Check that the 2-minute poll's extra queries (network, EDID, HDCP authorization, formats, transmitter inputs, slot and virtual planes) are answered with E-codes, not silence, on the models that lack them. ([spec](specs/extron-matrix.yaml#L2040))
- [ ] On an MMX 42/62 behind a serial-to-Ethernet interface's raw TCP port (an IPL T S at 2001), confirm the CR sent after every command is ignored and the replies come back one per line, ending CR LF. ([spec](specs/extron-matrix.yaml#L2078))
- [ ] On an MMX, confirm the poll's queries that are not MMX commands are answered E10 rather than ignored (a silent unit would hold each for the 3-second timeout every 2 minutes). ([spec](specs/extron-matrix.yaml#L2092))
- [ ] On an MMX, record the part number reply (the guide prints Nnn-nnn-nn: with or without a leading N) and the bare tie views (In05). ([spec](specs/extron-matrix.yaml#L936))
- [ ] On an MMX, record the I reply's exact spacing (V1*03 A1*03 V2*05 A2*06 Vmt1*0 Amt1*0 Vmt2*1 Amt2*0 assumed) and whether input numbers are padded. ([spec](specs/extron-matrix.yaml#L2808))
- [ ] On an MMX, check whether front panel mutes or level changes send any message (only front panel ties are documented), and whether set_input_attenuation takes 0 (g with 00). ([spec](specs/extron-matrix.yaml#L306))
- [ ] On each family, drive the matrix on its rear Remote port (link serial) and confirm replies, change reports and the verbose-mode step behave as over Telnet, with the banner only at power-up. ([spec](specs/extron-matrix.yaml#L157))
- [ ] On DXP HD 4K PLUS, confirm the Remote RS-232 and USB ports ask for no password when one is set (documented for DXP DVI/HDMI, XTP II and SMX only). ([spec](specs/extron-matrix.yaml#L181))
- [ ] On DXP HD 4K PLUS, confirm the Remote port uses no flow control (not documented). ([spec](specs/extron-matrix.yaml#L2059))
- [ ] On an MVX 44-88, confirm Esc nVC (the examples) reads a preset's ties, not Esc n1VC (the symbol line). ([specs/extron-matrix.yaml:2049](specs/extron-matrix.yaml#L2049))
- [ ] On an MVX 44-88, confirm n*40# reads the output level (the example) and record its answer. ([specs/extron-matrix.yaml:2101](specs/extron-matrix.yaml#L2101))
- [ ] On an MVX 128-1616, record the Esc VM answer (a digit per output assumed, with or without separators). ([specs/extron-matrix.yaml:3477](specs/extron-matrix.yaml#L3477))
- [ ] On an MVX, confirm front panel ties are reported as Qik only, and record the answer of a bare tie query (12 or In12). ([specs/extron-matrix.yaml:406](specs/extron-matrix.yaml#L406))
- [ ] On an MTPX Plus in verbose mode 3, record the VC answer (tagged Vgp or bare) for starting outputs 1 and 17. ([specs/extron-matrix.yaml:2842](specs/extron-matrix.yaml#L2842))
- [ ] On an MTPX Plus, record the answers of the twisted-pair commands (Typ, Lrpt, Ipek, Iseq, Oseq, Opek, Opol) in verbose mode 3. ([specs/extron-matrix.yaml:2202](specs/extron-matrix.yaml#L2202))
- [ ] On an MTPX Plus, confirm the input grouping answer (GrI in the example, Gri in the syntax). ([specs/extron-matrix.yaml:2373](specs/extron-matrix.yaml#L2373))
- [ ] On an MMX 32 VGA A, confirm the capitalized answers (OUT02 IN01 ALL, VMT2*1, AMT1) and the I answer. ([specs/extron-matrix.yaml:443](specs/extron-matrix.yaml#L443))

## extron-switcher — Extron IN-series scaling presentation switchers

- [ ] Record the tagged reply forms in verbose mode 3 (In X!*1 All, VnamI, Inf00, GrpmD padding). ([spec](specs/extron-switcher.yaml#L2424))
- [ ] Confirm that a CR after a simple command (1*3!, 2B, 1X) is ignored. ([spec](specs/extron-switcher.yaml#L2418))
- [ ] Check how often a change report arrives while a command waits. ([spec](specs/extron-switcher.yaml#L2402))
- [ ] On IN1606/IN1608, check whether the unsolicited signal message (IN00 ...) is sent, as on IN1808. ([spec](specs/extron-switcher.yaml#L2457))
- [ ] Confirm the HDCP status codes on each family (1 and 2 are documented the opposite way). ([spec](specs/extron-switcher.yaml#L2482))
- [ ] Record what a wrong password produces (the prompt again assumed). ([spec](specs/extron-switcher.yaml#L153))
- [ ] Confirm both families answer N with the part number, the IN1808 tagged (Pno60-1615-02) and the IN1606/IN1608 bare, so the reads on connecting are queued. ([spec](specs/extron-switcher.yaml#L2630))
- [ ] On an IN1606/IN1608, record whether the current-input picture queries (Esc CONT, BRIT, HDET, COLR, TINT) answer tagged with the input (Cont3*060) in verbose mode 3; a bare value is not kept. ([spec](specs/extron-switcher.yaml#L2474))
- [ ] Confirm an IN1806 or IN1606 answers queries about inputs 7 and 8 with an error rather than ignoring them (which would hold the reads for the timeout). ([spec](specs/extron-switcher.yaml#L2464))
- [ ] Record the screen saver reply forms on both families (SsavM1*2, SsavT1*501, SsavS1*0 on IN1808; SsavM0, SsavCFF0000, SsavT030 on IN1608), and confirm the IN1608 custom color has no query. ([spec](specs/extron-switcher.yaml#L1045))
- [ ] Confirm the IN1808 B query answers one digit per output separated by spaces (Vmt0 1 0). ([spec](specs/extron-switcher.yaml#L2770))
- [ ] Record the change reports for the IN1608 settings added here: automatic Auto-Image (Img4*1), video format (Typ1*2), overscan (Oscn2*1) and the Auto-Image threshold (Alvl025); and the IN1808 detected format (Vtyp3*2) and audio names (AnamI10*name). ([spec](specs/extron-switcher.yaml#L2837))
- [ ] Check whether an input or user preset recall and an Auto-Image report each picture and image setting they change; if they do, the re-reads after them can go. ([spec](specs/extron-switcher.yaml#L2729))
- [ ] Confirm the auto-switch priority order is sent and answered with spaces between the inputs (Esc P8 7 1 2 3 4 5 6AUSW). ([spec](specs/extron-switcher.yaml#L650))
- [ ] Confirm the image position and size replies carry signed, zero-padded positions (Ximg1,-0010*+0020*01920*01080; IN1608 without 1,) and the shift replies HctrI1*-0002 and Hctr+0012. ([spec](specs/extron-switcher.yaml#L2882))
- [ ] Confirm the embedded-audio OIDs 30012-30015 with H: which of each pair is the left channel (the guide's example takes 30012 as the right). ([spec](specs/extron-switcher.yaml#L1331))
- [ ] On an IN1604 behind a serial-to-Ethernet interface's raw TCP port (an IPL T S at 2001), confirm the CR sent after every command is ignored, Esc 3CV is accepted on RS-232, and no password is asked. ([spec](specs/extron-switcher.yaml#L2490))
- [ ] On an IN1604, record which queries answer tagged in verbose mode 3 where the guide prints only a bare value: the analog timing and picture views with the input (Hsrt2*128, Cont2*064), Esc A EDID (EdidA2*41), Esc I AFMT, Esc OSCN, Esc MUTM, Esc CN (Ipn), 2i, and the preset names (Pnam1*01,name); bare names and descriptions cannot be read. ([spec](specs/extron-switcher.yaml#L2511))
- [ ] On an IN1604, check whether automatic Auto-Image (n*1A) works for inputs 2-4 (the guide shows input 1 only). ([spec](specs/extron-switcher.yaml#L2511))
- [ ] On an IN1604, record the unsolicited signal message (In00 with spaces between digits assumed, as in the tagged 0LS reply) and whether one is sent at all. ([spec](specs/extron-switcher.yaml#L3149))
- [ ] On an IN1604, check whether an output format change from the front panel or menu is reported (Vtpo3), and that Esc 1*VTPO's reply has the same text, which is why the format is read only from the replies to its own set and view. ([spec](specs/extron-switcher.yaml#L3060))
- [ ] On an IN1604, record the preset delete reply (printed PrstX1*X2)), and whether it also clears the preset's name. ([spec](specs/extron-switcher.yaml#L2042))
- [ ] On an IN1604, record the zero padding and sign of the gain and volume replies (Aud-06, Vol-20) and the separator of the audio mute view (0 1). ([spec](specs/extron-switcher.yaml#L2173))
- [ ] On an IN1604, check the EDID and output rate codes against the guide's table (no 27 or 28) and what Esc A n*0EDID (automatic) answers. ([spec](specs/extron-switcher.yaml#L1669))
- [ ] On an IN1604, record the internal temperature reply (the guide prints xxx.x degrees Celsius) and its tagged form (20Stat). ([spec](specs/extron-switcher.yaml#L2386))
- [ ] On an IN1604, record the ranges the unit accepts for total pixels, active pixels and active lines (the guide gives only +/-512 or 256 of the signal's default). ([spec](specs/extron-switcher.yaml#L1786))
- [ ] On IN1806/IN1808 and IN1606/IN1608, confirm the Remote RS-232 port asks for no password when one is set (not documented either way). ([spec](specs/extron-switcher.yaml#L150))
- [ ] Confirm 8 data bits on the IN1606/IN1608 and IN1604 Remote port (not stated) and which stop bits the IN1806/IN1808 Esc...CP command accepts (1 assumed). ([spec](specs/extron-switcher.yaml#L126))
- [ ] Drive a unit on its rear Remote port (link serial) and confirm the same replies and change reports as over Telnet; check whether the front USB port appears as a serial port. ([spec](specs/extron-switcher.yaml#L2499))

## ezcoo-matrix — Ezcoo HDMI matrices

- [ ] Record the RS-232 line settings (57600 8N1 assumed). ([specs/ezcoo-matrix.yaml:54](specs/ezcoo-matrix.yaml#L54))
- [ ] Record what each command answers and the lines EZSTA prints (the OUT1 VS IN1 form is assumed from a community module). ([specs/ezcoo-matrix.yaml:50](specs/ezcoo-matrix.yaml#L50))
- [ ] Confirm the matrix sends a status line unasked after a front-panel or remote change. ([specs/ezcoo-matrix.yaml:13](specs/ezcoo-matrix.yaml#L13))
- [ ] Record the full EZH command list and the MX42HS and other models it applies to. ([specs/ezcoo-matrix.yaml:69](specs/ezcoo-matrix.yaml#L69))

## facebook-account — Facebook account (a user's Pages and Page access tokens)

- [ ] Confirm an invalid or expired user token answers HTTP 400 with error code 190 on /me and /me/accounts, as for a Page token. ([specs/facebook-account.yaml:107](specs/facebook-account.yaml#L107))
- [ ] Confirm GET /{api_version}/me?fields=id,name with a user token answers the app-scoped id and name, and record what the 10-second idle probe costs against the app's Platform rate limit. ([specs/facebook-account.yaml:110](specs/facebook-account.yaml#L110))
- [ ] Confirm GET /me/accounts with fields id,name,category,tasks,access_token answers every Page the user has a role on, each with a Page access token, and record whether Pages owned by a business portfolio appear for a user with a task on them through the portfolio. ([specs/facebook-account.yaml:146](specs/facebook-account.yaml#L146))
- [ ] Confirm paging: 25 Pages by default, limit up to 100 accepted, paging.next present only while there are more, and after with paging.cursors.after giving the next Pages; record the cursor's characters. ([specs/facebook-account.yaml:157](specs/facebook-account.yaml#L157))
- [ ] Confirm GET /{page-id}?fields=id,name,access_token with the user token answers the same Page token /me/accounts gives, and record what it answers for a Live Contributor or a Page the user has no role on. ([specs/facebook-account.yaml:197](specs/facebook-account.yaml#L197))
- [ ] Confirm GET /me/permissions with the user's own token lists each permission with status granted, declined or expired. ([specs/facebook-account.yaml:140](specs/facebook-account.yaml#L140))
- [ ] Confirm a Page token read with a short-lived user token stops working after about 1 hour, and that one read with a long-lived user token reports no expiry (debug_token expires_at 0). ([specs/facebook-account.yaml:239](specs/facebook-account.yaml#L239))
- [ ] Confirm which Page task creating and publishing a live video needs (CREATE_CONTENT expected), and what facebook-live answers with a Page token lacking it. ([specs/facebook-account.yaml:275](specs/facebook-account.yaml#L275))

## facebook-live — Facebook Live

- [ ] Confirm an invalid or expired Page token answers HTTP 400 with error code 190 (and which subcodes), that a 401 is never used for other errors, and that permission and rate-limit errors carry other codes. ([specs/facebook-live.yaml:210](specs/facebook-live.yaml#L210))
- [ ] Confirm GET /{api_version}/me with a Page token answers the Page's id, so the idle probe checks the token. ([specs/facebook-live.yaml:213](specs/facebook-live.yaml#L213))
- [ ] Confirm reading GET /{page-id}/live_videos still works on v26.0 (the current reference says reading is not supported; the v19.0 reference and the scheduling guide document it), and that source=owner includes UNPUBLISHED and scheduled live videos. ([specs/facebook-live.yaml:1046](specs/facebook-live.yaml#L1046))
- [ ] Record the shape of ingest_streams in a field expansion (a plain array, or an object with data), and the stream_health fields Facebook actually returns, including while the encoder is not sending. ([specs/facebook-live.yaml:1076](specs/facebook-live.yaml#L1076))
- [ ] Confirm the Graph API takes a JSON request body (Content-Type application/json) for creating and updating live videos, as it takes form-encoded parameters. ([specs/facebook-live.yaml:350](specs/facebook-live.yaml#L350))
- [ ] Confirm scheduling with status SCHEDULED_UNPUBLISHED and event_params start_time, and record the earliest and latest start times accepted. ([specs/facebook-live.yaml:368](specs/facebook-live.yaml#L368))
- [ ] Record which privacy values a Page's live video accepts, and whether privacy can be changed while live. ([specs/facebook-live.yaml:412](specs/facebook-live.yaml#L412))
- [ ] Confirm go_live and end_live_video answer the id (not success), and that a video ended with end_live_video reads back as VOD. ([specs/facebook-live.yaml:468](specs/facebook-live.yaml#L468))
- [ ] Confirm DELETE answers 200 with success true, and whether a live video can be deleted while live. ([specs/facebook-live.yaml:490](specs/facebook-live.yaml#L490))
- [ ] Confirm master_ingest_stream_id switches between primary and backup ingest streams on a live video created with enable_backup_ingest. ([specs/facebook-live.yaml:440](specs/facebook-live.yaml#L440))
- [ ] Confirm comments and reactions with summary=total_count and limit=0 answer the total count only, and which permission comments need with a Page token. ([specs/facebook-live.yaml:534](specs/facebook-live.yaml#L534))
- [ ] Confirm seconds_left counts down from the 8-hour maximum while live, and what it reads before and after. ([specs/facebook-live.yaml:1019](specs/facebook-live.yaml#L1019))
- [ ] Record the Page Business Use Case usage the 15-second poll costs (X-Business-Use-Case-Usage), on a Page with few engaged users. ([specs/facebook-live.yaml:1028](specs/facebook-live.yaml#L1028))
- [ ] Confirm a request naming a retired Graph API version is answered by the oldest available one, rather than refused. ([specs/facebook-live.yaml:222](specs/facebook-live.yaml#L222))
- [ ] Confirm commenting and replying as the Page answers the new comment's id, and that hiding and deleting answer success true with pages_manage_engagement. ([specs/facebook-live.yaml:556](specs/facebook-live.yaml#L556))
- [ ] Confirm the comments edge answers is_hidden and comment_count when asked for in fields. ([specs/facebook-live.yaml:511](specs/facebook-live.yaml#L511))
- [ ] Confirm rescheduling takes event_params as an object with start_time in a JSON body (the guide's sample passes the timestamp itself as event_params), and that it answers the id. ([specs/facebook-live.yaml:609](specs/facebook-live.yaml#L609))
- [ ] Confirm status SCHEDULED_CANCELED is accepted on an update of a scheduled live video, and what the video reads as afterwards. ([specs/facebook-live.yaml:623](specs/facebook-live.yaml#L623))
- [ ] Confirm a live poll created with POST /{live_video_id}/polls starts hidden, and that SHOW_VOTING shows it with status voting_open; record limits on the question and the number of options. ([specs/facebook-live.yaml:657](specs/facebook-live.yaml#L657))
- [ ] Confirm the polls edge answers poll_options as an edge ({"data": [...]}) with total_votes, and that votes count while the poll is open. ([specs/facebook-live.yaml:1115](specs/facebook-live.yaml#L1115))
- [ ] Confirm each poll action (CLOSE, SHOW_RESULTS, ATTACH_TO_VIDEO, DELETE_POLL) answers success true, and what ATTACH_TO_VIDEO does to a poll on a live video that has ended. ([specs/facebook-live.yaml:727](specs/facebook-live.yaml#L727))
- [ ] Confirm begin_crossposting_handshake and accept_crossposting_handshake are accepted in a JSON body as arrays of objects, and answer success true. ([specs/facebook-live.yaml:815](specs/facebook-live.yaml#L815))
- [ ] Confirm crossposting_actions in a JSON body answers the live video with crosspost_shared_pages and crossposted_broadcasts as edges, when asked for in fields on the POST. ([specs/facebook-live.yaml:780](specs/facebook-live.yaml#L780))

## figure53-go-button — Figure 53 Go Button

- [ ] Check the address a rootless query is answered on: /reply/<the address sent> (assumed, so query commands wait for it exactly), or with a show prefix as the dictionary's /reply/show_id/{id}/volumePercent example has it; and whether a hit read by /hit_id/{id} is answered on /reply/hit_id/{id}/... ([specs/figure53-go-button.yaml:1448](specs/figure53-go-button.yaml#L1448))
- [ ] Check that a wrong passcode answers /reply/connect with badpass over TCP and that later show and cue queries then go unanswered rather than answered with an error. ([specs/figure53-go-button.yaml:1467](specs/figure53-go-button.yaml#L1467))
- [ ] Check whether percentActionElapsed and percentPreWaitElapsed are fractions from 0 to 1 or percentages from 0 to 100. ([specs/figure53-go-button.yaml:1478](specs/figure53-go-button.yaml#L1478))
- [ ] Check the update Go Button sends when cues are added, deleted or moved: an update for the root group ("[root group of cue lists]" per a third-party client, or __root__), or /update/show/{id} alone. ([specs/figure53-go-button.yaml:1487](specs/figure53-go-button.yaml#L1487))
- [ ] Check that the * and ? wildcards work in an action address such as /hit/1*/start, and that cue and hit messages sent to /hit/{number} count hits in display order from the top left. ([specs/figure53-go-button.yaml:1496](specs/figure53-go-button.yaml#L1496))
- [ ] Check that Go Button 3.0 to 3.2 take the integers 0 and 1 as booleans (the dictionary marks OSC True and False as 3.3.0 and later). ([specs/figure53-go-button.yaml:1507](specs/figure53-go-button.yaml#L1507))
- [ ] Check the arguments of /update/show/{id}/volume (dB alone, or dB and a percentage) and whether a second update message carries the percentage. ([specs/figure53-go-button.yaml:1521](specs/figure53-go-button.yaml#L1521))
- [ ] Check what Go Button answers to show and cue queries with no show open (a third-party client reads data ignoring_show_methods) and with which status. ([specs/figure53-go-button.yaml:1529](specs/figure53-go-button.yaml#L1529))
- [ ] Check that /show/{displayName}/open opens a show by display name, and that rootless show messages reach the open show. ([specs/figure53-go-button.yaml:1536](specs/figure53-go-button.yaml#L1536))

## focusrite-rednet — Focusrite RedNet (AES70)

- [ ] Note the OCP.1 port a RedNet unit advertises as _oca._tcp once AES70 is enabled in RedNet Control, and check it accepts OCP.1 on plain TCP. ([specs/focusrite-rednet.yaml:414](specs/focusrite-rednet.yaml#L414))
- [ ] On an MP8R, check which object numbers and roles the firmware uses: the implementation chart's (0x01nn00pp, GComp-n, IpLvl-n) or the virtual device's (4352 + 256 per channel, TrimEnable-n, Lvl-n). ([specs/focusrite-rednet.yaml:421](specs/focusrite-rednet.yaml#L421))
- [ ] Check the switch positions: Phase-n 1 inverts, Impedance-n 0 is 10 kOhm, Pad-n 1 is -20 dB, and the unit-wide Identify, FP Lock and Preferred Master switches take 1 for on. ([specs/focusrite-rednet.yaml:430](specs/focusrite-rednet.yaml#L430))
- [ ] Read the PSU, network and clock leader sensors on a unit and note what each value means. ([specs/focusrite-rednet.yaml:443](specs/focusrite-rednet.yaml#L443))
- [ ] On a RedNet 4, RedNet 1 or 2, check the roles match the virtual device's lists (Source-n positions mic, line, DI; Link1-2 to Link7-8; Fan). ([specs/focusrite-rednet.yaml:430](specs/focusrite-rednet.yaml#L430))
- [ ] Walk an A16R MkII, D16R MkII or X2P with AES70 enabled and record its object tree, so their controls can be named. ([specs/focusrite-rednet.yaml:414](specs/focusrite-rednet.yaml#L414))

## focusrite-rednet-rcp — Focusrite RedNet MP8R (Yamaha RCP)

- [ ] Confirm the MP8R answers RCP on TCP 49280 directly, and whether it needs a Yamaha ID set in RedNet Control for it. ([specs/focusrite-rednet-rcp.yaml:416](specs/focusrite-rednet-rcp.yaml#L416))
- [ ] Check the get and set replies follow Yamaha's grammar (OK get ... value, OK set, OKm when clamped, ERROR) and that changes made in RedNet Control are pushed as NOTIFY set. ([specs/focusrite-rednet-rcp.yaml:409](specs/focusrite-rednet-rcp.yaml#L409))
- [ ] Check how HAGain 0-66 maps onto the MP8R's 10-65 dB preamp range, and what CompGain and ExecMode mean. ([specs/focusrite-rednet-rcp.yaml:423](specs/focusrite-rednet-rcp.yaml#L423))
- [ ] Try the same addresses on a RedNet 4. ([specs/focusrite-rednet-rcp.yaml:438](specs/focusrite-rednet-rcp.yaml#L438))
- [ ] Send mtrstart IO:Current/InCh/InputLevel 100 and confirm the unit answers OK mtrstart, the address it accepts (with or without /Meter), and the NOTIFY mtr line's form (address, a type word or none, eight two-digit hex values); record what 0-127 means in dB, and the shortest interval accepted. ([specs/focusrite-rednet-rcp.yaml:254](specs/focusrite-rednet-rcp.yaml#L254))
- [ ] Check whether the meters stop by themselves (as Yamaha meters time out) and need mtrstart again, and what mtrstop answers. ([specs/focusrite-rednet-rcp.yaml:349](specs/focusrite-rednet-rcp.yaml#L349))

## foscam-ptz — Foscam HD IP cameras (CGI)

- [ ] Check how current firmware answers a wrong password (result -2 with HTTP 200 assumed) and whether it locks the account out after repeated failures. ([specs/foscam-ptz.yaml:860](specs/foscam-ptz.yaml#L860))
- [ ] Check whether setContrast takes "contrast" (the parameter table) or "constrast" (the guide's example). ([specs/foscam-ptz.yaml:874](specs/foscam-ptz.yaml#L874))
- [ ] Record getPTZPresetPointList and ptzGetCruiseMapList replies to confirm the cnt then pointN or mapN element order the rules read. ([specs/foscam-ptz.yaml:784](specs/foscam-ptz.yaml#L784))
- [ ] Check which commands newer firmware adds or renames against the V1.0.4 guide (PTZ position, IR modes, newer image settings). ([specs/foscam-ptz.yaml:881](specs/foscam-ptz.yaml#L881))
- [ ] Confirm /configs/export/configs.bin serves the exported file right after exportConfig answers, without usr and pwd. ([specs/foscam-ptz.yaml:548](specs/foscam-ptz.yaml#L548))
- [ ] Confirm fwUpgrade answers its upgradeResult within three minutes of the upload (the guide gives 120 s for application firmware and 60 s for system firmware), and that importConfig and fwUpgrade accept any part file name. ([specs/foscam-ptz.yaml:568](specs/foscam-ptz.yaml#L568))

## freeshow — FreeShow

- [ ] Record the status code and body FreeShow's REST API answers to an action (204 assumed) and to a get_ query (200 with JSON assumed), and to an unknown action. ([specs/freeshow.yaml:1492](specs/freeshow.yaml#L1492))
- [ ] Check whether positions (index_select_slide, index_select_project, index_select_project_item, index_select_overlay) count from 1 or from 0. ([specs/freeshow.yaml:1500](specs/freeshow.yaml#L1500))
- [ ] Check that a get_cleared POST is harmless and answered while nothing is on output, since it is the liveness probe. ([specs/freeshow.yaml:65](specs/freeshow.yaml#L65))
- [ ] With an API password set, confirm a wrong or missing Bearer token gets 401 on REST, and that versions before 1.6.6-beta.4 ignore the header. ([specs/freeshow.yaml:1484](specs/freeshow.yaml#L1484))
- [ ] Check that numbers sent as JSON numbers are accepted (the Companion module sends every number and boolean as a string). ([specs/freeshow.yaml:324](specs/freeshow.yaml#L324))
- [ ] Check the change_volume range (0-1 assumed, from the Companion module) and the transition types and easing names. ([specs/freeshow.yaml:771](specs/freeshow.yaml#L771))
- [ ] Check that set_plain_text and create_show take line breaks in the text and split slides on a blank line. ([specs/freeshow.yaml:218](specs/freeshow.yaml#L218))
- [ ] Record what get_shows, get_output, get_slide, get_timers and get_cleared return, so state rules can be written for them. ([specs/freeshow.yaml:1208](specs/freeshow.yaml#L1208))
- [ ] Confirm that FreeShow's Socket.IO server on 5505 (Engine.IO 4 assumed) answers the variable request {"isVariable": true, "keys": [...]} on the "data" event with {isVariable: true, values: {...}}, as the Companion module receives it. ([specs/freeshow.yaml:1700](specs/freeshow.yaml#L1700))
- [ ] Record the type each variable arrives as: whether slide_number and layout_slides are numbers, and whether output_windows_active, outputs_locked and log_song_usage are true/false or text. ([specs/freeshow.yaml:1755](specs/freeshow.yaml#L1755))
- [ ] Check what slide_number counts from (1 assumed for the slide shown) and what it holds with nothing on output. ([specs/freeshow.yaml:1663](specs/freeshow.yaml#L1663))
- [ ] With an API password set, confirm the socket takes it as the handshake auth {"token": key}, refuses the namespace without it, and accepts {"token": ""} when no password is set. ([specs/freeshow.yaml:1696](specs/freeshow.yaml#L1696))
- [ ] Check whether FreeShow pushes changes unasked on the socket, which would let the once-a-second variable request go slower. ([specs/freeshow.yaml:1698](specs/freeshow.yaml#L1698))

## generic-midi — Generic MIDI device

- [ ] Join a macOS Audio MIDI Setup network session (and rtpMIDI on Windows, and a network MIDI interface) and confirm the invitation on both ports is accepted, notes and SysEx arrive, and the session stays up for an hour with the core's clock synchronisation every 10 s. ([specs/generic-midi.yaml:429](specs/generic-midi.yaml#L429))
- [ ] Confirm those hosts accept the core's recovery journals (J set) and its guard packets (an empty MIDI list with a journal), and that with packets to them dropped they repair notes, controllers, programs, RPN and NRPN, pitch bend, pressure, the sequencer, MTC and SysEx from the core's journal. ([specs/generic-midi.yaml:429](specs/generic-midi.yaml#L429))
- [ ] With packet loss forced (a lossy link or dropped packets), confirm the replay of Apple's recovery journal leaves notes, controllers, programs and pitch bend as the sender has them. ([specs/generic-midi.yaml:429](specs/generic-midi.yaml#L429))
- [ ] Host a session (link rtp_host) and join it from macOS Audio MIDI Setup (its Directory, adding the core's address and port) and from rtpMIDI on Windows: confirm both invitations are accepted, MIDI flows both ways, the initiator's clock synchronisation keeps the session up for an hour (it must come at least every 90 s), and the initiator leaving (BY) returns the core to waiting for an invitation. ([specs/generic-midi.yaml:429](specs/generic-midi.yaml#L429))
- [ ] Confirm that an initiator that has joined the core's hosted session sends its data invitation from its control port plus one, or that the core's answering the port it came from suits it, and that it accepts the core's count 1 answers and sends count 2. ([specs/generic-midi.yaml:465](specs/generic-midi.yaml#L465))
- [ ] Capture Apple's RS packet and confirm the sequence number sits in the upper 16 bits of its 32-bit field, and that a host starting its own clock synchronisation accepts the core's count 1 answer. ([specs/generic-midi.yaml:465](specs/generic-midi.yaml#L465))
- [ ] Confirm 5004 is the port Audio MIDI Setup offers for a new session, and that the session's advertised _apple-midi._udp SRV port is its control port. ([specs/generic-midi.yaml:101](specs/generic-midi.yaml#L101))
- [ ] With a USB MIDI device on Windows, macOS and Linux, confirm the port listing's names open the device, that unplugging it reports disconnected within a second or two and plugging it back reconnects, and that a port held by another program on Windows fails to open as described. ([specs/generic-midi.yaml:444](specs/generic-midi.yaml#L444))
- [ ] On Linux, confirm the core's own ALSA sequencer calls (libasound.so.2 loaded at first use) list a USB MIDI device and a software port under the same names midir gave, send and receive notes and long SysEx both ways, and that on a machine without libasound.so.2, and on one without the sequencer (snd-seq not loaded), the core starts, lists no port and reports why a port device stays disconnected. ([specs/generic-midi.yaml:450](specs/generic-midi.yaml#L450))
- [ ] Run the MTC generator into a DAW or lighting console chasing MTC, over a MIDI port and over RTP-MIDI, at 24, 25, 30df and 30: confirm it locks at the time located, reads drop-frame across a minute (00:00:59:28 to 00:01:00:02), and stays within a frame of the core's clock for an hour. ([specs/generic-midi.yaml:457](specs/generic-midi.yaml#L457))
- [ ] Against an MMC device (a DAW or recorder), confirm READ answers for selected_time_code and motion_control_tally decode as kept, including short time code updates. ([specs/generic-midi.yaml:475](specs/generic-midi.yaml#L475))

## generic-modbus — Generic Modbus TCP server

- [ ] Check the word_order setting against devices that document 32-bit floats and integers in either register order (a power meter, a PLC), and that 0-based addressing matches devices that document 1-based 4xxxx and 3xxxx registers. ([specs/generic-modbus.yaml:220](specs/generic-modbus.yaml#L220))
- [ ] Check that servers answer a liveness read of a register they lack with an exception rather than silence, and that common Modbus TCP to RTU gateways answer for the unit IDs behind them without stalling the single outstanding request. ([specs/generic-modbus.yaml:245](specs/generic-modbus.yaml#L245))

## generic-mqtt — Generic MQTT broker client

- [ ] Connect to Mosquitto, EMQX and HiveMQ with versions 3.1.1 and 5 over tcp, tls and wss, and confirm CONNACK, SUBACK and version 5 properties (server keep alive, assigned client id, maximum QoS) are read as described, and that their WebSocket endpoints accept the mqtt subprotocol at the paths they document. ([specs/generic-mqtt.yaml:42](specs/generic-mqtt.yaml#L42))
- [ ] With clean_start false and a session expiry, confirm a broker delivers the messages queued while the client was away after reconnecting, and that dropping unacknowledged QoS 1 and 2 publishes (not resent with DUP) is acceptable to it. ([specs/generic-mqtt.yaml:153](specs/generic-mqtt.yaml#L153))

## generic-ntp — NTP server status (RFC 5905 client request)

- [ ] Check against a GPS time server (a TimeMachines TM2000A, a Meinberg) that the stratum, reference ID and offset agree with the server's own display. ([specs/generic-ntp.yaml:47](specs/generic-ntp.yaml#L47))

## generic-ping — Network presence (ping) and Wake-on-LAN

- [ ] Check that network adapters with a SecureOn password take it as 4 or 6 bytes right after the sixteen MAC repetitions (from practice, not AMD's paper). ([specs/generic-ping.yaml:66](specs/generic-ping.yaml#L66))
- [ ] Check on macOS that an unprivileged ICMP datagram socket keeps the identifier sent and prefixes IPv4 echo replies with their IP header, as the core allows for (only Windows was run here; Linux and macOS builds were compiled, not run). ([specs/generic-ping.yaml:120](specs/generic-ping.yaml#L120))
- [ ] Check on Linux with net.ipv4.ping_group_range including the core's group that echo replies arrive, and that outside it the socket is refused and auto falls back to TCP. ([specs/generic-ping.yaml:120](specs/generic-ping.yaml#L120))
- [ ] Check that typical show hosts (consoles, media servers, displays) answer a connection to a closed port with a reset rather than dropping it, which a TCP probe counts as reachable. ([specs/generic-ping.yaml:127](specs/generic-ping.yaml#L127))
- [ ] Check Wake-on-LAN through a router forwarding directed broadcasts to a subnet's broadcast address in wol_address. ([specs/generic-ping.yaml:62](specs/generic-ping.yaml#L62))

## generic-snmp — Generic SNMP agent

- [ ] Check AES-192 and AES-256 against agents that implement draft-blumenthal-aes-usm-04 (Net-SNMP's AES-192/AES-256), with SHA-1 and MD5 users whose keys are extended, and confirm which agents use the draft-reeder extension instead. ([specs/generic-snmp.yaml:255](specs/generic-snmp.yaml#L255))
- [ ] Check CBC-DES privacy against a real agent (only the core's own round trip and the RFC 3414 §8 salt layout are tested). ([specs/generic-snmp.yaml:91](specs/generic-snmp.yaml#L91))
- [ ] Check v3 informs from a real sender: that it discovers local_engine_id from the unknownEngineIDs report, then authenticates against this receiver's clock (boots 1, time from opening). ([specs/generic-snmp.yaml:115](specs/generic-snmp.yaml#L115))
- [ ] Check v3 traps from a real agent are accepted with keys localized to its engine, without the timeliness check. ([specs/generic-snmp.yaml:248](specs/generic-snmp.yaml#L248))
- [ ] Check a v1 agent's trap through the shared notification port, and the snmpTrapOID derived from it (RFC 3584 §3.1) against what Net-SNMP's snmptrapd shows. ([specs/generic-snmp.yaml:220](specs/generic-snmp.yaml#L220))
- [ ] Check how agents of PDUs, UPSs and switches answer a polling GET over v1 when one object is missing (noSuchName and the error index), and a GET too big for them (tooBig), which the core splits. ([specs/generic-snmp.yaml:105](specs/generic-snmp.yaml#L105))
- [ ] Check agents that encode unsigned values (Counter32, Gauge32, TimeTicks) without the leading zero byte their top bit needs; the core reads them as unsigned. ([specs/generic-snmp.yaml:241](specs/generic-snmp.yaml#L241))

## grandma2 — MA Lighting grandMA2

- [ ] Record exactly what the console sends over telnet after a failed Login, since a failed login cannot be detected now and every later command is silently discarded. ([specs/grandma2.yaml:1195](specs/grandma2.yaml#L1195))
- [ ] Check what the console greets with before login (it claims the guest user) and whether that text differs from a logged-in session. ([specs/grandma2.yaml:1192](specs/grandma2.yaml#L1192))
- [ ] Check whether Login "user" "" is accepted for a user without a password. ([specs/grandma2.yaml:1201](specs/grandma2.yaml#L1201))
- [ ] Confirm the username is case sensitive over telnet ("administrator" against "Administrator"). ([specs/grandma2.yaml:1198](specs/grandma2.yaml#L1198))
- [ ] Check whether At accepts a fractional percentage. ([specs/grandma2.yaml:1266](specs/grandma2.yaml#L1266))
- [ ] Check the upper bounds of pool numbers and fixture and channel IDs (9999 and 99999 assumed). ([specs/grandma2.yaml:1257](specs/grandma2.yaml#L1257))
- [ ] Check the units and ranges of Rate, Speed and MasterFade values. ([specs/grandma2.yaml:1268](specs/grandma2.yaml#L1268))
- [ ] Record the address grandMA2 sends MSC via Ethernet to (broadcast on the console's network, or a configured host), since the manual does not say. ([specs/grandma2.yaml:1632](specs/grandma2.yaml#L1632))
- [ ] Confirm that MSC Out with Mode Ethernet sends the Exec.Page and Exec Page data the MSC In examples show (exec.page after the cue, or exec and page separated by 00). ([specs/grandma2.yaml:1625](specs/grandma2.yaml#L1625))
- [ ] Confirm which events send MSC Out (Go, Timed_Go, Stop, Resume, Go_Off, Set, Fire) and that the length field counts the 12-byte header. ([specs/grandma2.yaml:1623](specs/grandma2.yaml#L1623))
- [ ] Check the fader position MSC Set sends at full (7F 7F, read as 99.99 %). ([specs/grandma2.yaml:1643](specs/grandma2.yaml#L1643))

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

## grassvalley-amp — Grass Valley AMP video servers

- [ ] Confirm a K2's replies over TCP: hexadecimal digits with no prefix, line end or checksum, one reply per command, and lower or upper case. ([specs/grassvalley-amp.yaml:332](specs/grassvalley-amp.yaml#L332))
- [ ] Confirm the channel request (CRAT0007204Vtr1) is answered 1001, a refused one 1111, and the channel-less form (CRAT00014). ([specs/grassvalley-amp.yaml:53](specs/grassvalley-amp.yaml#L53))
- [ ] Check the inferred layouts: erase_clip (A2.10), create_folder (A2.31), delete_folder (A2.29), get_clip_duration (A2.17 answered 84.17) and A2.xx single-name commands sent as the name's length then the name. ([specs/grassvalley-amp.yaml:324](specs/grassvalley-amp.yaml#L324))
- [ ] Check set_mute's data byte (21.62 with 1 on, 0 off). ([specs/grassvalley-amp.yaml:119](specs/grassvalley-amp.yaml#L119))
- [ ] Confirm List Next ID (A1.15 with a count of ten) answers 8X.14 with more names and 80.14 when none are left, and List First ID as a2140000. ([specs/grassvalley-amp.yaml:280](specs/grassvalley-amp.yaml#L280))
- [ ] Check the status bits a K2 sets (the names follow the T2's AMP manual) and whether 61.20 0F returns fifteen bytes. ([specs/grassvalley-amp.yaml:354](specs/grassvalley-amp.yaml#L354))
- [ ] Check which AMP commands Ross Tria, Mira and Kiva servers take over TCP 3811 and what their channels are called. ([specs/grassvalley-amp.yaml:87](specs/grassvalley-amp.yaml#L87))
- [ ] Check the times a K2 answers for 61.0C 01, 04, 08 and 40, and whether 02 (VITC) is answered or 70.0D. ([specs/grassvalley-amp.yaml:199](specs/grassvalley-amp.yaml#L199))

## grassvalley-kaleido — Grass Valley Kaleido multiviewers (Remote Control Protocol gateway)

- [ ] Confirm `<openID></openID>` opens a session without a room as `<openID/>` does, and that a session opened with a user takes the clear password. ([specs/grassvalley-kaleido.yaml:44](specs/grassvalley-kaleido.yaml#L44))
- [ ] Confirm a carriage return alone ends each command (the guide asks for a carriage return, and a line feed in scripts). ([specs/grassvalley-kaleido.yaml:33](specs/grassvalley-kaleido.yaml#L33))
- [ ] Record whether getKRoom's answer arrives on one line or several. ([specs/grassvalley-kaleido.yaml:112](specs/grassvalley-kaleido.yaml#L112))
- [ ] Confirm the -20 dB mode is written with a hyphen (the guide prints an en dash) in setKAudioOutMode and its answer. ([specs/grassvalley-kaleido.yaml:343](specs/grassvalley-kaleido.yaml#L343))
- [ ] Confirm getKAudioOutVolume's value format (whole dB, tenths on the Kaleido-X series, with or without a unit). ([specs/grassvalley-kaleido.yaml:426](specs/grassvalley-kaleido.yaml#L426))
- [ ] Confirm the gateway answers each command before reading the next on one connection, and that getKCurrentLayout closes it as the guide says. ([specs/grassvalley-kaleido.yaml:452](specs/grassvalley-kaleido.yaml#L452))

## greenhippo-hippotizer — Green Hippo Hippotizer

- [ ] Check whether mix, layer and timeline indexes start at 0 or 1 (mix and layer from 1, timeline by iD are assumed). ([specs/greenhippo-hippotizer.yaml:682](specs/greenhippo-hippotizer.yaml#L682))
- [ ] Check the single preset number of load_mix_preset and load_layer_preset (bank * 256 + slot per the Companion module). ([specs/greenhippo-hippotizer.yaml:680](specs/greenhippo-hippotizer.yaml#L680))
- [ ] Check the body of a level reply (a bare integer is assumed) and of the action endpoints. ([specs/greenhippo-hippotizer.yaml:240](specs/greenhippo-hippotizer.yaml#L240))
- [ ] Capture the JSON wrapper of GET /timelines, which the Swagger document leaves empty. ([specs/greenhippo-hippotizer.yaml:698](specs/greenhippo-hippotizer.yaml#L698))
- [ ] Check that the Web Callbacks websocket on 40513 accepts the subscription array sent on opening, with the path /, and whether it acknowledges it. ([specs/greenhippo-hippotizer.yaml:511](specs/greenhippo-hippotizer.yaml#L511))
- [ ] Check whether the presets-reset event is PRESETS_RESET or PRESES_RESET (the manual's example). ([specs/greenhippo-hippotizer.yaml:711](specs/greenhippo-hippotizer.yaml#L711))
- [ ] Check that DELETE /media/delete/{id}, DELETE /media/deletemapentry/{index} and PUT /media/addmapentry/... behave as their GET forms. ([specs/greenhippo-hippotizer.yaml:652](specs/greenhippo-hippotizer.yaml#L652))
- [ ] Check how a pin name containing an underscore is written in a REST pin path. ([specs/greenhippo-hippotizer.yaml:675](specs/greenhippo-hippotizer.yaml#L675))
- [ ] Check whether the media id and preset id path forms are told apart from map indexes and preset numbers by the segment being a number. ([specs/greenhippo-hippotizer.yaml:689](specs/greenhippo-hippotizer.yaml#L689))
- [ ] Capture the 4.9.x REST additions (media encoding settings, strata folder, user name and colour) from the API help served at port 40512. ([specs/greenhippo-hippotizer.yaml:57](specs/greenhippo-hippotizer.yaml#L57))
- [ ] Confirm /media/map lists every filled slot (and leaves out empty ones), and /presets/<type> every bank of the type: each read replaces media_map or that type's preset_banks. ([specs/greenhippo-hippotizer.yaml:563](specs/greenhippo-hippotizer.yaml#L563))
- [ ] Confirm POST /media/upload takes the multipart/form-data fields addToMap (true or false), mapIndex, customFolderPath and MediaFile as sent, with the file's type taken from its name, and answers 200 with mediaID. ([specs/greenhippo-hippotizer.yaml:418](specs/greenhippo-hippotizer.yaml#L418))

## h2r-graphics — H2R Graphics

- [ ] Record the HTTP status of a successful request (200 assumed) and of an error reply from v3.4 onward. ([specs/h2r-graphics.yaml:541](specs/h2r-graphics.yaml#L541))
- [ ] Confirm the text variable id form text.N in updateVariableText. ([specs/h2r-graphics.yaml:594](specs/h2r-graphics.yaml#L594))
- [ ] Check the update_score team and level ranges. ([specs/h2r-graphics.yaml:600](specs/h2r-graphics.yaml#L600))
- [ ] Check whether select_list_row counts rows from 1 or 0. ([specs/h2r-graphics.yaml:603](specs/h2r-graphics.yaml#L603))
- [ ] On version 2, check how an older version answers commands it predates. ([specs/h2r-graphics.yaml:582](specs/h2r-graphics.yaml#L582))
- [ ] Confirm H2R serves Socket.IO 4 (Engine.IO 4) on the HTTP API's port in versions 2 and 3, and that updateFrontend comes on connecting and after every change without a request. ([specs/h2r-graphics.yaml:552](specs/h2r-graphics.yaml#L552))
- [ ] Record the timer fields H2R sends (state values, duration, startedAt and endAt in milliseconds, endTime's format) and whether duration is a number or text. ([specs/h2r-graphics.yaml:563](specs/h2r-graphics.yaml#L563))
- [ ] Check that a list's selected row (dynamicLists, row 0 cell 0) counts as select_list_row does. ([specs/h2r-graphics.yaml:554](specs/h2r-graphics.yaml#L554))
- [ ] Confirm that a graphic with no status field is off air. ([specs/h2r-graphics.yaml:562](specs/h2r-graphics.yaml#L562))
- [ ] Confirm that each project in updateFrontend carries its name in a name field (assumed; the Companion module does not read it). ([specs/h2r-graphics.yaml:488](specs/h2r-graphics.yaml#L488))
- [ ] Confirm a score graphic's scores are an array per team of {score} per level, with levels and delimiter on the graphic, as the Companion module reads them, and whether a score can be other than a whole number. ([specs/h2r-graphics.yaml:483](specs/h2r-graphics.yaml#L483))

## haivision-kb — Haivision KB Encoder/Transcoder

- [ ] Check that the login answers {"sessionid": ...} and that later requests take it in the Authorization header as it is (no scheme word). ([specs/haivision-kb.yaml:74](specs/haivision-kb.yaml#L74))
- [ ] Check how a request on an expired session (30 minutes idle) or after a logout is answered (status and body); 401 or 403 is taken as the session's end, and 403 as a refused login. ([specs/haivision-kb.yaml:75](specs/haivision-kb.yaml#L75))
- [ ] Check that software before 4.3.1 needs no login, and how 4.3.1 or later answers a request without one (the probe, GET /ecs/version.json, goes without the session). ([specs/haivision-kb.yaml:71](specs/haivision-kb.yaml#L71))
- [ ] Check whether the channel part of /ecs/channels/{channel}.json takes the channel's id, its label, or both (the document calls it the label; its lists and examples use the id). ([specs/haivision-kb.yaml:165](specs/haivision-kb.yaml#L165))
- [ ] Check that create_channel accepts an empty id (to have the encoder choose one) and the status a successful create answers. ([specs/haivision-kb.yaml:152](specs/haivision-kb.yaml#L152))
- [ ] Check the signal list's query form (index=-1, time=HH:MM:SS:mmm, timespan=60, without the quotes the document's syntax line shows) and the reply's top-level shape. ([specs/haivision-kb.yaml:309](specs/haivision-kb.yaml#L309))
- [ ] Check that ad cue durations and offsets with three decimals are accepted, and whether repeat takes true/false (the document's text) or no/yes (its example). ([specs/haivision-kb.yaml:348](specs/haivision-kb.yaml#L348))
- [ ] Check that reading each channel listed by GET /ecs.json every 2 s is light enough for the ECS with many channels running. ([specs/haivision-kb.yaml:974](specs/haivision-kb.yaml#L974))

## haivision-makito-x4 — Haivision Makito X4 encoder and decoder

- [ ] Check that the encoder's login sets the SessionID cookie, as the decoder's reference shows, and that the login takes a JSON body of username and password. ([specs/haivision-makito-x4.yaml:101](specs/haivision-makito-x4.yaml#L101))
- [ ] Check that an expired session (auto-logout policy) is answered 401 on any request, and that a refused login is answered 401. ([specs/haivision-makito-x4.yaml:102](specs/haivision-makito-x4.yaml#L102))
- [ ] Check that GET /apis/authentication without a session answers (401) promptly, as the liveness probe relies on any answer. ([specs/haivision-makito-x4.yaml:95](specs/haivision-makito-x4.yaml#L95))
- [ ] Check that PUT /apis/system_info with only deviceName (or only analytics, or only personality) keeps the other fields. ([specs/haivision-makito-x4.yaml:240](specs/haivision-makito-x4.yaml#L240))
- [ ] Check that PUT /apis/datetime without "server" (manual time) keeps the NTP server, and that the PTP fields alone are accepted. ([specs/haivision-makito-x4.yaml:742](specs/haivision-makito-x4.yaml#L742))
- [ ] Check that PUT /apis/datetime with "ntp":1/0 is what the decoder wants and true/false what the encoder wants (the two references differ). ([specs/haivision-makito-x4.yaml:742](specs/haivision-makito-x4.yaml#L742))
- [ ] Check that PUT /apis/datetime takes the five PTP fields alone (set_ptp), and what announcementTimeout range is allowed. ([specs/haivision-makito-x4.yaml:756](specs/haivision-makito-x4.yaml#L756))
- [ ] Check that PUT /apis/comport with only baudRate is accepted in pass-through mode (rugged encoder). ([specs/haivision-makito-x4.yaml:946](specs/haivision-makito-x4.yaml#L946))
- [ ] Check that PUT /apis/videnc/{id} with only some fields (such as bitrate) keeps the others. ([specs/haivision-makito-x4.yaml:1251](specs/haivision-makito-x4.yaml#L1251))
- [ ] Check that PUT /apis/metadata/{id}/start and the audio and video encoder start requests work without a body. ([specs/haivision-makito-x4.yaml:1399](specs/haivision-makito-x4.yaml#L1399))
- [ ] Check that the role is spelled Administrator when creating an account (the reference's field description spells Administator). ([specs/haivision-makito-x4.yaml:331](specs/haivision-makito-x4.yaml#L331))
- [ ] Check that PUT /apis/services with one service ({"onvif":1}) or one nic entry changes only that service, and that preview and passthrough take the same form. ([specs/haivision-makito-x4.yaml:824](specs/haivision-makito-x4.yaml#L824))
- [ ] Check the recordings API (1.10): POST and PUT bodies with limitSize and segmentSize as text, segmentType "0" for none, and whether the recording id is the stream's id. ([specs/haivision-makito-x4.yaml:1164](specs/haivision-makito-x4.yaml#L1164))
- [ ] Check that PUT /apis/decoders/{id} with streamId alone keeps the other decoder settings, and the reply of a decoder PUT. ([specs/haivision-makito-x4.yaml:1778](specs/haivision-makito-x4.yaml#L1778))
- [ ] Check that PUT /apis/st2110/{id} takes name, tos and ttl beside address and port, and packetTime for an audio essence. ([specs/haivision-makito-x4.yaml:1843](specs/haivision-makito-x4.yaml#L1843))
- [ ] Check the certificate upload's form fields: the file part's filename (the certificate's name here, the reference's example uses the original file name) and an empty password field. ([specs/haivision-makito-x4.yaml:508](specs/haivision-makito-x4.yaml#L508))
- [ ] Check that PUT /apis/logosettings/{id} without if_name, url and panelColor (which the web interface sends) is accepted. ([specs/haivision-makito-x4.yaml:1744](specs/haivision-makito-x4.yaml#L1744))
- [ ] Check what the audio encoder accepts for sampleRate 16 on non-analog inputs and the bit rate ranges per mode (the user's guide table). ([specs/haivision-makito-x4.yaml:1324](specs/haivision-makito-x4.yaml#L1324))
- [ ] Check what GET /apis/essences and GET /apis/logos answer outside the 2110 and DEFENSE personalities, and GET /apis/comport on a non-rugged encoder (the polls ask regardless). ([specs/haivision-makito-x4.yaml:3964](specs/haivision-makito-x4.yaml#L3964))
- [ ] Check that a rugged encoder answers the comport endpoints and that the plain encoder does not, as the model split assumes. ([specs/haivision-makito-x4.yaml:930](specs/haivision-makito-x4.yaml#L930))
- [ ] Check that POST /apis/leds takes duration as a number (the reference's example sends it as text). ([specs/haivision-makito-x4.yaml:306](specs/haivision-makito-x4.yaml#L306))
- [ ] Check how long an upgrade package upload with verification (POST /apis/upgrade) takes on a unit; the transport waits 20 s for an answer. ([specs/haivision-makito-x4.yaml:93](specs/haivision-makito-x4.yaml#L93))
- [ ] Confirm the id in /apis/preview/{id} is the video input's id as list_video_inputs gives it, and that the snapshot, preview and logo downloads answer the image itself with 200. ([specs/haivision-makito-x4.yaml:1685](specs/haivision-makito-x4.yaml#L1685))

## highend-hog4 — High End Systems Hog 4 / Hog OS

- [ ] Confirm that playback commands take the object number as the argument (/hog/playback/go/0 with 15), as the manual says, and not in the address as the Companion module sends it. ([specs/highend-hog4.yaml:522](specs/highend-hog4.yaml#L522))
- [ ] Check how goto_cue's float list.cue is read (15.2 sent as 15.19999981; cue 2 versus 20; point cues). ([specs/highend-hog4.yaml:531](specs/highend-hog4.yaml#L531))
- [ ] Record the argument types of the status outputs (LED value, LED colour, command line, encoder and H-key labels, chat lines). ([specs/highend-hog4.yaml:512](specs/highend-hog4.yaml#L512))
- [ ] Find the address the console uses for fader level status after consolefaderrefresh. ([specs/highend-hog4.yaml:506](specs/highend-hog4.yaml#L506))
- [ ] Check the real ranges of H keys, U keys, encoder wheels and the trackball values per model. ([specs/highend-hog4.yaml:539](specs/highend-hog4.yaml#L539))
- [ ] Check which keys of the Hog OS 5 table work on Hog 4 OS 3.x and 4.x. ([specs/highend-hog4.yaml:549](specs/highend-hog4.yaml#L549))
- [ ] Confirm that refreshall makes the console send every status, and whether repeating it every 60 s is noticeable on the console. ([specs/highend-hog4.yaml:504](specs/highend-hog4.yaml#L504))
- [ ] Check whether the HPU acts on front panel keys, encoders and faders sent over OSC. ([specs/highend-hog4.yaml:571](specs/highend-hog4.yaml#L571))
- [ ] Confirm that Hog 4 OS sends MSC on MSC Out for Go, Halt, Resume, Skip and Release, as 21.1's table of commands it recognises and sends suggests. ([specs/highend-hog4.yaml:499](specs/highend-hog4.yaml#L499))

## holographics — Holographics

- [ ] Confirm the REST API takes a call's data as the whole JSON body (as the vendor SDK sends it), not wrapped as {id, data}. ([specs/holographics.yaml:534](specs/holographics.yaml#L534))
- [ ] Check that list_entries' widgetId filter is honoured when sent as the body of a GET (or whether it must be a query parameter, ?widgetId=). ([specs/holographics.yaml:293](specs/holographics.yaml#L293))
- [ ] Confirm the Socket.IO server speaks Engine.IO 3 (Socket.IO 2) and pushes stateChanged with the whole state on every change. ([specs/holographics.yaml:421](specs/holographics.yaml#L421))
- [ ] Record whether an entry in the state carries widgetId or widget_id. ([specs/holographics.yaml:528](specs/holographics.yaml#L528))
- [ ] Record the reply of GET /api/update/ and /api/license/ so they can be kept in state. ([specs/holographics.yaml:385](specs/holographics.yaml#L385))

## holyrics — Holyrics

- [ ] Confirm the API Server's default port (8091 is taken from the GetAPIServerInfo example; the documentation names no default). ([specs/holyrics.yaml:31](specs/holyrics.yaml#L31))
- [ ] Record the HTTP status and body Holyrics answers an invalid or revoked token with ({"status":"error","error":"invalid token"} is documented, its status not; 401 is assumed for the refusal), and confirm CheckPermissions' 401 carries an error object instead. ([specs/holyrics.yaml:35](specs/holyrics.yaml#L35))
- [ ] Confirm actions documented as returning no value answer {"status":"ok"} with HTTP 200, and that an action the token lacks answers status error rather than 401. ([specs/holyrics.yaml:146](specs/holyrics.yaml#L146))
- [ ] Confirm GetCurrentPresentation answers data null while nothing is presented (the case that empties presentation state), and that GetHue answers data null while the hue is off. ([specs/holyrics.yaml:1537](specs/holyrics.yaml#L1537))
- [ ] Check which response Numbers can be fractional: time_ms, duration_ms, volume, slide_number and total_slides are read as integers, countdown_time, stopwatch_time, BPM and the others as decimals. ([specs/holyrics.yaml:1427](specs/holyrics.yaml#L1427))
- [ ] Confirm SetHue with hue null turns the hue off. ([specs/holyrics.yaml:1409](specs/holyrics.yaml#L1409))

## imimot-mitti — Mitti

- [ ] Check that a Custom OSC Feedback target (this host and the feedback_port setting) receives every feedback message, and which port Mitti suggests by default. ([specs/imimot-mitti.yaml:617](specs/imimot-mitti.yaml#L617))
- [ ] Confirm that /mitti/resendOSCFeedback sends every feedback value again, including each cue's properties, and that changes made over OSC are fed back too. ([specs/imimot-mitti.yaml:626](specs/imimot-mitti.yaml#L626))
- [ ] Compare the command names taken from the Companion module (stop, panic, jumpToPrevCue, playCueWithCueID, playCueAtIndex, the ForceCut forms, fullscreenOn, loopOn, transitionOnPlayOn, autoFade, mainFader, playhead, playheadScrub, locateCurrentCue, videoOutputsOn, unmuteAudio, setInFromPlayhead, the cue-level On/Off/toggle forms, setGotoToCueID and the AsPercent/AsPixels/AsDegrees/AsDecibels properties) with the address space Mitti's OSC Query server lists at http://<host>:51000. ([specs/imimot-mitti.yaml:635](specs/imimot-mitti.yaml#L635))
- [ ] Check the feedback addresses read into state that only the Companion module names: currentCueID, selectedCueName, selectedCueID, playhead, mainFader, toggleVideoOutputs, toggleAudio, toggleFullscreen, toggleLoop, toggleTransitionOnPlay, in/outFromPlayheadEnabled, and the per-cue cueName, deleted and property messages (and whether they use the AsPercent names). ([specs/imimot-mitti.yaml:635](specs/imimot-mitti.yaml#L635))
- [ ] Check the argument types Mitti accepts: values are sent as OSC strings ("75", "-3.5", "0.50") as the Companion module does; check whether floats or ints are what the OSC Query server advertises, and the types Mitti's feedback carries. ([specs/imimot-mitti.yaml:647](specs/imimot-mitti.yaml#L647))
- [ ] Check whether cue-level feedback keys a cue by its number or its ID, and the value range of the documented /mitti/<cue>/opacity. ([specs/imimot-mitti.yaml:654](specs/imimot-mitti.yaml#L654))
- [ ] Confirm the UDP string form of the no-value commands, and that OSC control is ignored while Mitti follows MTC or LTC. ([specs/imimot-mitti.yaml:664](specs/imimot-mitti.yaml#L664))

## integ-jnior — INTEG JNIOR automation controllers

- [ ] Check the WebSocket transport's path: the manual describes the interface on the web server's port but not the path the WebUI connects to; / is assumed. ([specs/integ-jnior.yaml:44](specs/integ-jnior.yaml#L44))
- [ ] Confirm every response the integration waits for (Registry, File, Console, Enumerate and Read Devices, Clock) echoes the request's Meta, as the manual says of responses in general; a response without it would time out the command. ([specs/integ-jnior.yaml:7](specs/integ-jnior.yaml#L7))
- [ ] Check on a unit with JMPServer/Login disabled that a Monitor or Authenticated message arrives without a challenge, which the integration takes as logged in. ([specs/integ-jnior.yaml:254](specs/integ-jnior.yaml#L254))

## ioversal-vertex — ioversal VERTEX

- [ ] Confirm the script server answers nothing to commands without return (a set, a Play), so nothing is taken for a later query's answer. ([specs/ioversal-vertex.yaml:307](specs/ioversal-vertex.yaml#L307))
- [ ] Confirm what Authenticate:username,password answers, and what an unauthenticated command gets when authentication is on. ([specs/ioversal-vertex.yaml:66](specs/ioversal-vertex.yaml#L66))
- [ ] Confirm FadeToCue's arguments are the cue, a preload time and a fade time in seconds, and that GotoFrame takes a frame number. ([specs/ioversal-vertex.yaml:181](specs/ioversal-vertex.yaml#L181))
- [ ] Check the TimeCode value's JSON field order (FPS before TotalFrames) on current versions, and that ListItems.Playback lists every playback by name. ([specs/ioversal-vertex.yaml:350](specs/ioversal-vertex.yaml#L350))

## irisdown-countdown-timer — Irisdown Countdown Timer

- [ ] Confirm that UPDATEMODE and UPDATES apply per connection and that UPDATEMODE 2 answers OK before UPDATES ON turns other answers off. ([specs/irisdown-countdown-timer.yaml:180](specs/irisdown-countdown-timer.yaml#L180))
- [ ] Record the mode 2 update line in overtime and past 99 minutes (the sign, the hour digits), and how often updates come while running. ([specs/irisdown-countdown-timer.yaml:132](specs/irisdown-countdown-timer.yaml#L132))
- [ ] Check whether a command line ended with CRLF is taken once (the document allows CR, LF or CRLF). ([specs/irisdown-countdown-timer.yaml:46](specs/irisdown-countdown-timer.yaml#L46))
- [ ] Check how MESSAGE treats a quote or a character outside ASCII in the text. ([specs/irisdown-countdown-timer.yaml:187](specs/irisdown-countdown-timer.yaml#L187))
- [ ] Confirm JOG takes negative minutes while stopped or paused as well as while running. ([specs/irisdown-countdown-timer.yaml:98](specs/irisdown-countdown-timer.yaml#L98))

## irisdown-remote-show-control — Irisdown Remote Show Control

- [ ] Confirm the add-in takes each command as a single UDP datagram with no line ending, and plain ASCII double quotes around names. ([specs/irisdown-remote-show-control.yaml:156](specs/irisdown-remote-show-control.yaml#L156))
- [ ] Record how FIND answers (over UDP and over TCP 61001) and how the end of the list is marked. ([specs/irisdown-remote-show-control.yaml:154](specs/irisdown-remote-show-control.yaml#L154))
- [ ] Check whether GO takes a slide number with the space (GO 23), as the manual's syntax shows, as well as without (GO2 in the Spyder example). ([specs/irisdown-remote-show-control.yaml:104](specs/irisdown-remote-show-control.yaml#L104))
- [ ] Record how FIND's answer arrives over UDP (one datagram back to the sender's port holding every line, as assumed, or one per line) and its line ending. ([specs/irisdown-remote-show-control.yaml:147](specs/irisdown-remote-show-control.yaml#L147))

## joy-playdeck — PLAYDECK

- [ ] Confirm a running PLAYDECK answers the REST API on 11411 as the OpenAPI document describes (the document says the implementation may follow the contract later), and which PLAYDECK version first serves it. ([specs/joy-playdeck.yaml:68](specs/joy-playdeck.yaml#L68))
- [ ] Confirm control routes answer 200 with {"ok": true} (ActionResponse), and what a channel outside the licence answers (404 with an ErrorResponse is documented). ([specs/joy-playdeck.yaml:120](specs/joy-playdeck.yaml#L120))
- [ ] Confirm the units of the status numbers: BlockPosition, BlockDuration, BlockRemain, ClipPosition, ClipDuration, ClipRemain, ScheduleRemain and AutoplayRemain in seconds, BlockProgress and ClipProgress in percent (0 to 100), and BlockEnd, ClipEnd, ScheduleTime and AutoplayTime in Unix seconds (taken from the Companion module, the contract giving none). ([specs/joy-playdeck.yaml:1416](specs/joy-playdeck.yaml#L1416))
- [ ] Record the value of OverlayActiveN and ActionActiveN while a button is active (kept as text as it arrives), and confirm the keys are absent while it is not. ([specs/joy-playdeck.yaml:801](specs/joy-playdeck.yaml#L801))
- [ ] Confirm a position request with a marker and an empty timestamp seeks to the marker (the schema requires timestamp, so an empty one is sent beside the marker). ([specs/joy-playdeck.yaml:378](specs/joy-playdeck.yaml#L378))
- [ ] Confirm that blocks and clips in the project are listed in playlist order, so their place from 1 is the block and clip number the cue, play and select routes take. ([specs/joy-playdeck.yaml:1253](specs/joy-playdeck.yaml#L1253))
- [ ] Confirm POST /api/v1/commands accepts args given as strings for numeric arguments (the schema says numbers as strings are fine) and what it answers an unknown command with. ([specs/joy-playdeck.yaml:500](specs/joy-playdeck.yaml#L500))

## jvc-camera — JVC camcorders and PTZ cameras (Camcorder Web API)

- [ ] Check the login exchange: whether /api.php answers 302 or 200 with the SessionID cookie, how many cookies it sets, and whether api.cgi needs the cookie, the body's SessionID or both. ([specs/jvc-camera.yaml:53](specs/jvc-camera.yaml#L53))
- [ ] Check whether a second API client is refused while one holds a session, and what a session left idle does (the document says SessionRenewal is no longer needed). ([specs/jvc-camera.yaml:1351](specs/jvc-camera.yaml#L1351))
- [ ] Record a full GetCamStatus answer on a camcorder and a KY-PZ camera, to check the Enable and ButtonString members the rules read and the field names the examples spell differently (AspectRetio, Sw, Agc). ([specs/jvc-camera.yaml:1109](specs/jvc-camera.yaml#L1109))
- [ ] Check how the camera answers text with a space (an alias or a name), given the document's rule against spaces in requests. ([specs/jvc-camera.yaml:1366](specs/jvc-camera.yaml#L1366))
- [ ] Confirm GET /cgi-bin/get_jpg.cgi answers the JPEG still with no query, carrying the session cookie, once JpegEncode has started. ([specs/jvc-camera.yaml:825](specs/jvc-camera.yaml#L825))

## kiloview-avx — Kiloview AVX24-4 Media HUB

- [ ] Confirm that the token from login (data.token) is accepted as "Authorization: Bearer <token>". The document never says how a request carries the token. ([specs/kiloview-avx.yaml:41](specs/kiloview-avx.yaml#L41))
- [ ] Check that a login token lasts about an hour (the example JWT's exp is 3660 s after its nbf), and that an expired or wrong token is answered HTTP 401, on which the session logs in again. ([specs/kiloview-avx.yaml:47](specs/kiloview-avx.yaml#L47))
- [ ] Check that there is no logout or session-check request, although the introduction mentions both. ([specs/kiloview-avx.yaml:1980](specs/kiloview-avx.yaml#L1980))
- [ ] Confirm that the web API answers on HTTP port 80. The document gives no port. ([specs/kiloview-avx.yaml:65](specs/kiloview-avx.yaml#L65))
- [ ] Confirm that GET requests whose parameters the document writes as a JSON object take them as query parameters. ([specs/kiloview-avx.yaml:843](specs/kiloview-avx.yaml#L843))
- [ ] Confirm that failures come back as HTTP 200 with a code other than 200 in the {code, message, data} body. ([specs/kiloview-avx.yaml:309](specs/kiloview-avx.yaml#L309))
- [ ] Confirm that Set System Time (timetype ntp, manual or pc) goes to /v1/systime/setLocation, the same path as Set Timezone. ([specs/kiloview-avx.yaml:404](specs/kiloview-avx.yaml#L404))
- [ ] Confirm that modify_user takes id as a number (as the example writes it; the table says string) and needs the password every time. ([specs/kiloview-avx.yaml:325](specs/kiloview-avx.yaml#L325))
- [ ] Confirm that modify_vlan_profile takes vlan_ids as a string, as the example writes it (the table says array). ([specs/kiloview-avx.yaml:1261](specs/kiloview-avx.yaml#L1261))
- [ ] Confirm that set_aggregation accepts lacp_passive. The example uses it, but the table lists only static and lacp_active. ([specs/kiloview-avx.yaml:1588](specs/kiloview-avx.yaml#L1588))
- [ ] Confirm that set_static_binding's bind_type is numbered as in the request table (1 port, 2 MAC, 3 unbound); the binding lists number it differently. ([specs/kiloview-avx.yaml:1723](specs/kiloview-avx.yaml#L1723))
- [ ] Check that the batch port request (12.2.4) accepts a single field and leaves the others unchanged. ([specs/kiloview-avx.yaml:1455](specs/kiloview-avx.yaml#L1455))
- [ ] Check that add_vlan with only id, alias and color is accepted. ([specs/kiloview-avx.yaml:1600](specs/kiloview-avx.yaml#L1600))
- [ ] Check that get_dhcp_clients with an empty vlan_id list returns every VLAN's clients. ([specs/kiloview-avx.yaml:1661](specs/kiloview-avx.yaml#L1661))
- [ ] Check that get_static_bindings with an empty body returns every binding. ([specs/kiloview-avx.yaml:1711](specs/kiloview-avx.yaml#L1711))
- [ ] Confirm that upload_icon takes multipart/form-data. The document types icon_file as a file but writes its example as JSON. ([specs/kiloview-avx.yaml:1169](specs/kiloview-avx.yaml#L1169))
- [ ] Find out what modify_icons (ids only) changes. ([specs/kiloview-avx.yaml:1190](specs/kiloview-avx.yaml#L1190))
- [ ] Find out the list key in the panel output configuration reply: cfg_outputs (the table) or outputs (the example). Both are read. ([specs/kiloview-avx.yaml:3138](specs/kiloview-avx.yaml#L3138))
- [ ] Check that the topology status reply has no code field, and whether it names its links edges (the example) or wires (the table). ([specs/kiloview-avx.yaml:1276](specs/kiloview-avx.yaml#L1276))
- [ ] Check whether the watched-ports reply comes bare or wrapped in {code, message, data}. Both are read. ([specs/kiloview-avx.yaml:1523](specs/kiloview-avx.yaml#L1523))
- [ ] Confirm that the time offset is in hours. ([specs/kiloview-avx.yaml:2135](specs/kiloview-avx.yaml#L2135))
- [ ] Confirm that the switch and system temperatures are in degrees Celsius. ([specs/kiloview-avx.yaml:2457](specs/kiloview-avx.yaml#L2457))
- [ ] Confirm the reading of the misaligned PTP columns: de_mainclock, average_delay, upstream_equipment and upstream_port. ([specs/kiloview-avx.yaml:2149](specs/kiloview-avx.yaml#L2149))
- [ ] Find out whether a preset's apply flag means "available" or "applied". ([specs/kiloview-avx.yaml:2158](specs/kiloview-avx.yaml#L2158))
- [ ] Find the units of PoE power, voltage and current. ([specs/kiloview-avx.yaml:2495](specs/kiloview-avx.yaml#L2495))
- [ ] Find out what the network status flag means (12.8.2). ([specs/kiloview-avx.yaml:2589](specs/kiloview-avx.yaml#L2589))
- [ ] Confirm that a panel output's input_id 0 means no input. ([specs/kiloview-avx.yaml:2256](specs/kiloview-avx.yaml#L2256))
- [ ] Confirm that a panel's other_inputs are all the inputs not in it. ([specs/kiloview-avx.yaml:3063](specs/kiloview-avx.yaml#L3063))
- [ ] Check that set_dhcp with gateway type default accepts an empty gateway ip. ([specs/kiloview-avx.yaml:1680](specs/kiloview-avx.yaml#L1680))
- [ ] Check that get_alarms with alarm_levels=1&2&3 returns every alarm event. ([specs/kiloview-avx.yaml:1797](specs/kiloview-avx.yaml#L1797))
- [ ] Check that enabling a panel on the physical front panel changes the front panel list. ([specs/kiloview-avx.yaml:4113](specs/kiloview-avx.yaml#L4113))
- [ ] Find out the virtual NIC interface type values. The example shows static; the VLAN list shows dhcp and manual. ([specs/kiloview-avx.yaml:531](specs/kiloview-avx.yaml#L531))

## kiloview-cube-r1 — Kiloview CUBE R1

- [ ] Find out how the CUBE R1 takes the login token back (the API document does not say): confirm that the token sent as it is in Authorization is accepted, or that the cookie user=<user>; token=<token> is, and which one the web interface uses. ([specs/kiloview-cube-r1.yaml:52](specs/kiloview-cube-r1.yaml#L52))
- [ ] Check that a recorder without a login takes requests without the cookie, which is left out while the password is empty. ([specs/kiloview-cube-r1.yaml:54](specs/kiloview-cube-r1.yaml#L54))
- [ ] Check which requests need the token at all, how long a token lasts, that a refused one is answered with HTTP 200 and {"msg":"Token.Error","result":"error"} on every request (the session then logs in again), and that a wrong password is answered with a msg naming the password (the terminal refusal). ([specs/kiloview-cube-r1.yaml:62](specs/kiloview-cube-r1.yaml#L62))
- [ ] Confirm that users/login.json takes a JSON body (the document shows one but no Content-Type) and that the token is at data.token. ([specs/kiloview-cube-r1.yaml:159](specs/kiloview-cube-r1.yaml#L159))
- [ ] Confirm the parameter names of users/modify.json (oldpassword, username, password) and that a non-admin user can change only its own password. ([specs/kiloview-cube-r1.yaml:176](specs/kiloview-cube-r1.yaml#L176))
- [ ] Confirm that source/get_param.json takes the group as the query parameter disc_id (the document gives no parameter), and that its answer carries disc_id. ([specs/kiloview-cube-r1.yaml:234](specs/kiloview-cube-r1.yaml#L234))
- [ ] Check that the source list rules read 3.1's answer as documented (data[].disc_id, disc_name, sources[].id, ip, name) and that every group's settings are then read without errors. ([specs/kiloview-cube-r1.yaml:864](specs/kiloview-cube-r1.yaml#L864))
- [ ] Confirm that output/setSource.json takes the source's name in stream_id (typed as a number 1-20 in the document), and what stream_id names for type display (the swap of two windows). ([specs/kiloview-cube-r1.yaml:318](specs/kiloview-cube-r1.yaml#L318))
- [ ] Confirm that output/setName.json takes name (the document's parameter table lists stream_id) and answers {"result":"ok"}. ([specs/kiloview-cube-r1.yaml:332](specs/kiloview-cube-r1.yaml#L332))
- [ ] Confirm that output/window_audio_set.json takes volume as "on"/"off" text rather than a boolean. ([specs/kiloview-cube-r1.yaml:387](specs/kiloview-cube-r1.yaml#L387))
- [ ] Confirm that layout/setLayout.json switches the layout and that output/get.json then reports the new layout_id and layout_number. ([specs/kiloview-cube-r1.yaml:299](specs/kiloview-cube-r1.yaml#L299))
- [ ] Confirm that storage/setStoLimit.json takes choose as a string ("1") and not only as the number its example sends, and which values limitType takes (size, time, default). ([specs/kiloview-cube-r1.yaml:406](specs/kiloview-cube-r1.yaml#L406))
- [ ] Check the units of the storage information's rate and speed, and whether total and used always carry a unit suffix (900G, 30M). ([specs/kiloview-cube-r1.yaml:940](specs/kiloview-cube-r1.yaml#L940))
- [ ] Confirm that record/getRecStatus.json and record/getinfo.json take project_id in the query, and that setRecStatus.json acts on the whole project without one. ([specs/kiloview-cube-r1.yaml:423](specs/kiloview-cube-r1.yaml#L423))
- [ ] Find out what "ready state" record/setRecStatus.json requires before recording starts, and how a start refused for it is answered. ([specs/kiloview-cube-r1.yaml:429](specs/kiloview-cube-r1.yaml#L429))
- [ ] Confirm that record/setinfo.json takes all six fields (solution as a number 0-2, times as YYYY-MM-DD HH:MM:SS) and in which time zone the scheduled times are read. ([specs/kiloview-cube-r1.yaml:447](specs/kiloview-cube-r1.yaml#L447))
- [ ] Check the units of performance net_up and net_down (KB per second?) and whether cpu and gpu are decimals. ([specs/kiloview-cube-r1.yaml:1017](specs/kiloview-cube-r1.yaml#L1017))
- [ ] Check what the ip of webrtc/get_sdp.json must be (the recorder's own address, or the client's), and whether the session can be completed by any request the document leaves out. ([specs/kiloview-cube-r1.yaml:475](specs/kiloview-cube-r1.yaml#L475))
- [ ] Confirm the playback list's channelName is the channel name the storage information lists on the disk, and what a missing disk or channel answers. ([specs/kiloview-cube-r1.yaml:494](specs/kiloview-cube-r1.yaml#L494))
- [ ] Confirm that playback/remove.json deletes the file from the disk at once, with no recycle bin. ([specs/kiloview-cube-r1.yaml:519](specs/kiloview-cube-r1.yaml#L519))
- [ ] Confirm the methods of the system and network requests (the document gives none): GET for getHostname, getTimeZone, getTimer and getNet, POST with a JSON body for setHostname, setTimeZone, setTimer and modifyNet. ([specs/kiloview-cube-r1.yaml:535](specs/kiloview-cube-r1.yaml#L535))
- [ ] Confirm the form of setTimeZone's offset (a string of whole hours, or with minutes for zones such as Asia/Kolkata) and that the time zone list is IANA names. ([specs/kiloview-cube-r1.yaml:550](specs/kiloview-cube-r1.yaml#L550))
- [ ] Confirm that setTimer.json takes ip with timeType dev as well, and that getTimer's time is milliseconds since 1970 (the document's example ends in a space). ([specs/kiloview-cube-r1.yaml:1052](specs/kiloview-cube-r1.yaml#L1052))
- [ ] Confirm the path of the one-click time synchronization (API URL system/timer/apply.json, example ntp/apply.json) and that it takes {"project_id":1}. ([specs/kiloview-cube-r1.yaml:580](specs/kiloview-cube-r1.yaml#L580))
- [ ] Confirm that network/modifyNet.json takes every field with dynamic y/n, applies the change at once, and whether state down disables the interface. ([specs/kiloview-cube-r1.yaml:594](specs/kiloview-cube-r1.yaml#L594))
- [ ] Find the websocket's path on port 12346 (the spec opens /), whether it needs the token, and that it pushes {"update":"record"} and {"socketType":"storePathUpdate"} as documented. ([specs/kiloview-cube-r1.yaml:813](specs/kiloview-cube-r1.yaml#L813))
- [ ] Check whether the API has a reboot, shutdown, disk format or firmware upgrade request the document leaves out (its contents name a 13.2 Update software version). ([specs/kiloview-cube-r1.yaml:975](specs/kiloview-cube-r1.yaml#L975))

## kiloview-cube-x1 — Kiloview CUBE X1

- [ ] Record how the server answers an expired or invalid bearer token: 401 or 403 is assumed, on which the session logs in again. ([specs/kiloview-ndi-core.yaml:64](specs/kiloview-ndi-core.yaml#L64))
- [ ] Confirm that get_long_term_token (user/token/permanent) works without an Authorization header, as the document's example sends it, and that the token it returns without minutes never expires. ([specs/kiloview-cube-x1.yaml:205](specs/kiloview-cube-x1.yaml#L205))
- [ ] Confirm that get_timed_token's minutes sets the token's lifetime in minutes. ([specs/kiloview-cube-x1.yaml:217](specs/kiloview-cube-x1.yaml#L217))
- [ ] Check how login answers a wrong user name or password: HTTP 200 with the error envelope (the document's error example, taken by the session as the terminal refusal), or a 401. ([specs/kiloview-ndi-core.yaml:65](specs/kiloview-ndi-core.yaml#L65))
- [ ] Confirm that errors come with HTTP 200 and {"result": "error", ...}, and list the error codes seen. ([specs/kiloview-cube-x1.yaml:739](specs/kiloview-cube-x1.yaml#L739))
- [ ] Confirm that system/info answers without a token, since the probe relies on it. ([specs/kiloview-cube-x1.yaml:57](specs/kiloview-cube-x1.yaml#L57))
- [ ] Check that the language header is accepted as en on every request, and what the server does without it. ([specs/kiloview-cube-x1.yaml:55](specs/kiloview-cube-x1.yaml#L55))
- [ ] Check whether HTTPS uses a self-signed certificate on port 443 by default. ([specs/kiloview-cube-x1.yaml:51](specs/kiloview-cube-x1.yaml#L51))
- [ ] Record the event websocket's messages (ws://<host>/api_event/cube/ws/info?token=...), which the document does not describe, so they can replace polling. ([specs/kiloview-cube-x1.yaml:747](specs/kiloview-cube-x1.yaml#L747))
- [ ] Check whether manage_panel_inputs removes the inputs left out of its list, or only adds and changes those given. ([specs/kiloview-cube-x1.yaml:509](specs/kiloview-cube-x1.yaml#L509))
- [ ] Check whether manage_panel_outputs removes the outputs left out of its list, or only adds and changes those given. ([specs/kiloview-cube-x1.yaml:517](specs/kiloview-cube-x1.yaml#L517))
- [ ] Confirm that output ids are unique across panels, and that route takes the output's id as panel/detail lists it. ([specs/kiloview-cube-x1.yaml:540](specs/kiloview-cube-x1.yaml#L540))
- [ ] Confirm that disconnect_output (input_id null) leaves the output with no source and that panel/detail then shows input_id null. ([specs/kiloview-cube-x1.yaml:549](specs/kiloview-cube-x1.yaml#L549))
- [ ] Confirm that stop_rotation (r_id and loop null) stops the rotation list and what the output then carries. ([specs/kiloview-cube-x1.yaml:567](specs/kiloview-cube-x1.yaml#L567))
- [ ] Check what select_input_all with select off does to the outputs carrying that input. ([specs/kiloview-cube-x1.yaml:575](specs/kiloview-cube-x1.yaml#L575))
- [ ] Confirm that restart_service (server/reboot) restarts only the service, not the hardware, and how long the outputs stop. ([specs/kiloview-cube-x1.yaml:357](specs/kiloview-cube-x1.yaml#L357))
- [ ] Check what switch_template does with a template of another panel, and that save saves the panel's current template first. ([specs/kiloview-cube-x1.yaml:659](specs/kiloview-cube-x1.yaml#L659))
- [ ] Check what text find_users' filter is matched against. ([specs/kiloview-cube-x1.yaml:290](specs/kiloview-cube-x1.yaml#L290))
- [ ] Check that log/operate/download answers the log as plain text. ([specs/kiloview-cube-x1.yaml:342](specs/kiloview-cube-x1.yaml#L342))
- [ ] Check that discover_server's dis_s_ip is the Discovery Server's address (the document calls it the device IP there). ([specs/kiloview-cube-x1.yaml:436](specs/kiloview-cube-x1.yaml#L436))
- [ ] Check that set_http_ports takes effect at once and whether turning HTTP off keeps HTTPS answering. ([specs/kiloview-cube-x1.yaml:309](specs/kiloview-cube-x1.yaml#L309))
- [ ] Check what get_webrtc_sdp's server_ip must be (the server's own address is assumed). ([specs/kiloview-cube-x1.yaml:676](specs/kiloview-cube-x1.yaml#L676))
- [ ] Confirm that system_running_time is in seconds. ([specs/kiloview-cube-x1.yaml:781](specs/kiloview-cube-x1.yaml#L781))
- [ ] Find the unit and period of bytes_sent and bytes_recv (bytes in the last second is likely), and which of the two is sent, since the document's descriptions swap upstream and downstream. ([specs/kiloview-cube-x1.yaml:795](specs/kiloview-cube-x1.yaml#L795))
- [ ] Find the unit of the bitrates v_b and a_b in panel details. ([specs/kiloview-cube-x1.yaml:835](specs/kiloview-cube-x1.yaml#L835))
- [ ] Find the unit of cr_progress (seconds played, or a percentage). ([specs/kiloview-cube-x1.yaml:894](specs/kiloview-cube-x1.yaml#L894))
- [ ] Confirm the user roles: 0 for the built-in admin, 1 administrator, 2 ordinary user. ([specs/kiloview-cube-x1.yaml:813](specs/kiloview-cube-x1.yaml#L813))

## kiloview-decoder — Kiloview D-series decoders (D350, D260, RD-350, RD-260, FD-360)

- [ ] Confirm the web server's ports: 80 for HTTP and 443 for HTTPS (the document's example uses https://, and /network/http/get gives the ports), and that HTTPS uses a self-signed certificate. ([spec](specs/kiloview-decoder.yaml#L75))
- [ ] Confirm that the app header carrying only {"username": ..., "token": ...} is accepted on every request (the document's other example copies the whole login object), and that an empty token in it does not stop the login itself. ([spec](specs/kiloview-decoder.yaml#L90))
- [ ] Find out how a missing, wrong or expired token is answered: the session logs in again on result error with msg 2 (authentication error) and takes codes 300, 301 and 302 to a login as a wrong credential; also how long a token lives, and whether a restart ends it. ([specs/kiloview-decoder.yaml:98](specs/kiloview-decoder.yaml#L98))
- [ ] Confirm the login reply carries the token at data.token and result ok (the document's example has a placeholder result), and that the login accepts a JSON body (the table also lists username and password as query parameters). ([spec](specs/kiloview-decoder.yaml#L237))
- [ ] Confirm logout takes the token in the body as {"token": ...}, and that it ends the session the app header carries. ([specs/kiloview-decoder.yaml:256](specs/kiloview-decoder.yaml#L256))
- [ ] Confirm every reply is HTTP 200 with result ok or result error and a msg code, including the error replies, and that users/list answers {count, list} without result as documented. ([spec](specs/kiloview-decoder.yaml#L249))
- [ ] Check the error code table: its columns are misaligned in the PDF, and the code-to-text pairing in the quirk is read in order. ([spec](specs/kiloview-decoder.yaml#L2045))
- [ ] Confirm the user roles (admin and user are inferred from the examples) and that users/modify (titled modifyPwd) is the password change. ([spec](specs/kiloview-decoder.yaml#L256))
- [ ] Find out what GET /api/sys/reset ("System Reset") does compared with /api/sys/restore, and whether either keeps the network settings. ([spec](specs/kiloview-decoder.yaml#L309))
- [ ] Confirm that restore_factory returns the decoder to 192.168.1.168 and admin / admin. ([spec](specs/kiloview-decoder.yaml#L314))
- [ ] Confirm the firmware upload is multipart/form-data with the file in the field upload, without the optional path and size fields, and whether the reply comes before the restart. ([spec](specs/kiloview-decoder.yaml#L336))
- [ ] Confirm the cards (RD-350, RD-260, FD-360) have the same two outputs, interfaces and API as the D350 and D260. ([spec](specs/kiloview-decoder.yaml#L2084))
- [ ] Confirm the output_id types the requests send (a string or a number, as each request's example does) are all accepted. ([spec](specs/kiloview-decoder.yaml#L2084))
- [ ] Confirm the stream add and modify requests per protocol: an SRT Listener sent without address, udp and http sent without user and password, the modify requests without group_id, and show_advanced "1". ([spec](specs/kiloview-decoder.yaml#L447))
- [ ] Confirm the RTMP server's security "" (none) or "token" and its port range 1026 to 65534. ([spec](specs/kiloview-decoder.yaml#L488))
- [ ] Confirm the RTP SDP text starts with SDP: and lines separated by line feeds, as the examples give it. ([spec](specs/kiloview-decoder.yaml#L506))
- [ ] Confirm the NDI add request (Type ndi with a data list) with one source, and the channel values (HB, HX, FULL). ([spec](specs/kiloview-decoder.yaml#L519))
- [ ] Confirm the RTSP transport values tcp and udp, and the NDI transport values tcp, multicast, unicast and rudp. ([spec](specs/kiloview-decoder.yaml#L542))
- [ ] Confirm whole percents are accepted for pane geometry (the document types them as integers while its 3x3 template reports 33.33). ([spec](specs/kiloview-decoder.yaml#L745))
- [ ] Confirm the audio interface write: samplerate in Hz (48000) while the read reports 44.1, the volume range in dB, and whether video interfaces can be set the same way. ([spec](specs/kiloview-decoder.yaml#L817))
- [ ] Confirm the source/set body for putting a source into a window (from Type source with the stream, to Type the stream's protocol with pos_id); only the move between windows is shown in the document's example. ([spec](specs/kiloview-decoder.yaml#L833))
- [ ] Confirm which of res_id and res_index /output/resolution/set reads (both are sent). ([spec](specs/kiloview-decoder.yaml#L877))
- [ ] Find out what the audio device id of /output/audio/check is. ([spec](specs/kiloview-decoder.yaml#L961))
- [ ] Confirm the PTZ requests: Type as one string, data holding only the fields that type needs (empty for the others), floats with three decimals, and no layout_id; and what ptz_is_supported answers. ([spec](specs/kiloview-decoder.yaml#L1100))
- [ ] Confirm the white balance types are spelled ptz_white_balance_auto, _indoor, _outdoor and _oneshot (the document's table spells some with spaces). ([spec](specs/kiloview-decoder.yaml#L1062))
- [ ] Confirm set_network needs the MAC address and dhcp as "y" / "n". ([spec](specs/kiloview-decoder.yaml#L1114))
- [ ] Confirm the time source values (ntp, pc, manual: only pc appears in an example). ([spec](specs/kiloview-decoder.yaml#L1217))
- [ ] Confirm which of id and pos_id /preview/source/remove reads (both are sent). ([spec](specs/kiloview-decoder.yaml#L1254))
- [ ] Confirm /ndi/discovery/sources takes its parameters in the query, and that public discovery (dis_method 1) needs no group or server. ([spec](specs/kiloview-decoder.yaml#L1268))
- [ ] Confirm the auto and server discovery entries are deleted by group and address, as the examples give them, and not by the id the tables name. ([spec](specs/kiloview-decoder.yaml#L1292))
- [ ] Confirm the server discovery delete body ({dis_s_ip, dis_g_name}). ([spec](specs/kiloview-decoder.yaml#L1308))
- [ ] Find out the unit of video_bitrate (a string such as 55479.4, taken as kbit/s). ([spec](specs/kiloview-decoder.yaml#L1348))
- [ ] Confirm the audio levels are dB per channel and how many channels each stream gives. ([spec](specs/kiloview-decoder.yaml#L1381))
- [ ] Confirm users/list's create_time unit (ms since 1970 in one example, a date in the login's). ([spec](specs/kiloview-decoder.yaml#L1485))
- [ ] Confirm the stream status codes 0 to 4 have the same meaning in output/get and preview/get as in the source list. ([spec](specs/kiloview-decoder.yaml#L1595))

## kiloview-e3 — Kiloview E3 / RE-3 encoder

- [ ] With API authentication on, check that every request takes the login token in the Authorization header as it is (no Bearer prefix), that a missing or wrong token is answered with HTTP 401 (on which the session logs in again), and how long a token lasts. ([specs/kiloview-e3.yaml:120](specs/kiloview-e3.yaml#L120))
- [ ] Check how a failed login is answered (status and body, with and without the App header) and that a successful one gives the token at data.token. ([specs/kiloview-e3.yaml:186](specs/kiloview-e3.yaml#L186))
- [ ] Confirm that set_hostname is a GET with the name in a query parameter named name, as the document's table implies. ([specs/kiloview-e3.yaml:339](specs/kiloview-e3.yaml#L339))
- [ ] Check which reads answer bare JSON and which wrap it in result and data (the document mixes both): stream list, audio encoder list, video encoding, input detail, audio input, crop, orientation, OSD, EDID, PIP, ONVIF and NDI bridge are taken as bare. ([specs/kiloview-e3.yaml:2759](specs/kiloview-e3.yaml#L2759))
- [ ] Check how the device answers a read or write for a channel that is not in use (pip or uvc when not active): HTTP status and body. ([specs/kiloview-e3.yaml:2514](specs/kiloview-e3.yaml#L2514))
- [ ] Check whether the NDI-standard check answers valid JSON (true/false) or Python True/False as in the document's example; with JSON the state rule applies. ([specs/kiloview-e3.yaml:1794](specs/kiloview-e3.yaml#L1794))
- [ ] Confirm the PTZ diagonal actions (left-up, left-down, right-up, right-down) on the E3, as the P3 document names them. ([specs/kiloview-e3.yaml:1113](specs/kiloview-e3.yaml#L1113))
- [ ] Find the names of the PTZ preset actions (ending in preset) and of the zoom, focus, iris and white balance actions the web panel sends. ([specs/kiloview-e3.yaml:1133](specs/kiloview-e3.yaml#L1133))
- [ ] Confirm the recording status read GET /api/record/{interface}/recording (the Companion module uses /get_recording_status instead). ([specs/kiloview-e3.yaml:739](specs/kiloview-e3.yaml#L739))
- [ ] Confirm that list_audio_encoders is a POST and delete_audio_encoder a POST to the encoder's path, as documented, and not a GET and a DELETE. ([specs/kiloview-e3.yaml:1596](specs/kiloview-e3.yaml#L1596))
- [ ] Check the multipart part names and file names the uploads need (certificate, firmware, preset, EDID, image with its name field, standby image), and how long a whole firmware upload takes against the 5-second timeout. ([specs/kiloview-e3.yaml:446](specs/kiloview-e3.yaml#L446))
- [ ] Check what merge_firmware's number means (the chunk count, or the last chunk's number) and the chunk size the web page uses. ([specs/kiloview-e3.yaml:481](specs/kiloview-e3.yaml#L481))
- [ ] Find the unit of the network speed's receive and transmit values and of the temperature (degrees Celsius assumed). ([specs/kiloview-e3.yaml:2268](specs/kiloview-e3.yaml#L2268))
- [ ] Check the shape of the static route list (a list, an object, or inside result and data) and where each route's id is given. ([specs/kiloview-e3.yaml:2713](specs/kiloview-e3.yaml#L2713))
- [ ] Check that the NDI bridge write takes enable (as its example and the read) rather than enabled (its table), and port as a string. ([specs/kiloview-e3.yaml:2190](specs/kiloview-e3.yaml#L2190))
- [ ] Check that set_orientation's rotate is accepted as a number (the example) and not only as a string (the table). ([specs/kiloview-e3.yaml:1912](specs/kiloview-e3.yaml#L1912))
- [ ] Check that SetSwitch, SetEthernets and ChangeLinkinfo take the port as ifname (their examples) rather than iframe (their tables). ([specs/kiloview-e3.yaml:1325](specs/kiloview-e3.yaml#L1325))
- [ ] Check the EDID priority values (1 for 1080p, 2 for 4K, empty for none) on writing. ([specs/kiloview-e3.yaml:1456](specs/kiloview-e3.yaml#L1456))
- [ ] Check what get_webrtc answers and what set_webrtc's mode changes; the document's request and reply are unclear. ([specs/kiloview-e3.yaml:1070](specs/kiloview-e3.yaml#L1070))
- [ ] Confirm that RTMPS is added as type rtmp with an rtmps:// address. ([specs/kiloview-e3.yaml:817](specs/kiloview-e3.yaml#L817))
- [ ] Check which fields each stream type gives in the stream list (bindVideo, address, user, passwords) beyond the NDI example. ([specs/kiloview-e3.yaml:2761](specs/kiloview-e3.yaml#L2761))
- [ ] Check that modify_user and set_user_password accept a body with only some fields, as the document says. ([specs/kiloview-e3.yaml:244](specs/kiloview-e3.yaml#L244))
- [ ] Check the intercom language values (en and cn assumed from the examples). ([specs/kiloview-e3.yaml:2055](specs/kiloview-e3.yaml#L2055))
- [ ] Check that set_ptz's enable is 0 or 1 and the parity names the device takes. ([specs/kiloview-e3.yaml:1172](specs/kiloview-e3.yaml#L1172))
- [ ] Check how set_pip_input (vin/source) and set_pip_input_format (pip_fps_setting) differ on the device. ([specs/kiloview-e3.yaml:1506](specs/kiloview-e3.yaml#L1506))
- [ ] On an RE-3 in a chassis, check what the quick-stream and tally display settings do without a front panel. ([specs/kiloview-e3.yaml:3411](specs/kiloview-e3.yaml#L3411))
- [ ] Check that set_time with mode pc sets the time given, as manual does. ([specs/kiloview-e3.yaml:304](specs/kiloview-e3.yaml#L304))
- [ ] Check that a write refused for a bad value answers HTTP 200 with result error, and not an HTTP error status. ([specs/kiloview-e3.yaml:220](specs/kiloview-e3.yaml#L220))
- [ ] Confirm system_preset/download takes modules in the query as the JSON array text the document's example shows (modules=["time"]), percent-encoded. ([specs/kiloview-e3.yaml:564](specs/kiloview-e3.yaml#L564))

## kiloview-kilolink-server — Kiloview KiloLink Server Pro

- [ ] Find out how the server takes the login token back (the document only says every request needs it and the user name). Check whether the token sent as it is in Authorization, or the cookie user=<user>; token=<token>, is accepted. ([specs/kiloview-kilolink-server.yaml:53](specs/kiloview-kilolink-server.yaml#L53))
- [ ] Confirm that the login answers the token at $.token, and how a refused or expired token is answered: the session logs in again on HTTP 401 and on Appendix 4 codes 1005, 1009 and 1127 under result error. ([specs/kiloview-kilolink-server.yaml:63](specs/kiloview-kilolink-server.yaml#L63))
- [ ] Confirm that request parameters go as a JSON body (the document gives no format), and that the requests with no parameters accept an empty JSON object. ([specs/kiloview-kilolink-server.yaml:162](specs/kiloview-kilolink-server.yaml#L162))
- [ ] Confirm that the web API is on port 80 by default, the installer's web port (user manual 2.6), and that it has no HTTPS. ([specs/kiloview-kilolink-server.yaml:47](specs/kiloview-kilolink-server.yaml#L47))
- [ ] Record the device list's entries (the document says only "complex structure"), so devices can go into state. ([specs/kiloview-kilolink-server.yaml:298](specs/kiloview-kilolink-server.yaml#L298))
- [ ] Check that the tree's first level under each organization is read as Appendix 1 shows it, and how deeper levels appear. ([specs/kiloview-kilolink-server.yaml:467](specs/kiloview-kilolink-server.yaml#L467))
- [ ] Find out what OpenLDAPaci entries set_device_access takes (Appendix 3 is cut off). ([specs/kiloview-kilolink-server.yaml:257](specs/kiloview-kilolink-server.yaml#L257))
- [ ] Check that rename_node and move_node also work for users, groups and folders, not only devices. ([specs/kiloview-kilolink-server.yaml:233](specs/kiloview-kilolink-server.yaml#L233))
- [ ] Find out where a stream's key and id come from for set_stream (not in any documented reply). ([specs/kiloview-kilolink-server.yaml:359](specs/kiloview-kilolink-server.yaml#L359))
- [ ] Check that modify_node replaces every attribute and needs all of them. ([specs/kiloview-kilolink-server.yaml:224](specs/kiloview-kilolink-server.yaml#L224))
- [ ] Check what create_device_key does with the serial number. ([specs/kiloview-kilolink-server.yaml:392](specs/kiloview-kilolink-server.yaml#L392))
- [ ] Check the default administrator's password (the login command's example gives Kiloview001 and kiloview001) and how a wrong password is answered. ([specs/kiloview-kilolink-server.yaml:76](specs/kiloview-kilolink-server.yaml#L76))

## kiloview-mg300 — Kiloview MG300 V2

- [ ] Check that the token is sent as the header app: {"token":"..."} on every request (1.1), and that a refused or expired token is answered with HTTP 200 and {"result":"error","msg":"2"}, on which the session logs in again. ([specs/kiloview-mg300.yaml:86](specs/kiloview-mg300.yaml#L86))
- [ ] Check that logout takes the token from the app header and needs no body, and that the next request, answered code 2, logs in again. ([specs/kiloview-mg300.yaml:304](specs/kiloview-mg300.yaml#L304))
- [ ] Check that the users list filter search works as a query parameter. ([specs/kiloview-mg300.yaml:313](specs/kiloview-mg300.yaml#L313))
- [ ] Check that GET requests carrying a JSON body work (users/info, source/groups/list, overlay/editing/get, overlay/preview/get). ([specs/kiloview-mg300.yaml:320](specs/kiloview-mg300.yaml#L320))
- [ ] Check that users/modify with only id and password changes the password and nothing else. ([specs/kiloview-mg300.yaml:360](specs/kiloview-mg300.yaml#L360))
- [ ] Check that roles/permupdate accepts permission_ids alone, and name and description alone. ([specs/kiloview-mg300.yaml:373](specs/kiloview-mg300.yaml#L373))
- [ ] Check that users/auth_info_get and users/session/check accept a POST with no body. ([specs/kiloview-mg300.yaml:390](specs/kiloview-mg300.yaml#L390))
- [ ] Check that any error reply to session/check (the example gives 300) means the token is invalid. ([specs/kiloview-mg300.yaml:402](specs/kiloview-mg300.yaml#L402))
- [ ] Find out what overlay/select does with output_id 0, as the document's example sends it. ([specs/kiloview-mg300.yaml:462](specs/kiloview-mg300.yaml#L462))
- [ ] Find out the overlay/items/set modes besides z_index. ([specs/kiloview-mg300.yaml:477](specs/kiloview-mg300.yaml#L477))
- [ ] Check that the overlay image upload's multipart field img_url and the file name sent are accepted. ([specs/kiloview-mg300.yaml:514](specs/kiloview-mg300.yaml#L514))
- [ ] Check what overlay_id overlay/editing/get needs. ([specs/kiloview-mg300.yaml:526](specs/kiloview-mg300.yaml#L526))
- [ ] Check that Zixi add accepts the example's extra newUrl field, equal to url. ([specs/kiloview-mg300.yaml:718](specs/kiloview-mg300.yaml#L718))
- [ ] Check whether a Zixi stream's id and its zixi_* transport fields can be modified. ([specs/kiloview-mg300.yaml:883](specs/kiloview-mg300.yaml#L883))
- [ ] Check that the audio interface set ignores the id, label and visible fields the example sends. ([specs/kiloview-mg300.yaml:1044](specs/kiloview-mg300.yaml#L1044))
- [ ] Check the video interface set with only enable, output_id, type video and intf_id. There is no example for it. ([specs/kiloview-mg300.yaml:1060](specs/kiloview-mg300.yaml#L1060))
- [ ] Check the audio sample rate units: 44.1 (kHz) when read, 48000 (Hz) when written. ([specs/kiloview-mg300.yaml:2114](specs/kiloview-mg300.yaml#L2114))
- [ ] Check the encoder deinterlace_mode (default written, weave read) and the profile numbering (0 high, 1 main, 2 baseline). ([specs/kiloview-mg300.yaml:1369](specs/kiloview-mg300.yaml#L1369))
- [ ] Check that encoder enable is read-only, and how it relates to gate multi_out_enable. ([specs/kiloview-mg300.yaml:2131](specs/kiloview-mg300.yaml#L2131))
- [ ] Check whether window mute's pos_id counts from 0 (the example) or from 1. ([specs/kiloview-mg300.yaml:1398](specs/kiloview-mg300.yaml#L1398))
- [ ] Check that gateway adds are accepted without stream_id, and the defaults conn_intv 3 and conn_timeout 10. ([specs/kiloview-mg300.yaml:1421](specs/kiloview-mg300.yaml#L1421))
- [ ] Check the gateway RTSP defaults taken from the list example. ([specs/kiloview-mg300.yaml:1436](specs/kiloview-mg300.yaml#L1436))
- [ ] Find the gateway RTP/TS field values the web console uses by default (the document gives none, so TTL, PIDs, PCR period, TS id, tables version and PTS/PCR delay are required) and the numbering of load_type. ([specs/kiloview-mg300.yaml:1456](specs/kiloview-mg300.yaml#L1456))
- [ ] Find the gateway SRT encryption values other than "0", and whether the address may be empty for Listener. ([specs/kiloview-mg300.yaml:1497](specs/kiloview-mg300.yaml#L1497))
- [ ] Find the gateway HLS mode values other than server, and the segment length and count the web console uses by default (the document gives none, so both are required). ([specs/kiloview-mg300.yaml:1518](specs/kiloview-mg300.yaml#L1518))
- [ ] Find the gateway NDI HX connection values other than multicast. ([specs/kiloview-mg300.yaml:1531](specs/kiloview-mg300.yaml#L1531))
- [ ] Check the gateway list's field names per protocol (address or dest_url), and SRT encryption read as "0"/"1". ([specs/kiloview-mg300.yaml:2285](specs/kiloview-mg300.yaml#L2285))
- [ ] Find out what sys/reset does. ([specs/kiloview-mg300.yaml:1864](specs/kiloview-mg300.yaml#L1864))
- [ ] Check the config import through the multipart field upload with a .zip file name. ([specs/kiloview-mg300.yaml:1903](specs/kiloview-mg300.yaml#L1903))
- [ ] Check the firmware upload with no size part, and whether an image arrives within the request timeout. ([specs/kiloview-mg300.yaml:1948](specs/kiloview-mg300.yaml#L1948))
- [ ] Check preview add without to.pos_id, as the example sends it. ([specs/kiloview-mg300.yaml:1967](specs/kiloview-mg300.yaml#L1967))
- [ ] Check that ndi/discovery/auto/del removes entries of methods 1, 3 and 4 by id. ([specs/kiloview-mg300.yaml:2027](specs/kiloview-mg300.yaml#L2027))
- [ ] Check the output_id type of each request (string in some examples, number in others) and the border get. ([specs/kiloview-mg300.yaml:1317](specs/kiloview-mg300.yaml#L1317))
- [ ] Find out which output the overlay list's selected refers to. ([specs/kiloview-mg300.yaml:2152](specs/kiloview-mg300.yaml#L2152))
- [ ] Check the overlay item units: x/y in percent, width/height per mille. ([specs/kiloview-mg300.yaml:2161](specs/kiloview-mg300.yaml#L2161))
- [ ] Check the permission ids, which the example repeats under two groups, and the depth of the tree. ([specs/kiloview-mg300.yaml:2381](specs/kiloview-mg300.yaml#L2381))
- [ ] Confirm that a wrong user name or password is answered with code 300, 301 or 302 (section 1.3), which the session takes as the terminal refusal. ([specs/kiloview-mg300.yaml:94](specs/kiloview-mg300.yaml#L94))
- [ ] Confirm config/export answers the zip to a GET whose JSON body names modules ["source"] and the zip's name, as the document gives it, and an error as JSON. ([specs/kiloview-mg300.yaml:1915](specs/kiloview-mg300.yaml#L1915))

## kiloview-multiview — Kiloview Multiview Pro

- [ ] Confirm the default port: 2.1's text says 81, its URL rule shows 65532. Also check whether the server speaks https there, as the QR code example suggests. ([specs/kiloview-multiview.yaml:40](specs/kiloview-multiview.yaml#L40))
- [ ] Check that the app header {"username", "token"} is accepted without the other login fields the document's example header carries (alias, changed, role, language, software), and what a missing or expired token is answered with: the session logs in again on any {"result": "error"}, so a request refused for another reason is sent twice. ([specs/kiloview-multiview.yaml:58](specs/kiloview-multiview.yaml#L58))
- [ ] Check that the login itself needs no app header and answers the token at data.token. ([specs/kiloview-multiview.yaml:181](specs/kiloview-multiview.yaml#L181))
- [ ] Confirm the program list path, program/list.json; the document's heading reads prrogram/list.json. ([specs/kiloview-multiview.yaml:220](specs/kiloview-multiview.yaml#L220))
- [ ] Confirm the program rename path, program/rername.json, as the document spells it. ([specs/kiloview-multiview.yaml:237](specs/kiloview-multiview.yaml#L237))
- [ ] Find out what set_pane_source's from pane is when a source is put in from the discovery list, and that stream_id is the NDI source's name. ([specs/kiloview-multiview.yaml:568](specs/kiloview-multiview.yaml#L568))
- [ ] Check that set_window_screen with select 0 takes the window off the screens. ([specs/kiloview-multiview.yaml:446](specs/kiloview-multiview.yaml#L446))
- [ ] Check that set_window_output sends select "NDI" to turn NDI on and "" to turn it off. ([specs/kiloview-multiview.yaml:463](specs/kiloview-multiview.yaml#L463))
- [ ] Find out the range and unit of the toolbar audio gain value. ([specs/kiloview-multiview.yaml:541](specs/kiloview-multiview.yaml#L541))
- [ ] Check that get_single_layout takes its parameters in the query of a POST, as the example shows. ([specs/kiloview-multiview.yaml:590](specs/kiloview-multiview.yaml#L590))
- [ ] Check the PTZ data field for each type (9.2 lists the fields, not which type takes which) and what ptz_is_supported answers. ([specs/kiloview-multiview.yaml:722](specs/kiloview-multiview.yaml#L722))
- [ ] Check that ptz_white_balance sends ptz_white_balance_<mode> with empty data. ([specs/kiloview-multiview.yaml:825](specs/kiloview-multiview.yaml#L825))
- [ ] Confirm that take/set.json performs the Take (progress 1.0) rather than only storing the settings, and that a T-bar move sends time 0 with the T-bar's value. ([specs/kiloview-multiview.yaml:879](specs/kiloview-multiview.yaml#L879))
- [ ] Check that take/get.json takes pro_id in the query of a POST. ([specs/kiloview-multiview.yaml:897](specs/kiloview-multiview.yaml#L897))
- [ ] Check what get_window_sdp's ip is (described as the video source's address) and that switch_window's session id is the one get_window_sdp returns. ([specs/kiloview-multiview.yaml:713](specs/kiloview-multiview.yaml#L713))
- [ ] Check that pane and layout coordinates (x, y, width, height) are percentages of the window (the examples run from 0 to 100). ([specs/kiloview-multiview.yaml:1038](specs/kiloview-multiview.yaml#L1038))
- [ ] Find out what a pane's audio mix follow follows (6.24 names always, follow and close only). ([specs/kiloview-multiview.yaml:626](specs/kiloview-multiview.yaml#L626))
- [ ] Check whether any read reports a pane's audio_enable (get_single_layout's reply has none). ([specs/kiloview-multiview.yaml:605](specs/kiloview-multiview.yaml#L605))
- [ ] Record how a wrong password is answered (the document gives no failure example) and whether a token expires. ([specs/kiloview-multiview.yaml:54](specs/kiloview-multiview.yaml#L54))

## kiloview-multiview-server — Kiloview Multiview Server (NDI Multiview, API 1.0)

- [ ] Confirm the default port 65532 and that the API needs no authentication. ([specs/kiloview-multiview-server.yaml:30](specs/kiloview-multiview-server.yaml#L30))
- [ ] Find out what output/layout.json's actions change, set and remove do, and how a grid's source is chosen (the document gives no source field). ([specs/kiloview-multiview-server.yaml:211](specs/kiloview-multiview-server.yaml#L211))
- [ ] Check that get_stream_mode takes the window and grid as output and id (the document gives no parameters). ([specs/kiloview-multiview-server.yaml:282](specs/kiloview-multiview-server.yaml#L282))
- [ ] Check that set_stream_mode needs no window, as its example sends only id and mode. ([specs/kiloview-multiview-server.yaml:290](specs/kiloview-multiview-server.yaml#L290))
- [ ] Check whether the border and the audio gain reads are per window (6.6 and 6.8 name no window). ([specs/kiloview-multiview-server.yaml:245](specs/kiloview-multiview-server.yaml#L245))
- [ ] Check that set_audio_gain takes the gain in dB (0, 6, 10, 20) and what get_audio_gain's 10 means. ([specs/kiloview-multiview-server.yaml:263](specs/kiloview-multiview-server.yaml#L263))
- [ ] Check that set_take accepts autoTake, which its table lists and its example leaves out. ([specs/kiloview-multiview-server.yaml:354](specs/kiloview-multiview-server.yaml#L354))
- [ ] Check what set_ndi_status answers (the example shows {output, enable}, no result). ([specs/kiloview-multiview-server.yaml:339](specs/kiloview-multiview-server.yaml#L339))
- [ ] Check whether set_ndi_output's frameRate is the frame rate or its id in the frame rate list. ([specs/kiloview-multiview-server.yaml:381](specs/kiloview-multiview-server.yaml#L381))
- [ ] Check that set_grid_audio's volume on means unmuted, off muted. ([specs/kiloview-multiview-server.yaml:273](specs/kiloview-multiview-server.yaml#L273))
- [ ] Check that close_window and the other requests accept window ids as the examples type them (string in some, number in others). ([specs/kiloview-multiview-server.yaml:173](specs/kiloview-multiview-server.yaml#L173))
- [ ] Check what get_window_sdp's stream_id is. ([specs/kiloview-multiview-server.yaml:458](specs/kiloview-multiview-server.yaml#L458))
- [ ] Check that layout grids' x, y, width and height are percentages of the window (the examples run from 0 to 100). ([specs/kiloview-multiview-server.yaml:126](specs/kiloview-multiview-server.yaml#L126))
- [ ] Check that set_toolbar's type takes soundColumn, securityBox, centerline and nameEnable, the names 6.4's reply uses (6.5 says only "item name"). ([specs/kiloview-multiview-server.yaml:240](specs/kiloview-multiview-server.yaml#L240))
- [ ] Check whether any read reports set_take's syncVolume (8.3's reply has none). ([specs/kiloview-multiview-server.yaml:365](specs/kiloview-multiview-server.yaml#L365))

## kiloview-n60 — Kiloview N60 / N50

- [ ] Find which endpoints need a login token, and which of the three ways the core sends it (Authorization header as the Companion module does, the Cookie of language, user and token the document describes, or the App header) the device checks. ([specs/kiloview-n60.yaml:1787](specs/kiloview-n60.yaml#L1787))
- [ ] Record how the device answers a missing, wrong or expired token (an HTTP 401 or 403 has the session log in again; a 200 with result error or code 401, as the Companion module reads it, fails the command), how long a token lasts, and whether a reboot or a new login ends it; and that a wrong password is answered with result error. ([specs/kiloview-n60.yaml:92](specs/kiloview-n60.yaml#L92))
- [ ] Confirm that a device without a login takes requests without the Cookie and App headers, which are left out while the password is empty (the Companion module sends them with an empty token). ([specs/kiloview-n60.yaml:83](specs/kiloview-n60.yaml#L83))
- [ ] Record the default user name and password of a new N60 and N50 (the document's example is admin / Admin123, the Companion module's admin / admin). ([specs/kiloview-n60.yaml:1809](specs/kiloview-n60.yaml#L1809))
- [ ] Confirm set_mode's reply is the bare value true and how long the device is away while it restarts. ([specs/kiloview-n60.yaml:1819](specs/kiloview-n60.yaml#L1819))
- [ ] Find whether upload_firmware installs and restarts on its own, and the form field name it really takes (file, per the document). ([specs/kiloview-n60.yaml:1835](specs/kiloview-n60.yaml#L1835))
- [ ] Check whether set_bonding answers at all, and whether link aggregation can be left through the API. ([specs/kiloview-n60.yaml:1843](specs/kiloview-n60.yaml#L1843))
- [ ] Find what target_bitrate and max_bitrate read back after set_encoder_quality sets 55, 75, 100, 150 or 200, and their unit. ([specs/kiloview-n60.yaml:1868](specs/kiloview-n60.yaml#L1868))
- [ ] Check which picture each of NV12, NV16, NV12_10LE32 and NV16_10LE32 gives (4:2:0 or 4:2:2, 8 or 10 bit); the document's labels disagree with the format names. ([specs/kiloview-n60.yaml:1881](specs/kiloview-n60.yaml#L1881))
- [ ] On firmware before 2.02.0008, check whether choose_encode accepts or rejects the srt field, and whether Kilolink answers on /api/kilolink/ or only /api/KiloLink/. ([specs/kiloview-n60.yaml:1898](specs/kiloview-n60.yaml#L1898))
- [ ] Check what the encoder endpoints answer in decoding mode, and the decoder endpoints in encoding mode, so the polls of the other mode cost nothing. ([specs/kiloview-n60.yaml:1911](specs/kiloview-n60.yaml#L1911))
- [ ] Find the unit of getNetworkSpeed's receive and transmit, and of a stream's get_dynamic_state bitrate. ([specs/kiloview-n60.yaml:1092](specs/kiloview-n60.yaml#L1092))
- [ ] Find the PTZ action names for zoom, focus and presets, and check that home, stop-move and stop-all accept a speed. ([specs/kiloview-n60.yaml:1940](specs/kiloview-n60.yaml#L1940))
- [ ] Check the camera address range ptz_addr accepts (0 to 255 here) and the parity values (none, odd, even) set_ptz_serial takes. ([specs/kiloview-n60.yaml:648](specs/kiloview-n60.yaml#L648))
- [ ] Check that set_ptz_network and set_ptz_serial keep the other link's saved settings when they leave its fields out. ([specs/kiloview-n60.yaml:1940](specs/kiloview-n60.yaml#L1940))
- [ ] On an N50, check what the HDMI channel count and map of set_decode_output do on its SDI output, and the line-out channel range. ([specs/kiloview-n60.yaml:1950](specs/kiloview-n60.yaml#L1950))
- [ ] Confirm the request forms the document leaves implicit: setHostname?name=, remove_nas?id=, get_disk_files?device=&stream=, ptzControl?action=&speed=, check_proxy_token?token=, and setDiscoveryServer's JSON body of enable and address. ([specs/kiloview-n60.yaml:1968](specs/kiloview-n60.yaml#L1968))
- [ ] Check get_mode's reply on current firmware: plain text (the document) or JSON (the Companion module). ([specs/kiloview-n60.yaml:1978](specs/kiloview-n60.yaml#L1978))
- [ ] Check whether current firmware serves the endpoints only the Companion module uses (encoders/get_encode, signal_status, net_stream, streams/multi_protocol/get), which would warrant an updated document. ([specs/kiloview-n60.yaml:1978](specs/kiloview-n60.yaml#L1978))
- [ ] Check the NAS file systems add_nas accepts (nfs per the document; cifs is inferred from its mention of CIFS options). ([specs/kiloview-n60.yaml:927](specs/kiloview-n60.yaml#L927))
- [ ] Check the record format values set_record_settings accepts besides mov, and whether it and start_recording answer with a body. ([specs/kiloview-n60.yaml:1991](specs/kiloview-n60.yaml#L1991))
- [ ] Check the shape of the static route reply on a device with several routes (the document shows a single object without an id). ([specs/kiloview-n60.yaml:777](specs/kiloview-n60.yaml#L777))
- [ ] Check which values set_talk_language takes (the document gives cn for English and zh for Chinese). ([specs/kiloview-n60.yaml:2002](specs/kiloview-n60.yaml#L2002))
- [ ] Check whether the Kiloview FN-50 and FN-60 cards answer this API. ([specs/kiloview-n60.yaml:1991](specs/kiloview-n60.yaml#L1991))

## kiloview-ndi — Kiloview NDI converters (N3, N4, N30, N40, N5, N6)

- [ ] Confirm the N40 Plus takes the Version 1.0 API (/api/v1/...), as its shared N40/N40 Plus documentation page suggests, and which API the N3-s takes (its documentation page links none). ([spec](specs/kiloview-ndi.yaml#L150))
- [ ] Check how HTTPS is served on each model (a self-signed certificate is assumed; the certificate is not checked). ([spec](specs/kiloview-ndi.yaml#L108))
- [ ] Confirm the Version 1.0 authorize accepts a JSON body with username and password, and the N5/N6 authorize.json a form body with user and password; record the reply to a wrong password. ([spec](specs/kiloview-ndi.yaml#L187))
- [ ] Confirm N3/N4/N30/N40 accept the session and token in the API-Session and API-Token headers on every method. ([spec](specs/kiloview-ndi.yaml#L113))
- [ ] Confirm N5/N6 accept the token in the token cookie, and that requests without it are accepted when API authorization is turned off in the web interface (the cookie is left out while the password is empty). ([specs/kiloview-ndi.yaml:126](specs/kiloview-ndi.yaml#L126))
- [ ] Find the N5/N6 token's lifetime (the version 3.0 document gives none; Version 1.0 says 10 minutes without requests), whether it survives a mode switch and a reboot, and the exact auth-failed reply. ([spec](specs/kiloview-ndi.yaml#L985))
- [ ] Confirm the N5/N6 methods accept parameters as a JSON POST body (the version 3.0 document states no encoding), including JSON booleans and numbers. ([spec](specs/kiloview-ndi.yaml#L1004))
- [ ] Find what the N5/N6 connection mode no_multi_tcp means. ([spec](specs/kiloview-ndi.yaml#L297))
- [ ] Confirm the NDI|HX quality range 2000 to 40000 and its unit. ([spec](specs/kiloview-ndi.yaml#L337))
- [ ] Confirm the multicast TTL range (1 to 255 assumed; the document gives none). ([spec](specs/kiloview-ndi.yaml#L349))
- [ ] Confirm the scaling value none and the resolution values set_ndi_scaling takes. ([spec](specs/kiloview-ndi.yaml#L362))
- [ ] Confirm the Version 1.0 set_audio signal values embedded and analog are accepted as written. ([spec](specs/kiloview-ndi.yaml#L435))
- [ ] Confirm the exact N5/N6 audio signal values (HDMI, SDI, Line In, USB); the document writes "HDMI/SDI". ([spec](specs/kiloview-ndi.yaml#L446))
- [ ] Confirm set_NDI_enable takes a JSON boolean enable. ([spec](specs/kiloview-ndi.yaml#L476))
- [ ] Check the N5/N6 get_manual_targets reply: the document's example shows each list as one string holding a JSON array; the spec reads each element as one address or group. ([spec](specs/kiloview-ndi.yaml#L1442))
- [ ] Confirm presets and current/set take the preset number as a JSON string ("1"), and add.json takes position on N5/N6 but id on Version 1.0. ([spec](specs/kiloview-ndi.yaml#L535))
- [ ] Confirm recall_preset by ID works on N5/N6 firmware (the Companion module found it unreliable). ([spec](specs/kiloview-ndi.yaml#L591))
- [ ] Confirm N5/N6 require group when decoding a source by name and URL (reported by the Companion module, not documented), and whether Version 1.0 accepts it too. ([spec](specs/kiloview-ndi.yaml#L607))
- [ ] Confirm set_output_frame_rate takes a JSON number such as 29.97 (the document types it as an integer). ([spec](specs/kiloview-ndi.yaml#L665))
- [ ] Check whether sys-time/set accepts some fields alone, and the timetype values (pc and ntp in lower case assumed from the example). ([spec](specs/kiloview-ndi.yaml#L743))
- [ ] Check the host name's allowed characters and length (63 assumed). ([spec](specs/kiloview-ndi.yaml#L774))
- [ ] Confirm the discovery server path is /api/device/set_discovery_server.json (the document prints it without the leading slash). ([spec](specs/kiloview-ndi.yaml#L790))
- [ ] Confirm users/modify accepts the alias, the permissions or the password alone with id. ([spec](specs/kiloview-ndi.yaml#L853))
- [ ] Confirm mode/status answers result error while not ready on both dialects, and how long switch_mode takes to answer. ([spec](specs/kiloview-ndi.yaml#L1308))
- [ ] Find what output/get's enable, frameRate, hdmi_mode and colorspace mean on N5/N6, and whether any can be set. ([spec](specs/kiloview-ndi.yaml#L1516))
- [ ] Confirm server_info's port arrives as a number or digits (the version 3.0 example gives "443"). ([spec](specs/kiloview-ndi.yaml#L1549))
- [ ] Check whether tally can be read and set in encoder mode on N5/N6 (documented for decoder mode only). ([spec](specs/kiloview-ndi.yaml#L1088))
- [ ] Confirm the failsafe and third addresses are cleared with empty strings, and that a static address change keeps the API reachable at the new address. ([spec](specs/kiloview-ndi.yaml#L950))
- [ ] Check that a missing, wrong or expired token is answered result auth-failed on every model, on which the session signs in again. ([specs/kiloview-ndi.yaml:113](specs/kiloview-ndi.yaml#L113))

## kiloview-ndi-core — Kiloview NDI CORE / NDI CORE MAX

- [ ] Record how the server answers an expired or invalid bearer token (401 is assumed). ([specs/kiloview-ndi-core.yaml:44](specs/kiloview-ndi-core.yaml#L44))
- [ ] Confirm that get_long_term_token (user/token/permanent) works without an Authorization header, and that the token never expires. ([specs/kiloview-ndi-core.yaml:215](specs/kiloview-ndi-core.yaml#L215))
- [ ] Confirm that get_timed_token's minutes sets the token's lifetime. ([specs/kiloview-ndi-core.yaml:227](specs/kiloview-ndi-core.yaml#L227))
- [ ] Check how login answers a wrong user name or password: HTTP 200 with the error envelope, or 401. ([specs/kiloview-ndi-core.yaml:191](specs/kiloview-ndi-core.yaml#L191))
- [ ] Confirm that errors come with HTTP 200 and {"result": "error", ...}, and list the codes. ([specs/kiloview-ndi-core.yaml:892](specs/kiloview-ndi-core.yaml#L892))
- [ ] Confirm that system/info answers without a token, since the probe relies on it. ([specs/kiloview-ndi-core.yaml:49](specs/kiloview-ndi-core.yaml#L49))
- [ ] Check that the language header en is accepted on every request. ([specs/kiloview-ndi-core.yaml:46](specs/kiloview-ndi-core.yaml#L46))
- [ ] Check that NDI CORE (software) answers on port 80, or on which port, and whether it or the MAX serves HTTPS. ([specs/kiloview-ndi-core.yaml:88](specs/kiloview-ndi-core.yaml#L88))
- [ ] Record the event websocket's messages (ws://<host>/api_event/cube/ws/info?token=...) so they can replace polling. ([specs/kiloview-ndi-core.yaml:900](specs/kiloview-ndi-core.yaml#L900))
- [ ] Check that add_input with dis_method 0 and dis_g_name *manual* (the example) adds a usable source. ([specs/kiloview-ndi-core.yaml:372](specs/kiloview-ndi-core.yaml#L372))
- [ ] Check that modify_input refuses conn_type 1 in this version. ([specs/kiloview-ndi-core.yaml:412](specs/kiloview-ndi-core.yaml#L412))
- [ ] Check whether manage_panel_inputs and manage_panel_outputs remove entries left out of their lists. ([specs/kiloview-ndi-core.yaml:689](specs/kiloview-ndi-core.yaml#L689))
- [ ] Check whether modify_panel and add_panel need every current user in users. ([specs/kiloview-ndi-core.yaml:606](specs/kiloview-ndi-core.yaml#L606))
- [ ] Check that panel/icon takes id as a number (the table says string). ([specs/kiloview-ndi-core.yaml:646](specs/kiloview-ndi-core.yaml#L646))
- [ ] Confirm that route takes the output id as panel/detail lists it, and that output ids are unique across panels. ([specs/kiloview-ndi-core.yaml:705](specs/kiloview-ndi-core.yaml#L705))
- [ ] Confirm that disconnect_output (input_id null) leaves the output with no source. ([specs/kiloview-ndi-core.yaml:714](specs/kiloview-ndi-core.yaml#L714))
- [ ] Confirm that stop_rotation (r_id and loop null) stops the rotation list. ([specs/kiloview-ndi-core.yaml:732](specs/kiloview-ndi-core.yaml#L732))
- [ ] Check switch_template without the id field the document's example also sends (its table lists only switch_id and save). ([specs/kiloview-ndi-core.yaml:815](specs/kiloview-ndi-core.yaml#L815))
- [ ] Confirm that restart_service restarts only the service, not the hardware. ([specs/kiloview-ndi-core.yaml:360](specs/kiloview-ndi-core.yaml#L360))
- [ ] Check that log/operate/download answers the log as plain text. ([specs/kiloview-ndi-core.yaml:338](specs/kiloview-ndi-core.yaml#L338))
- [ ] Check what update_authorization answers for a wrong code. ([specs/kiloview-ndi-core.yaml:837](specs/kiloview-ndi-core.yaml#L837))
- [ ] Confirm that system_running_time is in seconds. ([specs/kiloview-ndi-core.yaml:939](specs/kiloview-ndi-core.yaml#L939))
- [ ] Find the unit and period of bytes_sent and bytes_recv. ([specs/kiloview-ndi-core.yaml:953](specs/kiloview-ndi-core.yaml#L953))
- [ ] Find the unit of v_b and a_b. ([specs/kiloview-ndi-core.yaml:998](specs/kiloview-ndi-core.yaml#L998))
- [ ] Find the unit of cr_progress. ([specs/kiloview-ndi-core.yaml:1073](specs/kiloview-ndi-core.yaml#L1073))
- [ ] Confirm the user roles: 0 built-in admin, 1 administrator, 2 ordinary user. ([specs/kiloview-ndi-core.yaml:977](specs/kiloview-ndi-core.yaml#L977))

## kiloview-p3 — Kiloview P3 / P3 Mini

- [ ] With API authentication on, confirm the login's token is accepted sent as it is in the Authorization header (no Bearer), that a missing or wrong token is answered 401 (on which the session logs in again), and whether the token expires. ([specs/kiloview-p3.yaml:73](specs/kiloview-p3.yaml#L73))
- [ ] Confirm the login path /api/systemctrl/user/login and the auth switch path /api/systemctrl/user/auth_info (the usage example says /users/). ([specs/kiloview-p3.yaml:111](specs/kiloview-p3.yaml#L111))
- [ ] Check the document's typo paths on a unit: /api/systemctr/user/add, /api/ststemctrl/system/production_stat and /api/networkamanager/wifi/getWifiName, and whether the correctly spelled paths work instead. ([specs/kiloview-p3.yaml:1708](specs/kiloview-p3.yaml#L1708))
- [ ] Find how recording is started and stopped: the documented GET /api/record/v1/get_recording_status with {"start":true}, or POST /api/record/v1/recording as on the E3. ([specs/kiloview-p3.yaml:491](specs/kiloview-p3.yaml#L491))
- [ ] Confirm the KiloLink server field is ip (lower case) in /api/kilolink/bonding/connect and its reads. ([specs/kiloview-p3.yaml:1285](specs/kiloview-p3.yaml#L1285))
- [ ] Confirm the spellings chosen over garbled translation: SRT bandwidth (not bindWidth), limit_type time_loop (not time-loop), NDI|HX connection value multicast, PTZ typ network, OSD type date and style bold, upper or lower case of USB, DNS and Timezone keys. ([specs/kiloview-p3.yaml:1727](specs/kiloview-p3.yaml#L1727))
- [ ] Check the NDI|HX multicast connection value. ([specs/kiloview-p3.yaml:617](specs/kiloview-p3.yaml#L617))
- [ ] Find how a PTZ move stops (no stop action documented) and how long a move lasts. ([specs/kiloview-p3.yaml:781](specs/kiloview-p3.yaml#L781))
- [ ] Confirm the form field name file and the {file_type} values crt and key for the certificate upload. ([specs/kiloview-p3.yaml:297](specs/kiloview-p3.yaml#L297))
- [ ] Confirm the form field name file for the one-request firmware upload. ([specs/kiloview-p3.yaml:353](specs/kiloview-p3.yaml#L353))
- [ ] Confirm the form field name file for the EDID upload. ([specs/kiloview-p3.yaml:1338](specs/kiloview-p3.yaml#L1338))
- [ ] Check the log service name for the bonding service (Kilolink as documented, or kilolink). ([specs/kiloview-p3.yaml:419](specs/kiloview-p3.yaml#L419))
- [ ] Find the units of the link and NIC rates (receive, transmit), the recording size and time limits, memory, disk and uptime. ([specs/kiloview-p3.yaml:1777](specs/kiloview-p3.yaml#L1777))
- [ ] Confirm whether HTTPS uses a self-signed certificate (accept_invalid_certs is set). ([specs/kiloview-p3.yaml:62](specs/kiloview-p3.yaml#L62))
- [ ] Check the system information load fields' spelling (precent in the example, percent in the table). ([specs/kiloview-p3.yaml:346](specs/kiloview-p3.yaml#L346))
- [ ] Check what the P3 Mini lacks against the P3 (inputs, SIM slots, Wi-Fi interfaces) and whether any interface answers differently. ([specs/kiloview-p3.yaml:1784](specs/kiloview-p3.yaml#L1784))
- [ ] Check the panel write's mixed-case source values (HDMI, sdi, UVC, line-in) and its single gain number. ([specs/kiloview-p3.yaml:1675](specs/kiloview-p3.yaml#L1675))
- [ ] Check whether modify_user can change a password (the document says so but lists no password field). ([specs/kiloview-p3.yaml:175](specs/kiloview-p3.yaml#L175))
- [ ] Check whether a GET on the recording file list with a mount point needs it percent-encoded as sent. ([specs/kiloview-p3.yaml:438](specs/kiloview-p3.yaml#L438))

## kiloview-rf02 — Kiloview RF02 Cradle chassis

- [ ] Confirm that the front-screen request netmgr/lcd-checkConf answers without a JWT, since the probe relies on it. ([specs/kiloview-rf02.yaml:37](specs/kiloview-rf02.yaml#L37))
- [ ] Record how an invalid or expired JWT is answered (401 is assumed, on which the session logs in again), and how long a login token lasts (the example's expiry is a century later). ([specs/kiloview-rf02.yaml:46](specs/kiloview-rf02.yaml#L46))
- [ ] Confirm that the login answers the token at data.token. ([specs/kiloview-rf02.yaml:166](specs/kiloview-rf02.yaml#L166))
- [ ] Record the real replies of the writes and the requests the document shows as {} (directory tree and services, maintenance, version, logs, firmware types, device configuration, bonding traffic), so they can go into state. ([specs/kiloview-rf02.yaml:512](specs/kiloview-rf02.yaml#L512))
- [ ] Check that the GET requests documented with a body (firmware types, list and model, device configuration) take those values as query parameters. ([specs/kiloview-rf02.yaml:512](specs/kiloview-rf02.yaml#L512))
- [ ] Check the directory permission body: the example's {directoryId, permissions[{roleId, permission}]} is sent, while the table lists roleKeys and action. ([specs/kiloview-rf02.yaml:333](specs/kiloview-rf02.yaml#L333))
- [ ] Find out what move_to_directory's path id, dirId and targetId each are. ([specs/kiloview-rf02.yaml:315](specs/kiloview-rf02.yaml#L315))
- [ ] Check the chunked firmware upload fields: the body example has identifier, number, file_type and file, while the code comments name chunkNumber, totalChunks and filename. Also find out how the chunks are joined. ([specs/kiloview-rf02.yaml:534](specs/kiloview-rf02.yaml#L534))
- [ ] Find out the maintenance plan's enable, day and policy values. ([specs/kiloview-rf02.yaml:468](specs/kiloview-rf02.yaml#L468))
- [ ] Check the set_system time_type values (ntp is documented; manual is assumed). ([specs/kiloview-rf02.yaml:451](specs/kiloview-rf02.yaml#L451))
- [ ] Find out the date format of get_logs and export_logs. ([specs/kiloview-rf02.yaml:496](specs/kiloview-rf02.yaml#L496))
- [ ] Find out the operating modes set_card_mode takes besides encode. ([specs/kiloview-rf02.yaml:668](specs/kiloview-rf02.yaml#L668))
- [ ] Find out the dashboard type values. ([specs/kiloview-rf02.yaml:771](specs/kiloview-rf02.yaml#L771))
- [ ] Check that lcd_factory_reset's password is kiloview. ([specs/kiloview-rf02.yaml:596](specs/kiloview-rf02.yaml#L596))
- [ ] Check that the users and roles lists accept pageSize 1000. ([specs/kiloview-rf02.yaml:981](specs/kiloview-rf02.yaml#L981))
- [ ] Find out the user status codes besides 2 (enabled), and the gateway device status codes. ([specs/kiloview-rf02.yaml:882](specs/kiloview-rf02.yaml#L882))
- [ ] Find the units of the gateway device CPU, memory, uptime and disk values. ([specs/kiloview-rf02.yaml:923](specs/kiloview-rf02.yaml#L923))
- [ ] Check that delete_gateway_channel's single-entry list body is accepted. ([specs/kiloview-rf02.yaml:402](specs/kiloview-rf02.yaml#L402))

## kissbox-io — KissBox IO3CC and IO8CC I/O card frames

- [ ] Check where a frame on the UDP firmware sends its answers to reads: to the destination port set in the Editor as a broadcast (as the core expects, listening on feedback_port), or back to the sender's port. ([specs/kissbox-io.yaml:109](specs/kissbox-io.yaml#L109))
- [ ] Check that input change notifications use the all channel status (A1h) or single channel status (A3h) layout, which the protocol document does not state for notifications. ([specs/kissbox-io.yaml:7](specs/kissbox-io.yaml#L7))

## kramer-p3000 — Kramer Protocol 3000

- [ ] Check that the KDS-EN7/DEC7, DSP-62 and MV-4X accept a space after the command name, which their manuals leave out. ([specs/kramer-p3000.yaml:4103](specs/kramer-p3000.yaml#L4103))
- [ ] Record the MV-4X ROTATE? reply (written "~nn@#ROTATE" in the manual) and confirm the rotation needs aspect ratio Full and the border off. ([specs/kramer-p3000.yaml:2755](specs/kramer-p3000.yaml#L2755))
- [ ] Record the KDS-DEC7's KDS-CHANNEL-SELECT replies to a set (the signal list in brackets) and to a get (signal,channel), and whether signal names come back in lower case. ([specs/kramer-p3000.yaml:3904](specs/kramer-p3000.yaml#L3904))
- [ ] Find the largest grid VIEW-MOD 15 takes (16 by 16 is assumed; the manual gives none) and the TTL range of KDS-MULTICAST (1-255 assumed). ([specs/kramer-p3000.yaml:2988](specs/kramer-p3000.yaml#L2988))
- [ ] Check NET-SRV-DSCP's service numbering (0 stream, 1 control), which the manual gives the other way round from NET-SRV-802-1Q. ([specs/kramer-p3000.yaml:2971](specs/kramer-p3000.yaml#L2971))
- [ ] Confirm the DSP-62's DSP field ids are the lower-case words of its examples (attack, low-freq, band-bypass, tone-freq) and which values take decimals. ([specs/kramer-p3000.yaml:3183](specs/kramer-p3000.yaml#L3183))
- [ ] Confirm the DSP-62's signal generator command is DSP-SIGGEN (its syntax and example) rather than DSP-SIG-GEN (its table). ([specs/kramer-p3000.yaml:3335](specs/kramer-p3000.yaml#L3335))
- [ ] Confirm DSP-METER is sent with "?" (the manual's syntax leaves it out) and record its reply. ([specs/kramer-p3000.yaml:3352](specs/kramer-p3000.yaml#L3352))
- [ ] Check that X-MIX-LVL accepts one decimal place (-25.0) and replies with the level the same way. ([specs/kramer-p3000.yaml:3361](specs/kramer-p3000.yaml#L3361))
- [ ] Confirm KDS-MULTICAST takes any group address, which the KDS-7 ignores and reports as 0.0.0.0. ([specs/kramer-p3000.yaml:2890](specs/kramer-p3000.yaml#L2890))

- [ ] Check whether ROUTE with source 0 disconnects, as the legacy commands do. ([specs/kramer-p3000.yaml:3505](specs/kramer-p3000.yaml#L3505))
- [ ] Check whether AUD-LVL's channel means the Audio Channel table or an input/output number on the product in use. ([specs/kramer-p3000.yaml:3622](specs/kramer-p3000.yaml#L3622))
- [ ] Confirm the documentation inconsistencies taken as written: the LABEL query with only a port number, NAME? echoed with the question mark, the #INFO-PRST? query form, and PRST-STO/PRST-RCL replies without "nn@". ([specs/kramer-p3000.yaml:3521](specs/kramer-p3000.yaml#L3521))
- [ ] Find the LOGIN level argument's wire spelling, so login can be modelled. ([specs/kramer-p3000.yaml:3657](specs/kramer-p3000.yaml#L3657))
- [ ] Record the exact X-SIGNAL and X-AFV reply forms, which are matched leniently because the guide has typing errors. ([specs/kramer-p3000.yaml:3668](specs/kramer-p3000.yaml#L3668))
- [ ] Confirm VMUTE flag 2 (blank picture) is unsupported. ([specs/kramer-p3000.yaml:3620](specs/kramer-p3000.yaml#L3620))
- [ ] On a presentation switcher (VP-440X, VP-551X or VP-427X2), a multiviewer (MV-4X) and a streamer (KDS-EN7), record the values VID-RES, IMAGE-PROP, SCLR-AS, VIEW-MOD and W-COLOR take and answer, and which channels count from 0. ([specs/kramer-p3000.yaml:4067](specs/kramer-p3000.yaml#L4067))
- [ ] Check whether EDID-AUDIO takes the guide's single mode or the per-input form (input,format) on each product. ([specs/kramer-p3000.yaml:1597](specs/kramer-p3000.yaml#L1597))
- [ ] Confirm GPIO-CFG answers type,direction without the pull-up, and that GPIO-STATE is pushed on a change. ([specs/kramer-p3000.yaml:2027](specs/kramer-p3000.yaml#L2027))
- [ ] Confirm the KDS-ACTION values (0 stop, 1 play, 2 save), since the guide's table is a copy of another. ([specs/kramer-p3000.yaml:2194](specs/kramer-p3000.yaml#L2194))
- [ ] Record how ETH-PORT names the protocol in its reply (TCP/UDP or 0/1) and whether a TCP port change ends the connection before the answer. ([specs/kramer-p3000.yaml:2417](specs/kramer-p3000.yaml#L2417))
- [ ] Record the TIME? reply's date form (the guide's DD-MM-YYYY or SWT3's YYYY/MM/DD). ([specs/kramer-p3000.yaml:2444](specs/kramer-p3000.yaml#L2444))
- [ ] Check which network changes end the connection at once, and that NET-IP answers before the address changes. ([specs/kramer-p3000.yaml:4056](specs/kramer-p3000.yaml#L4056))
- [ ] On the VP-551X, check MIC-GAIN's three-parameter form (0,mic,level) against the two-parameter command offered. ([specs/kramer-p3000.yaml:2599](specs/kramer-p3000.yaml#L2599))
- [ ] Check that the queries added to the poll on connecting are answered ERR 002, not ignored, on products without them. ([specs/kramer-p3000.yaml:4084](specs/kramer-p3000.yaml#L4084))
- [ ] On each product, confirm the RS-232 control port runs 115200 8N1 with no flow control and takes the same commands and handshake as TCP 5000, and that #BAUD? 0 lists the rates link serial offers. ([specs/kramer-p3000.yaml:110](specs/kramer-p3000.yaml#L110))
- [ ] Check whether the RS-232 port needs LOGIN when security is on (product-specific, guide p.49). ([specs/kramer-p3000.yaml:3931](specs/kramer-p3000.yaml#L3931))

## kuma-timer — KUMA Timer

- [ ] Confirm the actions the API page does not list (add_seconds, cut_nudge, prev_cue, recall_layout, set_mode, count_up, warp_time, warp_duration, cancel_warp, cancel_jump, the QLab, OMT, LTC and Blackmagic switches) and send_sms's fullscreen flag on the current KUMA. ([specs/kuma-timer.yaml:485](specs/kuma-timer.yaml#L485))
- [ ] Confirm an empty key is accepted when no web control password is set, and record the answer to a wrong key. ([specs/kuma-timer.yaml:62](specs/kuma-timer.yaml#L62))
- [ ] Record the status fields beyond the documented ones (cues, preset_seconds, layout_presets, ppt, the QLab, OMT, LTC and Blackmagic flags) and their types. ([specs/kuma-timer.yaml:340](specs/kuma-timer.yaml#L340))
- [ ] Check whether recall_layout's slot counts from 1 and how a name that does not exist is answered. ([specs/kuma-timer.yaml:224](specs/kuma-timer.yaml#L224))

## labgruppen-lake — Lab.gruppen Lake (DLM)

- [ ] Confirm a frame answers `Dev.Network.ID?` sent to its address with the broadcast id and class 0, and that the answer's source id is its frame id. ([specs/labgruppen-lake.yaml:768](specs/labgruppen-lake.yaml#L768))
- [ ] Confirm dynamic port mode: a packet to UDP 6016 is answered to the sending port, so the core works beside Lake Controller. ([specs/labgruppen-lake.yaml:763](specs/labgruppen-lake.yaml#L763))
- [ ] Record the exact text of a get's answer (the value alone, or the command echoed) for one-value and several-value gets, such as `Mod.Out.Gain?A 1` and `Dev.LoadPilot.Readings?1`. ([specs/labgruppen-lake.yaml:806](specs/labgruppen-lake.yaml#L806))
- [ ] Check labels and preset names with spaces are accepted as the last argument (`Mod.Out.Label=A 1 Main Left`, `Dev.Preset.Store!1 Show A`). ([specs/labgruppen-lake.yaml:743](specs/labgruppen-lake.yaml#L743))
- [ ] Check the acknowledgement codes a frame really sends for a bad parameter and an unknown path (-3 to -6 in the document's misaligned table). ([specs/labgruppen-lake.yaml:819](specs/labgruppen-lake.yaml#L819))
- [ ] Check `Dev.MD.FullBin?3` answers with the 108-byte version 3 structure as the payload alone, with no text before it, and the same for `?2` on a PLM and the LM structure. ([specs/labgruppen-lake.yaml:812](specs/labgruppen-lake.yaml#L812))
- [ ] Check the RMS gain reduction scale (0.1 dB a step, as read here, or 0.5 dB as one table says). ([specs/labgruppen-lake.yaml:812](specs/labgruppen-lake.yaml#L812))
- [ ] Confirm the D 10:4L, D 20:4L and D 40:4L answer DLM as the other D Series, and note their model names from `Dev.ModelName?`. ([specs/labgruppen-lake.yaml:838](specs/labgruppen-lake.yaml#L838))
- [ ] Check whether `Dev.NetworkIPConf?` (as the document spells it) or `Dev.Network.IPConf?` answers, and whether the PLM 20000Q takes `Dev.PTG2.Active` (the example) or `Dev.PTG.Active` (the heading), for the set and the configuration round's read. ([specs/labgruppen-lake.yaml:832](specs/labgruppen-lake.yaml#L832))
- [ ] Note how many polled requests a second a frame takes before it falls behind, and whether a full parameter round every 5 s, and the configuration round (about 300 reads on a D Series with six channels a module) every minute, disturb Lake Controller. ([specs/labgruppen-lake.yaml:774](specs/labgruppen-lake.yaml#L774))
- [ ] Confirm a frame does not reset on any typed command on firmware before 2.50 (the Lake Controller 8 known issue about invalid DLM messages). ([specs/labgruppen-lake.yaml:750](specs/labgruppen-lake.yaml#L750))
- [ ] Record the answers of the several-value gets and check their word order: `Dev.Fuse.NominalCurrent?` (nominal, actual), `Dev.LoadPilot.Signal?`, `Threshold?` and `Readings?`, `Dev.Router.InputTypSel?` with a Dante name holding spaces, `Dev.Dante.BreakIn?` for a probe, `Mod.Out.Phase?` (polarity, lock) and `Mod.Out.AmpVPL?` (desired, minimum, maximum, actual). ([specs/labgruppen-lake.yaml:798](specs/labgruppen-lake.yaml#L798))
- [ ] Record `Dev.Route?` and the LM's `Dev.Out.Route?` answers: the character for a position not routed (read as anything but X), and whether `Dev.Route?` separates positions with spaces. ([specs/labgruppen-lake.yaml:798](specs/labgruppen-lake.yaml#L798))
- [ ] Check `Dev.PTG.Impedance?` without a channel answers all four impedances on a PLM, with `?` for a channel not measured. ([specs/labgruppen-lake.yaml:798](specs/labgruppen-lake.yaml#L798))
- [ ] Check `Dev.Out.Route=` on an LM takes - for a position not routed, as `set_output_route_lm` sends it. ([specs/labgruppen-lake.yaml:350](specs/labgruppen-lake.yaml#L350))
- [ ] Check which models answer `Dev.AesLoopTermination?` (the document lists PLM+, PLM and LM, not D Series), `Dev.Dante.SlaveOnly?` and `Dev.Network.Redund?` (PLM 4.33, LM 2.30), and what an older firmware answers to a read it lacks. ([specs/labgruppen-lake.yaml:774](specs/labgruppen-lake.yaml#L774))

## labgruppen-nlb60e — Lab.gruppen NLB 60E (NomadLink)

- [ ] Confirm the bridge accepts the operator with spaces around it (`Subnet.Mute = 1`, `Subnet.Mute ?`), as the examples write it, and answers with the value alone. ([specs/labgruppen-nlb60e.yaml:375](specs/labgruppen-nlb60e.yaml#L375))
- [ ] Check virtual device names with dots (the document's examples) are accepted, as its character rules exclude them; such names are not kept in state. ([specs/labgruppen-nlb60e.yaml:375](specs/labgruppen-nlb60e.yaml#L375))
- [ ] Check what `Subnet.Status ?` answers with no faulty amplifier (nothing after the faults flag, or a trailing space). ([specs/labgruppen-nlb60e.yaml:221](specs/labgruppen-nlb60e.yaml#L221))
- [ ] Note whether polling `Subnet.Status ?` and the 60 VDN slots every 5 s, with three reads per named amplifier, is acceptable to the bridge, and whether the third-party port still works on firmware newer than 2.1.0. ([specs/labgruppen-nlb60e.yaml:219](specs/labgruppen-nlb60e.yaml#L219))
- [ ] Confirm the bridge answers strictly one message at a time and in order, so a value is the answer to the message in flight, as the state per amplifier assumes (document section 2). ([specs/labgruppen-nlb60e.yaml:234](specs/labgruppen-nlb60e.yaml#L234))
- [ ] Record what `Subnet.VDN<n> ?` answers for a named slot (name and serial separated by one space assumed) and an empty one (* assumed). ([specs/labgruppen-nlb60e.yaml:246](specs/labgruppen-nlb60e.yaml#L246))
- [ ] Record an amplifier's `Status ?` answer for 2-, 4- and 8-channel models: three values, then nine per channel from A. ([specs/labgruppen-nlb60e.yaml:275](specs/labgruppen-nlb60e.yaml#L275))

## lectrosonics-aspen — Lectrosonics ASPEN processors

- [ ] Confirm that a verbose wildcard query is answered as OK target(*)={...} (and OK target(*,bus)={...} for crosspoints), the values in address order. ([specs/lectrosonics-aspen.yaml:4370](specs/lectrosonics-aspen.yaml#L4370))
- [ ] Confirm that a verbose update is answered with the value read back, as OK target(address)=value, and a verbose query of a target without an address as OK target=value. ([specs/lectrosonics-aspen.yaml:10](specs/lectrosonics-aspen.yaml#L10))
- [ ] Record the RS-232 port's default speed and line settings (8N1 without flow control assumed) and the cable wiring. ([specs/lectrosonics-aspen.yaml:61](specs/lectrosonics-aspen.yaml#L61))
- [ ] Confirm that the device answers its first request on a new TCP connection without any greeting, and how many TCP clients it takes on each port. ([specs/lectrosonics-aspen.yaml:56](specs/lectrosonics-aspen.yaml#L56))
- [ ] Record what the device sends for a command a model lacks and for a value out of range (ERROR assumed for both). ([specs/lectrosonics-aspen.yaml:55](specs/lectrosonics-aspen.yaml#L55))
- [ ] Check the virtual programmable input functions on SPNConference and SPNTrio (their pages give the range 0-16 but list 0-19). ([specs/lectrosonics-aspen.yaml:3270](specs/lectrosonics-aspen.yaml#L3270))
- [ ] Check the time array (the reference says four values and shows three). ([specs/lectrosonics-aspen.yaml:3515](specs/lectrosonics-aspen.yaml#L3515))

## lg-display — LG signage displays and commercial TVs

- [ ] Confirm the display takes the protocol on TCP 9761 with no greeting or login, and answers as over RS-232C; no LG document read names the port. ([specs/lg-display.yaml:3257](specs/lg-display.yaml#L3257))
- [ ] Confirm an acknowledgement ends with the literal `x` and nothing after it (no CR or LF), and that its Set ID echo is two hex digits (three above 255). ([specs/lg-display.yaml:99](specs/lg-display.yaml#L99))
- [ ] Record what a display in standby answers to `ka 01 ff` in each PM mode (Network Ready, Screen Off Always), and whether `ka 01 01` turns it on over TCP and is acknowledged. ([specs/lg-display.yaml:3266](specs/lg-display.yaml#L3266))
- [ ] Check whether lowercase hex (`kf 01 3c`), as LG's examples write it, and uppercase are both taken. ([specs/lg-display.yaml:103](specs/lg-display.yaml#L103))
- [ ] Check the OSD lock polarity (`kl 00` locks in the webOS 6.0 and 3.2 guides, unlocks in the L19 guide). ([specs/lg-display.yaml:1253](specs/lg-display.yaml#L1253))
- [ ] Check the tile mode byte order (columns then rows in the webOS guides, rows then columns in the SE3B manual). ([specs/lg-display.yaml:3302](specs/lg-display.yaml#L3302))
- [ ] Confirm the color temperature data on webOS 3.2 and L19 displays (70-D2 for 3200-13000 K, as webOS 6.0, or 00-64 as older generations). ([specs/lg-display.yaml:361](specs/lg-display.yaml#L361))
- [ ] Record whether `xb ff` answers the PC or the DTV code of a port in each input label setting. ([specs/lg-display.yaml:164](specs/lg-display.yaml#L164))
- [ ] Record the `sv` replies (is there a space between the sub-code and the data?) and the bit order of the temperature sensor health (`sv 07`). ([specs/lg-display.yaml:1985](specs/lg-display.yaml#L1985))
- [ ] Record the fan speed data of `sv 09` for one and for several fans. ([specs/lg-display.yaml:1990](specs/lg-display.yaml#L1990))
- [ ] Record the `xc ff` reply and whether `xc` takes 00 for an unused window (PIP with two inputs). ([specs/lg-display.yaml:1771](specs/lg-display.yaml#L1771))
- [ ] Record the `mj ff` reply and how many port codes each model expects. ([specs/lg-display.yaml:1855](specs/lg-display.yaml#L1855))
- [ ] Record what a timer, holiday and brightness schedule entry read answers (whether the entry code is echoed before the data) and an empty entry. ([specs/lg-display.yaml:1628](specs/lg-display.yaml#L1628))
- [ ] Confirm the units of `dn` (degrees Celsius assumed) and `dl` (hours assumed), and the 3-byte `dn` form of the SE3B. ([specs/lg-display.yaml:1945](specs/lg-display.yaml#L1945))
- [ ] Record the format of the serial number (`fy`) and software version (`fz`) data: plain text, or ASCII in hex digits as `jx` answers. ([specs/lg-display.yaml:1960](specs/lg-display.yaml#L1960))
- [ ] Confirm the IR codes LG prints only as icons: 7E, and the media keys B0, B1, BA, 8F, 8E, AA on signage. ([specs/lg-display.yaml:2020](specs/lg-display.yaml#L2020))
- [ ] Confirm the daylight saving read `sd 01 01 ff ff ff ff` answers the start entry with its 01 echoed first. ([specs/lg-display.yaml:876](specs/lg-display.yaml#L876))
- [ ] Confirm the equalizer byte (band in bits 7-5, step in bits 4-0) on a commercial TV. ([specs/lg-display.yaml:814](specs/lg-display.yaml#L814))
- [ ] Drive an LG display on its RS-232C port (link serial) and confirm the acknowledgements are those of TCP 9761; confirm a commercial TV (UV340C, UL3E) runs at 9600 bps, 8N1, no flow control as the signage guides state. ([specs/lg-display.yaml:107](specs/lg-display.yaml#L107))

## lightkey — Lightkey (OSC)

- [ ] Confirm Lightkey sends nothing back over OSC (no feedback or replies), so no state can be read. ([specs/lightkey.yaml:288](specs/lightkey.yaml#L288))
- [ ] Check that commands sent over UDP to 21600 act, and how Lightkey frames OSC over TCP on the same port. ([specs/lightkey.yaml:298](specs/lightkey.yaml#L298))
- [ ] Check the value ranges Lightkey accepts for modifiers, fixture properties, fade times and tempo. ([specs/lightkey.yaml:305](specs/lightkey.yaml#L305))

## lightware-lw2 — Lightware LW2 (bracket protocol) matrices and switchers

- [ ] Confirm that a CR LF after the closing bracket is accepted (Companion's module sends it; the manuals give no terminator). ([specs/lightware-lw2.yaml:696](specs/lightware-lw2.yaml#L696))
- [ ] Record the error answers ("(ERR04)" and others) and whether a failed command is answered at all. ([specs/lightware-lw2.yaml:696](specs/lightware-lw2.yaml#L696))
- [ ] Check whether front panel changes are reported unasked on TCP 10001 (none documented; the crosspoint is polled). ([specs/lightware-lw2.yaml:705](specs/lightware-lw2.yaml#L705))
- [ ] Confirm the EDID learn order ({location>output}, as the manual's example). ([specs/lightware-lw2.yaml:749](specs/lightware-lw2.yaml#L749))
- [ ] Check how the UMX answers {VC} without a layer. ([specs/lightware-lw2.yaml:705](specs/lightware-lw2.yaml#L705))
- [ ] On an MX-CPU1 frame and an MX8x8, check which of edid_route, RS232BAUD, GETTIME/SETTIME and PS/(PSC) exist (documented for MX-CPU2 only), and on an MX-CPU2 frame whether HDMIIGET/HDMIOGET/HDMISET still work. ([specs/lightware-lw2.yaml:740](specs/lightware-lw2.yaml#L740))
- [ ] Record the ISD and OSD digits for each board type on a live frame, and check the MX8x8's digits against its bit table. ([specs/lightware-lw2.yaml:733](specs/lightware-lw2.yaml#L733))
- [ ] Check that polling {VC}, {VM}, {:ISD}, {:OSD}, {VEDID}, {IP_CONFIG=?} and {RS232BAUD=?} every 5 seconds does not slow an MX matrix, and that an MX8x8 or MX-CPU1 answers {RS232BAUD=?} with an error that changes nothing. ([specs/lightware-lw2.yaml:800](specs/lightware-lw2.yaml#L800))
- [ ] Confirm set_ip_dhcp and reset_ip_config answer "(Changing IP configuration...)" before the connection drops. ([specs/lightware-lw2.yaml:691](specs/lightware-lw2.yaml#L691))
- [ ] Confirm a switch to a locked output answers (1LO<out>) on the MX8x8 too. ([specs/lightware-lw2.yaml:701](specs/lightware-lw2.yaml#L701))
- [ ] Check the UMX GPIO pin range (the manual prints 0 ...8 and pin 1 to 8; the hardware has 7) and the RS232_LOCAL_FORMAT reply. ([specs/lightware-lw2.yaml:656](specs/lightware-lw2.yaml#L656))
- [ ] Confirm set_hdmi_output's answer is the output's STO line. ([specs/lightware-lw2.yaml:528](specs/lightware-lw2.yaml#L528))
- [ ] Capture the {VM} answer on an MX matrix and confirm it lists one 1 or 0 per output in order after MUT, so the mutes poll keys them by position; and confirm {VC V} and {VC A} answer on the UMX as get_connection_layer's replies do. ([specs/lightware-lw2.yaml:842](specs/lightware-lw2.yaml#L842))
- [ ] Drive an MX8x8 Pro, an MX-CPU2 frame and a UMX-HDMI-140 on RS-232 (link serial) and confirm the same answers as over TCP 10001, and that no flow control is used. ([specs/lightware-lw2.yaml:81](specs/lightware-lw2.yaml#L81))
- [ ] On a UMX-HDMI-140-Plus with the Cleartext login on, confirm LW2 on RS-232 is refused (no LW2 login command). ([specs/lightware-lw2.yaml:674](specs/lightware-lw2.yaml#L674))

## lightware-lw3 — Lightware LW3 matrices, switchers and extenders

- [ ] Check whether "OPEN <node>/*" also subscribes to grandchild nodes. ([specs/lightware-lw3.yaml:4402](specs/lightware-lw3.yaml#L4402))
- [ ] Record the answers to OPEN, GET and CALL on each tree, including the error lines for another tree's paths. ([specs/lightware-lw3.yaml:4384](specs/lightware-lw3.yaml#L4384))
- [ ] On MX2, confirm the preset, device label and muteSource syntax where the manual contradicts itself, and the /MEDIA/NAMES/VIDEO port names. ([specs/lightware-lw3.yaml:4436](specs/lightware-lw3.yaml#L4436))
- [ ] On MMX2, check whether method replies are mO (the manual prints m0 in places). ([specs/lightware-lw3.yaml:4454](specs/lightware-lw3.yaml#L4454))
- [ ] On UMX-HDMI-140-Plus, record the answer to a wrong Cleartext login password. ([specs/lightware-lw3.yaml:4474](specs/lightware-lw3.yaml#L4474))
- [ ] Record the MX2 and UMX port status letters and bytes on a live port. ([specs/lightware-lw3.yaml:4418](specs/lightware-lw3.yaml#L4418))
- [ ] Check that MAN on a property or a method is answered with one line (pm, mm) on each tree, as get_manual expects, and that a method answered mO without = returns nothing. ([specs/lightware-lw3.yaml:4480](specs/lightware-lw3.yaml#L4480))
- [ ] On MX2, record the answer to applySettings (the manual prints m0 where other methods answer mO). ([specs/lightware-lw3.yaml:1984](specs/lightware-lw3.yaml#L1984))
- [ ] Check the /V1 GPIO pin range (P1-P6 assumed from the 8-pole connector; the manuals give no range) and that Direction and Output answer pw with = as in the examples. ([specs/lightware-lw3.yaml:1626](specs/lightware-lw3.yaml#L1626))
- [ ] On UMX, check the GPIO pins P1-P7, and that the UART's numbered ControlProtocol, Rs232Mode, Baudrate, StopBits and Parity codes are as translated. ([specs/lightware-lw3.yaml:4537](specs/lightware-lw3.yaml#L4537))
- [ ] On UMX-HDMI-140-Plus, record an Event Manager condition and action as GET answers them, and the RS-232 recognizer's Rx after a received message. ([specs/lightware-lw3.yaml:4537](specs/lightware-lw3.yaml#L4537))
- [ ] Check whether the /V1 LW3 service can be disabled over LW3 (SERVICES/LW3.Enabled is not documented) and whether set_web_service_enabled answers before the service stops. ([specs/lightware-lw3.yaml:1752](specs/lightware-lw3.yaml#L1752))
- [ ] On UCX and MMX2, check which remote syslog format property exists (UCX LogFormat, MMX2 SyslogFormat); it is left out. ([specs/lightware-lw3.yaml:1795](specs/lightware-lw3.yaml#L1795))
- [ ] Check that the reads on connecting (about 390 single GETs across the three trees, most answered with errors on the other trees) complete without the device dropping the connection. ([specs/lightware-lw3.yaml:2774](specs/lightware-lw3.yaml#L2774))
- [ ] On MX2, record the answers to PRESET:create and PRESET:rename, and whether GET /MEDIA/PRESET lists the presets as child nodes. ([specs/lightware-lw3.yaml:3406](specs/lightware-lw3.yaml#L3406))
- [ ] On MX2, confirm that port names cannot be set over LW3 (no property in the manual's LW3 chapter). ([specs/lightware-lw3.yaml:4523](specs/lightware-lw3.yaml#L4523))
- [ ] Drive an MX2 and a UMX-HDMI-140 (Control mode, LW3) on RS-232 (link serial) and confirm the same answers and CHG notifications as over TCP 6107, with no flow control. ([specs/lightware-lw3.yaml:161](specs/lightware-lw3.yaml#L161))
- [ ] Check when a Cleartext login on the RS-232 port ends (not documented) and that the login step works there. ([specs/lightware-lw3.yaml:4333](specs/lightware-lw3.yaml#L4333))

## logos-proclaim — Logos Proclaim

- [ ] Check what /appCommand/authenticate answers a wrong password with, and what /appCommand/perform answers an expired or wrong token with (401 or 403 is assumed for both), and whether a token expires. ([specs/logos-proclaim.yaml:314](specs/logos-proclaim.yaml#L314))
- [ ] Record /onair/session's answer on air and off air (an id as plain text, and an empty body, are assumed). ([specs/logos-proclaim.yaml:347](specs/logos-proclaim.yaml#L347))
- [ ] Record what /appCommand/perform answers for a command done, an unknown command name and an index out of range (status and body; the text success is assumed for done). ([specs/logos-proclaim.yaml:327](specs/logos-proclaim.yaml#L327))
- [ ] Check what a song part command does with an index past the song's parts, and that a part without a number is shown when the index is left out. ([specs/logos-proclaim.yaml:340](specs/logos-proclaim.yaml#L340))

## luminex-gigacore — Luminex GigaCore Gen2 switches (web API)

- [ ] Check the websocket API's notifications for a section subscription: that path is the subscribed path and new_value the whole section, and whether it needs the Basic login. ([specs/luminex-gigacore.yaml:4912](specs/luminex-gigacore.yaml#L4912))
- [ ] Check the write-only actions' bodies where the reference gives no default (reboot's wait) and what the switch answers to an empty one. ([specs/luminex-gigacore.yaml:77](specs/luminex-gigacore.yaml#L77))
- [ ] Check leaf PUT bodies (a bare JSON value such as "Port 3" or true) on firmware 1.9.2 and earlier. ([specs/luminex-gigacore.yaml:951](specs/luminex-gigacore.yaml#L951))
- [ ] Check every section's GET on models without the feature (PoE, SFP, Milan, display, extension board). ([specs/luminex-gigacore.yaml:59](specs/luminex-gigacore.yaml#L59))

## lyntec-rpc — LynTec RPC remote power control panels (TCP/IP protocol)

- [ ] Record the exact /p2.rpc answer (line breaks, spacing, the order of breakers=, zones= and events=), and what a control GET returns. ([specs/lyntec-rpc.yaml:43](specs/lyntec-rpc.yaml#L43))
- [ ] Check whether a panel whose web pages need a login asks for it on /p2.rpc too (Basic, Digest or none). ([specs/lyntec-rpc.yaml:28](specs/lyntec-rpc.yaml#L28))
- [ ] Check what a panel in Emergency Override answers a control GET with over HTTP. ([specs/lyntec-rpc.yaml:112](specs/lyntec-rpc.yaml#L112))
- [ ] Check zone numbers above 12 on panels with more zones, and breaker numbering across RPS expansion panels. ([specs/lyntec-rpc.yaml:125](specs/lyntec-rpc.yaml#L125))

## madrix — MADRIX 5 (OSC and HTTP Remote)

- [ ] Record the OSC feedback MADRIX sends: its addresses, argument types, and that it goes to the sender's address on the Destination Network Port. ([specs/madrix.yaml:1491](specs/madrix.yaml#L1491))
- [ ] Check whether /Main/FeedbackAll resends every value when sent by a client, and whether methods with no listed argument (Up, Down, Automatic, ToLeft, ToRight, ToCenter, Defaults, FeedbackAll) need one. ([specs/madrix.yaml:1500](specs/madrix.yaml#L1500))
- [ ] Check that layer, group, fader and trigger port indexes are written as plain decimals in the address (/DeckA/StoragePlace/Layers/0/Opacity). ([specs/madrix.yaml:1500](specs/madrix.yaml#L1500))
- [ ] Check the index bases (OSC 0-based, HTTP 1-based) and the bounds chosen where the help is silent (indexes, offsets, BPM). ([specs/madrix.yaml:1509](specs/madrix.yaml#L1509))
- [ ] Confirm an HTTP Remote Get function answers the value alone as the body (GET /RemoteCommands/?GetMaster), and what a Set function answers. ([specs/madrix.yaml:1516](specs/madrix.yaml#L1516))

## magewell-director — Magewell Director Mini, Director One and Director Plus

- [ ] Confirm the API listens on port 8080 (the document's demo port) and takes no authentication. ([specs/magewell-director.yaml:36](specs/magewell-director.yaml#L36))
- [ ] Confirm the get methods answer GET as the set methods do, and that their replies are the documented JSON. ([specs/magewell-director.yaml:3373](specs/magewell-director.yaml#L3373))
- [ ] Check the value formats the API accepts: true and false for booleans, decimals with two or three places, and the replay camera list [0,1,2,3] percent-encoded. ([specs/magewell-director.yaml:2017](specs/magewell-director.yaml#L2017))
- [ ] Confirm a refused action (content missing, action not allowed) answers HTTP 403, and what a 400 answer's body holds. ([specs/magewell-director.yaml:3787](specs/magewell-director.yaml#L3787))
- [ ] Confirm the scene list's order is the order switch_by_index counts from 1, and that the show list's switch_by_index counts from 0 as documented. ([specs/magewell-director.yaml:3401](specs/magewell-director.yaml#L3401))
- [ ] Check what a scoreboard command addresses with an ID but no scene ID, and with both, on global and private scoreboards. ([specs/magewell-director.yaml:582](specs/magewell-director.yaml#L582))
- [ ] Record the units of the BGM duration (documented as ms, 240 in the example), the phone camera's videoBitrate (documented as Mbps, 8192 in the example) and recording duration. ([specs/magewell-director.yaml:3673](specs/magewell-director.yaml#L3673))
- [ ] Confirm the phone camera's mute parameter and micMute field mean audio is captured when true, as the document says. ([specs/magewell-director.yaml:2683](specs/magewell-director.yaml#L2683))
- [ ] Confirm the Director Mini and One's USB-C output modes 0 to 5 match the Director Plus's numbering (mirror, program, loop HDMI 1 and 2, multiview, preview). ([specs/magewell-director.yaml:3252](specs/magewell-director.yaml#L3252))
- [ ] Check the meter levels' unit and refresh, and whether volume is whole dB. ([specs/magewell-director.yaml:3618](specs/magewell-director.yaml#L3618))

## magewell-proconvert — Magewell Pro Convert

- [ ] Confirm the HTTP port (80, or 443 with HTTPS), which no document states. ([spec](specs/magewell-proconvert.yaml#L186))
- [ ] Confirm the session cookie arrives in Set-Cookie on login for the encoders, the NDI decoders and the IP decoders (the IP decoder's reply also names it as sid, which the core no longer reads), and that every cookie the login sets may be sent back. ([spec](specs/magewell-proconvert.yaml#L213))
- [ ] Confirm status 37 is returned once the session has expired or the device restarted, and whether an expired session is ever answered with HTTP 401 instead (taken as an ended session too). ([spec](specs/magewell-proconvert.yaml#L215))
- [ ] Confirm a wrong password is answered with status 36 (16 for an unknown user on the IP decoders) and not with HTTP 401 or 403, which are taken as refusals too. ([spec](specs/magewell-proconvert.yaml#L218))
- [ ] Check the IP decoders' default credentials (Admin / Admin assumed). ([spec](specs/magewell-proconvert.yaml#L4782))
- [ ] Check whether Pro Convert for NDI to HDMI 4K runs Decoder API V1.3 or v1.1, and that the methods used here behave the same on both. ([spec](specs/magewell-proconvert.yaml#L404))
- [ ] Check that set_ndi_transport's four flags are accepted in one request, with tcp as all four false. ([spec](specs/magewell-proconvert.yaml#L4763))
- [ ] Confirm the IP decoder's summary reports hdmi-state as a number and the first stream of the current profile under profile.streams, as the document's example shows. ([spec](specs/magewell-proconvert.yaml#L3103))
- [ ] Confirm set-video-config takes in-auto-aspect, in-aspect-x and in-aspect-y, which get-video-config reports but no set-video-config example sends. ([spec](specs/magewell-proconvert.yaml#L739))
- [ ] Confirm out-color-fmt takes bt.601, bt.709 and bt.2020 as listed (the document's example sends rgb). ([spec](specs/magewell-proconvert.yaml#L755))
- [ ] Confirm out-fr-convertion takes raw, half, one-third and quarter as get-video-config lists them (the set-video-config example sends frame-rate-raw). ([spec](specs/magewell-proconvert.yaml#L776))
- [ ] Confirm reset-video-config answers with the reset fields (read into state as get-video-config is) and changes the picture at once. ([spec](specs/magewell-proconvert.yaml#L797))
- [ ] Confirm set-ptz-config accepts pelco-d and pelco-p (the parameter table says only VISCA for now) and each protocol's fields alone, as the examples send them. ([spec](specs/magewell-proconvert.yaml#L812))
- [ ] Confirm set_ptz_visca_udp's port is the camera's VISCA over IP port, and what visca-msg-hdr off sends. ([spec](specs/magewell-proconvert.yaml#L823))
- [ ] Confirm get-ptz-config reports focus-near-limit and focus-far-limit (described, but not in the example reply) and that the limits can be set without the protocol fields. ([spec](specs/magewell-proconvert.yaml#L843))
- [ ] Confirm set-edid-config accepts one option alone (the example sends all four). ([spec](specs/magewell-proconvert.yaml#L868))
- [ ] Confirm the decoder's set-audio-config takes one chN mapping alone, and that it applies only while channels is not 0. ([spec](specs/magewell-proconvert.yaml#L1088))
- [ ] Record which vu-meter-mode values the NDI decoders take and report (none, db, post-gain-db, post-gain-dbfs when read; none, dbu, dbvu, dbfs and post-gain- forms when set). ([spec](specs/magewell-proconvert.yaml#L1114))
- [ ] Confirm Sunday's mask in week-flags (the documents list Monday to Sunday as 1, 2, 4, 8, 16, 32, 0; 64 is assumed) and that the hour is UTC. ([spec](specs/magewell-proconvert.yaml#L1233))
- [ ] Confirm get-eth-status, get-ntp-server and get-net-access answer an administrator only, and what they answer another user. ([spec](specs/magewell-proconvert.yaml#L1263))
- [ ] Confirm set-eth-config takes name alone without changing the address settings. ([spec](specs/magewell-proconvert.yaml#L1268))
- [ ] Confirm set-eth-config with dhcp=false needs all four addresses, answers reconnect true, and moves the device at once. ([spec](specs/magewell-proconvert.yaml#L1285))
- [ ] Confirm switch-mode 3 (output off) turns the output off 30 seconds after the signal is lost, and that wake_screen wakes it. ([spec](specs/magewell-proconvert.yaml#L1472))
- [ ] Confirm /api/audio/config/set needs the whole channel-map array. ([spec](specs/magewell-proconvert.yaml#L1512))
- [ ] Confirm /api/settings/ndi/set accepts one field alone (its example sends every field) and leaves the others unchanged. ([spec](specs/magewell-proconvert.yaml#L1531))
- [ ] Record buffer-ms's range for discovered NDI sources (0 to 10000 assumed). ([spec](specs/magewell-proconvert.yaml#L1545))
- [ ] Confirm the IP decoders serve the base API's POST /api/system/info (its example is from another product) with the auto-reboot schedule and NTP server, and POST /api/system/auto-reboot and set-device-name. ([spec](specs/magewell-proconvert.yaml#L1583))
- [ ] Confirm get-ndi-config's subset per model: the encoder's example reply leaves out enable-rudp and enable-tcp, and ndi.transport reads tcp only when all four flags are reported off. ([spec](specs/magewell-proconvert.yaml#L3505))
- [ ] Confirm the IP decoder reports hdr-output-mode (its example reply shows hdr-output: false instead). ([spec](specs/magewell-proconvert.yaml#L3277))
- [ ] Confirm source ids are unique across the static and discovered NDI lists of /api/source/list, which share one state subtree. ([spec](specs/magewell-proconvert.yaml#L3596))
- [ ] Confirm the NDI decoders' summary reports ndi.connected (taken as ndi.enabled) and the encoders' ndi.enabled. ([spec](specs/magewell-proconvert.yaml#L3101))
- [ ] Confirm the 30-second settings reads (about 9 to 16 requests per device) do not disturb the encoder or decoder. ([spec](specs/magewell-proconvert.yaml#L2959))
- [ ] Confirm the AES67, Audio DX and IP to USB log in with POST /api/user/login and the SHA-256 of the password (IP to USB's login example shows a 32-digit hash), answer the session in Set-Cookie (sid, or sid-<serial number> on IP to USB), and use Admin / Admin by default. ([spec](specs/magewell-proconvert.yaml#L210))
- [ ] Confirm /api/aes67/general-settings and /api/aoip/general-apply accept one field alone (the documents give no request example) and leave the other settings unchanged. ([spec](specs/magewell-proconvert.yaml#L1689))
- [ ] Confirm the AES67 takes and reports unbld-in, unbld-out, bld-in and bld-out as the numbers its table lists (0 +12dBu ... 5 -10dBV; bld-in 0 +24dBu ... 6 -10dBV; bld-out 0 +18dBu ... 5 -10dBV), igmp as 0, 2 or 3 and audio-pattern as 0, 1 or 2. ([spec](specs/magewell-proconvert.yaml#L309))
- [ ] Confirm the Audio DX's text for -2 dB on the balanced connectors: general-range lists -2dBV, general-info's description -2dBu; and whether 0 dB is 0dBu or +0dBu. ([spec](specs/magewell-proconvert.yaml#L316))
- [ ] Confirm the Audio DX takes tx-sample-rate as text ("48000", as general-info reports it) and accepts 88200. ([spec](specs/magewell-proconvert.yaml#L1710))
- [ ] Confirm /api/aes67/ptp-settings accepts one field (or the two of set_ptp_priority and set_ptp_intervals) alone, as JSON numbers and text. ([spec](specs/magewell-proconvert.yaml#L1792))
- [ ] Confirm what *-apply does with a uid already in use (changes it, as on V1.0 firmware, or refuses) and with a new uid (adds it), for AES67 sources and the NDI, SRT and Icecast senders and receivers. ([spec](specs/magewell-proconvert.yaml#L1848))
- [ ] Confirm the update methods (V2.0) accept the uid and enable alone, and that source-update takes a plain object (its table names sources[i].uid, the other fields plainly). ([spec](specs/magewell-proconvert.yaml#L1862))
- [ ] Confirm sink-apply, sink-update and sink-del take their objects in a sinks array, as their tables (sinks[i].*) suggest, and what sink-apply needs for a sink set up from a discovered source. ([spec](specs/magewell-proconvert.yaml#L1882))
- [ ] Confirm sink-info's sources array lists the AES67 sources found on the network (SAP), as the spec reads it. ([spec](specs/magewell-proconvert.yaml#L3897))
- [ ] Confirm sink-statistics takes sink-idx in a JSON body and how it answers for a sink that is not set up. ([spec](specs/magewell-proconvert.yaml#L1903))
- [ ] Confirm V2.0 firmware takes the Icecast receiver's apply at POST /api/ices/receiver-apply (V1.0's path; V2.0's section 14.2 prints /api/live/receiver-select). ([spec](specs/magewell-proconvert.yaml#L2135))
- [ ] Confirm volume-mix's path is /api/aes67/volume-mix (V2.0's request line prints volume-tx under the volume-mix heading). ([spec](specs/magewell-proconvert.yaml#L2313))
- [ ] Confirm the matrix volumes take whole dB only (-36 to 36), and whether fractions are accepted. ([spec](specs/magewell-proconvert.yaml#L2300))
- [ ] Record the matrix's real TX and RX channel counts per model (the AES67 example reports 22 and 46, the Audio DX layout 32 and 32); the commands allow TX 0 to 31 and RX 0 to 63. ([spec](specs/magewell-proconvert.yaml#L2271))
- [ ] Confirm matrix-info's mix-state, rx-mute and rx-volume arrays are indexed by RX channel from 0 (the documents' examples leave them empty). ([spec](specs/magewell-proconvert.yaml#L4108))
- [ ] Confirm the summary's temperature is text with a unit ("79.61°C", as both examples show) and not a number. ([spec](specs/magewell-proconvert.yaml#L3757))
- [ ] Confirm the Dante state spells device.deivce-lock (as the example) and whether general-info's device-lock is the same flag. ([spec](specs/magewell-proconvert.yaml#L4212))
- [ ] Confirm if-info's interface types: the AES67 and Audio DX documents give 2 as a 4G module and 3 as USB, the IP to USB document 2 as USB sharing, 3 USB NET and 4 cellular. ([spec](specs/magewell-proconvert.yaml#L4360))
- [ ] Confirm /api/system/info's cpu.usage is the CPU load times 100 (2110 for 21.10%). ([spec](specs/magewell-proconvert.yaml#L3693))
- [ ] Confirm IP to USB's summary names the temperature cpu_temp (example) or cpu-temp (table); both are read. ([spec](specs/magewell-proconvert.yaml#L4407))
- [ ] Confirm IP to USB's stream status names the tally extra.tally_preview and extra.tally_program (the table prints tally-prview), and what the other state values mean. ([spec](specs/magewell-proconvert.yaml#L4495))
- [ ] Record the source list's default page size on IP to USB and whether one page holds every source; the core reads the first page only. ([spec](specs/magewell-proconvert.yaml#L2385))
- [ ] Confirm IP to USB's URL sources name the buffer buffer_ms (the list example) and not buffer-ms (the add table), and that add_source's PTZ settings read back as listed. ([spec](specs/magewell-proconvert.yaml#L4440))
- [ ] Confirm the no-signal upload's form fields: id and type beside the file part named file (the example shows only the file), and that a PNG of any size the device accepts is taken. ([spec](specs/magewell-proconvert.yaml#L2363))
- [ ] Confirm resource/image/get takes its id and type as a JSON POST body, and the ids the device has besides no_signal_1. ([spec](specs/magewell-proconvert.yaml#L2356))
- [ ] Confirm ndi/config/update needs enable with every request (its table marks it required) and accepts the three fields together. ([spec](specs/magewell-proconvert.yaml#L2346))
- [ ] Confirm set-dns accepts is-manual false alone, without dns1 and dns2. ([spec](specs/magewell-proconvert.yaml#L1669))
- [ ] Confirm set-date-time with ntp-enable false needs the time, in the device's local time, and accepts the NTP servers with it. ([spec](specs/magewell-proconvert.yaml#L1617))
- [ ] Confirm rx/live-apply adds a stream for a new uid and changes one for a uid in use, and that tx/live-apply does the same with the documents' stream objects. ([spec](specs/magewell-proconvert.yaml#L2195))
- [ ] Record the method that sets the USB audio channel count (uac-num-channels), which the AES67 document names /system/uac-format without describing it. ([spec](specs/magewell-proconvert.yaml#L4830))
- [ ] Record whether the Audio DX serves auto-reboot (POST /api/system/auto-reboot) and the certificate methods, whose document pages are empty. ([spec](specs/magewell-proconvert.yaml#L4836))
- [ ] Confirm the 2-second reads on the audio models (summary, PTP or Dante state, matrix meters) and the 30-second settings reads (about 20 requests on the AES67) do not disturb the audio. ([spec](specs/magewell-proconvert.yaml#L2950))
- [ ] Confirm the IP-to-USB source list numbers its pages from 1 and answers page with the device's default page_size, so asking for page 1 and each next page until total sources have arrived reads the whole list. ([specs/magewell-proconvert.yaml:2390](specs/magewell-proconvert.yaml#L2390))

## magewell-ultra-encode — Magewell Ultra Encode

- [ ] Confirm the default credentials Admin / Admin (the vendor's demos log in as Admin; no document states the default). ([spec](specs/magewell-ultra-encode.yaml#L89))
- [ ] Confirm the login answers the session in a Set-Cookie named sid on both APIs (the core sends back every cookie the login sets). ([spec](specs/magewell-ultra-encode.yaml#L149))
- [ ] Confirm an ended session (restart, idle) is answered with result -17, and whether it is ever answered with HTTP 401 instead (taken as an ended session too). ([spec](specs/magewell-ultra-encode.yaml#L151))
- [ ] Confirm what a wrong password answers: result 36 (login page) or -1 (status table), and -25 for an unknown user; HTTP 401 or 403 are taken as refusals too. ([spec](specs/magewell-ultra-encode.yaml#L154))
- [ ] Check HTTPS on the AIO and Plus models after set_web and generate_certificate (port 443 by default, self-signed certificate), and that the HDMI and SDI models serve HTTP only. ([spec](specs/magewell-ultra-encode.yaml#L130))
- [ ] Confirm the Control Hub API on port 8070 (/cloud-api) answers without the sid cookie, and that the HDMI and SDI models answer cloud-status under /usapi on port 80 while the AIO and Plus answer it on port 8070. ([spec](specs/magewell-ultra-encode.yaml#L140))
- [ ] Confirm every upload (EDID, no-signal JPEG, overlay image, certificate, private key, ZEN Master key, settings file, iCalendar file) takes multipart/form-data with the file in a part named file, as the demos upload firmware. ([spec](specs/magewell-ultra-encode.yaml#L2514))
- [ ] Check that a firmware upload (about 12 MB) is answered within the 10 second timeout. ([spec](specs/magewell-ultra-encode.yaml#L135))
- [ ] Confirm the HDMI and SDI models take set-volume gains in dB with two decimals (e.g. -6.50) and report them in dB in get-settings. ([spec](specs/magewell-ultra-encode.yaml#L578))
- [ ] Confirm the AIO and Plus models take and report set-volume gains in hundredths of a dB, as get-info's audio-range gives them (600 for +6 dB), where set-volume's page says dB. ([spec](specs/magewell-ultra-encode.yaml#L253))
- [ ] Confirm set-video's gop unit: seconds from get-info's gop-sec list (128 for half a second) on the HDMI and SDI models, while the AIO's get-info gives a GOP size range of 15 to 300 and its get-settings example a gop of 60. ([spec](specs/magewell-ultra-encode.yaml#L879))
- [ ] Confirm set-video takes rotation as 0, 90, 180 or 270 (its page) rather than 0 to 3 (get-info's video-rotation list), and what get-settings reports. ([spec](specs/magewell-ultra-encode.yaml#L889))
- [ ] Confirm get-status lists live sessions under live-status.live (the page shows both that and a bare array), and that the HDMI and SDI models' entries, which carry no id, follow get-settings' stream-server order. ([spec](specs/magewell-ultra-encode.yaml#L3299))
- [ ] Confirm enable_server starts streaming to that server at once on the HDMI and SDI models, which have no start-live. ([spec](specs/magewell-ultra-encode.yaml#L1534))
- [ ] Confirm start-live and start-rec (AIO and Plus) start every enabled streaming server and recording channel. ([spec](specs/magewell-ultra-encode.yaml#L495))
- [ ] Confirm get-rec-channels' rec-channels entries carry id, is-use and the add-rec-channel field names (type, stream-index, mode, dir-name, file-prefix, prefix-name, file-suffix, time-unit, audio); the document shows the array empty. ([spec](specs/magewell-ultra-encode.yaml#L3942))
- [ ] Confirm get-surfaces' surfaces entries carry id, is-use and the add-surface field names; the document shows the array empty. ([spec](specs/magewell-ultra-encode.yaml#L3965))
- [ ] Confirm get-settings' send-file-cloud entries carry id, type, is-use and the send-file-add-server field names; the document's example list is empty. ([spec](specs/magewell-ultra-encode.yaml#L3850))
- [ ] Confirm add-surface and set-surface take the overlay kind as type (example URL, get-info) and not class (their tables). ([spec](specs/magewell-ultra-encode.yaml#L2029))
- [ ] Confirm add-scheduler numbers repeat-event from 0 (daily) and set-scheduler from 1 (0 no repeat), that the weekly interval is repeat-interval (the tables write repeat-is-interval), and that every repeat field may be sent whatever the pattern. ([spec](specs/magewell-ultra-encode.yaml#L2199))
- [ ] Confirm set-lcd-control's polarity: the page gives no-touch 1 as touch enabled and no-flip 1 as page flipping enabled. ([spec](specs/magewell-ultra-encode.yaml#L771))
- [ ] Check which input set-video-color changes on the AIO and Plus models, whose page takes no input-device while get-settings reports the color per input. ([spec](specs/magewell-ultra-encode.yaml#L606))
- [ ] Check where get-settings reports set-ndi-bridge-config, set-zen-master-config, set-live-keep-last, set-ndi-find-config, set-audio-mixer-config, enable-4k60-input, set-audio-stream-count, set-crop, select-audio-channels, set-net-prior, enable-station and send-file-set-is-auto, which no document shows read back. ([spec](specs/magewell-ultra-encode.yaml#L4126))
- [ ] Check that the Plus models (and Modules) refuse select-input-source, set-video-mixer-config and set-lcd-control (marked AIO only on the Plus API's pages) and take set-3d-output-config, and that the Modules behave as the boxed units. ([spec](specs/magewell-ultra-encode.yaml#L297))
- [ ] Check that the SDI and SDI Plus models have no EDID methods (the EDID commands are offered on the HDMI, HDMI Plus and AIO only). ([spec](specs/magewell-ultra-encode.yaml#L2490))
- [ ] Confirm add-server and set-server accept empty optional fields (stream-id, token, event-data, discovery-server, the multicast and failover fields) as empty strings. ([spec](specs/magewell-ultra-encode.yaml#L1097))
- [ ] Confirm the PID fields are ignored while is-custom-pid is 0 (the core sends 4096, 256, 256 and 257 by default). ([spec](specs/magewell-ultra-encode.yaml#L1106))
- [ ] Confirm export-settings answers the configuration file itself as the reply body (returned as text). ([spec](specs/magewell-ultra-encode.yaml#L455))
- [ ] Confirm get-status' cpu-temperature is in thousandths of a degree Celsius (61100 in the example). ([spec](specs/magewell-ultra-encode.yaml#L256))
- [ ] Confirm the status mask's 0x02 (recording), 0x04 (streaming) and 0x10 (disk ready) bits, which the documents mark reserved (the AIO's disk pages test 0x02 as recording). ([spec](specs/magewell-ultra-encode.yaml#L2606))
- [ ] Confirm set-softap accepts a changed softap-ssid, documented as reserved. ([spec](specs/magewell-ultra-encode.yaml#L545))
- [ ] Confirm send-file-add-server and send-file-set-server accept parents and privacy (empty) for every cloud service. ([spec](specs/magewell-ultra-encode.yaml#L1864))
- [ ] Confirm set-date-time takes the manual time with its space percent-encoded (%20; the example writes +). ([spec](specs/magewell-ultra-encode.yaml#L702))
- [ ] Confirm Wowza over RTMP (4) and YouTube HLS (143) take the RTMP parameter list. ([spec](specs/magewell-ultra-encode.yaml#L988))
- [ ] Check the Zixi and RIST latency, FEC overhead and bandwidth ranges, which get-info's example does not give (0 to 100000 and 0 to 100 % allowed here). ([spec](specs/magewell-ultra-encode.yaml#L1393))
- [ ] Check the RTSP, SRT and RIST listeners' max-connection range (1 to 255 allowed here). ([spec](specs/magewell-ultra-encode.yaml#L1039))
- [ ] Check the scale of get-status' vumeters (33 and 32 in the example). ([spec](specs/magewell-ultra-encode.yaml#L2718))
- [ ] Confirm send-file-get-status with count 0 answers the totals (the core polls disk-type 1, start 0, count 0). ([spec](specs/magewell-ultra-encode.yaml#L3018))
- [ ] Confirm set-video-mixer-config's location numbering: corners 1 to 4 for picture in picture, splits 1 to 5 for side by side. ([spec](specs/magewell-ultra-encode.yaml#L855))
- [ ] Check the overlay font-color and line-color format (e.g. #ffffff), which no document gives. ([spec](specs/magewell-ultra-encode.yaml#L2023))
- [ ] Confirm get-schedulers with id -1 and chn-id -1 answers every schedule of the type, and the schedulers entries' fields. ([spec](specs/magewell-ultra-encode.yaml#L3031))
- [ ] Confirm get-status' disk-info and nas are arrays (the get-status page shows them as objects, the disk pages as arrays) with disk-type, disk-status, total-size and used-size. ([spec](specs/magewell-ultra-encode.yaml#L3344))
- [ ] Confirm get-signal-info on the HDMI Plus answers input-device 2 and on the SDI Plus input-device 1 (each polls only its own input). ([spec](specs/magewell-ultra-encode.yaml#L3016))
- [ ] Confirm upload-update-file answers status (not result), as its page shows. ([spec](specs/magewell-ultra-encode.yaml#L2452))

## magewell-ultra-stream — Magewell Ultra Stream

- [ ] Confirm the API is on port 80 over HTTP only (the document's URLs give no port and no HTTPS). ([spec](specs/magewell-ultra-stream.yaml#L83))
- [ ] Confirm the session ID arrives as the sid cookie in Set-Cookie on login, and that sending back every cookie the login set is accepted. ([spec](specs/magewell-ultra-stream.yaml#L97))
- [ ] Confirm an expired or missing session is answered with result -17 (authentication required), and whether it is ever answered with HTTP 401 instead (taken as an ended session too). ([spec](specs/magewell-ultra-stream.yaml#L99))
- [ ] Confirm a wrong password is answered with result 36 (the login page) rather than -1 (errPasswd), and an unknown user with -25, and not with HTTP 401 or 403, which are taken as refusals too. ([spec](specs/magewell-ultra-stream.yaml#L102))
- [ ] Check the default credentials (Admin / Admin, from the vendor's demos). ([spec](specs/magewell-ultra-stream.yaml#L61))
- [ ] Confirm the Cloud API on port 8070 answers without the session cookie, which the core does not send there. ([spec](specs/magewell-ultra-stream.yaml#L90))
- [ ] Confirm ping answers without a login and carries cur-status. ([spec](specs/magewell-ultra-stream.yaml#L205))
- [ ] Confirm get-report answers HTML with HTTP 200 and no JSON result. ([spec](specs/magewell-ultra-stream.yaml#L230))
- [ ] Confirm start-live with HEVC on the main stream answers -32, and a repeated stop-live answers 1. ([spec](specs/magewell-ultra-stream.yaml#L245))
- [ ] Check that set-first-over takes timezone, is-auto and time beside name and enable-passwd (its examples leave them out), and an empty time with automatic time. ([spec](specs/magewell-ultra-stream.yaml#L309))
- [ ] Check set-rec-control: file-name-mode as 0 (custom prefix) or 1 (time), mime-type 0/1, and that no parameter sets the custom prefix text (file-prefix). ([spec](specs/magewell-ultra-stream.yaml#L330))
- [ ] Check that set-softap accepts is-softap, is-visible and softap-ssid (documented as reserved) together with the password, and what is-softap 0 does. ([spec](specs/magewell-ultra-stream.yaml#L348))
- [ ] Check that set-date-time accepts the time with the space percent-encoded (%20) rather than as + in the document's example. ([spec](specs/magewell-ultra-stream.yaml#L358))
- [ ] Check set-volume takes gains with two decimals, and the gain ranges on the SDI model (get-info's audio-range, named hdmi there too). ([spec](specs/magewell-ultra-stream.yaml#L388))
- [ ] Confirm set-auto-live locks live streaming as described (it starts by itself and stop-live answers -43). ([spec](specs/magewell-ultra-stream.yaml#L417))
- [ ] Confirm add-nosignal-file takes a multipart/form-data part named file (as the demo uploads firmware) with Content-Type image/jpeg. ([spec](specs/magewell-ultra-stream.yaml#L453))
- [ ] Confirm del-media-files, send-file-add and send-file-del take standard JSON with quoted keys (the document's examples leave them unquoted). ([spec](specs/magewell-ultra-stream.yaml#L533))
- [ ] Check the values YouTube upload's privacy takes (the document names the field only). ([spec](specs/magewell-ultra-stream.yaml#L587))
- [ ] Check what send-file-get-status's status and media-files hold, and what count 0 returns. ([spec](specs/magewell-ultra-stream.yaml#L714))
- [ ] Check that add-server with type 100 takes the RTSP fields without url and key, and that get-settings reports port, is-main, main-stream-name, is-sub, sub-stream-name, is-audio and event-data for each server. ([spec](specs/magewell-ultra-stream.yaml#L764))
- [ ] Check set-video with is-auto 1 (follow the input) still needs every other field, and that the sub stream accepts codec 0 and is-auto 0 as sent. ([spec](specs/magewell-ultra-stream.yaml#L870))
- [ ] Confirm logout ends the session and the next request is answered -17, so the core logs in again. ([spec](specs/magewell-ultra-stream.yaml#L933))
- [ ] Check that set-net with is-dhcp 1 is accepted without ip, mask, router and dns, for type 1 (Ethernet) and 0 (Wi-Fi). ([spec](specs/magewell-ultra-stream.yaml#L978))
- [ ] Confirm upload-update-file takes a multipart/form-data part named file (the vendor's demo) and answers status, not result. ([spec](specs/magewell-ultra-stream.yaml#L1021))
- [ ] Check which models answer the EDID methods (the SDI model is assumed to have none). ([spec](specs/magewell-ultra-stream.yaml#L1099))
- [ ] Confirm import-edid takes a multipart/form-data part named file. ([spec](specs/magewell-ultra-stream.yaml#L1119))
- [ ] Check the units of sysstat (cpu, memory, network and storage rates, uptime). ([spec](specs/magewell-ultra-stream.yaml#L1166))
- [ ] Check what input-signal's status and channel-valid mean. ([spec](specs/magewell-ultra-stream.yaml#L1183))
- [ ] Confirm the storage status codes (0 none, 1 valid, 3 full, 4 read-only, 5 low space, 6 invalid) are reported as values, not as bit flags. ([spec](specs/magewell-ultra-stream.yaml#L1242))
- [ ] Check the unit of a recording's duration in get-media-files (documented as seconds; the example's 14480 for a 4 MB file suggests milliseconds). ([spec](specs/magewell-ultra-stream.yaml#L1462))
- [ ] Confirm get-media-files accepts count 100, and with no storage device answers an error rather than an empty list. ([spec](specs/magewell-ultra-stream.yaml#L1486))
- [ ] Check that get-settings' stream-server entries report is-use and net-mode, as the example shows. ([spec](specs/magewell-ultra-stream.yaml#L1910))
- [ ] Check that get-settings' send-file-cloud entries for FTP servers report proto, url, port, encryption, login-type and mode, and for YouTube privacy. ([spec](specs/magewell-ultra-stream.yaml#L1947))

## matrox-monarch-hd — Matrox Monarch HD

- [ ] Confirm the Control API accepts an HTTP Basic credential (the guide only shows `user:password@host`, wget and .NET `NetworkCredential`), or whether it challenges with Digest. ([specs/matrox-monarch-hd.yaml:48](specs/matrox-monarch-hd.yaml#L48))
- [ ] Confirm HTTPS for the Control API is served on port 443 with the unit's self-issued certificate (the guide only says HTTPS is "also supported"). ([specs/matrox-monarch-hd.yaml:43](specs/matrox-monarch-hd.yaml#L43))
- [ ] Confirm SetRTMP takes `SetRTMP,<url>,<stream>[,<username>,<password>]` with no spaces after the commas (the guide writes the arguments with spaces and without the command name). ([specs/matrox-monarch-hd.yaml:199](specs/matrox-monarch-hd.yaml#L199))
- [ ] Confirm the unit percent-decodes query arguments (stream name, RTMP username and password are sent percent-encoded, a space as %20). ([specs/matrox-monarch-hd.yaml:213](specs/matrox-monarch-hd.yaml#L213))
- [ ] Record the exact text of GetStatus replies (spacing after commas, line ending) and check the status rule reads them. ([specs/matrox-monarch-hd.yaml:270](specs/matrox-monarch-hd.yaml#L270))
- [ ] Record GetRTSP's reply ("URL, name, port") and check it is three comma-separated fields: full URL, stream name, port. ([specs/matrox-monarch-hd.yaml:306](specs/matrox-monarch-hd.yaml#L306))
- [ ] Record GetRTMP's reply ("Url name, stream name") and check it is two comma-separated fields. ([specs/matrox-monarch-hd.yaml:312](specs/matrox-monarch-hd.yaml#L312))
- [ ] Record GetInputStatus replies for several inputs (interlaced, progressive, fractional and whole rates) and "No video input", and check the parse into connector, size, scan and rate. ([specs/matrox-monarch-hd.yaml:290](specs/matrox-monarch-hd.yaml#L290))
- [ ] Record GetSerialNumber's reply and confirm it is the serial alone and holds a digit (the rule skips replies without one, such as RETRY). ([specs/matrox-monarch-hd.yaml:320](specs/matrox-monarch-hd.yaml#L320))
- [ ] Check what GetStreamingVideoDataRate answers outside Stream-only mode (documented as FAILED) and that the last bit rate staying in state is acceptable. ([specs/matrox-monarch-hd.yaml:166](specs/matrox-monarch-hd.yaml#L166))
- [ ] Check how often RETRY comes back with the 2-second GetStatus poll while the Command Center is open. ([specs/matrox-monarch-hd.yaml:377](specs/matrox-monarch-hd.yaml#L377))

## matrox-monarch-hdx — Matrox Monarch HDX

- [ ] Confirm the Control API accepts an HTTP Basic credential (the guide only shows `user:password@host`, wget and .NET `NetworkCredential`), or whether it challenges with Digest. ([specs/matrox-monarch-hdx.yaml:48](specs/matrox-monarch-hdx.yaml#L48))
- [ ] Confirm HTTPS for the Control API is served on port 443 with the unit's self-issued certificate (the guide only says HTTPS is "also supported"). ([specs/matrox-monarch-hdx.yaml:43](specs/matrox-monarch-hdx.yaml#L43))
- [ ] Confirm SetRTMP takes `SetRTMP,<encoder>,<url>,<stream>[,<username>,<password>]` with no spaces after the commas (the guide writes the arguments with spaces and without the command name). ([specs/matrox-monarch-hdx.yaml:195](specs/matrox-monarch-hdx.yaml#L195))
- [ ] Confirm the unit percent-decodes query arguments (stream name, RTMP username and password are sent percent-encoded). ([specs/matrox-monarch-hdx.yaml:210](specs/matrox-monarch-hdx.yaml#L210))
- [ ] Record the exact text of GetStatus replies (spacing after commas, line ending) and check the status rule reads them. ([specs/matrox-monarch-hdx.yaml:297](specs/matrox-monarch-hdx.yaml#L297))
- [ ] Record GetRTSP,<n>'s reply ("URL, name, port") and check it is three comma-separated fields. ([specs/matrox-monarch-hdx.yaml:340](specs/matrox-monarch-hdx.yaml#L340))
- [ ] Record GetRTMP,<n>'s reply ("Url name, stream name") and check it is two comma-separated fields. ([specs/matrox-monarch-hdx.yaml:346](specs/matrox-monarch-hdx.yaml#L346))
- [ ] Record GetInputStatus replies for SDI and HDMI, with and without closed captions, and "No video input", and check the parse into connector, size, scan, rate and captions. ([specs/matrox-monarch-hdx.yaml:318](specs/matrox-monarch-hdx.yaml#L318))
- [ ] Confirm GetAudioInput's reply ends with a period as the guide prints it ("AUDIO INPUT: ANALOG."), or not; the rule takes both. ([specs/matrox-monarch-hdx.yaml:357](specs/matrox-monarch-hdx.yaml#L357))
- [ ] Check how often RETRY comes back with the 2-second GetStatus poll while the Command Center is open. ([specs/matrox-monarch-hdx.yaml:423](specs/matrox-monarch-hdx.yaml#L423))

## matrox-monarch-lcs — Matrox Monarch LCS

- [ ] Confirm the Control API accepts an HTTP Basic credential (the guide only shows `user:password@host`, wget and .NET `NetworkCredential`), or whether it challenges with Digest. ([specs/matrox-monarch-lcs.yaml:49](specs/matrox-monarch-lcs.yaml#L49))
- [ ] Confirm HTTPS for the Control API is served on port 443 with the unit's self-issued certificate (the guide only says HTTPS is "also supported"). ([specs/matrox-monarch-lcs.yaml:44](specs/matrox-monarch-lcs.yaml#L44))
- [ ] Confirm SetRTMP takes `SetRTMP,<encoder>,<url>,<stream>[,<username>,<password>]` with no spaces after the commas. ([specs/matrox-monarch-lcs.yaml:190](specs/matrox-monarch-lcs.yaml#L190))
- [ ] Confirm SetBothRtmpPaths without credentials is `SetBothRtmpPaths,<url1>,<stream1>,<url2>,<stream2>` (the guide marks usernames and passwords optional but shows only the full form). ([specs/matrox-monarch-lcs.yaml:229](specs/matrox-monarch-lcs.yaml#L229))
- [ ] Confirm the unit percent-decodes query arguments (stream names, credentials, the transfer destination and domain are sent percent-encoded; a UNC backslash as %5C). ([specs/matrox-monarch-lcs.yaml:328](specs/matrox-monarch-lcs.yaml#L328))
- [ ] Confirm the crop argument of SetOperatingMode is accepted as TRUE or FALSE in upper case, as the guide writes it. ([specs/matrox-monarch-lcs.yaml:377](specs/matrox-monarch-lcs.yaml#L377))
- [ ] Record the exact text of GetStatus replies and find out what FILETRANSFER:ON means (a transfer running, or the feature enabled). ([specs/matrox-monarch-lcs.yaml:425](specs/matrox-monarch-lcs.yaml#L425))
- [ ] Record GetRTSP,<n> and GetRTMP,<n> replies and check they are the comma-separated fields the rules read. ([specs/matrox-monarch-lcs.yaml:468](specs/matrox-monarch-lcs.yaml#L468))
- [ ] Record GetInputStatus replies in each operating mode, including one input without a signal, and check the per-input parse (inputs A and B separated by ";"). ([specs/matrox-monarch-lcs.yaml:446](specs/matrox-monarch-lcs.yaml#L446))
- [ ] Record GetVideoInput's reply outside Single-isolated mode (the guide says input A's connector "and INPUT_B_HDMI") and GetAudioInput's (whether it names INPUT_A_SDI/HDMI or INPUT_A, and any final period). ([specs/matrox-monarch-lcs.yaml:482](specs/matrox-monarch-lcs.yaml#L482))
- [ ] Check how often RETRY comes back with the 2-second GetStatus poll while the Command Center is open. ([specs/matrox-monarch-lcs.yaml:566](specs/matrox-monarch-lcs.yaml#L566))

## mediamtx — MediaMTX

- [ ] Confirm a wrong Basic user or password, or an invalid JWT, is answered 401 (not 403) by the Control API, so it is taken as the terminal refusal. ([specs/mediamtx.yaml:129](specs/mediamtx.yaml#L129))
- [ ] Confirm the Control API takes a JWT as Authorization: Bearer (release notes 1.9.2) and, as the HTTP servers do, user:pass as a Bearer token. ([specs/mediamtx.yaml:129](specs/mediamtx.yaml#L129))
- [ ] Confirm GET /v3/paths/list?itemsPerPage=1 as the probe answers a client allowed the api action and 401 to any other. ([specs/mediamtx.yaml:139](specs/mediamtx.yaml#L139))
- [ ] Confirm apiEncryption: yes serves HTTPS on the same port 9997 with the configured certificate. ([specs/mediamtx.yaml:124](specs/mediamtx.yaml#L124))
- [ ] Confirm a path name holding a slash (live/stage) is reached percent-encoded (live%2Fstage) in /v3/config/paths/*/{name}, /v3/paths/get/{name}, /v3/hls/muxers/get/{name} and /v3/recordings/get/{name}. ([specs/mediamtx.yaml:336](specs/mediamtx.yaml#L336))
- [ ] Confirm configuration changes made through the API are not written to mediamtx.yml, are lost on restart, and are undone by a later edit of the file (not documented). ([specs/mediamtx.yaml:890](specs/mediamtx.yaml#L890))
- [ ] Record what get_global_config and get_path_config return in place of passwords and passphrases from 1.20.1 (left out, empty or masked), and whether patching other fields keeps them. ([specs/mediamtx.yaml:915](specs/mediamtx.yaml#L915))
- [ ] Confirm set_server_enabled turns a server on and off at runtime without restarting MediaMTX, and that releases before 1.19 answer 400 for moq. ([specs/mediamtx.yaml:266](specs/mediamtx.yaml#L266))
- [ ] Confirm replace_path_config adds a path that does not exist (1.8.2) and resets the fields it leaves out to the path defaults. ([specs/mediamtx.yaml:387](specs/mediamtx.yaml#L387))
- [ ] Confirm delete_path_config closes the path's source and disconnects its readers at once. ([specs/mediamtx.yaml:398](specs/mediamtx.yaml#L398))
- [ ] Confirm kicking a publishing session or connection takes the path offline for its readers, and whether the client may reconnect at once. ([specs/mediamtx.yaml:581](specs/mediamtx.yaml#L581))
- [ ] Confirm set_path_record_delete_after accepts the d unit (1d, 7d) as the configuration file does. ([specs/mediamtx.yaml:448](specs/mediamtx.yaml#L448))
- [ ] Confirm delete_recording_segment needs the start exactly as the recordings list gives it (fraction and offset), and answers 404 for another time. ([specs/mediamtx.yaml:835](specs/mediamtx.yaml#L835))
- [ ] Confirm get_static_source answers 404 for a path fed by a publisher. ([specs/mediamtx.yaml:534](specs/mediamtx.yaml#L534))
- [ ] Confirm list and get endpoints answer {"status": "ok"}-style JSON on success from 1.15.5 (the then_send re-reads match it) and record what 1.2 to 1.15.4 answer to a successful patch or kick. ([specs/mediamtx.yaml:1556](specs/mediamtx.yaml#L1556))
- [ ] Confirm pageCount is 0 for an empty list and 1 for a list on one page (whole-list replacement relies on it). ([specs/mediamtx.yaml:1367](specs/mediamtx.yaml#L1367))
- [ ] Confirm what the session and connection lists answer while their server is disabled (rtsp: no and so on), and that releases before 1.18 answer 404 to /v3/hlssessions/list (before 1.19 to /v3/moqsessions/list) without other effect. ([specs/mediamtx.yaml:1151](specs/mediamtx.yaml#L1151))
- [ ] Confirm 1.21 still answers the earlier endpoint names (release notes 1.21.0), and that the deprecated bytesReceived and bytesSent hold the same values as inboundBytes and outboundBytes from 1.17. ([specs/mediamtx.yaml:1333](specs/mediamtx.yaml#L1333))
- [ ] Confirm /v3 is served from 1.2.0 (the OpenAPI files show /v2 at 1.1.0 and /v3 at 1.2.0) and that each endpoint first appears in the release this spec names. ([specs/mediamtx.yaml:198](specs/mediamtx.yaml#L198))
- [ ] Record the transport strings an RTSP session reports (such as UDP, TCP, UDP-multicast). ([specs/mediamtx.yaml:1064](specs/mediamtx.yaml#L1064))
- [ ] Confirm the playback server's /list answers a client without credentials under the default configuration (the any user's playback permission), 401 when the path's playback is restricted, and takes the API's Basic or Bearer credential with playback_auth inherit. ([specs/mediamtx.yaml:163](specs/mediamtx.yaml#L163))
- [ ] Confirm playbackEncryption: yes serves HTTPS on the playback port with the configured certificate. ([specs/mediamtx.yaml:133](specs/mediamtx.yaml#L133))
- [ ] Confirm /list answers 404 for a path with no recording in the range (1.11.1), and record what releases 1.5.1 to 1.11.0 answer to an empty list and to start and end (filters from 1.11.0). ([specs/mediamtx.yaml:856](specs/mediamtx.yaml#L856))
- [ ] Confirm a path name holding a slash is found when percent-encoded in /list's path query (live%2Fstage). ([specs/mediamtx.yaml:849](specs/mediamtx.yaml#L849))
- [ ] Check that /v3/recordings/list lists one item per path with its segments in time order, that a path whose segments are all deleted leaves the list, and that a segment's start is the text delete_recording_segment takes. ([specs/mediamtx.yaml:1292](specs/mediamtx.yaml#L1292))
- [ ] Check whether /v3/config/paths/list reports a pull source's URL with its credentials, or masks them as other secrets are from 1.20.1. ([specs/mediamtx.yaml:1258](specs/mediamtx.yaml#L1258))
- [ ] Check that a span's duration grows while the path records, and that reading every recording's spans every 30 seconds does not burden a server with many recordings. ([specs/mediamtx.yaml:1315](specs/mediamtx.yaml#L1315))

## megapixel-helios — Megapixel HELIOS

- [ ] Confirm that the JSON-RPC websocket answers {"jsonrpc":"2.0","id":1,"method":"state"} with the whole tree under result, then pushes updates without a subscription, and that it takes the web application's credentials when authentication is on. ([specs/megapixel-helios.yaml:520](specs/megapixel-helios.yaml#L520))
- [ ] Check that basic access is accepted when authentication is on, or that the processor's digest challenge is answered. ([specs/megapixel-helios.yaml:83](specs/megapixel-helios.yaml#L83))
- [ ] Confirm that upload_still is answered 200 with the new still at the top level ($.id), as 3.4.5 shows, and which image types and sizes the still store accepts. ([specs/megapixel-helios.yaml:632](specs/megapixel-helios.yaml#L632))
- [ ] Confirm that a PATCH out of range answers 200 with the unchanged value, as the document says, on current firmware. ([specs/megapixel-helios.yaml:2844](specs/megapixel-helios.yaml#L2844))
- [ ] Record the ranges of display gains, output adjustment gain, gamma, lift, offset and saturation, which the document does not give. ([specs/megapixel-helios.yaml:199](specs/megapixel-helios.yaml#L199))
- [ ] Confirm that a group's gains are keyed r, g, b and i, its mask l, t, b and r, and its test pattern colour r, g, b and a, and record their ranges (0-1 or 0-255). ([specs/megapixel-helios.yaml:362](specs/megapixel-helios.yaml#L362))
- [ ] Confirm that blackClipping can be written (it is in the processor's data and fixtures, not the API tables). ([specs/megapixel-helios.yaml:191](specs/megapixel-helios.yaml#L191))
- [ ] Check whether redundancy mode accepts manual and single, which sys.fixtures lists beside none, failover and seamless. ([specs/megapixel-helios.yaml:286](specs/megapixel-helios.yaml#L286))
- [ ] List the test pattern type names the processor accepts. ([specs/megapixel-helios.yaml:2917](specs/megapixel-helios.yaml#L2917))
- [ ] Check that a receiver's x, y and groupId can be written by MAC address. ([specs/megapixel-helios.yaml:426](specs/megapixel-helios.yaml#L426))
- [ ] Record the status of a successful preset apply (200 is assumed) and of hide_still (204 is assumed, as for show). ([specs/megapixel-helios.yaml:458](specs/megapixel-helios.yaml#L458))

## meros-camera — Meros Camera

- [ ] Confirm that an output with a problem (outputs.problems) is not listed in outputs.on. ([specs/meros-camera.yaml:660](specs/meros-camera.yaml#L660))
- [ ] Check that signalDbm is a whole number on a phone, as battery and freeBytes are. ([specs/meros-camera.yaml:433](specs/meros-camera.yaml#L433))
- [ ] On a phone, confirm the event stream's first line arrives at once and a line follows at least every 5 s while nothing changes, so the driver's 10 s silence check never drops a healthy stream, and that a stream ended by the camera is reopened after the status is read again. ([specs/meros-camera.yaml:433](specs/meros-camera.yaml#L433))
- [ ] Confirm that the battery reads -1 while unknown (no battery_low event is sent from -1), and that warnings and error are plain sentences the derived warning and error events can repeat. ([specs/meros-camera.yaml:451](specs/meros-camera.yaml#L451))
- [ ] On a phone, confirm the status carries monitoring and outputSettings whole, changed on the event stream when the operator or a preset changes them, and outputSettings never with a key, token or passphrase (keySet and tokenSet only; the driver drops any it is sent). ([specs/meros-camera.yaml:433](specs/meros-camera.yaml#L433))
- [ ] On a phone, confirm the capabilities answer carries limits with the same ranges the app refuses and clamps to (camera-sim's are §7.6's, and the driver checks against the ones it receives). ([specs/meros-camera.yaml:678](specs/meros-camera.yaml#L678))
- [ ] On a phone, confirm the status's outputSettings carry omt.quality, changed on the event stream when the operator changes it, and whether changing the quality of a running OMT output restarts its stream (camera-sim only stores it). ([specs/meros-camera.yaml:361](specs/meros-camera.yaml#L361))
- [ ] Confirm that a lens's videoFormats (fps and manualFps) and highSpeedFormats list every size and rate the camera records, and that exposureTimeNs bounds the shutter at every rate, since the driver refuses anything outside them before sending. ([specs/meros-camera.yaml:507](specs/meros-camera.yaml#L507))
- [ ] On a phone, measure the preview's keyframes in wall-clock time: at least once a second, the first frame a keyframe, and one at once when a viewer joins or falls behind. camera-sim keeps them within a second of its own timestamps, but sends about 10 of its 25 frames a second, so they arrive up to 1.3 s apart. ([specs/meros-camera.yaml:523](specs/meros-camera.yaml#L523))
- [ ] Confirm customGuide is a number (width divided by height, 0.25 to 4) as the app reads it, where §7.4 writes width:height. ([specs/meros-camera.yaml:289](specs/meros-camera.yaml#L289))
- [ ] Confirm the clip server's ranges: a range ending past the file is cut at its last byte (206), a range starting at or past the end answers 416 with Content-Range bytes */<size>, and a slate sidecar (.json) is read the same way. ([specs/meros-camera.yaml:96](specs/meros-camera.yaml#L96))
- [ ] On a network with several cameras, confirm each announces _meros-cam._tcp with TXT id, name, model and proto and its port in SRV, that a camera whose address changes announces the new A record with the cache-flush bit, and that turning remote control off sends a goodbye. ([specs/meros-camera.yaml:52](specs/meros-camera.yaml#L52))
- [ ] Confirm that a camera given a new fleet key answers 401 to the old one at once, including on an event stream or preview already open. ([specs/meros-camera.yaml:462](specs/meros-camera.yaml#L462))
- [ ] Confirm that settings.exposure.iso is null (absent from state) in automatic exposure, and which settings fields the camera writes as null in other modes. ([specs/meros-camera.yaml:596](specs/meros-camera.yaml#L596))
- [ ] On a phone with talkback on, confirm that get_talkback_url's address takes a director's WebRTC offer, and that new_talkback_address cuts off a director on the old one. ([specs/meros-camera.yaml:536](specs/meros-camera.yaml#L536))

## meyersound-galaxy — Meyer Sound GALAXY

- [ ] Record the exact answer forms: whether get answers are quoted (path='value') or not, whether a set is answered at all, and the error text for a status point or an unknown path. ([specs/meyersound-galaxy.yaml:45](specs/meyersound-galaxy.yaml#L45))
- [ ] Confirm that subscribing with a regular expression per subtree is accepted, sends every current value, and that the device keeps up with every point subscribed at 100 ms. ([specs/meyersound-galaxy.yaml:444](specs/meyersound-galaxy.yaml#L444))
- [ ] Check how create_snapshot separates a name with spaces from the comment (the guide gives no quoting). ([specs/meyersound-galaxy.yaml:368](specs/meyersound-galaxy.yaml#L368))
- [ ] Record each typed point's range, units and enumeration with describe_control_point (delay types, filter types, slopes, scales), which the guide does not tabulate. ([specs/meyersound-galaxy.yaml:107](specs/meyersound-galaxy.yaml#L107))

## meyersound-spacemap-go — Meyer Sound Spacemap Go

- [ ] Confirm that general purpose OSC is received over UDP on 38033, and record the argument types Spacemap Go accepts (ints, floats, a string for the trajectory are assumed). ([specs/meyersound-spacemap-go.yaml:28](specs/meyersound-spacemap-go.yaml#L28))
- [ ] Confirm the position command takes X then Y as two arguments in one message. ([specs/meyersound-spacemap-go.yaml:50](specs/meyersound-spacemap-go.yaml#L50))

## middle-atlantic-racklink — Middle Atlantic RackLink power (control protocol)

- [ ] Check the status change registration: its length byte (the document says 6 while listing six data bytes), which bit is "bit 1", and that the unit then pushes outlet, contact, sequence, EPO and reading changes. ([specs/middle-atlantic-racklink.yaml:268](specs/middle-atlantic-racklink.yaml#L268))
- [ ] Check the sequencing response's subcommand (the document shows 0x01 where a response would be 0x10) and its states. ([specs/middle-atlantic-racklink.yaml:127](specs/middle-atlantic-racklink.yaml#L127))
- [ ] Check the current energy management state response (0x60): occupancy first and a letter per outlet (INFOCOMM 4:2012) or 16 letters (I-00472). ([specs/middle-atlantic-racklink.yaml:168](specs/middle-atlantic-racklink.yaml#L168))
- [ ] Check the temperature's unit in sensor readings and thresholds (the log gives Fahrenheit). ([specs/middle-atlantic-racklink.yaml:282](specs/middle-atlantic-racklink.yaml#L282))
- [ ] Check how often the unit pings and that answering each ping keeps the connection, on Select, Premium and Premium+. ([specs/middle-atlantic-racklink.yaml:9](specs/middle-atlantic-racklink.yaml#L9))
- [ ] Check RS-232 on a Premium: the same messages at 9600 8N1, and whether it needs the login. ([specs/middle-atlantic-racklink.yaml:53](specs/middle-atlantic-racklink.yaml#L53))

## midi-show-control — MIDI Show Control (MSC) device

- [ ] Against an MSC lighting console (ETC Eos, grandMA, Chamsys), confirm GO, STOP, RESUME, TIMED_GO, SET and FIRE act as described for its device ID and command format, and whether it answers all_types and the all-call ID 127. ([specs/midi-show-control.yaml:318](specs/midi-show-control.yaml#L318))
- [ ] Against a controlled device that implements two-phase commit, confirm the checksum is computed with the first byte of each pair as the low byte. ([specs/midi-show-control.yaml:352](specs/midi-show-control.yaml#L352))
- [ ] Confirm which device ID such a device addresses its STANDING_BY, COMPLETE, CANCELLED and ABORT to, and which status code it uses for "manual override in progress" in CANCELLED (8028 or 8030). ([specs/midi-show-control.yaml:352](specs/midi-show-control.yaml#L352))
- [ ] Confirm that a GO_2PC's COMPLETE arrives within 125 % of the STANDING_BY's maximum time on a real device, so the core's timeout does not fire early. ([specs/midi-show-control.yaml:332](specs/midi-show-control.yaml#L332))

## millumin — Millumin

- [ ] Confirm the default OSC input port of Millumin 5 (5000 in the documentation; a V5 screenshot shows 8000). ([specs/millumin.yaml:903](specs/millumin.yaml#L903))
- [ ] Check that changes made through the OSC API send no feedback, and which side effects (a column's media starting) still do. ([specs/millumin.yaml:838](specs/millumin.yaml#L838))
- [ ] Check where /? and /ping answers go (the configured senders are assumed) and what /ping sends. ([specs/millumin.yaml:839](specs/millumin.yaml#L839))
- [ ] Check the light intensity scale on input (0-255 in the documentation's example) and feedback (0 to 1), and the master levels' range. ([specs/millumin.yaml:890](specs/millumin.yaml#L890))
- [ ] Check the rate of /millumin/layer:<name>/media/time feedback and whether it is sent for every layer. ([specs/millumin.yaml:708](specs/millumin.yaml#L708))
- [ ] Check whether Millumin 5 still has a global API feedback switch beside each sender's send feedback. ([specs/millumin.yaml:830](specs/millumin.yaml#L830))
- [ ] Check which of the dashboard and the edited timeline play, pause and go_to_time act on. ([specs/millumin.yaml:869](specs/millumin.yaml#L869))
- [ ] Check that stop_column takes no column in Millumin 5. ([specs/millumin.yaml:871](specs/millumin.yaml#L871))
- [ ] Check that floats are accepted where the documentation shows whole numbers (positions, rotation, media time). ([specs/millumin.yaml:408](specs/millumin.yaml#L408))
- [ ] Check that /layer:<name>/mapping/<corner> (and its /x and /y forms) moves the corner for a named layer, as the documentation shows for selectedLayer. ([specs/millumin.yaml:471](specs/millumin.yaml#L471))
- [ ] Record the feedback for a moved mapping corner: its address (mapping/topLeft by layer name assumed) and arguments ([x, y] assumed, or x and y apart). ([specs/millumin.yaml:738](specs/millumin.yaml#L738))
- [ ] Record the feedback address and argument for an effect parameter change (layer:<name>/effect<N>/<param> [value] assumed). ([specs/millumin.yaml:747](specs/millumin.yaml#L747))
- [ ] Record the argument of /millumin/layer:<name>/selected and /millumin/light:<name>/selected (1 assumed from the /millumin/info example), and whether deselection sends 0. ([specs/millumin.yaml:750](specs/millumin.yaml#L750))
- [ ] Check whether media normalizedTime and speed feedback is also sent by layer name, or only for the selected layer as documented. ([specs/millumin.yaml:735](specs/millumin.yaml#L735))
- [ ] Record the argument types of /millumin/info (the original address as a string, then the values). ([specs/millumin.yaml:756](specs/millumin.yaml#L756))
- [ ] Check that a property's /? query sent right after an OSC change is answered with the new value, and which address the position, mapping corner and effect queries answer on. ([specs/millumin.yaml:841](specs/millumin.yaml#L841))
- [ ] Check which column feedback name Millumin V5 sends (launchedColumn in the Feedback section, columnLaunched in the Summary), and whether launching a column also reports the previous one stopped. ([specs/millumin.yaml:678](specs/millumin.yaml#L678))
- [ ] Confirm /millumin/info carries the original address (layer:Name/opacity, light:Name/intensity) as its first argument and the message's values after it, for every property read from it (the legacy example shows only selection), and that a setup can send only this form. ([specs/millumin.yaml:764](specs/millumin.yaml#L764))

## mixingstation — Mixing Station desktop (OSC API)

- [ ] A get on /con/v/<path> or /con/n/<path> is answered on the same address with the value as its one argument (the documentation shows the get but not its answer). ([specs/mixingstation.yaml:57](specs/mixingstation.yaml#L57))
- [ ] After /hi/v, every parameter change arrives as /con/v/<path> with the value in the parameter's own OSC type (f, i, T/F or s); the documentation does not show the update messages. ([specs/mixingstation.yaml:139](specs/mixingstation.yaml#L139))
- [ ] /hi/v resent every 4 seconds keeps the subscription alive, and Mixing Station answers to the address and port the subscription came from. ([specs/mixingstation.yaml:137](specs/mixingstation.yaml#L137))
- [ ] An on/off parameter takes T or F (OSC's true and false); the documentation lists bool as a data type without naming its type tag. ([specs/mixingstation.yaml:91](specs/mixingstation.yaml#L91))

## modulopi-kinetic — Modulo Pi Modulo Kinetic

- [ ] Confirm the websocket path (/ is used) and whether wss uses a self-signed certificate when SSL is on. ([specs/modulopi-kinetic.yaml:45](specs/modulopi-kinetic.yaml#L45))
- [ ] Confirm a reply carries an error object (not a result) when a method fails, and that every doaction and set.property answers with a result. ([specs/modulopi-kinetic.yaml:341](specs/modulopi-kinetic.yaml#L341))
- [ ] Check the units of a timeline's timecode, grandMasterFader and audioMaster in get.list.timelines (ms and percent are assumed), and the fade argument of grandMasterFader and audioMaster (timecode, in ms). ([specs/modulopi-kinetic.yaml:188](specs/modulopi-kinetic.yaml#L188))
- [ ] Find how the Designer's uuid for doaction.kinetic is obtained. ([specs/modulopi-kinetic.yaml:345](specs/modulopi-kinetic.yaml#L345))
- [ ] Check the forms of a vec2-vec4 and color variable's value in get.list.variables. ([specs/modulopi-kinetic.yaml:380](specs/modulopi-kinetic.yaml#L380))

## modulopi-player — Modulo Pi Modulo Player

- [ ] Confirm the websocket path is / on port 8080 and that connections are refused until a show is loaded. ([specs/modulopi-player.yaml:68](specs/modulopi-player.yaml#L68))
- [ ] Confirm mixer.state is pushed after get.state alone (the poll), with the same fields as get.state's result. ([specs/modulopi-player.yaml:828](specs/modulopi-player.yaml#L828))
- [ ] Check that the media list's duration is in seconds and the playlists' timecode in milliseconds. ([specs/modulopi-player.yaml:873](specs/modulopi-player.yaml#L873))
- [ ] Confirm set.monitoring.zone takes sourcename for an output and a live input, and set.cue.settings takes property name with value. ([specs/modulopi-player.yaml:623](specs/modulopi-player.yaml#L623))
- [ ] Check what get.list.playlists' playState holds when stopped (only true is documented). ([specs/modulopi-player.yaml:852](specs/modulopi-player.yaml#L852))

## modulopi-spydog — Modulo Pi Spydog

- [ ] Confirm the websocket path is / on port 8081 and the units of cpuTemperature, motherboardTemperature, upTime and serverTime. ([specs/modulopi-spydog.yaml:31](specs/modulopi-spydog.yaml#L31))
- [ ] Confirm doaction.computer answers false (not an error) when an action fails. ([specs/modulopi-spydog.yaml:47](specs/modulopi-spydog.yaml#L47))

## motu-avb — MOTU AVB interfaces

- [ ] Measure the size of a full GET /datastore on a 1248 or 16A with the mixer and whether reading it every 5 s burdens the device. ([specs/motu-avb.yaml:1419](specs/motu-avb.yaml#L1419))
- [ ] Confirm that PATCH with the form field json={"value": v} answers 200 or 204, and record the answer to a path the device lacks and to a wrong type. ([specs/motu-avb.yaml:189](specs/motu-avb.yaml#L189))
- [ ] Confirm the src pair is the input bank index and channel index, both from 0 (the document's placeholders are lost). ([specs/motu-avb.yaml:1212](specs/motu-avb.yaml#L1212))

## nec-display — NEC (Sharp NEC) large-format displays

- [ ] Confirm a reply timeout of 5 s suits every command (NEC gives none for the controller; the display's own is 10 s), and that a display answers the power status in standby. ([specs/nec-display.yaml:670](specs/nec-display.yaml#L670))
- [ ] Confirm the 15 s wait after power and 10 s after an input change are enough, and whether a message sent sooner is answered with the NULL message (BE). ([specs/nec-display.yaml:825](specs/nec-display.yaml#L825))
- [ ] Confirm 600 ms between messages is needed on the 2011-2013 generation only. ([specs/nec-display.yaml:49](specs/nec-display.yaml#L49))
- [ ] Check the green and blue gain codes (00-18, 00-1A) on a P404 or P403 generation display, whose tables swap them. ([specs/nec-display.yaml:233](specs/nec-display.yaml#L233))
- [ ] Confirm the mute code (00-8D) is not taken by the M series, as its table leaves it out. ([specs/nec-display.yaml:334](specs/nec-display.yaml#L334))
- [ ] Record the firmware version reply (CA02) on each generation: raw characters on the M series, two hex digits per character before. ([specs/nec-display.yaml:683](specs/nec-display.yaml#L683))
- [ ] Confirm the date year (C211/C212) is counted from 2000 on the M series, and what its last byte (daylight saving, reserved there) is answered. ([specs/nec-display.yaml:685](specs/nec-display.yaml#L685))
- [ ] Confirm the temperature readout's 0.5 °C two's complement and how many sensors each model has. ([specs/nec-display.yaml:698](specs/nec-display.yaml#L698))
- [ ] Record whether a display answers messages addressed to ALL or a group, and whether more than one TCP connection is taken at once. ([specs/nec-display.yaml:849](specs/nec-display.yaml#L849))
- [ ] Confirm out-of-range sets are ignored with result 00 (PN-ME §8.2) on the other series too. ([specs/nec-display.yaml:831](specs/nec-display.yaml#L831))

## nec-projector — NEC and Sharp NEC projectors

- [ ] Find the serial speed a projector ships with: the manual lists the speeds each model offers but no default, and 38400 bit/s is assumed. ([specs/nec-projector.yaml:59](specs/nec-projector.yaml#L59))
- [ ] Check whether any model needs RTS/CTS flow control on its PC CONTROL port, which the pinout wires but the communication conditions do not mention. ([specs/nec-projector.yaml:64](specs/nec-projector.yaml#L64))
- [ ] Confirm commands sent with ID1 and ID2 as 00h are answered when a control ID is set on the projector, and whether TCP 7142 keeps an idle connection open or closes it. ([specs/nec-projector.yaml:840](specs/nec-projector.yaml#L840))
- [ ] Measure how long POWER ON and POWER OFF take to answer (15 s is allowed). ([specs/nec-projector.yaml:840](specs/nec-projector.yaml#L840))
- [ ] Confirm each model's input terminal codes, which differ by family (HDMI is 1Ah on the older models and A1h on the newer). ([specs/nec-projector.yaml:845](specs/nec-projector.yaml#L845))
- [ ] Find what the two names the aspect table gives one code mean (02h WIDE SCREEN and 16:9, CINEMA and 16:9; 06h ZOOM and FULL), and which of 06h and 0Ch, 05h and 0Fh an NP-PX651X takes for 16:10 and AUTO. ([specs/nec-projector.yaml:1197](specs/nec-projector.yaml#L1197))
- [ ] Check that 037-6 sends kilograms little-endian: the manual's example for 2460.06375 kg prints 9Ch 90h where 2460 is 9Ch 09h. ([specs/nec-projector.yaml:919](specs/nec-projector.yaml#L919))
- [ ] Check that 053-1 and 060-1 values are signed 16-bit (negative lens shift positions, picture minimums). ([specs/nec-projector.yaml:948](specs/nec-projector.yaml#L948))
- [ ] Check how a one-lamp or laser model answers 037-4 for lamp 2 and for the remaining life on 2024 models. ([specs/nec-projector.yaml:913](specs/nec-projector.yaml#L913))
- [ ] Confirm the 098-198 reply for sub inputs 2 and 3 names them 03h and 04h, as the manual prints, or 09h and 0Ah as in the request. ([specs/nec-projector.yaml:1067](specs/nec-projector.yaml#L1067))
- [ ] Confirm the NP-U321H's eco modes (OFF 00h, ECO 01h; the appendix's cells are shared with the next columns). ([specs/nec-projector.yaml:571](specs/nec-projector.yaml#L571))
- [ ] Check the 2024 keys on 2020 models and the 2020 keys (AUTO, PICTURE, COMPUTER1-2, VIDEO1, S-VIDEO1, FREEZE, ASPECT, SOURCE) on 2024 models. ([specs/nec-projector.yaml:924](specs/nec-projector.yaml#L924))
- [ ] Check which characters 098-45 accepts in a projector name beyond letters and digits. ([specs/nec-projector.yaml:1044](specs/nec-projector.yaml#L1044))

## nec-projector-ascii — Sharp NEC projectors and monitors (Common ASCII Control Command)

- [ ] Confirm current-value responses repeat the command's words before the value (">volume 5", ">lens zoom 1000", ">usage light hours 120", ">avmute video on", ">tilematrix position 2"); the manual shows only the power, volume, input and status forms. ([specs/nec-projector-ascii.yaml:421](specs/nec-projector-ascii.yaml#L421))
- [ ] Confirm TCP 7142 takes ASCII and binary commands on the same connection, and whether the projector keeps an idle connection open. ([specs/nec-projector-ascii.yaml:52](specs/nec-projector-ascii.yaml#L52))
- [ ] Find the serial defaults of models other than the NP-P627UL family (38400 bit/s, 8N1, no flow control there), and of monitors. ([specs/nec-projector-ascii.yaml:47](specs/nec-projector-ascii.yaml#L47))
- [ ] Confirm a display without a polled function answers error:command-try'help' or error:parameter promptly, so the poll is not held up. ([specs/nec-projector-ascii.yaml:427](specs/nec-projector-ascii.yaml#L427))
- [ ] Check how negative lens travel times and half-second values are written ("lens focus time -1.5"). ([specs/nec-projector-ascii.yaml:246](specs/nec-projector-ascii.yaml#L246))
- [ ] Confirm the NORMAL standby mode refuses ASCII commands on the network on other models. ([specs/nec-projector-ascii.yaml:435](specs/nec-projector-ascii.yaml#L435))

## newblue-captivate — NewBlue Captivate / Titler Live

- [ ] Check that subscribe with empty parameters, sent at once on connecting, starts the notification stream on the TCP socket. ([specs/newblue-captivate.yaml:811](specs/newblue-captivate.yaml#L811))
- [ ] Record a data notification (event data) so variable changes can be read from the push instead of the poll. ([specs/newblue-captivate.yaml:924](specs/newblue-captivate.yaml#L924))
- [ ] Confirm a failed command answers with result.success false (and an error field) rather than an error reply, for macro and setTitleName with an unknown title. ([specs/newblue-captivate.yaml:926](specs/newblue-captivate.yaml#L926))
- [ ] Check whether boolean-like flags (merge, autoCache) are accepted as the text "1" and setDynamic, verbose, showUI and sendVideo as the number 1, as the document types them. ([specs/newblue-captivate.yaml:214](specs/newblue-captivate.yaml#L214))
- [ ] Confirm getTitleControlInfo with empty parameters lists every title, and that its reply carries result.command. ([specs/newblue-captivate.yaml:826](specs/newblue-captivate.yaml#L826))
- [ ] Confirm saveSettings answers {"result": null} on success and what it answers on failure. ([specs/newblue-captivate.yaml:166](specs/newblue-captivate.yaml#L166))

## newtek-3play — NewTek / Vizrt 3Play

- [ ] Record the shortcuts a 3Play's macro recorder shows for replay operations (mark in and out, cue, play on outputs A and B, play speed, play lists, recording), so they can become typed commands. ([specs/newtek-3play.yaml:258](specs/newtek-3play.yaml#L258))
- [ ] Confirm which dictionaries a 3Play serves (shortcut_states, tally, macros_list and others) and record their content. ([specs/newtek-3play.yaml:164](specs/newtek-3play.yaml#L164))
- [ ] Confirm a 3Play answers /v1/trigger, /v1/datalink and /v1/live as a TriCaster does. ([specs/newtek-3play.yaml:148](specs/newtek-3play.yaml#L148))
- [ ] Confirm the change notification websocket is on the web port on a 3Play and which dictionary names it sends. ([specs/newtek-3play.yaml:162](specs/newtek-3play.yaml#L162))
- [ ] Confirm the login scheme (Basic sent, Digest answered if challenged). ([specs/newtek-3play.yaml:273](specs/newtek-3play.yaml#L273))

## newtek-tricaster — NewTek / Vizrt TriCaster and Vectar

- [ ] Record the response body of a shortcut request, since whether the shortcut applied is not confirmed by the 200. ([specs/newtek-tricaster.yaml:770](specs/newtek-tricaster.yaml#L770))
- [ ] Confirm the login scheme (Basic sent, Digest answered if challenged). ([specs/newtek-tricaster.yaml:763](specs/newtek-tricaster.yaml#L763))
- [ ] Confirm the change notification websocket is on the web port (the guide's text says 5951, its example the web port), whether it needs the Live Panel login, and whether it stays open without the client sending anything. ([specs/newtek-tricaster.yaml:599](specs/newtek-tricaster.yaml#L599))
- [ ] Confirm each notification is the bare dictionary name, one per message, as the guide says. ([specs/newtek-tricaster.yaml:623](specs/newtek-tricaster.yaml#L623))
- [ ] Record the ddr_playlist and ddr_timecode dictionaries, whose content the guide does not show, so clips and remaining times can be read into state rather than kept as XML; and check that polling ddr_timecode every second is safe on current Vizion software. ([specs/newtek-tricaster.yaml:673](specs/newtek-tricaster.yaml#L673))
- [ ] Record the macros_list dictionary: confirm macro elements with a name attribute and find the id attribute's name. ([specs/newtek-tricaster.yaml:696](specs/newtek-tricaster.yaml#L696))
- [ ] Check which of /version (the guide) and /v1/version answers on each software version. ([specs/newtek-tricaster.yaml:619](specs/newtek-tricaster.yaml#L619))
- [ ] Record the buffer dictionary with several M/Es: check each M/E's index (from 0 as the guide's example shows, or from 1) and whether an M/E has more than one row of buffers. ([specs/newtek-tricaster.yaml:668](specs/newtek-tricaster.yaml#L668))
- [ ] Check whether media_select_clip counts clips from 0 or 1. ([specs/newtek-tricaster.yaml:337](specs/newtek-tricaster.yaml#L337))
- [ ] Check that the mode toggles take true or false as a value to set the mode rather than toggle it. ([specs/newtek-tricaster.yaml:404](specs/newtek-tricaster.yaml#L404))
- [ ] Check the volume shortcut's value is dB, with one decimal accepted, and its range. ([specs/newtek-tricaster.yaml:423](specs/newtek-tricaster.yaml#L423))
- [ ] Confirm the 3Play channel commands posted as XML through the net shortcut (<shortcut name="net1" value="clip_play" speed="1.000"/>) reach the 3Play, and how net_shortcut's shortcut_name key is written. ([specs/newtek-tricaster.yaml:447](specs/newtek-tricaster.yaml#L447))
- [ ] Confirm the 3Play channel commands' number formats (three decimals sent for speed, position and distance) are accepted. ([specs/newtek-tricaster.yaml:514](specs/newtek-tricaster.yaml#L514))
- [ ] Confirm POST /v1/file honours the Filename and Overwrite headers and answers with the stored file name as the body. ([specs/newtek-tricaster.yaml:569](specs/newtek-tricaster.yaml#L569))
- [ ] Record the audiomixer, audio_bins, ndi_sources and filebrowser dictionaries, whose content the guide does not show, so they can be read into state rather than kept as XML. ([specs/newtek-tricaster.yaml:681](specs/newtek-tricaster.yaml#L681))
- [ ] Confirm GET /v1/datalink lists every DataLink key as <data><key/><value/></data> elements. ([specs/newtek-tricaster.yaml:686](specs/newtek-tricaster.yaml#L686))
- [ ] Confirm Vizrt Vectar and Viz Vectar Plus answer the same shortcuts, dictionaries and change notification websocket as a TriCaster, and their input count for the net commands. ([specs/newtek-tricaster.yaml:852](specs/newtek-tricaster.yaml#L852))

## nimble-streamer — Nimble Streamer (native API)

- [ ] Confirm a server without management_token answers every call, and that a server with one takes salt and hash as query parameters on POST and DELETE as well as GET, with the hash's + / = percent-encoded (the reference's example sends them raw); record its refusal of a wrong hash. ([specs/nimble-streamer.yaml:67](specs/nimble-streamer.yaml#L67))
- [ ] Confirm the POST calls without a body (sync_panel_settings, reload_config, reload_ssl_certificates, dvr reload and cleanup, the Advertizer markers) are accepted with an empty body, and record what they answer. ([specs/nimble-streamer.yaml:100](specs/nimble-streamer.yaml#L100))
- [ ] Confirm get_cache_key is a POST with a JSON body (the reference's curl example sends -d to it). ([specs/nimble-streamer.yaml:452](specs/nimble-streamer.yaml#L452))
- [ ] Confirm create_rtmp_republish_rule takes dest_port as a number (the reference's example sends it as a string). ([specs/nimble-streamer.yaml:164](specs/nimble-streamer.yaml#L164))
- [ ] Confirm live_streams_status gives bandwidth and publish_time as strings of digits (as in the reference's example) or numbers, both read as integers. ([specs/nimble-streamer.yaml:605](specs/nimble-streamer.yaml#L605))
- [ ] Record the shapes of mpeg2ts_status, srt and rist statistics, ndi/list and dvr_status replies beyond the reference's examples, to keep them in state. ([specs/nimble-streamer.yaml:520](specs/nimble-streamer.yaml#L520))

## nmos-node — AMWA NMOS Node (IS-04, IS-05, IS-08, IS-07)

- [ ] Check against a real Node (and nmos-cpp's node) that GET /x-nmos/node answers as the liveness probe. ([specs/nmos-node.yaml:74](specs/nmos-node.yaml#L74))
- [ ] Check that an empty connection_port (and channelmapping_port, events_port) reaches a Node serving every API on its Node API port, and record which Nodes serve them on other ports. ([specs/nmos-node.yaml:107](specs/nmos-node.yaml#L107))
- [ ] Check which Nodes still accept the deprecated PUT /receivers/{id}/target, and what they answer (202, or 501). ([specs/nmos-node.yaml:297](specs/nmos-node.yaml#L297))
- [ ] Check a connect_receiver with an SDP against a Node, and its answer to the read-back of active. ([specs/nmos-node.yaml:441](specs/nmos-node.yaml#L441))
- [ ] Check that a Node's /single/senders and /single/receivers lists end each id with a slash, as the example shows, and that the per-id reads keep up with many senders every 5 seconds. ([specs/nmos-node.yaml:975](specs/nmos-node.yaml#L975))
- [ ] Check that a Sender's transportfile answers with the SDP itself (not a redirect the client must follow). ([specs/nmos-node.yaml:1036](specs/nmos-node.yaml#L1036))
- [ ] Check an IS-08 Node's channel indexes (from 0) and route_channel's single-channel activation. ([specs/nmos-node.yaml:623](specs/nmos-node.yaml#L623))
- [ ] Check that GET /map/activations/ (with the trailing slash) answers the scheduled activations on IS-08 Nodes. ([specs/nmos-node.yaml:1090](specs/nmos-node.yaml#L1090))
- [ ] Check that reading IS-07 event states every 2 seconds is fast enough for tally, and what a Node without IS-07 answers at /x-nmos/events. ([specs/nmos-node.yaml:716](specs/nmos-node.yaml#L716))
- [ ] Check against an IS-07 Node with a websocket Sender that one subscription command naming every ext_is_07_source_id at a connection_uri brings each source's state, and that a health command every 5 seconds keeps the connection. ([crates/core/src/modules/nmos_events.rs:529](crates/core/src/modules/nmos_events.rs#L529))
- [ ] Check against a broker that an IS-07 MQTT Sender's retained state arrives on subscribing at QoS 0 to its broker_topic, that its connection status topic gives the will message, and how a broker with broker_authorization true takes its credential (a user name and password assumed). ([crates/core/src/modules/nmos_events.rs:543](crates/core/src/modules/nmos_events.rs#L543))
- [ ] Check that a Sender's IS-05 active parameters give the broker or connection_uri the extension follows (not auto), and what Senders on a Node with many Devices list. ([crates/core/src/modules/nmos_events.rs:133](crates/core/src/modules/nmos_events.rs#L133))

## nmos-registry — AMWA NMOS registry (IS-04 Query API)

- [ ] Check against a registry (nmos-cpp's and a vendor's) that GET /x-nmos/query answers as the liveness probe. ([specs/nmos-registry.yaml:35](specs/nmos-registry.yaml#L35))
- [ ] Check that a registry answers the subscription POST with a ws_href the core can reach (a registry behind NAT or naming a host name), and that the first grain on each websocket is the whole list. ([specs/nmos-registry.yaml:39](specs/nmos-registry.yaml#L39))
- [ ] Check that a registry accepts max_update_rate_ms 0 and 100, and how it batches changes at 100. ([specs/nmos-registry.yaml:60](specs/nmos-registry.yaml#L60))
- [ ] Record a registry's default page size for the get_ commands, and check query_resources with basic queries and paging. ([specs/nmos-registry.yaml:202](specs/nmos-registry.yaml#L202))
- [ ] Check that a registry answers create_subscription with 200 for an existing identical subscription and 201 for a new one. ([specs/nmos-registry.yaml:231](specs/nmos-registry.yaml#L231))
- [ ] Check that a grain deleting a resource carries its id in path (and pre), as the module reads it. ([specs/nmos-registry.yaml:365](specs/nmos-registry.yaml#L365))
- [ ] Check what an mDNS browse of _nmos-query._tcp finds on a network with several registries (TXT pri), and whether registries advertise only by unicast DNS-SD there. ([crates/core/src/mdns.rs:258](crates/core/src/mdns.rs#L258))
- [ ] Check a unicast DNS-SD browse of _nmos-query._tcp against a site's DNS server (PTR, SRV, TXT and A answered in turn or together), and that the host's own DNS servers and domains are the ones asked on Windows, macOS and Linux. ([crates/core/src/dns_sd.rs:344](crates/core/src/dns_sd.rs#L344))

## novastar-central-control — NovaStar COEX central control protocol

- [ ] Record a refused request's answer (a non-zero ACK byte is assumed) and whether its checksum covers the aa 55 header as the documented answer's does. ([specs/novastar-central-control.yaml:153](specs/novastar-central-control.yaml#L153))
- [ ] Check whether the controller keeps an idle TCP connection open, and whether it answers anything that could serve as a liveness check. ([specs/novastar-central-control.yaml:146](specs/novastar-central-control.yaml#L146))
- [ ] Confirm that output card 255 addresses every card on single-card controllers and that card numbers start from 1 on MX6000 Pro and MX2000 Pro. ([specs/novastar-central-control.yaml:169](specs/novastar-central-control.yaml#L169))
- [ ] Confirm the card-based MX6000 Pro and MX2000 Pro answer the same frames as the single-card controllers. ([specs/novastar-central-control.yaml:52](specs/novastar-central-control.yaml#L52))

## novastar-coex — NovaStar COEX

- [ ] Check which API generation each model and firmware answers: the current OpenAPI paths (/api/v1/screen/..., /api/v1/preset/...) or the 2023 manual's (/api/v1/device/screen/..., /api/v1/device/currentpreset). ([specs/novastar-coex.yaml:1161](specs/novastar-coex.yaml#L1161))
- [ ] Confirm that the endpoints the OpenAPI pages list without /api/v1 (internal source, colour correction, 3D emitter, sync source, identify, controller name, no-signal image, thermal amount) answer with the prefix. ([specs/novastar-coex.yaml:2158](specs/novastar-coex.yaml#L2158))
- [ ] Record the displayMode numbers the display state read reports for blackout and freeze. ([specs/novastar-coex.yaml:2166](specs/novastar-coex.yaml#L2166))
- [ ] Check whether requests need the Device-Key header shown in the OpenAPI examples. ([specs/novastar-coex.yaml:2237](specs/novastar-coex.yaml#L2237))
- [ ] Record the screenID format (the examples here assume a braced GUID-like string) and that a one-element screenIdList acts on that screen only. ([specs/novastar-coex.yaml:213](specs/novastar-coex.yaml#L213))
- [ ] Check switch_layer_source in send-only and all-in-one modes, and what layer IDs each mode uses. ([specs/novastar-coex.yaml:452](specs/novastar-coex.yaml#L452))
- [ ] Record the colour component range of set_test_pattern (0-4095 in the 2023 example, gray 0-255 in OpenAPI) and how to return from a test pattern to the input. ([specs/novastar-coex.yaml:622](specs/novastar-coex.yaml#L622))
- [ ] Confirm identify_controller takes a JSON body (the OpenAPI page names application/xml). ([specs/novastar-coex.yaml:834](specs/novastar-coex.yaml#L834))
- [ ] Check whether set_working_mode to all-in-one (3) works through the API. ([specs/novastar-coex.yaml:947](specs/novastar-coex.yaml#L947))
- [ ] Confirm MX30 and MX20 answer the same API as the MX40 Pro. ([specs/novastar-coex.yaml:68](specs/novastar-coex.yaml#L68))
- [ ] Confirm /api/v1/screen/output/display/state lists every canvas: each poll replaces canvases. ([specs/novastar-coex.yaml:1284](specs/novastar-coex.yaml#L1284))
- [ ] Record the envelope of each list read, which the OpenAPI pages leave out: displayparams and cabinet count as data.list, screen output, schedules, input sources, cabinets and multifunction cards as a data array, presets as data.screenPresetList. ([specs/novastar-coex.yaml:2229](specs/novastar-coex.yaml#L2229))
- [ ] Confirm GET /api/v1/screen/output answers one element per screen in a data array, keyed by screenid, and not a single object. ([specs/novastar-coex.yaml:1455](specs/novastar-coex.yaml#L1455))
- [ ] Confirm InputPortConfig in GET /api/v1/device/input is an array (the page types it as an object) and that its logicId is the source ID the input commands take. ([specs/novastar-coex.yaml:1602](specs/novastar-coex.yaml#L1602))
- [ ] Confirm a preset's state in GET /api/v1/preset is true for the applied preset only, so current_preset follows recall_preset. ([specs/novastar-coex.yaml:1421](specs/novastar-coex.yaml#L1421))
- [ ] Check how set_sync_source shows in GET /api/v1/screen/output: whether genlock.masterLayerGroupId follows selectSource, and the values of genlock.selectedType. ([specs/novastar-coex.yaml:1465](specs/novastar-coex.yaml#L1465))
- [ ] Check what GET /api/v1/screen/base/info's characteristic reports (0 or 1, or a bit field). ([specs/novastar-coex.yaml:1481](specs/novastar-coex.yaml#L1481))
- [ ] Confirm the cabinet IDs of GET /api/v1/device/cabinet (id), GET /api/v1/screen (cabinetID) and the monitoring read (CabinetID) are the same numbers. ([specs/novastar-coex.yaml:1626](specs/novastar-coex.yaml#L1626))
- [ ] Record what a receiving card's nextCabinetLinkStatus.linkStatus true means (disconnected or connected). ([specs/novastar-coex.yaml:1773](specs/novastar-coex.yaml#L1773))
- [ ] Confirm a multifunction card's linkStatus is the string "0" or "1" (the page types it as a string). ([specs/novastar-coex.yaml:1822](specs/novastar-coex.yaml#L1822))
- [ ] Check that the layer window, border and crop values of GET /api/v1/screen change only from VMP, and whether any undocumented endpoint writes them or creates and removes layers. ([specs/novastar-coex.yaml:2213](specs/novastar-coex.yaml#L2213))
- [ ] Check whether any read reports the settings the API only writes (brightness limit, colour correction, 3D LUT, 3D emitter, time, per-cabinet test pattern, mapping, thermal compensation, no-signal image). ([specs/novastar-coex.yaml:2198](specs/novastar-coex.yaml#L2198))

## novastar-h — NovaStar H series

- [ ] Confirm that unencrypted requests signed with Base64(md5(timeStamp + pId)) are accepted with encryption off for the requestor, and how far the time may drift before error 11. ([specs/novastar-h.yaml:862](specs/novastar-h.yaml#L862))
- [ ] Confirm the OpenAPI listens on port 8000 (the documentation's example address) on every model. ([specs/novastar-h.yaml:47](specs/novastar-h.yaml#L47))
- [ ] Check which deviceId to send (0 in most examples, 1 on the brightness pages). ([specs/novastar-h.yaml:887](specs/novastar-h.yaml#L887))
- [ ] Record whether the splicer pushes notifications (the change history mentions a websocket for layer z-order) and on which address. ([specs/novastar-h.yaml:905](specs/novastar-h.yaml#L905))
- [ ] Check set_screen_bkg's enable sense (documented 0 on, 1 off), and what screen/readDetail's Bkg.enable reports after each. ([specs/novastar-h.yaml:770](specs/novastar-h.yaml#L770))
- [ ] Check update_input_crop with the documented field spelling heigth. ([specs/novastar-h.yaml:283](specs/novastar-h.yaml#L283))
- [ ] Capture a screen/readDetail reply (the page gives only its schema) and confirm brightness, gamma, Ftb, Freeze, Bkg, outputMode.size and mosaic, screen3D, Osd and OsdImage are where the module reads them. ([specs/novastar-h.yaml:897](specs/novastar-h.yaml#L897))
- [ ] Confirm Ftb.enable is 1 while a screen is faded to black and 0 once screen_ftb brings it back. ([specs/novastar-h.yaml:768](specs/novastar-h.yaml#L768))
- [ ] Check screen/readDetail's gamma after set_screen_gamma with a decimal (the schema types it as an integer). ([specs/novastar-h.yaml:767](specs/novastar-h.yaml#L767))
- [ ] Check the scale of an input's timing.refreshRate (60 in the example, documented as frame rate x100). ([specs/novastar-h.yaml:815](specs/novastar-h.yaml#L815))
- [ ] Check the sourceType a layer reports for an input (1 in the examples and on the write page, 2 on the list page). ([specs/novastar-h.yaml:790](specs/novastar-h.yaml#L790))
- [ ] Check that an input's slotId, interfaceType and sourceType from input/readList, sent as they are in set_layer_source's body with its inputId, put that input on the layer. ([specs/novastar-h.yaml:816](specs/novastar-h.yaml#L816))
- [ ] Capture an input's crops array with a cropped source (the example's is empty) and confirm each element has cropId, name, x, y, width and height. ([specs/novastar-h.yaml:829](specs/novastar-h.yaml#L829))
- [ ] Confirm bkg/readAllList and input/readSourceList answer in data (not body), as their examples show. ([specs/novastar-h.yaml:897](specs/novastar-h.yaml#L897))
- [ ] Check that polling six requests per screen at 5 s does not load the splicer, and whether a slower default is wiser. ([specs/novastar-h.yaml:899](specs/novastar-h.yaml#L899))

## obs-studio — OBS Studio (obs-websocket 5)

- [ ] Record the fields of the scene item, filter, input, transition and output objects in GetSceneItemList, GetSourceFilterList, GetInputList, GetSceneTransitionList and GetOutputList replies (sceneItemId, sceneItemBlendMode, filterSettings, unversionedInputKind, transitionFixed, outputWidth and the rest): the protocol description names the arrays but not their members. ([specs/obs-studio.yaml:1106](specs/obs-studio.yaml#L1106))
- [ ] Check that InputVolumeMeters arrives every 50 ms listing only active inputs, including when none is active, since an input it stops listing loses its levels. ([specs/obs-studio.yaml:1181](specs/obs-studio.yaml#L1181))
- [ ] Check which input kinds answer GetMediaInputStatus (the media source, VLC source and image slide show are assumed) and what other inputs reply. ([specs/obs-studio.yaml:1184](specs/obs-studio.yaml#L1184))
- [ ] Check that RecordStateChanged carries outputPath only on stop, and that RecordFileChanged fires on each split. ([specs/obs-studio.yaml:1143](specs/obs-studio.yaml#L1143))
- [ ] Check GetReplayBufferStatus's refusal when the profile has no replay buffer, and that the replay buffer state is read again after a profile change. ([specs/obs-studio.yaml:1145](specs/obs-studio.yaml#L1145))
- [ ] Check that the stream service settings name the server "server" for both rtmp_common and rtmp_custom. ([specs/obs-studio.yaml:1160](specs/obs-studio.yaml#L1160))
- [ ] Check the units of GetStats' memoryUsage and availableDiskSpace (MB assumed). ([specs/obs-studio.yaml:1162](specs/obs-studio.yaml#L1162))
- [ ] Check that no event follows set_scene_item_blend_mode, set_input_deinterlace_mode, set_scene_scene_transition_override or set_video_settings, which are read back after the command instead. ([specs/obs-studio.yaml:1217](specs/obs-studio.yaml#L1217))
- [ ] Check that GetCurrentSceneTransitionCursor follows the studio-mode T-bar while it is dragged in OBS or held part-way by set_t_bar_position with release off, that dragging the T-bar sends SceneTransitionStarted, and that the cursor reads 1.0 once an interrupted transition stops. ([specs/obs-studio.yaml:1128](specs/obs-studio.yaml#L1128))

## obsbot-tail — OBSBOT Tail 2 and Tail Air (OSC)

- [ ] Check whether the camera sends its UDP replies to the sender's port or always to port 57120, and that replies arrive when the core sends from feedback_port 57120. ([specs/obsbot-tail.yaml:405](specs/obsbot-tail.yaml#L405))
- [ ] Check that a Tail 2 answers GetDeviceInfo, GetZoomInfo and GetGimbalPosInfo, or only answers Connected with its full state (as a third-party integration reports), and which messages that state comes in. ([specs/obsbot-tail.yaml:412](specs/obsbot-tail.yaml#L412))
- [ ] Check the DeviceInfo device type number a Tail 2 and a Tail Air report, and whether SetShutterSpeed takes 12.5, 6.25 and 2.5 in any form. ([specs/obsbot-tail.yaml:418](specs/obsbot-tail.yaml#L418))
- [ ] Check that SetGimbalUp and SetGimbalLeft with speed 0 together stop a move in any direction. ([specs/obsbot-tail.yaml:425](specs/obsbot-tail.yaml#L425))

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

## omt — Open Media Transport (OMT) source

- [ ] Confirm a source accepts a metadata-only receiver and sends its OMTInfo, connection metadata and tally on accepting it, before any subscription. ([specs/omt.yaml:121](specs/omt.yaml#L121))
- [ ] Confirm tally from a metadata-only receiver is combined into the source's tally, and is dropped when that connection closes. ([specs/omt.yaml:112](specs/omt.yaml#L112))
- [ ] Confirm metadata without a terminating NUL is accepted by senders other than libomtnet (the protocol document says the length includes a NUL). ([specs/omt.yaml:131](specs/omt.yaml#L131))
- [ ] On a PTZ source announcing OMTPTZ Protocol VISCA, confirm inband commands reach the camera and the reply carries the same Sequence. ([specs/omt.yaml:140](specs/omt.yaml#L140))

## omt-discovery — Open Media Transport (OMT) Discovery Server

- [ ] Check whether a newly connected client receives the current registrations, or only later changes (the server sends them before the subscription can arrive). ([specs/omt-discovery.yaml:56](specs/omt-discovery.yaml#L56))

## ontime — Ontime

- [ ] Record the WebSocket pushes of Ontime 4 as sent (tag or type, and payload shape): whole runtime objects, section patches or per-section messages, and whether each section arrives whole. ([specs/ontime.yaml:1048](specs/ontime.yaml#L1048))
- [ ] Confirm that every client gets a runtime snapshot on connecting to /ws, and that {"tag":"poll"} and {"tag":"version"} are answered as the WebSocket page shows. ([specs/ontime.yaml:540](specs/ontime.yaml#L540))
- [ ] Record Ontime's WebSocket notice that the rundown or custom fields changed (Companion reads a refetch tag), so the 15-second rundown poll can follow it instead. ([specs/ontime.yaml:1052](specs/ontime.yaml#L1052))
- [ ] Confirm GET /data/rundowns/current and the entry writes on /data/rundowns/<id>/entry and /data/rundowns/<id>/<entry> (the Project data page writes /data/rundown/...), and record what POST, PUT and DELETE answer. ([specs/ontime.yaml:1059](specs/ontime.yaml#L1059))
- [ ] Check whether event indexes in /api/start/index and /api/load/index count from 0 or 1. ([specs/ontime.yaml:1071](specs/ontime.yaml#L1071))
- [ ] Confirm /api/load/cue/<cue> loads by cue (the HTTP page writes /api/load/index/<event-cue>). ([specs/ontime.yaml:1073](specs/ontime.yaml#L1073))
- [ ] Confirm auxiliary timers 2 and 3 answer /api/auxtimer/2/... and /3/... like timer 1, and that secondarySource takes aux1 to aux3 (and aux). ([specs/ontime.yaml:1074](specs/ontime.yaml#L1074))
- [ ] Confirm /api/message/timer takes text, visible and blink as query parameters, as it takes secondarySource and blackout. ([specs/ontime.yaml:1081](specs/ontime.yaml#L1081))
- [ ] Check whether the HTTP API and /ws take the session password's hashed token as Authorization: Bearer, and what they answer without it. ([specs/ontime.yaml:1036](specs/ontime.yaml#L1036))
- [ ] Record the shape of GET /data/custom-fields (label, type and colour per key are assumed). ([specs/ontime.yaml:1026](specs/ontime.yaml#L1026))
- [ ] Record the status and body of a refused playback command (an unknown event id or cue). ([specs/ontime.yaml:190](specs/ontime.yaml#L190))

## ontime-osc — Ontime (OSC API)

- [ ] Check whether the boolean arguments (blackout, skip, visible, blink) are taken as the strings true and false, or need OSC T/F or an int. ([specs/ontime-osc.yaml:364](specs/ontime-osc.yaml#L364))
- [ ] Confirm /ontime/message/timer/text, /visible and /blink exist beside the documented blackout and secondarySource. ([specs/ontime-osc.yaml:365](specs/ontime-osc.yaml#L365))
- [ ] Confirm /ontime/auxtimer/2/... and /3/... act on auxiliary timers 2 and 3. ([specs/ontime-osc.yaml:368](specs/ontime-osc.yaml#L368))
- [ ] Check whether integer arguments sent as OSC int32 are taken for millisecond values and indexes, or must be floats or strings. ([specs/ontime-osc.yaml:88](specs/ontime-osc.yaml#L88))

## onvif-ptz — ONVIF PTZ cameras (PTZ, imaging, media and device services)

- [ ] Check which cameras in use accept HTTP Digest for every service and which only WS-Security UsernameToken (auth wsse), that the UsernameToken in the SOAP header is accepted as the core writes it (mustUnderstand, PasswordDigest, Base64 nonce, Created in UTC to the second), how far the device's clock may differ, and the status of its NotAuthorized fault (400 or 401). ([specs/onvif-ptz.yaml:62](specs/onvif-ptz.yaml#L62))
- [ ] Collect the PTZ, media and imaging service paths common cameras list in GetServices (ptz_service, PTZ, services and others) to choose the defaults. ([specs/onvif-ptz.yaml:80](specs/onvif-ptz.yaml#L80))
- [ ] Confirm that a move naming no space uses the generic spaces on common cameras, and how a missing profile_token is answered (SOAP fault status). ([specs/onvif-ptz.yaml:897](specs/onvif-ptz.yaml#L897))
- [ ] Check that a camera's SOAP replies use the element names the rules read (GetServicesResponse/Service, PTZStatus, FocusStatus20, ImagingSettings) with any namespace prefix. ([specs/onvif-ptz.yaml:630](specs/onvif-ptz.yaml#L630))
- [ ] Record an imaging GetStatus reply: FocusStatus20 is read here; check whether older devices answer FocusStatus instead. ([specs/onvif-ptz.yaml:778](specs/onvif-ptz.yaml#L778))

## openlp — OpenLP

- [ ] Confirm that POST endpoints take their arguments as a JSON body with Content-Type application/json. ([specs/openlp.yaml:102](specs/openlp.yaml#L102))
- [ ] Confirm that the login token goes in the Authorization header with nothing before it, whether it stays valid across OpenLP restarts, that a missing or expired token answers 401 (on which the session logs in again), and that a wrong username or password answers the login 401. ([specs/openlp.yaml:58](specs/openlp.yaml#L58))
- [ ] Confirm that the websocket on 4317 sends its state in binary frames, sends the current state when a client connects, and needs no message from the client. ([specs/openlp.yaml:387](specs/openlp.yaml#L387))
- [ ] Confirm that the websocket's slide field is the selected slide's index from 0 (its documentation repeats the service field's words). ([specs/openlp.yaml:357](specs/openlp.yaml#L357))
- [ ] Check what the websocket's display, theme and blank fields hold for each display mode (desktop assumed for display). ([specs/openlp.yaml:398](specs/openlp.yaml#L398))
- [ ] Check how /service/show tells a position from an id, and whether positions count from 0 or 1. ([specs/openlp.yaml:327](specs/openlp.yaml#L327))
- [ ] Check which method /controller/clear/<controller> takes (POST assumed). ([specs/openlp.yaml:174](specs/openlp.yaml#L174))
- [ ] Check whether plugin item ids sent as JSON strings are accepted for songs, whose search results may give numbers. ([specs/openlp.yaml:267](specs/openlp.yaml#L267))
- [ ] Check what set_theme answers (200 with the name assumed from the documentation) and the bible search option names set_search_option takes. ([specs/openlp.yaml:165](specs/openlp.yaml#L165))

- [ ] Confirm the websocket sends a message for every change of the live item, slide and service, so re-reading on each one keeps the live item and service list current, and that the one-minute safety poll is enough for anything it misses. ([specs/openlp.yaml:399](specs/openlp.yaml#L399))
- [ ] Confirm live-item answers {} (status 200) while nothing is live, the case that removes live_item from state. ([specs/openlp.yaml:427](specs/openlp.yaml#L427))

## opensong — OpenSong

- [ ] Confirm what OpenSong expects in Authorization for POST requests: Basic followed by the base64 of the key alone, as the API page says (not of user:password), and what it answers a wrong key with (a Basic challenge, 401, is assumed). ([specs/opensong.yaml:40](specs/opensong.yaml#L40))
- [ ] Confirm the API's default port (8082; the page also says 8080 once) and where it is enabled and the key set in OpenSong's settings. ([specs/opensong.yaml:37](specs/opensong.yaml#L37))
- [ ] Confirm that percent-encoded names (spaces as %20, commas as %2C) in path segments and param:value parts are decoded by OpenSong, and what a POST answers on success (status 200 with an XML response is assumed). ([specs/opensong.yaml:168](specs/opensong.yaml#L168))
- [ ] Record the screen mode codes and names in the status for black, white, hide, logo and freeze (only N, normal, is documented). ([specs/opensong.yaml:417](specs/opensong.yaml#L417))
- [ ] Confirm the status reply while no presentation runs carries presentation running="0" with no children. ([specs/opensong.yaml:431](specs/opensong.yaml#L431))
- [ ] Check whether an alert message may hold a slash once percent-encoded (refused here), and which characters OpenSong's path parsing otherwise rejects. ([specs/opensong.yaml:397](specs/opensong.yaml#L397))
- [ ] Record the XML of the slide list, song list, song folders, set list and set slide list, so they can be read into state. ([specs/opensong.yaml:73](specs/opensong.yaml#L73))

## osc-listener — OSC received from any sender

- [ ] Check which TCP framing TouchOSC and Lemur send when set to TCP (SLIP as in OSC 1.1 is the default here, an int32 length prefix the alternative). ([specs/osc-listener.yaml:31](specs/osc-listener.yaml#L31))
- [ ] Check where TouchOSC and Lemur accept feedback over UDP: at the port they send from, or only at their configured receive port. ([specs/osc-listener.yaml:47](specs/osc-listener.yaml#L47))
- [ ] Check that a surface's press and release, and a repeated press, each arrive as a message event, including surfaces that send bundles. ([specs/osc-listener.yaml:70](specs/osc-listener.yaml#L70))
- [ ] Check that 64 simultaneous TCP senders is enough for a busy show. ([specs/osc-listener.yaml:87](specs/osc-listener.yaml#L87))

## oscpoint — OSCPoint (PowerPoint add-in)

- [ ] Confirm that OSCPoint 2.1 and later take section, folder and file names as a blob of UTF-8 text (its actions page says UTF-8 needs 2.1, while one note says v3.0.0), including names that are plain ASCII. ([specs/oscpoint.yaml:120](specs/oscpoint.yaml#L120))
- [ ] Confirm that name and presentation feedback arrives as one message holding the ASCII string and then the UTF-8 blob (`,sb`), and whether /oscpoint/v2/presentations and /oscpoint/v2/files carry a blob too (only a string is documented). ([specs/oscpoint.yaml:370](specs/oscpoint.yaml#L370))
- [ ] Confirm the argument layout of /oscpoint/v2/event: the ASCII file name then the UTF-8 blob (`,ssb`), slideshow_begin's slide number after them (`,ssbi`), the slide, click index and click count as ints, and add_in_startup's version as a string. ([specs/oscpoint.yaml:463](specs/oscpoint.yaml#L463))
- [ ] Check which OSC type /oscpoint/v2/files/enabled uses for its boolean (`T`/`F` and an int 1/0 are both read). ([specs/oscpoint.yaml:429](specs/oscpoint.yaml#L429))
- [ ] Confirm the UTF-8 notes address: the documentation says /oscpoint/slideshow/notes-utf8 and the Companion module listens on notes-utf-8; both are read. ([specs/oscpoint.yaml:516](specs/oscpoint.yaml#L516))
- [ ] Confirm that the media percent position runs from 0 to 100 (taken from the Companion module; the documentation gives no range). ([specs/oscpoint.yaml:274](specs/oscpoint.yaml#L274))
- [ ] Check what width and height set_wallpaper accepts (the Companion module limits both to 100-10000). ([specs/oscpoint.yaml:208](specs/oscpoint.yaml#L208))
- [ ] Confirm that /oscpoint/feedbacks/refresh resends every feedback, the v2 presentation and presentations JSON included, and that /oscpoint/files/list is harmless while file access is off. ([specs/oscpoint.yaml:356](specs/oscpoint.yaml#L356))
- [ ] Check the slide show state values (edit, running, paused from 2.1) and what OSCPoint sends for the slide show, section and media values when a slide show ends or a slide has no media. ([specs/oscpoint.yaml:501](specs/oscpoint.yaml#L501))

## panasonic-kairos — Panasonic KAIROS

- [ ] Confirm the control protocol answers an assignment or function with OK only after any change it pushes, or record the order, since replies name no request. ([specs/panasonic-kairos.yaml:71](specs/panasonic-kairos.yaml#L71))
- [ ] Record the paths of transitions, players, macros and audio in a typical production, so they can become typed commands. ([specs/panasonic-kairos.yaml:297](specs/panasonic-kairos.yaml#L297))
- [ ] Confirm the empty-line keep-alive every five seconds keeps the connection on current KAIROS software. ([specs/panasonic-kairos.yaml:48](specs/panasonic-kairos.yaml#L48))
- [ ] Confirm the REST API on current software answers the V1.0 paths, and record the macro states and the multiviewer preset field when set. ([specs/panasonic-kairos.yaml:191](specs/panasonic-kairos.yaml#L191))
- [ ] Confirm the REST login: user Kairos with the system ID, and the answer to a wrong one. ([specs/panasonic-kairos.yaml:54](specs/panasonic-kairos.yaml#L54))
- [ ] Record the tally values of inputs and scenes beyond 0 and 1. ([specs/panasonic-kairos.yaml:281](specs/panasonic-kairos.yaml#L281))

## panasonic-ptz — Panasonic PTZ

- [ ] On an HE camera, check whether menu up and down take DUP:1 and DDW:1, as sent, or the bare DUP and DDW the v1.12 HE rows show. ([specs/panasonic-ptz.yaml:13665](specs/panasonic-ptz.yaml#L13665))
- [ ] Check the white balance replies of AW-HE50/HE60/HE120/HE130 (AWB A and B as 2 and 3) against AW-HE40/HE65/HE70/UE70 (1 and 2, 3 ATW), as v1.12 states. ([specs/panasonic-ptz.yaml:13343](specs/panasonic-ptz.yaml#L13343))
- [ ] Check that a value one HE model lacks but a shared command allows (gain, AGC maximum, format, ND, audio input) is answered ER3. ([specs/panasonic-ptz.yaml:1872](specs/panasonic-ptz.yaml#L1872))
- [ ] Check the AW-HE120 R/B pedestal range 000-12C (-150 to +150) and that ORI/OBI replies are three digits while QGR/QGB answer two-digit OGR/OGB, which are not read. ([specs/panasonic-ptz.yaml:9286](specs/panasonic-ptz.yaml#L9286))
- [ ] Check which HE models answer OSD:50 (gamma level): v1.12 marks the reply row AK-UB300 only but lists HE models in its notes. ([specs/panasonic-ptz.yaml:9608](specs/panasonic-ptz.yaml#L9608))
- [ ] Check the AW-UE70's OSD:BF ELC limit codes (0 off, 1-4) against the UE family's 2-7. ([specs/panasonic-ptz.yaml:9650](specs/panasonic-ptz.yaml#L9650))
- [ ] Check that QSJ:03 is no longer sent to HE cameras (v1.12 marks OSJ:03-09 UE150 only) and that OSJ:01/02 replies never come from them. ([specs/panasonic-ptz.yaml:1805](specs/panasonic-ptz.yaml#L1805))
- [ ] Confirm the AW-HE130 colour temperature table (OSD:B1 000-078, 2000-15000 K) and the AW-HE40/UE70's 000-04B in 100 K steps. ([specs/panasonic-ptz.yaml:9328](specs/panasonic-ptz.yaml#L9328))
- [ ] Confirm the menu keys need DUP:1 (the AW-UE usage examples send DUP). ([specs/panasonic-ptz.yaml:13538](specs/panasonic-ptz.yaml#L13538))
- [ ] Confirm colour correction Yl_Yl_G uses OSD:1C/OSD:1D, not OSJ. ([specs/panasonic-ptz.yaml:13544](specs/panasonic-ptz.yaml#L13544))
- [ ] Confirm the AWB A/B numbering difference between control (1, 2) and query (2, 3), and get_scene 0-3 for scenes 1-4. ([specs/panasonic-ptz.yaml:13523](specs/panasonic-ptz.yaml#L13523))
- [ ] Confirm the AW-UE155 answers the AW-UE150 command set (assumed from the shared firmware; there is no AW-UE155 interface specification). ([specs/panasonic-ptz.yaml:1078](specs/panasonic-ptz.yaml#L1078))
- [ ] Confirm the AW-UE150 pan/tilt speed modes: the document lists 0 Normal (60 deg/s), 1 Fast (180), 2 Quick (60), where the other documents give 0 normal 60, 1 fast1 90, 2 fast2 180. ([specs/panasonic-ptz.yaml:9993](specs/panasonic-ptz.yaml#L9993))
- [ ] Confirm what awb_color_temperature (OSJ:4A) takes on AW-UE150/HE145/UE100: the usage example sends OSJ:4A:0, the command column OSJ:4A:[Data1]:[Data2]. ([specs/panasonic-ptz.yaml:2081](specs/panasonic-ptz.yaml#L2081))
- [ ] Confirm flip_detect_angle sends #FDA (the AW-UE150 usage example shows #SPF5A). ([specs/panasonic-ptz.yaml:5586](specs/panasonic-ptz.yaml#L5586))
- [ ] Confirm the AW-UE160 scene and user file name commands take OSL:91:[file]:[30 hex digits] (the usage examples send 32 hex digits without a file number). ([specs/panasonic-ptz.yaml:8238](specs/panasonic-ptz.yaml#L8238))
- [ ] Confirm the AW-UE160 takes the format code as two digits (OSA:87:01) and answers it the same way; the state rule accepts both. ([specs/panasonic-ptz.yaml:13619](specs/panasonic-ptz.yaml#L13619))
- [ ] Confirm awb_color_temperature and color_temperature_bch on the AW-UE160 take a trailing :0 (OSJ:4A:007D0:0, OSL:2F:007D0:0 in the usage examples). ([specs/panasonic-ptz.yaml:6409](specs/panasonic-ptz.yaml#L6409))
- [ ] Confirm OSD:3A answers 01 and 02 on the AW-UE160 both meaning on, as the document lists, and what ODT 2 means there. ([specs/panasonic-ptz.yaml:11241](specs/panasonic-ptz.yaml#L11241))
- [ ] Confirm the AW-UE160 custom iris window corners are single digits (OSL:CD:0:0:8:4 in the usage example, 00h-08h in the table). ([specs/panasonic-ptz.yaml:6308](specs/panasonic-ptz.yaml#L6308))
- [ ] Confirm SDR convert gain (OSI:43) takes every value from 74 to 80; the document lists 74 (-12 dB), 7B (-5 dB) and 80 (0 dB). ([specs/panasonic-ptz.yaml:7985](specs/panasonic-ptz.yaml#L7985))
- [ ] Confirm the AW-UE20/HE20/UE4 #TAA reply: Data1 is the tally LED and Data3 the command tally; the state reads Data1 as tally.red. ([specs/panasonic-ptz.yaml:13627](specs/panasonic-ptz.yaml#L13627))
- [ ] Confirm the streaming mode CGIs are set_stream_mode and get_stream_mode (the usage examples call set_priority_mode, and the AW-UE160 get reply is worded strean_mode). ([specs/panasonic-ptz.yaml:8672](specs/panasonic-ptz.yaml#L8672))
- [ ] Confirm each set_* CGI accepts a single parameter (the documents list all of a CGI's parameters together; the usage examples send some alone). ([specs/panasonic-ptz.yaml:13652](specs/panasonic-ptz.yaml#L13652))
- [ ] Confirm the Web CGIs of the AW-HE145 and AW-UE155 match the AW-UE150 supplement (only the AW-UE150 is documented). ([specs/panasonic-ptz.yaml:88](specs/panasonic-ptz.yaml#L88))
- [ ] Record the SRT latency and TTL ranges (the document gives no limits) and whether get_srt_info answers encryption or encrtyption. ([specs/panasonic-ptz.yaml:8777](specs/panasonic-ptz.yaml#L8777))
- [ ] Confirm the Admin CGIs answer a Digest challenge with the current firmware, and Basic with older firmware. ([specs/panasonic-ptz.yaml:136](specs/panasonic-ptz.yaml#L136))
- [ ] Confirm whether the 40 ms gap between pan-tilt head commands (chapter 4, restriction 1) counts from the previous command or from its reply; the core counts it from the reply, and the HE document's 130 ms between any two commands likewise. ([specs/panasonic-ptz.yaml:109](specs/panasonic-ptz.yaml#L109))

## panasonic-ptz-serial — Panasonic PTZ on RS-422

- [ ] On the RS-422 port, confirm that every camera command, queries included, is answered with ACK (06H) and then its reply between STX and ETX, as chapter 4-1 shows for a control command only. ([specs/panasonic-ptz-serial.yaml:10219](specs/panasonic-ptz-serial.yaml#L10219))
- [ ] Record what a refused camera command gets on the serial line: ACK and then ER1/ER2/ER3:<command> between STX and ETX, or the error alone (chapter 7 shows the IP sequence only); and that a pan-tilt error is eR<n>:<command> with CR. ([specs/panasonic-ptz-serial.yaml:10228](specs/panasonic-ptz-serial.yaml#L10228))
- [ ] Check that the camera sends update notifications on the RS-422 line with no registration, and whether the version (qSV, every 60 s), error (rER, every 30 s) and lens (lPI, every 300 ms) notifications of chapter 6 come there too. ([specs/panasonic-ptz-serial.yaml:10237](specs/panasonic-ptz-serial.yaml#L10237))
- [ ] Check the half-duplex pacing: whether a command sent right after a reply (less than 40 ms, or 16 ms on the AW-UE160, after the last pan-tilt command) is taken, and what happens to a command sent while the camera is sending a notification. ([specs/panasonic-ptz-serial.yaml:10210](specs/panasonic-ptz-serial.yaml#L10210))
- [ ] Confirm the AW-UE20/HE20 take these commands on the RS-422 connector with Serial Com. Protocol Panasonic, and that Panasonic is the factory setting. ([specs/panasonic-ptz-serial.yaml:10246](specs/panasonic-ptz-serial.yaml#L10246))
- [ ] Confirm the AW-UE160 leaves the factory at 9600 bps (its interface specification lists 9600, 38400 and 115200 without marking a default). ([specs/panasonic-ptz-serial.yaml:87](specs/panasonic-ptz-serial.yaml#L87))
- [ ] Confirm the AW-UE30, AW-UE40 and AW-UE50 have the RS-422 port the shared AW-UE80/UE50/UE40/UE30 interface specification describes (chapter 4-1 names no model). ([specs/panasonic-ptz-serial.yaml:94](specs/panasonic-ptz-serial.yaml#L94))
- [ ] Confirm the 40 ms gap between pan-tilt head commands (16 ms on the AW-UE160) on the RS-422 line, counted here from each reply, and that camera commands (STX ... ETX) need no gap of their own. ([specs/panasonic-ptz-serial.yaml:72](specs/panasonic-ptz-serial.yaml#L72))

## panasonic-projector — Panasonic projectors

- [ ] Confirm the NTCONTROL hash against a projector in protect mode: MD5 of "user:password:random" as 32 lowercase hex digits (the manual's example gives no random number to check against). ([specs/panasonic-projector.yaml:54](specs/panasonic-projector.yaml#L54))
- [ ] Confirm the projector closes the connection after every answer on all series, and how quickly it accepts the next connection. ([specs/panasonic-projector.yaml:9094](specs/panasonic-projector.yaml#L9094))
- [ ] Confirm a set command's network reply echoes the command after "00" (as "00PON"), and the serial reply is the bare data without "00". ([specs/panasonic-projector.yaml:9094](specs/panasonic-projector.yaml#L9094))
- [ ] Check which serial speeds the older series offer (the PT-RZ120's menu gives 9600, 19200 and 38400). ([specs/panasonic-projector.yaml:57](specs/panasonic-projector.yaml#L57))
- [ ] Check that commands addressed to ZZ are answered with RESPONSE (ID ALL) off on a single projector. ([specs/panasonic-projector.yaml:16368](specs/panasonic-projector.yaml#L16368))
- [ ] Check whether commands over the network also need the 60 s after the light source lights that the RS-232C section asks for (the core waits only on the serial port), and whether a power read during lighting answers 001 (the core starts the 60 s when it first reads on after standby). ([specs/panasonic-projector.yaml:16369](specs/panasonic-projector.yaml#L16369))
- [ ] Check the older section-per-command lists against a projector where their titles and bytes disagree (GMFI2 printed GMFI1, QJB as QJK, QFI as QEI, and value conflicts such as SHIFT -327 and -127). ([specs/panasonic-projector.yaml:16353](specs/panasonic-projector.yaml#L16353))
- [ ] Confirm QPW codes other than 000 and 001 that some series may return (warm-up, cooling). ([specs/panasonic-projector.yaml:9094](specs/panasonic-projector.yaml#L9094))

## phabrix-qx — PHABRIX Qx (REST API)

- [ ] Check that GET /api answers once REST API is enabled, as the liveness probe relies on it. ([specs/phabrix-qx.yaml:35](specs/phabrix-qx.yaml#L35))
- [ ] Check that generating a standard takes {"action":"start"} alone, and what a PUT without a body does. ([specs/phabrix-qx.yaml:110](specs/phabrix-qx.yaml#L110))
- [ ] Record the reply bodies of the status, CRC, generator, audio, eye, jitter, event log and IP list requests, so state can be kept field by field. ([specs/phabrix-qx.yaml:358](specs/phabrix-qx.yaml#L358))
- [ ] Record the QuickConfig mode, gain type and pitch names the unit takes besides the manual's example, and the pathological type names besides CheckField. ([specs/phabrix-qx.yaml:166](specs/phabrix-qx.yaml#L166))
- [ ] Check that {"action":"disable"} turns off an event log source, and which sources besides restApi exist. ([specs/phabrix-qx.yaml:219](specs/phabrix-qx.yaml#L219))
- [ ] Check the statuses the unit answers PUT requests with (200, 201 or 204 taken as success) and a request outside its boot mode. ([specs/phabrix-qx.yaml:260](specs/phabrix-qx.yaml#L260))
- [ ] Check that browse reaches the tree with percent-encoded segments as the manual's examples show (YCbCr%3A422%3A10, 100%25%20Bars). ([specs/phabrix-qx.yaml:60](specs/phabrix-qx.yaml#L60))

## pharos-designer — Pharos Designer controllers (HTTP and WebSocket API)

- [ ] With security enabled, check that POST /authenticate without original_url answers {"token": ...} with a 2xx, that later authenticated answers carry a new token as a top-level token attribute (taken in place of the old one), that an expired token is answered 401, and what wrong credentials produce. ([specs/pharos-designer.yaml:57](specs/pharos-designer.yaml#L57))
- [ ] Check whether resending the subscriptions every 10 s with the {} keep-alive duplicates broadcasts, and that the timeline broadcast is labeled "timeline". ([specs/pharos-designer.yaml:1173](specs/pharos-designer.yaml#L1173))
- [ ] Check that sending fade 0, delay 0, path Default and conditions true when not asked for behaves as leaving them out, and the endpoint the Ping API really uses. ([specs/pharos-designer.yaml:1183](specs/pharos-designer.yaml#L1183))
- [ ] On an LPC or TPC, check whether subscribing to content_target is refused harmlessly. ([specs/pharos-designer.yaml:1195](specs/pharos-designer.yaml#L1195))
- [ ] Check the HTTPS certificate controllers present, and the status codes of the POST/PUT/DELETE endpoints (200, 202 or 204). ([specs/pharos-designer.yaml:1200](specs/pharos-designer.yaml#L1200))
- [ ] Check that the WebSocket takes the token as each message's token attribute (subscriptions and keep-alive) and that the keep-alive renews the WebSocket's session; record the all-zero answer to a timed-out session. ([specs/pharos-designer.yaml:909](specs/pharos-designer.yaml#L909))

## philips-sicp — Philips professional displays (SICP)

- [ ] Confirm the command set of current models (Q, D, C, E and T Line, 55BDL4107X video walls) against SICP V2.03, the newest public version; record get_platform and get_sicp_version on each. ([specs/philips-sicp.yaml:947](specs/philips-sicp.yaml#L947))
- [ ] Record whether a display answers over TCP in standby in each power save mode, and whether set_power on is acknowledged from standby. ([specs/philips-sicp.yaml:941](specs/philips-sicp.yaml#L941))
- [ ] Confirm replies carry the group byte (and none with group_byte off on pre-1.86 firmware). ([specs/philips-sicp.yaml:52](specs/philips-sicp.yaml#L52))
- [ ] Record the input report (AD): whether a source type byte (FD) comes first, and the playlist byte; check 0x00 is taken for no playlist where Philips's examples send 0x09. ([specs/philips-sicp.yaml:438](specs/philips-sicp.yaml#L438))
- [ ] Confirm sending one video parameter with the others as last read leaves them unchanged, and the Phoenix 2.0 ranges (sharpness 0-10, signed tint). ([specs/philips-sicp.yaml:485](specs/philips-sicp.yaml#L485))
- [ ] Confirm the volume message takes two bytes on current models and one on Eagle and Himalaya 1. ([specs/philips-sicp.yaml:566](specs/philips-sicp.yaml#L566))
- [ ] Record the failover list's length per model. ([specs/philips-sicp.yaml:454](specs/philips-sicp.yaml#L454))
- [ ] Record a tiling report on a zero-bezel and a 5x5 model, and check the position and size encoding (the document's formula fails when the columns equal the row width). ([specs/philips-sicp.yaml:676](specs/philips-sicp.yaml#L676))
- [ ] Record whether the schedule report echoes its page, and whether a page set with D1 0x10 is page 1 disabled (the document's example). ([specs/philips-sicp.yaml:759](specs/philips-sicp.yaml#L759))
- [ ] Record whether the multi-window report (FD) starts with the window number. ([specs/philips-sicp.yaml:792](specs/philips-sicp.yaml#L792))
- [ ] Confirm the operating hours report is a big-endian 16-bit count of hours. ([specs/philips-sicp.yaml:724](specs/philips-sicp.yaml#L724))

## piclock — PiClock studio clock (tally server)

- [ ] Confirm the AUTH digest is lowercase hexadecimal SHA-512 of the challenge followed by the secret. ([specs/piclock.yaml:202](specs/piclock.yaml#L202))
- [ ] Confirm region commands take the region number directly before the command name (0SETTALLY:...). ([specs/piclock.yaml:208](specs/piclock.yaml#L208))
- [ ] Confirm SETCOUNTDOWN's empty flash field and SETLAYOUT's empty legacy field are accepted. ([specs/piclock.yaml:122](specs/piclock.yaml#L122))
- [ ] Confirm the clock sends GPI on connecting and on each change, and that a 16-bit word is read bit 0 first. ([specs/piclock.yaml:190](specs/piclock.yaml#L190))

## pigpio — Raspberry Pi GPIO (pigpio daemon)

- [ ] Check against pigpiod (version 79) on a Raspberry Pi 4 that NOIB on a second connection answers the handle and then streams reports for the GPIO NB names on the first, as the C interface describes for socket notifications. ([specs/pigpio.yaml:7](specs/pigpio.yaml#L7))

## pikvm — PiKVM

- [ ] Check the PiKVM Switch commands' answers (the reference shows FIXME for each) and that status 200 means the switch acted. ([specs/pikvm.yaml:694](specs/pikvm.yaml#L694))
- [ ] Check on current KVMD that the websocket events still carry whole states as the reference's examples show (hid_state with outputs and jiggler, msd_state with storage.images by name), and that ocr_right and ocr_bottom are the region's bounds rather than its width and height. ([specs/pikvm.yaml:681](specs/pikvm.yaml#L681))
- [ ] Confirm the GPIO switch route is /api/gpio/switch (the reference's heading says /gpio/switch, its example /api/gpio/switch). ([specs/pikvm.yaml:342](specs/pikvm.yaml#L342))

## pixotope — Pixotope

- [ ] Record the Gateway's HTTP answer bodies (to a Set, a Get, a Call, and a Call that times out), so replies can be read into state. ([specs/pixotope.yaml:341](specs/pixotope.yaml#L341))
- [ ] Check that ~LOCAL~-Engine addresses the Engine on the Gateway's machine, as the Editor's examples write it. ([specs/pixotope.yaml:353](specs/pixotope.yaml#L353))
- [ ] Check which API version the URL should carry for current releases (24.1.0 is used by default). ([specs/pixotope.yaml:81](specs/pixotope.yaml#L81))
- [ ] Confirm the Store answers a Get of State.General.FrameRate, used as the liveness probe. ([specs/pixotope.yaml:77](specs/pixotope.yaml#L77))
- [ ] Confirm a client subscribes on 16201 and publishes on 16202 (the page labels 16201 the subscriber and 16202 the publisher, and its example connects a PUB socket to 16202). ([specs/pixotope.yaml:90](specs/pixotope.yaml#L90))
- [ ] Confirm the Data Hub speaks ZMTP 3.0 or later with the NULL mechanism (no CURVE when Pixotope's encryption is off) and takes ZMTP 3.0 subscription messages. ([specs/pixotope.yaml:372](specs/pixotope.yaml#L372))
- [ ] Confirm services broadcast their Updates to BROADCAST with Source and Name in the topic, in that order after Type and Target, so the subscription prefix and the rule match them. ([specs/pixotope.yaml:374](specs/pixotope.yaml#L374))
- [ ] Confirm Log messages are published with a Source, and that a topic beginning {"Type":"Log" reaches them. ([specs/pixotope.yaml:376](specs/pixotope.yaml#L376))
- [ ] Confirm the Store answers a Get of State published with RespondTo set to the client name by an Update to that name holding the whole state, and where RespondTo goes in a Get's topic. ([specs/pixotope.yaml:380](specs/pixotope.yaml#L380))

## pjlink — PJLink

- [ ] With authentication auto, check how a projector that predates 2.10 answers "PJLINK 2", and whether it counts it as a failed login. ([specs/pjlink.yaml:213](specs/pjlink.yaml#L213))
- [ ] Check whether AVMT ? ever returns 10 or 20, and how they read. ([specs/pjlink.yaml:234](specs/pjlink.yaml#L234))
- [ ] Confirm the power-on response is "%1POWR=OK". ([specs/pjlink.yaml:240](specs/pjlink.yaml#L240))
- [ ] On a Class 2 projector, find how the controller address for status notifications is registered. ([specs/pjlink.yaml:254](specs/pjlink.yaml#L254))

## planningcenter-services — Planning Center Services LIVE

- [ ] Confirm the LIVE actions are POSTs to /service_types/{id}/plans/{id}/live/<action> without a live id (the documentation graph prints them under a series path with one). ([specs/planningcenter-services.yaml:183](specs/planningcenter-services.yaml#L183))
- [ ] Confirm go_to_next_item, go_to_previous_item and toggle_control answer 200 with the Live resource, and honour include=current_item_time,next_item_time. ([specs/planningcenter-services.yaml:186](specs/planningcenter-services.yaml#L186))
- [ ] Confirm toggle_control takes control from another user who holds it, rather than failing, and what it answers when it gives control up. ([specs/planningcenter-services.yaml:197](specs/planningcenter-services.yaml#L197))
- [ ] Record what the Live resource's relationships hold (the documentation's example has none): current_item_time, next_item_time and controller ids, and what they are when LIVE is not running. ([specs/planningcenter-services.yaml:1075](specs/planningcenter-services.yaml#L1075))
- [ ] Check that a move without control answers 403 and not another status. ([specs/planningcenter-services.yaml:111](specs/planningcenter-services.yaml#L111))
- [ ] Confirm GET /services/v2 answers any valid credential, and 401 for an expired OAuth token or a revoked Personal Access Token. ([specs/planningcenter-services.yaml:117](specs/planningcenter-services.yaml#L117))
- [ ] Record the format of LIVE times (ItemTime live_start_at and live_end_at, PlanTime live_starts_at): ISO 8601 as documented, or 2026/06/16 11:24:29 -0500 as seen. ([specs/planningcenter-services.yaml:930](specs/planningcenter-services.yaml#L930))
- [ ] Confirm per_page=100 is accepted on items, plan_times, item_notes and live_controllers, and how a plan with more than 100 items is paged. ([specs/planningcenter-services.yaml:912](specs/planningcenter-services.yaml#L912))
- [ ] Confirm the OAuth token endpoint is https://api.planningcenteronline.com/oauth/token (taken from Planning Center's example app and its OAuth library's default path, not from a documented URL) and that it accepts the refresh token grant with client_id and client_secret in the form body. ([specs/planningcenter-services.yaml:107](specs/planningcenter-services.yaml#L107))
- [ ] Record whether a refresh returns a new refresh token (rotation) and whether the old one then stops working, and what a revoked or expired refresh token answers (400 invalid_grant expected). ([specs/planningcenter-services.yaml:857](specs/planningcenter-services.yaml#L857))
- [ ] Confirm a list that fits on one page has top-level links with self and no next, and a longer one links.next (taken from Planning Center's pco_api_ruby README): only a list without next replaces a plan's items, its plan times or the service types. ([specs/planningcenter-services.yaml:1196](specs/planningcenter-services.yaml#L1196))
- [ ] Confirm the Live resource's included array holds only the current and next item times, and is empty or absent when LIVE is not running: each Live reply replaces plans.<plan_id>.item_times. ([specs/planningcenter-services.yaml:1096](specs/planningcenter-services.yaml#L1096))
- [ ] Confirm the API honours X-PCO-API-Version 2018-11-01 on every request, for an application whose default version is another. ([specs/planningcenter-services.yaml:65](specs/planningcenter-services.yaml#L65))
- [ ] Confirm creates (plans, items, notes, plan times) answer 201 with the new resource, updates 200, and deletes and item_reorder 204. ([specs/planningcenter-services.yaml:352](specs/planningcenter-services.yaml#L352))
- [ ] Confirm a PATCH whose attributes hold one field leaves the others unchanged. ([specs/planningcenter-services.yaml:430](specs/planningcenter-services.yaml#L430))
- [ ] Confirm an item's song, arrangement and key are set with song_id, arrangement_id and key_id as attributes, and that item_type takes only header on create. ([specs/planningcenter-services.yaml:417](specs/planningcenter-services.yaml#L417))
- [ ] Confirm item_reorder must name every item of the plan, and what it answers when one is left out. ([specs/planningcenter-services.yaml:498](specs/planningcenter-services.yaml#L498))
- [ ] Confirm creating an item note or plan note needs its category relationship, and that the note's category_name is answered. ([specs/planningcenter-services.yaml:519](specs/planningcenter-services.yaml#L519))
- [ ] Confirm include=song,arrangement,key on the plan's items answers Song, Arrangement and Key in included, with bpm a number and sequence an array, and record how much the 10-second poll grows with lyrics included. ([specs/planningcenter-services.yaml:1059](specs/planningcenter-services.yaml#L1059))
- [ ] Confirm a plan time created with an ISO 8601 time and offset is answered in the organization's time zone. ([specs/planningcenter-services.yaml:621](specs/planningcenter-services.yaml#L621))
- [ ] Confirm the where[title] query on songs matches part of a title or only the whole of it. ([specs/planningcenter-services.yaml:686](specs/planningcenter-services.yaml#L686))
- [ ] Confirm Attachment open works under an item's attachments path as under media's, and how long the attachment_url lasts. ([specs/planningcenter-services.yaml:823](specs/planningcenter-services.yaml#L823))

## posistagenet — PosiStageNet

- [ ] Check that consoles and media servers take the core's trackers with version 2.03 (high 2, low 3) in the packet header, and with every tracker's position sent even when only other fields were set. ([specs/posistagenet.yaml:139](specs/posistagenet.yaml#L139))
- [ ] With a tracking server, check that its trackers appear under its address, that a tracker removed on the server leaves state after the next PSN_INFO, and that frames split over several packets are read whole. ([specs/posistagenet.yaml:131](specs/posistagenet.yaml#L131))

## probel-swp02 — Probel / Grass Valley SW-P-02 routers

- [ ] Confirm SW-P-02 over TCP carries the same messages as the serial line, with nothing added, and find the port routers and gateways use. ([specs/probel-swp02.yaml:139](specs/probel-swp02.yaml#L139))
- [ ] Check that a router answers within the 1 s timeout, and what a router that does not implement a command (the salvo) sends. ([specs/probel-swp02.yaml:114](specs/probel-swp02.yaml#L114))
- [ ] Record what multiplier bit 3 means on each router tested (bad source, crosspoint updates disabled, or unused). ([specs/probel-swp02.yaml:135](specs/probel-swp02.yaml#L135))
- [ ] On an NK-3RD, confirm odd parity is needed and that CONNECTED is broadcast after a take. ([specs/probel-swp02.yaml:62](specs/probel-swp02.yaml#L62))

## probel-swp08 — Probel / Grass Valley SW-P-08 routers

- [ ] Confirm the TCP port in use (2008 assumed; SW-P-08 over IP leaves it to the controller's configuration). ([specs/probel-swp08.yaml:489](specs/probel-swp08.yaml#L489))
- [ ] Confirm matrix and level 1 are wire 0 on each controller family, and 1 on System 2. ([specs/probel-swp08.yaml:430](specs/probel-swp08.yaml#L430))
- [ ] Check what a controller sends after a connect to a protected destination (nothing is assumed, so it times out). ([specs/probel-swp08.yaml:445](specs/probel-swp08.yaml#L445))
- [ ] Check that a controller acknowledges DUAL CONTROLLER STATUS REQUEST (08) with DLE ACK even when it does not implement it, since that ACK is the liveness check. ([specs/probel-swp08.yaml:461](specs/probel-swp08.yaml#L461))
- [ ] Check whether a controller NAKs commands it does not implement, answers INVALID MESSAGE (99), or stays silent. ([specs/probel-swp08.yaml:472](specs/probel-swp08.yaml#L472))
- [ ] Check which device numbers a remote client may protect with, and that PROTECT CONNECTED reports state 3 (OEM) with that device. ([specs/probel-swp08.yaml:502](specs/probel-swp08.yaml#L502))
- [ ] Confirm the protect tally dump request (19) layout: matrix/level byte then a two-byte first destination. ([specs/probel-swp08.yaml:550](specs/probel-swp08.yaml#L550))
- [ ] Confirm the tie-line connect (111) bytes 7 and 8 are the source association number. ([specs/probel-swp08.yaml:550](specs/probel-swp08.yaml#L550))
- [ ] Check whether CONNECTED messages follow a salvo go on XD and Eclipse routers (Issue 30 says none; go-acp reports controllers that expect them). ([specs/probel-swp08.yaml:523](specs/probel-swp08.yaml#L523))
- [ ] On dual controllers over IP, record the unsolicited DUAL CONTROLLER STATUS RESPONSE on a changeover. ([specs/probel-swp08.yaml:495](specs/probel-swp08.yaml#L495))
- [ ] Record name responses with name counts below 16, where a community parser reports the names starting one byte later. ([specs/probel-swp08.yaml:541](specs/probel-swp08.yaml#L541))
- [ ] On Ultrix, confirm the SW-P-08 server port (8910) and how the Protocol Variant setting answers general and extended commands. ([specs/probel-swp08.yaml:198](specs/probel-swp08.yaml#L198))
- [ ] On an NK-3RD, confirm matrix 0 and level 0 on the wire address NK level 1, and that no tally dump is answered. ([specs/probel-swp08.yaml:213](specs/probel-swp08.yaml#L213))

## propresenter — ProPresenter

- [ ] Confirm the network API port in use (50001 assumed). ([specs/propresenter.yaml:2555](specs/propresenter.yaml#L2555))
- [ ] Confirm /v1/timers/current lists every configured timer: each poll replaces timers, so a deleted timer leaves. ([specs/propresenter.yaml:1987](specs/propresenter.yaml#L1987))
- [ ] Record what /v1/transport/{layer}/current and /time, /v1/presentation/slide_index, /v1/announcement/slide_index, /v1/presentation/focused and the active and focused playlist endpoints answer when nothing is on the layer or active (empty values, null or an error status): a value a reply leaves out keeps what it had. ([specs/propresenter.yaml:2442](specs/propresenter.yaml#L2442))
- [ ] Confirm that the id of a look, macro, prop, message, clear group, timer, library, mask, group and stage layout is an object with uuid, name and index, as the document's examples show (its schema gives the field by reference only). ([specs/propresenter.yaml:2123](specs/propresenter.yaml#L2123))
- [ ] Confirm the stage layout map's screen and layout are id objects with a uuid. ([specs/propresenter.yaml:2048](specs/propresenter.yaml#L2048))
- [ ] Check that media and audio playlist folders list their contents under children, and playlist folders under playlists, and how deep folders nest (three levels are read). ([specs/propresenter.yaml:2172](specs/propresenter.yaml#L2172))
- [ ] Check the units of a timer's countdown duration, time of day and elapsed start and end times (seconds assumed). ([specs/propresenter.yaml:2457](specs/propresenter.yaml#L2457))
- [ ] Check that ProPresenter answers about 31 status requests a second without slowing playback, and whether a slower poll should be used on older computers. ([specs/propresenter.yaml:2577](specs/propresenter.yaml#L2577))
- [ ] Confirm that /v1/status/updates streams one JSON object per change in a chunked reply, for a future streaming transport. ([specs/propresenter.yaml:2593](specs/propresenter.yaml#L2593))
- [ ] Record what /v1/presentation/active and /v1/announcement/active answer when nothing is active (404, an empty object or null), and check that its groups list every group of the selected arrangement in order. ([specs/propresenter.yaml:1865](specs/propresenter.yaml#L1865))
- [ ] Check that reading /v1/presentation/active and /v1/announcement/active every 5 s, with every slide's text and notes, does not slow a large presentation. ([specs/propresenter.yaml:1802](specs/propresenter.yaml#L1802))
- [ ] Check that playlist_get by UUID answers id and items for a playlist inside folders, as the document gives it. ([specs/propresenter.yaml:2232](specs/propresenter.yaml#L2232))
- [ ] Check the media and audio playlist replies: the document gives media's items as an object with array fields and audio's reply as an array of playlists; an object with id and an items array is assumed for both. ([specs/propresenter.yaml:2260](specs/propresenter.yaml#L2260))
- [ ] Open POST /v1/status/updates with Accept application/json and confirm each update arrives as {"url", "data"} followed by a blank line (not as server-sent events), that url carries /v1 as the document's example does, and that data is what each endpoint's own GET returns; and record whether the stream ends when ProPresenter changes presentations. ([specs/propresenter.yaml:1756](specs/propresenter.yaml#L1756))

## ptp — PTP version 2 (IEEE 1588) network monitor

- [ ] On a SMPTE ST 2059-2 or AES67 network with a real grandmaster (and a boundary clock), confirm the Announce dataset, the best master and the clock accuracy and time source texts match what the grandmaster's own interface shows. ([specs/ptp.yaml:87](specs/ptp.yaml#L87))
- [ ] Confirm the Delay_Req (1) and Delay_Resp (9) message types and the slave role they show, taken from a BSD-licensed decoder rather than IEEE 1588's own text, against a real end-to-end network. ([specs/ptp.yaml:107](specs/ptp.yaml#L107))
- [ ] Check host_offset_ms on a host disciplined by PTP (it should stay within a millisecond or so) and on one that is not, and that sharing ports 319 and 320 with a PTP daemon on the same host works on Windows, Linux and macOS. ([specs/ptp.yaml:158](specs/ptp.yaml#L158))

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
- [ ] Check whether any HTTP query reports pan, tilt, zoom or focus position, power, time of flight, display info, the tracking overlay or the tracking start location (the document shows none; trackstart in get_image_conf may be the start location). ([specs/ptzoptics.yaml:1820](specs/ptzoptics.yaml#L1820))

## qlab — QLab

- [ ] Check whether a cue that completes on its own produces a stop event, and how pause, panic, stop-all and reset-all events are addressed. ([specs/qlab.yaml:14766](specs/qlab.yaml#L14766))
- [ ] Confirm that /update/.../playbackPosition arrives with no argument when a cue list's playhead is unset. ([specs/qlab.yaml:13847](specs/qlab.yaml#L13847))
- [ ] Confirm that /workspaces lists every open workspace, and that /workspace/{id}/... reads reach a workspace listening on another port. ([specs/qlab.yaml:13783](specs/qlab.yaml#L13783))
- [ ] Check that a workspace-addressed reply (/reply/workspace/{id}/cueLists and the like) arrives on that address, and that the rootless /runningOrPausedCues/shallow reply carries workspace_id, once per workspace. ([specs/qlab.yaml:14444](specs/qlab.yaml#L14444))
- [ ] Check that valuesForKeys answers a cue with the keys that apply to its type when the list holds others too, whether the others are left out or null, and that keys holding a slash (colorName/live) are accepted. ([specs/qlab.yaml:14038](specs/qlab.yaml#L14038))
- [ ] Check that QLab sends /update/.../cue_id/{id} when a cue starts, pauses, loads or stops, and for a newly created cue, so its values are read again. ([specs/qlab.yaml:13838](specs/qlab.yaml#L13838))
- [ ] Confirm the address of an update to the cue lists themselves: /update/workspace/{id}/cue_id/__root__ (QLab 5.5) or [root group of cue lists] before. ([specs/qlab.yaml:13830](specs/qlab.yaml#L13830))
- [ ] Check whether /cueLists reports flagged and armed as numbers or booleans (the dictionary shows both). ([specs/qlab.yaml:13852](specs/qlab.yaml#L13852))
- [ ] Check that a cue list's playheadID answers none when no cue stands by. ([specs/qlab.yaml:14035](specs/qlab.yaml#L14035))
- [ ] Check that startTime and endTime read as numbers of seconds on Audio and Video cues. ([specs/qlab.yaml:5213](specs/qlab.yaml#L5213))
- [ ] Check that a slice or last-slice play count of 0, which the range does not refuse, is rejected by QLab. ([specs/qlab.yaml:5309](specs/qlab.yaml#L5309))
- [ ] Check that /solo/{output} (QLab 5) addresses a cue output as /mute/channel/{output} does. ([specs/qlab.yaml:5743](specs/qlab.yaml#L5743))
- [ ] Check that valuesForKeys accepts anchor/x, scale/x and translation/x as keys, and what cueSize and quaternion read as. ([specs/qlab.yaml:6653](specs/qlab.yaml#L6653))
- [ ] Check that QLab 4's /origin takes two numbers, as QLab 5's synonym does. ([specs/qlab.yaml:6749](specs/qlab.yaml#L6749))
- [ ] Check that valuesForKeys accepts text/format and text/format/alignment as keys. ([specs/qlab.yaml:8769](specs/qlab.yaml#L8769))
- [ ] Check that a Light cue's setLight takes the level as a number, as the dashboard's does, and QLab 4's updateLightCommand a string and a number. ([specs/qlab.yaml:9695](specs/qlab.yaml#L9695))
- [ ] Check that a Group cue refuses mode 0 and 5, which the range allows, and that QLab 4's Fade cue /mode is its absolute or relative mode. ([specs/qlab.yaml:9797](specs/qlab.yaml#L9797))
- [ ] Check that parameterValue takes a number as an OSC float and other values as strings. ([specs/qlab.yaml:10757](specs/qlab.yaml#L10757))
- [ ] Check the MSC timecode ranges (hours to 23, frames to 29, subframes to 99), which the dictionary leaves to the format. ([specs/qlab.yaml:11579](specs/qlab.yaml#L11579))
- [ ] Check that valuesForKeys accepts timecodeTrigger/text and the playlist/... keys. ([specs/qlab.yaml:12327](specs/qlab.yaml#L12327))
- [ ] Check that /workspace/{id}/fullScreen, lightDashboard and dashboard/mode answer a read without an argument. ([specs/qlab.yaml:13036](specs/qlab.yaml#L13036))
- [ ] Check that the rootless /selectedCues/shallow is answered once per open workspace with its workspace_id and an empty array when nothing is selected, whether a QLab 5 workspace answers with the current cue list's selection only, and whether a selection change sends any /update message. ([specs/qlab.yaml:14455](specs/qlab.yaml#L14455))
- [ ] Check whether QLab pushes anything when an override or a window changes; they are re-read only with the workspace. ([specs/qlab.yaml:13806](specs/qlab.yaml#L13806))

## qlcplus — QLC+ (Web API)

- [ ] Check that QLC+ without -wa accepts a WebSocket opened with an empty Basic credential. ([specs/qlcplus.yaml:490](specs/qlcplus.yaml#L490))
- [ ] Record the messages QLC+ pushes for widget and function changes, so they can be read into state. ([specs/qlcplus.yaml:490](specs/qlcplus.yaml#L490))
- [ ] Check the value ranges widgets take (sliders, XY pads, speed dial factors, page and step numbers) and what sdResetChannel and sdResetUniverse answer. ([specs/qlcplus.yaml:490](specs/qlcplus.yaml#L490))
- [ ] On QLC+ 4, check that each documented QLC+API command and widget message works as on version 5. ([specs/qlcplus.yaml:511](specs/qlcplus.yaml#L511))
- [ ] Check that getFunctionStatus answers Running or Stopped exactly, and record the forms of getWidgetFunction's and getWidgetStatus's answers, which are kept as text. ([specs/qlcplus.yaml:473](specs/qlcplus.yaml#L473))

## qsys — Q-SYS

- [ ] Record how the Core answers a refused Logon (error object shape, with and without an id). ([specs/qsys.yaml:443](specs/qsys.yaml#L443))
- [ ] Check whether the Core's replies end with a NUL. ([specs/qsys.yaml:452](specs/qsys.yaml#L452))
- [ ] Record how each automatic poll after ChangeGroup.AutoPoll is framed (a result with the AutoPoll id or a ChangeGroup.Poll notification). ([specs/qsys.yaml:457](specs/qsys.yaml#L457))
- [ ] Confirm StatusGet with "params": 0 and Component.GetComponents with {} are accepted. ([specs/qsys.yaml:478](specs/qsys.yaml#L478))
- [ ] Check whether a Component.Get reply carries an id. ([specs/qsys.yaml:480](specs/qsys.yaml#L480))
- [ ] Find the unit of Mixer.SetCrossPointDelay's value. ([specs/qsys.yaml:491](specs/qsys.yaml#L491))
- [ ] Confirm LoopPlayer.Start accepts RefID and Seek at the top level. ([specs/qsys.yaml:496](specs/qsys.yaml#L496))
- [ ] On Designer emulation, check whether PA Router paging and Loop Player playback behave as on a Core. ([specs/qsys.yaml:543](specs/qsys.yaml#L543))

## qsys-ecp — Q-SYS (External Control Protocol)

- [ ] Check that the three-line subscription (cgc 1, cgsna 1 <ms>, sg sent in one write) is accepted, and that cgc and cgsna are silent on success. ([specs/qsys-ecp.yaml:378](specs/qsys-ecp.yaml#L378))
- [ ] Check login's answers (login_success, login_failed, and the socket closed after login_failed) and that a Core without Access Control answers login with an error rather than ignoring it. ([specs/qsys-ecp.yaml:77](specs/qsys-ecp.yaml#L77))
- [ ] Check that quoting every control name (cg "gain1") is accepted for names without spaces. ([specs/qsys-ecp.yaml:116](specs/qsys-ecp.yaml#L116))
- [ ] Check csvvr's form: the command reference shows it without the value count that csvv takes. ([specs/qsys-ecp.yaml:199](specs/qsys-ecp.yaml#L199))
- [ ] Check what ssl answers on success and on an unknown bank. ([specs/qsys-ecp.yaml:330](specs/qsys-ecp.yaml#L330))
- [ ] Check the cvv layout for a meter (count, strings, count, values, count, positions). ([specs/qsys-ecp.yaml:403](specs/qsys-ecp.yaml#L403))
- [ ] Check that a choice control's cmvv aspect 2 lists its choices as the strings, with zero values and positions, as the document's slope example shows, and note any vector control longer than 16 elements (its values and positions are not kept). ([specs/qsys-ecp.yaml:582](specs/qsys-ecp.yaml#L582))
- [ ] Check that an sg every 30 s keeps the connection open while a scheduled change group is pushing. ([specs/qsys-ecp.yaml:385](specs/qsys-ecp.yaml#L385))

## red-rcp2 — RED DSMC3 cameras (RCP2 over WebSocket)

- [ ] Check every message form, parameter id, value scale and enumeration in this spec against RED's RCP2 documents (behind a RED account login) or a camera; none comes from RED's own documentation. ([specs/red-rcp2.yaml:369](specs/red-rcp2.yaml#L369))
- [ ] Record the camera's answers to rcp_config and rcp_get_types on connecting, and whether the WebSocket path is / on port 9998. ([specs/red-rcp2.yaml:58](specs/red-rcp2.yaml#L58))
- [ ] Record an rcp_session message, to confirm its data is echoed back as it came, and how long the camera waits for the echo before dropping the connection. ([specs/red-rcp2.yaml:53](specs/red-rcp2.yaml#L53))
- [ ] Confirm the RECORD_FORMAT numbers, the EXTERNAL_TALLY state, opacity, style and thickness numbers, the ISO and color temperature ranges, and the units of SENSOR_FRAME_RATE and EXPOSURE_ADJUST. ([specs/red-rcp2.yaml:275](specs/red-rcp2.yaml#L275))

## renewedvision-pvp — Renewed Vision PVP

- [ ] Confirm the API port in use (8080 assumed). ([specs/renewedvision-pvp.yaml:874](specs/renewedvision-pvp.yaml#L874))
- [ ] Confirm every successful command answers 200, not another 2xx status. ([specs/renewedvision-pvp.yaml:912](specs/renewedvision-pvp.yaml#L912))
- [ ] Check whether Layer Blending is at /blend/layer/{id} (document) or /layerBlend/layer/{id} (Companion). ([specs/renewedvision-pvp.yaml:919](specs/renewedvision-pvp.yaml#L919))
- [ ] Record the opacity GET reply shape and the Layer Blending GET reply shape. ([specs/renewedvision-pvp.yaml:928](specs/renewedvision-pvp.yaml#L928))
- [ ] Check which body the transition POST accepts ({"transition": {...}} or {"value": uuid}). ([specs/renewedvision-pvp.yaml:930](specs/renewedvision-pvp.yaml#L930))
- [ ] Confirm the opacity range is 0.0 to 1.0. ([specs/renewedvision-pvp.yaml:934](specs/renewedvision-pvp.yaml#L934))
- [ ] On the PVP version in use, check whether negative layer or cue indexes still crash it. ([specs/renewedvision-pvp.yaml:903](specs/renewedvision-pvp.yaml#L903))
- [ ] Confirm /api/0/data/playlists holds every top-level playlist and group under playlist.children: each poll replaces playlists. ([specs/renewedvision-pvp.yaml:974](specs/renewedvision-pvp.yaml#L974))

## resolume — Resolume Arena / Avenue

- [ ] Record the shape of the composition message pushed on the websocket (bare object or wrapped as {"type", "value"}). ([specs/resolume.yaml:3370](specs/resolume.yaml#L3370))
- [ ] Confirm that layer, column, deck and group positions reported in state match the order in the composition list Resolume sends. ([specs/resolume.yaml:3262](specs/resolume.yaml#L3262))
- [ ] Confirm that setters sending only the changed property leave other properties unchanged. ([specs/resolume.yaml:3297](specs/resolume.yaml#L3297))
- [ ] Check what Resolume does with a speed, crossfader phase or tempo value outside the parameter's min and max. ([specs/resolume.yaml:3310](specs/resolume.yaml#L3310))
- [ ] Confirm opacity and master levels are 0.0 to 1.0 over REST. ([specs/resolume.yaml:3308](specs/resolume.yaml#L3308))
- [ ] Confirm a deck switch, deck open and deck close bring no new composition on the websocket (so the REST read is needed). ([specs/resolume.yaml:3356](specs/resolume.yaml#L3356))
- [ ] On Avenue, check whether get_deck, replace_deck and deck delete and duplicate answer 402. ([specs/resolume.yaml:3402](specs/resolume.yaml#L3402))
- [ ] Confirm effect chain offsets (add at, move to, delete, rename at) count from 1, matching effects.*.position. ([specs/resolume.yaml:3417](specs/resolume.yaml#L3417))
- [ ] Confirm a move body of effects/by-id/<id> (no leading slash, as the document's example) moves an effect from any chain. ([specs/resolume.yaml:3414](specs/resolume.yaml#L3414))
- [ ] Confirm an effect name with a space is accepted percent-encoded in effect:///video/<name>, and that /<preset> takes the preset's name. ([specs/resolume.yaml:3412](specs/resolume.yaml#L3412))
- [ ] Confirm choice parameters (crossfader side, blend mode, play direction, colour and the rest) are set by sending the option name as value. ([specs/resolume.yaml:3426](specs/resolume.yaml#L3426))
- [ ] Record the units and ranges of audio volume and pan, a layer's transition duration and a clip's speed, position and duration. ([specs/resolume.yaml:3439](specs/resolume.yaml#L3439))
- [ ] Check whether adding, moving or removing an effect, or moving a layer or clip, resends the composition on the websocket (the extension reads it again after its own). ([specs/resolume.yaml:3359](specs/resolume.yaml#L3359))
- [ ] Measure how often a playing clip's transport position is pushed, and whether effect parameters subscribe by /parameter/by-id/<id> like the rest. ([specs/resolume.yaml:3346](specs/resolume.yaml#L3346))

## restream — Restream

- [ ] Confirm a refresh at https://api.restream.io/oauth/token with client_id and client_secret in the form body (not Basic) succeeds, answers a new refresh token every time, and that the old one then answers 400 invalid_grant. ([specs/restream.yaml:135](specs/restream.yaml#L135))
- [ ] Confirm a refresh extends the refresh token's one-year life (refreshTokenExpiresIn) or keeps the original grant's. ([specs/restream.yaml:135](specs/restream.yaml#L135))
- [ ] Confirm an expired or revoked access token answers 401 invalid_token on every endpoint, and that 403 is used only for a missing scope or plan feature. ([specs/restream.yaml:138](specs/restream.yaml#L138))
- [ ] Confirm GET /v2/user/profile as the idle probe needs only profile.read and is not rate limited at that rate. ([specs/restream.yaml:141](specs/restream.yaml#L141))
- [ ] Confirm the legacy PATCH /v2/user/channel/{id} with {"active": true|false} still exists, what it answers (200 assumed), and that it switches the channel for the next stream. ([specs/restream.yaml:284](specs/restream.yaml#L284))
- [ ] Confirm the legacy GET /v2/user/channel/all still exists and answers an array of channels with id and enabled. ([specs/restream.yaml:1028](specs/restream.yaml#L1028))
- [ ] Confirm the legacy GET and PATCH /v2/user/channel-meta/{id} still exist, take title and description, and answer 200. ([specs/restream.yaml:322](specs/restream.yaml#L322))
- [ ] Confirm get_channel_meta works for the platforms that have metadata (YouTube, Facebook, Twitch) and record what it answers for others. ([specs/restream.yaml:298](specs/restream.yaml#L298))
- [ ] Confirm In Progress Events answers [] when nothing is live, and record how soon an event appears there after the encoder starts. ([specs/restream.yaml:992](specs/restream.yaml#L992))
- [ ] Confirm an event that ends leaves In Progress and shows in Events History with status finished on the first page. ([specs/restream.yaml:955](specs/restream.yaml#L955))
- [ ] Record the status codes Create Ticker, Create Caption, Create QR Code and the Recording and Chat History Download URL methods answer (the reference does not say), and what Update Caption and Update QR Code answer. ([specs/restream.yaml:595](specs/restream.yaml#L595))
- [ ] Confirm Create Caption accepts an empty secondaryText, and Update Caption with an empty one clears the second line. ([specs/restream.yaml:649](specs/restream.yaml#L649))
- [ ] Confirm Add Event Destination for a YouTube channel without title is refused, so add_event_destination_custom is needed there. ([specs/restream.yaml:424](specs/restream.yaml#L424))
- [ ] Record Restream's API rate limits, which the reference does not state, against the 30-second poll of six requests. ([specs/restream.yaml:955](specs/restream.yaml#L955))
- [ ] Confirm the streaming-updates websocket accepts the OAuth access token in accessToken, which scope it needs (stream.read assumed; the page names none), and that an expired token is refused at the handshake with 401. ([specs/restream.yaml:947](specs/restream.yaml#L947))
- [ ] Record real streaming updates: that suid is stable for one encoder session, channelId matches the channel list's id, and deleteIncoming and deleteOutgoing arrive when the encoder and destinations stop. ([specs/restream.yaml:1034](specs/restream.yaml#L1034))
- [ ] Record which updateStatuses fields each platform fills (viewers, followers, streamViews), and how often they arrive. ([specs/restream.yaml:1082](specs/restream.yaml#L1082))

## rme-totalmix — RME TotalMix FX

- [ ] Confirm TotalMix sends values as OSC floats and display texts as strings, and how often it sends changes (and meters with Send Peak Level Data). ([specs/rme-totalmix.yaml:1046](specs/rme-totalmix.yaml#L1046))
- [ ] Confirm the snapshot and group button rows (snapshot 1 at /3/snapshots/8/1, group 1 at /3/muteGroups/4/1, as the chart lists them). ([specs/rme-totalmix.yaml:899](specs/rme-totalmix.yaml#L899))
- [ ] Confirm that setSubmix, setBankStart and setOffsetInBank count from 0 and take a float, and what TotalMix sends back after each. ([specs/rme-totalmix.yaml:195](specs/rme-totalmix.yaml#L195))

## rogueamoeba-farrago — Farrago

- [ ] Check Farrago's default OSC Input and Output ports (8090 and 8091 here, the Companion module's defaults; the manual gives none). ([specs/rogueamoeba-farrago.yaml:492](specs/rogueamoeba-farrago.yaml#L492))
- [ ] Check that /ping sends every current value to the OSC Output, that changes are sent there as they happen (from the app, from MIDI and from OSC), and whether feedback also comes by tile UUID or name and for the /inspector and /list addresses. ([specs/rogueamoeba-farrago.yaml:500](specs/rogueamoeba-farrago.yaml#L500))
- [ ] Check the argument types and ranges: an int 1 for actions, an int 1 or 0 for switches, 0/1 for A/B, layout and playback mode, floats 0.0 to 1.0 for volume and color, and the units of currentPosition (assumed a fraction) and crossfadeDuration (assumed seconds); and the types of the feedback values (play as an OSC boolean, the meters' scale). ([specs/rogueamoeba-farrago.yaml:509](specs/rogueamoeba-farrago.yaml#L509))
- [ ] Check that play on a playing tile stops it (or pauses it with pausing allowed), and what /set/<set>/position does with its value. ([specs/rogueamoeba-farrago.yaml:520](specs/rogueamoeba-farrago.yaml#L520))

## roland-m5000 — Roland M-5000 / M-5000C

- [ ] Over Telnet, confirm that each reply (OK, ERR:n; and request replies) is a line ended by CR LF with no stx, and that the echo of the command arrives as its own line before the reply. ([specs/roland-m5000.yaml:47](specs/roland-m5000.yaml#L47))
- [ ] Check whether the console sends Telnet option negotiation (IAC) bytes on connecting, and how many Telnet sessions it accepts. ([specs/roland-m5000.yaml:605](specs/roland-m5000.yaml#L605))
- [ ] On RS-232C, confirm the reply framing (stx before reply commands, a bare 06H ack, no line ending) and that Xon/Xoff is used. ([specs/roland-m5000.yaml:59](specs/roland-m5000.yaml#L59))
- [ ] Confirm that a request through a user fader (U1-U192) is answered with the U token, not the assigned channel's. ([specs/roland-m5000.yaml:600](specs/roland-m5000.yaml#L600))
- [ ] Check whether channel names shorter than 12 characters are padded with spaces in CNS replies. ([specs/roland-m5000.yaml:613](specs/roland-m5000.yaml#L613))
- [ ] Confirm the RNS reply's song name is closed by a quote (the reference's example leaves it out). ([specs/roland-m5000.yaml:951](specs/roland-m5000.yaml#L951))
- [ ] Measure whether the console keeps up with the polls (about 830 fader and mute requests every ten seconds) without slowing its surface or answering ERR:2 busy. ([specs/roland-m5000.yaml:657](specs/roland-m5000.yaml#L657))

## roland-p20hd — Roland P-20HD

- [ ] Confirm the login exchange: USR sent on connecting with no prompt, ACK after USR and after PSS, NAK on refusal. ([specs/roland-p20hd.yaml:588](specs/roland-p20hd.yaml#L588))
- [ ] Confirm the session closes after five minutes without commands and that ACS every ten seconds of silence keeps it open. ([specs/roland-p20hd.yaml:609](specs/roland-p20hd.yaml#L609))
- [ ] Confirm XON/XOFF flow control on the RS-232 port (only the XON and XOFF commands are documented, p.78) and that no login is asked for there. ([specs/roland-p20hd.yaml:50](specs/roland-p20hd.yaml#L50))

## roland-v1200hd — Roland V-1200HD

- [ ] Confirm ACS (verification of unit status) is answered by a bare ACK, as the liveness check assumes; the guide lists no reply for it. ([specs/roland-v1200hd.yaml:44](specs/roland-v1200hd.yaml#L44))
- [ ] Check whether ACK is the single byte 06H or "ACK;" as the guide's overview writes it, and that a trailing ";" is harmless. ([specs/roland-v1200hd.yaml:41](specs/roland-v1200hd.yaml#L41))
- [ ] Confirm the STS frames carry their values comma-separated without spaces, group 5 answering 8 for no memory, and the order of each group as listed. ([specs/roland-v1200hd.yaml:298](specs/roland-v1200hd.yaml#L298))

## roland-v160hd — Roland V-160HD, V-80HD, VR-120HD, VR-6HD, V-1-4K

- [ ] Confirm commands ended with CR LF are accepted (Roland documents only Telnet). ([specs/roland-v160hd.yaml:103](specs/roland-v160hd.yaml#L103))
- [ ] Confirm the password prompt and reply texts ("Enter password:", "Welcome to ...", "Authentication error", "Wait a moment"), which are community-sourced. ([specs/roland-v160hd.yaml:2855](specs/roland-v160hd.yaml#L2855))
- [ ] Check whether a query's value frame and its ACK; arrive on one line or two. ([specs/roland-v160hd.yaml:2871](specs/roland-v160hd.yaml#L2871))
- [ ] Check whether GRCH, AMCH, SPCH, AUXCH, QSRTIST and QBSY are followed by ACK;. ([specs/roland-v160hd.yaml:2873](specs/roland-v160hd.yaml#L2873))
- [ ] Confirm a poll of five queries a second does not lock the panel. ([specs/roland-v160hd.yaml:2894](specs/roland-v160hd.yaml#L2894))
- [ ] Confirm the wire spellings chosen: test tone levels OFF, -20, -10, 0 and frequencies 500, 1k, 2k, and values without the space before the comma. ([specs/roland-v160hd.yaml:2905](specs/roland-v160hd.yaml#L2905))
- [ ] Confirm the AUXLV and AUXCH replies are QAUXLV and QAUXCH, and QTIM names DSK on V-1-4K. ([specs/roland-v160hd.yaml:2875](specs/roland-v160hd.yaml#L2875))
- [ ] Confirm the minimum firmware version per model for the Basic commands. ([specs/roland-v160hd.yaml:2932](specs/roland-v160hd.yaml#L2932))
- [ ] Check whether a second control session receives tally frames after TALLY AUTO SEND. ([specs/roland-v160hd.yaml:2946](specs/roland-v160hd.yaml#L2946))
- [ ] Check whether RQH sizes other than 000001 are answered. ([specs/roland-v160hd.yaml:2944](specs/roland-v160hd.yaml#L2944))
- [ ] Drive a V-160HD, VR-120HD or VR-6HD on RS-232 (link serial): confirm commands with stx and nothing after the ";" are accepted, and whether replies end in LF there. ([specs/roland-v160hd.yaml:3281](specs/roland-v160hd.yaml#L3281))
- [ ] Drive a V-80HD on RS-232 and confirm commands without stx and line ending are accepted. ([specs/roland-v160hd.yaml:3281](specs/roland-v160hd.yaml#L3281))
- [ ] Check whether DTH is answered by the ack byte 06H or by ACK; on V-160HD, VR-120HD and VR-6HD, on RS-232 and over LAN. ([specs/roland-v160hd.yaml:3288](specs/roland-v160hd.yaml#L3288))
- [ ] Find the V-80HD's default RS-232 BAUDRATE (115200 assumed, as on the others). ([specs/roland-v160hd.yaml:3291](specs/roland-v160hd.yaml#L3291))
- [ ] Confirm XON/XOFF flow control on RS-232, which the communication standards leave out. ([specs/roland-v160hd.yaml:3294](specs/roland-v160hd.yaml#L3294))

## roland-v600uhd — Roland V-600UHD

- [ ] Confirm the password prompt "Enter password:" and greeting "Welcome to Roland V-600UHD.", and record what a wrong password produces. ([specs/roland-v600uhd.yaml:281](specs/roland-v600uhd.yaml#L281))
- [ ] Find which input audio numbers lie between HDMI IN1 (0) and TEST TONE (7). ([specs/roland-v600uhd.yaml:301](specs/roland-v600uhd.yaml#L301))
- [ ] Drive the V-600UHD on RS-232 (link serial): confirm commands with stx and no line ending are accepted, ACK arrives as the bare byte 06H, and whether any line ending follows a reply. ([specs/roland-v600uhd.yaml:50](specs/roland-v600uhd.yaml#L50))

## roland-v60hd — Roland V-60HD

- [ ] Confirm the QAL numbering (which of 11, 12 and 13 is MASTER OUT, AUX and all) and the order of the QAL:13 frame (inputs 0-10, AUX, MASTER OUT assumed). ([specs/roland-v60hd.yaml:441](specs/roland-v60hd.yaml#L441))
- [ ] Check whether the QPL frame for PANEL INFORMATION is also sent over LAN. ([specs/roland-v60hd.yaml:516](specs/roland-v60hd.yaml#L516))
- [ ] Drive the V-60HD on RS-232 (link serial) and confirm the same frames as over LAN, with XON/XOFF. ([specs/roland-v60hd.yaml:50](specs/roland-v60hd.yaml#L50))

## roland-vmixer — Roland V-Mixer M-480 / M-400 / M-380 / M-300 / M-200i

- [ ] On the M-200i over Telnet, confirm that each reply (OK, ERR:n; and request replies) is a line ended by CR LF with no stx, and that the echoed command arrives as its own line. ([specs/roland-vmixer.yaml:44](specs/roland-vmixer.yaml#L44))
- [ ] On RS-232C, confirm that reply commands begin with stx and end at ";" with no line ending, and that ack is the bare byte 06H. ([specs/roland-vmixer.yaml:63](specs/roland-vmixer.yaml#L63))
- [ ] Confirm the CNS name is six characters padded with spaces, quoted. ([specs/roland-vmixer.yaml:1131](specs/roland-vmixer.yaml#L1131))
- [ ] Confirm that a request through a user fader is answered with the U token, not the assigned channel's. ([specs/roland-vmixer.yaml:1115](specs/roland-vmixer.yaml#L1115))
- [ ] Measure whether the consoles keep up with the polls (up to about 230 fader and mute requests every ten seconds at 115200 bps) without answering ERR:2 busy. ([specs/roland-vmixer.yaml:1190](specs/roland-vmixer.yaml#L1190))
- [ ] Check the DTS reply for a delay in milliseconds and in samples (decimal places, unit token). ([specs/roland-vmixer.yaml:788](specs/roland-vmixer.yaml#L788))
- [ ] Check which input-patch channels and ports each model accepts (the reference lists the M-480 cascade inputs and per-model port ranges). ([specs/roland-vmixer.yaml:862](specs/roland-vmixer.yaml#L862))

## roland-vr400uhd — Roland VR-400UHD

- [ ] Confirm the password prompt (a line containing "Enter") and greeting, and record what a wrong password produces. ([specs/roland-vr400uhd.yaml:577](specs/roland-vr400uhd.yaml#L577))
- [ ] Check whether get is answered with an ack line as well as the set line. ([specs/roland-vr400uhd.yaml:588](specs/roland-vr400uhd.yaml#L588))
- [ ] Check the dB scale of levels 0-127. ([specs/roland-vr400uhd.yaml:601](specs/roland-vr400uhd.yaml#L601))
- [ ] Confirm get answers every parameter of the command list (OUTPUT FADE, DSK, LOGO, transition, MIC and LINE channels, outputs, auto mixing, audio follow video, reverb) with its current value, in the ranges set takes. ([specs/roland-vr400uhd.yaml:433](specs/roland-vr400uhd.yaml#L433))
- [ ] Confirm the RS-232 port uses no flow control (not documented) and asks for no password. ([specs/roland-vr400uhd.yaml:49](specs/roland-vr400uhd.yaml#L49))

## roland-vr4hd — Roland VR-4HD

- [ ] Confirm VER, used as the liveness check (the VR-4HD lists no ACS), is answered with REMOTE RS-232 on, and nothing when it is off. ([specs/roland-vr4hd.yaml:44](specs/roland-vr4hd.yaml#L44))
- [ ] Check that a status frame pushed on a change (QPI, QDK, QPG, QUB) comes besides, not in place of, the ACK of the command that caused it. ([specs/roland-vr4hd.yaml:268](specs/roland-vr4hd.yaml#L268))

## roland-xs1hd — Roland XS-1HD

- [ ] Confirm ACS (obtain the status) is answered by a bare ACK and nothing else, as the liveness check assumes; the manual lists no ACS reply. ([specs/roland-xs1hd.yaml:39](specs/roland-xs1hd.yaml#L39))

## roland-xs42h — Roland XS-42H / VP-42H

- [ ] Check whether the unit prompts before taking the password, and record the login exchange and what a wrong password produces. ([specs/roland-xs42h.yaml:354](specs/roland-xs42h.yaml#L354))
- [ ] Check the dB scale of levels 0-127. ([specs/roland-xs42h.yaml:381](specs/roland-xs42h.yaml#L381))
- [ ] Confirm the RS-232 port uses no flow control (not documented) and asks for no password. ([specs/roland-xs42h.yaml:39](specs/roland-xs42h.yaml#L39))

## roland-xs62s — Roland XS-62S

- [ ] Confirm the forms the manual leaves open: VOS with only the bus, IL2 with both a and b, the two TRS values, and VER's empty model field. ([specs/roland-xs62s.yaml:653](specs/roland-xs62s.yaml#L653))
- [ ] Confirm QAL:14 answers fourteen levels (the manual's example shows thirteen), in the order inputs 0-10, MASTER OUT, PVW/2, AUX/3, and whether the input levels are the PGM/1 (IL1) or PVW/2 (IL2) bus levels. ([specs/roland-xs62s.yaml:580](specs/roland-xs62s.yaml#L580))
- [ ] Drive the XS-62S on RS-232 (link serial) and confirm the same frames as over LAN, with XON/XOFF. ([specs/roland-xs62s.yaml:40](specs/roland-xs62s.yaml#L40))

## roland-xs80h — Roland XS-82H/83H/84H

- [ ] Check whether any line ending follows ";" over LAN. ([specs/roland-xs80h.yaml:564](specs/roland-xs80h.yaml#L564))
- [ ] Check whether ITS, OTS, CTS, KLS and VER are followed by ACK;. ([specs/roland-xs80h.yaml:566](specs/roland-xs80h.yaml#L566))
- [ ] Check whether a setting enables LAN control and whether a password applies. ([specs/roland-xs80h.yaml:584](specs/roland-xs80h.yaml#L584))
- [ ] Drive an XS-8xH on RS-232C (link serial, FUNCTION RECEIVER) with no CR after the ";" and confirm the same replies as over LAN, with XON/XOFF. ([specs/roland-xs80h.yaml:48](specs/roland-xs80h.yaml#L48))
- [ ] Confirm an XS-82H or XS-83H answers ITS and OTS for the inputs and outputs it lacks with an error, as for CTS. ([specs/roland-xs80h.yaml:510](specs/roland-xs80h.yaml#L510))

## ross-ultrix — Ross Ultrix (RossTalk)

- [ ] Confirm GPI takes the salvo number unpadded (GPI 4 as well as GPI 04). ([specs/ross-ultrix.yaml:91](specs/ross-ultrix.yaml#L91))
- [ ] Check the TIMER SET value format: two-digit fields and one digit of tenths. ([specs/ross-ultrix.yaml:133](specs/ross-ultrix.yaml#L133))
- [ ] Confirm a TXTLABEL with text and no color fields, ending TEXT:text;, sets the text and keeps the colors. ([specs/ross-ultrix.yaml:196](specs/ross-ultrix.yaml#L196))
- [ ] Confirm decimal R,G,B colors in TXTLABEL (255,255,255) as well as 0x hex. ([specs/ross-ultrix.yaml:216](specs/ross-ultrix.yaml#L216))
- [ ] Record the largest salvo, timer, user, label, source, destination and level IDs accepted. ([specs/ross-ultrix.yaml:245](specs/ross-ultrix.yaml#L245))
- [ ] Confirm Ultrix sends nothing back on the RossTalk connection, not even an error for a bad command. ([specs/ross-ultrix.yaml:229](specs/ross-ultrix.yaml#L229))

## ross-video-server — Ross Video servers (Tria, Mira, Kiva)

- [ ] Confirm the servers send nothing back to RossTalk commands (no acknowledgement or error text), as Ross documents none. ([specs/ross-video-server.yaml:47](specs/ross-video-server.yaml#L47))
- [ ] Confirm the jog amount's forms: frames with an f ("-20f", "5f" forward), and whether a plain or timecode amount is taken. ([specs/ross-video-server.yaml:128](specs/ross-video-server.yaml#L128))
- [ ] Confirm the ISO angle numbering starts at 1 and its upper limit. ([specs/ross-video-server.yaml:143](specs/ross-video-server.yaml#L143))
- [ ] Confirm LOAD_PLAYLIST with two channels takes the preview channel first, then the program channel. ([specs/ross-video-server.yaml:162](specs/ross-video-server.yaml#L162))
- [ ] Confirm a RossTalk port other than 7788 (7788-7798) is taken only from V9.2 software. ([specs/ross-video-server.yaml:50](specs/ross-video-server.yaml#L50))
- [ ] Confirm the channel letters each model has (A-H; G and H on a Mira with two PGM players). ([specs/ross-video-server.yaml:87](specs/ross-video-server.yaml#L87))

## ross-xpression — Ross XPression (TCP)

- [ ] Confirm take IDs are read padded or unpadded, including UNCUE and UPNEXT in the padded form. ([specs/ross-xpression.yaml:355](specs/ross-xpression.yaml#L355))
- [ ] Check the take ID, layer, framebuffer, GPI and channel ranges. ([specs/ross-xpression.yaml:361](specs/ross-xpression.yaml#L361))
- [ ] Check the syntax for RESUME with a tessera source name. ([specs/ross-xpression.yaml:406](specs/ross-xpression.yaml#L406))

## ross-xpression-udp — Ross XPression (UDP)

- [ ] Confirm the UDP port configured on the board (7788 assumed; Ross states no default). ([specs/ross-xpression-udp.yaml:310](specs/ross-xpression-udp.yaml#L310))
- [ ] Confirm one command per datagram, ended with CR LF, is accepted. ([specs/ross-xpression-udp.yaml:319](specs/ross-xpression-udp.yaml#L319))
- [ ] Confirm take IDs are read padded or unpadded, including UNCUE and UPNEXT in the padded form. ([specs/ross-xpression-udp.yaml:372](specs/ross-xpression-udp.yaml#L372))
- [ ] Check the take ID, layer, framebuffer, GPI and channel ranges. ([specs/ross-xpression-udp.yaml:378](specs/ross-xpression-udp.yaml#L378))
- [ ] Check the syntax for RESUME with a tessera source name. ([specs/ross-xpression-udp.yaml:430](specs/ross-xpression-udp.yaml#L430))

## rosstalk — RossTalk (switchers)

- [ ] Check how memory numbers are written for banks of two digits. ([specs/rosstalk.yaml:1102](specs/rosstalk.yaml#L1102))
- [ ] Confirm CC and GPI are accepted with a colon and two-digit number as in the examples. ([specs/rosstalk.yaml:1085](specs/rosstalk.yaml#L1085))
- [ ] Confirm MVBOX's I/O MultiViewer token (IO or OP) and that NTP SET (not NET SET) is accepted. ([specs/rosstalk.yaml:1161](specs/rosstalk.yaml#L1161))
- [ ] Confirm Media-Store IDs zero-padded to three digits on Carbonite and unpadded on Acuity. ([specs/rosstalk.yaml:1151](specs/rosstalk.yaml#L1151))
- [ ] On Acuity, Ultrix Acuity and Vision, check what the RTalk-IN "Cmd Response" option does. ([specs/rosstalk.yaml:1182](specs/rosstalk.yaml#L1182))
- [ ] On Vision, check whether command words such as MECUT and MEAUTO must use MLE instead of ME. ([specs/rosstalk.yaml:1202](specs/rosstalk.yaml#L1202))
- [ ] On Acuity, record the reply CAPTURE sends when the capture completes. ([specs/rosstalk.yaml:1221](specs/rosstalk.yaml#L1221))
- [ ] Capture, on a Carbonite-family and an Acuity-family switcher with Cmd Response on and off, the bytes each documented query answers (XPT, MS, MNEM, TRANSRATE, TRANSTYPE, TRANSINCL, MVBOX, USERVAR and KEYSHAPED with ?, KEYSTATE, ACTIVEID, VERSION, HELP, MEM ?), the answer to an unknown source or ME, the answer to a set, the order of answers to queries sent together, and any unasked messages, so state can be kept. ([specs/rosstalk.yaml:1469](specs/rosstalk.yaml#L1469))

## rundown-studio — Rundown Studio

- [ ] Confirm the event stream takes the bearer token and ?events=rundown:fat,cue:fat,column:fat,cell:fat, and sends status first on each connection. ([specs/rundown-studio.yaml:596](specs/rundown-studio.yaml#L596))
- [ ] Confirm the include query is taken percent-encoded (include=cues%2Ccolumns) and answers cues and columns in included. ([specs/rundown-studio.yaml:288](specs/rundown-studio.yaml#L288))
- [ ] Record what a delete answers (status and body) and whether the event stream reports deletions of mentions and text variables (it is read every minute). ([specs/rundown-studio.yaml:910](specs/rundown-studio.yaml#L910))
- [ ] Check how often a restricted-tier token is refused with 429 at the core's pacing and polls. ([specs/rundown-studio.yaml:64](specs/rundown-studio.yaml#L64))
- [ ] Confirm the cue order's children carry {id} as the shallow rundown example shows (the reorder body names them cue_id). ([specs/rundown-studio.yaml:720](specs/rundown-studio.yaml#L720))
- [ ] Confirm the cells list is cursor-paged like the rundown and cue lists (data.next_cursor and data.has_more), so a rundown of more than 1,000 cells is read whole. ([specs/rundown-studio.yaml:442](specs/rundown-studio.yaml#L442))

## sacn — sACN (ANSI E1.31)

- [ ] Check that receivers take the core's universes, keep-alives and Stream_Terminated packets (multicast and unicast), and list it from its Universe Discovery. ([specs/sacn.yaml:150](specs/sacn.yaml#L150))
- [ ] With two consoles at one priority, compare the received levels (merged highest takes precedence) with what the fixtures do, and check that a source is dropped 2.5 s after it stops. ([specs/sacn.yaml:159](specs/sacn.yaml#L159))
- [ ] Check that multicast is received on each interface the core joins on, with several programs on the host listening on port 5568. ([specs/sacn.yaml:142](specs/sacn.yaml#L142))
- [ ] Check what changed in the current edition of E1.31 against the 2016 edition this follows. ([specs/sacn.yaml:174](specs/sacn.yaml#L174))

## samsung-mdc — Samsung displays (MDC)

- [ ] Check the secured mode handshake on a display with the secured protocol on: the plain-text MDCSTART<<TLS>> greeting, a TLS handshake on the same connection, the 4-digit PIN sent bare, and MDCAUTH<<PASS>> or MDCAUTH<<FAIL:xxxx>> (taken from a third-party library, not Ver. 15.0). ([specs/samsung-mdc.yaml:2884](specs/samsung-mdc.yaml#L2884))
- [ ] Check which TLS versions the secured mode offers; the core speaks only TLS 1.2 and 1.3. ([specs/samsung-mdc.yaml:2890](specs/samsung-mdc.yaml#L2890))
- [ ] Check power on over RJ45: whether a display in standby (Network Standby on and off) still answers MDC, whether Wake-on-LAN to the subnet broadcast wakes it, and whether reconnecting 10 s after an acknowledged power on is needed. ([specs/samsung-mdc.yaml:2875](specs/samsung-mdc.yaml#L2875))
- [ ] Check the factory display ID (0 assumed) and that a display reached over RJ45 stays silent for any other ID, and that chained displays answer through its RS-232C loop-out. ([specs/samsung-mdc.yaml:40](specs/samsung-mdc.yaml#L40))
- [ ] Check that a command a model lacks is answered NAK rather than left unanswered, and what error bytes NAKs carry. ([specs/samsung-mdc.yaml:2903](specs/samsung-mdc.yaml#L2903))
- [ ] Check which timer layout (13 or 15 values) and which Maintenance Control, Sound Control and multi-screen layouts each model family replies with, and that a display refuses the set layout it does not take. ([specs/samsung-mdc.yaml:2919](specs/samsung-mdc.yaml#L2919))
- [ ] Check the display status temperature encoding: signed (two's complement) is assumed, with 0xFF taken as invalid. ([specs/samsung-mdc.yaml:251](specs/samsung-mdc.yaml#L251))
- [ ] Check the panel on time's unit: a counter rising every 10 minutes is assumed (count of 10-minute periods, or minutes). ([specs/samsung-mdc.yaml:1354](specs/samsung-mdc.yaml#L1354))
- [ ] Check the older color temperature codes 0x00-0x0A (5000 K to 15000 K, steps not given) and whether a display answers a read with the extended (kelvin / 100) code. ([specs/samsung-mdc.yaml:872](specs/samsung-mdc.yaml#L872))
- [ ] Check whether holiday indexes count from 0 or 1. ([specs/samsung-mdc.yaml:1921](specs/samsung-mdc.yaml#L1921))
- [ ] Check the byte order of two-byte values whose sections do not state it (custom fit size, screen position, final duty, custom PIP, light sensor and brightness output ranges); high byte first is assumed. ([specs/samsung-mdc.yaml:2929](specs/samsung-mdc.yaml#L2929))
- [ ] Check the light sensor effective range and brightness output range data width (two bytes per value assumed). ([specs/samsung-mdc.yaml:2288](specs/samsung-mdc.yaml#L2288))
- [ ] Check the ticker frame: data length 17 plus the message, the message's encoding (UTF-8 sent), and the foreground opacity codes (0-2 opacities, 3-5 flashing and off assumed). ([specs/samsung-mdc.yaml:1194](specs/samsung-mdc.yaml#L1194))
- [ ] Check the Wi-Fi access point frame: a type byte, a one-byte length and the text for the SSID then the password, and what the display echoes. ([specs/samsung-mdc.yaml:413](specs/samsung-mdc.yaml#L413))
- [ ] Check what a display answers to set_osd_types (the shown-kinds bit mask, as for a read, is assumed). ([specs/samsung-mdc.yaml:1569](specs/samsung-mdc.yaml#L1569))
- [ ] Check the latitude and longitude text format a display expects. ([specs/samsung-mdc.yaml:2313](specs/samsung-mdc.yaml#L2313))
- [ ] Check that the serial number is 15 bytes padded with NULs or spaces. ([specs/samsung-mdc.yaml:246](specs/samsung-mdc.yaml#L246))
- [ ] Check the LED white balance, color correction, seam correction and gradation bit layouts (sub-commands 0x92, 0x93, 0x97, 0x9B, 0x9C, 0x9F, 0xA2), sent and read as hex. ([specs/samsung-mdc.yaml:2938](specs/samsung-mdc.yaml#L2938))
- [ ] Check the menu size reply's sub-command echo (the document prints 0x83 for 0x85); any echo is taken as the reply. ([specs/samsung-mdc.yaml:2903](specs/samsung-mdc.yaml#L2903))

## sennheiser-adn — Sennheiser ADN conference system

- [ ] Confirm that replies and updates are "Keyword value;" with nothing between messages, or record the separator the central unit sends. ([specs/sennheiser-adn.yaml:35](specs/sennheiser-adn.yaml#L35))
- [ ] Record how many clients the central unit accepts on 53252 and whether a new client gets anything unasked on connecting. ([specs/sennheiser-adn.yaml:42](specs/sennheiser-adn.yaml#L42))
- [ ] Confirm the reply to MicButtonSN for a wired unit is MicStatus with its bus position. ([specs/sennheiser-adn.yaml:852](specs/sennheiser-adn.yaml#L852))
- [ ] Confirm the direction of relative equalizer changes (#1 moves the wire value up, towards -12 dB). ([specs/sennheiser-adn.yaml:248](specs/sennheiser-adn.yaml#L248))

## sennheiser-chg-70n — Sennheiser CHG 70N

- [ ] Confirm the charger answers a command datagram with the method's new value (or an /osc/error entry) to the sender's port, so each command is acknowledged by its reply. ([specs/sennheiser-chg-70n.yaml:66](specs/sennheiser-chg-70n.yaml#L66))
- [ ] Confirm one bay can be identified with null in the other bay's place, leaving that bay as it is (Companion does this; the guide's example writes both). ([specs/sennheiser-chg-70n.yaml:66](specs/sennheiser-chg-70n.yaml#L66))
- [ ] Confirm one address tree may hold every /device method, and another every /bays method, in a single subscription, with a 60 s lifetime. ([specs/sennheiser-chg-70n.yaml:155](specs/sennheiser-chg-70n.yaml#L155))
- [ ] Check whether a bay with nothing in it reports null, 0 or an empty string for its battery values. ([specs/sennheiser-chg-70n.yaml:313](specs/sennheiser-chg-70n.yaml#L313))
- [ ] Check whether /bays/warnings gives one string per bay (as read) or a list of warnings per bay, and whether /device/warnings is a string or a list. ([specs/sennheiser-chg-70n.yaml:287](specs/sennheiser-chg-70n.yaml#L287))
- [ ] Check whether the network fields (ipaddr and the others) come as plain strings or one-element arrays; both are read. ([specs/sennheiser-chg-70n.yaml:187](specs/sennheiser-chg-70n.yaml#L187))
- [ ] Check the shape /bays/sync_settings accepts (the guide's example quotes a frequency oddly) and whether the charger answers it. ([specs/sennheiser-chg-70n.yaml:131](specs/sennheiser-chg-70n.yaml#L131))
- [ ] Find out what /bays/update/enable does when written; it is read only. ([specs/sennheiser-chg-70n.yaml:293](specs/sennheiser-chg-70n.yaml#L293))
- [ ] Check which characters /device/location accepts (the guide allows printable ASCII with escaped quote, slash and backslash). ([specs/sennheiser-chg-70n.yaml:99](specs/sennheiser-chg-70n.yaml#L99))
- [ ] Confirm the name query used as the liveness probe is answered while nothing else is happening. ([specs/sennheiser-chg-70n.yaml:42](specs/sennheiser-chg-70n.yaml#L42))
- [ ] Check whether restart is answered before the charger goes away. ([specs/sennheiser-chg-70n.yaml:328](specs/sennheiser-chg-70n.yaml#L328))

## sennheiser-digital-6000 — Sennheiser Digital 6000

- [ ] Confirm the SSC port is 45 (the Companion module defaults to 6970). ([specs/sennheiser-digital-6000.yaml:266](specs/sennheiser-digital-6000.yaml#L266))
- [ ] Confirm that subscriptions lapse after 20 s and that renewal every 6.7 s keeps telemetry flowing. ([specs/sennheiser-digital-6000.yaml:258](specs/sennheiser-digital-6000.yaml#L258))
- [ ] Confirm out-of-range set_frequency and set_af_out values are adapted and replied with the value applied. ([specs/sennheiser-digital-6000.yaml:272](specs/sennheiser-digital-6000.yaml#L272))
- [ ] Opened for commands only, confirm the receiver answers an unsubscribed `device.name` read and echoes its `osc.xid`, the liveness check. ([crates/core/src/modules/sennheiser_d6000.rs:163](crates/core/src/modules/sennheiser_d6000.rs#L163))
- [ ] Confirm name, carrier, active_status, the other skx methods and both wsm_master_cnt counters can be subscribed in the one tree (the guide marks only some as subscribable). ([crates/core/src/modules/sennheiser_d6000.rs:457](crates/core/src/modules/sennheiser_d6000.rs#L457))
- [ ] Confirm wsm_master_cnt increments for changes made on the receiver only, so re-reading on it misses nothing the core did itself, and which settings each counter covers. ([crates/core/src/modules/sennheiser_d6000.rs:232](crates/core/src/modules/sennheiser_d6000.rs#L232))
- [ ] Check that active_bank_channel accepts user banks u4-u6 (the guide's limits list u1-u3; Companion and the frequency tables have u1-u6), and whether banks come back upper or lower case. ([specs/sennheiser-digital-6000.yaml:104](specs/sennheiser-digital-6000.yaml#L104))
- [ ] Check the channel name limit (8 characters and the forbidden characters are Companion's; the guide gives none) and what the receiver does with a longer name. ([specs/sennheiser-digital-6000.yaml:89](specs/sennheiser-digital-6000.yaml#L89))
- [ ] Check that /device/name is replied with the MAC address appended, as the guide's example shows, and its length limit. ([specs/sennheiser-digital-6000.yaml:96](specs/sennheiser-digital-6000.yaml#L96))
- [ ] Confirm a sync settings write with several leaves in one message is applied and replied, and that a gain off the 3 dB grid is refused. ([specs/sennheiser-digital-6000.yaml:121](specs/sennheiser-digital-6000.yaml#L121))
- [ ] Confirm a read of all 100 channels of a bank in one datagram is answered in one reply that fits a datagram. ([specs/sennheiser-digital-6000.yaml:148](specs/sennheiser-digital-6000.yaml#L148))
- [ ] Check set_user_frequency's adaptation of frequencies off the 25 kHz grid, and whether a stored user bank channel can be cleared. ([specs/sennheiser-digital-6000.yaml:139](specs/sennheiser-digital-6000.yaml#L139))
- [ ] Check that /sys/clock accepts 4 (internal manual) as a write and what it means. ([specs/sennheiser-digital-6000.yaml:169](specs/sennheiser-digital-6000.yaml#L169))
- [ ] Check whether set_network is replied before the receiver changes address, whether mdns may be written with auto off, and the ipv4_dante auto write (the guide shows only the settings array). ([specs/sennheiser-digital-6000.yaml:175](specs/sennheiser-digital-6000.yaml#L175))
- [ ] Confirm encryption and sync setting changes reach the transmitter only at its next sync. ([specs/sennheiser-digital-6000.yaml:302](specs/sennheiser-digital-6000.yaml#L302))

## sennheiser-ew-dx — Sennheiser EW-DX

- [ ] Confirm each resource added beyond the six per channel tested on hardware (identify, signal strength, diversity, user preset bank, sync settings and their ignore flags, the transmitter and its warnings, and the device's site, state, identification, rf, transmission, encryption, legacyMode and firmware update state) can be subscribed and notifies on change, and that a refused batch leaves the later ones subscribed. ([specs/sennheiser-ew-dx.yaml:487](specs/sennheiser-ew-dx.yaml#L487))
- [ ] Check whether select_preset's bank and preset, and channels.<n>.preset.bank and .channel, count from 0 or from 1, against the receiver's own preset display. ([specs/sennheiser-ew-dx.yaml:179](specs/sennheiser-ew-dx.yaml#L179))
- [ ] Check whether /api/rf answers each range's step as stepSize (the schema) or stepsize (the document's example); both are read. ([specs/sennheiser-ew-dx.yaml:329](specs/sennheiser-ew-dx.yaml#L329))
- [ ] Measure how long the receiver stays busy after set_link_density and set_encryption, whether it restarts or drops the HTTPS connection, and whether 10 s is enough for their answers. ([specs/sennheiser-ew-dx.yaml:429](specs/sennheiser-ew-dx.yaml#L429))
- [ ] Confirm /api/transmitters/{id} and its warnings answer 422 like the battery when no transmitter is linked, and what the stream sends when a transmitter links or unlinks. ([specs/sennheiser-ew-dx.yaml:387](specs/sennheiser-ew-dx.yaml#L387))
- [ ] Check what set_user_presets does with frequencies outside the receiver's ranges or off its preset spacing, and whether the bank must be in order. ([specs/sennheiser-ew-dx.yaml:188](specs/sennheiser-ew-dx.yaml#L188))
- [ ] On firmware 4, confirm that SSCv1 answers on UDP 45 while Secure (SSCv2) access is also on once Legacy is enabled (Control Cockpit presents Secure and Legacy as a choice; Companion's help says Legacy Mode runs beside it), and where Legacy is switched on. ([specs/sennheiser-ew-dx.yaml:444](specs/sennheiser-ew-dx.yaml#L444))
- [ ] Confirm an SSCv1 reply carries the request's /osc/xid back, as the Digital 6000's does; commands are matched to replies by it alone. ([specs/sennheiser-ew-dx.yaml:444](specs/sennheiser-ew-dx.yaml#L444))
- [ ] Check whether /device/network/ipv4/ipaddr and the other control-network fields are answered as plain strings or one-element arrays (the guide's count: 1); both are read. ([specs/sennheiser-ew-dx.yaml:343](specs/sennheiser-ew-dx.yaml#L343))
- [ ] Check whether restart and restore_factory_defaults are answered before the receiver goes away, or time out after 2 s although they worked. ([specs/sennheiser-ew-dx.yaml:276](specs/sennheiser-ew-dx.yaml#L276))
- [ ] Check what set_auto_lock (/device/lock) locks: the guide says the receiver's keys, Companion calls it auto lock. ([specs/sennheiser-ew-dx.yaml:264](specs/sennheiser-ew-dx.yaml#L264))
- [ ] Check the location limit: SSCv2's site allows 255 characters without quotes, slash or backslash, SSCv1's /device/location 400; the stricter one is enforced. ([specs/sennheiser-ew-dx.yaml:252](specs/sennheiser-ew-dx.yaml#L252))
- [ ] Check whether set_network and set_dante_network take effect at once or after a restart, and whether a static address may be written while auto is on. ([specs/sennheiser-ew-dx.yaml:284](specs/sennheiser-ew-dx.yaml#L284))
- [ ] On an EM 4 Dante, confirm subscriptions and every channel command work on channels 3 and 4 (only channels 1 and 2 have been tested on hardware). ([specs/sennheiser-ew-dx.yaml:508](specs/sennheiser-ew-dx.yaml#L508))
- [ ] Confirm a 3 s silence check against /api/ssc/version does not lose a healthy connection. ([specs/sennheiser-ew-dx.yaml:480](specs/sennheiser-ew-dx.yaml#L480))

## sennheiser-ew-g3-g4 — Sennheiser ew G3 / G4 and 2000 series

- [ ] Check which bit of States is TX mute against a real receiver, using mute_flags, since the document's examples disagree with its table. ([specs/sennheiser-ew-g3-g4.yaml:351](specs/sennheiser-ew-g3-g4.yaml#L351))
- [ ] On a G3 (EM 300/500 G3, SR 300 IEM G3) and a 2000 series unit, record the replies and cyclic attributes the G3 and 2000 series document describes and check them against the G4 behavior this module follows. ([specs/sennheiser-ew-g3-g4.yaml:366](specs/sennheiser-ew-g3-g4.yaml#L366))
- [ ] Find out how the EM 2050's two receivers and the SR 2050 IEM's two transmitters are addressed over the protocol (taken as one address each; the document does not say). ([specs/sennheiser-ew-g3-g4.yaml:88](specs/sennheiser-ew-g3-g4.yaml#L88))
- [ ] On an EM 2000 or 2050, confirm AF out takes every whole dB from -24 to +18, and whether +19 to +23 dB are accepted (the document gives "-24..+18(+24)" with 1 dB steps). ([specs/sennheiser-ew-g3-g4.yaml:366](specs/sennheiser-ew-g3-g4.yaml#L366))
- [ ] Record an SR's Af cyclic attribute: the document's example has five values ("Af 15 25 40 38 5") where its table gives four (two levels, two peak holds), and af.levels keeps them raw. ([specs/sennheiser-ew-g3-g4.yaml:289](specs/sennheiser-ew-g3-g4.yaml#L289))
- [ ] Opened for commands only, confirm a receiver with no Push subscription answers a bare `Name`, the liveness check. ([crates/core/src/modules/sennheiser_mcp.rs:37](crates/core/src/modules/sennheiser_mcp.rs#L37))
- [ ] Confirm a BankList reply carries no bank number (as the document's example shows), so it can only be tied to its request, and how many channels a preset bank lists. ([specs/sennheiser-ew-g3-g4.yaml:198](specs/sennheiser-ew-g3-g4.yaml#L198))
- [ ] Confirm `Frequency f bank channel` with the channel's own frequency is answered with that bank and channel, and with 0 0 when they do not match. ([specs/sennheiser-ew-g3-g4.yaml:209](specs/sennheiser-ew-g3-g4.yaml#L209))
- [ ] Check the reply to a relative change: the document's frequency example shows `821025 0 0` without the Frequency keyword, which the core would not match. ([specs/sennheiser-ew-g3-g4.yaml:224](specs/sennheiser-ew-g3-g4.yaml#L224))
- [ ] Confirm the SR answers `Equalizer 1` with all six values, and the EM's AF third value carries the current mute bits as the AF table gives them. ([specs/sennheiser-ew-g3-g4.yaml:256](specs/sennheiser-ew-g3-g4.yaml#L256))
- [ ] Confirm a Push with mode 3 sends every configuration attribute at once, so renewing it every 8 s keeps the configuration current without other reads. ([specs/sennheiser-ew-g3-g4.yaml:377](specs/sennheiser-ew-g3-g4.yaml#L377))

## sennheiser-l-6000 — Sennheiser L 6000

- [ ] Confirm the charger answers a command datagram with the method's new value (or an /osc/error entry) to the sender's port, so each command is acknowledged by its reply. ([spec](specs/sennheiser-l-6000.yaml#L63))
- [ ] Confirm the SSC port is 45 on the L 6000 (the guide gives 45 as the SSC default; its DNS-SD record names the port actually used). ([spec](specs/sennheiser-l-6000.yaml#L38))
- [ ] Confirm the name query used as the liveness probe is answered while nothing else is happening. ([spec](specs/sennheiser-l-6000.yaml#L44))
- [ ] Confirm that querying a bay's identify (and /device/identify) with null starts the identification and is answered true, how long it lasts, and whether anything stops it. ([spec](specs/sennheiser-l-6000.yaml#L423))
- [ ] Check whether /device/storage_mode accepts a write (the guide marks it read-only but describes it as set and returned) and what the reply is. ([spec](specs/sennheiser-l-6000.yaml#L414))
- [ ] Check the device name's length and character limits (none documented; the spec takes 32 characters without braces, brackets, backslash or quote, as for the EM 6000) and that the reply carries the MAC address appended. ([spec](specs/sennheiser-l-6000.yaml#L86))
- [ ] Confirm auto false and the three static_* values are applied from one message, and what the charger answers. ([spec](specs/sennheiser-l-6000.yaml#L102))
- [ ] Check what /device/network/ipv4/mdns true does to the addressing in practice, and whether it may be written while auto is true. ([spec](specs/sennheiser-l-6000.yaml#L112))
- [ ] Find out what the asterisk on ipaddr, netmask and gateway ("Read/Write*") means, and whether they come as plain strings (as the guide shows) or arrays. ([spec](specs/sennheiser-l-6000.yaml#L436))
- [ ] Confirm the three subscription address trees (slot LEDs, detection, types and device warnings and product; accu_parameter of slots 1-2; of slots 3-4) are each accepted in one subscription with a 60 s lifetime, and that the notifications fit a datagram. ([spec](specs/sennheiser-l-6000.yaml#L121))
- [ ] Confirm the two multi-method poll queries (device basics; network) are answered in one reply each. ([spec](specs/sennheiser-l-6000.yaml#L127))
- [ ] Record what /slotN/type reports for an LM 6062 and an LM 6070 module (the guide lists 0-3 only). ([spec](specs/sennheiser-l-6000.yaml#L397))
- [ ] Check what accu_parameter holds for an empty bay, and whether its integer fields arrive as integers or as decimals (24.0 for temperature in the guide's example). ([spec](specs/sennheiser-l-6000.yaml#L153))
- [ ] Confirm the operating time and time to full are hours plus minutes, and what time to full reads when the battery is full or not charging (the guide's example shows 340 h 30 min). ([spec](specs/sennheiser-l-6000.yaml#L405))
- [ ] Confirm /device/warnings uses the numbers the guide lists (0-36) and reports an empty array when nothing is wrong. ([spec](specs/sennheiser-l-6000.yaml#L303))
- [ ] Confirm /device/identity/product, version and vendor answer as the guide shows ("L6000"), and that /device/language is an array. ([spec](specs/sennheiser-l-6000.yaml#L313))
- [ ] Confirm the charger accepts subscriptions from sixteen clients and refuses a seventeenth. ([spec](specs/sennheiser-l-6000.yaml#L388))

## sennheiser-spectera — Sennheiser Spectera

- [ ] Confirm the API accepts controlSennheiser with the device password on firmware 1.4.x, and whether the user api works on any firmware yet. ([specs/sennheiser-spectera.yaml:539](specs/sennheiser-spectera.yaml#L539))
- [ ] Note how long the Base Station blocks an address after bad credentials, so the terminal refusal can be weighed against it. ([specs/sennheiser-spectera.yaml:539](specs/sennheiser-spectera.yaml#L539))
- [ ] Confirm the first notification of each collection after it is added is the whole list, and that later ones are the changed item alone on the collection's path (fixed lists) or on the item's path (links, mobile devices). ([specs/sennheiser-spectera.yaml:591](specs/sennheiser-spectera.yaml#L591))
- [ ] Confirm a created audio link is notified as {} and then in full, and a deleted one as null. ([specs/sennheiser-spectera.yaml:591](specs/sennheiser-spectera.yaml#L591))
- [ ] Check the Base Station sends nothing on an idle stream, and that a version check after 3 s of quiet never counts against its connection limit or trips a 503. ([specs/sennheiser-spectera.yaml:612](specs/sennheiser-spectera.yaml#L612))
- [ ] Confirm a PUT to a mobile device without its type is refused, and that one naming the type with only the changed field is applied. ([specs/sennheiser-spectera.yaml:605](specs/sennheiser-spectera.yaml#L605))
- [ ] Check how long an open WebUI waits before deleting an audio link no mobile device uses (documented as about 5 s). ([specs/sennheiser-spectera.yaml:578](specs/sennheiser-spectera.yaml#L578))
- [ ] Check polling /api/audio/metering at the chosen interval while subscribed elsewhere does not disturb the stream. ([specs/sennheiser-spectera.yaml:599](specs/sennheiser-spectera.yaml#L599))
- [ ] On firmware 1.3.x (API 17.0), check which subscriptions are refused (the aoip status, metering) and whether the rest still subscribe in their batch. ([specs/sennheiser-spectera.yaml:619](specs/sennheiser-spectera.yaml#L619))
- [ ] Confirm bandwidthMode 10000 is accepted but never transmits, as the release notes say, which is why set_rf_bandwidth offers 6000 and 8000 only. ([specs/sennheiser-spectera.yaml:559](specs/sennheiser-spectera.yaml#L559))

## sennheiser-tc-bar — Sennheiser TeamConnect Bar S / Bar M

- [ ] Confirm that one PUT of every readable resource to the session is accepted on both bars, and that the initial values arrive on the stream. ([specs/sennheiser-tc-bar.yaml:1292](specs/sennheiser-tc-bar.yaml#L1292))
- [ ] Confirm that restart, the camera full-view and preset PUTs take no body. ([specs/sennheiser-tc-bar.yaml:954](specs/sennheiser-tc-bar.yaml#L954))
- [ ] Confirm that a PUT of one field leaves the resource's other fields as they were. ([specs/sennheiser-tc-bar.yaml:1146](specs/sennheiser-tc-bar.yaml#L1146))
- [ ] Record a full 802.1X configuration exchange (both methods), the error codes for bad PEM files, and how the bar answers while it re-authenticates. ([specs/sennheiser-tc-bar.yaml:1061](specs/sennheiser-tc-bar.yaml#L1061))

## sennheiser-tcc-m — Sennheiser TeamConnect Ceiling Medium

- [ ] Confirm that one PUT of every readable resource to the session is accepted (a refused entry refuses the whole list) and that the initial values arrive on the stream. ([specs/sennheiser-tcc-m.yaml:812](specs/sennheiser-tcc-m.yaml#L812))
- [ ] Confirm that a PUT of one field leaves the resource's other fields as they were. ([specs/sennheiser-tcc-m.yaml:722](specs/sennheiser-tcc-m.yaml#L722))
- [ ] Confirm the device's certificate is self-signed (accepted here) and that the event stream needs the same Basic credentials. ([specs/sennheiser-tcc-m.yaml:41](specs/sennheiser-tcc-m.yaml#L41))
- [ ] Record what a reference gain write answers while automatic adjustment is on (409 or 422 documented). ([specs/sennheiser-tcc-m.yaml:395](specs/sennheiser-tcc-m.yaml#L395))

## sennheiser-tcc2 — Sennheiser TeamConnect Ceiling 2

- [ ] Confirm the three subscriptions (device and beam, audio, meters limited to 250 ms) are accepted with a 60 s lifetime, renewed every 20 s, and that their notifications fit a datagram. ([specs/sennheiser-tcc2.yaml:597](specs/sennheiser-tcc2.yaml#L597))
- [ ] Confirm that the noise gate threshold and hold time, Dante output gain, AEC reference gain, voice lift emergency mute time and installation type accept writes (the document gives examples but does not mark them writeable). ([specs/sennheiser-tcc2.yaml:220](specs/sennheiser-tcc2.yaml#L220))
- [ ] Confirm that null elements in /audio/exclusion/zones and /audio/exclusion/active keep those zones, and what the device answers. ([specs/sennheiser-tcc2.yaml:395](specs/sennheiser-tcc2.yaml#L395))
- [ ] Confirm that the manual addresses and auto false in one message move the device, and what it answers before moving. ([specs/sennheiser-tcc2.yaml:354](specs/sennheiser-tcc2.yaml#L354))
- [ ] Check whether the TCC2 also takes SSC over TCP 45 (Sennheiser documents UDP only). ([specs/sennheiser-tcc2.yaml:41](specs/sennheiser-tcc2.yaml#L41))
- [ ] Record the device time's format in answers (seconds since 2000) and whether it can be set while synchronised. ([specs/sennheiser-tcc2.yaml:172](specs/sennheiser-tcc2.yaml#L172))

## sharp-display — Sharp professional displays (S-Format)

- [ ] Record the login exchange on TCP 10008: the exact `Login:` and `Password:` prompts (trailing space, any line ending), the `OK` after the password, and what a wrong user name or password is answered with (Sharp documents nothing). ([specs/sharp-display.yaml:101](specs/sharp-display.yaml#L101))
- [ ] Confirm the login is asked for with no user name or password set, and that CR alone is taken then. ([specs/sharp-display.yaml:105](specs/sharp-display.yaml#L105))
- [ ] Confirm zero padding (`VOLM0030`) is taken on the PN-HB and PN-R903A, whose manuals show space padding (`VOLM  30`). ([specs/sharp-display.yaml:1257](specs/sharp-display.yaml#L1257))
- [ ] Confirm a negative value as a minus and three digits (`OFSR-005`) and a positive one zero-padded (`OFSR0005`). ([specs/sharp-display.yaml:371](specs/sharp-display.yaml#L371))
- [ ] Note whether sending the next command right after a reply, without Sharp's 100 ms gap, is ever refused, and how long WAIT lasts for power, input and size changes against the 15 s timeout. ([specs/sharp-display.yaml:71](specs/sharp-display.yaml#L71))
- [ ] Confirm query replies carry no leading zeros (`30`, not `0030`) for the enum codes read into names (inputs and others). ([specs/sharp-display.yaml:159](specs/sharp-display.yaml#L159))
- [ ] Record the `INF1` and `SRNO` replies (padding, quotes) on each family. ([specs/sharp-display.yaml:3493](specs/sharp-display.yaml#L3493))
- [ ] Confirm `ERRT` is in degrees Celsius and record the PN-R903A's six-value reply. ([specs/sharp-display.yaml:3478](specs/sharp-display.yaml#L3478))
- [ ] Record the `TSAD????` and `INnE????` replies (with or without quotes). ([specs/sharp-display.yaml:2074](specs/sharp-display.yaml#L2074))
- [ ] Record what `SCnn????` answers and the meaning of its two weekday digits. ([specs/sharp-display.yaml:2255](specs/sharp-display.yaml#L2255))
- [ ] Confirm the exact `LOCKED` and `UNSELECTED` texts on the PN-HB and PN-R903A (read from garbled Japanese PDFs). ([specs/sharp-display.yaml:5727](specs/sharp-display.yaml#L5727))
- [ ] Record whether the PN-LC, PN-HW2 and PN-ME2 take commands their short tables leave out (for example `MPOS`, `CONT`). ([specs/sharp-display.yaml:5698](specs/sharp-display.yaml#L5698))
- [ ] Drive a Sharp display on its RS-232C terminal (link serial): confirm no login is asked for and the replies are those of the LAN, each ended CR LF. ([specs/sharp-display.yaml:5736](specs/sharp-display.yaml#L5736))
- [ ] Confirm the minute Sharp asks for after a power or reset command counts from the final OK (after any WAIT), and that RSET (all reset) is the reset command meant; ARST and CRST are not held. ([specs/sharp-display.yaml:80](specs/sharp-display.yaml#L80))

## showcuesystems-scs — SCS (Show Cue System)

- [ ] Check that OSC over UDP is accepted with SCS as a Network Server on the Listen on Port No. given, and whether SCS offers a port of its own when none is set. ([specs/showcuesystems-scs.yaml:187](specs/showcuesystems-scs.yaml#L187))
- [ ] Confirm that SCS sends nothing back to the sender for network cue control messages. ([specs/showcuesystems-scs.yaml:195](specs/showcuesystems-scs.yaml#L195))
- [ ] Check the master and device fader range SCS caps at (the help page gives -75 dB to 0 dB as an example). ([specs/showcuesystems-scs.yaml:214](specs/showcuesystems-scs.yaml#L214))
- [ ] Check that /ctrl/fadeall (from the Companion module, not in SCS's table) fades all cues. ([specs/showcuesystems-scs.yaml:216](specs/showcuesystems-scs.yaml#L216))

## shure-ani — Shure ANI4IN, ANI4OUT, ANIUSB-MATRIX, ANI22

- [ ] Check that the ANI4IN and ANI4OUT accept two-digit channel numbers (01), which the session sends, though their documents write one digit. ([specs/shure-ani.yaml:443](specs/shure-ani.yaml#L443))
- [ ] Check that < REP ERR > is the answer to an invalid command and nothing else. ([specs/shure-ani.yaml:60](specs/shure-ani.yaml#L60))
- [ ] Check that GET 00 ALL is accepted by every ANI (the ANIUSB-MATRIX document limits GET ALL to V1 units) and that the answers end. ([specs/shure-ani.yaml:767](specs/shure-ani.yaml#L767))
- [ ] Check that the analog gain is sent and reported as two digits (00-51) and in 3 dB steps. ([specs/shure-ani.yaml:466](specs/shure-ani.yaml#L466))
- [ ] Confirm that CHAN_LED_IN_STATE is not answered on the ANI4IN and whether it is on the ANI22. ([specs/shure-ani.yaml:544](specs/shure-ani.yaml#L544))
- [ ] Check LED_BRIGHTNESS's range on current firmware (0-2 documented). ([specs/shure-ani.yaml:577](specs/shure-ani.yaml#L577))
- [ ] Capture SAMPLE with metering on, on each model, and confirm the values' order (ANI4IN's document labels its four values output 1-4; ANI22's inputs 1-2 then outputs 1-2; ANIUSB-MATRIX's ten channels) and whether the four-value form ends with two spaces before >. ([specs/shure-ani.yaml:775](specs/shure-ani.yaml#L775))

## shure-chargers — Shure networked battery chargers

- [ ] Check whether SBC250, SBC450 and SBC850 report changes unasked (their bulletin does not say), and how they answer GET 0 ALL. ([specs/shure-chargers.yaml:542](specs/shure-chargers.yaml#L542))
- [ ] Confirm that each charger accepts a TCP connection on 2202 (the documents say it is the server, the user guides the client). ([specs/shure-chargers.yaml:623](specs/shure-chargers.yaml#L623))
- [ ] Record BATT_MODULE_TYPE on an SBC220 and an SBC240, primary and secondary units. ([specs/shure-chargers.yaml:629](specs/shure-chargers.yaml#L629))
- [ ] Record the SBC250/SBC450/SBC850 DEVICE_ID and MODEL replies (with or without braces). ([specs/shure-chargers.yaml:328](specs/shure-chargers.yaml#L328))
- [ ] Check that SET NET_SETTINGS is answered with nothing (no asynchronous report is documented) and accepts unpadded addresses. ([specs/shure-chargers.yaml:367](specs/shure-chargers.yaml#L367))

## shure-imx-room — Shure IntelliMix Room

- [ ] Check that GET 00 ALL is answered by IntelliMix Room and that its answers end. ([specs/shure-imx-room.yaml:398](specs/shure-imx-room.yaml#L398))
- [ ] Check that the gain step keywords are accepted in lower case (inc, dec), as the document writes them. ([specs/shure-imx-room.yaml:185](specs/shure-imx-room.yaml#L185))
- [ ] Check how licence values are padded (LIC_TYPE is documented as 10 characters). ([specs/shure-imx-room.yaml:260](specs/shure-imx-room.yaml#L260))
- [ ] Check the matrix gain REP's spacing (the document shows the gain run into the output number in one example). ([specs/shure-imx-room.yaml:343](specs/shure-imx-room.yaml#L343))

## shure-mxa — Shure Microflex Advance arrays

- [ ] Check whether the MXA910 and MXA310 accept lower-case inc and dec (their documents write INC and DEC). ([specs/shure-mxa.yaml:1077](specs/shure-mxa.yaml#L1077))
- [ ] Check that GET 00 ALL is accepted on every model and firmware (some documents write GET 0 ALL or GET ALL). ([specs/shure-mxa.yaml:1842](specs/shure-mxa.yaml#L1842))
- [ ] Check PRESET_NAME's answer form (< REP PRESET_NAME nn name >) on the MXA910 and MXA310, whose documents use PRESET1-PRESET10 instead. ([specs/shure-mxa.yaml:972](specs/shure-mxa.yaml#L972))
- [ ] Check the speech gating and noise filter values each model accepts (Off/Low/Medium/High on the MXA920, ON/OFF on the MXA902 and MXA901) and their case in REP. ([specs/shure-mxa.yaml:1232](specs/shure-mxa.yaml#L1232))
- [ ] Check LED_BRIGHTNESS's range per model and firmware (0-2 or 0-5). ([specs/shure-mxa.yaml:1337](specs/shure-mxa.yaml#L1337))
- [ ] Check that lobe X and Y are accepted as four digits (0000-3048) on SET. ([specs/shure-mxa.yaml:1472](specs/shure-mxa.yaml#L1472))
- [ ] Check the MXA310 polar pattern names (SUPER, HYPER, BIDIRECTION as documented). ([specs/shure-mxa.yaml:1731](specs/shure-mxa.yaml#L1731))
- [ ] Capture SAMPLE with metering on, and confirm the values follow Channel Number Assignments for automatic coverage on and off, and that METER_RATE, METER_RATE_POSTGATE, METER_RATE_MXR_GAIN and METER_RATE_AECREF all report as plain SAMPLE (so only one should run at a time); record the MXA910, MXA710, MXA310 and MXA902/901 forms, taken here from the MXA920 document. ([specs/shure-mxa.yaml:1851](specs/shure-mxa.yaml#L1851))
- [ ] Capture SAMPLE TALKER_POSITIONS on an MXA920 (and an MXA910 if it reports them) and confirm the six values per talker (lobe, coverage area, X, Y, Z, reserved) as the document's example gives them. ([specs/shure-mxa.yaml:1864](specs/shure-mxa.yaml#L1864))

## shure-mxcw — Shure Microflex Complete Wireless

- [ ] Record the ranges of MAX_TOTAL_SPEAKERS, MAX_DELEGATE_SPEAKERS, MAX_NUM_REQUESTS and MIC_PRIORITY (the document gives one digit and no range). ([specs/shure-mxcw.yaml:298](specs/shure-mxcw.yaml#L298))
- [ ] Confirm the AUDIO_SAMPLE layout (aux input peak and RMS, then a peak and RMS per uplink slot) and how many slots it carries. ([specs/shure-mxcw.yaml:913](specs/shure-mxcw.yaml#L913))
- [ ] Record the RSSI report's value format (RF_METER_RATE). ([specs/shure-mxcw.yaml:917](specs/shure-mxcw.yaml#L917))
- [ ] Check whether AUX_INPUT_GAIN replies are padded to three digits (the examples show 4 and 12). ([specs/shure-mxcw.yaml:505](specs/shure-mxcw.yaml#L505))
- [ ] Confirm START_VOTE takes the configuration number unpadded and what VOTING_CONFIGURATION reports. ([specs/shure-mxcw.yaml:700](specs/shure-mxcw.yaml#L700))

## shure-mxn5 — Shure MXN5-C

- [ ] Check what an MXN5-C that is not V1 answers to GET 00 ALL. ([specs/shure-mxn5.yaml:460](specs/shure-mxn5.yaml#L460))
- [ ] Check that the gain step keywords are accepted in lower case. ([specs/shure-mxn5.yaml:267](specs/shure-mxn5.yaml#L267))
- [ ] Check that SIG_GEN on channel 03 starts and stops the generator and what the 00 index does. ([specs/shure-mxn5.yaml:403](specs/shure-mxn5.yaml#L403))
- [ ] Capture SAMPLE with metering on, and confirm its four values follow Channel Number Assignments (1-2 Dante inputs, 3 summed input, 4 Dante output). ([specs/shure-mxn5.yaml:468](specs/shure-mxn5.yaml#L468))
- [ ] Check the PRESET_NAME answer for an empty preset ({empty}). ([specs/shure-mxn5.yaml:162](specs/shure-mxn5.yaml#L162))

## shure-mxw — Shure Microflex Wireless and MXW neXt

- [ ] Confirm the SAMPLE scales on legacy MXW access points (the neXt document's RF level minus 110 dBm and audio minus 120 dB are assumed for both). ([specs/shure-mxw.yaml:1163](specs/shure-mxw.yaml#L1163))
- [ ] Confirm that GET 0 <parameter> on a legacy access point or charger is answered with one REP per channel or bay (used on connecting). ([specs/shure-mxw.yaml:661](specs/shure-mxw.yaml#L661))
- [ ] Check which TX_STATUS values each generation takes (MUTE and STANDBY on legacy, MUTED on neXt) and what an MXWNDX charger does with SET TX_STATUS. ([specs/shure-mxw.yaml:677](specs/shure-mxw.yaml#L677))
- [ ] Confirm the mute word of INPUT_CH_MUTE, OUTPUT_CH_MUTE and AUTOMIX_OUT_MUTE (MUTE in the text, MUTED in the examples) and whether AUTOMIX_OUT_MUTE takes a channel number. ([specs/shure-mxw.yaml:594](specs/shure-mxw.yaml#L594))
- [ ] Record the Auracast sample rates the access points accept, and the AVAILABLE_CHANNELS query the document mentions. ([specs/shure-mxw.yaml:1014](specs/shure-mxw.yaml#L1014))
- [ ] Check the LED_STATUS reply to a secondary microphone's set (the document shows it without SEC) and the neXt reply's CH prefix. ([specs/shure-mxw.yaml:1197](specs/shure-mxw.yaml#L1197))
- [ ] Check whether UNLINK also works on MXWNCS charger bays, as the document's wording suggests. ([specs/shure-mxw.yaml:634](specs/shure-mxw.yaml#L634))

## shure-p300 — Shure P300

- [ ] Check that a SET answered by the REP of the new value is never interleaved with a change report for the same parameter on another channel, or note how often it happens. ([specs/shure-p300.yaml:40](specs/shure-p300.yaml#L40))
- [ ] Check that recall_preset accepts the two-digit form (SET PRESET 03). ([specs/shure-p300.yaml:261](specs/shure-p300.yaml#L261))
- [ ] Check that INC and DEC steps are in tenths of a dB as documented, and how a step beyond the range is answered. ([specs/shure-p300.yaml:366](specs/shure-p300.yaml#L366))
- [ ] Check the AEC reference values on firmware 4.1 and later (Dante outputs 3-8 added). ([specs/shure-p300.yaml:554](specs/shure-p300.yaml#L554))
- [ ] Check what GATE_INHIBIT answers on firmware 4.1 and later, where it is documented not to work. ([specs/shure-p300.yaml:681](specs/shure-p300.yaml#L681))
- [ ] Check the matrix gain REP's spacing (the document shows < REP xx MATRIX_MXR_GAIN yyzzzz > once). ([specs/shure-p300.yaml:991](specs/shure-p300.yaml#L991))
- [ ] Capture SAMPLE_IN, SAMPLE_OUT, SAMPLE_PROC, SAMPLE_ERLE and SAMPLE_AGC with metering on, and confirm each lists one value per channel in the document's order (14, 12 as the order lists, 12, 8 and 8 values), three digits each (ERLE and AGC shown as aaa too), and record the AGC value's unit and range, which the document gives only as ERLE's. ([specs/shure-p300.yaml:1110](specs/shure-p300.yaml#L1110))

## shure-uhfr — Shure UHF-R receivers

- [ ] Confirm the framing: a "* " before and " *" after each message (as the bulletin prints it), no line ending, and replies sent to the sender's address and port. ([specs/shure-uhfr.yaml:40](specs/shure-uhfr.yaml#L40))
- [ ] Confirm that SET GROUP_CHAN takes the group and the channel each one less than their numbers, and its digit width. ([specs/shure-uhfr.yaml:137](specs/shure-uhfr.yaml#L137))
- [ ] Check how long METER updating lasts and whether repeating it every minute disturbs anything; and whether REPORTs come without METER at all. ([specs/shure-uhfr.yaml:234](specs/shure-uhfr.yaml#L234))
- [ ] Record the dB value of each AUDIO_GAIN step (00 the most, 32 the least). ([specs/shure-uhfr.yaml:98](specs/shure-uhfr.yaml#L98))
- [ ] Record the SAMPLE field widths and the RF codes on a receiver. ([specs/shure-uhfr.yaml:227](specs/shure-uhfr.yaml#L227))

## shure-wireless — Shure wireless receivers

- [ ] On AD4Q with frequency diversity combined (FD-C), check the second RF section in SAMPLE and that the first is the right one to report. ([specs/shure-wireless.yaml:741](specs/shure-wireless.yaml#L741))
- [ ] Confirm the PSM1000 answers REPORT with string values without braces. ([specs/shure-wireless.yaml:772](specs/shure-wireless.yaml#L772))
- [ ] Opened for commands only, confirm a receiver that is not metering answers `GET DEVICE_ID` (`GET DEVICE_NAME` on the PSM1000) promptly enough for the 5 s liveness check. ([crates/core/src/modules/shure.rs:50](crates/core/src/modules/shure.rs#L50))
- [ ] Confirm `< REP ERR >` carries no channel or parameter and that SETs are answered in order, so failing the oldest waiting command with it is right. ([specs/shure-wireless.yaml:680](specs/shure-wireless.yaml#L680))
- [ ] Check that ULX-D, QLX-D and the PSM1000 accept a group and channel padded to two characters (`GROUP_CHAN 03,07`), and how a three-character channel is written. ([specs/shure-wireless.yaml:280](specs/shure-wireless.yaml#L280))
- [ ] On Axient Digital and SLX-D, confirm a channel name SET is limited to 8 characters (the current documents) rather than 31 (the 2018 Axient Digital document). ([specs/shure-wireless.yaml:297](specs/shure-wireless.yaml#L297))
- [ ] On the PSM1000, check whether a device or channel name with a space can be set unbraced, and whether `GET x CHAN_NAME` is answered with the channel number (the table omits it). ([specs/shure-wireless.yaml:308](specs/shure-wireless.yaml#L308))
- [ ] Confirm `GET x SLOT_... 0` answers every one of the channel's eight slots, and whether `GET x ALL` already includes the slots. ([crates/core/src/modules/shure.rs:62](crates/core/src/modules/shure.rs#L62))
- [ ] Check whether `SET NET_SETTINGS` is answered before the receiver moves to its new address or restarts. ([specs/shure-wireless.yaml:341](specs/shure-wireless.yaml#L341))
- [ ] On ULX-D, confirm the sampled audio level is dBFS + 50 (the current document; the 2018 one gives 0-50 without units). ([specs/shure-wireless.yaml:600](specs/shure-wireless.yaml#L600))
- [ ] On ULX-D, confirm TX_OFFSET is reported offset by 12 (the current document; the 2018 one lists 000-021 as the offset itself). ([specs/shure-wireless.yaml:617](specs/shure-wireless.yaml#L617))
- [ ] On ULX-D firmware before 1.4.8, check that the per-channel `ENCRYPTION ON/OFF` reports are the same on every channel, since they are kept as one device value. ([specs/shure-wireless.yaml:557](specs/shure-wireless.yaml#L557))
- [ ] Check that ULX-D answers `SET HIGH_DENSITY` with a REP of the new value, and whether changing it retunes the channels. ([specs/shure-wireless.yaml:377](specs/shure-wireless.yaml#L377))
- [ ] Check that a dual ULX-D answers an inapplicable summing or diversity mode with the mode it kept, not REP ERR. ([specs/shure-wireless.yaml:401](specs/shure-wireless.yaml#L401))
- [ ] On the PSM1000, confirm AUDIO_IN_LVL is written and reported as signed dB from -67 to 0 and RF_TX_LVL as 10, 50 or 100 (Companion's values; Shure's page gives none), and whether the reply is RF_TX_LVL or RF_TX_LEVEL. ([specs/shure-wireless.yaml:506](specs/shure-wireless.yaml#L506))
- [ ] On Axient Digital, confirm slot SETs for a linked, active ADX transmitter are answered with the slot's REP, and that SLOT_INPUT_PAD takes 0 and 12. ([specs/shure-wireless.yaml:422](specs/shure-wireless.yaml#L422))
- [ ] On QLX-D, check whether FLASH, DEVICE_ID, GROUP_CHAN and METER_RATE behave as on ULX-D (Companion offers QLX-D no FLASH; Shure publishes no QLX-D document). ([specs/shure-wireless.yaml:152](specs/shure-wireless.yaml#L152))
- [ ] On ULX-D, check that a metering interval above 65535 ms (to 99999, as documented) is accepted. ([specs/shure-wireless.yaml:327](specs/shure-wireless.yaml#L327))
- [ ] On ULX-D, confirm set_encryption, regenerate_encryption_key and set_frequency_diversity_mode need an IR sync before audio returns, and how long a key regeneration takes. ([specs/shure-wireless.yaml:654](specs/shure-wireless.yaml#L654))
- [ ] On an SLX-D+ receiver, record the REM_PAIR messages of a whole pairing (ON, REQUEST with the name in braces, ACCEPT or REJECT, OFF on timeout) and whether GET x ALL includes the LINK_* parameters. ([specs/shure-wireless.yaml:271](specs/shure-wireless.yaml#L271))
- [ ] On an SLX-D+ receiver, confirm the LINK_STATUS and LINK_TX_MODEL replies carry the slot 1 before the value, and what LINK_TX_REBOOT answers. ([specs/shure-wireless.yaml:92](specs/shure-wireless.yaml#L92))

## singular-datastream — Singular.live data stream

- [ ] Record the answer to a payload over 60 KB and to an unknown private token. ([specs/singular-datastream.yaml:69](specs/singular-datastream.yaml#L69))

## singular-live — Singular.live control app

- [ ] Record Singular's answer to an unknown app token, an unknown sub-composition and a malformed payload (status and body). ([specs/singular-live.yaml:352](specs/singular-live.yaml#L352))
- [ ] Check whether a request may name both subCompositionId and subCompositionName (the reference marks both required, the examples send one). ([specs/singular-live.yaml:347](specs/singular-live.yaml#L347))
- [ ] Confirm the control data carries subCompositionName for each sub-composition (the reference lists it, its example omits it). ([specs/singular-live.yaml:272](specs/singular-live.yaml#L272))
- [ ] Check that GET requests are not counted against the rate limits (only the POST/PATCH control and command endpoints are listed). ([specs/singular-live.yaml:261](specs/singular-live.yaml#L261))

## snapav-wattbox — SnapAV WattBox IP power (Integration Protocol)

- [ ] Record the login exchange: the exact prompts, what a wrong password is answered with, and whether "Successfully Logged In!" follows a good one on every model and firmware. ([specs/snapav-wattbox.yaml:59](specs/snapav-wattbox.yaml#L59))
- [ ] Check what "API locked" looks like and how long the lock lasts after failed logins. ([specs/snapav-wattbox.yaml:65](specs/snapav-wattbox.yaml#L65))
- [ ] Record which "~" messages a unit pushes (an outlet switched from its front panel or web page), so they can be read instead of waiting for the next poll. ([specs/snapav-wattbox.yaml:413](specs/snapav-wattbox.yaml#L413))
- [ ] Check ?ServiceTag on units with v1.7 firmware, which the document does not list. ([specs/snapav-wattbox.yaml:103](specs/snapav-wattbox.yaml#L103))
- [ ] Check what non-metered models answer ?OutletPowerStatus and ?PowerStatus with (#Error is assumed). ([specs/snapav-wattbox.yaml:83](specs/snapav-wattbox.yaml#L83))
- [ ] Check !NetworkSet with the host name alone selects DHCP, as the document implies without giving the exact message. ([specs/snapav-wattbox.yaml:259](specs/snapav-wattbox.yaml#L259))

## sofie-chef — Sofie Chef

- [ ] Record the REST answers: GET /api/status and /api/list bodies (the list is read as the WebSocket list body) and the PUT routes' status and body. ([specs/sofie-chef.yaml:177](specs/sofie-chef.yaml#L177))
- [ ] Confirm an empty apiKey query parameter is accepted when no apiKey is configured. ([specs/sofie-chef.yaml:175](specs/sofie-chef.yaml#L175))
- [ ] Confirm the WebSocket API on apiPort + 1 (path /) pushes status on connecting and on each change without a request or the apiKey. ([specs/sofie-chef.yaml:186](specs/sofie-chef.yaml#L186))

## sofie-core — Sofie Core (User Actions REST API and Live Status Gateway)

- [ ] Confirm the API answers under /api/v1.0 on port 3000 of a production Sofie Core, and which statuses a PUT or POST answers with. ([specs/sofie-core.yaml:44](specs/sofie-core.yaml#L44))
- [ ] Confirm the Live Status Gateway accepts the four subscriptions on one connection and sends each topic's current state when subscribed. ([specs/sofie-core.yaml:915](specs/sofie-core.yaml#L915))
- [ ] Confirm sending false for an optional boolean (activate's rehearsal, the T-timer flags, move_next_segment's ignoreQuickLoop) behaves as leaving it out. ([specs/sofie-core.yaml:1076](specs/sofie-core.yaml#L1076))
- [ ] Confirm an activePlaylist event with no active playlist carries id null, as the gateway's schema allows. ([specs/sofie-core.yaml:1084](specs/sofie-core.yaml#L1084))

## softouch-easyworship — Softouch EasyWorship

- [ ] Confirm the whole exchange against EasyWorship 7.3 or later: the pairing request and its paired and notPaired answers, a heartbeat after each message, and status messages. ([specs/softouch-easyworship.yaml:130](specs/softouch-easyworship.yaml#L130))
- [ ] Check whether EasyWorship sends paired on the same connection once the pairing request is approved, or only on the next request (sent again every 15 seconds). ([crates/core/src/modules/easyworship.rs:39](crates/core/src/modules/easyworship.rs#L39))
- [ ] Check whether the pairing request is accepted with any uid text (meros-<name> by default), and whether repeating it while EasyWorship is asking shows a second request. ([specs/softouch-easyworship.yaml:39](specs/softouch-easyworship.yaml#L39))
- [ ] Record whether the advertised port stays the same across EasyWorship restarts. ([specs/softouch-easyworship.yaml:44](specs/softouch-easyworship.yaml#L44))
- [ ] Check what a status sent with only the logo, black and clear flags (before EasyWorship's first status) does. ([specs/softouch-easyworship.yaml:101](specs/softouch-easyworship.yaml#L101))
- [ ] Check whether gotoSlide and gotoSchedule count from 1, and what pres_no, slide_no and rectype hold. ([specs/softouch-easyworship.yaml:134](specs/softouch-easyworship.yaml#L134))
- [ ] Check whether UTF-8 is accepted where the Companion module sends Latin-1 (only the controller name can hold non-ASCII text, and it is limited to ASCII here). ([specs/softouch-easyworship.yaml:62](specs/softouch-easyworship.yaml#L62))

## softron-movierecorder — Softron MovieRecorder

- [ ] Confirm PUT /sources/record, /pause, /resume and /stop take a JSON list of source ids, as the Companion module sends them (not in Softron's public material). ([specs/softron-movierecorder.yaml:89](specs/softron-movierecorder.yaml#L89))
- [ ] Confirm GET /sources/<id>/lock, /unlock and /manual_split (from the Companion module). ([specs/softron-movierecorder.yaml:119](specs/softron-movierecorder.yaml#L119))
- [ ] Confirm PUT /sources/<id>/recording_name {recording_name} and PUT /sources/<id>/destinations with a list of ids (from the Companion module). ([specs/softron-movierecorder.yaml:137](specs/softron-movierecorder.yaml#L137))
- [ ] Confirm PUT /scheduled_recordings/<id> with duration and start_time in minutes, and stopped_by_user false (fields from the Companion module and the 4.6.0 release note). ([specs/softron-movierecorder.yaml:155](specs/softron-movierecorder.yaml#L155))
- [ ] Confirm PUT /restore_configuration and its restore_* keys, restore_vtr_bach_list as spelled. ([specs/softron-movierecorder.yaml:180](specs/softron-movierecorder.yaml#L180))
- [ ] Confirm the password goes as ?password= on every request, that an empty one is accepted when none is set, and the answer to a wrong one (401 assumed). ([specs/softron-movierecorder.yaml:61](specs/softron-movierecorder.yaml#L61))
- [ ] Confirm the /remote websocket needs no password, and the source fields of its messages (unique_id, display_name, is_recording, is_paused, is_locked, recording_name, video_format, recording_start_date, recording_end_date). ([specs/softron-movierecorder.yaml:256](specs/softron-movierecorder.yaml#L256))
- [ ] Confirm the scheduled recording fields (unique_id, name, source_unique_id, start_time, duration, is_enabled, weekly_repeated, date, stopped_by_user). ([specs/softron-movierecorder.yaml:295](specs/softron-movierecorder.yaml#L295))
- [ ] Confirm the statuses the commands answer with (200, 201 or 202 assumed). ([specs/softron-movierecorder.yaml:94](specs/softron-movierecorder.yaml#L94))

## softron-mreplay — Softron M|Replay

- [ ] Confirm the playback rate call: GET /playbackrate?rate= as the 2.2 release note gives it, or /set_playback_rate?rate= as the Companion module sends it. ([specs/softron-mreplay.yaml:137](specs/softron-mreplay.yaml#L137))
- [ ] Confirm GET /set_position?position=&reference= and its references (from the Companion module). ([specs/softron-mreplay.yaml:144](specs/softron-mreplay.yaml#L144))
- [ ] Confirm PUT /outputs/<id> with {selected: true} or {input_unique_id} (from the Companion module). ([specs/softron-mreplay.yaml:152](specs/softron-mreplay.yaml#L152))
- [ ] Confirm POST /clips and its keys (name, unique_name, reference_in_point, in_point, reference_out_point, out_point, duration) (from the Companion module). ([specs/softron-mreplay.yaml:100](specs/softron-mreplay.yaml#L100))
- [ ] Confirm the playlist calls /playlists/<id>/play, pause, stop, skip_previous, skip_next and /items/<n>/play and pause (from the Companion module). ([specs/softron-mreplay.yaml:177](specs/softron-mreplay.yaml#L177))
- [ ] Confirm the /status fields and their types (playback_rate, the *_time and *_timecode pairs, remaining_space, framerate, drop_frame, loop_recording). ([specs/softron-mreplay.yaml:291](specs/softron-mreplay.yaml#L291))
- [ ] Confirm the /inputs, /outputs, /playlists and /info fields (unique_id, name, enabled, selected, input_unique_id; application_version and the others). ([specs/softron-mreplay.yaml:317](specs/softron-mreplay.yaml#L317))
- [ ] Confirm the password goes as ?password= on every request and an empty one is accepted when none is set; and the port, 8090 (the marker article gives 8082). ([specs/softron-mreplay.yaml:56](specs/softron-mreplay.yaml#L56))

## softron-multicam-logger — Softron Multicam Logger

- [ ] Confirm add_marker and angle_changes are served at /add_marker and /angle_changes (the guide writes them without a leading slash). ([specs/softron-multicam-logger.yaml:66](specs/softron-multicam-logger.yaml#L66))
- [ ] Confirm the status and body Multicam Logger answers the commands with, and its answer when no document is open. ([specs/softron-multicam-logger.yaml:45](specs/softron-multicam-logger.yaml#L45))
- [ ] Confirm GET /angle_changes answers a JSON list (the guide's example also wraps it in braces). ([specs/softron-multicam-logger.yaml:165](specs/softron-multicam-logger.yaml#L165))
- [ ] Confirm /status gives program and preview as numbers in strings, and logging_state as a boolean. ([specs/softron-multicam-logger.yaml:105](specs/softron-multicam-logger.yaml#L105))

## softron-ontheair-video — Softron OnTheAir Video (REST API and WebSocket)

- [ ] Compare every call with the app's api.html: method (GET for the transport calls), path, and answer status (200, 201 or 202). ([specs/softron-ontheair-video.yaml:109](specs/softron-ontheair-video.yaml#L109))
- [ ] Confirm how a client authenticates when Bypass authorisation is off, and what the API answers then. ([specs/softron-ontheair-video.yaml:72](specs/softron-ontheair-video.yaml#L72))
- [ ] Confirm the playback websocket sends to a client offering playback_update_v1, and record its event fields (playback_status_changed, item_changed, playback_timing_changed, CG project property). ([specs/softron-ontheair-video.yaml:481](specs/softron-ontheair-video.yaml#L481))
- [ ] Record /playback/playing's fields and units (seconds as numbers) and /info's available_actions. ([specs/softron-ontheair-video.yaml:529](specs/softron-ontheair-video.yaml#L529))
- [ ] Confirm the item fields of `/playlists/<id>/items` (unique_id, name, duration) and that a playlist is found by index and by name. ([specs/softron-ontheair-video.yaml:554](specs/softron-ontheair-video.yaml#L554))
- [ ] Confirm `POST /playlists/<p>/items[/<index>]` with clip_type 0, 3, 4 and 5 and the name, url, live_source_name and duration fields, and what it answers. ([specs/softron-ontheair-video.yaml:316](specs/softron-ontheair-video.yaml#L316))
- [ ] Confirm GET /playback/clocked_items?enabled=0 turns the option off (only enabled=1 is in the release notes). ([specs/softron-ontheair-video.yaml:292](specs/softron-ontheair-video.yaml#L292))
- [ ] Confirm the timecode queries position_relative_timecode and position_media_timecode take HH:MM:SS:FF, and position_relative_seconds a decimal. ([specs/softron-ontheair-video.yaml:228](specs/softron-ontheair-video.yaml#L228))

## songbeamer — SongBeamer (OSC remote control)

- [ ] Confirm a query (the address alone) is answered on the same address to the sender, as on the X32. ([specs/songbeamer.yaml:37](specs/songbeamer.yaml#L37))
- [ ] Record the integer each enum answers with (presentation state, video state, live video) and confirm enums are taken as strings. ([specs/songbeamer.yaml:342](specs/songbeamer.yaml#L342))
- [ ] Confirm /xremote subscribes for 10 s and what it sends, and whether /playlist/changed reaches subscribers. ([specs/songbeamer.yaml:256](specs/songbeamer.yaml#L256))
- [ ] Confirm changes made over OSC are still not reported back on current versions (the document's known bug). ([specs/songbeamer.yaml:337](specs/songbeamer.yaml#L337))
- [ ] Confirm /video/position and /video/length answer as doubles (1.0 = 24 h) and take the string forms hh:mm:ss and +seconds. ([specs/songbeamer.yaml:321](specs/songbeamer.yaml#L321))

## sony-9pin — Sony 9-pin (RS-422 VTR protocol) decks and servers

- [ ] Check the jog, var and shuttle speeds the device runs at for the speed bytes sent (10^(N/32-2) times play speed, a second byte for finer steps), and whether speed 0 pauses it or creeps. ([specs/sony-9pin.yaml:121](specs/sony-9pin.yaml#L121))
- [ ] Check how many status bytes the device returns for 61.20 0A (ten asked; a NAK makes the core ask four), and which bits it sets. ([specs/sony-9pin.yaml:299](specs/sony-9pin.yaml#L299))
- [ ] Check what 61.0C 03 (the best of LTC and VITC) returns on the device (74.04, 74.06, 74.14 or 74.16), and the 70.0D answer when a time is missing. ([specs/sony-9pin.yaml:302](specs/sony-9pin.yaml#L302))
- [ ] Confirm the user bits' byte and nibble order (Drastic's example: 06364454 is sent 60 63 44 45) on a deck's generator and in its 74.05 returns. ([specs/sony-9pin.yaml:201](specs/sony-9pin.yaml#L201))
- [ ] Check whether the device steps a frame on 20.14 and 20.24 (documented by Drastic as an extension, not in Sony's table). ([specs/sony-9pin.yaml:135](specs/sony-9pin.yaml#L135))
- [ ] Record the device type code the device returns, and whether its model is named. ([specs/sony-9pin.yaml:95](specs/sony-9pin.yaml#L95))
- [ ] Check a serial-to-Ethernet converter at 38400 8-O-1 carries the bytes and the 9-pin timing (a reply within about 10 ms, no gap over 10 ms inside a message) closely enough for the device. ([specs/sony-9pin.yaml:351](specs/sony-9pin.yaml#L351))
- [ ] Confirm the DA-6400's 9-pin subset (shuttle by its first speed byte, cue up then pause, edit on and off) and its device type F0 1A. ([specs/sony-9pin.yaml:82](specs/sony-9pin.yaml#L82))

## sony-bravia — Sony BRAVIA Professional Displays (REST API and IRCC-IP)

- [ ] Check what a wrong or missing pre-shared key is answered with (HTTP 403 or 401, or a 200 carrying error 403), since that decides whether the core reports unauthorized. ([specs/sony-bravia.yaml:460](specs/sony-bravia.yaml#L460))
- [ ] Check that getCurrentExternalInputsStatus v1.1 answers on /sony/avContent (the reference lists it under /sony/system). ([specs/sony-bravia.yaml:499](specs/sony-bravia.yaml#L499))
- [ ] Check which getSystemInformation and getPictureQualitySettings versions each generation answers (v1.7 and v1.1 assumed on the P series). ([specs/sony-bravia.yaml:496](specs/sony-bravia.yaml#L496))
- [ ] Check the IRCC-IP SOAP envelope of the 2019 page is still accepted, with SOAPACTION and X-Auth-PSK. ([specs/sony-bravia.yaml:438](specs/sony-bravia.yaml#L438))
- [ ] Check whether getPowerStatus is answered in standby, and setPowerStatus on from standby with remote start on and off. ([specs/sony-bravia.yaml:484](specs/sony-bravia.yaml#L484))
- [ ] Check the volume range (maxVolume) on each model; 0-100 is assumed. ([specs/sony-bravia.yaml:510](specs/sony-bravia.yaml#L510))
- [ ] Check set_text (setTextForm v1.0, unencrypted) on P series displays outside Japan and on other models outside Japan and the EU. ([specs/sony-bravia.yaml:393](specs/sony-bravia.yaml#L393))

## sony-bravia-ip — Sony BRAVIA Professional Displays (Simple IP Control)

- [ ] Check the picture mute notification's polarity: the document's notification table gives 0 for muted, its answer table 1. ([specs/sony-bravia-ip.yaml:261](specs/sony-bravia-ip.yaml#L261))
- [ ] Check the volume range Simple IP control takes (0-100 assumed). ([specs/sony-bravia-ip.yaml:266](specs/sony-bravia-ip.yaml#L266))
- [ ] Check what the input notification of sixteen 0 means and when it is sent. ([specs/sony-bravia-ip.yaml:256](specs/sony-bravia-ip.yaml#L256))
- [ ] Check which commands EU models with each RED-DA specification refuse. ([specs/sony-bravia-ip.yaml:248](specs/sony-bravia-ip.yaml#L248))
- [ ] Check that an enquiry the display cannot answer (scene on an unsupported input) is answered sixteen N. ([specs/sony-bravia-ip.yaml:255](specs/sony-bravia-ip.yaml#L255))

## sony-bravia-serial — Sony BRAVIA Professional Displays (RS-232C serial control)

- [ ] Capture the data each query returns (power, input, volume, muting, ID, product information 1-3); the core reads power and muting as one byte, the input as kind and number. ([specs/sony-bravia-serial.yaml:312](specs/sony-bravia-serial.yaml#L312))
- [ ] Check which functions answer canceled (0x03) in standby. ([specs/sony-bravia-serial.yaml:301](specs/sony-bravia-serial.yaml#L301))
- [ ] Check the V shift up and down bytes: the document prints function 0x42 for them, the core sends 0x43. ([specs/sony-bravia-serial.yaml:326](specs/sony-bravia-serial.yaml#L326))
- [ ] Check the hue up/down layout (a color byte, then up or down). ([specs/sony-bravia-serial.yaml:179](specs/sony-bravia-serial.yaml#L179))
- [ ] Check the answer timing at 9600 bit/s (3 s allowed) and whether the display sends anything unasked. ([specs/sony-bravia-serial.yaml:319](specs/sony-bravia-serial.yaml#L319))

## sony-camera — Sony cameras

- [ ] Check which settings the 500 ms after an exposure mode change really covers (the reference says shooting parameters; the core holds every property change and step control, counted from the camera's answer). ([specs/sony-camera.yaml:2968](specs/sony-camera.yaml#L2968))
- [ ] Check user base look numbering on each body: whether properties.D0C7 lists the plain number 1-16 or the user-LUT form (0x0100 plus the number), and that select, delete and PPLUT commands hit the intended look. ([specs/sony-camera.yaml:3187](specs/sony-camera.yaml#L3187))
- [ ] Check zoom and focus preset slot numbering: that slot 0 is the first preset, matching save_zoom_focus_position, before relying on set_preset_zoom_only. ([specs/sony-camera.yaml:2609](specs/sony-camera.yaml#L2609))
- [ ] Check the display-list mapping: that base look, AE level offset, scene file, creative look and stream destination names come from the list types the module uses and match what the camera shows. ([crates/core/src/modules/sony_camera_info.rs:94](crates/core/src/modules/sony_camera_info.rs#L94))
- [ ] Check long shutter speeds in the 32/32 numerator-over-denominator properties (Shutter Speed Setting and Current Value): the reference fixes the denominator at 10 for long exposures only in the 16/16 property, so check that a value such as 2.5" sent as tenths of a second matches the camera's listed values. ([crates/core/src/modules/sony_camera_props.rs:31](crates/core/src/modules/sony_camera_props.rs#L31))
- [ ] On a Camera Control PTP 2 body (ILCE-7M3), check which way a positive exposure_step moves for iris, shutter speed, ISO and exposure and flash compensation. ([specs/sony-camera.yaml:2084](specs/sony-camera.yaml#L2084))
- [ ] On ILCE-7CM2, ILCE-7CR and ILX-LR1 firmware 1.00, check that focus.indication follows the AF status events. ([specs/sony-camera.yaml:3040](specs/sony-camera.yaml#L3040))
- [ ] Opened for commands only, confirm the camera tolerates GetDeviceInfo once a second as the liveness check. ([crates/core/src/modules/sony_camera.rs:48](crates/core/src/modules/sony_camera.rs#L48))
- [ ] Record the unit of Recording Duration (0xD120) while a movie records (seconds, frames or another count), so recording.duration can state it; the reference gives none. ([specs/sony-camera.yaml:2717](specs/sony-camera.yaml#L2717))
- [ ] Check what the range of Custom WB Capture Frame Size (0xD26C) holds (the frame's width and height as the maximum, or something else), since the property has no current value and the reference describes only the packing. ([specs/sony-camera.yaml:2685](specs/sony-camera.yaml#L2685))

## sounddevices-pixnet — Sound Devices / Video Devices PIX and 970 (PIXNET)

- [ ] Check every call against Sound Devices' PIXNET Web API (v1.02), which could not be retrieved: the POST of {} to /sounddevices/settransport/play, stop and rec. ([specs/sounddevices-pixnet.yaml:75](specs/sounddevices-pixnet.yaml#L75))
- [ ] Confirm the RemoteApi call encoding (/invoke/RemoteApi/<name>(<types>)/<count>/<type>,<value>...) and the PlaybackSpeed values PlayX2 and PlayX16. ([specs/sounddevices-pixnet.yaml:90](specs/sounddevices-pixnet.yaml#L90))
- [ ] Confirm the key codes and KeyEventType values of simulateKey. ([specs/sounddevices-pixnet.yaml:118](specs/sounddevices-pixnet.yaml#L118))
- [ ] Confirm setsetting/RecordToDrive<n>=<mode> and the mode names (Off, Record, Ethernet File Transfer, Switch to Network upon Full), and the drives each model has. ([specs/sounddevices-pixnet.yaml:153](specs/sounddevices-pixnet.yaml#L153))
- [ ] Confirm formatAllDrives's label rules and the EXFAT argument. ([specs/sounddevices-pixnet.yaml:141](specs/sounddevices-pixnet.yaml#L141))
- [ ] Confirm PIXNET answers 200 to a command, its answer to a refused one, and whether it asks for a login. ([specs/sounddevices-pixnet.yaml:78](specs/sounddevices-pixnet.yaml#L78))
- [ ] Find the PIXNET status replies (GET /sounddevices/transport and the settings) so the recorder's state can be read. ([specs/sounddevices-pixnet.yaml:186](specs/sounddevices-pixnet.yaml#L186))
- [ ] Check that the recorder answers GET /sounddevices/transport, and record the reply's fields so they can be read into state on their own. ([specs/sounddevices-pixnet.yaml:170](specs/sounddevices-pixnet.yaml#L170))

## spx-graphics — SPX Graphics Controller

- [ ] Record what SPX Production and SPX Broadcast answer for a wrong or missing API key (SPX Solo's code answers 200 with {"error": ...}). ([specs/spx-graphics.yaml:518](specs/spx-graphics.yaml#L518))
- [ ] Record the reply of rundown/get on SPX Broadcast and confirm it is the rundown file (templates with itemID, onair as "true"/"false", playserver, playchannel, playlayer, webplayout, relpath, out, DataFields). ([specs/spx-graphics.yaml:534](specs/spx-graphics.yaml#L534))
- [ ] Record the replies of getprojects, getrundowns, allrundowns, gettemplates, getlayerstate and getFileList, so more of them can be kept in state. ([specs/spx-graphics.yaml:539](specs/spx-graphics.yaml#L539))
- [ ] Check whether the commercial editions answer the controller commands with 200 and the same JSON as SPX Solo, and the status of an unknown item id. ([specs/spx-graphics.yaml:516](specs/spx-graphics.yaml#L516))
- [ ] Check that a load_rundown file value with its slash percent-encoded (%2F) is read as project/rundown. ([specs/spx-graphics.yaml:195](specs/spx-graphics.yaml#L195))

## stagetimer — Stagetimer

- [ ] Confirm the desktop app serves the API and Socket.IO under /v1/ on its own port, as the cloud's API URL has it, and record the API URL its Menu > API shows. ([specs/stagetimer.yaml:971](specs/stagetimer.yaml#L971))
- [ ] Confirm the Socket.IO endpoint takes Engine.IO 4 and the handshake auth {room_id, api_key}, and sends room, playback_status, current_timer, next_timer and the message on connecting. ([specs/stagetimer.yaml:683](specs/stagetimer.yaml#L683))
- [ ] Confirm a single labels[0][name] and labels[0][color] pair replaces a timer's labels rather than adding to them. ([specs/stagetimer.yaml:998](specs/stagetimer.yaml#L998))
- [ ] Confirm boolean query values are taken as true and false (autostart, from_memory, focus, bold, uppercase, start_time_uses_date). ([specs/stagetimer.yaml:162](specs/stagetimer.yaml#L162))
- [ ] Check whether the rate limit answers 429 and how RateLimit-* headers report it with the 600 ms pacing. ([specs/stagetimer.yaml:72](specs/stagetimer.yaml#L72))
- [ ] Check whether create_timer accepts every documented field at once with its documented default (including an empty speaker and notes). ([specs/stagetimer.yaml:376](specs/stagetimer.yaml#L376))

## streamlabs-desktop — Streamlabs Desktop

- [ ] Confirm the raw websocket at /api/websocket on port 59650 carries the JSON-RPC messages unframed, and that remote connections reach it from another computer. ([specs/streamlabs-desktop.yaml:52](specs/streamlabs-desktop.yaml#L52))
- [ ] Record the auth answer for a good and a wrong token, and confirm the refused and accepted patterns. ([specs/streamlabs-desktop.yaml:64](specs/streamlabs-desktop.yaml#L64))
- [ ] Confirm string request ids are echoed in the replies, and what a void method returns as its result. ([specs/streamlabs-desktop.yaml:103](specs/streamlabs-desktop.yaml#L103))
- [ ] Record getScenes with getModel data: confirm the nodes' fields (id, sceneNodeType, sourceId, visible, transform, parentId) and that a node's id is the SceneItem resource's second part. ([specs/streamlabs-desktop.yaml:637](specs/streamlabs-desktop.yaml#L637))
- [ ] Confirm the event payloads: itemUpdated as an ISceneItemModel with sceneId, sourceUpdated as an ISourceModel, and the streaming status events as bare strings. ([specs/streamlabs-desktop.yaml:662](specs/streamlabs-desktop.yaml#L662))
- [ ] Confirm AudioService.getSources returns each source's IAudioSourceModel (fader db, deflection, mul). ([specs/streamlabs-desktop.yaml:716](specs/streamlabs-desktop.yaml#L716))
- [ ] Confirm SceneItemFolder resources take the scene and folder IDs as SceneItemFolder["scene","folder"]. ([specs/streamlabs-desktop.yaml:191](specs/streamlabs-desktop.yaml#L191))

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

## tally-arbiter — Tally Arbiter

- [ ] With a source switching a device between preview and program, confirm devices.<id>.program and .preview follow it, including for a device with linked busses and for a bus whose type is changed; they are worked out from device_states (a bus with any source listed is active) and each bus's type in bus_options, read from Tally Arbiter's source rather than its documentation. ([specs/tally-arbiter.yaml:500](specs/tally-arbiter.yaml#L500))
- [ ] Confirm a connection registered as a listener client (listenerclient_connect, device given by id or by name) appears in the settings page's listener list, that flash and a producer message reach it (listener.flashes, listener.last_message), and that a reassignment from the settings page moves it: signed in, by answering listener_reassign; not signed in, by reconnecting and registering again, which leaves the old entry inactive. ([specs/tally-arbiter.yaml:511](specs/tally-arbiter.yaml#L511))
- [ ] Confirm device_states with no argument answers every device's busses without signing in, that an admin account's settings room and a producer account's companion room push every device's busses as they change, and that a producer account is refused the settings room with an error event (the module then joins the companion room). ([specs/tally-arbiter.yaml:523](specs/tally-arbiter.yaml#L523))
- [ ] The module takes Tally Arbiter's error event as the answer to the oldest command waiting, and manage_response, which goes to every client in the settings room, as the answer to its oldest manage command: check that two admin sessions changing things at once do not see each other's results. ([specs/tally-arbiter.yaml:533](specs/tally-arbiter.yaml#L533))
- [ ] Check that a wrong password and the "Too many attemps" lockout both come back as login_response with loginOk false (taken as a final refusal), that an account on the shipped password is sent must_change_password and can then use change_password, and that after two days (an expired token, after 3.3.0) invalid_access_token arrives and signing in again restores the session. ([specs/tally-arbiter.yaml:547](specs/tally-arbiter.yaml#L547))
- [ ] Confirm delete_user deletes the account yet is answered with result error, "User not found." (deleteUser returns false in every case in the source read). ([specs/tally-arbiter.yaml:569](specs/tally-arbiter.yaml#L569))
- [ ] On a 3.3.0 server, confirm chat_enabled is ignored and a duplicate manage event is answered with no result; on a later one, that set_chat_enabled is echoed to every client and duplicate_device answers the copy's deviceId. ([specs/tally-arbiter.yaml:584](specs/tally-arbiter.yaml#L584))
- [ ] On a phone, confirm Tally Arbiter's Engine.IO pings are answered so it keeps the connection, that a connection dropped while the app was suspended is reopened by mi_reconnect_all with the listener client registered again, and that two unanswered version requests (10 s) drop a silent connection. ([specs/tally-arbiter.yaml:591](specs/tally-arbiter.yaml#L591))

## tascam-bd-mp1 — TASCAM BD-MP1 / BD-MP1MKII

- [ ] Confirm how the player ends its messages: "ack" and "nack" with or without CR, and statuses (such as !7SSTPL) ended by CR; TEAC's later document shows them without CR. ([specs/tascam-bd-mp1.yaml:50](specs/tascam-bd-mp1.yaml#L50))
- [ ] Confirm the host's ACK is "ack" alone (no CR after it), that each status notification answered by it is not sent again, and that a status request's own answer and ?PWR's ack want none. ([specs/tascam-bd-mp1.yaml:56](specs/tascam-bd-mp1.yaml#L56))
- [ ] Confirm no flow control is needed on RS-232C. ([specs/tascam-bd-mp1.yaml:66](specs/tascam-bd-mp1.yaml#L66))
- [ ] Confirm a status request is answered "ack" first, then the status, and a set with "ack" alone. ([specs/tascam-bd-mp1.yaml:134](specs/tascam-bd-mp1.yaml#L134))
- [ ] Confirm the status notifications of the chapter and chapter count use the short forms TN and TT, the answers TNM and TTN. ([specs/tascam-bd-mp1.yaml:649](specs/tascam-bd-mp1.yaml#L649))
- [ ] Confirm the answer to ?SCN is the direction and speed letters (!7SCNFf). ([specs/tascam-bd-mp1.yaml:675](specs/tascam-bd-mp1.yaml#L675))
- [ ] Confirm the BD-MP1MKII's color keys are CBCX1-CBCX4 as its document writes them, not CBC1-CBC4. ([specs/tascam-bd-mp1.yaml:287](specs/tascam-bd-mp1.yaml#L287))
- [ ] Confirm power on works only on RS-232C, and that get_power gets no answer in standby. ([specs/tascam-bd-mp1.yaml:131](specs/tascam-bd-mp1.yaml#L131))
- [ ] Confirm the speaker setting's level form (a sign and two digits) and the delay's unit (centimeters on the BD-MP1MKII, milliseconds on the BD-MP1) and range. ([specs/tascam-bd-mp1.yaml:473](specs/tascam-bd-mp1.yaml#L473))

## tascam-bd-mp4k — TASCAM BD-MP4K

- [ ] Confirm how the player ends its messages: "ack" and "nack" with or without CR, and statuses (such as !7SSTPL) ended by CR; Ver. 1.01 shows them without CR. ([specs/tascam-bd-mp4k.yaml:48](specs/tascam-bd-mp4k.yaml#L48))
- [ ] Confirm the host's ACK is "ack" alone (no CR after it), that each status notification answered by it is not sent again, and that a status request's own answer and ?PWR's ack want none. ([specs/tascam-bd-mp4k.yaml:54](specs/tascam-bd-mp4k.yaml#L54))
- [ ] Confirm the RS-232C speed's factory setting (115200 assumed), and that no flow control is needed. ([specs/tascam-bd-mp4k.yaml:60](specs/tascam-bd-mp4k.yaml#L60))
- [ ] Confirm a status request is answered "ack" first, then the status, and a set with "ack" alone. ([specs/tascam-bd-mp4k.yaml:149](specs/tascam-bd-mp4k.yaml#L149))
- [ ] Confirm the status notifications of the chapter and chapter count use the short forms TN and TT, the answers TNM and TTN. ([specs/tascam-bd-mp4k.yaml:765](specs/tascam-bd-mp4k.yaml#L765))
- [ ] Confirm the downmix is read with ?DNM (as the document gives it) and answered DMX. ([specs/tascam-bd-mp4k.yaml:629](specs/tascam-bd-mp4k.yaml#L629))
- [ ] Confirm the color adjustment answer is the color letter then three signed two-digit values (!7MVCR+01-02+03). ([specs/tascam-bd-mp4k.yaml:830](specs/tascam-bd-mp4k.yaml#L830))
- [ ] Confirm power on works only on RS-232C at 57600 bit/s, and that get_power gets no answer in standby. ([specs/tascam-bd-mp4k.yaml:146](specs/tascam-bd-mp4k.yaml#L146))
- [ ] Confirm the speaker level's form, four characters in tenths of a dB rounded to 0 or 5, and the delay in centimeters. ([specs/tascam-bd-mp4k.yaml:485](specs/tascam-bd-mp4k.yaml#L485))

## tascam-cd-400u — TASCAM CD-400U / CD-400UDAB

- [ ] Confirm the RS-232C framing: LF before and CR after each message, and the player's own messages framed the same way; the network framing CR LF (the vendor command page shows LF CR). ([specs/tascam-cd-400u.yaml:68](specs/tascam-cd-400u.yaml#L68))
- [ ] Confirm no flow control is needed on RS-232C (RTS and CTS are joined inside the player). ([specs/tascam-cd-400u.yaml:73](specs/tascam-cd-400u.yaml#L73))
- [ ] Confirm the IF-E100's Telnet password is 400 by default, digits only, and what the card answers to a wrong one (TEAC documents nothing). ([specs/tascam-cd-400u.yaml:78](specs/tascam-cd-400u.yaml#L78))
- [ ] Confirm 100 ms between commands is enough, polls included. ([specs/tascam-cd-400u.yaml:66](specs/tascam-cd-400u.yaml#L66))
- [ ] Check TRACK No. RETURN's layout: the document lists four digits but gives six bytes; confirm whether two EOM digits come first (as on the SS-R250N) or there are only four. ([specs/tascam-cd-400u.yaml:512](specs/tascam-cd-400u.yaml#L512))
- [ ] Check how a DAB station travels in CURRENT TRACK INFORMATION RETURN: the channel's three characters (padding of a two-character channel such as 5A) and the service number's two digits. ([specs/tascam-cd-400u.yaml:531](specs/tascam-cd-400u.yaml#L531))
- [ ] Confirm the AM (kHz, five digits) and FM (MHz to two decimals) frequency digits in CURRENT TRACK INFORMATION RETURN with the FM or AM tuner selected. ([specs/tascam-cd-400u.yaml:529](specs/tascam-cd-400u.yaml#L529))
- [ ] Confirm the play area sense answers 0F for folder skip mode, and that polls of it on an audio CD only set last_illegal. ([specs/tascam-cd-400u.yaml:402](specs/tascam-cd-400u.yaml#L402))
- [ ] Confirm which senses answer ILLEGAL in which states (track number in tuner frequency mode, remaining and total times on data CDs, SD and USB), as the polls ask for them. ([specs/tascam-cd-400u.yaml:417](specs/tascam-cd-400u.yaml#L417))
- [ ] Confirm the CD-400UDAB answers device code 30 for DAB and 31 for FM, and the CD-400U 30 for FM and 31 for AM. ([specs/tascam-cd-400u.yaml:569](specs/tascam-cd-400u.yaml#L569))
- [ ] Confirm ENTER and BACK take data 01 (and BACK 20 for push and hold), and that no release message is needed after them. ([specs/tascam-cd-400u.yaml:404](specs/tascam-cd-400u.yaml#L404))

## tascam-da6400 — TASCAM DA-6400 / DA-6400dp (TELNET protocol)

- [ ] Confirm the recorder takes and sends LF then CR at the end of each message, as its TELNET document says (TEAC's SS-series document gives CR LF for TELNET), and whether it takes CR LF as well. ([specs/tascam-da6400.yaml:56](specs/tascam-da6400.yaml#L56))
- [ ] Confirm the login: an Enter, "Enter Password", the password, "Login Successful", and what the recorder answers to a wrong password. ([specs/tascam-da6400.yaml:77](specs/tascam-da6400.yaml#L77))
- [ ] Check whether the Telnet server echoes the commands it receives (the spec never takes an echo for a reply). ([specs/tascam-da6400.yaml:62](specs/tascam-da6400.yaml#L62))
- [ ] Check what CURRENT TRACK INFORMATION (D7) returns besides the take number: the take's length (assumed) or the position in it. ([specs/tascam-da6400.yaml:915](specs/tascam-da6400.yaml#L915))
- [ ] Check where the start and end status sits in the long-operation answers (create, rebuild and delete project, format, take rename, erase and copy): the document puts it at Data 5-6 for some and Data 7-8 for others. ([specs/tascam-da6400.yaml:1063](specs/tascam-da6400.yaml#L1063))
- [ ] Confirm the output routing sense is 7F1251 with 0, the slot and FF, and its answer's 01, slot, 0 lead-in. ([specs/tascam-da6400.yaml:821](specs/tascam-da6400.yaml#L821))
- [ ] Check how MEDIA REMAIN splits the free space (whole GB and the MB beyond them, or a total in MB in the last four digits). ([specs/tascam-da6400.yaml:968](specs/tascam-da6400.yaml#L968))
- [ ] Confirm error and caution codes arrive as N2 N3 0 N1, so 1-06 is sent 0601. ([specs/tascam-da6400.yaml:1042](specs/tascam-da6400.yaml#L1042))
- [ ] Confirm CLOCK DATA RETURN carries seconds (twelve digits) while the preset takes ten. ([specs/tascam-da6400.yaml:1029](specs/tascam-da6400.yaml#L1029))
- [ ] Confirm the new project's name follows 7F024001 directly (Data 7 on) and is answered the same way. ([specs/tascam-da6400.yaml:428](specs/tascam-da6400.yaml#L428))
- [ ] Confirm take numbers above 99 (four digits tens, ones, thousands, hundreds) on a project with that many takes. ([specs/tascam-da6400.yaml:282](specs/tascam-da6400.yaml#L282))

## tascam-ss-r250n — TASCAM SS-CDR250N / SS-R250N / SS-R300N

- [ ] Confirm messages on TELNET end CR LF (the vendor command page shows LF CR), and that the recorder's own messages end the same way. ([specs/tascam-ss-r250n.yaml:44](specs/tascam-ss-r250n.yaml#L44))
- [ ] Confirm the recorder's RS-232C answers are LF, the message, then CR, as its commands are. ([specs/tascam-ss-r250n.yaml:64](specs/tascam-ss-r250n.yaml#L64))
- [ ] Confirm the login (Enter, "Enter Password", the password, "Login Successful") and the SS-R300N's default password. ([specs/tascam-ss-r250n.yaml:73](specs/tascam-ss-r250n.yaml#L73))
- [ ] Check whether the Telnet server echoes commands. ([specs/tascam-ss-r250n.yaml:50](specs/tascam-ss-r250n.yaml#L50))
- [ ] Check whether CLOCK DATA RETURN carries ten digits (as its example) or twelve (as its byte count says). ([specs/tascam-ss-r250n.yaml:992](specs/tascam-ss-r250n.yaml#L992))
- [ ] Confirm SPECIFIED DEVICE STATUS is answered D1 (the document prints 51 for the return) on the SS-CDR250N, and refused on the SS-R250N and SS-R300N. ([specs/tascam-ss-r250n.yaml:592](specs/tascam-ss-r250n.yaml#L592))
- [ ] Check that the SS-R300N takes the SS-R250N's commands (the document adds it to the list of devices without other changes) and whether it has commands of its own. ([specs/tascam-ss-r250n.yaml:118](specs/tascam-ss-r250n.yaml#L118))
- [ ] Confirm CREATE FOLDER's final answer carries the new folder's number after 11. ([specs/tascam-ss-r250n.yaml:1068](specs/tascam-ss-r250n.yaml#L1068))
- [ ] Check where MEDIA FORMAT's status sits (the document says Data 7-8). ([specs/tascam-ss-r250n.yaml:1064](specs/tascam-ss-r250n.yaml#L1064))
- [ ] Confirm TIME SEARCH's minutes are four digits in the tens, ones, thousands, hundreds order and its seconds are seconds and hundredths (the document's example disagrees with its own description); the command takes minutes up to 999. ([specs/tascam-ss-r250n.yaml:440](specs/tascam-ss-r250n.yaml#L440))

## teradek-prism — Teradek Prism (Jetpack, Flex, Mobile, Rack Unit)

- [ ] Confirm the API answers on HTTPS port 443 with a self-signed certificate the core accepts, and whether plain HTTP is served at all (the SDK has an https: false option; the documents name only HTTPS). ([specs/teradek-prism.yaml:72](specs/teradek-prism.yaml#L72))
- [ ] Confirm the token endpoint takes the form-encoded password and refresh grants without an Authorization header, and record its answer to a wrong password and to an expired refresh token (status and body), which the documents do not give. ([specs/teradek-prism.yaml:172](specs/teradek-prism.yaml#L172))
- [ ] Confirm the token endpoint is on port 443 beside the API, and how its 30 requests per minute are counted apart from the API's 60. ([specs/teradek-prism.yaml:93](specs/teradek-prism.yaml#L93))
- [ ] Record what an expired or missing access token answers on an /api/v1 path (401 is assumed, on which the session signs in again with the password grant), and what a wrong password answers the sign-in (400 or 401 with invalid_grant, taken as the terminal refusal). ([specs/teradek-prism.yaml:82](specs/teradek-prism.yaml#L82))
- [ ] Confirm GET /api/v1 answers without a token on both 2.25 and 2.26, and whether it counts toward the 60 requests per minute (the core probes with it after 10 s of silence). ([specs/teradek-prism.yaml:90](specs/teradek-prism.yaml#L90))
- [ ] Confirm the rate limit headers come with every API answer, as decimal text. ([specs/teradek-prism.yaml:1108](specs/teradek-prism.yaml#L1108))
- [ ] Check that the core's bodiless POSTs (record start and stop, stream actions, restarts, adding pipelines and streams, a plain SCTE marker, the firmware upgrade) carry Content-Length: 0 and are not answered 411. ([specs/teradek-prism.yaml:2187](specs/teradek-prism.yaml#L2187))
- [ ] Confirm input_type network is the value that selects a network input on PUT /api/v1/encoders/{id} (the documents show only physical). ([specs/teradek-prism.yaml:306](specs/teradek-prism.yaml#L306))
- [ ] Record the allowed ranges of the snapshot's width, height and quality (taken here as up to 7680 x 4320 and 1-100), and check that POST snapshot answers 200 with the image. ([specs/teradek-prism.yaml:568](specs/teradek-prism.yaml#L568))
- [ ] Check the video bitrate range (100-80000 kbit/s from the 2.25 description example) on each product and codec. ([specs/teradek-prism.yaml:381](specs/teradek-prism.yaml#L381))
- [ ] Confirm that every settings GET answers with a message starting "Settings retrieved" and every PUT with "Set successfully", so the whole-object JSON text is taken from reads only, on 2.25 and 2.26. ([specs/teradek-prism.yaml:1335](specs/teradek-prism.yaml#L1335))
- [ ] Confirm GET /api/v1/network/wifi answers with the message "Statuses received" (its whole-object JSON text depends on it). ([specs/teradek-prism.yaml:1761](specs/teradek-prism.yaml#L1761))
- [ ] Confirm the delete answers' exact messages ("Encoder 3 removed", "Decoder 1 removed", "Stream 8 removed"), which remove the item from state. ([specs/teradek-prism.yaml:1228](specs/teradek-prism.yaml#L1228))
- [ ] Confirm the 16 audio output channels, like the audio output settings, are shared by all decoders, so reading them through the first decoder listed is right. ([specs/teradek-prism.yaml:1528](specs/teradek-prism.yaml#L1528))
- [ ] Confirm GET /api/v1/streams lists the base stream 0 first, so the fast poll reads the main pipeline's status. ([specs/teradek-prism.yaml:1682](specs/teradek-prism.yaml#L1682))
- [ ] Record which products have modem endpoints and what a modem slot with nothing in it answers (the core polls slots 0-7 every 5 minutes). ([specs/teradek-prism.yaml:1097](specs/teradek-prism.yaml#L1097))
- [ ] Confirm start_all and stop_all act on every stream when sent on stream 0, as Teradek's SDK sends them. ([specs/teradek-prism.yaml:945](specs/teradek-prism.yaml#L945))
- [ ] Record what complete_stream does on each mode (taken to end a platform's broadcast), and what preview and endpreview do. ([specs/teradek-prism.yaml:2124](specs/teradek-prism.yaml#L2124))
- [ ] Record which settings other than the encoder input restart encoding when changed (codec, resolution, audio, decoder ingest). ([specs/teradek-prism.yaml:2111](specs/teradek-prism.yaml#L2111))
- [ ] Record what upgrade_firmware does (download and install, reboot, output stopped) and how long the unit is gone. ([specs/teradek-prism.yaml:2130](specs/teradek-prism.yaml#L2130))
- [ ] Record the values import_settings's type takes besides encoder. ([specs/teradek-prism.yaml:1041](specs/teradek-prism.yaml#L1041))
- [ ] On a 2.25 unit, confirm the list, inputs and capacity endpoints answer 404 and that encoder 0, decoder 0 and streams 0-4 answer on the shared paths. ([specs/teradek-prism.yaml:2088](specs/teradek-prism.yaml#L2088))
- [ ] Record the units of the audio delay and volume fields (encoder audio delay, audio output delay and embedded volume, channel volume). ([specs/teradek-prism.yaml:1991](specs/teradek-prism.yaml#L1991))
- [ ] Check that the session's own sign-in to /oauth/token, paced with the API's requests, stays within the token endpoint's 30 requests per minute. ([specs/teradek-prism.yaml:79](specs/teradek-prism.yaml#L79))
- [ ] Confirm the encoder snapshot is read with GET (as Teradek's SDK does) and the decoder's with the settings POST, each answering the JPEG itself. ([specs/teradek-prism.yaml:560](specs/teradek-prism.yaml#L560))

## timemachines-clock — TimeMachines clocks, timers and time servers (Locator Protocol API)

- [ ] Record a clock's 40-byte status and confirm bytes 20 to 39 (alarm, days, digits value, WiFi signal, name). ([specs/timemachines-clock.yaml:260](specs/timemachines-clock.yaml#L260))
- [ ] Confirm the clock answers from port 7372 to the sender's port, and answers every control request with 'A'. ([specs/timemachines-clock.yaml:30](specs/timemachines-clock.yaml#L30))
- [ ] Record what Get Program and Get Active Program answer. ([specs/timemachines-clock.yaml:168](specs/timemachines-clock.yaml#L168))
- [ ] Confirm Load Program's second byte is 4Fh as the guide writes, and that the steps are the same 32-byte structure as Store Program's. ([specs/timemachines-clock.yaml:183](specs/timemachines-clock.yaml#L183))
- [ ] Confirm the two-line clock's text takes a line feed for a new line, and the DotMatrix text's ESC form on the TM848. ([specs/timemachines-clock.yaml:154](specs/timemachines-clock.yaml#L154))
- [ ] Confirm the TM1000A and TM2000A answer the locator query with the 80-byte status. ([specs/timemachines-clock.yaml:68](specs/timemachines-clock.yaml#L68))

## tractus-fullview — Tractus FullView for NDI

- [ ] Record the answer of /displays, and whether it names each display's source, to keep state field by field. ([specs/tractus-fullview.yaml:71](specs/tractus-fullview.yaml#L71))
- [ ] Check the answer to a source change (status and body) and to a display number that does not exist. ([specs/tractus-fullview.yaml:46](specs/tractus-fullview.yaml#L46))

## tractus-multiview — Tractus Multiview for NDI

- [ ] Record /viewers, /receivers and /current answers: whether the fields are PascalCase as documented or camelCase, and the shapes of Layout, AudioPeakdBFS, ReceiverWatchdogTimeout, DefaultInstances and NdiSenderType. ([specs/tractus-multiview.yaml:539](specs/tractus-multiview.yaml#L539))
- [ ] Check that GET /current answers promptly enough to serve as the liveness probe. ([specs/tractus-multiview.yaml:40](specs/tractus-multiview.yaml#L40))
- [ ] Check which layout codes /viewer/{viewer}/layout takes (the default list gives 1L12SV for "1 Large + 12 Small", possibly a typo) and whether custom layouts' codes work. ([specs/tractus-multiview.yaml:176](specs/tractus-multiview.yaml#L176))
- [ ] Check that /viewer/{viewer}/ndi/ takes true and false (the documentation says "likely"). ([specs/tractus-multiview.yaml:217](specs/tractus-multiview.yaml#L217))
- [ ] Record what a preset bank identifier is (a number, a name) and its range. ([specs/tractus-multiview.yaml:230](specs/tractus-multiview.yaml#L230))
- [ ] Record the RGBA format caption backgrounds take, and whether caption positions take decimals. ([specs/tractus-multiview.yaml:386](specs/tractus-multiview.yaml#L386))
- [ ] Record the caption, layout import and layout template bodies from the Swagger reference on a running instance. ([specs/tractus-multiview.yaml:454](specs/tractus-multiview.yaml#L454))

## tractus-router — Tractus Router for NDI

- [ ] Record the router's HTTP port, and whether its routes take GET. ([specs/tractus-router.yaml:28](specs/tractus-router.yaml#L28))
- [ ] Record the answers of /slots and /sources to keep the slots field by field, and the answer to a slot or source that does not exist. ([specs/tractus-router.yaml:136](specs/tractus-router.yaml#L136))
- [ ] Check that a slot or source name holding a slash or a percent sign reaches the router intact. ([specs/tractus-router.yaml:56](specs/tractus-router.yaml#L56))

## tsl-umd-display — TSL UMD tally sent to displays and multiviewers

- [ ] Check that a V4.0 display or multiviewer with a right display lights the right display's tallies from the second XDATA byte (right_lh, right_text_tally, right_rh), and that one without ignores it. ([specs/tsl-umd-display.yaml:76](specs/tsl-umd-display.yaml#L76))

## tsl-umd-listener — TSL UMD tally received from a switcher

- [ ] On a Ross Carbonite sending TSLUMD_1.0 over TCP, confirm the core receives each 18-byte V3.1 packet back to back with nothing between them, and record the tally and text for program, preview and a key. ([specs/tsl-umd-listener.yaml:46](specs/tsl-umd-listener.yaml#L46))
- [ ] On a Carbonite, confirm tally 1 is preview and tally 2 program on the TSL feed, and what ShowUMDId and ShowBusName add to the text. ([specs/tsl-umd-listener.yaml:94](specs/tsl-umd-listener.yaml#L94))

## turtleav-amp150 — Turtle AV 150W Dante amplifier

- [ ] Check the master volume and mute reply texts (assumed Master volume: 50 and Master mute on), the line ending, and that port 8000 needs no login. ([specs/turtleav-amp150.yaml:682](specs/turtleav-amp150.yaml#L682))
- [ ] Record the line and Dante wordings of the input and output replies (the document shows only Input line volume: 50 and Output speaker volume: 50), and whether they are capitalised. ([specs/turtleav-amp150.yaml:692](specs/turtleav-amp150.yaml#L692))
- [ ] Check the LCD and ID LED timed forms: s lcd on 15 (this document's example) or s lcd 15 (the 30W amplifier's), and the replies to r lcd and r idled. ([specs/turtleav-amp150.yaml:72](specs/turtleav-amp150.yaml#L72))
- [ ] Check the TCP port command's spelling: s tcp/ipport 8000 (the example, sent) or s tcp/ip port 8000 (the command column). ([specs/turtleav-amp150.yaml:346](specs/turtleav-amp150.yaml#L346))
- [ ] Record r fan's reply when the two fans differ, r auto stb's reply with auto standby off, and r trigger's with the trigger off. ([specs/turtleav-amp150.yaml:682](specs/turtleav-amp150.yaml#L682))
- [ ] Check what a graphic EQ value means: 0-20 with 10 the default, read here as the device's number, possibly -10 to +10 dB. ([specs/turtleav-amp150.yaml:243](specs/turtleav-amp150.yaml#L243))
- [ ] Check how many lines a network change answers (the value and the net reboot reminder) and that later reads stay in step. ([specs/turtleav-amp150.yaml:692](specs/turtleav-amp150.yaml#L692))

## turtleav-avoip-control — Turtle AV DARWIN and CHAZY controllers

- [ ] Check the line ending (CR LF sent), whether Telnet on port 23 negotiates options, prints a banner or asks for a login, and whether it echoes commands. ([specs/turtleav-avoip-control.yaml:676](specs/turtleav-avoip-control.yaml#L676))
- [ ] Check CHAZY Control Pro's acknowledgement texts: whether they start with [SUCCESS] and [ERROR]. ([specs/turtleav-avoip-control.yaml:686](specs/turtleav-avoip-control.yaml#L686))
- [ ] Check whether the status blocks between = lines end in a way a client can find, so state could be read rather than kept from replies. ([specs/turtleav-avoip-control.yaml:665](specs/turtleav-avoip-control.yaml#L665))
- [ ] Capture GET STATUS on a DARWIN Control and a CHAZY Control and confirm the block opens and ends with lines of = signs as the documents' examples show (DARWIN with two opening lines), with nothing after the closing line, so it frames as one message. ([specs/turtleav-avoip-control.yaml:59](specs/turtleav-avoip-control.yaml#L59))
- [ ] Confirm the status tables' columns are separated by spaces as the examples print them (the encoder and decoder rows read here: In/ENC, EDID, IP, NET/Sig and Out/DEC, FromIn/From, IP, NET/HDMI, Res, Mode, Osp), that EDID names always start with a letter, and that the IP addresses are zero-padded with no spaces; the columns are parsed from the documents' examples only. ([specs/turtleav-avoip-control.yaml:570](specs/turtleav-avoip-control.yaml#L570))
- [ ] Confirm reading GET STATUS every 10 seconds is acceptable on a controller with many encoders and decoders, and record how long the block takes to arrive. ([specs/turtleav-avoip-control.yaml:552](specs/turtleav-avoip-control.yaml#L552))

## turtleav-bt-wallplate — Turtle AV Dante Bluetooth wall plate

- [ ] Check the line ending, which port carries the commands (8000 or 23), and the get bt mute reply text. ([specs/turtleav-bt-wallplate.yaml:479](specs/turtleav-bt-wallplate.yaml#L479))
- [ ] Check that get bt name answers with the name alone, and that set bt name takes the name in angle brackets as the example shows. ([specs/turtleav-bt-wallplate.yaml:489](specs/turtleav-bt-wallplate.yaml#L489))
- [ ] Record the reply to set 1 eq 1 typ ...: whether it is one line naming the channel and band (read into state) or several. ([specs/turtleav-bt-wallplate.yaml:489](specs/turtleav-bt-wallplate.yaml#L489))
- [ ] Record get 1 eq's reply: one line for the channel (Bluetooth input left EQ: on) or one per band. ([specs/turtleav-bt-wallplate.yaml:501](specs/turtleav-bt-wallplate.yaml#L501))
- [ ] Record get artist, get album and get track with nothing playing, and get bt format with no device connected. ([specs/turtleav-bt-wallplate.yaml:479](specs/turtleav-bt-wallplate.yaml#L479))

## turtleav-dante — Turtle AV Dante bridges, Downtown and 30W amplifier

- [ ] Check the line ending (CR LF sent), that port 8000 is a raw socket with the same commands as Telnet 23, and that no login is asked. ([specs/turtleav-dante.yaml:2550](specs/turtleav-dante.yaml#L2550))
- [ ] Check the real channel ranges and get type strings of Mineola 4x4, 8x8 and 16x16 (their manuals copy the 2x2's), which the reads assume are 4, 8 and 16. ([specs/turtleav-dante.yaml:2576](specs/turtleav-dante.yaml#L2576))
- [ ] Check the reply to an error, and Downtown's auto event report format. ([specs/turtleav-dante.yaml:2592](specs/turtleav-dante.yaml#L2592))
- [ ] Check every model answers get master vol with Master volume: <n> and get power with Power: on or off, since those replies queue the model's other reads. ([specs/turtleav-dante.yaml:2540](specs/turtleav-dante.yaml#L2540))
- [ ] Check set master member takes the digits in angle brackets (set master member <11>, as the examples show) and the reply's form. ([specs/turtleav-dante.yaml:217](specs/turtleav-dante.yaml#L217))
- [ ] Record the reply to set input/output x eq y typ ...: one line (XLR IN1 EQ : Type: 2, ...) as read here, and the 30W amplifier's with the band. ([specs/turtleav-dante.yaml:2567](specs/turtleav-dante.yaml#L2567))
- [ ] Record get input/output x eq on each model: one line for the channel or one per band, and how far the extra lines put later replies out of step. ([specs/turtleav-dante.yaml:2558](specs/turtleav-dante.yaml#L2558))
- [ ] Check the Phoenix's phantom power and input sensitivity commands (in its manual, not on its front panel) and the Phoenix output level range 1-5. ([specs/turtleav-dante.yaml:344](specs/turtleav-dante.yaml#L344))
- [ ] Record the 30W amplifier's LCD, ID LED and trigger replies (its manual is partly scanned; the 150W amplifier's wording is assumed) and whether set lcd 15 or set lcd on 15 is right. ([specs/turtleav-dante.yaml:637](specs/turtleav-dante.yaml#L637))
- [ ] Record the 30W amplifier's ducking reply (the manual shows it with and without with) and get output x from with several sources. ([specs/turtleav-dante.yaml:595](specs/turtleav-dante.yaml#L595))
- [ ] Check Downtown takes 1 and 0 for its master mute, upmixer and virtualizer, and the bare temperature and uptime replies. ([specs/turtleav-dante.yaml:2583](specs/turtleav-dante.yaml#L2583))
- [ ] Check Downtown's TCP port command: set tcp/ip port 8000, or set tcp/ipport 8000 as its example shows. ([specs/turtleav-dante.yaml:1084](specs/turtleav-dante.yaml#L1084))

## turtleav-matrix — Turtle AV 4K60 video wall, matrix and multiviewer

- [ ] Check which port carries the s/r commands (23 or 8000), whether a CR LF after ! is accepted, and the replies' line ending. ([specs/turtleav-matrix.yaml:1238](specs/turtleav-matrix.yaml#L1238))
- [ ] Check r power! and r lock! answer power on/off and panel button lock on/off, since those replies queue the other reads, and the 4x4's error replies. ([specs/turtleav-matrix.yaml:1246](specs/turtleav-matrix.yaml#L1246))
- [ ] Record the lines of r fw version! and r vw info! and check the later reads stay in step after them. ([specs/turtleav-matrix.yaml:1257](specs/turtleav-matrix.yaml#L1257))
- [ ] Record the 4x4 multiviewer layout replies other than single screen, and the r dual/triple/quad/user mode! replies. ([specs/turtleav-matrix.yaml:1277](specs/turtleav-matrix.yaml#L1277))
- [ ] Record r mx preset z!, r vw preset z! and r preset z! replies, which the documents give only as video/audio crosspoint. ([specs/turtleav-matrix.yaml:1266](specs/turtleav-matrix.yaml#L1266))
- [ ] Record the 8x8 mirror reply for each mode and whether the 8x8 answers the 4x4's hmirror and vmirror. ([specs/turtleav-matrix.yaml:1266](specs/turtleav-matrix.yaml#L1266))
- [ ] Check what s reset! keeps: whether the network settings survive a factory reset. ([specs/turtleav-matrix.yaml:1229](specs/turtleav-matrix.yaml#L1229))

## tvone-coriomaster — tvONE CORIOmaster

- [ ] Record how a failed command answers (an "!Error" line is assumed) and what a wrong login answers. ([specs/tvone-coriomaster.yaml:1027](specs/tvone-coriomaster.yaml#L1027))
- [ ] Confirm that the unit greets a connection with a caret prompt that needs no reply, and that login() before any other command is enough. ([specs/tvone-coriomaster.yaml:60](specs/tvone-coriomaster.yaml#L60))
- [ ] Confirm that every command, including methods and StartBatch/EndBatch, ends with exactly one "!Done" line after its value lines. ([specs/tvone-coriomaster.yaml:46](specs/tvone-coriomaster.yaml#L46))
- [ ] Record the exact form of the WINDOW, PRESET, STBD and CANVAS event lines (spaces after the commas vary in the reference). ([specs/tvone-coriomaster.yaml:816](specs/tvone-coriomaster.yaml#L816))
- [ ] Check what happens when a second client (CORIOgrapher) connects while this one is connected. ([specs/tvone-coriomaster.yaml:1019](specs/tvone-coriomaster.yaml#L1019))
- [ ] Confirm that On and Off are accepted for HFlip, VFlip, SCFTB and the shrink animations, and whether Yes and No are also accepted. ([specs/tvone-coriomaster.yaml:264](specs/tvone-coriomaster.yaml#L264))
- [ ] Confirm the Windows, Canvases, Layouts and Stbds lists answer one "Windows.Window<n> = <...>" line per entry, and record whether windows that are not in use (FREE) are listed too. ([specs/tvone-coriomaster.yaml:807](specs/tvone-coriomaster.yaml#L807))
- [ ] Confirm a preset take or storyboard changes window properties without echoing them, and that PRESET COMPLETE follows every take (the windows are read again on it). ([specs/tvone-coriomaster.yaml:820](specs/tvone-coriomaster.yaml#L820))
- [ ] Record the CANVAS PROPERTY_CHANGED value forms (AudioFollowWindow as Window1 or 1) and whether STBDCURRENT_CHANGED ends with a comma. ([specs/tvone-coriomaster.yaml:835](specs/tvone-coriomaster.yaml#L835))
- [ ] Confirm a set echoes "<path> = <value>" before "!Done" for every window, canvas and layout property, so state follows this connection's commands without a read. ([specs/tvone-coriomaster.yaml:1066](specs/tvone-coriomaster.yaml#L1066))
- [ ] Record the values Layout<n>.Mode takes (Normal shown, not documented). ([specs/tvone-coriomaster.yaml:940](specs/tvone-coriomaster.yaml#L940))

## twitch — Twitch

- [ ] Confirm a public client's refresh at https://id.twitch.tv/oauth2/token with client_id alone succeeds, answers a new refresh token every time, and that the old one then answers 400 "Invalid refresh token". ([specs/twitch.yaml:149](specs/twitch.yaml#L149))
- [ ] Record the body Helix answers with a 401 (expired token, revoked token, and a Client-Id that does not match the token), confirm an expired or revoked token's message is "Invalid OAuth token" (the only 401 taken as a refusal) and a missing scope's is different, and confirm 403 is never used for a refused token. ([specs/twitch.yaml:163](specs/twitch.yaml#L163))
- [ ] Confirm every Helix request is refused without the Client-Id header, and accepted with it beside the bearer token. ([specs/twitch.yaml:157](specs/twitch.yaml#L157))
- [ ] Confirm GET /helix/users with no parameters answers the token's own user and costs one rate-limit point. ([specs/twitch.yaml:167](specs/twitch.yaml#L167))
- [ ] Confirm Get Streams answers an empty data list while the channel is offline, and record how soon after the encoder starts and stops it changes. ([specs/twitch.yaml:1577](specs/twitch.yaml#L1577))
- [ ] Record how far Get Streams' viewer_count lags the viewers Twitch shows. ([specs/twitch.yaml:1569](specs/twitch.yaml#L1569))
- [ ] Confirm Update Chat Settings answers 200 with the new settings (the reference also implies 204). ([specs/twitch.yaml:443](specs/twitch.yaml#L443))
- [ ] Confirm Get Chat Settings returns the non-moderator chat delay to the broadcaster's own token with moderator_id set. ([specs/twitch.yaml:436](specs/twitch.yaml#L436))
- [ ] Confirm the non-moderator chat delay duration is accepted as a JSON number (2, 4 or 6). ([specs/twitch.yaml:531](specs/twitch.yaml#L531))
- [ ] Confirm the follower count's total is returned with a token lacking moderator:read:followers. ([specs/twitch.yaml:342](specs/twitch.yaml#L342))
- [ ] Confirm a stream marker with an empty description is accepted. ([specs/twitch.yaml:362](specs/twitch.yaml#L362))
- [ ] Confirm Create Clip answers 202 with the clip's id and edit URL while live, and 404 while offline. ([specs/twitch.yaml:385](specs/twitch.yaml#L385))
- [ ] Confirm Start a raid answers 200 with created_at, and Cancel a raid 204. ([specs/twitch.yaml:418](specs/twitch.yaml#L418))
- [ ] Record what Twitch answers for a delay change on a channel that is not a Partner (400 expected). ([specs/twitch.yaml:307](specs/twitch.yaml#L307))
- [ ] Confirm tags are refused with a 400 when one has a space or more than 25 characters, and that [] removes them all. ([specs/twitch.yaml:285](specs/twitch.yaml#L285))
- [ ] Record the Ratelimit-Limit Twitch gives a user token (800 points a minute expected), so the 30-second poll is known to be well within it. ([specs/twitch.yaml:1523](specs/twitch.yaml#L1523))

- [ ] Confirm https://id.twitch.tv/oauth2/validate answers 200 to "Authorization: OAuth <token>" without a Client-Id, and 401 with message "invalid access token" once the user disconnects the application or the token is revoked. ([specs/twitch.yaml:154](specs/twitch.yaml#L154))
- [ ] Confirm validating when the integration opens, after each refresh and hourly satisfies Twitch's audit, and record whether validation counts against the Helix rate limit. ([specs/twitch.yaml:154](specs/twitch.yaml#L154))
- [ ] Confirm the nineteen EventSub subscriptions are created (202, or 403 for a missing scope) within the 10 seconds after the welcome when they wait behind the poll, and that the welcome's session id is accepted as sent. ([specs/twitch.yaml:1610](specs/twitch.yaml#L1610))
- [ ] Confirm that not following session_reconnect costs only the events between Twitch's close (4004) and the new session's subscriptions, and that the disabled subscriptions of old sessions do not count against the limit of three websockets or the total cost. ([specs/twitch.yaml:1518](specs/twitch.yaml#L1518))
- [ ] Confirm channel.follow v2 is refused with 403 without moderator:read:followers and accepted with the broadcaster as moderator_user_id. ([specs/twitch.yaml:1682](specs/twitch.yaml#L1682))
- [ ] Confirm a missing scope answers 401 on every new endpoint (polls, predictions, moderation, rewards, schedule), as the reference says, with a message other than "Invalid OAuth token", so it fails only that command; and record whether any answers 403 instead. ([specs/twitch.yaml:1311](specs/twitch.yaml#L1311))
- [ ] Confirm Create Poll accepts the choices array as sent and refuses channel_points_per_vote without channel_points_voting_enabled, and that End Poll answers 200 with the poll. ([specs/twitch.yaml:580](specs/twitch.yaml#L580))
- [ ] Confirm Get Polls and Get Predictions list the newest first, so data[0] is the running or most recent one kept in state. ([specs/twitch.yaml:631](specs/twitch.yaml#L631))
- [ ] Confirm EventSub poll begin, progress and end events carry the votes per choice, that the end status is lowercase (completed, terminated, archived), and that choices keep their order across events. ([specs/twitch.yaml:1735](specs/twitch.yaml#L1735))
- [ ] Confirm lock_prediction, resolve_prediction and cancel_prediction answer 200 with the prediction, and that EventSub prediction colors are lowercase blue and pink. ([specs/twitch.yaml:670](specs/twitch.yaml#L670))
- [ ] Confirm Ban User accepts an empty reason, and that a timeout of 1 second ends a timeout early. ([specs/twitch.yaml:706](specs/twitch.yaml#L706))
- [ ] Confirm Delete Chat Messages with a message id from chat removes that message, and without one clears chat. ([specs/twitch.yaml:771](specs/twitch.yaml#L771))
- [ ] Confirm Update Shield Mode Status answers 200 with the status, and that the shield mode events accept the broadcaster as moderator_user_id. ([specs/twitch.yaml:796](specs/twitch.yaml#L796))
- [ ] Confirm adding and removing a moderator or VIP answers 204, and that the list read right after already reflects the change. ([specs/twitch.yaml:814](specs/twitch.yaml#L814))
- [ ] Confirm Send a Shoutout answers 204, and 400 while offline or without viewers. ([specs/twitch.yaml:894](specs/twitch.yaml#L894))
- [ ] Confirm a reward created by this client id can be changed and one made on Twitch is refused with 403, and record what Get Custom Reward returns for redemptions_redeemed_current_stream and cooldown_expires_at. ([specs/twitch.yaml:918](specs/twitch.yaml#L918))
- [ ] Confirm Get Custom Reward Redemption with status UNFULFILLED lists the queue oldest first, and that EventSub redemption add arrives for rewards of any client. ([specs/twitch.yaml:982](specs/twitch.yaml#L982))
- [ ] Confirm Update Redemption Status answers 200 with the redemptions changed and their new status. ([specs/twitch.yaml:999](specs/twitch.yaml#L999))
- [ ] Confirm Get Channel Stream Schedule answers 404 for a channel with no schedule, and that vacation is null (not absent) when none is set. ([specs/twitch.yaml:1975](specs/twitch.yaml#L1975))
- [ ] Confirm Create Channel Stream Schedule Segment takes duration as a string, and that segment ids hold only base64 characters (letters, digits, +, /, =). ([specs/twitch.yaml:1024](specs/twitch.yaml#L1024))
- [ ] Confirm Update Channel Stream Schedule takes its vacation fields as query parameters, not a body, and answers 204. ([specs/twitch.yaml:1073](specs/twitch.yaml#L1073))
- [ ] Confirm Delete Videos answers 200 with the deleted ids, and what it answers for a video that is not the channel's. ([specs/twitch.yaml:1160](specs/twitch.yaml#L1160))

## unifi-network — UniFi Network (Network API)

- [ ] Check the API on a self-hosted UniFi OS Server and a Cloud Key: port, path prefix and certificate. ([specs/unifi-network.yaml:43](specs/unifi-network.yaml#L43))
- [ ] Check power_cycle_port's off time and that it refuses ports without PoE. ([specs/unifi-network.yaml:1395](specs/unifi-network.yaml#L1395))
- [ ] Check the filter syntax and its percent-encoding as sent in the query (name.eq('x')). ([specs/unifi-network.yaml:257](specs/unifi-network.yaml#L257))
- [ ] Check the polled lists with offset=0&limit=200 on a large site, and the load of reading each device's details and statistics every 15 s. ([specs/unifi-network.yaml:1409](specs/unifi-network.yaml#L1409))

## ups-mib — UPS over SNMP (UPS-MIB, RFC 1628)

- [ ] Walk an Eaton Network-M2 or Network-M3 card and a CyberPower RMCARD and check which UPS-MIB groups each fills. ([specs/ups-mib.yaml:39](specs/ups-mib.yaml#L39))
- [ ] Start a quick battery test with set_test_id and check the card accepts upsTestSpinLock written back with upsTestId, and refuses it when the lock changed. ([specs/ups-mib.yaml:287](specs/ups-mib.yaml#L287))
- [ ] Check that a UPS cancels a counting-down shutdown when -1 is written to upsShutdownAfterDelay. ([specs/ups-mib.yaml:283](specs/ups-mib.yaml#L283))
- [ ] Receive upsTrapOnBattery and upsTrapAlarmEntryAdded from a card, and check the alarm table is replaced as alarms come and go. ([specs/ups-mib.yaml:302](specs/ups-mib.yaml#L302))

## vaddio-roboshot — Vaddio RoboSHOT PTZ cameras (Telnet API)

- [ ] Record the login exchange from connection to the first ">" (the exact login and password prompts, any Telnet option bytes, the "Welcome" line) and what a wrong password produces. ([specs/vaddio-roboshot.yaml:86](specs/vaddio-roboshot.yaml#L86))
- [ ] Confirm that every answer, including the gets and errors, ends with the ">" prompt, and whether the camera echoes the command line before its answer. ([specs/vaddio-roboshot.yaml:80](specs/vaddio-roboshot.yaml#L80))
- [ ] Record the error message for an out-of-range position (pan, tilt, zoom set) and for an unknown command. ([specs/vaddio-roboshot.yaml:886](specs/vaddio-roboshot.yaml#L886))
- [ ] Record the answers to streaming ip enable on and get on a RoboSHOT 40 UHD or SDI camera, to settle whether OK comes after a second prompt ("> OK"). ([specs/vaddio-roboshot.yaml:893](specs/vaddio-roboshot.yaml#L893))
- [ ] Record camera glass mode get on a RoboSHOT IW. ([specs/vaddio-roboshot.yaml:898](specs/vaddio-roboshot.yaml#L898))

## vdcp — VDCP video servers

- [ ] Record the TCP port the server takes VDCP on, and whether it carries the RS-422 bytes unchanged. ([specs/vdcp.yaml:54](specs/vdcp.yaml#L54))
- [ ] Confirm opening and selecting the signal port on connecting (3X.01 with the lock byte, answered 3X.81, then 2X.22), and what a denied open does. ([specs/vdcp.yaml:38](specs/vdcp.yaml#L38))
- [ ] Check the port status's lengths: Status 2 one byte (two extended), Status 3 three bytes (four extended), and that the error bits clear once read. ([specs/vdcp.yaml:476](specs/vdcp.yaml#L476))
- [ ] Confirm a macro's completion return comes in place of a port status and that the controller's ACK (04) is what the server waits for. ([specs/vdcp.yaml:452](specs/vdcp.yaml#L452))
- [ ] Check that asking the position only while the port is active keeps the error bits clear, and the load the 250 ms port status puts on the server. ([specs/vdcp.yaml:445](specs/vdcp.yaml#L445))
- [ ] Check the server's variable-length ID commands (8X, AX, BX, DX) and the length byte before each ID in returns. ([specs/vdcp.yaml:42](specs/vdcp.yaml#L42))
- [ ] Confirm the ID list's first two bytes count the IDs still to come after this message, and that NEXT (3X.02, answered 3X.82) continues it ten at a time. ([specs/vdcp.yaml:344](specs/vdcp.yaml#L344))
- [ ] Check which VDCP commands Ross Tria, Mira and Kiva servers take and on which port. ([specs/vdcp.yaml:72](specs/vdcp.yaml#L72))

## vdoninja — VDO.Ninja

- [ ] Confirm the relay accepts {"join": key} with nothing answered, and how long an idle websocket lasts (a 30 s getDetails keep-alive is assumed to hold it). ([specs/vdoninja.yaml:90](specs/vdoninja.yaml#L90))
- [ ] Confirm every websocket answer is {"callback": {action, target, value, result}} with the request's action, as the Companion module reads it (the README shows it only as a screenshot). ([specs/vdoninja.yaml:859](specs/vdoninja.yaml#L859))
- [ ] Record getDetails' answer: whether result is an array or an object, and its fields (streamID, label, director, position, localStream, muted, videoMuted, speakerMuted, scenes, others mute-guest and hide-guest). ([specs/vdoninja.yaml:924](specs/vdoninja.yaml#L924))
- [ ] Record getGuestList's answer shape (slots keyed "1", "2" with streamID and label assumed). ([specs/vdoninja.yaml:957](specs/vdoninja.yaml#L957))
- [ ] Confirm values sent as strings ("true", "false", "toggle") are taken like booleans, and that activeSpeaker off is false. ([specs/vdoninja.yaml:104](specs/vdoninja.yaml#L104))
- [ ] Confirm videoConstraint takes the constraint's name as target and its value in value2, and requestAspectRatio the largest dimension in value2. ([specs/vdoninja.yaml:416](specs/vdoninja.yaml#L416))
- [ ] Record which update actions a page pushes (mic, camera, remoteMuted, remoteVideoMuted, directorMuted, directorVideoHide, hangup, endViewConnection, newViewConnection, positionChange, details) and their value types. ([specs/vdoninja.yaml:969](specs/vdoninja.yaml#L969))

## ventuz-director — Ventuz Director (Director Remoting)

- [ ] Confirm Director Remoting's websocket answers on port 22404 (the documentation's port; the vendor's Companion module defaults to 21404). ([specs/ventuz-director.yaml:50](specs/ventuz-director.yaml#L50))
- [ ] Confirm a numeric RequestID is echoed as sent (the documentation writes it as an integer in quotes) and that each reply carries it. ([specs/ventuz-director.yaml:618](specs/ventuz-director.yaml#L618))
- [ ] Check whether channel and playlist item indexes count from 0 or from 1, and what show.cue and show.take do without a ChannelIndex. ([specs/ventuz-director.yaml:620](specs/ventuz-director.yaml#L620))
- [ ] Check the shape of show.getchannels, macro.get and playlist.get results (arrays of ChannelIndex/Name, Id/Name and ChannelIndex/ChannelName/Items) and of remoteplaylist.get's PublishContexts. ([specs/ventuz-director.yaml:707](specs/ventuz-director.yaml#L707))
- [ ] Confirm show.setprojectdata takes a boolean's ValueToSet as the text true or false, and a relative number as text. ([specs/ventuz-director.yaml:382](specs/ventuz-director.yaml#L382))
- [ ] Check that the can_* commands return Code 0 when the action is possible and another code (5, ErrorInvalidOperation) when not. ([specs/ventuz-director.yaml:286](specs/ventuz-director.yaml#L286))

## videolan-vlc — VLC media player

- [ ] Confirm the .json pages take the same commands as the .xml ones, the status and playlist field names read here, and the rate and audiodelay commands. ([specs/videolan-vlc.yaml:315](specs/videolan-vlc.yaml#L315))
- [ ] Check that the playlist tree's first child is the Playlist (not the Media Library) in a localized VLC, and how nested folders appear. ([specs/videolan-vlc.yaml:332](specs/videolan-vlc.yaml#L332))
- [ ] Check the largest volume value VLC accepts on the web interface. ([specs/videolan-vlc.yaml:325](specs/videolan-vlc.yaml#L325))

## vimeo-live — Vimeo Live

- [ ] Confirm a refused or revoked token answers 401 with error_code 8000 or 8003 on the live endpoints, and that error 3200 under 401 is only a permission refusal. ([specs/vimeo-live.yaml:91](specs/vimeo-live.yaml#L91))
- [ ] Confirm the live endpoints accept Accept application/vnd.vimeo.*+json;version=3.4 and JSON request bodies with Content-Type application/json. ([specs/vimeo-live.yaml:86](specs/vimeo-live.yaml#L86))
- [ ] Confirm GET /me?fields=uri as the idle probe, and record the X-RateLimit figures against the 30-second poll of three requests. ([specs/vimeo-live.yaml:94](specs/vimeo-live.yaml#L94))
- [ ] Confirm GET /me/live_events answers a page object with data (the reference shows an array), and that type, sort, direction, per_page and page are honoured. ([specs/vimeo-live.yaml:127](specs/vimeo-live.yaml#L127))
- [ ] Confirm GET /me/live_events/{id}/destinations answers a page object with data or a plain array, as the two rules expect. ([specs/vimeo-live.yaml:667](specs/vimeo-live.yaml#L667))
- [ ] Confirm fields= filtering works on GET /me/live_events/{id} for stream_key, backup_stream_key, rtmp_link, rtmps_link, srt_link and srt_passphrase. ([specs/vimeo-live.yaml:154](specs/vimeo-live.yaml#L154))
- [ ] Confirm Create an event answers 200 (not 201) with the event, its uri /live_events/{id}. ([specs/vimeo-live.yaml:196](specs/vimeo-live.yaml#L196))
- [ ] Confirm Activate an event accepts an empty JSON body, answers 200, and what it answers once already activated (400 error 2428). ([specs/vimeo-live.yaml:373](specs/vimeo-live.yaml#L373))
- [ ] Confirm End an event works without clip_id and answers 200, and record what viewers see. ([specs/vimeo-live.yaml:380](specs/vimeo-live.yaml#L380))
- [ ] Confirm GET /live_events/{id}/session_status (no /me alias) answers while no session exists, and record ingest.status as a number or a string. ([specs/vimeo-live.yaml:397](specs/vimeo-live.yaml#L397))
- [ ] Confirm the low_latency PATCH answers {"lowLatency": ...} and whether it is the same setting as latency low. ([specs/vimeo-live.yaml:312](specs/vimeo-live.yaml#L312))
- [ ] Confirm the Update an event body's auto_cc_language takes de-DE, en-US, es-ES, fr-FR and pt-BR (the auto_cc endpoint lists other codes). ([specs/vimeo-live.yaml:348](specs/vimeo-live.yaml#L348))
- [ ] Confirm set_stream_title's automatically_title_stream false with stream_title is accepted in one PATCH. ([specs/vimeo-live.yaml:264](specs/vimeo-live.yaml#L264))
- [ ] Confirm PATCH /destination/{id} with is_enabled alone toggles a simulcast destination while live. ([specs/vimeo-live.yaml:466](specs/vimeo-live.yaml#L466))
- [ ] Record what Get an M3U8 playback URL answers (the reference gives no schema). ([specs/vimeo-live.yaml:401](specs/vimeo-live.yaml#L401))
- [ ] Record which ingest.status values occur in practice through a stream's life (0 to 5 documented). ([specs/vimeo-live.yaml:579](specs/vimeo-live.yaml#L579))
- [ ] Record whether GET /me/live_events/{id}/destinations answers a bare array or a page {total, page, per_page, data}: a bare array, or a first page of 25 whose total fits on it, replaces the event's destinations. ([specs/vimeo-live.yaml:665](specs/vimeo-live.yaml#L665))

## vioso-exaplay — VIOSO EXAPLAY

- [ ] Confirm what EXAPLAY answers to a command it refuses or does not know (the documentation gives no error reply). ([specs/vioso-exaplay.yaml:118](specs/vioso-exaplay.yaml#L118))
- [ ] Confirm hello's reply text (documented as hallo) and that get cue answers without the argument the documentation's example shows (get cue 012345). ([specs/vioso-exaplay.yaml:51](specs/vioso-exaplay.yaml#L51))
- [ ] Check whether the composition-addressed commands of later versions (get:status,comp1 and the like on port 8100) replace the documented ones, and on which versions each works. ([specs/vioso-exaplay.yaml:124](specs/vioso-exaplay.yaml#L124))
- [ ] Confirm get cue's fields and their order (status, cue, frame, time, duration, fps) and the volume range 0-100. ([specs/vioso-exaplay.yaml:144](specs/vioso-exaplay.yaml#L144))

## visca — VISCA over IP

- [ ] On Sony VISCA over IP, confirm the sequence number the camera expects after RESET (the core sends 1). ([specs/visca.yaml:940](specs/visca.yaml#L940))
- [ ] On PTZOptics, check whether UDP 1259 expects Sony's 8-byte header. ([specs/visca.yaml:987](specs/visca.yaml#L987))
- [ ] On PTZOptics, confirm the shutter, gain, bright, focus mode and focus position codes the core uses (0A/4A, 4C, 4D, 04 38, 04 48) against the 2026 list's conflicting codes. ([specs/visca.yaml:994](specs/visca.yaml#L994))
- [ ] On SRG-X400, check which pan direction is positive. ([specs/visca.yaml:981](specs/visca.yaml#L981))
- [ ] On BirdDog, confirm 32-bit sequence numbers are accepted, and the P400 white balance codes (04 ATW, 05/06 manual). ([specs/visca.yaml:1007](specs/visca.yaml#L1007))
- [ ] On Lumens and Marshall, confirm preset 1 is wire 00. ([specs/visca.yaml:1013](specs/visca.yaml#L1013))
- [ ] On AVer, confirm presets are numbered from 0 on the wire (preset n is wire n-1). ([specs/visca.yaml:1020](specs/visca.yaml#L1020))
- [ ] On Canon, confirm replies go to UDP 52381 with the default response port setting. ([specs/visca.yaml:1032](specs/visca.yaml#L1032))
- [ ] On Datavideo, find the IP framing and port, which the document does not give (52381 with Sony framing assumed). ([specs/visca.yaml:1040](specs/visca.yaml#L1040))
- [ ] On a serial camera (Sony, Datavideo, Lumens, Marshall), confirm replies come from y0 with y = address + 8 and that only Address Set and IF_Clear broadcasts answer from 88. ([specs/visca.yaml:1126](specs/visca.yaml#L1126))
- [ ] Confirm a camera answers commands right after Address Set and IF_Clear broadcast on opening, and whether one that already has its address needs them. ([specs/visca.yaml:130](specs/visca.yaml#L130))
- [ ] On Sony, confirm the 200 ms before a cancel counts from the pan/tilt drive's ACK (the core's reading of "after executing"); on the other vendors' cameras, which document no such gap and get none, check whether a cancel sent within 200 ms of a drive starting still stops it. ([specs/visca.yaml:1194](specs/visca.yaml#L1194))

## visualproductions-cuecore — Visual Productions CueCore, QuadCore, B-Station2, IoCore2, TimeCore and KioscTouch2 (UDP API)

- [ ] Record the UDP feedback messages with their values (playback intensity, rate, active, cue; master intensity, rate, fade; action list enable; buttons), and check the "=value" form and the on/off words. ([specs/visualproductions-cuecore.yaml:499](specs/visualproductions-cuecore.yaml#L499))
- [ ] Confirm hello makes the session a feedback client and that the unit echoes "<prefix>-hello". ([specs/visualproductions-cuecore.yaml:490](specs/visualproductions-cuecore.yaml#L490))
- [ ] Check the cue range (1-32 per the HTTP tables), the DMX channel range on units other than the IoCore2, and the float precision accepted. ([specs/visualproductions-cuecore.yaml:516](specs/visualproductions-cuecore.yaml#L516))
- [ ] On a B-Station2, check the button and LED numbering (1-8 assumed). ([specs/visualproductions-cuecore.yaml:536](specs/visualproductions-cuecore.yaml#L536))
- [ ] On a KioscTouch2, check whether page select counts from 0 or 1. ([specs/visualproductions-cuecore.yaml:542](specs/visualproductions-cuecore.yaml#L542))

## vizrt-trio — Vizrt Viz Trio

- [ ] Record what Viz Trio sends back for a command with no result (nothing, an empty line, or an acknowledgement); the spec waits for nothing. ([specs/vizrt-trio.yaml:939](specs/vizrt-trio.yaml#L939))
- [ ] Record what a read answers when its result is empty (page:getpagename with no page read, playlist:get_current with none): an empty line, or nothing. ([specs/vizrt-trio.yaml:959](specs/vizrt-trio.yaml#L959))
- [ ] Confirm a command ended by a line feed alone is accepted (the documents say a new line; some clients send CR LF). ([specs/vizrt-trio.yaml:54](specs/vizrt-trio.yaml#L54))
- [ ] Check how boolean arguments are written (true/false as sent here, or 1/0) for page:fire, page:fire_all and goto_manual_graphics_mode. ([specs/vizrt-trio.yaml:270](specs/vizrt-trio.yaml#L270))
- [ ] Check how arguments holding spaces are quoted when they are not the last one (channel and page names). ([specs/vizrt-trio.yaml:949](specs/vizrt-trio.yaml#L949))
- [ ] Record how show:get_pages and show:get_templates separate their names (spaces, commas or line breaks) and what they answer for an empty show. ([specs/vizrt-trio.yaml:969](specs/vizrt-trio.yaml#L969))

## vmix — vMix

- [ ] Capture a full XML state from vMix 27 or later (with a replay, a GT title, a list, a video call and layered inputs) so the parsing can be checked against the real shape; the shapes used here are inferred from vMix's old documented example and the MIT-licensed Companion module. ([specs/vmix.yaml:87](specs/vmix.yaml#L87))
- [ ] Confirm that the mix elements of the XML are numbered from 2 (the first extra mix), one more than the Mix command parameter. ([specs/vmix.yaml:5111](specs/vmix.yaml#L5111))
- [ ] Confirm that the XML lists overlays 1 to 8 as the overlay channels and 9 to 16 as Stinger 1 to 8 (taken from the Companion module; vMix's own XML example predates eight channels and shows six), and what the preview attribute of an overlay means. ([specs/vmix.yaml:5174](specs/vmix.yaml#L5174))
- [ ] Confirm that SetText, SetImage, SetColor and the other title field functions change the first field when neither SelectedName nor SelectedIndex is given, and that SelectedIndex counts text, image and colour fields in one sequence from 0, as the XML's index attribute does. ([specs/vmix.yaml:5126](specs/vmix.yaml#L5126))
- [ ] Confirm that SetTextColour and the countdown functions (StartCountdown, StopCountdown, PauseCountdown, SuspendCountdown, SetCountdown, ChangeCountdown, AdjustCountdown) take SelectedName or SelectedIndex, which the function reference does not say and the Companion module sends. ([specs/vmix.yaml:1434](specs/vmix.yaml#L1434))
- [ ] Confirm the units of the recording element's duration attribute (seconds assumed) and the names of its file attributes (filename1, filename2). ([specs/vmix.yaml:5187](specs/vmix.yaml#L5187))
- [ ] Confirm that the streaming element carries one channelN attribute per stream, numbered from 1. ([specs/vmix.yaml:5190](specs/vmix.yaml#L5190))
- [ ] Confirm the outputs element (type, number, source, inputNumber, mix, ndi, omt, srt) and which vMix version first reports it. ([specs/vmix.yaml:5178](specs/vmix.yaml#L5178))
- [ ] Confirm that the InputMixN and InputPreviewMixN activators number mixes as the XML does (2 to 16). ([specs/vmix.yaml:5112](specs/vmix.yaml#L5112))
- [ ] Confirm that activator volumes (InputVolume, MasterVolume, MasterHeadphones, BusXVolume, InputVolumeChannelMixerN) are the 0 to 100 volume divided by 100, so that multiplying by 100 matches the XML. ([specs/vmix.yaml:5153](specs/vmix.yaml#L5153))
- [ ] Confirm that ACTS InputAudioAuto N and ACTS InputVolumeChannelMixerN N answer with the activator's current value for an input with audio, and what they answer for an input without. ([specs/vmix.yaml:5151](specs/vmix.yaml#L5151))
- [ ] Confirm that meterF1 and meterF2 are the first and second channel's levels, linear from 0 (not dB). ([specs/vmix.yaml:5154](specs/vmix.yaml#L5154))
- [ ] Confirm the audiobusses attribute's form (letters M and A to G, comma-separated). ([specs/vmix.yaml:5152](specs/vmix.yaml#L5152))
- [ ] Confirm which attributes the audio element's master and bus elements carry (volume, muted, meterF1, meterF2, headphonesVolume, solo, sendToMaster assumed). ([specs/vmix.yaml:5158](specs/vmix.yaml#L5158))
- [ ] Confirm the input attributes shortTitle, markIn, markOut (ms), selectedIndex and frameDelay (frames), and that position and duration are milliseconds. ([specs/vmix.yaml:5117](specs/vmix.yaml#L5117))
- [ ] Confirm that GT image and color fields are reported as image and color elements with index and name, and whether their indexes share one sequence with the text fields. ([specs/vmix.yaml:5128](specs/vmix.yaml#L5128))
- [ ] Confirm that a list input reports its items as list/item elements in order, with selected="true" on the selected one. ([specs/vmix.yaml:5132](specs/vmix.yaml#L5132))
- [ ] Confirm that layers are overlay elements with a 0-based index (layer number less 1) and the layer input's key, with position (panX, panY, zoomX, zoomY, x, y, width, height) and crop (X1, Y1, X2, Y2) children, and whether a layer turned off (LayerOff) is still listed. ([specs/vmix.yaml:5134](specs/vmix.yaml#L5134))
- [ ] Confirm the input's own position, crop and cc (colour correction) elements and their attribute names, and which vMix version first reports them. ([specs/vmix.yaml:5140](specs/vmix.yaml#L5140))
- [ ] Confirm the replay element's attributes and timecode children, and what events (the selected event list) holds. ([specs/vmix.yaml:5204](specs/vmix.yaml#L5204))
- [ ] Confirm the replay activators: ReplayPlaying, ReplayLive, ReplayRecording, ReplayQuadMode, ReplayPlayForward, ReplayPlayBackward, ReplayChannelAB/A/B, ReplayCameraN (current channel), ReplayACameraN and ReplayBCameraN. ([specs/vmix.yaml:5208](specs/vmix.yaml#L5208))
- [ ] Confirm the dynamic element's input1 to input4 and value1 to value4 children. ([specs/vmix.yaml:5196](specs/vmix.yaml#L5196))
- [ ] Confirm the video call attributes (callPassword, callConnected, callVideoSource, callAudioSource) and the VideoCallAudioSourceX and VideoCallSourceOutputN activators. ([specs/vmix.yaml:5141](specs/vmix.yaml#L5141))

## wmspanel — WMSPanel

- [ ] Confirm wrong credentials and a calling host missing from the API whitelist both answer 403, and record the body. ([specs/wmspanel.yaml:2477](specs/wmspanel.yaml#L2477))
- [ ] Confirm create and update calls take the request data fields as a JSON body with Content-Type application/json, for every resource (the reference shows it for some). ([specs/wmspanel.yaml:2491](specs/wmspanel.yaml#L2491))
- [ ] Confirm the single get calls' answer shapes: stream (an object) for a live stream, rule (an array in the reference's example) for a re-publish rule. ([specs/wmspanel.yaml:2611](specs/wmspanel.yaml#L2611))
- [ ] Confirm the action calls (pause, resume, restart, start, stop, clone) answer {"status": "Ok"}, and record how soon the lists show the change. ([specs/wmspanel.yaml:2683](specs/wmspanel.yaml#L2683))
- [ ] Record how WMSPanel counts the probe and poll calls against the 15,000-call daily limit, and what it answers once the limit is reached. ([specs/wmspanel.yaml:2465](specs/wmspanel.yaml#L2465))
- [ ] Confirm list_servers lists every server kind, and that the server answer's status is online, offline or pending. ([specs/wmspanel.yaml:2519](specs/wmspanel.yaml#L2519))

## workpro-lightshark — Work Pro LightShark LS-1 / LS-Core (OSC)

- [ ] Check the default OSC input and output ports (8000 and 9000 per Companion) and where the console sends its output. ([specs/workpro-lightshark.yaml:265](specs/workpro-lightshark.yaml#L265))
- [ ] Record the console's feedback after /LS/Sync (grand master, master levels, playback levels, executors) and the executor value format. ([specs/workpro-lightshark.yaml:272](specs/workpro-lightshark.yaml#L272))
- [ ] Check that a release-only message (0.0) triggers keys, playbacks and executors as the manual's hexadecimal table shows. ([specs/workpro-lightshark.yaml:279](specs/workpro-lightshark.yaml#L279))
- [ ] Check /LS/Color against /LS/Colour, /LS/Fx, the /LS/ExecutorLine form, and the Companion-only addresses (Tap, FxTap, TAP/PB, ChaseValue/PB, Level/Sm*, Reset/Sm*). ([specs/workpro-lightshark.yaml:286](specs/workpro-lightshark.yaml#L286))

## wowza-streaming-engine — Wowza Streaming Engine

- [ ] Confirm every GET answers JSON with Accept: application/json (the API answers XML otherwise), and that writes take JSON bodies. ([specs/wowza-streaming-engine.yaml:63](specs/wowza-streaming-engine.yaml#L63))
- [ ] Confirm the default digest authentication is answered, that basic and none work when Server.xml sets them, and that a wrong credential answers 401. ([specs/wowza-streaming-engine.yaml:59](specs/wowza-streaming-engine.yaml#L59))
- [ ] Confirm an update body holding only some fields leaves the others as they were, for the configuration commands (update_app, update_transcoder, update_push_publish_map_entry and the like). ([specs/wowza-streaming-engine.yaml:3405](specs/wowza-streaming-engine.yaml#L3405))
- [ ] Confirm GET .../instances lists every instance with incomingStreams, recorders and streamGroups in the shapes the reference gives (its OutgoingStreamConfig shows placeholder fields only). ([specs/wowza-streaming-engine.yaml:3692](specs/wowza-streaming-engine.yaml#L3692))
- [ ] Confirm start_recording's body (recorderName, instanceName, defaultRecorder true) starts a recording with the application's recorder defaults. ([specs/wowza-streaming-engine.yaml:3363](specs/wowza-streaming-engine.yaml#L3363))
- [ ] Confirm write operations answer {"success": true, ...}, which the stream target and recorder re-reads rely on. ([specs/wowza-streaming-engine.yaml:3789](specs/wowza-streaming-engine.yaml#L3789))
- [ ] Confirm per-stream monitoring (incomingstreams/{stream}/monitoring/current) answers for live streams, including names with spaces, and what it answers for a stream not publishing. ([specs/wowza-streaming-engine.yaml:3761](specs/wowza-streaming-engine.yaml#L3761))
- [ ] Record the stream recorder's currentDuration unit (ms assumed) and recordingStartTime format, and the sessionStatus values of stream targets. ([specs/wowza-streaming-engine.yaml:3539](specs/wowza-streaming-engine.yaml#L3539))
- [ ] Confirm the media caster type is rtp-record (the reference lists it with a leading space). ([specs/wowza-streaming-engine.yaml:3420](specs/wowza-streaming-engine.yaml#L3420))
- [ ] Record the action names the DVR store and media cache store actions take, which the reference leaves open. ([specs/wowza-streaming-engine.yaml:1273](specs/wowza-streaming-engine.yaml#L1273))
- [ ] Confirm startupstreams answers its list under Streams (capitalized, as the reference gives it). ([specs/wowza-streaming-engine.yaml:3802](specs/wowza-streaming-engine.yaml#L3802))

## wyrestorm-matrix — WyreStorm matrix switchers

- [ ] Confirm SET CEC_PWR takes a space before the port on H2A and MXV matrices (the syntax line has none, the example one). ([specs/wyrestorm-matrix.yaml:497](specs/wyrestorm-matrix.yaml#L497))
- [ ] Record the answer to GET MP: MP GET hdmiinN outN (syntax line) or MP hdmiinN outN (example), and hdmiout or out on H2A units. ([specs/wyrestorm-matrix.yaml:209](specs/wyrestorm-matrix.yaml#L209))
- [ ] Record what GET MP all answers (assumed a line per output). ([specs/wyrestorm-matrix.yaml:884](specs/wyrestorm-matrix.yaml#L884))
- [ ] Confirm the AUTOCEC_FN and AUTOCEC_D port names (out1 in the examples, hdmiout1 and hdbtout1 in the syntax lines). ([specs/wyrestorm-matrix.yaml:517](specs/wyrestorm-matrix.yaml#L517))
- [ ] Confirm GET AUDIOMP takes the audio output alone on H2A and MXV units (its syntax line also names an input). ([specs/wyrestorm-matrix.yaml:273](specs/wyrestorm-matrix.yaml#L273))
- [ ] Confirm HDCP_S takes in5 or 5 (syntax line and example differ) and record its answer. ([specs/wyrestorm-matrix.yaml:552](specs/wyrestorm-matrix.yaml#L552))
- [ ] Record what a refused command or a parameter a model lacks answers, if anything (WyreStorm warns it can lock the unit up). ([specs/wyrestorm-matrix.yaml:15](specs/wyrestorm-matrix.yaml#L15))
- [ ] Confirm the EXP-SW answers GET AUTOSW_FN, which its document lists under the heading Save a Scene. ([specs/wyrestorm-matrix.yaml:256](specs/wyrestorm-matrix.yaml#L256))
- [ ] Record changes the matrix sends on its own after a front-panel or web UI change, if any. ([specs/wyrestorm-matrix.yaml:832](specs/wyrestorm-matrix.yaml#L832))

## yamaha-cl-ql — Yamaha CL / QL

- [ ] Confirm the command set beyond scene recall and input fader, ON and pan (get and its reply, OK/OKm acknowledgements, ERROR codes, NOTIFY set pushes, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-cl-ql.yaml:5771](specs/yamaha-cl-ql.yaml#L5771))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-cl-ql.yaml:5834](specs/yamaha-cl-ql.yaml#L5834))
- [ ] Confirm the sscurrent_ex reply form "OK sscurrent_ex MIXER:Lib/Scene <number> [modified|unmodified]", taken by analogy with the DME7. ([specs/yamaha-cl-ql.yaml:5869](specs/yamaha-cl-ql.yaml#L5869))
- [ ] Confirm an out-of-range set is clamped and answered OKm. ([specs/yamaha-cl-ql.yaml:5816](specs/yamaha-cl-ql.yaml#L5816))
- [ ] On a CL and a QL, confirm the fader level, ON, pan and balance addresses from the Companion parameter list (StInCh, Mix, Mtrx, St, DCA Fader/Level and Fader/On; StInCh and Mix ToSt/Pan; StInCh, Mix, Mtrx and St Out/Balance; MuteMaster/On), and that set, get and NOTIFY set use them. ([specs/yamaha-cl-ql.yaml:5784](specs/yamaha-cl-ql.yaml#L5784))
- [ ] Confirm the numbering inferred from the list's counts: StInCh 1-16 as ST IN 1L to 8R, St 1-3 as ST L, ST R and MONO, St/Out/Balance 1-2, MuteMaster 1-8 as the mute groups. ([specs/yamaha-cl-ql.yaml:5795](specs/yamaha-cl-ql.yaml#L5795))
- [ ] Confirm the Label/Name, Label/Color and Label/Icon addresses on every channel type, the 8-character name limit, and the colour and icon names (Companion's chColors and chIcons; its list spells one icon "Wedge " with a trailing space). ([specs/yamaha-cl-ql.yaml:5888](specs/yamaha-cl-ql.yaml#L5888))
- [ ] Confirm the send addresses (InCh and StInCh ToMix and ToMtrx; Mix and St ToMtrx; Level, On, Pan, PrePost) and that PrePost 1 is PRE and 0 POST, as on RIVAGE PM. ([specs/yamaha-cl-ql.yaml:5896](specs/yamaha-cl-ql.yaml#L5896))
- [ ] Confirm Port/HA/Gain (hundredths of a dB, -6 to +66), the Dyna1/Dyna2 Threshold ranges (tenths of a dB) and DCA/Assign (Y = DCA 1-16) on every channel type. ([specs/yamaha-cl-ql.yaml:5904](specs/yamaha-cl-ql.yaml#L5904))
- [ ] Check whether newer firmware adds EQ, HPF, phantom power, phase, delay, insert, dynamics type/ratio/attack/release, or mute group assignment to the RCP parameter list, which the recorded list lacks. ([specs/yamaha-cl-ql.yaml:5913](specs/yamaha-cl-ql.yaml#L5913))
- [ ] Confirm the cue and monitor addresses (Cue/<type>/On, Cue/Output, CueMode, FaderCueRelease, OutputLevel, ActiveCue; Monitor/On, DimmerOn, CueInterruption, Fader/Level) and the CueMode and ActiveCue text values. ([specs/yamaha-cl-ql.yaml:5923](specs/yamaha-cl-ql.yaml#L5923))
- [ ] Confirm the output patch addresses (DanteOutPort, OmniOutPort, DigitalOutPort, SlotOut1-3Port, Recorder) and their source names, the OMNI output count per model, and that SlotOut3Port is CL only. ([specs/yamaha-cl-ql.yaml:5932](specs/yamaha-cl-ql.yaml#L5932))
- [ ] Confirm the CL: and QL: fader bank addresses, what their X and Y count (layers, strips, sections) and what the bank numbers 0-8 select, and that each family refuses the other's. ([specs/yamaha-cl-ql.yaml:5941](specs/yamaha-cl-ql.yaml#L5941))
- [ ] Confirm event MIXER:Lib/Scene/RecallInc and RecallDec (from the console's scninfo list and the DM7 document) and their replies. ([specs/yamaha-cl-ql.yaml:5949](specs/yamaha-cl-ql.yaml#L5949))
- [ ] Record what NOTIFY sscurrent_ex looks like after a scene recall from the surface, which now triggers a re-read of the connect-time settings. ([specs/yamaha-cl-ql.yaml:5956](specs/yamaha-cl-ql.yaml#L5956))
- [ ] Check whether the console answers prmnum and prminfo <index> as the DME7 document describes (the Companion list suggests it does), and whether the list differs by firmware and model. ([specs/yamaha-cl-ql.yaml:5965](specs/yamaha-cl-ql.yaml#L5965))
- [ ] Confirm store_scene: the command form ssupdate_ex MIXER:Lib/Scene <n> comes from the Companion module, while the console's own scninfo list names MIXER:Lib/Scene/Store (0-300); record its reply and whether scene #000 is refused. ([specs/yamaha-cl-ql.yaml:5979](specs/yamaha-cl-ql.yaml#L5979))
- [ ] Check that the console takes mtrstart with its meter list's address without the Meter segment (MIXER:Current/<channel type>/<pick-off>), as the Companion module sends it, and keeps sending NOTIFY mtr while it is asked again every 9 s. ([specs/yamaha-cl-ql.yaml:6380](specs/yamaha-cl-ql.yaml#L6380))
- [ ] Check the console's NOTIFY mtr layout: a type word, then one two-digit hexadecimal value per channel, as the DME7 document gives it, and the address it names. ([specs/yamaha-cl-ql.yaml:7435](specs/yamaha-cl-ql.yaml#L7435))
- [ ] Check that the console's meter values follow the DME7 document's table (00-7E for -126 to 0 dBFS, 7F OVER). ([specs/yamaha-cl-ql.yaml:94](specs/yamaha-cl-ql.yaml#L94))
- [ ] Check that the console answers ssinfo_ex with the DME7 document's layout (number text, title, comment, attribute) for every scene number read on connecting, empty scenes included. ([specs/yamaha-cl-ql.yaml:7449](specs/yamaha-cl-ql.yaml#L7449))

## yamaha-dm3 — Yamaha DM3

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-dm3.yaml:7838](specs/yamaha-dm3.yaml#L7838))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-dm3.yaml:7880](specs/yamaha-dm3.yaml#L7880))
- [ ] Record the sscurrent_ex reply form, which is matched leniently, and whether the query for the inactive scene list is answered with an error. ([specs/yamaha-dm3.yaml:7915](specs/yamaha-dm3.yaml#L7915))
- [ ] Confirm that the stereo input and FX return sends take the destination bus as Y (6 mixes, 2 FX, 2 matrices), as the console's parameter list gives it but Yamaha's document does not. ([specs/yamaha-dm3.yaml:7929](specs/yamaha-dm3.yaml#L7929))
- [ ] Check what send pre/post 0 and 1 mean. ([specs/yamaha-dm3.yaml:7930](specs/yamaha-dm3.yaml#L7930))
- [ ] Confirm local head-amp gain is whole dB 0 to 64. ([specs/yamaha-dm3.yaml:7931](specs/yamaha-dm3.yaml#L7931))
- [ ] Confirm on a console that the mix and matrix HPF/On, LPF/On, PEQ/On and PEQ/Band/Freq, Gain, Q and Type (4 bands) answer as the captured parameter list gives them, and on which firmware. ([specs/yamaha-dm3.yaml:7937](specs/yamaha-dm3.yaml#L7937))
- [ ] List the text values the console takes for PEQ band type (7, Bell the default), mix bus type (9, VARI), cue mode (6, MIX) and the input and output cue points (PFL, AFL). ([specs/yamaha-dm3.yaml:7946](specs/yamaha-dm3.yaml#L7946))
- [ ] Find what mix, input, stereo input, FX return and stereo pan mode 0 and 1 mean, and the meaning of the IO:Current/Dev SystemStatus (0-11) and SyncStatus (0-12) codes. ([specs/yamaha-dm3.yaml:7946](specs/yamaha-dm3.yaml#L7946))
- [ ] Confirm the cue switches, monitor on, level, cue interruption, mono and source selections answer as the captured parameter list gives them. ([specs/yamaha-dm3.yaml:7940](specs/yamaha-dm3.yaml#L7940))
- [ ] Check on current firmware whether input channel EQ, HPF, dynamics, DCA assignment or effect parameters have RCP addresses (none appear in Yamaha's document or the captured parameter list). ([specs/yamaha-dm3.yaml:7953](specs/yamaha-dm3.yaml#L7953))
- [ ] Check that the console takes mtrstart with its meter list's address without the Meter segment (MIXER:Current/<channel type>/<pick-off>), as the Companion module sends it, and keeps sending NOTIFY mtr while it is asked again every 9 s. ([specs/yamaha-dm3.yaml:8632](specs/yamaha-dm3.yaml#L8632))
- [ ] Check the console's NOTIFY mtr layout: a type word, then one two-digit hexadecimal value per channel, as the DME7 document gives it, and the address it names. ([specs/yamaha-dm3.yaml:8994](specs/yamaha-dm3.yaml#L8994))
- [ ] Check that the console's meter values follow the DME7 document's table (00-7E for -126 to 0 dBFS, 7F OVER). ([specs/yamaha-dm3.yaml:80](specs/yamaha-dm3.yaml#L80))
- [ ] Check that the console answers ssinfo_ex with the DME7 document's layout (number text, title, comment, attribute) for every scene number read on connecting, empty scenes included. ([specs/yamaha-dm3.yaml:9032](specs/yamaha-dm3.yaml#L9032))

## yamaha-dm7 — Yamaha DM7

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-dm7.yaml:9071](specs/yamaha-dm7.yaml#L9071))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-dm7.yaml:9114](specs/yamaha-dm7.yaml#L9114))
- [ ] Record the sscurrentt_ex reply form, which is matched leniently, and check whether ssrecallt_ex and sscurrentt_ex need scpmode sstype "text" first (the Companion module sends it). ([specs/yamaha-dm7.yaml:9153](specs/yamaha-dm7.yaml#L9153))
- [ ] Check what send pre/post 0 and 1 mean (RIVAGE gives 0 Post, 1 Pre). ([specs/yamaha-dm7.yaml:9164](specs/yamaha-dm7.yaml#L9164))
- [ ] Check the PEQ band gain scaling (10 or 100). ([specs/yamaha-dm7.yaml:9164](specs/yamaha-dm7.yaml#L9164))
- [ ] Confirm on current DM7 firmware (V2.0 or later) that no dynamics, insert, delay or effect parameter has an RCP address, and whether the RIVAGE PM addresses (MIXER:Current/InCh/Dyna1/...) are answered. ([specs/yamaha-dm7.yaml:9173](specs/yamaha-dm7.yaml#L9173))
- [ ] Confirm MIXER:Current/Mix/PanMode, Mix/BusType, Cue/ActiveCue, InCh/PanMode and St/PanMode answer as the captured parameter list gives them, and name the bus types and pan modes. ([specs/yamaha-dm7.yaml:9180](specs/yamaha-dm7.yaml#L9180))
- [ ] Check that the console takes mtrstart with its meter list's address without the Meter segment (MIXER:Current/<channel type>/<pick-off>), as the Companion module sends it, and keeps sending NOTIFY mtr while it is asked again every 9 s. ([specs/yamaha-dm7.yaml:9980](specs/yamaha-dm7.yaml#L9980))
- [ ] Check the console's NOTIFY mtr layout: a type word, then one two-digit hexadecimal value per channel, as the DME7 document gives it, and the address it names. ([specs/yamaha-dm7.yaml:10642](specs/yamaha-dm7.yaml#L10642))
- [ ] Check that the console's meter values follow the DME7 document's table (00-7E for -126 to 0 dBFS, 7F OVER). ([specs/yamaha-dm7.yaml:79](specs/yamaha-dm7.yaml#L79))
- [ ] Check that the console answers ssinfot_ex with the DME7 document's ssinfo_ex layout (number text, title, comment, attribute) for a scene number given as text. ([specs/yamaha-dm7.yaml:9136](specs/yamaha-dm7.yaml#L9136))

## yamaha-dme7 — Yamaha DME7

- [ ] Check whether Remote Control Setup List indexes start at 0 or 1. ([specs/yamaha-dme7.yaml:877](specs/yamaha-dme7.yaml#L877))
- [ ] Record which reply forms the device actually sends where the document shows two (setr, sscurrent_ex, ssnum_ex, ssinfo_ex, mtrinfo, devinfo port quoting, devstatus Power1/power1). ([specs/yamaha-dme7.yaml:947](specs/yamaha-dme7.yaml#L947))
- [ ] Check whether AudioPlayerGetStatus and listitemnum need the trailing "" from the syntax tables. ([specs/yamaha-dme7.yaml:957](specs/yamaha-dme7.yaml#L957))
- [ ] Check the scheduler time format beyond "9:00". ([specs/yamaha-dme7.yaml:962](specs/yamaha-dme7.yaml#L962))

## yamaha-mcp1 — Yamaha MCP1

- [ ] Confirm the preset index range (ssnum's count, numbered from 0 or 1) and the ssinfo answer layout. ([specs/yamaha-mcp1.yaml:127](specs/yamaha-mcp1.yaml#L127))
- [ ] Confirm the NOTIFY sscurrent push carries only the index. ([specs/yamaha-mcp1.yaml:199](specs/yamaha-mcp1.yaml#L199))

## yamaha-rivage — Yamaha RIVAGE PM

- [ ] Confirm the RCP command set (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-rivage.yaml:9682](specs/yamaha-rivage.yaml#L9682))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-rivage.yaml:9724](specs/yamaha-rivage.yaml#L9724))
- [ ] Find which address answers RCP on TCP 49280: the DSP engine, the console's Network PC port, or both. ([specs/yamaha-rivage.yaml:9750](specs/yamaha-rivage.yaml#L9750))
- [ ] Confirm get_current_scene (sscurrentt_ex) and its reply form, taken from the Companion module, and whether scpmode sstype "text" is needed first. ([specs/yamaha-rivage.yaml:9757](specs/yamaha-rivage.yaml#L9757))
- [ ] Check the Dyna1/Dyna2 Attack and Ratio scaling for COMP260 and other types, and the -720 threshold for gate and ducking. ([specs/yamaha-rivage.yaml:9769](specs/yamaha-rivage.yaml#L9769))
- [ ] Confirm the surround monitor channel switches are 0 ON, 1 MUTE. ([specs/yamaha-rivage.yaml:9776](specs/yamaha-rivage.yaml#L9776))
- [ ] Check that the console takes mtrstart with its meter list's address without the Meter segment (MIXER:Current/<channel type>/<pick-off>), as the Companion module sends it, and keeps sending NOTIFY mtr while it is asked again every 9 s. ([specs/yamaha-rivage.yaml:12214](specs/yamaha-rivage.yaml#L12214))
- [ ] Check the console's NOTIFY mtr layout: a type word, then one two-digit hexadecimal value per channel, as the DME7 document gives it, and the address it names. ([specs/yamaha-rivage.yaml:13523](specs/yamaha-rivage.yaml#L13523))
- [ ] Check that the console's meter values follow the DME7 document's table (00-7E for -126 to 0 dBFS, 7F OVER). ([specs/yamaha-rivage.yaml:70](specs/yamaha-rivage.yaml#L70))
- [ ] Check that the console answers ssinfot_ex with the DME7 document's ssinfo_ex layout (number text, title, comment, attribute) for a scene number given as text. ([specs/yamaha-rivage.yaml:11277](specs/yamaha-rivage.yaml#L11277))

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

- [ ] Confirm the command set beyond scene recall and input fader, ON and pan (get, OK/OKm, ERROR, NOTIFY set, devinfo, devstatus, scpmode keepalive), which Yamaha publishes only for DME7 and RM. ([specs/yamaha-tf.yaml:4624](specs/yamaha-tf.yaml#L4624))
- [ ] Check whether a scene recall pushes NOTIFY set for every changed parameter. ([specs/yamaha-tf.yaml:4686](specs/yamaha-tf.yaml#L4686))
- [ ] Confirm get_current_scene (sscurrent_ex scene_a|scene_b) and its reply, and that the inactive list's query is answered with an error. ([specs/yamaha-tf.yaml:4717](specs/yamaha-tf.yaml#L4717))
- [ ] Confirm the address uses "InCh" (the QLab guide prints "Inch"). ([specs/yamaha-tf.yaml:4732](specs/yamaha-tf.yaml#L4732))
- [ ] On a TF, confirm the fader level and ON addresses of every channel type (InCh, StInCh, FxRtnCh, DCA, Mix, Mtrx, St, Mono), ToSt/Pan on InCh, StInCh and FxRtnCh, Mix and St Out/Balance, Mix/PanLink and MuteMaster/On, and that set, get and NOTIFY set use them. ([specs/yamaha-tf.yaml:4637](specs/yamaha-tf.yaml#L4637))
- [ ] Confirm the numbering inferred from the list's counts: StInCh 1-4 as ST IN 1L, 1R, 2L, 2R; FxRtnCh 1-4 as FX1 L, FX1 R, FX2 L, FX2 R; St 1-2 as L and R; and whether ToStereo/Pan is an alias of ToSt/Pan. ([specs/yamaha-tf.yaml:4648](specs/yamaha-tf.yaml#L4648))
- [ ] Confirm the Label/Name (up to 64 characters in the list), Label/Color, Label/Icon and Label/Category addresses, the colour and icon names, and the category names (the list types icon and category as binary). ([specs/yamaha-tf.yaml:4738](specs/yamaha-tf.yaml#L4738))
- [ ] Confirm the send addresses (InCh and StInCh ToMix, ToFx, ToMono; FxRtnCh ToMix and ToMono; Mix, St and Mono ToMtrx), and that PrePost 1 is PRE and 0 POST. ([specs/yamaha-tf.yaml:4747](specs/yamaha-tf.yaml#L4747))
- [ ] Check whether newer TF firmware adds head-amp, EQ, dynamics, DCA or mute group assignment to the RCP parameter list, which the recorded list lacks. ([specs/yamaha-tf.yaml:4754](specs/yamaha-tf.yaml#L4754))
- [ ] Confirm event MIXER:Lib/Scene/RecallInc and RecallDec on TF (scninfo lists them) and their replies. ([specs/yamaha-tf.yaml:4763](specs/yamaha-tf.yaml#L4763))
- [ ] Record what NOTIFY sscurrent_ex looks like after a scene recall from the surface, which now triggers a re-read of the connect-time settings. ([specs/yamaha-tf.yaml:4771](specs/yamaha-tf.yaml#L4771))
- [ ] Check whether the console answers prmnum and prminfo <index> as the DME7 document describes, and whether the list differs by firmware and model. ([specs/yamaha-tf.yaml:4780](specs/yamaha-tf.yaml#L4780))
- [ ] Confirm store_scene: the command form ssupdate_ex scene_a|scene_b <n> comes from the Companion module, while the console's own scninfo list names MIXER:Lib/Bank/Scene/Store (two lists, 0-100); record its reply and whether scene 0 is refused. ([specs/yamaha-tf.yaml:4794](specs/yamaha-tf.yaml#L4794))
- [ ] Check that the console takes mtrstart with its meter list's address without the Meter segment (MIXER:Current/<channel type>/<pick-off>), as the Companion module sends it, and keeps sending NOTIFY mtr while it is asked again every 9 s. ([specs/yamaha-tf.yaml:5143](specs/yamaha-tf.yaml#L5143))
- [ ] Check the console's NOTIFY mtr layout: a type word, then one two-digit hexadecimal value per channel, as the DME7 document gives it, and the address it names. ([specs/yamaha-tf.yaml:5753](specs/yamaha-tf.yaml#L5753))
- [ ] Check that the console's meter values follow the DME7 document's table (00-7E for -126 to 0 dBFS, 7F OVER). ([specs/yamaha-tf.yaml:93](specs/yamaha-tf.yaml#L93))
- [ ] Check that the console answers ssinfo_ex with the DME7 document's layout (number text, title, comment, attribute) for every scene number read on connecting, empty scenes included. ([specs/yamaha-tf.yaml:5797](specs/yamaha-tf.yaml#L5797))

## yamaha-vxl — Yamaha VXL1-16P / VXC2P

- [ ] Record what the speaker answers to an unknown command or AccessID (the document has no error section; ERROR <command> <code> as on the MCP1 is assumed). ([specs/yamaha-vxl.yaml:39](specs/yamaha-vxl.yaml#L39))
- [ ] Record the raw scale of the compressor and ducker thresholds, ranges and attack (dB as integers assumed) and that a ratio of 201 is infinity:1. ([specs/yamaha-vxl.yaml:494](specs/yamaha-vxl.yaml#L494))
- [ ] Confirm whether NOTIFY mtr carries a level or gr word before the values (the document shows both forms). ([specs/yamaha-vxl.yaml:1697](specs/yamaha-vxl.yaml#L1697))
- [ ] Confirm that answering devstatus runmode is all the speaker needs before other commands, and that it pushes NOTIFY set for every change. ([specs/yamaha-vxl.yaml:40](specs/yamaha-vxl.yaml#L40))

## youtube-live — YouTube Live

- [ ] Confirm GET /youtube/v3/ answers without spending quota and without checking the token (a 404), so idle probes are free. ([specs/youtube-live.yaml:183](specs/youtube-live.yaml#L183))
- [ ] Confirm an expired or revoked access token answers 401 on every method, and that quota and permission failures answer 403. ([specs/youtube-live.yaml:179](specs/youtube-live.yaml#L179))
- [ ] Confirm transition, bind and cuepoint answer 200 (the reference gives no status), and that a transition's reply is the broadcast. ([specs/youtube-live.yaml:273](specs/youtube-live.yaml#L273))
- [ ] Confirm the deletes (broadcasts, streams, chat messages, bans) answer 204. ([specs/youtube-live.yaml:363](specs/youtube-live.yaml#L363))
- [ ] Record the limits on an ad break's durationSecs and a temporary ban's banDurationSeconds (the reference gives none). ([specs/youtube-live.yaml:326](specs/youtube-live.yaml#L326))
- [ ] Record the longest live chat message YouTube accepts (the reference gives no limit). ([specs/youtube-live.yaml:463](specs/youtube-live.yaml#L463))
- [ ] Check that lastUpdateTimeSeconds and concurrentViewers arrive as JSON strings, as Google's 64-bit integers do. ([specs/youtube-live.yaml:973](specs/youtube-live.yaml#L973))
- [ ] Confirm that a stream's health status and configuration issues are reported while it is active and bound, and how soon after a change. ([specs/youtube-live.yaml:974](specs/youtube-live.yaml#L974))
- [ ] Confirm a refresh at https://oauth2.googleapis.com/token with client_id alone (an installed-app client, no client_secret) succeeds, and that Google never rotates the refresh token on refresh. ([specs/youtube-live.yaml:174](specs/youtube-live.yaml#L174))
- [ ] Confirm a refresh token from a project in publishing status Testing fails with 400 invalid_grant after 7 days, and that a revoked one answers the same. ([specs/youtube-live.yaml:724](specs/youtube-live.yaml#L724))
- [ ] Record the access token lifetime Google gives (expires_in, about 3599 s expected), so the 300 s refresh margin is right. ([specs/youtube-live.yaml:175](specs/youtube-live.yaml#L175))
- [ ] Confirm pageInfo.totalResults on liveStreams.list with mine=true counts every stream of the channel: when it is 50 or fewer, the poll's page replaces streams. ([specs/youtube-live.yaml:994](specs/youtube-live.yaml#L994))
- [ ] Confirm the quota costs this spec now gives (every liveBroadcasts and liveStreams method 1 unit; chat list 5, insert, delete and transition 20; bans 200; chat moderators list 50, insert and delete 200) against the units the Cloud console counts. ([specs/youtube-live.yaml:737](specs/youtube-live.yaml#L737))
- [ ] Confirm videos.list with part snippet,status,liveStreamingDetails,statistics still costs 1 unit, and that a broadcast's video answers status.madeForKids and containsSyntheticMedia. ([specs/youtube-live.yaml:924](specs/youtube-live.yaml#L924))
- [ ] Confirm videos.update with part status alone keeps the snippet, and that a status body leaving out publicStatsViewable or embeddable resets them, as the reference says. ([specs/youtube-live.yaml:648](specs/youtube-live.yaml#L648))
- [ ] Confirm videos.update answers the video resource (kind youtube#video) with the parts asked for. ([specs/youtube-live.yaml:626](specs/youtube-live.yaml#L626))
- [ ] Confirm the moderator id liveChatModerators.list returns is the id liveChatModerators.delete takes, and that its characters are letters, digits, - and _ only. ([specs/youtube-live.yaml:551](specs/youtube-live.yaml#L551))
- [ ] Confirm liveChatModerators.list answers pageInfo.totalResults, so a list of 50 or fewer replaces chat_moderators. ([specs/youtube-live.yaml:1051](specs/youtube-live.yaml#L1051))
- [ ] Confirm a pollEvent insert with 2 to 4 options answers the message with its id, and record the longest question and option YouTube accepts. ([specs/youtube-live.yaml:561](specs/youtube-live.yaml#L561))
- [ ] Confirm close_chat_poll answers the message with status closed and each option's tally as a numeric string. ([specs/youtube-live.yaml:579](specs/youtube-live.yaml#L579))
- [ ] Confirm superChatEvents.list answers newest first and amountMicros as a JSON string. ([specs/youtube-live.yaml:590](specs/youtube-live.yaml#L590))
- [ ] Confirm thumbnails.set accepts the image as the whole request body with uploadType=media (the reference names the upload URL but not the upload type), answers 200 with the thumbnails set, and refuses a channel that may not use custom thumbnails with 403. ([specs/youtube-live.yaml:672](specs/youtube-live.yaml#L672))

## Integrations with no open items

Nothing specific is listed for these yet, but test results are still welcome: generic-http, generic-osc, generic-tcp-udp, generic-websocket, http-listener, http-snapshot, obs-studio.

## zcam — Z CAM cinema and PTZ cameras (HTTP API)

- [ ] Check that GET /ctrl/session occupies the control session for this client, what it answers (status and body) when another client holds it, and how long a held session lasts without requests. ([specs/zcam.yaml:3169](specs/zcam.yaml#L3169))
- [ ] Record what the websocket on port 81 sends (it could replace polling), and confirm /ctrl/get answers code -1 for a key the model does not have. ([specs/zcam.yaml:3176](specs/zcam.yaml#L3176))
- [ ] Record the answers of /ctrl/tc, /ctrl/stream_setting?action=query, /ctrl/rtmp, /ctrl/srt, /ctrl/ndi, /ctrl/pt?action=query, /ctrl/network?action=info, /ctrl/card?action=present, /ctrl/preset?action=get_info and /ctrl/framing/all_status, so their fields can be read into state. ([specs/zcam.yaml:3191](specs/zcam.yaml#L3191))
- [ ] Check the valid ranges of fspeed for /ctrl/lens zoom and focus, the pan/tilt speeds of action=pt, preset and trace numbers, and the framing fov, target_lost and placement values. ([specs/zcam.yaml:3191](specs/zcam.yaml#L3191))
- [ ] Find out how a client logs in when the camera's user accounts or HTTPS authentication are on, and whether clip downloads need it. ([specs/zcam.yaml:3199](specs/zcam.yaml#L3199))

## zerodensity-reality-hub — Zero Density Reality Hub

- [ ] Record the port Reality Hub serves its REST API on (80 assumed) and whether it is served over https. ([specs/zerodensity-reality-hub.yaml:656](specs/zerodensity-reality-hub.yaml#L656))
- [ ] Confirm a 403 for a missing module permission carries code INSUFFICIENT_PERMISSIONS, so it is not taken for a refused key. ([specs/zerodensity-reality-hub.yaml:54](specs/zerodensity-reality-hub.yaml#L54))
- [ ] Check that a node, property or function path holding slashes is accepted percent-encoded (%2F) as one segment. ([specs/zerodensity-reality-hub.yaml:672](specs/zerodensity-reality-hub.yaml#L672))
- [ ] Record the units of an engine's usedSpace and diskSpace. ([specs/zerodensity-reality-hub.yaml:642](specs/zerodensity-reality-hub.yaml#L642))
- [ ] Check whether a rundown no longer loaded should be removed from state (the spec keeps its last items). ([specs/zerodensity-reality-hub.yaml:663](specs/zerodensity-reality-hub.yaml#L663))
