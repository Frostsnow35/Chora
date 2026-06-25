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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulingContext {
    pub priority: i32,
    pub quantum_used: u64,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
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
