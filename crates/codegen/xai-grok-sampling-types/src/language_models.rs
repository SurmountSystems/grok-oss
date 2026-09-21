//! Official `GET /v1/language-models` JSON (id, fingerprint, version, created).
//!
//! Fingerprint is the xAI system configuration hosting the model. It is not a
//! SHA of the weights. A flip means the serving path changed. Behavior still
//! decides if weights moved.
//!
//! See [List language models](https://docs.x.ai/docs/api-reference#list-language-models)
//! (accessed: 2026-09-09).

use serde::{Deserialize, Serialize};

/// One language-model serving record from the official list endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LanguageModelServing {
    pub id: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Proto / SDK spelling. REST uses [`Self::fingerprint`].
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// Unix creation time from the official JSON. Not a dated slug.
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl LanguageModelServing {
    /// REST `fingerprint`, else proto `system_fingerprint`. Empty is absent.
    pub fn serving_fingerprint(&self) -> Option<&str> {
        nonempty(self.fingerprint.as_deref())
            .or_else(|| nonempty(self.system_fingerprint.as_deref()))
    }

    pub fn version_str(&self) -> Option<&str> {
        nonempty(self.version.as_deref())
    }

    pub fn matches_id_or_alias(&self, model: &str) -> bool {
        if model.is_empty() {
            return false;
        }
        self.id == model || self.aliases.iter().any(|a| a == model)
    }
}

/// Official list body: `{ "models": [ ... ] }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LanguageModelsList {
    #[serde(default)]
    pub models: Vec<LanguageModelServing>,
}

/// Parse official language-models JSON. Does not invent dated slugs.
pub fn parse_language_models_json(json: &str) -> Result<LanguageModelsList, serde_json::Error> {
    let raw: LanguageModelsList = serde_json::from_str(json)?;
    Ok(LanguageModelsList {
        models: raw
            .models
            .into_iter()
            .filter(|m| !m.id.is_empty())
            .collect(),
    })
}

/// Derive `GET .../language-models` from an OpenAI-compatible `.../models` URL.
pub fn language_models_url_from_models_list_url(models_url: &str) -> Option<String> {
    let trimmed = models_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.ends_with("/language-models") {
        return Some(trimmed.to_string());
    }
    trimmed
        .strip_suffix("/models")
        .map(|prefix| format!("{prefix}/language-models"))
}

fn nonempty(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFICIAL_SHAPE: &str = r#"{
  "models": [
    {
      "id": "grok-4.6",
      "fingerprint": "fp_84ff176447",
      "created": 1776556800,
      "object": "model",
      "owned_by": "xai",
      "version": "1.0",
      "aliases": ["grok-4.6-latest", "grok-latest"]
    },
    {
      "id": "latest",
      "system_fingerprint": "fp_777a9f8466",
      "created": 1776556800,
      "version": "1.0",
      "aliases": ["grok-4.6"]
    }
  ]
}"#;

    /// Named contract: parse language-models JSON for id, fingerprint, version,
    /// and created (Unix). Fingerprint is serving-path config, not a SHA of the
    /// weights.
    #[test]
    fn parse_language_models_json_reads_id_fingerprint_version_created() {
        let list = parse_language_models_json(OFFICIAL_SHAPE).expect("parse");
        assert_eq!(list.models.len(), 2, "got {:?}", list.models);
        let first = &list.models[0];
        assert_eq!(first.id, "grok-4.6");
        assert_eq!(first.serving_fingerprint(), Some("fp_84ff176447"));
        assert_eq!(first.version_str(), Some("1.0"));
        assert_eq!(first.created, Some(1_776_556_800));
        assert!(first.matches_id_or_alias("grok-4.6-latest"));
        let second = &list.models[1];
        assert_eq!(second.id, "latest");
        assert_eq!(second.serving_fingerprint(), Some("fp_777a9f8466"));
        assert!(second.matches_id_or_alias("grok-4.6"));
    }

    /// Named contract: do not invent dated slugs such as grok-4.6-20260812
    /// unless `/v1/language-models` lists them.
    #[test]
    fn parse_language_models_json_does_not_invent_dated_slugs() {
        let list = parse_language_models_json(OFFICIAL_SHAPE).expect("parse");
        let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
        let aliases: Vec<&str> = list
            .models
            .iter()
            .flat_map(|m| m.aliases.iter().map(String::as_str))
            .collect();
        assert!(!ids.iter().any(|id| id.contains("20260812")), "{ids:?}");
        assert!(
            !aliases.iter().any(|a| a.contains("20260812")),
            "{aliases:?}"
        );
        assert!(
            !ids.contains(&"grok-4.6-20260812") && !aliases.contains(&"grok-4.6-20260812"),
            "parser must not invent dated slugs; ids={ids:?} aliases={aliases:?}"
        );
    }

    #[test]
    fn language_models_url_from_models_list_url_rewrites_v1_models() {
        assert_eq!(
            language_models_url_from_models_list_url("https://api.x.ai/v1/models"),
            Some("https://api.x.ai/v1/language-models".into())
        );
        assert_eq!(
            language_models_url_from_models_list_url("https://api.x.ai/v1/language-models"),
            Some("https://api.x.ai/v1/language-models".into())
        );
        assert_eq!(
            language_models_url_from_models_list_url("https://example.invalid/other"),
            None
        );
    }

    #[test]
    fn parse_skips_models_with_empty_id() {
        let list = parse_language_models_json(r#"{"models":[{"id":""},{"id":"grok-4.6"}]}"#)
            .expect("parse");
        assert_eq!(list.models.len(), 1);
        assert_eq!(list.models[0].id, "grok-4.6");
    }
}
