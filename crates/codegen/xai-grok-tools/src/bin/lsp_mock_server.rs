//! Stdio language server used by the LSP tests.
//!
//! argv[1] is the mock script path the tests already write. The file stem
//! selects the behavior. Two values are read from that file as data, not
//! executed: the initialize delay for `slow_init_lsp`, and the failure count
//! for `init_fail_lsp`. This process does not start an interpreter.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

const DELAYED_OPEN_WAIT: Duration = Duration::from_secs(1);
const SLOW_PULL_WAIT: Duration = Duration::from_millis(300);
const STALE_CLEAN_WAIT: Duration = Duration::from_millis(300);
const STILL_LOADING_WAIT: Duration = Duration::from_millis(100);
const LOADED_ANSWER_WAIT: Duration = Duration::from_millis(250);
const CHECK_PUSH_WAIT: Duration = Duration::from_millis(300);

fn send(msg: &Value) {
    let body = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = io::stdout().lock();
    let _ = out.write_all(header.as_bytes());
    let _ = out.write_all(body.as_bytes());
    let _ = out.flush();
}

fn read_message(input: &mut BufReader<io::Stdin>) -> Option<Value> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let n = input.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }
    let length = content_length?;
    if length == 0 {
        return None;
    }
    let mut buf = vec![0u8; length];
    input.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn result(id: &Value, value: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": value})
}

fn error_reply(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message}
    })
}

fn diagnostic(
    start_line: u64,
    start_char: u64,
    end_line: u64,
    end_char: u64,
    severity: u64,
    source: &str,
    message: &str,
) -> Value {
    json!({
        "range": {
            "start": {"line": start_line, "character": start_char},
            "end": {"line": end_line, "character": end_char}
        },
        "severity": severity,
        "source": source,
        "message": message
    })
}

fn one_diagnostic(message: &str) -> Value {
    diagnostic(0, 0, 0, 1, 1, "mock", message)
}

fn publish(uri: &str, items: Vec<Value>) {
    send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": items}
    }));
}

fn publish_at(uri: &str, items: Vec<Value>, version: i64) {
    send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": items, "version": version}
    }));
}

fn publish_message(uri: &str, message: &str) {
    publish(uri, vec![one_diagnostic(message)]);
}

fn publish_mock_pair(uri: &str) {
    publish(
        uri,
        vec![
            diagnostic(0, 5, 0, 10, 1, "mock", "mock error: undeclared variable"),
            diagnostic(2, 0, 2, 15, 2, "mock", "mock warning: unused import"),
        ],
    );
}

fn notify(method: &str, params: Value) {
    send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
}

fn ask(method: &str, params: Value, request_id: i64) {
    send(&json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params
    }));
}

fn pull_full(id: &Value, result_id: &str, items: Vec<Value>) -> Value {
    result(
        id,
        json!({"kind": "full", "resultId": result_id, "items": items}),
    )
}

fn pull_unchanged(id: &Value, result_id: &str) -> Value {
    result(id, json!({"kind": "unchanged", "resultId": result_id}))
}

fn location(uri: &str, line: u64, start: u64, end: u64) -> Value {
    json!({
        "uri": uri,
        "range": {
            "start": {"line": line, "character": start},
            "end": {"line": line, "character": end}
        }
    })
}

fn definition_at(id: &Value, uri: &str, line: u64, start: u64, end: u64) -> Value {
    result(id, json!([location(uri, line, start, end)]))
}

fn references(id: &Value, uri: &str) -> Value {
    result(
        id,
        json!([location(uri, 5, 0, 10), location(uri, 15, 3, 13)]),
    )
}

fn uri_of(msg: &Value) -> Option<&str> {
    msg.pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
}

fn doc_version(msg: &Value) -> i64 {
    msg.pointer("/params/textDocument/version")
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn previous_label(msg: &Value) -> String {
    match msg.pointer("/params/previousResultId") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => "None".to_string(),
    }
}

fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

fn save_has_text(msg: &Value) -> bool {
    !matches!(msg.pointer("/params/text"), None | Some(Value::Null))
}

fn changed_label(msg: &Value) -> String {
    let Some(range) = msg.pointer("/params/contentChanges/0/range") else {
        return "changed without range".to_string();
    };
    if range.is_null() {
        return "changed without range".to_string();
    }
    let number = |pointer: &str| range.pointer(pointer).and_then(Value::as_i64).unwrap_or(0);
    format!(
        "changed with range {}:{}-{}:{}",
        number("/start/line"),
        number("/start/character"),
        number("/end/line"),
        number("/end/character")
    )
}

fn beside_script(script: &Path, name: &str) -> PathBuf {
    script.parent().unwrap_or(Path::new(".")).join(name)
}

fn dump_json(path: &Path, value: &Value) {
    let Ok(text) = serde_json::to_string(value) else {
        return;
    };
    let _ = fs::write(path, text);
}

/// Marker sits next to the document the request named, not next to the script.
fn touch_beside_document(uri: &str, name: &str) {
    let Some(path) = uri.strip_prefix("file://") else {
        return;
    };
    let Some(dir) = Path::new(path).parent() else {
        return;
    };
    let _ = fs::write(dir.join(name), "");
}

fn assignment_u32(text: &str, name: &str) -> Option<u32> {
    let idx = text.find(name)?;
    let rest = text[idx + name.len()..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn sleep_secs_from_script(text: &str) -> f64 {
    let Some(idx) = text.find("time.sleep(") else {
        return 0.0;
    };
    let rest = &text[idx + "time.sleep(".len()..];
    let Some(end) = rest.find(')') else {
        return 0.0;
    };
    let expr = rest[..end].trim();
    if let Ok(value) = expr.parse::<f64>() {
        return value;
    }
    if let Some((lhs, rhs)) = expr.split_once('/') {
        if let (Ok(numer), Ok(denom)) = (lhs.trim().parse::<f64>(), rhs.trim().parse::<f64>()) {
            if denom != 0.0 {
                return numer / denom;
            }
        }
    }
    0.0
}

fn next_init_attempt() -> Option<u32> {
    let path = std::env::var("INIT_FAILURE_COUNTER_FILE").ok()?;
    let current = fs::read_to_string(&path)
        .ok()
        .and_then(|text| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Some(0)
            } else {
                trimmed.parse::<u32>().ok()
            }
        })
        .unwrap_or(0);
    let attempts = current.saturating_add(1);
    let _ = fs::write(&path, attempts.to_string());
    Some(attempts)
}

fn caps_pull(inter_file: bool) -> Value {
    json!({
        "textDocumentSync": {"openClose": true, "change": 2},
        "diagnosticProvider": {
            "interFileDependencies": inter_file,
            "workspaceDiagnostics": false
        }
    })
}

fn caps_for(stem: &str) -> Value {
    match stem {
        "mock_lsp" => json!({
            "textDocumentSync": 1,
            "definitionProvider": true,
            "referencesProvider": true
        }),
        "delayed_lsp" | "silent_lsp" | "init_fail_lsp" | "file_watch_lsp" => {
            json!({"textDocumentSync": 1})
        }
        "slow_init_lsp" => json!({
            "textDocumentSync": 1,
            "definitionProvider": true
        }),
        "incremental_lsp" => json!({"textDocumentSync": 2}),
        "pull_lsp"
        | "loads_late_lsp"
        | "diagnostic_refresh_lsp"
        | "push_and_pull_lsp"
        | "clean_then_silent_lsp"
        | "loads_after_quiet_lsp"
        | "slow_check_lsp" => caps_pull(true),
        "selective_pull_lsp"
        | "mid_analysis_pull_lsp"
        | "slow_pull_lsp"
        | "stale_clean_pull_lsp"
        | "partial_pull_lsp" => caps_pull(false),
        "save_text_lsp" => json!({
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": {"includeText": true}
            }
        }),
        "versioned_push_lsp"
        | "publishes_before_open_lsp"
        | "refresh_without_pull_lsp"
        | "pull_rejecting_lsp" => {
            json!({"textDocumentSync": {"openClose": true, "change": 1}})
        }
        _ => json!({"textDocumentSync": 1}),
    }
}

