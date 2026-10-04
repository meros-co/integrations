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

Each device type is one **integration**, named by its spec id
(`sennheiser-ew-dx`, `shure-wireless`, `sony-camera`, ...; the full list is
in [DEVICES.md](DEVICES.md)). You choose which integrations you get, one by one,
or by vendor (`vendor-sennheiser`, `vendor-sony`, ...), or `all`.

**When building**, name the integrations to build in. Every other
integration, with its spec, code, discovery and dependencies, is left out:

| Package | How to choose |
|---|---|
| Rust | `meros-integrations = { version = "...", features = ["sennheiser-ew-dx", "shure-wireless"] }` |
| C | `cargo build --release -p meros-integrations-c --no-default-features --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless` |
| Node | `MEROS_INTEGRATIONS=sennheiser-ew-dx,shure-wireless npm run build` (in `bindings/node`) |
| Python | `python build_dev.py --integrations sennheiser-ew-dx,shure-wireless` (in `bindings/python`), or for a wheel `maturin build --release --no-default-features --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless` |
| HTTP service | `cargo build --release -p meros-integrations-sidecar --no-default-features --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless` |

The Rust crate builds no integration unless you name some. The C library,
Node and Python packages and the HTTP service build all of them unless you
name some. The features are listed in `crates/core/Cargo.toml`; each package's README
has the details.

**When starting the core**, you can narrow it further to some of the
integrations it was built with: pass their spec ids or vendor groups as
`devices` in the core options (`CoreOptions::new().devices([...])` in Rust,
the options JSON in C, `new Core({ devices })` in Node, `Core(devices=...)` in
Python, `--devices a,b,c` for the HTTP service). Only those devices are
listed, opened and discovered.

**The catalogue** (`catalog` in every package, `GET /v1/catalog` from the
HTTP service) describes every integration in the build: its models,
commands and their parameters, settings, state, and every port it uses with
its default (`ports`), including the integrations that have no default port
and need one given.

**When opening a device**, the core normally keeps its state current:
it subscribes to changes, reads the state on connecting, and polls where it
has to. Open it with `monitor: false` to send commands only. The core then
asks for nothing but what commands and its connection check need, so it
doesn't take a subscription slot the device has few of (a Behringer WING has
one). Each device's most recent request-to-reply time is reported as
`latency_ms` on its snapshot and `alive` events.

**Events** come from one queue, the same JSON in every package (SPEC.md,
Events). A listener such as `osc-listener`, which hears OSC from any control
surface and is opened with no host, reports each message it receives as a
`message` event, even when it repeats the last, so a button pressed twice
arrives twice; a slow consumer loses state patches before it loses those.

**When shutting down**, call `close_all` (`closeAll` in Node, `mi_close_all`
in C) before the process exits. The core refuses new work, gives commands in
flight a few seconds, then closes every device so each ends what it started
on it (subscriptions, metering, sessions). The HTTP service does this itself
on SIGTERM or SIGINT, so it can run under systemd.

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

[DEVICES.md](DEVICES.md) lists every supported integration, with its vendor
group, transport, models, command count and verification status, and the
devices on the roadmap.

## Verification

Each model records how far it has been tested.

| Status | Meaning |
|---|---|
| `none` | Written from documentation, not tested against hardware |
| `bench` | Tested against the device |
| `field` | In use in a production installation |

A model moves to `bench` or `field` only with a conformance vector recorded
from the device. Every model is currently `none`.

[VERIFICATION.md](VERIFICATION.md) lists the behaviour that most needs
checking on real devices, and how to report what you find.

## Repository layout

```
specs/      one YAML file per integration
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

Builds with only some integrations should also pass, for example:

```
cargo test -p meros-integrations --features sennheiser-ew-dx,shure-wireless
cargo test -p meros-integrations --features all
cargo build -p meros-integrations
python tools/check_features.py
```

`check_features.py` builds the library once per integration, each on its own.

## Contributing

Corrections, undocumented device behaviour, test results from real hardware
and new devices are all welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, see [LICENSE](LICENSE). Product names and trademarks belong to their
owners.
