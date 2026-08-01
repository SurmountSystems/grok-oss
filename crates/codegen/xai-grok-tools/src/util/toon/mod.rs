//! JSON ↔ TOON helpers for model-facing structured tool results (UDAX AX).
//!
//! **Use JSON programmatically; encode as TOON for LLM input.**
//!
//! - Spec pin: [Token-Oriented Object Notation](https://github.com/toon-format/toon)
//!   (working draft SPEC; Rust port `toon-format` 0.5.x, default-features off).
//! - Runtime path never shells out to node/python CLI.
//! - Policy env: [`ENV_TOOL_RESULT_FORMAT`] = `toon` | `json` | `auto` (default **auto**).
//!
//! Integration: structured [`serde_json::Value`] results (e.g. `ToolOutput::Dynamic`)
//! go through [`maybe_encode_for_llm`] when rendered to model prompt text.
//! Structured JSON **text** blobs (MCP results, subagent handoff output,
//! large agent-facing context dumps) go through [`densify_structured_text`].
//! Free text and ACP/MCP protocol framing are unchanged.

use serde_json::Value;

/// Env: model-facing structured tool result format.
///
/// - `toon` — always TOON (fallback to compact JSON on encode error)
/// - `json` — always compact JSON
/// - `auto` (default when unset/empty/unknown) — TOON when tabular-eligible
///   or TOON is shorter than compact JSON; else compact JSON
pub const ENV_TOOL_RESULT_FORMAT: &str = "GROK_TOOL_RESULT_FORMAT";

/// Policy for [`maybe_encode_for_llm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolResultFormat {
    /// Prefer TOON when tabular-eligible or byte-smaller than compact JSON.
    #[default]
    Auto,
    /// Always emit TOON (compact JSON only if encode fails).
    Toon,
    /// Always emit compact JSON.
    Json,
}

impl ToolResultFormat {
    /// Parse a policy token (case-insensitive). Unknown / empty → [`Self::Auto`].
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "toon" => Self::Toon,
            "json" => Self::Json,
            "auto" | "" => Self::Auto,
            // Unknown values: safe default (auto), same as unset.
            _ => Self::Auto,
        }
    }
}

/// Resolve policy from [`ENV_TOOL_RESULT_FORMAT`]. Unset → [`ToolResultFormat::Auto`].
pub fn tool_result_format_from_env() -> ToolResultFormat {
    match std::env::var(ENV_TOOL_RESULT_FORMAT) {
        Ok(v) => ToolResultFormat::parse(&v),
        Err(_) => ToolResultFormat::Auto,
    }
}

/// Encode `value` as TOON text.
pub fn encode(value: &Value) -> Result<String, toon_format::ToonError> {
    toon_format::encode_default(value)
}

/// Decode TOON text to a JSON [`Value`].
pub fn decode(input: &str) -> Result<Value, toon_format::ToonError> {
    toon_format::decode_default(input)
}

/// Compact JSON (no pretty spaces). Fail-open to empty string on serialize error.
pub fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// True when `value` is (or contains) a sweet-spot TOON shape: uniform object
/// arrays (tabular form) or non-empty primitive arrays.
///
/// Shallow scan: the value itself, or any immediate object-field array.
/// Deep nested-only tables still may win on byte length under `auto`.
pub fn is_tabular_eligible(value: &Value) -> bool {
    match value {
        Value::Array(items) => array_is_toon_friendly(items),
        Value::Object(map) => map.values().any(|v| match v {
            Value::Array(items) => array_is_toon_friendly(items),
            other => is_tabular_eligible(other),
        }),
        _ => false,
    }
}

