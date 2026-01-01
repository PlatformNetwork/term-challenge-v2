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

    // Step 1c: Check if this is a new version of existing agent
    println!();
    let is_new_version = check_existing_agent(&agent_name)?;
    if is_new_version {
        println!(
            "  {} Creating new version of existing agent",
            style("ℹ").blue()
        );
        println!(
            "  {} Previous versions will keep their scores on the leaderboard",
            style("ℹ").blue()
        );
    }

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

    // Step 5: Configure cost limit
    println!();
    let cost_limit = configure_cost_limit()?;

    // Step 6: Review and confirm
    println!();
    print_review(&agent_name, &miner_hotkey, &provider, cost_limit);

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
    let (submission_id, hash, version) = submit_agent(
        platform_url,
        &source,
        &signing_key,
        &miner_hotkey,
        &agent_name,
        &api_key,
        &provider,
        cost_limit,
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
    println!("  Version:       {}", style(version).cyan());
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

fn check_existing_agent(agent_name: &str) -> Result<bool> {
    println!("  {}", style("Checking agent name...").dim());

    let is_update = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "  Is '{}' an update to an existing agent? (creates new version)",
            style(agent_name).cyan()
        ))
        .default(false)
        .interact()?;

    if is_update {
        println!();
        println!(
            "  {}",
            style("╔═══════════════════════════════════════════════════════════════╗").blue()
        );
        println!(
            "  {}",
            style("║                     ℹ️  VERSION INFO  ℹ️                        ║").blue()
        );
        println!(
            "  {}",
            style("╠═══════════════════════════════════════════════════════════════╣").blue()
        );
        println!(
            "  {}",
            style("║                                                               ║").blue()
        );
        println!(
            "  {}",
            style("║  A new version will be created for this agent.                ║").blue()
        );
        println!(
            "  {}",
            style("║  Your previous version(s) will KEEP their scores.            ║").blue()
        );
        println!(
            "  {}",
            style("║  The version number is auto-assigned by the network.         ║").blue()
        );
        println!(
            "  {}",
            style("║                                                               ║").blue()
        );
        println!(
            "  {}",
            style("╚═══════════════════════════════════════════════════════════════╝").blue()
        );
    }

    Ok(is_update)
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

/// Maximum cost limit allowed (USD)
const MAX_COST_LIMIT_USD: f64 = 100.0;

/// Default cost limit (USD)
const DEFAULT_COST_LIMIT_USD: f64 = 10.0;

fn configure_cost_limit() -> Result<f64> {
    println!("  {}", style("Step 4: Configure Cost Limit").bold());
    println!("  {}", style("Maximum cost per validator in USD").dim());
    println!();

    // Warning box
    println!(
        "  {}",
        style("╔═══════════════════════════════════════════════════════════════╗").yellow()
    );
    println!(
        "  {}",
        style("║                    ⚠️  IMPORTANT WARNING  ⚠️                    ║").yellow()
    );
    println!(
        "  {}",
        style("╠═══════════════════════════════════════════════════════════════╣").yellow()
    );
    println!(
        "  {}",
        style("║                                                               ║").yellow()
    );
    println!(
        "  {}",
        style("║  Your API key will be used to make LLM calls during          ║").yellow()
    );
    println!(
        "  {}",
        style("║  evaluation. Each agent is evaluated by up to 3 validators.  ║").yellow()
    );
    println!(
        "  {}",
        style("║                                                               ║").yellow()
    );
    println!(
        "  {}",
        style("║  ▶ SET A CREDIT LIMIT ON YOUR API KEY PROVIDER! ◀            ║").yellow()
    );
    println!(
        "  {}",
        style("║                                                               ║").yellow()
    );
    println!(
        "  {}",
        style("║  We are NOT responsible for any additional costs incurred    ║").yellow()
    );
    println!(
        "  {}",
        style("║  if you do not set appropriate spending limits on your       ║").yellow()
    );
    println!(
        "  {}",
        style("║  API key provider account.                                   ║").yellow()
    );
    println!(
        "  {}",
        style("║                                                               ║").yellow()
    );
    println!(
        "  {}",
        style("╚═══════════════════════════════════════════════════════════════╝").yellow()
    );
    println!();

    let cost_str: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("  Cost limit per validator (USD, max 100)")
        .default(format!("{:.2}", DEFAULT_COST_LIMIT_USD))
        .validate_with(|input: &String| -> Result<(), String> {
            match input.parse::<f64>() {
                Ok(v) if (0.0..=MAX_COST_LIMIT_USD).contains(&v) => Ok(()),
                Ok(_) => Err(format!("Must be between 0 and {}", MAX_COST_LIMIT_USD)),
                Err(_) => Err("Invalid number".to_string()),
            }
        })
        .interact_text()?;

    let cost_limit: f64 = cost_str.parse().unwrap_or(DEFAULT_COST_LIMIT_USD);
    let total_max = cost_limit * 3.0;

    println!();
    println!(
        "  {} Cost limit: ${:.2}/validator (max total: ${:.2} for 3 validators)",
        style("✓").green(),
        cost_limit,
        total_max
    );

    Ok(cost_limit)
}

fn print_review(agent_name: &str, miner_hotkey: &str, provider: &str, cost_limit: f64) {
    println!("  {}", style("Review Submission").bold());
    println!();
    println!("  Agent:      {}", style(agent_name).cyan());
    println!("  Hotkey:     {}...", style(&miner_hotkey[..16]).cyan());
    println!("  Provider:   {}", style(provider).cyan());
    println!(
        "  Cost Limit: {} per validator (max ${:.2} total)",
        style(format!("${:.2}", cost_limit)).cyan(),
        cost_limit * 3.0
    );
    println!();
}

#[allow(clippy::too_many_arguments)]
async fn submit_agent(
    platform_url: &str,
    source: &str,
    signing_key: &sr25519::Pair,
    miner_hotkey: &str,
    name: &str,
    api_key: &str,
    provider: &str,
    cost_limit_usd: f64,
) -> Result<(String, String, i32)> {
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
        "cost_limit_usd": cost_limit_usd,
    });

    // Use bridge route: /api/v1/bridge/{challenge}/submit
    let url = format!("{}/api/v1/bridge/term-challenge/submit", platform_url);

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
        let version = resp["version"].as_i64().unwrap_or(1) as i32;
        Ok((submission_id, hash, version))
    } else {
        let error = response.text().await?;
        Err(anyhow::anyhow!("Submission failed: {}", error))
    }
}
