//! LLM client with dynamic model selection.
//!
//! Agents can use multiple models - specify the model on each call.
//!
//! ```rust,no_run
//! use term_sdk::LLM;
//!
//! let mut llm = LLM::new();
//!
//! // Use different models for different tasks
//! let quick = llm.ask("Simple question", "claude-3-haiku")?;
//! let complex = llm.ask("Complex analysis", "claude-3-opus")?;
//! let code = llm.ask("Write code", "gpt-4o")?;
//!
//! // Get stats per model
//! println!("{:?}", llm.get_stats(Some("claude-3-haiku")));
//! ```

use std::collections::HashMap;
use std::env;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::types::{Tool, FunctionCall};

/// LLM response.
#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub text: String,
    pub model: String,
    pub tokens: u32,
    pub cost: f64,
    pub latency_ms: u64,
    pub function_calls: Vec<FunctionCall>,
    pub raw: Option<serde_json::Value>,
}

impl LLMResponse {
    pub fn has_function_calls(&self) -> bool {
        !self.function_calls.is_empty()
    }
}

/// Chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: "tool".to_string(), content: Some(content.into()), tool_calls: None, tool_call_id: Some(tool_call_id.into()) }
    }
}

/// Model usage statistics.
#[derive(Debug, Clone, Default)]
pub struct ModelStats {
    pub tokens: u32,
    pub cost: f64,
    pub requests: u32,
}

pub type FunctionHandler = Box<dyn Fn(&HashMap<String, serde_json::Value>) -> Result<String, String> + Send + Sync>;

/// LLM client with dynamic model selection.
///
/// ```rust,no_run
/// use term_sdk::LLM;
///
/// let mut llm = LLM::new();
///
/// // Different models for different tasks
/// let quick = llm.ask("Quick question", "claude-3-haiku")?;
/// let detailed = llm.ask("Detailed analysis", "claude-3-opus")?;
///
/// // With parameters
/// let result = llm.chat_with_model(
///     &[term_sdk::Message::user("Hello")],
///     "gpt-4o",
///     Some(0.7),  // temperature
///     Some(2000), // max_tokens
///     None,       // no tools
/// )?;
/// ```
pub struct LLM {
    default_model: Option<String>,
    temperature: f32,
    max_tokens: u32,
    api_url: String,
    api_key: String,
    client: reqwest::blocking::Client,
    function_handlers: HashMap<String, FunctionHandler>,
    stats: HashMap<String, ModelStats>,

    pub total_tokens: u32,
    pub total_cost: f64,
    pub request_count: u32,
}

impl LLM {
    /// Create new LLM client without default model.
    pub fn new() -> Self {
        Self::with_default_model(None)
    }

