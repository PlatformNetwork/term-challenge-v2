//! Submit Wizard - Interactive CLI

use anyhow::Result;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use sha2::{Digest, Sha256};
use sp_core::{crypto::Ss58Codec, sr25519, Pair};
use std::path::PathBuf;
use std::time::Duration;
use term_challenge::{PythonWhitelist, WhitelistConfig};

pub async fn run_submit_wizard(platform_url: &str) -> Result<()> {
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

    // Step 4: Configure API key
    println!();
    let (api_key, provider) = configure_api_key()?;

    // Step 5: Review and confirm
    println!();
    print_review(&agent_name, &miner_hotkey, &provider);

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("  Submit agent to network?")
        .default(true)
        .interact()?;

    if !confirmed {
        println!();
        println!("  {} Cancelled", style("✗").red());
        return Ok(());
    }

    // Step 6: Submit
    println!();
    let (submission_id, hash) = submit_agent(
        platform_url,
        &source,
        &signing_key,
        &miner_hotkey,
        &agent_name,
        &api_key,
        &provider,
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
    println!("  Submission ID: {}", style(&submission_id).cyan().bold());
    println!("  Agent Hash:    {}", style(&hash).cyan());
    println!();
    println!(
        "  Check status: {} status -H {}",
        style("term").cyan(),
        &hash[..16]
    );
    println!();

    Ok(())
}

fn print_banner() {
    println!(
        "{}",
        style(
            r#"
  ████████╗███████╗██████╗ ███╗   ███╗
  ╚══██╔══╝██╔════╝██╔══██╗████╗ ████║
     ██║   █████╗  ██████╔╝██╔████╔██║
     ██║   ██╔══╝  ██╔══██╗██║╚██╔╝██║
     ██║   ███████╗██║  ██║██║ ╚═╝ ██║
     ╚═╝   ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝
"#
        )
        .cyan()
    );
}

fn select_agent_file() -> Result<PathBuf> {
    println!("  {}", style("Step 1: Select Agent File").bold());
    println!(
        "  {}",
        style("Enter the path to your Python agent file").dim()
    );
    println!();

    let path_str: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("  Agent file path")
        .validate_with(|input: &String| -> Result<(), String> {
            let path = PathBuf::from(input);
            if !path.exists() {
                return Err(format!("File not found: {}", input));
            }
            if !input.ends_with(".py") {
                return Err("File must be a Python file (.py)".to_string());
            }
            Ok(())
        })
        .interact_text()?;

    Ok(PathBuf::from(path_str))
}

fn enter_miner_key() -> Result<(sr25519::Pair, String)> {
    println!("  {}", style("Step 2: Enter Miner Key").bold());
    println!(
        "  {}",
        style("Your miner secret key (hex or mnemonic)").dim()
    );
    println!();

    let key: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("  Secret key")
        .interact()?;

    // Try hex first
    if key.len() == 64 {
        if let Ok(bytes) = hex::decode(&key) {
            if bytes.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&bytes);
                let pair = sr25519::Pair::from_seed(&seed);
                // Use SS58 format for hotkey (Bittensor standard)
                let hotkey_ss58 = pair.public().to_ss58check();
                return Ok((pair, hotkey_ss58));
            }
        }
    }

    // Try mnemonic
    if key.split_whitespace().count() >= 12 {
        let (pair, _) = sr25519::Pair::from_phrase(&key, None)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {:?}", e))?;
        // Use SS58 format for hotkey (Bittensor standard)
        let hotkey_ss58 = pair.public().to_ss58check();
        return Ok((pair, hotkey_ss58));
    }

    Err(anyhow::anyhow!(
        "Invalid key format. Use 64-char hex or 12+ word mnemonic"
    ))
}

