//! `facebook-account` on real sockets: the spec switched to plain HTTP,
//! against a local Graph API, driven through the public API. Page access
//! tokens must come back as command values and nowhere else.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::catalog::{Catalog, Params};
use crate::{CommandError, Connection, Core, CoreOptions, Event, OpenRequest, Outcome};

const PAGE_TOKEN: &str = "EAAGpagetokenSECRET";
const USER_TOKEN: &str = "EAAGusertokenSECRET";

/// A Graph API answering /me, /me/accounts and /{page-id} for USER_TOKEN,
/// and code 190 for any other token.
async fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") else {
                        match stream.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => data.extend_from_slice(&buf[..n]),
                        }
                        continue;
                    };
                    let head = String::from_utf8_lossy(&data[..end]).to_string();
                    data.drain(..end + 4);
                    let target = head
                        .lines()
                        .next()
                        .and_then(|l| l.split(' ').nth(1))
                        .unwrap_or("")
                        .to_string();
                    let authorized = head.lines().any(|l| {
                        l.to_ascii_lowercase().starts_with("authorization:")
                            && l.ends_with(&format!("Bearer {USER_TOKEN}"))
                    });
                    let path = target.split('?').next().unwrap_or("");
                    let (status, body) = if !authorized {
                        (
                            "400 Bad Request",
                            json!({"error": {"message": "Error validating access token",
                                   "type": "OAuthException", "code": 190, "error_subcode": 463}}),
                        )
                    } else if path.ends_with("/me/accounts") {
                        let page = json!({"id": "1093482904102", "name": "Grace Church",
                                          "category": "Church", "tasks": ["CREATE_CONTENT"]});
                        let mut page = page;
                        if target.contains("access_token") {
                            page["access_token"] = json!(PAGE_TOKEN);
                        }
                        (
                            "200 OK",
                            json!({"data": [page], "paging": {"cursors": {"before": "b", "after": "a"}}}),
                        )
                    } else if path.ends_with("/me") {
                        (
                            "200 OK",
                            json!({"id": "10229384756", "name": "Sam Operator"}),
                        )
                    } else {
                        (
                            "200 OK",
                            json!({"id": "1093482904102", "name": "Grace Church",
                                   "access_token": PAGE_TOKEN}),
                        )
                    };
                    let body = body.to_string();
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if stream.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

fn core() -> Core {
    let mut spec = Catalog::source_tree()
        .device("facebook-account")
        .unwrap()
        .clone();
    spec.transport.as_mut().unwrap()["scheme"] = json!("http");
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    Core::with_catalog(catalog, CoreOptions::new()).unwrap()
}

fn open(core: &Core, port: u16, token: &str) -> u64 {
    let settings: Params = json!({"token": token}).as_object().unwrap().clone();
    core.open(OpenRequest {
        device: "facebook-account".into(),
        model: "graph-api".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings,
        monitor: false,
    })
    .unwrap()
}

fn value(result: crate::CommandResult) -> Value {
    match result {
        Ok(Outcome::Value { value }) => value,
        other => panic!("{other:?}"),
    }
}

/// Every event so far, without waiting long.
async fn drain(core: &Core) -> Vec<Event> {
    let mut all = Vec::new();
    loop {
        let batch = tokio::time::timeout(Duration::from_millis(100), core.next_events(256)).await;
        match batch {
            Ok(events) if !events.is_empty() => all.extend(events),
            _ => return all,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn page_tokens_are_command_values_only() {
    let port = serve().await;
    let core = core();
    let device = open(&core, port, USER_TOKEN);

    let pages = value(core.execute(device, "list_pages", Params::new()).await);
    assert_eq!(pages["data"][0]["access_token"], PAGE_TOKEN);
    assert_eq!(pages["paging"]["cursors"]["after"], "a");
    let names = value(core.execute(device, "list_page_names", Params::new()).await);
    assert!(names["data"][0].get("access_token").is_none());
    let one: Params = json!({"page_id": "1093482904102"})
        .as_object()
        .unwrap()
        .clone();
    let page = value(core.execute(device, "get_page_token", one).await);
    assert_eq!(page["access_token"], PAGE_TOKEN);
    let me = value(core.execute(device, "get_me", Params::new()).await);
    assert_eq!(me["name"], "Sam Operator");

    // State has the user and nothing of the Pages.
    let state = core.snapshot(device).unwrap().state;
    assert_eq!(
        state,
        json!({"user": {"id": "10229384756", "name": "Sam Operator"}})
    );
    for event in drain(&core).await {
        let text = format!("{event:?}");
        assert!(
            !text.contains(PAGE_TOKEN) && !text.contains(USER_TOKEN),
            "{text}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expired_user_token_is_terminal() {
    let port = serve().await;
    let core = core();
    let device = open(&core, port, "expired");
    let result = core.execute(device, "list_pages", Params::new()).await;
    assert!(
        matches!(result, Err(CommandError::Auth { .. })),
        "{result:?}"
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            core.snapshot(device).unwrap().connection,
            Connection::Unauthorized { .. }
        ) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("unauthorized within 5 s");
}
