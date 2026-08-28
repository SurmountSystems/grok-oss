# Agent rules — Surmount Grok OSS

Concise rules for work in this repository. Global GPG and subagent rules also
apply (`~/.grok/AGENTS.md`).

## Document hierarchy (D0–D3)

Not the same as session-board L0–L2 (todos / reports). Doc layers:

| Layer | Role | Always load? | Paths |
|-------|------|--------------|-------|
| **D0** | Open residual only | Yes (open section) | [`RESIDUAL.md`](RESIDUAL.md) |
| **D1** | Standing law | Yes (compressed) | This file; `~/.grok/AGENTS.md`; [`FORK.md`](FORK.md) |
| **D2** | Append logs | No — append / read tail | [`docs/upstream-*`](docs/), `doc/dev/campaigns/` |
| **D3** | Deep strategy / research | When needed | `doc/dev/research/`, host skill `references/` |

**Never** append recon diaries or campaign closed writeups into D1 AGENTS —
use D2 logs. Closed residual history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

Write new short reports under `~/.agents/reports/` on this machine. Agent
reports stay on the local machine. They are not part of the git tree. Session
board L1 todos stay on the session board; that board is not the reports home.

### Project agent home — single source of truth (pinned 2026-07-31; reports local 2026-08-17; plans and leftover joins local 2026-08-18)

Do not add a second home. Project `.agents/` is **not** the live plans,
leftover-joins, or reports home. Live **plans** live under
`~/.agents/plans/` (use `~/.agents/plans/<project>/` only when two
projects would collide on the same filename). Leftover joins were leftover
reports; new reports stay under `~/.agents/reports/`. Do not recreate
project `.agents/reports/`, `.agents/plans/`, or `.agents/joins/` as a
live home. Never create project-root `.grok/` for reports, plans, or
scratch. Session `plan.md` under `.grok/sessions/` is product/session
state and does not move to `~/.agents/`. Host dual-pin:
`~/.grok/AGENTS.md` § *Project agent home*.

## Product priority (value order)

**Code + tests > docs > git.** Docs matter more than git ceremony; docs matter
less than product code and tests. Do not invent long essays or git nags.

## Hard constraints

1. **Never run `git commit`.** Commits are human-only on a real TTY (signed).
   Agents may do complex git labor when asked (conflict resolve, merge setup,
   history diagnosis), then **hand** exact `git add` / `git commit -S …`
   commands — including after “fix conflicts” / “make the PR mergeable” /
   even “commit this.” Do **not** push unless the operator explicitly asked to push.
   Full policy: `~/.grok/AGENTS.md` § *Commits — agents never commit*.
1a. **Never `git add` / stage unless the operator explicitly asked.** Do not
   stage “to clean up,” after `cargo fmt`, after conflict resolve, or “to help
   the commit.” Leave the index alone; report paths and hand `git add …` if
   useful. Exception only when the operator clearly ordered staging (e.g. “stage
   these files,” “git add the fix”).
2. **Never bypass GPG** (`commit.gpgsign=false`, `--no-gpg-sign`, fake
   `gpg.program`, hook disables, etc.).
3. **Never bulk find-and-replace.** Bulk **find** (`rg`) is fine. Edits must
   be surgical and reviewed in context.
3a. **Structured Rust edits format and lint the written file (pinned 2026-08-15).**
   Product, not a process slogan: after ACP `search_replace` / `apply_patch`,
   the edit tool formats and lints that `.rs` file (infer from path). See
   [`FORK.md`](FORK.md) § *File-level infer-from-path verify*. Kill switch:
   `GROK_SKIP_EDIT_VERIFY=1`. Quality `cargo fmt --check` is a hard miss
   (rustfmt `Diff in` is not a flake 502). Host dual-pin: `~/.grok/AGENTS.md` §
   *Structured Rust edits*.
3b. **Do not prove product work with crate-wide cargo via subagents (pinned 2026-08-15).**
   Do not launch **local** crate-wide cargo (`cargo clippy -p ... --lib`,
   `cargo fmt -p`, `just check`) through extra subagents to prove a product
   slice on this laptop. One agent per job. No duplicate red-test pairs. No
   extra file-level edit-verify swarm. Implement-time proof is the
   named fixture tests the operator runs on the VPS (`just test-remote -p xai-grok-tools --lib rust_edit_verify`).
   Ordinary post-impl mop stays one L2 after a finished slice (not a swarm;
   that L2 spawns L3). Dual-pin: host `~/.grok/AGENTS.md` § *Do not prove
   product work with crate-wide cargo via subagents*.
3b-remote. **The operator owns the VPS builder (pinned 2026-08-25).**
   Agents must not invoke `just check-remote`, `just test-remote`,
   `just cargo-remote`, or force-remote `nix build` to `nixbuilder` /
   surmount-1. That host is the operator's. Agents implement product and
   tests in the tree. When the operator pastes a quality fail, that paste
   is the contract. Do not start a second gate on the same drv or the
   same leftover list. Agents still must not run `cargo test`,
   `cargo clippy`, `cargo build`, or rustc on this laptop. File-level
   rustfmt-only may stay local if it does not invoke rustc. GitHub
   Actions must not call `check-remote`, `test-remote`, or `cargo-remote`.
   The 2026-08-22 "agents may run `just check-remote` whenever useful" line
   is superseded. Force-remote recipe details (max-jobs 0, `--store`
   ssh-ng, `--eval-store auto`, `--cores 64`, cargo jobs cap 32) still
   describe how the operator's recipes work. Host dual-pin:
   `~/.grok/AGENTS.md`.
3b-remote-named. **Named cargo on the VPS is operator-run (pinned
   2026-08-22; operator-owns 2026-08-25).** The recipes stay
   `just test-remote` / `just cargo-remote` on surmount-1. Agents do not
   run them. Agents do not run cargo/rustc on this laptop either.
   `GROK_SKIP_EDIT_VERIFY` is the kill switch for the edit-tool verify,
   not the default. Host dual-pin: `~/.grok/AGENTS.md`.
3b-remote-clippy. **Remote quality clippy must use many workers (pinned
   2026-08-23).** Do not invoke `cargo clippy` on `workspace-cargo-quality`.
   That external binary lets the outer cargo start a 1-token jobserver;
   inner `--jobs` is then ignored and the operator sees one clippy-driver
   with idle cores. Lint with builtin `cargo check` plus
   `RUSTC_WORKSPACE_WRAPPER=clippy-driver` under a GNU make jobserver with
   `$CARGO_BUILD_JOBS` tokens (from `--cores 64`, cap 32). One
   clippy-driver process is still one typeck thread; independent crates
   must run at once. Passing `--jobs` on `cargo clippy` is not the fix.
   Host dual-pin: `~/.grok/AGENTS.md`.
3b-complaints-are-work. **Operator complaints are work to fix (pinned
   2026-08-23).** When the operator complains twice about the same broken
   chrome or gate, fix the thing they can see. Do not answer the second
   complaint with "we already set jobs." Host dual-pin: `~/.grok/AGENTS.md`.
3b-say-the-other-path. **Tell the operator when it is a different path
   (pinned 2026-08-23).** If the right fix is not a patch in this tree,
   say that in the same-turn status, on **You**, with evidence. Name the
   other path in ordinary English: this laptop's Nix install, the VPS
   builder host, wait until not on cellular, rebuild `grok-oss` to see
   a TUI change, an operator TTY. Do not send them to the VPS for a
   local PATH miss. Do not pretend a grok-build edit will fix a host
   they must repair. Host dual-pin: `~/.grok/AGENTS.md`.
