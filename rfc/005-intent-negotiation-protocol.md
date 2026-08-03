# RFC-005 — Intent Negotiation Protocol

| Field | Value |
|---|---|
| Status | Draft |
| Author | — |
| Created | 2026-06-28 |
| Depends on | `rfc/001-agent-process.md`, `rfc/003-sovereignty-system.md`, `rfc/004-agent-file-system.md` |

---

## Abstract

This RFC proposes the **Intent Negotiation Protocol (INP)** — a framework for multi-sovereign agent collaboration through genuine intent negotiation, not just message passing.

The protocol enables Level 3 agents to propose **ExecutionPlan amendments** (not Constitution changes), affected agents to evaluate and submit counter-proposals, and consensus to emerge through a two-round weighted voting process.

The protocol is built on **bilateral trust models** — directional trust (A→B) that evolves based on collaboration history, affecting negotiation weight and IPC bandwidth. Bilateral trust events feed back to global trust through a **capped settlement mechanism**, linking collaboration quality to sovereignty growth without coupling the two systems.

**Philosophical mapping**:
- Runtime = 场域 (the space that enables multi-sovereign coexistence)
- Negotiation protocol = 场域的"律" (allows forms to emerge, but doesn't prescribe them)
- Multi-agent collaboration = 场域中形式的具体化 (forms concretizing from multi-agent negotiation)

---

## 1. Motivation

### 1.1 The Problem with Current Agent Collaboration

Most multi-agent frameworks treat agents as **task executors**: they receive instructions, execute them, and return results. This model has critical limitations:

- **No genuine negotiation**: Agents cannot propose changes to shared goals
- **No bilateral trust**: Trust is either global or binary (allowed/denied)
- **No constitutional evaluation**: Agents cannot assess proposals against their fundamental identity
- **Central control**: Collaboration is orchestrated by a supervisor, not emergent from agents

### 1.2 The Vision: Agents as Sovereign Collaborators

This RFC proposes a different model: agents as **sovereign collaborators** with:

- **Plan amendment proposals**: Level 3 agents can propose changes to shared ExecutionPlan
- **Counter-proposals**: Affected agents can reject and suggest alternatives
- **Constitutional evaluation**: Each agent assesses proposals against their immutable Constitution
- **Bilateral trust**: Directional trust (A→B) that evolves based on collaboration history
- **Emergent consensus**: Collaboration emerges from negotiation, not central control

### 1.3 Validation Scenario: Software Development Team

To validate the protocol, we will implement a **software development team** scenario:
- **Architect Agent** (Level 3): Proposes plan amendments when requirements change
- **Implementer Agent** (Level 2): Evaluates proposals based on implementation feasibility
- **Tester Agent** (Level 1): Evaluates proposals based on testability

Core conflict: Requirement changes force architectural adjustments, architect proposes Plan Amendment, other agents negotiate and reach consensus.

---

## 2. Core Architecture

### 2.1 Plan Amendment Proposal

Amendments operate on `ExecutionPlan` fields only. `Constitution` (purpose, termination_conditions, sovereignty_boundaries) is **never modifiable** through this protocol.

```rust
pub struct AmendmentProposal {
    pub id: ProposalId,
    pub proposer: AgentId,
    pub target_agent: AgentId,        // Whose ExecutionPlan is being amended
    pub amendment: PlanAmendment,
    pub rationale: String,
    pub timestamp: Instant,
    pub status: ProposalStatus,
}

pub enum PlanAmendment {
    ModifyGoal { new_goal: GoalDescription },
    AddConstraint { constraint: Constraint },
    RemoveConstraint { constraint_id: ConstraintId },
    ModifyPriority { new_priorities: Vec<(String, f64)> },
}

pub enum ProposalStatus {
    Distributed,
    Evaluating,
    CounterProposalReceived,
    FinalVoting,
    Accepted,
    Rejected,
    Aborted(AbortReason),
}
```

**Constitution immutability guarantee**: Any attempt to amend Constitution fields is rejected at the SovereigntyGate level with `AmendmentError::ConstitutionImmutable`. The Gate inspects the amendment type before forwarding to the negotiation protocol.

### 2.2 Bilateral Trust Model

```rust
pub struct DirectedTrustMeter {
    source: AgentId,
    target: AgentId,
    score: f64,                       // 0.0 to 1.0
    history: Vec<BilateralTrustEvent>,
    thresholds: BilateralThresholds,
}

pub struct BilateralThresholds {
    pub low: f64,      // Below this: limited IPC bandwidth
    pub medium: f64,   // Above this: normal collaboration
    pub high: f64,     // Above this: full trust, higher negotiation weight
}

pub enum CollaborationEvent {
    ProposalAccepted { proposal_id: ProposalId },
    ProposalRejected { proposal_id: ProposalId },
    CommitmentFulfilled { task_id: TaskId },
    CommitmentViolated { task_id: TaskId },
    CounterProposalConstructive { proposal_id: ProposalId },
    AmendmentBoundaryViolation { proposal_id: ProposalId },
}
```

**Score update logic (bilateral, immediate)**:

| Event | Bilateral Impact | Rationale |
|-------|-----------------|-----------|
| `ProposalAccepted` | +0.05 | Trust builds slowly |
| `CommitmentFulfilled` | +0.03 | Reliability signal |
| `CounterProposalConstructive` | +0.02 | Engaged collaboration |
| `ProposalRejected` | -0.01 | Reasonable disagreement |
| `CommitmentViolated` | -0.15 | Trust degrades quickly |
| `AmendmentBoundaryViolation` | -0.10 | Attempted Constitution modification |

### 2.3 Bilateral ↔ Global Trust Bridge

The two trust systems are **independent but connected** through a `CollaborationCollector`:

```rust
pub struct CollaborationCollector {
    pending_events: Vec<GlobalTrustContribution>,
    settlement_cap: f64,       // Max ±0.1 per settlement cycle
    settlement_interval: u64,  // Settle every N negotiations (default: 1)
    event_count: u64,
}

pub struct GlobalTrustContribution {
    agent_id: AgentId,
    event: CollaborationEvent,
    global_delta: f64,
}
```

**Settlement logic**:
- Bilateral trust updates are **immediate** (real-time relationship tracking)
- Global trust updates are **batched** (settled after each negotiation completes)
- Each settlement is **capped** at ±0.1 to prevent trust farming through repeated negotiations

**Global trust contribution per event**:

| Event | Bilateral | Global Settlement |
|-------|-----------|-------------------|
| `ProposalAccepted` | +0.05 | +0.02 |
| `CommitmentFulfilled` | +0.03 | +0.01 |
| `CounterProposalConstructive` | +0.02 | +0.01 |
| `ProposalRejected` | -0.01 | 0 (reasonable disagreement) |
| `CommitmentViolated` | -0.15 | -0.05 |
| `AmendmentBoundaryViolation` | -0.10 | -0.03 |

**Key properties**:
- Bilateral → Global: **yes**, through capped settlement
- Global → Bilateral: **no**, global level drops do NOT affect bilateral trust
- Global level determines *who can propose* (Level 3 gate), bilateral trust determines *negotiation weight*

### 2.4 Negotiation Mechanism

```rust
pub struct NegotiationSession {
    pub id: SessionId,
    pub proposal: AmendmentProposal,
    pub affected_agents: Vec<AgentId>,
    pub phase: NegotiationPhase,
    pub initial_responses: HashMap<AgentId, InitialResponse>,
    pub counter_proposals: Vec<CounterProposal>,
    pub final_votes: HashMap<AgentId, Vote>,
    pub consensus_threshold: f64,  // 0.6 (supermajority)
    pub timeout: Duration,
    pub created_at: Instant,
}

pub enum NegotiationPhase {
    ProposalDistributed,
    EvaluationInProgress,
    CounterProposalResolved,
    FinalVoting,
    Completed(Outcome),
    Aborted(AbortReason),
}

pub enum Outcome {
    Accepted,
    Rejected,
}

pub enum AbortReason {
    ProposerTerminated,
    TotalTimeout,
    ConstitutionViolation,
}
```

### 2.5 Response and Vote Types

```rust
pub enum InitialResponse {
    Accept,
    Reject { rationale: String },
    CounterProposal { amendment: PlanAmendment, rationale: String },
    Abstain,
}

pub struct CounterProposal {
    pub proposer: AgentId,
    pub amendment: PlanAmendment,
    pub rationale: String,
    pub supporters: Vec<AgentId>,  // Agents who accept this counter
}

pub struct Vote {
    pub agent_id: AgentId,
    pub decision: VoteDecision,
    pub rationale: String,
    pub weight: f64,
    pub timestamp: Instant,
}

pub enum VoteDecision {
    Accept,
    Reject,
    Abstain,
}
```

**Vote weight formula**:

```
weight = 0.6 × bilateral_trust(voter → proposer) + 0.4 × global_trust(voter)
```

- `bilateral_trust(voter → proposer)`: the voter's trust in the proposer (0.0–1.0)
- `global_trust(voter)`: the voter's global TrustMeter score (0.0–1.0)
- Composite formula ensures: bilateral trust dominates (60%), but global reputation contributes (40%)
- Prevents a voter with zero bilateral trust from having zero influence

**Consensus formula**:

```
accept_ratio = sum(accept_weights) / sum(all_weights)
consensus_reached = accept_ratio ≥ 0.6
```

- Supermajority (60%), not simple majority
- Abstain counts in denominator but not numerator
- Proposer participates in voting with same weight formula

---

## 3. Negotiation Lifecycle

### 3.1 Phase 1: Proposal Distribution

```
1. Proposer (Level 3) creates AmendmentProposal
   │
   ▼
2. Runtime determines affected agents:
   │  ├─ Agents with Intent.parent relationship to proposer
   │  ├─ Agents sharing /shared/ paths with proposer in AFS
   │  └─ Proposer's explicit affected_agents list (validated by Runtime)
   │
   ▼
3. IPC broadcasts proposal to all affected agents
   │
   ▼
4. NegotiationSession enters EvaluationInProgress phase
   (timeout starts)
```

### 3.2 Phase 2: Parallel Evaluation

All affected agents evaluate **concurrently**:

```
Each affected agent:
  a. Checks AmendmentProposal against own Constitution
     └─ If violates boundaries → Reject with rationale
  b. Checks bilateral trust with proposer
     └─ Low trust may bias toward Reject (agent's internal policy)
  c. Submits InitialResponse:
     └─ Accept / Reject / CounterProposal / Abstain
```

Agents evaluate in parallel (no ordering dependency). The session waits until all responses arrive or evaluation timeout expires. Agents that don't respond within timeout are recorded as Abstain.

### 3.3 Phase 3: Counter-Proposal Resolution

```
Case A: 0 counter-proposals
  → Proceed to Phase 4 (final vote on original proposal)

Case B: 1 counter-proposal
  → Counter-proposal becomes the final amendment
  → Proceed to Phase 4

Case C: Multiple counter-proposals
  → Rank by supporter count (agents who Accept each counter)
  → Top-ranked counter-proposal becomes the final amendment
  → Proceed to Phase 4
```

If a counter-proposal violates any agent's Constitution, it is discarded. If all counter-proposals are discarded, fall back to the original proposal.

### 3.4 Phase 4: Final Voting

All affected agents (including proposer) vote on the final amendment:

```
For each agent:
  weight = 0.6 × bilateral(agent → proposer) + 0.4 × global_trust(agent)

accept_ratio = sum(weights where decision=Accept) / sum(all weights)

if accept_ratio ≥ 0.6:
  → Outcome::Accepted
  → Apply PlanAmendment to target agent's ExecutionPlan
else:
  → Outcome::Rejected
  → Maintain original ExecutionPlan
```

### 3.5 Phase 5: Settlement

```
1. Bilateral trust updates (immediate):
   ├─ Accept voters: +0.05 bilateral trust toward proposer
   ├─ Reject voters: -0.01 bilateral trust toward proposer
   └─ CommitmentViolated: -0.15 bilateral trust

2. CollaborationCollector records global contributions:
   ├─ Settle after negotiation completes
   ├─ Apply capped delta to each agent's global TrustMeter
   └─ Cap: max ±0.1 per settlement cycle

3. AFS integration:
   ├─ Record result in /shared/negotiations/<session_id>
   └─ Update /proc/<id>/bilateral_trust virtual files
```

---

## 4. Error Handling

| Scenario | Handling | Trust Impact |
|----------|----------|--------------|
| Proposer terminates mid-negotiation | Session → Aborted(ProposerTerminated), original ExecutionPlan unchanged | Proposer global -0.05 (unreliable) |
| Voter doesn't respond (timeout) | Recorded as Abstain | No impact (agent may be busy) |
| Multiple concurrent proposals | Processed in timestamp order, later ones queued | No impact |
| Trust level changes during negotiation | Snapshot at proposal creation time; changes apply to next session | No impact on current session |
| Counter-proposal violates Constitution | That counter-proposal discarded; fall back to original or next-best | Counter-proposer bilateral -0.03 |
| Total negotiation timeout | Session → Aborted(TotalTimeout) | No impact |
| Amendment targets Constitution | Rejected at SovereigntyGate with ConstitutionImmutable | Proposer bilateral -0.10 |

---

## 5. Integration with Existing Systems

### 5.1 Sovereignty System Integration (RFC-003)

- **Level 3 requirement**: Only Level 3 agents can propose amendments (enforced by SovereigntyGate)
- **Constitutional evaluation**: Each agent's Constitution is consulted during evaluation and voting
- **Global trust impact**: Settlement contributions flow to existing TrustMeter through CollaborationCollector
- **SovereigntyApi extension**:

```rust
pub enum SovereigntyApi {
    RejectRequest,              // Level 1
    SelfTerminate,              // Level 2
    ProposeAmendment,           // Level 3 (existing)
    DirectPeerCommunication,    // Level 3 (existing)
    SubmitCounterProposal,      // Level 1 (any agent that can evaluate)
    VoteOnAmendment,            // Level 0 (all affected agents can vote)
}
```

### 5.2 AFS Integration (RFC-004)

- **`/shared/proposals/`**: Amendment proposals stored here (L2+ read, proposer write)
- **`/shared/negotiations/`**: Negotiation session records (L2+ read, Runtime write)
- **`/proc/<id>/bilateral_trust`**: Virtual file exposing directional trust scores:
  ```
  {
    "outgoing": { "agent_b": 0.75, "agent_c": 0.60 },
    "incoming": { "agent_b": 0.80, "agent_c": 0.45 }
  }
  ```

### 5.3 IPC Integration

- **BroadcastChannel**: For broadcasting proposals to affected agents
- **UnidirectionalChannel**: For submitting InitialResponse and Vote
- **Backpressure**: Limit concurrent negotiation sessions per agent (configurable, default: 3)

---

## 6. Validation Plan

### 6.1 Unit Tests

- [ ] PlanAmendment creation and serialization
- [ ] Constitution immutability enforcement (AmendmentError on Constitution targets)
- [ ] Bilateral trust score updates (asymmetric growth/decay)
- [ ] CollaborationCollector settlement with cap enforcement
- [ ] Vote weight calculation (composite formula)
- [ ] Consensus detection (supermajority ≥ 0.6)
- [ ] Counter-proposal ranking by supporter count
- [ ] NegotiationSession state machine transitions
- [ ] Timeout handling (evaluation, voting, total)

### 6.2 Integration Tests

- [ ] Proposal broadcast via IPC to affected agents
- [ ] Multi-agent negotiation session with parallel evaluation
- [ ] Counter-proposal flow (single and multiple)
- [ ] Trust settlement after negotiation completion
- [ ] AFS integration (proposals in /shared, bilateral_trust in /proc)
- [ ] Proposer termination mid-negotiation
- [ ] Concurrent negotiation limit (backpressure)

### 6.3 Experiment: `experiments/negotiation_experiment.rs`

Demonstrates:
1. **Software development team**: Architect (Level 3), Implementer (Level 2), Tester (Level 1)
2. **Requirement change**: Architect detects conflict, creates AmendmentProposal
3. **Parallel evaluation**: Implementer and Tester evaluate concurrently
4. **Counter-proposal**: Implementer suggests modified plan
5. **Final voting**: Weighted supermajority consensus
6. **Trust settlement**: Bilateral and global trust updates
7. **AFS integration**: Proposals stored in /shared/proposals/
8. **Failure scenario**: Negotiation fails, original ExecutionPlan maintained
9. **Constitution protection**: Attempted Constitution amendment rejected at Gate

Validation criteria:
1. ✓ Level 3 agents can propose ExecutionPlan amendments
2. ✓ Constitution modifications are rejected at SovereigntyGate
3. ✓ Affected agents evaluate in parallel against their constitutions
4. ✓ Counter-proposals are supported and ranked by supporters
5. ✓ Vote weights use composite formula (0.6 bilateral + 0.4 global)
6. ✓ Consensus requires supermajority (≥ 0.6)
7. ✓ Bilateral trust updates immediately after negotiation
8. ✓ Global trust settles through CollaborationCollector with cap
9. ✓ AFS integration works (shared proposals, bilateral_trust proc)
10. ✓ Negotiation failure and abort scenarios handled gracefully

---

## 7. Implementation Roadmap

### Phase 1: Core Types (Week 1)
- [ ] Define PlanAmendment, AmendmentProposal, NegotiationSession
- [ ] Implement DirectedTrustMeter
- [ ] Implement CollaborationCollector with settlement cap
- [ ] Implement vote weight and consensus formulas
- [ ] Write unit tests for all core types

### Phase 2: Protocol Logic (Week 2)
- [ ] Implement proposal distribution via IPC
- [ ] Implement parallel evaluation (InitialResponse)
- [ ] Implement counter-proposal collection and ranking
- [ ] Implement final voting with weighted supermajority
- [ ] Implement state machine transitions and timeouts
- [ ] Write integration tests

### Phase 3: Integration & Validation (Week 3)
- [ ] Integrate with SovereigntyGate (Level 3 check, Constitution guard)
- [ ] Integrate with AFS (/shared/proposals, /proc bilateral_trust)
- [ ] Implement `experiments/negotiation_experiment.rs`
- [ ] Run software development team scenario
- [ ] Validate all 10 criteria
- [ ] Document results in `notes/05_negotiation-experiments.md`

---

## 8. Open Questions

| Question | Why it matters | Current leaning |
|----------|----------------|-----------------|
| Settlement interval? | How often to flush bilateral → global | **Every negotiation** (interval=1), simple and predictable |
| Backpressure limit? | Max concurrent negotiations per agent | **3** (configurable) |
| Evaluation timeout? | Prevents indefinite waiting | **30s** (configurable) |
| Negotiation history retention? | Enables learning and audit | **Store in AFS /shared/negotiations/** indefinitely |
| Proposer self-voting? | Should proposer vote on own proposal? | **Yes**, with same weight formula |
| Minimum affected agents? | Can you negotiate with yourself? | **No**, minimum 1 affected agent (besides proposer) |

---

## 9. References

- `rfc/001-agent-process.md` — Agent trait, Intent, ExecutionPlan (via IntentCore)
- `rfc/003-sovereignty-system.md` — TrustMeter, SovereigntyLevel, Constitution, SovereigntyGate
- `rfc/004-agent-file-system.md` — AFS, /shared directory, /proc virtual files, IPC channels
- **Literature review** (pending from deep-research workflow)

---

## 10. Design Decision Log

| Decision | Rationale | Alternatives considered |
|----------|-----------|------------------------|
| Amendment targets ExecutionPlan, not Constitution | Constitution is immutable per RFC-003; ExecutionPlan is mutable by design | Intent-level amendment (rejected: violates RFC-001 immutability) |
| Bilateral trust events feed global via capped collector | Links collaboration quality to sovereignty growth; cap prevents farming | Fully independent systems (rejected: no collaboration signal); direct bilateral→global (rejected: no cap, exploitable) |
| One round of counter-proposals | Balances expressiveness and complexity; prevents infinite negotiation loops | No counter-proposals (rejected: just voting, not negotiation); unlimited rounds (rejected: too complex, no termination guarantee) |
| Composite vote weight (0.6 bilateral + 0.4 global) | Bilateral trust dominates but global reputation prevents zero-weight for low-trust relationships | Pure bilateral (rejected: ignores global competence); pure global (rejected: ignores relationship quality) |
| Supermajority threshold 0.6 | Prevents tyranny of slim majority; allows minority dissent without blocking progress | Simple majority 0.5 (rejected: too easy to pass); unanimous (rejected: too easy to block) |
| Parallel evaluation | No ordering dependency between agents; faster negotiation | Sequential evaluation (rejected: slower, artificial ordering) |
| Snapshot trust at proposal creation | Prevents trust changes mid-negotiation from creating inconsistent outcomes | Live trust (rejected: non-deterministic, hard to audit) |

---

*End of RFC-005 (Draft)*
