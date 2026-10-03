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

## h2r-graphics — H2R Graphics

- [ ] Record the HTTP status of a successful request (200 assumed) and of an error reply from v3.4 onward. ([specs/h2r-graphics.yaml:352](specs/h2r-graphics.yaml#L352))
- [ ] Confirm the text variable id form text.N in updateVariableText. ([specs/h2r-graphics.yaml:391](specs/h2r-graphics.yaml#L391))
- [ ] Check the update_score team and level ranges. ([specs/h2r-graphics.yaml:398](specs/h2r-graphics.yaml#L398))
- [ ] Check whether select_list_row counts rows from 1 or 0. ([specs/h2r-graphics.yaml:401](specs/h2r-graphics.yaml#L401))
- [ ] On version 2, check how an older version answers commands it predates. ([specs/h2r-graphics.yaml:383](specs/h2r-graphics.yaml#L383))

## kramer-p3000 — Kramer Protocol 3000

- [ ] Check whether ROUTE with source 0 disconnects, as the legacy commands do. ([specs/kramer-p3000.yaml:1135](specs/kramer-p3000.yaml#L1135))
- [ ] Check whether AUD-LVL's channel means the Audio Channel table or an input/output number on the product in use. ([specs/kramer-p3000.yaml:1164](specs/kramer-p3000.yaml#L1164))
- [ ] Confirm the documentation inconsistencies taken as written: the LABEL query with only a port number, NAME? echoed with the question mark, the #INFO-PRST? query form, and PRST-STO/PRST-RCL replies without "nn@". ([specs/kramer-p3000.yaml:1151](specs/kramer-p3000.yaml#L1151))
- [ ] Find the LOGIN level argument's wire spelling, so login can be modelled. ([specs/kramer-p3000.yaml:1176](specs/kramer-p3000.yaml#L1176))
- [ ] Record the exact X-SIGNAL and X-AFV reply forms, which are matched leniently because the guide has typing errors. ([specs/kramer-p3000.yaml:1187](specs/kramer-p3000.yaml#L1187))
- [ ] Confirm VMUTE flag 2 (blank picture) is unsupported. ([specs/kramer-p3000.yaml:1162](specs/kramer-p3000.yaml#L1162))

## newtek-tricaster — NewTek TriCaster

- [ ] Record the response body of a shortcut request, since whether the shortcut applied is not confirmed by the 200. ([specs/newtek-tricaster.yaml:269](specs/newtek-tricaster.yaml#L269))
- [ ] Confirm the login scheme (Basic sent, Digest answered if challenged). ([specs/newtek-tricaster.yaml:262](specs/newtek-tricaster.yaml#L262))

## panasonic-ptz — Panasonic PTZ

- [ ] Confirm the menu keys need DUP:1 (the AW-UE usage examples send DUP). ([specs/panasonic-ptz.yaml:4325](specs/panasonic-ptz.yaml#L4325))
- [ ] Confirm colour correction Yl_Yl_G uses OSD:1C/OSD:1D, not OSJ. ([specs/panasonic-ptz.yaml:4331](specs/panasonic-ptz.yaml#L4331))
- [ ] Confirm the AWB A/B numbering difference between control (1, 2) and query (2, 3), and get_scene 0-3 for scenes 1-4. ([specs/panasonic-ptz.yaml:4310](specs/panasonic-ptz.yaml#L4310))

## pjlink — PJLink

- [ ] With authentication auto, check how a projector that predates 2.10 answers "PJLINK 2", and whether it counts it as a failed login. ([specs/pjlink.yaml:213](specs/pjlink.yaml#L213))
- [ ] Check whether AVMT ? ever returns 10 or 20, and how they read. ([specs/pjlink.yaml:234](specs/pjlink.yaml#L234))
- [ ] Confirm the power-on response is "%1POWR=OK". ([specs/pjlink.yaml:240](specs/pjlink.yaml#L240))
- [ ] On a Class 2 projector, find how the controller address for status notifications is registered. ([specs/pjlink.yaml:254](specs/pjlink.yaml#L254))

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
