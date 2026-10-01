# integrations

Control and monitoring for production equipment: audio consoles, video
switchers and routers, cameras, recorders, lighting desks, wireless
microphones, projectors and playback software.

Each device is implemented once, in a Rust library, and used from Rust, C,
Node, Python or a local HTTP service.

This project is pre-release. Every integration is written from the
manufacturer's documentation and tested against simulated devices. None has
been tested against real hardware yet.

## Using it

| Package | Name |
|---|---|
| Rust | `meros-integrations` (`crates/core`) |
| C | static library and header (`bindings/c`) |
| Node | `@meros/integrations` (`bindings/node`) |
| Python | `meros-integrations` (`bindings/python`) |
| HTTP service | `meros-integrations serve` (`crates/sidecar`) |

All of them run the same library, so a device behaves the same way whichever
one you use.

Pin an exact version. A new release can change what is sent to your hardware.

## How devices are described

Most devices are described in a YAML file in `specs/`: their models, commands,
parameters, replies and the state they report. The library reads these files
and talks to the device. A command looks like this:

```yaml
commands:
  mute_channel:
    params:
      channel: { type: int, min: 1, max: 32, required: true }
      muted:   { type: bool, default: true }
    send:
      address: /ch/{channel:02d}/mix/on
      args: [ { value: "{muted:bool10}", type: int } ]
```

The X32 uses `0` for muted on `mix/on`, so `bool10` inverts the value. The
format is documented in [SPEC.md](SPEC.md).

Devices whose protocols need more than fixed messages and replies, such as
handshakes, sessions or binary framing, are written in Rust instead. They
still have a YAML file listing their models, commands and quirks, so every
device looks the same to whoever uses the library.

## Devices

