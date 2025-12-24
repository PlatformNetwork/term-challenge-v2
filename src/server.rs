//! Always-On Challenge Server
//!
//! This module implements the challenge container server as per the Platform architecture:
//!
//! Architecture:
//! ```text
//! Challenge Container (always-on)
//!  ├── Service Mode (continuous)
//!  │   └── Claim tasks via Data API → Process → Write results
//!  └── Weights Mode (epoch-triggered)
//!      └── GET /get_weights → Read-only, deterministic
//! ```
//!
//! Key invariants:
//! - Always running (one container per challenge)
//! - No direct Docker access (use Sandbox Runner via UDS)
//! - No DB writes during /get_weights
//! - Weights must be deterministic (no RNG, no clock, no local state)

use crate::central_client::PlatformClient;
use crate::challenge::TerminalBenchChallenge;
use crate::config::ChallengeConfig;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

// ============================================================================
// SERVER STATE
// ============================================================================

pub struct ChallengeServerState {
    pub config: RwLock<ChallengeConfig>,
    pub platform_client: PlatformClient,
    pub challenge_id: String,
}

impl ChallengeServerState {
    pub fn new(config: ChallengeConfig, platform_url: &str, challenge_id: &str) -> Self {
        Self {
            config: RwLock::new(config),
            platform_client: PlatformClient::new(platform_url),
            challenge_id: challenge_id.to_string(),
        }
    }
}

// ============================================================================
// /get_weights ENDPOINT (Critical for Epoch Weight Calculation)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GetWeightsQuery {
    pub epoch: Option<u64>,
}

/// Response format as per architecture spec
#[derive(Debug, Serialize)]
pub struct GetWeightsResponse {
    pub epoch: u64,
    pub weights: Vec<WeightEntry>,
}

#[derive(Debug, Serialize)]
pub struct WeightEntry {
    pub hotkey: String,
    pub weight: f64,
}

/// GET /get_weights - Deterministic weight calculation
///
/// STRICT RULES (from architecture spec):
/// - Method: GET
/// - Response: JSON
/// - Weights ∈ [0, 1]
/// - Read-only (NO DB writes)
/// - No RNG
/// - No clock dependence
/// - No local state dependence
///
/// Weight Calculation:
/// - Reads leaderboard snapshot from Data API
/// - Computes weights based on consensus scores
/// - Remaining weight (1.0 - sum) goes to UID 0 (burn)
pub async fn get_weights(
    State(state): State<Arc<ChallengeServerState>>,
    Query(query): Query<GetWeightsQuery>,
) -> Result<Json<GetWeightsResponse>, (StatusCode, String)> {
    // Get snapshot from platform server (Data API)
    let snapshot = state
        .platform_client
        .get_snapshot(query.epoch)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let epoch = snapshot.epoch;

    // Compute weights deterministically from leaderboard
    // Using consensus_score as the basis for weight calculation
    let mut weights = Vec::new();
    let total_score: f64 = snapshot
        .leaderboard
        .iter()
        .map(|e| e.consensus_score.max(0.0))
        .sum();

    if total_score > 0.0 {
        for entry in &snapshot.leaderboard {
            if entry.consensus_score > 0.0 {
                // Normalize to [0, 1] range
                // Note: We only distribute a portion of weight, rest goes to burn (UID 0)
                let weight = (entry.consensus_score / total_score) * 0.9; // 90% distributed, 10% burn
                weights.push(WeightEntry {
                    hotkey: entry.miner_hotkey.clone(),
                    weight: weight.clamp(0.0, 1.0),
                });
            }
        }
    }

    // Sort by hotkey for determinism
    weights.sort_by(|a, b| a.hotkey.cmp(&b.hotkey));

    info!(
        "Computed weights for epoch {}: {} miners, total weight: {:.4}",
        epoch,
        weights.len(),
        weights.iter().map(|w| w.weight).sum::<f64>()
    );

    Ok(Json(GetWeightsResponse { epoch, weights }))
}

// ============================================================================
// /health ENDPOINT
// ============================================================================

pub async fn health_check() -> &'static str {
    "OK"
}

// ============================================================================
// /evaluate ENDPOINT (Called by platform-server)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub submission_id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub source_code: String,
    pub api_key: Option<String>,
    pub api_provider: Option<String>,
    pub epoch: u64,
}

