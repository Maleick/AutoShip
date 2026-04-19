# Soul Engine GitHub Issues — Labeling Strategy

## Label Taxonomy

### Milestone Labels
- `m11` — Soul Engine + LLM milestone
- `m10` — Economy/loot system (if referenced)

### Domain Labels
- `soul-engine` — Core Soul Engine system
- `feature:memory` — Memory store, decay, summarization
- `feature:sentiment` — Sentiment analysis, player interaction
- `feature:combat` — Combat reactions, event integration
- `feature:gossip` — Gossip propagation, relationships
- `feature:banter` — Inter-character dialogue
- `feature:personality` — Trait drift, personality evolution
- `feature:zone` — Zone awareness, environmental behavior
- `feature:mood` — Mood system, mood decay
- `feature:rate-limit` — LLM rate limiting, queueing
- `feature:speech` — Speech patterns, catchphrases, slang
- `feature:operator-control` — TUI, dashboard, admin controls

### Type Labels
- `enhancement` — New feature or capability
- `refactor` — Code structure improvement
- `bug` — Bug fix (if applicable)
- `docs` — Documentation

### Priority Labels
- `p0-critical` — Blocks M11 gate
- `p1-high` — High priority for M11 execution
- `p2-medium` — Medium priority
- `p3-low` — Nice to have

### Status Labels
- `agent:ready` — Ready for Claude/Copilot agent work
- `help-wanted` — Community contribution welcome
- `blocked` — Blocked by another issue

## Issue Classification

### Epic Issues (Parent containers)
- Issue #818 (Memory decay) → `soul-engine`, `m11`, `feature:memory`, `enhancement`, `p0-critical`
- Issue #821 (Sentiment) → `soul-engine`, `m11`, `feature:sentiment`, `enhancement`, `p1-high`
- Issue #823 (Combat reactions) → `soul-engine`, `m11`, `feature:combat`, `enhancement`, `p1-high`
- Issue #827 (Gossip) → `soul-engine`, `m11`, `feature:gossip`, `enhancement`, `p2-medium`
- Issue #829 (Banter) → `soul-engine`, `m11`, `feature:banter`, `enhancement`, `p2-medium`
- Issue #832 (Personality drift) → `soul-engine`, `m11`, `feature:personality`, `enhancement`, `p2-medium`
- Issue #835 (Zone awareness) → `soul-engine`, `m11`, `feature:zone`, `enhancement`, `p1-high`
- Issue #837 (Mood decay) → `soul-engine`, `m11`, `feature:mood`, `enhancement`, `p0-critical`
- Issue #841 (Rate limiting) → `soul-engine`, `m11`, `feature:rate-limit`, `enhancement`, `p0-critical`
- Issue #846 (Speech evolution) → `soul-engine`, `m11`, `feature:speech`, `enhancement`, `p2-medium`
- Issue #850 (Operator controls) → `soul-engine`, `m11`, `feature:operator-control`, `enhancement`, `p1-high`

### Slice Issues (Specific implementation tasks)
Each slice inherits parent domain + adds specific labels:
- #1018 (Memory decay core) → `soul-engine`, `m11`, `feature:memory`, `enhancement`, `p0-critical`
- #1023 (Memory summarization) → `soul-engine`, `m11`, `feature:memory`, `enhancement`, `p0-critical`
- #1030 (Memory context) → `soul-engine`, `m11`, `feature:memory`, `enhancement`, `p0-critical`
- #1037 (Sentiment scorer) → `soul-engine`, `m11`, `feature:sentiment`, `enhancement`, `p1-high`
- #1044 (Sentiment integration) → `soul-engine`, `m11`, `feature:sentiment`, `enhancement`, `p1-high`
- #1058 (Combat event injection) → `soul-engine`, `m11`, `feature:combat`, `enhancement`, `p1-high`
- #1059 (Mood decay) → `soul-engine`, `m11`, `feature:mood`, `enhancement`, `p0-critical`
- #1068 (Rate limiting) → `soul-engine`, `m11`, `feature:rate-limit`, `enhancement`, `p0-critical`
- #1076 (Gossip handler) → `soul-engine`, `m11`, `feature:gossip`, `enhancement`, `p2-medium`
- #1084 (Personality drift) → `soul-engine`, `m11`, `feature:personality`, `enhancement`, `p2-medium`
- #1092 (Zone awareness) → `soul-engine`, `m11`, `feature:zone`, `enhancement`, `p1-high`
- #1106 (Banter triggering) → `soul-engine`, `m11`, `feature:banter`, `enhancement`, `p2-medium`
- #1112 (Speech evolution) → `soul-engine`, `m11`, `feature:speech`, `enhancement`, `p2-medium`
- #1120 (TUI soul panel) → `soul-engine`, `m11`, `feature:operator-control`, `enhancement`, `p1-high`
- #1130 (Sentiment relationships) → `soul-engine`, `m11`, `feature:sentiment`, `enhancement`, `p1-high`

## Dependency Tracking via Labels

Use these conventions in issue descriptions:
- **Blocks**: Link to issues this blocks
- **Depends on**: Link to issues this depends on
- **Related**: Cross-references for context

Example in issue body:
```
## Dependencies
- Blocks: #841 (Rate limiting)
- Depends on: #1037 (Sentiment scorer)
- Related: #827 (Gossip)
```

## Query Examples

Find all critical-path M11 work:
```
is:open label:m11 label:p0-critical
```

Find all memory system work:
```
is:open label:feature:memory
```

Find sentiment-related issues:
```
is:open label:feature:sentiment
```

Find all enhancement issues for M11:
```
is:open label:m11 label:enhancement
```
