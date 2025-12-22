//! Code Visibility System for Term-Challenge
//!
//! Controls when miner code becomes visible to the public:
//! - Code is hidden by default
//! - Becomes visible after 3+ validators complete all tasks for 3+ epochs
//! - Sudo can see any code at any time
//!
//! Flow:
//! 1. Agent submitted -> Code hidden (only top 3 validators + root see it)
//! 2. Validators evaluate agent -> Track completion per validator
//! 3. After 3+ validators complete AND 3+ epochs pass -> Code becomes public
//! 4. Sudo users can always view code regardless of visibility status

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Minimum validators required for code visibility
pub const MIN_VALIDATORS_FOR_VISIBILITY: usize = 3;

/// Minimum epochs after validation for code visibility
pub const MIN_EPOCHS_FOR_VISIBILITY: u64 = 3;

#[derive(Debug, Error)]
pub enum VisibilityError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Code not yet visible: {reason}")]
    NotYetVisible { reason: String },
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Visibility status for an agent's code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisibilityStatus {
    /// Code is hidden - not enough validations or epochs
    Hidden,
    /// Code is pending - enough validations but epochs not met
    PendingEpochs,
    /// Code is visible to public
    Public,
    /// Code was manually revealed by sudo
    ManuallyRevealed,
}

/// Validator completion record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorCompletion {
    /// Validator hotkey
    pub validator_hotkey: String,
    /// Epoch when evaluation was completed
    pub completed_epoch: u64,
    /// Number of tasks completed
    pub tasks_completed: usize,
    /// Total tasks in evaluation
    pub total_tasks: usize,
    /// Final score achieved
    pub score: f64,
    /// Timestamp of completion
    pub completed_at: u64,
    /// Hash of evaluation results for verification
    pub results_hash: String,
}

/// Agent visibility tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVisibility {
    /// Agent hash
    pub agent_hash: String,
    /// Miner hotkey who submitted
    pub miner_hotkey: String,
    /// Current visibility status
    pub status: VisibilityStatus,
    /// Epoch when agent was submitted
    pub submitted_epoch: u64,
    /// Validators who have completed evaluation
    pub completions: Vec<ValidatorCompletion>,
    /// First epoch when MIN_VALIDATORS completed
    pub visibility_eligible_epoch: Option<u64>,
    /// Epoch when code became visible
    pub visible_since_epoch: Option<u64>,
    /// Who manually revealed (if applicable)
    pub manually_revealed_by: Option<String>,
    /// Timestamp when visibility changed
    pub status_updated_at: u64,
    /// Encrypted/obfuscated code (for hidden state)
    pub code_hash: String,
    /// Actual source code (stored encrypted, revealed when visible)
    source_code: Option<String>,
}

