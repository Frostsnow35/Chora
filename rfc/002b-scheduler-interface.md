# Scheduler Interface Specification

> This document defines the concrete interfaces for the scheduler subsystem. It serves as both a design specification and an implementation blueprint. All types are intended to be stable — changes require RFC update.

## 1. Core Types

### 1.1 `Scheduler` Trait

```rust
/// The scheduler trait defines the interface between the runtime and scheduling policies.
///
/// ## Design Principles
/// - P1 (Mechanism/Policy): This trait exposes only mechanisms, not policies.
/// - T2 (Fairness/Efficiency): Different implementations can optimize for different goals.
/// - T3 (Runtime Power/Agent Autonomy): The scheduler only decides "who", not "what".
pub trait Scheduler: Send + Sync {
    /// Get the next agent ID to run. Returns None if no agents are ready.
    fn next_to_run(&mut self) -> Option<AgentId>;

    /// Called when an agent yields voluntarily.
    fn on_yield(&mut self, agent_id: AgentId);

    /// Called when an agent becomes blocked.
    fn on_block(&mut self, agent_id: AgentId, reason: BlockReason);

    /// Called when an agent becomes ready to run.
    fn on_ready(&mut self, agent_id: AgentId);

    /// Called when an agent is suspended.
    fn on_suspend(&mut self, agent_id: AgentId);

    /// Called when an agent resumes from suspension.
    fn on_resume(&mut self, agent_id: AgentId);

    /// Called when an agent terminates.
    fn on_terminate(&mut self, agent_id: AgentId, reason: TerminationReason);

    /// Called when an external event occurs (timer, user input, etc.).
    fn on_event(&mut self, event: SchedulingEvent);

    /// Update an agent's scheduling context.
    fn update_context(&mut self, agent_id: AgentId, context: SchedulingContextUpdate);

    /// Return true if any agents are in the scheduler's queue.
    fn has_ready_agents(&self) -> bool;

    /// Return the total number of runnable agents.
    fn ready_count(&self) -> usize;
}
```

### 1.2 `SchedulingEvent`

```rust
/// Represents an external event that may affect scheduling decisions.
#[derive(Debug, Clone)]
pub enum SchedulingEvent {
    /// A timer has fired for a specific agent.
    TimerFired { agent_id: AgentId, elapsed_ms: u64 },
    
    /// User input has arrived for a waiting agent.
    UserInputReady { agent_id: AgentId },
    
    /// A message has been delivered to an agent.
    MessageDelivered { sender: AgentId, receiver: AgentId },
    
    /// A tool call result has arrived for an agent.
    ToolResultReady { agent_id: AgentId, call_id: String },
    
    /// Memory retrieval completed.
    MemoryReady { agent_id: AgentId },
}
```

### 1.3 `SchedulingContextUpdate`

```rust
/// Fields that can be updated in an agent's scheduling context.
#[derive(Debug, Clone)]
pub struct SchedulingContextUpdate {
    pub priority: Option<i32>,
    pub quantum_used: Option<u64>,
}

impl SchedulingContextUpdate {
    pub fn new() -> Self {
        Self {
            priority: None,
            quantum_used: None,
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = Some(p);
        self
    }

    pub fn with_quantum(mut self, q: u64) -> Self {
        self.quantum_used = Some(q);
        self
    }
}
```

## 2. Concrete Scheduler Implementations

### 2.1 `FifoScheduler`

```rust
/// FIFO scheduler: runs agents in the order they became ready.
pub struct FifoScheduler {
    queue: Vec<AgentId>,
    ready_set: HashSet<AgentId>,
}

impl FifoScheduler {
    pub fn new() -> Self {
        Self {
            queue: vec![],
            ready_set: HashSet::new(),
        }
    }

    /// Get the next agent to run in FIFO order.
    pub fn next_to_run(&mut self) -> Option<AgentId> {
        if let Some(id) = self.queue.first().copied() {
            let _ = self.ready_set.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}

impl Default for FifoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for FifoScheduler {
    // ... implementation
}
```

