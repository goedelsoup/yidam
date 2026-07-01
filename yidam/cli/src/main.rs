use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    /// Bundle ontology, corpus, skills, decisions, and vector index into .yidam/bundle.yiz
    Bundle,
    /// Extract embedding text from corpus instances to .yidam/embeddings/
    Embed,
    /// Build LanceDB vector index from embeddings and export Arrow IPC for the web shell
    #[command(name = "index-build")]
    IndexBuild,
    /// Copy the yidam template into TARGET and initialise a fresh git repo
    Clone {
        /// Directory to create (must not already exist)
        target: PathBuf,
    },
    /// Overlay yidam infrastructure onto an existing git repo at TARGET
    Overlay {
        /// Root of an existing git repository
        target: PathBuf,
        /// Scan the repo's git history to accumulate knowledge and decisions into .yidam/
        #[arg(long)]
        backfill: bool,
        /// Only scan history reachable from HEAD but not from this ref (e.g. v1.0.0, main~100)
        #[arg(long, value_name = "REF", requires = "backfill")]
        backfill_ref: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
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
        Command::Embed => yidam::embed(),
        Command::IndexBuild => yidam::index_build().await,
        Command::Clone { target } => yidam::clone(&target),
        Command::Overlay { target, backfill, backfill_ref } => {
            yidam::overlay(&target, backfill, backfill_ref.as_deref())
        }
    }
}