| Spec | Devices | Transport | Models | Commands |
|---|---|---|---|---|
| `aja-kipro` | AJA Ki Pro recorders | HTTP 80 | 8 | 69 |
| `aja-kumo` | AJA KUMO SDI routers and control panels | HTTP 80 | 9 | 33 |
| `allenheath-ahm` | Allen & Heath AHM | MIDI over TCP 51325, native | 3 | 28 |
| `allenheath-cq` | Allen & Heath CQ (CQ-12T, CQ-18T, CQ-20B) | MIDI over TCP 51325, native | 3 | 19 |
| `allenheath-dlive` | Allen & Heath dLive and Avantis | MIDI over TCP 51325 (MixRack) or 51328 (Surface), native | 3 | 38 |
| `allenheath-qu` | Allen & Heath Qu (Qu-16, Qu-24, Qu-32, Qu-Pac, Qu-SB) | MIDI over TCP 51325, native | 5 | 28 |
| `allenheath-sq` | Allen & Heath SQ, SQ+ and Qu-5/6/7 | MIDI over TCP 51325, native | 9 | 28 |
| `analogway-alta4k` | Analog Way Alta 4K (AWJ) | AWJ over TCP 10606, native | 2 | 51 |
| `analogway-livecore` | Analog Way LiveCore series (TPP) | Line/TCP 10600 | 14 | 39 |
| `analogway-livepremier` | Analog Way LivePremier / Aquilon (AWJ) | AWJ over TCP 10606, native | 11 | 32 |
| `analogway-midra` | Analog Way Midra series (TPP) | Line/TCP 10500 | 10 | 34 |
| `analogway-midra4k` | Analog Way Midra 4K (AWJ) | AWJ over TCP 10606, native | 4 | 51 |
| `analogway-picturall` | Analog Way Picturall media servers | Line/TCP 11000 | 9 | 15 |
| `barco-eventmaster` | Barco Event Master (JSON-RPC API) | HTTP 9999 | 4 | 104 |
| `behringer-wing` | Behringer WING | OSC/UDP 2223 | 3 | 242 |
| `behringer-x32` | Behringer X32 / Midas M32 | OSC/UDP 10023 | 8 | 178 |
| `behringer-xair` | Behringer X AIR / Midas M AIR | OSC/UDP 10024 | 6 | 241 |
| `birddog` | BirdDog cameras (RESTful API) | HTTP 8080 | 19 | 334 |
| `blackmagic-atem` | Blackmagic ATEM switchers | Proprietary UDP 9910, native | 26 | 56 |
| `blackmagic-camera` | Blackmagic cameras (Camera Control REST API) | HTTP 80 | 12 | 223 |
| `blackmagic-hyperdeck` | Blackmagic HyperDeck | Line/TCP 9993 | 5 | 165 |
| `blackmagic-streaming` | Blackmagic Web Presenter / Streaming Encoder | Line/TCP 9977 | 4 | 31 |
| `blackmagic-videohub` | Blackmagic Videohub | Line/TCP 9990 | 20 | 54 |
| `chamsys-magicq-udp` | ChamSys MagicQ / QuickQ Remote Ethernet Protocol | Line/UDP 6553 | 18 | 63 |
| `chamsys-magicq` | ChamSys MagicQ / QuickQ | OSC/UDP 8000 | 16 | 88 |
| `emberplus` | Ember+ provider (generic consumer) | Ember+ (S101) over TCP, native | 1 | 10 |
| `etc-eos` | ETC Eos family | OSC/TCP 3032 | 18 | 175 |
| `generic-http` | Generic HTTP device | HTTP or HTTPS, native | 1 | 2 |
| `generic-osc` | Generic OSC device | OSC over UDP or TCP, native | 1 | 3 |
| `generic-tcp-udp` | Generic TCP or UDP device | Text or bytes over TCP or UDP, native | 1 | 3 |
| `grandma2` | MA Lighting grandMA2 | Line/TCP 30000 | 6 | 114 |
| `grandma3` | MA Lighting grandMA3 | OSC/UDP 8000 | 7 | 165 |
| `h2r-graphics` | H2R Graphics (HTTP API) | HTTP 4001 | 2 | 21 |
| `kramer-p3000` | Kramer Protocol 3000 matrices and switchers | Line/TCP 5000 | 2 | 86 |
| `newtek-tricaster` | NewTek / Vizrt TriCaster | HTTP 80 | 2 | 20 |
| `obs-studio` | OBS Studio (obs-websocket 5) | obs-websocket 5 over WebSocket 4455, native | 1 | 147 |
| `panasonic-ptz` | Panasonic AW-series PTZ cameras | HTTP 80 | 12 | 446 |
| `pjlink` | PJLink projectors and displays (Class 1 and Class 2) | PJLink over TCP 4352, native | 2 | 27 |
| `propresenter` | ProPresenter 7 (ProPresenter API) | HTTP 50001 | 1 | 213 |
| `ptzoptics` | PTZOptics cameras | HTTP 80 | 7 | 162 |
| `qlab` | Figure 53 QLab | OSC/TCP 53000 | 2 | 413 |
| `qsys` | QSC Q-SYS Core (Q-SYS Remote Control, QRC) | QRC JSON-RPC over TCP 1710, native | 2 | 44 |
| `renewedvision-pvp` | ProVideoPlayer 3 (PVP3 HTTP API) | HTTP 8080 | 1 | 65 |
| `resolume` | Resolume Arena / Avenue (REST API) | HTTP 8080 | 2 | 66 |
| `roland-p20hd` | Roland P-20HD (LAN control) | Line/TCP 8023 | 1 | 54 |
| `roland-v160hd` | Roland V-160HD / V-80HD / VR-120HD / VR-6HD / V-1-4K (LAN control) | Line/TCP 8023 | 5 | 192 |
| `roland-v600uhd` | Roland V-600UHD (LAN control) | Line/TCP 8023 | 1 | 20 |
| `roland-v60hd` | Roland V-60HD (LAN control) | Line/TCP 8023 | 1 | 40 |
| `roland-vr400uhd` | Roland VR-400UHD (LAN control) | Line/TCP 8023 | 1 | 36 |
| `roland-xs42h` | Roland XS-42H / VP-42H (LAN control) | Line/TCP 8023 | 2 | 24 |
| `roland-xs62s` | Roland XS-62S (LAN control) | Line/TCP 8023 | 1 | 53 |
| `roland-xs80h` | Roland XS-82H / XS-83H / XS-84H (LAN control) | Line/TCP 8023 | 3 | 37 |
| `ross-xpression` | Ross XPression (RossTalk) | Line/TCP 7788 | 1 | 28 |
| `rosstalk` | Ross Video production switchers (RossTalk) | Line/TCP 7788 | 13 | 101 |
| `sennheiser-digital-6000` | Sennheiser Digital 6000 receivers (SSC) | SSC over UDP 45, native | 2 | 5 |
| `sennheiser-ew-dx` | Sennheiser Evolution Wireless Digital EW-DX receivers (SSCv2) | HTTPS + SSE (SSCv2), native | 3 | 1 |
| `sennheiser-ew-g3-g4` | Sennheiser evolution wireless G3 / G4 (Media Control Protocol) | MCP over UDP 53212, native | 3 | 11 |
| `shure-wireless` | Shure networked wireless (command strings) | Command strings over TCP 2202, native | 9 | 6 |
| `tsl-umd-display` | TSL UMD tally sent to displays and multiviewers | TSL UMD over UDP, or TCP for V5.0; sends, native | 4 | 1 |
| `tsl-umd-listener` | TSL UMD tally received from a switcher | TSL UMD over UDP, or TCP for V5.0; receives, native | 2 | 0 |
| `visca` | VISCA PTZ cameras (Sony VISCA over IP, and VISCA over TCP/UDP) | VISCA over IP (UDP 52381), raw VISCA over TCP or UDP, native | 9 | 142 |
| `vmix` | vMix | TCP API 8099, native | 1 | 778 |
| `yamaha-cl-ql` | Yamaha CL and QL series digital mixing consoles (Remote Control Protocol, TCP 49280) | Line/TCP 49280 | 5 | 16 |
| `yamaha-dm3` | Yamaha DM3 series digital mixing consoles (Remote Control Protocol, TCP 49280) | Line/TCP 49280 | 2 | 211 |
| `yamaha-dm7` | Yamaha DM7 series digital mixing consoles (Remote Control Protocol, TCP 49280) | Line/TCP 49280 | 2 | 401 |
| `yamaha-dme7` | Yamaha DME7 digital signal processor | Line/TCP 49280 | 1 | 67 |
| `yamaha-rivage` | Yamaha RIVAGE PM series digital mixing systems (Remote Control Protocol, TCP 49280) | Line/TCP 49280 | 4 | 463 |
| `yamaha-rm` | Yamaha RM series (RM-CR, RM-CG, RM-TT, RM-WAP) | Line/TCP 49280 | 5 | 495 |
| `yamaha-tf` | Yamaha TF series digital mixing consoles (Remote Control Protocol, TCP 49280) | Line/TCP 49280 | 4 | 15 |

Specs marked `native` are implemented in Rust.

## Verification

Each model records how far it has been tested.

| Status | Meaning |
|---|---|
| `none` | Written from documentation, not tested against hardware |
| `bench` | Tested against the device |
| `field` | In use in a production installation |

A model moves to `bench` or `field` only with a conformance vector recorded
from the device. Every model is currently `none`.

## Repository layout

```
specs/      one YAML file per device family
schema/     JSON Schema for the spec format
vectors/    expected bytes for each command, used by the tests
crates/     the Rust library, the HTTP service and a device simulator
bindings/   C, Node and Python bindings
tools/      spec validator and vector generator
```

## Building and testing

```
cargo test
pip install pyyaml jsonschema
python tools/validate.py
```

`cargo test` runs the library's tests, including every conformance vector.
`validate.py` checks each spec against the schema and the format's rules.

## Contributing

Corrections, undocumented device behaviour, test results from real hardware
and new devices are all welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, see [LICENSE](LICENSE). Product names and trademarks belong to their
owners.
