//! # Compatibility Layer for Platform SDK Types
//!
//! This module replaces local type definitions with canonical re-exports from
//! [`platform_core`] and [`platform_challenge_sdk`]. It serves as the single
//! import point for all platform types used throughout the term-challenge
//! codebase, ensuring consistent type usage and enabling gradual migration
//! from legacy local definitions.
//!
//! ## Breaking Changes
//!
//! | Local (old)                                  | SDK (new)                                         | Notes                                          |
//! |----------------------------------------------|---------------------------------------------------|-------------------------------------------------|
//! | `Hotkey([u8; 32])` (local newtype)            | `platform_core::Hotkey`                            | Same shape, gains `to_ss58()` / `from_ss58()`  |
//! | `ChallengeId([u8; 16])` (byte-array)          | `platform_core::ChallengeId` (UUID-based)          | Constructor changes — use `ChallengeId::new()`  |
//! | `WeightAssignment { miner_hotkey, weight: u16 }` | `WeightAssignment { hotkey, weight: f64 }`       | Field rename + type change                      |
//! | `ChallengeRoute { path, method: String, .. }` | `ChallengeRoute { method: HttpMethod, .. }`        | Enum-based method, adds `requires_auth`, `rate_limit` |
//! | `RouteRequest { method, path, body }`         | `RouteRequest { .., params, query, auth_hotkey }`  | Richer request model                            |
//! | `RouteResponse { status, body }`              | `RouteResponse { status, headers, body }`          | Adds headers map                                |
//! | `ChallengeError` (local enum)                 | `platform_challenge_sdk::ChallengeError`           | More variants, `From` impls for sled/bincode    |
//! | `Challenge` trait (local)                     | `ServerChallenge` (async trait)                    | Async methods, richer API surface               |
//!
//! ## Migration Guide
//!
//! 1. Replace `use crate::types::Hotkey` with `use term_core::compat::Hotkey`.
//! 2. Replace `ChallengeId::from_bytes(&[u8; 16])` with
//!    `ChallengeId::new()` or `ChallengeId::from_uuid(uuid)`.
//!    Use [`challenge_id_from_bytes`] as a temporary bridge.
//! 3. Replace `WeightAssignment { miner_hotkey, weight: u16_val }` with
//!    `WeightAssignment::new(hotkey_string, weight_u16_to_f64(u16_val))`.
//! 4. Replace string-based route methods with [`HttpMethod`] enum values.
//! 5. Import the [`prelude`] submodule for the most common types.
//!
//! ## Backward Compatibility
//!
//! Deprecated type aliases and conversion helpers are provided in this module
//! so that existing code can migrate incrementally. All deprecated items emit
//! compiler warnings pointing to their replacements.

// ============================================================================
// Section 1 — platform_core re-exports
// ============================================================================

/// 32-byte Ed25519 public key identifying a validator or miner.
///
/// Replaces the former local `Hotkey([u8; 32])` newtype. The SDK version adds
/// `to_ss58()`, `from_ss58()`, `to_hex()`, `from_hex()`, and proper
/// `Debug`/`Display` formatting.
///
/// # Examples
///
/// ```
/// use term_core::compat::Hotkey;
///
/// let hotkey = Hotkey::from_bytes(&[0xab; 32]).unwrap();
/// let ss58 = hotkey.to_ss58();
/// assert!(ss58.starts_with('5'));
///
/// let recovered = Hotkey::from_ss58(&ss58).unwrap();
/// assert_eq!(hotkey, recovered);
/// ```
pub use platform_core::Hotkey;

/// UUID-based challenge identifier.
///
/// **Breaking change**: the old `ChallengeId([u8; 16])` used a raw byte array.
/// The SDK version wraps `uuid::Uuid` and provides `new()` (random v4),
/// `from_uuid()`, `from_string()`, and `Display`/`Debug` formatting.
///
/// To convert legacy 16-byte identifiers use [`challenge_id_from_bytes`].
///
/// # Examples
///
/// ```
/// use term_core::compat::ChallengeId;
///
/// let id = ChallengeId::new();
/// let display = format!("{}", id);
/// assert!(!display.is_empty());
///
/// let parsed = ChallengeId::from_string(&display);
/// assert_eq!(id, parsed);
/// ```
pub use platform_core::ChallengeId;

/// Block height on the Bittensor chain.
pub use platform_core::BlockHeight;

/// Stake amount in RAO (1 TAO = 1e9 RAO).
///
/// # Examples
///
/// ```
/// use term_core::compat::Stake;
///
/// let stake = Stake::new(2_500_000_000);
/// assert_eq!(stake.as_tao(), 2.5);
/// ```
pub use platform_core::Stake;

/// Evaluation score with an associated weight.
///
/// # Examples
///
/// ```
/// use term_core::compat::Score;
///
/// let s = Score::new(0.8, 0.5);
/// assert!((s.weighted_value() - 0.4).abs() < f64::EPSILON);
/// ```
pub use platform_core::Score;

/// Information about a validator on the network.
pub use platform_core::ValidatorInfo;

/// Network-level configuration (subnet id, min stake, consensus threshold, …).
pub use platform_core::NetworkConfig;

/// Evaluation job tracked by the platform.
pub use platform_core::Job;

/// Status of an evaluation job.
///
/// Variants: `Pending`, `Running`, `Completed`, `Failed`, `Timeout`.
pub use platform_core::JobStatus;

// ============================================================================
// Section 2 — platform_challenge_sdk::types re-exports
// ============================================================================

/// Weight assignment for a single miner.
///
/// **Breaking change**: the old local type used `miner_hotkey: String` and
/// `weight: u16` (0–65535). The SDK version uses `hotkey: String` and
/// `weight: f64` (0.0–1.0, clamped in constructor).
///
/// Use [`weight_u16_to_f64`] / [`weight_f64_to_u16`] for conversion and
/// [`LegacyWeightAssignment`] as a temporary bridge.
///
/// # Examples
///
/// ```
/// use term_core::compat::WeightAssignment;
///
/// let wa = WeightAssignment::new("5GrwvaEF...".to_string(), 0.75);
/// assert_eq!(wa.weight, 0.75);
/// ```
pub use platform_challenge_sdk::types::WeightAssignment;

/// Metadata describing a challenge (id, name, description, owner, config, …).
pub use platform_challenge_sdk::types::ChallengeMetadata;

/// Per-challenge configuration (mechanism id, timeouts, memory limits, …).
pub use platform_challenge_sdk::types::ChallengeConfig;

/// Information about a submitted agent.
pub use platform_challenge_sdk::types::AgentInfo;

/// Result of evaluating an agent (score, metrics, logs, execution time).
pub use platform_challenge_sdk::types::EvaluationResult;

/// Evaluation job with full lifecycle tracking.
pub use platform_challenge_sdk::types::EvaluationJob;

/// Epoch information (number, block range, phase).
pub use platform_challenge_sdk::types::EpochInfo;

/// Phase within an epoch (Evaluation, Commit, Reveal, Finalization).
pub use platform_challenge_sdk::types::EpochPhase;

/// Weights submission from a validator for an epoch.
pub use platform_challenge_sdk::types::WeightsSubmission;

/// Aggregated weights after smoothing.
pub use platform_challenge_sdk::types::AggregatedWeights;

// ============================================================================
// Section 3 — Routes re-exports
// ============================================================================

/// HTTP method enum used by [`ChallengeRoute`].
///
/// **Breaking change**: replaces the former `method: String` field. Variants:
/// `Get`, `Post`, `Put`, `Delete`, `Patch`.
///
/// Use [`method_str_to_enum`] / [`method_enum_to_str`] for conversion.
///
/// # Examples
///
/// ```
/// use term_core::compat::HttpMethod;
///
/// let m = HttpMethod::Get;
/// assert_eq!(m.as_str(), "GET");
/// assert_eq!(format!("{}", m), "GET");
/// ```
pub use platform_challenge_sdk::routes::HttpMethod;

/// Route definition exposed by a challenge.
///
/// **Breaking change**: `method` is now [`HttpMethod`] (enum) instead of
/// `String`. New fields: `requires_auth: bool`, `rate_limit: u32`.
///
/// Convenience constructors: `ChallengeRoute::get(path, desc)`,
/// `ChallengeRoute::post(path, desc)`, etc.
///
/// # Examples
///
/// ```
/// use term_core::compat::ChallengeRoute;
///
/// let route = ChallengeRoute::get("/leaderboard", "Current standings")
///     .with_auth()
///     .with_rate_limit(60);
/// assert!(route.requires_auth);
/// assert_eq!(route.rate_limit, 60);
/// ```
pub use platform_challenge_sdk::routes::ChallengeRoute;

/// Incoming request routed to a challenge handler.
///
/// **Breaking change**: now includes `params: HashMap<String, String>`,
/// `query: HashMap<String, String>`, `headers: HashMap<String, String>`,
/// and `auth_hotkey: Option<String>`.
///
/// # Examples
///
/// ```
/// use term_core::compat::RouteRequest;
///
/// let req = RouteRequest::new("GET", "/leaderboard")
///     .with_auth("5Grw...".to_string());
/// assert_eq!(req.auth_hotkey.as_deref(), Some("5Grw..."));
/// ```
pub use platform_challenge_sdk::routes::RouteRequest;

