# Plan Mode

Plan mode is a structured planning phase: the agent explores the codebase and designs an implementation approach before writing any code. Use it for tasks with genuine ambiguity about the right approach, where getting your input before coding prevents significant rework.

---

## What Plan Mode Does

When plan mode is active, the agent:

1. Reads and searches the codebase to understand existing patterns and architecture
2. Designs an implementation approach and writes it to the plan file
3. Puts open questions as plain bullets in the plan file or freeform chat (not multi-choice `ask_user_question` questionnaires)
4. Calls `exit_plan_mode` to present the plan for your approval

Plan mode is read-only except for the plan file: plan-file edits (`plan.md` in the session directory) are auto-approved, and edits to any other file are rejected outright — the tool call fails with a short message naming the plan file as the only editable path. This holds in every permission mode, including always-approve. Separating planning from implementation lets you review and correct the approach before any code is written.

---

## How to Enter Plan Mode

### Agent-Initiated Entry

The agent enters plan mode when it determines a task has genuine ambiguity. It calls the `enter_plan_mode` tool, which requires your approval before plan mode activates. If you decline, the agent stays in normal mode.

**Good triggers for plan mode:**

- "Add user authentication to the app" -- genuinely ambiguous (session vs JWT, token storage, middleware structure)
- "Redesign the data pipeline" -- major restructuring where the wrong approach wastes significant effort
- "Add caching to the API" -- multiple reasonable approaches (Redis vs in-memory vs file-based)
- "Add real-time updates" -- architectural decision (WebSockets vs SSE vs polling)

**Not appropriate for plan mode:**

- "Add a delete button to the user profile" -- clear implementation path
- "Fix the typo in the README" -- straightforward
- "Update the error handling in the API" -- start working, ask specific questions if needed
- "Can we work on the search feature?" -- user wants to get started, not plan

### User-Initiated Entry

You can enter plan mode yourself in two ways:

- **`/plan`** -- Enter plan mode. Plan mode activates when you send your next prompt. Run `/plan <description>` to enter plan mode and start a turn with that description in one step.
- **Shift+Tab** -- Cycle the session mode: Normal, then Plan, then Always-approve, then back to Normal. From Normal, a single press lands on Plan.

After a plan exists, run **`/view-plan`** (aliases `/show-plan`, `/plan-view`) to reopen its saved preview.

---

## The Plan File

The plan is written to `plan.md` inside the session directory (`~/.grok/sessions/<cwd>/<session-id>/plan.md`, where `<cwd>` is an encoded directory name, not the literal path).

The plan file contains:

- A **Context** section explaining why the change is being made
- The recommended approach (not every alternative)
- The paths of critical files to modify
- Existing functions and utilities to reuse, with their file paths
- A verification section describing how to test the changes end to end

---

## Plan Approval

### Present is not approval

When the agent finishes planning, it calls the `exit_plan_mode` tool. That call **presents** the plan for review. It is **not** operator approval.

| What happened | What it means |
| ------------- | ------------- |
| Agent tool `exit_plan_mode` succeeded; toast/status says “Plan ready…” | Plan is **parked for review**. Do **not** treat this as Approve. The model is told the same: present only, wait for a real panel decision. |
| You click **Approve** (or Approve w/ comment) on the plan panel / soft-park strip | **Real approval.** Plan mode leaves; implement may start. The model hears that you approved **via the plan panel CTAs**. |
| Footer shows `always-approve` (permission mode) | Skips **tool permission** prompts only. It does **not** auto-click plan panel Approve. You still use the plan CTAs. |
| Headless / no interactive client | No plan panel. Plan mode may exit with an honest “no interactive panel” message. That is **not** the same as a panel Approve click. |

**Never** approve by freeform chat (“reply approve / revise / abandon”). Real decisions are the plan panel CTAs below (mouse primary; empty-prompt key accelerators `a` / `A` / `?` / `s` / `q` when the side panel is open).

### Soft park and side panel

By default the TUI **soft-parks** the plan and **auto-opens the side panel** so Approve / Revise / Clarify / Quit are visible **without** an extra click or `/view-plan`:

