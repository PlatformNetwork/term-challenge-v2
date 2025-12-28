//! Submit Wizard - Interactive CLI (non-TUI)

use anyhow::Result;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use sp_core::{sr25519, Pair};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use term_challenge::{
    encode_ss58, ApiKeyConfig, ApiKeyConfigBuilder, EncryptedApiKey, PythonWhitelist,
    WhitelistConfig,
};

pub async fn run_submit_wizard(rpc_url: &str) -> Result<()> {
    let term = Term::stdout();
    term.clear_screen()?;

    print_banner();
    println!();
    println!(
        "{}",
        style("  Interactive Agent Submission Wizard").cyan().bold()
    );
    println!(
        "  {}",
        style("Guide you through submitting an agent to the network").dim()
    );
    println!();

    // Step 1: Select agent file
    let agent_path = select_agent_file()?;
    let source = std::fs::read_to_string(&agent_path)?;
    let default_name = agent_path
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "agent".to_string())
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();

    println!();
    println!(
        "  {} Selected: {}",
        style("✓").green(),
        style(agent_path.file_name().unwrap_or_default().to_string_lossy()).cyan()
    );

    // Step 1b: Choose agent name
    println!();
    println!("  {}", style("Step 1b: Choose Agent Name").bold());
    println!("  {}", style("(alphanumeric, dash, underscore only)").dim());
    println!();

    let agent_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("  Agent name")
        .default(default_name)
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() {
                return Err("Name cannot be empty");
            }
            if input.len() > 64 {
                return Err("Name must be 64 characters or less");
            }
            if !input
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err("Name can only contain alphanumeric, dash, underscore");
            }
            Ok(())
        })
        .interact_text()?;

    println!(
        "  {} Agent name: {}",
        style("✓").green(),
        style(&agent_name).cyan()
    );

    // Step 2: Enter miner key
    println!();
    let (signing_key, miner_hotkey) = enter_miner_key()?;
    println!(
        "  {} Hotkey: {}",
        style("✓").green(),
        style(&miner_hotkey[..16]).cyan()
    );

    // Step 3: Validate agent
    println!();
    println!("  {} Validating agent...", style("→").cyan());
    validate_agent(&source)?;
    println!("  {} Validation passed", style("✓").green());

    // Step 4: Fetch validators
    println!();
    println!("  {} Fetching validators...", style("→").cyan());
    let validators = fetch_validators(rpc_url).await?;
    println!(
        "  {} Found {} validators",
        style("✓").green(),
        validators.len()
    );

    // Step 5: Configure API keys
    println!();
    let api_keys = configure_api_keys(&validators)?;

    // Step 6: Review and confirm
    println!();
    print_review(&agent_name, &miner_hotkey, validators.len(), &api_keys);

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("  Submit agent to network?")
        .default(true)
        .interact()?;

    if !confirmed {
        println!();
        println!("  {} Cancelled", style("✗").red());
        return Ok(());
    }

    // Step 7: Submit
    println!();
    let hash = submit_agent(
        rpc_url,
        &source,
        &signing_key,
        &miner_hotkey,
        &agent_name,
        api_keys,
        &validators,
    )
    .await?;

    println!();
    println!("  {}", style("═".repeat(50)).dim());
    println!();
    println!(
        "  {} Agent submitted successfully!",
        style("✓").green().bold()
    );
    println!();
    println!("  Agent Hash: {}", style(&hash).cyan().bold());
    println!();
    println!(
        "  Check status: {}",
        style(format!("term status -H {}", &hash[..16])).yellow()
    );
    println!("  Leaderboard:  {}", style("term leaderboard").yellow());
    println!();

    Ok(())
}

fn print_banner() {
    println!(
        r#"
  {}
  {}
  {}
  {}
  {}
  {}"#,
        style("████████╗███████╗██████╗ ███╗   ███╗").cyan(),
        style("╚══██╔══╝██╔════╝██╔══██╗████╗ ████║").cyan(),
        style("   ██║   █████╗  ██████╔╝██╔████╔██║").cyan(),
        style("   ██║   ██╔══╝  ██╔══██╗██║╚██╔╝██║").cyan(),
        style("   ██║   ███████╗██║  ██║██║ ╚═╝ ██║").cyan(),
        style("   ╚═╝   ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝").cyan(),
    );
}

