# Meros Device Spec — format v1

A device spec describes how to control one family of third-party hardware or
software. An interpreter reads the spec and drives the device without
device-specific code.

The format has no conditionals, loops, expressions or scripting. Every construct
is a closed vocabulary. Protocols that need logic are out of scope and use the
[escape hatch](#escape-hatch) instead.

## 1. Document shape

```yaml
spec: 1                       # format version, required
id: behringer-x32             # stable slug, required, unique, lowercase-kebab
name: Behringer X32 / Midas M32
vendor: Behringer
category: mixer               # mixer | router | recorder | switcher | camera |
                              # lighting | playback | wireless | display | other
source:                       # provenance of the protocol details, required
  - title: Unofficial X32/M32 OSC Remote Protocol
    author: Patrick-Gilles Maillot
    url: https://…
transport: { … }              # §2
models: [ … ]                 # §3, at least one
commands: { … }               # §4
quirks: [ … ]                 # §6, optional
```

`id` is the consumer-facing identifier. It does not change once published.

## 2. Transports

One transport per spec. Interpreters implement each transport once.

### `line-tcp`

Plaintext over TCP. Four framings cover the devices specced so far.

```yaml
transport:
  type: line-tcp
  port: 2202
  framing: delimited          # delimited | terminated | block | length-prefixed
  open: "< "                  # framing: delimited
  close: " >"
  encoding: ascii             # ascii | utf-8
  timeout_ms: 2000
  reply: expected             # expected | none
  probe: "< GET 1 CHAN_NAME >"   # liveness check; any reply means reachable
```

| Framing | Shape | Example |
|---|---|---|
| `delimited` | Payload wrapped in `open`/`close` | Shure: `< GET 1 CHAN_NAME >` |
| `terminated` | Payload plus `terminator` | Kramer P3000 (`cr`), RossTalk and HyperDeck (`crlf`) |
| `block` | Multi-line payload ended by a blank line | Blackmagic Videohub |
| `length-prefixed` | Payload preceded by a byte count | — |

`terminated` requires `terminator`: `cr`, `crlf` or `lf`.

`reply: none` marks a device that never acknowledges over TCP (RossTalk).
Interpreters report `unverified` for those writes, as with `osc-udp`.

### `osc-udp`

OSC 1.0 over UDP.

```yaml
transport:
  type: osc-udp
  port: 10023
  timeout_ms: 500
  reply: to-source            # to-source | none
  probe: /info
```

`reply: none` marks a device that never acknowledges (QLab, Resolume).
Interpreters report `unverified` for writes to such devices rather than success.

### `osc-tcp`

OSC over TCP, used by ETC Eos.

```yaml
transport:
  type: osc-tcp
  port: 3032
  framing: slip               # slip | length-prefixed
  timeout_ms: 2000
```

### `http`

HTTP GET/POST. Covers REST/JSON devices and CGI devices taking positional query
arguments.

```yaml
transport:
  type: http
  port: 80
  scheme: http                # http | https
  auth: none                  # none | basic | digest | bearer | query-token
  timeout_ms: 4000
  probe: { method: GET, path: /cgi-bin/ptzctrl.cgi, raw_query: "ptzcmd&ptzstop&1&1" }
```

### Settings and connection setup

Per-installation values that are not command parameters — credentials, ports,
workspace passcodes — are declared as `settings` and supplied by the operator
when the device is registered.

```yaml
settings:
  username: { type: string, default: administrator }
  password: { type: string, secret: true, default: admin }
```

`secret: true` marks a value interpreters must not log or echo.

`on_connect` is an ordered list of messages sent once after the socket opens,
before any command. It references settings as `{settings.<name>}`.

```yaml
on_connect:
  - "login {settings.username} {settings.password}"
```

This covers grandMA2's console login and QLab's workspace passcode
(`/connect` with the passcode). It is a fixed sequence, not a handshake:
there is no branching on the response, no retry and no negotiation. Protocols
whose connection setup depends on what the device answers are native cases —
see the [escape hatch](#escape-hatch).

## 3. Models and capabilities

A spec covers a device family. Differences between models are expressed as data.

```yaml
models:
  - id: ulxd
    name: Shure ULX-D
    channels: 4
    supports: [mute, set_gain, get_battery_bars, get_channel_name, flash]
    verification: bench
  - id: slxd
    name: Shure SLX-D
    channels: 2
    supports: [set_gain, get_battery_bars, get_channel_name]
    verification: field
    notes: >
      No mute command exists in the SLX-D command set.
```

`supports` is an allow-list of `commands` keys. An interpreter refuses a command
not listed for the resolved model and reports the reason. Sending an unsupported
command would otherwise return success while the device ignores it.

`verification` values: `none` (documentation only), `bench` (tested against the
device), `field` (running in production). `bench` and `field` require a vector
(§7); `tools/validate.py` enforces this.

## 4. Commands

```yaml
commands:
  mute:
    summary: Mute or unmute a receiver channel
    params:
      channel: { type: int, min: 1, max: 4, required: true }
      muted:   { type: bool, default: true }
    send: "< SET {channel} AUDIO_MUTE {muted:on_off} >"
    expect:
      contains: "REP"
    returns: ack               # ack | value | none
```

### Parameter types

| Type | Constraints |
|---|---|
| `int` | `min`, `max`, `default` |
| `float` | `min`, `max`, `default` |
| `bool` | `default` |
| `enum` | `values`, `default` |
| `string` | `max_length`, `pattern` (RE2-safe), `default` |

Values outside the declared range are rejected before transmission. Interpreters
do not clamp, because a clamped value masks a caller error and produces a
different device state than the caller requested.

### Formatting directives

Substitution is `{param}`, with an optional directive after `:`. The set is
closed.

| Directive | Effect |
|---|---|
| `02d`, `03d`, … | Zero-padded integer: `{channel:02d}` → `07` |
| `on_off` | Boolean → `ON` / `OFF` |
| `bool01` | Boolean → `1` / `0` |
| `bool10` | Boolean → `0` / `1`, inverted |
| `upper`, `lower` | String case |
| `-1`, `+1`, … | Integer offset applied before formatting; combines as `{preset:-1:02d}` |
| `signed` | Integer with an explicit leading sign: `7` → `+7`, `-7` → `-7` |

`bool10` covers flags whose sense is inverted relative to the parameter name,
such as the X32's `mix/on` where `0` is muted.

Offsets exist because several protocols number from zero while operators count
from one: Videohub inputs and outputs, and Panasonic PTZ presets, are all
0-based on the wire. Declaring the offset keeps the operator-facing parameter
1-based without each interpreter reimplementing the conversion.

### OSC commands

```yaml
commands:
  mute_channel:
    params:
      channel: { type: int, min: 1, max: 32, required: true }
      muted:   { type: bool, default: true }
    send:
      address: /ch/{channel:02d}/mix/on
      args: [ { value: "{muted:bool10}", type: int } ]
    returns: none
```

OSC argument types: `int`, `float`, `string`, `blob`.

### Multi-message commands

`send` may be a list, transmitted in order. The command succeeds only if every
message succeeds.

```yaml
commands:
  go:
    summary: Press and release the Go key
    send:
      - { address: /eos/key/go_0, args: [ { value: "1.0", type: float } ] }
      - { address: /eos/key/go_0, args: [ { value: "0.0", type: float } ] }
    returns: none
```

ETC Eos models key presses as a down/up pair; the release is not optional. A
list is an ordered sequence, not control flow — there is no branching or
iteration.

### HTTP commands

```yaml
commands:
  recall_preset:
    params: { preset: { type: int, min: 0, max: 89, required: true } }
    send:
      method: GET
      path: /cgi-bin/ptzctrl.cgi
      raw_query: "ptzcmd&poscall={preset}"
    expect: { status: 200 }
    returns: ack
```

`raw_query` is transmitted without re-encoding, for devices taking positional
`&`-separated arguments. Use `query: {k: v}` for key/value APIs; interpreters
URL-encode that form. A spec sets one or the other, not both.

## 5. Responses

```yaml
expect:
  contains: "REP"           # literal substring
  not_contains: "ERR"       # success is the absence of an error marker
  matches: "^OK (\\d+)$"    # RE2-safe regex; capture group 1 is the value
  status: 200               # HTTP only
  json_path: "$.transport.status"   # HTTP JSON only
  code_range: [200, 299]    # leading numeric response code
```

`not_contains` covers devices that acknowledge by not complaining. Kramer
Protocol 3000 replies `~01@ROUTE 1,2,3 OK` on success and includes `ERR` on
failure, with no single success token to match on.

### Numeric response codes

Devices that answer with a leading status code declare the success range and a
message table, so every interpreter reports the same diagnosis.

```yaml
expect:
  code_range: [200, 299]
codes:
  104: disk full
  105: no disk
  111: remote control disabled
  150: invalid state
```

A code outside `code_range` fails the command. A code present in `codes`
supplies the failure message; one absent from it is reported as an unexpected
response carrying the raw code. This is the Blackmagic HyperDeck shape.

`returns` declares the result type:

| Value | Result |
|---|---|
| `ack` | Boolean. True when `expect` matched |
| `value` | Captured value: regex group 1, or the `json_path` result |
| `none` | No acknowledgement available. Interpreter reports `unverified` |

## 6. Quirks

Device behaviour that is not derivable from the commands above.

```yaml
quirks:
  - models: [slxd]
    severity: critical
    text: >
      SLX-D has no mute command. Setting gain to minimum is not equivalent and
      does not persist across a power cycle.
  - models: [all]
    severity: info
    text: >
      Metered values carry a -120 offset. Values read directly are display
      scaling, not dBFS.
```

`models` lists model ids or the single entry `all`. `severity` is `info`,
`warning` or `critical`.

## 7. Conformance vectors

Vectors record the bytes a correct interpreter produces and, where available,
the device's reply. They live in `vectors/<spec-id>/<command>.yaml`.

```yaml
spec: behringer-x32
command: mute_channel
input: { channel: 7, muted: true }
expect_wire_hex: "2f63682f30372f6d69782f6f6e0000002c690000 00000000"
notes: OSC address /ch/07/mix/on, int arg 0
```

A spec change that alters wire format updates its vectors in the same commit.
Consumers run vectors in CI, so a change that breaks an implementation fails
before release.

See [vectors/README.md](vectors/README.md) for the full vector format.

## Escape hatch

Protocols requiring session state, sequencing or conditional logic are declared
rather than described:

```yaml
spec: 1
id: blackmagic-atem
name: Blackmagic ATEM
implementation: native        # carries no commands
reason: >
  Proprietary UDP with stateful session handshake, per-packet sequencing and
  retransmission.
native:
  crate: meros-device-atem
  bindings: [rust, node-napi, python-cffi, cpp]
```

A `native` spec still carries `models`, `quirks` and vectors, so the shared
knowledge and conformance suite cover it even though the implementation does
not live here.

Known native cases: Blackmagic ATEM, Sennheiser Digital 6000 (subscription
renewal), Ember+ (BER/S101 framing), Dante (no public protocol).
