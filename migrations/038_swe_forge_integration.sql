-- Migration 038: SWE-Forge integration
-- Tracks evaluation results from term-executor workers via Basilica

CREATE TABLE IF NOT EXISTS swe_forge_evaluations (
    id SERIAL PRIMARY KEY,
    submission_id TEXT NOT NULL,
    agent_hash TEXT NOT NULL,
    miner_hotkey TEXT NOT NULL,
    executor_url TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    tasks_passed INTEGER NOT NULL DEFAULT 0,
    tasks_total INTEGER NOT NULL DEFAULT 0,
    tasks_failed INTEGER NOT NULL DEFAULT 0,
    aggregate_reward DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    execution_time_ms BIGINT,
    result_json JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_swe_forge_evaluations_agent ON swe_forge_evaluations(agent_hash);
CREATE INDEX IF NOT EXISTS idx_swe_forge_evaluations_miner ON swe_forge_evaluations(miner_hotkey);
CREATE INDEX IF NOT EXISTS idx_swe_forge_evaluations_batch ON swe_forge_evaluations(batch_id);
