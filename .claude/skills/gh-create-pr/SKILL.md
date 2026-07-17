---
name: gh-create-pr
description: >
  Create a PR to main, then run a reviewer↔coder loop on it until all issues are
  resolved. Reviewer posts inline comments on the PR diff. Coder stress-tests
  proposed fixes with grill-me before applying them. Finally updates bruno sample
  requests. Use when the user wants a reviewed, production-grade PR that has been
  adversarially interrogated before merge.
metadata:
  version: 1.0
  effort: high
  tags:
    - github
    - pull-request
    - code-review
    - grill-me
    - quality
    - agent-team
    - bruno
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
  - mcp__github__create_pull_request
  - mcp__github__pull_request_read
  - mcp__github__pull_request_review_write
  - mcp__github__add_comment_to_pending_review
  - mcp__github__get_commit
  - mcp__github__push_files
  - mcp__github__get_file_contents
  - mcp__github__create_or_update_file
---

# GH Create PR — Reviewer ↔ Coder Loop on GitHub

Create a PR to `main`, then run a tight loop: **reviewer** audits the diff and
posts inline comments, **coder** stress-tests proposed fixes with `/grill-me`
before applying them. Loop until zero unresolved threads. Update bruno samples.

## Voice

- Mediator. Neutral. Precise.
- Relay between GitHub PR and agents. Don't soften findings.
- Track unresolved thread count. Announce each round.

## Agents

| Agent | Definition | Role |
|---|---|---|
| **reviewer** | `.claude/agents/reviewer.md` | Reviews PR diff. Posts inline comments. Approves/rejects fixes. |
| **coder** | `.claude/agents/coder.md` | Reads review comments. Uses `/grill-me` to stress-test fixes. Applies fixes. Pushes. |

## Flow

```
STEP 1: CREATE PR      → push branch, create PR to main
STEP 2: REVIEW         → spawn reviewer, post inline comments
STEP 3: FIX + GRILL    → spawn coder, grill-me each fix, apply, push
STEP 4: RE-REVIEW      → spawn reviewer, verify fixes, resolve or re-open
         ↓
...loop 2→3→4 until 0 unresolved threads...
         ↓
STEP 5: BRUNO          → update bruno sample requests
STEP 6: REPORT         → final summary
```

---

## Step 1 — Create PR

### 1.1 Gather Context

Ask the user:
- *"What branch are we creating the PR from?"*
- *"One-line summary of the change?"*

If the branch isn't pushed yet: `git push -u origin <branch>`.

### 1.2 Generate Description

Read the branch's commits and changed files. Generate a PR description:

```
<concise summary of what changed>

### Changes
- <bullet list of key changes>

### Testing
- <how to verify>
```

### 1.3 Create the PR

```
mcp__github__create_pull_request(
  owner, repo, title="<short title>",
  head="<branch>", base="main",
  body="<description>"
)
```

Record the PR number. Announce: `PR #N created: <url>`.

---

## Step 2 — Spawn Reviewer (Review)

### 2.1 Fetch PR Diff

```
mcp__github__pull_request_read(method="get_diff", owner, repo, pullNumber=N)
mcp__github__pull_request_read(method="get_files", owner, repo, pullNumber=N)
```

### 2.2 Spawn Reviewer Agent

Pass the diff + files to the reviewer agent:

```
Agent "Review this PR diff. Use the reviewer persona.

PR #N: <title>
Branch: <branch> → main

<diff content>

For each finding:
- Post as an inline review comment on the PR.
- Use severity: CRITICAL | HIGH | MEDIUM | LOW.
- Include: file, line, what's wrong, failure scenario, suggested fix.

Start a pending review. Add all comments. Submit as COMMENT (not approve/request changes).

Tool reference:
- mcp__github__pull_request_review_write(method='create', event='COMMENT', owner, repo, pullNumber=N, body='Review summary')
- mcp__github__add_comment_to_pending_review(owner, repo, pullNumber=N, path, line, body, subjectType='LINE')"

Use subagent_type matching the reviewer persona definition.
```

### 2.3 Collect Findings

After reviewer completes:
- Count unresolved threads.
- List each finding: thread ID, severity, file, line, summary.

If **0 findings**: skip to Step 5 (Bruno).

---

## Step 3 — Spawn Coder (Fix + Grill)

### 3.1 Fetch Review Comments

```
mcp__github__pull_request_read(method="get_review_comments", owner, repo, pullNumber=N)
```

### 3.2 Spawn Coder Agent

