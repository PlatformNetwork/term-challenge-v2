//! LLM client for Term Challenge agents.
//!
//! Supports multiple providers:
//! - OpenRouter (default)
//! - OpenAI
//! - Anthropic

use std::env;
use std::time::Instant;
use serde::{Deserialize, Serialize};

/// LLM provider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Provider {
    OpenRouter,
    OpenAI,
    Anthropic,
}

impl Provider {
    fn url(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            Provider::OpenAI => "https://api.openai.com/v1/chat/completions",
            Provider::Anthropic => "https://api.anthropic.com/v1/messages",
        }
    }
    
    fn env_key(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "OPENROUTER_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
        }
    }
}

/// LLM response.
#[derive(Debug, Clone)]
pub struct LLMResponse {
    /// Response text
    pub text: String,
    /// Model used
    pub model: String,
    /// Total tokens used
    pub tokens: u32,
    /// Cost in USD
    pub cost: f64,
    /// Latency in milliseconds
    pub latency_ms: u64,
}

/// Chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

/// LLM client.
pub struct LLM {
    provider: Provider,
    model: String,
    api_key: String,
    temperature: f32,
    max_tokens: u32,
    client: reqwest::blocking::Client,
    
    // Stats
    pub total_tokens: u32,
    pub total_cost: f64,
    pub request_count: u32,
}

impl LLM {
    /// Create new LLM client with OpenRouter.
    pub fn new(model: impl Into<String>) -> Self {
        Self::with_provider(Provider::OpenRouter, model)
    }
    
    /// Create new LLM client with specific provider.
    pub fn with_provider(provider: Provider, model: impl Into<String>) -> Self {
        let api_key = env::var(provider.env_key()).unwrap_or_default();
        if api_key.is_empty() {
            eprintln!("[llm] Warning: {} not set", provider.env_key());
        }
        
        Self {
            provider,
            model: model.into(),
            api_key,
            temperature: 0.3,
            max_tokens: 1024,
            client: reqwest::blocking::Client::new(),
            total_tokens: 0,
            total_cost: 0.0,
            request_count: 0,
        }
    }
    
    /// Set temperature.
    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = t;
        self
    }
    
    /// Set max tokens.
    pub fn max_tokens(mut self, t: u32) -> Self {
        self.max_tokens = t;
        self
    }
    
    /// Set API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }
    
    /// Ask a simple question.
    pub fn ask(&mut self, prompt: &str) -> Result<LLMResponse, String> {
        self.chat(&[Message::user(prompt)])
    }
    
    /// Ask with system prompt.
    pub fn ask_with_system(&mut self, system: &str, prompt: &str) -> Result<LLMResponse, String> {
        self.chat(&[Message::system(system), Message::user(prompt)])
    }
    
    /// Chat with messages.
    pub fn chat(&mut self, messages: &[Message]) -> Result<LLMResponse, String> {
        let start = Instant::now();
        
        let response = if self.provider == Provider::Anthropic {
            self.chat_anthropic(messages)?
        } else {
            self.chat_openai(messages)?
        };
        
        let mut response = response;
        response.latency_ms = start.elapsed().as_millis() as u64;
        
        self.total_tokens += response.tokens;
        self.total_cost += response.cost;
        self.request_count += 1;
        
        eprintln!("[llm] {}: {} tokens, ${:.4}, {}ms", 
            response.model, response.tokens, response.cost, response.latency_ms);
        
        Ok(response)
    }
    
    fn chat_openai(&self, messages: &[Message]) -> Result<LLMResponse, String> {
        #[derive(Serialize)]
        struct OpenAIRequest<'a> {
            model: &'a str,
            messages: &'a [Message],
            temperature: f32,
            max_tokens: u32,
        }
        
        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<Choice>,
            usage: Option<Usage>,
        }
        
        #[derive(Deserialize)]
        struct Choice {
            message: ChoiceMessage,
        }
        
        #[derive(Deserialize)]
        struct ChoiceMessage {
            content: String,
        }
        
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
        }
        
        let request = OpenAIRequest {
            model: &self.model,
            messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };
        
        let response = self.client
            .post(self.provider.url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| e.to_string())?;
        
        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }
        
        let data: OpenAIResponse = response.json().map_err(|e| e.to_string())?;
        
        let text = data.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        
        let (prompt_tokens, completion_tokens) = data.usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((0, 0));
        
        let cost = self.calculate_cost(prompt_tokens, completion_tokens);
        
        Ok(LLMResponse {
            text,
            model: self.model.clone(),
            tokens: prompt_tokens + completion_tokens,
            cost,
            latency_ms: 0,
        })
    }
    
    fn chat_anthropic(&self, messages: &[Message]) -> Result<LLMResponse, String> {
        #[derive(Serialize)]
        struct AnthropicRequest<'a> {
            model: &'a str,
            messages: Vec<&'a Message>,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<&'a str>,
            temperature: f32,
            max_tokens: u32,
        }
        
        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<Content>,
            usage: Option<AnthropicUsage>,
        }
        
        #[derive(Deserialize)]
        struct Content {
            text: String,
        }
        
        #[derive(Deserialize)]
        struct AnthropicUsage {
            input_tokens: u32,
            output_tokens: u32,
        }
        
        let mut system = None;
        let mut user_messages = Vec::new();
        for msg in messages {
            if msg.role == "system" {
                system = Some(msg.content.as_str());
            } else {
                user_messages.push(msg);
            }
        }
        
        let request = AnthropicRequest {
            model: &self.model,
            messages: user_messages,
            system,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };
        
        let response = self.client
            .post(self.provider.url())
            .header("x-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .map_err(|e| e.to_string())?;
        
        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }
        
        let data: AnthropicResponse = response.json().map_err(|e| e.to_string())?;
        
        let text = data.content.first()
            .map(|c| c.text.clone())
            .unwrap_or_default();
        
        let (prompt_tokens, completion_tokens) = data.usage
            .map(|u| (u.input_tokens, u.output_tokens))
            .unwrap_or((0, 0));
        
        let cost = self.calculate_cost(prompt_tokens, completion_tokens);
        
        Ok(LLMResponse {
            text,
            model: self.model.clone(),
            tokens: prompt_tokens + completion_tokens,
            cost,
            latency_ms: 0,
        })
    }
    
    fn calculate_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        // Pricing per 1M tokens (input, output)
        let (input_price, output_price) = match self.model.as_str() {
            "anthropic/claude-3-haiku" | "claude-3-haiku-20240307" => (0.25, 1.25),
            "anthropic/claude-3-sonnet" | "claude-3-sonnet-20240229" => (3.0, 15.0),
            "anthropic/claude-3-opus" | "claude-3-opus-20240229" => (15.0, 75.0),
            "openai/gpt-4o" | "gpt-4o" => (5.0, 15.0),
            "openai/gpt-4o-mini" | "gpt-4o-mini" => (0.15, 0.6),
            _ => (0.5, 1.5),
        };
        
        (prompt_tokens as f64 * input_price + completion_tokens as f64 * output_price) / 1_000_000.0
    }
}
