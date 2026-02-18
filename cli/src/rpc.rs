use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};

use crate::app::{EvalTaskRow, LeaderboardRow};

pub struct RpcClient {
    url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: Option<u64>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub struct EpochInfo {
    pub epoch: u64,
    pub phase: String,
    pub block_height: u64,
}

#[derive(Deserialize)]
struct EpochInfoRaw {
    #[serde(default)]
    epoch: u64,
    #[serde(default)]
    phase: String,
    #[serde(default)]
    block_height: u64,
}

pub struct ChallengeInfo {
    pub id: String,
}

#[derive(Deserialize)]
struct ChallengeInfoRaw {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize)]
struct LeaderboardRowRaw {
    #[serde(default)]
    rank: u32,
    #[serde(default)]
    miner_hotkey: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    pass_rate: f64,
    #[serde(default)]
    submissions: u32,
    #[serde(default)]
    last_submission: String,
}

#[derive(Deserialize)]
struct EvalTaskRowRaw {
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    error: Option<String>,
}

impl RpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }

    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow!("RPC HTTP error: {status}"));
        }

        let rpc_response: JsonRpcResponse = response
            .json()
            .await
            .context("Failed to parse RPC response")?;

        if let Some(err) = rpc_response.error {
            return Err(anyhow!("RPC error {}: {}", err.code, err.message));
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow!("RPC response missing result"))
    }

    pub async fn fetch_leaderboard(
        &self,
        challenge_id: &str,
    ) -> anyhow::Result<Vec<LeaderboardRow>> {
        let params = serde_json::json!({
            "challenge_id": challenge_id,
            "path": "/leaderboard"
        });
        let result = self.call("challenge_call", params).await?;
        let raw: Vec<LeaderboardRowRaw> =
            serde_json::from_value(result).context("Failed to parse leaderboard data")?;
        Ok(raw
            .into_iter()
            .map(|r| LeaderboardRow {
                rank: r.rank,
                miner_hotkey: r.miner_hotkey,
                score: r.score,
                pass_rate: r.pass_rate,
                submissions: r.submissions,
                last_submission: r.last_submission,
            })
            .collect())
    }

    pub async fn fetch_epoch_info(&self) -> anyhow::Result<EpochInfo> {
        let result = self.call("epoch_current", serde_json::json!({})).await?;
        let raw: EpochInfoRaw =
            serde_json::from_value(result).context("Failed to parse epoch info")?;
        Ok(EpochInfo {
            epoch: raw.epoch,
            phase: raw.phase,
            block_height: raw.block_height,
        })
    }

    pub async fn fetch_system_health(&self) -> anyhow::Result<serde_json::Value> {
        self.call("system_health", serde_json::json!({})).await
    }

    pub async fn fetch_validator_count(&self) -> anyhow::Result<usize> {
        let result = self.call("validator_count", serde_json::json!({})).await?;
        let count = result.as_u64().unwrap_or_default() as usize;
        Ok(count)
    }

    pub async fn fetch_evaluation_progress(
        &self,
        submission_id: &str,
    ) -> anyhow::Result<Vec<EvalTaskRow>> {
        let params = serde_json::json!({
            "submission_id": submission_id
        });
        let result = self.call("evaluation_getProgress", params).await?;
        let raw: Vec<EvalTaskRowRaw> =
            serde_json::from_value(result).context("Failed to parse evaluation progress")?;
        Ok(raw
            .into_iter()
            .map(|r| EvalTaskRow {
                task_id: r.task_id,
                status: r.status,
                score: r.score,
                duration_ms: r.duration_ms,
                error: r.error,
            })
            .collect())
    }

    pub async fn fetch_challenge_list(&self) -> anyhow::Result<Vec<ChallengeInfo>> {
        let result = self.call("challenge_list", serde_json::json!({})).await?;
        let raw: Vec<ChallengeInfoRaw> =
            serde_json::from_value(result).context("Failed to parse challenge list")?;
        Ok(raw
            .into_iter()
            .map(|r| ChallengeInfo { id: r.id })
            .collect())
    }

    pub async fn fetch_agent_journey(
        &self,
        challenge_id: &str,
        hotkey: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::json!({
            "challengeId": challenge_id,
            "method": "GET",
            "path": format!("/agent/{}/journey", hotkey)
        });
        let result = self.call("challenge_call", params).await?;
        Ok(result)
    }

    pub async fn fetch_submission_history(
        &self,
        challenge_id: &str,
        hotkey: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::json!({
            "challengeId": challenge_id,
            "method": "GET",
            "path": format!("/agent/{}/logs", hotkey)
        });
        let result = self.call("challenge_call", params).await?;
        Ok(result)
    }

    pub async fn fetch_stats(&self, challenge_id: &str) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::json!({
            "challengeId": challenge_id,
            "method": "GET",
            "path": "/stats"
        });
        let result = self.call("challenge_call", params).await?;
        Ok(result)
    }

    pub async fn fetch_decay_status(
        &self,
        challenge_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::json!({
            "challengeId": challenge_id,
            "method": "GET",
            "path": "/decay"
        });
        let result = self.call("challenge_call", params).await?;
        Ok(result)
    }
}
