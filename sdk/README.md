# Term Challenge SDK

Professional multi-language SDK for building terminal agents.

## Languages

| Language | Status | Location |
|----------|--------|----------|
| Python | ✅ Production | `python/` |
| TypeScript | ✅ Production | `typescript/` |
| Rust | ✅ Production | `rust/` |

## Quick Start

### Python

```python
from term_sdk import Agent, AgentResponse, Command, Harness
from term_sdk.llm_client import LLMClient

class MyAgent(Agent):
    async def setup(self):
        self.client = LLMClient(provider="openrouter")
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        response = await self.client.chat([
            {"role": "system", "content": "You are a terminal expert."},
            {"role": "user", "content": f"Task: {instruction}\n\nTerminal:\n{screen}"}
        ])
        return self.parse_response(response.content)
    
    def parse_response(self, content: str) -> AgentResponse:
        # Parse LLM JSON response
        import json
        data = json.loads(content)
        return AgentResponse(
            analysis=data.get("analysis", ""),
            plan=data.get("plan", ""),
            commands=[Command(c["keystrokes"], c.get("duration", 1.0)) 
                      for c in data.get("commands", [])],
            task_complete=data.get("task_complete", False)
        )

if __name__ == "__main__":
    Harness(MyAgent()).run()
```

### TypeScript

```typescript
import { Agent, AgentResponse, Command, Harness, LLMClient } from 'term-sdk';

class MyAgent extends Agent {
    private client: LLMClient;

    async setup(): Promise<void> {
        this.client = new LLMClient({ provider: 'openrouter' });
    }

    async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
        const response = await this.client.chat([
            { role: 'system', content: 'You are a terminal expert.' },
            { role: 'user', content: `Task: ${instruction}\n\nTerminal:\n${screen}` }
        ]);
        return this.parseResponse(response.content);
    }

    private parseResponse(content: string): AgentResponse {
        const data = JSON.parse(content);
        return new AgentResponse({
            analysis: data.analysis ?? '',
            plan: data.plan ?? '',
            commands: (data.commands ?? []).map(
                (c: any) => new Command(c.keystrokes, c.duration ?? 1.0)
            ),
            taskComplete: data.task_complete ?? false
        });
    }
}

new Harness(new MyAgent()).run();
```

### Rust

```rust
use term_sdk::{Agent, AgentResponse, Command, Harness, LLMClient, Message};
use async_trait::async_trait;
use anyhow::Result;

struct MyAgent {
    client: Option<LLMClient>,
}

#[async_trait]
impl Agent for MyAgent {
    async fn setup(&mut self) -> Result<()> {
        self.client = Some(LLMClient::from_env()?);
        Ok(())
    }

    async fn step(&self, instruction: &str, screen: &str, step: u32) -> Result<AgentResponse> {
        let client = self.client.as_ref().unwrap();
        let messages = vec![
            Message::system("You are a terminal expert."),
            Message::user(format!("Task: {}\n\nTerminal:\n{}", instruction, screen)),
        ];
        
        let response = client.chat(&messages).await?;
        self.parse_response(&response.content)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    Harness::new(MyAgent { client: None }).run().await
}
```

## Protocol

Agents communicate with the harness via stdin/stdout JSON:

### Request (Harness → Agent)

```json
{
  "instruction": "Create a file called hello.txt",
  "screen": "root@container:/app# ",
  "step": 1
}
```

### Response (Agent → Harness)

```json
{
  "analysis": "Terminal shows empty prompt in /app directory",
  "plan": "Create the file using echo command",
  "commands": [
    {"keystrokes": "echo 'Hello, world!' > hello.txt\n", "duration": 1.0}
  ],
  "task_complete": false
}
```

## API Reference

### Agent (Base Class)

| Method | Description |
|--------|-------------|
| `setup()` | Initialize resources (optional) |
| `step(instruction, screen, step)` | Process one step (required) |
| `cleanup()` | Release resources (optional) |

### Command

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `keystrokes` | string | required | Text to send to terminal |
| `duration` | float | 1.0 | Seconds to wait after sending |

### AgentResponse

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `analysis` | string | "" | Analysis of terminal state |
| `plan` | string | "" | Plan for next steps |
| `commands` | Command[] | [] | Commands to execute |
| `task_complete` | bool | false | Whether task is finished |

### LLMClient

| Method | Description |
|--------|-------------|
| `chat(messages)` | Send chat completion request |
| `cost_tracker` | Access cumulative cost tracking |

### Providers

| Provider | Env Variable | Default Model |
|----------|--------------|---------------|
| openrouter | `OPENROUTER_API_KEY` | `anthropic/claude-3-haiku` |
| chutes | `CHUTES_API_KEY` | `Qwen/Qwen3-32B` |
| openai | `OPENAI_API_KEY` | `gpt-4o-mini` |
| anthropic | `ANTHROPIC_API_KEY` | `claude-3-haiku-20240307` |

## Special Keys

| Key | Keystroke | Description |
|-----|-----------|-------------|
| Enter | `\n` | Execute command |
| Tab | `\t` | Autocomplete |
| Ctrl+C | `\x03` | Interrupt |
| Ctrl+D | `\x04` | EOF |
| Escape | `\x1b` | Escape key |
| Up Arrow | `\x1b[A` | Previous command |
| Down Arrow | `\x1b[B` | Next command |

## Running Agents

```bash
# With term CLI
term bench agent -a ./my_agent.py -t ~/.cache/term-challenge/datasets/hello-world

# Set provider via environment
export LLM_PROVIDER=openrouter
export OPENROUTER_API_KEY=sk-or-...
export LLM_MODEL=anthropic/claude-3-haiku

# Or use Chutes
export LLM_PROVIDER=chutes
export CHUTES_API_KEY=cpk_...
```

## Examples

See `examples/` directory for complete working agents:

- `python/llm_agent.py` - Full Python LLM agent
- `typescript/llm_agent.ts` - TypeScript LLM agent
- `rust/llm_agent.rs` - Rust LLM agent

## License

Apache-2.0
