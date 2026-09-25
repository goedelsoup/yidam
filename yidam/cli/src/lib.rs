//! Corpus analysis for yidam-derived repositories.
//!
//! # What is an API here, and what is not
//!
//! This crate is published so that `yidam` can be installed with `cargo install`. The
//! surface splits in two, and the split is load-bearing rather than incidental (#926):
//!
//! - **The modules below are the API.** [`report`] is the machine-readable contract
//!   (RFC-0016), [`retrieval`] the vocabulary a search answers in, [`policy`] and [`kuten`]
//!   what a repository declares about itself, [`vault`] and [`s3vectors`] its artifacts and
//!   its remote index. They compute and return; none of them writes to stdout.
//! - **The `#[doc(hidden)]` re-exports are the binary's entry points.** They exist because
//!   `main.rs` is a separate compilation unit and cannot reach a private module, not because
//!   anyone should call them. Each one renders a report to stdout in whichever
//!   [`report::Format`] it was handed — that is its whole job — and the gating ones return
//!   [`report::GateFailed`] rather than a value you can inspect.
//!
//! Nothing outside this repository is invited to call the second set, and nothing inside it
//! does either: `main.rs` dispatches to them and twenty-two test files use the first set.
//! They are hidden rather than private because the binary needs them, and documented as
//! hidden rather than left looking like an API because they behave like a program.
//!
//! # They no longer take your process
//!
//! Until #926, thirteen of those commands called `std::process::exit(1)` when their gate
//! failed. They now return [`report::GateFailed`] and the binary owns the exit. The exit
//! code did not change; what changed is that a caller gets its process back.

mod authorship;
mod claims;
mod cmd;
// Not feature-gated. `.yidam/config.toml` carries `[lint] escalate_after`, and `lint` is
// in the light `reports` binary — gating this on `index` made the corpus's own declaration
// unreadable in the build most repositories actually install.
mod config;
/// What a corpus is made of (#924). Private, like [`cmd`]: nothing outside the crate reads
/// a [`corpus::Node`] today, and the point of the module is which way the dependency runs —
/// the library owns the corpus model and `lint` consumes it, rather than the reverse.
mod corpus;
pub mod dates;
pub mod deps;
pub mod embed_config;
#[cfg(feature = "vector-read")]
pub mod embedding;
mod findings;
pub mod git;
/// What a corpus declares its practice is aimed at (RFC-0028). Public so the guards over
/// the shipped profiles can parse them the way the binary does, rather than a second way.
pub mod kuten;
mod markdown;
mod parse;
mod paths;
/// The rules a repository writes about itself (RFC-0024). Public so the equivalence
/// tests can hold it beside the Rust guards it re-expresses.
pub mod policy;
mod prose;
pub mod provenance;
mod regen;
pub mod report;
/// Which of a node's properties belong in its embedding although they are not prose (#717).
/// Named for the question rather than for [`retrieval`], which answers a different one: that
/// module is how a search is *run*, this one is what a node is made retrievable *on*.
mod retrievable;
/// How text becomes entry nodes. Public for the reason [`vault`] is: the vocabulary a search
/// answers in — [`retrieval::Hit`] and [`retrieval::Filter`] — is shared by a local scan and a
/// remote vector index, and [`s3vectors`] has to be able to speak it.
pub mod retrieval;
/// What a contribution is scored on. Public for the reason [`kuten`] is: the guard that holds
/// the declared criteria to the implemented ones has to be able to ask both sides.
pub mod score;
pub mod universal;
// Ungated, and that is the design rather than an oversight. The vault's addressing, cache
// and `file://` backend need only `sha2`, `hex` and std — all base dependencies — so the
// light build every derived repository installs can hash, cache, verify and read a vault
// on a mounted archive. The transport is what a feature will buy, which is the split
// `deps.rs` already arrived at for `tonpa`.
/// The S3 Vectors transport (RFC-0033). Ungated for the reason [`vault`] gives about its own
/// addressing: everything that decides what would be sent and what came back needs only
/// dependencies the light build already has, so every pull request compiles and tests it. Only
/// the I/O is behind `s3-vectors`, and only embedding a query is behind `vector-read`.
pub mod s3vectors;
pub mod vault;
mod walk;

/// The one tokio runtime (#930). Compiled for exactly the features that pull `tokio` —
/// `tests/light_build.rs` derives that set from Cargo.toml and holds this list to it.
#[cfg(any(
    feature = "index",
    feature = "tonpa",
    feature = "vault-s3",
    feature = "s3-vectors",
    feature = "catalog-fetch",
    feature = "serve-http",
))]
pub mod runtime;

pub mod model;

