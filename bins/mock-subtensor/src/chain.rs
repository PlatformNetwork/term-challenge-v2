//! Chain module - Block production and management

use super::Config;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Block header
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub number: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub extrinsics_root: [u8; 32],
    pub timestamp: i64,
}

impl BlockHeader {
    pub fn new(number: u64, parent_hash: [u8; 32]) -> Self {
        let timestamp = Utc::now().timestamp();
        let mut hasher = Sha256::new();
        hasher.update(number.to_le_bytes());
        hasher.update(parent_hash);
        hasher.update(timestamp.to_le_bytes());

        let hash: [u8; 32] = hasher.finalize().into();

        Self {
            number,
            hash,
            parent_hash,
            state_root: hash, // Simplified
            extrinsics_root: [0u8; 32],
            timestamp,
        }
    }
}

/// Block
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub hash: [u8; 32],
    pub extrinsics: Vec<Extrinsic>,
}

/// Extrinsic (transaction)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Extrinsic {
    pub hash: [u8; 32],
    pub method: String,
    pub params: serde_json::Value,
    pub success: bool,
    pub block_number: u64,
    pub timestamp: i64,
}

impl Extrinsic {
    pub fn new(method: &str, params: serde_json::Value, block_number: u64) -> Self {
        let timestamp = Utc::now().timestamp();
        let mut hasher = Sha256::new();
        hasher.update(method.as_bytes());
        hasher.update(serde_json::to_vec(&params).unwrap_or_default());
        hasher.update(block_number.to_le_bytes());

        Self {
            hash: hasher.finalize().into(),
            method: method.to_string(),
            params,
            success: true,
            block_number,
            timestamp,
        }
    }
}

/// Chain state
pub struct Chain {
    pub blocks: HashMap<u64, Block>,
    pub block_hashes: HashMap<String, u64>,
    pub pending_extrinsics: Vec<Extrinsic>,
    pub finalized_number: u64,
    pub config: ChainConfig,
}

#[derive(Clone, Debug)]
pub struct ChainConfig {
    pub tempo: u64,
    pub netuid: u16,
    pub commit_reveal: bool,
    pub reveal_period: u64,
    pub ss58_format: u16,
    pub token_decimals: u8,
}

impl Chain {
    pub fn new(config: &Config) -> Self {
        let chain_config = ChainConfig {
            tempo: config.tempo,
            netuid: config.netuid,
            commit_reveal: config.commit_reveal,
            reveal_period: config.reveal_period,
            ss58_format: 42,
            token_decimals: 9,
        };

        let mut chain = Self {
            blocks: HashMap::new(),
            block_hashes: HashMap::new(),
            pending_extrinsics: Vec::new(),
            finalized_number: 0,
            config: chain_config,
        };

        // Create genesis block
        chain.create_genesis();
        chain
    }

    fn create_genesis(&mut self) {
        let header = BlockHeader::new(0, [0u8; 32]);
        let block = Block {
            hash: header.hash,
            header,
            extrinsics: Vec::new(),
        };

        let hash_hex = format!("0x{}", hex::encode(block.hash));
        self.block_hashes.insert(hash_hex, 0);
        self.blocks.insert(0, block);
    }

    /// Produce a new block
    pub fn produce_block(&mut self) -> Block {
        let current_number = self.best_number();
        let new_number = current_number + 1;

        // Get parent hash
        let parent_hash = if let Some(block) = self.blocks.get(&current_number) {
            block.hash
        } else {
            [0u8; 32]
        };

        // Include pending extrinsics
        let extrinsics: Vec<Extrinsic> = self.pending_extrinsics.drain(..).collect();

        let header = BlockHeader::new(new_number, parent_hash);
        let block = Block {
            hash: header.hash,
            header: header.clone(),
            extrinsics: extrinsics.clone(),
        };

        // Store block
        let hash_hex = format!("0x{}", hex::encode(block.hash));
        self.block_hashes.insert(hash_hex, new_number);
        self.blocks.insert(new_number, block.clone());

        // Update finalized (simplified: finalize 3 blocks back)
        if new_number >= 3 {
            self.finalized_number = new_number - 3;
        }

        block
    }

