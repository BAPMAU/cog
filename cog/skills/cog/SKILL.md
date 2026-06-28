---
name: cog
description: External persistent memory for agents via the `cog` CLI (SQLite): state that survives process runs and context compaction. Use when a multi-run loop or poll must persist then later recover its state — an append-only event journal (`cog log`) for a ledger/history, or a data-defined state machine (`cog fsm`) for a lifecycle/cursor that outlives the conversation. Example: watch-pull-request's poll cursor and per-thread status.
---

# cog — external memory for agents

`cog` is a SQLite-backed CLI for state that must **survive between runs / compaction**.
Every command prints **JSON on stdout**; the exit code carries the outcome.

Check it's installed with `cog --help`. If `command not found`, install it (needs a Rust
toolchain), then retry the command:

```sh
cargo install --git https://github.com/BAPMAU/cog --locked
```

See [REFERENCE.md](REFERENCE.md) if `cargo` is unavailable or to update an existing install.

## The contract (read first)

- **stdout is always JSON**: `{"ok":true,"value":…}` or `{"ok":false,"error":{"kind","code","message"}}`. Parse stdout; help/usage text goes to stderr.
- **Exit codes**: `0` ok · `2` domain error — expected, branch on `error.code`, don't retry · `64` usage error — your call is malformed, fix the args · `70` technical — infra, may retry.
- **Single-quote the JSON** so the shell leaves it untouched: `cog log add s '{"a":1}'`.
- **One store per task** via `--store <path>` (default `.cog/state.db` in cwd), e.g. `--store .cog/watch-pr-1234.db`.

## `log` — append-only event journal

An ordered, append-only stream of JSON entries: a ledger / history.

```sh
cog log add <stream> '<json>'     # → value.seq  (monotonic per stream)
cog log query <stream>            # → value.entries: [{seq,at,payload}, …]  newest first
```

Querying a never-written stream is an expected domain error `empty_stream` (exit 2) —
treat empty as a normal outcome, not a failure.

## `fsm` — state machine with data-defined rules

A lifecycle whose states and transitions are **data**, not hard-coded. Define once, then
advance step by step; illegal moves are refused.

```sh
cog fsm define <name> '<def-json>'   # → value.current  (starts at "initial")
cog fsm transition <name> <state>    # → value.current  (validates the move)
cog fsm state <name>                 # → value.current
```

Minimal definition (full schema in [REFERENCE.md](REFERENCE.md)):

```json
{"states":[{"name":"idle"},{"name":"done","terminal":true}],
 "transitions":[{"from":"idle","to":"done"}],
 "initial":"idle"}
```

Transition failures are all domain errors (exit 2), so they're control flow, not crashes:
`illegal_transition` (move not allowed, or source is terminal), `unknown_state` (target not
declared), `not_initialized` (machine never defined).

## Pattern: survive a compaction

Persist what the context would lose, then recover it on the next run:

```sh
cog fsm define watch-pr-1234 '<def>'                      # once: the loop's lifecycle
cog log add "pr-1234:reviews" '{"comment_id":42}'         # each event: record it…
cog fsm transition watch-pr-1234 processing               # …and advance the lifecycle
# after a compaction / new run — recover where you were:
cog fsm state watch-pr-1234                               # current phase
cog log query "pr-1234:reviews"                           # what you handled (dedup against this)
```

[REFERENCE.md](REFERENCE.md): fsm definition schema, install/update, durability.
