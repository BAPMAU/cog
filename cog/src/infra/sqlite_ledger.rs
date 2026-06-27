//! `LedgerStore` adapter over a shared `&Transaction` (one transaction per command).

use crate::application::ports::LedgerStore;
use crate::domain::ledger::{LogEntry, Stream};
use crate::error::TechnicalError;
use rusqlite::Transaction;

pub struct SqliteLedger<'tx> {
    pub tx: &'tx Transaction<'tx>,
}

impl<'tx> LedgerStore for SqliteLedger<'tx> {
    fn append(&self, stream: &str, at_millis: i64, payload: &str) -> Result<i64, TechnicalError> {
        // seq = current max + 1, atomic within the enclosing transaction.
        let next: i64 = self.tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM log_entry WHERE stream = ?1",
            [stream],
            |r| r.get(0),
        )?;
        self.tx.execute(
            "INSERT INTO log_entry (stream, seq, at_millis, payload) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![stream, next, at_millis, payload],
        )?;
        Ok(next)
    }

    fn load_stream(&self, stream: &str) -> Result<Stream, TechnicalError> {
        let mut stmt = self.tx.prepare(
            "SELECT seq, at_millis, payload FROM log_entry WHERE stream = ?1 ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map([stream], |r| {
            Ok(LogEntry {
                seq: r.get(0)?,
                at_millis: r.get(1)?,
                payload: r.get(2)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(Stream::load(stream, entries))
    }
}
