use platform_challenge_sdk::routes::{ChallengeRoute, RouteRequest, RouteResponse};
use platform_challenge_sdk::server::ChallengeContext;
use serde_json::json;

use crate::types::{LlmReviewResult, TimeoutConfig, WhitelistConfig};
use crate::{
    agent_storage, ast_validation, dataset, llm_review, scoring, submission, timeout_handler,
};

pub fn challenge_routes() -> Vec<ChallengeRoute> {
    vec![
        ChallengeRoute::get("/leaderboard", "Get current leaderboard"),
        ChallengeRoute::get("/stats", "Get challenge statistics"),
        ChallengeRoute::get("/decay", "Get top agent decay status"),
        ChallengeRoute::get("/dataset/history", "Get dataset selection history"),
        ChallengeRoute::get("/dataset/consensus", "Check dataset consensus"),
        ChallengeRoute::get("/timeout/config", "Get timeout configuration"),
        ChallengeRoute::get("/whitelist/config", "Get AST whitelist configuration"),
        ChallengeRoute::get("/review/:id", "Get LLM review result"),
        ChallengeRoute::get("/ast/:id", "Get AST validation result"),
        ChallengeRoute::get("/submission/:name", "Get submission by name"),
        ChallengeRoute::get("/agent/:hotkey/journey", "Get evaluation status"),
        ChallengeRoute::get("/agent/:hotkey/logs", "Get agent logs"),
        ChallengeRoute::get("/agent/:hotkey/code", "Get agent code"),
        ChallengeRoute::post("/timeout/config", "Set timeout configuration").with_auth(),
        ChallengeRoute::post("/whitelist/config", "Set AST whitelist configuration").with_auth(),
        ChallengeRoute::post("/dataset/propose", "Propose dataset task indices").with_auth(),
        ChallengeRoute::post("/dataset/random", "Generate random task indices").with_auth(),
        ChallengeRoute::post("/review/select", "Select review validators").with_auth(),
        ChallengeRoute::post("/review/aggregate", "Aggregate review results").with_auth(),
        ChallengeRoute::post("/timeout/record", "Record review assignment").with_auth(),
        ChallengeRoute::post("/timeout/check", "Check assignment timeout").with_auth(),
        ChallengeRoute::post("/timeout/replace", "Select replacement validator").with_auth(),
        ChallengeRoute::post("/timeout/mark", "Mark assignment as timed out").with_auth(),
    ]
}

pub async fn handle_route(ctx: &ChallengeContext, request: RouteRequest) -> RouteResponse {
    let method = request.method.as_str();
    let path = request.path.as_str();

    match (method, path) {
        ("GET", "/leaderboard") => handle_leaderboard(ctx),
        ("GET", "/stats") => handle_stats(ctx),
        ("GET", "/decay") => handle_decay(ctx),
        ("GET", "/dataset/history") => handle_dataset_history(ctx),
        ("GET", "/dataset/consensus") => handle_dataset_consensus(ctx),
        ("GET", "/timeout/config") => handle_get_timeout_config(ctx),
        ("GET", "/whitelist/config") => handle_get_whitelist_config(ctx),
        ("GET", p) if p.starts_with("/review/") => handle_review(ctx, &request),
        ("GET", p) if p.starts_with("/ast/") => handle_ast(ctx, &request),
        ("GET", p) if p.starts_with("/submission/") => handle_submission_by_name(ctx, &request),
        ("GET", p) if p.starts_with("/agent/") && p.ends_with("/journey") => {
            handle_journey(ctx, &request)
        }
        ("GET", p) if p.starts_with("/agent/") && p.ends_with("/logs") => {
            handle_logs(ctx, &request)
        }
        ("GET", p) if p.starts_with("/agent/") && p.ends_with("/code") => {
            handle_code(ctx, &request)
        }
        ("POST", "/timeout/config") => handle_set_timeout_config(ctx, &request),
        ("POST", "/whitelist/config") => handle_set_whitelist_config(ctx, &request),
        ("POST", "/dataset/propose") => handle_dataset_propose(ctx, &request),
        ("POST", "/dataset/random") => handle_dataset_random(ctx, &request),
        ("POST", "/review/select") => handle_review_select(&request),
        ("POST", "/review/aggregate") => handle_review_aggregate(&request),
        ("POST", "/timeout/record") => handle_timeout_record(ctx, &request),
        ("POST", "/timeout/check") => handle_timeout_check(ctx, &request),
        ("POST", "/timeout/replace") => handle_timeout_replace(&request),
        ("POST", "/timeout/mark") => handle_timeout_mark(ctx, &request),
        _ => RouteResponse::not_found(),
    }
}

