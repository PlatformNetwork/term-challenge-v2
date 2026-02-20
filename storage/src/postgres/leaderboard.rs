use platform_challenge_sdk::types::WeightAssignment;
use platform_core::{ChallengeId, Hotkey};
use uuid::Uuid;

use crate::pg::PgPool;
use crate::Result;

#[derive(Clone, Debug)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub hotkey_ss58: String,
    pub score: f64,
    pub weight: f64,
    pub submissions_count: i64,
    pub last_evaluated_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn upsert_leaderboard_entry(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    hotkey: &Hotkey,
    score: f64,
    weight: f64,
) -> Result<()> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = hotkey.to_ss58();

    client
        .execute(
            "INSERT INTO leaderboard (challenge_id, hotkey, score, weight, updated_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (challenge_id, hotkey) DO UPDATE SET
                score = EXCLUDED.score,
                weight = EXCLUDED.weight,
                updated_at = NOW()",
            &[&challenge_uuid, &hotkey_ss58, &score, &weight],
        )
        .await?;

    Ok(())
}

pub async fn get_leaderboard(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    limit: i64,
    offset: i64,
) -> Result<Vec<LeaderboardEntry>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let rows = client
        .query(
            "SELECT hotkey, score, weight,
                    COALESCE(submissions_count, 0) as submissions_count,
                    updated_at,
                    ROW_NUMBER() OVER (ORDER BY score DESC) as rank
             FROM leaderboard
             WHERE challenge_id = $1
             ORDER BY score DESC
             LIMIT $2 OFFSET $3",
            &[&challenge_uuid, &limit, &offset],
        )
        .await?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        let rank: i64 = row.get("rank");
        entries.push(LeaderboardEntry {
            rank: rank as i32,
            hotkey_ss58: row.get("hotkey"),
            score: row.get("score"),
            weight: row.get("weight"),
            submissions_count: row.get("submissions_count"),
            last_evaluated_at: row.get("updated_at"),
        });
    }

    Ok(entries)
}

pub async fn get_weight_assignments(
    pool: &PgPool,
    challenge_id: &ChallengeId,
) -> Result<Vec<WeightAssignment>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    let rows = client
        .query(
            "SELECT hotkey, weight FROM leaderboard
             WHERE challenge_id = $1 AND weight > 0.0
             ORDER BY weight DESC",
            &[&challenge_uuid],
        )
        .await?;

    let mut assignments = Vec::with_capacity(rows.len());
    for row in &rows {
        let hotkey: String = row.get("hotkey");
        let weight: f64 = row.get("weight");
        assignments.push(WeightAssignment::new(hotkey, weight));
    }

    Ok(assignments)
}

pub async fn set_weight_assignments(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    assignments: &[WeightAssignment],
) -> Result<()> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;

    for assignment in assignments {
        client
            .execute(
                "INSERT INTO leaderboard (challenge_id, hotkey, score, weight, updated_at)
                 VALUES ($1, $2, 0.0, $3, NOW())
                 ON CONFLICT (challenge_id, hotkey) DO UPDATE SET
                    weight = EXCLUDED.weight,
                    updated_at = NOW()",
                &[&challenge_uuid, &assignment.hotkey, &assignment.weight],
            )
            .await?;
    }

    Ok(())
}

pub async fn get_entry_by_hotkey(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    hotkey: &Hotkey,
) -> Result<Option<LeaderboardEntry>> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = hotkey.to_ss58();

    let row = client
        .query_opt(
            "SELECT hotkey, score, weight,
                    COALESCE(submissions_count, 0) as submissions_count,
                    updated_at,
                    (SELECT COUNT(*) + 1 FROM leaderboard lb2
                     WHERE lb2.challenge_id = $1 AND lb2.score > leaderboard.score) as rank
             FROM leaderboard
             WHERE challenge_id = $1 AND hotkey = $2",
            &[&challenge_uuid, &hotkey_ss58],
        )
        .await?;

    match row {
        Some(row) => {
            let rank: i64 = row.get("rank");
            Ok(Some(LeaderboardEntry {
                rank: rank as i32,
                hotkey_ss58: row.get("hotkey"),
                score: row.get("score"),
                weight: row.get("weight"),
                submissions_count: row.get("submissions_count"),
                last_evaluated_at: row.get("updated_at"),
            }))
        }
        None => Ok(None),
    }
}

pub async fn delete_leaderboard_entry(
    pool: &PgPool,
    challenge_id: &ChallengeId,
    hotkey: &Hotkey,
) -> Result<bool> {
    let client = pool.get().await?;
    let challenge_uuid: Uuid = challenge_id.0;
    let hotkey_ss58 = hotkey.to_ss58();

    let rows_affected = client
        .execute(
            "DELETE FROM leaderboard WHERE challenge_id = $1 AND hotkey = $2",
            &[&challenge_uuid, &hotkey_ss58],
        )
        .await?;

    Ok(rows_affected > 0)
}
