# Sebilis Farming Validation

This page is the canonical validation ledger for issue `#1526` and the source
of truth for any claim that Old Sebilis is a primary M10 farming hub.

Use this page to separate what the repository already supports from what still
needs live EverQuest proof. Do not promote Sebilis routing, spawn, forage, or
automation claims beyond the evidence state recorded here.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Old Sebilis is a plausible Kunark-era endgame farm for 50-60 groups. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md`, `docs/wiki/P99-Zone-Guide.md` | These guides describe Sebilis as a top-end XP and plat zone, but they are still planning inputs, not live TextQuest validation. |
| TextQuest already has Sebilis-specific camp and named configuration. | Research-backed | `config/camps/sebilis_disco.toml`, `config/named_mobs/sebilis.toml` | The repo knows about Disco, named placeholders, and a bounded pull/camp radius. |
| TextQuest has a forage automation surface that can drive `/forage` on an interval. | Research-backed | `textquest/src/camp/forage.rs` | The manager tracks attempts and result strings, but no Sebilis-specific Nodding Blue Lily baseline is captured in-repo. |
| TextQuest already exposes operator pause controls and a zone-scoped observability contract for attended validation runs. | Research-backed | `textquest/src/tui/event.rs`, `textquest/src/tui/state.rs`, `docs/dev/observability.md` | HOME/END and economy control bindings can pause, resume, or abort automation, and observability docs already standardize `zone` labels like `sebilis`; none of that is the same as a live Sebilis spawn-rate or safety baseline. |
| TextQuest already emits spawn-refresh observability that can support attended measurement runs. | Research-backed | `textquest/src/eq/spawn.rs`, `textquest/src/tui/run.rs`, `docs/dev/observability.md` | Spawn traversal and TUI refresh logs already include `spawn_count`, and observability docs define a `spawn_count` metric with `zone` labels, but the repo still does not persist camp-by-camp Sebilis respawn timing or overlap measurements. |
| TextQuest already has zone-failure recovery plumbing that can frame corpse-recovery risk during route validation. | Research-backed | `textquest/src/zoning/failure_codes.rs`, `textquest/src/zoning/recovery.rs`, `docs/zone-transition-state-map.md` | The repo models `CorpseInZone = -22`, safe-coordinate recovery, and live validation tasks for recovery outcomes, but none of that proves the intended Sebilis access route is corpse-recovery-safe in practice. |
| TextQuest already has generic HVT and named-priority surfaces that can aid attended named sampling. | Research-backed | `textquest/src/eq/hvt.rs`, `textquest/src/camp/puller.rs`, `config/hvt_watchlist.toml`, `docs/wiki/Operator-Guide.md` | The repo can load a watchlist and prefer HVTs during pull selection, but the checked-in watchlist is generic and does not currently anchor Sebilis-specific named tracking or overlap baselines. |
| TextQuest already exposes a map-screen named tracker panel and `watch named` controls for attended spawn monitoring. | Research-backed | `textquest/src/tui/ui/map.rs`, `textquest/src/tui/app.rs`, `docs/wiki/Research-MQ2-Parity-Matrix.md` | The UI already exposes generic named-tracking surfaces, but the current parity audit still calls them display-only and missing per-zone Sebilis configuration or validated timer baselines. |
| TextQuest already exposes generic camp-overlay visuals for attended radius tuning. | Research-backed | `textquest/src/tui/ui/map.rs`, `textquest/src/tui/config_panel.rs` | The map already renders camp and pull radius circles and the config panel surfaces pull-radius state, but the repo still has no Sebilis-specific optimal radius baseline or overlap proof. |
| Timing variation and operator hardening reduce visibility. | Research-backed | `docs/wiki/Security-and-Anti-Detection-Notes.md`, `docs/wiki/Research-Anti-Detection.md` | The repo explicitly treats anti-detection value as bounded guidance rather than proof of safety. |
| The repo already tracks missing unattended-session safeguards and telemetry as overnight requirements or gaps. | Research-backed blocker | `docs/MQ2_COVERAGE_GAP_ANALYSIS.md`, `docs/OVERNIGHT-ISSUE-SUMMARY.md` | GM alerts, auto-camp-on-death, kill or plat tracking, and session logs are documented as required or gap-tracked overnight tooling, not validated Sebilis-safe automation. |
| Launch-zone routing support is still planned rather than proven. | Research-backed blocker | `docs/orchestration-design.md` | The orchestration roadmap still lists a `Camp database for launch zones` as unfinished TLP-launch work, so this repo does not yet present a complete launch-zone routing surface for Sebilis. |

## Research-backed inputs already in repo

- `config/camps/sebilis_disco.toml` defines an initial Disco camp center, pull
  point, leash, mana thresholds, and pull list for Sebilis.
- Current `sebilis_disco` defaults are `pull_radius = 220`,
  `camp_radius = 30`, `level_range = [45, 55]`, and
  `prev_camp = "lguk_dead_side"`.
- These defaults are planning inputs only, not live-validated route or spawn proof.
- Current pull-control defaults are `leash_radius = 110`,
  `rest_mana_pct = 70`, and `pull_mana_pct = 40`.
- Current mob filters are `a sebilite guardian`, `a sebilite protector`, and
  `a crypt caretaker` in `pull_mob_names`, `Trakanon` in
  `ignore_mob_names`, and `a sebilite juggernaut` in `burn_mob_names`.
- These pull-control defaults describe current camp intent only; they do not prove live camp-rotation efficiency or safe overlap.
- `config/camps/lguk_dead_side.toml` currently links forward with
  `next_camp = "sebilis_disco"`.
- `scripts/generate_maps.py` currently labels Sebilis with
  `to_Field_of_Bone` and `to_Trakanons_Teeth` map exits.
- `docs/orchestration-design.md` still lists `Camp database for launch zones`
  as unfinished TLP-launch work.
- These routing references show current repo assumptions, not a live-confirmed Scars launch path into Sebilis.
- `config/named_mobs/sebilis.toml` records named placeholders and timer ranges
  for Trakanon, Baron Yosig, Crypt Caretaker, and Sebilite Protector.
- Current named-config timer windows are:
  - `Trakanon` -> `72-84 minutes`
  - `Baron Yosig` -> `28-36 minutes`
  - `Crypt Caretaker` -> `22-28 minutes`
  - `Sebilite Protector` -> `22-28 minutes`
- Current named-config drop lists are `Singing Short Sword`,
  `Trakanon's Tooth`, `Elder Spiritist's Helm`,
  `Crypt Caretaker's Shield`, and `Sebilite Scale Leggings`.
