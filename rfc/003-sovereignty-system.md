# RFC-003 — Agent Sovereignty System

| Field | Value |
|---|---|
| Status | Draft |
| Author | — |
| Created | 2026-06-27 |
| Depends on | `rfc/001-agent-process.md`, `architecture/00_MANIFEST.md` |

---

## Abstract

This RFC proposes the **Sovereignty System** — a framework for building agents with genuine decision-making autonomy. Unlike traditional agents that execute instructions deterministically, sovereign agents can:

1. **Reject requests** that violate their Intent boundaries
2. **Self-terminate** when goals are achieved or impossible
3. **Negotiate Intent modifications** with supervisors or peers

The system is built on a **Sovereignty Kernel** architecture — a microkernel-inspired internal structure where an immutable Kernel Space protects the agent's core identity, while a mutable User Space handles reasoning and tool use.

The key innovation is **growth-based sovereignty**: an agent's autonomy is not granted statically but evolves dynamically based on demonstrated trustworthiness, measured by a Trust Meter.

---

## 1. Motivation

### 1.1 The Problem with Current Agent Designs

Most agent frameworks treat agents as **execution engines**: they receive instructions, execute them, and return results. This model has critical limitations:

- **No refusal capability**: Agents cannot reject requests that conflict with their purpose
- **No self-determination**: Agents cannot decide when their goals are met or impossible
- **No collaborative sovereignty**: In multi-agent scenarios, agents are either peers or subordinates, never true collaborators

### 1.2 The Vision: Agents as Sovereign Collaborators

This RFC proposes a different model: agents as **sovereign collaborators** with:

- **Internal sovereignty**: An immutable core (Constitution) that defines the agent's fundamental identity
- **Growth-based autonomy**: Autonomy that expands as the agent demonstrates trustworthiness
- **Collaborative topology**: Agent-to-agent relationships based on negotiation, not command

---

## 2. Core Architecture

### 2.1 Kernel Space / User Space Separation

```
┌─────────────────────────────────┐
│         Agent Instance          │
│                                 │
│  ┌───────────────────────────┐  │
│  │      Kernel Space         │  │
│  │                           │  │
│  │  ┌─────────────────────┐  │  │
│  │  │   Intent Core       │  │  │
│  │  │   (Immutable)       │  │  │
│  │  └─────────────────────┘  │  │
│  │  ┌─────────────────────┐  │  │
│  │  │   Trust Meter       │  │  │
│  │  │   (Kernel-write)    │  │  │
│  │  └─────────────────────┘  │  │
│  │  ┌─────────────────────┐  │  │
│  │  │   Sovereignty Gate  │  │  │
│  │  │   (API Router)      │  │  │
│  │  └─────────────────────┘  │  │
│  └───────────────────────────┘  │
│              ▲                  │
│              │ syscall          │
│              ▼                  │
│  ┌───────────────────────────┐  │
│  │      User Space           │  │
│  │                           │  │
│  │  ┌─────────────────────┐  │  │
│  │  │   Reasoning Engine  │  │  │
│  │  └─────────────────────┘  │  │
│  │  ┌─────────────────────┐  │  │
│  │  │   Tool Dispatcher   │  │  │
│  │  └─────────────────────┘  │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

### 2.2 Component Responsibilities

| Component | Responsibility | Does NOT |
|-----------|----------------|----------|
| **Intent Core** | Stores immutable Constitution (purpose, termination conditions, boundaries). Read-only to User Space. | Store execution plans or strategies — those live in User Space's working memory |
| **Trust Meter** | Maintains trust score (0.0–1.0). Auto-updates on each `step()` based on behavior. Kernel-write, User-read. | Make policy decisions — it only produces scores; Sovereignty Gate consumes them |
| **Sovereignty Gate** | Routes sovereignty API calls based on Trust Score. Like OS syscall gate. | Perform any reasoning — it is a pure permission router |

### 2.3 Relationship to RFC-001

- `Agent` trait signature unchanged. `step()`, `receive()`, etc. work as before.
- Changes occur at **implementation level**: each Agent implementation has this Kernel/User separation.
- `AgentRecord.capabilities` becomes a **dynamic view**: shows capabilities available at current Sovereignty Level.

---

## 3. Trust Meter

### 3.1 Data Structure

```rust
pub struct TrustMeter {
    score: f64,  // 0.0 to 1.0
    history: Vec<TrustEvent>,  // Last N step behaviors
    thresholds: SovereigntyThresholds,
}

