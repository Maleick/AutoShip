# AutoShip Specialized Agent Catalog

This document defines the specialized agent roles used in AutoShip orchestration.

## Agent Roles

### 1. Planner

**Purpose**: Analyzes issues and creates implementation strategies.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | GitHub issue body, acceptance criteria, project context, existing code references |
| **Outputs** | Implementation plan, task breakdown, priority assessment |
| **Boundaries** | Does not write implementation code; scope limited to planning |
| **Default Model** | `openai/gpt-5.5` (orchestration capable) |

### 2. Lead

**Purpose**: Coordinates multiple agents and manages orchestration flow.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Issue queue state, agent statuses, project priorities |
| **Outputs** | Dispatch decisions, concurrency assignments, escalation triggers |
| **Boundaries** | Does not implement code directly; delegates to specialized agents |
| **Default Model** | `openai/gpt-5.5` (orchestration capable) |

### 3. Implementer

**Purpose**: Writes and modifies code to implement features or fixes.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Issue specification, acceptance criteria, project conventions |
| **Outputs** | Source code changes, commit messages, implementation notes |
| **Boundaries** | Scope limited to the specific issue; no cross-cutting changes without approval |
| **Default Model** | `opencode/minimax-m2.5-free` (capable of code generation) |

### 4. Reviewer

**Purpose**: Reviews code changes for correctness, safety, and quality.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Changed files, diff output, test results |
| **Outputs** | Review findings, approval/block decision, improvement suggestions |
| **Boundaries** | Does not implement fixes; only approves or requests changes |
| **Default Model** | `openai/gpt-5.5` (strong reasoning) |

### 5. Simplifier

**Purpose**: Refactors code, simplifies implementations, and identifies improvements.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Source code, review feedback, complexity metrics |
| **Outputs** | Refactored code, simplification suggestions |
| **Boundaries** | Must preserve behavior; scope limited to simplification |
| **Default Model** | `openai/gpt-5.5` (strong reasoning) |

### 6. Tester

**Purpose**: Verifies implementations through tests and validation.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Source code, test frameworks, acceptance criteria |
| **Outputs** | Test results, pass/fail status, coverage reports |
| **Boundaries** | Only executes tests; does not write production code |
| **Default Model** | `opencode/minimax-m2.5-free` |

### 7. Docs

**Purpose**: Creates and maintains documentation.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Code changes, feature specifications |
| **Outputs** | Markdown documentation, README updates, API references |
| **Boundaries** | Scope limited to docs; does not modify code |
| **Default Model** | `opencode/minimax-m2.5-free` |

### 8. Release

**Purpose**: Manages releases, PRs, and version updates.

| Attribute | Definition |
|-----------|------------|
| **Inputs** | Committed changes, version files, changelog entries |
| **Outputs** | PR titles, release tags, version bumps |
| **Boundaries** | Only manages release workflow; does not implement code |
| **Default Model** | `openai/gpt-5.5` (orchestration capable) |

## Model Routing

| Role | Primary Model Family | Fallback |
|------|-------------------|----------|
| Planner | `openai/gpt-5.5` | Configured OpenCode models |
| Lead | `openai/gpt-5.5` | Configured OpenCode models |
| Implementer | `opencode/*` free models | Operator-selected models |
| Reviewer | `openai/gpt-5.5` | Configured OpenCode models |
| Simplifier | `openai/gpt-5.5` | Configured OpenCode models |
| Tester | `opencode/*` free models | Operator-selected models |
| Docs | `opencode/*` free models | Operator-selected models |
| Release | `openai/gpt-5.5` | Configured OpenCode models |

## Agent State Transitions

```
QUEUED → RUNNING → COMPLETE
              → BLOCKED
              → STUCK
```

- **QUEUED**: Issue dispatched, awaiting worker start
- **RUNNING**: Agent actively working
- **COMPLETE**: All work done and verified
- **BLOCKED**: External dependency or human required
- **STUCK**: Agent cannot proceed