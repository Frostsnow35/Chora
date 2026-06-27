# RFC-002 — Scheduling for the Execution Field

| Field | Value |
|---|---|
| Status | Draft |
| Author | — |
| Created | 2026-06-25 |
| Depends on | `architecture/00_MANIFEST.md`, `notes/01_os-agent-mapping.md`, `rfc/001-agent-process.md` |

---

## Abstract

This RFC defines the **scheduling subsystem** of the execution field. The goal is to implement a scheduler that:

1. **Fits the agent model** (cooperative, event-driven, not preemptive),
2. **Respects the principles** of P1 (mechanism/policy separation) and P3 (orthogonality),
3. **Solves the 23-196342 problem** (Q2: how do we schedule agents when the "CPU time" is LLM inference budget),
4. **Exposes pluggable policies** (e.g., FIFO, token-budget-priority) while keeping the core runtime dumb.

The scheduler is *not* part of the `Agent` trait. It is a **separate component** that interacts with the runtime's process table through well-defined mechanisms.

---

## 1. Grounding

This RFC builds on:

- **`rfc/001`** — the agent abstraction: `step()`, `yield_now()`, `resume()`.
- **`notes/01`** — design principles P1 (mechanism/policy), P3 (orthogonality), and tensions T2 (fairness ↔ efficiency), T3 (runtime power ↔ agent autonomy).
- **`00_MANIFEST.md`** — the research question Q2: "When multiple agents are runnable, who goes next, and by what rule?"

The *key insight* from Q2's reliability assessment (01-os-agent-mapping) is that **preemptive scheduling does not apply** to agents (unlike OSes). The runtime must work with **cooperative, event-driven agents**.

---

## 2. The Scheduling Problem

### Why the OS model is wrong here

| OS Scheduling | Agent Scheduling |
|---|---|
| **Resource**: CPU time | **Resource**: LLM inference budget (tokens + wall-clock) |
| **Unit**: ~100μs | **Unit**: 100ms–10s (a full LLM inference) |
| **Preemption**: Hardware timer interrupt | **Preemption**: *impossible* — mid-inference state is undefined |
| **Workload**: CPU-bound | **Workload**: *I/O-bound* — waiting for messages, tool results, user input |
| **Fairness**: time-slices | **Fairness**: token budget, step count, wall-clock |
| **Priority**: static policy | **Priority**: dynamic (e.g., by intent constraints) |

### The new problem: how do we *not* pre-empt?

Since we cannot interrupt an agent mid-step, we **rely on agents to yield explicitly** (via `yield_now()`) at clean points (end of a tool call, end of a message, when waiting for external input). The scheduler's job is to:

- **Know when agents can run**: they have `state = Ready`.
- **Know when agents must wait**: they are `Blocked` or `Suspended`.
- **Choose who runs next** using a pluggable policy (FIFO, token-budget-priority, etc.).

This is the *core of the scheduling problem*.

---

## 3. The Scheduler API

### 3.1 `Scheduler` Trait

The scheduler is **not** part of the runtime core. It is a **pluggable policy** that implements the following trait:

```rust
pub trait Scheduler {
    /// Get the next agent to run, if any.
    fn next_to_run(&mut self) -> Option<AgentId>;

    /// Signal that an agent has yielded.
    fn on_yield(&mut self, agent_id: AgentId);

    /// Signal that an agent has been blocked.
    fn on_block(&mut self, agent_id: AgentId, reason: BlockReason);

    /// Signal that an agent has become ready.
    fn on_ready(&mut self, agent_id: AgentId);

    /// Signal that an agent has terminated.
    fn on_terminate(&mut self, agent_id: AgentId, reason: TerminationReason);

    /// Optional: handle external event (e.g., timer firing).
    fn on_event(&mut self, event: SchedulingEvent);

    /// Update scheduling context for an agent (e.g., priority).
    fn update_agent_context(&mut self, agent_id: AgentId, context: SchedulingContext);
}
```

#### Key design decisions

- **P1 (mechanism/policy)**: The trait only exposes *mechanisms* (what to do, not what to do it). The *who* is implemented by the policy.
- **T3 (runtime power ↔ agent autonomy)**: The scheduler *only* decides *who* runs next, **not** what the agent does in its turn.
- **T2 (fairness ↔ efficiency)**: The scheduler policy is *not* part of the `runtime` crate. It lives in a `scheduling` crate that the user can swap.

### 3.2 `SchedulingContext` (from `001`)

Each agent has a `SchedulingContext` (a `SchedulingContext` object) in its `AgentRecord` (see `001`).

```rust
pub struct SchedulingContext {
    pub priority: i32,
    pub quantum_used: u64,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
}
```

- **priority**: The scheduler can update this. A higher number means higher priority.
- **quantum_used**: Used only for *time-sliced* policies (not recommended for agents).
- **last_run**: Last time the agent ran.

The scheduler uses these values, but **does not own** them. The policy is **free to use any of these values** to make its decision.

### 3.3 Example policies

1. **FIFO**:
   - The scheduler is a simple queue.
   - `on_ready()` adds the agent to the end.
   - `next_to_run()` removes the first agent.

