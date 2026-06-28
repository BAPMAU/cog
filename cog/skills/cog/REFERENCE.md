# cog — reference

Disclosed detail for [`cog`](SKILL.md): install, the full fsm schema, durability.

## Install / update

`cog` installs to `~/.cargo/bin/cog` (on PATH). Install or update from the public repo
(needs a Rust toolchain — `rustup`):

```sh
cargo install --git https://github.com/BAPMAU/cog cog --locked      # install or update
```

`cargo install` freezes a snapshot of the binary; re-run the same command to pull a newer version.

### Developing cog itself

From a local clone, `cargo install-local` (alias for `cargo install --path .`) installs the
working copy; `./target/debug/cog` is always the freshest build without reinstalling.

## fsm definition schema

```json
{
  "states": [
    {"name": "idle"},
    {"name": "polling", "description": "waiting for events"},
    {"name": "done", "terminal": true}
  ],
  "transitions": [
    {"from": "idle", "to": "polling", "criterion": "watch started"},
    {"from": "polling", "to": "done"}
  ],
  "initial": "idle"
}
```

- `states[]` — `name` (required, identity); `terminal` (default `false`); `description` (optional).
- `transitions[]` — `from`, `to` (required, must be declared states); `criterion`, `description`
  (optional free text documenting the edge — not auto-evaluated).
- `initial` — starting state (must be declared).

An edge or `initial` referencing an undeclared state fails with `unknown_state` (domain,
exit 2). The whole definition (rules + current state) is persisted, so a machine rehydrates
from its name alone.

## Cursor (Context) vs set-membership dedup

For a **resumable position** (high-water marks), keep it in the fsm **Context** so phase and
cursor advance in one transaction: `cog fsm transition <name> <state> --context '<json>'`,
read it back with `cog fsm state <name>` → `value.context`. The blob is opaque (cog never
validates its shape) and replaced wholesale. There is no standalone cursor entity — a cursor
lives inside a machine (root ADR 0001).

For **set-membership dedup** (which exact items were handled, not just "up to where"), append
handled items to a `log` stream and `query` it on the next run to skip what's already there.

## Durability / concurrency

Each command runs in one `IMMEDIATE` SQLite transaction (WAL + busy_timeout), so multiple
`cog` processes can share a store safely and a failed command rolls back atomically.
