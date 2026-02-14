# Documentation Convention

Every project has four documentation layers. Each serves a different audience.
Stale documentation poisons agent context. Keep all layers maintained.

## README.md — For humans
The project's public face. Written for humans: what the project does, how to set it up, how to use it. No agent jargon. If a new developer cloned the repo, README.md should get them oriented.

## .claude/CLAUDE.md — For agents
Project-specific context consumed at session start. Keep it small and actionable:
- One-sentence project description
- Tech stack with versions
- Key commands (build, test, lint) with flags — put these early
- Project structure as capabilities, not exhaustive file paths (paths go stale fast)
- Conventions and gotchas specific to this project

Budget: aim for under 150 lines. Every token loads on every request. Be ruthless about what goes here vs what belongs in docs/ or .agents/docs/.

Use progressive disclosure: reference other files (`see docs/ARCHITECTURE.md`) instead of inlining everything. Agents navigate documentation hierarchies efficiently.

## docs/ — Shared knowledge
High-level documentation readable by both humans and agents. Architecture overviews, design documents, API guides, onboarding docs. If it helps someone understand the project at a conceptual level, it belongs here.

Describe capabilities and domain concepts, not file paths. "The auth system handles OAuth + PKCE" is stable. "Auth lives in src/auth/handlers.ts" will go stale.

## .agents/docs/ — Agent working memory
The agent's persistent scratchpad. Committed to git. Survives context windows and agent rotations.

### plan.md
The living project plan. Update as scope changes, features ship, or priorities shift. Read at the start of every session.

### assumptions.md
Document assumptions before acting on them. Update as they're confirmed or invalidated. Written-down version of the Assumptions coding rule.

### notes/
Timestamped session notes in `[YYYYMMDD]-[HHMM]-slug.md` format:
- Lessons learned from debugging
- Architecture decisions and rationale
- Mistakes and how to avoid them
- Investigation findings (API quirks, library gotchas, performance)
- Failed approaches and why they didn't work

## Session Discipline
**Start of session:** Read `.agents/docs/plan.md` and `.claude/CLAUDE.md`.
**End of session:** Update both if anything changed. Write a note if you learned something worth preserving.

## Documentation Maintenance
Don't maintain documentation yourself. Spawn the `@docs-keeper` sub-agent to handle it.

Trigger a docs-keeper run when:
- You've completed a significant feature or refactor
- You notice documentation is stale or contradicts the code
- CLAUDE.md feels bloated (over 150 lines)
- A new developer would struggle to onboard from current docs
- At least once per major project milestone

The docs-keeper audits all four layers, fixes what it can, and flags what it can't.

## Rules
- One code example beats three paragraphs of explanation
- Describe capabilities, not file paths (paths go stale fast)
- Don't document what the agent already knows (language syntax, common patterns)
- Don't let CLAUDE.md grow past 150 lines — move overflow to docs/ or .agents/docs/
- Create `.agents/docs/` and subdirectories if they don't exist
