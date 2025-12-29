//! Authentication and Authorization Module
//!
//! Provides:
//! - SS58 hotkey validation
//! - Sr25519 signature verification
//! - Validator whitelist management
//! - Message creation helpers

use sp_core::crypto::Ss58Codec;
use sp_core::sr25519::{Public, Signature};
use std::collections::HashSet;
use tokio::sync::RwLock;
use tracing::{debug, warn};

// ============================================================================
// SS58 VALIDATION
// ============================================================================

/// Check if a string is a valid SS58-encoded sr25519 public key
pub fn is_valid_ss58_hotkey(hotkey: &str) -> bool {
    if hotkey.len() < 40 || hotkey.len() > 60 {
        return false;
    }
    Public::from_ss58check(hotkey).is_ok()
}

// ============================================================================
// SIGNATURE VERIFICATION
// ============================================================================

/// Verify an sr25519 signature
///
/// # Arguments
/// * `hotkey` - SS58-encoded public key
/// * `message` - The message that was signed (plaintext)
/// * `signature_hex` - Hex-encoded signature (64 bytes = 128 hex chars)
pub fn verify_signature(hotkey: &str, message: &str, signature_hex: &str) -> bool {
    // Parse public key from SS58
    let public_key = match Public::from_ss58check(hotkey) {
        Ok(pk) => pk,
        Err(e) => {
            debug!("Failed to parse SS58 hotkey: {}", e);
            return false;
        }
    };

    // Clean up signature (remove 0x prefix if present)
    let sig_hex = signature_hex
        .strip_prefix("0x")
        .unwrap_or(signature_hex)
        .to_lowercase();

    // Parse signature from hex
    let sig_bytes = match hex::decode(&sig_hex) {
        Ok(b) => b,
        Err(e) => {
            debug!("Failed to decode signature hex: {}", e);
            return false;
        }
    };

    if sig_bytes.len() != 64 {
        debug!(
            "Invalid signature length: {} (expected 64)",
            sig_bytes.len()
        );
        return false;
    }

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&sig_bytes);
    let signature = Signature::from_raw(sig_array);

    // Verify
    use sp_core::Pair;
    let is_valid = sp_core::sr25519::Pair::verify(&signature, message.as_bytes(), &public_key);

    if !is_valid {
        debug!(
            "Signature verification failed for message '{}' with hotkey {}",
            &message[..50.min(message.len())],
            &hotkey[..16.min(hotkey.len())]
        );
    }

    is_valid
}

// ============================================================================
// MESSAGE CREATION HELPERS
// ============================================================================

/// Create the message to sign for submission
pub fn create_submit_message(source_code: &str) -> String {
    use sha2::{Digest, Sha256};
    let source_hash = hex::encode(Sha256::digest(source_code.as_bytes()));
    format!("submit_agent:{}", source_hash)
}

/// Create the message to sign for listing own agents
pub fn create_list_agents_message(timestamp: i64) -> String {
    format!("list_agents:{}", timestamp)
}

/// Create the message to sign for getting own source code
pub fn create_get_source_message(agent_hash: &str, timestamp: i64) -> String {
    format!("get_source:{}:{}", agent_hash, timestamp)
}

/// Create the message to sign for validator claim
pub fn create_claim_message(timestamp: i64) -> String {
    format!("claim_job:{}", timestamp)
}

// ============================================================================
// TIMESTAMP VALIDATION
// ============================================================================

/// Check if a timestamp is within the acceptable window (5 minutes)
pub fn is_timestamp_valid(timestamp: i64) -> bool {
    let now = chrono::Utc::now().timestamp();
    let window = 5 * 60; // 5 minutes
    (now - timestamp).abs() < window
}

// ============================================================================
// VALIDATOR WHITELIST
// ============================================================================

/// Manages the validator whitelist
pub struct AuthManager {
    whitelist: RwLock<HashSet<String>>,
}

impl AuthManager {
    /// Create a new AuthManager with an empty whitelist
    pub fn new() -> Self {
        Self {
            whitelist: RwLock::new(HashSet::new()),
        }
    }

    /// Create a new AuthManager with an initial whitelist
    pub fn with_whitelist(hotkeys: Vec<String>) -> Self {
        let mut set = HashSet::new();
        for hotkey in hotkeys {
            if is_valid_ss58_hotkey(&hotkey) {
                set.insert(hotkey);
            } else {
                warn!("Invalid hotkey in whitelist: {}", hotkey);
            }
        }
        Self {
            whitelist: RwLock::new(set),
        }
    }

    /// Check if a validator is in the whitelist
    pub async fn is_whitelisted_validator(&self, hotkey: &str) -> bool {
        let whitelist = self.whitelist.read().await;
        whitelist.contains(hotkey)
    }

    /// Add a validator to the whitelist
    pub async fn add_validator(&self, hotkey: &str) -> bool {
        if !is_valid_ss58_hotkey(hotkey) {
            warn!("Cannot add invalid hotkey to whitelist: {}", hotkey);
            return false;
        }
        let mut whitelist = self.whitelist.write().await;
        whitelist.insert(hotkey.to_string())
    }

    /// Remove a validator from the whitelist
    pub async fn remove_validator(&self, hotkey: &str) -> bool {
        let mut whitelist = self.whitelist.write().await;
        whitelist.remove(hotkey)
    }

    /// Get all whitelisted validators
    pub async fn get_whitelist(&self) -> Vec<String> {
        let whitelist = self.whitelist.read().await;
        whitelist.iter().cloned().collect()
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ss58_validation() {
        // Valid SS58 address (example Substrate address)
        assert!(is_valid_ss58_hotkey(
            "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
        ));

        // Invalid addresses
        assert!(!is_valid_ss58_hotkey("not_a_valid_address"));
        assert!(!is_valid_ss58_hotkey("da220409678df5f0")); // Hex hash, not SS58
        assert!(!is_valid_ss58_hotkey("0x1234"));
        assert!(!is_valid_ss58_hotkey(""));
    }

    #[test]
    fn test_timestamp_validation() {
        let now = chrono::Utc::now().timestamp();

        // Valid timestamps
        assert!(is_timestamp_valid(now));
        assert!(is_timestamp_valid(now - 60)); // 1 minute ago
        assert!(is_timestamp_valid(now - 240)); // 4 minutes ago

        // Invalid timestamps
        assert!(!is_timestamp_valid(now - 600)); // 10 minutes ago
        assert!(!is_timestamp_valid(now + 600)); // 10 minutes in future
    }

    #[test]
    fn test_message_creation() {
        let source = "print('hello')";
        let msg = create_submit_message(source);
        assert!(msg.starts_with("submit_agent:"));
        assert_eq!(msg.len(), 13 + 64); // "submit_agent:" + sha256 hex

        let list_msg = create_list_agents_message(12345);
        assert_eq!(list_msg, "list_agents:12345");

        let src_msg = create_get_source_message("abc123", 12345);
        assert_eq!(src_msg, "get_source:abc123:12345");
    }

    #[tokio::test]
    async fn test_auth_manager() {
        let auth = AuthManager::new();

        // Initially empty
        assert!(
            !auth
                .is_whitelisted_validator("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .await
        );

        // Add validator
        assert!(
            auth.add_validator("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .await
        );
        assert!(
            auth.is_whitelisted_validator("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .await
        );

        // Cannot add invalid
        assert!(!auth.add_validator("invalid").await);

        // Remove validator
        assert!(
            auth.remove_validator("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .await
        );
        assert!(
            !auth
                .is_whitelisted_validator("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .await
        );
    }
}
