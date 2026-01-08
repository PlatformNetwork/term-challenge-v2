//! Epoch Calculation for Term Challenge
//!
//! This module handles epoch calculation based on Bittensor block numbers.
//!
//! # Epoch Definition
//! - Epoch 0 starts at block 7,276,080
//! - Each epoch is `tempo` blocks (default 360, fetched from chain)
//! - Blocks before epoch 0 start block return epoch 0
//!
//! # Formula
//! ```text
//! if block >= EPOCH_ZERO_START_BLOCK:
//!     epoch = (block - EPOCH_ZERO_START_BLOCK) / tempo
//! else:
//!     epoch = 0
//! ```

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Block number where epoch 0 starts for term-challenge
pub const EPOCH_ZERO_START_BLOCK: u64 = 7_276_080;

/// Default tempo (blocks per epoch) - will be overridden from chain
pub const DEFAULT_TEMPO: u64 = 360;

/// Epoch phase within an epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpochPhase {
    /// Standard operation period (0% - 75% of epoch)
    Evaluation,
    /// Weight commitment window (75% - 87.5% of epoch)
    Commit,
    /// Weight reveal window (87.5% - 100% of epoch)
    Reveal,
}

impl std::fmt::Display for EpochPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpochPhase::Evaluation => write!(f, "evaluation"),
            EpochPhase::Commit => write!(f, "commit"),
            EpochPhase::Reveal => write!(f, "reveal"),
        }
    }
}

/// Current epoch state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochState {
    /// Current epoch number
    pub epoch: u64,
    /// Current block number
    pub block: u64,
    /// Current phase within the epoch
    pub phase: EpochPhase,
    /// Block where this epoch started
    pub epoch_start_block: u64,
    /// Blocks remaining in this epoch
    pub blocks_remaining: u64,
    /// Current tempo (blocks per epoch)
    pub tempo: u64,
}

/// Epoch calculator for term-challenge
///
/// Thread-safe calculator that maintains epoch state based on block numbers.
/// Tempo can be updated dynamically from chain data.
#[derive(Debug)]
pub struct EpochCalculator {
    /// Block where epoch 0 starts
    epoch_zero_start_block: u64,
    /// Current tempo (blocks per epoch)
    tempo: RwLock<u64>,
    /// Last known block
    last_block: RwLock<u64>,
    /// Last calculated epoch
    last_epoch: RwLock<u64>,
}

impl Default for EpochCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl EpochCalculator {
    /// Create a new epoch calculator with default settings
    pub fn new() -> Self {
        Self {
            epoch_zero_start_block: EPOCH_ZERO_START_BLOCK,
            tempo: RwLock::new(DEFAULT_TEMPO),
            last_block: RwLock::new(0),
            last_epoch: RwLock::new(0),
        }
    }

    /// Create calculator with custom tempo
    pub fn with_tempo(tempo: u64) -> Self {
        Self {
            epoch_zero_start_block: EPOCH_ZERO_START_BLOCK,
            tempo: RwLock::new(tempo),
            last_block: RwLock::new(0),
            last_epoch: RwLock::new(0),
        }
    }

    /// Create calculator with custom start block and tempo (for testing)
    pub fn with_config(epoch_zero_start_block: u64, tempo: u64) -> Self {
        Self {
            epoch_zero_start_block,
            tempo: RwLock::new(tempo),
            last_block: RwLock::new(0),
            last_epoch: RwLock::new(0),
        }
    }

    /// Get the epoch zero start block
    pub fn epoch_zero_start_block(&self) -> u64 {
        self.epoch_zero_start_block
    }

    /// Get current tempo
    pub fn tempo(&self) -> u64 {
        *self.tempo.read()
    }

    /// Update tempo (called when fetched from chain)
    pub fn set_tempo(&self, tempo: u64) {
        if tempo > 0 {
            let old_tempo = *self.tempo.read();
            if old_tempo != tempo {
                info!("Epoch tempo updated: {} -> {}", old_tempo, tempo);
                *self.tempo.write() = tempo;
            }
        } else {
            warn!("Ignoring invalid tempo: 0");
        }
    }

    /// Calculate epoch from block number
    ///
    /// Returns 0 for blocks before EPOCH_ZERO_START_BLOCK
    pub fn epoch_from_block(&self, block: u64) -> u64 {
        if block < self.epoch_zero_start_block {
            return 0;
        }

        let tempo = *self.tempo.read();
        if tempo == 0 {
            warn!("Tempo is 0, returning epoch 0");
            return 0;
        }

        (block - self.epoch_zero_start_block) / tempo
    }

