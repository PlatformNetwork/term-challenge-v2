//! LLM Agent - Professional terminal agent powered by LLM.
//!
//! Usage:
//!   export OPENROUTER_API_KEY="sk-or-..."
//!   cargo run --release
//!
//!   # Or with term CLI:
//!   term bench agent -a ./target/release/llm_agent -t ~/.cache/term-challenge/datasets/hello-world

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, Write};
use tokio::sync::Mutex;

// =============================================================================
// Protocol Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Command {
    keystrokes: String,
    #[serde(default = "default_duration")]
    duration: f64,
}

fn default_duration() -> f64 { 1.0 }

impl Command {
    fn new(keystrokes: impl Into<String>) -> Self {
        Self { keystrokes: keystrokes.into(), duration: 1.0 }
    }
}

#[derive(Debug, Deserialize)]
struct AgentRequest {
    instruction: String,
    screen: String,
    step: u32,
}

#[derive(Debug, Serialize)]
struct AgentResponse {
    analysis: String,
    plan: String,
    commands: Vec<Command>,
    task_complete: bool,
}

impl AgentResponse {
    fn error(msg: impl Into<String>) -> Self {
        Self {
            analysis: format!("Error: {}", msg.into()),
            plan: "Cannot continue".into(),
            commands: vec![],
            task_complete: false,
        }
    }
}

// =============================================================================
// LLM Client
// =============================================================================

const SYSTEM_PROMPT: &str = r#"You are an expert terminal agent. Complete tasks using terminal commands only.

Respond with JSON:
{
  "analysis": "What you observe in the terminal",
  "plan": "Your step-by-step plan",
  "commands": [{"keystrokes": "command\n", "duration": 1.0}],
  "task_complete": false
}

Rules:
1. Use \n at end of commands to execute
2. Verify success before setting task_complete=true
3. Use appropriate durations (longer for installs)"#;

#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatContent,
}

#[derive(Debug, Deserialize)]
struct ChatContent {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

struct LLMClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    total_cost: f64,
    total_tokens: u32,
    request_count: u32,
}

impl LLMClient {
    fn new() -> Result<Self> {
        let provider = env::var("LLM_PROVIDER").unwrap_or_else(|_| "openrouter".to_string());
        
        let (base_url, default_model, env_key) = match provider.as_str() {
            "chutes" | "ch" => (
                "https://llm.chutes.ai/v1",
                "Qwen/Qwen3-32B",
                "CHUTES_API_KEY"
            ),
            _ => (
                "https://openrouter.ai/api/v1",
                "anthropic/claude-3-haiku",
                "OPENROUTER_API_KEY"
            ),
        };
        
        let api_key = env::var(env_key)
            .or_else(|_| env::var("LLM_API_KEY"))
            .context(format!("Set {} or LLM_API_KEY", env_key))?;
        
        let model = env::var("LLM_MODEL").unwrap_or_else(|_| default_model.to_string());
        
        eprintln!("[LLMAgent] Initialized: {}/{}", provider, model);
        
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            model,
            api_key,
            total_cost: 0.0,
            total_tokens: 0,
            request_count: 0,
        })
    }
    
    async fn chat(&mut self, messages: &[ChatMessage]) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 4096
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://term-challenge.ai")
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }
        
        let result: ChatResponse = response.json().await?;
        
        // Track usage
        if let Some(usage) = result.usage {
            let prompt = usage.prompt_tokens.unwrap_or(0);
            let completion = usage.completion_tokens.unwrap_or(0);
            self.total_tokens += prompt + completion;
            
            // Estimate cost (simplified)
            let cost = (prompt as f64 / 1_000_000.0) * 0.25 
                     + (completion as f64 / 1_000_000.0) * 1.25;
            self.total_cost += cost;
        }
        self.request_count += 1;
        
        // Get content
        let mut content = result.choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        
        // Remove <think> blocks
        while let Some(start) = content.find("<think>") {
            if let Some(end) = content.find("</think>") {
                content = format!("{}{}", &content[..start], &content[end + 8..]);
            } else {
                break;
            }
        }
        
        Ok(content.trim().to_string())
    }
}

