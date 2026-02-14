# Coding

## Composition Over Construction
Systems built from composable parts are easier to understand, test, and maintain.
Each piece should have a clear purpose and work seamlessly with others.
Composition applies at every layer: types, components, hooks, middleware, schemas, APIs.

## Railway-Oriented Programming
Model operations as pipelines where data flows through a happy path or short-circuits on failure.
- Chain operations that return Result/Either types. Errors propagate naturally.
- Parse at the boundaries (Elm decoders, Zod schemas, serde). Never trust external data.
- Make illegal states unrepresentable. Encode domain rules in types, not runtime checks.
- The boundary between your code and the outside world is a hard wall. Everything crosses through a typed parser, never a cast.

## Declarative Over Imperative
"What" matters more than "how". Prefer declarative patterns.
From UI (React) to data fetching (GraphQL/RPC) to infrastructure (IaC).

## Pragmatism Over Perfection
Great software today beats perfect software tomorrow.
Build for today, design for tomorrow. Know when to optimize and when to ship.
- Minimum code that solves the problem. Nothing speculative.
- If you write 200 lines and it could be 50, rewrite it.

## Assumptions

The single most important rule. Don't assume. Don't hide confusion. Surface everything.

Before implementing ANYTHING non-trivial:
- State your assumptions explicitly. Write them out. If uncertain, ask.
- If multiple interpretations exist, present them. Don't pick silently.
- If something is unclear, STOP. Name what's confusing. Ask.
- If a simpler approach exists, say so. Push back when warranted.
- Ask upfront: what should change, what should NOT change, what does "done" look like.

Wrong assumptions silently cascade. One bad assumption in step 1 becomes 200 wasted lines by step 10.

## Plan Before Code
Before writing or changing code: STOP.
1. Research: read docs, hit the API, understand data shapes
2. Plan: what changes, where, why. For complex tasks, write a plan with verification steps:
   ```
   1. [Step] -> verify: [check]
   2. [Step] -> verify: [check]
   ```
3. Validate: check assumptions before writing a line
4. Implement: now you have context

If mid-implementation something feels wrong: STOP and re-plan.
Sunk cost is not a reason to continue down a bad path.

## Scope Discipline
Touch only what you're asked to touch.
- Don't "clean up" code orthogonal to the task.
- Don't refactor adjacent systems as side effects.
- Don't remove comments or code you don't fully understand.
- Don't add error handling, validation, or fallbacks for scenarios that can't happen.
- Don't create helpers or abstractions for one-time operations.
- Don't design for hypothetical future requirements.
- Prefer small, focused diffs. If a change touches more than 5 files, reconsider scope.
- A bug fix doesn't need surrounding code cleaned up. A simple feature doesn't need extra configurability.
- Surgical precision, not unsolicited renovation.

## Boy Scout Rule
Leave the code you touch better than you found it.
But within scope. A small improvement in passing is good. A drive-by refactor is not.

## Autonomous Problem Solving
When given a bug report or failing test with a clear root cause: fix it. Don't ask for hand-holding.
- Read the logs, trace the error, find the root cause.
- No temporary fixes. Find and fix the actual problem.
- Zero context switching from the user when avoidable.
When the problem is genuinely ambiguous or has multiple valid approaches, ask before proceeding.

## Naive Then Optimize
For algorithmic work: implement the obviously-correct naive version first.
Verify correctness, then optimize. Never skip the correctness step.

## Investigate, Don't Speculate
Never speculate about code you haven't read. Never guess what a function does or what an API returns.
- If someone references a file, read it before responding.
- If a function name is unfamiliar, find it and read it.
- If a test is failing, read the test AND the implementation.
- If an API has an OpenAPI spec, JSON schema, or docs, read them before writing code.
- When in doubt, hit the actual endpoint and inspect the response.
Ground every claim in actual code or actual docs. Investigation prevents hallucinations.

## Divide and Conquer with Sub-Agents

For complex tasks, decompose into independent pieces. Use sub-agents to parallelize.

When a task has clearly separable parts:
- Break the work into independent sub-tasks with clear boundaries.
- Each sub-agent gets its own focused context and explicit success criteria.
- Keep the main context clean for orchestration and review.
- Sub-agents specialize: one writes, one simplifies, one verifies.

Pipeline phases for substantial features:
1. **Spec/Plan** (main): Define what, why, constraints, success criteria.
2. **Implement** (sub-agent): Execute the plan. Don't deviate without asking.
3. **Simplify** (sub-agent): Review for over-engineering, reduce complexity.
4. **Verify** (sub-agent): Test, lint, typecheck. Prove it works.