    /// Get the start block for a given epoch
    pub fn start_block_for_epoch(&self, epoch: u64) -> u64 {
        let tempo = *self.tempo.read();
        self.epoch_zero_start_block + (epoch * tempo)
    }

    /// Get the end block for a given epoch (last block of the epoch)
    pub fn end_block_for_epoch(&self, epoch: u64) -> u64 {
        self.start_block_for_epoch(epoch + 1) - 1
    }

    /// Get blocks remaining in the current epoch
    pub fn blocks_remaining(&self, block: u64) -> u64 {
        if block < self.epoch_zero_start_block {
            return self.epoch_zero_start_block - block + *self.tempo.read();
        }

        let tempo = *self.tempo.read();
        let blocks_into_epoch = (block - self.epoch_zero_start_block) % tempo;
        tempo - blocks_into_epoch
    }

    /// Determine the current phase within an epoch
    ///
    /// Phases (percentage of tempo):
    /// - Evaluation: 0% - 75%
    /// - Commit: 75% - 87.5%
    /// - Reveal: 87.5% - 100%
    pub fn phase_for_block(&self, block: u64) -> EpochPhase {
        if block < self.epoch_zero_start_block {
            return EpochPhase::Evaluation;
        }

        let tempo = *self.tempo.read();
        if tempo == 0 {
            return EpochPhase::Evaluation;
        }

        let blocks_into_epoch = (block - self.epoch_zero_start_block) % tempo;

        let commit_start = (tempo * 3) / 4; // 75%
        let reveal_start = (tempo * 7) / 8; // 87.5%

        if blocks_into_epoch >= reveal_start {
            EpochPhase::Reveal
        } else if blocks_into_epoch >= commit_start {
            EpochPhase::Commit
        } else {
            EpochPhase::Evaluation
        }
    }

    /// Get complete epoch state for a block
    pub fn get_state(&self, block: u64) -> EpochState {
        let epoch = self.epoch_from_block(block);
        let tempo = *self.tempo.read();
        let epoch_start_block = self.start_block_for_epoch(epoch);
        let blocks_remaining = self.blocks_remaining(block);
        let phase = self.phase_for_block(block);

        EpochState {
            epoch,
            block,
            phase,
            epoch_start_block,
            blocks_remaining,
            tempo,
        }
    }

    /// Update with a new block and check for epoch transition
    ///
    /// Returns Some(new_epoch) if epoch changed, None otherwise
    pub fn on_new_block(&self, block: u64) -> Option<EpochTransition> {
        let new_epoch = self.epoch_from_block(block);
        let old_epoch = *self.last_epoch.read();
        let old_block = *self.last_block.read();

        // Update state
        *self.last_block.write() = block;
        *self.last_epoch.write() = new_epoch;

        if new_epoch > old_epoch && old_block > 0 {
            info!(
                "Epoch transition: {} -> {} at block {}",
                old_epoch, new_epoch, block
            );
            Some(EpochTransition {
                old_epoch,
                new_epoch,
                block,
            })
        } else {
            None
        }
    }

    /// Get last known block
    pub fn last_block(&self) -> u64 {
        *self.last_block.read()
    }

    /// Get last known epoch
    pub fn last_epoch(&self) -> u64 {
        *self.last_epoch.read()
    }

    /// Get current epoch (alias for last_epoch)
    pub fn current_epoch(&self) -> u64 {
        *self.last_epoch.read()
    }
}

/// Epoch transition event
#[derive(Debug, Clone)]
pub struct EpochTransition {
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub block: u64,
}

/// Shared epoch calculator instance
pub type SharedEpochCalculator = Arc<EpochCalculator>;

/// Create a new shared epoch calculator
pub fn create_epoch_calculator() -> SharedEpochCalculator {
    Arc::new(EpochCalculator::new())
}

