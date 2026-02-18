use alloc::string::String;
use alloc::vec::Vec;
use platform_challenge_sdk_wasm::host_functions::{
    host_random_seed, host_storage_get, host_storage_set,
};

use crate::types::DatasetSelection;

const DATASET_PROPOSALS_KEY: &[u8] = b"dataset_proposals";

pub fn propose_task_indices(validator_id: &str, indices: &[u32]) -> bool {
    let mut proposals: Vec<(String, Vec<u32>)> = host_storage_get(DATASET_PROPOSALS_KEY)
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode::deserialize(&d).ok()
            }
        })
        .unwrap_or_default();

    if let Some(pos) = proposals.iter().position(|(v, _)| v == validator_id) {
        proposals[pos].1 = indices.to_vec();
    } else {
        proposals.push((String::from(validator_id), indices.to_vec()));
    }

    if let Ok(data) = bincode::serialize(&proposals) {
        return host_storage_set(DATASET_PROPOSALS_KEY, &data).is_ok();
    }
    false
}

pub fn check_dataset_consensus() -> Option<Vec<u32>> {
    let proposals: Vec<(String, Vec<u32>)> = host_storage_get(DATASET_PROPOSALS_KEY)
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode::deserialize(&d).ok()
            }
        })
        .unwrap_or_default();

    if proposals.is_empty() {
        return None;
    }

    let validator_count = proposals.len();
    let threshold = (validator_count / 2) + 1;

    let mut counts: Vec<(Vec<u32>, usize)> = Vec::new();
    for (_, indices) in &proposals {
        let mut sorted = indices.clone();
        sorted.sort();
        if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == sorted) {
            entry.1 += 1;
        } else {
            counts.push((sorted, 1));
        }
    }

    for (indices, count) in counts {
        if count >= threshold {
            return Some(indices);
        }
    }
    None
}

pub fn generate_random_indices(total_tasks: u32, select_count: u32) -> Vec<u32> {
    let mut seed = [0u8; 32];
    let _ = host_random_seed(&mut seed);

    let count = select_count.min(total_tasks) as usize;
    let mut indices = Vec::with_capacity(count);
    let mut used = Vec::new();

    for i in 0..count {
        let idx_bytes = if i * 4 + 4 <= seed.len() {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&seed[i * 4..i * 4 + 4]);
            u32::from_le_bytes(buf)
        } else {
            seed[i % seed.len()] as u32
        };

        let mut idx = idx_bytes % total_tasks;
        let mut attempts = 0;
        while used.contains(&idx) && attempts < total_tasks {
            idx = (idx + 1) % total_tasks;
            attempts += 1;
        }
        if !used.contains(&idx) {
            used.push(idx);
            indices.push(idx);
        }
    }
    indices
}

pub fn get_dataset_history() -> Vec<DatasetSelection> {
    host_storage_get(b"dataset_history")
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode::deserialize(&d).ok()
            }
        })
        .unwrap_or_default()
}