fn validate_agent(source: &str) -> Result<()> {
    // Check for forbidden patterns
    let forbidden = ["subprocess", "os.system", "eval(", "exec("];
    for f in forbidden {
        if source.contains(f) {
            println!("  {} Forbidden pattern detected: {}", style("✗").red(), f);
            return Err(anyhow::anyhow!("Forbidden pattern: {}", f));
        }
    }

    // Check whitelist
    let whitelist = PythonWhitelist::new(WhitelistConfig::default());
    let result = whitelist.verify(source);
    if result.valid {
        println!("  {} Module whitelist check passed", style("✓").green());
    } else {
        for error in &result.errors {
            println!("  {} {}", style("✗").red(), error);
        }
        for warning in &result.warnings {
            println!("  {} {}", style("⚠").yellow(), warning);
        }
    }

    // Check for Agent class
    if !source.contains("class") || !source.contains("Agent") {
        println!(
            "  {} No Agent class detected (should extend Agent)",
            style("⚠").yellow()
        );
    }

    Ok(())
}

fn configure_api_key() -> Result<(String, String)> {
    println!("  {}", style("Step 3: Configure API Key").bold());
    println!("  {}", style("Your LLM API key for evaluation").dim());
    println!();

    let providers = vec!["OpenRouter (recommended)", "Chutes", "OpenAI", "Anthropic"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  Select LLM provider")
        .items(&providers)
        .default(0)
        .interact()?;

    let (provider, env_hint) = match selection {
        0 => ("openrouter", "OPENROUTER_API_KEY"),
        1 => ("chutes", "CHUTES_API_KEY"),
        2 => ("openai", "OPENAI_API_KEY"),
        3 => ("anthropic", "ANTHROPIC_API_KEY"),
        _ => ("openrouter", "OPENROUTER_API_KEY"),
    };

    println!();
    println!(
        "  {} Get your key from the provider's website",
        style("ℹ").blue()
    );
    println!(
        "  {} Or set {} env var",
        style("ℹ").blue(),
        style(env_hint).yellow()
    );
    println!();

    let api_key: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("  Enter API key")
        .interact()?;

    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key is required"));
    }

    println!("  {} API key configured ({})", style("✓").green(), provider);

    Ok((api_key, provider.to_string()))
}

fn print_review(agent_name: &str, miner_hotkey: &str, provider: &str) {
    println!("  {}", style("Review Submission").bold());
    println!();
    println!("  Agent:    {}", style(agent_name).cyan());
    println!("  Hotkey:   {}...", style(&miner_hotkey[..16]).cyan());
    println!("  Provider: {}", style(provider).cyan());
    println!();
}

async fn submit_agent(
    platform_url: &str,
    source: &str,
    signing_key: &sr25519::Pair,
    miner_hotkey: &str,
    name: &str,
    api_key: &str,
    provider: &str,
) -> Result<(String, String)> {
    println!("  {} Submitting to {}...", style("→").cyan(), platform_url);

    let client = reqwest::Client::new();

    // Compute source code hash
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let source_hash = hex::encode(hasher.finalize());

    // Create message to sign: "submit_agent:<sha256_of_source_code>"
    // This proves the miner owns this hotkey and is submitting this specific code
    let message = format!("submit_agent:{}", source_hash);

    // Sign the message (not the source code directly)
    let signature = signing_key.sign(message.as_bytes());
    let signature_hex = hex::encode(signature.0);

    // Compute agent hash (first 16 bytes of source hash)
    let agent_hash = source_hash[..32].to_string();

    let request = serde_json::json!({
        "source_code": source,
        "miner_hotkey": miner_hotkey,  // SS58 format
        "signature": signature_hex,     // No 0x prefix
        "name": name,
        "api_key": api_key,
        "api_provider": provider,
    });

    let url = format!("{}/api/v1/submissions", platform_url);

    let response = client
        .post(&url)
        .json(&request)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    if response.status().is_success() {
        let resp: serde_json::Value = response.json().await?;
        let submission_id = resp["submission_id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let hash = resp["agent_hash"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or(agent_hash);
        Ok((submission_id, hash))
    } else {
        let error = response.text().await?;
        Err(anyhow::anyhow!("Submission failed: {}", error))
    }
}
