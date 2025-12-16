# Term SDK for Rust

Build AI agents for the Terminal Benchmark Challenge.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
term-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use term_sdk::{Agent, AgentResponse, response, run, Error};
use async_trait::async_trait;

struct MyAgent;

#[async_trait]
impl Agent for MyAgent {
    async fn step(
        &mut self,
        task: &str,
        terminal_state: &str,
    ) -> Result<AgentResponse, Error> {
        // Analyze and plan
        Ok(response()
            .analysis("I see the terminal prompt")
            .plan("List files to understand structure")
            .run("ls -la")
            .build())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut agent = MyAgent;
    let response = agent.solve("Create a hello world script", "", 50).await?;
    
    println!("Task complete: {}", response.task_complete);
    Ok(())
}
```

## Agent Response Format

```rust
use term_sdk::{AgentResponse, Command, response, run, keys};

// Using builder pattern
let response = response()
    .analysis("Analyzing current state...")
    .plan("1) List files, 2) Create script")
    .run("ls -la")           // Command with \n appended
    .keys("vim hello.py")    // Quick keystroke
    .build();

// Or directly
let response = AgentResponse::with_commands(
    "Analysis text",
    "Plan text",
    vec![
        Command::run("ls -la", 0.1),
        Command::run("cat file.txt", 0.5),
    ],
);

// Mark task complete
let done = AgentResponse::complete("Task finished successfully");
```

## Terminal Interface

```rust
use term_sdk::{Terminal, special_keys};

let terminal = Terminal::simulated();

// Run commands
let result = terminal.run("ls -la", true, Some(5.0)).await?;
println!("Output: {}", result.output);

// Send keystrokes (for interactive programs)
terminal.send_keys(&["vim", "Enter"]).await?;
terminal.send_keys(&["i", "print('hello')"]).await?;
terminal.send_keys(&[special_keys::ESCAPE, ":wq", "Enter"]).await?;

// Capture screen
let screen = terminal.capture_screen(false).await?;
```

## LLM Integration

```rust
use term_sdk::{LlmClient, Message, Provider};

let llm = LlmClient::new(
    Provider::OpenRouter,
    "your-api-key",
    10.0  // $10 cost limit
);

let response = llm.chat(
    vec![
        Message::system("You are a terminal expert."),
        Message::user("How do I list files?"),
    ],
    "openai/gpt-4o-mini",
).await?;

println!("Response: {}", response.content);
println!("Cost: ${:.4}", response.cost);
```

## Full Agent Example

```rust
use term_sdk::{
    Agent, AgentResponse, LlmClient, Provider, Message,
    response, run, Error
};
use async_trait::async_trait;

struct LlmPoweredAgent {
    llm: LlmClient,
    episode: usize,
}

impl LlmPoweredAgent {
    fn new(api_key: &str) -> Self {
        Self {
            llm: LlmClient::new(Provider::OpenRouter, api_key, 10.0),
            episode: 0,
        }
    }
}

#[async_trait]
impl Agent for LlmPoweredAgent {
    async fn setup(&mut self) -> Result<(), Error> {
        println!("Agent starting with ${:.2} budget", self.llm.remaining_budget());
        Ok(())
    }

    async fn step(
        &mut self,
        task: &str,
        terminal_state: &str,
    ) -> Result<AgentResponse, Error> {
        self.episode += 1;
        
        // Ask LLM for next action
        let llm_response = self.llm.chat(
            vec![
                Message::system("You are a terminal expert. Respond with the next command."),
                Message::user(format!("Task: {}\n\nTerminal:\n{}", task, terminal_state)),
            ],
            "openai/gpt-4o-mini",
        ).await?;

        // Check if we should complete
        if llm_response.content.contains("DONE") || self.episode > 20 {
            return Ok(AgentResponse::complete("Task completed"));
        }

        Ok(response()
            .analysis(&format!("Episode {}", self.episode))
            .plan(&llm_response.content)
            .run(&llm_response.content)
            .build())
    }

    async fn teardown(&mut self) -> Result<(), Error> {
        println!("Total cost: ${:.4}", self.llm.total_cost());
        Ok(())
    }
}
```

## Special Keys

```rust
use term_sdk::special_keys;

// Available special keys
special_keys::ENTER      // Enter key
special_keys::ESCAPE     // Escape key
special_keys::TAB        // Tab key
special_keys::BACKSPACE  // Backspace
special_keys::CTRL_C     // Ctrl+C
special_keys::CTRL_D     // Ctrl+D
special_keys::CTRL_Z     // Ctrl+Z
special_keys::UP         // Arrow up
special_keys::DOWN       // Arrow down
```

## Protocol Keys (for keystrokes string)

```rust
use term_sdk::keys;

// Use in Command keystrokes
let cmd = Command::new(keys::CTRL_C, 0.1);  // Send Ctrl+C
let cmd = Command::new(keys::ESCAPE, 0.1);  // Send Escape
```

## Error Handling

```rust
use term_sdk::{Error, Result};

async fn my_agent_step() -> Result<AgentResponse> {
    // Errors automatically propagate
    let response = llm.chat(...).await?;
    
    // Or handle specific errors
    match result {
        Err(Error::CostLimitExceeded(msg)) => {
            println!("Budget exceeded: {}", msg);
        }
        Err(Error::Provider(msg)) => {
            println!("API error: {}", msg);
        }
        _ => {}
    }
    
    Ok(response)
}
```

## License

Apache-2.0
