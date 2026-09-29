//! HTTP for device modules: one client per trust mode, shared by every session.
//!
//! rustls on every platform, so TLS behaves identically wherever the core runs.
//! TLS 1.2 is the floor: no device the core speaks HTTPS to has been found to
//! need less (RFDeck `docs/INTEGRATIONS_CORE_REVIEW.md`, item D).

use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::module::{HttpRequest, HttpResponse, Key, RequestId, SseInput};
use crate::session::Inbound;
use crate::sse::SseParser;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct HttpClients {
    strict: reqwest::Client,
    accept_invalid: reqwest::Client,
}

impl HttpClients {
    pub(crate) fn new() -> reqwest::Result<HttpClients> {
        let build = |accept_invalid: bool| {
            reqwest::Client::builder()
                .use_rustls_tls()
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .danger_accept_invalid_certs(accept_invalid)
                .connect_timeout(CONNECT_TIMEOUT)
                // SSCv2 requires HTTP/1.1; nothing the core speaks needs h2.
                .http1_only()
                .build()
        };
        Ok(HttpClients {
            strict: build(false)?,
            accept_invalid: build(true)?,
        })
    }

    fn build(&self, request: &HttpRequest) -> reqwest::RequestBuilder {
        let client = if request.accept_invalid_certs {
            &self.accept_invalid
        } else {
            &self.strict
        };
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        if let Some(ms) = request.timeout {
            builder = builder.timeout(Duration::from_millis(ms));
        }
        builder
    }

    /// Make a request and report its response to the session.
    pub(crate) fn spawn_request(
        &self,
        id: RequestId,
        request: &HttpRequest,
        inbound: mpsc::Sender<Inbound>,
    ) {
        let builder = self.build(request);
        tokio::spawn(async move {
            let result = async {
                let response = builder.send().await.map_err(describe)?;
                let status = response.status().as_u16();
                let body = response.bytes().await.map_err(describe)?.to_vec();
                Ok(HttpResponse { status, body })
            }
            .await;
            let _ = inbound.send(Inbound::Http { id, result }).await;
        });
    }

    /// Open a server-sent event stream and report its events to the session.
    pub(crate) fn spawn_stream(
        &self,
        stream: Key,
        generation: u64,
        request: &HttpRequest,
        inbound: mpsc::Sender<Inbound>,
    ) -> tokio::task::JoinHandle<()> {
        let builder = self.build(request);
        tokio::spawn(async move {
            let send = |input: SseInput| {
                let inbound = inbound.clone();
                async move {
                    inbound
                        .send(Inbound::Sse {
                            stream,
                            generation,
                            input,
                        })
                        .await
                        .is_ok()
                }
            };
            let response = match builder.send().await {
                Ok(r) => r,
                Err(e) => {
                    send(SseInput::Closed {
                        status: None,
                        reason: describe(e),
                    })
                    .await;
                    return;
                }
            };
            let status = response.status().as_u16();
            if status != 200 {
                send(SseInput::Closed {
                    status: Some(status),
                    reason: format!("HTTP {status}"),
                })
                .await;
                return;
            }
            if !send(SseInput::Opened).await {
                return;
            }
            let mut parser = SseParser::default();
            let mut body = response.bytes_stream();
            let reason = loop {
                match body.next().await {
                    Some(Ok(chunk)) => {
                        if !send(SseInput::Activity).await {
                            return;
                        }
                        for event in parser.feed(&chunk) {
                            if !send(SseInput::Event(event)).await {
                                return;
                            }
                        }
                    }
                    Some(Err(e)) => break describe(e),
                    None => break "stream ended".to_string(),
                }
            };
            send(SseInput::Closed {
                status: None,
                reason,
            })
            .await;
        })
    }
}

fn describe(e: reqwest::Error) -> String {
    use std::error::Error;
    let mut message = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        message.push_str(": ");
        message.push_str(&s.to_string());
        source = s.source();
    }
    message
}
