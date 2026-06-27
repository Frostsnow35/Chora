//! The core agent abstraction.
//!
//! This module defines the minimal interface through which `runtime` interacts with an agent,
//! along with the supporting types: Intent, AgentProgram, AgentRecord, and scheduling state.
//!
//! # Design Principles Applied
//!
//! - **P2 (Abstraction by Reduction)**: A single `Agent` trait regardless of underlying substrate.
//! - **P4 (End-to-End)**: The runtime is dumb; semantic work lives outside `runtime`.
//! - **P1 (Mechanism/Policy)**: Mechanisms exposed via trait; scheduling policy is pluggable.
//! - **T5 (Simplicity ↔ Expressiveness)**: Minimal sufficient set of operations.
//!
//! See `../../rfc/001-agent-process.md` for full design rationale.

mod error;
mod intent;
mod metrics;
mod mock_agent;
mod program;
mod record;
mod state;
mod tool;
pub mod sovereignty;
pub mod scheduling;
pub mod ipc;
pub mod fs;

pub use error::*;
pub use intent::{GoalDescription, Intent};
pub use mock_agent::{MockAgent, ScriptedAction};
pub use program::{AgentProgram, ModelDescriptor, PromptDescriptor, ToolDescriptor, ToolSet};
pub use record::{AgentRecord, SchedulingContext};
pub use state::{BlockReason, SchedulingState, TerminationReason};
pub use tool::{BuiltinToolExecutor, MemoryStore, MemoryTool, ToolExecutor, ToolOutcome, ToolResult};
pub use scheduling::{Scheduler, SchedulingEvent, FifoScheduler, Runtime};
pub use ipc::{ChannelId, IpcError, Channel, ChannelType, UnidirectionalChannel, BroadcastChannel, IpcBroker};
pub use sovereignty::{SovereigntyError, SovereigntyLevel, SovereignAgent};
pub use sovereignty::{SovereignAgentImpl, TrustBehavior};
pub use sovereignty::{
    ReasoningConfig, MappingStrategy, LinearMapping, StepMapping, ConservativeMapping,
    ReasoningMapper, SimulatedLLM, SimulatedResponse, ResponseStyle,
};

/// Unique identifier for an agent within a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AgentId(uuid::Uuid);

impl AgentId {
    /// Generate a new random agent ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Get the raw UUID bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Create from bytes (for serialization).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(uuid::Uuid::from_bytes(bytes))
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct IntentId(uuid::Uuid);

impl IntentId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(uuid::Uuid::from_bytes(bytes))
    }
}

impl Default for IntentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for IntentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Criteria for determining whether an intent's goal has been achieved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Criterion {
    /// A boolean expression that, when true, indicates success.
    Check(CheckCriterion),
    /// External validation from another agent or service.
    Validation(ValidationCriterion),
    /// All sub-criteria must be met.
    And(Vec<Criterion>),
    /// Any one criterion suffices.
    Or(Vec<Criterion>),
}

/// Internal check criteria (time-bound, step-count bound, resource consumption).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckCriterion {
    /// Maximum wall-clock duration allowed.
    pub duration_limit: Option<std::time::Duration>,
    /// Maximum steps allowed.
    pub step_limit: Option<u64>,
    /// Maximum token budget.
    pub token_limit: Option<TokenBudget>,
}

/// External validation criterion (requires validator agent).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationCriterion {
    /// Identifier of the validator agent (or pattern for matching validators).
    pub validator: String,
    /// Additional context to pass to the validator.
    pub context: serde_json::Value,
}

