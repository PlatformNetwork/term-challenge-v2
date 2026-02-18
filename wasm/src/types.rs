use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub base_commit: String,
    pub difficulty: Difficulty,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub passed: bool,
    pub score: f64,
    pub execution_time_ms: u64,
    pub test_output: String,
    pub agent_output: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeParams {
    pub tasks: Vec<TaskDefinition>,
    pub llm_judge_url: Option<String>,
    pub decay_params: Option<DecayParams>,
    pub active_dataset: Option<Vec<TaskDefinition>>,
}

#[derive(Clone, Deserialize)]
pub struct Submission {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub signature: Vec<u8>,
    pub epoch: u64,
    pub package_zip: Vec<u8>,
    pub basilica_instance: String,
    pub executor_url: String,
    pub executor_token_hash: Vec<u8>,
    pub task_results: Vec<TaskResult>,
}

impl core::fmt::Debug for Submission {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Submission")
            .field("agent_hash", &self.agent_hash)
            .field("miner_hotkey", &self.miner_hotkey)
            .field("signature", &"[REDACTED]")
            .field("epoch", &self.epoch)
            .field("package_zip_len", &self.package_zip.len())
            .field("basilica_instance", &self.basilica_instance)
            .field("executor_url", &self.executor_url)
            .field("executor_token_hash", &"[REDACTED]")
            .field("task_results", &self.task_results)
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DifficultyStats {
    pub total: u32,
    pub passed: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmJudgeRequest {
    pub task_id: String,
    pub instruction: String,
    pub agent_output: String,
    pub test_output: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmJudgeResponse {
    pub score: f64,
    pub reasoning: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecayParams {
    pub grace_period_hours: u64,
    pub half_life_hours: u64,
    pub min_multiplier: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetSelection {
    pub tasks: Vec<TaskDefinition>,
    pub selected_at_epoch: u64,
    pub dataset_hash: String,
}
