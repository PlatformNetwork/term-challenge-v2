use crate::runtime::{HostFunctionRegistrar, RuntimeState, WasmRuntimeError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use wasmtime::{Caller, Linker, Memory};

pub const HOST_EXEC_NAMESPACE: &str = "platform_exec";
pub const HOST_EXEC_COMMAND: &str = "exec_command";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecHostFunction {
    ExecCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecPolicy {
    pub enabled: bool,
    pub allowed_commands: Vec<String>,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    pub max_executions: u32,
    pub allowed_env_vars: Vec<String>,
    pub blocked_args: Vec<String>,
}

impl Default for ExecPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_commands: Vec::new(),
            timeout_ms: 5_000,
            max_output_bytes: 512 * 1024,
            max_executions: 8,
            allowed_env_vars: Vec::new(),
            blocked_args: vec![
                "..".to_string(),
                "/etc".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
            ],
        }
    }
}

impl ExecPolicy {
    pub fn development() -> Self {
        Self {
            enabled: true,
            allowed_commands: vec![
                "echo".to_string(),
                "cat".to_string(),
                "ls".to_string(),
                "wc".to_string(),
                "grep".to_string(),
                "head".to_string(),
                "tail".to_string(),
            ],
            timeout_ms: 15_000,
            max_output_bytes: 2 * 1024 * 1024,
            max_executions: 32,
            allowed_env_vars: Vec::new(),
            blocked_args: vec![
                "..".to_string(),
                "/etc/shadow".to_string(),
                "/etc/passwd".to_string(),
            ],
        }
    }

    pub fn is_command_allowed(&self, command: &str) -> bool {
        if !self.enabled {
            return false;
        }
        self.allowed_commands.iter().any(|c| c == command)
    }

