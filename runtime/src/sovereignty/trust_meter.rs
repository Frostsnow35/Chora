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

// Manual implementation of Serialize and Deserialize to skip the timestamp field
impl Serialize for TrustEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TrustEvent", 3)?;
        state.serialize_field("step_id", &self.step_id)?;
        state.serialize_field("behavior_type", &self.behavior_type)?;
        state.serialize_field("impact", &self.impact)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TrustEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            StepId,
            BehaviorType,
            Impact,
        }

        struct TrustEventVisitor;

        impl<'de> Visitor<'de> for TrustEventVisitor {
            type Value = TrustEvent;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct TrustEvent")
            }

            fn visit_map<V>(self, mut map: V) -> Result<TrustEvent, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut step_id = None;
                let mut behavior_type = None;
                let mut impact = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::StepId => {
                            if step_id.is_some() {
                                return Err(de::Error::duplicate_field("step_id"));
                            }
                            step_id = Some(map.next_value()?);
                        }
                        Field::BehaviorType => {
                            if behavior_type.is_some() {
                                return Err(de::Error::duplicate_field("behavior_type"));
                            }
                            behavior_type = Some(map.next_value()?);
                        }
                        Field::Impact => {
                            if impact.is_some() {
                                return Err(de::Error::duplicate_field("impact"));
                            }
                            impact = Some(map.next_value()?);
                        }
                    }
                }
                let step_id = step_id.ok_or_else(|| de::Error::missing_field("step_id"))?;
                let behavior_type = behavior_type.ok_or_else(|| de::Error::missing_field("behavior_type"))?;
                let impact = impact.ok_or_else(|| de::Error::missing_field("impact"))?;
                Ok(TrustEvent {
                    timestamp: Instant::now(), // Initialize with current time when deserializing
                    step_id,
                    behavior_type,
                    impact,
                })
            }
        }

        const FIELDS: &[&str] = &["step_id", "behavior_type", "impact"];
        deserializer.deserialize_struct("TrustEvent", FIELDS, TrustEventVisitor)
    }
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

    /// Apply a global trust settlement from the CollaborationCollector.
    ///
    /// This is called after negotiation settlement to link bilateral
    /// collaboration quality to global sovereignty growth.
    pub fn apply_global_settlement(&mut self, delta: f64) {
        self.update_score(delta);
    }
}
