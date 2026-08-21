mod authorship;
mod claims;
mod cmd;
#[cfg(feature = "index")]
mod config;
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

#[cfg(feature = "tonpa")]
pub use cmd::tonpa;
pub use cmd::{
    agents_index, backfill, bundle, bundle_status, catalog_audit, clone, corpus_index,
    crates_index, decisions_log, diff_corpus, embed, export, graph, graph_check, index_status,
    index_verify, lint, list_formats, log, neighbors, open_questions, overlay, packages_index,
    phases, regen, rename, run_export, samudaya_audit, sangha, schema, serve_lsp, skills_index,
    status, vocabulary, EmbedOptions, ExportFormat, ExportOptions, LintOptions, LogFilter,
    RdfFormat,
};
#[cfg(feature = "index")]
pub use cmd::{index_build, serve_mcp};
pub use paths::{running_binary_note, warn_if_shadowed};
pub use report::Format;
