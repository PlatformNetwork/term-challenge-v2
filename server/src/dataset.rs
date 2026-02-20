use std::collections::BTreeMap;

use platform_challenge_sdk::ChallengeDatabase;
use rand::Rng;

use crate::types::{DatasetConsensusResult, DatasetHistoryEntry};

const CONSENSUS_THRESHOLD_PERCENT: usize = 50;

pub fn propose_task_indices(db: &ChallengeDatabase, validator_id: &str, indices: &[u32]) -> bool {
    let key = "dataset_proposals";
    let mut proposals: BTreeMap<String, Vec<u32>> = db
        .kv_get::<BTreeMap<String, Vec<u32>>>(key)
        .ok()
        .flatten()
        .unwrap_or_default();

    proposals.insert(validator_id.to_string(), indices.to_vec());
    db.kv_set(key, &proposals).is_ok()
}

pub fn check_dataset_consensus(db: &ChallengeDatabase) -> DatasetConsensusResult {
    let proposals: BTreeMap<String, Vec<u32>> = db
        .kv_get::<BTreeMap<String, Vec<u32>>>("dataset_proposals")
        .ok()
        .flatten()
        .unwrap_or_default();

    let validator_count = proposals.len();
    let threshold = (validator_count * CONSENSUS_THRESHOLD_PERCENT) / 100 + 1;

    let mut index_votes: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    for indices in proposals.values() {
        let mut sorted = indices.clone();
        sorted.sort();
        *index_votes.entry(sorted).or_insert(0) += 1;
    }

    let best = index_votes.iter().max_by_key(|(_, count)| *count);

    match best {
        Some((indices, count)) if *count >= threshold => DatasetConsensusResult {
            consensus_reached: true,
            agreed_indices: indices.clone(),
            proposals,
            threshold,
        },
        _ => DatasetConsensusResult {
            consensus_reached: false,
            agreed_indices: Vec::new(),
            proposals,
            threshold,
        },
    }
}

pub fn get_dataset_history(db: &ChallengeDatabase) -> Vec<DatasetHistoryEntry> {
    db.kv_get::<Vec<DatasetHistoryEntry>>("dataset_history")
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn generate_random_indices(total_tasks: u32, select_count: u32) -> Vec<u32> {
    if total_tasks == 0 || select_count == 0 {
        return Vec::new();
    }
    let count = select_count.min(total_tasks);
    let mut rng = rand::thread_rng();
    let mut indices: Vec<u32> = (0..total_tasks).collect();

    for i in (1..indices.len()).rev() {
        let j = rng.gen_range(0..=i);
        indices.swap(i, j);
    }

    indices.truncate(count as usize);
    indices.sort();
    indices
}
