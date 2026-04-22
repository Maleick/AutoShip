# Epic #2118 — Production Deployment Paths: CARGO_MANIFEST_DIR Removal

**Source audit:** `audit/AUDIT-2026-04-19.md`
**Filed:** 2026-04-21
**Status:** Decomposed — 6 child issues filed

## Problem

`env!("CARGO_MANIFEST_DIR")` bakes the build-machine filesystem path into release
binaries at compile time. On Frostreaver, where the binary runs outside the repo
tree, all path-dependent features silently 404 (SPA assets) or 501 (credential
routes) because the resolved paths don't exist on the deployment machine.

`TEXTQUEST_DATA_DIR` is already partially established in `textquest-web/src/main.rs`
(lines 204–211) as the canonical runtime override. The fix pattern is consistent
across all occurrences:

```
TEXTQUEST_DATA_DIR env var  →  current_exe().parent()  →  "." (cwd fallback)
```

## Occurrences Found (20 total across 4 crates)

| Crate         | File                              | Lines                   | Category               |
| ------------- | --------------------------------- | ----------------------- | ---------------------- |
| textquest-web | `src/ws.rs`                       | 119, 298                | runtime snapshot path  |
| textquest     | `src/orchestrator/mod.rs`         | 67, 71                  | config + session JSON  |
| textquest     | `src/alerts.rs`                   | 44                      | alert DB ancestor walk |
| textquest     | `src/tui/app.rs`                  | 3440                    | TUI base path          |
| textquest     | `src/tui/hotkeys/mod.rs`          | 644                     | hotkeys config         |
| textquest     | `src/tui/demo_data.rs`            | 1971                    | maps directory         |
| textquest     | `src/camp/config.rs`              | 280                     | camp config TOML       |
| textquest     | `src/camp/class_config.rs`        | 632, 704, 811, 865, 907 | class config files     |
| textquest-dll | `src/combat/classes/paladin.rs`   | 978, 1075               | test fixtures          |
| textquest-dll | `src/combat/classes/enchanter.rs` | 603                     | test fixture           |
| textquest-dll | `src/lib.rs`                      | 745, 770                | debug diagnostics      |

## Child Issues

| #                                                         | Title                                                             | Priority | Scope                                      |
| --------------------------------------------------------- | ----------------------------------------------------------------- | -------- | ------------------------------------------ |
| [#2250](https://github.com/Maleick/TextQuest/issues/2250) | Replace CARGO_MANIFEST_DIR in `textquest-web/src/ws.rs`           | HIGH     | 2 lines, ws runtime path                   |
| [#2254](https://github.com/Maleick/TextQuest/issues/2254) | Replace CARGO_MANIFEST_DIR in `textquest/src/orchestrator/mod.rs` | HIGH     | 2 lines, orchestrator config/session paths |
| [#2257](https://github.com/Maleick/TextQuest/issues/2257) | Replace CARGO_MANIFEST_DIR in `textquest/src/alerts.rs`           | HIGH     | 1 line, alert DB resolution                |
| [#2259](https://github.com/Maleick/TextQuest/issues/2259) | Replace CARGO_MANIFEST_DIR in TUI + camp modules                  | MEDIUM   | 9 lines across 6 files                     |
| [#2261](https://github.com/Maleick/TextQuest/issues/2261) | Remove CARGO_MANIFEST_DIR from textquest-dll (test/debug guard)   | LOW      | 5 lines, test-only or debug                |
| [#2263](https://github.com/Maleick/TextQuest/issues/2263) | Document TEXTQUEST_DATA_DIR in release guide                      | DOCS     | RELEASE_TEMPLATE.md + deployment guide     |

## Rationale for Grouping

- **#2250** — Isolated to `ws.rs`, directly causes 501 on credential routes. Smallest blast radius, dispatch first.
- **#2254** — Orchestrator paths are critical for session management. Two-function fix.
- **#2257** — Alert DB path mismatch causes TUI/web dashboard split. Requires understanding of fallback resolution order.
- **#2259** — Largest surface area (6 files) but all follow the same `data_dir().join(...)` substitution pattern. Batch dispatch.
- **#2261** — DLL occurrences are mostly test-only; production impact is low. Lowest priority.
- **#2263** — Must ship after #2250/#2254 so the documented env var matches the actual resolver behavior.

## Shared Fix Pattern

```rust
// Before (compile-time — breaks on Frostreaver)
let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/foo.toml");

// After (runtime resolver)
fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("TEXTQUEST_DATA_DIR") {
        return PathBuf::from(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.to_path_buf();
        }
    }
    PathBuf::from(".")
}

let path = data_dir().join("config/foo.toml");
// Note: "../" prefix removed — deployed layout is flat beside the binary
```

## Deployment Layout (Frostreaver Target)

```
TextQuest/
├── textquest.exe        # TUI binary
├── textquest-web.exe    # Web server binary
├── config/
│   ├── character-configs.json
│   ├── camps/
│   └── maps/
└── data/
    └── runtime/
        ├── live_sessions.json
        └── admin_sessions.json
```

`TEXTQUEST_DATA_DIR` should point to the `TextQuest/` directory above.
