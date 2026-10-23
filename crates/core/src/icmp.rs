//! ICMP echo (ping) without elevated privileges.
//!
//! An echo request is RFC 792's type 8 (IPv4) or RFC 4443's type 128 (IPv6),
//! answered by type 0 or 129 carrying the same identifier, sequence number
//! and data. Sending one normally needs a raw socket, which only a privileged
//! process may open, so each system's unprivileged way is used instead:
//!
//! - Windows: the ICMP helper API in iphlpapi.dll (`IcmpSendEcho`,
//!   `Icmp6SendEcho2`), which any process may call. It blocks, so it runs on
//!   a blocking thread. It measures the round trip in whole milliseconds.
//! - Linux, Android, macOS, iOS and the BSDs: an ICMP datagram socket
//!   (`socket(AF_INET, SOCK_DGRAM, IPPROTO_ICMP)`). Linux allows it to the
//!   groups in `net.ipv4.ping_group_range` (icmp(7)), which many
//!   distributions leave empty: it is then refused, and reported
//!   [`EchoResult::Unavailable`] so a caller can fall back to something else.
//!   Linux sets the identifier itself and hands the socket only its own
//!   replies, without the IP header; macOS keeps the identifier given and
//!   prefixes IPv4 replies with their IP header, so both are allowed for.
//! - Anything else: [`EchoResult::Unavailable`].

#[cfg(any(windows, test))]
use std::net::IpAddr;
#[cfg(unix)]
use std::sync::atomic::{AtomicU16, Ordering};

use crate::module::{EchoRequest, EchoResult};

/// Send one echo request and wait for its reply.
pub(crate) async fn echo(request: EchoRequest) -> EchoResult {
    #[cfg(windows)]
    {
        tokio::task::spawn_blocking(move || windows::echo(&request))
            .await
            .unwrap_or_else(|e| EchoResult::Unavailable {
                message: format!("the ICMP request failed: {e}"),
            })
    }
    #[cfg(unix)]
    {
        unix::echo(&request).await
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = request;
        EchoResult::Unavailable {
            message: "ICMP echo is not available on this platform".into(),
        }
    }
}

/// The data an echo request carries: a recognisable repeating pattern, the
/// same in every request, so a reply can be checked against it.
fn payload(size: u16) -> Vec<u8> {
    (0..size).map(|i| b'a' + (i % 23) as u8).collect()
}

/// A sequence number for each request, so replies to requests that timed
/// out are not taken for the reply to a later one.
#[cfg(unix)]
static SEQUENCE: AtomicU16 = AtomicU16::new(1);

/// The Internet checksum (RFC 1071): the ones' complement of the ones'
/// complement sum of the 16-bit words.
#[cfg(any(unix, test))]
fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for pair in data.chunks(2) {
        let word = u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]);
        sum += u32::from(word);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// An echo request message: type, code 0, checksum, identifier, sequence
/// number, data. The checksum is filled in for IPv4; for IPv6 it covers a
/// pseudo-header only the system knows, and the system fills it in.
#[cfg(any(unix, test))]
fn request_message(v6: bool, identifier: u16, sequence: u16, data: &[u8]) -> Vec<u8> {
    let mut message = vec![if v6 { 128 } else { 8 }, 0, 0, 0];
    message.extend_from_slice(&identifier.to_be_bytes());
    message.extend_from_slice(&sequence.to_be_bytes());
    message.extend_from_slice(data);
    if !v6 {
        let sum = checksum(&message);
        message[2..4].copy_from_slice(&sum.to_be_bytes());
    }
    message
}

