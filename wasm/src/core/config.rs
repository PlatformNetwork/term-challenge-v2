use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::types::{ChallengeId, ChallengeRoute, Hotkey};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeConfig {
    pub challenge_id: ChallengeId,
    pub name: String,
    pub version: String,
    pub owner: Hotkey,
    pub routes: Vec<ChallengeRoute>,
}

impl ChallengeConfig {
    pub fn new(challenge_id: ChallengeId, name: String, version: String, owner: Hotkey) -> Self {
        Self {
            challenge_id,
            name,
            version,
            owner,
            routes: Vec::new(),
        }
    }

    pub fn with_routes(mut self, routes: Vec<ChallengeRoute>) -> Self {
        self.routes = routes;
        self
    }
}
