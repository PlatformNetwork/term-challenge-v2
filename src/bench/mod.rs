//! Terminal-Bench Integration Module
//!
//! This module provides support for running Terminal-Bench 2.0 tasks locally.
//! It handles downloading datasets, managing Docker environments, running agents,
//! and verifying results.

pub mod agent;
pub mod environment;
pub mod external_agent;
pub mod llm;
pub mod registry;
pub mod results;
pub mod runner;
pub mod session;
pub mod task;
pub mod verifier;

pub use agent::{create_agent, LlmAgent};
pub use environment::DockerEnvironment;
pub use external_agent::{create_external_agent, AgentLanguage, ExternalAgent};
pub use llm::{CostTracker, LlmClient, Message, Provider};
pub use registry::{Dataset, RegistryClient, TaskSource};
pub use results::{BenchmarkResults, ResultExporter, TaskResult};
pub use runner::{Agent, TrialConfig, TrialResult, TrialRunner};
pub use session::TmuxSession;
pub use task::{Task, TaskConfig};
pub use verifier::Verifier;
