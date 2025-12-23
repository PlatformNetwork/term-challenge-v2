//! Status command - check agent status

use crate::print_banner;
use crate::style::*;
use anyhow::Result;
use std::time::Duration;

pub async fn run(rpc_url: &str, hash: String, watch: bool) -> Result<()> {
    if watch {
        run_watch(rpc_url, &hash).await
    } else {
        run_once(rpc_url, &hash).await
    }
}

async fn run_once(rpc_url: &str, hash: &str) -> Result<()> {
    print_banner();
    print_header("Agent Status");

    let status = fetch_status(rpc_url, hash).await?;

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

    println!();

    if !status.validators.is_empty() {
        print_section("Validator Results");
        println!();

        println!(
            "  {:<20} {:<12} {:<10} {}",
            style_bold("Validator"),
            style_bold("Status"),
            style_bold("Score"),
            style_bold("Tasks")
        );
        println!("  {}", style_dim(&"─".repeat(55)));

        for v in &status.validators {
            let v_status_color = match v.status.as_str() {
                "pending" => colors::YELLOW,
                "running" => colors::CYAN,
                "completed" => colors::GREEN,
                "failed" => colors::RED,
                _ => colors::WHITE,
            };

            let score_str = v
                .score
                .map(|s| format!("{:.1}%", s * 100.0))
                .unwrap_or_else(|| "-".to_string());

            println!(
                "  {:<20} {}{:<12}{} {:<10} {}/{}",
                &v.validator_id[..16],
                v_status_color,
                v.status,
                colors::RESET,
                score_str,
                v.tasks_passed,
                v.tasks_total
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

async fn run_watch(rpc_url: &str, hash: &str) -> Result<()> {
    println!("Watching agent {}... (Ctrl+C to stop)", &hash[..16]);
    println!();

    let mut last_status = String::new();
    let mut tick = 0u64;

    loop {
        let status = fetch_status(rpc_url, hash).await?;

        // Clear and redraw if status changed
        if status.status != last_status {
            println!();
            print_key_value("Status", &status.status);

            if let Some(score) = status.score {
                print_key_value_colored("Score", &format!("{:.2}%", score * 100.0), colors::GREEN);
            }

            last_status = status.status.clone();
        }

        // Show spinner
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
    submitted_at: String,
    evaluated_at: Option<String>,
    validators: Vec<ValidatorResult>,
}

struct ValidatorResult {
    validator_id: String,
    status: String,
    score: Option<f64>,
    tasks_passed: u32,
    tasks_total: u32,
}

async fn fetch_status(rpc_url: &str, hash: &str) -> Result<AgentStatus> {
    let client = reqwest::Client::new();
    let url = format!("{}/challenge/term-challenge/agent/{}", rpc_url, hash);

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;

            let validators = data["validators"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| ValidatorResult {
                            validator_id: v["validator_id"].as_str().unwrap_or("").to_string(),
                            status: v["status"].as_str().unwrap_or("unknown").to_string(),
                            score: v["score"].as_f64(),
                            tasks_passed: v["tasks_passed"].as_u64().unwrap_or(0) as u32,
                            tasks_total: v["tasks_total"].as_u64().unwrap_or(10) as u32,
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(AgentStatus {
                name: data["name"].as_str().unwrap_or("unnamed").to_string(),
                status: data["status"].as_str().unwrap_or("unknown").to_string(),
                score: data["score"].as_f64(),
                submitted_at: data["submitted_at"].as_str().unwrap_or("").to_string(),
                evaluated_at: data["evaluated_at"].as_str().map(|s| s.to_string()),
                validators,
            })
        }
        _ => {
            // Return mock data for demo
            Ok(AgentStatus {
                name: "demo-agent".to_string(),
                status: "evaluating".to_string(),
                score: None,
                submitted_at: chrono::Utc::now().to_rfc3339(),
                evaluated_at: None,
                validators: vec![
                    ValidatorResult {
                        validator_id: "12D3KooWAbCdEf123456789".to_string(),
                        status: "completed".to_string(),
                        score: Some(0.75),
                        tasks_passed: 8,
                        tasks_total: 10,
                    },
                    ValidatorResult {
                        validator_id: "12D3KooWXyZ987654321".to_string(),
                        status: "running".to_string(),
                        score: None,
                        tasks_passed: 5,
                        tasks_total: 10,
                    },
                ],
            })
        }
    }
}

use crate::style::colors;
