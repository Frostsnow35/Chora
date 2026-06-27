# Chora — Project Manifest

> This document is the root reference for Chora. It states why the project exists, the metaphor that guides it, the research questions it pursues, and the shape of the system as currently understood. It is meant to be read in full before touching code, and updated whenever a foundational assumption changes.

## 1. Mission

Chora is a research project. Its object of study is the **agent system** — not as an application to be shipped, but as a class of computational system whose structure, dynamics, and failure modes are still poorly understood.

The working hypothesis is that **operating systems and classical systems design offer the most productive lens available today for thinking about agents**. OSes have spent decades solving, at scale, the exact category of problems agent systems are now reinventing: concurrent execution of opaque entities, shared resource management, isolation, scheduling, context switching, persistent state, failure containment, and inter-process communication.

Chora exists to:

- study agents through this systems lens,
- distill reusable abstractions from the study,
- and validate those abstractions by implementing them in a concrete runtime.

Chora is not a product. Shipping is not the success criterion. Clarity of abstraction is.

## 2. The Chora Metaphor

The name comes from Plato's *Timaeus*, where **chora** (χώρα) designates the "receptacle" — the indeterminate space in which forms come to appear. It is neither the model nor the copy, but the *place* that makes the appearance of either possible.

This is the guiding metaphor for the project:

> The runtime is not a machine that executes agents the way a CPU executes instructions. It is the **field in which agents take form and persist**.

Consequences for design:

- The primary design attitude is **hospitality** — making room — rather than command.
- Identity belongs to the agent; the runtime supplies the conditions.
- Structure emerges from the relationship between field and occupant, not from either alone.

**Important**: the metaphor is heuristic, not doctrinal. Wherever the metaphor leads to a bad design (and it will), the design wins. The metaphor's job is to suggest, not to legislate.

## 3. Core Research Questions

The following six questions organize the research. They are written as analogies to OS concepts, but each should be read as an open question, not a claim that the analogy holds. The project's work is to discover, for each, whether the OS mapping is genuinely illuminating or merely evocative.

### Q1 — The Process Abstraction
What is the minimal coherent "agent process"? What belongs in its PCB-equivalent — identity, current goal, context pointer, memory handles, scheduling state? At what granularity do we snapshot it?

### Q2 — Scheduling
When multiple agents are runnable, who goes next, and by what rule? Does a preemptive model make sense, or must agents run to explicit yield points? How do priorities, fairness, and starvation translate when the "CPU time" is actually LLM inference budget?

### Q3 — Context and Memory
Is the context window usefully modeled as RAM, with long-term storage as virtual memory and a notion of page faults? If so, what is the page replacement policy, and who pays the cost? How do the three memory tiers — working, episodic, semantic — map onto classical memory hierarchy?

### Q4 — Inter-Agent Communication
What are the right primitives: message passing, shared state, events, rendezvous, blackboards? Should communication be synchronous or asynchronous? Is there a meaningful analogue of a file descriptor — a uniform handle over which agents exchange typed streams?

### Q5 — Lifecycle and Hierarchy
What does `fork` mean for an agent? Is there a meaningful `exec` — replacing an agent's program while keeping its identity? How are parent/child relationships, orphans, and adoption handled? What composes the equivalent of a process group or session?

### Q6 — Failure, Isolation, and Signals
What is the agent analogue of a segfault? Of `SIGKILL` vs `SIGTERM`? Should one agent's failure be containable, and if so, what is the fault domain? Does the runtime offer anything like a supervisor — a process whose only job is to restart failed children?

## 4. System Overview

The implementation is a Rust workspace with two crates, reflecting the two axes the project treats as fundamental:

### `runtime`
The execution-field crate. Owns:

- the **Agent** abstraction and its lifecycle,
- the **process table** (the registry of live agents and their state),
- the **execution field** — defined as the unified space in which agents run, composed of **state + rules + environment**,
- the scheduler (shape TBD),
- the IPC mechanisms (shape TBD).

### `memory`
The context-and-memory crate. Owns:

- **working memory** (the agent's current context window / active scratch),
- **episodic memory** (timestamped records of what has happened),
- **semantic memory** (distilled knowledge, beliefs, concepts).

The boundary between the crates is intentional: `runtime` should not know how memory is stored or indexed; `memory` should not know about scheduling. They meet at a narrow interface — handles and requests — analogous to the syscall boundary.

### The execution field

Defined above as **state + rules + environment**:

- **state** — everything observable about the present configuration of agents and shared resources,
- **rules** — invariants and transition policies the field enforces,
- **environment** — the external world the agents perceive and act upon (tools, I/O, other services).

A kernel layer very likely exists in this picture — a distinction between privileged runtime machinery and the agents it hosts. **It is deliberately not named here.** The project's position is that naming it too early would freeze a design that should be allowed to emerge from the concrete problems Q1–Q6 force us to solve. When the distinction becomes unavoidable, it will be named and scoped in an RFC.

## 5. Open Architectural Questions

These are the questions the project does not yet have answers to. Each is expected to produce at least one RFC before implementation.

- What is the atomic unit of agent execution — a *step*, a *tick*, a *turn*? What properties must it have?
- Is there a single global scheduler, or one scheduler per field / per agent group?
- Does context promotion (working ↔ episodic ↔ semantic) happen inside the agent, inside the memory crate, or cooperatively?
- What is the agent's handle on the world? A capability system? A resource table? Something else?
- How is non-determinism (inherent in LLM-based agents) contained? Is reproducibility a first-class requirement, and if so, at what layer?
- What does "correctness" mean for an agent system, and how could the benchmark suite ever measure it?

## 6. Current State and Next Steps

**Current state.** The repository contains only the workspace scaffold: two empty crates with clippy configuration and a project-level `instructions.md`. No abstractions have been committed. The directory layout is:

| Directory | Purpose |
|---|---|
| `architecture/` | Stable structural descriptions — this document lives here. |
| `rfc/` | Numbered decision documents, one per design choice. |
| `notes/` | Working thought, analogy exploration, half-formed ideas. |
| `projects/` | Plans for experimental sub-projects. |
| `experiments/` | Throwaway prototypes; each must carry a README reproducing the insight. |
| `benchmarks/` | Performance and behavioral benchmarks. |

**Next steps**, in order:

1. Drive Q1 to a first-draft position: write `rfc/001-agent-process.md` proposing the minimal agent-PCB structure and the `Agent` trait surface.
2. Implement the smallest possible runtime that can **spawn, step, and terminate** a single mock agent — enough to make the abstraction hurt, so we can see where it breaks.
3. Based on what breaks, return to Q2 (scheduling) with actual evidence rather than speculation.

The project advances by writing the question, then writing the code that forces the question to become precise, then revising the question.