- These timer windows come from the checked-in named config and remain unvalidated until a live sample confirms them.
- Those checked-in named drops do not currently anchor the issue's primary
  target outputs of `Nodding Blue Lily`, `Runebranded Girdle`, `Fungi Tunic`,
  or `Froglok Blood`.
- `textquest/src/camp/forage.rs` provides the current `/forage` loop and result
  history that live sampling can reuse.
- Current forage defaults are `enabled = false`, `interval_ms = 3000`, and
  `max_results_history = 50`.
- These forage defaults describe the current command loop only; they do not prove a safe unattended cadence or a live Nodding Blue Lily rate.
- `textquest/src/tui/event.rs` already wires global operator controls:
  `HOME` pauses automation, `END` resumes automation, and the economy panel
  binds `P` to pause, `R` to resume, and `A` to abort.
- `textquest/src/tui/state.rs` tracks `automation_paused` as explicit operator
  state and defaults it to `false`.
- `docs/dev/observability.md` already standardizes structured `zone = "sebilis"`
  fields and `zone` metric labels for logs and counters.
- These operator controls and observability examples are useful for attended
  Sebilis sampling, but they do not prove that unattended handling is safe or
  that Sebilis-specific spawn or forage metrics are already captured.
- `textquest/src/eq/spawn.rs` already logs `spawn_count = spawns.len()` when
  spawn traversal stops on unreadable or overflow conditions.
- `textquest/src/tui/run.rs` already emits `TUI spawn snapshot refreshed` with
  `spawn_count`, `spawn_revision`, and `elapsed_ms` when perf tracing is
  enabled.
- `docs/dev/observability.md` already defines a `spawn_count` metric example
  keyed by `zone`.
- These spawn observability hooks help instrument attended sampling sessions,
  but they still do not capture camp-by-camp Sebilis respawn timing, named
  overlap, or validated wait-time baselines.
- `textquest/src/zoning/failure_codes.rs` already maps
  `CorpseInZone = -22` as a zone-entry failure that needs investigation before
  retry.
- `textquest/src/zoning/recovery.rs` already exposes safe-coordinate recovery
  orchestration with `MAX_RECOVERY_RETRIES = 3`, backoff scheduling, and
  response validation.
- `docs/zone-transition-state-map.md` already says live route tests should
  record trigger, visible behavior, and recovery outcome, including whether
  recovery returns the client to an origin or destination safe point.
