//! Task definitions and registry.

pub mod challenge;
pub mod config;
pub mod harness;
pub mod registry;

// Re-export commonly used types from config for convenience
pub use config::{
    AddTaskRequest, Difficulty, Task, TaskConfig, TaskDescription, TaskInfo, TaskRegistry,
    TaskResult,
};
