# Stock / product skill bodies on this branch

Date: 2026-08-15  
Workspace: `/home/hunter/Projects/surmount/grok-build`  
Question: which in-repo paths are **stock/product skill bodies** that ship with Grok Build?

**Finding:** this branch ships **skill machinery** (discovery, bundle install/sync, user-guide). It does **not** ship any `SKILL.md` skill-pack trees. There is no in-repo catalog of named stock skill directories to list. Platform skill **bodies** arrive at runtime from the network bundle (Build disk cache) or from grok.com REST (chat kind only). Those payloads are authored outside this git tree.

---

## 1. Direct inventory of stock roots (absolute paths)

### 1.1 In-repo product skill roots (supported, empty)

These are the **only** git-trackable product skill homes the loader treats as project stock. On this checkout they hold **zero** skill packs.

| Absolute path | On disk | `SKILL.md` parent dirs |
|---------------|---------|------------------------|
| `/home/hunter/Projects/surmount/grok-build/.agents/skills` | **Missing.** `.agents/` exists (`plans/`, `reports/`, `joins/`) with no `skills/` | none |
| `/home/hunter/Projects/surmount/grok-build/.grok/skills` | **Missing.** `.grok/` exists with `workflows/git-recon-status.rhai` only | none |
| `/home/hunter/Projects/surmount/grok-build/.agents/commands` | Missing | none (legacy flat `*.md` commands) |
| `/home/hunter/Projects/surmount/grok-build/.grok/commands` | Missing | none |

Repo-wide search for files named `SKILL.md` under `crates/`, `doc/`, and `.agents/` returned **no files**. No `include_str!` / rust-embed of a skill pack exists.

### 1.2 Crate trees that look like they might hold packs (they do not)

| Path | What it actually is |
|------|---------------------|
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-bundle/` | Cache writer + Python-allowlist tests. No skill bodies. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md` | User-guide. Not a skill pack. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-tools/src/implementations/skills/` | Parse / walk / denylist. Not packs. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/prompt/skills.rs` | Load-order orchestrator. Not packs. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/templates/` | Agent system prompts (`prompt.md`, `subagent_prompt.md`, `apply_patch_prompt.md`). Not `SKILL.md`. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/templates/` | Goal planner prompts. Not skills. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/workflows/deep_research.rhai` | Built-in Rhai workflow, not a skill pack. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-plugin-marketplace/` | Marketplace scanner. No shipped plugin skill trees. |
| `/home/hunter/Projects/surmount/grok-build/prod/mc/cli-chat-proxy-types/src/subagent_bundle.rs` | Wire type `SubagentBundle { skills: HashMap }`. Empty by default. |

**Stock skill names from in-repo `SKILL.md` parent dirs:** none.

---

## 2. How this was verified (code + load paths)

Docs (`AGENTS.md`, `FORK.md`, `08-skills.md`, `where-skills-come-from-2026-07-24.md`) were treated as untrusted until the loader matched them.

### 2.1 Build (Grok Build) disk discovery

Orchestrator: `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/prompt/skills.rs`

Priority (module docs + `list_skills_with_plugins` / `list_skills_with_options`):

1. Walk cwd up to git root. At each directory, scan config dir **names** in this order: `.agents`, `.grok`, then vendor `.claude` / `.cursor` if compat skills cells are on.
2. Optional workspace-user dir (same names).
3. User: `$HOME/.agents` then `grok_home` (`~/.grok`) then vendor homes.
4. Always scan `<grok_home>/bundled` as `SkillScope::Bundled`.
5. `[skills].paths`, then injected `server_skill_dirs`, then injected `bundled_skill_dirs`.
6. Plugins last. Native bare names win.

Config dir names (always-on first two):

```360:377:crates/codegen/xai-grok-tools/src/types/compat.rs
    /// Config directories that may contain `skills/` subdirectories, in
    /// priority order. `.agents` and `.grok` are always included (`.agents`
    /// first so maintained agent skills override Grok-owned trees at the same
    /// tier); `.claude` and `.cursor` are gated on their respective `skills` cell.
    pub fn skill_config_dirs(&self) -> Vec<&'static str> {
        let mut dirs = vec![".agents", ".grok"];
        // ...
    }
```

What counts as a skill file under those dirs:

