# Operating System Thinking for Agent Systems

> This is Chora's methodological ground. It states what OS thinking gives to agent design — not as a set of analogies ("agent ≟ process"), but as **principles of thought** and **structural tensions** whose force has been proven at OS scale and whose shape recurs wherever many autonomous entities must coexist.
>
> This document is not descriptive. Every principle it names is meant to be **enforceable in code**, every tension is meant to be **usable as a design criterion** in RFCs, and every projection onto Chora's open questions is meant to be **testable in a prototype**. Where a principle cannot be made operational, it does not belong here.
>
> Readers should be able to finish this document and know, for any upcoming design decision, which principle or tension to invoke, and what evidence would settle the question.

---

## Part I — Principles of Thought

Six principles distilled from OS design. Each is stated as a principle, grounded briefly in its OS origin, then made **operational for Chora** — with a concrete way to tell whether we are honoring it or violating it.

### P1. Mechanism / Policy Separation

**Principle.** The runtime provides *mechanisms* — primitive operations for creating, communicating, synchronizing, terminating. *Policies* — which agent runs next, which memory to retrieve, how to route messages — are supplied from outside and are pluggable.

**OS origin.** The single most important architectural insight of modern OS design. Unix provides `fork`, `read`, `write`, `mmap`; it does not prescribe scheduling algorithms. This is why the same kernel runs web servers and batch jobs.

**Chora operationalization.**
- `runtime`'s public API exposes **only mechanisms**: `spawn`, `yield`, `send`, `receive`, `terminate`, `grant_capability`. It never says *which* scheduling algorithm, *which* memory replacement strategy, or *which* routing rule.
- Policies are injected as traits or configuration. Different workloads can plug different schedulers without touching `runtime`.

**How to detect violation.** Any line of code in `runtime` of the form "use algorithm X" or "always prefer Y" is a violation. Policies belong outside the crate boundary.

### P2. Abstraction by Reduction

**Principle.** Heterogeneous entities are reduced to a **single minimal abstraction** the system can handle uniformly. Diversity lives behind the abstraction; the system sees only the common shape.

**OS origin.** Process. Regardless of what program runs — browser, database, game — the OS sees a PCB, an address space, a set of file descriptors.

**Chora operationalization.**
- `Agent` is a single trait. Regardless of underlying LLM, goal, memory shape, or toolset, `runtime` sees the same interface.
- The abstraction must include what distinguishes agents from OS processes — primarily the **intent descriptor** — but must not leak LLM-specific or task-specific structure into the runtime layer.

**How to detect violation.** If `runtime` contains a method named after a specific LLM provider, a specific tool, or a specific task pattern, the abstraction has leaked.

**Empirical test.** Implement three agents with radically different LLMs / tools / goals. All three must live under the same `Agent` trait without modification. If they cannot, the abstraction has failed.

### P3. Orthogonality

**Principle.** Each primitive concept does **one thing**. Complexity arises from **combination**, not from concepts that smuggle in multiple concerns.

**OS origin.** Files, processes, threads, signals, sockets — each is a distinct concept with its own lifecycle. Their composition produces the system's expressive power.

**Chora operationalization.**
- Chora's primitive concepts should be mutually independent: `Agent`, `Intent`, `Memory` (with three tiers), `Message`, `Channel`, `Capability`, `Field`. Each has its own lifecycle and interface; none is a subset of another.
- When a concept appears to overlap with another, the overlap should be resolved by decomposition, not by merging.

**How to detect violation.** A dependency graph of the crates' core types should be acyclic. No type should be expressible as "just a specialization of" another.

### P4. End-to-End Principle

**Principle.** Functionality should be implemented at the **lowest layer that genuinely requires it**. Layers above should not duplicate what layers below can do; layers below should not preempt what only the endpoints can judge.

**OS origin.** Saltzer, Reed, Clark (1984). The network is dumb; intelligence lives at the edges. Same principle applies inside an OS: the kernel does not interpret file contents.

**Chora operationalization.**
- `runtime` is **dumb**. It does not understand messages, judge goal progress, summarize memories, or detect hallucination. These are **agent-side** or **specialized-agent-side** concerns.
- The verb set of `runtime` is mechanical: `spawn`, `step`, `yield`, `send`, `receive`, `grant`, `revoke`, `terminate`. If a verb like `understand`, `judge`, `summarize`, or `validate` appears in `runtime`, the end-to-end principle has been violated.

