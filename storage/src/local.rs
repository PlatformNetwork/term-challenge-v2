use crate::traits::{ChallengeStorage, Result, StorageError};
use platform_challenge_sdk::{AgentInfo, EvaluationResult, WeightAssignment};
use platform_core::{ChallengeId, Hotkey};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

pub struct LocalStorage {
    conn: Mutex<Connection>,
    challenge_id: ChallengeId,
}

impl LocalStorage {
    pub fn open<P: AsRef<Path>>(base_path: P, challenge_id: ChallengeId) -> Result<Self> {
        let db_path = base_path
            .as_ref()
            .join(format!("challenge_{}.db", challenge_id));

        let conn = Connection::open(&db_path)
            .map_err(|e| StorageError::Database(format!("Failed to open database: {}", e)))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| StorageError::Database(format!("Failed to set pragmas: {}", e)))?;

        let storage = Self {
            conn: Mutex::new(conn),
            challenge_id,
        };

        storage.create_tables()?;

        info!("Opened local storage at {:?}", db_path);

        Ok(storage)
    }

    pub fn open_in_memory(challenge_id: ChallengeId) -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| StorageError::Database(format!("Failed to open in-memory db: {}", e)))?;

        let storage = Self {
            conn: Mutex::new(conn),
            challenge_id,
        };

        storage.create_tables()?;

        Ok(storage)
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agents (
                hash TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS results (
                key TEXT PRIMARY KEY,
                agent_hash TEXT NOT NULL,
                data BLOB NOT NULL,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_results_agent ON results(agent_hash);
            CREATE TABLE IF NOT EXISTS weights (
                epoch INTEGER PRIMARY KEY,
                data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS validator_scores (
                key TEXT PRIMARY KEY,
                validator_hex TEXT NOT NULL,
                agent_hash TEXT NOT NULL,
                score REAL NOT NULL,
                epoch INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vs_agent ON validator_scores(agent_hash);",
        )
        .map_err(|e| StorageError::Database(format!("Failed to create tables: {}", e)))?;

        Ok(())
    }
}

impl ChallengeStorage for LocalStorage {
    fn challenge_id(&self) -> ChallengeId {
        self.challenge_id
    }

    // ==================== Agents ====================

    fn save_agent(&self, agent: &AgentInfo) -> Result<()> {
        let data = bincode::serialize(agent)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO agents (hash, data) VALUES (?1, ?2)",
            params![agent.hash, data],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_agent(&self, hash: &str) -> Result<Option<AgentInfo>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM agents WHERE hash = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt
            .query_row(params![hash], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .optional()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
            Some(bytes) => {
                let agent: AgentInfo = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(agent))
            }
            None => Ok(None),
        }
    }

    fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM agents")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let agents = stmt
            .query_map([], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|bytes| bincode::deserialize::<AgentInfo>(&bytes).ok())
            .collect();

        Ok(agents)
    }

    // ==================== Evaluation Results ====================

    fn save_result(&self, result: &EvaluationResult) -> Result<()> {
        let key = format!("{}:{}", result.agent_hash, result.job_id);
        let data = bincode::serialize(result)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let timestamp = result.timestamp.to_rfc3339();

        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO results (key, agent_hash, data, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![key, result.agent_hash, data, timestamp],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_results_for_agent(&self, agent_hash: &str) -> Result<Vec<EvaluationResult>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM results WHERE agent_hash = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let results = stmt
            .query_map(params![agent_hash], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|bytes| bincode::deserialize::<EvaluationResult>(&bytes).ok())
            .collect();

        Ok(results)
    }

    fn get_all_results(&self) -> Result<Vec<EvaluationResult>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM results")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let results = stmt
            .query_map([], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|bytes| bincode::deserialize::<EvaluationResult>(&bytes).ok())
            .collect();

        Ok(results)
    }

    fn get_latest_results(&self) -> Result<Vec<EvaluationResult>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT r.data FROM results r
                 INNER JOIN (
                     SELECT agent_hash, MAX(timestamp) as max_ts
                     FROM results
                     GROUP BY agent_hash
                 ) latest ON r.agent_hash = latest.agent_hash AND r.timestamp = latest.max_ts",
            )
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let results = stmt
            .query_map([], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|bytes| bincode::deserialize::<EvaluationResult>(&bytes).ok())
            .collect();

        Ok(results)
    }

    // ==================== Weights ====================

    fn save_weights(&self, epoch: u64, weights: &[WeightAssignment]) -> Result<()> {
        let data = bincode::serialize(weights)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO weights (epoch, data) VALUES (?1, ?2)",
            params![epoch as i64, data],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_weights(&self, epoch: u64) -> Result<Vec<WeightAssignment>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM weights WHERE epoch = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt
            .query_row(params![epoch as i64], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .optional()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
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
        let data = bincode::serialize(value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO kv (key, data) VALUES (?1, ?2)",
            params![key, data],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn kv_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT data FROM kv WHERE key = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt
            .query_row(params![key], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .optional()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
            Some(bytes) => {
                let value: T = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn kv_delete(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let rows = conn
            .execute("DELETE FROM kv WHERE key = ?1", params![key])
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(rows > 0)
    }

    fn kv_keys(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT key FROM kv")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let keys = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                Ok(key)
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(keys)
    }

    // ==================== Metadata ====================

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt
            .query_row(params![key], |row| {
                let value: String = row.get(0)?;
                Ok(value)
            })
            .optional()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(result)
    }

    // ==================== Validator Tracking ====================

    fn save_validator_score(
        &self,
        validator: &Hotkey,
        agent_hash: &str,
        score: f64,
        epoch: u64,
    ) -> Result<()> {
        let validator_hex = hex::encode(validator.as_bytes());
        let key = format!("{}:{}:{}", validator_hex, agent_hash, epoch);

        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO validator_scores (key, validator_hex, agent_hash, score, epoch) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![key, validator_hex, agent_hash, score, epoch as i64],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_validator_scores(
        &self,
        agent_hash: &str,
    ) -> Result<Vec<(Hotkey, f64)>> {
        let conn = self.conn.lock().map_err(|e| {
            StorageError::Database(format!("Failed to acquire lock: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT validator_hex, score FROM validator_scores WHERE agent_hash = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let scores = stmt
            .query_map(params![agent_hash], |row| {
                let validator_hex: String = row.get(0)?;
                let score: f64 = row.get(1)?;
                Ok((validator_hex, score))
            })
            .map_err(|e| StorageError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|(hex_str, score)| {
                Hotkey::from_hex(&hex_str).map(|h| (h, score))
            })
            .collect();

        Ok(scores)
    }

    // ==================== Lifecycle ====================

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_storage() -> LocalStorage {
        LocalStorage::open_in_memory(ChallengeId::new()).unwrap()
    }

    #[test]
    fn test_local_storage_open() {
        let dir = tempfile::tempdir().unwrap();
        let db = LocalStorage::open(dir.path(), ChallengeId::new());
        assert!(db.is_ok());
    }

    #[test]
    fn test_local_storage_challenge_id() {
        let challenge_id = ChallengeId::new();
        let db = LocalStorage::open_in_memory(challenge_id).unwrap();
        assert_eq!(db.challenge_id(), challenge_id);
    }

    #[test]
    fn test_agent_crud() {
        let db = make_storage();

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
        let db = make_storage();

        let agent1 = AgentInfo::new("hash1".to_string());
        let agent2 = AgentInfo::new("hash2".to_string());

        db.save_agent(&agent1).unwrap();
        db.save_agent(&agent2).unwrap();

        let agents = db.list_agents().unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_result_storage() {
        let db = make_storage();

        let result = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.85);
        db.save_result(&result).unwrap();

        let results = db.get_results_for_agent("agent1").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 0.85);
    }

    #[test]
    fn test_get_all_results() {
        let db = make_storage();

        let result1 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent1".to_string(), 0.85);
        let result2 = EvaluationResult::new(uuid::Uuid::new_v4(), "agent2".to_string(), 0.90);

        db.save_result(&result1).unwrap();
        db.save_result(&result2).unwrap();

        let results = db.get_all_results().unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_latest_results() {
        let db = make_storage();

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
        let db = make_storage();

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
        let db = make_storage();

        db.kv_set("my_key", &42i32).unwrap();

        let value: Option<i32> = db.kv_get("my_key").unwrap();
        assert_eq!(value, Some(42));

        let none: Option<i32> = db.kv_get("nonexistent").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_kv_delete() {
        let db = make_storage();

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
        let db = make_storage();

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
        let db = make_storage();

        db.set_meta("author", "test_author").unwrap();

        let value = db.get_meta("author").unwrap();
        assert_eq!(value, Some("test_author".to_string()));

        let none = db.get_meta("nonexistent").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_validator_scores() {
        let db = make_storage();

        let hotkey = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        db.save_validator_score(&hotkey, "agent1", 0.95, 1).unwrap();

        let scores = db.get_validator_scores("agent1").unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].0, hotkey);
        assert_eq!(scores[0].1, 0.95);
    }

    #[test]
    fn test_flush() {
        let db = make_storage();
        db.kv_set("test_key", &"test_value").unwrap();
        db.flush().unwrap();
    }

    #[test]
    fn test_data_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let challenge_id = ChallengeId::new();

        {
            let db = LocalStorage::open(dir.path(), challenge_id).unwrap();

            let agent = AgentInfo::new("persistent_agent".to_string());
            db.save_agent(&agent).unwrap();

            let result =
                EvaluationResult::new(uuid::Uuid::new_v4(), "persistent_agent".to_string(), 0.95);
            db.save_result(&result).unwrap();

            db.kv_set("persistent_key", &"persistent_value").unwrap();
            db.set_meta("test_meta", "meta_value").unwrap();
        }

        {
            let db = LocalStorage::open(dir.path(), challenge_id).unwrap();

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
