# Context Map

## Contexts

- [cog](./CONTEXT.md) — the durable-memory CLI: `Ledger` and `State Machine` primitives over SQLite.
- [watch-pr](./skills/watch-pr/CONTEXT.md) — a skill that watches a GitHub PR, using cog as external memory.

## Relationships

- **watch-pr → cog**: the `Harness` stores its phase and poll cursor in a cog `State Machine`
  (in its `Context`) and records its decisions in a cog `Ledger`; `Worker`s read that Ledger
  to stay coherent across spawns. watch-pr is a consumer of cog; cog knows nothing of it.
