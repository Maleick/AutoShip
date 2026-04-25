# TTS Backends

TextQuest now exposes a pluggable TTS layer in the `textquest-voice` crate.
The default path is local-first, with cloud synthesis opt-in.

## Backends

| Backend | Default | Notes |
| --- | --- | --- |
| Coqui XTTS-v2 | Yes | Local clone-based backend. Uses a 6-second WAV sample as the voice reference. |
| Piper | No | Local fallback for lower-end systems with fixed voice models. |
| SAPI 5 | No | Windows system fallback. On macOS/Linux it resolves to a no-op stub. |
| ElevenLabs | No | Cloud-only backend. Requires an API key and remains opt-in. |

## Per-character voice config

Character personality files can select a backend and reference voice:

```toml
[voice]
backend = "coqui"
voice_id = "~/.textquest/voices/motherly_cleric.wav"
speed = 1.0
pitch = 0.0
```

The config loader accepts the `voice` table from TOML and defaults to Coqui when
`backend` is omitted.

## Starter voice pack

The bundled starter pack includes eight archetypes:

- `gruff_warrior.wav`
- `sardonic_wizard.wav`
- `motherly_cleric.wav`
- `methodical_paladin.wav`
- `theatrical_bard.wav`
- `terse_rogue.wav`
- `gentle_druid.wav`
- `dry_enchanter.wav`

Operator-supplied clips in `~/.textquest/voices/<name>.wav` take precedence over
the bundled starter file with the same name.

## Current implementation note

The bundled clips are valid 6-second WAV assets checked into the repository so
the path-resolution and packaging flow can be verified locally.