fn handle_leaderboard(ctx: &ChallengeContext) -> RouteResponse {
    let entries: Vec<crate::types::LeaderboardEntry> = ctx
        .db
        .kv_get("leaderboard")
        .ok()
        .flatten()
        .unwrap_or_default();
    RouteResponse::json(&entries)
}

fn handle_stats(ctx: &ChallengeContext) -> RouteResponse {
    let active_miners: u64 = ctx
        .db
        .kv_get("active_miner_count")
        .ok()
        .flatten()
        .unwrap_or(0);
    let validator_count: u64 = ctx.db.kv_get("validator_count").ok().flatten().unwrap_or(0);

    let stats = crate::types::StatsResponse {
        total_submissions: 0,
        active_miners,
        validator_count,
    };
    RouteResponse::json(&stats)
}

fn handle_decay(ctx: &ChallengeContext) -> RouteResponse {
    let state = scoring::get_top_agent_state(&ctx.db);
    RouteResponse::json(&state)
}

fn handle_dataset_history(ctx: &ChallengeContext) -> RouteResponse {
    let history = dataset::get_dataset_history(&ctx.db);
    RouteResponse::json(&history)
}

fn handle_dataset_consensus(ctx: &ChallengeContext) -> RouteResponse {
    let result = dataset::check_dataset_consensus(&ctx.db);
    RouteResponse::json(&result)
}

fn handle_get_timeout_config(ctx: &ChallengeContext) -> RouteResponse {
    let config = timeout_handler::get_timeout_config(&ctx.db);
    RouteResponse::json(&config)
}

fn handle_get_whitelist_config(ctx: &ChallengeContext) -> RouteResponse {
    let config = ast_validation::get_whitelist_config(&ctx.db);
    RouteResponse::json(&config)
}

fn handle_review(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let id = match request.param("id") {
        Some(id) => id,
        None => return RouteResponse::bad_request("Missing id parameter"),
    };
    let result = llm_review::get_review_result(&ctx.db, id);
    RouteResponse::json(&result)
}

fn handle_ast(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let id = match request.param("id") {
        Some(id) => id,
        None => return RouteResponse::bad_request("Missing id parameter"),
    };
    let result = ast_validation::get_ast_result(&ctx.db, id);
    RouteResponse::json(&result)
}

fn handle_submission_by_name(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let name = match request.param("name") {
        Some(name) => name,
        None => return RouteResponse::bad_request("Missing name parameter"),
    };
    let result = submission::get_submission_by_name(&ctx.db, name);
    RouteResponse::json(&result)
}

fn handle_journey(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let hotkey = match request.param("hotkey") {
        Some(hotkey) => hotkey,
        None => return RouteResponse::bad_request("Missing hotkey parameter"),
    };
    let status = agent_storage::get_evaluation_status(&ctx.db, hotkey, ctx.epoch);
    RouteResponse::json(&status)
}

fn handle_logs(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let hotkey = match request.param("hotkey") {
        Some(hotkey) => hotkey,
        None => return RouteResponse::bad_request("Missing hotkey parameter"),
    };
    let logs = agent_storage::get_agent_logs(&ctx.db, hotkey, ctx.epoch);
    RouteResponse::json(&logs)
}

