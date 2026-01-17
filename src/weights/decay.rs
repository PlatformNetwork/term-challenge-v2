//! Reward and time decay mechanisms.
//!
//! This module provides two types of decay:
//! - Reward decay: When no one beats the top performer for N epochs
//! - Time decay: Based on submission age after a grace period

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Reward Decay System
// ============================================================================
//
// This section implements a reward decay mechanism to encourage continuous competition.
// When no new agent beats the top performer for a certain number of epochs,
// rewards start decaying by allocating more weight to UID 0 (burn address).
//
// ## How it works:
// 1. Track the top agent and their score
// 2. If no one beats the top for `grace_epochs`, start decay
// 3. Each epoch without improvement, `decay_rate` of remaining emission goes to burn (UID 0)
// 4. Decay stops when someone beats the top score
// 5. Optional: Reset decay on any improvement (not just beating top)

/// UID 0 is the burn address in Bittensor - weights sent here are burned
pub const BURN_UID: u16 = 0;

/// Decay curve types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum DecayCurve {
    /// Linear decay: burn_percent = decay_rate * epochs_stale
    #[default]
    Linear,
    /// Exponential decay: burn_percent = 1 - (1 - decay_rate)^epochs_stale
    Exponential,
    /// Step decay: burn_percent increases in steps
    Step { step_size: f64, step_epochs: u64 },
    /// Logarithmic decay: slower decay over time
    Logarithmic,
    /// Custom decay with specific percentages per epoch
    Custom { percentages: Vec<f64> },
}

/// Configuration for the reward decay system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayConfig {
    /// Whether decay is enabled
    pub enabled: bool,
    /// Number of epochs without improvement before decay starts
    pub grace_epochs: u64,
    /// Decay rate per epoch (0.0 - 1.0)
    /// For linear: burn_percent = rate * stale_epochs
    /// For exponential: burn_percent = 1 - (1 - rate)^stale_epochs
    pub decay_rate: f64,
    /// Maximum burn percentage (cap)
    pub max_burn_percent: f64,
    /// Decay curve type
    pub curve: DecayCurve,
    /// Reset decay on any improvement (not just beating top)
    pub reset_on_any_improvement: bool,
    /// Minimum score improvement to count as "beating" (e.g., 0.01 = 1%)
    pub min_improvement_threshold: f64,
    /// Whether to notify when decay starts/changes
    pub emit_events: bool,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            grace_epochs: 10,       // 10 epochs (~12 hours with 360 block tempo)
            decay_rate: 0.05,       // 5% decay per epoch
            max_burn_percent: 80.0, // Max 80% goes to burn
            curve: DecayCurve::Linear,
            reset_on_any_improvement: false,
            min_improvement_threshold: 0.02, // 2% improvement needed to beat current winner
            emit_events: true,
        }
    }
}

/// State of the top agent for decay tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopAgentState {
    /// Hash of the top agent
    pub agent_hash: String,
    /// Miner UID of top agent
    pub miner_uid: u16,
    /// Miner hotkey
    pub miner_hotkey: String,
    /// Top score achieved
    pub score: f64,
    /// Epoch when this score was achieved
    pub achieved_epoch: u64,
    /// Epoch when last improvement was made
    pub last_improvement_epoch: u64,
    /// Number of epochs without improvement
    pub epochs_without_improvement: u64,
    /// Whether decay is currently active
    pub decay_active: bool,
    /// Current burn percentage
    pub current_burn_percent: f64,
}

/// Decay event for logging/notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecayEvent {
    /// Decay has started
    DecayStarted {
        top_agent: String,
        top_score: f64,
        epochs_stale: u64,
        burn_percent: f64,
    },
    /// Decay percentage increased
    DecayIncreased {
        previous_burn: f64,
        new_burn: f64,
        epochs_stale: u64,
    },
    /// New top agent - decay reset
    DecayReset {
        new_agent: String,
        new_score: f64,
        previous_top: String,
        previous_score: f64,
    },
    /// Improvement detected but not new top
    ImprovementDetected {
        agent: String,
        score: f64,
        improvement_over: f64,
    },
    /// Max decay reached
    MaxDecayReached { burn_percent: f64 },
}

/// Competition-specific decay state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionDecayState {
    pub competition_id: String,
    pub config: DecayConfig,
    pub top_agent: Option<TopAgentState>,
    pub event_history: Vec<(DateTime<Utc>, DecayEvent)>,
    pub last_updated: DateTime<Utc>,
}

impl CompetitionDecayState {
    pub fn new(competition_id: String, config: DecayConfig) -> Self {
        Self {
            competition_id,
            config,
            top_agent: None,
            event_history: Vec::new(),
            last_updated: Utc::now(),
        }
    }
}

/// Main decay manager
pub struct RewardDecayManager {
    /// Decay states per competition
    states: HashMap<String, CompetitionDecayState>,
    /// Global default config
    default_config: DecayConfig,
}

