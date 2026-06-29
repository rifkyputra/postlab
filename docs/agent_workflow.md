# postlab multi-agent workflow

A multi-agent process for postlab. The goal is **context preservation**:
in a multi-agent effort the conversation window is volatile and gets compacted, so
load-bearing state must live in files (a "system of record"), and heavy investigation is
pushed into subagents whose tool-churn never pollutes the orchestrator's window.

> **Principle.** The orchestrator (the parent agent driving the session) holds the
> stable thread and never touches raw detail. Subagents are disposable context buffers: they
> do the churn, write a durable artifact, then return only a summary. **The handoff artifact
> IS the context** — if losing the conversation would lose the information, it's in the wrong
> place.

postlab maps onto this pattern well:

- **Adapter symmetry** (apt/dnf/brew/pacman, ufw/firewalld/pf) → natural parallel fan-out
  via `Agent(explorer)` ×N with `run_in_background=true`.
- **"TUI can't be verified headlessly"** → a ready-made HITL gate (parent agent pauses and
  tells the user to run `sudo postlab`).
- **Root + system-mutating** surfaces (firewall, users, ssh, services, migrations) → the
  `security-reviewer` agent in the `review` workflow.
- **Zero-warning clippy + `make check && make test`** → the `verifier` + `test-engineer`
  agents (built into `implementation` and `review` workflows).
- **`feature_list.json` is source of truth** → a mandatory `writer` sync step.

---

## Agent runtime

This document assumes an agent runtime that provides:

- **Agent types** — specialized subagents: `planner`, `explorer`, `executor`, `reviewer`,
  `verifier`, `security-reviewer`, `test-engineer`, `writer`, `analyst`, `cold-verifier`.
  Each is a disposable context buffer: it runs independently, writes results to files, and
  returns a summary.

- **Workflows** — pre-composed agent pipelines. `fast-fix` (explorer → executor →
  verifier), `implementation` (planner-led fanout with review/security/verification),
  `review` (explorer → reviewer → security-reviewer → verifier), `research` (explorer →
  analyst → writer), `parallel-research` (fan-out explorers → synthesize → write).

- **Teams** — named groupings of agents with a default workflow. `fast-fix`,
  `implementation`, `review`, `research`, `parallel-research`.

- **Skills** — injectable context packs that pre-load an agent with domain knowledge
  (`postlab`, `git-master`, `security-review`, `context-artifact-hygiene`, etc.).

- **Orchestration primitives** — the parent agent dispatches work via `team` (run a
  workflow end-to-end) or `Agent` (run a single agent). Parallel dispatch uses
  `run_in_background` + `batch_id`; results are collected with `get_subagent_result`.

- **No native HITL** — the runtime has no built-in human-in-the-loop. The parent agent
  must pause and ask the user directly for any gate that requires human confirmation.

These constructs are surfaced as function/tool calls available to the orchestrator. The
worked example in §7 shows the concrete syntax.

---

## 1. Role mapping (postlab roles → agent types)

| postlab role | Responsibility | Agent / workflow | Artifact |
|---|---|---|---|
| BSA / TDM | Scope against `feature_list.json`; write acceptance criteria; right-size the workflow | Parent agent (orchestrator) + `team action='recommend'` | `docs/plan/<f>.md` |
| System Architect | Design; adapter/screen pattern fit; root/DB invariants | `planner` agent (via `team action='plan'` or direct) | `docs/plan/<f>.md` |
| Recon | Map adapter symmetry & call sites before coding | `explorer` ×N parallel (`run_in_background` + `batch_id`) | `docs/research/<topic>.md` |
| Developer | `core/` adapters, `tui/screens/*.rs`, `db/` | `executor` agent via `team action='run', team='implementation'` (or `fast-fix` for small changes) | diff + commit |
| **QAS** *(non-collapsible)* | `make check` (zero warnings) + `make test`; `feature_list.json` sync check | `verifier` + `test-engineer` (cold, built into `implementation`/`review` workflows) | `docs/reviews/<f>.md` |
| **Security** *(non-collapsible for system-mutating)* | root/firewall/users/ssh/services/ports/migrations review | `security-reviewer` agent (built into `review` workflow) | `docs/reviews/<f>.md` |
| Tech Writer | Update `feature_list.json` + `docs/` for added/renamed features | `writer` agent | `feature_list.json` diff |
| RTE | Branch, commit, PR — **only when the user asks** | Parent agent + `git-master` skill | PR |

