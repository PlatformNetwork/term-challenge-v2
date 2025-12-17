//! Agent runner for Term Challenge.

use std::io::{self, Read, Write};
use crate::{Agent, Request, Response};

/// Log to stderr.
fn log(msg: &str) {
    eprintln!("[agent] {}", msg);
}

/// Run an agent in the Term Challenge harness.
///
/// Reads request from stdin, calls agent.solve(), writes response to stdout.
///
/// ```rust,no_run
/// use term_sdk::{Agent, Request, Response, run};
///
/// struct MyAgent;
///
/// impl Agent for MyAgent {
///     fn solve(&mut self, req: &Request) -> Response {
///         Response::cmd("ls")
///     }
/// }
///
/// fn main() {
///     run(&mut MyAgent);
/// }
/// ```
pub fn run(agent: &mut impl Agent) {
    // Setup
    agent.setup();
    
    // Read input
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        log(&format!("Failed to read stdin: {}", e));
        println!("{}", Response::done().to_json());
        return;
    }
    
    let input = input.trim();
    if input.is_empty() {
        log("No input received");
        println!("{}", Response::done().to_json());
        return;
    }
    
    // Parse request
    let request = match Request::parse(input) {
        Ok(req) => req,
        Err(e) => {
            log(&format!("Invalid JSON: {}", e));
            println!("{}", Response::done().to_json());
            return;
        }
    };
    
    log(&format!("Step {}: {}...", request.step, &request.instruction.chars().take(50).collect::<String>()));
    
    // Solve
    let response = agent.solve(&request);
    
    // Output
    println!("{}", response.to_json());
    io::stdout().flush().ok();
    
    // Cleanup
    agent.cleanup();
}

/// Run agent in loop mode (for testing).
pub fn run_loop(agent: &mut impl Agent) {
    use std::io::BufRead;
    
    agent.setup();
    
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        let request = match Request::parse(line) {
            Ok(req) => req,
            Err(e) => {
                log(&format!("Invalid JSON: {}", e));
                println!("{}", Response::done().to_json());
                break;
            }
        };
        
        let response = agent.solve(&request);
        println!("{}", response.to_json());
        io::stdout().flush().ok();
        
        if response.task_complete {
            break;
        }
    }
    
    agent.cleanup();
}