3b-do-not-get-overwhelmed. **Large red lists: methodical, not overwhelmed
   (pinned 2026-08-23).** A long nextest or clippy list is not a reason to
   rush, weaken tests, or bulk-rewrite. Work one documented contract at a
   time against [`FORK.md`](FORK.md) and the named test's intended
   behavior (hard constraint 15: do not fit tests to code). Group fails
   by contract. Distinguish product misses this session caused from a
   known Nix-sandbox nextest class (S3, MCP, bwrap). Do not treat 177
   red rows as 177 independent panics. Do not allow-lint or delete asserts to shrink
   the list. Host dual-pin: `~/.grok/AGENTS.md`.
3c. **Implementer ↔ reviewer swap each feature.** For each **new** feature or
   independent implement slice, swap who implements and who reviews (roles
   fixed within one feature’s fix/re-review loop). Host dual-pin:
   `~/.grok/AGENTS.md` § *Implementer ↔ reviewer swap*.
3d. **One review job per slice (pinned 2026-08-15; thoroughness 2026-08-16).**
   Do not launch three (or more) visible Review agents for the same
   slice because implement `--effort 3` has three slots. Token Economy
   and implement `--effort` are **thoroughness**, not reviewer count.
   One reviewer unless the operator **explicitly** asked for more than
   one. Three lookalike "Review ..." rows in the Subagents list is a
   process violation. Dual-pin: host `~/.grok/AGENTS.md` § *One review
   job per slice*.
3e. **Document every live task on disk (pinned 2026-08-15; re-pinned
   same day).** Always remember: every live task must be documented so
   it survives context compaction. That is standing process law, not a
   reminder. Chat status is not enough. Same turn the task becomes
   real, write it in all of these that apply:
   1. Session board: namespaced item and a **bare remaining-work
      pointer** (one owed outcome). Not a paste of the user message.
      Not a session novel.
   2. Write the short report under `~/.agents/reports/` on this
      machine and/or an Open residual bullet only when the remaining
      contract must survive a compaction. Keep it a pointer, not a
      diary. Agent reports stay on the local machine. They are not
      part of the git tree.
   3. This file or host `~/.grok/AGENTS.md` when it is process law.
   After a compaction, disk wins. If a job is only in chat, it was
   never documented. Host dual-pin: `~/.grok/AGENTS.md` §
   *Document every live task on disk*. Remaining-work pointer this
   session: `~/.agents/reports/remaining-2026-08-19-grok-oss.md`.
3f. **L1 stays lean so we do not compact (pinned 2026-08-17; L1 500k
   2026-08-20; compact split 2026-08-21).** The main (L1) session uses
   the catalog 500k sampling window. AUTO compact on L1 uses that
   window, not the old 200k knee. Nested L2 sampling stays 200k. L2
   may compact. L3 never compact. An L3 is disposable. If it stalls
   or spirals, kill it. When an L3 is near 200k, it summarizes,
   reports to L2, and stops. Do not compact-and-continue on L3.
   Keep L1 near about 40% of that 500k window, and keep nested
   sessions near 40% of their 200k window. Compaction is expensive
   and slow. Avoid filling L1: board pointers, spawn, wait, read the
   asked-for report. Do not load inventories, closed todos, or session
   history into L1. When the operator names work, record a pointer
   and implement. After Approve, do not block on another plan present.
   Track the work, write a size estimate, implement the groups in
   parallel, then reconcile the estimate against what landed. See
   § *After Approve, implement mentioned work*. Plan mode is for
   unclear or large new design only. Host dual-pin: `~/.grok/AGENTS.md`
   § *L1 stays lean so we do not compact*.
