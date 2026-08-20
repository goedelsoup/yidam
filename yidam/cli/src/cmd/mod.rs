mod backfill;
mod build;
mod bundle;
mod catalog;
mod clone;
mod copy;
pub(crate) mod corpus;
pub(crate) mod decisions;
mod diff;
mod embed;
mod export;
mod export_graphml;
mod export_llms;
#[cfg(feature = "export-graph")]
mod export_rdf;
#[cfg(feature = "export-sqlite")]
mod export_sqlite;
mod export_web;
pub(crate) mod graph;
#[cfg(feature = "index")]
mod index_build;
mod index_verify;
mod lint;
mod log;
mod lsp;
mod overlay;
mod phases;
mod regen;
pub(crate) mod registry;
mod rename;
mod samudaya_audit;
mod sangha;
mod schema;
#[cfg(feature = "index")]
mod serve;
mod status;
#[cfg(feature = "tonpa")]
pub mod tonpa;
mod vocabulary;
mod web;

pub use backfill::backfill;
pub use build::{crates_index, packages_index};
pub use bundle::bundle;
pub use catalog::catalog_audit;
pub use clone::clone;
pub use corpus::{corpus_index, graph_check, open_questions};
pub use decisions::decisions_log;
pub use diff::diff_corpus;
pub use embed::{embed, EmbedOptions};
pub use export::{export, list_formats, run_export, ExportFormat, ExportOptions, RdfFormat};
pub use graph::{graph, neighbors};
#[cfg(feature = "index")]
pub use index_build::index_build;
pub use index_verify::index_verify;
pub use lint::{lint, Options as LintOptions};
pub use lsp::serve_lsp;

pub use log::{log, Filter as LogFilter};
pub use overlay::overlay;
pub use phases::phases;
pub use regen::regen;
pub use registry::{agents_index, skills_index};
pub use rename::rename;
pub use samudaya_audit::samudaya_audit;
pub use sangha::sangha;
pub use schema::schema;
#[cfg(feature = "index")]
pub use serve::serve_mcp;
pub use status::{index_status, status};
pub use vocabulary::vocabulary;
pub use web::bundle_status;

fn has_open_claim(text: &str) -> bool {
    text.contains("[open]")
}