impl Criterion {
    /// Evaluate a CheckCriterion against current metrics.
    pub fn is_met(&self, metrics: &crate::metrics::AgentMetrics) -> CriteriaMet {
        match self {
            Criterion::Check(cc) => {
                let met = cc.duration_limit.map_or(true, |d| {
                    d.as_millis() as u64 >= metrics.steps_executed // placeholder evaluation
                }) && cc.step_limit.map_or(true, |n| n > metrics.steps_executed)
                    && cc.token_limit.as_ref().map_or(true, |t| t.tokens > metrics.tokens_consumed);
                if met {
                    CriteriaMet::Yes
                } else {
                    CriteriaMet::Maybe
                }
            }
            Criterion::Validation(_) => CriteriaMet::Unknown,
            Criterion::And(criteria) => {
                if criteria.iter().all(|c| c.is_met(metrics) == CriteriaMet::Yes) {
                    CriteriaMet::Yes
                } else if criteria.iter().any(|c| c.is_met(metrics) == CriteriaMet::No) {
                    CriteriaMet::No
                } else {
                    CriteriaMet::Maybe
                }
            }
            Criterion::Or(criteria) => {
                if criteria.iter().any(|c| c.is_met(metrics) == CriteriaMet::Yes) {
                    CriteriaMet::Yes
                } else if criteria.iter().all(|c| c.is_met(metrics) == CriteriaMet::No) {
                    CriteriaMet::No
                } else {
                    CriteriaMet::Maybe
                }
            }
        }
    }
}

/// Result of checking whether success criteria have been met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriteriaMet {
    Yes,
    No,
    Maybe,
    /// Cannot determine without external validation.
    Unknown,
}

/// Token budget constraint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TokenBudget {
    /// Total tokens allowed.
    pub tokens: u64,
}

impl TokenBudget {
    pub fn new(tokens: u64) -> Self {
        Self { tokens }
    }
}

/// Hard constraints within which an agent must operate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Constraints {
    /// Maximum tokens this intent may consume across all agents working on it.
    pub token_budget: Option<TokenBudget>,
    /// Wall-clock deadline.
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    /// Maximum number of steps.
    pub step_budget: Option<u64>,
    /// Capability bounds (what this intent is allowed to touch).
    pub capability_bounds: Vec<String>,
}

impl Default for Constraints {
    fn default() -> Self {
        Self {
            token_budget: None,
            deadline: None,
            step_budget: None,
            capability_bounds: vec![],
        }
    }
}

/// Metadata for tracking intent provenance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub creator_agent: Option<AgentId>,
    pub version: u64,
    pub labels: Vec<String>,
}

/// The minimal interface through which the runtime interacts with an agent.
///
/// Implementations may wrap any reasoning substrate (LLM, rule engine,
/// symbolic planner, human-in-the-loop). The runtime sees only this shape.
///
/// # Design Principles
///
/// - **P2 (Abstraction by Reduction)**: A single trait regardless of substrate.
/// - **P4 (End-to-End)**: The runtime is dumb; semantic work lives outside.
/// - **P1 (Mechanism/Policy)**: Mechanisms exposed via trait; policy is pluggable.
/// - **T5 (Simplicity ↔ Expressiveness)**: Minimal sufficient set of operations.
///
/// # Safety
///
/// This trait is safe. It does not require any unsafe operations or expose
/// unsafe methods.
pub trait Agent {
    /// Unique identity within the field.
    fn id(&self) -> AgentId;

    /// The agent's current intent — what it is trying to achieve and why.
    fn intent(&self) -> &Intent;

    /// Current scheduling state (Ready / Running / Blocked / Suspended / Terminated).
    ///
    /// Agents are responsible for reporting accurate state changes in their step implementation.
    fn state(&self) -> SchedulingState;

    /// Execute one step of the agent's reasoning, within the given context.
    ///
    /// This is the atomic unit of agent execution (cf. manifest §5 "what is
    /// the atomic unit?"). The step must be:
    ///   - self-contained: all inputs come from `ctx`
    ///   - auditable: must produce a StepRecord for the episodic log
    ///   - bounded: must terminate in finite time / tokens
    ///
    /// The agent returns a `StepResult` describing what happened and what to do next:
    /// - `status`: whether the step succeeded, failed, or was interrupted
    /// - `output`: what the step produced (text, tool call, message, etc.)
    /// - `wants_yield`: whether the agent voluntarily yields control
    /// - `metrics`: resource usage for this step (tokens, time)
    ///
    /// If the agent previously requested a tool call via `StepOutput::ToolCall`,
    /// the runtime will have executed it and placed the result in
    /// `ctx.pending_tool_result`. The agent should read and integrate this
    /// result on this call.
    ///
    /// # Errors
    ///
    /// Returns an error if the step cannot be completed due to external failures
    /// (e.g., tool call failure, memory access denied). The agent should enter
    /// Blocked state and report the error via its internal state if appropriate.
    fn step(&mut self, ctx: &mut StepContext) -> StepResult;

