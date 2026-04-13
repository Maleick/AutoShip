# CheaterLdFlag Address Research

## Discovery Summary
- `CheaterLdFlag` string format: `"CheaterLdFlag=%d\n"`
- Address: `0x140afebc8` (`CHEATER_LD_FLAG_STRING`)
- Address: `0x140afed90` (`CHEATER_LD_FLAG_VAR`)

## Search Findings
- Searched for `CheaterLdFlag` references in:
  - `textquest-common/src/offsets.rs`
  - project-wide source tree (`rg -n "CheaterLdFlag"`)
- Existing codebase currently has **no prior `CheaterLdFlag` references**.
- Prior to this change, existing anti-cheat related constants were under
  `textquest-common/src/offsets.rs`, including `MEMCHECK4_PROCESS_ENUM`,
  `FILE_INTEGRITY_DISPATCHER`, and network/auth helpers.

## Interpretation
`CheaterLdFlag` appears to be a global anti-cheat state value in `eqgame.exe`. The presence of a dedicated
format string and flag variable strongly suggests:
- A runtime write/read path updates a cheat-detection score/state.
- Non-zero values likely indicate suspicious behavior or an active cheat state.
- Reading this memory can indicate when protection logic is becoming active.

## Monitoring / Diagnostics Guidance
- Poll `CHEATER_LD_FLAG_VAR` via `read_cheater_ld_flag` and log value transitions over time.
- Treat non-baseline values (typically anything other than `0`) as a diagnostic signal that anti-cheat logic may be active.
- Emit an alert when the value changes from its normal baseline, and include relevant context such as timestamp, character/session, and recent system activity.
- Preserve surrounding telemetry so responders can correlate the flag transition with recent commands, memory reads, process events, or other anti-cheat-related checks.
- For incident response, review recent logs, capture the current state for analysis, and investigate what change in application behavior or environment preceded the transition.

