//! Protocol types for Term Challenge communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// A command to send to the terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// The exact text to send (include \n to execute).
    pub keystrokes: String,
    /// Seconds to wait after sending (default 1.0).
    #[serde(default = "default_duration")]
    pub duration: f64,
}

fn default_duration() -> f64 {
    1.0
}

// =============================================================================
// Agent Logger
// =============================================================================

/// Log level for agent logs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Success,
    Warning,
    Error,
}

/// A single log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        
        Self {
            level,
            message: message.into(),
            timestamp,
            data: None,
        }
    }

    pub fn with_data(mut self, data: HashMap<String, serde_json::Value>) -> Self {
        self.data = Some(data);
        self
    }
}

/// Logger that captures logs for inclusion in agent responses.
///
/// # Example
///
/// ```rust
/// use term_sdk::{log, LogLevel};
///
/// log::info("Starting task");
/// log::debug("Details", &[("key", "value")]);
/// log::success("Task completed!");
/// log::error("Something failed");
/// ```
pub struct AgentLogger {
    entries: Arc<RwLock<Vec<LogEntry>>>,
    step_entries: Arc<RwLock<Vec<LogEntry>>>,
    verbose: bool,
}

impl Default for AgentLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentLogger {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            step_entries: Arc::new(RwLock::new(Vec::new())),
            verbose: true,
        }
    }

    fn log(&self, level: LogLevel, message: &str, data: Option<HashMap<String, serde_json::Value>>) {
        let mut entry = LogEntry::new(level.clone(), message);
        if let Some(d) = data {
            entry = entry.with_data(d);
        }

        if let Ok(mut entries) = self.entries.write() {
            entries.push(entry.clone());
        }
        if let Ok(mut step_entries) = self.step_entries.write() {
            step_entries.push(entry.clone());
        }

        // Output to stderr for local debugging
        if self.verbose {
            let icon = match level {
                LogLevel::Debug => "🔍",
                LogLevel::Info => "ℹ️",
                LogLevel::Success => "✅",
                LogLevel::Warning => "⚠️",
                LogLevel::Error => "❌",
            };
            eprintln!("{} [{:?}] {}", icon, level, message);
        }
    }

    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message, None);
    }

    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message, None);
    }

    pub fn success(&self, message: &str) {
        self.log(LogLevel::Success, message, None);
    }

    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message, None);
    }

    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message, None);
    }

    pub fn llm_request(&self, provider: &str, model: &str) {
        self.info(&format!("LLM request: {}/{}", provider, model));
    }

    pub fn llm_response(&self, model: &str, tokens: u32, cost: f64, latency_ms: u64) {
        self.success(&format!(
            "LLM response: {} tokens, ${:.4}, {}ms",
            tokens, cost, latency_ms
        ));
    }

    pub fn llm_error(&self, error: &str) {
        self.error(&format!("LLM error: {}", error));
    }

    pub fn get_step_logs(&self) -> Vec<LogEntry> {
        let mut step_entries = self.step_entries.write().unwrap();
        let logs = step_entries.drain(..).collect();
        logs
    }

    pub fn get_all_logs(&self) -> Vec<LogEntry> {
        self.entries.read().unwrap().clone()
    }

    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
        self.step_entries.write().unwrap().clear();
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }
}

// Global logger instance
lazy_static::lazy_static! {
    pub static ref LOGGER: AgentLogger = AgentLogger::new();
}

/// Log convenience functions using the global logger.
pub mod log {
    use super::*;

    pub fn debug(message: &str) {
        LOGGER.debug(message);
    }

    pub fn info(message: &str) {
        LOGGER.info(message);
    }

    pub fn success(message: &str) {
        LOGGER.success(message);
    }

    pub fn warning(message: &str) {
        LOGGER.warning(message);
    }

    pub fn error(message: &str) {
        LOGGER.error(message);
    }

