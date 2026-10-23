//! The host's MIDI ports on Linux: the ALSA sequencer, reached through
//! `libasound.so.2` loaded with `dlopen` the first time a MIDI port is listed
//! or opened, never linked. A Linux build therefore needs no ALSA headers,
//! and starts on a machine without the library; there the list of ports is
//! empty and opening one fails with a reason saying the library is missing.
//!
//! Only the few sequencer calls the ports need are declared here, from the
//! ALSA library's documented API (alsa-lib's "Sequencer interface" and
//! "MIDI Event Encoder/Decoder" reference): list the clients and their
//! ports, open a port of the core's own and connect it to the device's
//! input or output, send each message as one event and receive events as
//! bytes. Port names are the ones `midir` gives on Linux, `client:port
//! client-number:port-number`, so settings made before keep working.
//!
//! Every call goes through the table in [`Alsa`], so a test hands it a fake
//! sequencer and never needs the real library.

// Outside Linux this is compiled only for its tests, which use part of it.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_long, c_short, c_uint, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use tokio::sync::mpsc;

use super::{MidiPortInfo, MidiSink};
use crate::midi::Parser;
use crate::module::MidiPort;

/// The library loaded, by its soname (the runtime package: `libasound2` on
/// Debian and Ubuntu, `alsa-lib` on Fedora and Arch).
pub(super) const LIBRARY: &str = "libasound.so.2";

/// The client name other programs see for the core's connections.
const CLIENT: &CStr = c"Meros";

// Values from the ALSA sequencer headers (seq.h, seq_event.h).
const OPEN_OUTPUT: c_int = 1;
const OPEN_INPUT: c_int = 2;
const OPEN_DUPLEX: c_int = OPEN_OUTPUT | OPEN_INPUT;
const NONBLOCK: c_int = 1;
const CAP_READ: c_uint = 1 << 0;
const CAP_WRITE: c_uint = 1 << 1;
const CAP_SUBS_READ: c_uint = 1 << 5;
const CAP_SUBS_WRITE: c_uint = 1 << 6;
const TYPE_MIDI_GENERIC: c_uint = 1 << 1;
const TYPE_SYNTH: c_uint = 1 << 10;
const TYPE_APPLICATION: c_uint = 1 << 20;
const ADDRESS_SUBSCRIBERS: u8 = 254;
const ADDRESS_UNKNOWN: u8 = 253;
const QUEUE_DIRECT: u8 = 253;
const EVENT_SYSEX: u8 = 130;
const EVENT_NONE: u8 = 255;
const LENGTH_MASK: u8 = 3 << 2;
const LENGTH_VARIABLE: u8 = 1 << 2;
/// poll(2)'s POLLIN, the same on every Linux architecture.
const POLLIN: c_short = 1;
/// Linux's ENOSPC: the input buffer overran.
const ENOSPC: c_int = 28;

/// An ALSA object only ever handled by pointer (`snd_seq_t` and the rest).
#[repr(C)]
pub(super) struct Opaque {
    _private: [u8; 0],
}

/// `snd_seq_addr_t`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Addr {
    pub client: u8,
    pub port: u8,
}

/// `snd_seq_event_t`: 28 bytes, 4-aligned. The data union is 12 bytes; a
/// System Exclusive event keeps its length (u32) and a pointer to its bytes
/// there, packed (`snd_seq_ev_ext_t`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Event {
    pub kind: u8,
    pub flags: u8,
    pub tag: u8,
    pub queue: u8,
    pub time: [u32; 2],
    pub source: Addr,
    pub dest: Addr,
    pub data: [u32; 3],
}

impl Event {
    /// The bytes a variable-length event (System Exclusive) points at.
    ///
    /// # Safety
    /// The event must have come from the sequencer, whose buffer holds the
    /// bytes until the next event is read.
    unsafe fn external(&self) -> Option<&[u8]> {
        if self.flags & LENGTH_MASK != LENGTH_VARIABLE {
            return None;
        }
        let len = self.data[0] as usize;
        let at = self.data.as_ptr().cast::<u8>().add(4).cast::<*const u8>();
        let ptr = at.read_unaligned();
        if ptr.is_null() || len == 0 {
            return Some(&[]);
        }
        Some(std::slice::from_raw_parts(ptr, len))
    }
}

