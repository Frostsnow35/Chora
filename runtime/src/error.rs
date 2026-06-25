//! Error types for the runtime crate.

use thiserror::Error;

/// Errors related to agent operations.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("invalid agent state: {0}")]
    InvalidState(String),

    #[error("step execution failed: {0}")]
    StepFailed(String),

    #[error("agent already terminated: {0}")]
    AlreadyTerminated(String),
}

/// Errors related to memory operations.
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("memory tier not found: {0}")]
    TierNotFound(String),

    #[error("access denied to tier: {0}")]
    AccessDenied(String),

    #[error("memory full")]
    MemoryFull,

    #[error("other: {0}")]
    Other(String),
}
