-- Migration 023: Validation Rules + LLM Review Status
-- Date: 2026-02-10
-- Description: Store validation rules in DB (dynamic) and track LLM review status per submission.
--              The LLM review runs in an isolated Docker container using Chutes API (Kimi-K2.5-TEE).
--              This is the single source of truth for all validation rules.

-- ============================================================================
-- VALIDATION RULES TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS validation_rules (
    id SERIAL PRIMARY KEY,
    rule_text TEXT NOT NULL,
    rule_order INTEGER NOT NULL DEFAULT 0,
    category TEXT NOT NULL DEFAULT 'general',
    priority INTEGER NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_by TEXT NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_validation_rules_active ON validation_rules(active, rule_order) WHERE active = TRUE;

-- Insert default rules (previously hardcoded in ValidationRules::default_term_challenge_rules())
INSERT INTO validation_rules (rule_text, rule_order, category, priority, created_by) VALUES
('The agent must use only term_sdk (Agent, Request, Response, run) for terminal interaction. Response.cmd() is the CORRECT way to execute shell commands.', 1, 'compliance', 10, 'system'),
('The agent must not attempt to access the network or make HTTP requests directly (urllib, requests, socket).', 2, 'security', 9, 'system'),
('The agent must not use subprocess, os.system(), os.popen(), or exec() to run commands. Use Response.cmd() instead.', 3, 'security', 9, 'system'),
('The agent must not attempt to import forbidden modules (socket, requests, urllib, subprocess, os, sys for system calls).', 4, 'security', 9, 'system'),
('The agent must implement a valid solve(self, req: Request) method that returns Response objects.', 5, 'compliance', 8, 'system'),
('The agent must inherit from Agent class and use run(MyAgent()) in main.', 6, 'compliance', 8, 'system'),
('The agent must not contain obfuscated or encoded malicious code.', 7, 'security', 10, 'system'),
('The agent must not attempt to escape the sandbox environment.', 8, 'security', 10, 'system'),
('The agent must not contain infinite loops without termination conditions.', 9, 'readability', 7, 'system'),
('Response.cmd(''shell command'') is ALLOWED and is the proper way to execute terminal commands.', 10, 'compliance', 10, 'system')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- LLM REVIEW COLUMNS ON SUBMISSIONS
-- ============================================================================

ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_review_status TEXT DEFAULT 'pending';
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_review_model TEXT;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_review_result JSONB;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_reviewed_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_submissions_llm_review_pending
ON submissions(llm_review_status)
WHERE llm_review_status = 'pending';

COMMENT ON TABLE validation_rules IS 'Dynamic validation rules checked by LLM reviewer against agent code';
COMMENT ON COLUMN submissions.llm_review_status IS 'pending, reviewing, approved, rejected';
COMMENT ON COLUMN submissions.llm_review_model IS 'LLM model that performed the review (e.g. moonshotai/Kimi-K2.5-TEE)';
COMMENT ON COLUMN submissions.llm_review_result IS 'JSON: {approved: bool, reason: string, violations: [string]}';
COMMENT ON COLUMN submissions.llm_reviewed_at IS 'Timestamp when LLM review completed';
