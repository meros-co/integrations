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

A device whose error answer names nothing (Shure's `< REP ERR >`) declares it
as the transport's `error_match`, a regex: such a message is taken as the
reply of a command waiting with `reply_contains`, which then fails on its
`expect`.

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

The session is a secret: the login's answer is never offered to the
telemetry rules, and the session is never logged or kept in state.
`update_settings` drops it, so the next request logs in with the new
credential. A login that is the same for every model is one request; a list
names each request's `models` (§4, "Messages per model").

A device opened by host name rather than address keeps the name in its HTTP,
websocket and event-stream URLs, so the certificate is checked against the
name and `Host` carries it. A cloud API is opened that way: Planning Center's
`api.planningcenteronline.com` or YouTube's `www.googleapis.com` on port 443.

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
device pushes and what it sends back goes to the rules. Pings are answered by
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
| `protocol` | `tcp`, `udp`, `http`, `https`, `ws`, `wss`, `tls` or `ssh` |
| `role` | `control` (where commands go; the port given when opening a device overrides it), `push` (a websocket or event stream beside it), `feedback` (where the device sends replies or changes), `notification` (pushed status the device starts), `discovery` |
| `listener` | Who listens: `device` (the default) or `core` |
| `setting` | The setting that changes this port, if any |
| `when` | When the entry applies, if not always: a setting's value or a model |

There is always at least one `control` entry. For a spec-driven device the
transport's `port` must be one of them, `listen_port` needs a `feedback`
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
`line-udp`), a login step can declare the text a device answers a failed login
with, even when it answers in free text. A device whose console echoes free
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

The label is a short noun phrase in sentence case, the way a form names the
field. The description is one plain sentence on what the value means: its
unit, how it is numbered, what its ends or values do. It shouldn't just
restate the type and range.

### Templates

