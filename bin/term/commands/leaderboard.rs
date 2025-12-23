//! Leaderboard command - show top agents

use crate::print_banner;
use crate::style::*;
use anyhow::Result;

pub async fn run(rpc_url: &str, limit: usize) -> Result<()> {
    print_banner();
    print_header("Leaderboard");

    let entries = fetch_leaderboard(rpc_url, limit).await?;

    if entries.is_empty() {
        println!("  {} No agents on the leaderboard yet.", style_dim("─"));
        println!();
        print_info("Be the first to submit an agent!");
        println!(
            "  Run: {}",
            style_cyan("term submit -a your_agent.py -k YOUR_KEY")
        );
        println!();
        return Ok(());
    }

    // Table header
    println!(
        "  {:<4} {:<10} {:<18} {:<10} {:<12} {}",
        style_bold("Rank"),
        style_bold("Score"),
        style_bold("Agent"),
        style_bold("Tasks"),
        style_bold("Submitted"),
        style_bold("Miner")
    );
    println!("  {}", style_dim(&"─".repeat(75)));

    // Table rows
    for (i, entry) in entries.iter().enumerate() {
        let rank = i + 1;
        let rank_icon = match rank {
            1 => "🥇",
            2 => "🥈",
            3 => "🥉",
            _ => "  ",
        };

        let score_color = if entry.score >= 0.8 {
            colors::GREEN
        } else if entry.score >= 0.6 {
            colors::YELLOW
        } else {
            colors::RED
        };

        let tasks_str = format!("{}/{}", entry.tasks_passed, entry.tasks_total);

        println!(
            "  {}{:<2} {}{:>6.2}%{}  {:<18} {:<10} {:<12} {}",
            rank_icon,
            rank,
            score_color,
            entry.score * 100.0,
            colors::RESET,
            truncate(&entry.name, 16),
            tasks_str,
            &entry.submitted[..10],
            style_gray(&format!("{}...", &entry.miner[..8]))
        );
    }

    println!();

    // Summary
    let total = entries.len();
    let avg_score: f64 = entries.iter().map(|e| e.score).sum::<f64>() / total as f64;

    print_section("Summary");
    print_key_value("Total Agents", &total.to_string());
    print_key_value("Average Score", &format!("{:.2}%", avg_score * 100.0));

    if let Some(best) = entries.first() {
        print_key_value_colored(
            "Best Score",
            &format!("{:.2}%", best.score * 100.0),
            colors::GREEN,
        );
    }

    println!();
    Ok(())
}

struct LeaderboardEntry {
    name: String,
    score: f64,
    tasks_passed: u32,
    tasks_total: u32,
    submitted: String,
    miner: String,
}

async fn fetch_leaderboard(rpc_url: &str, limit: usize) -> Result<Vec<LeaderboardEntry>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/challenge/term-challenge/leaderboard?limit={}",
        rpc_url, limit
    );

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: Vec<serde_json::Value> = resp.json().await?;
            Ok(data
                .iter()
                .map(|v| LeaderboardEntry {
                    name: v["name"].as_str().unwrap_or("unnamed").to_string(),
                    score: v["score"].as_f64().unwrap_or(0.0),
                    tasks_passed: v["tasks_passed"].as_u64().unwrap_or(0) as u32,
                    tasks_total: v["tasks_total"].as_u64().unwrap_or(10) as u32,
                    submitted: v["submitted_at"].as_str().unwrap_or("").to_string(),
                    miner: v["miner_hotkey"].as_str().unwrap_or("").to_string(),
                })
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

use crate::style::colors;
