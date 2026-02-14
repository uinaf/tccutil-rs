---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
  - "**/Cargo.lock"
---

# Rust

## Project Defaults
```toml
[package]
edition = "2024"
rust-version = "1.85"

[lints.rust]
unsafe_code = "warn"

[lints.clippy]
all = "warn"
pedantic = "warn"
```

## Error Handling
- Errors are values. Use `Result<T, E>` everywhere, not panics.
- `?` operator over `.unwrap()` in library code. `.unwrap()` only in tests or with a comment.
- Use `thiserror` for library errors, `anyhow` for application errors.
- Map errors with context: `.map_err(|e| MyError::Io(e))?`
- No `.expect("should work")` without a genuine invariant explanation.

## Unsafe
- Avoid unless absolutely necessary.
- Every `unsafe` block MUST have a `// SAFETY:` comment explaining the invariant.
- Prefer safe abstractions. If you need unsafe, encapsulate it behind a safe API.

## Functional Patterns
- Prefer iterators and combinators over imperative loops.
- Use `map`, `filter`, `and_then`, `unwrap_or_else` chains.
- Favor immutability. Use `let` by default, `let mut` only when needed.
- Prefer owned types in APIs. Accept `&str`/`&[T]` in parameters, return `String`/`Vec<T>`.
- Use `impl Trait` in argument position for flexibility.

## Type System
- Encode invariants in types. Make illegal states unrepresentable.
- Newtypes over primitive obsession (`struct UserId(u64)` not bare `u64`).
- Exhaustive pattern matching. No wildcard `_` on enums unless intentional.
- Derive liberally: `Debug`, `Clone`, `PartialEq` as baseline.

## Style
- `snake_case` for functions/variables, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Max 100 chars per line.
- `cargo fmt` before every commit. `cargo clippy` must be clean.

## Common Pitfalls
- `.clone()` is not free. Use references first, clone only when ownership is needed.
- Don't fight the borrow checker. If it's hard, the design might be wrong.
- Prefer `&str` over `String` in function parameters.
- Use `Arc<T>` for shared ownership across threads, `Rc<T>` only in single-threaded contexts.
