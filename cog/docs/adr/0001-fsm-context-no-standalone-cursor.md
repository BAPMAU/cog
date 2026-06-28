# State Machine carries a mutable Context; no standalone Cursor entity

A polling consumer (e.g. a watch-pull-request harness) must persist both a lifecycle phase
and a resumable poll position. We make the State Machine carry a small mutable JSON
**Context**, advanced atomically with each transition, instead of building the separate
`Cursor` / `SeenSet` entities sketched in the roadmap (`work.md`, Iteration 3).

Decisive reason: phase and position must move together. Folding the position into the
machine's Context lets one `cog fsm transition` advance both in a single SQLite transaction;
two separate entities would need two transactions and could desync if the process crashed
between them. GitHub comment ids are monotonic, so a high-water mark in the Context is enough
— a full `SeenSet` is unnecessary here.

## Consequences

A cursor cannot exist without a State Machine. A future consumer that polls without any
lifecycle would justify reintroducing a standalone `Cursor`; until such a consumer exists,
it stays out of the model.
