# AGENTS.md — src/chain/ (Blockchain Integration)

## Purpose

Integrates with the Bittensor blockchain for block synchronization, epoch calculation, and on-chain evaluation result submission.

## Module Structure

| File | Purpose |
|------|---------|
| `block_sync.rs` | `BlockSync` — subscribes to new blocks, tracks network state |
| `epoch.rs` | `EpochCalculator` — calculates epoch boundaries, phases, transitions |
| `evaluation.rs` | `BlockchainEvaluationManager` — submits evaluation results to chain, manages consensus |

## Key Constants

- `DEFAULT_TEMPO` — default epoch length in blocks
- `EPOCH_ZERO_START_BLOCK` — genesis epoch start
- `MINIMUM_STAKE_RAO` — minimum stake for validator participation
- `MINIMUM_VALIDATORS` — minimum validators needed for consensus
