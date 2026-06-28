//! Core types for the Intent Negotiation Protocol.
//!
//! Types follow RFC-005 §2: PlanAmendment operates on ExecutionPlan only.
//! Constitution is never modified through this protocol.

use std::collections::HashMap;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::AgentId;
use crate::GoalDescription;

// ─── ID Types ───────────────────────────────────────────────

/// Unique identifier for an amendment proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProposalId(uuid::Uuid);

impl ProposalId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ProposalId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProposalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a negotiation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(uuid::Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConstraintId(uuid::Uuid);

impl ConstraintId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ConstraintId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Amendment Types ────────────────────────────────────────

/// A constraint that can be added to or removed from an ExecutionPlan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: ConstraintId,
    pub description: String,
}

/// Amendment targeting ExecutionPlan fields only.
/// Constitution is never touched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanAmendment {
    ModifyGoal {
        new_goal: GoalDescription,
    },
    AddConstraint {
        constraint: Constraint,
    },
    RemoveConstraint {
        constraint_id: ConstraintId,
    },
    ModifyPriority {
        new_priorities: Vec<(String, f64)>,
    },
}

// ─── Proposal ───────────────────────────────────────────────

/// Status of an amendment proposal within a negotiation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Distributed,
    Evaluating,
    CounterProposalReceived,
    FinalVoting,
    Accepted,
    Rejected,
    Aborted,
}

/// An amendment proposal submitted by a Level 3 agent.
#[derive(Debug, Clone)]
pub struct AmendmentProposal {
    pub id: ProposalId,
    pub proposer: AgentId,
    pub target_agent: AgentId,
    pub amendment: PlanAmendment,
    pub rationale: String,
    pub timestamp: Instant,
    pub status: ProposalStatus,
}

// ─── Response & Counter-Proposal ────────────────────────────

/// An affected agent's initial response to a proposal (Phase 2).
#[derive(Debug, Clone)]
pub enum InitialResponse {
    Accept,
    Reject {
        rationale: String,
    },
    CounterProposal {
        amendment: PlanAmendment,
        rationale: String,
    },
    Abstain,
}

/// A counter-proposal with its proposer and supporter list.
#[derive(Debug, Clone)]
pub struct CounterProposal {
    pub proposer: AgentId,
    pub amendment: PlanAmendment,
    pub rationale: String,
    pub supporters: Vec<AgentId>,
}

// ─── Vote ───────────────────────────────────────────────────

/// Final vote decision on the (possibly amended) proposal (Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteDecision {
    Accept,
    Reject,
    Abstain,
}

/// A weighted vote from an affected agent.
#[derive(Debug, Clone)]
pub struct Vote {
    pub agent_id: AgentId,
    pub decision: VoteDecision,
    pub rationale: String,
    pub weight: f64,
    pub timestamp: Instant,
}

// ─── Session ────────────────────────────────────────────────

/// Outcome of a completed negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Accepted,
    Rejected,
}

/// Reason for negotiation abort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortReason {
    ProposerTerminated,
    TotalTimeout,
    ConstitutionViolation,
}

/// Current phase of a negotiation session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NegotiationPhase {
    ProposalDistributed,
    EvaluationInProgress,
    CounterProposalResolved,
    FinalVoting,
    Completed(Outcome),
    Aborted(AbortReason),
}

/// Trust snapshot for an agent at session creation time.
/// Prevents mid-negotiation trust changes from affecting the current session.
#[derive(Debug, Clone)]
pub struct TrustSnapshot {
    pub agent_id: AgentId,
    pub global_trust: f64,
    pub bilateral_to_proposer: f64,
}

