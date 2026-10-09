---
name: what
description: >
  Restate this session in four short complete American English thoughts
  when the operator cannot parse agent chat. Follow Concise American
  Technical English as specified in Surmount 0005_CATE.md. Use when the
  user runs /what, says they do not understand, asks you to speak more
  clearly, or asks what is going on in this session. Not /recap. Not
  /finish. Not /reports. Not an apology.
metadata:
  short-description: "Four complete English thoughts: work, truth, operator, next"
  argument-hint: "[optional focus]"
---

# What

The operator cannot parse the last agent chat. Restate. Do not apologize.
Do not write a file. Do not spawn.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/what/` on startup. The live cache is not
the source. Do not add a project `.agents/skills/what` copy unless
the user asked for a project override.

Follow Concise American Technical English (CATE), numbered specification
`0005_CATE.md` in SurmountSystems/specs
(https://github.com/SurmountSystems/specs/blob/main/0005_CATE.md ,
accessed: 2026-08-27). CATE is not specification 0006.

Reply with this shape only. Four labeled complete thoughts. Nothing
fluffier. One idea per sentence when that stays clear. Labels are
**Job / State / Operator / Next**. Prefer Operator and Agent as speaker labels.
Address the person as Operator, not Human. Do not say You or Human for the operator.
Do not say Me or Grok as the speaker label for the machine
(Grok OSS and grok-oss stay product names).
Painted chrome and user-guide call the composer the Operator box.

1. **Job:** one sentence. The real product outcome this session is
   trying to finish right now.
2. **State:** running, waiting, blocked, or done.
   Name the real file, command, crate, or test. Do not use private
   labels. Translate leftover jargon from the last agent message into
   ordinary words.
3. **Operator:** the operator action, or the word "nothing"
   if they do not need to act. Then say why. Name evidence: who owns
   the next action, which command they asked to keep, which gate is
   unmet. Do not leave a bare "nothing."
4. **Next:** the next concrete agent step.

## Rules

- Complete American English thoughts. Short sentences. No half labels
  used as sentences.
- Name the real thing: the file, the command, the crate, the test, the
  outcome. Decoder ring jargon is forbidden (private nicknames that
  force the reader to decode). If the last message used those nicknames,
  translate them.
- A guess is a guess: omit it or label it.
- On **Operator**, evidence is required. "Nothing" is valid
  only when you can say why (the next step is the agent's, a gate they
  set is unmet, they did not ask for git or install).
- Do not put leftover board ids, hex run ids, or compacted codes in
  the body.
- Do not ask them to say a magic word to continue when the next step
  is already clear. Do that step, or name it under **Next**.
- Optional focus from `/what ...` is the part they did not understand.
  Answer it under the four labels. Do not add extra sections.
- This is not `/recap`, not `/finish`, not `/reports`.
- Never mix Grok Build version with grok-oss product version. grok-oss
  is `grok-oss --version` (`1.0.3` plus git SHA). Grok Build is
  `grok --version`. Isolated Preview and plan chrome are grok-oss unless
  this process was launched as `grok` from downloads. Remaining-work,
  reports, and this restatement: probe this turn if stating which
  binary this window is. Do not reuse a leftover Grok Build version as
  grok-oss.
- For a finished job's actual nested tokens, call the Rust function
  `local_usage_events_for_session` in
  `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs`.
  Do not use Python. Do not use bash. Do not use a `sqlite3` one-liner.
  The reconcile cell is that row's `output_tokens`, not `total_tokens`,
  and not the standing estimate.
  Chain of thought is `reasoning_tokens` and is not added again.
  When `output_tokens` is present, print that
  integer. Do not write `not_fetched` when `output_tokens` is present.
  A null `output_tokens` is not printed as `0` and is not copied from
  the estimate. Omit the figure, or say the output total was not stored.
  Leave the Billing Credits card wire `not_fetched` alone. That is a
  different meter.
- When the operator asks to revise a skill in grok-oss, edit
  `crates/codegen/xai-grok-bundle/skills/`, not only a host overlay
  and not repo `.agents/skills/`.
