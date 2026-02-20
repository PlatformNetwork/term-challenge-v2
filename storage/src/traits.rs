use platform_challenge_sdk::{AgentInfo, EvaluationResult, WeightAssignment};
use platform_core::{ChallengeId, Hotkey};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
}

impl From<tokio_postgres::Error> for StorageError {
    fn from(err: tokio_postgres::Error) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for StorageError {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        StorageError::Serialization(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub trait ChallengeStorage: Send + Sync {
    fn challenge_id(&self) -> ChallengeId;

    // ==================== Agents ====================

    fn save_agent(&self, agent: &AgentInfo) -> Result<()>;
    fn get_agent(&self, hash: &str) -> Result<Option<AgentInfo>>;
    fn list_agents(&self) -> Result<Vec<AgentInfo>>;

    // ==================== Evaluation Results ====================

    fn save_result(&self, result: &EvaluationResult) -> Result<()>;
    fn get_results_for_agent(&self, agent_hash: &str) -> Result<Vec<EvaluationResult>>;
    fn get_all_results(&self) -> Result<Vec<EvaluationResult>>;
    fn get_latest_results(&self) -> Result<Vec<EvaluationResult>>;

    // ==================== Weights ====================

    fn save_weights(&self, epoch: u64, weights: &[WeightAssignment]) -> Result<()>;
    fn get_weights(&self, epoch: u64) -> Result<Vec<WeightAssignment>>;

    // ==================== Key-Value Store ====================

    fn kv_set<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
    fn kv_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    fn kv_delete(&self, key: &str) -> Result<bool>;
    fn kv_keys(&self) -> Result<Vec<String>>;

    // ==================== Metadata ====================

    fn set_meta(&self, key: &str, value: &str) -> Result<()>;
    fn get_meta(&self, key: &str) -> Result<Option<String>>;

    // ==================== Validator Tracking ====================

    fn save_validator_score(
        &self,
        validator: &Hotkey,
        agent_hash: &str,
        score: f64,
        epoch: u64,
    ) -> Result<()>;
    fn get_validator_scores(&self, agent_hash: &str) -> Result<Vec<(Hotkey, f64)>>;

    // ==================== Lifecycle ====================

    fn flush(&self) -> Result<()>;
}
