use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "yidam",
    about = "Corpus analysis and index CLI for yidam-derived repositories"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Status,
    #[command(name = "open-questions")]
    OpenQuestions,
    #[command(name = "corpus-index")]
    CorpusIndex,
    #[command(name = "index-status")]
    IndexStatus,
    #[command(name = "catalog-audit")]
    CatalogAudit,
    #[command(name = "agents-index")]
    AgentsIndex,
    #[command(name = "skills-index")]
    SkillsIndex,
    #[command(name = "crates-index")]
    CratesIndex,
    #[command(name = "packages-index")]
    PackagesIndex,
    #[command(name = "bundle-status")]
    BundleStatus,
    #[command(name = "graph-check")]
    GraphCheck,
    #[command(name = "decisions-log")]
    DecisionsLog,
    /// Bundle ontology, corpus, skills, and decisions into .yidam/CONTEXT.md
    Bundle,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Status => yidam::status(),
        Command::OpenQuestions => yidam::open_questions(),
        Command::CorpusIndex => yidam::corpus_index(),
        Command::IndexStatus => yidam::index_status(),
        Command::CatalogAudit => yidam::catalog_audit(),
        Command::AgentsIndex => yidam::agents_index(),
        Command::SkillsIndex => yidam::skills_index(),
        Command::CratesIndex => yidam::crates_index(),
        Command::PackagesIndex => yidam::packages_index(),
        Command::BundleStatus => yidam::bundle_status(),
        Command::GraphCheck => yidam::graph_check(),
        Command::DecisionsLog => yidam::decisions_log(),
        Command::Bundle => yidam::bundle(),
    }
}
