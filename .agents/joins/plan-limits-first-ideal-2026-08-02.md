# Join: plan limits-first ideal (2026-08-02)

**Mode:** plan only. No product edits.

## One-line diagnosis

**Auth path is already SuperGrok-session limits-first (console not live); the ideal still fails because included weekly debit under load is unproven, flat-poll honesty is not auto-wired, and ExhaustedAll still prefers console over SuperGrok $ extras.**

## Ideal (short)

While SuperGrok included weekly has room → session proxy + that traffic debits included; after included full → SuperGrok $ extras as after-burner, then console; UI never claims burn from flat % alone. License Usage zeros are not this problem.

## Gap snapshot

| Class | Status |
|-------|--------|
| SuperGrok path + Design A strip console | **Working** (live + code) |
| Included debit (C4) | **Unproven** (flat 65% / $100.29 under heavy dogfood) |
| Flat-poll honesty live wire | **Gap** (flag only set in tests) |
| Extras before console after included full | **Gap** (code → console on ExhaustedAll) |
| Grok Business licenses zeros | **Not a fail mode** |

## Ordered slices

0. Dogfood baseline after rebuild
1. **Prove debit** (poll history + wire `flat_poll_unproven_debit` + productUsage delta)
2. Branch on evidence (server debit / server flat / extras-early)
3. After-burner policy: keep SuperGrok session for $ extras before console
4. Product console-edge audit (BYOK / env / non-chat)
5. UI polish (Build % surface, doctor copy)

**Prefer Slice 1 before any hop rewrite.**

## Full plan

[`.agents/plans/limits-first-ideal-2026-08-02.md`](../plans/limits-first-ideal-2026-08-02.md)
