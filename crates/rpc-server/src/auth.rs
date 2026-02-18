//! Authentication for RPC requests
//!
//! Validators authenticate using their hotkey signature (sr25519).

use platform_core::Hotkey;
use sp_core::{crypto::Pair as _, sr25519};
use tracing::warn;

/// Verify a signed message from a validator (sr25519)
pub fn verify_validator_signature(
    hotkey_hex: &str,
    message: &str,
    signature_hex: &str,
) -> Result<bool, AuthError> {
    // Parse hotkey
    let hotkey = Hotkey::from_hex(hotkey_hex).ok_or(AuthError::InvalidHotkey)?;

    // Parse signature
    let signature_bytes = hex::decode(signature_hex).map_err(|_| AuthError::InvalidSignature)?;

    if signature_bytes.len() != 64 {
        return Err(AuthError::InvalidSignature);
    }

    // Verify using sr25519
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&signature_bytes);
    let signature = sr25519::Signature::from_raw(sig_bytes);

    let public = sr25519::Public::from_raw(hotkey.0);
    let is_valid = sr25519::Pair::verify(&signature, message.as_bytes(), &public);

    if !is_valid {
        warn!("Invalid signature for hotkey: {}", &hotkey_hex[..16]);
    }

    Ok(is_valid)
}

/// Create a message for signing
pub fn create_auth_message(action: &str, timestamp: i64, nonce: &str) -> String {
    format!("{}:{}:{}", action, timestamp, nonce)
}

/// Verify message is recent (within 5 minutes)
pub fn verify_timestamp(timestamp: i64) -> bool {
    let now = chrono::Utc::now().timestamp();
    let diff = (now - timestamp).abs();
    diff < 300 // 5 minutes
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid hotkey format")]
    InvalidHotkey,

    #[error("Invalid signature format")]
    InvalidSignature,

    #[error("Signature verification failed")]
    VerificationFailed,

    #[error("Message expired")]
    MessageExpired,
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_core::Keypair;

    #[test]
    fn test_create_auth_message() {
        let msg = create_auth_message("register", 1234567890, "abc123");
        assert_eq!(msg, "register:1234567890:abc123");
    }

    #[test]
    fn test_verify_timestamp() {
        let now = chrono::Utc::now().timestamp();
        assert!(verify_timestamp(now));
        assert!(verify_timestamp(now - 60)); // 1 minute ago
        assert!(!verify_timestamp(now - 600)); // 10 minutes ago
    }

    #[test]
    fn test_signature_verification() {
        let kp = Keypair::generate();
        let message = "test:1234567890:nonce";
        let signed = kp.sign(message.as_bytes());

        let hotkey_hex = kp.hotkey().to_hex();
        let sig_hex = hex::encode(&signed.signature);

        let result = verify_validator_signature(&hotkey_hex, message, &sig_hex);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_signature_verification_invalid_hotkey() {
        let result = verify_validator_signature("invalid_hotkey", "message", "signature");
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_verification_invalid_signature_hex() {
        let kp = Keypair::generate();
        let result = verify_validator_signature(&kp.hotkey().to_hex(), "message", "not_hex");
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_verification_wrong_signature() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let message = "test:1234567890:nonce";
        let signed = kp1.sign(message.as_bytes());

        // Use kp2's hotkey but kp1's signature - should fail
        let hotkey_hex = kp2.hotkey().to_hex();
        let sig_hex = hex::encode(&signed.signature);

        let result = verify_validator_signature(&hotkey_hex, message, &sig_hex);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Signature doesn't match
    }

    #[test]
    fn test_signature_verification_wrong_message() {
        let kp = Keypair::generate();
        let message1 = "test:1234567890:nonce1";
        let message2 = "test:1234567890:nonce2";
        let signed = kp.sign(message1.as_bytes());

        let hotkey_hex = kp.hotkey().to_hex();
        let sig_hex = hex::encode(&signed.signature);

        // Try to verify with different message - should fail
        let result = verify_validator_signature(&hotkey_hex, message2, &sig_hex);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_timestamp_edge_case() {
        let now = chrono::Utc::now().timestamp();
        // Test exactly at 5 minute boundary
        assert!(!verify_timestamp(now - 301)); // 5 minutes 1 second ago
        assert!(verify_timestamp(now - 299)); // 4 minutes 59 seconds ago
    }

    #[test]
    fn test_verify_timestamp_future() {
        let now = chrono::Utc::now().timestamp();
        assert!(verify_timestamp(now + 10)); // Future timestamp within 5 min should be valid
        assert!(verify_timestamp(now + 299)); // Just under 5 minutes in future
    }

    #[test]
    fn test_signature_verification_invalid_length() {
        let kp = Keypair::generate();
        let message = "test:1234567890:nonce";

        // Test with signature that's too short (not 64 bytes)
        let short_sig = hex::encode([0u8; 32]); // Only 32 bytes
        let result = verify_validator_signature(&kp.hotkey().to_hex(), message, &short_sig);
        assert!(result.is_err());

        // Test with signature that's too long
        let long_sig = hex::encode([0u8; 128]); // 128 bytes
        let result = verify_validator_signature(&kp.hotkey().to_hex(), message, &long_sig);
        assert!(result.is_err());
    }
}
