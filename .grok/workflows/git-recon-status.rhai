// Status-only recon probe. Does NOT resolve conflicts, commit, or bypass GPG.
// Skill SOP: ~/.agents/skills/git-recon/SKILL.md
// Survival: .grok/workflows is in import FORK_PATHS + assert-process-pins
// REQUIRED_DIRS. Must be present on BASE_REF (committed on main) for restore.
// Dual-pin: host git-recon skill + branch docs/upstream-history.md.
// Hierarchical subagents (specification 0016): the contract is FORK.md, heading Hierarchical subagents (specification 0016). This workflow does not copy that contract.
// L0 is the laptop coordinator, crate surmount-coordinator-gui. L0 queues prompts and is not a nested agent. L1 is the main session.
// This workflow is the L2 that L1 spawned. Each agent() call is that L2 spawning an L3.
// The word coordinator means L2 only. The word specialist means L3 only. An L3 does not spawn. There is no L4.
// Default accomplishment is L1 spawning an L2. Do not keep implement work on L1.
// An L2 can restart and compact its L3s when those L3s are doing well.
// The hierarchical fast path on L1 is four acts only: one already-named command, one already-named file read, the asked-for short report, or one already-named one-line edit. "Just a quick look" is not that path. That fast path is not this workflow.
// Speaker labels are Operator and Agent. Do not use kinship words for a layer. Do not compare layers to operating system process trees. Call the on-disk handoff a report. Reports live under ~/.agents/reports/ on this machine and are not part of the git tree.

let meta = #{
    name: "git-recon-status",
    description: "L1 spawns L2. L2 spawns L3. Probe onto/cherry-pick/merge state and report the next recon action",
    when_to_use: "onto mid-stack, cherry-pick conflict, join staged, recon where am I",
    phases: [
        #{ title: "Status", detail: "L2 spawns an L3 for porcelain + next action" },
    ],
};

let status_schema = #{
    "type": "object",
    "required": ["branch", "state", "next_action", "unmerged", "summary"],
    "properties": #{
        "branch": #{ "type": "string" },
        "state": #{
            "type": "string",
            "description": "clean | cherry_pick_conflict | cherry_pick_clean | merge_staged | unknown",
        },
        "next_action": #{ "type": "string" },
        "unmerged": #{ "type": "array", "items": #{ "type": "string" }, "maxItems": 80 },
        "summary": #{ "type": "string" },
        "human_commands": #{ "type": "string" },
    },
};

phase("Status");

let prompt = "";
prompt += "You are an L3 specialist doing a read/execute status probe for Surmount git recon. ";
prompt += "L1 spawned this coordinator L2. This L2 spawned you. L2 coordinates. You implement or review. An L3 does not spawn. There is no L4. Speaker labels are Operator and Agent. ";
prompt += "Prefer the product script (read-only; do not invent SHAs from memory):\n";
prompt += "  ./scripts/recon-status.sh\n";
prompt += "  # or: just recon-status\n";
prompt += "If the script is missing (bare tip mid-stack), fall back to:\n";
prompt += "  git rev-parse --abbrev-ref HEAD\n";
prompt += "  git status -sb\n";
prompt += "  test -f \"$(git rev-parse --git-path CHERRY_PICK_HEAD)\"; ";
prompt += "test -d \"$(git rev-parse --git-path sequencer)\"; ";
prompt += "test -f \"$(git rev-parse --git-path MERGE_HEAD)\"\n";
prompt += "  git diff --name-only --diff-filter=U\n";
prompt += "  git merge-base --is-ancestor origin/main HEAD 2>/dev/null; echo $?\n";
prompt += "Optionally skim docs/upstream-history.md section Live stack if present.\n";
prompt += "Script output wins over stale Live stack docs. ";
prompt += "Classify state and set next_action in plain English. ";
prompt += "If unmerged paths exist, next_action is conflict fan-out then Operator cherry-pick --continue. ";
prompt += "If MERGE_HEAD and join-looking merge, next_action is Operator git commit -S join. ";
prompt += "Never run git commit, cherry-pick --continue, merge --abort, FORCE rebuild, or GPG bypass. ";
prompt += "Never invent MODE=overlay. Fill human_commands with exact pasteable lines when an Operator gate is next.";

let r = agent(prompt, #{
    label: "recon-status",
    capability_mode: "execute",
    output_schema: status_schema,
});

if r == () || !r.success || r.output == () {
    complete(#{
        summary: "Status probe failed",
        ok: false,
    });
}

let report = "";
report += "# git-recon status\n\n";
report += "- branch: " + r.output.branch + "\n";
report += "- state: " + r.output.state + "\n";
report += "- next: " + r.output.next_action + "\n";
report += "- summary: " + r.output.summary + "\n";
if r.output.human_commands != () && r.output.human_commands != "" {
    report += "\n## Hand to Operator\n\n```bash\n";
    report += r.output.human_commands;
    report += "\n```\n";
}
report += "\nUnmerged count handled in output.unmerged. SOP: /git-recon\n";

let path = write_scratch_file("git-recon-status.md", report);
complete(#{
    path: path,
    branch: r.output.branch,
    state: r.output.state,
    next_action: r.output.next_action,
    summary: r.output.summary,
});