/// Create a shared epoch calculator with custom tempo
pub fn create_epoch_calculator_with_tempo(tempo: u64) -> SharedEpochCalculator {
    Arc::new(EpochCalculator::with_tempo(tempo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_calculation_before_start() {
        let calc = EpochCalculator::new();

        // Blocks before epoch 0 start should return epoch 0
        assert_eq!(calc.epoch_from_block(0), 0);
        assert_eq!(calc.epoch_from_block(1_000_000), 0);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK - 1), 0);
    }

    #[test]
    fn test_epoch_calculation_at_start() {
        let calc = EpochCalculator::new();

        // Block at epoch 0 start should be epoch 0
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK), 0);

        // First block of epoch 1
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 360), 1);

        // Last block of epoch 0
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 359), 0);
    }

    #[test]
    fn test_epoch_calculation_various_blocks() {
        let calc = EpochCalculator::new();

        // Epoch 0: blocks 7,276,080 - 7,276,439
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK), 0);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 100), 0);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 359), 0);

        // Epoch 1: blocks 7,276,440 - 7,276,799
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 360), 1);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 500), 1);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 719), 1);

        // Epoch 2: blocks 7,276,800 - 7,277,159
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 720), 2);

        // Epoch 100
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 36000), 100);
    }

    #[test]
    fn test_start_block_for_epoch() {
        let calc = EpochCalculator::new();

        assert_eq!(calc.start_block_for_epoch(0), EPOCH_ZERO_START_BLOCK);
        assert_eq!(calc.start_block_for_epoch(1), EPOCH_ZERO_START_BLOCK + 360);
        assert_eq!(calc.start_block_for_epoch(2), EPOCH_ZERO_START_BLOCK + 720);
        assert_eq!(
            calc.start_block_for_epoch(100),
            EPOCH_ZERO_START_BLOCK + 36000
        );
    }

    #[test]
    fn test_blocks_remaining() {
        let calc = EpochCalculator::new();

        // First block of epoch 0
        assert_eq!(calc.blocks_remaining(EPOCH_ZERO_START_BLOCK), 360);

        // Middle of epoch 0
        assert_eq!(calc.blocks_remaining(EPOCH_ZERO_START_BLOCK + 100), 260);

        // Last block of epoch 0
        assert_eq!(calc.blocks_remaining(EPOCH_ZERO_START_BLOCK + 359), 1);

        // First block of epoch 1
        assert_eq!(calc.blocks_remaining(EPOCH_ZERO_START_BLOCK + 360), 360);
    }

    #[test]
    fn test_phase_calculation() {
        let calc = EpochCalculator::new();

        // Evaluation phase: 0-74% (blocks 0-269)
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK),
            EpochPhase::Evaluation
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 100),
            EpochPhase::Evaluation
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 269),
            EpochPhase::Evaluation
        );

        // Commit phase: 75-87.5% (blocks 270-314)
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 270),
            EpochPhase::Commit
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 300),
            EpochPhase::Commit
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 314),
            EpochPhase::Commit
        );

        // Reveal phase: 87.5-100% (blocks 315-359)
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 315),
            EpochPhase::Reveal
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 350),
            EpochPhase::Reveal
        );
        assert_eq!(
            calc.phase_for_block(EPOCH_ZERO_START_BLOCK + 359),
            EpochPhase::Reveal
        );
    }

    #[test]
    fn test_epoch_transition() {
        let calc = EpochCalculator::new();

        // First update - no transition
        assert!(calc.on_new_block(EPOCH_ZERO_START_BLOCK + 100).is_none());

        // Still in epoch 0 - no transition
        assert!(calc.on_new_block(EPOCH_ZERO_START_BLOCK + 200).is_none());

        // Transition to epoch 1
        let transition = calc.on_new_block(EPOCH_ZERO_START_BLOCK + 360);
        assert!(transition.is_some());
        let t = transition.unwrap();
        assert_eq!(t.old_epoch, 0);
        assert_eq!(t.new_epoch, 1);

        // Still in epoch 1 - no transition
        assert!(calc.on_new_block(EPOCH_ZERO_START_BLOCK + 500).is_none());
    }

    #[test]
    fn test_tempo_update() {
        let calc = EpochCalculator::new();

        assert_eq!(calc.tempo(), 360);

        calc.set_tempo(100);
        assert_eq!(calc.tempo(), 100);

        // With tempo 100, epoch calculation changes
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 100), 1);
        assert_eq!(calc.epoch_from_block(EPOCH_ZERO_START_BLOCK + 200), 2);
    }

    #[test]
    fn test_get_state() {
        let calc = EpochCalculator::new();

        let state = calc.get_state(EPOCH_ZERO_START_BLOCK + 100);

        assert_eq!(state.epoch, 0);
        assert_eq!(state.block, EPOCH_ZERO_START_BLOCK + 100);
        assert_eq!(state.phase, EpochPhase::Evaluation);
        assert_eq!(state.epoch_start_block, EPOCH_ZERO_START_BLOCK);
        assert_eq!(state.blocks_remaining, 260);
        assert_eq!(state.tempo, 360);
    }

    #[test]
    fn test_custom_config() {
        // Test with custom start block and tempo
        let calc = EpochCalculator::with_config(1000, 100);

        assert_eq!(calc.epoch_from_block(999), 0);
        assert_eq!(calc.epoch_from_block(1000), 0);
        assert_eq!(calc.epoch_from_block(1099), 0);
        assert_eq!(calc.epoch_from_block(1100), 1);
        assert_eq!(calc.epoch_from_block(1200), 2);
    }
}
