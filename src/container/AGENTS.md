# AGENTS.md — src/container/ (Docker Management)

## Purpose

Abstracts Docker container operations. Provides a backend trait (`ContainerBackend`) with implementations for standard Docker (via Bollard) and a secure container runtime (from platform). Also handles Python agent compilation to PyInstaller binaries.

## Module Structure

| File | Purpose |
|------|---------|
| `backend.rs` | `ContainerBackend` trait + implementations: `DockerBackend`, `SecureBrokerBackend`, `WsBrokerBackend` |
| `docker.rs` | `DockerExecutor` — low-level Docker operations via Bollard |
| `compiler.rs` | Compiles Python agents to standalone binaries using PyInstaller in Docker |

## Key Types

- `ContainerBackend` — trait for container operations (create, exec, destroy)
- `ContainerHandle` — handle to a running container
- `SandboxConfig` — security settings (memory limit, CPU, network mode)
- `MountConfig` — volume mount configuration
- `DockerConfig` — Docker connection and image settings

## Security

- Containers have memory limits (default 2GB), CPU limits, and configurable network modes (`none`, `bridge`, `host`)
- The `SecureBrokerBackend` communicates with an external broker process for enhanced isolation
- Development mode (`DEVELOPMENT_MODE=1`) uses standard Docker; production uses the secure runtime
