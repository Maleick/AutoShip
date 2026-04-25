# Improve Panel: Self-Improvement Loop UX

## Architecture

The Improve panel implements UX patterns from Warcraft Logs, FFLogs, Raidbots, WoWAnalyzer, and EQLogParser. All data is stored locally; no cloud upload, no leaderboards.

## Core Concepts

### Camp Fingerprint

Deterministic hash of: `zone + mob set + party comp + level range`. Groups sessions by identical farming conditions, enabling "compare to your best" without global leaderboards.

### Personal Baseline (Time-Decay)

Stores top-N sessions per camp+metric. Marked stale after 60 days or a recorded patch event. Pinning a session as "gold standard" makes it the active baseline.

### Noise Floor

Configurable epsilon (default 1%). Suggestions within the noise band show "≈ personal best" instead of a delta value, preventing superstitious chasing of variance.

### Severity Tiers

- **Major**: High impact, low noise. Always visible.
- **Average**: Moderate impact. Always visible.
- **Minor**: Low impact or niche. Hidden by default behind toggle.

The single most important anti-nag pattern across all surveyed tools.

### Checklist Twin

Every suggestion has a companion checklist row showing what _worked_. Leads with wins before criticism.

## Components (15 total)

### UI Controls

- **SeverityToggle**: Show/hide Minor suggestions
- **FilterChipBar**: Per-character / per-ability toggles (shared across Timeline + EventList)
- **CampPicker**: Filter sessions by camp/zone

### Data Display

- **MetricCard**: Single KPI tile with delta vs personal baseline
- **SuggestionCard**: Severity-tagged, title + evidence + delta, dismiss button
- **Checklist**: Collapsible per-character pass/fail rows with progress bars

### Views

- **TabBar**: DPS / Healing / CC / Deaths / Resources / Pulls facets
- **Timeline**: Time axis, swimlanes per character, ability glyphs, filter chips
- **TimelineScrubber**: Cursor + range-select bound to all sibling views
- **EventList**: Virtualized raw-log table with column filters and search
- **ComparisonChart**: Two-series overlay (current vs personal-best) with noise-floor shading
- **HighlightReel**: Horizontal scrollable strip of flagged moments (wins + losses)
- **ReplayMap**: Optional 2D top-down position replay (v2, stub)

### Containers

- **SessionDetail**: Tabbed shell hosting Overview / Timeline / Events
- **SessionList**: List of recorded sessions (date, camp, duration, metrics)
- **ImprovePanel**: Top-level component

## Data Types

```typescript
Session
├── camp: { zone, mobSet[], partyComp[], levelRange }
├── duration, xpPerHour, metrics: { dps?, healing?, deaths? }
└── buildSha, tlpRuleset

PersonalBest
├── campId, metric: "xphour" | "dps" | "deaths"
├── recordedAt, staleAt (time-decay)
└── isPinned

Suggestion
├── severity: "major" | "average" | "minor"
├── title, evidence
├── deltaValue, deltaType: "dps" | "healing" | "deaths" | "xphour"
└── timestamp (log offset for "Jump to log")

ChecklistItem
├── character, description
├── passed: boolean (true = positive, false = needs work)
└── progress: 0-100

LogEvent
├── timestamp, type, actor, action, target
└── details: Record<string, unknown>
```

## Utilities

- `computeCampFingerprint()`: Hash zone + mobs + party comp + level range
- `isBaselineStale()`: Check time-decay or explicit stale date
- `findPersonalBest()`: Locate baseline for a camp+metric, prefer pinned
- `computeDelta()`: Calculate vs personal best, suppress within noise floor
- `formatDuration()`, `formatMetric()`: Human-readable output

## Anti-Patterns Forbidden

✗ Mandatory account / cloud upload
✗ Tiered upsell gating insights
✗ Global leaderboards / percentile shaming
✗ Ad-supported chrome / "rate this analysis"
✗ Mid-session nagging / interrupting overlays

All data is local, operator-controlled, and always accessible.

## Usage

```typescript
import { ImprovePanel, Session, PersonalBest, Suggestion } from "./components/Improve";

const sessions: Session[] = [
  {
    id: "abc123",
    date: "2026-04-25T14:30:00Z",
    camp: {
      zone: "Cazic-Thule",
      mobSet: ["Cazic", "Gelatinous"],
      partyComp: ["Cleric", "Enchanter", "Necro", "Mage", "Mage", "Mage"],
      levelRange: "65-68",
    },
    duration: 3600000, // 1 hour
    xpPerHour: 485000,
    metrics: { dps: 1200, healing: 850, deaths: 0 },
  },
];

const suggestions = new Map<string, Suggestion[]>([
  [
    "abc123",
    [
      {
        id: "s1",
        severity: "major",
        title: "Missing mana check rotation",
        evidence: "Cleric OOM twice, pulling could wait",
        deltaValue: -5.2,
        deltaType: "xphour",
      },
    ],
  ],
]);

<ImprovePanel sessions={sessions} suggestions={suggestions} personalBests={[]} />;
```

## v1 Scope → v2 Roadmap

✓ v1: Severity tiers, checklist twin, noise floor, camp fingerprint, time-decay
✓ v1: Tables, Timeline (functional), Events (functional) views
✓ v1: All 15 component primitives
✓ v1: No cloud, no leaderboards, no mid-session modals

→ v2: Timeline animation, ability glyphs, better replay
→ v2: ReplayMap full 2D animation with position history
→ v3: Integration with combat-events schema updates from #2596
→ v3: EQ knowledge layer integration (item upgrades, pop windows, loot ROI)
