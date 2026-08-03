//! AgentRecord — the PCB-equivalent held by the process table.

use crate::{AgentId, Intent, AgentProgram};
use serde::{Deserialize, Serialize};

/// The complete picture of an agent held by the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    /// Identity.
    pub id: AgentId,

    /// Scheduling state (managed by runtime).
    pub state: StateSnapshot,

    /// The agent's intent (read-only from runtime perspective).
    pub intent: Intent,

    /// The three-layer program.
    pub program: AgentProgram,

    /// Memory handles (capabilities).
    pub memory_handles: Vec<String>,

    /// Channel endpoints.
    pub channel_endpoints: Vec<String>,

    /// Current capabilities.
    pub capabilities: Vec<String>,

    /// Scheduling context (opaque to runtime core).
    pub scheduling_context: SchedulingContext,

    /// Metrics for auditability.
    pub metrics: MetricsSnapshot,
}

impl AgentRecord {
    pub fn new(
        id: AgentId,
        intent: Intent,
        program: AgentProgram,
    ) -> Self {
        Self {
            id,
            state: StateSnapshot::from(crate::SchedulingState::Ready),
            intent,
            program,
            memory_handles: vec![],
            channel_endpoints: vec![],
            capabilities: vec![],
            scheduling_context: SchedulingContext::default(),
            metrics: MetricsSnapshot::new(id),
        }
    }

    /// Update state and record transition time.
    pub fn update_state(&mut self, new_state: StateSnapshot) {
        if self.state != new_state {
            tracing::debug!(
                agent_id = %self.id,
                from = ?self.state,
                to = ?new_state,
                "state transition"
            );
            self.state = new_state;
        }
    }

    /// Record a step completion.
    pub fn record_step(&mut self) {
        self.metrics.steps_executed += 1;
        self.metrics.last_stepped_at = Some(chrono::Utc::now());
    }

    /// Record tokens consumed in this step.
    pub fn record_tokens(&mut self, amount: u64) {
        self.metrics.tokens_consumed += amount;
    }

    /// Check if constraints are violated.
    pub fn check_constraints_violated(&self) -> bool {
        if let Some(budget) = &self.intent.constraints.token_budget {
            if self.metrics.tokens_consumed >= budget.tokens {
                return true;
            }
        }

        if let Some(limit) = self.intent.constraints.step_budget {
            if self.metrics.steps_executed >= limit {
                return true;
            }
        }

        if let Some(deadline) = &self.intent.constraints.deadline {
            if chrono::Utc::now() > *deadline {
                return true;
            }
        }

        false
    }
}

/// Snapshot of state at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub current: String, // "Ready", "Running", etc.
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub block_reason: Option<String>,
}

impl From<crate::SchedulingState> for StateSnapshot {
    fn from(state: crate::SchedulingState) -> Self {
        match state {
            crate::SchedulingState::Ready => StateSnapshot {
                current: "Ready".to_string(),
                updated_at: chrono::Utc::now(),
                block_reason: None,
            },
            crate::SchedulingState::Running => StateSnapshot {
                current: "Running".to_string(),
                updated_at: chrono::Utc::now(),
                block_reason: None,
            },
            crate::SchedulingState::Blocked { reason } => StateSnapshot {
                current: "Blocked".to_string(),
                updated_at: chrono::Utc::now(),
                block_reason: Some(reason.to_string()),
            },
            crate::SchedulingState::Suspended => StateSnapshot {
                current: "Suspended".to_string(),
                updated_at: chrono::Utc::now(),
                block_reason: None,
            },
            crate::SchedulingState::Terminated { reason, at } => StateSnapshot {
                current: "Terminated".to_string(),
                updated_at: at,
                block_reason: Some(format!("Terminated: {:?}", reason)),
            },
        }
    }
}

/// Opaque scheduling context for plugins.
///
/// This struct holds scheduling-related metadata that policies can use
/// to make decisions. The runtime core does not interpret these fields;
/// only the scheduler policy reads them.
///
/// # Fields
///
/// - `priority`: Higher number = higher priority. Policies may use this
///   to prefer certain agents.
/// - `quantum_used`: Accumulated execution quantum (for time-sliced policies).
/// - `last_run`: When the agent last executed a step.
/// - `deadline`: Optional deadline by which the agent must complete.
///   Deadline-aware policies prioritize agents closer to their deadline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulingContext {
    pub priority: i32,
    pub quantum_used: u64,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional deadline for the agent's current intent.
    /// `None` means no deadline constraint.
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
}

impl SchedulingContext {
    /// Create a new scheduling context with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the priority (higher = more important).
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the deadline.
    pub fn with_deadline(mut self, deadline: chrono::DateTime<chrono::Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Check if this agent has a deadline and whether it's urgent.
    ///
    /// Returns `Some(duration_remaining)` if the agent has a deadline,
    /// or `None` if no deadline is set.
    pub fn time_until_deadline(&self) -> Option<chrono::Duration> {
        self.deadline.map(|d| d - chrono::Utc::now())
    }

    /// Check if the deadline has passed.
    pub fn is_overdue(&self) -> bool {
        match self.deadline {
            Some(d) => chrono::Utc::now() > d,
            None => false,
        }
    }
}

/// Snapshot of metrics for auditability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub agent_id: AgentId,
    pub steps_executed: u64,
    pub tokens_consumed: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_stepped_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl MetricsSnapshot {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            steps_executed: 0,
            tokens_consumed: 0,
            messages_sent: 0,
            messages_received: 0,
            created_at: chrono::Utc::now(),
            last_stepped_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduling_context_default() {
        let ctx = SchedulingContext::default();
        assert_eq!(ctx.priority, 0);
        assert_eq!(ctx.quantum_used, 0);
        assert!(ctx.last_run.is_none());
        assert!(ctx.deadline.is_none());
    }

    #[test]
    fn test_scheduling_context_builder() {
        let deadline = chrono::Utc::now() + chrono::Duration::seconds(60);
        let ctx = SchedulingContext::new()
            .with_priority(100)
            .with_deadline(deadline);

        assert_eq!(ctx.priority, 100);
        assert!(ctx.deadline.is_some());
    }

    #[test]
    fn test_scheduling_context_no_deadline() {
        let ctx = SchedulingContext::new();
        assert!(ctx.time_until_deadline().is_none());
        assert!(!ctx.is_overdue());
    }

    #[test]
    fn test_scheduling_context_future_deadline() {
        let deadline = chrono::Utc::now() + chrono::Duration::hours(1);
        let ctx = SchedulingContext::new().with_deadline(deadline);

        let remaining = ctx.time_until_deadline().unwrap();
        assert!(remaining > chrono::Duration::zero());
        assert!(!ctx.is_overdue());
    }

    #[test]
    fn test_scheduling_context_past_deadline() {
        let deadline = chrono::Utc::now() - chrono::Duration::hours(1);
        let ctx = SchedulingContext::new().with_deadline(deadline);

        let remaining = ctx.time_until_deadline().unwrap();
        assert!(remaining < chrono::Duration::zero());
        assert!(ctx.is_overdue());
    }
}