For each unresolved finding, present to coder:

```
Agent "Review comments on your PR #N. Use the coder persona.

<finding: severity, file, line, reviewer's comment, suggested fix>

For each finding:
1. Read the relevant code.
2. Use the grill-me skill to stress-test the proposed fix before applying it:
   - What's the worst-case if this fix is wrong?
   - Does it introduce new edge cases?
   - Is there a simpler fix?
   - What existing behaviour could this break?
3. After grill-me passes: apply the fix to the code.
4. Push the fix to branch <branch>.

Report per finding:
- FIXED: what you changed and why
- DISAGREE: why the finding is invalid (be specific)
- NEEDS CLARIFICATION: what you need from the reviewer"

Use subagent_type matching the coder persona definition.
```

### 3.3 Push Loop Safety

- Coder should push after each batch of fixes.
- Verify `git status` is clean after each push.
- If push fails due to conflicts: stop, escalate to user.

---

## Step 4 — Spawn Reviewer (Re-Review)

### 4.1 Fetch Updated PR

```
mcp__github__get_commit(owner, repo, sha="<branch>")
mcp__github__pull_request_read(method="get_diff", owner, repo, pullNumber=N)
```

### 4.2 Spawn Reviewer Agent

```
Agent "Re-review PR #N. Coder has addressed your findings.

Original findings + coder's responses:
<List each: original comment, coder's action (FIXED/DISAGREE/NEEDS CLARIFICATION)>

For each thread:
- If FIXED and correct: resolve the thread (method='resolve_thread').
- If FIXED but insufficient: leave unresolved, add follow-up comment explaining what's still wrong.
- If DISAGREE and coder is right: resolve the thread, note 'withdrawn'.
- If DISAGREE and coder is wrong: leave unresolved, add counter-argument.
- If NEEDS CLARIFICATION: answer the question, leave unresolved.

No new findings unrelated to existing threads. Only rule on what's in front of you.

Tool reference:
- mcp__github__pull_request_review_write(method='resolve_thread', threadId='...')
- mcp__github__add_reply_to_pull_request_comment(owner, repo, pullNumber=N, commentId=..., body='...')"

Use subagent_type matching the reviewer persona definition.
```

### 4.3 Tally

- **All threads resolved or withdrawn** → proceed to Step 5.
- **Unresolved threads remain** → return to Step 3.
- **Same thread unresolved 3 times** → escalate to user: present both sides, ask for decision.

---

## Step 5 — Update Bruno Samples

### 5.1 Check for API Changes

Compare the PR diff against `docs/api/`. Identify:
- New endpoints → needs a new `.bru` file.
- Modified endpoints (path, method, body, response shape) → update existing `.bru`.
- Deleted endpoints → remove or mark deprecated in `.bru`.

### 5.2 Bruno File Format

```
meta {
  name: <Short Name>
  type: http
  seq: <N>
}

<method> {
  url: {{baseUrl}}/api/v1/<path>
  body: <json | none | multipartForm>
  auth: inherit
}

<if body is json, include example payload>

assert {
  res.status: eq <expected status>
  <additional assertions>
}

docs {
  # <Title>
  <Brief description of what the endpoint does and key responses>
}
```

### 5.3 Generate/Update Files

For each affected endpoint, create or update the corresponding `.bru` file in
`docs/api/`. Match the existing naming convention and directory structure
(e.g. `docs/api/users/<resource>.bru` for resource-specific endpoints).

---

## Step 6 — Report

```
## PR #N — Review Complete

**PR:** <url>
**Branch:** <branch> → main
**Rounds:** N rounds of review
**Findings:** X total, 0 unresolved
**Verdict:** READY TO MERGE

### Files Changed
- <list>

### Review Summary
<What was flagged and fixed>

### Bruno Samples
<Which .bru files were created/updated>

### Next Steps
- [ ] Merge PR
- [ ] Deploy
```

---

## Rules

- **One round at a time.** Wait for each agent to finish before spawning the next.
- **Coder always grill-me's fixes.** Stress-test before applying — never skip.
- **Reviewer never approves the PR.** Only resolves threads. Human merges.
- **No scope creep.** Reviewer only flags issues in the diff. Coder only fixes
  what reviewer flagged.
- **Escalate on deadlock.** 3-round stalemate → user decides.
- **Stop on CRITICAL.** If reviewer flags a CRITICAL requiring design change,
  pause and ask the user before continuing.
- **Branch hygiene.** Coder pushes to the PR branch. Never force-push.
