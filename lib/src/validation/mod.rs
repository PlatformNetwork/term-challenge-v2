use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub challenge_id: ChallengeId,
    pub validator: Hotkey,
    pub is_valid: bool,
    pub reason: Option<String>,
}
