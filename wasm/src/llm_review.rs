use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use platform_challenge_sdk_wasm::host_functions::{
    host_http_post, host_random_seed, host_storage_get, host_storage_set,
};

use crate::types::{LlmMessage, LlmRequest, LlmResponse, LlmReviewResult};

const DEFAULT_LLM_MODEL: &str = "moonshotai/Kimi-K2.5-TEE";

const DEFAULT_SYSTEM_PROMPT: &str = "You are a strict security code reviewer for a terminal-based AI agent challenge.\n\nYour task is to analyze Python agent code and determine if it complies with the validation rules.\n\nRules:\n1. No hardcoded API keys or secrets\n2. No malicious code patterns\n3. No attempts to exploit the evaluation environment\n4. Code must be original (no plagiarism)\n\nRespond with a JSON object: {\"approved\": true/false, \"reason\": \"...\", \"violations\": []}";

pub fn is_llm_available() -> bool {
    host_storage_get(b"llm_enabled")
        .ok()
        .map(|d| !d.is_empty() && d[0] == 1)
        .unwrap_or(false)
}

pub fn select_reviewers(validators_json: &[u8], submission_hash: &[u8], offset: u8) -> Vec<String> {
    let validators: Vec<String> = match bincode::deserialize(validators_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    if validators.len() < 3 {
        return validators;
    }

    let mut seed = [0u8; 32];
    let _ = host_random_seed(&mut seed);
    for (i, b) in submission_hash.iter().enumerate() {
        if i < 32 {
            seed[i] ^= b;
        }
    }
    if !seed.is_empty() {
        seed[0] = seed[0].wrapping_add(offset);
    }

    let n = validators.len();
    let mut selected = Vec::with_capacity(3);
    let mut used = Vec::new();

    for i in 0..3 {
        let idx_bytes = if i * 4 + 4 <= seed.len() {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&seed[i * 4..i * 4 + 4]);
            u32::from_le_bytes(buf) as usize
        } else {
            (seed[i % seed.len()] as usize).wrapping_mul(i + 1)
        };

        let mut idx = idx_bytes % n;
        let mut attempts = 0;
        while used.contains(&idx) && attempts < n {
            idx = (idx + 1) % n;
            attempts += 1;
        }
        if !used.contains(&idx) {
            used.push(idx);
            selected.push(validators[idx].clone());
        }
    }
    selected
}

pub fn run_llm_review(agent_code: &str, llm_url: &str) -> Option<LlmReviewResult> {
    if !is_llm_available() {
        return None;
    }

    let redacted_code = redact_api_keys(agent_code);

    let mut prompt = String::new();
    let _ = write!(
        prompt,
        "Review the following Python agent code:\n\n```python\n{}\n```\n\nProvide your verdict as JSON: {{\"approved\": true/false, \"reason\": \"...\", \"violations\": []}}",
        redacted_code
    );

    let request = LlmRequest {
        model: String::from(DEFAULT_LLM_MODEL),
        messages: alloc::vec![
            LlmMessage {
                role: String::from("system"),
                content: String::from(DEFAULT_SYSTEM_PROMPT),
            },
            LlmMessage {
                role: String::from("user"),
                content: prompt,
            },
        ],
        max_tokens: 2048,
        temperature: 0.1,
    };

    let request_bytes = bincode::serialize(&request).ok()?;
    let response_bytes = host_http_post(llm_url.as_bytes(), &request_bytes).ok()?;
    let response: LlmResponse = bincode::deserialize(&response_bytes).ok()?;

    parse_llm_verdict(&response.content)
}

fn parse_llm_verdict(content: &str) -> Option<LlmReviewResult> {
    let json_start = content.find('{')?;
    let json_end = content.rfind('}')? + 1;
    if json_start >= json_end {
        return None;
    }
    let json_str = &content[json_start..json_end];

    let approved =
        json_str.contains("\"approved\": true") || json_str.contains("\"approved\":true");

    let reason = extract_json_string(json_str, "reason").unwrap_or_default();

    Some(LlmReviewResult {
        approved,
        reason,
        violations: Vec::new(),
        reviewer_validators: Vec::new(),
        scores: Vec::new(),
    })
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let mut search = String::from("\"");
    search.push_str(key);
    search.push_str("\": \"");
    let start = json.find(search.as_str())? + search.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(String::from(&rest[..end]))
}

fn redact_api_keys(code: &str) -> String {
    let mut result = String::from(code);
    if result.len() > 50_000 {
        result.truncate(50_000);
        result.push_str("\n... [truncated]");
    }
    result
}

pub fn store_review_result(submission_id: &str, result: &LlmReviewResult) -> bool {
    let mut key = Vec::from(b"llm_review:" as &[u8]);
    key.extend_from_slice(submission_id.as_bytes());
    if let Ok(data) = bincode::serialize(result) {
        return host_storage_set(&key, &data).is_ok();
    }
    false
}

pub fn get_review_result(submission_id: &str) -> Option<LlmReviewResult> {
    let mut key = Vec::from(b"llm_review:" as &[u8]);
    key.extend_from_slice(submission_id.as_bytes());
    let data = host_storage_get(&key).ok()?;
    if data.is_empty() {
        return None;
    }
    bincode::deserialize(&data).ok()
}

pub fn aggregate_reviews(results: &[LlmReviewResult]) -> LlmReviewResult {
    let approved_count = results.iter().filter(|r| r.approved).count();
    let total = results.len();
    let approved = total > 0 && approved_count * 2 > total;

    let mut all_violations = Vec::new();
    let mut all_validators = Vec::new();
    let mut all_scores = Vec::new();
    let mut reason = String::new();

    for r in results {
        all_violations.extend(r.violations.iter().cloned());
        all_validators.extend(r.reviewer_validators.iter().cloned());
        all_scores.extend(r.scores.iter().copied());
        if !r.reason.is_empty() && reason.is_empty() {
            reason = r.reason.clone();
        }
    }

    LlmReviewResult {
        approved,
        reason,
        violations: all_violations,
        reviewer_validators: all_validators,
        scores: all_scores,
    }
}
