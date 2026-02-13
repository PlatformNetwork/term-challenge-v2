-- Migration 036: Separate LLM Review Rules and Similarity Columns
-- Adds separate columns to track rules validation and similarity/plagiarism review independently.

-- Rules validation review columns
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_rules_review_status TEXT DEFAULT 'pending';
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_rules_review_model TEXT;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_rules_review_result JSONB;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_rules_reviewed_at TIMESTAMPTZ;

-- Similarity/plagiarism review columns
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_similarity_review_status TEXT DEFAULT 'pending';
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_similarity_review_model TEXT;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_similarity_review_result JSONB;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS llm_similarity_reviewed_at TIMESTAMPTZ;

-- Migrate existing data: copy legacy review data to rules columns
UPDATE submissions 
SET llm_rules_review_status = llm_review_status,
    llm_rules_review_model = llm_review_model,
    llm_rules_review_result = llm_review_result,
    llm_rules_reviewed_at = llm_reviewed_at
WHERE llm_review_status IS NOT NULL;

-- For agents flagged by plagiarism and rejected, set similarity status
UPDATE submissions 
SET llm_similarity_review_status = 'rejected'
WHERE plagiarism_status = 'flagged' 
  AND llm_review_status = 'rejected'
  AND llm_similarity_review_status = 'pending';
