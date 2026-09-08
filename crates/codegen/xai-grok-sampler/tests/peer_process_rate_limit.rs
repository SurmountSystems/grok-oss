//! Process B must not fire sampling HTTP while process A's flock cooldown is live.
//!
//! Own integration binary so `$GROK_HOME` is pinned before
//! [`grok_rate_limit::SharedRateLimitStore::process_default`] latches.

#[allow(dead_code)]
mod support;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::response::sse::Sse;
use axum::routing::post;
use futures_util::stream;
use grok_rate_limit::{
    DISABLE_ENV, ProviderKey, RateLimitMeta, SharedRateLimitStore, fingerprint_secret,
};
use support::test_config;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use xai_grok_sampler::{RequestId, RetryPolicy, SamplerActor};
use xai_grok_sampling_types::{ContentPart, ConversationItem, ConversationRequest, UserItem};
use xai_grok_test_support::{EnvGuard, sse};

fn user_request(text: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ContentPart::Text {
                text: Arc::<str>::from(text),
            }],
            ..Default::default()
        })],
        ..Default::default()
    }
}

/// Contract: a peer grok-oss process must not sample (no HTTP) while another
/// process's HTTP 429 cooldown is live on disk under `$GROK_HOME/rate_limits/`.
/// Fingerprint the bearer; never put the raw token in the filename.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_process_does_not_sample_during_shared_rate_limit_cooldown() {
    let dir = tempfile::TempDir::new().expect("temp GROK_HOME");
    let _home = EnvGuard::set("GROK_HOME", dir.path());
    let _enable = EnvGuard::unset(DISABLE_ENV);

    let hits = Arc::new(AtomicU32::new(0));
    let hits_h = Arc::clone(&hits);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let hits_h = Arc::clone(&hits_h);
            async move {
                hits_h.fetch_add(1, Ordering::SeqCst);
                let events = sse::chat_completion_events("ok", "test-model");
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let api_key = "super-secret-session-token-value";
    let base_url = format!("http://{addr}/v1");
    let mut cfg = test_config(&base_url, api_key);
    cfg.max_retries = Some(0);

    // Process A: write the flock JSON (as after HTTP 429). Separate store
    // handle so process B's process_default cache stays empty and must read disk.
    let writer = SharedRateLimitStore::open(dir.path()).expect("open writer store");
    let key =
        ProviderKey::from_base_url_and_key_fingerprint(&base_url, &fingerprint_secret(api_key));
    writer
        .observe(
            &key,
            Duration::from_secs(3),
            RateLimitMeta {
                status: Some(429),
                reason: Some("from-process-a".into()),
            },
        )
        .expect("process A observe");
    assert!(
        writer.remaining(&key) > Duration::from_secs(1),
        "process A cooldown must still be live after observe"
    );

    let rate_dir = dir.path().join("rate_limits");
    let names: Vec<String> = fs::read_dir(&rate_dir)
        .expect("rate_limits dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with(".json")),
        "process A must write a json cooldown file, names={names:?}"
    );
    assert!(
        names
            .iter()
            .all(|n| !n.contains(api_key) && !n.contains("super-secret")),
        "raw bearer must not appear in rate_limits filenames: {names:?}"
    );

    let peer = SharedRateLimitStore::process_default();
    assert!(
        peer.remaining(&key) > Duration::from_secs(1),
        "process B must see process A's cooldown on disk before sampling"
    );

    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = SamplerActor::spawn(cfg, RetryPolicy::default(), event_tx);
    handle.submit(RequestId::from("peer-b"), user_request("hi"));

    let hold = Instant::now();
    while hold.elapsed() < Duration::from_millis(800) {
        assert_eq!(
            hits.load(Ordering::SeqCst),
            0,
            "process B must not fire HTTP while process A's shared rate-limit cooldown is live"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    peer.wait_if_limited(&key).await;
    let until = Instant::now() + Duration::from_secs(2);
    while hits.load(Ordering::SeqCst) == 0 && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        hits.load(Ordering::SeqCst) >= 1,
        "after the shared cooldown expires, process B may sample"
    );
}
