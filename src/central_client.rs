//! Platform API Interface for Challenge Containers
//!
//! This module provides the interface between challenge containers and platform-server.
//!
//! IMPORTANT SECURITY MODEL:
//! - Challenge containers NEVER have access to validator keypairs
//! - All authentication is handled by platform-server
//! - Challenge containers receive data via HTTP from platform-server
//! - Results are sent back to platform-server which handles signing
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Platform Server                               │
//! │  (handles all auth, keypairs, WebSocket to validators)          │
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
//! │  │  Validator   │◄──►│   Platform   │◄──►│  Challenge   │      │
//! │  │  (keypair)   │ WS │   Server     │HTTP│  Container   │      │
//! │  └──────────────┘    └──────────────┘    └──────────────┘      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! The challenge container:
//! 1. Receives submissions via HTTP POST from platform-server
//! 2. Evaluates the agent
//! 3. Returns results via HTTP response
//! 4. Platform-server handles signing and broadcasting

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// ============================================================================
// TYPES FOR CHALLENGE CONTAINER <-> PLATFORM COMMUNICATION
// ============================================================================

/// Request sent by platform-server to challenge container to evaluate an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRequest {
    /// Unique submission ID
    pub submission_id: String,
    /// Hash of the agent (miner_hotkey + source)
    pub agent_hash: String,
    /// Miner's hotkey (for logging only, not for auth)
    pub miner_hotkey: String,
    /// Agent name
    pub name: Option<String>,
    /// Source code to evaluate
    pub source_code: String,
    /// Decrypted API key for LLM calls (platform decrypted it)
    pub api_key: Option<String>,
    /// API provider (openai, anthropic, etc.)
    pub api_provider: Option<String>,
    /// Current epoch
    pub epoch: u64,
    /// Challenge configuration
    pub config: ChallengeConfig,
}

/// Response from challenge container after evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateResponse {
    /// Whether evaluation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Evaluation score (0.0 - 1.0)
    pub score: f64,
    /// Number of tasks passed
    pub tasks_passed: u32,
    /// Total number of tasks
    pub tasks_total: u32,
    /// Number of tasks failed
    pub tasks_failed: u32,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Execution time in milliseconds
    pub execution_time_ms: i64,
    /// Per-task results
    pub task_results: Option<Vec<TaskResult>>,
    /// Execution log (truncated if too long)
    pub execution_log: Option<String>,
}

/// Individual task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub passed: bool,
    pub score: f64,
    pub execution_time_ms: i64,
    pub cost_usd: f64,
    pub error: Option<String>,
}

/// Challenge configuration sent by platform-server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeConfig {
    pub challenge_id: String,
    pub max_tasks: u32,
    pub timeout_seconds: u32,
    pub max_cost_usd: f64,
    pub module_whitelist: Vec<String>,
    pub model_whitelist: Vec<String>,
}

/// Network state info (read-only for challenge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkState {
    pub current_epoch: u64,
    pub current_block: u64,
    pub active_validators: u32,
}

/// Leaderboard entry (read-only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub consensus_score: f64,
    pub evaluation_count: u32,
    pub rank: u32,
}

// ============================================================================
// CHALLENGE CONTAINER ROUTES (exposed by term-challenge in server mode)
// ============================================================================

// Routes that the challenge container must expose for platform-server to call:
//
// POST /evaluate
//   - Receives: EvaluateRequest
//   - Returns: EvaluateResponse
//   - Platform-server calls this when a validator needs to evaluate an agent
//
// GET /health
//   - Returns: "OK" or health status
//   - Platform-server uses this to check container is alive
//
// GET /config
//   - Returns: Challenge-specific configuration schema
//   - Used by platform-server to know what config options are available
//
// POST /validate
//   - Receives: { "source_code": "..." }
//   - Returns: { "valid": bool, "errors": [...] }
//   - Quick validation without full evaluation

// ============================================================================
// HELPER FOR CHALLENGE CONTAINERS
// ============================================================================

/// Simple HTTP client for challenge containers to query platform-server.
/// Read-only operations only, no auth needed for public data.
pub struct PlatformClient {
    base_url: String,
    client: reqwest::Client,
}

