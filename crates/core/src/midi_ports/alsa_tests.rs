//! The ALSA backend against a fake sequencer: the library missing, a call
//! missing, and listing, opening, sending, receiving and a port going away.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use super::*;

#[test]
fn event_has_the_c_layout() {
    assert_eq!(std::mem::size_of::<Event>(), 28);
    assert_eq!(std::mem::align_of::<Event>(), 4);
    assert_eq!(std::mem::offset_of!(Event, time), 4);
    assert_eq!(std::mem::offset_of!(Event, source), 12);
    assert_eq!(std::mem::offset_of!(Event, dest), 14);
    assert_eq!(std::mem::offset_of!(Event, data), 16);
    assert_eq!(std::mem::size_of::<PollFd>(), 8);
}

#[test]
fn a_missing_library_lists_nothing_and_opens_nothing() {
    let missing = load("libasound-missing.so.0");
    let reason = missing.as_ref().err().cloned().expect("no such library");
    assert!(reason.contains("libasound-missing.so.0"), "{reason}");
    assert!(reason.contains("could not be loaded"), "{reason}");

    let failed = || -> Result<&'static Alsa, String> { Err(reason.clone()) };
    assert!(list_from(failed()).is_empty());
    let (tx, _rx) = mpsc::unbounded_channel();
    let port = MidiPort {
        input: Some("Keys:Keys MIDI 1 24:0".into()),
        output: None,
    };
    let error = open_from(failed(), &port, tx).err().expect("not opened");
    assert_eq!(error, reason);
}

unsafe extern "C" fn noop() {}

#[test]
fn a_library_without_a_call_is_refused() {
    let error = Alsa::resolve(&mut |name| {
        if name == c"snd_seq_open" {
            std::ptr::null_mut()
        } else {
            noop as unsafe extern "C" fn() as *mut c_void
        }
    })
    .err()
    .expect("refused");
    assert_eq!(error, "libasound.so.2 has no snd_seq_open");
}

// --- A fake sequencer --------------------------------------------------------

struct FakePort {
    client: c_int,
    client_name: &'static str,
    port: c_int,
    name: &'static str,
    caps: c_uint,
    kind: c_uint,
}

#[derive(Default)]
struct Fake {
    ports: Vec<FakePort>,
    refuse_open: bool,
    open_clients: usize,
    own_ports: Vec<String>,
    connected_from: Vec<(c_int, c_int)>,
    connected_to: Vec<(c_int, c_int)>,
    no_status: bool,
    /// What reached the device: the bytes and the event's addressing.
    sent: Vec<(Vec<u8>, Event)>,
    /// What the device sends: bytes, or None for a non-MIDI event.
    incoming: VecDeque<Option<Vec<u8>>>,
}

static FAKE: Mutex<Option<Fake>> = Mutex::new(None);

fn fake<T>(f: impl FnOnce(&mut Fake) -> T) -> T {
    f(FAKE.lock().unwrap().get_or_insert_with(Fake::default))
}

#[derive(Default)]
struct Info {
    client: c_int,
    port: c_int,
    client_name: CString,
    name: CString,
    caps: c_uint,
    kind: c_uint,
}

fn info<'a>(p: P) -> &'a mut Info {
    // SAFETY: every info pointer the fake hands out is a boxed Info.
    unsafe { &mut *p.cast::<Info>() }
}

