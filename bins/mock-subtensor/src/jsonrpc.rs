//! JSON-RPC 2.0 module - Substrate-compatible RPC handlers

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{trace, warn};

use crate::{state::WeightCommitment, AppState};

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: Value,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn result(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    /// Create an error with data
    #[allow(dead_code)]
    pub fn error_with_data(id: Value, code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
            id,
        }
    }
}

// Standard JSON-RPC error codes
#[allow(dead_code)]
pub const PARSE_ERROR: i32 = -32700;
#[allow(dead_code)]
pub const INVALID_REQUEST: i32 = -32600;
#[allow(dead_code)]
pub const METHOD_NOT_FOUND: i32 = -32601;
#[allow(dead_code)]
pub const INVALID_PARAMS: i32 = -32602;
#[allow(dead_code)]
pub const INTERNAL_ERROR: i32 = -32603;

/// RPC Handler
pub struct RpcHandler {
    state: Arc<AppState>,
}

impl RpcHandler {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Handle a JSON-RPC request
    pub fn handle(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        trace!("RPC: {}", req.method);

        // Verify JSON-RPC version
        if req.jsonrpc != "2.0" {
            return JsonRpcResponse::error(req.id, INVALID_REQUEST, "Invalid JSON-RPC version");
        }

        // Route to appropriate handler
        match req.method.as_str() {
            // System namespace
            "system_health" => self.system_health(req.id),
            "system_version" => self.system_version(req.id),
            "system_name" => self.system_name(req.id),
            "system_properties" => self.system_properties(req.id),
            "system_peers" => self.system_peers(req.id),
            "system_chain" => self.system_chain(req.id),
            "system_chainType" => self.system_chain_type(req.id),
            "system_syncState" => self.system_sync_state(req.id),
            "system_addLogFilter" => self.system_add_log_filter(req.id, req.params),

            // Chain namespace
            "chain_getHeader" => self.chain_get_header(req.id, req.params),
            "chain_getHead" => self.chain_get_head(req.id),
            "chain_getBlock" => self.chain_get_block(req.id, req.params),
            "chain_getBlockHash" => self.chain_get_block_hash(req.id, req.params),
            "chain_getFinalizedHead" => self.chain_get_finalized_head(req.id),
            "chain_getFinalizedBlock" => self.chain_get_finalized_block(req.id),
            "chain_subscribeNewHeads" => self.chain_subscribe_new_heads(req.id),
            "chain_subscribeFinalizedHeads" => self.chain_subscribe_finalized_heads(req.id),
            "chain_unsubscribeNewHeads" => self.chain_unsubscribe_new_heads(req.id, req.params),
            "chain_unsubscribeFinalizedHeads" => {
                self.chain_unsubscribe_finalized_heads(req.id, req.params)
            }

            // State namespace
            "state_getStorage" => self.state_get_storage(req.id, req.params),
            "state_getKeys" => self.state_get_keys(req.id, req.params),
            "state_getKeysPaged" => self.state_get_keys_paged(req.id, req.params),
            "state_getMetadata" => self.state_get_metadata(req.id),
            "state_getRuntimeVersion" => self.state_get_runtime_version(req.id),
            "state_subscribeStorage" => self.state_subscribe_storage(req.id, req.params),
            "state_unsubscribeStorage" => self.state_unsubscribe_storage(req.id, req.params),
            "state_queryStorageAt" => self.state_query_storage_at(req.id, req.params),

            // Author namespace
            "author_submitExtrinsic" => self.author_submit_extrinsic(req.id, req.params),
            "author_pendingExtrinsics" => self.author_pending_extrinsics(req.id),
            "author_submitAndWatchExtrinsic" => {
                self.author_submit_and_watch_extrinsic(req.id, req.params)
            }

            // Subtensor-specific methods
            "subtensor_getNeurons" => self.subtensor_get_neurons(req.id, req.params),
            "subtensor_getNeuronLite" => self.subtensor_get_neuron_lite(req.id, req.params),
            "subtensor_getSubnetInfo" => self.subtensor_get_subnet_info(req.id, req.params),
            "subtensor_getBalance" => self.subtensor_get_balance(req.id, req.params),
            "subtensor_commitWeights" => self.subtensor_commit_weights(req.id, req.params),
            "subtensor_revealWeights" => self.subtensor_reveal_weights(req.id, req.params),

            // Bittensor compatibility
            "bt_getNeurons" => self.subtensor_get_neurons(req.id, req.params),
            "bt_getBalance" => self.subtensor_get_balance(req.id, req.params),

            // RPC discovery
            "rpc_methods" => self.rpc_methods(req.id),
            "rpc_discover" => self.rpc_discover(req.id),

            _ => {
                warn!("Unknown RPC method: {}", req.method);
                JsonRpcResponse::error(
                    req.id,
                    METHOD_NOT_FOUND,
                    format!("Method not found: {}", req.method),
                )
            }
        }
    }

