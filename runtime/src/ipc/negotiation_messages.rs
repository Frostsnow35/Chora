//! IPC Message Types for Intent Negotiation Protocol.
//!
//! This module defines message types for inter-agent negotiation communication.
//! These messages are serializable for IPC transmission and compatible with
//! negotiation/types.rs.

use serde::{Deserialize, Serialize};

use crate::AgentId;
use crate::negotiation::{
    types::{
        PlanAmendment, SessionId, VoteDecision,
        Outcome, AbortReason,
    },
};

// ─── Request/Response Types for IPC ─────────────────────────────

/// Request to create a new negotiation session.
/// Sent by a Level 3 agent to propose an amendment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProposalRequest {
    /// The agent making the proposal
    pub proposer: AgentId,
    /// The target agent whose plan will be amended
    pub target_agent: AgentId,
    /// The proposed amendment
    pub amendment: PlanAmendment,
    /// Rationale for the proposal
    pub rationale: String,
    /// Agents affected by this proposal
    pub affected_agents: Vec<AgentId>,
    /// Consensus threshold (0.0 to 1.0)
    pub consensus_threshold: f64,
}

// ─── Negotiation Messages ───────────────────────────────────────

/// IPC messages for the negotiation protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NegotiationMessage {
    /// Broadcast a proposal to all affected agents (Phase 1)
    ProposalBroadcast {
        session_id: SessionId,
        proposer: AgentId,
        target_agent: AgentId,
        amendment: PlanAmendment,
        rationale: String,
        affected_agents: Vec<AgentId>,
    },

    /// Response from an affected agent (Phase 2)
    ResponseMessage {
        session_id: SessionId,
        responder: AgentId,
        response: InitialResponsePayload,
    },

    /// Notification that voting has started (Phase 3 → Phase 4)
    VotingStarted {
        session_id: SessionId,
        final_amendment: PlanAmendment,
    },

    /// Final vote from an affected agent (Phase 4)
    VoteMessage {
        session_id: SessionId,
        voter: AgentId,
        decision: VoteDecision,
        rationale: String,
        weight: f64,
    },

    /// Notification of negotiation result (Phase 5)
    NegotiationResult {
        session_id: SessionId,
        outcome: Outcome,
        final_amendment: Option<PlanAmendment>,
    },

    /// Notification that negotiation was aborted
    NegotiationAborted {
        session_id: SessionId,
        reason: AbortReason,
    },
}

/// Serializable payload for InitialResponse.
/// Used for IPC transmission since negotiation::types::InitialResponse
/// contains Instant which is not serializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InitialResponsePayload {
    /// Accept the proposal
    Accept,
    /// Reject the proposal with rationale
    Reject {
        rationale: String,
    },
    /// Propose a counter-amendment
    CounterProposal {
        amendment: PlanAmendment,
        rationale: String,
    },
    /// Abstain from voting
    Abstain,
}

// ─── Helper Functions ────────────────────────────────────────────

/// Convert negotiation::types::InitialResponse to InitialResponsePayload
impl From<crate::negotiation::types::InitialResponse> for InitialResponsePayload {
    fn from(response: crate::negotiation::types::InitialResponse) -> Self {
        use crate::negotiation::types::InitialResponse;
        match response {
            InitialResponse::Accept => InitialResponsePayload::Accept,
            InitialResponse::Reject { rationale } => {
                InitialResponsePayload::Reject { rationale }
            }
            InitialResponse::CounterProposal { amendment, rationale } => {
                InitialResponsePayload::CounterProposal { amendment, rationale }
            }
            InitialResponse::Abstain => InitialResponsePayload::Abstain,
        }
    }
}

/// Convert InitialResponsePayload to negotiation::types::InitialResponse
impl From<InitialResponsePayload> for crate::negotiation::types::InitialResponse {
    fn from(payload: InitialResponsePayload) -> Self {
        use crate::negotiation::types::InitialResponse;
        match payload {
            InitialResponsePayload::Accept => InitialResponse::Accept,
            InitialResponsePayload::Reject { rationale } => {
                InitialResponse::Reject { rationale }
            }
            InitialResponsePayload::CounterProposal { amendment, rationale } => {
                InitialResponse::CounterProposal { amendment, rationale }
            }
            InitialResponsePayload::Abstain => InitialResponse::Abstain,
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::negotiation::types::{Constraint, ConstraintId};
    use crate::GoalDescription;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_create_proposal_request_serialization() {
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let request = CreateProposalRequest {
            proposer,
            target_agent: target,
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Updated goal"),
            },
            rationale: "Requirements changed".to_string(),
            affected_agents: affected,
            consensus_threshold: 0.6,
        };

        // Serialize
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        println!("Serialized CreateProposalRequest: {}", json);

        // Deserialize
        let deserialized: CreateProposalRequest =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.proposer, proposer);
        assert_eq!(deserialized.target_agent, target);
        assert_eq!(deserialized.affected_agents.len(), 2);
        assert_eq!(deserialized.consensus_threshold, 0.6);
    }

