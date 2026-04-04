# GhidraMCP API Reference

**Server:** Bethington fork v4.3.0 (176 endpoints)
**Access:** `http://127.0.0.1:8089/` via SSH tunnel to Frostreaver
**Harvested:** 2026-04-03

## Working Endpoints

### List Endpoints (bulk data)

| Endpoint | Description | Count | Format | Notes |
|----------|-------------|-------|--------|-------|
| `GET /list_functions` | All functions | 19,542 | `NAME at ADDRESS` | No default limit |
| `GET /list_strings?limit=N` | All strings | 21,982 | `ADDRESS: "VALUE"` | Default limit: 99 |
| `GET /list_imports?limit=N` | All imports | 427 | `NAME -> ADDRESS` | Default limit: 99 |
| `GET /list_classes?limit=N` | RTTI class names | 882 | One per line | Default limit: 99 |
| `GET /list_exports` | Exports | 1 | `NAME -> ADDRESS` | Only `entry` |
| `GET /list_namespaces?limit=N` | Namespaces | 882 | One per line | Same as classes |
| `GET /list_segments` | Memory segments | 9 | `NAME: START - END` | No limit needed |
| `GET /list_bookmarks` | Analysis bookmarks | 4,229 | JSON `{"bookmarks":[...]}` | No limit needed |
| `GET /list_data_types?limit=N` | Data types | 1,412 | `NAME \| SOURCE \| SIZE \| PATH` | Default limit: 99 |

### Query Endpoints (per-item)

| Endpoint | Description | Format |
|----------|-------------|--------|
| `GET /decompile_function?name=NAME` | Decompile a function by name | C pseudocode |
| `GET /get_function_by_address?address=0xADDR` | Get function at address | Function info |
| `GET /search_functions?name=TERM` | Search functions by substring | List of matches |

### Info Endpoints

| Endpoint | Description | Format |
|----------|-------------|--------|
| `GET /get_version` | Plugin version + metadata | JSON |

## Non-Working Endpoints (404)

These return `404 Not Found`:
- `list_data`
- `list_labels`
- `list_references`
- `list_memory_blocks`
- `list_defined_data`
- `list_external_functions`

## Pagination

Most list endpoints default to **99 entries**. Pass `?limit=50000` to get full results.
The `/list_functions` endpoint returns all entries without a limit.
The `/list_bookmarks` endpoint returns all entries as JSON.

## File Inventory

| File | Format | Records |
|------|--------|---------|
| `functions.json` | `[{"name", "address"}]` | 19,542 |
| `strings.json` | `[{"address", "value"}]` | 21,982 |
| `imports.json` | `[{"name", "address"}]` | 427 |
| `classes.json` | `["ClassName"]` | 882 |
| `namespaces.json` | `["Namespace"]` | 882 |
| `exports.json` | `[{"name", "address"}]` | 1 |
| `segments.json` | `[{"name", "start", "end", "size_bytes"}]` | 9 |
| `bookmarks.json` | `[{"address", "category", "comment", "type"}]` | 4,229 |
| `data_types.json` | `[{"name", "source", "size_bytes", "category_path"}]` | 1,412 |
| `metadata.json` | Plugin info + harvest counts | 1 |
| `*_raw.txt` | Raw API responses | — |
