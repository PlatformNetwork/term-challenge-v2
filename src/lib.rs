#![allow(dead_code, unused_variables, unused_imports)]
//! Terminal Benchmark Challenge for Platform Network
//!
//! This challenge evaluates AI agents on terminal-based tasks.
//! Agents are run in Docker containers and scored based on task completion.
//!
//! ## Module Structure
//!
//! The crate is organized into thematic modules:
//! - `core/`: Fundamental types (Hotkey, ChallengeId, TaskResult)
//! - `crypto/`: Authentication and encryption
//! - `util/`: Shared utilities (timestamp, hash, encoding)
//! - `storage/`: Data persistence (local, postgres, chain)
//! - `cache/`: Caching systems
//! - `client/`: HTTP and WebSocket clients
//! - `chain/`: Blockchain integration
//! - `weights/`: Weight calculation and emission
//! - `evaluation/`: Evaluation pipeline
//! - `validation/`: Code validation
//! - `worker/`: Background workers
//! - `container/`: Docker management
//! - `task/`: Task definitions
//! - `agent/`: Agent management
//! - `admin/`: Administration
//! - `server/`: Challenge server
//! - `api/`: REST API
//! - `bench/`: Benchmarking framework

// ============================================================================
// NEW MODULAR STRUCTURE
// ============================================================================

/// Shared utility functions
pub mod util;

/// Core types and traits
pub mod core;

/// Cryptographic utilities (auth, x25519, ss58, api_key)
pub mod crypto;

/// Data persistence layer
pub mod storage;

/// Caching systems
pub mod cache;

/// HTTP and WebSocket clients
pub mod client;

/// Blockchain integration (block_sync, epoch, evaluation)
pub mod chain;

/// Weight calculation and emission
pub mod weights;

/// Evaluation pipeline
pub mod evaluation;

/// Code validation
pub mod validation;

/// Background workers
pub mod worker;

/// Container management
pub mod container;

/// Task definitions and registry
pub mod task;

/// Agent management
pub mod agent;

/// Administration (sudo, subnet control)
pub mod admin;

/// Challenge server
pub mod server;

/// REST API
pub mod api;

/// Benchmarking framework
pub mod bench;

// ============================================================================
// LEGACY MODULES (renamed to avoid conflicts, will be removed)
// ============================================================================

#[path = "task_legacy.rs"]
pub mod task_legacy;

#[path = "server_legacy.rs"]
pub mod server_legacy;

#[path = "api_legacy.rs"]
pub mod api_legacy;

// Legacy modules still at root (to be migrated)
pub mod agent_queue;
pub mod agent_registry;
pub mod agent_submission;
pub mod assignment_monitor;
pub mod block_sync;
pub mod blockchain_evaluation;
pub mod central_client;
pub mod chain_storage;
pub mod challenge;
pub mod code_visibility;
pub mod compat;
pub mod compile_worker;
pub mod compiler;
pub mod config;
pub mod container_backend;
pub mod docker;
pub mod emission;
pub mod encrypted_api_key;
pub mod epoch;
pub mod evaluation_orchestrator;
pub mod evaluation_pipeline;
pub mod evaluator;
pub mod llm_client;
pub mod llm_review;
pub mod local_storage;
pub mod metagraph_cache;
pub mod migrations;
pub mod package_validator;
pub mod pg_storage;
pub mod platform_llm;
pub mod platform_ws_client;
pub mod python_whitelist;
pub mod reward_decay;
pub mod scoring;
pub mod subnet_control;
pub mod sudo;
pub mod task_execution;
pub mod task_stream_cache;
pub mod terminal_harness;
pub mod time_decay;
pub mod timeout_retry_monitor;
pub mod validator_distribution;
pub mod validator_worker;
pub mod validator_ws_client;

// ============================================================================
// RE-EXPORTS FROM NEW MODULES
// ============================================================================

// Auth re-exports (from crypto module)
pub mod auth {
    //! Re-exports from crypto::auth for backwards compatibility.
    pub use crate::crypto::auth::*;
}

// x25519 re-exports (from crypto module)
pub mod x25519_encryption {
    //! Re-exports from crypto::x25519 for backwards compatibility.
    pub use crate::crypto::x25519::*;
}

// ============================================================================
// LEGACY RE-EXPORTS (for backwards compatibility)
// ============================================================================

pub use compat::{
    AgentInfo as SdkAgentInfo, ChallengeId, EvaluationResult as SdkEvaluationResult,
    EvaluationsResponseMessage, Hotkey, PartitionStats, WeightAssignment,
};

