//! `Ledger` entity: an append-only journal per stream.
//!
//! A "closed" entity: the invariant (append-only, ascending `seq` order) is
//! guaranteed *by the type*. An existing entry cannot be mutated or deleted;
//! the only operation is `append`, which produces a new sealed entry.

use crate::error::DomainError;

/// A sealed journal entry. Fields are read-only once created.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub seq: i64,
    pub at_millis: i64,
    pub payload: String,
}

/// View of a stream. Used to express read invariants.
pub struct Stream {
    name: String,
    entries: Vec<LogEntry>,
}

impl Stream {
    /// Rebuild a stream from the store (entries already sorted by `seq`).
    pub fn load(name: impl Into<String>, entries: Vec<LogEntry>) -> Self {
        Stream { name: name.into(), entries }
    }

    /// Entries, most recent first.
    pub fn most_recent_first(&self) -> Vec<LogEntry> {
        let mut out = self.entries.clone();
        out.sort_by(|a, b| b.seq.cmp(&a.seq));
        out
    }

    /// Read invariant: querying an empty stream is a *domain* error.
    pub fn require_non_empty(&self) -> Result<&Self, DomainError> {
        if self.entries.is_empty() {
            return Err(DomainError::EmptyStream { stream: self.name.clone() });
        }
        Ok(self)
    }
}
