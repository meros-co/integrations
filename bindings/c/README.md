# C interface

`include/meros_integrations.h` declares the interface; the library is built
from this directory's crate.

```
cargo build --release -p meros-integrations-c
# target/release/meros_integrations_c.lib (Windows) or libmeros_integrations_c.a
# target/release/meros_integrations_c.dll / libmeros_integrations_c.so / .dylib
```

Linking the static library also needs the system libraries Rust's standard
library and the network stack use:

| Platform | Libraries |
|---|---|
| Windows (MSVC, `/MD`) | `ws2_32 userenv ntdll bcrypt advapi32 kernel32 crypt32 secur32 ncrypt ole32 oleaut32 iphlpapi` |
| Linux | `-lpthread -ldl -lm` |
| macOS | `-framework Security -framework CoreFoundation` |
| Android | `-llog -lunwind -ldl -lm -lc` (from the NDK; API level 24 or later) |
| iOS | none beyond the system library |

Android (`aarch64-linux-android`, `armv7-linux-androideabi`) and iOS
(`aarch64-apple-ios`) builds, the permissions and background handling a
mobile app needs, and an xcframework recipe are in
[docs/MOBILE.md](../../docs/MOBILE.md).

`examples/smoke.c` builds against the header and the static library and checks
the interface end to end. `cargo test -p meros-integrations-c` plays the shared
binding script through the same functions.

## Choosing integrations

The library contains every integration unless you name the ones you want
when you build it. Name spec ids, vendor groups (`vendor-sennheiser`) or
`all`, each as a feature of the core (the list is in
`crates/core/Cargo.toml`):

```
cargo build --release -p meros-integrations-c --no-default-features \
  --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless
```

Integrations left out have neither their spec, their code nor their
dependencies in the library.

At run time a core can be narrowed further to some of the integrations the
library was built with:
`mi_core_create("{\"devices\":[\"sennheiser-ew-g3-g4\",\"shure-wireless\"]}", &error)`.
`devices` takes spec ids, vendor groups or `"all"`. The catalogue then lists
only those devices, `mi_open` of any other returns
`{"error":{"error":"not_selected"}}`, and discovery runs only the protocols
that find them. Opening a device whose integration was not built in returns
`{"error":{"error":"not_built","feature":"<spec id>"}}`, naming the feature
to build with.

`cargo test -p meros-integrations-c` plays the shared script, which needs a
build with every integration (the default).

## Serial ports

A device can be reached on a serial port of this machine (RS-232, RS-422, a
USB serial adapter): a serial-only device, or one whose spec offers both
links, chosen with the `link` setting (`"tcp"` or `"serial"`). The
`serial_port` setting names the port. `mi_serial_ports(core)` lists this
machine's ports to choose from, as `{"ok":[{"name":"COM3"},...]}` (names
only; empty on Android and iOS):
`mi_open(core, "{\"device\":\"extron-matrix\",\"model\":\"mmx-42-62\",\"settings\":{\"link\":\"serial\",\"serial_port\":\"COM3\"}}")`
needs no host.

## MIDI ports

A MIDI device can be reached on a MIDI port of this machine (a USB or other
class-compliant MIDI interface or instrument, or another program's port):
`generic-midi` and `midi-show-control` take the `link` setting `"port"` and
name the ports in `midi_input` and `midi_output`. `mi_midi_ports(core)`
lists this machine's ports to choose from, as
`{"ok":[{"name":"...","input":true,"output":true},...]}` (empty in a build
without a MIDI integration and on Android). Linux builds with a MIDI
integration link the ALSA library (`libasound2-dev` to build).

## Files sent to devices

A command's `binary` parameter (a show file, a media file, an image) takes
the file's bytes as base64 text or a file on this machine as
`{"file": "<path>"}` in the params JSON:
`mi_execute(core, device, "load_show_file", "{\"file\":{\"file\":\"D:/Shows/Main.watch\"}}")`.
The core reads the file from disk as the request goes out, never all at once,
so its size does not matter. A file that is missing or cannot be read returns
`{"error":{"error":"invalid_params"}}` naming the parameter (SPEC.md §4,
"Binary request bodies").

## Resuming

A host whose network changes, or that resumes after being suspended (a
mobile app coming back to the foreground), calls `mi_reconnect_all`: every
device drops its connections and opens them again at once, rather than
waiting for its own timeouts and retry delays to find out they are dead.
`mi_reconnect` does one device.

## Threads and streams

Every call blocks, so make calls from worker threads. The library never calls
back into the host: read events with `mi_wait_events` on a thread of your own,
and wake it with `mi_interrupt_events` to stop.

Streams (a camera's live view, listed under a device's `streams` in the
catalogue) are read the same way: `mi_stream_open` a stream, then
`mi_stream_wait` for frames on a thread of your own, freeing each with
`mi_frame_free`. Each open stream keeps only the newest frame, so a reader that
falls behind skips frames (counted in `dropped`) rather than queueing them.
`mi_stream_close` wakes a waiting reader from another thread; free the stream
with `mi_stream_free` once nothing uses it, and before `mi_core_free`.