```68:77:crates/codegen/xai-grok-tools/src/implementations/skills/discovery.rs
pub fn find_skill_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for subdir in SKILL_SUBDIRS {
        let skills_dir = dir.join(subdir);
        if skills_dir.is_dir() {
            walk_for_skill_md(&skills_dir, &mut paths, 0);
        }
    }
    paths
}
```

`SKILL_SUBDIRS` is only `"skills"` (depth ≤ 5). Flat `commands/*.md` is a legacy slash-command layout, not a stock pack tree.

Bundled scan (lowest native scope):

```331:337:crates/codegen/xai-grok-agent/src/prompt/skills.rs
    let bundled_dir = global_dir.join("bundled");
    collect_discovered_paths(
        find_skill_paths(&bundled_dir),
        SkillScope::Bundled,
        &mut seen_canonical_paths,
        &mut skill_files,
    );
```

`find_skill_paths` on `<grok_home>/bundled` looks for `<grok_home>/bundled/skills/**/SKILL.md`. That is the **install cache**, not an in-repo source.

Product test that treats project roots as the in-repo skill homes (and allows them to be absent):

```894:903:crates/codegen/xai-grok-bundle/src/lib.rs
    fn product_repo_skill_roots_have_no_non_excepted_python() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut bad = Vec::new();
        for rel in [".agents/skills", ".grok/skills"] {
            collect_non_excepted_skill_python(&repo.join(rel), &mut bad);
        }
        assert!(
            bad.is_empty(),
            "product skill roots must not contain non-excepted Python: {bad:?}"
        );
    }
```

Missing dirs are a no-op (`read_dir` fail returns). That matches this checkout.

Workspace inject (runtime, not in-repo source): `GROK_WORKSPACE_SERVER_SKILLS_DIR` and `GROK_WORKSPACE_BUNDLED_SKILLS_DIR` in `crates/codegen/xai-grok-workspace/src/handle.rs` around L3986–3999.

### 2.2 Bundle install/sync (platform packs, not git)

Source of **bodies** the product installs:

1. `GET {cli-chat-proxy}/bundle/archive` (tar.gz).
2. On non-success (except 401), fall back to `GET {cli-chat-proxy}/subagents/bundle` JSON (`SubagentBundle.skills`).

Evidence: `crates/codegen/xai-grok-shell/src/remote/client.rs` `fetch_bundle_inner` (archive URL `…/bundle/archive`, fallback `fetch_subagent_bundle`).

Writer: `crates/codegen/xai-grok-bundle/src/lib.rs` `write_bundle_to_cache` / `extract_bundle_archive` → `<grok_home>/bundled/skills/<name>/SKILL.md` (plus companions under that tree). Sync entry: `crates/codegen/xai-grok-shell/src/extensions/bundle.rs` `sync_bundle_to_root`.

The archive is **not** vendored in this repo. Tests invent fake `commit` / `implement` entries. There is no committed default `SubagentBundle`.

User-guide (aligned with that write path): bundled skills live under `~/.grok/bundled/skills/`; the product never writes them into `~/.grok/skills/`.

### 2.3 Chat-kind “product skills” (different rail)

`AcuSkillSource::Product` when session kind is chat; `AcuSkillSource::Disk` for Build.

```1092:1106:crates/codegen/xai-grok-shell/src/session/slash_commands.rs
pub(crate) enum AcuSkillSource {
    /// Product REST catalog (`kind: chat`).
    Product,
    /// Disk / plugin skills via `SkillManager` (Build).
    Disk,
}
pub(crate) fn acu_skill_source(is_chat_kind: bool) -> AcuSkillSource {
    if is_chat_kind {
        AcuSkillSource::Product
    } else {
        AcuSkillSource::Disk
    }
}
```

Chat REST: `POST https://grok.com/rest/skills` (first-party bundled names) and `GET /rest/user-skills`. Comment in `crates/codegen/xai-grok-shell/src/remote/skills_client.rs`: bundled rows are advertised with `body: None` and a synthetic `chat-product://` path. Shell does not load a local `SKILL.md` for them. Expansion is product/gateway-side.

Grok Build sessions use **Disk**, so chat REST names are **not** Build stock bodies.

### 2.4 Former in-binary extract (removed)

