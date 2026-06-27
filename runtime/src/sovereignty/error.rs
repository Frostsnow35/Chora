use thiserror::Error;

/// Errors from sovereignty system operations.
#[derive(Error, Debug, Clone)]
pub enum SovereigntyError {
    #[error("unauthorized sovereignty API call: required level {required}, current level {current}")]
    Unauthorized { required: u8, current: u8 },

    #[error("boundary violation: {0}")]
    BoundaryViolation(String),

    #[error("constitution is immutable")]
    ConstitutionImmutable,

    #[error("trust meter error: {0}")]
    TrustMeterError(String),
}

/// Errors from Intent Core operations.
#[derive(Error, Debug, Clone)]
pub enum ConstitutionError {
    #[error("cannot modify constitution: {0}")]
    Immutable(String),

    #[error("constitution validation failed: {0}")]
    ValidationFailed(String),
}

/// Errors from Trust Meter operations.
#[derive(Error, Debug, Clone)]
pub enum TrustError {
    #[error("invalid trust score: {0} (must be 0.0-1.0)")]
    InvalidScore(f64),

    #[error("history overflow: max {0} events")]
    HistoryOverflow(usize),
}