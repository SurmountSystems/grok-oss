# Stock Grok Build skills: Python and UUIDs

Date: 2026-08-15  
Workspace: `/home/hunter/Projects/surmount/grok-build`  
Sources (only these three reports; this file does not re-search the tree):

- `/home/hunter/Projects/surmount/grok-build/.agents/reports/ask-stock-skills-roots.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/ask-stock-skills-python.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/ask-stock-skills-uuids.md`

Operator question: "And we're also certain that we are no longer using python or uuids in the stock grok build skills, correct?"

**Partial.** For the in-repo Grok Build stock skill trees, yes: this branch does not ship any `SKILL.md` packs, so those trees cannot be using Python or UUIDs. That is certain on the product tree. It is not a complete "we no longer use Python or UUIDs" if stock is taken to include the last network bundle sitting on this machine. The product still allowlists some skill-tree `.py` paths when it installs a remote archive. The leftover cache under `~/.grok/bundled/skills` still has those allowlisted scripts, one non-excepted leftover test file, office and pdf Python, and leftover skill markdown that still tells the model to run `python3`. UUIDs are absent from in-repo stock skill bodies. Host overlay and cache leftover still contain anti-teach text, one example UUID literal, and parse/validate of foreign or Office IDs. Product Rust outside skills still names session and request UUIDs and rejects a fake `uuid_helper.py` on extract.

---

## Stock roots checked (from the roots report)

This branch ships skill machinery (discovery, bundle install/sync, user-guide). It does not ship skill-pack trees. Platform bodies arrive at runtime from the network bundle cache, or from grok.com REST for chat kind only.

### In-repo product skill roots (supported, empty)

These are the only git-trackable product skill homes the loader treats as project stock. On this checkout they hold zero skill packs.

| Absolute path | On disk | `SKILL.md` parent dirs |
|---------------|---------|------------------------|
| `/home/hunter/Projects/surmount/grok-build/.agents/skills` | Missing. `.agents/` exists (`plans/`, `reports/`, `joins/`) with no `skills/` | none |
| `/home/hunter/Projects/surmount/grok-build/.grok/skills` | Missing. `.grok/` exists with `workflows/git-recon-status.rhai` only | none |
| `/home/hunter/Projects/surmount/grok-build/.agents/commands` | Missing | none (legacy flat `*.md` commands) |
| `/home/hunter/Projects/surmount/grok-build/.grok/commands` | Missing | none |

Repo-wide search for files named `SKILL.md` under `crates/`, `doc/`, and `.agents/` returned no files. No `include_str!` or rust-embed of a skill pack exists.

Crate trees that look like they might hold packs do not: `xai-grok-bundle` is a cache writer plus Python-allowlist tests; pager user-guide `08-skills.md` is docs; `xai-grok-tools` skills code is parse/walk/denylist; agent `prompt/skills.rs` is the load-order orchestrator; agent and shell templates are prompts, not `SKILL.md`; `deep_research.rhai` is a built-in workflow; plugin-marketplace has no shipped plugin skill trees.

**Stock skill names from in-repo `SKILL.md` parent dirs:** none.

Grok Build sessions use disk discovery, not the chat REST catalog. Chat product skills are advertised with `body: None` and a synthetic `chat-product://` path. Those names are not Build stock bodies.

Former in-binary extract into `~/.grok/skills/` is removed. `builtin.rs` only extracts `README.md`. The leftover hash table still names `best-of-n`, `check`, `check-work`, `code-review`, `create-skill`, `create-workflow`, `docx`, `help`, `imagine`, `pptx`, `xlsx`. That is a legacy name list, not a live stock tree.

### Agreement across the three reports on roots

All three reports say the same in-repo roots are missing and that there is no `SKILL.md` in the grok-build workspace.

The Python and UUID reports both say the sibling roots report was not on disk when they ran, and that they re-inventoried roots from the tree. That is a timing note, not a disagreement about the roots. The later roots report and those two inventories still match: `.agents/skills` absent, `.grok/skills` absent, crate trees are machinery not packs, network archive is not vendored.

The UUID report cites `doc/dev/research/where-skills-come-from-2026-07-24.md` as matching (no committed `SKILL.md` packs). The roots report says that same research note is stale on one point: it said there was no `.grok/` project dir, and today `/home/hunter/Projects/surmount/grok-build/.grok/workflows/` exists. Both still agree there is no `.grok/skills` and no committed `SKILL.md` packs.