impl RewardDecayManager {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            default_config: DecayConfig::default(),
        }
    }

    pub fn with_default_config(config: DecayConfig) -> Self {
        Self {
            states: HashMap::new(),
            default_config: config,
        }
    }

    /// Register a competition for decay tracking
    pub fn register_competition(&mut self, competition_id: String, config: Option<DecayConfig>) {
        let config = config.unwrap_or_else(|| self.default_config.clone());
        let state = CompetitionDecayState::new(competition_id.clone(), config);
        self.states.insert(competition_id, state);
    }

    /// Update config for a competition
    pub fn update_config(
        &mut self,
        competition_id: &str,
        config: DecayConfig,
    ) -> Result<(), String> {
        let state = self
            .states
            .get_mut(competition_id)
            .ok_or_else(|| format!("Competition {} not registered", competition_id))?;
        state.config = config;
        state.last_updated = Utc::now();
        Ok(())
    }

    /// Enable/disable decay for a competition
    pub fn set_enabled(&mut self, competition_id: &str, enabled: bool) -> Result<(), String> {
        let state = self
            .states
            .get_mut(competition_id)
            .ok_or_else(|| format!("Competition {} not registered", competition_id))?;
        state.config.enabled = enabled;
        state.last_updated = Utc::now();
        Ok(())
    }

    /// Process scores for an epoch and update decay state
    pub fn process_epoch(
        &mut self,
        competition_id: &str,
        current_epoch: u64,
        scores: &[(u16, String, String, f64)], // (uid, hotkey, agent_hash, score)
    ) -> Result<DecayResult, String> {
        let state = self
            .states
            .get_mut(competition_id)
            .ok_or_else(|| format!("Competition {} not registered", competition_id))?;

        if !state.config.enabled {
            return Ok(DecayResult {
                burn_percent: 0.0,
                burn_weight: 0,
                events: vec![],
                decay_active: false,
            });
        }

        // Find current epoch's best score
        let current_best = scores
            .iter()
            .max_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

        let mut events = Vec::new();

        match (&mut state.top_agent, current_best) {
            // No top agent yet, set first one
            (None, Some((uid, hotkey, agent_hash, score))) => {
                state.top_agent = Some(TopAgentState {
                    agent_hash: agent_hash.clone(),
                    miner_uid: *uid,
                    miner_hotkey: hotkey.clone(),
                    score: *score,
                    achieved_epoch: current_epoch,
                    last_improvement_epoch: current_epoch,
                    epochs_without_improvement: 0,
                    decay_active: false,
                    current_burn_percent: 0.0,
                });
            }

            // Have top agent, check for improvement
            (Some(top), Some((uid, hotkey, agent_hash, score))) => {
                let improvement = *score - top.score;

                // Check if this beats the top
                if improvement >= state.config.min_improvement_threshold {
                    // New top agent!
                    if state.config.emit_events {
                        events.push(DecayEvent::DecayReset {
                            new_agent: agent_hash.clone(),
                            new_score: *score,
                            previous_top: top.agent_hash.clone(),
                            previous_score: top.score,
                        });
                    }

                    *top = TopAgentState {
                        agent_hash: agent_hash.clone(),
                        miner_uid: *uid,
                        miner_hotkey: hotkey.clone(),
                        score: *score,
                        achieved_epoch: current_epoch,
                        last_improvement_epoch: current_epoch,
                        epochs_without_improvement: 0,
                        decay_active: false,
                        current_burn_percent: 0.0,
                    };
                } else if state.config.reset_on_any_improvement && improvement > 0.0 {
                    // Any improvement resets decay counter
                    if state.config.emit_events {
                        events.push(DecayEvent::ImprovementDetected {
                            agent: agent_hash.clone(),
                            score: *score,
                            improvement_over: improvement,
                        });
                    }
                    top.last_improvement_epoch = current_epoch;
                    top.epochs_without_improvement = 0;
                    top.decay_active = false;
                    top.current_burn_percent = 0.0;
                } else {
                    // No improvement, increment stale counter
                    top.epochs_without_improvement =
                        current_epoch.saturating_sub(top.last_improvement_epoch);

                    // Check if decay should start
                    // Decay starts when epochs_without_improvement >= grace_epochs
                    if top.epochs_without_improvement >= state.config.grace_epochs {
                        // Calculate stale epochs: how many epochs past the grace period (1-indexed)
                        let stale_epochs =
                            top.epochs_without_improvement - state.config.grace_epochs + 1;
                        let new_burn_percent = calculate_burn_percent(&state.config, stale_epochs);

                        if !top.decay_active && state.config.emit_events {
                            events.push(DecayEvent::DecayStarted {
                                top_agent: top.agent_hash.clone(),
                                top_score: top.score,
                                epochs_stale: stale_epochs,
                                burn_percent: new_burn_percent,
                            });
                        } else if new_burn_percent > top.current_burn_percent
                            && state.config.emit_events
                        {
                            events.push(DecayEvent::DecayIncreased {
                                previous_burn: top.current_burn_percent,
                                new_burn: new_burn_percent,
                                epochs_stale: stale_epochs,
                            });
                        }

                        if new_burn_percent >= state.config.max_burn_percent
                            && state.config.emit_events
                        {
                            events.push(DecayEvent::MaxDecayReached {
                                burn_percent: state.config.max_burn_percent,
                            });
                        }

                        top.decay_active = true;
                        top.current_burn_percent = new_burn_percent;
                    }
                }
            }

            // No scores this epoch
            (Some(top), None) => {
                top.epochs_without_improvement =
                    current_epoch.saturating_sub(top.last_improvement_epoch);

                if top.epochs_without_improvement >= state.config.grace_epochs {
                    let stale_epochs =
                        top.epochs_without_improvement - state.config.grace_epochs + 1;
                    top.current_burn_percent = calculate_burn_percent(&state.config, stale_epochs);
                    top.decay_active = true;
                }
            }

            (None, None) => {}
        }

        // Record events
        for event in &events {
            state.event_history.push((Utc::now(), event.clone()));
        }
        state.last_updated = Utc::now();

        // Calculate result
        let burn_percent = state
            .top_agent
            .as_ref()
            .map(|t| t.current_burn_percent)
            .unwrap_or(0.0);

        let burn_weight = ((burn_percent / 100.0) * 65535.0).round() as u16;
        let decay_active = state
            .top_agent
            .as_ref()
            .map(|t| t.decay_active)
            .unwrap_or(false);

        Ok(DecayResult {
            burn_percent,
            burn_weight,
            events,
            decay_active,
        })
    }

    /// Apply decay to weights (adds burn weight to UID 0)
    pub fn apply_decay_to_weights(
        &self,
        competition_id: &str,
        weights: &mut HashMap<u16, u16>,
    ) -> Result<AppliedDecay, String> {
        let state = self
            .states
            .get(competition_id)
            .ok_or_else(|| format!("Competition {} not registered", competition_id))?;

        if !state.config.enabled {
            return Ok(AppliedDecay {
                burn_percent: 0.0,
                burn_weight_added: 0,
                original_total: weights.values().map(|w| *w as u32).sum(),
                adjusted_total: weights.values().map(|w| *w as u32).sum(),
            });
        }

        let burn_percent = state
            .top_agent
            .as_ref()
            .filter(|t| t.decay_active)
            .map(|t| t.current_burn_percent)
            .unwrap_or(0.0);

        if burn_percent <= 0.0 {
            return Ok(AppliedDecay {
                burn_percent: 0.0,
                burn_weight_added: 0,
                original_total: weights.values().map(|w| *w as u32).sum(),
                adjusted_total: weights.values().map(|w| *w as u32).sum(),
            });
        }

        // Calculate how much to burn
        let original_total: u32 = weights.values().map(|w| *w as u32).sum();
        let burn_fraction = burn_percent / 100.0;

        // Scale down existing weights
        let scale_factor = 1.0 - burn_fraction;
        for weight in weights.values_mut() {
            *weight = ((*weight as f64) * scale_factor).round() as u16;
        }

        // Calculate burn weight
        let new_total: u32 = weights.values().map(|w| *w as u32).sum();
        let burn_weight = (original_total - new_total) as u16;

        // Add burn weight to UID 0
        *weights.entry(BURN_UID).or_insert(0) += burn_weight;

        let adjusted_total: u32 = weights.values().map(|w| *w as u32).sum();

        Ok(AppliedDecay {
            burn_percent,
            burn_weight_added: burn_weight,
            original_total,
            adjusted_total,
        })
    }

    /// Get current decay state for a competition
    pub fn get_state(&self, competition_id: &str) -> Option<&CompetitionDecayState> {
        self.states.get(competition_id)
    }

    /// Get decay summary for a competition
    pub fn get_summary(&self, competition_id: &str) -> Option<DecaySummary> {
        let state = self.states.get(competition_id)?;

        Some(DecaySummary {
            competition_id: competition_id.to_string(),
            enabled: state.config.enabled,
            decay_active: state
                .top_agent
                .as_ref()
                .map(|t| t.decay_active)
                .unwrap_or(false),
            current_burn_percent: state
                .top_agent
                .as_ref()
                .map(|t| t.current_burn_percent)
                .unwrap_or(0.0),
            epochs_without_improvement: state
                .top_agent
                .as_ref()
                .map(|t| t.epochs_without_improvement)
                .unwrap_or(0),
            grace_epochs_remaining: state
                .top_agent
                .as_ref()
                .map(|t| {
                    state
                        .config
                        .grace_epochs
                        .saturating_sub(t.epochs_without_improvement)
                })
                .unwrap_or(state.config.grace_epochs),
            top_agent: state.top_agent.as_ref().map(|t| TopAgentSummary {
                agent_hash: t.agent_hash.clone(),
                miner_uid: t.miner_uid,
                score: t.score,
                achieved_epoch: t.achieved_epoch,
            }),
            config: state.config.clone(),
        })
    }

    /// Manually reset decay for a competition (admin action)
    pub fn reset_decay(&mut self, competition_id: &str) -> Result<(), String> {
        let state = self
            .states
            .get_mut(competition_id)
            .ok_or_else(|| format!("Competition {} not registered", competition_id))?;

        if let Some(top) = &mut state.top_agent {
            top.epochs_without_improvement = 0;
            top.decay_active = false;
            top.current_burn_percent = 0.0;
            top.last_improvement_epoch = Utc::now().timestamp() as u64; // Use current as "improvement"
        }

        state.last_updated = Utc::now();
        Ok(())
    }
}

