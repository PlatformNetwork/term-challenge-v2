//! Term-Challenge API Endpoints
//!
//! Provides all REST endpoints for:
//! - Agent submissions (miners)
//! - Leaderboard (public)
//! - Owner endpoints (authenticated)
//! - Validator endpoints (whitelisted)

use crate::auth::{
    create_get_source_message, create_list_agents_message, create_submit_message,
    is_timestamp_valid, is_valid_ss58_hotkey, verify_signature, AuthManager,
};
use crate::pg_storage::{LeaderboardEntry, PgStorage, Submission, SubmissionInfo};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{info, warn};

// ============================================================================
// SHARED STATE
// ============================================================================

/// API state shared across all handlers
pub struct ApiState {
    pub storage: PgStorage,
    pub auth: AuthManager,
}

// ============================================================================
// SUBMISSION ENDPOINTS (Miners)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SubmitAgentRequest {
    pub source_code: String,
    pub miner_hotkey: String,
    pub signature: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitAgentResponse {
    pub success: bool,
    pub submission_id: Option<String>,
    pub agent_hash: Option<String>,
    pub error: Option<String>,
}

/// POST /api/v1/submit - Submit a new agent
///
/// Requires:
/// - Valid SS58 miner_hotkey
/// - Valid signature of "submit_agent:<sha256_of_source_code>"
pub async fn submit_agent(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SubmitAgentRequest>,
) -> Result<Json<SubmitAgentResponse>, (StatusCode, Json<SubmitAgentResponse>)> {
    // Validate miner_hotkey is a valid SS58 address
    if !is_valid_ss58_hotkey(&req.miner_hotkey) {
        warn!(
            "Invalid miner_hotkey format: {}",
            &req.miner_hotkey[..32.min(req.miner_hotkey.len())]
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(SubmitAgentResponse {
                success: false,
                submission_id: None,
                agent_hash: None,
                error: Some(format!(
                    "Invalid miner_hotkey: must be a valid SS58 address. Received: {}",
                    &req.miner_hotkey[..32.min(req.miner_hotkey.len())]
                )),
            }),
        ));
    }

    // Verify signature
    let expected_message = create_submit_message(&req.source_code);
    if !verify_signature(&req.miner_hotkey, &expected_message, &req.signature) {
        warn!(
            "Invalid signature for submission from {}",
            &req.miner_hotkey[..16.min(req.miner_hotkey.len())]
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(SubmitAgentResponse {
                success: false,
                submission_id: None,
                agent_hash: None,
                error: Some(format!(
                    "Invalid signature. Message to sign: '{}'. Use sr25519 signature.",
                    expected_message
                )),
            }),
        ));
    }

    // Compute hashes
    let source_hash = hex::encode(Sha256::digest(req.source_code.as_bytes()));
    let agent_hash = format!(
        "{}{}",
        &hex::encode(Sha256::digest(req.miner_hotkey.as_bytes()))[..16],
        &source_hash[..16]
    );

    // Get current epoch
    let epoch = state.storage.get_current_epoch().await.unwrap_or(0);

    // Create submission
    let submission_id = uuid::Uuid::new_v4().to_string();
    let submission = Submission {
        id: submission_id.clone(),
        agent_hash: agent_hash.clone(),
        miner_hotkey: req.miner_hotkey.clone(),
        source_code: req.source_code,
        source_hash,
        name: req.name,
        epoch,
        status: "pending".to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };

    // Store submission
    if let Err(e) = state.storage.create_submission(&submission).await {
        warn!("Failed to create submission: {:?}", e);
        tracing::error!(
            "Submission error details - id: {}, agent_hash: {}, miner: {}, epoch: {}, error: {:?}",
            submission.id,
            submission.agent_hash,
            submission.miner_hotkey,
            submission.epoch,
            e
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SubmitAgentResponse {
                success: false,
                submission_id: None,
                agent_hash: None,
                error: Some(format!("Failed to store submission: {}", e)),
            }),
        ));
    }

    info!(
        "Agent submitted: {} from {} (epoch {})",
        &agent_hash[..16],
        &req.miner_hotkey[..16.min(req.miner_hotkey.len())],
        epoch
    );

    Ok(Json(SubmitAgentResponse {
        success: true,
        submission_id: Some(submission_id),
        agent_hash: Some(agent_hash),
        error: None,
    }))
}

// ============================================================================
// LEADERBOARD ENDPOINTS (Public)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntryResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntryResponse {
    pub rank: i32,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub best_score: f64,
    pub evaluation_count: i32,
}