---

## Python: what exists, intercepts vs runtime, leftover cache

### Verdict from the Python report

**No Python in stock skill bodies.** The grok-build git tree does not ship any product skill tree. In-repo `.agents/skills` and `.grok/skills` are absent. There is no `SKILL.md` anywhere under the workspace. `xai-grok-bundle` is a network-archive cache and sanitizer, not an embedded skill pack. Land class 7 is not violated in the product tree. No file was deleted.

### Allowlist in product code (not in-tree files)

`is_allowed_product_skill_python` in `crates/codegen/xai-grok-bundle/src/lib.rs` may install these relative cache paths from a remote archive:

| Relative cache path | What it implies |
|---------------------|-----------------|
| `skills/implement/scripts/memory.py` | intercept CLI |
| `skills/execute-plan/scripts/validate-plan.py` | intercept CLI |
| `skills/shared/resume-session/session_reader.py` | intercept CLI |
| `skills/docx/**/*.py` | office exception |
| `skills/pptx/**/*.py` | office exception |
| `skills/xlsx/**/*.py` | office exception |
| `skills/pdf/**/*.py` | office exception |

Sanitize drops any other `skills/**/*.py`. Tests reject `skills/review/scripts/build_pending_review.py`, `skills/implement/scripts/uuid_helper.py`, `skills/implement/tests/test_memory.py`, and `skills/create-skill/scripts/scaffold.py`. `create-skill` must not ship a Python scaffold.

The three intercept CLI forms are parsed in Rust (`implement_memory`, `plan_validate`, `session_reader`) and run in-process. That is product intercept, not a skill body, and not a Python runtime for those commands.

### Do the allowlisted files exist in the product tree?

**No. Host overlay and cache only.**

| Allowlisted relative path | Product tree | Host overlay | Bundled cache |
|---------------------------|--------------|--------------|---------------|
| `skills/implement/scripts/memory.py` | absent | `/home/hunter/.agents/skills/implement/scripts/memory.py` | `/home/hunter/.grok/bundled/skills/implement/scripts/memory.py` |
| `skills/execute-plan/scripts/validate-plan.py` | absent | `/home/hunter/.agents/skills/execute-plan/scripts/validate-plan.py` | `/home/hunter/.grok/bundled/skills/execute-plan/scripts/validate-plan.py` |
| `skills/shared/resume-session/session_reader.py` | absent | `/home/hunter/.agents/skills/shared/resume-session/session_reader.py` | `/home/hunter/.grok/bundled/skills/shared/resume-session/session_reader.py` |
| office/docx/pptx/xlsx/pdf `*.py` | absent | host has docx, pptx, pdf, **and xlsx** | cache has docx, pptx, pdf; **no xlsx skill dir** |

Cache copies of the three intercept files are full Python programs (`#!/usr/bin/env python3`), not empty stubs. Grok is supposed to intercept those CLI forms in Rust and not spawn Python. That does not put the `.py` files into this git tree.

### Product-tree `*.py` that are not skill bodies

None sit under a skill root (those roots do not exist):

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/scripts/encrypt_templates.py` (crate helper)
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/tests/memory_integration/run_tests.py` (test harness)
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-hooks/examples/hooks/bin/no-recursive-grep-guard.py` (example hook)

No `pip` installers and no `python3` skill helpers under product skill roots.

### Leftover cache under `~/.grok/bundled/skills`

Not stock source. Install leftover from the last successful network fetch on this machine. Current extract sanitize would refuse non-excepted `.py`. This cache still has one.

**Non-excepted `.py` (sanitize would skip on a fresh extract):**

- `/home/hunter/.grok/bundled/skills/implement/tests/test_memory.py`

That exact path is the named reject case in `sanitize_rejects_non_excepted_skill_python` and `extract_archive_skips_non_excepted_skill_python`. It is a cache leftover, not an in-tree land-class-7 fail.

**Allowlisted intercept `.py` in cache:** the three paths listed above. Cache `implement/SKILL.md` still tells the model to run `python3 "${MEMORY_HELPER}" snapshot|update`. Cache `shared/resume-session/CORE.md` still shows `python3 "${SHARED_DIR}/session_reader.py" …`.

**Allowlisted office/pdf `.py` in cache:** full `docx/scripts/` and `pptx/scripts/` trees (including shared `office/` helpers and validators), plus `pdf/scripts/` form and conversion helpers. There is no `/home/hunter/.grok/bundled/skills/xlsx` directory.

**Cache skill-body Python invocations (markdown, not `.py` files):**

- `/home/hunter/.grok/bundled/skills/review/SKILL.md` still has a `python3 <<'PY'` heredoc to `json.dumps` a GitHub review payload. Host overlay review has already dropped that.
- `/home/hunter/.grok/bundled/skills/pptx/SKILL.md` line 220: `pip install "markitdown[pptx]" Pillow pdf2image python-pptx numpy defusedxml`
- `/home/hunter/.grok/bundled/skills/pdf/SKILL.md` line 92: `# pip install pytesseract pdf2image`
- `/home/hunter/.grok/bundled/skills/build-with-ai/SKILL.md`: `from openai import OpenAI  # pip install openai` (example snippet)
- Office editing docs under cache `docx/editing.md` and `pptx/editing.md` invoke `python scripts/…`

