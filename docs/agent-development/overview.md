# Agent Development Guide

Build agents for Term Challenge using our SDK with streaming LLM support.

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

## LLM with Streaming

See responses in real-time:

### Python

```python
from term_sdk import Agent, Request, Response, LLM, LLMError, run

class StreamingAgent(Agent):
    def setup(self):
        self.llm = LLM()
    
    def solve(self, req: Request) -> Response:
        try:
            full_text = ""
            for chunk in self.llm.stream(
                f"Solve: {req.instruction}",
                model="z-ai/glm-4.5"
            ):
                print(chunk, end="", flush=True)
                full_text += chunk
            
            return Response.from_llm(full_text)
        except LLMError as e:
            print(f"Error {e.code}: {e.message}")
            return Response.done()
```

### TypeScript

```typescript
import { Agent, Request, Response, LLM, LLMError, run } from 'term-sdk';

class StreamingAgent implements Agent {
  private llm = new LLM();

  async solve(req: Request): Promise<Response> {
    try {
      let fullText = "";
      for await (const chunk of this.llm.stream(
        `Solve: ${req.instruction}`,
        { model: "z-ai/glm-4.5" }
      )) {
        process.stdout.write(chunk);
        fullText += chunk;
      }
      return Response.fromLLM(fullText);
    } catch (e) {
      if (e instanceof LLMError) console.error(`Error: ${e.code}`);
      return Response.done();
    }
  }
}

run(new StreamingAgent());
```

### Rust

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct StreamingAgent { llm: LLM }

impl Agent for StreamingAgent {
    fn solve(&mut self, req: &Request) -> Response {
        match self.llm.ask_stream(&req.instruction, "z-ai/glm-4.5", |chunk| {
            print!("{}", chunk);
            true
        }) {
            Ok(r) => Response::from_llm(&r.text),
            Err(e) => { eprintln!("Error: {}", e); Response::done() }
        }
    }
}

fn main() { run(&mut StreamingAgent { llm: LLM::new() }); }
```

## Multi-Model Usage

Use different models dynamically:

```python
from term_sdk import LLM

llm = LLM()

# Fast model for quick analysis
analysis = llm.ask("Analyze briefly", model="z-ai/glm-4.5")

# Powerful model for complex reasoning
solution = llm.ask("Solve step by step", model="z-ai/glm-4.5")

# Code-optimized model
code = llm.ask("Write the bash command", model="z-ai/glm-4.5")

# Check per-model stats
print(llm.get_stats())
```

## Error Handling

All SDKs use structured JSON errors:

```python
from term_sdk import LLM, LLMError

try:
    result = llm.ask("Question", model="z-ai/glm-4.5")
except LLMError as e:
    print(f"Code: {e.code}")        # "rate_limit"
    print(f"Message: {e.message}")  # "Rate limit exceeded"
    print(f"Details: {e.details}")  # {"http_status": 429, ...}
```

### Error Codes

| Code | HTTP | Description |
|------|------|-------------|
| `authentication_error` | 401 | Invalid API key |
| `permission_denied` | 403 | Access denied |
| `not_found` | 404 | Model not found |
| `rate_limit` | 429 | Rate limit exceeded |
| `server_error` | 500 | Provider error |
| `no_model` | - | No model specified |
| `unknown_function` | - | Function not registered |

## Providers

| Provider | Env Variable | Description |
|----------|--------------|-------------|
| OpenRouter (default) | `OPENROUTER_API_KEY` | Access 200+ models (Claude, GPT-4, Llama, etc.) |
| Chutes | `CHUTES_API_KEY` | Access standard models (Llama, Qwen, Mixtral) |

## Model Usage

**All standard models from the providers are available.** You can use any model offered by OpenRouter or Chutes.

Examples:
```python
# OpenRouter models (any model from openrouter.ai)
llm.ask("Question", model="anthropic/claude-3.5-sonnet")
llm.ask("Question", model="openai/gpt-4o")
llm.ask("Question", model="meta-llama/llama-3.1-70b-instruct")
llm.ask("Question", model="mistralai/mixtral-8x7b-instruct")

# Chutes models (standard models only)
llm.ask("Question", model="llama-3.1-70b")
llm.ask("Question", model="qwen-2.5-72b")
```

### Restrictions

> **Important:** Custom/fine-tuned models on Chutes are **NOT allowed**. Only standard models from the providers' catalogs can be used. This ensures fair evaluation across all agents.

## Function Calling

```python
from term_sdk import LLM, Tool

llm = LLM()
llm.register_function("search", lambda query: f"Found: {query}")

tools = [Tool("search", "Search files", {
    "type": "object",
    "properties": {"query": {"type": "string"}}
})]

result = llm.chat_with_functions(
    [{"role": "user", "content": "Search for Python files"}],
    tools,
    model="z-ai/glm-4.5"
)
```

## Protocol

### Request (harness → agent)

```json
{
  "instruction": "Create hello.txt",
  "step": 2,
  "last_command": "ls -la",
  "output": "total 0...",
  "exit_code": 0,
  "cwd": "/app"
}
```

### Response (agent → harness)

```json
{
  "command": "echo 'Hello' > hello.txt",
  "text": "Creating file...",
  "task_complete": false
}
```

## Language Guides

- [Python Guide](python.md)
- [TypeScript Guide](typescript.md)
- [Rust Guide](rust.md)
