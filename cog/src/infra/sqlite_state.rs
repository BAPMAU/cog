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
        let row: Option<(String, String, String)> = self
            .tx
            .query_row(
                "SELECT def, current, context FROM state_machine WHERE name = ?1",
                [name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((def_json, current, context_json)) => {
                let def: Definition = serde_json::from_str(&def_json)
                    .map_err(|e| TechnicalError::new(format!("corrupt definition: {e}")))?;
                let context: serde_json::Value = serde_json::from_str(&context_json)
                    .map_err(|e| TechnicalError::new(format!("corrupt context: {e}")))?;
                Ok(Some(StateMachine::rehydrate(def, current, context)))
            }
        }
    }

    fn save(&self, name: &str, machine: &StateMachine) -> Result<(), TechnicalError> {
        let def_json = serde_json::to_string(&machine.def)
            .map_err(|e| TechnicalError::new(format!("cannot serialize definition: {e}")))?;
        let context_json = serde_json::to_string(&machine.context)
            .map_err(|e| TechnicalError::new(format!("cannot serialize context: {e}")))?;
        self.tx.execute(
            "INSERT INTO state_machine (name, def, current, context) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
                 def = excluded.def, current = excluded.current, context = excluded.context",
            rusqlite::params![name, def_json, machine.current, context_json],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<(String, StateMachine)>, TechnicalError> {
        let mut stmt = self
            .tx
            .prepare("SELECT name, def, current, context FROM state_machine ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (name, def_json, current, context_json) = row?;
            let def: Definition = serde_json::from_str(&def_json)
                .map_err(|e| TechnicalError::new(format!("corrupt definition: {e}")))?;
            let context: serde_json::Value = serde_json::from_str(&context_json)
                .map_err(|e| TechnicalError::new(format!("corrupt context: {e}")))?;
            out.push((name, StateMachine::rehydrate(def, current, context)));
        }
        Ok(out)
    }
}
