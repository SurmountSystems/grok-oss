# Plan: Test intent + red/green TDD + agent probe hygiene

## Context

Three related process failures around agent work (triggered by plan park B/C review):

1. **Throwaway `rustc` probe** left `rust_out` in the repo root. Reviewer tried to
   “prove” `u16::clamp` panics with a stdin compile. Real Rust workflows for this
   product use **`cargo test` / nextest**, not bare `rustc`. Operator has never
   used `rustc` directly in ~8 years of Rust work — agents should not invent that path.
2. **Fitting tests to code** risk: agents rewrite asserts so they pass the current
   implementation instead of asking whether the **test’s intent** (or product
   contract) is what must change. That is **assuming intent**.
3. **Green-only / skipped red** risk: implement “fix” and only then shape tests to
   match, or claim TDD without ever observing a **red** run. Host already says
   red/green for user bugs (`~/.grok/AGENTS.md` § *User-reported bugs & features*);
   this plan makes that **proper**, concrete, and global for implement/review —
   not a slogan.

**Operator corrections this plan must honor:**

- Prefer **unit/integration tests in the crate**, not ad-hoc compiler probes.
- **Do not gitignore `rust_out`.** Visibility is the point. (Revert applied:
  `.gitignore` has no `/rust_out` / `/a.out`.)
- Changing a unit test requires **high confidence** the test (or its contract) is
  supposed to change — not “make green.”
- **Proper red/green TDD**: fail first (observed), then minimal product change to
  pass; do not invert the loop.

**Non-goals**

- Product feature work (plan park B/C already shipped).
- Banning `cargo` / normal build tooling.
- Mechanical hooks that block every shell compile (too brittle; `cargo test` is fine).
- Dual OAuth or other parked product work.
- Ritual TDD for pure typo/docs one-liners (see exceptions below).

**Assumptions**

- Process pins live dual: host personas/AGENTS + project `AGENTS.md` for recon survival.
- Reviewers **suggest** the red test case; implementers **land** red then green
  (reviewers still do not fix product code).

## Approach

Tighten **three process laws** and wire them into personas, host AGENTS, project
AGENTS, and implement skill. No product runtime code. No gitignore for probe binaries.

```
  contract (intent) ──► RED (observed fail) ──► GREEN (minimal product fix)
         ▲                      │                        │
         │                      │                        │
         └──── park if unclear ─┴── never reshape test ◄─┘
                                   to “fit” code
```

### A — Proper red/green TDD

**Law:** For behavior changes and bug fixes, the proof order is **red → green →
(optional) refactor**. Green without a prior observed red is not TDD.

#### What “red” means

| Requirement | Detail |
|-------------|--------|
| **In-tree test** | A real `#[test]` / integration / contract test under the crate (or nextest filter), not a shell one-liner, not bare `rustc` |
| **Observed fail** | Implementer (or CI) **ran** the test and saw it fail for the **named contract reason** — not “it would fail if we had a test” |
| **Named contract** | Plain-language: what user-visible or API behavior is wrong/missing |
| **Logged** | Impl summary notes: test name + command + that it failed before the product edit |

#### What “green” means

| Requirement | Detail |
|-------------|--------|
| **Minimal product change** | Smallest production edit that makes the red test pass |
| **Same test body** (usually) | Do **not** edit the red test’s expectations to pass; fix code |
| **Re-run** | Same test filter green; no unrelated suite required for the micro-loop |
| **Summary** | Note green command + that the **same** test now passes |

#### When red/green is **required**

- User-reported bugs / regressions
- New product behavior (features, UX contracts)
- Review **bug** issues that need a regression guard
- False-green fixes: first make the assert correct (**red** if code is still wrong;
  if code is already right, the tightened assert should **pass** — that is a
  test-quality fix, not a free pass to weaken elsewhere)

#### When a lighter path is OK (exceptions — still no bare `rustc`)

- Pure docs, comments, formatting, renames with no behavior change
- Wiring that is covered by an **existing** red test you re-run (extend green only
  after confirming the existing red still fails for the right reason if you are
  fixing that bug)
- Operator explicitly says skip TDD / “just fix the typo”

If unsure whether behavior changes → treat as **required** red/green or **park**.

#### Anti-patterns (explicit ban)