    /// Get best (latest) block number
    pub fn best_number(&self) -> u64 {
        self.blocks.keys().max().copied().unwrap_or(0)
    }

    /// Get block by number
    pub fn get_block(&self, number: u64) -> Option<&Block> {
        self.blocks.get(&number)
    }

    /// Get block by hash
    pub fn get_block_by_hash(&self, hash: &str) -> Option<&Block> {
        self.block_hashes.get(hash).and_then(|n| self.blocks.get(n))
    }

    /// Get finalized block number
    pub fn finalized_number(&self) -> u64 {
        self.finalized_number
    }

    /// Submit extrinsic
    pub fn submit_extrinsic(&mut self, method: &str, params: serde_json::Value) -> Extrinsic {
        let block_number = self.best_number();
        let extrinsic = Extrinsic::new(method, params, block_number);
        self.pending_extrinsics.push(extrinsic.clone());
        extrinsic
    }

    /// Get runtime version
    pub fn runtime_version(&self) -> serde_json::Value {
        serde_json::json!({
            "specName": "subtensor",
            "implName": "mock-subtensor",
            "authoringVersion": 1,
            "specVersion": 100,
            "implVersion": 1,
            "apis": [
                ["0xdf6acb689907609b", 3],
                ["0x37e397fc7c91f5e4", 1],
                ["0x40fe3ad401f8949", 5],
                ["0xd2bc9897eed08f15", 3],
                ["0xf78b278be53f454c", 2],
                ["0xaf2c0297a23e6d3d", 2],
                ["0xed99c5acb25eedf5", 2],
                ["0xcbca25e39f142387", 2],
                ["0x687ad44ad37b03f2", 1],
                ["0xab3c0572291feb8b", 1],
                ["0xbc9d89904f5b923f", 1],
                ["0x37c8bb1350a9a2a8", 1],
            ],
            "transactionVersion": 10,
            "stateVersion": 0,
        })
    }

    /// Get chain properties
    pub fn properties(&self) -> serde_json::Value {
        serde_json::json!({
            "ss58Format": self.config.ss58_format,
            "tokenDecimals": self.config.token_decimals,
            "tokenSymbol": "TAO"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            bind: "127.0.0.1:9944".parse().unwrap(),
            tempo: 12,
            netuid: 100,
            validator_count: 256,
            min_stake: 1_000_000_000_000,
            commit_reveal: true,
            reveal_period: 12,
            log_level: "info".to_string(),
            inspection: true,
        }
    }

    #[test]
    fn test_chain_genesis() {
        let chain = Chain::new(&test_config());
        assert_eq!(chain.best_number(), 0);
        assert!(chain.get_block(0).is_some());
    }

    #[test]
    fn test_block_production() {
        let mut chain = Chain::new(&test_config());
        let block = chain.produce_block();
        assert_eq!(block.header.number, 1);
        assert_eq!(chain.best_number(), 1);
    }

    #[test]
    fn test_block_hash_lookup() {
        let mut chain = Chain::new(&test_config());
        let block = chain.produce_block();
        let hash_hex = format!("0x{}", hex::encode(block.hash));

        assert!(chain.get_block_by_hash(&hash_hex).is_some());
        assert_eq!(chain.get_block_by_hash(&hash_hex).unwrap().header.number, 1);
    }

    #[test]
    fn test_extrinsic_submission() {
        let mut chain = Chain::new(&test_config());
        let ext = chain.submit_extrinsic("test_method", serde_json::json!({"test": true}));

        assert_eq!(ext.method, "test_method");
        assert!(ext.success);

        // Produce block to include extrinsic
        let block = chain.produce_block();
        assert_eq!(block.extrinsics.len(), 1);
    }

    #[test]
    fn test_runtime_version() {
        let chain = Chain::new(&test_config());
        let version = chain.runtime_version();

        assert!(version.get("specName").is_some());
        assert!(version.get("apis").is_some());
    }

    #[test]
    fn test_chain_properties() {
        let chain = Chain::new(&test_config());
        let props = chain.properties();

        assert_eq!(props["ss58Format"], 42);
        assert_eq!(props["tokenSymbol"], "TAO");
        assert_eq!(props["tokenDecimals"], 9);
    }
}
