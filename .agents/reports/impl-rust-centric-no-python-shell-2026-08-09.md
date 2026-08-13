# Report: Rust-centric skills / no invent-and-run Python or shell

**Date:** 2026-08-09
**Scope:** Process law audit + same-turn D1 pins (host + product + skill-rules).
**No product Rust code. No git add/commit/push.**

## Audit of existing docs (paths + one-line each)

| Path | What was there (before this turn) |
|------|-----------------------------------|
| `/home/hunter/.grok/AGENTS.md` § *Prefer Rust tools over inventing Python/bash* | Short “prefer product/host Rust bins over ad-hoc python3/bash”; pointed at D2; office/pdf + user-project as exceptions. **Soft prefer, no supply-chain ban, no allowed/forbidden table.** |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` hard constraint **6** | One bullet: prefer Rust when a tool already covers the job; link D2. **Soft prefer only.** |
| `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md` rule **17** | Prefer Rust bins for hot skill/hook paths; no new Python for memory/bulk-edit/plan-validate/resume if Rust exists; office exception. **Skill-author scope; not a general agent invent ban.** |
| Same file § run-id / token table | Explicit: no `python3 -c uuid`, no Bash random for run ids (host-minted). **Narrow identity rule, not general.** |
| `doc/dev/research/python-to-rust-tools-2026-07-26.md` | **Thorough D2 migration brief:** inventory, P0–P3 rewrite order, embed+intercept (A1–A4 done), non-goals (do not ban user-project Python), pin map. Architecture: intercept known skill paths into Rust. |
| `FORK.md` prefer-Rust checkbox | Campaign checklist: A1–A4 + skill demotion done; remaining py = allowlisted intercept + office/PDF. **Product progress note, not hard agent ban.** FORK was mid-touch (mtime advancing) this session; **not edited here.** |
| `shared/personas/implementer.md` | No Rust/Python invent rule before this turn. |
| Office/pdf skills (`docx`, `pptx`, `xlsx`, `pdf`) | Intentionally teach **pre-shipped** `python scripts/…` and some `pip install` for ecosystem tools. **Exception class, not agent invent.** |
| `implement` / `execute-plan` skills | Teach `python3 …/memory.py` allowlisted form (Grok intercepts to Rust). **Do not invent alternate scripts** already stated in places. Still heavy Python CLI surface in body text. |
| `resume-session` / `session_reader.py` | Allowlisted `python3 …/session_reader.py` + intercept note. |
| `execute-plan/scripts/validate-plan.py` | Host dual-pin helper; product `plan_validate` intercept. |
| Host hooks `~/.grok/hooks/block-bulk-replace-edit.py` (+ `.sh`) | Pre-reviewed host hook dual-pin; product bulk-edit is in-process Rust. |
| `zed-settings` skill | Explicitly **do not** shell `python3` for JSON when `jq` exists. Good. |
| `build-with-ai` skill | Example `from openai import OpenAI  # pip install openai` — teaches user-facing sample code, residual if used as agent glue pattern. |
| `pdf/recipes.md` | Comment `# pip install pytesseract pdf2image` — office/pdf exception surface. |

## Gap assessment (thorough or not)

**Honest answer: partially documented, not thoroughly as a hard process ban.**

What was strong:

- D2 migration research is solid (what to port, what stayed Python, intercept design).
- Skill-rules had a hot-path “no new Python” for known helpers.
- Product already embeds/intercepts several former Python hot paths (memory, bulk-edit, plan-validate, session_reader).
- Run-id path bans Python/Bash identity minting.

What was weak / missing for the operator’s supply-chain concern:

1. Language was **prefer**, not **must not invent and execute new scripts**.
2. **No supply-chain rationale** (Python ecosystem attacks) in D1 law.
3. **No clear distinction** between shell-for-named-product-commands vs writing a new `.py`/`.sh` payload.
4. **No allowed/forbidden table** agents can apply mid-task without reading the whole D2 novel.
5. Implementer persona silent → L2 implementers could still improvise `python3 -c` / `/tmp/foo.py`.
6. FORK checkbox tracks **port campaign**, not the **agent invent ban**.

**After this turn:** D1 dual-pin + skill-rules + implementer persona cover the ban with tables and exceptions. D2 brief has a short pointer section. FORK still needs a one-line pointer when no other agent is editing it.