| Ban | Why |
|-----|-----|
| **Green-only drive-by** | Product edit first, tests only if CI complains |
| **Test-last that rewrites expectations** | Fitting tests to code (see § B) |
| **Claimed red without running** | “Would fail” is not red |
| **Red via bare `rustc` / toy binary** | Wrong tool; leaves junk; not the project test suite |
| **One mega-test after the whole feature** | Prefer smallest red for the slice; expand coverage after green |
| **Delete red test when green is hard** | Park or split; do not erase the contract |

#### Role split

| Role | Red/green duty |
|------|----------------|
| **Reviewer** | Suggest the failing test (name, setup, assert, expected fail mode). Do **not** implement product fix. May note missing red as a review issue. |
| **Implementer** | Land the test → run → **observe red** → product fix → **observe green**. Document both in summary. |
| **Orchestrator (`/implement`)** | Reject “done” summaries that only show green or that changed tests without naming contract + red evidence when TDD was required. |

### B — Test-change discipline (do not fit tests to code)

**Law:** A test encodes an intended contract. Editing it is a **behavior/intent
decision**, not a way to finish green.

| Situation | Correct move |
|-----------|----------------|
| New product behavior (TDD) | **Red first:** write test for the new contract (fails) → implement → green |
| Bug fix; existing test still right | Keep test; fix production code (re-run: red if bug present, then green) |
| Missing regression test | Add **new** red test → fix → green; do not only “adjust” an unrelated test |
| Existing test is **false-green** / wrong assert | Fix the **assert** so it measures the real property; document why; if product still wrong, that tightened test should go **red** until product fix |
| Existing test fails after a change and **intent is unclear** | **Park** — do not rewrite the test to match code; `ask:*` / residual / ask operator |
| Reviewer wants more coverage | Suggest case; implementer adds as red→green when behavior is in scope |

**Anti-patterns (explicit ban)**

- Change assert so CI passes without stating contract change.
- “Code does X now, so the test should expect X” without product/spec intent.
- Weakening asserts (looser `contains`, dropping cases) to silence failures.
- Deleting tests that fail after a refactor without named reason + replacement.
- Using a test edit as a substitute for a product fix in the green step.

**When an implementer *may* change a test (confidence bar)**

All of:

1. **Named contract** — summary or review issue states intended behavior in plain language.
2. **Evidence** — code path + (when relevant) user/spec/prior test name agrees.
3. **Stronger or equal check** — false-green fixes get **more precise** asserts (exact equality, before/after snapshot), not looser ones.
4. **TDD order preserved** — if the change is behavior, red observed before green; if the change is only assert precision and product is already correct, state that explicitly (no fake red).
5. **If unsure → park**, do not ship a rewritten expectation.

**Reviewer duty:** Flag “test rewritten to match code” and “green without red” as
**bug** or **suggestion**. Flag false-green asserts (always-pass).

### C — Probe / verification hygiene (four layers; no gitignore)

| # | Layer | Do | Do **not** |
|---|--------|-----|------------|
| **1** | **Norm (personas)** | Red/green via **crate tests** (`cargo test -p …`); read code; suggest `#[test]` | Bare `rustc` / `gcc` / stdin compiles; toy probes as proof |
| **2** | **Process pins** | Host + project AGENTS: hygiene + test-intent + red/green law | Claim gitignore as safety net for agent junk |
| **3** | **Visibility (not ignore)** | Leave junk visible in `git status` so humans catch it | `.gitignore` for `rust_out` / probe ELFs |
| **4** | **Memory / loop** | Implement-memory + implement skill: require red evidence in summaries when TDD applies | Silent “make tests match”; claim TDD without a red log line |

**Why never bare `rustc` here**

- Cargo workspace: compile+prove with **`cargo test` / nextest**.
- `rustc -` → binary name **`rust_out`**, multi-MB ELF in cwd, no workspace deps/lints.
- Stdlib doubt (`clamp` panics when min > max): **read docs** or add an **in-tree
  unit test** (red if product misuses clamp, green after safe clamp). Do not compile a toy crate.

**Rare escape (almost never):** language-runtime curiosity only under `/tmp`,
unique names, delete source+binary same command — and still prefer a suggested
or real unit test. This is **not** a substitute for the red step of TDD.

### D — Align already-partial pins

Already done (keep, then refine on implement):

- Reviewer persona: “Proving bugs” (forbid workspace probes; suggest tests).
- Implementer persona: prefer project tests; no leftover `rust_out`.
- Host AGENTS § *Workspace hygiene* — **drop** any “gitignore backstop” wording.
- Host AGENTS § *User-reported bugs & features* — already says red/green; **extend**
  with observed-red requirements and link to test-intent.