**Behavior guarantees**:
- Agents run in the exact order they become ready.
- No starvation: every ready agent eventually runs.
- Deterministic: same spawn order → same execution order.

### 2.2 `TokenBudgetPriorityScheduler`

```rust
/// Scheduler that prioritizes agents by remaining token budget.
///
/// ## Strategy
/// - Higher remaining token budget = higher priority
/// - Breaks ties by time since last run
pub struct TokenBudgetPriorityScheduler {
    // Priority queue ordered by (remaining_budget, last_run_time)
    queue: BinaryHeap<TokenBudgetEntry>,
    ready_set: HashSet<AgentId>,
    // Cache of agent IDs -> budgets (updated on yield/block)
    budget_cache: HashMap<AgentId, u64>,
}

#[derive(Debug, Clone)]
struct TokenBudgetEntry {
    agent_id: AgentId,
    remaining_budget: u64,
    last_run: chrono::DateTime<chrono::Utc>,
}

// PartialOrd implements max-heap ordering
impl PartialOrd for TokenBudgetEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Higher remaining_budget = higher priority
        // For tie-breaker, earlier last_run = higher priority
        match other.remaining_budget.cmp(&self.remaining_budget) {
            std::cmp::Ordering::Equal => self.last_run.partial_cmp(&other.last_run),
            ord => ord,
        }
    }
}

impl Ord for TokenBudgetEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq for TokenBudgetEntry {
    fn eq(&self, other: &Self) -> bool {
        self.agent_id == other.agent_id && self.remaining_budget == other.remaining_budget
    }
}

impl Eq for TokenBudgetEntry {}

impl TokenBudgetPriorityScheduler {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            ready_set: HashSet::new(),
            budget_cache: HashMap::new(),
        }
    }

    /// Set the budget for an agent (called when tokens are consumed).
    pub fn set_budget(&mut self, agent_id: AgentId, budget: u64) {
        self.budget_cache.insert(agent_id, budget);
    }
}
```

**Behavior guarantees**:
- Agents with more remaining budget run first.
- Ties broken by fairness (longest wait time).
- Prevents low-budget agents from starving completely.

### 2.3 `DeadlineAwareScheduler`

```rust
/// Scheduler that prioritizes agents by deadline proximity.
///
/// ## Strategy
/// - Closer deadline = higher priority
/// - If no deadline, fall back to FIFO
pub struct DeadlineAwareScheduler {
    queue: BinaryHeap<DeadlineEntry>,
    ready_set: HashSet<AgentId>,
    deadline_cache: HashMap<AgentId, chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
struct DeadlineEntry {
    agent_id: AgentId,
    deadline: chrono::DateTime<chrono::Utc>,
    last_run: chrono::DateTime<chrono::Utc>,
}

// PartialOrd: earlier deadline = higher priority
impl PartialOrd for DeadlineEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match other.deadline.cmp(&self.deadline) {
            std::cmp::Ordering::Equal => self.last_run.partial_cmp(&other.last_run),
            ord => ord,
        }
    }
}

// ... similar to TokenBudgetPriorityScheduler
```

---

## 3. Runtime Integration

### 3.1 `Runtime` Struct

