# AGENTS.md — src/task/ (Task Definitions)

## Purpose

Defines the task model for Terminal-Bench challenges. Tasks are terminal-based problems that agents solve inside Docker containers. Each task has an instruction, a Docker image, and a verification script.

## Module Structure

| File | Purpose |
|------|---------|
| `types.rs` | Core types: `Task`, `TaskConfig`, `TaskDescription`, `TaskResult`, `Difficulty` |
| `config.rs` | Task configuration loading and defaults |
| `registry.rs` | `TaskRegistry` — loads and manages task collections |
| `challenge.rs` | `TerminalBenchChallenge` — creates challenge instances from datasets |
| `harness.rs` | Task execution harness — sets up container, runs agent, verifies results |

## Task Format

```
task-directory/
├── task.yaml          # Task metadata (instruction, difficulty, timeouts, docker config)
├── Dockerfile         # Optional custom container image
├── setup.sh           # Optional setup script
└── tests/
    └── test.sh        # Verification script — writes 1 or 0 to /logs/verifier/reward.txt
```

## Key Types

- `Task` — full task definition with instruction, config, and test paths
- `TaskResult` — pass (1.0) or fail (0.0) with execution metadata
- `Difficulty` — `easy`, `medium`, `hard`
