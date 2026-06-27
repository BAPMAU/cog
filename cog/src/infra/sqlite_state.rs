//! `StateStore` adapter over a shared `&Transaction` (one transaction per command).
//!
//! The definition is serialized to JSON and stored alongside the current state,
//! so a machine fully rehydrates from one row.

use crate::application::ports::StateStore;
use crate::domain::state_machine::{Definition, StateMachine};
use crate::error::TechnicalError;
use rusqlite::{OptionalExtension, Transaction};

pub struct SqliteState<'tx> {
    pub tx: &'tx Transaction<'tx>,
}

impl<'tx> StateStore for SqliteState<'tx> {
    fn load(&self, name: &str) -> Result<Option<StateMachine>, TechnicalError> {
        let row: Option<(String, String)> = self
            .tx
            .query_row(
                "SELECT def, current FROM state_machine WHERE name = ?1",
                [name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((def_json, current)) => {
                let def: Definition = serde_json::from_str(&def_json)
                    .map_err(|e| TechnicalError::new(format!("corrupt definition: {e}")))?;
                Ok(Some(StateMachine::rehydrate(def, current)))
            }
        }
    }

    fn save(&self, name: &str, machine: &StateMachine) -> Result<(), TechnicalError> {
        let def_json = serde_json::to_string(&machine.def)
            .map_err(|e| TechnicalError::new(format!("cannot serialize definition: {e}")))?;
        self.tx.execute(
            "INSERT INTO state_machine (name, def, current) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET def = excluded.def, current = excluded.current",
            rusqlite::params![name, def_json, machine.current],
        )?;
        Ok(())
    }
}