fn send_nuget_registration() {
    ask(
        "client/registerCapability",
        json!({
            "registrations": [{
                "id": "nuget-dlls",
                "method": "workspace/didChangeWatchedFiles",
                "registerOptions": {
                    "watchers": [
                        {
                            "globPattern": {
                                "baseUri": "file:///tmp/fake-nuget/packages",
                                "pattern": "**/*.dll"
                            }
                        },
                        {
                            "globPattern": "**/*.{ts,tsx,js}"
                        }
                    ]
                }
            }]
        }),
        100,
    );
}

struct Server {
    stem: String,
    script_path: PathBuf,
    failures_before_success: u32,
    init_delay_secs: f64,
    saves: u64,
    pulls: u64,
    revisions: u64,
    answered_refresh: bool,
}

impl Server {
    fn beside(&self, name: &str) -> PathBuf {
        beside_script(&self.script_path, name)
    }

    fn on_message(&mut self, msg: &Value) -> bool {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        match msg.get("method").and_then(Value::as_str) {
            Some("initialize") => self.on_initialize(msg, &id),
            Some("shutdown") => {
                send(&result(&id, Value::Null));
                true
            }
            Some("exit") => false,
            Some("initialized") => {
                self.on_initialized();
                true
            }
            Some(method) => {
                self.on_method(msg, method, &id);
                true
            }
            None => {
                self.on_response(msg, &id);
                true
            }
        }
    }

    fn on_initialize(&self, msg: &Value, id: &Value) -> bool {
        if self.stem == "init_fail_lsp" {
            return self.answer_init_attempt(id);
        }
        if self.stem == "file_watch_lsp" {
            if let Some(caps) = msg.pointer("/params/capabilities") {
                dump_json(&self.beside("initialize_caps.json"), caps);
            }
        }
        if self.stem == "slow_init_lsp" && self.init_delay_secs > 0.0 {
            thread::sleep(Duration::from_secs_f64(self.init_delay_secs));
        }
        send(&result(id, json!({"capabilities": caps_for(&self.stem)})));
        true
    }

    /// Fail the first N initialize attempts recorded in the counter file, then
    /// answer. N is the count written into the script. A failed attempt exits
    /// so the client can start the next one.
    fn answer_init_attempt(&self, id: &Value) -> bool {
        let attempts = next_init_attempt().unwrap_or(0);
        if attempts <= self.failures_before_success {
            let message = format!(
                "init failed on purpose after {} failures",
                self.failures_before_success
            );
            send(&error_reply(id, -32603, &message));
            return false;
        }
        send(&result(
            id,
            json!({"capabilities": caps_for("init_fail_lsp")}),
        ));
        true
    }

    fn on_initialized(&self) {
        match self.stem.as_str() {
            "publishes_before_open_lsp" => {
                if let Ok(uri) = std::env::var("PREOPENED_URI") {
                    publish_message(&uri, "reported before the file was opened");
                }
            }
            "file_watch_lsp" => send_nuget_registration(),
            _ => {}
        }
    }

    fn on_response(&mut self, msg: &Value, id: &Value) {
        if self.stem == "diagnostic_refresh_lsp" && id.as_i64() == Some(9001) {
            self.answered_refresh = true;
        }
        if self.stem == "file_watch_lsp" && msg.get("id").is_some() {
            dump_json(&self.beside("register_reply.json"), msg);
        }
    }