Every string in `send`, `on_connect` and `probe` is a template. Substitution is
`{param}` for a command parameter or `{settings.name}` for a setting, optionally
followed by directives after `:`. A name is made of letters, digits, `_` and
`.`, so any other brace is literal: `{"status":"toggle"}` is sent as written.

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
| `-1`, `+1`, … | Integer offset applied before formatting; combines as `{preset:-1:02d}` |
| `signed` | Integer with an explicit leading sign: `7` → `+7`, `-7` → `-7` |
| `.1f`, `.2f`, … | Float with a fixed number of decimals, rounded half away from zero: `{level:.2f}` → `0.75`. In a telemetry value, a whole number a device sends where it means a decimal is shown the same way (`60` → `60.00`) |
| `md5`, `sha256` | String hashed, as lowercase hex: `{settings.password:md5}` → `e3afed0047b08059d0fada10f400c1e5` for `Admin`. For a device that takes a password hashed (Magewell's login) |
| `to.<name>` | A number converted to the wire by the named conversion (below): `{level_db:to.x32_fader}` |
| `from.<name>` | A wire number converted back to the operator's value: `{arg0:from.x32_fader:.1f}` |
| `map.<name>` | A value replaced by its wire text from the named value table (below): `{level:map.reference_level}`. The only directive on the placeholder |

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

A message naming `models` is sent only to those models; one without is sent
to every model. This is decided from the model the device was opened as,
before anything is sent, like `when_set` (§2), never from what the device
says. It applies to the object messages of `send` (HTTP requests and OSC
messages), telemetry `poll` and `subscribe` items, `on_connect` steps, the
`probe` (a list whose first message for the model is used) and a session's
`login`. `tools/validate.py` checks that every model a command `supports`
has a message.

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
  json_path: "$.transport.status"   # JSON: the HTTP body, or the OSC argument at `arg`
  json_equals: { "$.status": "ok" } # JSON values the reply must hold
  code_range: [200, 299]    # leading numeric response code
  code_path: "$.status"     # with code_range: the code is this JSON number instead
  header: Location          # HTTP only: this response header's value is returned
  address: /ch/01/config/name       # OSC only: the reply's address
  arg: 0                    # OSC only: argument returned as the value
  reply_json: { "$.id": "{id}" }    # ws only: what identifies the reply (§2)
  reply_contains: "REP {channel:02d} AUDIO_MUTE "  # line transports: text the reply holds (§2)
  convert: x32_fader        # the returned value, converted from the wire (§4)
```

Some OSC devices answer with JSON inside a string argument: QLab replies on
`/reply/<address>` with `{"status": "ok", "data": ...}`. On an OSC reply,
`json_path` and `json_equals` read the string argument at `arg` as JSON, so
`json_path: "$.data"` returns the data and `json_equals: { "$.status": "ok" }`
fails the command on an error or denied reply.

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
| `address` + `json` | As `address`, where the string argument at `json_arg` (default 0) holds JSON | as `address`, plus `json` names by JSON path |

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
name something else. Within one message a removal wins over a value the same
message assigns under it. A telemetry vector for a removal gives
`state_before`, the state the message arrives on.

Over HTTP, `poll` requests and command requests alike have their replies
offered to `path` rules, which match the request's path and query:

| Rule | Matches | Captures |
|---|---|---|
| `path` + `json` | A JSON reply | `json` names values by JSON path (`$`, `$.a.b`) |
| `path` + `json` + `json_each` | A JSON reply holding an array at `json_each` | once per element; `json` paths relative to it |
| `path` + `xml_each` | An XML reply | once per element of that name; its attributes |
| `path` + `json` + `json_match` | A JSON reply whose value at each JSON path matches its regex | the path's, then `json_match`'s, in order |
| `path` + `json` + `request_match` | A JSON reply to a request whose JSON body matches | the path's, then `request_match`'s, then `json_match`'s |
| `path` + `headers` | Any reply, JSON or not | `headers` names response header values, by header name in any case; with `json` too, both |

`request_match` is for protocols whose replies all arrive on one path and
don't say what they answer, such as JSON-RPC: the rule looks at the request
that the reply answers.

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

A telemetry vector for HTTP gives `inbound_http: { path, body }` in place of
`inbound`, plus `request` (the JSON request body) for a `request_match` rule
and `headers` (name to value) for a `headers` rule.

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
requests as `expect_request` gives them.

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

A telemetry vector for an event stream gives `inbound_sse: { event, data }`,
one event as the stream would deliver it (`event` defaults to `message`).

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
frame). `live` is the conventional name for a device's main picture, so a
consumer can show a preview of any device without knowing it.

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
closed and each is named in the spec, so it is never hidden:

| Extension | Adds |
|---|---|
| `panasonic-update-notification` | Panasonic AW-series cameras' update notifications: registers a local TCP port with the camera (`/cgi-bin/event?connect=start`), receives the changed settings it pushes there, and passes each one, a response text such as `p1`, to the spec's telemetry rules. Registers again when the camera's 60-second version notices stop, and unregisters on closing |
| `aja-config-events` | AJA's event connection (Ki Pro, KUMO): sends the spec's `open_event_connection` request, then its `wait_for_events` request with the returned connection id, again as each reply arrives, beside the command queue. Each element of the reply, `{param_id, param_type, int_value, str_value}`, is offered to the spec's telemetry rules as two text messages, `event <param_id> value=<value>` and `event <param_id> value_name=<name>`: as a `/config?action=get` reply gives them, a string parameter's value is `str_value` and its name empty, and any other's value is `int_value` and its name `str_value`, or the value when that is empty. Opens a new connection when the id has expired or a request fails, after 1 s doubling to 30 s |
| `resolume-push` | Resolume Arena and Avenue's websocket API (`ws://host:port/api/v1`): reads the composition Resolume sends on connecting and after each structural change into state keyed by unique id (composition, layers, columns, clips, decks, layer groups), subscribes to every parameter that state shows with `{"action":"subscribe","parameter":"/parameter/by-id/<id>"}`, and applies each `parameter_subscribed` and `parameter_update`. A new composition is diffed against the last: removed items leave state and are unsubscribed, new ones are subscribed. Reads the composition again over REST after a deck switch, after a structural command of the session's own, and as a slow safety refresh (`composition_refresh_s`) for a deck's `closed`, which is not a parameter. Reopens the websocket after 1 s doubling to 30 s |
