# AGENTS.md — src/bench/ (Benchmarking Framework)

## Purpose

Provides local benchmarking for miners to test their agents against Terminal-Bench 2.0 tasks. Handles dataset downloading, Docker environment setup, agent execution (Python scripts, compiled binaries, in-container), result verification, and result export.

## Module Structure

| File | Purpose |
|------|---------|
| `agent.rs` | Built-in LLM agent for `bench run` |
| `binary_agent.rs` | Runs pre-compiled PyInstaller binary agents |
| `external_agent.rs` | Runs external Python agent scripts |
| `in_container_agent.rs` | Runs agents inside Docker containers |
| `environment.rs` | `DockerEnvironment` — manages Docker containers for tasks |
| `llm.rs` | LLM client for benchmarking (OpenRouter, Chutes, etc.) |
| `registry.rs` | Dataset registry client — downloads from GitHub releases |
| `runner.rs` | `TrialRunner` — orchestrates single task trials |
| `session.rs` | `TmuxSession` — manages tmux sessions for agent I/O |
| `task.rs` | Task loading and configuration |
| `verifier.rs` | Runs test scripts and checks `/logs/verifier/reward.txt` |
| `results.rs` | `BenchmarkResults` — aggregates and exports results |

## Key Types

- `ExternalAgent` — wraps a Python agent script for execution
- `DockerEnvironment` — creates/manages Docker containers per task
- `Verifier` — runs `tests/test.sh` inside the container and reads reward
- `TrialRunner` — full trial lifecycle: setup → execute → verify → cleanup
