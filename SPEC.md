# Meros Device Spec — format v1 (draft)

The format is pre-release. It may change without a version bump until the
first tagged release; after that, `spec:` changes on any incompatible change.

A device spec describes how to control one family of third-party hardware or
software. The core's spec engine reads the spec and drives the device without
device-specific code.

The format has no conditionals, loops, expressions or scripting. Every construct
is a closed vocabulary. Protocols that need logic are implemented as
[native modules](#native-modules) instead.

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
state: { … }                  # the state the device reports, optional
telemetry: { … }              # §8, optional
```

`id` is the consumer-facing identifier. It does not change once published.

`state` declares every state path the device reports, with its type, unit and
meaning, keyed by dotted path with `*` for a number such as a channel:

```yaml
state:
  outputs.*.input: { type: int, description: "Input routed to the output, numbered from 1" }
```

## 2. Transports

One transport per spec. The core implements each transport once.

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
  probe: "GET 1 CHAN_NAME"    # liveness check; any reply means reachable
```

| Framing | Shape | Example |
|---|---|---|
| `delimited` | Payload wrapped in `open`/`close` | Shure: `GET 1 CHAN_NAME` is sent as `< GET 1 CHAN_NAME >` |
| `terminated` | Payload plus `terminator` | Kramer P3000 (`cr`), RossTalk and HyperDeck (`crlf`) |
| `block` | Multi-line payload ended by a blank line | Blackmagic Videohub |
| `length-prefixed` | Payload preceded by a byte count | — |

`terminated` requires `terminator`: `cr`, `crlf` or `lf`.

Templates and `probe` hold the payload only; the framing is added when the
message is sent. A `delimited` reply is the text from `open` to `close`; for
the other framings, CR, LF and CRLF all end a received line, whichever the
device uses.

`reply: none` marks a device that never acknowledges over TCP (RossTalk).
The core reports `unverified` for those writes, as with `osc-udp`.

#### Replies

`reply_framing` says how a reply is delimited, which can differ from how a
command is sent:

| Value | Reply ends at | Example |
|---|---|---|
| `line` | The transport terminator or delimiter | Kramer P3000, Shure |
| `block` | A blank line | Blackmagic Videohub |
| `headed-block` | The end of the first line, unless that line ends in `:`, in which case a blank line | Blackmagic HyperDeck: `200 ok`, or `208 transport info:` followed by fields |

It defaults to `block` for `framing: block` and to `line` otherwise.

`reply_match` is an RE2-safe regex separating replies from unsolicited
messages. A device that pushes status at any time (HyperDeck's `5xx` messages,
including its connection banner) would otherwise have that push consumed as
the answer to the pending command. When `reply_match` is set, an inbound message
that does not match it is never taken as a command reply.

```yaml
transport:
  type: line-tcp
  port: 9993
  framing: terminated
  terminator: crlf
  reply_framing: headed-block
  reply_match: "^[12][0-9][0-9] "
```

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
The core reports `unverified` for writes to such devices rather than success.

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

With `auth: basic`, the `username` and `password` settings are sent on every
request. A 401 or 403 on any request is a refusal of the credential, and it is
terminal: pending and later commands fail with `auth`, the connection reports
`unauthorized`, and nothing further is sent until the host opens the device
again. A credential is never retried on a schedule, because repeated failed
logins can lock a device out.

### Settings and connection setup

Per-installation values that are not command parameters — credentials, ports,
workspace passcodes — are declared as `settings` and supplied by the operator
when the device is registered.

```yaml
settings:
  username: { type: string, default: administrator }
  password: { type: string, secret: true, default: admin }
```

`secret: true` marks a value the core never logs or echoes.

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
see [native modules](#native-modules).

A step can be limited to installations that configure a setting:

```yaml
on_connect:
  - when_set: passcode
    send: { address: /connect, args: [ { value: "{settings.passcode}", type: string } ] }
```

The step is sent only when `passcode` is non-empty. This is decided from the
operator's configuration before the socket opens, never from anything the device
sends. QLab needs it: a workspace without a passcode expects no `/connect`, and
repeated wrong passcodes, including an empty one, add a growing delay.

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

`supports` is an allow-list of `commands` keys. The core refuses a command
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
    send: "SET {channel} AUDIO_MUTE {muted:on_off}"
    expect:
      contains: "REP"
    returns: ack               # ack | value | fields | text | none
```

### Parameter types

| Type | Constraints |
|---|---|
| `int` | `min`, `max`, `default` |
| `float` | `min`, `max`, `default` |
| `bool` | `default` |
| `enum` | `values`, `default` |
| `string` | `max_length`, `pattern` (RE2-safe), `default` |

Values outside the declared range are rejected before transmission. The core
does not clamp, because a clamped value masks a caller error and produces a
different device state than the caller requested.

### Templates

Every string in `send`, `on_connect` and `probe` is a template. Substitution is
`{param}` for a command parameter or `{settings.name}` for a setting, optionally
followed by directives after `:`.

**Every substituted value is always present.** A parameter used in a template
must be `required` or have a `default`. A protocol clause that exists only when
a value is given, such as HyperDeck's `record` and `record: name: {name}`,
becomes two commands: `record` and `record_named`. The format has no optional
segments, because an optional segment is a conditional. `tools/validate.py`
enforces this.

Without a directive, values render as follows:

| Type | Rendering |
|---|---|
| `int` | Decimal, `-` for negatives, no leading zeros |
| `bool` | `true` / `false` |
| `enum` | The value exactly as listed in `values` |
| `string` | The value unchanged |
| `float` | No default. A float rendered as text needs a `.Nf` directive. A float that is the whole value of an OSC `float` argument is sent as a number and needs none |

Floats have no default text form because languages disagree on one (`0.1` vs
`0.10000000000000001`, `1` vs `1.0`).

**String values containing a control character (U+0000–U+001F) are rejected**
before transmission. A CR or LF in a label would otherwise end the command and
start another. Rejecting rather than stripping follows the no-clamping rule
above.

Encoding depends on where the value lands:

| Location | Encoding |
|---|---|
| Line/TCP payload, OSC address, OSC string argument | None; text encoded per `encoding` |
| HTTP `path` | Percent-encoded as a path segment (RFC 3986 unreserved characters kept) |
| HTTP `query` value | Percent-encoded as a query value; pairs are sent in the order the spec lists them |
| HTTP `raw_query` | None. Only `int`, `float`, `bool`, `enum`, or a `string` with a `pattern`, may appear here |

### Formatting directives

The directive set is closed.

| Directive | Effect |
|---|---|
| `02d`, `03d`, … | Zero-padded integer: `{channel:02d}` → `07` |
| `on_off` | Boolean → `ON` / `OFF` |
| `bool01` | Boolean → `1` / `0` |
| `bool10` | Boolean → `0` / `1`, inverted |
| `upper`, `lower` | String case |
| `json` | String as a JSON string literal, quotes and escapes included: `{text:json}` → `"Say \"hi\""`. For JSON request bodies |
| `-1`, `+1`, … | Integer offset applied before formatting; combines as `{preset:-1:02d}` |
| `signed` | Integer with an explicit leading sign: `7` → `+7`, `-7` → `-7` |
| `.1f`, `.2f`, … | Float with a fixed number of decimals, rounded half away from zero: `{level:.2f}` → `0.75` |

Offsets apply before formatting, and directives apply left to right.

`bool10` covers flags whose sense is inverted relative to the parameter name,
such as the X32's `mix/on` where `0` is muted.

Offsets exist because several protocols number from zero while operators count
from one: Videohub inputs and outputs, and Panasonic PTZ presets, are all
0-based on the wire. Declaring the offset keeps the operator-facing parameter
1-based without each consumer reimplementing the conversion.

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

A query is a message carrying no arguments. For a device that answers on the
same address, the spec declares the reply address, and the value is the
argument at index `arg` (default 0):

```yaml
commands:
  get_channel_name:
    params:
      channel: { type: int, min: 1, max: 32, required: true }
    send: { address: "/ch/{channel:02d}/config/name" }
    expect: { address: "/ch/{channel:02d}/config/name", arg: 0 }
    returns: value
```

Inbound messages on other addresses are not the reply.

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
`&`-separated arguments. Use `query: {k: v}` for key/value APIs; the core
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
  address: /ch/01/config/name       # OSC only: the reply's address
  arg: 0                    # OSC only: argument returned as the value
```

`not_contains` covers devices that acknowledge by not complaining. Kramer
Protocol 3000 replies `~01@ROUTE 1,2,3 OK` on success and includes `ERR` on
failure, with no single success token to match on.

### Numeric response codes

Devices that answer with a leading status code declare the success range and a
message table, so every failure carries the same diagnosis.

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
| `value` | Captured value: regex group 1, the `json_path` result, or the OSC argument. Requires one of `matches`, `json_path` or `address` |
| `fields` | Map of `key: value` lines from the reply body, split at the first `: `. For HyperDeck-style replies such as `208 transport info:` |
| `text` | The reply body as text: for `headed-block`, the lines after the first; otherwise the whole reply |
| `none` | No acknowledgement available. The core reports `unverified` |

`json_path: "$"` returns the whole JSON body.

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

Vectors record the bytes the core must produce and, where available,
the device's reply. They live in `vectors/<spec-id>/<command>.yaml`.

```yaml
spec: behringer-x32
command: mute_channel
input: { channel: 7, muted: true }
expect_wire_hex: "2f63682f30372f6d69782f6f6e0000002c69000000000000"
notes: OSC address /ch/07/mix/on, int arg 0
```

A spec change that alters wire format updates its vectors in the same commit.
The core's test suite runs every vector, so a change that alters wire format
without updating its vectors fails before release.

See [vectors/README.md](vectors/README.md) for the full vector format.

A telemetry vector, `vectors/<spec-id>/telemetry-<name>.yaml`, states an
inbound message and the state it must produce, and optionally what the core
sends on connecting:

```yaml
spec: blackmagic-videohub
telemetry: routing
inbound: "VIDEO OUTPUT ROUTING:\n0 5\n1 0\n\n"      # inbound_hex for OSC
expect_state: { outputs: { "1": { input: 6 }, "2": { input: 1 } } }
```

Every spec with a `telemetry` section has at least one.

## 8. Telemetry

`telemetry` says how the device's own messages become state. The paths it
writes must be declared in `state` (§1), whose `type` decides how each value
is converted: `int`, `float`, `bool` (from `true` / `false`, or through a
`map`) or `string`. A value that does not convert is not assigned; the core
never guesses.

```yaml
telemetry:
  subscribe:                       # sent after connecting
    send: ["/xremote"]
    every_ms: 9000                 # and again at this interval
  poll:                            # queries; their replies go through the rules
    send: ["transport info"]
    every_ms: 5000                 # omit to ask once, on connecting
  updates:
    - header: "^VIDEO OUTPUT ROUTING:$"      # a block's first line
      each_line: "^(\\d+) (\\d+)$"             # applied to every following line
      state: { "outputs.{1:+1}.input": "{2:+1}" }
```

`send` items are the same as a command's. They go through the command queue,
one at a time, wherever a reply is expected: `subscribe` items on a line
transport whose device answers, and `poll` items (queries) on any transport
that answers, including OSC, where each query waits for the reply on its own
address. A long poll list, such as every channel's mute, fader and name, is
therefore paced by the device's replies. Commands go ahead of queued telemetry,
so an operator never waits for a poll to finish. Other items are sent straight
away.

Every inbound message is offered to every rule: pushed changes and replies to
commands alike, since a reply to a query carries the same data. A rule is one
of:

| Rule | Matches | Captures |
|---|---|---|
| `match` | The whole message, by regex | `{1}`, `{2}`, … |
| `header` + `each_line` | A block whose first line matches `header`; `each_line` is applied to each following line | per line: `{1}`, `{2}`, … |
| `header` + `fields` | A block whose first line matches `header`; the following lines are `name: value` | `fields` maps each name to a state path |
| `address` | An OSC message whose address matches | `{1}`, … from the address; `{arg0}`, `{arg1}`, … the arguments |

`state` maps a path template to a value template. Both use the template rules
of §4: a numeric capture is an integer, so `{1:+1}` converts a 0-based wire
number to 1-based. A value in the device's own words is converted with a map,
and a wire value the map does not list is not assigned:

```yaml
    - address: "^/ch/(\\d\\d)/mix/on$"
      state: { "channels.{1}.mute": { value: "{arg0}", map: { "0": true, "1": false } } }
    - header: "^[25]08 transport info:$"
      fields:
        status: transport.status
        single clip: { path: transport.single_clip, map: { "true": true, "false": false } }
```

Over HTTP, `poll` requests and command requests alike have their replies
offered to `path` rules, which match the request's path and query:

| Rule | Matches | Captures |
|---|---|---|
| `path` + `json` | A JSON reply | `json` names values by JSON path (`$`, `$.a.b`) |
| `path` + `json` + `json_each` | A JSON reply holding an array at `json_each` | once per element; `json` paths relative to it |
| `path` + `xml_each` | An XML reply | once per element of that name; its attributes |

```yaml
    - path: "^/v1/timers/current$"
      json_each: "$"
      json: { uuid: "$.id.uuid", time: "$.time" }
      state: { "timers.{uuid}.time": "{time}" }
    - path: "^/v1/dictionary\\?key=tally$"
      xml_each: column
      state:
        "tally.{name}.program": { value: "{on_pgm}", map: { "true": true, "false": false } }
```

A telemetry vector for HTTP gives `inbound_http: { path, body }` in place of
`inbound`.

## Native modules

Protocols requiring session state, sequencing or logic that depends on what the
device replies are implemented as Rust modules in the core, not as data. The
format stays free of control flow; the logic lives in reviewed, tested code
instead.

A native device still has a spec. It carries everything except wire behaviour:
models, commands with their parameters and return types, settings, quirks and
sources. That keeps the catalogue identical for both kinds of device, and gives
the Rust module's parameter validation the same single source as spec-driven
commands.

```yaml
spec: 1
id: blackmagic-atem
name: Blackmagic ATEM
vendor: Blackmagic Design
category: switcher
source:
  - title: …
implementation: native
reason: >
  Proprietary UDP with stateful session handshake, per-packet sequencing and
  retransmission.
models:
  - id: atem-mini
    name: ATEM Mini
    supports: [cut]
    verification: none
commands:
  cut:
    summary: Cut the preview source to program
    params:
      me: { type: int, min: 1, max: 4, default: 1 }
    returns: ack
```

A native spec's commands have no `send`, `expect` or `transport`; the module
defines those. `reason` says why the protocol cannot be expressed as data.
Vectors apply to native modules exactly as to spec-driven ones.

### Native extensions

A spec-driven device can name one native `extension` for a single thing the
format cannot express, while its commands, queries and telemetry rules stay in
the spec:

```yaml
extension: panasonic-update-notification
```

The extension is a small Rust module that wraps the spec engine. The set is
closed and each is named in the spec, so it is never hidden:

| Extension | Adds |
|---|---|
| `panasonic-update-notification` | Panasonic AW-series cameras' update notifications: registers a local TCP port with the camera (`/cgi-bin/event?connect=start`), receives the changed settings it pushes there, and passes each one, a response text such as `p1`, to the spec's telemetry rules. Registers again when the camera's 60-second version notices stop, and unregisters on closing |
