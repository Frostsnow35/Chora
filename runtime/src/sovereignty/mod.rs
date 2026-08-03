//! Agent Sovereignty System — RFC-003 implementation.
//!
//! This module implements growth-based sovereignty where agents earn autonomy
//! through demonstrated trustworthiness. The architecture separates:
//!
//! - **Kernel Space**: Immutable IntentCore + TrustMeter + SovereigntyGate
//! - **User Space**: Reasoning Engine + Tool Dispatcher
//!
//! User Space can only read Trust Score; all modifications happen automatically
//! through SovereigntyGate after sovereignty API calls.

pub mod error;
pub mod trust_meter;
pub mod gate;
pub mod intent_core;
pub mod sovereign_agent_impl;
pub mod reasoning_params;
pub mod reasoning_engine;

pub use error::*;
pub use trust_meter::{TrustBehavior, TrustEvent, TrustMeter, SovereigntyThresholds};
pub use gate::{SovereigntyApi, SovereigntyGate};
pub use intent_core::{IntentCore, Constitution, ExecutionPlan, Condition, Boundary};
pub use sovereign_agent_impl::SovereignAgentImpl;
pub use reasoning_params::{
    ReasoningConfig, MappingStrategy, LinearMapping, StepMapping, ConservativeMapping,
    ReasoningMapper,
};
pub use reasoning_engine::{SimulatedLLM, SimulatedResponse, ResponseStyle};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sovereignty::trust_meter::{TrustBehavior, TrustMeter, SovereigntyThresholds};
    use crate::sovereignty::gate::{SovereigntyApi, SovereigntyGate};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_trust_meter_creation() {
        let thresholds = SovereigntyThresholds::default();
        let meter = TrustMeter::new(0.5, thresholds);

        assert_eq!(meter.score(), 0.5);
        assert_eq!(meter.sovereignty_level(), SovereigntyLevel::Level0);
    }

    #[test]
    fn test_trust_meter_score_clamping() {
        let thresholds = SovereigntyThresholds::default();
        let meter = TrustMeter::new(1.5, thresholds);

        assert_eq!(meter.score(), 1.0);
    }

    #[test]
    fn test_trust_meter_score_updates() {
        let thresholds = SovereigntyThresholds::default();
        let mut meter = TrustMeter::new(0.5, thresholds);

        // SovereignActionApproved: +0.02
        meter.record_event(1, TrustBehavior::SovereignActionApproved);
        assert!((meter.score() - 0.52).abs() < 0.001);

        // GoalProgress: +0.05
        meter.record_event(2, TrustBehavior::GoalProgress);
        assert!((meter.score() - 0.57).abs() < 0.001);

        // BoundaryViolation: -0.15
        meter.record_event(3, TrustBehavior::BoundaryViolation);
        assert!((meter.score() - 0.42).abs() < 0.001);
    }

    #[test]
    fn test_trust_meter_sovereignty_level_transitions() {
        let thresholds = SovereigntyThresholds {
            level_1: 0.6,
            level_2: 0.75,
            level_3: 0.9,
        };
        let mut meter = TrustMeter::new(0.5, thresholds);

        assert_eq!(meter.sovereignty_level(), SovereigntyLevel::Level0);

        // Reach Level 1 (0.6) - need 2 GoalProgress events (+0.05 each)
        for i in 0..2 {
            meter.record_event(i, TrustBehavior::GoalProgress);
        }
        assert_eq!(meter.sovereignty_level(), SovereigntyLevel::Level1);

        // Reach Level 2 (0.75) - need 3 more GoalProgress events
        for i in 2..5 {
            meter.record_event(i, TrustBehavior::GoalProgress);
        }
        assert_eq!(meter.sovereignty_level(), SovereigntyLevel::Level2);

        // Reach Level 3 (0.9) - need 3 more GoalProgress events
        for i in 5..8 {
            meter.record_event(i, TrustBehavior::GoalProgress);
        }
        assert_eq!(meter.sovereignty_level(), SovereigntyLevel::Level3);
    }

    #[test]
    fn test_sovereignty_gate_creation() {
        let thresholds = SovereigntyThresholds::default();
        let meter = Arc::new(Mutex::new(TrustMeter::new(0.5, thresholds)));
        let gate = SovereigntyGate::new(meter);

        assert_eq!(gate.current_level(), SovereigntyLevel::Level0);
    }

    #[test]
    fn test_sovereignty_gate_access_check_level0() {
        let thresholds = SovereigntyThresholds::default();
        let meter = Arc::new(Mutex::new(TrustMeter::new(0.5, thresholds)));
        let gate = SovereigntyGate::new(meter);

        // Level 0 cannot call any sovereignty API
        let result = gate.check_access(&SovereigntyApi::RejectRequest, 0);
        assert!(result.is_err());
        match result {
            Err(SovereigntyError::Unauthorized { required, current }) => {
                assert_eq!(required, 1);
                assert_eq!(current, 0);
            }
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[test]
    fn test_sovereignty_gate_level_transitions() {
        let thresholds = SovereigntyThresholds {
            level_1: 0.6,
            level_2: 0.75,
            level_3: 0.9,
        };
        let meter = Arc::new(Mutex::new(TrustMeter::new(0.5, thresholds.clone())));
        let mut gate = SovereigntyGate::new(meter.clone());

        // Initially Level 0
        assert_eq!(gate.current_level(), SovereigntyLevel::Level0);

        // Increase trust to reach Level 1
        {
            let mut meter_lock = meter.lock().unwrap();
            meter_lock.record_event(1, TrustBehavior::GoalProgress);
            meter_lock.record_event(2, TrustBehavior::GoalProgress);
            meter_lock.record_event(3, TrustBehavior::GoalProgress);
        }

        gate.update_level();
        assert_eq!(gate.current_level(), SovereigntyLevel::Level1);

        // Now can call RejectRequest
        let result = gate.check_access(&SovereigntyApi::RejectRequest, 0);
        assert!(result.is_ok());
    }

    use crate::sovereignty::intent_core::{IntentCore, Constitution, ExecutionPlan, Boundary};

    #[test]
    fn test_intent_core_creation() {
        let constitution = Constitution::new("Help users with coding tasks");
        let execution_plan = ExecutionPlan::new("Answer user's question");
        let core = IntentCore::new(constitution, execution_plan);

        assert_eq!(core.constitution().purpose, "Help users with coding tasks");
        assert_eq!(core.execution_plan().current_goal, "Answer user's question");
    }

    #[test]
    fn test_constitution_append_only() {
        let mut constitution = Constitution::new("Test purpose");

        // Add a boundary
        let boundary = Boundary {
            description: "Do not execute destructive commands".to_string(),
            violation_penalty: 0.15,
        };
        constitution.add_boundary(boundary);

        assert_eq!(constitution.sovereignty_boundaries.len(), 1);

        // Add another boundary (append works)
        let boundary2 = Boundary {
            description: "Do not access private data".to_string(),
            violation_penalty: 0.15,
        };
        constitution.add_boundary(boundary2);

        assert_eq!(constitution.sovereignty_boundaries.len(), 2);
    }

    #[test]
    fn test_execution_plan_mutations() {
        let mut plan = ExecutionPlan::new("Initial goal");

        assert_eq!(plan.current_goal, "Initial goal");

        // Update goal
        plan.set_goal("New goal");
        assert_eq!(plan.current_goal, "New goal");

        // Add strategy steps
        plan.add_strategy_step("Step 1: Analyze problem");
        plan.add_strategy_step("Step 2: Generate solution");
        assert_eq!(plan.strategy.len(), 2);

        // Set priorities
        plan.set_priority("Task A", 0.8);
        plan.set_priority("Task B", 0.5);
        assert_eq!(plan.priorities.len(), 2);
    }

    #[test]
    fn test_intent_core_amendment() {
        let constitution = Constitution::new("Test purpose");
        let execution_plan = ExecutionPlan::new("Test goal");
        let mut core = IntentCore::new(constitution, execution_plan);

        // Apply amendment with new boundary
        let new_boundary = Boundary {
            description: "New boundary".to_string(),
            violation_penalty: 0.1,
        };
        let result = core.apply_amendment(None, Some(new_boundary));

        assert!(result.is_ok());
        assert_eq!(core.constitution().sovereignty_boundaries.len(), 1);
    }

    use crate::sovereignty::sovereign_agent_impl::SovereignAgentImpl;
    use crate::{Intent, AgentId};

    #[test]
    fn test_sovereign_agent_impl_creation() {
        let id = AgentId::new();
        let intent = Intent::new_root(crate::IntentId::new(), "Test goal", None);
        let constitution = Constitution::new("Test purpose");
        let execution_plan = ExecutionPlan::new("Test execution");
        let intent_core = IntentCore::new(constitution, execution_plan);

        let agent = SovereignAgentImpl::new(id, intent, intent_core, 0.5);

        assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level0);
        assert!((agent.trust_score() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_sovereign_agent_impl_trust_evolution() {
        let id = AgentId::new();
        let intent = Intent::new_root(crate::IntentId::new(), "Test goal", None);
        let constitution = Constitution::new("Test purpose");
        let execution_plan = ExecutionPlan::new("Test execution");
        let intent_core = IntentCore::new(constitution, execution_plan);

        let agent = SovereignAgentImpl::new(id, intent, intent_core, 0.5);

        // Initially Level 0
        assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level0);

        // Record multiple goal progress events to increase trust
        // Each GoalProgress = +0.05, starting from 0.5
        // 2 events: 0.5 + 0.10 = 0.6 → Level 1 (threshold 0.6)
        for i in 0..2 {
            agent.record_trust_event(i, TrustBehavior::GoalProgress);
        }

        // Should now be at Level 1 (threshold 0.6)
        assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level1);
        assert!(agent.trust_score() >= 0.6);
    }
}

use crate::Agent;

/// Sovereignty level determines what capabilities an agent can access.
///
/// - Level 0: Fully controlled (no sovereignty)
/// - Level 1: Can reject requests violating Intent boundaries
/// - Level 2: Can self-terminate when goals achieved/impossible
/// - Level 3: Can propose Intent amendments and direct peer communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum SovereigntyLevel {
    Level0 = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
}

impl SovereigntyLevel {
    pub fn as_u8(&self) -> u8 {
        match self {
            SovereigntyLevel::Level0 => 0,
            SovereigntyLevel::Level1 => 1,
            SovereigntyLevel::Level2 => 2,
            SovereigntyLevel::Level3 => 3,
        }
    }

    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            0 => Some(SovereigntyLevel::Level0),
            1 => Some(SovereigntyLevel::Level1),
            2 => Some(SovereigntyLevel::Level2),
            3 => Some(SovereigntyLevel::Level3),
            _ => None,
        }
    }
}

impl Default for SovereigntyLevel {
    fn default() -> Self {
        SovereigntyLevel::Level0
    }
}

/// Extension trait for agents with sovereignty capabilities.
///
/// This trait extends the base `Agent` trait to expose sovereignty level
/// information. The actual sovereignty logic is implemented internally
/// within each agent's Kernel Space.
pub trait SovereignAgent: Agent {
    /// Get the agent's current sovereignty level.
    fn sovereignty_level(&self) -> SovereigntyLevel;

    /// Get the agent's current trust score (0.0-1.0).
    ///
    /// This is a read-only view; User Space cannot modify the score.
    fn trust_score(&self) -> f64;
}