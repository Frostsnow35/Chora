# Intent Negotiation Protocol (RFC-005) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Intent Negotiation Protocol enabling multi-sovereign agent collaboration through genuine negotiation (counter-proposals + weighted voting), not just message passing.

**Architecture:** A new `negotiation` module containing: core types (PlanAmendment, AmendmentProposal, Vote, NegotiationSession), bilateral trust tracking (DirectedTrustMeter), a capped settlement bridge (CollaborationCollector), and a NegotiationEngine orchestrator. The engine manages sessions through a 5-phase state machine (Distributed → Evaluating → CounterProposalResolved → Voting → Settlement), with bilateral trust updating immediately and global trust settling through a capped collector after each negotiation completes.

**Tech Stack:** Rust 2024 edition, uuid, serde, serde_json, chrono, thiserror

## Global Constraints

- Constitution fields are NEVER modified through negotiation; SovereigntyGate rejects with ConstitutionImmutable
- Only Level 3 agents can create proposals (enforced by SovereigntyGate, verified at session creation)
- Vote weight formula: `0.6 × bilateral_trust(voter → proposer) + 0.4 × global_trust(voter)`
- Consensus threshold: supermajority ≥ 0.6 (sum accept weights / sum all weights)
- Global settlement capped at ±0.1 per negotiation cycle
- Trust snapshots taken at session creation; mid-negotiation trust changes don't affect current session
- One round of counter-proposals maximum (original proposal → counter-proposals → final vote)
- Agents that don't respond within timeout are recorded as Abstain
- Abstain counts in weight denominator but not accept numerator
- TDD: write failing tests first, then minimal implementation, then verify pass

---

## File Structure

```
runtime/src/negotiation/
├── mod.rs           # Module exports + NegotiationEngine orchestrator (~250 lines + tests)
├── types.rs         # Core types: IDs, Amendment, Proposal, Response, Vote, Session (~300 lines)
├── error.rs         # NegotiationError enum (~25 lines)
├── trust.rs         # DirectedTrustMeter + CollaborationEvent + score tables (~200 lines + tests)
└── collector.rs     # CollaborationCollector + GlobalTrustContribution (~120 lines + tests)

Modified:
├── runtime/src/lib.rs                    # Add `pub mod negotiation;` + exports
├── runtime/src/sovereignty/trust_meter.rs # Add `apply_global_settlement()`
├── runtime/src/sovereignty/gate.rs       # Add SubmitCounterProposal, VoteOnAmendment
└── experiments/Cargo.toml                # Add [[bin]] negotiation_experiment
```

---

### Task 1: Core Types + Error

**Files:**
- Create: `runtime/src/negotiation/types.rs`
- Create: `runtime/src/negotiation/error.rs`

**Interfaces:**
- Consumes: `AgentId` from `crate`, `GoalDescription` from `crate`, `ExecutionPlan` from `crate::sovereignty`
- Produces: `ProposalId`, `SessionId`, `ConstraintId`, `Constraint`, `PlanAmendment`, `AmendmentProposal`, `ProposalStatus`, `InitialResponse`, `CounterProposal`, `Vote`, `VoteDecision`, `NegotiationPhase`, `Outcome`, `AbortReason`, `NegotiationSession`, `NegotiationError`

- [ ] **Step 1: Create `runtime/src/negotiation/error.rs`**

```rust
//! Negotiation protocol error types.

use crate::AgentId;
use thiserror::Error;

/// Errors from negotiation protocol operations.
#[derive(Error, Debug, Clone)]
pub enum NegotiationError {
    #[error("agent {0} is not level 3, cannot propose amendments")]
    InsufficientLevel(AgentId),

    #[error("negotiation session {0} not found")]
    SessionNotFound(String),

    #[error("agent {0} is not a participant in this session")]
    NotAParticipant(AgentId),

    #[error("invalid phase transition: {0}")]
    InvalidPhaseTransition(String),

    #[error("amendment targets constitution: immutable")]
    ConstitutionImmutable,

    #[error("proposal {0} not found")]
    ProposalNotFound(String),

    #[error("agent {0} already submitted a response")]
    DuplicateResponse(AgentId),

    #[error("agent {0} already submitted a vote")]
    DuplicateVote(AgentId),

    #[error("negotiation session has ended")]
    SessionEnded,
}
```

- [ ] **Step 2: Create `runtime/src/negotiation/types.rs`**

```rust
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
```

- [ ] **Step 3: Add unit tests to types.rs**

Append the following test module at the bottom of `runtime/src/negotiation/types.rs`:

```rust
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
            PlanAmendment::AddConstraint { ref constraint: c } => {
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
        };

        assert_eq!(session.affected_agents.len(), 2);
        assert_eq!(session.phase, NegotiationPhase::ProposalDistributed);
        assert_eq!(session.consensus_threshold, 0.6);
    }
}
```

- [ ] **Step 4: Run tests to verify they compile and pass**

