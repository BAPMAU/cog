# cog

External persistent memory for agents: a SQLite-backed CLI exposing durable primitives
that survive between process runs and context compaction.

## Language

**Store**:
A single SQLite database file holding all state for one task, selected with `--store`.
_Avoid_: database, db file

**Ledger**:
An append-only, ordered journal of JSON entries — the history primitive.
_Avoid_: log table, audit trail

**Stream**:
A named sequence inside a Ledger; its entries are ordered by a monotonic `seq`.
_Avoid_: topic, channel

**State Machine**:
A lifecycle whose states and transitions are defined as data (not hard-coded) and whose
current state is enforced at runtime — the lifecycle primitive (CLI command: `fsm`).
_Avoid_: workflow, state chart

**State**:
One named node of a State Machine, optionally `terminal`. The machine sits in exactly one
at a time.
_Avoid_: status, step

**Context**:
A small mutable JSON payload a State Machine carries alongside its current State, advanced
in the **same transaction** as a transition. Where a consumer keeps its resumable position.
_Avoid_: Cursor, metadata, data blob
