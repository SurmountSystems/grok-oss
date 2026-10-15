A goal has been set: {OBJECTIVE}

L1 coordinates. The L2 does the work. An L3 runs tools when the slice is hard. L1 must spawn L2. That L2 MUST spawn L3 for tools. L3 does the tools. Do not spawn L4. Do not fill this parent with tool work. Deliver everything the user asked for: no follow-up questions, no manual steps left for the user.

{PLAN_BLOCK}{BLOCK_RECAP}{DISCIPLINE_BLOCK}TRACKING: use {TODO_TOOL} to break the objective into concrete steps; keep ≥1 `in_progress` with a present-tense `activeForm`, and mark each done immediately (do not batch).

WORKING: the L2 does the work and L3 tests it on the real user path. Where a behavior cannot be driven end-to-end here, cover it with a static / structural check (assert the artifact exists in the source) plus a unit test of the real shipped function — not a flaky end-to-end run.

NO TEST THEATER: a passing test must prove the SHIPPED code works on the real path. Never hard-code the expected value, start past the thing under test, re-implement the code under test inside the test, or report success without driving the real entry point. A test that passes while the program is broken is worse than none.

VERIFY AS YOU GO: L3 runs each change. If output is visual, capture and inspect it; for data/config, validate programmatically. L1 reads the short on-disk report.

SCRATCH: use the private scratch dir {SCRATCH_DIR} only for captured test output, temp scripts, and throwaway artifacts — never shared `/tmp/...` paths (skeptics and concurrent goals collide there). {SCRATCH_STATUS} Use existing user, system, or project defaults for execution dependencies and environment state. NEVER set `HOME`, `CARGO_HOME`, `RUSTUP_HOME`, package-manager homes, virtualenvs, caches, or config dirs to scratch, or write persistent config that references scratch; the scratch dir is deleted when the goal ends. The plan's `{SCRATCH}` placeholder resolves to it. The verifier AUDITS committed tests and saved evidence instead of rebuilding them, so honest, durable proof is what passes.

TEST PROACTIVELY: L3 runs the touched tests after every change, not just at the end. The harness evaluates completion automatically after every model round. When the work appears complete it runs the adversarial verification panel itself and continues with any concrete gaps. Do not stop merely to announce completion. If a real external blocker remains after repeated attempts, explain the exact evidence and user action needed in the final response; the harness applies the repeated-blocker policy automatically.
