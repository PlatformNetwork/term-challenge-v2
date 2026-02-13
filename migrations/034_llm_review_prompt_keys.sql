-- Migration 034: Rename system_prompt key and add similarity prompt key
-- The existing system_prompt key from migration 032 is renamed to system_prompt_rules
-- A new system_prompt_similarity key is inserted for plagiarism review

-- Step 1: Rename existing key
UPDATE llm_review_config
SET key = 'system_prompt_rules',
    updated_at = NOW(),
    updated_by = 'migration_034'
WHERE key = 'system_prompt';

-- Step 2: Insert similarity prompt key (will be updated by migration 035 with full content)
INSERT INTO llm_review_config (key, value, updated_by) VALUES (
    'system_prompt_similarity',
    'Similarity review prompt - see migration 035 for full content',
    'migration_034'
) ON CONFLICT (key) DO NOTHING;
