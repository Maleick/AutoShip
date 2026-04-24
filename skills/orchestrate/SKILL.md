---
name: orchestrate
description: OpenCode-only AutoShip orchestration protocol
tools: ["Bash", "Read", "Write"]
---

# AutoShip Orchestration

Use `skills/autoship-orchestrate/SKILL.md` for the current OpenCode-only protocol.

The orchestrator must use OpenCode workers only, plan issues in ascending issue-number order, cap active workers at the configured limit, and block unsafe/evasion work for human review.
