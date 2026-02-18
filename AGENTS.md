# AGENTS.md — Term Challenge

## Project Purpose

Term Challenge is a WASM evaluation module for AI agents on the Bittensor network via platform-v2. Miners submit Python agent packages (as zip files) that solve SWE-bench tasks. The WASM module runs inside platform-v2 validators to validate submissions, evaluate task results, and compute scores. A companion native CLI (`term-cli`) provides a TUI for monitoring leaderboards, evaluation progress, and network health.

## Architecture Overview

```
term-challenge/
├── Cargo.toml          # workspace with members = ["wasm", "cli"]
├── wasm/
│   ├── Cargo.toml      # cdylib, depends on platform-challenge-sdk-wasm
│   └── src/
│       ├── lib.rs           # Challenge impl + register_challenge!
│       ├── types.rs         # Submission, TaskDefinition, AgentLogs, etc.
│       ├── scoring.rs       # Aggregate scoring, decay, weight calculation
│       ├── tasks.rs         # Active dataset storage (SWE-bench tasks)
│       ├── dataset.rs       # Dataset selection and consensus logic
│       ├── routes.rs        # Challenge route definitions for RPC
│       └── agent_storage.rs # Agent code & log storage functions
├── cli/
│   ├── Cargo.toml      # native binary, ratatui TUI
│   └── src/
│       ├── main.rs     # Entry point, event loop
│       ├── app.rs      # Application state
│       ├── ui.rs       # Ratatui UI rendering
│       └── rpc.rs      # JSON-RPC 2.0 client
├── AGENTS.md
├── README.md
├── LICENSE
├── CHANGELOG.md
└── .githooks/
```

### Data Flow

1. **Miner** submits a zip package with agent code and task results
2. **RPC** receives submission, verifies signature, relays to validators
3. **Validators** run WASM `validate()` — checks signature, epoch rate limit, Basilica metadata
4. **50% validator approval** → submission stored in blockchain
5. **Validators** run WASM `evaluate()` — scores task results, applies LLM judge
6. **Agent code & logs** stored on-chain for auditability (code ≤ 1MB, logs ≤ 256KB)
7. **Log consensus** — validators propose logs, >50% hash agreement required
8. **Consensus** aggregates scores, applies decay, submits weights to Bittensor

### Key Concepts

- **WASM-only**: All challenge logic runs as a `wasm32-unknown-unknown` module loaded by platform-v2
- **Host functions**: WASM interacts with the outside world via `host_http_post()`, `host_storage_get()`, `host_storage_set()`, `host_consensus_get_epoch()`, etc.
- **SWE-bench datasets**: Tasks are selected from HuggingFace CortexLM/swe-bench via P2P consensus
- **Epoch rate limiting**: 1 submission per 3 epochs per miner
- **Top agent decay**: 72h grace period, then 50% daily decay to 0 weight

## Agent Code Storage

Agent submissions are stored on-chain for auditability and retrieval. The `agent_storage` module manages three storage categories:

| Storage Key Format | Content | Max Size |
|---|---|---|
| `agent_code:<hotkey>:<epoch>` | Raw zip package bytes | 1 MB (1,048,576 bytes) |
| `agent_hash:<hotkey>:<epoch>` | Hash of the agent package | — |
| `agent_logs:<hotkey>:<epoch>` | Serialized `AgentLogs` struct | 256 KB (262,144 bytes) |

- **Package size limit**: Submissions with `package_zip` exceeding 1 MB are rejected at the storage layer.
- **Log size limit**: Serialized logs exceeding 256 KB are rejected. Individual task output previews are truncated to 4 KB (4,096 bytes) before storage.
- **Key format**: Keys are constructed as `<prefix><hotkey_bytes>:<epoch_le_bytes>` using little-endian encoding for the epoch.

## CLI

The `term-cli` crate is a **native binary** (NOT `no_std`) that provides a terminal user interface for monitoring the term-challenge network.

### Design

- **Framework**: Built with [ratatui](https://ratatui.rs/) for TUI rendering
- **Transport**: Connects to validators via JSON-RPC 2.0 over HTTP
- **Target**: Standard `x86_64` / `aarch64` native targets (not WASM)

### Available Tabs

| Tab | Description |
|---|---|
| Leaderboard | Current scores, ranks, and miner hotkeys |
| Evaluation | Live evaluation progress for pending submissions |
| Submission | Recent submission history and status |
| Network | Validator count, epoch info, system health |

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch between tabs |
| `↑` / `↓` / `j` / `k` | Navigate rows |
| `r` | Refresh data |
| `q` / `Esc` | Quit |

### RPC Methods Used

- `epoch_current` — Current epoch number
- `challenge_call /leaderboard` — Leaderboard data
- `evaluation_getProgress` — Evaluation progress for a submission
- `agent_getLogs` — Validated evaluation logs
- `system_health` — Node health status
- `validator_count` — Number of active validators

## Build Commands

```bash
# Build CLI (native)
cargo build --release -p term-cli

# Build WASM module
cargo build --release --target wasm32-unknown-unknown -p term-challenge-wasm

# Check (no target needed for workspace check)
cargo check -p term-challenge-wasm
```

## Git Hooks

Git hooks live in `.githooks/` and are activated with `git config core.hooksPath .githooks`.

| Hook | What it does |
|------|-------------|
| `pre-commit` | Runs `cargo fmt --all`, stages formatted files. Skippable with `SKIP_GIT_HOOKS=1`. |
| `pre-push` | Full quality gate: format check → `cargo check` → `cargo clippy`. Skippable with `SKIP_GIT_HOOKS=1` or `git push --no-verify`. |

## CRITICAL RULES

1. **No `std` in WASM code.** The module compiles with `#![no_std]`. Use `alloc::` equivalents.
2. **Cryptographic signatures use sr25519.** SS58 prefix 42. Do NOT switch schemes.
3. **Conventional commits required.** The project uses `release-please`.
4. **No `.unwrap()` or `.expect()` in library paths.** Use pattern matching or `unwrap_or_default()`.
5. **Host functions are the ONLY external interface.** No direct HTTP, no filesystem, no std::net.
6. **Do NOT add `#[allow(dead_code)]` broadly.** Fix unused code or remove it.

> **Note:** The `cli/` crate is exempt from the `no_std` rule (rule 1) and the host-functions-only rule (rule 5) since it is a native binary that runs outside the WASM sandbox. Rules 2, 3, 4, and 6 still apply to CLI code.

## DO / DO NOT

### DO
- Use `alloc::string::String`, `alloc::vec::Vec`, `alloc::collections::BTreeMap` (WASM code)
- Use `serde` with `default-features = false, features = ["derive", "alloc"]` (WASM code)
- Use `bincode` with `default-features = false` for serialization (WASM code)
- Use host functions for all I/O: `host_storage_get/set`, `host_http_post`, `host_consensus_get_epoch` (WASM code)
- Keep the `register_challenge!` macro ABI contract intact
- Use standard `std` library features in the `cli/` crate (it is a native binary)

### DO NOT
- Do NOT use `std::`, `println!`, `std::collections::HashMap` in WASM code
- Do NOT add heavy dependencies — the WASM module must stay minimal
- Do NOT break the WASM ABI (evaluate, validate, get_name, get_version, get_tasks, configure, alloc)
- Do NOT store sensitive data in plain text in blockchain storage