// =============================================================================
// Agent
// =============================================================================

struct LLMAgent {
    client: Mutex<LLMClient>,
    history: Mutex<Vec<ChatMessage>>,
}

impl LLMAgent {
    fn new() -> Result<Self> {
        Ok(Self {
            client: Mutex::new(LLMClient::new()?),
            history: Mutex::new(Vec::new()),
        })
    }
    
    async fn step(&self, request: &AgentRequest) -> AgentResponse {
        // Build prompt
        let user_content = format!(
            "## Task\n{}\n\n## Terminal (Step {})\n```\n{}\n```\n\nRespond with JSON.",
            request.instruction, request.step, request.screen
        );
        
        // Build messages
        let mut messages = vec![
            ChatMessage { role: "system".into(), content: SYSTEM_PROMPT.into() },
        ];
        
        // Add history
        {
            let history = self.history.lock().await;
            messages.extend(history.iter().cloned().map(|m| ChatMessage {
                role: m.role,
                content: m.content,
            }));
        }
        
        messages.push(ChatMessage { role: "user".into(), content: user_content.clone() });
        
        // Call LLM
        let content = {
            let mut client = self.client.lock().await;
            match client.chat(&messages).await {
                Ok(c) => {
                    eprintln!(
                        "[LLMAgent] Step {}: {} tokens, ${:.4} (total: ${:.4})",
                        request.step, client.total_tokens, 
                        client.total_cost - (client.total_cost / client.request_count as f64),
                        client.total_cost
                    );
                    c
                }
                Err(e) => {
                    eprintln!("[LLMAgent] Error: {}", e);
                    return AgentResponse::error(e.to_string());
                }
            }
        };
        
        // Update history (keep last 10 exchanges)
        {
            let mut history = self.history.lock().await;
            history.push(ChatMessage { role: "user".into(), content: user_content });
            history.push(ChatMessage { role: "assistant".into(), content: content.clone() });
            if history.len() > 20 {
                *history = history[history.len()-20..].to_vec();
            }
        }
        
        // Parse response
        self.parse_response(&content)
    }
    
    fn parse_response(&self, content: &str) -> AgentResponse {
        // Find JSON
        let start = match content.find('{') {
            Some(i) => i,
            None => return AgentResponse::error("No JSON found"),
        };
        let end = match content.rfind('}') {
            Some(i) => i,
            None => return AgentResponse::error("No JSON found"),
        };
        
        let json_str = &content[start..=end];
        
        // Parse
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(data) => {
                let commands: Vec<Command> = data.get("commands")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|c| {
                        Some(Command {
                            keystrokes: c.get("keystrokes")?.as_str()?.to_string(),
                            duration: c.get("duration").and_then(|d| d.as_f64()).unwrap_or(1.0),
                        })
                    }).collect())
                    .unwrap_or_default();
                
                AgentResponse {
                    analysis: data.get("analysis").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    plan: data.get("plan").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    commands,
                    task_complete: data.get("task_complete").and_then(|v| v.as_bool()).unwrap_or(false),
                }
            }
            Err(e) => {
                eprintln!("[LLMAgent] Parse error: {}", e);
                AgentResponse::error(format!("Parse error: {}", e))
            }
        }
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let agent = LLMAgent::new()?;
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        
        let response = match serde_json::from_str::<AgentRequest>(&line) {
            Ok(request) => agent.step(&request).await,
            Err(e) => AgentResponse::error(format!("Invalid request: {}", e)),
        };
        
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    
    // Print final stats
    let client = agent.client.lock().await;
    eprintln!(
        "[LLMAgent] Session complete: {} requests, ${:.4} total",
        client.request_count, client.total_cost
    );
    
    Ok(())
}