- These zone-recovery hooks help scope corpse-risk and route-failure checks for
  attended Sebilis validation, but they still do not prove the Scars-launch
  route is stable for corpse recovery or repeated turnover.
- `textquest/src/eq/hvt.rs` already loads a per-mob watchlist with `zone`,
  `priority`, `alert_discord`, and free-text notes.
- `textquest/src/camp/puller.rs` already prefers HVT watchlist targets after
  explicit camp pull-mob matches and before generic closest-NPC fallback.
- `config/hvt_watchlist.toml` and `docs/wiki/Operator-Guide.md` show the
  operator-facing watchlist surface for named tracking and alerts.
- The checked-in HVT watchlist is still generic and currently does not include
  Sebilis names like `Trakanon` or `Crypt Caretaker`, so it does not yet serve
  as a Sebilis-specific named-overlap or respawn baseline.
- `textquest/src/tui/ui/map.rs` already declares the map screen as a `named
  tracker panel`.
- `textquest/src/tui/app.rs` already exposes `watch named [on|off]` and reports
  `Named spawn alerts: ON/OFF` to the operator.
- `docs/wiki/Research-MQ2-Parity-Matrix.md` still calls the named tracker panel
  display-only and says per-zone configuration remains missing.
- These named-tracking controls help attended Sebilis observation, but they do
  not yet provide a Sebilis-specific timer feed, alert contract, or overlap
  baseline that could close this issue.
- `textquest/src/tui/ui/map.rs` already draws camp overlays with a green camp
  radius circle, a red pull radius circle, and center markers for both points.
- `textquest/src/tui/config_panel.rs` already surfaces `Pull Radius` as
  `camp.pull_radius` in the generic camp config tree.
- These map and config surfaces help an operator inspect current camp geometry
  during attended Sebilis tuning, but they do not prove that the checked-in
  radius values are optimal for Disco rotation, named overlap, or safe pull
  recovery.
- `docs/wiki/Frostreaver-Farming-Guide.md` and `docs/wiki/P99-Zone-Guide.md`
  contain the existing research narrative about Disco, left wing, crypt,
  juggernauts, and myconids.
- `docs/wiki/Security-and-Anti-Detection-Notes.md` says timing variation is a
  practical hardening measure, not evidence of safety against a specific
  Daybreak detection path.
- Current repo anti-detection docs classify `Timing variation` as `Medium` confidence and `Operator environment` as `High` confidence.
- `docs/MQ2_COVERAGE_GAP_ANALYSIS.md` and `docs/OVERNIGHT-ISSUE-SUMMARY.md`
  document GM alerts, auto-camp-on-death, kill or plat tracking, and
  per-character logs as important overnight-session safeguards or gaps.
- Those overnight safety docs describe required or proposed operator tooling;
  they do not prove that unattended Sebilis macroing is currently safe or fully
  instrumented in TextQuest.
- These repo-grounded exposure labels do not make unattended Sebilis macroing safe.

## Needs live proof before this issue can close

- Confirm whether the intended Scars-of-Velious launch route to Sebilis is
  actually practical on the live server, including zone sequence, travel time,
  corpse-recovery risk, and key requirements.
- Record real spawn cadence for each candidate camp instead of relying on P99 or
  community assumptions.
- Measure camp-rotation overlap: how long each group waits on placeholders, how
  often named placeholders line up, and whether Disco, left wing, crypt, and
  juggs can be chained without dead time.
- Capture a per-hour Nodding Blue Lily forage baseline from Sebilis sessions
  that use the current `textquest/src/camp/forage.rs` path.
- Treat AFK or overnight macro safety as unresolved until a live session proves
  the route and timing are operator-safe. The repo explicitly does **not**
  guarantee anti-detection safety.

## Validation procedure

### 1. Routing validation

- Start from the actual launch-zone staging point used on the live server.
- Record the zone path into Sebilis, required keying, and total travel time.
- Note whether the path is stable enough for repeated camp turnover and death
  recovery.

### 2. Spawn and rotation validation

- Sample each candidate camp for a fixed window, then log placeholder count,
  named count, and mean respawn delay.
- Track cross-camp overlap so the operator can tell whether rotation beats
  sitting on a single camp.
- Compare the live sample against the repo's named configuration before
  expanding any automation logic.

### 3. Forage validation

- Use a Shaman or other forager on the current `/forage` loop.
- Record attempts, successes, and actual Nodding Blue Lily hits.
- Convert the session totals to a per-hour baseline before calling Sebilis an
  alchemy hub.