unsafe extern "C" fn strerror(_: c_int) -> *const c_char {
    c"fake failure".as_ptr()
}
unsafe extern "C" fn seq_open(h: *mut P, _: *const c_char, _: c_int, _: c_int) -> c_int {
    if fake(|f| f.refuse_open) {
        return -2;
    }
    fake(|f| f.open_clients += 1);
    *h = Box::into_raw(Box::new(0u8)).cast();
    0
}
unsafe extern "C" fn seq_close(h: P) -> c_int {
    drop(Box::from_raw(h.cast::<u8>()));
    fake(|f| f.open_clients -= 1);
    0
}
unsafe extern "C" fn set_client_name(_: P, _: *const c_char) -> c_int {
    0
}
unsafe extern "C" fn info_malloc(p: *mut P) -> c_int {
    *p = Box::into_raw(Box::<Info>::default()).cast();
    0
}
unsafe extern "C" fn info_free(p: P) {
    drop(Box::from_raw(p.cast::<Info>()));
}
unsafe extern "C" fn set_client(p: P, client: c_int) {
    info(p).client = client;
}
unsafe extern "C" fn set_port(p: P, port: c_int) {
    info(p).port = port;
}
unsafe extern "C" fn get_client(p: P) -> c_int {
    info(p).client
}
unsafe extern "C" fn get_port(p: P) -> c_int {
    info(p).port
}
unsafe extern "C" fn get_client_name(p: P) -> *const c_char {
    info(p).client_name.as_ptr()
}
unsafe extern "C" fn get_name(p: P) -> *const c_char {
    info(p).name.as_ptr()
}
unsafe extern "C" fn get_caps(p: P) -> c_uint {
    info(p).caps
}
unsafe extern "C" fn get_kind(p: P) -> c_uint {
    info(p).kind
}
unsafe extern "C" fn next_client(_: P, p: P) -> c_int {
    let i = info(p);
    let next = fake(|f| {
        f.ports
            .iter()
            .filter(|x| x.client > i.client)
            .min_by_key(|x| x.client)
            .map(|x| (x.client, x.client_name))
    });
    match next {
        Some((client, name)) => {
            i.client = client;
            i.client_name = CString::new(name).unwrap();
            0
        }
        None => -2,
    }
}
unsafe extern "C" fn next_port(_: P, p: P) -> c_int {
    let i = info(p);
    let next = fake(|f| {
        f.ports
            .iter()
            .filter(|x| x.client == i.client && x.port > i.port)
            .min_by_key(|x| x.port)
            .map(|x| (x.port, x.name, x.caps, x.kind))
    });
    match next {
        Some((port, name, caps, kind)) => {
            i.port = port;
            i.name = CString::new(name).unwrap();
            i.caps = caps;
            i.kind = kind;
            0
        }
        None => -2,
    }
}
unsafe extern "C" fn create_port(_: P, name: *const c_char, _: c_uint, _: c_uint) -> c_int {
    let name = CStr::from_ptr(name).to_string_lossy().into_owned();
    fake(|f| {
        f.own_ports.push(name);
        f.own_ports.len() as c_int - 1
    })
}
fn exists(client: c_int, port: c_int) -> bool {
    fake(|f| f.ports.iter().any(|p| p.client == client && p.port == port))
}
unsafe extern "C" fn connect_from(_: P, _: c_int, client: c_int, port: c_int) -> c_int {
    if !exists(client, port) {
        return -2;
    }
    fake(|f| f.connected_from.push((client, port)));
    0
}
unsafe extern "C" fn connect_to(_: P, _: c_int, client: c_int, port: c_int) -> c_int {
    if !exists(client, port) {
        return -2;
    }
    fake(|f| f.connected_to.push((client, port)));
    0
}

/// The fake encoder: a short message in the data bytes with its length in
/// `tag`, System Exclusive pointed at like ALSA does.
unsafe extern "C" fn encode(_: P, buf: *const u8, count: c_long, ev: *mut Event) -> c_long {
    let bytes = std::slice::from_raw_parts(buf, count as usize).to_vec();
    let ev = &mut *ev;
    if bytes[0] == 0xF0 {
        ev.kind = EVENT_SYSEX;
        ev.flags = LENGTH_VARIABLE;
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        ev.data[0] = leaked.len() as u32;
        ev.data
            .as_mut_ptr()
            .cast::<u8>()
            .add(4)
            .cast::<*const u8>()
            .write_unaligned(leaked.as_ptr());
    } else {
        ev.kind = 6;
        ev.tag = bytes.len() as u8;
        ev.data
            .as_mut_ptr()
            .cast::<u8>()
            .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
    }
    count
}
unsafe extern "C" fn decode(_: P, buf: *mut u8, count: c_long, ev: *const Event) -> c_long {
    let ev = &*ev;
    if ev.kind != 6 {
        return -2;
    }
    let n = ev.tag as usize;
    assert!(n as c_long <= count);
    buf.copy_from_nonoverlapping(ev.data.as_ptr().cast::<u8>(), n);
    n as c_long
}
unsafe extern "C" fn output_direct(_: P, ev: *mut Event) -> c_int {
    let ev = *ev;
    let bytes = match ev.external() {
        Some(bytes) => bytes.to_vec(),
        None => std::slice::from_raw_parts(ev.data.as_ptr().cast::<u8>(), ev.tag as usize).to_vec(),
    };
    fake(|f| f.sent.push((bytes, ev)));
    0
}

