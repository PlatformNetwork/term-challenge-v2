//! Stats command - show network statistics

use crate::print_banner;
use crate::style::*;
use anyhow::Result;

pub async fn run(rpc_url: &str) -> Result<()> {
    print_banner();
    print_header("Network Statistics");

    let stats = fetch_stats(rpc_url).await.unwrap_or_default();

    print_section("Network Status");
    println!();

    let status_color = if stats.validators > 0 {
        colors::GREEN
    } else {
        colors::RED
    };
    let status_text = if stats.validators > 0 {
        "Online"
    } else {
        "Offline"
    };
    print_key_value_colored("Status", status_text, status_color);
    print_key_value("Validators", &stats.validators.to_string());
    print_key_value("Current Epoch", &stats.current_epoch.to_string());
    println!();

    print_section("Agents");
    println!();
    print_key_value("Total Submitted", &stats.total_agents.to_string());
    print_key_value("Active", &stats.active_agents.to_string());
    print_key_value("Evaluated Today", &stats.evaluated_today.to_string());
    println!();

    print_section("Scores");
    println!();
    print_key_value_colored(
        "Best Score",
        &format!("{:.2}%", stats.best_score * 100.0),
        colors::GREEN,
    );
    print_key_value("Average Score", &format!("{:.2}%", stats.avg_score * 100.0));
    print_key_value(
        "Median Score",
        &format!("{:.2}%", stats.median_score * 100.0),
    );
    println!();

    print_section("Recent Activity");
    println!();

    if stats.recent_submissions.is_empty() {
        println!("    {} No recent submissions", style_dim("─"));
    } else {
        for sub in &stats.recent_submissions {
            let score_str = sub
                .score
                .map(|s| format!("{:.1}%", s * 100.0))
                .unwrap_or_else(|| "pending".to_string());

            let score_color = sub
                .score
                .map(|s| {
                    if s >= 0.7 {
                        colors::GREEN
                    } else if s >= 0.5 {
                        colors::YELLOW
                    } else {
                        colors::RED
                    }
                })
                .unwrap_or(colors::GRAY);

            println!(
                "    {} {}  {}{}{}  {}",
                icon_bullet(),
                style_dim(&sub.time),
                score_color,
                score_str,
                colors::RESET,
                style_gray(&format!("({})", &sub.hash[..8]))
            );
        }
    }

    println!();
    Ok(())
}

#[derive(Default)]
struct NetworkStats {
    validators: u32,
    current_epoch: u64,
    total_agents: u32,
    active_agents: u32,
    evaluated_today: u32,
    best_score: f64,
    avg_score: f64,
    median_score: f64,
    recent_submissions: Vec<RecentSubmission>,
}

struct RecentSubmission {
    hash: String,
    score: Option<f64>,
    time: String,
}

async fn fetch_stats(rpc_url: &str) -> Result<NetworkStats> {
    let client = reqwest::Client::new();
    let url = format!("{}/challenge/term-bench/stats", rpc_url);

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;
            Ok(NetworkStats {
                validators: data["validators"].as_u64().unwrap_or(0) as u32,
                current_epoch: data["current_epoch"].as_u64().unwrap_or(0),
                total_agents: data["total_agents"].as_u64().unwrap_or(0) as u32,
                active_agents: data["active_agents"].as_u64().unwrap_or(0) as u32,
                evaluated_today: data["evaluated_today"].as_u64().unwrap_or(0) as u32,
                best_score: data["best_score"].as_f64().unwrap_or(0.0),
                avg_score: data["avg_score"].as_f64().unwrap_or(0.0),
                median_score: data["median_score"].as_f64().unwrap_or(0.0),
                recent_submissions: Vec::new(),
            })
        }
        _ => Ok(NetworkStats::default()),
    }
}

use crate::style::colors;
