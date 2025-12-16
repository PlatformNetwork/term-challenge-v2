//! LLM Client for Term Challenge agents.

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Instant;
use tracing::{debug, info};

/// Supported LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenRouter,
    Chutes,
    OpenAI,
    Anthropic,
    Custom,
}

impl Provider {
    /// Parse provider from string.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openrouter" | "or" => Self::OpenRouter,
            "chutes" | "ch" => Self::Chutes,
            "openai" => Self::OpenAI,
            "anthropic" => Self::Anthropic,
            _ => Self::Custom,
        }
    }

    /// Get base URL for provider.
    pub fn base_url(&self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Chutes => "https://llm.chutes.ai/v1",
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Custom => "",
        }
    }

    /// Get environment variable name for API key.
    pub fn env_key(&self) -> &'static str {
        match self {
            Self::OpenRouter => "OPENROUTER_API_KEY",
            Self::Chutes => "CHUTES_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::Custom => "LLM_API_KEY",
        }
    }

    /// Get default model for provider.
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::OpenRouter => "anthropic/claude-3-haiku",
            Self::Chutes => "Qwen/Qwen3-32B",
            Self::OpenAI => "gpt-4o-mini",
            Self::Anthropic => "claude-3-haiku-20240307",
            Self::Custom => "anthropic/claude-3-haiku",
        }
    }
}

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// Response from LLM chat completion.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost: f64,
    pub latency_ms: u64,
}

/// Tracks cumulative costs across requests.
#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub total_cost: f64,
    pub total_tokens: u32,
    pub total_prompt_tokens: u32,
    pub total_completion_tokens: u32,
    pub request_count: u32,
    pub budget: Option<f64>,
}

impl CostTracker {
    /// Create a new tracker with optional budget.
    pub fn new(budget: Option<f64>) -> Self {
        Self { budget, ..Default::default() }
    }

    /// Add a response to the tracker.
    pub fn add(&mut self, response: &ChatResponse) {
        self.total_cost += response.cost;
        self.total_tokens += response.total_tokens;
        self.total_prompt_tokens += response.prompt_tokens;
        self.total_completion_tokens += response.completion_tokens;
        self.request_count += 1;
    }

    /// Check if over budget.
    pub fn over_budget(&self) -> bool {
        self.budget.map_or(false, |b| self.total_cost >= b)
    }

    /// Get remaining budget.
    pub fn remaining(&self) -> Option<f64> {
        self.budget.map(|b| (b - self.total_cost).max(0.0))
    }
}

// Pricing per 1M tokens (prompt, completion)
fn get_pricing(model: &str) -> (f64, f64) {
    match model {
        "anthropic/claude-3-haiku" => (0.25, 1.25),
        "anthropic/claude-3-sonnet" | "anthropic/claude-sonnet-4" => (3.0, 15.0),
        "anthropic/claude-3-opus" => (15.0, 75.0),
        "openai/gpt-4o" | "gpt-4o" => (5.0, 15.0),
        "openai/gpt-4o-mini" | "gpt-4o-mini" => (0.15, 0.60),
        "Qwen/Qwen3-32B" => (0.10, 0.30),
        "Qwen/Qwen3-235B-A22B" => (0.20, 0.60),
        _ => (0.50, 1.50), // Default
    }
}

/// Calculate cost for a request.
pub fn estimate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    let (prompt_price, completion_price) = get_pricing(model);
    (prompt_tokens as f64 / 1_000_000.0) * prompt_price
        + (completion_tokens as f64 / 1_000_000.0) * completion_price
}

// API types
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

/// Multi-provider LLM client with cost tracking.
pub struct LLMClient {
    provider: Provider,
    api_key: String,
    base_url: String,
    pub model: String,
    pub cost_tracker: CostTracker,
    client: Client,
    temperature: f32,
    max_tokens: u32,
}

impl LLMClient {
    /// Create a new LLM client.
    ///
    /// # Arguments
    ///
    /// * `provider` - LLM provider (openrouter, chutes, openai, anthropic).
    /// * `api_key` - API key (or None to use environment variable).
    /// * `model` - Model name (or None for provider default).
    /// * `budget` - Maximum cost budget in USD.
    pub fn new(
        provider: Provider,
        api_key: Option<String>,
        model: Option<String>,
        budget: Option<f64>,
    ) -> Result<Self> {
        // Get API key
        let api_key = api_key
            .or_else(|| env::var(provider.env_key()).ok())
            .or_else(|| env::var("LLM_API_KEY").ok())
            .context(format!("API key required. Set {} or LLM_API_KEY", provider.env_key()))?;

        let model = model.unwrap_or_else(|| provider.default_model().to_string());
        let base_url = provider.base_url().to_string();

        info!("LLM client initialized: {:?}/{}", provider, model);

        Ok(Self {
            provider,
            api_key,
            base_url,
            model,
            cost_tracker: CostTracker::new(budget),
            client: Client::new(),
            temperature: 0.7,
            max_tokens: 4096,
        })
    }

