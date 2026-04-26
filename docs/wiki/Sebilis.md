# Old Sebilis — Farming Notes

Farming notes for Old Sebilis captured under issue `#3360` (child of zone tracker `#1768`).
This page records DPS expectations, loot-per-kill observations, and multibox pull-sequence behavior.
All values are **research-backed planning inputs** until live TextQuest validation runs update them with observed data.

For the canonical live-evidence ledger, see [Sebilis Farming Validation](Sebilis-Farming-Validation.md).
For camp geometry, waypoint lattice, and restriction zones, see [Sebilis Disco Camp](Sebilis-Disco-Camp.md).

---

## DPS Expectations

### Research-Backed Baselines (not yet live-validated)

| Camp Area | Mob Type | Expected DPS Context | Source | Evidence State |
| --- | --- | --- | --- | --- |
| Disco 1 (right wing) | Froglok knights, shamans, wizards | Group DPS target sufficient to clear a right-wing loop in ~10-12 min with 6-box. Single-group parse window not yet recorded. | `docs/wiki/Frostreaver-Farming-Guide.md`, `docs/wiki/P99-Zone-Guide.md` | Research-backed |
| Disco 2 (continuation) | Froglok commanders, ostiary, pickler | Similar mob HP to Disco 1; assumes ~same per-group throughput. Adds ~4-6 min to sweep if extended into pickler and Brogg rooms. | `docs/wiki/Frostreaver-Farming-Guide.md` | Research-backed |
| Crypt (optional extension) | Crypt caretaker, sebilite guardian | Lower mob density; crypt caretaker summons and sees invis — expect slower pulls and longer recovery windows. DPS throughput lower than Disco. | `docs/wiki/Sebilis-Disco-Camp.md` | Research-backed |
| Underground (juggs/myconids) | Sebilite juggernauts, myconid spore king | High HP targets; recommend dedicated burn group. Expect longer kill times (30-60 sec per mob) vs. right-wing froglok trash (~10-20 sec). | `docs/wiki/P99-Zone-Guide.md` | Research-backed |

### Live Validation Fields (to be filled on first attended run)

| Metric | Target Value | Observed Value | Session Date | Notes |
| --- | --- | --- | --- | --- |
| Right-wing loop clear time (6-box) | ~10-12 min | _not yet observed_ | — | Full Disco 1 sweep from pull_handoff_corner back to safe_med_hall |
| Average mob kill time (froglok trash) | ~10-20 sec | _not yet observed_ | — | Tracked as combat_start → loot_window |
| Average mob kill time (crypt caretaker) | ~25-40 sec | _not yet observed_ | — | Includes summon interrupt overhead |
| DPS per hour (XP context) | research estimate: high for 53-57 | _not yet observed_ | — | Compare against LGUK dead side reference |

---

## Loot-Per-Kill Observations

### Named Drop Research Baseline

The following drops are tracked in `config/named_mobs/sebilis.toml`. Evidence state: research-backed configuration only — not observed from live TextQuest runs.

| Named Mob | Respawn Window | Notable Drops | Evidence State |
| --- | --- | --- | --- |
| Trakanon | 72-84 min | Trakanon's Tooth, Elder Spiritist's Helm | Research-backed (config) |
| Baron Yosig | 28-36 min | Singing Short Sword | Research-backed (config) |
| Crypt Caretaker | 22-28 min | Crypt Caretaker's Shield | Research-backed (config) |
| Sebilite Protector | 22-28 min | Sebilite Scale Leggings | Research-backed (config) |

### Trash Loot Research Baseline

| Loot Target | Source Mob / Method | Expected Rate | Evidence State |
| --- | --- | --- | --- |
| Nodding Blue Lily | Forageable (Shaman/Druid with `/forage`) | Unknown — no live baseline captured yet | Issue-theory only |
| Runebranded Girdle | Right-wing froglok trash or named | Listed in farming guides as notable Sebilis loot | Research-backed |
| Fungi Tunic | Named / special drop | Appears in `docs/wiki/Research-MQ2-Deep-Dive.md` item-command example only | Issue-theory only |
| Froglok Blood | Right-wing froglok trash | No repo-local Sebilis evidence source | Issue-theory only |
| Raw plat (vendor trash) | All froglok trash | Research estimate: ~400pp/hr at gem/trash camp; 500-1000pp/hr at juggs/myconids | Research-backed (guides) |

### Live Loot-Per-Kill Fields (to be filled on first attended run)

| Metric | Observed Value | Session Date | Camp | Notes |
| --- | --- | --- | --- | --- |
| Runebranded Girdle drop rate | _not yet observed_ | — | Disco 1 | Confirm whether right-wing trash or named-only |
| Nodding Blue Lily per hour (forage) | _not yet observed_ | — | Any | Use `textquest/src/camp/forage.rs` loop; log attempts and hits |
| Raw plat per hour (vendor trash) | _not yet observed_ | — | Disco 1 | Sum vendor value across one full sweep |
| Named kill per session | _not yet observed_ | — | Varies | Baron Yosig and Crypt Caretaker are primary Disco-range named targets |