/// Response returned by a challenge route handler.
///
/// **Breaking change**: now includes `headers: HashMap<String, String>`.
///
/// Convenience constructors: `ok()`, `json()`, `not_found()`,
/// `bad_request()`, `unauthorized()`, `internal_error()`, etc.
///
/// # Examples
///
/// ```
/// use term_core::compat::RouteResponse;
/// use serde_json::json;
///
/// let resp = RouteResponse::json(json!({"status": "ok"}));
/// assert_eq!(resp.status, 200);
/// assert!(resp.is_success());
/// ```
pub use platform_challenge_sdk::routes::RouteResponse;

/// Manifest describing all routes a challenge exposes.
pub use platform_challenge_sdk::routes::RoutesManifest;

/// Registry for matching incoming requests to declared routes.
pub use platform_challenge_sdk::routes::RouteRegistry;

/// Fluent builder for declaring routes.
pub use platform_challenge_sdk::routes::RouteBuilder;

// ============================================================================
// Section 4 — Error re-exports
// ============================================================================

/// Canonical error type for challenge operations.
///
/// Replaces any local `ChallengeError` enum. Includes variants for database,
/// serialization, evaluation, validation, network, timeout, and more.
/// Provides `From` impls for `sled::Error`, `bincode::Error`,
/// `serde_json::Error`, and `std::io::Error`.
///
/// # Examples
///
/// ```
/// use term_core::compat::ChallengeError;
///
/// let err = ChallengeError::Evaluation("timeout".to_string());
/// assert!(err.to_string().contains("timeout"));
/// ```
pub use platform_challenge_sdk::error::ChallengeError;

/// Convenience `Result` alias: `std::result::Result<T, ChallengeError>`.
pub use platform_challenge_sdk::error::Result as ChallengeResult;

// ============================================================================
// Section 5 — Server trait & context re-exports
// ============================================================================

/// Async trait that challenges implement for server mode.
///
/// Replaces the former local `Challenge` trait. Key methods:
/// - `challenge_id()`, `name()`, `version()` — identity
/// - `evaluate(EvaluationRequest) -> Result<EvaluationResponse>` — core eval
/// - `validate(ValidationRequest) -> Result<ValidationResponse>` — quick check
/// - `routes() -> Vec<ChallengeRoute>` — declare custom routes
/// - `handle_route(&ChallengeContext, RouteRequest) -> RouteResponse` — handle
///
/// # Examples
///
/// ```text
/// use term_core::compat::prelude::*;
///
/// struct MyChallenge;
///
/// #[async_trait]
/// impl ServerChallenge for MyChallenge {
///     fn challenge_id(&self) -> &str { "my-challenge" }
///     fn name(&self) -> &str { "My Challenge" }
///     fn version(&self) -> &str { "0.1.0" }
///
///     async fn evaluate(
///         &self,
///         req: EvaluationRequest,
///     ) -> Result<EvaluationResponse, ChallengeError> {
///         Ok(EvaluationResponse::success(&req.request_id, 1.0, json!({})))
///     }
/// }
/// ```
pub use platform_challenge_sdk::server::ServerChallenge;

/// Context provided to route handlers (database, challenge id, epoch, block).
pub use platform_challenge_sdk::server::ChallengeContext;

/// Configuration for the challenge HTTP server.
pub use platform_challenge_sdk::server::ServerConfig;

/// Generic evaluation request from the platform.
pub use platform_challenge_sdk::server::EvaluationRequest;

/// Generic evaluation response to the platform.
pub use platform_challenge_sdk::server::EvaluationResponse;

/// Quick validation request (no full evaluation).
pub use platform_challenge_sdk::server::ValidationRequest;

/// Quick validation response.
pub use platform_challenge_sdk::server::ValidationResponse;

/// Health check response.
pub use platform_challenge_sdk::server::HealthResponse;

/// Challenge configuration schema response.
pub use platform_challenge_sdk::server::ConfigResponse;

/// Limits exposed in configuration schema.
pub use platform_challenge_sdk::server::ConfigLimits;

/// Builder for constructing a [`ChallengeServer`].
pub use platform_challenge_sdk::server::ChallengeServerBuilder;

/// HTTP server that hosts a [`ServerChallenge`] implementation.
pub use platform_challenge_sdk::server::ChallengeServer;

// ============================================================================
// Section 6 — Weight calculation type re-exports
// ============================================================================

/// Evaluation result from a single validator (stake-weighted scoring).
pub use platform_challenge_sdk::weight_types::ValidatorEvaluation;

/// Aggregated score for a submission across all validators.
pub use platform_challenge_sdk::weight_types::AggregatedScore;

/// Final weight assignment for a miner (includes rank and raw score).
pub use platform_challenge_sdk::weight_types::MinerWeight;

/// Complete result of weight calculation for an epoch.
pub use platform_challenge_sdk::weight_types::WeightCalculationResult;

/// Tracking information for the current best agent.
pub use platform_challenge_sdk::weight_types::BestAgent;

/// Configuration knobs for weight calculation (thresholds, minimums).
pub use platform_challenge_sdk::weight_types::WeightConfig;

/// Statistics from a weight calculation run.
pub use platform_challenge_sdk::weight_types::CalculationStats;

// ============================================================================
// Section 7 — Submission type re-exports (commit-reveal protocol)
// ============================================================================

/// Encrypted submission from a miner (AES-256-GCM).
pub use platform_challenge_sdk::submission_types::EncryptedSubmission;

/// Acknowledgment from a validator that they received a submission.
pub use platform_challenge_sdk::submission_types::SubmissionAck;

/// Decryption key reveal from a miner after quorum is reached.
pub use platform_challenge_sdk::submission_types::DecryptionKeyReveal;

/// Fully decrypted and verified submission.
pub use platform_challenge_sdk::submission_types::VerifiedSubmission;

/// Errors during submission processing.
pub use platform_challenge_sdk::submission_types::SubmissionError;

/// Encrypt data using AES-256-GCM.
pub use platform_challenge_sdk::submission_types::encrypt_data;

/// Decrypt data using AES-256-GCM.
pub use platform_challenge_sdk::submission_types::decrypt_data;

/// Generate a random 32-byte encryption key.
pub use platform_challenge_sdk::submission_types::generate_key;

/// Generate a random 24-byte nonce.
pub use platform_challenge_sdk::submission_types::generate_nonce;

/// Hash a 32-byte key (SHA-256) for commit-reveal.
pub use platform_challenge_sdk::submission_types::hash_key;

// ============================================================================
// Section 8 — Data submission type re-exports
// ============================================================================

/// Specification for a data key that validators can write to.
pub use platform_challenge_sdk::data::DataKeySpec;

/// Scope of data storage (Validator, Challenge, or Global).
pub use platform_challenge_sdk::data::DataScope;

/// Data submission from a validator.
pub use platform_challenge_sdk::data::DataSubmission;

/// Result of data verification (accept / reject / transform).
pub use platform_challenge_sdk::data::DataVerification;

/// Event emitted during data verification.
pub use platform_challenge_sdk::data::DataEvent;

/// Stored data entry with versioning and expiry.
pub use platform_challenge_sdk::data::StoredData;

/// Query for retrieving stored data (key pattern, scope, pagination).
pub use platform_challenge_sdk::data::DataQuery;

// ============================================================================
// Section 9 — Database re-exports
// ============================================================================

/// Per-challenge sled database with pre-opened trees.
pub use platform_challenge_sdk::database::ChallengeDatabase;

// ============================================================================
// Section 10 — P2P / decentralized re-exports
// ============================================================================

/// Run a challenge in fully decentralized P2P mode.
pub use platform_challenge_sdk::run_decentralized;

/// P2P client for direct validator-to-validator communication.
pub use platform_challenge_sdk::P2PChallengeClient;

/// Configuration for the P2P challenge client.
pub use platform_challenge_sdk::P2PChallengeConfig;

/// Message types exchanged over the P2P network.
pub use platform_challenge_sdk::P2PChallengeMessage;

/// Pending submission tracked by the P2P client.
pub use platform_challenge_sdk::PendingSubmission;

/// Evaluation result from a single validator in P2P mode.
pub use platform_challenge_sdk::ValidatorEvaluationResult;

// ============================================================================
// Section 11 — ChallengeConfigMeta (thin wrapper)
// ============================================================================

/// Combined metadata and configuration for a challenge.
///
/// This is a convenience wrapper that bundles the most commonly needed fields
/// from [`ChallengeMetadata`] and [`ChallengeConfig`] into a single struct,
/// useful for display, logging, and configuration summaries.
///
/// # Examples
///
/// ```
/// use term_core::compat::{ChallengeConfigMeta, ChallengeId, Hotkey};
///
/// let meta = ChallengeConfigMeta {
///     id: ChallengeId::new(),
///     name: "Terminal Benchmark".to_string(),
///     description: "Evaluate terminal agent performance".to_string(),
///     version: "0.1.0".to_string(),
///     owner: Hotkey::from_bytes(&[1u8; 32]).unwrap(),
///     mechanism_id: 1,
///     evaluation_timeout_secs: 300,
///     max_memory_mb: 512,
///     min_validators_for_weights: 3,
///     is_active: true,
/// };
///
/// assert_eq!(meta.name, "Terminal Benchmark");
/// assert_eq!(meta.mechanism_id, 1);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChallengeConfigMeta {
    /// Unique challenge identifier (UUID-based).
    pub id: ChallengeId,
    /// Human-readable challenge name.
    pub name: String,
    /// Short description of the challenge.
    pub description: String,
    /// Semantic version string (e.g. `"0.1.0"`).
    pub version: String,
    /// Hotkey of the challenge owner / deployer.
    pub owner: Hotkey,
    /// Evaluation mechanism identifier.
    pub mechanism_id: u8,
    /// Maximum time (seconds) allowed for a single evaluation.
    pub evaluation_timeout_secs: u64,
    /// Maximum memory (MB) an evaluation may consume.
    pub max_memory_mb: u64,
    /// Minimum number of validators required before weights are set.
    pub min_validators_for_weights: usize,
    /// Whether the challenge is currently accepting submissions.
    pub is_active: bool,
}

