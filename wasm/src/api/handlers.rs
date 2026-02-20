use alloc::string::String;
use alloc::vec::Vec;
use bincode::Options;
use platform_challenge_sdk_wasm::host_functions::{
    host_consensus_get_epoch, host_consensus_get_submission_count, host_storage_get,
};
use platform_challenge_sdk_wasm::{WasmRouteRequest, WasmRouteResponse};

use crate::types::{
    LeaderboardEntry, StatsResponse, TimeoutConfig, TopAgentState, WhitelistConfig,
};
use crate::{
    agent_storage, ast_validation, dataset, llm_review, scoring, submission, timeout_handler,
};

const MAX_ROUTE_BODY_SIZE: usize = 1_048_576;

fn bincode_options_route_body() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_ROUTE_BODY_SIZE as u64)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

fn ok_response(body: Vec<u8>) -> WasmRouteResponse {
    WasmRouteResponse { status: 200, body }
}

fn unauthorized_response() -> WasmRouteResponse {
    WasmRouteResponse {
        status: 401,
        body: bincode::serialize(&false).unwrap_or_default(),
    }
}

fn bad_request_response() -> WasmRouteResponse {
    WasmRouteResponse {
        status: 400,
        body: bincode::serialize(&false).unwrap_or_default(),
    }
}

fn empty_response() -> WasmRouteResponse {
    WasmRouteResponse {
        status: 200,
        body: Vec::new(),
    }
}

fn is_authenticated(request: &WasmRouteRequest) -> bool {
    request
        .auth_hotkey
        .as_ref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
}

fn get_param<'a>(request: &'a WasmRouteRequest, name: &str) -> Option<&'a str> {
    request
        .params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

pub fn handle_leaderboard(_request: &WasmRouteRequest) -> WasmRouteResponse {
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
    ok_response(bincode::serialize(&entries).unwrap_or_default())
}

pub fn handle_stats(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let total_submissions = host_consensus_get_submission_count() as u64;
    let epoch = host_consensus_get_epoch();
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
    let _ = epoch;
    ok_response(bincode::serialize(&stats).unwrap_or_default())
}

pub fn handle_decay(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let state: Option<TopAgentState> = scoring::get_top_agent_state();
    ok_response(bincode::serialize(&state).unwrap_or_default())
}

pub fn handle_dataset_history(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let history = dataset::get_dataset_history();
    ok_response(bincode::serialize(&history).unwrap_or_default())
}

pub fn handle_dataset_consensus(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let result = dataset::check_dataset_consensus();
    ok_response(bincode::serialize(&result).unwrap_or_default())
}

pub fn handle_get_timeout_config(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let config = timeout_handler::get_timeout_config();
    ok_response(bincode::serialize(&config).unwrap_or_default())
}

pub fn handle_get_whitelist_config(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let config = ast_validation::get_whitelist_config();
    ok_response(bincode::serialize(&config).unwrap_or_default())
}

pub fn handle_review(request: &WasmRouteRequest) -> WasmRouteResponse {
    let id = match get_param(request, "id") {
        Some(id) => id,
        None => return bad_request_response(),
    };
    let result = llm_review::get_review_result(id);
    ok_response(bincode::serialize(&result).unwrap_or_default())
}

pub fn handle_ast(request: &WasmRouteRequest) -> WasmRouteResponse {
    let id = match get_param(request, "id") {
        Some(id) => id,
        None => return bad_request_response(),
    };
    let result = ast_validation::get_ast_result(id);
    ok_response(bincode::serialize(&result).unwrap_or_default())
}

pub fn handle_submission_by_name(request: &WasmRouteRequest) -> WasmRouteResponse {
    let name = match get_param(request, "name") {
        Some(name) => name,
        None => return bad_request_response(),
    };
    let result = submission::get_submission_by_name(name);
    ok_response(bincode::serialize(&result).unwrap_or_default())
}

pub fn handle_journey(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(hotkey) => hotkey,
        None => return bad_request_response(),
    };
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    let status = agent_storage::get_evaluation_status(hotkey, current_epoch);
    ok_response(bincode::serialize(&status).unwrap_or_default())
}

pub fn handle_logs(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(hotkey) => hotkey,
        None => return bad_request_response(),
    };
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    let logs = agent_storage::get_agent_logs(hotkey, current_epoch);
    ok_response(bincode::serialize(&logs).unwrap_or_default())
}

