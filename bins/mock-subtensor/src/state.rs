//! State module - Mock metagraph and validator management

use super::Config;
use chrono::Utc;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Validator in the mock metagraph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    pub uid: u16,
    pub hotkey: String,
    pub coldkey: String,
    pub stake: u64,
    pub trust: f64,
    pub validator_trust: f64,
    pub consensus: f64,
    pub incentive: f64,
    pub dividends: f64,
    pub emission: u64,
    pub validator_permit: bool,
    pub last_update: u64,
    pub active: bool,
    pub axon_info: AxonInfo,
    pub prometheus_info: PrometheusInfo,
}

impl Validator {
    pub fn with_uid(uid: u16, netuid: u16) -> Self {
        let mut rng = StdRng::seed_from_u64(uid as u64 + netuid as u64);

        // Deterministic hotkey generation
        let mut hasher = Sha256::new();
        hasher.update(b"validator");
        hasher.update(uid.to_le_bytes());
        hasher.update(netuid.to_le_bytes());
        let hotkey_bytes: [u8; 32] = hasher.finalize().into();

        // Generate SS58 address
        let hotkey = ss58_encode(42, &hotkey_bytes);

        // Coldkey is different
        let mut hasher_cold = Sha256::new();
        hasher_cold.update(b"coldkey");
        hasher_cold.update(uid.to_le_bytes());
        let coldkey_bytes: [u8; 32] = hasher_cold.finalize().into();
        let coldkey = ss58_encode(42, &coldkey_bytes);

        // Realistic stake distribution - most validators have moderate stake
        // Pareto-like distribution: few validators with high stake, many with low
        let stake_tao = match uid % 10 {
            0 => rng.gen_range(5000.0..50000.0),    // 10% whales
            1..=3 => rng.gen_range(1000.0..5000.0), // 30% high
            _ => rng.gen_range(100.0..1000.0),      // 60% moderate
        };
        let stake = (stake_tao * 1_000_000_000.0) as u64; // Convert to RAO

        // Generate realistic performance metrics
        let trust = rng.gen_range(0.5..1.0);
        let validator_trust = if uid.is_multiple_of(4) {
            rng.gen_range(0.7..1.0) // Validators
        } else {
            rng.gen_range(0.0..0.5) // Miners
        };
        let consensus = rng.gen_range(0.5..1.0);
        let incentive = rng.gen_range(0.0..1.0);
        let dividends = rng.gen_range(0.0..1.0);
        let emission = rng.gen_range(0..100_000_000_000); // 0-100 TAO

        // Validator permit based on stake and trust
        let validator_permit = stake > 1_000_000_000_000 && trust > 0.8;

        Self {
            uid,
            hotkey,
            coldkey,
            stake,
            trust,
            validator_trust,
            consensus,
            incentive,
            dividends,
            emission,
            validator_permit,
            last_update: rng.gen_range(100..10000),
            active: true,
            axon_info: AxonInfo::generate(&mut rng),
            prometheus_info: PrometheusInfo::generate(&mut rng),
        }
    }

    pub fn to_neuron_info(&self) -> serde_json::Value {
        serde_json::json!({
            "hotkey": self.hotkey,
            "coldkey": self.coldkey,
            "uid": self.uid,
            "netuid": 0,
            "active": self.active,
            "axon_info": {
                "block": self.axon_info.block,
                "version": self.axon_info.version,
                "ip": self.axon_info.ip,
                "port": self.axon_info.port,
                "ip_type": self.axon_info.ip_type,
                "protocol": self.axon_info.protocol,
                "placeholder1": 0,
                "placeholder2": 0,
            },
            "prometheus_info": {
                "block": self.prometheus_info.block,
                "version": self.prometheus_info.version,
                "ip": self.prometheus_info.ip,
                "port": self.prometheus_info.port,
                "ip_type": self.prometheus_info.ip_type,
            },
            "stake": [
                [self.coldkey.clone(), self.stake.to_string()]
            ],
            "rank": 0.0,
            "emission": self.emission,
            "incentive": self.incentive,
            "consensus": self.consensus,
            "trust": self.trust,
            "validator_trust": self.validator_trust,
            "dividends": self.dividends,
            "weights": [],
            "bonds": [],
            "validator_permit": self.validator_permit,
        })
    }

