//! Provides the generic route infrastructure for challenges to define custom
//! HTTP routes that get mounted on the RPC server. Each challenge declares its
//! own routes and handlers via the `ServerChallenge` trait — the platform SDK
//! does NOT hardcode any challenge-specific routes.
//! use platform_challenge_sdk::server::{ServerChallenge, ChallengeContext};
//! impl ServerChallenge for MyChallenge {
//!     // ... challenge_id, name, version, evaluate ...
//!
//! on the RPC server. Each challenge can expose its own API endpoints.
//!
//! # Example
//!
//! ```text
//! use platform_challenge_sdk::routes::*;
//!
//! impl Challenge for MyChallenge {
//!     fn routes(&self) -> Vec<ChallengeRoute> {
//!         vec![
//!             ChallengeRoute::get("/leaderboard", "Get current leaderboard"),
//!             ChallengeRoute::get("/stats", "Get challenge statistics"),
//!             ChallengeRoute::post("/submit", "Submit evaluation result"),
//!             ChallengeRoute::get("/agent/:hash", "Get agent details"),
//!         ]
//!     }
//!
//!     async fn handle_route(
//!         &self,
//!         ctx: &ChallengeContext,
//!         req: RouteRequest,
//!     ) -> RouteResponse {
//!         match (req.method.as_str(), req.path.as_str()) {
//!             ("GET", "/leaderboard") => {
//!                 let data = self.get_leaderboard(ctx).await;
//!                 RouteResponse::json(data)
//!             }
//!             ("GET", path) if path.starts_with("/agent/") => {
//!                 let hash = &path[7..];
//!                 let agent = self.get_agent(ctx, hash).await;
//!                 RouteResponse::json(agent)
//!             }
//!             _ => RouteResponse::not_found()
//!         }
//!     }
//! }
//! ```
//!
//! // The platform SDK provides the generic building blocks (ChallengeRoute,
//! // RouteRequest, RouteResponse, RouteRegistry, RouteBuilder, RoutesManifest,
//! // HttpMethod) — challenges use these to declare their own routes.
//!             _ => RouteResponse::not_found()
//!         }
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Routes manifest returned by /.well-known/routes endpoint
/// This is the standard format for dynamic route discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutesManifest {
    /// Challenge name (normalized: lowercase, dashes only)
    pub name: String,
    /// Challenge version
    pub version: String,
    /// Human-readable description
    pub description: String,
    /// List of available routes
    pub routes: Vec<ChallengeRoute>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl RoutesManifest {
    /// Create a new routes manifest
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: Self::normalize_name(&name.into()),
            version: version.into(),
            description: String::new(),
            routes: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Normalize challenge name: lowercase, replace spaces/underscores with dashes
    pub fn normalize_name(name: &str) -> String {
        name.trim()
            .to_lowercase()
            .replace([' ', '_'], "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add a single route
    pub fn add_route(mut self, route: ChallengeRoute) -> Self {
        self.routes.push(route);
        self
    }

    /// Add multiple routes
    pub fn with_routes(mut self, routes: Vec<ChallengeRoute>) -> Self {
        self.routes.extend(routes);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// HTTP method for routes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Definition of a custom route exposed by a challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeRoute {
    /// HTTP method (GET, POST, etc.)
    pub method: HttpMethod,
    /// Path pattern (e.g., "/leaderboard", "/agent/:hash")
    pub path: String,
    /// Description of what this route does
    pub description: String,
    /// Whether authentication is required
    pub requires_auth: bool,
    /// Rate limit (requests per minute, 0 = unlimited)
    pub rate_limit: u32,
}

impl ChallengeRoute {
    /// Create a new route
    pub fn new(
        method: HttpMethod,
        path: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            description: description.into(),
            requires_auth: false,
            rate_limit: 0,
        }
    }

    /// Create a GET route
    pub fn get(path: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, path, description)
    }

    /// Create a POST route
    pub fn post(path: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(HttpMethod::Post, path, description)
    }

    /// Create a PUT route
    pub fn put(path: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(HttpMethod::Put, path, description)
    }

    /// Create a DELETE route
    pub fn delete(path: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(HttpMethod::Delete, path, description)
    }

    /// Require authentication for this route
    pub fn with_auth(mut self) -> Self {
        self.requires_auth = true;
        self
    }

    /// Set rate limit (requests per minute)
    pub fn with_rate_limit(mut self, rpm: u32) -> Self {
        self.rate_limit = rpm;
        self
    }

    /// Check if a request matches this route
    pub fn matches(&self, method: &str, path: &str) -> Option<HashMap<String, String>> {
        if method != self.method.as_str() {
            return None;
        }

        // Simple pattern matching with :param support
        let pattern_parts: Vec<&str> = self.path.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return None;
        }

        let mut params = HashMap::new();

        for (pattern, actual) in pattern_parts.iter().zip(path_parts.iter()) {
            if let Some(param_name) = pattern.strip_prefix(':') {
                // This is a parameter
                params.insert(param_name.to_string(), actual.to_string());
            } else if pattern != actual {
                return None;
            }
        }

        Some(params)
    }
}