`crates/codegen/xai-grok-shell/src/builtin.rs` no longer extracts skills into `~/.grok/skills/`. It only extracts `README.md`. `purge_stale_extracted_skills` deletes leftover dirs whose `SKILL.md` still hashes to a **former** shipped body.

Names that used to be extracted (hash table only; bodies are **not** in the crate):

- `best-of-n`
- `check`
- `check-work`
- `code-review`
- `create-skill`
- `create-workflow`
- `docx`
- `help`
- `imagine`
- `pptx`
- `xlsx`

That is a **legacy name list**, not a live stock tree.

### 2.5 Names the bundle writer is willing to treat as product skill tree (still not bodies)

Python allowlist when installing a **remote** archive into the cache (`is_allowed_product_skill_python`):

| Relative cache path | Implies skill / tree name |
|---------------------|---------------------------|
| `skills/implement/scripts/memory.py` | `implement` |
| `skills/execute-plan/scripts/validate-plan.py` | `execute-plan` |
| `skills/shared/resume-session/session_reader.py` | shared helper, not a `SKILL.md` parent |
| `skills/docx/**/*.py` | `docx` |
| `skills/pptx/**/*.py` | `pptx` |
| `skills/xlsx/**/*.py` | `xlsx` |
| `skills/pdf/**/*.py` | `pdf` |

`create-skill` must **not** ship a Python scaffold. Review helper `.py` is rejected. These names are **filters** on a network archive, not in-repo packs.

---

## 3. What is **not** stock

| Path / source | Why it is not in-repo stock |
|---------------|-----------------------------|
| `~/.agents/skills` | Host operator overlay. Wins at User tier (`.agents` before `.grok`). Own git. Contrast only. |
| `~/.grok/skills` | User-owned. Product explicitly stopped extracting platform skills here. |
| `~/.grok/bundled/skills` | **Cache leftover** written by bundle sync. Not source. Version/names follow the last successful network fetch on that machine. |
| `~/.claude/skills`, `~/.cursor/skills` and project twins | Vendor compat. Cursor/Claude default **names** are denylisted when under those path segments. |
| `[skills].paths`, plugins, marketplace installs, `GROK_WORKSPACE_*` dirs | Config / managed / third-party. |
| Rust slash builtins (`/login`, `/compact`, …) | Product commands, not `SKILL.md` packs. |
| `.grok/workflows/` in this repo | Rhai workflow, not a skill. |

Land class 7 (“product skills are not a Python runtime”) is a **constraint** on what the product may install into skill trees. It is not a list of skill names. Tests: `xai-grok-bundle` `product_repo_skill_roots_have_no_non_excepted_python`; `xai-grok-pager` `user_guide_skills_are_not_a_python_runtime`.

---

## 4. Ambiguity (named, not guessed)

1. **“Stock” has two product rails.** Build stock bodies, when they exist at all, are disk `SKILL.md` under project roots or the **network subagent bundle** cache. Chat “product skills” are a grok.com REST catalog without in-repo bodies. This report is about Grok Build on this branch; chat names are noted only so they are not mixed in.

2. **The remote bundle’s current skill list is not in this repo.** Code does not hardcode a complete live set. Former extract hashes plus the Python allowlist are **hints**, not a guarantee of what `/v1/bundle/archive` returns today. A host `~/.grok/bundled/skills` listing would describe that machine’s cache, not branch stock.

3. **Project roots are allowed to be empty.** `FORK.md` says `.agents/skills` and `.grok/skills` are “supported; may be empty.” That matches the tree. Collaborators *could* commit packs there; this checkout has not.

4. **Research note stale on `.grok/`.** `doc/dev/research/where-skills-come-from-2026-07-24.md` §3.1 said “No `.grok/` project dir.” Today `/home/hunter/Projects/surmount/grok-build/.grok/workflows/` exists. Still no `.grok/skills`. The “no committed `SKILL.md` packs” claim still holds.

5. **`shared/` under a bundled skills tree** is allowed as companion files (`skills/shared/resume-session/…`, `skills/shared/personas/…`). That is not a skill named `shared` unless a `SKILL.md` parent exists.

---

## 5. One-sentence contrast (host overlay / cache)

Operator packs live under `~/.agents/skills` (not this branch). `~/.grok/bundled/skills` is only the product’s network-install cache, not stock source.

STATUS: COMPLETE