    fn on_method(&mut self, msg: &Value, method: &str, id: &Value) {
        match self.stem.as_str() {
            "mock_lsp" => on_mock(msg, method, id),
            "delayed_lsp" => on_delayed(msg, method),
            "slow_init_lsp" => on_slow_init(msg, method, id),
            "incremental_lsp" => on_incremental(msg, method),
            "pull_lsp" => self.on_pull(msg, method, id),
            "selective_pull_lsp" => self.on_selective_pull(msg, method, id),
            "mid_analysis_pull_lsp" => self.on_mid_analysis_pull(method, id),
            "slow_pull_lsp" => self.on_slow_pull(msg, method, id),
            "stale_clean_pull_lsp" => self.on_stale_clean_pull(msg, method, id),
            "partial_pull_lsp" => self.on_partial_pull(msg, method, id),
            "silent_lsp" | "init_fail_lsp" | "publishes_before_open_lsp" => {}
            "save_text_lsp" => on_save_text(msg, method),
            "loads_late_lsp" => self.on_loads_late(method, id),
            "diagnostic_refresh_lsp" => self.on_diagnostic_refresh(method, id),
            "versioned_push_lsp" => on_versioned_push(msg, method),
            "push_and_pull_lsp" => self.on_push_and_pull(msg, method, id),
            "refresh_without_pull_lsp" => on_refresh_without_pull(msg, method),
            "pull_rejecting_lsp" => self.on_pull_rejecting(method, id),
            "clean_then_silent_lsp" => self.on_clean_then_silent(method, id),
            "loads_after_quiet_lsp" => self.on_loads_after_quiet(method, id),
            "slow_check_lsp" => self.on_slow_check(msg, method, id),
            "file_watch_lsp" => self.on_file_watch(msg, method),
            _ => {
                if msg.get("id").is_some() {
                    send(&error_reply(id, -32601, "Method not found"));
                }
            }
        }
    }

