# watch-pr

A skill that watches a GitHub pull request and resolves CI failures and review comments
autonomously, using cog as external memory so the watch survives long runs and context
compaction.

## Language

**Durable Harness**:
The reusable pattern this skill instantiates: a thin Harness reads its next move from a
durable cog State Machine and delegates execution to disposable Workers, so the loop
survives process death and context compaction. See root ADR 0002.
_Avoid_: durable loop, supervised loop

**Harness**:
The long-lived, thin orchestrator loop — in practice the main agent session itself. It
polls the PR, owns all writes to the cog State Machine, decides what to do, spawns Workers,
and sleeps. It externalises its memory to cog because its own context gets compacted.
_Avoid_: watcher, loop, conductor, main agent

**Worker**:
An ephemeral sub-agent spawned to handle exactly one batch of work (a CI fix, or a batch of
comments), which commits, returns a structured JSON result, then is discarded. It rehydrates
from cog (reads the Ledger for coherence) rather than holding state across spawns.
_Avoid_: handler, executor, sub-task agent

**Poll cursor**:
The high-water marks (last-seen comment ids, CI head sha) the Harness keeps in the State
Machine's Context to answer "is anything new since last poll?". Advanced only as far as the
last contiguously-handled item.
_Avoid_: offset, pointer, Cursor (cog has no Cursor entity — see root ADR 0001)
