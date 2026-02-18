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
    pub difficulty: Difficulty,
    pub instruction: String,
    pub timeout_secs: u64,
    pub docker_image: String,
    pub test_script: String,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub task_results: Vec<TaskResult>,
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
