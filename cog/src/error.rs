//! The two error channels of `cog`.
//!
//! - `DomainError`: an *expected* business error, non-retryable. Serialized as
//!   `kind:"domain"`, exit code 2. E.g. querying an empty stream, an illegal
//!   transition.
//! - `TechnicalError`: an infra failure, possibly retryable. `kind:"technical"`,
//!   exit code 70 (EX_SOFTWARE). E.g. SQLite unavailable, malformed JSON input.
//!
//! No business error goes through `panic!` — panic is reserved for bugs.

use std::fmt;

#[derive(Debug)]
pub enum DomainError {
    /// Querying a stream that never received any entry.
    EmptyStream { stream: String },
    /// A transition not permitted by the machine's definition (or from a terminal state).
    IllegalTransition { from: String, to: String },
    /// Referencing a state the definition does not declare.
    UnknownState { state: String },
    /// Operating on a machine that has not been defined yet.
    NotInitialized { name: String },
}

impl DomainError {
    /// Stable, machine-readable code exposed in the JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::EmptyStream { .. } => "empty_stream",
            DomainError::IllegalTransition { .. } => "illegal_transition",
            DomainError::UnknownState { .. } => "unknown_state",
            DomainError::NotInitialized { .. } => "not_initialized",
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::EmptyStream { stream } => {
                write!(f, "stream '{stream}' is empty")
            }
            DomainError::IllegalTransition { from, to } => {
                write!(f, "illegal transition from '{from}' to '{to}'")
            }
            DomainError::UnknownState { state } => {
                write!(f, "unknown state '{state}'")
            }
            DomainError::NotInitialized { name } => {
                write!(f, "machine '{name}' is not initialized")
            }
        }
    }
}

#[derive(Debug)]
pub struct TechnicalError {
    pub message: String,
}

impl TechnicalError {
    pub fn new(message: impl Into<String>) -> Self {
        TechnicalError { message: message.into() }
    }
}

impl fmt::Display for TechnicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<rusqlite::Error> for TechnicalError {
    fn from(e: rusqlite::Error) -> Self {
        TechnicalError::new(format!("sqlite: {e}"))
    }
}

/// A use case error: either domain or technical. This is the application
/// boundary — the CLI maps one to exit 2, the other to exit 70.
#[derive(Debug)]
pub enum AppError {
    Domain(DomainError),
    Technical(TechnicalError),
}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        AppError::Domain(e)
    }
}

impl From<TechnicalError> for AppError {
    fn from(e: TechnicalError) -> Self {
        AppError::Technical(e)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Technical(e.into())
    }
}