/// A negotiation session tracking the full lifecycle.
#[derive(Debug, Clone)]
pub struct NegotiationSession {
    pub id: SessionId,
    pub proposal: AmendmentProposal,
    pub affected_agents: Vec<AgentId>,
    pub phase: NegotiationPhase,
    pub initial_responses: HashMap<AgentId, InitialResponse>,
    pub counter_proposals: Vec<CounterProposal>,
    pub final_votes: HashMap<AgentId, Vote>,
    pub active_amendment: PlanAmendment,
    pub consensus_threshold: f64,
    pub created_at: Instant,
    pub trust_snapshots: Vec<TrustSnapshot>,
    pub timeout_duration: std::time::Duration,
    pub last_activity_at: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_proposal_id_uniqueness() {
        let id1 = ProposalId::new();
        let id2 = ProposalId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_uniqueness() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_plan_amendment_modify_goal() {
        let amendment = PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("New goal"),
        };
        match amendment {
            PlanAmendment::ModifyGoal { ref new_goal } => {
                assert_eq!(new_goal.as_str(), "New goal");
            }
            _ => panic!("Expected ModifyGoal"),
        }
    }

    #[test]
    fn test_plan_amendment_add_constraint() {
        let constraint = Constraint {
            id: ConstraintId::new(),
            description: "No destructive operations".to_string(),
        };
        let amendment = PlanAmendment::AddConstraint {
            constraint: constraint.clone(),
        };
        match amendment {
            PlanAmendment::AddConstraint { constraint: ref c } => {
                assert_eq!(c.description, "No destructive operations");
            }
            _ => panic!("Expected AddConstraint"),
        }
    }

    #[test]
    fn test_plan_amendment_modify_priority() {
        let amendment = PlanAmendment::ModifyPriority {
            new_priorities: vec![
                ("task_a".to_string(), 0.9),
                ("task_b".to_string(), 0.3),
            ],
        };
        match amendment {
            PlanAmendment::ModifyPriority { ref new_priorities } => {
                assert_eq!(new_priorities.len(), 2);
                assert_eq!(new_priorities[0].1, 0.9);
            }
            _ => panic!("Expected ModifyPriority"),
        }
    }

    #[test]
    fn test_initial_response_variants() {
        let accept = InitialResponse::Accept;
        let reject = InitialResponse::Reject {
            rationale: "Too risky".to_string(),
        };
        let counter = InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Modified goal"),
            },
            rationale: "Better approach".to_string(),
        };
        let abstain = InitialResponse::Abstain;

        assert!(matches!(accept, InitialResponse::Accept));
        assert!(matches!(reject, InitialResponse::Reject { .. }));
        assert!(matches!(counter, InitialResponse::CounterProposal { .. }));
        assert!(matches!(abstain, InitialResponse::Abstain));
    }

    #[test]
    fn test_vote_decision_eq() {
        assert_eq!(VoteDecision::Accept, VoteDecision::Accept);
        assert_ne!(VoteDecision::Accept, VoteDecision::Reject);
    }

    #[test]
    fn test_negotiation_phase_completed() {
        let phase = NegotiationPhase::Completed(Outcome::Accepted);
        assert!(matches!(phase, NegotiationPhase::Completed(Outcome::Accepted)));
    }

    #[test]
    fn test_negotiation_session_creation() {
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];
        let amendment = PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Updated goal"),
        };

        let session = NegotiationSession {
            id: SessionId::new(),
            proposal: AmendmentProposal {
                id: ProposalId::new(),
                proposer,
                target_agent: target,
                amendment: amendment.clone(),
                rationale: "Requirement changed".to_string(),
                timestamp: Instant::now(),
                status: ProposalStatus::Distributed,
            },
            affected_agents: affected.clone(),
            phase: NegotiationPhase::ProposalDistributed,
            initial_responses: HashMap::new(),
            counter_proposals: Vec::new(),
            final_votes: HashMap::new(),
            active_amendment: amendment,
            consensus_threshold: 0.6,
            created_at: Instant::now(),
            trust_snapshots: Vec::new(),
            timeout_duration: std::time::Duration::from_secs(300),
            last_activity_at: Instant::now(),
        };

        assert_eq!(session.affected_agents.len(), 2);
        assert_eq!(session.phase, NegotiationPhase::ProposalDistributed);
        assert_eq!(session.consensus_threshold, 0.6);
    }
}