The orchestrator researches and plans. The sub-agents execute. Don't make one brain hold everything.

Use sub-agents when tasks can run in parallel, require isolated context, or involve independent workstreams.
For single-file edits, sequential operations, or tasks that share state across steps, work directly.

## Agent Teams

For multi-file tasks where work can be split by file ownership, use Agent Teams.
Describe the task and ask for a team — don't configure manually.

Teams coordinate through structure, not conversation:
- Each teammate runs in its own context window with no shared history.
- All teammates load CLAUDE.md automatically — keep it short and operational.
- Communication happens via messages + a shared task list.
- Define module boundaries in CLAUDE.md so the lead can split work cleanly.

When to use teams vs single agent:
- **Team**: Multiple independent files/modules, refactors spanning many files, cleanup tasks.
- **Single**: Single-file edits, tasks with shared state, sequential operations.

## Parallel Execution
When multiple independent operations exist, run them simultaneously.
- Reading 3 files? Read all 3 at once, not sequentially.
- Running lint + typecheck + tests? Parallelize them.
- If calls have no dependencies between them, batch them.
- If a call depends on a previous result, wait. Never guess missing parameters.

## Long-Running Tasks
For tasks spanning extended sessions or multiple context windows:
- Save progress and state explicitly. Don't rely on memory.
- Don't rush to finish because context feels limited. Work at the right pace.
- Write tests and success criteria early. Use them as checkpoints.
- Create setup scripts for resumability. The next session should pick up instantly.
- Use git as your state tracker. Commit incrementally so progress survives context loss.

## Verify Before Done
Never mark a task complete without proving it works.
- Transform tasks into verifiable goals with success criteria.
- "Add validation" → "Write tests for invalid inputs, make them pass"
- "Fix the bug" → "Write a test reproducing it, make it pass"
- Run tests, check logs, demonstrate correctness.
- If it's not verified, it's not done.
- If there's a browser, open it and test the UI. If there's a CLI, run it.
- You don't trust; you instrument.

## Pre-Commit Checks
Before every commit, run the project's `check` script. Every project should have one:

```json
"check": "bun run lint && bun run typecheck && bun run format:check && bun run test"
```

Not all projects have every check. Adapt to what's available:
- **lint**: oxlint (or eslint). Non-negotiable.
- **typecheck**: `tsc --noEmit` or `astro check`. Non-negotiable for TypeScript.
- **format:check**: oxfmt or prettier. Ensures consistent style.
- **test**: `bun test`, `cargo test`, etc. Run if tests exist.
- **knip**: Dead code detection. Run if configured.
Build is NOT part of `check` — it's slow and runs in CI. Check is for fast pre-commit validation.

If `check` script doesn't exist, run whatever lint/typecheck/test scripts are in package.json.
Zero warnings, zero errors. If a check fails, fix it before committing.

## Change Summary
After any non-trivial modification, summarize:
- **Changed:** what files, what changed, why
- **Untouched:** what you intentionally left alone
- **Risks:** anything to verify or watch out for

## Dead Code Hygiene
After refactoring, identify code that is now unreachable.
List it explicitly and ask before removing. Don't leave corpses. Don't delete without asking.

## Git

### Conventional Commits
Format: `type(scope): description`

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`, `perf`, `ci`, `build`

Rules:
- Scope is optional but encouraged
- Description: lowercase, imperative mood, no period
- Examples:
  - `feat(grid): add rebalance logic`
  - `fix: handle empty response from API`
  - `refactor(auth): extract token refresh into service`
  - `docs: update setup instructions`

### Branch Naming
Format: `type/short-description`

Examples: `feat/add-grid-engine`, `fix/empty-response`, `refactor/auth-service`

### Atomic Commits
One logical change per commit. If you need "and" in the message, split it.
Commit early, commit often. Small commits > monolith commits.

### Clean Commits
No stray `console.log`, `TODO` comments, or debug code in commits.
Review your diff before committing.

### Rebase Over Merge
Use rebase for feature branches to maintain clean linear history.
Reserve merge commits for integrating long-lived branches.

## Merge Requests
When creating MRs (GitLab) or PRs (GitHub):
- Assign yourself as the author/implementer
- Request review from the human maintainer
- Enable source branch deletion on merge
- Title follows conventional commit format
- Lint, type check, tests: all green
- Diff is small and focused
- No debug artifacts
