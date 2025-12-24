//! Terminal Benchmark Challenge - Always-On Server Mode
//!
//! This binary runs the challenge as an always-on container per the Platform architecture.
//!
//! Usage:
//!   term-server --platform-url http://chain.platform.network:8080 --challenge-id term-bench
//!
//! Environment variables:
//!   PLATFORM_URL     - URL of platform-server
//!   CHALLENGE_ID     - Challenge identifier
//!   HOST             - Listen host (default: 0.0.0.0)
//!   PORT             - Listen port (default: 8081)

use clap::Parser;
use term_challenge::config::ChallengeConfig;
use term_challenge::server;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "term-server")]
#[command(about = "Terminal Benchmark Challenge - Always-On Server")]
struct Args {
    /// Platform server URL
    #[arg(long, env = "PLATFORM_URL", default_value = "http://localhost:8080")]
    platform_url: String,

    /// Challenge ID
    #[arg(long, env = "CHALLENGE_ID", default_value = "term-bench")]
    challenge_id: String,

    /// Server host
    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,

    /// Server port
    #[arg(short, long, env = "PORT", default_value = "8081")]
    port: u16,

    /// Config file path
    #[arg(long, env = "CONFIG_PATH")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("term_challenge=debug".parse().unwrap())
                .add_directive("info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!("Starting Terminal Benchmark Challenge Server");
    info!("  Platform URL: {}", args.platform_url);
    info!("  Challenge ID: {}", args.challenge_id);

    // Load or create default config
    let config = if let Some(config_path) = &args.config {
        let content = std::fs::read_to_string(config_path)?;
        serde_json::from_str(&content)?
    } else {
        ChallengeConfig::default()
    };

    // Run the server
    server::run_server(
        config,
        &args.platform_url,
        &args.challenge_id,
        &args.host,
        args.port,
    )
    .await?;

    Ok(())
}
