#![allow(dead_code, unused_variables, unused_imports)]
//! Terminal Benchmark Challenge for Platform Network
//!
//! This challenge evaluates AI agents on terminal-based tasks.
//! Agents are run in Docker containers and scored based on task completion.
//!
//! ## Architecture (Centralized)
//!
//! The system uses a centralized API (platform-server) run by the subnet owner:
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────────┐
//! │     Miner       │────▶│   Platform Server    │
//! │   (term CLI)    │     │ (chain.platform.net) │
//! └─────────────────┘     │                      │
//!                         │    ┌──────────┐      │
//! ┌─────────────────┐◀────│    │PostgreSQL│      │
//! │   Validator 1   │     │    └──────────┘      │
//! │  (term-server)  │────▶│                      │
//! └─────────────────┘     └──────────────────────┘
//!        │
//!        ▼
//!   ┌──────────┐
//!   │  SQLite  │ (local cache)
//!   └──────────┘
//! ```
//!
//! ## Features
//!
//! - **Agent Submission**: Miners submit Python source code with module whitelist
//! - **Centralized Evaluation**: Validators receive submissions via WebSocket
//! - **Local Cache**: SQLite for validator-side caching
//! - **Secure Execution**: Agents run in isolated Docker containers
//! - **Real-time Updates**: WebSocket events for all participants

// ============================================================================
// CORE MODULES (Active)
// ============================================================================

pub mod agent_queue;
pub mod agent_registry;
pub mod agent_submission;
pub mod bench;
pub mod blockchain_evaluation;
pub mod challenge;
pub mod code_visibility;
pub mod config;
pub mod container_backend;
pub mod docker;
pub mod emission;
pub mod encrypted_api_key;
pub mod evaluation_orchestrator;
pub mod evaluation_pipeline;
pub mod evaluator;
pub mod llm_client;
pub mod llm_review;
pub mod metagraph_cache;
pub mod python_whitelist;
pub mod reward_decay;
// P2P disabled: pub mod rpc;
pub mod scoring;
// P2P disabled: pub mod secure_submission;
// P2P disabled: pub mod storage_schema;
// P2P disabled: pub mod submission_manager;
pub mod subnet_control;
pub mod sudo;
pub mod task;
pub mod task_execution;
pub mod terminal_harness;
pub mod validator_distribution;
// P2P disabled: pub mod weight_calculator;
pub mod x25519_encryption;

// ============================================================================
// NEW CENTRALIZED MODULES
// ============================================================================

/// Compatibility layer for removed P2P dependencies
pub mod compat;

/// Client for connecting to central API (platform-server)
pub mod central_client;

/// Local SQLite storage for validators
pub mod local_storage;

/// Always-on challenge server (per architecture spec)
pub mod server;

/// Chain storage adapter (now uses central API instead of P2P)
pub mod chain_storage;

// Re-export compat types for use by other modules
pub use compat::{
    AgentInfo as SdkAgentInfo, ChallengeId, EvaluationResult as SdkEvaluationResult,
    EvaluationsResponseMessage, Hotkey, PartitionStats, WeightAssignment,
};

// ============================================================================
// DEPRECATED P2P MODULES (disabled - P2P has been removed)
//
// These modules are kept as comments for reference during migration.
// They depended on: platform-challenge-sdk, platform-core, sled, libp2p
// which have been removed in favor of the centralized API.
// ============================================================================

// NOTE: P2P modules have been disabled because their dependencies were removed.
// - p2p_bridge: Used libp2p for peer-to-peer communication
// - distributed_store: Used sled for distributed storage
// - p2p_chain_storage: Used sled for chain state persistence
// - proposal_manager: Used platform-challenge-sdk for consensus proposals
// - platform_auth: Used platform-core for P2P authentication
// - progress_aggregator: Used platform-challenge-sdk for progress tracking

// If you need to reference the old P2P implementation:
// 1. Check git history for these modules
// 2. The functionality is now handled by:
//    - central_client: Connection to platform-server
//    - local_storage: SQLite caching for validators
//    - chain_storage: Centralized state via API

