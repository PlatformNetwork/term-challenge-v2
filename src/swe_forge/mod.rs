//! SWE-Forge integration module
//!
//! Communicates with term-executor workers running on Basilica miner nodes
//! for SWE-Forge evaluation tasks. Replaces the previous Docker-based
//! evaluation pipeline.

pub mod client;
pub mod types;

pub use client::SweForgeClient;
pub use types::{BatchResult, BatchStatus, SubmitResponse, SweForgeTaskResult, TaskStatus};
