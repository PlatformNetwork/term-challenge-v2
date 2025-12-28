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

    // Try to find submission by hash (partial match)
    let submissions_url = format!("{}/api/v1/submissions", platform_url);

    let submissions: Vec<serde_json::Value> = client
        .get(&submissions_url)
        .send()
        .await?
        .json()
        .await
        .unwrap_or_default();

    // Find matching submission
    let submission = submissions.iter().find(|s| {
        s["agent_hash"]
            .as_str()
            .map(|h| h.starts_with(hash) || hash.starts_with(&h[..hash.len().min(h.len())]))
            .unwrap_or(false)
    });

    if let Some(sub) = submission {
        let agent_hash = sub["agent_hash"].as_str().unwrap_or(hash);

        // Fetch evaluations for this agent
        let evals_url = format!("{}/api/v1/evaluations/{}", platform_url, agent_hash);
        let evals: Vec<serde_json::Value> = client
            .get(&evals_url)
            .send()
            .await
            .ok()
            .and_then(|r| {
                if r.status().is_success() {
                    Some(r)
                } else {
                    None
                }
            })
            .and_then(|r| futures::executor::block_on(r.json()).ok())
            .unwrap_or_default();

        let evaluations: Vec<EvaluationInfo> = evals
            .iter()
            .map(|e| EvaluationInfo {
                validator_hotkey: e["validator_hotkey"].as_str().unwrap_or("").to_string(),
                score: e["score"].as_f64().unwrap_or(0.0),
                tasks_passed: e["tasks_passed"].as_u64().unwrap_or(0) as u32,
                tasks_total: e["tasks_total"].as_u64().unwrap_or(0) as u32,
                total_cost_usd: e["total_cost_usd"].as_f64().unwrap_or(0.0),
            })
            .collect();

        // Compute aggregate score
        let avg_score = if !evaluations.is_empty() {
            Some(evaluations.iter().map(|e| e.score).sum::<f64>() / evaluations.len() as f64)
        } else {
            None
        };

        let status = sub["status"].as_str().unwrap_or("pending").to_string();
        let tasks_info = if !evaluations.is_empty() {
            let total_passed: u32 = evaluations.iter().map(|e| e.tasks_passed).sum();
            let total_tasks: u32 = evaluations.iter().map(|e| e.tasks_total).sum();
            Some(format!("{}/{}", total_passed, total_tasks))
        } else {
            None
        };

        return Ok(AgentStatus {
            name: sub["name"].as_str().unwrap_or("unnamed").to_string(),
            status,
            score: avg_score,
            tasks_info,
            submitted_at: sub["created_at"].as_str().unwrap_or("").to_string(),
            evaluated_at: None,
            evaluations,
        });
    }

    // Not found
    Err(anyhow::anyhow!(
        "Agent not found. Check the hash or submit an agent first.\n\
         Searched for: {}\n\
         API: {}/api/v1/submissions",
        hash,
        platform_url
    ))
}

use crate::style::colors;
