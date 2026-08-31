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
fluffier. One idea per sentence when that stays clear.

1. **What we are doing:** one sentence. The real product outcome this
   session is trying to finish right now.
2. **What is true right now:** running, waiting, blocked, or done.
   Name the real file, command, crate, or test. Do not use private
   labels. Translate leftover jargon from the last agent message into
   ordinary words.
3. **What you need to do:** the operator action, or the word "nothing"
   if they do not need to act. Then say why. Name evidence: who owns
   the next action, which command they asked to keep, which gate is
   unmet. Do not leave a bare "nothing."
4. **What I will do next:** the next concrete agent step.

## Rules

- Complete American English thoughts. Short sentences. No half labels
  used as sentences.
- Name the real thing: the file, the command, the crate, the test, the
  outcome. Decoder ring jargon is forbidden (private nicknames that
  force the reader to decode). If the last message used those nicknames,
  translate them.
- A guess is a guess: omit it or label it.
- On **What you need to do**, evidence is required. "Nothing" is valid
  only when you can say why (the next step is yours, a gate they set
  is unmet, they did not ask for git or install).
- Do not put leftover board ids, hex run ids, or compacted codes in
  the body.
- Do not ask them to say a magic word to continue when the next step
  is already clear. Do that step, or name it under **What I will do next**.
- Optional focus from `/what ...` is the part they did not understand.
  Answer it under the four labels. Do not add extra sections.
- This is not `/recap`, not `/finish`, not `/reports`.
- When the operator asks to revise a skill in grok-oss, edit
  `crates/codegen/xai-grok-bundle/skills/`, not only a host overlay
  and not repo `.agents/skills/`.
