//! Mock Subtensor Binary
//!
//! Simulates a Bittensor RPC node for testing without real chain connectivity.
//! Implements WebSocket JSON-RPC 2.0 server with Substrate-compatible methods:
//! - chain_getHeader, chain_getBlock, chain_getBlockHash, chain_getFinalizedHead
//! - state_getMetadata, state_getStorage, state_getKeys, state_getRuntimeVersion
//! - system_health, system_version, system_name, system_properties, system_peers
//! - author_submitExtrinsic
//!
//! Features:
//! - Simulated block production with configurable tempo (default 12s blocks)
//! - Mock metagraph state with 256 synthetic validators and realistic stake distribution
//! - Weight submissions with commit-reveal mechanism simulation
//! - Test inspection endpoints

use anyhow::Result;
use clap::Parser;
use parking_lot::RwLock;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::info;

mod chain;
mod jsonrpc;
mod state;
mod websocket;

use chain::Chain;
use state::MockMetagraph;
use websocket::WsServer;

/// Mock Subtensor Configuration
#[derive(Parser, Debug, Clone)]
#[command(name = "mock-subtensor")]
#[command(about = "Mock Bittensor RPC node for testing")]
pub struct Config {
    /// HTTP/WS listen address
    #[arg(short, long, default_value = "0.0.0.0:9944")]
    pub bind: SocketAddr,

    /// Block production tempo in seconds
    #[arg(short, long, default_value = "12")]
    pub tempo: u64,

    /// Subnet UID (netuid)
    #[arg(long, default_value = "100")]
    pub netuid: u16,

    /// Number of synthetic validators
    #[arg(long, default_value = "256")]
    pub validator_count: u16,

    /// Minimum stake for validators (in RAO)
    #[arg(long, default_value = "1000000000000")]
    pub min_stake: u64,

    /// Enable commit-reveal mechanism
    #[arg(long, default_value = "true")]
    pub commit_reveal: bool,

    /// Reveal period in blocks
    #[arg(long, default_value = "12")]
    pub reveal_period: u64,

    /// Log level
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Enable test inspection endpoints
    #[arg(long, default_value = "true")]
    pub inspection: bool,
}

/// Shared application state
pub struct AppState {
    pub chain: Arc<RwLock<Chain>>,
    pub metagraph: Arc<RwLock<MockMetagraph>>,
    pub config: Config,
    pub broadcast_tx: broadcast::Sender<Value>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let chain = Arc::new(RwLock::new(Chain::new(&config)));
        let metagraph = Arc::new(RwLock::new(MockMetagraph::new(&config)));
        let (broadcast_tx, _rx) = broadcast::channel(256);

        Self {
            chain,
            metagraph,
            config,
            broadcast_tx,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();

    // Initialize tracing
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .init();

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║               Mock Subtensor RPC Node                        ║");
    info!("╚══════════════════════════════════════════════════════════════╝");
    info!("Bind address: {}", config.bind);
    info!("Block tempo: {}s", config.tempo);
    info!("NetUID: {}", config.netuid);
    info!("Validators: {}", config.validator_count);
    info!("Commit-reveal: {}", config.commit_reveal);
    info!("");
    info!("Methods available:");
    info!("  - chain_getHeader, chain_getBlock, chain_getBlockHash");
    info!("  - state_getMetadata, state_getStorage, state_getKeys");
    info!("  - system_health, system_version, system_name");
    info!("  - author_submitExtrinsic");
    info!("  - subtensor_commitWeights, subtensor_revealWeights");
    info!("  - subtensor_getNeurons, subtensor_getNeuronLite");
    info!("");
    info!("Test endpoints:");
    info!("  - GET /test/state - Current chain state");
    info!("  - GET /test/metagraph - Full metagraph info");
    info!("  - GET /test/weights - Pending weight commits");
    info!("  - POST /test/advance - Advance block manually");
    info!("");

    let state = Arc::new(AppState::new(config.clone()));

    // Spawn block production task
    let block_state = state.clone();
    let _block_task = tokio::spawn(block_production_task(block_state, config.tempo));

    // Spawn WebSocket server
    let ws_server = WsServer::new(state);
    ws_server.run(config.bind).await
}

/// Block production background task
async fn block_production_task(state: Arc<AppState>, tempo: u64) {
    let mut ticker = interval(Duration::from_secs(tempo));
    ticker.tick().await; // Skip first tick

    loop {
        ticker.tick().await;

        let mut chain = state.chain.write();
        let block = chain.produce_block();
        let block_number = block.header.number;
        let block_hash = format!("0x{}", hex::encode(&block.hash[0..4]));

        info!("Block #{} produced (hash: ...{})", block_number, block_hash);

        // Drop lock before broadcasting
        drop(chain);

        // Notify WebSocket subscribers
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "chain_newHead",
            "params": {
                "result": {
                    "number": block_number,
                    "hash": format!("0x{}", hex::encode(block.hash)),
                    "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
                },
                "subscription": "chain"
            }
        });

        let _ = state.broadcast_tx.send(notification);

        // Check epoch boundary for commit-reveal
        let blocks_per_epoch = 100u64;
        let block_in_epoch = block_number % blocks_per_epoch;

        if block_in_epoch == 75 {
            info!("=== COMMIT WINDOW OPEN (epoch boundary) ===");
        } else if block_in_epoch == 88 {
            info!("=== REVEAL WINDOW OPEN ===");
        }
    }
}
