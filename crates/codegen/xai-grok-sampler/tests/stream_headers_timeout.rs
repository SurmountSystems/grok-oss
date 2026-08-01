//! First-byte / response-headers timeout on streaming `execute`.
//!
//! Own integration binary so `GROK_STREAM_HEADERS_TIMEOUT_SECS` cannot poison
//! other sampler tests that share a process under nextest.

#[allow(dead_code)] // shared with other integration binaries; this test only needs test_config
mod support;

use std::sync::Once;
use std::time::{Duration, Instant};

use support::test_config;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use xai_grok_sampler::SamplingClient;
use xai_grok_sampling_types::{ChatCompletionRequest, ChatRequestMessage};

fn pin_short_headers_timeout() {
    static PIN: Once = Once::new();
    PIN.call_once(|| {
        // SAFETY: this binary owns the env; runs before any client build.
        unsafe {
            std::env::set_var("GROK_STREAM_HEADERS_TIMEOUT_SECS", "1");
            // Keep connect fast; hang is after accept, before headers.
            std::env::set_var("GROK_CONNECT_TIMEOUT_SECS", "5");
            std::env::remove_var("GROK_SAMPLER_SHARED_CLIENT");
        }
    });
}

/// Contract: TCP accept with no HTTP headers must fail within the headers
/// budget (retryable), not hang for minutes with frozen Retrying chrome.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_execute_times_out_waiting_for_headers() {
    pin_short_headers_timeout();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hang server");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.expect("accept");
        // Read the request bytes so the client finishes the write, then
        // never send HTTP response headers.
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await;
        tokio::time::sleep(Duration::from_secs(60)).await;
    });

    let base = format!("http://{addr}");
    let client = SamplingClient::new(test_config(&base, "token-headers-timeout")).unwrap();
    let request = ChatCompletionRequest {
        model: Some("test-model".into()),
        messages: vec![ChatRequestMessage::user("hi")],
        temperature: None,
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        user: None,
        tools: None,
        tool_choice: None,
        search_parameters: None,
        response_format: None,
        reasoning_effort: None,
        x_grok_conv_id: None,
        x_grok_req_id: None,
        x_grok_session_id: None,
        x_grok_turn_idx: None,
        x_grok_agent_id: None,
        x_grok_deployment_id: None,
        x_grok_user_id: None,
        trace: None,
    };

    let started = Instant::now();
    let result = client.chat_completion_stream(request).await;
    let elapsed = started.elapsed();
    let err = match result {
        Ok(_) => panic!("must time out waiting for headers, got Ok stream"),
        Err(e) => e,
    };

    assert!(
        elapsed < Duration::from_secs(8),
        "headers timeout should fire near 1s budget, got {elapsed:?}"
    );
    assert!(
        err.is_retryable(),
        "headers timeout must be retryable, got {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("timed out waiting for response headers") || msg.contains("headers"),
        "expected headers-timeout wording, got {msg}"
    );
}
