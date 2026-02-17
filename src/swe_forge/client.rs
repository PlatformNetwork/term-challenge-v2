use super::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use sp_core::{sr25519, Pair};
use std::time::Duration;
use tracing::debug;

/// Client for communicating with term-executor workers
pub struct SweForgeClient {
    client: Client,
    api_key: String,
    keypair: sr25519::Pair,
    hotkey: String,
}

impl SweForgeClient {
    pub fn new(api_key: String, keypair: sr25519::Pair) -> Result<Self> {
        use sp_core::crypto::Ss58Codec;
        let hotkey = keypair.public().to_ss58check();
        let client = Client::builder()
            .timeout(Duration::from_secs(3600))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        Ok(Self {
            client,
            api_key,
            keypair,
            hotkey,
        })
    }

    fn sign_request(&self, nonce: &str) -> String {
        let message = format!("{}{}", self.hotkey, nonce);
        let signature = self.keypair.sign(message.as_bytes());
        format!("0x{}", hex::encode(signature.0))
    }

    /// Check health of a term-executor instance
    pub async fn check_health(&self, base_url: &str) -> Result<HealthResponse> {
        let url = format!("{}/health", base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("Health check failed")?;
        resp.json().await.context("Failed to parse health response")
    }

    /// Submit a batch of tasks to a term-executor instance
    pub async fn submit_batch(
        &self,
        base_url: &str,
        archive_data: Vec<u8>,
    ) -> Result<SubmitResponse> {
        let url = format!("{}/submit", base_url);
        let nonce = uuid::Uuid::new_v4().to_string();
        let signature = self.sign_request(&nonce);

        let part = reqwest::multipart::Part::bytes(archive_data)
            .file_name("archive.tar.gz")
            .mime_str("application/gzip")
            .context("Failed to create multipart part")?;
        let form = reqwest::multipart::Form::new().part("archive", part);

        let resp = self
            .client
            .post(&url)
            .header("X-Hotkey", &self.hotkey)
            .header("X-Nonce", &nonce)
            .header("X-Signature", &signature)
            .header("X-Api-Key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .context("Batch submission failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Batch submission failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse submit response")
    }

    /// Get batch status
    pub async fn get_batch(&self, base_url: &str, batch_id: &str) -> Result<BatchResult> {
        let url = format!("{}/batch/{}", base_url, batch_id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Get batch failed")?;
        resp.json().await.context("Failed to parse batch response")
    }

    /// Poll until batch completes or times out
    pub async fn poll_batch_completion(
        &self,
        base_url: &str,
        batch_id: &str,
        poll_interval: Duration,
        max_duration: Duration,
    ) -> Result<BatchResult> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > max_duration {
                anyhow::bail!("Batch {} timed out after {:?}", batch_id, max_duration);
            }
            let result = self.get_batch(base_url, batch_id).await?;
            match result.status {
                BatchStatus::Completed | BatchStatus::Failed => return Ok(result),
                _ => {
                    debug!(batch_id = batch_id, status = ?result.status, "Batch in progress");
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> sr25519::Pair {
        sr25519::Pair::from_string("//Alice", None).expect("valid dev keypair")
    }

    #[test]
    fn test_client_creation() {
        let client = SweForgeClient::new("test-key".to_string(), test_keypair());
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.api_key, "test-key");
        assert!(!client.hotkey.is_empty());
    }

    #[test]
    fn test_sign_request_deterministic() {
        let client = SweForgeClient::new("key".to_string(), test_keypair()).unwrap();
        let sig1 = client.sign_request("nonce-1");
        let sig2 = client.sign_request("nonce-1");
        assert!(sig1.starts_with("0x"));
        assert_eq!(sig1.len(), sig2.len());
    }

    #[test]
    fn test_sign_request_different_nonces() {
        let client = SweForgeClient::new("key".to_string(), test_keypair()).unwrap();
        let sig1 = client.sign_request("nonce-1");
        let sig2 = client.sign_request("nonce-2");
        assert_ne!(sig1, sig2);
    }
}
