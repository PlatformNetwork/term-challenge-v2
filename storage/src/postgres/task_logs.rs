use chrono::{DateTime, Utc};
use platform_core::{ChallengeId, Hotkey};
use uuid::Uuid;

use crate::pg::PgPool;
use crate::Result;

pub type TaskLogRecord = (String, bool, f64, u64, Option<String>, Option<String>);

#[derive(Clone, Debug)]
pub struct StoredTaskLog {
    pub id: Uuid,
    pub submission_id: Uuid,
    pub challenge_id: Uuid,
    pub miner_hotkey_ss58: String,
    pub task_id: String,
    pub passed: bool,
    pub score: f64,
    pub execution_time_ms: i64,
    pub output_preview: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_task_log(
    pool: &PgPool,
    submission_id: &Uuid,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
    task_id: &str,
    passed: bool,
    score: f64,
    execution_time_ms: u64,
    output_preview: Option<&str>,
    error: Option<&str>,
) -> Result<Uuid> {
    let client = pool.get().await?;
    let id = Uuid::new_v4();
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();
    let exec_time_i64 = execution_time_ms as i64;

    client
        .execute(
            "INSERT INTO task_logs (
                id, submission_id, challenge_id, miner_hotkey, task_id,
                passed, score, execution_time_ms, output_preview, error, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())",
            &[
                &id,
                submission_id,
                &challenge_uuid,
                &hotkey_ss58,
                &task_id,
                &passed,
                &score,
                &exec_time_i64,
                &output_preview,
                &error,
            ],
        )
        .await?;

    Ok(id)
}

pub async fn insert_task_logs_batch(
    pool: &PgPool,
    submission_id: &Uuid,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
    logs: &[TaskLogRecord],
) -> Result<Vec<Uuid>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();

    let mut ids = Vec::with_capacity(logs.len());

    for (task_id, passed, score, execution_time_ms, output_preview, error) in logs {
        let id = Uuid::new_v4();
        let exec_time_i64 = *execution_time_ms as i64;
        let output_ref = output_preview.as_deref();
        let error_ref = error.as_deref();

        client
            .execute(
                "INSERT INTO task_logs (
                    id, submission_id, challenge_id, miner_hotkey, task_id,
                    passed, score, execution_time_ms, output_preview, error, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())",
                &[
                    &id,
                    submission_id,
                    &challenge_uuid,
                    &hotkey_ss58,
                    task_id,
                    passed,
                    score,
                    &exec_time_i64,
                    &output_ref,
                    &error_ref,
                ],
            )
            .await?;

        ids.push(id);
    }

    Ok(ids)
}

pub async fn get_task_logs_by_submission(
    pool: &PgPool,
    submission_id: &Uuid,
) -> Result<Vec<StoredTaskLog>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT id, submission_id, challenge_id, miner_hotkey, task_id,
                    passed, score, execution_time_ms, output_preview, error, created_at
             FROM task_logs
             WHERE submission_id = $1
             ORDER BY created_at ASC",
            &[submission_id],
        )
        .await?;

    let mut logs = Vec::with_capacity(rows.len());
    for row in &rows {
        logs.push(StoredTaskLog {
            id: row.get("id"),
            submission_id: row.get("submission_id"),
            challenge_id: row.get("challenge_id"),
            miner_hotkey_ss58: row.get("miner_hotkey"),
            task_id: row.get("task_id"),
            passed: row.get("passed"),
            score: row.get("score"),
            execution_time_ms: row.get("execution_time_ms"),
            output_preview: row.get("output_preview"),
            error: row.get("error"),
            created_at: row.get("created_at"),
        });
    }

    Ok(logs)
}

pub async fn get_task_logs_by_miner(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
    limit: i64,
) -> Result<Vec<StoredTaskLog>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();

    let rows = client
        .query(
            "SELECT id, submission_id, challenge_id, miner_hotkey, task_id,
                    passed, score, execution_time_ms, output_preview, error, created_at
             FROM task_logs
             WHERE challenge_id = $1 AND miner_hotkey = $2
             ORDER BY created_at DESC
             LIMIT $3",
            &[&challenge_uuid, &hotkey_ss58, &limit],
        )
        .await?;

    let mut logs = Vec::with_capacity(rows.len());
    for row in &rows {
        logs.push(StoredTaskLog {
            id: row.get("id"),
            submission_id: row.get("submission_id"),
            challenge_id: row.get("challenge_id"),
            miner_hotkey_ss58: row.get("miner_hotkey"),
            task_id: row.get("task_id"),
            passed: row.get("passed"),
            score: row.get("score"),
            execution_time_ms: row.get("execution_time_ms"),
            output_preview: row.get("output_preview"),
            error: row.get("error"),
            created_at: row.get("created_at"),
        });
    }

    Ok(logs)
}

pub async fn get_pass_rate_by_miner(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
) -> Result<f64> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();

    let row = client
        .query_one(
            "SELECT
                COUNT(*) FILTER (WHERE passed = true) as passed_count,
                COUNT(*) as total_count
             FROM task_logs
             WHERE challenge_id = $1 AND miner_hotkey = $2",
            &[&challenge_uuid, &hotkey_ss58],
        )
        .await?;

    let passed: i64 = row.get("passed_count");
    let total: i64 = row.get("total_count");

    if total == 0 {
        return Ok(0.0);
    }

    Ok(passed as f64 / total as f64)
}

pub async fn delete_task_logs_by_submission(pool: &PgPool, submission_id: &Uuid) -> Result<u64> {
    let client = pool.get().await?;
    let rows_affected = client
        .execute(
            "DELETE FROM task_logs WHERE submission_id = $1",
            &[submission_id],
        )
        .await?;
    Ok(rows_affected)
}
