---
name: textquest-patch-day
description: Repeatable workflow for EverQuest patch day offset research and TextQuest updates
---

# TextQuest Patch Day Workflow

Monthly workflow for updating TextQuest after EverQuest patch day.

## Prerequisites

1. **Ghidra Docker container** must be running:
   ```bash
   docker start ghidra-mcp
   ```

2. **New binary** must be in: `~/Projects/TextQuest-Ghidra/staging/live/YYYY-MM-DD/eqgame.exe`

## Load New Binary

Copy the new binary to the container and load it:

```bash
# From host, copy binary to container
docker cp ~/Projects/TextQuest-Ghidra/staging/live/YYYY-MM-DD/eqgame.exe ghidra-mcp:/data/eqgame.exe

# Load binary (use param name "file" not "path"!)
curl -s "http://127.0.0.1:8089/load_program" -X POST \
  -H "Content-Type: application/json" \
  -d '{"file": "/data/eqgame.exe", "language": "x86:LE:64:default", "compiler": "windows"}'
```

## Search Binary

Key endpoints for finding offset addresses:

```bash
# List all functions
curl -s "http://127.0.0.1:8089/list_functions"

# Find functions in address range (example: 0x140055xxx)
curl -s "http://127.0.0.1:8089/list_functions" | grep "^FUN_140055"

# Get function decompile
curl -s "http://127.0.0.1:8089/decompile_function?program=eqgame.exe&address=140055030"

# List global variables
curl -s "http://127.0.0.1:8089/list_globals"

# List imports/exports
curl -s "http://127.0.0.1:8089/list_imports"
curl -s "http://127.0.0.1:8089/list_exports"
```

## Key Address Ranges (Old to New)

| Old Address  | What to Find                          | New Address |
| ----------- | ------------------------------------ | ---------- |
| 0x140563130 | Network send function                | 0x140563330 (+0x200) |
| 0x14001a460 | Message counter heartbeat           | 0x1401a4650 (+0xF0) |
| ~0x14021d730 | File integrity check dispatcher      | 0x140564bc0 (moved)      |

## Update Offsets

1. Update `textquest-common/src/offsets.rs`:
   - `CLIENT_DATE`: "YYYYMMDD"
   - `EXPECTED_VERSION_DATE`: "Mon DD YYYY"

2. Document new addresses in `docs/wiki/Research-Anti-Detection.md`

## CLI Reference

| Command                                | Description                    |
| -------------------------------------- | ------------------------------ |
| `docker start ghidra-mcp`              | Start container                |
| `docker exec ghidra-mcp ls -la /data/` | List binaries in container    |
| `curl http://127.0.0.1:8089/...`       | Query Ghidra API               |

## Notes

- Ghidra running at `http://127.0.0.1:8089`
- Use param name `file` NOT `path` for `/load_program` endpoint
- Decompilation times out on large functions - search smaller ones first
- Function list available via `list_functions` endpoint