fn handle_code(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let hotkey = match request.param("hotkey") {
        Some(hotkey) => hotkey,
        None => return RouteResponse::bad_request("Missing hotkey parameter"),
    };
    let code = agent_storage::get_agent_code(&ctx.db, hotkey, ctx.epoch);
    match code {
        Some(data) => RouteResponse::ok(json!({ "code": data })),
        None => RouteResponse::not_found(),
    }
}

fn handle_set_timeout_config(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let config: TimeoutConfig = match request.parse_body() {
        Ok(c) => c,
        Err(_) => return RouteResponse::bad_request("Invalid timeout config"),
    };
    let result = timeout_handler::set_timeout_config(&ctx.db, &config);
    RouteResponse::json(result)
}

fn handle_set_whitelist_config(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    let config: WhitelistConfig = match request.parse_body() {
        Ok(c) => c,
        Err(_) => return RouteResponse::bad_request("Invalid whitelist config"),
    };
    let result = ast_validation::set_whitelist_config(&ctx.db, &config);
    RouteResponse::json(result)
}

fn handle_dataset_propose(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct ProposeBody {
        validator_id: String,
        indices: Vec<u32>,
    }
    let body: ProposeBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid propose body"),
    };
    let result = dataset::propose_task_indices(&ctx.db, &body.validator_id, &body.indices);
    RouteResponse::json(result)
}

fn handle_dataset_random(_ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct RandomBody {
        total_tasks: u32,
        select_count: u32,
    }
    let body: RandomBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid random body"),
    };
    let indices = dataset::generate_random_indices(body.total_tasks, body.select_count);
    RouteResponse::json(&indices)
}

fn handle_review_select(request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct SelectBody {
        validators_json: Vec<u8>,
        submission_hash: Vec<u8>,
        offset: u8,
    }
    let body: SelectBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid select body"),
    };
    let reviewers =
        llm_review::select_reviewers(&body.validators_json, &body.submission_hash, body.offset);
    RouteResponse::json(&reviewers)
}

fn handle_review_aggregate(request: &RouteRequest) -> RouteResponse {
    let results: Vec<LlmReviewResult> = match request.parse_body() {
        Ok(r) => r,
        Err(_) => return RouteResponse::bad_request("Invalid aggregate body"),
    };
    let aggregated = llm_review::aggregate_reviews(&results);
    RouteResponse::json(&aggregated)
}

fn handle_timeout_record(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct RecordBody {
        submission_id: String,
        validator: String,
        review_type: String,
    }
    let body: RecordBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid record body"),
    };
    let result = timeout_handler::record_assignment(
        &ctx.db,
        &body.submission_id,
        &body.validator,
        &body.review_type,
    );
    RouteResponse::json(result)
}

fn handle_timeout_check(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct CheckBody {
        submission_id: String,
        validator: String,
        review_type: String,
        timeout_blocks: u64,
    }
    let body: CheckBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid check body"),
    };
    let timed_out = timeout_handler::check_timeout(
        &ctx.db,
        &body.submission_id,
        &body.validator,
        &body.review_type,
        body.timeout_blocks,
    );
    RouteResponse::json(timed_out)
}

fn handle_timeout_replace(request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct ReplaceBody {
        validators: Vec<String>,
        excluded: Vec<String>,
        seed: Vec<u8>,
    }
    let body: ReplaceBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid replace body"),
    };
    let replacement =
        timeout_handler::select_replacement(&body.validators, &body.excluded, &body.seed);
    RouteResponse::json(&replacement)
}

fn handle_timeout_mark(ctx: &ChallengeContext, request: &RouteRequest) -> RouteResponse {
    #[derive(serde::Deserialize)]
    struct MarkBody {
        submission_id: String,
        validator: String,
        review_type: String,
    }
    let body: MarkBody = match request.parse_body() {
        Ok(b) => b,
        Err(_) => return RouteResponse::bad_request("Invalid mark body"),
    };
    let result = timeout_handler::mark_timed_out(
        &ctx.db,
        &body.submission_id,
        &body.validator,
        &body.review_type,
    );
    RouteResponse::json(result)
}