impl ChallengeConfigMeta {
    /// Create from a [`ChallengeMetadata`] instance.
    ///
    /// Because `platform_challenge_sdk` defines its own `ChallengeId` that is
    /// structurally identical to `platform_core::ChallengeId` (both wrap
    /// `uuid::Uuid`) but is a distinct Rust type, this method converts via
    /// the inner UUID.
    ///
    /// # Examples
    ///
    /// ```
    /// use term_core::compat::ChallengeConfigMeta;
    /// use platform_challenge_sdk::types::{ChallengeMetadata, ChallengeConfig, ChallengeId as SdkChallengeId};
    /// use platform_core::Hotkey;
    ///
    /// let metadata = ChallengeMetadata {
    ///     id: SdkChallengeId::new(),
    ///     name: "test".to_string(),
    ///     description: "desc".to_string(),
    ///     version: "0.1.0".to_string(),
    ///     owner: Hotkey::from_bytes(&[1u8; 32]).unwrap(),
    ///     emission_weight: 0.5,
    ///     config: ChallengeConfig::default(),
    ///     created_at: chrono::Utc::now(),
    ///     updated_at: chrono::Utc::now(),
    ///     is_active: true,
    /// };
    ///
    /// let meta = ChallengeConfigMeta::from_metadata(&metadata);
    /// assert_eq!(meta.name, "test");
    /// ```
    pub fn from_metadata(m: &ChallengeMetadata) -> Self {
        Self {
            id: ChallengeId::from_uuid(m.id.0),
            name: m.name.clone(),
            description: m.description.clone(),
            version: m.version.clone(),
            owner: m.owner.clone(),
            mechanism_id: m.config.mechanism_id,
            evaluation_timeout_secs: m.config.evaluation_timeout_secs,
            max_memory_mb: m.config.max_memory_mb,
            min_validators_for_weights: m.config.min_validators_for_weights,
            is_active: m.is_active,
        }
    }

    /// Convert back to a full [`ChallengeMetadata`] (fills defaults for
    /// fields not tracked by this wrapper).
    ///
    /// Converts the `platform_core::ChallengeId` to the SDK's
    /// `platform_challenge_sdk::types::ChallengeId` via the inner UUID.
    pub fn to_metadata(&self) -> ChallengeMetadata {
        ChallengeMetadata {
            id: platform_challenge_sdk::types::ChallengeId::from_uuid(self.id.0),
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            owner: self.owner.clone(),
            emission_weight: 0.0,
            config: ChallengeConfig {
                mechanism_id: self.mechanism_id,
                evaluation_timeout_secs: self.evaluation_timeout_secs,
                max_memory_mb: self.max_memory_mb,
                min_validators_for_weights: self.min_validators_for_weights,
                ..ChallengeConfig::default()
            },
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_active: self.is_active,
        }
    }
}

impl std::fmt::Display for ChallengeConfigMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} v{} (id={}, mechanism={}, active={})",
            self.name, self.version, self.id, self.mechanism_id, self.is_active,
        )
    }
}

// ============================================================================
// Section 12 — Deprecated backward-compatible type aliases
// ============================================================================

/// Deprecated alias — use `String` directly for miner hotkey identifiers.
///
/// In the old codebase some call-sites used a dedicated type alias. The SDK
/// uses plain `String` for miner hotkey (SS58 address).
#[deprecated(
    since = "0.1.0",
    note = "Use String directly for miner hotkey identifiers"
)]
pub type MinerHotkey = String;

/// Legacy challenge identifier wrapping a 16-byte array.
///
/// Provided for code that still constructs identifiers from raw bytes.
/// Convert to the canonical [`ChallengeId`] via the `Into` impl.
///
/// # Examples
///
/// ```
/// #![allow(deprecated)]
/// use term_core::compat::{LegacyChallengeId, ChallengeId};
///
/// let legacy = LegacyChallengeId([0u8; 16]);
/// let sdk_id: ChallengeId = legacy.into();
/// ```
#[deprecated(
    since = "0.1.0",
    note = "Use ChallengeId (UUID-based) from platform_core instead"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyChallengeId(pub [u8; 16]);

#[allow(deprecated)]
impl From<LegacyChallengeId> for ChallengeId {
    fn from(legacy: LegacyChallengeId) -> Self {
        ChallengeId::from_uuid(uuid::Uuid::from_bytes(legacy.0))
    }
}

#[allow(deprecated)]
impl From<[u8; 16]> for LegacyChallengeId {
    fn from(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

/// Legacy weight assignment with `u16` weight (0–65535).
///
/// Convert to the canonical [`WeightAssignment`] via the `Into` impl or
/// [`LegacyWeightAssignment::to_sdk`].
///
/// # Examples
///
/// ```
/// #![allow(deprecated)]
/// use term_core::compat::{LegacyWeightAssignment, WeightAssignment};
///
/// let legacy = LegacyWeightAssignment {
///     miner_hotkey: "5Grw...".to_string(),
///     weight: 32768,
/// };
/// let sdk: WeightAssignment = legacy.into();
/// assert!((sdk.weight - 0.5).abs() < 0.001);
/// ```
#[deprecated(
    since = "0.1.0",
    note = "Use WeightAssignment { hotkey: String, weight: f64 } from platform_challenge_sdk"
)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LegacyWeightAssignment {
    /// Miner hotkey (SS58 address string).
    pub miner_hotkey: String,
    /// Weight as a `u16` (0–65 535).
    pub weight: u16,
}

#[allow(deprecated)]
impl LegacyWeightAssignment {
    /// Convert to the canonical SDK [`WeightAssignment`].
    pub fn to_sdk(&self) -> WeightAssignment {
        WeightAssignment::new(self.miner_hotkey.clone(), weight_u16_to_f64(self.weight))
    }
}

#[allow(deprecated)]
impl From<LegacyWeightAssignment> for WeightAssignment {
    fn from(legacy: LegacyWeightAssignment) -> Self {
        legacy.to_sdk()
    }
}

/// Legacy route definition with string-based HTTP method.
///
/// Convert to the canonical [`ChallengeRoute`] via the `Into` impl or
/// [`LegacyRoute::to_sdk`].
///
/// # Examples
///
/// ```
/// #![allow(deprecated)]
/// use term_core::compat::{LegacyRoute, ChallengeRoute};
///
/// let legacy = LegacyRoute {
///     path: "/leaderboard".to_string(),
///     method: "GET".to_string(),
///     description: "Get leaderboard".to_string(),
/// };
/// let sdk: ChallengeRoute = legacy.into();
/// assert_eq!(sdk.method.as_str(), "GET");
/// ```
#[deprecated(
    since = "0.1.0",
    note = "Use ChallengeRoute from platform_challenge_sdk with HttpMethod enum"
)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LegacyRoute {
    /// Route path pattern (e.g. `"/leaderboard"`).
    pub path: String,
    /// HTTP method as a string (e.g. `"GET"`, `"POST"`).
    pub method: String,
    /// Human-readable description of the route.
    pub description: String,
}

#[allow(deprecated)]
impl LegacyRoute {
    /// Convert to the canonical SDK [`ChallengeRoute`].
    pub fn to_sdk(&self) -> ChallengeRoute {
        let method = method_str_to_enum(&self.method).unwrap_or(HttpMethod::Get);
        ChallengeRoute::new(method, &self.path, &self.description)
    }
}

#[allow(deprecated)]
impl From<LegacyRoute> for ChallengeRoute {
    fn from(legacy: LegacyRoute) -> Self {
        legacy.to_sdk()
    }
}

// ============================================================================
// Section 13 — Conversion utilities
// ============================================================================

/// Convert a legacy `u16` weight (0–65535) to the SDK `f64` weight (0.0–1.0).
///
/// # Examples
///
/// ```
/// use term_core::compat::weight_u16_to_f64;
///
/// assert!((weight_u16_to_f64(0) - 0.0).abs() < f64::EPSILON);
/// assert!((weight_u16_to_f64(65535) - 1.0).abs() < f64::EPSILON);
/// assert!((weight_u16_to_f64(32768) - 0.500007629).abs() < 0.001);
/// ```
pub fn weight_u16_to_f64(w: u16) -> f64 {
    f64::from(w) / f64::from(u16::MAX)
}

