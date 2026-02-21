use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use platform_challenge_sdk::routes::{ChallengeRoute, RouteRequest, RouteResponse};
use platform_challenge_sdk::server::{
    ChallengeContext, ConfigResponse, EvaluationRequest, EvaluationResponse, HealthResponse,
    ServerChallenge, ServerConfig, ValidationRequest, ValidationResponse,
};
use platform_challenge_sdk::types::ChallengeId;
use platform_challenge_sdk::ChallengeDatabase;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Shared state for the challenge HTTP server.
///
/// Wraps a `ServerChallenge` implementation and provides the axum state
/// needed to serve evaluation, health, config, and custom challenge routes.
pub struct ChallengeServerState<C: ServerChallenge> {
    pub challenge: Arc<C>,
    pub config: ServerConfig,
    pub started_at: Instant,
    pub pending_count: Arc<RwLock<u32>>,
    pub challenge_id: ChallengeId,
}

impl<C: ServerChallenge + 'static> ChallengeServerState<C> {
    /// Create a new server state from a challenge, config, and UUID-based challenge ID.
    pub fn new(challenge: C, config: ServerConfig, challenge_id: ChallengeId) -> Self {
        Self {
            challenge: Arc::new(challenge),
            config,
            started_at: Instant::now(),
            pending_count: Arc::new(RwLock::new(0)),
            challenge_id,
        }
    }

    /// Build and return the axum `Router` with all platform and custom routes.
    ///
    /// Platform endpoints:
    /// - `POST /evaluate` — receive evaluation requests
    /// - `GET /health` — health check
    ///
    /// Custom routes declared by `ServerChallenge::routes()` are handled via
    /// a catch-all fallback that matches against `ChallengeRoute` definitions.
    pub fn router(self) -> Router {
        let state = Arc::new(self);

        let custom_routes = state.challenge.routes();
        if !custom_routes.is_empty() {
            info!(
                "Challenge {} declares {} custom route(s)",
                state.challenge.challenge_id(),
                custom_routes.len()
            );
            for route in &custom_routes {
                debug!(
                    "  {} {} (auth={}, rate_limit={}): {}",
                    route.method.as_str(),
                    route.path,
                    route.requires_auth,
                    route.rate_limit,
                    route.description,
                );
            }
        }

        Router::new()
            .route("/health", get(health_handler::<C>))
            .route("/config", get(config_handler::<C>))
            .route("/evaluate", post(evaluate_handler::<C>))
            .route("/validate", post(validate_handler::<C>))
            .fallback(custom_route_handler::<C>)
            .with_state(state)
    }

    /// Run the axum server, binding to the configured host and port.
    pub async fn run(self) -> Result<(), platform_challenge_sdk::ChallengeError> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| {
                platform_challenge_sdk::ChallengeError::Config(format!("Invalid address: {}", e))
            })?;

        info!(
            "Starting challenge server {} on {}",
            self.challenge.challenge_id(),
            addr
        );

        let app = self.router();

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| platform_challenge_sdk::ChallengeError::Io(e.to_string()))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| platform_challenge_sdk::ChallengeError::Io(e.to_string()))?;

        Ok(())
    }
}

// ============================================================================
// HEALTH HANDLER
// ============================================================================

async fn health_handler<C: ServerChallenge + 'static>(
    State(state): State<Arc<ChallengeServerState<C>>>,
) -> Json<HealthResponse> {
    let pending = *state.pending_count.read().await;
    let load = pending as f64 / state.config.max_concurrent as f64;

    Json(HealthResponse {
        healthy: true,
        load: load.min(1.0),
        pending,
        uptime_secs: state.started_at.elapsed().as_secs(),
        version: state.challenge.version().to_string(),
        challenge_id: state.challenge_id.to_string(),
    })
}

// ============================================================================
// CONFIG HANDLER — SDK ConfigResponse
// ============================================================================

