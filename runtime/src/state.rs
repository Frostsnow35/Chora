//! Scheduling-related types.

use serde::{Deserialize, Serialize};

/// The agent's scheduling state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingState {
    /// Agent is ready to run but not currently scheduled.
    Ready,

    /// Agent is currently executing.
    Running,

    /// Agent is blocked waiting for something.
    Blocked { reason: BlockReason },

    /// Agent has been voluntarily suspended.
    Suspended,

    /// Agent has terminated.
    Terminated {
        reason: TerminationReason,
        at: chrono::DateTime<chrono::Utc>,
    },
}

/// Reason an agent is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockReason {
    /// Waiting for a message from another agent.
    WaitForMessage,

    /// Waiting for a tool call result.
    WaitForToolResult,

    /// Waiting for user input.
    WaitForUserInput,

    /// Waiting for memory retrieval.
    WaitForMemory,

    /// Waiting for external event (timer, etc.).
    WaitForEvent,

    /// Other blocking reason.
    Other(String),
}

impl std::fmt::Display for BlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockReason::WaitForMessage => write!(f, "wait_for_message"),
            BlockReason::WaitForToolResult => write!(f, "wait_for_tool_result"),
            BlockReason::WaitForUserInput => write!(f, "wait_for_user_input"),
            BlockReason::WaitForMemory => write!(f, "wait_for_memory"),
            BlockReason::WaitForEvent => write!(f, "wait_for_event"),
            BlockReason::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Reason for agent termination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationReason {
    /// Agent completed its goal successfully.
    Success,

    /// Agent failed to complete its goal.
    Failure,

    /// Constraint was violated (token budget, deadline, etc.).
    ConstraintViolation,

    /// Runtime forcibly terminated the agent.
    ForcefullyTerminated,

    /// Parent intent was adopted by another agent.
    IntentAdopted,

    /// Custom termination reason provided by supervisor.
    Custom(String),
}

impl std::fmt::Display for TerminationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminationReason::Success => write!(f, "success"),
            TerminationReason::Failure => write!(f, "failure"),
            TerminationReason::ConstraintViolation => write!(f, "constraint_violation"),
            TerminationReason::ForcefullyTerminated => write!(f, "forcefully_terminated"),
            TerminationReason::IntentAdopted => write!(f, "intent_adopted"),
            TerminationReason::Custom(s) => write!(f, "{}", s),
        }
    }
}
