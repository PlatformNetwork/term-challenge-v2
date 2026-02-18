# AGENTS.md — Term Challenge

## Project Purpose

Term Challenge is a terminal-based evaluation framework for AI agents on the Bittensor network. Miners submit Python agents that solve command-line tasks inside Docker containers; validators evaluate them across distributed nodes and produce consensus scores that determine miner weights and TAO emissions. The system is written in Rust (~69k lines) with a Python SDK and agent runner.

## Architecture Overview

```
term-challenge/
├── bin/
│   └── term/main.rs         # term — CLI for miners (submit, bench, status, leaderboard)
├── src/
│   ├── lib.rs               # Crate root — module declarations and re-exports
│   ├── core/                # Fundamental types: Hotkey, ChallengeId, TaskResult
│   ├── crypto/              # sr25519 auth, x25519 encryption, SS58, API key handling
│   ├── util/                # Timestamp, hashing (SHA-256, Blake2), encoding helpers
│   ├── storage/             # Persistence: PostgreSQL, SQLite (validator), chain
│   ├── cache/               # In-memory caches: metagraph, task stream
│   ├── client/              # HTTP client, WebSocket (platform & validator), LLM proxy
│   ├── chain/               # Bittensor integration: block sync, epoch calc, on-chain eval
│   ├── weights/             # Weight calculation: scoring, decay, emission, distribution
│   ├── evaluation/          # Eval pipeline: evaluator, orchestrator, progress tracking
│   ├── validation/          # Code validation: Python whitelist, package checks, visibility
│   ├── worker/              # Background workers: compile, queue, plagiarism, LLM review
│   ├── swe_forge/           # SWE-Forge integration: term-executor client, result types
│   ├── task/                # Task types, registry, harness, challenge definitions
│   ├── agent/               # Agent management: registry, submission, review
│   └── admin/               # Sudo/admin controls, subnet config, challenge config
├── wasm/                    # WASM challenge evaluation module (no_std)
├── migrations/              # PostgreSQL schema migrations (001–038)
├── data/tasks/              # Built-in task definitions (hello-world, etc.)
├── checkpoints/             # Checkpoint JSON files for evaluation datasets
├── tests/                   # Rust integration tests + Python integration tests
├── examples/                # Example agents (validator_agent)
├── scripts/                 # Build scripts (WASM, hooks) and multi-agent review (Python)
└── docs/                    # Documentation (miner, validator, reference, architecture)
```

### Data Flow

1. **Miner** writes a Python agent and submits via `term wizard` CLI
2. **Validators** receive agent submissions, dispatch evaluation batches to **term-executor** workers via **Basilica** for SWE-Forge evaluation
3. **term-executor** workers run agents against SWE-Forge tasks and return results through Basilica
4. **WASM module** (`wasm/`) scores task results deterministically, optionally invoking an LLM judge
5. Consensus scores determine miner weights and TAO emissions on the Bittensor chain

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust 1.90+ (edition 2021) |
| Async Runtime | Tokio (full features) |
| CLI Framework | Clap 4.5 (derive) |
| Database (server) | PostgreSQL via `tokio-postgres` + `deadpool-postgres` |
| Database (validator) | SQLite via `rusqlite` (bundled) |
| Crypto | `sp-core` (sr25519), `schnorrkel`, `x25519-dalek`, `chacha20poly1305` |
| Serialization | serde + serde_json + serde_yaml + toml |
| Agent Language | Python 3.10+ |
| Agent SDK | `term_sdk` (Python) / litellm (SDK 3.0) |
| WASM Evaluation | `platform-challenge-sdk-wasm` (no_std) |

## Build & Test Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run tests (skip live/integration tests that need external services)
cargo test --workspace -- --skip live --skip integration

# Run tests with nextest (CI uses this)
cargo nextest run --workspace -E 'not (test(/live/) | test(/integration/))'

# Format code
cargo fmt --all

# Format check (CI)
cargo fmt --check

# Lint
cargo clippy --all-targets --workspace -- -W clippy::all \
  -A clippy::too_many_arguments \
  -A clippy::type_complexity \
  -A clippy::large_enum_variant \
  -A clippy::should_implement_trait

# Run the CLI
cargo run --bin term -- --help