    fn on_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        match method {
            "textDocument/didSave" => self.saves += 1,
            "textDocument/diagnostic" => {
                self.pulls += 1;
                let message = format!(
                    "pull #{} saves={} prev={}",
                    self.pulls,
                    self.saves,
                    previous_label(msg)
                );
                let result_id = format!("result-{}", self.pulls);
                send(&pull_full(id, &result_id, vec![one_diagnostic(&message)]));
            }
            _ => {}
        }
    }

    fn on_selective_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        let uri = uri_of(msg).unwrap_or("");
        let items = if uri.contains("broken") {
            vec![one_diagnostic("pulled problem")]
        } else {
            Vec::new()
        };
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(id, &result_id, items));
    }

    fn on_mid_analysis_pull(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        let items = if self.pulls == 2 {
            Vec::new()
        } else {
            vec![one_diagnostic(&format!("real problem {}", self.pulls))]
        };
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(id, &result_id, items));
    }

    fn on_slow_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        match method {
            "textDocument/didOpen" | "textDocument/didChange" => self.revisions += 1,
            "textDocument/diagnostic" => {
                self.pulls += 1;
                let asked = self.revisions;
                if self.pulls == 1 {
                    if let Some(uri) = uri_of(msg) {
                        touch_beside_document(uri, "first-pull-started");
                    }
                }
                thread::sleep(SLOW_PULL_WAIT);
                let message = format!("pull {} answers revision {}", self.pulls, asked);
                let result_id = format!("r-{}", self.pulls);
                send(&pull_full(id, &result_id, vec![one_diagnostic(&message)]));
            }
            _ => {}
        }
    }

    fn on_stale_clean_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        let previous = msg
            .pointer("/params/previousResultId")
            .and_then(Value::as_str);
        if self.pulls == 1 {
            send(&pull_full(id, "r1", vec![one_diagnostic("the problem")]));
        } else if self.pulls == 2 {
            if let Some(uri) = uri_of(msg) {
                touch_beside_document(uri, "second-pull-started");
            }
            thread::sleep(STALE_CLEAN_WAIT);
            send(&pull_full(id, "clean", Vec::new()));
        } else if previous == Some("clean") {
            send(&pull_unchanged(id, "clean"));
        } else {
            send(&pull_full(id, "clean", Vec::new()));
        }
    }

    fn on_partial_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        let uri = uri_of(msg).unwrap_or("");
        if !uri.contains("loud") {
            return;
        }
        self.pulls += 1;
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(
            id,
            &result_id,
            vec![one_diagnostic("loud problem")],
        ));
    }

    fn on_loads_late(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        if self.pulls == 1 {
            send(&pull_full(id, "r-1", Vec::new()));
            notify("workspace/projectInitializationComplete", Value::Null);
            return;
        }
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(
            id,
            &result_id,
            vec![one_diagnostic("found once the solution was loaded")],
        ));
    }

    fn on_diagnostic_refresh(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        if self.pulls == 1 {
            send(&pull_full(id, "r-1", Vec::new()));
            ask("workspace/diagnostic/refresh", Value::Null, 9001);
            return;
        }
        let message = format!("refresh answered={}", py_bool(self.answered_refresh));
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(id, &result_id, vec![one_diagnostic(&message)]));
    }

    fn on_push_and_pull(&mut self, msg: &Value, method: &str, id: &Value) {
        match method {
            "textDocument/didOpen" | "textDocument/didChange" => {
                let Some(uri) = uri_of(msg) else {
                    return;
                };
                let message = format!(
                    "the check that only the push channel runs, pulls={}",
                    self.pulls
                );
                publish_at(uri, vec![one_diagnostic(&message)], doc_version(msg));
            }
            "textDocument/diagnostic" => {
                self.pulls += 1;
                let result_id = format!("r-{}", self.pulls);
                send(&pull_full(id, &result_id, Vec::new()));
            }
            _ => {}
        }
    }

    /// Count file sits next to the script path argument. The tests read that path.
    fn on_pull_rejecting(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        let _ = fs::write(self.beside("pulls.txt"), self.pulls.to_string());
        send(&error_reply(id, -32601, "method not found"));
    }

    fn on_clean_then_silent(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        let items = if self.pulls == 1 {
            vec![one_diagnostic("the real problem")]
        } else if self.pulls <= 3 {
            Vec::new()
        } else {
            return;
        };
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(id, &result_id, items));
    }

    fn on_loads_after_quiet(&mut self, method: &str, id: &Value) {
        if method != "textDocument/diagnostic" {
            return;
        }
        self.pulls += 1;
        if self.pulls == 1 {
            send(&error_reply(id, -32603, "still loading"));
            thread::sleep(STILL_LOADING_WAIT);
            notify("workspace/projectInitializationComplete", Value::Null);
            return;
        }
        thread::sleep(LOADED_ANSWER_WAIT);
        let result_id = format!("r-{}", self.pulls);
        send(&pull_full(
            id,
            &result_id,
            vec![one_diagnostic("found once the solution was loaded")],
        ));
    }

    fn on_slow_check(&mut self, msg: &Value, method: &str, id: &Value) {
        match method {
            "textDocument/didOpen" | "textDocument/didChange" => {
                let Some(uri) = uri_of(msg) else {
                    return;
                };
                let uri = uri.to_string();
                let version = doc_version(msg);
                thread::spawn(move || {
                    thread::sleep(CHECK_PUSH_WAIT);
                    publish_at(
                        &uri,
                        vec![one_diagnostic("an error only the check finds")],
                        version,
                    );
                });
            }
            "textDocument/diagnostic" => {
                self.pulls += 1;
                let result_id = format!("r-{}", self.pulls);
                send(&pull_full(id, &result_id, Vec::new()));
            }
            _ => {}
        }
    }

    fn on_file_watch(&self, msg: &Value, method: &str) {
        if method == "workspace/didChangeWatchedFiles" {
            if let Some(params) = msg.get("params") {
                dump_json(&self.beside("watched.json"), params);
            }
        }
    }
}