/// GET /api/v1/leaderboard - Get public leaderboard
///
/// No authentication required. Does NOT include source code.
pub async fn get_leaderboard(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<LeaderboardResponse>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(100).min(1000);

    let entries = state
        .storage
        .get_leaderboard(limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response_entries: Vec<LeaderboardEntryResponse> = entries
        .into_iter()
        .map(|e| LeaderboardEntryResponse {
            rank: e.rank.unwrap_or(0),
            agent_hash: e.agent_hash,
            miner_hotkey: e.miner_hotkey,
            name: e.name,
            best_score: e.best_score,
            evaluation_count: e.evaluation_count,
        })
        .collect();

    let total = response_entries.len();

    Ok(Json(LeaderboardResponse {
        entries: response_entries,
        total,
    }))
}

/// GET /api/v1/leaderboard/:agent_hash - Get agent details
///
/// No authentication required. Does NOT include source code.
pub async fn get_agent_details(
    State(state): State<Arc<ApiState>>,
    Path(agent_hash): Path<String>,
) -> Result<Json<LeaderboardEntryResponse>, (StatusCode, String)> {
    let entry = state
        .storage
        .get_leaderboard_entry(&agent_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Agent not found".to_string()))?;

    Ok(Json(LeaderboardEntryResponse {
        rank: entry.rank.unwrap_or(0),
        agent_hash: entry.agent_hash,
        miner_hotkey: entry.miner_hotkey,
        name: entry.name,
        best_score: entry.best_score,
        evaluation_count: entry.evaluation_count,
    }))
}

// ============================================================================
// OWNER ENDPOINTS (Authenticated miners - their own data only)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AuthenticatedRequest {
    pub miner_hotkey: String,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct MyAgentsResponse {
    pub agents: Vec<SubmissionInfo>,
}

/// POST /api/v1/my/agents - List owner's agents
///
/// Requires authentication. Returns only the requesting miner's agents.
/// Does NOT include source code in listings.
pub async fn list_my_agents(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<AuthenticatedRequest>,
) -> Result<Json<MyAgentsResponse>, (StatusCode, String)> {
    // Validate hotkey
    if !is_valid_ss58_hotkey(&req.miner_hotkey) {
        return Err((StatusCode::BAD_REQUEST, "Invalid hotkey format".to_string()));
    }

    // Validate timestamp
    if !is_timestamp_valid(req.timestamp) {
        return Err((StatusCode::BAD_REQUEST, "Timestamp expired".to_string()));
    }

    // Verify signature
    let message = create_list_agents_message(req.timestamp);
    if !verify_signature(&req.miner_hotkey, &message, &req.signature) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid signature".to_string()));
    }

    // Get miner's submissions
    let agents = state
        .storage
        .get_miner_submissions(&req.miner_hotkey)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(MyAgentsResponse { agents }))
}

#[derive(Debug, Deserialize)]
pub struct GetSourceRequest {
    pub miner_hotkey: String,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct SourceCodeResponse {
    pub agent_hash: String,
    pub source_code: String,
    pub name: Option<String>,
}

/// POST /api/v1/my/agents/:agent_hash/source - Get source code of own agent
///
/// Requires authentication. Only returns source code if the requester owns the agent.
pub async fn get_my_agent_source(
    State(state): State<Arc<ApiState>>,
    Path(agent_hash): Path<String>,
    Json(req): Json<GetSourceRequest>,
) -> Result<Json<SourceCodeResponse>, (StatusCode, String)> {
    // Validate hotkey
    if !is_valid_ss58_hotkey(&req.miner_hotkey) {
        return Err((StatusCode::BAD_REQUEST, "Invalid hotkey format".to_string()));
    }

    // Validate timestamp
    if !is_timestamp_valid(req.timestamp) {
        return Err((StatusCode::BAD_REQUEST, "Timestamp expired".to_string()));
    }

    // Verify signature
    let message = create_get_source_message(&agent_hash, req.timestamp);
    if !verify_signature(&req.miner_hotkey, &message, &req.signature) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid signature".to_string()));
    }

