# Where skills come from (Surmount / Grok OSS)

**Date:** 2026-07-24  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`  
**Mode:** evidence-only (product code + host trees + docs). No product edits in this pass.

---

## 1. Plain-language correction (wrong claim)

| | |
|--|--|
| **Wrong** | “Skills don’t live on the grok-oss branch” / “skills only live in `~/.agents/skills`, separate from product.” |
| **Correct** | Skills are **multi-source**. The **product on this branch** owns discovery, precedence, parse, plugin load, bundle **install/sync**, workspace inject, and user-guide docs. Runtime loads `SKILL.md` packs from **project trees** (git-trackable on the branch), **user trees** (host), **`[skills].paths`**, **server-injected dirs**, **platform bundled cache** (host path filled by product network sync), and **plugins**. |

**Nuance for this machine today**

- There are **no committed `SKILL.md` skill packs** under the grok-build repo root (search: none outside this research note).
- Hunter’s orchestrator **bodies** (`implement`, `pr-babysit`, …) live under **`~/.agents/skills`** (host git) and **shadow** same-named `~/.grok/skills` and `~/.grok/bundled/skills` copies.
- Editing **only** `~/.agents/skills` is correct for **this host’s effective slash skills**. It is **wrong** as a claim that skills are “off-branch only” or that product skill work has nothing to do with this git tree (loader + docs + optional project skills + bundle cache writer **are** on the branch).

---

## 2. Ranked load / discovery order (with evidence)

### 2.1 Scope enum (bare-name priority)

Source:  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-tools/src/implementations/skills/types.rs` **L20–33**

| Rank | `SkillScope` | `repr` | Meaning |
|------|--------------|--------|---------|
| 1 highest | `Local` | 0 | Config dirs whose **parent is cwd** |
| 2 | `Repo` | 1 | Other dirs under git worktree |
| 3 | `User` | 2 | Home-level trees; many `[skills].paths` outside repo |
| 4 | `Server` | 3 | Launcher/workspace-injected server skill dirs |
| 5 | `Bundled` | 4 | Platform pack under `<grok_home>/bundled` (+ injected bundled dirs) |
| 6 lowest bare-name | `Plugin` | 5 | Plugin skills; bare name loses to native; qualified `plugin:name` kept |

Cross-scope: higher scope shadows lower (after sort-by-scope + name dedup). Same-scope name collisions can re-key to directory basename (`dedupe_skills` in `skills.rs`).

### 2.2 Config dir **names** at the same tier

