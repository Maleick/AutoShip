# Pre-Launch Automation Validation

> **Purpose:** Validate the full automation stack works under live/test server conditions before deploying a multi-client session. Run this checklist after any major code change, DLL rebuild, or config migration.
>
> **Parent issue:** #1575 — Pre-launch readiness

---

## Checklist

### 1. DLL Load Verification

| # | Check | Method | Pass Criteria |
|---|-------|--------|---------------|
| 1.1 | TextQuest DLL injects without crash | Launch one EQ client, attach injector | Process stays alive >30 s, no crash dialog |
| 1.2 | IPC pipe connects | `textquest status <account>` in TUI | State transitions to `Connected` |
| 1.3 | Heartbeat loop active | Monitor TUI dashboard for >60 s | Heartbeat counter increments every 6 s (server tick) |
| 1.4 | No anti-cheat trip on inject | Check EQ client chat window | No "You have been disconnected" message within 2 min |

**Blocking:** Any failure in 1.1–1.3 stops the rest of the checklist. Log to `.wolf/buglog.json` and open a GitHub issue before proceeding.

---

### 2. Camp Config Active

| # | Check | Method | Pass Criteria |
|---|-------|--------|---------------|
| 2.1 | `config/accounts.toml` loads without error | `textquest config validate` | Exit code 0, no WARN/ERROR lines |
| 2.2 | Camp TOML parses | `:config reload` in TUI | TUI status bar shows config version/hash, no parse error overlay |
| 2.3 | Target zone matches server | Compare config `zone` field vs. active server zone | Zone name resolves; no "unknown zone" log line |
| 2.4 | Profile group membership correct | `:profile list` in TUI | All expected accounts appear in the correct group |
| 2.5 | Pull radius and camp center set | Review camp TOML `[camp]` section | `center`, `radius`, and `pull_radius` fields present and non-zero |

---

### 3. Pull Cycle Works

| # | Check | Method | Pass Criteria |
|---|-------|--------|---------------|
| 3.1 | Puller reaches camp center | Issue `:camp start` for a single puller | Character navigates to camp center within 15 s |
| 3.2 | Pull target acquired | Wait for next spawn cycle | TUI shows a pull target selected (zone-dependent) |
| 3.3 | Pull completes without timeout | Observe one full pull cycle | Target reaches camp, combat starts, no "pull timeout" log |
| 3.4 | Camp loop restarts after kill | Wait for mob death | Puller returns to camp center; loop counter increments |
| 3.5 | Multi-client pull coordination | Launch full group, `:camp start all` | No duplicate pulls; assist chain fires in class order |

---

### 4. Login Chain (if cold-starting clients)

| # | Check | Method | Pass Criteria |
|---|-------|--------|---------------|
| 4.1 | AutoLogin stages advance | `:login all`, watch TUI phases | Each client progresses: `AtLoginScreen → EnteringCredentials → ServerSelecting → InGame` |
| 4.2 | No stuck-at-EULA | Monitor for 90 s post-launch | No client stays in `AtLoginScreen` >60 s (EULA regression indicator) |
| 4.3 | Character select resolves | After server select | `InGame` phase reached without manual input |

---

### 5. Baseline Smoke Test (Live Server)

Run after all above checks pass on a test/dev session.

```
# Run dev preflight (non-cargo path)
python3 scripts/dev-preflight.py
```

Expected: all preflight gates PASS. Any WARN is noted; any FAIL is blocking.

---

## Pass/Fail Result Log

Fill this out after each validation run. Append; do not overwrite previous runs.

```
## Run: YYYY-MM-DD — <operator> — <server/zone>

### DLL Load
- [ ] 1.1 Inject clean
- [ ] 1.2 IPC connects
- [ ] 1.3 Heartbeat active
- [ ] 1.4 No AC trip

### Camp Config
- [ ] 2.1 accounts.toml valid
- [ ] 2.2 Config reload clean
- [ ] 2.3 Zone resolves
- [ ] 2.4 Profile groups correct
- [ ] 2.5 Pull config present

### Pull Cycle
- [ ] 3.1 Puller to camp center
- [ ] 3.2 Pull target acquired
- [ ] 3.3 Pull completes
- [ ] 3.4 Loop restarts
- [ ] 3.5 Multi-client pull OK

### Login Chain (if applicable)
- [ ] 4.1 Phases advance
- [ ] 4.2 No EULA stuck
- [ ] 4.3 InGame reached

### Preflight
- [ ] 5. dev-preflight.py PASS

### Blocking Issues
<!-- List issue numbers or "none" -->

### Notes
<!-- Observations, timing anomalies, environment details -->
```

---

## Known Gaps / Blocking Issues

> Update this section when a validation run reveals a gap. Reference the GitHub issue number.

| Gap | Issue | Status |
|-----|-------|--------|
| Navmesh-based pull navigation not yet validated under live latency | #TBD | Open |
| Zone-line transition during pull cycle not covered | #TBD | Open |

---

## References

- [DLL Injection and IPC Pipeline](./DLL-Injection-and-IPC-Pipeline.md)
- [Camp Runbooks](./Camp-Runbooks.md)
- [Login Automation](./Login-Automation.md)
- [Configuration](./Configuration.md)
- [Combat and Camp Loop](./Combat-and-Camp-Loop.md)
