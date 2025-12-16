//! Harness for running agents in Term Challenge.

use crate::protocol::{AgentRequest, AgentResponse};
use anyhow::Result;
use async_trait::async_trait;
use std::io::{self, BufRead, Write};
use tracing::{debug, error, info};

/// Base trait for Term Challenge agents.
///
/// Implement this trait to create your agent.
///
/// # Example
///
/// ```rust,no_run
/// use term_sdk::{Agent, AgentResponse, Command};
/// use async_trait::async_trait;
/// use anyhow::Result;
///
/// struct MyAgent {
///     // Your agent state
/// }
///
/// #[async_trait]
/// impl Agent for MyAgent {
///     async fn setup(&mut self) -> Result<()> {
///         // Initialize resources
///         Ok(())
///     }
///
///     async fn step(&self, instruction: &str, screen: &str, step: u32) -> Result<AgentResponse> {
///         // Your agent logic
///         Ok(AgentResponse::new()
///             .with_analysis("Analyzed terminal")
///             .with_plan("Execute command")
///             .add_command(Command::new("ls\n")))
///     }
///
///     async fn cleanup(&mut self) -> Result<()> {
///         // Release resources
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Agent: Send + Sync {
    /// Initialize the agent. Override to set up resources.
    async fn setup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Process one step of the task.
    ///
    /// # Arguments
    ///
    /// * `instruction` - The task instruction/goal.
    /// * `screen` - Current terminal screen content.
    /// * `step` - Current step number (1-indexed).
    ///
    /// # Returns
    ///
    /// `AgentResponse` with analysis, plan, commands, and task_complete flag.
    async fn step(&self, instruction: &str, screen: &str, step: u32) -> Result<AgentResponse>;

    /// Clean up resources. Override to release resources.
    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Runs an agent in the Term Challenge harness.
///
/// The harness handles:
/// - Reading requests from stdin
/// - Calling the agent's step method
/// - Writing responses to stdout
/// - Error handling and logging
///
/// # Example
///
/// ```rust,no_run
/// use term_sdk::{Agent, Harness};
///
/// # struct MyAgent;
/// # #[async_trait::async_trait]
/// # impl Agent for MyAgent {
/// #     async fn step(&self, _: &str, _: &str, _: u32) -> anyhow::Result<term_sdk::AgentResponse> {
/// #         Ok(term_sdk::AgentResponse::default())
/// #     }
/// # }
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let agent = MyAgent;
///     Harness::new(agent).run().await
/// }
/// ```
pub struct Harness<A: Agent> {
    agent: A,
}

impl<A: Agent> Harness<A> {
    /// Create a new harness with the given agent.
    pub fn new(agent: A) -> Self {
        Self { agent }
    }

    /// Run the agent loop.
    ///
    /// This is the main entry point. It reads from stdin, processes requests,
    /// and writes responses to stdout.
    pub async fn run(mut self) -> Result<()> {
        // Setup
        info!("Setting up agent...");
        if let Err(e) = self.agent.setup().await {
            error!("Setup failed: {}", e);
            self.send_response(&AgentResponse::error(format!("Setup failed: {}", e)));
            return Err(e);
        }
        info!("Agent ready");

        // Process loop
        let result = self.process_loop().await;

        // Cleanup
        if let Err(e) = self.agent.cleanup().await {
            error!("Cleanup error: {}", e);
        }

        result
    }

    async fn process_loop(&self) -> Result<()> {
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = self.process_request(trimmed).await;
            self.send_response(&response);
        }

        Ok(())
    }

    async fn process_request(&self, line: &str) -> AgentResponse {
        // Parse request
        let request: AgentRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                error!("Invalid JSON: {}", e);
                return AgentResponse::error(format!("Invalid JSON: {}", e));
            }
        };

        debug!("Step {}: Processing...", request.step);

        // Call agent
        match self.agent.step(&request.instruction, &request.screen, request.step).await {
            Ok(response) => {
                debug!("Step {}: Complete (task_complete={})", request.step, response.task_complete);
                response
            }
            Err(e) => {
                error!("Agent error at step {}: {}", request.step, e);
                AgentResponse::error(format!("Agent error: {}", e))
            }
        }
    }

    fn send_response(&self, response: &AgentResponse) {
        match serde_json::to_string(response) {
            Ok(json) => {
                println!("{}", json);
                let _ = io::stdout().flush();
            }
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                println!(r#"{{"analysis":"Error: {}","plan":"","commands":[],"task_complete":false}}"#, e);
                let _ = io::stdout().flush();
            }
        }
    }
}

/// Convenience function to run an agent.
///
/// Equivalent to `Harness::new(agent).run().await`.
pub async fn run<A: Agent>(agent: A) -> Result<()> {
    Harness::new(agent).run().await
}
