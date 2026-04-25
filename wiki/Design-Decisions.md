# Design Decisions

## OpenCode-only runtime

AutoShip supports OpenCode as the only worker runtime. This keeps setup, state, dispatch, and verification paths simple and consistent.

## GPT-5.5 role model

`openai/gpt-5.5` is the default planner, coordinator, orchestrator, reviewer, and lead role because those steps require global judgment and consistency.

## Worker model selection

Workers are selected per task from configured live OpenCode models. The selector considers task compatibility, cost class, configured strength, and previous success/failure history.

## Editable model routing

`.autoship/model-routing.json` is intentionally user-editable. Setup preserves manual changes unless refresh or explicit model selection is requested.

## Verification before PR

Completed worker results are verified before AutoShip opens a pull request.