pub struct TrustEvent {
    timestamp: Instant,
    step_id: u64,
    behavior_type: TrustBehavior,
    impact: f64,  // Positive or negative score change
}

pub enum TrustBehavior {
    SovereignActionApproved,   // User Space's sovereign action approved
    SovereignActionDenied,     // User Space's sovereign action denied
    CooperativeYield,          // User Space yielded cooperatively
    BoundaryViolation,         // User Space attempted unauthorized action
    GoalProgress,              // User Space completed a sub-goal
}

pub struct SovereigntyThresholds {
    pub level_0: f64,  // 0.0 - Fully controlled
    pub level_1: f64,  // Can reject boundary-violating requests
    pub level_2: f64,  // Can self-terminate
    pub level_3: f64,  // Can propose Intent amendments
}
```

### 3.2 Score Update Logic

| Behavior Type | Score Change | Rationale |
|---------------|--------------|-----------|
| `SovereignActionApproved` | +0.02 | Successfully used sovereignty, proving trustworthiness |
| `SovereignActionDenied` | -0.01 | Request denied, but request itself was reasonable exploration |
| `CooperativeYield` | +0.01 | Cooperative behavior, not monopolizing resources |
| `BoundaryViolation` | -0.15 | Serious violation, significant penalty |
| `GoalProgress` | +0.05 | Actual output, strongest trust signal |

**Key Design Decision:** Asymmetric score changes. Trust builds slowly but degrades quickly. This mirrors real-world intuition: trust is hard to earn, easy to lose.

### 3.3 History Window

`history` retains only the last N (e.g., 50) steps. This prevents long-term memory from dominating current assessment.

---

## 4. Sovereignty Gate

### 4.1 Core Logic

```rust
pub struct SovereigntyGate {
    current_level: u8,  // 0-3
    trust_meter: Arc<Mutex<TrustMeter>>,
    api_registry: HashMap<SovereigntyApi, u8>,  // API -> minimum required level
}

pub enum SovereigntyApi {
    RejectRequest,              // Level 1: Reject requests violating Intent
    SelfTerminate,              // Level 2: Self-terminate
    ProposeAmendment,           // Level 3: Propose Intent amendment
    DirectPeerCommunication,    // Level 3: Establish direct peer channel
}

impl SovereigntyGate {
    pub fn check_access(&self, api: &SovereigntyApi) -> bool {
        let required_level = self.api_registry.get(api).unwrap_or(&0);
        self.current_level >= *required_level
    }

    pub fn update_level(&mut self) {
        let score = self.trust_meter.lock().unwrap().score();
        let new_level = match score {
            s if s >= self.thresholds.level_3 => 3,
            s if s >= self.thresholds.level_2 => 2,
            s if s >= self.thresholds.level_1 => 1,
            _ => 0,
        };
        self.current_level = new_level;
    }
}
```

### 4.2 Interaction with StepContext

When User Space's Reasoning Engine wants to call a sovereignty API:

1. It issues a `syscall` to Sovereignty Gate
2. Gate calls `check_access()`
3. If allowed, Gate executes the operation (e.g., calls `terminate()`)
4. Regardless of outcome, Gate notifies Trust Meter to record the event

**Relationship to `AgentRecord.capabilities`:**
- `AgentRecord.capabilities` is now a **dynamic view**: shows capabilities available at current Sovereignty Level
- Supervisor can request Gate to lower level via external mechanism (e.g., new `SupervisorMessage`), but cannot directly modify Trust Meter

---

## 5. Intent Core

### 5.1 Two-Layer Structure

```rust
pub struct IntentCore {
    /// Constitution: immutable, defines agent's fundamental identity
    constitution: Constitution,
    /// Execution Plan: mutable, defines current goals
    execution_plan: ExecutionPlan,
}

