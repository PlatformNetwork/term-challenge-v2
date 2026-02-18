use alloc::string::String;
use core::fmt::Write as _;
use platform_challenge_sdk_wasm::host_functions::{
    host_consensus_get_epoch, host_storage_get, host_storage_set,
};

use crate::types::{
    DecayParams, Difficulty, DifficultyStats, TaskDefinition, TaskResult, TopAgentState,
};

const TOP_AGENT_KEY: &[u8] = b"top_agent_state";
const GRACE_EPOCHS: u64 = 60;
const HALF_LIFE_EPOCHS: f64 = 20.0;

pub struct AggregateScore {
    pub tasks_passed: u32,
    pub tasks_failed: u32,
    pub pass_rate: f64,
    pub total_execution_time_ms: u64,
    pub easy_stats: DifficultyStats,
    pub medium_stats: DifficultyStats,
    pub hard_stats: DifficultyStats,
}

impl AggregateScore {
    pub fn total_tasks(&self) -> u32 {
        self.tasks_passed.saturating_add(self.tasks_failed)
    }
}

pub fn calculate_aggregate(tasks: &[TaskDefinition], results: &[TaskResult]) -> AggregateScore {
    let mut passed: u32 = 0;
    let mut failed: u32 = 0;
    let mut total_execution_time_ms: u64 = 0;
    let mut easy = DifficultyStats {
        total: 0,
        passed: 0,
    };
    let mut medium = DifficultyStats {
        total: 0,
        passed: 0,
    };
    let mut hard = DifficultyStats {
        total: 0,
        passed: 0,
    };

    for (task, result) in tasks.iter().zip(results.iter()) {
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
        }

        total_execution_time_ms = total_execution_time_ms.saturating_add(result.execution_time_ms);

        let stats = match task.difficulty {
            Difficulty::Easy => &mut easy,
            Difficulty::Medium => &mut medium,
            Difficulty::Hard => &mut hard,
        };
        stats.total += 1;
        if result.passed {
            stats.passed += 1;
        }
    }

    let total = passed + failed;
    let pass_rate = if total > 0 {
        passed as f64 / total as f64
    } else {
        0.0
    };

    AggregateScore {
        tasks_passed: passed,
        tasks_failed: failed,
        pass_rate,
        total_execution_time_ms,
        easy_stats: easy,
        medium_stats: medium,
        hard_stats: hard,
    }
}

/// Convert aggregate score to weight (normalized 0.0-1.0).
pub fn to_weight(score: &AggregateScore) -> f64 {
    score.pass_rate.clamp(0.0, 1.0)
}

/// Apply decay to weight based on hours since top score.
pub fn apply_decay(weight: f64, hours_since_top: f64, params: &DecayParams) -> f64 {
    let grace = params.grace_period_hours as f64;
    if hours_since_top <= grace {
        return weight;
    }

    let elapsed = hours_since_top - grace;
    let half_life = params.half_life_hours as f64;
    if half_life <= 0.0 {
        return params.min_multiplier;
    }

    let multiplier = 0.5f64.powf(elapsed / half_life);
    let clamped = multiplier.max(params.min_multiplier);
    weight * clamped
}

pub fn format_summary(score: &AggregateScore) -> String {
    let mut msg = String::new();
    let _ = write!(
        msg,
        "passed={}/{} rate={:.2}%",
        score.tasks_passed,
        score.total_tasks(),
        score.pass_rate * 100.0,
    );
    if score.easy_stats.total > 0 {
        let _ = write!(
            msg,
            " easy={}/{}",
            score.easy_stats.passed, score.easy_stats.total,
        );
    }
    if score.medium_stats.total > 0 {
        let _ = write!(
            msg,
            " med={}/{}",
            score.medium_stats.passed, score.medium_stats.total,
        );
    }
    if score.hard_stats.total > 0 {
        let _ = write!(
            msg,
            " hard={}/{}",
            score.hard_stats.passed, score.hard_stats.total,
        );
    }
    let _ = write!(msg, " time={}ms", score.total_execution_time_ms);
    msg
}

pub fn get_top_agent_state() -> Option<TopAgentState> {
    let data = host_storage_get(TOP_AGENT_KEY).ok()?;
    if data.is_empty() {
        return None;
    }
    bincode::deserialize(&data).ok()
}

pub fn update_top_agent_state(agent_hash: &str, score: f64, epoch: u64) -> bool {
    let current = get_top_agent_state();
    let should_update = match &current {
        Some(state) => score > state.score,
        None => true,
    };

    if should_update {
        let state = TopAgentState {
            agent_hash: String::from(agent_hash),
            score,
            achieved_epoch: epoch,
            epochs_stale: 0,
            decay_active: false,
            current_burn_percent: 0.0,
        };
        if let Ok(data) = bincode::serialize(&state) {
            return host_storage_set(TOP_AGENT_KEY, &data).is_ok();
        }
    } else if let Some(mut state) = current {
        let current_epoch = host_consensus_get_epoch();
        if current_epoch >= 0 {
            state.epochs_stale = (current_epoch as u64).saturating_sub(state.achieved_epoch);
            state.decay_active = state.epochs_stale > GRACE_EPOCHS;
            if state.decay_active {
                let decay_epochs = state.epochs_stale.saturating_sub(GRACE_EPOCHS);
                let multiplier = 0.5f64.powf(decay_epochs as f64 / HALF_LIFE_EPOCHS);
                state.current_burn_percent = (1.0 - multiplier) * 100.0;
            }
            if let Ok(data) = bincode::serialize(&state) {
                let _ = host_storage_set(TOP_AGENT_KEY, &data);
            }
        }
    }
    false
}

pub fn apply_epoch_decay(weight: f64, params: &DecayParams) -> f64 {
    if let Some(state) = get_top_agent_state() {
        if state.decay_active {
            let multiplier = 1.0 - (state.current_burn_percent / 100.0);
            return weight * multiplier.max(params.min_multiplier);
        }
    }
    weight
}
