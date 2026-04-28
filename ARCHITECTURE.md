# AutoShip Architecture

## Overview

AutoShip is an OpenCode-only GitHub issue → pull request orchestration plugin.
It automates the full lifecycle of issue resolution through a pipeline of
specialized shell scripts and TypeScript types.

## System Architecture

\`\`\`
┌─────────────────────────────────────────────────────────────────┐
│                        GitHub Issues                             │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                    hooks/opencode/plan-issues.sh                 │
│  - Fetch and filter eligible issues                              │
│  - Sort by priority and exclude blocked/terminal                 │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                    hooks/opencode/dispatch.sh                    │
│  - Select model via select-model.sh                              │
│  - A/B testing group assignment                                  │
│  - Resource monitoring (CPU/memory load check)                   │
│  - Create worktree and write AUTOSHIP_PROMPT.md                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                    hooks/opencode/runner.sh                      │
│  - Execute opencode run with selected model                      │
│  - Auto-commit workspace changes                                 │
│  - Salvage truncated workers                                     │
│  - Fallback model on billing/quota failure                       │
│  - Metrics collection, circuit breaker, A/B testing              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                 hooks/opencode/process-event-queue.sh            │
│  - Reconcile completed/blocked/stuck events                      │
│  - Trigger verification and PR creation                          │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                 hooks/opencode/verify-result.sh                  │
│  - Reviewer verification (PASS/FAIL)                             │
│  - Deterministic result freshness check                          │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                 hooks/opencode/create-pr.sh                      │
│  - Generate conventional PR title and body                       │
│  - Create GitHub PR via gh CLI                                   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                    Auto-Merge (optional)                         │
│  - Monitor CI checks                                             │
│  - Merge PR after CI passes                                      │
└─────────────────────────────────────────────────────────────────┘
\`\`\`

## Key Components

### Model Routing

- \`hooks/opencode/select-model.sh\` — Selects the best model for a task type
- \`hooks/opencode/setup.sh\` — Generates model-routing.json from live OpenCode models
- \`.autoship/model-routing.json\` — Role assignments, pools, and model configurations

### Reliability Features

| Feature | Script | Description |
|---------|--------|-------------|
| Metrics Collection | \`metrics-collector.sh\` | Per-model success/failure rates, completion times, token usage |
| Circuit Breaker | \`circuit-breaker.sh\` | Disables models after >3 consecutive failures for 10 min |
| Exponential Backoff | \`retry.sh\` | Retries with 1/2/4/8/16 min delays + jitter |
| Resource Monitoring | \`resource-monitor.sh\` | Reduces concurrency if CPU/memory > 80% |
| A/B Testing | \`ab-test.sh\` | Compares model performance across groups |

### State Management

- \`.autoship/state.json\` — Central state file with issues, stats, and config
- \`.autoship/event-queue.json\` — Async event processing queue
- \`.autoship/token-ledger.json\` — Per-session token usage tracking
- \`.autoship/quota.json\` — Per-provider quota monitoring

### Type System

See \`docs/API.md\` for full TypeScript API documentation generated from \`src/types.ts\`.

Core types:
- \`Issue\` — Per-issue metadata (state, model, role, task type)
- \`StateFile\` — Top-level orchestration state
- \`ModelRouting\` — Model configuration and routing rules
- \`FailureArtifact\` — Captured failure evidence
- \`TokenRecord\` — Token usage per issue

## Data Flow

1. **Planning** — \`plan-issues.sh\` fetches GitHub issues and filters eligible ones
2. **Dispatch** — \`dispatch.sh\` creates a worktree, selects a model, and queues the issue
3. **Execution** — \`runner.sh\` runs the OpenCode worker in the worktree
4. **Monitoring** — \`monitor-agents.sh\` and \`reconcile-state.sh\` track worker health
5. **Verification** — \`verify-result.sh\` reviews the worker output
6. **Delivery** — \`create-pr.sh\` creates a PR from verified work
7. **Cleanup** — \`merge-pr.sh\` or \`cleanup-worktree.sh\` removes completed worktrees

## Safety Features

- **Prompt Injection Guard** — \`sanitize-issue.sh\`
- **Diff Size Guard** — \`diff-size-guard.sh\`
- **Anti-Flake Test Retry** — \`anti-flake.sh\`
- **Protected Label Guards** — \`classify-issue.sh\`
- **Worktree Checksum** — \`worktree-checksum.sh\`
- **Quota Guard** — \`quota-guard.sh\`

## Hooks Directory Structure

\`\`\`
hooks/
├── opencode/
│   ├── plan-issues.sh        # Issue planning and filtering
│   ├── dispatch.sh           # Worktree creation and model selection
│   ├── runner.sh             # Worker execution
│   ├── monitor-agents.sh     # Worker health monitoring
│   ├── reconcile-state.sh    # State reconciliation
│   ├── verify-result.sh      # Result verification
│   ├── create-pr.sh          # PR creation
│   ├── merge-pr.sh           # PR merge and cleanup
│   ├── select-model.sh       # Model routing logic
│   ├── setup.sh              # Initial setup wizard
│   ├── init.sh               # State initialization
│   ├── dashboard.sh          # Status dashboard
│   ├── status.sh             # Status reporting
│   ├── metrics-collector.sh  # Performance metrics
│   ├── circuit-breaker.sh    # Model reliability circuit breaker
│   ├── resource-monitor.sh   # System resource monitoring
│   ├── ab-test.sh            # A/B testing framework
│   ├── retry.sh              # Exponential backoff retry
│   ├── reviewer.sh           # Code review agent
│   ├── pr-title.sh           # PR title generation
│   ├── pr-body.sh            # PR body generation
│   └── ...
├── update-state.sh           # State mutation utility
├── capture-failure.sh        # Failure artifact capture
└── ...
\`\`\`

## Dependencies

- \`gh\` — GitHub CLI for issue/PR operations
- \`opencode\` — OpenCode CLI for agent execution
- \`jq\` — JSON processing
- \`git\` — Worktree and branch management
