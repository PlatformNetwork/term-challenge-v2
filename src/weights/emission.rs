//! Emission and weight calculation.
//!
//! Distributes rewards across competitions with configurable strategies
//! (Linear, Softmax, WinnerTakesAll, Ranked, Quadratic).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum weight value for Bittensor (u16::MAX)
pub const MAX_WEIGHT: u16 = 65535;

/// Minimum weight to be considered valid
pub const MIN_WEIGHT: u16 = 1;

// ============================================================================
// Emission Configuration
// ============================================================================

/// Emission allocation for a competition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionAllocation {
    /// Competition ID
    pub competition_id: String,
    /// Percentage of total emission (0.0 - 100.0)
    /// Sum of all active competitions must equal 100%
    pub emission_percent: f64,
    /// Whether this competition is currently active for emission
    pub active: bool,
    /// Priority for weight calculation (higher = processed first)
    pub priority: u32,
    /// Minimum score threshold to receive emission
    pub min_score_threshold: f64,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Global emission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionConfig {
    /// Allocations per competition
    pub allocations: HashMap<String, EmissionAllocation>,
    /// Default emission for unallocated percentage (goes to default competition)
    pub default_competition_id: Option<String>,
    /// Whether to auto-rebalance when competitions are added/removed
    pub auto_rebalance: bool,
    /// Epoch when this config was last updated
    pub last_update_epoch: u64,
}

impl Default for EmissionConfig {
    fn default() -> Self {
        Self {
            allocations: HashMap::new(),
            default_competition_id: None,
            auto_rebalance: true,
            last_update_epoch: 0,
        }
    }
}

impl EmissionConfig {
    /// Get total allocated emission percentage
    pub fn total_allocated(&self) -> f64 {
        self.allocations
            .values()
            .filter(|a| a.active)
            .map(|a| a.emission_percent)
            .sum()
    }

    /// Check if allocations sum to 100%
    pub fn is_valid(&self) -> bool {
        let total = self.total_allocated();
        (total - 100.0).abs() < 0.001 // Allow small floating point error
    }

    /// Get unallocated emission percentage
    pub fn unallocated(&self) -> f64 {
        100.0 - self.total_allocated()
    }

    /// Add or update competition allocation
    pub fn set_allocation(&mut self, allocation: EmissionAllocation) -> Result<(), String> {
        let competition_id = allocation.competition_id.clone();

        // Calculate what total would be with this new allocation
        let current_for_this = self
            .allocations
            .get(&competition_id)
            .filter(|a| a.active)
            .map(|a| a.emission_percent)
            .unwrap_or(0.0);

        let new_total = self.total_allocated() - current_for_this
            + if allocation.active {
                allocation.emission_percent
            } else {
                0.0
            };

        if new_total > 100.0 + 0.001 {
            return Err(format!(
                "Total emission would exceed 100%: {:.2}% (max 100%)",
                new_total
            ));
        }

        self.allocations.insert(competition_id, allocation);
        Ok(())
    }

    /// Remove competition allocation
    pub fn remove_allocation(&mut self, competition_id: &str) {
        self.allocations.remove(competition_id);
    }

    /// Auto-rebalance allocations to sum to 100%
    pub fn rebalance(&mut self) {
        let active_count = self.allocations.values().filter(|a| a.active).count();
        if active_count == 0 {
            return;
        }

        let equal_share = 100.0 / active_count as f64;
        for allocation in self.allocations.values_mut() {
            if allocation.active {
                allocation.emission_percent = equal_share;
                allocation.updated_at = Utc::now();
            }
        }
    }
}

// ============================================================================
// Miner Scores
// ============================================================================

/// Score for a miner in a specific competition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerScore {
    pub miner_uid: u16,
    pub miner_hotkey: String,
    pub competition_id: String,
    pub score: f64,
    pub tasks_completed: u32,
    pub tasks_total: u32,
    pub rank: u32,
    pub evaluated_at: DateTime<Utc>,
}

/// Aggregated scores across all competitions for a miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMinerScore {
    pub miner_uid: u16,
    pub miner_hotkey: String,
    /// Scores per competition
    pub competition_scores: HashMap<String, f64>,
    /// Weighted aggregate score (0.0 - 1.0)
    pub weighted_score: f64,
    /// Final weight for Bittensor (0 - 65535)
    pub final_weight: u16,
}

// ============================================================================
// Weight Calculator
// ============================================================================

/// Strategy for calculating weights from scores
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeightStrategy {
    /// Linear: weight proportional to score
    #[default]
    Linear,
    /// Softmax: exponential emphasis on top performers
    Softmax { temperature: u32 }, // temperature * 100 (e.g., 100 = 1.0)
    /// Winner takes all: top N get all emission
    WinnerTakesAll { top_n: u32 },
    /// Ranked: fixed weights by rank
    Ranked,
    /// Quadratic: score squared (more reward to top)
    Quadratic,
}

/// Weight calculation result for a single competition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionWeights {
    pub competition_id: String,
    pub emission_percent: f64,
    /// Weights for each miner UID (before applying emission percentage)
    pub raw_weights: HashMap<u16, u16>,
    /// Weights after applying emission percentage
    pub weighted_weights: HashMap<u16, f64>,
    pub strategy_used: WeightStrategy,
    pub calculated_at: DateTime<Utc>,
}

/// Final aggregated weights for all miners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalWeights {
    /// Final weights to submit to Bittensor (UID -> weight)
    pub weights: HashMap<u16, u16>,
    /// Competition breakdown
    pub competition_breakdown: Vec<CompetitionWeights>,
    /// Total miners with non-zero weights
    pub miners_with_weights: usize,
    /// Epoch for these weights
    pub epoch: u64,
    pub calculated_at: DateTime<Utc>,
}

