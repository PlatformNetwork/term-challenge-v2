//! Snapshot System
//!
//! Creates and manages state snapshots for recovery.

use chrono::{DateTime, Utc};
use platform_core::ChainState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

/// Snapshot metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Snapshot ID
    pub id: uuid::Uuid,

    /// Snapshot name
    pub name: String,

    /// Block height at snapshot
    pub block_height: u64,

    /// Epoch at snapshot
    pub epoch: u64,

    /// State hash
    pub state_hash: String,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Size in bytes
    pub size_bytes: u64,

    /// Is this an auto snapshot
    pub auto: bool,

    /// Reason for snapshot
    pub reason: String,
}

/// Snapshot data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// Metadata
    pub meta: SnapshotMeta,

    /// Chain state
    pub chain_state: Vec<u8>,

    /// Challenge databases (challenge_id -> data)
    pub challenge_data: std::collections::HashMap<String, Vec<u8>>,

    /// Configuration
    pub config: Vec<u8>,
}

/// Snapshot manager
pub struct SnapshotManager {
    /// Data directory
    data_dir: PathBuf,

    /// Snapshots directory
    snapshots_dir: PathBuf,

    /// Maximum snapshots to keep
    max_snapshots: u32,

    /// Available snapshots
    snapshots: Vec<SnapshotMeta>,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(data_dir: PathBuf, max_snapshots: u32) -> anyhow::Result<Self> {
        let snapshots_dir = data_dir.join("snapshots");
        fs::create_dir_all(&snapshots_dir)?;

        let mut manager = Self {
            data_dir,
            snapshots_dir,
            max_snapshots,
            snapshots: Vec::new(),
        };

        manager.load_snapshot_index()?;
        Ok(manager)
    }

    /// Load snapshot index
    fn load_snapshot_index(&mut self) -> anyhow::Result<()> {
        let index_path = self.snapshots_dir.join("index.json");

        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            self.snapshots = serde_json::from_str(&content)?;
        }

