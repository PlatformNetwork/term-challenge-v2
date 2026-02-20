use chrono::{DateTime, Utc};
use platform_core::{ChallengeId, Hotkey};
use uuid::Uuid;

use crate::pg::PgPool;
use crate::{Result, StorageError};

#[derive(Clone, Debug)]
pub struct StoredSubmission {
    pub id: Uuid,
    pub challenge_id: Uuid,
    pub miner_hotkey_ss58: String,
    pub agent_hash: String,
    pub epoch: i64,
    pub score: Option<f64>,
    pub status: String,
    pub submitted_at: DateTime<Utc>,
    pub evaluated_at: Option<DateTime<Utc>>,
}

pub async fn insert_submission(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
    agent_hash: &str,
    epoch: u64,
) -> Result<Uuid> {
    let client = pool.get().await?;
    let id = Uuid::new_v4();
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();
    let epoch_i64 = epoch as i64;

    client
        .execute(
            "INSERT INTO submissions (id, challenge_id, miner_hotkey, agent_hash, epoch, status, submitted_at)
             VALUES ($1, $2, $3, $4, $5, 'pending', NOW())",
            &[&id, &challenge_uuid, &hotkey_ss58, &agent_hash, &epoch_i64],
        )
        .await?;

    Ok(id)
}

pub async fn get_submission(pool: &PgPool, id: &Uuid) -> Result<StoredSubmission> {
    let client = pool.get().await?;

    let row = client
        .query_opt(
            "SELECT id, challenge_id, miner_hotkey, agent_hash, epoch, score, status,
                    submitted_at, evaluated_at
             FROM submissions WHERE id = $1",
            &[id],
        )
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("submission {}", id)))?;

    Ok(StoredSubmission {
        id: row.get("id"),
        challenge_id: row.get("challenge_id"),
        miner_hotkey_ss58: row.get("miner_hotkey"),
        agent_hash: row.get("agent_hash"),
        epoch: row.get("epoch"),
        score: row.get("score"),
        status: row.get("status"),
        submitted_at: row.get("submitted_at"),
        evaluated_at: row.get("evaluated_at"),
    })
}

pub async fn update_submission_score(
    pool: &PgPool,
    id: &Uuid,
    score: f64,
    status: &str,
) -> Result<()> {
    let client = pool.get().await?;

    client
        .execute(
            "UPDATE submissions SET score = $2, status = $3, evaluated_at = NOW()
             WHERE id = $1",
            &[id, &score, &status],
        )
        .await?;

    Ok(())
}

pub async fn update_submission_status(pool: &PgPool, id: &Uuid, status: &str) -> Result<()> {
    let client = pool.get().await?;

    client
        .execute(
            "UPDATE submissions SET status = $2 WHERE id = $1",
            &[id, &status],
        )
        .await?;

    Ok(())
}

pub async fn list_submissions_by_challenge(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    limit: i64,
    offset: i64,
) -> Result<Vec<StoredSubmission>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let rows = client
        .query(
            "SELECT id, challenge_id, miner_hotkey, agent_hash, epoch, score, status,
                    submitted_at, evaluated_at
             FROM submissions
             WHERE challenge_id = $1
             ORDER BY submitted_at DESC
             LIMIT $2 OFFSET $3",
            &[&challenge_uuid, &limit, &offset],
        )
        .await?;

    let mut submissions = Vec::with_capacity(rows.len());
    for row in &rows {
        submissions.push(StoredSubmission {
            id: row.get("id"),
            challenge_id: row.get("challenge_id"),
            miner_hotkey_ss58: row.get("miner_hotkey"),
            agent_hash: row.get("agent_hash"),
            epoch: row.get("epoch"),
            score: row.get("score"),
            status: row.get("status"),
            submitted_at: row.get("submitted_at"),
            evaluated_at: row.get("evaluated_at"),
        });
    }

    Ok(submissions)
}

pub async fn list_submissions_by_miner(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
    limit: i64,
) -> Result<Vec<StoredSubmission>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();

    let rows = client
        .query(
            "SELECT id, challenge_id, miner_hotkey, agent_hash, epoch, score, status,
                    submitted_at, evaluated_at
             FROM submissions
             WHERE challenge_id = $1 AND miner_hotkey = $2
             ORDER BY submitted_at DESC
             LIMIT $3",
            &[&challenge_uuid, &hotkey_ss58, &limit],
        )
        .await?;

    let mut submissions = Vec::with_capacity(rows.len());
    for row in &rows {
        submissions.push(StoredSubmission {
            id: row.get("id"),
            challenge_id: row.get("challenge_id"),
            miner_hotkey_ss58: row.get("miner_hotkey"),
            agent_hash: row.get("agent_hash"),
            epoch: row.get("epoch"),
            score: row.get("score"),
            status: row.get("status"),
            submitted_at: row.get("submitted_at"),
            evaluated_at: row.get("evaluated_at"),
        });
    }

    Ok(submissions)
}

pub async fn get_last_submission_epoch(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    miner_hotkey: &Hotkey,
) -> Result<Option<i64>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = miner_hotkey.to_ss58();

    let row = client
        .query_opt(
            "SELECT MAX(epoch) as last_epoch FROM submissions
             WHERE challenge_id = $1 AND miner_hotkey = $2",
            &[&challenge_uuid, &hotkey_ss58],
        )
        .await?;

    match row {
        Some(row) => Ok(row.get("last_epoch")),
        None => Ok(None),
    }
}

pub async fn count_submissions_by_challenge(
    pool: &PgPool,
    challenge_id: &ChallengeId,
) -> Result<i64> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let row = client
        .query_one(
            "SELECT COUNT(*) as count FROM submissions WHERE challenge_id = $1",
            &[&challenge_uuid],
        )
        .await?;

    Ok(row.get("count"))
}

pub async fn delete_submission(pool: &PgPool, id: &Uuid) -> Result<bool> {
    let client = pool.get().await?;
    let rows_affected = client
        .execute("DELETE FROM submissions WHERE id = $1", &[id])
        .await?;
    Ok(rows_affected > 0)
}