pub use agent_queue::{
    AgentQueue, EvalRequest, EvalResult, QueueAgentInfo, QueueConfig, QueueStats,
    TaskEvalResult as QueueTaskResult,
};
pub use agent_registry::{AgentEntry, AgentNameEntry, AgentRegistry, AgentStatus, RegistryConfig};
pub use agent_submission::{
    AgentSubmission, AgentSubmissionHandler, SubmissionError, SubmissionStatus,
};
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
    ContainerBackend, ContainerHandle, DirectDockerBackend, ExecOutput, MountConfig, SandboxConfig,
    SecureBrokerBackend, DEFAULT_BROKER_SOCKET,
};
// P2P removed: pub use distributed_store::{DistributedStore, StoreError, TERM_BENCH_CHALLENGE_ID};
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
pub use evaluation_pipeline::{
    AgentSubmission as PipelineAgentSubmission, EvaluationPipeline,
    EvaluationResult as PipelineEvaluationResult, PackageType, ReceiveResult, ReceiveStatus,
    TaskEvalResult,
};
pub use evaluator::{AgentInfo, TaskEvaluator};
// P2P removed: pub use p2p_bridge::{...};
// P2P removed: pub use p2p_chain_storage::{...};
// P2P removed: pub use progress_aggregator::{...};
pub use python_whitelist::{ModuleVerification, PythonWhitelist, WhitelistConfig};
pub use reward_decay::{
    AppliedDecay, CompetitionDecayState, DecayConfig, DecayCurve, DecayEvent, DecayResult,
    DecaySummary, RewardDecayManager, TopAgentState, BURN_UID,
};
// P2P disabled: pub use rpc::{RpcConfig as TermRpcConfig, TermChallengeRpc};
pub use scoring::{AggregateScore, Leaderboard, ScoreCalculator};
// P2P disabled: pub use secure_submission::{...};
// P2P disabled: pub use submission_manager::{...};
pub use sudo::{
    Competition, CompetitionStatus, CompetitionTask, DynamicLimits, DynamicPricing,
    DynamicWhitelist, SubnetControlStatus, SudoAuditEntry, SudoConfigExport, SudoController,
    SudoError, SudoKey, SudoLevel, SudoPermission, TaskDifficulty as SudoTaskDifficulty,
    WeightStrategy,
};
pub use task::{
    AddTaskRequest, Difficulty, Task, TaskConfig, TaskDescription, TaskInfo, TaskRegistry,
    TaskResult,
};
pub use task_execution::{
    EvaluationProgress, EvaluationResult, EvaluationStatus, LLMCallInfo, ProgressStore,
    TaskExecutionResult, TaskExecutionState, TaskExecutor, TaskStatus,
};
pub use validator_distribution::{
    CodePackage, DistributionConfig, ValidatorDistributor, ValidatorInfo,
};
// P2P disabled: pub use weight_calculator::TermWeightCalculator;

// Subnet control and evaluation orchestrator
pub use evaluation_orchestrator::{
    AgentEvaluationResult, EvaluationOrchestrator, SourceCodeProvider,
};
pub use subnet_control::{
    ControlError, ControlStatus, EvaluatingAgent, EvaluationQueueState, PendingAgent,
    SubnetControlState, SubnetController, MAX_CONCURRENT_AGENTS, MAX_CONCURRENT_TASKS,
    MAX_TASKS_PER_AGENT,
};

/// Root validator hotkey - always receives source code
pub const ROOT_VALIDATOR_HOTKEY: &str = "5GziQCcRpN8NCJktX343brnfuVe3w6gUYieeStXPD1Dag2At";

/// Default max agents per epoch (0.5 = 1 agent per 2 epochs)
pub const DEFAULT_MAX_AGENTS_PER_EPOCH: f64 = 0.5;

/// Number of top validators by stake to receive source code (plus root)
pub const TOP_VALIDATORS_FOR_SOURCE: usize = 3;
