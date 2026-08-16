# Stock product skills vs Python

**Verdict: no Python in stock skill bodies.** The grok-build git tree does not ship any product skill tree. In-repo `.agents/skills` and `.grok/skills` are absent. There is no `SKILL.md` anywhere under `/home/hunter/Projects/surmount/grok-build`. Crate `xai-grok-bundle` is a network-archive cache + sanitizer, not an embedded skill pack. The three allowlisted intercept CLI files and the office/pdf scripts do **not** exist in this product tree; they exist only as host overlay and as `~/.grok/bundled/skills` cache leftovers. Land class 7 is not violated in the product tree. No file was deleted.

Sibling roots report `/home/hunter/Projects/surmount/grok-build/.agents/reports/ask-stock-skills-roots.md` was not present. Roots were inventoried from the tree.

## Product-tree skill roots (stock)

| Path | Present? | `*.py` |
|------|----------|--------|
| `/home/hunter/Projects/surmount/grok-build/.agents/skills` | No (directory does not exist) | none |
| `/home/hunter/Projects/surmount/grok-build/.grok/skills` | No (`.grok/` exists; only `workflows/git-recon-status.rhai`) | none |
| Crate-bundled skill files under `crates/codegen/xai-grok-bundle/` | Crate is `Cargo.toml` + `src/lib.rs` only | none |
| `crates/codegen/xai-grok-pager` skill trees | User-guide + Rust only; no `skills/` pack | none |

`rg` for `SKILL.md` under the workspace: no matches.

`xai-grok-bundle` writes installed skills to `<grok home>/bundled` after sanitizing archive paths. It does not `include_str!` or vendor a skill directory.

## Allowlist from product code

Source: `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-bundle/src/lib.rs`

```415:434:/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-bundle/src/lib.rs
/// Skill-tree `.py` the product may install into `~/.grok/bundled/skills`.
///
/// Allowlisted: intercept CLI stubs (`memory.py`, `validate-plan.py`,
/// `session_reader.py`) and pre-reviewed office/docx/pptx/xlsx/pdf scripts.
/// Everything else is a restack regression. Agents must not add new helpers.
fn is_allowed_product_skill_python(relative_path: &str) -> bool {
    const INTERCEPT_CLI: &[&str] = &[
        "skills/implement/scripts/memory.py",
        "skills/execute-plan/scripts/validate-plan.py",
        "skills/shared/resume-session/session_reader.py",
    ];
    if INTERCEPT_CLI.contains(&relative_path) {
        return true;
    }
    let office = relative_path.starts_with("skills/docx/")
        || relative_path.starts_with("skills/pptx/")
        || relative_path.starts_with("skills/xlsx/")
        || relative_path.starts_with("skills/pdf/");
    office && relative_path.ends_with(".py")
}
```

Sanitize drops any other `skills/**/*.py` (`relative_path.ends_with(".py") && !is_allowed_product_skill_python(...)` → `None`).

Named contract tests in the same file:

- `sanitize_rejects_non_excepted_skill_python` rejects `skills/review/scripts/build_pending_review.py`, `skills/implement/scripts/uuid_helper.py`, `skills/implement/tests/test_memory.py`, `skills/create-skill/scripts/scaffold.py`; keeps the three intercept paths plus example office/pdf paths.
- `extract_archive_skips_non_excepted_skill_python` asserts extract writes `memory.py` and does **not** write `implement/tests/test_memory.py` or `review/scripts/build_pending_review.py`.
- `product_repo_skill_roots_have_no_non_excepted_python` walks repo `.agents/skills` and `.grok/skills` only (missing dirs are a no-op).

Pin script `/home/hunter/Projects/surmount/grok-build/scripts/assert-process-pins.sh` uses the same allowlist (`implement/scripts/memory.py`, `execute-plan/scripts/validate-plan.py`, `shared/resume-session/session_reader.py`, plus `docx/*` `pptx/*` `xlsx/*` `pdf/*`).

User-guide `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md` (section "Skills are not a Python runtime") names those three CLI forms and the office/PDF exception. Test `user_guide_skills_are_not_a_python_runtime` in `crates/codegen/xai-grok-pager/src/docs.rs` requires that wording.

