---
paths:
  - "**/*.ts"
  - "**/*.tsx"
  - "**/*.mts"
  - "**/*.cts"
---

# TypeScript

## Functional First
- Pure functions by default. Minimize side effects.
- Isolate side effects at the edges (IO, network, DOM). Keep the core pure.
- Prefer immutable data. Use `readonly`, `as const`, spread over mutation.
- Composition over inheritance. Pipe small functions into larger ones.
- Prefer `map`/`filter`/`reduce` over imperative loops.

## Type-Driven Development
Types are the source of truth. The type system encodes business rules, domain constraints, and data flow. If the types are right, the code is right.

- `strict: true` always. No exceptions.
- No `any`. Use `unknown` and narrow with type guards.
- No `as` casts. Ever. Use `satisfies`, type guards, or Zod parsing instead.
- No `!` non-null assertions. Ever. Use narrowing, optional chaining, or proper null checks.
- **Make illegal states unrepresentable.** Use discriminated unions, branded types, and exhaustive switches.
- **Schema-first design.** Define Zod schemas first, derive types with `z.infer<>`. The schema IS the type AND the runtime validator.
- `interface` for objects, `type` for unions/intersections.
- Colocate types with usage. No god `types.ts` files.
- Types are contracts. They communicate intent and enforce correctness.
- If you're writing runtime checks that duplicate what the type system could enforce, redesign the types.

## Errors as Values
- Errors are first-class citizens, not afterthoughts.
- Use Result pattern (`{ ok, data } | { ok, error }`) or Zod `.safeParse()`.
- Throw only for truly exceptional/unrecoverable cases.
- No empty catch blocks. Always handle the error case.
- Prefer early returns over deeply nested conditionals.

## Parse, Don't Validate
Use Zod at every external boundary. Define the schema, derive the type, parse at the boundary. One source of truth.

```typescript
// Schema is the source of truth
const UserSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  role: z.enum(["admin", "user"]),
});
type User = z.infer<typeof UserSchema>;

// Parse at the boundary
const user = UserSchema.parse(await response.json()); // good
const user = (await response.json()) as User; // banned
```

Parse these. Always:
- API responses, form data, env vars, URL params, config files
- Anything from outside your trust boundary
- Between service layers when data shape changes

## Runtime: Bun
- Bun as default runtime, not Node.js.
- `Bun.serve()` for HTTP/WS, `bun:sqlite` for SQLite.
- `Bun.file()` over `node:fs`. Bun auto-loads `.env`.
- `bun test` for testing, `bun build` for bundling.

## Tooling
- **oxlint** for linting (not ESLint). Fast, zero-config, catches real bugs.
- **oxfmt** for formatting (or Biome). Deterministic, no debates.
- Run both in CI. No lint warnings in main branch.

## Naming
- Functions: `camelCase`. Types: `PascalCase`. Constants: `UPPER_SNAKE_CASE`.
- Booleans: prefix with `is`/`has`/`should` (`isLoading`, `hasError`).