impl Default for RewardDecayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate burn percentage based on config and stale epochs
fn calculate_burn_percent(config: &DecayConfig, stale_epochs: u64) -> f64 {
    let raw_percent = match config.curve {
        DecayCurve::Linear => config.decay_rate * stale_epochs as f64 * 100.0,
        DecayCurve::Exponential => {
            (1.0 - (1.0 - config.decay_rate).powi(stale_epochs as i32)) * 100.0
        }
        DecayCurve::Step {
            step_size,
            step_epochs,
        } => {
            let steps = stale_epochs / step_epochs;
            (steps as f64 * step_size).min(100.0)
        }
        DecayCurve::Logarithmic => {
            // ln(1 + stale_epochs) * decay_rate * 20
            (1.0 + stale_epochs as f64).ln() * config.decay_rate * 20.0
        }
        DecayCurve::Custom { ref percentages } => {
            let idx = (stale_epochs as usize).min(percentages.len().saturating_sub(1));
            percentages
                .get(idx)
                .copied()
                .unwrap_or(config.max_burn_percent)
        }
    };

    raw_percent.min(config.max_burn_percent).max(0.0)
}

/// Result of processing an epoch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayResult {
    pub burn_percent: f64,
    pub burn_weight: u16,
    pub events: Vec<DecayEvent>,
    pub decay_active: bool,
}

/// Result of applying decay to weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedDecay {
    pub burn_percent: f64,
    pub burn_weight_added: u16,
    pub original_total: u32,
    pub adjusted_total: u32,
}

/// Summary of decay state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecaySummary {
    pub competition_id: String,
    pub enabled: bool,
    pub decay_active: bool,
    pub current_burn_percent: f64,
    pub epochs_without_improvement: u64,
    pub grace_epochs_remaining: u64,
    pub top_agent: Option<TopAgentSummary>,
    pub config: DecayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopAgentSummary {
    pub agent_hash: String,
    pub miner_uid: u16,
    pub score: f64,
    pub achieved_epoch: u64,
}

// ============================================================================
// Time-Based Decay System
// ============================================================================
//
// Implements a decay mechanism based on time since submission:
// - Grace period: 48 hours after submission = no decay
// - After grace period: Rewards decay by 50% each day (24 hours)
//
// Formula: multiplier = 0.5 ^ (days_past_grace)

/// Configuration for time-based decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDecayConfig {
    /// Whether time decay is enabled
    pub enabled: bool,
    /// Grace period in hours before decay starts (default: 48 hours)
    pub grace_period_hours: u64,
    /// Half-life in hours - time for weight to decay by 50% (default: 24 hours = 1 day)
    pub half_life_hours: u64,
    /// Minimum multiplier (weight never goes below this, default: 0.01 = 1%)
    pub min_multiplier: f64,
}

impl Default for TimeDecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            grace_period_hours: 48, // 48 hours = 2 days grace period
            half_life_hours: 24,    // 24 hours = 50% decay per day
            min_multiplier: 0.01,
        }
    }
}

impl TimeDecayConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("TIME_DECAY_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            grace_period_hours: std::env::var("TIME_DECAY_GRACE_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(48),
            half_life_hours: std::env::var("TIME_DECAY_HALF_LIFE_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            min_multiplier: std::env::var("TIME_DECAY_MIN_MULTIPLIER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.01),
        }
    }
}

/// Result of time decay calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayInfo {
    /// The decay multiplier to apply to weight (0.0 to 1.0)
    pub multiplier: f64,
    /// Age of submission in hours
    pub age_hours: f64,
    /// Hours remaining in grace period (0 if grace period expired)
    pub grace_period_remaining_hours: f64,
    /// Whether decay is currently active
    pub decay_active: bool,
    /// Days since grace period ended (for display)
    pub days_decaying: f64,
}

/// Calculate decay multiplier based on time since submission
///
/// Formula:
/// - If hours_elapsed <= grace_period_hours: multiplier = 1.0
/// - Otherwise: multiplier = 0.5 ^ (hours_past_grace / half_life_hours)
///
/// The multiplier is clamped to min_multiplier to prevent complete decay.
pub fn calculate_decay_multiplier(submission_time: DateTime<Utc>, config: &TimeDecayConfig) -> f64 {
    if !config.enabled {
        return 1.0;
    }

    let now = Utc::now();
    let hours_elapsed = (now - submission_time).num_minutes() as f64 / 60.0;

    if hours_elapsed <= config.grace_period_hours as f64 {
        return 1.0;
    }

    let hours_past_grace = hours_elapsed - config.grace_period_hours as f64;
    let half_lives = hours_past_grace / config.half_life_hours as f64;

    // multiplier = 0.5 ^ half_lives
    let multiplier = 0.5_f64.powf(half_lives);

    // Clamp to minimum
    multiplier.max(config.min_multiplier)
}

