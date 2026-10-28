//! Apple Keynote on the Mac the core runs on, driven through its AppleScript
//! dictionary with `osascript` (`Cx::osascript`).
//!
//! - Every command is a fixed script from Keynote's dictionary (the Standard,
//!   Keynote and Compatibility suites): opening, making, saving, closing and
//!   exporting documents, playing them (start, stop, show next and previous,
//!   the slide switcher, going to a slide), their settings, and their slides
//!   (making, duplicating, moving, deleting, skipping, layout, title, body
//!   and presenter notes). A value a caller gives is only ever put in a
//!   script as an AppleScript string literal, its backslashes and quotes
//!   escaped and its line breaks and tabs joined in as `return`,
//!   `linefeed` and `tab`; any other control character is refused before
//!   the script is built. Numbers and choices come from the spec's ranges
//!   and enumerations. No script is ever taken from a caller.
//! - Scripts run one at a time. A status script every `poll_ms` reads the
//!   application (running, playing, the slide switcher, version, frontmost)
//!   and every open document's settings and current slide number; a fuller
//!   one every `slides_poll_ms` reads each document's slides, layouts and
//!   current slide's index, and the installed themes. Neither starts Keynote
//!   when it is not running. A command that changes something is followed
//!   at once by the read it affects.
//! - Exports can outlast a command's timeout: each answers at once with its
//!   transfer's number, runs beside the other scripts (one at a time), and
//!   is followed in `transfers.<number>` and by `transfer_started` and
//!   `transfer_finished` message events.
//! - On a host other than macOS there is no AppleScript: the device stays
//!   disconnected with a reason saying so, and every command fails with
//!   `transport`. macOS asks the user once to let the program running the
//!   core control Keynote (Privacy & Security, Automation); until then every
//!   script fails with error -1743 and the device is disconnected with a
//!   reason saying how to allow it.
//!
//! Opened for commands only (`monitor` false), neither read is made: a
//! script asking only whether Keynote is running goes every 10 s as the
//! liveness check (it also keeps `running` current), and each command's
//! own result is still applied.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

const SCRIPT: Key = "script";
const EXPORT: Key = "export";
const POLL: Key = "poll";
const SLIDES: Key = "slides-poll";
const LIVENESS: Key = "liveness";

/// The application the scripts address, by bundle identifier, so a renamed
/// or localized Keynote is still found.
const APP: &str = "application id \"com.apple.iWork.Keynote\"";

const LIVENESS_EVERY: Millis = 10_000;
/// A read is ended after this long, so a stalled Keynote (a dialog open) does
/// not hold the queue.
const READ_TIMEOUT: Millis = 10_000;
/// Within the session's own 30 s safety limit for a command.
const COMMAND_TIMEOUT: Millis = 25_000;
/// Finished transfers kept in state.
const KEEP_FINISHED: usize = 16;

/// The separators the read scripts put between fields and records: ASCII
/// unit and record separators, which no title, note or name holds.
const US: char = '\u{1f}';
const RS: char = '\u{1e}';

/// Why the device cannot be reached on a host other than macOS.
const UNAVAILABLE: &str =
    "Keynote is controlled through AppleScript, which needs the core to run on macOS";

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// A caller's text as an AppleScript string expression. Backslashes and
/// quotes are escaped; CR, LF and tab are joined in as `return`, `linefeed`
/// and `tab`, so the literal itself never holds a line break; any other
/// control character is refused.
pub(crate) fn quote(text: &str) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::from("\"");
    for c in text.chars() {
        let joiner = match c {
            '\r' => Some("return"),
            '\n' => Some("linefeed"),
            '\t' => Some("tab"),
            '\\' => {
                current.push_str("\\\\");
                None
            }
            '"' => {
                current.push_str("\\\"");
                None
            }
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                return Err(format!(
                    "control character U+{:04X} cannot be sent to Keynote",
                    c as u32
                ))
            }
            c => {
                current.push(c);
                None
            }
        };
        if let Some(j) = joiner {
            current.push('"');
            parts.push(std::mem::replace(&mut current, String::from("\"")));
            parts.push(j.to_string());
        }
    }
    current.push('"');
    parts.push(current);
    // `"" & return & ""` is still one string, and parenthesized so it binds
    // as one value wherever it stands.
    Ok(if parts.len() == 1 {
        parts.remove(0)
    } else {
        format!("({})", parts.join(" & "))
    })
}

