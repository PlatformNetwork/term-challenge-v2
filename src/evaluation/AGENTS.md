# AGENTS.md — src/evaluation/ (Evaluation Pipeline)

## Purpose

Implements the full agent evaluation pipeline: from receiving an agent submission through task execution to result aggregation. Used by both server (orchestration) and validator (execution) modes.

## Module Structure

| File | Purpose |
|------|---------|
| `evaluator.rs` | `TaskEvaluator` — evaluates a single agent on a single task |
| `orchestrator.rs` | `EvaluationOrchestrator` — coordinates multi-task evaluation across validators |
| `pipeline.rs` | `EvaluationPipeline` — end-to-end pipeline: receive → compile → distribute → evaluate |
| `progress.rs` | `EvaluationProgress`, `ProgressStore` — tracks evaluation state and task results |

## Key Types

- `TaskEvaluator` — runs an agent binary in a container, monitors execution, collects results
- `EvaluationOrchestrator` — splits tasks across validators, aggregates results
- `EvaluationPipeline` — full lifecycle from submission to scored result
- `TaskExecutionState` — tracks per-task state (pending, running, completed, failed)
