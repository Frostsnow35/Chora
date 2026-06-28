//! Bilateral trust tracking for negotiation protocol.

use serde::{Deserialize, Serialize};
use crate::AgentId;
use super::types::ProposalId;

/// Collaboration events that affect bilateral trust.
#[derive(Debug, Clone)]
pub enum CollaborationEvent {
    ProposalAccepted { proposal_id: ProposalId },
    ProposalRejected { proposal_id: ProposalId },
    CommitmentFulfilled { task_id: uuid::Uuid },
    CommitmentViolated { task_id: uuid::Uuid },
    CounterProposalConstructive { proposal_id: ProposalId },
    AmendmentBoundaryViolation { proposal_id: ProposalId },
}

/// Thresholds for bilateral trust levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BilateralThresholds {
    pub low: f64,
    pub medium: f64,
    pub high: f64,
}

impl Default for BilateralThresholds {
    fn default() -> Self {
        Self {
            low: 0.3,
            medium: 0.5,
            high: 0.8,
        }
    }
}

/// A single bilateral trust event in history.
#[derive(Debug, Clone)]
pub struct BilateralTrustEvent {
    pub event: CollaborationEvent,
    pub impact: f64,
}

/// Directional trust meter: tracks A→B trust score.
#[derive(Debug)]
pub struct DirectedTrustMeter {
    source: AgentId,
    target: AgentId,
    score: f64,
    history: Vec<BilateralTrustEvent>,
    thresholds: BilateralThresholds,
}

impl DirectedTrustMeter {
    pub fn new(source: AgentId, target: AgentId, initial_score: f64) -> Self {
        Self {
            source,
            target,
            score: initial_score.clamp(0.0, 1.0),
            history: Vec::new(),
            thresholds: BilateralThresholds::default(),
        }
    }

    pub fn with_thresholds(
        source: AgentId,
        target: AgentId,
        initial_score: f64,
        thresholds: BilateralThresholds,
    ) -> Self {
        Self {
            source,
            target,
            score: initial_score.clamp(0.0, 1.0),
            history: Vec::new(),
            thresholds,
        }
    }

    pub fn source(&self) -> AgentId {
        self.source
    }

    pub fn target(&self) -> AgentId {
        self.target
    }

    pub fn score(&self) -> f64 {
        self.score
    }

    pub fn thresholds(&self) -> &BilateralThresholds {
        &self.thresholds
    }

    pub fn history(&self) -> &[BilateralTrustEvent] {
        &self.history
    }

    /// Record a collaboration event and update trust score.
    pub fn record_event(&mut self, event: CollaborationEvent) {
        let impact = Self::bilateral_impact(&event);
        self.score = (self.score + impact).clamp(0.0, 1.0);
        self.history.push(BilateralTrustEvent { event, impact });
    }

    fn bilateral_impact(event: &CollaborationEvent) -> f64 {
        match event {
            CollaborationEvent::ProposalAccepted { .. } => 0.05,
            CollaborationEvent::CommitmentFulfilled { .. } => 0.03,
            CollaborationEvent::CounterProposalConstructive { .. } => 0.02,
            CollaborationEvent::ProposalRejected { .. } => -0.01,
            CollaborationEvent::CommitmentViolated { .. } => -0.15,
            CollaborationEvent::AmendmentBoundaryViolation { .. } => -0.10,
        }
    }

    /// Calculate global trust contribution for settlement.
    pub fn global_contribution(event: &CollaborationEvent) -> f64 {
        match event {
            CollaborationEvent::ProposalAccepted { .. } => 0.02,
            CollaborationEvent::CommitmentFulfilled { .. } => 0.01,
            CollaborationEvent::CounterProposalConstructive { .. } => 0.01,
            CollaborationEvent::ProposalRejected { .. } => 0.0,
            CollaborationEvent::CommitmentViolated { .. } => -0.05,
            CollaborationEvent::AmendmentBoundaryViolation { .. } => -0.03,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentId;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_directed_trust_meter_creation() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let meter = DirectedTrustMeter::new(source, target, 0.5);

        assert_eq!(meter.source(), source);
        assert_eq!(meter.target(), target);
        assert!((meter.score() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_directed_trust_meter_score_clamping() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let meter = DirectedTrustMeter::new(source, target, 1.5);
        assert!((meter.score() - 1.0).abs() < 0.001);

        let meter2 = DirectedTrustMeter::new(source, target, -0.5);
        assert!((meter2.score() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_proposal_accepted_increases_trust() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::ProposalAccepted {
            proposal_id: crate::negotiation::types::ProposalId::new(),
        });

        assert!((meter.score() - 0.55).abs() < 0.001);
    }

    #[test]
    fn test_commitment_fulfilled_increases_trust() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::CommitmentFulfilled {
            task_id: uuid::Uuid::new_v4(),
        });

        assert!((meter.score() - 0.53).abs() < 0.001);
    }

    #[test]
    fn test_commitment_violated_decreases_trust_fast() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::CommitmentViolated {
            task_id: uuid::Uuid::new_v4(),
        });

        assert!((meter.score() - 0.35).abs() < 0.001);
    }

    #[test]
    fn test_proposal_rejected_small_decrease() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::ProposalRejected {
            proposal_id: crate::negotiation::types::ProposalId::new(),
        });

        assert!((meter.score() - 0.49).abs() < 0.001);
    }

    #[test]
    fn test_asymmetric_trust_slow_growth_fast_decay() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        for _ in 0..5 {
            meter.record_event(CollaborationEvent::ProposalAccepted {
                proposal_id: crate::negotiation::types::ProposalId::new(),
            });
        }
        assert!((meter.score() - 0.75).abs() < 0.001);

        meter.record_event(CollaborationEvent::CommitmentViolated {
            task_id: uuid::Uuid::new_v4(),
        });
        assert!((meter.score() - 0.60).abs() < 0.001);
    }

    #[test]
    fn test_bilateral_thresholds_default() {
        let thresholds = BilateralThresholds::default();
        assert!(thresholds.low < thresholds.medium);
        assert!(thresholds.medium < thresholds.high);
    }

    #[test]
    fn test_counter_proposal_constructive() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::CounterProposalConstructive {
            proposal_id: crate::negotiation::types::ProposalId::new(),
        });

        assert!((meter.score() - 0.52).abs() < 0.001);
    }

    #[test]
    fn test_boundary_violation_penalty() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        meter.record_event(CollaborationEvent::AmendmentBoundaryViolation {
            proposal_id: crate::negotiation::types::ProposalId::new(),
        });

        assert!((meter.score() - 0.40).abs() < 0.001);
    }
}
