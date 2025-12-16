//! Protocol types for Term Challenge communication.

use serde::{Deserialize, Serialize};

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
}

impl Default for AgentResponse {
    fn default() -> Self {
        Self {
            analysis: String::new(),
            plan: String::new(),
            commands: Vec::new(),
            task_complete: false,
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
        Self {
            analysis: format!("Error: {}", message.into()),
            plan: "Cannot continue due to error".into(),
            commands: Vec::new(),
            task_complete: false,
        }
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