thread_local! {
    /// The event last read, kept like ALSA keeps it in its input buffer.
    static LAST: RefCell<(Event, Vec<u8>)> = RefCell::default();
}

unsafe extern "C" fn event_input(_: P, out: *mut *mut Event) -> c_int {
    let Some(next) = fake(|f| f.incoming.pop_front()) else {
        return -11;
    };
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        let (event, keep) = &mut *last;
        *event = Event::default();
        match next {
            None => event.kind = 66,
            Some(bytes) if bytes[0] == 0xF0 => {
                *keep = bytes;
                event.kind = EVENT_SYSEX;
                event.flags = LENGTH_VARIABLE;
                event.data[0] = keep.len() as u32;
                event
                    .data
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(4)
                    .cast::<*const u8>()
                    .write_unaligned(keep.as_ptr());
            }
            Some(bytes) => {
                event.kind = 6;
                event.tag = bytes.len() as u8;
                event
                    .data
                    .as_mut_ptr()
                    .cast::<u8>()
                    .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
            }
        }
        *out = event as *mut Event;
    });
    0
}
unsafe extern "C" fn poll_count(_: P, _: c_short) -> c_int {
    0
}
unsafe extern "C" fn poll_fill(_: P, _: *mut PollFd, _: c_uint, _: c_short) -> c_int {
    0
}
unsafe extern "C" fn coder_new(_: usize, p: *mut P) -> c_int {
    *p = Box::into_raw(Box::new(0u8)).cast();
    0
}
unsafe extern "C" fn coder_free(p: P) {
    drop(Box::from_raw(p.cast::<u8>()));
}
unsafe extern "C" fn coder_resize(_: P, _: usize) -> c_int {
    0
}
unsafe extern "C" fn coder_reset(_: P) {}
unsafe extern "C" fn coder_no_status(_: P, on: c_int) {
    fake(|f| f.no_status = on != 0);
}

fn fake_alsa() -> &'static Alsa {
    Box::leak(Box::new(Alsa {
        strerror,
        seq_open,
        seq_close,
        seq_set_client_name: set_client_name,
        seq_client_info_malloc: info_malloc,
        seq_client_info_free: info_free,
        seq_client_info_set_client: set_client,
        seq_client_info_get_client: get_client,
        seq_client_info_get_name: get_client_name,
        seq_query_next_client: next_client,
        seq_port_info_malloc: info_malloc,
        seq_port_info_free: info_free,
        seq_port_info_set_client: set_client,
        seq_port_info_set_port: set_port,
        seq_port_info_get_client: get_client,
        seq_port_info_get_port: get_port,
        seq_port_info_get_name: get_name,
        seq_port_info_get_capability: get_caps,
        seq_port_info_get_type: get_kind,
        seq_query_next_port: next_port,
        seq_create_simple_port: create_port,
        seq_connect_from: connect_from,
        seq_connect_to: connect_to,
        seq_event_output_direct: output_direct,
        seq_event_input: event_input,
        seq_poll_descriptors_count: poll_count,
        seq_poll_descriptors: poll_fill,
        midi_event_new: coder_new,
        midi_event_free: coder_free,
        midi_event_resize_buffer: coder_resize,
        midi_event_reset_encode: coder_reset,
        midi_event_no_status: coder_no_status,
        midi_event_encode: encode,
        midi_event_decode: decode,
    }))
}

const BOTH: c_uint = CAP_READ | CAP_SUBS_READ | CAP_WRITE | CAP_SUBS_WRITE;

