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

async fn fetch_stats(platform_url: &str) -> Result<NetworkStats> {
    let client = reqwest::Client::new();
    let mut stats = NetworkStats::default();

    // Fetch validators from REST API
    let validators_url = format!("{}/api/v1/validators", platform_url);
    if let Ok(resp) = client.get(&validators_url).send().await {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<Vec<serde_json::Value>>().await {
                stats.validators = data.len() as u32;
            }
        }
    }

    // Fetch network state from REST API
    let state_url = format!("{}/api/v1/network/state", platform_url);
    if let Ok(resp) = client.get(&state_url).send().await {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                stats.current_epoch = data["current_epoch"].as_u64().unwrap_or(0);
                stats.validators = data["active_validators"]
                    .as_u64()
                    .unwrap_or(stats.validators as u64) as u32;
            }
        }
    }

    // Fetch leaderboard from REST API
    let leaderboard_url = format!("{}/api/v1/leaderboard", platform_url);
    if let Ok(resp) = client.get(&leaderboard_url).send().await {
        if resp.status().is_success() {
            if let Ok(entries) = resp.json::<Vec<serde_json::Value>>().await {
                stats.total_agents = entries.len() as u32;
                stats.active_agents = entries
                    .iter()
                    .filter(|e| e["consensus_score"].as_f64().unwrap_or(0.0) > 0.0)
                    .count() as u32;

                if !entries.is_empty() {
                    let scores: Vec<f64> = entries
                        .iter()
                        .filter_map(|e| e["consensus_score"].as_f64())
                        .filter(|&s| s > 0.0)
                        .collect();

                    if !scores.is_empty() {
                        stats.best_score = scores.iter().cloned().fold(0.0, f64::max);
                        stats.avg_score = scores.iter().sum::<f64>() / scores.len() as f64;

                        let mut sorted = scores.clone();
                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        stats.median_score = sorted[sorted.len() / 2];
                    }

                    // Recent submissions
                    stats.recent_submissions = entries
                        .iter()
                        .take(5)
                        .filter_map(|e| {
                            Some(RecentSubmission {
                                hash: e["agent_hash"].as_str()?.to_string(),
                                score: e["consensus_score"].as_f64(),
                                time: e["updated_at"].as_str().unwrap_or("recent").to_string(),
                            })
                        })
                        .collect();
                }
            }
        }
    }

    Ok(stats)
}

use crate::style::colors;
