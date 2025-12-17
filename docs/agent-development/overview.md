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
        if req.is_first() { return Response::cmd("ls -la"); }
        Response::done()
    }
}

fn main() { run(&mut MyAgent); }
```

## LLM Integration - Multiple Models

Use different models dynamically - specify the model on each call:

### Python

```python
from term_sdk import Agent, Request, Response, LLM, run

class MultiModelAgent(Agent):
    def setup(self):
        self.llm = LLM()  # No default model needed
    
    def solve(self, req: Request) -> Response:
        # Fast model for quick analysis
        analysis = self.llm.ask(
            "Analyze this briefly",
            model="claude-3-haiku"
        )
        
        # Powerful model for complex reasoning
        solution = self.llm.ask(
            f"Solve: {req.instruction}",
            model="claude-3-opus"
        )
        
        # Different model for code
        code = self.llm.ask(
            "Write the code",
            model="gpt-4o"
        )
        
        return Response.from_llm(solution.text)
    
    def cleanup(self):
        # See per-model stats
        print(self.llm.get_stats())
```

### TypeScript

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

class MultiModelAgent implements Agent {
  private llm = new LLM();

  async solve(req: Request): Promise<Response> {
    // Fast model for quick tasks
    const analysis = await this.llm.ask("Quick analysis", {
      model: "claude-3-haiku"
    });
    
    // Powerful model for reasoning
    const solution = await this.llm.ask(`Solve: ${req.instruction}`, {
      model: "claude-3-opus",
      temperature: 0.2
    });
    
    return Response.fromLLM(solution.text);
  }
}

run(new MultiModelAgent());
```

### Rust

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct MultiModelAgent { llm: LLM }

impl Agent for MultiModelAgent {
    fn solve(&mut self, req: &Request) -> Response {
        // Fast model
        let _ = self.llm.ask("Quick check", "claude-3-haiku");
        
        // Powerful model
        match self.llm.ask(&req.instruction, "claude-3-opus") {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }
}

fn main() {
    run(&mut MultiModelAgent { llm: LLM::new() });
}
```

## Available Models

Any model supported by the provider (configured at upload):

| Model | Use Case |
|-------|----------|
| `claude-3-haiku` | Fast, cheap, simple tasks |
| `claude-3-sonnet` | Balanced performance |
| `claude-3-opus` | Complex reasoning |
| `gpt-4o` | Code generation |
| `gpt-4o-mini` | Fast, cheap |
| `llama-3-70b` | Open source |
| `mixtral-8x7b` | Open source |

## Function Calling

Define custom functions the LLM can call:

```python
from term_sdk import Agent, Request, Response, LLM, Tool, run

class ToolAgent(Agent):
    def setup(self):
        self.llm = LLM()
        self.llm.register_function("search", self.search)
    
    def search(self, query: str) -> str:
        return f"Found: {query}"
    
    def solve(self, req: Request) -> Response:
        tools = [Tool(
            name="search",
            description="Search for files",
            parameters={"type": "object", "properties": {"query": {"type": "string"}}}
        )]
        
        # Use any model with function calling
        result = self.llm.chat_with_functions(
            [{"role": "user", "content": req.instruction}],
            tools,
            model="claude-3-sonnet"
        )
        return Response.from_llm(result.text)
```

## Response Types

```python
Response.cmd("ls -la")                    # Execute command
Response.say("Analyzing...")              # Text only
Response.cmd("make").with_text("Building") # Command + text
Response.done("Completed!")               # Done with message
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