Cache `create-skill` has no `python3` / `.py`. Cache `resume-claude/scripts/` is an empty directory (no `cc_session.py`).

---

## UUIDs: what exists in stock skills

**Verdict from the UUID report: none.** There are no in-tree stock skill packs on this branch, so there is no stock-skill teaching of UUIDs as ids, no UUID literals in stock skill text, and no Python `uuid` module in stock skill scripts.

The stock hits table is empty for:

1. Teaching agents to use UUIDs as ids
2. UUID literals in skill text
3. Python `uuid` module

No product crate ships `import uuid` as a skill script. Allowlisted intercept stubs named in product code are `memory.py`, `validate-plan.py`, and `session_reader.py` only.

### Product code outside skills (class 4, not stock skill bodies)

Outside skill packs, product Rust talks about UUIDs for sessions and requests, and it **rejects** a fake implement helper named `uuid_helper.py` on bundle extract.

| Path | Quote | Class |
|------|--------|-------|
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-bundle/src/lib.rs` | `sanitize_relative_path("skills/implement/scripts/uuid_helper.py")` … `"invented implement helpers are not an allowlisted exception"` | (4) |
| Same file, comment on that test | `"invented uuid helpers"` must not land in the skills archive | (4) |
| `/home/hunter/Projects/surmount/grok-build/prod/mc/cli-chat-proxy-types/src/metadata_types.rs` | `Session id (UUIDv7)` / `Request id for this prompt (uuid v4 we generate per prompt)` | (4) |

That is product machinery, not a stock skill using UUIDs.

---

## What is host overlay only (not stock)

`~/.agents/skills` is the operator overlay. It wins at User tier (`.agents` before `.grok`). It is its own git. It is not this branch.

`~/.grok/skills` is user-owned. The product stopped extracting platform skills there. On this host it has only `upstream-export-import/SKILL.md` (no `.py`).

`~/.grok/bundled/skills` is the product’s network-install cache, not stock source. Version and names follow the last successful network fetch on that machine.

Vendor compat (`.claude` / `.cursor` and project twins), `[skills].paths`, plugins, marketplace installs, and `GROK_WORKSPACE_*` dirs are not in-repo stock.

### Host overlay Python (not stock)

**Non-excepted (would fail land class 7 if they were product skills):**

- `/home/hunter/.agents/skills/resume-claude/scripts/cc_session.py` (not in cache; cache `resume-claude/scripts/` is empty)
- `/home/hunter/.agents/skills/implement/tests/test_memory.py` (same leftover class as cache)

**Allowlisted intercept copies on host:** the three CLI files named above.

**Allowlisted office copies on host:** same docx/pptx/pdf trees as cache, plus the whole **xlsx** skill (absent from this cache), including `recalc.py` and the shared `office/` pack/unpack/validate/helpers/validators tree.

**Host-only skills with no cache counterpart** (markdown/process, not a product pack): `check-work`, `git-recon`, `grok-tool-policy`, `help`, `hierarchically-structured-subagents`, `plan`, `skill-maintenance`, `upstream-export-import`, `xlsx`, `zed-settings`, plus `_SKILL_RULES-read-first-pls.md`. Host `review` and `zed-settings` already say not to shell `python3` heredocs.

### Host overlay UUIDs (not stock)

Overlay does **not** teach minting UUIDs. It teaches the opposite, plus parses foreign session UUIDs and validates Office UUID-shaped IDs.

**Anti-teach (class 1 inverted):**

- `/home/hunter/.agents/skills/implement/SKILL.md`: `do not invent hex, run Python/Bash uuid, or re-mint`
- `/home/hunter/.agents/skills/pr-babysit/SKILL.md`: `Never invent ids or shell out for uuid.` / `# INSTANCE_ID comes from skill envelope run_id — no uuid shell`
- `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md`: `Python uuid` / `` `python3 -c "import uuid…"` `` as **Don't**. That file is overlay-only (no bundled peer). It mentions `import uuid` as forbidden, not a real import.