---

## Multibox Pull-Sequence Behavior

Pull-sequence notes are derived from `docs/wiki/Sebilis-Disco-Camp.md` and the checked-in `config/camps/sebilis_disco.toml`. These describe intended automation behavior, not yet confirmed from live TextQuest sessions.

### Default 6-Box Pull Loop (Disco 1 — Right Wing)

```
safe_med_hall → pull_handoff_corner → armory_door → bar_room
    → armsman_bedroom → froggy_room → abc_stairs → chef_room
    → repairer_room → pull_handoff_corner → safe_med_hall
```

**Automation parameters driving this loop:**

| Parameter | Value | Behavior |
| --- | --- | --- |
| `pull_radius` | 380 | Covers right-wing bartender, Froggy, armorer, ABC without reaching Trakanon space |
| `camp_radius` | 35 | Tight stack in the bartender/armsman hallway |
| `leash_radius` | 160 | Short leash; drops bad pulls before bar or ABC rooms collapse |
| `rest_mana_pct` | 70 | Holds camp at med until healer pair recovers |
| `pull_mana_pct` | 40 | Blocks new right-wing pull when mana is below floor |
| `return_no_aggro` | true | Puller does not auto-snap back while still carrying a live train |

### Extended Pull Loops

**Optional crypt extension (when short loop is dry):**

```
pull_handoff_corner → guardian_approach → crypt_split
    → crypt_west_cube → guardian_approach → pull_handoff_corner
```

Note: Crypt caretaker summons and sees invis. An active driver and crowd control ready are required before extending to this loop.

**Optional Disco 2 sweep (active driver + CC required):**

```
pull_handoff_corner → ostiary_corner → disco2_fork → commander_east
    → commander_west → pickler_room → hidden_passage → brogg_library
    → hidden_passage → pull_handoff_corner
```

Note: `froglok commander` is a wander PH with hallway travel time; its practical cadence is longer than raw spawn timer implies.

### Pull-Sequence Restrictions

| Restriction | Boundary | Reason |
| --- | --- | --- |
| `trak_no_pull` | Hard stop at `jugg_line_start` / `protector_hard_stop` | Juggernaut and Trakanon traffic outside Disco scope |
| `pond_ground_only` | No fighting on rock or underwater near myconid pond | Server-rule risk per P99 zone notes |
| `crypt_soft_stop` | Do not drag crypt caretaker past `guardian_approach` without active CC | Summoning cube + tighter hallway geometry |
| `entry_bridge_no_med` | No med at entry bridge or first drop | Frequent train lane; zone exit ≠ entry portal |

### Live Pull-Sequence Validation Fields (to be filled on first attended run)

| Metric | Target | Observed | Session Date | Notes |
| --- | --- | --- | --- | --- |
| Short loop completion without wipe | ≥ 30 min uninterrupted | _not yet observed_ | — | Must confirm leash drops bad pulls cleanly |
| Crypt extension viability | Pull caretaker without collapse | _not yet observed_ | — | Require CC confirmation before enabling |
| Disco 2 sweep viability | Commander/pickler pulls clean | _not yet observed_ | — | Wander path must not bleed into Disco 1 stack |
| Camp overlap (multiple 6-box groups) | No spawn contention | _not yet observed_ | — | Requires 4+ groups simultaneously |

---

## Farming Notes Summary

| Claim | Evidence State | Next Required Step |
| --- | --- | --- |
| Old Sebilis right wing (Disco 1-2) viable for 6-box farming at 53-57 | Research-backed | Confirm with one attended 30+ min session; record loop time and loot |
| Underground juggs/myconids viable for plat farming | Research-backed (500-1000pp/hr estimate) | Confirm drop rate per kill with live session |
| Named spawns (Yosig, Caretaker) killable on standard timer | Research-backed (config) | Confirm timer window with live log |
| Nodding Blue Lily forageable in zone | Issue-theory only | Run live forage baseline using `textquest/src/camp/forage.rs` |
| Pull-sequence loops safe for unattended automation | Not validated | Must complete attended validation first; see exit criteria in [Sebilis Farming Validation](Sebilis-Farming-Validation.md) |

---

## Related Documentation

- [Sebilis Disco Camp](Sebilis-Disco-Camp.md) — Waypoint lattice, restriction zones, camp geometry
- [Sebilis Farming Validation](Sebilis-Farming-Validation.md) — Canonical evidence ledger, exit criteria, live proof tracking
- [Sebilis Guide](Sebilis-Guide.md) — Overview, zone access, mob composition
- `config/camps/sebilis_disco.toml` — Runtime camp configuration
- `config/named_mobs/sebilis.toml` — Named mob timers and drop lists
- `textquest/src/camp/forage.rs` — Forage loop implementation
