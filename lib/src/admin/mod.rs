use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminAction {
    pub challenge_id: ChallengeId,
    pub issuer: Hotkey,
    pub action: String,
    pub payload: serde_json::Value,
}