    /// Deliver an incoming message. The agent decides how (or whether) to integrate it.
    /// The runtime does not interpret the message.
    fn receive(&mut self, msg: Message) -> Result<(), DeliveryError>;

    /// Cooperatively yield. Called by the scheduler at clean boundaries.
    ///
    /// After yielding, the agent should call `resume()` when it wants to continue
    /// processing (either manually or automatically via the scheduler).
    fn yield_now(&mut self);

    /// Resume after a yield or block.
    fn resume(&mut self);

    /// Terminate with a stated reason. The agent may perform cleanup.
    ///
    /// After calling this, subsequent calls to `step()` should immediately return
    /// an error indicating the agent is terminated.
    fn terminate(&mut self, reason: TerminationReason);
}

/// Context passed to each agent step.
///
/// This is the runtime's mechanism for giving agents access to resources
/// without leaking internals (P5: capabilities, P1: mechanism vs policy).
///
/// # Design
///
/// The context is intentionally minimal and capability-based:
/// - **Working memory** is provided as a read-only view (`WorkingMemoryView`)
/// - **Capabilities** grant permission but do not bypass checks
/// - **Pending messages** are delivered all-at-once, not incrementally
/// - **Time** is provided for deadlines and timeouts
/// - **Token counter** enforces budgets atomically
/// - **Scheduling hint** allows the runtime to signal urgency
/// - **Pending tool result** enables end-to-end tool call flow (P4)
///
/// By keeping the context small and capability-based, the runtime avoids
/// becoming a bottleneck (P1) while ensuring agents cannot bypass checks.
#[derive(Debug)]
pub struct StepContext<'a> {
    /// The agent's working memory (read-only view during this step).
    pub working_memory: &'a dyn WorkingMemoryView,

    /// Capabilities for memory access. These grant permission but do not bypass checks.
    pub memory_capabilities: &'a [Capability],

    /// Pending incoming messages awaiting delivery.
    pub pending_messages: &'a [Message],

    /// Current time (for deadlines, timeouts).
    pub now: chrono::DateTime<chrono::Utc>,

    /// Current token counter (for enforcing budgets).
    pub token_counter: &'a mut TokenCounter,

    /// Scheduling hint from runtime.
    pub scheduling_hint: SchedulingHint,

    /// Result from a previously requested tool call, if any.
    ///
    /// The runtime populates this field after dispatching a `StepOutput::ToolCall`
    /// through a `ToolExecutor`. The agent reads it on its next `step()` and
    /// integrates the result into its working memory. The runtime does not
    /// interpret the result (P4).
    ///
    /// # Example
    ///
    /// ```rust
    /// // In agent.step():
    /// match ctx.pending_tool_result.take() {
    ///     Some(result) => {
    ///         match result.outcome {
    ///             ToolOutcome::Success(value) => {
    ///                 // Integrate `value` into working memory
    ///                 self.memory.append(format!(
    ///                     "Tool '{}' returned: {}",
    ///                     result.tool_name, value
    ///                 ));
    ///             }
    ///             ToolOutcome::Error { code, message } => {
    ///                 // Handle error appropriately
    ///                 self.handle_tool_error(&result.tool_name, &code, &message);
    ///             }
    ///         }
    ///     }
    ///     None => {
    ///         // No tool result pending — proceed with normal reasoning
    ///     }
    /// }
    /// ```
    pub pending_tool_result: Option<ToolResult>,
}

/// View over working memory provided by the runtime.
pub trait WorkingMemoryView: Send + Sync + std::fmt::Debug {
    /// Get the current context window content.
    fn peek(&self) -> &str;

    /// Check if we're near capacity (for pressure-aware decisions).
    fn utilization(&self) -> f32;
}

/// Capability granted by the runtime.
#[derive(Debug, Clone)]
pub struct Capability {
    /// Resource type (memory, tool, channel, etc.).
    pub resource_type: String,
    /// Resource identifier.
    pub resource_id: String,
    /// Access level (read, write, execute, etc.).
    pub access_level: String,
}

/// Token counting helper for enforcing budgets.
#[derive(Debug, Clone)]
pub struct TokenCounter(u64);

