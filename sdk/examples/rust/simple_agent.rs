//! Simple Term Challenge Agent Example (Rust)
//!
//! This agent uses GPT-4o-mini to solve terminal tasks step by step.

use async_trait::async_trait;
use std::env;
use term_sdk::{
    Agent, AgentResponse, Error, LlmClient, Message, Provider,
    response, run,
};

const SYSTEM_PROMPT: &str = r#"You are an expert Linux terminal user.
You receive a task and the current terminal state.
Respond with the NEXT SINGLE COMMAND to execute.
Just output the command, nothing else.
If the task is complete, respond with: DONE"#;

struct SimpleAgent {
    llm: LlmClient,
}

impl SimpleAgent {
    fn new() -> Result<Self, Error> {
        let api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| Error::Configuration("OPENROUTER_API_KEY not set".to_string()))?;
        
        Ok(Self {
            llm: LlmClient::new(Provider::OpenRouter, &api_key, 5.0), // $5 budget
        })
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    async fn step(&mut self, task: &str, terminal_state: &str) -> Result<AgentResponse, Error> {
        // Build prompt
        let user_message = format!(
            r#"Task: {}

Current terminal output:
```
{}
```

What is the next command? (or DONE if finished)"#,
            task,
            &terminal_state[terminal_state.len().saturating_sub(2000)..]
        );

        // Ask LLM
        let response = self.llm.chat(
            vec![
                Message::system(SYSTEM_PROMPT),
                Message::user(&user_message),
            ],
            "openai/gpt-4o-mini",
        ).await?;

        let command = response.content.trim();

        // Check if task is complete
        if command.to_uppercase().contains("DONE") {
            return Ok(AgentResponse::complete(format!(
                "Task appears complete. Terminal shows: {}",
                &terminal_state[terminal_state.len().saturating_sub(200)..]
            )));
        }

        // Execute the command
        Ok(response()
            .analysis(format!("LLM suggested: {}", command))
            .plan(format!("Execute: {}", command))
            .command(run(command))
            .build())
    }

    async fn teardown(&mut self) -> Result<(), Error> {
        println!("\nTotal LLM cost: ${:.4}", self.llm.total_cost());
        println!("Remaining budget: ${:.2}", self.llm.remaining_budget());
        Ok(())
    }

    fn name(&self) -> &str {
        "simple-agent"
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut agent = SimpleAgent::new()?;
    
    let task = "Create a Python script that prints 'Hello World' and run it";
    let mut terminal_state = String::from("user@sandbox:~$ ");

    println!("Task: {}", task);
    println!("{}", "-".repeat(50));

    for i in 0..10 {
        let response = agent.step(task, &terminal_state).await?;

        println!("\nStep {}:", i + 1);
        println!("  Analysis: {}...", &response.analysis[..response.analysis.len().min(80)]);
        println!("  Plan: {}", response.plan);
        println!("  Commands: {:?}", response.commands.iter().map(|c| &c.keystrokes).collect::<Vec<_>>());
        println!("  Complete: {}", response.task_complete);

        if response.task_complete {
            println!("\n✓ Task completed!");
            break;
        }

        // Simulate terminal output
        for cmd in &response.commands {
            terminal_state.push_str(&format!(
                "\n{}\n[simulated output]",
                cmd.keystrokes.trim()
            ));
        }
    }

    agent.teardown().await?;
    Ok(())
}
