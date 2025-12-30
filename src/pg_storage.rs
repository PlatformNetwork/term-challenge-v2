//! PostgreSQL Storage for Challenge Server Mode
//!
//! Provides persistent storage for challenge server running in subnet owner mode.
//! Uses the same PostgreSQL instance as platform-server but with a separate database.

use anyhow::Result;
use deadpool_postgres::{Config, Pool, Runtime};
use serde::{Deserialize, Serialize};
use tokio_postgres::NoTls;
use tracing::{debug, info};

const SCHEMA: &str = r#"
-- ============================================================================
-- MIGRATION: Drop old pending_evaluations table if it has old schema
-- ============================================================================
DO $$
BEGIN
    -- Check if pending_evaluations has old schema (claimed_by column)
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'pending_evaluations' AND column_name = 'claimed_by'
    ) THEN
        -- Drop old table and its indexes
        DROP TABLE IF EXISTS pending_evaluations CASCADE;
        RAISE NOTICE 'Dropped old pending_evaluations table (migration to new schema)';
    END IF;
END $$;

-- ============================================================================
-- SCHEMA
-- ============================================================================

-- Agent submissions (source code is SENSITIVE - only owner and validators can access)
CREATE TABLE IF NOT EXISTS submissions (
    id TEXT PRIMARY KEY,
    agent_hash TEXT NOT NULL UNIQUE,
    miner_hotkey TEXT NOT NULL,
    source_code TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    name TEXT,
    epoch BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_submissions_agent ON submissions(agent_hash);
CREATE INDEX IF NOT EXISTS idx_submissions_miner ON submissions(miner_hotkey);
CREATE INDEX IF NOT EXISTS idx_submissions_status ON submissions(status);
CREATE INDEX IF NOT EXISTS idx_submissions_epoch ON submissions(epoch);

-- Evaluation results from this challenge
CREATE TABLE IF NOT EXISTS evaluations (
    id TEXT PRIMARY KEY,
    submission_id TEXT NOT NULL,
    agent_hash TEXT NOT NULL,
    miner_hotkey TEXT NOT NULL,
    score REAL NOT NULL,
    tasks_passed INTEGER NOT NULL,
    tasks_total INTEGER NOT NULL,
    tasks_failed INTEGER NOT NULL,
    total_cost_usd REAL NOT NULL DEFAULT 0.0,
    execution_time_ms BIGINT,
    task_results JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_evaluations_agent ON evaluations(agent_hash);
CREATE INDEX IF NOT EXISTS idx_evaluations_submission ON evaluations(submission_id);
CREATE INDEX IF NOT EXISTS idx_evaluations_created ON evaluations(created_at DESC);

-- Leaderboard for this challenge (PUBLIC - no source code)
CREATE TABLE IF NOT EXISTS leaderboard (
    agent_hash TEXT PRIMARY KEY,
    miner_hotkey TEXT NOT NULL,
    name TEXT,
    best_score REAL NOT NULL,
    avg_score REAL NOT NULL,
    evaluation_count INTEGER NOT NULL DEFAULT 0,
    total_cost_usd REAL NOT NULL DEFAULT 0.0,
    rank INTEGER,
    first_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_leaderboard_rank ON leaderboard(rank);
CREATE INDEX IF NOT EXISTS idx_leaderboard_score ON leaderboard(best_score DESC);

-- Pending evaluations (queued for processing by ALL validators)
-- Each agent needs evaluation by ALL active validators
CREATE TABLE IF NOT EXISTS pending_evaluations (
    id TEXT PRIMARY KEY,
    submission_id TEXT NOT NULL,
    agent_hash TEXT NOT NULL UNIQUE,
    miner_hotkey TEXT NOT NULL,
    epoch BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    validators_completed INTEGER NOT NULL DEFAULT 0,
    total_validators INTEGER NOT NULL DEFAULT 0,
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    window_expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '6 hours'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pending_status ON pending_evaluations(status);
CREATE INDEX IF NOT EXISTS idx_pending_agent ON pending_evaluations(agent_hash);
CREATE INDEX IF NOT EXISTS idx_pending_window ON pending_evaluations(window_expires_at);

-- Validator evaluations: ONE evaluation per validator per agent
-- ALL validators must evaluate each agent (except late ones after 6h)
CREATE TABLE IF NOT EXISTS validator_evaluations (
    id TEXT PRIMARY KEY,
    agent_hash TEXT NOT NULL,
    validator_hotkey TEXT NOT NULL,
    submission_id TEXT NOT NULL,
    miner_hotkey TEXT NOT NULL,
    score REAL NOT NULL,
    tasks_passed INTEGER NOT NULL,
    tasks_total INTEGER NOT NULL,
    tasks_failed INTEGER NOT NULL,
    total_cost_usd REAL NOT NULL DEFAULT 0.0,
    execution_time_ms BIGINT,
    task_results JSONB,
    epoch BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- UNIQUE: 1 evaluation per validator per agent
    UNIQUE(agent_hash, validator_hotkey)
);

CREATE INDEX IF NOT EXISTS idx_val_evals_agent ON validator_evaluations(agent_hash);
CREATE INDEX IF NOT EXISTS idx_val_evals_validator ON validator_evaluations(validator_hotkey);
CREATE INDEX IF NOT EXISTS idx_val_evals_epoch ON validator_evaluations(epoch);

-- Track which validators have claimed which agents (in progress)
CREATE TABLE IF NOT EXISTS validator_claims (
    id TEXT PRIMARY KEY,
    agent_hash TEXT NOT NULL,
    validator_hotkey TEXT NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL DEFAULT 'claimed',
    
    -- UNIQUE: 1 active claim per validator per agent
    UNIQUE(agent_hash, validator_hotkey)
);

CREATE INDEX IF NOT EXISTS idx_claims_agent ON validator_claims(agent_hash);
CREATE INDEX IF NOT EXISTS idx_claims_validator ON validator_claims(validator_hotkey);

-- Config cache
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Current epoch tracking
CREATE TABLE IF NOT EXISTS epoch_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    current_epoch BIGINT NOT NULL DEFAULT 0,
    last_epoch_change TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO epoch_state (id, current_epoch) VALUES (1, 0) ON CONFLICT DO NOTHING;
"#;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Agent submission record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub source_code: String,
    pub source_hash: String,
    pub name: Option<String>,
    pub epoch: i64,
    pub status: String,
    pub created_at: i64,
}

/// Submission without source code (for listings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionInfo {
    pub id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub epoch: i64,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRecord {
    pub id: String,
    pub submission_id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub score: f64,
    pub tasks_passed: i32,
    pub tasks_total: i32,
    pub tasks_failed: i32,
    pub total_cost_usd: f64,
    pub execution_time_ms: Option<i64>,
    pub task_results: Option<serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub name: Option<String>,
    pub best_score: f64,
    pub avg_score: f64,
    pub evaluation_count: i32,
    pub total_cost_usd: f64,
    pub rank: Option<i32>,
}

/// Pending evaluation - one per agent, ALL validators must evaluate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEvaluation {
    pub id: String,
    pub submission_id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub epoch: i64,
    pub status: String,
    pub validators_completed: i32,
    pub total_validators: i32,
    pub window_started_at: i64,
    pub window_expires_at: i64,
    pub created_at: i64,
}

/// Validator's evaluation result for one agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorEvaluation {
    pub id: String,
    pub agent_hash: String,
    pub validator_hotkey: String,
    pub submission_id: String,
    pub miner_hotkey: String,
    pub score: f64,
    pub tasks_passed: i32,
    pub tasks_total: i32,
    pub tasks_failed: i32,
    pub total_cost_usd: f64,
    pub execution_time_ms: Option<i64>,
    pub task_results: Option<serde_json::Value>,
    pub epoch: i64,
    pub created_at: i64,
}

/// Active claim - validator is working on this agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorClaim {
    pub id: String,
    pub agent_hash: String,
    pub validator_hotkey: String,
    pub claimed_at: i64,
    pub status: String,
}

