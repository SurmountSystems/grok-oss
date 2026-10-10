---
name: plan
description: >
  Write a session plan under a thoughtful filename. A new plan does not
  replace an older plan. Call an existing Rust function the product
  already has. Do not invent a one-off Python or bash script. Use when
  the Operator runs /plan or asks for an implementation plan before
  coding.
metadata:
  short-description: "Thoughtful plan filename; do not replace an older plan"
  argument-hint: "[what to plan]"
---

# Plan

A new plan uses a thoughtful filename and does not replace an older plan.
`plan.md` overwrites the previous plan. Two plans in one session must
keep two files.

The source of this skill is
`crates/codegen/xai-grok-bundle/skills/plan/SKILL.md`. Do not add a copy
under project `.agents/`.

## Call Rust the product already has

Call an existing Rust function. Do not invent a one-off Python or bash
script. This skill has no Python or bash payload. The session plan file
is still written by the product Rust that joins `plan.md`
(`PlanModeTracker::new` and `PlanModeTracker::from_snapshot` in
`crates/codegen/xai-grok-shell/src/session/plan_mode.rs`). Do not add a
second writer in Python or bash beside that function.

## Surmount tests are not SpaceXAI tests

Surmount named tests are Operator contracts. They are not SpaceXAI
upstream tests. Do not fit a Surmount assert to upstream paint.

FORK.md records that difference on rows that say Grok OSS vs SpaceXAI.
Cite those rows. Do not reshape the named Surmount test so it matches
upstream paint.

- **L2 implement coordinator strips grep/read/edit (GitHub #141).**
  Grok OSS vs SpaceXAI: SpaceXAI nested L2 keeps grep, read, and edit.
  The Surmount implement coordinator strips those and keeps spawn.
- **Turbo planning.** Grok OSS vs SpaceXAI: upstream keeps session
  effort through `/plan`. Surmount turbo off is that upstream option.
  The plan stays at session effort.
- **Soft process-rule reminders.** Grok OSS vs SpaceXAI: upstream has
  no this `/settings` list.

The same difference is on the FORK.md rows that say `Grok OSS:` and
then name SpaceXAI: live Subagents list already_exited closeout,
Compacting Subagents row open, Ctrl+C two-stage, `/model` last Tab,
plan search, and Isolated Preview screenshot paste. Each named test
on those rows is the Operator contract.

FORK.md **Named tests are contracts** says the same thing in plain
words: do not fit the test to the code. Tests are Surmount contracts.
Keep the stronger assert. That section is reconciled with AGENTS.md
hard constraint 15 and § *The operator's words are the spec*.
