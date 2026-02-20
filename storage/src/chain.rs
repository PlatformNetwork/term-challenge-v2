use crate::traits::{ChallengeStorage, Result, StorageError};
use platform_challenge_sdk::{AgentInfo, EvaluationResult, WeightAssignment};
use platform_core::{ChallengeId, Hotkey};
use serde::{de::DeserializeOwned, Serialize};
use sled::{Db, Tree};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

pub struct ChainStorage {
    db: Db,
    challenge_id: ChallengeId,
    agents_tree: Tree,
    results_tree: Tree,
    weights_tree: Tree,
    kv_tree: Tree,
    meta_tree: Tree,
    validator_scores_tree: Tree,
}

impl ChainStorage {
    pub fn open<P: AsRef<Path>>(base_path: P, challenge_id: ChallengeId) -> Result<Self> {
        let db_path = base_path
            .as_ref()
            .join(format!("challenge_{}", challenge_id));

        let db = sled::open(&db_path)
            .map_err(|e| StorageError::Database(format!("Failed to open database: {}", e)))?;

        let agents_tree = db
            .open_tree("agents")
            .map_err(|e| StorageError::Database(format!("Failed to open agents tree: {}", e)))?;

        let results_tree = db
            .open_tree("results")
            .map_err(|e| StorageError::Database(format!("Failed to open results tree: {}", e)))?;

        let weights_tree = db
            .open_tree("weights")
            .map_err(|e| StorageError::Database(format!("Failed to open weights tree: {}", e)))?;

        let kv_tree = db
            .open_tree("kv")
            .map_err(|e| StorageError::Database(format!("Failed to open kv tree: {}", e)))?;

        let meta_tree = db
            .open_tree("meta")
            .map_err(|e| StorageError::Database(format!("Failed to open meta tree: {}", e)))?;

        let validator_scores_tree = db.open_tree("validator_scores").map_err(|e| {
            StorageError::Database(format!("Failed to open validator_scores tree: {}", e))
        })?;

        info!("Opened chain storage at {:?}", db_path);

        Ok(Self {
            db,
            challenge_id,
            agents_tree,
            results_tree,
            weights_tree,
            kv_tree,
            meta_tree,
            validator_scores_tree,
        })
    }
}

impl ChallengeStorage for ChainStorage {
    fn challenge_id(&self) -> ChallengeId {
        self.challenge_id
    }

    // ==================== Agents ====================

    fn save_agent(&self, agent: &AgentInfo) -> Result<()> {
        let data =
            bincode::serialize(agent).map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.agents_tree
            .insert(agent.hash.as_bytes(), data)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_agent(&self, hash: &str) -> Result<Option<AgentInfo>> {
        let data = self
            .agents_tree
            .get(hash.as_bytes())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match data {
            Some(bytes) => {
                let agent: AgentInfo = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(agent))
            }
            None => Ok(None),
        }
    }

    fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        let mut agents = Vec::new();

        for result in self.agents_tree.iter() {
            let (_, value) = result.map_err(|e| StorageError::Database(e.to_string()))?;

            let agent: AgentInfo = bincode::deserialize(&value)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;

            agents.push(agent);
        }

