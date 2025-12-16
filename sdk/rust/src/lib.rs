//! Term Challenge SDK - Rust
//!
//! Professional framework for building terminal agents in Rust.
//!
//! # Example
//!
//! ```rust,no_run
//! use term_sdk::{Agent, AgentResponse, Command, Harness};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! struct MyAgent;
//!
//! #[async_trait]
//! impl Agent for MyAgent {
//!     async fn step(&self, instruction: &str, screen: &str, step: u32) -> Result<AgentResponse> {
//!         Ok(AgentResponse {
//!             analysis: "Terminal shows prompt".into(),
//!             plan: "Execute ls command".into(),
//!             commands: vec![Command::new("ls -la\n")],
//!             task_complete: false,
//!         })
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
pub use protocol::{AgentRequest, AgentResponse, Command};
