//! Tests bind loopback only (CONTRIBUTING.md): a test that binds or listens on
//! the wildcard address makes the operating system's firewall ask for network
//! access on every run, since each test build is a new executable. This reads
//! every test source in the repository (session and unit tests, the
//! simulator, the sidecar's and every binding's tests) and fails on a core
//! built without a loopback bind address, a sidecar started without one, or
//! a socket bound to the wildcard address.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "node_modules") {
                files(&path, extension, out);
            }
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

/// The text from `at` to the end of its statement, with each run of
/// whitespace one space, however rustfmt laid it out.
fn statement(text: &str, at: usize) -> String {
    let rest = &text[at..];
    rest[..rest.find(';').unwrap_or(rest.len())]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lines that bind or listen on the wildcard address.
fn wildcard_binds(text: &str, problems: &mut Vec<String>, name: &str) {
    for (n, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let binds = lower.contains("bind") || lower.contains("listen");
        let wildcard = ["0.0.0.0", "UNSPECIFIED", "[::]", "\"::\"", "'::'"]
            .iter()
            .any(|w| line.contains(w));
        if binds && wildcard {
            problems.push(format!("{name}:{}: binds the wildcard address", n + 1));
        }
    }
}

/// Where a Rust source's test code starts: all of a `*_tests.rs` file, or its
/// first inline `#[cfg(test)] mod … {` (unit tests come last in a module).
fn test_part(path: &Path, text: &str) -> Option<usize> {
    if path.to_string_lossy().ends_with("_tests.rs") {
        return Some(0);
    }
    text.match_indices("#[cfg(test)]")
        .map(|(at, _)| at)
        .find(|&at| {
            let next = text[at..].lines().nth(1).unwrap_or("").trim();
            next.starts_with("mod ") && next.ends_with('{')
        })
}

#[test]
fn tests_bind_loopback_only() {
    let root = root();
    let mut problems = Vec::new();
    let name = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };

    // Rust: session tests, unit tests, the simulator, sidecar and C tests.
    let mut rust = Vec::new();
    for dir in [
        "crates/core/tests",
        "crates/core/src",
        "crates/sidecar/tests",
        "crates/sim/src",
        "bindings/c/tests",
    ] {
        files(&root.join(dir), "rs", &mut rust);
    }
    assert!(rust.iter().any(|p| p.ends_with("osc_listener_session.rs")));
    for path in &rust {
        let n = name(path);
        if n.ends_with("tests/common/mod.rs") || n.ends_with("tests/loopback_only.rs") {
            continue;
        }
        let whole = std::fs::read_to_string(path).unwrap();
        let in_src = n.contains("/src/") && !n.starts_with("crates/sim/");
        let text = if in_src {
            match test_part(path, &whole) {
                Some(at) => &whole[at..],
                None => continue,
            }
        } else {
            &whole[..]
        };
        wildcard_binds(text, &mut problems, &n);
        for call in ["Core::new()", "Core::with_options(", "mi_core_new()"] {
            if text.contains(call) {
                problems.push(format!(
                    "{n}: {call} builds a core bound to every interface; use common::core / \
                     common::core_with, or a loopback bind_address"
                ));
            }
        }
        for (at, _) in text.match_indices("Core::with_catalog(") {
            if !statement(text, at).contains("bind_address") {
                problems.push(format!(
                    "{n}: Core::with_catalog without a loopback bind_address"
                ));
            }
        }
        for (at, _) in text.match_indices("let options = cs(") {
            if !statement(text, at).contains("\"bind_address\":\"127.0.0.1\"") {
                problems.push(format!(
                    "{n}: C core options without a loopback bind_address"
                ));
            }
        }
        for (at, _) in text.match_indices("\"serve\"") {
            let s = statement(text, at);
            if !s.contains("\"--bind-address\", \"127.0.0.1\"")
                || !s.contains("\"--listen\", \"127.0.0.1:")
            {
                problems.push(format!(
                    "{n}: a sidecar started without --listen and --bind-address on 127.0.0.1"
                ));
            }
        }
    }

    // Node and Python binding tests.
    let mut scripts = Vec::new();
    files(&root.join("bindings/node/test"), "js", &mut scripts);
    files(&root.join("bindings/python/tests"), "py", &mut scripts);
    assert_eq!(scripts.len(), 3, "{scripts:?}");
    for path in &scripts {
        let n = name(path);
        let text = std::fs::read_to_string(path).unwrap();
        wildcard_binds(&text, &mut problems, &n);
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("from ") || code.starts_with("import ") || code.starts_with('#') {
                continue;
            }
            let builds = ["Core(", "NativeCore("].iter().any(|c| {
                line.match_indices(c).any(|(at, _)| {
                    // `Core(` itself, not `NativeCore(` read as `Core(`.
                    at == 0 || !line.as_bytes()[at - 1].is_ascii_alphanumeric()
                })
            });
            let loopback = [
                "bindAddress: '127.0.0.1'",
                "bind_address: '127.0.0.1'",
                "LOOPBACK",
            ]
            .iter()
            .any(|l| line.contains(l));
            if builds && !loopback {
                problems.push(format!(
                    "{n}:{}: a core without a loopback bind address",
                    i + 1
                ));
            }
            let listens = line.contains(".listen(") || line.contains(".bind(");
            if listens && !(line.contains("127.0.0.1") || line.contains("::1")) {
                problems.push(format!("{n}:{}: a socket not bound to loopback", i + 1));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "tests bind loopback only (CONTRIBUTING.md):\n{}",
        problems.join("\n")
    );
}

#[test]
fn the_scan_finds_a_wildcard_bind() {
    let mut problems = Vec::new();
    wildcard_binds(
        "let s = UdpSocket::bind(\"0.0.0.0:0\");\nlet t = bind_any(Ipv4Addr::UNSPECIFIED);\nhost: UNSPECIFIED,",
        &mut problems,
        "x.rs",
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}
