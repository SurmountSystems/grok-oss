//! Prompt-task drafts, templates, and prompt-as-task rows in `grok_oss.db`.
//!
//! Additive Surmount tables (schema v2). Not session todos, not composer
//! unsent drafts, not Token Economy ledger rows.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;

use super::GrokOssStore;

/// Stored incomplete prompt text (durable drafts, not the live composer file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTaskDraft {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Reusable prompt text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    pub id: String,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// One queued or executed prompt as a task. Primary key is a minted ULID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTask {
    pub id: String,
    pub body: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub draft_id: Option<String>,
    pub template_id: Option<String>,
}

/// Typed prefix must reach this many Unicode characters before a stored
/// draft or template is suggested. Shorter prefixes stay silent.
pub const STORED_PROMPT_SUGGEST_MIN_CHARS: usize = 3;

/// Whether the stored match is a durable draft or a reusable template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredPromptKind {
    Draft,
    Template,
}

/// Composer suggestion taken from `prompt_task_drafts` or `prompt_templates`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPromptSuggestion {
    pub full_text: String,
    pub kind: StoredPromptKind,
}

impl StoredPromptSuggestion {
    /// Remainder after `typed` when it is a proper prefix of the stored text.
    pub fn ghost_remainder(&self, typed: &str) -> Option<&str> {
        let rest = self.full_text.strip_prefix(typed)?;
        if rest.is_empty() { None } else { Some(rest) }
    }
}

/// Pick the most complete stored draft or template whose text starts with
/// `prefix`. Novel prefixes, short prefixes, exact already-typed matches,
/// and multiline stored text return `None` (ghost text is a single line).
pub fn suggest_most_complete_stored_prompt<D, T, DS, TS>(
    prefix: &str,
    drafts: D,
    templates: T,
) -> Option<StoredPromptSuggestion>
where
    D: IntoIterator<Item = DS>,
    T: IntoIterator<Item = TS>,
    DS: AsRef<str>,
    TS: AsRef<str>,
{
    if prefix.chars().count() < STORED_PROMPT_SUGGEST_MIN_CHARS {
        return None;
    }

    let mut best: Option<StoredPromptSuggestion> = None;
    let mut consider = |text: &str, kind: StoredPromptKind| {
        if text.contains('\n') || !text.starts_with(prefix) || text.len() <= prefix.len() {
            return;
        }
        let chars = text.chars().count();
        let take = match &best {
            None => true,
            Some(cur) => {
                let cur_chars = cur.full_text.chars().count();
                chars > cur_chars
                    || (chars == cur_chars
                        && kind == StoredPromptKind::Draft
                        && cur.kind == StoredPromptKind::Template)
            }
        };
        if take {
            best = Some(StoredPromptSuggestion {
                full_text: text.to_owned(),
                kind,
            });
        }
    };

    for draft in drafts {
        consider(draft.as_ref(), StoredPromptKind::Draft);
    }
    for template in templates {
        consider(template.as_ref(), StoredPromptKind::Template);
    }
    best
}

/// Accept completes the composer to the stored value when `typed` is a
/// proper prefix of that stored text.
pub fn accept_stored_prompt_suggestion(
    typed: &str,
    suggestion: &StoredPromptSuggestion,
) -> Option<String> {
    suggestion.ghost_remainder(typed)?;
    Some(suggestion.full_text.clone())
}

