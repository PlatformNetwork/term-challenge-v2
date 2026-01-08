//! Block Synchronization for Term Challenge
//!
//! Subscribes to block events from platform server and syncs epoch state.
//!
//! This module:
//! - Connects to platform server to receive block updates
//! - Fetches current tempo from chain
//! - Updates the epoch calculator on each new block
//! - Notifies listeners of epoch transitions

use crate::epoch::{EpochCalculator, EpochTransition, SharedEpochCalculator};
use crate::pg_storage::PgStorage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Block event from platform server
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum BlockEvent {
    /// New block received
    #[serde(rename = "new_block")]
    NewBlock {
        block_number: u64,
        #[serde(default)]
        tempo: Option<u64>,
    },
    /// Epoch transition
    #[serde(rename = "epoch_transition")]
    EpochTransition {
        old_epoch: u64,
        new_epoch: u64,
        block: u64,
    },
    /// Network state update
    #[serde(rename = "network_state")]
    NetworkState {
        block_number: u64,
        tempo: u64,
        epoch: u64,
    },
}

/// Events emitted by the block sync
#[derive(Debug, Clone)]
pub enum BlockSyncEvent {
    /// New block received
    NewBlock { block: u64, epoch: u64 },
    /// Epoch changed
    EpochTransition(EpochTransition),
    /// Connected to platform
    Connected,
    /// Disconnected from platform
    Disconnected(String),
    /// Tempo updated
    TempoUpdated { old_tempo: u64, new_tempo: u64 },
}

/// Configuration for block sync
#[derive(Debug, Clone)]
pub struct BlockSyncConfig {
    /// Platform server URL
    pub platform_url: String,
    /// Poll interval for REST fallback (seconds)
    pub poll_interval_secs: u64,
    /// Enable WebSocket subscription (if available)
    pub use_websocket: bool,
    /// Event channel capacity
    pub channel_capacity: usize,
}

impl Default for BlockSyncConfig {
    fn default() -> Self {
        Self {
            platform_url: "https://chain.platform.network".to_string(),
            poll_interval_secs: 12, // ~1 block
            use_websocket: true,
            channel_capacity: 100,
        }
    }
}

/// Network state response from platform API
#[derive(Debug, Clone, Deserialize)]
pub struct NetworkStateResponse {
    pub current_block: u64,
    pub current_epoch: u64,
    pub tempo: u64,
    #[serde(default)]
    pub phase: Option<String>,
}

/// Block synchronizer
///
/// Keeps the epoch calculator in sync with the blockchain by:
/// 1. Polling platform server for current block/tempo
/// 2. Updating epoch calculator on each new block
/// 3. Broadcasting epoch transition events
pub struct BlockSync {
    config: BlockSyncConfig,
    epoch_calculator: SharedEpochCalculator,
    storage: Option<Arc<PgStorage>>,
    event_tx: broadcast::Sender<BlockSyncEvent>,
    running: Arc<RwLock<bool>>,
    http_client: reqwest::Client,
}

impl BlockSync {
    /// Create a new block sync
    pub fn new(
        config: BlockSyncConfig,
        epoch_calculator: SharedEpochCalculator,
        storage: Option<Arc<PgStorage>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(config.channel_capacity);

        Self {
            config,
            epoch_calculator,
            storage,
            event_tx,
            running: Arc::new(RwLock::new(false)),
            http_client: reqwest::Client::new(),
        }
    }

    /// Subscribe to block sync events
    pub fn subscribe(&self) -> broadcast::Receiver<BlockSyncEvent> {
        self.event_tx.subscribe()
    }

    /// Get the epoch calculator
    pub fn epoch_calculator(&self) -> &SharedEpochCalculator {
        &self.epoch_calculator
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        self.epoch_calculator.current_epoch()
    }

    /// Get current block
    pub fn current_block(&self) -> u64 {
        self.epoch_calculator.last_block()
    }