pub fn handle_code(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(hotkey) => hotkey,
        None => return bad_request_response(),
    };
    let epoch = host_consensus_get_epoch();
    let current_epoch = if epoch >= 0 { epoch as u64 } else { 0 };
    let body = agent_storage::get_agent_code(hotkey, current_epoch).unwrap_or_default();
    ok_response(body)
}

pub fn handle_set_timeout_config(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok(config) = bincode_options_route_body().deserialize::<TimeoutConfig>(&request.body) {
        let result = timeout_handler::set_timeout_config(&config);
        ok_response(bincode::serialize(&result).unwrap_or_default())
    } else {
        bad_request_response()
    }
}

pub fn handle_set_whitelist_config(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok(config) = bincode_options_route_body().deserialize::<WhitelistConfig>(&request.body) {
        let result = ast_validation::set_whitelist_config(&config);
        ok_response(bincode::serialize(&result).unwrap_or_default())
    } else {
        bad_request_response()
    }
}

pub fn handle_dataset_propose(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok((validator_id, indices)) =
        bincode_options_route_body().deserialize::<(String, Vec<u32>)>(&request.body)
    {
        let result = dataset::propose_task_indices(&validator_id, &indices);
        ok_response(bincode::serialize(&result).unwrap_or_default())
    } else {
        bad_request_response()
    }
}

pub fn handle_dataset_random(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return empty_response();
    }
    if let Ok((total_tasks, select_count)) =
        bincode_options_route_body().deserialize::<(u32, u32)>(&request.body)
    {
        let indices = dataset::generate_random_indices(total_tasks, select_count);
        ok_response(bincode::serialize(&indices).unwrap_or_default())
    } else {
        empty_response()
    }
}

pub fn handle_review_select(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return empty_response();
    }
    if let Ok((validators_json, submission_hash, offset)) =
        bincode_options_route_body().deserialize::<(Vec<u8>, Vec<u8>, u8)>(&request.body)
    {
        let reviewers = llm_review::select_reviewers(&validators_json, &submission_hash, offset);
        ok_response(bincode::serialize(&reviewers).unwrap_or_default())
    } else {
        empty_response()
    }
}

pub fn handle_review_aggregate(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return empty_response();
    }
    if let Ok(results) =
        bincode_options_route_body().deserialize::<Vec<crate::types::LlmReviewResult>>(&request.body)
    {
        let aggregated = llm_review::aggregate_reviews(&results);
        ok_response(bincode::serialize(&aggregated).unwrap_or_default())
    } else {
        empty_response()
    }
}

pub fn handle_timeout_record(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok((submission_id, validator, review_type)) =
        bincode_options_route_body().deserialize::<(String, String, String)>(&request.body)
    {
        let result = timeout_handler::record_assignment(&submission_id, &validator, &review_type);
        ok_response(bincode::serialize(&result).unwrap_or_default())
    } else {
        bad_request_response()
    }
}

pub fn handle_timeout_check(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok((submission_id, validator, review_type, timeout_ms)) =
        bincode_options_route_body()
            .deserialize::<(String, String, String, u64)>(&request.body)
    {
        let timed_out =
            timeout_handler::check_timeout(&submission_id, &validator, &review_type, timeout_ms);
        ok_response(bincode::serialize(&timed_out).unwrap_or_default())
    } else {
        bad_request_response()
    }
}

pub fn handle_timeout_replace(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return ok_response(
            bincode::serialize(&Option::<String>::None).unwrap_or_default(),
        );
    }
    if let Ok((validators, excluded, seed)) =
        bincode_options_route_body()
            .deserialize::<(Vec<String>, Vec<String>, Vec<u8>)>(&request.body)
    {
        let replacement = timeout_handler::select_replacement(&validators, &excluded, &seed);
        ok_response(bincode::serialize(&replacement).unwrap_or_default())
    } else {
        ok_response(bincode::serialize(&Option::<String>::None).unwrap_or_default())
    }
}

pub fn handle_timeout_mark(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }
    if request.body.len() > MAX_ROUTE_BODY_SIZE {
        return bad_request_response();
    }
    if let Ok((submission_id, validator, review_type)) =
        bincode_options_route_body().deserialize::<(String, String, String)>(&request.body)
    {
        let result = timeout_handler::mark_timed_out(&submission_id, &validator, &review_type);
        ok_response(bincode::serialize(&result).unwrap_or_default())
    } else {
        bad_request_response()
    }
}