/// Main weight calculator
pub struct WeightCalculator {
    /// Emission configuration
    emission_config: EmissionConfig,
    /// Default weight strategy
    default_strategy: WeightStrategy,
    /// Maximum weight cap per miner (percentage of total)
    max_weight_cap_percent: f64,
}

impl WeightCalculator {
    pub fn new(emission_config: EmissionConfig) -> Self {
        Self {
            emission_config,
            default_strategy: WeightStrategy::Linear,
            max_weight_cap_percent: 50.0, // No single miner can get more than 50%
        }
    }

    pub fn with_strategy(mut self, strategy: WeightStrategy) -> Self {
        self.default_strategy = strategy;
        self
    }

    pub fn with_max_cap(mut self, cap_percent: f64) -> Self {
        self.max_weight_cap_percent = cap_percent;
        self
    }

    /// Calculate weights for a single competition
    pub fn calculate_competition_weights(
        &self,
        competition_id: &str,
        scores: &[MinerScore],
        strategy: Option<WeightStrategy>,
    ) -> Result<CompetitionWeights, String> {
        let allocation = self
            .emission_config
            .allocations
            .get(competition_id)
            .ok_or_else(|| {
                format!(
                    "Competition {} not found in emission config",
                    competition_id
                )
            })?;

        if !allocation.active {
            return Err(format!("Competition {} is not active", competition_id));
        }

        let strategy = strategy.unwrap_or(self.default_strategy);

        // Filter scores above threshold
        let valid_scores: Vec<_> = scores
            .iter()
            .filter(|s| s.score >= allocation.min_score_threshold)
            .collect();

        if valid_scores.is_empty() {
            return Ok(CompetitionWeights {
                competition_id: competition_id.to_string(),
                emission_percent: allocation.emission_percent,
                raw_weights: HashMap::new(),
                weighted_weights: HashMap::new(),
                strategy_used: strategy,
                calculated_at: Utc::now(),
            });
        }

        // Calculate raw weights based on strategy
        let raw_weights = match strategy {
            WeightStrategy::Linear => self.calculate_linear(&valid_scores),
            WeightStrategy::Softmax { temperature } => {
                self.calculate_softmax(&valid_scores, temperature as f64 / 100.0)
            }
            WeightStrategy::WinnerTakesAll { top_n } => {
                self.calculate_winner_takes_all(&valid_scores, top_n as usize)
            }
            WeightStrategy::Ranked => self.calculate_ranked(&valid_scores),
            WeightStrategy::Quadratic => self.calculate_quadratic(&valid_scores),
        };

        // Apply emission percentage
        let weighted_weights: HashMap<u16, f64> = raw_weights
            .iter()
            .map(|(uid, weight)| {
                let weighted = (*weight as f64 / MAX_WEIGHT as f64) * allocation.emission_percent;
                (*uid, weighted)
            })
            .collect();

        Ok(CompetitionWeights {
            competition_id: competition_id.to_string(),
            emission_percent: allocation.emission_percent,
            raw_weights,
            weighted_weights,
            strategy_used: strategy,
            calculated_at: Utc::now(),
        })
    }

    /// Calculate final aggregated weights across all competitions
    pub fn calculate_final_weights(
        &self,
        all_scores: &HashMap<String, Vec<MinerScore>>,
        epoch: u64,
    ) -> Result<FinalWeights, String> {
        // Validate emission config
        if !self.emission_config.is_valid() {
            return Err(format!(
                "Invalid emission config: total is {:.2}%, should be 100%",
                self.emission_config.total_allocated()
            ));
        }

        let mut competition_weights = Vec::new();
        let mut aggregated: HashMap<u16, f64> = HashMap::new();

        // Calculate weights for each competition
        for (competition_id, allocation) in &self.emission_config.allocations {
            if !allocation.active {
                continue;
            }

            let scores = all_scores.get(competition_id).cloned().unwrap_or_default();

            match self.calculate_competition_weights(competition_id, &scores, None) {
                Ok(comp_weights) => {
                    // Aggregate weighted weights
                    for (uid, weighted_weight) in &comp_weights.weighted_weights {
                        *aggregated.entry(*uid).or_insert(0.0) += weighted_weight;
                    }
                    competition_weights.push(comp_weights);
                }
                Err(e) => {
                    tracing::warn!("Failed to calculate weights for {}: {}", competition_id, e);
                }
            }
        }

        // Apply weight cap
        let total_weight: f64 = aggregated.values().sum();
        let max_allowed = total_weight * (self.max_weight_cap_percent / 100.0);

        let mut capped: HashMap<u16, f64> = HashMap::new();
        let mut excess = 0.0;
        let mut uncapped_count = 0;

        for (uid, weight) in &aggregated {
            if *weight > max_allowed {
                capped.insert(*uid, max_allowed);
                excess += weight - max_allowed;
            } else {
                capped.insert(*uid, *weight);
                uncapped_count += 1;
            }
        }

        // Redistribute excess to uncapped miners proportionally
        if excess > 0.0 && uncapped_count > 0 {
            let uncapped_total: f64 = capped
                .iter()
                .filter(|(uid, w)| {
                    **w < max_allowed && aggregated.get(uid).unwrap_or(&0.0) < &max_allowed
                })
                .map(|(_, w)| w)
                .sum();

            if uncapped_total > 0.0 {
                for (uid, weight) in capped.iter_mut() {
                    if *weight < max_allowed {
                        let proportion = *weight / uncapped_total;
                        *weight += excess * proportion;
                    }
                }
            }
        }

        // Normalize to u16 weights (0 - 65535)
        let final_total: f64 = capped.values().sum();
        let final_weights: HashMap<u16, u16> = if final_total > 0.0 {
            capped
                .iter()
                .map(|(uid, weight)| {
                    let normalized = (weight / final_total * MAX_WEIGHT as f64).round() as u16;
                    (*uid, normalized.max(MIN_WEIGHT))
                })
                .filter(|(_, w)| *w > 0)
                .collect()
        } else {
            HashMap::new()
        };

        Ok(FinalWeights {
            weights: final_weights.clone(),
            competition_breakdown: competition_weights,
            miners_with_weights: final_weights.len(),
            epoch,
            calculated_at: Utc::now(),
        })
    }

