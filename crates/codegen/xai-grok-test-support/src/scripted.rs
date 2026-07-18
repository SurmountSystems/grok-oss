//! Data-driven scripted responses: status/header/body triples the mock inference server queues per path and the loopback mocks serve on request, rendered to HTTP at serve time.
//! Pure data: no router or handler types are public.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
<<<<<<< HEAD
use std::sync::Arc;
=======
>>>>>>> e3fdf3ed (Merge 2 (#4))

use axum::Json;
use axum::body::{Body, Bytes};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::sse::{KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream;
use serde_json::Value;

pub(crate) type BoxWait = Pin<Box<dyn Future<Output = ()> + Send>>;
pub(crate) type TerminalWait = Box<dyn FnOnce() -> BoxWait + Send>;

<<<<<<< HEAD
/// An SSE comment the hang body flushes so the response head reaches the client, then the stream
/// produces no chunk; a comment carries no event, so the client's idle timer runs from here.
const HANG_OPENING_FRAME: &[u8] = b": grok-mock stream open\n\n";

/// SSE `event:` name that is not written on the wire. The mock waits on the server's reply hold
/// after it, then continues with the next event.
pub const SSE_HOLD_EVENT: &str = "grok-mock-hold";

=======
>>>>>>> e3fdf3ed (Merge 2 (#4))
/// One SSE event as data: optional `event:` name plus the `data:` payload.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

impl SseEvent {
    pub fn data(data: impl Into<String>) -> Self {
        Self {
            event: None,
            data: data.into(),
        }
    }

    pub fn with_event(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event: Some(event.into()),
            data: data.into(),
        }
    }

    /// A pause in the event list. The mock emits nothing for it and waits on the server's reply hold.
    pub fn hold() -> Self {
        Self {
            event: Some(SSE_HOLD_EVENT.to_owned()),
            data: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScriptedBody {
    Json(Value),
    Sse(Vec<SseEvent>),
    /// Raw body bytes served verbatim, for byte-exact payloads such as malformed SSE.
    Raw(String),
    /// The connection closes before the response head reaches the client: the body stream fails on
    /// its first poll, so hyper tears the connection down without flushing the head it queued.
    Dropped,
    /// The head reaches the client, then the body stalls forever with no chunk, so the client's
    /// inference idle timeout fires.
    Hang,
}

<<<<<<< HEAD
/// Waits until the server's one reply hold is released. A hold that was never armed returns at once.
#[derive(Clone)]
pub(crate) struct BodyHold(Arc<dyn Fn() -> BoxWait + Send + Sync>);

impl std::fmt::Debug for BodyHold {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BodyHold")
    }
}

impl BodyHold {
    pub(crate) fn new(wait: impl Fn() -> BoxWait + Send + Sync + 'static) -> Self {
        Self(Arc::new(wait))
    }

    fn wait(&self) -> BoxWait {
        (self.0)()
    }
}

/// A scripted reply; see `inference_override` for where it sits in the tier order.
=======
/// A scripted reply served by a matched expectation or compatibility FIFO.
/// Scripted replies take precedence over required auth and fallback modes.
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[derive(Debug, Clone)]
pub struct ScriptedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: ScriptedBody,
    body_hold: Option<BodyHold>,
}

impl ScriptedResponse {
    /// 200 SSE response built from an event list.
    pub fn sse(events: Vec<SseEvent>) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: ScriptedBody::Sse(events),
            body_hold: None,
        }
    }

    /// [`SseEvent::hold`] waits on the server's reply hold.
    pub(crate) fn with_body_hold(mut self, hold: BodyHold) -> Self {
        self.body_hold = Some(hold);
        self
    }

    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: ScriptedBody::Json(body),
            body_hold: None,
        }
    }

    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: ScriptedBody::Raw(body.into()),
            body_hold: None,
        }
    }

<<<<<<< HEAD
    pub fn dropped() -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: ScriptedBody::Dropped,
            body_hold: None,
        }
    }

    /// A 200 stream whose head reaches the client, then never yields a chunk, so the client's
    /// inference idle timeout fires.
    pub fn hang() -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/event-stream".to_owned())],
            body: ScriptedBody::Hang,
            body_hold: None,
        }
    }