/// Calculate full decay info for a submission
pub fn calculate_decay_info(submission_time: DateTime<Utc>, config: &TimeDecayConfig) -> DecayInfo {
    let now = Utc::now();
    let hours_elapsed = (now - submission_time).num_minutes() as f64 / 60.0;

    if !config.enabled {
        return DecayInfo {
            multiplier: 1.0,
            age_hours: hours_elapsed,
            grace_period_remaining_hours: 0.0,
            decay_active: false,
            days_decaying: 0.0,
        };
    }

    let grace_remaining = (config.grace_period_hours as f64 - hours_elapsed).max(0.0);
    let decay_active = hours_elapsed > config.grace_period_hours as f64;

    let (multiplier, days_decaying) = if decay_active {
        let hours_past_grace = hours_elapsed - config.grace_period_hours as f64;
        let half_lives = hours_past_grace / config.half_life_hours as f64;
        let mult = 0.5_f64.powf(half_lives).max(config.min_multiplier);
        (mult, hours_past_grace / 24.0)
    } else {
        (1.0, 0.0)
    };

    DecayInfo {
        multiplier,
        age_hours: hours_elapsed,
        grace_period_remaining_hours: grace_remaining,
        decay_active,
        days_decaying,
    }
}

/// Decay status response for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayStatusResponse {
    pub winner: Option<WinnerDecayStatus>,
    pub config: TimeDecayConfigResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinnerDecayStatus {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub submitted_at: String,
    pub age_hours: f64,
    pub grace_period_remaining_hours: f64,
    pub decay_active: bool,
    pub decay_multiplier: f64,
    pub effective_weight: f64,
    pub days_decaying: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDecayConfigResponse {
    pub enabled: bool,
    pub grace_period_hours: u64,
    pub half_life_hours: u64,
    pub min_multiplier: f64,
}

impl From<&TimeDecayConfig> for TimeDecayConfigResponse {
    fn from(config: &TimeDecayConfig) -> Self {
        Self {
            enabled: config.enabled,
            grace_period_hours: config.grace_period_hours,
            half_life_hours: config.half_life_hours,
            min_multiplier: config.min_multiplier,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Reward Decay Tests
    // ------------------------------------------------------------------------

    fn create_test_scores(epoch: u64) -> Vec<(u16, String, String, f64)> {
        vec![
            (1, "miner1".into(), format!("agent1_e{}", epoch), 0.80),
            (2, "miner2".into(), format!("agent2_e{}", epoch), 0.75),
            (3, "miner3".into(), format!("agent3_e{}", epoch), 0.60),
        ]
    }

    #[test]
    fn test_decay_config_default() {
        let config = DecayConfig::default();
        assert!(config.enabled);
        assert_eq!(config.grace_epochs, 10);
        assert_eq!(config.decay_rate, 0.05);
        assert_eq!(config.max_burn_percent, 80.0);
    }

    #[test]
    fn test_no_decay_during_grace_period() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 5,
            decay_rate: 0.1,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // First epoch - set top agent
        let scores = create_test_scores(1);
        let result = manager.process_epoch("test", 1, &scores).unwrap();
        assert!(!result.decay_active);
        assert_eq!(result.burn_percent, 0.0);

        // Epochs 2-5 - same scores, still in grace period
        for epoch in 2..=5 {
            let result = manager.process_epoch("test", epoch, &scores).unwrap();
            assert!(!result.decay_active);
            assert_eq!(result.burn_percent, 0.0);
        }
    }

    #[test]
    fn test_decay_starts_after_grace_period() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 3, // After 3 epochs without improvement, decay starts
            decay_rate: 0.1,
            max_burn_percent: 50.0,
            curve: DecayCurve::Linear,
            emit_events: true,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Set initial top agent at epoch 1 (last_improvement = 1)
        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();

        // Epoch 2: epochs_without_improvement = 1 (< 3)
        // Epoch 3: epochs_without_improvement = 2 (< 3)
        for epoch in 2..=3 {
            let result = manager.process_epoch("test", epoch, &scores).unwrap();
            assert!(
                !result.decay_active,
                "Epoch {} should not have decay",
                epoch
            );
        }

        // Epoch 4: epochs_without_improvement = 3 (>= 3), decay should start
        let result = manager.process_epoch("test", 4, &scores).unwrap();
        assert!(result.decay_active, "Epoch 4 should have decay active");
        assert!(result.burn_percent > 0.0);

        // Check for DecayStarted event
        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, DecayEvent::DecayStarted { .. })));
    }

    #[test]
    fn test_decay_resets_on_new_top() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 2,
            decay_rate: 0.2,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Initial scores
        let scores = vec![(1, "miner1".into(), "agent1".into(), 0.80)];
        manager.process_epoch("test", 1, &scores).unwrap();

        // No improvement for 5 epochs - decay should be active
        for epoch in 2..=5 {
            manager.process_epoch("test", epoch, &scores).unwrap();
        }

        let state = manager.get_state("test").unwrap();
        assert!(state.top_agent.as_ref().unwrap().decay_active);

        // New top agent with better score
        let better_scores = vec![(2, "miner2".into(), "agent2_better".into(), 0.90)];
        let result = manager.process_epoch("test", 6, &better_scores).unwrap();

        // Decay should be reset
        assert!(!result.decay_active);
        assert_eq!(result.burn_percent, 0.0);

        // Check for DecayReset event
        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, DecayEvent::DecayReset { .. })));
    }

    #[test]
    fn test_linear_decay_curve() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 2, // After 2 epochs, decay starts
            decay_rate: 0.1, // 10% per stale epoch
            max_burn_percent: 80.0,
            curve: DecayCurve::Linear,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        // Epoch 1: last_improvement = 1
        manager.process_epoch("test", 1, &scores).unwrap();

        // Epoch 2: epochs_without_improvement = 1 (< 2, no decay)
        manager.process_epoch("test", 2, &scores).unwrap();

        // Epoch 3: epochs_without_improvement = 2 >= 2, stale_epochs = 1 -> 10%
        let result = manager.process_epoch("test", 3, &scores).unwrap();
        assert!(
            (result.burn_percent - 10.0).abs() < 0.01,
            "Expected 10%, got {}",
            result.burn_percent
        );

        // Epoch 4: epochs_without_improvement = 3 >= 2, stale_epochs = 2 -> 20%
        let result = manager.process_epoch("test", 4, &scores).unwrap();
        assert!(
            (result.burn_percent - 20.0).abs() < 0.01,
            "Expected 20%, got {}",
            result.burn_percent
        );

        // Epoch 5: epochs_without_improvement = 4 >= 2, stale_epochs = 3 -> 30%
        let result = manager.process_epoch("test", 5, &scores).unwrap();
        assert!(
            (result.burn_percent - 30.0).abs() < 0.01,
            "Expected 30%, got {}",
            result.burn_percent
        );
    }

    #[test]
    fn test_max_burn_cap() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.5,        // 50% per epoch - very aggressive
            max_burn_percent: 30.0, // But capped at 30%
            curve: DecayCurve::Linear,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();

        // Many epochs without improvement
        for epoch in 2..=10 {
            let result = manager.process_epoch("test", epoch, &scores).unwrap();
            // Should never exceed 30%
            assert!(result.burn_percent <= 30.0);
        }
    }

    #[test]
    fn test_apply_decay_to_weights() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.2,
            max_burn_percent: 50.0,
            curve: DecayCurve::Linear,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Set top agent and trigger decay
        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();
        manager.process_epoch("test", 3, &scores).unwrap(); // Decay starts

        // Original weights
        let mut weights: HashMap<u16, u16> = HashMap::new();
        weights.insert(1, 30000);
        weights.insert(2, 20000);
        weights.insert(3, 15535);

        let original_total: u32 = weights.values().map(|w| *w as u32).sum();

        // Apply decay
        let result = manager
            .apply_decay_to_weights("test", &mut weights)
            .unwrap();

        // UID 0 (burn) should have weight now
        assert!(weights.contains_key(&BURN_UID));
        assert!(result.burn_weight_added > 0);

        // Total should be preserved
        let new_total: u32 = weights.values().map(|w| *w as u32).sum();
        assert!((new_total as i32 - original_total as i32).abs() <= 3); // Small rounding error ok
    }

    #[test]
    fn test_exponential_decay() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.3,
            max_burn_percent: 90.0,
            curve: DecayCurve::Exponential,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();

        // Exponential decay should increase faster initially then slow down
        let r1 = manager.process_epoch("test", 3, &scores).unwrap();
        let r2 = manager.process_epoch("test", 4, &scores).unwrap();
        let r3 = manager.process_epoch("test", 5, &scores).unwrap();

        // Verify it's increasing
        assert!(r2.burn_percent > r1.burn_percent);
        assert!(r3.burn_percent > r2.burn_percent);

        // Verify exponential curve (increase rate slows down)
        let delta1 = r2.burn_percent - r1.burn_percent;
        let delta2 = r3.burn_percent - r2.burn_percent;
        assert!(delta2 < delta1); // Slowing increase
    }

    #[test]
    fn test_step_decay() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1, // After 1 epoch, decay starts
            decay_rate: 0.1, // Not used for step
            max_burn_percent: 50.0,
            curve: DecayCurve::Step {
                step_size: 10.0,
                step_epochs: 2,
            },
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        // Epoch 1: Set top agent (last_improvement = 1)
        manager.process_epoch("test", 1, &scores).unwrap();

        // Epoch 2: epochs_without_improvement = 1 >= 1, stale_epochs = 1, steps = 0 -> 0%
        let r1 = manager.process_epoch("test", 2, &scores).unwrap();
        assert!(
            (r1.burn_percent - 0.0).abs() < 0.01,
            "Epoch 2: stale=1, steps=0, expected 0%, got {}",
            r1.burn_percent
        );

        // Epoch 3: epochs_without_improvement = 2 >= 1, stale_epochs = 2, steps = 1 -> 10%
        let r2 = manager.process_epoch("test", 3, &scores).unwrap();
        assert!(
            (r2.burn_percent - 10.0).abs() < 0.01,
            "Epoch 3: stale=2, steps=1, expected 10%, got {}",
            r2.burn_percent
        );

        // Epoch 4: epochs_without_improvement = 3 >= 1, stale_epochs = 3, steps = 1 -> 10%
        let r3 = manager.process_epoch("test", 4, &scores).unwrap();
        assert!(
            (r3.burn_percent - 10.0).abs() < 0.01,
            "Epoch 4: stale=3, steps=1, expected 10%, got {}",
            r3.burn_percent
        );

        // Epoch 5: epochs_without_improvement = 4 >= 1, stale_epochs = 4, steps = 2 -> 20%
        let r4 = manager.process_epoch("test", 5, &scores).unwrap();
        assert!(
            (r4.burn_percent - 20.0).abs() < 0.01,
            "Epoch 5: stale=4, steps=2, expected 20%, got {}",
            r4.burn_percent
        );
    }

    #[test]
    fn test_decay_disabled() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: false,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);

        // Many epochs
        for epoch in 1..=20 {
            let result = manager.process_epoch("test", epoch, &scores).unwrap();
            assert!(!result.decay_active);
            assert_eq!(result.burn_percent, 0.0);
        }
    }

    #[test]
    fn test_get_summary() {
        let mut manager = RewardDecayManager::new();
        manager.register_competition("test".into(), None);

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();

        let summary = manager.get_summary("test").unwrap();
        assert!(summary.enabled);
        assert!(!summary.decay_active);
        assert!(summary.top_agent.is_some());
        assert_eq!(summary.top_agent.as_ref().unwrap().score, 0.80);
    }

    #[test]
    fn test_logarithmic_decay_curve() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.2, // ln(1 + stale_epochs) * 0.2 * 20
            max_burn_percent: 80.0,
            curve: DecayCurve::Logarithmic,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();

        // Logarithmic decay: ln(1 + stale_epochs) * decay_rate * 20
        let r1 = manager.process_epoch("test", 3, &scores).unwrap();
        // stale_epochs = 2, ln(3) * 0.2 * 20 ≈ 4.39
        assert!(r1.burn_percent > 0.0);
        assert!(r1.burn_percent < 10.0);

        let r2 = manager.process_epoch("test", 4, &scores).unwrap();
        assert!(r2.burn_percent > r1.burn_percent);
    }

    #[test]
    fn test_custom_decay_curve() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.1,
            max_burn_percent: 100.0,
            curve: DecayCurve::Custom {
                percentages: vec![5.0, 10.0, 25.0, 50.0, 75.0],
            },
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();

        // Custom percentages indexed by stale_epochs:
        // At epoch 3: epochs_without_improvement = 2 >= 1, stale_epochs = 2 - 1 + 1 = 2
        // percentages[2] = 25.0
        let r1 = manager.process_epoch("test", 3, &scores).unwrap();
        assert!(
            (r1.burn_percent - 25.0).abs() < 0.01,
            "Expected 25%, got {}",
            r1.burn_percent
        );

        // At epoch 4: stale_epochs = 3, percentages[3] = 50.0
        let r2 = manager.process_epoch("test", 4, &scores).unwrap();
        assert!(
            (r2.burn_percent - 50.0).abs() < 0.01,
            "Expected 50%, got {}",
            r2.burn_percent
        );

        // At epoch 5: stale_epochs = 4, percentages[4] = 75.0
        let r3 = manager.process_epoch("test", 5, &scores).unwrap();
        assert!(
            (r3.burn_percent - 75.0).abs() < 0.01,
            "Expected 75%, got {}",
            r3.burn_percent
        );
    }

    #[test]
    fn test_custom_decay_curve_overflow() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.1,
            max_burn_percent: 50.0,
            curve: DecayCurve::Custom {
                percentages: vec![10.0, 20.0], // Only 2 entries (index 0 and 1)
            },
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();

        // At epoch 3: stale_epochs = 2, but only 2 entries so clamps to index 1
        // percentages[1] = 20.0
        let r = manager.process_epoch("test", 3, &scores).unwrap();
        assert!(
            (r.burn_percent - 20.0).abs() < 0.01,
            "Expected 20%, got {}",
            r.burn_percent
        );

        // Even at later epochs, should stay at last entry
        let r = manager.process_epoch("test", 10, &scores).unwrap();
        assert!(
            (r.burn_percent - 20.0).abs() < 0.01,
            "Expected 20%, got {}",
            r.burn_percent
        );
    }

    #[test]
    fn test_reset_decay() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.2,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Set up decay
        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();
        manager.process_epoch("test", 3, &scores).unwrap();

        // Verify decay is active
        let state = manager.get_state("test").unwrap();
        assert!(state.top_agent.as_ref().unwrap().decay_active);

        // Reset decay
        manager.reset_decay("test").unwrap();

        let state = manager.get_state("test").unwrap();
        let top = state.top_agent.as_ref().unwrap();
        assert!(!top.decay_active);
        assert_eq!(top.epochs_without_improvement, 0);
        assert_eq!(top.current_burn_percent, 0.0);
    }

    #[test]
    fn test_reset_decay_unknown_competition() {
        let mut manager = RewardDecayManager::new();
        let result = manager.reset_decay("unknown");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }

    #[test]
    fn test_improvement_resets_decay() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 2,
            decay_rate: 0.1,
            min_improvement_threshold: 0.05,
            reset_on_any_improvement: true,
            emit_events: true,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Set initial agent with score 0.70
        let scores = vec![(1, "miner1".into(), "agent1".into(), 0.70)];
        manager.process_epoch("test", 1, &scores).unwrap();

        // Trigger decay
        manager.process_epoch("test", 2, &scores).unwrap();
        manager.process_epoch("test", 3, &scores).unwrap();
        manager.process_epoch("test", 4, &scores).unwrap();

        let state = manager.get_state("test").unwrap();
        assert!(state.top_agent.as_ref().unwrap().decay_active);

        // Small improvement (below min_improvement_threshold but > 0)
        let improved_scores = vec![(1, "miner1".into(), "agent1_v2".into(), 0.72)];
        let result = manager.process_epoch("test", 5, &improved_scores).unwrap();

        // Should reset decay due to reset_on_any_improvement
        assert!(!result.decay_active);
        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, DecayEvent::ImprovementDetected { .. })));
    }

    #[test]
    fn test_apply_decay_disabled() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: false,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let mut weights: HashMap<u16, u16> = HashMap::new();
        weights.insert(1, 30000);
        weights.insert(2, 20000);

        let original_total: u32 = weights.values().map(|w| *w as u32).sum();

        let result = manager
            .apply_decay_to_weights("test", &mut weights)
            .unwrap();

        assert_eq!(result.burn_percent, 0.0);
        assert_eq!(result.burn_weight_added, 0);
        assert_eq!(result.original_total, original_total);
    }

    #[test]
    fn test_apply_decay_unknown_competition() {
        let manager = RewardDecayManager::new();
        let mut weights: HashMap<u16, u16> = HashMap::new();
        weights.insert(1, 30000);

        let result = manager.apply_decay_to_weights("unknown", &mut weights);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_decay_no_decay_active() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 10,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();

        let mut weights: HashMap<u16, u16> = HashMap::new();
        weights.insert(1, 30000);

        let result = manager
            .apply_decay_to_weights("test", &mut weights)
            .unwrap();

        assert_eq!(result.burn_percent, 0.0);
        assert_eq!(result.burn_weight_added, 0);
    }

    #[test]
    fn test_process_epoch_unknown_competition() {
        let mut manager = RewardDecayManager::new();
        let result = manager.process_epoch("unknown", 1, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_summary_unknown_competition() {
        let manager = RewardDecayManager::new();
        let summary = manager.get_summary("unknown");
        assert!(summary.is_none());
    }

    #[test]
    fn test_get_state_unknown_competition() {
        let manager = RewardDecayManager::new();
        let state = manager.get_state("unknown");
        assert!(state.is_none());
    }

    #[test]
    fn test_decay_result_serialization() {
        let result = DecayResult {
            burn_percent: 25.5,
            burn_weight: 16384,
            events: vec![DecayEvent::DecayStarted {
                top_agent: "agent1".to_string(),
                top_score: 0.85,
                epochs_stale: 3,
                burn_percent: 25.5,
            }],
            decay_active: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DecayResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.burn_percent, 25.5);
        assert_eq!(deserialized.burn_weight, 16384);
        assert!(deserialized.decay_active);
    }

    #[test]
    fn test_decay_summary_serialization() {
        let summary = DecaySummary {
            competition_id: "test".to_string(),
            enabled: true,
            decay_active: true,
            current_burn_percent: 15.0,
            epochs_without_improvement: 5,
            grace_epochs_remaining: 0,
            top_agent: Some(TopAgentSummary {
                agent_hash: "abc123".to_string(),
                miner_uid: 1,
                score: 0.9,
                achieved_epoch: 10,
            }),
            config: DecayConfig::default(),
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: DecaySummary = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.competition_id, "test");
        assert!(deserialized.enabled);
        assert!(deserialized.decay_active);
    }

    #[test]
    fn test_applied_decay_serialization() {
        let applied = AppliedDecay {
            burn_percent: 10.0,
            burn_weight_added: 1000,
            original_total: 50000,
            adjusted_total: 49000,
        };

        let json = serde_json::to_string(&applied).unwrap();
        let deserialized: AppliedDecay = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.burn_percent, 10.0);
        assert_eq!(deserialized.burn_weight_added, 1000);
    }

    #[test]
    fn test_no_scores_decay_progression() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 2,
            decay_rate: 0.1,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        // Set initial top agent
        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();

        // Empty scores for subsequent epochs
        let empty: Vec<(u16, String, String, f64)> = vec![];
        manager.process_epoch("test", 2, &empty).unwrap();
        manager.process_epoch("test", 3, &empty).unwrap();
        manager.process_epoch("test", 4, &empty).unwrap();

        let state = manager.get_state("test").unwrap();
        let top = state.top_agent.as_ref().unwrap();
        assert!(top.decay_active);
        assert!(top.current_burn_percent > 0.0);
    }

    #[test]
    fn test_max_decay_reached_event() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 1,
            decay_rate: 0.5, // 50% per epoch
            max_burn_percent: 20.0,
            curve: DecayCurve::Linear,
            emit_events: true,
            ..Default::default()
        };

        manager.register_competition("test".into(), Some(config));

        let scores = create_test_scores(1);
        manager.process_epoch("test", 1, &scores).unwrap();
        manager.process_epoch("test", 2, &scores).unwrap();

        // This should trigger max decay
        let result = manager.process_epoch("test", 3, &scores).unwrap();

        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, DecayEvent::MaxDecayReached { .. })));
        assert!((result.burn_percent - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_decay_config_clone() {
        let config = DecayConfig {
            enabled: true,
            grace_epochs: 5,
            decay_rate: 0.15,
            max_burn_percent: 60.0,
            curve: DecayCurve::Exponential,
            min_improvement_threshold: 0.02,
            reset_on_any_improvement: true,
            emit_events: true,
        };

        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.grace_epochs, cloned.grace_epochs);
        assert_eq!(config.decay_rate, cloned.decay_rate);
    }

    #[test]
    fn test_default_manager() {
        let manager = RewardDecayManager::default();
        assert!(manager.states.is_empty());
    }

    /// Test with_default_config constructor
    #[test]
    fn test_with_default_config() {
        let custom_config = DecayConfig {
            enabled: false,
            grace_epochs: 20,
            decay_rate: 0.15,
            max_burn_percent: 50.0,
            curve: DecayCurve::Exponential,
            ..Default::default()
        };

        let mut manager = RewardDecayManager::with_default_config(custom_config.clone());
        assert!(manager.states.is_empty());

        // Register competition without explicit config - should use custom default
        manager.register_competition("test".into(), None);

        let state = manager.get_state("test").unwrap();
        assert!(!state.config.enabled); // Should use custom default
        assert_eq!(state.config.grace_epochs, 20);
        assert_eq!(state.config.decay_rate, 0.15);
        assert_eq!(state.config.max_burn_percent, 50.0);
        assert_eq!(state.config.curve, DecayCurve::Exponential);
    }

    /// Test update_config success
    #[test]
    fn test_update_config_success() {
        let mut manager = RewardDecayManager::new();
        manager.register_competition("test".into(), None);

        let state_before = manager.get_state("test").unwrap();
        let last_updated_before = state_before.last_updated;
        assert!(state_before.config.enabled);
        assert_eq!(state_before.config.grace_epochs, 10);

        // Update config
        let new_config = DecayConfig {
            enabled: false,
            grace_epochs: 5,
            decay_rate: 0.25,
            max_burn_percent: 40.0,
            curve: DecayCurve::Step {
                step_size: 15.0,
                step_epochs: 3,
            },
            ..Default::default()
        };

        let result = manager.update_config("test", new_config);
        assert!(result.is_ok());

        let state_after = manager.get_state("test").unwrap();
        assert!(!state_after.config.enabled);
        assert_eq!(state_after.config.grace_epochs, 5);
        assert_eq!(state_after.config.decay_rate, 0.25);
        assert_eq!(state_after.config.max_burn_percent, 40.0);
        assert!(state_after.last_updated >= last_updated_before);
    }

    /// Test update_config error for unregistered competition
    #[test]
    fn test_update_config_error() {
        let mut manager = RewardDecayManager::new();

        let new_config = DecayConfig::default();
        let result = manager.update_config("unknown", new_config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not registered"));
        assert!(err.contains("unknown"));
    }

    /// Test set_enabled success - enable
    #[test]
    fn test_set_enabled_enable() {
        let mut manager = RewardDecayManager::new();
        let config = DecayConfig {
            enabled: false,
            ..Default::default()
        };
        manager.register_competition("test".into(), Some(config));

        let state_before = manager.get_state("test").unwrap();
        assert!(!state_before.config.enabled);
        let last_updated_before = state_before.last_updated;

        // Enable decay
        let result = manager.set_enabled("test", true);
        assert!(result.is_ok());

        let state_after = manager.get_state("test").unwrap();
        assert!(state_after.config.enabled);
        assert!(state_after.last_updated >= last_updated_before);
    }

    /// Test set_enabled success - disable
    #[test]
    fn test_set_enabled_disable() {
        let mut manager = RewardDecayManager::new();
        manager.register_competition("test".into(), None); // Default is enabled

        let state_before = manager.get_state("test").unwrap();
        assert!(state_before.config.enabled);

        // Disable decay
        let result = manager.set_enabled("test", false);
        assert!(result.is_ok());

        let state_after = manager.get_state("test").unwrap();
        assert!(!state_after.config.enabled);
    }

    /// Test set_enabled error for unregistered competition
    #[test]
    fn test_set_enabled_error() {
        let mut manager = RewardDecayManager::new();

        let result = manager.set_enabled("unknown", true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not registered"));
    }

    // ------------------------------------------------------------------------
    // Time Decay Tests
    // ------------------------------------------------------------------------

    fn default_time_config() -> TimeDecayConfig {
        TimeDecayConfig {
            enabled: true,
            grace_period_hours: 48,
            half_life_hours: 24,
            min_multiplier: 0.01,
        }
    }

    #[test]
    fn test_time_no_decay_during_grace_period() {
        let config = default_time_config();

        // 24 hours ago - in grace period
        let submission_time = Utc::now() - Duration::hours(24);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert_eq!(multiplier, 1.0);

        // 48 hours ago - exactly at grace period boundary
        let submission_time = Utc::now() - Duration::hours(48);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert_eq!(multiplier, 1.0);
    }

    #[test]
    fn test_time_decay_after_grace_period() {
        let config = default_time_config();

        // 72 hours ago - 24 hours past grace (1 half-life = 50%)
        let submission_time = Utc::now() - Duration::hours(72);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert!(
            (multiplier - 0.5).abs() < 0.01,
            "After 24 hours past grace should be ~0.5, got {}",
            multiplier
        );

        // 96 hours ago - 48 hours past grace (2 half-lives = 25%)
        let submission_time = Utc::now() - Duration::hours(96);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert!(
            (multiplier - 0.25).abs() < 0.01,
            "After 48 hours past grace should be ~0.25, got {}",
            multiplier
        );

        // 120 hours ago - 72 hours past grace (3 half-lives = 12.5%)
        let submission_time = Utc::now() - Duration::hours(120);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert!(
            (multiplier - 0.125).abs() < 0.01,
            "After 72 hours past grace should be ~0.125, got {}",
            multiplier
        );
    }

    #[test]
    fn test_time_min_multiplier_cap() {
        let config = TimeDecayConfig {
            enabled: true,
            grace_period_hours: 48,
            half_life_hours: 24,
            min_multiplier: 0.1, // 10% minimum
        };

        // Many days past grace - would be very small without cap
        let submission_time = Utc::now() - Duration::hours(500);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert_eq!(multiplier, 0.1, "Should be capped at min_multiplier");
    }

    #[test]
    fn test_time_decay_disabled() {
        let config = TimeDecayConfig {
            enabled: false,
            ..default_time_config()
        };

        // Even after long time, no decay when disabled
        let submission_time = Utc::now() - Duration::hours(500);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert_eq!(multiplier, 1.0);
    }

    #[test]
    fn test_time_decay_info_in_grace() {
        let config = default_time_config();

        // 24 hours ago - in grace period
        let submission_time = Utc::now() - Duration::hours(24);
        let info = calculate_decay_info(submission_time, &config);

        assert!(!info.decay_active);
        assert!(info.grace_period_remaining_hours > 20.0);
        assert_eq!(info.multiplier, 1.0);
        assert_eq!(info.days_decaying, 0.0);
    }

    #[test]
    fn test_time_decay_info_after_grace() {
        let config = default_time_config();

        // 72 hours ago (24 hours past grace)
        let submission_time = Utc::now() - Duration::hours(72);
        let info = calculate_decay_info(submission_time, &config);

        assert!(info.decay_active);
        assert_eq!(info.grace_period_remaining_hours, 0.0);
        assert!(
            (info.multiplier - 0.5).abs() < 0.02,
            "Expected ~0.5, got {}",
            info.multiplier
        );
        assert!((info.days_decaying - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_half_decay_per_day() {
        let config = default_time_config();

        // Verify that after 1 day past grace, we have 50% decay
        let submission_time = Utc::now() - Duration::hours(48 + 24); // Grace + 1 day
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert!(
            (multiplier - 0.5).abs() < 0.01,
            "1 day past grace should be 50%, got {}",
            multiplier
        );

        // After 2 days past grace, we have 25% decay
        let submission_time = Utc::now() - Duration::hours(48 + 48); // Grace + 2 days
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert!(
            (multiplier - 0.25).abs() < 0.01,
            "2 days past grace should be 25%, got {}",
            multiplier
        );
    }

    #[test]
    fn test_time_decay_info_disabled() {
        let config = TimeDecayConfig {
            enabled: false,
            ..default_time_config()
        };

        // Even after long time, no decay when disabled
        let submission_time = Utc::now() - Duration::hours(500);
        let info = calculate_decay_info(submission_time, &config);

        assert!(!info.decay_active);
        assert_eq!(info.multiplier, 1.0);
        assert_eq!(info.grace_period_remaining_hours, 0.0);
        assert_eq!(info.days_decaying, 0.0);
        // age_hours should still reflect actual age
        assert!(info.age_hours > 400.0);
    }

    #[test]
    fn test_time_decay_config_default() {
        let config = TimeDecayConfig::default();

        assert!(config.enabled);
        assert_eq!(config.grace_period_hours, 48);
        assert_eq!(config.half_life_hours, 24);
        assert_eq!(config.min_multiplier, 0.01);
    }

    #[test]
    fn test_time_decay_config_response_from() {
        let config = TimeDecayConfig {
            enabled: true,
            grace_period_hours: 72,
            half_life_hours: 12,
            min_multiplier: 0.05,
        };

        let response = TimeDecayConfigResponse::from(&config);

        assert!(response.enabled);
        assert_eq!(response.grace_period_hours, 72);
        assert_eq!(response.half_life_hours, 12);
        assert_eq!(response.min_multiplier, 0.05);
    }

    #[test]
    fn test_time_decay_info_just_past_grace() {
        let config = default_time_config();

        // Just past grace period (1 minute)
        let submission_time = Utc::now() - Duration::hours(48) - Duration::minutes(1);
        let info = calculate_decay_info(submission_time, &config);

        assert!(info.decay_active);
        assert_eq!(info.grace_period_remaining_hours, 0.0);
        // Multiplier should be very close to 1.0 (just started decaying)
        assert!(info.multiplier > 0.99);
        // days_decaying should be very small
        assert!(info.days_decaying < 0.01);
    }

    #[test]
    fn test_time_decay_multiplier_exactly_at_grace_boundary() {
        let config = default_time_config();

        // Exactly at grace period boundary (should be 1.0)
        let submission_time = Utc::now() - Duration::hours(48);
        let multiplier = calculate_decay_multiplier(submission_time, &config);
        assert_eq!(multiplier, 1.0);
    }

    #[test]
    fn test_time_decay_info_fields_consistency() {
        let config = default_time_config();

        // Test various times and ensure fields are consistent
        for hours in [0, 24, 48, 72, 96, 200] {
            let submission_time = Utc::now() - Duration::hours(hours);
            let info = calculate_decay_info(submission_time, &config);

            // age_hours should roughly match
            assert!((info.age_hours - hours as f64).abs() < 1.0);

            // If in grace period, decay should not be active
            if hours <= 48 {
                assert!(!info.decay_active);
                assert!(info.grace_period_remaining_hours >= 0.0);
            } else {
                assert!(info.decay_active);
                assert_eq!(info.grace_period_remaining_hours, 0.0);
            }
        }
    }

    #[test]
    fn test_decay_status_response_serialization() {
        let response = DecayStatusResponse {
            winner: Some(WinnerDecayStatus {
                agent_hash: "abc123".to_string(),
                miner_hotkey: "5GrwvaEF...".to_string(),
                name: Some("TestAgent".to_string()),
                submitted_at: "2024-01-01T00:00:00Z".to_string(),
                age_hours: 72.0,
                grace_period_remaining_hours: 0.0,
                decay_active: true,
                decay_multiplier: 0.5,
                effective_weight: 0.5,
                days_decaying: 1.0,
            }),
            config: TimeDecayConfigResponse {
                enabled: true,
                grace_period_hours: 48,
                half_life_hours: 24,
                min_multiplier: 0.01,
            },
        };

        // Verify serialization works
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("TestAgent"));

        // Verify deserialization works
        let deserialized: DecayStatusResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.winner.is_some());
        let winner = deserialized.winner.unwrap();
        assert_eq!(winner.agent_hash, "abc123");
        assert_eq!(winner.decay_multiplier, 0.5);
    }

    #[test]
    fn test_decay_status_response_no_winner() {
        let response = DecayStatusResponse {
            winner: None,
            config: TimeDecayConfigResponse {
                enabled: false,
                grace_period_hours: 48,
                half_life_hours: 24,
                min_multiplier: 0.01,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: DecayStatusResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.winner.is_none());
        assert!(!deserialized.config.enabled);
    }

    #[test]
    fn test_from_env_defaults() {
        // Test from_env() uses defaults when env vars are not set
        // We can't easily set env vars in tests, but we can verify the function runs
        let config = TimeDecayConfig::from_env();
        // With no env vars set, should return defaults
        // Note: This may pick up actual env vars if set, so we just verify it doesn't panic
        assert!(config.grace_period_hours > 0);
        assert!(config.half_life_hours > 0);
        assert!(config.min_multiplier > 0.0);
    }

    #[test]
    fn test_decay_info_serialization() {
        let info = DecayInfo {
            multiplier: 0.75,
            age_hours: 60.0,
            grace_period_remaining_hours: 0.0,
            decay_active: true,
            days_decaying: 0.5,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: DecayInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.multiplier, 0.75);
        assert!(deserialized.decay_active);
    }

    #[test]
    fn test_winner_decay_status_fields() {
        let status = WinnerDecayStatus {
            agent_hash: "hash123".to_string(),
            miner_hotkey: "5Grwva...".to_string(),
            name: None,
            submitted_at: "2024-01-01T00:00:00Z".to_string(),
            age_hours: 100.0,
            grace_period_remaining_hours: 0.0,
            decay_active: true,
            decay_multiplier: 0.25,
            effective_weight: 0.25,
            days_decaying: 2.0,
        };

        assert_eq!(status.agent_hash, "hash123");
        assert!(status.name.is_none());
        assert!(status.decay_active);
    }
}