#[cfg(feature = "index")]
#[doc(hidden)]
pub use cmd::index_build;
#[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
#[doc(hidden)]
pub use cmd::index_push;
/// Every REGEN generator `yidam regen` runs. Public so `help.rs` — which lives in the binary,
/// a separate compilation unit — can assert that each one carries the write marker.
#[doc(hidden)]
pub use cmd::regen_generator_names;
#[cfg(feature = "tonpa")]
#[doc(hidden)]
pub use cmd::tonpa;
/// Top-level paths `yidam clone` leaves behind. Public so the guard that holds the template
/// root to the bootstrap protocol can ask the copy what it excludes, rather than restating
/// the list a third time.
#[doc(hidden)]
pub use cmd::NOT_INHERITED;
/// The seed kinds `yidam samudaya-audit` accepts. See [`samudaya_seed_kind`].
#[doc(hidden)]
pub use cmd::SAMUDAYA_KINDS;
/// The directories that make a checkout the template. Public for the same reason as
/// [`NOT_INHERITED`]: the guard over `yidam clone` builds a fixture that has to be a
/// template, and a fixture restating the predicate would go on satisfying itself.
#[doc(hidden)]
pub use cmd::TEMPLATE_MARKERS;
#[doc(hidden)]
pub use cmd::{
    agents_index, backfill, bench, bundle, bundle_status, catalog_audit, catalog_fetch,
    catalog_reconcile, check_diff, citation_label_not_cited, citation_range_stated_twice, clone,
    cohort, collect_line_citations, corpus_index, crates_index, cycle, dead_line_citation,
    decisions_log, diff_corpus, doctor, due, embed, estimate, export, graph, graph_check,
    index_status, index_verify, label_range, label_symbols, lint, list_formats, log, migrate,
    neighbors, open_questions, overlay, pack, packages_index, parse_bench_goals, parse_binding,
    phases, propose, query, regen, relocate, rename, replay, retrieve, run_capability, run_export,
    run_kuten, run_phase, run_policy, run_practice, run_score, run_vault, samudaya_audit, sangha,
    schema, serve_lsp, serve_mcp, skills_index, slid_line_citation, status,
    unverified_line_citation, vault_status, vocabulary, BenchGoal, BenchGoalSet, CohortOptions,
    EmbedOptions, ExportFormat, ExportOptions, FetchOptions, KutenCommand, LineCitation,
    LineFragment, LintCheck, LintOptions, LintViolation, LogFilter, MigrateOperation, PhaseCommand,
    PolicyCommand, PreludeNorm, ProposeOptions, RdfFormat, ReconcileOptions, Relocation,
    RetrieveOptions, RunOptions, VaultCommand, COMMIT_KINDS, LINT_SEVERITIES, PRELUDE_NORMS,
};

/// The remote transport (#423). Gated because the feature is what pulls the server, and
/// `--no-default-features --features reports` has no business linking one — see the note on
/// `serve-http` in Cargo.toml for why it is nonetheless in the default set.
#[cfg(feature = "serve-http")]
#[doc(hidden)]
pub use cmd::serve_mcp_http;

/// The `kind` a samudaya seed file declares, or `None` where it declares none.
///
/// Exposed for `tests/samudaya_examples.rs`, which validates the domain seed sets under
/// `samudaya/examples/`. `samudaya-audit` deliberately does not read them — it skips
/// `examples/`, and that skip is the whole reason those sets are inert rather than live
/// seeds of this repository — so the test is the only thing that can hold them to the
/// vocabulary. It goes through the same parse the command uses; a second reading of the
/// frontmatter would be a second opinion about what a seed file is.
#[doc(hidden)]
pub fn samudaya_seed_kind(text: &str) -> Option<String> {
    parse::parse_samudaya_seed(text).kind
}

/// The per-class schemas compiled from a repository's own ontology.
///
/// Exposed for the test suite that holds the compiler and the gate to the same corpus. It
/// takes a root rather than resolving one so it can run against a materialized fixture.
#[doc(hidden)]
pub fn class_schemas_at(root: &std::path::Path) -> Vec<(String, String, serde_json::Value)> {
    cmd::class_schemas(root)
}

#[doc(hidden)]
pub use cmd::query::Scope as QueryScope;
#[doc(hidden)]
pub use cmd::query::DEFAULT_ANCHOR_K as QUERY_DEFAULT_ANCHOR_K;
#[doc(hidden)]
pub use cmd::query::DEFAULT_LIMIT as QUERY_DEFAULT_LIMIT;
#[doc(hidden)]
pub use paths::{running_binary_note, warn_if_shadowed};
pub use report::Format;
