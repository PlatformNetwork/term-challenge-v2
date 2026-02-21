//! RPC request and response types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Generic RPC response wrapper
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> RpcResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
            timestamp: Utc::now(),
        }
    }
}

/// Health check response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
}

/// Subnet status response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub netuid: u16,
    pub name: String,
    pub version: String,
    pub block_height: u64,
    pub epoch: u64,
    pub validators_count: usize,
    pub challenges_count: usize,
    pub pending_jobs: usize,
    pub is_paused: bool,
}

/// Validator info for RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorResponse {
    pub hotkey: String,
    pub stake: u64,
    pub stake_tao: f64,
    pub is_active: bool,
    pub last_seen: DateTime<Utc>,
    pub peer_id: Option<String>,
    /// X25519 public key for API key encryption (hex, 32 bytes)
    /// Derived from validator's sr25519 seed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x25519_pubkey: Option<String>,
}

/// Challenge info for RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub code_hash: String,
    pub is_active: bool,
    pub emission_weight: f64,
    pub timeout_secs: u64,
}

/// Register validator request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub hotkey: String,
    pub signature: String,
    pub message: String,
    pub peer_id: Option<String>,
}

/// Register response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub accepted: bool,
    pub uid: Option<u16>,
    pub reason: Option<String>,
}

/// Heartbeat request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub hotkey: String,
    pub signature: String,
    pub block_height: u64,
    pub peer_id: Option<String>,
}

/// Heartbeat response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub accepted: bool,
    pub current_block: u64,
    pub current_epoch: u64,
    pub next_sync_block: Option<u64>,
}

/// Job info for RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobResponse {
    pub id: String,
    pub challenge_id: String,
    pub agent_hash: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub assigned_validator: Option<String>,
}

/// Job result submission
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobResultRequest {
    pub job_id: String,
    pub hotkey: String,
    pub signature: String,
    pub score: f64,
    pub metadata: Option<serde_json::Value>,
}

/// Job result response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobResultResponse {
    pub accepted: bool,
    pub job_id: String,
}

/// Weight assignment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightEntry {
    pub hotkey: String,
    pub weight: f64,
}

/// Current weights response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightsResponse {
    pub epoch: u64,
    pub challenge_id: String,
    pub weights: Vec<WeightEntry>,
    pub finalized: bool,
}

/// Weight commit request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightCommitRequest {
    pub hotkey: String,
    pub signature: String,
    pub challenge_id: String,
    pub commitment_hash: String,
    pub epoch: u64,
}

/// Weight reveal request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightRevealRequest {
    pub hotkey: String,
    pub signature: String,
    pub challenge_id: String,
    pub weights: Vec<WeightEntry>,
    pub salt: String,
    pub epoch: u64,
}

/// Epoch info response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochResponse {
    pub current_epoch: u64,
    pub current_block: u64,
    pub blocks_per_epoch: u64,
    pub phase: String,
    pub phase_progress: f64,
    pub blocks_until_next_phase: u64,
}

/// Sync data response (for new validators)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncResponse {
    pub block_height: u64,
    pub epoch: u64,
    pub state_hash: String,
    pub validators: Vec<ValidatorResponse>,
    pub challenges: Vec<ChallengeResponse>,
}

