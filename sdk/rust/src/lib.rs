//! Term Challenge SDK - Rust
//!
//! Professional framework for building terminal agents in Rust.
//!
//! # Example
//!
//! ```rust,no_run
//! use term_sdk::{Agent, AgentResponse, Command, Harness, log};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! struct MyAgent;
//!
//! #[async_trait]
//! impl Agent for MyAgent {
//!     async fn step(&self, instruction: &str, screen: &str, step: u32) -> Result<AgentResponse> {
//!         log::info("Processing step");
//!         
//!         // Your logic here...
//!         
//!         log::success("Generated response");
//!         Ok(AgentResponse::new()
//!             .with_analysis("Terminal shows prompt")
//!             .with_plan("Execute ls command")
//!             .add_command(Command::new("ls -la\n"))
//!             .with_logs())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     Harness::new(MyAgent).run().await
//! }
//! ```

pub mod harness;
pub mod llm;
pub mod protocol;

pub use harness::{Agent, Harness};
pub use llm::{LLMClient, Provider, ChatResponse, CostTracker, Message};
pub use protocol::{log, AgentLogger, AgentRequest, AgentResponse, Command, LogEntry, LogLevel, LOGGER};