/// Incoming request to a challenge route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    /// HTTP method
    pub method: String,
    /// Request path (relative to challenge)
    pub path: String,
    /// URL parameters extracted from path (e.g., :hash -> "abc123")
    pub params: HashMap<String, String>,
    /// Query parameters
    pub query: HashMap<String, String>,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body (JSON)
    pub body: Value,
    /// Authenticated validator hotkey (if any)
    pub auth_hotkey: Option<String>,
}

impl RouteRequest {
    /// Create a new request
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            params: HashMap::new(),
            query: HashMap::new(),
            headers: HashMap::new(),
            body: Value::Null,
            auth_hotkey: None,
        }
    }

    /// Set path parameters
    pub fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = params;
        self
    }

    /// Set query parameters
    pub fn with_query(mut self, query: HashMap<String, String>) -> Self {
        self.query = query;
        self
    }

    /// Set request body
    pub fn with_body(mut self, body: Value) -> Self {
        self.body = body;
        self
    }

    /// Set auth hotkey
    pub fn with_auth(mut self, hotkey: String) -> Self {
        self.auth_hotkey = Some(hotkey);
        self
    }

    /// Get a path parameter
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(|s| s.as_str())
    }

    /// Get a query parameter
    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(|s| s.as_str())
    }

    /// Parse body as type T
    pub fn parse_body<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.body.clone())
    }
}

/// Response from a challenge route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body (JSON)
    pub body: Value,
}

impl RouteResponse {
    /// Create a new response
    pub fn new(status: u16, body: Value) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body,
        }
    }

    /// Create a 200 OK response with JSON body
    pub fn ok(body: Value) -> Self {
        Self::new(200, body)
    }

    /// Create a 200 OK response by serializing data
    pub fn json<T: Serialize>(data: T) -> Self {
        Self::new(200, serde_json::to_value(data).unwrap_or(Value::Null))
    }

    /// Create a 201 Created response
    pub fn created(body: Value) -> Self {
        Self::new(201, body)
    }

    /// Create a 204 No Content response
    pub fn no_content() -> Self {
        Self::new(204, Value::Null)
    }

    /// Create a 400 Bad Request response
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(
            400,
            serde_json::json!({
                "error": "bad_request",
                "message": message.into()
            }),
        )
    }

    /// Create a 401 Unauthorized response
    pub fn unauthorized() -> Self {
        Self::new(
            401,
            serde_json::json!({
                "error": "unauthorized",
                "message": "Authentication required"
            }),
        )
    }

    /// Create a 403 Forbidden response
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            403,
            serde_json::json!({
                "error": "forbidden",
                "message": message.into()
            }),
        )
    }

    /// Create a 404 Not Found response
    pub fn not_found() -> Self {
        Self::new(
            404,
            serde_json::json!({
                "error": "not_found",
                "message": "Route not found"
            }),
        )
    }

    /// Create a 429 Too Many Requests response
    pub fn rate_limited() -> Self {
        Self::new(
            429,
            serde_json::json!({
                "error": "rate_limited",
                "message": "Too many requests"
            }),
        )
    }

    /// Create a 500 Internal Server Error response
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(
            500,
            serde_json::json!({
                "error": "internal_error",
                "message": message.into()
            }),
        )
    }

    /// Add a header to the response
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Check if response is successful (2xx)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// Route registry for a challenge
#[derive(Debug, Clone, Default)]
pub struct RouteRegistry {
    routes: Vec<ChallengeRoute>,
}

