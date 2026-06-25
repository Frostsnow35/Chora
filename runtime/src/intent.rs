//! Intent descriptor for agents.

use crate::{AgentId, CriteriaMet, Metadata};
use serde::{Deserialize, Serialize};

/// Human- and machine-readable description of the goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GoalDescription(pub String);

impl GoalDescription {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for GoalDescription {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Intent — the agent's "why am I running"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Unique identity for this intent.
    pub id: super::IntentId,

    /// The goal description.
    pub goal: GoalDescription,

    /// Success criteria.
    pub success_criteria: Vec<super::Criterion>,

    /// Constraints (token budget, deadline, etc.).
    pub constraints: super::Constraints,

    /// Parent intent ID if delegated.
    pub parent: Option<super::IntentId>,

    /// Metadata (created_at, creator_agent, etc.).
    pub metadata: Metadata,
}

impl Intent {
    /// Create a new root intent.
    pub fn new_root(
        id: super::IntentId,
        goal: impl Into<String>,
        creator_agent: Option<AgentId>,
    ) -> Self {
        Self {
            id,
            goal: GoalDescription(goal.into()),
            success_criteria: vec![],
            constraints: super::Constraints::default(),
            parent: None,
            metadata: Metadata {
                created_at: chrono::Utc::now(),
                creator_agent,
                version: 1,
                labels: vec![],
            },
        }
    }

    /// Check if any success criteria have been met.
    pub fn check_success_criteria(&self, metrics: &crate::metrics::AgentMetrics) -> CriteriaMet {
        self.success_criteria
            .iter()
            .fold(CriteriaMet::Maybe, |acc, c| match c.is_met(metrics) {
                CriteriaMet::Yes => CriteriaMet::Yes,
                CriteriaMet::No => {
                    if matches!(acc, CriteriaMet::Yes) {
                        CriteriaMet::Yes
                    } else {
                        acc
                    }
                }
                CriteriaMet::Maybe => {
                    if matches!(acc, CriteriaMet::Yes) {
                        CriteriaMet::Yes
                    } else {
                        CriteriaMet::Maybe
                    }
                }
                CriteriaMet::Unknown => CriteriaMet::Unknown,
            })
    }

    /// Check if constraints have been violated.
    pub fn check_constraints(&self, metrics: &crate::metrics::AgentMetrics) -> bool {
        // Token budget check
        if let Some(budget) = &self.constraints.token_budget {
            if metrics.tokens_consumed >= budget.tokens {
                return true;
            }
        }

        // Step budget check
        if let Some(limit) = self.constraints.step_budget {
            if metrics.steps_executed >= limit {
                return true;
            }
        }

        // Deadline check
        if let Some(deadline) = self.constraints.deadline {
            if chrono::Utc::now() > deadline {
                return true;
            }
        }

        false
    }
}