pub use agent_queue::{
    AgentQueue, EvalRequest, EvalResult, QueueAgentInfo, QueueConfig, QueueStats,
    TaskEvalResult as QueueTaskResult,
};
pub use agent_registry::{AgentEntry, AgentNameEntry, AgentRegistry, AgentStatus, RegistryConfig};
pub use agent_submission::{
    AgentSubmission, AgentSubmissionHandler, SubmissionError, SubmissionStatus,
};
pub use block_sync::{BlockSync, BlockSyncConfig, BlockSyncEvent, NetworkStateResponse};
pub use blockchain_evaluation::{
    AggregatedResult, BlockchainEvaluationManager, EvaluationContract, EvaluationError,
    EvaluationSubmission, MINIMUM_STAKE_RAO, MINIMUM_VALIDATORS, SUCCESS_CODE_PREFIX,
};
pub use chain_storage::{
    allowed_data_keys, ChainStorage, ConsensusResult, Leaderboard as ChainLeaderboard,
    LeaderboardEntry, OnChainEvaluationResult, ValidatorVote,
};
pub use challenge::{create_terminal_bench_challenge, TerminalBenchChallenge};
pub use code_visibility::{
    AgentVisibility, CodeViewResult, CodeVisibilityManager, ValidatorCompletion, VisibilityConfig,
    VisibilityError, VisibilityRequirements, VisibilityStats, VisibilityStatus,
    MIN_EPOCHS_FOR_VISIBILITY, MIN_VALIDATORS_FOR_VISIBILITY,
};
pub use config::{
    ChallengeConfig, EvaluationConfig, ExecutionConfig, ModelWhitelist, ModuleWhitelist,
    PricingConfig,
};
pub use container_backend::{
    create_backend as create_container_backend, is_development_mode, is_secure_mode,
    ContainerBackend, ContainerHandle, ExecOutput, MountConfig, SandboxConfig, SecureBrokerBackend,
    WsBrokerBackend, DEFAULT_BROKER_SOCKET, DEFAULT_BROKER_WS_URL,
};
pub use docker::{DockerConfig, DockerExecutor};
pub use emission::{
    AggregatedMinerScore, CompetitionWeights, EmissionAllocation, EmissionConfig, EmissionManager,
    EmissionSummary, FinalWeights, MinerScore, WeightCalculator,
    WeightStrategy as EmissionWeightStrategy, MAX_WEIGHT, MIN_WEIGHT,
};
pub use encrypted_api_key::{
    decode_ss58, decrypt_api_key, encode_ss58, encrypt_api_key, parse_hotkey, ApiKeyConfig,
    ApiKeyConfigBuilder, ApiKeyError, EncryptedApiKey, SecureSubmitRequest, SS58_PREFIX,
};
pub use epoch::{
    create_epoch_calculator, EpochCalculator, EpochPhase, EpochState, EpochTransition,
    SharedEpochCalculator, DEFAULT_TEMPO, EPOCH_ZERO_START_BLOCK,
};
pub use evaluation_pipeline::{
    AgentSubmission as PipelineAgentSubmission, EvaluationPipeline,
    EvaluationResult as PipelineEvaluationResult, PackageType, ReceiveResult, ReceiveStatus,
    TaskEvalResult,
};
pub use evaluator::{AgentInfo, TaskEvaluator};
pub use python_whitelist::{ModuleVerification, PythonWhitelist, WhitelistConfig};
pub use reward_decay::{
    AppliedDecay, CompetitionDecayState, DecayConfig, DecayCurve, DecayEvent, DecayResult,
    DecaySummary, RewardDecayManager, TopAgentState, BURN_UID,
};
pub use scoring::{AggregateScore, Leaderboard, ScoreCalculator};
pub use sudo::{
    Competition, CompetitionStatus, CompetitionTask, DynamicLimits, DynamicPricing,
    DynamicWhitelist, SubnetControlStatus, SudoAuditEntry, SudoConfigExport, SudoController,
    SudoError, SudoKey, SudoLevel, SudoPermission, TaskDifficulty as SudoTaskDifficulty,
    WeightStrategy,
};

// Task re-exports from legacy module
pub use task_legacy::{
    AddTaskRequest, Difficulty, Task, TaskConfig, TaskDescription, TaskInfo, TaskRegistry,
    TaskResult,
};

pub use task_execution::{
    EvaluationProgress, EvaluationResult, EvaluationStatus, LLMCallInfo, ProgressStore,
    TaskExecutionResult, TaskExecutionState, TaskExecutor, TaskStatus,
};
pub use time_decay::{
    calculate_decay_info, calculate_decay_multiplier, DecayInfo, DecayStatusResponse,
    TimeDecayConfig, TimeDecayConfigResponse, WinnerDecayStatus,
};
pub use validator_distribution::{
    CodePackage, DistributionConfig, ValidatorDistributor, ValidatorInfo,
};

// API re-exports from legacy module
pub use api_legacy::{
    claim_jobs, download_binary, get_agent_details, get_agent_eval_status, get_leaderboard,
    get_my_agent_source, get_my_jobs, get_status, list_my_agents, submit_agent, ApiState,
};

pub use auth::{
    create_submit_message, is_timestamp_valid, is_valid_ss58_hotkey, verify_signature, AuthManager,
};
pub use evaluation_orchestrator::{
    AgentEvaluationResult, EvaluationOrchestrator, SourceCodeProvider,
};
pub use pg_storage::{
    MinerSubmissionHistory, Submission, SubmissionInfo, DEFAULT_COST_LIMIT_USD, MAX_COST_LIMIT_USD,
    MAX_VALIDATORS_PER_AGENT, SUBMISSION_COOLDOWN_SECS,
};
pub use platform_ws_client::PlatformWsClient;
pub use subnet_control::{
    ControlError, ControlStatus, EvaluatingAgent, EvaluationQueueState, PendingAgent,
    SubnetControlState, SubnetController, MAX_CONCURRENT_AGENTS, MAX_CONCURRENT_TASKS,
    MAX_TASKS_PER_AGENT,
};
pub use timeout_retry_monitor::{
    spawn_timeout_retry_monitor, TimeoutRetryMonitor, TimeoutRetryMonitorConfig,
};
pub use validator_worker::{EvalResult as ValidatorEvalResult, ValidatorWorker};
pub use validator_ws_client::{ValidatorEvent, ValidatorWsClient};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Root validator hotkey
pub const ROOT_VALIDATOR_HOTKEY: &str = "5GziQCcRpN8NCJktX343brnfuVe3w6gUYieeStXPD1Dag2At";

/// Default max agents per epoch
pub const DEFAULT_MAX_AGENTS_PER_EPOCH: f64 = 0.5;

/// Number of top validators for source code
pub const TOP_VALIDATORS_FOR_SOURCE: usize = 3;
