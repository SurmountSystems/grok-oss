# Stock product skills: UUID usage

**Verdict: none.** There are no in-tree stock skill packs on this branch, so there is no stock-skill teaching of UUIDs as ids, no UUID literals in stock skill text, and no Python `uuid` module in stock skill scripts. Sibling inventory `/home/hunter/Projects/surmount/grok-build/.agents/reports/ask-stock-skills-roots.md` was not on disk; this search re-inventoried roots.

## Roots searched (stock)

| Path | Present? |
|------|----------|
| `/home/hunter/Projects/surmount/grok-build/.agents/skills` | No (directory does not exist) |
| `/home/hunter/Projects/surmount/grok-build/.grok/skills` | No (`.grok/` has only `workflows/`) |
| `**/SKILL.md` under the grok-build workspace | Zero files |
| Crate-bundled skill *bodies* (`xai-grok-bundle`, pager, codegen) | Loader/cache/tests only; no shipped `SKILL.md` trees |
| Network bundle *source* in this git tree | Not present. Skills arrive via network archive into host cache |

Product `xai-grok-bundle` writes platform skills to `~/.grok/bundled/skills`. That cache is leftover only (not stock source). Host overlay `~/.agents/skills` is not stock.

Research pin that matches this tree: `doc/dev/research/where-skills-come-from-2026-07-24.md` (no committed `SKILL.md` packs under grok-build).

## Stock hits

None. Classification table is empty for (1) teaching agents to use UUIDs as ids, (2) UUID literals in skill text, and (3) Python `uuid` module, because there is no stock skill tree to hit.

## Product code outside skills (class 4, one sentence plus the named hits)

Outside skill packs, product Rust talks about UUIDs for sessions/requests and **rejects** a fake implement helper named `uuid_helper.py` on bundle extract.

| Path | Quote | Class |
|------|--------|-------|
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-bundle/src/lib.rs` | `sanitize_relative_path("skills/implement/scripts/uuid_helper.py")` … `"invented implement helpers are not an allowlisted exception"` | (4) |
| Same file, comment on that test | `"invented uuid helpers"` must not land in the skills archive | (4) |
| `/home/hunter/Projects/surmount/grok-build/prod/mc/cli-chat-proxy-types/src/metadata_types.rs` | `Session id (UUIDv7)` / `Request id for this prompt (uuid v4 we generate per prompt)` | (4) |

No product crate ships `import uuid` as a skill script. Allowlisted intercept stubs are `memory.py`, `validate-plan.py`, `session_reader.py` only.

## Host overlay only (`~/.agents/skills`)

Not stock. Effective slash skills on this machine. Overlay **does not teach minting UUIDs**; it teaches the opposite, plus parses foreign session UUIDs and validates Office UUID-shaped IDs.

### Teaching: do **not** mint UUIDs as skill ids (class 1 inverted)

These tell the agent to use host `run_id`, not Python/Bash uuid:

| Path | Quote | Class |
|------|--------|-------|
| `/home/hunter/.agents/skills/implement/SKILL.md` | `do not invent hex, run Python/Bash uuid, or re-mint` | (1) anti-teach |
| `/home/hunter/.agents/skills/pr-babysit/SKILL.md` | `Never invent ids or shell out for uuid.` / `# INSTANCE_ID comes from skill envelope run_id — no uuid shell` | (1) anti-teach |
| `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md` | `Python uuid` / `` `python3 -c "import uuid…"` `` as **Don't** | (1) anti-teach; mentions `import uuid` as forbidden, not a real import |

`_SKILL_RULES-read-first-pls.md` is overlay-only (no bundled peer).

### UUID literal in skill text (class 2)

| Path | Quote | Class |
|------|--------|-------|
| `/home/hunter/.agents/skills/pr-babysit/SKILL.md` | `"subagent_id": "019d91b8-21e0-7c41-91a0-2b163d2c5481"` (example JSON for `groups`) | (2) |

No other `8-4-4-4-12` hex literal in overlay `*.md` / `*.py`.

### Python `uuid` module (class 3)

**None.** No `import uuid` / `from uuid` under `/home/hunter/.agents/skills`.

### Parse / validate existing UUIDs (not mint; not class 3)

| Path | Quote | Class |
|------|--------|-------|
| `/home/hunter/.agents/skills/shared/resume-session/session_reader.py` | `UUID_RE = re.compile(r"^[0-9a-fA-F]{8}-…")` and `record.get("uuid")` for Claude/Codex/Cursor history | parse foreign ids; no `uuid` module |
| `/home/hunter/.agents/skills/resume-claude/scripts/cc_session.py` | `path = <config>/projects/<slug>/<uuid>.jsonl` / `parentUuid` chain | overlay-only script (bundled `resume-claude/scripts/` is empty) |
| `/home/hunter/.agents/skills/pptx/scripts/office/validators/pptx.py` (also copied under `docx/` and `xlsx/`) | `validate_uuid_ids` / `appears to be a UUID but contains invalid hex` | Office ID hex check |
| Office XSDs under `docx`/`pptx`/`xlsx` `scripts/office/schemas/` | `ST_Guid` / `CT_Guid` / element `guid` | vendor schema types, not agent teaching |

`xlsx/` (including its validator and `ST_Guid` schemas) is **overlay-only** on this host; `~/.grok/bundled/skills/xlsx` does not exist.

`guide` / `guidelines` / `guidance` matches were ignored (not UUID/GUID).

## Cache leftover only (`~/.grok/bundled/skills`)

Not stock source. Same implement / pr-babysit teaching and the same example `subagent_id` literal; same `session_reader.py` UUID regex; same office `validate_uuid_ids` and `ST_Guid` XSDs under `docx`/`pptx`. Still **no** `import uuid`. No `_SKILL_RULES`, no `cc_session.py`, no `xlsx`.

## Classification key

1. Teaching agents to use UUIDs as ids  
2. Actual UUID literals in skill text  
3. Python `uuid` module  
4. Product code outside skills  

Stock = **none**. Overlay + cache leftover = anti-teach (1 inverted) + one example literal (2) + parse/validate (not 3). Product crate = (4) only.
