# Contributing

Meros owns this repository and reviews and merges all changes. Pull requests,
issues and corrections from outside Meros are welcome and are held to the same
review checklist below.

## Most useful contributions

1. **Quirks** — device behaviour that is not in the documentation, or that
   contradicts it. This is the hardest information to obtain and the easiest to
   lose.
2. **Verification** — running an existing spec against hardware and recording a
   vector. Most specs here have not been tested against a device.
3. **Corrections** — wrong ranges, misread response formats, addresses absent on
   a particular model.
4. **New device specs.**

## Rules

### Do not copy code

Protocol facts are not copyrightable; implementations are. Contribute behaviour
you observed or facts from manufacturer documentation. Do not translate another
project's source into a spec, particularly from copyleft-licensed projects.

Where a fact originated from reading another implementation, cite it in `source`
and describe the behaviour independently.

### Verification status must match evidence

A new spec starts at `verification: none`. Promotion to `bench` or `field`
requires a vector recorded from the device, submitted in the same pull request.
`tools/validate.py` fails a promotion with no vectors behind it.

Documentation is evidence, not verification. One manufacturer PDF referenced in
this repository specifies two different sample layouts in two sections.

### Destructive commands

Some commands cannot be reversed in time by an operator: scene recall on a live
console, preset overwrite on a camera, transport control on a recorder mid-take.

- If the address is unverified, omit the command rather than including it
  untested. The `behringer-x32` spec omits scene recall for this reason.
- Commands that are included carry a `severity: critical` quirk describing the
  failure mode.

### No control flow

The format has no conditionals, loops or expressions, and will not gain them.
Protocols needing logic use the escape hatch in `SPEC.md`; the spec then carries
only models, quirks and vectors.

### YAML quoting

Parameter substitution uses braces, which collide with YAML flow mappings. A
value containing `{…}` inside flow style must be quoted:

```yaml
send: { address: "/cue/{cue}/start" }     # quoted, parses
send: { address: /cue/{cue}/start }       # parse error
```

Block style needs no quoting as long as the value does not begin with a brace.
`tools/validate.py` catches this, since the file will not parse.

YAML 1.1 also reads some bare words as other types. A parameter named `on`,
`off`, `yes` or `no` loads as a boolean key, and a bare number key such as a
response code loads as an integer. The file parses, but the entry is silently
renamed:

```yaml
params:
  on: { type: bool }        # loads as True: — the parameter "on" does not exist
  enabled: { type: bool }   # use a different name
codes:
  "104": disk full          # quote numeric keys
```

`tools/validate.py` rejects any key that does not load as a string.

## Review checklist

A spec change reaches every consuming product at once, so review is not a
formality.

- [ ] Validates against `schema/device-spec-1.json` (`python tools/validate.py`)
- [ ] Every command has a `source` entry supporting it
- [ ] Parameter ranges are device limits, not assumptions
- [ ] Verification status matches the vectors present
- [ ] Destructive commands are verified or absent
- [ ] Model `supports` lists are accurate; an unsupported capability is omitted
      rather than inherited from a sibling model
- [ ] No copied code; cited sources are licence-compatible

Changes to `SPEC.md` or the schema additionally require that every published
spec still validates, since a format change affects all consumers
simultaneously.

## Vectors

```yaml
spec: shure-wireless
model: slxd
command: get_channel_name
input: { channel: 1 }
expect_wire: "< GET 1 CHAN_NAME >"
device_reply: "< REP 1 CHAN_NAME {Pulpit        } >"
recorded:
  device: Shure SLX-D4
  firmware: "1.3.9"
  date: 2026-09-29
```

A spec change that alters wire format updates its vectors in the same commit.
Format details in [vectors/README.md](vectors/README.md).
