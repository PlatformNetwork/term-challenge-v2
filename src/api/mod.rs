//! REST API implementation.

pub mod errors;
pub mod llm;
pub mod middleware;
pub mod routes;
pub mod state;
pub mod types;

// Re-export state for convenience
pub use state::ApiState;

// Re-export key types from routes for backward compatibility
pub use routes::CompletedTaskInfo;
