# Soul Engine M11 Implementation Roadmap

**Status**: Phase 1 Research Complete → Phase 2 (M11) Planning Complete  
**Last updated**: 2026-04-12  
**Branch**: `claude/define-soul-system-W8Gbb`

---

## Executive Summary

The Soul Engine is TextQuest's character personality and behavioral autonomy layer. Phase 1 is complete and functional with deterministic fallback responses. M11 will integrate local LLM inference (ollama/Gemma 4) and expand social/behavioral complexity.

This document maps all M11 work into:
- **11 Epic issues** (domain containers)
- **15+ Slice issues** (specific implementation tasks)
- **Comprehensive labeling** for tracking and querying
- **Execution priorities** (P0-P3)

---

## Phase 1 vs Phase 2

### Phase 1 (Current — COMPLETE)

✅ Implemented and tested:
- Personality trait system (Big Five + EQ-themed)
- 10-state mood machine with event-driven transitions
- SQLite memory store (schema for decay/summaries)
- Social relationship graph (faction/trust)
- Idle behavior scheduler (trait-weighted)
- LLM provider trait abstraction
- Fallback responder (deterministic)
- Configuration framework
- 40+ unit tests

### Phase 2 (M11 — PLANNED)

🚀 Implementation roadmap:
- Local LLM integration (ollama, Gemma 4)
- Memory decay and auto-summarization
- Richer sentiment analysis (keyword + future LLM)
- Personality drift (traits evolve)
- Gossip propagation (three-party dynamics)
- Inter-character banter (social interaction)
- Zone-aware behaviors (environment-based)
- Natural mood decay (emotional normalization)
- Combat reaction system (event integration)
- Rate limiting and async queueing
- Speech pattern evolution (catchphrases, slang)
- Operator controls (TUI + dashboard)

---

## Epic Issues (11 total)

### Critical Path (P0 — Gates M11 Entry)

| Issue | Epic | Slices | Status |
|-------|------|--------|--------|
| #818 | Memory decay and auto-summarization | 3 | Design ✅ Slices Created ✅ |
| #837 | Natural mood decay and neutral drift | 1 | Design ✅ Slice Created ✅ |
| #841 | LLM request rate limiting and queueing | 1 | Design ✅ Slice Created ✅ |

### High Priority (P1 — Early M11)

| Issue | Epic | Slices | Status |
|-------|------|--------|--------|
| #821 | Player sentiment analysis | 3 | Design ✅ Slices Created ✅ |
| #823 | Combat reactions integration | 1 | Design ✅ Slice Created ✅ |
| #835 | Zone-aware idle behaviors | 1 | Design ✅ Slice Created ✅ |
| #850 | Operator controls (TUI/Web) | 1 | Design ✅ Slice Created ✅ |

### Medium Priority (P2 — Mid M11)

| Issue | Epic | Slices | Status |
|-------|------|--------|--------|
| #827 | Gossip propagation | 1 | Design ✅ Slice Created ✅ |
| #829 | Inter-character banter | 1 | Design ✅ Slice Created ✅ |
| #832 | Personality drift | 1 | Design ✅ Slice Created ✅ |
| #846 | Speech pattern evolution | 1 | Design ✅ Slice Created ✅ |

---

## Slice Issues (15+ total)

### Memory System (P0-Critical) — 3 slices

| Issue | Title | Dependencies |
|-------|-------|---|
| #1018 | Memory decay: Mark old memories and apply weight reduction | — |
| #1023 | Memory summarization: Periodic auto-generation | #1018 |
| #1030 | Memory context: Prioritization and recency scoring | #1018, #1023 |

### Sentiment Analysis (P1-High) — 3 slices

| Issue | Title | Dependencies |
|-------|-------|---|
| #1037 | Sentiment scorer: Keyword-based text analysis | — |
| #1044 | Sentiment integration: Record in memory and drive mood | #1037 |
| #1130 | Sentiment impact: Relationship faction updates | #1044 |

### Combat Reactions (P1-High) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1058 | Combat reactions: Event injection from combat coordinator | #821 (sentiment) |

### Mood System (P0-Critical) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1059 | Mood decay: Natural drift toward neutral state | — |

### Rate Limiting (P0-Critical) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1068 | Rate limiting: Per-character and global LLM request limits | — |

### Gossip (P2-Medium) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1076 | Gossip: Three-party relationship propagation | #829 (banter) |

### Personality (P2-Medium) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1084 | Personality drift: Trait changes driven by events | — |

### Zone Awareness (P1-High) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1092 | Zone awareness: Location-based behavior constraints | — |

### Banter (P2-Medium) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1106 | Inter-character banter: Proximity and relationship-based dialogue | — |

### Speech Evolution (P2-Medium) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1112 | Speech evolution: Catchphrase adoption and slang contagion | — |

### Operator Controls (P1-High) — 1 slice

| Issue | Title | Dependencies |
|-------|-------|---|
| #1120 | Operator controls: TUI soul panel and emergency controls | — |

---

## Label Taxonomy

### All issues tagged with:
- `soul-engine` — Core system label
- `m11` — Milestone label
- `enhancement` — Type (all are new features)

