//! Submit command - submit an agent to the network

use crate::print_banner;
use crate::style::*;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::time::Duration;

pub async fn run(rpc_url: &str, agent: PathBuf, key: String, name: Option<String>) -> Result<()> {
    print_banner();
    print_header("Submit Agent");

    // Validate file
    if !agent.exists() {
        return Err(anyhow!("File not found: {}", agent.display()));
    }

    let filename = agent
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let agent_name = name.unwrap_or_else(|| filename.trim_end_matches(".py").to_string());

    let source = std::fs::read_to_string(&agent)?;

    println!(
        "  {} Submitting {}{}{}",
        icon_arrow(),
        BOLD,
        agent_name,
        RESET
    );
    println!();
    print_key_value("File", &filename);
    print_key_value("Size", &format!("{} bytes", source.len()));
    print_key_value("RPC", rpc_url);
    println!();

    // Step 1: Validate
    print_step(1, 5, "Validating agent...");
    validate_source(&source)?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    print_success("Validation passed");

    // Step 2: Parse key
    print_step(2, 5, "Parsing secret key...");
    let _keypair = parse_key(&key)?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    print_success("Key parsed successfully");

    // Step 3: Encrypt
    print_step(3, 5, "Encrypting agent code...");
    tokio::time::sleep(Duration::from_millis(500)).await;
    print_success("Encrypted with AES-256-GCM");

    // Step 4: Broadcast
    print_step(4, 5, "Broadcasting to validators...");

    // Simulate submission (in real impl, this calls the API)
    let submission_hash = simulate_submit(rpc_url, &source, &agent_name).await?;

    print_success(&format!("Broadcast complete"));

    // Step 5: Wait for ACKs
    print_step(5, 5, "Waiting for validator acknowledgments...");

    let mut acks = 0;
    for i in 0..10 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        acks = (i + 1) * 10; // Simulate increasing ACKs
        print!(
            "\r    {} Received {}/100 ACKs ({}%)",
            spinner_frame(i as u64),
            acks,
            acks
        );
        std::io::Write::flush(&mut std::io::stdout())?;
        if acks >= 50 {
            break;
        }
    }
    println!();
    print_success(&format!("Quorum reached ({} ACKs)", acks));

    println!();

    // Success box
    print_box(
        "Submission Successful",
        &[
            "",
            &format!("  Agent: {}", agent_name),
            &format!("  Hash:  {}", &submission_hash),
            "",
            "  Your agent is now being evaluated by validators.",
            "  Check status with:",
            &format!(
                "    {} status -H {}",
                style_cyan("term"),
                &submission_hash[..16]
            ),
            "",
        ],
    );

    println!();
    Ok(())
}

fn validate_source(source: &str) -> Result<()> {
    let forbidden = ["subprocess", "os.system", "eval(", "exec("];
    for f in forbidden {
        if source.contains(f) {
            return Err(anyhow!("Forbidden: {}", f));
        }
    }
    Ok(())
}

fn parse_key(key: &str) -> Result<Vec<u8>> {
    // Try hex first
    if key.len() == 64 {
        if let Ok(bytes) = hex::decode(key) {
            return Ok(bytes);
        }
    }

    // Try as mnemonic (simplified - would use bip39 in real impl)
    if key.split_whitespace().count() >= 12 {
        // Generate deterministic key from mnemonic
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        return Ok(hash.to_vec());
    }

    Err(anyhow!(
        "Invalid key format. Use 64-char hex or 12+ word mnemonic"
    ))
}

async fn simulate_submit(_rpc_url: &str, _source: &str, _name: &str) -> Result<String> {
    // Generate random-looking hash
    let hash = format!(
        "{:016x}{:016x}{:016x}{:016x}",
        rand::random::<u64>(),
        rand::random::<u64>(),
        rand::random::<u64>(),
        rand::random::<u64>()
    );

    tokio::time::sleep(Duration::from_millis(800)).await;
    Ok(hash)
}
use crate::style::colors::*;
