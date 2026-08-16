CLOSE: COMPLETE

The always-on bubble copy glyph is `⧉` (`copy_icon()`, U+29C9; ConHost `c`).
It sits on the first line of a human (and assistant) bubble when **Bubble
copy buttons** (`bubble_copy_buttons`, default on) is enabled. Policy A:
one `⧉` per bubble; the selection-box icon is hidden. A typical human line
has no fullscreen `↗`, so this glyph next to the green rail is the only
copy affordance. Not keyboard `y`, not `/copy`, not drag selection.

Source was broken, not only an old live binary. Paint-only: no hit recorded,
`hit_sb_copy` hidden when bubble copy is on, click returned `Changed`.
Crate is still `1.0.3`; a live pre-fix process needs rebuild and full
quit/reopen.

Red (test added first, then product): `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`.
`cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt`
failed 101: `must copy via CopyBlockContent, got Changed`. Icon was painted.

Green, same filter after `hit_bubble_copy` plus `Action::CopyBlockContent`:
exit 0, 1 passed. Related `bubble_copy_` paint tests: 3 passed, not rewritten.

Nine `xai-grok-pager` product files: `scrollback/{types,blocks/mod,selection,render,scrollback_pane}.rs`,
`app/agent_view/{mod,session,render}.rs`, `app/mouse.rs`. No user-guide.
No settings-registry change. Mop later lint-only: render expect, selection
identity math, bench loop, doctor/diagnostics canonicalize, settings_e2e min/max.

Mop ran yes (`bug-copy-human-message-mop.md`, status `mop_done`).
Implementer: fmt 0, `--lib` clippy 0, `--all-targets` 101, red 101, green 0.
Mop: fmt 0; lib clippy 0; all-targets 101 then 101 then 0; contract+paint
tests 0 (3, later 6); settings_e2e 0. No second cargo on close.

Leftovers: unit test does not talk to a host clipboard; wide first lines
still drop the icon; no dedicated assistant click test; catalog still
lists only the paint test; live `1.0.3` until rebuild; stale action/brief
still say impl is missing (ignore). Final report is not weaker than impl.
No product edit and no git on this close.
