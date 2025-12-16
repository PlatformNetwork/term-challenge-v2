# Rust Agent Development

Complete guide for building Term Challenge agents in Rust.

## Setup

Add the SDK to your `Cargo.toml` from the git repository:

```toml
[dependencies]
term-sdk = { git = "https://github.com/PlatformNetwork/term-challenge.git", path = "sdk/rust" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

Or if you have the repository cloned locally:

```toml
[dependencies]
term-sdk = { path = "/path/to/term-challenge/sdk/rust" }
```

## SDK Overview

```rust
use term_sdk::{
    // Core types
    Agent,           // Agent trait
    AgentRequest,    // Request from harness
    AgentResponse,   // Response to harness
    Command,         // Terminal command
    Harness,         // Agent runner
    
    // LLM client
    LlmClient,
    Provider,
    Message,
    ChatResponse,
};
```

## Basic Agent Structure

```rust
use anyhow::Result;
use term_sdk::{Agent, AgentResponse, Command, Harness};

struct MyAgent {
    // Your state here
}

impl MyAgent {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl Agent for MyAgent {
    fn name(&self) -> &str {
        "my-agent"
    }

    async fn setup(&self) -> Result<()> {
        // Initialize resources
        Ok(())
    }

    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        Ok(AgentResponse {
            analysis: "What I observe...".to_string(),
            plan: "What I'll do...".to_string(),
            commands: vec![
                Command::new("ls -la\n").with_duration(0.5)
            ],
            task_complete: false,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let agent = MyAgent::new();
    Harness::new(agent).run().await
}
```

## Core Types

### Command

```rust
use term_sdk::Command;

// Basic command with Enter
let cmd = Command::new("ls -la\n");

// Command with custom duration
let cmd = Command::new("pip install numpy\n").with_duration(10.0);

// From string (convenience)
let cmd = Command::from("ls -la\n");

// Special keys
let ctrl_c = Command::new("C-c").with_duration(0.1);
let tab = Command::new("Tab").with_duration(0.1);
let escape = Command::new("Escape").with_duration(0.1);
```

### AgentResponse

```rust
use term_sdk::{AgentResponse, Command};

let response = AgentResponse {
    analysis: "Terminal shows empty directory".to_string(),
    plan: "Create the requested file".to_string(),
    commands: vec![
        Command::new("echo 'Hello' > hello.txt\n").with_duration(0.3),
        Command::new("cat hello.txt\n").with_duration(0.3),
    ],
    task_complete: false,
};

// Create error response
let error = AgentResponse::error("Something went wrong");

// Mark task complete
let done = AgentResponse::complete("Task finished successfully");
```

### AgentRequest

```rust
use term_sdk::AgentRequest;

// Deserialized from JSON automatically by Harness
#[derive(Debug, Deserialize)]
pub struct AgentRequest {
    pub instruction: String,
    pub screen: String,
    pub step: u32,
}
```

## LLM Integration

### Basic LLM Agent

```rust
use anyhow::Result;
use term_sdk::{Agent, AgentResponse, Command, Harness, LlmClient, Provider, Message};

struct LlmAgent {
    client: LlmClient,
}

impl LlmAgent {
    async fn new() -> Result<Self> {
        let client = LlmClient::new(Provider::OpenRouter)?
            .with_model("anthropic/claude-3-haiku")
            .with_budget(10.0);
        
        Ok(Self { client })
    }

    fn parse_response(&self, content: &str) -> AgentResponse {
        // Find JSON in response
        let start = content.find('{').unwrap_or(0);
        let end = content.rfind('}').map(|i| i + 1).unwrap_or(content.len());
        let json_str = &content[start..end];

        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(data) => {
                let commands: Vec<Command> = data["commands"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|c| {
                                Command::new(
                                    c["keystrokes"].as_str().unwrap_or("")
                                ).with_duration(
                                    c["duration"].as_f64().unwrap_or(1.0)
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                AgentResponse {
                    analysis: data["analysis"].as_str().unwrap_or("").to_string(),
                    plan: data["plan"].as_str().unwrap_or("").to_string(),
                    commands,
                    task_complete: data["task_complete"].as_bool().unwrap_or(false),
                }
            }
            Err(e) => AgentResponse::error(&format!("Parse error: {}", e)),
        }
    }
}

#[async_trait::async_trait]
impl Agent for LlmAgent {
    fn name(&self) -> &str {
        "llm-agent"
    }

    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        let prompt = format!(
            r#"Task: {}

Terminal (step {}):
```
{}
```

Respond with JSON:
{{
  "analysis": "your analysis",
  "plan": "your plan",
  "commands": [{{"keystrokes": "...", "duration": 1.0}}],
  "task_complete": false
}}"#,
            instruction, step, screen
        );

        let response = self.client.chat(&[
            Message::system("You are a terminal expert."),
            Message::user(&prompt),
        ]).await?;

        Ok(self.parse_response(&response.content))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let agent = LlmAgent::new().await?;
    Harness::new(agent).run().await
}
```

### LlmClient Configuration

```rust
use term_sdk::{LlmClient, Provider};

// OpenRouter (default)
let client = LlmClient::new(Provider::OpenRouter)?
    .with_model("anthropic/claude-3-haiku")
    .with_api_key("sk-or-...")  // Or use OPENROUTER_API_KEY env
    .with_budget(5.0)
    .with_timeout(Duration::from_secs(300));

// Chutes
let client = LlmClient::new(Provider::Chutes)?
    .with_model("Qwen/Qwen3-32B");

// OpenAI
let client = LlmClient::new(Provider::OpenAI)?
    .with_model("gpt-4o-mini");
```

### Chat Options

```rust
use term_sdk::Message;

let response = client.chat_with_options(
    &[
        Message::system("You are helpful."),
        Message::user("Hello!"),
    ],
    ChatOptions {
        model: Some("gpt-4o".to_string()),
        temperature: Some(0.7),
        max_tokens: Some(4096),
    }
).await?;

// Response fields
println!("Content: {}", response.content);
println!("Prompt tokens: {}", response.prompt_tokens);
println!("Completion tokens: {}", response.completion_tokens);
println!("Cost: ${:.4}", response.cost);
println!("Latency: {}ms", response.latency_ms);
```

### Cost Tracking

```rust
let client = LlmClient::new(Provider::OpenRouter)?
    .with_budget(10.0);

// After making calls...
println!("Total cost: ${:.4}", client.total_cost());
println!("Total tokens: {}", client.total_tokens());
println!("Requests: {}", client.request_count());
```

## Advanced Patterns

### Stateful Agent

```rust
use std::sync::Mutex;

struct StatefulAgent {
    client: LlmClient,
    history: Mutex<Vec<Message>>,
}

impl StatefulAgent {
    fn new() -> Result<Self> {
        Ok(Self {
            client: LlmClient::new(Provider::OpenRouter)?,
            history: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl Agent for StatefulAgent {
    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        let mut history = self.history.lock().unwrap();
        
        // Add current state
        history.push(Message::user(&format!("Step {}:\n{}", step, screen)));
        
        // Keep history manageable
        if history.len() > 20 {
            *history = history[history.len()-20..].to_vec();
        }
        
        // Build messages
        let mut messages = vec![Message::system(&format!("Task: {}", instruction))];
        messages.extend(history.iter().cloned());
        
        let response = self.client.chat(&messages).await?;
        
        // Add response to history
        history.push(Message::assistant(&response.content));
        
        Ok(self.parse_response(&response.content))
    }
}
```

### Error Recovery

```rust
#[async_trait::async_trait]
impl Agent for RobustAgent {
    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        // Detect common errors
        if screen.contains("command not found") {
            return Ok(AgentResponse {
                analysis: "Previous command not found".to_string(),
                plan: "Try alternative command".to_string(),
                commands: vec![Command::new("which python3\n").with_duration(0.3)],
                task_complete: false,
            });
        }
        
        if screen.contains("Permission denied") {
            return Ok(AgentResponse {
                analysis: "Permission error detected".to_string(),
                plan: "Try with elevated privileges".to_string(),
                commands: vec![Command::new("sudo !!\n").with_duration(1.0)],
                task_complete: false,
            });
        }
        
        // Normal processing...
        self.normal_step(instruction, screen, step).await
    }
}
```

### Timeout Handling

```rust
use std::time::Instant;

struct TimeoutAgent {
    client: LlmClient,
    start_time: Mutex<Option<Instant>>,
    max_duration: Duration,
}

#[async_trait::async_trait]
impl Agent for TimeoutAgent {
    async fn setup(&self) -> Result<()> {
        *self.start_time.lock().unwrap() = Some(Instant::now());
        Ok(())
    }

    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        let start = self.start_time.lock().unwrap().unwrap();
        
        if start.elapsed() > self.max_duration {
            return Ok(AgentResponse::complete("Timeout reached"));
        }
        
        // Normal processing...
    }
}
```

## Logging

Use `tracing` for structured logging to stderr:

```rust
use tracing::{info, debug, warn, error};

#[async_trait::async_trait]
impl Agent for MyAgent {
    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        info!(step, "Processing step");
        debug!(screen_len = screen.len(), "Screen content");
        
        // ...
        
        if let Err(e) = result {
            error!(error = %e, "LLM call failed");
        }
    }
}

fn main() {
    // Initialize logging to stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    
    // ...
}
```

## Complete Example

```rust
//! Complete LLM-powered terminal agent for Term Challenge.

use anyhow::Result;
use serde_json::Value;
use std::sync::Mutex;
use term_sdk::{Agent, AgentResponse, Command, Harness, LlmClient, Message, Provider};
use tracing::{info, warn};

const SYSTEM_PROMPT: &str = r#"You are an expert terminal agent. Complete tasks using shell commands.

Rules:
1. Analyze the terminal output carefully
2. Execute one logical step at a time
3. Verify your actions worked before proceeding
4. Use appropriate wait durations
5. Set task_complete=true only when verified complete

Respond with JSON:
{
  "analysis": "What you observe in the terminal",
  "plan": "What you will do next",
  "commands": [{"keystrokes": "command\n", "duration": 1.0}],
  "task_complete": false
}"#;

struct TerminalAgent {
    client: LlmClient,
    history: Mutex<Vec<Message>>,
}

impl TerminalAgent {
    async fn new(model: &str) -> Result<Self> {
        info!(model, "Initializing agent");
        
        let client = LlmClient::new(Provider::OpenRouter)?
            .with_model(model)
            .with_budget(10.0)
            .with_temperature(0.3);
        
        Ok(Self {
            client,
            history: Mutex::new(Vec::new()),
        })
    }

    fn parse_response(&self, content: &str) -> AgentResponse {
        // Remove <think> blocks (Qwen models)
        let content = remove_think_blocks(content);
        
        // Find JSON
        let start = match content.find('{') {
            Some(i) => i,
            None => {
                warn!("No JSON found in response");
                return AgentResponse::error("No JSON in response");
            }
        };
        
        let end = content.rfind('}').map(|i| i + 1).unwrap_or(content.len());
        let json_str = &content[start..end];

        match serde_json::from_str::<Value>(json_str) {
            Ok(data) => {
                let commands: Vec<Command> = data["commands"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| {
                                let keystrokes = c["keystrokes"].as_str()?;
                                let duration = c["duration"].as_f64().unwrap_or(1.0);
                                Some(Command::new(keystrokes).with_duration(duration))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                AgentResponse {
                    analysis: data["analysis"].as_str().unwrap_or("").to_string(),
                    plan: data["plan"].as_str().unwrap_or("").to_string(),
                    commands,
                    task_complete: data["task_complete"].as_bool().unwrap_or(false),
                }
            }
            Err(e) => {
                warn!(error = %e, "JSON parse error");
                AgentResponse {
                    analysis: format!("Parse error: {}", e),
                    plan: content[..content.len().min(500)].to_string(),
                    commands: vec![],
                    task_complete: false,
                }
            }
        }
    }
}

fn remove_think_blocks(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            result = format!("{}{}", &result[..start], &result[end + 8..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}

#[async_trait::async_trait]
impl Agent for TerminalAgent {
    fn name(&self) -> &str {
        "terminal-agent"
    }

    async fn setup(&self) -> Result<()> {
        info!("Agent ready");
        Ok(())
    }

    async fn step(
        &self,
        instruction: &str,
        screen: &str,
        step: u32,
    ) -> Result<AgentResponse> {
        info!(step, "Processing");

        let user_msg = format!(
            r#"Task: {}

Current Terminal (Step {}):
```
{}
```

What's your next action?"#,
            instruction,
            step,
            &screen[screen.len().saturating_sub(2000)..]
        );

        // Update conversation
        {
            let mut history = self.history.lock().unwrap();
            history.push(Message::user(&user_msg));
            
            // Keep manageable
            if history.len() > 10 {
                *history = history[history.len() - 10..].to_vec();
            }
        }

        // Build messages
        let messages: Vec<Message> = {
            let history = self.history.lock().unwrap();
            let mut msgs = vec![Message::system(SYSTEM_PROMPT)];
            msgs.extend(history.iter().cloned());
            msgs
        };

        // Call LLM
        let response = self.client.chat(&messages).await?;
        
        info!(
            latency_ms = response.latency_ms,
            cost = format!("${:.4}", response.cost),
            "LLM response"
        );

        // Add to history
        {
            let mut history = self.history.lock().unwrap();
            history.push(Message::assistant(&response.content));
        }

        Ok(self.parse_response(&response.content))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info")
        .init();

    // Parse args
    let model = std::env::args()
        .skip_while(|a| a != "--model")
        .nth(1)
        .unwrap_or_else(|| "anthropic/claude-3-haiku".to_string());

    let agent = TerminalAgent::new(&model).await?;
    Harness::new(agent).run().await
}
```

## Building & Running

```bash
# Build release binary
cargo build --release

# Run directly
./target/release/my_agent

# With the harness
term bench agent -a ./target/release/my_agent -t /path/to/task

# With custom model
./target/release/my_agent --model gpt-4o
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `CHUTES_API_KEY` | Chutes API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `RUST_LOG` | Log level (e.g., `info`, `debug`) |

## Dependencies

Recommended `Cargo.toml`:

```toml
[package]
name = "my-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
term-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[profile.release]
opt-level = 3
lto = true
```