impl PlatformClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Get current network state (public endpoint)
    pub async fn get_network_state(&self) -> Result<NetworkState> {
        let resp = self
            .client
            .get(format!("{}/api/v1/network/state", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to get network state: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }

    /// Get leaderboard (public endpoint)
    pub async fn get_leaderboard(&self, limit: usize) -> Result<Vec<LeaderboardEntry>> {
        let resp = self
            .client
            .get(format!(
                "{}/api/v1/leaderboard?limit={}",
                self.base_url, limit
            ))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to get leaderboard: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }

    /// Get challenge config (public endpoint)
    pub async fn get_config(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(format!("{}/api/v1/config", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to get config: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }

    /// Get database snapshot for deterministic weight calculation
    /// Used by /get_weights endpoint
    pub async fn get_snapshot(&self, epoch: Option<u64>) -> Result<SnapshotResponse> {
        let url = match epoch {
            Some(e) => format!("{}/api/v1/data/snapshot?epoch={}", self.base_url, e),
            None => format!("{}/api/v1/data/snapshot", self.base_url),
        };

        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to get snapshot: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }

    /// Claim a task for exclusive processing (Data API)
    pub async fn claim_task(
        &self,
        task_id: &str,
        validator_hotkey: &str,
        ttl_seconds: u64,
    ) -> Result<ClaimTaskResponse> {
        let resp = self
            .client
            .post(format!("{}/api/v1/data/tasks/claim", self.base_url))
            .json(&serde_json::json!({
                "task_id": task_id,
                "validator_hotkey": validator_hotkey,
                "signature": "placeholder", // TODO: Real signature
                "ttl_seconds": ttl_seconds,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to claim task: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }

    /// Acknowledge task completion
    pub async fn ack_task(&self, task_id: &str, validator_hotkey: &str) -> Result<bool> {
        let resp = self
            .client
            .post(format!(
                "{}/api/v1/data/tasks/{}/ack",
                self.base_url, task_id
            ))
            .json(&serde_json::json!({
                "validator_hotkey": validator_hotkey,
                "signature": "placeholder",
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to ack task: {}", resp.status()));
        }

        let result: serde_json::Value = resp.json().await?;
        Ok(result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Write evaluation result to platform server
    pub async fn write_result(&self, result: &WriteResultRequest) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/api/v1/data/results", self.base_url))
            .json(result)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to write result: {}", resp.status()));
        }

        Ok(resp.json().await?)
    }
}

/// Snapshot response from Data API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub epoch: u64,
    pub snapshot_time: i64,
    pub leaderboard: Vec<SnapshotLeaderboardEntry>,
    pub validators: Vec<SnapshotValidator>,
    pub total_stake: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotLeaderboardEntry {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub consensus_score: f64,
    pub evaluation_count: u32,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotValidator {
    pub hotkey: String,
    pub stake: u64,
    pub is_active: bool,
}

/// Claim task response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimTaskResponse {
    pub success: bool,
    pub lease: Option<TaskLease>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLease {
    pub task_id: String,
    pub validator_hotkey: String,
    pub claimed_at: i64,
    pub expires_at: i64,
}

/// Write result request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResultRequest {
    pub agent_hash: String,
    pub validator_hotkey: String,
    pub signature: String,
    pub score: f64,
    pub task_results: Option<serde_json::Value>,
    pub execution_time_ms: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_request_serialization() {
        let req = EvaluateRequest {
            submission_id: "sub-123".to_string(),
            agent_hash: "abc123".to_string(),
            miner_hotkey: "5GrwvaEF...".to_string(),
            name: Some("test-agent".to_string()),
            source_code: "print('hello')".to_string(),
            api_key: Some("sk-test".to_string()),
            api_provider: Some("openai".to_string()),
            epoch: 100,
            config: ChallengeConfig {
                challenge_id: "term-bench".to_string(),
                max_tasks: 10,
                timeout_seconds: 300,
                max_cost_usd: 1.0,
                module_whitelist: vec!["os".to_string()],
                model_whitelist: vec!["gpt-4".to_string()],
            },
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: EvaluateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.submission_id, "sub-123");
    }

    #[test]
    fn test_evaluate_response_serialization() {
        let resp = EvaluateResponse {
            success: true,
            error: None,
            score: 0.85,
            tasks_passed: 8,
            tasks_total: 10,
            tasks_failed: 2,
            total_cost_usd: 0.15,
            execution_time_ms: 5000,
            task_results: None,
            execution_log: Some("Log...".to_string()),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: EvaluateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.score, 0.85);
    }
}
