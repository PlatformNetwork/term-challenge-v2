//! LLM Client for Agent Execution

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::terminal_harness::{AgentRequest, AgentResponse};

/// LLM configuration
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_base: std::env::var("LLM_API_BASE")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-3-haiku".to_string()),
            max_tokens: 1024,
            temperature: 0.3,
            timeout_secs: 60,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

/// LLM client
pub struct LlmClient {
    client: Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;
        
        info!("LLM client: model={}", config.model);
        Ok(Self { client, config })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(LlmConfig::default())
    }

    fn system_prompt(&self) -> String {
        r#"You are a terminal agent. Execute shell commands to complete tasks.

RESPONSE FORMAT (JSON only):
{"command": "your shell command here", "task_complete": false}

When done:
{"command": null, "task_complete": true}

RULES:
- One command at a time
- You receive the output of each command
- Set task_complete=true only when finished
- Respond with valid JSON only, no other text"#.to_string()
    }

    fn build_user_message(&self, req: &AgentRequest) -> String {
        let mut msg = format!("TASK: {}\n\nSTEP: {}\nCWD: {}", 
            req.instruction, req.step, req.cwd);
        
        if let Some(cmd) = &req.last_command {
            msg.push_str(&format!("\n\nLAST COMMAND: {}", cmd));
        }
        if let Some(code) = req.exit_code {
            msg.push_str(&format!("\nEXIT CODE: {}", code));
        }
        if let Some(out) = &req.output {
            let truncated = if out.len() > 4000 { 
                format!("{}...[truncated]", &out[..4000]) 
            } else { 
                out.clone() 
            };
            msg.push_str(&format!("\n\nOUTPUT:\n{}", truncated));
        }
        
        msg
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse> {
        let messages = vec![
            Message { role: "system".to_string(), content: self.system_prompt() },
            Message { role: "user".to_string(), content: self.build_user_message(&request) },
        ];

        debug!("Calling LLM: step={}", request.step);

        let resp = self.client
            .post(format!("{}/chat/completions", self.config.api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&ChatRequest {
                model: self.config.model.clone(),
                messages,
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
            })
            .send()
            .await
            .context("LLM request failed")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM error: {}", err);
        }

        let chat: ChatResponse = resp.json().await?;
        let content = chat.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        debug!("LLM response: {}", content);
        crate::terminal_harness::parse_agent_response(&content)
    }
}

/// Source code agent - runs Python code locally
pub struct SourceCodeAgent {
    source: String,
}

impl SourceCodeAgent {
    pub fn new(source: String) -> Self {
        Self { source }
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse> {
        use tokio::process::Command;
        use tokio::io::AsyncWriteExt;

        let input = serde_json::to_string(&request)?;
        
        let temp = tempfile::tempdir()?;
        let script = temp.path().join("agent.py");
        tokio::fs::write(&script, &self.source).await?;

        let mut child = Command::new("python3")
            .arg(&script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await?;
        }

        let output = child.wait_with_output().await?;
        
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Agent script failed: {}", err);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        crate::terminal_harness::parse_agent_response(&stdout)
    }
}

/// Agent type
pub enum Agent {
    Llm(LlmClient),
    Source(SourceCodeAgent),
}

impl Agent {
    pub fn from_source(code: String) -> Self {
        Self::Source(SourceCodeAgent::new(code))
    }

    pub fn from_llm(config: LlmConfig) -> Result<Self> {
        Ok(Self::Llm(LlmClient::new(config)?))
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse> {
        match self {
            Self::Llm(c) => c.execute(request).await,
            Self::Source(a) => a.execute(request).await,
        }
    }
}