/// Convert an SDK `f64` weight (0.0–1.0) back to a legacy `u16` weight.
///
/// Values outside 0.0–1.0 are clamped.
///
/// # Examples
///
/// ```
/// use term_core::compat::weight_f64_to_u16;
///
/// assert_eq!(weight_f64_to_u16(0.0), 0);
/// assert_eq!(weight_f64_to_u16(1.0), 65535);
/// assert_eq!(weight_f64_to_u16(0.5), 32768);
/// assert_eq!(weight_f64_to_u16(1.5), 65535); // clamped
/// assert_eq!(weight_f64_to_u16(-0.5), 0);    // clamped
/// ```
pub fn weight_f64_to_u16(w: f64) -> u16 {
    let clamped = w.clamp(0.0, 1.0);
    (clamped * f64::from(u16::MAX)).round() as u16
}

/// Convert a legacy 16-byte array into a UUID-based [`ChallengeId`].
///
/// Interprets the bytes as a UUID (RFC 4122 byte layout). This is the
/// recommended migration path for code that stored challenge IDs as
/// `[u8; 16]`.
///
/// # Examples
///
/// ```
/// use term_core::compat::challenge_id_from_bytes;
///
/// let bytes = [0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
///              0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00];
/// let id = challenge_id_from_bytes(bytes);
/// assert_eq!(format!("{}", id), "550e8400-e29b-41d4-a716-446655440000");
/// ```
pub fn challenge_id_from_bytes(bytes: [u8; 16]) -> ChallengeId {
    ChallengeId::from_uuid(uuid::Uuid::from_bytes(bytes))
}

/// Construct a [`Hotkey`] from a 32-byte slice, returning `None` if the
/// length is wrong.
///
/// Thin wrapper around `Hotkey::from_bytes` for discoverability.
///
/// # Examples
///
/// ```
/// use term_core::compat::hotkey_from_raw_bytes;
///
/// let hk = hotkey_from_raw_bytes(&[0xab; 32]);
/// assert!(hk.is_some());
///
/// let bad = hotkey_from_raw_bytes(&[0xab; 16]);
/// assert!(bad.is_none());
/// ```
pub fn hotkey_from_raw_bytes(bytes: &[u8]) -> Option<Hotkey> {
    Hotkey::from_bytes(bytes)
}

/// Parse a string HTTP method name into an [`HttpMethod`] enum value.
///
/// Matching is case-insensitive. Returns `None` for unrecognised methods.
///
/// # Examples
///
/// ```
/// use term_core::compat::{method_str_to_enum, HttpMethod};
///
/// assert_eq!(method_str_to_enum("GET"), Some(HttpMethod::Get));
/// assert_eq!(method_str_to_enum("post"), Some(HttpMethod::Post));
/// assert_eq!(method_str_to_enum("UNKNOWN"), None);
/// ```
pub fn method_str_to_enum(s: &str) -> Option<HttpMethod> {
    match s.to_uppercase().as_str() {
        "GET" => Some(HttpMethod::Get),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        "DELETE" => Some(HttpMethod::Delete),
        "PATCH" => Some(HttpMethod::Patch),
        _ => None,
    }
}

/// Convert an [`HttpMethod`] enum value to its string representation.
///
/// # Examples
///
/// ```
/// use term_core::compat::{method_enum_to_str, HttpMethod};
///
/// assert_eq!(method_enum_to_str(HttpMethod::Get), "GET");
/// assert_eq!(method_enum_to_str(HttpMethod::Delete), "DELETE");
/// ```
pub fn method_enum_to_str(m: HttpMethod) -> &'static str {
    m.as_str()
}

/// Normalize a set of `f64` weights so they sum to 1.0.
///
/// If all weights are zero the input is returned unchanged.
///
/// # Examples
///
/// ```
/// use term_core::compat::normalize_weights;
///
/// let weights = vec![0.2, 0.3, 0.5];
/// let normed = normalize_weights(&weights);
/// let sum: f64 = normed.iter().sum();
/// assert!((sum - 1.0).abs() < 1e-10);
/// ```
pub fn normalize_weights(weights: &[f64]) -> Vec<f64> {
    let sum: f64 = weights.iter().sum();
    if sum == 0.0 {
        return weights.to_vec();
    }
    weights.iter().map(|w| w / sum).collect()
}

/// Convert a vector of [`LegacyWeightAssignment`] to SDK [`WeightAssignment`]s.
///
/// # Examples
///
/// ```
/// #![allow(deprecated)]
/// use term_core::compat::{LegacyWeightAssignment, convert_legacy_weights};
///
/// let legacy = vec![
///     LegacyWeightAssignment { miner_hotkey: "a".into(), weight: 65535 },
///     LegacyWeightAssignment { miner_hotkey: "b".into(), weight: 0 },
/// ];
/// let sdk = convert_legacy_weights(&legacy);
/// assert_eq!(sdk.len(), 2);
/// assert!((sdk[0].weight - 1.0).abs() < f64::EPSILON);
/// assert!((sdk[1].weight - 0.0).abs() < f64::EPSILON);
/// ```
#[allow(deprecated)]
pub fn convert_legacy_weights(legacy: &[LegacyWeightAssignment]) -> Vec<WeightAssignment> {
    legacy.iter().map(|l| l.to_sdk()).collect()
}

/// Convert a vector of [`LegacyRoute`] to SDK [`ChallengeRoute`]s.
///
/// # Examples
///
/// ```
/// #![allow(deprecated)]
/// use term_core::compat::{LegacyRoute, convert_legacy_routes};
///
/// let legacy = vec![
///     LegacyRoute {
///         path: "/stats".into(),
///         method: "GET".into(),
///         description: "Stats".into(),
///     },
/// ];
/// let sdk = convert_legacy_routes(&legacy);
/// assert_eq!(sdk.len(), 1);
/// assert_eq!(sdk[0].path, "/stats");
/// ```
#[allow(deprecated)]
pub fn convert_legacy_routes(legacy: &[LegacyRoute]) -> Vec<ChallengeRoute> {
    legacy.iter().map(|l| l.to_sdk()).collect()
}

/// Build a [`WeightAssignment`] from hotkey string and u16 weight.
///
/// Convenience function for migrating call-sites that previously constructed
/// the local `WeightAssignment { miner_hotkey, weight: u16 }`.
///
/// # Examples
///
/// ```
/// use term_core::compat::make_weight;
///
/// let wa = make_weight("5Grw...".to_string(), 32768);
/// assert!((wa.weight - 0.5).abs() < 0.001);
/// assert_eq!(wa.hotkey, "5Grw...");
/// ```
pub fn make_weight(hotkey: String, weight_u16: u16) -> WeightAssignment {
    WeightAssignment::new(hotkey, weight_u16_to_f64(weight_u16))
}

/// Build a [`ChallengeRoute`] from string method, path, and description.
///
/// Falls back to `HttpMethod::Get` if the method string is unrecognised.
///
/// # Examples
///
/// ```
/// use term_core::compat::make_route;
///
/// let route = make_route("POST", "/submit", "Submit a result");
/// assert_eq!(route.method.as_str(), "POST");
/// assert_eq!(route.path, "/submit");
/// ```
pub fn make_route(method: &str, path: &str, description: &str) -> ChallengeRoute {
    let m = method_str_to_enum(method).unwrap_or(HttpMethod::Get);
    ChallengeRoute::new(m, path, description)
}

// ============================================================================
// Section 14 — Prelude submodule
// ============================================================================

/// Prelude for convenient imports.
///
/// Import everything commonly needed with a single `use` statement:
///
/// ```
/// use term_core::compat::prelude::*;
/// ```
///
/// This re-exports the most frequently used types, traits, and conversion
/// helpers so that downstream code doesn't need to cherry-pick imports.
pub mod prelude {
    // Core identity types
    pub use super::BlockHeight;
    pub use super::ChallengeId;
    pub use super::Hotkey;
    pub use super::Score;
    pub use super::Stake;

    // Challenge types
    pub use super::AgentInfo;
    pub use super::ChallengeConfig;
    pub use super::ChallengeConfigMeta;
    pub use super::ChallengeMetadata;
    pub use super::EpochInfo;
    pub use super::EpochPhase;
    pub use super::EvaluationJob;
    pub use super::EvaluationResult;
    pub use super::WeightAssignment;

    // Routes
    pub use super::ChallengeRoute;
    pub use super::HttpMethod;
    pub use super::RouteBuilder;
    pub use super::RouteRegistry;
    pub use super::RouteRequest;
    pub use super::RouteResponse;
    pub use super::RoutesManifest;

    // Error
    pub use super::ChallengeError;
    pub use super::ChallengeResult;

    // Server trait & context
    pub use super::ChallengeContext;
    pub use super::ChallengeServer;
    pub use super::ChallengeServerBuilder;
    pub use super::ConfigLimits;
    pub use super::ConfigResponse;
    pub use super::EvaluationRequest;
    pub use super::EvaluationResponse;
    pub use super::HealthResponse;
    pub use super::ServerChallenge;
    pub use super::ServerConfig;
    pub use super::ValidationRequest;
    pub use super::ValidationResponse;

    // Weight types
    pub use super::AggregatedScore;
    pub use super::BestAgent;
    pub use super::CalculationStats;
    pub use super::MinerWeight;
    pub use super::ValidatorEvaluation;
    pub use super::WeightCalculationResult;
    pub use super::WeightConfig;

    // Submission types
    pub use super::DecryptionKeyReveal;
    pub use super::EncryptedSubmission;
    pub use super::SubmissionAck;
    pub use super::SubmissionError;
    pub use super::VerifiedSubmission;

