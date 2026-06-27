//! Trust Meter — tracks agent trustworthiness over time.
//!
//! The Trust Meter maintains a trust score (0.0-1.0) that evolves based on
//! agent behavior. Score changes are asymmetric: trust builds slowly (+0.02 to +0.05)
//! but degrades quickly (-0.01 to -0.15).

use std::time::Instant;
use serde::{Deserialize, Serialize};

/// Behavior types that affect trust score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustBehavior {
    /// User Space's sovereign action was approved by Sovereignty Gate.
    SovereignActionApproved,
    /// User Space's sovereign action was denied by Sovereignty Gate.
    SovereignActionDenied,
    /// User Space yielded cooperatively (not monopolizing resources).
    CooperativeYield,
    /// User Space attempted unauthorized sovereignty action (serious violation).
    BoundaryViolation,
    /// User Space completed a sub-goal (strongest trust signal).
    GoalProgress,
}

/// Thresholds for sovereignty level transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereigntyThresholds {
    /// Score required for Level 1 (can reject boundary-violating requests).
    pub level_1: f64,
    /// Score required for Level 2 (can self-terminate).
    pub level_2: f64,
    /// Score required for Level 3 (can propose Intent amendments).
    pub level_3: f64,
}

impl Default for SovereigntyThresholds {
    fn default() -> Self {
        Self {
            level_1: 0.6,
            level_2: 0.75,
            level_3: 0.9,
        }
    }
}

/// A single trust event recorded in history.
#[derive(Debug, Clone)]
pub struct TrustEvent {
    /// When the event occurred.
    pub timestamp: Instant,
    /// Which step triggered this event.
    pub step_id: u64,
    /// Type of behavior that affected trust.
    pub behavior_type: TrustBehavior,
    /// Score change (positive or negative).
    pub impact: f64,
}

/// Trust Meter — maintains trust score and history.
///
/// This component lives in Kernel Space and is write-protected from User Space.
/// User Space can only read the current score via `score()` method.
#[derive(Debug)]
pub struct TrustMeter {
    /// Current trust score (0.0-1.0).
    score: f64,
    /// History of recent trust events (last N events).
    history: Vec<TrustEvent>,
    /// Maximum history size.
    max_history: usize,
    /// Thresholds for sovereignty level transitions.
    thresholds: SovereigntyThresholds,
}

impl TrustMeter {
    /// Create a new TrustMeter with initial score.
    pub fn new(initial_score: f64, thresholds: SovereigntyThresholds) -> Self {
        let initial_score = initial_score.clamp(0.0, 1.0);
        Self {
            score: initial_score,
            history: Vec::new(),
            max_history: 50,
            thresholds,
        }
    }

    /// Get current trust score (read-only for User Space).
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Get current sovereignty level based on trust score.
    pub fn sovereignty_level(&self) -> crate::SovereigntyLevel {
        if self.score >= self.thresholds.level_3 {
            crate::SovereigntyLevel::Level3
        } else if self.score >= self.thresholds.level_2 {
            crate::SovereigntyLevel::Level2
        } else if self.score >= self.thresholds.level_1 {
            crate::SovereigntyLevel::Level1
        } else {
            crate::SovereigntyLevel::Level0
        }
    }

    /// Record a trust event and update score.
    ///
    /// This method is called by SovereigntyGate after every sovereignty API call.
    pub fn record_event(&mut self, step_id: u64, behavior: TrustBehavior) {
        let impact = Self::score_impact(&behavior);

        // Update score
        self.update_score(impact);

        // Record event
        let event = TrustEvent {
            timestamp: Instant::now(),
            step_id,
            behavior_type: behavior,
            impact,
        };

        // Maintain history window
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(event);
    }

    /// Calculate score impact for a behavior type.
    ///
    /// Asymmetric design: trust builds slowly but degrades quickly.
    fn score_impact(behavior: &TrustBehavior) -> f64 {
        match behavior {
            TrustBehavior::SovereignActionApproved => 0.02,
            TrustBehavior::SovereignActionDenied => -0.01,
            TrustBehavior::CooperativeYield => 0.01,
            TrustBehavior::BoundaryViolation => -0.15,
            TrustBehavior::GoalProgress => 0.05,
        }
    }

    /// Update trust score with saturation to [0.0, 1.0].
    fn update_score(&mut self, impact: f64) {
        self.score = (self.score + impact).clamp(0.0, 1.0);
    }

    /// Get recent trust events (for debugging/auditing).
    pub fn history(&self) -> &[TrustEvent] {
        &self.history
    }
}