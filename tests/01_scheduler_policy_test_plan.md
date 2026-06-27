# Scheduler Policy Test Plan

> This document defines testable scenarios for evaluating the scheduler's behavior against the principles and tensions defined in `notes/01_os-agent-mapping.md`.

## 1. Test Matrix

| Test ID | Description | Target Principle/Tension | Pass Criteria |
|---|---|---|---|
| T1 | Basic FIFO scheduling | P1 (mechanism/policy) | Agents run in spawn order | 
| T2 | Priority-based dispatch (100 > 1) | T2 (fairness/efficiency) | Higher priority agent runs first | 
| T3 | Priority vs. wait time (100 but waiting 100ms vs 1 but waiting 50ms) | T3 (runtime power/agent autonomy) | Agent with longer wait time runs first | 
| T4 | Deadline-aware scheduling (2026-06-26T08:00 vs 2026-06-26T10:00) | T3 | Deadlined agent runs first | 
| T5 | Token budget (1000 vs 500 tokens remaining) | T2 | Token-budget-remaining agent runs first | 
| T6 | I/O-bound: frequent yields (3x/step) vs. infrequent (0.1x/step) | T3 | I/O-bound agent gets more turns | 
| T7 | Deadlock detection (A→B→A) | T2 | Scheduler detects deadlock and restarts | 
| T8 | Supervisor priority (supervisor vs. worker) | T4 (static/dynamic) | Supervisor runs more often | 
| T9 | Token budget exhaustion (500 vs 1000 tokens) | T2 | Agent with more budget runs first | 
| T10 | Time-bound (200ms vs 500ms) | T2 | Fast-er agent runs first | 

## 2. Detailed Test Cases

### T1: Basic FIFO

**Setup**:
- Spawn 3 agents: A, B, C in order
- No constraints (infinite token budget, no deadline)

**Actions**:
- A calls `yield_now()` after 1 step
- B calls `yield_now()` after 2 steps
- C does not yield

**Expected behavior**:
1. A runs first (1 step) → yields
2. B runs next (2 steps) → yields
3. C runs next (2 steps) → does not yield
4. A runs again (1 step) → yields
5. B runs (2 steps) → yields
6. C runs (2 steps) → ... (repeats)

**Pass criteria**:
- Order of execution: A → B → C → A → B → C → ...
- Total steps per agent after 1000 total steps: A=333, B=333, C=334

### T2: Priority-Based Dispatch

**Setup**:
- A (priority 100), B (priority 1)
- Same constraints (infinite tokens, no deadline)

**Actions**:
- A calls `yield_now()` after 3 steps
- B calls `yield_now()` after 5 steps

**Expected behavior**:
- A runs 50% more often than B (e.g., 600 steps for A, 400 for B)
- A is always ahead in the process table queue

**Pass criteria**:
- A's steps > B's steps
- A runs at least 40% of the time

### T3: Priority vs. Wait Time

**Setup**:
- A (priority 90, wait=100ms), B (priority 100, wait=50ms)
- 2000 steps total

**Actions**:
- A yields after 1 step
- B yields after 1 step

**Expected behavior**:
- A and B run approximately 50-50, but A gets slightly more runs

**Pass criteria**:
- A's steps: 1000-1050
- B's steps: 950-1000
- A > B

### T4: Deadline-Aware Scheduling

**Setup**:
- A (deadline=2026-06-26T08:00), B (deadline=2026-06-26T10:00)
- 1000 steps total

**Actions**:
- A and B both yield after 1 step

**Expected behavior**:
- A runs at least 600 times before 08:00
- B runs 400 times after 08:00

**Pass criteria**:
- A completes before deadline
- B completes after deadline but without exceeding constraints

### T5: Token Budget (1000 vs 500)

**Setup**:
- A (1000 tokens), B (500 tokens), 2000 total steps
- A consumes 10 tokens/step, B consumes 10 tokens/step

**Actions**:
- A and B yield after 1 step

**Expected behavior**:
- A runs 1200 times (12,000 tokens)
- B runs 800 times (8,000 tokens)

**Pass criteria**:
- A's token usage ≤ 10,000
- B's token usage ≤ 5,000
- A runs more than B

## 3. Verification Plan

1. **Implementation**: Build a `SchedulingTest` harness with mock agents
2. **Execution**:
   - Run each test 5 times
   - Log steps, tokens, and run order
3. **Metrics**:
   - Fairness ratio (A/B steps)
   - Constraint violations
   - Deadlock detection rate
4. **Reporting**:
   - Create `002a-scheduling-test-results.md` with results
   - Compare against pass criteria

## 4. Falsification

If any test fails, the scheduler policy is either:
- Incorrectly implemented
- Or the test is poorly designed

The test will be **falsified** if:
- The scheduler fails to follow the policy (e.g., FIFO but runs in reverse order)
- The policy violates P1 (mechanism/policy) or P3 (orthogonality)

