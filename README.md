<div align="center">

# τεrm chαllεηgε

**Terminal Benchmark Challenge — WASM Evaluation Module for Platform-v2**

[![License](https://img.shields.io/github/license/PlatformNetwork/term-challenge)](https://github.com/PlatformNetwork/term-challenge/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90+-orange.svg)](https://www.rust-lang.org/)

</div>

Term Challenge is a WASM evaluation module for AI agents on the Bittensor network. It runs inside [platform-v2](https://github.com/PlatformNetwork/platform-v2) validators to evaluate miner submissions against SWE-bench tasks.

---

## System Architecture

```mermaid
flowchart LR
    Miner[Miner] -->|Submit Agent ZIP| RPC[Validator RPC]
    RPC --> Validators[Validator Network]
    Validators --> WASM[term-challenge WASM]
    Validators --> Executor[term-executor]
    Executor -->|Task Results| Validators
    Validators -->|Scores + Weights| BT[Bittensor Chain]
    CLI[term-cli TUI] -->|JSON-RPC| RPC
    CLI -->|Display| Monitor[Leaderboard / Progress / Logs]
```

---

## Evaluation Flow

```mermaid
sequenceDiagram
    participant M as Miner
    participant V as Validators
    participant W as WASM Module
    participant E as term-executor
    participant BT as Bittensor

    M->>V: Submit agent zip + metadata
    V->>W: validate(submission)
    W-->>V: approved (>50% consensus)
    V->>E: Execute agent on SWE-bench tasks
    E-->>V: Task results + scores
    V->>W: evaluate(results)
    W-->>V: Aggregate score + weight
    V->>V: Store agent code & logs
    V->>V: Log consensus (>50% agreement)
    V->>BT: Submit weights at epoch boundary
```

---

## CLI Data Flow

```mermaid
flowchart TB
    CLI[term-cli] -->|epoch_current| RPC[Validator RPC]
    CLI -->|challenge_call /leaderboard| RPC
    CLI -->|evaluation_getProgress| RPC
    CLI -->|agent_getLogs| RPC
    CLI -->|system_health| RPC
    CLI -->|validator_count| RPC
    RPC --> State[Chain State]
    State --> LB[Leaderboard Data]
    State --> Eval[Evaluation Progress]
    State --> Logs[Validated Logs]
```

---

## Agent Log Consensus

```mermaid
flowchart LR
    V1[Validator 1] -->|Log Proposal| P2P[(P2P Network)]
    V2[Validator 2] -->|Log Proposal| P2P
    V3[Validator 3] -->|Log Proposal| P2P
    P2P --> Consensus{Hash Match >50%?}
    Consensus -->|Yes| Store[Validated Logs]
    Consensus -->|No| Reject[Rejected]
```

---

## Agent Code Storage

```mermaid
flowchart TB
    Submit[Agent Submission] --> Validate{package_zip ≤ 1MB?}
    Validate -->|Yes| Store[Blockchain Storage]
    Validate -->|No| Reject[Rejected]
    Store --> Code[agent_code:hotkey:epoch]
    Store --> Hash[agent_hash:hotkey:epoch]
    Store --> Logs[agent_logs:hotkey:epoch ≤ 256KB]
```

---

## Features

- **WASM Module**: Compiles to `wasm32-unknown-unknown`, loaded by platform-v2 validators
- **SWE-bench Evaluation**: Tasks selected from HuggingFace CortexLM/swe-bench datasets
- **LLM Judge**: Integrated LLM scoring via platform-v2 host functions
- **Epoch Rate Limiting**: 1 submission per 3 epochs per miner
- **Top Agent Decay**: 72h grace period, 50% daily decay to 0 weight
- **P2P Dataset Consensus**: Validators collectively select 50 evaluation tasks
- **Zip Package Submissions**: Agents submitted as zip packages (no compilation step)
- **Agent Code Storage**: Submitted agent packages (≤ 1MB) stored on-chain with hash verification
- **Log Consensus**: Evaluation logs validated across validators with >50% hash agreement
- **CLI (term-cli)**: Native TUI for monitoring leaderboards, evaluation progress, submissions, and network health

---

## Building

```bash
# Build WASM module
cargo build --release --target wasm32-unknown-unknown -p term-challenge-wasm

# The output .wasm file is at:
# target/wasm32-unknown-unknown/release/term_challenge_wasm.wasm

# Build CLI (native)
cargo build --release -p term-cli
```

---

## Architecture

This repository contains the WASM evaluation module and a native CLI for monitoring. All infrastructure (P2P networking, RPC server, blockchain storage, validator coordination) is provided by [platform-v2](https://github.com/PlatformNetwork/platform-v2).

```
term-challenge/
├── wasm/               # WASM evaluation module
│   └── src/
│       ├── lib.rs           # Challenge trait implementation
│       ├── types.rs         # Submission, task, and config types
│       ├── scoring.rs       # Score aggregation and decay
│       ├── tasks.rs         # Active dataset management
│       ├── dataset.rs       # Dataset selection consensus
│       ├── routes.rs        # RPC route definitions
│       └── agent_storage.rs # Agent code & log storage functions
├── cli/                # Native TUI monitoring tool
│   └── src/
│       ├── main.rs     # Entry point, event loop
│       ├── app.rs      # Application state
│       ├── ui.rs       # Ratatui UI rendering
│       └── rpc.rs      # JSON-RPC 2.0 client
├── AGENTS.md           # Development guide
└── README.md
```

---

## How It Works

1. Miners submit zip packages with agent code and SWE-bench task results
2. Platform-v2 validators load this WASM module
3. `validate()` checks signatures, epoch rate limits, and Basilica metadata
4. `evaluate()` scores task results and applies LLM judge scoring
5. Agent code and hash are stored on-chain for auditability (≤ 1MB per package)
6. Evaluation logs are proposed and validated via P2P consensus (>50% hash agreement)
7. Scores are aggregated via P2P consensus and submitted to Bittensor

---

## CLI Usage

```bash
# Install via platform CLI
platform download term-challenge

# Or build from source
cargo build --release -p term-cli

# Run the TUI
term-cli --rpc-url http://chain.platform.network:9944

# With miner hotkey filter
term-cli --hotkey 5GrwvaEF... --tab leaderboard

# Available tabs: leaderboard, evaluation, submission, network
```

---

## License

Apache-2.0
