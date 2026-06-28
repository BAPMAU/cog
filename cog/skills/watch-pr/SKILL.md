---
name: watch-pr
description: >
  DURABLE variant (built on cog) of watch-pull-request: watches a GitHub PR — red CI
  and new review comments — and handles them continuously, but externalises all its
  memory (phase + poll cursor + decision journal) into cog (SQLite), so the loop
  SURVIVES process death and context compaction. Adaptive 2–5 min pauses, debounce,
  hard-stop escalation.

  USE this skill when the user says "/watch-pr", "watch my PR (durable)", "watch the PR
  over time", "babysit the PR for a while", "resumable PR watch", "surveille ma PR sur la
  durée", or asks for a PR watch that must last / survive compaction.

  DO NOT use for a one-shot review (→ /code-review), nor to answer a single comment. For a
  short watch with no persistence, /watch-pull-request is enough.
---

# watch-pr workflow — Durable Harness on cog

You are the **Harness**: a **thin** orchestrator that keeps NOTHING in context. All memory
(phase, poll cursor, decisions) lives in **cog**. Load the `cog` skill if you don't know its
commands. You are the **sole writer** of the State Machine; **Workers** (disposable sub-agents)
read the Ledger and return JSON, they never write the FSM.

**Autonomous** loop: fix CI, reply, resolve, push **without confirmation** (the global "only
push when asked" rule is lifted here). You interrupt ONLY to escalate (§4).

> **Exit rule** — every time you **hand back** (escalation §4, stop §5, a pause that awaits the
> user), restate the **PR URL** in plain text (`gh pr view PR -q .url`).

## 0. Init / resume (idempotent)

Identity: store `.cog/watch-pr-<n>.db`, FSM `watch-pr-<n>`, streams `pr-<n>:ledger` (decisions)
and `pr-<n>:escalations`. Exact cog/gh commands: [REFERENCE.md](REFERENCE.md).

1. **Target**: arg = PR number/URL; else the current branch's PR. None → ask, stop.
2. **Resume**: `cog fsm state watch-pr-<n>`. If it exists → **rehydrate** from its phase +
   Context (cursor) + the Ledger, and resume. Otherwise `cog fsm define` the FSM (schema in
   REFERENCE) with an empty-cursor initial Context, state `triage`.
3. **Cap**: note the start time. Default stop after **2 h** or **30 cycles**.
4. If you resume while the phase is a work phase (`fix_ci`/`handle_comments`, a run was
   in-flight) → **escalate** (§4): never replay a batch blind.

## 1. The loop (poll → triage → dispatch)

1. **Poll** the state (CI + 3 comment sources + `reviewDecision`) and compute the **delta** vs
   the **poll cursor** (high-water marks) in the Context. See REFERENCE.
2. **Nothing new** → transition `triage → await_review`, **adaptive pause** (background sleep:
   ~120 s if recently active, back off to ~300 s when quiet), then re-poll (`await_review → triage`).
3. **New comment** → **debounce 60–90 s** (background sleep) to group siblings.
4. **Triage** (you, not a Worker) — pick by **priority**: `fix_ci` **>** `handle_comments` **>**
   `await_review`. Transition to the chosen phase, run the matching **Worker** (§2), then
   advance the cursor and return to `triage` (§3).

## 2. Workers (one per batch)

For red CI or a comment batch, spawn **one Worker** (`Explore`/`general-purpose`). Strict
contract (full prompt + JSON schema in REFERENCE):

- **In**: store path, PR (number + owner/repo), current FSM phase, the **delta items** only.
  The Worker reads `pr-<n>:ledger` for coherence.
- **Does**: the CI fix **or** the comment handling (reply +/− thread resolution), atomic
  conventional commits (load `git`), push.
- **Out (JSON)**: `{handled_state, items:[{thread_id,decision,commit,thread_resolved,replied,
  root_cause}], ci_fixed, escalate?}`.

You (Harness) **log** each returned item to `pr-<n>:ledger`, then advance the cursor. Never two
Workers in parallel on the same file; when in doubt, serialize.

## 3. Advance the cursor (atomic)

After a Worker, transition `phase → triage` **with** the new Context (`--context`), in the
**same command** (phase + cursor move together). Advance each high-water mark **only up to the
last contiguously-handled item**: a mid-batch escalation must never skip the unhandled tail.
Mark details in REFERENCE.

## 4. Escalation — hard stop

When a Worker returns `escalate`, or you hit a critical topic (architecture, out of scope,
reviewers disagree, in-flight resume §0.4):

1. `cog log add pr-<n>:escalations '<summary+options+reco>'` (survives compaction).
2. Transition `→ escalated`. **The cursor does NOT advance** (the item stays to be re-handled).
3. Play the sound (REFERENCE), write a **clear summary** in the conversation, restate the URL.
4. **Hand back.** The user's next message = the decision → transition `escalated → triage` and
   resume. (After compaction, rehydrate from `fsm state` + the `pr-<n>:escalations` stream.)

## 5. Stop conditions

Transition `→ done` (terminal) then final summary (fixed / replied / resolved / still open) if:
- **PR merged/approved** (`state == MERGED/CLOSED` or `reviewDecision == APPROVED`);
- **cap** reached (duration or cycles); **manual** interruption.

⚠️ A **clean state** (green CI + threads handled) does **NOT** stop the loop: keep watching
(`await_review`), new comments may still arrive.