impl GrokOssStore {
    /// Insert a durable draft. Mints a 26-character Crockford ULID.
    pub fn insert_prompt_task_draft(&self, text: &str) -> Result<PromptTaskDraft> {
        let id = xai_grok_tools::util::ulid::mint();
        let now = Utc::now().to_rfc3339();
        self.connection()
            .execute(
                "INSERT INTO prompt_task_drafts (id, text, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, text, now, now],
            )
            .context("insert prompt_task_drafts")?;
        Ok(PromptTaskDraft {
            id,
            text: text.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Load a draft by ULID.
    pub fn load_prompt_task_draft(&self, id: &str) -> Result<Option<PromptTaskDraft>> {
        self.connection()
            .query_row(
                "SELECT id, text, created_at, updated_at FROM prompt_task_drafts WHERE id = ?1",
                [id],
                |row| {
                    Ok(PromptTaskDraft {
                        id: row.get(0)?,
                        text: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .context("load prompt_task_drafts")
    }

    /// Insert a reusable template. Mints a ULID.
    pub fn insert_prompt_template(&self, title: &str, body: &str) -> Result<PromptTemplate> {
        let id = xai_grok_tools::util::ulid::mint();
        let now = Utc::now().to_rfc3339();
        self.connection()
            .execute(
                "INSERT INTO prompt_templates (id, title, body, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![id, title, body, now, now],
            )
            .context("insert prompt_templates")?;
        Ok(PromptTemplate {
            id,
            title: title.to_owned(),
            body: body.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Load a template by ULID.
    pub fn load_prompt_template(&self, id: &str) -> Result<Option<PromptTemplate>> {
        self.connection()
            .query_row(
                "SELECT id, title, body, created_at, updated_at FROM prompt_templates WHERE id = ?1",
                [id],
                |row| {
                    Ok(PromptTemplate {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        body: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .context("load prompt_templates")
    }

    /// Insert a prompt as a task. Mints a ULID. Optional draft/template links.
    pub fn insert_prompt_task(
        &self,
        body: &str,
        status: &str,
        draft_id: Option<&str>,
        template_id: Option<&str>,
    ) -> Result<PromptTask> {
        let id = xai_grok_tools::util::ulid::mint();
        let now = Utc::now().to_rfc3339();
        self.connection()
            .execute(
                "INSERT INTO prompt_tasks
                   (id, body, status, created_at, updated_at, draft_id, template_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, body, status, now, now, draft_id, template_id],
            )
            .context("insert prompt_tasks")?;
        Ok(PromptTask {
            id,
            body: body.to_owned(),
            status: status.to_owned(),
            created_at: now.clone(),
            updated_at: now,
            draft_id: draft_id.map(str::to_owned),
            template_id: template_id.map(str::to_owned),
        })
    }

    /// Load a prompt-task by ULID.
    pub fn load_prompt_task(&self, id: &str) -> Result<Option<PromptTask>> {
        self.connection()
            .query_row(
                "SELECT id, body, status, created_at, updated_at, draft_id, template_id
                 FROM prompt_tasks WHERE id = ?1",
                [id],
                |row| {
                    Ok(PromptTask {
                        id: row.get(0)?,
                        body: row.get(1)?,
                        status: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        draft_id: row.get(5)?,
                        template_id: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("load prompt_tasks")
    }

    /// Draft bodies for composer autocomplete. Order is insertion order.
    fn list_prompt_task_draft_texts(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .connection()
            .prepare("SELECT text FROM prompt_task_drafts")
            .context("prepare list prompt_task_drafts")?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .context("query prompt_task_drafts texts")?;
        rows.collect::<rusqlite::Result<Vec<String>>>()
            .context("collect prompt_task_drafts texts")
    }

    /// Template bodies for composer autocomplete.
    fn list_prompt_template_bodies(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .connection()
            .prepare("SELECT body FROM prompt_templates")
            .context("prepare list prompt_templates")?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .context("query prompt_templates bodies")?;
        rows.collect::<rusqlite::Result<Vec<String>>>()
            .context("collect prompt_templates bodies")
    }

    /// After a few typed characters, suggest the most complete stored draft
    /// or template whose text starts with `prefix`. Novel prefixes hide.
    pub fn suggest_prompt_from_stored(
        &self,
        prefix: &str,
    ) -> Result<Option<StoredPromptSuggestion>> {
        if prefix.chars().count() < STORED_PROMPT_SUGGEST_MIN_CHARS {
            return Ok(None);
        }
        let drafts = self.list_prompt_task_draft_texts()?;
        let templates = self.list_prompt_template_bodies()?;
        Ok(suggest_most_complete_stored_prompt(
            prefix, drafts, templates,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    #[test]
    fn insert_draft_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let inserted = store
            .insert_prompt_task_draft("finish the schema slice")
            .unwrap();
        assert_eq!(inserted.text, "finish the schema slice");
        assert!(xai_grok_tools::util::ulid::is_valid(&inserted.id));
        assert_eq!(inserted.id.len(), 26);
        assert!(!inserted.created_at.is_empty());
        assert_eq!(inserted.created_at, inserted.updated_at);

        let loaded = store
            .load_prompt_task_draft(&inserted.id)
            .unwrap()
            .expect("draft row");
        assert_eq!(loaded, inserted);
        assert!(
            store
                .load_prompt_task_draft("01AAAAAAAAAAAAAAAAAAAAAAAA")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn insert_prompt_as_task_with_minted_ulid_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let inserted = store
            .insert_prompt_task("queued operator prompt", "queued", None, None)
            .unwrap();
        assert_eq!(inserted.body, "queued operator prompt");
        assert_eq!(inserted.status, "queued");
        assert!(inserted.draft_id.is_none());
        assert!(inserted.template_id.is_none());
        assert!(xai_grok_tools::util::ulid::is_valid(&inserted.id));
        assert_eq!(inserted.id.len(), 26);

        let loaded = store
            .load_prompt_task(&inserted.id)
            .unwrap()
            .expect("task row");
        assert_eq!(loaded, inserted);

        let draft = store.insert_prompt_task_draft("draft body").unwrap();
        let linked = store
            .insert_prompt_task("from draft", "planned", Some(&draft.id), None)
            .unwrap();
        let loaded_linked = store
            .load_prompt_task(&linked.id)
            .unwrap()
            .expect("linked task");
        assert_eq!(loaded_linked.draft_id.as_deref(), Some(draft.id.as_str()));
        assert!(loaded_linked.template_id.is_none());
    }

    /// After a few characters that uniquely prefix a stored draft, suggest
    /// that draft's full text. Too-short prefixes stay silent.
    #[test]
    fn after_a_few_characters_suggests_the_most_complete_matching_stored_draft() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .insert_prompt_task_draft("finish the schema slice")
            .unwrap();

        assert!(
            store.suggest_prompt_from_stored("fi").unwrap().is_none(),
            "two characters is not yet a few; hide the suggestion"
        );

        let suggestion = store
            .suggest_prompt_from_stored("fin")
            .unwrap()
            .expect("three characters uniquely prefix the stored draft");
        assert_eq!(suggestion.full_text, "finish the schema slice");
        assert_eq!(suggestion.kind, StoredPromptKind::Draft);
        assert_eq!(
            suggestion.ghost_remainder("fin").as_deref(),
            Some("ish the schema slice")
        );
    }

    /// Typed text that is not a prefix of any stored draft or template hides
    /// the suggestion.
    #[test]
    fn novel_prefix_hides_the_suggestion() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .insert_prompt_task_draft("finish the schema slice")
            .unwrap();
        store
            .insert_prompt_template("review", "review the pull request")
            .unwrap();

        assert!(
            store
                .suggest_prompt_from_stored("xyzzy cannot match")
                .unwrap()
                .is_none(),
            "a novel prompt must not show a stored suggestion"
        );
        assert!(
            suggest_most_complete_stored_prompt(
                "xyz",
                ["finish the schema slice"],
                ["review the pull request"],
            )
            .is_none()
        );
    }

    /// Accept completes the typed prefix to the stored value.
    #[test]
    fn accept_completes_composer_text_to_the_stored_value() {
        let suggestion = suggest_most_complete_stored_prompt(
            "fin",
            ["finish the schema slice"],
            [] as [&str; 0],
        )
        .expect("prefix matches the stored draft");
        assert_eq!(
            accept_stored_prompt_suggestion("fin", &suggestion).as_deref(),
            Some("finish the schema slice")
        );
        assert!(
            accept_stored_prompt_suggestion("novel", &suggestion).is_none(),
            "accept must not fire when the typed text is not a prefix"
        );
    }

    /// Several stored rows may share a prefix. Prefer the most complete
    /// (longest) stored text, not a shorter prefix-only match.
    #[test]
    fn prefers_the_longest_matching_stored_text_over_a_shorter_prefix_match() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store.insert_prompt_task_draft("finish").unwrap();
        store
            .insert_prompt_task_draft("finish the schema slice")
            .unwrap();
        store
            .insert_prompt_template("finisher", "finish later")
            .unwrap();

        let suggestion = store
            .suggest_prompt_from_stored("fini")
            .unwrap()
            .expect("several rows share this prefix");
        assert_eq!(suggestion.full_text, "finish the schema slice");
        assert_eq!(suggestion.kind, StoredPromptKind::Draft);
    }

    /// A template body can win when it is the most complete stored match.
    #[test]
    fn template_body_is_suggested_when_it_is_the_most_complete_match() {
        let picked = suggest_most_complete_stored_prompt(
            "rev",
            ["rev draft note"],
            ["review the pull request with the named tests"],
        )
        .expect("template body is longer");
        assert_eq!(
            picked.full_text,
            "review the pull request with the named tests"
        );
        assert_eq!(picked.kind, StoredPromptKind::Template);
    }
}
