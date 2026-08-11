# Plan Mode & Permissions

Grok asks before doing anything risky — and can plan before it codes.

## Permissions

When Grok wants to run a risky command or edit a file, it pauses and asks:
allow once, always allow that kind of action, or deny.

Reading is always free: file reads, searches, and safe read-only commands
(`ls`, `git status`, `grep`, …) never prompt. Chained commands are
checked piece by piece — `ls && rm -rf tmp` still prompts for the `rm`.

Trust the session? `/always-approve` (or `Ctrl+O`) skips **tool permission**
prompts. It does **not** auto-approve a plan: plan decisions stay on the plan
panel CTAs.

## Plan mode

For bigger or more ambiguous tasks, use **plan mode**: Grok explores the
codebase read-only, designs an approach, and **presents** a plan for you to
approve *before* any code is written. Agent tool `exit_plan_mode` means
“plan ready for review,” not “already approved.”

- **`Shift+Tab`** (prompt focused) cycles the mode: Normal → Plan →
  Always-approve.
- **`/plan`** enters plan mode directly; `/plan <task>` plans that task in
  one step.

When the plan is ready, use the **side panel or soft-park strip** CTAs
(mouse primary): **Approve**, **Approve w/ comment**, **Clarify**, **Revise**,
**Quit**. With empty prompt focus on the panel, keys `a` / `A` / `?` / `s` /
`q` mirror those actions. Do not approve by freeform chat. After one decisive
Approve or Quit, decision chrome stays quiet until a **new** present.

A good habit: plan mode for "how should we even do this?", normal mode for
"just do it".

## Long-running commands

A build or test run hogging the turn? **`Ctrl+B`** sends it to the
background — Grok keeps working and you're notified when it finishes
(`Ctrl+G` shows the tasks pane).

*Go deeper: `/docs Plan Mode` or `/docs Permissions and Safety`*