    pub fn are_args_allowed(&self, args: &[String]) -> bool {
        for arg in args {
            for blocked in &self.blocked_args {
                if arg.contains(blocked.as_str()) {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub stdin: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ExecError {
    #[error("exec disabled")]
    Disabled,
    #[error("command not allowed: {0}")]
    CommandNotAllowed(String),
    #[error("args not allowed: {0}")]
    ArgsNotAllowed(String),
    #[error("env var not allowed: {0}")]
    EnvVarNotAllowed(String),
    #[error("execution limit exceeded")]
    LimitExceeded,
    #[error("execution timeout")]
    Timeout,
    #[error("output too large: {0}")]
    OutputTooLarge(u64),
    #[error("execution failed: {0}")]
    Failed(String),
}

pub struct ExecState {
    policy: ExecPolicy,
    executions: u32,
    challenge_id: String,
    validator_id: String,
}

impl ExecState {
    pub fn new(policy: ExecPolicy, challenge_id: String, validator_id: String) -> Self {
        Self {
            policy,
            executions: 0,
            challenge_id,
            validator_id,
        }
    }

    pub fn executions(&self) -> u32 {
        self.executions
    }

    pub fn reset_counters(&mut self) {
        self.executions = 0;
    }

    pub fn handle_exec(&mut self, request: ExecRequest) -> Result<ExecResponse, ExecError> {
        if !self.policy.enabled {
            return Err(ExecError::Disabled);
        }

        if !self.policy.is_command_allowed(&request.command) {
            warn!(
                challenge_id = %self.challenge_id,
                validator_id = %self.validator_id,
                command = %request.command,
                "exec command not allowed"
            );
            return Err(ExecError::CommandNotAllowed(request.command));
        }

        if !self.policy.are_args_allowed(&request.args) {
            warn!(
                challenge_id = %self.challenge_id,
                validator_id = %self.validator_id,
                command = %request.command,
                "exec args not allowed"
            );
            return Err(ExecError::ArgsNotAllowed(request.args.join(" ")));
        }

        for key in request.env.keys() {
            if !self.policy.allowed_env_vars.is_empty()
                && !self.policy.allowed_env_vars.contains(key)
            {
                return Err(ExecError::EnvVarNotAllowed(key.clone()));
            }
        }

        if self.executions >= self.policy.max_executions {
            return Err(ExecError::LimitExceeded);
        }

        self.executions = self.executions.saturating_add(1);

        let start = Instant::now();
        let timeout = Duration::from_millis(self.policy.timeout_ms);

        let mut cmd = Command::new(&request.command);
        cmd.args(&request.args);
        cmd.env_clear();
        for (key, value) in &request.env {
            cmd.env(key, value);
        }

        if !request.stdin.is_empty() {
            cmd.stdin(std::process::Stdio::piped());
        } else {
            cmd.stdin(std::process::Stdio::null());
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| ExecError::Failed(e.to_string()))?;

        if !request.stdin.is_empty() {
            if let Some(ref mut stdin) = child.stdin {
                use std::io::Write;
                let _ = stdin.write_all(&request.stdin);
            }
            child.stdin.take();
        }

        let output = loop {
            if start.elapsed() > timeout {
                let _ = child.kill();
                return Err(ExecError::Timeout);
            }
            match child.try_wait() {
                Ok(Some(_)) => {
                    break child
                        .wait_with_output()
                        .map_err(|e| ExecError::Failed(e.to_string()))?
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => return Err(ExecError::Failed(e.to_string())),
            }
        };

        let stdout_len = output.stdout.len() as u64;
        let stderr_len = output.stderr.len() as u64;
        let total = stdout_len.saturating_add(stderr_len);
        if total > self.policy.max_output_bytes {
            return Err(ExecError::OutputTooLarge(total));
        }

        info!(
            challenge_id = %self.challenge_id,
            validator_id = %self.validator_id,
            command = %request.command,
            exit_code = output.status.code().unwrap_or(-1),
            stdout_bytes = stdout_len,
            stderr_bytes = stderr_len,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "exec command completed"
        );

        Ok(ExecResponse {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ExecHostFunctions {
    enabled: Vec<ExecHostFunction>,
}

impl ExecHostFunctions {
    pub fn new(enabled: Vec<ExecHostFunction>) -> Self {
        Self { enabled }
    }

    pub fn all() -> Self {
        Self {
            enabled: vec![ExecHostFunction::ExecCommand],
        }
    }
}

impl Default for ExecHostFunctions {
    fn default() -> Self {
        Self::all()
    }
}

impl HostFunctionRegistrar for ExecHostFunctions {
    fn register(&self, linker: &mut Linker<RuntimeState>) -> Result<(), WasmRuntimeError> {
        if self.enabled.contains(&ExecHostFunction::ExecCommand) {
            linker
                .func_wrap(
                    HOST_EXEC_NAMESPACE,
                    HOST_EXEC_COMMAND,
                    |mut caller: Caller<RuntimeState>,
                     req_ptr: i32,
                     req_len: i32,
                     resp_ptr: i32,
                     resp_len: i32|
                     -> i32 {
                        handle_exec_command(&mut caller, req_ptr, req_len, resp_ptr, resp_len)
                    },
                )
                .map_err(|err| WasmRuntimeError::HostFunction(err.to_string()))?;
        }

        Ok(())
    }
}

fn handle_exec_command(
    caller: &mut Caller<RuntimeState>,
    req_ptr: i32,
    req_len: i32,
    resp_ptr: i32,
    resp_len: i32,
) -> i32 {
    let request_bytes = match read_memory(caller, req_ptr, req_len) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(
                challenge_id = %caller.data().challenge_id,
                validator_id = %caller.data().validator_id,
                error = %err,
                "exec host memory read failed"
            );
            return write_result::<ExecResponse>(
                caller,
                resp_ptr,
                resp_len,
                Err(ExecError::Failed(err)),
            );
        }
    };

    let request = match bincode::deserialize::<ExecRequest>(&request_bytes) {
        Ok(req) => req,
        Err(err) => {
            warn!(
                challenge_id = %caller.data().challenge_id,
                validator_id = %caller.data().validator_id,
                error = %err,
                "exec request decode failed"
            );
            return write_result::<ExecResponse>(
                caller,
                resp_ptr,
                resp_len,
                Err(ExecError::Failed(format!("invalid exec request: {err}"))),
            );
        }
    };

    let result = caller.data_mut().exec_state.handle_exec(request);
    if let Err(ref err) = result {
        warn!(
            challenge_id = %caller.data().challenge_id,
            validator_id = %caller.data().validator_id,
            error = %err,
            "exec command denied"
        );
    }
    write_result(caller, resp_ptr, resp_len, result)
}

fn read_memory(caller: &mut Caller<RuntimeState>, ptr: i32, len: i32) -> Result<Vec<u8>, String> {
    if ptr < 0 || len < 0 {
        return Err("negative pointer/length".to_string());
    }
    let ptr = ptr as usize;
    let len = len as usize;
    let memory = get_memory(caller).ok_or_else(|| "memory export not found".to_string())?;
    let data = memory.data(caller);
    let end = ptr
        .checked_add(len)
        .ok_or_else(|| "pointer overflow".to_string())?;
    if end > data.len() {
        return Err("memory read out of bounds".to_string());
    }
    Ok(data[ptr..end].to_vec())
}

fn write_result<T: serde::Serialize>(
    caller: &mut Caller<RuntimeState>,
    resp_ptr: i32,
    resp_len: i32,
    result: Result<T, ExecError>,
) -> i32 {
    let response_bytes = match bincode::serialize(&result) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(error = %err, "failed to serialize exec response");
            return -1;
        }
    };

    write_bytes(caller, resp_ptr, resp_len, &response_bytes)
}

fn write_bytes(
    caller: &mut Caller<RuntimeState>,
    resp_ptr: i32,
    resp_len: i32,
    bytes: &[u8],
) -> i32 {
    if resp_ptr < 0 || resp_len < 0 {
        return -1;
    }
    if bytes.len() > i32::MAX as usize {
        return -1;
    }
    let resp_len = resp_len as usize;
    if bytes.len() > resp_len {
        return -(bytes.len() as i32);
    }

    let memory = match get_memory(caller) {
        Some(memory) => memory,
        None => return -1,
    };

    let ptr = resp_ptr as usize;
    let end = match ptr.checked_add(bytes.len()) {
        Some(end) => end,
        None => return -1,
    };
    let data = memory.data_mut(caller);
    if end > data.len() {
        return -1;
    }
    data[ptr..end].copy_from_slice(bytes);
    bytes.len() as i32
}

fn get_memory(caller: &mut Caller<RuntimeState>) -> Option<Memory> {
    let memory_export = caller.data().memory_export.clone();
    caller
        .get_export(&memory_export)
        .and_then(|export| export.into_memory())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_policy_default_disabled() {
        let policy = ExecPolicy::default();
        assert!(!policy.enabled);
        assert!(policy.allowed_commands.is_empty());
    }

    #[test]
    fn test_exec_policy_development() {
        let policy = ExecPolicy::development();
        assert!(policy.enabled);
        assert!(policy.is_command_allowed("echo"));
        assert!(policy.is_command_allowed("cat"));
        assert!(!policy.is_command_allowed("rm"));
    }

    #[test]
    fn test_exec_policy_command_allowlist() {
        let policy = ExecPolicy {
            enabled: true,
            allowed_commands: vec!["echo".to_string(), "ls".to_string()],
            ..Default::default()
        };

        assert!(policy.is_command_allowed("echo"));
        assert!(policy.is_command_allowed("ls"));
        assert!(!policy.is_command_allowed("rm"));
        assert!(!policy.is_command_allowed("cat"));
    }

    #[test]
    fn test_exec_policy_disabled_blocks_all() {
        let policy = ExecPolicy {
            enabled: false,
            allowed_commands: vec!["echo".to_string()],
            ..Default::default()
        };

        assert!(!policy.is_command_allowed("echo"));
    }

    #[test]
    fn test_exec_policy_blocked_args() {
        let policy = ExecPolicy::default();

        assert!(!policy.are_args_allowed(&["../../../etc/passwd".to_string()]));
        assert!(!policy.are_args_allowed(&["/etc/shadow".to_string()]));
        assert!(!policy.are_args_allowed(&["/proc/self/maps".to_string()]));
        assert!(policy.are_args_allowed(&["hello".to_string(), "world".to_string()]));
    }

    #[test]
    fn test_exec_state_creation() {
        let state = ExecState::new(
            ExecPolicy::development(),
            "test-challenge".into(),
            "test-validator".into(),
        );
        assert_eq!(state.executions(), 0);
    }

    #[test]
    fn test_exec_state_disabled() {
        let mut state = ExecState::new(ExecPolicy::default(), "test".into(), "test".into());

        let req = ExecRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: HashMap::new(),
            stdin: Vec::new(),
        };

        let err = state.handle_exec(req).unwrap_err();
        assert!(matches!(err, ExecError::Disabled));
    }

    #[test]
    fn test_exec_state_command_not_allowed() {
        let mut state = ExecState::new(ExecPolicy::development(), "test".into(), "test".into());

        let req = ExecRequest {
            command: "rm".to_string(),
            args: vec!["-rf".to_string(), "/".to_string()],
            env: HashMap::new(),
            stdin: Vec::new(),
        };

        let err = state.handle_exec(req).unwrap_err();
        assert!(matches!(err, ExecError::CommandNotAllowed(_)));
    }

    #[test]
    fn test_exec_state_limit_exceeded() {
        let mut state = ExecState::new(
            ExecPolicy {
                enabled: true,
                allowed_commands: vec!["echo".to_string()],
                max_executions: 1,
                ..Default::default()
            },
            "test".into(),
            "test".into(),
        );

        let req = ExecRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: HashMap::new(),
            stdin: Vec::new(),
        };

        let result = state.handle_exec(req.clone());
        assert!(result.is_ok());

        let err = state.handle_exec(req).unwrap_err();
        assert!(matches!(err, ExecError::LimitExceeded));
    }

    #[test]
    fn test_exec_state_reset_counters() {
        let mut state = ExecState::new(ExecPolicy::development(), "test".into(), "test".into());

        state.executions = 5;
        state.reset_counters();
        assert_eq!(state.executions(), 0);
    }

    #[test]
    fn test_exec_state_env_var_not_allowed() {
        let mut state = ExecState::new(
            ExecPolicy {
                enabled: true,
                allowed_commands: vec!["echo".to_string()],
                allowed_env_vars: vec!["PATH".to_string()],
                ..Default::default()
            },
            "test".into(),
            "test".into(),
        );

        let mut env = HashMap::new();
        env.insert("SECRET".to_string(), "value".to_string());

        let req = ExecRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env,
            stdin: Vec::new(),
        };

        let err = state.handle_exec(req).unwrap_err();
        assert!(matches!(err, ExecError::EnvVarNotAllowed(_)));
    }

    #[test]
    fn test_exec_echo_command() {
        let mut state = ExecState::new(
            ExecPolicy {
                enabled: true,
                allowed_commands: vec!["echo".to_string()],
                ..Default::default()
            },
            "test".into(),
            "test".into(),
        );

        let req = ExecRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: HashMap::new(),
            stdin: Vec::new(),
        };

        let resp = state.handle_exec(req).unwrap();
        assert_eq!(resp.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&resp.stdout).trim(), "hello");
        assert_eq!(state.executions(), 1);
    }
}
