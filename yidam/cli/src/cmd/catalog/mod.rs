//! The catalog: what a corpus draws on, where it can be reached, and what it has obtained.
//!
//! Three surfaces over one set of files.
//!
//! - [`audit`] reports what the catalog holds and who cites it. It reads and writes a REGEN
//!   block, and is the oldest of the three.
//! - [`fetch`] follows a declared address, files the bytes under their digest, and records
//!   what it kept — the `extract:`/`refresh:` row of #460's table.
//! - [`reconcile`] brings a hand-maintained `used-by` list back into agreement with the
//!   citations, which are authoritative — the `reconcile:` row of the same table.
//!
//! The last two write commits, which nothing under `cmd/` but `propose` did before. What
//! licenses them to is RFC-0026's invariant, and [`commit::author`] enforces it against
//! `classify_commit` rather than trusting the call sites: **a run authors operational commits
//! directly, and every epistemic commit it produces goes to a proposal branch.**

pub(crate) mod audit;
mod commit;
mod fetch;
mod location;
mod reconcile;
mod record;
mod transport;

pub use audit::catalog_audit;
pub use fetch::{fetch, FetchOptions};
pub use location::parse_binding;
pub use reconcile::{reconcile, ReconcileOptions};
