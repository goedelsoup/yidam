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

        /// Model to score the result with. Omit to run the structural checks only —
        /// the judge costs a second model call, and a run being iterated on rarely needs it.
        #[arg(long)]
        judge_model: Option<String>,

        /// Score with the default judge (see `judge::DEFAULT_JUDGE_MODEL`).
        #[arg(long, conflicts_with = "judge_model")]
        judge: bool,

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
    /// Score a captured result against the quality criteria, without re-running the bootstrap
    Judge {
        /// Path to the result directory
        #[arg(long)]
        result: std::path::PathBuf,

        /// The scenario the result was produced from — the judge needs its reference
        /// description to score Q7
        #[arg(long)]
        scenario: std::path::PathBuf,

        /// Model to score with
        #[arg(long, default_value = yidam_harness::judge::DEFAULT_JUDGE_MODEL)]
        model: String,
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
            judge_model,
            judge,
            output,
        } => {
            let judge_model = judge_model
                .or_else(|| judge.then(|| yidam_harness::judge::DEFAULT_JUDGE_MODEL.to_string()));
            yidam_harness::run(scenario, model, judge_model, output)
        }
        Command::Check { result } => yidam_harness::check(result),
        Command::Judge {
            result,
            scenario,
            model,
        } => yidam_harness::judge_result(result, scenario, model),
        Command::Diff {
            baseline,
            candidate,
        } => yidam_harness::diff(baseline, candidate),
    }
}
