//! # term-core
//!
//! Core compatibility layer for the term-challenge crate. Re-exports canonical
//! types from [`platform_core`] and [`platform_challenge_sdk`] so that the rest
//! of the workspace has a single, stable import surface.

#![allow(deprecated)]

/// Internal module hierarchy — the public API is re-exported at the crate root.
pub mod core;

pub use self::core::compat;
