//! Epoch Manager
//!
//! Manages epoch transitions and phase changes.

use crate::{EpochConfig, EpochPhase, EpochState};
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::info;

/// Epoch manager
pub struct EpochManager {
    config: RwLock<EpochConfig>,
    state: Arc<RwLock<EpochState>>,
}

impl EpochManager {
    /// Create a new epoch manager
    pub fn new(config: EpochConfig, current_block: u64) -> Self {
        let epoch = current_block / config.blocks_per_epoch;
        let start_block = epoch * config.blocks_per_epoch;

        let state = EpochState::new(epoch, start_block, &config);

        Self {
            config: RwLock::new(config),
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Update configuration (e.g., when syncing with Bittensor tempo)
    pub fn update_config(&self, new_config: EpochConfig) {
        info!(
            "Updating epoch config: blocks_per_epoch={}, eval={}, commit={}, reveal={}",
            new_config.blocks_per_epoch,
            new_config.evaluation_blocks,
            new_config.commit_blocks,
            new_config.reveal_blocks
        );
        *self.config.write() = new_config;
    }

    /// Get current epoch state
    pub fn state(&self) -> EpochState {
        self.state.read().clone()
    }

    /// Get current epoch number
    pub fn current_epoch(&self) -> u64 {
        self.state.read().epoch
    }

    /// Get current phase
    pub fn current_phase(&self) -> EpochPhase {
        self.state.read().phase
    }

    /// Update with new block
    pub fn on_new_block(&self, block_number: u64) -> Option<EpochTransition> {
        let config = self.config.read();
        let mut state = self.state.write();
        state.current_block = block_number;

        // Check if we've moved to a new epoch
        let new_epoch = block_number / config.blocks_per_epoch;
        if new_epoch > state.epoch {
            let old_epoch = state.epoch;
            state.epoch = new_epoch;
            state.start_block = new_epoch * config.blocks_per_epoch;
            state.phase = EpochPhase::Evaluation;
            state.blocks_remaining = config.evaluation_blocks;

            info!("New epoch started: {} -> {}", old_epoch, new_epoch);

            return Some(EpochTransition::NewEpoch {
                old_epoch,
                new_epoch,
            });
        }

        // Calculate position within epoch
        let blocks_into_epoch = block_number - state.start_block;

        // Determine phase (use saturating_sub to prevent overflow)
        let (new_phase, blocks_remaining) = if blocks_into_epoch < config.evaluation_blocks {
            (
                EpochPhase::Evaluation,
                config.evaluation_blocks.saturating_sub(blocks_into_epoch),
            )
        } else if blocks_into_epoch < config.evaluation_blocks + config.commit_blocks {
            (
                EpochPhase::Commit,
                (config.evaluation_blocks + config.commit_blocks).saturating_sub(blocks_into_epoch),
            )
        } else if blocks_into_epoch
            < config.evaluation_blocks + config.commit_blocks + config.reveal_blocks
        {
            (
                EpochPhase::Reveal,
                (config.evaluation_blocks + config.commit_blocks + config.reveal_blocks)
                    .saturating_sub(blocks_into_epoch),
            )
        } else {
            (
                EpochPhase::Finalization,
                config.blocks_per_epoch.saturating_sub(blocks_into_epoch),
            )
        };

        // Check for phase transition
        if new_phase != state.phase {
            let old_phase = state.phase;
            state.phase = new_phase;
            state.blocks_remaining = blocks_remaining;

            info!(
                "Epoch {} phase transition: {} -> {}",
                state.epoch, old_phase, new_phase
            );

            return Some(EpochTransition::PhaseChange {
                epoch: state.epoch,
                old_phase,
                new_phase,
            });
        }

        state.blocks_remaining = blocks_remaining;
        None
    }

    /// Check if we can submit commitments
    pub fn can_commit(&self) -> bool {
        self.state.read().phase == EpochPhase::Commit
    }

    /// Check if we can reveal weights
    pub fn can_reveal(&self) -> bool {
        self.state.read().phase == EpochPhase::Reveal
    }

    /// Check if weights are being finalized
    pub fn is_finalizing(&self) -> bool {
        self.state.read().phase == EpochPhase::Finalization
    }

    /// Get blocks until next phase
    pub fn blocks_until_next_phase(&self) -> u64 {
        self.state.read().blocks_remaining
    }

    /// Get epoch for a given block
    pub fn epoch_for_block(&self, block: u64) -> u64 {
        block / self.config.read().blocks_per_epoch
    }

    /// Get start block for an epoch
    pub fn start_block_for_epoch(&self, epoch: u64) -> u64 {
        epoch * self.config.read().blocks_per_epoch
    }

    /// Get phase for a given block
    pub fn phase_for_block(&self, block: u64) -> EpochPhase {
        let config = self.config.read();
        let epoch_start = self.start_block_for_epoch(self.epoch_for_block(block));
        let blocks_into_epoch = block - epoch_start;

        if blocks_into_epoch < config.evaluation_blocks {
            EpochPhase::Evaluation
        } else if blocks_into_epoch < config.evaluation_blocks + config.commit_blocks {
            EpochPhase::Commit
        } else if blocks_into_epoch
            < config.evaluation_blocks + config.commit_blocks + config.reveal_blocks
        {
            EpochPhase::Reveal
        } else {
            EpochPhase::Finalization
        }
    }
}

/// Epoch transition event
#[derive(Clone, Debug)]
pub enum EpochTransition {
    /// New epoch started
    NewEpoch { old_epoch: u64, new_epoch: u64 },
    /// Phase changed within epoch
    PhaseChange {
        epoch: u64,
        old_phase: EpochPhase,
        new_phase: EpochPhase,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_manager() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        assert_eq!(manager.current_epoch(), 0);
        assert_eq!(manager.current_phase(), EpochPhase::Evaluation);
    }

    #[test]
    fn test_phase_transitions() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        // Should be in evaluation at block 0
        assert_eq!(manager.current_phase(), EpochPhase::Evaluation);

        // Move to commit phase
        let transition = manager.on_new_block(70);
        assert!(matches!(
            transition,
            Some(EpochTransition::PhaseChange {
                new_phase: EpochPhase::Commit,
                ..
            })
        ));

        // Move to reveal phase
        let transition = manager.on_new_block(85);
        assert!(matches!(
            transition,
            Some(EpochTransition::PhaseChange {
                new_phase: EpochPhase::Reveal,
                ..
            })
        ));

        // Move to new epoch
        let transition = manager.on_new_block(100);
        assert!(matches!(
            transition,
            Some(EpochTransition::NewEpoch { new_epoch: 1, .. })
        ));
    }

    #[test]
    fn test_can_commit() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config.clone(), 0);
        // Start at block 70 (start of commit phase)
        manager.on_new_block(70);
        assert!(manager.can_commit());
        assert!(!manager.can_reveal());
        assert!(!manager.is_finalizing());
    }

    #[test]
    fn test_can_reveal() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config.clone(), 0);
        // Start at block 85 (start of reveal phase)
        manager.on_new_block(85);
        assert!(!manager.can_commit());
        assert!(manager.can_reveal());
        assert!(!manager.is_finalizing());
    }

    #[test]
    fn test_is_finalizing() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 10,
            ..Default::default()
        };

        let manager = EpochManager::new(config.clone(), 0);
        // Start at block 95 (start of finalization phase)
        manager.on_new_block(95);
        assert!(!manager.can_commit());
        assert!(!manager.can_reveal());
        assert!(manager.is_finalizing());
    }

    #[test]
    fn test_blocks_until_next_phase() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);
        assert_eq!(manager.blocks_until_next_phase(), 70);

        manager.on_new_block(50);
        assert_eq!(manager.blocks_until_next_phase(), 20);
    }

    #[test]
    fn test_epoch_for_block() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);
        assert_eq!(manager.epoch_for_block(0), 0);
        assert_eq!(manager.epoch_for_block(99), 0);
        assert_eq!(manager.epoch_for_block(100), 1);
        assert_eq!(manager.epoch_for_block(250), 2);
    }

    #[test]
    fn test_start_block_for_epoch() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);
        assert_eq!(manager.start_block_for_epoch(0), 0);
        assert_eq!(manager.start_block_for_epoch(1), 100);
        assert_eq!(manager.start_block_for_epoch(5), 500);
    }

    #[test]
    fn test_phase_for_block() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 10,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);
        assert_eq!(manager.phase_for_block(0), EpochPhase::Evaluation);
        assert_eq!(manager.phase_for_block(50), EpochPhase::Evaluation);
        assert_eq!(manager.phase_for_block(70), EpochPhase::Commit);
        assert_eq!(manager.phase_for_block(85), EpochPhase::Reveal);
        assert_eq!(manager.phase_for_block(95), EpochPhase::Finalization);
    }

    #[test]
    fn test_update_config() {
        let config = EpochConfig::default();
        let manager = EpochManager::new(config, 0);

        let new_config = EpochConfig {
            blocks_per_epoch: 200,
            evaluation_blocks: 150,
            commit_blocks: 25,
            reveal_blocks: 25,
            ..Default::default()
        };

        manager.update_config(new_config);
        // Config should be updated and used for subsequent calculations
    }

    #[test]
    fn test_state_clone() {
        let config = EpochConfig::default();
        let manager = EpochManager::new(config, 0);

        let state1 = manager.state();
        let state2 = manager.state();

        assert_eq!(state1.epoch, state2.epoch);
        assert_eq!(state1.phase, state2.phase);
    }

    #[test]
    fn test_no_transition_same_phase() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        // Move within same phase
        let transition = manager.on_new_block(10);
        assert!(transition.is_none());

        let transition = manager.on_new_block(20);
        assert!(transition.is_none());
    }

    #[test]
    fn test_finalization_phase() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 10,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        // Move to finalization
        manager.on_new_block(95);
        assert_eq!(manager.current_phase(), EpochPhase::Finalization);
    }

    #[test]
    fn test_epoch_transition_across_multiple_epochs() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        // Jump multiple epochs
        let transition = manager.on_new_block(250);
        assert!(matches!(
            transition,
            Some(EpochTransition::NewEpoch {
                old_epoch: 0,
                new_epoch: 2,
                ..
            })
        ));

        assert_eq!(manager.current_epoch(), 2);
    }

    #[test]
    fn test_blocks_remaining_decreases() {
        let config = EpochConfig {
            blocks_per_epoch: 100,
            evaluation_blocks: 70,
            commit_blocks: 15,
            reveal_blocks: 15,
            ..Default::default()
        };

        let manager = EpochManager::new(config, 0);

        let initial_remaining = manager.blocks_until_next_phase();
        manager.on_new_block(10);
        let new_remaining = manager.blocks_until_next_phase();

        assert!(new_remaining < initial_remaining);
    }
}
