# Pin: tools improve tools (2026-08-15)

Operator restated: it is wasteful for tools to write disposable bash
or Python. Always better to improve the tools we use. A lever: tools
build better tools. Not one-off curl against vague API surfaces.

## Where it landed

- Project `AGENTS.md` hard constraint 6, named paragraph **Tools
  improve tools**.
- Host `~/.grok/AGENTS.md` same heading under Prefer Rust tools.
- Host skill-rules rule 17 one-line add.
- `RESIDUAL.md` Open process bullet.

## What it means

Named product tool surfaces (`search_replace`, `apply_patch`, `write`,
native fetch, existing bins) are the work. When a surface is missing
or wrong, extend that tool. Do not invent a throwaway script or curl
wrapper.

Supply-chain ban on invent-and-run Python/shell stays. This pin is
the product-design reason, not only the security reason.

## Not this turn

Token economy diagnosis is parked. Tools-don't-screw-up first.
