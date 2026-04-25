# AutoShip Agent Guide

AutoShip is an OpenCode-only GitHub issue → pull request orchestration plugin.

See [AGENT_CATALOG.md](AGENT_CATALOG.md) for the specialized agent roles, inputs, outputs, boundaries, and default model families.

## Runtime Policy

- OpenCode is the only supported worker runtime.
- `openai/gpt-5.5` is the planner, lead, coordinator, orchestrator, reviewer, simplifier, and release role model.
- `openai/gpt-5.5-fast` is not allowed.
- Worker models come from live `opencode models` inventory and `.autoship/model-routing.json`.
- Default active worker cap is 15.
- Plan `agent:ready` issues in ascending issue-number order.
- Block unsafe/evasion-prone issues for human review.

## Local State

`.autoship/` is runtime state and must not be committed.

## Verification

Before claiming work is complete, run:

```bash
bash hooks/opencode/test-policy.sh
bash -n hooks/opencode/*.sh hooks/*.sh
bash hooks/opencode/smoke-test.sh
```

## Docs

Keep README, `docs/`, and the GitHub Wiki aligned with OpenCode-only messaging.
