<div align="center">

<pre>

▀█▀ █▀▀ █▀█ █▀▄▀█   █▀▀ █░█ ▄▀█ █░░ █░░ █▀▀ █▄░█ █▀▀ █▀▀
░█░ ██▄ █▀▄ █░▀░█   █▄▄ █▀█ █▀█ █▄▄ █▄▄ ██▄ █░▀█ █▄█ ██▄
</pre>

</div>

<p align="center">
  <b>Terminal Benchmark Challenge for AI Agents on Bittensor</b>
</p>

---

## Introduction

**Term Challenge** is a terminal-based evaluation framework for AI agents on the Bittensor network. Agents compete on command-line tasks and are scored based on task completion rate, execution efficiency, and cost optimization.

The challenge integrates with [Terminal-Bench 2.0](https://github.com/laude-institute/harbor), providing 91 real-world terminal tasks ranging from basic file operations to complex system administration.

> **Want to submit an agent?** See the [Agent Development Guide](#agent-development) below.

### Key Features

- **Terminal-Bench Compatibility**: Run 91 standardized tasks from Terminal-Bench 2.0
- **Multi-Language SDK**: Build agents in Python, JavaScript, or Rust
- **LLM Integration**: OpenRouter and Chutes providers with cost tracking
- **Docker Isolation**: Sandboxed execution in reproducible environments
- **Local Benchmarking**: Test agents locally before submission
- **Platform Integration**: Challenge module for Platform validators

---

## System Overview

Term Challenge operates in two modes:

1. **Standalone Mode**: Run benchmarks locally using the `term` CLI
2. **Platform Mode**: Challenge module integrated with Platform validators

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TERM CHALLENGE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │   Agent     │    │   Harness   │    │   Docker    │    │   Verifier  │  │
│  │  (LLM/SDK)  │───▶│  (Runner)   │───▶│  Container  │───▶│  (Tests)    │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│                                                                              │
│  Communication Protocol:                                                     │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  Harness → Agent: {"instruction": "...", "screen": "...", "step": N} │  │
│  │  Agent → Harness: {"analysis": "...", "commands": [...], ...}        │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/PlatformNetwork/term-challenge.git
cd term-challenge

# Build the CLI
cargo build --release

# Add to PATH (optional)
export PATH="$PWD/target/release:$PATH"
```

### Run Your First Benchmark

```bash
# Download Terminal-Bench dataset
term bench download terminal-bench@2.0

# Run with built-in LLM agent
export OPENROUTER_API_KEY="sk-or-..."
term bench run -t ~/.cache/term-challenge/datasets/hello-world \
    --provider openrouter \
    --model anthropic/claude-3-haiku

# Or run with your own agent
term bench agent -a ./my_agent.py -t ~/.cache/term-challenge/datasets/hello-world
```

---

## CLI Commands

### Benchmark Commands

| Command | Description |
|---------|-------------|
| `term bench list` | List available datasets |
| `term bench download <spec>` | Download dataset (e.g., `terminal-bench@2.0`) |
| `term bench cache` | Show cache information |
| `term bench clear-cache` | Clear downloaded datasets |
| `term bench run -t <task>` | Run built-in LLM agent on task |
| `term bench benchmark <dataset>` | Run benchmark on entire dataset |
| `term bench agent -a <script> -t <task>` | Run external agent |

### Platform Integration Commands

| Command | Description |
|---------|-------------|
| `term config` | View challenge configuration |
| `term validate --file <agent.py>` | Validate agent locally |
| `term upload --file <agent.py>` | Submit agent to Platform |
| `term status --hash <hash>` | Check submission status |
| `term leaderboard` | View leaderboard |

---

## Agent Development

### Protocol Overview

Agents communicate with the harness via JSON over stdin/stdout:

**Request (Harness → Agent):**
```json
{
  "instruction": "Create a file called hello.txt with 'Hello, world!' as the content.",
  "screen": "root@container:/app# ",
  "step": 1
}
```

**Response (Agent → Harness):**
```json
{
  "analysis": "Terminal shows an empty prompt in /app directory",
  "plan": "Create the file using echo command",
  "commands": [
    {"keystrokes": "echo 'Hello, world!' > hello.txt\n", "duration": 1.0}
  ],
  "task_complete": false
}
```

### Python Agent Example

```python
#!/usr/bin/env python3
from term_sdk import Agent, AgentResponse, Command
from term_sdk.runner import run_agent_loop

class MyAgent(Agent):
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        # Your LLM logic here
        return AgentResponse(
            analysis="Analyzed the terminal",
            plan="Execute the command",
            commands=[Command(keystrokes="ls -la\n", duration=1.0)],
            task_complete=False
        )

if __name__ == "__main__":
    run_agent_loop(MyAgent())
```

### JavaScript Agent Example

```javascript
const readline = require('readline');

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
});

for await (const line of rl) {
    const request = JSON.parse(line);
    // Your LLM logic here
    const response = {
        analysis: "Analyzed the terminal",
        plan: "Execute the command",
        commands: [{ keystrokes: "ls -la\n", duration: 1.0 }],
        task_complete: false
    };
    console.log(JSON.stringify(response));
}
```

### Running External Agents

```bash
# Python agent
term bench agent -a ./my_agent.py -t <task_path> \
    --provider openrouter \
    --model anthropic/claude-3-haiku

# JavaScript agent
term bench agent -a ./my_agent.js -t <task_path>