- Toast + status chip (**“Plan ready. Side panel open”**) without a fullscreen takeover
- Status does **not** say “Plan written. Click or /view-plan” while a live park with CTAs is active (that string is only a short idle cue before decision chrome parks)
- After **Revise** or **Clarify** unparks, status says **“Revising plan...”** or **“Waiting for updated plan...”** (not idle click ceremony) until the agent calls `exit_plan_mode` again. Idle decision chrome does **not** re-arm mid-rewrite.
- The **side panel** opens beside chat immediately (same surface as `/view-plan`)
- An **inline plan card** in the transcript (preview only; not a fake button menu)
- **Clickable footer buttons** (Approve / Notes / Clarify / Revise / Quit). Mouse works even when the prompt has draft text.
- Live draft text is **kept** (not stashed/cleared). Prompt stays focused so typing is live.
- Soft-park is **non-capturing** for the main thread when the panel is dismissed: bare printable keys type into the composer; decisions use the soft-park strip buttons.

### Three approval surfaces

There are **three** distinct plan surfaces after soft park. Looking only at the
transcript card is **not** the full approval UI.

| Surface | Borders | CTAs |
|---------|---------|------|
| **Side panel** (auto-open after `exit_plan_mode`, or `/view-plan`) | Full box / title-footer lines | Approve / Notes / Clarify / Revise / Quit in the **panel footer** |
| **Soft-park strip** (panel dismissed, or terminal too small for panel paint) | None | Same five clickable strip buttons in the shortcuts row (never silent empty) |
| **Transcript plan card** | None | Plain preview / pointer text only — **not** a fake button menu |

The side panel has **clickable CTA buttons** in the panel footer (Approve / Approve w/ comment / Clarify / Revise / Quit). With an **empty** prompt while the side panel is open, keys `a` / `A` / `?` / `s` / `q` are accelerators (including when Prompt is focused after soft-park present). **Empty Enter never approves.** On a narrow side panel the footer uses shorter labels (or key-only) so the hit targets always stay clickable. If the panel is open but too small to paint those footer hits, the soft-park strip CTAs reappear so approval is never zero-chrome; status still says **Plan ready** (not click ceremony). **Ctrl+F** enlarges the panel to fullscreen and back. The panel always re-reads the latest session `plan.md` when you open it (so rewrites while parked show up, not a frozen snapshot from park time).

If you dismiss the panel, reopen with **`/view-plan`**, the status chip, or **`ShowPlan`**.

To open a full-screen plan modal immediately every time (and stash the live draft), set:

```toml
[ui]
plan_approval_park = "modal"   # default is "soft"
```

(Settings UI: **Plan approval park** — Soft (side panel) vs Modal.)

If the agent exits without writing a plan (empty or missing `plan.md`), the same approval surface still opens (auto side panel, or fullscreen when modal is configured) with a clear empty-state message so you can approve and start implementing, clarify, revise (send the agent back to planning), or quit. In minimal mode the empty notice is committed into scrollback and the controls strip header reads **No plan written yet**.

### Reviewing the Plan

Scroll the plan with the arrow keys or `j`/`k`. Copy like conversation scrollback: **`y`** copies the selected line (or visual range); **`Y`** copies the whole plan body. The plan panel top bar also has a **`⧉`** button next to `[↗]` (and `[✗]` when close is shown) that copies the whole plan body (same payload as **`Y`**). Conversation user/assistant bubbles also show always-on **`⧉`** (independent of plan chrome). In plan review, close is omitted on purpose. CTAs and revise→agent selection are unchanged.

The action bar shows these shortcuts:

| Shortcut | Action                                                                                               |
| -------- | ---------------------------------------------------------------------------------------------------- |
| `a`      | **Approve** — leave plan mode and start building immediately (no notes required). Pending line notes, if any, still ride along. Empty-prompt only when the side panel is open (or Preview focus). |
| `A`      | **Approve w/ comment** — focus the prompt for overall notes; `Enter` with text approves and attaches those notes. |
| `?`      | **Clarify** — answers without rewriting the plan. Focus moves to the prompt; type your question and press `Enter`. Plan mode stays active; the agent answers read-only and should call `exit_plan_mode` again so approval reappears. |
| `s`      | **Revise** — immediately sends the agent back to rewrite the plan (toast: "Revision sent…"). Any freeform already in the prompt rides as notes. Plan mode stays active while the agent revises and calls `exit_plan_mode` again. Do not use Clarify when you want the plan file changed. |
| `q`      | **Quit** — abandon the plan without approving and turn plan mode off. |

There is **no primary Comment button**. You can still attach line-level notes with `Enter` on a selected line (or double-click); those notes go with Approve / Clarify / Revise when you submit.

**Revise is decisive** (mouse footer button and empty-prompt `s` / panel footer Revise): it unparks approval and notifies the agent right away — it does **not** only flip a silent intent while the panel stays open. To attach written notes, type them first, then click **Revise** (or press freeform `Enter`, which revises by default when the prompt has text).

