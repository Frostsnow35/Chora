//! Sovereignty Gate — API router enforcing sovereignty level constraints.
//!
//! The Sovereignty Gate lives in Kernel Space and routes sovereignty API calls
//! based on the agent's current trust score. It acts like an OS syscall gate:
//! User Space issues a syscall, Gate checks permissions, and either allows or
//! denies the operation.

use std::sync::Arc;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use super::trust_meter::TrustMeter;
use super::trust_meter::TrustBehavior;
use super::error::SovereigntyError;
use super::SovereigntyLevel;

/// Sovereignty APIs that require different access levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SovereigntyApi {
    /// Level 1: Reject requests violating Intent boundaries.
    RejectRequest,
    /// Level 2: Self-terminate when goals achieved/impossible.
    SelfTerminate,
    /// Level 3: Propose Intent amendment to supervisor or peers.
    ProposeAmendment,
    /// Level 3: Establish direct peer communication channel.
    DirectPeerCommunication,
}

/// Sovereignty Gate — enforces sovereignty level constraints.
#[derive(Debug)]
pub struct SovereigntyGate {
    /// Current sovereignty level (updated based on trust score).
    current_level: SovereigntyLevel,
    /// Shared reference to TrustMeter (for reading score).
    trust_meter: Arc<Mutex<TrustMeter>>,
    /// API registry: maps each API to minimum required level.
    api_registry: std::collections::HashMap<SovereigntyApi, u8>,
}

impl SovereigntyGate {
    /// Create a new SovereigntyGate with initial trust score.
    pub fn new(trust_meter: Arc<Mutex<TrustMeter>>) -> Self {
        let mut api_registry = std::collections::HashMap::new();
        api_registry.insert(SovereigntyApi::RejectRequest, 1);
        api_registry.insert(SovereigntyApi::SelfTerminate, 2);
        api_registry.insert(SovereigntyApi::ProposeAmendment, 3);
        api_registry.insert(SovereigntyApi::DirectPeerCommunication, 3);

        let current_level = {
            let meter = trust_meter.lock().unwrap();
            meter.sovereignty_level()
        };

        Self {
            current_level,
            trust_meter,
            api_registry,
        }
    }

    /// Check if a sovereignty API call is allowed at current level.
    ///
    /// # Side Effects
    ///
    /// This method records a trust event via the TrustMeter for audit purposes.
    /// Approved calls record `SovereignActionApproved`; denied calls record
    /// `BoundaryViolation`. Callers should be aware that repeated calls for
    /// the same logical operation will accumulate trust score changes.
    pub fn check_access(&self, api: &SovereigntyApi, step_id: u64) -> Result<(), SovereigntyError> {
        let required_level = self.api_registry.get(api).unwrap_or(&0);
        let current = self.current_level.as_u8();

        let result = if current >= *required_level {
            Ok(())
        } else {
            Err(SovereigntyError::Unauthorized {
                required: *required_level,
                current,
            })
        };

        // Record the event for audit
        let behavior = if current >= *required_level {
            TrustBehavior::SovereignActionApproved
        } else {
            TrustBehavior::BoundaryViolation
        };
        let mut meter = self.trust_meter.lock().unwrap();
        meter.record_event(step_id, behavior);

        result
    }

    /// Update sovereignty level based on current trust score.
    ///
    /// This method should be called after every trust score change.
    pub fn update_level(&mut self) {
        let new_level = {
            let meter = self.trust_meter.lock().unwrap();
            meter.sovereignty_level()
        };
        self.current_level = new_level;
    }

    /// Get current sovereignty level.
    pub fn current_level(&self) -> SovereigntyLevel {
        self.current_level
    }

    /// Get current trust score (read-only).
    pub fn trust_score(&self) -> f64 {
        let meter = self.trust_meter.lock().unwrap();
        meter.score()
    }
}
