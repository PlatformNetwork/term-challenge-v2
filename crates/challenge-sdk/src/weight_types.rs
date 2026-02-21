//! Weight Calculation Types
//!
//! Base types for weight calculation. The actual calculation logic
//! is implemented by each challenge according to their specific rules.

use platform_core::Hotkey;
use serde::{Deserialize, Serialize};

/// Evaluation result from a single validator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorEvaluation {
    /// Validator's hotkey
    pub validator_hotkey: Hotkey,
    /// Validator's stake in RAO
    pub validator_stake: u64,
    /// Submission hash being evaluated
    pub submission_hash: String,
    /// Content hash (for duplicate detection)
    pub content_hash: String,
    /// Miner's hotkey
    pub miner_hotkey: String,
    /// Miner's coldkey
    pub miner_coldkey: String,
    /// Score (0.0 - 1.0)
    pub score: f64,
    /// Tasks passed / total tasks
    pub tasks_passed: u32,
    pub tasks_total: u32,
    /// When the agent was originally submitted (for priority in case of similar scores)
    pub submitted_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp of this evaluation
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Epoch
    pub epoch: u64,
}

/// Aggregated score for a submission across all validators
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregatedScore {
    /// Submission hash
    pub submission_hash: String,
    /// Content hash (for duplicate detection)
    pub content_hash: String,
    /// Miner's hotkey
    pub miner_hotkey: String,
    /// Miner's coldkey
    pub miner_coldkey: String,
    /// Weighted average score
    pub weighted_score: f64,
    /// Number of validators who evaluated
    pub validator_count: u32,
    /// Total stake that evaluated
    pub total_stake: u64,
    /// Individual evaluations
    pub evaluations: Vec<ValidatorEvaluation>,
    /// Outlier validators (excluded from calculation)
    pub outliers: Vec<Hotkey>,
    /// Consensus confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Original submission timestamp (for priority in ties)
    pub submitted_at: chrono::DateTime<chrono::Utc>,
}

/// Weight assignment for a miner
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinerWeight {
    /// Miner's hotkey
    pub miner_hotkey: String,
    /// Miner's coldkey
    pub miner_coldkey: String,
    /// Submission hash
    pub submission_hash: String,
    /// Final weight (0.0 - 1.0, normalized)
    pub weight: f64,
    /// Raw weighted score before normalization
    pub raw_score: f64,
    /// Rank (1 = best)
    pub rank: u32,
}

/// Result of weight calculation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightCalculationResult {
    /// Epoch
    pub epoch: u64,
    /// Challenge ID
    pub challenge_id: String,
    /// Calculated weights
    pub weights: Vec<MinerWeight>,
    /// Current best agent
    pub best_agent: Option<BestAgent>,
    /// Previous best agent (from last epoch)
    pub previous_best: Option<BestAgent>,
    /// Whether a new best was found
    pub new_best_found: bool,
    /// Statistics
    pub stats: CalculationStats,
}

/// Best agent tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BestAgent {
    pub submission_hash: String,
    pub miner_hotkey: String,
    pub score: f64,
    pub epoch: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Statistics from calculation
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CalculationStats {
    pub total_submissions: u32,
    pub valid_submissions: u32,
    pub excluded_banned: u32,
    pub excluded_low_confidence: u32,
    pub outlier_validators: u32,
    pub total_evaluations: u32,
}

/// Configuration for weight calculation (challenge can customize)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightConfig {
    /// Minimum validators required to consider a submission
    pub min_validators: u32,
    /// Minimum stake percentage required (0.0 - 1.0)
    pub min_stake_percentage: f64,
    /// Z-score threshold for outlier detection
    pub outlier_zscore_threshold: f64,
    /// Maximum score variance allowed before flagging
    pub max_variance_threshold: f64,
    /// Improvement threshold for new best agent (e.g., 0.02 = 2%)
    pub improvement_threshold: f64,
    /// Minimum score to be considered for weights
    pub min_score_threshold: f64,
}

impl Default for WeightConfig {
    fn default() -> Self {
        Self {
            min_validators: 3,
            min_stake_percentage: 0.3,
            outlier_zscore_threshold: 2.5,
            max_variance_threshold: 0.15,
            improvement_threshold: 0.02,
            min_score_threshold: 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_config_default() {
        let config = WeightConfig::default();
        assert_eq!(config.min_validators, 3);
        assert_eq!(config.min_stake_percentage, 0.3);
        assert_eq!(config.outlier_zscore_threshold, 2.5);
        assert_eq!(config.max_variance_threshold, 0.15);
        assert_eq!(config.improvement_threshold, 0.02);
        assert_eq!(config.min_score_threshold, 0.01);
    }

    #[test]
    fn test_weight_config_clone() {
        let config = WeightConfig::default();
        let cloned = config.clone();
        assert_eq!(config.min_validators, cloned.min_validators);
    }
}
