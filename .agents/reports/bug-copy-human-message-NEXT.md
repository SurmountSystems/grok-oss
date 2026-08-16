NEXT: WAIT

ACTION: WAIT_IMPLEMENTER (action.md + action-head.md).
CONCLUSION: do not respawn and do not write a final report. Cargo is live; impl.md is still missing.
impl.md: MISSING. explore.md: EXISTS (14547 bytes, Aug 15 13:10). Same-size explore-READY copy.
head.md: MISSING. Closest file is bug-copy-human-message-action-head.md (13:46), same WAIT line.
status.md: exists. Named table explore YES, impl NO. Older conclusion was died_no_report / idle compile.
brief.md: IMPL_MISSING, EXPLORE_EXISTS. Embedded decision is stale RESPAWN_IMPLEMENTER from the idle scan.
impl-TIMEOUT.md: exists (13:31). Waited 20 minutes; required impl path never appeared.
cargo running: YES. clippy -p xai-grok-pager --all-targets PID 2425933 (~2m42s).
Also live: cargo check -p xai-grok-pager --all-targets PID 2425954, plus rustc/clippy-driver on grok-build-target.
Status/decision "no cargo, cache idle 13:15" is stale versus this clippy/check wave.
Do not RESPAWN: that would race the live implementer compile.
Do not WRITE_FINAL: impl.md is absent, so there is no red+green handoff to synthesize.
Explore finding (still valid): always-on bubble copy glyph is paint-only; no hit rect, no hover, no click-to-copy.
When bubble copy is on (default), selection-box copy is hidden. Keyboard y on a selected human line should still copy.
Suggested fix: publish hit rects and fire Action::CopyBlockContent. UserPromptBlock::copy_text is already correct.
Existing tests paint the glyph only. No click-to-copy contract yet.
After this compile exits, re-read bug-copy-human-message-impl.md. Only then choose WRITE_FINAL or RESPAWN.
If impl.md appears with observed red then green, next is WRITE_FINAL. If cargo dies and impl.md is still gone, then RESPAWN.
No product edits in this coordinator pass. No crate changes.
