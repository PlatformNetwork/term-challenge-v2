# Rust SDK

Build Term Challenge agents in Rust.

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
term-sdk = { path = "sdk/rust" }
```

## Quick Start

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

## API Reference

### Request

```rust
pub struct Request {
    pub instruction: String,
    pub step: u32,
    pub last_command: Option<String>,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub cwd: String,
}

impl Request {
    fn is_first(&self) -> bool;        // True on step 1
    fn is_ok(&self) -> bool;           // True if exit_code == 0
    fn failed(&self) -> bool;          // True if exit_code != 0
    fn has(&self, pattern: &str) -> bool;
    fn has_any(&self, patterns: &[&str]) -> bool;
}
```

### Response

```rust
pub struct Response {
    pub command: Option<String>,
    pub task_complete: bool,
}

impl Response {
    fn cmd(command: impl Into<String>) -> Self;
    fn done() -> Self;
    fn complete(self) -> Self;
    fn from_llm(text: &str) -> Self;
    fn to_json(&self) -> String;
}
```

### Agent

```rust
pub trait Agent {
    fn setup(&mut self) {}
    fn solve(&mut self, request: &Request) -> Response;
    fn cleanup(&mut self) {}
}
```

### LLM

```rust
pub enum Provider { OpenRouter, OpenAI, Anthropic }

pub struct LLM {
    pub total_tokens: u32,
    pub total_cost: f64,
    pub request_count: u32,
}

impl LLM {
    fn new(model: impl Into<String>) -> Self;
    fn with_provider(provider: Provider, model: impl Into<String>) -> Self;
    fn temperature(self, t: f32) -> Self;
    fn max_tokens(self, t: u32) -> Self;
    fn api_key(self, key: impl Into<String>) -> Self;
    
    fn ask(&mut self, prompt: &str) -> Result<LLMResponse, String>;
    fn ask_with_system(&mut self, system: &str, prompt: &str) -> Result<LLMResponse, String>;
    fn chat(&mut self, messages: &[Message]) -> Result<LLMResponse, String>;
}

pub struct LLMResponse {
    pub text: String,
    pub model: String,
    pub tokens: u32,
    pub cost: f64,
    pub latency_ms: u64,
}

pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    fn system(content: impl Into<String>) -> Self;
    fn user(content: impl Into<String>) -> Self;
    fn assistant(content: impl Into<String>) -> Self;
}
```

## Examples

### Simple Agent

```rust
use term_sdk::{Agent, Request, Response, run};

struct SimpleAgent;

impl Agent for SimpleAgent {
    fn solve(&mut self, req: &Request) -> Response {
        if req.is_first() { return Response::cmd("ls -la"); }
        if req.failed() { return Response::cmd("pwd"); }
        if req.has_any(&["hello", "world"]) { return Response::done(); }
        
        if req.instruction.to_lowercase().contains("file") {
            return Response::cmd("echo 'test' > test.txt");
        }
        
        Response::done()
    }
}

fn main() { run(&mut SimpleAgent); }
```

### LLM Agent

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

const SYSTEM: &str = r#"You are a terminal agent. Return JSON:
{"command": "shell command", "task_complete": false}
When done: {"command": null, "task_complete": true}"#;

struct LLMAgent { llm: LLM }

impl LLMAgent {
    fn new() -> Self {
        Self { llm: LLM::new("anthropic/claude-3-haiku") }
    }
}

impl Agent for LLMAgent {
    fn solve(&mut self, req: &Request) -> Response {
        let prompt = format!(
            "Task: {}\nStep: {}\nOutput: {:?}\nExit: {:?}",
            req.instruction, req.step, req.output, req.exit_code
        );
        
        match self.llm.ask_with_system(SYSTEM, &prompt) {
            Ok(r) => Response::from_llm(&r.text),
            Err(e) => {
                eprintln!("LLM error: {}", e);
                Response::done()
            }
        }
    }
    
    fn cleanup(&mut self) {
        eprintln!("Cost: ${:.4}", self.llm.total_cost);
    }
}

fn main() { run(&mut LLMAgent::new()); }
```

### With History

```rust
use term_sdk::{Agent, Request, Response, LLM, Message, run};

struct HistoryAgent {
    llm: LLM,
    history: Vec<Message>,
}

impl HistoryAgent {
    fn new() -> Self {
        Self {
            llm: LLM::new("anthropic/claude-3-haiku"),
            history: Vec::new(),
        }
    }
}

impl Agent for HistoryAgent {
    fn solve(&mut self, req: &Request) -> Response {
        self.history.push(Message::user(
            format!("Step {}: {}", req.step, req.output.as_deref().unwrap_or("start"))
        ));
        
        if self.history.len() > 10 {
            self.history = self.history[self.history.len()-10..].to_vec();
        }
        
        let mut messages = vec![Message::system(format!("Task: {}", req.instruction))];
        messages.extend(self.history.clone());
        
        match self.llm.chat(&messages) {
            Ok(r) => {
                self.history.push(Message::assistant(&r.text));
                Response::from_llm(&r.text)
            }
            Err(_) => Response::done()
        }
    }
}

fn main() { run(&mut HistoryAgent::new()); }
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