> **Independence rule.** The agent that wrote the code is never the agent that reviews it.
> QAS (`verifier`/`test-engineer`), Security (`security-reviewer`), and code review
> (`reviewer`) always run as *fresh, cold* agents so they aren't anchored on the author's
> reasoning. The runtime enforces this: `implementation` and `review` are separate workflow
> runs, so reviewers always start cold.

---

## 2. Value stream (the flow)

### Phase 1 — Intake & architecture

1. **Intake.** Parent agent reads `feature_list.json`, confirms scope. Uses
   `team action='recommend'` if unsure which workflow to pick. Writes acceptance criteria
   into `docs/plan/<f>.md`. **Stop-the-line:** no work begins without acceptance criteria.

2. **Architecture.** Dispatch `team action='plan'` or a direct `Agent(planner)`. Design
   saved to `docs/plan/<f>.md`.

3. **Update `PROGRESS.md`.** Add the feature to the Active work table with status
   `in-progress` and link to the plan artifact.

### Phase 2 — Recon (parallel, optional)

4. **Recon fan-out.** Launch `Agent(explorer)` ×N in parallel with
   `run_in_background=true` + shared `batch_id`. One per adapter family or subsystem. Each
   writes a short summary to `docs/research/<topic>.md`. Collect results with
   `get_subagent_result` per agent.

   This is where postlab's symmetry pays off — 4 adapter reads cost the orchestrator ~4
   paragraphs, not 4 files. Skip for trivial changes.

5. **Update `PROGRESS.md`.** Note recon complete + link research artifacts.

### Phase 3 — Implementation

6. **Implementation.** Dispatch `team action='run', team='implementation'` for multi-file
   features, or `team action='run', team='fast-fix'` for 1-2 file changes. The `executor`
   agent does all code edits (the parent agent does not edit directly — unlike SAW, this runtime
   pushes implementation into a specialist agent).

   For parallel, conflict-safe work in clean repos, use `workspaceMode: 'worktree'`.

7. **Update `PROGRESS.md`.** Set status to `in-progress` (implementation phase).

### Phase 4 — Gates (non-collapsible)

8. **QAS + Code Review + Security gate.** Dispatch `team action='run', team='review'`.
   This runs 4 agents in sequence: `explorer` → `reviewer` → `security-reviewer` →
   `verifier`. Evidence lands in `docs/reviews/<f>.md`.

   If you need an unbiased second opinion after the review workflow, run a
   `cold-verifier` agent separately — it re-verifies without seeing prior analysis,
   catching confirmation bias the chained path can introduce.

9. **Review-feedback loop.** After the review gate, check each finding in
   `docs/reviews/<f>.md`. If a finding reveals a missing constraint, add it to
   `CLAUDE.md` or `agents.md` so the same mistake can't recur. See §9.

### Phase 5 — Human gates (HITL)

10. **Migration gate.** If `migrations/*.sql` was touched, parent agent asks the user for
    confirmation before proceeding. The runtime has no native HITL — the parent agent must
    gate this explicitly. Move feature to Blocked table in `PROGRESS.md` while waiting.

11. **TUI visual gate.** If `tui/screens/*.rs` changed, parent agent tells the user to run
    `sudo postlab`. Agents cannot self-certify TUI changes.

12. **CI / `install.sh` gate.** If `.github/workflows/` or `install.sh` was touched, parent
    agent asks the user for confirmation.

### Phase 6 — Docs & release

13. **Docs sync.** Dispatch `Agent(writer)` to update `feature_list.json` if a screen, tab,
    or CLI command was added or renamed.

14. **Release.** Parent agent branches, commits, opens PR — **only when the user asks**.
    Use the `git-master` skill. Put `[release]` in the commit message only when the full
    matrix should build binaries.

15. **Update `PROGRESS.md`.** Move feature from Active to Recently merged, or remove
    entirely if merged. The file should reflect current state — stale entries are worse
    than no entries.