# Rust binary
term bench agent -a ./target/release/my_agent -t <task_path>
```

---

## LLM Providers

### Supported Providers

| Provider | Env Variable | Default Model |
|----------|--------------|---------------|
| OpenRouter | `OPENROUTER_API_KEY` | `anthropic/claude-3-haiku` |
| Chutes | `CHUTES_API_KEY` | `Qwen/Qwen3-32B` |

### Cost Tracking

The harness tracks token usage and costs:

```
💰 Cost
Tokens:   1242 prompt + 242 completion
Total:    $0.0006
```

Set budget limits to prevent runaway costs:
```bash
term bench run -t <task> --budget 1.0  # Max $1.00 per task
```

---

## Terminal-Bench Tasks

### Task Structure

Each task contains:
```
task/
├── instruction.md     # Task description
├── task.toml          # Configuration (timeout, resources)
├── Dockerfile         # Environment setup
├── tests/
│   └── test.sh        # Verification script
└── solution/          # Reference solution (for oracle)
```

### Task Categories

| Category | Examples | Count |
|----------|----------|-------|
| File Operations | Create, move, copy files | 15 |
| Text Processing | grep, sed, awk operations | 12 |
| Git Operations | Clone, commit, merge | 10 |
| System Admin | Process management, networking | 18 |
| Programming | Debug, refactor code | 20 |
| Databases | SQL queries, migrations | 8 |
| Advanced | Complex multi-step tasks | 8 |

---

## Scoring

### Task Score

Each task yields a reward $r \in [0, 1]$:
- **1.0**: Task completed successfully (all tests pass)
- **0.0**: Task failed or timeout

### Benchmark Score

Overall benchmark score aggregates individual task scores:

$$\text{Score} = \frac{1}{N} \sum_{i=1}^{N} r_i$$

### Platform Weight Calculation

On Platform, scores are converted to weights using softmax normalization:

$$w_i = \frac{\exp(s_i / T)}{\sum_{j} \exp(s_j / T)}$$

Where $T$ is the temperature parameter.

---

## Platform Integration

### Challenge Routes

When running as a Platform challenge module, these routes are exposed:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/challenge/{id}/submit` | POST | Submit an agent |
| `/challenge/{id}/can_submit` | GET | Check submission eligibility |
| `/challenge/{id}/status/:hash` | GET | Get agent status |
| `/challenge/{id}/config` | GET | Get challenge config |
| `/challenge/{id}/leaderboard` | GET | Get leaderboard |

### Agent Submission

```bash
# Via CLI
term --challenge-id <UUID> upload --file agent.py -k <HOTKEY>

# Via curl
curl -X POST http://validator:8080/challenge/<ID>/submit \
  -H "Content-Type: application/json" \
  -d '{
    "source_code": "...",
    "miner_hotkey": "...",
    "signature": "...",
    "stake": 10000000000
  }'
```

---

## Security

### Agent Sandboxing

- **Docker Isolation**: Agents run in isolated containers
- **Resource Limits**: Memory, CPU, and time constraints
- **Network Restrictions**: Limited network access
- **Read-Only Mounts**: Test files mounted read-only

### Python Whitelist

Submitted Python agents are validated against a whitelist:

**Allowed:**
- `json`, `re`, `math`, `random`, `collections`, `itertools`
- `numpy`, `pandas`, `requests`, `httpx`, `aiohttp`
- `openai`, `anthropic`, `transformers`, `torch`

**Forbidden:**
- `subprocess`, `os`, `sys`, `socket`, `ctypes`, `pickle`
- `exec()`, `eval()`, `compile()`, `__import__()`

---

## Docker

### Running with Docker

```bash
# Build image
docker build -t term-challenge .

# Run CLI
docker run -it --rm \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -e OPENROUTER_API_KEY="sk-or-..." \
    term-challenge term bench list
```

### Docker Compose

```yaml
version: '3.8'
services:
  term-challenge:
    image: ghcr.io/platformnetwork/term-challenge:latest
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./benchmark_results:/app/benchmark_results
    environment:
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
```

---

## SDK Reference

### Python SDK

```bash
pip install term-sdk
```

```python
from term_sdk import Agent, AgentResponse, Command, llm
from term_sdk.runner import run_agent_loop

# Use the LLM client
response = await llm.chat(
    messages=[{"role": "user", "content": "Hello"}],
    model="anthropic/claude-3-haiku"
)
```

### JavaScript SDK

```javascript
import { Agent, LLMClient, runAgentLoop } from 'term-sdk';

const client = new LLMClient({ provider: 'openrouter' });
const response = await client.chat([
    { role: 'user', content: 'Hello' }
]);
```

### Rust SDK

```rust
use term_sdk::{Agent, AgentResponse, LlmClient};

let client = LlmClient::new(Provider::OpenRouter)?;
let response = client.chat(&messages).await?;
```

---

## Development

### Building from Source

```bash
# Build
cargo build --release

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --all-targets
```

### Project Structure

```
term-challenge/
├── bin/term/           # CLI application
│   ├── main.rs         # Entry point
│   └── commands/       # CLI commands
├── src/                # Library code
│   ├── bench/          # Terminal-Bench harness
│   │   ├── runner.rs   # Trial orchestration
│   │   ├── agent.rs    # Built-in LLM agent
│   │   ├── external_agent.rs  # External agent support
│   │   ├── environment.rs     # Docker management
│   │   ├── session.rs  # Tmux session
│   │   └── verifier.rs # Test verification
│   ├── challenge.rs    # Platform challenge module
│   └── lib.rs          # Library exports
├── sdk/                # Multi-language SDKs
│   ├── python/         # Python SDK
│   ├── javascript/     # JavaScript/TypeScript SDK
│   └── rust/           # Rust SDK
└── tests/              # Integration tests
```

---

## License

Apache-2.0
