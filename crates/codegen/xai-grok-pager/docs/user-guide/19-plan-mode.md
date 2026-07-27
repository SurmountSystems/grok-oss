# Plan Mode

Plan mode is a structured planning phase: the agent explores the codebase and designs an implementation approach before writing any code. Use it for tasks with genuine ambiguity about the right approach, where getting your input before coding prevents significant rework.

---

## What Plan Mode Does

When plan mode is active, the agent:

1. Reads and searches the codebase to understand existing patterns and architecture
2. Designs an implementation approach and writes it to the plan file
3. May use `ask_user_question` to clarify specific questions
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

When the agent finishes planning, it calls the `exit_plan_mode` tool. By default the TUI **soft-parks** the plan:

- Toast + status chip (“Plan parked — press `/view-plan`…”) without taking over the screen
- An **inline plan card** in the transcript (preview + CTA legend)
- With an empty prompt, **`a` / `A` / `?` / `s` / `q`** work from the soft-park surface (no modal required)

Open the full review surface on demand with **`/view-plan`**, the status chip, or **`ShowPlan`**. That opens a **side panel** beside chat (CTAs in the panel footer). **Ctrl+F** enlarges the panel to fullscreen and back.

To open a full-screen plan modal immediately every time, set:

```toml
[ui]
plan_approval_park = "modal"   # default is "soft"
```

(Settings UI: **Plan approval park** — Soft (toast) vs Modal.)

If the agent exits without writing a plan (empty or missing `plan.md`), the same approval surface still opens (on demand, or immediately when modal is configured) with a clear empty-state message so you can approve and start implementing, clarify, revise (send the agent back to planning), or quit. In minimal mode the empty notice is committed into scrollback and the controls strip header reads **No plan written yet**.

### Reviewing the Plan

Scroll the plan with the arrow keys or `j`/`k`. Copy like conversation scrollback: **`y`** copies the selected line (or visual range); **`Y`** copies the whole plan body. CTAs and revise→agent selection are unchanged.

The action bar shows these shortcuts:

| Shortcut | Action                                                                                               |
| -------- | ---------------------------------------------------------------------------------------------------- |
| `a`      | **Approve** — leave plan mode and start building immediately (no notes required). Pending line notes, if any, still ride along. |
| `A`      | **Approve w/ comment** — focus the prompt for overall notes; `Enter` with text approves and attaches those notes. |
| `?`      | **Clarify** — ask about the plan without rewriting it. Focus moves to the prompt; type your question and press `Enter`. Plan mode stays active; the agent answers read-only and should call `exit_plan_mode` again so approval reappears. |
| `s`      | **Revise** — ask the agent to rewrite the plan. Focus moves to the prompt; type revision notes and press `Enter`. Plan mode stays active while the agent revises. |
| `q`      | **Quit** — abandon the plan without approving and turn plan mode off. |

There is **no primary Comment button**. You can still attach line-level notes with `Enter` on a selected line (or double-click); those notes go with Approve / Clarify / Revise when you submit.

Press `Tab` to move focus between the plan preview and the prompt. **Empty `Enter` on the prompt still approves** (same as `a`), even if you opened the prompt with `A`, `?`, or `s`.

### Clarify vs Revise vs Approve

| Path | Intent | What the agent does |
| ---- | ------ | ------------------- |
| **Approve** (`a` / empty Enter) | Build it | Leave plan mode and implement |
| **Approve w/ comment** (`A` + notes + Enter) | Build it, with notes | Leave plan mode; notes are attached as review comments |
| **Clarify** (`?`) | Understand the plan | Answer from the plan and research; **do not** rewrite `plan.md` unless you explicitly ask to change it; re-present approval when done |
| **Revise** (`s`) | Change the plan | Revise `plan.md` from your notes; stay in plan mode; re-present when ready |
| **Quit** (`q`) | Abort | Leave plan mode; no implement |

### Providing Feedback

The approval view has three focus states:

- **Preview**: Scroll the plan and use the primary CTAs above.
- **Commenting** (secondary): Add an inline note on a selected line range (`Enter` on a line, or double-click). Not a primary CTA.
- **Prompt**: Type freeform notes. What Enter does depends on how you opened the prompt (`A` approve w/ comment, `?` clarify, or `s` revise). Line notes attach to whichever action submits them.

Press `Tab` to switch between the preview and the prompt. After Clarify or Revise, plan mode stays active so you can iterate.

### Line selection and multi-line highlight

When you revise or clarify with a plan line selected, the agent receives the
selection context — not just your freeform words:

- **Path + line**: `@plan.md:N` for a single line, or `@plan.md:N-M` for a range
- **Quoted text**: each selected source line, prefixed with `>`
- **Your notes**: freeform prompt text and/or saved line comments

**Single line:** move the cursor to a line (or click it), focus the prompt with
`s` / `?` / `A`, type notes, press `Enter`. The agent sees that line’s text.

**Multi-line highlight:** start a visual selection over several plan lines
(same motion as multi-line select elsewhere in the line viewer), then submit
revise/clarify or attach a line comment with `Enter`. The agent gets the full
range and every quoted line in that range.

Saved line comments already store their range; freeform revise/clarify also
picks up the live viewer selection when you have not saved a comment yet.

### Screenshots in plan mode

You can paste or attach **screenshots** (and other images) on the plan-approval
prompt the same way as the normal chat composer. On submit:

| Action | What happens |
| ------ | ------------ |
| **Revise** (`s`) / **Clarify** (`?`) | Text feedback goes with the plan decision; screenshots ride as multimodal content on the same turn |
| **Approve w/ comment** (`A`) | Notes + screenshots ride together on the approve path |
| **Approve** with only screenshots | Screenshots still attach so the implement turn has visual context |

Image chips clear after submit. Empty `Enter` with no text, comments, or images
still means plain approve.

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
Active      --> Inactive (exit_plan_mode approved, or you toggle plan mode off when idle)
Active      --> ExitPending (you toggle plan mode off while a turn is in-flight)
ExitPending --> Inactive (after the turn completes)
```

Plan mode state is persisted to disk and survives process restarts. Transient states (`Pending`, `ExitPending`) are collapsed to `Inactive` on restart since they depend on in-flight interactions.

---

## Edits During Plan Mode

During active plan mode, edits to the plan file are auto-approved without prompting, so the agent can iterate on the plan freely. Edits to **any other file are rejected** before they run — the agent receives a short message naming the plan file as the only editable path.

This enforcement is independent of the permission mode:

- **Always-approve (yolo) stays armed underneath plan mode.** Non-edit tools (bash commands, reads, MCP tools) still auto-run, but file edits are blocked until you approve exiting plan mode. Once the plan is approved, always-approve resumes for implementation.
- Bash commands are not inspected for file writes — plan mode blocks the edit tools, not shell redirection.
- Subagents are not covered by the parent session's plan-mode edit gate. Each subagent starts with a fresh plan-mode tracker (`Inactive`), so a `general-purpose` (or other write-capable) subagent can edit files while the parent is still in plan mode — and it inherits the parent's permission mode (including always-approve). Read-only types such as `explore` remain limited by their own toolset.

The status flag shows `plan` while plan mode is active. If always-approve is enabled underneath, its flag reappears when plan mode exits.

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
