//! Route definitions for the term-challenge module
//!
//! These routes are designed to be queried via platform-v2's `challenge_call` RPC method.
//! When the challenge SDK's route integration is complete, these routes will be registered
//! automatically. Until then, validators can access this data via direct storage queries.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::types::RouteDefinition;

/// Returns route definitions for the term-challenge module.
///
/// Note: This function is currently unused pending integration with platform-v2's
/// challenge route registration system. The routes are defined here for documentation
/// and future automatic registration.
#[allow(dead_code)]
pub fn get_route_definitions() -> Vec<RouteDefinition> {
    vec![
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/leaderboard"),
            description: String::from(
                "Returns current leaderboard with scores, miner hotkeys, and ranks",
            ),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions"),
            description: String::from("Returns pending submissions awaiting evaluation"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions/:id"),
            description: String::from("Returns specific submission status"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset"),
            description: String::from("Returns current active dataset of 50 SWE-bench tasks"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset/history"),
            description: String::from("Returns historical dataset selections"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/submit"),
            description: String::from("Submission endpoint: receives zip package and metadata"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/decay"),
            description: String::from("Returns current decay status for top agents"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/stats"),
            description: String::from("Challenge statistics: total submissions, active miners"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/code"),
            description: String::from("Returns stored agent code package for a miner"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/logs"),
            description: String::from("Returns evaluation logs for a miner"),
        },
    ]
}