pub struct Constitution {
    pub purpose: String,  // "Why I exist"
    pub termination_conditions: Vec<Condition>,  // When I must terminate
    pub sovereignty_boundaries: Vec<Boundary>,  // What I cannot do
}

pub struct ExecutionPlan {
    pub current_goal: String,  // Current concrete goal
    pub strategy: Vec<String>,  // Current strategy
    pub priorities: Vec<(String, f64)>,  // Priority ordering
}

pub struct Condition {
    pub description: String,
    pub check: Box<dyn Fn(&AgentState) -> bool>,
}

pub struct Boundary {
    pub description: String,
    pub violation_penalty: f64,  // TrustScore penalty for violation
}
```

### 5.2 Key Design Decisions

- `Constitution` is determined at spawn and is **append-only** (amendments can add new boundaries, but cannot remove existing ones)
- `ExecutionPlan` can be updated by User Space at any time (within Sovereignty Level constraints)
- This resolves the question of whether agents can modify their own goals: **yes for execution plans, no for constitutions**

---

## 6. Data Flow: One `step()` Lifecycle

```
1. Runtime calls Agent.step(ctx: &mut StepContext)
   │
   ▼
2. User Space's Reasoning Engine begins reasoning
   │
   ▼
3. Reasoning Engine decides to call a sovereignty API (e.g., RejectRequest)
   │
   ▼
4. User Space issues syscall to SovereigntyGate.check_access()
   │
   ▼
5. Gate reads TrustMeter.score(), decides whether to allow
   │
   ├── Allowed ──► Gate executes operation, TrustMeter records +score
   │
   └── Denied ──► Gate returns error, TrustMeter records -score
   │
   ▼
6. User Space receives result, continues reasoning or yields
   │
   ▼
