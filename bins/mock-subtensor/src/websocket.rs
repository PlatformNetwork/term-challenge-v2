//! WebSocket server module - Handles WebSocket connections for JSON-RPC 2.0

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};

use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, RpcHandler};
use crate::AppState;

/// WebSocket server
pub struct WsServer {
    state: Arc<AppState>,
}

impl WsServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Build the router with all routes
    fn router(&self) -> Router {
        let state = self.state.clone();

        let mut router = Router::new()
            // WebSocket endpoint (primary for Substrate)
            .route("/", any(ws_handler))
            // HTTP RPC endpoint for compatibility
            .route("/rpc", post(post_rpc_handler))
            .route("/jsonrpc", post(post_rpc_handler))
            // Test inspection endpoints
            .route("/test/state", get(get_state_handler))
            .route("/test/metagraph", get(get_metagraph_handler))
            .route("/test/weights", get(get_weights_handler))
            .route("/test/advance", post(post_advance_handler))
            .route("/health", get(health_handler))
            .with_state(state)
            .layer(TraceLayer::new_for_http());

        // CORS
        router = router.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

        router
    }

    /// Run the server
    pub async fn run(self, addr: SocketAddr) -> anyhow::Result<()> {
        let router = self.router();

        info!("Mock Subtensor WebSocket server starting on {}", addr);
        info!("  - WebSocket: ws://{}/", addr);
        info!("  - HTTP RPC: http://{}/rpc", addr);
        info!("  - Health: http://{}/health", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}

/// WebSocket handler
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    info!("WebSocket connection from {}", addr);
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let handler = Arc::new(RpcHandler::new(state.clone()));

    // Subscribe to block notifications
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Spawn task to handle outgoing messages
    let tx_for_send = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle broadcast messages
                Ok(notification) = broadcast_rx.recv() => {
                    if let Ok(msg) = serde_json::to_string(&notification) {
                        if tx_for_send.send(Message::Text(msg)).is_err() {
                            break;
                        }
                    }
                }
                // Handle direct messages from main task
                Some(msg) = rx.recv() => {
                    if tx_for_send.send(msg).is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Process incoming messages
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received: {}", text);

                // Parse JSON-RPC request
                match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        // Handle batch requests
                        if let Some(array) = value.as_array() {
                            let mut responses = Vec::new();
                            for item in array {
                                let req =
                                    match serde_json::from_value::<JsonRpcRequest>(item.clone()) {
                                        Ok(r) => r,
                                        Err(e) => {
                                            responses.push(JsonRpcResponse::error(
                                                item.get("id").cloned().unwrap_or(Value::Null),
                                                -32700,
                                                format!("Parse error: {}", e),
                                            ));
                                            continue;
                                        }
                                    };
                                let resp = handler.handle(req);
                                responses.push(resp);
                            }

                            if let Ok(json) = serde_json::to_string(&responses) {
                                if tx.send(Message::Text(json)).is_err() {
                                    break;
                                }
                            }
                        } else {
                            // Single request
                            let req = match serde_json::from_value::<JsonRpcRequest>(value) {
                                Ok(r) => r,
                                Err(e) => {
                                    let resp = JsonRpcResponse::error(
                                        Value::Null,
                                        -32700,
                                        format!("Parse error: {}", e),
                                    );
                                    if let Ok(json) = serde_json::to_string(&resp) {
                                        let _ = tx.send(Message::Text(json));
                                    }
                                    continue;
                                }
                            };

                            let resp = handler.handle(req);
                            if let Ok(json) = serde_json::to_string(&resp) {
                                if tx.send(Message::Text(json)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse JSON: {}", e);
                        let resp = JsonRpcResponse::error(
                            Value::Null,
                            -32700,
                            format!("Parse error: {}", e),
                        );
                        if let Ok(json) = serde_json::to_string(&resp) {
                            let _ = tx.send(Message::Text(json));
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                debug!("Client closed connection");
                break;
            }
            Ok(Message::Ping(data)) => {
                if tx.send(Message::Pong(data)).is_err() {
                    break;
                }
            }
            Err(e) => {
                warn!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    debug!("WebSocket connection closed");
}

/// HTTP POST handler for JSON-RPC
async fn post_rpc_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let handler = RpcHandler::new(state.clone());

    // Handle batch requests
    if let Some(array) = body.as_array() {
        let mut responses = Vec::new();
        for item in array {
            let req = match serde_json::from_value::<crate::jsonrpc::JsonRpcRequest>(item.clone()) {
                Ok(r) => r,
                Err(e) => {
                    responses.push(JsonRpcResponse::error(
                        item.get("id").cloned().unwrap_or(Value::Null),
                        -32700,
                        format!("Parse error: {}", e),
                    ));
                    continue;
                }
            };
            let resp = handler.handle(req);
            responses.push(resp);
        }

        (StatusCode::OK, Json(json!(responses)))
    } else {
        // Single request
        let req = match serde_json::from_value::<crate::jsonrpc::JsonRpcRequest>(body) {
            Ok(r) => r,
            Err(e) => {
                let resp =
                    JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {}", e));
                return (StatusCode::OK, Json(json!(resp)));
            }
        };

        let resp = handler.handle(req);
        (StatusCode::OK, Json(json!(resp)))
    }
}

/// Health check handler
async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let chain = state.chain.read();
    let metagraph = state.metagraph.read();

    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "block_number": chain.best_number(),
            "finalized_number": chain.finalized_number(),
            "validator_count": metagraph.validators.len(),
            "netuid": state.config.netuid,
            "tempo": state.config.tempo,
        })),
    )
}

