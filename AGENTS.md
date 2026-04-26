# AutoShip Agent Guide

AutoShip is an OpenCode-only GitHub issue → pull request orchestration plugin.

See [AGENT_CATALOG.md](AGENT_CATALOG.md) for the specialized agent roles, inputs, outputs, boundaries, and default model families.

## Runtime Policy

- OpenCode is the only supported worker runtime.
- Role models are selected from live `opencode models` inventory and `.autoship/model-routing.json`; do not assume `openai/gpt-5.5` is available or preferred.
- Prefer capable free models first, then OpenCode Go role models when available; use Kimi/Kimmy 2.6 only through `opencode-go/*` unless the operator explicitly selects a paid Zen/OpenRouter model.
- `openai/gpt-5.5-fast` is not allowed.
- Worker models come from live `opencode models` inventory and are routed free-first with deterministic rotation across compatible workers.
- Default active worker cap is 15.
- Plan `agent:ready` issues in ascending issue-number order.

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
