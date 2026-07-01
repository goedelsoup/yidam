use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "harness", about = "yidam bootstrap test harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a scenario through the bootstrap and evaluate the result
    Run {
        /// Path to the scenario file
        #[arg(long)]
        scenario: std::path::PathBuf,

        /// Claude model to use for the bootstrap agent
        #[arg(long, default_value = "claude-sonnet-4-6")]
        model: String,

        /// Where to write the result snapshot
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Run structural checks against an existing result directory
    Check {
        /// Path to the result directory
        #[arg(long)]
        result: std::path::PathBuf,
    },
    /// Compare two result snapshots and report regressions
    Diff {
        /// Path to the baseline result
        #[arg(long)]
        baseline: std::path::PathBuf,

        /// Path to the candidate result
        #[arg(long)]
        candidate: std::path::PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            scenario,
            model,
            output,
        } => yidam_harness::run(scenario, model, output),
        Command::Check { result } => yidam_harness::check(result),
        Command::Diff {
            baseline,
            candidate,
        } => yidam_harness::diff(baseline, candidate),
    }
}
