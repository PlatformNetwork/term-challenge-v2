//! LLM Provider implementations

/// Supported LLM providers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// OpenRouter - Access 100+ models
    OpenRouter,
    /// Chutes - Fast, cheap LLM inference
    Chutes,
}

impl Provider {
    /// Get the base URL for the provider
    pub fn base_url(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            Provider::Chutes => "https://llm.chutes.ai/v1/chat/completions",
        }
    }

    /// Get the provider name
    pub fn name(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "openrouter",
            Provider::Chutes => "chutes",
        }
    }

    /// Get the environment variable name for the API key
    pub fn env_key(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "OPENROUTER_API_KEY",
            Provider::Chutes => "CHUTES_API_KEY",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// OpenRouter provider helper
pub struct OpenRouterProvider;

impl OpenRouterProvider {
    /// Get API key from environment
    pub fn api_key_from_env() -> Option<String> {
        std::env::var("OPENROUTER_API_KEY").ok()
    }
}

/// Chutes provider helper
pub struct ChutesProvider;

impl ChutesProvider {
    /// Get API key from environment
    pub fn api_key_from_env() -> Option<String> {
        std::env::var("CHUTES_API_KEY").ok()
    }
}