async fn config_handler<C: ServerChallenge + 'static>(
    State(state): State<Arc<ChallengeServerState<C>>>,
) -> Json<ConfigResponse> {
    Json(state.challenge.config())
}

// ============================================================================
// VALIDATE HANDLER — SDK ValidationRequest / ValidationResponse
// ============================================================================

async fn validate_handler<C: ServerChallenge + 'static>(
    State(state): State<Arc<ChallengeServerState<C>>>,
    Json(request): Json<ValidationRequest>,
) -> Json<ValidationResponse> {
    match state.challenge.validate(request).await {
        Ok(response) => Json(response),
        Err(e) => Json(ValidationResponse {
            valid: false,
            errors: vec![e.to_string()],
            warnings: vec![],
        }),
    }
}

// ============================================================================
// EVALUATE HANDLER — SDK EvaluationRequest / EvaluationResponse
// ============================================================================

async fn evaluate_handler<C: ServerChallenge + 'static>(
    State(state): State<Arc<ChallengeServerState<C>>>,
    Json(request): Json<EvaluationRequest>,
) -> (StatusCode, Json<EvaluationResponse>) {
    let request_id = request.request_id.clone();
    let start = Instant::now();

    {
        let mut count = state.pending_count.write().await;
        *count += 1;
    }

    let result = state.challenge.evaluate(request).await;

    {
        let mut count = state.pending_count.write().await;
        *count = count.saturating_sub(1);
    }

    match result {
        Ok(mut response) => {
            response.execution_time_ms = start.elapsed().as_millis() as i64;
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            error!("Evaluation failed for {}: {}", request_id, e);
            let response = EvaluationResponse::error(&request_id, e.to_string())
                .with_time(start.elapsed().as_millis() as i64);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

// ============================================================================
// CUSTOM ROUTE HANDLER — SDK ChallengeRoute / RouteRequest / RouteResponse
// ============================================================================

/// Catch-all handler that matches incoming requests against `ChallengeRoute`
/// definitions declared by the challenge.
///
/// Constructs an SDK `RouteRequest` with:
/// - `params` — path parameters extracted via `ChallengeRoute::matches()`
/// - `query` — query-string key/value pairs from the axum `Query` extractor
/// - `auth_hotkey` — extracted from the `X-Auth-Hotkey` request header
///
/// Returns an SDK `RouteResponse` mapped to an axum response with the
/// appropriate status code, JSON body, and response headers.
async fn custom_route_handler<C: ServerChallenge + 'static>(
    State(state): State<Arc<ChallengeServerState<C>>>,
    method: Method,
    uri: Uri,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let method_str = method.as_str().to_string();

    let custom_routes = state.challenge.routes();

    let mut matched_params = HashMap::new();
    let mut matched_route: Option<&ChallengeRoute> = None;
    for route in &custom_routes {
        if let Some(params) = route.matches(&method_str, &path) {
            matched_params = params;
            matched_route = Some(route);
            break;
        }
    }

    let route = match matched_route {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                HeaderMap::new(),
                Json(serde_json::json!({
                    "error": "not_found",
                    "message": format!("No route matches {} {}", method_str, path)
                })),
            );
        }
    };

    let auth_hotkey = headers
        .get("x-auth-hotkey")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if route.requires_auth && auth_hotkey.is_none() {
        let resp = RouteResponse::unauthorized();
        return route_response_to_axum(resp);
    }

    let mut headers_map = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            headers_map.insert(key.as_str().to_string(), v.to_string());
        }
    }

    let request = RouteRequest {
        method: method_str,
        path,
        params: matched_params,
        query,
        headers: headers_map,
        body: body.map(|b| b.0).unwrap_or(serde_json::Value::Null),
        auth_hotkey,
    };

    let ctx = ChallengeContext {
        db: Arc::new(
            ChallengeDatabase::open(
                std::env::temp_dir(),
                ChallengeId::from_uuid(state.challenge_id.0),
            )
            .unwrap_or_else(|_| {
                ChallengeDatabase::open(std::env::temp_dir(), ChallengeId::new())
                    .expect("Failed to open temporary challenge database")
            }),
        ),
        challenge_id: state.challenge.challenge_id().to_string(),
        epoch: 0,
        block_height: 0,
    };

    let response = state.challenge.handle_route(&ctx, request).await;
    route_response_to_axum(response)
}

