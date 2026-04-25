# 2462 — OpenVanilla foreground handoff race cross-check

**Issue:** #2462
**Date:** 2026-04-25
**Evidence:** `https://github.com/RedGuides/openvanilla/commit/16605f7c43ed3284b791547c2df3bff0f9aa7310`

## Upstream fix summary (`Fix foreground in some situations`)

MQ changed foregrounding in two places:

- `src/loader/MacroQuest.cpp::SetForegroundWindowInternal`
- `src/main/MQPostOffice.cpp::RequestActivateWindow`

Both paths now:

1. Read current foreground thread via `GetWindowThreadProcessId(GetForegroundWindow(), nullptr)`.
2. Attach input queue with `AttachThreadInput(ourThread, fgThread, true)` when thread IDs differ.
3. Call `BringWindowToTop(hWnd)` then `SetForegroundWindow(hWnd)`.
4. Restore attachment with `AttachThreadInput(..., false)` in a scope-exit.
5. Use `SetFocus(hWnd)` and only fallback to secondary logic if `GetForegroundWindow() != hWnd`.

## TextQuest audit

- `textquest/src/window_title_runtime.rs` only loads title templates and sends `SetWindowTitleConfig` IPC. No Win32 foreground APIs.
- `textquest/src/launcher/` spawns/moves account state only; no HWND or foreground APIs are called.
- `textquest-dll/src/overlay/` focus logic is internal overlay widget focus state only; no `SetForegroundWindow`-style activation.
- Repo-wide search found no `SetForegroundWindow`, `AttachThreadInput`, or `AllowSetForegroundWindow` calls.

## Conclusion

No equivalent foreground race exists in current TextQuest code paths inspected for this issue, so a fix mirroring openvanilla was not applied. If future code introduces direct foreground activation APIs, apply the upstream attach/detach sequence in the same order for multi-client handoff safety.
