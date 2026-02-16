# AGENTS.md — src/weights/ (Weight Calculation)

## Purpose

Calculates miner weights for Bittensor emission based on agent evaluation scores. Implements scoring aggregation, time-based decay, reward decay, and weight distribution to validators.

## Module Structure

| File | Purpose |
|------|---------|
| `scoring.rs` | `ScoreCalculator` — aggregates task scores into benchmark scores |
| `decay.rs` | `RewardDecayManager` — applies decay to stale agent scores over time |
| `time_decay.rs` | Time-based decay configuration and calculation |
| `emission.rs` | `EmissionManager`, `WeightCalculator` — final weight calculation for chain submission |
| `distribution.rs` | `ValidatorDistributor` — distributes compiled agent binaries to validators |

## Key Formula

```
Score = tasks_passed / total_tasks
Weight = score / sum(all_scores)  (stake-weighted)
```

Decay is applied to agents that haven't been re-evaluated recently, incentivizing continuous improvement.
