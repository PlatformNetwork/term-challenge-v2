#![allow(dead_code, unused_variables, unused_imports)]
//! Subnet Manager - Hot Updates & Fault Tolerance
//!
//! Provides:
//! - Hot code updates (WASM challenges)
//! - State snapshots and recovery
//! - Hard reset capability
//! - Health monitoring
//! - Graceful degradation

pub mod commands;
pub mod config;
pub mod health;
pub mod recovery;
pub mod snapshot;
pub mod update;

pub use commands::*;
pub use config::*;
pub use health::*;
pub use recovery::*;
pub use snapshot::*;
pub use update::*;