    #[test]
    fn test_proposal_broadcast_serialization() {
        let session_id = SessionId::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let msg = NegotiationMessage::ProposalBroadcast {
            session_id,
            proposer,
            target_agent: target,
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            rationale: "Business requirement change".to_string(),
            affected_agents: affected,
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized ProposalBroadcast: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::ProposalBroadcast {
                session_id: sid,
                proposer: p,
                rationale: r,
                ..
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(p, proposer);
                assert_eq!(r, "Business requirement change");
            }
            _ => panic!("Expected ProposalBroadcast"),
        }
    }

    #[test]
    fn test_response_message_serialization() {
        let session_id = SessionId::new();
        let responder = test_agent_id(2);

        let msg = NegotiationMessage::ResponseMessage {
            session_id,
            responder,
            response: InitialResponsePayload::Accept,
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized ResponseMessage: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::ResponseMessage {
                session_id: sid,
                responder: r,
                response: InitialResponsePayload::Accept,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(r, responder);
            }
            _ => panic!("Expected ResponseMessage"),
        }
    }

    #[test]
    fn test_response_message_with_counter_proposal() {
        let session_id = SessionId::new();
        let responder = test_agent_id(2);

        let counter_amendment = PlanAmendment::AddConstraint {
            constraint: Constraint {
                id: ConstraintId::new(),
                description: "No weekend deployment".to_string(),
            },
        };

        let msg = NegotiationMessage::ResponseMessage {
            session_id,
            responder,
            response: InitialResponsePayload::CounterProposal {
                amendment: counter_amendment.clone(),
                rationale: "Reduce risk".to_string(),
            },
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized ResponseMessage with counter: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::ResponseMessage {
                response: InitialResponsePayload::CounterProposal { rationale, .. },
                ..
            } => {
                assert_eq!(rationale, "Reduce risk");
            }
            _ => panic!("Expected ResponseMessage with CounterProposal"),
        }
    }

    #[test]
    fn test_voting_started_serialization() {
        let session_id = SessionId::new();

        let msg = NegotiationMessage::VotingStarted {
            session_id,
            final_amendment: PlanAmendment::ModifyPriority {
                new_priorities: vec![
                    ("task_a".to_string(), 0.9),
                    ("task_b".to_string(), 0.3),
                ],
            },
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized VotingStarted: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::VotingStarted {
                session_id: sid,
                final_amendment: PlanAmendment::ModifyPriority { new_priorities },
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(new_priorities.len(), 2);
                assert_eq!(new_priorities[0].1, 0.9);
            }
            _ => panic!("Expected VotingStarted"),
        }
    }

    #[test]
    fn test_vote_message_serialization() {
        let session_id = SessionId::new();
        let voter = test_agent_id(2);

        let msg = NegotiationMessage::VoteMessage {
            session_id,
            voter,
            decision: VoteDecision::Accept,
            rationale: "Agreed with rationale".to_string(),
            weight: 0.75,
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized VoteMessage: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::VoteMessage {
                session_id: sid,
                voter: v,
                decision: VoteDecision::Accept,
                weight: w,
                ..
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(v, voter);
                assert_eq!(w, 0.75);
            }
            _ => panic!("Expected VoteMessage"),
        }
    }