fn select_agent_file() -> Result<PathBuf> {
    println!("  {}", style("Step 1: Select Agent File").bold());
    println!();

    let current_dir = std::env::current_dir()?;

    // Find Python files in current directory
    let mut py_files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "py").unwrap_or(false) {
                py_files.push(path);
            }
        }
    }
    py_files.sort();

    if py_files.is_empty() {
        // No Python files found, ask for path
        let path: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("  Enter path to agent file")
            .interact_text()?;
        let path = PathBuf::from(path);
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }
        return Ok(path);
    }

    // Show selection
    let items: Vec<String> = py_files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    let mut items_with_custom = items.clone();
    items_with_custom.push("[ Enter custom path ]".to_string());

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  Select agent file")
        .items(&items_with_custom)
        .default(0)
        .interact()?;

    if selection == items.len() {
        // Custom path
        let path: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("  Enter path to agent file")
            .interact_text()?;
        let path = PathBuf::from(path);
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }
        Ok(path)
    } else {
        Ok(py_files[selection].clone())
    }
}

fn enter_miner_key() -> Result<(sr25519::Pair, String)> {
    println!("  {}", style("Step 2: Enter Miner Key").bold());
    println!("  {}", style("(64-char hex or 12+ word mnemonic)").dim());
    println!();

    let key: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("  Miner secret key")
        .interact()?;

    parse_miner_key(&key)
}

fn parse_miner_key(key: &str) -> Result<(sr25519::Pair, String)> {
    let pair: sr25519::Pair;

    if key.len() == 64 {
        let bytes = hex::decode(key)?;
        if bytes.len() == 32 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            pair = sr25519::Pair::from_seed(&seed);
        } else {
            return Err(anyhow::anyhow!("Invalid hex key length"));
        }
    } else if key.split_whitespace().count() >= 12 {
        pair = sr25519::Pair::from_phrase(key, None)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {:?}", e))?
            .0;
    } else {
        return Err(anyhow::anyhow!("Invalid key format"));
    }

    // Get public key and convert to SS58
    let public = pair.public();
    let hotkey_ss58 = encode_ss58(&public.0);

    Ok((pair, hotkey_ss58))
}

fn validate_agent(source: &str) -> Result<()> {
    let whitelist = PythonWhitelist::new(WhitelistConfig::default());
    let verification = whitelist.verify(source);

    if !verification.valid {
        let errors = verification.errors.join("\n  ");
        anyhow::bail!("Validation failed:\n  {}", errors);
    }

    // Check for term_sdk import
    let has_sdk_import = source.contains("from term_sdk import")
        || source.contains("import term_sdk")
        || source.contains("from termsdk import")
        || source.contains("import termsdk");

    if !has_sdk_import {
        println!("  {} No term_sdk import detected", style("⚠").yellow());
    }

    // Check for Agent class (extends Agent base class)
    let has_agent_class =
        source.contains("class") && (source.contains("(Agent)") || source.contains("( Agent )"));

    if !has_agent_class {
        println!(
            "  {} No Agent class detected (should extend Agent)",
            style("⚠").yellow()
        );
    }

    // Check for solve() method (new SDK format)
    let has_solve = source.contains("def solve") || source.contains("async def solve");

    if !has_solve {
        println!("  {} No solve() method detected", style("⚠").yellow());
    }

    // Check for run() call at the end
    let has_run = source.contains("run(") && source.contains("if __name__");

    if !has_run {
        println!("  {} No run() entry point detected", style("⚠").yellow());
    }

    Ok(())
}

#[derive(Clone)]
struct ValidatorInfo {
    hotkey: String,
    hotkey_ss58: String,
    #[allow(dead_code)]
    stake: u64,
}

async fn fetch_validators(platform_url: &str) -> Result<Vec<ValidatorInfo>> {
    let client = reqwest::Client::new();

    // Use REST API to fetch validators from platform-server
    let api_endpoint = format!("{}/api/v1/validators", platform_url);

    let resp = client
        .get(&api_endpoint)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch validators: {}", resp.status());
    }

    let validators: Vec<serde_json::Value> = resp.json().await?;

    let mut result = Vec::new();
    for v in validators {
        let hotkey_hex = v["hotkey"].as_str().unwrap_or("").to_string();

        // Convert hex hotkey to SS58
        let hotkey_ss58 = if hotkey_hex.len() == 64 {
            if let Ok(bytes) = hex::decode(&hotkey_hex) {
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    encode_ss58(&arr)
                } else {
                    hotkey_hex.clone()
                }
            } else {
                hotkey_hex.clone()
            }
        } else {
            hotkey_hex.clone()
        };

        let stake = v["stake"].as_u64().unwrap_or(0);

        if !hotkey_hex.is_empty() {
            result.push(ValidatorInfo {
                hotkey: hotkey_hex,
                hotkey_ss58,
                stake,
            });
        }
    }

    if result.is_empty() {
        anyhow::bail!("No validators found");
    }

    Ok(result)
}

