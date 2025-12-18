# Term Challenge Protocol Specification

This document defines the communication protocol between the harness and agents.

## Overview

Agents communicate with the harness via **JSON over stdin/stdout**, one message per line. The agent process stays alive between steps, preserving memory and state.

## Communication Flow

```
┌─────────┐                    ┌─────────┐                    ┌──────────┐
│  Agent  │                    │ Harness │                    │ Terminal │
└────┬────┘                    └────┬────┘                    └────┬─────┘
     │                              │                              │
     │  ◄── Request (JSON line) ────│                              │
     │                              │                              │
     │── Response (JSON line) ──►   │                              │
     │                              │── Execute command ──►        │
     │                              │                              │
     │                              │  ◄── Command output ────     │
     │  ◄── Request (JSON line) ────│                              │
     │                              │                              │
     │      (repeat until task_complete = true)                    │
     ▼                              ▼                              ▼
```

## Request Format (Harness → Agent)

One JSON object per line:

```json
{"instruction": "Create hello.txt with 'Hello, world!'", "step": 1, "last_command": null, "output": null, "exit_code": null, "cwd": "/app"}
```

### Request Fields

| Field | Type | Description |
|-------|------|-------------|
| `instruction` | string | Task to complete (same for all steps) |
| `step` | integer | Current step number (starts at 1) |
| `last_command` | string \| null | Previous command executed (null on step 1) |
| `output` | string \| null | Output from last command (null on step 1) |
| `exit_code` | integer \| null | Exit code from last command (null on step 1) |
| `cwd` | string | Current working directory |

### Example Request Sequence

**Step 1 (initial):**
```json
{"instruction": "Create a file hello.txt containing 'Hello, world!'", "step": 1, "last_command": null, "output": null, "exit_code": null, "cwd": "/app"}
```

**Step 2 (after command):**
```json
{"instruction": "Create a file hello.txt containing 'Hello, world!'", "step": 2, "last_command": "echo 'Hello, world!' > hello.txt", "output": "", "exit_code": 0, "cwd": "/app"}
```

**Step 3 (verification):**
```json
{"instruction": "Create a file hello.txt containing 'Hello, world!'", "step": 3, "last_command": "cat hello.txt", "output": "Hello, world!", "exit_code": 0, "cwd": "/app"}
```

## Response Format (Agent → Harness)

One JSON object per line:

```json
{"command": "echo 'Hello, world!' > hello.txt", "task_complete": false}
```

### Response Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `command` | string \| null | null | Shell command to execute |
| `task_complete` | boolean | false | Set to `true` when task is finished |
| `text` | string \| null | null | Optional message/analysis (for logging) |

### Response Examples

**Execute a command:**
```json
{"command": "ls -la", "task_complete": false}
```

**Execute with message:**
```json
{"command": "cat hello.txt", "task_complete": false, "text": "Verifying file contents"}
```

**Signal completion:**
```json
{"command": null, "task_complete": true}
```

**Complete with message:**
```json
{"command": null, "task_complete": true, "text": "Task completed successfully"}
```

## Agent Lifecycle

```python
# 1. Setup (called once at start)
agent.setup()

# 2. Process steps (called for each request)
while True:
    request = read_line_from_stdin()
    response = agent.solve(request)
    write_line_to_stdout(response)
    
    if response.task_complete:
        break

# 3. Cleanup (called at end)
agent.cleanup()
```

**Important:** The agent process stays alive between steps. Use `setup()` to initialize state that persists across steps (LLM client, plans, history, etc.).

## Reading Input Correctly

**IMPORTANT:** Read stdin line-by-line, not until EOF.

### Python
```python
# CORRECT - line by line
for line in sys.stdin:
    request = json.loads(line)
    # process...

# WRONG - waits for EOF
data = sys.stdin.read()  # Will timeout!
```

### TypeScript
```typescript
// CORRECT - readline interface
import * as readline from 'readline';
const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
    const request = JSON.parse(line);
    // process...
}
```

### Rust
```rust
// CORRECT - lines iterator
use std::io::BufRead;
for line in std::io::stdin().lock().lines() {
    let request: Request = serde_json::from_str(&line?)?;
    // process...
}
```

## Writing Output Correctly

Always flush stdout after writing:

### Python
```python
print(json.dumps(response), flush=True)
```

### TypeScript
```typescript
console.log(JSON.stringify(response));  // Auto-flushes
```

### Rust
```rust
println!("{}", serde_json::to_string(&response)?);
std::io::stdout().flush()?;
```

## Logging (stderr)

Use stderr for debug output - only stdout is used for protocol:

```python
import sys
print("[agent] Processing step 1...", file=sys.stderr)
```

Logs from stderr are captured and displayed by the harness.

## Complete Example

### Request 1
```json
{"instruction": "Create hello.txt with 'Hello, world!'", "step": 1, "output": null, "exit_code": null, "cwd": "/app"}
```

### Response 1
```json
{"command": "echo 'Hello, world!' > hello.txt", "task_complete": false}
```

### Request 2
```json
{"instruction": "Create hello.txt with 'Hello, world!'", "step": 2, "last_command": "echo 'Hello, world!' > hello.txt", "output": "", "exit_code": 0, "cwd": "/app"}
```

### Response 2
```json
{"command": "cat hello.txt", "task_complete": false, "text": "Verifying file was created"}
```

### Request 3
```json
{"instruction": "Create hello.txt with 'Hello, world!'", "step": 3, "last_command": "cat hello.txt", "output": "Hello, world!", "exit_code": 0, "cwd": "/app"}
```

### Response 3
```json
{"command": null, "task_complete": true, "text": "File created successfully"}
```

## Error Handling

If the agent outputs invalid JSON:
- Harness logs the error
- Agent is given another chance
- After multiple failures, task fails

If the agent times out (300s default):
- Agent process is terminated
- Task fails with timeout error

## SDK Support

Use the official SDKs for easier implementation:

| Language | Installation |
|----------|-------------|
| Python | `pip install term-sdk` or copy `sdk/python/term_sdk/` |
| TypeScript | `npm install term-sdk` or copy `sdk/typescript/` |
| Rust | Add `term-sdk` to `Cargo.toml` |

### Minimal Python Agent

```python
#!/usr/bin/env python3
from term_sdk import Agent, Request, Response, run

class MyAgent(Agent):
    def solve(self, req: Request) -> Response:
        if req.first:
            return Response.cmd("echo 'Hello, world!' > hello.txt")
        return Response.done()

if __name__ == "__main__":
    run(MyAgent())
```

## Legacy Format Support

The harness also accepts the legacy Terminal-Bench format for backward compatibility:

```json
{
  "analysis": "Terminal shows empty directory",
  "plan": "Create file using echo",
  "commands": [{"keystrokes": "echo 'hello' > file.txt\n", "duration": 0.5}],
  "task_complete": false
}
```

However, the new format (`command` + `task_complete`) is recommended.
