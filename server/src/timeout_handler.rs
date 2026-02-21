use platform_challenge_sdk::ChallengeDatabase;
use rand::Rng;

use crate::types::{ReviewAssignment, TimeoutConfig};

fn current_block_number() -> i64 {
    chrono::Utc::now().timestamp() / 10
}

pub fn get_timeout_config(db: &ChallengeDatabase) -> TimeoutConfig {
    db.kv_get::<TimeoutConfig>("timeout_config")
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn set_timeout_config(db: &ChallengeDatabase, config: &TimeoutConfig) -> bool {
    db.kv_set("timeout_config", config).is_ok()
}

fn assignment_key(submission_id: &str, validator: &str, review_type: &str) -> String {
    format!("timeout:{}:{}:{}", submission_id, validator, review_type)
}

pub fn record_assignment(
    db: &ChallengeDatabase,
    submission_id: &str,
    validator: &str,
    review_type: &str,
) -> bool {
    let current_block = current_block_number();
    let assignment = ReviewAssignment {
        submission_id: submission_id.to_string(),
        validator: validator.to_string(),
        review_type: review_type.to_string(),
        assigned_at_block: current_block,
        timed_out: false,
    };
    let key = assignment_key(submission_id, validator, review_type);
    db.kv_set(&key, &assignment).is_ok()
}

pub fn check_timeout(
    db: &ChallengeDatabase,
    submission_id: &str,
    validator: &str,
    review_type: &str,
    timeout_blocks: u64,
) -> bool {
    let key = assignment_key(submission_id, validator, review_type);
    let assignment = db.kv_get::<ReviewAssignment>(&key).ok().flatten();

    match assignment {
        Some(a) => {
            let current_block = current_block_number();
            let elapsed_blocks = (current_block - a.assigned_at_block) as u64;
            elapsed_blocks >= timeout_blocks
        }
        None => false,
    }
}

pub fn mark_timed_out(
    db: &ChallengeDatabase,
    submission_id: &str,
    validator: &str,
    review_type: &str,
) -> bool {
    let key = assignment_key(submission_id, validator, review_type);
    let assignment = db.kv_get::<ReviewAssignment>(&key).ok().flatten();

    match assignment {
        Some(mut a) => {
            a.timed_out = true;
            db.kv_set(&key, &a).is_ok()
        }
        None => false,
    }
}

pub fn select_replacement(
    validators: &[String],
    excluded: &[String],
    seed: &[u8],
) -> Option<String> {
    let eligible: Vec<&String> = validators
        .iter()
        .filter(|v| !excluded.contains(v))
        .collect();

    if eligible.is_empty() {
        return None;
    }

    let seed_val = seed
        .iter()
        .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));

    let mut rng = rand::thread_rng();
    let idx = if seed_val > 0 {
        (seed_val as usize) % eligible.len()
    } else {
        rng.gen_range(0..eligible.len())
    };

    Some(eligible[idx].clone())
}
