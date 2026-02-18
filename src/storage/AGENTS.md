# AGENTS.md — src/storage/ (Data Persistence)

## Purpose

Provides data persistence for both server mode (PostgreSQL) and validator mode (SQLite). Also includes chain storage for on-chain data and a migration runner.

## Module Structure

| File | Purpose |
|------|---------|
| `pg.rs` | `PgStorage` — PostgreSQL storage (submissions, evaluations, leaderboard, LLM usage) |
| `local.rs` | SQLite storage for validators (local evaluation results) |
| `chain.rs` | `ChainStorage` — reads/writes on-chain evaluation data |
| `migrations.rs` | Migration runner — applies SQL files from `migrations/` directory |
| `traits.rs` | Storage trait definitions |
| `postgres/` | PostgreSQL query implementations split by domain |

## PostgreSQL Schema

Migrations are in `/migrations/` (001–038). Key tables:
- `submissions` — agent submissions with status tracking
- `evaluations` — evaluation results per agent per validator
- `task_logs` — per-task execution logs
- `validators` — validator registration and readiness
- `llm_usage` — LLM API cost tracking
- `swe_forge_evaluations` — SWE-Forge evaluation results from term-executor workers

## Rules

- **Migrations are append-only.** Never modify existing migration files.
- New migrations use the next sequential number: `039_description.sql`.
- Use `tokio-postgres` for async queries (not `diesel` or `sqlx`).
