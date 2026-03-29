# Knowledge Base — Frostreaver

A self-improving knowledge system. Insights start as hypotheses, get promoted to rules after 5+ confirmations, and get demoted back when contradicted by new data.

## Domain Routing

| Domain | Path | Description |
|--------|------|-------------|
| EQ Internals | [eq-internals/](eq-internals/) | EQ memory layout, CXWnd, CXStr, vtables, thread model |
| Login Automation | [login-automation/](login-automation/) | Credential entry, EULA bypass, server/char select |
| Combat | [combat/](combat/) | Class strategies, pulling, aggro, GCD, mana |
| Navigation | [navigation/](navigation/) | Waypoints, navmesh, stuck detection, movement |

## How It Works

1. **Before starting a task**: Review existing rules and hypotheses for the domain
2. **During work**: Check if any hypothesis can be tested
3. **After task**: Extract insights → knowledge.md (facts), hypotheses.md (need more data), rules.md (confirmed)
4. **Promotion**: Hypothesis confirmed 5+ times → promoted to rule
5. **Demotion**: Rule contradicted by new data → demoted to hypothesis
