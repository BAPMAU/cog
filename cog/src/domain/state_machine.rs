//! `StateMachine` entity: an "open" entity whose rules live in *data*, not in a
//! hard-coded Rust enum. A `Definition` lists the legal states, the allowed
//! transitions, and which states are terminal. The invariant is therefore checked
//! at *runtime* against that data — this is what makes one binary able to carry
//! arbitrary lifecycles (loop cadence, thread status, queue item state, ...).

use crate::error::DomainError;
use serde::{Deserialize, Serialize};

/// A state of the machine. Identity is its `name`; the rest is domain context.
/// `terminal` is intrinsic to a state, so it lives here rather than in a parallel list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub name: String,
    #[serde(default)]
    pub terminal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A directed edge `from -> to`, with optional context. `criterion` describes the
/// condition that gates the edge (free text for now); `description` documents it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criterion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The rules of a machine, as data. Validated once when the machine is defined.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub initial: String,
}

impl Definition {
    /// A definition is well-formed when every referenced state name is declared.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.require_known(&self.initial)?;
        for t in &self.transitions {
            self.require_known(&t.from)?;
            self.require_known(&t.to)?;
        }
        Ok(())
    }

    fn state(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }

    fn require_known(&self, name: &str) -> Result<(), DomainError> {
        if self.state(name).is_none() {
            return Err(DomainError::UnknownState { state: name.to_string() });
        }
        Ok(())
    }

    fn is_terminal(&self, name: &str) -> bool {
        self.state(name).map(|s| s.terminal).unwrap_or(false)
    }

    fn allows(&self, from: &str, to: &str) -> bool {
        self.transitions.iter().any(|t| t.from == from && t.to == to)
    }
}

/// A live machine: its rules, where it currently sits, and an opaque mutable
/// `context` blob. The context is consumer-owned JSON (e.g. a poll cursor); the
/// domain never inspects its shape — it only carries it, advancing it atomically
/// with each transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    pub def: Definition,
    pub current: String,
    pub context: serde_json::Value,
}

impl StateMachine {
    /// Create a machine sitting at its initial state, born with `context`.
    /// Validates the definition. A machine with no cursor is defined with
    /// `Value::Null` — there is no separate "uninitialized context" state.
    pub fn define(def: Definition, context: serde_json::Value) -> Result<Self, DomainError> {
        def.validate()?;
        let current = def.initial.clone();
        Ok(StateMachine { def, current, context })
    }

    /// Rebuild a persisted machine. The definition was validated when first defined.
    pub fn rehydrate(def: Definition, current: String, context: serde_json::Value) -> Self {
        StateMachine { def, current, context }
    }

    /// Move to `to`. Consumes `self` and returns the advanced machine, so an
    /// illegal transition cannot leave a half-mutated value behind.
    ///
    /// Runtime invariant — three domain errors:
    /// - `to` is not a declared state → `UnknownState`,
    /// - current state is terminal → `IllegalTransition`,
    /// - edge `(current, to)` is not allowed → `IllegalTransition`.
    pub fn transition(self, to: &str) -> Result<Self, DomainError> {
        self.def.require_known(to)?;
        if self.def.is_terminal(&self.current) {
            return Err(DomainError::IllegalTransition {
                from: self.current.clone(),
                to: to.to_string(),
            });
        }
        if !self.def.allows(&self.current, to) {
            return Err(DomainError::IllegalTransition {
                from: self.current.clone(),
                to: to.to_string(),
            });
        }
        Ok(StateMachine { current: to.to_string(), ..self })
    }
}