```rust
pub struct Runtime {
    process_table: ProcessTable,
    scheduler: Box<dyn Scheduler>,
    event_loop: EventLoop,
}

impl Runtime {
    pub fn new<S: Scheduler + 'static>(scheduler: S) -> Self {
        Self {
            process_table: ProcessTable::new(),
            scheduler: Box::new(scheduler),
            event_loop: EventLoop::new(),
        }
    }

    /// Spawn an agent into the runtime.
    pub fn spawn(&mut self, id: AgentId, intent: Intent, program: AgentProgram) {
        self.process_table.spawn(id, intent, program);
        self.scheduler.on_ready(id);
    }

    /// Run the next available agent. Returns None if no agents are ready.
    pub fn step_next(&mut self) -> Option<StepResult> {
        let agent_id = self.scheduler.next_to_run()?;
        
        let mut record = self.process_table.get_mut(agent_id)?;
        if record.state.current != "Running" {
            record.update_state(SchedulingState::Running);
        }

        let step_result = record.agent.step(&mut StepContext::new(
            &record.memory_handles,
            &record.channel_endpoints,
            chrono::Utc::now(),
        ));

        self.scheduler.on_step_complete(agent_id, step_result);
        Some(step_result)
    }

    /// Signal that an agent has yielded.
    pub fn on_yield(&mut self, agent_id: AgentId) {
        self.scheduler.on_yield(agent_id);
        self.process_table.update_state(agent_id, SchedulingState::Suspended);
    }

    /// Signal that an agent has terminated.
    pub fn on_terminate(&mut self, agent_id: AgentId, reason: TerminationReason) {
        self.scheduler.on_terminate(agent_id, reason);
        self.process_table.terminate(agent_id, reason);
    }

    /// Handle an external event.
    pub fn on_event(&mut self, event: SchedulingEvent) {
        self.scheduler.on_event(event);
        self.event_loop.dispatch(event);
    }
}
```

### 3.2 `ProcessTable`

```rust
pub struct ProcessTable {
    agents: HashMap<AgentId, AgentRecord>,
}

impl ProcessTable {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, id: AgentId, intent: Intent, program: AgentProgram) {
        self.agents.insert(
            id,
            AgentRecord::new(id, intent, program),
        );
    }

    pub fn get(&self, id: AgentId) -> Option<&AgentRecord> {
        self.agents.get(&id)
    }

    pub fn get_mut(&mut self, id: AgentId) -> Option<&mut AgentRecord> {
        self.agents.get_mut(&id)
    }

    pub fn terminate(&mut self, id: AgentId, reason: TerminationReason) {
        if let Some(record) = self.agents.get_mut(&id) {
            record.update_state(SchedulingState::Terminated { reason, at: chrono::Utc::now() });
        }
        let _ = self.agents.remove(&id);
    }

    pub fn all_ready_ids(&self) -> Vec<AgentId> {
        self.agents
            .values()
            .filter(|r| r.state.current == "Ready")
            .map(|r| r.id)
            .collect()
    }
}
```

---

## 4. Test Fixtures

### 4.1 `MockScheduler`

Used for testing — records all calls for verification.

```rust
#[derive(Debug, Default)]
pub struct MockScheduler {
    pub on_yield_calls: Arc<Mutex<Vec<AgentId>>>,
    pub on_terminate_calls: Arc<Mutex<Vec<(AgentId, TerminationReason)>>>,
    pub next_to_run_calls: Arc<Mutex<Option<AgentId>>>,
}

impl Scheduler for MockScheduler {
    fn on_yield(&mut self, agent_id: AgentId) {
        self.on_yield_calls.lock().unwrap().push(agent_id);
    }

    fn on_terminate(&mut self, agent_id: AgentId, reason: TerminationReason) {
        self.on_terminate_calls.lock().unwrap().push((agent_id, reason));
    }

    fn next_to_run(&mut self) -> Option<AgentId> {
        let result = self.next_to_run_calls.lock().unwrap().take();
        result
    }

    // ... other methods record calls
}
```

---

## 5. API Usage Example

```rust
// examples/fifo_scheduling.rs

use chora_runtime::{Runtime, Agent, AgentId, Intent, AgentProgram, MockScheduler};

fn main() {
    // Create runtime with FIFO scheduler
    let mut runtime = Runtime::new(FifoScheduler::new());

    // Spawn agents
    for i in 0..3 {
        let id = AgentId::new();
        let intent = Intent::new_root(id, format!("Task {}", i), Some(id));
        let program = AgentProgram::default();
        runtime.spawn(id, intent, program);
    }

    // Run until completion
    while runtime.has_ready_agents() {
        runtime.step_next();
    }

    println!("All agents completed!");
}
```

---

## 6. References

- `rfc/001-agent-process.md` — Agent trait and states
- `rfc/002-scheduling.md` — Scheduler design rationale
- `notes/01_os-agent-mapping.md` — Principles P1, P3; tensions T2, T3
