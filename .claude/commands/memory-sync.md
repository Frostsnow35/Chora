---
description: Cross-task memory inheritance (working/episodic/semantic)
---

# Memory Sync Protocol

## Levels
1. Working: Per-task scratchpad (variables, temporary decisions). Stored in memory, not persisted.
2. Episodic: Compressed summaries of completed tasks (goal, outcome, errors, key observations). Stored in SQLite (episodic table).
3. Semantic: Long-term knowledge derived from episodes. Schema, architecture decisions, performance benchmarks. Stored in SQLite (semantic table).

## Workflow
- Start: Load latest episodic context (last 5 entries) into working memory.
- During: Update working memory with new findings.
- End: Summarize working memory into 3 bullet points. Append to episodic SQLite.
- Conflict: If new semantic conflicts with old, trigger research skill to validate.

## SQLite Schema (reference)
CREATE TABLE episodic (
    id INTEGER PRIMARY KEY,
    timestamp INTEGER,
    summary TEXT,
    tags TEXT
);
CREATE TABLE semantic (
    id INTEGER PRIMARY KEY,
    key TEXT UNIQUE,
    value TEXT,
    confidence REAL
);