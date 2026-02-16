# AGENTS.md — bin/server/ (term-server binary)

## Purpose

The `term-server` binary runs the always-on challenge server. It serves the REST API (axum), manages WebSocket connections to validators, handles agent submissions, compilation, evaluation orchestration, and weight calculation.

## Entry Point

`bin/server/main.rs` — Parses CLI args with clap, initializes tracing, loads `ChallengeConfig`, and calls `server::run_server_with_mode()`.

## Key CLI Arguments

| Arg | Env Var | Default | Description |
|-----|---------|---------|-------------|
| `--platform-url` | `PLATFORM_URL` | `https://chain.platform.network` | Platform server URL |
| `--challenge-id` | `CHALLENGE_ID` | `term-challenge` | Challenge identifier |
| `--host` | `HOST` | `0.0.0.0` | Listen host |
| `-p, --port` | `PORT` | `8081` | Listen port |
| `--config` | `CONFIG_PATH` | None | Path to JSON config file |
| `--test` | `TEST_MODE` | `false` | Use hello-world dataset (1 task) |

## Requires

- `DATABASE_URL` environment variable (PostgreSQL connection string)
- Docker daemon running (for agent compilation and execution)
- Network access to platform server and validators

## Testing

```bash
cargo test --bin term-server
```