#[tokio::test]
async fn ports_are_listed_opened_and_watched() {
    fake(|f| {
        f.ports = vec![
            FakePort {
                client: 0,
                client_name: "System",
                port: 0,
                name: "Timer",
                caps: CAP_READ | CAP_SUBS_READ,
                kind: 0,
            },
            FakePort {
                client: 24,
                client_name: "Keys",
                port: 0,
                name: "Keys MIDI 1",
                caps: BOTH,
                kind: TYPE_MIDI_GENERIC,
            },
            FakePort {
                client: 128,
                client_name: "Synth",
                port: 0,
                name: "In",
                caps: CAP_WRITE | CAP_SUBS_WRITE,
                kind: TYPE_SYNTH,
            },
            FakePort {
                client: 129,
                client_name: "Monitor",
                port: 0,
                name: "Out",
                // Readable but not subscribable: not listed as an input.
                caps: CAP_READ,
                kind: TYPE_APPLICATION,
            },
        ];
    });
    let alsa = fake_alsa();

    let listed = list_with(alsa).unwrap();
    assert_eq!(
        listed,
        vec![
            MidiPortInfo {
                name: "Keys:Keys MIDI 1 24:0".into(),
                input: true,
                output: true,
            },
            MidiPortInfo {
                name: "Synth:In 128:0".into(),
                input: false,
                output: true,
            },
        ]
    );
    assert_eq!(fake(|f| f.open_clients), 0);

    // No such port.
    let (tx, _rx) = mpsc::unbounded_channel();
    let missing = MidiPort {
        input: Some("Gone:Gone 30:0".into()),
        output: None,
    };
    let error = open_with(alsa, &missing, tx).err().expect("not opened");
    assert_eq!(error, "no MIDI input named 'Gone:Gone 30:0'");
    // An output is not opened as an input.
    let (tx, _rx) = mpsc::unbounded_channel();
    let wrong = MidiPort {
        input: Some("Synth:In 128:0".into()),
        output: None,
    };
    assert!(open_with(alsa, &wrong, tx).is_err());
    assert_eq!(fake(|f| f.open_clients), 0);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let port = MidiPort {
        input: Some("Keys:Keys MIDI 1 24:0".into()),
        output: Some("Keys:Keys MIDI 1 24:0".into()),
    };
    let mut sink = open_with(alsa, &port, tx).unwrap();
    assert_eq!(fake(|f| f.connected_from.clone()), vec![(24, 0)]);
    assert_eq!(fake(|f| f.connected_to.clone()), vec![(24, 0)]);
    assert_eq!(
        fake(|f| f.own_ports.clone()),
        vec!["meros-in".to_string(), "meros-out".to_string()]
    );
    assert!(sink.present());

    // A note on and a System Exclusive message split across two writes:
    // one event each, to the subscribers, directly.
    sink.send(&[0x90, 0x40, 0x7F, 0xF0, 0x7E]).unwrap();
    sink.send(&[0x01, 0xF7]).unwrap();
    let sent = fake(|f| std::mem::take(&mut f.sent));
    let bytes: Vec<Vec<u8>> = sent.iter().map(|(b, _)| b.clone()).collect();
    assert_eq!(
        bytes,
        vec![vec![0x90, 0x40, 0x7F], vec![0xF0, 0x7E, 0x01, 0xF7]]
    );
    for (_, event) in &sent {
        assert_eq!(event.source.port, 1);
        assert_eq!(
            event.dest,
            Addr {
                client: 254,
                port: 253
            }
        );
        assert_eq!(event.queue, 253);
    }

    // What the device sends: a note, an event that is not MIDI, a System
    // Exclusive message.
    fake(|f| {
        f.incoming.push_back(Some(vec![0x80, 0x40, 0x00]));
        f.incoming.push_back(None);
        f.incoming.push_back(Some(vec![0xF0, 0x7F, 0x01, 0xF7]));
    });
    let first = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap();
    assert_eq!(first, Some(vec![0x80, 0x40, 0x00]));
    let second = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap();
    assert_eq!(second, Some(vec![0xF0, 0x7F, 0x01, 0xF7]));
    assert!(fake(|f| f.no_status));

    // Unplugged.
    fake(|f| f.ports.retain(|p| p.client != 24));
    assert!(!sink.present());
    drop(sink);
    assert_eq!(fake(|f| f.open_clients), 0);

    // A host without a sequencer: nothing listed, and opening says why.
    fake(|f| f.refuse_open = true);
    assert!(list_from(Ok(alsa)).is_empty());
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = open_from(Ok(alsa), &port, tx).err().expect("not opened");
    assert_eq!(
        error,
        "the ALSA sequencer could not be opened: fake failure"
    );
}
