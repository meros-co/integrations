//! `transport.cookies: true` (SPEC.md §2, "Cookies"): the cookies a device
//! sets on any answer, kept and sent back on every later request to it, as
//! a browser does (an Axis camera's PTZ control queue tells its clients
//! apart by a cookie its queue request sets).
//!
//! A small jar: one origin, names and values only. A cookie set again
//! replaces the old one; one set empty, with `Max-Age` zero or below, or
//! with an `Expires` date in the past, is removed. Paths and domains are not
//! told apart, since every request goes to the one device. The cookies are
//! secrets: never logged, offered to the rules or kept in state.

use crate::module::HttpResponse;

#[derive(Debug, Default, Clone)]
pub(crate) struct Jar {
    cookies: Vec<(String, String)>,
}

/// Whether an `Expires` date (RFC 1123, `Wdy, DD Mon YYYY HH:MM:SS GMT`)
/// lies before `unix` seconds. A date that cannot be read is not.
fn expired(date: &str, unix: i64) -> bool {
    let parts: Vec<&str> = date
        .split([' ', ',', '-', ':'])
        .filter(|p| !p.is_empty())
        .collect();
    // Wdy DD Mon YYYY HH MM SS GMT
    if parts.len() < 7 {
        return false;
    }
    let months = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let (Ok(day), Some(month), Ok(mut year), Ok(h), Ok(m), Ok(s)) = (
        parts[1].parse::<i64>(),
        months
            .iter()
            .position(|x| parts[2].to_ascii_lowercase().starts_with(x)),
        parts[3].parse::<i64>(),
        parts[4].parse::<i64>(),
        parts[5].parse::<i64>(),
        parts[6].parse::<i64>(),
    ) else {
        return false;
    };
    if year < 100 {
        year += if year < 70 { 2000 } else { 1900 };
    }
    // Days from civil (Howard Hinnant).
    let month = month as i64 + 1;
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + h * 3600 + m * 60 + s < unix
}

impl Jar {
    /// Take the cookies `response` sets, at `unix_ms` on the wall clock.
    pub(crate) fn take(&mut self, response: &HttpResponse, unix_ms: u64) {
        let unix = (unix_ms / 1000) as i64;
        for header in response.header_values("set-cookie") {
            let mut parts = header.split(';');
            let Some((name, value)) = parts.next().and_then(|p| p.split_once('=')) else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim().trim_matches('"'));
            if name.is_empty() {
                continue;
            }
            let mut gone = value.is_empty();
            for attribute in parts {
                let (key, v) = attribute.split_once('=').unwrap_or((attribute, ""));
                match key.trim().to_ascii_lowercase().as_str() {
                    "max-age" => gone |= v.trim().parse::<i64>().is_ok_and(|age| age <= 0),
                    "expires" => gone |= expired(v.trim(), unix),
                    _ => {}
                }
            }
            self.cookies.retain(|(n, _)| n != name);
            if !gone {
                self.cookies.push((name.to_string(), value.to_string()));
            }
        }
    }

    /// The `Cookie` header's value, or `None` while the jar is empty.
    pub(crate) fn header(&self) -> Option<String> {
        (!self.cookies.is_empty()).then(|| {
            self.cookies
                .iter()
                .map(|(n, v)| format!("{n}={v}"))
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    pub(crate) fn clear(&mut self) {
        self.cookies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(cookies: &[&str]) -> HttpResponse {
        HttpResponse {
            status: 200,
            headers: cookies
                .iter()
                .map(|c| ("set-cookie".to_string(), c.to_string()))
                .collect(),
            body: Vec::new(),
        }
    }

    #[test]
    fn cookies_are_kept_replaced_and_removed() {
        let mut jar = Jar::default();
        assert_eq!(jar.header(), None);
        jar.take(&setting(&["ptz=1a2b; path=/", "other=x"]), 0);
        assert_eq!(jar.header().as_deref(), Some("ptz=1a2b; other=x"));
        jar.take(&setting(&["ptz=3c4d; HttpOnly"]), 0);
        assert_eq!(jar.header().as_deref(), Some("other=x; ptz=3c4d"));
        jar.take(&setting(&["other=; Max-Age=0"]), 0);
        assert_eq!(jar.header().as_deref(), Some("ptz=3c4d"));
        jar.take(
            &setting(&["ptz=gone; Expires=Thu, 01 Jan 1970 00:00:00 GMT"]),
            5_000,
        );
        assert_eq!(jar.header(), None);
        jar.take(
            &setting(&["a=b; Expires=Fri, 31 Dec 2100 23:59:59 GMT"]),
            5_000,
        );
        assert_eq!(jar.header().as_deref(), Some("a=b"));
        jar.clear();
        assert_eq!(jar.header(), None);
    }
}