Product intercept (Rust, not a skill body): `xai-grok-tools` `implement_memory`, `plan_validate`, `session_reader` parse `python3 …/memory.py`, `validate-plan.py`, `session_reader.py` and run in-process.

## Do the allowlisted files exist in the product tree?

**No. Host / cache only.**

| Allowlisted relative path | Product tree | Host overlay | Bundled cache |
|---------------------------|--------------|--------------|---------------|
| `skills/implement/scripts/memory.py` | absent | `/home/hunter/.agents/skills/implement/scripts/memory.py` | `/home/hunter/.grok/bundled/skills/implement/scripts/memory.py` |
| `skills/execute-plan/scripts/validate-plan.py` | absent | `/home/hunter/.agents/skills/execute-plan/scripts/validate-plan.py` | `/home/hunter/.grok/bundled/skills/execute-plan/scripts/validate-plan.py` |
| `skills/shared/resume-session/session_reader.py` | absent | `/home/hunter/.agents/skills/shared/resume-session/session_reader.py` | `/home/hunter/.grok/bundled/skills/shared/resume-session/session_reader.py` |
| office/docx/pptx/xlsx/pdf `*.py` | absent | host has docx, pptx, pdf, **and xlsx** | cache has docx, pptx, pdf; **no xlsx skill dir** |

Cache copies of the three intercept files are full Python programs (`#!/usr/bin/env python3`), not empty stubs. Grok is supposed to intercept those CLI forms in Rust and not spawn Python. That does not put the `.py` files into this git tree.

## Product-tree `*.py` that are not skill bodies

Workspace `*.py` files (shebang search). None sit under a skill root.

| Absolute path | Role |
|---------------|------|
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/scripts/encrypt_templates.py` | Crate helper to regenerate encrypted prompt Rust; not a skill |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/tests/memory_integration/run_tests.py` | Shell crate test harness; not a skill |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-hooks/examples/hooks/bin/no-recursive-grep-guard.py` | Example hook, not a skill pack |

No `pip` installers and no `python3` skill helpers under product skill roots (those roots do not exist).

## Cache leftovers: `/home/hunter/.grok/bundled/skills`

Not stock source. Install leftover from the network bundle. Current extract sanitize would refuse non-excepted `.py`; this cache still has one.

### Non-excepted `.py` in cache (sanitize would skip on a fresh extract)

- `/home/hunter/.grok/bundled/skills/implement/tests/test_memory.py`

That exact path is the named reject case in `sanitize_rejects_non_excepted_skill_python` and `extract_archive_skips_non_excepted_skill_python`. It is a cache leftover, not an in-tree land-class-7 fail.

### Allowlisted intercept `.py` in cache

- `/home/hunter/.grok/bundled/skills/implement/scripts/memory.py`
- `/home/hunter/.grok/bundled/skills/execute-plan/scripts/validate-plan.py`
- `/home/hunter/.grok/bundled/skills/shared/resume-session/session_reader.py`

Cache `implement/SKILL.md` still tells the model to run `python3 "${MEMORY_HELPER}" snapshot|update`. Cache `shared/resume-session/CORE.md` still shows `python3 "${SHARED_DIR}/session_reader.py" …`.

### Allowlisted office/pdf `.py` in cache

**docx** (`/home/hunter/.grok/bundled/skills/docx/scripts/`):

- `__init__.py`, `accept_changes.py`, `comment.py`, `convert_doc.py`, `delete_sections.py`, `docx_patch.py`, `inspect_doc.py`, `inspect_headers.py`, `inspect_tables.py`, `list_sections.py`, `render_doc.py`, `replace_field.py`, `replace_text.py`
- `office/pack.py`, `office/soffice.py`, `office/unpack.py`, `office/validate.py`
- `office/helpers/__init__.py`, `office/helpers/merge_runs.py`, `office/helpers/simplify_redlines.py`
- `office/validators/__init__.py`, `office/validators/base.py`, `office/validators/docx.py`, `office/validators/pptx.py`, `office/validators/redlining.py`

**pptx** (`/home/hunter/.grok/bundled/skills/pptx/scripts/`):

- `__init__.py`, `add_slide.py`, `check_overlaps.py`, `clean.py`, `delete_slide.py`, `detect_fonts.py`, `inspect_slide.py`, `media_grid.py`, `render_slides.py`, `replace_nth_text.py`, `replace_text.py`, `resize_shape.py`, `search_templates.py`, `thumbnail.py`
- same `office/` tree as docx (`pack.py`, `soffice.py`, `unpack.py`, `validate.py`, helpers, validators)

**pdf** (`/home/hunter/.grok/bundled/skills/pdf/scripts/`):

- `check_bounding_boxes.py`, `check_fillable_fields.py`, `convert_pdf_to_images.py`, `create_validation_image.py`, `extract_form_field_info.py`, `extract_form_structure.py`, `fill_fillable_fields.py`, `fill_pdf_form_with_annotations.py`

**xlsx:** no `/home/hunter/.grok/bundled/skills/xlsx` directory.

### Cache skill-body Python invocations (markdown, not `.py` files)

These are leftover bundled skill text, not product-tree files:

- `/home/hunter/.grok/bundled/skills/review/SKILL.md` still has a `python3 <<'PY'` heredoc to `json.dumps` a GitHub review payload. Host overlay review has already dropped that (`Do **not** shell python3 / heredocs`).
- `/home/hunter/.grok/bundled/skills/pptx/SKILL.md` line 220: `pip install "markitdown[pptx]" Pillow pdf2image python-pptx numpy defusedxml`
- `/home/hunter/.grok/bundled/skills/pdf/SKILL.md` line 92: `# pip install pytesseract pdf2image`
- `/home/hunter/.grok/bundled/skills/build-with-ai/SKILL.md`: `from openai import OpenAI  # pip install openai` (example snippet)
- Office editing docs under cache `docx/editing.md` and `pptx/editing.md` invoke `python scripts/…`