2. **Token-Budget Priority**:
   - Agents with more available token budget are prioritized.
   - This requires the scheduler to know about token budgets (from `AgentRecord`).

3. **Deadline-Aware**:
   - Prioritize agents that have the most urgent deadlines.

### 3.4 The scheduler's relationship with the runtime

The scheduler is **completely external** to the `runtime` crate. The only interaction is:

- The `runtime` calls `next_to_run()` on the scheduler.
- The `runtime` calls the appropriate handler (`on_yield()`, `on_block()`, etc.) when an agent's state changes.

The scheduler **does not** access the agent's state or context. It only uses `AgentId` (and `SchedulingContext`) to make its choice.

---

## 4. The Runtime's Scheduling Logic

### 4.1 The scheduler in the runtime

The `runtime` crate contains *one* scheduler instance:

```rust
pub struct Runtime {
    // ...
    scheduler: Box<dyn Scheduler>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            scheduler: Box::new(FifoScheduler::new()),
            // ...
        }
    }

    /// Get the next agent to run.
    pub fn next_to_run(&mut self) -> Option<AgentId> {
        self.scheduler.next_to_run()
    }

    /// Signal a state change (e.g., an agent yielded).
    pub fn on_agent_state_change(&mut self, agent_id: AgentId, new_state: SchedulingState) {
        // Update the process table
        self.process_table.update_state(agent_id, new_state);

        // Call the scheduler
        match new_state {
            SchedulingState::Ready => self.scheduler.on_ready(agent_id),
            SchedulingState::Blocked { .. } => self.scheduler.on_block(agent_id, BlockReason::Other("")),
            SchedulingState::Suspended => self.scheduler.on_suspended(agent_id),
            // ...
        }
    }
}
```

### 4.2 The main execution loop (simplified)

1. **Check if any agent is ready to run**.
2. **Get the next agent to run** from the scheduler.
3. **Run that agent's `step()`**.
4. **If the agent yields or terminates**, update the scheduler (via `on_yield()` or `on_terminate()`).
5. **If a timeout or event occurs**, call `on_event()`.

This is **as simple as possible**. All the complexity is in the scheduler policy.

---

## 5. Testing the Scheduling Policy

We will validate this RFC using the mock agent from `001`.

### 5.1 The `SchedulingTest` harness

```rust
struct SchedulingTest {
    runtime: Runtime,
    // A set of test agents (10–20)
    agents: Vec<MockAgent>,
    // The current state of the process table
    process_table: ProcessTable,
}

impl SchedulingTest {
    pub fn new() -> Self {
        // ...
    }

    pub fn run_test(&mut self, scheduler: Box<dyn Scheduler>) {
        // Initialize agents and scheduler
        self.runtime = Runtime::new(scheduler);
        self.process_table = ProcessTable::new();

        for agent in &self.agents {
            // Spawn the agent
            let id = agent.id();
            self.process_table.spawn(id, agent.clone());
        }

        // Run the scheduler
        while self.runtime.has_ready_agents() {
            let next = self.runtime.next_to_run();
            match next {
                Some(agent_id) => {
                    // Run the agent's step
                    let result = self.runtime.run_step(agent_id);
                    // Handle result (e.g., if it yielded)
                    self.runtime.on_agent_state_change(agent_id, result.state);
                }
                None => break,
            }
        }
    }
}
```

### 5.2 Test cases

| Test | Purpose |
|---|---|
| **1. Basic FIFO** | The scheduler correctly runs agents in the order they were spawned. |
| **2. Priority (1)** | An agent with `priority=100` runs before an agent with `priority=1`. |
| **3. Priority (2)** | A lower-priority agent runs first if it has been waiting longer. |
| **4. Deadline-Aware** | An agent with a tight deadline runs before one with a looser deadline. |
| **5. Token Budget** | An agent with more tokens remaining runs before one with fewer. |
| **6. I/O Bound** | An agent that yields frequently runs more often than one that rarely yields. |

---

## 6. Open Questions

| Question | Why it matters |
|---|---|
| **1. How to handle deadlocks?** | What if two agents are waiting for each other? |
| **2. How to implement fair share?** | Should we track tokens used by agent (100 tokens each) or by intent (1000 tokens total)? |
| **3. How to handle the "run to completion" pattern?** | Some agents (e.g., a long-running analysis task) may never yield. |
| **4. How to handle external events (e.g., a timer)?** | How does the scheduler know a timer fired? |
| **5. How to handle supervisor policies?** | Should supervisors get special scheduling treatment? |
| **6. How to handle "hijacking"?** | What if an agent yields but never resumes? |

---

## 7. Next Steps

1. **Implement `FifoScheduler`** as a proof-of-concept.
2. **Write unit tests** using the test harness above.
3. **Implement a token-budget priority scheduler**.
4. **Run the three validation agents from RFC-001** and observe scheduling behavior.
5. **Document the results** in a new RFC: `002b-scheduling-policy-evaluation`.

---

## 8. References

- `rfc/001-agent-process.md` — the agent abstraction, `step()`, `yield_now()`, `SchedulingState`.
- `notes/01_os-agent-mapping.md` — principles P1, P3, and tensions T2, T3.
- `00_MANIFEST.md` — Q2: "When multiple agents are runnable, who goes next, and by what rule?".
