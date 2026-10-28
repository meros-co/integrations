//! Paged lists (SPEC.md §4, "Paged lists"): a request item with `pages`
//! asks for every page of a list in turn and answers with one reply holding
//! every item, so a list command returns the whole list and a poll's
//! `replace` rule clears and rewrites it once.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use super::expect::json_path;
use super::template::percent_encode;
use crate::module::HttpResponse;

/// Where a value of a page's answer is read: its JSON body, a header, or
/// JSON held in a header (BoxCast's `X-Pagination`).
#[derive(Debug, Clone, PartialEq)]
enum Source {
    Body(String),
    Header(String),
    HeaderJson(String, String),
}

impl Source {
    fn parse(v: &Value, what: &str) -> Result<Source, String> {
        let path = |p: &str| -> Result<String, String> {
            if p.starts_with('$') {
                Ok(p.to_string())
            } else {
                Err(format!("pages: {what} '{p}' is not a JSON path"))
            }
        };
        match v {
            Value::String(p) => Ok(Source::Body(path(p)?)),
            Value::Object(o) => {
                let header = o
                    .get("header")
                    .and_then(Value::as_str)
                    .filter(|h| !h.is_empty())
                    .ok_or(format!("pages: {what} is a JSON path or {{header, json?}}"))?;
                match o.get("json") {
                    None => Ok(Source::Header(header.to_ascii_lowercase())),
                    Some(Value::String(p)) => {
                        Ok(Source::HeaderJson(header.to_ascii_lowercase(), path(p)?))
                    }
                    Some(_) => Err(format!("pages: {what}.json is a JSON path")),
                }
            }
            _ => Err(format!("pages: {what} is a JSON path or {{header, json?}}")),
        }
    }

    /// The value it names in a page's answer, if any.
    fn read(&self, response: &HttpResponse, body: Option<&Value>) -> Option<Value> {
        let header = |name: &str| {
            response
                .headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
        };
        match self {
            Source::Body(path) => body.and_then(|b| json_path(b, path)).cloned(),
            Source::Header(name) => header(name).map(Value::String),
            Source::HeaderJson(name, path) => {
                let doc: Value = serde_json::from_str(&header(name)?).ok()?;
                json_path(&doc, path).cloned()
            }
        }
    }
}

