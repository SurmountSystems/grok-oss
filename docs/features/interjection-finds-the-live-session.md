# Interjection finds the live session

Source: screenshot Fri Sep 25, 2026, 3:13 AM. Project on screen was `~/Projects/surmount`. The green line was:

`Interjection failed – requeued: couldn't send interjection: Invalid params: "session not found: 01a0d5e4-85b2-7103-99a7-fb22bb0cddfb"`

The message was put back in the queue. The Operator says this is recent.

This is not approve-with-comment. Approve with comment is a presented plan, a typed comment, and the Approve button. Mid-turn Enter to a live session is an interjection. Those stay separate.

Contract: an interjection to a live session is delivered. It is not rejected with `session not found` for a session that is still on the list. A real missing session may still fail, and the text must say the session is gone. It must not report success.

Status: a live-session interjection is delivered, and a missing session fails without claiming success. Test: `interject_to_a_live_nested_session_is_delivered_and_a_missing_session_says_gone`.
