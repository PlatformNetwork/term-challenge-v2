//! Database Migration System
//!
//! Handles running SQL migrations in order, tracking which have been applied.

use anyhow::Result;
use deadpool_postgres::Object;
use std::path::Path;
use tracing::{info, warn};

/// Migration entry
struct Migration {
    version: i32,
    name: String,
    sql: String,
}

/// Run all pending migrations
pub async fn run_migrations(client: &Object, migrations_dir: &Path) -> Result<()> {
    // Create migrations tracking table
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
            &[],
        )
        .await?;

    // Get applied migrations
    let applied: Vec<i32> = client
        .query(
            "SELECT version FROM schema_migrations ORDER BY version",
            &[],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();

    // Load migration files
    let mut migrations = load_migrations(migrations_dir)?;
    migrations.sort_by_key(|m| m.version);

    // Run pending migrations
    let mut applied_count = 0;
    for migration in migrations {
        if applied.contains(&migration.version) {
            continue;
        }

        info!(
            "Applying migration {}: {}",
            migration.version, migration.name
        );

        // Run migration in a transaction
        client.execute("BEGIN", &[]).await?;

        match client.batch_execute(&migration.sql).await {
            Ok(_) => {
                // Record migration
                client
                    .execute(
                        "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)",
                        &[&migration.version, &migration.name],
                    )
                    .await?;

                client.execute("COMMIT", &[]).await?;
                info!("Migration {} applied successfully", migration.version);
                applied_count += 1;
            }
            Err(e) => {
                client.execute("ROLLBACK", &[]).await?;
                return Err(anyhow::anyhow!(
                    "Migration {} failed: {}",
                    migration.version,
                    e
                ));
            }
        }
    }

    if applied_count > 0 {
        info!("Applied {} migrations", applied_count);
    } else {
        info!("Database schema is up to date");
    }

    Ok(())
}

/// Load migrations from directory
fn load_migrations(dir: &Path) -> Result<Vec<Migration>> {
    let mut migrations = Vec::new();

    if !dir.exists() {
        warn!("Migrations directory not found: {:?}", dir);
        return Ok(migrations);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "sql").unwrap_or(false) {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            // Parse version from filename (e.g., "001_initial_schema.sql")
            if let Some(version) = parse_migration_version(filename) {
                let name = filename
                    .split('_')
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("_")
                    .trim_end_matches(".sql")
                    .to_string();

                let sql = std::fs::read_to_string(&path)?;

                migrations.push(Migration { version, name, sql });
            }
        }
    }

    Ok(migrations)
}

/// Parse migration version from filename
fn parse_migration_version(filename: &str) -> Option<i32> {
    filename
        .split('_')
        .next()
        .and_then(|v| v.parse::<i32>().ok())
}

/// Embedded migrations (fallback when directory is not available)
pub const EMBEDDED_MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        include_str!("../migrations/001_initial_schema.sql"),
    ),
    (
        "002_add_api_keys_and_versioning",
        include_str!("../migrations/002_add_api_keys_and_versioning.sql"),
    ),
    (
        "003_add_epoch_submission_limit",
        include_str!("../migrations/003_add_epoch_submission_limit.sql"),
    ),
    (
        "004_validator_assignments",
        include_str!("../migrations/004_validator_assignments.sql"),
    ),
    (
        "005_task_logs",
        include_str!("../migrations/005_task_logs.sql"),
    ),
];

/// Run embedded migrations (when migrations dir is not available)
pub async fn run_embedded_migrations(client: &Object) -> Result<()> {
    // Create migrations tracking table
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
            &[],
        )
        .await?;

    // Get applied migrations
    let applied: Vec<i32> = client
        .query(
            "SELECT version FROM schema_migrations ORDER BY version",
            &[],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();

    let mut applied_count = 0;
    for (name, sql) in EMBEDDED_MIGRATIONS {
        let version = parse_migration_version(name).unwrap_or(0);

        if applied.contains(&version) {
            continue;
        }

        info!("Applying embedded migration {}: {}", version, name);

        client.execute("BEGIN", &[]).await?;

        match client.batch_execute(sql).await {
            Ok(_) => {
                client
                    .execute(
                        "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)",
                        &[&version, &name.to_string()],
                    )
                    .await?;

                client.execute("COMMIT", &[]).await?;
                info!("Migration {} applied successfully", version);
                applied_count += 1;
            }
            Err(e) => {
                client.execute("ROLLBACK", &[]).await?;
                return Err(anyhow::anyhow!("Migration {} failed: {}", version, e));
            }
        }
    }

    if applied_count > 0 {
        info!("Applied {} embedded migrations", applied_count);
    } else {
        info!("Database schema is up to date");
    }

    Ok(())
}
