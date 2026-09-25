# Plan Mode

Plan mode is a structured planning phase: the agent explores the codebase and designs an implementation approach before writing any code. Use it for tasks with genuine ambiguity about the right approach, where getting your input before coding prevents significant rework.

---

## What Plan Mode Does

When plan mode is active, the agent:

1. Reads and searches the codebase to understand existing patterns and architecture
2. Designs an implementation approach and writes it to the plan file
3. May ask questions in the plan file or in chat. This fork prefers **freeform** questions. The questionnaire modal (`ask_user_question`) is `--legacy` only.
4. Calls `exit_plan_mode` to **present** the plan for your review. Present is not Approve. Always-approve permission mode does **not** click Approve for you. Soft present docks a real right-side pane. The pane paints over the conversation text behind it. The left of the transcript stays visible and is not dimmed. It is not a centered overlay that dims the transcript.

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

- **Exclusive `/plan`** -- Bare `/plan` without `--soft`. Exclusive `/plan` enters plan mode. Plan mode activates when you send your next prompt. Exclusive `/plan` exclusive-blocks nested implementers. Nested implementers do not stay Working under exclusive `/plan`. Run `/plan <description>` to enter exclusive `/plan` and start a turn with that description in one step. `/plan` with extra Operator text submits a plan-update turn even when Isolated Preview leftover is docked. It does not only pull up a stale plan. That submit writes the prompt write-ahead log. While that plan-update turn is running, Isolated Preview stays docked as rewriting-wait: the pane quotes the Operator's second prompt, idle Approve / Comment / Revise / Exit do not arm, and leftover stale `plan.md` is not a live present. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). When `exit_plan_mode` writes current disk `plan.md`, Isolated Preview presents that file and idle CTAs arm.
- **Shift+Tab** -- Cycle the session mode: Normal, then Plan, then Always-approve, then back to Normal. From Normal, a single press lands on Plan.

After a plan exists, run **`/view-plan`** (aliases `/show-plan`, `/plan-view`) to reopen Isolated Preview. That viewer uses the same four idle actions as a live present: **Approve**, **Comment**, **Revise**, **Exit**. Copy lives on `y` and a title-bar control, not as a fifth idle CTA on the Approve row. On this Surmount fork, **Approve** also files a GitHub issue with the plan text ([`docs/github-tracking.md`](../../../../../../docs/github-tracking.md)). A clickable **copy** control copies the plan. While the plan comment composer is focused, y inserts the letter and does not copy. A dot marks the **selected** CTA (the one Enter will submit). That mark is live selection, not a leftover grok-oss.db recorded row. Present, empty Enter, and always-approve tool permissions do not Approve. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). Clicking Approve is a real Approve only while a live waiter is parked. After Approve or Exit, the four buttons still paint; they do not re-arm Plan ready.

### Isolated Preview (`/plan --soft`)

**`/plan --soft`** docks Isolated Preview on the right without a covering exclusive present. Isolated Preview is the existing plan present surface on the right of the transcript. It does not enter plan mode. It does not park L1. Nested implementers stay Working. Soft planning does not reset the primary plan. It makes a secondary plan. Isolated Preview does not immediately pull up leftover current `plan.md`. Isolated Preview does not dock leftover primary `plan.md`. `/plan --soft add feature` seeds Isolated Preview with that description and does not enqueue it as a Prompt. Exclusive `/plan` (bare `/plan` without `--soft`) enters plan mode and exclusive-blocks nested implementers. Present is not Approve. Isolated Preview stays after present so Comment then Approve can run. Isolated Preview does not close when nested implementers continue. Isolated Preview stays until Esc, Exit, or Approve. Isolated Preview is not a Plan Exit timer. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). Comment then Approve still works on a real present of that secondary plan after `exit_plan_mode` writes it. The Isolated Preview composer is an Operator box unless you click Comment. Isolated Preview idle after present, a non-empty Operator box (including a paste chip), plus Enter Approves with those notes. It does not Plan-Exit and leave the paste. Composer clears only after that Approve lands. Comment CTA still focuses the Operator box for notes then Approve. After Plan Exit, non-empty Enter still sends. `--soft` is not the queue hold token (`queue` / `later`). Comment then Approve carries notes. Approve still files a GitHub issue with the plan text ([`docs/github-tracking.md`](../../../../../../docs/github-tracking.md)). `/view-plan` and a primary Isolated Preview re-reads session `plan.md`. After Revise rewrites that file, the side panel shows the current file, not the first-draft snapshot.

