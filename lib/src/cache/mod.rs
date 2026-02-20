use platform_challenge_sdk::ChallengeId;
use platform_core::Hotkey;
use std::collections::HashMap;

pub struct ScoreCache {
    scores: HashMap<(ChallengeId, Hotkey), f64>,
}

impl ScoreCache {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
        }
    }

    pub fn get(&self, challenge_id: &ChallengeId, hotkey: &Hotkey) -> Option<f64> {
        self.scores.get(&(*challenge_id, hotkey.clone())).copied()
    }

    pub fn insert(&mut self, challenge_id: ChallengeId, hotkey: Hotkey, score: f64) {
        self.scores.insert((challenge_id, hotkey), score);
    }
}

impl Default for ScoreCache {
    fn default() -> Self {
        Self::new()
    }
}
