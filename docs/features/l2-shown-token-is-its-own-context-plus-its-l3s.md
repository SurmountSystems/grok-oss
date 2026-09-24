# L2 shown token is that L2 plus the L3s it spawned

Source: the open `secondary-plan.md` pane in the screenshot from Thu Sep 24, 2026, 10:53 AM. One paragraph. Three failures.

1. Token rows say `not_fetched`. That means fetching the count is broken again.
2. The atomic count on the row is not updating as the count changes.
3. The number shown for an L2 is that L2's own context plus every L3 it spawned, added once. An L3 is not counted again outside that L2. The sum is not added into the L1 footer. If the host has no count, omit the figure. Do not invent one.

Quote from that pane:

"Tokens keep saying not_fetched, which seems to indicate that there's some kind of bug with fetching token counts again... Also, atomic token counts are not live updating properly, but at least they aren't cluttered. I'm pretty sure I recall telling you to add all token counts from the L2 context along with all L3s it spawned, added all up before displaying, but being careful not to double count."

Home: `docs/features/` in the grok-build project. Not a grok session directory.

Status: the sqlite insert is in the source. This display contract is not built yet. Later notes go in this file.
