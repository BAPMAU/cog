//! Use cases (inbound ports). The use case IS the API — no trait, the CLI calls it.
//!
//! Each use case orchestrates domain + port and returns `AppError` (domain or
//! technical channel depending on the cause).

use crate::application::ports::{LedgerStore, StateStore};
use crate::domain::ledger::{LogEntry, StreamSummary};
use crate::domain::state_machine::{Definition, StateMachine};
use crate::error::{AppError, DomainError};

/// Append an entry to a stream. The timestamp is supplied by the caller
/// (composition root) — the domain never reads the clock.
pub struct AddLogEntry<'a, S: LedgerStore> {
    pub store: &'a S,
    pub at_millis: i64,
}

impl<'a, S: LedgerStore> AddLogEntry<'a, S> {
    pub fn run(&self, stream: &str, payload: &str) -> Result<i64, AppError> {
        let seq = self.store.append(stream, self.at_millis, payload)?;
        Ok(seq)
    }
}

/// Read a stream, most recent first. *Domain* error if the stream is empty.
pub struct QueryLog<'a, S: LedgerStore> {
    pub store: &'a S,
}

impl<'a, S: LedgerStore> QueryLog<'a, S> {
    pub fn run(&self, stream: &str) -> Result<Vec<LogEntry>, AppError> {
        let s = self.store.load_stream(stream)?;
        s.require_non_empty()?;
        Ok(s.most_recent_first())
    }
}

/// Define (or redefine) a machine from its rules; it starts at the initial state.
/// Domain error if the definition references unknown states.
pub struct DefineMachine<'a, S: StateStore> {
    pub store: &'a S,
}

impl<'a, S: StateStore> DefineMachine<'a, S> {
    pub fn run(
        &self,
        name: &str,
        def: Definition,
        context: serde_json::Value,
    ) -> Result<String, AppError> {
        let machine = StateMachine::define(def, context)?;
        self.store.save(name, &machine)?;
        Ok(machine.current)
    }
}

/// The read-modify-write use case: load the machine, apply a domain transition,
/// save it back. Load/save are technical; the transition rule is domain.
/// The whole thing is atomic thanks to the enclosing per-command transaction.
pub struct Transition<'a, S: StateStore> {
    pub store: &'a S,
}

impl<'a, S: StateStore> Transition<'a, S> {
    /// `context` is the whole-blob replacement: `Some` overwrites the cursor in the
    /// same transaction as the phase move (atomic); `None` leaves it untouched.
    pub fn run(
        &self,
        name: &str,
        to: &str,
        context: Option<serde_json::Value>,
    ) -> Result<String, AppError> {
        let machine = self
            .store
            .load(name)?
            .ok_or_else(|| DomainError::NotInitialized {
                name: name.to_string(),
            })?;
        let advanced = machine.transition(to)?.with_context(context);
        self.store.save(name, &advanced)?;
        Ok(advanced.current)
    }
}

/// Read the current state of a machine. Domain error if it was never defined.
pub struct GetState<'a, S: StateStore> {
    pub store: &'a S,
}

impl<'a, S: StateStore> GetState<'a, S> {
    /// Returns `(current state, context blob)` — the consumer reads its cursor here.
    pub fn run(&self, name: &str) -> Result<(String, serde_json::Value), AppError> {
        let machine = self
            .store
            .load(name)?
            .ok_or_else(|| DomainError::NotInitialized {
                name: name.to_string(),
            })?;
        Ok((machine.current, machine.context))
    }
}

/// A machine seen in an overview: just enough to scan the store at a glance.
pub struct MachineSummary {
    pub name: String,
    pub current: String,
    pub terminal: bool,
    pub has_context: bool,
}

/// The whole-store overview: every stream and every machine, summarized.
pub struct Overview {
    pub machines: Vec<MachineSummary>,
    pub streams: Vec<StreamSummary>,
}

/// Read-only overview of everything in the store. With `name_filter`, keeps only
/// streams and machines whose name contains that substring. Touches both stores
/// but neither entity's invariants — it never loads stream entries.
pub struct Inspect<'a, L: LedgerStore, S: StateStore> {
    pub ledger: &'a L,
    pub state: &'a S,
}

impl<'a, L: LedgerStore, S: StateStore> Inspect<'a, L, S> {
    pub fn run(&self, name_filter: Option<&str>) -> Result<Overview, AppError> {
        let matches = |n: &str| name_filter.is_none_or(|f| n.contains(f));

        let streams = self
            .ledger
            .stream_summaries()?
            .into_iter()
            .filter(|s| matches(&s.name))
            .collect();

        let machines = self
            .state
            .list()?
            .into_iter()
            .filter(|(name, _)| matches(name))
            .map(|(name, m)| MachineSummary {
                name,
                terminal: m.is_terminal(),
                has_context: !m.context.is_null(),
                current: m.current,
            })
            .collect();

        Ok(Overview { machines, streams })
    }
}
