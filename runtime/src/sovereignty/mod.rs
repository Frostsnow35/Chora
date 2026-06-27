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

pub use error::*;
pub use trust_meter::{TrustBehavior, TrustEvent, TrustMeter, SovereigntyThresholds};
pub use gate::{SovereigntyApi, SovereigntyGate};

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
        let result = gate.check_access(&SovereigntyApi::RejectRequest);
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
        let result = gate.check_access(&SovereigntyApi::RejectRequest);
        assert!(result.is_ok());
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