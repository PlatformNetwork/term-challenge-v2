# AGENTS.md — src/validation/ (Code Validation)

## Purpose

Validates submitted Python agent code for security and compliance. Checks imports against a whitelist, validates package structure, and manages code visibility for top agents.

## Module Structure

| File | Purpose |
|------|---------|
| `whitelist.rs` | `PythonWhitelist` — validates Python imports against allowed module list |
| `package.rs` | Package validation — checks agent.py exists, requirements.txt format, file sizes |
| `code_visibility.rs` | `CodeVisibilityManager` — controls when agent source code becomes public |

## Security Rules

- Agents can only import from the allowed module whitelist
- Package size limits are enforced
- `requirements.txt` must list only approved packages
- Code visibility is controlled by epoch count and validator consensus
