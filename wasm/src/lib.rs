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

const MAX_SUBMISSION_SIZE: u64 = 16 * 1024 * 1024;
const MAX_PARAMS_SIZE: u64 = 4 * 1024 * 1024;
const MAX_LLM_RESPONSE_SIZE: u64 = 1024 * 1024;
const MAX_TASKS: usize = 256;
const MAX_OUTPUT_FIELD_SIZE: usize = 1024 * 1024;

fn bincode_options_submission() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_SUBMISSION_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

fn bincode_options_params() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_PARAMS_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

fn bincode_options_llm() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_LLM_RESPONSE_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

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

pub struct TermChallengeWasm;

impl Default for TermChallengeWasm {
    fn default() -> Self {
        Self
    }
}

impl TermChallengeWasm {
    pub const fn new() -> Self {
        Self
    }

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
        let submission: Submission =
            match bincode_options_submission().deserialize(&input.agent_data) {
                Ok(s) => s,
                Err(_) => return EvaluationOutput::failure("failed to deserialize submission"),
            };

        let params: ChallengeParams = match bincode_options_params().deserialize(&input.params) {
            Ok(p) => p,
            Err(_) => return EvaluationOutput::failure("failed to deserialize challenge params"),
        };

        if submission.agent_hash.is_empty() || submission.miner_hotkey.is_empty() {
            return EvaluationOutput::failure("missing agent_hash or miner_hotkey");
        }

        if submission.task_results.is_empty() {
            return EvaluationOutput::failure("submission contains no task results");
        }

        if submission.task_results.len() > MAX_TASKS {
            return EvaluationOutput::failure("submission exceeds maximum task count");
        }

        if submission.task_results.len() != params.tasks.len() {
            return EvaluationOutput::failure("task result count does not match task definitions");
        }

        for result in &submission.task_results {
            if !validate_task_result(result) {
                return EvaluationOutput::failure(
                    "invalid task result: bad score or empty task_id",
                );
            }
        }

        let mut results: Vec<TaskResult> = submission.task_results;

        if let Some(ref url) = params.llm_judge_url {
            for (result, task) in results.iter_mut().zip(params.tasks.iter()) {
                if !result.passed {
                    continue;
                }
                if let Some(llm_score) = Self::try_llm_judge(url, result, &task.name) {
                    result.score = llm_score;
                    if llm_score < 0.5 {
                        result.passed = false;
                    }
                }
            }
        }

        let aggregate = calculate_aggregate(&params.tasks, &results);
        let weight = to_weight(&aggregate);
        let score = (weight * 10_000.0) as i64;
        let message = format_summary(&aggregate);

        EvaluationOutput::success(score, &message)
    }

    fn validate(&self, input: EvaluationInput) -> bool {
        let submission: Submission =
            match bincode_options_submission().deserialize(&input.agent_data) {
                Ok(s) => s,
                Err(_) => return false,
            };

        let params: ChallengeParams = match bincode_options_params().deserialize(&input.params) {
            Ok(p) => p,
            Err(_) => return false,
        };

        if submission.agent_hash.is_empty() || submission.miner_hotkey.is_empty() {
            return false;
        }

        if submission.task_results.is_empty() {
            return false;
        }

        if submission.task_results.len() > MAX_TASKS {
            return false;
        }

        if submission.task_results.len() != params.tasks.len() {
            return false;
        }

        for result in &submission.task_results {
            if !validate_task_result(result) {
                return false;
            }
        }

        true
    }

    fn tasks(&self) -> Vec<u8> {
        let task_defs = tasks::builtin_tasks();
        bincode::serialize(&task_defs).unwrap_or_default()
    }
}

platform_challenge_sdk_wasm::register_challenge!(TermChallengeWasm, TermChallengeWasm::new());
