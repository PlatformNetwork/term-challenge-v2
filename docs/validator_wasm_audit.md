# Validator/Core/P2P/WASM Audit Notes

## Scope

Reviewed: `bins/validator-node`, `crates/core`, `crates/p2p-consensus`, `crates/challenge-registry`, `crates/wasm-runtime-interface`.

## Key Findings

### Validator Node
- `bins/validator-node` wires consensus + storage with WASM challenge execution integrated into the validator node runtime path.

### Challenge Registry
- Registry entries store WASM module metadata as primary. `ChallengeEntry` includes WASM module hash, path, and network policy.
- `discovery` supports WASM module registry and signed P2P announcements for challenge distribution.

### Core State
- `core::ChainState` includes WASM `wasm_challenge_configs` for challenge configuration.
- `core::ChallengeConfig` stores WASM module metadata (hash/path/entrypoint/policy) for WASM-only evaluation.

### P2P Consensus
- `p2p-consensus::ChainState` stores `ChallengeConfig` with WASM module metadata and weight allocation.
- Consensus engine is PBFT-style and uses validator stake data from `ValidatorSet`. Stake is taken from heartbeats unless verified stake is set (metagraph refresh uses `set_verified_stake`), which is a potential gap if verified stakes are not enforced.

### WASM Runtime Interface
- Runtime is strict and well-structured: `NetworkPolicy` with validation, explicit host functions, request limits, and audit log hooks.
- No apparent recursion; resource caps are enforced via wasmtime `StoreLimits` and request limits.
- Runtime interface is integrated into the validator execution path.

## Cleanup / Follow-up Recommendations

1. **Unify challenge configs**
   - Ensure all challenge configs in `core::ChainState` and `p2p-consensus::ChainState` use WASM-only representations with `WasmChallengeConfig` metadata and network policy.
2. **Registry WASM-only**
   - `ChallengeEntry` should store WASM module metadata as primary.
   - `discovery` should focus on WASM module registry or signed P2P announcements.
3. **Consensus state challenge metadata**
   - Ensure `p2p-consensus::ChallengeConfig` uses WASM module metadata (hash/path/entrypoint/policy) for WASM-only evaluation.

## Suggested Next Steps

- Align registry and core state to store WASM metadata only, with migration of existing state.
- Continue hardening WASM runtime execution policies and audit logging.
