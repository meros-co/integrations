//! Running an AppleScript on the host with `/usr/bin/osascript`, for native
//! modules that drive an application on the same Mac (`Action::OsaScript`).
//!
//! The script is one argument of `osascript -e`, never passed through a
//! shell. Its result is what `osascript` prints on standard output, the
//! trailing line break removed; a script that fails gives `osascript`'s error
//! text from standard error (such as `execution error: Keynote got an error:
//! ... (-1728)`). On any other system than macOS there is no `osascript`, and
//! every script fails saying so.

use std::time::Duration;

/// Why a script cannot run on this host.
#[cfg(not(target_os = "macos"))]
pub(crate) const UNAVAILABLE: &str =
    "AppleScript needs the core to run on macOS: this host has no osascript";

#[cfg(not(target_os = "macos"))]
pub(crate) fn run(script: &str, timeout: Option<Duration>) -> Result<String, String> {
    let _ = (script, timeout);
    Err(UNAVAILABLE.into())
}

#[cfg(target_os = "macos")]
pub(crate) fn run(script: &str, timeout: Option<Duration>) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("osascript could not start: {e}"))?;
    // Both pipes are read while the script runs, so a long result never
    // fills a pipe and stalls it.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out = std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut text);
        }
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut text);
        }
        text
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(format!("osascript: {e}")),
        }
        if timeout.is_some_and(|t| started.elapsed() >= t) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "the script did not finish within {} ms",
                timeout.map_or(0, |t| t.as_millis())
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    let trim = |b: Vec<u8>| {
        let text = String::from_utf8_lossy(&b).into_owned();
        text.strip_suffix('\n').map(str::to_string).unwrap_or(text)
    };
    if status.success() {
        Ok(trim(out))
    } else {
        let message = trim(err);
        Err(if message.is_empty() {
            format!("osascript failed ({status})")
        } else {
            message
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn without_macos_every_script_fails_saying_so() {
        assert_eq!(
            super::run("return 1", None),
            Err(super::UNAVAILABLE.to_string())
        );
    }
}
