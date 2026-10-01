# integrations

One implementation of control and telemetry for third-party production hardware
and software: audio consoles, video routers, recorders, switchers, cameras,
lighting desks, wireless systems and playback software.

**Status:** pre-release. The core drives every spec in `specs/`: the
spec-driven ones through its spec engine and the Sennheiser families through
native modules. All of it is tested against simulated devices only. No model is
hardware-verified, and nothing here is ready for a show.

## Rationale

The same device protocol written separately in several products diverges.
Internally, the Shure wireless protocol was implemented twice, and only one of
the two recorded that SLX-D has no mute command; the other reports success for a
command the receiver ignores. Sharing a description of the protocol is not
enough either: two interpreters of the same description are still two
implementations, each with its own bugs.

So every integration here has exactly one implementation, in a Rust core that
owns the whole conversation with the device: sockets, TLS, framing, replies,
subscriptions, reconnection. Products in other languages use that same compiled
core through a binding, and bindings only convert calls and data.

## How a device is implemented

| Kind | For | Written as |
|---|---|---|
| Spec-driven | Protocols made of fixed messages and fixed replies | A YAML spec in `specs/`, run by the core's spec engine |
| Native | Protocols needing real logic: handshakes, sequencing, subscriptions | A Rust module in the core, plus a YAML spec carrying its models, commands and quirks |

Both kinds present the same catalogue and the same API. A consumer cannot tell
which kind a device is.

A spec-driven command:

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

`bool10` inverts the boolean: the X32 treats `mix/on` as `0` for muted. The full
format is in [SPEC.md](SPEC.md).

## Layout

```
specs/        one YAML spec per device family
vectors/      conformance vectors: exact bytes per command, used as core test fixtures
schema/       JSON Schema for specs
crates/       the Rust core
bindings/     C, Node, Python and sidecar deliveries of the core
tools/        spec validator
```

## Deliveries

| Delivery | Package | Status |
|---|---|---|
| Rust | `meros-integrations` crate | working; unreleased |
| C | static library and header (`bindings/c`) | working; unreleased |
| Node | `@meros/integrations` | working; unreleased |
| Python | `meros-integrations` (`bindings/python`) | working; unreleased |
| Sidecar | `meros-integrations serve`, a local HTTP service | working; unreleased |

Every delivery runs the same core and behaves identically.

Pin an exact version. A release changes what every consumer sends to customer
hardware, so it should never be picked up automatically in a show-critical path.

## Specs

| Spec | Devices | Transport | Models | Commands |
|---|---|---|---|---|
| `aja-kipro` | AJA Ki Pro recorders | HTTP 80 | 8 | 69 |
| `aja-kumo` | AJA KUMO SDI routers and control panels | HTTP 80 | 9 | 33 |
| `allenheath-ahm` | Allen & Heath AHM | MIDI over TCP 51325, native | 3 | 28 |
| `allenheath-cq` | Allen & Heath CQ (CQ-12T, CQ-18T, CQ-20B) | MIDI over TCP 51325, native | 3 | 19 |
| `allenheath-dlive` | Allen & Heath dLive and Avantis | MIDI over TCP 51325 (MixRack) or 51328 (Surface), native | 3 | 38 |
| `allenheath-qu` | Allen & Heath Qu (Qu-16, Qu-24, Qu-32, Qu-Pac, Qu-SB) | MIDI over TCP 51325, native | 5 | 28 |
| `allenheath-sq` | Allen & Heath SQ, SQ+ and Qu-5/6/7 | MIDI over TCP 51325, native | 9 | 28 |
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
| `etc-eos` | ETC Eos family | OSC/TCP 3032 | 18 | 175 |
| `grandma2` | MA Lighting grandMA2 | Line/TCP 30000 | 6 | 114 |
| `grandma3` | MA Lighting grandMA3 | OSC/UDP 8000 | 7 | 165 |
| `h2r-graphics` | H2R Graphics (HTTP API) | HTTP 4001 | 2 | 21 |
| `kramer-p3000` | Kramer Protocol 3000 matrices and switchers | Line/TCP 5000 | 2 | 86 |
| `newtek-tricaster` | NewTek / Vizrt TriCaster | HTTP 80 | 2 | 20 |
| `obs-studio` | OBS Studio (obs-websocket 5) | obs-websocket 5 over WebSocket 4455, native | 1 | 147 |
| `panasonic-ptz` | Panasonic AW-series PTZ cameras | HTTP 80 | 12 | 446 |
| `propresenter` | ProPresenter 7 (ProPresenter API) | HTTP 50001 | 1 | 213 |
| `ptzoptics` | PTZOptics cameras | HTTP 80 | 7 | 162 |
| `qlab` | Figure 53 QLab | OSC/TCP 53000 | 2 | 413 |
| `renewedvision-pvp` | ProVideoPlayer 3 (PVP3 HTTP API) | HTTP 8080 | 1 | 65 |
| `resolume` | Resolume Arena / Avenue (REST API) | HTTP 8080 | 2 | 66 |
| `roland-v160hd` | Roland V-160HD / V-80HD / VR-120HD / VR-6HD / V-1-4K (LAN control) | Line/TCP 8023 | 5 | 192 |
| `roland-v600uhd` | Roland V-600UHD (LAN control) | Line/TCP 8023 | 1 | 20 |
| `roland-vr400uhd` | Roland VR-400UHD (LAN control) | Line/TCP 8023 | 1 | 36 |
| `roland-xs42h` | Roland XS-42H / VP-42H (LAN control) | Line/TCP 8023 | 2 | 24 |
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

Every model is currently `none` (see Verification): written from manufacturer
documentation, not yet run against hardware.

## Verification

Each model carries a verification status.

| Status | Meaning |
|---|---|
| `none` | Written from documentation. Not tested against hardware |
| `bench` | Exercised against the physical device |
| `field` | Running in a production installation |

Promotion to `bench` or `field` requires a conformance vector recorded from the
device. Documentation alone is not sufficient: one manufacturer PDF referenced
here gives two different sample layouts in two sections.

## Building and testing

```
cargo test                      # the core: modules, spec engine, every vector
pip install pyyaml jsonschema
python tools/validate.py        # every spec against the schema and format rules
```

Checks each spec against the JSON Schema plus cross-field rules: template
references and directives, command references in `supports`, model references
in `quirks`, unique spec ids, and whether a claimed verification status has
vectors behind it.

## Out of scope

| Protocol | Reason |
|---|---|
| Dante | No public protocol |

## Contributing

Corrections, quirks, verification runs and new devices are welcome. Meros owns
this repository and reviews and merges all changes. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, see [LICENSE](LICENSE). The licence covers this repository's contents. It
does not extend to the devices described or their manufacturers' trademarks.
Contribute observed behaviour and documented facts, not code copied from other
projects.
