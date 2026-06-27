# Project Status: Chora Agent Runtime (2026-06-25)

## Executive Summary

Chora is a research project exploring **agent systems through the lens of operating system design**. The project's core thesis is that OS concepts — process, scheduler, memory hierarchy, IPC, failure isolation — offer a productive framework for understanding and building agent runtimes.

**Current State** (as of 2026-06-25):

| Area | Status | Files |
|---|---|---|
| Design Philosophy | ✅ Complete | `notes/01_os-agent-mapping.md` |
| Agent Abstraction | ✅ Complete | `rfc/001-agent-process.md` |
| Scheduling Design | ✅ Complete | `rfc/002-scheduling.md`, `rfc/002b-scheduler-interface.md` |
| Test Planning | ✅ Complete | `tests/01_scheduler_policy_test_plan.md`, `tests/002a-scheduling-test-harness-implementation.md` |
| Runtime Implementation | ⚠️ In Progress | `runtime/src/` (7 modules + modifications) |
| Memory Crate | ⏳ Pending | `memory/Cargo.toml` (empty) |
| Experiments | ⏳ Pending | `experiments/` directory |

---

## Key Decisions Made

### 1. Scheduler = External Policy (P1: Mechanism/Policy)

**Decision**: The scheduler is not hardcoded in the runtime crate. It is a **pluggable policy** implemented by external code.

**Rationale**: 
- Different workloads require different scheduling strategies (FIFO vs token-budget-priority vs deadline-aware).
- Hardcoding any strategy would violate P1 (mechanism/policy separation).

**Implementation**: `Scheduler` trait with three built-in implementations planned:
- `FifoScheduler`
- `TokenBudgetPriorityScheduler`
- `DeadlineAwareScheduler`

### 2. Cooperative Yielding (Not Preemptive)

**Decision**: Agents must explicitly yield at clean boundaries. The runtime cannot preempt an agent mid-step.

**Rationale**:
- LLM inference cannot be safely interrupted (undefined state if stopped mid-generation).
- Agents are naturally I/O-bound (waiting for messages, tools, user input), so preemption is unnecessary.

**Implementation**: `Agent::yield_now()` method; runtime transitions yielding agents to `Suspended` state.

### 3. Supervisor as Plain Agent (T5: Simplicity)

**Decision**: A supervisor is just another agent whose intent includes monitoring and restarting other agents. No separate abstraction.

**Rationale**:
- Adding a `Supervisor` type violates T5 (unnecessary entity multiplication).
- Supervision behavior is expressible via existing primitives (`spawn`, `terminate`, `receive`).

### 4. Three-Layer Program Model (Model/Prompt/Tools)

**Decision**: An agent's "program" is decomposed into:
1. **Model**: The reasoning substrate (e.g., GPT-4o, Claude-3)
2. **Prompt**: Behavior-shaping instructions and persona
3. **Tools**: Available capabilities for interacting with the world

**Rationale**:
- Enables independent swapping (change prompt without changing model, grant new tools without rewriting prompts).
- Aligns with OS separation of "binary" from "environment variables."

---

## Current Blockers / Unknowns

| Issue | Status | Next Action |
|---|---|---|
| Rust toolchain | ❌ Not available in current environment | Run `cargo check` locally |
| Missing types in lib.rs | ⚠️ Some imports may fail | Verify `tool.rs`, `InterruptReason`, `StepMetrics` exist |
| `process_table` module | ⏳ Not implemented | Create `runtime/src/process_table.rs` |
| `EventLoop` module | ⏳ Not implemented | Create `runtime/src/event_loop.rs` |
| Memory crate interface | ⏳ TBD | Define `memory/` crate API in RFC-003 |
| Three validation agents | ⏳ TBD | Implement in `experiments/` per RFC-001 §8 |

---

## Recommended Next Steps

### Immediate (Week 1)
1. **Fix compilation errors**: Run `cargo check -p runtime` and resolve all warnings/errors.
2. **Implement minimal `ProcessTable`**: HashMap-backed store for `AgentRecord`.
3. **Add event loop stub**: Basic async loop that dispatches events to agents.

### Short-term (Week 2-3)
4. **Implement `FifoScheduler`** and write basic unit tests.
5. **Create `memory/` crate skeleton**: Define working/episodic/semantic tier interfaces.
6. **Start `experiments/calculator.rs`**: First validation agent (ToolUsing calculator).

### Medium-term (Week 4-6)
7. **Implement `TokenBudgetPriorityScheduler`** and test against FIFO.
8. **Complete all three validation agents** from RFC-001.
9. **Run comparative benchmarks**: FIFO vs TokenBudget under various load patterns.

---

## References

| Document | Purpose | Location |
|---|---|---|
| Project Manifest | Mission, metaphor, research questions | `architecture/00_MANIFEST.md` |
| OS→Agent Mapping | Six principles, eight tensions | `notes/01_os-agent-mapping.md` |
| Agent Abstraction RFC | Trait surface, intent, program, record | `rfc/001-agent-process.md` |
| Scheduling RFC | Scheduler design rationale | `rfc/002-scheduling.md` |
| Scheduler Interface | Concrete APIs and examples | `rfc/002b-scheduler-interface.md` |
| Test Plan | Scheduler policies and pass criteria | `tests/01_scheduler_policy_test_plan.md` |
| Harness Blueprint | Test implementation blueprint | `tests/002a-scheduling-test-harness-implementation.md` |
