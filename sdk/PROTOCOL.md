# Term Challenge Protocol Specification

This document defines the protocol for building agents compatible with Term Challenge and Terminal-Bench.

## Overview

Agents interact with a sandboxed Linux terminal environment via a simple request-response protocol. The agent receives the current terminal state and responds with commands to execute.

## Communication Flow

```
┌─────────┐                    ┌─────────┐                    ┌──────────┐
│  Agent  │                    │ Harness │                    │ Terminal │
└────┬────┘                    └────┬────┘                    └────┬─────┘
     │                              │                              │
     │  ◄── Task + Terminal State ──│                              │
     │                              │                              │
     │── Response (commands) ──►    │                              │
     │                              │── Execute keystrokes ──►     │
     │                              │                              │
     │                              │  ◄── Terminal output ────    │
     │  ◄── New Terminal State ──── │                              │
     │                              │                              │
     │      (repeat until task_complete = true)                    │
     ▼                              ▼                              ▼
```

## Request Format (Harness → Agent)

The harness sends task instructions and terminal state:

```
Task Description:
{instruction}

Current terminal state:
{terminal_output}
```

## Response Format (Agent → Harness)

### JSON Format (Recommended)

```json
{
  "analysis": "string - Analysis of current terminal state",
  "plan": "string - Plan for next steps",
  "commands": [
    {
      "keystrokes": "string - Exact keystrokes to send",
      "duration": 0.1
    }
  ],
  "task_complete": false
}
```

### XML Format (Alternative)

```xml
<response>
<analysis>
Analysis of current terminal state
</analysis>
<plan>
Plan for next steps
</plan>
<commands>
<command>
<keystrokes>ls -la
</keystrokes>
<duration>0.1</duration>
</command>
</commands>
<task_complete>false</task_complete>
</response>
```

## Field Specifications

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `analysis` | string | Your analysis of the current terminal state. What do you see? What has been accomplished? |
| `plan` | string | Your plan for the next steps. What commands will you run and why? |
| `commands` | array | List of command objects to execute |

### Command Object

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `keystrokes` | string | (required) | Exact text to send to terminal. Include `\n` to execute commands. |
| `duration` | float | 1.0 | Seconds to wait after sending keystrokes (0.1 - 60.0) |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `task_complete` | boolean | false | Set to `true` when task is finished (requires double confirmation) |

## Keystrokes Syntax

### Basic Commands
```json
{"keystrokes": "ls -la\n", "duration": 0.1}
{"keystrokes": "cd /home/user\n", "duration": 0.1}
{"keystrokes": "python script.py\n", "duration": 5.0}
```

### Special Keys (tmux-style)
| Key | Syntax | Example |
|-----|--------|---------|
| Enter | `\n` or send "Enter" | `"ls -la\n"` |
| Ctrl+C | `C-c` | `{"keystrokes": "C-c", "duration": 0.1}` |
| Ctrl+D | `C-d` | `{"keystrokes": "C-d", "duration": 0.1}` |
| Ctrl+Z | `C-z` | `{"keystrokes": "C-z", "duration": 0.1}` |
| Ctrl+L | `C-l` | `{"keystrokes": "C-l", "duration": 0.1}` |
| Escape | `Escape` | `{"keystrokes": "Escape", "duration": 0.1}` |
| Tab | `Tab` | `{"keystrokes": "Tab", "duration": 0.1}` |
| Backspace | `BSpace` | `{"keystrokes": "BSpace", "duration": 0.1}` |
| Arrow Up | `Up` | `{"keystrokes": "Up", "duration": 0.1}` |
| Arrow Down | `Down` | `{"keystrokes": "Down", "duration": 0.1}` |

### Interactive Programs (vim, less, etc.)

For interactive programs, send keystrokes without waiting:

```json
{
  "commands": [
    {"keystrokes": "vim test.py\n", "duration": 0.5},
    {"keystrokes": "i", "duration": 0.1},
    {"keystrokes": "print('hello')", "duration": 0.1},
    {"keystrokes": "Escape", "duration": 0.1},
    {"keystrokes": ":wq\n", "duration": 0.3}
  ]
}
```

## Duration Guidelines

| Command Type | Recommended Duration |
|--------------|---------------------|
| Instant (cd, echo, ls) | 0.1s |
| Fast (cat, grep, find) | 0.5s |
| Medium (gcc, rustc, npm install) | 1.0 - 5.0s |
| Slow (make, docker build, wget) | 5.0 - 30.0s |
| Very slow (large compilation) | 30.0 - 60.0s |

**Important**: Never wait longer than 60 seconds. Use polling instead:
```json
{"keystrokes": "", "duration": 10.0}
```

## Task Completion

When you believe the task is complete:

1. First response: Set `"task_complete": true`
2. Harness will ask for confirmation
3. Second response: Set `"task_complete": true` again to confirm

This double-confirmation prevents premature completion.

## Error Handling

If your response has errors:
- Invalid JSON → Error message sent back
- Missing required fields → Warning + retry
- Invalid keystrokes → Command skipped

## Example: Complete Agent Response

```json
{
  "analysis": "I can see an empty directory. The task asks me to create a Python script that prints 'Hello World'.",
  "plan": "I will: 1) Create a file called hello.py using echo, 2) Verify the file was created, 3) Run the script to test it.",
  "commands": [
    {
      "keystrokes": "echo \"print('Hello World')\" > hello.py\n",
      "duration": 0.1
    },
    {
      "keystrokes": "cat hello.py\n",
      "duration": 0.1
    },
    {
      "keystrokes": "python3 hello.py\n",
      "duration": 0.5
    }
  ],
  "task_complete": false
}
```

## Compatibility

This protocol is compatible with:
- **Terminal-Bench** harness (terminus-2 JSON/XML parsers)
- **Term Challenge** evaluation system
- Any framework that can produce JSON/XML responses

## Building Compatible Agents

Your agent must:
1. Parse the task instruction and terminal state
2. Generate a valid JSON or XML response
3. Handle the conversation loop until task completion

See language-specific SDKs for implementation helpers:
- [Python SDK](./python/README.md)
- [JavaScript SDK](./javascript/README.md)
- [Rust SDK](./rust/README.md)
