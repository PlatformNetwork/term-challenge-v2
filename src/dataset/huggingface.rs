use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context};
use serde::Deserialize;

use super::types::DatasetEntry;

const HF_API_BASE: &str = "https://huggingface.co/api/datasets";
const HF_RESOLVE_BASE: &str = "https://huggingface.co/datasets";
const ROWS_API_BASE: &str = "https://datasets-server.huggingface.co/rows";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct HuggingFaceTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct RowsResponse {
    rows: Vec<RowWrapper>,
}

#[derive(Debug, Deserialize)]
struct RowWrapper {
    row: DatasetEntry,
}

pub struct HuggingFaceDataset {
    repo_id: String,
    cache_dir: PathBuf,
    client: reqwest::Client,
}

impl HuggingFaceDataset {
    pub fn new(repo_id: &str, cache_dir: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default();

        Self {
            repo_id: repo_id.to_string(),
            cache_dir,
            client,
        }
    }

    pub async fn list_files(&self) -> anyhow::Result<Vec<String>> {
        let mut files = Vec::new();
        let mut dirs_to_visit = vec![String::new()];

        while let Some(dir) = dirs_to_visit.pop() {
            let url = if dir.is_empty() {
                format!("{}/{}/tree/main", HF_API_BASE, self.repo_id)
            } else {
                format!("{}/{}/tree/main/{}", HF_API_BASE, self.repo_id, dir)
            };

            let response = self
                .client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("failed to list files at '{dir}'"))?;

            if !response.status().is_success() {
                return Err(anyhow!(
                    "HuggingFace API returned {} for path '{}'",
                    response.status(),
                    dir
                ));
            }

            let entries: Vec<HuggingFaceTreeEntry> = response
                .json()
                .await
                .with_context(|| format!("failed to parse tree response for '{dir}'"))?;

            for entry in entries {
                match entry.entry_type.as_str() {
                    "file" => files.push(entry.path),
                    "directory" => dirs_to_visit.push(entry.path),
                    _ => {}
                }
            }
        }

        files.sort();
        Ok(files)
    }

    pub async fn download_file(&self, filename: &str) -> anyhow::Result<PathBuf> {
        let dest = self.cache_dir.join(filename);

        if dest.exists() {
            tracing::debug!(path = %dest.display(), "using cached file");
            return Ok(dest);
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create cache directory '{}'", parent.display()))?;
        }

        let url = format!(
            "{}/{}/resolve/main/{}",
            HF_RESOLVE_BASE, self.repo_id, filename
        );

        tracing::debug!(url = %url, dest = %dest.display(), "downloading file");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to download '{filename}'"))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "HuggingFace returned {} for file '{}'",
                response.status(),
                filename
            ));
        }

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed to read response body for '{filename}'"))?;

        let tmp_path = dest.with_extension("tmp");
        tokio::fs::write(&tmp_path, &bytes)
            .await
            .with_context(|| format!("failed to write '{}'", tmp_path.display()))?;

        tokio::fs::rename(&tmp_path, &dest)
            .await
            .with_context(|| format!("failed to rename temp file to '{}'", dest.display()))?;

        tracing::debug!(
            path = %dest.display(),
            size_bytes = bytes.len(),
            "file downloaded"
        );

        Ok(dest)
    }

    pub async fn download_dataset(&self) -> anyhow::Result<Vec<DatasetEntry>> {
        let entries = self.fetch_rows("default", "validation").await;

        if let Ok(rows) = entries {
            if !rows.is_empty() {
                return Ok(rows);
            }
        }

        let files = self.list_files().await?;
        let json_files: Vec<&str> = files
            .iter()
            .map(|f| f.as_str())
            .filter(|f| f.ends_with(".json") || f.ends_with(".jsonl"))
            .collect();

        if json_files.is_empty() {
            for file in &files {
                self.download_file(file).await?;
            }
            return Ok(Vec::new());
        }

        let mut all_entries = Vec::new();
        for file in json_files {
            let path = self.download_file(file).await?;
            let mut parsed = load_json_entries(&path).await?;
            all_entries.append(&mut parsed);
        }

        Ok(all_entries)
    }

    async fn fetch_rows(
        &self,
        config: &str,
        split: &str,
    ) -> anyhow::Result<Vec<DatasetEntry>> {
        let url = format!(
            "{}?dataset={}&config={}&split={}&length=100",
            ROWS_API_BASE, self.repo_id, config, split
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to fetch rows from datasets server")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "datasets server returned {}",
                response.status()
            ));
        }

        let body: RowsResponse = response
            .json()
            .await
            .context("failed to parse rows response")?;

        Ok(body.rows.into_iter().map(|w| w.row).collect())
    }
}

async fn load_json_entries(path: &Path) -> anyhow::Result<Vec<DatasetEntry>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read '{}'", path.display()))?;

    if let Ok(entries) = serde_json::from_str::<Vec<DatasetEntry>>(&content) {
        return Ok(entries);
    }

    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<DatasetEntry>(trimmed) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "skipping malformed line"
                );
            }
        }
    }

    Ok(entries)
}
