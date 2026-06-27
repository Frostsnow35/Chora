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
pub use error::*;

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