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

| Spec | Devices | Transport | Verification |
|---|---|---|---|
| `aja-kipro` | AJA Ki Pro, Ki Pro GO | HTTP REST | none |
| `behringer-x32` | Behringer X32/Compact/Rack, Midas M32 | OSC/UDP 10023 | none |
| `behringer-wing` | Behringer WING, WING Compact, WING Rack | OSC/UDP 2223 | none |
| `blackmagic-atem` | ATEM Mini, SDI, Television Studio, Production Studio and Constellation families | Proprietary UDP 9910, native | none |
| `blackmagic-hyperdeck` | HyperDeck (protocol 1.8, 1.11+) | Line/TCP 9993, numeric codes | none |
| `blackmagic-videohub` | Smart Videohub family | Block/TCP 9990 | none |
| `etc-eos` | ETC Eos, Ion, Gio, Element | OSC/TCP 3032 | none |
| `grandma2` | grandMA2 console and onPC | Line/TCP 30000 | none |
| `kramer-p3000` | Kramer Protocol 3000 matrices | Line/TCP 5000 | none |
| `panasonic-ptz` | Panasonic AW-series PTZ | HTTP-CGI | none |
| `propresenter` | ProPresenter 7.9 and later (every API operation) | HTTP API | none |
| `ptzoptics` | PTZOptics cameras | HTTP-CGI | none |
| `qlab` | QLab 4, QLab 5 | OSC/UDP 53000 | none |
| `resolume` | Resolume Arena, Avenue | OSC/UDP 7000 | none |
| `rosstalk` | Ross Carbonite, Graphite, Acuity | Line/TCP 7788 | none |
| `shure-wireless` | Shure Axient Digital, ULX-D, QLX-D, SLX-D, PSM1000 | Command strings over TCP 2202, native | none |
| `tsl-umd-display` | Tally displays and multiviewers (TSL UMD V3.1, V4.0, V5.0) | UDP, or TCP for V5.0; sends; native | none |
| `tsl-umd-listener` | Tally from a switcher (TSL UMD V3.1, V4.0, V5.0) | UDP, or TCP for V5.0; receives; native | none |
| `vmix` | vMix | TCP API 8099, native | none |
| `sennheiser-ew-dx` | Sennheiser EW-DX EM 2, EM 2 Dante, EM 4 Dante | HTTPS + SSE (SSCv2), native | none |
| `sennheiser-ew-g3-g4` | Sennheiser EM 300-500 G4, SR IEM G4, EM 300-500 G3 | MCP over UDP 53212, native | none |
| `sennheiser-digital-6000` | Sennheiser EM 6000, EM 6000 Dante | SSC over UDP 45, native | none |
| `newtek-tricaster` | TriCaster Advanced Edition, TC1, TC2 Elite, Mini, Vizion | HTTP shortcuts 80 | none |
| `obs-studio` | OBS Studio 28 and later | obs-websocket 5 over WebSocket 4455, native | none |

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
