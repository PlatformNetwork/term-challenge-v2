use clap::Parser;
use term_challenge_lib::{ChallengeId, Hotkey};

#[derive(Parser)]
#[command(name = "term-sudo", about = "Term Challenge admin CLI")]
struct Cli {
    #[arg(long)]
    challenge_id: String,

    #[arg(long)]
    hotkey: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Status,
    ResetEpoch {
        #[arg(long)]
        epoch: u64,
    },
}

fn main() {
    let cli = Cli::parse();

    let challenge_id = ChallengeId::from_str(&cli.challenge_id).unwrap_or_else(|| {
        eprintln!("Invalid challenge ID: {}", cli.challenge_id);
        std::process::exit(1);
    });

    let hotkey = Hotkey::from_ss58(&cli.hotkey).unwrap_or_else(|| {
        eprintln!("Invalid SS58 hotkey: {}", cli.hotkey);
        std::process::exit(1);
    });

    match cli.command {
        Command::Status => {
            println!("Challenge: {}", challenge_id);
            println!("Hotkey: {:?}", hotkey);
            println!("Status: OK");
        }
        Command::ResetEpoch { epoch } => {
            println!("Challenge: {}", challenge_id);
            println!("Hotkey: {:?}", hotkey);
            println!("Reset epoch: {}", epoch);
        }
    }
}
