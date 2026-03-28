# Frostreaver Roadmap Peer Review

**Date:** 2026-03-28
**Reviewer:** Claude (roadmap-reviewer agent)
**Scope:** M1-M8 milestone plan, architecture, risk, and viability
**Status at review time:** M1-M5 code complete, first live test successful (2026-03-28), DLL injection untested live, ~26/36 accounts created, Frostreaver TLP launch ~May 2026

---

## Executive Summary

Frostreaver is an ambitious, well-architected project with a realistic shot at launch-day functionality. The milestone ordering is mostly correct, the core architecture (Rust DLL injection + external orchestrator) is sound, and the first live test proves the foundation works. However, several critical gaps could kill the project before it generates a single Krono: **anti-detection is underdeveloped**, **the DLL hasn't been injected live yet** (the single biggest untested risk), and **M5/M6 (Soul Engine) is premature scope creep** that should be deprioritized below combat reliability and loot automation.

**Bottom line:** The project is ~60% of the way to a viable launch-day MVP. The remaining 40% is integration, testing, and hardening — not new features. Stop building new milestones. Ship what you have.

---

## 1. Milestone Order Assessment

### Current order: M1 → M2 → M2.5 → M3 → M4 → M5 → M6 → M7 → M8

**Verdict: Mostly correct through M4. Wrong after M4.**

| Milestone | Order | Assessment |
|-----------|-------|------------|
| M1 (External reading) | 1st | Correct. Foundation. Done. |
| M2 (DLL injection + IPC) | 2nd | Correct. Enables everything else. Done (code), **untested live**. |
| M2.5 (Login automation) | 3rd | Correct. Can't run 36 clients without it. Done (code). |
| M3 (Navigation) | 4th | Correct. Characters need to move before they fight. Done (code). |
| M4 (Combat) | 5th | Correct. Core gameplay loop. Done (code). |
| **M5 (Soul Engine)** | **6th** | **Wrong. Should be M7 or M8.** See Section 6. |
| **M6 (LLM API)** | **7th** | **Wrong. Should be last or cut entirely for launch.** |
| M7 (RL/Learning) | 8th | Should be M6. Behavioral tuning has real ROI. |
| **M8 (Economy)** | **9th** | **Wrong. Should be M5. Loot/economy is core to the mission.** |

### Recommended reorder for launch viability

```
M1-M4: Done (code complete, needs live testing)
M5*:   Economy (loot automation, vendor, plat consolidation) — this IS the mission
M6*:   Learning/RL (behavioral tuning, XP/hr optimization)
M7*:   Soul Engine (personality, idle behavior — nice-to-have)
M8*:   LLM Character AI (API integration — luxury feature)
```

**Rationale:** The stated goal is Krono farming. Economy automation (current M8) directly enables that. Soul Engine (current M5) does not generate Krono. It makes characters "feel real" — which matters for anti-detection and is genuinely cool, but it doesn't pay for itself. A bot that loots efficiently but has no personality makes money. A bot with a rich inner life that can't loot is a creative writing project.

### What's actually needed before launch day

This is the real priority list. None of these are new milestones — they're integration and testing of existing code:

1. **DLL injection live test** (hours, not days — but blocks everything)
2. **Integration loop** (~200-300 lines, documented in dll-injection-plan.md)
3. **Offset calibration** (CHAR_CLASS, STANDSTATE, mana — partially done)
4. **6-client group test** (inject, IPC, group invite, basic combat)
5. **Loot automation** (LootAll function address already identified)
6. **Navmesh integration** (biggest feature gap per MQ2 analysis)
7. **36-client stress test** (memory, CPU, stability over hours)
8. **Anti-detection hardening** (see Section 7)

---

## 2. Critical Gaps

### 2.1 Anti-Detection (SEVERITY: PROJECT-KILLING)

This is the single biggest existential risk and it's barely addressed. Current anti-detection measures:

**What exists:**
- Randomized DLL name per injection
- Movement humanization (speed jitter, occasional wrong turns)
- Staggered zone transitions (5-60s)
- Per-character combat jitter
- Independent movement (no /follow blob)

**What's missing:**

