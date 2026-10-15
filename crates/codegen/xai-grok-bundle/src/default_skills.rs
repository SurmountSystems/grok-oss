//! In-tree Grok OSS default skills.
//!
//! Source of truth is this crate's `skills/` directory (compiled in with
//! `include_str!`). On startup, and after network bundle extract, Grok
//! writes those files into `<grok home>/bundled/skills/`. Discovery already
//! scans that tree as `Bundled` scope (`list_skills_with_options`).
//!
//! These files are **not** listed in the network `manifest.json`. A later
//! cli-chat-proxy extract therefore does not prune them as removed archive
//! entries. User edits (checksum mismatch vs every shipped body) are kept.

use anyhow::Result;
use std::path::Path;

use super::{checksum_bytes, checksum_file_if_exists, ensure_bundle_dirs, write_bundle_file};

/// Skill directory names shipped as Grok OSS defaults.
pub const DEFAULT_PRODUCT_SKILL_NAMES: &[&str] =
    &["polish", "pull-remote-tree", "subagent", "what"];

struct DefaultProductSkillFile {
    relative_path: &'static str,
    content: &'static str,
    /// Older shipped bodies we may overwrite on upgrade. Empty on first land.
    previous_checksums: &'static [&'static str],
}

const DEFAULT_PRODUCT_SKILL_FILES: &[DefaultProductSkillFile] = &[
    DefaultProductSkillFile {
        relative_path: "skills/polish/SKILL.md",
        content: include_str!("../skills/polish/SKILL.md"),
        previous_checksums: &[],
    },
    DefaultProductSkillFile {
        relative_path: "skills/polish/references/incident-classes.md",
        content: include_str!("../skills/polish/references/incident-classes.md"),
        previous_checksums: &[],
    },
    DefaultProductSkillFile {
        relative_path: "skills/subagent/SKILL.md",
        content: include_str!("../skills/subagent/SKILL.md"),
        // Tree body before output_tokens. Same bytes as the installed cache.
        previous_checksums: &["ff6b45403003ad75e67c770ab1240a6c61f14001392e57da220499a9de5129c8"],
    },
    DefaultProductSkillFile {
        relative_path: "skills/what/SKILL.md",
        content: include_str!("../skills/what/SKILL.md"),
        // Installed cache from 2026-08-28, then the later tree body before
        // output_tokens. Both lack local_usage_events_for_session.
        previous_checksums: &[
            "f34d41a91fd6ebc26204b9de85665e4080e4f35cbded31b440504f28d04e342d",
            "6ced030035c14a22ad6abfd6416d2915b9506591e8f366fdad3fe50ce9cb0b96",
        ],
    },
    DefaultProductSkillFile {
        relative_path: "skills/pull-remote-tree/SKILL.md",
        content: include_str!("../skills/pull-remote-tree/SKILL.md"),
        previous_checksums: &[],
    },
];

/// Install Grok OSS default skills into `root` (`<grok home>/bundled`).
///
/// Writes when the path is missing, matches the current shipped body, or
/// matches a previous shipped body. Leaves any other on-disk bytes alone.
pub fn install_default_product_skills(root: &Path) -> Result<()> {
    ensure_bundle_dirs(root)?;
    for file in DEFAULT_PRODUCT_SKILL_FILES {
        install_one(root, file)?;
    }
    Ok(())
}

