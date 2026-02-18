use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Task difficulty level used for per-category scoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

/// Definition of a single SWE-bench evaluation task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub base_commit: String,
    pub difficulty: Difficulty,
    pub timeout_secs: u64,
}

/// Result produced by a miner for a single task.
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

/// Parameters supplied to the challenge during evaluation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeParams {
    pub tasks: Vec<TaskDefinition>,
    pub llm_judge_url: Option<String>,
}

/// A miner's submission containing agent metadata and task results.
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

/// Pass/total statistics for a single difficulty level.
#[derive(Clone, Debug)]
pub struct DifficultyStats {
    pub total: u32,
    pub passed: u32,
}

/// Request payload sent to the LLM judge endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmJudgeRequest {
    pub task_id: String,
    pub instruction: String,
    pub agent_output: String,
    pub test_output: String,
}

/// Response received from the LLM judge endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmJudgeResponse {
    pub score: f64,
    pub reasoning: String,
}

/// A selected set of tasks stored via `configure()` for later retrieval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetSelection {
    pub tasks: Vec<TaskDefinition>,
    pub selected_at_epoch: u64,
    pub dataset_hash: String,
}