/// Job info returned when claiming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimableJob {
    pub pending_id: String,
    pub submission_id: String,
    pub agent_hash: String,
    pub miner_hotkey: String,
    pub source_code: String,
    pub window_expires_at: i64,
}

#[derive(Clone)]
pub struct PgStorage {
    pool: Pool,
}

impl PgStorage {
    /// Create storage from DATABASE_URL
    pub async fn new(database_url: &str) -> Result<Self> {
        let mut config = Config::new();
        config.url = Some(database_url.to_string());
        let pool = config.create_pool(Some(Runtime::Tokio1), NoTls)?;

        // Test connection
        let client = pool.get().await?;
        info!("Connected to PostgreSQL database");

        // Run migrations
        client.batch_execute(SCHEMA).await?;
        info!("Database schema initialized");

        Ok(Self { pool })
    }

    /// Create storage from DATABASE_URL environment variable
    pub async fn from_env() -> Result<Self> {
        let url =
            std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?;
        Self::new(&url).await
    }

    // ========================================================================
    // EVALUATIONS
    // ========================================================================

    /// Store an evaluation result
    pub async fn store_evaluation(&self, eval: &EvaluationRecord) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "INSERT INTO evaluations (id, submission_id, agent_hash, miner_hotkey, score, tasks_passed, tasks_total, tasks_failed, total_cost_usd, execution_time_ms, task_results)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT(id) DO UPDATE SET
                score = EXCLUDED.score,
                tasks_passed = EXCLUDED.tasks_passed,
                tasks_total = EXCLUDED.tasks_total,
                tasks_failed = EXCLUDED.tasks_failed,
                total_cost_usd = EXCLUDED.total_cost_usd,
                execution_time_ms = EXCLUDED.execution_time_ms,
                task_results = EXCLUDED.task_results",
            &[
                &eval.id, &eval.submission_id, &eval.agent_hash, &eval.miner_hotkey,
                &eval.score, &eval.tasks_passed, &eval.tasks_total, &eval.tasks_failed,
                &eval.total_cost_usd, &eval.execution_time_ms, &eval.task_results,
            ],
        ).await?;

        // Update leaderboard
        self.update_leaderboard(
            &eval.agent_hash,
            &eval.miner_hotkey,
            eval.score,
            eval.total_cost_usd,
        )
        .await?;

        debug!(
            "Stored evaluation {} for agent {}",
            eval.id, eval.agent_hash
        );
        Ok(())
    }

    /// Get evaluations for an agent
    pub async fn get_evaluations(&self, agent_hash: &str) -> Result<Vec<EvaluationRecord>> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, submission_id, agent_hash, miner_hotkey, score, tasks_passed, tasks_total, tasks_failed, total_cost_usd, execution_time_ms, task_results, EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM evaluations WHERE agent_hash = $1 ORDER BY created_at DESC",
            &[&agent_hash],
        ).await?;

        Ok(rows
            .iter()
            .map(|r| EvaluationRecord {
                id: r.get(0),
                submission_id: r.get(1),
                agent_hash: r.get(2),
                miner_hotkey: r.get(3),
                score: r.get(4),
                tasks_passed: r.get(5),
                tasks_total: r.get(6),
                tasks_failed: r.get(7),
                total_cost_usd: r.get(8),
                execution_time_ms: r.get(9),
                task_results: r.get(10),
                created_at: r.get(11),
            })
            .collect())
    }

    // ========================================================================
    // LEADERBOARD
    // ========================================================================

    /// Update leaderboard entry
    async fn update_leaderboard(
        &self,
        agent_hash: &str,
        miner_hotkey: &str,
        score: f64,
        cost: f64,
    ) -> Result<()> {
        let client = self.pool.get().await?;

        // Upsert leaderboard entry
        client.execute(
            "INSERT INTO leaderboard (agent_hash, miner_hotkey, best_score, avg_score, evaluation_count, total_cost_usd)
             VALUES ($1, $2, $3, $3, 1, $4)
             ON CONFLICT(agent_hash) DO UPDATE SET
                best_score = GREATEST(leaderboard.best_score, EXCLUDED.best_score),
                avg_score = (leaderboard.avg_score * leaderboard.evaluation_count + EXCLUDED.avg_score) / (leaderboard.evaluation_count + 1),
                evaluation_count = leaderboard.evaluation_count + 1,
                total_cost_usd = leaderboard.total_cost_usd + EXCLUDED.total_cost_usd,
                last_updated = NOW()",
            &[&agent_hash, &miner_hotkey, &score, &cost],
        ).await?;

        // Update ranks
        client.execute(
            "UPDATE leaderboard SET rank = subq.new_rank
             FROM (SELECT agent_hash, ROW_NUMBER() OVER (ORDER BY best_score DESC) as new_rank FROM leaderboard) subq
             WHERE leaderboard.agent_hash = subq.agent_hash",
            &[],
        ).await?;

        Ok(())
    }

    /// Get leaderboard
    pub async fn get_leaderboard(&self, limit: i64) -> Result<Vec<LeaderboardEntry>> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT agent_hash, miner_hotkey, name, best_score, avg_score, evaluation_count, total_cost_usd, rank
             FROM leaderboard ORDER BY rank ASC NULLS LAST LIMIT $1",
            &[&limit],
        ).await?;

        Ok(rows
            .iter()
            .map(|r| LeaderboardEntry {
                agent_hash: r.get(0),
                miner_hotkey: r.get(1),
                name: r.get(2),
                best_score: r.get(3),
                avg_score: r.get(4),
                evaluation_count: r.get(5),
                total_cost_usd: r.get(6),
                rank: r.get(7),
            })
            .collect())
    }

    /// Get leaderboard entry for an agent
    pub async fn get_leaderboard_entry(
        &self,
        agent_hash: &str,
    ) -> Result<Option<LeaderboardEntry>> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT agent_hash, miner_hotkey, name, best_score, avg_score, evaluation_count, total_cost_usd, rank
             FROM leaderboard WHERE agent_hash = $1",
            &[&agent_hash],
        ).await?;

        Ok(row.map(|r| LeaderboardEntry {
            agent_hash: r.get(0),
            miner_hotkey: r.get(1),
            name: r.get(2),
            best_score: r.get(3),
            avg_score: r.get(4),
            evaluation_count: r.get(5),
            total_cost_usd: r.get(6),
            rank: r.get(7),
        }))
    }

    // ========================================================================
    // SUBMISSIONS (SENSITIVE - source code access controlled)
    // ========================================================================

    /// Create a new submission
    pub async fn create_submission(&self, submission: &Submission) -> Result<()> {
        debug!(
            "Creating submission: id={}, agent_hash={}, miner={}",
            submission.id, submission.agent_hash, submission.miner_hotkey
        );

        let client = self.pool.get().await.map_err(|e| {
            tracing::error!("Failed to get DB connection: {:?}", e);
            anyhow::anyhow!("db connection error: {}", e)
        })?;

        debug!("Inserting into submissions table...");
        client.execute(
            "INSERT INTO submissions (id, agent_hash, miner_hotkey, source_code, source_hash, name, epoch, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT(agent_hash) DO UPDATE SET
                source_code = EXCLUDED.source_code,
                source_hash = EXCLUDED.source_hash,
                name = EXCLUDED.name,
                status = EXCLUDED.status",
            &[
                &submission.id, &submission.agent_hash, &submission.miner_hotkey,
                &submission.source_code, &submission.source_hash, &submission.name,
                &submission.epoch, &submission.status,
            ],
        ).await.map_err(|e| {
            tracing::error!("Failed to insert submission: {:?}", e);
            anyhow::anyhow!("db insert error: {}", e)
        })?;

        info!(
            "Created submission {} for agent {}",
            submission.id, submission.agent_hash
        );
        Ok(())
    }

    /// Queue a submission for evaluation by all validators
    /// Call this after creating submission, with validator count from platform-server
    pub async fn queue_submission_for_evaluation(
        &self,
        submission_id: &str,
        agent_hash: &str,
        miner_hotkey: &str,
        total_validators: i32,
    ) -> Result<String> {
        debug!(
            "Queueing submission {} for {} validators",
            agent_hash, total_validators
        );

        self.queue_for_all_validators(submission_id, agent_hash, miner_hotkey, total_validators)
            .await
            .map_err(|e| {
                tracing::error!("Failed to queue evaluation: {:?}", e);
                anyhow::anyhow!("db queue error: {}", e)
            })
    }

    /// Get submission by agent hash (includes source code - SENSITIVE)
    pub async fn get_submission(&self, agent_hash: &str) -> Result<Option<Submission>> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, agent_hash, miner_hotkey, source_code, source_hash, name, epoch, status, EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM submissions WHERE agent_hash = $1",
            &[&agent_hash],
        ).await?;

        Ok(row.map(|r| Submission {
            id: r.get(0),
            agent_hash: r.get(1),
            miner_hotkey: r.get(2),
            source_code: r.get(3),
            source_hash: r.get(4),
            name: r.get(5),
            epoch: r.get(6),
            status: r.get(7),
            created_at: r.get(8),
        }))
    }

    /// Get submission info by agent hash (NO source code - safe for listings)
    pub async fn get_submission_info(&self, agent_hash: &str) -> Result<Option<SubmissionInfo>> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, agent_hash, miner_hotkey, name, epoch, status, EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM submissions WHERE agent_hash = $1",
            &[&agent_hash],
        ).await?;

        Ok(row.map(|r| SubmissionInfo {
            id: r.get(0),
            agent_hash: r.get(1),
            miner_hotkey: r.get(2),
            name: r.get(3),
            epoch: r.get(4),
            status: r.get(5),
            created_at: r.get(6),
        }))
    }

    /// Get all submissions for a miner (NO source code)
    pub async fn get_miner_submissions(&self, miner_hotkey: &str) -> Result<Vec<SubmissionInfo>> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, agent_hash, miner_hotkey, name, epoch, status, EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM submissions WHERE miner_hotkey = $1 ORDER BY created_at DESC",
            &[&miner_hotkey],
        ).await?;

        Ok(rows
            .iter()
            .map(|r| SubmissionInfo {
                id: r.get(0),
                agent_hash: r.get(1),
                miner_hotkey: r.get(2),
                name: r.get(3),
                epoch: r.get(4),
                status: r.get(5),
                created_at: r.get(6),
            })
            .collect())
    }

    /// Update submission status
    pub async fn update_submission_status(&self, agent_hash: &str, status: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(
                "UPDATE submissions SET status = $1 WHERE agent_hash = $2",
                &[&status, &agent_hash],
            )
            .await?;
        Ok(())
    }

    /// Check if agent hash exists
    pub async fn submission_exists(&self, agent_hash: &str) -> Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT 1 FROM submissions WHERE agent_hash = $1",
                &[&agent_hash],
            )
            .await?;
        Ok(row.is_some())
    }

    // ========================================================================
    // DISTRIBUTED EVALUATION SYSTEM
    // All validators must evaluate each agent. 6h window for late validators.
    // ========================================================================

    /// Queue an agent for evaluation by ALL validators
    pub async fn queue_for_all_validators(
        &self,
        submission_id: &str,
        agent_hash: &str,
        miner_hotkey: &str,
        total_validators: i32,
    ) -> Result<String> {
        let client = self.pool.get().await?;
        let id = uuid::Uuid::new_v4().to_string();
        let epoch = self.get_current_epoch().await.unwrap_or(0);

        client.execute(
            "INSERT INTO pending_evaluations 
             (id, submission_id, agent_hash, miner_hotkey, epoch, status, total_validators, validators_completed)
             VALUES ($1, $2, $3, $4, $5, 'pending', $6, 0)
             ON CONFLICT(agent_hash) DO UPDATE SET
                total_validators = EXCLUDED.total_validators,
                status = CASE WHEN pending_evaluations.status = 'completed' THEN pending_evaluations.status ELSE 'pending' END",
            &[&id, &submission_id, &agent_hash, &miner_hotkey, &epoch, &total_validators],
        ).await?;

        info!(
            "Queued agent {} for evaluation by {} validators",
            agent_hash, total_validators
        );
        Ok(id)
    }

    /// Get jobs available for a specific validator
    /// Returns jobs that:
    /// 1. Are in 'pending' or 'evaluating' status
    /// 2. Have NOT been evaluated by this validator yet
    /// 3. Are within the 6h window (not expired)
    pub async fn get_jobs_for_validator(
        &self,
        validator_hotkey: &str,
        limit: i64,
    ) -> Result<Vec<ClaimableJob>> {
        let client = self.pool.get().await?;

        let rows = client
            .query(
                "SELECT p.id, p.submission_id, p.agent_hash, p.miner_hotkey, s.source_code,
                    EXTRACT(EPOCH FROM p.window_expires_at)::BIGINT
             FROM pending_evaluations p
             JOIN submissions s ON s.agent_hash = p.agent_hash
             WHERE p.status IN ('pending', 'evaluating')
               AND p.window_expires_at > NOW()
               AND NOT EXISTS (
                   SELECT 1 FROM validator_evaluations ve 
                   WHERE ve.agent_hash = p.agent_hash 
                   AND ve.validator_hotkey = $1
               )
               AND NOT EXISTS (
                   SELECT 1 FROM validator_claims vc
                   WHERE vc.agent_hash = p.agent_hash
                   AND vc.validator_hotkey = $1
                   AND vc.status = 'claimed'
               )
             ORDER BY p.created_at ASC
             LIMIT $2",
                &[&validator_hotkey, &limit],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|r| ClaimableJob {
                pending_id: r.get(0),
                submission_id: r.get(1),
                agent_hash: r.get(2),
                miner_hotkey: r.get(3),
                source_code: r.get(4),
                window_expires_at: r.get(5),
            })
            .collect())
    }

    /// Claim jobs for a validator (mark as in-progress)
    pub async fn claim_jobs(
        &self,
        validator_hotkey: &str,
        agent_hashes: &[String],
    ) -> Result<usize> {
        let client = self.pool.get().await?;
        let mut claimed = 0;

        for agent_hash in agent_hashes {
            let id = uuid::Uuid::new_v4().to_string();
            let result = client
                .execute(
                    "INSERT INTO validator_claims (id, agent_hash, validator_hotkey, status)
                 VALUES ($1, $2, $3, 'claimed')
                 ON CONFLICT(agent_hash, validator_hotkey) DO NOTHING",
                    &[&id, &agent_hash, &validator_hotkey],
                )
                .await?;

            if result > 0 {
                claimed += 1;
                debug!(
                    "Validator {} claimed agent {}",
                    validator_hotkey, agent_hash
                );
            }
        }

        Ok(claimed)
    }

    /// Check if validator has already evaluated an agent
    pub async fn has_validator_evaluated(
        &self,
        agent_hash: &str,
        validator_hotkey: &str,
    ) -> Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT 1 FROM validator_evaluations 
             WHERE agent_hash = $1 AND validator_hotkey = $2",
                &[&agent_hash, &validator_hotkey],
            )
            .await?;
        Ok(row.is_some())
    }

    /// Check if evaluation window has expired (6h rule)
    pub async fn is_window_expired(&self, agent_hash: &str) -> Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT 1 FROM pending_evaluations 
             WHERE agent_hash = $1 AND window_expires_at < NOW()",
                &[&agent_hash],
            )
            .await?;
        Ok(row.is_some())
    }

    /// Submit a validator's evaluation result
    /// Returns (is_late, consensus_reached, final_score)
    pub async fn submit_validator_evaluation(
        &self,
        eval: &ValidatorEvaluation,
    ) -> Result<(bool, bool, Option<f64>)> {
        let client = self.pool.get().await?;

        // Check if window expired
        let window_row = client.query_opt(
            "SELECT window_expires_at < NOW() as expired, validators_completed, total_validators
             FROM pending_evaluations WHERE agent_hash = $1",
            &[&eval.agent_hash],
        ).await?;

        let (is_expired, validators_completed, total_validators) = match window_row {
            Some(r) => {
                let expired: bool = r.get(0);
                let completed: i32 = r.get(1);
                let total: i32 = r.get(2);
                (expired, completed, total)
            }
            None => return Err(anyhow::anyhow!("Agent not found in pending evaluations")),
        };

        if is_expired {
            info!(
                "Validator {} is LATE for agent {} (window expired)",
                &eval.validator_hotkey[..16.min(eval.validator_hotkey.len())],
                &eval.agent_hash[..16]
            );
            // Remove the claim since they're late
            client
                .execute(
                    "DELETE FROM validator_claims WHERE agent_hash = $1 AND validator_hotkey = $2",
                    &[&eval.agent_hash, &eval.validator_hotkey],
                )
                .await?;
            return Ok((true, false, None));
        }

        // Insert the evaluation (UNIQUE constraint ensures 1 per validator per agent)
        let insert_result = client.execute(
            "INSERT INTO validator_evaluations 
             (id, agent_hash, validator_hotkey, submission_id, miner_hotkey, score, 
              tasks_passed, tasks_total, tasks_failed, total_cost_usd, execution_time_ms, task_results, epoch)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             ON CONFLICT(agent_hash, validator_hotkey) DO UPDATE SET
                score = EXCLUDED.score,
                tasks_passed = EXCLUDED.tasks_passed,
                tasks_total = EXCLUDED.tasks_total,
                tasks_failed = EXCLUDED.tasks_failed,
                total_cost_usd = EXCLUDED.total_cost_usd,
                execution_time_ms = EXCLUDED.execution_time_ms,
                task_results = EXCLUDED.task_results",
            &[
                &eval.id, &eval.agent_hash, &eval.validator_hotkey, &eval.submission_id,
                &eval.miner_hotkey, &eval.score, &eval.tasks_passed, &eval.tasks_total,
                &eval.tasks_failed, &eval.total_cost_usd, &eval.execution_time_ms,
                &eval.task_results, &eval.epoch,
            ],
        ).await?;

        if insert_result > 0 {
            // Update claim status
            client
                .execute(
                    "UPDATE validator_claims SET status = 'completed' 
                 WHERE agent_hash = $1 AND validator_hotkey = $2",
                    &[&eval.agent_hash, &eval.validator_hotkey],
                )
                .await?;

            // Increment validators_completed counter
            client
                .execute(
                    "UPDATE pending_evaluations SET validators_completed = validators_completed + 1
                 WHERE agent_hash = $1",
                    &[&eval.agent_hash],
                )
                .await?;
        }

        // Check if all validators have completed
        let new_completed = validators_completed + 1;
        let all_done = new_completed >= total_validators;

        if all_done {
            // Calculate consensus score and finalize
            let final_score = self.calculate_and_store_consensus(&eval.agent_hash).await?;
            return Ok((false, true, Some(final_score)));
        }

        info!(
            "Validator {} submitted evaluation for {} ({}/{} validators done)",
            &eval.validator_hotkey[..16.min(eval.validator_hotkey.len())],
            &eval.agent_hash[..16],
            new_completed,
            total_validators
        );

        Ok((false, false, None))
    }

    /// Calculate consensus score from all validator evaluations
    /// Currently uses simple average (can be extended to stake-weighted)
    async fn calculate_and_store_consensus(&self, agent_hash: &str) -> Result<f64> {
        let client = self.pool.get().await?;

        // Get all evaluations for this agent
        let rows = client
            .query(
                "SELECT score, tasks_passed, tasks_total, tasks_failed, total_cost_usd, 
                    execution_time_ms, submission_id, miner_hotkey
             FROM validator_evaluations WHERE agent_hash = $1",
                &[&agent_hash],
            )
            .await?;

        if rows.is_empty() {
            return Err(anyhow::anyhow!("No evaluations found for agent"));
        }

        // Calculate averages
        let mut total_score = 0.0;
        let mut total_tasks_passed = 0;
        let mut total_tasks_total = 0;
        let mut total_tasks_failed = 0;
        let mut total_cost = 0.0;
        let mut total_time: i64 = 0;
        let count = rows.len() as f64;

        let mut submission_id = String::new();
        let mut miner_hotkey = String::new();

        for row in &rows {
            let score: f64 = row.get(0);
            let passed: i32 = row.get(1);
            let total: i32 = row.get(2);
            let failed: i32 = row.get(3);
            let cost: f64 = row.get(4);
            let time: Option<i64> = row.get(5);

            total_score += score;
            total_tasks_passed += passed;
            total_tasks_total += total;
            total_tasks_failed += failed;
            total_cost += cost;
            total_time += time.unwrap_or(0);

            if submission_id.is_empty() {
                submission_id = row.get(6);
                miner_hotkey = row.get(7);
            }
        }

        let final_score = total_score / count;
        let avg_passed = (total_tasks_passed as f64 / count).round() as i32;
        let avg_total = (total_tasks_total as f64 / count).round() as i32;
        let avg_failed = (total_tasks_failed as f64 / count).round() as i32;
        let avg_cost = total_cost / count;
        let avg_time = (total_time as f64 / count).round() as i64;

        // Store final consensus result
        let eval_id = uuid::Uuid::new_v4().to_string();
        client
            .execute(
                "INSERT INTO evaluations 
             (id, submission_id, agent_hash, miner_hotkey, score, tasks_passed, tasks_total, 
              tasks_failed, total_cost_usd, execution_time_ms)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT(id) DO NOTHING",
                &[
                    &eval_id,
                    &submission_id,
                    &agent_hash,
                    &miner_hotkey,
                    &final_score,
                    &avg_passed,
                    &avg_total,
                    &avg_failed,
                    &avg_cost,
                    &avg_time,
                ],
            )
            .await?;

        // Update pending_evaluations status
        client
            .execute(
                "UPDATE pending_evaluations SET status = 'completed' WHERE agent_hash = $1",
                &[&agent_hash],
            )
            .await?;

        // Update leaderboard
        self.update_leaderboard(agent_hash, &miner_hotkey, final_score, avg_cost)
            .await?;

        info!(
            "Consensus reached for agent {}: score={:.4} from {} validators",
            &agent_hash[..16],
            final_score,
            rows.len()
        );

        Ok(final_score)
    }

    /// Get all validator evaluations for an agent
    pub async fn get_validator_evaluations(
        &self,
        agent_hash: &str,
    ) -> Result<Vec<ValidatorEvaluation>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, agent_hash, validator_hotkey, submission_id, miner_hotkey,
                    score, tasks_passed, tasks_total, tasks_failed, total_cost_usd,
                    execution_time_ms, task_results, epoch, 
                    EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM validator_evaluations WHERE agent_hash = $1
             ORDER BY created_at ASC",
                &[&agent_hash],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|r| ValidatorEvaluation {
                id: r.get(0),
                agent_hash: r.get(1),
                validator_hotkey: r.get(2),
                submission_id: r.get(3),
                miner_hotkey: r.get(4),
                score: r.get(5),
                tasks_passed: r.get(6),
                tasks_total: r.get(7),
                tasks_failed: r.get(8),
                total_cost_usd: r.get(9),
                execution_time_ms: r.get(10),
                task_results: r.get(11),
                epoch: r.get(12),
                created_at: r.get(13),
            })
            .collect())
    }

    /// Get pending evaluation status for an agent
    pub async fn get_pending_status(&self, agent_hash: &str) -> Result<Option<PendingEvaluation>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT id, submission_id, agent_hash, miner_hotkey, epoch, status,
                    validators_completed, total_validators,
                    EXTRACT(EPOCH FROM window_started_at)::BIGINT,
                    EXTRACT(EPOCH FROM window_expires_at)::BIGINT,
                    EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM pending_evaluations WHERE agent_hash = $1",
                &[&agent_hash],
            )
            .await?;

        Ok(row.map(|r| PendingEvaluation {
            id: r.get(0),
            submission_id: r.get(1),
            agent_hash: r.get(2),
            miner_hotkey: r.get(3),
            epoch: r.get(4),
            status: r.get(5),
            validators_completed: r.get(6),
            total_validators: r.get(7),
            window_started_at: r.get(8),
            window_expires_at: r.get(9),
            created_at: r.get(10),
        }))
    }

    /// Expire old evaluation windows and calculate consensus for partial results
    pub async fn expire_old_windows(&self) -> Result<u64> {
        let client = self.pool.get().await?;

        // Get agents with expired windows that haven't been completed
        let rows = client
            .query(
                "SELECT agent_hash FROM pending_evaluations 
             WHERE status != 'completed' AND window_expires_at < NOW()",
                &[],
            )
            .await?;

        let mut expired_count = 0u64;
        for row in rows {
            let agent_hash: String = row.get(0);

            // Calculate consensus with whatever evaluations we have
            match self.calculate_and_store_consensus(&agent_hash).await {
                Ok(score) => {
                    info!(
                        "Expired window for agent {} - consensus score: {:.4}",
                        &agent_hash[..16],
                        score
                    );
                    expired_count += 1;
                }
                Err(e) => {
                    // No evaluations yet - mark as failed
                    debug!(
                        "No evaluations for expired agent {}: {}",
                        &agent_hash[..16],
                        e
                    );
                    client.execute(
                        "UPDATE pending_evaluations SET status = 'expired' WHERE agent_hash = $1",
                        &[&agent_hash],
                    ).await?;
                    expired_count += 1;
                }
            }
        }

        if expired_count > 0 {
            info!("Expired {} evaluation windows", expired_count);
        }

        Ok(expired_count)
    }

    /// Get validator's active claims
    pub async fn get_validator_claims(
        &self,
        validator_hotkey: &str,
    ) -> Result<Vec<ValidatorClaim>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, agent_hash, validator_hotkey, 
                    EXTRACT(EPOCH FROM claimed_at)::BIGINT, status
             FROM validator_claims 
             WHERE validator_hotkey = $1 AND status = 'claimed'
             ORDER BY claimed_at ASC",
                &[&validator_hotkey],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|r| ValidatorClaim {
                id: r.get(0),
                agent_hash: r.get(1),
                validator_hotkey: r.get(2),
                claimed_at: r.get(3),
                status: r.get(4),
            })
            .collect())
    }

    /// Release a claim (validator giving up)
    pub async fn release_claim(&self, agent_hash: &str, validator_hotkey: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(
                "DELETE FROM validator_claims WHERE agent_hash = $1 AND validator_hotkey = $2",
                &[&agent_hash, &validator_hotkey],
            )
            .await?;
        Ok(())
    }

    /// Get all pending evaluations (for status endpoint)
    pub async fn get_all_pending(&self) -> Result<Vec<PendingEvaluation>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, submission_id, agent_hash, miner_hotkey, epoch, status,
                    validators_completed, total_validators,
                    EXTRACT(EPOCH FROM window_started_at)::BIGINT,
                    EXTRACT(EPOCH FROM window_expires_at)::BIGINT,
                    EXTRACT(EPOCH FROM created_at)::BIGINT
             FROM pending_evaluations 
             WHERE status IN ('pending', 'evaluating')
             ORDER BY created_at ASC",
                &[],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|r| PendingEvaluation {
                id: r.get(0),
                submission_id: r.get(1),
                agent_hash: r.get(2),
                miner_hotkey: r.get(3),
                epoch: r.get(4),
                status: r.get(5),
                validators_completed: r.get(6),
                total_validators: r.get(7),
                window_started_at: r.get(8),
                window_expires_at: r.get(9),
                created_at: r.get(10),
            })
            .collect())
    }

    // ========================================================================
    // EPOCH
    // ========================================================================

    /// Get current epoch
    pub async fn get_current_epoch(&self) -> Result<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT current_epoch FROM epoch_state WHERE id = 1", &[])
            .await?;
        Ok(row.get(0))
    }

    /// Set current epoch
    pub async fn set_current_epoch(&self, epoch: i64) -> Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(
                "UPDATE epoch_state SET current_epoch = $1, last_epoch_change = NOW() WHERE id = 1",
                &[&epoch],
            )
            .await?;
        Ok(())
    }

    // ========================================================================
    // CONFIG
    // ========================================================================

    /// Set config value
    pub async fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO config (key, value, updated_at) VALUES ($1, $2, NOW())
             ON CONFLICT(key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
                &[&key, &value],
            )
            .await?;
        Ok(())
    }

    /// Get config value
    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt("SELECT value FROM config WHERE key = $1", &[&key])
            .await?;
        Ok(row.map(|r| r.get(0)))
    }
}
