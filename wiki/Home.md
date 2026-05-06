# AutoShip Wiki

![AutoShip — Turn backlog into reviewed PRs](../assets/autoship-banner.svg)

**Turn backlog into reviewed PRs.**
AutoShip is the OpenCode plugin for solo maintainers who want their GitHub issue queue planned, routed, verified, and packaged into pull requests without babysitting every worker.

## Core Behavior

- Plans `agent:ready` issues in ascending issue-number order
- Verifies completed work before opening PRs
- Selects role models from live OpenCode inventory instead of requiring one fixed provider
- Selects worker models from the live `opencode models` inventory
- Defaults to ranked free worker models from the live OpenCode inventory and rotates compatible workers by issue number
- Includes OpenCode Go models as allowed low-cost subscription fallbacks and allows explicitly selected Spark, Nvidia, OpenRouter, Zen, and other provider models when available
- Runs up to 20 active workers by default

## Pages

| Page | Purpose |
| --- | --- |
| [Architecture](Architecture) | Runtime flow and hook responsibilities |
| [Configuration](Configuration) | `.autoship/` files and model routing |
| [Troubleshooting](Troubleshooting) | Common recovery steps |
| [Design Decisions](Design-Decisions) | Current OpenCode-only policy decisions |
