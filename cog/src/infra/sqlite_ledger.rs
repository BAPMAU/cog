//! `LedgerStore` adapter over a shared `&Transaction` (one transaction per command).

use crate::application::ports::LedgerStore;
use crate::domain::ledger::{LogEntry, Stream, StreamSummary};
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

    fn stream_summaries(&self) -> Result<Vec<StreamSummary>, TechnicalError> {
        // One aggregate row per stream — the entries themselves stay in the table.
        let mut stmt = self.tx.prepare(
            "SELECT stream, COUNT(*), MAX(seq), MAX(at_millis)
             FROM log_entry GROUP BY stream ORDER BY stream",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(StreamSummary {
                name: r.get(0)?,
                count: r.get(1)?,
                last_seq: r.get(2)?,
                last_at: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}