When nested implementers continue, Isolated Preview does not close. Isolated Preview stays until Esc, Exit, or Approve. Isolated Preview is not a Plan Exit timer. Nested implementers continuing is not a reason to close Isolated Preview. Isolated Preview does not dock leftover primary `plan.md` because nested implementers rewrote that file. After nested implementers finish, Isolated Preview must not paint leftover primary `plan.md` as the Isolated Preview body, and it must not paint a TECH.md persist overwrite. Isolated Preview leftover after the first present stays so Comment then Approve can run. A second plan prompt, or exclusive `/plan` plus extra Operator text, is a plan-update turn. While that plan-update turn is running, Isolated Preview stays docked as rewriting-wait and quotes the Operator's second prompt. It does not paint leftover stale `plan.md` as a live present. Idle Approve / Comment / Revise / Exit do not arm. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). When `exit_plan_mode` writes current disk `plan.md`, Isolated Preview presents that file and idle CTAs arm. Paste-then-Enter Approve still works after that new present. `/view-plan` reopens Isolated Preview from current disk `plan.md`.

When Isolated Preview is docked on the main session (L1), nested implementers keep running. Those sessions stay Working. Docking Isolated Preview does not cancel them and does not paint Cancelling. Exclusive `/plan` is the path that exclusive-blocks nested implementers.

If Approve still needs a live waiter, that waiter must not send cancel to nested implementers. Clicking Approve remains a real Approve only while that waiter is parked.

**`/rebuild`** with Isolated Preview open restores that pane after relaunch. Resume does not auto-dock leftover `plan.md` when the pane was not open at persist.

After Plan Exit, Isolated Preview must not trap the session on a leftover plan. Isolated Preview is not a Plan Exit timer. Isolated Preview stays until Esc, Exit, or Approve. Esc:close, `/start`, or `/unstick` (when the last parent prompt is hung) leave parked Isolated Preview. `/start` continues paused or interrupted work in this process. It is not `/resume`. The painted Isolated Preview body is this session's current disk `plan.md` for a primary Isolated Preview, not leftover primary `plan.md` on a secondary Isolated Preview, and not a leftover TECH.md snapshot. With Isolated Preview closed, chrome must not stay plan. Exclusive `/plan` (bare `/plan` with no extra text) exclusive-blocks nested implementers. It is not Isolated Preview. `/view-plan` docks Isolated Preview from this session's current disk `plan.md`, not leftover "why the agent stopped" or a TECH.md persist overwrite. `/plan --soft` does not reset the primary plan. It makes a secondary plan. Isolated Preview does not immediately pull up leftover current `plan.md`. Isolated Preview does not dock leftover primary `plan.md`. `/plan` with extra Operator text submits a plan-update turn of the primary plan. Compact at 100% / over 500k must not swallow `/plan`. Typing an Operator sentence after Exit still sends. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)).

---

## Reasoning effort on a live plan turn

While exclusive `/plan` or Isolated Preview `/plan --soft` is the live plan turn, grok-oss uses reasoning effort **xhigh** even if the session `/effort` is medium. After you click Exit or Approve, later turns use the session effort again. `/effort` is still how you set the stored session effort.

**Turbo planning** in `/settings` is on by default. Turn it off when you want those plan turns to keep the session effort. That off path is the upstream-like behavior.

The only live indication is the existing lower-right yellow model/effort line showing `xhigh`. The magenta model id stays the model id. There is no TURBO badge, banner, or toast. See [Configuration → Turbo planning](05-configuration.md#turbo-planning).

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

When the agent finishes planning, it calls the `exit_plan_mode` tool. The tool reads the plan file from disk. Soft present (`plan_approval_park = "soft"`, the default) docks a scrollable plan pane on the **right** of the transcript. The pane paints over the conversation text behind it. The left of the transcript stays visible and is not dimmed. It is not a centered overlay that dims the transcript. Status says **Plan ready. Side panel open** only when that plan viewer is actually open. Force a covering overlay with `plan_approval_park = "modal"` (or enlarge). The four footer CTAs stay on the pane.

### Present is not Approve

A successful `exit_plan_mode` (or a **Plan ready** status) means the plan is **presented for review**. It is not operator approval. Always-approve skips tool-permission prompts only. It does not auto-click Approve. When Isolated Preview is docked on L1, nested implementers keep running. Exclusive `/plan` exclusive-blocks nested implementers. If a waiter is required for Approve, that waiter must not send cancel to those nested implementers.

The four idle actions are mouse buttons: **Approve**, **Comment**, **Revise**, **Exit**. Click a button to mark it and run it (the selected one is marked). Enter submits the marked CTA except Approve. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). A first click on **Approve** still Approves. A first click on **Comment** focuses the comment composer. A first click on idle **Revise** focuses the box and waits; after a comment is typed, Revise rewrites. A first click on **Exit** abandons. After **Comment**, **Clarify** sends questions (not a rewrite). A second click on an already-selected CTA still submits that action. Letter keys type into the prompt and into the plan pane box, so you can type `also` or `Also` while review is open. Capital A is not a notes action. Empty `Enter` never Approves. When the Operator box is composing a line comment or revise draft, Enter still saves or sends that draft (`Enter:save comment` on the line-comment overlay, including while session Multiline is on).

Approving while the plan panel is open keeps the composer comment. The session quotes `The user approved the plan with the following review comments:` and then the comment, leaves plan review, and can start the work. A confirmed comment is `Love it! Execute now.` Pressing Enter with an empty composer does not approve the plan.

