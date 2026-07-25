# Plan approve / reject / continue — flush pending chat/comment first

Date: 2026-07-24  
Scope: pager UX where a decision button (approve / reject / continue / send)
runs while the composer holds an unsaved draft (line comment, freeform
feedback, freeform “Other”, followup, voice interim).

## Pattern (what “flush first” means)

Before the decisive action closes the surface or restores a stashed prompt:

1. Snapshot any in-progress composer text into the durable structure the
   action is supposed to send (comment list, freeform slot, followup meta,
   main prompt), **or** route that text into the action payload.
2. Only then run approve / reject / revise / submit / restore-stash.

Discard-on-leave is a different rule (Tab away, click list while Commenting)
and is intentional in several places.

## Helpers (reusable / closest)

| Name | File | Role |
|------|------|------|
| `swap_question_freeform` | `crates/codegen/xai-grok-pager/src/app/agent_view/interactions.rs` | **Flush model (good).** Writes current prompt into `per_question_freeform[active]` before tab change / submit. |
| `load_question_freeform` | same | Pair of the above after tab change. |
| `submit_question_answers` | same | Always calls `swap_question_freeform()` first — **fixed** flush-before-submit. |
| `save_plan_comment` | `…/agent_view/plan.rs` | Commits **Commenting** draft into `pav.comments` (Enter only). |
| `save_casual_plan_comment` | same | Casual equivalent. |
| `discard_in_progress_comment` | same | **Discard**, not flush (Tab leave Commenting). |
| `cancel_casual_plan_commenting` | same | Restore `casual_stashed_prompt`; drop draft. |
| `format_feedback` / `format_plan_comments` | `…/views/plan_approval_view.rs` | Build feedback strings from **saved** comments (+ optional freeform). |
| `commit_interim_into_prompt` | `…/voice/handle.rs` | Promote voice interim into bound prompt. |
| `maybe_commit_voice_interim_before_submit_key` | `…/app/app_view.rs` | Flush interim on real send / interject keys. |
| `voice_stop_on_submit` / `merge_prompt_with_voice_interim` | `…/dispatch/voice.rs` (+ callers in `prompt` / `dashboard` / `interject`) | Flush/stop voice on send paths. |
| `restore_permission_stashes` / `resolve_permission_queue_transition` | `…/dispatch/permissions.rs` | Restore pre-permission composer after queue empties; clear followup text on next front. |
| `prompt.stash` / `prompt.restore` | prompt widget | Overlay stashes (`pav.stashed_prompt`, `permission_stashed_prompt`, `casual_stashed_prompt`, `qv.stashed_prompt`). |

**Missing helper:** no shared
`flush_plan_comment_or_freeform_before_decision()` used by approve / abandon /
mouse footer / casual send. Plan decisions call `approve_plan` /
`abandon_plan` / `send_casual_plan_comments` with only **already-saved**
comment lists.

---

## Surfaces

### 1. Plan approval (exit plan mode) — **still broken for button paths**

Code: `plan.rs` (`approve_plan`, `abandon_plan`, `send_plan_feedback`,
`handle_plan_feedback_key`), `viewer.rs` (keys + mouse footer).

| Path | Flushes draft? | Notes |
|------|----------------|--------|
| Enter in **Commenting** | Yes → `save_plan_comment` | Good. |
| Enter in **Prompt** (empty + no comments) | N/A → `approve_plan` | Good. |
| Enter in **Prompt** (freeform and/or saved comments) | Freeform via `send_plan_feedback` | **Revise**, not approve. Consistent with “request changes”. |
| `a` / mouse **approve** | Uses only `pav.comments` | **Does not** call `save_plan_comment`. Unsaved Commenting draft is dropped when `plan_approval_view` is taken and stashed prompt restored. Freeform still in Prompt is also **not** folded into approve Interject. |
| Mouse **quit** / `q` → `abandon_plan` | No | Same loss of Commenting draft / freeform. |
| Mouse **request changes** (`s` area) | Only switches focus to Prompt | Does not submit; freeform not flushed into revise until Enter. |
| Tab / click list while Commenting | `discard_in_progress_comment` | Intentional discard, not flush. |
| Keyboard while Commenting/Prompt | `a`/`q` go to textarea | Buttons are the main hole (mouse still hits footer while Commenting/Prompt). |

Approve **with already-saved** line comments is OK (`approve_plan` →
`format_feedback` → Interject “approved with review comments”). The gap is
**unsaved composer text** at click time.

No unit tests cover “approve while Commenting with non-empty prompt”.

### 2. Casual plan comments (preview without approval) — **still broken for send-while-drafting**

