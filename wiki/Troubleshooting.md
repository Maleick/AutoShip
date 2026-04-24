# Troubleshooting

## Setup finds no models

Run:

```bash
opencode models
```

If the expected provider models are missing, reconnect the provider in OpenCode and rerun setup.

## Manual model edits disappeared

Setup preserves `.autoship/model-routing.json` by default. It regenerates only when you use `AUTOSHIP_REFRESH_MODELS=1` or provide `AUTOSHIP_MODELS=...`.

## Workers are queued but not running

Run:

```bash
bash hooks/opencode/status.sh
bash hooks/opencode/runner.sh
```

The runner starts queued work up to the configured active cap.

## Unsafe issue was blocked

This is expected for anti-cheat evasion, stealth, VM/fingerprint evasion, shellcode, hook signature evasion, detour hiding, or similar abuse-prone work. Review manually before proceeding.

## Status looks stale

Run:

```bash
bash hooks/opencode/reconcile-state.sh
bash hooks/opencode/status.sh
```
