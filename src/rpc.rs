//! RPC Endpoints for Term Challenge
//!
//! Provides HTTP endpoints for:
//! - Agent submission
//! - Status queries
//! - Whitelist info
//! - Consensus signatures

use crate::{
    agent_registry::SubmissionAllowance, chain_storage::ChainStorage, config::ChallengeConfig,
    task_execution::ProgressStore, validator_distribution::ObfuscatedPackage, AgentSubmission,
    AgentSubmissionHandler, SubmissionStatus, ValidatorInfo,
};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// RPC Configuration
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub host: String,
    pub port: u16,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

/// RPC Server State
pub struct RpcState {
    pub handler: Arc<AgentSubmissionHandler>,
    pub progress_store: Arc<ProgressStore>,
    pub chain_storage: Arc<ChainStorage>,
    pub challenge_config: ChallengeConfig,
}

/// Term Challenge RPC Server
pub struct TermChallengeRpc {
    config: RpcConfig,
    state: Arc<RpcState>,
}

impl TermChallengeRpc {
    pub fn new(
        config: RpcConfig,
        handler: AgentSubmissionHandler,
        progress_store: Arc<ProgressStore>,
        chain_storage: Arc<ChainStorage>,
        challenge_config: ChallengeConfig,
    ) -> Self {
        Self {
            config,
            state: Arc::new(RpcState {
                handler: Arc::new(handler),
                progress_store,
                chain_storage,
                challenge_config,
            }),
        }
    }

    /// Create the router
    pub fn router(&self) -> Router {
        Router::new()
            // Agent submission
            .route("/submit", post(submit_agent))
            .route("/can_submit", get(can_submit))
            // Status
            .route("/status/:agent_hash", get(get_status))
            .route("/agent/:agent_hash", get(get_agent))
            .route("/agents/miner/:miner_hotkey", get(get_miner_agents))
            .route("/agents/pending", get(get_pending_agents))
            .route("/agents/active", get(get_active_agents))
            // Consensus (for top validators)
            .route("/consensus/sign", post(sign_consensus))
            .route("/consensus/source/:agent_hash", get(get_source))
            .route("/consensus/obfuscated/:agent_hash", get(get_obfuscated))
            .route("/consensus/verify", post(verify_obfuscated))
            // Real-time progress
            .route("/progress/:evaluation_id", get(get_progress))
            .route("/progress/agent/:agent_hash", get(get_agent_progress))
            .route(
                "/progress/agent/:agent_hash/latest",
                get(get_latest_progress),
            )
            .route(
                "/progress/validator/:validator_hotkey",
                get(get_validator_progress),
            )
            .route("/progress/running", get(get_running_evaluations))
            // Configuration
            .route("/config", get(get_challenge_config))
            .route("/config/whitelist/modules", get(get_module_whitelist))
            .route("/config/whitelist/models", get(get_model_whitelist))
            .route("/config/pricing", get(get_pricing_config))
            // On-chain results (consensus)
            .route("/chain/result/:agent_hash", get(get_chain_results))
            .route(
                "/chain/result/:agent_hash/:validator",
                get(get_chain_result_by_validator),
            )
            .route("/chain/consensus/:agent_hash", get(get_chain_consensus))
            .route("/chain/votes/:agent_hash", get(get_chain_votes))
            .route("/chain/leaderboard", get(get_chain_leaderboard))
            // Info
            .route("/whitelist", get(get_whitelist))
            .route("/stats", get(get_stats))
            .route("/validators", post(update_validators))
            .with_state(self.state.clone())
    }

    /// Start the RPC server
    pub async fn start(&self) -> anyhow::Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        info!("Term Challenge RPC server listening on {}", addr);

        axum::serve(listener, self.router()).await?;

        Ok(())
    }
}

