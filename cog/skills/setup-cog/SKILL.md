---
name: setup-cog
description: Installs the `cog` binary (the Rust/SQLite CLI behind the `cog` skill) and verifies it works on the PATH. Run once after adding the cog skill to a machine, or when `cog --help` reports `command not found`. Builds from the public repo with `cargo install --git`, so it needs a Rust toolchain.
disable-model-invocation: true
---

# Setup cog

Make the `cog` command available on this machine. The `cog` skill is just docs; it
needs the compiled binary to do anything. This skill installs it and proves it runs.

Run the steps in order. Stop and report as soon as a step fails — don't guess past it.

## 1. Already installed?

```sh
cog --help
```

If this prints the usage text (exit 0), `cog` is already on the PATH. Tell the user the
version is current and offer to update it (step 3 re-run pulls the latest). Skip to step 4
to confirm it still works. Otherwise (`command not found`), continue.

## 2. Check the toolchain

Installation builds from source, so a Rust toolchain is required:

```sh
command -v cargo
```

- **cargo found** → go to step 3.
- **cargo missing** → stop. Tell the user to install Rust first (`https://rustup.rs`,
  the one-liner `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`), open a
  new shell so `~/.cargo/bin` is on the PATH, then re-run `/setup-cog`. Do not install Rust
  for them without asking — it modifies their shell profile.

## 3. Install

```sh
cargo install --git https://github.com/BAPMAU/cog cog --locked
```

This compiles `cog` and places it in `~/.cargo/bin/cog` (~20–30 s). The same command later
pulls a newer version. If it fails:

- `could not find Cargo.toml` / package `cog` not found → the repo URL is wrong; confirm it.
- `~/.cargo/bin` not on PATH afterwards → add it (`export PATH="$HOME/.cargo/bin:$PATH"`)
  and have the user persist it in their shell profile.

## 4. Verify

Presence:

```sh
cog --help
```

Then a real round-trip against a throwaway store, to prove SQLite works end-to-end:

```sh
cog --store /tmp/cog-setup-check.db log add setup '{"ok":true}'   # → {"ok":true,"value":{"seq":1}}
cog --store /tmp/cog-setup-check.db log query setup                # → entries with that payload
rm -f /tmp/cog-setup-check.db*
```

Both commands must print `{"ok":true,...}` on stdout and exit 0.

## 5. Done

Report that `cog` is installed and verified, with its path (`which cog`). Point the user at
the `cog` skill for usage (`cog log` for event journals, `cog fsm` for state machines).