    pub fn to_neuron_info_lite(&self) -> serde_json::Value {
        serde_json::json!({
            "hotkey": self.hotkey,
            "coldkey": self.coldkey,
            "uid": self.uid,
            "stake": self.stake,
            "trust": self.trust,
            "validator_trust": self.validator_trust,
            "consensus": self.consensus,
            "incentive": self.incentive,
            "dividends": self.dividends,
            "emission": self.emission,
            "validator_permit": self.validator_permit,
            "axon_info": {
                "ip": self.axon_info.ip,
                "port": self.axon_info.port,
            },
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AxonInfo {
    pub block: u64,
    pub version: u32,
    pub ip: u32,
    pub port: u16,
    pub ip_type: u8,
    pub protocol: u8,
}

impl AxonInfo {
    fn generate<R: Rng>(rng: &mut R) -> Self {
        Self {
            block: rng.gen_range(1000..100000),
            version: rng.gen_range(1..10),
            ip: rng.gen::<u32>(),
            port: rng.gen_range(8000..9000),
            ip_type: 4,
            protocol: rng.gen_range(0..3),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrometheusInfo {
    pub block: u64,
    pub version: u32,
    pub ip: u32,
    pub port: u16,
    pub ip_type: u8,
}

impl PrometheusInfo {
    fn generate<R: Rng>(rng: &mut R) -> Self {
        Self {
            block: rng.gen_range(1000..100000),
            version: rng.gen_range(1..10),
            ip: rng.gen::<u32>(),
            port: rng.gen_range(7000..8000),
            ip_type: 4,
        }
    }
}

/// Weight commitment for commit-reveal
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightCommitment {
    pub hotkey: String,
    pub netuid: u16,
    pub uids: Vec<u16>,
    pub commitment_hash: String,
    pub salt: Option<String>,
    pub revealed_weights: Option<Vec<u16>>,
    pub commit_block: u64,
    pub reveal_block: Option<u64>,
    pub revealed: bool,
}

impl WeightCommitment {
    pub fn new(
        hotkey: String,
        netuid: u16,
        uids: Vec<u16>,
        weights: Vec<u16>,
        salt: String,
        commit_block: u64,
    ) -> Self {
        // Calculate commitment hash
        let mut hasher = Sha256::new();
        hasher.update(hotkey.as_bytes());
        hasher.update(netuid.to_le_bytes());
        for uid in &uids {
            hasher.update(uid.to_le_bytes());
        }
        for weight in &weights {
            hasher.update(weight.to_le_bytes());
        }
        hasher.update(salt.as_bytes());

        let commitment_hash = format!("0x{}", hex::encode(hasher.finalize()));

        Self {
            hotkey,
            netuid,
            uids,
            commitment_hash,
            salt: Some(salt),
            revealed_weights: Some(weights),
            commit_block,
            reveal_block: None,
            revealed: false,
        }
    }

    pub fn verify_reveal(&self, uids: &[u16], weights: &[u16], salt: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.hotkey.as_bytes());
        hasher.update(self.netuid.to_le_bytes());
        for uid in uids {
            hasher.update(uid.to_le_bytes());
        }
        for weight in weights {
            hasher.update(weight.to_le_bytes());
        }
        hasher.update(salt.as_bytes());

        let calculated_hash = format!("0x{}", hex::encode(hasher.finalize()));
        calculated_hash == self.commitment_hash
    }
}

/// Mock metagraph state
pub struct MockMetagraph {
    pub validators: HashMap<u16, Validator>,
    pub validator_count: u16,
    pub netuid: u16,
    pub tempo: u16,
    pub max_uids: u16,
    pub min_stake: u64,
    pub total_stake: u64,
    pub weight_commitments: HashMap<String, WeightCommitment>, // hotkey -> commitment
    pub last_weight_update: HashMap<String, u64>,              // hotkey -> block
}

impl MockMetagraph {
    pub fn new(config: &Config) -> Self {
        let mut validators = HashMap::new();
        let mut total_stake = 0u64;

        for uid in 0..config.validator_count {
            let validator = Validator::with_uid(uid, config.netuid);
            total_stake += validator.stake;
            validators.insert(uid, validator);
        }

        Self {
            validators,
            validator_count: config.validator_count,
            netuid: config.netuid,
            tempo: config.tempo as u16,
            max_uids: config.validator_count,
            min_stake: config.min_stake,
            total_stake,
            weight_commitments: HashMap::new(),
            last_weight_update: HashMap::new(),
        }
    }

    /// Get validator by UID
    pub fn get_validator(&self, uid: u16) -> Option<&Validator> {
        self.validators.get(&uid)
    }

    /// Get validator by hotkey
    pub fn get_validator_by_hotkey(&self, hotkey: &str) -> Option<&Validator> {
        self.validators.values().find(|v| v.hotkey == hotkey)
    }

    /// Get all neuron info
    pub fn get_neurons(&self) -> Vec<serde_json::Value> {
        let mut neurons: Vec<_> = self
            .validators
            .values()
            .map(|v| v.to_neuron_info())
            .collect();

        // Sort by UID
        neurons.sort_by(|a, b| {
            let uid_a = a["uid"].as_u64().unwrap_or(0);
            let uid_b = b["uid"].as_u64().unwrap_or(0);
            uid_a.cmp(&uid_b)
        });

        neurons
    }

    /// Get neuron info lite for all validators
    pub fn get_neurons_lite(&self) -> Vec<serde_json::Value> {
        let mut neurons: Vec<_> = self
            .validators
            .values()
            .map(|v| v.to_neuron_info_lite())
            .collect();

        neurons.sort_by(|a, b| {
            let uid_a = a["uid"].as_u64().unwrap_or(0);
            let uid_b = b["uid"].as_u64().unwrap_or(0);
            uid_a.cmp(&uid_b)
        });

        neurons
    }

    /// Get metagraph summary
    pub fn get_summary(&self) -> serde_json::Value {
        let active_count = self.validators.values().filter(|v| v.active).count() as u16;
        let validator_count = self
            .validators
            .values()
            .filter(|v| v.validator_permit)
            .count();

        serde_json::json!({
            "netuid": self.netuid,
            "n": self.validator_count,
            "block": Utc::now().timestamp(),
            "tempo": self.tempo,
            "total_stake": self.total_stake,
            "min_stake": self.min_stake,
            "validators": validator_count,
            "active_validators": active_count,
            "pending_commits": self.weight_commitments.len(),
        })
    }

    /// Add weight commitment
    pub fn commit_weights(&mut self, commitment: WeightCommitment) -> Result<(), String> {
        // Check if validator exists
        if !self
            .validators
            .values()
            .any(|v| v.hotkey == commitment.hotkey)
        {
            return Err(format!("Hotkey {} not registered", commitment.hotkey));
        }

        self.weight_commitments
            .insert(commitment.hotkey.clone(), commitment);

        Ok(())
    }

    /// Reveal weights
    pub fn reveal_weights(
        &mut self,
        hotkey: &str,
        uids: Vec<u16>,
        weights: Vec<u16>,
        salt: String,
        block: u64,
    ) -> Result<(), String> {
        let commitment = self
            .weight_commitments
            .get_mut(hotkey)
            .ok_or_else(|| format!("No pending commitment for hotkey {}", hotkey))?;

        // Verify the reveal
        if !commitment.verify_reveal(&uids, &weights, &salt) {
            return Err("Invalid reveal: commitment hash mismatch".to_string());
        }

        commitment.revealed = true;
        commitment.revealed_weights = Some(weights);
        commitment.reveal_block = Some(block);

        self.last_weight_update.insert(hotkey.to_string(), block);

        Ok(())
    }

    /// Get pending commitments
    pub fn get_pending_commits(&self) -> Vec<&WeightCommitment> {
        self.weight_commitments
            .values()
            .filter(|c| !c.revealed)
            .collect()
    }

    /// Get revealed commitments
    pub fn get_revealed_commits(&self) -> Vec<&WeightCommitment> {
        self.weight_commitments
            .values()
            .filter(|c| c.revealed)
            .collect()
    }

    /// Clean old commitments (older than reveal_period blocks)
    pub fn clean_old_commits(&mut self, current_block: u64, reveal_period: u64) {
        let to_remove: Vec<String> = self
            .weight_commitments
            .iter()
            .filter(|(_, c)| current_block - c.commit_block > reveal_period && !c.revealed)
            .map(|(k, _)| k.clone())
            .collect();

        for key in to_remove {
            self.weight_commitments.remove(&key);
        }
    }
}

/// Encode to SS58 address
fn ss58_encode(prefix: u16, public_key: &[u8; 32]) -> String {
    // Simplified SS58 encoding (not cryptographically accurate but deterministic)
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(prefix.to_le_bytes());
    hasher.update(public_key);
    let hash: [u8; 32] = hasher.finalize().into();

    // Take first 2 bytes as checksum
    let checksum = &hash[0..2];

    // Combine: prefix (1 byte if < 64, 2 otherwise) + public_key + checksum
    let mut bytes = Vec::new();
    if prefix < 64 {
        bytes.push(prefix as u8);
    } else {
        bytes.push(((prefix & 0b1111_1100_0000_0000) >> 8) as u8 | 0b01000000);
        bytes.push((prefix & 0b1111_1111) as u8);
    }
    bytes.extend_from_slice(public_key);
    bytes.extend_from_slice(checksum);

    // Base58 encode
    bs58::encode(bytes).into_string()
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
    fn test_validator_generation() {
        let validator = Validator::with_uid(0, 100);
        assert_eq!(validator.uid, 0);
        assert!(!validator.hotkey.is_empty());
        assert!(!validator.coldkey.is_empty());
        assert!(validator.stake > 0);
    }

    #[test]
    fn test_metagraph_creation() {
        let metagraph = MockMetagraph::new(&test_config());
        assert_eq!(metagraph.validators.len(), 256);
        assert_eq!(metagraph.validator_count, 256);
    }

    #[test]
    fn test_get_validator_by_uid() {
        let metagraph = MockMetagraph::new(&test_config());
        let validator = metagraph.get_validator(0);
        assert!(validator.is_some());
        assert_eq!(validator.unwrap().uid, 0);
    }

    #[test]
    fn test_get_neurons() {
        let metagraph = MockMetagraph::new(&test_config());
        let neurons = metagraph.get_neurons();
        assert_eq!(neurons.len(), 256);
    }

    #[test]
    fn test_weight_commitment() {
        let hotkey = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_string();
        let uids = vec![1, 2, 3];
        let weights = vec![100, 200, 300];
        let salt = "random_salt".to_string();

        let commitment = WeightCommitment::new(
            hotkey.clone(),
            100,
            uids.clone(),
            weights.clone(),
            salt.clone(),
            1000,
        );

        assert!(commitment.verify_reveal(&uids, &weights, &salt));
        assert!(!commitment.verify_reveal(&uids, &weights, "wrong_salt"));
    }

    #[test]
    fn test_commit_reveal_flow() {
        let mut metagraph = MockMetagraph::new(&test_config());

        // Get a validator's hotkey
        let hotkey = metagraph.get_validator(0).unwrap().hotkey.clone();

        // Create commitment
        let uids = vec![1, 2, 3];
        let weights = vec![100, 200, 300];
        let salt = "test_salt".to_string();

        let commitment = WeightCommitment::new(
            hotkey.clone(),
            100,
            uids.clone(),
            weights.clone(),
            salt.clone(),
            1000,
        );

        // Commit
        metagraph.commit_weights(commitment).unwrap();
        assert_eq!(metagraph.get_pending_commits().len(), 1);

        // Reveal
        metagraph
            .reveal_weights(&hotkey, uids, weights, salt, 1010)
            .unwrap();
        assert_eq!(metagraph.get_revealed_commits().len(), 1);
        assert!(metagraph.get_pending_commits().is_empty());
    }

    #[test]
    fn test_stake_distribution() {
        let mut high_stake = 0;
        let mut _moderate_stake = 0;
        let mut low_stake = 0;

        for uid in 0..256 {
            let validator = Validator::with_uid(uid, 100);
            let stake_tao = validator.stake as f64 / 1_000_000_000.0;

            if stake_tao >= 1000.0 {
                high_stake += 1;
            } else if stake_tao >= 500.0 {
                _moderate_stake += 1;
            } else {
                low_stake += 1;
            }
        }

        // Verify distribution
        assert!(high_stake > 0, "Should have some high-stake validators");
        assert!(low_stake > 0, "Should have some low-stake validators");
    }
}