fn array_is_toon_friendly(items: &[Value]) -> bool {
    if items.is_empty() {
        return false;
    }
    // Non-empty primitive array → inline form.
    if items.iter().all(is_json_primitive) {
        return true;
    }
    // Uniform non-empty objects with the same key set → tabular form.
    let Some(Value::Object(first)) = items.first() else {
        return false;
    };
    if first.is_empty() {
        return false;
    }
    let keys: Vec<&String> = first.keys().collect();
    items.iter().all(|item| match item {
        Value::Object(obj) => {
            !obj.is_empty()
                && obj.len() == keys.len()
                && keys.iter().all(|k| obj.contains_key(*k))
                && obj.values().all(is_json_primitive)
        }
        _ => false,
    })
}

fn is_json_primitive(v: &Value) -> bool {
    matches!(
        v,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

/// Encode `value` for LLM context under `policy`.
///
/// Never panics: encode errors fall back to compact JSON.
pub fn maybe_encode_for_llm(value: &Value, policy: ToolResultFormat) -> String {
    let json = compact_json(value);
    match policy {
        ToolResultFormat::Json => json,
        ToolResultFormat::Toon => match encode(value) {
            Ok(toon) => toon,
            Err(_) => json,
        },
        ToolResultFormat::Auto => match encode(value) {
            Ok(toon) if is_tabular_eligible(value) || toon.len() < json.len() => toon,
            Ok(_) | Err(_) => json,
        },
    }
}

/// Convenience: env policy + [`maybe_encode_for_llm`].
pub fn maybe_encode_for_llm_from_env(value: &Value) -> String {
    maybe_encode_for_llm(value, tool_result_format_from_env())
}

/// Parse object/array JSON from `text` (trimmed). Scalars / invalid / free text → `None`.
fn parse_structured_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !matches!(trimmed.as_bytes().first(), Some(b'{' | b'[')) {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

/// Fail-open T6 savings line. Never panics; logging errors are ignored.
fn log_densify_savings(before_bytes: usize, after_bytes: usize) {
    if before_bytes == after_bytes {
        return;
    }
    // debug: visible under RUST_LOG=debug; never fails the turn.
    tracing::debug!(
        before_bytes,
        after_bytes,
        saved_bytes = before_bytes.saturating_sub(after_bytes),
        "toon densify: N_json → N_toon"
    );
}

/// If `text` is structured JSON (object or array), re-encode for the model under
/// the same TOON policy as [`maybe_encode_for_llm_from_env`].
///
/// Free text, bare scalars, and invalid JSON are left unchanged. Encode/parse
/// errors fail open (original text kept). Call this on **model-facing** text
/// only — not on ACP/MCP protocol envelopes or on-disk persistence JSON that
/// must round-trip as JSON.
///
/// No-op without allocate when the body is not structured JSON.
pub fn densify_structured_text_in_place(text: &mut String) {
    let Some(value) = parse_structured_json(text) else {
        return;
    };
    let before = text.len();
    let densified = maybe_encode_for_llm_from_env(&value);
    log_densify_savings(before, densified.len());
    *text = densified;
}

/// Convenience: densify a borrowed slice (allocates; may return the input
/// unchanged when free text / non-object-array).
pub fn densify_structured_text(text: &str) -> String {
    let mut owned = text.to_owned();
    densify_structured_text_in_place(&mut owned);
    owned
}

/// Test helpers for process-global format env mutation.
#[cfg(test)]
pub mod test_env {
    use std::sync::Mutex;

    /// Serialize env-mutating TOON policy tests across the crate.
    pub static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Sets env pairs; removes those keys on drop (panic-safe cleanup).
    pub struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        pub fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let keys: Vec<_> = pairs.iter().map(|(k, _)| *k).collect();
            for (k, v) in pairs {
                match v {
                    Some(val) => unsafe { std::env::set_var(k, val) },
                    None => unsafe { std::env::remove_var(k) },
                }
            }
            Self { keys }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for k in &self.keys {
                unsafe { std::env::remove_var(k) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::{ENV_LOCK, EnvGuard};
    use super::*;
    use serde_json::json;

    #[test]
    fn encode_object_fields() {
        let v = json!({"name": "Ada", "age": 36});
        let toon = encode(&v).unwrap();
        assert!(toon.contains("name: Ada"), "got: {toon}");
        assert!(toon.contains("age: 36"), "got: {toon}");
        let back = decode(&toon).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn encode_primitive_array_inline() {
        let v = json!({"tags": ["a", "b", "c"]});
        let toon = encode(&v).unwrap();
        assert!(
            toon.contains("tags[3]:") || toon.contains("tags[3]"),
            "expected inline array header, got: {toon}"
        );
        assert!(toon.contains("a"), "got: {toon}");
        let back = decode(&toon).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn encode_uniform_object_array_tabular() {
        let v = json!({
            "users": [
                {"id": 1, "name": "Alice", "role": "admin"},
                {"id": 2, "name": "Bob", "role": "user"}
            ]
        });
        let toon = encode(&v).unwrap();
        // Tabular: field list once, then rows.
        assert!(
            toon.contains("users[2]{") && toon.contains("id") && toon.contains("name"),
            "expected tabular header, got: {toon}"
        );
        assert!(
            toon.contains("Alice") && toon.contains("Bob"),
            "got: {toon}"
        );
        // No repeated key names per row (keys only in header).
        assert!(
            !toon.contains("\"id\""),
            "tabular should not quote-repeat JSON keys: {toon}"
        );
        let back = decode(&toon).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn encode_nested_object() {
        let v = json!({
            "user": {
                "name": "Ada",
                "address": {"city": "London", "zip": "E1"}
            }
        });
        let toon = encode(&v).unwrap();
        let back = decode(&toon).unwrap();
        assert_eq!(back, v);
        assert!(toon.contains("user:"), "got: {toon}");
        assert!(
            toon.contains("city:") || toon.contains("London"),
            "got: {toon}"
        );
    }

    #[test]
    fn encode_primitives_and_null() {
        for v in [
            json!(null),
            json!(true),
            json!(false),
            json!(42),
            json!("hello"),
            json!([]),
            json!({}),
        ] {
            let toon = encode(&v).unwrap();
            let back = decode(&toon).unwrap();
            assert_eq!(back, v, "round-trip failed for {v:?} → {toon}");
        }
    }

    #[test]
    fn tabular_eligible_detects_uniform_objects() {
        let tabular = json!([
            {"id": 1, "name": "a"},
            {"id": 2, "name": "b"}
        ]);
        assert!(is_tabular_eligible(&tabular));
        assert!(is_tabular_eligible(&json!({"rows": tabular})));

        let primitive = json!({"nums": [1, 2, 3]});
        assert!(is_tabular_eligible(&primitive));

        let mixed = json!([{"a": 1}, {"b": 2}]);
        assert!(!is_tabular_eligible(&mixed));

        let nested_only = json!({"x": 1, "y": "z"});
        assert!(!is_tabular_eligible(&nested_only));
    }

    #[test]
    fn maybe_encode_auto_prefers_toon_for_tabular() {
        let v = json!({
            "hits": (0..20)
                .map(|i| json!({"path": format!("f{i}.rs"), "line": i, "text": "match"}))
                .collect::<Vec<_>>()
        });
        let out = maybe_encode_for_llm(&v, ToolResultFormat::Auto);
        assert!(
            out.contains("hits[") && out.contains("{"),
            "auto should emit tabular TOON for uniform array, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "should not be pretty/compact JSON object start"
        );
        let json = compact_json(&v);
        assert!(
            out.len() < json.len(),
            "TOON should be denser: toon={} json={}",
            out.len(),
            json.len()
        );
    }

    #[test]
    fn maybe_encode_json_policy_is_compact_json() {
        let v = json!({
            "users": [
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"}
            ]
        });
        let out = maybe_encode_for_llm(&v, ToolResultFormat::Json);
        assert_eq!(out, compact_json(&v));
        assert!(out.starts_with('{'));
        assert!(!out.contains('\n'), "compact JSON is single-line: {out}");
    }

    #[test]
    fn maybe_encode_toon_policy_always_toon() {
        let v = json!({"name": "x", "n": 1});
        let out = maybe_encode_for_llm(&v, ToolResultFormat::Toon);
        assert!(out.contains("name:"), "got: {out}");
        assert!(!out.starts_with('{'), "got: {out}");
    }

    #[test]
    fn policy_parse_tokens() {
        assert_eq!(ToolResultFormat::parse("toon"), ToolResultFormat::Toon);
        assert_eq!(ToolResultFormat::parse("JSON"), ToolResultFormat::Json);
        assert_eq!(ToolResultFormat::parse("auto"), ToolResultFormat::Auto);
        assert_eq!(ToolResultFormat::parse(""), ToolResultFormat::Auto);
        assert_eq!(ToolResultFormat::parse("wat"), ToolResultFormat::Auto);
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn env_policy_default_auto() {
        let _lock = lock_env();
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, None)]);
        assert_eq!(tool_result_format_from_env(), ToolResultFormat::Auto);
    }

    #[test]
    fn env_policy_toon_and_json() {
        let _lock = lock_env();
        {
            let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("toon"))]);
            assert_eq!(tool_result_format_from_env(), ToolResultFormat::Toon);
        }
        {
            let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("json"))]);
            assert_eq!(tool_result_format_from_env(), ToolResultFormat::Json);
        }
    }

    // ── T5: densify structured text (handoff / prompt blobs) ──

    #[test]
    fn densify_structured_free_text_unchanged() {
        let plain = "hello world\nnot json at all";
        assert_eq!(densify_structured_text(plain), plain);

        let scalar = "12345";
        assert_eq!(
            densify_structured_text(scalar),
            scalar,
            "bare scalar is not object/array"
        );

        let mut owned = plain.to_owned();
        let ptr_before = owned.as_ptr();
        densify_structured_text_in_place(&mut owned);
        assert_eq!(owned, plain);
        assert_eq!(
            owned.as_ptr(),
            ptr_before,
            "free text densify-in-place must not reallocate"
        );
    }

    #[test]
    fn densify_structured_invalid_json_unchanged() {
        let junk = "{not valid json";
        assert_eq!(densify_structured_text(junk), junk);
    }

    #[test]
    fn densify_structured_tabular_auto_emits_toon() {
        let _lock = lock_env();
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, None)]); // auto

        let v = json!({
            "hits": [
                {"path": "a.rs", "line": 1, "text": "fn main"},
                {"path": "b.rs", "line": 2, "text": "fn other"},
                {"path": "c.rs", "line": 3, "text": "fn third"},
            ]
        });
        let pretty = serde_json::to_string_pretty(&v).unwrap();
        let out = densify_structured_text(&pretty);
        assert!(
            out.contains("hits[") && out.contains('{'),
            "auto should emit tabular TOON for uniform array, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "must not remain JSON object: {out}"
        );
    }

    #[test]
    fn densify_structured_json_policy_is_compact_json() {
        let _lock = lock_env();
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("json"))]);

        let v = json!({
            "users": [
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"}
            ]
        });
        let pretty = serde_json::to_string_pretty(&v).unwrap();
        let out = densify_structured_text(&pretty);
        assert_eq!(out, compact_json(&v));
        assert!(out.starts_with('{'));
        assert!(!out.contains('\n'));
    }

    #[test]
    fn densify_structured_fail_open_keeps_text_on_non_json() {
        // Mixed free text that starts with '{' but is not valid JSON → unchanged.
        let almost = "{ open brace but not json\nmore free text";
        assert_eq!(densify_structured_text(almost), almost);
    }

    #[test]
    fn log_densify_savings_is_fail_open() {
        // Must not panic regardless of sizes (T6).
        log_densify_savings(100, 40);
        log_densify_savings(40, 40);
        log_densify_savings(0, 0);
        log_densify_savings(10, 50); // growth still ok (e.g. policy forced TOON)
    }
}
