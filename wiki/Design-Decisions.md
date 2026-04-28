# Design Decisions

## OpenCode-only runtime

AutoShip supports OpenCode as the only worker runtime. This keeps setup, state, dispatch, and verification paths simple and consistent.

## Role model selection

Role models are selected from the live `opencode models` inventory and stored in `.autoship/model-routing.json`. Setup prefers capable free or OpenCode Go Kimi/Kimmy/Ling 2.6-family models when available, and first-run setup asks the operator to choose orchestrator and reviewer models.

## Worker model selection

Workers are selected per task from configured live OpenCode models. The selector considers task compatibility, cost class, configured strength, previous success/failure history, and deterministic issue-number rotation across compatible workers.

## Editable model routing

`.autoship/model-routing.json` is intentionally user-editable. Setup preserves manual changes unless refresh or explicit model selection is requested.

## Verification before PR

Completed worker results are verified before AutoShip opens a pull request.