### 4. Automation-risk review

- Re-read `docs/wiki/Security-and-Anti-Detection-Notes.md` before any
  unattended session.
- Keep anti-detection claims bounded: timing jitter and humanization are
  research-backed hardening, not proof that overnight AFK macroing is safe.
- If the session depends on unattended or low-observability behavior, treat the
  run as a blocker until there is live proof the operator risk is acceptable.

## Checked-in sampling template

Use [sebilis-validation-template.csv](assets/sebilis-validation-template.csv)
for live sampling. It is intentionally blank so the repo does not invent spawn,
route, or forage numbers that were never observed.

The checked-in template now includes explicit columns for
`launch_staging_point`, `required_keying`, `travel_time_minutes`,
`placeholder_count`, `named_count`, `mean_respawn_minutes`,
`wait_time_minutes`, and `operator_mode` so the eventual spreadsheet can
capture routing access, spawn cadence, camp overlap, and attended versus
unattended posture without inventing values ahead of live testing.

Record theorized Sebilis outputs as hypotheses only until a live sample observes them.
Current theory items worth capturing explicitly in the template include
`Nodding Blue Lily`, `Runebranded Girdle`, `Fungi Tunic`, and
`Froglok Blood`.

### Current routing, rotation, and output theory status

| Claim | Current evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Sebilis can support a multi-camp rotation across Disco 1+2, left wing, crypt, and juggs/myconids. | Research-backed rotation theory | `docs/wiki/Frostreaver-Farming-Guide.md`, `docs/wiki/P99-Zone-Guide.md` | Current guides describe `4-6 groups` or `5-6 groups` across these camp areas, but the repo still lacks live wait-time and overlap measurements. |
| Sebilis access likely depends on keying before deeper farming is practical. | Research-backed route requirement | `docs/wiki/P99-Zone-Guide.md` | The guide says `Requires key`, but this repo still has no live Scars-launch route or corpse-recovery sample proving the requirement in practice. |
| Existing research suggests Sebilis money camps may land around `~400pp/hr` at gem camp and `500-1000pp` at juggs/myconids. | Research-backed economy theory | `docs/wiki/P99-Zone-Guide.md`, `docs/wiki/Frostreaver-Farming-Guide.md` | These are planning priors from guides, not live TextQuest output data. |
| Overnight Sebilis output should land around `1000-2000pp per Shaman per night`. | Issue-theory only | Issue `#1526` description | This output target is still unanchored by repo-local evidence or live samples. |

### Current drop theory status

| Target | Current evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| `Runebranded Girdle` | Research-backed loot theory | `docs/wiki/Frostreaver-Farming-Guide.md`, `docs/wiki/P99-Zone-Guide.md` | Current Sebilis planning guides already list this as notable Sebilis loot, but it still needs a live sample before it can be promoted as validated output. |
| `Nodding Blue Lily` | Issue-theory only | Issue `#1526` description | `Nodding Blue Lily` remains an issue-theory hypothesis until a repo-local source or live sample anchors it. |
| `Fungi Tunic` | Issue-theory only | `docs/wiki/Research-MQ2-Deep-Dive.md` | `Fungi Tunic` currently appears only in a generic item-command example, not a Sebilis evidence source. |
| `Froglok Blood` | Issue-theory only | Issue `#1526` description | `Froglok Blood` currently has no repo-local Sebilis evidence source beyond the issue theory. |

The current `config/named_mobs/sebilis.toml` drop list instead tracks
`Singing Short Sword`, `Trakanon's Tooth`, `Elder Spiritist's Helm`,
`Crypt Caretaker's Shield`, and `Sebilite Scale Leggings`, which means the
repo's checked-in named evidence still diverges from most of the issue's
proposed Sebilis output targets.

Suggested `target_metric` values for the validation template's
`target_metric` column (do not add these as new CSV columns):

- `route_duration_minutes`
- `placeholder_respawn_interval_minutes`
- `named_seen_per_hour`
- `camp_wait_duration_minutes`
- `forage_hits_per_hour`
- `operator_interruptions_per_hour`

## Exit criteria

Do not call Sebilis a validated farming hub until the template has at least:

- one live route sample from the intended launch staging point
- one spawn sample each for Disco, left wing, crypt, and juggs or a documented
  reason that a camp was rejected
- one forage sample that includes total attempts and Nodding Blue Lily hits
- one explicit automation-risk note that states whether the session remained
  attended, semi-attended, or unattended