If the agent exits without writing a plan (empty or missing `plan.md`), the same approval surface still opens with a clear empty-state message so you can approve and start implementing, comment (then Approve, Clarify, or Revise), revise, or exit. In minimal mode the empty notice is committed into scrollback and the controls strip header reads **No plan written yet**.

### Reviewing the Plan

Scroll the plan with the arrow keys or `j`/`k`. Clicking a plan row focuses or scrolls that line. It does **not** enter Commenting and it does not steal the composer. `c` is the explicit line-comment gesture. The right-pane footer and the composer shortcut row share the same four actions (mouse buttons are the primary path; letter keys type):

| Control | Action |
| ------- | ------ |
| **Approve** | Approve the plan and start building. An empty click still implements. Typed comments and pending line comments ride with the approval. |
| **Comment** | Focus the prompt as the comment composer. After you type (or with an empty box), click **Approve** to implement with notes, **Clarify** for a read-only question, or **Revise** to rewrite the plan. |
| **Clarify** | Shown after **Comment** (or after focusing the prompt). Sends the current comment as a read-only question. Does not rewrite the plan. Empty Preview `?` still arms this path. While you are typing in the Operator box, `?` inserts. |
| **Revise** | Idle click focuses the box and waits. After a comment is typed, Revise rewrites the plan with that text. An empty Revise click does not submit. |
| **Exit** | Abandon the plan without approving and turn plan mode off. Empty `Ctrl+C` also exits. |
| **copy** / `y` | Copy the full plan to the clipboard. Empty Preview `y` copies. While the plan comment composer is focused, y inserts the letter and does not copy. While you are typing a Prompt draft in the Operator box, `y` inserts. The clickable copy control is the mouse path. The comment footer does not advertise `y:copy`. |
| `Tab` | Move focus between the plan preview and the prompt. |
| `Enter` | Submit the marked CTA when Preview is empty, except Approve. Empty `Enter` never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). While commenting, Enter saves the line comment. |

Empty `Enter` never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). Use the clickable **Approve** button.

### Screenshots in plan mode

You can paste or attach screenshots while plan approval is open, the same way as the normal chat composer. Plan-review `Event::Paste` and Ctrl+V run the clipboard image probe (including a GNOME screenshot that is pixels, not a file path). Linux empty bracketed paste on the normal composer also probes. Approve and Revise **drain** attached image chips onto the feedback they send. They do not drop those chips.

**TUI self-screenshot:** `/screenshot` or **F9** captures the current pager frame under `$GROK_HOME/screenshots/tui-*.png`. When plan approval is open, that PNG **auto-attaches** to the plan composer so Approve / Revise / Clarify can send it without a separate paste. Outside plan approval the capture is toast plus path only.

Empty `Enter` with no freeform text, no line comments, and no images is still a no-op. Empty Enter never Approves. Clickable Approve only ([GitHub #122](https://github.com/SurmountSystems/grok-oss/issues/122)). Approve without notes via the clickable **Approve** button. Attached screenshots ride with Approve, Revise, or Clarify when you submit those actions. Comment is the idle entry to that composer.

While the plan approval view is open, `Ctrl+P` (command palette → model) still works for switching model before you click **Approve**.

### Providing Feedback

The approval view has three focus states:

- **Preview**: Scroll the plan. A click on a row stays here (or leaves the composer typeable). It does not enter Commenting.
- **Commenting**: Add an inline comment to the selected line range. Press `c` for that explicit line-comment gesture. Do not use a row click for this.
- **Prompt**: Type a comment. Then click **Approve** (implement with notes), **Clarify** (read-only answers), or **Revise** (rewrite). You can also type `also` or `Also` here.

Press `Tab` to switch between the preview and the prompt. **Approve** implements (typed comments ride along). You can type those comments in the Operator box while Preview is focused; you do not have to Tab to Prompt first. Empty Approve still implements without inventing notes. **Clarify** asks a read-only question. **Revise** rewrites the plan. Plan mode stays active after Clarify or Revise so you can iterate. `Ctrl+Z` undoes the last Operator-box edit, including a wipe (first `Ctrl+C`), while Preview or Prompt is focused. Shift+Enter in Preview matches the main Operator box: it inserts a newline when composer multiline is on, and it sends (or interjects) when `[ui] composer_multiline = false` or session Multiline is on. Overlay copy, clarify, and approve do not steal Shift+Enter.

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

Plan mode state is persisted to disk and survives process restarts. Transient states (`Pending`, `ExitPending`) are collapsed to `Inactive` on restart since they depend on in-flight interactions. **`/rebuild`** with Isolated Preview open restores that pane after relaunch. Isolated Preview stays until Esc, Exit, or Approve. Isolated Preview is not a Plan Exit timer. That restore does not cancel nested implementers. Exclusive `/plan` exclusive-blocks nested implementers. `/plan --soft` keeps nested implementers running.

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
