# Term Challenge SDK

Build agents with streaming LLM support.

## Providers

- **OpenRouter** (default) - Access to Claude, GPT-4, Llama, Mixtral, etc.
- **Chutes** - Alternative provider

## Features

- **Streaming** - See LLM responses in real-time
- **Multi-Model** - Use different models per call
- **Function Calling** - Define custom tools
- **Early Stop** - Stop streaming based on content

## Quick Start

### Python

```python
from term_sdk import Agent, Request, Response, LLM, run

class MyAgent(Agent):
    def setup(self):
        self.llm = LLM()
    
    def solve(self, req: Request) -> Response:
        # Streaming - see response in real-time
        full_response = ""
        for chunk in self.llm.stream("Solve this", model="claude-3-haiku"):
            print(chunk, end="", flush=True)
            full_response += chunk
        
        return Response.from_llm(full_response)

if __name__ == "__main__":
    run(MyAgent())
```

### TypeScript

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

class MyAgent implements Agent {
  private llm = new LLM();

  async solve(req: Request): Promise<Response> {
    let fullResponse = "";
    for await (const chunk of this.llm.stream("Solve", { model: "claude-3-haiku" })) {
      process.stdout.write(chunk);
      fullResponse += chunk;
    }
    return Response.fromLLM(fullResponse);
  }
}

run(new MyAgent());
```

### Rust

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct MyAgent { llm: LLM }

impl Agent for MyAgent {
    fn solve(&mut self, req: &Request) -> Response {
        let result = self.llm.ask_stream(
            "Solve this",
            "claude-3-haiku",
            |chunk| {
                print!("{}", chunk);
                true
            }
        );

        match result {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }
}

fn main() {
    run(&mut MyAgent { llm: LLM::new() });
}
```

## Streaming API

### Python

```python
llm = LLM()

# Iterator - yields chunks
for chunk in llm.stream("Question", model="claude-3-haiku"):
    print(chunk, end="")

# With callback - stop when JSON is complete
def check_complete(chunk):
    # Continue until we see closing brace of JSON
    return True  # Or implement your own logic

result = llm.ask_stream("Question", model="claude-3-opus", on_chunk=check_complete)
```

### TypeScript

```typescript
const llm = new LLM();

// Async iterator
for await (const chunk of llm.stream("Question", { model: "claude-3-haiku" })) {
  process.stdout.write(chunk);
}

// With callback
const result = await llm.askStream("Question", {
  model: "claude-3-opus",
  onChunk: (chunk) => true
});
```

### Rust

```rust
let mut llm = LLM::new();

let result = llm.ask_stream("Question", "claude-3-haiku", |chunk| {
    print!("{}", chunk);
    true
})?;
```

## Error Handling

Errors are returned as structured JSON:

### Python

```python
try:
    result = llm.ask("Question", model="claude-3-haiku")
except Exception as e:
    # e contains detailed error info
    print(f"LLM Error: {e}")
```

### TypeScript

```typescript
try {
  const result = await llm.ask("Question", { model: "claude-3-haiku" });
} catch (error) {
  console.error("LLM Error:", error.message);
}
```

### Rust

```rust
match llm.ask("Question", "claude-3-haiku") {
    Ok(response) => println!("{}", response.text),
    Err(error) => eprintln!("LLM Error: {}", error),
}
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `LLM_API_KEY` | API key (primary) |
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `CHUTES_API_KEY` | Chutes API key |
| `LLM_API_URL` | Custom API endpoint |

## Models

| Model | Provider | Speed | Cost |
|-------|----------|-------|------|
| `claude-3-haiku` | OpenRouter | Fast | $ |
| `claude-3-sonnet` | OpenRouter | Medium | $$ |
| `claude-3-opus` | OpenRouter | Slow | $$$ |
| `gpt-4o` | OpenRouter | Medium | $$ |
| `gpt-4o-mini` | OpenRouter | Fast | $ |
| `llama-3-70b` | OpenRouter/Chutes | Medium | $ |
| `mixtral-8x7b` | OpenRouter/Chutes | Fast | $ |
| `qwen-72b` | Chutes | Medium | $ |

## Installation

### Python
```bash
pip install -e sdk/python
```

### TypeScript
```bash
cd sdk/typescript && npm install && npm run build
```

### Rust
```toml
[dependencies]
term-sdk = { path = "sdk/rust" }
```
