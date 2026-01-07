//! Assignment Monitor Worker
//!
//! Background service that monitors validator assignments and reassigns
//! agents when validators don't start evaluation within timeout period.
//!
//! Flow:
//! 1. Poll DB every 5 minutes for stale assignments (no task_logs after 30 min)
//! 2. For each stale assignment with < 3 reassignments:
//!    a. Find available validator (not already assigned to this agent)
//!    b. Delete old assignment, create new one
//!    c. Increment reassignment_count
//!    d. Log the reassignment (new validator will pick up via manual poll)

use crate::pg_storage::PgStorage;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration for the assignment monitor
pub struct AssignmentMonitorConfig {
    /// How often to check for stale assignments (default: 5 minutes)
    pub poll_interval_secs: u64,
    /// Timeout before reassignment (default: 30 minutes)
    pub stale_timeout_minutes: i64,
    /// Maximum number of reassignments per agent (default: 3)
    pub max_reassignments: i32,
}

impl Default for AssignmentMonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 300,   // 5 minutes
            stale_timeout_minutes: 30, // 30 minutes
            max_reassignments: 3,
        }
    }
}

/// Validator info from platform-server
#[derive(Debug, Deserialize)]
struct ValidatorInfo {
    hotkey: String,
    is_active: bool,
}

/// Background worker that monitors validator assignments
pub struct AssignmentMonitor {
    storage: Arc<PgStorage>,
    platform_url: String,
    config: AssignmentMonitorConfig,
}

impl AssignmentMonitor {
    pub fn new(
        storage: Arc<PgStorage>,
        platform_url: String,
        config: AssignmentMonitorConfig,
    ) -> Self {
        Self {
            storage,
            platform_url,
            config,
        }
    }

    /// Start the monitor (runs forever)
    pub async fn run(&self) {
        info!(
            "Assignment monitor started (poll={}s, timeout={}min, max_reassign={})",
            self.config.poll_interval_secs,
            self.config.stale_timeout_minutes,
            self.config.max_reassignments
        );

        let mut ticker = interval(Duration::from_secs(self.config.poll_interval_secs));

        loop {
            ticker.tick().await;

            if let Err(e) = self.check_and_reassign_stale().await {
                error!("Error checking stale assignments: {}", e);
            }
        }
    }

    /// Check for stale assignments and reassign to new validators
    async fn check_and_reassign_stale(&self) -> anyhow::Result<()> {
        // Get stale assignments from database
        let stale = self
            .storage
            .get_stale_assignments(
                self.config.stale_timeout_minutes,
                self.config.max_reassignments,
            )
            .await?;

        if stale.is_empty() {
            debug!("No stale validator assignments found");
            return Ok(());
        }

        info!("Found {} stale validator assignments", stale.len());

        // Fetch all active validators once (for efficiency)
        let all_validators = self.fetch_active_validators().await?;
        if all_validators.is_empty() {
            warn!("No active validators available from platform-server");
            return Ok(());
        }

        for assignment in stale {
            let short_hash = &assignment.agent_hash[..16.min(assignment.agent_hash.len())];
            let short_validator =
                &assignment.validator_hotkey[..16.min(assignment.validator_hotkey.len())];

            // Skip if max reassignments reached (shouldn't happen due to query filter, but safety check)
            if assignment.reassignment_count >= self.config.max_reassignments {
                warn!(
                    "Agent {} reached max reassignments ({}), skipping",
                    short_hash, assignment.reassignment_count
                );
                continue;
            }

            // Get validators already assigned or previously tried
            let excluded_validators = self
                .storage
                .get_validators_assigned_to_agent(&assignment.agent_hash)
                .await
                .unwrap_or_default();

            // Filter available validators (active and not excluded)
            let available: Vec<&String> = all_validators
                .iter()
                .filter(|v| !excluded_validators.contains(v))
                .collect();

            if available.is_empty() {
                warn!(
                    "No available validators for agent {} (all {} active validators already tried or assigned)",
                    short_hash,
                    all_validators.len()
                );
                continue;
            }

            // Select a random validator from available ones
            use rand::SeedableRng;
            let mut rng = rand::rngs::StdRng::from_entropy();
            let new_validator = match available.choose(&mut rng) {
                Some(v) => (*v).clone(),
                None => continue,
            };

            let short_new = &new_validator[..16.min(new_validator.len())];

            // Perform the reassignment
            match self
                .storage
                .reassign_validator(
                    &assignment.agent_hash,
                    &assignment.validator_hotkey,
                    &new_validator,
                    "timeout",
                )
                .await
            {
                Ok(_) => {
                    info!(
                        "Reassigned agent {} from stale validator {} to {} (reassignment #{}/{})",
                        short_hash,
                        short_validator,
                        short_new,
                        assignment.reassignment_count + 1,
                        self.config.max_reassignments
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to reassign agent {} from {} to {}: {}",
                        short_hash, short_validator, short_new, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Fetch active validators from platform-server
    async fn fetch_active_validators(&self) -> anyhow::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let url = format!("{}/api/v1/validators", self.platform_url);

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch validators: HTTP {}", response.status());
        }

        let validators: Vec<ValidatorInfo> = response.json().await?;

        let active: Vec<String> = validators
            .into_iter()
            .filter(|v| v.is_active)
            .map(|v| v.hotkey)
            .collect();

        debug!(
            "Fetched {} active validators from platform-server",
            active.len()
        );

        Ok(active)
    }
}

/// Start the assignment monitor in background
pub fn spawn_assignment_monitor(
    storage: Arc<PgStorage>,
    platform_url: String,
    config: AssignmentMonitorConfig,
) {
    tokio::spawn(async move {
        let monitor = AssignmentMonitor::new(storage, platform_url, config);
        monitor.run().await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = AssignmentMonitorConfig::default();
        assert_eq!(config.poll_interval_secs, 300);
        assert_eq!(config.stale_timeout_minutes, 30);
        assert_eq!(config.max_reassignments, 3);
    }
}