fn string(p: &Params, name: &str) -> Result<String, CommandError> {
    p.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn quoted(p: &Params, name: &str) -> Result<String, CommandError> {
    quote(&string(p, name)?).map_err(|e| bad(format!("'{name}': {e}")))
}

/// A non-empty host path, as a file reference.
fn path(p: &Params, name: &str) -> Result<String, CommandError> {
    let text = string(p, name)?;
    if !text.starts_with('/') {
        return Err(bad(format!("'{name}' must be an absolute path on the Mac")));
    }
    Ok(format!("(POSIX file {})", quote(&text).map_err(bad)?))
}

fn int(p: &Params, name: &str) -> Result<i64, CommandError> {
    p.get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn float(p: &Params, name: &str) -> Result<f64, CommandError> {
    p.get(name)
        .and_then(Value::as_f64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn boolean(p: &Params, name: &str) -> Result<&'static str, CommandError> {
    p.get(name)
        .and_then(Value::as_bool)
        .map(|b| if b { "true" } else { "false" })
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

/// One of an enumeration's values, mapped to the dictionary's term.
fn choice(
    p: &Params,
    name: &str,
    terms: &[(&str, &'static str)],
) -> Result<&'static str, CommandError> {
    let value = string(p, name)?;
    terms
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, t)| *t)
        .ok_or_else(|| bad(format!("'{name}' cannot be '{value}'")))
}

/// The document a command names: by name, or the front document when the
/// name is empty.
fn document(p: &Params) -> Result<String, CommandError> {
    let name = p.get("document").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        Ok("front document".into())
    } else {
        Ok(format!("document {}", quote(name).map_err(bad)?))
    }
}

fn slide(p: &Params, doc: &str) -> Result<String, CommandError> {
    Ok(format!("slide {} of {doc}", int(p, "slide")?))
}

/// What a command does after it succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// Read the status (application and documents).
    Status,
    /// Read the status and the slides.
    Both,
}

/// A command's script, without the `tell` around it, and the read it
/// affects.
fn command_script(name: &str, p: &Params) -> Result<(String, Then), CommandError> {
    use Then::*;
    let saving = |p: &Params| choice(p, "saving", &[("yes", "yes"), ("no", "no")]);
    Ok(match name {
        "activate" => ("activate".into(), Status),
        "quit" => (format!("quit saving {}", saving(p)?), Both),
        "open_document" => (format!("open {}", path(p, "path")?), Both),
        "new_document" => ("make new document".into(), Both),
        "new_document_with_theme" => (
            format!(
                "make new document with properties {{document theme:theme {}, width:{}, height:{}}}",
                quoted(p, "theme")?,
                int(p, "width")?,
                int(p, "height")?
            ),
            Both,
        ),
        "close_document" => (
            format!("close {} saving {}", document(p)?, saving(p)?),
            Both,
        ),
        "close_all_documents" => (format!("close every document saving {}", saving(p)?), Both),
        "save_document" => (format!("save {}", document(p)?), Status),
        "save_document_as" => (
            format!("save {} in {}", document(p)?, path(p, "path")?),
            Status,
        ),
        "start_slideshow" => {
            let doc = document(p)?;
            match int(p, "slide")? {
                0 => (format!("start {doc}"), Status),
                n => (format!("start {doc} from slide {n} of {doc}"), Status),
            }
        }
        "stop_slideshow" => (format!("stop {}", document(p)?), Status),
        "show_next" => ("show next".into(), Status),
        "show_previous" => ("show previous".into(), Status),
        "go_to_slide" => (
            format!(
                "tell {} to set current slide to (first slide whose slide number is {})",
                document(p)?,
                int(p, "slide_number")?
            ),
            Both,
        ),
        "go_to_slide_index" => (
            format!(
                "tell {} to set current slide to slide {}",
                document(p)?,
                int(p, "slide")?
            ),
            Both,
        ),
        "show_slide_switcher" => ("show slide switcher".into(), Status),
        "cancel_slide_switcher" => ("cancel slide switcher".into(), Status),
        "accept_slide_switcher" => ("accept slide switcher".into(), Both),
        "move_slide_switcher_forward" => ("move slide switcher forward".into(), Status),
        "move_slide_switcher_backward" => ("move slide switcher backward".into(), Status),
        "set_auto_loop" => (
            format!("set auto loop of {} to {}", document(p)?, boolean(p, "enabled")?),
            Status,
        ),
        "set_auto_play" => (
            format!("set auto play of {} to {}", document(p)?, boolean(p, "enabled")?),
            Status,
        ),
        "set_auto_restart" => (
            format!("set auto restart of {} to {}", document(p)?, boolean(p, "enabled")?),
            Status,
        ),
        "set_maximum_idle_duration" => (
            format!(
                "set maximum idle duration of {} to {}",
                document(p)?,
                int(p, "minutes")?
            ),
            Status,
        ),
        "set_slide_numbers_showing" => (
            format!(
                "set slide numbers showing of {} to {}",
                document(p)?,
                boolean(p, "showing")?
            ),
            Status,
        ),
        "set_slide_size" => {
            let doc = document(p)?;
            (
                format!(
                    "set width of {doc} to {}\n\tset height of {doc} to {}",
                    int(p, "width")?,
                    int(p, "height")?
                ),
                Both,
            )
        }
        "set_document_theme" => (
            format!(
                "set document theme of {} to theme {}",
                document(p)?,
                quoted(p, "theme")?
            ),
            Both,
        ),
        "new_slide" => {
            let doc = document(p)?;
            let at = match int(p, "before")? {
                0 => "end of slides".to_string(),
                n => format!("before slide {n}"),
            };
            let layout = string(p, "layout")?;
            let props = if layout.is_empty() {
                String::new()
            } else {
                format!(
                    " with properties {{base slide:master slide {}}}",
                    quote(&layout).map_err(bad)?
                )
            };
            (format!("tell {doc} to make new slide at {at}{props}"), Both)
        }
        "duplicate_slide" => {
            let n = int(p, "slide")?;
            (
                format!(
                    "tell {} to duplicate slide {n} to after slide {n}",
                    document(p)?
                ),
                Both,
            )
        }
        "move_slide" => {
            let n = int(p, "slide")?;
            let to = match int(p, "before")? {
                0 => "end of slides".to_string(),
                m => format!("before slide {m}"),
            };
            (format!("tell {} to move slide {n} to {to}", document(p)?), Both)
        }
        "delete_slide" => (
            format!("tell {} to delete slide {}", document(p)?, int(p, "slide")?),
            Both,
        ),
        "set_slide_skipped" => {
            let doc = document(p)?;
            (
                format!(
                    "set skipped of {} to {}",
                    slide(p, &doc)?,
                    boolean(p, "skipped")?
                ),
                Both,
            )
        }
        "set_presenter_notes" => {
            let doc = document(p)?;
            (
                format!(
                    "set presenter notes of {} to {}",
                    slide(p, &doc)?,
                    quoted(p, "notes")?
                ),
                Both,
            )
        }
        "set_slide_title" => {
            let doc = document(p)?;
            (
                format!(
                    "set object text of default title item of {} to {}",
                    slide(p, &doc)?,
                    quoted(p, "text")?
                ),
                Both,
            )
        }
        "set_slide_body" => {
            let doc = document(p)?;
            (
                format!(
                    "set object text of default body item of {} to {}",
                    slide(p, &doc)?,
                    quoted(p, "text")?
                ),
                Both,
            )
        }
        "set_title_showing" => {
            let doc = document(p)?;
            (
                format!(
                    "set title showing of {} to {}",
                    slide(p, &doc)?,
                    boolean(p, "showing")?
                ),
                Both,
            )
        }
        "set_body_showing" => {
            let doc = document(p)?;
            (
                format!(
                    "set body showing of {} to {}",
                    slide(p, &doc)?,
                    boolean(p, "showing")?
                ),
                Both,
            )
        }
        "set_slide_layout" => {
            let doc = document(p)?;
            (
                format!(
                    "set base slide of {} to master slide {} of {doc}",
                    slide(p, &doc)?,
                    quoted(p, "layout")?
                ),
                Both,
            )
        }
        other => return Err(bad(format!("unknown command '{other}'"))),
    })
}

/// An export's script (without the `tell`), its kind and the host path it
/// writes.
fn export_script(name: &str, p: &Params) -> Result<Option<(String, String)>, CommandError> {
    let doc = document(p)?;
    let (target, props) = match name {
        "export_pdf" => {
            let mut props = format!(
                "skipped slides:{}, all stages:{}, slide numbers:{}, date:{}, borders:{}, PDF image quality:{}",
                boolean(p, "skipped_slides")?,
                boolean(p, "all_stages")?,
                boolean(p, "slide_numbers")?,
                boolean(p, "date")?,
                boolean(p, "borders")?,
                choice(p, "image_quality", &[("good", "Good"), ("better", "Better"), ("best", "Best")])?
            );
            let password = p.get("password").and_then(Value::as_str).unwrap_or("");
            if !password.is_empty() {
                props.push_str(&format!(", password:{}", quote(password).map_err(bad)?));
                let hint = p.get("password_hint").and_then(Value::as_str).unwrap_or("");
                if !hint.is_empty() {
                    props.push_str(&format!(", password hint:{}", quote(hint).map_err(bad)?));
                }
            }
            ("PDF", Some(props))
        }
        "export_powerpoint" => ("Microsoft PowerPoint", None),
        "export_movie" => (
            "QuickTime movie",
            Some(format!(
                "movie format:{}",
                choice(
                    p,
                    "movie_format",
                    &[("small", "small"), ("medium", "medium"), ("large", "large")]
                )?
            )),
        ),
        "export_images" => {
            let format = choice(
                p,
                "image_format",
                &[("jpeg", "JPEG"), ("png", "PNG"), ("tiff", "TIFF")],
            )?;
            let quality = float(p, "compression_factor")?;
            (
                "slide images",
                Some(format!(
                    "image format:{format}, skipped slides:{}, all stages:{}, compression factor:{quality:.2}",
                    boolean(p, "skipped_slides")?,
                    boolean(p, "all_stages")?
                )),
            )
        }
        "export_html" => ("HTML", None),
        "export_keynote09" => ("Keynote 09", None),
        _ => return Ok(None),
    };
    let to = path(p, "path")?;
    let props = props
        .map(|p| format!(" with properties {{{p}}}"))
        .unwrap_or_default();
    Ok(Some((
        // Keynote may take long over an export: the Apple event waits a day.
        format!("with timeout of 86400 seconds\n\t\texport {doc} to {to} as {target}{props}\n\tend timeout"),
        string(p, "path")?,
    )))
}

/// A script wrapped in its `tell`.
fn tell(body: &str) -> String {
    format!("tell {APP}\n\t{body}\nend tell")
}

/// One field read into the output, or an empty field when it fails.
fn field(out: &mut String, expr: &str) {
    out.push_str(&format!(
        "\t\ttry\n\t\t\tset out to out & US & (({expr}) as text)\n\t\ton error\n\t\t\tset out to out & US\n\t\tend try\n"
    ));
}

/// The status script: the application, then a record per open document.
pub(crate) fn status_script() -> String {
    let mut s = String::from(
        "set US to character id 31\nset RS to character id 30\n\
         if not (running of application id \"com.apple.iWork.Keynote\") then return \"0\"\n\
         tell application id \"com.apple.iWork.Keynote\"\n\tset out to \"1\"\n",
    );
    let mut app = String::new();
    for expr in ["playing", "slide switcher visible", "version", "frontmost"] {
        field(&mut app, expr);
    }
    s.push_str(&app.replace("\t\t", "\t"));
    s.push_str("\trepeat with i from 1 to count of documents\n\t\tset d to document i\n\t\tset out to out & RS & \"D\"\n");
    for expr in [
        "name of d",
        "POSIX path of (file of d)",
        "modified of d",
        "count of slides of d",
        "slide number of current slide of d",
        "auto loop of d",
        "auto play of d",
        "auto restart of d",
        "maximum idle duration of d",
        "slide numbers showing of d",
        "width of d",
        "height of d",
        "name of document theme of d",
    ] {
        field(&mut s, expr);
    }
    s.push_str("\tend repeat\nend tell\nreturn out");
    s
}

/// The fuller read: the themes, then per document its layouts, its current
/// slide's index and its slides.
pub(crate) fn slides_script() -> String {
    String::from(
        "set US to character id 31\nset RS to character id 30\n\
         if not (running of application id \"com.apple.iWork.Keynote\") then return \"0\"\n\
         tell application id \"com.apple.iWork.Keynote\"\n\
         \tset out to \"1\"\n\
         \ttry\n\t\tset tn to name of every theme\n\t\tset ti to id of every theme\n\
         \t\trepeat with k from 1 to count of tn\n\
         \t\t\tset out to out & RS & \"T\" & US & k & US & (item k of tn) & US & my at(ti, k)\n\
         \t\tend repeat\n\tend try\n\
         \trepeat with i from 1 to count of documents\n\
         \t\tset d to document i\n\
         \t\tset ci to 0\n\
         \t\ttry\n\t\t\tset cs to current slide of d\n\
         \t\t\trepeat with k from 1 to count of slides of d\n\
         \t\t\t\tif slide k of d is cs then\n\t\t\t\t\tset ci to k\n\t\t\t\t\texit repeat\n\t\t\t\tend if\n\
         \t\t\tend repeat\n\t\tend try\n\
         \t\tset out to out & RS & \"D\" & US & (name of d) & US & ci\n\
         \t\tset ln to {}\n\t\ttry\n\t\t\tset ln to name of every master slide of d\n\t\tend try\n\
         \t\trepeat with k from 1 to count of ln\n\
         \t\t\tset out to out & RS & \"L\" & US & k & US & (item k of ln)\n\
         \t\tend repeat\n\
         \t\tset sn to {}\n\t\tset sk to {}\n\t\tset ts to {}\n\t\tset bs to {}\n\t\tset ly to {}\n\
         \t\tset tt to {}\n\t\tset bt to {}\n\t\tset pn to {}\n\
         \t\ttry\n\t\t\tset sn to slide number of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset sk to skipped of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset ts to title showing of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset bs to body showing of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset ly to name of base slide of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset tt to object text of default title item of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset bt to object text of default body item of every slide of d\n\t\tend try\n\
         \t\ttry\n\t\t\tset pn to presenter notes of every slide of d\n\t\tend try\n\
         \t\trepeat with k from 1 to count of slides of d\n\
         \t\t\tset out to out & RS & \"S\" & US & k & US & my at(sn, k) & US & my at(sk, k) & US & my at(ts, k) & US & my at(bs, k) & US & my at(ly, k) & US & my at(tt, k) & US & my at(bt, k) & US & my at(pn, k)\n\
         \t\tend repeat\n\
         \tend repeat\n\
         end tell\n\
         return out\n\
         on at(L, k)\n\ttry\n\t\treturn (item k of L) as text\n\ton error\n\t\treturn \"\"\n\tend try\nend at",
    )
}

/// The liveness script for commands-only opening.
fn liveness_script() -> String {
    "if running of application id \"com.apple.iWork.Keynote\" then return \"1\"\nreturn \"0\""
        .into()
}

fn tf(text: &str) -> Option<Value> {
    match text {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

fn number(text: &str) -> Option<Value> {
    text.trim()
        .parse::<i64>()
        .ok()
        .map(Value::from)
        .or_else(|| {
            // AppleScript writes a real with a decimal comma in some locales.
            text.trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|f| f.fract() == 0.0)
                .map(|f| Value::from(f as i64))
        })
}

fn text_value(text: &str) -> Value {
    Value::String(text.to_string())
}

/// A field, or no value when it is empty (a property the script could not
/// read).
fn put(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.into(), v);
    }
}

