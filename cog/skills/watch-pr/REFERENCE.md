# REFERENCE — watch-pr (FSM, cog, Worker, gh)

Disclosed detail for [`watch-pr`](SKILL.md). Replace `<n>` with the PR number,
`OWNER`/`REPO` with the repo (`gh repo view --json owner,name`).

## State Machine

Business states: `triage` (router, initial) → `fix_ci` | `handle_comments` | `await_review`
→ back to `triage`; plus `escalated` and `done` (terminal). Priority is in the triage:
`fix_ci > handle_comments > await_review`.

```sh
cog --store .cog/watch-pr-<n>.db fsm define watch-pr-<n> '{
  "states":[
    {"name":"triage"},{"name":"fix_ci"},{"name":"handle_comments"},
    {"name":"await_review"},{"name":"escalated"},{"name":"done","terminal":true}
  ],
  "transitions":[
    {"from":"triage","to":"fix_ci"},{"from":"triage","to":"handle_comments"},
    {"from":"triage","to":"await_review"},{"from":"triage","to":"escalated"},
    {"from":"triage","to":"done"},{"from":"fix_ci","to":"triage"},
    {"from":"fix_ci","to":"escalated"},{"from":"handle_comments","to":"triage"},
    {"from":"handle_comments","to":"escalated"},{"from":"await_review","to":"triage"},
    {"from":"await_review","to":"done"},{"from":"await_review","to":"escalated"},
    {"from":"escalated","to":"triage"},{"from":"escalated","to":"done"}
  ],
  "initial":"triage"
}' --context '{"ci_head":null,"ci_red":[],"last_issue_comment_id":0,"last_review_comment_id":0,"last_review_id":0,"review_decision":null}'
```

## Poll cursor (Context)

High-water marks carried in the Context, advanced in the **same transaction** as the phase:

| mark | meaning |
|---|---|
| `ci_head` | last CI head sha seen |
| `ci_red[]` | red checks already taken on (don't re-spawn a Worker for them) |
| `last_issue_comment_id` | greatest issue-comment id handled |
| `last_review_comment_id` | greatest review-comment (inline) id handled |
| `last_review_id` | greatest review id handled |
| `review_decision` | last `reviewDecision` seen (stop condition) |

**Advance rule**: move a mark only up to the **last contiguously-handled item**. If an
escalation cuts a batch mid-way, downstream items stay below the mark → they get re-polled and
re-handled. Read the cursor with `cog fsm state watch-pr-<n>` → `value.context`.

Advance phase + cursor together after a Worker:

```sh
cog --store .cog/watch-pr-<n>.db fsm transition watch-pr-<n> triage \
  --context '{"ci_head":"<sha>","ci_red":[],"last_issue_comment_id":128,"last_review_comment_id":540,"last_review_id":77,"review_decision":"CHANGES_REQUESTED"}'
```

## Ledger and escalations

```sh
cog --store .cog/watch-pr-<n>.db log add pr-<n>:ledger '{"thread_id":"PRRT_x","decision":"resolved","commit":"abc123","root_cause":"missing convention"}'
cog --store .cog/watch-pr-<n>.db log query pr-<n>:ledger        # cross-batch coherence (newest first)
cog --store .cog/watch-pr-<n>.db log add pr-<n>:escalations '{"subject":"...","options":["..."],"reco":"..."}'
```

## Worker contract

Spawn one `Explore`/`general-purpose` per batch. Prompt:

> You are a disposable Worker of the watch-pr harness. External memory: cog store
> `.cog/watch-pr-<n>.db`. PR `<n>` on `OWNER/REPO`. FSM phase: `<fix_ci|handle_comments>`.
> Read the Ledger for coherence: `cog --store .cog/watch-pr-<n>.db log query pr-<n>:ledger`.
> Items to handle (delta only): `<JSON list>`.
> Handle **only** these items: fix CI **or** answer comments (reply + resolve the thread if
> actionable & clear; reply only if it's a question/disagreement). Atomic conventional commits
> + push. Serialize edits on the same file. **Never write the State Machine.** If an item is
> beyond your mandate (architecture, out of PR scope, reviewers disagree) → don't decide, mark
> it `escalate`. Reply with **only** this JSON:
> ```json
> {"handled_state":"<phase>","items":[{"thread_id":"","decision":"resolved|replied|skipped","commit":"","thread_resolved":true,"replied":true,"root_cause":""}],"ci_fixed":false,"escalate":null}
> ```

On return: log each `item` to `pr-<n>:ledger`, then advance the cursor (except escalated
items). Non-null `escalate` → SKILL §4.

## `gh` commands (poll / reply / resolve)

```sh
# PR state (target + stop conditions)
gh pr view <n> --json number,headRefName,url,state,reviewDecision,mergeable

# Red CI
gh pr checks <n> --json name,state,bucket,link                     # bucket=fail → red
gh run list --branch "$(gh pr view <n> --json headRefName -q .headRefName)" \
  --json databaseId,workflowName,status,conclusion,headSha --limit 20
gh run view <databaseId> --log-failed                              # failed-step logs only

# 3 comment sources (diff on id > cursor mark; skip your own comments)
gh api --paginate repos/OWNER/REPO/issues/<n>/comments  --jq '.[] | {id,user:.user.login,created_at,body}'
gh api --paginate repos/OWNER/REPO/pulls/<n>/comments   --jq '.[] | {id,user:.user.login,path,line,in_reply_to:.in_reply_to_id,created_at,body}'
gh api --paginate repos/OWNER/REPO/pulls/<n>/reviews    --jq '.[] | {id,user:.user.login,state,submitted_at,body}'

# Review threads with resolution state (node id = threadId)
gh api graphql -f query='query($owner:String!,$repo:String!,$num:Int!){repository(owner:$owner,name:$repo){pullRequest(number:$num){reviewThreads(first:100){nodes{id isResolved isOutdated comments(first:50){nodes{databaseId author{login} path line body createdAt}}}}}}}' -F owner=OWNER -F repo=REPO -F num=<n>

# Reply
gh api repos/OWNER/REPO/pulls/<n>/comments/<comment_id>/replies -f body="…"   # inline thread
gh pr comment <n> --body "…"                                                   # general conversation

# Resolve a thread
gh api graphql -f query='mutation($threadId:ID!){resolveReviewThread(input:{threadId:$threadId}){thread{isResolved}}}' -f threadId="<threadId>"
```

## Pause & escalation

Foreground `sleep` is blocked → run it in the background; you're re-invoked when it ends.
Cadence: ~120 s when active, back off to ~300 s when quiet, 60–90 s debounce after a new
comment. Default cap: 2 h or 30 cycles.

```sh
# Bash(command="sleep 240", run_in_background=true)   # quiet pause ~5 min
# Bash(command="sleep 120", run_in_background=true)   # active pause ~2 min
# Bash(command="sleep 75",  run_in_background=true)   # debounce ~1 min
afplay /System/Library/Sounds/Funk.aiff               # escalation
```
