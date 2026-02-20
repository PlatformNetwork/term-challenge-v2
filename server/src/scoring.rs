use platform_challenge_sdk::ChallengeDatabase;

use crate::types::{LeaderboardEntry, TopAgentState, DECAY_HALF_LIFE_EPOCHS, GRACE_PERIOD_EPOCHS};

pub fn calculate_aggregate_score(passed: u32, total: u32) -> f64 {
    if total == 0 {
        return 0.0;
    }
    passed as f64 / total as f64
}

pub fn apply_decay(score: f64, current_epoch: u64, state: &TopAgentState) -> f64 {
    if current_epoch <= state.epoch_set + state.grace_period {
        return score;
    }
    let elapsed = current_epoch - state.epoch_set - state.grace_period;
    let half_life = state.decay_half_life.max(1);
    let decay_factor = 0.5_f64.powf(elapsed as f64 / half_life as f64);
    score * decay_factor
}

pub fn get_top_agent_state(db: &ChallengeDatabase) -> Option<TopAgentState> {
    db.kv_get::<TopAgentState>("top_agent_state").ok().flatten()
}

pub fn set_top_agent_state(
    db: &ChallengeDatabase,
    state: &TopAgentState,
) -> Result<(), platform_challenge_sdk::ChallengeError> {
    db.kv_set("top_agent_state", state)
}

pub struct LeaderboardUpdate<'a> {
    pub hotkey: &'a str,
    pub score: f64,
    pub epoch: u64,
    pub submission_name: Option<&'a str>,
    pub tasks_passed: u32,
    pub tasks_total: u32,
    pub current_epoch: u64,
}

pub fn update_leaderboard(
    db: &ChallengeDatabase,
    update: &LeaderboardUpdate<'_>,
) -> Result<(), platform_challenge_sdk::ChallengeError> {
    let mut entries: Vec<LeaderboardEntry> = db
        .kv_get::<Vec<LeaderboardEntry>>("leaderboard")
        .ok()
        .flatten()
        .unwrap_or_default();

    let top_state = get_top_agent_state(db);

    if let Some(existing) = entries.iter_mut().find(|e| e.hotkey == update.hotkey) {
        existing.score = update.score;
        existing.epoch = update.epoch;
        existing.submission_name = update.submission_name.map(String::from);
        existing.tasks_passed = update.tasks_passed;
        existing.tasks_total = update.tasks_total;
        existing.decay_active = top_state
            .as_ref()
            .map(|s| {
                s.hotkey == update.hotkey && update.current_epoch > s.epoch_set + s.grace_period
            })
            .unwrap_or(false);
    } else {
        entries.push(LeaderboardEntry {
            rank: 0,
            hotkey: update.hotkey.to_string(),
            score: update.score,
            epoch: update.epoch,
            submission_name: update.submission_name.map(String::from),
            tasks_passed: update.tasks_passed,
            tasks_total: update.tasks_total,
            decay_active: false,
        });
    }

    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, entry) in entries.iter_mut().enumerate() {
        entry.rank = (i + 1) as u32;
    }

    if let Some(top) = entries.first() {
        let should_update = match &top_state {
            Some(state) => state.hotkey != top.hotkey,
            None => true,
        };
        if should_update {
            let new_state = TopAgentState {
                hotkey: top.hotkey.clone(),
                score: top.score,
                epoch_set: update.current_epoch,
                grace_period: GRACE_PERIOD_EPOCHS,
                decay_half_life: DECAY_HALF_LIFE_EPOCHS,
            };
            set_top_agent_state(db, &new_state)?;
        }
    }

    db.kv_set("leaderboard", &entries)?;

    let active_count = entries.len() as u64;
    db.kv_set("active_miner_count", &active_count)?;

    Ok(())
}
