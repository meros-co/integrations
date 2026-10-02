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

`examples/smoke.c` builds against the header and the static library and checks
the interface end to end. `cargo test -p meros-integrations-c` plays the shared
binding script through the same functions.

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