/// Whether a received datagram is the reply to the request with this
/// sequence number and data, and its TTL when the datagram carried its IP
/// header. `identifier` is checked only where the system keeps the one
/// given (not Linux, which replaces it with its own and filters for it).
#[cfg(any(unix, test))]
fn match_reply(
    v6: bool,
    packet: &[u8],
    identifier: Option<u16>,
    sequence: u16,
    data: &[u8],
) -> Option<Option<u8>> {
    let mut ttl = None;
    let mut message = packet;
    // An IPv4 header first (macOS): version 4, its length in the low nibble.
    if !v6 && packet.first().is_some_and(|b| b >> 4 == 4) {
        let header = usize::from(packet[0] & 0x0F) * 4;
        if header < 20 || packet.len() < header + 8 {
            return None;
        }
        ttl = Some(packet[8]);
        message = &packet[header..];
    }
    if message.len() < 8 {
        return None;
    }
    let reply_type = if v6 { 129 } else { 0 };
    if message[0] != reply_type || message[1] != 0 {
        return None;
    }
    if identifier.is_some_and(|id| u16::from_be_bytes([message[4], message[5]]) != id) {
        return None;
    }
    if u16::from_be_bytes([message[6], message[7]]) != sequence || &message[8..] != data {
        return None;
    }
    Some(ttl)
}

#[cfg(unix)]
mod unix {
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use socket2::{Domain, Protocol, Socket, Type};

    use super::*;