        Ok(agents)
    }

    // ==================== Evaluation Results ====================

    fn save_result(&self, result: &EvaluationResult) -> Result<()> {
        let key = format!("{}:{}", result.agent_hash, result.job_id);
        let data =
            bincode::serialize(result).map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.results_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_results_for_agent(&self, agent_hash: &str) -> Result<Vec<EvaluationResult>> {
        let prefix = format!("{}:", agent_hash);
        let mut results = Vec::new();

        for item in self.results_tree.scan_prefix(prefix.as_bytes()) {
            let (_, value) = item.map_err(|e| StorageError::Database(e.to_string()))?;

            let result: EvaluationResult = bincode::deserialize(&value)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;

            results.push(result);
        }

        Ok(results)
    }

    fn get_all_results(&self) -> Result<Vec<EvaluationResult>> {
        let mut results = Vec::new();

        for item in self.results_tree.iter() {
            let (_, value) = item.map_err(|e| StorageError::Database(e.to_string()))?;

            let result: EvaluationResult = bincode::deserialize(&value)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;

            results.push(result);
        }

        Ok(results)
    }

    fn get_latest_results(&self) -> Result<Vec<EvaluationResult>> {
        let mut latest: HashMap<String, EvaluationResult> = HashMap::new();

        for result in self.get_all_results()? {
            let existing = latest.get(&result.agent_hash);
            if existing.is_none() || existing.is_some_and(|e| e.timestamp < result.timestamp) {
                latest.insert(result.agent_hash.clone(), result);
            }
        }

        Ok(latest.into_values().collect())
    }

    // ==================== Weights ====================

    fn save_weights(&self, epoch: u64, weights: &[WeightAssignment]) -> Result<()> {
        let key = epoch.to_be_bytes();
        let data =
            bincode::serialize(weights).map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.weights_tree
            .insert(key.as_ref(), data)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_weights(&self, epoch: u64) -> Result<Vec<WeightAssignment>> {
        let key = epoch.to_be_bytes();
        let data = self
            .weights_tree
            .get(key.as_ref())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match data {
            Some(bytes) => {
                let weights: Vec<WeightAssignment> = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(weights)
            }
            None => Ok(Vec::new()),
        }
    }

    // ==================== Key-Value Store ====================

    fn kv_set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let data =
            bincode::serialize(value).map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.kv_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn kv_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let data = self
            .kv_tree
            .get(key.as_bytes())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match data {
            Some(bytes) => {
                let value: T = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn kv_delete(&self, key: &str) -> Result<bool> {
        let removed = self
            .kv_tree
            .remove(key.as_bytes())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(removed.is_some())
    }

    fn kv_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        for item in self.kv_tree.iter() {
            let (key, _) = item.map_err(|e| StorageError::Database(e.to_string()))?;

            if let Ok(key_str) = std::str::from_utf8(&key) {
                keys.push(key_str.to_string());
            }
        }

        Ok(keys)
    }

    // ==================== Metadata ====================

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.meta_tree
            .insert(key.as_bytes(), value.as_bytes())
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let data = self
            .meta_tree
            .get(key.as_bytes())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match data {
            Some(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).to_string())),
            None => Ok(None),
        }
    }

    // ==================== Validator Tracking ====================

    fn save_validator_score(
        &self,
        validator: &Hotkey,
        agent_hash: &str,
        score: f64,
        epoch: u64,
    ) -> Result<()> {
        let key = format!(
            "{}:{}:{}",
            hex::encode(validator.as_bytes()),
            agent_hash,
            epoch
        );
        let data =
            bincode::serialize(&score).map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.validator_scores_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_validator_scores(&self, agent_hash: &str) -> Result<Vec<(Hotkey, f64)>> {
        let mut scores = Vec::new();

        for item in self.validator_scores_tree.iter() {
            let (key, value) = item.map_err(|e| StorageError::Database(e.to_string()))?;

            let key_str =
                std::str::from_utf8(&key).map_err(|e| StorageError::InvalidData(e.to_string()))?;

            let parts: Vec<&str> = key_str.splitn(3, ':').collect();
            if parts.len() >= 2 && parts[1] == agent_hash {
                if let Some(hotkey) = Hotkey::from_hex(parts[0]) {
                    let score: f64 = bincode::deserialize(&value)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?;
                    scores.push((hotkey, score));
                }
            }
        }

        Ok(scores)
    }

    // ==================== Lifecycle ====================

    fn flush(&self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_chain_storage_open() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new());
        assert!(db.is_ok());
    }

    #[test]
    fn test_chain_storage_challenge_id() {
        let dir = tempdir().unwrap();
        let challenge_id = ChallengeId::new();
        let db = ChainStorage::open(dir.path(), challenge_id).unwrap();
        assert_eq!(db.challenge_id(), challenge_id);
    }

    #[test]
    fn test_agent_crud() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let agent = AgentInfo::new("test_hash_123".to_string());
        db.save_agent(&agent).unwrap();

        let loaded = db.get_agent("test_hash_123").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().hash, "test_hash_123");

        let loaded_none = db.get_agent("nonexistent").unwrap();
        assert!(loaded_none.is_none());
    }

    #[test]
    fn test_list_agents() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let agent1 = AgentInfo::new("hash1".to_string());
        let agent2 = AgentInfo::new("hash2".to_string());

        db.save_agent(&agent1).unwrap();
        db.save_agent(&agent2).unwrap();

        let agents = db.list_agents().unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_result_storage() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let result = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.85);
        db.save_result(&result).unwrap();

        let results = db.get_results_for_agent("agent1").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 0.85);
    }

    #[test]
    fn test_get_all_results() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let result1 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.85);
        let result2 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent2".to_string(), 0.90);

        db.save_result(&result1).unwrap();
        db.save_result(&result2).unwrap();

        let results = db.get_all_results().unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_latest_results() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let mut result1 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.70);
        result1.timestamp = chrono::Utc::now() - chrono::Duration::hours(1);

        let result2 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.90);

        db.save_result(&result1).unwrap();
        db.save_result(&result2).unwrap();

        let result3 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent2".to_string(), 0.80);
        db.save_result(&result3).unwrap();

        let latest = db.get_latest_results().unwrap();
        assert_eq!(latest.len(), 2);

        let agent1_result = latest.iter().find(|r| r.agent_hash == "agent1").unwrap();
        let agent2_result = latest.iter().find(|r| r.agent_hash == "agent2").unwrap();

        assert_eq!(agent1_result.score, 0.90);
        assert_eq!(agent2_result.score, 0.80);
    }

    #[test]
    fn test_weights_storage() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let weights = vec![
            WeightAssignment::new("hotkey1".to_string(), 0.6),
            WeightAssignment::new("hotkey2".to_string(), 0.4),
        ];

        db.save_weights(1, &weights).unwrap();

        let loaded = db.get_weights(1).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].hotkey, "hotkey1");
        assert!((loaded[0].weight - 0.6).abs() < 0.001);

        let empty = db.get_weights(999).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_kv_store() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        db.kv_set("my_key", &42i32).unwrap();

        let value: Option<i32> = db.kv_get("my_key").unwrap();
        assert_eq!(value, Some(42));

        let none: Option<i32> = db.kv_get("nonexistent").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_kv_delete() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        db.kv_set("key_to_delete", &"value").unwrap();

        let deleted = db.kv_delete("key_to_delete").unwrap();
        assert!(deleted);

        let value: Option<String> = db.kv_get("key_to_delete").unwrap();
        assert!(value.is_none());

        let deleted = db.kv_delete("non_existent").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_kv_keys() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        db.kv_set("key1", &1).unwrap();
        db.kv_set("key2", &2).unwrap();
        db.kv_set("key3", &3).unwrap();

        let keys = db.kv_keys().unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[test]
    fn test_metadata() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        db.set_meta("author", "test_author").unwrap();

        let value = db.get_meta("author").unwrap();
        assert_eq!(value, Some("test_author".to_string()));

        let none = db.get_meta("nonexistent").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_validator_scores() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        let hotkey = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        db.save_validator_score(&hotkey, "agent1", 0.95, 1).unwrap();

        let scores = db.get_validator_scores("agent1").unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].0, hotkey);
        assert_eq!(scores[0].1, 0.95);
    }

    #[test]
    fn test_flush() {
        let dir = tempdir().unwrap();
        let db = ChainStorage::open(dir.path(), ChallengeId::new()).unwrap();

        db.kv_set("test_key", &"test_value").unwrap();
        db.flush().unwrap();
    }

    #[test]
    fn test_data_persistence() {
        let dir = tempdir().unwrap();
        let challenge_id = ChallengeId::new();

        {
            let db = ChainStorage::open(dir.path(), challenge_id).unwrap();

            let agent = AgentInfo::new("persistent_agent".to_string());
            db.save_agent(&agent).unwrap();

            let result =
                EvaluationResult::new(uuid::Uuid::new_v4(), "persistent_agent".to_string(), 0.95);
            db.save_result(&result).unwrap();

            db.kv_set("persistent_key", &"persistent_value").unwrap();
            db.set_meta("test_meta", "meta_value").unwrap();
            db.flush().unwrap();
        }

        {
            let db = ChainStorage::open(dir.path(), challenge_id).unwrap();

            let agent = db.get_agent("persistent_agent").unwrap();
            assert!(agent.is_some());
            assert_eq!(agent.unwrap().hash, "persistent_agent");

            let results = db.get_results_for_agent("persistent_agent").unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].score, 0.95);

            let value: Option<String> = db.kv_get("persistent_key").unwrap();
            assert_eq!(value, Some("persistent_value".to_string()));

            let meta = db.get_meta("test_meta").unwrap();
            assert_eq!(meta, Some("meta_value".to_string()));
        }
    }
}
