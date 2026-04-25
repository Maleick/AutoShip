# Self-Improvement Loop Deep Research — 2026-04-25

**Branch**: `claude/audit-textquest-openvanilla-HPifL`
**PR**: #2618
**Predecessor**: `docs/openvanilla-redguides-2026-04-25-audit.md`

This doc captures a second research pass after the first 18 issues were filed. The first pass set up the self-improvement loop epic (#2594) with 8 sub-issues. This pass deepens the design in three dimensions.

## Findings

### F1 — The proposed `sessions.db` schema is too thin

EQLogParser, Gamparse, and rumstil/eqlogparser all carry a substantially richer event taxonomy than the `combat_events`/`heal_events`/etc. tables proposed in #2596. Concrete deltas:

- No attacker/defender separation (we only have `mob_name`); pet attribution is impossible.
- No damage `type` / `sub_type` (Melee / DD / DoT / Proc / DamageShield / Riposte).
- No modifier flags (Critical, Lucky, Twincast, Rampage, Assassinate, Headshot, FinishingBlow, DoubleBow, Flurry, Strikethrough, Riposte, Slay) — EQLogParser carries a 12-bit modifier mask on every damage event.
- No outcome enum (hit / miss / dodge / parry / block / riposte / strikethrough / immune / absorb / invuln).
- No resist tracking (element + partial-resist amount).
- No spell lifecycle (begin / success / interrupt / fizzle / resist / reflect / wear-off).
- **No heal table at all** — gap-of-record for a multibox controller.
- No buff-uptime tracking.
- No taunt / mez-break / random-roll / zone-change / ding / faction-change events.
- No raw-log offset for replay back to source line.

Fields TextQuest *should keep* that the parsers don't have:

- `hp_pct_*` / `mana_pct` / `target_distance` (memory-read, not log-derived).
- `stuck_events` (pathing failures — controller-only signal).
- `route_costs` (per-edge route quality — genuinely novel).
- `rotation_ticks` (intent-to-cast vs. the eventual cast outcome — controller-only upstream signal).
- `pulls.time_to_engage_ms` and `pulls.success` (intent-vs-outcome on pulls).
- `build_sha` per session (A/B testing controller versions).

### F2 — The `/improve` panel scope (#2602) was undersized

Common UX patterns across Warcraft Logs, FFLogs, Raidbots, WoWAnalyzer, OpenDota, Mobalytics, Slippi, Gamparse, EQLogParser:

- **List → Detail → Drilldown hierarchy**. Sessions list → session detail → tab facets (DPS / Healing / Casts / Resources / Deaths / Pulls).
- **Three rendering modes** for the same data: Tables, Timelines, Events.
- **Severity tiers (Major / Average / Minor)** with Minor hidden by default — the single most important anti-nag pattern (WoWAnalyzer).
- **Checklist twin to suggestions** — leads with what worked, not what failed.
- **Quantified deltas with noise floor** (Raidbots' 0.1% sidegrade rule) — suppress noise.
- **Auto-flagged moments** with click-through to log timestamp (Mobalytics Smart Highlights, Warcraft Logs Deaths tab).
- **"Compare to your best"** via a camp fingerprint = `hash(zone + mob set + party comp + level range)` — gives leaderboard pressure without leaderboards.
- **Time-decay weighting** on personal bests so post-patch records don't haunt forever.

Anti-patterns to explicitly forbid:

- Mandatory account / cloud upload.
- Tiered upsell gating insights.
- Global leaderboards / percentile shaming (toxic for solo multiboxers).
- Ad-supported chrome.
- Mid-session nagging / interrupting overlays.

### F3 — The EQ knowledge layer is its own epic

The self-improvement loop alone is incomplete. To convert raw events into operationally useful suggestions ("you missed three Cazic-Thule pop windows", "you skipped Ssraeshza Helm where your tank's slot is upgrade-eligible"), TextQuest needs an authoritative EverQuest knowledge layer: items, spells, NPCs, spawns, loot tables, factions, zones.

The right seed is the EQEmu PEQ nightly SQL dump (`db.projecteq.net`) — open-source, GPL, 30 days of nightly + monthly archives. Per-server differences (P99, Project Lazarus, TAKP, EQ Might) live in YAML overlays. Recent expansions absent from PEQ get a Lucy CSV overlay. Spawn-timer corrections live in a community-curated YAML refined by session kill data.

This unlocks (at minimum):

- Item upgrade scoring (MQ2ItemScore parity).
- Loot expectation values (camp ROI in pp/hr per character).
- Named pop-window prediction with confidence interval.
- Quest-reward vs vendor-value comparison.
- Faction-aware vendor selection.
- Spell DPM efficiency analysis.

Because the data is reusable by combat AI, vendor automation, and the help system — not just suggestions — it should be a **sibling epic to #2594, not a child**.

## Action items

### Update #2596 — schema additions

Add to `combat_events`:
- `attacker TEXT`, `defender TEXT`, `attacker_owner TEXT`, `defender_owner TEXT` (pet attribution)
- `dmg_type TEXT` (Melee/DD/DoT/Proc/DS/Riposte)
- `outcome TEXT` (hit/miss/dodge/parry/block/riposte/strikethrough/immune/absorb/invuln)
- `modifiers INTEGER` (12-bit mask: Critical/Lucky/Twincast/Rampage/Assassinate/Headshot/FinishingBlow/DoubleBow/Flurry/Strikethrough/Riposte/Slay)
- `resist TEXT`, `partial_resist_pct REAL`
- `raw_line_offset INTEGER`

New tables:
- `heal_events(session_id, ts, healer, healed, spell, type, amount, overheal, modifiers)`
- `spell_events(session_id, ts, caster, target, spell, phase, interrupted_by, ambiguity_json)`
- `buff_uptime(session_id, target, spell, applied_ts, faded_ts, source)`
- `taunt_events`, `mez_break_events`, `random_rolls`
- `zone_events(session_id, ts, zone, x, y, z)` — replaces `sessions.zone_seq TEXT`
- `xp_events(session_id, ts, level_after, aa_after)`
- `faction_events(session_id, ts, faction, delta, zone)`

Existing-table additions:
- `pulls`: `aggro_count INTEGER`, `unintended_adds INTEGER`
- `deaths`: `attacker TEXT`, `attacker_max_hit INTEGER`, `last_5_damage_json TEXT`
- `rotation_ticks`: `cooldown_remaining_ms INTEGER`, `reason_failed TEXT` (oom/gcd/range/los/target-dead/interrupted)
- `loot`: `looter TEXT`, `is_currency INTEGER`, `quantity INTEGER`, `source TEXT`
- `sessions`: `client_version TEXT`, `tlp_ruleset TEXT`

### Update #2602 — UX refinements

Add to scope:
- Severity tiers (Major / Average / Minor) with Minor hidden by default.
- Checklist twin panel rendered alongside the suggestion list.
- Quantified deltas with noise-floor suppression (configurable epsilon, default 1%).
- Three views over the same session: Tables / Timeline / Events.
- Camp fingerprint as the join key for "compare to your best".
- Time-decay weighting on baselines (stale badge after 60 days or after a recorded patch event).
- 15 React component primitives: SessionList, SessionDetail, TabBar, MetricCard, Checklist, SuggestionCard, HighlightReel, Timeline, TimelineScrubber, EventList, ComparisonChart, ReplayMap (v2), SeverityToggle, FilterChipBar, CampPicker.

Forbidden by design:
- Cloud upload (local-only).
- Account requirement.
- Global leaderboards.
- Ad chrome / upsell gating.
- Mid-session modals.

### New epic: EQ Knowledge Layer

Sibling to #2594. Tree:

```
Epic: EQ Knowledge Layer
├── Schema and ingestion
│   ├── PEQ nightly dump → SQLite (content tables only)
│   ├── Bitmask views (slots, classes, races, deity)
│   ├── Per-server overlay loader (P99/Laz/TAKP YAML)
│   ├── Lucy CSV overlay for post-PEQ expansions
│   └── eq_data_history audit table + diff CLI
├── Spawn-timer DB
│   ├── Curated YAML schema (npc_id, zone, respawn, variance, source)
│   ├── Seed from Allakhazam respawn wiki + RedGuides threads
│   └── Live correction from session kill events
├── Item / spell APIs
│   ├── ItemRepo (lookup by id, name, link-hash)
│   ├── SpellRepo (mana/dmg/duration/class-level/effects)
│   └── LootExpectation service (E[gp]/E[upgrades] per npc)
├── Faction / vendor APIs
│   ├── FactionRepo (char × faction → standing)
│   └── VendorRouter (best merchant for sell with faction adjustment)
└── Self-Improvement integrations (link to #2594)
    ├── Item-upgrade suggestor (MQ2ItemScore parity)
    ├── Pop-window-miss detector
    ├── Camp ROI suggestor (pp/hr vs alternatives)
    ├── Quest-reward-vs-vendor advisor
    ├── Faction-aware vendor advisor
    └── Spell DPS/DPM efficiency advisor
```

Each leaf is sized as a single Task issue. The spawn-timer and ingestion sub-trees can ship before the self-improvement integrations consume them.

## Exit criteria refresh

The original audit's parity exit criteria are unchanged. Adds:

- The `sessions.db` schema covers every event type EQLogParser/Gamparse/rumstil model.
- The `/improve` panel implements at least the severity tiers, checklist twin, and noise floor.
- The EQ knowledge layer ships at minimum the PEQ ingest pipeline, item/spell/loot lookup APIs, and the loot-expectation service.
- "Compare to your best" works without any cloud or external account.