fn install_one(root: &Path, file: &DefaultProductSkillFile) -> Result<()> {
    let absolute_path = root.join(file.relative_path);
    let shipped = checksum_bytes(file.content.as_bytes());
    match checksum_file_if_exists(&absolute_path)? {
        None => write_bundle_file(&absolute_path, file.content.as_bytes())?,
        Some(on_disk) if on_disk == shipped => {}
        Some(on_disk) if file.previous_checksums.iter().any(|old| *old == on_disk) => {
            write_bundle_file(&absolute_path, file.content.as_bytes())?;
        }
        Some(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn read(root: &Path, rel: &str) -> String {
        std::fs::read_to_string(root.join(rel)).unwrap()
    }

    // Grok OSS: /polish /subagent /what /pull-remote-tree are default Grok OSS skills in-tree. This diverges from upstream xAI because FORK.md and the skills catalog pin those four as product defaults.
    #[test]
    fn default_product_skills_include_polish_and_subagent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
        for name in DEFAULT_PRODUCT_SKILL_NAMES {
            let skill_md = root.join(name).join("SKILL.md");
            assert!(
                skill_md.is_file(),
                "default product skill {name} must exist at {}",
                skill_md.display()
            );
            let body = std::fs::read_to_string(&skill_md).unwrap();
            assert!(
                body.contains(&format!("name: {name}")),
                "{name} SKILL.md must declare its name in frontmatter"
            );
            assert!(
                body.contains("default Grok OSS skill"),
                "{name} must say it is a default Grok OSS skill"
            );
            assert!(
                body.contains(&format!("bundled/skills/{name}")),
                "{name} must name the bundled install path"
            );
        }
        assert!(
            root.join("polish/references/incident-classes.md").is_file(),
            "polish must ship incident-classes.md"
        );
    }

    #[test]
    fn install_writes_polish_and_subagent_into_bundled_skills() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        install_default_product_skills(root).unwrap();
        assert!(
            read(root, "skills/polish/SKILL.md").contains("name: polish"),
            "install must write polish"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md").contains("name: subagent"),
            "install must write subagent"
        );
        assert!(
            read(root, "skills/what/SKILL.md").contains("name: what"),
            "install must write what"
        );
        assert!(
            read(root, "skills/what/SKILL.md")
                .contains("Never mix Grok Build version with grok-oss product version"),
            "installed what skill must keep grok-oss and Grok Build versions distinct"
        );
        assert!(
            read(root, "skills/what/SKILL.md").contains("local_usage_events_for_session"),
            "installed what skill must name local_usage_events_for_session"
        );
        assert!(
            read(root, "skills/what/SKILL.md")
                .contains("A null `output_tokens` is not printed as `0`"),
            "installed what skill must not print a null output_tokens as 0"
        );
        assert!(
            read(root, "skills/what/SKILL.md").contains("not `total_tokens`"),
            "installed what skill reconcile cell must not be total_tokens"
        );
        assert!(
            read(root, "skills/what/SKILL.md")
                .contains("`reasoning_tokens` and is not added again"),
            "installed what skill must not add chain of thought again"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md").contains("local_usage_events_for_session"),
            "installed subagent skill must name local_usage_events_for_session"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md")
                .contains("A null `output_tokens` is not printed as `0`"),
            "installed subagent skill must not print a null output_tokens as 0"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md").contains("not `total_tokens`"),
            "installed subagent skill reconcile cell must not be total_tokens"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md")
                .contains("`reasoning_tokens` and is not added again"),
            "installed subagent skill must not add chain of thought again"
        );
        assert!(
            read(root, "skills/polish/SKILL.md")
                .contains("Never mix Grok Build version with grok-oss product version"),
            "installed polish remaining-work pointer must keep grok-oss and Grok Build versions distinct"
        );
        assert!(
            read(root, "skills/pull-remote-tree/SKILL.md").contains("name: pull-remote-tree"),
            "install must write pull-remote-tree"
        );
        assert!(
            read(root, "skills/polish/references/incident-classes.md").contains("Incident classes"),
            "install must write polish references"
        );
        assert!(
            !root.join("manifest.json").exists(),
            "default product skills must not join the network bundle manifest"
        );
    }

    // Previous shipped bodies live in this tracked file. Nix does not copy untracked markdown.
    const WHAT_INSTALLED_BEFORE_OUTPUT_TOKENS: &str = r#"---
name: what
description: >
  Restate this session in four short complete American English thoughts
  when the operator cannot parse agent chat. Follow Concise American
  Technical English as specified in Surmount 0005_CATE.md. Use when the
  user runs /what, says they do not understand, asks you to speak more
  clearly, or asks what is going on in this session. Not /recap. Not
  /finish. Not /reports. Not an apology.
metadata:
  short-description: "Four complete English thoughts: work, truth, operator, next"
  argument-hint: "[optional focus]"
---

# What

The operator cannot parse the last agent chat. Restate. Do not apologize.
Do not write a file. Do not spawn.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/what/` on startup. The live cache is not
the source. Do not add a project `.agents/skills/what` copy unless
the user asked for a project override.

Follow Concise American Technical English (CATE), numbered specification
`0005_CATE.md` in SurmountSystems/specs
(https://github.com/SurmountSystems/specs/blob/main/0005_CATE.md ,
accessed: 2026-08-27). CATE is not specification 0006.

Reply with this shape only. Four labeled complete thoughts. Nothing
fluffier. One idea per sentence when that stays clear.

1. **What we are doing:** one sentence. The real product outcome this
   session is trying to finish right now.
2. **What is true right now:** running, waiting, blocked, or done.
   Name the real file, command, crate, or test. Do not use private
   labels. Translate leftover jargon from the last agent message into
   ordinary words.
3. **What you need to do:** the operator action, or the word "nothing"
   if they do not need to act. Then say why. Name evidence: who owns
   the next action, which command they asked to keep, which gate is
   unmet. Do not leave a bare "nothing."
4. **What I will do next:** the next concrete agent step.

## Rules

- Complete American English thoughts. Short sentences. No half labels
  used as sentences.
- Name the real thing: the file, the command, the crate, the test, the
  outcome. Decoder ring jargon is forbidden (private nicknames that
  force the reader to decode). If the last message used those nicknames,
  translate them.
- A guess is a guess: omit it or label it.
- On **What you need to do**, evidence is required. "Nothing" is valid
  only when you can say why (the next step is yours, a gate they set
  is unmet, they did not ask for git or install).
- Do not put leftover board ids, hex run ids, or compacted codes in
  the body.
- Do not ask them to say a magic word to continue when the next step
  is already clear. Do that step, or name it under **What I will do next**.
- Optional focus from `/what ...` is the part they did not understand.
  Answer it under the four labels. Do not add extra sections.
- This is not `/recap`, not `/finish`, not `/reports`.
- When the operator asks to revise a skill in grok-oss, edit
  `crates/codegen/xai-grok-bundle/skills/`, not only a host overlay
  and not repo `.agents/skills/`.
"#;

    const WHAT_TREE_BEFORE_OUTPUT_TOKENS: &str = r#"---
name: what
description: >
  Restate this session in four short complete American English thoughts
  when the operator cannot parse agent chat. Follow Concise American
  Technical English as specified in Surmount 0005_CATE.md. Use when the
  user runs /what, says they do not understand, asks you to speak more
  clearly, or asks what is going on in this session. Not /recap. Not
  /finish. Not /reports. Not an apology.
metadata:
  short-description: "Four complete English thoughts: work, truth, operator, next"
  argument-hint: "[optional focus]"
---

# What

The operator cannot parse the last agent chat. Restate. Do not apologize.
Do not write a file. Do not spawn.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/what/` on startup. The live cache is not
the source. Do not add a project `.agents/skills/what` copy unless
the user asked for a project override.

Follow Concise American Technical English (CATE), numbered specification
`0005_CATE.md` in SurmountSystems/specs
(https://github.com/SurmountSystems/specs/blob/main/0005_CATE.md ,
accessed: 2026-08-27). CATE is not specification 0006.

Reply with this shape only. Four labeled complete thoughts. Nothing
fluffier. One idea per sentence when that stays clear. Labels are
**Job / State / Operator / Next**. Prefer Operator and Agent as speaker labels.
Address the person as Operator, not Human. Do not say You or Human for the operator.
Do not say Me or Grok as the speaker label for the machine
(Grok OSS and grok-oss stay product names).
Painted chrome and user-guide call the composer the Operator box.

1. **Job:** one sentence. The real product outcome this session is
   trying to finish right now.
2. **State:** running, waiting, blocked, or done.
   Name the real file, command, crate, or test. Do not use private
   labels. Translate leftover jargon from the last agent message into
   ordinary words.
3. **Operator:** the operator action, or the word "nothing"
   if they do not need to act. Then say why. Name evidence: who owns
   the next action, which command they asked to keep, which gate is
   unmet. Do not leave a bare "nothing."
4. **Next:** the next concrete agent step.

## Rules

- Complete American English thoughts. Short sentences. No half labels
  used as sentences.
- Name the real thing: the file, the command, the crate, the test, the
  outcome. Decoder ring jargon is forbidden (private nicknames that
  force the reader to decode). If the last message used those nicknames,
  translate them.
- A guess is a guess: omit it or label it.
- On **Operator**, evidence is required. "Nothing" is valid
  only when you can say why (the next step is the agent's, a gate they
  set is unmet, they did not ask for git or install).
- Do not put leftover board ids, hex run ids, or compacted codes in
  the body.
- Do not ask them to say a magic word to continue when the next step
  is already clear. Do that step, or name it under **Next**.
- Optional focus from `/what ...` is the part they did not understand.
  Answer it under the four labels. Do not add extra sections.
- This is not `/recap`, not `/finish`, not `/reports`.
- Never mix Grok Build version with grok-oss product version. grok-oss
  is `grok-oss --version` (`1.0.3` plus git SHA). Grok Build is
  `grok --version`. Isolated Preview and plan chrome are grok-oss unless
  this process was launched as `grok` from downloads. Remaining-work,
  reports, and this restatement: probe this turn if stating which
  binary this window is. Do not reuse a leftover Grok Build version as
  grok-oss.
- When the operator asks to revise a skill in grok-oss, edit
  `crates/codegen/xai-grok-bundle/skills/`, not only a host overlay
  and not repo `.agents/skills/`.
"#;

    const SUBAGENT_BEFORE_OUTPUT_TOKENS: &str = r#"---
name: subagent
description: >
  Spawn one L2 coordinator with the user job as a self-contained
  prompt. The L1 main thread does not do the job. Use when the user
  runs /subagent, /subagent this, says they want this explicitly
  subagented, or asks to spawn an L2 coordinator. Not /polish. Not
  /implement. Not Hierarchical fast path on L1.
metadata:
  short-description: "Spawn one L2 coordinator for this job"
  argument-hint: "this <job> | <job>"
---

# Subagent

Spawn **one L2 coordinator** for the named job. The L1 main thread
does not diagnose, implement, or walk files for that job.

Not `/polish` (session polish pass). Not `/implement` (plan handoff).
Not the Hierarchical fast path on L1.

Product CLI is `grok-oss`. SuperGrok is a paid product. Never call
SuperGrok free. Complete American English thoughts. No nicknames.
Never bare child/children as agent names.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/subagent/` on startup. The live cache is not
the source. Do not add a project `.agents/skills/subagent` copy unless
the user asked for a project override.

## Steps

1. **Job text.** Take the rest of the slash line. `/subagent this ...`
   and `/subagent ...` are the same: strip a leading `this` token if
   present. That remainder is the job. If empty, ask once in
   freeform for the job. Do not invent it.
2. **Board.** Same turn `todo_write` merge upsert: short owed
   outcome. Bugs → `bug:<slug>`, features → `feat:<slug>`, other
   work → a namespaced id. Never `merge: false` wipe.
3. **Disk pointer.** When the contract must survive compaction,
   write a remaining-work pointer under `~/.agents/reports/` on
   this machine. Call it a report, not a join. Chat is not enough.
4. **Spawn one L2.** The product tool is `spawn_subagent`. The prompt
   is **self-contained**: the full job text, this tree, product CLI
   `grok-oss`, agent depth, report path, never `git add` / `git
   commit`. Do not tell L2 to "see the conversation above."
5. **Wait or other tracks.** Wait on that L2, or keep other
   healthy tracks running. Additive **also** / **btw** spawns
   another L2 (or queues same-file). Do not kill a healthy
   in-flight L2.
6. **Read the report.** L1 reads only the short on-disk L2 report
   under `~/.agents/reports/`. Do not re-do the L2 greps.
7. **Close out.** Complete the board item the same turn the
   substance lands. Cancel only with a recorded reason.
8. **Stop git.** Never `git add`. Never `git commit`. Never push.

## Sub-agents

| When | Owns | L1 keeps |
|------|------|----------|
| `/subagent` job | One L2 coordinator. L3 only if that L2 decides the problem is actually hard | Goal, board id, report path, wait |
| Additive disjoint job | Another L2 (do not kill the first) | Both report paths |
| Same-file race | Queue the new writer | First L2 stays live |
| Skill-body file writes | L3 (L2 must spawn) | Path, wait, read the report |

## Agent depth (not session-board L0-L2)

| Depth | Does | Does not |
|-------|------|----------|
| **L1 main** | Status. Board upsert. Spawn one L2. Wait or other tracks. Read the short report. Hierarchical fast path only for named one-liners that are not this job. | Do this job. Diagnose. Implement. Multi-file reads. CI logs |
| **L2 coordinator** | Do the job, or parallelize. Spawn L3 **only if the problem is actually hard**. Easy work can stay on L2. Write the report. Throw context away after. | Spawn L4. Show raw edits as if L1 |
| **L3 specialist** | Tools and work when spawned. Same agency as L2 except no spawn. | Spawn L4 |

**Hierarchical fast path** (L1 only): one-command host question; a
single known-path read already named; read and quote the asked-for
report; a single already-named one-line file edit. `/subagent`
means the job is **not** that path. Spawn L2.

No L4.

## Honesty

- Do not invent remaining SuperGrok.
- Name meters in complete thoughts: the included SuperGrok period
  limits for the current billing period (how much of that included
  quota is already used) vs SuperGrok dollar credits (prepaid
  top-ups on the SuperGrok account) vs console team prepaid /
  console API credits. Never call SuperGrok free.
- Do not call any pool used up unless the live product Usage view
  or `/limits` surface they can see agrees, or a named live fetch
  of that same named meter agrees. A subagent snapshot is not
  enough to override the user.
- grok-oss limits chrome is a client printout, not xAI billing truth.
- SuperGrok Heavy is a distinct weekly pool from standard SuperGrok.

## Hard rules

- No em dashes. No unicode ellipsis. Never bare child/children as
  agent nicknames.
- Wait times of a minute or more in minutes (`15m43s`, `1h2m`).
- Never assume. Docs can lie. Verify before claiming the job is
  done.
- One reviewer per slice unless the user asked for more.
- Behavior changes: red/green TDD (observed fail, then the same
  test green). After a structured `.rs` edit, file-level
  infer-from-path verify. Do not prove product work with crate-wide
  cargo via extra agents.
"#;

    // Grok OSS: an already installed what or subagent that matches a previous
    // shipped body must be replaced once the tree copy names
    // local_usage_events_for_session. An empty previous_checksums list skips
    // that file. A body that is not a previous ship stays, so user edits stay.
    #[test]
    fn install_refreshes_previous_what_and_subagent_output_token_bodies() {
        let cases = [
            (
                "skills/what/SKILL.md",
                WHAT_INSTALLED_BEFORE_OUTPUT_TOKENS,
                "f34d41a91fd6ebc26204b9de85665e4080e4f35cbded31b440504f28d04e342d",
            ),
            (
                "skills/what/SKILL.md",
                WHAT_TREE_BEFORE_OUTPUT_TOKENS,
                "6ced030035c14a22ad6abfd6416d2915b9506591e8f366fdad3fe50ce9cb0b96",
            ),
            (
                "skills/subagent/SKILL.md",
                SUBAGENT_BEFORE_OUTPUT_TOKENS,
                "ff6b45403003ad75e67c770ab1240a6c61f14001392e57da220499a9de5129c8",
            ),
        ];
        for (rel, old_body, sum) in cases {
            assert!(
                !old_body.contains("local_usage_events_for_session"),
                "{rel} previous body must predate the reader so an empty checksum list stays red"
            );
            assert_eq!(
                checksum_bytes(old_body.as_bytes()),
                sum,
                "{rel} previous body checksum drifted"
            );
            let shipped = DEFAULT_PRODUCT_SKILL_FILES
                .iter()
                .find(|file| file.relative_path == rel)
                .unwrap_or_else(|| panic!("missing shipped file {rel}"));
            assert!(
                shipped.previous_checksums.contains(&sum),
                "{rel} previous checksum {sum} must be listed or install skips the old body"
            );
            let tmp = TempDir::new().unwrap();
            let root = tmp.path();
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, old_body).unwrap();
            install_default_product_skills(root).unwrap();
            let installed = read(root, rel);
            assert!(
                installed.contains("local_usage_events_for_session"),
                "{rel} install must copy the tree skill over the previous body"
            );
            assert!(
                installed.contains("A null `output_tokens` is not printed as `0`"),
                "{rel} install must keep the null-versus-zero sentence"
            );
        }
    }

    #[test]
    fn install_does_not_overwrite_user_edits() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        install_default_product_skills(root).unwrap();
        let path = root.join("skills/polish/SKILL.md");
        std::fs::write(&path, "user customized polish\n").unwrap();
        install_default_product_skills(root).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "user customized polish\n"
        );
    }

    #[test]
    fn install_overwrites_when_previous_checksum_matches() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let old_body = "old shipped polish\n";
        let path = root.join("skills/polish/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old_body).unwrap();
        let old_checksum = checksum_bytes(old_body.as_bytes());
        let leaked: &'static str = Box::leak(old_checksum.into_boxed_str());
        let previous: &'static [&'static str] = Box::leak(Box::new([leaked]));
        let file = DefaultProductSkillFile {
            relative_path: "skills/polish/SKILL.md",
            content: include_str!("../skills/polish/SKILL.md"),
            previous_checksums: previous,
        };
        install_one(root, &file).unwrap();
        assert!(
            read(root, "skills/polish/SKILL.md").contains("name: polish"),
            "previous shipped body must upgrade"
        );
    }
}