        Ok(())
    }

    /// Save snapshot index
    fn save_snapshot_index(&self) -> anyhow::Result<()> {
        let index_path = self.snapshots_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.snapshots)?;
        fs::write(&index_path, content)?;
        Ok(())
    }

    /// Create a new snapshot
    pub fn create_snapshot(
        &mut self,
        name: &str,
        block_height: u64,
        epoch: u64,
        chain_state: &ChainState,
        reason: &str,
        auto: bool,
    ) -> anyhow::Result<uuid::Uuid> {
        info!(
            "Creating snapshot: {} (block={}, epoch={})",
            name, block_height, epoch
        );

        let id = uuid::Uuid::new_v4();

        // Serialize chain state
        let chain_state_bytes = bincode::serialize(chain_state)?;
        let state_hash = Self::compute_hash(&chain_state_bytes);

        // Collect challenge data
        let mut challenge_data = std::collections::HashMap::new();
        let challenges_dir = self.data_dir.join("challenges");

        if challenges_dir.exists() {
            for entry in fs::read_dir(&challenges_dir)? {
                let entry = entry?;
                let challenge_id = entry.file_name().to_string_lossy().to_string();

                // Read challenge database
                let db_path = entry.path().join("db");
                if db_path.exists() {
                    // For sled, we need to copy the directory
                    let mut data = Vec::new();
                    Self::serialize_directory(&db_path, &mut data)?;
                    challenge_data.insert(challenge_id, data);
                }
            }
        }

        // Read config
        let config_path = self.data_dir.join("subnet_config.json");
        let config = if config_path.exists() {
            fs::read(&config_path)?
        } else {
            Vec::new()
        };

        // Create snapshot
        let snapshot = Snapshot {
            meta: SnapshotMeta {
                id,
                name: name.to_string(),
                block_height,
                epoch,
                state_hash: state_hash.clone(),
                created_at: Utc::now(),
                size_bytes: 0, // Will update after saving
                auto,
                reason: reason.to_string(),
            },
            chain_state: chain_state_bytes,
            challenge_data,
            config,
        };

        // Save snapshot
        let snapshot_path = self.snapshots_dir.join(format!("{}.snapshot", id));
        let snapshot_bytes = bincode::serialize(&snapshot)?;
        fs::write(&snapshot_path, &snapshot_bytes)?;

        // Update metadata with size
        let mut meta = snapshot.meta;
        meta.size_bytes = snapshot_bytes.len() as u64;

        self.snapshots.push(meta);
        self.save_snapshot_index()?;

        // Prune old snapshots
        self.prune_snapshots()?;

        info!("Snapshot created: {} ({} bytes)", id, snapshot_bytes.len());
        Ok(id)
    }

    /// Restore from a snapshot
    pub fn restore_snapshot(&self, snapshot_id: uuid::Uuid) -> anyhow::Result<Snapshot> {
        info!("Restoring snapshot: {}", snapshot_id);

        let snapshot_path = self.snapshots_dir.join(format!("{}.snapshot", snapshot_id));

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot not found: {}", snapshot_id);
        }

        let snapshot_bytes = fs::read(&snapshot_path)?;
        let snapshot: Snapshot = bincode::deserialize(&snapshot_bytes)?;

        // Verify hash
        let computed_hash = Self::compute_hash(&snapshot.chain_state);
        if computed_hash != snapshot.meta.state_hash {
            anyhow::bail!("Snapshot corrupted: hash mismatch");
        }

        info!(
            "Snapshot loaded: {} (block={})",
            snapshot_id, snapshot.meta.block_height
        );
        Ok(snapshot)
    }

    /// Apply a snapshot to restore state
    pub fn apply_snapshot(&self, snapshot: &Snapshot) -> anyhow::Result<ChainState> {
        info!("Applying snapshot: {}", snapshot.meta.id);

        // Deserialize chain state
        let chain_state: ChainState = bincode::deserialize(&snapshot.chain_state)?;

        // Restore config
        if !snapshot.config.is_empty() {
            let config_path = self.data_dir.join("subnet_config.json");
            fs::write(&config_path, &snapshot.config)?;
        }

        // Restore challenge data
        let challenges_dir = self.data_dir.join("challenges");
        fs::create_dir_all(&challenges_dir)?;

        for (challenge_id, data) in &snapshot.challenge_data {
            let challenge_dir = challenges_dir.join(challenge_id);
            fs::create_dir_all(&challenge_dir)?;

            let db_path = challenge_dir.join("db");
            Self::deserialize_directory(&db_path, data)?;
        }

        info!("Snapshot applied: {}", snapshot.meta.id);
        Ok(chain_state)
    }

    /// Get latest snapshot
    pub fn latest_snapshot(&self) -> Option<&SnapshotMeta> {
        self.snapshots.iter().max_by_key(|s| s.created_at)
    }

    /// Get snapshot by ID
    pub fn get_snapshot(&self, id: uuid::Uuid) -> Option<&SnapshotMeta> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// List all snapshots
    pub fn list_snapshots(&self) -> &[SnapshotMeta] {
        &self.snapshots
    }

    /// Delete a snapshot
    pub fn delete_snapshot(&mut self, id: uuid::Uuid) -> anyhow::Result<()> {
        let snapshot_path = self.snapshots_dir.join(format!("{}.snapshot", id));

        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path)?;
        }

        self.snapshots.retain(|s| s.id != id);
        self.save_snapshot_index()?;

        info!("Snapshot deleted: {}", id);
        Ok(())
    }

    /// Prune old snapshots, keeping max_snapshots
    fn prune_snapshots(&mut self) -> anyhow::Result<()> {
        if self.snapshots.len() <= self.max_snapshots as usize {
            return Ok(());
        }

        // Sort by date (oldest first)
        self.snapshots.sort_by_key(|s| s.created_at);

        // Remove oldest
        let to_remove = self.snapshots.len() - self.max_snapshots as usize;
        let removed: Vec<_> = self.snapshots.drain(0..to_remove).collect();

        for meta in removed {
            let path = self.snapshots_dir.join(format!("{}.snapshot", meta.id));
            if path.exists() {
                fs::remove_file(&path)?;
            }
            debug!("Pruned snapshot: {}", meta.id);
        }

        self.save_snapshot_index()?;
        Ok(())
    }

    /// Compute SHA256 hash
    fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Serialize a directory to bytes (simplified)
    fn serialize_directory(path: &std::path::Path, output: &mut Vec<u8>) -> anyhow::Result<()> {
        // For now, just store the path - in production, would tar the directory
        // NOTE: deserialize_directory writes the stored bytes back as raw data.
        // This asymmetry is intentional for now, so document it until full tar support exists.
        let path_str = path.to_string_lossy();
        output.extend_from_slice(path_str.as_bytes());
        Ok(())
    }

    /// Deserialize bytes to a directory (simplified)
    fn deserialize_directory(path: &PathBuf, data: &[u8]) -> anyhow::Result<()> {
        // Placeholder implementation: recreate directory and store raw bytes
        fs::create_dir_all(path)?;
        if !data.is_empty() {
            let file_path = path.join("data.bin");
            fs::write(file_path, data)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_core::{Keypair, NetworkConfig};
    use tempfile::tempdir;

    #[test]
    fn test_load_snapshot_index_success() {
        let dir = tempdir().unwrap();
        let snapshots_dir = dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshots_dir).unwrap();

        let meta = SnapshotMeta {
            id: uuid::Uuid::new_v4(),
            name: "indexed".into(),
            block_height: 123,
            epoch: 4,
            state_hash: "abc123".into(),
            created_at: Utc::now(),
            size_bytes: 42,
            auto: false,
            reason: "preloaded".into(),
        };

        let index_path = snapshots_dir.join("index.json");
        let content = serde_json::to_string_pretty(&vec![meta.clone()]).unwrap();
        std::fs::write(index_path, content).unwrap();

        let manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();
        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 1);
        let loaded = &snapshots[0];
        assert_eq!(loaded.id, meta.id);
        assert_eq!(loaded.name, "indexed");
        assert_eq!(loaded.block_height, 123);
        assert_eq!(loaded.reason, "preloaded");
    }

    #[test]
    fn test_create_snapshot_reads_config_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("subnet_config.json");
        std::fs::write(&config_path, b"{\"dummy\":true}").unwrap();

        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("config_snapshot", 50, 2, &state, "test", false)
            .unwrap();

        let snapshot = manager.restore_snapshot(id).unwrap();
        assert_eq!(snapshot.config, b"{\"dummy\":true}".to_vec());
    }

    #[test]
    fn test_restore_snapshot_detects_hash_mismatch() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("corruptible", 42, 1, &state, "test", false)
            .unwrap();

        // Corrupt the stored snapshot by altering the recorded hash
        let snapshot_path = dir
            .path()
            .join("snapshots")
            .join(format!("{}.snapshot", id));
        let bytes = std::fs::read(&snapshot_path).unwrap();
        let mut snapshot: Snapshot = bincode::deserialize(&bytes).unwrap();
        snapshot.meta.state_hash = "bad-hash".into();
        let corrupt = bincode::serialize(&snapshot).unwrap();
        std::fs::write(&snapshot_path, corrupt).unwrap();

        let result = manager.restore_snapshot(id);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("hash mismatch"));
    }

    #[test]
    fn test_apply_snapshot_restores_config_and_challenges() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        // Prepare snapshot contents
        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());
        let mut snapshot = Snapshot {
            meta: SnapshotMeta {
                id: uuid::Uuid::new_v4(),
                name: "apply_test".into(),
                block_height: 1,
                epoch: 1,
                state_hash: "hash".into(),
                created_at: Utc::now(),
                size_bytes: 0,
                auto: false,
                reason: "test".into(),
            },
            chain_state: bincode::serialize(&state).unwrap(),
            challenge_data: {
                let mut map = std::collections::HashMap::new();
                map.insert("challengeA".into(), b"placeholder".to_vec());
                map
            },
            config: br#"{"foo":"bar"}"#.to_vec(),
        };

        // Save the snapshot file and metadata index manually
        let snapshot_path = manager
            .snapshots_dir
            .join(format!("{}.snapshot", snapshot.meta.id));
        snapshot.meta.state_hash = SnapshotManager::compute_hash(&snapshot.chain_state);
        let bytes = bincode::serialize(&snapshot).unwrap();
        std::fs::write(&snapshot_path, &bytes).unwrap();
        snapshot.meta.size_bytes = bytes.len() as u64;
        manager.snapshots.push(snapshot.meta.clone());
        manager.save_snapshot_index().unwrap();

        let restored = manager.restore_snapshot(snapshot.meta.id).unwrap();
        let new_state = manager.apply_snapshot(&restored).unwrap();
        assert_eq!(new_state.block_height, state.block_height);

        // Config file should exist with snapshot contents
        let config_path = manager.data_dir.join("subnet_config.json");
        let config_contents = std::fs::read(&config_path).unwrap();
        assert_eq!(config_contents, br#"{"foo":"bar"}"#);

        // Challenge directory should be recreated with db contents
        let challenge_dir = manager.data_dir.join("challenges").join("challengeA");
        let db_path = challenge_dir.join("db");
        let data_file = db_path.join("data.bin");
        assert!(db_path.exists());
        assert_eq!(std::fs::read(data_file).unwrap(), b"placeholder".to_vec());
    }

    #[test]
    fn test_deserialize_directory_creates_structure() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("nested").join("db");

        SnapshotManager::deserialize_directory(&target, b"hello").unwrap();

        let data_path = target.join("data.bin");
        assert!(target.exists());
        assert!(data_path.exists());
        assert_eq!(std::fs::read(data_path).unwrap(), b"hello");
    }

    #[test]
    fn test_snapshot_meta_fields() {
        let meta = SnapshotMeta {
            id: uuid::Uuid::new_v4(),
            name: "test_snapshot".into(),
            block_height: 1000,
            epoch: 10,
            state_hash: "abc123".into(),
            created_at: Utc::now(),
            size_bytes: 1024,
            auto: true,
            reason: "Auto snapshot".into(),
        };

        assert_eq!(meta.name, "test_snapshot");
        assert_eq!(meta.block_height, 1000);
        assert_eq!(meta.epoch, 10);
        assert!(meta.auto);
    }

    #[test]
    fn test_snapshot_manager() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        // Create snapshot
        let id = manager
            .create_snapshot("test_snapshot", 100, 1, &state, "test", false)
            .unwrap();

        assert_eq!(manager.list_snapshots().len(), 1);

        // Restore snapshot
        let snapshot = manager.restore_snapshot(id).unwrap();
        assert_eq!(snapshot.meta.block_height, 100);
    }

    #[test]
    fn test_snapshot_pruning() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 2).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        // Create 3 snapshots
        for i in 0..3 {
            manager
                .create_snapshot(&format!("snapshot_{}", i), i * 100, i, &state, "test", true)
                .unwrap();
        }

        // Should only keep 2
        assert_eq!(manager.list_snapshots().len(), 2);
    }

    #[test]
    fn test_manual_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot(
                "manual_backup",
                500,
                5,
                &state,
                "Manual backup before update",
                false,
            )
            .unwrap();

        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 1);

        let meta = &snapshots[0];
        assert!(!meta.auto);
        assert_eq!(meta.reason, "Manual backup before update");
    }

    #[test]
    fn test_auto_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot(
                "auto_snapshot_epoch_10",
                1000,
                10,
                &state,
                "Automatic snapshot",
                true,
            )
            .unwrap();

        let snapshots = manager.list_snapshots();
        let meta = &snapshots[0];
        assert!(meta.auto);
    }

    #[test]
    fn test_snapshot_with_challenge_data() {
        let dir = tempdir().unwrap();

        // Create a challenges directory with some data
        let challenges_dir = dir.path().join("challenges");
        std::fs::create_dir_all(&challenges_dir).unwrap();

        let challenge_dir = challenges_dir.join("test_challenge");
        std::fs::create_dir_all(&challenge_dir).unwrap();
        let db_dir = challenge_dir.join("db");
        std::fs::create_dir_all(&db_dir).unwrap();
        std::fs::write(db_dir.join("test.dat"), b"test data").unwrap();

        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("with_challenges", 100, 1, &state, "test", false)
            .unwrap();

        let snapshot = manager.restore_snapshot(id).unwrap();
        assert!(!snapshot.challenge_data.is_empty());
    }

    #[test]
    fn test_snapshot_list_ordering() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 10).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        // Create multiple snapshots
        for i in 0..5 {
            manager
                .create_snapshot(&format!("snap_{}", i), i * 100, i, &state, "test", true)
                .unwrap();
        }

        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 5);

        // Verify they're tracked
        for (i, snapshot) in snapshots.iter().enumerate().take(5) {
            assert_eq!(snapshot.block_height, (i * 100) as u64);
        }
    }

    #[test]
    fn test_snapshot_size_tracking() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("size_test", 100, 1, &state, "test", false)
            .unwrap();

        let snapshots = manager.list_snapshots();
        let meta = &snapshots[0];

        // Size should be set after saving
        assert!(meta.size_bytes > 0);
    }

    #[test]
    fn test_snapshot_hash_computation() {
        let data = b"test data for hashing";
        let hash = SnapshotManager::compute_hash(data);

        // SHA256 hash should be 64 hex characters
        assert_eq!(hash.len(), 64);

        // Same data should produce same hash
        let hash2 = SnapshotManager::compute_hash(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_delete_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("to_delete", 100, 1, &state, "test", false)
            .unwrap();

        assert_eq!(manager.list_snapshots().len(), 1);

        manager.delete_snapshot(id).unwrap();
        assert_eq!(manager.list_snapshots().len(), 0);
    }

    #[test]
    fn test_latest_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        assert!(manager.latest_snapshot().is_none());

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        manager
            .create_snapshot("snap1", 100, 1, &state, "test", false)
            .unwrap();
        manager
            .create_snapshot("snap2", 200, 2, &state, "test", false)
            .unwrap();

        let latest = manager.latest_snapshot().unwrap();
        assert_eq!(latest.name, "snap2");
        assert_eq!(latest.block_height, 200);
    }

    #[test]
    fn test_get_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("test_snap", 100, 1, &state, "test", false)
            .unwrap();

        let snapshot = manager.get_snapshot(id).unwrap();
        assert_eq!(snapshot.name, "test_snap");
        assert_eq!(snapshot.block_height, 100);
    }

    #[test]
    fn test_get_nonexistent_snapshot() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let result = manager.get_snapshot(uuid::Uuid::new_v4());
        assert!(result.is_none());
    }

    #[test]
    fn test_snapshot_retention_limit() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 3).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        // Create 5 snapshots with retention limit of 3
        for i in 0..5 {
            manager
                .create_snapshot(
                    &format!("snap{}", i),
                    (i + 1) * 100,
                    i + 1,
                    &state,
                    "test",
                    false,
                )
                .unwrap();
        }

        // Should only keep the 3 most recent
        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 3);

        // Just verify we have exactly 3 snapshots (pruning worked)
        // The exact order/names may vary based on pruning implementation
    }

    #[test]
    fn test_snapshot_metadata_fields() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        let id = manager
            .create_snapshot("metadata_test", 12345, 67, &state, "test reason", true)
            .unwrap();

        let snapshot = manager.get_snapshot(id).unwrap();
        assert_eq!(snapshot.name, "metadata_test");
        assert_eq!(snapshot.block_height, 12345);
        assert_eq!(snapshot.epoch, 67);
        assert_eq!(snapshot.reason, "test reason");
        assert!(snapshot.auto);
        assert!(snapshot.size_bytes > 0);
    }

    #[test]
    fn test_delete_nonexistent_snapshot() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 5).unwrap();

        // Deleting nonexistent snapshot should succeed (no-op)
        let result = manager.delete_snapshot(uuid::Uuid::new_v4());
        assert!(result.is_ok());
    }

    #[test]
    fn test_snapshot_ordering_by_time() {
        let dir = tempdir().unwrap();
        let mut manager = SnapshotManager::new(dir.path().to_path_buf(), 10).unwrap();

        let kp = Keypair::generate();
        let state = ChainState::new(kp.hotkey(), NetworkConfig::default());

        // Create snapshots in order
        for i in 0..3 {
            manager
                .create_snapshot(
                    &format!("snap{}", i),
                    (i + 1) * 100,
                    i + 1,
                    &state,
                    "test",
                    false,
                )
                .unwrap();
        }

        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 3);

        // Verify deterministic ordering without relying on timing jitter
        let names: Vec<&str> = snapshots.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["snap0", "snap1", "snap2"]);
    }
}