impl AgentVisibility {
    pub fn new(
        agent_hash: String,
        miner_hotkey: String,
        code_hash: String,
        source_code: String,
        submitted_epoch: u64,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            agent_hash,
            miner_hotkey,
            status: VisibilityStatus::Hidden,
            submitted_epoch,
            completions: Vec::new(),
            visibility_eligible_epoch: None,
            visible_since_epoch: None,
            manually_revealed_by: None,
            status_updated_at: now,
            code_hash,
            source_code: Some(source_code),
        }
    }

    /// Get number of unique validators who completed evaluation
    pub fn validator_count(&self) -> usize {
        self.completions
            .iter()
            .map(|c| &c.validator_hotkey)
            .collect::<HashSet<_>>()
            .len()
    }

    /// Check if visibility requirements are met
    pub fn check_visibility(&self, current_epoch: u64) -> VisibilityStatus {
        // Already manually revealed
        if self.status == VisibilityStatus::ManuallyRevealed {
            return VisibilityStatus::ManuallyRevealed;
        }

        // Already public
        if self.status == VisibilityStatus::Public {
            return VisibilityStatus::Public;
        }

        let validator_count = self.validator_count();

        // Not enough validators
        if validator_count < MIN_VALIDATORS_FOR_VISIBILITY {
            return VisibilityStatus::Hidden;
        }

        // Check if we have eligibility epoch
        let eligible_epoch = match self.visibility_eligible_epoch {
            Some(epoch) => epoch,
            None => return VisibilityStatus::Hidden, // Should not happen if validator_count >= MIN
        };

        // Check epochs passed since eligibility
        let epochs_since_eligible = current_epoch.saturating_sub(eligible_epoch);
        if epochs_since_eligible >= MIN_EPOCHS_FOR_VISIBILITY {
            VisibilityStatus::Public
        } else {
            VisibilityStatus::PendingEpochs
        }
    }

    /// Get epochs remaining until visibility
    pub fn epochs_until_visible(&self, current_epoch: u64) -> Option<u64> {
        if self.status == VisibilityStatus::Public
            || self.status == VisibilityStatus::ManuallyRevealed
        {
            return Some(0);
        }

        if self.validator_count() < MIN_VALIDATORS_FOR_VISIBILITY {
            return None; // Need more validators first
        }

        let eligible_epoch = self.visibility_eligible_epoch?;
        let target_epoch = eligible_epoch + MIN_EPOCHS_FOR_VISIBILITY;

        if current_epoch >= target_epoch {
            Some(0)
        } else {
            Some(target_epoch - current_epoch)
        }
    }

    /// Get validators still needed for visibility
    pub fn validators_needed(&self) -> usize {
        MIN_VALIDATORS_FOR_VISIBILITY.saturating_sub(self.validator_count())
    }
}

/// Code visibility request result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeViewResult {
    /// Agent hash
    pub agent_hash: String,
    /// Miner hotkey
    pub miner_hotkey: String,
    /// Visibility status
    pub status: VisibilityStatus,
    /// Source code (only if visible or sudo)
    pub source_code: Option<String>,
    /// Code hash (always available)
    pub code_hash: String,
    /// Number of validators who completed
    pub validator_completions: usize,
    /// Epochs until visible (if pending)
    pub epochs_until_visible: Option<u64>,
    /// Validators needed (if not enough)
    pub validators_needed: usize,
    /// List of validators who completed
    pub completed_by: Vec<String>,
    /// Visibility requirements summary
    pub requirements: VisibilityRequirements,
}

/// Visibility requirements for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityRequirements {
    pub min_validators: usize,
    pub min_epochs: u64,
    pub current_validators: usize,
    pub epochs_since_eligible: Option<u64>,
    pub met: bool,
}

/// Code Visibility Manager
pub struct CodeVisibilityManager {
    /// Agent visibility tracking
    agents: Arc<RwLock<HashMap<String, AgentVisibility>>>,
    /// Sudo hotkeys who can view any code
    sudo_hotkeys: Arc<RwLock<HashSet<String>>>,
    /// Root validator hotkey (always has access)
    root_validator: String,
    /// Current epoch
    current_epoch: Arc<RwLock<u64>>,
    /// Configuration
    config: VisibilityConfig,
}

/// Visibility configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityConfig {
    /// Minimum validators for visibility
    pub min_validators: usize,
    /// Minimum epochs after validation
    pub min_epochs: u64,
    /// Allow miner to see their own code always
    pub allow_self_view: bool,
    /// Store code encrypted
    pub encrypt_stored_code: bool,
}

impl Default for VisibilityConfig {
    fn default() -> Self {
        Self {
            min_validators: MIN_VALIDATORS_FOR_VISIBILITY,
            min_epochs: MIN_EPOCHS_FOR_VISIBILITY,
            allow_self_view: true,
            encrypt_stored_code: true,
        }
    }
}