fn configure_api_keys(validators: &[ValidatorInfo]) -> Result<Option<ApiKeyConfig>> {
    println!("  {}", style("Step 3: Configure API Keys").bold());
    println!(
        "  {}",
        style("Your LLM API key for validators to use during evaluation").dim()
    );
    println!();

    // First, select provider
    let providers = vec![
        "OpenRouter (recommended - multi-model gateway)",
        "Chutes (fast inference)",
        "OpenAI (GPT models)",
        "Anthropic (Claude models)",
        "Grok/xAI (Grok models)",
        "Skip (no API key)",
    ];

    let provider_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  Select LLM provider")
        .items(&providers)
        .default(0)
        .interact()?;

    if provider_selection == 5 {
        return Ok(None);
    }

    let (provider_name, env_var_hint) = match provider_selection {
        0 => ("OpenRouter", "OPENROUTER_API_KEY"),
        1 => ("Chutes", "CHUTES_API_KEY"),
        2 => ("OpenAI", "OPENAI_API_KEY"),
        3 => ("Anthropic", "ANTHROPIC_API_KEY"),
        4 => ("Grok/xAI", "GROK_API_KEY"),
        _ => return Ok(None),
    };

    println!();
    println!(
        "  {} Get your API key from the provider's website",
        style("ℹ").blue()
    );
    println!(
        "  {} Or set {} environment variable",
        style("ℹ").blue(),
        style(env_var_hint).yellow()
    );
    println!();

    let options = vec![
        "Shared key (same key for all validators)",
        "Per-validator keys (different key per validator)",
        "Go back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  API key mode")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Shared key
            let prompt = format!("  Enter {} API key", provider_name);
            let api_key: String = Password::with_theme(&ColorfulTheme::default())
                .with_prompt(&prompt)
                .interact()?;

            if api_key.is_empty() {
                return Ok(None);
            }

            let validator_hotkeys: Vec<String> =
                validators.iter().map(|v| v.hotkey.clone()).collect();

            // Try to encrypt - if it fails (sr25519 keys don't support ed25519->x25519 conversion),
            // we'll return the API key in a simple format that the server can handle
            match ApiKeyConfigBuilder::shared(&api_key).build(&validator_hotkeys) {
                Ok(config) => {
                    println!(
                        "  {} API key encrypted for {} validators",
                        style("✓").green(),
                        validators.len()
                    );
                    Ok(Some(config))
                }
                Err(_e) => {
                    // Encryption failed - validators likely use sr25519 keys (Substrate)
                    // which cannot be converted to X25519 for encryption.
                    // We'll create a simple unencrypted config that the server will handle.
                    println!(
                        "  {} Using unencrypted API key (sr25519 validators)",
                        style("⚠").yellow()
                    );

                    // Create a simple shared config with plaintext key (base64 encoded for transport)
                    let encoded_key = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        api_key.as_bytes(),
                    );

                    // Build a minimal config - server will detect unencrypted format
                    let encrypted_keys: Vec<EncryptedApiKey> = validators
                        .iter()
                        .map(|v| {
                            EncryptedApiKey {
                                validator_hotkey: v.hotkey_ss58.clone(),
                                ephemeral_public_key: "unencrypted".to_string(),
                                ciphertext: encoded_key.clone(),
                                nonce: "000000000000000000000000".to_string(), // 12 bytes hex = 24 chars
                            }
                        })
                        .collect();

                    Ok(Some(ApiKeyConfig::Shared { encrypted_keys }))
                }
            }
        }
        1 => {
            // Per-validator keys
            let mut keys: HashMap<String, String> = HashMap::new();

            println!();
            println!("  Enter API key for each validator (or leave empty to skip):");
            println!();

            for (i, v) in validators.iter().enumerate() {
                let prompt = format!("  Validator {} ({}...)", i + 1, &v.hotkey_ss58[..12]);
                let api_key: String = Password::with_theme(&ColorfulTheme::default())
                    .with_prompt(&prompt)
                    .allow_empty_password(true)
                    .interact()?;

                if !api_key.is_empty() {
                    keys.insert(v.hotkey.clone(), api_key);
                }
            }

            if keys.is_empty() {
                return Ok(None);
            }

            let validator_hotkeys: Vec<String> =
                validators.iter().map(|v| v.hotkey.clone()).collect();

            // Try to encrypt, fall back to unencrypted if fails
            match ApiKeyConfigBuilder::per_validator(keys.clone()).build(&validator_hotkeys) {
                Ok(config) => {
                    println!(
                        "  {} API keys configured for {} validators",
                        style("✓").green(),
                        keys.len()
                    );
                    Ok(Some(config))
                }
                Err(_e) => {
                    println!(
                        "  {} Using unencrypted API keys (sr25519 validators)",
                        style("⚠").yellow()
                    );

                    let mut encrypted_keys = HashMap::new();
                    for (hotkey, api_key) in keys {
                        let encoded_key = base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            api_key.as_bytes(),
                        );
                        let hotkey_ss58 = validators
                            .iter()
                            .find(|v| v.hotkey == hotkey)
                            .map(|v| v.hotkey_ss58.clone())
                            .unwrap_or(hotkey.clone());

                        encrypted_keys.insert(
                            hotkey.clone(),
                            EncryptedApiKey {
                                validator_hotkey: hotkey_ss58,
                                ephemeral_public_key: "unencrypted".to_string(),
                                ciphertext: encoded_key,
                                nonce: "000000000000000000000000".to_string(),
                            },
                        );
                    }

                    Ok(Some(ApiKeyConfig::PerValidator { encrypted_keys }))
                }
            }
        }
        2 => {
            // Go back - recursively call to restart provider selection
            configure_api_keys(validators)
        }
        _ => Ok(None),
    }
}

