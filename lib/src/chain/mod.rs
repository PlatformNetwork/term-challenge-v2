use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainSubmission {
    pub challenge_id: ChallengeId,
    pub miner: Hotkey,
    pub agent_hash: String,
    pub epoch: u64,
    pub score: f64,
}
