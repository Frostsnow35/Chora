# Scheduler Test Harness Implementation Plan

> This document details how the `SchedulingTest` harness will implement the test scenarios defined in `tests/01_scheduler_policy_test_plan.md`. It serves as a blueprint for both the code implementation and the verification process.

## 1. Harness Structure

### 1.1. Core Components

| Component | Purpose |
|---|---|
| **ProcessTable** | Tracks agent state (Ready/Running/Blocked) and `AgentRecord` snapshots |
| **SchedulerFactory** | Instantiates scheduler policies (FIFO, TokenBudget, DeadlineAware) |
| **TestRunner** | Orchestrates test execution and metrics collection |
| **MockAgentFactory** | Creates configurable mock agents (with scripted behavior) |
| **TestResultLogger** | Logs raw metrics and verification status |

### 1.2. File Structure

```
src/
  tests/
    scheduling/
      00_fi
      01_priority_100_vs_1.rs
      02_priority_vs_wait_time.rs
      03_deadline_aware.rs
      04_token_budget.rs
      05_io_bound.rs
      06_deadlock_detection.rs
      07_supervisor_priority.rs
      08_token_exhaustion.rs
      09_time_bound.rs
      10_falsification.rs
    common/
      process_table.rs
      scheduler_factory.rs
      test_runner.rs
      mock_agent_factory.rs
      test_result_logger.rs
```

## 2. Test Case Implementation

### 2.1. Test 1: Basic FIFO (00_fifo)

```rust
// tests/scheduling/00_fifo.rs

#[test]
fn fifo_scheduling_test() {
    let mut test = SchedulingTest::new();
    let agent_a = MockAgent::default();
    let agent_b = MockAgent::default();
    let agent_c = MockAgent::default();

    // Spawn agents in order A → B → C
    test.spawn(agent_a.clone(), 100);
    test.spawn(agent_b.clone(), 100);
    test.spawn(agent_c.clone(), 100);

    // Configure agents to yield after specific steps
    test.set_agent_behavior(agent_a.id(), vec![
        ScriptedAction::Yield,
    ]);
    test.set_agent_behavior(agent_b.id(), vec![
        ScriptedAction::Yield,
        ScriptedAction::Yield,
    ]);
    test.set_agent_behavior(agent_c.id(), vec![]);

    // Run 1000 total steps
    test.run(1000);

    // Verify order of execution
    assert!(test.get_execution_order().starts_with(&[1, 2, 3]));
    assert_eq!(test.get_step_counts(), (333, 333, 334));
}
```

### 2.2. Test 2: Priority-Based Dispatch (01_priority_100_vs_1)

**Key parameters**:
- `priority_a = 100`
- `priority_b = 1`

```rust
// tests/scheduling/01_priority_100_vs_1.rs

#[test]
fn priority_scheduling_test() {
    let mut test = SchedulingTest::new();
    let (agent_a, agent_b) = (MockAgent::default(), MockAgent::default());

    // Configure agents with different priorities
    test.set_priority(agent_a.id(), 100);
    test.set_priority(agent_b.id(), 1);

    // Run 1000 total steps
    test.run(1000);

    // Verify A runs 50% more than B
    let (a_steps, b_steps) = test.get_step_counts();
    assert!(a_steps > b_steps);
    assert!(a_steps as f32 / b_steps as f32 > 1.2);
}
```

### 2.3. Test 3: Priority vs. Wait Time (02_priority_vs_wait_time)

**Key parameters**:
- `A (priority=90, wait=100ms)`
- `B (priority=100, wait=50ms)`

```rust
// tests/scheduling/02_priority_vs_wait_time.rs

#[test]
fn wait_time_priority_test() {
    let mut test = SchedulingTest::new();
    let (agent_a, agent_b) = (MockAgent::default(), MockAgent::default());

    // Configure wait times
    test.set_wait_time(agent_a.id(), 100);
    test.set_wait_time(agent_b.id(), 50);

    // Run 1000 total steps
    test.run(1000);

    // Verify A gets slightly more runs
    let (a_steps, b_steps) = test.get_step_counts();
    assert!(a_steps > b_steps);
    assert!(a_steps - b_steps < 100);
}
```

## 3. Metrics Collection

### 3.1. Core Metrics

| Metric | Type | How Collected |
|---|---|---|
| `steps_per_agent` | u64 | `process_table` `metrics` field |
| `tokens_consumed` | u64 | MockAgent `token_counter` |
| `execution_order` | Vec<u64> | Log of `step()` calls |
| `constraint_violations` | u64 | Agent `check_constraints_violated()` |
| `yields_per_agent` | u64 | Count of `yield_now()` calls |

### 3.2. Collection Workflow

1. **Pre-test setup**: Reset all metrics
2. **During test**: Capture metrics for each agent's step
3. **Post-test**: Aggregate metrics across all agents

**Example**: 
```rust
// common/test_result_logger.rs

pub struct TestResult {
    pub agent_id: AgentId,
    pub steps_executed: u64,
    pub tokens_consumed: u64,
    pub execution_order: Vec<u64>,
}

impl TestResult {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            steps_executed: 0,
            tokens_consumed: 0,
            execution_order: Vec::new(),
        }
    }

    pub fn record_step(&mut self, tokens: u64) {
        self.steps_executed += 1;
        self.tokens_consumed += tokens;
    }
}
```

## 4. Result Validation

### 4.1. Pass Criteria Implementation

| Test ID | Validation Code |
|---|---|
| T1 | `assert_eq!(steps, (333, 333, 334));` |
| T2 | `assert!(a_steps > b_steps && a_steps / b_steps > 1.2);` |
| T3 | `assert!(a_steps > b_steps && (a_steps - b_steps) < 100);` |
| T4 | `assert!(a_completes_before_deadline);` |
| T5 | `assert!(a_tokens <= 10_000 && b_tokens <= 5_000);` |

### 4.2. Falsification Workflow

1. If test fails, run debug mode (enable `tracing` logs)
2. Check:
   - Incorrect scheduler policy implementation
   - Mock agent behavior not matching expectations
   - Process table state not updated correctly
3. If issue is in scheduler logic, update `002-scheduling.md`
4. If issue is in test setup, update `01_scheduler_policy_test_plan.md`

## 5. Expected Execution Flow

1. **Setup**:
   - Create `SchedulingTest` instance
   - Configure scheduler policy
   - Spawn test agents
2. **Execution**:
   - Run the scheduler's main loop
   - Log results after each iteration
3. **Verification**:
   - Run validation assertions
   - Log test results
4. **Tear Down**:
   - Clean up resources
   - Report results

## 6. Falsification Plan

1. **Falsification Trigger**: Any test failure (assertion failure)
2. **Debugging Steps**:
   - Check `tracing` logs for unexpected state transitions
   - Inspect agent state (`SchedulingState`) at failure point
   - Verify scheduler's `next_to_run()` logic
3. **Root Cause Classification**:
   - **Scheduler bug**: Fix in `002-scheduling.md`
   - **Test bug**: Update `01_scheduler_policy_test_plan.md`
   - **Principle misapplication**: Re-examine `001-agent-process.md`

## 7. References

- `tests/01_scheduler_policy_test_plan.md` — Test cases and pass criteria
- `rfc/002-scheduling.md` — Scheduler design
- `rfc/001-agent-process.md` — Agent abstraction
- `00_MANIFEST.md` — Core research questions