// ==================== Request/Response Types ====================

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub source_code: String,
    pub miner_hotkey: String,
    pub signature: String, // hex encoded
    pub stake: u64,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    pub success: bool,
    pub agent_hash: Option<String>,
    pub status: Option<SubmissionStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CanSubmitQuery {
    pub miner_hotkey: String,
    pub stake: u64,
}

#[derive(Debug, Deserialize)]
pub struct SignConsensusRequest {
    pub agent_hash: String,
    pub validator_hotkey: String,
    pub obfuscated_hash: String,
    pub signature: String, // hex encoded
}

#[derive(Debug, Serialize)]
pub struct SignConsensusResponse {
    pub success: bool,
    pub consensus_reached: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetSourceQuery {
    pub validator_hotkey: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyObfuscatedRequest {
    pub package: ObfuscatedPackage,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_agents: usize,
    pub pending_agents: usize,
    pub active_agents: usize,
    pub rejected_agents: usize,
    pub total_miners: usize,
    pub current_epoch: u64,
}

// ==================== Handlers ====================

async fn submit_agent(
    State(state): State<Arc<RpcState>>,
    Json(req): Json<SubmitRequest>,
) -> impl IntoResponse {
    info!("Received submission from miner {}", req.miner_hotkey);

    let signature = match hex::decode(&req.signature) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SubmitResponse {
                    success: false,
                    agent_hash: None,
                    status: None,
                    error: Some(format!("Invalid signature hex: {}", e)),
                }),
            );
        }
    };

    let submission = AgentSubmission {
        source_code: req.source_code,
        miner_hotkey: req.miner_hotkey,
        signature,
        name: req.name,
        description: req.description,
        metadata: None,
    };

    match state.handler.submit(submission, req.stake).await {
        Ok(status) => (
            StatusCode::OK,
            Json(SubmitResponse {
                success: true,
                agent_hash: Some(status.agent_hash.clone()),
                status: Some(status),
                error: None,
            }),
        ),
        Err(e) => {
            warn!("Submission failed: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(SubmitResponse {
                    success: false,
                    agent_hash: None,
                    status: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}

async fn can_submit(
    State(state): State<Arc<RpcState>>,
    Query(query): Query<CanSubmitQuery>,
) -> impl IntoResponse {
    match state.handler.can_submit(&query.miner_hotkey, query.stake) {
        Ok(allowance) => (StatusCode::OK, Json(allowance)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SubmissionAllowance {
                allowed: false,
                reason: Some(e.to_string()),
                next_allowed_epoch: None,
                remaining_slots: 0.0,
            }),
        ),
    }
}

async fn get_status(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    match state.handler.get_status(&agent_hash) {
        Some(status) => (StatusCode::OK, Json(Some(status))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_agent(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    match state.handler.get_agent(&agent_hash) {
        Some(agent) => (StatusCode::OK, Json(Some(agent))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_miner_agents(
    State(state): State<Arc<RpcState>>,
    Path(miner_hotkey): Path<String>,
) -> impl IntoResponse {
    let agents = state.handler.get_miner_agents(&miner_hotkey);
    Json(agents)
}

async fn get_pending_agents(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    let agents = state.handler.get_pending_agents();
    Json(agents)
}

async fn get_active_agents(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    let agents = state.handler.get_active_agents();
    Json(agents)
}

async fn sign_consensus(
    State(state): State<Arc<RpcState>>,
    Json(req): Json<SignConsensusRequest>,
) -> impl IntoResponse {
    let signature = match hex::decode(&req.signature) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SignConsensusResponse {
                    success: false,
                    consensus_reached: false,
                    error: Some(format!("Invalid signature hex: {}", e)),
                }),
            );
        }
    };

    match state.handler.add_consensus_signature(
        &req.agent_hash,
        &req.validator_hotkey,
        &req.obfuscated_hash,
        signature,
    ) {
        Ok(consensus_reached) => (
            StatusCode::OK,
            Json(SignConsensusResponse {
                success: true,
                consensus_reached,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(SignConsensusResponse {
                success: false,
                consensus_reached: false,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn get_source(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
    Query(query): Query<GetSourceQuery>,
) -> impl IntoResponse {
    match state
        .handler
        .get_source_package(&agent_hash, &query.validator_hotkey)
    {
        Some(pkg) => (StatusCode::OK, Json(Some(pkg))),
        None => (StatusCode::FORBIDDEN, Json(None)),
    }
}

async fn get_obfuscated(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    match state.handler.get_obfuscated_package(&agent_hash) {
        Some(pkg) => (StatusCode::OK, Json(Some(pkg))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn verify_obfuscated(
    State(state): State<Arc<RpcState>>,
    Json(req): Json<VerifyObfuscatedRequest>,
) -> impl IntoResponse {
    match state.handler.verify_obfuscated_package(&req.package) {
        Ok(valid) => (StatusCode::OK, Json(VerifyResponse { valid, error: None })),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(VerifyResponse {
                valid: false,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn get_whitelist(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    Json(state.handler.get_whitelist_config().clone())
}

async fn get_stats(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    let stats = state.handler.stats();
    Json(StatsResponse {
        total_agents: stats.total_agents,
        pending_agents: stats.pending_agents,
        active_agents: stats.active_agents,
        rejected_agents: stats.rejected_agents,
        total_miners: stats.total_miners,
        current_epoch: stats.current_epoch,
    })
}

async fn update_validators(
    State(state): State<Arc<RpcState>>,
    Json(validators): Json<Vec<ValidatorInfo>>,
) -> impl IntoResponse {
    state.handler.update_validators(validators);
    StatusCode::OK
}

// ==================== Progress Handlers ====================

async fn get_progress(
    State(state): State<Arc<RpcState>>,
    Path(evaluation_id): Path<String>,
) -> impl IntoResponse {
    match state.progress_store.get(&evaluation_id) {
        Some(progress) => (StatusCode::OK, Json(Some(progress))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_agent_progress(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    let evaluations = state.progress_store.get_by_agent(&agent_hash);
    Json(evaluations)
}

async fn get_latest_progress(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    match state.progress_store.get_latest_for_agent(&agent_hash) {
        Some(progress) => (StatusCode::OK, Json(Some(progress))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_validator_progress(
    State(state): State<Arc<RpcState>>,
    Path(validator_hotkey): Path<String>,
) -> impl IntoResponse {
    let evaluations = state.progress_store.get_by_validator(&validator_hotkey);
    Json(evaluations)
}

async fn get_running_evaluations(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    let running = state.progress_store.get_running();
    Json(running)
}

// ==================== Config Handlers ====================

async fn get_challenge_config(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    Json(state.challenge_config.clone())
}

async fn get_module_whitelist(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    Json(state.challenge_config.module_whitelist.clone())
}

async fn get_model_whitelist(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    Json(state.challenge_config.model_whitelist.clone())
}

async fn get_pricing_config(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    Json(state.challenge_config.pricing.clone())
}

// ==================== Chain Storage Handlers ====================

async fn get_chain_results(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    let results = state.chain_storage.get_agent_results(&agent_hash);
    Json(results)
}

async fn get_chain_result_by_validator(
    State(state): State<Arc<RpcState>>,
    Path((agent_hash, validator)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.chain_storage.get_result(&agent_hash, &validator) {
        Some(result) => (StatusCode::OK, Json(Some(result))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_chain_consensus(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    match state.chain_storage.get_consensus(&agent_hash) {
        Some(consensus) => (StatusCode::OK, Json(Some(consensus))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

async fn get_chain_votes(
    State(state): State<Arc<RpcState>>,
    Path(agent_hash): Path<String>,
) -> impl IntoResponse {
    let votes = state.chain_storage.get_votes(&agent_hash);
    Json(votes)
}

async fn get_chain_leaderboard(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    let leaderboard = state.chain_storage.get_leaderboard();
    Json(leaderboard)
}