/// A value's text as a cursor, page link or number: nothing for null,
/// `false`, an empty string or an absent value.
fn text(v: Option<Value>) -> Option<String> {
    match v? {
        Value::Null | Value::Bool(false) => None,
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

fn number(v: Option<Value>) -> Option<u64> {
    match v? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// What the items are: arrays at JSON paths, joined page after page, or the
/// elements of one name directly inside an XML answer's root.
#[derive(Debug, Clone, PartialEq)]
enum Items {
    Json(Vec<String>),
    Xml(String),
}

/// How the next page is asked for.
#[derive(Debug, Clone, PartialEq)]
enum Next {
    /// A link to the next page, its path and query sent to the same server.
    Link(Source),
    /// A `Link` header's target with this relation (RFC 8288).
    Rel(String, String),
    /// A cursor from the answer, sent in this query parameter.
    Cursor(Source, String),
    /// This query parameter counts pages, one more each time.
    Page(String),
    /// This query parameter counts items skipped, by the items received.
    Offset(String),
}

/// A request item's `pages`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pages {
    items: Items,
    next: Next,
    /// Go on only while this is present and not false.
    more: Option<Source>,
    /// Every item has arrived once this many have.
    total: Option<Source>,
    /// Every page has arrived once this many have.
    page_count: Option<Source>,
    /// The query parameter giving the page size: a page holding fewer items
    /// is the last.
    size: Option<String>,
}

impl Pages {
    pub(crate) fn parse(v: &Value) -> Result<Pages, String> {
        let o = v.as_object().ok_or("pages is a mapping")?;
        for key in o.keys() {
            if ![
                "items",
                "next",
                "cursor",
                "query",
                "page",
                "offset",
                "more",
                "total",
                "page_count",
                "size",
            ]
            .contains(&key.as_str())
            {
                return Err(format!("pages: unknown key '{key}'"));
            }
        }
        let items = match o.get("items") {
            Some(Value::String(p)) if p.starts_with('$') => Items::Json(vec![p.clone()]),
            Some(Value::Array(ps)) if !ps.is_empty() => Items::Json(
                ps.iter()
                    .map(|p| {
                        p.as_str()
                            .filter(|p| p.starts_with('$'))
                            .map(str::to_string)
                            .ok_or("pages: items are JSON paths".to_string())
                    })
                    .collect::<Result<_, _>>()?,
            ),
            Some(Value::Object(x)) => match x.get("xml").and_then(Value::as_str) {
                Some(name) if !name.is_empty() && x.len() == 1 => Items::Xml(name.into()),
                _ => return Err("pages: items is {xml: element}".into()),
            },
            _ => {
                return Err(
                    "pages needs items: a JSON path, a list of them or {xml: element}".into(),
                )
            }
        };
        let query = |key: &str| -> Result<String, String> {
            o.get(key)
                .and_then(Value::as_str)
                .filter(|q| !q.is_empty())
                .map(str::to_string)
                .ok_or(format!("pages: {key} names a query parameter"))
        };
        let mut nexts = Vec::new();
        if let Some(n) = o.get("next") {
            nexts.push(match n {
                Value::Object(x) if x.contains_key("rel") => Next::Rel(
                    x.get("header")
                        .and_then(Value::as_str)
                        .unwrap_or("link")
                        .to_ascii_lowercase(),
                    x.get("rel")
                        .and_then(Value::as_str)
                        .ok_or("pages: next.rel is a link relation")?
                        .to_string(),
                ),
                other => Next::Link(Source::parse(other, "next")?),
            });
        }
        if let Some(c) = o.get("cursor") {
            nexts.push(Next::Cursor(Source::parse(c, "cursor")?, query("query")?));
        } else if o.contains_key("query") {
            return Err("pages: query goes with cursor".into());
        }
        if o.contains_key("page") {
            nexts.push(Next::Page(query("page")?));
        }
        if o.contains_key("offset") {
            nexts.push(Next::Offset(query("offset")?));
        }
        if nexts.len() != 1 {
            return Err("pages needs one of next, cursor, page or offset".into());
        }
        let source = |key: &str| o.get(key).map(|v| Source::parse(v, key)).transpose();
        Ok(Pages {
            items,
            next: nexts.remove(0),
            more: source("more")?,
            total: source("total")?,
            page_count: source("page_count")?,
            size: match o.get("size") {
                None => None,
                Some(_) => Some(query("size")?),
            },
        })
    }
}

/// What a page's answer leads to.
pub(crate) enum Step {
    /// Ask for the page at this URL.
    Fetch(String),
    /// Every page has arrived: the reply holding every item.
    Done(Vec<u8>),
}

/// A paged list being read: the pages so far.
#[derive(Debug)]
pub(crate) struct Paging {
    pages: Arc<Pages>,
    /// The URL of the page asked for last.
    url: String,
    /// The answers' items so far: per JSON path, or the XML elements' text.
    json: Vec<Vec<Value>>,
    xml: Vec<String>,
    received: u64,
    fetched: u64,
    seen: HashSet<String>,
}

impl Paging {
    pub(crate) fn new(pages: Arc<Pages>, url: &str) -> Paging {
        let lists = match &pages.items {
            Items::Json(paths) => paths.len(),
            Items::Xml(_) => 0,
        };
        Paging {
            pages,
            url: url.to_string(),
            json: vec![Vec::new(); lists],
            xml: Vec::new(),
            received: 0,
            fetched: 0,
            seen: HashSet::from([url.to_string()]),
        }
    }

    /// Take a page's successful answer. An answer that is not the list the
    /// spec names (not JSON, or no array where the items are) ends the
    /// reading with what it is, so its rules and `expect` see it as it came.
    pub(crate) fn page(&mut self, response: &HttpResponse) -> Step {
        self.fetched += 1;
        let pages = self.pages.clone();
        let (on_page, merged) = match &pages.items {
            Items::Json(paths) => {
                let Ok(doc) = serde_json::from_slice::<Value>(&response.body) else {
                    return Step::Done(response.body.clone());
                };
                let mut on_page = 0;
                for (i, path) in paths.iter().enumerate() {
                    match json_path(&doc, path) {
                        Some(Value::Array(items)) => {
                            if i == 0 {
                                on_page = items.len() as u64;
                            }
                            self.json[i].extend(items.iter().cloned());
                        }
                        // A list a page leaves out is empty there.
                        None | Some(Value::Null) if i > 0 => {}
                        _ => return Step::Done(response.body.clone()),
                    }
                }
                self.received += on_page;
                let next = self.next_url(response, Some(&doc), on_page);
                match next {
                    Some(url) => return Step::Fetch(url),
                    None => {
                        let mut doc = doc;
                        for (path, items) in paths.iter().zip(&mut self.json) {
                            set_path(&mut doc, path, Value::Array(std::mem::take(items)));
                        }
                        (on_page, doc.to_string().into_bytes())
                    }
                }
            }
            Items::Xml(name) => {
                let Ok(text) = std::str::from_utf8(&response.body) else {
                    return Step::Done(response.body.clone());
                };
                let Some((elements, _)) = xml_items(text, name) else {
                    return Step::Done(response.body.clone());
                };
                let on_page = elements.len() as u64;
                self.received += on_page;
                self.xml
                    .extend(elements.into_iter().map(|r| text[r].to_string()));
                match self.next_url(response, None, on_page) {
                    Some(url) => return Step::Fetch(url),
                    None => (on_page, xml_merged(text, name, &self.xml).into_bytes()),
                }
            }
        };
        let _ = on_page;
        Step::Done(merged)
    }

    /// The next page's URL, or none when this page was the last.
    fn next_url(
        &mut self,
        response: &HttpResponse,
        body: Option<&Value>,
        on_page: u64,
    ) -> Option<String> {
        let p = self.pages.clone();
        if let Some(more) = &p.more {
            text(more.read(response, body))?;
        }
        if let Some(total) = &p.total {
            if number(total.read(response, body)).is_some_and(|t| self.received >= t) {
                return None;
            }
        }
        if let Some(count) = &p.page_count {
            if number(count.read(response, body)).is_some_and(|c| self.fetched >= c) {
                return None;
            }
        }
        if let Some(size) = &p.size {
            let asked = query_value(&self.url, size).and_then(|v| v.parse::<u64>().ok());
            if asked.is_some_and(|n| on_page < n) {
                return None;
            }
        }
        let url = match &p.next {
            Next::Link(source) => resolve(&self.url, &text(source.read(response, body))?)?,
            Next::Rel(header, rel) => {
                let target = response
                    .headers
                    .iter()
                    .filter(|(n, _)| n == header)
                    .find_map(|(_, v)| link_target(v, rel))?;
                resolve(&self.url, &target)?
            }
            Next::Cursor(source, query) => {
                with_query(&self.url, query, &text(source.read(response, body))?)
            }
            Next::Page(query) => {
                if on_page == 0 {
                    return None;
                }
                let n: i64 = query_value(&self.url, query)?.parse().ok()?;
                with_query(&self.url, query, &(n + 1).to_string())
            }
            Next::Offset(query) => {
                if on_page == 0 {
                    return None;
                }
                let n: u64 = query_value(&self.url, query).map_or(Some(0), |v| v.parse().ok())?;
                with_query(&self.url, query, &(n + on_page).to_string())
            }
        };
        // A page that names one already read would go round forever.
        if !self.seen.insert(url.clone()) {
            return None;
        }
        self.url = url.clone();
        Some(url)
    }
}

/// A request item's extras over HTTP: its `pages`, and where a `save`
/// parameter has its answer saved (SPEC.md §4).
#[derive(Debug, Default)]
pub(crate) struct HttpExtra {
    pub(crate) pages: Option<Arc<Pages>>,
    pub(crate) save: Option<PathBuf>,
}

/// Set the value at a JSON path of members (`$`, `$.a.b`), creating the
/// objects on the way.
fn set_path(doc: &mut Value, path: &str, value: Value) {
    let rest = path.strip_prefix('$').unwrap_or(path);
    let mut node = doc;
    for key in rest.split('.').filter(|k| !k.is_empty()) {
        if !node.is_object() {
            *node = Value::Object(Default::default());
        }
        node = node
            .as_object_mut()
            .unwrap()
            .entry(key.to_string())
            .or_insert(Value::Null);
    }
    *node = value;
}

/// `scheme://authority` of a URL.
fn origin(url: &str) -> &str {
    let after = url.find("://").map_or(0, |i| i + 3);
    match url[after..].find('/') {
        Some(i) => &url[..after + i],
        None => url,
    }
}

/// A link to the next page, as a URL on the server the list came from: an
/// absolute link gives its path and query only, so a page is never asked
/// of another host; a link from `/` is that path; one starting `?` is a new
/// query on the same path.
fn resolve(current: &str, link: &str) -> Option<String> {
    let link = link.trim();
    if link.contains(['\r', '\n', ' ']) {
        return None;
    }
    let base = origin(current);
    if let Some(after) = link.split_once("://").map(|(_, a)| a) {
        let target = after.find('/').map_or("/", |i| &after[i..]);
        return Some(format!("{base}{target}"));
    }
    if link.starts_with('/') {
        return Some(format!("{base}{link}"));
    }
    if link.starts_with('?') {
        let path = current.split('?').next().unwrap_or(current);
        return Some(format!("{path}{link}"));
    }
    None
}

/// The target of the first link with relation `rel` in a `Link` header
/// (RFC 8288): `<url>; rel="next", <url>; rel="last"`.
fn link_target(header: &str, rel: &str) -> Option<String> {
    let mut rest = header;
    while let Some(start) = rest.find('<') {
        let end = rest[start..].find('>')? + start;
        let target = &rest[start + 1..end];
        let params_end = rest[end..].find(['<']).map_or(rest.len(), |i| end + i);
        let params = &rest[end + 1..params_end];
        let named = params.split(';').any(|p| {
            let p = p.trim().trim_end_matches(',').trim();
            p.strip_prefix("rel=")
                .map(|v| v.trim_matches('"'))
                .is_some_and(|v| v.split_whitespace().any(|r| r.eq_ignore_ascii_case(rel)))
        });
        if named {
            return Some(target.to_string());
        }
        rest = &rest[params_end..];
    }
    None
}

/// The value of a query parameter as the URL sends it.
fn query_value(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let key = percent_encode(key);
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")).map(str::to_string))
}

/// The URL with a query parameter set to `value` (percent-encoded), in its
/// place when the URL has it, otherwise added at the end.
fn with_query(url: &str, key: &str, value: &str) -> String {
    let key = percent_encode(key);
    let pair = format!("{key}={}", percent_encode(value));
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => return format!("{url}?{pair}"),
    };
    let mut found = false;
    let mut pairs: Vec<String> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p == key || p.starts_with(&format!("{key}=")) {
                found = true;
                pair.clone()
            } else {
                p.to_string()
            }
        })
        .collect();
    if !found {
        pairs.push(pair);
    }
    format!("{path}?{}", pairs.join("&"))
}

