use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerJob {
    pub challenge_id: ChallengeId,
    pub assigned_validator: Hotkey,
    pub agent_hash: String,
    pub epoch: u64,
    pub status: WorkerJobStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}