# Install Python SDK (for agent development)
pip install -e sdk/python  # if sdk/python exists
pip install git+https://github.com/PlatformNetwork/term-challenge.git#subdirectory=sdk/python
```

## Git Hooks

Git hooks live in `.githooks/` and are activated with `git config core.hooksPath .githooks`.

| Hook | What it does |
|------|-------------|
| `pre-commit` | Runs `cargo fmt --all`, stages formatted files. Skippable with `SKIP_GIT_HOOKS=1`. |
| `pre-push` | Full quality gate: format check → `cargo check` → `cargo clippy` → `cargo test` (skipping live/integration). Skippable with `SKIP_GIT_HOOKS=1` or `git push --no-verify`. |

To install hooks: `bash .githooks/install.sh` or `git config core.hooksPath .githooks`.

## CRITICAL RULES

1. **Never hardcode secrets or API keys.** All credentials (hotkeys, API keys, database URLs) must come from environment variables. The codebase uses `clap(env = "...")` for CLI args and `std::env::var()` for runtime config. Agents that hardcode secrets will be rejected by the validation pipeline (`src/validation/`).

2. **All async code must use Tokio.** The entire crate uses `tokio` with full features. Do NOT introduce alternative async runtimes (async-std, smol). All `#[tokio::main]` and `#[tokio::test]` annotations must remain consistent.

3. **SWE-Forge evaluations run on term-executor workers.** Agents are evaluated by term-executor workers coordinated through Basilica. The `src/swe_forge/` module handles communication with these workers.

4. **Cryptographic signatures use sr25519 (Substrate/Bittensor standard).** Authentication uses `sp-core` and `schnorrkel` for sr25519 signing/verification. SS58 encoding uses prefix 42. Do NOT switch to ed25519 or secp256k1 — the Bittensor chain requires sr25519.

5. **PostgreSQL migrations are append-only.** The `migrations/` directory contains numbered SQL files (001–038). Never modify existing migrations. Always add new migrations with the next sequential number. The migration runner in `src/storage/migrations.rs` applies them in order.

6. **Clippy must pass with the project's specific allow-list.** CI runs clippy with `-W clippy::all` plus these allowed lints: `too_many_arguments`, `type_complexity`, `large_enum_variant`, `should_implement_trait`. Do not add new global allows without justification.

7. **Error handling uses `anyhow` for binaries and `thiserror` for library code.** Binary crates (`bin/term/`) return `anyhow::Result`. Library modules in `src/` define typed errors with `thiserror::Error` derive. Do not use `unwrap()` or `expect()` in library code paths that handle user input or network data.

8. **Conventional commits are required.** The project uses `release-please` for automated releases. All commits must follow the conventional commits format (`feat:`, `fix:`, `chore:`, `docs:`, `perf:`, `refactor:`, `ci:`, `test:`). Breaking changes use `feat!:` or `fix!:` or a `BREAKING CHANGE:` footer.

## DO / DO NOT

### DO

- Use `tracing::info!`, `tracing::debug!`, `tracing::error!` for logging (not `println!` in library code)
- Add tests for new functionality; run `cargo test --workspace -- --skip live --skip integration` before pushing
- Use `serde` derive macros for all serializable types
- Follow the existing module structure: add new modules under the appropriate thematic directory in `src/`
- Use `clap` derive macros for any new CLI arguments
- Use `parking_lot::Mutex`/`RwLock` over `std::sync::Mutex` (the project already uses `parking_lot`)
- Keep re-exports in `src/lib.rs` updated when adding public types

### DO NOT

- Do NOT add new direct dependencies without checking if an existing dep already covers the use case
- Do NOT use `tokio::spawn` without proper error handling — spawned tasks must log errors
- Do NOT modify the agent protocol endpoints (`/health`, `/start`, `/status`) without updating validators AND the SDK
- Do NOT use `std::thread` for concurrent work — use `tokio::spawn` or `tokio::task::spawn_blocking`
- Do NOT store sensitive data in logs — the system handles hotkeys, API keys, and agent source code
- Do NOT break the `term_sdk` Python API contract — miners depend on `AgentContext`, `ShellResult`, `LLM`
- Do NOT change SS58 prefix (42) or signature scheme (sr25519) — these are Bittensor chain requirements
- Do NOT add `#[allow(dead_code)]` broadly — fix unused code or remove it
