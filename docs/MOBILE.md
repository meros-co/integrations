# Phones and tablets

The core and its C binding build for Android (`aarch64-linux-android`,
`armv7-linux-androideabi`) and iOS (`aarch64-apple-ios`). A mobile app links
the C library (`bindings/c`) and calls it from its own code: through JNI on
Android, directly from Swift or Objective-C on iOS. This page covers building
it, what the app itself must do that the core cannot (permissions,
multicast, background), and how the tally integrations behave on a phone.

CI builds both Android targets and `aarch64-apple-ios`, each with the tally
integrations below and with `all` (`.github/workflows/ci.yml`, job `mobile`).

## Choosing integrations

Build in only what the app uses. For tally:

```
--no-default-features --features meros-integrations/blackmagic-atem,meros-integrations/vmix,meros-integrations/obs-studio,meros-integrations/newtek-tricaster,meros-integrations/rosstalk,meros-integrations/tsl-umd-listener,meros-integrations/omt,meros-integrations/omt-discovery
```

`meros-integrations/all` builds too; the library is then several times
larger.

## Android

Install the Rust targets and an Android NDK (r27 or later is what has been
built with):

```
rustup target add aarch64-linux-android armv7-linux-androideabi
```

The NDK's clang is needed twice: as the linker, and as the C compiler for
`ring` (the TLS library's cryptography). The simplest way is
[cargo-ndk](https://github.com/bbqsrc/cargo-ndk), which sets both:

```
cargo ndk -t arm64-v8a -t armeabi-v7a -P 24 -o app/src/main/jniLibs \
  build --release -p meros-integrations-c --no-default-features --features <integrations>
```

Without it, name the tools in `.cargo/config.toml` (paths are the NDK's
`toolchains/llvm/prebuilt/<host>/bin`; on Windows the compilers end in
`.cmd`):

```toml
[target.aarch64-linux-android]
linker = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"

[target.armv7-linux-androideabi]
linker = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi24-clang"

[env]
CC_aarch64_linux_android = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
AR_aarch64_linux_android = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
CC_armv7_linux_androideabi = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi24-clang"
AR_armv7_linux_androideabi = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
```

then `cargo build --release --target aarch64-linux-android -p meros-integrations-c ...`.
The build gives `libmeros_integrations_c.so` (load it with
`System.loadLibrary("meros_integrations_c")`) and a static
`libmeros_integrations_c.a` for linking into the app's own JNI library.

- **minSdk 24.** The core reads the network interfaces with `getifaddrs`,
  which Android has from API level 24 (Android 7.0).
- The library has no JNI entry points of its own: the app's JNI code calls
  the C functions in `include/meros_integrations.h` and passes the JSON
  strings across.
- The `.so` needs only libc, libm and libdl. Linking the static library into
  the app's own JNI library also takes `-llog -lunwind -ldl -lm -lc`.

## iOS

On a Mac with Xcode:

```
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo build --release --target aarch64-apple-ios -p meros-integrations-c --no-default-features --features <integrations>
cargo build --release --target aarch64-apple-ios-sim -p meros-integrations-c --no-default-features --features <integrations>
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libmeros_integrations_c.a -headers bindings/c/include \
  -library target/aarch64-apple-ios-sim/release/libmeros_integrations_c.a -headers bindings/c/include \
  -output MerosIntegrations.xcframework
```

Link the static library (`libmeros_integrations_c.a`); App Store apps cannot
load a `.dylib` outside a framework. Set `IPHONEOS_DEPLOYMENT_TARGET` to the
app's deployment target when building. No framework beyond the system
library is known to be needed (TLS uses rustls with built-in root
certificates, and randomness comes from libSystem); add `-framework Security`
only if the linker asks for it. Add `x86_64-apple-ios` for Intel simulators.

## What the app must do

The core opens sockets; it cannot ask the operating system for permission to
use the network. The app does that:

| | Android | iOS |
|---|---|---|
| Any network use | `android.permission.INTERNET` in the manifest. | `NSLocalNetworkUsageDescription` in Info.plist. iOS asks the user the first time the app reaches the local network; until they allow it, connections to local devices fail and the core keeps retrying (call `mi_reconnect_all` once access is granted so it does not wait out a retry delay). |
| Multicast and broadcast (only the core's own discovery: `mi_discover` with mDNS, used for ATEM and Meros Camera) | `android.permission.CHANGE_WIFI_MULTICAST_STATE`, and hold a `WifiManager.MulticastLock` while discovering; without it the Wi-Fi driver drops multicast. | The `com.apple.developer.networking.multicast` entitlement, which Apple grants on request. |
| Finding devices without it | Android's `NsdManager` browses DNS-SD (`_blackmagic._tcp` for ATEM, `_meros-cam._tcp` for Meros Camera, `_omt._tcp` for OMT) without a multicast lock. | `NWBrowser` (Network framework) browses them with only the local network permission; list the types under `NSBonjourServices`. |
| Keep running in the background | A foreground service (type `connectedDevice` or `dataSync`) with its notification; otherwise Doze and App Standby cut the app's network. A `WifiManager.WifiLock` (low-latency mode) keeps Wi-Fi power saving from delaying tally. | iOS suspends an app shortly after it leaves the screen; its sockets stop, and the OS may reclaim them. There is no background mode for this kind of traffic. |
| On resume or network change | Call `mi_reconnect_all` from `onResume` and from a `ConnectivityManager.NetworkCallback`. | Call `mi_reconnect_all` on `sceneWillEnterForeground` / `applicationWillEnterForeground` and when an `NWPathMonitor` reports a change. |

Plain HTTP to a device (TriCaster, port 80) is not affected by App Transport
Security or Android's cleartext policy: those govern the platforms' own HTTP
stacks, and the core uses its own sockets.

### Resuming

A suspended app's connections can be dead without either end having
noticed, and a core left to find that out by itself takes its modules' own
timeouts and retry delays (seconds to half a minute). `mi_reconnect_all`
(`Core::reconnect_all`) drops every device's connections and opens them
again at once; `mi_reconnect` does one device. Devices keep their ids and
last known state, and the state is read afresh on the new connections.
What the app asked for that a device forgets with its connection (an OMT
receiver's tally, an ATEM audio level subscription) is asked for again.
Every delivery has the same two calls, for a mobile consumer built on one
of the other packages or on the HTTP service:

| Delivery | One device | Every open device |
|---|---|---|
| Rust | `Core::reconnect(device)` | `Core::reconnect_all()` |
| C | `mi_reconnect(core, device)` | `mi_reconnect_all(core)` |
| Node | `await core.reconnect(device)` | `await core.reconnectAll()` |
| Python | `core.reconnect(device)` (`reconnect_async`) | `core.reconnect_all()` (`reconnect_all_async`) |
| HTTP service | `POST /v1/reconnect` `{device}` | `POST /v1/reconnect_all` |

Each returns once the devices have restarted, not once they are connected;
the single-device call answers false (`{"reconnected": false}`) for a
device that is not open.

Without it the core still recovers by itself:

- Every TCP connection sends keepalive probes after 10 s of silence, so a
  peer that vanished is dropped within about half a minute even where the
  protocol never replies (RossTalk, OMT, an incoming TSL connection).
- Each module's own liveness check (ATEM: 5 s without data; vMix and OBS:
  their polls; TriCaster: its HTTP reads) drops a silent connection, and the
  module reconnects with a growing delay (1 s, doubling to 30 s).
- A listening port that fails (a UDP port for TSL, a TCP listener) is closed
  and opened again by the listener, with the same delays.

### Threads

Every C call blocks; never make one on the main thread (Android reports the
app as not responding, iOS's watchdog ends it). `mi_open` resolves a host
name before returning, so pass IP addresses where possible. Android's
resolver does not answer `.local` names; resolve them with `NsdManager`
first. Read events with `mi_wait_events` on a thread of the app's own (see
`bindings/c/README.md`).

### Addresses

Leave the core's `bind_address` unset on a phone: the Wi-Fi address changes
with the network, and a core bound to an old one has to be recreated.
Devices are reached over IPv4: host names resolve to IPv4 addresses only,
and the listeners and shared ports below are IPv4. A device given as an
IPv6 literal is reached over IPv6 for TCP, HTTP, WebSockets and its own UDP
socket.

## The tally integrations on a phone

| Integration | Transport | On a phone |
|---|---|---|
| `blackmagic-atem` | UDP 9910, from an ephemeral port the core binds | Unicast only; needs no multicast. Tally (program and preview per input) arrives as the switcher pushes it. 5 s without data starts a new session on a fresh socket. Discovery by mDNS needs multicast (above); a platform DNS-SD browse for `_blackmagic._tcp` avoids it. |
| `vmix` | TCP 8099 | `SUBSCRIBE TALLY` pushes tally; the XML read every `state_poll_ms` (1 s by default) is also the liveness check. Raise `state_poll_ms` to save battery; tally still arrives as pushed. |
| `obs-studio` | WebSocket 4455 | Tally comes from the program and preview scenes (`program_scene`, `preview_scene`) and the sources in them (`scene_items`), pushed as they change. A request every 5 s is the liveness check. |
| `newtek-tricaster` | HTTP 80 and its change-notification WebSocket | Tally is read on each tally change notification. |
| `rosstalk` | TCP 7788 | Commands only: RossTalk reports no state, so no tally. For tally from a Ross switcher, have it send TSL UMD (a Carbonite sends TSL 3.1 over TCP or UDP) to the phone and open `tsl-umd-listener` beside it (model `tsl-umd-3-tcp`, or `tsl-umd` for UDP; `specs/rosstalk.yaml` quirks). |
| `tsl-umd-listener` | UDP, or TCP that the switcher connects to, on the port given at open | The core listens, so the switcher must be set to send to the phone's address, which changes with DHCP: reserve one for it. Use a port of 1024 or above: Android refuses lower ports to apps, and iOS can too. The port is bound on every interface (IPv4), and only datagrams or connections from the switcher's address are taken. |
| `meros-camera` | HTTP 8790 (the port in the camera's `_meros-cam._tcp` SRV record), outgoing only | Tally, recording and the camera's health arrive on its event stream, a line per change and at least every 5 s; 10 s without one reads the status again and reopens the stream. `set_tally` makes the app the camera's tally source; `auto` hands it back. The preview runs only while the `live` stream is watched; on the HTTP service it is `GET /v1/devices/{device}/streams/live.frames`, starting on a keyframe and resuming at the next keyframe after frames a slow reader missed. `mi_reconnect_all` on resume reopens the event stream, and the preview while watched, at once. |
| `omt` | TCP to the source, 6400 and up | The source sends its combined tally to subscribed receivers. A tally this app sets (`set_tally`) is sent again after every reconnection. |
| `omt-discovery` | TCP 6399 to a discovery server | Lists sources with no multicast. Without a discovery server, browse `_omt._tcp` with the platform's DNS-SD. |
| `tally-arbiter` | WebSocket 4455 (Socket.IO), outgoing only | With `device_id` set, the app registers as a listener client and that device's tally is pushed; every device's tally is pushed when signed in with an admin or producer account, and otherwise read every `poll_ms` (raise it to save battery). A version request every 5 s is the liveness check; `mi_reconnect_all` on resume reopens the connection and registers the listener client again. |

## Serial ports

Phones have no serial port an app can open: Android reaches a USB serial
adapter only through its Java USB host API, and iOS not at all. The serial
transport (`line-serial`, link serial of a spec with a `serial` block, and
the serial link of the native modules that offer one) is therefore left out of Android and iOS builds,
which do not depend on the serial crate: a device opened on a serial port
stays `disconnected`, its reason saying serial ports are not available on
this platform, and the list of the host's serial ports (`serial_ports` in
every delivery) is empty. Everything else in those integrations builds and works over
the network as elsewhere.

## MIDI ports

The MIDI integrations (`generic-midi`, `midi-show-control`) reach a device
over an RTP-MIDI session on the network, which works on a phone as
anywhere, or on a MIDI port of the host (`link` `port`). MIDI ports are
opened with the `midir` crate:

- **iOS** has CoreMIDI, which `midir` uses as on macOS: a USB MIDI device
  on the camera connection kit or USB-C, a Bluetooth MIDI device paired in
  the app, or another app's port is listed and opened. The app needs no
  entitlement for it. This has not been built for iOS here; CI builds it.
- **Android** reaches MIDI only through its Java `android.media.midi` API,
  which a native library cannot start on its own, so MIDI ports are left
  out of Android builds: a device opened on a MIDI port stays
  `disconnected`, its reason saying MIDI ports are not available on this
  platform, and the list of the host's MIDI ports (`midi_ports` in every
  delivery) is empty. RTP-MIDI works.

## What has been verified

- Android: `cargo build` of the core and the C binding (including linking
  the `.so`) for `aarch64-linux-android` and `armv7-linux-androideabi`, with
  the tally integrations and with `all`, using NDK r28.
- iOS: `cargo check` of the core and the C binding, and a build of the
  static library, for `aarch64-apple-ios` with `all` and with the tally
  integrations. The static library has not been linked into an app, and
  nothing has been run on a device; CI builds it with Xcode.
- Reconnecting: loopback session tests (`crates/core/tests/resume_session.rs`)
  pause a simulated vMix without closing its connection, and check that the
  core notices, reconnects when it answers again, and that
  `Core::reconnect` replaces a stale connection at once; and that a TSL
  listener hears the switcher again after reconnecting.
