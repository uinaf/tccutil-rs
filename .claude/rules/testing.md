# Testing

## The Rule

Every project must have tests. Every feature must have tests. Every bug fix must have a regression test. No exceptions.

Target: **~90% test coverage** across all projects. If a project is below this, improving coverage is a valid task on its own.

## Test-First Workflow

1. **Bug fix?** Write a failing test that reproduces the bug FIRST. Then fix it. The test proves the fix works.
2. **New feature?** Write tests alongside or before the implementation. Tests define the contract.
3. **Refactor?** Tests must pass before AND after. If no tests exist for the code being refactored, write them first.

## What to Test

- **Public API surface** — every exported function, every endpoint, every command
- **Edge cases** — empty inputs, null/undefined, boundary values, error paths
- **Error handling** — verify errors are thrown/returned correctly, not swallowed
- **Integration points** — API calls, database queries, file I/O (mock external deps)
- **Regressions** — every bug that was fixed gets a test so it never comes back

## What Not to Test

- Private implementation details (test behavior, not internals)
- Third-party library internals
- Trivial getters/setters with no logic
- Generated code

## Test Quality

- Tests must be **readable**. A test is documentation. Someone should understand the feature by reading the test.
- Tests must be **independent**. No shared mutable state between tests. No ordering dependencies.
- Tests must be **fast**. Mock external services. Use in-memory databases for integration tests.
- Tests must be **deterministic**. No flaky tests. No timing dependencies. No random data without seeds.
- One assertion per test concept. Multiple assertions are fine if they verify one logical thing.

## Coverage

- Aim for **~90% line coverage**. 100% is not the goal — diminishing returns on trivial code.
- Coverage is a floor, not a ceiling. High coverage with bad assertions is worse than moderate coverage with good assertions.
- Uncovered code should be intentional: error handlers for impossible states, platform-specific branches, etc.
- Run coverage checks in CI when available:
  - TypeScript: `bun test --coverage` or `c8`/`v8-to-istanbul`
  - Rust: `cargo tarpaulin` or `cargo llvm-cov`

## Test Structure

### TypeScript (Bun)
```typescript
import { describe, expect, it } from "bun:test";

describe("parseConfig", () => {
  it("parses valid config", () => {
    const result = parseConfig({ port: 3000 });
    expect(result.ok).toBe(true);
  });

  it("rejects negative port", () => {
    const result = parseConfig({ port: -1 });
    expect(result.ok).toBe(false);
  });
});
```

### Rust
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_config() {
        let result = parse_config("port = 3000");
        assert!(result.is_ok());
    }

    #[test]
    fn reject_negative_port() {
        let result = parse_config("port = -1");
        assert!(result.is_err());
    }
}
```

## Pre-Commit

Tests MUST pass before committing. Include in the project's check script:
```json
"check": "bun run lint && bun run typecheck && bun run test"
```

If tests fail, fix them. Don't skip them. Don't comment them out. Don't mark them as `.todo()`.

## No Test Infrastructure?

If a project has no tests at all:
1. Set up the test framework first (`bun test` for TS, `cargo test` for Rust)
2. Add a `test` script to package.json or verify `cargo test` works
3. Write tests for existing code before adding new features
4. This is not optional. Ship tests or don't ship code.