**Continuous revise loop:** after Revise (or Clarify) unparks, the TUI stays in a rewrite/answer wait state. Status is **“Revising plan...”** / **“Waiting for updated plan...”** while the rewrite has not started yet (not idle **“Plan written. Click or /view-plan”**). When the agent is already busy rewriting, the normal turn status shows thinking/tools/cancel instead of a barren exclusive wait. A human line always appears in the scrollback (your notes, or **“Revise the plan”** / **“Clarify the plan”** when empty). The composer clears so you do not get ghost draft **Enter:queue** while the rewrite runs. Approve / Revise CTAs do not re-arm until a **new** `exit_plan_mode` present. When the agent re-presents, CTAs arm once again (same soft-park auto-open as the first present). Freeform Enter during that wait cannot attach to the closed plan-feedback channel; if the turn is busy, the message **queues as a normal follow-up** with an honest toast (never silent fail).

Press `Tab` to move focus between the plan preview and the prompt. **Empty `Enter` on the prompt does not approve** (P1 / operator default): bare Enter with no freeform text, no line comments, and no screenshots is a no-op so typing is not accidental-approve. Use **mouse Approve** or empty-prompt **`a`** (side panel open) to approve without notes.

### Clarify vs Revise vs Approve

| Path | Intent | What the agent does |
| ---- | ------ | ------------------- |
| **Approve** (mouse Approve / empty-prompt `a`) | Build it | Leave plan mode and implement |
| **Approve w/ comment** (`A` + notes + Enter) | Build it, with notes | Leave plan mode; notes are attached as review comments |
| **Clarify** (`?`) | Understand the plan | Answer from the plan and research; **do not** rewrite `plan.md` unless you explicitly ask to change it; re-present approval when done |
| **Revise** (`s` / mouse Revise) | Change the plan | Unpark immediately; revise `plan.md` from any freeform notes (or ask what to change if empty); stay in plan mode; re-present when ready |
| **Quit** (`q`) | Abort | Leave plan mode; no implement |

### After one decisive Approve or Quit

After you **Approve** (with or without notes) or **Quit** once for a presented plan, the TUI must **not** re-arm Approve / “Plan ready” decision chrome for that same present. Status should not invite a second decision on the same `plan.md` body, even if plan mode is still active for a moment or the file still says “approved and implemented.”

Decision chrome re-arms only when the agent calls **`exit_plan_mode` again** (a new present). **Approve** and **Quit** set a sticky “already decided” flag until that new present. **Revise** and **Clarify** do **not** set that sticky flag; they set a separate **in-flight** wait so idle “Plan written / Click or /view-plan” chrome and decision CTAs stay off until re-present. After re-present, CTAs arm once for the updated plan.

### Providing Feedback

The approval view has three focus states:

- **Preview**: Scroll the plan and use the primary CTAs above.
- **Commenting** (secondary): Add an inline note on a selected line range (`Enter` on a line, or double-click). Not a primary CTA.
- **Prompt**: Type freeform notes. What Enter does depends on how you opened the prompt (`A` approve w/ comment or `?` clarify; freeform Enter defaults to revise). Line notes attach to whichever action submits them. The **Revise** CTA itself submits immediately and does not leave you on the prompt.

Press `Tab` to switch between the preview and the prompt. After Clarify or Revise, plan mode stays active so you can iterate.

### Line selection and multi-line highlight

When you revise or clarify with a plan line selected, the agent receives the
selection context — not just your freeform words:

- **Path + line**: `@plan.md:N` for a single line, or `@plan.md:N-M` for a range
- **Quoted text**: each selected source line, prefixed with `>`
- **Your notes**: freeform prompt text and/or saved line comments

**Single line:** move the cursor to a line (or click it), type notes in the
prompt (or use `?` / `A` for clarify / approve-with-comment), then press
`Enter` or click **Revise**. The agent sees that line’s text.

**Multi-line highlight:** start a visual selection over several plan lines
(same motion as multi-line select elsewhere in the line viewer), then submit
revise/clarify or attach a line comment with `Enter`. The agent gets the full
range and every quoted line in that range.

Saved line comments already store their range; freeform revise/clarify also
picks up the live viewer selection when you have not saved a comment yet.

### Screenshots in plan mode

You can paste or attach **screenshots** (and other images) while plan approval
is parked — including soft-park Preview and with the side panel open on Preview
— the same way as the normal chat composer (paste path / drop / wrap image).