/// The status script's output: the application's fields and each document's,
/// by name.
pub(crate) fn parse_status(
    out: &str,
) -> (Map<String, Value>, BTreeMap<String, Map<String, Value>>) {
    let mut app = Map::new();
    let mut docs = BTreeMap::new();
    let mut records = out.split(RS);
    let head: Vec<&str> = records.next().unwrap_or("").split(US).collect();
    let running = head.first() == Some(&"1");
    app.insert("running".into(), Value::Bool(running));
    if !running {
        app.insert("playing".into(), Value::Bool(false));
        app.insert("slide_switcher_visible".into(), Value::Bool(false));
        app.insert("frontmost".into(), Value::Bool(false));
        app.insert("front_document".into(), text_value(""));
        return (app, docs);
    }
    let at = |i: usize| head.get(i).copied().unwrap_or("");
    put(&mut app, "playing", tf(at(1)));
    put(&mut app, "slide_switcher_visible", tf(at(2)));
    if !at(3).is_empty() {
        app.insert("version".into(), text_value(at(3)));
    }
    put(&mut app, "frontmost", tf(at(4)));
    let mut front = String::new();
    for record in records {
        let f: Vec<&str> = record.split(US).collect();
        if f.first() != Some(&"D") || f.get(1).is_none_or(|n| n.is_empty()) {
            continue;
        }
        let g = |i: usize| f.get(i).copied().unwrap_or("");
        let name = g(1).to_string();
        if front.is_empty() {
            front = name.clone();
        }
        let mut d = Map::new();
        d.insert("file".into(), text_value(g(2)));
        put(&mut d, "modified", tf(g(3)));
        put(&mut d, "slide_count", number(g(4)));
        put(&mut d, "current_slide_number", number(g(5)));
        put(&mut d, "auto_loop", tf(g(6)));
        put(&mut d, "auto_play", tf(g(7)));
        put(&mut d, "auto_restart", tf(g(8)));
        put(&mut d, "maximum_idle_duration", number(g(9)));
        put(&mut d, "slide_numbers_showing", tf(g(10)));
        put(&mut d, "width", number(g(11)));
        put(&mut d, "height", number(g(12)));
        if !g(13).is_empty() {
            d.insert("theme".into(), text_value(g(13)));
        }
        docs.insert(name, d);
    }
    app.insert("front_document".into(), text_value(&front));
    (app, docs)
}