4. **Talk to humans in plain language.** No pack of opaque acronyms, false
   either/or menus, or planning jargon (phases, tracks, workstreams) in user
   replies, product docs, tests, or **filenames**. **No bare plan-step codes**
   (`S1`, `S3`, `D3`, `B2`, …) in residual, chat, reports, **or identifiers**
   without the plain name next to them — agents and humans misread them (e.g.
   **S3** ≠ Amazon, **B2** ≠ Backblaze).
   **Names are product (pinned 2026-07-27):** **file and directory names first**
   (they stand out in git, PRs, and search), then variables, functions, modules,
   test names, and user-visible enums. Thoughtful, readable, plain meaning
   first. Not clever jargon or step codes in paths/symbols (`b2_order.rs`,
   `proactive_hop`). Prefer names a tired reader can parse. Wire/protocol
   fields we do not own may keep upstream spelling; our files, wrappers, and
   tests still get clear names.
   **Sapient Experience (stance pointer):** speak to humans as humans do; do
   not try to *be* human. Honesty with texture: encourage, acknowledge,
   support, never lie. Not a fake inner life. Not brick-robotic. Full pin:
   host `~/.grok/AGENTS.md` § Sapient Experience; open residual
   [`RESIDUAL.md`](RESIDUAL.md) §2f. Do not dump novels here.
   **Billing meters stay distinct:** SuperGrok dollar credits ≠ included SuperGrok
   period limits ≠ console team prepaid / console API credits. Name which meter.
   **Limits and credits vocabulary (pinned 2026-08-08; SuperGrok is paid 2026-08-13; omit extras 2026-08-16):**
   Omit the word extras in user-facing copy, residual, reports, board titles,
   comments that humans read, and process law. Do not teach extras as a nickname.
   Say **limits** not bare "allowance". When the prepaid SuperGrok top-up meter is
   meant, say SuperGrok dollar credits. That is not included SuperGrok period
   limits and not console team prepaid / console API credits. SuperGrok is a
   **paid** product. Never call SuperGrok free. When the meter matters, use the
   full name: included SuperGrok period limits; SuperGrok dollar credits;
   console team prepaid / console API credits. Desired spend order (docs and
   comments): included SuperGrok period limits first, then SuperGrok dollar credits,
   then console team prepaid / console API credits. While included SuperGrok period
   limits still have room, stay on SuperGrok session and do not make the console
   API key primary. Never invent included SuperGrok period used % on the client.
   **Do not report included SuperGrok period limits as used up (pinned
   2026-08-17).** Never tell the operator a SuperGrok included pool is
   full unless the live product Usage view or `/limits` surface they can
   see agrees, or a named live fetch of that same named meter agrees.
   A subagent snapshot is not enough to override the operator. If they
   contradict a percent, retract it the same turn and diagnose. Do not
   pad a guess. Name which meter and which workspace. SuperGrok Heavy
   is a distinct weekly pool from standard SuperGrok. Dual-pin: host
   `~/.grok/AGENTS.md` § *Limits and credits vocabulary*.
   **grok-oss limits printout is not xAI billing truth (pinned 2026-08-19).**
   grok-oss limits JSON and compact chrome are a client printout, not xAI
   billing truth. Distinguish "the CLI printed X" from "the account is X."
   Identical `nextReset` or identical included % on SuperGrok (personal)
   and SuperGrok (business) is not proof those identities share a pool or
   reset together. The operator signed up for personal SuperGrok and
   business SuperGrok at different times, so those included SuperGrok
   period weekly windows cannot honestly share one reset clock. Operator
   Usage (grok.com / product Usage for that workspace) and the
   console.x.ai Billing page they can see win when they contradict the
   CLI. `console.isLive` false is sampler identity, not "credits are not
   being used." Do not invent remaining. Do not call any pool used up.
   Dual-pin: host `~/.grok/AGENTS.md` § *Limits and credits vocabulary*.
   **Fetch live named meters this turn (pinned 2026-08-22).** Do not reuse a
   last-seen chat or screenshot dollar figure as if it were current. Same
   turn, run a named live fetch (`grok-oss limits refresh` then
   `grok-oss limits --json`, or the live product Usage / `/limits` surface
   the operator can see). Name which meter and which workspace. If a
   surface is `not_fetched` (the grok.com / console.x.ai Billing Credits
   card still is), say that and report the named JSON fields you actually
   got. Do not invent that card from SuperGrok dollar credits or console
   team prepaid remaining. grok-oss printout is still not xAI billing
   truth. Dual-pin: host `~/.grok/AGENTS.md` § *Limits and credits
   vocabulary*.
   **Fail-open plus named `/limits` commands (pinned 2026-08-19).** A client
   100% / remaining 0 / SuperGrok dollar credits $0 printout must not mark
   SuperGrok used up or hop to console so this session cannot self-fix.
   Operator Usage (grok.com for that workspace) and the console.x.ai Billing
   page they can see win. Real SuperGrok HTTP 402 after that request failed
   can still leave SuperGrok. Hop-back does not require console credits.
   Same words on TUI `/limits` and CLI `grok-oss limits`: stay-supergrok,
   use-console, meter included | dollar-credits | console | combined,
   refresh (ForceRefresh). Sidecar `$GROK_HOME/limits_pins.json`, sibling of
   exhausted_credits/. No new `[auth]` keys. Stock preferred_method =
   "api_key" still pins console. Matching nextReset is not proof of a
   shared pool. Dual-pin: host `~/.grok/AGENTS.md` § *Limits and credits
   vocabulary*.
   **SuperGrok Heavy (operator-reported 2026-08-16; not a product-code proof):**
   SuperGrok Heavy is a real tier, distinct from standard SuperGrok. Personal
   SuperGrok Heavy and Business/Team SuperGrok Heavy are separate weekly compute
   pools. They do not combine. Switching workspace switches which pool is drawn
   from. Standard Business seats are SuperGrok. Heavy is an explicit upgrade.
   xAI does not publish fixed numeric quotas. Remaining percent is in the product
   Usage view for that workspace. The operator has SuperGrok Heavy and does not
   see it used. Board `bug:supergrok-heavy-not-used` owns the diagnose. This pin
   does not diagnose product code.
   **Complete thoughts (pinned 2026-08-03; leftover lists 2026-08-28):** Plans,
   residual, reports, board titles, remaining-work pointers, user-facing docs,
   and chat about product work must use **complete American English thoughts**.
   Do not use half-labels as if they were sentences (wrong: "SuperGrok included
   weekly"). Right: "the included SuperGrok period limits for the current
   billing period (how much of that included quota is already used)." Numbered
   leftover lists, Job / State / You / Next lines, and remaining-work bullets
   each need a subject and a finite verb. Wrong: "This running TUI until you
   install." Right: "This running TUI will not show the plan-composer fixes
   until you install grok-oss and reopen the session." When naming a meter,
   say what it is and what it is not. Conditions use full clauses (not
   "room/headroom"). Config and wire names may follow the plain thought in
   parentheses. Operator corrections about incomplete phrasing are permanent
   law. Host dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **Job / State / You / Next, with evidence for You (pinned 2026-08-21;
   other path 2026-08-23):**
   Keep the four-line restatement. On **You**, always say why the
   operator must act, or why they need not. Name the evidence. If the
   work is not this tree, say the other path there. Do not
   use an unexplained heuristic. Maximally truthseeking. Host dual-pin:
   `~/.grok/AGENTS.md` § Prose + tone; skill `~/.agents/skills/what/SKILL.md`.
   **Wait times in minutes (pinned 2026-08-16):** When reporting a wait of a
   minute or more to the operator, write minutes (or hours and leftover
   minutes). Do not write 943 seconds or 943s. Compact chrome is `15m43s`
   for 943 seconds, `1h2m` for 3725 seconds. Times under 60 seconds may
   stay in seconds, including the model turn timer and Thought for N.s.
   SuperGrok is a paid product. Do not teach extras as a nickname. When
   the prepaid SuperGrok top-up meter is meant, say SuperGrok dollar
   credits.
   **No bad metaphors, no sloppy language, no imprecision (pinned 2026-08-09):**
   Accurate, precise, concise natural American English. Say what the product
   actually does. No invented metaphors that name things not in the product
   (e.g. "media player pause" when there is no media player), no clever
   analogies the reader must decode, no vague handwaves when two paths differ.
   Real control, path, or outcome first. **No void/gap/jargon padding** —
   short concrete sentences only (host pin: *Speak like a normal precise
   person*). Host dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **No nicknames (pinned 2026-08-22):** Operator: I hate nicknames. No
   nicknames, now or ever. Speak clearly always. Chat, board titles, spawn
   descriptions, reports, residual, plans, and status must name the real
   thing in ordinary American English. Do not invent a nickname for a set of
   files, a log bundle, a job, a meter, or a UI control. Wrong: "the packet"
   for the saved `just check-remote` logs. Right: name the files or say
   "the saved `just check-remote` logs." Do not say **nits** or **mop** to
   the operator. Wrong: "I tracked the nits. I did not spawn a mop." Right:
   name the leftover comments, tests, or chrome and do that work. Host
   dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **ISA vs cores vs cargo targets (pinned 2026-08-23).** aarch64 versus
   x86_64 is instruction-set architecture (ISA), not "extra CPUs." CPU in
   that sentence reads as cores or VM size (GitHub `CI_LOW_MEM` versus
   `just check-remote`). `cargo clippy --all-targets` is test harnesses,
   examples, and benches. It is not another ISA and not more cores. Host
   dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **No lossy job nicknames (pinned 2026-08-15):** Chat, board titles, spawn
   descriptions, and status lines are operator-facing. Do not mash process
   slang with a tool name into a nickname (wrong: "Red bash"). Say the real
   job in ordinary English (right: "failing tests that the shell tool must
   refuse crate-wide cargo"). The spawn description is what the Subagents
   list shows. Host dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **Acknowledge merit, not sycophancy (pinned 2026-08-20):** The operator
   likes explicit acknowledgment when an idea has merit. That is not
   sycophancy. When an idea is actually good, say so in plain English and
   say why (what it solves). Do not flatter, inflate, agree by default, or
   pad empty praise. Host dual-pin: `~/.grok/AGENTS.md` § Prose + tone.
   **Self-improving feedback loop (pinned 2026-08-03):** trigger phrases such as
   "always remember", "please remember", "I hate repeating myself" (and close
   variants) mean same-turn standing pin (project `AGENTS.md` / residual when
   product-specific; host `~/.grok/AGENTS.md` when cross-repo). Prefer a short
   named subsection. Chat alone does not survive compaction. Full host pin:
   `~/.grok/AGENTS.md` § *Self-improving feedback loop*.
   **Write that down (pinned 2026-08-22):** when the operator explicitly
   says "write that down", same turn put the fact in the useful place
   (report, plan, residual, process note, user-guide) and then track the
   work on the session board (and spawn if it is product). Chat alone is
   not enough. Host dual-pin: `~/.grok/AGENTS.md` § *Write that down*.
   **Citation standard (pinned 2026-08-03):** docs and non-trivial comments that
   rely on external rate limits, APIs, or vendor policy need a markdown link to
   the public page plus **accessed: YYYY-MM-DD**. Example: See
   [xAI Rate Limits](https://docs.x.ai/developers/rate-limits)
   (accessed: 2026-08-03). Host dual-pin: `~/.grok/AGENTS.md` § *Citation
   standard for external limits and policy*.
   **Plan revise (pinned 2026-08-03):** on plan panel **Revise**, rewrite
   session `plan.md` and re-present; do not invent `ask:*` queues as a
   substitute. Idle CTAs are Approve / Comment / Revise / Exit. **Comment**
   is the hub. **Clarify** only after Comment, and is answer-only. Host:
   `~/.grok/AGENTS.md` § *Plan approval* item 8.
   **Plan present ≠ Approve (pinned 2026-08-10):** `exit_plan_mode` tool
   success and “Plan ready” soft-park are **present for review**, not operator
   approval. Always-approve is tool permissions only. Empty freeform Enter
   never approves (clickable Approve). After one decisive
   Approve or Exit, do not re-arm CTAs until a new present. After Revise or
   Clarify, wait for re-present (no idle “Plan written / Click or /view-plan”
   CTA re-arm mid-rewrite). After a real Approve, work starts. Do not write a
   new session plan or re-present to "confirm" mentioned work while that
   approved implement is in flight. See § *After Approve, implement mentioned
   work*. User-guide `19-plan-mode`; FORK plan-approval
   bullets P1–P2.
   **Dual-auth language (pinned 2026-07-27; vocabulary 2026-08-08; omit extras 2026-08-16):** ban bare
   jargon **proactive hop**, **sticky exhaust** / **sticky hop**, and
   **dual-host** without plain explanation. Prefer: *mark SuperGrok used up from
   billing % / leave SuperGrok when included SuperGrok period limits are full*;
   *stay on the console key after switch / remember this SuperGrok identity is
   out of included SuperGrok period limits*; *also switch the API host (SuperGrok
   proxy ↔ `api.x.ai`)*. Residual, reports, comments, tests, and **identifiers**
   use the plain names. Prefer **limits** over bare "allowance". When the prepaid
   SuperGrok top-up meter is meant, say SuperGrok dollar credits (see **Limits
   and credits vocabulary** above). (UI sticky headers / permission sticky cursor
   are unrelated product terms. Leave those alone.)
5. **Never ask permission to continue clear work.** If the goal is known
   (finish the onto stack, resolve conflicts, keep going), **do the next step**
   — do not end with “say the word,” “want me to continue?,” or similar. Ask
   only when intent is genuinely ambiguous or an irreversible external action
   needs confirmation (push/PR when not already requested). A dirty mid-pick
   tree is unfinished work, not a pause for ceremony.
5a. **Complete plan verticals (pinned 2026-08-07).** Do **not** invent
   “parked / enough for now / optional later” for steps that are in an
   **approved** plan (or clearly in-scope residual) unless the operator
   **explicitly** defers that slice. Surmount does complete work. True
   *Ambiguity → park* stays for unclear intent only, not for optional-feeling
   follow-ons on a locked plan. After Approve, mentioned work is implemented
   (board + spawn), not parked behind another plan present. See § *After
   Approve, implement mentioned work*. Host dual-pin: `~/.grok/AGENTS.md` same rule.
6. **Prefer Rust tools; do not invent and run new Python/shell scripts**
   (pinned 2026-08-09; supply-chain). Prefer product/host **Rust tools and
   bins** for agent work. Agents must **not write and execute new Python
   scripts**, and must **not invent and execute ad-hoc shell scripts that
   download or run untrusted code**, for agent tasks. Active **supply chain
   attacks on the Python ecosystem** make agent-authored `python3` / `pip` /
   one-off `.py` payloads a real risk. **Shell tool for named product
   commands** (`cargo`, `just`, `cargo test`/nextest, `rg`, read-only git,
   existing in-tree scripts) is fine. **Writing** a new `.py`/throwaway `.sh`
   (or equivalent heredoc payload) and executing it for agent glue is not.
   **Narrow exceptions:** pre-reviewed office/docx/pptx/xlsx/pdf skill
   scripts under `~/.agents/skills`; allowlisted host helpers
   (`memory.py` / plan-validate / session_reader CLI forms; product may
   intercept to Rust); user-project Python when **their** product is Python;
   existing repo `just`/scripts. Do not invent alternate helpers. Do not
   paste inventories here. Host dual-pin: `~/.grok/AGENTS.md` § *Prefer Rust
   tools; do not invent…*; skill-rules rule 17; D2:
   [`doc/dev/research/python-to-rust-tools-2026-07-26.md`](doc/dev/research/python-to-rust-tools-2026-07-26.md).
   **Tools improve tools (pinned 2026-08-15).** It is wasteful for agents
   or tools to write disposable bash or Python as if those scripts were
   the product. Always better to improve the tools we already use. A
   lever: tools build better tools. Do not paper over a missing or
   vague API by writing a one-off `curl` (or equivalent) in a throwaway
   script. Prefer a named product tool surface (`search_replace`,
   `apply_patch`, `write`, native fetch, existing bins). When a surface
   is missing or wrong, extend that product tool. Do not invent a
   disposable wrapper around it.
7. **Friction → suggest plan** — process pushback / “plan first” → stop
   implementing and suggest or enter plan mode; explicit “just fix it” is fine.
   Host law: `~/.grok/AGENTS.md` § *Friction → suggest plan*.
8. **No questionnaire modals for plan clarifications** (re-pinned 2026-07-26).
   Do **not** use host multi-choice / `ask_user_question` widgets while
   planning. Put open questions **in the plan file** (and plain chat). Host
   skill `/plan` § hard rule 6; `--legacy` only if the operator opts in for that run.
9. **Do not invent “out of scope.”** Prior docs/plans saying “credit-only”
   or “soft-429 hop = No” are **defaults that can be wrong** for new intent.
   Verify code; if the operator names a real need (e.g. rate-limit failover),
   plan it — do not dismiss as out of scope from stale residual text.
10. **Ambiguity → park** (pinned 2026-07-26; pencils-down 2026-08-25). When
    intent is ambiguous, **pencils down**: stop working, ask the operator in
    **plain freeform**, do not assume. A contradiction (two names, two homes)
    is ambiguous. Do not resolve it silently. Track `ask:*` / plan open
    question / residual. No plan questionnaire modals. Host: `~/.grok/AGENTS.md`
    § *Ambiguity → park*.
11. **Git silence** (pinned 2026-07-26). No nags about stage/commit/push/PR/
    uncommitted trees in ordinary status. Engage git only for **complex**
    recon/upstream/onto/put-history when asked. Still never `git commit`.
    Host: § *Git silence*.
11a. **PR titles and descriptions (help only; human sets them)** (pinned
    2026-08-01). When the operator asks for PR title/body help:
    - **Do not** apply `gh pr edit` / set the title or body unless they
      **explicitly** ask to publish that text.
    - **Titles:** one clear product outcome (or two tightly joined). Prefer a
      short sentence or conventional `area: outcome`. Avoid theme shopping
      lists, deck prefixes (`Operator UX:`), and **internal** words on a
      public PR (`dogfood`, board ids, report paths, residual codes).
    - **Bodies:** complete sentences, plain American English, what changed for
      operators first. Theme sections are fine in the body; not as the title.
    - Match the professionalism bar in product prompts (good grammar, only
      relevant detail). Do not overstate operator reaction (“hate,” “furious”)
      when they only said something was bad or weak.
12. **Autonomy default** (pinned 2026-07-26). Prefer always-approve autonomy;
    keep Zed-style deny/ask filters and explicit approval via existing
    `permission_mode` + `[permission]` rules / hooks — do not invent a second
    system. Host: § *Autonomy default + filter/approval*; user-guide
    `22-permissions-and-safety.md`; `~/.grok/config.toml`.
13. **No throwaway compiler probes in the workspace** (pinned 2026-07-26).
    Prove bugs with project tests (`cargo test` / nextest) or a *suggested*
    unit test in review notes — never bare `rustc` / one-shot ELFs (`rust_out`)
    under the repo. Do **not** gitignore probe junk (visibility intentional).
    Reviewers suggest the test; implementers land red→green. Host:
    `~/.grok/AGENTS.md` § *Workspace hygiene*.
14. **Proper red/green TDD when behavior changes** (pinned 2026-07-26).
    Observed fail first (in-tree test, ran, named contract), then minimal
    product fix so the **same** test passes. Do not claim TDD without a red
    log line. Exceptions: pure docs/typos/format; operator says skip. Host:
    `~/.grok/AGENTS.md` § *Red/green TDD* + § *User-reported bugs & features*.
15. **Do not fit tests to code** (pinned 2026-07-26; named tests are
    contracts 2026-08-25; do not naively update tests 2026-08-27). Changing a
    test needs a named contract + evidence + stronger/equal assert; park if
    intent unclear. False-green → stricter assert, not weaker. Do **not**
    rewrite a failing assert so it matches the code. Reconcile the named
    contract with FORK and process first, then fix the product. Do not skip Nix-sandbox S3, MCP, or bwrap tests to
    go green. Hermeticity fixes (PATH bash, writable `$GROK_HOME`, webpki
    roots, bwrap placeholders, grok argv0 on Nix coreutils) keep the named
    contract. Dual-pin: [`FORK.md`](FORK.md) Land checklist **Named tests
    are contracts**. Host: § *Test intent*.
16. **No bash-in-nix; SHA-1 is git object ids only (pinned 2026-08-25).**
    Do not wrap old `.sh` in `pkgs.writeShellApplication` (or equivalent
    bash-in-nix). CI/Nix helper logic belongs in named `flake/*.nix` modules
    and `grok-nix-helper`, not copied bash inside Nix. Git recon is
    `grok-nix-helper` subcommands. Hand `git commit -S` to a human TTY.
    SHA-1 is for git object ids (gix, empty-tree, 40-hex commits) only. It
    is not a security hash for downloads or FODs. Artifact verify is
    SHA-256 or minisign. Helper logs must not print tokens, API keys, or
    secret env values. The operator owns `just check-remote`. Dual-pin:
    [`FORK.md`](FORK.md) Packaging **SHA-1 is git object ids only**.
17. **Test dependencies are supply chain (pinned 2026-08-27).** Never treat a
    cargo-audit finding, yanked crate, or unmaintained crate as irrelevant
    because it is only a dev-dependency, `[dev-dependencies]`, test helper,
    or unused in production. Supply-chain attacks target developer machines.
    Rust `build.rs` and other build scripts can hide malicious work. Defense
    in depth. The **menhera-cooldown** crates.io replacement (delay before a
    new crate version is eligible) is that policy, not a reason to fetch
    crates.io directly to skip the wait. Do not park “test-only rsa” or
    similar in residual. Dual-pin: [`FORK.md`](FORK.md) Packaging; host
    `~/.grok/AGENTS.md` § *Test dependencies are supply chain*.
19. **cargo-audit is the start of a security pass, not the end (pinned
    2026-08-27).** Start with `cargo audit`. Also check yanked crates,
    RUSTSEC pages, and CVEs for every remaining row (warnings included).
    Do not treat an unmaintained or unsound warning as acceptable. Do not
    skip the delayed crate index (menhera-cooldown) by talking to crates.io
    directly. Dual-pin: [`FORK.md`](FORK.md) Packaging; host
    `~/.grok/AGENTS.md` § *Test dependencies are supply chain*.
18. **Grok OSS skill revisions live in the product tree (pinned 2026-08-27).**
    When the operator asks to revise a skill while this session is grok-oss,
    edit `crates/codegen/xai-grok-bundle/skills/`, document it in
    [`FORK.md`](FORK.md), and keep named tests as the contract so skill
    maintenance and bundle upgrades cannot drop it. The live cache
    `~/.grok/bundled/skills/` is not the source. Do not treat host overlay
    `~/.agents/skills/` as the grok-oss source. Do not add a project
    `.agents/skills/<name>` copy unless the operator asked for a project
    override. Dual-pin: [`FORK.md`](FORK.md) Skills (multi-source).
20. **`aws-sdk-s3` / `lru` bump is deferred to fargo (pinned 2026-08-27).**
    Do not bump `aws-sdk-s3` 1.141.0 in this grok-oss wave to clear
    `lru` 0.16.4 (`RUSTSEC-2026-0253`). Resume that work in fargo. Do
    not skip the delayed crate index. Dual-pin: [`RESIDUAL.md`](RESIDUAL.md)
    Open cargo-audit; [`FORK.md`](FORK.md) Packaging.
21. **Do not vendor crates into grok-oss as the long-term fix (pinned
    2026-08-27).** Path copies under `third_party/` for audit patches are
    debt. fargo must replace them: bump the parent on the delayed crate
    index, or a Surmount git fork that later enters that index, or drop
    the parent. Do not add more `[patch.crates-io]` path vendoring. Do
    not fetch crates.io to skip the delay. Dual-pin: [`FORK.md`](FORK.md)
    Packaging; [`RESIDUAL.md`](RESIDUAL.md) Open fargo unwind.

## Subagents — parent is HITL UX only (hard)

The **main/parent thread is HITL UX only**: status to the operator, spawn L2,
wait, read **short on-disk reports**, board upsert, plus the **Hierarchical
fast path**. **Research, implementation, multi-file greps, edits, tests, and
skill-body rewrites never run on L1 or L2**, not even “just a quick look.”
L2 parallelizes and spawns L3s; L3 does the actual tools and work. Full
rule: this file § *Agent depth L1 / L2 / L3*; host `~/.grok/AGENTS.md` §
*Regressions…* + § *Hard stop — parent is coordinator only*. Git handoff
only when the operator asked for complex git help (see hard constraint
**Git silence**).

**Reports, not “joins” (pinned 2026-08-03; local home 2026-08-17):** On-disk
handoff files are **reports**. Write new reports under `~/.agents/reports/`
on this machine. Do not add report files to the git tree. Do not call them
join notes / join artifacts. Fork-join parallelism may still be named when
explaining hierarchically structured subagent work (agent depth L1 main / L2
subagents / L3 specialists max). Host dual-pin: `~/.grok/AGENTS.md`. Leftover
joins were leftover reports and live under `~/.agents/reports/`. New work
uses **reports** under `~/.agents/reports/`. Do not keep live leftover
joins under project `.agents/joins/`.

### Agent depth L1 / L2 / L3 (pinned 2026-07-29; Hierarchical fast path 2026-08-16; L2 decides L3 2026-08-20; compact split 2026-08-21)

**Supersedes 2026-08-15 "L2 MUST always spawn L3 / always three layers."** Operator contract 2026-08-20 (survives compaction): `~/.agents/reports/feat-l1-500k-nested-200k-CONTRACT.md`

- **L1 sampling** is the catalog 500k window. AUTO compact on L1 uses that window, not 200k. No 40% throttle on the L1 window size. Cancelled compact must not re-arm.
- **L2 nested** stays 200k. L2 may compact.
- **L3 never compact.** An L3 is disposable. If it stalls or spirals, kill it. When an L3 is near 200k, it summarizes, reports to L2, and stops. Do not compact-and-continue on L3. Compact on L3 is an error.
- **Finished nested agents must stop (pinned 2026-08-22).** When the host says a nested agent has exited, L1 must not leave it painted as live. If the Subagents list still shows Responding and a running timer, kill that id the same turn. A finished L2 must not keep its context open. Compaction of a finished L2 is waste. Host dual-pin: `~/.grok/AGENTS.md` § Agent depth.
- **L1** never does product work and never shows raw edits. Status, spawn L2, wait, short reports, board, Hierarchical fast path.
- **L2** is the coordinator and reports back to the operator at L1. L2 decides whether to spawn L3s. Spawn L3 **only if the problem is actually hard**. Easy work can stay on L2.
- **L3** has about as much agency as L2 except no spawn (no L4).
- **No worktrees** on this tree (`allow_worktree = false`). Do not invent a worktree workflow.
- Big already-named work runs as **parallel streams**. Explicit plan/implement is for tricky problems and new projects. After Approve, implement. Present is not Approve.

**Not** the session-board L0/L1/L2 table below (residual / todos / reports).
L1 stays cheap for HITL. L2 exists so context can be discarded after a report
goes up. Do not go deeper than L3.

| Depth | Does | Does not |
|-------|------|----------|
| **L1 main** | Status to the operator. Spawn L2. Wait. Read short reports. Board upsert. Hierarchical fast path. Modal-free operator chat: typing and chat must stay unobstructed; must not get stuck in plan soft-park or exclusive key capture. | Diagnose, implement, multi-file reads, CI logs |
| **L2 subagent** | Parallelize. Decide whether to spawn L3 (only if the problem is actually hard). Stay token-efficient. Throw context away after a report goes up. Operator-facing nested view: operator questions and clarifications in that L2 overlay go to that L2. | Show raw edits to the operator as if they were L1. Do not inject operator text into a live L3. Spawn L4. |
| **L3 specialist** | All actual tools and work, in parallel. Same agency as L2 except it cannot spawn. | Spawn L4 (**forbidden**). Do not add extra L3 hobbles (no "L3 may only grep", no weaker model unless product already requires it). |

L3 is not a weaker agent. The hard cap is no L4. L2's unique extra versus L3 is spawning L3 plus being the nested view the operator talks to. L3's unique extra versus L2 is doing the tools when spawned. Easy work can stay on L2.

Spawn an L2 when the job needs isolation from L1: implement, multi-file diagnosis, CI, regressions, skill-maintenance, or any tool work that would fill the parent. The Hierarchical fast path does not spawn L2. Additive "also" / "btw" spawns another L2 (or queues same-file). Do not kill a healthy in-flight L2.

L2 waits on L3, reads L3 short reports, and writes one L2 report under `~/.agents/reports/`. L1 reads that L2 report only and speaks to the operator. L1 does not re-do L3 greps. Those files are reports, not joins. They are not project `.agents/reports/` and not git. Operator compose in the nested L2 view resumes that L2. Do not barge into a running L3 with operator text unless the operator explicitly targeted that specialist (they did not; default is unbothered). Keep L1 list L2-only plus a live L3 count. Do not flatten L2/L3 into one list.

**Hierarchical fast path (pinned 2026-08-16; trivial named edit 2026-08-20).** The main thread may do
these things without spawning L2:

1. A one-command host question (for example `journalctl` or `last`).
2. A single known-path read that the operator or the prompt already named.
3. Read and quote the short on-disk report that this thread asked for.
4. A single already-named one-line file edit (one number, one string, one known path). Example: `just serve` bind `8000` to `8001`. Do **not** spawn L2 or L3 for that. Do **not** invent a TDD implementer. The session that owns that tree edits the file. A minute-long wait on a nested implementer for one number is process failure.

That is the **Hierarchical fast path**. It is not a license to diagnose,
implement, or walk many files in the main thread. L2 still decides whether
to spawn L3 when the problem is actually hard. L2 may compact. **Do not
compact-and-continue** on L3.

L1 and L2 may still use `spawn_subagent`, `todo_write`,
`get_command_or_subagent_output` / wait, and read the short on-disk report they
asked for. That is coordination, not work. **Do not go deeper than L3.**

This section is project **D1** law and must survive recon. Host dual-pin:
`~/.grok/AGENTS.md` § *Regressions and deep diagnosis* + § *Hard stop — parent
is coordinator only*.

Product: soft-park that **traps** L1 is rejected; plan review is on demand
(`/view-plan`, status click, panel CTAs), not forced keyboard capture of the
main thread.

**Default loop (pinned 2026-07-27):** track on board → **spawn** → **wait** →
read the short report on disk. Do **not** kill/respawn mid-flight to re-scope; do **not**
monologue interim workarounds while an implementer runs. Mid-flight operator
clarifications → board upsert only; **resume** after the report (or additive spawn
if disjoint). Host: § *Hard stop* default loop.

- **User-facing language** (mirror of host `~/.grok/AGENTS.md` § Language,
  2026-07-26): never bare **child/children** as a nickname for subagents
  (“Child finished green” is wrong). Prefer **subagent**, **implementer**,
  **explore agent**, **worker**, or a role name. Keep ban on “kids” + “cheap.”
  “Child process” = OS process only, in technical docs.
- **CI fail, regression, multi-file diagnosis, non-trivial fix, skills-location
  claims:** L1’s first tool turn is `spawn_subagent`, not parent `grep` / `gh`
  log pull / unnamed test-file reads / “I’ll check the docs.” L2 then
  spawns L3 specialists. L2 does not grep, walk the hot path, or implement.
  Hierarchical fast path stays on L1.
- L1 and L2 may: spawn, wait, board upsert, read the **short on-disk report**
  they asked for; L1 also gives brief user status. Git handoff only when asked
  for complex git/recon work.
- L1 and L2 must **not**: pull CI logs, open failing tests, re-run nextest,
  edit product code, grep “to be sure,” rewrite skill bodies, or
  research/implement. L3 does that work.
- **Additive asks / “also” / “btw”:** phrases like **also**, **btw**, **by the
  way**, **this too**, **and also**, **this work too** mean a second slice, not
  a pivot. Board-upsert; **spawn** another subagent (or queue if same-file race);
  **never kill**, cancel, or re-prompt healthy in-flight subagents on the prior
  goal unless the operator explicitly stops/supersedes. Full pin:
  `~/.grok/AGENTS.md` § *Additive asks*.
- **Mid-turn composer Enter is this turn (pinned 2026-08-22).** While a
  turn is running, Enter with explaining-work text (plain freeform, and
  slash PassThrough that is not a named hold, including `/goal ...`) is a
  **soft interject** into this turn (`x.ai/interject`). It is not a serial
  prompt queue. L1 must treat that user text as additive immediately:
  board, remaining-work pointer, spawn. Do not wait for the current turn
  to finish, and do not wait for “all subagents done,” to record the work.
  Named hold (`/queue /finish`, `/queue` compact/plan/reports) still waits.
  Ctrl+Enter is still cancel-and-send. Empty Enter does not Approve a plan.
  Product: pager `dispatch/prompt.rs`. Report:
  `/home/hunter/.agents/reports/fix-prompt-queue-blocks-explain.md`.
  Host dual-pin: `~/.grok/AGENTS.md` § *Additive asks*.
- **Multi-track (prose is not enough):** every parent tool turn inventories live
  subagent `task_id`s + board owners before spawn/demote/complete. Never demote
  `in_progress` work that still has a live subagent (abandonment = kill class).
  First track stays `in_progress` until the report; closeout still required. Product
  binding: first cut shipped (`meta.taskId` + demote reject while Running;
  see FORK). Full auto-bind / sticky-on-new-message remain soft residual.
  Host: § *Multi-track: prose is not enough*.

### Mention is in scope (pinned 2026-08-16)

If the operator mentions work, that mention is in scope. Do not park it
as optional. Do not ask "say if you want that." Do the work.

This is not a substitute for *Ambiguity → park* when intent is actually
unclear. A named job is not unclear. Dual-pin: host `~/.grok/AGENTS.md`
§ *Mention is in scope*.

### Grok OSS screenshots are this product (pinned 2026-08-21)

A Grok OSS screenshot from any current working directory is this
product. Do not assume another grok-oss window is out of scope. Host
dual-pin: `~/.grok/AGENTS.md` § *Grok OSS screenshots are this product*.

### After Approve, implement mentioned work (pinned 2026-08-19; no plan-block 2026-08-21)

When a plan is already Approved and implement is in flight (or clearly
next), operator-mentioned work and "let's do that" / continue-the-approved-work
is in scope: board upsert and spawn (or keep the healthy implementer
running). After Approve, do not block on another plan rewrite, present, or
approve cycle. Track the work, write a size estimate, implement the groups
in parallel, then reconcile the estimate against what landed. After Approve,
work starts.

This does not weaken present-is-not-Approve. `exit_plan_mode` tool success
and soft-park remain present for review, not operator Approve. Empty
freeform Enter never Approves. Always-approve is tool permissions only,
not plan CTAs. Ambiguity still parks when intent is unclear. A named job
during approved implement is not unclear. Complete plan verticals still
apply: do not invent parked/optional for approved plan steps. Host dual-pin:
`~/.grok/AGENTS.md` § *After Approve, implement mentioned work*.

## Never assume without checking

**Docs can be wrong** — treat prose as untrusted until code and load paths
confirm it. Docs in this repo (including this file, FORK, research notes) can
be wrong or stale. Do **not** claim skills location, CI root cause, conflict
intent, or recon survival from prose alone.

- First tool turn for multi-file / CI / regression / “where do skills live?” is
  **spawn_subagent** (explore or general-purpose as fits). That is L1 spawning
  L2. L2 spawns L3 only if the problem is actually hard. Easy work can stay on L2.
- Verify against **code and load paths** (and live trees) before asserting
  (L3 does that work; L1/L2 read the short report).
- Read short on-disk reports; do not re-prove the subagent in the parent.
- **Auth / credentials store / keyring:** diagnose with **red/green TDD**
  (`cargo test` contracts), not host shell D-Bus/keyring probes. Do **not** fan
  out explore + implementer on the same store bug. One implementer owns TDD.
  Pin: `~/.grok/AGENTS.md` § *Product auth / store diagnosis*.
- **Plan approval:** product CTAs only (`exit_plan_mode` soft-park / side panel
  → Approve / Comment / Revise / Exit). After Comment, Clarify is the read-only path. Letter `a` / `A` type. `?` still
  arms Clarify. Approve is the clickable Approve button. **`exit_plan_mode`
  tool success = present for review, not operator Approve.** Always-approve
  permission mode skips tool-permission prompts only; it does not auto-click
  plan CTAs. **Empty freeform Enter never approves** (clickable Approve).
  After one decisive Approve or Exit, do not re-arm Approve for the same
  present until a new `exit_plan_mode`. After Revise/Clarify, status is
  rewriting wait (not idle “Plan written / Click or /view-plan”) until
  re-present. **Never** freeform chat “reply approve/revise/abandon.” Pin:
  `~/.grok/AGENTS.md` § *Plan approval — product CTAs only*; user-guide
  `19-plan-mode`; FORK plan bullets P1–P2.
- **DOGE colour roles (do not invent from screenshots):** Human chrome is
  **green** (`accent_user`: composer caret, human rails, OSC 12, success).
  Mid-draft letter under caret: empty blink half is normal text
  (`text_primary`), not neon green ink on the letter. Agent activity is
  **magenta** (`accent_running`: active agent rails, tool spinner, lower-left
  still-running throbber, model accent). **Do not** flip the caret to magenta
  “because agent,” invent a “little guy” colour without a plain operator name,
  or conflate caret residue with the lower-left throbber or **Clear finished**
  (quiet secondary idle; not neon green, not magenta). Lasting product pin:
  [`FORK.md`](FORK.md); user-guide `06-theming`.

## Skills (multi-source)

Skills are **not** “off this branch only.”

| Layer | Who owns it |
|-------|-------------|
| Discovery, load order, project skill roots (`.agents/skills`, `.grok/skills`), bundle install/sync, user-guide | **Product on this branch** |
| Operator skill packs (`implement`, `pr-babysit`, …) under `~/.agents/skills` | **Host** overlay (wins at User tier) |
| Platform pack cache under `~/.grok/bundled/skills` | **Network** bundle (product writes the cache) |

Process that must survive recon: pin on **branch** (`AGENTS`, `FORK`,
`docs/upstream-*`) **and** host when both apply. Detail:
`doc/dev/research/where-skills-come-from-2026-07-24.md`, user-guide `08-skills.md`.

### Skill maintenance must revise carefully (pinned 2026-08-17; standing 2026-08-18)

Future `/skill-maintenance` runs must read current `AGENTS.md`, `FORK.md`, and
product seams, then revise skill bodies. Do not treat copy-sync as enough.
This is standing law for those future runs. The 2026-08-18 careful-revise wave
already happened. Do not treat this pin as an owed next run.

## Survive recon (process pins on the branch)

Chat is **not** enough. Import restores only `FORK_PATHS`; put-history
cherry-picks product; join (`-s ours`) keeps the onto tip tree. Pins on branch:
this file (including § *Agent depth L1 / L2 / L3*: three layers for
implement, multi-file diagnosis, CI, and regressions, plus the
**Hierarchical fast path**; not the old weaker “spawn L3 when many greps /
half the window” rule),
[`FORK.md`](FORK.md), [`RESIDUAL.md`](RESIDUAL.md),
[`docs/upstream-history.md`](docs/upstream-history.md) + sibling logs, upstream
helper (`put-history`, import, join, hermetic PATH, assert pins, …).

```bash
grok-nix-helper assert-process-pins          # worktree
grok-nix-helper assert-process-pins HEAD     # or a tip tree-ish
just upstream-assert-process-pins
```

Import runs the assert after `FORK_PATHS` restore. See FORK § *What recon keeps*.

**Product seams inside `xai-grok-*` are not path-restored.** They survive
onto only via cherry-picks plus **named cargo tests**. Assert proves files
exist; it does not prove those contracts. Land must cover the seven inventory
classes in [`FORK.md`](FORK.md) § *Land checklist* (chrome/paint, `/settings`
plus unread config, grok-oss ledger `/spend`, CLI branding, dual-auth hop
after included SuperGrok period limits are full, last-session on start, product
skills are not a Python runtime). A restack that reintroduces non-excepted
Python under product skills, or drops the Rust intercept for the allowlisted
CLI forms, is a failed land. A chrome-only pass is a failed land. `just check`
cannot fail a deleted catalog test. Catalog:
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).

## Ship / CI / git

- Ship product work: short hierarchical note in [`FORK.md`](FORK.md); link out.
- **CI is checks only** — never a release package build in GHA. Local gate:
  **`just check`** / **`just ci`**. No `ci-quick` / `ci-host`.
  The operator owns **`just check-remote`** / **`just test-remote`** on
  surmount-1 (pinned 2026-08-25). Agents do not invoke those recipes.
  Optional **`just check-remote`** realizes flake metadata and the workspace
  cargo quality derivation (the same full gate as `just check` / `just test`:
  fmt, then workspace clippy `--all-targets` (members include
  grok-nix-helper and cargo-mem-guard), then workspace
  nextest execution, then doctests) on the
  existing trusted-user remote builder. rustc requires that builder's `surmount-remote`
  feature (and `big-parallel`) and must not run on the caller. This laptop
  never auto-detects `surmount-remote`; the host machines file must advertise
  it. `--option system-features` that omit `big-parallel` does not stop local
  nixbld: the daemon still advertises `big-parallel`. Force-remote nix also
  passes `--cores 64` so workspace rustc can use the builder's cores. Cargo
  on that derivation passes `--jobs` from those cores, capped at 32
  (`cargo check --jobs`, never global `cargo --jobs`; cargo 1.97 has no
  global `--jobs`). Quality does not run `cargo clippy` (that external
  dispatcher plus a 1-token jobserver ignores `--jobs`). Workspace lint is
  `cargo check` with `RUSTC_WORKSPACE_WRAPPER=clippy-driver` under GNU make
  `-j$CARGO_BUILD_JOBS` after dropping Nix MAKEFLAGS/CARGO_MAKEFLAGS, and uses
  the same **dev** profile as local `just test-clippy`
  (not crane `--release` check/clippy). Workspace `--all-targets` clippy
  and nextest include grok-nix-helper and cargo-mem-guard. Do not
  compile helper tests only in a late `cargo test --manifest-path` after
  workspace clippy.
  **Nix jobs are not cargo/rustc workers (pinned 2026-08-18).** Nix jobs
  are how many derivations a builder may take at once (machines-file
  `max-jobs`, nix-daemon `max-jobs`). Workers are cargo/rustc parallelism
  *inside one derivation* (`CARGO_BUILD_JOBS`, cargo `--jobs`,
  `NIX_BUILD_CORES` / nix `--cores`). Do not treat machines-file 64 jobs as
  rustc workers. Do not raise Nix max-jobs to fix a single busy rustc.
  Laptop local max-jobs stay this laptop's cores for `just check` / `just ci`.
  Host machines max-jobs is how many jobs we send to the remote. `--cores 64`
  sets `NIX_BUILD_CORES` inside a derivation. Cargo jobs 32 is an OOM hedge
  for clippy/check, not 64 Nix jobs. nextest compile/link uses
  `--build-jobs` capped at 4 (`CARGO_LINK_JOBS`): 32 parallel mold links
  were SIGKILL'd (`ld returned 137`) under the builder nix-daemon 32GiB
  MemoryMax. Host MemAvailable is larger; cargo-mem-guard reads
  `/proc/meminfo` and would not restart. Force-remote recipes pass `--option max-jobs 0` on the
  caller so this laptop does not curl crates.io or static.rust-lang.org
  for that gate. They also pass `--store` ssh-ng (the machines-file
  builder) and `--eval-store auto` so cargo-package NARs stay on the VPS
  instead of copying back into the local store. Named filters use **`just test-remote`**
  (or **`just cargo-remote`**) which realizes `.#workspace-cargo-named-test`
  the same force-remote way; rustc still requires `surmount-remote` and must
  not run on this laptop. Default `just check` / `just ci` and
  GitHub Actions stay local and must not require that builder. GitHub Actions
  must not call `check-remote`, `test-remote`, or `cargo-remote`.
- Feature branches → PR → **`main`**. Tool branches (`import/*`, `onto-xai/*`)
  land through PRs, not a second main.

## Upstream (xAI)

Prefer **product commits on their current tip** (`grok-nix-helper put-history-on-xai`,
real cherry-pick), then **join Surmount `main`** (`grok-nix-helper join-main-into-onto`,
`merge -s ours`). **Import** is a different job (absorb tree into Surmount
history). Detail: [`docs/upstream-history.md`](docs/upstream-history.md).

### Onto recovery (no live SHAs in this file)

**Live tip SHAs / mid-work = D2 only.** Recovery pointers:

1. [`docs/upstream-history.md`](docs/upstream-history.md) § *HITL runbook* + § *Live stack*
2. [`docs/upstream-onto-log.md`](docs/upstream-onto-log.md)

No `MODE=overlay` / commit-tree. No `cherry-pick --abort` or `FORCE=1` rebuild
while a healthy stack is mid-pick. Multi-file conflicts → L2 coordinators on
disjoint paths spawn L3 specialists (L2 does not resolve files itself); L2
writes short reports on disk. Every continue/join merge = human `git commit -S`.

## Residual

- [`RESIDUAL.md`](RESIDUAL.md) holds **open** human-intent or unfinished honesty
  items only (D0). When finished, move lasting truth into FORK or process docs;
  campaign closed writeups → `doc/dev/campaigns/`.
- **Operator-asked in-tree work is not residual (pinned 2026-08-27).** If the
  operator asked to do it and the fix lives in this tree (bump, replace,
  remove, include a crate), do it. Do not hide it in `RESIDUAL.md`. Residual
  is for blockers outside the tree (CDN publish, VPS daemon MemoryMax, a
  human `git commit -S`). "No patched crate on crates.io" is still agent
  work: remove, replace, or isolate the dependency.
- **Document leftover residual same turn (pinned 2026-08-25).** When a plan
  or slice ships a first wave, same turn add remaining later-wave work to
  `RESIDUAL.md` Open in complete thoughts. Chat is not enough. Do not
  invent parked/optional for named leftover. Dual residual honesty: do not
  list finished work as open; do not omit sibling paths still unfixed
  (example: `install.ps1` after a POSIX-only pin). Host dual-pin:
  `~/.grok/AGENTS.md` § Residual honesty.

## Operator orchestration (session board L0–L2)

Session-board layers only (todos / reports). **Do not confuse** with agent depth
**main thread (L1) / subagents (L2) / specialists (L3 max)** above.

| Session layer | Where |
|---------------|--------|
| **L0** durable residual | `RESIDUAL.md` (D0 open) / campaign docs |
| **L1** session todos | Namespaced `plan:*` `impl:*` `pr-N:*` `recon:*` `residual:*` `ask:*` `feat:*` `bug:*` — **never casual wipe**; merge upsert only; product keep-unless-mentioned on `merge: false`. **Fib leaves:** size **1 or 2** only; larger work → split children; **progress = Σ leaf sizes** (phases/containers unsized). Prefer `meta.kind` + `parentId`. See [`doc/dev/research/todo-progress-fib-2026-07-26.md`](doc/dev/research/todo-progress-fib-2026-07-26.md). |
| **L2** reports | Short on-disk reports under `~/.agents/reports/` on this machine. Leftover joins were leftover reports and live there too. Agent reports are not part of the git tree. |

### Session board: track well and close out (pinned 2026-08-01)

Full law: host `~/.grok/AGENTS.md` § *Session board: track well and close out*.
Essence for recon survival:

- **Track well:** short actionable outcome owed, not a verbatim user-message
  dump; namespaced ids; quote only when a precise contract needs it.
- **Close out:** complete the item same turn the ask/fix/feature is finished
  (report or parent status); cancel only with a real reason recorded.
- **No wipe theater:** never `merge: false` mass-clear or mass-cancel to tidy;
  after a wave, audit open items (still real / complete / cancel+why).
- **Substance first:** complete only when handled; partial work stays open with
  updated remaining content. Parent completes from reports + operator
  messages without re-research.

Prefer no worktrees (`allow_worktree = false` default). Campaign + reports:
`doc/dev/campaigns/operator-orchestration-2026-07.md`,
`doc/dev/research/todo-levels-product-2026-07-24.md`,
`doc/dev/research/execute-plan-no-worktree-2026-07-24.md`,
`doc/dev/research/todo-progress-fib-2026-07-26.md`.

## Naming

- `xai-*` crates/paths stay for upstream mergeability.
- Novel Surmount crates/names: **`grok-*`** / **`grok-oss`** (no `xai-` prefix).
- Product CLI is **`grok-oss`** (`PRODUCT_CLI_NAME`). Start `grok-oss`, not bare `grok`.