impl RouteRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self { routes: vec![] }
    }

    /// Register a route
    pub fn register(&mut self, route: ChallengeRoute) {
        self.routes.push(route);
    }

    /// Register multiple routes
    pub fn register_all(&mut self, routes: Vec<ChallengeRoute>) {
        self.routes.extend(routes);
    }

    /// Find a matching route
    pub fn find_route(
        &self,
        method: &str,
        path: &str,
    ) -> Option<(&ChallengeRoute, HashMap<String, String>)> {
        for route in &self.routes {
            if let Some(params) = route.matches(method, path) {
                return Some((route, params));
            }
        }
        None
    }

    /// Get all registered routes
    pub fn routes(&self) -> &[ChallengeRoute] {
        &self.routes
    }

    /// Check if any routes are registered
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

/// Builder for creating routes fluently
pub struct RouteBuilder {
    routes: Vec<ChallengeRoute>,
}

impl RouteBuilder {
    pub fn new() -> Self {
        Self { routes: vec![] }
    }

    pub fn get(mut self, path: impl Into<String>, desc: impl Into<String>) -> Self {
        self.routes.push(ChallengeRoute::get(path, desc));
        self
    }

    pub fn post(mut self, path: impl Into<String>, desc: impl Into<String>) -> Self {
        self.routes.push(ChallengeRoute::post(path, desc));
        self
    }

    pub fn put(mut self, path: impl Into<String>, desc: impl Into<String>) -> Self {
        self.routes.push(ChallengeRoute::put(path, desc));
        self
    }

    pub fn delete(mut self, path: impl Into<String>, desc: impl Into<String>) -> Self {
        self.routes.push(ChallengeRoute::delete(path, desc));
        self
    }

    pub fn build(self) -> Vec<ChallengeRoute> {
        self.routes
    }
}

