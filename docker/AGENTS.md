# AGENTS.md — docker/ (Docker Build Files)

## Purpose

Contains Dockerfiles and scripts for building container images used in the system.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile.base` | Base image for agent execution containers |
| `Dockerfile.compiler` | Image used to compile Python agents to PyInstaller binaries |
| `agent_runner.py` | Python script that runs inside agent containers — manages agent lifecycle, HTTP server, command execution |

## Root-Level Dockerfiles

| File | Purpose |
|------|---------|
| `/Dockerfile` | Main multi-stage build: builds `term` and `term-server` binaries with cargo-chef caching, packages with Python/litellm |
| `/Dockerfile.agent` | Builds agent execution environment |
| `/Dockerfile.server` | Server-specific build variant |

## Build

```bash
# Build main image
docker build -t term-challenge .

# Build with custom repo path (for platform integration)
docker build --build-arg TERM_REPO_PATH=. -t term-challenge .
```
