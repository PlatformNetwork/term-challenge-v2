use chrono::{DateTime, Utc};
use platform_core::Hotkey;

use crate::pg::PgPool;
use crate::{Result, StorageError};

#[derive(Clone, Debug)]
pub struct StoredValidator {
    pub hotkey_ss58: String,
    pub stake: i64,
    pub is_active: bool,
    pub last_seen: DateTime<Utc>,
    pub peer_id: Option<String>,
    pub registered_at: DateTime<Utc>,
}

pub async fn upsert_validator(
    pool: &PgPool,
    hotkey: &Hotkey,
    stake: u64,
    peer_id: Option<&str>,
) -> Result<()> {
    let client = pool.get().await?;
    let hotkey_ss58 = hotkey.to_ss58();
    let stake_i64 = stake as i64;

    client
        .execute(
            "INSERT INTO validators (hotkey, stake, is_active, last_seen, peer_id, registered_at)
             VALUES ($1, $2, true, NOW(), $3, NOW())
             ON CONFLICT (hotkey) DO UPDATE SET
                stake = EXCLUDED.stake,
                is_active = true,
                last_seen = NOW(),
                peer_id = COALESCE(EXCLUDED.peer_id, validators.peer_id)",
            &[&hotkey_ss58, &stake_i64, &peer_id],
        )
        .await?;

    Ok(())
}

pub async fn get_validator(pool: &PgPool, hotkey: &Hotkey) -> Result<Option<StoredValidator>> {
    let client = pool.get().await?;
    let hotkey_ss58 = hotkey.to_ss58();

    let row = client
        .query_opt(
            "SELECT hotkey, stake, is_active, last_seen, peer_id, registered_at
             FROM validators WHERE hotkey = $1",
            &[&hotkey_ss58],
        )
        .await?;

    match row {
        Some(row) => Ok(Some(StoredValidator {
            hotkey_ss58: row.get("hotkey"),
            stake: row.get("stake"),
            is_active: row.get("is_active"),
            last_seen: row.get("last_seen"),
            peer_id: row.get("peer_id"),
            registered_at: row.get("registered_at"),
        })),
        None => Ok(None),
    }
}

pub async fn list_active_validators(pool: &PgPool) -> Result<Vec<StoredValidator>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT hotkey, stake, is_active, last_seen, peer_id, registered_at
             FROM validators
             WHERE is_active = true
             ORDER BY stake DESC",
            &[],
        )
        .await?;

    let mut validators = Vec::with_capacity(rows.len());
    for row in &rows {
        validators.push(StoredValidator {
            hotkey_ss58: row.get("hotkey"),
            stake: row.get("stake"),
            is_active: row.get("is_active"),
            last_seen: row.get("last_seen"),
            peer_id: row.get("peer_id"),
            registered_at: row.get("registered_at"),
        });
    }

    Ok(validators)
}

pub async fn list_all_validators(pool: &PgPool) -> Result<Vec<StoredValidator>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT hotkey, stake, is_active, last_seen, peer_id, registered_at
             FROM validators
             ORDER BY stake DESC",
            &[],
        )
        .await?;

    let mut validators = Vec::with_capacity(rows.len());
    for row in &rows {
        validators.push(StoredValidator {
            hotkey_ss58: row.get("hotkey"),
            stake: row.get("stake"),
            is_active: row.get("is_active"),
            last_seen: row.get("last_seen"),
            peer_id: row.get("peer_id"),
            registered_at: row.get("registered_at"),
        });
    }

    Ok(validators)
}

pub async fn deactivate_validator(pool: &PgPool, hotkey: &Hotkey) -> Result<bool> {
    let client = pool.get().await?;
    let hotkey_ss58 = hotkey.to_ss58();

    let rows_affected = client
        .execute(
            "UPDATE validators SET is_active = false WHERE hotkey = $1",
            &[&hotkey_ss58],
        )
        .await?;

    Ok(rows_affected > 0)
}

pub async fn update_last_seen(pool: &PgPool, hotkey: &Hotkey) -> Result<()> {
    let client = pool.get().await?;
    let hotkey_ss58 = hotkey.to_ss58();

    client
        .execute(
            "UPDATE validators SET last_seen = NOW() WHERE hotkey = $1",
            &[&hotkey_ss58],
        )
        .await?;

    Ok(())
}

pub async fn count_active_validators(pool: &PgPool) -> Result<i64> {
    let client = pool.get().await?;

    let row = client
        .query_one(
            "SELECT COUNT(*) as count FROM validators WHERE is_active = true",
            &[],
        )
        .await?;

    Ok(row.get("count"))
}

pub async fn delete_validator(pool: &PgPool, hotkey: &Hotkey) -> Result<bool> {
    let client = pool.get().await?;
    let hotkey_ss58 = hotkey.to_ss58();

    let rows_affected = client
        .execute("DELETE FROM validators WHERE hotkey = $1", &[&hotkey_ss58])
        .await?;

    Ok(rows_affected > 0)
}

pub async fn hotkey_from_ss58(ss58: &str) -> Result<Hotkey> {
    Hotkey::from_ss58(ss58)
        .ok_or_else(|| StorageError::InvalidData(format!("invalid SS58 address: {}", ss58)))
}