## Pins added/changed

| File | Change |
|------|--------|
| `/home/hunter/.grok/AGENTS.md` | Replaced soft § with **Prefer Rust tools; do not invent and run new Python/shell scripts (pinned 2026-08-09)**: why (supply chain), allowed/forbidden table, four narrow exceptions, shell≠script distinction, dual-pin links. |
| `AGENTS.md` (project) hard constraint **6** | Same ban, compact; dual-pin host + skill-rules + D2. |
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` rule **17** | Expanded to full invent ban + supply chain + shell vs script + dual-pin. |
| `~/.agents/skills/shared/personas/implementer.md` | One mandatory bullet under Rules. |
| `doc/dev/research/python-to-rust-tools-2026-07-26.md` | New short § *Agent invent ban* pointing at D1 law (migration brief stays inventory SoT). |
| `FORK.md` | **Skipped** — mtime still advancing during this work; parent said do not race. Residual below. |

## Exceptions spelled clearly

| Exception | Meaning |
|-----------|---------|
| Pre-reviewed office/docx/pptx/xlsx/pdf skill scripts | May run **shipped** skill `scripts/*.py`; not agent-written one-offs. |
| Allowlisted host helpers (`memory.py`, plan-validate, session_reader CLI forms) | Keep documented `python3 …/known-script.py` shape; product may intercept to Rust; **do not invent replacements**. |
| User-project Python | When the **user’s product** is Python (their tests/app), agents may work in that codebase. |
| Existing repo `just` / `scripts/` / cargo | Run maintained project recipes and named tools. |
| Shell one-liners that only call existing binaries | `cargo test`, `just check`, `rg`, read-only git — not “writing a script.” |

**Forbidden by default:** write new `.py`/throwaway `.sh` (or equivalent heredoc) and execute for agent glue; `pip install` / `uv add` / `curl | sh` as agent improvisation for the task.

## Residual (skills still teaching python invent / surface)

| Item | Severity | Note |
|------|----------|------|
| `implement` / `execute-plan` bodies still lead with `python3 …/memory.py` | Medium (docs noise) | Correct allowlisted surface; product intercepts. Optional later: lead with “native intercept / Rust; CLI form for dual-pin only.” Not rewritten this turn (goal: standing law first). |
| Host `memory.py`, `validate-plan.py`, `session_reader.py`, hook `.py` still on disk | Low | Dual-pin / fallback; P0 ports largely done in product. Deleting host py is a separate migration step. |
| Office/pdf skills + `pip install` notes | Expected exception | Leave unless operator wants tighter install policy (e.g. never pip without ask). |
| `build-with-ai` sample `pip install openai` | Low residual | Sample app code; if agents treat it as “write python glue,” skill-rules ban still applies to agent-invented scripts. |
| FORK prefer-Rust checkbox soft language | Process mop | Add one sentence: agent invent ban + link host AGENTS §; do when FORK is free. |
| No mechanical PreToolUse deny for `python3 -c` / writing `/tmp/*.py` | Product residual | Law is prose today. Optional later: host hook deny for invent patterns (careful not to break allowlisted skill scripts and user-project Python). |

## What next session should read first

1. Host `~/.grok/AGENTS.md` § **Prefer Rust tools; do not invent and run new Python/shell scripts** (canonical table).
2. Project `AGENTS.md` hard constraint **6** (branch pin / recon survival).
3. `~/.agents/skills/_SKILL_RULES-read-first-pls.md` rule **17** (skill authors).
4. D2 `doc/dev/research/python-to-rust-tools-2026-07-26.md` only if working on **ports/intercept**, not for the ban itself.
5. When free: one-line FORK checkbox update pointing at the 2026-08-09 ban.

## Operator concern answer (one paragraph)

The **Rust-centric migration** (embed + intercept + inventory) was already well documented in D2 research and FORK campaign checkboxes. The **agent must not invent and execute new Python/shell scripts** rule was only a soft “prefer” until this turn. That gap is closed in host AGENTS, project AGENTS, skill-rules, and the implementer persona, with supply-chain rationale and explicit exceptions. Remaining work is optional skill-body demotion of `python3 memory.py` noise, a FORK one-liner when the file is free, and any mechanical hook if the operator wants enforcement beyond prose.