`create-skill` cache skill: no `python3` / `.py`. `resume-claude/scripts/` in cache is an empty directory (no `cc_session.py`).

## Host overlay only (`~/.agents/skills`)

Not stock. Operator overlay wins at User tier. Extra vs product tree and vs this cache:

**Non-excepted (would fail land class 7 if they were product skills):**

- `/home/hunter/.agents/skills/resume-claude/scripts/cc_session.py` (not in cache; cache `resume-claude/scripts/` is empty)
- `/home/hunter/.agents/skills/implement/tests/test_memory.py` (same leftover class as cache)

**Allowlisted intercept copies (host, not product tree):** the three CLI files listed above.

**Allowlisted office copies (host):** same docx/pptx/pdf trees as cache, plus the whole **xlsx** skill (absent from this cache):

- `/home/hunter/.agents/skills/xlsx/scripts/recalc.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/pack.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/soffice.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/unpack.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validate.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/helpers/__init__.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/helpers/merge_runs.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/helpers/simplify_redlines.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validators/__init__.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validators/base.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validators/docx.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validators/pptx.py`
- `/home/hunter/.agents/skills/xlsx/scripts/office/validators/redlining.py`

**Host-only skills with no cache counterpart** (markdown/process, not a product pack): `check-work`, `git-recon`, `grok-tool-policy`, `help`, `hierarchically-structured-subagents`, `plan`, `skill-maintenance`, `upstream-export-import`, `xlsx`, `zed-settings`, plus `_SKILL_RULES-read-first-pls.md`. Host `review` and `zed-settings` already say not to shell `python3` heredocs.

User dir `/home/hunter/.grok/skills/` has only `upstream-export-import/SKILL.md` (no `.py`).

## What this is not

- Not "allowlisted intercept stubs exist" **in the product tree**. They exist on host and in cache only.
- Not "non-excepted Python still present" **in the product tree**. The only non-excepted skill `.py` found is cache leftover `implement/tests/test_memory.py` (and host-only `cc_session.py` / host `test_memory.py`).
- Did not invent or run Python. Did not edit product files. Cache leftover was reported, not deleted (not a product-tree ship).

STATUS: COMPLETE
