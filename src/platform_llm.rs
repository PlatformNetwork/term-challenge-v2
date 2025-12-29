//! Platform LLM Client - All LLM requests go through platform-server
//!
//! This module replaces direct LLM API calls with centralized requests
//! through platform-server, which handles:
//! - API key lookup per agent
//! - Cost tracking
//! - Provider routing

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info};

/// Platform LLM client configuration
#[derive(Debug, Clone)]
pub struct PlatformLlmConfig {
    /// Platform server URL
    pub platform_url: String,
    /// Agent hash (to identify which miner's API key to use)
    pub agent_hash: String,
    /// Validator hotkey (for audit)
    pub validator_hotkey: String,
    /// Model to use (optional)
    pub model: Option<String>,
    /// Max tokens
    pub max_tokens: u32,
    /// Temperature
    pub temperature: f32,
    /// Timeout in seconds
    pub timeout_secs: u64,
}

impl Default for PlatformLlmConfig {
    fn default() -> Self {
        Self {
            platform_url: std::env::var("PLATFORM_URL")
                .unwrap_or_else(|_| "https://chain.platform.network".to_string()),
            agent_hash: String::new(),
            validator_hotkey: String::new(),
            model: None,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_secs: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: content.to_string(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: content.to_string(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PlatformLlmRequest {
    agent_hash: String,
    validator_hotkey: String,
    messages: Vec<ChatMessage>,
    model: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformLlmResponse {
    pub success: bool,
    pub content: Option<String>,
    pub model: Option<String>,
    pub usage: Option<LlmUsage>,
    pub cost_usd: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Platform LLM client - routes all requests through platform-server
pub struct PlatformLlmClient {
    client: Client,
    config: PlatformLlmConfig,
}

impl PlatformLlmClient {
    pub fn new(config: PlatformLlmConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self { client, config })
    }

    /// Create a new client for a specific agent evaluation
    pub fn for_agent(platform_url: &str, agent_hash: &str, validator_hotkey: &str) -> Result<Self> {
        Self::new(PlatformLlmConfig {
            platform_url: platform_url.to_string(),
            agent_hash: agent_hash.to_string(),
            validator_hotkey: validator_hotkey.to_string(),
            ..Default::default()
        })
    }

    /// Send a chat completion request through platform-server
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!("{}/api/v1/llm/chat", self.config.platform_url);

        let request = PlatformLlmRequest {
            agent_hash: self.config.agent_hash.clone(),
            validator_hotkey: self.config.validator_hotkey.clone(),
            messages,
            model: self.config.model.clone(),
            max_tokens: Some(self.config.max_tokens),
            temperature: Some(self.config.temperature),
        };

        debug!(
            "Platform LLM request for agent {} via {}",
            &self.config.agent_hash[..16.min(self.config.agent_hash.len())],
            self.config.platform_url
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Platform LLM request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Platform LLM error {}: {}", status, text));
        }

        let result: PlatformLlmResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Invalid platform response: {}", e))?;

        if !result.success {
            return Err(anyhow!(
                "Platform LLM failed: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        let content = result
            .content
            .ok_or_else(|| anyhow!("No content in response"))?;

        if let Some(usage) = &result.usage {
            info!(
                "LLM response: {} tokens, cost: ${:.4}",
                usage.total_tokens,
                result.cost_usd.unwrap_or(0.0)
            );
        }

        Ok(content)
    }

    /// Send a chat completion and get full response with usage
    pub async fn chat_with_usage(&self, messages: Vec<ChatMessage>) -> Result<PlatformLlmResponse> {
        let url = format!("{}/api/v1/llm/chat", self.config.platform_url);

        let request = PlatformLlmRequest {
            agent_hash: self.config.agent_hash.clone(),
            validator_hotkey: self.config.validator_hotkey.clone(),
            messages,
            model: self.config.model.clone(),
            max_tokens: Some(self.config.max_tokens),
            temperature: Some(self.config.temperature),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Platform LLM request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Platform LLM error {}: {}", status, text));
        }

        let result: PlatformLlmResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Invalid platform response: {}", e))?;

        Ok(result)
    }

    /// Get agent hash
    pub fn agent_hash(&self) -> &str {
        &self.config.agent_hash
    }

    /// Get total cost so far (from last response)
    pub fn platform_url(&self) -> &str {
        &self.config.platform_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let sys = ChatMessage::system("You are helpful");
        assert_eq!(sys.role, "system");

        let user = ChatMessage::user("Hello");
        assert_eq!(user.role, "user");

        let asst = ChatMessage::assistant("Hi there");
        assert_eq!(asst.role, "assistant");
    }

    #[test]
    fn test_config_default() {
        let config = PlatformLlmConfig::default();
        assert_eq!(config.platform_url, "https://chain.platform.network");
        assert_eq!(config.max_tokens, 4096);
    }
}
