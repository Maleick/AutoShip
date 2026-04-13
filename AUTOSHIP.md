---
routing:
  research: [gemini, claude-haiku]
  docs: [gemini, claude-haiku]
  simple_code: [codex-spark, gemini]
  medium_code: [codex-gpt, claude-sonnet]
  complex: [claude-sonnet, codex-gpt]
  mechanical: [claude-haiku, gemini]
  ci_fix: [claude-haiku, gemini]
quota_thresholds:
  low: 10
  exhausted: 0
stall_timeout_ms: 300000
max_concurrent_agents: 20
---

# AutoShip Configuration

Routing matrix and quota thresholds for the AutoShip orchestration system.
Edit the front matter above to configure agent assignments per task type.
