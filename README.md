# Chora: OS-Inspired AI Agent Runtime

![Trust Evolution](https://i.imgur.com/chora-trust-metrics.svg)

> **"场域先于形式而存在，使形式得以生发"**  
> Chora (χώρα) - the primordial field/container that allows forms to emerge without prescribing them.

Chora is a research open-source project exploring the integration of operating system philosophy into AI agent design. The core idea: **the field precedes form** - Runtime as a "field" accommodating multi-sovereign Agent symbiosis, providing infrastructure for collaboration (IPC, AFS, negotiation protocols), but not dictating how Agents collaborate.

## � Core Innovations

| Feature | Description |
|---------|-------------|
| **Sovereign Agent Architecture** | Each agent has sequential sovereignty levels (Level 0-3) |
| **Trust Evolution System** | TrustMeter implements asymmetric mapping: `GoalProgress` → `SovereigntyLevel` |
| **OS-Inspired Design** | Process scheduling + capability model + kernel/user space separation |
| **Intent Negotiation Protocol** | Multi-agent intent negotiation with weighted voting |
| **Directed Trust Model** | Agent-specific trust relationships (A→B trust ≠ B→A trust) |

## 📚 RFC Status

| RFC | Status | Capability |
|-----|--------|------------|
| [RFC-001](rfc/001-agent-process.md) | ✅ Completed | Agent abstraction foundation |
| [RFC-002](rfc/002-scheduling.md) | ✅ Completed | Collaborative scheduling |
| [RFC-003](rfc/003-sovereignty-system.md) | ✅ Completed | Sovereignty system |
| [RFC-004](rfc/004-agent-file-system.md) | ✅ Completed | Agent File System (AFS) |
| [RFC-005](rfc/005-intent-negotiation-protocol.md) | ✅ Completed | Intent Negotiation Protocol |

## 🚀 Quick Start

### Prerequisites

```bash
# Rust 1.70+ required
rustc --version
```

### Clone and Build

```bash
git clone https://github.com/Frostsnow35/Chora.git
cd Chora
cargo build --release
```

### Run Experiments

```bash
# Personal Assistant Agent (sovereignty demo)
cargo run -p experiments --bin personal_assistant

# Sovereignty LLM Experiment (trust→LLM params mapping)
cargo run -p experiments --bin sovereignty_llm_experiment

# Scheduling Experiment (collaborative scheduling)
cargo run -p experiments --bin scheduling_experiment

# IPC Experiment (P2P, broadcast, backpressure)
cargo run -p experiments --bin ipc_experiment

# AFS Experiment (POSIX API, /proc, permissions)
cargo run -p experiments --bin afs_experiment

# Negotiation Experiment (human-AI collaboration demo)
cargo run -p experiments --bin negotiation_experiment
```

## 📖 API Documentation

### Creating a Runtime

```rust
use chora_runtime::{Runtime, FifoScheduler, NegotiationEngine};

// Create a runtime with FIFO scheduler
let scheduler = FifoScheduler::new();
let mut runtime = Runtime::new(Box::new(scheduler));

// Or with negotiation engine for multi-agent collaboration
let negotiation_engine = NegotiationEngine::new();
let mut runtime = Runtime::with_negotiation(
    Box::new(FifoScheduler::new()),
    negotiation_engine
);
```

### Creating a Sovereign Agent

```rust
use chora_runtime::{
    AgentId, Intent, GoalDescription,
    SovereignAgentImpl, TrustBehavior,
    ReasoningConfig, LinearMapping
};

let agent_id = AgentId::new();
let intent = Intent::new(
    GoalDescription::new("Complete project milestone"),
    vec!["deadline: 2026-07-01".to_string()]
);

let mut agent = SovereignAgentImpl::new(
    agent_id,
    intent,
    TrustBehavior::new(),
    ReasoningConfig::default()
);

// Agent can propose intent modifications at Level 3
if agent.trust_meter().sovereignty_level() >= SovereigntyLevel::Level3 {
    // Propose modifications via negotiation
}
```

### Negotiation Protocol

```rust
use chora_runtime::{
    negotiation::{NegotiationEngine, PlanAmendment},
    StepOutput, SessionId
};

let mut engine = NegotiationEngine::new();

// Level 3 agent creates a negotiation session
let session_id = engine.create_session(
    proposer_id,
    SovereigntyLevel::Level3,
    0.9,  // global trust
    target_agent_id,
    vec![affected_agent_1, affected_agent_2],
    PlanAmendment::ModifyGoal {
        new_goal: GoalDescription::new("Revised project scope")
    },
    "Market conditions changed, need to adjust scope".to_string()
).unwrap();

// Handle agent responses via IPC
let response = InitialResponse::Accept { rationale: "Agreed".to_string() };
engine.submit_response(session_id, affected_agent_1, response).unwrap();

// Final voting
engine.submit_vote(session_id, affected_agent_1, VoteDecision::Approve).unwrap();
engine.complete_session(session_id).unwrap();
```

### IPC Communication

```rust
use chora_runtime::ipc::{IpcBroker, ChannelType, NegotiationMessage};

let mut broker = IpcBroker::new();

// Create broadcast channel for negotiation
let channel_id = broker.create_channel(
    ChannelType::Broadcast,
    vec![agent_1_id, agent_2_id, agent_3_id],
    100
).unwrap();

// Send negotiation message
let msg = NegotiationMessage::ProposalBroadcast {
    session_id,
    proposal: /* ... */
};
broker.send(channel_id, serde_json::to_string(&msg).unwrap()).unwrap();
```

## 🤝 Human-AI Collaboration

Chora enables seamless human-AI collaboration through the Intent Negotiation Protocol:

```rust
// Human proposes a change, AI agents evaluate and vote
let human_proposal = PlanAmendment::ModifyGoal {
    new_goal: GoalDescription::new("Customer request: add dark mode")
};

// AI agents respond with Accept/Reject/CounterProposal
// Weighted voting: 0.6×directed trust + 0.4×global trust
// Supermajority (≥60%) required for approval
```

**Scenario**: A product manager (human) proposes adding dark mode. The architect agent reviews technical feasibility, the implementer agent assesses effort, and the tester agent evaluates test coverage. Through negotiation, they reach consensus on the implementation plan.

## � Agent Sovereignty Levels

| Level | Trust Score | Capabilities |
|-------|-------------|--------------|
| **Level 0** | 0.0-0.59 | Fully controlled, no autonomy |
| **Level 1** | 0.6-0.74 | Can reject out-of-bounds requests |
| **Level 2** | 0.75-0.89 | Can autonomously terminate tasks |
| **Level 3** | 0.9-1.0 | Can propose intent modifications |

## �️ Architecture

### Kernel Space / User Space Separation

```
┌─────────────────────────────────────────────────────────┐
│                   User Space                            │
│  ┌──────────────────┐  ┌──────────────────┐            │
│  │ ReasoningEngine  │  │ ToolDispatcher   │            │
│  │ (≥20ms)          │  │ (network calls)  │            │
│  └──────────────────┘  └──────────────────┘            │
├─────────────────────────────────────────────────────────┤
│                   Kernel Space                          │
│  ┌──────────┐ ┌───────────────┐ ┌──────────┐           │
│  │TrustMeter│ │SovereigntyGate│ │IntentCore│           │
│  │ (≤0.5ms) │ │  (≤0.5ms)     │ │(≤0.5ms)  │           │
│  └──────────┘ └───────────────┘ └──────────┘           │
├─────────────────────────────────────────────────────────┤
│                   Runtime                               │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │
│  │ Scheduler    │ │ IPC Subsystem│ │ AFS          │    │
│  │ (O(1) hot    │ │ (P2P/Broadcast│ │ (/proc,      │    │
│  │  path)       │ │  Backpressure)│ │  /shared)    │    │
│  └──────────────┘ └──────────────┘ └──────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## 📁 Project Structure

```
Chora/
├── runtime/          # Core runtime library
│   ├── src/
│   │   ├── sovereignty/   # TrustMeter, SovereigntyGate
│   │   ├── scheduling/    # Scheduler, Runtime
│   │   ├── ipc/           # Inter-process communication
│   │   ├── fs/            # Agent File System
│   │   └── negotiation/   # Intent Negotiation Protocol
│   └── examples/          # Usage examples
├── memory/           # Memory subsystem
├── experiments/      # Verification agents
└── rfc/              # Design documents
```

## 🔧 Running Tests

```bash
# All tests
cargo test --workspace

# Specific module tests
cargo test -p runtime --lib sovereignty
cargo test -p runtime --lib scheduling
cargo test -p runtime --lib negotiation

# Integration tests
cargo test -p runtime --test sovereignty_integration_tests
```

## 📝 Contributing

### Development Flow

1. **Research Phase**: Document design decisions in RFCs
2. **Implementation**: TDD + subagent-driven development
3. **Verification**: Run experiments and integration tests
4. **Review**: Double review (implementer + reviewer)

### Performance Constraints

- **O(1) hot path**: `next_to_run()`, `on_ready()`, `on_block()` must be O(1)
- **No heap allocations**: Kernel space operations must not allocate
- **History window**: 50 events maximum

### Code Style

```bash
cargo fmt        # Format code
cargo clippy     # Lint code
```

## 📜 License

MIT License

## 📧 Contact

For questions, suggestions, or collaboration:
- GitHub: https://github.com/Frostsnow35/Chora
- Issues: https://github.com/Frostsnow35/Chora/issues

---

> Chora 之名源自希腊哲学中的 χώρα（khôra），意为"原始场域/容器"——  
> 场域先于任何具体形式而存在，使形式得以生发，但不规定形式本身。  
> Runtime 作为"场域"容纳多主权 Agent 共生，让协作从协商中自然涌现。