use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetEntry {
    pub instance_id: String,
    pub repo: String,
    pub base_commit: String,
    pub patch: String,
    pub test_patch: String,
    pub problem_statement: String,
    #[serde(default)]
    pub hints_text: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "FAIL_TO_PASS")]
    pub fail_to_pass: String,
    #[serde(default, rename = "PASS_TO_PASS")]
    pub pass_to_pass: String,
    #[serde(default)]
    pub environment_setup_commit: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub difficulty_score: u8,
    #[serde(default)]
    pub quality_score: f64,
}