### Domain-specific labels:
- `feature:memory` — Memory system (issues #1018, #1023, #1030)
- `feature:sentiment` — Sentiment analysis (issues #1037, #1044, #1130)
- `feature:combat` — Combat reactions (issue #1058)
- `feature:mood` — Mood system (issue #1059)
- `feature:rate-limit` — Rate limiting (issue #1068)
- `feature:gossip` — Gossip (issue #1076)
- `feature:personality` — Personality drift (issue #1084)
- `feature:zone` — Zone awareness (issue #1092)
- `feature:banter` — Inter-character dialogue (issue #1106)
- `feature:speech` — Speech patterns (issue #1112)
- `feature:operator-control` — Operator controls (issue #1120)

### Priority labels:
- `p0-critical` — Blocks M11 gate (3 issues)
- `p1-high` — Early M11 execution (4 issues + 4 slices)
- `p2-medium` — Mid M11 execution (4 issues + 4 slices)

---

## Recommended Execution Order

### Phase 2a — Foundation (Weeks 1-2)

Critical path must complete first; enables all downstream work.

1. ✅ #1018 — Memory decay core logic
2. ✅ #1023 — Memory summarization
3. ✅ #1030 — Memory context prioritization
4. ✅ #1059 — Mood decay
5. ✅ #1068 — Rate limiting

**Outcome**: Memory system production-ready, mood realistic, LLM requests rate-limited.

### Phase 2b — Interaction (Weeks 3-4)

Sentiment and combat reactions unlock player interaction.

6. ✅ #1037 — Sentiment scorer
7. ✅ #1044 — Sentiment integration
8. ✅ #1130 — Sentiment relationships
9. ✅ #1058 — Combat event injection
10. ✅ #1092 — Zone awareness

**Outcome**: Characters react to players and combat, adapt to zones.

### Phase 2c — Social Dynamics (Weeks 5-6)

Build out relationship complexity and social behaviors.

11. ✅ #1076 — Gossip propagation
12. ✅ #1106 — Inter-character banter
13. ✅ #1084 — Personality drift
14. ✅ #1112 — Speech evolution

**Outcome**: Characters develop unique relationships, speech patterns, personalities.

### Phase 2d — Operator Integration (Week 7)

Visibility and control for the operator.

15. ✅ #1120 — TUI soul panel
16. ✅ Web dashboard integration (future)

**Outcome**: Operator can monitor and control soul system safely.

---

## GitHub Query Patterns

### Find all M11 work:
```
is:open label:m11
```

### Find critical path (P0):
```
is:open label:m11 label:p0-critical
```

### Find sentiment-related issues:
```
is:open label:feature:sentiment
```

### Find high-priority items:
```
is:open label:m11 label:p1-high OR label:p0-critical
```

### Find memory system work:
```
is:open label:feature:memory
```

---

## Dependencies and Blocking

### Critical dependency chain:

```
#1018 (Memory decay)
  ↓
#1023 (Summarization)
  ↓
#1030 (Context)
  ↓
#1068 (Rate limiting) — can work in parallel
#1059 (Mood decay) — can work in parallel
  ↓
#1037 (Sentiment scorer)
  ↓
#1044 (Sentiment integration)
  ↓
#1130 (Sentiment relationships)
  ↓
#1058 (Combat reactions)
  ↓
#1106 (Banter)
  ↓
#1076 (Gossip)
```

### Parallel work streams:

- **Memory track**: #1018 → #1023 → #1030 (weeks 1-2)
- **Mood/LLM track**: #1059, #1068 (parallel, week 1-2)
- **Sentiment track**: #1037 → #1044 → #1130 (weeks 3-4)
- **Combat track**: #1058 (week 3-4, depends on #821)
- **Zone track**: #1092 (week 3-4, independent)
- **Social track**: #1106, #1084, #1112 (weeks 5-6, depend on sentiment)
- **Gossip track**: #1076 (week 6, depends on #1106)
- **Operator track**: #1120 (week 7, independent)

---

## Success Criteria

### Phase 2a (Foundation) Complete When:
- Memory decay marks old memories correctly
- Memory summaries auto-generate on schedule
- Context window prioritization works
- Mood naturally decays toward neutral
- LLM requests are rate-limited and queued

### Phase 2b (Interaction) Complete When:
- Sentiment scores player messages accurately
- Mood changes driven by sentiment
- Relationships update based on sentiment
- Combat coordinator integrates with soul
- Idle behaviors respect zone constraints

### Phase 2c (Social) Complete When:
- Gossip propagates between three parties
- Characters banter in same zone
- Personality traits drift based on experiences
- Characters adopt catchphrases and slang
- Speech styles are unique and evolving

### Phase 2d (Operator) Complete When:
- TUI panel shows soul state clearly
- Operator can enable/disable soul per character
- Operator can mute or reset moods
- Web dashboard has soul monitoring tab
- Audit trail exists for all soul actions

---

## Files Created

| File | Purpose |
|------|---------|
| `docs/wiki/Soul-Engine-Definition.md` | Complete architecture definition |
| `docs/wiki/Soul-Engine-Issue-Labels.md` | Labeling strategy and taxonomy |
| `docs/wiki/Soul-Engine-M11-Roadmap.md` | This file — execution roadmap |

---

## Conclusion

M11 is a well-scoped, clearly prioritized implementation roadmap with 15+ granular issues, comprehensive labeling, and explicit dependencies. The critical path (memory + mood + rate-limiting) can be completed in 2 weeks, unlocking downstream social and behavioral work.

All issues have:
- ✅ Clear acceptance criteria
- ✅ Implementation guidance
- ✅ Test specifications
- ✅ Configuration defaults
- ✅ Domain labeling
- ✅ Priority assignment
- ✅ Dependency tracking

Ready for team execution.
