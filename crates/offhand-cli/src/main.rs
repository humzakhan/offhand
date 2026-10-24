//! `offhand` command-line entry point.

use clap::Parser;

/// Offhand: an offline-first computer-use agent.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// The task to perform.
    task: String,
}

fn main() {
    let cli = Cli::parse();
    println!("offhand: not implemented yet: {}", cli.task);
}