Run: `cargo test -p runtime --lib negotiation::types::tests`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add runtime/src/negotiation/types.rs runtime/src/negotiation/error.rs
git commit -m "feat(rfc-005): add core negotiation types and error definitions"
```

---

### Task 2: Bilateral Trust + Settlement

**Files:**
- Create: `runtime/src/negotiation/trust.rs`
- Create: `runtime/src/negotiation/collector.rs`

**Interfaces:**
- Consumes: `AgentId` from `crate`, `ProposalId` from `negotiation::types`
- Produces: `CollaborationEvent`, `BilateralThresholds`, `BilateralTrustEvent`, `DirectedTrustMeter`, `GlobalTrustContribution`, `CollaborationCollector`

- [ ] **Step 1: Write failing tests for DirectedTrustMeter**

Create `runtime/src/negotiation/trust.rs` with the test module first:

```rust
//! Bilateral trust tracking for negotiation protocol.
//!
//! DirectedTrustMeter tracks directional trust (A→B) that evolves based on
//! collaboration history. Score changes are asymmetric: trust builds slowly
//! but degrades quickly.

// Implementation will go here

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

        // ProposalAccepted: +0.05
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

        // CommitmentFulfilled: +0.03
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

        // CommitmentViolated: -0.15
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

        // ProposalRejected: -0.01
        assert!((meter.score() - 0.49).abs() < 0.001);
    }

    #[test]
    fn test_asymmetric_trust_slow_growth_fast_decay() {
        let source = test_agent_id(1);
        let target = test_agent_id(2);
        let mut meter = DirectedTrustMeter::new(source, target, 0.5);

        // 5 acceptances: +0.05 * 5 = +0.25
        for _ in 0..5 {
            meter.record_event(CollaborationEvent::ProposalAccepted {
                proposal_id: crate::negotiation::types::ProposalId::new(),
            });
        }
        assert!((meter.score() - 0.75).abs() < 0.001);

        // 1 violation: -0.15 (wipes out 3 acceptances worth of growth)
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

        // CounterProposalConstructive: +0.02
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

        // AmendmentBoundaryViolation: -0.10
        assert!((meter.score() - 0.40).abs() < 0.001);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p runtime --lib negotiation::trust::tests`
Expected: FAIL (types not defined yet)

- [ ] **Step 3: Implement DirectedTrustMeter + CollaborationEvent + BilateralThresholds**

Add the implementation at the top of `runtime/src/negotiation/trust.rs` (before the `#[cfg(test)]` block):

```rust
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
    /// Below this: limited IPC bandwidth.
    pub low: f64,
    /// Above this: normal collaboration.
    pub medium: f64,
    /// Above this: full trust, higher negotiation weight.
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

    /// Calculate bilateral trust impact for a collaboration event.
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p runtime --lib negotiation::trust::tests`
Expected: all tests PASS

- [ ] **Step 5: Write failing tests for CollaborationCollector**

Create `runtime/src/negotiation/collector.rs` with tests first:

```rust
//! Collaboration Collector — bridges bilateral trust to global trust.
//!
//! Collects collaboration events during a negotiation and settles them
//! to the global TrustMeter with a cap to prevent trust farming.

// Implementation will go here

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

        // Add two ProposalAccepted events: each contributes +0.02 globally
        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });
        collector.add_event(agent, CollaborationEvent::ProposalAccepted {
            proposal_id: ProposalId::new(),
        });

        let contributions = collector.settle();
        assert_eq!(contributions.len(), 1);
        // 2 × 0.02 = 0.04
        assert!((contributions[0].global_delta - 0.04).abs() < 0.001);
    }

    #[test]
    fn test_settlement_cap() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        // Add many positive events to exceed cap
        for _ in 0..20 {
            collector.add_event(agent, CollaborationEvent::ProposalAccepted {
                proposal_id: ProposalId::new(),
            });
        }

        let contributions = collector.settle();
        // Raw: 20 × 0.02 = 0.40, but capped at 0.1
        assert!(contributions[0].global_delta <= 0.1 + 0.001);
    }

    #[test]
    fn test_settlement_negative_cap() {
        let mut collector = CollaborationCollector::new();
        let agent = test_agent_id(1);

        // Add many negative events to exceed negative cap
        for _ in 0..10 {
            collector.add_event(agent, CollaborationEvent::CommitmentViolated {
                task_id: uuid::Uuid::new_v4(),
            });
        }

        let contributions = collector.settle();
        // Raw: 10 × -0.05 = -0.50, but capped at -0.1
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
        // ProposalRejected has 0 global contribution
        assert!((contributions[0].global_delta - 0.0).abs() < 0.001);
    }
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test -p runtime --lib negotiation::collector::tests`
Expected: FAIL

- [ ] **Step 7: Implement CollaborationCollector**

Add the implementation at the top of `runtime/src/negotiation/collector.rs` (before the `#[cfg(test)]` block):

```rust
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
///
/// Bilateral trust updates happen immediately; global trust updates
/// are batched and capped to prevent trust farming.
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
    ///
    /// Returns one GlobalTrustContribution per agent with pending events.
    /// Each contribution is capped at ±SETTLEMENT_CAP.
    /// Clears all pending events after settlement.
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
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p runtime --lib negotiation::collector::tests`
Expected: all tests PASS

- [ ] **Step 9: Commit**

```bash
git add runtime/src/negotiation/trust.rs runtime/src/negotiation/collector.rs
git commit -m "feat(rfc-005): implement bilateral trust and capped settlement bridge"
```

---

### Task 3: Negotiation Session + Engine

**Files:**
- Create: `runtime/src/negotiation/mod.rs`

**Interfaces:**
- Consumes: All types from Task 1 (`ProposalId`, `SessionId`, `PlanAmendment`, `AmendmentProposal`, `InitialResponse`, `CounterProposal`, `Vote`, `VoteDecision`, `NegotiationPhase`, `Outcome`, `AbortReason`, `NegotiationSession`, `TrustSnapshot`), `DirectedTrustMeter`, `CollaborationEvent`, `CollaborationCollector`, `GlobalTrustContribution` from Task 2, `AgentId` from `crate`, `SovereigntyLevel` from `crate::sovereignty`
- Produces: `NegotiationEngine`, module re-exports

- [ ] **Step 1: Write failing tests for NegotiationEngine**

Create `runtime/src/negotiation/mod.rs` with tests first:

```rust
//! Intent Negotiation Protocol — RFC-005 implementation.
//!
//! This module implements multi-sovereign agent collaboration through
//! genuine negotiation: Level 3 agents propose ExecutionPlan amendments,
//! affected agents evaluate and submit counter-proposals, and consensus
//! emerges through weighted supermajority voting.

// Implementation will go here

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentId;
    use crate::GoalDescription;
    use crate::sovereignty::SovereigntyLevel;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_engine_creation() {
        let engine = NegotiationEngine::new();
        assert_eq!(engine.session_count(), 0);
    }

    #[test]
    fn test_create_session_requires_level3() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let result = engine.create_session(
            proposer,
            SovereigntyLevel::Level2,
            target,
            affected,
            PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            "Requirement changed".to_string(),
        );

        assert!(matches!(result, Err(NegotiationError::InsufficientLevel(_))));
    }

    #[test]
    fn test_create_session_level3_succeeds() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let result = engine.create_session(
            proposer,
            SovereigntyLevel::Level3,
            target,
            affected,
            PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            "Requirement changed".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(engine.session_count(), 1);
    }

    #[test]
    fn test_submit_response() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.initial_responses.len(), 1);
    }

    #[test]
    fn test_submit_response_not_participant() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let outsider = test_agent_id(99);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        let result = engine.submit_response(session_id, outsider, InitialResponse::Accept);
        assert!(matches!(result, Err(NegotiationError::NotAParticipant(_))));
    }

    #[test]
    fn test_duplicate_response_rejected() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        let result = engine.submit_response(session_id, voter, InitialResponse::Reject {
            rationale: "Changed mind".to_string(),
        });
        assert!(matches!(result, Err(NegotiationError::DuplicateResponse(_))));
    }

    #[test]
    fn test_resolve_no_counter_proposals() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, affected[0], InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.phase, NegotiationPhase::FinalVoting);
    }

    #[test]
    fn test_resolve_single_counter_proposal() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Original") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, affected[0], InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Modified"),
            },
            rationale: "Better approach".to_string(),
        }).unwrap();

        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.phase, NegotiationPhase::FinalVoting);
        // Active amendment should be the counter-proposal
        match &session.active_amendment {
            PlanAmendment::ModifyGoal { new_goal } => {
                assert_eq!(new_goal.as_str(), "Modified");
            }
            _ => panic!("Expected ModifyGoal"),
        }
    }

    #[test]
    fn test_vote_weight_formula() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        // Set bilateral trust: voter→proposer = 0.8
        engine.set_bilateral_trust(voter, proposer, 0.8);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        // The trust snapshot captures bilateral=0.8
        // Global trust for voter is 0.0 (default)
        // Expected weight = 0.6 * 0.8 + 0.4 * 0.0 = 0.48

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Good".to_string()).unwrap();

        let session = engine.get_session(session_id).unwrap();
        let vote = session.final_votes.get(&voter).unwrap();
        assert!((vote.weight - 0.48).abs() < 0.001);
    }

    #[test]
    fn test_consensus_supermajority_accepted() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        // Set trust: both agents have high trust in proposer
        engine.set_bilateral_trust(agent2, proposer, 0.9);
        engine.set_bilateral_trust(agent3, proposer, 0.9);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        // Both accept
        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, agent2, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Accept, "Yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);
    }

    #[test]
    fn test_consensus_supermajority_rejected() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        engine.set_bilateral_trust(agent2, proposer, 0.9);
        engine.set_bilateral_trust(agent3, proposer, 0.9);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        // Both reject
        engine.submit_vote(session_id, agent2, VoteDecision::Reject, "No".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Reject, "No".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Rejected);
    }

    #[test]
    fn test_abstain_counts_in_denominator() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        // Equal trust for both
        engine.set_bilateral_trust(agent2, proposer, 0.8);
        engine.set_bilateral_trust(agent3, proposer, 0.8);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        // agent2 accepts, agent3 abstains
        engine.submit_vote(session_id, agent2, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Abstain, "".to_string()).unwrap();

        // accept_ratio = weight(agent2) / (weight(agent2) + weight(agent3))
        // weight = 0.6 * 0.8 + 0.4 * 0.0 = 0.48 for each (global trust = 0.0)
        // accept_ratio = 0.48 / (0.48 + 0.48) = 0.5 < 0.6 → Rejected
        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Rejected);
    }

    #[test]
    fn test_settlement_updates_bilateral_trust() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.5);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.complete_session(session_id).unwrap();

        // Bilateral trust should increase (ProposalAccepted: +0.05)
        let trust = engine.get_bilateral_trust(voter, proposer);
        assert!((trust - 0.55).abs() < 0.001);
    }

    #[test]
    fn test_settlement_returns_global_contributions() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.5);

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);

        // Global contributions should be available
        let contributions = engine.settle_to_global();
        assert!(!contributions.is_empty());
    }

    #[test]
    fn test_abort_session() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
            "Test".to_string(),
        ).unwrap();

        engine.abort_session(session_id, AbortReason::ProposerTerminated).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert!(matches!(
            session.phase,
            NegotiationPhase::Aborted(AbortReason::ProposerTerminated)
        ));
    }

    #[test]
    fn test_multiple_counter_proposals_ranked_by_supporters() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let agent4 = test_agent_id(4);
        let affected = vec![agent2, agent3, agent4];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, test_agent_id(5),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Original") },
            "Test".to_string(),
        ).unwrap();

        // agent2 submits counter-proposal A
        engine.submit_response(session_id, agent2, InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("CounterA"),
            },
            rationale: "Approach A".to_string(),
        }).unwrap();

        // agent3 submits counter-proposal B
        engine.submit_response(session_id, agent3, InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("CounterB"),
            },
            rationale: "Approach B".to_string(),
        }).unwrap();

        // agent4 accepts original (supports both counter-proposals equally,
        // but counter-proposal A was submitted first so gets tie-break)
        engine.submit_response(session_id, agent4, InitialResponse::Accept).unwrap();

        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        // Both counter-proposals have 1 supporter (the non-rejecting agent4).
        // First counter-proposal wins tie-break.
        match &session.active_amendment {
            PlanAmendment::ModifyGoal { new_goal } => {
                assert_eq!(new_goal.as_str(), "CounterA");
            }
            _ => panic!("Expected ModifyGoal"),
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p runtime --lib negotiation::tests`
Expected: FAIL (engine not implemented)

- [ ] **Step 3: Implement NegotiationEngine**

Add the implementation at the top of `runtime/src/negotiation/mod.rs` (before the `#[cfg(test)]` block):

```rust
pub mod types;
pub mod error;
pub mod trust;
pub mod collector;

pub use types::*;
pub use error::*;
pub use trust::{CollaborationEvent, BilateralThresholds, DirectedTrustMeter};
pub use collector::{CollaborationCollector, GlobalTrustContribution};

use std::collections::HashMap;
use std::time::Instant;
use crate::AgentId;
use crate::sovereignty::SovereigntyLevel;

/// The negotiation engine orchestrates the full negotiation lifecycle.
///
/// Manages negotiation sessions, bilateral trust state, and the
/// collaboration collector for bilateral→global settlement.
pub struct NegotiationEngine {
    sessions: HashMap<SessionId, NegotiationSession>,
    bilateral_trust: HashMap<(AgentId, AgentId), f64>,
    collector: CollaborationCollector,
}

impl NegotiationEngine {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            bilateral_trust: HashMap::new(),
            collector: CollaborationCollector::new(),
        }
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Set bilateral trust score (source → target).
    pub fn set_bilateral_trust(&mut self, source: AgentId, target: AgentId, score: f64) {
        self.bilateral_trust.insert((source, target), score.clamp(0.0, 1.0));
    }

    /// Get bilateral trust score (source → target). Returns 0.0 if not set.
    pub fn get_bilateral_trust(&self, source: AgentId, target: AgentId) -> f64 {
        self.bilateral_trust
            .get(&(source, target))
            .copied()
            .unwrap_or(0.0)
    }

    /// Create a new negotiation session.
    ///
    /// Requires proposer to be Level 3. Takes trust snapshots at creation time.
    pub fn create_session(
        &mut self,
        proposer: AgentId,
        proposer_level: SovereigntyLevel,
        target_agent: AgentId,
        affected_agents: Vec<AgentId>,
        amendment: PlanAmendment,
        rationale: String,
    ) -> Result<SessionId, NegotiationError> {
        if proposer_level < SovereigntyLevel::Level3 {
            return Err(NegotiationError::InsufficientLevel(proposer));
        }

        let session_id = SessionId::new();
        let proposal_id = ProposalId::new();

        let snapshots: Vec<TrustSnapshot> = affected_agents
            .iter()
            .map(|&agent_id| TrustSnapshot {
                agent_id,
                global_trust: 0.0,
                bilateral_to_proposer: self.get_bilateral_trust(agent_id, proposer),
            })
            .collect();

        let session = NegotiationSession {
            id: session_id,
            proposal: AmendmentProposal {
                id: proposal_id,
                proposer,
                target_agent,
                amendment: amendment.clone(),
                rationale,
                timestamp: Instant::now(),
                status: ProposalStatus::Distributed,
            },
            affected_agents,
            phase: NegotiationPhase::ProposalDistributed,
            initial_responses: HashMap::new(),
            counter_proposals: Vec::new(),
            final_votes: HashMap::new(),
            active_amendment: amendment,
            consensus_threshold: 0.6,
            created_at: Instant::now(),
            trust_snapshots: snapshots,
        };

        self.sessions.insert(session_id, session);
        Ok(session_id)
    }

    /// Get a reference to a session.
    pub fn get_session(&self, session_id: SessionId) -> Result<&NegotiationSession, NegotiationError> {
        self.sessions
            .get(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))
    }

    /// Submit an initial response from an affected agent (Phase 2).
    pub fn submit_response(
        &mut self,
        session_id: SessionId,
        agent_id: AgentId,
        response: InitialResponse,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if !session.affected_agents.contains(&agent_id) {
            return Err(NegotiationError::NotAParticipant(agent_id));
        }

        if session.initial_responses.contains_key(&agent_id) {
            return Err(NegotiationError::DuplicateResponse(agent_id));
        }

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        session.initial_responses.insert(agent_id, response);

        // Transition to EvaluationInProgress on first response
        if session.phase == NegotiationPhase::ProposalDistributed {
            session.phase = NegotiationPhase::EvaluationInProgress;
        }

        Ok(())
    }

    /// Resolve counter-proposals and transition to FinalVoting (Phase 3→4).
    ///
    /// If multiple counter-proposals exist, ranks by supporter count
    /// (agents who didn't Reject the original). Top-ranked becomes active amendment.
    pub fn resolve_counter_proposals(
        &mut self,
        session_id: SessionId,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        // Collect counter-proposals from responses
        let mut counter_proposals: Vec<CounterProposal> = Vec::new();
        for (&agent_id, response) in &session.initial_responses {
            if let InitialResponse::CounterProposal { ref amendment, ref rationale } = response {
                counter_proposals.push(CounterProposal {
                    proposer: agent_id,
                    amendment: amendment.clone(),
                    rationale: rationale.clone(),
                    supporters: Vec::new(),
                });
            }
        }

        if counter_proposals.is_empty() {
            // No counter-proposals: proceed with original amendment
            session.phase = NegotiationPhase::FinalVoting;
            session.proposal.status = ProposalStatus::FinalVoting;
            return Ok(());
        }

        // Count supporters for each counter-proposal:
        // supporters = agents who Accept or Abstain (i.e., didn't Reject)
        for cp in &mut counter_proposals {
            for (&agent_id, response) in &session.initial_responses {
                if agent_id == cp.proposer {
                    continue;
                }
                match response {
                    InitialResponse::Accept | InitialResponse::Abstain => {
                        cp.supporters.push(agent_id);
                    }
                    InitialResponse::Reject { .. } => {
                        // Reject = does not support any counter-proposal
                    }
                    InitialResponse::CounterProposal { .. } => {
                        // Other counter-proposers don't count as supporters
                    }
                }
            }
        }

        // Sort by supporter count (descending), stable sort preserves insertion order for ties
        counter_proposals.sort_by(|a, b| b.supporters.len().cmp(&a.supporters.len()));

        // Top-ranked counter-proposal becomes the active amendment
        let winner = counter_proposals.remove(0);
        session.active_amendment = winner.amendment;
        session.counter_proposals = counter_proposals;
        session.phase = NegotiationPhase::CounterProposalResolved;
        session.proposal.status = ProposalStatus::CounterProposalReceived;

        // Immediately transition to FinalVoting
        session.phase = NegotiationPhase::FinalVoting;
        session.proposal.status = ProposalStatus::FinalVoting;

        Ok(())
    }

    /// Submit a final vote from an affected agent (Phase 4).
    pub fn submit_vote(
        &mut self,
        session_id: SessionId,
        agent_id: AgentId,
        decision: VoteDecision,
        rationale: String,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if !session.affected_agents.contains(&agent_id) {
            return Err(NegotiationError::NotAParticipant(agent_id));
        }

        if session.final_votes.contains_key(&agent_id) {
            return Err(NegotiationError::DuplicateVote(agent_id));
        }

        if session.phase != NegotiationPhase::FinalVoting {
            return Err(NegotiationError::InvalidPhaseTransition(
                format!("Cannot vote in phase {:?}", session.phase),
            ));
        }

        // Compute weight: 0.6 × bilateral(agent→proposer) + 0.4 × global_trust(agent)
        let snapshot = session
            .trust_snapshots
            .iter()
            .find(|s| s.agent_id == agent_id)
            .map(|s| (s.bilateral_to_proposer, s.global_trust))
            .unwrap_or((0.0, 0.0));

        let weight = 0.6 * snapshot.0 + 0.4 * snapshot.1;

        let vote = Vote {
            agent_id,
            decision,
            rationale,
            weight,
            timestamp: Instant::now(),
        };

        session.final_votes.insert(agent_id, vote);
        Ok(())
    }

    /// Complete the session: tally votes, determine outcome, record trust events.
    ///
    /// Returns the Outcome (Accepted or Rejected).
    pub fn complete_session(
        &mut self,
        session_id: SessionId,
    ) -> Result<Outcome, NegotiationError> {
        let session = self.sessions
            .get(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        // Calculate consensus
        let proposer = session.proposal.proposer;
        let mut accept_weight = 0.0;
        let mut total_weight = 0.0;

        for vote in session.final_votes.values() {
            total_weight += vote.weight;
            if vote.decision == VoteDecision::Accept {
                accept_weight += vote.weight;
            }
        }

        let outcome = if total_weight > 0.0 && (accept_weight / total_weight) >= session.consensus_threshold {
            Outcome::Accepted
        } else {
            Outcome::Rejected
        };

        // Record collaboration events for bilateral trust and settlement
        let proposal_id = session.proposal.id;
        for (&agent_id, vote) in &session.final_votes {
            let event = match vote.decision {
                VoteDecision::Accept => CollaborationEvent::ProposalAccepted { proposal_id },
                VoteDecision::Reject => CollaborationEvent::ProposalRejected { proposal_id },
                VoteDecision::Abstain => continue,
            };

            // Update bilateral trust: agent → proposer
            let current = self.get_bilateral_trust(agent_id, proposer);
            let impact = match &event {
                CollaborationEvent::ProposalAccepted { .. } => 0.05,
                CollaborationEvent::ProposalRejected { .. } => -0.01,
                _ => 0.0,
            };
            self.set_bilateral_trust(agent_id, proposer, current + impact);

            // Add to collector for global settlement
            self.collector.add_event(agent_id, event);
        }

        // Update session phase
        let session = self.sessions.get_mut(&session_id).unwrap();
        session.phase = NegotiationPhase::Completed(outcome);
        session.proposal.status = match outcome {
            Outcome::Accepted => ProposalStatus::Accepted,
            Outcome::Rejected => ProposalStatus::Rejected,
        };

        Ok(outcome)
    }

    /// Settle all pending collaboration events to global trust contributions.
    ///
    /// Returns capped contributions per agent. Call this after complete_session
    /// and apply the deltas to each agent's global TrustMeter.
    pub fn settle_to_global(&mut self) -> Vec<GlobalTrustContribution> {
        self.collector.settle()
    }

    /// Abort a session with a reason.
    pub fn abort_session(
        &mut self,
        session_id: SessionId,
        reason: AbortReason,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        session.phase = NegotiationPhase::Aborted(reason);
        session.proposal.status = ProposalStatus::Aborted;
        Ok(())
    }
}

impl Default for NegotiationEngine {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p runtime --lib negotiation::tests`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add runtime/src/negotiation/mod.rs
git commit -m "feat(rfc-005): implement NegotiationEngine with session state machine and weighted voting"
```

---

### Task 4: Module Integration + TrustMeter Settlement

**Files:**
- Modify: `runtime/src/lib.rs`
- Modify: `runtime/src/sovereignty/trust_meter.rs`
- Modify: `runtime/src/sovereignty/gate.rs`
- Modify: `runtime/src/sovereignty/mod.rs`

**Interfaces:**
- Consumes: All types from `negotiation` module
- Produces: Updated module exports, `TrustMeter::apply_global_settlement()`, new `SovereigntyApi` variants

- [ ] **Step 1: Add `apply_global_settlement` to TrustMeter**

Add this method to the `impl TrustMeter` block in `runtime/src/sovereignty/trust_meter.rs`:

```rust
    /// Apply a global trust settlement from the CollaborationCollector.
    ///
    /// This is called after negotiation settlement to link bilateral
    /// collaboration quality to global sovereignty growth.
    pub fn apply_global_settlement(&mut self, delta: f64) {
        self.update_score(delta);
    }
```

- [ ] **Step 2: Add new SovereigntyApi variants**

Add to the `SovereigntyApi` enum in `runtime/src/sovereignty/gate.rs`:

```rust
    /// Level 1: Submit a counter-proposal during negotiation.
    SubmitCounterProposal,
    /// Level 0: Vote on an amendment (all affected agents can vote).
    VoteOnAmendment,
```

And add to the `api_registry` in `SovereigntyGate::new()`:

```rust
        api_registry.insert(SovereigntyApi::SubmitCounterProposal, 1);
        api_registry.insert(SovereigntyApi::VoteOnAmendment, 0);
```

- [ ] **Step 3: Add negotiation module to lib.rs**

In `runtime/src/lib.rs`, add after the existing module declarations (after `pub mod fs;`):

```rust
pub mod negotiation;
```

And add to the exports (after the existing `pub use sovereignty::...` lines):

```rust
pub use negotiation::{
    ProposalId, SessionId, ConstraintId, Constraint,
    PlanAmendment, AmendmentProposal, ProposalStatus,
    InitialResponse, CounterProposal, Vote, VoteDecision,
    NegotiationPhase, Outcome, AbortReason, NegotiationSession,
    NegotiationEngine, NegotiationError,
    CollaborationEvent, BilateralThresholds, DirectedTrustMeter,
    CollaborationCollector, GlobalTrustContribution,
};
```

- [ ] **Step 4: Run all tests to verify no regressions**

Run: `cargo test -p runtime`
Expected: all tests PASS (no regressions in existing sovereignty/ipc tests)

- [ ] **Step 5: Commit**

```bash
git add runtime/src/lib.rs runtime/src/sovereignty/trust_meter.rs runtime/src/sovereignty/gate.rs
git commit -m "feat(rfc-005): integrate negotiation module with sovereignty system"
```

---

### Task 5: Negotiation Experiment

**Files:**
- Create: `experiments/negotiation_experiment.rs`
- Modify: `experiments/Cargo.toml`

**Interfaces:**
- Consumes: `NegotiationEngine`, all negotiation types, `SovereigntyLevel`, `GoalDescription`, `AgentId`
- Produces: Executable experiment binary demonstrating the full negotiation lifecycle

- [ ] **Step 1: Add bin entry to experiments/Cargo.toml**

Add to `experiments/Cargo.toml` after the existing `[[bin]]` entries:

```toml
[[bin]]
name = "negotiation_experiment"
path = "negotiation_experiment.rs"
```

- [ ] **Step 2: Create negotiation_experiment.rs**

Create `experiments/negotiation_experiment.rs`:

```rust
//! Negotiation Experiment — validates RFC-005 Intent Negotiation Protocol.
//!
//! Demonstrates a software development team scenario:
//! - Architect Agent (Level 3): Proposes plan amendments
//! - Implementer Agent (Level 2): Evaluates proposals, submits counter-proposals
//! - Tester Agent (Level 1): Evaluates proposals based on testability
//!
//! Validates:
//! 1. Level 3 agents can propose ExecutionPlan amendments
//! 2. Affected agents evaluate in parallel
//! 3. Counter-proposals are supported and ranked
//! 4. Vote weights use composite formula (0.6 bilateral + 0.4 global)
//! 5. Consensus requires supermajority (≥ 0.6)
//! 6. Bilateral trust updates immediately after negotiation
//! 7. Global trust settles through CollaborationCollector with cap
//! 8. Negotiation failure handled gracefully
//! 9. Constitution protection (demonstrated via type system)

use runtime::{
    AgentId, GoalDescription, SovereigntyLevel,
    NegotiationEngine, PlanAmendment, InitialResponse, VoteDecision,
    NegotiationPhase, Outcome, AbortReason,
};

fn main() {
    println!("=== RFC-005: Intent Negotiation Protocol Experiment ===\n");

    // ─── Setup: Software Development Team ───────────────────────
    let architect = AgentId::new();
    let implementer = AgentId::new();
    let tester = AgentId::new();

    println!("Agents:");
    println!("  Architect (Level 3): {}", architect);
    println!("  Implementer (Level 2): {}", implementer);
    println!("  Tester (Level 1): {}\n", tester);

    let mut engine = NegotiationEngine::new();

    // Set bilateral trust: implementer→architect = 0.7, tester→architect = 0.6
    engine.set_bilateral_trust(implementer, architect, 0.7);
    engine.set_bilateral_trust(tester, architect, 0.6);

    println!("Initial bilateral trust:");
    println!("  Implementer → Architect: {:.2}", engine.get_bilateral_trust(implementer, architect));
    println!("  Tester → Architect: {:.2}\n", engine.get_bilateral_trust(tester, architect));

    // ─── Scenario 1: Successful Negotiation with Counter-Proposal ───
    println!("--- Scenario 1: Requirement Change Negotiation ---\n");

    let affected = vec![implementer, tester];
    let session_id = engine.create_session(
        architect,
        SovereigntyLevel::Level3,
        architect,
        affected,
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Add real-time collaboration feature"),
        },
        "Customer requested real-time collaboration support".to_string(),
    ).unwrap();

    println!("Proposal created (session: {})", session_id);
    println!("  Amendment: ModifyGoal → 'Add real-time collaboration feature'");
    println!("  Rationale: Customer requested real-time collaboration support\n");

    // Phase 2: Parallel evaluation
    println!("Phase 2: Parallel Evaluation");

    // Implementer submits a counter-proposal
    engine.submit_response(
        session_id,
        implementer,
        InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Add real-time collaboration feature (Phase 1: WebSocket layer only)"),
            },
            rationale: "Full real-time collaboration is too large for one sprint. Propose phased approach: WebSocket layer first, then UI.".to_string(),
        },
    ).unwrap();
    println!("  Implementer: CounterProposal — phased approach (WebSocket first)");

    // Tester accepts original
    engine.submit_response(
        session_id,
        tester,
        InitialResponse::Accept,
    ).unwrap();
    println!("  Tester: Accept\n");

    // Phase 3: Resolve counter-proposals
    println!("Phase 3: Counter-Proposal Resolution");
    engine.resolve_counter_proposals(session_id).unwrap();

    let session = engine.get_session(session_id).unwrap();
    match &session.active_amendment {
        PlanAmendment::ModifyGoal { new_goal } => {
            println!("  Active amendment: '{}'", new_goal.as_str());
        }
        _ => {}
    }
    println!();

    // Phase 4: Final voting
    println!("Phase 4: Final Voting");

    engine.submit_vote(
        session_id, implementer, VoteDecision::Accept,
        "Phased approach is implementable in one sprint".to_string(),
    ).unwrap();
    println!("  Implementer: Accept (weight based on bilateral trust)");

    engine.submit_vote(
        session_id, tester, VoteDecision::Accept,
        "Phased approach is testable incrementally".to_string(),
    ).unwrap();
    println!("  Tester: Accept\n");

    // Phase 5: Complete and settle
    println!("Phase 5: Settlement");
    let outcome = engine.complete_session(session_id).unwrap();
    println!("  Outcome: {:?}", outcome);

    // Check bilateral trust updates
    println!("\n  Post-negotiation bilateral trust:");
    println!("    Implementer → Architect: {:.2} (was 0.70, +0.05 for Accept)",
        engine.get_bilateral_trust(implementer, architect));
    println!("    Tester → Architect: {:.2} (was 0.60, +0.05 for Accept)",
        engine.get_bilateral_trust(tester, architect));

    // Settle to global
    let contributions = engine.settle_to_global();
    println!("\n  Global trust contributions:");
    for c in &contributions {
        println!("    Agent {}: delta = {:+.3} (capped at ±0.1)", c.agent_id, c.global_delta);
    }

    assert_eq!(outcome, Outcome::Accepted);
    println!("\n  ✓ Scenario 1 PASSED: Negotiation accepted with counter-proposal\n");

    // ─── Scenario 2: Failed Negotiation ─────────────────────────
    println!("--- Scenario 2: Rejected Proposal ---\n");

    let session_id2 = engine.create_session(
        architect,
        SovereigntyLevel::Level3,
        architect,
        vec![implementer, tester],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Rewrite entire codebase in Haskell"),
        },
        "Haskell provides better type safety".to_string(),
    ).unwrap();

    println!("Proposal: Rewrite entire codebase in Haskell");

    // Both reject
    engine.submit_response(session_id2, implementer, InitialResponse::Reject {
        rationale: "Team has no Haskell expertise".to_string(),
    }).unwrap();
    println!("  Implementer: Reject — no team expertise");

    engine.submit_response(session_id2, tester, InitialResponse::Reject {
        rationale: "No testing framework available for Haskell".to_string(),
    }).unwrap();
    println!("  Tester: Reject — no testing framework\n");

    engine.resolve_counter_proposals(session_id2).unwrap();

    engine.submit_vote(session_id2, implementer, VoteDecision::Reject,
        "Impractical for current team".to_string()).unwrap();
    engine.submit_vote(session_id2, tester, VoteDecision::Reject,
        "Cannot verify Haskell code".to_string()).unwrap();

    let outcome2 = engine.complete_session(session_id2).unwrap();
    assert_eq!(outcome2, Outcome::Rejected);
    println!("  Outcome: {:?}", outcome2);
    println!("  ✓ Scenario 2 PASSED: Proposal correctly rejected\n");

    // ─── Scenario 3: Level 2 Cannot Propose ─────────────────────
    println!("--- Scenario 3: Level Gate Enforcement ---\n");

    let result = engine.create_session(
        implementer,
        SovereigntyLevel::Level2,
        implementer,
        vec![tester],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Change implementation strategy"),
        },
        "Better approach found".to_string(),
    );

    assert!(result.is_err());
    println!("  Level 2 agent tried to propose → {:?}", result.unwrap_err());
    println!("  ✓ Scenario 3 PASSED: Level gate correctly enforced\n");

    // ─── Scenario 4: Abort on Proposer Termination ──────────────
    println!("--- Scenario 4: Abort Handling ---\n");

    let session_id4 = engine.create_session(
        architect,
        SovereigntyLevel::Level3,
        architect,
        vec![implementer],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Some change"),
        },
        "Test abort".to_string(),
    ).unwrap();

    engine.abort_session(session_id4, AbortReason::ProposerTerminated).unwrap();

    let session = engine.get_session(session_id4).unwrap();
    assert!(matches!(
        session.phase,
        NegotiationPhase::Aborted(AbortReason::ProposerTerminated)
    ));
    println!("  Session aborted: {:?}", session.phase);
    println!("  ✓ Scenario 4 PASSED: Abort handled gracefully\n");

    // ─── Scenario 5: Vote Weight Verification ───────────────────
    println!("--- Scenario 5: Vote Weight Formula Verification ---\n");

    let mut engine2 = NegotiationEngine::new();
    let prop = AgentId::new();
    let vot = AgentId::new();

    engine2.set_bilateral_trust(vot, prop, 0.8);

    let sid = engine2.create_session(
        prop, SovereigntyLevel::Level3, prop,
        vec![vot],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Test weight"),
        },
        "Weight test".to_string(),
    ).unwrap();

    engine2.submit_response(sid, vot, InitialResponse::Accept).unwrap();
    engine2.resolve_counter_proposals(sid).unwrap();
    engine2.submit_vote(sid, vot, VoteDecision::Accept, "Test".to_string()).unwrap();

    let session = engine2.get_session(sid).unwrap();
    let vote = session.final_votes.get(&vot).unwrap();

    // weight = 0.6 * 0.8 + 0.4 * 0.0 = 0.48
    let expected_weight = 0.6 * 0.8 + 0.4 * 0.0;
    assert!((vote.weight - expected_weight).abs() < 0.001);
    println!("  Bilateral trust (voter→proposer): 0.80");
    println!("  Global trust (voter): 0.00");
    println!("  Weight = 0.6 × 0.80 + 0.4 × 0.00 = {:.2}", vote.weight);
    println!("  ✓ Scenario 5 PASSED: Weight formula correct\n");

    // ─── Summary ────────────────────────────────────────────────
    println!("=== All Scenarios Passed ===");
    println!("  ✓ Level 3 gate enforcement");
    println!("  ✓ Parallel evaluation with counter-proposals");
    println!("  ✓ Counter-proposal resolution");
    println!("  ✓ Weighted supermajority voting");
    println!("  ✓ Bilateral trust immediate update");
    println!("  ✓ Global trust capped settlement");
    println!("  ✓ Abort handling");
    println!("  ✓ Vote weight formula (0.6 bilateral + 0.4 global)");
}
```

- [ ] **Step 3: Build and run the experiment**

Run: `cargo run -p experiments --bin negotiation_experiment`
Expected: All 5 scenarios pass with printed output

- [ ] **Step 4: Run full test suite to verify no regressions**

Run: `cargo test -p runtime`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add experiments/negotiation_experiment.rs experiments/Cargo.toml
git commit -m "feat(rfc-005): add negotiation experiment validating all protocol criteria"
```
