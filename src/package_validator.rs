//! Package Validator - Validates multi-file agent packages
//!
//! Supports:
//! - ZIP archives
//! - TAR.GZ archives
//!
//! Validates:
//! - Total size limits
//! - Entry point exists and contains Agent class
//! - All Python files pass whitelist check
//! - No forbidden file types
//! - No path traversal attacks

use crate::python_whitelist::{PythonWhitelist, WhitelistConfig};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Cursor, Read};
use tar::Archive;
use tracing::{debug, info, warn};

/// Maximum package size (10MB)
pub const MAX_PACKAGE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum number of files in package
pub const MAX_FILES: usize = 100;

/// Maximum single file size (1MB)
pub const MAX_FILE_SIZE: usize = 1024 * 1024;

/// Allowed file extensions
pub const ALLOWED_EXTENSIONS: &[&str] = &[
    "py", "txt", "json", "yaml", "yml", "toml", "md", "csv", "xml",
];

/// Forbidden file extensions (binary/executable)
pub const FORBIDDEN_EXTENSIONS: &[&str] = &[
    "so", "dll", "dylib", "exe", "bin", "sh", "bash", "pyc", "pyo", "class", "jar",
];

/// A file extracted from a package
#[derive(Debug, Clone)]
pub struct PackageFile {
    pub path: String,
    pub size: usize,
    pub content: Vec<u8>,
    pub is_python: bool,
}

/// Result of package validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub file_paths: Vec<String>,
    pub total_size: usize,
    pub entry_point_found: bool,
    pub python_files_count: usize,
}

/// Configuration for package validation
#[derive(Debug, Clone)]
pub struct PackageValidatorConfig {
    pub max_package_size: usize,
    pub max_files: usize,
    pub max_file_size: usize,
    pub allowed_extensions: HashSet<String>,
    pub forbidden_extensions: HashSet<String>,
}