**One UUID literal in skill text (class 2):**

- `/home/hunter/.agents/skills/pr-babysit/SKILL.md`: `"subagent_id": "019d91b8-21e0-7c41-91a0-2b163d2c5481"` (example JSON for `groups`)

No other `8-4-4-4-12` hex literal in overlay `*.md` / `*.py`.

**Python `uuid` module (class 3):** none. No `import uuid` / `from uuid` under `/home/hunter/.agents/skills`.

**Parse / validate existing UUIDs (not mint; not class 3):**

- Host `session_reader.py`: `UUID_RE` and `record.get("uuid")` for Claude/Codex/Cursor history
- Host `resume-claude/scripts/cc_session.py`: path and `parentUuid` chain (overlay-only script)
- Office validators under host `pptx` (also copied under `docx/` and `xlsx/`): `validate_uuid_ids`
- Office XSDs under host `docx`/`pptx`/`xlsx`: `ST_Guid` / `CT_Guid` / element `guid` (vendor schema types)

`xlsx/` including its validator and `ST_Guid` schemas is overlay-only on this host.

### Cache leftover UUIDs (not stock)

Same implement / pr-babysit teaching and the same example `subagent_id` literal. Same `session_reader.py` UUID regex. Same office `validate_uuid_ids` and `ST_Guid` XSDs under `docx`/`pptx`. Still no `import uuid`. No `_SKILL_RULES`, no `cc_session.py`, no `xlsx`.

---

## Leftovers that are real, not guesses

These were found on disk or named by product tests. They are leftovers or overlays, not in-repo stock source.

1. `/home/hunter/.grok/bundled/skills/implement/tests/test_memory.py` exists. A fresh sanitize would skip it. It is the named reject case in product extract tests.
2. The three allowlisted intercept `.py` files exist on host and in cache as full Python programs. They do not exist in the product git tree.
3. Cache `docx`, `pptx`, and `pdf` still hold large allowlisted `scripts/**/*.py` trees. Cache has no `xlsx` directory. Host overlay does have `xlsx`.
4. Cache skill markdown still tells the model to run `python3` for implement memory and session_reader, still has a review `python3` heredoc, and still mentions `pip install` in pptx, pdf, and build-with-ai skill text.
5. Host-only `cc_session.py` exists. Cache `resume-claude/scripts/` is empty.
6. Host-only `_SKILL_RULES-read-first-pls.md` exists. It has no bundled peer.
7. One example UUID literal exists in host and cache `pr-babysit/SKILL.md`. Overlay and cache teach not to mint UUIDs. There is no `import uuid` in overlay or cache.
8. Product Rust still rejects `skills/implement/scripts/uuid_helper.py` on extract. That file is not reported as present in stock, overlay, or cache.

What is **not** claimed: the remote bundle’s current live skill list is not in this repo. The roots report says former extract hashes plus the Python allowlist are hints, not a guarantee of what `/v1/bundle/archive` returns today. A host cache listing describes this machine’s leftover, not branch stock.

---

## How the three reports sit together

They agree on the product-tree fact that matters: there are no in-repo stock skill packs, therefore no stock-skill Python runtime and no stock-skill UUID teaching.

They do not disagree about that fact. The Python and UUID reports independently walked the same missing roots because the roots report was not on disk yet. The roots report later names the same empty paths.

The only named doc mismatch is the stale research sentence about “no `.grok/` project dir.” That does not change the skills finding. `.grok/workflows/` exists. `.grok/skills` does not.

STATUS: COMPLETE
