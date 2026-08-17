# Plan Mode

Plan mode is a structured planning phase: the agent explores the codebase and designs an implementation approach before writing any code. Use it for tasks with genuine ambiguity about the right approach, where getting your input before coding prevents significant rework.

---

## What Plan Mode Does

When plan mode is active, the agent:

1. Reads and searches the codebase to understand existing patterns and architecture
2. Designs an implementation approach and writes it to the plan file
3. May ask questions in the plan file or in chat. This fork prefers **freeform** questions. The questionnaire modal (`ask_user_question`) is `--legacy` only.
4. Calls `exit_plan_mode` to **present** the plan for your review. Present is not Approve. Always-approve permission mode does **not** click Approve for you. Soft present docks a real right-side pane. It is not a centered overlay that dims the transcript.

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

When the agent finishes planning, it calls the `exit_plan_mode` tool. The tool reads the plan file from disk. Soft present (`plan_approval_park = "soft"`, the default) docks a scrollable plan pane on the **right** of the transcript. The transcript stays visible and is not dimmed. Status says **Plan ready. Side panel open** only when that plan viewer is actually open. Force a covering overlay with `plan_approval_park = "modal"` (or enlarge). The four footer CTAs stay on the pane.

### Present is not Approve

A successful `exit_plan_mode` (or a **Plan ready** status) means the plan is **presented for review**. It is not operator approval. Always-approve skips tool-permission prompts only. It does not auto-click Approve.

The four primary actions are mouse buttons: **Approve**, **Clarify**, **Revise**, **Exit**. Letter keys type into the prompt and into the plan pane box, so you can type `also` or `Also` while review is open. Capital A is not a notes action. Empty `Enter` on the prompt never Approves. Use the clickable **Approve** button.

If the agent exits without writing a plan (empty or missing `plan.md`), the same approval surface still opens with a clear empty-state message so you can approve and start implementing, revise (type notes, then send the agent back to planning), clarify, or exit. In minimal mode the empty notice is committed into scrollback and the controls strip header reads **No plan written yet**.

### Reviewing the Plan

Scroll the plan with the arrow keys or `j`/`k`. Clicking a plan row focuses or scrolls that line. It does **not** enter Commenting and it does not steal the composer. `c` is the explicit line-comment gesture. The right-pane footer and the composer shortcut row share the same four actions (mouse buttons are the primary path; letter keys type):

| Control | Action |
| ------- | ------ |
| **Approve** | Approve the plan and start building. Typed notes and pending line comments ride with the approval. |
| **Clarify** (`?`) | Focus the box so you can type a question; the agent answers without rewriting the plan. |
| **Revise** | Focus the box in revise mode and wait. Type notes, then press `Enter` to send. An empty Revise click does not submit. |
| **Exit** | Abandon the plan without approving and turn plan mode off. Empty `Ctrl+C` also exits. |
| `y` | Copy the full plan to the clipboard. |
| `Tab` | Move focus between the plan preview and the prompt. |

Empty `Enter` on the prompt never Approves. Use the clickable **Approve** button.

### Screenshots in plan mode

You can paste or attach screenshots while plan approval is open, the same way as the normal chat composer. Plan-review `Event::Paste` and Ctrl+V run the clipboard image probe (including a GNOME screenshot that is pixels, not a file path). Linux empty bracketed paste on the normal composer also probes. Approve and Revise **drain** attached image chips onto the feedback they send. They do not drop those chips.

**TUI self-screenshot:** `/screenshot` or **F9** captures the current pager frame under `$GROK_HOME/screenshots/tui-*.png`. When plan approval is open, that PNG **auto-attaches** to the plan composer so Approve / Revise / Clarify can send it without a separate paste. Outside plan approval the capture is toast plus path only.

Empty `Enter` with no freeform text, no line comments, and no images is still a no-op. Approve without notes via the clickable **Approve** button. Attached screenshots ride with Approve, Revise, or Clarify when you submit those actions.

While the plan approval view is open, `Ctrl+P` (command palette → model) still works for switching model before you click **Approve**.

### Providing Feedback

The approval view has three focus states:

- **Preview**: Scroll the plan. A click on a row stays here (or leaves the composer typeable). It does not enter Commenting.
- **Commenting**: Add an inline comment to the selected line range. Press `c` for that explicit line-comment gesture. Do not use a row click for this.
- **Prompt**: Type freeform revision notes (Revise and Clarify). You can also type `also` or `Also` here.

Press `Tab` to switch between the preview and the prompt. When you send feedback (inline comments, freeform notes, or both) the agent receives it and revises the plan. Plan mode stays active so you can iterate.

### Leaving the Approval View

Press `Esc` to return focus from the prompt to the plan preview. To dismiss the approval without approving or sending feedback, click **Exit**. That abandons the proposed plan and turns plan mode off. Empty `Ctrl+C` does the same.

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
