use platform_challenge_sdk::ChallengeDatabase;

use crate::types::{AstValidationResult, WhitelistConfig};

pub fn get_whitelist_config(db: &ChallengeDatabase) -> WhitelistConfig {
    db.kv_get::<WhitelistConfig>("whitelist_config")
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn set_whitelist_config(db: &ChallengeDatabase, config: &WhitelistConfig) -> bool {
    db.kv_set("whitelist_config", config).is_ok()
}

pub fn validate_ast(
    db: &ChallengeDatabase,
    submission_id: &str,
    code: &str,
) -> AstValidationResult {
    let config = get_whitelist_config(db);
    let mut violations = Vec::new();
    let mut warnings = Vec::new();

    for line in code.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            let module = extract_import_module(trimmed);
            if !module.is_empty() && !is_import_allowed(&module, &config.allowed_imports) {
                violations.push(format!("Forbidden import: {}", module));
            }
        }

        for builtin in &config.forbidden_builtins {
            if trimmed.contains(&format!("{}(", builtin)) {
                violations.push(format!("Forbidden builtin call: {}", builtin));
            }
        }

        for pattern in &config.forbidden_patterns {
            if trimmed.contains(pattern.as_str()) {
                warnings.push(format!("Suspicious pattern: {}", pattern));
            }
        }
    }

    let passed = violations.is_empty();

    let result = AstValidationResult {
        submission_id: submission_id.to_string(),
        passed,
        violations,
        warnings,
    };

    let key = format!("ast_result:{}", submission_id);
    let _ = db.kv_set(&key, &result);

    result
}

pub fn get_ast_result(db: &ChallengeDatabase, submission_id: &str) -> Option<AstValidationResult> {
    let key = format!("ast_result:{}", submission_id);
    db.kv_get::<AstValidationResult>(&key).ok().flatten()
}

fn extract_import_module(line: &str) -> String {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("from ") {
        rest.split_whitespace()
            .next()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .to_string()
    } else if let Some(rest) = trimmed.strip_prefix("import ") {
        rest.split(',')
            .next()
            .unwrap_or("")
            .trim()
            .split('.')
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    }
}

fn is_import_allowed(module: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|a| a == module)
}
