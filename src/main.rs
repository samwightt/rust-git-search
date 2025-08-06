use clap::{Parser, Subcommand};
use std::env;

#[derive(Parser)]
#[command(name = "git-history")]
#[command(about = "A tool for analyzing git history")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Scan {
        directory: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { directory } => {
            let current_dir = env::current_dir().expect("Failed to get current directory");
            let target_path = current_dir.join(&directory);
            let resolved_path = target_path.canonicalize()
                .unwrap_or_else(|_| panic!("Failed to resolve path: {}", directory));
            
            println!("Scanning directory: {}", resolved_path.display());
        }
    }
}
