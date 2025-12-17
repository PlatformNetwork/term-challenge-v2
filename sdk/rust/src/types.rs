//! Protocol types for Term Challenge.

use serde::{Deserialize, Serialize};

/// Request from the harness.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Task to complete
    pub instruction: String,
    /// Step number (1-indexed)
    pub step: u32,
    /// Previous command executed
    pub last_command: Option<String>,
    /// Output from last command
    pub output: Option<String>,
    /// Exit code from last command
    pub exit_code: Option<i32>,
    /// Current working directory
    #[serde(default = "default_cwd")]
    pub cwd: String,
}

fn default_cwd() -> String {
    "/app".to_string()
}

impl Request {
    /// Parse request from JSON string.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    
    /// True if this is the first step.
    pub fn is_first(&self) -> bool {
        self.step == 1
    }
    
    /// True if last command succeeded.
    pub fn is_ok(&self) -> bool {
        self.exit_code == Some(0)
    }
    
    /// True if last command failed.
    pub fn failed(&self) -> bool {
        matches!(self.exit_code, Some(code) if code != 0)
    }
    
    /// Check if output contains pattern (case-insensitive).
    pub fn has(&self, pattern: &str) -> bool {
        self.output
            .as_ref()
            .map(|o| o.to_lowercase().contains(&pattern.to_lowercase()))
            .unwrap_or(false)
    }
    
    /// Check if output matches any pattern.
    pub fn has_any(&self, patterns: &[&str]) -> bool {
        patterns.iter().any(|p| self.has(p))
    }
}

/// Response to the harness.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Response {
    /// Command to execute (None = no command)
    pub command: Option<String>,
    /// True when task is complete
    pub task_complete: bool,
}

impl Response {
    /// Create response with a command.
    pub fn cmd(command: impl Into<String>) -> Self {
        Self {
            command: Some(command.into()),
            task_complete: false,
        }
    }
    
    /// Create response marking task complete.
    pub fn done() -> Self {
        Self {
            command: None,
            task_complete: true,
        }
    }
    
    /// Mark task as complete.
    pub fn complete(mut self) -> Self {
        self.task_complete = true;
        self
    }
    
    /// Convert to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"command":null,"task_complete":true}"#.to_string()
        })
    }
    
    /// Parse response from LLM output.
    pub fn from_llm(text: &str) -> Self {
        // Try to find JSON in response
        let text = text.trim();
        
        // Remove markdown code blocks
        let text = if text.contains("```") {
            if let Some(start) = text.find('{') {
                if let Some(end) = text.rfind('}') {
                    &text[start..=end]
                } else {
                    text
                }
            } else {
                text
            }
        } else {
            text
        };
        
        // Find JSON object
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                    return Self {
                        command: data.get("command").and_then(|v| v.as_str()).map(String::from),
                        task_complete: data.get("task_complete").and_then(|v| v.as_bool()).unwrap_or(false),
                    };
                }
            }
        }
        
        Self::done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_request_parse() {
        let json = r#"{"instruction":"test","step":1,"cwd":"/app"}"#;
        let req = Request::parse(json).unwrap();
        assert_eq!(req.instruction, "test");
        assert_eq!(req.step, 1);
        assert!(req.is_first());
    }
    
    #[test]
    fn test_response_cmd() {
        let resp = Response::cmd("ls -la");
        assert_eq!(resp.command, Some("ls -la".to_string()));
        assert!(!resp.task_complete);
    }
    
    #[test]
    fn test_response_from_llm() {
        let text = r#"{"command": "ls", "task_complete": false}"#;
        let resp = Response::from_llm(text);
        assert_eq!(resp.command, Some("ls".to_string()));
        
        let text = "Some text ```json\n{\"command\": \"pwd\", \"task_complete\": true}\n```";
        let resp = Response::from_llm(text);
        assert_eq!(resp.command, Some("pwd".to_string()));
        assert!(resp.task_complete);
    }
}