/// One document in the fuller read: its current slide's index, its layouts
/// and its slides, each keyed by its place from 1.
pub(crate) type DocumentSlides = (Value, Map<String, Value>, Map<String, Value>);

/// What the fuller read gave.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Slides {
    pub(crate) running: bool,
    /// Index (from 1) to name and id.
    pub(crate) themes: BTreeMap<String, Value>,
    /// Per document: its current slide's index, its layouts and its slides.
    pub(crate) documents: BTreeMap<String, DocumentSlides>,
}

pub(crate) fn parse_slides(out: &str) -> Slides {
    let mut s = Slides::default();
    let mut records = out.split(RS);
    s.running = records.next() == Some("1");
    let mut doc: Option<String> = None;
    for record in records {
        let f: Vec<&str> = record.split(US).collect();
        let g = |i: usize| f.get(i).copied().unwrap_or("");
        match g(0) {
            "T" if !g(1).is_empty() => {
                s.themes
                    .insert(g(1).into(), json!({"name": g(2), "id": g(3)}));
            }
            "D" if !g(1).is_empty() => {
                let name = g(1).to_string();
                let current = number(g(2)).unwrap_or(json!(0));
                s.documents
                    .insert(name.clone(), (current, Map::new(), Map::new()));
                doc = Some(name);
            }
            "L" => {
                if let Some(entry) = doc.as_ref().and_then(|d| s.documents.get_mut(d)) {
                    entry.1.insert(g(1).into(), text_value(g(2)));
                }
            }
            "S" => {
                if let Some(entry) = doc.as_ref().and_then(|d| s.documents.get_mut(d)) {
                    let mut slide = Map::new();
                    put(&mut slide, "slide_number", number(g(2)));
                    put(&mut slide, "skipped", tf(g(3)));
                    put(&mut slide, "title_showing", tf(g(4)));
                    put(&mut slide, "body_showing", tf(g(5)));
                    slide.insert("layout".into(), text_value(g(6)));
                    slide.insert("title".into(), text_value(g(7)));
                    slide.insert("body".into(), text_value(g(8)));
                    slide.insert("presenter_notes".into(), text_value(g(9)));
                    entry.2.insert(g(1).into(), Value::Object(slide));
                }
            }
            _ => {}
        }
    }
    s
}

/// The number in an AppleScript error, such as `(-1728)` at the end of
/// `execution error: Keynote got an error: Can't get document "x". (-1728)`.
fn error_code(message: &str) -> Option<String> {
    let open = message.rfind('(')?;
    let inner = message[open + 1..].trim_end().strip_suffix(')')?;
    inner.parse::<i64>().ok().map(|n| n.to_string())
}

