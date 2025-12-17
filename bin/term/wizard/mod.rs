//! Interactive Wizard for Term Challenge
//!
//! A beautiful TUI wizard that guides miners through:
//! 1. Agent selection
//! 2. Validation & testing
//! 3. API key configuration
//! 4. Secure submission

pub mod components;
pub mod state;
pub mod submit_wizard;

pub use submit_wizard::run_submit_wizard;
