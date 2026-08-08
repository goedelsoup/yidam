mod claims;
mod cmd;
#[cfg(feature = "index")]
mod config;
pub mod embed_config;
mod git;
mod parse;
mod paths;
pub mod provenance;
mod regen;
mod walk;

pub mod model;

#[cfg(feature = "tonpa")]
pub use cmd::tonpa;
pub use cmd::{
    agents_index, backfill, bundle, bundle_status, catalog_audit, clone, corpus_index,
    crates_index, decisions_log, diff_corpus, embed, export, graph_check, index_status, lint,
    list_formats, open_questions, overlay, packages_index, phases, run_export, samudaya_audit,
    schema, skills_index, status, ExportFormat, ExportOptions, LintOptions, RdfFormat,
};
#[cfg(feature = "index")]
pub use cmd::{index_build, serve_mcp};
