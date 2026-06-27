//! Connection opening and schema migration.
//!
//! WAL + busy_timeout to tolerate several concurrent `cog` processes.
//! The per-command transaction is opened by the composition root, not here.

use crate::error::TechnicalError;
use rusqlite::Connection;

pub fn open(path: &str) -> Result<Connection, TechnicalError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), TechnicalError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS log_entry (
            stream    TEXT NOT NULL,
            seq       INTEGER NOT NULL,
            at_millis INTEGER NOT NULL,
            payload   TEXT NOT NULL,
            PRIMARY KEY (stream, seq)
        );
        CREATE TABLE IF NOT EXISTS state_machine (
            name    TEXT PRIMARY KEY,
            def     TEXT NOT NULL,
            current TEXT NOT NULL
        );",
    )?;
    Ok(())
}