/// Pagination params
#[derive(Clone, Debug, Deserialize)]
pub struct PaginationParams {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            offset: Some(0),
            limit: Some(100),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::default();
        assert_eq!(params.offset, Some(0));
        assert_eq!(params.limit, Some(100));
    }

    #[test]
    fn test_status_response_serde() {
        let status = StatusResponse {
            netuid: 100,
            name: "Mini-Chain".to_string(),
            version: "1.0".to_string(),
            block_height: 100,
            epoch: 5,
            validators_count: 10,
            challenges_count: 3,
            pending_jobs: 50,
            is_paused: false,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: StatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.block_height, 100);
        assert_eq!(parsed.validators_count, 10);
    }

    #[test]
    fn test_challenge_response_serde() {
        let challenge = ChallengeResponse {
            id: "test-id".to_string(),
            name: "Test Challenge".to_string(),
            description: "Desc".to_string(),
            code_hash: "abc123".to_string(),
            is_active: true,
            emission_weight: 0.5,
            timeout_secs: 300,
        };
        let json = serde_json::to_string(&challenge).unwrap();
        let parsed: ChallengeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Test Challenge");
    }

    #[test]
    fn test_validator_response_serde() {
        let validator = ValidatorResponse {
            hotkey: "abc123".to_string(),
            stake: 1_000_000_000_000,
            stake_tao: 1000.0,
            is_active: true,
            last_seen: chrono::Utc::now(),
            peer_id: Some("peer1".to_string()),
            x25519_pubkey: None,
        };
        let json = serde_json::to_string(&validator).unwrap();
        let parsed: ValidatorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.stake_tao, 1000.0);
        assert!(parsed.is_active);
    }

    #[test]
    fn test_heartbeat_request_serde() {
        let req = HeartbeatRequest {
            hotkey: "hotkey1".to_string(),
            signature: "sig".to_string(),
            block_height: 500,
            peer_id: Some("peer".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: HeartbeatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.block_height, 500);
    }

    #[test]
    fn test_heartbeat_response() {
        let resp = HeartbeatResponse {
            accepted: true,
            current_block: 100,
            current_epoch: 5,
            next_sync_block: Some(110),
        };
        assert!(resp.accepted);
        assert_eq!(resp.current_epoch, 5);
    }

    #[test]
    fn test_job_response_serde() {
        let job = JobResponse {
            id: "job-1".to_string(),
            challenge_id: "ch-1".to_string(),
            agent_hash: "agent".to_string(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now(),
            assigned_validator: None,
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("pending"));
    }

    #[test]
    fn test_weight_entry() {
        let entry = WeightEntry {
            hotkey: "hk1".to_string(),
            weight: 0.75,
        };
        assert_eq!(entry.weight, 0.75);
    }

    #[test]
    fn test_weights_response() {
        let resp = WeightsResponse {
            epoch: 10,
            challenge_id: "ch1".to_string(),
            weights: vec![
                WeightEntry {
                    hotkey: "h1".to_string(),
                    weight: 0.5,
                },
                WeightEntry {
                    hotkey: "h2".to_string(),
                    weight: 0.5,
                },
            ],
            finalized: true,
        };
        assert_eq!(resp.weights.len(), 2);
        assert!(resp.finalized);
    }

    #[test]
    fn test_epoch_response() {
        let resp = EpochResponse {
            current_epoch: 5,
            current_block: 250,
            blocks_per_epoch: 100,
            phase: "evaluation".to_string(),
            phase_progress: 0.5,
            blocks_until_next_phase: 25,
        };
        assert_eq!(resp.phase, "evaluation");
    }

    #[test]
    fn test_sync_response() {
        let resp = SyncResponse {
            block_height: 1000,
            epoch: 10,
            state_hash: "hash123".to_string(),
            validators: vec![],
            challenges: vec![],
        };
        assert_eq!(resp.block_height, 1000);
        assert!(resp.validators.is_empty());
    }

    #[test]
    fn test_rpc_response_ok() {
        let resp: RpcResponse<String> = RpcResponse::ok("test data".to_string());
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.data.unwrap(), "test data");
    }

    #[test]
    fn test_rpc_response_error() {
        let resp: RpcResponse<String> = RpcResponse::error("error message");
        assert!(resp.data.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap(), "error message");
    }

    #[test]
    fn test_health_response_serde() {
        let health = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0".to_string(),
            uptime_secs: 100,
        };
        let json = serde_json::to_string(&health).unwrap();
        let parsed: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, "healthy");
    }

    #[test]
    fn test_register_request_serde() {
        let req = RegisterRequest {
            hotkey: "hotkey123".to_string(),
            signature: "sig".to_string(),
            message: "msg".to_string(),
            peer_id: Some("peer123".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RegisterRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hotkey, "hotkey123");
        assert!(parsed.peer_id.is_some());
    }

    #[test]
    fn test_register_response_accepted() {
        let resp = RegisterResponse {
            accepted: true,
            uid: Some(5),
            reason: None,
        };
        assert!(resp.accepted);
        assert_eq!(resp.uid, Some(5));
    }

    #[test]
    fn test_register_response_rejected() {
        let resp = RegisterResponse {
            accepted: false,
            uid: None,
            reason: Some("Insufficient stake".to_string()),
        };
        assert!(!resp.accepted);
        assert!(resp.reason.is_some());
    }

    #[test]
    fn test_job_result_request_serde() {
        let req = JobResultRequest {
            job_id: "job-1".to_string(),
            hotkey: "hk".to_string(),
            signature: "sig".to_string(),
            score: 0.85,
            metadata: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JobResultRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.score, 0.85);
        assert!(parsed.metadata.is_some());
    }

    #[test]
    fn test_job_result_response() {
        let resp = JobResultResponse {
            accepted: true,
            job_id: "job-123".to_string(),
        };
        assert!(resp.accepted);
        assert_eq!(resp.job_id, "job-123");
    }

    #[test]
    fn test_weight_commit_request_serde() {
        let req = WeightCommitRequest {
            hotkey: "hk".to_string(),
            signature: "sig".to_string(),
            challenge_id: "ch1".to_string(),
            commitment_hash: "hash".to_string(),
            epoch: 10,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WeightCommitRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.epoch, 10);
        assert_eq!(parsed.challenge_id, "ch1");
    }

    #[test]
    fn test_weight_reveal_request_serde() {
        let req = WeightRevealRequest {
            hotkey: "hk".to_string(),
            signature: "sig".to_string(),
            challenge_id: "ch1".to_string(),
            weights: vec![
                WeightEntry {
                    hotkey: "h1".to_string(),
                    weight: 0.6,
                },
                WeightEntry {
                    hotkey: "h2".to_string(),
                    weight: 0.4,
                },
            ],
            salt: "salt123".to_string(),
            epoch: 10,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WeightRevealRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.weights.len(), 2);
        assert_eq!(parsed.salt, "salt123");
    }

    #[test]
    fn test_pagination_params_custom() {
        let params = PaginationParams {
            offset: Some(50),
            limit: Some(200),
        };
        assert_eq!(params.offset, Some(50));
        assert_eq!(params.limit, Some(200));
    }
}