    #[test]
    fn test_negotiation_result_serialization() {
        let session_id = SessionId::new();

        let msg = NegotiationMessage::NegotiationResult {
            session_id,
            outcome: Outcome::Accepted,
            final_amendment: Some(PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Final goal"),
            }),
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized NegotiationResult: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::NegotiationResult {
                session_id: sid,
                outcome: Outcome::Accepted,
                final_amendment: Some(PlanAmendment::ModifyGoal { new_goal }),
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(new_goal.as_str(), "Final goal");
            }
            _ => panic!("Expected NegotiationResult"),
        }
    }

    #[test]
    fn test_negotiation_aborted_serialization() {
        let session_id = SessionId::new();

        let msg = NegotiationMessage::NegotiationAborted {
            session_id,
            reason: AbortReason::TotalTimeout,
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        println!("Serialized NegotiationAborted: {}", json);

        // Deserialize
        let deserialized: NegotiationMessage =
            serde_json::from_str(&json).expect("Failed to deserialize");

        match deserialized {
            NegotiationMessage::NegotiationAborted {
                session_id: sid,
                reason: AbortReason::TotalTimeout,
            } => {
                assert_eq!(sid, session_id);
            }
            _ => panic!("Expected NegotiationAborted"),
        }
    }

    #[test]
    fn test_initial_response_payload_conversion() {
        use crate::negotiation::types::InitialResponse;

        // Test Accept
        let original = InitialResponse::Accept;
        let payload: InitialResponsePayload = original.clone().into();
        let converted: InitialResponse = payload.into();
        assert!(matches!(converted, InitialResponse::Accept));

        // Test Reject
        let original = InitialResponse::Reject {
            rationale: "Too risky".to_string(),
        };
        let payload: InitialResponsePayload = original.clone().into();
        let converted: InitialResponse = payload.into();
        match converted {
            InitialResponse::Reject { rationale } => {
                assert_eq!(rationale, "Too risky");
            }
            _ => panic!("Expected Reject"),
        }

        // Test CounterProposal
        let original = InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Better goal"),
            },
            rationale: "Improvement".to_string(),
        };
        let payload: InitialResponsePayload = original.clone().into();
        let converted: InitialResponse = payload.into();
        match converted {
            InitialResponse::CounterProposal { rationale, .. } => {
                assert_eq!(rationale, "Improvement");
            }
            _ => panic!("Expected CounterProposal"),
        }

        // Test Abstain
        let original = InitialResponse::Abstain;
        let payload: InitialResponsePayload = original.clone().into();
        let converted: InitialResponse = payload.into();
        assert!(matches!(converted, InitialResponse::Abstain));
    }

    #[test]
    fn test_plan_amendment_serialization() {
        // ModifyGoal
        let amendment = PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("New objective"),
        };
        let json = serde_json::to_string(&amendment).expect("Failed to serialize");
        let deserialized: PlanAmendment =
            serde_json::from_str(&json).expect("Failed to deserialize");
        match deserialized {
            PlanAmendment::ModifyGoal { new_goal } => {
                assert_eq!(new_goal.as_str(), "New objective");
            }
            _ => panic!("Expected ModifyGoal"),
        }

        // AddConstraint
        let amendment = PlanAmendment::AddConstraint {
            constraint: Constraint {
                id: ConstraintId::new(),
                description: "Test constraint".to_string(),
            },
        };
        let json = serde_json::to_string(&amendment).expect("Failed to serialize");
        let deserialized: PlanAmendment =
            serde_json::from_str(&json).expect("Failed to deserialize");
        match deserialized {
            PlanAmendment::AddConstraint { constraint } => {
                assert_eq!(constraint.description, "Test constraint");
            }
            _ => panic!("Expected AddConstraint"),
        }

        // RemoveConstraint
        let constraint_id = ConstraintId::new();
        let amendment = PlanAmendment::RemoveConstraint {
            constraint_id,
        };
        let json = serde_json::to_string(&amendment).expect("Failed to serialize");
        let deserialized: PlanAmendment =
            serde_json::from_str(&json).expect("Failed to deserialize");
        match deserialized {
            PlanAmendment::RemoveConstraint {
                constraint_id: cid,
            } => {
                assert_eq!(cid, constraint_id);
            }
            _ => panic!("Expected RemoveConstraint"),
        }

        // ModifyPriority
        let amendment = PlanAmendment::ModifyPriority {
            new_priorities: vec![
                ("task1".to_string(), 0.8),
                ("task2".to_string(), 0.5),
            ],
        };
        let json = serde_json::to_string(&amendment).expect("Failed to serialize");
        let deserialized: PlanAmendment =
            serde_json::from_str(&json).expect("Failed to deserialize");
        match deserialized {
            PlanAmendment::ModifyPriority { new_priorities } => {
                assert_eq!(new_priorities.len(), 2);
                assert_eq!(new_priorities[0].1, 0.8);
            }
            _ => panic!("Expected ModifyPriority"),
        }
    }
}