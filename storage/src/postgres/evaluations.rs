use std::collections::HashMap;

use platform_challenge_sdk::types::EvaluationResult;
use platform_core::{ChallengeId, Hotkey};
use uuid::Uuid;

use crate::pg::PgPool;
use crate::{Result, StorageError};

pub async fn insert_evaluation(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    validator: &Hotkey,
    result: &EvaluationResult,
) -> Result<()> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let validator_ss58 = validator.to_ss58();
    let metrics_json = serde_json::to_value(&result.metrics)?;

    client
        .execute(
            "INSERT INTO evaluations (
                job_id, challenge_id, validator_hotkey, agent_hash,
                score, metrics, logs, execution_time_ms, evaluated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (job_id) DO UPDATE SET
                score = EXCLUDED.score,
                metrics = EXCLUDED.metrics,
                logs = EXCLUDED.logs,
                execution_time_ms = EXCLUDED.execution_time_ms,
                evaluated_at = EXCLUDED.evaluated_at",
            &[
                &result.job_id,
                &challenge_uuid,
                &validator_ss58,
                &result.agent_hash,
                &result.score,
                &metrics_json,
                &result.logs,
                &(result.execution_time_ms as i64),
                &result.timestamp,
            ],
        )
        .await?;

    Ok(())
}

pub async fn get_evaluation(pool: &PgPool, job_id: &Uuid) -> Result<EvaluationResult> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT job_id, agent_hash, score, metrics, logs, execution_time_ms, evaluated_at
             FROM evaluations WHERE job_id = $1",
            &[job_id],
        )
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("evaluation {}", job_id)))?;

    let metrics_json: serde_json::Value = row.get("metrics");
    let metrics: HashMap<String, f64> = serde_json::from_value(metrics_json)?;
    let execution_time_ms: i64 = row.get("execution_time_ms");

    Ok(EvaluationResult {
        job_id: row.get("job_id"),
        agent_hash: row.get("agent_hash"),
        score: row.get("score"),
        metrics,
        logs: row.get("logs"),
        execution_time_ms: execution_time_ms as u64,
        timestamp: row.get("evaluated_at"),
    })
}

pub async fn list_evaluations_by_challenge(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    limit: i64,
    offset: i64,
) -> Result<Vec<EvaluationResult>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let rows = client
        .query(
            "SELECT job_id, agent_hash, score, metrics, logs, execution_time_ms, evaluated_at
             FROM evaluations
             WHERE challenge_id = $1
             ORDER BY evaluated_at DESC
             LIMIT $2 OFFSET $3",
            &[&challenge_uuid, &limit, &offset],
        )
        .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let metrics_json: serde_json::Value = row.get("metrics");
        let metrics: HashMap<String, f64> = serde_json::from_value(metrics_json)?;
        let execution_time_ms: i64 = row.get("execution_time_ms");

        results.push(EvaluationResult {
            job_id: row.get("job_id"),
            agent_hash: row.get("agent_hash"),
            score: row.get("score"),
            metrics,
            logs: row.get("logs"),
            execution_time_ms: execution_time_ms as u64,
            timestamp: row.get("evaluated_at"),
        });
    }

    Ok(results)
}

pub async fn list_evaluations_by_agent(
    pool: &PgPool,
    agent_hash: &str,
) -> Result<Vec<EvaluationResult>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT job_id, agent_hash, score, metrics, logs, execution_time_ms, evaluated_at
             FROM evaluations
             WHERE agent_hash = $1
             ORDER BY evaluated_at DESC",
            &[&agent_hash],
        )
        .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let metrics_json: serde_json::Value = row.get("metrics");
        let metrics: HashMap<String, f64> = serde_json::from_value(metrics_json)?;
        let execution_time_ms: i64 = row.get("execution_time_ms");

        results.push(EvaluationResult {
            job_id: row.get("job_id"),
            agent_hash: row.get("agent_hash"),
            score: row.get("score"),
            metrics,
            logs: row.get("logs"),
            execution_time_ms: execution_time_ms as u64,
            timestamp: row.get("evaluated_at"),
        });
    }

    Ok(results)
}

pub async fn get_latest_evaluation_for_agent(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    agent_hash: &str,
) -> Result<Option<EvaluationResult>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let row = client
        .query_opt(
            "SELECT job_id, agent_hash, score, metrics, logs, execution_time_ms, evaluated_at
             FROM evaluations
             WHERE challenge_id = $1 AND agent_hash = $2
             ORDER BY evaluated_at DESC
             LIMIT 1",
            &[&challenge_uuid, &agent_hash],
        )
        .await?;

    match row {
        Some(row) => {
            let metrics_json: serde_json::Value = row.get("metrics");
            let metrics: HashMap<String, f64> = serde_json::from_value(metrics_json)?;
            let execution_time_ms: i64 = row.get("execution_time_ms");

            Ok(Some(EvaluationResult {
                job_id: row.get("job_id"),
                agent_hash: row.get("agent_hash"),
                score: row.get("score"),
                metrics,
                logs: row.get("logs"),
                execution_time_ms: execution_time_ms as u64,
                timestamp: row.get("evaluated_at"),
            }))
        }
        None => Ok(None),
    }
}

pub async fn delete_evaluation(pool: &PgPool, job_id: &Uuid) -> Result<bool> {
    let client = pool.get().await?;
    let rows_affected = client
        .execute("DELETE FROM evaluations WHERE job_id = $1", &[job_id])
        .await?;
    Ok(rows_affected > 0)
}

pub async fn count_evaluations_by_challenge(
    pool: &PgPool,
    challenge_id: &ChallengeId,
) -> Result<i64> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let row = client
        .query_one(
            "SELECT COUNT(*) as count FROM evaluations WHERE challenge_id = $1",
            &[&challenge_uuid],
        )
        .await?;

    Ok(row.get("count"))
}
