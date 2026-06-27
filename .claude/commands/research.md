---
description: Technical research (web search / literature review / comparison)
---

# Research Protocol

## Triggers
- Unknown dependency choice (e.g., "SQLite vs SurrealDB")
- Performance issue analysis (e.g., "tokio vs async-std")
- Best practice validation (e.g., "Rust agent pattern")

## Steps
1. Search: Use brave-search MCP if available. Fallback to Python httpx crawling (via experiments/research_fetch.py).
2. Filter: Prioritize official docs (docs.rs, rust-lang.org), high-star GitHub repos (>1k stars), recent (last 12 months) blog posts / benchmarks.
3. Analysis: Create comparison table (Features, Performance, Community, Maintenance).
4. Output: Markdown report with conclusion and action items.

## Output Template
# Research: [Topic]
## Summary
[1-paragraph]
## Comparison
| Feature | Option A | Option B |
...
## Recommendation
[Choose A/B based on X]