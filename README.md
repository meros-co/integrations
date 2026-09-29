# meros-device-spec

Declarative specifications for controlling third-party production hardware and
software: audio consoles, video routers, recorders, switchers, cameras, lighting
desks, wireless receivers and playback software.

A spec describes a control protocol as data. An interpreter library reads the
spec and talks to the device, so protocol details are defined once instead of
reimplemented per language.

**Status:** format v1. Three specs published, none hardware-verified. No
interpreter released yet. The format may still change.

## Rationale

Meros products are written in PHP, TypeScript, Rust, Go, Python, C++ and C. A
shared library cannot span those, so protocol code has been written more than
once for the same device. Duplicated implementations diverge: the Shure wireless
protocol was implemented twice internally, and only one of the two recorded that
SLX-D has no mute command. The other reports success for a command the receiver
ignores.

Specs are data, so every language reads the same file and inherits the same
corrections.

## Format

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

`bool10` inverts the boolean: the X32 treats `mix/on` as `0` for muted.

Full definition in [SPEC.md](SPEC.md).

## Layout

```
devices/      one spec per device family
vectors/      conformance vectors: exact bytes per command
schema/       JSON Schema for specs
tools/        validator
```

## Published specs

| Spec | Devices | Transport | Verification |
|---|---|---|---|
| `aja-kipro` | AJA Ki Pro, Ki Pro GO | HTTP REST | none |
| `behringer-x32` | Behringer X32/Compact/Rack, Midas M32 | OSC/UDP 10023 | none |
| `blackmagic-hyperdeck` | HyperDeck (protocol 1.8, 1.11+) | Line/TCP 9993, numeric codes | none |
| `blackmagic-videohub` | Smart Videohub family | Block/TCP 9990 | none |
| `etc-eos` | ETC Eos, Ion, Gio, Element | OSC/TCP 3032 | none |
| `grandma2` | grandMA2 console and onPC | Line/TCP 30000 | none |
| `kramer-p3000` | Kramer Protocol 3000 matrices | Line/TCP 5000 | none |
| `panasonic-ptz` | Panasonic AW-series PTZ | HTTP-CGI | none |
| `propresenter` | ProPresenter 7 | HTTP API 50000 | none |
| `ptzoptics` | PTZOptics cameras | HTTP-CGI | none |
| `qlab` | QLab 4, QLab 5 | OSC/UDP 53000 | none |
| `resolume` | Resolume Arena, Avenue | OSC/UDP 7000 | none |
| `rosstalk` | Ross Carbonite, Graphite, Acuity | Line/TCP 7788 | none |
| `shure-wireless` | Shure ULX-D, QLX-D, Axient Digital, SLX-D, PSM1000 | Line/TCP 2202 | none |

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

## Consuming a spec

Interpreters are published separately, one per language.

| Language | Package | Status |
|---|---|---|
| PHP | `meros-device-php` | not started |
| TypeScript | `meros-device-ts` | not started |
| Rust | `meros-device-rs` | not started |
| Python | `meros-device-py` | not started |
| Go | `meros-device-go` | not started |

Pin a version. A spec change alters what every consumer sends to customer
hardware, so it should not be picked up automatically in a show-critical path.

## Validation

```
pip install pyyaml jsonschema
python tools/validate.py
```

Checks each spec against the JSON Schema plus cross-field rules: command
references in `supports`, model references in `quirks`, unique spec ids, and
whether a claimed verification status has vectors behind it.

## Scope

Protocols that require conditionals, sequencing or session state are out of
scope and are not forced into the format. Known cases:

| Protocol | Reason |
|---|---|
| Blackmagic ATEM | Proprietary UDP with session handshake, sequencing, retransmission |
| Sennheiser Digital 6000 | Subscription lifecycle with periodic renewal |
| Ember+ (Wisycom) | BER/S101 framing, Glow object model |
| Dante | No public protocol |

These get a `native` spec carrying models, quirks and vectors, with the
implementation in code. See the escape hatch in [SPEC.md](SPEC.md).

## Contributing

Corrections, quirks, verification runs and new specs are welcome. Meros owns
this repository and reviews and merges all changes. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, see [LICENSE](LICENSE). The licence covers this repository's contents:
specs, vectors, schema and tooling. It does not extend to the devices described
or their manufacturers' trademarks. Contribute observed behaviour and documented
facts, not code copied from other projects.