/// Get current chain state
async fn get_state_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let chain = state.chain.read();

    (
        StatusCode::OK,
        Json(json!({
            "best_number": chain.best_number(),
            "finalized_number": chain.finalized_number(),
            "pending_extrinsics": chain.pending_extrinsics.len(),
            "config": {
                "tempo": chain.config.tempo,
                "netuid": chain.config.netuid,
                "commit_reveal": chain.config.commit_reveal,
                "reveal_period": chain.config.reveal_period,
                "token_decimals": chain.config.token_decimals,
                "ss58_format": chain.config.ss58_format,
            },
        })),
    )
}

/// Get metagraph information
async fn get_metagraph_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let metagraph = state.metagraph.read();

    (StatusCode::OK, Json(metagraph.get_summary()))
}

/// Get weight commitments
async fn get_weights_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let metagraph = state.metagraph.read();

    let pending: Vec<_> = metagraph
        .get_pending_commits()
        .iter()
        .map(|c| {
            json!({
                "hotkey": c.hotkey,
                "netuid": c.netuid,
                "uids": c.uids,
                "commitment_hash": c.commitment_hash,
                "commit_block": c.commit_block,
                "revealed": c.revealed,
            })
        })
        .collect();

    let revealed: Vec<_> = metagraph
        .get_revealed_commits()
        .iter()
        .map(|c| {
            json!({
                "hotkey": c.hotkey,
                "netuid": c.netuid,
                "uids": c.uids,
                "weights": c.revealed_weights,
                "reveal_block": c.reveal_block,
                "revealed": c.revealed,
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "pending": pending,
            "revealed": revealed,
            "total_pending": pending.len(),
            "total_revealed": revealed.len(),
        })),
    )
}

/// Advance block manually
async fn post_advance_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut chain = state.chain.write();
    let block = chain.produce_block();
    let block_number = block.header.number;

    // Broadcast notification
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "chain_newHead",
        "params": {
            "result": {
                "number": block_number,
                "hash": format!("0x{}", hex::encode(block.hash)),
            },
            "subscription": "chain"
        }
    });

    drop(chain);
    let _ = state.broadcast_tx.send(notification);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "block_number": block_number,
            "block_hash": format!("0x{}", hex::encode(block.hash)),
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> Arc<AppState> {
        let config = crate::Config {
            bind: "127.0.0.1:9944".parse().unwrap(),
            tempo: 12,
            netuid: 100,
            validator_count: 256,
            min_stake: 1_000_000_000_000,
            commit_reveal: true,
            reveal_period: 12,
            log_level: "info".to_string(),
            inspection: true,
        };
        Arc::new(AppState::new(config))
    }

    #[tokio::test]
    async fn test_router_creation() {
        let state = test_state();
        let server = WsServer::new(state);
        let _router = server.router();

        let _ = _router;
    }

    #[tokio::test]
    async fn test_health_handler() {
        let state = test_state();
        let _response = health_handler(State(state)).await;

        let _ = _response;
    }

    #[tokio::test]
    async fn test_get_state_handler() {
        let state = test_state();
        let _response = get_state_handler(State(state)).await;

        let _ = _response;
    }

    #[tokio::test]
    async fn test_get_metagraph_handler() {
        let state = test_state();
        let _response = get_metagraph_handler(State(state)).await;

        let _ = _response;
    }

    #[tokio::test]
    async fn test_advance_block() {
        let state = test_state();
        let initial_number = state.chain.read().best_number();

        let _response = post_advance_handler(State(state.clone())).await;

        // Verify block advanced
        let new_number = state.chain.read().best_number();
        assert!(new_number > initial_number);
    }

    #[tokio::test]
    async fn test_post_rpc_handler() {
        let state = test_state();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "system_health",
            "params": [],
            "id": 1
        });

        let response = post_rpc_handler(State(state), Json(request)).await;

        let response = response.into_response();
        assert!(response.status().is_success());
    }
}
