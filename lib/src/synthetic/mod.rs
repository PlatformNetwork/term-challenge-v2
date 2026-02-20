use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyntheticTask {
    pub challenge_id: ChallengeId,
    pub owner: Hotkey,
    pub task_data: Vec<u8>,
    pub created_epoch: u64,
}