    // ==================== Strategy Implementations ====================

    fn calculate_linear(&self, scores: &[&MinerScore]) -> HashMap<u16, u16> {
        let total_score: f64 = scores.iter().map(|s| s.score).sum();
        if total_score == 0.0 {
            return HashMap::new();
        }

        scores
            .iter()
            .map(|s| {
                let weight = ((s.score / total_score) * MAX_WEIGHT as f64).round() as u16;
                (s.miner_uid, weight.max(MIN_WEIGHT))
            })
            .collect()
    }

    fn calculate_softmax(&self, scores: &[&MinerScore], temperature: f64) -> HashMap<u16, u16> {
        let temp = if temperature <= 0.0 { 1.0 } else { temperature };

        // Calculate exp(score/temp) for each
        let exp_scores: Vec<(u16, f64)> = scores
            .iter()
            .map(|s| (s.miner_uid, (s.score / temp).exp()))
            .collect();

        let total_exp: f64 = exp_scores.iter().map(|(_, e)| e).sum();
        if total_exp == 0.0 {
            return HashMap::new();
        }

        exp_scores
            .iter()
            .map(|(uid, exp_score)| {
                let weight = ((exp_score / total_exp) * MAX_WEIGHT as f64).round() as u16;
                (*uid, weight.max(MIN_WEIGHT))
            })
            .collect()
    }

    fn calculate_winner_takes_all(
        &self,
        scores: &[&MinerScore],
        top_n: usize,
    ) -> HashMap<u16, u16> {
        let mut sorted: Vec<_> = scores.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let winners: Vec<_> = sorted.into_iter().take(top_n).collect();
        if winners.is_empty() {
            return HashMap::new();
        }

        let weight_per_winner = MAX_WEIGHT / winners.len() as u16;
        winners
            .iter()
            .map(|s| (s.miner_uid, weight_per_winner.max(MIN_WEIGHT)))
            .collect()
    }

    fn calculate_ranked(&self, scores: &[&MinerScore]) -> HashMap<u16, u16> {
        let mut sorted: Vec<_> = scores.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let n = sorted.len();
        if n == 0 {
            return HashMap::new();
        }

        // Weight decreases by rank: rank 1 gets n points, rank 2 gets n-1, etc.
        let total_points: usize = (1..=n).sum();

        sorted
            .iter()
            .enumerate()
            .map(|(rank, s)| {
                let points = n - rank;
                let weight =
                    ((points as f64 / total_points as f64) * MAX_WEIGHT as f64).round() as u16;
                (s.miner_uid, weight.max(MIN_WEIGHT))
            })
            .collect()
    }

    fn calculate_quadratic(&self, scores: &[&MinerScore]) -> HashMap<u16, u16> {
        let total_squared: f64 = scores.iter().map(|s| s.score * s.score).sum();
        if total_squared == 0.0 {
            return HashMap::new();
        }

        scores
            .iter()
            .map(|s| {
                let squared = s.score * s.score;
                let weight = ((squared / total_squared) * MAX_WEIGHT as f64).round() as u16;
                (s.miner_uid, weight.max(MIN_WEIGHT))
            })
            .collect()
    }
}

// ============================================================================
// Emission Manager (integrates with SudoController)
// ============================================================================

/// Manages emission allocations and weight calculations
pub struct EmissionManager {
    config: EmissionConfig,
    calculator: WeightCalculator,
    /// Historical weights by epoch
    weight_history: HashMap<u64, FinalWeights>,
}

impl EmissionManager {
    pub fn new() -> Self {
        let config = EmissionConfig::default();
        let calculator = WeightCalculator::new(config.clone());
        Self {
            config,
            calculator,
            weight_history: HashMap::new(),
        }
    }

    /// Add a competition with emission percentage
    pub fn add_competition(
        &mut self,
        competition_id: String,
        emission_percent: f64,
        min_score_threshold: f64,
    ) -> Result<(), String> {
        if emission_percent <= 0.0 || emission_percent > 100.0 {
            return Err("Emission percent must be between 0 and 100".into());
        }

        let allocation = EmissionAllocation {
            competition_id: competition_id.clone(),
            emission_percent,
            active: true,
            priority: self.config.allocations.len() as u32,
            min_score_threshold,
            updated_at: Utc::now(),
        };

        self.config.set_allocation(allocation)?;
        self.calculator = WeightCalculator::new(self.config.clone());
        Ok(())
    }

    /// Update competition emission percentage
    pub fn update_emission(
        &mut self,
        competition_id: &str,
        emission_percent: f64,
    ) -> Result<(), String> {
        // First check if competition exists
        if !self.config.allocations.contains_key(competition_id) {
            return Err(format!("Competition {} not found", competition_id));
        }

        // Check if new total would be valid
        let other_total: f64 = self
            .config
            .allocations
            .values()
            .filter(|a| a.active && a.competition_id != competition_id)
            .map(|a| a.emission_percent)
            .sum();

        if other_total + emission_percent > 100.0 + 0.001 {
            return Err(format!(
                "Total emission would exceed 100%: {:.2}%",
                other_total + emission_percent
            ));
        }

        // Now update
        if let Some(allocation) = self.config.allocations.get_mut(competition_id) {
            allocation.emission_percent = emission_percent;
            allocation.updated_at = Utc::now();
        }
        self.calculator = WeightCalculator::new(self.config.clone());
        Ok(())
    }

