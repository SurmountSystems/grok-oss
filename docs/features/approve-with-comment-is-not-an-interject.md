# Approve with a comment is not an interject

Approve with a typed comment keeps that comment on the approval, approves the plan, and lets the work start. It is not an interject, and it is not a queued prompt. An idle turn does not make this fail. The plan pane does not make this fail.

Mid-turn Enter that is not on a presented plan is still an interject. Revise and Clarify may still use an interject. Empty Enter never Approves. Typing the comment without clicking Approve does not approve.

Before this fix, the same Approve click returned an interject. That interject often failed, so the comment did not stay on the approval and the work did not start. An earlier success that kept `Love it! Execute now.` and started the work is not what the product was doing.
