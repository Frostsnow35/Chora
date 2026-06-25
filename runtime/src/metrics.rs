//! Metrics module for agent auditability (T7).

use serde::{Deserialize, Serialize};

/// Agent metrics collected for auditability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub steps_executed: u64,
    pub tokens_consumed: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_stepped_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            steps_executed: 0,
            tokens_consumed: 0,
            messages_sent: 0,
            messages_received: 0,
            created_at: chrono::Utc::now(),
            last_stepped_at: None,
        }
    }
}
