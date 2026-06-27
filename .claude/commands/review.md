---
description: Rust code review (correctness/performance/documentation/style)
---

# Rust Code Review Checklist

## 1. Correctness
- Unsafe: Ensure unsafe blocks have safety comments and preconditions documented.
- Error Handling: Prefer anyhow for binaries, thiserror for libraries. Avoid unwrap/expect in production. Handle Result and Option with ? operator.
- Lifetimes: Elide when possible; avoid unnecessary annotations.
- Edge Cases: Handle overflow (checked/saturating), empty collections, None variants.

## 2. Performance
- Allocation: Avoid unnecessary Vec/String allocations. Prefer &str over String for parameters. Use Cow or bytes::Bytes for zero-copy.
- Iterators: Use iter/into_iter with adapters (map, filter, fold) over manual loops.
- Async: Ensure async functions are Send + 'static if spawned. Avoid blocking calls (std::thread::sleep, sync I/O). Use tokio::task::spawn_blocking for CPU-heavy work.
- Clone: Derive only when necessary. Prefer Arc<[T]> over Vec<T> for shared read-only data.

## 3. Documentation
- Public API: All public items must have /// doc comments including # Examples.
- Examples: Ensure code examples in docs compile (cargo test --doc).
- Panics: Document panic conditions with # Panics section.

## 4. Style & Idioms
- Follow Rust API Guidelines (https://rust-lang.github.io/api-guidelines/).
- Use Default derive for structs with defaults.
- Use From/Into for conversions, not as casting.
- Match exhaustively? Use _ catch-all but consider new variants.

## 5. Security
- Command injection: Use std::process::Command with args(), not sh/cmd.
- Path traversal: Use std::path::Path canonicalization.