**How to detect violation.** Grep for semantic verbs in `runtime/src/`. Any match is a design bug.

### P5. Least Privilege / Capability Model

**Principle.** Access to resources is granted via **unforgeable, composable, revocable handles**. An agent holds only the capabilities it has been explicitly given. There is no ambient authority.

**OS origin.** KeyKOS, E, Coyotos, seL4. Decades of refinement. The insight: ambient authority (global namespaces, inherited permissions) is the root of most systemic insecurity.

**Chora operationalization.**
- Every external access an agent makes — to a tool, to another agent, to a memory tier, to the environment — is mediated by a `Capability`.
- Capabilities are granted by a parent agent or by the field; they are revocable; they compose (a capability to "read memory tier X" is distinct from "write memory tier X").
- No agent has ambient access to anything.

**How to detect violation.** Any code path where an agent touches a resource without presenting a capability is a violation. This includes "the agent that spawned me implicitly has access to my memory" — that is ambient authority.

### P6. Selective Transparency

**Principle.** Some things are made **transparent** (hidden from the entity, handled by the system). Other things are made **explicit** (the entity must request them, and know it is requesting them). The choice of which is which is a *design decision*, not a default.

**OS origin.** Virtual memory is transparent — the process believes it owns a contiguous address space. System calls are explicit — the process must invoke them by name.

**Chora operationalization.** Chora must **explicitly decide**, for each abstraction boundary, whether it is transparent or explicit — and be able to state why. Examples:
- *Message destination*: transparent. Agents send to a handle; the handle's physical location is hidden.
- *Memory promotion (working ↔ episodic ↔ semantic)*: explicit. Agents (or their designated memory-managing peers) decide what to promote; the runtime provides the mechanism.
- *Scheduling*: partially transparent. Agents `yield`, but do not see the run queue.
- *Capability revocation*: explicit. An agent is notified when a capability it holds is revoked.

**How to detect violation.** For every abstraction boundary, there should be a documented reason for its transparency level. "We didn't think about it" is not a reason.

---

## Part II — Structural Tensions

OS design has never *solved* any of the following tensions — it has learned to **balance** them. Agent systems will encounter the same tensions in new form. Naming them here gives Chora a vocabulary for recognizing them at design time.

For each tension, the entry gives: the OS-side balance, the Chora-side form, and a **criterion** for recognizing when we have drifted too far toward one pole.

### T1. Isolation ↔ Cooperation

**OS balance.** Separate address spaces (isolation) + IPC primitives (cooperation). Default: isolated. Cooperation must be explicitly arranged.

**Chora form.** Agents need independent reasoning (isolation) and joint task completion (cooperation).

**Criterion.** Default state of a freshly spawned agent: no channels, no shared memory, no access to any other agent's state. All cooperation is established by explicit `channel(...)` or `share(...)` calls. If a newly-spawned agent can observe another agent without a capability, isolation has failed.

### T2. Fairness ↔ Efficiency

**OS balance.** Schedulers oscillate between "everyone gets a turn" (fairness) and "maximize throughput" (efficiency). Neither pole is right alone.

**Chora form.** Token budget and inference turns are finite. Should every agent get equal turns, or should agents closer to completing valuable tasks get more?

**Criterion.** Fairness is defined in token-budget units, not time. The scheduler's policy is pluggable (P1). A fairness metric is always observable — we should be able to answer "was this run fair?" after the fact.

### T3. Runtime Power ↔ Agent Autonomy

**OS balance.** The kernel decides *when* and *where* things run; the process decides *what* to compute. The balance is what makes the system both orderly and useful.

**Chora form.** The runtime decides scheduling, memory pressure handling, capability mediation. The agent decides its goal, its reasoning, its interpretation of incoming messages.

**Criterion.** The runtime never chooses *for* the agent what the agent could choose for itself. If `runtime` contains a line like "choose the agent's next action," it has crossed into agent territory. This is the instantiation of P1 (mechanism/policy) at the runtime/agent boundary.

### T4. Static Configuration ↔ Dynamic Adaptation