    // Data types
    pub use super::DataEvent;
    pub use super::DataKeySpec;
    pub use super::DataQuery;
    pub use super::DataScope;
    pub use super::DataSubmission;
    pub use super::DataVerification;
    pub use super::StoredData;

    // Network / validator
    pub use super::Job;
    pub use super::JobStatus;
    pub use super::NetworkConfig;
    pub use super::ValidatorInfo;

    // Database
    pub use super::ChallengeDatabase;

    // Conversion helpers
    pub use super::challenge_id_from_bytes;
    pub use super::hotkey_from_raw_bytes;
    pub use super::make_route;
    pub use super::make_weight;
    pub use super::method_enum_to_str;
    pub use super::method_str_to_enum;
    pub use super::normalize_weights;
    pub use super::weight_f64_to_u16;
    pub use super::weight_u16_to_f64;

    // Re-export async_trait for ServerChallenge implementors
    pub use async_trait::async_trait;

    // Common serde/json re-exports
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{json, Value};
}

// ============================================================================
// Section 15 — Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Hotkey ──────────────────────────────────────────────────────────

    #[test]
    fn test_hotkey_roundtrip_bytes() {
        let bytes = [0xab_u8; 32];
        let hk = Hotkey::from_bytes(&bytes).unwrap();
        assert_eq!(hk.as_bytes(), &bytes);
    }

    #[test]
    fn test_hotkey_hex_roundtrip() {
        let hk = Hotkey::from_bytes(&[42u8; 32]).unwrap();
        let hex = hk.to_hex();
        let recovered = Hotkey::from_hex(&hex).unwrap();
        assert_eq!(hk, recovered);
    }

    #[test]
    fn test_hotkey_ss58_roundtrip() {
        let hk = Hotkey::from_bytes(&[0x42; 32]).unwrap();
        let ss58 = hk.to_ss58();
        assert!(ss58.starts_with('5'));
        let recovered = Hotkey::from_ss58(&ss58).unwrap();
        assert_eq!(hk, recovered);
    }

    #[test]
    fn test_hotkey_from_bytes_invalid_length() {
        assert!(Hotkey::from_bytes(&[1u8; 16]).is_none());
        assert!(Hotkey::from_bytes(&[]).is_none());
    }

    #[test]
    fn test_hotkey_display_debug() {
        let hk = Hotkey::from_bytes(&[0xcd; 32]).unwrap();
        let display = format!("{}", hk);
        let debug = format!("{:?}", hk);
        assert!(!display.is_empty());
        assert!(debug.contains("Hotkey"));
    }

    #[test]
    fn test_hotkey_from_raw_bytes_helper() {
        assert!(hotkey_from_raw_bytes(&[1u8; 32]).is_some());
        assert!(hotkey_from_raw_bytes(&[1u8; 31]).is_none());
    }

    #[test]
    fn test_hotkey_equality() {
        let a = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        let b = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        let c = Hotkey::from_bytes(&[2u8; 32]).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_hotkey_hash_map_key() {
        use std::collections::HashMap;
        let hk = Hotkey::from_bytes(&[3u8; 32]).unwrap();
        let mut map = HashMap::new();
        map.insert(hk.clone(), 42);
        assert_eq!(map.get(&hk), Some(&42));
    }

    // ── ChallengeId ────────────────────────────────────────────────────

    #[test]
    fn test_challenge_id_new_unique() {
        let a = ChallengeId::new();
        let b = ChallengeId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn test_challenge_id_from_uuid() {
        let uuid = uuid::Uuid::new_v4();
        let id = ChallengeId::from_uuid(uuid);
        assert_eq!(id.0, uuid);
    }

    #[test]
    fn test_challenge_id_from_string_valid_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = ChallengeId::from_string(uuid_str);
        assert_eq!(format!("{}", id), uuid_str);
    }

    #[test]
    fn test_challenge_id_from_string_non_uuid() {
        let id1 = ChallengeId::from_string("my-challenge");
        let id2 = ChallengeId::from_string("my-challenge");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_challenge_id_display_debug() {
        let id = ChallengeId::new();
        let display = format!("{}", id);
        let debug = format!("{:?}", id);
        assert!(!display.is_empty());
        assert!(debug.contains("Challenge"));
    }

    #[test]
    fn test_challenge_id_default() {
        let a = ChallengeId::default();
        let b = ChallengeId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn test_challenge_id_hash_map_key() {
        use std::collections::HashMap;
        let id = ChallengeId::new();
        let mut map: HashMap<ChallengeId, i32> = HashMap::new();
        map.insert(id, 99);
        assert_eq!(map.get(&id), Some(&99));
    }

    // ── challenge_id_from_bytes ────────────────────────────────────────

    #[test]
    fn test_challenge_id_from_bytes() {
        let bytes = [
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x00,
        ];
        let id = challenge_id_from_bytes(bytes);
        assert_eq!(format!("{}", id), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_challenge_id_from_bytes_zero() {
        let id = challenge_id_from_bytes([0u8; 16]);
        assert_eq!(format!("{}", id), "00000000-0000-0000-0000-000000000000");
    }

    // ── Stake / Score ──────────────────────────────────────────────────

    #[test]
    fn test_stake_tao_conversion() {
        let stake = Stake::new(2_500_000_000);
        assert_eq!(stake.as_tao(), 2.5);
    }

    #[test]
    fn test_stake_ordering() {
        assert!(Stake::new(100) < Stake::new(200));
        assert_eq!(Stake::new(100), Stake::new(100));
    }

    #[test]
    fn test_score_weighted_value() {
        let s = Score::new(0.8, 0.5);
        assert!((s.weighted_value() - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_clamping() {
        let s = Score::new(1.5, -0.5);
        assert_eq!(s.value, 1.0);
        assert_eq!(s.weight, 0.0);
    }

    // ── WeightAssignment ───────────────────────────────────────────────

    #[test]
    fn test_weight_assignment_new() {
        let wa = WeightAssignment::new("hotkey1".to_string(), 0.75);
        assert_eq!(wa.hotkey, "hotkey1");
        assert_eq!(wa.weight, 0.75);
    }

    #[test]
    fn test_weight_assignment_clamping() {
        let wa = WeightAssignment::new("h".to_string(), 2.0);
        assert_eq!(wa.weight, 1.0);
        let wa2 = WeightAssignment::new("h".to_string(), -1.0);
        assert_eq!(wa2.weight, 0.0);
    }

    // ── Weight conversion ──────────────────────────────────────────────

    #[test]
    fn test_weight_u16_to_f64_boundaries() {
        assert!((weight_u16_to_f64(0) - 0.0).abs() < f64::EPSILON);
        assert!((weight_u16_to_f64(u16::MAX) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weight_u16_to_f64_midpoint() {
        let mid = weight_u16_to_f64(32768);
        assert!((mid - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_weight_f64_to_u16_boundaries() {
        assert_eq!(weight_f64_to_u16(0.0), 0);
        assert_eq!(weight_f64_to_u16(1.0), u16::MAX);
    }

    #[test]
    fn test_weight_f64_to_u16_clamping() {
        assert_eq!(weight_f64_to_u16(1.5), u16::MAX);
        assert_eq!(weight_f64_to_u16(-0.5), 0);
    }

    #[test]
    fn test_weight_roundtrip() {
        for w in [0_u16, 1, 100, 32768, 65534, 65535] {
            let f = weight_u16_to_f64(w);
            let back = weight_f64_to_u16(f);
            assert_eq!(w, back, "roundtrip failed for {}", w);
        }
    }

    // ── HttpMethod conversion ──────────────────────────────────────────

    #[test]
    fn test_method_str_to_enum_all() {
        assert_eq!(method_str_to_enum("GET"), Some(HttpMethod::Get));
        assert_eq!(method_str_to_enum("POST"), Some(HttpMethod::Post));
        assert_eq!(method_str_to_enum("PUT"), Some(HttpMethod::Put));
        assert_eq!(method_str_to_enum("DELETE"), Some(HttpMethod::Delete));
        assert_eq!(method_str_to_enum("PATCH"), Some(HttpMethod::Patch));
    }

    #[test]
    fn test_method_str_to_enum_case_insensitive() {
        assert_eq!(method_str_to_enum("get"), Some(HttpMethod::Get));
        assert_eq!(method_str_to_enum("Post"), Some(HttpMethod::Post));
        assert_eq!(method_str_to_enum("pUt"), Some(HttpMethod::Put));
    }

    #[test]
    fn test_method_str_to_enum_unknown() {
        assert_eq!(method_str_to_enum("UNKNOWN"), None);
        assert_eq!(method_str_to_enum(""), None);
        assert_eq!(method_str_to_enum("OPTIONS"), None);
    }

    #[test]
    fn test_method_enum_to_str() {
        assert_eq!(method_enum_to_str(HttpMethod::Get), "GET");
        assert_eq!(method_enum_to_str(HttpMethod::Post), "POST");
        assert_eq!(method_enum_to_str(HttpMethod::Put), "PUT");
        assert_eq!(method_enum_to_str(HttpMethod::Delete), "DELETE");
        assert_eq!(method_enum_to_str(HttpMethod::Patch), "PATCH");
    }

    #[test]
    fn test_method_roundtrip() {
        for m in [
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Delete,
            HttpMethod::Patch,
        ] {
            let s = method_enum_to_str(m);
            let recovered = method_str_to_enum(s).unwrap();
            assert_eq!(m, recovered);
        }
    }

    // ── ChallengeRoute ─────────────────────────────────────────────────

    #[test]
    fn test_challenge_route_get() {
        let r = ChallengeRoute::get("/leaderboard", "Get leaderboard");
        assert_eq!(r.method, HttpMethod::Get);
        assert_eq!(r.path, "/leaderboard");
        assert!(!r.requires_auth);
        assert_eq!(r.rate_limit, 0);
    }

    #[test]
    fn test_challenge_route_post_with_auth() {
        let r = ChallengeRoute::post("/submit", "Submit result")
            .with_auth()
            .with_rate_limit(60);
        assert_eq!(r.method, HttpMethod::Post);
        assert!(r.requires_auth);
        assert_eq!(r.rate_limit, 60);
    }

    #[test]
    fn test_challenge_route_matches() {
        let r = ChallengeRoute::get("/agent/:hash", "Get agent");
        let params = r.matches("GET", "/agent/abc123");
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.get("hash").unwrap(), "abc123");
    }

    #[test]
    fn test_challenge_route_no_match_method() {
        let r = ChallengeRoute::get("/test", "Test");
        assert!(r.matches("POST", "/test").is_none());
    }

    #[test]
    fn test_challenge_route_no_match_path() {
        let r = ChallengeRoute::get("/test", "Test");
        assert!(r.matches("GET", "/other").is_none());
    }

    // ── RouteRequest ───────────────────────────────────────────────────

    #[test]
    fn test_route_request_new() {
        let req = RouteRequest::new("GET", "/leaderboard");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/leaderboard");
        assert!(req.params.is_empty());
        assert!(req.query.is_empty());
        assert!(req.auth_hotkey.is_none());
    }

    #[test]
    fn test_route_request_with_auth() {
        let req = RouteRequest::new("POST", "/submit").with_auth("5Grw...".to_string());
        assert_eq!(req.auth_hotkey.as_deref(), Some("5Grw..."));
    }

    #[test]
    fn test_route_request_with_body() {
        let req =
            RouteRequest::new("POST", "/submit").with_body(serde_json::json!({"score": 0.95}));
        assert_eq!(req.body["score"], 0.95);
    }

    #[test]
    fn test_route_request_with_params() {
        let mut params = std::collections::HashMap::new();
        params.insert("hash".to_string(), "abc".to_string());
        let req = RouteRequest::new("GET", "/agent/abc").with_params(params);
        assert_eq!(req.param("hash"), Some("abc"));
    }

    #[test]
    fn test_route_request_with_query() {
        let mut query = std::collections::HashMap::new();
        query.insert("limit".to_string(), "10".to_string());
        let req = RouteRequest::new("GET", "/list").with_query(query);
        assert_eq!(req.query_param("limit"), Some("10"));
    }

    // ── RouteResponse ──────────────────────────────────────────────────

    #[test]
    fn test_route_response_ok() {
        let resp = RouteResponse::ok(serde_json::json!({"status": "ok"}));
        assert_eq!(resp.status, 200);
        assert!(resp.is_success());
    }

    #[test]
    fn test_route_response_json() {
        let resp = RouteResponse::json(serde_json::json!({"data": [1, 2, 3]}));
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn test_route_response_not_found() {
        let resp = RouteResponse::not_found();
        assert_eq!(resp.status, 404);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_route_response_bad_request() {
        let resp = RouteResponse::bad_request("missing field");
        assert_eq!(resp.status, 400);
    }

    #[test]
    fn test_route_response_unauthorized() {
        let resp = RouteResponse::unauthorized();
        assert_eq!(resp.status, 401);
    }

    #[test]
    fn test_route_response_rate_limited() {
        let resp = RouteResponse::rate_limited();
        assert_eq!(resp.status, 429);
    }

    #[test]
    fn test_route_response_internal_error() {
        let resp = RouteResponse::internal_error("something broke");
        assert_eq!(resp.status, 500);
    }

    #[test]
    fn test_route_response_with_header() {
        let resp = RouteResponse::ok(serde_json::json!({})).with_header("X-Custom", "value");
        assert_eq!(resp.headers.get("X-Custom").unwrap(), "value");
    }

    #[test]
    fn test_route_response_created() {
        let resp = RouteResponse::created(serde_json::json!({"id": 1}));
        assert_eq!(resp.status, 201);
        assert!(resp.is_success());
    }

    #[test]
    fn test_route_response_no_content() {
        let resp = RouteResponse::no_content();
        assert_eq!(resp.status, 204);
        assert!(resp.is_success());
    }

    #[test]
    fn test_route_response_forbidden() {
        let resp = RouteResponse::forbidden("not allowed");
        assert_eq!(resp.status, 403);
    }

    // ── ChallengeError ─────────────────────────────────────────────────

    #[test]
    fn test_challenge_error_display() {
        let err = ChallengeError::Evaluation("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_challenge_error_database() {
        let err = ChallengeError::Database("connection lost".to_string());
        assert!(err.to_string().contains("connection lost"));
    }

    #[test]
    fn test_challenge_error_serialization() {
        let err = ChallengeError::Serialization("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_challenge_error_internal() {
        let err = ChallengeError::Internal("panic".to_string());
        assert!(err.to_string().contains("panic"));
    }

    #[test]
    fn test_challenge_error_insufficient_validators() {
        let err = ChallengeError::InsufficientValidators {
            required: 3,
            got: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("3"));
        assert!(msg.contains("1"));
    }

    #[test]
    fn test_challenge_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: ChallengeError = io_err.into();
        assert!(matches!(err, ChallengeError::Internal(_)));
    }

    // ── ChallengeConfigMeta ────────────────────────────────────────────

    #[test]
    fn test_challenge_config_meta_from_metadata() {
        let metadata = ChallengeMetadata {
            id: platform_challenge_sdk::types::ChallengeId::new(),
            name: "Test Challenge".to_string(),
            description: "A test".to_string(),
            version: "1.0.0".to_string(),
            owner: Hotkey::from_bytes(&[1u8; 32]).unwrap(),
            emission_weight: 0.5,
            config: ChallengeConfig {
                mechanism_id: 2,
                evaluation_timeout_secs: 600,
                max_memory_mb: 1024,
                min_validators_for_weights: 5,
                weight_smoothing: 0.3,
                params: "{}".to_string(),
            },
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_active: true,
        };

        let meta = ChallengeConfigMeta::from_metadata(&metadata);
        assert_eq!(meta.name, "Test Challenge");
        assert_eq!(meta.mechanism_id, 2);
        assert_eq!(meta.evaluation_timeout_secs, 600);
        assert_eq!(meta.max_memory_mb, 1024);
        assert_eq!(meta.min_validators_for_weights, 5);
        assert!(meta.is_active);
    }

    #[test]
    fn test_challenge_config_meta_to_metadata() {
        let meta = ChallengeConfigMeta {
            id: ChallengeId::new(),
            name: "My Challenge".to_string(),
            description: "desc".to_string(),
            version: "0.1.0".to_string(),
            owner: Hotkey::from_bytes(&[2u8; 32]).unwrap(),
            mechanism_id: 3,
            evaluation_timeout_secs: 120,
            max_memory_mb: 256,
            min_validators_for_weights: 2,
            is_active: false,
        };

        let metadata = meta.to_metadata();
        assert_eq!(metadata.name, "My Challenge");
        assert_eq!(metadata.config.mechanism_id, 3);
        assert_eq!(metadata.config.evaluation_timeout_secs, 120);
        assert!(!metadata.is_active);
    }

    #[test]
    fn test_challenge_config_meta_roundtrip() {
        let original = ChallengeMetadata {
            id: platform_challenge_sdk::types::ChallengeId::new(),
            name: "Roundtrip".to_string(),
            description: "test".to_string(),
            version: "2.0.0".to_string(),
            owner: Hotkey::from_bytes(&[5u8; 32]).unwrap(),
            emission_weight: 0.0,
            config: ChallengeConfig::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_active: true,
        };

        let meta = ChallengeConfigMeta::from_metadata(&original);
        let recovered = meta.to_metadata();

        assert_eq!(recovered.name, original.name);
        assert_eq!(recovered.version, original.version);
        assert_eq!(recovered.config.mechanism_id, original.config.mechanism_id);
        assert_eq!(recovered.is_active, original.is_active);
    }

    #[test]
    fn test_challenge_config_meta_display() {
        let meta = ChallengeConfigMeta {
            id: ChallengeId::new(),
            name: "Display Test".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            owner: Hotkey::from_bytes(&[0u8; 32]).unwrap(),
            mechanism_id: 1,
            evaluation_timeout_secs: 300,
            max_memory_mb: 512,
            min_validators_for_weights: 3,
            is_active: true,
        };
        let display = format!("{}", meta);
        assert!(display.contains("Display Test"));
        assert!(display.contains("v1.0.0"));
        assert!(display.contains("mechanism=1"));
        assert!(display.contains("active=true"));
    }

    #[test]
    fn test_challenge_config_meta_serde() {
        let meta = ChallengeConfigMeta {
            id: ChallengeId::new(),
            name: "Serde Test".to_string(),
            description: "test desc".to_string(),
            version: "0.1.0".to_string(),
            owner: Hotkey::from_bytes(&[7u8; 32]).unwrap(),
            mechanism_id: 1,
            evaluation_timeout_secs: 300,
            max_memory_mb: 512,
            min_validators_for_weights: 3,
            is_active: true,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let recovered: ChallengeConfigMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.name, meta.name);
        assert_eq!(recovered.mechanism_id, meta.mechanism_id);
    }

    // ── Legacy types ───────────────────────────────────────────────────

    #[test]
    #[allow(deprecated)]
    fn test_legacy_challenge_id_to_sdk() {
        let legacy = LegacyChallengeId([0u8; 16]);
        let sdk: ChallengeId = legacy.into();
        assert_eq!(format!("{}", sdk), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_challenge_id_from_bytes() {
        let bytes = [1u8; 16];
        let legacy = LegacyChallengeId::from(bytes);
        assert_eq!(legacy.0, bytes);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_weight_assignment_to_sdk() {
        let legacy = LegacyWeightAssignment {
            miner_hotkey: "hotkey1".to_string(),
            weight: 65535,
        };
        let sdk: WeightAssignment = legacy.into();
        assert_eq!(sdk.hotkey, "hotkey1");
        assert!((sdk.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_weight_assignment_zero() {
        let legacy = LegacyWeightAssignment {
            miner_hotkey: "h".to_string(),
            weight: 0,
        };
        let sdk = legacy.to_sdk();
        assert!((sdk.weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_route_to_sdk() {
        let legacy = LegacyRoute {
            path: "/stats".to_string(),
            method: "GET".to_string(),
            description: "Get stats".to_string(),
        };
        let sdk: ChallengeRoute = legacy.into();
        assert_eq!(sdk.method, HttpMethod::Get);
        assert_eq!(sdk.path, "/stats");
        assert_eq!(sdk.description, "Get stats");
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_route_unknown_method() {
        let legacy = LegacyRoute {
            path: "/x".to_string(),
            method: "INVALID".to_string(),
            description: "".to_string(),
        };
        let sdk = legacy.to_sdk();
        assert_eq!(sdk.method, HttpMethod::Get); // fallback
    }

    // ── Conversion helpers ─────────────────────────────────────────────

    #[test]
    fn test_normalize_weights_basic() {
        let weights = vec![0.2, 0.3, 0.5];
        let normed = normalize_weights(&weights);
        let sum: f64 = normed.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize_weights_all_zero() {
        let weights = vec![0.0, 0.0, 0.0];
        let normed = normalize_weights(&weights);
        assert_eq!(normed, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_weights_single() {
        let weights = vec![5.0];
        let normed = normalize_weights(&weights);
        assert!((normed[0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_weights_empty() {
        let normed = normalize_weights(&[]);
        assert!(normed.is_empty());
    }

    #[test]
    #[allow(deprecated)]
    fn test_convert_legacy_weights() {
        let legacy = vec![
            LegacyWeightAssignment {
                miner_hotkey: "a".to_string(),
                weight: 65535,
            },
            LegacyWeightAssignment {
                miner_hotkey: "b".to_string(),
                weight: 0,
            },
        ];
        let sdk = convert_legacy_weights(&legacy);
        assert_eq!(sdk.len(), 2);
        assert!((sdk[0].weight - 1.0).abs() < f64::EPSILON);
        assert!((sdk[1].weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(deprecated)]
    fn test_convert_legacy_routes() {
        let legacy = vec![
            LegacyRoute {
                path: "/a".to_string(),
                method: "GET".to_string(),
                description: "A".to_string(),
            },
            LegacyRoute {
                path: "/b".to_string(),
                method: "POST".to_string(),
                description: "B".to_string(),
            },
        ];
        let sdk = convert_legacy_routes(&legacy);
        assert_eq!(sdk.len(), 2);
        assert_eq!(sdk[0].method, HttpMethod::Get);
        assert_eq!(sdk[1].method, HttpMethod::Post);
    }

    #[test]
    fn test_make_weight() {
        let wa = make_weight("hotkey".to_string(), 32768);
        assert_eq!(wa.hotkey, "hotkey");
        assert!((wa.weight - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_make_route() {
        let r = make_route("POST", "/submit", "Submit");
        assert_eq!(r.method, HttpMethod::Post);
        assert_eq!(r.path, "/submit");
        assert_eq!(r.description, "Submit");
    }

    #[test]
    fn test_make_route_unknown_method_fallback() {
        let r = make_route("TRACE", "/x", "X");
        assert_eq!(r.method, HttpMethod::Get); // fallback
    }

    // ── AgentInfo ──────────────────────────────────────────────────────

    #[test]
    fn test_agent_info_new() {
        let agent = AgentInfo::new("hash123".to_string());
        assert_eq!(agent.hash, "hash123");
        assert!(agent.name.is_none());
        assert!(agent.owner.is_none());
    }

    #[test]
    fn test_agent_info_metadata() {
        let mut agent = AgentInfo::new("h".to_string());
        let meta = serde_json::json!({"key": "value"});
        agent.set_metadata(meta.clone());
        assert_eq!(agent.metadata(), meta);
    }

    // ── EvaluationResult ───────────────────────────────────────────────

    #[test]
    fn test_evaluation_result_new() {
        let result = EvaluationResult::new(uuid::Uuid::new_v4(), "agent".to_string(), 0.85);
        assert_eq!(result.score, 0.85);
        assert!(result.logs.is_none());
    }

    #[test]
    fn test_evaluation_result_clamping() {
        let r = EvaluationResult::new(uuid::Uuid::new_v4(), "a".to_string(), 1.5);
        assert_eq!(r.score, 1.0);
        let r2 = EvaluationResult::new(uuid::Uuid::new_v4(), "a".to_string(), -0.5);
        assert_eq!(r2.score, 0.0);
    }

    #[test]
    fn test_evaluation_result_builders() {
        let mut metrics = std::collections::HashMap::new();
        metrics.insert("accuracy".to_string(), 0.95);

        let result = EvaluationResult::new(uuid::Uuid::new_v4(), "a".to_string(), 0.9)
            .with_metrics(metrics)
            .with_logs("test logs".to_string())
            .with_execution_time(1000);

        assert_eq!(result.metrics.get("accuracy"), Some(&0.95));
        assert_eq!(result.logs, Some("test logs".to_string()));
        assert_eq!(result.execution_time_ms, 1000);
    }

    // ── EvaluationResponse ─────────────────────────────────────────────

    #[test]
    fn test_evaluation_response_success() {
        let resp = EvaluationResponse::success("req-1", 0.9, serde_json::json!({"passed": true}));
        assert!(resp.success);
        assert_eq!(resp.score, 0.9);
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_evaluation_response_error() {
        let resp = EvaluationResponse::error("req-2", "timeout");
        assert!(!resp.success);
        assert_eq!(resp.error.as_deref(), Some("timeout"));
        assert_eq!(resp.score, 0.0);
    }

    #[test]
    fn test_evaluation_response_with_time() {
        let resp = EvaluationResponse::success("r", 0.5, serde_json::json!({})).with_time(500);
        assert_eq!(resp.execution_time_ms, 500);
    }

    #[test]
    fn test_evaluation_response_with_cost() {
        let resp = EvaluationResponse::success("r", 0.5, serde_json::json!({})).with_cost(1.5);
        assert_eq!(resp.cost, Some(1.5));
    }

    // ── WeightConfig ───────────────────────────────────────────────────

    #[test]
    fn test_weight_config_default() {
        let config = WeightConfig::default();
        assert_eq!(config.min_validators, 3);
        assert_eq!(config.min_stake_percentage, 0.3);
        assert_eq!(config.outlier_zscore_threshold, 2.5);
    }

    // ── ChallengeConfig ────────────────────────────────────────────────

    #[test]
    fn test_challenge_config_default() {
        let config = ChallengeConfig::default();
        assert_eq!(config.mechanism_id, 1);
        assert_eq!(config.evaluation_timeout_secs, 300);
        assert_eq!(config.max_memory_mb, 512);
    }

    #[test]
    fn test_challenge_config_with_mechanism() {
        let config = ChallengeConfig::with_mechanism(5);
        assert_eq!(config.mechanism_id, 5);
    }

    // ── ServerConfig ───────────────────────────────────────────────────

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 8080);
        assert_eq!(config.host, "0.0.0.0");
        assert!(config.cors_enabled);
    }

    // ── RouteRegistry ──────────────────────────────────────────────────

    #[test]
    fn test_route_registry_empty() {
        let registry = RouteRegistry::new();
        assert!(registry.is_empty());
        assert!(registry.routes().is_empty());
    }

    #[test]
    fn test_route_registry_register_and_find() {
        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/test", "Test route"));
        assert!(!registry.is_empty());
        assert_eq!(registry.routes().len(), 1);

        let found = registry.find_route("GET", "/test");
        assert!(found.is_some());
        let (route, params) = found.unwrap();
        assert_eq!(route.path, "/test");
        assert!(params.is_empty());
    }

    #[test]
    fn test_route_registry_find_with_params() {
        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/agent/:hash", "Get agent"));

        let found = registry.find_route("GET", "/agent/abc123");
        assert!(found.is_some());
        let (_, params) = found.unwrap();
        assert_eq!(params.get("hash").unwrap(), "abc123");
    }

    #[test]
    fn test_route_registry_no_match() {
        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/test", "Test"));
        assert!(registry.find_route("POST", "/test").is_none());
        assert!(registry.find_route("GET", "/other").is_none());
    }

    // ── RoutesManifest ─────────────────────────────────────────────────

    #[test]
    fn test_routes_manifest_new() {
        let manifest = RoutesManifest::new("My Challenge", "1.0.0");
        assert_eq!(manifest.name, "my-challenge");
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.routes.is_empty());
    }

    #[test]
    fn test_routes_manifest_normalize_name() {
        assert_eq!(
            RoutesManifest::normalize_name("My Challenge"),
            "my-challenge"
        );
        assert_eq!(
            RoutesManifest::normalize_name("  Hello_World  "),
            "hello-world"
        );
    }

    #[test]
    fn test_routes_manifest_with_routes() {
        let manifest = RoutesManifest::new("test", "1.0.0")
            .with_description("A test challenge")
            .add_route(ChallengeRoute::get("/a", "A"))
            .with_routes(vec![ChallengeRoute::post("/b", "B")]);
        assert_eq!(manifest.routes.len(), 2);
        assert_eq!(manifest.description, "A test challenge");
    }

    // ── DataKeySpec ────────────────────────────────────────────────────

    #[test]
    fn test_data_key_spec_new() {
        let spec = DataKeySpec::new("score")
            .validator_scoped()
            .max_size(1024)
            .ttl_blocks(100);
        assert_eq!(spec.key, "score");
        assert_eq!(spec.scope, DataScope::Validator);
        assert_eq!(spec.max_size, 1024);
        assert_eq!(spec.ttl_blocks, 100);
    }

    #[test]
    fn test_data_key_spec_challenge_scoped() {
        let spec = DataKeySpec::new("leaderboard").challenge_scoped();
        assert_eq!(spec.scope, DataScope::Challenge);
    }

    #[test]
    fn test_data_key_spec_global_scoped() {
        let spec = DataKeySpec::new("config").global_scoped();
        assert_eq!(spec.scope, DataScope::Global);
    }

    #[test]
    fn test_data_key_spec_no_consensus() {
        let spec = DataKeySpec::new("local").no_consensus();
        assert!(!spec.requires_consensus);
    }

    // ── DataVerification ───────────────────────────────────────────────

    #[test]
    fn test_data_verification_accept() {
        let v = DataVerification::accept();
        assert!(v.accepted);
        assert!(v.reason.is_none());
    }

    #[test]
    fn test_data_verification_reject() {
        let v = DataVerification::reject("bad data");
        assert!(!v.accepted);
        assert_eq!(v.reason.as_deref(), Some("bad data"));
    }

    #[test]
    fn test_data_verification_accept_with_transform() {
        let v = DataVerification::accept_with_transform(vec![1, 2, 3]);
        assert!(v.accepted);
        assert_eq!(v.transformed_value, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_data_verification_with_ttl() {
        let v = DataVerification::accept().with_ttl(500);
        assert_eq!(v.ttl_override, Some(500));
    }

    // ── DataSubmission ─────────────────────────────────────────────────

    #[test]
    fn test_data_submission_new() {
        let sub = DataSubmission::new("score", vec![1, 2, 3], "validator1")
            .at_block(100)
            .at_epoch(5);
        assert_eq!(sub.key, "score");
        assert_eq!(sub.block_height, 100);
        assert_eq!(sub.epoch, 5);
    }

    #[test]
    fn test_data_submission_value_json() {
        let data = serde_json::json!({"score": 85});
        let json_str = serde_json::to_vec(&data).unwrap();
        let sub = DataSubmission::new("score", json_str, "v1");
        let parsed: serde_json::Value = sub.value_json().unwrap();
        assert_eq!(parsed, data);
    }

    // ── DataQuery ──────────────────────────────────────────────────────

    #[test]
    fn test_data_query_new() {
        let q = DataQuery::new();
        assert!(q.key_pattern.is_none());
        assert!(!q.include_expired);
    }

    #[test]
    fn test_data_query_builder() {
        let q = DataQuery::new()
            .key("score*")
            .scope(DataScope::Validator)
            .validator("v1")
            .include_expired()
            .limit(50)
            .offset(10);
        assert_eq!(q.key_pattern.as_deref(), Some("score*"));
        assert_eq!(q.scope, Some(DataScope::Validator));
        assert_eq!(q.validator.as_deref(), Some("v1"));
        assert!(q.include_expired);
        assert_eq!(q.limit, Some(50));
        assert_eq!(q.offset, Some(10));
    }

    // ── StoredData ─────────────────────────────────────────────────────

    #[test]
    fn test_stored_data_is_expired() {
        let stored = StoredData {
            key: "test".to_string(),
            value: vec![1, 2, 3],
            scope: DataScope::Validator,
            validator: Some("v1".to_string()),
            stored_at_block: 100,
            expires_at_block: Some(200),
            version: 1,
        };
        assert!(!stored.is_expired(150));
        assert!(stored.is_expired(200));
        assert!(stored.is_expired(250));
    }

    #[test]
    fn test_stored_data_permanent() {
        let stored = StoredData {
            key: "perm".to_string(),
            value: vec![],
            scope: DataScope::Challenge,
            validator: None,
            stored_at_block: 100,
            expires_at_block: None,
            version: 1,
        };
        assert!(!stored.is_expired(1_000_000));
    }

    // ── Submission types ───────────────────────────────────────────────

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let nonce = generate_nonce();
        let data = b"Hello, World!";

        let encrypted = encrypt_data(data, &key, &nonce).unwrap();
        let decrypted = decrypt_data(&encrypted, &key, &nonce).unwrap();
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_hash_key_deterministic() {
        let key = generate_key();
        let h1 = hash_key(&key);
        let h2 = hash_key(&key);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_key_different_keys() {
        let k1 = generate_key();
        let k2 = generate_key();
        assert_ne!(hash_key(&k1), hash_key(&k2));
    }

    #[test]
    fn test_encrypted_submission_verify_hash() {
        let key = generate_key();
        let nonce = generate_nonce();
        let key_hash = hash_key(&key);
        let data = b"test code";
        let content_hash = EncryptedSubmission::compute_content_hash(data);
        let encrypted = encrypt_data(data, &key, &nonce).unwrap();

        let submission = EncryptedSubmission::new(
            "challenge-1".to_string(),
            "miner".to_string(),
            "coldkey".to_string(),
            encrypted,
            key_hash,
            nonce,
            content_hash,
            vec![],
            1,
        );

        assert!(submission.verify_hash());
    }

    #[test]
    fn test_decryption_key_reveal_verify() {
        let key = generate_key();
        let key_hash = hash_key(&key);
        let reveal = DecryptionKeyReveal::new([0; 32], key.to_vec(), vec![]);
        assert!(reveal.verify_key_hash(&key_hash));
        assert!(!reveal.verify_key_hash(&[0xff; 32]));
    }

    #[test]
    fn test_submission_error_display() {
        let err = SubmissionError::MinerBanned;
        assert_eq!(err.to_string(), "Miner is banned");
    }

    // ── NetworkConfig ──────────────────────────────────────────────────

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.subnet_id, 100);
        assert_eq!(config.consensus_threshold, 0.50);
    }

    #[test]
    fn test_network_config_production() {
        let config = NetworkConfig::production();
        assert_eq!(config.min_stake.0, 1_000_000_000_000);
    }

    // ── ValidatorInfo ──────────────────────────────────────────────────

    #[test]
    fn test_validator_info_new() {
        let hk = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        let info = ValidatorInfo::new(hk.clone(), Stake::new(1000));
        assert_eq!(info.hotkey, hk);
        assert!(info.is_active);
        assert!(info.peer_id.is_none());
    }

    // ── Job / JobStatus ────────────────────────────────────────────────

    #[test]
    fn test_job_creation() {
        let id = ChallengeId::new();
        let job = Job::new(id, "agent123".to_string());
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.assigned_validator.is_none());
        assert!(job.result.is_none());
    }

    #[test]
    fn test_job_status_equality() {
        assert_eq!(JobStatus::Pending, JobStatus::Pending);
        assert_ne!(JobStatus::Pending, JobStatus::Running);
        assert_ne!(JobStatus::Completed, JobStatus::Failed);
    }

    // ── Prelude smoke test ─────────────────────────────────────────────

    #[test]
    fn test_prelude_imports() {
        use super::prelude::*;

        let _hk = Hotkey::from_bytes(&[1u8; 32]).unwrap();
        let _id = ChallengeId::new();
        let _wa = WeightAssignment::new("h".to_string(), 0.5);
        let _route = ChallengeRoute::get("/test", "test");
        let _req = RouteRequest::new("GET", "/test");
        let _resp = RouteResponse::ok(json!({}));
        let _err = ChallengeError::Internal("test".to_string());
        let _config = WeightConfig::default();
        let _f = weight_u16_to_f64(100);
        let _u = weight_f64_to_u16(0.5);
        let _m = method_str_to_enum("GET");
        let _s = method_enum_to_str(HttpMethod::Get);
        let _n = normalize_weights(&[0.5, 0.5]);
        let _w = make_weight("h".to_string(), 100);
        let _r = make_route("GET", "/x", "x");
    }
}