    /// Remove competition and optionally redistribute its emission
    pub fn remove_competition(
        &mut self,
        competition_id: &str,
        redistribute: bool,
    ) -> Result<(), String> {
        let removed_emission = self
            .config
            .allocations
            .get(competition_id)
            .filter(|a| a.active)
            .map(|a| a.emission_percent)
            .unwrap_or(0.0);

        self.config.remove_allocation(competition_id);

        if redistribute && removed_emission > 0.0 {
            self.config.rebalance();
        }

        self.calculator = WeightCalculator::new(self.config.clone());
        Ok(())
    }

    /// Set competition active/inactive
    pub fn set_competition_active(
        &mut self,
        competition_id: &str,
        active: bool,
    ) -> Result<(), String> {
        let allocation = self
            .config
            .allocations
            .get_mut(competition_id)
            .ok_or_else(|| format!("Competition {} not found", competition_id))?;

        allocation.active = active;
        allocation.updated_at = Utc::now();
        self.calculator = WeightCalculator::new(self.config.clone());
        Ok(())
    }

    /// Calculate weights for the current epoch
    pub fn calculate_weights(
        &mut self,
        all_scores: &HashMap<String, Vec<MinerScore>>,
        epoch: u64,
    ) -> Result<FinalWeights, String> {
        let weights = self.calculator.calculate_final_weights(all_scores, epoch)?;
        self.weight_history.insert(epoch, weights.clone());
        Ok(weights)
    }

    /// Get emission config summary
    pub fn get_emission_summary(&self) -> EmissionSummary {
        let allocations: Vec<_> = self
            .config
            .allocations
            .values()
            .map(|a| AllocationSummary {
                competition_id: a.competition_id.clone(),
                emission_percent: a.emission_percent,
                active: a.active,
            })
            .collect();

        EmissionSummary {
            total_allocated: self.config.total_allocated(),
            unallocated: self.config.unallocated(),
            is_valid: self.config.is_valid(),
            allocations,
        }
    }

    /// Get historical weights for an epoch
    pub fn get_weights_for_epoch(&self, epoch: u64) -> Option<&FinalWeights> {
        self.weight_history.get(&epoch)
    }
}

