//! End-to-end tests: run the compiled `cog` binary against a temp store and
//! assert on its JSON stdout and process exit code. This exercises the whole
//! composition root — CLI parsing, the per-command transaction, SQLite, and the
//! two error channels — exactly as a skill/agent would invoke it.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// A fresh, isolated store path per test (named, so failures point at the test).
fn store_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    p.push(format!("{name}.db"));
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(p.with_extension("db-wal"));
    let _ = std::fs::remove_file(p.with_extension("db-shm"));
    p
}

/// Run `cog --store <store> <args...>`; return (exit_code, parsed stdout JSON).
fn run(store: &PathBuf, args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_cog"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("failed to spawn cog");
    let code = output.status.code().expect("process killed by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf8");
    let json: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout:?}"));
    (code, json)
}

/// Run cog without a store; return (exit_code, stdout, stderr). For the usage
/// channel (help/errors are plain text, not JSON, and go to stderr).
fn run_raw(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cog"))
        .args(args)
        .output()
        .expect("failed to spawn cog");
    let code = output.status.code().expect("process killed by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr not utf8");
    (code, stdout, stderr)
}

// ---------- Help / usage channel ----------

#[test]
fn bare_invocation_is_usage_error_with_top_help() {
    let (code, stdout, stderr) = run_raw(&[]);
    assert_eq!(code, 64);
    assert!(stdout.is_empty(), "stdout must stay clean: {stdout:?}");
    assert!(stderr.contains("No command given."));
    assert!(stderr.contains("USAGE:"));
}

#[test]
fn help_flag_prints_top_help_on_stdout_exit_zero() {
    for flag in ["-h", "--help"] {
        let (code, stdout, _) = run_raw(&[flag]);
        assert_eq!(code, 0, "flag {flag}");
        assert!(stdout.contains("USAGE:"), "flag {flag}");
    }
}

#[test]
fn group_help_flag_shows_scoped_help() {
    let (code, stdout, _) = run_raw(&["log", "-h"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("cog log"));
    assert!(stdout.contains("log query"));
    assert!(!stdout.contains("fsm define"), "log help must be scoped");
}

#[test]
fn missing_subcommand_is_usage_error_with_group_help() {
    let (code, _, stderr) = run_raw(&["log"]);
    assert_eq!(code, 64);
    assert!(stderr.contains("needs a subcommand"));
    assert!(stderr.contains("cog log add"));
}

#[test]
fn missing_argument_is_usage_error_with_one_sentence_and_help() {
    let (code, _, stderr) = run_raw(&["log", "add", "stream-only"]);
    assert_eq!(code, 64);
    assert!(stderr.contains("Missing argument: json payload."));
    assert!(stderr.contains("cog log add"));
}

#[test]
fn unknown_command_is_usage_error() {
    let (code, _, stderr) = run_raw(&["bogus", "thing"]);
    assert_eq!(code, 64);
    assert!(stderr.contains("Unknown command 'bogus'."));
}

// ---------- Ledger ----------

#[test]
fn log_add_assigns_monotonic_seq() {
    let s = store_path("log_add_seq");
    let (code, v) = run(&s, &["log", "add", "events", "{\"a\":1}"]);
    assert_eq!(code, 0);
    assert_eq!(v["ok"], true);
    assert_eq!(v["value"]["seq"], 1);

    let (code, v) = run(&s, &["log", "add", "events", "{\"a\":2}"]);
    assert_eq!(code, 0);
    assert_eq!(v["value"]["seq"], 2);
}

#[test]
fn log_query_returns_most_recent_first() {
    let s = store_path("log_query_order");
    run(&s, &["log", "add", "events", "{\"a\":1}"]);
    run(&s, &["log", "add", "events", "{\"a\":2}"]);

    let (code, v) = run(&s, &["log", "query", "events"]);
    assert_eq!(code, 0);
    let entries = v["value"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["seq"], 2);
    assert_eq!(entries[1]["seq"], 1);
    // payload is round-tripped as JSON, not a string
    assert_eq!(entries[0]["payload"]["a"], 2);
}

#[test]
fn log_query_empty_stream_is_domain_error() {
    let s = store_path("log_query_empty");
    let (code, v) = run(&s, &["log", "query", "nope"]);
    assert_eq!(code, 2);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "domain");
    assert_eq!(v["error"]["code"], "empty_stream");
}

#[test]
fn log_add_non_json_is_technical_error() {
    let s = store_path("log_add_badjson");
    let (code, v) = run(&s, &["log", "add", "events", "not-json"]);
    assert_eq!(code, 70);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "technical");
}

// ---------- StateMachine ----------

const DEF: &str = r#"{
    "states": [
        {"name":"idle"},
        {"name":"polling","description":"waiting for events"},
        {"name":"processing"},
        {"name":"done","terminal":true}
    ],
    "transitions": [
        {"from":"idle","to":"polling","criterion":"watch started"},
        {"from":"polling","to":"processing"},
        {"from":"processing","to":"idle"},
        {"from":"idle","to":"done"}
    ],
    "initial": "idle"
}"#;

