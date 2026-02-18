use alloc::vec::Vec;
use bincode::Options;
use platform_challenge_sdk_wasm::host_functions::{host_storage_get, host_storage_set};

use crate::types::{DatasetSelection, TaskDefinition};

const ACTIVE_DATASET_KEY: &[u8] = b"active_dataset";
const DATASET_HISTORY_KEY: &[u8] = b"dataset_history";
const MAX_DATASET_SIZE: u64 = 4 * 1024 * 1024;
const MAX_HISTORY_SIZE: u64 = 16 * 1024 * 1024;
const MAX_DATASET_HISTORY: usize = 100;

fn bincode_options_dataset() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_DATASET_SIZE)
        .with_fixint_encoding()
}

fn bincode_options_history() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_HISTORY_SIZE)
        .with_fixint_encoding()
}

/// Retrieve the currently active task dataset from host storage.
pub fn get_active_dataset() -> Option<Vec<TaskDefinition>> {
    let data = host_storage_get(ACTIVE_DATASET_KEY).ok()?;
    if data.is_empty() {
        return None;
    }
    let selection: DatasetSelection = bincode_options_dataset().deserialize(&data).ok()?;
    Some(selection.tasks)
}

/// Persist a dataset selection to host storage and append it to history.
pub fn store_dataset(selection: &DatasetSelection) -> bool {
    let data = match bincode_options_dataset().serialize(selection) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if host_storage_set(ACTIVE_DATASET_KEY, &data).is_err() {
        return false;
    }
    let _ = append_dataset_history(selection);
    true
}

fn append_dataset_history(selection: &DatasetSelection) -> bool {
    let mut history: Vec<DatasetSelection> = host_storage_get(DATASET_HISTORY_KEY)
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode_options_history().deserialize(&d).ok()
            }
        })
        .unwrap_or_default();

    history.push(selection.clone());

    if history.len() > MAX_DATASET_HISTORY {
        history.drain(0..history.len() - MAX_DATASET_HISTORY);
    }

    let data = match bincode_options_history().serialize(&history) {
        Ok(d) => d,
        Err(_) => return false,
    };
    host_storage_set(DATASET_HISTORY_KEY, &data).is_ok()
}
