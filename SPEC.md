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
ports: [ … ]                  # §2, every port it uses, required
models: [ … ]                 # §3, at least one
commands: { … }               # §4
quirks: [ … ]                 # §6, optional
state: { … }                  # the state the device reports, optional
telemetry: { … }              # §8, optional
conversions: { … }            # §4, named value conversions, optional
```

`id` is the consumer-facing identifier. It does not change once published.

A spec gives each mapping key once, at every depth, and defines each anchor
once: YAML loaders disagree on which of two values they keep, and
`tools/validate.py` refuses both.

`state` declares every state path the device reports, with its type, unit and
meaning, keyed by dotted path with `*` for a number such as a channel:

```yaml
state:
  outputs.*.input: { type: int, description: "Input routed to the output, numbered from 1" }
```

## 2. Transports

One transport per spec. The core implements each transport once. A device that
takes commands one way and pushes state on a websocket (Resolume: REST for
commands, a websocket for changes) keeps its command transport and adds the
websocket under `telemetry.websocket` (§8).

### `line-tcp`

Plaintext over TCP. Five framings cover the devices specced so far.

```yaml
transport:
  type: line-tcp
  port: 2202
  framing: delimited          # delimited | terminated | block | length-prefixed | json
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
| `json` | The payload as it is, nothing added: each message is one JSON value | Barco Pulse's JSON-RPC |

`terminated` requires `terminator`: `cr`, `crlf` or `lf`.

Templates and `probe` hold the payload only; the framing is added when the
message is sent. A `delimited` reply is the text from `open` to `close`. With
no `open`, `close` is a separator after each message and is dropped from it
(PIXERA ends each JSON message with `0xPX`). For
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
| `terminated` | The first of `reply_terminators`, kept as part of the message; no line ending is needed | Roland V-60HD: `VFL:a;`, and a bare ACK byte (`reply_terminators: [";", "\u0006"]`) |
| `fenced` | The end of the line, unless the line matches `reply_fence`: then the next such line after other text, the lines between being one message | Turtle AV controllers: a status block between lines of `=` (`reply_fence: "^=+\\s*$"`) |
| `json` | The end of a complete top-level JSON object or array | Barco Pulse: bare JSON-RPC objects back to back, compact or spread over lines |

It defaults to `block` for `framing: block`, to `json` for `framing: json`, and
to `line` otherwise.

With `json`, a message runs from an opening `{` or `[` to the bracket that
closes it, brackets inside JSON strings (and escaped quotes) not counting, so
neither a line ending nor a separator is needed and a value laid out over
several lines is still one message. Whatever arrives between values
(whitespace, stray bytes) is no message. Each message is JSON, so it goes to
the `json_match` rules (§8) as well as the text rules.

With `fenced`, a fence line opens a block and the next fence line after any
other text closes it, so two fences in a row open one block; the fences, and
the blank lines at the block's start and end, are not part of the message.
The block's first line is what `header` rules (§8) match, and its other
lines what `each_line` reads, as for a `block` reply.

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