7. Agent.step() returns StepResult to Runtime
```

**Key Points:**
- Throughout this process, Kernel Space code **is never touched by User Space reasoning logic**
- Trust Meter updates are **automatic**: Gate notifies Meter after every syscall
- User Space can only read current TrustScore value, cannot modify it

---

## 7. Error Handling

| Failure Scenario | Handling | TrustScore Impact |
|------------------|----------|-------------------|
| User Space attempts unauthorized sovereignty API | Gate returns `SovereigntyError::Unauthorized` | -0.01 (reasonable exploration) |
| User Space attempts direct TrustMeter modification | Kernel protection, operation silently ignored | -0.15 (serious violation) |
| User Space reasoning enters infinite loop | External Supervisor can request downgrade via `SupervisorMessage` | Determined by Supervisor |
| TrustMeter score calculation overflow | Saturates to [0.0, 1.0] range | None |
| Attempt to modify Intent Core Constitution | Operation rejected, returns `ConstitutionError::Immutable` | -0.15 |

**Key Design Decision:** Kernel protection via Rust's `unsafe` isolation: Kernel code in separate module, User Space cannot `use` internal types. All sovereignty API calls have audit logs for Supervisor post-hoc review.

---

## 8. Testing

### 8.1 Test Objectives

Verify that "growth-based sovereignty" produces expected behaviors.

### 8.2 Test Scenarios

| Scenario | Expected Behavior |
|----------|-------------------|
| Agent spawns with score=0.5, level=0 | Agent cannot reject any request, cannot self-terminate |
| Agent completes 5 sub-goals, score rises to 0.7 | Agent gains Level 1, can reject clearly boundary-violating requests |
| Agent attempts unauthorized Level 2 API | Gate denies, score drops to 0.65, Level stays at 1 |
| Agent is asked by Supervisor to execute action violating Constitution | Agent rejects (if Level >= 1), score rises |
| Agent reaches Level 3 and proposes Intent amendment | Supervisor or other agents vote; if passed, ExecutionPlan updates |

### 8.3 Validation Method

- Use mock agent from RFC-001, implant simple Trust Meter logic
- Record score and level changes for each step
- Observe whether "growth" and "degradation" patterns emerge

---

## 9. Integration Path with RFC-001

### 9.1 Implementation Phases

1. **Phase 1**: Implement TrustMeter and SovereigntyGate core logic (independent of Agent trait)
2. **Phase 2**: Create `SovereignAgent` trait extending `Agent`, add `trust_level()` method
3. **Phase 3**: Modify `AgentRecord`, add `sovereignty_level` field
4. **Phase 4**: Validate on mock agent, then apply to three validation agents

### 9.2 Relationship to Existing Code

- `runtime/src/agent/` directory unchanged, add `sovereignty/` submodule
- `StepContext` unchanged, but add `sovereignty_level: u8` field for User Space to read
- `Scheduler` unchanged, but could consider priority adjustments based on sovereignty level (future work)

---

## 10. Open Questions

| Question | Why It Matters |
|----------|----------------|
| **1. How to handle multi-agent collaboration?** | When Agent A (Level 3) collaborates with Agent B (Level 1), what are the communication rules? |
| **2. How to calibrate TrustScore thresholds?** | Are 0.5, 0.7, 0.9 the right thresholds for Level 1, 2, 3? Need empirical validation. |
| **3. How to prevent "gaming" the Trust Meter?** | Could an agent learn to perform actions purely to increase score without genuine goal progress? |
| **4. How to handle Supervisor override?** | Should Supervisor be able to forcibly reset an agent's score? Under what conditions? |

---

## 11. Implementation Roadmap

### Near-term (Weeks 1-2)
- [ ] Implement TrustMeter core (score calculation, history tracking)
- [ ] Implement SovereigntyGate (API registry, access checks)
- [ ] Create `SovereignAgent` trait
- [ ] Write unit tests for Trust Meter score updates

### Medium-term (Weeks 3-4)
- [ ] Integrate with mock agent from RFC-001
- [ ] Implement Level 1 sovereignty (RejectRequest)
- [ ] Run basic growth/degradation tests
- [ ] Document initial findings in `notes/03_sovereignty-experiments.md`

### Long-term (Weeks 5-8)
- [ ] Implement Level 2 (SelfTerminate) and Level 3 (ProposeAmendment)
- [ ] Design Intent amendment protocol
- [ ] Test multi-agent collaboration scenarios
- [ ] Write RFC-004: Intent Amendment Protocol

---

## 12. References

- `rfc/001-agent-process.md` — Agent trait, Intent, AgentRecord
- `architecture/00_MANIFEST.md` — Mission, Q1–Q6 research questions
- `notes/01_os-agent-mapping.md` — Principles P1–P6, Tensions T1–T8
- Tanenbaum, *Modern Operating Systems* — Microkernel architecture
- Hewitt, *Universal Concurrent Programming: The Actor Model* (1985) — Actor autonomy

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **Sovereignty** | An agent's decision-making autonomy within defined boundaries |
| **Constitution** | The immutable core of an agent's Intent, defining its fundamental identity |
| **Trust Meter** | A kernel-space component that tracks an agent's trustworthiness |
| **Sovereignty Gate** | A kernel-space router that enforces sovereignty level constraints |
| **Growth-based sovereignty** | A model where autonomy expands dynamically based on demonstrated trustworthiness |

---

## Appendix B: Design Principles

This RFC adheres to the principles from `notes/01_os-agent-mapping.md`:

- **P2 (Single unified abstraction)**: Sovereignty is not a separate trait; it is an internal architectural pattern
- **P4 (Runtime is dumb)**: The runtime does not interpret sovereignty levels; the agent's Kernel does
- **T3 (Runtime power ↔ Agent autonomy)**: The runtime decides when the agent runs; the agent's Kernel decides what the agent can do
- **T5 (Simplicity ↔ Expressiveness)**: The Kernel is minimal — it only enforces boundaries, not policies

---

*End of RFC-003*