impl Default for RouteBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routes_manifest_new() {
        let manifest = RoutesManifest::new("Test Challenge", "1.0.0");
        assert_eq!(manifest.name, "test-challenge"); // normalized
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.routes.is_empty());
    }

    #[test]
    fn test_normalize_name() {
        assert_eq!(
            RoutesManifest::normalize_name("Test Challenge"),
            "test-challenge"
        );
        assert_eq!(
            RoutesManifest::normalize_name("Test_Challenge"),
            "test-challenge"
        );
        assert_eq!(
            RoutesManifest::normalize_name("Test-Challenge-123"),
            "test-challenge-123"
        );
        assert_eq!(RoutesManifest::normalize_name("-test-"), "test");
        // Multiple spaces get replaced with multiple dashes (no collapsing)
        assert_eq!(
            RoutesManifest::normalize_name("Test  Challenge"),
            "test--challenge"
        );
    }

    #[test]
    fn test_routes_manifest_with_description() {
        let manifest = RoutesManifest::new("test", "1.0").with_description("A test challenge");
        assert_eq!(manifest.description, "A test challenge");
    }

    #[test]
    fn test_routes_manifest_add_route() {
        let route = ChallengeRoute::get("/test", "Test route");
        let manifest = RoutesManifest::new("test", "1.0").add_route(route.clone());

        assert_eq!(manifest.routes.len(), 1);
        assert_eq!(manifest.routes[0].path, "/test");
    }

    #[test]
    fn test_routes_manifest_with_routes() {
        let routes = vec![
            ChallengeRoute::get("/route1", "Route 1"),
            ChallengeRoute::post("/route2", "Route 2"),
        ];

        let manifest = RoutesManifest::new("test", "1.0").with_routes(routes);
        assert_eq!(manifest.routes.len(), 2);
    }

    #[test]
    fn test_routes_manifest_with_metadata() {
        let manifest = RoutesManifest::new("test", "1.0")
            .with_metadata("author", serde_json::json!("John Doe"))
            .with_metadata("license", serde_json::json!("MIT"));

        assert_eq!(manifest.metadata.len(), 2);
        assert_eq!(
            manifest.metadata.get("author"),
            Some(&serde_json::json!("John Doe"))
        );
    }

    #[test]
    fn test_http_method_display() {
        assert_eq!(format!("{}", HttpMethod::Get), "GET");
        assert_eq!(format!("{}", HttpMethod::Post), "POST");
        assert_eq!(format!("{}", HttpMethod::Put), "PUT");
        assert_eq!(format!("{}", HttpMethod::Delete), "DELETE");
        assert_eq!(format!("{}", HttpMethod::Patch), "PATCH");
    }

    #[test]
    fn test_http_method_as_str() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
        assert_eq!(HttpMethod::Put.as_str(), "PUT");
        assert_eq!(HttpMethod::Delete.as_str(), "DELETE");
        assert_eq!(HttpMethod::Patch.as_str(), "PATCH");
    }

    #[test]
    fn test_challenge_route_put() {
        let route = ChallengeRoute::put("/update", "Update resource");
        assert_eq!(route.method, HttpMethod::Put);
        assert_eq!(route.path, "/update");
        assert_eq!(route.description, "Update resource");
    }

    #[test]
    fn test_challenge_route_delete() {
        let route = ChallengeRoute::delete("/remove", "Remove resource");
        assert_eq!(route.method, HttpMethod::Delete);
        assert_eq!(route.path, "/remove");
    }

    #[test]
    fn test_challenge_route_with_auth() {
        let route = ChallengeRoute::get("/private", "Private route").with_auth();
        assert!(route.requires_auth);
    }

    #[test]
    fn test_challenge_route_with_rate_limit() {
        let route = ChallengeRoute::post("/submit", "Submit").with_rate_limit(10);
        assert_eq!(route.rate_limit, 10);
    }

    #[test]
    fn test_route_matching() {
        let route = ChallengeRoute::get("/agent/:hash", "Get agent");

        // Should match
        let params = route.matches("GET", "/agent/abc123");
        assert!(params.is_some());
        assert_eq!(params.unwrap().get("hash"), Some(&"abc123".to_string()));

        // Should not match wrong method
        assert!(route.matches("POST", "/agent/abc123").is_none());

        // Should not match wrong path
        assert!(route.matches("GET", "/user/abc123").is_none());
    }

    #[test]
    fn test_route_request_new() {
        let req = RouteRequest::new("GET", "/test");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test");
        assert!(req.params.is_empty());
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_route_request_with_params() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "123".to_string());

        let req = RouteRequest::new("GET", "/test").with_params(params);
        assert_eq!(req.param("id"), Some("123"));
    }

    #[test]
    fn test_route_request_with_query() {
        let mut query = HashMap::new();
        query.insert("limit".to_string(), "10".to_string());

        let req = RouteRequest::new("GET", "/test").with_query(query);
        assert_eq!(req.query_param("limit"), Some("10"));
    }

    #[test]
    fn test_route_request_with_body() {
        let body = serde_json::json!({"key": "value"});
        let req = RouteRequest::new("POST", "/test").with_body(body.clone());
        assert_eq!(req.body, body);
    }

    #[test]
    fn test_route_request_with_auth() {
        let req = RouteRequest::new("GET", "/test").with_auth("hotkey123".to_string());
        assert_eq!(req.auth_hotkey, Some("hotkey123".to_string()));
    }

    #[test]
    fn test_route_request_param() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "abc".to_string());

        let req = RouteRequest::new("GET", "/test").with_params(params);
        assert_eq!(req.param("id"), Some("abc"));
        assert_eq!(req.param("missing"), None);
    }

    #[test]
    fn test_route_request_query_param() {
        let mut query = HashMap::new();
        query.insert("page".to_string(), "2".to_string());

        let req = RouteRequest::new("GET", "/test").with_query(query);
        assert_eq!(req.query_param("page"), Some("2"));
        assert_eq!(req.query_param("missing"), None);
    }

    #[test]
    fn test_route_request_parse_body() {
        #[derive(serde::Deserialize)]
        struct TestData {
            value: i32,
        }

        let body = serde_json::json!({"value": 42});
        let req = RouteRequest::new("POST", "/test").with_body(body);

        let parsed: TestData = req.parse_body().unwrap();
        assert_eq!(parsed.value, 42);
    }

    #[test]
    fn test_route_response_ok() {
        let resp = RouteResponse::ok(serde_json::json!({"status": "ok"}));
        assert_eq!(resp.status, 200);
        assert!(resp.is_success());
    }

    #[test]
    fn test_route_response_created() {
        let resp = RouteResponse::created(serde_json::json!({"id": "123"}));
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
    fn test_route_response_unauthorized() {
        let resp = RouteResponse::unauthorized();
        assert_eq!(resp.status, 401);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_route_response_forbidden() {
        let resp = RouteResponse::forbidden("Access denied");
        assert_eq!(resp.status, 403);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_route_response_rate_limited() {
        let resp = RouteResponse::rate_limited();
        assert_eq!(resp.status, 429);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_route_response_internal_error() {
        let resp = RouteResponse::internal_error("Something went wrong");
        assert_eq!(resp.status, 500);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_route_response_with_header() {
        let resp = RouteResponse::ok(serde_json::json!({})).with_header("X-Custom", "value");

        assert_eq!(resp.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_route_response_is_success() {
        assert!(RouteResponse::ok(serde_json::json!({})).is_success());
        assert!(RouteResponse::created(serde_json::json!({})).is_success());
        assert!(!RouteResponse::bad_request("error").is_success());
        assert!(!RouteResponse::not_found().is_success());
    }

    #[test]
    fn test_route_registry_register_all() {
        let mut registry = RouteRegistry::new();
        let routes = vec![
            ChallengeRoute::get("/a", "Route A"),
            ChallengeRoute::post("/b", "Route B"),
        ];

        registry.register_all(routes);
        assert_eq!(registry.routes().len(), 2);
    }

    #[test]
    fn test_route_registry_routes() {
        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/test", "Test"));

        let routes = registry.routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].path, "/test");
    }

    #[test]
    fn test_route_registry_is_empty() {
        let registry = RouteRegistry::new();
        assert!(registry.is_empty());

        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/test", "Test"));
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_route_builder() {
        let routes = RouteBuilder::new()
            .get("/leaderboard", "Get leaderboard")
            .post("/submit", "Submit result")
            .get("/agent/:hash", "Get agent")
            .build();

        assert_eq!(routes.len(), 3);
    }

    #[test]
    fn test_route_builder_default() {
        let builder = RouteBuilder::default();
        let routes = builder.build();
        assert!(routes.is_empty());
    }

    #[test]
    fn test_route_builder_put() {
        let routes = RouteBuilder::new()
            .put("/update/:id", "Update item")
            .build();

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, HttpMethod::Put);
    }

    #[test]
    fn test_route_builder_delete() {
        let routes = RouteBuilder::new()
            .delete("/remove/:id", "Remove item")
            .build();

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, HttpMethod::Delete);
    }

    #[test]
    fn test_route_registry_new() {
        let registry = RouteRegistry::new();
        assert!(registry.routes.is_empty());
    }

    #[test]
    fn test_route_registry_register() {
        let mut registry = RouteRegistry::new();
        registry.register(ChallengeRoute::get("/test", "Test"));
        registry.register(ChallengeRoute::get("/user/:id", "Get user"));

        let (route, params) = registry.find_route("GET", "/user/123").unwrap();
        assert_eq!(route.path, "/user/:id");
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_response_helpers() {
        let resp = RouteResponse::json(serde_json::json!({"key": "value"}));
        assert_eq!(resp.status, 200);

        let resp = RouteResponse::not_found();
        assert_eq!(resp.status, 404);

        let resp = RouteResponse::bad_request("Invalid input");
        assert_eq!(resp.status, 400);
    }
}
