# Contributing

Issues and pull requests are welcome. Meros maintains this repository and
reviews every change against the checklist below.

## What helps most

1. **Device behaviour the documentation gets wrong or leaves out.** Add it to
   the spec as a quirk.
2. **Test results from real hardware.** Most models here have only been tested
   against simulated devices. [VERIFICATION.md](VERIFICATION.md) lists what
   most needs checking; report results with an issue or a pull request, as
   described there. See also [Verification](#verification).
3. **Corrections.** Wrong ranges, misread replies, commands a model doesn't
   have.
4. **New devices.**

## Sources

Write specs from the manufacturer's documentation or from behaviour you have
observed on the device. Cite the document, its version and the page or section
in the spec's `source` list.

Don't copy code from other projects. If you learned something by reading
another implementation, cite it and describe the behaviour in your own words.
Only read code whose licence allows it: MIT, BSD and Apache are fine; don't use
GPL, LGPL or AGPL code at all.

Some manufacturers license their documentation (Sony's Camera Remote Command,
for example). Never commit a licensed document or text copied from one.

## Verification

Every model has a `verification` status: `none`, `bench` or `field`. New models
start at `none`. To raise it, record a conformance vector from the device and
include it in the same pull request. `tools/validate.py` rejects a raised status
with no recorded vector behind it.

A recorded vector looks like this:

```yaml
spec: blackmagic-hyperdeck
model: hyperdeck-studio
command: record
input: {}
expect_wire: "record\r\n"
device_reply: "200 ok\r\n"
recorded:
  device: HyperDeck Studio HD Mini
  firmware: "8.4"
  date: 2026-09-29
```

If a change alters what goes on the wire, update the vectors in the same
commit. The vector format is described in [vectors/README.md](vectors/README.md).

## Commands that can't be undone

Recalling a scene on a live console, overwriting a camera preset or stopping a
recorder mid-take can't be reversed by the operator in time. Give these
commands a `severity: critical` quirk that says what happens. If you can't
confirm the wire format from the documentation or the device, leave the command
out.

## Spec format notes

The format has no conditionals, loops or expressions. A protocol that needs
logic (handshakes, sessions, binary framing) is written as a Rust module
instead; its spec then lists only the models, commands, state and quirks. See
[SPEC.md](SPEC.md).

Braces in templates clash with YAML flow mappings, so quote any value that
contains `{...}` inside `{ }`:

```yaml
send: { address: "/cue/{cue}/start" }     # works
send: { address: /cue/{cue}/start }       # parse error
```

YAML also reads some bare words as other types. A key named `on`, `off`, `yes`
or `no` loads as a boolean, and a bare number loads as an integer. Rename such
parameters and quote numeric keys:

```yaml
params:
  enabled: { type: bool }   # not "on"
codes:
  "104": disk full
```

`tools/validate.py` catches both.

## Checks

```
python tools/validate.py
python tools/make_vectors.py
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test -p meros-integrations --features sennheiser-ew-dx,shure-wireless
cargo build -p meros-integrations
python tools/check_features.py
```

Every spec is an integration, and every integration is a Cargo feature of the
core named after its spec id. Adding a spec means adding its feature:

- add the spec id to its vendor group in `VENDOR_GROUPS` in
  `crates/core/build.rs` (the build fails until it is there), or a new
  `vendor-...` group for a new vendor;
- in `crates/core/Cargo.toml`, add `<spec-id> = []` (listing `dep:...` for an
  optional dependency only it needs), add it to its `vendor-...` group, and a
  new group to `all`. A test checks that the two tables agree;
- compile its native module, native extension or discovery protocol under
  `#[cfg(feature = "<spec-id>")]` (`crates/core/src/modules/mod.rs`), and its
  tests under the same feature;
- run `python tools/check_features.py <spec-id>` to check it builds alone;
- run `python tools/devices_table.py` to add it to [DEVICES.md](DEVICES.md)
  (a native integration also needs its transport described in that script).

## Review checklist

- [ ] `python tools/validate.py` passes
- [ ] Every command is backed by a cited source
- [ ] `ports` lists every port the integration uses, with its default
- [ ] Parameter ranges are the device's limits, not guesses
- [ ] Every parameter and setting has a `label` and a one-sentence `description`
- [ ] Each model's `supports` list is accurate; a model doesn't inherit a
      command from a sibling it doesn't have
- [ ] Verification status matches the recorded vectors
- [ ] Commands that can't be undone are documented or left out
- [ ] No copied code; cited sources have compatible licences

Changes to SPEC.md or the schema must leave every existing spec valid.
