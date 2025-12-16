//! Submission Manager for Term-Challenge
//!
//! Implements the commit-reveal submission protocol specific to term-challenge.
//! This handles:
//! - Encrypted submission tracking
//! - Stake-weighted quorum for ACKs
//! - Key reveal and ownership verification
//! - Duplicate content detection
//! - Ban list management

use platform_challenge_sdk::{
    decrypt_data, DecryptionKeyReveal, EncryptedSubmission, SubmissionAck, SubmissionError,
    VerifiedSubmission,
};
use platform_core::Hotkey;
use std::collections::{HashMap, HashSet};

/// State of a pending submission
#[derive(Clone, Debug)]
pub enum SubmissionState {
    /// Waiting for validator acknowledgments
    WaitingForAcks {
        submission: EncryptedSubmission,
        acks: HashMap<Hotkey, SubmissionAck>,
        total_stake_acked: u64,
        total_network_stake: u64,
    },
    /// Quorum reached, waiting for key reveal
    WaitingForKey {
        submission: EncryptedSubmission,
        acks: HashMap<Hotkey, SubmissionAck>,
    },
    /// Key revealed, submission verified
    Verified(VerifiedSubmission),
    /// Failed (timeout, invalid key, etc.)
    Failed { reason: String },
}

impl SubmissionState {
    /// Check if stake-weighted quorum (>= 50%) has been reached
    pub fn has_quorum(&self) -> bool {
        match self {
            Self::WaitingForAcks {
                total_stake_acked,
                total_network_stake,
                ..
            } => {
                if *total_network_stake == 0 {
                    return false;
                }
                (*total_stake_acked as f64 / *total_network_stake as f64) >= 0.5
            }
            Self::WaitingForKey { .. } | Self::Verified(_) => true,
            Self::Failed { .. } => false,
        }
    }

    /// Get quorum percentage
    pub fn quorum_percentage(&self) -> f64 {
        match self {
            Self::WaitingForAcks {
                total_stake_acked,
                total_network_stake,
                ..
            } => {
                if *total_network_stake == 0 {
                    return 0.0;
                }
                (*total_stake_acked as f64 / *total_network_stake as f64) * 100.0
            }
            Self::WaitingForKey { .. } | Self::Verified(_) => 100.0,
            Self::Failed { .. } => 0.0,
        }
    }
}

/// Record of content for duplicate detection
#[derive(Clone, Debug)]
pub struct ContentRecord {
    pub submission_hash: [u8; 32],
    pub miner_hotkey: String,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
    pub epoch: u64,
}

/// Manager for pending submissions in term-challenge
pub struct TermSubmissionManager {
    /// Pending submissions by hash
    pending: HashMap<[u8; 32], SubmissionState>,
    /// Verified submissions by hash
    verified: HashMap<[u8; 32], VerifiedSubmission>,
    /// Content hash -> record for duplicate detection
    content_index: HashMap<[u8; 32], ContentRecord>,
    /// Banned miner hotkeys
    banned_hotkeys: HashSet<String>,
    /// Banned miner coldkeys
    banned_coldkeys: HashSet<String>,
    /// Submission timeout in seconds
    timeout_secs: u64,
}