/// The byte ranges of the elements named `name` directly inside the root of
/// an XML document, and the place just before the root's closing tag.
fn xml_items(text: &str, name: &str) -> Option<(Vec<std::ops::Range<usize>>, usize)> {
    let doc = roxmltree::Document::parse(text).ok()?;
    let root = doc.root_element();
    let items = root
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == name)
        .map(|c| c.range())
        .collect();
    let range = root.range();
    let close = text[range.clone()]
        .rfind("</")
        .map_or(range.end, |i| range.start + i);
    Some((items, close))
}

/// The last page's document with its own items replaced by every page's.
fn xml_merged(last: &str, name: &str, all: &[String]) -> String {
    let Some((own, close)) = xml_items(last, name) else {
        return last.to_string();
    };
    let mut out = String::with_capacity(last.len() + all.iter().map(String::len).sum::<usize>());
    let mut at = 0;
    for range in &own {
        out.push_str(&last[at..range.start]);
        at = range.end;
    }
    // A root written empty (`<list/>`) holds nothing: its items go after
    // its own text, before the closing tag that is not there.
    if at > close {
        out.push_str(&last[at..]);
        return out;
    }
    out.push_str(&last[at..close]);
    for item in all {
        out.push_str(item);
    }
    out.push_str(&last[close..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer(body: Value, headers: &[(&str, &str)]) -> HttpResponse {
        HttpResponse {
            status: 200,
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
            body: body.to_string().into_bytes(),
        }
    }

    fn paging(spec: Value, url: &str) -> Paging {
        Paging::new(Arc::new(Pages::parse(&spec).unwrap()), url)
    }

    fn fetch(step: Step) -> String {
        match step {
            Step::Fetch(url) => url,
            Step::Done(_) => panic!("ended early"),
        }
    }

    fn done(step: Step) -> Value {
        match step {
            Step::Done(body) => serde_json::from_slice(&body).unwrap(),
            Step::Fetch(url) => panic!("asked for {url}"),
        }
    }

    #[test]
    fn a_next_link_is_followed_on_the_same_server_and_the_items_joined() {
        let mut p = paging(
            json!({"items": ["$.data", "$.included"], "next": "$.links.next"}),
            "https://api.example.test:443/v2/items?per_page=2",
        );
        let url = fetch(p.page(&answer(
            json!({"data": [1, 2], "included": [{"id": "a"}],
                   "links": {"next": "https://other.test/v2/items?offset=2&per_page=2"}}),
            &[],
        )));
        assert_eq!(
            url,
            "https://api.example.test:443/v2/items?offset=2&per_page=2"
        );
        let body = done(p.page(&answer(json!({"data": [3], "links": {"self": "x"}}), &[])));
        assert_eq!(
            body,
            json!({"data": [1, 2, 3], "links": {"self": "x"}, "included": [{"id": "a"}]})
        );
    }

    #[test]
    fn a_cursor_goes_in_its_query_until_there_is_no_more() {
        let mut p = paging(
            json!({"items": "$.items", "cursor": "$.next_cursor", "query": "cursor", "more": "$.has_more"}),
            "http://h/rundowns?limit=100",
        );
        let url = fetch(p.page(&answer(
            json!({"items": [1], "next_cursor": "a b", "has_more": true}),
            &[],
        )));
        assert_eq!(url, "http://h/rundowns?limit=100&cursor=a%20b");
        let url = fetch(p.page(&answer(
            json!({"items": [2], "next_cursor": "c", "has_more": true}),
            &[],
        )));
        assert_eq!(url, "http://h/rundowns?limit=100&cursor=c");
        let body = done(p.page(&answer(
            json!({"items": [3], "next_cursor": "d", "has_more": false}),
            &[],
        )));
        assert_eq!(body["items"], json!([1, 2, 3]));
    }

    #[test]
    fn pages_and_offsets_count_on_until_a_short_page_or_the_total() {
        let mut p = paging(
            json!({"items": "$", "page": "page", "size": "per_page"}),
            "http://h/list?page=1&per_page=2",
        );
        assert_eq!(
            fetch(p.page(&answer(json!([1, 2]), &[]))),
            "http://h/list?page=2&per_page=2"
        );
        assert_eq!(done(p.page(&answer(json!([3]), &[]))), json!([1, 2, 3]));

        let mut p = paging(
            json!({"items": "$.data", "offset": "offset", "total": "$.totalCount"}),
            "http://h/sites?offset=0&limit=2",
        );
        let url = fetch(p.page(&answer(json!({"data": [1, 2], "totalCount": 3}), &[])));
        assert_eq!(url, "http://h/sites?offset=2&limit=2");
        assert_eq!(
            done(p.page(&answer(json!({"data": [3], "totalCount": 3}), &[])))["data"],
            json!([1, 2, 3])
        );

        // An empty page ends a list whose size is not known.
        let mut p = paging(
            json!({"items": "$.items", "page": "page", "page_count": "$.pageCount"}),
            "http://h/p?page=0",
        );
        let url = fetch(p.page(&answer(json!({"items": [1], "pageCount": 2}), &[])));
        assert_eq!(url, "http://h/p?page=1");
        assert_eq!(
            done(p.page(&answer(json!({"items": [2], "pageCount": 2}), &[])))["items"],
            json!([1, 2])
        );
    }

    #[test]
    fn link_headers_and_header_json_name_the_next_page() {
        let mut p = paging(
            json!({"items": "$", "next": {"header": "Link", "rel": "next"}}),
            "http://h:8080/x-nmos/query/v1.3/nodes?paging.limit=2",
        );
        let link = r#"<http://h:8080/x-nmos/query/v1.3/nodes?paging.since=1:0&paging.limit=2>; rel="next", </x?a=1>; rel="prev""#;
        let url = fetch(p.page(&answer(json!([1, 2]), &[("link", link)])));
        assert_eq!(
            url,
            "http://h:8080/x-nmos/query/v1.3/nodes?paging.since=1:0&paging.limit=2"
        );
        assert_eq!(
            done(p.page(&answer(json!([3]), &[("link", "</x>; rel=\"prev\"")]))),
            json!([1, 2, 3])
        );

        let mut p = paging(
            json!({"items": "$", "page": "p", "total": {"header": "X-Pagination", "json": "$.total"}}),
            "http://h/account/channels?p=0&l=2",
        );
        let h = [("x-pagination", r#"{"total":3,"next":1,"last":1}"#)];
        assert_eq!(
            fetch(p.page(&answer(json!([1, 2]), &h))),
            "http://h/account/channels?p=1&l=2"
        );
        assert_eq!(done(p.page(&answer(json!([3]), &h))), json!([1, 2, 3]));
    }

    #[test]
    fn a_page_that_repeats_one_already_read_ends_the_list() {
        let mut p = paging(json!({"items": "$.d", "next": "$.n"}), "http://h/a?x=1");
        assert_eq!(
            done(p.page(&answer(json!({"d": [1], "n": "/a?x=1"}), &[])))["d"],
            json!([1])
        );
    }

    #[test]
    fn an_answer_that_is_not_the_list_ends_it_as_it_is() {
        let mut p = paging(json!({"items": "$.d", "next": "$.n"}), "http://h/a");
        match p.page(&HttpResponse {
            status: 200,
            headers: vec![],
            body: b"oops".to_vec(),
        }) {
            Step::Done(body) => assert_eq!(body, b"oops"),
            Step::Fetch(_) => panic!(),
        }
    }

    #[test]
    fn xml_pages_join_their_elements_in_the_last_pages_root() {
        let mut p = paging(
            json!({"items": {"xml": "live_event"}, "page": "page", "size": "per_page"}),
            "http://h/api/live_events?page=1&per_page=2",
        );
        let one = r#"<?xml version="1.0"?><live_event_list><live_event id="1"><name>a</name></live_event><live_event id="2"/></live_event_list>"#;
        let two = r#"<?xml version="1.0"?><live_event_list><live_event id="3"/></live_event_list>"#;
        let ok = |t: &str| HttpResponse {
            status: 200,
            headers: vec![],
            body: t.as_bytes().to_vec(),
        };
        assert_eq!(
            fetch(p.page(&ok(one))),
            "http://h/api/live_events?page=2&per_page=2"
        );
        match p.page(&ok(two)) {
            Step::Done(body) => assert_eq!(
                String::from_utf8(body).unwrap(),
                r#"<?xml version="1.0"?><live_event_list><live_event id="1"><name>a</name></live_event><live_event id="2"/><live_event id="3"/></live_event_list>"#
            ),
            Step::Fetch(_) => panic!(),
        }
    }

    #[test]
    fn the_spec_block_is_checked() {
        for bad in [
            json!({"next": "$.n"}),
            json!({"items": "$.d"}),
            json!({"items": "$.d", "next": "$.n", "page": "p"}),
            json!({"items": "$.d", "cursor": "$.c"}),
            json!({"items": "d", "next": "$.n"}),
            json!({"items": "$.d", "next": "$.n", "bogus": 1}),
        ] {
            assert!(Pages::parse(&bad).is_err(), "{bad}");
        }
    }
}
