use platform_challenge_sdk::ChallengeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetEntry {
    pub challenge_id: ChallengeId,
    pub task_ids: Vec<String>,
    pub selected_at_epoch: u64,
    pub dataset_hash: String,
}

impl DatasetEntry {
    pub fn new(challenge_id: ChallengeId, task_ids: Vec<String>, epoch: u64, hash: String) -> Self {
        Self {
            challenge_id,
            task_ids,
            selected_at_epoch: epoch,
            dataset_hash: hash,
        }
    }
}
