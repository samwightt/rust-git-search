use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;

mod scan;

#[derive(Parser)]
#[command(name = "git-history")]
#[command(about = "A tool for analyzing git history")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Scan { directory: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { directory } => {
            let current_dir = env::current_dir()?;
            let target_path = current_dir.join(&directory);
            let resolved_path = target_path.canonicalize()?;

            println!("Scanning directory: {}", resolved_path.display());
            scan::scan(&resolved_path)?;
        }
    }

    Ok(())
}