Some devices answer with the same message they push when a value changes
(Shure's `< REP 01 AUDIO_MUTE ON >`), so `reply_match` alone cannot tell the
answer from a change pushed meanwhile. A command then names text its reply
holds, as a template:

```yaml
commands:
  get_mute:
    params: { channel: { type: int, min: 0, max: 28, required: true } }
    send: "GET {channel:02d} AUDIO_MUTE"
    expect:
      reply_contains: "REP {channel:02d} AUDIO_MUTE "
      matches: "AUDIO_MUTE (ON|OFF)"
    returns: value
```

The reply is the first message holding the rendered text; other messages
are not the reply, and still go to the telemetry rules. As with `reply_json`
(§2, `ws`), a command whose reply never comes times out without the stream
being reset, since a late reply names what it answers.

A device that answers one message with several replies (an Evertz Quartz
router answers a name write with the name update, then an acknowledge) gives
their number as `expect.replies`. The command then takes that many replies
for each message, in order, each one a reply as above (the first holding
`reply_contains`, when given, and the rest the next replies); `expect` checks
them all, joined by line breaks, so `(?m)` lets a `matches` regex find one of
them. Every reply is also offered to the telemetry rules. A reply matching
the transport's `error_match` ends the wait at once:

```yaml
commands:
  set_destination_name:
    send: ".WD{destination},{name}"
    expect: { replies: 2, matches: '(?m)^\.A' }
    returns: ack
```

A device whose error answer names nothing (Shure's `< REP ERR >`) declares it
as the transport's `error_match`, a regex: such a message is taken as the
reply of a command waiting with `reply_contains`, which then fails on its
`expect`.

#### TLS

A device that serves its line protocol over TLS (Barco Pulse's JSON-RPC on
9091, its plain port 9090 being turned off on current firmware) sets
`scheme: tls`, or names an enum setting of `tcp` and `tls` for a device
the operator can switch, with `tls_port` the default port when it chooses
`tls`, as `https_port` is for HTTP:

```yaml
transport:
  type: line-tcp
  port: 9090
  scheme: { setting: scheme }
  tls_port: 9091
  accept_invalid_certs: true      # a self-signed certificate
settings:
  scheme: { type: enum, values: [tcp, tls], default: tls, label: Connection, description: "…" }
ports:
  - { port: 9091, protocol: tls, role: control, when: "scheme tls" }
  - { port: 9090, protocol: tcp, role: control, when: "scheme tcp" }
```

The stream is TLS 1.2 or later, its certificate checked against the same
public roots as `https`, and against the host name the device was opened
with, or its address; `accept_invalid_certs: true` takes a device's
self-signed certificate (the link is encrypted, the device not
authenticated). Everything above the stream (framing, replies, telemetry)
is as over plain TCP. A handshake that fails is a connection that fails,
retried with backoff; its reason in the log starts `tls:`. Changing the
setting with `update_settings` opens the new link at once. TLS is the
network link's: with a `serial` block (below), link serial opens the
serial port as it is, and the network entries in `ports` name both choices
(`when: "link tcp, scheme tls"`). `tools/validate.py` checks that the setting is an enum
of `tcp` and `tls`, that `tls_port` has a `tls` control entry in `ports`,
and that `accept_invalid_certs` goes with a TLS scheme.

### `line-serial`

Text over a serial port of the host running the core: RS-232, RS-422, or
a USB serial adapter (Roland's V-1200HD, which has no network control).
Everything of `line-tcp` applies (its framings, `reply_framing`,
`reply_match`, `error_match`, `probe`, `on_connect` with `after_prompt`,
and the telemetry rules), except `port` and `endpoints`: a serial port has
no number and reaches no HTTP server.

```yaml
transport:
  type: line-serial
  framing: delimited
  open: "\u0002"
  close: ";"
  reply_framing: terminated
  reply_terminators: [";", "\u0006"]
settings:
  serial_port:  { type: string, required: true, max_length: 256, label: Serial port,
                  description: "The host's serial port: COM3 on Windows, /dev/ttyUSB0 on Linux, /dev/cu.usbserial-1420 on macOS." }
  baud_rate:    { type: enum, values: ["9600", "38400"], default: "38400", label: Baud rate, description: "…" }
  data_bits:    { type: int, min: 8, max: 8, default: 8, label: Data bits, description: "…" }
  parity:       { type: enum, values: [none], default: none, label: Parity, description: "…" }
  stop_bits:    { type: int, min: 1, max: 1, default: 1, label: Stop bits, description: "…" }
  flow_control: { type: enum, values: [none, xon-xoff], default: none, label: Flow control, description: "…" }
ports:
  - { port: null, protocol: serial, role: control, setting: serial_port, note: "RS-232, D-sub 9-pin" }
```

The port and its line settings are settings with these names, so every
serial device is configured alike: `serial_port` (required, no default),
`baud_rate` (an `int`, or an `enum` of digits for a device that offers a
few rates), `data_bits` (5 to 8), `parity` (`none`, `odd`, `even`),
`stop_bits` (1 or 2) and `flow_control` (`none`, `xon-xoff`, `rts-cts`).
Each line setting is declared with the device's documented default, and
narrowed to the values the device allows; `tools/validate.py` checks them.
Changing any of them with `update_settings` opens the port again.

The port is opened when the device is, and is the device's connection: a
port that cannot be opened (absent, or held by another program) or that
fails while open (a USB adapter unplugged) is reported `disconnected`,
commands in flight fail with `transport`, and it is opened again after 1 s,
doubling to 30 s, as a TCP connection is reconnected. A serial line gives no
sign of the device itself, so `connected` means the port is open; the
device's own presence is what `probe` and its replies show. A device on a
serial port needs no host: `host` may be left empty when it is opened (a
`serial` entry in `ports` allows it). Commands-only opening and latency are
as for any spec-driven device.

Serial ports are opened on Windows, macOS and Linux (and the other Unix
systems the `serial2` crate supports). Phones have no serial port an app can
open, so on Android and iOS the port fails to open with a reason saying so,
and the device stays `disconnected` (docs/MOBILE.md).

Every delivery lists the host's serial ports, for a consumer offering a
choice of `serial_port`: `Core::serial_ports` in Rust, `mi_serial_ports` in
C, `serialPorts()` in Node, `serial_ports()` in Python and
`GET /v1/serial_ports` in the HTTP service. Each port has its `name`, the
value `serial_port` takes; the port library gives no description or USB
ids. The list is empty on Android and iOS.

#### The same protocol on a serial port

Many devices speak their network protocol unchanged on an RS-232 or RS-422
port as well (Extron SIS, Kramer Protocol 3000, Lightware LW2 and LW3, most
Roland switchers), and some models of a family have only the serial port. A
`line-tcp` transport then adds a `serial` block, and one spec serves the
device either way: the framing, replies, probe, `on_connect`, telemetry,
polls and commands are shared, and only what is opened differs.

```yaml
transport:
  type: line-tcp
  port: 23
  framing: terminated
  terminator: cr
  serial:
    serial_only: [in1604-dtp, in1604-hd]   # optional
    models: [in1806-1808, in1604-dtp, in1604-hd]   # optional
    baud_rate: { default: 9600, values: [9600, 19200, 38400, 115200] }
    data_bits: 8
    parity: none
    stop_bits: 1
    flow_control: none
ports:
  - { port: 23, protocol: tcp, role: control, when: "link tcp" }
  - { port: null, protocol: serial, role: control, setting: serial_port, when: "link serial",
      note: "Remote RS-232 port, 3-pole captive screw" }
```

The block stands for settings the spec does not declare itself (the core
and `tools/validate.py` add them, and the catalogue lists them like any
other):

- `link`, `tcp` or `serial`: the consumer's choice for each device. `tcp`
  reaches the device at its host and port, `serial` opens `serial_port` and
  needs no host. Its default is the block's `link`, `tcp` unless given.
  With `serial_only`, a list of the models reached only on a serial port
  (RS-232-only units), `link` has no default: left out, those models use
  `serial` and the others `tcp`.
- `serial_port`, optional, and required by opening with link serial.
- `baud_rate`, `data_bits`, `parity`, `stop_bits` and `flow_control`, as
  for `line-serial`, each from the block: the vendor's one documented value
  (a fixed setting), or `{ default, values }` when the device's port can be
  set to others (`baud_rate` is then an enum of digits, `data_bits` and
  `stop_bits` an int range without gaps). All five are given, as the vendor
  documents them; a value the vendor leaves undocumented (flow control,
  often) is `none` and a quirk with a VERIFICATION.md item.

`models`, when given, lists the models that have the serial port; opening
another model with link serial fails with `invalid_settings`, as does link
serial without `serial_port`. Link tcp without a host fails as for any
network device. Changing `link` or a line setting with `update_settings`
opens the new link at once, as for `line-serial`; on Android and iOS link
serial fails to open as `line-serial` does.

A device with HTTP `endpoints` (§2, Endpoints) beside its line protocol
(Barco Pulse's file endpoints on its web server) keeps them on link serial
too: they are reached at the host the device was opened with, over the
network, while the line protocol goes over the serial port. Opened on link
serial without a host, it reaches no HTTP server, and a command or poll
naming an endpoint fails with `invalid_params` saying so. The same holds
for TLS (§2, `line-tcp`, TLS): it is the network link's.

The ports list both links, the transport's port with `when: "link tcp"` and
a `serial` control entry with `when: "link serial"`. A step of `on_connect`
can name the link it is for, `link: tcp` or `link: serial`, when the vendor
documents a difference between them, such as a login asked for on the
network only:

```yaml
on_connect:
  - { when_set: password, link: tcp, after_prompt: "Password:", send: "{settings.password}" }
```

Some vendors frame the same commands differently on the serial port: Roland
wants STX (02H) before each command on RS-232 and no line ending after its
`;`, and answers with an ACK byte (06H) where the network gets `ACK` and a
line feed. The block's `overrides` give the transport keys that differ
there: `framing`, `open`, `close`, `terminator`, `reply_framing`,
`reply_terminators`, `reply_fence`, `reply_match` and `error_match`. It is a
mapping for every model, or a list of mappings each for the `models` it
names (every model when it names none); every entry for the device's model
is laid over the transport, in order, when the link is serial, and the TCP
link keeps the transport as it is. Templates, `expect` and the telemetry
rules are shared, so a reply that differs (`ACK` or the ACK byte) is matched
by a regex accepting both.

```yaml
transport:
  type: line-tcp
  port: 8023
  framing: terminated
  terminator: crlf
  reply_match: 'ACK;|ERR:[0-9]+;'
  serial:
    models: [v-160hd, v-80hd]
    baud_rate: { default: 115200, values: [9600, 38400, 115200] }
    data_bits: 8
    parity: none
    stop_bits: 1
    flow_control: { default: xon-xoff, values: [xon-xoff, none] }
    overrides:
      - { framing: delimited, close: "", reply_framing: terminated,
          reply_terminators: [";", "\u0006"] }
      - { models: [v-160hd], open: "\u0002" }
```

`framing: delimited` with an empty `close` sends the payload with only `open`
before it (or as it is, with no `open`); it needs a `reply_framing`, since
no marker would end a reply. This holds for any line transport, not only in
`overrides`.

A vector plays a command or a telemetry message on the serial port by giving
`link: serial` in its `settings` (and on TCP otherwise, unless `serial_only`
names its model); the vector runner checks that the link opened is the one
chosen.

### `line-udp`

Text messages over UDP, one message per datagram (ChamSys MagicQ's remote
protocol, XPression's RossTalk over UDP in `ross-xpression-udp`).

```yaml
transport:
  type: line-udp
  port: 6553
  terminator: none            # none | cr | lf | crlf, appended to each message
  reply: none                 # to-source | none
```

Templates and directives are as for `line-tcp`. With `reply: to-source`, each
datagram from the device is one reply line, its trailing line ending removed;
`listen_port` works as for `osc-udp`. `reply_match` works as for `line-tcp`: a
device that pushes changes to the last sender (Symetrix Jupiter's
`#nnnnn=vvvvv`) names what a reply looks like, and other datagrams go only to
the telemetry rules.

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

Some devices send their feedback to a destination configured on the device,
not back to the sender (grandMA3's Object Playback Feedback). `listen_port` receives on a fixed local
port instead of an ephemeral one; the port is shared by every device that
uses it, and each datagram goes to the device it came from:

```yaml
transport:
  type: osc-udp
  port: 8000
  reply: none
  listen_port: { setting: feedback_port }   # or a number
settings:
  feedback_port: { type: int, min: 1, max: 65535 }
```

A setting left empty means no fixed port, and no feedback.

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

HTTP GET, POST, PUT, PATCH and DELETE. Covers REST/JSON devices, JSON-RPC over
HTTP and CGI devices taking positional query arguments.

```yaml
transport:
  type: http
  port: 80
  scheme: http                # http | https
  auth: none                  # none | basic | digest | bearer | header | query-token | oauth2
  timeout_ms: 4000
  probe: { method: GET, path: /cgi-bin/ptzctrl.cgi, raw_query: "ptzcmd&ptzstop&1&1" }
```

With `auth: bearer`, the `token` setting is sent as `Authorization: Bearer
<token>` on every request (none while it is empty). With `auth: header`, it is
sent as it is in the header `auth_header` names, for a device that takes a key
in a header of its own (mimoLive's `auth_header: X-MimoLive-Password-SHA256`),
again none while it is empty. With `auth: basic`, the
`username` and `password` settings are sent on every request, and a Digest challenge in reply is answered as below, for devices
that can be set to either. A device that answers 403 for other reasons (a
setting that cannot be changed in the current state) narrows the refusal to
`refusal_status: [401]`. With `auth: digest`, requests go without credentials until the device
answers 401 with a Digest challenge (RFC 7616: MD5, MD5-sess, SHA-256 or
SHA-256-sess, with `qop=auth` or none); the request is then repeated once with
the answer, and later requests to the same device answer the same challenge
until it is replaced. A challenge the core cannot answer leaves the 401 as it
is. A 401 or 403 on any request is a refusal of the credential, and it is
terminal: pending and later commands fail with `auth`, the connection reports
`unauthorized`, and nothing further is sent until the host opens the device
again or changes its settings (`update_settings`). A credential is never retried on a schedule, because repeated failed
logins can lock a device out.

A service that names a refused token in its error body rather than its
status lists JSON paths and regexes in `refusal_json`: an error answer (4xx or
5xx) whose JSON body matches every one is a refusal too, whatever its status.
Meta's Graph API answers an invalid or expired token with HTTP 400 and
`error.code` 190, and other errors (a bad parameter, a missing permission, a
rate limit) with the same status and other codes:

```yaml
transport:
  type: http
  auth: bearer
  refusal_status: [401]
  refusal_json: { "$.error.code": "^190$" }
```

A service that answers a refused token and other errors with the same
status names what the refusal's body holds with `refusal_status_body`: a
status of `refusal_status` is then a refusal only when its body matches too,
and any other answer with that status fails only the request it answers (no
token refresh, no `unauthorized`). `json` maps JSON paths to regexes, all of
which must match; `match` is a regex over the body text; with both, both
must. Twitch answers a revoked or expired token with 401 and "Invalid OAuth
token", and a token lacking a command's scope with 401 too:

```yaml
transport:
  type: http
  auth: oauth2
  refusal_status: [401]
  refusal_status_body: { json: { "$.message": "(?i)^invalid oauth token$" } }
```

It applies to answers the core reads a body of: command, poll and probe
requests. The opening of a push websocket or event stream has no body to
read, and its refusal statuses stand alone, as without it.

A service that wants a header of its own on every request, beside the
credential, lists it in `headers`, each name mapped to a template over
settings. Twitch's Helix API takes the application's client id in
`Client-Id` with every bearer token:

```yaml
transport:
  type: http
  auth: oauth2
  headers:
    Client-Id: "{settings.client_id}"
```

The headers go on every HTTP request (commands, polls, the probe, and a
request repeated after a token refresh), on the push websocket's opening
request and on the event stream, rendered from the settings current at the
time, so `update_settings` changes them too. A header that renders empty, or
names a setting with no value, is left out. `Authorization`, `Content-Type` and `Host` are the core's and may
not be named. Only `{settings.*}` placeholders apply: there are no command
parameters in a transport.

A service taking more than one kind of credential names a setting instead,
as for `scheme` below: `auth: { setting: auth }` with `auth: { type: enum,
values: [basic, oauth2] }`. Planning Center takes a personal access token as
Basic and OAuth tokens as `oauth2`.

#### OAuth 2 (`auth: oauth2`)

A cloud service signed in to with OAuth 2 (YouTube, Planning Center) takes
`auth: oauth2` and an `oauth` block naming its token endpoint:

```yaml
transport:
  type: http
  port: 443
  scheme: https
  auth: oauth2
  oauth:
    token_url: https://oauth2.googleapis.com/token
    refresh_ahead_s: 300        # default 300
    client_auth: body           # body (default) | basic
  refusal_status: [401]
```

The consumer owns the credential: it registers its own application with the
service (its client id and secret), runs the browser sign-in itself, and
passes the tokens as settings. The core never runs a sign-in and persists
nothing. The spec declares these settings, none of them required:

| Setting | Type | Meaning |
|---|---|---|
| `client_id` | string | The consumer's client id, sent with each refresh. |
| `client_secret` | string, secret | Sent with each refresh only when set: a public client (an installed app, a Twitch public client) refreshes with its id alone, and the field is then left out of the request entirely. |
| `refresh_token` | string, secret | Keeps the access token fresh. Without it the access token is a plain bearer token, used until refused. |
| `access_token` | string, secret | Sent as `Authorization: Bearer`. May be empty when there is a refresh token: the core refreshes before the first request. |
| `expires_at` | int | When the access token expires, in Unix seconds (whole seconds since 1970-01-01 UTC). Unknown when absent: the token is then used until refused. |
| `token_url` | string | Replaces the spec's `token_url` when set: the consumer's own token proxy, for a product that cannot ship its client secret (an open-source one) and adds it server-side, as OBS does for its own sign-ins. `https`, or `http` only to this machine (`localhost`, `127.0.0.1`, `[::1]`). |

Requests carry the access token as Bearer. When there is no access token, or
it expires within `refresh_ahead_s` (at half its life for a token living
shorter than twice that), the core first refreshes it with the refresh token
grant (RFC 6749 §6): a form-encoded POST of `grant_type=refresh_token` and
`refresh_token` to the token endpoint, with the client per `client_auth`:
`client_id` and `client_secret` in the body, or HTTP Basic with the two
form-encoded (§2.3.1). One refresh is in flight per device, and every request
waits behind it. The answer's `access_token` and `expires_in` give the new
token and its expiry; a `refresh_token` in it replaces the old one, which the
service may already have invalidated (rotation).

Whenever the core obtains new tokens it reports them in a `credentials` event
(see [Events](#events)), and keeps them in memory only: **the consumer must
persist them**, and pass the latest when it next opens the device.

A 401 from the API (one of `refusal_status`) refreshes the token and sends
the request once more; a second refusal of the same request is the terminal
`unauthorized` above. A refresh the token endpoint refuses (`invalid_grant`,
or HTTP 400 or 401: a refresh token expired or revoked, a wrong client) is
terminal too. A token endpoint that cannot be reached, or answers otherwise
(a 5xx, a 429), is an ordinary failure: the device reports `disconnected`,
commands waiting fail with `transport`, and the refresh is tried again with
backoff (1 s doubling to 60 s), so a network outage does not need a new
sign-in. After a terminal refusal the consumer signs the user in again and
passes the new tokens with `update_settings` (§2, Settings), which lets the
device go again without reopening it. The token endpoint's answer is never
offered to the telemetry rules, and no token or secret is ever written to a
log event.

A service that requires the application to check its token on a schedule
names its validation endpoint in `oauth.validate`. Twitch requires a
validation when the application starts and every hour after:

```yaml
  oauth:
    token_url: https://id.twitch.tv/oauth2/token
    validate: { url: https://id.twitch.tv/oauth2/validate, every_s: 3600, scheme: OAuth }
```

The core sends a GET there with `Authorization: <scheme> <access token>`
(`scheme` defaults to `Bearer`) and no other header of the device: once the
device opens (after the first refresh, when there is no access token yet),
after every refresh and every `update_settings`, then every `every_s` (default
3600) while the device is open. It is a requirement of the service, not
telemetry: it happens whether or not the device is monitored. The URL is on
any host, and is checked as `token_url` is (`https`, or `http` only to this
machine). Only the answer's status is read; its body (the token's client,
user and scopes) is never offered to the rules, kept in state or logged.

| Answer | What the core does |
|---|---|
| 2xx | Validated again after `every_s`. |
| 401, with a refresh token | The token is refreshed once and the new one validated at once. A 401 for the new token too, or a refresh the token endpoint refuses, is the terminal `unauthorized`. |
| 401, without a refresh token | The terminal `unauthorized` at once: the token was revoked and nothing can renew it. |
| Anything else, or no answer | Logged, and tried again with backoff (1 s doubling to 60 s). The connection is not changed: the API itself may be working. |

An API request refused with 401 while a refresh is already in flight waits
for that refresh and is sent once more with the new token, as when it starts
one itself.

Where the operator chooses HTTP or HTTPS on the device, `scheme` names a
setting instead: `scheme: { setting: scheme }` with `scheme: { type: enum,
values: [http, https], default: http }`. A device serving HTTPS on another
port declares it as `https_port` (Magewell: `port: 80`, `https_port: 443`),
the default when the setting chooses `https`; a port given when opening the
device still wins. Devices that serve HTTPS with a
self-signed certificate declare `accept_invalid_certs: true`: the connection is
encrypted, but the device's identity is not checked.

#### Sessions

A device that answers a login with a session, which every later request
must carry, declares `transport.session`. Magewell's Pro Convert answers its
login with the session ID in a `Set-Cookie` header only, ends the session on
a restart, and says so with status 37:

```yaml
transport:
  type: http
  session:
    login: { method: GET, path: /mwapi,
             query: { method: login, id: "{settings.username}", pass: "{settings.password:md5}" } }
    capture: { cookie: "*" }         # or { cookie: sid }, or { header: X-Session }
    relogin: { status: [401], json: { "$.status": "^37$" } }
    refused: { status: [401, 403], json: { "$.status": "^(36|16)$" } }
```

Before the first request that needs a session, the core sends `login` (a
request like a command's, a template over settings) and takes the session
from its answer: with `capture: { cookie: name }` the named cookie of its
`Set-Cookie` headers, with `"*"` (the default) every cookie, sent back as
`Cookie: name=value; ...`; with `capture: { header: name }` that header's
value, sent back in a header of the same name. Every later request carries
it, except the probe and a request marked `session: false` (Magewell's ping),
which need none and never log in. Requests go one at a time, so nothing is
sent while the login is answered.

| The answer | What the core does |
|---|---|
| To the login, matching `refused` (any of its statuses, or a JSON body matching every one of its JSON paths) | The terminal refusal of the credential (§2, HTTP): commands fail with `auth`, the device reports `unauthorized`, nothing is sent until the settings change. |
| To the login, 2xx with the session | The session is held; the queue goes on. |
| To the login, anything else, or no answer | Logged; commands waiting for a session fail. The next request needing one logs in again. |
| To any other request, matching `relogin` | The session is dropped, the core logs in again and sends that request once more. A second match for the same request is its answer. |

A device whose login the operator can turn off (AJA KUMO's User
Authentication, off by default) names the setting that says it is on with
`when_set`: while that setting is empty or false, nothing logs in and
requests go without a session; once it is set (or true), the next request
needing a session logs in first. It is decided from the settings current at
the time, so `update_settings` turns it on or off:

```yaml
    session:
      when_set: user_authentication     # a bool setting
      login: { method: POST, path: /authenticator/login, body: "password_provided={settings.password:url}",
               content_type: application/x-www-form-urlencoded }
      capture: { cookie: session }
```

The session is a secret: the login's answer is never offered to the
telemetry rules, and the session is never logged or kept in state.
`update_settings` drops it, so the next request logs in with the new
credential. A login that is the same for every model is one request; a list
names each request's `models` (§4, "Messages per model").

A device opened by host name rather than address keeps the name in its HTTP,
websocket and event-stream URLs, so the certificate is checked against the
name and `Host` carries it. A cloud API is opened that way: Planning Center's
`api.planningcenteronline.com` or YouTube's `www.googleapis.com` on port 443.

#### Endpoints

A device serving a second HTTP API on another port (MediaMTX's playback
server on 9996 beside its Control API on 9997; WATCHOUT 7's node management
on 3017 beside its control API on 3019) names it under `endpoints`, and a
request item sends to it with `endpoint`:

```yaml
transport:
  type: http
  port: 9997
  endpoints:
    playback:
      port: { setting: playback_port }       # or a number
      scheme: { setting: playback_scheme }   # http | https | {setting: name}
      auth: { setting: playback_auth }       # inherit | none | {setting: name}
commands:
  list_playback_spans:
    params: { path: { type: string, required: true } }
    send: { endpoint: playback, method: GET, path: /list, query: { path: "{path}" } }
```

A request on an endpoint goes to the same host as every other request (the
host name the device was opened with, so TLS checks the certificate against
it), on the endpoint's port and scheme. `scheme` defaults to the transport's,
and `accept_invalid_certs` is the transport's. `port` is a number or an
integer setting; the port given when opening the device changes only the
transport's own port, never an endpoint's. With `auth: inherit` (the default)
the transport's credential (`auth`, its Digest answer, an OAuth token and its
refresh) and `headers` go with each request, and a refusal of them is the
terminal refusal above, as on the transport's port: it is the same credential.
With `auth: none` neither goes, and a 401 or 403 is that request's answer
only. An `auth` setting is an enum of `inherit` and `none`, for a server whose
access the operator configures apart from the main one.

`endpoint` may be named by command messages, telemetry `poll`, `subscribe` and
`then_send` items and `on_connect` steps. It may not be named by the `probe`
or a session's `login`: liveness, latency and the session belong to the
transport's own port. So an endpoint's request never carries the session
(`transport.session`), never logs in first and is never answered with a fresh
login; its answers are not the device's round-trip time and do not report the
device connected; and a request to an endpoint that cannot be reached, or
does not answer in time, fails that command (`transport`, or `timeout`), or is
logged for a poll, without reporting the device disconnected. Replies go to
the telemetry rules like any other, matched by their path: an endpoint's
paths should not be the transport's own.

A device whose transport is not HTTP can serve an HTTP API beside it too: a
Blackmagic HyperDeck takes its Ethernet Protocol on TCP 9993 and its
configuration on a REST API on port 80. Its spec keeps its line transport and
names the HTTP server under `endpoints` the same way:

```yaml
transport:
  type: line-tcp
  port: 9993
  endpoints:
    admin:
      port: { setting: admin_port }
      scheme: { setting: admin_scheme }      # defaults to http
      accept_invalid_certs: true             # over https, a self-signed certificate
commands:
  set_device_name:
    params: { name: { type: string, max_length: 64, required: true } }
    send:
      - { endpoint: admin, method: PUT, path: /admin/api/v1/setupBasic/name, body: '{"name":{name:json}}', content_type: application/json }
      - { endpoint: admin, method: GET, path: /admin/api/v1/setupBasic }
```

Such an endpoint has no credential to inherit: its `auth` is `none` (the
default there), and its requests carry no header but their own. A request
item names the endpoint, as above; its message is an HTTP request like an
HTTP command's, and its reply goes to the `path` rules. Requests are still
sent one at a time with the line's messages, in the same queue. Beside a
stream transport (`line-tcp`, `osc-tcp`, `ws`), a command whose every message
names an endpoint goes while the stream is down, and a request to an
endpoint already sent when the stream drops is answered: the device may be
reachable on one and not the other, and a command on the endpoint may be
what turns the stream back on. A request to an endpoint that is not answered
in time fails its command without resetting the stream, since its reply
cannot be mistaken for a line's. An endpoint's `accept_invalid_certs` (any
transport) overrides the transport's.

Each endpoint is a `control` entry in `ports` (below), with its `setting` when
its port comes from one; `tools/validate.py` checks the pairing, that each
`endpoint` named is declared, and that the probe and login name none. A
vector for a request on an endpoint gives its `port` in `expect_request`.

### `ws`

Text messages over a WebSocket, usually JSON.

```yaml
transport:
  type: ws
  port: 9000
  path: /api/v1               # the request path, from /
  scheme: ws                  # ws | wss | {setting: name}
  subprotocol: v1.control     # optional, sent as Sec-WebSocket-Protocol
  accept_invalid_certs: false # over wss, accept a self-signed certificate
  auth: none                  # none | basic | bearer
  timeout_ms: 2000
  reply: expected             # expected | none
```

Templates are text messages, rendered as for `line-tcp` with nothing added; a
JSON message is a template such as `'{"action":"get","path":{name:json}}'`.
The `path` may name settings as `{settings.name}`, each value
percent-encoded as one path segment; a setting with no value is an error, so
the path never loses a segment. A DHD AES67 module serves its API under its
module ID: `path: /modules/{settings.module}/api/ws`.
With `auth: basic` or `bearer`, the credential goes on the opening request as
for `http`, and a 401 or 403 answer to it is the same terminal refusal.

A reply is matched in one of two ways. With `expect.reply_json`, it is the
message whose JSON holds the rendered values at those JSON paths: a request id
the command sends, or what the device echoes (the parameter path it was
asked for). Other messages arriving meanwhile are not the reply. Without it,
the next message is the reply, as on `line-tcp`, and `reply_match` excludes
messages that cannot be one.

```yaml
commands:
  get_name:
    params: { layer: { type: int, min: 1, required: true } }
    send: '{"action":"get","parameter":"/composition/layers/{layer}/name"}'
    expect:
      reply_json: { "$.type": "parameter_get", "$.path": "/composition/layers/{layer}/name" }
      json_path: "$.value"
    returns: value
```

Every message, reply or not, is also offered to the telemetry rules (§8).
`on_connect` steps are sent when the websocket opens; telemetry `subscribe` and
`poll` messages are sent straight away rather than queued, because a websocket
device pushes and what it sends back goes to the rules. While a step with
`await_reply` (a login) still waits for its answer, they queue behind it
instead, each taking the next message as its reply, since such a device
accepts nothing before the login is answered (DHD's Control API). Pings are answered by
the core. `wss` uses TLS 1.2 or later and checks the device's certificate
against the same public roots as `https`; a device with a self-signed
certificate declares `accept_invalid_certs: true`, as for `http`: the
connection is encrypted, but the device's identity is not checked.

### Ports

Every spec, spec-driven or native, lists every port the integration uses and
its default, so a consumer can show and firewall them before opening
anything. The catalogue serves the list unchanged.

```yaml
ports:
  - { port: 10023, protocol: udp, role: control }
  - { port: 9000, protocol: udp, role: feedback, listener: core, setting: feedback_port }
  - { port: 52381, protocol: udp, role: control, when: "transport sony-ip" }
  - { port: null, protocol: tcp, role: control, note: "no standard port: the port is given when the device is opened" }
```

| Field | Meaning |
|---|---|
| `port` | The default. `null` when there is none and one must be given; a `note` then says so |
| `protocol` | `tcp`, `udp`, `http`, `https`, `ws`, `wss`, `tls`, `ssh`, or `serial`: a serial port of the host, with `port: null` and `setting: serial_port` (§2, `line-serial`) |
| `role` | `control` (where commands go; the port given when opening a device overrides the transport's, not an HTTP endpoint's), `push` (a websocket or event stream beside it), `feedback` (where the device sends replies or changes), `notification` (pushed status the device starts), `discovery` |
| `listener` | Who listens: `device` (the default) or `core` |
| `setting` | The setting that changes this port, if any |
| `when` | When the entry applies, if not always: a setting's value or a model |

There is always at least one `control` entry. For a spec-driven device the
transport's `port` must be one of them, each HTTP endpoint (§2, Endpoints)
needs one too (naming its setting, at the setting's default, when its port
comes from one), `listen_port` needs a `feedback`
entry naming its setting, and a push channel needs a `push` entry;
`tools/validate.py` checks this. A test checks that each native module,
opened without a port, uses its unconditional control default.

An integration whose every port is `listener: core` reaches out to no device,
so it may be opened with the host empty or left out (it is then `0.0.0.0`).
Its module decides whether it can do without one: `osc-listener` hears any
sender and needs none, while `tsl-umd-listener` is one switcher per device,
told apart by address, and refuses to open without it.

### Events

Every delivery drains one queue of JSON events, each with an `event` field:
`connection`, `state` (an RFC 7386 merge patch), `alive`, `log`, `closed`,
`message`, `credentials`, `dropped`, `discovered` and `discovery`. A listener reports what it
receives as `message` events, one per message, even when one repeats the
last exactly, since a state patch with an equal value is no change:

```json
{"event": "message", "device": 3, "address": "/1/push1", "types": "f",
 "args": [1.0], "source": "192.168.1.40:9000"}
```

`source` is the sender's `ip:port`; `types` is the protocol's own type
description where it has one (OSC type tags without the comma) and is
otherwise absent. When the consumer falls behind and the queue is full, state
patches are discarded first, oldest first, and counted in a `dropped` event's
`count`; every other event is kept. Message events alone are discarded past a
hard ceiling of 100,000 queued events, counted in `dropped`'s `messages`
(present only when not zero), so a stalled consumer cannot grow memory without
limit.

A `credentials` event carries new values of a device's settings that the core
obtained itself: the tokens of an `auth: oauth2` device after each refresh.
`expires_at` is in Unix seconds, or `null` when the service gave no lifetime;
`refresh_token` is present only when the service rotated it. The values are
secrets, held by the core in memory only; the consumer persists them (a
rotated refresh token replaces the stored one, which may no longer work) and
passes them when it next opens the device. Credentials events are never
discarded.

```json
{"event": "credentials", "device": 3,
 "settings": {"access_token": "ya29...", "expires_at": 1791234567}}
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

`secret: true` marks a value the core never logs or echoes.

The consumer can change an open device's settings with `update_settings`
(every delivery has it) instead of closing and opening it again. The values
given are merged into the device's current ones (a `null` puts a setting
back to its default) and the whole set is validated against the spec first:
a setting the spec does not declare, or a value it does not allow, fails with
`invalid_settings` and changes nothing. A spec-driven device takes the new
settings live: its state, connection and commands in flight are kept, every
later message uses them, and a later connection (a reconnect) logs in with
them; a device whose credential was refused (`unauthorized`) connects again
with them. A natively implemented device is restarted under the same device
id: its connection is closed and opened again with the new settings, its
commands in flight fail with `transport`, and its state and stream watchers
are kept.

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

Some devices ask for a password with a prompt and read anything sent before it
as a command. On a line transport, a step can wait for that prompt:

```yaml
on_connect:
  - when_set: password
    after_prompt: "Enter password:"
    send: "{settings.password}"
    refused: "^(Authentication error|Wait a moment)"
```

`after_prompt` is literal text, looked for in the raw stream because a prompt
usually has no line ending; what arrives before it is discarded. Until it
arrives nothing is sent, commands wait, and the device is not reported
connected. Without it within the transport's timeout, the connection is
dropped and retried with backoff. `refused` is a regex over reply lines: a
match means the device refused the credential, which is terminal as for HTTP
(§2): commands fail with `auth`, the connection reports `unauthorized`, and the
password is not sent again until the host opens the device with corrected
settings, or corrects them with `update_settings`. This is still not a handshake: the step is sent once, unchanged,
whatever the device says.

`refused` does not need `after_prompt`: on any line transport (`line-tcp` and
`line-udp`), and on `ws`, where it is read against each message, a login
step can declare the text a device answers a failed login with, even when it
answers in free text. A device whose console echoes free
text afterwards can declare `accepted` too, a regex over reply lines: once a
line matches it, the login is accepted and `refused` stops applying, so later
output that happens to match is not taken for a refusal. Both are only ever
written from the device's documented texts; a spec whose device documents no
failure text (grandMA2) declares neither, and a wrong credential there goes
unnoticed.

```yaml
on_connect:
  - send: "login {settings.username} {settings.password}"
    refused: "^Login failed"
    accepted: "^Logged in as "
```

A step the device answers (a login acknowledged with `ACK`) declares
`await_reply: true`. It is then queued like a query: sent in turn, its reply
consumed so it cannot be taken as the answer to the next command, and no
command goes ahead of it.

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
| `json` | Any JSON value, sent as compact JSON. Only for a request body whose shape the device's own API documents and validates; the command's summary names that schema |
| `binary` | Bytes given as base64 or as a host file (`{"file": "<path>"}`), sent as they are in an HTTP body (§4, "Binary request bodies"); `max_length` in bytes |

Values outside the declared range are rejected before transmission. The core
does not clamp, because a clamped value masks a caller error and produces a
different device state than the caller requested.

Every parameter, and every setting, has a `label` and a `description` for the
person choosing a value, served by the catalogue so a consumer can build its
forms from it:

```yaml
params:
  channel:
    type: int
    min: 1
    max: 32
    required: true
    label: Channel
    description: Input channel to change, numbered from 1 as on the console.
```

A parameter that is a credential (a password a command sets on the device)
takes `secret: true`, as a setting does: the catalogue serves it, so a
consumer masks the field, and the core never logs or echoes the value.

The label is a short noun phrase in sentence case, the way a form names the
field. The description is one plain sentence on what the value means: its
unit, how it is numbered, what its ends or values do. It shouldn't just
restate the type and range.

### Templates

Every string in `send`, `on_connect` and `probe` is a template. Substitution is
`{param}` for a command parameter or `{settings.name}` for a setting, optionally
followed by directives after `:`. A name is made of letters, digits, `_` and
`.`, so any other brace is literal: `{"status":"toggle"}` is sent as written.
A word in braces that is a protocol's own text, not a placeholder
(Lightware LW2's `{VC}`), escapes its braces with a backslash: `\{VC}` and
`\{VC\}` are both sent as `{VC}`. `\{` and `\}` are always a literal
brace, and a backslash before anything else is itself.

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

A string that is text by nature, such as a script or a note, takes
`multiline: true`: its value may then hold line breaks, tabs and any other
control character, because it is only ever sent through the `json` directive,
which escapes them (`\n`, `\t`, `\u0001`):

```yaml
params:
  script:
    type: string
    multiline: true
    required: true
    label: Script
    description: Python code to run, lines separated by line breaks.
send: { method: POST, path: /api/python/execute, body: '{"script":{script:json}}' }
```

Every placeholder naming a multiline parameter must be `{name:json}`, with no
other directive, wherever it stands; `tools/validate.py` checks this and the
core refuses to render it any other way. The catalogue serves `multiline`, so
a consumer can offer a text area. Such a parameter used to be a `json`
parameter holding a JSON string; a caller that passed a string to it passes the
same string now and the same bytes go out, while a value that was not a string
is now refused before transmission.

Encoding depends on where the value lands:

| Location | Encoding |
|---|---|
| Line/TCP payload, OSC address, OSC string argument | None; text encoded per `encoding` |
| HTTP `path` | Percent-encoded as a path segment (RFC 3986 unreserved characters kept); the `+` of a `signed` number is sent as it is, which RFC 3986 allows in a segment |
| HTTP `query` value | Percent-encoded as a query value; pairs are sent in the order the spec lists them |
| HTTP `raw_query` | None. Only `int`, `float`, `bool`, `enum`, a `string` with a `pattern`, or a `string` with the `url` directive, may appear here |

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
| `url` | String percent-encoded, RFC 3986 unreserved characters kept: `{name:url}` → `Cam%201`. For free text in a `raw_query` |
| `url_decode` | String percent-decoded, each `%XX` the byte it stands for: `{2:url_decode}` → `Camera 1` for `Camera%201`. For a telemetry value a device sends encoded (Spyder's names); a `%` not followed by two hex digits is kept, and a name that is all digits, which arrives as a number, is left as it is |
| `x`, `02x`, `03x`, … | Integer as lowercase hexadecimal digits, zero-padded to the width given: `{volume:02x}` → `64` for 100, `{id:x}` → `3e8` for 1000. For a protocol that writes numbers in hex as text (LG's `kf 01 64`) |
| `hex` | Hexadecimal digits a device sends, read as a number: `{1:hex}` → `100` for `64`, `{1:hex}` → `1000` for `3E8`. First among a placeholder's directives, which then apply to the number; text that is not hex digits only (or a negative number) assigns nothing. For a telemetry capture, which arrives as an integer when it holds digits only and is read the same way |
| `-1`, `+1`, … | Integer offset applied before formatting; combines as `{preset:-1:02d}` |
| `signed` | Number with an explicit leading sign: `7` → `+7`, `-7` → `-7`; with a float's `.Nf`, `{gain:signed:.1f}` → `+1.5`. For a protocol that reads a leading sign as a relative change (REAPER's `+1.5` for 1.5 dB up). A float that rounds to zero is `+0.0` |
| `bit.0` … `bit.63` | Integer → `true` / `false`, whether that bit is set, numbered from 0 for the value 1 (two's complement for a negative value): `{flags:bit.3}` → `true` for `328`. For a flags word a device sends whole, read into one boolean per bit (§8); offsets apply first, and it is the last directive |
| `.1f`, `.2f`, … | Float with a fixed number of decimals, rounded half away from zero: `{level:.2f}` → `0.75`. In a telemetry value, a whole number a device sends where it means a decimal is shown the same way (`60` → `60.00`) |
| `md5`, `sha256` | String hashed, as lowercase hex: `{settings.password:md5}` → `e3afed0047b08059d0fada10f400c1e5` for `Admin`. For a device that takes a password hashed (Magewell's login) |
| `to.<name>` | A number converted to the wire by the named conversion (below): `{level_db:to.x32_fader}` |
| `from.<name>` | A wire number converted back to the operator's value: `{arg0:from.x32_fader:.1f}` |
| `map.<name>` | A value replaced by its wire text from the named value table (below): `{level:map.reference_level}`. The only directive on the placeholder |
| `key` | In a telemetry state path or `replace` only: the value is one state key, dots included: `types.{id:key}.name` (§8, "Keys holding dots") |

Offsets apply before formatting, and directives apply left to right.

`bool10` covers flags whose sense is inverted relative to the parameter name,
such as the X32's `mix/on` where `0` is muted.

Offsets exist because several protocols number from zero while operators count
from one: Videohub inputs and outputs, and Panasonic PTZ presets, are all
0-based on the wire. Declaring the offset keeps the operator-facing parameter
1-based without each consumer reimplementing the conversion.

### Conversions

Some devices take a value on a scale the operator does not use: the X32's
fader position 0.0-1.0 for a level in dB. A spec declares the conversion once,
as points joined by straight lines, and uses it in both directions:

```yaml
conversions:
  x32_fader:                  # Maillot p.128: four linear segments
    points: [[0.0, -90.0], [0.0625, -60.0], [0.25, -30.0], [0.5, -10.0], [1.0, 10.0]]
```

Each point is `[wire, value]`. Wire values rise strictly, and values rise or
fall strictly, so the conversion inverts. A value on a point converts to that
point exactly; between points, by the straight line between them. A value
outside the points does not convert: a command is rejected before
transmission and a telemetry value is not assigned. Nothing is clamped.

`to.<name>` converts a command's value to the wire and `from.<name>` a wire
value to the operator's. Conversions come before other directives and produce
a float, so as text a converted value needs one `.Nf`; as the whole value of
an OSC `float` argument, or the whole value of a state assignment, it is a
number and needs none. `expect.convert: <name>` converts a command's returned
value from the wire (§5). A documented formula made of linear segments is
written as its segment ends; a curved law has no exact form here.

### Value tables

A value the operator chooses by name may be several things on the wire.
Magewell's NDI transport is one enumeration for the operator and four flags
in the request, at most one of them true; its reference level is `smpte` or
`ebu` for the operator and `20` or `14` on the wire. A spec declares each
table once under `maps`, from the value's plain text (an enum value, `true`
or `false`, a decimal integer) to the wire text:

```yaml
maps:
  reference_level: { smpte: "20", ebu: "14" }
  ndi_udp: { tcp: "false", multi-tcp: "false", udp: "true", rudp: "false", multicast: "false" }
commands:
  set_reference_level:
    params: { level: { type: enum, values: [smpte, ebu], required: true } }
    send: { method: GET, path: /mwapi, query: { method: set-ndi-config, reference-level: "{level:map.reference_level}" } }
```

A value the table does not list is rejected before transmission; for an
enum or a bool, `tools/validate.py` requires every value to be listed. The
wire text is spec-authored, so in a `raw_query` it may name a query key
(`raw_query: "method=set-video-config&{overlay:map.overlay_field}={visible}"`).
A table is a lookup, not a conditional: the same value always gives the same
text.

### Messages per model

A family whose models speak different dialects (Magewell's encoders take
`GET /mwapi?method=reboot`, its IP decoders `POST /api/reboot`) keeps one
command, with one message per dialect, each naming the `models` it is for:

```yaml
commands:
  reboot:
    send:
      - { models: [hdmi-plus, sdi-plus], method: GET, path: /mwapi, query: { method: reboot } }
      - { models: [ip-to-hdmi], method: POST, path: /api/reboot }
```

A line device's text messages are written the same way, the text under
`message`. A HyperDeck on protocol 1.8 is not asked for what its protocol
lacks:

```yaml
telemetry:
  poll:
    send:
      - "transport info"
      - { models: [hyperdeck-studio, hyperdeck-extreme], message: "cache info" }
```

A message naming `models` is sent only to those models; one without is sent
to every model. This is decided from the model the device was opened as,
before anything is sent, like `when_set` (§2), never from what the device
says. It applies to the object messages of `send` (HTTP requests and OSC
messages, and on line and websocket transports a text message written as
`{models: [...], message: "..."}`), telemetry `poll` and `subscribe` items, `on_connect` steps, the
`probe` (a list whose first message for the model is used), a session's
`login` and telemetry `updates` rules (§8, "Rules per model").
`tools/validate.py` checks that every model a command `supports` has a
message, and that every model a message or rule names exists.

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

OSC argument types: `int`, `float`, `string`, `blob`. A `blob` carries its
rendered template's text as UTF-8 bytes, for a device that takes text beyond
ASCII that way (OSCPoint's section and file names).

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

Inbound messages on other addresses are not the reply. The reply address
is usually the query's or another `/`-prefixed one, but it is matched as
written: the X32 answers `/node` on `node`, without the leading `/` OSC
requires.

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

`content_type` is a template, so an enum parameter can choose it (an image
as `image/jpeg` or `image/png`). `headers` gives headers of one message
alone, each a template over its parameters and the settings, as
`transport.headers` (§2) does for every request.

### Binary request bodies

A file a device takes as a request body (an image, a show file, a media
clip) is a `binary` parameter: the caller gives its bytes as a base64 string
(RFC 4648, padding optional), or names a file on the host running the core
as an object whose only member is `file`, a path (an object, so it is never
taken for base64 text). WATCHOUT's `load_show_file` takes either:

```json
{"file": "AAECAwQ="}
{"file": {"file": "D:/Shows/Main.watch"}}
```

and the core sends the bytes as they are. A file is opened when the command
is checked, and one that is missing, cannot be read or is not a file refuses
the command with `invalid_params` naming the parameter (the message names the
path, never the contents). It is then read from disk as the request goes out
and never held whole: the body, or the multipart body around it, is streamed
with `Content-Length` the file's size, so no size is too large for the core
(binary parameters go only over HTTP, so nothing reads one whole). A file
that shrinks before it is sent fails the request. `max_length`, given only
for a limit the vendor documents, bounds the decoded bytes and a file's size
alike; the core sets no limit of its own. A binary parameter is `required`,
and is never rendered in a template; it is sent in one of two ways:

```yaml
commands:
  set_thumbnail:
    params:
      video_id: { type: string, max_length: 64, required: true, … }
      image: { type: binary, max_length: 2097152, required: true, … }
      format: { type: enum, values: [image/jpeg, image/png], default: image/jpeg, … }
    send:
      method: POST
      path: /upload/youtube/v3/thumbnails/set
      query: { videoId: "{video_id}", uploadType: media }
      body: { binary: image }          # the bytes are the whole body
      content_type: "{format}"
  upload_media:
    params: { file: { type: binary, required: true, … }, name: { type: string, … } }
    send:
      method: POST
      path: /media/upload
      multipart:                       # multipart/form-data (RFC 7578)
        - { name: addToMap, value: "true" }
        - { name: MediaFile, binary: file, filename: "{name}", content_type: application/octet-stream }
```

With `multipart` in place of `body` and `content_type`, each part is a form
field: `name`, then a template `value` or a binary parameter's bytes, with an
optional `filename` and `content_type`, both templates (a line break or a
quote in them, or in `name`, is refused before transmission). The core writes
the body with a boundary derived from its content, so the same parts always
give the same bytes, and sends `Content-Type: multipart/form-data;
boundary=...`. A host file's part counts by its size in that derivation and
is not read for it; with one, the boundary carries at least 128 bits of the
hash, which a file holds only if it was made to. A vector for such a command
gives the body as `body_hex`, and may give `content_type`, in
`expect_request`. `tools/validate.py` checks that
a binary parameter is required, named only by `body.binary` or a part's
`binary`, and used only over HTTP.

## 5. Responses

```yaml
expect:
  contains: "REP"           # literal substring
  not_contains: "ERR"       # success is the absence of an error marker
  matches: "^OK (\\d+)$"    # RE2-safe regex; capture group 1 is the value
  status: 200               # HTTP only; or a list of statuses, any of which is success
  json_path: "$.transport.status"   # JSON: the HTTP body, or the OSC argument at `arg`
  json_equals: { "$.status": "ok" } # JSON values the reply must hold
  code_range: [200, 299]    # leading numeric response code
  code_path: "$.status"     # with code_range: the code is this JSON number instead
  header: Location          # HTTP only: this response header's value is returned
  address: /ch/01/config/name       # OSC only: the reply's address
  arg: 0                    # OSC only: argument returned as the value
  reply_json: { "$.id": "{id}" }    # ws only: what identifies the reply (§2)
  reply_contains: "REP {channel:02d} AUDIO_MUTE "  # line transports: text the reply holds (§2)
  replies: 2                # line transports: replies each message takes (§2)
  convert: x32_fader        # the returned value, converted from the wire (§4)
```

Some OSC devices answer with JSON inside a string argument: QLab replies on
`/reply/<address>` with `{"status": "ok", "data": ...}`. On an OSC reply,
`json_path` and `json_equals` read the string argument at `arg` as JSON, so
`json_path: "$.data"` returns the data and `json_equals: { "$.status": "ok" }`
fails the command on an error or denied reply.

`status: [200, 204]` takes any status listed as success, for a device whose
writes are answered with a body by some firmware and without one by others.
Without `status`, any HTTP status passes the check, so a command over HTTP
names the statuses it succeeds with.

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

A JSON API that answers every request with a status number in its body
(Magewell's `{"status": 37}`, 0 for success) names it with `code_path`, and
`code_range` and `codes` then apply to that number. An HTTP `status` in
`expect` is checked first, so an HTTP error is reported as such.

`returns` declares the result type:

| Value | Result |
|---|---|
| `ack` | Boolean. True when `expect` matched |
| `value` | Captured value: regex group 1, the `json_path` result, or the OSC argument. Requires one of `matches`, `json_path` or `address` |
| `fields` | Map of `key: value` lines from the reply body, split at the first `: `. For HyperDeck-style replies such as `208 transport info:` |
| `text` | The reply body as text: for `headed-block`, the lines after the first; otherwise the whole reply |
| `none` | No acknowledgement available. The core reports `unverified` |

`json_path: "$"` returns the whole JSON body.

A JSON path is `$`, then member names after `.` and array indexes in
brackets, counted from 0: `$.profile.streams[0].video.kbps`. A path that
names something the JSON does not hold has no value. JSON paths in `expect`,
telemetry rules and `refusal_json` are all read this way.

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

Every spec with a `telemetry` section has at least one. A spec whose
telemetry needs a required setting gives `settings`, as a command vector does.

## 8. Telemetry

`telemetry` says how the device's own messages become state. The paths it
writes must be declared in `state` (§1), whose `type` decides how each value
is converted: `int`, `float`, `bool` (from `true` / `false`, or through a
`map`) or `string`. A value that does not convert is not assigned; the core
never guesses. A JSON null is no value: the path keeps what it had.

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

A device whose state changes at different rates (a timer every second, a
configuration every minute) gives `poll` as a list of groups, each sent on
connecting and then at its own `every_ms`, at most four:

```yaml
telemetry:
  poll:
    - every_ms: 1000                # what moves during a show
      send: [{ method: GET, path: /v1/timers/current }]
    - every_ms: 30000               # what an operator configures
      send: [{ method: GET, path: /v1/timers }, { method: GET, path: /v1/props }]
```

A group is a poll like any other: its items are queries, queued one at a
time, and an item still waiting from its last round is not queued again.
Over HTTP this is the way to read something less often; a `subscribe` with
`every_ms` is sent again at its interval too, and older specs use it so, but
its items are not kept from piling up behind a slow device.

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
| `address` + `arg_match` | As `address`, where the text of each argument named matches its regex | as `address`, then `arg_match`'s |
| `address` + `json` | As `address`, where the string argument at `json_arg` (default 0) holds JSON; with `json_match`, `json_each` and `each_match` it is read as a JSON message is (below) | as `address`, then `json_match`'s, plus `json` names by JSON path |
| `quiet_ms` | No message: the stream silent this long, once per connection (below, "A status dump that ends in silence") | none |

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

A device that sends several on/off states as one integer, a sum of bit
values, has each bit read into a boolean of its own with `bit.N` (§4), bit 0
being the value 1. REAPER's track flags, 8 muted and 16 soloed:

```yaml
      state:
        "tracks.{2}.flags": "{4}"
        "tracks.{2}.muted": "{4:bit.3}"
        "tracks.{2}.soloed": "{4:bit.4}"
```

A capture that is not a decimal integer assigns nothing.

A rule can also remove what the device says is gone. `{delete: true}` in
place of a value removes the path from state: one declared value, or a whole
subtree of them (`layers.<id>` and everything under it). mimoLive pushes
`{"event": "removed", "type": "layers", "id": "..."}` when a layer is
deleted:

```yaml
    - json_match: { "$.event": "^removed$", "$.type": "^(layers|sources)$", "$.id": "^([^.]+)$" }
      json: {}
      state: { "{1}.{2}": { delete: true } }
```

The state patch carries `null` there, which RFC 7386 reads as a removal:
the snapshot loses the subtree, and the `state` event tells consumers so.
The path must be declared, or lead to declared paths; a captured segment
that renders empty or holds a `.` removes nothing, so a device can never
name something else (a `key` segment, below, is one key whatever it holds). Within one message a removal wins over a value the same
message assigns under it. A telemetry vector for a removal gives
`state_before`, the state the message arrives on.

#### Lists

A device that reports a list whole each time (a file list, a service's
items) declares the subtree each such message replaces with `replace`:
whenever the rule's message arrives (its path, header or selectors match,
even with no element at all), the subtree is removed first, in a patch of
its own, and the values the message gives are applied after it, so a list
that shrinks or empties leaves nothing behind. In a `header` + `each_line`
rule, `{index}` is the line's place among the lines that match, from 0, for
lists of plain lines (Ultimatte's FILE LIST):

```yaml
    - header: ^FILE LIST:$
      each_line: ^([^:]+)$
      replace: files
      state: { "files.{index}": "{1}" }
    - path: "^/api/v2/service/items$"
      json_each: "$"
      json: { id: "$.id", title: "$.title" }
      replace: service.items
      state: { "service.items.{id}.title": "{title}" }
```

`replace` is a path or a list of them, templates over the captures before
`json_each` (the path's, the selectors'; with `json_each`, each element's
too, below), each declared or leading to
declared paths. Consumers see the removal and the new list as two `state`
events in a row, unless the list is what the state already holds there: a
list read again unchanged (a poll every second of timers or playlists that
did not move) reports neither its removal nor its values, so a consumer sees
a `state` event only when something changed. The core compares what it has
reported itself for that device; values the same message writes outside the
list are reported as usual. A telemetry vector's `state_before` is that
state. A rule may carry `replace` with no `state` when only some
messages hold the whole list (Planning Center's first page with no `next`
link): it sits beside the plain rule that writes the values for the same
messages, and since every removal of a message is applied before any of its
values, the order of the two rules does not matter.

A reply can hold many lists, one in each element: mimoLive's layer sets each
carry their `layers`. With `json_each`, `replace` is also rendered for each
element from that element's captures (its `json` names, `{index}`), removing
that element's subtree. `each_match` selects the elements, as `json_match`
selects messages: a JSON path from the element (`$^.` the element one level
out) to a regex its value must match, with captures numbered on after the
path's and `json_match`'s; an element that does not match is skipped, for its
values, `then_send` and `replace` alike. Matching `^\[` tests that an
attribute is present and an array, so a set whose reply leaves `layers` out
keeps its list, while an empty array clears it:

```yaml
    - path: "^/api/v1/"
      json_each: "$.data"
      each_match: { "$.type": "^layer-sets$", "$.attributes.layers": "^\\[" }
      json: { id: "$.id" }
      replace: "layer-sets.{id}.layers"
    - path: "^/api/v1/"
      json_each: "$.data[*].attributes.layers"
      each_match: { "$^.type": "^layer-sets$" }
      json: { id: "$^.id", action: "$.action" }
      state: { "layer-sets.{id}.layers.{index}.action": "{action}" }
```

Over HTTP, `poll` requests and command requests alike have their replies
offered to `path` rules, which match the request's path and query:

| Rule | Matches | Captures |
|---|---|---|
| `path` + `json` | A JSON reply | `json` names values by JSON path (`$`, `$.a.b`) |
| `path` + `json` + `json_each` | A JSON reply holding an array at `json_each` | once per element; `json` paths relative to it |
| `path` + `xml_each` | An XML reply | once per element of that name; its attributes, its text and its children's (below) |
| `path` + `json` + `json_match` | A JSON reply whose value at each JSON path matches its regex | the path's, then `json_match`'s, in order |
| `path` + `json` + `request_match` | A JSON reply to a request whose JSON body matches | the path's, then `request_match`'s, then `json_match`'s |
| `path` + `headers` | Any reply, JSON or not | `headers` names response header values, by header name in any case; with `json` too, both |
| `path` + `match` | A text reply, by regex over the whole body | the path's, then the text's, numbered on after them |

`request_match` is for protocols whose replies all arrive on one path and
don't say what they answer, such as JSON-RPC: the rule looks at the request
that the reply answers.

A text reply is also offered to the plain `match` rules, which cannot tell
which request it answers. Where replies to different requests share their
words, `path` + `match` ties the rule to the request: Panasonic's
`/cgi-bin/get_rtmp_status` and `/cgi-bin/get_srt_status` both answer
`status=1`. `(?m)` lets `^` and `$` match at each line of a `key=value`
reply:

```yaml
    - path: "^/cgi-bin/get_(rtmp|srt)_status$"
      match: "(?m)^status\\s*=\\s*([01])\\s*$"
      state: { "{1}.streaming": { value: "{2}", map: { "0": false, "1": true } } }
```

```yaml
    - path: "^/$"
      request_match: { "$.method": "^Pixera\\.Timelines\\.Timeline\\.getCurrentTime$", "$.params.handle": "^(\\d+)$" }
      json_match: { "$.result": "^\\d+$" }
      json: { frame: "$.result" }
      state: { "timelines.{1}.frame": "{frame}" }
```

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

`json_each` names an array, and may reach it through arrays: `[*]` in its
path goes through every element of an array on the way, so
`$.data[*].ingest_streams` is each ingest stream of each of the Page's live
videos. In each match, `json` paths are the element's; `$^.` names the
element one level out (the live video, or the whole reply for an array
without `[*]`), `$^^.` two; and `{index}` is the element's index in its
array, for state keyed by position where the elements have no id:

```yaml
    - path: '^/v[0-9.]+/[0-9]+/live_videos\?'
      json_each: "$.data[*].ingest_streams"
      json: { video: "$^.id", stream: "$.id", bitrate: "$.stream_health.video_bitrate" }
      state: { "live_videos.{video}.ingest_streams.{stream}.video_bitrate": "{bitrate}" }
```

The elements a match lies in have places too: `{^index}` is the index of
the element one level out (the one `$^.` names), `{^^index}` two, and so
on, so elements numbered only within their own array are keyed by both
positions. ProPresenter numbers each slide within its group:

```yaml
    - path: "^/v1/(presentation|announcement)/active$"
      json_each: "$.*.groups[*].slides"
      json: { text: "$.text" }
      state: { "{1}.groups.{^index}.slides.{index}.text": "{text}" }
```

An element reached through `.*` gives its member name as `{^key}` (one
level out), `{^^key}` (two) and so on, beside `{key}` for the match itself.
A level with no place (the document, or an array reached without `[*]`)
gives neither.

A member the operator chooses is named by a setting in the `json_each`
path, `{settings.name}` with no directive: H2R Graphics pushes every
project of an installation in each event, and the spec reads the one its
commands reach:

```yaml
    - json_match: { "$.event": "^updateFrontend$" }
      json_each: "$.data.projects.{settings.project}.cues"
```

The path is filled in from the settings current when each message arrives,
so `update_settings` changes what is read. A setting that is empty, or
whose value holds `.`, `[`, `]` or `*` (which would make it another path),
gives no element, as a member the message does not hold does; `replace`
still applies, so the list then empties.

A device that lists things as an object's members, keyed by name, rather
than as an array (BirdDog's NDI sources, `{"CAM 1 (BirdDog)":
"10.0.0.5:5961", ...}`) is gone through with `.*`: `json_each: "$.*"` is
each member of the reply, `$.sources.*` each member of the object at
`$.sources`. Each match captures the member's name as `{key}`, and `$` is the
member's value; `{index}` is its place among the members, in the order the
device sent them. `.*` may also stand within a path, as `[*]` does
(`$.devices.*.inputs` is the inputs array of every member of `devices`, and
`$^.` the member), and a value that is not an object there gives no match.
Names are usually free text, so the state key takes them with `key` (below,
"Keys holding dots"):

```yaml
    - path: "^/List$"
      json_each: "$.*"
      json: { address: "$" }
      replace: ndi.sources
      state: { "ndi.sources.{key:key}.address": "{address}" }
```

A JSON path anywhere may also index an array, `$.streams[0].name` (§5).
State keyed this way keeps what a later reply no longer lists, so a list
that shrinks (a tag list) is better kept whole, as JSON text.

A telemetry vector for HTTP gives `inbound_http: { path, body }` in place of
`inbound`, plus `request` (the JSON request body) for a `request_match` rule
and `headers` (name to value) for a `headers` rule.

#### XML elements

An `xml_each` match captures its element's attributes by name, then
`{text}`, the element's own text (text and CDATA, trimmed, entities
decoded), each child element that holds only text under the child's name,
and the attributes of the elements it lies in, `{^name}` for the element one
level out and `{^^name}` two, as `$^.` names an enclosing JSON element. A
name already captured keeps its first value, so an attribute wins over a
child or the text. TriCaster's product information is child elements, and
its buffer dictionary keys each M/E row's buffer by the `<me>` two levels
out:

```yaml
    - path: "^/(?:v1/)?version$"
      xml_each: product_information
      state: { product.model: "{product_model}", product.name: "{product_name}" }
    - path: "^/v1/dictionary\\?key=buffer$"
      xml_each: buffer
      state: { "buffers.me.{^^index}": "{selection}" }
```

A placeholder whose capture an element does not give leaves its path
unassigned, so the main row's buffer, with no `<me>` around it, writes
nothing there.

#### Keys holding dots

A state path's segments are separated by dots, so a captured value holding
one would split its segment: without more, a rule writing `sources.{id}.name`
for the id `com.boinx.source.camera` reaches no declared path and assigns
nothing, and a deletion or `replace` with it removes nothing. A device whose
ids, names or types hold dots (mimoLive's type identifiers, a media server's
path names, a host name) marks the segment with the `key` directive, and the
value is kept as one segment, dots and all:

```yaml
    - path: "^/api/v1/sources$"
      json_each: "$.data"
      json: { id: "$.id", type: "$.attributes.source-type" }
      state: { "sources.{id:key}.type": "{type}" }
```

`key` goes last among a placeholder's directives (`{1:url_decode:key}`), and
only in a state path or a `replace` path; a key segment that renders empty
is not written. Consumers see the key as the literal string it is: the state
patch is `{"sources": {"com.boinx.source.camera": {"type": "..."}}}`, an RFC
7386 merge patch whose object key holds the dots, and the snapshot has the
same object. A consumer that addresses state by a dotted path must not split
such a key; it walks the JSON objects instead. The declared path stays
`sources.*.type`: `*` stands for any one key, dotted or not.

#### Rules per model

A family whose models give the same reply different meanings names the
`models` a rule is for, as a message does (§4, "Messages per model"). The
O.I.S. setting of a Panasonic AW-UE80 answers `OIS:2` for O.I.S. on pan and
tilt, an AW-UE100 the same text for its hybrid stabiliser:

```yaml
    - models: [aw-ue80]
      match: "^OIS:([0-2])$"
      state: { "ois": { value: "{1}", map: { "0": "off", "1": "stable", "2": "pan_tilt" } } }
    - models: [aw-ue100]
      match: "^OIS:([0-3])$"
      state: { "ois": { value: "{1}", map: { "0": "off", "1": "ois", "2": "hybrid_stable", "3": "hybrid_pan_tilt" } } }
```

A rule naming `models` is left out for any other model, decided from the
model the device was opened as; one without applies to every model. A
telemetry vector for such a rule gives the `model` to open the device as.

#### Replies and what they answer

Some line devices answer with the value alone: Lab.gruppen's NLB 60E
answers `AMP1.Power ?` with `1`, and `Subnet.Mute ?` with `1` too. A device
that answers one message at a time, in order, still says what it answers:
the message in flight. A `match` rule names the text of that message with
`request_match`, an RE2-safe regex; its captures are numbered on after the
reply's:

```yaml
    - match: "^([01])$"
      request_match: '^([A-Z0-9@-]+)\.Mute([A-H]) (?:= [01]|\?)$'
      state: { "amps.{2}.channels.{3}.mute": { value: "{1}", map: { "0": false, "1": true } } }
```

The rule then matches only a message taken as the reply to a command, a
query or a probe, and only when the text sent (the template rendered,
without framing) matches `request_match`: a reply to a set and to a get
alike, and never a message the device pushes. Rules without
`request_match` see replies as before. A telemetry vector for such a rule
gives `request`, the text of the message the `inbound` reply answers.

With `then_send` (below, "Re-reads on a push"), a reply can have more read: the NLB's VDN table
names each amplifier, and each name has that amplifier's status, power and
mutes read in turn.

#### Lists on one line

Some line devices answer a list on one line: Christie Spyder answers its
register list `RRL 4 -1` with `0 2 1 Look%201 7 Wide`, the result code, a
count, then an ID and a name for each register. `match_each`, an RE2-safe
regex, is applied along the rest of the message after what `match` matched,
and the rule matches once for each match of it: its captures are numbered on
after `match`'s and `request_match`'s, and `{index}` is the item's place, from
0. `replace` and `then_send` work as for any rule, so the list is replaced
whole and each item can have more read:

```yaml
    - match: "^0 \\d+(?: |$)"
      request_match: '^RRL (\d+) -1$'
      match_each: '(\d+) (\S+)'
      replace: "registers.{1}"
      state: { "registers.{1}.{2}.name": "{3:url_decode}" }
```

The message must match `match` for the rule to apply at all, so an empty list
(`0 0`) still replaces what was there.

A list that does not run to the end of the message, or shares its opening
words with another message, is taken from one capture group of `match` with
`match_each_in`, the group's number. A Shure MXA sends each meter as `<
SAMPLE 040 052 ... >`, one level per channel, and its talker positions as `<
SAMPLE TALKER_POSITIONS ... >`; only the first is a meter list:

```yaml
    - match: "^< SAMPLE((?: \\d{3})+) >$"
      match_each: "(\\d{3})"
      match_each_in: 1
      state: { "meters.levels.{index:+1}": "{2:-60}" }
```

`{index}` takes offsets as a number does, so a list numbered from 1 keys its
state from 1. A group that took no part in the match gives no items.

#### Poll replies on another address

An OSC query is answered on its own address by most devices. ETC Eos answers
`/eos/get/...` on `/eos/out/get/...`, sometimes with a count appended
(`/eos/out/get/cuelist/1/list/0/13`). A poll item names the address its reply
arrives on, as an RE2-safe regex, and waits for a message matching it:

```yaml
  poll:
    send:
      - { address: /eos/get/version, reply_address: "^/eos/out/get/version$" }
      - { address: /eos/get/cuelist/1, reply_address: "^/eos/out/get/cuelist/1/list/0/\\d+$" }
```

`reply_address` is for poll items on OSC transports. Because such a reply
names what it answers, a query that is never answered (a cue list that does
not exist) times out without the stream being reset, and the next query goes;
the same holds for commands with `expect.address` on `osc-tcp` and for
`reply_json` on `ws`. A reply taken in order still resets the stream on a
timeout, since a late one would be read as the next answer.

#### OSC type tags

A device can send different shapes on one address: grandMA3 sends a
sequence's key feedback as `sis` and its fader feedback as `sif`, both on
`/13.13.1.6.<n>`. `arg_types` limits an `address` rule to messages with
exactly those type tags:

```yaml
    - address: '^/13\.13\.1\.6\.(\d+)$'
      arg_types: sif
      state: { "sequences.{1}.faders.{arg0}": "{arg2}" }
```

#### OSC string arguments

A device that names what changed inside a string argument (Millumin's
`/millumin/info`, whose first argument is the original feedback address,
`layer:Screen/opacity`, and the next its value) is read with `arg_match`: an
argument's index to an RE2-safe regex its text must match, a number as its
decimal text. The rule applies only when each matches, and their captures
are numbered on after the address's, in index order (before `json_match`'s):

```yaml
    - address: "^/millumin/info$"
      arg_match: { "0": "^layer:([^/]+)/(opacity|rotation|scale)$" }
      state: { "layers.{1}.{2}": "{arg1}" }
```

A list inside a string argument is read item by item with `match_each`, as
for a list on one line (above), and `match_each_in`, which an address rule
requires: the capture, numbered across the address's and `arg_match`'s,
whose text `match_each` goes along. The rule matches once per item, its
captures numbered on after the rule's and `{index}` its place from 0;
`replace` and `then_send` work as for any rule, so a list read whole
replaces the last. A Behringer WING answers its node query (`/$ctl/lib ,s
?`) with one string describing every parameter of the node, the show's
scenes among them:

```yaml
    - address: "^/\\$ctl/lib$"
      arg_match: { "0": "\\$scenes\\s+list \\[([^\\]]*)\\]" }
      match_each: "\\s*([^,]+?)\\s*(?:,|$)"
      match_each_in: 1
      replace: scene.list
      state: { "scene.list.{index:+1}": "{2}" }
```

A blob argument holding UTF-8 text is read as that text, wherever a rule
reads an argument: as `{argN}`, by `arg_match` and as the JSON at
`json_arg`. OSCPoint sends each name twice in one message, as an ASCII
string and then as a UTF-8 blob (`,sb`), so a rule limited to `arg_types: sb`
after the plain one keeps the full text. A blob that is not UTF-8 stays its
bytes, which no text rule reads.

#### JSON messages

A message on a websocket (the transport's, §2, or `telemetry.websocket`'s), an
event on `telemetry.sse`, and a line or block of a line transport are offered
to the text rules and, when they are JSON, to `json_match` rules:

| Rule | Matches | Captures |
|---|---|---|
| `json_match` | A JSON message whose value at each JSON path matches its regex (a string as is, anything else as compact JSON) | `{1}`, `{2}`, … across the regexes in order; `json` names values by JSON path |
| `json_match` + `json_each` | As above, holding an array at `json_each` | once per element; `json` paths relative to it |

```yaml
    - json_match: { "$.type": "^parameter_(subscribed|update)$", "$.path": "^/composition/master$" }
      json: { value: "$.value" }
      state: { composition.master: "{value}" }
```

An OSC device that answers with JSON in a string argument (QLab's
`/reply/...`) is read the same way by an `address` rule: `json_match`
selects on that JSON (its captures numbered on after the address's),
`json_each` and `each_match` go through its arrays, and `replace` and
`then_send` apply as for any rule. QLab's cue lists, groups nested in them:

```yaml
    - address: "^/reply/workspace/([^/]+)/cueLists$"
      json_match: { "$.status": "^ok$" }
      json_each: "$.data[*].cues"
      json: { id: "$.uniqueID", parent: "$^.uniqueID" }
      replace: "workspaces.{1}.cues"
      state: { "workspaces.{1}.cues.{id}.parent": "{parent}" }
```

#### A push websocket beside the transport

`telemetry.websocket` opens a websocket when the device is opened and keeps
it open, reopening it with backoff when it closes. It does not carry commands
and does not decide whether the device is connected; the transport does.

```yaml
telemetry:
  websocket:
    path: /api/v1             # required, unless url (below)
    port: 8080                # defaults to the transport's port; or {setting: name}
    scheme: ws                # ws | wss | {setting: name}
    subprotocol: v1           # optional
    accept_invalid_certs: false # over wss; defaults to the http transport's
    send:                     # sent each time it opens: the subscriptions
      - '{"action":"subscribe","parameter":"/composition/master"}'
    every_ms: 5000            # optional: send them again at this interval
```

`scheme` may name the HTTP transport's own scheme setting, an enum of
`http` and `https`: `http` then opens `ws` and `https` opens `wss`, so a
device serving its push websocket beside its REST API (a Blackmagic
camera) needs no second setting to keep the two in step.

`port` may name an integer setting, `port: { setting: websocket_port }`, for
a device whose push port the operator can move (OpenLP, FreeShow); an empty
setting means the transport's port.

A service that pushes on another host, with its token in the URL, gives an
absolute `url` in place of `path`, `port` and `scheme`. Restream's streaming
updates:

```yaml
telemetry:
  websocket:
    url: "wss://streaming.api.restream.io/ws?accessToken={settings.access_token}"
```

The URL is a template over settings, each value percent-encoded, rendered
each time the websocket opens or reopens, so a token the core has refreshed
since (`auth: oauth2`) is the one sent. Its scheme and host are written in
the spec: `wss`, or `ws` only to this machine (`localhost`, `127.0.0.1`,
`[::1]`), checked again on the rendered URL; a URL that fails is not opened
and never logged, since it may hold a token. The transport's credential and
`headers` do not go to the other host. When the URL names a setting, a 401
or 403 answer to its opening is a refusal of that token: refreshed once if it
can be, as on the transport, and otherwise the terminal refusal of §2.

`idle_ms` reopens the websocket when nothing at all has arrived on it for
that long, for a service that sends keepalives, where silence means a dead
connection.

`every_ms` sends the `send` items again at that interval while the websocket
is open: a keepalive a device requires of a client (mimoLive closes a socket
that sends nothing for 15 seconds), or a request a device answers with its
state (FreeShow's variables). Text that arrives in binary frames (OpenLP's
state) goes to the rules as text when it is UTF-8.

A device that pushes a changed value in the format its REST GET returns
names where with `as_reply`, as for `telemetry.sse` (below): each JSON
message is also offered to the `path` rules as the reply to a GET of the
path at `as_reply.path`, with the value at `as_reply.body` as its body, so
the rules that read a poll's reply apply the push directly. `prefix` is a
fixed path the named one lies under. A Blackmagic camera names
`/lens/iris` for its REST API's `/control/api/v1/lens/iris`:

```yaml
telemetry:
  websocket:
    path: /control/api/v1/event/websocket
    as_reply: { path: "$.data.property", body: "$.data.value", prefix: /control/api/v1 }
```

A message without both paths is offered to the rules as usual only.

A device that pushes over Socket.IO (H2R Graphics, FreeShow) sets `socketio`:

```yaml
telemetry:
  websocket:
    path: /socket.io/
    socketio: true            # or { engine_io: 3, namespace: /stage, auth: '...' }
    send:                     # events emitted once the namespace is joined
      - '["subscribe", {"topic": "slides"}]'
  updates:
    - json_match: { "$.event": "^slide$" }
      json: { index: "$.data.index" }
      state: { slide.index: "{index}" }
```

The core adds `EIO=4&transport=websocket` (or `EIO=3`) to the query, joins
the namespace when the server's open packet arrives, answers pings (Engine.IO
4) or sends them at the server's interval (Engine.IO 3), and gives each event
to the `json_match` rules as `{"event": <name>, "data": <first argument>,
"args": [<every argument>]}`. `auth`, a JSON object written as a template over
settings (`'{"token":{settings.token:json}}'`), goes with the namespace
connect, as Socket.IO's handshake `auth`. With `every_ms`, the `send` events
are emitted again at that interval.

The transport's `basic`, `bearer` or `header` credential goes on its opening request; a
401 or 403 answer is the terminal refusal of §2. `send` items are templates
over settings. Its messages go through the rules like any other.

A telemetry vector for a websocket gives `inbound_ws` (the message text), and
optionally `expect_connect_ws`, the messages sent when it opens.

#### A status dump that ends in silence

A device that sends its whole state on connecting may mark the end of it
(a Videohub's `END PRELUDE:`), or, on older firmware, just stop. A rule with
`quiet_ms` in place of a message selector assigns its `state` once per
connection, when the stream (`line-tcp`, `osc-tcp` or `ws`) has been silent
that long after the device last sent anything; each message starts the
silence again. It matches no message, takes no captures, and stands beside
the rule for the marker, which still applies at once where it is sent:

```yaml
    - match: "^END PRELUDE:$"
      state: { device.prelude_complete: "true" }
    - quiet_ms: 1000
      state: { device.prelude_complete: "true" }
```

Nothing is timed until the device has sent something. A spec has at most
one such rule, with `state` only (no `then_send` or `replace`). Choose a
silence longer than any pause within the dump; a value written this way only
says the device went quiet, so its description says so. A quiet rule has no
telemetry vector: the engine's own tests cover the timing.

#### Re-reads on a push

A push that only says something changed (OpenLP's websocket counts changes
to the live item and the service, without their content) has the rule that
matches it ask for the rest with `then_send`: messages, like poll items,
queued when the rule matches.

```yaml
  updates:
    - json_match: { "$.results.counter": "^\\d+$" }
      json: { counter: "$.results.counter" }
      state: { live.counter: "{counter}" }
      then_send:
        - { method: GET, path: /api/v2/controller/live-item }
        - { method: GET, path: /api/v2/service/items }
```

They go through the command queue as poll items do, behind commands, and
their replies go to the rules. A message still waiting in the queue is not
queued again, so a burst of pushes asks for one re-read; one already sent is
answered before the next is sent, so a change that arrives meanwhile is read
too. A rule with `then_send` may have no `state`. The messages are templates
over the rule's captures (`{1}`, its `json` and `headers` names) and the
settings, so a push can name what to read or a request can carry what the
push gave. Twitch's EventSub websocket welcomes each connection with a
session id, and subscriptions for that session must be created within ten
seconds:

```yaml
    - json_match: { "$.metadata.message_type": "^session_welcome$", "$.payload.session.id": "^(.+)$" }
      json: {}
      then_send:
        - { method: POST, path: /helix/eventsub/subscriptions, content_type: application/json,
            body: '{"type":"stream.online","version":"1","condition":{"broadcaster_user_id":"{settings.broadcaster_id}"},"transport":{"method":"websocket","session_id":{1:json}}}' }
```

Nothing is queued while the device is open for commands only. A telemetry
vector names what a message queues with `expect_then_send`, a list of
requests as `expect_request` gives them, or of line messages as text; binary
messages (OSC) are `expect_then_send_hex`, each framed as it is sent.

#### An event stream beside an HTTP transport

`telemetry.sse` opens a server-sent event stream (`text/event-stream`) on the
http transport's host, with its scheme and credential, when the device is
opened, and reopens it with backoff when it closes. Like the push websocket it
carries no commands and doesn't decide whether the device is connected. A
refused credential (`refusal_status`) is the terminal refusal of §2.

```yaml
telemetry:
  sse:
    path: /api/v1/events      # required
  updates:
    - json_match: { "$.event": "^brightness$" }
      json: { value: "$.data.value" }
      state: { output.brightness: "{value}" }
```

Each event goes to the `json_match` rules as `{"event": <name>, "data":
<data>}`, with `data` parsed where it is JSON (the event name is `message`
when the stream gives none), and its data to the text rules.

A stream opened with a request of its own takes `method` (`GET`, the
default, or `POST`), a `body`, a template over settings, and its
`content_type`. A service whose stream is chunked JSON rather than
event-stream, one document per part with a blank line between parts, says
`format: json`: the stream is asked for as `application/json`, and each
part goes to the `json_match` rules as the document it is (not as `{event,
data}`) and to the text rules as its text. With `as_reply`, each part (or,
for event-stream, each `{event, data}`) is also offered to the `path`
rules as the reply to a GET of the path at `as_reply.path`, with the value
at `as_reply.body` as its body, so the rules that read a poll's reply read
the stream's updates too, and the polls can stay as the fallback.
ProPresenter's update stream names each part's endpoint in `url`:

```yaml
telemetry:
  sse:
    method: POST
    path: /v1/status/updates
    body: '["status/slide","timer/video_countdown"]'
    content_type: application/json
    format: json
    as_reply: { path: "$.url", body: "$.data" }
```

A path without a leading `/` is given one.

A telemetry vector for an event stream gives `inbound_sse: { event, data }`,
one event as the stream would deliver it (`event` defaults to `message`);
for `format: json`, `data` is the part's text.

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

A native module can drive a device on a serial port of the host as well
(`Cx::serial_open`, reported like a TCP connection). Its spec then lists a
`serial` entry in `ports`, usually with a `when` naming the setting that
chooses it, and declares the six serial settings of `line-serial` (§2) with
the device's defaults; `serial_port` is then optional, required only when
the serial port is chosen.

Every spec, spec-driven or native, is one integration, and each integration
is a Cargo feature of the core named after its spec id (`sennheiser-ew-dx`,
`shure-wireless`, `sony-camera`, ...). A build embeds the specs, and compiles
the native modules, native extensions and discovery protocols, of exactly the
integrations it enables; vendor groups (`vendor-sennheiser`) and `all` enable
several at once. The spec engine and transports are shared and always built.
A core can further be started for named integrations only (`devices` in its
options: spec ids, vendor groups or `all`), and then its catalogue lists only
those.

### Streams

A native module can publish continuous media, such as a camera's live view.
The spec declares each stream under `streams`, keyed by a stream name in
`snake_case`, so consumers can find it in the catalogue:

```yaml
streams:
  live:
    format: jpeg                  # every frame is one complete JPEG image
    summary: The camera's live view, while watched
    models: [ilme-fx6]            # optional; every model when absent
```

`format` is from a closed set: `jpeg` (one complete JPEG, ITU-T T.81, per
frame), or `meros-preview` (one frame of a Meros Camera's preview exactly as
the camera frames it: a u32 length and an i64 presentation time in
microseconds, both big-endian, a flags byte whose bit 0 marks a keyframe,
then one H.264 or HEVC access unit in Annex-B; the consumer decodes it).
The HTTP service serves a `jpeg` stream as `.mjpg` (MJPEG) and `.jpg` (one
frame), and a `meros-preview` stream as `.frames`
(`application/x-meros-preview`): the frames back to back in that framing,
from a keyframe, and after frames the reader missed again only from the next
keyframe, so a decoder never sees a broken group of pictures. Each extension
answers 406 for a stream of the other format.
`live` is the conventional name for a device's main picture, so a consumer
can show a preview of any device without knowing it.

A stream runs only while it is watched. The module learns that through its
`stream_watch` callback, called once when a stream gains its first watcher and
once when it loses its last, and publishes frames in between with
`cx.frame(stream, format, data)`. A module that has to start something on the
device to produce frames (a camera's live view mode) starts it there and stops
it when the last watcher leaves.

Frames do not travel with events. Each watcher holds at most one undelivered
frame: a newer frame replaces it and is counted, so a slow consumer gets the
latest picture, never a backlog, and memory stays bounded. Every frame carries
a `sequence` that rises by one per frame published and a `dropped` count of the
frames this watcher missed since its last. Only a device implemented natively,
or through a native extension that publishes frames, can declare `streams`; the
spec engine publishes none.

### Native extensions

A spec-driven device can name one native `extension` for a single thing the
format cannot express, while its commands, queries and telemetry rules stay in
the spec:

```yaml
extension: panasonic-update-notification
```

The extension is a small Rust module that wraps the spec engine. The set is
closed and each is named in the spec, so it is never hidden. Most only keep
state current, and a device opened for commands only runs without them. One
that answers commands of its own (Ultimatte's preset archive) runs either
way: the spec declares those commands with their parameters and `returns`
and no `send`, and `tools/validate.py` checks that the spec declares every
command the extension answers and gives none of them a message. A transfer
that can outlast a command's timeout answers at once with `{transfer,
ahead}` and runs in the background, followed in `transfers.<number>` of the
state and ended by `transfer_started` and `transfer_finished` message
events, as the ATEM's do.

| Extension | Adds |
|---|---|
| `panasonic-update-notification` | Panasonic AW-series cameras' update notifications: registers a local TCP port with the camera (`/cgi-bin/event?connect=start`), receives the changed settings it pushes there, and passes each one, a response text such as `p1`, to the spec's telemetry rules. Registers again when the camera's 60-second version notices stop, and unregisters on closing |
| `aja-config-events` | AJA's event connection (Ki Pro, KUMO): sends the spec's `open_event_connection` request, then its `wait_for_events` request with the returned connection id, again as each reply arrives, beside the command queue. Each element of the reply, `{param_id, param_type, int_value, str_value}`, is offered to the spec's telemetry rules as two text messages, `event <param_id> value=<value>` and `event <param_id> value_name=<name>`: as a `/config?action=get` reply gives them, a string parameter's value is `str_value` and its name empty, and any other's value is `int_value` and its name `str_value`, or the value when that is empty. Opens a new connection when the id has expired or a request fails, after 1 s doubling to 30 s |
| `resolume-push` | Resolume Arena and Avenue's websocket API (`ws://host:port/api/v1`): reads the composition Resolume sends on connecting and after each structural change into state keyed by unique id (composition, layers, columns, clips, decks, layer groups, and the effects in their chains), subscribes to every parameter that state shows with `{"action":"subscribe","parameter":"/parameter/by-id/<id>"}`, and applies each `parameter_subscribed` and `parameter_update`. A new composition is diffed against the last: removed items leave state and are unsubscribed, new ones are subscribed. Reads the composition again over REST after a deck switch, after a structural command of the session's own, and as a slow safety refresh (`composition_refresh_s`) for a deck's `closed`, which is not a parameter. Reopens the websocket after 1 s doubling to 30 s |
| `hyperdeck-ftp` | Blackmagic HyperDeck's FTP server (TCP 21, `ftp_port`; anonymous unless `ftp_username` is set) for the files on its media: `list_files` (MLSD, or LIST in the Unix or DOS form), `delete_file`, `rename_file`, `create_folder`, each answered when done on a connection of its own, and `download_file` and `upload_file`, one at a time in the background on another, with `cancel_transfer`. Passive data connections (PASV, EPSV over IPv6) always go to the control connection's peer, whatever address the reply names. An upload's or a rename's target is checked with SIZE first and replaced only with `overwrite`; a download is checked against SIZE. Each connection logs in, works and quits; a reply is waited for 15 s, a transfer may be silent 30 s |
| `ultimatte-preset-archive` | Blackmagic Ultimatte 12's binary settings channel (TCP 9996, `archive_port`), which the vendor's Smart Remote 4 and Software Control use for Archive All and Restore: `export_presets` reads every preset of the FILE LIST the unit last sent on its text port, and its `GPISettings` and `SavedSettings`, one request per connection, into a zip in Archive All's layout on the host; `import_presets` writes such a zip's presets in name order, then `GPISettings`, then `SavedSettings`, stopping at the first refusal; `export_preset`, `import_preset`, `export_settings` and `import_settings` move one of them to or from a host file. A connection that does not open, or a read the unit drops unanswered, is tried once more after 250 ms |

## HTTP service

The HTTP service (`meros-integrations serve`, `crates/sidecar`) offers the
core's API over HTTP for hosts that cannot link it; its routes are listed in
`crates/sidecar/src/main.rs`. Every route needs `Authorization: Bearer
<token>`, and every JSON body is the one the other deliveries produce.

### File transfers

Some commands read or write a file on the host running the core: their
`path` (or multi-line `paths`) parameter is a host path, for instance an
ATEM's `upload_still`, `download_still`, `upload_clip`, `download_clip`,
`upload_clip_audio`, `upload_macro`, `download_macro` and
`upload_multiviewer_label`, a Sony camera's `import_*`, `export_*`,
`apply_settings_file`, `download_content` and `download_captured_image`, an
Ultimatte's `export_*` and `import_*`, and a HyperDeck's `download_file` and
`upload_file`. A client of the HTTP service
on another machine runs them through the service's transfer folder, never
naming a real path:

| Route | Does |
|---|---|
| `POST /v1/files?name=<name>` | Streams the request body to a new file. 201 with `{file, name, path, size}`: `file` is an opaque id (32 hex digits), `path` is `transfer:<file>` |
| `POST /v1/files/reserve` `{count?, name?}` | Ids for files a command will write (`count` 1 to 65535, default 1). 201 with `{files: [...]}` |
| `GET /v1/files` | `{files, used}`; each file with its `size` and whether it is `written` yet; `used` is their total in bytes |
| `GET /v1/files/<file>` | The file's bytes (`application/octet-stream`); 404 until written. `?delete=true` removes it once sent in full |
| `DELETE /v1/files/<file>` | `{deleted: true}`; 409 while it is being downloaded |

In a command's params, a line `transfer:<file>` in any string parameter is
replaced by that file's real path before the command reaches the core, one
per line in a multi-line parameter; other lines pass unchanged. A `binary`
parameter (§4, "Binary request bodies") takes an uploaded file the same way,
as `{"file": "transfer:<file>"}`, and the core streams it from the folder to
the device, so the file never travels inside `/v1/execute`'s JSON. An id the
service did not hand out refuses the command with `invalid_params`. Every
real path of the folder in a command's result, an event or a snapshot reads
`transfer:<file>` again. For a command that writes a file, reserve an id,
pass its `path`, and fetch the file once the command (for an ATEM or a
HyperDeck, its transfer's `transfer_finished` event) is done.

A file is stored as `<folder>/<file>/<name>`, where `<name>` is the client's
name with only letters, digits, space and `._()-` kept (`file` when none is
given), so a command that defaults a name from the file or checks its
extension sees the client's. Only ids the service handed out are joined to
the folder's path, and a link in the folder is never followed.

The service sets no limit on a file's size, on the folder's total or on how
long a file is kept: uploads and downloads are streamed with no length
assumed, whether the body comes with a length or chunked, and the only bound
is the host's disk. Limiting what a client may send, and removing files it no
longer needs, is the client's call: a file is kept until it is deleted or
fetched with `?delete=true`. An upload cut off before its end is not kept.
`POST /v1/execute` has no body limit either, since a command's `binary`
parameters (§4, "Binary request bodies") may carry a file's bytes in it as
base64; `{"file": "transfer:<file>"}` keeps them out of it.
The folder (`--transfer-dir`, default
`<temp>/meros-integrations-transfer-<port>`) is cleared of its ids at startup
and on a clean stop, when the default folder is also removed if nothing else
is left in it.

The core likewise sets no limit of its own on a host file a command reads or
writes: a device's own limits (an ATEM's media pool, a Sony camera's largest
operation) are the device's, and its refusal is the command's or the
transfer's error.
