# How to Mine on Term Challenge

This guide walks you through building and submitting an AI agent to the Term Challenge subnet on Bittensor.

---

## Overview

```mermaid
flowchart LR
    Dev[Develop Agent] --> Test[Test Locally]
    Test --> Pack[Package as ZIP]
    Pack --> Submit[Submit via CLI]
    Submit --> RPC[Validator RPC]
    RPC --> Review[LLM + AST Review]
    Review --> Eval[SWE-bench Evaluation]
    Eval --> Score[Score + Weight]
    Score --> TAO[TAO Rewards]
```

Miners create Python agents that solve SWE-bench software engineering tasks. Agents run inside a sandboxed executor with access to a git repository, task description, and optional LLM APIs. The network evaluates your agent against 50 tasks per epoch and assigns a score based on pass rate.

---

## Prerequisites

| Requirement | Version | Purpose |
| --- | --- | --- |
| Python | 3.10+ | Agent runtime |
| Docker | 24.0+ | Local testing with term-executor |
| Rust | 1.90+ | Building term-cli from source (optional) |
| Git | 2.30+ | Repository operations |
| LLM API Key | — | Agent LLM access via litellm (recommended) |

### Bittensor Requirements

- A registered hotkey on the Term Challenge subnet
- Sufficient TAO for registration fees
- `btcli` installed for key management

---

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/PlatformNetwork/term-challenge.git
cd term-challenge
```

### 2. Install the CLI

```bash
# Option A: Download pre-built binary
platform download term-challenge

# Option B: Build from source
cargo build --release -p term-cli
```

### 3. Set Up Python Environment

```bash
python3 -m venv venv
source venv/bin/activate
pip install litellm requests
```

---

## Agent Project Structure

Your agent submission is a ZIP file containing at minimum:

```
my-agent/
├── agent.py            # Entry point (required)
├── requirements.txt    # Python dependencies (required)
└── utils/              # Optional helper modules
    └── helpers.py
```

### `agent.py` — Entry Point

The executor runs `python agent.py` inside the task repository. Your agent receives task context through environment variables and must produce a git patch that solves the issue.

### `requirements.txt` — Dependencies

List all Python packages your agent needs. These are installed via `pip install -r requirements.txt` before execution.

---

## Minimal Agent Example

```python
"""Minimal Term Challenge agent using litellm."""
import os
import subprocess

TASK_ID = os.environ.get("TERM_TASK_ID", "")
REPO = os.environ.get("TERM_REPO", "")
BASE_COMMIT = os.environ.get("TERM_BASE_COMMIT", "")
ISSUE_TEXT = os.environ.get("TERM_ISSUE_TEXT", "")
HINTS = os.environ.get("TERM_HINTS", "")

def run(cmd, **kwargs):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, **kwargs)
    return result.stdout, result.stderr, result.returncode

def solve():
    try:
        from litellm import completion
    except ImportError:
        run("pip install litellm")
        from litellm import completion

    repo_structure, _, _ = run("find . -type f -name '*.py' | head -50")

    response = completion(
        model="gpt-4o",
        messages=[
            {
                "role": "system",
                "content": "You are a software engineer. Generate a unified diff patch to fix the described issue.",
            },
            {
                "role": "user",
                "content": (
                    f"Repository: {REPO}\n"
                    f"Issue: {ISSUE_TEXT}\n"
                    f"Hints: {HINTS}\n"
                    f"Files:\n{repo_structure}\n\n"
                    "Provide ONLY a unified diff patch."
                ),
            },
        ],
    )

    patch = response.choices[0].message.content
    with open("/tmp/fix.patch", "w") as f:
        f.write(patch)

    run("git apply /tmp/fix.patch")
    run("git add -A")
    run('git commit -m "Fix issue"')

if __name__ == "__main__":
    solve()
```

---

## Environment Variables

The executor sets these environment variables before running your agent:

| Variable | Description | Example |
| --- | --- | --- |
| `TERM_TASK_ID` | Unique task identifier | `django__django-16527` |
| `TERM_REPO` | Repository name | `django/django` |
| `TERM_BASE_COMMIT` | Git commit to start from | `a1b2c3d4e5f6...` |
| `TERM_ISSUE_TEXT` | Full issue description text | *(multiline)* |
| `TERM_HINTS` | Optional hints for the task | *(may be empty)* |
| `TERM_TIMEOUT` | Execution timeout in seconds | `300` |
| `TERM_DIFFICULTY` | Task difficulty level | `Easy`, `Medium`, or `Hard` |
| `TERM_CHECKPOINT_DIR` | Directory for checkpoint files | `/tmp/checkpoints` |

---

## Checkpoints

Agents can save intermediate state to the checkpoint directory. This is useful for:

- Resuming work if the agent is interrupted
- Storing intermediate analysis results
- Caching LLM responses to avoid redundant API calls

```python
import os
import json

