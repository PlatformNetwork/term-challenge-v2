//! Terminal Benchmark Challenge — WASM evaluation module.
//!
//! This crate implements the challenge evaluation logic as a `no_std` WASM
//! module. It deserialises submissions and challenge parameters via `bincode`,
//! scores each task result, optionally invokes an LLM judge over the host HTTP
//! bridge, and produces an aggregate score.

#![no_std]

extern crate alloc;

mod scoring;
mod tasks;
mod types;

use alloc::string::String;
use alloc::vec::Vec;
use bincode::Options;
use platform_challenge_sdk_wasm::host_functions::host_http_post;
use platform_challenge_sdk_wasm::{Challenge, EvaluationInput, EvaluationOutput};

use crate::scoring::{calculate_aggregate, format_summary, to_weight};
use crate::types::{ChallengeParams, LlmJudgeRequest, LlmJudgeResponse, Submission, TaskResult};

/// Maximum allowed size for a serialised submission (16 MiB).
const MAX_SUBMISSION_SIZE: u64 = 16 * 1024 * 1024;
/// Maximum allowed size for serialised challenge parameters (4 MiB).
const MAX_PARAMS_SIZE: u64 = 4 * 1024 * 1024;
/// Maximum allowed size for a serialised LLM judge response (1 MiB).
const MAX_LLM_RESPONSE_SIZE: u64 = 1024 * 1024;
/// Maximum number of tasks a single submission may contain.
const MAX_TASKS: usize = 256;
/// Maximum byte length for `agent_output` or `test_output` fields (1 MiB).
const MAX_OUTPUT_FIELD_SIZE: usize = 1024 * 1024;
/// Multiplier applied to the 0.0–1.0 weight to produce an integer score
/// (basis-point precision: 1.0 → 10 000).
const SCORE_SCALE_FACTOR: f64 = 10_000.0;
/// Minimum LLM judge score required for a task to remain marked as passed.
const LLM_PASS_THRESHOLD: f64 = 0.5;

/// Bincode options for deserialising submissions with a size limit.
fn bincode_options_submission() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_SUBMISSION_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

/// Bincode options for deserialising challenge parameters with a size limit.
fn bincode_options_params() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_PARAMS_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

/// Bincode options for deserialising LLM judge responses with a size limit.
fn bincode_options_llm() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_LLM_RESPONSE_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

/// Validates a single task result for well-formedness.
///
/// Returns `false` if the task ID is empty, the score is non-finite or out of
/// range, or either output field exceeds [`MAX_OUTPUT_FIELD_SIZE`].
fn validate_task_result(result: &TaskResult) -> bool {
    if result.task_id.is_empty() {
        return false;
    }
    if !result.score.is_finite() || !(0.0..=1.0).contains(&result.score) {
        return false;
    }
    if result.agent_output.len() > MAX_OUTPUT_FIELD_SIZE {
        return false;
    }
    if result.test_output.len() > MAX_OUTPUT_FIELD_SIZE {
        return false;
    }
    true
}

/// Deserialises and validates a submission and its challenge parameters.
///
/// Shared by both [`TermChallengeWasm::evaluate`] and
/// [`TermChallengeWasm::validate`] to avoid duplicating deserialisation and
/// validation logic.
fn deserialize_and_validate(
    input: &EvaluationInput,
) -> Result<(Submission, ChallengeParams), &'static str> {
    let submission: Submission = bincode_options_submission()
        .deserialize(&input.agent_data)
        .map_err(|_| "failed to deserialize submission")?;

    let params: ChallengeParams = bincode_options_params()
        .deserialize(&input.params)
        .map_err(|_| "failed to deserialize challenge params")?;

    if submission.agent_hash.is_empty() || submission.miner_hotkey.is_empty() {
        return Err("missing agent_hash or miner_hotkey");
    }

    if submission.task_results.is_empty() {
        return Err("submission contains no task results");
    }

    if submission.task_results.len() > MAX_TASKS {
        return Err("submission exceeds maximum task count");
    }

    if submission.task_results.len() != params.tasks.len() {
        return Err("task result count does not match task definitions");
    }

    for result in &submission.task_results {
        if !validate_task_result(result) {
            return Err("invalid task result: bad score or empty task_id");
        }
    }

    Ok((submission, params))
}

/// WASM challenge implementation for the Terminal Benchmark.
pub struct TermChallengeWasm;

impl Default for TermChallengeWasm {
    fn default() -> Self {
        Self
    }
}

impl TermChallengeWasm {
    /// Creates a new [`TermChallengeWasm`] instance.
    pub const fn new() -> Self {
        Self
    }

    /// Attempts to call the LLM judge endpoint for a task result.
    ///
    /// Returns `None` on any error (serialisation, HTTP, or invalid response),
    /// allowing the caller to fall back to the original score.
    fn try_llm_judge(url: &str, result: &TaskResult, instruction: &str) -> Option<f64> {
        if !url.starts_with("https://") {
            return None;
        }

        let request = LlmJudgeRequest {
            task_id: result.task_id.clone(),
            instruction: String::from(instruction),
            agent_output: result.agent_output.clone(),
            test_output: result.test_output.clone(),
        };

        let url_bytes = url.as_bytes();
        let body = match bincode::serialize(&request) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let response_bytes = match host_http_post(url_bytes, &body) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let judge_resp: LlmJudgeResponse = match bincode_options_llm().deserialize(&response_bytes)
        {
            Ok(r) => r,
            Err(_) => return None,
        };

        if !judge_resp.score.is_finite() {
            return None;
        }

        Some(judge_resp.score.clamp(0.0, 1.0))
    }
}

impl Challenge for TermChallengeWasm {
    fn name(&self) -> &'static str {
        "term-challenge"
    }

    fn version(&self) -> &'static str {
        "3.0.0"
    }

    fn evaluate(&self, input: EvaluationInput) -> EvaluationOutput {
        let (submission, params) = match deserialize_and_validate(&input) {
            Ok(pair) => pair,
            Err(msg) => return EvaluationOutput::failure(msg),
        };

        let mut results: Vec<TaskResult> = submission.task_results;

        if let Some(ref url) = params.llm_judge_url {
            for (result, task) in results.iter_mut().zip(params.tasks.iter()) {
                if !result.passed {
                    continue;
                }
                if let Some(llm_score) = Self::try_llm_judge(url, result, &task.name) {
                    result.score = llm_score;
                    if llm_score < LLM_PASS_THRESHOLD {
                        result.passed = false;
                    }
                }
            }
        }

        let aggregate = calculate_aggregate(&params.tasks, &results);
        let weight = to_weight(&aggregate);
        let score = (weight * SCORE_SCALE_FACTOR) as i64;
        let message = format_summary(&aggregate);

        EvaluationOutput::success(score, &message)
    }

    fn validate(&self, input: EvaluationInput) -> bool {
        deserialize_and_validate(&input).is_ok()
    }

    fn tasks(&self) -> Vec<u8> {
        let task_defs = tasks::builtin_tasks();
        bincode::serialize(&task_defs).unwrap_or_default()
    }
}

platform_challenge_sdk_wasm::register_challenge!(TermChallengeWasm, TermChallengeWasm::new());