fn on_mock(msg: &Value, method: &str, id: &Value) {
    match method {
        "textDocument/didOpen" | "textDocument/didChange" => {
            if let Some(uri) = uri_of(msg) {
                publish_mock_pair(uri);
            }
        }
        "textDocument/didSave" => {}
        "textDocument/definition" => {
            let uri = uri_of(msg).unwrap_or("");
            send(&definition_at(id, uri, 10, 0, 20));
        }
        "textDocument/references" => {
            let uri = uri_of(msg).unwrap_or("");
            send(&references(id, uri));
        }
        _ => {
            if msg.get("id").is_some() {
                send(&error_reply(id, -32601, "Method not found"));
            }
        }
    }
}

fn on_delayed(msg: &Value, method: &str) {
    if method != "textDocument/didOpen" {
        return;
    }
    let Some(uri) = uri_of(msg) else {
        return;
    };
    thread::sleep(DELAYED_OPEN_WAIT);
    publish(
        uri,
        vec![diagnostic(
            0,
            0,
            0,
            5,
            1,
            "delayed",
            "delayed diagnostic after restart",
        )],
    );
}

fn on_slow_init(msg: &Value, method: &str, id: &Value) {
    if method != "textDocument/definition" {
        return;
    }
    let uri = uri_of(msg).unwrap_or("");
    send(&definition_at(id, uri, 1, 0, 5));
}

fn on_incremental(msg: &Value, method: &str) {
    let Some(uri) = uri_of(msg) else {
        return;
    };
    match method {
        "textDocument/didOpen" => publish_message(uri, "opened"),
        "textDocument/didChange" => publish_message(uri, &changed_label(msg)),
        _ => {}
    }
}

fn on_save_text(msg: &Value, method: &str) {
    if method != "textDocument/didSave" {
        return;
    }
    let Some(uri) = uri_of(msg) else {
        return;
    };
    let message = format!("saved with text={}", py_bool(save_has_text(msg)));
    publish_message(uri, &message);
}

fn on_versioned_push(msg: &Value, method: &str) {
    let Some(uri) = uri_of(msg) else {
        return;
    };
    let version = doc_version(msg);
    match method {
        "textDocument/didOpen" => {
            let message = format!("verdict on version {version}");
            publish_at(uri, vec![one_diagnostic(&message)], version);
        }
        "textDocument/didChange" => {
            let stale = version - 1;
            let message = format!("stale verdict on version {stale}");
            publish_at(uri, vec![one_diagnostic(&message)], stale);
        }
        _ => {}
    }
}

fn on_refresh_without_pull(msg: &Value, method: &str) {
    if method != "textDocument/didOpen" && method != "textDocument/didChange" {
        return;
    }
    let Some(uri) = uri_of(msg) else {
        return;
    };
    publish_message(uri, "a real problem");
    ask("workspace/diagnostic/refresh", Value::Null, 9002);
}

fn main() -> ExitCode {
    let script = std::env::args().nth(1).unwrap_or_default();
    let script_path = PathBuf::from(&script);
    let stem = script_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("mock_lsp")
        .to_string();
    let script_text = if stem == "slow_init_lsp" || stem == "init_fail_lsp" {
        fs::read_to_string(&script_path).unwrap_or_default()
    } else {
        String::new()
    };
    let mut server = Server {
        failures_before_success: if stem == "init_fail_lsp" {
            assignment_u32(&script_text, "FAILURES_BEFORE_SUCCESS").unwrap_or(u32::MAX)
        } else {
            0
        },
        init_delay_secs: if stem == "slow_init_lsp" {
            sleep_secs_from_script(&script_text)
        } else {
            0.0
        },
        stem,
        script_path,
        saves: 0,
        pulls: 0,
        revisions: 0,
        answered_refresh: false,
    };

    let mut input = BufReader::new(io::stdin());
    while let Some(msg) = read_message(&mut input) {
        if !server.on_message(&msg) {
            break;
        }
    }
    ExitCode::SUCCESS
}
