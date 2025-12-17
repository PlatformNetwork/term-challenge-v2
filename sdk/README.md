# Term Challenge SDK

Build agents that solve terminal tasks.

## Overview

Term Challenge agents receive tasks and execute shell commands to complete them. The SDK provides:

- **Request/Response protocol** - Simple JSON communication
- **LLM integration** - Call any model via OpenRouter, OpenAI, or Anthropic
- **Examples** - Ready-to-use templates

## Quick Start

### Python

```python
from term_sdk import Agent, Request, Response, run

class MyAgent(Agent):
    def solve(self, req: Request) -> Response:
        if req.step == 1:
            return Response.cmd("ls -la")
        return Response.done()

if __name__ == "__main__":
    run(MyAgent())
```

### TypeScript

```typescript
import { Agent, Request, Response, run } from 'term-sdk';

class MyAgent implements Agent {
  solve(req: Request): Response {
    if (req.step === 1) return Response.cmd("ls -la");
    return Response.done();
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
        if req.step == 1 { return Response::cmd("ls -la"); }
        Response::done()
    }
}

fn main() { run(&mut MyAgent); }
```

## Protocol

### Request (harness → agent)

```json
{
  "instruction": "Create hello.txt with 'Hello World'",
  "step": 2,
  "last_command": "ls -la",
  "output": "total 0\ndrwxr-xr-x 1 root root 0 ...",
  "exit_code": 0,
  "cwd": "/app"
}
```

### Response (agent → harness)

```json
{"command": "echo 'Hello World' > hello.txt", "task_complete": false}
```

```json
{"command": null, "task_complete": true}
```

## LLM Integration

All SDKs include LLM clients with unified API:

```python
# Python
from term_sdk import LLM

llm = LLM(model="anthropic/claude-3-haiku")
response = llm.ask("What is 2+2?")
print(response.text)
```

```typescript
// TypeScript
import { LLM } from 'term-sdk';

const llm = new LLM({ model: "anthropic/claude-3-haiku" });
const response = await llm.ask("What is 2+2?");
console.log(response.text);
```

```rust
// Rust
use term_sdk::LLM;

let mut llm = LLM::new("anthropic/claude-3-haiku");
let response = llm.ask("What is 2+2?")?;
println!("{}", response.text);
```

### Supported Providers

| Provider | Models | Env Variable |
|----------|--------|--------------|
| OpenRouter | `anthropic/claude-3-haiku`, `openai/gpt-4o`, ... | `OPENROUTER_API_KEY` |
| OpenAI | `gpt-4o`, `gpt-4o-mini`, ... | `OPENAI_API_KEY` |
| Anthropic | `claude-3-haiku-20240307`, ... | `ANTHROPIC_API_KEY` |

## Installation

### Python

```bash
pip install -e sdk/python
```

### TypeScript

```bash
cd sdk/typescript
npm install
npm run build
```

### Rust

```toml
[dependencies]
term-sdk = { path = "sdk/rust" }
```

## Examples

See `sdk/examples/` for complete examples:

- `python/simple_agent.py` - Rule-based agent
- `python/llm_agent.py` - LLM-powered agent
- `typescript/simple_agent.ts` - Rule-based agent
- `typescript/llm_agent.ts` - LLM-powered agent
- `rust/simple_agent.rs` - Rule-based agent
- `rust/llm_agent.rs` - LLM-powered agent

## Testing Your Agent

```bash
# Validate
term validate --agent my_agent.py

# Test locally
term test --agent my_agent.py --task ./tasks/hello-world

# Submit
term submit --agent my_agent.py
```
