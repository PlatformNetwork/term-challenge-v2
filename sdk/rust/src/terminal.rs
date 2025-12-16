//! Terminal interface for Term Challenge agents
//!
//! Provides tools to interact with the sandboxed terminal environment.

use crate::error::{Error, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Result of a terminal command execution
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Command output
    pub output: String,
    /// Exit code (if available)
    pub exit_code: Option<i32>,
    /// Execution duration in seconds
    pub duration_sec: f64,
    /// Whether the command timed out
    pub timed_out: bool,
}

impl CommandResult {
    /// Create a new command result
    pub fn new(output: impl Into<String>, duration_sec: f64) -> Self {
        Self {
            output: output.into(),
            exit_code: None,
            duration_sec,
            timed_out: false,
        }
    }

    /// Create a timeout result
    pub fn timeout(duration_sec: f64) -> Self {
        Self {
            output: String::new(),
            exit_code: None,
            duration_sec,
            timed_out: true,
        }
    }
}

/// Special keys for terminal interaction
pub mod special_keys {
    pub const ENTER: &str = "Enter";
    pub const ESCAPE: &str = "Escape";
    pub const TAB: &str = "Tab";
    pub const BACKSPACE: &str = "BSpace";
    pub const DELETE: &str = "DC";
    pub const UP: &str = "Up";
    pub const DOWN: &str = "Down";
    pub const LEFT: &str = "Left";
    pub const RIGHT: &str = "Right";
    pub const CTRL_C: &str = "C-c";
    pub const CTRL_D: &str = "C-d";
    pub const CTRL_Z: &str = "C-z";
    pub const CTRL_L: &str = "C-l";
}

/// Terminal interface trait
///
/// Implement this trait to provide a terminal backend.
/// The SDK provides a simulation backend for testing.
#[async_trait::async_trait]
pub trait TerminalBackend: Send + Sync {
    /// Send keystrokes to the terminal
    async fn send_keys(&self, keys: &[&str], block: bool, timeout_sec: f64) -> Result<()>;

    /// Capture the current terminal screen
    async fn capture_pane(&self, full_history: bool) -> Result<String>;

    /// Get incremental output since last call
    async fn get_incremental_output(&self) -> Result<String>;

    /// Check if terminal is available
    fn is_available(&self) -> bool;
}

/// Simulated terminal backend for testing
pub struct SimulatedTerminal {
    output_buffer: std::sync::Mutex<String>,
}

impl SimulatedTerminal {
    pub fn new() -> Self {
        Self {
            output_buffer: std::sync::Mutex::new(String::new()),
        }
    }
}

impl Default for SimulatedTerminal {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TerminalBackend for SimulatedTerminal {
    async fn send_keys(&self, keys: &[&str], _block: bool, _timeout_sec: f64) -> Result<()> {
        let mut buffer = self.output_buffer.lock().unwrap();
        for key in keys {
            buffer.push_str(&format!("[Sent: {}] ", key));
        }
        Ok(())
    }

    async fn capture_pane(&self, _full_history: bool) -> Result<String> {
        Ok("[Simulated terminal screen]".to_string())
    }

    async fn get_incremental_output(&self) -> Result<String> {
        let buffer = self.output_buffer.lock().unwrap();
        Ok(buffer.clone())
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Terminal interface for interacting with the sandbox
pub struct Terminal {
    backend: Box<dyn TerminalBackend>,
    default_timeout: f64,
}

impl Terminal {
    /// Create a new terminal with a backend
    pub fn new(backend: impl TerminalBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            default_timeout: 60.0,
        }
    }

    /// Create a simulated terminal for testing
    pub fn simulated() -> Self {
        Self::new(SimulatedTerminal::new())
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: f64) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Run a command in the terminal
    ///
    /// # Arguments
    /// * `command` - Command to execute
    /// * `block` - Whether to wait for completion
    /// * `timeout_sec` - Maximum wait time
    pub async fn run(
        &self,
        command: &str,
        block: bool,
        timeout_sec: Option<f64>,
    ) -> Result<CommandResult> {
        let timeout = timeout_sec.unwrap_or(self.default_timeout);
        let start = std::time::Instant::now();

        // Append newline if needed
        let keys: Vec<&str> = if command.ends_with('\n') {
            vec![command.trim_end_matches('\n'), "Enter"]
        } else {
            vec![command, "Enter"]
        };

        self.backend.send_keys(&keys, block, timeout).await?;

        let duration = start.elapsed().as_secs_f64();
        let output = if block {
            self.backend.get_incremental_output().await?
        } else {
            String::new()
        };

        Ok(CommandResult::new(output, duration))
    }

    /// Send keystrokes to the terminal
    ///
    /// For interactive programs (vim, less, etc.)
    pub async fn send_keys(&self, keys: &[&str]) -> Result<()> {
        self.backend.send_keys(keys, false, 0.1).await
    }

    /// Capture the current terminal screen
    pub async fn capture_screen(&self, full_history: bool) -> Result<String> {
        self.backend.capture_pane(full_history).await
    }

    /// Get new output since last call
    pub async fn get_output(&self) -> Result<String> {
        self.backend.get_incremental_output().await
    }

    /// Wait for specified time
    pub async fn wait(&self, seconds: f64) {
        sleep(Duration::from_secs_f64(seconds)).await;
    }

    /// Clear the terminal
    pub async fn clear(&self) -> Result<()> {
        self.backend.send_keys(&["clear", "Enter"], false, 0.1).await
    }

    /// Check if terminal is available
    pub fn is_available(&self) -> bool {
        self.backend.is_available()
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::simulated()
    }
}

/// Global terminal instance
static TERMINAL: std::sync::OnceLock<std::sync::Mutex<Option<Terminal>>> =
    std::sync::OnceLock::new();

/// Get or create the global terminal instance
pub fn get_terminal() -> std::sync::MutexGuard<'static, Option<Terminal>> {
    TERMINAL
        .get_or_init(|| std::sync::Mutex::new(Some(Terminal::simulated())))
        .lock()
        .unwrap()
}

/// Set the global terminal instance
pub fn set_terminal(terminal: Terminal) {
    let mut guard = get_terminal();
    *guard = Some(terminal);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simulated_terminal() {
        let terminal = Terminal::simulated();

        let result = terminal.run("ls -la", true, None).await.unwrap();
        assert!(!result.timed_out);

        let screen = terminal.capture_screen(false).await.unwrap();
        assert!(screen.contains("Simulated"));
    }

    #[tokio::test]
    async fn test_send_keys() {
        let terminal = Terminal::simulated();

        terminal.send_keys(&["vim", "Enter"]).await.unwrap();
        terminal.send_keys(&["i", "hello"]).await.unwrap();
        terminal
            .send_keys(&[special_keys::ESCAPE, ":wq", "Enter"])
            .await
            .unwrap();
    }
}