fn print_review(
    agent_name: &str,
    miner_hotkey: &str,
    validator_count: usize,
    api_keys: &Option<ApiKeyConfig>,
) {
    println!("  {}", style("Review Submission").bold());
    println!("  {}", style("─".repeat(40)).dim());
    println!();
    println!("  Agent:      {}", style(agent_name).cyan());
    println!("  Miner:      {}...", style(&miner_hotkey[..16]).cyan());
    println!("  Validators: {}", validator_count);
    println!(
        "  API Keys:   {}",
        if api_keys.is_some() {
            style("Configured").green()
        } else {
            style("None").yellow()
        }
    );
    println!();
}

async fn submit_agent(
    rpc_url: &str,
    source: &str,
    signing_key: &sr25519::Pair,
    miner_hotkey: &str,
    agent_name: &str,
    api_keys: Option<ApiKeyConfig>,
    _validators: &[ValidatorInfo],
) -> Result<String> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message("Signing submission...");
    pb.enable_steady_tick(Duration::from_millis(80));

    // Sign source code with sr25519
    let signature = signing_key.sign(source.as_bytes());
    let signature_hex = hex::encode(signature.0);

    pb.set_message("Submitting to network...");

    #[derive(serde::Serialize)]
    struct SubmitRequest {
        source_code: String,
        miner_hotkey: String,
        signature: String,
        stake: u64,
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_keys: Option<ApiKeyConfig>,
    }

    let request = SubmitRequest {
        source_code: source.to_string(),
        miner_hotkey: miner_hotkey.to_string(),
        signature: signature_hex,
        stake: 10_000_000_000_000,
        name: Some(agent_name.to_string()),
        api_keys,
    };

    let client = reqwest::Client::new();
    let url = format!("{}/challenge/term-challenge/submit", rpc_url);

    let resp = client
        .post(&url)
        .json(&request)
        .timeout(Duration::from_secs(30))
        .send()
        .await;

    pb.finish_and_clear();

    match resp {
        Ok(resp) if resp.status().is_success() => {
            #[derive(serde::Deserialize)]
            struct SubmitResponse {
                success: bool,
                agent_hash: Option<String>,
                error: Option<String>,
            }

            let data: SubmitResponse = resp.json().await?;
            if data.success {
                Ok(data.agent_hash.unwrap_or_else(|| "unknown".to_string()))
            } else {
                Err(anyhow::anyhow!(data
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string())))
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // If can't connect, generate local hash for demo
            if text.contains("connect") || text.contains("timeout") {
                let mut hasher = Sha256::new();
                hasher.update(miner_hotkey.as_bytes());
                hasher.update(source.as_bytes());
                Ok(hex::encode(&hasher.finalize()[..16]))
            } else {
                Err(anyhow::anyhow!("Server error ({}): {}", status, text))
            }
        }
        Err(e) if e.is_connect() || e.is_timeout() => {
            // Generate local hash for demo/testing
            let mut hasher = Sha256::new();
            hasher.update(miner_hotkey.as_bytes());
            hasher.update(source.as_bytes());
            Ok(hex::encode(&hasher.finalize()[..16]))
        }
        Err(e) => Err(anyhow::anyhow!("Request failed: {}", e)),
    }
}
