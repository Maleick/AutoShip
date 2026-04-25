# textquest-learn: L-6 Offline RL Harness

Offline reinforcement learning training harness for TextQuest policies. Trains CQL (Conservative Q-Learning) and IQL (Implicit Q-Learning) policies on replay data from the L-1 experience ledger, warm-started from L-5 behavior-cloned policies.

## Architecture

```
                 L-5 BC Policy
                      │
                      ▼
    Ledger Data → CQL/IQL Training → ONNX Model
       (L-1)          │                   │
                      ▼                   ▼
                Offline Evaluation  Rust Inference
                 (WIS, FQE, OPE)    (<500 μs decision)
                      │
                      ▼
             Promotion Gate (beats BC?)
                      │
        ┌─────────────┴─────────────┐
        ▼                           ▼
      L-7 Canary              Archive + Report
```

## Key Features

### Training

- **CQL (Conservative Q-Learning)**: Penalizes out-of-distribution actions for safe policy improvement
- **IQL (Implicit Q-Learning)**: Value-based offline RL with implicit policy modeling
- Warm-start from L-5 BC policy for accelerated learning
- Configurable epochs (default: 200, ≤30 min on single GPU)

### Evaluation

**Offline Policy Evaluation (OPE)** — no online exploration:

1. **Weighted Importance Sampling (WIS)** — variance-capped
   - Estimates policy return from off-policy data
   - Caps importance weights to reduce variance
2. **Fitted Q Evaluation (FQE)** — cross-check
   - Fits Q-function to evaluate policy
   - Provides independent confirmation of WIS

3. **Action Distribution Coverage** — sanity check
   - Ensures learned policy stays within behavioral support
   - Requires >80% overlap with BC actions

### Promotion Gate

Policy is promoted to L-7 canary if and only if:

- WIS mean > BC baseline WIS mean (confidence bounds don't overlap)
- FQE mean > BC baseline FQE mean (confidence bounds don't overlap)
- Action coverage > 80%

Otherwise, archived with diagnostic report.

### Artifact Manifest

Each policy carries:

```json
{
  "version": "0.6.0",
  "algorithm": "cql",
  "class": "cleric",
  "reward_spec": "combat.heal.cleric.v1",
  "training_data_hash": "abc123...",
  "context_schema_version": "2024-04",
  "git_sha": "def456...",
  "ope_report": {
    "wis_mean": 42.5,
    "wis_lower_ci": 40.2,
    "wis_upper_ci": 44.8,
    "fqe_mean": 41.8,
    "fqe_lower_ci": 39.5,
    "fqe_upper_ci": 44.1,
    "action_coverage": 0.92,
    "bc_baseline_wis": 38.0,
    "beats_bc": true
  },
  "bc_warm_start": "cleric.bc.v1.onnx"
}
```

## CLI

```bash
textquest-learn rl train \
    --algo cql \
    --warm-start cleric.bc.v1.onnx \
    --ledger runs/ \
    --reward combat.heal.cleric.v1 \
    --class cleric \
    --epochs 200 \
    --out cleric.rl.v1.onnx
```

## Python Sidecar

The Python trainer (`python/trainer.py`) handles:

1. Loading L-1 ledger trajectories
2. Loading L-5 BC warm-start (ONNX)
3. Running CQL/IQL training with PyTorch
4. Exporting trained policy to ONNX

Dependencies: `torch`, `numpy`, `onnx`

### Installation

```bash
cd textquest-learn/python
pip install -r requirements.txt
```

## ONNX Export

Policies are exported as ONNX with:

- Input: state vector (f32, shape [batch, state_dim])
- Output: action logits (f32, shape [batch, action_dim])
- Target: Rust inference via `ort` crate at ≤500 μs/decision

## Rust Integration

The Rust crate provides:

- **policy.rs**: Policy artifact holder and manifest serialization
- **evaluator.rs**: WIS, FQE, and action coverage evaluators
- **py_trainer.rs**: Spawn Python training sidecar

Evaluation uses exact importance sampling with variance capping to prevent
numerical instability on small batch sizes.

## Acceptance Criteria ✓

- [x] CQL + IQL training harnesses (Python sidecar)
- [x] ONNX export with PyTorch
- [x] WIS + FQE evaluators (Rust)
- [x] Action coverage sanity check
- [x] Policy manifest with OPE report
- [x] Promotion gate (beats BC check)
- [x] CLI: `textquest-learn rl train --algo {cql,iql} ...`
- [x] Refuse to promote if doesn't beat BC

## Notes

- Training is fully offline; no online exploration against live EQ client
- All evaluation metrics use confidence bounds; promotion requires non-overlapping CIs
- Action coverage ensures policy respects behavioral distribution
- Manifest includes git SHA and training-data hash for reproducibility

## Related Issues

- Parent: #2708 (M12 epic)
- Depends on: L-1 (ledger), L-2 (rewards), L-5 (BC warm-start)
- Feeds: L-7 (canary), L-8 (policy store)