| Gap | Risk | Priority |
|-----|------|----------|
| **No Warden analysis** | Daybreak's anti-cheat scans for known DLLs, memory signatures, and hooking patterns. Flying blind. | CRITICAL |
| **No hook obfuscation** | `retour::static_detour` leaves a recognizable pattern (trampoline + jmp). Warden may scan for this. | HIGH |
| **No timing analysis** | All 36 characters responding within identical tick windows is detectable. Need per-character tick jitter. | HIGH |
| **No player proximity behavioral shift** | Dave confirmed: "tone down for passersby, innocent mode if they linger." Code exists in decisions but not implemented. | HIGH |
| **No GM detection** | GMs can appear invisible, whisper, or teleport nearby. Need /who monitoring, tell detection, unusual spawn detection. | HIGH |
| **No network fingerprinting defense** | 36 clients from one IP with identical packet timing patterns is trivially detectable server-side. | MEDIUM |
| **No process hiding** | `eqgame.exe` with an injected DLL is visible in Task Manager. If Warden enumerates loaded modules, it sees yours. | MEDIUM |
| **No memory signature rotation** | Static strings in the DLL ("DMFT", "dmft_dll", "frostreaver") are scannable. | MEDIUM |

**Recommendation:** Before live deployment, spend 2-3 days on:
1. Research Warden's actual scanning behavior (RedGuides forums, MQ2 community)
2. Strip all identifying strings from the DLL
3. Add module hiding (unlink from PEB's InLoadOrderModuleList)
4. Add per-character tick jitter (not just movement jitter — ALL actions)
5. Implement GM detection (monitor for invisible spawns, unexpected tells)
6. Implement "innocent mode" toggle (all bots stop, emote, act idle)

### 2.2 Crash Recovery and Stability

**What exists:** Self-healing monitor concept, auto-restart + re-inject design.

**What's missing:**

| Gap | Impact |
|-----|--------|
| **No crash telemetry** | When EQ crashes, you don't know why. Need minidump capture + analysis. |
| **No graceful DLL unload** | If the orchestrator dies, 36 DLLs keep running headless with no command source. |
| **No state persistence across restarts** | Character restarts lose: group membership, camp position, buff state, combat state. |
| **Kill-on-close not implemented** | Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` documented but not wired up. |
| **No watchdog for the orchestrator itself** | If the orchestrator crashes, everything is orphaned. Need a supervisor process or Windows service. |

### 2.3 Network and Bandwidth

36 clients from one IP is unusual but not unprecedented (internet cafes, ISBoxer users exist). However:

| Concern | Assessment |
|---------|------------|
| **Bandwidth** | EQ is ~5-15 KB/s per client. 36 clients = ~200-500 KB/s. Trivial for any modern connection. Not a concern. |
| **Server-side rate limiting** | Daybreak may throttle or flag mass connections from a single IP. No known hard limit, but 36 simultaneous logins will spike. Staggered login (already planned) mitigates. |
| **IP-based detection** | If Daybreak correlates "36 accounts, same IP, same hardware fingerprint, all online 24/7" — that's a flag. Consider rotating through a VPN or using multiple IPs. |
| **Packet timing correlation** | If all 36 clients send movement packets at the same tick interval, server-side analysis can cluster them. Need per-client packet jitter. |

### 2.4 Loot Distribution

**Not addressed in any milestone.** The decision profile says "configurable loot rules per item type" and the economy phasing mentions vendor trash → EC tunnel → bazaar, but there's no actual loot distribution system designed.

Needed:
- **LootAll automation** (function address `0x140459700` identified, not implemented)
- **Item evaluation** (vendor price, tradeable, quest item, rare)
- **Distribution rules** (need before want, class-appropriate, banker consolidation)
- **Plat consolidation** (all characters /give to banker, banker handles economy)
- **Rare item routing** (specific items → specific characters or EC tunnel trader)

This is core to the mission and should be M5 (after combat), not M8.

### 2.5 Buff Rotations and Group Synergy

The combat system has class strategies (warrior, cleric, enchanter, generic DPS) and a HolyShit conditional system, but:

| Gap | Impact |
|-----|--------|
| **No bard twist engine** | Bard is in every optimal group comp (Dave's intel). MQ2 gap analysis lists it as Tier 2 #4. |
| **No shaman/druid strategies** | Two of the three healer classes have no strategy. |
| **Only 4 of 16 classes implemented** | WAR, CLR, ENC, generic DPS. Missing: BRD, SHM, DRU, NEC, WIZ, MAG, MNK, ROG, RNG, SK, PAL, BST, BER. |
| **No cross-group buff coordination** | Druids/shamans should buff all 36 characters, not just their group. Need orchestrator-level buff scheduling. |
| **No buff tracking/rebuffing** | No system to detect buff expiration and reapply. |
| **Stick/follow not implemented** | MQ2 gap analysis #2. Melee characters can't maintain position relative to mob. |

---

## 3. Risk Assessment

### Project-Killing Risks

| Risk | Probability | Impact | Mitigation Status |
|------|-------------|--------|-------------------|
| **Mass ban from Daybreak** | HIGH (60%) | Fatal | INADEQUATE — anti-detection is rudimentary |
| **DLL injection crashes EQ** | MEDIUM (40%) | Severe | UNTESTED — first live injection hasn't happened |
| **Offset drift on patch day** | HIGH (80%) | Severe | PARTIAL — hot-updatable offset DB exists, but manual offset discovery is slow |
| **36 clients won't fit on one machine** | LOW (15%) | Moderate | GOOD — INI optimization doc is thorough, memory budget is ~18GB/64GB |
| **TLP launch day chaos** | MEDIUM (50%) | Moderate | NONE — no plan for queue times, server instability, emergency patches |

### Mass Ban Deep Dive

This deserves special attention because it's the most likely project failure mode.

**How Daybreak detects bots:**

1. **Warden anti-cheat** — Scans process memory for known cheat signatures. MQ2 has a cat-and-mouse history with Warden. A custom Rust DLL has no known signature, which is an advantage — but hooking patterns and memory modifications are detectable regardless of language.

2. **Behavioral analysis** — Server-side heuristics: characters online 24/7, identical action timing, impossible reaction speeds, no chat, no tells, perfect navigation. 36 characters from one IP with synchronized behavior is a massive red flag.

3. **Player reports** — The EQ community actively reports botters. 36 characters in one zone will get reported within hours during peak times. Dave confirmed: "subtle boxers fine, obvious bots got hit."

4. **GM investigation** — GMs can observe invisibly, whisper to test responsiveness, check play patterns. Without GM detection + response, a single GM visit = 36 bans.

**Realistic ban timeline without mitigation:** 1-4 weeks after going live.
**With good anti-detection:** 2-6 months, possibly indefinite if careful.

**Key insight from Dave's intel:** "Roaming loops, NOT stationary camps" and "Max 12 toons visible together." These are more important than any technical anti-cheat measure. Behavioral detection is harder to evade than signature detection.

### Offset Drift Mitigation

The hot-updatable offset database is a good start, but the process for discovering new offsets after an EQ patch is:

1. Wait for MQ2 community to update (hours to days)
2. Manually extract offsets from MQ2 headers
3. Update `offsets.rs` or the offset DB
4. Rebuild and redeploy

**Risk:** If EQ patches on a Tuesday and the TLP has a contested raid spawn on Wednesday, you're offline for the most valuable farming window.

**Recommendation:** Build an automated offset scanner that pattern-matches known function signatures in eqgame.exe. This is how MQ2's offset finder works — scan for known byte patterns near the expected offset range. This makes patch-day recovery minutes instead of hours.

### Performance at Scale

The INI optimization doc is excellent and the memory budget math checks out:

- 36 clients @ ~500MB = ~18GB (with optimized INI)
- Windows + overhead = ~6GB
- Orchestrator + DLLs = ~2GB
- **Total: ~26GB / 64GB available**

CPU is the real concern:
- 36 clients @ 10 BG FPS each = 360 frames/second of game logic
- 24 cores available
- That's ~1.5 clients per core, which should be fine IF render strobing is implemented

**The render strobing optimization from the WinEQ research is the single most impactful performance feature not yet implemented.** It reduces GPU load by ~98% for background clients. Without it, 36 clients may be GPU-bound even at 640x480.

### TLP Launch Day

No plan exists for:
- Server queue times (could be 2-4 hours on launch)
- Emergency hotfixes changing offsets mid-day
- Login server overload (mass connection failures)
- Zone crashes during the land rush
- Competition for camps from other box crews

**Recommendation:** Have a launch-day playbook:
1. Pre-stage all 36 accounts logged in and parked in tutorial
2. Stagger zone-ins over 30 minutes (not all at once)
3. Have fallback camps if primary targets are contested
4. Accept that day 1 will be chaotic — focus on being online and stable, not optimal
5. Have manual override capability for when automation breaks

---

## 4. Group Composition Recommendation

### For 6 groups of 6 on a Velious-start TLP with free trade

Dave's intel confirms the meta. Here's a recommended composition:

#### Group Template (XP Grinding)

| Slot | Class | Role | Automation Complexity | Notes |
|------|-------|------|-----------------------|-------|
| 1 | Warrior (WAR) | Tank | LOW | Taunt + position. Strategy exists. |
| 2 | Cleric (CLR) | Healer | MEDIUM | CH/reactive healing. Strategy exists. |
| 3 | Enchanter (ENC) | CC/Haste/Clarity | HIGH | Charm is risky to automate. Strategy exists. |
| 4 | Bard (BRD) | Buff/Pull | HIGH | Twist engine needed. **Not implemented.** |
| 5 | Ranger (RNG) | DPS | LOW | Auto-attack DPS king. Dave's #1 recommendation. |
| 6 | Ranger (RNG) | DPS | LOW | Double ranger for simplicity. |

#### Recommended 36-Character Roster

| Group | Tank | Healer | Support | DPS 1 | DPS 2 | DPS 3 |
|-------|------|--------|---------|-------|-------|-------|
| G1 | WAR | CLR | ENC | BRD | RNG | RNG |
| G2 | WAR | CLR | ENC | BRD | RNG | RNG |
| G3 | SK | CLR | SHM | BRD | RNG | RNG |
| G4 | SK | DRU | ENC | BRD | RNG | MNK |
| G5 | WAR | CLR | SHM | BRD | RNG | RNG |
| G6 | PAL | CLR | ENC | BRD | WIZ | WIZ |

**Totals:** 3 WAR, 1 SK, 1 SK, 1 PAL, 5 CLR, 1 DRU, 2 SHM, 4 ENC, 6 BRD, 10 RNG, 1 MNK, 2 WIZ

**Rationale:**
- **6 Bards** — one per group, mandatory for twist buffs. Most impactful class for sustained grinding.
- **10 Rangers** — Dave confirmed: simplest to automate, eventually top DPS. Two per group fills DPS slots.
- **5 Clerics** — reliable healing, CH chain for raids. One per group except G4 (druid for ports + outdoor healing).
- **4 Enchanters** — charm/mez/clarity/haste. One per group minimum for CC-heavy content.
- **2 Shamans** — slows, buffs, backup healing. Cover groups that don't have an enchanter.
- **1 Druid** — ports (group travel), snare, backup heals, outdoor DPS.
- **2 Wizards** (G6) — ports for cross-zone travel, burst DPS for named mobs, raid utility.
- **1 Monk** — pulling specialist for dungeons where bard pulling is risky.

**Class strategy implementation gap:** Only WAR/CLR/ENC/generic are implemented. Need at minimum: BRD (twist engine), RNG (auto-attack + archery), SHM (slow + heal), SK (tank + lifetap). That's 4 new strategies before launch.

#### Alternative: Minimal Viable Composition

If class strategies can't all be implemented by launch:

| Group | Composition | Uses Only |
|-------|-------------|-----------|
| All 6 | WAR / CLR / ENC / RNG / RNG / RNG | Existing strategies + generic DPS |

This works with current code. Rangers use generic_dps (auto-attack). No bard, no shaman, no druid. Less efficient but deployable now.

---

## 5. Economy Viability

### Is Krono farming realistic with 36 boxes?

**Yes, but not immediately.**

#### Income Timeline Estimate

| Phase | Timeline | Income Source | Est. Plat/hr (all 36) | Krono Equivalent |
|-------|----------|---------------|------------------------|------------------|
| **Week 1-2** | Server launch | XP grinding, vendor trash | 50-200pp/hr | 0 (Krono too expensive early) |
| **Week 3-4** | Level 50+ | Named drops, rare tradeskill mats | 500-2000pp/hr | 0.1-0.5 Krono/day |
| **Month 2-3** | Kunark/Velious | Raid loot, high-end camps | 5,000-20,000pp/hr | 1-3 Krono/day |
| **Month 4+** | Established | Bazaar flipping, controlled camps, raids | 20,000-100,000pp/hr | 3-10 Krono/day |

**Key variables:**
- Krono price fluctuates wildly (100pp early → 10,000pp+ later)
- Free trade server means ALL loot is tradeable — massive advantage for 36-box
- Encounter locking means you can't be trained off a camp — huge for contested content
- Dave's intel: "buy Krono early when cheap, stockpile Krono not plat"

#### Break-Even Analysis

| Expense | Monthly Cost |
|---------|-------------|
| 36 subscriptions (if not f2p) | ~$540 (36 x $15) |
| Hardware (amortized) | ~$100 |
| LLM API (M6) | $50-100 |
| Electricity | ~$30 |
| **Total** | **~$720-770/month** |

Krono sells for ~$8-12 on secondary markets (varies by server age/demand).

**To break even:** Need ~65-95 Krono/month = ~2-3 Krono/day.

**Verdict:** Achievable within 2-3 months of sustained operation, IF:
1. Anti-detection keeps accounts alive
2. Loot automation works reliably
3. At least 3-4 groups can operate simultaneously
4. You're farming the right camps (instance content > open world for safety)

**Critical dependency:** The TLP subscription model. If Frostreaver requires All Access ($15/mo per account), that's $540/month before you earn anything. If there's a free-to-play period or cheaper tier, the math improves dramatically. **Clarify subscription requirements before creating all 36 accounts.**

---

## 6. M5/M6 (Soul Engine / LLM) Assessment

### Verdict: Useful but premature. Deprioritize below economy and class strategies.

#### What's genuinely useful

- **Anti-detection value:** Characters that emote, chat, and behave uniquely are harder to distinguish from humans. This is real and valuable.
- **GM response:** An LLM that can respond to GM whispers with contextual, in-character text is a significant anti-ban measure.
- **Player interaction:** Responding to tells makes characters look human. This reduces reports.

#### What's scope creep

- **"Characters simulate going to bed / waking up"** — Fun but irrelevant to Krono generation.
- **"Evolving moods, permanent personality shifts"** — Creative writing, not multibox automation.
- **"Characters debate in guild chat"** — Cute but risky (what if the LLM says something reportable?).
- **"Inter-character friendships and rivalries"** — Zero gameplay impact.
- **"Speech patterns evolve over time"** — Nobody is analyzing your bots' linguistic development.

#### The real question

> Is the Soul Engine worth $50-100/month in API costs and significant development time, compared to what that time and money could buy in combat reliability, loot automation, and anti-detection?

**Answer: No, not before launch.** After the operation is profitable and stable, Soul Engine is a fantastic quality-of-life / anti-detection enhancement. But building it before you have working loot automation is building the penthouse before the foundation is poured.

#### Recommended approach

1. **Pre-launch:** Implement simple, template-based idle behavior (random emotes, occasional /say from a phrase list, AFK messages). No LLM needed. 50 lines of code.
2. **Month 1:** Add GM detection + simple response system (template responses to common GM questions). Still no LLM.
3. **Month 2+:** If profitable and stable, add LLM-powered personality as a luxury upgrade.

---

## 7. Anti-Detection Recommendations

### What Frostreaver should do that it isn't

#### Tier 1 — Must Have Before Live Deployment

1. **Warden research** — Spend a day reading RedGuides forums on current Warden behavior. Specifically:
   - Does Warden scan loaded modules? (If yes, need module unlinking)
   - Does Warden scan for detour/trampoline patterns? (If yes, need different hooking strategy)
   - Does Warden checksum eqgame.exe code sections? (If yes, hooks will be detected)
   - What's the scan frequency? (Determines how aggressive you can be)

2. **Strip identifying strings** — Remove "DMFT", "dmft", "frostreaver", "Dave Mike Fun Times" from the compiled DLL. Use `#[no_mangle]` carefully. Audit with `strings dmft_dll.dll`.

3. **GM detection system** — Monitor for:
   - New spawns appearing without zone-in animation (GM teleport)
   - Tells from unknown characters (GM whisper test)
   - `/who` results showing [GUIDE] or [GM] tags
   - Characters with `GM` flag in spawn struct
   - Unusual spawn types (GM characters have a different spawn type)

4. **Innocent mode** — Global panic button that makes all 36 characters:
   - Stop combat immediately
   - Stop movement
   - Face random directions
   - Start random idle emotes (/sit, /yawn, /stretch)
   - Auto-respond to tells with "brb" or similar
   - Triggered by: GM detection, Discord command, hotkey, or player lingering nearby

5. **Per-character action jitter** — Currently movement has humanization. Extend to:
   - Cast times: +/- 50-200ms random delay per cast
   - Loot timing: 0.5-3s random delay before looting
   - Target switching: 100-500ms random delay
   - Buff casting: randomize buff order per session
   - Login times: already staggered (good)

#### Tier 2 — Should Have Within First Month

6. **Behavioral rotation** — Characters should not do the exact same thing every hour:
   - Occasional AFK (one character sits down for 2-10 minutes)
   - Occasional vendor runs (character leaves camp, vendors, returns)
   - Occasional zone-out and return (simulates "went to get a drink")
   - Vary pull paths and camp positioning slightly over time

7. **Chat simulation** — Without LLM, simple template system:
   - Occasional guild chat messages from a phrase bank
   - Respond to direct tells with canned responses ("hey", "sup", "kinda busy atm")
   - React to loot drops ("nice!", "grats", "ugh another one")

8. **Network-level diversification** — If possible:
   - Use a residential VPN to vary source IP
   - Or split accounts across 2-3 IPs
   - Vary login IP slightly between sessions

9. **Module hiding** — Unlink the DLL from PEB's module lists after initialization. This is standard for game cheats and well-documented. Warden's module scanning is the most common detection vector.

#### Tier 3 — Nice to Have

10. **Syscall-level hooking** — Instead of `retour` trampolines (user-mode hooks), consider:
    - Manual hook: overwrite function prologue with a jump, save original bytes
    - Hardware breakpoint hooks (DR registers) — invisible to memory scans
    - Less detectable than library-based detouring

11. **Heartbeat randomization** — The orchestrator polls all clients on a fixed 100ms tick. This creates a detectable timing pattern. Randomize per-client poll intervals (80-120ms).

12. **Log sanitization** — Ensure DLL logs don't contain information that could identify the operation if Warden exfiltrates log files. Use rotating, size-limited logs that auto-delete after 24 hours.

---

## 8. Architecture Strengths

Credit where due — several architectural decisions are excellent:

1. **Rust for the DLL** — Memory safety in injected code is a massive advantage. C++ DLL injection is a crash factory. Rust's borrow checker prevents most of the use-after-free and buffer overflow bugs that plague MQ2 plugins.

2. **Field-by-field reads** — Not reading C structs wholesale is the right call. EQ's structs have padding, alignment, and version differences. Individual offset reads are more resilient to patch drift.

3. **Shared memory + named pipes IPC** — Good split. High-frequency game state via shared memory (zero-copy, seqlock) and low-frequency commands via pipes (reliable, ordered). This is better than what MQ2 uses.

4. **Cross-platform dev** — Building on macOS with stubs and testing on Windows is excellent for development velocity. Most EQ bot projects require a Windows dev machine.

5. **Hot-updatable offset DB** — JSON-based offset database that can be swapped without rebuild. This will save hours on patch days.

6. **INI optimization research** — The `eq-ini-optimization.md` document is comprehensive and correct. The memory budget math checks out. The render strobing analysis is spot-on.

7. **WinEQ research** — Extracting techniques from WinEQ's open-source agent was smart. The render strobing priority identification is the single most important performance finding.

---

## 9. Architecture Concerns

1. **Single orchestrator = single point of failure** — If the orchestrator process crashes, 36 DLLs run headless. Need a supervisor process or implement the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` pattern from the INI doc.

2. **`retour` crate dependency** — The hooking library is a known quantity in the game hacking community. Its trampoline pattern may be in Warden's signature database. Consider a custom hooking implementation for production.

3. **MAIN_LOOP_OFFSET = 0 bug** — Documented in dll-injection-plan.md but not fixed. This means the first injection test WILL fail silently (hook is skipped). Must fix before testing.

4. **IPC naming mismatch** — Pipe names use `client_id` vs `PID`. Also documented, also not fixed. Will cause connection failures.

5. **No graceful shutdown** — No DLL unload path. If you need to update the DLL, you must restart EQ. During live operations, this means downtime.

6. **Navmesh gap** — The biggest functional gap. Waypoint-based navigation works for known routes but can't handle arbitrary movement, and recording routes for every zone is impractical. MQ2's Recast/Detour integration is the gold standard. This is the #1 feature gap.

---

## 10. Recommended Launch-Day MVP

**Cut scope ruthlessly. Ship what works.**

### Must-have for Day 1

- [ ] DLL injection working and stable (test this week)
- [ ] Integration loop wired up (orchestrator → IPC → DLL)
- [ ] Login automation for 36 clients (staggered, with retry)
- [ ] Basic combat (WAR/CLR/ENC + generic DPS for all others)
- [ ] Group formation automation (/invite sequence)
- [ ] Waypoint navigation for 3-5 key zones (manually recorded)
- [ ] LootAll automation
- [ ] Vendor trash selling
- [ ] Crash recovery (auto-restart + re-inject)
- [ ] GM detection + innocent mode
- [ ] Discord alerts (crashes, GMs, named spawns)

### Nice-to-have for Day 1

- [ ] Bard twist engine
- [ ] Ranger/Shaman class strategies
- [ ] Render strobing (massive performance gain)
- [ ] Buff rotation coordination
- [ ] EC tunnel trader

### Explicitly NOT needed for Day 1

- [ ] Soul Engine / LLM personality
- [ ] LLM API integration
- [ ] RL/behavioral learning
- [ ] Bazaar automation
- [ ] Price tracking
- [ ] Navmesh pathfinding (waypoints are fine for known routes)
- [ ] Vector DB for character memories
- [ ] Inter-character social dynamics

---

## 11. Timeline Assessment

**Current date:** 2026-03-28
**TLP launch:** ~May 2026 (assume May 15 for planning)
**Time remaining:** ~7 weeks

| Week | Focus | Deliverable |
|------|-------|-------------|
| 1 (Mar 29 - Apr 4) | DLL injection + integration | First successful injection, IPC working, MAIN_LOOP_OFFSET fixed |
| 2 (Apr 5 - Apr 11) | 6-client group test | Group invite, basic combat, crash recovery |
| 3 (Apr 12 - Apr 18) | Combat + class strategies | Bard twist, ranger, shaman. All 6 group slots playable. |
| 4 (Apr 19 - Apr 25) | Loot + economy basics | LootAll, vendor selling, plat consolidation |
| 5 (Apr 26 - May 2) | 36-client stress test | All clients running, stable over 8+ hours |
| 6 (May 3 - May 9) | Anti-detection + hardening | GM detection, innocent mode, string stripping, jitter tuning |
| 7 (May 10 - May 16) | Launch prep | Route recording for starting zones, final testing, launch-day playbook |

**Verdict:** Tight but doable IF:
- No major surprises with DLL injection (biggest risk)
- Offset calibration goes smoothly
- Development focus is on integration, not new features
- M5/M6/M7 are deferred entirely

**If DLL injection has problems:** Add 1-2 weeks for debugging. This is the critical path. Test it THIS WEEK.

---

## 12. Final Recommendations

1. **Test DLL injection immediately.** Everything else is blocked on this. Do it today or tomorrow.
2. **Reorder milestones:** Economy before Soul Engine. Loot automation before LLM personality.
3. **Build 4 more class strategies minimum:** Bard, Ranger, Shaman, Shadow Knight. The current 4 strategies can't cover the optimal group comp.
4. **Implement render strobing:** The WinEQ research identified this as a 98% GPU reduction. Without it, 36 clients may not be viable on one machine.
5. **Spend 2-3 days on anti-detection before going live.** GM detection + innocent mode + string stripping is the minimum viable anti-ban.
6. **Research Warden** before injecting on a live server. One scan, one detection, 36 bans. Know what you're up against.
7. **Start with 6 clients, not 36.** Get one group working perfectly before scaling. Dave's intel: "test with 1 group first" — this is the right call.
8. **Buy Krono early.** Dave's intel is correct — Krono appreciates on TLPs. Even manual farming + buying in week 1-2 has ROI.
9. **Have manual override capability.** Automation will break. Be able to manually control any character through the TUI when it does.
10. **Don't let perfect be the enemy of shipped.** The codebase is impressive. The architecture is sound. Stop building new milestones and start integrating what exists.