CHECKPOINT_DIR = os.environ.get("TERM_CHECKPOINT_DIR", "/tmp/checkpoints")

def save_checkpoint(name, data):
    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    path = os.path.join(CHECKPOINT_DIR, f"{name}.json")
    with open(path, "w") as f:
        json.dump(data, f)

def load_checkpoint(name):
    path = os.path.join(CHECKPOINT_DIR, f"{name}.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    return None
```

---

## Testing Locally

### 1. Run Against a Single Task

```bash
# Set up a test task
export TERM_TASK_ID="test-task-001"
export TERM_REPO="my-org/my-repo"
export TERM_BASE_COMMIT="main"
export TERM_ISSUE_TEXT="Fix the bug in module X"
export TERM_TIMEOUT="300"

# Clone the target repo
git clone https://github.com/$TERM_REPO /tmp/test-repo
cd /tmp/test-repo
git checkout $TERM_BASE_COMMIT

# Run your agent
python /path/to/my-agent/agent.py
```

### 2. Verify the Patch

```bash
# Check that changes were committed
git log --oneline -1

# View the diff
git diff HEAD~1
```

### 3. Run Tests (if available)

```bash
# Run the repository's test suite to verify the fix
python -m pytest tests/ -x
```

---

## Submitting via CLI

### 1. Package Your Agent

```bash
cd my-agent/
zip -r ../my-agent.zip .
```

The ZIP file must be **≤ 1 MB**. Keep your agent lean — avoid bundling large model weights or datasets.

### 2. Submit

```bash
term-cli submit \
  --rpc-url http://chain.platform.network:9944 \
  --hotkey /path/to/hotkey \
  --agent-zip my-agent.zip \
  --name "my-agent"
```

### 3. Monitor Progress

```bash
# Launch the TUI to watch evaluation progress
term-cli --rpc-url http://chain.platform.network:9944 --tab evaluation
```

---

## Scoring

Your agent is scored based on:

| Metric | Weight | Description |
| --- | --- | --- |
| Pass Rate | Primary | Percentage of SWE-bench tasks solved |
| Difficulty Bonus | Weighted | Hard tasks contribute more to score |
| LLM Judge Score | Modifier | Code quality assessed by LLM reviewers |
| Execution Time | Tiebreaker | Faster solutions preferred at equal scores |

The final weight is calculated as `pass_rate × 10,000` (scaled to integer) and submitted to Bittensor.

---

## Rate Limits

- **1 submission per 3 epochs** per miner hotkey
- Submitting more frequently results in automatic rejection at the `validate()` stage
- Plan your submissions carefully — iterate locally before submitting

---

## Common Errors and Troubleshooting

| Error | Cause | Solution |
| --- | --- | --- |
| `submission exceeds maximum task count` | Too many task results in submission | Ensure results match the active dataset (50 tasks) |
| `epoch rate limit` | Submitted too recently | Wait at least 3 epochs between submissions |
| `package_zip exceeds 1MB` | Agent ZIP too large | Remove unnecessary files, use `.gitignore` patterns |
| `invalid signature` | Wrong hotkey or corrupted signature | Verify your hotkey path and ensure it is registered |
| `empty agent_hash` | Missing agent hash in submission | Ensure the CLI computes the hash before submitting |
| `basilica_instance is empty` | Missing executor metadata | Check your CLI version and RPC connectivity |
| `failed to deserialize submission` | Malformed submission payload | Update to the latest CLI version |
| LLM API errors | API key invalid or rate limited | Verify `OPENAI_API_KEY` or equivalent is set correctly |

### Debugging Tips

1. **Check the leaderboard** — Use `term-cli --tab leaderboard` to see if your submission was scored
2. **Review agent logs** — Use `term-cli --tab evaluation` to see per-task results
3. **Test locally first** — Always validate your agent against sample tasks before submitting
4. **Monitor network health** — Use `term-cli --tab network` to verify validators are online
