use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;

mod scan;
mod timeline;

#[cfg(test)]
mod test_helpers;

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
        search_string: String,
    },
    Timeline {
        directory: String,
        search_string: String,
        #[arg(long, short, default_value = "timeline.jsonl")]
        output: String,
        #[arg(long, short = 'i')]
        case_insensitive: bool,
        #[arg(long)]
        regex: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { directory, search_string } => {
            let resolved_path = env::current_dir()?.join(&directory).canonicalize()?;

            println!("Scanning directory: {}", resolved_path.display());
            println!("Searching for: '{}'", search_string);
            scan::scan(&resolved_path, &search_string)?;
        }
        Commands::Timeline { directory, search_string, output, case_insensitive, regex } => {
            let resolved_path = env::current_dir()?.join(&directory).canonicalize()?;

            timeline::timeline(&resolved_path, &search_string, &output, case_insensitive, regex)?;
        }
    }

    Ok(())
}
