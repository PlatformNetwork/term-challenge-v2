# AGENTS.md — src/worker/ (Background Workers)

## Purpose

Background workers that run as long-lived tasks in the server process. Handle compilation, evaluation queuing, plagiarism detection, LLM-based code review, timeout monitoring, and validator task assignment.

## Module Structure

| File | Purpose |
|------|---------|
| `compile.rs` | Compilation worker — compiles Python agents to PyInstaller binaries in Docker |
| `queue.rs` | `AgentQueue` — manages evaluation queue with priority and concurrency control |
| `plagiarism.rs` | Plagiarism detection — compares agent code using AST analysis (`rustpython-parser`) |
| `llm_review.rs` | `LlmReviewWorker` — automated LLM-based code review for rule compliance |
| `timeout_monitor.rs` | `TimeoutRetryMonitor` — monitors stuck evaluations and retries them |
| `assignment_monitor.rs` | Monitors validator task assignments and reassigns on failure |

## Key Patterns

- Workers are spawned with `tokio::spawn` and run indefinitely
- Each worker has a config struct and a `spawn_*` function
- Workers communicate via shared state (`Arc<...>`) and channels
- All workers must handle errors gracefully — a single failure should not crash the server