**OS balance.** Boot-time configuration (what devices, what filesystems) vs runtime adjustment (loadable modules, cgroups tuning).

**Chora form.** An agent's strategy (memory retrieval, scheduling, cooperation pattern) — is it fixed at spawn, or can it evolve?

**Criterion.** All **policies** (P1) are dynamic — they can be swapped at runtime. All **mechanisms** are static — the trait boundaries of `runtime` do not shift. If a policy is hardcoded, or a mechanism is runtime-configurable, the balance has drifted.

### T5. Simplicity ↔ Expressiveness

**OS balance.** POSIX is small but expressive enough. Every added feature must justify itself.

**Chora form.** How thin can Chora's abstractions be while still supporting the behaviors we need?

**Criterion.** Every primitive concept in `runtime` or `memory` must pass the test: "name a behavior that cannot be expressed without this concept." If no such behavior can be named, the concept should be removed.

### T6. Transparency ↔ Explicitness

See P6. This is the same tension viewed as a balance rather than a principle.

**Criterion.** Same as P6. Every abstraction boundary has a documented transparency choice with a reason.

### T7. Performance ↔ Auditability

**OS balance.** Security vs speed. Address-space isolation, permission checks, syscall overhead — all cost performance, all are kept.

**Chora form.** Inference speed vs the ability to replay, audit, and validate what an agent did and why.

**Criterion.** Every agent action is recorded in episodic memory as a replayable log entry by default. This is not optional — it is the audit trail. Optimization of this path is permitted; elimination is not. If we cannot answer "what did agent X do between time A and time B?" from the log, auditability has failed.

### T8. Mechanism Universality ↔ Policy Configurability

See P1. Same tension, different aspect.

**Criterion.** Same as P1.

---

## Part III — Projections onto Chora's Six Questions

Each of the six open questions from `architecture/00_MANIFEST.md §3` re-read through the lens of the principles and tensions above, with a concrete path to empirical validation.

### Q1 — The Process Abstraction

**Read through the lens.**
- P2 (abstraction by reduction) demands a single unified `Agent` trait.
- The OS-unique failure is omitting **intent** — the agent's "why am I running." This is not in any OS abstraction. It must be added.
- T5 (simplicity ↔ expressiveness): the minimal PCB plus intent plus the three-layer program description (model / prompt / tools) is the **minimal sufficient** abstraction. Anything less cannot express agent behavior; anything more violates simplicity.

**Empirical validation.**
- Define `Agent` trait, `Intent` struct, and the three-layer program description.
- Implement three agents: one tool-using, one pure-reasoning, one with long-term memory dependency. All three must inhabit the same `Agent` trait without modification.
- **Falsification:** if any of the three requires a different trait, the abstraction has failed; return to the drawing board.

### Q2 — Scheduling

**Read through the lens.**
- P1 (mechanism/policy) is decisive here: `runtime` provides `yield` / `resume` / `register_scheduler` mechanisms. Scheduler algorithms are pluggable policies outside the crate.
- T2 (fairness ↔ efficiency): fairness is measured in token budgets, not time.
- T3 (runtime power ↔ agent autonomy): the runtime decides *when* an agent runs; the agent decides *what* to do in its turn.

**Empirical validation.**
- Implement two scheduler plugins (FIFO and token-budget-priority) against the same runtime mechanisms.
- Demonstrate that swapping the scheduler does not require touching `runtime` internals.
- **Falsification:** if the runtime has to be modified to support a new scheduling strategy, P1 has been violated.

### Q3 — Context and Memory

**Read through the lens.**
- P6 (selective transparency): agents observe working-memory pressure explicitly, but do not see the mechanics of retrieval from episodic/semantic memory.
- T4 (static ↔ dynamic): memory retrieval *strategy* is pluggable; memory *tiers* are fixed mechanisms.
- The OS paging metaphor, as established in the earlier reliability assessment, is a mechanism-level mismatch. The hierarchy metaphor is structurally sound; the paging mechanism is not transferable.

**Empirical validation.**
- Implement working memory with observable pressure signals.
- Implement two retrieval strategies (recency-based, relevance-based) behind a common trait.
- Demonstrate an agent noticing its own working-memory pressure and making an explicit decision to offload.
- **Falsification:** if retrieval cannot be swapped without changing the memory crate's structure, T4 has been violated.