impl Default for PackageValidatorConfig {
    fn default() -> Self {
        Self {
            max_package_size: MAX_PACKAGE_SIZE,
            max_files: MAX_FILES,
            max_file_size: MAX_FILE_SIZE,
            allowed_extensions: ALLOWED_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            forbidden_extensions: FORBIDDEN_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Package validator for multi-file agent submissions
pub struct PackageValidator {
    config: PackageValidatorConfig,
    python_whitelist: PythonWhitelist,
}

impl PackageValidator {
    pub fn new() -> Self {
        Self::with_config(PackageValidatorConfig::default())
    }

    pub fn with_config(config: PackageValidatorConfig) -> Self {
        Self {
            config,
            python_whitelist: PythonWhitelist::new(WhitelistConfig::default()),
        }
    }

    /// Validate a package archive
    ///
    /// Returns validation result with errors/warnings and extracted file info
    pub fn validate(
        &self,
        data: &[u8],
        format: &str,
        entry_point: &str,
    ) -> Result<PackageValidation> {
        let mut validation = PackageValidation::default();

        // 1. Check total compressed size
        if data.len() > self.config.max_package_size {
            validation.errors.push(format!(
                "Package too large: {} bytes (max: {} bytes)",
                data.len(),
                self.config.max_package_size
            ));
            return Ok(validation);
        }

        // 2. Extract files based on format
        let files = match format.to_lowercase().as_str() {
            "zip" => self.extract_zip(data)?,
            "tar.gz" | "tgz" | "targz" => self.extract_tar_gz(data)?,
            _ => {
                validation.errors.push(format!(
                    "Unsupported format: {}. Use 'zip' or 'tar.gz'",
                    format
                ));
                return Ok(validation);
            }
        };

        // 3. Validate extracted files
        self.validate_files(&mut validation, files, entry_point)?;

        // Set valid flag based on errors
        validation.valid = validation.errors.is_empty();

        Ok(validation)
    }

    /// Validate a package and return the extracted files if valid
    pub fn validate_and_extract(
        &self,
        data: &[u8],
        format: &str,
        entry_point: &str,
    ) -> Result<(PackageValidation, Vec<PackageFile>)> {
        let mut validation = PackageValidation::default();

        // 1. Check total compressed size
        if data.len() > self.config.max_package_size {
            validation.errors.push(format!(
                "Package too large: {} bytes (max: {} bytes)",
                data.len(),
                self.config.max_package_size
            ));
            return Ok((validation, Vec::new()));
        }

        // 2. Extract files based on format
        let files = match format.to_lowercase().as_str() {
            "zip" => self.extract_zip(data)?,
            "tar.gz" | "tgz" | "targz" => self.extract_tar_gz(data)?,
            _ => {
                validation.errors.push(format!(
                    "Unsupported format: {}. Use 'zip' or 'tar.gz'",
                    format
                ));
                return Ok((validation, Vec::new()));
            }
        };

        // 3. Validate extracted files
        let files_clone = files.clone();
        self.validate_files(&mut validation, files, entry_point)?;

        // Set valid flag based on errors
        validation.valid = validation.errors.is_empty();

        if validation.valid {
            Ok((validation, files_clone))
        } else {
            Ok((validation, Vec::new()))
        }
    }

    /// Extract files from ZIP archive
    fn extract_zip(&self, data: &[u8]) -> Result<Vec<PackageFile>> {
        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

        let mut files = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).context("Failed to read ZIP entry")?;

            // Skip directories
            if file.is_dir() {
                continue;
            }

            // Get the raw name first to detect path traversal attempts
            let raw_name = file.name().to_string();

            // Check for path traversal in the raw name
            if raw_name.contains("..") || raw_name.starts_with('/') {
                // Return this as a file with a special marker path so validation catches it
                files.push(PackageFile {
                    path: raw_name,
                    size: 0,
                    content: Vec::new(),
                    is_python: false,
                });
                continue;
            }

            let path = file
                .enclosed_name()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip empty paths (after sanitization, if somehow still empty)
            if path.is_empty() {
                continue;
            }

            // Read content
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .context("Failed to read ZIP file content")?;

            let is_python = path.ends_with(".py");

            files.push(PackageFile {
                path,
                size: content.len(),
                content,
                is_python,
            });
        }

        Ok(files)
    }

    /// Extract files from TAR.GZ archive
    fn extract_tar_gz(&self, data: &[u8]) -> Result<Vec<PackageFile>> {
        let cursor = Cursor::new(data);
        let decoder = GzDecoder::new(cursor);
        let mut archive = Archive::new(decoder);

        let mut files = Vec::new();

        for entry in archive.entries().context("Failed to read TAR entries")? {
            let mut entry = entry.context("Failed to read TAR entry")?;

            // Skip directories
            if entry.header().entry_type().is_dir() {
                continue;
            }

            let path = entry
                .path()
                .context("Failed to get entry path")?
                .to_string_lossy()
                .to_string();

            // Skip empty paths
            if path.is_empty() {
                continue;
            }

            // Read content
            let mut content = Vec::new();
            entry
                .read_to_end(&mut content)
                .context("Failed to read TAR file content")?;

            let is_python = path.ends_with(".py");

            files.push(PackageFile {
                path,
                size: content.len(),
                content,
                is_python,
            });
        }

        Ok(files)
    }

    /// Validate extracted files
    fn validate_files(
        &self,
        validation: &mut PackageValidation,
        files: Vec<PackageFile>,
        entry_point: &str,
    ) -> Result<()> {
        // Check file count
        if files.len() > self.config.max_files {
            validation.errors.push(format!(
                "Too many files: {} (max: {})",
                files.len(),
                self.config.max_files
            ));
            return Ok(());
        }

        let mut total_size = 0;
        let mut python_count = 0;
        let mut entry_found = false;

        // Normalize entry point (remove leading ./)
        let entry_point_normalized = entry_point.trim_start_matches("./");

        for file in &files {
            // Check for path traversal
            if file.path.contains("..") {
                validation
                    .errors
                    .push(format!("Path traversal detected: {}", file.path));
                continue;
            }

            // Normalize path (remove leading ./)
            let normalized_path = file.path.trim_start_matches("./");

            // Check file size
            if file.size > self.config.max_file_size {
                validation.errors.push(format!(
                    "File too large: {} ({} bytes, max: {} bytes)",
                    file.path, file.size, self.config.max_file_size
                ));
                continue;
            }

            // Check extension
            let extension = std::path::Path::new(&file.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if self.config.forbidden_extensions.contains(&extension) {
                validation
                    .errors
                    .push(format!("Forbidden file type: {}", file.path));
                continue;
            }

            if !extension.is_empty() && !self.config.allowed_extensions.contains(&extension) {
                validation.warnings.push(format!(
                    "Unknown file type (will be ignored): {}",
                    file.path
                ));
            }

            // Track total size
            total_size += file.size;

            // Store file path
            validation.file_paths.push(file.path.clone());

            // Check if this is the entry point
            if normalized_path == entry_point_normalized {
                entry_found = true;
            }

            // Validate Python files with whitelist
            if file.is_python {
                python_count += 1;

                let source = String::from_utf8_lossy(&file.content);
                let whitelist_result = self.python_whitelist.verify(&source);

                if !whitelist_result.valid {
                    for error in whitelist_result.errors {
                        validation.errors.push(format!("{}: {}", file.path, error));
                    }
                }

                for warning in whitelist_result.warnings {
                    validation
                        .warnings
                        .push(format!("{}: {}", file.path, warning));
                }
            }
        }

        // Check entry point exists
        if !entry_found {
            validation.errors.push(format!(
                "Entry point not found: '{}'. Available files: {:?}",
                entry_point,
                validation.file_paths.iter().take(10).collect::<Vec<_>>()
            ));
        }

        // Check total uncompressed size
        if total_size > self.config.max_package_size * 2 {
            validation.errors.push(format!(
                "Total uncompressed size too large: {} bytes (max: {} bytes)",
                total_size,
                self.config.max_package_size * 2
            ));
        }

        validation.total_size = total_size;
        validation.python_files_count = python_count;
        validation.entry_point_found = entry_found;

        Ok(())
    }
}

impl Default for PackageValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            for (name, content) in files {
                zip.start_file(*name, options).unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn test_valid_package() {
        let validator = PackageValidator::new();

        let zip_data = create_test_zip(&[
            (
                "agent.py",
                "from term_sdk import Agent\nclass MyAgent(Agent):\n    pass",
            ),
            ("utils.py", "def helper(): pass"),
            ("config.json", "{}"),
        ]);

        let result = validator.validate(&zip_data, "zip", "agent.py").unwrap();
        assert!(result.valid, "Errors: {:?}", result.errors);
        assert!(result.entry_point_found);
        assert_eq!(result.python_files_count, 2);
    }

    #[test]
    fn test_missing_entry_point() {
        let validator = PackageValidator::new();

        let zip_data = create_test_zip(&[("utils.py", "def helper(): pass")]);

        let result = validator.validate(&zip_data, "zip", "agent.py").unwrap();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("Entry point not found")));
    }

    #[test]
    fn test_forbidden_extension() {
        let validator = PackageValidator::new();

        let zip_data = create_test_zip(&[
            ("agent.py", "from term_sdk import Agent"),
            ("malicious.so", "binary"),
        ]);

        let result = validator.validate(&zip_data, "zip", "agent.py").unwrap();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("Forbidden file type")));
    }

    #[test]
    fn test_path_traversal() {
        let validator = PackageValidator::new();

        let zip_data = create_test_zip(&[
            ("agent.py", "from term_sdk import Agent"),
            ("../etc/passwd", "root:x:0:0"),
        ]);

        let result = validator.validate(&zip_data, "zip", "agent.py").unwrap();
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("Path traversal")));
    }

    #[test]
    fn test_whitelist_violation() {
        let validator = PackageValidator::new();

        let zip_data = create_test_zip(&[("agent.py", "import term_sdk\nexec('malicious')")]);

        let result = validator.validate(&zip_data, "zip", "agent.py").unwrap();
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("exec")));
    }
}
