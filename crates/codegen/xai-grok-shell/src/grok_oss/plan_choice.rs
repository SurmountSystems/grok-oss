//! Explicit plan-review choices in `grok_oss.db`.
//!
//! Additive Surmount table (schema v4). Not session `plan_mode.json` sticky.
//! A row exists only when the operator clicked Approve, Comment, Revise, or
//! Exit. Present, empty Enter, and always-approve tool permissions do not
//! write a row.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;

use super::GrokOssStore;

/// Identity for the session's current `plan.md`.
pub const SESSION_PLAN_IDENTITY: &str = "plan.md";

/// One idle plan CTA the operator explicitly chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanRecordedChoice {
    Approve,
    Comment,
    Revise,
    Exit,
}

impl PlanRecordedChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Comment => "comment",
            Self::Revise => "revise",
            Self::Exit => "exit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "approve" => Some(Self::Approve),
            "comment" => Some(Self::Comment),
            "revise" => Some(Self::Revise),
            "exit" => Some(Self::Exit),
            _ => None,
        }
    }
}

/// One recorded-choice row. Primary key is a minted ULID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRecordedChoiceRow {
    pub id: String,
    pub session_id: String,
    pub plan_identity: String,
    pub choice: PlanRecordedChoice,
    pub chosen_at: String,
}

impl GrokOssStore {
    /// Insert an explicit plan choice. Mints a 26-character Crockford ULID.
    pub fn insert_plan_recorded_choice(
        &self,
        session_id: &str,
        plan_identity: &str,
        choice: PlanRecordedChoice,
    ) -> Result<PlanRecordedChoiceRow> {
        let id = xai_grok_tools::util::ulid::mint();
        let now = Utc::now().to_rfc3339();
        self.connection()
            .execute(
                "INSERT INTO plan_recorded_choice (id, session_id, plan_identity, choice, chosen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![id, session_id, plan_identity, choice.as_str(), now],
            )
            .context("insert plan_recorded_choice")?;
        Ok(PlanRecordedChoiceRow {
            id,
            session_id: session_id.to_owned(),
            plan_identity: plan_identity.to_owned(),
            choice,
            chosen_at: now,
        })
    }

    /// Latest explicit choice for this session and plan identity, if any.
    pub fn latest_plan_recorded_choice(
        &self,
        session_id: &str,
        plan_identity: &str,
    ) -> Result<Option<PlanRecordedChoiceRow>> {
        self.connection()
            .query_row(
                "SELECT id, session_id, plan_identity, choice, chosen_at
                 FROM plan_recorded_choice
                 WHERE session_id = ?1 AND plan_identity = ?2
                 ORDER BY chosen_at DESC, id DESC
                 LIMIT 1",
                rusqlite::params![session_id, plan_identity],
                |row| {
                    let choice_raw: String = row.get(3)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        choice_raw,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .context("load plan_recorded_choice")?
            .map(|(id, session_id, plan_identity, choice_raw, chosen_at)| {
                let choice = PlanRecordedChoice::parse(&choice_raw).ok_or_else(|| {
                    anyhow::anyhow!("unknown plan_recorded_choice value {choice_raw}")
                })?;
                Ok(PlanRecordedChoiceRow {
                    id,
                    session_id,
                    plan_identity,
                    choice,
                    chosen_at,
                })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    #[test]
    fn insert_plan_recorded_choice_with_minted_ulid_then_load_latest() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        assert!(
            store
                .latest_plan_recorded_choice("sess-1", SESSION_PLAN_IDENTITY)
                .unwrap()
                .is_none()
        );

        let first = store
            .insert_plan_recorded_choice(
                "sess-1",
                SESSION_PLAN_IDENTITY,
                PlanRecordedChoice::Comment,
            )
            .unwrap();
        assert!(xai_grok_tools::util::ulid::is_valid(&first.id));
        assert_eq!(first.id.len(), 26);
        assert_eq!(first.choice, PlanRecordedChoice::Comment);

        let second = store
            .insert_plan_recorded_choice(
                "sess-1",
                SESSION_PLAN_IDENTITY,
                PlanRecordedChoice::Approve,
            )
            .unwrap();
        let latest = store
            .latest_plan_recorded_choice("sess-1", SESSION_PLAN_IDENTITY)
            .unwrap()
            .expect("latest row");
        assert_eq!(latest.id, second.id);
        assert_eq!(latest.choice, PlanRecordedChoice::Approve);

        assert!(
            store
                .latest_plan_recorded_choice("sess-other", SESSION_PLAN_IDENTITY)
                .unwrap()
                .is_none()
        );
    }
}
