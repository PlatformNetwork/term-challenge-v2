//! Term Challenge Server
//!
//! Runs the term-challenge as a standalone HTTP server for the platform validator.

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use term_challenge::{
    AgentSubmissionHandler, ChainStorage, ChallengeConfig, DistributionConfig, ProgressStore,
    RegistryConfig, TermChallengeRpc, TermRpcConfig, WhitelistConfig,
};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "term-challenge-server")]
#[command(about = "Term Challenge HTTP Server for Platform Validators")]
struct Args {
    /// Server port
    #[arg(short, long, default_value = "8080", env = "CHALLENGE_PORT")]
    port: u16,

    /// Server host
    #[arg(long, default_value = "0.0.0.0", env = "CHALLENGE_HOST")]
    host: String,

    /// Data directory
    #[arg(short, long, default_value = "/data", env = "DATA_DIR")]
    data_dir: String,

    /// Challenge ID
    #[arg(long, default_value = "term-bench", env = "CHALLENGE_ID")]
    challenge_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("term_challenge=debug".parse().unwrap())
                .add_directive("info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!("Starting Term Challenge Server");
    info!("  Challenge ID: {}", args.challenge_id);
    info!("  Data dir: {}", args.data_dir);
    info!("  Listening on: {}:{}", args.host, args.port);

    // Create data directory
    std::fs::create_dir_all(&args.data_dir)?;

    // Initialize components
    let registry_config = RegistryConfig::default();
    let whitelist_config = WhitelistConfig::default();
    let distribution_config = DistributionConfig::default();
    let challenge_config = ChallengeConfig::default();

    let handler =
        AgentSubmissionHandler::new(registry_config, whitelist_config, distribution_config);

    let progress_store = Arc::new(ProgressStore::new());
    let chain_storage = Arc::new(ChainStorage::new());

    // Create RPC server
    let rpc_config = TermRpcConfig {
        host: args.host,
        port: args.port,
    };

    let rpc = TermChallengeRpc::new(
        rpc_config,
        handler,
        progress_store,
        chain_storage,
        challenge_config,
    );

    info!("Term Challenge Server ready");

    // Start server (blocks until shutdown)
    rpc.start().await?;

    Ok(())
}
