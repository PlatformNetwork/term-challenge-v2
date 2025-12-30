//! Status command - check agent status

use crate::print_banner;
use crate::style::*;
use anyhow::Result;
use std::time::Duration;

pub async fn run(platform_url: &str, hash: String, watch: bool) -> Result<()> {
    if watch {
        run_watch(platform_url, &hash).await
    } else {
        run_once(platform_url, &hash).await
    }
}

async fn run_once(platform_url: &str, hash: &str) -> Result<()> {
    print_banner();
    print_header("Agent Status");

    let status = fetch_status(platform_url, hash).await?;

    print_key_value("Hash", hash);
    print_key_value("Name", &status.name);

    let status_color = match status.status.as_str() {
        "pending" => colors::YELLOW,
        "evaluating" => colors::CYAN,
        "completed" => colors::GREEN,
        "failed" => colors::RED,
        _ => colors::WHITE,
    };
    print_key_value_colored("Status", &status.status, status_color);

    if let Some(score) = status.score {
        print_key_value_colored("Score", &format!("{:.2}%", score * 100.0), colors::GREEN);
    }

    if let Some(tasks) = &status.tasks_info {
        print_key_value("Tasks", tasks);
    }

    println!();

    if !status.evaluations.is_empty() {
        print_section("Evaluations");
        println!();

        println!(
            "  {:<20} {:<12} {:<10} {}",
            style_bold("Validator"),
            style_bold("Score"),
            style_bold("Tasks"),
            style_bold("Cost")
        );
        println!("  {}", style_dim(&"─".repeat(55)));

        for eval in &status.evaluations {
            let score_str = format!("{:.1}%", eval.score * 100.0);
            let tasks_str = format!("{}/{}", eval.tasks_passed, eval.tasks_total);

            println!(
                "  {:<20} {}{:<12}{} {:<10} ${:.4}",
                &eval.validator_hotkey[..16.min(eval.validator_hotkey.len())],
                colors::GREEN,
                score_str,
                colors::RESET,
                tasks_str,
                eval.total_cost_usd
            );
        }
    }

    println!();

    // Show timeline
    print_section("Timeline");
    println!();

    println!(
        "    {} {} Submitted",
        icon_success(),
        style_dim(&status.submitted_at)
    );

    if status.status != "pending" {
        println!(
            "    {} {} Evaluation started",
            icon_success(),
            style_dim("...")
        );
    }

    if status.status == "completed" {
        if let Some(eval_at) = &status.evaluated_at {
            println!(
                "    {} {} Evaluation completed",
                icon_success(),
                style_dim(eval_at)
            );
        }
    } else if status.status == "evaluating" {
        println!("    {} {} Evaluating...", style_cyan("◉"), style_dim("now"));
    }

    println!();
    Ok(())
}

async fn run_watch(platform_url: &str, hash: &str) -> Result<()> {
    println!(
        "Watching agent {}... (Ctrl+C to stop)",
        &hash[..16.min(hash.len())]
    );
    println!();

    let mut last_status = String::new();
    let mut tick = 0u64;

    loop {
        let status = fetch_status(platform_url, hash).await?;

        if status.status != last_status {
            println!();
            print_key_value("Status", &status.status);

            if let Some(score) = status.score {
                print_key_value_colored("Score", &format!("{:.2}%", score * 100.0), colors::GREEN);
            }

            last_status = status.status.clone();
        }

        print!("\r  {} Watching... ", spinner_frame(tick));
        std::io::Write::flush(&mut std::io::stdout())?;

        if status.status == "completed" || status.status == "failed" {
            println!();
            println!();
            print_success("Agent evaluation complete!");
            break;
        }

        tick += 1;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

struct AgentStatus {
    name: String,
    status: String,
    score: Option<f64>,
    tasks_info: Option<String>,
    submitted_at: String,
    evaluated_at: Option<String>,
    evaluations: Vec<EvaluationInfo>,
}

struct EvaluationInfo {
    validator_hotkey: String,
    score: f64,
    tasks_passed: u32,
    tasks_total: u32,
    total_cost_usd: f64,
}

async fn fetch_status(platform_url: &str, hash: &str) -> Result<AgentStatus> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Use bridge route to term-challenge - get agent details
    let agent_url = format!(
        "{}/api/v1/bridge/term-challenge/leaderboard/{}",
        platform_url, hash
    );

    let resp = client.get(&agent_url).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Agent not found. Check the hash or submit an agent first.\n\
             Searched for: {}\n\
             Status: {}\n\
             Response: {}",
            hash,
            status,
            text
        ));
    }

    let agent: serde_json::Value = resp.json().await?;

    // Build status from response
    let status = agent["status"].as_str().unwrap_or("pending").to_string();
    let validators_completed = agent["validators_completed"].as_i64().unwrap_or(0) as i32;
    let total_validators = agent["total_validators"].as_i64().unwrap_or(0) as i32;

    let tasks_info = if validators_completed > 0 && total_validators > 0 {
        Some(format!(
            "{}/{} validators",
            validators_completed, total_validators
        ))
    } else {
        None
    };

    Ok(AgentStatus {
        name: agent["name"].as_str().unwrap_or("unnamed").to_string(),
        status,
        score: agent["best_score"].as_f64(),
        tasks_info,
        submitted_at: agent["submitted_at"].as_str().unwrap_or("").to_string(),
        evaluated_at: None,
        evaluations: vec![],
    })
}

use crate::style::colors;
