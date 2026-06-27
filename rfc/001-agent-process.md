# RFC-001 — The Agent Abstraction

| Field | Value |
|---|---|
| Status | Draft |
| Author | — |
| Created | 2026-06-25 |
| Depends on | `architecture/00_MANIFEST.md`, `notes/01_os-agent-mapping.md` |

---

## Abstract

This RFC proposes the **minimal abstraction through which `runtime` views an agent**. The abstraction has four components:

1. An `Agent` trait — the interface the runtime calls.
2. An `Intent` descriptor — the agent's *why*, absent from OS abstractions.
3. An `AgentProgram` — the three-layer description of *what the agent is* (model / prompt / tools).
4. An `AgentRecord` — the PCB-equivalent that the process table holds.

The design is constrained by the principles and tensions laid out in `notes/01_os-agent-mapping.md`. In particular: **P2** (single unified abstraction), **P4** (runtime is dumb), **P1** (mechanism/policy separation), and **T5** (minimal sufficient).

The abstraction is considered successful if it can host three radically different agents (tool-using, pure-reasoning, long-term-memory-dependent) without modification.

---

## 1. Grounding

This RFC is an application of the methodology in `notes/01`. The relevant citations:

- **P2 (Abstraction by Reduction)** — mandates a single `Agent` trait regardless of internal diversity.
- **P4 (End-to-End)** — the `Agent` trait must not ask the agent to do the runtime's job, nor the runtime to do the agent's job. Semantic work (understanding, judging, summarizing) lives outside `runtime`.
- **P1 (Mechanism / Policy)** — the trait exposes *mechanisms* (`step`, `receive`, `yield`). Scheduling policy is injected.
- **T5 (Simplicity ↔ Expressiveness)** — the trait is minimal sufficient. We must be able to name a behavior that each method enables.
- **T3 (Runtime Power ↔ Agent Autonomy)** — the runtime decides *when* the agent runs; the agent decides *what to do* in its turn.
- **T7 (Performance ↔ Auditability)** — every step must produce an auditable record.

The OS process is the starting inspiration, but this RFC explicitly departs from OS shape in two places: the **intent descriptor** (OS has no analogue) and the **three-layer program** (OS has one: the binary).

---

## 2. The `Agent` Trait

```rust
/// The minimal interface through which the runtime interacts with an agent.
///
/// Implementations may wrap any reasoning substrate (LLM, rule engine,
/// symbolic planner, human-in-the-loop). The runtime sees only this shape.
///
/// # Safety
///
/// This trait is safe. It does not require any unsafe operations or expose unsafe methods.
public trait Agent {
    /// Unique identity within the field.
    fn id(&self) -> AgentId;

    /// The agent's current intent — what it is trying to achieve and why.
    fn intent(&self) -> &Intent;

    /// Current scheduling state (Ready / Running / Blocked / Suspended / Terminated).
    fn state(&self) -> SchedulingState;

    /// Execute one step of the agent's reasoning, within the given context.
    ///
    /// This is the atomic unit of agent execution (cf. manifest §5 "what is
    /// the atomic unit?"). The step must be:
    ///   - self-contained: all inputs come from `ctx`
    ///   - auditable: must produce a StepRecord for the episodic log
    ///   - bounded: must terminate in finite time / tokens
    fn step(&mut self, ctx: &mut StepContext) -> StepResult;

    /// Deliver an incoming message. The agent decides how (or whether) to
    /// integrate it. The runtime does not interpret the message.
    fn receive(&mut self, msg: Message) -> Result<(), DeliveryError>;

    /// Cooperatively yield. Called by the scheduler at clean boundaries.
    fn yield_now(&mut self);

    /// Resume after a yield or block.
    fn resume(&mut self);

    /// Terminate with a stated reason. The agent may perform cleanup.
    fn terminate(&mut self, reason: TerminationReason);
}
```

### 2.1 What each method enables (T5 justification)

| Method | Behavior it enables (otherwise inexpressible) |
|---|---|
| `id` | The runtime can refer to the agent in the process table. |
| `intent` | Observability of "why is this agent running" — essential for scheduling, delegation, adoption. |
| `state` | The scheduler can decide who is runnable. |
| `step` | The basic execution unit. Without it, no execution. |
| `receive` | Inter-agent communication (T1: cooperation must be explicit). |
| `yield_now` | Cooperative scheduling (Q2 verdict: preemptive rejected). |
| `resume` | Resuming after yield / block. |
| `terminate` | Clean end-of-life, with reason for supervisors (Q6). |