impl TokenCounter {
    pub fn new(initial: u64) -> Self {
        Self(initial)
    }

    pub fn consumed(&self) -> u64 {
        self.0
    }

    pub fn add(&mut self, amount: u64) {
        self.0 += amount;
    }

    pub fn remaining(&self, budget: Option<&TokenBudget>) -> u64 {
        let budget = budget.map(|b| b.tokens).unwrap_or(u64::MAX);
        budget.saturating_sub(self.0)
    }
}

/// Hint to the agent about scheduling context.
#[derive(Debug, Clone, Copy)]
pub enum SchedulingHint {
    /// Continue normally.
    Normal,
    /// Agent is behind schedule; consider accelerating.
    Urgent,
    /// Agent has low priority currently.
    LowPriority,
}

/// Step result produced by an agent's execution.
#[derive(Debug)]
pub struct StepResult {
    /// Success/failure status of the step.
    pub status: StepStatus,

    /// What this step produced (output, tool call request, state change).
    pub output: StepOutput,

    /// Whether the agent requests to yield voluntarily.
    pub wants_yield: bool,

    /// Metrics captured during this step (tokens, time).
    pub metrics: StepMetrics,
}

/// Status of a step execution.
#[derive(Debug, Clone)]
pub enum StepStatus {
    /// Step completed successfully.
    Success,
    /// Step failed due to an error (tool call failed, etc.).
    Failed { reason: String },
    /// Step was interrupted (timeout, budget exceeded).
    Interrupted { reason: InterruptReason },
}

/// Reason for interruption.
#[derive(Debug, Clone, Copy)]
pub enum InterruptReason {
    /// Step exceeded allocated tokens.
    TokenLimitExceeded,
    /// Step exceeded time limit.
    TimeLimitExceeded,
    /// Step exceeded step count for the intent.
    StepLimitExceeded,
    /// Agent was forcibly terminated by runtime.
    ForcefullyTerminated,
}

/// Output produced by a step.
#[derive(Debug)]
pub enum StepOutput {
    /// Plain text output from the agent.
    Text(String),
    /// Request to invoke a tool.
    ToolCall(ToolCallRequest),
    /// Request to communicate with another agent.
    Communication(Message),
    /// State transition request.
    StateChange(SchedulingState),
    /// Memory operation request (write, update, etc.).
    MemoryOperation(MemoryOp),
    /// Request to send a message via IPC channel.
    SendIpc {
        channel_id: ipc::ChannelId,
        message: Message,
    },
    /// Request to create an IPC channel.
    CreateChannel {
        channel_type: ipc::ChannelType,
        receivers: Vec<AgentId>,
        capacity: usize,
    },
}

/// Step metrics captured during execution.
#[derive(Debug, Clone)]
pub struct StepMetrics {
    pub tokens_used: u64,
    pub wall_time_ms: u64,
}

/// Request to invoke a tool.
#[derive(Debug)]
pub struct ToolCallRequest {
    /// Stable identifier for this call, used to match the eventual
    /// `ToolResult` back to the request.
    pub call_id: String,
    /// Name of the tool to invoke.
    pub tool_name: String,
    /// Arguments to pass to the tool, encoded as JSON.
    pub args: serde_json::Value,
}

/// Memory operation request.
#[derive(Debug)]
pub struct MemoryOp {
    pub tier: MemoryTier,
    pub operation: MemoryOperation,
    pub data: serde_json::Value,
}

/// Memory tier.
#[derive(Debug, Clone, Copy)]
pub enum MemoryTier {
    Working,
    Episodic,
    Semantic,
}

/// Memory operation type.
#[derive(Debug, Clone)]
pub enum MemoryOperation {
    Read,
    Write,
    Update,
    Delete,
}

/// Message between agents.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub sender: AgentId,
    pub receiver: AgentId,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageMetadata {
    pub sent_at: chrono::DateTime<chrono::Utc>,
    pub priority: u8,
    pub requires_ack: bool,
}

/// Error types for delivery failures.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DeliveryError {
    #[error("receiver not found")]
    ReceiverNotFound,

    #[error("message too large")]
    MessageTooLarge,

    #[error("capability denied: {0}")]
    PermissionDenied(String),

    #[error("other: {0}")]
    Other(String),
}