---

## 3. Exit states (custody handoffs)

Each artifact carries a `status:` line in its frontmatter. The next agent reads the
artifact cold and continues — so context survives compaction and fresh sessions. This is a
file convention, not a runtime feature.

```
Ready-for-Architecture → Ready-for-Implementation → Ready-for-Review → Ready-for-HITL → Merged
```

`PROGRESS.md` (repo root) is the cross-session handoff file. It tracks what's active, what's
blocked, and what recently merged. A fresh parent agent reads it first to discover in-flight
work without needing the conversation history. See §6.

---

## 4. Non-collapsible gates

Always run, regardless of task size:

- **QAS** — `verifier` + `test-engineer` (built into `implementation`/`review` workflows).
- **Migration confirmation** — parent agent asks user before any `migrations/*.sql`.
- **Security review** — `security-reviewer` agent for system-mutating diffs.
- **TUI visual verification** — parent agent tells user to run `sudo postlab`.
- **CI / `install.sh` confirmation** — parent agent asks user.

## 5. Collapsibility (right-sizing)

- **Trivial** (1 file, no migration, no TUI, no system-mutating surface): collapse intake
  and architecture into the parent agent. Dispatch `team action='run', team='fast-fix'` for
  implementation. The review gate still runs (QAS + code review).
- **Single-area, 2-5 files:** `team action='run', team='implementation'`. Recon skipped.
- **Adapter-wide / multi-screen feature:** full fan-out with parallel recon, implementation
  workflow, review workflow.
- **Never** collapse the non-collapsible gates above.

> Multi-agent isn't free: every fresh spawn starts cold and re-derives context. Use fan-out
> when the work is genuinely heavy or parallel — not for a task you could do in three tool
> calls inline.

## 6. Cross-session continuity (`PROGRESS.md`)

`PROGRESS.md` in the repo root is the handoff file between sessions. It follows the
harness engineering principle: **knowledge not in the repo doesn't exist for the agent.**
A fresh parent agent reads it first to discover in-flight work, blocked features, and
recent merges — no conversation history needed.

Format:

```markdown
## Active work
| Feature | Phase | Status | Artifacts | Last updated |
|---|---|---|---|---|
| dry-run mode | implementation | in-progress | docs/plan/dry_run_mode.md | 2026-06-29 |

## Blocked
| Feature | Blocked by | Since |
|---|---|---|
| swap management | waiting on user to confirm migration | 2026-06-28 |

## Recently merged
| Feature | Merged | PR |
|---|---|---|
| Processes screen | 2026-06-27 | #42 |
```

Rules:
- **Update at every phase transition.** Each step in §2 says when.
- **Move to Blocked during HITL gates.** If waiting on user confirmation, record it.
- **Remove stale entries.** Merged features older than 5 entries drop off (git history is
  the archive). Out-of-date information is worse than no information.
- **Read first in a fresh session.** Before any other action.

## 7. Review-feedback loop

The review gate (§2, step 8) produces findings in `docs/reviews/<f>.md`. Not all findings
are one-time fixes — some reveal missing constraints. The loop:

1. **Read** `docs/reviews/<f>.md` after the review gate completes.
2. **Identify** findings that point to a missing rule (e.g., "forgot to check for root"
   → add "must verify root before mutating system state" to constraints).
3. **Encode** the rule into `CLAUDE.md` (and `agents.md` if present) so future agents
   see it before they make the same mistake.
4. **Document** the addition in the review artifact so the trail is traceable.

This is the harness engineering L10 insight: each review finding is a free constraint.
Encode it once, prevent the failure class forever.

## 8. Workflow reference

| Workflow | Agents (in order) | When to use |
|---|---|---|
| `fast-fix` | explorer → executor → verifier | 1-2 file bug fixes |
| `implementation` | planner (assess + fanout) → explorer/executor/reviewer/security-reviewer/test-engineer/verifier/writer (as planned) | Multi-file features |
| `review` | explorer → reviewer → security-reviewer → verifier | Post-implementation review gate |
| `research` | explorer → analyst → writer | Investigation before implementation |
| `parallel-research` | explorer (discover) → explorer ×4 (fan-out) → analyst (synthesize) → writer | Cross-cutting recon across subsystems |

