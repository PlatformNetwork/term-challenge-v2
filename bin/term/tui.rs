//! TUI Dashboard module
//!
//! Full interactive dashboard for managing agents.
//! For now, redirects to simpler functionality.

use crate::print_banner;
use crate::style::*;
use anyhow::Result;

pub async fn run(rpc_url: &str, key: Option<String>) -> Result<()> {
    print_banner();

    println!("  {} Interactive Dashboard", style_bold("Starting"));
    println!();

    if key.is_none() {
        print_warning("No secret key provided. Some features will be limited.");
        println!("  Run with: {} dashboard -k YOUR_KEY", style_cyan("term"));
        println!();
    }

    print_info(&format!("Connecting to {}...", rpc_url));
    println!();

    // For now, show menu
    println!("  {}", style_bold("Available Commands:"));
    println!();
    println!(
        "    {}  Test an agent locally",
        style_cyan("term test -a agent.py")
    );
    println!(
        "    {}  Submit to network",
        style_cyan("term submit -a agent.py -k KEY")
    );
    println!("    {}  Check status", style_cyan("term status -H HASH"));
    println!("    {}  View leaderboard", style_cyan("term leaderboard"));
    println!("    {}  Show config", style_cyan("term config"));
    println!();

    print_box(
        "Coming Soon",
        &[
            "Full interactive TUI dashboard with:",
            "",
            "  • Real-time agent monitoring",
            "  • Live evaluation progress",
            "  • Network statistics",
            "  • Submission management",
        ],
    );

    println!();
    Ok(())
}