#[derive(Debug, Serialize)]
pub struct EvaluateResponse {
    pub success: bool,
    pub error: Option<String>,
    pub score: f64,
    pub tasks_passed: u32,
    pub tasks_total: u32,
    pub tasks_failed: u32,
    pub total_cost_usd: f64,
    pub execution_time_ms: i64,
    pub task_results: Option<Vec<TaskResultResponse>>,
    pub execution_log: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskResultResponse {
    pub task_id: String,
    pub passed: bool,
    pub score: f64,
    pub execution_time_ms: i64,
    pub cost_usd: f64,
    pub error: Option<String>,
}

/// POST /evaluate - Evaluate an agent submission
/// Called by platform-server when a validator needs to evaluate
pub async fn evaluate_agent(
    State(state): State<Arc<ChallengeServerState>>,
    Json(req): Json<EvaluateRequest>,
) -> Result<Json<EvaluateResponse>, (StatusCode, String)> {
    info!(
        "Evaluating agent: {} (hash: {}) from {}",
        req.name.as_deref().unwrap_or("unnamed"),
        &req.agent_hash[..16],
        req.miner_hotkey
    );

    let start = std::time::Instant::now();

    // TODO: Implement actual evaluation using TerminalBenchChallenge
    // For now, return a placeholder response
    let config = state.config.read().await;

    // Placeholder evaluation - in real implementation:
    // 1. Create agent container via Sandbox Runner (UDS)
    // 2. Run tasks
    // 3. Collect results
    // 4. Return scores

    let execution_time_ms = start.elapsed().as_millis() as i64;

    Ok(Json(EvaluateResponse {
        success: true,
        error: None,
        score: 0.5, // Placeholder
        tasks_passed: 5,
        tasks_total: 10,
        tasks_failed: 5,
        total_cost_usd: 0.0,
        execution_time_ms,
        task_results: None,
        execution_log: Some("Evaluation placeholder".to_string()),
    }))
}

// ============================================================================
// /validate ENDPOINT (Quick validation without full evaluation)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub source_code: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub errors: Vec<String>,
}

pub async fn validate_source(
    State(state): State<Arc<ChallengeServerState>>,
    Json(req): Json<ValidateRequest>,
) -> Json<ValidateResponse> {
    let config = state.config.read().await;
    let mut errors = Vec::new();

    // Basic validation
    if req.source_code.is_empty() {
        errors.push("Source code is empty".to_string());
    }

    if req.source_code.len() > 1_000_000 {
        errors.push("Source code exceeds maximum size (1MB)".to_string());
    }

    // Check for required imports/structure
    if !req.source_code.contains("def") && !req.source_code.contains("class") {
        errors.push("Source code must contain at least one function or class".to_string());
    }

    Json(ValidateResponse {
        valid: errors.is_empty(),
        errors,
    })
}

// ============================================================================
// /config ENDPOINT
// ============================================================================

pub async fn get_config(State(state): State<Arc<ChallengeServerState>>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(serde_json::json!({
        "challenge_id": state.challenge_id,
        "tasks_per_evaluation": config.evaluation.tasks_per_evaluation,
        "max_concurrent_tasks": config.evaluation.max_concurrent_tasks_per_agent,
        "max_cost_per_task_usd": config.pricing.max_cost_per_task_usd,
        "max_total_cost_usd": config.pricing.max_total_cost_usd,
        "min_stake_tao": config.min_stake_tao,
    }))
}

// ============================================================================
// SERVER STARTUP
// ============================================================================

pub async fn run_server(
    config: ChallengeConfig,
    platform_url: &str,
    challenge_id: &str,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let state = Arc::new(ChallengeServerState::new(
        config,
        platform_url,
        challenge_id,
    ));

    let app = Router::new()
        // Required endpoints per architecture spec
        .route("/health", get(health_check))
        .route("/get_weights", get(get_weights))
        // Challenge-specific endpoints
        .route("/evaluate", post(evaluate_agent))
        .route("/validate", post(validate_source))
        .route("/config", get(get_config))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║     Terminal Benchmark Challenge - Always-On Container       ║");
    info!("╠══════════════════════════════════════════════════════════════╣");
    info!("║  Challenge ID: {:44} ║", challenge_id);
    info!("║  Platform URL: {:44} ║", platform_url);
    info!("║  Listening on: {:44} ║", addr);
    info!("╠══════════════════════════════════════════════════════════════╣");
    info!("║  Endpoints:                                                  ║");
    info!("║    GET  /health      - Health check                          ║");
    info!("║    GET  /get_weights - Deterministic weights (epoch)         ║");
    info!("║    POST /evaluate    - Evaluate agent submission             ║");
    info!("║    POST /validate    - Quick source validation               ║");
    info!("║    GET  /config      - Challenge configuration               ║");
    info!("╚══════════════════════════════════════════════════════════════╝");

    axum::serve(listener, app).await?;

    Ok(())
}
