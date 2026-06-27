//! Composition root.
//!
//! Responsibilities that live here and nowhere else:
//! - read the clock (`now_millis`) and inject the timestamp into use cases,
//! - open one transaction per command (RAII rollback on any early return),
//! - build the right adapter, dispatch to the use case,
//! - map the two error channels to JSON shape + process exit code.

mod application;
mod domain;
mod error;
mod infra;

use application::usecases::{AddLogEntry, DefineMachine, GetState, QueryLog, Transition};
use domain::state_machine::Definition;
use error::AppError;
use infra::cli::{self, Command};
use infra::sqlite_ledger::SqliteLedger;
use infra::sqlite_state::SqliteState;
use serde_json::json;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const EXIT_DOMAIN: u8 = 2;
const EXIT_USAGE: u8 = 64;
const EXIT_TECHNICAL: u8 = 70;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Usage channel: help and malformed invocations go to stderr, stdout stays JSON.
    let inv = match cli::parse(&args) {
        Ok(inv) => inv,
        Err(cli::Usage::Help(help)) => {
            println!("{help}");
            return ExitCode::SUCCESS;
        }
        Err(cli::Usage::Error { sentence, help }) => {
            eprintln!("{sentence}\n\n{help}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match run(inv) {
        Ok(value) => {
            print_ok(value);
            ExitCode::SUCCESS
        }
        Err(AppError::Domain(e)) => {
            print_err("domain", e.code(), &e.to_string());
            ExitCode::from(EXIT_DOMAIN)
        }
        Err(AppError::Technical(e)) => {
            print_err("technical", "technical", &e.to_string());
            ExitCode::from(EXIT_TECHNICAL)
        }
    }
}

fn run(inv: cli::Invocation) -> Result<serde_json::Value, AppError> {
    ensure_parent_dir(&inv.store)?;
    let mut conn = infra::db::open(&inv.store)?;
    // One transaction per command. IMMEDIATE so writers serialize cleanly.
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(error::TechnicalError::from)?;

    // Each command builds the adapter it needs, all sharing the same `&tx`.
    let value = match inv.command {
        Command::LogAdd { stream, payload } => {
            validate_json(&payload)?;
            let store = SqliteLedger { tx: &tx };
            let uc = AddLogEntry { store: &store, at_millis: now_millis() };
            let seq = uc.run(&stream, &payload)?;
            json!({ "seq": seq })
        }
        Command::LogQuery { stream } => {
            let store = SqliteLedger { tx: &tx };
            let uc = QueryLog { store: &store };
            let entries = uc.run(&stream)?;
            let items: Vec<_> = entries
                .iter()
                .map(|e| {
                    json!({
                        "seq": e.seq,
                        "at": e.at_millis.to_string(),
                        "payload": serde_json::from_str::<serde_json::Value>(&e.payload)
                            .unwrap_or(serde_json::Value::String(e.payload.clone())),
                    })
                })
                .collect();
            json!({ "entries": items })
        }
        Command::FsmDefine { name, def_json } => {
            let def: Definition = serde_json::from_str(&def_json)
                .map_err(|e| error::TechnicalError::new(format!("invalid definition JSON: {e}")))?;
            let store = SqliteState { tx: &tx };
            let uc = DefineMachine { store: &store };
            let current = uc.run(&name, def)?;
            json!({ "name": name, "current": current })
        }
        Command::FsmTransition { name, to } => {
            let store = SqliteState { tx: &tx };
            let uc = Transition { store: &store };
            let current = uc.run(&name, &to)?;
            json!({ "name": name, "current": current })
        }
        Command::FsmState { name } => {
            let store = SqliteState { tx: &tx };
            let uc = GetState { store: &store };
            let current = uc.run(&name)?;
            json!({ "name": name, "current": current })
        }
    };

    tx.commit().map_err(error::TechnicalError::from)?;
    Ok(value)
}

/// A payload that is not valid JSON is a *technical* error (bad input to the tool),
/// not a domain rule violation.
fn validate_json(payload: &str) -> Result<(), AppError> {
    serde_json::from_str::<serde_json::Value>(payload)
        .map_err(|e| error::TechnicalError::new(format!("payload is not valid JSON: {e}")))?;
    Ok(())
}

fn ensure_parent_dir(store: &str) -> Result<(), AppError> {
    if let Some(parent) = std::path::Path::new(store).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| error::TechnicalError::new(format!("cannot create store dir: {e}")))?;
        }
    }
    Ok(())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn print_ok(value: serde_json::Value) {
    println!("{}", json!({ "ok": true, "value": value }));
}

fn print_err(kind: &str, code: &str, message: &str) {
    println!(
        "{}",
        json!({ "ok": false, "error": { "kind": kind, "code": code, "message": message } })
    );
}
