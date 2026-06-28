//! Outbound ports: one trait per entity, in the domain's vocabulary.
//!
//! Ports only know `TechnicalError`: a persistence failure is always technical.
//! Domain errors are born in the domain, not here.

use crate::domain::ledger::{Stream, StreamSummary};
use crate::domain::state_machine::StateMachine;
use crate::error::TechnicalError;

pub trait LedgerStore {
    /// Append an entry to the stream and return the assigned `seq` (monotonic per stream).
    fn append(&self, stream: &str, at_millis: i64, payload: &str) -> Result<i64, TechnicalError>;

    /// Load every entry of a stream (empty Stream if the stream is unknown).
    fn load_stream(&self, stream: &str) -> Result<Stream, TechnicalError>;

    /// One summary per stream, for an overview — entries are not loaded.
    fn stream_summaries(&self) -> Result<Vec<StreamSummary>, TechnicalError>;
}

pub trait StateStore {
    /// Load a machine by name (`None` if never defined). Persists def + current together.
    fn load(&self, name: &str) -> Result<Option<StateMachine>, TechnicalError>;

    /// Upsert the machine's definition and current state.
    fn save(&self, name: &str, machine: &StateMachine) -> Result<(), TechnicalError>;

    /// Every machine paired with its name, for an overview.
    fn list(&self) -> Result<Vec<(String, StateMachine)>, TechnicalError>;
}