### Q4 — Inter-Agent Communication

**Read through the lens.**
- P3 (orthogonality): `Message`, `Channel`, `Capability` are three independent concepts. Do not merge them.
- T1 (isolation ↔ cooperation): channels are the *explicit cooperation* mechanism; without a channel, agents are isolated.
- The OS IPC metaphor transfers at the transport layer; the semantic layer (typed, intent-bearing messages) must be built.

**Empirical validation.**
- Define typed, versioned `Message` objects.
- Implement channels with explicit establishment and teardown.
- Demonstrate two agents completing a joint task using only explicit channels.
- **Falsification:** if agents ever observe each other's state without a channel, T1 has been violated.

### Q5 — Lifecycle and Hierarchy

**Read through the lens.**
- P2 (abstraction by reduction): lifecycle events are uniform across agents.
- T5 (simplicity ↔ expressiveness): DAG relationships (not strict trees) are required to express multi-parent collaboration. Trees are simpler but insufficient.

**Empirical validation.**
- Implement `spawn`, `terminate`, `delegate`, `merge`, `adopt`.
- Demonstrate an agent with multiple parents and a merged-offspring scenario.
- **Falsification:** if a realistic collaboration pattern cannot be expressed without reaching outside the lifecycle vocabulary, the vocabulary is too small.

### Q6 — Failure, Isolation, Signals

**Read through the lens.**
- P4 (end-to-end): `runtime` provides the supervisor *mechanism*; failure *detection policies* live with the supervisor agent, not in the runtime.
- T7 (performance ↔ auditability): episodic memory as replayable log is non-negotiable.
- The OS binary-failure model must be extended: supervisor trees (from Erlang) transfer directly, but the notion of "health" must go beyond alive/dead.

**Empirical validation.**
- Implement a supervisor that restarts a failed child agent.
- Demonstrate replay of an agent's behavior from its episodic log.
- Introduce a validator agent that scores the outputs of another agent; show the supervisor responding to low scores.
- **Falsification:** if we cannot answer "what did the failed agent do in its last N turns?" from the log, T7 has been violated.

---

## Part IV — Falsification Schedule

This document makes claims. Each claim should be **exposed to evidence** early, so that wrong ones are discarded before they calcify into architecture.

| Claim | How to falsify | When |
|---|---|---|
| P1 Mechanism/policy separation is the right stance | If `runtime` accumulates hardcoded strategies within the first prototype, the principle is either wrong or we cannot uphold it. | After RFC-001 code |
| P2 Single `Agent` abstraction is sufficient | If three test agents cannot inhabit the same trait, abstraction has failed. | During RFC-001 prototype |
| P3 Orthogonality of primitives | If a dependency-cycle or concept overlap appears in the type graph, orthogonality has failed. | After core types drafted |
| P4 End-to-end (runtime is dumb) | If `runtime/src/` contains semantic verbs, violated. | After every code review |
| P5 Capability model | If ambient authority appears, violated. | After first multi-agent prototype |
| P6 Selective transparency | If any boundary has no documented transparency decision, violated. | At each RFC |
| T1 Isolation default | If a spawned agent can observe another without explicit channel, violated. | After first multi-agent prototype |
| T2 Fairness observability | If we cannot answer "was this run fair?" post-hoc, violated. | After first scheduler plugin |
| T7 Auditability | If an agent's recent history cannot be replayed from episodic log, violated. | After first agent prototype |

A principle that fails its falsification test is **discarded or revised**, not defended. This document is a tool, not a belief system.

---

## Part V — How to Use This Document

This document is meant to be invoked, not just read. When writing an RFC or reviewing a design:

1. **Identify the tension.** Most design questions are instances of one of the eight tensions (T1–T8). Name it.
2. **Invoke the relevant principle.** P1–P6 usually constrain which side of the tension Chora should favor by default.
3. **State where on the balance Chora should sit.** Not "it depends" — a default position, with an escape clause for justified exceptions.
4. **Describe the empirical test.** What would show, in running code, that the chosen position is wrong?

A design discussion that does not reference at least one principle or tension has not engaged with Chora's methodology.
