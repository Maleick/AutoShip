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
| Timing variation and operator hardening reduce visibility. | Research-backed | `docs/wiki/Security-and-Anti-Detection-Notes.md`, `docs/wiki/Research-Anti-Detection.md` | The repo explicitly treats anti-detection value as bounded guidance rather than proof of safety. |

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
- These routing references show current repo assumptions, not a live-confirmed Scars launch path into Sebilis.
- `config/named_mobs/sebilis.toml` records named placeholders and timer ranges
  for Trakanon, Baron Yosig, Crypt Caretaker, and Sebilite Protector.
- Current named-config timer windows are:
  - `Trakanon` -> `72-84 minutes`
  - `Baron Yosig` -> `28-36 minutes`
  - `Crypt Caretaker` -> `22-28 minutes`
  - `Sebilite Protector` -> `22-28 minutes`
- These timer windows come from the checked-in named config and remain unvalidated until a live sample confirms them.
- `textquest/src/camp/forage.rs` provides the current `/forage` loop and result
  history that live sampling can reuse.
- Current forage defaults are `enabled = false`, `interval_ms = 3000`, and
  `max_results_history = 50`.
- These forage defaults describe the current command loop only; they do not prove a safe unattended cadence or a live Nodding Blue Lily rate.
- `docs/wiki/Frostreaver-Farming-Guide.md` and `docs/wiki/P99-Zone-Guide.md`
  contain the existing research narrative about Disco, left wing, crypt,
  juggernauts, and myconids.
- `docs/wiki/Security-and-Anti-Detection-Notes.md` says timing variation is a
  practical hardening measure, not evidence of safety against a specific
  Daybreak detection path.
- Current repo anti-detection docs classify `Timing variation` as `Medium` confidence and `Operator environment` as `High` confidence.
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

Suggested target metrics:

- `route_time_minutes`
- `placeholder_respawn_minutes`
- `named_seen_per_hour`
- `wait_time_minutes`
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