    pub fn llm_request(provider: &str, model: &str) {
        LOGGER.llm_request(provider, model);
    }

    pub fn llm_response(model: &str, tokens: u32, cost: f64, latency_ms: u64) {
        LOGGER.llm_response(model, tokens, cost, latency_ms);
    }

    pub fn llm_error(error: &str) {
        LOGGER.llm_error(error);
    }

    pub fn get_step_logs() -> Vec<LogEntry> {
        LOGGER.get_step_logs()
    }

    pub fn clear() {
        LOGGER.clear();
    }
}

impl Command {
    /// Create a new command with default duration.
    pub fn new(keystrokes: impl Into<String>) -> Self {
        Self {
            keystrokes: keystrokes.into(),
            duration: 1.0,
        }
    }

    /// Create a new command with specified duration.
    pub fn with_duration(keystrokes: impl Into<String>, duration: f64) -> Self {
        Self {
            keystrokes: keystrokes.into(),
            duration,
        }
    }
}

/// Request from harness to agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// The task instruction/goal.
    pub instruction: String,
    /// Current terminal screen content.
    pub screen: String,
    /// Current step number (1-indexed).
    pub step: u32,
}

/// Response from agent to harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// Analysis of the current terminal state.
    pub analysis: String,
    /// Plan for the next steps.
    pub plan: String,
    /// List of commands to execute.
    pub commands: Vec<Command>,
    /// Set true when task is finished.
    pub task_complete: bool,
    /// Optional logs from this step.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub logs: Vec<LogEntry>,
}

impl Default for AgentResponse {
    fn default() -> Self {
        Self {
            analysis: String::new(),
            plan: String::new(),
            commands: Vec::new(),
            task_complete: false,
            logs: Vec::new(),
        }
    }
}

impl AgentResponse {
    /// Create a new empty response.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an error response.
    pub fn error(message: impl Into<String>) -> Self {
        let msg = message.into();
        log::error(&msg);
        Self {
            analysis: format!("Error: {}", msg),
            plan: "Cannot continue due to error".into(),
            commands: Vec::new(),
            task_complete: false,
            logs: log::get_step_logs(),
        }
    }
    
    /// Attach step logs to response.
    pub fn with_logs(mut self) -> Self {
        if self.logs.is_empty() {
            self.logs = log::get_step_logs();
        }
        self
    }

    /// Builder: set analysis.
    pub fn with_analysis(mut self, analysis: impl Into<String>) -> Self {
        self.analysis = analysis.into();
        self
    }

    /// Builder: set plan.
    pub fn with_plan(mut self, plan: impl Into<String>) -> Self {
        self.plan = plan.into();
        self
    }

    /// Builder: set commands.
    pub fn with_commands(mut self, commands: Vec<Command>) -> Self {
        self.commands = commands;
        self
    }

    /// Builder: add a command.
    pub fn add_command(mut self, command: Command) -> Self {
        self.commands.push(command);
        self
    }

    /// Builder: set task_complete.
    pub fn complete(mut self) -> Self {
        self.task_complete = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_new() {
        let cmd = Command::new("ls -la\n");
        assert_eq!(cmd.keystrokes, "ls -la\n");
        assert_eq!(cmd.duration, 1.0);
    }

    #[test]
    fn test_command_with_duration() {
        let cmd = Command::with_duration("apt install -y foo\n", 30.0);
        assert_eq!(cmd.duration, 30.0);
    }

    #[test]
    fn test_response_builder() {
        let response = AgentResponse::new()
            .with_analysis("Terminal ready")
            .with_plan("Execute command")
            .add_command(Command::new("ls\n"));

        assert_eq!(response.analysis, "Terminal ready");
        assert_eq!(response.commands.len(), 1);
        assert!(!response.task_complete);
    }

    #[test]
    fn test_response_error() {
        let response = AgentResponse::error("Something failed");
        assert!(response.analysis.contains("Error"));
        assert!(response.commands.is_empty());
    }
}