/// Why scripts cannot reach Keynote, when an error says so.
fn unreachable_reason(message: &str) -> Option<String> {
    if message.contains("-1743") {
        return Some(
            "macOS has not allowed this program to control Keynote: allow it in System Settings, \
             Privacy & Security, Automation"
                .into(),
        );
    }
    if message.starts_with("osascript could not start") || message.starts_with("AppleScript needs")
    {
        return Some(message.to_string());
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    Status,
    Slides,
    Liveness,
}

#[derive(Debug)]
enum Job {
    Command {
        id: CommandId,
        script: String,
        then: Then,
    },
    Read(Read),
}

#[derive(Debug)]
struct Export {
    number: u64,
    kind: String,
    path: String,
    document: String,
    script: String,
}

pub(crate) struct Keynote {
    monitor: bool,
    /// Whether this host can run AppleScript at all (macOS).
    available: bool,
    poll_ms: Millis,
    slides_poll_ms: Millis,
    source: SocketAddr,
    queue: VecDeque<Job>,
    /// The job whose script runs, and when it was sent.
    running: Option<(Job, Millis)>,
    exports: VecDeque<Export>,
    exporting: Option<Export>,
    next_transfer: u64,
    finished: VecDeque<u64>,
    connected: Option<bool>,
    /// Documents, and per document the slides and layouts, last reported,
    /// so what is gone can be removed.
    documents: BTreeSet<String>,
    shown: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>,
    themes: BTreeSet<String>,
}

impl Keynote {
    pub(crate) fn new(context: OpenContext) -> Result<Keynote, String> {
        Self::with_host(context, cfg!(target_os = "macos"))
    }

    fn with_host(context: OpenContext, available: bool) -> Result<Keynote, String> {
        let ms = |name: &str, default: Millis| {
            context
                .settings
                .get(name)
                .and_then(Value::as_u64)
                .unwrap_or(default)
        };
        Ok(Keynote {
            monitor: context.monitor,
            available,
            poll_ms: ms("poll_ms", 1000),
            slides_poll_ms: ms("slides_poll_ms", 5000),
            source: SocketAddr::new(context.host, 0),
            queue: VecDeque::new(),
            running: None,
            exports: VecDeque::new(),
            exporting: None,
            next_transfer: 1,
            finished: VecDeque::new(),
            connected: None,
            documents: BTreeSet::new(),
            shown: BTreeMap::new(),
            themes: BTreeSet::new(),
        })
    }

    fn set_connected(&mut self, cx: &mut Cx, connected: bool, reason: &str) {
        if self.connected == Some(connected) && connected {
            return;
        }
        if connected {
            cx.connection(if self.monitor {
                Connection::Connected
            } else {
                Connection::Unmonitored
            });
        } else {
            cx.connection(Connection::Disconnected {
                reason: reason.to_string(),
            });
        }
        self.connected = Some(connected);
    }

    /// Queue a read unless one is already waiting.
    fn read(&mut self, cx: &mut Cx, read: Read) {
        let waiting = self
            .queue
            .iter()
            .any(|j| matches!(j, Job::Read(r) if *r == read));
        if !waiting {
            self.queue.push_back(Job::Read(read));
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.running.is_some() {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        let (script, timeout) = match &job {
            Job::Command { script, .. } => (tell(script), COMMAND_TIMEOUT),
            Job::Read(Read::Status) => (status_script(), READ_TIMEOUT),
            Job::Read(Read::Slides) => (slides_script(), READ_TIMEOUT),
            Job::Read(Read::Liveness) => (liveness_script(), READ_TIMEOUT),
        };
        cx.osascript(SCRIPT, script, Some(timeout));
        self.running = Some((job, cx.now()));
    }

    fn pump_exports(&mut self, cx: &mut Cx) {
        if self.exporting.is_some() {
            return;
        }
        let Some(export) = self.exports.pop_front() else {
            return;
        };
        cx.state(json!({"transfers": {export.number.to_string(): {"status": "exporting"}}}));
        cx.message("transfer_started", None, Self::target(&export), self.source);
        cx.osascript(EXPORT, tell(&export.script), None);
        self.exporting = Some(export);
    }

    fn target(e: &Export) -> Value {
        json!({"transfer": e.number, "kind": e.kind, "path": e.path, "document": e.document})
    }

    fn apply_status(&mut self, cx: &mut Cx, out: &str) {
        let (app, docs) = parse_status(out);
        let mut patch = app;
        let mut documents = Map::new();
        let names: BTreeSet<String> = docs.keys().cloned().collect();
        for gone in self.documents.difference(&names) {
            documents.insert(gone.clone(), Value::Null);
            self.shown.remove(gone);
        }
        for (name, d) in docs {
            documents.insert(name, Value::Object(d));
        }
        self.documents = names;
        if !documents.is_empty() {
            patch.insert("documents".into(), Value::Object(documents));
        }
        cx.state(Value::Object(patch));
    }

    fn apply_slides(&mut self, cx: &mut Cx, out: &str) {
        let read = parse_slides(out);
        if !read.running {
            return;
        }
        let mut patch = Map::new();
        let names: BTreeSet<String> = read.themes.keys().cloned().collect();
        let mut themes = Map::new();
        for gone in self.themes.difference(&names) {
            themes.insert(gone.clone(), Value::Null);
        }
        for (k, v) in read.themes {
            themes.insert(k, v);
        }
        self.themes = names;
        if !themes.is_empty() {
            patch.insert("themes".into(), Value::Object(themes));
        }
        let mut documents = Map::new();
        for (name, (current, layouts, slides)) in read.documents {
            let (old_slides, old_layouts) = self.shown.remove(&name).unwrap_or_default();
            let slide_keys: BTreeSet<String> = slides.keys().cloned().collect();
            let layout_keys: BTreeSet<String> = layouts.keys().cloned().collect();
            let mut s = slides;
            for gone in old_slides.difference(&slide_keys) {
                s.insert(gone.clone(), Value::Null);
            }
            let mut l = layouts;
            for gone in old_layouts.difference(&layout_keys) {
                l.insert(gone.clone(), Value::Null);
            }
            self.shown.insert(name.clone(), (slide_keys, layout_keys));
            if self.documents.contains(&name) {
                documents.insert(
                    name,
                    json!({"current_slide": current, "slides": s, "layouts": l}),
                );
            }
        }
        if !documents.is_empty() {
            patch.insert("documents".into(), Value::Object(documents));
        }
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
    }

    fn finish_export(&mut self, cx: &mut Cx, result: Result<String, String>) {
        let Some(export) = self.exporting.take() else {
            return;
        };
        let (status, error) = match result {
            Ok(_) => ("done", None),
            Err(e) => {
                cx.log(
                    Level::Warning,
                    format!("transfer {} ({}) failed: {e}", export.number, export.kind),
                );
                ("failed", Some(e))
            }
        };
        cx.state(
            json!({"transfers": {export.number.to_string(): {"status": status, "error": error}}}),
        );
        let mut args = Self::target(&export);
        args["status"] = json!(status);
        args["error"] = json!(error);
        cx.message("transfer_finished", None, args, self.source);
        self.finished.push_back(export.number);
        while self.finished.len() > KEEP_FINISHED {
            let old = self.finished.pop_front().unwrap_or_default();
            cx.state(json!({"transfers": {old.to_string(): null}}));
        }
        self.pump_exports(cx);
    }
}

impl Module for Keynote {
    fn start(&mut self, cx: &mut Cx) {
        if !self.available {
            self.set_connected(cx, false, UNAVAILABLE);
            return;
        }
        cx.connection(Connection::Connecting);
        if self.monitor {
            self.read(cx, Read::Status);
            self.read(cx, Read::Slides);
        } else {
            self.read(cx, Read::Liveness);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.available {
            cx.complete(
                id,
                Err(CommandError::Transport {
                    message: UNAVAILABLE.into(),
                }),
            );
            return;
        }
        match export_script(name, params) {
            Ok(Some((script, path))) => {
                let number = self.next_transfer;
                self.next_transfer += 1;
                let document = params
                    .get("document")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let export = Export {
                    number,
                    kind: name.to_string(),
                    path,
                    document,
                    script,
                };
                let mut entry = Self::target(&export);
                if let Some(o) = entry.as_object_mut() {
                    o.remove("transfer");
                    o.insert("status".into(), json!("queued"));
                    o.insert("error".into(), Value::Null);
                }
                cx.state(json!({"transfers": {number.to_string(): entry}}));
                let ahead = self.exports.len() + usize::from(self.exporting.is_some());
                self.exports.push_back(export);
                cx.complete(
                    id,
                    Ok(Outcome::Value {
                        value: json!({"transfer": number, "ahead": ahead}),
                    }),
                );
                self.pump_exports(cx);
                return;
            }
            Ok(None) => {}
            Err(e) => {
                cx.complete(id, Err(e));
                return;
            }
        }
        if name == "refresh" {
            // Answered once both reads are queued; their results follow in
            // state.
            self.read(cx, Read::Status);
            self.read(cx, Read::Slides);
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        match command_script(name, params) {
            Ok((script, then)) => {
                // Ahead of queued reads, behind other commands.
                let at = self
                    .queue
                    .iter()
                    .position(|j| matches!(j, Job::Read(_)))
                    .unwrap_or(self.queue.len());
                self.queue.insert(at, Job::Command { id, script, then });
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn osascript(&mut self, cx: &mut Cx, key: Key, result: Result<String, String>) {
        if key == EXPORT {
            self.finish_export(cx, result);
            return;
        }
        if key != SCRIPT {
            return;
        }
        let Some((job, sent)) = self.running.take() else {
            return;
        };
        cx.round_trip(cx.now().saturating_sub(sent));
        match &result {
            Ok(_) => {
                self.set_connected(cx, true, "");
                cx.alive();
            }
            Err(e) => {
                if let Some(reason) = unreachable_reason(e) {
                    self.set_connected(cx, false, &reason);
                }
            }
        }
        match job {
            Job::Command { id, then, .. } => match result {
                Ok(_) => {
                    cx.complete(id, Ok(Outcome::Ack));
                    if self.monitor {
                        self.queue.push_back(Job::Read(Read::Status));
                        if then == Then::Both {
                            self.queue.push_back(Job::Read(Read::Slides));
                        }
                    } else {
                        self.queue.push_back(Job::Read(Read::Liveness));
                    }
                }
                Err(message) => {
                    let code = error_code(&message);
                    let error = if message.starts_with("the script did not finish") {
                        CommandError::Timeout
                    } else if unreachable_reason(&message).is_some() {
                        CommandError::Transport { message }
                    } else {
                        CommandError::DeviceError { code, message }
                    };
                    cx.complete(id, Err(error));
                }
            },
            Job::Read(read) => {
                match (&result, read) {
                    (Ok(out), Read::Status) => self.apply_status(cx, out),
                    (Ok(out), Read::Slides) => self.apply_slides(cx, out),
                    (Ok(out), Read::Liveness) => {
                        cx.state(json!({"running": out.trim() == "1"}));
                    }
                    (Err(e), _) => cx.log(Level::Debug, format!("Keynote read failed: {e}")),
                }
                let (timer, every) = match read {
                    Read::Status => (POLL, self.poll_ms),
                    Read::Slides => (SLIDES, self.slides_poll_ms),
                    Read::Liveness => (LIVENESS, LIVENESS_EVERY),
                };
                cx.set_timer(timer, every);
            }
        }
        self.pump(cx);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            POLL => self.read(cx, Read::Status),
            SLIDES => self.read(cx, Read::Slides),
            LIVENESS => self.read(cx, Read::Liveness),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        for timer in [POLL, SLIDES, LIVENESS] {
            cx.cancel_timer(timer);
        }
        // Commands still waiting will not run.
        for job in self.queue.drain(..) {
            if let Job::Command { id, .. } = job {
                cx.complete(id, Err(CommandError::Closed));
            }
        }
        if let Some((Job::Command { id, .. }, _)) = self.running.take() {
            cx.complete(id, Err(CommandError::Closed));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};

    fn keynote(monitor: bool, available: bool) -> Keynote {
        let spec = Catalog::source_tree().devices["apple-keynote"].clone();
        let settings = validate(&spec.settings, json!({}).as_object().unwrap()).unwrap();
        Keynote::with_host(
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                host_name: None,
                port: None,
                model: "keynote".into(),
                channels: None,
                settings,
                monitor,
            },
            available,
        )
        .unwrap()
    }

    /// The scripts the module asked to run, by key.
    fn scripts(actions: &[Action]) -> Vec<(Key, String)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::OsaScript { key, script, .. } => Some((*key, script.clone())),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn states(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// Start a monitored module and answer its first reads: Keynote not
    /// running.
    fn started() -> Keynote {
        let mut k = keynote(true, true);
        let mut cx = Cx::new(0);
        k.start(&mut cx);
        k.osascript(&mut cx, SCRIPT, Ok("0".into()));
        k.osascript(&mut cx, SCRIPT, Ok("0".into()));
        k
    }

    #[test]
    fn caller_text_is_only_ever_an_escaped_string_literal() {
        assert_eq!(quote("Show").unwrap(), "\"Show\"");
        assert_eq!(quote("a \"b\" \\ c").unwrap(), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(
            quote("one\ntwo\r\tthree").unwrap(),
            "(\"one\" & linefeed & \"two\" & return & \"\" & tab & \"three\")"
        );
        // A quote cannot end the literal and start a statement.
        let evil = "\" & (do shell script \"rm -rf ~\") & \"";
        let q = quote(evil).unwrap();
        assert_eq!(q, "\"\\\" & (do shell script \\\"rm -rf ~\\\") & \\\"\"");
        assert!(quote("bell\u{7}").is_err());
        assert!(quote("\u{1e}").is_err());
    }

    #[test]
    fn commands_run_their_fixed_scripts_and_read_back_what_they_changed() {
        let mut k = started();
        let mut cx = Cx::new(100);
        k.command(
            &mut cx,
            1,
            "set_presenter_notes",
            &params(json!({"document": "Keynote \"Q3\".key", "slide": 4, "notes": "Hello\nworld"})),
        );
        let a = cx.take();
        assert_eq!(
            scripts(&a),
            [(
                SCRIPT,
                "tell application id \"com.apple.iWork.Keynote\"\n\tset presenter notes of slide 4 of document \"Keynote \\\"Q3\\\".key\" to (\"Hello\" & linefeed & \"world\")\nend tell".to_string()
            )]
        );
        let mut cx = Cx::new(140);
        k.osascript(&mut cx, SCRIPT, Ok(String::new()));
        let a = cx.take();
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        assert!(a.iter().any(|a| matches!(a, Action::RoundTrip(40))));
        // The status is read at once, then the slides.
        assert_eq!(scripts(&a), [(SCRIPT, status_script())]);
        let mut cx = Cx::new(150);
        k.osascript(&mut cx, SCRIPT, Ok("0".into()));
        assert_eq!(scripts(&cx.take()), [(SCRIPT, slides_script())]);
    }

    #[test]
    fn every_command_the_spec_declares_builds_a_script() {
        let spec = Catalog::source_tree().devices["apple-keynote"].clone();
        for (name, command) in &spec.commands {
            let mut p = Params::new();
            for (param, ps) in &command.params {
                let v = match (&ps.default, ps.kind) {
                    (Some(d), _) => d.clone(),
                    (None, crate::catalog::ParamType::Int) => {
                        json!(ps.min.unwrap_or(1.0).max(1.0) as i64)
                    }
                    (None, crate::catalog::ParamType::Float) => json!(ps.min.unwrap_or(0.0)),
                    (None, crate::catalog::ParamType::Bool) => json!(true),
                    (None, crate::catalog::ParamType::Enum) => {
                        json!(ps.values.as_ref().unwrap()[0])
                    }
                    (None, _) if param == "path" => json!("/Users/me/Show.key"),
                    (None, _) => json!("Name"),
                };
                p.insert(param.clone(), v);
            }
            let p = validate(&command.params, &p).unwrap();
            let mut k = started();
            let mut cx = Cx::new(10);
            k.command(&mut cx, 7, name, &p);
            let a = cx.take();
            let ran = scripts(&a);
            let answered = completed(&a);
            assert!(
                !ran.is_empty() || answered.iter().any(|(_, r)| r.is_ok()),
                "{name}: no script and no answer: {a:?}"
            );
            assert!(
                answered.iter().all(|(_, r)| r.is_ok()),
                "{name}: {answered:?}"
            );
        }
    }

    #[test]
    fn the_status_and_slides_reads_become_state_and_what_is_gone_is_removed() {
        let mut k = started();
        let mut cx = Cx::new(1000);
        k.timer(&mut cx, POLL);
        cx.take();
        let u = US.to_string();
        let r = RS.to_string();
        let status = ["1", "true", "false", "14.4", "true"].join(&u)
            + &r
            + &[
                "D",
                "Show.key",
                "/Users/me/Show.key",
                "false",
                "12",
                "3",
                "false",
                "false",
                "false",
                "15",
                "true",
                "1920",
                "1080",
                "Basic Black",
            ]
            .join(&u)
            + &r
            + &[
                "D", "Untitled", "", "true", "1", "1", "", "", "", "", "", "", "", "",
            ]
            .join(&u);
        let mut cx = Cx::new(1010);
        k.osascript(&mut cx, SCRIPT, Ok(status));
        let s = states(&cx.take());
        assert_eq!(
            s,
            [json!({
                "running": true, "playing": true, "slide_switcher_visible": false, "version": "14.4",
                "frontmost": true, "front_document": "Show.key",
                "documents": {
                    "Show.key": {"file": "/Users/me/Show.key", "modified": false, "slide_count": 12,
                                 "current_slide_number": 3, "auto_loop": false, "auto_play": false,
                                 "auto_restart": false, "maximum_idle_duration": 15,
                                 "slide_numbers_showing": true, "width": 1920, "height": 1080,
                                 "theme": "Basic Black"},
                    "Untitled": {"file": "", "modified": true, "slide_count": 1, "current_slide_number": 1}
                }
            })]
        );
        let slides = "1".to_string()
            + &r
            + &["T", "1", "Basic Black", "Application/Black/Standard"].join(&u)
            + &r
            + &["D", "Show.key", "3"].join(&u)
            + &r
            + &["L", "1", "Title"].join(&u)
            + &r
            + &[
                "S",
                "1",
                "1",
                "false",
                "true",
                "false",
                "Title",
                "Welcome",
                "",
                "Line one\nLine two",
            ]
            .join(&u)
            + &r
            + &[
                "S",
                "2",
                "-1",
                "true",
                "true",
                "true",
                "Title & Bullets",
                "Agenda",
                "a\nb",
                "",
            ]
            .join(&u);
        let mut cx = Cx::new(1020);
        k.timer(&mut cx, SLIDES);
        k.osascript(&mut cx, SCRIPT, Ok(slides));
        let s = states(&cx.take());
        assert_eq!(
            s,
            [json!({
                "themes": {"1": {"name": "Basic Black", "id": "Application/Black/Standard"}},
                "documents": {"Show.key": {
                    "current_slide": 3,
                    "layouts": {"1": "Title"},
                    "slides": {
                        "1": {"slide_number": 1, "skipped": false, "title_showing": true, "body_showing": false,
                              "layout": "Title", "title": "Welcome", "body": "", "presenter_notes": "Line one\nLine two"},
                        "2": {"slide_number": -1, "skipped": true, "title_showing": true, "body_showing": true,
                              "layout": "Title & Bullets", "title": "Agenda", "body": "a\nb", "presenter_notes": ""}
                    }
                }}
            })]
        );
        // The second slide deleted, the untitled document closed.
        let status = ["1", "false", "false", "14.4", "false"].join(&u)
            + &r
            + &["D", "Show.key", "/Users/me/Show.key", "true", "1", "1"].join(&u);
        let mut cx = Cx::new(2000);
        k.timer(&mut cx, POLL);
        k.osascript(&mut cx, SCRIPT, Ok(status));
        let s = states(&cx.take());
        assert_eq!(s[0]["documents"]["Untitled"], Value::Null);
        let slides = "1".to_string()
            + &r
            + &["D", "Show.key", "1"].join(&u)
            + &r
            + &[
                "S", "1", "1", "false", "true", "false", "Title", "Welcome", "", "",
            ]
            .join(&u);
        let mut cx = Cx::new(2010);
        k.timer(&mut cx, SLIDES);
        k.osascript(&mut cx, SCRIPT, Ok(slides));
        let s = states(&cx.take());
        assert_eq!(s[0]["documents"]["Show.key"]["slides"]["2"], Value::Null);
        assert_eq!(s[0]["documents"]["Show.key"]["layouts"]["1"], Value::Null);
        assert_eq!(s[0]["themes"]["1"], Value::Null);
    }

    #[test]
    fn keynote_not_running_is_reported_and_not_started_by_a_read() {
        let mut k = keynote(true, true);
        let mut cx = Cx::new(0);
        k.start(&mut cx);
        let a = cx.take();
        assert_eq!(scripts(&a), [(SCRIPT, status_script())]);
        assert!(status_script().starts_with("set US to character id 31\nset RS to character id 30\nif not (running of application id \"com.apple.iWork.Keynote\") then return \"0\""));
        let mut cx = Cx::new(5);
        k.osascript(&mut cx, SCRIPT, Ok("0".into()));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Connected))));
        assert_eq!(
            states(&a),
            [
                json!({"running": false, "playing": false, "slide_switcher_visible": false,
                    "frontmost": false, "front_document": ""})
            ]
        );
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key, after: 1000 } if *key == POLL)));
    }

    #[test]
    fn errors_carry_applescripts_number_and_a_refused_permission_disconnects() {
        let mut k = started();
        let mut cx = Cx::new(10);
        k.command(
            &mut cx,
            3,
            "stop_slideshow",
            &params(json!({"document": "Gone.key"})),
        );
        k.osascript(
            &mut cx,
            SCRIPT,
            Err(
                "execution error: Keynote got an error: Can't get document \"Gone.key\". (-1728)"
                    .into(),
            ),
        );
        let a = cx.take();
        assert_eq!(
            completed(&a),
            [(
                3,
                Err(CommandError::DeviceError {
                    code: Some("-1728".into()),
                    message: "execution error: Keynote got an error: Can't get document \"Gone.key\". (-1728)".into()
                })
            )]
        );
        let mut cx = Cx::new(20);
        k.command(&mut cx, 4, "show_next", &Params::new());
        k.osascript(
            &mut cx,
            SCRIPT,
            Err("execution error: Not authorized to send Apple events to Keynote. (-1743)".into()),
        );
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("Automation"))));
        assert!(matches!(
            completed(&a)[0].1,
            Err(CommandError::Transport { .. })
        ));
    }

    #[test]
    fn an_export_answers_at_once_and_is_followed_as_a_transfer() {
        let mut k = started();
        let mut cx = Cx::new(10);
        k.command(
            &mut cx,
            5,
            "export_pdf",
            &params(json!({"document": "", "path": "/Users/me/Show.pdf", "skipped_slides": false,
                           "all_stages": false, "slide_numbers": true, "date": false, "borders": false,
                           "image_quality": "best", "password": "", "password_hint": ""})),
        );
        let a = cx.take();
        assert_eq!(
            completed(&a),
            [(
                5,
                Ok(Outcome::Value {
                    value: json!({"transfer": 1, "ahead": 0})
                })
            )]
        );
        assert_eq!(
            scripts(&a),
            [(EXPORT, "tell application id \"com.apple.iWork.Keynote\"\n\twith timeout of 86400 seconds\n\t\texport front document to (POSIX file \"/Users/me/Show.pdf\") as PDF with properties {skipped slides:false, all stages:false, slide numbers:true, date:false, borders:false, PDF image quality:Best}\n\tend timeout\nend tell".to_string())]
        );
        assert!(a.iter().any(
            |a| matches!(a, Action::Message { address, .. } if address == "transfer_started")
        ));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::OsaScript { timeout: None, .. })));
        let mut cx = Cx::new(90_000);
        k.osascript(&mut cx, EXPORT, Ok(String::new()));
        let a = cx.take();
        assert_eq!(
            states(&a),
            [json!({"transfers": {"1": {"status": "done", "error": null}}})]
        );
        assert!(a.iter().any(
            |a| matches!(a, Action::Message { address, .. } if address == "transfer_finished")
        ));
    }

    #[test]
    fn a_relative_path_or_a_control_character_is_refused_before_any_script() {
        let mut k = started();
        let mut cx = Cx::new(10);
        k.command(
            &mut cx,
            1,
            "open_document",
            &params(json!({"path": "Show.key"})),
        );
        k.command(
            &mut cx,
            2,
            "set_slide_title",
            &params(json!({"document": "", "slide": 1, "text": "a\u{1b}b"})),
        );
        let a = cx.take();
        assert!(scripts(&a).is_empty());
        assert!(completed(&a)
            .iter()
            .all(|(_, r)| matches!(r, Err(CommandError::InvalidParams { .. }))));
    }

    #[test]
    fn opened_for_commands_only_it_checks_only_that_keynote_runs() {
        let mut k = keynote(false, true);
        let mut cx = Cx::new(0);
        k.start(&mut cx);
        let a = cx.take();
        assert_eq!(scripts(&a), [(SCRIPT, liveness_script())]);
        let mut cx = Cx::new(30);
        k.osascript(&mut cx, SCRIPT, Ok("1".into()));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unmonitored))));
        assert_eq!(states(&a), [json!({"running": true})]);
        assert!(a.iter().any(|a| matches!(a, Action::RoundTrip(30))));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key, after: 10_000 } if *key == LIVENESS)));
        // A command still works, and is followed by the liveness check only.
        let mut cx = Cx::new(40);
        k.command(&mut cx, 9, "show_next", &Params::new());
        k.osascript(&mut cx, SCRIPT, Ok(String::new()));
        let a = cx.take();
        assert_eq!(completed(&a), [(9, Ok(Outcome::Ack))]);
        assert_eq!(scripts(&a)[1], (SCRIPT, liveness_script()));
    }

    #[test]
    fn without_macos_it_is_unavailable() {
        let mut k = keynote(true, false);
        let mut cx = Cx::new(0);
        k.start(&mut cx);
        k.command(&mut cx, 1, "show_next", &Params::new());
        let a = cx.take();
        assert!(scripts(&a).is_empty());
        assert!(a.iter().any(|a| matches!(a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("macOS"))));
        assert!(matches!(
            completed(&a)[0].1,
            Err(CommandError::Transport { .. })
        ));
    }
}