impl TermSubmissionManager {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            pending: HashMap::new(),
            verified: HashMap::new(),
            content_index: HashMap::new(),
            banned_hotkeys: HashSet::new(),
            banned_coldkeys: HashSet::new(),
            timeout_secs,
        }
    }

    /// Check if a miner is banned
    pub fn is_banned(&self, hotkey: &str, coldkey: &str) -> bool {
        self.banned_hotkeys.contains(hotkey) || self.banned_coldkeys.contains(coldkey)
    }

    /// Ban a miner by hotkey
    pub fn ban_hotkey(&mut self, hotkey: &str) {
        self.banned_hotkeys.insert(hotkey.to_string());
    }

    /// Ban a miner by coldkey
    pub fn ban_coldkey(&mut self, coldkey: &str) {
        self.banned_coldkeys.insert(coldkey.to_string());
    }

    /// Add a new encrypted submission
    pub fn add_submission(
        &mut self,
        submission: EncryptedSubmission,
        total_network_stake: u64,
    ) -> Result<(), SubmissionError> {
        // Check if banned
        if self.is_banned(&submission.miner_hotkey, &submission.miner_coldkey) {
            return Err(SubmissionError::MinerBanned);
        }

        // Verify hash
        if !submission.verify_hash() {
            return Err(SubmissionError::InvalidHash);
        }

        // Check if already exists
        if self.pending.contains_key(&submission.submission_hash)
            || self.verified.contains_key(&submission.submission_hash)
        {
            return Err(SubmissionError::AlreadyExists);
        }

        self.pending.insert(
            submission.submission_hash,
            SubmissionState::WaitingForAcks {
                submission,
                acks: HashMap::new(),
                total_stake_acked: 0,
                total_network_stake,
            },
        );

        Ok(())
    }

    /// Add an acknowledgment for a submission
    pub fn add_ack(&mut self, ack: SubmissionAck) -> Result<bool, SubmissionError> {
        let state = self
            .pending
            .get_mut(&ack.submission_hash)
            .ok_or(SubmissionError::NotFound)?;

        match state {
            SubmissionState::WaitingForAcks {
                acks,
                total_stake_acked,
                total_network_stake,
                submission,
            } => {
                // Don't count duplicate acks
                if acks.contains_key(&ack.validator_hotkey) {
                    return Ok(false);
                }

                *total_stake_acked += ack.validator_stake;
                acks.insert(ack.validator_hotkey.clone(), ack);

                // Check if quorum reached
                let percentage = *total_stake_acked as f64 / *total_network_stake as f64;
                if percentage >= 0.5 {
                    // Transition to WaitingForKey
                    let submission = submission.clone();
                    let acks = acks.clone();
                    *state = SubmissionState::WaitingForKey { submission, acks };
                    return Ok(true); // Quorum reached
                }

                Ok(false)
            }
            _ => Err(SubmissionError::InvalidState),
        }
    }

    /// Reveal decryption key and verify submission
    pub fn reveal_key(
        &mut self,
        reveal: DecryptionKeyReveal,
    ) -> Result<VerifiedSubmission, SubmissionError> {
        let state = self
            .pending
            .remove(&reveal.submission_hash)
            .ok_or(SubmissionError::NotFound)?;

        match state {
            SubmissionState::WaitingForKey { submission, .. } => {
                // Verify key hash matches
                if !reveal.verify_key_hash(&submission.key_hash) {
                    self.pending.insert(
                        reveal.submission_hash,
                        SubmissionState::Failed {
                            reason: "Key hash mismatch".to_string(),
                        },
                    );
                    return Err(SubmissionError::InvalidKey);
                }

                // Decrypt data
                let decrypted = decrypt_data(
                    &submission.encrypted_data,
                    &reveal.decryption_key,
                    &submission.nonce,
                )?;

                // CRITICAL: Verify ownership - content hash must match what miner signed
                let actual_content_hash = EncryptedSubmission::compute_content_hash(&decrypted);
                let ownership_verified = actual_content_hash == submission.content_hash;

                if !ownership_verified {
                    self.pending.insert(
                        reveal.submission_hash,
                        SubmissionState::Failed {
                            reason: "Content hash mismatch - ownership verification failed"
                                .to_string(),
                        },
                    );
                    return Err(SubmissionError::OwnershipVerificationFailed);
                }

                // Check for duplicate content (same code already submitted)
                if let Some(existing) = self.content_index.get(&actual_content_hash) {
                    if existing.submitted_at < submission.submitted_at {
                        self.pending.insert(
                            reveal.submission_hash,
                            SubmissionState::Failed {
                                reason: format!(
                                    "Duplicate content - same code already submitted by {} at {}",
                                    existing.miner_hotkey, existing.submitted_at
                                ),
                            },
                        );
                        return Err(SubmissionError::DuplicateContent);
                    }
                }

                let verified = VerifiedSubmission {
                    submission_hash: submission.submission_hash,
                    content_hash: actual_content_hash,
                    challenge_id: submission.challenge_id.clone(),
                    miner_hotkey: submission.miner_hotkey.clone(),
                    miner_coldkey: submission.miner_coldkey,
                    data: decrypted,
                    epoch: submission.epoch,
                    submitted_at: submission.submitted_at,
                    verified_at: chrono::Utc::now(),
                    ownership_verified,
                };

                // Index content for duplicate detection
                self.content_index.insert(
                    actual_content_hash,
                    ContentRecord {
                        submission_hash: submission.submission_hash,
                        miner_hotkey: submission.miner_hotkey,
                        submitted_at: submission.submitted_at,
                        epoch: submission.epoch,
                    },
                );

                self.verified
                    .insert(submission.submission_hash, verified.clone());
                Ok(verified)
            }
            SubmissionState::WaitingForAcks { .. } => Err(SubmissionError::QuorumNotReached),
            _ => Err(SubmissionError::InvalidState),
        }
    }

    /// Get a verified submission
    pub fn get_verified(&self, hash: &[u8; 32]) -> Option<&VerifiedSubmission> {
        self.verified.get(hash)
    }

    /// Get all verified submissions for an epoch
    pub fn get_verified_for_epoch(&self, epoch: u64) -> Vec<&VerifiedSubmission> {
        self.verified
            .values()
            .filter(|s| s.epoch == epoch)
            .collect()
    }

    /// Get pending submission state
    pub fn get_pending(&self, hash: &[u8; 32]) -> Option<&SubmissionState> {
        self.pending.get(hash)
    }

    /// Check if content already exists (for duplicate detection)
    pub fn get_content_record(&self, content_hash: &[u8; 32]) -> Option<&ContentRecord> {
        self.content_index.get(content_hash)
    }

    /// Cleanup expired submissions
    pub fn cleanup_expired(&mut self) {
        let now = chrono::Utc::now();
        let timeout = chrono::Duration::seconds(self.timeout_secs as i64);

        self.pending.retain(|_, state| match state {
            SubmissionState::WaitingForAcks { submission, .. }
            | SubmissionState::WaitingForKey { submission, .. } => {
                now - submission.submitted_at < timeout
            }
            _ => false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_challenge_sdk::{encrypt_data, generate_key, generate_nonce, hash_key};

    fn create_test_submission(epoch: u64) -> (EncryptedSubmission, [u8; 32], [u8; 24]) {
        let key = generate_key();
        let nonce = generate_nonce();
        let key_hash = hash_key(&key);
        let data = b"test agent code";
        let content_hash = EncryptedSubmission::compute_content_hash(data);
        let encrypted = encrypt_data(data, &key, &nonce).unwrap();

        let submission = EncryptedSubmission::new(
            "term-bench".to_string(),
            "miner-hotkey".to_string(),
            "miner-coldkey".to_string(),
            encrypted,
            key_hash,
            nonce,
            content_hash,
            vec![],
            epoch,
        );

        (submission, key, nonce)
    }

    #[test]
    fn test_submission_flow() {
        let mut manager = TermSubmissionManager::new(300);
        let (submission, key, _nonce) = create_test_submission(1);
        let submission_hash = submission.submission_hash;

        // Add submission
        manager.add_submission(submission, 1000).unwrap();

        // Add ACKs
        let ack1 = SubmissionAck::new(submission_hash, Hotkey([1u8; 32]), 300, vec![]);
        let ack2 = SubmissionAck::new(submission_hash, Hotkey([2u8; 32]), 300, vec![]);

        assert!(!manager.add_ack(ack1).unwrap()); // Not yet quorum
        assert!(manager.add_ack(ack2).unwrap()); // Quorum reached

        // Reveal key
        let reveal = DecryptionKeyReveal::new(submission_hash, key.to_vec(), vec![]);
        let verified = manager.reveal_key(reveal).unwrap();

        assert!(verified.ownership_verified);
        assert_eq!(verified.miner_hotkey, "miner-hotkey");
    }

    #[test]
    fn test_banned_miner() {
        let mut manager = TermSubmissionManager::new(300);
        manager.ban_hotkey("bad-miner");

        let key = generate_key();
        let nonce = generate_nonce();
        let key_hash = hash_key(&key);
        let data = b"bad code";
        let content_hash = EncryptedSubmission::compute_content_hash(data);
        let encrypted = encrypt_data(data, &key, &nonce).unwrap();

        let submission = EncryptedSubmission::new(
            "term-bench".to_string(),
            "bad-miner".to_string(),
            "coldkey".to_string(),
            encrypted,
            key_hash,
            nonce,
            content_hash,
            vec![],
            1,
        );

        let result = manager.add_submission(submission, 1000);
        assert!(matches!(result, Err(SubmissionError::MinerBanned)));
    }
}