    pub(super) async fn echo(request: &EchoRequest) -> EchoResult {
        let v6 = request.to.is_ipv6();
        let (domain, protocol) = if v6 {
            (Domain::IPV6, Protocol::ICMPV6)
        } else {
            (Domain::IPV4, Protocol::ICMPV4)
        };
        let socket = match Socket::new(domain, Type::DGRAM, Some(protocol)) {
            Ok(socket) => socket,
            Err(e) => {
                return EchoResult::Unavailable {
                    message: format!("the system refused an ICMP datagram socket: {e}"),
                }
            }
        };
        if let Err(e) = socket.set_nonblocking(true) {
            return EchoResult::Unavailable {
                message: e.to_string(),
            };
        }
        let std: std::net::UdpSocket = socket.into();
        let socket = match tokio::net::UdpSocket::from_std(std) {
            Ok(socket) => socket,
            Err(e) => {
                return EchoResult::Unavailable {
                    message: e.to_string(),
                }
            }
        };
        let identifier = std::process::id() as u16;
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let data = payload(request.size);
        let message = request_message(v6, identifier, sequence, &data);
        let sent = Instant::now();
        if let Err(e) = socket
            .send_to(&message, SocketAddr::new(request.to, 0))
            .await
        {
            return EchoResult::Unreachable {
                message: e.to_string(),
            };
        }
        // Linux puts its own identifier in, and delivers only replies to it.
        let check_identifier = if cfg!(any(target_os = "linux", target_os = "android")) {
            None
        } else {
            Some(identifier)
        };
        let deadline = sent + Duration::from_millis(request.timeout);
        let mut buf = vec![0u8; usize::from(request.size) + 128];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(left, socket.recv_from(&mut buf)).await {
                Err(_) => return EchoResult::Timeout,
                Ok(Err(e)) => {
                    return EchoResult::Unreachable {
                        message: e.to_string(),
                    }
                }
                Ok(Ok((n, from))) => {
                    if from.ip() != request.to {
                        continue;
                    }
                    if let Some(ttl) = match_reply(v6, &buf[..n], check_identifier, sequence, &data)
                    {
                        return EchoResult::Reply {
                            rtt_micros: sent.elapsed().as_micros() as u64,
                            ttl,
                        };
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::Foundation::{GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        Icmp6CreateFile, Icmp6ParseReplies, Icmp6SendEcho2, IcmpCloseHandle, IcmpCreateFile,
        IcmpSendEcho, ICMPV6_ECHO_REPLY_LH, ICMP_ECHO_REPLY, IP_REQ_TIMED_OUT, IP_SUCCESS,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET6, SOCKADDR_IN6};

    use super::*;

    /// An ICMP handle, closed when dropped.
    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from IcmpCreateFile or Icmp6CreateFile
            // and is closed once, here.
            unsafe {
                IcmpCloseHandle(self.0);
            }
        }
    }

    /// What an IP_STATUS code means (Microsoft's IP_STATUS list in
    /// ipexport.h).
    fn status_text(status: u32) -> String {
        let text = match status {
            11002 => "destination network unreachable",
            11003 => "destination host unreachable",
            11004 => "destination protocol unreachable",
            11005 => "destination port unreachable",
            11006 => "no resources",
            11007 => "bad option",
            11008 => "hardware error",
            11009 => "packet too big",
            11010 => "request timed out",
            11011 => "bad request",
            11012 => "bad route",
            11013 => "time to live expired in transit",
            11014 => "time to live expired during reassembly",
            11015 => "parameter problem",
            11016 => "source quench",
            11018 => "bad destination",
            11040 => "destination unreachable",
            11041 => "time exceeded",
            11050 => "general failure",
            _ => return format!("ICMP status {status}"),
        };
        text.to_string()
    }

    fn result(status: u32, rtt_millis: u32, ttl: Option<u8>) -> EchoResult {
        match status {
            IP_SUCCESS => EchoResult::Reply {
                rtt_micros: u64::from(rtt_millis) * 1000,
                ttl,
            },
            IP_REQ_TIMED_OUT => EchoResult::Timeout,
            other => EchoResult::Unreachable {
                message: status_text(other),
            },
        }
    }

    pub(super) fn echo(request: &EchoRequest) -> EchoResult {
        let data = payload(request.size);
        let timeout = request.timeout.clamp(1, u64::from(u32::MAX)) as u32;
        // Room for the reply structure, the data echoed, an ICMP error
        // message's 8 bytes and IO_STATUS_BLOCK, as the API asks; u64 for
        // the structure's alignment.
        let bytes = std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8 + 64;
        let mut buf = vec![0u64; bytes.div_ceil(8)];
        let len = (buf.len() * 8) as u32;
        match request.to {
            IpAddr::V4(to) => {
                // SAFETY: no arguments.
                let raw = unsafe { IcmpCreateFile() };
                if raw == INVALID_HANDLE_VALUE {
                    // SAFETY: reads the calling thread's last error.
                    let error = unsafe { GetLastError() };
                    return EchoResult::Unavailable {
                        message: format!("IcmpCreateFile failed (error {error})"),
                    };
                }
                let handle = Handle(raw);
                // SAFETY: `data` and `buf` outlive the call, and `len` is
                // `buf`'s size in bytes. The address is in network byte order.
                let replies = unsafe {
                    IcmpSendEcho(
                        handle.0,
                        u32::from_ne_bytes(to.octets()),
                        data.as_ptr().cast(),
                        data.len() as u16,
                        std::ptr::null(),
                        buf.as_mut_ptr().cast(),
                        len,
                        timeout,
                    )
                };
                if replies == 0 {
                    // SAFETY: reads the calling thread's last error.
                    let error = unsafe { GetLastError() };
                    return result(error, 0, None);
                }
                // SAFETY: on success the buffer starts with an
                // ICMP_ECHO_REPLY, aligned by `buf`'s element type.
                let reply = unsafe { &*(buf.as_ptr() as *const ICMP_ECHO_REPLY) };
                result(reply.Status, reply.RoundTripTime, Some(reply.Options.Ttl))
            }
            IpAddr::V6(to) => {
                // SAFETY: no arguments.
                let raw = unsafe { Icmp6CreateFile() };
                if raw == INVALID_HANDLE_VALUE {
                    // SAFETY: reads the calling thread's last error.
                    let error = unsafe { GetLastError() };
                    return EchoResult::Unavailable {
                        message: format!("Icmp6CreateFile failed (error {error})"),
                    };
                }
                let handle = Handle(raw);
                // SAFETY: all-zero is a valid SOCKADDR_IN6 (the unspecified
                // address, which lets the system choose the source).
                let mut source: SOCKADDR_IN6 = unsafe { std::mem::zeroed() };
                source.sin6_family = AF_INET6;
                let mut destination = source;
                destination.sin6_addr.u.Byte = to.octets();
                // SAFETY: the addresses, `data` and `buf` outlive the call,
                // and `len` is `buf`'s size in bytes. No event or APC
                // routine, so the call blocks until it is answered.
                let replies = unsafe {
                    Icmp6SendEcho2(
                        handle.0,
                        std::ptr::null_mut(),
                        None,
                        std::ptr::null(),
                        &source,
                        &destination,
                        data.as_ptr().cast(),
                        data.len() as u16,
                        std::ptr::null(),
                        buf.as_mut_ptr().cast(),
                        len,
                        timeout,
                    )
                };
                if replies == 0 {
                    // SAFETY: reads the calling thread's last error.
                    let error = unsafe { GetLastError() };
                    return result(error, 0, None);
                }
                // SAFETY: the buffer holds the reply Icmp6SendEcho2 wrote.
                if unsafe { Icmp6ParseReplies(buf.as_mut_ptr().cast(), len) } == 0 {
                    // SAFETY: reads the calling thread's last error.
                    let error = unsafe { GetLastError() };
                    return result(error, 0, None);
                }
                // SAFETY: the buffer starts with an ICMPV6_ECHO_REPLY,
                // aligned by `buf`'s element type.
                let reply = unsafe { &*(buf.as_ptr() as *const ICMPV6_ECHO_REPLY_LH) };
                result(reply.Status, reply.RoundTripTime, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 1071 §3's worked example: the words 0001 f203 f4f5 f6f7 sum to
    /// ddf2, whose complement is the checksum.
    #[test]
    fn the_checksum_is_rfc_1071s() {
        let data = [0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
        assert_eq!(checksum(&data), !0xddf2);
        // A message carrying its checksum sums to zero.
        let message = request_message(false, 0x1234, 7, b"abc");
        assert_eq!(checksum(&message), 0);
        assert_eq!(&message[..2], &[8, 0]);
        assert_eq!(&message[4..8], &[0x12, 0x34, 0, 7]);
    }

    #[test]
    fn replies_are_matched_with_or_without_their_ip_header() {
        let data = payload(8);
        let mut reply = request_message(false, 0x1234, 9, &data);
        reply[0] = 0;
        assert_eq!(match_reply(false, &reply, None, 9, &data), Some(None));
        assert_eq!(
            match_reply(false, &reply, Some(0x1234), 9, &data),
            Some(None)
        );
        assert_eq!(match_reply(false, &reply, Some(0x9999), 9, &data), None);
        assert_eq!(match_reply(false, &reply, None, 10, &data), None);
        // With a 20-byte IPv4 header (macOS), TTL 64.
        let mut with_header = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 1];
        with_header.resize(20, 0);
        with_header.extend_from_slice(&reply);
        assert_eq!(
            match_reply(false, &with_header, None, 9, &data),
            Some(Some(64))
        );
        // A request is not a reply; neither is other data.
        let request = request_message(false, 0x1234, 9, &data);
        assert_eq!(match_reply(false, &request, None, 9, &data), None);
        assert_eq!(match_reply(false, &reply, None, 9, b"other..."), None);
        // IPv6: type 129.
        let mut v6 = request_message(true, 1, 3, &data);
        v6[0] = 129;
        assert_eq!(match_reply(true, &v6, None, 3, &data), Some(None));
    }

    /// Loopback answers where the system allows unprivileged ICMP; where it
    /// does not (Linux outside ping_group_range), the test says so and
    /// passes, since that is the refusal callers fall back on.
    #[tokio::test]
    async fn loopback_answers_an_echo_request() {
        for to in [
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        ] {
            let result = echo(EchoRequest {
                to,
                timeout: 2000,
                size: 32,
            })
            .await;
            match result {
                EchoResult::Reply { rtt_micros, .. } => assert!(rtt_micros < 2_000_000),
                EchoResult::Unavailable { message } => {
                    eprintln!("skipped: ICMP echo to {to} is not allowed here: {message}");
                }
                // A host without IPv6 on loopback.
                EchoResult::Unreachable { message } if to.is_ipv6() => {
                    eprintln!("skipped: {to} is unreachable here: {message}");
                }
                other => panic!("{to}: {other:?}"),
            }
        }
    }

    #[test]
    fn the_payload_is_the_size_asked_for() {
        assert_eq!(payload(0).len(), 0);
        assert_eq!(payload(56).len(), 56);
        assert_eq!(payload(3), b"abc");
    }
}
