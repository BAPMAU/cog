# Resumable agent loops use the "Durable Harness" pattern

Long-running agent loops (watch a PR, a deploy, a queue) are built as a **Durable Harness**:
a thin orchestrator (the Harness) reads its next move from a durable cog `State Machine`
(phase + `Context`/poll cursor), records decisions in a cog `Ledger`, and delegates each
unit of execution to a disposable `Worker` sub-agent that rehydrates from cog and returns a
structured result. All loop memory lives in cog, never in the agent's context.

Decisive reason: the loop must survive both process death and context compaction. Keeping
the state in the agent's own context loses it on compaction; externalising it to cog lets
any fresh process (Harness or Worker) resume exactly where the last left off. The Harness
stays thin so it does not itself get compacted; Workers stay stateless so they never drift.

`watch-pr` is the first instance of this pattern; future watch-style skills should follow it
rather than reinventing in-context loops.

## Consequences

The Harness is the sole writer of the State Machine (single-writer invariant); Workers only
read the Ledger and return data. The pattern presumes the work decomposes into batches a
fresh Worker can pick up from cog alone — work that needs deep in-context continuity is a
poor fit.