=======
>>>>>>> e3fdf3ed (Merge 2 (#4))
    pub(crate) fn is_sse(&self) -> bool {
        matches!(self.body, ScriptedBody::Sse(_))
    }

<<<<<<< HEAD
    /// Validate status and headers eagerly so a bad script panics at the enqueue call site rather than far away at serve time.
=======
    /// Validate status and headers eagerly so a bad script panics at the
    /// enqueue call site rather than far away at serve time.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    pub(crate) fn validate(&self) {
        StatusCode::from_u16(self.status).expect("invalid scripted status code");
        for (name, value) in &self.headers {
            HeaderName::from_bytes(name.as_bytes()).expect("invalid scripted header name");
            HeaderValue::from_str(value).expect("invalid scripted header value");
        }
    }

<<<<<<< HEAD
    /// Render to HTTP with SSE events paced by `delay` and `before_terminal` awaited before the last event.
    /// Non-SSE bodies await `before_terminal` before returning, so every body mode waits the same way.
=======
    /// Render to HTTP with SSE events paced by `delay` and optional terminal
    /// completion gating. Non-SSE bodies wait before returning so every body
    /// mode obeys the same release barrier.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    pub(crate) async fn into_response_paced(
        self,
        delay: Option<std::time::Duration>,
        before_terminal: Option<TerminalWait>,
    ) -> Response {
<<<<<<< HEAD
        let body_hold = self.body_hold;
        let mut response = match self.body {
            ScriptedBody::Json(body_json) => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                Json(body_json).into_response()
            }
            ScriptedBody::Raw(raw_body) => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                raw_body.into_response()
            }
            ScriptedBody::Dropped => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                Response::new(Body::from_stream(stream::once(async {
                    Err::<Bytes, _>(std::io::Error::other("the mock dropped the connection"))
                })))
            }
            ScriptedBody::Hang => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                let body = stream::unfold(false, |flushed| async move {
                    if flushed {
                        std::future::pending::<()>().await;
                        None
                    } else {
                        Some((
                            Ok::<Bytes, std::io::Error>(Bytes::from_static(HANG_OPENING_FRAME)),
                            true,
                        ))
                    }
                });
                Response::new(Body::from_stream(body))
            }
            ScriptedBody::Sse(events) => {
                let last_index = events.len().checked_sub(1);
=======
        let mut resp = match self.body {
            ScriptedBody::Json(v) => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                Json(v).into_response()
            }
            ScriptedBody::Raw(s) => {
                if let Some(wait) = before_terminal {
                    wait().await;
                }
                s.into_response()
            }
            ScriptedBody::Sse(events) => {
                let last_idx = events.len().checked_sub(1);
>>>>>>> e3fdf3ed (Merge 2 (#4))
                let mut events: Vec<_> = events.into_iter().enumerate().map(Some).collect();
                if events.is_empty() && before_terminal.is_some() {
                    events.push(None);
                }
                let stream = stream::unfold(
<<<<<<< HEAD
                    (events.into_iter(), before_terminal, body_hold),
                    move |(mut events, mut before_terminal, body_hold)| async move {
                        loop {
                            let item = events.next()?;
                            let Some((index, scripted_event)) = item else {
=======
                    (events.into_iter(), before_terminal),
                    move |(mut events, mut before_terminal)| async move {
                        loop {
                            let item = events.next()?;
                            let Some((idx, scripted_event)) = item else {
>>>>>>> e3fdf3ed (Merge 2 (#4))
                                if let Some(wait) = before_terminal.take() {
                                    wait().await;
                                }
                                continue;
                            };
<<<<<<< HEAD
                            if scripted_event.event.as_deref() == Some(SSE_HOLD_EVENT) {
                                if let Some(hold) = &body_hold {
                                    hold.wait().await;
                                }
                                continue;
                            }
                            if let Some(delay) = delay {
                                tokio::time::sleep(delay).await;
                            }
                            if Some(index) == last_index
=======
                            if let Some(d) = delay {
                                tokio::time::sleep(d).await;
                            }
                            if Some(idx) == last_idx
>>>>>>> e3fdf3ed (Merge 2 (#4))
                                && let Some(wait) = before_terminal.take()
                            {
                                wait().await;
                            }
                            let event =
                                axum::response::sse::Event::default().data(scripted_event.data);
                            let event = match scripted_event.event {
                                Some(name) => event.event(name),
                                None => event,
                            };
<<<<<<< HEAD
                            return Some((
                                Ok::<_, Infallible>(event),
                                (events, before_terminal, body_hold),
                            ));
=======
                            return Some((Ok::<_, Infallible>(event), (events, before_terminal)));
>>>>>>> e3fdf3ed (Merge 2 (#4))
                        }
                    },
                );
                Sse::new(stream)
                    .keep_alive(KeepAlive::default())
                    .into_response()
            }
        };
        *response.status_mut() =
            StatusCode::from_u16(self.status).expect("valid scripted status code");
        for (name, value) in self.headers {
            response.headers_mut().insert(
                HeaderName::from_bytes(name.as_bytes()).expect("valid scripted header name"),
                HeaderValue::from_str(&value).expect("valid scripted header value"),
            );
        }
        response
    }
}
