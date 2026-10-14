//! What every session test shares: a core bound to loopback. A test binds
//! nothing but 127.0.0.1 or ::1, so a run never asks the operating system's
//! firewall for network access (CONTRIBUTING.md, "Tests bind loopback only").
//! `tests/loopback_only.rs` keeps tests building their cores here.

#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr};

use meros_integrations::{Core, CoreOptions};

/// A core with every integration in the build, bound to 127.0.0.1.
pub fn core() -> std::io::Result<Core> {
    core_with(CoreOptions::new())
}

/// A core with `options`, bound to 127.0.0.1 unless they name another
/// loopback address. A non-loopback address fails the test.
pub fn core_with(options: CoreOptions) -> std::io::Result<Core> {
    Core::with_options(loopback(options))
}

/// `options` bound to 127.0.0.1 unless they name another loopback address.
pub fn loopback(options: CoreOptions) -> CoreOptions {
    match options.bind_address {
        None => options.bind_address(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        Some(a) if a.is_loopback() => options,
        Some(a) => panic!("tests bind loopback only, not {a}"),
    }
}
