mod authorship;
mod claims;
mod cmd;
// Not feature-gated. `.yidam/config.toml` carries `[lint] escalate_after`, and `lint` is
// in the light `reports` binary — gating this on `index` made the corpus's own declaration
// unreadable in the build most repositories actually install.
mod config;
pub mod deps;
pub mod embed_config;
mod git;
mod markdown;
mod parse;
mod paths;
pub mod provenance;
mod regen;
pub mod report;
mod walk;

pub mod model;

#[cfg(feature = "index")]
pub use cmd::index_build;
#[cfg(feature = "tonpa")]
pub use cmd::tonpa;
pub use cmd::{
    agents_index, backfill, bundle, bundle_status, catalog_audit, clone, corpus_index,
    crates_index, decisions_log, diff_corpus, doctor, embed, export, graph, graph_check,
    index_status, index_verify, lint, list_formats, log, neighbors, open_questions, overlay,
    packages_index, phases, regen, rename, replay, run_export, samudaya_audit, sangha, schema,
    serve_lsp, serve_mcp, skills_index, status, vocabulary, EmbedOptions, ExportFormat,
    ExportOptions, LintOptions, LogFilter, RdfFormat,
};
/// The per-class schemas compiled from a repository's own ontology.
///
/// Exposed for the test suite that holds the compiler and the gate to the same corpus. It
/// takes a root rather than resolving one so it can run against a materialized fixture.
pub fn class_schemas_at(root: &std::path::Path) -> Vec<(String, String, serde_json::Value)> {
    cmd::class_schemas(root)
}

pub use paths::{running_binary_note, warn_if_shadowed};
pub use report::Format;
