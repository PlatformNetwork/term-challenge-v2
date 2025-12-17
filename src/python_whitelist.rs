//! Python Module Whitelist Verification
//!
//! Verifies that submitted Python code only uses allowed modules.
//! This prevents malicious code execution and ensures fair evaluation.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WhitelistError {
    #[error("Forbidden module: {0}")]
    ForbiddenModule(String),
    #[error("Forbidden import pattern: {0}")]
    ForbiddenPattern(String),
    #[error("Syntax error in code: {0}")]
    SyntaxError(String),
    #[error("Code too large: {size} bytes (max: {max})")]
    CodeTooLarge { size: usize, max: usize },
    #[error("Forbidden builtin: {0}")]
    ForbiddenBuiltin(String),
}

/// Configuration for the Python whitelist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistConfig {
    /// Allowed standard library modules
    pub allowed_stdlib: HashSet<String>,
    /// Allowed third-party modules
    pub allowed_third_party: HashSet<String>,
    /// Forbidden builtins (e.g., exec, eval, compile)
    pub forbidden_builtins: HashSet<String>,
    /// Maximum code size in bytes
    pub max_code_size: usize,
    /// Allow subprocess/os.system calls
    pub allow_subprocess: bool,
    /// Allow network access
    pub allow_network: bool,
    /// Allow file system access
    pub allow_filesystem: bool,
}

impl Default for WhitelistConfig {
    fn default() -> Self {
        let mut allowed_stdlib = HashSet::new();
        // Safe standard library modules
        for module in &[
            "json",
            "re",
            "math",
            "random",
            "collections",
            "itertools",
            "functools",
            "operator",
            "string",
            "textwrap",
            "unicodedata",
            "datetime",
            "time",
            "calendar",
            "copy",
            "pprint",
            "typing",
            "dataclasses",
            "enum",
            "abc",
            "contextlib",
            "warnings",
            "bisect",
            "heapq",
            "array",
            "weakref",
            "types",
            "decimal",
            "fractions",
            "statistics",
            "hashlib",
            "hmac",
            "secrets",
            "base64",
            "binascii",
            "struct",
            "codecs",
            "io",
            "pathlib",
            "argparse",
            "logging",
            "traceback",
            "linecache",
            "difflib",
            "uuid",
            "html",
            "xml",
            "csv",
            "configparser",
            "tomllib",
            "subprocess",
            "os",
            "sys",
            "shutil",
            "glob", // Allowed for terminal bench
        ] {
            allowed_stdlib.insert(module.to_string());
        }

        let mut allowed_third_party = HashSet::new();
        // Safe third-party modules for AI agents
        for module in &[
            // Term SDK (official SDK)
            "term_sdk",
            "term-sdk",
            "termsdk",
            // AI/ML libraries
            "numpy",
            "pandas",
            "scipy",
            "sklearn",
            "torch",
            "tensorflow",
            "transformers",
            "openai",
            "anthropic",
            "httpx",
            "aiohttp",
            "requests",
            "pydantic",
            "attrs",
            "dataclasses_json",
            "rich",
            "click",
            "typer",
            "tqdm",
            "tabulate",
        ] {
            allowed_third_party.insert(module.to_string());
        }

        let mut forbidden_builtins = HashSet::new();
        for builtin in &[
            "exec",
            "eval",
            "compile",
            "__import__",
            "globals",
            "locals",
            "vars",
            "dir",
            "getattr",
            "setattr",
            "delattr",
            "hasattr",
            // "open",  // File access controlled separately - Allowed for terminal bench
        ] {
            forbidden_builtins.insert(builtin.to_string());
        }

        Self {
            allowed_stdlib,
            allowed_third_party,
            forbidden_builtins,
            max_code_size: 1024 * 1024, // 1MB
            allow_subprocess: true,     // Allowed for terminal bench
            allow_network: true,        // Agents need network for LLM calls
            allow_filesystem: true,     // Allowed for terminal bench
        }
    }
}

/// Result of module verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleVerification {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub imported_modules: Vec<String>,
    pub detected_patterns: Vec<String>,
}

impl ModuleVerification {
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            imported_modules: vec![],
            detected_patterns: vec![],
        }
    }

    pub fn invalid(error: impl Into<String>) -> Self {
        Self {
            valid: false,
            errors: vec![error.into()],
            warnings: vec![],
            imported_modules: vec![],
            detected_patterns: vec![],
        }
    }
}

/// Python module whitelist verifier
pub struct PythonWhitelist {
    config: WhitelistConfig,
    import_regex: Regex,
    from_import_regex: Regex,
    dangerous_patterns: Vec<(Regex, String)>,
}