    // ==================== System Namespace ====================

    fn system_health(&self, id: Value) -> JsonRpcResponse {
        let _chain = self.state.chain.read();
        let metagraph = self.state.metagraph.read();

        JsonRpcResponse::result(
            id,
            json!({
                "peers": metagraph.validators.len(),
                "isSyncing": false,
                "shouldHavePeers": true,
                "genesisHash": format!("0x{}", hex::encode([0u8; 32])),
            }),
        )
    }

    fn system_version(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!("mock-subtensor/1.0.0"))
    }

    fn system_name(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!("mock-subtensor"))
    }

    fn system_properties(&self, id: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();
        JsonRpcResponse::result(id, chain.properties())
    }

    fn system_peers(&self, id: Value) -> JsonRpcResponse {
        let metagraph = self.state.metagraph.read();
        let peers: Vec<Value> = metagraph
            .validators
            .values()
            .filter(|v| v.validator_permit)
            .map(|v| {
                json!({
                    "peerId": format!("12D3KooW{}", &v.hotkey[4..16]),
                    "roles": "FULL",
                    "bestHash": format!("0x{}", hex::encode(&v.hotkey.as_bytes()[0..4])),
                    "bestNumber": 0,
                })
            })
            .collect();

        JsonRpcResponse::result(id, json!(peers))
    }

    fn system_chain(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!("Bittensor"))
    }

    fn system_chain_type(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!("Live"))
    }

    fn system_sync_state(&self, id: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();
        JsonRpcResponse::result(
            id,
            json!({
                "startingBlock": 0,
                "currentBlock": chain.best_number(),
                "highestBlock": chain.best_number(),
                "syncPeer": 1,
                "warpSyncProgress": null,
            }),
        )
    }

    fn system_add_log_filter(&self, id: Value, _params: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!(null))
    }

    // ==================== Chain Namespace ====================

    fn chain_get_head(&self, id: Value) -> JsonRpcResponse {
        self.chain_get_header(id, Value::Null)
    }

    fn chain_get_header(&self, id: Value, params: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();

        // Parse block hash parameter if provided
        let block_number = if let Some(hash) = params.get(0).and_then(|h| h.as_str()) {
            chain
                .get_block_by_hash(hash)
                .map(|b| b.header.number)
                .unwrap_or_else(|| chain.best_number())
        } else {
            chain.best_number()
        };

        let block = chain
            .get_block(block_number)
            .cloned()
            .unwrap_or_else(|| chain.get_block(0).cloned().unwrap());

        JsonRpcResponse::result(
            id,
            json!({
                "number": block.header.number,
                "hash": format!("0x{}", hex::encode(block.hash)),
                "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
                "stateRoot": format!("0x{}", hex::encode(block.header.state_root)),
                "extrinsicsRoot": format!("0x{}", hex::encode(block.header.extrinsics_root)),
                "digest": {
                    "logs": []
                },
            }),
        )
    }

    fn chain_get_block(&self, id: Value, params: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();

        let block_number = if let Some(hash) = params.get(0).and_then(|h| h.as_str()) {
            chain
                .get_block_by_hash(hash)
                .map(|b| b.header.number)
                .unwrap_or(0)
        } else {
            chain.best_number()
        };

        let block = match chain.get_block(block_number) {
            Some(b) => b,
            None => return JsonRpcResponse::result(id, Value::Null),
        };

        let extrinsics: Vec<String> = block
            .extrinsics
            .iter()
            .map(|e| format!("0x{}", hex::encode(e.hash)))
            .collect();

        JsonRpcResponse::result(
            id,
            json!({
                "block": {
                    "header": {
                        "number": block.header.number,
                        "hash": format!("0x{}", hex::encode(block.hash)),
                        "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
                        "stateRoot": format!("0x{}", hex::encode(block.header.state_root)),
                        "extrinsicsRoot": format!("0x{}", hex::encode(block.header.extrinsics_root)),
                        "digest": {
                            "logs": []
                        },
                    },
                    "extrinsics": extrinsics,
                },
                "justifications": null,
            }),
        )
    }

    fn chain_get_block_hash(&self, id: Value, params: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();

        let block_number = params
            .get(0)
            .and_then(|n| n.as_u64())
            .unwrap_or_else(|| chain.best_number());

        match chain.get_block(block_number) {
            Some(block) => {
                JsonRpcResponse::result(id, json!(format!("0x{}", hex::encode(block.hash))))
            }
            None => JsonRpcResponse::result(id, Value::Null),
        }
    }

    fn chain_get_finalized_head(&self, id: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();
        let finalized = chain.finalized_number();

        match chain.get_block(finalized) {
            Some(block) => {
                JsonRpcResponse::result(id, json!(format!("0x{}", hex::encode(block.hash))))
            }
            None => JsonRpcResponse::result(id, Value::Null),
        }
    }

    fn chain_get_finalized_block(&self, id: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();
        let finalized = chain.finalized_number();

        match chain.get_block(finalized) {
            Some(b) => JsonRpcResponse::result(
                id,
                json!({
                    "number": b.header.number,
                    "hash": format!("0x{}", hex::encode(b.hash)),
                }),
            ),
            None => JsonRpcResponse::result(id, Value::Null),
        }
    }

    fn chain_subscribe_new_heads(&self, id: Value) -> JsonRpcResponse {
        let subscription_id = format!("{:?}", id);
        JsonRpcResponse::result(
            id,
            json!({
                "subscription": subscription_id,
                "result": null,
            }),
        )
    }

    fn chain_subscribe_finalized_heads(&self, id: Value) -> JsonRpcResponse {
        let subscription_id = format!("{:?}", id);
        JsonRpcResponse::result(
            id,
            json!({
                "subscription": subscription_id,
                "result": null,
            }),
        )
    }

    fn chain_unsubscribe_new_heads(&self, id: Value, _params: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!(true))
    }

    fn chain_unsubscribe_finalized_heads(&self, id: Value, _params: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!(true))
    }

    // ==================== State Namespace ====================

    fn state_get_storage(&self, id: Value, params: Value) -> JsonRpcResponse {
        let key = params.get(0).and_then(|k| k.as_str());

        if key.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing storage key");
        }

        let key = key.unwrap();
        let _chain = self.state.chain.read();
        let metagraph = self.state.metagraph.read();

        // Parse storage key (simplified)
        let result = if key.starts_with("0x") && key.len() > 66 && key.contains("Balances") {
            // Balance query - return mock balance
            json!(format!("0x{:016x}", 1000000000000u64))
        } else if key.contains("SubtensorModule") {
            // Subtensor storage query
            if key.contains("Neurons") {
                let neurons = metagraph.get_neurons();
                json!(hex::encode(
                    serde_json::to_vec(&neurons).unwrap_or_default()
                ))
            } else if key.contains("Uids") {
                json!(hex::encode(
                    (metagraph.validators.len() as u16).to_le_bytes()
                ))
            } else {
                json!(null)
            }
        } else {
            json!(null)
        };

        JsonRpcResponse::result(id, result)
    }

    fn state_get_keys(&self, id: Value, params: Value) -> JsonRpcResponse {
        let prefix = params.get(0).and_then(|p| p.as_str()).unwrap_or("");

        // Return some mock keys
        let keys: Vec<String> = vec![
            format!("{}/Balances/TotalIssuance", prefix),
            format!("{}/SubtensorModule/Uids", prefix),
            format!("{}/System/BlockHash", prefix),
        ];

        JsonRpcResponse::result(id, json!(keys))
    }

    fn state_get_keys_paged(&self, id: Value, params: Value) -> JsonRpcResponse {
        self.state_get_keys(id, params)
    }

    fn state_get_metadata(&self, id: Value) -> JsonRpcResponse {
        let _chain = self.state.chain.read();
        let metagraph = self.state.metagraph.read();

        JsonRpcResponse::result(
            id,
            json!({
                "version": 14,
                "modules": [
                    {
                        "name": "System",
                        "storage": [{"name": "BlockHash", "modifier": "Default", "type": "map"}],
                        "calls": [],
                        "events": [],
                        "constants": [],
                        "errors": [],
                    },
                    {
                        "name": "Balances",
                        "storage": [{"name": "TotalIssuance", "modifier": "Default", "type": "value"}],
                        "calls": ["transfer", "set_balance"],
                        "events": [],
                        "constants": [],
                        "errors": [],
                    },
                    {
                        "name": "SubtensorModule",
                        "storage": [
                            {"name": "Neurons", "modifier": "Default", "type": "map"},
                            {"name": "Uids", "modifier": "Default", "type": "value"},
                        ],
                        "calls": ["add_stake", "remove_stake", "commit_weights", "reveal_weights"],
                        "events": [],
                        "constants": [],
                        "errors": [],
                    },
                ],
                "extrinsic": {
                    "version": 4,
                    "signedExtensions": [
                        "CheckSpecVersion",
                        "CheckTxVersion",
                        "CheckGenesis",
                        "CheckMortality",
                        "CheckNonce",
                        "CheckWeight",
                        "ChargeTransactionPayment",
                    ],
                },
                "netuid": self.state.config.netuid,
                "tempo": self.state.config.tempo,
                "validator_count": metagraph.validator_count,
            }),
        )
    }

    fn state_get_runtime_version(&self, id: Value) -> JsonRpcResponse {
        let _chain = self.state.chain.read();
        JsonRpcResponse::result(id, self.state.chain.read().runtime_version())
    }

    fn state_subscribe_storage(&self, id: Value, _params: Value) -> JsonRpcResponse {
        let subscription_id = format!("{:?}", id);
        JsonRpcResponse::result(
            id,
            json!({
                "subscription": subscription_id,
                "result": null,
            }),
        )
    }

    fn state_unsubscribe_storage(&self, id: Value, _params: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(id, json!(true))
    }

    fn state_query_storage_at(&self, id: Value, params: Value) -> JsonRpcResponse {
        let keys = params.get(0).and_then(|k| k.as_array());

        if keys.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing keys array");
        }

        let results: Vec<Value> = keys
            .unwrap()
            .iter()
            .map(|k| {
                json!({
                    "block": format!("0x{}", hex::encode([0u8; 32])),
                    "key": k,
                    "value": null,
                })
            })
            .collect();

        JsonRpcResponse::result(id, json!(results))
    }

    // ==================== Author Namespace ====================

    fn author_submit_extrinsic(&self, id: Value, params: Value) -> JsonRpcResponse {
        let extrinsic_data = params.get(0).and_then(|e| e.as_str());

        if extrinsic_data.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing extrinsic data");
        }

        let extrinsic_hex = extrinsic_data.unwrap();
        let mut chain = self.state.chain.write();

        // Parse extrinsic (simplified)
        let method = "author_submitExtrinsic";
        let params = json!({
            "extrinsic": extrinsic_hex,
        });

        let extrinsic = chain.submit_extrinsic(method, params);

        JsonRpcResponse::result(id, json!(format!("0x{}", hex::encode(extrinsic.hash))))
    }

    fn author_pending_extrinsics(&self, id: Value) -> JsonRpcResponse {
        let chain = self.state.chain.read();

        let extrinsics: Vec<String> = chain
            .pending_extrinsics
            .iter()
            .map(|e| format!("0x{}", hex::encode(e.hash)))
            .collect();

        JsonRpcResponse::result(id, json!(extrinsics))
    }

    fn author_submit_and_watch_extrinsic(&self, id: Value, params: Value) -> JsonRpcResponse {
        // Same as submit_extrinsic but returns subscription
        self.author_submit_extrinsic(id, params)
    }

    // ==================== Subtensor Namespace ====================

    fn subtensor_get_neurons(&self, id: Value, params: Value) -> JsonRpcResponse {
        let netuid = params.get(0).and_then(|n| n.as_u64()).unwrap_or(0) as u16;

        if netuid != self.state.config.netuid {
            return JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                format!("NetUID {} not found", netuid),
            );
        }

        let metagraph = self.state.metagraph.read();
        let neurons = metagraph.get_neurons();

        JsonRpcResponse::result(id, json!(neurons))
    }

    fn subtensor_get_neuron_lite(&self, id: Value, params: Value) -> JsonRpcResponse {
        let netuid = params.get(0).and_then(|n| n.as_u64()).unwrap_or(0) as u16;
        let uid = params.get(1).and_then(|u| u.as_u64()).map(|u| u as u16);

        let metagraph = self.state.metagraph.read();

        if netuid != self.state.config.netuid {
            return JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                format!("NetUID {} not found", netuid),
            );
        }

        if let Some(uid) = uid {
            match metagraph.get_validator(uid) {
                Some(v) => JsonRpcResponse::result(id, v.to_neuron_info_lite()),
                None => JsonRpcResponse::error(id, INVALID_PARAMS, "UID not found"),
            }
        } else {
            // Return all neurons lite
            let neurons = metagraph.get_neurons_lite();
            JsonRpcResponse::result(id, json!(neurons))
        }
    }

    fn subtensor_get_subnet_info(&self, id: Value, params: Value) -> JsonRpcResponse {
        let netuid = params.get(0).and_then(|n| n.as_u64()).unwrap_or(0) as u16;
        let metagraph = self.state.metagraph.read();
        let chain = self.state.chain.read();

        if netuid != self.state.config.netuid {
            return JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                format!("NetUID {} not found", netuid),
            );
        }

        JsonRpcResponse::result(
            id,
            json!({
                "netuid": netuid,
                "rho": 10,
                "kappa": 32767,
                "min_allowed_weights": 8,
                "max_weights_limit": 512,
                "tempo": metagraph.tempo,
                "difficulty": 1000000000000u64,
                "immunity_period": 7200,
                "max_allowed_validators": metagraph.validator_count,
                "min_allowed_uids": 8,
                "max_allowed_uids": metagraph.max_uids,
                "blocks_since_last_step": 0,
                "blocks_until_next_epoch": 100 - (chain.best_number() % 100),
                "activity_cutoff": 5000,
                "max_stake": u64::MAX,
                "min_stake": metagraph.min_stake,
                "total_stake": metagraph.total_stake,
            }),
        )
    }

    fn subtensor_get_balance(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = params.get(0).and_then(|a| a.as_str());

        if address.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing address");
        }

        // Return mock balance (100 TAO)
        JsonRpcResponse::result(
            id,
            json!(100000000000u64), // 100 TAO in RAO
        )
    }

    fn subtensor_commit_weights(&self, id: Value, params: Value) -> JsonRpcResponse {
        // Parse parameters
        let netuid = params.get(0).and_then(|n| n.as_u64()).unwrap_or(0) as u16;
        let uids = params.get(1).and_then(|u| u.as_array());
        let commitment_hash = params.get(2).and_then(|c| c.as_str());
        let hotkey = params.get(3).and_then(|h| h.as_str());

        if hotkey.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing hotkey");
        }

        let mut metagraph = self.state.metagraph.write();
        let chain = self.state.chain.read();

        // Verify hotkey is registered
        if metagraph.get_validator_by_hotkey(hotkey.unwrap()).is_none() {
            return JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                format!("Hotkey {} not registered", hotkey.unwrap()),
            );
        }

        // Create commitment
        let uids: Vec<u16> = uids
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_u64().map(|u| u as u16))
            .collect();

        let commitment = WeightCommitment::new(
            hotkey.unwrap().to_string(),
            netuid,
            uids.clone(),
            vec![65535; uids.len()], // Mock weights
            commitment_hash.unwrap_or("").to_string(),
            chain.best_number(),
        );

        match metagraph.commit_weights(commitment) {
            Ok(_) => JsonRpcResponse::result(id, json!(true)),
            Err(e) => JsonRpcResponse::error(id, INTERNAL_ERROR, e),
        }
    }

    fn subtensor_reveal_weights(&self, id: Value, params: Value) -> JsonRpcResponse {
        // Parse parameters
        let _netuid = params.get(0).and_then(|n| n.as_u64()).unwrap_or(0) as u16;
        let uids = params.get(1).and_then(|u| u.as_array());
        let weights = params.get(2).and_then(|w| w.as_array());
        let salt = params.get(3).and_then(|s| s.as_str());
        let hotkey = params.get(4).and_then(|h| h.as_str());

        if hotkey.is_none() {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing hotkey");
        }

        let mut metagraph = self.state.metagraph.write();
        let chain = self.state.chain.read();

        let uids: Vec<u16> = uids
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_u64().map(|u| u as u16))
            .collect();

        let weights: Vec<u16> = weights
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_u64().map(|u| u as u16))
            .collect();

        match metagraph.reveal_weights(
            hotkey.unwrap(),
            uids,
            weights,
            salt.unwrap_or("").to_string(),
            chain.best_number(),
        ) {
            Ok(_) => JsonRpcResponse::result(id, json!(true)),
            Err(e) => JsonRpcResponse::error(id, INTERNAL_ERROR, e),
        }
    }

    // ==================== RPC Discovery ====================

    fn rpc_methods(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::result(
            id,
            json!({
                "methods": [
                    "system_health",
                    "system_version",
                    "system_name",
                    "system_properties",
                    "system_peers",
                    "system_chain",
                    "system_chainType",
                    "chain_getHeader",
                    "chain_getHead",
                    "chain_getBlock",
                    "chain_getBlockHash",
                    "chain_getFinalizedHead",
                    "chain_getFinalizedBlock",
                    "chain_subscribeNewHeads",
                    "chain_subscribeFinalizedHeads",
                    "state_getStorage",
                    "state_getKeys",
                    "state_getKeysPaged",
                    "state_getMetadata",
                    "state_getRuntimeVersion",
                    "state_subscribeStorage",
                    "state_unsubscribeStorage",
                    "state_queryStorageAt",
                    "author_submitExtrinsic",
                    "author_pendingExtrinsics",
                    "subtensor_getNeurons",
                    "subtensor_getNeuronLite",
                    "subtensor_getSubnetInfo",
                    "subtensor_getBalance",
                    "subtensor_commitWeights",
                    "subtensor_revealWeights",
                    "rpc_methods",
                    "rpc_discover",
                ],
                "version": 1,
            }),
        )
    }

    fn rpc_discover(&self, id: Value) -> JsonRpcResponse {
        // OpenRPC discovery
        JsonRpcResponse::result(
            id,
            json!({
                "openrpc": "1.0.0",
                "info": {
                    "title": "Mock Subtensor RPC",
                    "version": "1.0.0",
                },
                "methods": [
                    {
                        "name": "system_health",
                        "description": "Returns the current health status of the node",
                        "params": [],
                        "result": {"name": "health", "schema": {"type": "object"}},
                    },
                    {
                        "name": "chain_getBlock",
                        "description": "Get block by hash or number",
                        "params": [{"name": "hash", "schema": {"type": "string"}}],
                        "result": {"name": "block", "schema": {"type": "object"}},
                    },
                    {
                        "name": "subtensor_getNeurons",
                        "description": "Get all neurons for a subnet",
                        "params": [{"name": "netuid", "schema": {"type": "integer"}}],
                        "result": {"name": "neurons", "schema": {"type": "array"}},
                    },
                ],
            }),
        )
    }
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

    #[test]
    fn test_system_health() {
        let state = test_state();
        let handler = RpcHandler::new(state);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "system_health".to_string(),
            params: json!(null),
            id: json!(1),
        };

        let resp = handler.handle(req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_chain_get_head() {
        let state = test_state();
        let handler = RpcHandler::new(state);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "chain_getHead".to_string(),
            params: json!(null),
            id: json!(1),
        };

        let resp = handler.handle(req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_subtensor_get_neurons() {
        let state = test_state();
        let handler = RpcHandler::new(state);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "subtensor_getNeurons".to_string(),
            params: json!([100]),
            id: json!(1),
        };

        let resp = handler.handle(req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_unknown_method() {
        let state = test_state();
        let handler = RpcHandler::new(state);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "unknown_method".to_string(),
            params: json!(null),
            id: json!(1),
        };

        let resp = handler.handle(req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_rpc_methods() {
        let state = test_state();
        let handler = RpcHandler::new(state);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "rpc_methods".to_string(),
            params: json!(null),
            id: json!(1),
        };

        let resp = handler.handle(req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let methods = result["methods"].as_array().unwrap();
        assert!(methods.len() > 10);
    }
}
