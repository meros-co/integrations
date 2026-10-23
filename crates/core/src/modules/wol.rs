//! Wake-on-LAN: AMD's Magic Packet (white paper 20213, "Magic Packet
//! Technology", November 1995). Six bytes of FF, then the target's MAC
//! address sixteen times, anywhere in a frame; sent in a UDP datagram to a
//! broadcast address, because a host that is off answers no ARP request
//! and so cannot be reached by unicast once the network forgets it.
//!
//! A SecureOn password, which some network adapters require, is 4 or 6
//! bytes appended after the sixteen repetitions; that layout is
//! established by practice rather than by AMD's paper.

use std::net::{IpAddr, SocketAddr};

use crate::module::{Bind, Cx, Key};

/// A MAC address written as six hex pairs separated by `:` or `-`
/// (`00:11:22:33:44:55`), or as twelve hex digits, optionally grouped in
/// threes of four by dots (`0011.2233.4455`).
pub(crate) fn parse_mac(text: &str) -> Option<[u8; 6]> {
    parse_bytes(text)?.try_into().ok()
}

/// A SecureOn password: 4 or 6 bytes written as a MAC address is.
pub(crate) fn parse_password(text: &str) -> Option<Vec<u8>> {
    parse_bytes(text).filter(|b| b.len() == 4 || b.len() == 6)
}

fn parse_bytes(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.contains([':', '-']) {
        return text
            .split([':', '-'])
            .map(|p| (p.len() == 2).then(|| u8::from_str_radix(p, 16).ok())?)
            .collect();
    }
    let digits: String = text.chars().filter(|c| *c != '.').collect();
    if digits.is_empty()
        || !digits.len().is_multiple_of(2)
        || !digits.chars().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok())
        .collect()
}

/// The magic packet for `mac`, with a SecureOn password when given.
pub(crate) fn magic_packet(mac: [u8; 6], password: Option<&[u8]>) -> Vec<u8> {
    let mut packet = vec![0xFF; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&mac);
    }
    if let Some(password) = password {
        packet.extend_from_slice(password);
    }
    packet
}

/// Send the magic packet from an ephemeral socket of the session's (which
/// may broadcast) to `broadcast`, and to the host itself as well when one is
/// known and differs, for a network that still has its address.
pub(crate) fn send(
    cx: &mut Cx,
    socket: Key,
    packet: Vec<u8>,
    broadcast: SocketAddr,
    host: Option<IpAddr>,
) {
    cx.udp_open(socket, Bind::Ephemeral);
    cx.udp_send(socket, broadcast, packet.clone());
    if let Some(host) = host.filter(|h| !h.is_unspecified() && *h != broadcast.ip()) {
        cx.udp_send(socket, SocketAddr::new(host, broadcast.port()), packet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_magic_packet_is_ff_then_sixteen_macs_then_the_password() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let packet = magic_packet(mac, None);
        assert_eq!(packet.len(), 102);
        assert_eq!(&packet[..6], &[0xFF; 6]);
        for i in 0..16 {
            assert_eq!(&packet[6 + i * 6..12 + i * 6], &mac);
        }
        let with = magic_packet(mac, Some(&[1, 2, 3, 4, 5, 6]));
        assert_eq!(with.len(), 108);
        assert_eq!(&with[102..], &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn macs_and_passwords_are_read_in_the_usual_spellings() {
        let mac = Some([0x00, 0x11, 0x22, 0xaa, 0xbb, 0xcc]);
        assert_eq!(parse_mac("00:11:22:aa:bb:cc"), mac);
        assert_eq!(parse_mac("00-11-22-AA-BB-CC"), mac);
        assert_eq!(parse_mac("001122aabbcc"), mac);
        assert_eq!(parse_mac("0011.22aa.bbcc"), mac);
        assert_eq!(parse_mac("00:11:22:aa:bb"), None);
        assert_eq!(parse_mac("0:11:22:aa:bb:cc"), None);
        assert_eq!(parse_mac("zz:11:22:aa:bb:cc"), None);
        assert_eq!(parse_password("01:02:03:04"), Some(vec![1, 2, 3, 4]));
        assert_eq!(parse_password("010203040506"), Some(vec![1, 2, 3, 4, 5, 6]));
        assert_eq!(parse_password("01:02:03"), None);
    }
}
