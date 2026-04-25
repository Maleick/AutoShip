# Result: #727 — Per-patch detection function diffing via MemoryQuest pipeline
- Added detection-aware anti-cheat fingerprinting to `scripts/collect_patch_evidence.py`.
- Manifest now emits `anti_cheat_diffs` and `detection_change_gate` in live/test module comparisons.
- Added detection signature rules for SystemFingerprint, CheaterLdFlag strings, VM detection APIs, and module enumeration API/function chain.
- Updated `feature-list.json` to mark issue-727 complete.