    /// Fetch current network state from platform
    pub async fn fetch_network_state(&self) -> Result<NetworkStateResponse, String> {
        let url = format!("{}/api/v1/network/state", self.config.platform_url);

        let response = self
            .http_client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch network state: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Network state request failed: {}",
                response.status()
            ));
        }

        response
            .json::<NetworkStateResponse>()
            .await
            .map_err(|e| format!("Failed to parse network state: {}", e))
    }

    /// Fetch tempo from platform
    pub async fn fetch_tempo(&self) -> Result<u64, String> {
        let state = self.fetch_network_state().await?;
        Ok(state.tempo)
    }

    /// Initialize by fetching current state
    pub async fn init(&self) -> Result<(), String> {
        info!("Initializing block sync from {}", self.config.platform_url);

        match self.fetch_network_state().await {
            Ok(state) => {
                // Update tempo
                if state.tempo > 0 {
                    self.epoch_calculator.set_tempo(state.tempo);
                    info!("Initialized tempo: {}", state.tempo);
                }

                // Process the current block
                self.process_block(state.current_block).await;

                info!(
                    "Block sync initialized: block={}, epoch={}, tempo={}",
                    state.current_block,
                    self.epoch_calculator.current_epoch(),
                    self.epoch_calculator.tempo()
                );

                Ok(())
            }
            Err(e) => {
                warn!("Failed to initialize block sync: {}", e);
                Err(e)
            }
        }
    }

    /// Process a new block
    async fn process_block(&self, block: u64) {
        // Check for epoch transition
        if let Some(transition) = self.epoch_calculator.on_new_block(block) {
            let epoch = transition.new_epoch;

            // Update database
            if let Some(ref storage) = self.storage {
                if let Err(e) = storage.set_current_epoch(epoch as i64).await {
                    error!("Failed to update epoch in database: {}", e);
                }
            }

            // Broadcast transition event
            let _ = self
                .event_tx
                .send(BlockSyncEvent::EpochTransition(transition));
        }

        // Broadcast new block event
        let _ = self.event_tx.send(BlockSyncEvent::NewBlock {
            block,
            epoch: self.epoch_calculator.current_epoch(),
        });
    }

    /// Start the block sync polling loop
    pub async fn start(&self) -> Result<(), String> {
        // Check if already running
        {
            let mut running = self.running.write().await;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        // Initialize first
        if let Err(e) = self.init().await {
            warn!("Initial sync failed, will retry: {}", e);
        }

        let running = self.running.clone();
        let platform_url = self.config.platform_url.clone();
        let poll_interval = Duration::from_secs(self.config.poll_interval_secs);
        let epoch_calculator = self.epoch_calculator.clone();
        let storage = self.storage.clone();
        let event_tx = self.event_tx.clone();
        let http_client = self.http_client.clone();

        // Start polling task
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;

            loop {
                if !*running.read().await {
                    info!("Block sync stopped");
                    break;
                }

                let url = format!("{}/api/v1/network/state", platform_url);

                match http_client
                    .get(&url)
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => {
                        match response.json::<NetworkStateResponse>().await {
                            Ok(state) => {
                                consecutive_failures = 0;

                                // Update tempo if changed
                                let current_tempo = epoch_calculator.tempo();
                                if state.tempo > 0 && state.tempo != current_tempo {
                                    epoch_calculator.set_tempo(state.tempo);
                                    let _ = event_tx.send(BlockSyncEvent::TempoUpdated {
                                        old_tempo: current_tempo,
                                        new_tempo: state.tempo,
                                    });
                                }

                                // Process block
                                if let Some(transition) =
                                    epoch_calculator.on_new_block(state.current_block)
                                {
                                    let epoch = transition.new_epoch;

                                    // Update database
                                    if let Some(ref storage) = storage {
                                        if let Err(e) =
                                            storage.set_current_epoch(epoch as i64).await
                                        {
                                            error!("Failed to update epoch in database: {}", e);
                                        }
                                    }

                                    // Broadcast transition
                                    let _ =
                                        event_tx.send(BlockSyncEvent::EpochTransition(transition));
                                }

                                // Broadcast new block
                                let _ = event_tx.send(BlockSyncEvent::NewBlock {
                                    block: state.current_block,
                                    epoch: epoch_calculator.current_epoch(),
                                });

                                debug!(
                                    "Block sync: block={}, epoch={}, tempo={}",
                                    state.current_block,
                                    epoch_calculator.current_epoch(),
                                    epoch_calculator.tempo()
                                );
                            }
                            Err(e) => {
                                consecutive_failures += 1;
                                warn!(
                                    "Failed to parse network state: {} (attempt {})",
                                    e, consecutive_failures
                                );
                            }
                        }
                    }
                    Ok(response) => {
                        consecutive_failures += 1;
                        warn!(
                            "Network state request failed: {} (attempt {})",
                            response.status(),
                            consecutive_failures
                        );
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        warn!(
                            "Failed to fetch network state: {} (attempt {})",
                            e, consecutive_failures
                        );

                        if consecutive_failures >= 3 {
                            let _ = event_tx.send(BlockSyncEvent::Disconnected(e.to_string()));
                        }
                    }
                }

                // Exponential backoff on failures
                let sleep_duration = if consecutive_failures > 0 {
                    poll_interval * (1 << consecutive_failures.min(5))
                } else {
                    poll_interval
                };

                tokio::time::sleep(sleep_duration).await;
            }
        });

        info!(
            "Block sync started (polling every {}s)",
            self.config.poll_interval_secs
        );
        Ok(())
    }

    /// Stop the block sync
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// Create a block sync from environment variables
pub fn create_from_env(
    epoch_calculator: SharedEpochCalculator,
    storage: Option<Arc<PgStorage>>,
) -> BlockSync {
    let platform_url = std::env::var("PLATFORM_URL")
        .unwrap_or_else(|_| "https://chain.platform.network".to_string());

    let poll_interval = std::env::var("BLOCK_SYNC_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);

    let config = BlockSyncConfig {
        platform_url,
        poll_interval_secs: poll_interval,
        ..Default::default()
    };

    BlockSync::new(config, epoch_calculator, storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epoch::create_epoch_calculator;

    #[test]
    fn test_block_sync_config_default() {
        let config = BlockSyncConfig::default();
        assert_eq!(config.poll_interval_secs, 12);
        assert!(config.use_websocket);
    }

    #[tokio::test]
    async fn test_block_sync_creation() {
        let calc = create_epoch_calculator();
        let config = BlockSyncConfig::default();
        let sync = BlockSync::new(config, calc, None);

        assert_eq!(sync.current_epoch(), 0);
        assert_eq!(sync.current_block(), 0);
        assert!(!sync.is_running().await);
    }

    #[tokio::test]
    async fn test_block_sync_subscribe() {
        let calc = create_epoch_calculator();
        let config = BlockSyncConfig::default();
        let sync = BlockSync::new(config, calc, None);

        let mut rx = sync.subscribe();

        // Process a block manually
        sync.process_block(7_276_080).await;

        // Should receive the event
        let event = rx.try_recv();
        assert!(event.is_ok());
    }
}