impl CodeVisibilityManager {
    pub fn new(root_validator: String, config: VisibilityConfig) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            sudo_hotkeys: Arc::new(RwLock::new(HashSet::new())),
            root_validator,
            current_epoch: Arc::new(RwLock::new(0)),
            config,
        }
    }

    /// Set current epoch
    pub fn set_epoch(&self, epoch: u64) {
        *self.current_epoch.write() = epoch;

        // Update visibility status for all agents
        self.update_all_visibility_status();
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        *self.current_epoch.read()
    }

    /// Add sudo hotkey
    pub fn add_sudo(&self, hotkey: &str) {
        self.sudo_hotkeys.write().insert(hotkey.to_string());
        info!("Added sudo hotkey for code visibility: {}", hotkey);
    }

    /// Remove sudo hotkey
    pub fn remove_sudo(&self, hotkey: &str) {
        self.sudo_hotkeys.write().remove(hotkey);
        info!("Removed sudo hotkey: {}", hotkey);
    }

    /// Check if hotkey is sudo
    pub fn is_sudo(&self, hotkey: &str) -> bool {
        hotkey == self.root_validator || self.sudo_hotkeys.read().contains(hotkey)
    }

    /// Register a new agent submission
    pub fn register_agent(
        &self,
        agent_hash: &str,
        miner_hotkey: &str,
        source_code: &str,
    ) -> AgentVisibility {
        let code_hash = hex::encode(Sha256::digest(source_code.as_bytes()));
        let current_epoch = *self.current_epoch.read();

        let visibility = AgentVisibility::new(
            agent_hash.to_string(),
            miner_hotkey.to_string(),
            code_hash,
            source_code.to_string(),
            current_epoch,
        );

        self.agents
            .write()
            .insert(agent_hash.to_string(), visibility.clone());

        info!(
            "Registered agent {} from {} for visibility tracking (epoch {})",
            agent_hash, miner_hotkey, current_epoch
        );

        visibility
    }

    /// Record validator completion of agent evaluation
    pub fn record_completion(
        &self,
        agent_hash: &str,
        validator_hotkey: &str,
        tasks_completed: usize,
        total_tasks: usize,
        score: f64,
        results_hash: &str,
    ) -> Result<AgentVisibility, VisibilityError> {
        let current_epoch = *self.current_epoch.read();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut agents = self.agents.write();
        let visibility = agents
            .get_mut(agent_hash)
            .ok_or_else(|| VisibilityError::AgentNotFound(agent_hash.to_string()))?;

        // Check if this validator already completed (update if so)
        if let Some(existing) = visibility
            .completions
            .iter_mut()
            .find(|c| c.validator_hotkey == validator_hotkey)
        {
            // Update existing completion
            existing.completed_epoch = current_epoch;
            existing.tasks_completed = tasks_completed;
            existing.total_tasks = total_tasks;
            existing.score = score;
            existing.completed_at = now;
            existing.results_hash = results_hash.to_string();

            debug!(
                "Updated completion for agent {} by validator {} (epoch {})",
                agent_hash, validator_hotkey, current_epoch
            );
        } else {
            // Add new completion
            visibility.completions.push(ValidatorCompletion {
                validator_hotkey: validator_hotkey.to_string(),
                completed_epoch: current_epoch,
                tasks_completed,
                total_tasks,
                score,
                completed_at: now,
                results_hash: results_hash.to_string(),
            });

            info!(
                "Recorded completion for agent {} by validator {} ({}/{} validators, epoch {})",
                agent_hash,
                validator_hotkey,
                visibility.validator_count(),
                self.config.min_validators,
                current_epoch
            );
        }

        // Check if we just reached minimum validators
        if visibility.visibility_eligible_epoch.is_none()
            && visibility.validator_count() >= self.config.min_validators
        {
            visibility.visibility_eligible_epoch = Some(current_epoch);
            info!(
                "Agent {} reached {} validators at epoch {} - visibility eligible in {} epochs",
                agent_hash, self.config.min_validators, current_epoch, self.config.min_epochs
            );
        }

        // Update visibility status
        let new_status = visibility.check_visibility(current_epoch);
        if new_status != visibility.status {
            visibility.status = new_status;
            visibility.status_updated_at = now;

            if new_status == VisibilityStatus::Public {
                visibility.visible_since_epoch = Some(current_epoch);
                info!(
                    "Agent {} code is now PUBLIC (epoch {})",
                    agent_hash, current_epoch
                );
            }
        }

        Ok(visibility.clone())
    }

    /// Manually reveal code (sudo only)
    pub fn sudo_reveal(
        &self,
        agent_hash: &str,
        sudo_hotkey: &str,
    ) -> Result<AgentVisibility, VisibilityError> {
        // Verify sudo permission
        if !self.is_sudo(sudo_hotkey) {
            return Err(VisibilityError::Unauthorized(format!(
                "{} is not a sudo user",
                sudo_hotkey
            )));
        }

        let current_epoch = *self.current_epoch.read();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut agents = self.agents.write();
        let visibility = agents
            .get_mut(agent_hash)
            .ok_or_else(|| VisibilityError::AgentNotFound(agent_hash.to_string()))?;

        visibility.status = VisibilityStatus::ManuallyRevealed;
        visibility.manually_revealed_by = Some(sudo_hotkey.to_string());
        visibility.visible_since_epoch = Some(current_epoch);
        visibility.status_updated_at = now;

        info!(
            "Agent {} code manually revealed by sudo {} (epoch {})",
            agent_hash, sudo_hotkey, current_epoch
        );

        Ok(visibility.clone())
    }

    /// Get code for an agent
    ///
    /// Returns code if:
    /// - Requester is sudo (can always view)
    /// - Requester is the miner who submitted (if allow_self_view)
    /// - Code visibility is Public or ManuallyRevealed
    pub fn get_code(
        &self,
        agent_hash: &str,
        requester_hotkey: &str,
    ) -> Result<CodeViewResult, VisibilityError> {
        let current_epoch = *self.current_epoch.read();
        let agents = self.agents.read();

        let visibility = agents
            .get(agent_hash)
            .ok_or_else(|| VisibilityError::AgentNotFound(agent_hash.to_string()))?;

        let is_sudo = self.is_sudo(requester_hotkey);
        let is_owner = visibility.miner_hotkey == requester_hotkey;
        let is_visible = matches!(
            visibility.status,
            VisibilityStatus::Public | VisibilityStatus::ManuallyRevealed
        );

        // Determine if code should be returned
        let can_view = is_sudo || (self.config.allow_self_view && is_owner) || is_visible;

        let epochs_since_eligible = visibility
            .visibility_eligible_epoch
            .map(|e| current_epoch.saturating_sub(e));

        let source_code = if can_view {
            visibility.source_code.clone()
        } else {
            None
        };

        Ok(CodeViewResult {
            agent_hash: visibility.agent_hash.clone(),
            miner_hotkey: visibility.miner_hotkey.clone(),
            status: visibility.status,
            source_code,
            code_hash: visibility.code_hash.clone(),
            validator_completions: visibility.validator_count(),
            epochs_until_visible: visibility.epochs_until_visible(current_epoch),
            validators_needed: visibility.validators_needed(),
            completed_by: visibility
                .completions
                .iter()
                .map(|c| c.validator_hotkey.clone())
                .collect(),
            requirements: VisibilityRequirements {
                min_validators: self.config.min_validators,
                min_epochs: self.config.min_epochs,
                current_validators: visibility.validator_count(),
                epochs_since_eligible,
                met: is_visible,
            },
        })
    }

    /// Get visibility status for an agent
    pub fn get_status(&self, agent_hash: &str) -> Option<AgentVisibility> {
        self.agents.read().get(agent_hash).cloned()
    }

    /// Get all agents with public visibility
    pub fn get_public_agents(&self) -> Vec<AgentVisibility> {
        self.agents
            .read()
            .values()
            .filter(|v| {
                matches!(
                    v.status,
                    VisibilityStatus::Public | VisibilityStatus::ManuallyRevealed
                )
            })
            .cloned()
            .collect()
    }

    /// Get agents pending visibility (have enough validators but waiting for epochs)
    pub fn get_pending_agents(&self) -> Vec<AgentVisibility> {
        self.agents
            .read()
            .values()
            .filter(|v| v.status == VisibilityStatus::PendingEpochs)
            .cloned()
            .collect()
    }

    /// Get all hidden agents
    pub fn get_hidden_agents(&self) -> Vec<AgentVisibility> {
        self.agents
            .read()
            .values()
            .filter(|v| v.status == VisibilityStatus::Hidden)
            .cloned()
            .collect()
    }

    /// Update visibility status for all agents based on current epoch
    fn update_all_visibility_status(&self) {
        let current_epoch = *self.current_epoch.read();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut agents = self.agents.write();

        for (agent_hash, visibility) in agents.iter_mut() {
            let new_status = visibility.check_visibility(current_epoch);

            if new_status != visibility.status
                && visibility.status != VisibilityStatus::ManuallyRevealed
            {
                let old_status = visibility.status;
                visibility.status = new_status;
                visibility.status_updated_at = now;

                if new_status == VisibilityStatus::Public {
                    visibility.visible_since_epoch = Some(current_epoch);
                    info!(
                        "Agent {} visibility changed {:?} -> {:?} (epoch {})",
                        agent_hash, old_status, new_status, current_epoch
                    );
                }
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> VisibilityStats {
        let agents = self.agents.read();

        let mut hidden = 0;
        let mut pending = 0;
        let mut public = 0;
        let mut revealed = 0;

        for v in agents.values() {
            match v.status {
                VisibilityStatus::Hidden => hidden += 1,
                VisibilityStatus::PendingEpochs => pending += 1,
                VisibilityStatus::Public => public += 1,
                VisibilityStatus::ManuallyRevealed => revealed += 1,
            }
        }

        VisibilityStats {
            total_agents: agents.len(),
            hidden_agents: hidden,
            pending_agents: pending,
            public_agents: public,
            manually_revealed: revealed,
            sudo_count: self.sudo_hotkeys.read().len(),
            current_epoch: *self.current_epoch.read(),
            config: self.config.clone(),
        }
    }
}

/// Visibility statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityStats {
    pub total_agents: usize,
    pub hidden_agents: usize,
    pub pending_agents: usize,
    pub public_agents: usize,
    pub manually_revealed: usize,
    pub sudo_count: usize,
    pub current_epoch: u64,
    pub config: VisibilityConfig,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_manager() -> CodeVisibilityManager {
        CodeVisibilityManager::new("root_validator".to_string(), VisibilityConfig::default())
    }

    #[test]
    fn test_register_agent() {
        let manager = create_manager();
        manager.set_epoch(10);

        let visibility = manager.register_agent("agent1", "miner1", "print('hello')");

        assert_eq!(visibility.agent_hash, "agent1");
        assert_eq!(visibility.miner_hotkey, "miner1");
        assert_eq!(visibility.status, VisibilityStatus::Hidden);
        assert_eq!(visibility.submitted_epoch, 10);
        assert!(visibility.completions.is_empty());
    }

    #[test]
    fn test_visibility_progression() {
        let manager = create_manager();
        manager.set_epoch(10);

        // Register agent
        manager.register_agent("agent1", "miner1", "print('hello')");

        // Add 2 validator completions - not enough
        manager
            .record_completion("agent1", "validator1", 10, 10, 0.9, "hash1")
            .unwrap();
        manager
            .record_completion("agent1", "validator2", 10, 10, 0.85, "hash2")
            .unwrap();

        let status = manager.get_status("agent1").unwrap();
        assert_eq!(status.status, VisibilityStatus::Hidden);
        assert_eq!(status.validator_count(), 2);

        // Add 3rd validator - now eligible but need to wait epochs
        manager
            .record_completion("agent1", "validator3", 10, 10, 0.88, "hash3")
            .unwrap();

        let status = manager.get_status("agent1").unwrap();
        assert_eq!(status.status, VisibilityStatus::PendingEpochs);
        assert_eq!(status.visibility_eligible_epoch, Some(10));

        // Advance 2 epochs - still pending
        manager.set_epoch(12);
        let status = manager.get_status("agent1").unwrap();
        assert_eq!(status.check_visibility(12), VisibilityStatus::PendingEpochs);

        // Advance to epoch 13 (3 epochs since eligibility) - now public
        manager.set_epoch(13);
        let status = manager.get_status("agent1").unwrap();
        assert_eq!(status.check_visibility(13), VisibilityStatus::Public);
    }

    #[test]
    fn test_sudo_can_always_view() {
        let manager = create_manager();
        manager.set_epoch(10);

        // Register agent
        manager.register_agent("agent1", "miner1", "print('secret')");

        // Root validator can view
        let result = manager.get_code("agent1", "root_validator").unwrap();
        assert!(result.source_code.is_some());
        assert_eq!(result.source_code.unwrap(), "print('secret')");

        // Add sudo user
        manager.add_sudo("sudo_user");

        // Sudo can view
        let result = manager.get_code("agent1", "sudo_user").unwrap();
        assert!(result.source_code.is_some());

        // Random user cannot view
        let result = manager.get_code("agent1", "random_user").unwrap();
        assert!(result.source_code.is_none());
        assert_eq!(result.status, VisibilityStatus::Hidden);
    }

    #[test]
    fn test_owner_can_view_own_code() {
        let manager = create_manager();
        manager.set_epoch(10);

        // Register agent
        manager.register_agent("agent1", "miner1", "print('my code')");

        // Owner can view their own code
        let result = manager.get_code("agent1", "miner1").unwrap();
        assert!(result.source_code.is_some());
        assert_eq!(result.source_code.unwrap(), "print('my code')");

        // Other miner cannot view
        let result = manager.get_code("agent1", "miner2").unwrap();
        assert!(result.source_code.is_none());
    }

    #[test]
    fn test_sudo_reveal() {
        let manager = create_manager();
        manager.set_epoch(10);
        manager.add_sudo("sudo_admin");

        // Register agent
        manager.register_agent("agent1", "miner1", "print('reveal me')");

        // Verify it's hidden
        let result = manager.get_code("agent1", "random_user").unwrap();
        assert!(result.source_code.is_none());

        // Sudo reveals
        manager.sudo_reveal("agent1", "sudo_admin").unwrap();

        // Now anyone can view
        let result = manager.get_code("agent1", "random_user").unwrap();
        assert!(result.source_code.is_some());
        assert_eq!(result.status, VisibilityStatus::ManuallyRevealed);
    }

    #[test]
    fn test_non_sudo_cannot_reveal() {
        let manager = create_manager();
        manager.set_epoch(10);

        manager.register_agent("agent1", "miner1", "print('secret')");

        // Non-sudo cannot reveal
        let result = manager.sudo_reveal("agent1", "random_user");
        assert!(result.is_err());
    }

    #[test]
    fn test_visibility_requirements() {
        let manager = create_manager();
        manager.set_epoch(10);

        manager.register_agent("agent1", "miner1", "code");

        let result = manager.get_code("agent1", "random").unwrap();
        assert_eq!(result.validators_needed, 3);
        assert!(result.epochs_until_visible.is_none()); // Need validators first

        // Add validators
        manager
            .record_completion("agent1", "v1", 10, 10, 0.9, "h1")
            .unwrap();
        manager
            .record_completion("agent1", "v2", 10, 10, 0.9, "h2")
            .unwrap();
        manager
            .record_completion("agent1", "v3", 10, 10, 0.9, "h3")
            .unwrap();

        let result = manager.get_code("agent1", "random").unwrap();
        assert_eq!(result.validators_needed, 0);
        assert_eq!(result.epochs_until_visible, Some(3)); // Need 3 more epochs

        // Advance epochs
        manager.set_epoch(13);
        let result = manager.get_code("agent1", "random").unwrap();
        assert_eq!(result.epochs_until_visible, Some(0));
        assert!(result.requirements.met);
    }
}
