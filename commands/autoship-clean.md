---
description: Clean terminal AutoShip workspaces
---

```bash
bash hooks/opencode/clean.sh
```

To prune rebuildable Rust build artifacts from retained AutoShip workspaces without deleting the workspaces or active `RUNNING`/`VERIFYING`/`ACTIVE` workspaces:

```bash
bash hooks/opencode/clean.sh --build-artifacts
```
