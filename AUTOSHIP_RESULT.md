# Result: #2462 — Cross-check openvanilla Fix foreground in some situations (thread-input attachment) against TextQuest foreground paths

Outcome: **No-applicable race path found.**

- Upstream openvanilla commit `16605f7` adds `AttachThreadInput` around `SetForegroundWindow` activation with fallback checks.
- TextQuest codepaths reviewed (`textquest/src/window_title_runtime.rs`, `textquest/src/launcher/`, `textquest-dll/src/overlay/`) do not call `SetForegroundWindow`, `AttachThreadInput`, `AllowSetForegroundWindow`, or related foreground APIs.
- Audit note added: `docs/research/2462-openvanilla-foreground-race-audit.md`.
- Patch-day log updated: `docs/wiki/Research-Patch-Day-Reproduction.md`.

Next step: none (documentation complete).
