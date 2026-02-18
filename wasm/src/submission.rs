use alloc::string::String;
use alloc::vec::Vec;
use platform_challenge_sdk_wasm::host_functions::{
    host_consensus_get_epoch, host_storage_get, host_storage_set,
};

use crate::types::{SubmissionName, SubmissionVersion};

pub fn register_submission_name(name: &str, hotkey: &str) -> bool {
    let mut key = Vec::from(b"name_registry:" as &[u8]);
    key.extend_from_slice(name.as_bytes());

    if let Ok(data) = host_storage_get(&key) {
        if !data.is_empty() {
            if let Ok(existing) = bincode::deserialize::<SubmissionName>(&data) {
                return existing.owner_hotkey == hotkey;
            }
            return false;
        }
    }

    let epoch = host_consensus_get_epoch();
    let entry = SubmissionName {
        name: String::from(name),
        owner_hotkey: String::from(hotkey),
        registered_epoch: if epoch >= 0 { epoch as u64 } else { 0 },
    };
    if let Ok(data) = bincode::serialize(&entry) {
        return host_storage_set(&key, &data).is_ok();
    }
    false
}

pub fn submit_versioned(name: &str, hotkey: &str, agent_hash: &str, epoch: u64) -> Option<u32> {
    if !register_submission_name(name, hotkey) {
        return None;
    }

    let mut key = Vec::from(b"submission_versions:" as &[u8]);
    key.extend_from_slice(hotkey.as_bytes());
    key.push(b':');
    key.extend_from_slice(name.as_bytes());

    let mut versions: Vec<SubmissionVersion> = host_storage_get(&key)
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode::deserialize(&d).ok()
            }
        })
        .unwrap_or_default();

    let next_version = versions.last().map(|v| v.version + 1).unwrap_or(1);
    versions.push(SubmissionVersion {
        version: next_version,
        agent_hash: String::from(agent_hash),
        epoch,
        score: None,
    });

    if let Ok(data) = bincode::serialize(&versions) {
        if host_storage_set(&key, &data).is_ok() {
            return Some(next_version);
        }
    }
    None
}

pub fn get_submission_history(hotkey: &str, name: &str) -> Vec<SubmissionVersion> {
    let mut key = Vec::from(b"submission_versions:" as &[u8]);
    key.extend_from_slice(hotkey.as_bytes());
    key.push(b':');
    key.extend_from_slice(name.as_bytes());

    host_storage_get(&key)
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

pub fn get_submission_by_name(name: &str) -> Option<(String, SubmissionVersion)> {
    let mut key = Vec::from(b"name_registry:" as &[u8]);
    key.extend_from_slice(name.as_bytes());

    let data = host_storage_get(&key).ok()?;
    if data.is_empty() {
        return None;
    }
    let entry: SubmissionName = bincode::deserialize(&data).ok()?;

    let versions = get_submission_history(&entry.owner_hotkey, name);
    let latest = versions.last()?.clone();
    Some((entry.owner_hotkey, latest))
}
