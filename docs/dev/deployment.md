# Frostreaver Deployment Guide

This guide captures the minimum runtime environment needed for production-style deployment on Frostreaver and similar single-machine hosts.

## Deployment checklist

1. Build the binaries and copy both executables to the deployment directory:
   - `textquest.exe`
   - `textquest-web.exe`
2. Copy required runtime assets next to the binaries:
   - `config/` tree (character config, camps, maps, alerting config)
   - `data/` tree (credentials DB, alert DB, runtime snapshots)
3. Set required environment variables before service startup.
4. Start `textquest.exe` and `textquest-web.exe` in any order.
5. Verify runtime health:
   - `/ws` socket responds
   - dashboard pages load without 404/501 for web assets/config routes
   - session snapshots appear under `data/runtime/`
   - logs are written under the resolved data/log directory

## Required environment variables

| Variable | Required | Value | Fallback |
|----------|----------|-------|----------|
| `TEXTQUEST_DATA_DIR` | Yes (production) | Absolute path to deployment root | If unset, runtime uses executable parent, then `.` |
| `TEXTQUEST_ALERT_DB_PATH` | No | Absolute path to `alerts.db` (for example `C:\\TextQuest\\live\\alerts.db`) | If unset, falls back to `TEXTQUEST_DATA_DIR` + `data/alerts.db` behavior in alert resolver |

## Frostreaver layout example

`TEXTQUEST_DATA_DIR` should point to the root folder that contains both `config/` and `data/`:

```text
TextQuest/
├── textquest.exe
├── textquest-web.exe
└── config/
    ├── character-configs.json
    ├── camps/
    ├── maps/
    └── alerting.toml
└── data/
    ├── credentials.db
    ├── alerts.db
    └── runtime/
        ├── live_sessions.json
        ├── admin_sessions.json
        └── live_spawns.json
```

Then set:

```powershell
$env:TEXTQUEST_DATA_DIR = "C:\TextQuest\live"
```

To move only the alert database file:

```powershell
$env:TEXTQUEST_ALERT_DB_PATH = "D:\textquest-state\alerts.db"
```

## Notes

- `TEXTQUEST_DATA_DIR` is the canonical override for all runtime path resolution touched by the recent deployment-path fixes.
- `TEXTQUEST_ALERT_DB_PATH` is only for alert DB override and does not affect other data paths.
- Always use absolute paths on Frostreaver; relative values can resolve incorrectly under service launch contexts.
