//! Collaboration Collector — bridges bilateral trust to global trust.

use std::collections::HashMap;
use crate::AgentId;
use super::trust::{CollaborationEvent, DirectedTrustMeter};

/// A global trust contribution for a single agent.
#[derive(Debug, Clone)]
pub struct GlobalTrustContribution {
    pub agent_id: AgentId,
    pub global_delta: f64,
}

/// Maximum global trust change per settlement cycle.
const SETTLEMENT_CAP: f64 = 0.1;

/// Collects collaboration events and settles them to global TrustMeter.
#[derive(Debug)]
pub struct CollaborationCollector {
    pending: HashMap<AgentId, Vec<CollaborationEvent>>,
}

impl CollaborationCollector {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add a collaboration event for an agent.
    pub fn add_event(&mut self, agent_id: AgentId, event: CollaborationEvent) {
        self.pending
            .entry(agent_id)
            .or_default()
            .push(event);
    }

    /// Get the number of pending events.
    pub fn pending_count(&self) -> usize {
        self.pending.values().map(|v| v.len()).sum()
    }

    /// Settle all pending events: compute capped global contributions per agent.
    pub fn settle(&mut self) -> Vec<GlobalTrustContribution> {
        let mut contributions = Vec::new();

        for (agent_id, events) in self.pending.drain() {
            let raw_delta: f64 = events
                .iter()
                .map(|e| DirectedTrustMeter::global_contribution(e))
                .sum();

            let capped = raw_delta.clamp(-SETTLEMENT_CAP, SETTLEMENT_CAP);

            contributions.push(GlobalTrustContribution {
                agent_id,
                global_delta: capped,
            });
        }

        contributions
    }
}

impl Default for CollaborationCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentId;
    use crate::negotiation::types::ProposalId;
    use crate::negotiation::trust::CollaborationEvent;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_collector_creation() {
        let collector = CollaborationCollector::new();
        assert_eq!(collector.pending_count(), 0);
    }

    #[test]
    fn test_collector_add_event() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });

        assert_eq!(collector.pending_count(), 1);
    }

    #[test]
    fn test_settlement_basic() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });
        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });

        let contributions = collector.settle();
        assert_eq!(contributions.len(), 1);
        assert!((contributions[0].global_delta - 0.04).abs() < 0.001);
    }

    #[test]
    fn test_settlement_cap() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        for _ in 0..20 {
            collector.add_event(agent, CollaborationEvent::ProposalAccepted {
                proposal_id: ProposalId::new(),
            });
        }

        let contributions = collector.settle();
        assert!(contributions[0].global_delta <= 0.1 + 0.001);
    }

    #[test]
    fn test_settlement_negative_cap() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        for _ in 0..10 {
            collector.add_event(agent, CollaborationEvent::CommitmentViolated {
                task_id: uuid::Uuid::new_v4(),
            });
        }

        let contributions = collector.settle();
        assert!(contributions[0].global_delta >= -0.1 - 0.001);
    }

    #[test]
    fn test_settlement_clears_pending() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });

        assert_eq!(collector.pending_count(), 1);
        let _ = collector.settle();
        assert_eq!(collector.pending_count(), 0);
    }

    #[test]
    fn test_settlement_multiple_agents() {
        let mut collector = CollaborationCollector::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        collector.add_event(agent1, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });
        collector.add_event(agent2, CollaborationEvent::CommitmentViolated {
            task_id: uuid::Uuid::new_v4(),
        });

        let contributions = collector.settle();
        assert_eq!(contributions.len(), 2);

        let c1 = contributions.iter().find(|c| c.agent_id == agent1).unwrap();
        let c2 = contributions.iter().find(|c| c.agent_id == agent2).unwrap();

        assert!((c1.global_delta - 0.02).abs() < 0.001);
        assert!((c2.global_delta - (-0.05)).abs() < 0.001);
    }

    #[test]
    fn test_settlement_zero_contribution_for_rejection() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        collector.add_event(agent, CollaborationEvent::ProposalRejected {
            proposal_id: ProposalId::new(),
        });

        let contributions = collector.settle();
        assert!((contributions[0].global_delta - 0.0).abs() < 0.001);
    }
}
