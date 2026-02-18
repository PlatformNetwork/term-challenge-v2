//! Container Host Functions for WASM Challenges (DISABLED)
//!
//! This module previously provided host functions for container execution.
//! It is now disabled as part of the migration to WASM-only architecture.
//! All container operations return `Disabled` status.
//!
//! # Host Functions
//!
//! - `container_run(req_ptr, req_len, resp_ptr, resp_len) -> i32` - Always returns Disabled

use crate::runtime::{HostFunctionRegistrar, RuntimeState, WasmRuntimeError};
use serde::{Deserialize, Serialize};
use tracing::warn;
use wasmtime::{Caller, Linker, Memory};

pub const HOST_CONTAINER_NAMESPACE: &str = "platform_container";
pub const HOST_CONTAINER_RUN: &str = "container_run";

/// Container host status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ContainerHostStatus {
    /// Operation succeeded
    Success = 0,
    /// Container execution is disabled in WASM-only mode
    Disabled = 1,
    /// Image not in allowed list
    ImageNotAllowed = -1,
    /// Execution timeout
    ExecutionTimeout = -2,
    /// Execution failed
    ExecutionFailed = -3,
    /// Resource limit exceeded
    ResourceLimitExceeded = -4,
    /// Internal error
    InternalError = -100,
}

impl ContainerHostStatus {
    /// Convert to i32
    pub fn to_i32(self) -> i32 {
        self as i32
    }

    /// Convert from i32
    pub fn from_i32(code: i32) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::Disabled,
            -1 => Self::ImageNotAllowed,
            -2 => Self::ExecutionTimeout,
            -3 => Self::ExecutionFailed,
            -4 => Self::ResourceLimitExceeded,
            _ => Self::InternalError,
        }
    }
}

/// Container execution error types
#[derive(Debug, Serialize, Deserialize)]
pub enum ContainerExecError {
    /// Container execution is disabled
    Disabled,
}

/// Container execution policy.
///
/// All container execution is disabled in WASM-only mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPolicy {
    /// Whether container execution is enabled (always false in WASM-only mode).
    pub enabled: bool,
}

impl Default for ContainerPolicy {
    fn default() -> Self {
        Self { enabled: false }
    }
}

/// Container execution request (stub for backward compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRunRequest {
    /// Container image to run.
    pub image: String,
    /// Command to execute.
    pub command: Vec<String>,
}

/// Container execution response (stub for backward compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRunResponse {
    /// Exit code from the container.
    pub exit_code: i32,
    /// Standard output.
    pub stdout: Vec<u8>,
    /// Standard error.
    pub stderr: Vec<u8>,
}

/// Container execution state tracking.
pub struct ContainerState {
    /// Policy governing container execution.
    pub policy: ContainerPolicy,
    /// Challenge identifier.
    pub challenge_id: String,
    /// Validator identifier.
    pub validator_id: String,
    /// Number of container executions attempted.
    pub executions: u32,
}

impl ContainerState {
    /// Create a new container state.
    pub fn new(policy: ContainerPolicy, challenge_id: String, validator_id: String) -> Self {
        Self {
            policy,
            challenge_id,
            validator_id,
            executions: 0,
        }
    }

    /// Reset execution counters.
    pub fn reset_counters(&mut self) {
        self.executions = 0;
    }
}

/// Container host functions - DISABLED
///
/// All operations return Disabled status. This is a stub implementation
/// for backward compatibility with WASM modules that may reference
/// container host functions.
pub struct ContainerHostFunctions;

impl ContainerHostFunctions {
    /// Create new disabled container host functions
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContainerHostFunctions {
    fn default() -> Self {
        Self::new()
    }
}

impl HostFunctionRegistrar for ContainerHostFunctions {
    fn register(&self, linker: &mut Linker<RuntimeState>) -> Result<(), WasmRuntimeError> {
        linker
            .func_wrap(
                HOST_CONTAINER_NAMESPACE,
                HOST_CONTAINER_RUN,
                |mut caller: Caller<RuntimeState>,
                 _req_ptr: i32,
                 _req_len: i32,
                 resp_ptr: i32,
                 resp_len: i32|
                 -> i32 {
                    handle_container_run_disabled(&mut caller, resp_ptr, resp_len)
                },
            )
            .map_err(|e| {
                WasmRuntimeError::HostFunction(format!(
                    "failed to register {}: {}",
                    HOST_CONTAINER_RUN, e
                ))
            })?;

        Ok(())
    }
}

/// Handle container run - always returns disabled
fn handle_container_run_disabled(
    caller: &mut Caller<RuntimeState>,
    resp_ptr: i32,
    resp_len: i32,
) -> i32 {
    let result: Result<(), ContainerExecError> = Err(ContainerExecError::Disabled);

    let response_bytes = match bincode::serialize(&result) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(error = %err, "failed to serialize container response");
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
    fn test_container_host_status_conversion() {
        assert_eq!(ContainerHostStatus::Success.to_i32(), 0);
        assert_eq!(ContainerHostStatus::Disabled.to_i32(), 1);
        assert_eq!(ContainerHostStatus::ImageNotAllowed.to_i32(), -1);
        assert_eq!(ContainerHostStatus::InternalError.to_i32(), -100);

        assert_eq!(
            ContainerHostStatus::from_i32(0),
            ContainerHostStatus::Success
        );
        assert_eq!(
            ContainerHostStatus::from_i32(1),
            ContainerHostStatus::Disabled
        );
        assert_eq!(
            ContainerHostStatus::from_i32(-999),
            ContainerHostStatus::InternalError
        );
    }
}