### 2.2 What is deliberately absent

- No LLM-specific methods. No `call_openai`, no `invoke_model`. The agent decides internally how to reason. (P2, P4)
- No direct memory access. Memory is reached through capabilities handed in via `StepContext`. (P5)
- No `fork`, no `exec`. Agent identity is singular; delegation is expressed via spawning a new agent with a derived intent (see §6).
- No built-in tools. Tools are provided through `AgentProgram` and executed via `ToolExecutor` as described in §8. (P4, P5)

---

## 3. The Intent Descriptor

### 3.1 Intent as First-Class Concept

The element that distinguishes agent systems from OS process systems. A process has no "why." An agent does.

```rust
pub struct Intent {
    pub id: IntentId,
    pub goal: GoalDescription,
    pub success_criteria: Vec<Criterion>,
    pub constraints: Constraints,
    pub parent: Option<IntentId>,
    pub metadata: Metadata,
}
```

### 3.2 Why Intent is First-Class

- **Q1 verdict**: without intent, the agent abstraction collapses into a process — losing the agent's essential characteristic.
- **Scheduling**: when two agents compete for resources, intent's `constraints` and `goal` inform priority (T2 fairness is defined over intents, not agents).
- **Delegation**: when agent A spawns agent B for a subtask, B's intent carries `parent = Some(A's intent id)`. The runtime can track the lineage.
- **Adoption**: if an agent dies and its intent is still live, the runtime can reassign the intent to a new agent. The intent outlives the agent.

### 3.3 What Intent is *not*

- It is not the plan. The plan is in the agent's working memory.
- It is not the goal's achievement state. That is observable via success-criteria evaluation.
- It is not mutable. Once an intent is issued, it is a fixed contract.

---

## 4. The `AgentProgram` — Three-Layer Description

### 4.1 Why three layers

- **Model** is shared, often provider-wide. Many agents may share a model. It is infrastructure.
- **Prompt** is per-agent (or per-role). It is the agent's *character*.
- **Tools** are per-task. They define the agent's *reach into the world*.

### 4.2 Layer definitions

```rust
pub struct AgentProgram {
    pub model: ModelDescriptor,
    pub prompt: PromptDescriptor,
    pub tools: ToolSet,
}
```

- **ModelDescriptor**: LLM provider, version, and configuration
- **PromptDescriptor**: system prompt, persona, behavioral rules
- **ToolSet**: capabilities that the agent can invoke (see §8)

### 4.3 What is not in `AgentProgram`

- The agent's *memory* — that is held separately, accessed via capabilities.
- The agent's *current goal* — that is `Intent`, held separately.

---

## 5. The `AgentRecord` — PCB-Equivalent

### 5.1 PCB-Equivalent components

```rust
pub struct AgentRecord {
    pub id: AgentId,
    pub state: StateSnapshot,
    pub intent: Intent,
    pub program: AgentProgram,
    pub memory_handles: Vec<String>,
    pub channel_endpoints: Vec<String>,
    pub capabilities: Vec<String>,
    pub scheduling_context: SchedulingContext,
    pub metrics: AgentMetrics,
}
```

### 5.2 Key relationships

| Field | Owner | Runtime role |
|---|---|---|
| `id` | Runtime (assigns) | Immutable reference |
| `state` | Runtime (assigns) | Updated on lifecycle events |
| `intent` | Agent (configures at spawn) | Runtime reads for scheduling |
| `program` | Agent (configures at spawn) | Runtime reads for capability mediation |
| `memory_handles` | Runtime (grants) | Enforced at access time |
| `channel_endpoints` | Runtime (grants) | Enforced at send/receive |
| `capabilities` | Runtime (grants/revokes) | Enforced at every external access |

---

## 6. Lifecycle

### 6.1 Spawn is single-operation

Deliberately rejecting the OS split of `fork` + `exec` (cf. notes/01, Q5 verdict). Spawn = create a new agent with intent + program + initial capabilities. Atomic.

### 6.2 Delegation

Delegation is expressed as:
1. Agent A's intent is I_A.
2. A calls a spawn mechanism with a **derived intent** I_B whose `parent = Some(I_A.id)`.
3. B runs with I_B. When B terminates, its results are available to A via a channel or a result handle.

Delegation is a **pattern**, not a primitive. This preserves orthogonality (P3).

### 6.3 Orphan adoption

If agent A dies and A's intent I_A is still live (success criteria not met, constraints not exhausted), the runtime may:
- Propagate termination to children (default: fail-fast).
- Or offer I_A for adoption by another agent (supervisor policy).

---

## 7. Boundaries

| Concept | Relationship to `Agent` |
|---|---|
| `Memory` | Agent holds *capabilities* to memory tiers; memory crate is independent. |
| `Channel` | Agent holds *endpoints*; channel crate is independent. |
| `Capability` | Agent holds a set; all external access is capability-gated. |
| `Field` | Agent is an occupant of a field; field is the container (state + rules + environment). |
| `Supervisor` | Not a distinct abstraction in this RFC. A supervisor is just an agent whose intent includes monitoring other agents. |
| `Scheduler` | Not an agent. A pluggable policy (P1). |

---

## 8. Empirical Validation Plan

### 8.1 Tool Integration Update

The `tool` system is now fully implemented in `runtime/src/tool.rs` and includes:

- `ToolExecutor` trait: execute tools, return structured results
- `ToolRegistry`: map tool names to executors
- `BuiltinToolExecutor`: registry of standard tools (echo, add, shell)
- `ToolResult` and `ToolOutcome`: well-defined result structures

This system supports the three validation agents below through the `StepContext`'s `pending_tool_result` field.

### 8.2 Tool-Using Agent (Calculator)

- **Definition**: Agent with tools (add, shell)
- **Validation steps**:
  1. Spawn with goal: "compute the value of (2 + 3) * 4"
  2. Verify the agent sends a `ToolCallRequest` for `add` with args `{a: 2, b: 3}`
  3. Verify the agent receives a `pending_tool_result` with result `5`
  4. Verify the agent sends a second `ToolCallRequest` for `add` with args `{a: 5, b: 4}`
  5. Verify the agent returns the final result `20`

### 8.3 Pure-Reasoning (Summarizer)

- **Definition**: Agent with no tools
- **Validation steps**:
  1. Spawn with goal: "summarize this text: [sample]"
  2. Verify the agent never attempts a tool call
  3. Verify the agent returns a concise summary

### 8.4 Long-Term-Memory-Dependent (Personal Assistant)

- **Definition**: Agent with memory tools
- **Validation steps**:
  1. Spawn with goal: "track my project deadlines"
  2. Verify the agent stores information via memory operations
  3. Verify the agent retrieves data on follow-up requests

### 8.5 Cross-Validation

All three agents must:
- Share one `AgentRecord` shape in the process table
- Run on the same runtime without modification
- Not require LLM-specific code in `runtime`

---

## 9. Open Questions

| Question | Why it matters |
|---|---|
| **1. How to handle deadlocks?** | What if two agents are waiting for each other? |
| **2. How to handle the "run to completion" pattern?** | Some agents (e.g., a long-running analysis task) may never yield. |
| **3. How to handle "hijacking"?** | What if an agent yields but never resumes? |

---

## 10. Implementation Path

1. Add the types to `runtime/src/agent/` behind a `agent` module: `Agent` trait, `Intent`, `AgentProgram`, `AgentRecord`, `SchedulingState`, `AgentMetrics`
2. Implement a **mock agent** (no LLM, just a scripted step function) to validate the trait surface
3. Implement **three validation agents**. The first one that does not fit forces a revision of this RFC
4. Only then proceed to RFC-002 (scheduling), with a concrete `Agent` abstraction in hand

---

## References

- `architecture/00_MANIFEST.md` — mission, metaphor, Q1–Q6.
- `notes/01_os-agent-mapping.md` — principles P1–P6, tensions T1–T8.
- Joe Armstrong, *Making reliable distributed systems in the presence of software errors* (2003) — supervisor philosophy.
- Saltzer, Reed, Clark, *End-to-End Arguments in System Design* (1984).
- Hewitt, *Universal Concurrent Programming: The Actor Model* (1985).