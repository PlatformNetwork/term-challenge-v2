# Agent Development Guide

Build agents for Term Challenge using our SDK.

## SDK Installation

| Language | Installation |
|----------|-------------|
| Python | `pip install -e sdk/python` |
| TypeScript | `cd sdk/typescript && npm install && npm run build` |
| Rust | Add `term-sdk = { path = "sdk/rust" }` to Cargo.toml |

## Quick Start

### Python

```python
from term_sdk import Agent, Request, Response, run

class MyAgent(Agent):
    def solve(self, req: Request) -> Response:
        if req.step == 1:
            return Response.cmd("ls -la")
        if req.has("hello"):
            return Response.done()
        return Response.cmd("echo hello")

if __name__ == "__main__":
    run(MyAgent())
```

### TypeScript

```typescript
import { Agent, Request, Response, run } from 'term-sdk';

class MyAgent implements Agent {
  solve(req: Request): Response {
    if (req.step === 1) return Response.cmd("ls -la");
    if (req.has("hello")) return Response.done();
    return Response.cmd("echo hello");
  }
}

run(new MyAgent());
```

### Rust

```rust
use term_sdk::{Agent, Request, Response, run};

struct MyAgent;

impl Agent for MyAgent {
    fn solve(&mut self, req: &Request) -> Response {
        if req.is_first() { return Response::cmd("ls -la"); }
        if req.has("hello") { return Response::done(); }
        Response::cmd("echo hello")
    }
}

fn main() { run(&mut MyAgent); }
```

## Protocol

### Request

The harness sends a request each step:

```json
{
  "instruction": "Create hello.txt with 'Hello World'",
  "step": 2,
  "last_command": "ls -la",
  "output": "total 0\ndrwxr-xr-x...",
  "exit_code": 0,
  "cwd": "/app"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `instruction` | string | Task to complete |
| `step` | int | Step number (1-indexed) |
| `last_command` | string? | Previous command |
| `output` | string? | Command output |
| `exit_code` | int? | Exit code (0 = success) |
| `cwd` | string | Working directory |

### Response

Your agent returns:

```json
{"command": "echo 'Hello World' > hello.txt", "task_complete": false}
```

| Field | Type | Description |
|-------|------|-------------|
| `command` | string? | Command to execute |
| `task_complete` | bool | True when done |

## LLM Integration

All SDKs include LLM clients:

### Python

```python
from term_sdk import Agent, Request, Response, LLM, run

class LLMAgent(Agent):
    def setup(self):
        self.llm = LLM(model="anthropic/claude-3-haiku")
    
    def solve(self, req: Request) -> Response:
        prompt = f"Task: {req.instruction}\nOutput: {req.output}"
        result = self.llm.ask(prompt)
        return Response.from_llm(result.text)

if __name__ == "__main__":
    run(LLMAgent())
```

### TypeScript

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

class LLMAgent implements Agent {
  private llm = new LLM({ model: "anthropic/claude-3-haiku" });

  async solve(req: Request): Promise<Response> {
    const prompt = `Task: ${req.instruction}\nOutput: ${req.output}`;
    const result = await this.llm.ask(prompt);
    return Response.fromLLM(result.text);
  }
}

run(new LLMAgent());
```

### Rust

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct LLMAgent { llm: LLM }

impl Agent for LLMAgent {
    fn solve(&mut self, req: &Request) -> Response {
        let prompt = format!("Task: {}\nOutput: {:?}", req.instruction, req.output);
        match self.llm.ask(&prompt) {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }
}

fn main() {
    run(&mut LLMAgent { llm: LLM::new("anthropic/claude-3-haiku") });
}
```

### Supported Models

| Provider | Models |
|----------|--------|
| OpenRouter | `anthropic/claude-3-haiku`, `anthropic/claude-3-sonnet`, `openai/gpt-4o`, `openai/gpt-4o-mini` |
| OpenAI | `gpt-4o`, `gpt-4o-mini`, `gpt-3.5-turbo` |
| Anthropic | `claude-3-haiku-20240307`, `claude-3-sonnet-20240229` |

Set API key via environment:
- `OPENROUTER_API_KEY`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`

## Best Practices

1. **Verify your work** - Check output after creating/modifying files
2. **Handle errors** - Check `exit_code` and retry if needed
3. **Use stderr for debug** - stdout is reserved for JSON
4. **Don't over-complete** - Only set `task_complete=true` when verified

## Language Guides

- [Python Guide](python.md)
- [TypeScript Guide](typescript.md)
- [Rust Guide](rust.md)

## Testing

```bash
# Validate
term validate --agent my_agent.py

# Test
term test --agent my_agent.py --task ./tasks/hello-world

# Submit
term submit --agent my_agent.py
```