impl PythonWhitelist {
    pub fn new(config: WhitelistConfig) -> Self {
        // Match "import x, y, z" but stop at "as" keyword
        let import_regex = Regex::new(r"^\s*import\s+([\w\.,\s]+?)(?:\s+as\s+|\s*$)").unwrap();
        let from_import_regex = Regex::new(r"^\s*from\s+([\w\.]+)\s+import").unwrap();

        let dangerous_patterns = vec![
            // Subprocess patterns
            (
                Regex::new(r"subprocess\.(run|call|Popen|check_output|check_call)").unwrap(),
                "subprocess execution".to_string(),
            ),
            (
                Regex::new(r"os\.(system|popen|exec|spawn)").unwrap(),
                "os command execution".to_string(),
            ),
            // Code execution patterns
            (
                Regex::new(r"\bexec\s*\(").unwrap(),
                "exec() call".to_string(),
            ),
            (
                Regex::new(r"\beval\s*\(").unwrap(),
                "eval() call".to_string(),
            ),
            (
                Regex::new(r"\bcompile\s*\(").unwrap(),
                "compile() call".to_string(),
            ),
            (
                Regex::new(r"__import__\s*\(").unwrap(),
                "__import__() call".to_string(),
            ),
            // Pickle (arbitrary code execution)
            (
                Regex::new(r"pickle\.(loads?|dump)").unwrap(),
                "pickle serialization (security risk)".to_string(),
            ),
            // ctypes (memory manipulation)
            (
                Regex::new(r"\bctypes\b").unwrap(),
                "ctypes module (memory access)".to_string(),
            ),
        ];

        Self {
            config,
            import_regex,
            from_import_regex,
            dangerous_patterns,
        }
    }

    /// Verify Python source code
    pub fn verify(&self, source_code: &str) -> ModuleVerification {
        let mut result = ModuleVerification::valid();

        // Check size
        if source_code.len() > self.config.max_code_size {
            return ModuleVerification::invalid(format!(
                "Code too large: {} bytes (max: {})",
                source_code.len(),
                self.config.max_code_size
            ));
        }

        // Extract and verify imports
        let mut imported_modules = HashSet::new();

        for line in source_code.lines() {
            // Check "import x, y, z" pattern
            if let Some(caps) = self.import_regex.captures(line) {
                let modules_str = caps.get(1).unwrap().as_str();
                for module in modules_str.split(',') {
                    let module = module.trim().split('.').next().unwrap_or("").trim();
                    if !module.is_empty() {
                        imported_modules.insert(module.to_string());
                    }
                }
            }

            // Check "from x import y" pattern
            if let Some(caps) = self.from_import_regex.captures(line) {
                let module = caps.get(1).unwrap().as_str();
                let root_module = module.split('.').next().unwrap_or(module);
                imported_modules.insert(root_module.to_string());
            }
        }

        // Verify each imported module
        for module in &imported_modules {
            if !self.is_module_allowed(module) {
                result.valid = false;
                result.errors.push(format!("Forbidden module: {}", module));
            }
        }

        result.imported_modules = imported_modules.into_iter().collect();

        // Check for dangerous patterns
        for (pattern, description) in &self.dangerous_patterns {
            if pattern.is_match(source_code) {
                if self.is_pattern_allowed(description) {
                    result.warnings.push(format!("Detected: {}", description));
                } else {
                    result.valid = false;
                    result
                        .errors
                        .push(format!("Forbidden pattern: {}", description));
                }
                result.detected_patterns.push(description.clone());
            }
        }

        // Check for forbidden builtins
        for builtin in &self.config.forbidden_builtins {
            let pattern = format!(r"\b{}\s*\(", regex::escape(builtin));
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(source_code) {
                    result.valid = false;
                    result
                        .errors
                        .push(format!("Forbidden builtin: {}()", builtin));
                }
            }
        }

        result
    }

    fn is_module_allowed(&self, module: &str) -> bool {
        self.config.allowed_stdlib.contains(module)
            || self.config.allowed_third_party.contains(module)
    }

    fn is_pattern_allowed(&self, description: &str) -> bool {
        if description.contains("subprocess") || description.contains("os command") {
            return self.config.allow_subprocess;
        }
        false
    }

    /// Get the whitelist configuration
    pub fn config(&self) -> &WhitelistConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_imports() {
        let whitelist = PythonWhitelist::new(WhitelistConfig::default());

        let code = r#"
import json
import math
from collections import defaultdict
from typing import List, Dict
import numpy as np
"#;

        let result = whitelist.verify(code);
        assert!(result.valid, "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_term_sdk_allowed() {
        let whitelist = PythonWhitelist::new(WhitelistConfig::default());

        // Test all variants of term_sdk
        let code1 = "import term_sdk\nfrom term_sdk import Agent";
        let code2 = "from term_sdk.agent import BaseAgent";
        let code3 = "import termsdk";

        let result1 = whitelist.verify(code1);
        assert!(
            result1.valid,
            "term_sdk should be allowed: {:?}",
            result1.errors
        );

        let result2 = whitelist.verify(code2);
        assert!(
            result2.valid,
            "term_sdk.agent should be allowed: {:?}",
            result2.errors
        );

        let result3 = whitelist.verify(code3);
        assert!(
            result3.valid,
            "termsdk should be allowed: {:?}",
            result3.errors
        );
    }

    #[test]
    fn test_forbidden_module() {
        // Create a restrictive config that disallows subprocess
        let mut config = WhitelistConfig {
            allow_subprocess: false,
            ..Default::default()
        };
        config.allowed_stdlib.remove("subprocess");
        config.allowed_stdlib.remove("os");
        config.allowed_stdlib.remove("sys");

        let whitelist = PythonWhitelist::new(config);

        let code = "import subprocess\nsubprocess.run(['ls'])";

        let result = whitelist.verify(code);
        assert!(
            !result.valid,
            "Expected forbidden module to fail: {:?}",
            result
        );
        assert!(result.errors.iter().any(|e| e.contains("subprocess")));
    }

    #[test]
    fn test_forbidden_builtin() {
        let whitelist = PythonWhitelist::new(WhitelistConfig::default());

        let code = "exec('print(1)')";

        let result = whitelist.verify(code);
        assert!(!result.valid);
    }
}
