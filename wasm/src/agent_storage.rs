use alloc::string::String;
use alloc::vec::Vec;
use platform_challenge_sdk_wasm::host_functions::{host_storage_get, host_storage_set};

use crate::types::{AgentLogs, EvaluationStatus};

pub const MAX_AGENT_PACKAGE_SIZE: usize = 1_048_576;
const MAX_LOG_SIZE: usize = 262_144;
pub const MAX_TASK_OUTPUT_PREVIEW: usize = 4_096;

fn make_key(prefix: &[u8], miner_hotkey: &str, epoch: u64) -> Vec<u8> {
    let mut key = Vec::from(prefix);
    key.extend_from_slice(miner_hotkey.as_bytes());
    key.push(b':');
    key.extend_from_slice(&epoch.to_le_bytes());
    key
}

pub fn store_agent_code(miner_hotkey: &str, epoch: u64, package_zip: &[u8]) -> bool {
    if package_zip.len() > MAX_AGENT_PACKAGE_SIZE {
        return false;
    }
    let key = make_key(b"agent_code:", miner_hotkey, epoch);
    host_storage_set(&key, package_zip).is_ok()
}

pub fn store_agent_hash(miner_hotkey: &str, epoch: u64, agent_hash: &str) -> bool {
    let key = make_key(b"agent_hash:", miner_hotkey, epoch);
    host_storage_set(&key, agent_hash.as_bytes()).is_ok()
}

pub fn store_agent_logs(miner_hotkey: &str, epoch: u64, logs: &AgentLogs) -> bool {
    let data = match bincode::serialize(logs) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if data.len() > MAX_LOG_SIZE {
        return false;
    }
    let key = make_key(b"agent_logs:", miner_hotkey, epoch);
    host_storage_set(&key, &data).is_ok()
}

pub fn get_agent_code(miner_hotkey: &str, epoch: u64) -> Option<Vec<u8>> {
    let key = make_key(b"agent_code:", miner_hotkey, epoch);
    let data = host_storage_get(&key).ok()?;
    if data.is_empty() {
        return None;
    }
    Some(data)
}

pub fn get_agent_logs(miner_hotkey: &str, epoch: u64) -> Option<AgentLogs> {
    let key = make_key(b"agent_logs:", miner_hotkey, epoch);
    let data = host_storage_get(&key).ok()?;
    if data.is_empty() {
        return None;
    }
    bincode::deserialize(&data).ok()
}

pub fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        return String::from(output);
    }
    let mut end = max_len;
    while end > 0 && !output.is_char_boundary(end) {
        end -= 1;
    }
    String::from(&output[..end])
}

pub fn store_evaluation_status(miner_hotkey: &str, epoch: u64, status: EvaluationStatus) -> bool {
    let key = make_key(b"eval_status:", miner_hotkey, epoch);
    if let Ok(data) = bincode::serialize(&status) {
        return host_storage_set(&key, &data).is_ok();
    }
    false
}

pub fn get_evaluation_status(miner_hotkey: &str, epoch: u64) -> Option<EvaluationStatus> {
    let key = make_key(b"eval_status:", miner_hotkey, epoch);
    let data = host_storage_get(&key).ok()?;
    if data.is_empty() {
        return None;
    }
    bincode::deserialize(&data).ok()
}