| Path | Flushes? |
|------|----------|
| Enter while casual commenting | Yes → `save_casual_plan_comment` |
| `s` / Ctrl+Enter / mouse send when **not** commenting | Sends `plan_comments` only |
| Mouse **send** while `casual_commenting_range` is set | **No** — `send_casual_plan_comments` ignores prompt draft |
| Close / click-outside | Restores `casual_stashed_prompt` (`cancel_line_viewer`) — draft not sent |

Same shape as plan approval: save helper exists; decisive send does not call it.

### 3. Ask-user question view — **mostly fixed (reference implementation)**

| Path | Behavior |
|------|----------|
| `submit_question_answers` | Always `swap_question_freeform()` first |
| Nav footer clicks while InputMode | Flushes freeform into slot, then synthesizes key |
| Leave InputMode via Esc / click outside editor | Writes freeform into slot |
| Tab switch | swap then load |

**Reuse this** for plan: a single “flush composer into durable state” call at
the top of `approve_plan` / `abandon_plan` / `send_casual_plan_comments` (or a
wrapper used by mouse + key decision entries).

### 4. Permission Allow / Reject / followup — **partial; different product rule**

| Path | Behavior |
|------|----------|
| Enter in **FollowupInput** | `PermissionFollowup(text)` with full prompt — OK |
| Double-click option / Enter in Options | `PermissionSelect` — **no** followup meta |
| Click option while FollowupInput | Resets focus to Options; draft stays in prompt until queue transition clears or restores stash |
| `resolve_permission_queue_transition` | Empty queue → `restore_permission_stashes`; else `prompt.set_text("")` so followup does not leak |

Allow/always while typed reject text is present: draft is **not** attached to
the allow (correct product), but is also **not** re-queued — user loses the
typed followup when the panel closes. If product wants “flush” here, it would
be “promote draft to pending main prompt after restore,” not “attach to Allow.”

Dashboard peek: `DashboardPermissionSelect` / `DashboardPermissionFollowup`
mirror agent paths (`dispatch/dashboard.rs`).

### 5. Cancel-turn panel “Continue” — N/A for chat draft

`CancelTurnChoice::ContinueToRun` has no freeform composer; no flush needed.

### 6. Voice interim before send / interject — **fixed**

`maybe_commit_voice_interim_before_submit_key` + `voice_stop_on_submit` /
`commit_interim_into_prompt`. Pattern: promote overlay text into the real
composer **before** the action that would otherwise ignore interim.

### 7. Overlay open / replace — restore, not flush

- New plan approval while casual commenting: restore `casual_stashed_prompt`
  before `stash()` for the new approval (`acp_handler/interactions.rs`).
- Replacing plan approval: restore old `stashed_prompt`, drop line viewer.
- These protect **pre-modal chat**, not in-progress **review comments**.

### 8. Minimal plan strip

Same agent handlers (`approve_plan` / `handle_plan_feedback_key`); only the
chrome differs (`xai-grok-pager-minimal/src/plan.rs`). Same gaps apply when
mouse/key hits those methods.

---

## Recommended fix shape (not implemented here)

1. Add something like `flush_plan_composer_into_comments(&mut self) -> bool`
   that, if Commenting (or casual commenting) and prompt non-empty, runs the
   existing save path; if Prompt-focused freeform non-empty and the action is
   **revise**, pass through to `send_plan_feedback`; if action is **approve**,
   product choice: either save freeform as an “overall” comment / Interject
   note, or refuse approve until user clears/revises (today: silent drop).
2. Call it from **all** entry points: mouse footer, `a`/`q`/`s` send (when
   those keys are decision shortcuts, not textarea input), and any future
   minimal click targets.
3. Mirror question-view tests: submit/approve with non-empty freeform and with
   Commenting focus; assert payload / Interject / comment list.

## Severity ranking

1. **Plan approve/abandon mouse while Commenting** — silent loss of line comment draft.  
2. **Casual send mouse while composing** — same.  
3. **Plan mouse approve while Prompt has freeform** — inconsistent with Enter (Enter revises; mouse approves and drops text).  
4. Permission followup discarded on Allow — lower if product is intentional; optional re-stash to main prompt.  

## Key file index

- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/interactions.rs` (question + permission keys/mouse)
- `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` (routing Commenting → mouse still hits viewer)
- `crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs`
- `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` (footer buttons)
- `crates/codegen/xai-grok-pager/src/app/dispatch/permissions.rs`
- `crates/codegen/xai-grok-pager/src/voice/handle.rs`
- `crates/codegen/xai-grok-pager-minimal/src/plan.rs`