### Key skills

| Skill | Use |
|---|---|
| `postlab` | Inject postlab project context (build, architecture, constraints) into any agent |
| `git-master` | Safe version-control work (branch, commit, PR) |
| `context-artifact-hygiene` | Constructing prompts, reading artifacts, compacting context between agents |
| `delegation-patterns` | Subagent/team delegation workflow patterns |
| `worktree-isolation` | Conflict-safe parallel edits in git worktrees |
| `verification-before-done` | Evidence-before-claims discipline for verifier/test-engineer |
| `multi-perspective-review` | Code review with simpler-alternative pass |
| `security-review` | Security review patterns with audit and detection authoring |

## 9. Worked example — add a feature across the package adapters

Goal: add a `--dry-run` flag to package install/remove operations across apt, dnf, brew,
and pacman adapters.

```
# Phase 1: Intake & architecture
# Parent agent reads feature_list.json, confirms scope
Parent: team action='plan', goal='Design dry-run mode for package adapters'
  → Agent(planner) writes docs/plan/dry_run_mode.md

# Phase 2: Recon (parallel fan-out)
Parent: Agent(explorer, run_in_background=true, batch_id='dry-run-recon',
         prompt='Map apt adapter install/remove call sites in core/packages/apt.rs')
Parent: Agent(explorer, run_in_background=true, batch_id='dry-run-recon',
         prompt='Map dnf adapter install/remove call sites in core/packages/dnf.rs')
Parent: Agent(explorer, run_in_background=true, batch_id='dry-run-recon',
         prompt='Map brew adapter install/remove call sites in core/packages/brew.rs')
Parent: Agent(explorer, run_in_background=true, batch_id='dry-run-recon',
         prompt='Map pacman adapter install/remove call sites in core/packages/pacman.rs')
# All four finish; parent collects results via get_subagent_result
# Each wrote docs/research/packages_<adapter>_dry_run.md

# Phase 3: Implementation
Parent: team action='run', team='implementation',
         goal='Add --dry-run flag to all package adapters per docs/plan/dry_run_mode.md',
         skill='postlab'

# Phase 4: Review gate (cold agents)
Parent: team action='run', team='review',
         goal='Review dry-run implementation for correctness and security',
         skill='postlab'
  → explorer → reviewer → security-reviewer → verifier
  → Evidence written to docs/reviews/dry_run_mode.md

# Phase 5: HITL (no TUI change, no migration — skip TUI/migration gates)

# Phase 6: Docs sync
Parent: Agent(writer, prompt='Update feature_list.json with dry-run feature',
         skill='postlab')

# Phase 7: Release (only if user asks)
Parent: commit with git-master skill, open PR
```

## 10. Triggering this workflow

Any of these phrases trigger the full workflow. The parent agent reads this document,
right-sizes the phases per §5, then dispatches.

| Trigger | Example |
|---|---|
| `do agentic workflow for <goal>` | `do agentic workflow for adding --dry-run to package adapters` |
| `spawn agents for <goal>` | `spawn agents for a new Processes TUI screen` |
| `run the full workflow for <goal>` | `run the full workflow for SSH key import fix` |
| `multi-agent <goal>` | `multi-agent add swap management to the system screen` |
| `fan out <goal>` | `fan out the firewall adapter audit` |
| `follow docs/agent_workflow.md for <goal>` | `follow docs/agent_workflow.md for dry-run mode` |

All are equivalent — pick whichever feels natural.

For a single-phase shortcut (skip the full value stream):

| Want | Say |
|---|---|
| Research only | `team action='run', team='parallel-research', goal='...', skill='postlab'` |
| Implement only | `team action='run', team='implementation', goal='...', skill='postlab'` |
| Review only | `team action='run', team='review', goal='...', skill='postlab'` |
| Quick fix | `team action='run', team='fast-fix', goal='...', skill='postlab'` |

### Making it automatic

To make the parent agent auto-detect when to use this workflow, add this line to
`CLAUDE.md`:

> For any non-trivial postlab feature work, follow `docs/agent_workflow.md`.

This lets the parent agent self-trigger without the user needing to remember the
invocation phrase.
