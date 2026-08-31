mod authorship;
mod claims;
mod cmd;
// Not feature-gated. `.yidam/config.toml` carries `[lint] escalate_after`, and `lint` is
// in the light `reports` binary — gating this on `index` made the corpus's own declaration
// unreadable in the build most repositories actually install.
mod config;
pub mod dates;
pub mod deps;
pub mod embed_config;
#[cfg(feature = "vector-read")]
pub mod embedding;
mod git;
mod markdown;
mod parse;
mod paths;
/// The rules a repository writes about itself (RFC-0024). Public so the equivalence
/// tests can hold it beside the Rust guards it re-expresses.
pub mod policy;
pub mod provenance;
mod regen;
pub mod report;
mod retrieval;
pub mod universal;
// Ungated, and that is the design rather than an oversight. The vault's addressing, cache
// and `file://` backend need only `sha2`, `hex` and std — all base dependencies — so the
// light build every derived repository installs can hash, cache, verify and read a vault
// on a mounted archive. The transport is what a feature will buy, which is the split
// `deps.rs` already arrived at for `tonpa`.
pub mod vault;
mod walk;

pub mod model;

#[cfg(feature = "index")]
pub use cmd::index_build;
#[cfg(feature = "tonpa")]
pub use cmd::tonpa;
pub use cmd::{
    agents_index, backfill, bench, bundle, bundle_status, catalog_audit, check_diff, clone,
    corpus_index, crates_index, decisions_log, diff_corpus, doctor, due, embed, estimate, export,
    graph, graph_check, index_status, index_verify, lint, list_formats, log, migrate, neighbors,
    open_questions, overlay, pack, packages_index, parse_bench_goals, phases, propose, query,
    regen, rename, replay, run_export, run_policy, run_vault, samudaya_audit, sangha, schema,
    serve_lsp, serve_mcp, skills_index, status, vault_status, vocabulary, BenchGoal, BenchGoalSet,
    EmbedOptions, ExportFormat, ExportOptions, LintOptions, LogFilter, MigrateOperation,
    PolicyCommand, ProposeOptions, RdfFormat, VaultCommand,
};
/// The per-class schemas compiled from a repository's own ontology.
///
/// Exposed for the test suite that holds the compiler and the gate to the same corpus. It
/// takes a root rather than resolving one so it can run against a materialized fixture.
pub fn class_schemas_at(root: &std::path::Path) -> Vec<(String, String, serde_json::Value)> {
    cmd::class_schemas(root)
}

pub use cmd::query::Scope as QueryScope;
pub use cmd::query::DEFAULT_ANCHOR_K as QUERY_DEFAULT_ANCHOR_K;
pub use cmd::query::DEFAULT_LIMIT as QUERY_DEFAULT_LIMIT;
pub use paths::{running_binary_note, warn_if_shadowed};
pub use report::Format;
