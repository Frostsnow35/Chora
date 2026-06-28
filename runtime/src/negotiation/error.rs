//! Negotiation protocol error types.

use crate::AgentId;
use thiserror::Error;

/// Errors from negotiation protocol operations.
#[derive(Error, Debug, Clone)]
pub enum NegotiationError {
    #[error("agent {0} is not level 3, cannot propose amendments")]
    InsufficientLevel(AgentId),

    #[error("negotiation session {0} not found")]
    SessionNotFound(String),

    #[error("agent {0} is not a participant in this session")]
    NotAParticipant(AgentId),

    #[error("invalid phase transition: {0}")]
    InvalidPhaseTransition(String),

    #[error("amendment targets constitution: immutable")]
    ConstitutionImmutable,

    #[error("proposal {0} not found")]
    ProposalNotFound(String),

    #[error("agent {0} already submitted a response")]
    DuplicateResponse(AgentId),

    #[error("agent {0} already submitted a vote")]
    DuplicateVote(AgentId),

    #[error("negotiation session has ended")]
    SessionEnded,

    #[error("session {0} timed out after {1:?}")]
    SessionTimeout(String, std::time::Duration),

    #[error("backpressure: too many active sessions ({0} >= {1})")]
    Backpressure(usize, usize),

    #[error("rate limited: agent {0} exceeded session creation rate ({1}/s)")]
    RateLimited(AgentId, u32),
}