Source:  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-tools/src/types/compat.rs` **`skill_config_dirs` L360–378**, tests `skill_config_dirs_agents_before_grok` **L514+**

```text
.agents → .grok → [.claude if compat] → [.cursor if compat]
```

Always-on: `.agents` then `.grok`. Order is **load-bearing** (first-seen-wins within a scope tier).

Under each config dir:

| Layout | Function | Evidence |
|--------|----------|----------|
| `<config_dir>/skills/**/SKILL.md` | `find_skill_paths` → `walk_for_skill_md` (depth ≤ 5) | `discovery.rs` **L68–78**, **L122–146** |
| `<config_dir>/commands/*.md` | `find_command_paths` (flat) | `discovery.rs` **L80–83** |
| Skills collected **before** commands | skills win name collisions | `skills.rs` **L316–328** |

### 2.3 Full pipeline (`list_skills_with_plugins`)

Primary orchestration:  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/prompt/skills.rs`

| Step | What | Symbols / lines |
|------|------|-----------------|
| 0 | Module docs: priority Local → Repo → User → paths → Server → Bundled; `.agents` before `.grok` | **L49–59** |
| 1 | Walk **cwd → git root**; for each dir, config names via `compat.skill_config_dirs()`; scope Local if parent==cwd else Repo | `collect_skill_config_dirs` **L179–197**, `scope_for_config_dir` **L241–264**, walk in `list_skills_with_options` **L307–329** |
| 2 | Optional **workspace-user** dir (`optional_workspace_user_dir`) | **L90–97**, **L199–204** |
| 3 | **User / home:** `$HOME/.agents` then `grok_home` (`~/.grok`) then vendor homes if enabled | **L206–222** |
| 4 | Always scan **`<grok_home>/bundled/skills`** as `Bundled` | `list_skills_with_options` **L331–337** |
| 5 | **`SkillsConfig.paths`** — direct `SKILL.md` or dir walk; scope Repo if under git root else User; stamp `ConfigToml` | `collect_config_skills` **L352–397**; called from `list_skills_with_plugins` **L106** |
| 6 | Injected **`server_skill_dirs`** → `Server` | **L108–110**, `collect_injected_skills` **L399–414** |
| 7 | Injected **`bundled_skill_dirs`** → `Bundled` | **L111–114** |
| 8 | **`[skills].ignore`** path-prefix hide | **L116** `filter_skills` |
| 9 | **Sort by `SkillScope`**, then merge | **L117** |
| 10 | **Plugins** append; native bare names win; qualified kept | `merge_skills_with_plugins` **L119–125**, **L608+** |
| 11 | **`[skills].disabled`** → `enabled = false` (still listed) | **L130–137** |

**Workspace env inject (product):**  
`xai-grok-workspace/src/handle.rs` ~**L3927–3935**  
- `GROK_WORKSPACE_SERVER_SKILLS_DIR` → `server_skill_dirs`  
- `GROK_WORKSPACE_BUNDLED_SKILLS_DIR` (+ optional `GROK_WORKSPACE_BUNDLED_SKILLS_ALLOWLIST`)

**Tests (shadow rules):**

| Test | File | Proves |
|------|------|--------|
| `agents_home_skills_shadow_grok_user_skills` | `skills.rs` **L2134+** | `~/.agents` beats grok user skills |
| `user_skills_shadow_bundled_skills` | `skills.rs` **L2203+** | User beats bundled |
| `bundled_skills_are_discovered` | `skills.rs` **L2106+** | Bundled under `home/bundled/skills` loads |
| `server_skill_beats_bundled` | `skills.rs` **L807+** | Server > Bundled |

**Special cases**

- No working directory → no local/repo walk (user-oriented path only) — module docs **L64–65**.
- Vendor default **names** under `/.cursor/` or `/.claude/` denylisted — `discovery.rs` **L33–66**.
- Discovery does **not** consult `.gitignore` — `skills.rs` **L269–274**. Hide via `[skills].ignore`.
- Max walk depth 5 — `MAX_SKILL_WALK_DEPTH` `discovery.rs` **L19**.

### 2.4 Diagram

```text
  higher bare-name priority
  ─────────────────────────────────────────────
  Local/Repo walk (cwd→git root):
      .agents/skills  then  .grok/skills
      (+ .claude/.cursor if compat)
  User:
      ~/.agents/skills  then  ~/.grok/skills
      (+ vendor homes if enabled)
  [skills].paths  (Repo if in git root else User)
  Server          (injected / managed)
  Bundled         ~/.grok/bundled/skills   ← network archive cache
  Plugin          (bare loses to native)
  ─────────────────────────────────────────────
  lower bare-name priority
```

---

## 3. What is ON the grok-oss git branch (product-bundled)

### 3.1 Product machinery (committed)

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-agent/src/prompt/skills.rs` | `list_skills*`, `SkillsConfig`, collect dirs, inject, dedupe, plugin merge |
| `crates/codegen/xai-grok-tools/src/implementations/skills/**` | Parse `SKILL.md`, walk, vendor denylist, `SkillScope` |
| `crates/codegen/xai-grok-tools/src/types/compat.rs` | `.agents` before `.grok`; vendor skill toggles |
| `crates/codegen/xai-grok-tools/src/reminders/skill_discovery.rs` | Mid-session path touch → skill remind |
| `crates/codegen/xai-grok-workspace/src/discovery.rs` | Workspace RPC → agent `list_skills` |
| `crates/codegen/xai-grok-workspace/src/handle.rs` | Env inject server/bundled skill dirs |
| `crates/codegen/xai-grok-shell/src/bundle.rs` | Write `SubagentBundle` / archive into `~/.grok/bundled` (skills/agents/personas/roles) |
| `crates/codegen/xai-grok-shell/src/extensions/bundle.rs` | ACP sync/status; TTL; `fetch_bundle` |
| `crates/codegen/xai-grok-shell/src/remote/client.rs` | `GET …/subagents/bundle` (archive + legacy JSON) |
| `prod/mc/cli-chat-proxy-types/src/subagent_bundle.rs` | `SubagentBundle { skills: HashMap }` wire type |
| `crates/codegen/xai-grok-plugin-marketplace/**` | Plugin `skills/` scan (`scanner.rs`) |
| `crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md` | User-facing locations / bundled rules |
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | Subagent / token-efficiency **product policy** (not skill pack bodies) |
| `AGENTS.md`, `docs/upstream-history.md` | Project process pins (not skill packs) |

**Project skill roots supported by product (empty of packs on this checkout):**

- `./.agents/skills/`, `./.grok/skills/` (and intermediate path variants between cwd and git root)
- `.agents/` exists with **`plans/` only** — no `.agents/skills/`
- No `.grok/` project dir
- Shell `build.rs` bundles **ripgrep**, not skill packs
- **No** packaging/flake entries shipping skill pack trees into the binary

### 3.2 What “product-bundled skills” means in practice

**Not** “skill packs checked into this monorepo.”  
**Yes** “platform skill packs delivered at runtime”:

1. Product fetches bundle from **cli-chat-proxy** `GET /v1/subagents/bundle` (archive preferred; legacy JSON fallback) — `remote/client.rs` **L86–144**, extensions `bundle.rs` auth gate references same endpoint.
2. Writes/updates cache under **`~/.grok/bundled/{skills,agents,personas,roles}`** — `shell/src/bundle.rs` `bundled_root` **L86–88**, `write_bundle_to_cache` **L106+**.
3. Discovery always scans that cache as `SkillScope::Bundled` — `skills.rs` **L331–337**.
4. Docs: never write bundled into `~/.grok/skills/`; same-named local/repo/user overrides — `08-skills.md` ~**L202–204**.

**Observed host cache (2026-07-24 sample):**  
`/home/hunter/.grok/bundled/skills/` includes e.g. `implement`, `pr-babysit`, `execute-plan`, `review`, `create-skill`, office/game skills, `resume-*`, etc. Authoritative **version** lives in cache `manifest.json` (product-managed checksums).

**Bundle archive provenance (who authors the remote payload)** is **outside** this repo’s tree; product only installs/syncs.

---

## 4. Host operator (`~/.agents`, `~/.grok`) and how it relates

| Path | Role | Branch? |
|------|------|---------|
| `/home/hunter/.agents/skills/**` | **Maintained operator skill home** (own git). Zed/Surmount first; Grok loads **first** at User tier. Rules: `_SKILL_RULES-read-first-pls.md` | Host only |
| `/home/hunter/.grok/skills/**` | Sparse Grok user skills; lag/mirror; **shadowed** by same-named agents skills | Host only |
| `/home/hunter/.grok/bundled/skills/**` | Platform pack **cache** (product sync) | Host install path; content from network |
| `/home/hunter/.grok/bundled/{agents,personas,roles}/**` | Sibling bundled catalog | Host |
| `/home/hunter/.grok/docs/user-guide/**` | Install mirror of pager user-guide | Host install; **source** is in-repo pager |
| `/home/hunter/.grok/config.toml` `[skills]` | `paths` / `ignore` / `disabled` / compat | Host |
| `/home/hunter/.claude/skills`, `~/.cursor/skills` | Vendor compat (gated) | Host |
| Plugin install roots | Marketplace content | Host; discovery on branch |
| Server skill store dirs | Managed/workspace | Host/remote |

**Operator maintenance law** (`~/.agents/skills/_SKILL_RULES-read-first-pls.md` **L11–15**, **L86–103**):

1. `~/.agents/skills` is maintained home (git).
2. On maintenance, compare **`~/.grok/skills` and `~/.grok/bundled/skills`**.
3. Every bundled skill with `SKILL.md` should exist under agents as a **real copy** (rsync; no symlinks).
4. Runtime: agents win by discovery order; pull peer improvements **into** agents via `/skill-maintenance`.

**Agents-only (not in bundled sample as primary):** e.g. `check-work`, `help`, `skill-maintenance`, `plan`, `upstream-export-import`, `zed-settings`, `grok-tool-policy`, `xlsx` (and shared references/personas under agents).

---

## 5. Reconciliation / export with upstream xAI

| Channel | Skills relationship |
|---------|---------------------|
| **Import** (`scripts/import-upstream-export.sh`) | Absorbs xAI **monorepo tree** into Surmount `import/*`. Brings/updates **product skill machinery** (agent/tools/shell/docs crates) when present in export — **not** a dedicated Surmount skill-pack tree. |
| **Onto / put-history** (`put-history-on-xai.sh`, join scripts) | Replays Surmount product commits (including discovery/bundle code) onto xAI tip. Same: **code + docs**, not `~/.agents` bodies. |
| **`scripts/detect-upstream-export.sh` / sync** | Detects new export tips; no special-case skill-pack export. |
| **Network `SubagentBundle`** | Runtime platform skills from **cli-chat-proxy** `/v1/subagents/bundle` — parallel to git import; not written into grok-build’s git skill packs. |
| **Host skill-maintenance** | Reconciles `~/.agents` ↔ `~/.grok/skills` ↔ `~/.grok/bundled/skills` on the **operator machine**; not an xAI git export path. |
| **FORK.md** | No skills mentions (grep empty). Skills residual not tracked there today. |
| **Project skill git** | Product **supports** committing `.agents/skills` or `.grok/skills` on the branch for team share; **this checkout does not**. |

There is **no** in-repo pipeline that exports Surmount `~/.agents/skills` back to xAI or into the remote bundle authoring system.

---

## 6. If skills ship with product — which **in-repo** paths must get process pins

Interpret “ship with product” as: every install sees the pin without Hunter’s home skill git.

| Must pin on branch | Why |
|--------------------|-----|
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` § *Token efficiency* | Shipped user-guide; AGENTS + skills link here |
| `crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md` | Skill discovery UX; keep aligned with code (`.agents` first, bundled path, no gitignore filter) |
| Project `AGENTS.md` | Repo process for work **in this tree** (Hard stop, never commit) |
| `docs/upstream-history.md` (onto / multi-file conflict) | Branch workflow that triggers skill-heavy ops |
| Optional: **project** `.agents/skills/**` or `.grok/skills/**` if Surmount chooses to **version** skill packs on the product branch | Only way skill **bodies** ride the git branch for collaborators |
| Optional: fix stale `crates/codegen/xai-grok-shell/README.md` Skills section | Currently wrong vs code (omits `.agents`/bundled; claims gitignore filter) — **L1535–1546** area |

| Host / cache (not durable “product ship” alone) | Why |
|--------------------------------------------------|-----|
| `~/.agents/skills/**` orchestrators + `_SKILL_RULES` + `shared/references/subagent-token-strategy.md` | This machine’s effective skill bodies; not the branch pack tree |
| `~/.grok/bundled/skills/**` | Platform defaults for users without agents overrides — but **cache**; durable change needs **bundle source pipeline**, not only local cache edit |
| `~/.grok/skills/**` alone | Sparse/stale; loses to agents |

**If Hard stop must ship as platform skill default:** change must land in **remote bundle authoring** (then sync → bundled), **and/or** committed project skills on the branch — not “edit only `~/.agents`” for all users.

---

## 7. Docs honesty

| Doc | Status vs code |
|-----|----------------|
| `08-skills.md` | Good multi-source story + bundled under `~/.grok/bundled/skills/`; table still heavier on `.grok` than code’s “`.agents` first” (prose has both) |
| `xai-grok-shell/README.md` Skills | **Stale**: only `.grok` / `~/.claude`; claims repo skills respect `.gitignore` (**false** per `skills.rs` L269–274); omits `.agents` first and bundled |
| `FORK.md` | No skills residual |
| Earlier research `skill-subagent-pin-inventory-2026-07-24.md` | Correct that orchestrator **bodies** are host-dir; phrasing “not on branch” **understates** product loader + project roots + bundle distribution — corrected by **this** note |

---

## 8. Evidence index (primary)

| Claim | Evidence |
|-------|----------|
| Priority + `.agents` before `.grok` | `xai-grok-agent/.../prompt/skills.rs` **L49–59**, **L206–214**, **L331–337** |
| Scope enum order | `xai-grok-tools/.../skills/types.rs` **L20–33** |
| `skill_config_dirs` | `xai-grok-tools/.../compat.rs` **L360–378**, tests **L514+** |
| `find_skill_paths` | `xai-grok-tools/.../skills/discovery.rs` **L68–78** |
| Bundled cache root | `shell/src/bundle.rs` `bundled_root` **L86–88** |
| Network fetch | `shell/src/remote/client.rs` **L86–144**; `SubagentBundle` in `prod/mc/cli-chat-proxy-types/.../subagent_bundle.rs` |
| Agents shadow grok user | test `agents_home_skills_shadow_grok_user_skills` **L2134+** |
| User shadows bundled | test `user_skills_shadow_bundled_skills` **L2203+** |
| No in-repo SKILL packs | workspace search: no product `SKILL.md` packs under grok-build |
| Operator rules | `~/.agents/skills/_SKILL_RULES-read-first-pls.md` **L11–15**, **L86–103** |
| User-facing bundled rule | `08-skills.md` ~**L202–204** |

---

## 9. Residual honesty

1. **Shell README** skill table is outdated relative to code and `08-skills.md`.
2. **Bundle archive authorship** is outside this repo; only install/sync code is on-branch.
3. Inventory notes that said “skills are only home-dir / not on branch” correctly locate **operator bodies**, but **misstate product ownership** of discovery + project roots + bundled distribution — that is the claim this note rejects.

---

## 10. Answer table (required)

| Question | Answer |
|----------|--------|
| **1. Ranked load order** | Local → Repo (path walk; `.agents` then `.grok` [+vendors]) → User (`~/.agents` then `~/.grok` [+vendors]) → `[skills].paths` → Server → Bundled (`~/.grok/bundled`) → Plugin; name shadow by `SkillScope` + first-seen. Evidence: `skills.rs` + `types.rs` + `compat.rs` as above. |
| **2. ON grok-oss branch** | Discovery/load/bundle-sync/plugin/workspace **code** + user-guide; **no** committed skill packs today; project `.agents`/`.grok` skills **supported** if added. |
| **3. Host operator** | `~/.agents/skills` maintained bodies (win User tier); `~/.grok/skills` lag; `~/.grok/bundled` product cache; skill-maintenance reconciles peers. |
| **4. Wrong claim fix** | Skills are multi-source; branch owns machinery + can hold project skills; host agents is maintained skill **bodies** for this operator, not “the only place skills exist.” |
| **5. In-repo pin paths if skills ship with product** | User-guide `08`/`16`, project `AGENTS.md`, onto docs; optionally commit project `.agents/skills` or fix shell README; platform skill **bodies** need bundle pipeline or project packs — not only host agents. |
| **Upstream reconciliation** | Git import/onto move **code**; bundle is network; host skill-maintenance is operator-only; no skill-pack export path to xAI in this repo. |
