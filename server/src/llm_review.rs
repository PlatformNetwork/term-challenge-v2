use platform_challenge_sdk::ChallengeDatabase;
use serde_json::json;

use crate::types::{ChallengeParams, LlmReviewResult, SingleReview};

pub fn select_reviewers(validators_json: &[u8], submission_hash: &[u8], offset: u8) -> Vec<String> {
    let validators: Vec<String> = serde_json::from_slice(validators_json).unwrap_or_default();
    if validators.is_empty() {
        return Vec::new();
    }

    let seed = submission_hash
        .iter()
        .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));

    let count = 3.min(validators.len());
    let mut selected = Vec::with_capacity(count);
    let start = (seed.wrapping_add(offset as u64) as usize) % validators.len();

    for i in 0..count {
        let idx = (start + i) % validators.len();
        selected.push(validators[idx].clone());
    }

    selected
}

pub fn aggregate_reviews(results: &[LlmReviewResult]) -> LlmReviewResult {
    if results.is_empty() {
        return LlmReviewResult {
            submission_id: String::new(),
            approved: false,
            score: 0.0,
            explanation: "No reviews available".to_string(),
            reviewer_count: 0,
            reviews: Vec::new(),
        };
    }

    let submission_id = results[0].submission_id.clone();
    let total = results.len() as f64;
    let approved_count = results.iter().filter(|r| r.approved).count();
    let avg_score = results.iter().map(|r| r.score).sum::<f64>() / total;
    let approved = approved_count as f64 / total > 0.5;

    let mut all_reviews = Vec::new();
    for r in results {
        all_reviews.extend(r.reviews.clone());
    }

    LlmReviewResult {
        submission_id,
        approved,
        score: avg_score,
        explanation: format!(
            "{}/{} reviewers approved (avg score: {:.2})",
            approved_count,
            results.len(),
            avg_score
        ),
        reviewer_count: results.len() as u32,
        reviews: all_reviews,
    }
}

pub async fn perform_review(
    db: &ChallengeDatabase,
    submission_id: &str,
    code: &str,
    params: &ChallengeParams,
) -> LlmReviewResult {
    let api_url = match &params.llm_api_url {
        Some(url) if !url.is_empty() => url.clone(),
        _ => {
            return LlmReviewResult {
                submission_id: submission_id.to_string(),
                approved: true,
                score: 1.0,
                explanation: "LLM review skipped: no API URL configured".to_string(),
                reviewer_count: 0,
                reviews: Vec::new(),
            };
        }
    };

    let api_key = params.llm_api_key.as_deref().unwrap_or("").to_string();
    let model = params.llm_model.as_deref().unwrap_or("gpt-4").to_string();

    let prompt = format!(
        "Review this Python agent code for security issues. \
         Check for: malicious code, data exfiltration, unauthorized network access, \
         resource abuse, and code injection. \
         Respond with JSON: {{\"approved\": true/false, \"score\": 0.0-1.0, \"explanation\": \"...\"}}\n\n\
         Code:\n```python\n{}\n```",
        code
    );

    let request_body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a security code reviewer."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.1,
        "max_tokens": 500
    });

    let client = reqwest::Client::new();
    let mut request = client.post(&api_url).json(&request_body);

    if !api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = match request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            return LlmReviewResult {
                submission_id: submission_id.to_string(),
                approved: true,
                score: 0.5,
                explanation: format!("LLM review failed (defaulting to pass): {}", e),
                reviewer_count: 0,
                reviews: Vec::new(),
            };
        }
    };

    let body = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            return LlmReviewResult {
                submission_id: submission_id.to_string(),
                approved: true,
                score: 0.5,
                explanation: format!("Failed to read LLM response: {}", e),
                reviewer_count: 0,
                reviews: Vec::new(),
            };
        }
    };

    let review = parse_llm_response(&body, submission_id);

    let key = format!("review_result:{}", submission_id);
    let _ = db.kv_set(&key, &review);

    review
}

pub fn get_review_result(db: &ChallengeDatabase, submission_id: &str) -> Option<LlmReviewResult> {
    let key = format!("review_result:{}", submission_id);
    db.kv_get::<LlmReviewResult>(&key).ok().flatten()
}

fn parse_llm_response(body: &str, submission_id: &str) -> LlmReviewResult {
    let json_val: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            return LlmReviewResult {
                submission_id: submission_id.to_string(),
                approved: true,
                score: 0.5,
                explanation: "Failed to parse LLM response JSON".to_string(),
                reviewer_count: 1,
                reviews: Vec::new(),
            };
        }
    };

    let content = json_val
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let review_json: serde_json::Value = serde_json::from_str(content).unwrap_or(json!({
        "approved": true,
        "score": 0.5,
        "explanation": "Could not parse review content"
    }));

    let approved = review_json
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let score = review_json
        .get("score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let explanation = review_json
        .get("explanation")
        .and_then(|v| v.as_str())
        .unwrap_or("No explanation provided")
        .to_string();

    LlmReviewResult {
        submission_id: submission_id.to_string(),
        approved,
        score,
        explanation: explanation.clone(),
        reviewer_count: 1,
        reviews: vec![SingleReview {
            reviewer_id: "llm".to_string(),
            approved,
            score,
            explanation,
        }],
    }
}
