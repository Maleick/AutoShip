# Roadmap and Known Gaps

## Milestone Status

Current repo milestone framing:

- `M1`: external memory reading and TUI dashboard
- `M2`: DLL injection, hooks, IPC, self-healing monitor
- `M2.5`: login automation and launch coordination
- `M3`: navigation and route handling
- `M4`: combat automation
- `M5`: Soul Engine
- `M6`: live provider-backed character AI and richer chat behavior
- `M7`: learning and RL work
- `M8`: economy automation

Repository docs and code currently treat M1 through M5 as implemented, with M6 next.

## What Is Current Today

- TUI with four primary screens
- demo mode for non-Windows and no-client workflows
- live Windows injection and IPC pipeline
- login automation structure and in-client login logic
- navmesh-backed routing and map overlays
- combat FSM plus class strategies and CH chain
- Soul Engine with deterministic fallback and persistent memory

## Known Gaps That Still Need Live Validation

- live EQ login widgets and phase transitions after client patches
- offset stability after upstream EQ updates
- cross-zone nav behavior in more zones than the current dev/test set
- class-by-class combat tuning under real combat load
- large-scale multibox validation closer to the 36-client target
- post-login group sequencing and camp setup under real launch conditions

## Current M6 Boundary

The code already has:

- an LLM provider trait
- request and priority queue types
- fallback response generation

What it does not yet claim as current:

- fully integrated real provider calls for routine live character chat behavior
- a validated operator workflow around external AI credentials, quotas, and failures

## Developer Guidance

When writing docs or PR descriptions:

- mark implemented-but-not-revalidated behavior clearly
- keep roadmap claims separate from current operator instructions
- prefer "current code does X" over "the system will eventually X"

## Current Behavior vs Roadmap

### Current behavior

- The project is already well past prototype status in structure and surface area.

### Validation notes

- The biggest remaining uncertainty is not whether the features exist in code, but how broadly and recently they were validated on live EQ.
