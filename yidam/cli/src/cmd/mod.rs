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
mod index_build;
mod lint;
mod overlay;
mod phases;
pub(crate) mod registry;
mod samudaya_audit;
mod status;
pub mod tonpa;
mod web;

pub use backfill::backfill;
pub use build::{crates_index, packages_index};
pub use bundle::bundle;
pub use catalog::catalog_audit;
pub use clone::clone;
pub use corpus::{corpus_index, graph_check, open_questions};
pub use decisions::decisions_log;
pub use diff::diff_corpus;
pub use embed::embed;
pub use export::{export, list_formats, run_export, ExportFormat};
pub use index_build::index_build;
pub use lint::lint;
pub use overlay::overlay;
pub use phases::phases;
pub use registry::{agents_index, skills_index};
pub use samudaya_audit::samudaya_audit;
pub use status::{index_status, status};
pub use web::bundle_status;

fn has_open_claim(text: &str) -> bool {
    text.contains("[open]")
}