    // Get submission
    let submission = state
        .storage
        .get_submission(&agent_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Agent not found".to_string()))?;

    // Verify ownership
    if submission.miner_hotkey != req.miner_hotkey {
        warn!(
            "Unauthorized source access attempt: {} tried to access {}",
            &req.miner_hotkey[..16.min(req.miner_hotkey.len())],
            &agent_hash[..16]
        );
        return Err((
            StatusCode::FORBIDDEN,
            "You do not own this agent".to_string(),
        ));
    }

    Ok(Json(SourceCodeResponse {
        agent_hash: submission.agent_hash,
        source_code: submission.source_code,
        name: submission.name,
    }))
}

// ============================================================================
// VALIDATOR ENDPOINTS (Whitelisted validators only)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ClaimJobRequest {
    pub validator_hotkey: String,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct ClaimJobResponse {
    pub success: bool,
    pub job: Option<JobInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobInfo {
    pub job_id: String,
    pub submission_id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub source_code: String,
}

/// POST /api/v1/validator/claim_job - Claim a pending evaluation job
///
/// Requires validator to be in whitelist.
/// Returns source code ONLY to whitelisted validators.
pub async fn claim_job(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ClaimJobRequest>,
) -> Result<Json<ClaimJobResponse>, (StatusCode, Json<ClaimJobResponse>)> {
    // Validate hotkey
    if !is_valid_ss58_hotkey(&req.validator_hotkey) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ClaimJobResponse {
                success: false,
                job: None,
                error: Some("Invalid hotkey format".to_string()),
            }),
        ));
    }

    // Validate timestamp
    if !is_timestamp_valid(req.timestamp) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ClaimJobResponse {
                success: false,
                job: None,
                error: Some("Timestamp expired".to_string()),
            }),
        ));
    }

    // Verify signature
    let message = format!("claim_job:{}", req.timestamp);
    if !verify_signature(&req.validator_hotkey, &message, &req.signature) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ClaimJobResponse {
                success: false,
                job: None,
                error: Some("Invalid signature".to_string()),
            }),
        ));
    }

    // Check if validator is whitelisted
    if !state
        .auth
        .is_whitelisted_validator(&req.validator_hotkey)
        .await
    {
        warn!(
            "Unauthorized validator claim attempt: {}",
            &req.validator_hotkey[..16.min(req.validator_hotkey.len())]
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(ClaimJobResponse {
                success: false,
                job: None,
                error: Some("Validator not in whitelist".to_string()),
            }),
        ));
    }

    // Claim evaluation
    let result = state
        .storage
        .claim_evaluation(&req.validator_hotkey)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClaimJobResponse {
                    success: false,
                    job: None,
                    error: Some(e.to_string()),
                }),
            )
        })?;

    match result {
        Some((eval, source_code)) => {
            info!(
                "Validator {} claimed job {} for agent {}",
                &req.validator_hotkey[..16.min(req.validator_hotkey.len())],
                &eval.id[..8],
                &eval.agent_hash[..16]
            );
            Ok(Json(ClaimJobResponse {
                success: true,
                job: Some(JobInfo {
                    job_id: eval.id,
                    submission_id: eval.submission_id,
                    agent_hash: eval.agent_hash,
                    miner_hotkey: eval.miner_hotkey,
                    source_code,
                }),
                error: None,
            }))
        }
        None => Ok(Json(ClaimJobResponse {
            success: true,
            job: None,
            error: Some("No pending jobs available".to_string()),
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct CompleteJobRequest {
    pub validator_hotkey: String,
    pub signature: String,
    pub timestamp: i64,
    pub job_id: String,
    pub score: f64,
    pub tasks_passed: i32,
    pub tasks_total: i32,
    pub success: bool,
}

#[derive(Debug, Serialize)]
pub struct CompleteJobResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// POST /api/v1/validator/complete_job - Complete an evaluation job
///
/// Requires validator to be in whitelist.
pub async fn complete_job(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CompleteJobRequest>,
) -> Result<Json<CompleteJobResponse>, (StatusCode, Json<CompleteJobResponse>)> {
    // Validate hotkey
    if !is_valid_ss58_hotkey(&req.validator_hotkey) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CompleteJobResponse {
                success: false,
                error: Some("Invalid hotkey format".to_string()),
            }),
        ));
    }

    // Check if validator is whitelisted
    if !state
        .auth
        .is_whitelisted_validator(&req.validator_hotkey)
        .await
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(CompleteJobResponse {
                success: false,
                error: Some("Validator not in whitelist".to_string()),
            }),
        ));
    }

    // Complete the job
    state
        .storage
        .complete_evaluation(&req.job_id, req.success)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CompleteJobResponse {
                    success: false,
                    error: Some(e.to_string()),
                }),
            )
        })?;

    info!(
        "Validator {} completed job {} (success: {}, score: {:.2})",
        &req.validator_hotkey[..16.min(req.validator_hotkey.len())],
        &req.job_id[..8],
        req.success,
        req.score
    );

    Ok(Json(CompleteJobResponse {
        success: true,
        error: None,
    }))
}

// ============================================================================
// STATUS ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub epoch: i64,
    pub pending_jobs: i64,
}

/// GET /api/v1/status - Get challenge status
pub async fn get_status(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let epoch = state
        .storage
        .get_current_epoch()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pending = state
        .storage
        .get_pending_evaluations(1000)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(StatusResponse {
        status: "running".to_string(),
        epoch,
        pending_jobs: pending.len() as i64,
    }))
}
