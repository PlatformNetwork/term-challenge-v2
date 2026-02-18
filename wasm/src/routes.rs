use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use platform_challenge_sdk_wasm::host_functions::{
    host_consensus_get_epoch, host_consensus_get_submission_count, host_storage_get,
};

use crate::types::{
    LeaderboardEntry, RouteDefinition, StatsResponse, TimeoutConfig, TopAgentState,
    WasmRouteRequest, WhitelistConfig,
};
use crate::{
    agent_storage, ast_validation, dataset, llm_review, scoring, submission, timeout_handler,
};

pub fn get_route_definitions() -> Vec<RouteDefinition> {
    vec![
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/leaderboard"),
            description: String::from(
                "Returns current leaderboard with scores, miner hotkeys, and ranks",
            ),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions"),
            description: String::from("Returns pending submissions awaiting evaluation"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions/:id"),
            description: String::from("Returns specific submission status"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset"),
            description: String::from("Returns current active dataset of 50 SWE-bench tasks"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset/history"),
            description: String::from("Returns historical dataset selections"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/submit"),
            description: String::from("Submission endpoint: receives zip package and metadata"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/decay"),
            description: String::from("Returns current decay status for top agents"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/stats"),
            description: String::from("Challenge statistics: total submissions, active miners"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/code"),
            description: String::from("Returns stored agent code package for a miner"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/logs"),
            description: String::from("Returns evaluation logs for a miner"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/journey"),
            description: String::from("Returns evaluation status journey for a miner"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/review/:id"),
            description: String::from("Returns LLM review result for a submission"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/ast/:id"),
            description: String::from("Returns AST validation result for a submission"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/submission/:name"),
            description: String::from("Returns submission info by name"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/config/timeout"),
            description: String::from("Returns current timeout configuration"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/config/timeout"),
            description: String::from("Updates timeout configuration"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/config/whitelist"),
            description: String::from("Returns current AST whitelist configuration"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/config/whitelist"),
            description: String::from("Updates AST whitelist configuration"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/dataset/propose"),
            description: String::from("Propose task indices for dataset consensus"),
        },
        RouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset/consensus"),
            description: String::from("Check dataset consensus status"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/review/select"),
            description: String::from("Select reviewers for a submission"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/review/aggregate"),
            description: String::from("Aggregate multiple review results"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/timeout/record"),
            description: String::from("Record a review assignment for timeout tracking"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/timeout/check"),
            description: String::from("Check if a review assignment has timed out"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/dataset/random"),
            description: String::from("Generate random task indices"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/timeout/replace"),
            description: String::from("Select a replacement validator for a timed-out review"),
        },
        RouteDefinition {
            method: String::from("POST"),
            path: String::from("/timeout/mark"),
            description: String::from("Mark a review assignment as timed out"),
        },
    ]
}

pub fn handle_route_request(request: &WasmRouteRequest) -> Vec<u8> {
    let path = request.path.as_str();
    let method = request.method.as_str();

    match (method, path) {
        ("GET", "/leaderboard") => handle_leaderboard(),
        ("GET", "/stats") => handle_stats(),
        ("GET", "/decay") => handle_decay(),
        ("GET", "/dataset/history") => handle_dataset_history(),
        ("GET", "/dataset/consensus") => handle_dataset_consensus(),
        ("GET", "/config/timeout") => handle_get_timeout_config(),
        ("GET", "/config/whitelist") => handle_get_whitelist_config(),
        ("POST", "/config/timeout") => handle_set_timeout_config(&request.body),
        ("POST", "/config/whitelist") => handle_set_whitelist_config(&request.body),
        ("POST", "/dataset/propose") => handle_dataset_propose(&request.body),
        ("POST", "/dataset/random") => handle_dataset_random(&request.body),
        ("POST", "/review/select") => handle_review_select(&request.body),
        ("POST", "/review/aggregate") => handle_review_aggregate(&request.body),
        ("POST", "/timeout/record") => handle_timeout_record(&request.body),
        ("POST", "/timeout/check") => handle_timeout_check(&request.body),
        ("POST", "/timeout/replace") => handle_timeout_replace(&request.body),
        ("POST", "/timeout/mark") => handle_timeout_mark(&request.body),
        _ => {
            if method == "GET" {
                if let Some(id) = path.strip_prefix("/review/") {
                    return handle_review(id);
                }
                if let Some(id) = path.strip_prefix("/ast/") {
                    return handle_ast(id);
                }
                if let Some(name) = path.strip_prefix("/submission/") {
                    return handle_submission_by_name(name);
                }
                if let Some(rest) = path.strip_prefix("/agent/") {
                    if let Some(hotkey) = rest.strip_suffix("/journey") {
                        return handle_journey(hotkey);
                    }
                    if let Some(hotkey) = rest.strip_suffix("/logs") {
                        return handle_logs(hotkey);
                    }
                    if let Some(hotkey) = rest.strip_suffix("/code") {
                        return handle_code(hotkey);
                    }
                }
            }
            Vec::new()
        }
    }
}

fn handle_leaderboard() -> Vec<u8> {
    let entries: Vec<LeaderboardEntry> = host_storage_get(b"leaderboard")
        .ok()
        .and_then(|d| {
            if d.is_empty() {
                None
            } else {
                bincode::deserialize(&d).ok()
            }
        })
        .unwrap_or_default();
    bincode::serialize(&entries).unwrap_or_default()
}

fn handle_stats() -> Vec<u8> {
    let total_submissions = host_consensus_get_submission_count() as u64;
    let active_miners = host_storage_get(b"active_miner_count")
        .ok()
        .and_then(|d| {
            if d.len() >= 8 {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&d[..8]);
                Some(u64::from_le_bytes(buf))
            } else {
                None
            }
        })
        .unwrap_or(0);
    let validator_count = host_storage_get(b"validator_count")
        .ok()
        .and_then(|d| {
            if d.len() >= 8 {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&d[..8]);
                Some(u64::from_le_bytes(buf))
            } else {
                None
            }
        })
        .unwrap_or(0);

    let stats = StatsResponse {
        total_submissions,
        active_miners,
        validator_count,
    };
    bincode::serialize(&stats).unwrap_or_default()
}

fn handle_decay() -> Vec<u8> {
    let state: Option<TopAgentState> = scoring::get_top_agent_state();
    bincode::serialize(&state).unwrap_or_default()
}

fn handle_dataset_history() -> Vec<u8> {
    let history = dataset::get_dataset_history();
    bincode::serialize(&history).unwrap_or_default()
}

fn handle_review(id: &str) -> Vec<u8> {
    let result = llm_review::get_review_result(id);
    bincode::serialize(&result).unwrap_or_default()
}

fn handle_ast(id: &str) -> Vec<u8> {
    let result = ast_validation::get_ast_result(id);
    bincode::serialize(&result).unwrap_or_default()
}

fn handle_submission_by_name(name: &str) -> Vec<u8> {
    let result = submission::get_submission_by_name(name);
    bincode::serialize(&result).unwrap_or_default()
}

fn handle_journey(hotkey: &str) -> Vec<u8> {
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    let status = agent_storage::get_evaluation_status(hotkey, current_epoch);
    bincode::serialize(&status).unwrap_or_default()
}

fn handle_logs(hotkey: &str) -> Vec<u8> {
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    let logs = agent_storage::get_agent_logs(hotkey, current_epoch);
    bincode::serialize(&logs).unwrap_or_default()
}

fn handle_code(hotkey: &str) -> Vec<u8> {
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    agent_storage::get_agent_code(hotkey, current_epoch).unwrap_or_default()
}

fn handle_get_timeout_config() -> Vec<u8> {
    let config = timeout_handler::get_timeout_config();
    bincode::serialize(&config).unwrap_or_default()
}

fn handle_set_timeout_config(body: &[u8]) -> Vec<u8> {
    if let Ok(config) = bincode::deserialize::<TimeoutConfig>(body) {
        let ok = timeout_handler::set_timeout_config(&config);
        bincode::serialize(&ok).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}

fn handle_get_whitelist_config() -> Vec<u8> {
    let config = ast_validation::get_whitelist_config();
    bincode::serialize(&config).unwrap_or_default()
}

fn handle_set_whitelist_config(body: &[u8]) -> Vec<u8> {
    if let Ok(config) = bincode::deserialize::<WhitelistConfig>(body) {
        let ok = ast_validation::set_whitelist_config(&config);
        bincode::serialize(&ok).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}

fn handle_dataset_consensus() -> Vec<u8> {
    let result = dataset::check_dataset_consensus();
    bincode::serialize(&result).unwrap_or_default()
}

fn handle_dataset_propose(body: &[u8]) -> Vec<u8> {
    if let Ok((validator_id, indices)) = bincode::deserialize::<(String, Vec<u32>)>(body) {
        let ok = dataset::propose_task_indices(&validator_id, &indices);
        bincode::serialize(&ok).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}

fn handle_dataset_random(body: &[u8]) -> Vec<u8> {
    if let Ok((total_tasks, select_count)) = bincode::deserialize::<(u32, u32)>(body) {
        let indices = dataset::generate_random_indices(total_tasks, select_count);
        bincode::serialize(&indices).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn handle_review_select(body: &[u8]) -> Vec<u8> {
    if let Ok((validators_json, submission_hash, offset)) =
        bincode::deserialize::<(Vec<u8>, Vec<u8>, u8)>(body)
    {
        let reviewers = llm_review::select_reviewers(&validators_json, &submission_hash, offset);
        bincode::serialize(&reviewers).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn handle_review_aggregate(body: &[u8]) -> Vec<u8> {
    if let Ok(results) = bincode::deserialize::<Vec<crate::types::LlmReviewResult>>(body) {
        let aggregated = llm_review::aggregate_reviews(&results);
        bincode::serialize(&aggregated).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn handle_timeout_record(body: &[u8]) -> Vec<u8> {
    if let Ok((submission_id, validator, review_type)) =
        bincode::deserialize::<(String, String, String)>(body)
    {
        let ok = timeout_handler::record_assignment(&submission_id, &validator, &review_type);
        bincode::serialize(&ok).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}

fn handle_timeout_check(body: &[u8]) -> Vec<u8> {
    if let Ok((submission_id, validator, review_type, timeout_ms)) =
        bincode::deserialize::<(String, String, String, u64)>(body)
    {
        let timed_out =
            timeout_handler::check_timeout(&submission_id, &validator, &review_type, timeout_ms);
        bincode::serialize(&timed_out).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}

fn handle_timeout_replace(body: &[u8]) -> Vec<u8> {
    if let Ok((validators, excluded, seed)) =
        bincode::deserialize::<(Vec<String>, Vec<String>, Vec<u8>)>(body)
    {
        let replacement = timeout_handler::select_replacement(&validators, &excluded, &seed);
        bincode::serialize(&replacement).unwrap_or_default()
    } else {
        bincode::serialize(&Option::<String>::None).unwrap_or_default()
    }
}

fn handle_timeout_mark(body: &[u8]) -> Vec<u8> {
    if let Ok((submission_id, validator, review_type)) =
        bincode::deserialize::<(String, String, String)>(body)
    {
        let ok = timeout_handler::mark_timed_out(&submission_id, &validator, &review_type);
        bincode::serialize(&ok).unwrap_or_default()
    } else {
        bincode::serialize(&false).unwrap_or_default()
    }
}
