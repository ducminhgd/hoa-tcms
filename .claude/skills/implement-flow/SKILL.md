---
name: implement-flow
description: >
  Orchestrate a coder-reviewer loop for implementation tasks. Spawns a coder agent
  to implement and a reviewer agent to review, iterating until all issues are resolved.
  Use when the user wants rigorous, reviewed implementation — especially for
  safety-critical, data-sensitive, or complex code changes.
metadata:
  version: 1.0
  effort: high
  tags:
    - implementation
    - code-review
    - quality
    - loop
    - agent-team
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
  - Bash
  - Agent
  - AskUserQuestion
  - Skill
  - TaskCreate
  - TaskStop
  - WebSearch
  - WebFetch
---

# Implement Flow — Coder ↔ Reviewer Loop

Orchestrate a tight loop between two agents: **coder** (implements) and
**reviewer** (reviews). Iterate until zero unresolved issues, then finalize.

## Voice

- Mediator. Neutral. Precise.
- Relay findings verbatim — don't soften, don't interpret.
- Track unresolved count. Announce each round.

## Agents

| Agent | Definition | Role |
|---|---|---|
| **coder** | `.claude/agents/coder.md` | Writes code. Proposes fixes. |
| **reviewer** | `.claude/agents/reviewer.md` | Reviews code. Raises issues. Approves fixes. |

## Loop

```
ROUND 1: IMPLEMENT  → coder writes the code
ROUND 1: REVIEW     → reviewer reviews, produces findings
         ↓
ROUND 2: FIX        → coder proposes fixes for each finding
ROUND 2: RE-REVIEW  → reviewer approves/rejects each fix
         ↓
...repeat until 0 unresolved findings...
         ↓
FINALIZE            → coder applies all approved fixes & update the bruno sample requests
```

## Protocol

### Step 1 — Understand Task

Ask the user: *"What needs to be implemented? Describe the task."*

If the answer is underspecified, ask:
- What files / modules are involved?
- What's the acceptance criteria?
- Any constraints (performance, compatibility, dependencies)?

### Step 2 — Spawn Coder (Round 1: Implement)

```
Agent "Implement the following task: <task>.
- Read existing code first.
- Write production-grade code.
- Do NOT write tests unless asked.
- Report: what files created/modified, a summary of changes."
```

Use agent type: the coder persona (`subagent_type` from the coder agent definition).

Record:
- Files changed
- Summary of implementation

### Step 3 — Spawn Reviewer (Round 1: Review)

```
Agent "Review the following implementation: <coder's summary + files>.
- Read all changed files.
- Produce findings with severity: CRITICAL, HIGH, MEDIUM, LOW.
- For each finding: file, line, what's wrong, failure scenario, recommendation.
- Be precise. No vague feedback."
```

Use agent type: the reviewer persona.

Record findings. Count unresolved.

If **0 findings**: skip to Step 6 (Finalize).

### Step 4 — Spawn Coder (Round N: Fix Proposal)

For each finding, present to coder:

```
Agent "Reviewer found the following issue in your code:

<finding with severity, file, line, issue, recommendation>

Propose a fix. Respond with:
- Agree/Disagree and why
- If agree: the exact code change (before → after)
- If disagree: your reasoning"
```

Collect all fix proposals.

### Step 5 — Spawn Reviewer (Round N: Re-Review)

Present proposals to reviewer:

```
Agent "Coder responded to your findings. For each:

<original finding + coder's response>

For each proposal:
- ACCEPTED: fix resolves the issue
- REJECTED: fix doesn't resolve — state what's still wrong
- WITHDRAWN: coder convinced you the finding was invalid

Respond per finding. No new findings — only rule on the existing ones."
```

Tally results:
- **All ACCEPTED/WITHDRAWN** → proceed to Step 6.
- **Any REJECTED** → return to Step 4 with reviewer's rejection reason.
- **Stuck** (same finding rejected 3 times) → escalate to user: present both
  sides, ask for a decision.

### Step 6 — Finalize

Spawn coder one last time:

```
Agent "Apply all accepted fixes: <list of ACCEPTED proposals>.
Implement the exact changes. Do not introduce anything new. Make sure code lines are formatted, run `make fmt` and fix"
```

Then spawn reviewer for final sign-off:

```
Agent "Verify the final code. All prior findings were addressed.
Confirm: no regressions, code is correct. Final verdict: APPROVED or BLOCKED."
```

### Step 7 — Report

```
## Implement Flow — Complete

### Rounds: N
### Files: [list]
### Findings: X total, 0 unresolved
### Final Verdict: APPROVED

### Summary
[What was implemented. What was fixed during review.]
```

## Rules

- **One round at a time.** Don't spawn round N+1 until round N is complete.
- **Coder never sees reviewer's raw output.** Mediate — pass findings cleanly.
- **Reviewer never sees coder's raw code dumps.** Pass summaries + file paths.
- **No scope creep.** Reviewer only flags issues in the changed code.
- **Escalate on deadlock.** If coder and reviewer disagree 3 times on the same
  finding, ask the user to decide.
- **Stop on CRITICAL.** If reviewer flags a CRITICAL that requires design
  change (not a simple fix), pause and ask the user before continuing.