    /// Create client from environment variables.
    pub fn from_env() -> Result<Self> {
        let provider_str = env::var("LLM_PROVIDER").unwrap_or_else(|_| "openrouter".to_string());
        let provider = Provider::from_str(&provider_str);
        let model = env::var("LLM_MODEL").ok();
        let budget = env::var("LLM_BUDGET").ok().and_then(|s| s.parse().ok());

        Self::new(provider, None, model, budget)
    }

    /// Set temperature for chat completions.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    /// Set max tokens for chat completions.
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = max;
        self
    }

    /// Send a chat completion request.
    pub async fn chat(&mut self, messages: &[Message]) -> Result<ChatResponse> {
        self.chat_with_options(messages, None, None).await
    }

    /// Send a chat completion with custom options.
    pub async fn chat_with_options(
        &mut self,
        messages: &[Message],
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<ChatResponse> {
        // Check budget
        if self.cost_tracker.over_budget() {
            bail!(
                "Over budget: ${:.4} >= ${:.4}",
                self.cost_tracker.total_cost,
                self.cost_tracker.budget.unwrap_or(0.0)
            );
        }

        let url = format!("{}/chat/completions", self.base_url);
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: temperature.unwrap_or(self.temperature),
            max_tokens: max_tokens.unwrap_or(self.max_tokens),
        };

        let start = Instant::now();

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://term-challenge.ai")
            .json(&request)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("API error {}: {}", status, body);
        }

        let result: ApiResponse = response.json().await.context("Failed to parse response")?;
        let latency_ms = start.elapsed().as_millis() as u64;

        // Parse response
        let choice = result.choices.first().context("No choices in response")?;
        let mut content = choice.message.content.clone();

        // Remove <think> blocks (Qwen models)
        while let Some(start) = content.find("<think>") {
            if let Some(end) = content.find("</think>") {
                content = format!("{}{}", &content[..start], &content[end + 8..]);
            } else {
                break;
            }
        }
        content = content.trim().to_string();

        // Get usage
        let usage = result.usage.unwrap_or(Usage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        });
        let prompt_tokens = usage.prompt_tokens.unwrap_or(0);
        let completion_tokens = usage.completion_tokens.unwrap_or(0);
        let total_tokens = usage.total_tokens.unwrap_or(prompt_tokens + completion_tokens);

        // Calculate cost
        let cost = estimate_cost(&self.model, prompt_tokens, completion_tokens);

        let response = ChatResponse {
            content,
            model: self.model.clone(),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cost,
            latency_ms,
        };

        // Track
        self.cost_tracker.add(&response);

        debug!(
            "Chat completed: {} tokens, ${:.4}, {}ms",
            total_tokens, cost, latency_ms
        );

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_parsing() {
        assert_eq!(Provider::from_str("openrouter"), Provider::OpenRouter);
        assert_eq!(Provider::from_str("OR"), Provider::OpenRouter);
        assert_eq!(Provider::from_str("chutes"), Provider::Chutes);
        assert_eq!(Provider::from_str("unknown"), Provider::Custom);
    }

    #[test]
    fn test_message_constructors() {
        let sys = Message::system("You are helpful");
        assert_eq!(sys.role, "system");

        let user = Message::user("Hello");
        assert_eq!(user.role, "user");

        let asst = Message::assistant("Hi there");
        assert_eq!(asst.role, "assistant");
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new(Some(1.0));

        let response = ChatResponse {
            content: "".into(),
            model: "test".into(),
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cost: 0.5,
            latency_ms: 100,
        };

        tracker.add(&response);
        assert_eq!(tracker.request_count, 1);
        assert_eq!(tracker.total_cost, 0.5);
        assert!(!tracker.over_budget());

        tracker.add(&response);
        assert!(tracker.over_budget());
    }

    #[test]
    fn test_estimate_cost() {
        let cost = estimate_cost("anthropic/claude-3-haiku", 1000, 500);
        assert!(cost > 0.0);
    }
}