impl Default for EmissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of emission allocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionSummary {
    pub total_allocated: f64,
    pub unallocated: f64,
    pub is_valid: bool,
    pub allocations: Vec<AllocationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationSummary {
    pub competition_id: String,
    pub emission_percent: f64,
    pub active: bool,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn create_test_scores(competition_id: &str) -> Vec<MinerScore> {
        vec![
            MinerScore {
                miner_uid: 1,
                miner_hotkey: "miner1".to_string(),
                competition_id: competition_id.to_string(),
                score: 0.95,
                tasks_completed: 9,
                tasks_total: 10,
                rank: 1,
                evaluated_at: Utc::now(),
            },
            MinerScore {
                miner_uid: 2,
                miner_hotkey: "miner2".to_string(),
                competition_id: competition_id.to_string(),
                score: 0.80,
                tasks_completed: 8,
                tasks_total: 10,
                rank: 2,
                evaluated_at: Utc::now(),
            },
            MinerScore {
                miner_uid: 3,
                miner_hotkey: "miner3".to_string(),
                competition_id: competition_id.to_string(),
                score: 0.60,
                tasks_completed: 6,
                tasks_total: 10,
                rank: 3,
                evaluated_at: Utc::now(),
            },
        ]
    }

    #[test]
    fn test_emission_config_validation() {
        let mut config = EmissionConfig::default();

        // Empty config should not be valid (0% allocated)
        assert!(!config.is_valid());

        // Add 100% allocation
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        assert!(config.is_valid());
        assert_eq!(config.total_allocated(), 100.0);
    }

    #[test]
    fn test_emission_split() {
        let mut config = EmissionConfig::default();

        // 60% to comp1, 40% to comp2
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 60.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp2".to_string(),
                emission_percent: 40.0,
                active: true,
                priority: 1,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        assert!(config.is_valid());
        assert_eq!(config.total_allocated(), 100.0);
    }

    #[test]
    fn test_emission_overflow() {
        let mut config = EmissionConfig::default();

        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 70.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // This should fail - would exceed 100%
        let result = config.set_allocation(EmissionAllocation {
            competition_id: "comp2".to_string(),
            emission_percent: 50.0,
            active: true,
            priority: 1,
            min_score_threshold: 0.0,
            updated_at: Utc::now(),
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_weight_calculator_linear() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("comp1");

        let weights = calculator
            .calculate_competition_weights("comp1", &scores, Some(WeightStrategy::Linear))
            .unwrap();

        assert!(!weights.raw_weights.is_empty());

        // Higher score should get higher weight
        assert!(weights.raw_weights.get(&1).unwrap() > weights.raw_weights.get(&2).unwrap());
        assert!(weights.raw_weights.get(&2).unwrap() > weights.raw_weights.get(&3).unwrap());
    }

    #[test]
    fn test_weight_calculator_winner_takes_all() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("comp1");

        let weights = calculator
            .calculate_competition_weights(
                "comp1",
                &scores,
                Some(WeightStrategy::WinnerTakesAll { top_n: 1 }),
            )
            .unwrap();

        // Only top 1 should have weight
        assert_eq!(weights.raw_weights.len(), 1);
        assert!(weights.raw_weights.contains_key(&1)); // miner1 is top scorer
    }

    #[test]
    fn test_multi_competition_weights() {
        let mut manager = EmissionManager::new();

        // Add two competitions: 60% and 40%
        manager
            .add_competition("comp1".to_string(), 60.0, 0.0)
            .unwrap();
        manager
            .add_competition("comp2".to_string(), 40.0, 0.0)
            .unwrap();

        let summary = manager.get_emission_summary();
        assert!(summary.is_valid);
        assert_eq!(summary.total_allocated, 100.0);

        // Create scores for both competitions
        let mut all_scores = HashMap::new();
        all_scores.insert("comp1".to_string(), create_test_scores("comp1"));
        all_scores.insert(
            "comp2".to_string(),
            vec![
                MinerScore {
                    miner_uid: 1,
                    miner_hotkey: "miner1".to_string(),
                    competition_id: "comp2".to_string(),
                    score: 0.50, // Different score in comp2
                    tasks_completed: 5,
                    tasks_total: 10,
                    rank: 2,
                    evaluated_at: Utc::now(),
                },
                MinerScore {
                    miner_uid: 4, // Different miner
                    miner_hotkey: "miner4".to_string(),
                    competition_id: "comp2".to_string(),
                    score: 0.90,
                    tasks_completed: 9,
                    tasks_total: 10,
                    rank: 1,
                    evaluated_at: Utc::now(),
                },
            ],
        );

        let weights = manager.calculate_weights(&all_scores, 100).unwrap();

        // All miners should have weights
        assert!(weights.weights.contains_key(&1)); // In both competitions
        assert!(weights.weights.contains_key(&2)); // Only in comp1
        assert!(weights.weights.contains_key(&3)); // Only in comp1
        assert!(weights.weights.contains_key(&4)); // Only in comp2

        // Total weights should sum to approximately MAX_WEIGHT
        let total: u32 = weights.weights.values().map(|w| *w as u32).sum();
        assert!(total > 60000 && total <= MAX_WEIGHT as u32 + 10);
    }

    #[test]
    fn test_rebalance() {
        let mut config = EmissionConfig {
            auto_rebalance: true,
            ..Default::default()
        };

        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 30.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp2".to_string(),
                emission_percent: 20.0,
                active: true,
                priority: 1,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // Before rebalance: 30% + 20% = 50%
        assert_eq!(config.total_allocated(), 50.0);

        // Rebalance to equal shares
        config.rebalance();

        // After rebalance: 50% + 50% = 100%
        assert!(config.is_valid());
        assert_eq!(
            config.allocations.get("comp1").unwrap().emission_percent,
            50.0
        );
        assert_eq!(
            config.allocations.get("comp2").unwrap().emission_percent,
            50.0
        );
    }

    #[test]
    fn test_weight_cap() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // One miner with 99% of the score
        let scores = vec![
            MinerScore {
                miner_uid: 1,
                miner_hotkey: "whale".to_string(),
                competition_id: "comp1".to_string(),
                score: 0.99,
                tasks_completed: 99,
                tasks_total: 100,
                rank: 1,
                evaluated_at: Utc::now(),
            },
            MinerScore {
                miner_uid: 2,
                miner_hotkey: "small".to_string(),
                competition_id: "comp1".to_string(),
                score: 0.01,
                tasks_completed: 1,
                tasks_total: 100,
                rank: 2,
                evaluated_at: Utc::now(),
            },
        ];

        let calculator = WeightCalculator::new(config).with_max_cap(50.0); // Max 50% per miner

        let mut all_scores = HashMap::new();
        all_scores.insert("comp1".to_string(), scores);

        let weights = calculator
            .calculate_final_weights(&all_scores, 100)
            .unwrap();

        // Whale should be capped
        let whale_weight = *weights.weights.get(&1).unwrap_or(&0);
        let total: u32 = weights.weights.values().map(|w| *w as u32).sum();
        let whale_percent = (whale_weight as f64 / total as f64) * 100.0;

        assert!(
            whale_percent <= 51.0,
            "Whale got {:.1}% but max is 50%",
            whale_percent
        );
    }

    // =========================================================================
    // Constants tests
    // =========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(MAX_WEIGHT, 65535);
        assert_eq!(MIN_WEIGHT, 1);
    }

    // =========================================================================
    // EmissionAllocation tests
    // =========================================================================