#[test]
fn fsm_define_starts_at_initial_state() {
    let s = store_path("fsm_define");
    let (code, v) = run(&s, &["fsm", "define", "w", DEF]);
    assert_eq!(code, 0);
    assert_eq!(v["value"]["current"], "idle");

    let (code, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(code, 0);
    assert_eq!(v["value"]["current"], "idle");
}

#[test]
fn fsm_legal_transition_advances_and_persists() {
    let s = store_path("fsm_legal");
    run(&s, &["fsm", "define", "w", DEF]);

    let (code, v) = run(&s, &["fsm", "transition", "w", "polling"]);
    assert_eq!(code, 0);
    assert_eq!(v["value"]["current"], "polling");

    // a separate process still sees the advanced state (survives compaction)
    let (_, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(v["value"]["current"], "polling");
}

#[test]
fn fsm_illegal_transition_is_domain_error() {
    let s = store_path("fsm_illegal");
    run(&s, &["fsm", "define", "w", DEF]);
    run(&s, &["fsm", "transition", "w", "polling"]);

    let (code, v) = run(&s, &["fsm", "transition", "w", "done"]);
    assert_eq!(code, 2);
    assert_eq!(v["error"]["kind"], "domain");
    assert_eq!(v["error"]["code"], "illegal_transition");
}

#[test]
fn fsm_unknown_target_is_domain_error() {
    let s = store_path("fsm_unknown");
    run(&s, &["fsm", "define", "w", DEF]);

    let (code, v) = run(&s, &["fsm", "transition", "w", "zzz"]);
    assert_eq!(code, 2);
    assert_eq!(v["error"]["code"], "unknown_state");
}

#[test]
fn fsm_state_of_undefined_machine_is_domain_error() {
    let s = store_path("fsm_undefined");
    let (code, v) = run(&s, &["fsm", "state", "ghost"]);
    assert_eq!(code, 2);
    assert_eq!(v["error"]["code"], "not_initialized");
}

#[test]
fn fsm_terminal_state_blocks_further_transitions() {
    let s = store_path("fsm_terminal");
    run(&s, &["fsm", "define", "w", DEF]);
    run(&s, &["fsm", "transition", "w", "done"]); // idle -> done (terminal)

    let (code, v) = run(&s, &["fsm", "transition", "w", "polling"]);
    assert_eq!(code, 2);
    assert_eq!(v["error"]["code"], "illegal_transition");
}

// ---------- StateMachine Context (poll cursor) ----------

#[test]
fn fsm_define_with_context_is_returned_by_state() {
    let s = store_path("fsm_ctx_define");
    let (code, _) = run(&s, &["fsm", "define", "w", DEF, "--context", "{\"ci_head\":\"abc\"}"]);
    assert_eq!(code, 0);

    let (code, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(code, 0);
    assert_eq!(v["value"]["current"], "idle");
    assert_eq!(v["value"]["context"]["ci_head"], "abc");
}

#[test]
fn fsm_transition_with_context_advances_state_and_replaces_blob() {
    let s = store_path("fsm_ctx_transition");
    run(&s, &["fsm", "define", "w", DEF, "--context", "{\"ci_head\":\"old\"}"]);

    let (code, v) = run(
        &s,
        &["fsm", "transition", "w", "polling", "--context", "{\"ci_head\":\"new\"}"],
    );
    assert_eq!(code, 0);
    assert_eq!(v["value"]["current"], "polling");

    // a separate process sees both the advanced state and the replaced blob
    let (_, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(v["value"]["current"], "polling");
    assert_eq!(v["value"]["context"]["ci_head"], "new");
}

#[test]
fn fsm_transition_without_context_preserves_the_blob() {
    let s = store_path("fsm_ctx_preserve");
    run(&s, &["fsm", "define", "w", DEF, "--context", "{\"ci_head\":\"keep\"}"]);

    let (code, _) = run(&s, &["fsm", "transition", "w", "polling"]);
    assert_eq!(code, 0);

    let (_, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(v["value"]["current"], "polling");
    assert_eq!(v["value"]["context"]["ci_head"], "keep");
}

#[test]
fn fsm_define_without_context_defaults_to_null() {
    let s = store_path("fsm_ctx_null");
    run(&s, &["fsm", "define", "w", DEF]);

    let (_, v) = run(&s, &["fsm", "state", "w"]);
    assert_eq!(v["value"]["context"], Value::Null);
}

#[test]
fn fsm_invalid_context_json_is_technical_error() {
    let s = store_path("fsm_ctx_badjson");
    let (code, v) = run(&s, &["fsm", "define", "w", DEF, "--context", "not-json"]);
    assert_eq!(code, 70);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "technical");
}
