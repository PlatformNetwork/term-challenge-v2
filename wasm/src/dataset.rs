use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use platform_challenge_sdk_wasm::host_functions::{
    host_consensus_get_epoch, host_random_seed, host_storage_set,
};

use crate::types::{DatasetSelection, TaskDefinition};

const DATASET_SELECTION_PREFIX: &[u8] = b"dataset_selection:";
const TOTAL_SWE_BENCH_TASKS: usize = 2294;
const TASKS_TO_SELECT: usize = 100;
const CONSENSUS_DATASET_SIZE: usize = 50;

fn fnv1a_mix(data: &[u8]) -> [u8; 32] {
    let mut h0: u64 = 0xcbf29ce484222325;
    let mut h1: u64 = 0x100000001b3_u64.wrapping_mul(0xcbf29ce484222325);
    let mut h2: u64 = 0x6c62272e07bb0142;
    let mut h3: u64 = 0x62b821756295c58d;

    for &b in data {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(0x100000001b3);
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3).wrapping_add(h0);
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(0x100000001b3).wrapping_add(h1);
        h3 ^= b as u64;
        h3 = h3.wrapping_mul(0x100000001b3).wrapping_add(h2);
    }

    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&h0.to_le_bytes());
    out[8..16].copy_from_slice(&h1.to_le_bytes());
    out[16..24].copy_from_slice(&h2.to_le_bytes());
    out[24..32].copy_from_slice(&h3.to_le_bytes());
    out
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

pub fn select_random_task_indices() -> Vec<usize> {
    let mut seed = [0u8; 32];
    if host_random_seed(&mut seed).is_err() {
        return Vec::new();
    }

    let mut indices = Vec::with_capacity(TASKS_TO_SELECT);
    let mut state: u64 = u64::from_le_bytes([
        seed[0], seed[1], seed[2], seed[3], seed[4], seed[5], seed[6], seed[7],
    ]);

    while indices.len() < TASKS_TO_SELECT {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let idx = (state >> 33) as usize % TOTAL_SWE_BENCH_TASKS;
        if !indices.contains(&idx) {
            indices.push(idx);
        }
    }

    indices
}

pub fn store_my_selection(indices: &[usize]) -> bool {
    let epoch = host_consensus_get_epoch();
    if epoch < 0 {
        return false;
    }

    let mut key = Vec::with_capacity(DATASET_SELECTION_PREFIX.len() + 8);
    key.extend_from_slice(DATASET_SELECTION_PREFIX);
    key.extend_from_slice(&(epoch as u64).to_le_bytes());

    let data = match bincode::serialize(indices) {
        Ok(d) => d,
        Err(_) => return false,
    };
    host_storage_set(&key, &data).is_ok()
}

pub fn build_consensus_dataset(
    all_tasks: &[TaskDefinition],
    validator_selections: &[Vec<usize>],
) -> Vec<TaskDefinition> {
    if validator_selections.is_empty() || all_tasks.is_empty() {
        return Vec::new();
    }

    let threshold = validator_selections.len().div_ceil(2);
    let mut counts = alloc::collections::BTreeMap::new();

    for selection in validator_selections {
        for &idx in selection {
            *counts.entry(idx).or_insert(0usize) += 1;
        }
    }

    let mut consensus_indices: Vec<usize> = counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(idx, _)| idx)
        .collect();

    consensus_indices.sort_unstable();
    consensus_indices.truncate(CONSENSUS_DATASET_SIZE);

    consensus_indices
        .iter()
        .filter_map(|&idx| all_tasks.get(idx).cloned())
        .collect()
}

pub fn create_dataset_selection(tasks: Vec<TaskDefinition>) -> DatasetSelection {
    let epoch = host_consensus_get_epoch();

    let mut hash_input = Vec::new();
    for task in &tasks {
        hash_input.extend_from_slice(task.id.as_bytes());
        hash_input.push(b':');
        hash_input.extend_from_slice(task.name.as_bytes());
        hash_input.push(b';');
    }

    let hash_bytes = fnv1a_mix(&hash_input);
    let dataset_hash = bytes_to_hex(&hash_bytes);

    DatasetSelection {
        tasks,
        selected_at_epoch: if epoch >= 0 { epoch as u64 } else { 0 },
        dataset_hash,
    }
}
