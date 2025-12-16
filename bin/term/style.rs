//! Terminal styling utilities for beautiful CLI output

/// ANSI color codes
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const ITALIC: &str = "\x1b[3m";
    pub const UNDERLINE: &str = "\x1b[4m";

    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    pub const GRAY: &str = "\x1b[90m";

    pub const BG_RED: &str = "\x1b[41m";
    pub const BG_GREEN: &str = "\x1b[42m";
    pub const BG_YELLOW: &str = "\x1b[43m";
    pub const BG_BLUE: &str = "\x1b[44m";
}

use colors::*;

// Style functions
pub fn style_bold(s: &str) -> String {
    format!("{}{}{}", BOLD, s, RESET)
}

pub fn style_dim(s: &str) -> String {
    format!("{}{}{}", DIM, s, RESET)
}

pub fn style_red(s: &str) -> String {
    format!("{}{}{}", RED, s, RESET)
}

pub fn style_green(s: &str) -> String {
    format!("{}{}{}", GREEN, s, RESET)
}

pub fn style_yellow(s: &str) -> String {
    format!("{}{}{}", YELLOW, s, RESET)
}

pub fn style_blue(s: &str) -> String {
    format!("{}{}{}", BLUE, s, RESET)
}

pub fn style_cyan(s: &str) -> String {
    format!("{}{}{}", CYAN, s, RESET)
}

pub fn style_magenta(s: &str) -> String {
    format!("{}{}{}", MAGENTA, s, RESET)
}

pub fn style_gray(s: &str) -> String {
    format!("{}{}{}", GRAY, s, RESET)
}

// Status indicators
pub fn icon_success() -> String {
    format!("{}✓{}", GREEN, RESET)
}

pub fn icon_error() -> String {
    format!("{}✗{}", RED, RESET)
}

pub fn icon_warning() -> String {
    format!("{}⚠{}", YELLOW, RESET)
}

pub fn icon_info() -> String {
    format!("{}ℹ{}", BLUE, RESET)
}

pub fn icon_arrow() -> String {
    format!("{}→{}", CYAN, RESET)
}

pub fn icon_bullet() -> String {
    format!("{}•{}", GRAY, RESET)
}

// Print helpers
pub fn print_success(msg: &str) {
    println!("{} {}", icon_success(), msg);
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}{}{}", icon_error(), RED, msg, RESET);
}

pub fn print_warning(msg: &str) {
    println!("{} {}{}{}", icon_warning(), YELLOW, msg, RESET);
}

pub fn print_info(msg: &str) {
    println!("{} {}", icon_info(), msg);
}

pub fn print_step(step: u32, total: u32, msg: &str) {
    println!(
        "{} {}{}/{}{} {}",
        icon_arrow(),
        CYAN,
        step,
        total,
        RESET,
        msg
    );
}

// Section headers
pub fn print_header(title: &str) {
    println!();
    println!(
        "{}{} {} {}{}",
        BOLD,
        CYAN,
        title,
        "─".repeat(50 - title.len()),
        RESET
    );
    println!();
}

pub fn print_section(title: &str) {
    println!();
    println!("  {}{}{}", BOLD, title, RESET);
    println!("  {}", style_dim(&"─".repeat(40)));
}

// Table helpers
pub fn print_key_value(key: &str, value: &str) {
    println!("  {}{}:{} {}", GRAY, key, RESET, value);
}

pub fn print_key_value_colored(key: &str, value: &str, color: &str) {
    println!("  {}{}:{} {}{}{}", GRAY, key, RESET, color, value, RESET);
}

// Progress bar
pub fn progress_bar(progress: f64, width: usize) -> String {
    let filled = (progress * width as f64) as usize;
    let empty = width - filled;

    format!(
        "{}{}{}{}{}",
        GREEN,
        "█".repeat(filled),
        GRAY,
        "░".repeat(empty),
        RESET
    )
}

// Box drawing
pub fn print_box(title: &str, content: &[&str]) {
    let max_len = content
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(0)
        .max(title.len());
    let width = max_len + 4;

    println!("  {}╭{}╮{}", GRAY, "─".repeat(width), RESET);
    println!(
        "  {}│{} {}{}{} {}{}│{}",
        GRAY,
        RESET,
        BOLD,
        title,
        RESET,
        " ".repeat(width - title.len() - 1),
        GRAY,
        RESET
    );
    println!("  {}├{}┤{}", GRAY, "─".repeat(width), RESET);

    for line in content {
        println!(
            "  {}│{} {} {}{}│{}",
            GRAY,
            RESET,
            line,
            " ".repeat(width - line.len() - 1),
            GRAY,
            RESET
        );
    }

    println!("  {}╰{}╯{}", GRAY, "─".repeat(width), RESET);
}

// Spinner frames
pub const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_frame(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}
