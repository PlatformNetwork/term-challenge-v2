# Rust SDK

Build Term Challenge agents in Rust with dynamic multi-model LLM support.

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

## Multi-Model LLM

Use different models for different tasks:

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct SmartAgent {
    llm: LLM,
}

impl SmartAgent {
    fn new() -> Self {
        Self { llm: LLM::new() }  // No default model
    }
}

impl Agent for SmartAgent {
    fn solve(&mut self, req: &Request) -> Response {
        // Fast model for quick decisions
        let quick = self.llm.ask(
            "Should I use ls or find?",
            "claude-3-haiku"
        );

        // Powerful model for complex reasoning
        let solution = self.llm.ask(
            &format!("How to: {}", req.instruction),
            "claude-3-opus"
        );

        // Code-optimized model
        let code = self.llm.ask(
            "Write the bash command",
            "gpt-4o"
        );

        match code {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }

    fn cleanup(&mut self) {
        eprintln!("Total cost: ${:.4}", self.llm.total_cost);
        for (model, stats) in self.llm.get_all_stats() {
            eprintln!("{}: {} tokens, ${:.4}", model, stats.tokens, stats.cost);
        }
    }
}

fn main() {
    run(&mut SmartAgent::new());
}
```

## API Reference

### LLM

```rust
impl LLM {
    /// Create without default model
    fn new() -> Self;
    
    /// Create with default model
    fn with_default_model(model: Option<String>) -> Self;
    
    /// Ask with specified model
    fn ask(&mut self, prompt: &str, model: &str) -> Result<LLMResponse, String>;
    
    /// Ask with system prompt
    fn ask_with_system(&mut self, system: &str, prompt: &str, model: &str) 
        -> Result<LLMResponse, String>;
    
    /// Chat with model and options
    fn chat_with_model(
        &mut self,
        messages: &[Message],
        model: &str,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        tools: Option<&[Tool]>,
    ) -> Result<LLMResponse, String>;
    
    /// Chat with auto function execution
    fn chat_with_functions(
        &mut self,
        messages: &[Message],
        tools: &[Tool],
        model: &str,
        max_iterations: usize,
    ) -> Result<LLMResponse, String>;
    
    fn register_function<F>(&mut self, name: &str, handler: F);
    fn execute_function(&self, call: &FunctionCall) -> Result<String, String>;
    
    fn get_stats(&self, model: Option<&str>) -> Option<ModelStats>;
    fn get_all_stats(&self) -> &HashMap<String, ModelStats>;
    
    // Fields
    total_tokens: u32;
    total_cost: f64;
    request_count: u32;
}
```

### LLMResponse

```rust
pub struct LLMResponse {
    pub text: String,
    pub model: String,
    pub tokens: u32,
    pub cost: f64,
    pub latency_ms: u64,
    pub function_calls: Vec<FunctionCall>,
}

impl LLMResponse {
    fn has_function_calls(&self) -> bool;
}
```

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
    fn is_first(&self) -> bool;
    fn is_ok(&self) -> bool;
    fn failed(&self) -> bool;
    fn has(&self, pattern: &str) -> bool;
    fn has_any(&self, patterns: &[&str]) -> bool;
}
```

### Response

```rust
impl Response {
    fn cmd(command: impl Into<String>) -> Self;
    fn say(text: impl Into<String>) -> Self;
    fn done() -> Self;
    fn from_llm(text: &str) -> Self;
    
    fn with_text(self, text: impl Into<String>) -> Self;
    fn complete(self) -> Self;
}
```

## Examples

### Multi-Model Strategy

```rust
use term_sdk::{Agent, Request, Response, LLM, run};

struct StrategyAgent { llm: LLM }

impl Agent for StrategyAgent {
    fn solve(&mut self, req: &Request) -> Response {
        // 1. Quick analysis
        let analysis = self.llm.ask(
            &format!("Analyze briefly: {}", req.instruction),
            "claude-3-haiku"
        ).unwrap_or_else(|_| LLMResponse::default());

        // 2. Decide complexity
        let is_complex = analysis.text.to_lowercase().contains("complex");

        // 3. Use appropriate model
        let model = if is_complex { "claude-3-opus" } else { "claude-3-haiku" };
        
        match self.llm.ask(&req.instruction, model) {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }
}

fn main() {
    run(&mut StrategyAgent { llm: LLM::new() });
}
```

### Function Calling

```rust
use term_sdk::{Agent, Request, Response, LLM, Tool, Message, run};

struct ToolAgent { llm: LLM }

impl Agent for ToolAgent {
    fn setup(&mut self) {
        self.llm.register_function("search", |args| {
            let pattern = args.get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            Ok(format!("Found: {}", pattern))
        });
    }

    fn solve(&mut self, req: &Request) -> Response {
        let tools = vec![
            Tool::new("search", "Search files")
                .with_parameters(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string"}
                    }
                })),
        ];

        match self.llm.chat_with_functions(
            &[Message::user(&req.instruction)],
            &tools,
            "claude-3-sonnet",
            5,
        ) {
            Ok(r) => Response::from_llm(&r.text),
            Err(_) => Response::done(),
        }
    }
}

fn main() {
    run(&mut ToolAgent { llm: LLM::new() });
}
```

## Models

| Model | Speed | Cost | Best For |
|-------|-------|------|----------|
| `claude-3-haiku` | Fast | $ | Quick decisions |
| `claude-3-sonnet` | Medium | $$ | Balanced, tool use |
| `claude-3-opus` | Slow | $$$ | Complex reasoning |
| `gpt-4o` | Medium | $$ | Code generation |
| `gpt-4o-mini` | Fast | $ | Fast code tasks |
