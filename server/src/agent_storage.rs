use platform_challenge_sdk::ChallengeDatabase;

use crate::types::{
    AgentLogs, EvaluationStatus, EvaluationStep, MAX_AGENT_CODE_SIZE, MAX_AGENT_LOGS_SIZE,
    MAX_OUTPUT_PREVIEW,
};

fn code_key(hotkey: &str, epoch: u64) -> String {
    format!("agent_code:{}:{}", hotkey, epoch)
}

fn hash_key(hotkey: &str, epoch: u64) -> String {
    format!("agent_hash:{}:{}", hotkey, epoch)
}

fn logs_key(hotkey: &str, epoch: u64) -> String {
    format!("agent_logs:{}:{}", hotkey, epoch)
}

fn status_key(hotkey: &str, epoch: u64) -> String {
    format!("agent_status:{}:{}", hotkey, epoch)
}

pub fn store_agent_code(
    db: &ChallengeDatabase,
    hotkey: &str,
    epoch: u64,
    code: &[u8],
    hash: &str,
) -> Result<bool, platform_challenge_sdk::ChallengeError> {
    if code.len() > MAX_AGENT_CODE_SIZE {
        return Ok(false);
    }
    db.kv_set(&code_key(hotkey, epoch), &code.to_vec())?;
    db.kv_set(&hash_key(hotkey, epoch), &hash.to_string())?;
    Ok(true)
}

pub fn get_agent_code(db: &ChallengeDatabase, hotkey: &str, epoch: u64) -> Option<Vec<u8>> {
    db.kv_get::<Vec<u8>>(&code_key(hotkey, epoch))
        .ok()
        .flatten()
}

pub fn store_agent_logs(
    db: &ChallengeDatabase,
    hotkey: &str,
    epoch: u64,
    logs: &mut AgentLogs,
) -> Result<bool, platform_challenge_sdk::ChallengeError> {
    for task_log in &mut logs.task_logs {
        if task_log.output_preview.len() > MAX_OUTPUT_PREVIEW {
            task_log.output_preview = task_log.output_preview[..MAX_OUTPUT_PREVIEW].to_string();
        }
    }

    let serialized = bincode::serialize(logs)
        .map_err(|e| platform_challenge_sdk::ChallengeError::Serialization(e.to_string()))?;

    if serialized.len() > MAX_AGENT_LOGS_SIZE {
        return Ok(false);
    }

    db.kv_set(&logs_key(hotkey, epoch), logs)?;
    Ok(true)
}

pub fn get_agent_logs(db: &ChallengeDatabase, hotkey: &str, epoch: u64) -> Option<AgentLogs> {
    db.kv_get::<AgentLogs>(&logs_key(hotkey, epoch))
        .ok()
        .flatten()
}

pub fn set_evaluation_status(
    db: &ChallengeDatabase,
    hotkey: &str,
    epoch: u64,
    status: &EvaluationStatus,
) -> Result<(), platform_challenge_sdk::ChallengeError> {
    db.kv_set(&status_key(hotkey, epoch), status)
}

pub fn get_evaluation_status(
    db: &ChallengeDatabase,
    hotkey: &str,
    epoch: u64,
) -> Option<EvaluationStatus> {
    db.kv_get::<EvaluationStatus>(&status_key(hotkey, epoch))
        .ok()
        .flatten()
}

pub fn create_initial_status(hotkey: &str, epoch: u64) -> EvaluationStatus {
    EvaluationStatus {
        hotkey: hotkey.to_string(),
        epoch,
        phase: "started".to_string(),
        steps: vec![
            EvaluationStep {
                name: "submission_received".to_string(),
                status: "complete".to_string(),
                detail: None,
            },
            EvaluationStep {
                name: "ast_validation".to_string(),
                status: "pending".to_string(),
                detail: None,
            },
            EvaluationStep {
                name: "llm_review".to_string(),
                status: "pending".to_string(),
                detail: None,
            },
            EvaluationStep {
                name: "task_scoring".to_string(),
                status: "pending".to_string(),
                detail: None,
            },
            EvaluationStep {
                name: "aggregate".to_string(),
                status: "pending".to_string(),
                detail: None,
            },
        ],
    }
}
