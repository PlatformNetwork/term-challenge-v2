#![allow(dead_code, unused_variables, unused_imports)]
//! Epoch Management for Mini-Chain
//!
//! Handles:
//! - Epoch transitions
//! - Weight commit-reveal scheme (per mechanism)
//! - Weight aggregation and smoothing
//! - Emission distribution
//! - Mechanism-based weight grouping

mod aggregator;
mod commit_reveal;
mod manager;
mod mechanism_weights;

pub use aggregator::*;
pub use commit_reveal::*;
pub use manager::*;
pub use mechanism_weights::*;

use platform_challenge_sdk::{ChallengeId, WeightAssignment};
use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

/// Epoch configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochConfig {
    /// Blocks per epoch
    pub blocks_per_epoch: u64,
    /// Blocks for evaluation phase
    pub evaluation_blocks: u64,
    /// Blocks for commit phase
    pub commit_blocks: u64,
    /// Blocks for reveal phase
    pub reveal_blocks: u64,
    /// Minimum validators for weight consensus
    pub min_validators_for_consensus: usize,
    /// Weight smoothing factor
    pub weight_smoothing: f64,
}

impl Default for EpochConfig {
    fn default() -> Self {
        Self {
            blocks_per_epoch: 360,  // ~1 hour at 10s blocks
            evaluation_blocks: 270, // 75% for evaluation
            commit_blocks: 45,      // 12.5% for commit
            reveal_blocks: 45,      // 12.5% for reveal
            min_validators_for_consensus: 3,
            weight_smoothing: 0.3,
        }
    }
}

/// Epoch phase
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpochPhase {
    /// Validators are evaluating agents
    Evaluation,
    /// Validators commit weight hashes
    Commit,
    /// Validators reveal actual weights
    Reveal,
    /// Weights are being finalized
    Finalization,
}

impl std::fmt::Display for EpochPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpochPhase::Evaluation => write!(f, "evaluation"),
            EpochPhase::Commit => write!(f, "commit"),
            EpochPhase::Reveal => write!(f, "reveal"),
            EpochPhase::Finalization => write!(f, "finalization"),
        }
    }
}

/// Current epoch state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochState {
    /// Current epoch number
    pub epoch: u64,
    /// Current phase
    pub phase: EpochPhase,
    /// Start block of this epoch
    pub start_block: u64,
    /// Current block
    pub current_block: u64,
    /// Blocks remaining in current phase
    pub blocks_remaining: u64,
}

impl EpochState {
    pub fn new(epoch: u64, start_block: u64, config: &EpochConfig) -> Self {
        Self {
            epoch,
            phase: EpochPhase::Evaluation,
            start_block,
            current_block: start_block,
            blocks_remaining: config.evaluation_blocks,
        }
    }
}

/// Weight commitment from a validator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightCommitment {
    pub validator: Hotkey,
    pub challenge_id: ChallengeId,
    pub epoch: u64,
    pub commitment_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Weight reveal from a validator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightReveal {
    pub validator: Hotkey,
    pub challenge_id: ChallengeId,
    pub epoch: u64,
    pub weights: Vec<WeightAssignment>,
    pub secret: Vec<u8>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Finalized weights for an epoch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizedWeights {
    pub challenge_id: ChallengeId,
    pub epoch: u64,
    pub weights: Vec<WeightAssignment>,
    pub participating_validators: Vec<Hotkey>,
    pub excluded_validators: Vec<Hotkey>, // Malicious or non-participating
    pub smoothing_applied: f64,
    pub finalized_at: chrono::DateTime<chrono::Utc>,
}

/// Emission distribution for an epoch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmissionDistribution {
    pub epoch: u64,
    pub total_emission: u64,
    pub distributions: Vec<AgentEmission>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Emission for a single miner
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEmission {
    /// Miner hotkey (SS58 address)
    pub hotkey: String,
    pub weight: f64,
    pub emission: u64,
    pub challenge_id: ChallengeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_config_default() {
        let config = EpochConfig::default();
        assert_eq!(config.blocks_per_epoch, 360);
        assert_eq!(config.evaluation_blocks, 270);
        assert_eq!(config.commit_blocks, 45);
        assert_eq!(config.reveal_blocks, 45);
        assert_eq!(config.min_validators_for_consensus, 3);
        assert_eq!(config.weight_smoothing, 0.3);
    }

    #[test]
    fn test_epoch_phase_display() {
        assert_eq!(EpochPhase::Evaluation.to_string(), "evaluation");
        assert_eq!(EpochPhase::Commit.to_string(), "commit");
        assert_eq!(EpochPhase::Reveal.to_string(), "reveal");
        assert_eq!(EpochPhase::Finalization.to_string(), "finalization");
    }

    #[test]
    fn test_epoch_phase_equality() {
        assert_eq!(EpochPhase::Evaluation, EpochPhase::Evaluation);
        assert_ne!(EpochPhase::Evaluation, EpochPhase::Commit);
    }

    #[test]
    fn test_epoch_state_new() {
        let config = EpochConfig::default();
        let state = EpochState::new(5, 1800, &config);

        assert_eq!(state.epoch, 5);
        assert_eq!(state.phase, EpochPhase::Evaluation);
        assert_eq!(state.start_block, 1800);
        assert_eq!(state.current_block, 1800);
        assert_eq!(state.blocks_remaining, config.evaluation_blocks);
    }
}