    /// Create new LLM client with default model.
    pub fn with_default_model(default_model: Option<String>) -> Self {
        let api_url = env::var("LLM_API_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1/chat/completions".to_string());
        let api_key = env::var("LLM_API_KEY")
            .or_else(|_| env::var("OPENROUTER_API_KEY"))
            .unwrap_or_default();

        if api_key.is_empty() {
            eprintln!("[llm] Warning: LLM_API_KEY or OPENROUTER_API_KEY not set");
        }

        Self {
            default_model,
            temperature: 0.3,
            max_tokens: 4096,
            api_url,
            api_key,
            client: reqwest::blocking::Client::new(),
            function_handlers: HashMap::new(),
            stats: HashMap::new(),
            total_tokens: 0,
            total_cost: 0.0,
            request_count: 0,
        }
    }

    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = t;
        self
    }

    pub fn max_tokens(mut self, t: u32) -> Self {
        self.max_tokens = t;
        self
    }

    fn get_model(&self, model: Option<&str>) -> Result<String, String> {
        if let Some(m) = model {
            return Ok(m.to_string());
        }
        if let Some(ref m) = self.default_model {
            return Ok(m.clone());
        }
        Err("No model specified. Pass model parameter or set default_model.".to_string())
    }

    /// Register a function handler.
    pub fn register_function<F>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(&HashMap<String, serde_json::Value>) -> Result<String, String> + Send + Sync + 'static,
    {
        self.function_handlers.insert(name.into(), Box::new(handler));
    }

    /// Ask a question with specified model.
    pub fn ask(&mut self, prompt: &str, model: &str) -> Result<LLMResponse, String> {
        self.chat_with_model(&[Message::user(prompt)], model, None, None, None)
    }

    /// Ask with system prompt.
    pub fn ask_with_system(&mut self, system: &str, prompt: &str, model: &str) -> Result<LLMResponse, String> {
        self.chat_with_model(&[Message::system(system), Message::user(prompt)], model, None, None, None)
    }

    /// Chat with default model.
    pub fn chat(&mut self, messages: &[Message], tools: Option<&[Tool]>) -> Result<LLMResponse, String> {
        let model = self.get_model(None)?;
        self.chat_with_model(messages, &model, None, None, tools)
    }

    /// Chat with specified model and options.
    pub fn chat_with_model(
        &mut self,
        messages: &[Message],
        model: &str,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        tools: Option<&[Tool]>,
    ) -> Result<LLMResponse, String> {
        let start = Instant::now();
        let temp = temperature.unwrap_or(self.temperature);
        let tokens = max_tokens.unwrap_or(self.max_tokens);

        #[derive(Serialize)]
        struct ChatRequest<'a> {
            model: &'a str,
            messages: &'a [Message],
            temperature: f32,
            max_tokens: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            tools: Option<Vec<serde_json::Value>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            tool_choice: Option<&'static str>,
        }

        let tools_json = tools.map(|t| t.iter().map(|tool| tool.to_json()).collect());

        let request = ChatRequest {
            model,
            messages,
            temperature: temp,
            max_tokens: tokens,
            tools: tools_json,
            tool_choice: if tools.is_some() { Some("auto") } else { None },
        };

        let response = self.client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().map_err(|e| e.to_string())?;

        let choice = data.get("choices").and_then(|c| c.get(0)).unwrap_or(&serde_json::Value::Null);
        let message = choice.get("message").unwrap_or(&serde_json::Value::Null);

        let text = message.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        // Parse function calls
        let mut function_calls = Vec::new();
        if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                if tc.get("type").and_then(|t| t.as_str()) == Some("function") {
                    if let Some(func) = tc.get("function") {
                        let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                        let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                        let arguments: HashMap<String, serde_json::Value> = 
                            serde_json::from_str(args_str).unwrap_or_default();
                        let id = tc.get("id").and_then(|i| i.as_str()).map(String::from);
                        function_calls.push(FunctionCall { name, arguments, id });
                    }
                }
            }
        }

        let usage = data.get("usage").unwrap_or(&serde_json::Value::Null);
        let prompt_tokens = usage.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
        let completion_tokens = usage.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
        let total_tokens = prompt_tokens + completion_tokens;
        let cost = self.calculate_cost(model, prompt_tokens, completion_tokens);
        let latency_ms = start.elapsed().as_millis() as u64;

        // Update stats
        self.total_tokens += total_tokens;
        self.total_cost += cost;
        self.request_count += 1;

        // Per-model stats
        let model_stats = self.stats.entry(model.to_string()).or_default();
        model_stats.tokens += total_tokens;
        model_stats.cost += cost;
        model_stats.requests += 1;

        eprintln!("[llm] {}: {} tokens, ${:.4}, {}ms", model, total_tokens, cost, latency_ms);

        Ok(LLMResponse {
            text,
            model: model.to_string(),
            tokens: total_tokens,
            cost,
            latency_ms,
            function_calls,
            raw: Some(data),
        })
    }

    /// Execute a registered function.
    pub fn execute_function(&self, call: &FunctionCall) -> Result<String, String> {
        let handler = self.function_handlers.get(&call.name)
            .ok_or_else(|| format!("Unknown function: {}", call.name))?;
        handler(&call.arguments)
    }

    /// Chat with automatic function execution.
    pub fn chat_with_functions(
        &mut self,
        messages: &[Message],
        tools: &[Tool],
        model: &str,
        max_iterations: usize,
    ) -> Result<LLMResponse, String> {
        let mut msgs = messages.to_vec();

        for _ in 0..max_iterations {
            let response = self.chat_with_model(&msgs, model, None, None, Some(tools))?;

            if response.function_calls.is_empty() {
                return Ok(response);
            }

            for call in &response.function_calls {
                let result = match self.execute_function(call) {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                msgs.push(Message {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![serde_json::json!({
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.name,
                            "arguments": serde_json::to_string(&call.arguments).unwrap_or_default(),
                        }
                    })]),
                    tool_call_id: None,
                });

                msgs.push(Message::tool(call.id.as_deref().unwrap_or(""), result));
            }
        }

        self.chat_with_model(&msgs, model, None, None, Some(tools))
    }

    /// Get usage stats, optionally for a specific model.
    pub fn get_stats(&self, model: Option<&str>) -> Option<ModelStats> {
        model.and_then(|m| self.stats.get(m).cloned())
    }

    /// Get all stats.
    pub fn get_all_stats(&self) -> &HashMap<String, ModelStats> {
        &self.stats
    }

    fn calculate_cost(&self, model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        let (input_price, output_price) = if model.contains("claude-3-haiku") {
            (0.25, 1.25)
        } else if model.contains("claude-3-sonnet") {
            (3.0, 15.0)
        } else if model.contains("claude-3-opus") {
            (15.0, 75.0)
        } else if model.contains("gpt-4o-mini") {
            (0.15, 0.6)
        } else if model.contains("gpt-4o") {
            (5.0, 15.0)
        } else if model.contains("llama") {
            (0.2, 0.2)
        } else if model.contains("mixtral") {
            (0.5, 0.5)
        } else {
            (0.5, 1.5)
        };

        (prompt_tokens as f64 * input_price + completion_tokens as f64 * output_price) / 1_000_000.0
    }
}

impl Default for LLM {
    fn default() -> Self {
        Self::new()
    }
}
