---
name: pull-remote-tree
description: >
  Pull a remote project tree onto a local directory with the
  pull_remote_tree tool. Use when the user wants a HOST:SRC tree on
  this machine, says pull the remote tree, or asks to copy a remote
  checkout here. Not rsync as a tool id. Never git commit.
metadata:
  short-description: "Pull HOST:SRC onto a local dest with pull_remote_tree"
  argument-hint: "[HOST:SRC] [local dest]"
---

# Pull remote tree

Copy a remote tree onto a local destination. Call the named tool
`pull_remote_tree`. Do not invent a shell rsync as the copy. Do not
treat rsync as the tool id.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/pull-remote-tree/` on startup. The live cache
is not the source. Do not add a project `.agents/skills/pull-remote-tree`
copy unless the user asked for a project override.

## When to call the tool

Call `pull_remote_tree` when the user wants a remote tree on this
machine. `from` is `HOST:SRC` (OpenSSH) or a local directory that is
not dest. `dest` is a local directory only.

Do not call it to send a local tree to a remote host. Dest that looks
like `HOST:PATH` is refused.

## How to call it

1. Resolve `from` and local `dest` from the user. If dest is missing,
   ask once in freeform. Do not invent a remote dest.
2. Call `pull_remote_tree` with those two fields.
3. The tool excludes directories named `.git`, `target`, `.lake`, and
   `result`.
4. Copy is a Rust walk plus `std::fs`. OpenSSH may fetch a remote
   tree. That is transport, not a shell rsync copy.

## Hard rules

- Never `git add`. Never `git commit`. Never `git push`.
- Never pass argv or paths that would git commit or git push.
- Never treat this skill as permission to commit.
- Product CLI is `grok-oss`.
- When the operator asks to revise a skill in grok-oss, edit
  `crates/codegen/xai-grok-bundle/skills/`, not only a host overlay
  and not repo `.agents/skills/`.