/// `struct pollfd`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(super) struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

type P = *mut Opaque;

macro_rules! table {
    ($($name:ident: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        /// The library's calls, each resolved from `snd_<field name>`.
        pub(super) struct Alsa {
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }

        impl Alsa {
            /// Resolve every call with `symbol`, which gives a null pointer
            /// for one the library does not have.
            pub(super) fn resolve(
                symbol: &mut dyn FnMut(&CStr) -> *mut c_void,
            ) -> Result<Alsa, String> {
                Ok(Alsa {
                    $($name: {
                        let name = concat!("snd_", stringify!($name), "\0");
                        let name = CStr::from_bytes_with_nul(name.as_bytes())
                            .expect("a symbol name ends in one NUL");
                        let found = symbol(name);
                        if found.is_null() {
                            return Err(format!(
                                "{LIBRARY} has no {}",
                                name.to_string_lossy()
                            ));
                        }
                        // SAFETY: the symbol is the documented function, whose
                        // C signature the field's type declares.
                        unsafe {
                            std::mem::transmute::<
                                *mut c_void,
                                unsafe extern "C" fn($($arg),*) $(-> $ret)?,
                            >(found)
                        }
                    },)*
                })
            }
        }
    };
}

table! {
    strerror: fn(c_int) -> *const c_char;
    seq_open: fn(*mut P, *const c_char, c_int, c_int) -> c_int;
    seq_close: fn(P) -> c_int;
    seq_set_client_name: fn(P, *const c_char) -> c_int;
    seq_client_info_malloc: fn(*mut P) -> c_int;
    seq_client_info_free: fn(P);
    seq_client_info_set_client: fn(P, c_int);
    seq_client_info_get_client: fn(P) -> c_int;
    seq_client_info_get_name: fn(P) -> *const c_char;
    seq_query_next_client: fn(P, P) -> c_int;
    seq_port_info_malloc: fn(*mut P) -> c_int;
    seq_port_info_free: fn(P);
    seq_port_info_set_client: fn(P, c_int);
    seq_port_info_set_port: fn(P, c_int);
    seq_port_info_get_client: fn(P) -> c_int;
    seq_port_info_get_port: fn(P) -> c_int;
    seq_port_info_get_name: fn(P) -> *const c_char;
    seq_port_info_get_capability: fn(P) -> c_uint;
    seq_port_info_get_type: fn(P) -> c_uint;
    seq_query_next_port: fn(P, P) -> c_int;
    seq_create_simple_port: fn(P, *const c_char, c_uint, c_uint) -> c_int;
    seq_connect_from: fn(P, c_int, c_int, c_int) -> c_int;
    seq_connect_to: fn(P, c_int, c_int, c_int) -> c_int;
    seq_event_output_direct: fn(P, *mut Event) -> c_int;
    seq_event_input: fn(P, *mut *mut Event) -> c_int;
    seq_poll_descriptors_count: fn(P, c_short) -> c_int;
    seq_poll_descriptors: fn(P, *mut PollFd, c_uint, c_short) -> c_int;
    midi_event_new: fn(usize, *mut P) -> c_int;
    midi_event_free: fn(P);
    midi_event_resize_buffer: fn(P, usize) -> c_int;
    midi_event_reset_encode: fn(P);
    midi_event_no_status: fn(P, c_int);
    midi_event_encode: fn(P, *const u8, c_long, *mut Event) -> c_long;
    midi_event_decode: fn(P, *mut u8, c_long, *const Event) -> c_long;
}

impl Alsa {
    fn error(&self, code: c_int) -> String {
        // SAFETY: snd_strerror returns a static string for any code.
        let text = unsafe { (self.strerror)(code) };
        text_of(text).unwrap_or_else(|| format!("error {code}"))
    }
}

fn text_of(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: ALSA returns NUL-terminated strings it keeps.
    Some(
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// The library, loaded once at first use; the error says why it could not
/// be, and is the same every time after.
pub(super) fn library() -> Result<&'static Alsa, String> {
    static LOADED: OnceLock<Result<Alsa, String>> = OnceLock::new();
    LOADED
        .get_or_init(|| load(LIBRARY))
        .as_ref()
        .map_err(Clone::clone)
}

/// Load `name` and resolve its calls. The library stays loaded for the life
/// of the process.
#[cfg(target_os = "linux")]
pub(super) fn load(name: &str) -> Result<Alsa, String> {
    let soname = CString::new(name).map_err(|e| e.to_string())?;
    // SAFETY: dlopen with a NUL-terminated name; the handle is never closed
    // once the calls are resolved, so they stay valid.
    let handle = unsafe { libc::dlopen(soname.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        // SAFETY: dlerror describes the dlopen that just failed.
        let why = text_of(unsafe { libc::dlerror() }).unwrap_or_default();
        return Err(not_loaded(name, &why));
    }
    let table = Alsa::resolve(&mut |symbol| {
        // SAFETY: a symbol name looked up in the handle opened above.
        unsafe { libc::dlsym(handle, symbol.as_ptr()) }
    });
    if table.is_err() {
        // SAFETY: nothing resolved from the handle is kept.
        unsafe { libc::dlclose(handle) };
    }
    table
}

/// Outside Linux the library is never there (only tests compile this).
#[cfg(not(target_os = "linux"))]
pub(super) fn load(name: &str) -> Result<Alsa, String> {
    Err(not_loaded(name, "not a Linux system"))
}

fn not_loaded(name: &str, why: &str) -> String {
    let mut reason = format!(
        "the ALSA library ({name}) could not be loaded, so MIDI ports cannot be opened \
         (install the libasound2 or alsa-lib package)"
    );
    if !why.is_empty() {
        reason.push_str(": ");
        reason.push_str(why);
    }
    reason
}

/// Wait up to `ms` for one of `fds` to be readable.
#[cfg(target_os = "linux")]
fn wait(fds: &mut [PollFd], ms: c_int) {
    // SAFETY: PollFd has struct pollfd's layout and `fds` is that long.
    unsafe {
        libc::poll(
            fds.as_mut_ptr().cast::<libc::pollfd>(),
            fds.len() as libc::nfds_t,
            ms,
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn wait(_fds: &mut [PollFd], ms: c_int) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}

/// An open sequencer client.
struct Seq {
    alsa: &'static Alsa,
    handle: P,
}

// SAFETY: a sequencer handle may be used from any thread, one at a time;
// each Seq is owned by one thread at a time.
unsafe impl Send for Seq {}

impl Seq {
    fn open(alsa: &'static Alsa, streams: c_int, mode: c_int) -> Result<Seq, String> {
        let mut handle: P = std::ptr::null_mut();
        // SAFETY: snd_seq_open writes the handle on success.
        let code = unsafe { (alsa.seq_open)(&mut handle, c"default".as_ptr(), streams, mode) };
        if code < 0 || handle.is_null() {
            return Err(format!(
                "the ALSA sequencer could not be opened: {}",
                alsa.error(code)
            ));
        }
        let seq = Seq { alsa, handle };
        // SAFETY: an open handle and a NUL-terminated name.
        unsafe { (alsa.seq_set_client_name)(handle, CLIENT.as_ptr()) };
        Ok(seq)
    }

    /// Every MIDI port of every client (the types `midir` lists too).
    fn ports(&self) -> Result<Vec<Found>, String> {
        let a = self.alsa;
        let mut client: P = std::ptr::null_mut();
        let mut port: P = std::ptr::null_mut();
        // SAFETY: the info objects are allocated here, used only with this
        // open handle, and freed by the guards.
        unsafe {
            if (a.seq_client_info_malloc)(&mut client) < 0 || client.is_null() {
                return Err("out of memory".into());
            }
            let _client = Free(client, a.seq_client_info_free);
            if (a.seq_port_info_malloc)(&mut port) < 0 || port.is_null() {
                return Err("out of memory".into());
            }
            let _port = Free(port, a.seq_port_info_free);
            let mut found = Vec::new();
            (a.seq_client_info_set_client)(client, -1);
            while (a.seq_query_next_client)(self.handle, client) >= 0 {
                let number = (a.seq_client_info_get_client)(client);
                let client_name = text_of((a.seq_client_info_get_name)(client)).unwrap_or_default();
                (a.seq_port_info_set_client)(port, number);
                (a.seq_port_info_set_port)(port, -1);
                while (a.seq_query_next_port)(self.handle, port) >= 0 {
                    let kind = (a.seq_port_info_get_type)(port);
                    if kind & (TYPE_MIDI_GENERIC | TYPE_SYNTH | TYPE_APPLICATION) == 0 {
                        continue;
                    }
                    let caps = (a.seq_port_info_get_capability)(port);
                    let client = (a.seq_port_info_get_client)(port);
                    let number = (a.seq_port_info_get_port)(port);
                    let port_name = text_of((a.seq_port_info_get_name)(port)).unwrap_or_default();
                    found.push(Found {
                        name: format!("{client_name}:{port_name} {client}:{number}"),
                        client,
                        port: number,
                        input: caps & (CAP_READ | CAP_SUBS_READ) == CAP_READ | CAP_SUBS_READ,
                        output: caps & (CAP_WRITE | CAP_SUBS_WRITE) == CAP_WRITE | CAP_SUBS_WRITE,
                    });
                }
            }
            Ok(found)
        }
    }

    /// A port of the core's own, named `name`.
    fn own_port(&self, name: &CStr, caps: c_uint) -> Result<c_int, String> {
        // SAFETY: an open handle and a NUL-terminated name.
        let port = unsafe {
            (self.alsa.seq_create_simple_port)(
                self.handle,
                name.as_ptr(),
                caps,
                TYPE_MIDI_GENERIC | TYPE_APPLICATION,
            )
        };
        if port < 0 {
            return Err(format!(
                "no ALSA port could be created: {}",
                self.alsa.error(port)
            ));
        }
        Ok(port)
    }
}

impl Drop for Seq {
    fn drop(&mut self) {
        // SAFETY: closed once, by its owner; closing removes its ports and
        // connections.
        unsafe { (self.alsa.seq_close)(self.handle) };
    }
}

/// Frees an ALSA object when dropped.
struct Free(P, unsafe extern "C" fn(P));

impl Drop for Free {
    fn drop(&mut self) {
        // SAFETY: the object was allocated by the matching malloc call.
        unsafe { (self.1)(self.0) };
    }
}

/// A port as listed.
struct Found {
    name: String,
    client: c_int,
    port: c_int,
    input: bool,
    output: bool,
}

fn merge(found: Vec<Found>) -> Vec<MidiPortInfo> {
    let mut ports: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for port in found {
        if port.input || port.output {
            let entry = ports.entry(port.name).or_default();
            entry.0 |= port.input;
            entry.1 |= port.output;
        }
    }
    ports
        .into_iter()
        .map(|(name, (input, output))| MidiPortInfo {
            name,
            input,
            output,
        })
        .collect()
}

/// The host's ports; none when the library or the sequencer is missing
/// (a server without sound hardware has no sequencer).
pub(super) fn list() -> Vec<MidiPortInfo> {
    list_from(library())
}

fn list_from(loaded: Result<&'static Alsa, String>) -> Vec<MidiPortInfo> {
    loaded.and_then(list_with).unwrap_or_default()
}

pub(super) fn list_with(alsa: &'static Alsa) -> Result<Vec<MidiPortInfo>, String> {
    let seq = Seq::open(alsa, OPEN_DUPLEX, 0)?;
    Ok(merge(seq.ports()?))
}

pub(super) fn open(
    port: &MidiPort,
    received: mpsc::UnboundedSender<Vec<u8>>,
) -> Result<Box<dyn MidiSink>, String> {
    open_from(library(), port, received)
}

fn open_from(
    loaded: Result<&'static Alsa, String>,
    port: &MidiPort,
    received: mpsc::UnboundedSender<Vec<u8>>,
) -> Result<Box<dyn MidiSink>, String> {
    Ok(Box::new(open_with(loaded?, port, received)?))
}

/// An open port: the output's client, the input's reading thread, and a
/// client watching both are still listed.
pub(super) struct Sink {
    output: Option<Output>,
    input: Option<Input>,
    watch: Seq,
    watched: Vec<String>,
    parser: Parser,
}

struct Output {
    seq: Seq,
    port: c_int,
    encoder: P,
    size: usize,
}

// SAFETY: the encoder belongs to this output alone.
unsafe impl Send for Output {}

impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: allocated by snd_midi_event_new; the client closes after.
        unsafe { (self.seq.alsa.midi_event_free)(self.encoder) };
    }
}

struct Input {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Input {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// How long the input thread waits for events before checking it should
/// stop.
const INPUT_WAIT_MS: c_int = 100;

pub(super) fn open_with(
    alsa: &'static Alsa,
    port: &MidiPort,
    received: mpsc::UnboundedSender<Vec<u8>>,
) -> Result<Sink, String> {
    let watch = Seq::open(alsa, OPEN_DUPLEX, 0)?;
    let listed = watch.ports()?;
    let find = |name: &str, input: bool| {
        listed
            .iter()
            .find(|p| p.name == name && if input { p.input } else { p.output })
            .map(|p| (p.client, p.port))
    };
    let mut watched = Vec::new();
    let mut sink_output = None;
    let mut sink_input = None;
    if let Some(name) = &port.input {
        let (client, number) =
            find(name, true).ok_or_else(|| format!("no MIDI input named '{name}'"))?;
        let seq = Seq::open(alsa, OPEN_INPUT, NONBLOCK)?;
        let own = seq.own_port(c"meros-in", CAP_WRITE | CAP_SUBS_WRITE)?;
        // SAFETY: an open handle and the core's own port.
        let code = unsafe { (alsa.seq_connect_from)(seq.handle, own, client, number) };
        if code < 0 {
            return Err(format!("MIDI input '{name}': {}", alsa.error(code)));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("meros-midi-in".into())
                .spawn(move || read(seq, stop, received))
                .map_err(|e| format!("MIDI input '{name}': {e}"))?
        };
        sink_input = Some(Input {
            stop,
            thread: Some(thread),
        });
        watched.push(name.clone());
    }
    if let Some(name) = &port.output {
        let (client, number) =
            find(name, false).ok_or_else(|| format!("no MIDI output named '{name}'"))?;
        let seq = Seq::open(alsa, OPEN_OUTPUT, 0)?;
        let own = seq.own_port(c"meros-out", CAP_READ | CAP_SUBS_READ)?;
        // SAFETY: an open handle and the core's own port.
        let code = unsafe { (alsa.seq_connect_to)(seq.handle, own, client, number) };
        if code < 0 {
            return Err(format!("MIDI output '{name}': {}", alsa.error(code)));
        }
        let size = 256;
        let mut encoder: P = std::ptr::null_mut();
        // SAFETY: snd_midi_event_new writes the encoder on success.
        let code = unsafe { (alsa.midi_event_new)(size, &mut encoder) };
        if code < 0 || encoder.is_null() {
            return Err(format!("MIDI output '{name}': {}", alsa.error(code)));
        }
        sink_output = Some(Output {
            seq,
            port: own,
            encoder,
            size,
        });
        if !watched.contains(name) {
            watched.push(name.clone());
        }
    }
    Ok(Sink {
        output: sink_output,
        input: sink_input,
        watch,
        watched,
        parser: Parser::default(),
    })
}

/// The input thread: every event the device's port sends, as MIDI bytes,
/// until told to stop.
fn read(seq: Seq, stop: Arc<AtomicBool>, received: mpsc::UnboundedSender<Vec<u8>>) {
    let alsa = seq.alsa;
    let mut decoder: P = std::ptr::null_mut();
    // SAFETY: snd_midi_event_new writes the decoder on success.
    if unsafe { (alsa.midi_event_new)(0, &mut decoder) } < 0 || decoder.is_null() {
        return;
    }
    let _decoder = Free(decoder, alsa.midi_event_free);
    // SAFETY: the decoder allocated above. Each status byte written out,
    // never running status.
    unsafe { (alsa.midi_event_no_status)(decoder, 1) };
    // SAFETY: an open handle; the descriptors are written into `fds`.
    let mut fds = unsafe {
        let count = (alsa.seq_poll_descriptors_count)(seq.handle, POLLIN).max(0) as usize;
        let mut fds = vec![
            PollFd {
                fd: -1,
                events: 0,
                revents: 0,
            };
            count
        ];
        let filled =
            (alsa.seq_poll_descriptors)(seq.handle, fds.as_mut_ptr(), count as c_uint, POLLIN);
        fds.truncate(filled.max(0) as usize);
        fds
    };
    let mut buffer = [0u8; 16];
    while !stop.load(Ordering::SeqCst) {
        loop {
            let mut event: *mut Event = std::ptr::null_mut();
            // SAFETY: the client reads non-blocking; the event stays valid
            // until the next read.
            let code = unsafe { (alsa.seq_event_input)(seq.handle, &mut event) };
            if code == -ENOSPC {
                // The input buffer overran; what was lost is gone.
                continue;
            }
            if code < 0 || event.is_null() {
                break;
            }
            // SAFETY: as above.
            let event = unsafe { &*event };
            let bytes = if event.kind == EVENT_SYSEX {
                // SAFETY: a System Exclusive event from the sequencer.
                unsafe { event.external() }.map(<[u8]>::to_vec)
            } else {
                // SAFETY: the decoder and a buffer of the length given.
                let n = unsafe {
                    (alsa.midi_event_decode)(
                        decoder,
                        buffer.as_mut_ptr(),
                        buffer.len() as c_long,
                        event,
                    )
                };
                // Not MIDI (a port subscribed or gone) decodes to an error.
                (n > 0).then(|| buffer[..n as usize].to_vec())
            };
            if let Some(bytes) = bytes.filter(|b| !b.is_empty()) {
                if received.send(bytes).is_err() {
                    return;
                }
            }
        }
        wait(&mut fds, INPUT_WAIT_MS);
    }
}

impl MidiSink for Sink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(output) = self.output.as_mut() else {
            return Ok(());
        };
        let alsa = output.seq.alsa;
        // One whole message per event.
        for message in crate::midi::split(&mut self.parser, bytes) {
            if message.len() > output.size {
                // SAFETY: the output's encoder.
                let code =
                    unsafe { (alsa.midi_event_resize_buffer)(output.encoder, message.len()) };
                if code < 0 {
                    return Err(alsa.error(code));
                }
                output.size = message.len();
            }
            let mut event = Event::default();
            // SAFETY: the output's encoder, the message and its length, and
            // an event to fill.
            let used = unsafe {
                (alsa.midi_event_reset_encode)(output.encoder);
                (alsa.midi_event_encode)(
                    output.encoder,
                    message.as_ptr(),
                    message.len() as c_long,
                    &mut event,
                )
            };
            if used < 0 || event.kind == EVENT_NONE {
                return Err("the ALSA encoder refused a MIDI message".into());
            }
            event.source.port = output.port as u8;
            event.dest = Addr {
                client: ADDRESS_SUBSCRIBERS,
                port: ADDRESS_UNKNOWN,
            };
            event.queue = QUEUE_DIRECT;
            // SAFETY: an open handle and a filled event.
            let code = unsafe { (alsa.seq_event_output_direct)(output.seq.handle, &mut event) };
            if code < 0 {
                return Err(alsa.error(code));
            }
        }
        Ok(())
    }

    fn present(&mut self) -> bool {
        match self.watch.ports() {
            Ok(ports) => self
                .watched
                .iter()
                .all(|name| ports.iter().any(|p| &p.name == name)),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
#[path = "alsa_tests.rs"]
mod tests;
