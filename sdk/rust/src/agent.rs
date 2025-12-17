//! Agent trait for Term Challenge.

use crate::{Request, Response};

/// Trait for Term Challenge agents.
///
/// Implement `solve()` to create your agent:
///
/// ```rust,no_run
/// use term_sdk::{Agent, Request, Response};
///
/// struct MyAgent;
///
/// impl Agent for MyAgent {
///     fn solve(&mut self, req: &Request) -> Response {
///         if req.step == 1 {
///             return Response::cmd("ls -la");
///         }
///         Response::done()
///     }
/// }
/// ```
pub trait Agent {
    /// Initialize the agent (optional).
    fn setup(&mut self) {}
    
    /// Process a request and return a response.
    fn solve(&mut self, request: &Request) -> Response;
    
    /// Clean up resources (optional).
    fn cleanup(&mut self) {}
}