**TUI self-screenshot:** `/screenshot` or **F9** captures the current pager
frame under `$GROK_HOME/screenshots/tui-*.png`. When plan approval is open, that
PNG is **auto-attached** to the plan composer (same multimodal chip as a paste),
so approve / revise / clarify can send it without a separate paste step. Outside
plan approval the capture is toast + path only.

On submit:

| Action | What happens |
| ------ | ------------ |
| **Revise** (`s`) / **Clarify** (`?`) | Text feedback goes with the plan decision; screenshots ride as multimodal content on the same turn |
| **Approve w/ comment** (`A`) | Notes + screenshots ride together on the approve path |
| **Approve** (mouse Approve / empty-prompt `a`) with only screenshots attached | Screenshots ride so the implement turn has visual context; empty Enter alone is still a no-op |

Image chips clear after submit. Empty `Enter` with no freeform text, no line
comments, and no images is a **no-op** (never plain approve). Approve without notes
via mouse **Approve** or empty-prompt **`a`** (side panel open). When you Approve,
Revise, or Clarify with screenshots already attached, those chips ride on that
decision.

### Leaving the Approval View

Press `Esc` to return focus from the prompt to the plan preview. To dismiss the approval without approving or sending feedback, press `q` to quit the plan. Quitting abandons the proposed plan and turns plan mode off.

---

## Plan Mode Lifecycle

The plan mode state machine has four states:

| State          | Description                                                    |
| -------------- | -------------------------------------------------------------- |
| `Inactive`     | Normal operating mode. No plan mode constraints.               |
| `Pending`      | Client toggled plan mode ON, but no prompt has been sent yet.  |
| `Active`       | Plan mode is active. Plan-file edits are auto-approved; edits to other files are rejected. |
| `ExitPending`  | User toggled plan mode OFF while a turn is in-flight.          |

Transitions:

```
Inactive    --> Active   (enter_plan_mode tool called and approved -- skips Pending)
Inactive    --> Pending  (you toggle plan mode on with /plan or Shift+Tab)
Pending     --> Active   (your first prompt activates plan mode)
Active      --> Inactive (plan panel Approve/Quit after present, or you toggle plan mode off when idle)
Active      --> ExitPending (you toggle plan mode off while a turn is in-flight)
ExitPending --> Inactive (after the turn completes)
```

Plan mode state is persisted to disk and survives process restarts. Transient states (`Pending`, `ExitPending`) are collapsed to `Inactive` on restart since they depend on in-flight interactions.

---

## Edits During Plan Mode

During active plan mode, edits to the plan file are auto-approved without prompting, so the agent can iterate on the plan freely. Edits to **any other file are rejected** before they run — the agent receives a short message naming the plan file as the only editable path.

This enforcement is independent of the permission mode:

- **Always-approve (yolo) stays armed underneath plan mode for tool permissions only.** Non-edit tools (bash commands, reads, MCP tools) still auto-run without permission prompts, but file edits are blocked until you leave plan mode via a **plan panel** decision. Always-approve does **not** auto-click Approve on a soft-parked plan.
- Once you **Approve** on the plan panel, plan mode ends and always-approve (if still on) resumes for implementation tool prompts as usual.
- Bash commands are not inspected for file writes — plan mode blocks the edit tools, not shell redirection.
- Subagents are not covered by the parent session's plan-mode edit gate. Each subagent starts with a fresh plan-mode tracker (`Inactive`), so a `general-purpose` (or other write-capable) subagent can edit files while the parent is still in plan mode — and it inherits the parent's permission mode (including always-approve). Read-only types such as `explore` remain limited by their own toolset.

The status flag shows `plan` while plan mode is active. If always-approve is enabled underneath, its flag may still show in the footer during soft-park; that only means tool permissions are relaxed, not that the plan was auto-approved.

---

## Plan Mode and Compaction

When `/compact` runs during an active plan mode session, the plan mode state is preserved. The compacted context includes a reminder that plan mode is active, so the agent continues planning after compaction.

---

## When Plan Mode is Appropriate

**Use plan mode for:**

- Tasks with significant architectural ambiguity (multiple reasonable approaches)
- Unclear requirements that need exploration before implementation
- High-impact restructuring where the wrong approach wastes significant effort

**Skip plan mode for:**

- Tasks with a clear implementation path
- Bug fixes where the fix is obvious once you understand the bug
- Adding features that follow existing conventions
- Straightforward modifications (renaming, formatting, adding tests)
- Research and exploration tasks (use subagents instead)