    #[test]
    fn test_emission_allocation_serialization() {
        let allocation = EmissionAllocation {
            competition_id: "test".to_string(),
            emission_percent: 50.0,
            active: true,
            priority: 1,
            min_score_threshold: 0.1,
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&allocation).unwrap();
        let deserialized: EmissionAllocation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.competition_id, "test");
        assert_eq!(deserialized.emission_percent, 50.0);
        assert!(deserialized.active);
        assert_eq!(deserialized.priority, 1);
    }

    #[test]
    fn test_emission_allocation_clone() {
        let allocation = EmissionAllocation {
            competition_id: "clone_test".to_string(),
            emission_percent: 75.0,
            active: false,
            priority: 5,
            min_score_threshold: 0.5,
            updated_at: Utc::now(),
        };

        let cloned = allocation.clone();
        assert_eq!(allocation.competition_id, cloned.competition_id);
        assert_eq!(allocation.emission_percent, cloned.emission_percent);
        assert_eq!(allocation.active, cloned.active);
    }

    #[test]
    fn test_emission_allocation_debug() {
        let allocation = EmissionAllocation {
            competition_id: "debug".to_string(),
            emission_percent: 25.0,
            active: true,
            priority: 0,
            min_score_threshold: 0.0,
            updated_at: Utc::now(),
        };

        let debug = format!("{:?}", allocation);
        assert!(debug.contains("EmissionAllocation"));
        assert!(debug.contains("debug"));
    }

    // =========================================================================
    // EmissionConfig tests
    // =========================================================================

    #[test]
    fn test_emission_config_default() {
        let config = EmissionConfig::default();
        assert!(config.allocations.is_empty());
        assert!(config.default_competition_id.is_none());
        assert!(config.auto_rebalance);
        assert_eq!(config.last_update_epoch, 0);
    }

    #[test]
    fn test_emission_config_unallocated() {
        let mut config = EmissionConfig::default();
        assert_eq!(config.unallocated(), 100.0);

        config
            .set_allocation(EmissionAllocation {
                competition_id: "c1".to_string(),
                emission_percent: 60.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        assert_eq!(config.unallocated(), 40.0);
    }

    #[test]
    fn test_emission_config_remove_allocation() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "to_remove".to_string(),
                emission_percent: 50.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        assert!(config.allocations.contains_key("to_remove"));
        config.remove_allocation("to_remove");
        assert!(!config.allocations.contains_key("to_remove"));
    }

    #[test]
    fn test_emission_config_inactive_allocation() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "inactive".to_string(),
                emission_percent: 50.0,
                active: false, // Inactive
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // Inactive allocation shouldn't count toward total
        assert_eq!(config.total_allocated(), 0.0);
    }

    #[test]
    fn test_emission_config_serialization() {
        let mut config = EmissionConfig::default();
        config.default_competition_id = Some("default".to_string());
        config.auto_rebalance = false;
        config.last_update_epoch = 100;

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EmissionConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.default_competition_id,
            Some("default".to_string())
        );
        assert!(!deserialized.auto_rebalance);
        assert_eq!(deserialized.last_update_epoch, 100);
    }

    #[test]
    fn test_emission_config_clone() {
        let mut config = EmissionConfig::default();
        config.last_update_epoch = 50;
        let cloned = config.clone();
        assert_eq!(config.last_update_epoch, cloned.last_update_epoch);
    }

    #[test]
    fn test_emission_config_debug() {
        let config = EmissionConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("EmissionConfig"));
    }

    #[test]
    fn test_emission_config_update_existing_allocation() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 60.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // Update the same competition
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 80.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        assert_eq!(
            config.allocations.get("comp1").unwrap().emission_percent,
            80.0
        );
    }

    #[test]
    fn test_emission_config_rebalance_no_active() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "inactive".to_string(),
                emission_percent: 50.0,
                active: false,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        // Rebalance with no active allocations should do nothing
        config.rebalance();
        assert_eq!(
            config.allocations.get("inactive").unwrap().emission_percent,
            50.0
        );
    }

    // =========================================================================
    // MinerScore tests
    // =========================================================================

    #[test]
    fn test_miner_score_serialization() {
        let score = MinerScore {
            miner_uid: 42,
            miner_hotkey: "5Grwva...".to_string(),
            competition_id: "term".to_string(),
            score: 0.85,
            tasks_completed: 17,
            tasks_total: 20,
            rank: 5,
            evaluated_at: Utc::now(),
        };

        let json = serde_json::to_string(&score).unwrap();
        let deserialized: MinerScore = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.miner_uid, 42);
        assert_eq!(deserialized.score, 0.85);
        assert_eq!(deserialized.rank, 5);
    }

    #[test]
    fn test_miner_score_clone() {
        let score = MinerScore {
            miner_uid: 1,
            miner_hotkey: "miner".to_string(),
            competition_id: "comp".to_string(),
            score: 0.5,
            tasks_completed: 5,
            tasks_total: 10,
            rank: 1,
            evaluated_at: Utc::now(),
        };

        let cloned = score.clone();
        assert_eq!(score.miner_uid, cloned.miner_uid);
        assert_eq!(score.score, cloned.score);
    }

    #[test]
    fn test_miner_score_debug() {
        let score = MinerScore {
            miner_uid: 1,
            miner_hotkey: "debug_miner".to_string(),
            competition_id: "comp".to_string(),
            score: 0.9,
            tasks_completed: 9,
            tasks_total: 10,
            rank: 1,
            evaluated_at: Utc::now(),
        };

        let debug = format!("{:?}", score);
        assert!(debug.contains("MinerScore"));
        assert!(debug.contains("debug_miner"));
    }

    // =========================================================================
    // AggregatedMinerScore tests
    // =========================================================================

    #[test]
    fn test_aggregated_miner_score_serialization() {
        let mut competition_scores = HashMap::new();
        competition_scores.insert("comp1".to_string(), 0.9);
        competition_scores.insert("comp2".to_string(), 0.8);

        let agg = AggregatedMinerScore {
            miner_uid: 10,
            miner_hotkey: "agg_miner".to_string(),
            competition_scores,
            weighted_score: 0.85,
            final_weight: 50000,
        };

        let json = serde_json::to_string(&agg).unwrap();
        let deserialized: AggregatedMinerScore = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.miner_uid, 10);
        assert_eq!(deserialized.weighted_score, 0.85);
        assert_eq!(deserialized.final_weight, 50000);
    }

    #[test]
    fn test_aggregated_miner_score_clone() {
        let agg = AggregatedMinerScore {
            miner_uid: 5,
            miner_hotkey: "miner".to_string(),
            competition_scores: HashMap::new(),
            weighted_score: 0.5,
            final_weight: 32768,
        };

        let cloned = agg.clone();
        assert_eq!(agg.miner_uid, cloned.miner_uid);
        assert_eq!(agg.final_weight, cloned.final_weight);
    }

    #[test]
    fn test_aggregated_miner_score_debug() {
        let agg = AggregatedMinerScore {
            miner_uid: 1,
            miner_hotkey: "debug".to_string(),
            competition_scores: HashMap::new(),
            weighted_score: 0.0,
            final_weight: 0,
        };

        let debug = format!("{:?}", agg);
        assert!(debug.contains("AggregatedMinerScore"));
    }

    // =========================================================================
    // WeightStrategy tests
    // =========================================================================

    #[test]
    fn test_weight_strategy_default() {
        let strategy = WeightStrategy::default();
        assert_eq!(strategy, WeightStrategy::Linear);
    }

    #[test]
    fn test_weight_strategy_serialization() {
        let strategies = vec![
            WeightStrategy::Linear,
            WeightStrategy::Softmax { temperature: 100 },
            WeightStrategy::WinnerTakesAll { top_n: 5 },
            WeightStrategy::Ranked,
            WeightStrategy::Quadratic,
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let deserialized: WeightStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_weight_strategy_equality() {
        assert_eq!(WeightStrategy::Linear, WeightStrategy::Linear);
        assert_eq!(WeightStrategy::Ranked, WeightStrategy::Ranked);
        assert_eq!(WeightStrategy::Quadratic, WeightStrategy::Quadratic);
        assert_eq!(
            WeightStrategy::Softmax { temperature: 100 },
            WeightStrategy::Softmax { temperature: 100 }
        );
        assert_ne!(
            WeightStrategy::Softmax { temperature: 100 },
            WeightStrategy::Softmax { temperature: 200 }
        );
        assert_ne!(WeightStrategy::Linear, WeightStrategy::Quadratic);
    }

    #[test]
    fn test_weight_strategy_clone() {
        let strategy = WeightStrategy::WinnerTakesAll { top_n: 3 };
        let cloned = strategy;
        assert_eq!(strategy, cloned);
    }

    #[test]
    fn test_weight_strategy_debug() {
        let strategy = WeightStrategy::Softmax { temperature: 150 };
        let debug = format!("{:?}", strategy);
        assert!(debug.contains("Softmax"));
        assert!(debug.contains("150"));
    }

    // =========================================================================
    // CompetitionWeights tests
    // =========================================================================

    #[test]
    fn test_competition_weights_serialization() {
        let mut raw = HashMap::new();
        raw.insert(1u16, 40000u16);
        raw.insert(2u16, 25535u16);

        let mut weighted = HashMap::new();
        weighted.insert(1u16, 40.0);
        weighted.insert(2u16, 25.535);

        let weights = CompetitionWeights {
            competition_id: "test".to_string(),
            emission_percent: 100.0,
            raw_weights: raw,
            weighted_weights: weighted,
            strategy_used: WeightStrategy::Linear,
            calculated_at: Utc::now(),
        };

        let json = serde_json::to_string(&weights).unwrap();
        let deserialized: CompetitionWeights = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.competition_id, "test");
        assert_eq!(deserialized.emission_percent, 100.0);
    }

    #[test]
    fn test_competition_weights_clone() {
        let weights = CompetitionWeights {
            competition_id: "clone".to_string(),
            emission_percent: 50.0,
            raw_weights: HashMap::new(),
            weighted_weights: HashMap::new(),
            strategy_used: WeightStrategy::Ranked,
            calculated_at: Utc::now(),
        };

        let cloned = weights.clone();
        assert_eq!(weights.competition_id, cloned.competition_id);
    }

    #[test]
    fn test_competition_weights_debug() {
        let weights = CompetitionWeights {
            competition_id: "debug".to_string(),
            emission_percent: 50.0,
            raw_weights: HashMap::new(),
            weighted_weights: HashMap::new(),
            strategy_used: WeightStrategy::Linear,
            calculated_at: Utc::now(),
        };

        let debug = format!("{:?}", weights);
        assert!(debug.contains("CompetitionWeights"));
    }

    // =========================================================================
    // FinalWeights tests
    // =========================================================================

    #[test]
    fn test_final_weights_serialization() {
        let mut weights_map = HashMap::new();
        weights_map.insert(1u16, 40000u16);
        weights_map.insert(2u16, 25535u16);

        let final_weights = FinalWeights {
            weights: weights_map,
            competition_breakdown: vec![],
            miners_with_weights: 2,
            epoch: 100,
            calculated_at: Utc::now(),
        };

        let json = serde_json::to_string(&final_weights).unwrap();
        let deserialized: FinalWeights = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.miners_with_weights, 2);
        assert_eq!(deserialized.epoch, 100);
    }

    #[test]
    fn test_final_weights_clone() {
        let final_weights = FinalWeights {
            weights: HashMap::new(),
            competition_breakdown: vec![],
            miners_with_weights: 0,
            epoch: 50,
            calculated_at: Utc::now(),
        };

        let cloned = final_weights.clone();
        assert_eq!(final_weights.epoch, cloned.epoch);
    }

    #[test]
    fn test_final_weights_debug() {
        let final_weights = FinalWeights {
            weights: HashMap::new(),
            competition_breakdown: vec![],
            miners_with_weights: 0,
            epoch: 1,
            calculated_at: Utc::now(),
        };

        let debug = format!("{:?}", final_weights);
        assert!(debug.contains("FinalWeights"));
    }

    // =========================================================================
    // WeightCalculator tests
    // =========================================================================

    #[test]
    fn test_weight_calculator_with_strategy() {
        let config = EmissionConfig::default();
        let calculator = WeightCalculator::new(config).with_strategy(WeightStrategy::Quadratic);
        assert_eq!(calculator.default_strategy, WeightStrategy::Quadratic);
    }

    #[test]
    fn test_weight_calculator_with_max_cap() {
        let config = EmissionConfig::default();
        let calculator = WeightCalculator::new(config).with_max_cap(25.0);
        assert_eq!(calculator.max_weight_cap_percent, 25.0);
    }

    #[test]
    fn test_weight_calculator_competition_not_found() {
        let config = EmissionConfig::default();
        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("nonexistent");

        let result = calculator.calculate_competition_weights("nonexistent", &scores, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_weight_calculator_inactive_competition() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "inactive".to_string(),
                emission_percent: 50.0,
                active: false,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("inactive");

        let result = calculator.calculate_competition_weights("inactive", &scores, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_weight_calculator_empty_scores() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "empty".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores: Vec<MinerScore> = vec![];

        let result = calculator
            .calculate_competition_weights("empty", &scores, None)
            .unwrap();
        assert!(result.raw_weights.is_empty());
    }

    #[test]
    fn test_weight_calculator_softmax() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("comp1");

        let weights = calculator
            .calculate_competition_weights(
                "comp1",
                &scores,
                Some(WeightStrategy::Softmax { temperature: 100 }),
            )
            .unwrap();

        assert!(!weights.raw_weights.is_empty());
    }

    #[test]
    fn test_weight_calculator_ranked() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("comp1");

        let weights = calculator
            .calculate_competition_weights("comp1", &scores, Some(WeightStrategy::Ranked))
            .unwrap();

        assert!(!weights.raw_weights.is_empty());
    }

    #[test]
    fn test_weight_calculator_quadratic() {
        let mut config = EmissionConfig::default();
        config
            .set_allocation(EmissionAllocation {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
                priority: 0,
                min_score_threshold: 0.0,
                updated_at: Utc::now(),
            })
            .unwrap();

        let calculator = WeightCalculator::new(config);
        let scores = create_test_scores("comp1");

        let weights = calculator
            .calculate_competition_weights("comp1", &scores, Some(WeightStrategy::Quadratic))
            .unwrap();

        assert!(!weights.raw_weights.is_empty());
    }

    #[test]
    fn test_final_weights_invalid_config() {
        let config = EmissionConfig::default(); // 0% allocated - invalid
        let calculator = WeightCalculator::new(config);

        let all_scores = HashMap::new();
        let result = calculator.calculate_final_weights(&all_scores, 100);
        assert!(result.is_err());
    }

    // =========================================================================
    // EmissionManager tests
    // =========================================================================

    #[test]
    fn test_emission_manager_default() {
        let manager = EmissionManager::default();
        let summary = manager.get_emission_summary();
        assert!(!summary.is_valid);
        assert_eq!(summary.total_allocated, 0.0);
    }

    #[test]
    fn test_emission_manager_add_competition_invalid_percent() {
        let mut manager = EmissionManager::new();

        let result = manager.add_competition("comp".to_string(), 0.0, 0.0);
        assert!(result.is_err());

        let result = manager.add_competition("comp".to_string(), 101.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_emission_manager_update_nonexistent() {
        let mut manager = EmissionManager::new();
        let result = manager.update_emission("nonexistent", 50.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_emission_manager_update_would_exceed() {
        let mut manager = EmissionManager::new();
        manager
            .add_competition("comp1".to_string(), 60.0, 0.0)
            .unwrap();
        manager
            .add_competition("comp2".to_string(), 40.0, 0.0)
            .unwrap();

        // Trying to update comp1 to 70% would make total 110%
        let result = manager.update_emission("comp1", 70.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_emission_manager_remove_competition() {
        let mut manager = EmissionManager::new();
        manager
            .add_competition("comp1".to_string(), 100.0, 0.0)
            .unwrap();

        manager.remove_competition("comp1", false).unwrap();
        let summary = manager.get_emission_summary();
        assert_eq!(summary.total_allocated, 0.0);
    }

    #[test]
    fn test_emission_manager_remove_and_redistribute() {
        let mut manager = EmissionManager::new();
        manager
            .add_competition("comp1".to_string(), 50.0, 0.0)
            .unwrap();
        manager
            .add_competition("comp2".to_string(), 50.0, 0.0)
            .unwrap();

        manager.remove_competition("comp1", true).unwrap();
        let summary = manager.get_emission_summary();
        assert!(summary.is_valid);
    }

    #[test]
    fn test_emission_manager_set_active() {
        let mut manager = EmissionManager::new();
        manager
            .add_competition("comp1".to_string(), 100.0, 0.0)
            .unwrap();

        manager.set_competition_active("comp1", false).unwrap();
        let summary = manager.get_emission_summary();
        assert_eq!(summary.total_allocated, 0.0);
    }

    #[test]
    fn test_emission_manager_set_active_nonexistent() {
        let mut manager = EmissionManager::new();
        let result = manager.set_competition_active("nonexistent", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_emission_manager_get_weights_for_epoch() {
        let mut manager = EmissionManager::new();
        manager
            .add_competition("comp1".to_string(), 100.0, 0.0)
            .unwrap();

        let mut all_scores = HashMap::new();
        all_scores.insert("comp1".to_string(), create_test_scores("comp1"));

        manager.calculate_weights(&all_scores, 100).unwrap();

        assert!(manager.get_weights_for_epoch(100).is_some());
        assert!(manager.get_weights_for_epoch(101).is_none());
    }

    // =========================================================================
    // EmissionSummary tests
    // =========================================================================

    #[test]
    fn test_emission_summary_serialization() {
        let summary = EmissionSummary {
            total_allocated: 100.0,
            unallocated: 0.0,
            is_valid: true,
            allocations: vec![AllocationSummary {
                competition_id: "comp1".to_string(),
                emission_percent: 100.0,
                active: true,
            }],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: EmissionSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_allocated, 100.0);
        assert!(deserialized.is_valid);
    }

    // =========================================================================
    // AllocationSummary tests
    // =========================================================================

    #[test]
    fn test_allocation_summary_serialization() {
        let summary = AllocationSummary {
            competition_id: "test".to_string(),
            emission_percent: 50.0,
            active: true,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: AllocationSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.competition_id, "test");
        assert_eq!(deserialized.emission_percent, 50.0);
    }
}
