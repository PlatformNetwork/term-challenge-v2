use alloc::string::String;
use alloc::vec::Vec;

pub type Hotkey = String;

pub type ChallengeId = String;

pub type WeightAssignment = platform_challenge_sdk_wasm::WeightEntry;

pub type ChallengeRoute = platform_challenge_sdk_wasm::WasmRouteDefinition;

pub type RouteRequest = platform_challenge_sdk_wasm::WasmRouteRequest;

pub type RouteResponse = platform_challenge_sdk_wasm::WasmRouteResponse;

#[derive(Debug, Clone)]
pub enum ChallengeError {
    Connection(String),
    Auth(String),
    Config(String),
    Io(String),
    Evaluation(String),
    Validation(String),
    Network(String),
    Timeout(String),
    Database(String),
    Serialization(String),
}

impl core::fmt::Display for ChallengeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ChallengeError::Connection(msg) => write!(f, "Connection error: {msg}"),
            ChallengeError::Auth(msg) => write!(f, "Authentication error: {msg}"),
            ChallengeError::Config(msg) => write!(f, "Configuration error: {msg}"),
            ChallengeError::Io(msg) => write!(f, "IO error: {msg}"),
            ChallengeError::Evaluation(msg) => write!(f, "Evaluation error: {msg}"),
            ChallengeError::Validation(msg) => write!(f, "Validation error: {msg}"),
            ChallengeError::Network(msg) => write!(f, "Network error: {msg}"),
            ChallengeError::Timeout(msg) => write!(f, "Timeout: {msg}"),
            ChallengeError::Database(msg) => write!(f, "Database error: {msg}"),
            ChallengeError::Serialization(msg) => write!(f, "Serialization error: {msg}"),
        }
    }
}

pub trait ServerChallenge {
    fn challenge_id(&self) -> &str;

    fn name(&self) -> &str;

    fn version(&self) -> &str;

    fn evaluate(
        &self,
        input: platform_challenge_sdk_wasm::EvaluationInput,
    ) -> platform_challenge_sdk_wasm::EvaluationOutput;

    fn validate(&self, input: platform_challenge_sdk_wasm::EvaluationInput) -> bool;

    fn routes(&self) -> Vec<ChallengeRoute> {
        Vec::new()
    }

    fn handle_route(&self, _request: RouteRequest) -> RouteResponse {
        RouteResponse {
            status: 404,
            body: Vec::new(),
        }
    }
}
