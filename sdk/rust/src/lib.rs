//! # Term SDK for Rust
//!
//! Build agents for Term Challenge.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use term_sdk::{Agent, Request, Response, run};
//!
//! struct MyAgent;
//!
//! impl Agent for MyAgent {
//!     fn solve(&mut self, req: &Request) -> Response {
//!         if req.step == 1 {
//!             return Response::cmd("ls -la");
//!         }
//!         if req.has("hello") {
//!             return Response::done();
//!         }
//!         Response::cmd("echo hello")
//!     }
//! }
//!
//! fn main() {
//!     run(&mut MyAgent);
//! }
//! ```
//!
//! ## With LLM
//!
//! ```rust,no_run
//! use term_sdk::{Agent, Request, Response, LLM, run};
//!
//! struct LLMAgent {
//!     llm: LLM,
//! }
//!
//! impl LLMAgent {
//!     fn new() -> Self {
//!         Self {
//!             llm: LLM::new("anthropic/claude-3-haiku"),
//!         }
//!     }
//! }
//!
//! impl Agent for LLMAgent {
//!     fn solve(&mut self, req: &Request) -> Response {
//!         let prompt = format!(
//!             "Task: {}\nOutput: {:?}\nReturn JSON: {{\"command\": \"...\", \"task_complete\": false}}",
//!             req.instruction, req.output
//!         );
//!         match self.llm.ask(&prompt) {
//!             Ok(resp) => Response::from_llm(&resp.text),
//!             Err(_) => Response::done(),
//!         }
//!     }
//! }
//!
//! fn main() {
//!     run(&mut LLMAgent::new());
//! }
//! ```

mod types;
mod agent;
mod runner;
mod llm;

pub use types::{Request, Response};
pub use agent::Agent;
pub use runner::run;
pub use llm::{LLM, LLMResponse, Provider};
