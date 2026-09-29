# Conformance vectors

A vector records the bytes a correct interpreter produces for a given command
and input, and where the device answers, its reply. Every language
implementation tests against the same vectors.

```
vectors/<spec-id>/<command>.yaml
```

## Purpose

Specs define intended behaviour; vectors record observed behaviour. Independent
implementations agreeing with each other proves nothing if they share a misread
of the documentation. Agreement with a recorded transcript does.

Vectors also gate spec changes. A fix reaches every consumer at once, so
consumers run vectors in CI to catch a wire-format change before release.

## Format

```yaml
spec: shure-wireless
model: slxd                  # optional; omit if the vector holds for all models
command: get_channel_name
input: { channel: 1 }
expect_wire: "< GET 1 CHAN_NAME >"        # text protocols
# expect_wire_hex: "2f63682f…"            # binary and OSC, lowercase hex
device_reply: "< REP 1 CHAN_NAME {Pulpit        } >"
expect_value: "Pulpit"                     # for commands with returns: value
recorded:
  device: Shure SLX-D4
  firmware: "1.3.9"
  date: 2026-09-29
  by: "@someone"
```

`recorded` is present only for vectors captured from hardware. Without it the
vector is an expectation: it locks wire format across implementations but does
not promote a model's verification status.

## Rules

- A spec change altering wire format updates its vectors in the same commit.
- Only a vector with `recorded` promotes a model to `bench` or `field`.
- Include failure cases. A device's response to an out-of-range channel is as
  useful as the success case.
- Binary and OSC payloads are lowercase hex with no separators; OSC padding
  errors are not visible in other representations.

## Status

Empty. No vectors have been recorded, which is why every published spec reads
`verification: none`. Recording transcripts against an X32 and a Shure receiver
is the next step.