- Project AGENTS #13 — point at hygiene + test-intent + TDD.
- `.gitignore` probe rules — **already reverted**.

On implement, also:

- Add **Test intent** + **Red/green TDD** sections (host AGENTS; both personas;
  project AGENTS; implement skill implementer duties + summary checklist).
- Soften “/tmp probe OK” so it does **not** normalize bare `rustc`.
- Implement-memory: patterns for proper TDD, false-green, no test-reshape, no rustc.

## Critical files

| Path | Why |
|------|-----|
| `~/.agents/skills/shared/personas/reviewer.md` | Suggest red tests; flag reshape + missing red |
| `~/.agents/skills/shared/personas/implementer.md` | Red observed → green; confidence bar for test edits |
| `~/.grok/AGENTS.md` | Hygiene (no gitignore) + test intent + proper red/green |
| `AGENTS.md` (project) | Branch-surviving hard constraints |
| `~/.agents/skills/implement/SKILL.md` | Orchestrator: summary must show red then green when required |
| `~/.agents/skills/plan/SKILL.md` (light touch) | Plan handoff / verification bullets mention red→green for behavior work |
| `.gitignore` | Confirm **no** `rust_out` ignore |
| `~/.grok/implement-memory/…` | Flush patterns after pin |

## Reuse

| What | Where | How |
|------|--------|-----|
| Ambiguity → park | Host AGENTS | Unclear test intent / unclear whether behavior changes |
| User bugs → TDD red/green | Host AGENTS § bugs & features | Expand to **observed red**, full implement loop |
| Implement skill TDD mention | `implement/SKILL.md` | Make checklist concrete (commands + same test) |
| False-green example | plan park soft CTA draft-guard | Canonical “stricter assert,” not looser |
| Value order code+tests > docs > git | Project AGENTS | Honest tests + real red |

## Steps

1. **Revise workspace-hygiene pins** — No gitignore backstop; visibility intentional;
   **never bare `rustc`**; proof via `cargo test` / suggested `#[test]`. Confirm
   `.gitignore` clean of `/rust_out`.
2. **Add red/green TDD law** — Host AGENTS (expand bugs/TDD section or sibling);
   implementer persona: red observed → green; reviewer: suggest red, flag missing red;
   implement skill: summary checklist (`RED: <cmd> <test> failed because…` /
   `GREEN: <same> passed`).
3. **Add test-intent law** — Confidence bar; park-if-unclear; ban fit-to-code;
   link to TDD (green step does not rewrite the red test’s contract).
4. **Light plan-skill note** — Behavior work in plans should list verification as
   red→green when applicable (not only “run tests at end”).
5. **Project AGENTS** — Hard constraint / pointer for TDD + test-intent + no probes.
6. **Memory flush** — Generalized patterns (observed red; no reshape; no rustc).
7. **Optional** — Human commits host skills tree when ready (agent never commits).

## Risks

| Risk | Mitigation |
|------|------------|
| Agents still reach for `rustc` | Personas + AGENTS; no friendly “/tmp rustc” default |
| Fake red / skip run | Summary requires command + observed fail reason |
| Parking forever instead of fixing | Park only when intent unclear; clear contracts still TDD |
| Over-rigid “never change tests” | Allowlist: new red tests, false-green stricter asserts, named contract changes |
| TDD theater (useless tests) | Red must fail for the **named contract**, not a tautology |
| gitignore reintroduced | Explicit ban in pins |

## Verification

- [ ] `.gitignore` has no `/rust_out` / `/a.out` probe rules.
- [ ] Host AGENTS: hygiene without gitignore; test-intent; **proper red/green** (observed red).
- [ ] Project AGENTS matches: no bare `rustc`, no test-reshape without intent, TDD when required.
- [ ] Reviewer + implementer personas: all three laws; no “gitignore backstop.”
- [ ] Implement skill: summary red/green checklist for behavior work.
- [ ] Plan skill: light verification note for red→green.
- [ ] Grep: no “ignore rust_out” claims in pins.
- [ ] No product feature code required for this plan.

## Open questions

None blocking. Optional later (parked):

- PreToolUse deny for bare `rustc` outside `/tmp` — only if personas fail in practice.
- CI fail if `rust_out` present at repo root — human-visible dirty tree preferred first;
  CI only if the junk keeps returning.