/// Map an SDK `RouteResponse` to an axum `(StatusCode, HeaderMap, Json<Value>)`.
fn route_response_to_axum(
    response: RouteResponse,
) -> (StatusCode, HeaderMap, Json<serde_json::Value>) {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut headers = HeaderMap::new();
    for (key, value) in &response.headers {
        if let (Ok(name), Ok(val)) = (
            axum::http::header::HeaderName::from_bytes(key.as_bytes()),
            axum::http::header::HeaderValue::from_str(value),
        ) {
            headers.insert(name, val);
        }
    }

    (status, headers, Json(response.body))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use platform_challenge_sdk::error::ChallengeError;
    use platform_challenge_sdk::routes::{ChallengeRoute, HttpMethod};
    use platform_challenge_sdk::server::{EvaluationRequest, EvaluationResponse, ServerConfig};
    use serde_json::json;

    struct MockChallenge;

    #[async_trait::async_trait]
    impl ServerChallenge for MockChallenge {
        fn challenge_id(&self) -> &str {
            "mock-challenge"
        }

        fn name(&self) -> &str {
            "Mock Challenge"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        async fn evaluate(
            &self,
            request: EvaluationRequest,
        ) -> Result<EvaluationResponse, ChallengeError> {
            Ok(EvaluationResponse::success(
                &request.request_id,
                0.85,
                json!({"mock": true}),
            ))
        }

        fn routes(&self) -> Vec<ChallengeRoute> {
            vec![
                ChallengeRoute::get("/leaderboard", "Get leaderboard"),
                ChallengeRoute::post("/submit", "Submit result")
                    .with_auth()
                    .with_rate_limit(10),
                ChallengeRoute::get("/agent/:hotkey/stats", "Get agent stats"),
            ]
        }

        async fn handle_route(&self, _ctx: &ChallengeContext, req: RouteRequest) -> RouteResponse {
            match (req.method.as_str(), req.path.as_str()) {
                ("GET", "/leaderboard") => RouteResponse::json(json!({"entries": [], "total": 0})),
                ("POST", "/submit") => RouteResponse::created(json!({"status": "accepted"})),
                _ if req.path.starts_with("/agent/") => {
                    let hotkey = req.param("hotkey").unwrap_or("unknown");
                    RouteResponse::json(json!({
                        "hotkey": hotkey,
                        "score": 0.75,
                        "auth": req.auth_hotkey,
                    }))
                }
                _ => RouteResponse::not_found(),
            }
        }
    }

    #[test]
    fn test_challenge_server_state_new() {
        let state =
            ChallengeServerState::new(MockChallenge, ServerConfig::default(), ChallengeId::new());

        assert_eq!(state.challenge.challenge_id(), "mock-challenge");
        assert_eq!(state.config.port, 8080);
    }

    #[test]
    fn test_challenge_server_state_with_custom_id() {
        let uuid = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = ChallengeId::from_uuid(uuid);
        let state = ChallengeServerState::new(MockChallenge, ServerConfig::default(), id);

        assert_eq!(
            state.challenge_id.to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_challenge_route_with_http_method() {
        let route = ChallengeRoute::new(HttpMethod::Get, "/test", "Test route");
        assert_eq!(route.method, HttpMethod::Get);
        assert!(!route.requires_auth);
        assert_eq!(route.rate_limit, 0);
    }

    #[test]
    fn test_challenge_route_with_auth_and_rate_limit() {
        let route = ChallengeRoute::post("/submit", "Submit")
            .with_auth()
            .with_rate_limit(60);

        assert_eq!(route.method, HttpMethod::Post);
        assert!(route.requires_auth);
        assert_eq!(route.rate_limit, 60);
    }

    #[test]
    fn test_challenge_route_matching_with_params() {
        let route = ChallengeRoute::get("/agent/:hotkey/stats", "Stats");
        let params = route.matches("GET", "/agent/abc123/stats");

        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.get("hotkey"), Some(&"abc123".to_string()));
    }

    #[test]
    fn test_route_request_construction() {
        let mut params = HashMap::new();
        params.insert("hotkey".to_string(), "abc123".to_string());

        let mut query = HashMap::new();
        query.insert("limit".to_string(), "10".to_string());

        let req = RouteRequest {
            method: "GET".to_string(),
            path: "/agent/abc123/stats".to_string(),
            params,
            query,
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            auth_hotkey: Some("validator-key".to_string()),
        };

        assert_eq!(req.param("hotkey"), Some("abc123"));
        assert_eq!(req.query_param("limit"), Some("10"));
        assert_eq!(req.auth_hotkey, Some("validator-key".to_string()));
    }

    #[test]
    fn test_route_response_to_axum_mapping() {
        let response = RouteResponse::ok(json!({"status": "ok"})).with_header("X-Custom", "value");

        let (status, headers, body) = route_response_to_axum(response);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-custom").and_then(|v| v.to_str().ok()),
            Some("value")
        );
        assert_eq!(body.0["status"], "ok");
    }

    #[test]
    fn test_route_response_not_found_mapping() {
        let response = RouteResponse::not_found();
        let (status, _, _) = route_response_to_axum(response);
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_route_response_unauthorized_mapping() {
        let response = RouteResponse::unauthorized();
        let (status, _, _) = route_response_to_axum(response);
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_route_response_bad_request_mapping() {
        let response = RouteResponse::bad_request("Invalid input");
        let (status, _, body) = route_response_to_axum(response);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0["error"], "bad_request");
    }

    #[test]
    fn test_route_response_internal_error_mapping() {
        let response = RouteResponse::internal_error("Something broke");
        let (status, _, _) = route_response_to_axum(response);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_route_response_rate_limited_mapping() {
        let response = RouteResponse::rate_limited();
        let (status, _, _) = route_response_to_axum(response);
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_evaluation_request_sdk_type() {
        let req = EvaluationRequest {
            request_id: "req-123".to_string(),
            submission_id: "sub-456".to_string(),
            participant_id: "miner-789".to_string(),
            data: json!({"code": "fn main() {}"}),
            metadata: Some(json!({"version": "1.0"})),
            epoch: 5,
            deadline: Some(1700000000),
        };

        assert_eq!(req.request_id, "req-123");
        assert_eq!(req.epoch, 5);
        assert!(req.metadata.is_some());
    }

    #[test]
    fn test_evaluation_response_sdk_success() {
        let resp = EvaluationResponse::success("req-123", 0.95, json!({"passed": 19}));
        assert!(resp.success);
        assert_eq!(resp.score, 0.95);
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_evaluation_response_sdk_error() {
        let resp = EvaluationResponse::error("req-456", "Timeout");
        assert!(!resp.success);
        assert_eq!(resp.score, 0.0);
        assert_eq!(resp.error, Some("Timeout".to_string()));
    }

    #[test]
    fn test_evaluation_response_with_time_and_cost() {
        let resp = EvaluationResponse::success("req", 0.8, json!({}))
            .with_time(1500)
            .with_cost(0.05);

        assert_eq!(resp.execution_time_ms, 1500);
        assert_eq!(resp.cost, Some(0.05));
    }

    #[test]
    fn test_challenge_id_uuid_based() {
        let id = ChallengeId::new();
        let display = format!("{}", id);
        assert!(!display.is_empty());

        let parsed = ChallengeId::from_str(&display);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_challenge_id_from_str() {
        let valid = ChallengeId::from_str("550e8400-e29b-41d4-a716-446655440000");
        assert!(valid.is_some());

        let invalid = ChallengeId::from_str("not-a-uuid");
        assert!(invalid.is_none());
    }

    #[tokio::test]
    async fn test_mock_challenge_evaluate() {
        let req = EvaluationRequest {
            request_id: "test-eval".to_string(),
            submission_id: "sub-1".to_string(),
            participant_id: "miner-1".to_string(),
            data: json!({}),
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let challenge = MockChallenge;
        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert_eq!(result.score, 0.85);
        assert_eq!(result.request_id, "test-eval");
    }

    #[test]
    fn test_mock_challenge_routes_use_sdk_types() {
        let challenge = MockChallenge;
        let routes = challenge.routes();

        assert_eq!(routes.len(), 3);

        assert_eq!(routes[0].method, HttpMethod::Get);
        assert_eq!(routes[0].path, "/leaderboard");
        assert!(!routes[0].requires_auth);
        assert_eq!(routes[0].rate_limit, 0);

        assert_eq!(routes[1].method, HttpMethod::Post);
        assert_eq!(routes[1].path, "/submit");
        assert!(routes[1].requires_auth);
        assert_eq!(routes[1].rate_limit, 10);
    }

    #[tokio::test]
    async fn test_mock_challenge_handle_route_with_params() {
        let challenge = MockChallenge;
        let mut params = HashMap::new();
        params.insert("hotkey".to_string(), "test-hotkey".to_string());

        let req = RouteRequest {
            method: "GET".to_string(),
            path: "/agent/test-hotkey/stats".to_string(),
            params,
            query: HashMap::new(),
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            auth_hotkey: Some("validator-1".to_string()),
        };

        let ctx = ChallengeContext {
            db: Arc::new(
                ChallengeDatabase::open(std::env::temp_dir(), ChallengeId::new())
                    .expect("Failed to open temp db"),
            ),
            challenge_id: "mock-challenge".to_string(),
            epoch: 0,
            block_height: 0,
        };

        let response = challenge.handle_route(&ctx, req).await;

        assert!(response.is_success());
        assert_eq!(response.status, 200);
        assert_eq!(response.body["hotkey"], "test-hotkey");
        assert_eq!(response.body["auth"], "validator-1");
    }

    #[test]
    fn test_router_builds_without_panic() {
        let state =
            ChallengeServerState::new(MockChallenge, ServerConfig::default(), ChallengeId::new());
        let _router = state.router();
    }

    #[tokio::test]
    async fn test_pending_count_tracking() {
        let state =
            ChallengeServerState::new(MockChallenge, ServerConfig::default(), ChallengeId::new());

        assert_eq!(*state.pending_count.read().await, 0);

        {
            let mut count = state.pending_count.write().await;
            *count += 1;
        }
        assert_eq!(*state.pending_count.read().await, 1);

        {
            let mut count = state.pending_count.write().await;
            *count = count.saturating_sub(1);
        }
        assert_eq!(*state.pending_count.read().await, 0);
    }

    #[test]
    fn test_http_method_enum_variants() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
        assert_eq!(HttpMethod::Put.as_str(), "PUT");
        assert_eq!(HttpMethod::Delete.as_str(), "DELETE");
        assert_eq!(HttpMethod::Patch.as_str(), "PATCH");
    }

    #[test]
    fn test_route_response_headers_mapping() {
        let response = RouteResponse::ok(json!({}))
            .with_header("Content-Type", "application/json")
            .with_header("X-Request-Id", "abc-123");

        let (_, headers, _) = route_response_to_axum(response);

        assert_eq!(
            headers.get("content-type").and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            headers.get("x-request-id").and_then(|v| v.to_str().ok()),
            Some("abc-123")
        );
    }
}
