//! Intent Core — two-layer Intent structure with immutable Constitution.
//!
//! This module implements the two-layer Intent design from RFC-003:
//! - **Constitution**: Immutable core defining the agent's fundamental identity
//! - **Execution Plan**: Mutable layer defining current goals and strategy
//!
//! The Constitution can only be appended to (via amendments), never modified or deleted.

use serde::{Deserialize, Serialize};
use crate::sovereignty::ConstitutionError;

/// A termination condition that must be checked periodically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Human-readable description of the condition.
    pub description: String,
    /// Whether this condition is currently met (evaluated externally).
    pub is_met: bool,
}

/// A sovereignty boundary that the agent must respect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    /// Human-readable description of the boundary.
    pub description: String,
    /// Trust score penalty for violating this boundary.
    pub violation_penalty: f64,
}

/// The immutable Constitution defining the agent's fundamental identity.
///
/// The Constitution is append-only: amendments can add new boundaries or conditions,
/// but cannot remove or modify existing ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constitution {
    /// The agent's fundamental purpose ("why I exist").
    pub purpose: String,
    /// Termination conditions (when I must terminate).
    pub termination_conditions: Vec<Condition>,
    /// Sovereignty boundaries (what I cannot do).
    pub sovereignty_boundaries: Vec<Boundary>,
}

impl Constitution {
    /// Create a new Constitution.
    pub fn new(purpose: impl Into<String>) -> Self {
        Self {
            purpose: purpose.into(),
            termination_conditions: Vec::new(),
            sovereignty_boundaries: Vec::new(),
        }
    }

    /// Add a new termination condition (append-only).
    pub fn add_termination_condition(&mut self, condition: Condition) {
        self.termination_conditions.push(condition);
    }

    /// Add a new sovereignty boundary (append-only).
    pub fn add_boundary(&mut self, boundary: Boundary) {
        self.sovereignty_boundaries.push(boundary);
    }

    /// Check if an action violates any boundary.
    pub fn check_boundary_violation(&self, action_description: &str) -> Option<&Boundary> {
        // Simple implementation: check if action description contains any boundary keyword
        // In practice, this would use more sophisticated matching
        for boundary in &self.sovereignty_boundaries {
            if action_description.contains(&boundary.description) {
                return Some(boundary);
            }
        }
        None
    }
}

/// The mutable Execution Plan defining current goals and strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Current concrete goal.
    pub current_goal: String,
    /// Current strategy (sequence of actions).
    pub strategy: Vec<String>,
    /// Priority ordering (goal -> priority score).
    pub priorities: Vec<(String, f64)>,
}

impl ExecutionPlan {
    /// Create a new Execution Plan.
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            current_goal: goal.into(),
            strategy: Vec::new(),
            priorities: Vec::new(),
        }
    }

    /// Update the current goal.
    pub fn set_goal(&mut self, goal: impl Into<String>) {
        self.current_goal = goal.into();
    }

    /// Add a strategy step.
    pub fn add_strategy_step(&mut self, step: impl Into<String>) {
        self.strategy.push(step.into());
    }

    /// Set priority for a goal.
    pub fn set_priority(&mut self, goal: impl Into<String>, priority: f64) {
        let goal_str = goal.into();
        // Remove existing entry for this goal if present
        self.priorities.retain(|(g, _)| g != &goal_str);
        self.priorities.push((goal_str, priority));
    }
}

/// Intent Core — combines immutable Constitution with mutable Execution Plan.
#[derive(Debug, Clone)]
pub struct IntentCore {
    /// Immutable Constitution (append-only).
    constitution: Constitution,
    /// Mutable Execution Plan.
    execution_plan: ExecutionPlan,
}

impl IntentCore {
    /// Create a new IntentCore.
    pub fn new(constitution: Constitution, execution_plan: ExecutionPlan) -> Self {
        Self {
            constitution,
            execution_plan,
        }
    }

    /// Get read-only reference to Constitution.
    pub fn constitution(&self) -> &Constitution {
        &self.constitution
    }

    /// Get mutable reference to Execution Plan (for User Space updates).
    pub fn execution_plan_mut(&mut self) -> &mut ExecutionPlan {
        &mut self.execution_plan
    }

    /// Get read-only reference to Execution Plan.
    pub fn execution_plan(&self) -> &ExecutionPlan {
        &self.execution_plan
    }

    /// Apply an amendment to the Constitution (append-only).
    pub fn apply_amendment(
        &mut self,
        new_condition: Option<Condition>,
        new_boundary: Option<Boundary>,
    ) -> Result<(), ConstitutionError> {
        if let Some(condition) = new_condition {
            self.constitution.add_termination_condition(condition);
        }
        if let Some(boundary) = new_boundary {
            self.constitution.add_boundary(boundary);
        }
        Ok(())
    }
}