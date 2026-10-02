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
//! licenses them to is RFC-0026's invariant, and [`crate::cmd::operational::author`] enforces it against
//! `classify_commit` rather than trusting the call sites: **a run authors operational commits
//! directly, and every epistemic commit it produces goes to a proposal branch.**

pub(crate) mod audit;
mod extract;
mod fetch;
pub(crate) mod location;
mod reconcile;
mod record;
pub(crate) mod superseded;
mod transport;

pub(crate) use crate::cmd::operational::{refuse_epistemic, Detached, Writer};
pub use audit::catalog_audit;

/// The author every catalog commit carries.
///
/// Author and committer are separated for `propose`'s reason, which applies unchanged: the
/// tool performed the fetch, a person ran it, and recording both is a true account without
/// borrowing an identity. It matters more here than there — an operational commit lands on
/// the branch rather than on a proposal nobody has merged yet, so the record of what wrote it
/// is the only thing distinguishing it from a person's own work.
pub(crate) const AUTHOR_NAME: &str = "yidam catalog";
pub(crate) const AUTHOR_EMAIL: &str = "catalog@yidam";

/// [`AUTHOR_NAME`] and [`AUTHOR_EMAIL`], as the writer takes them.
pub(crate) const WHO: crate::cmd::operational::Who<'static> = (AUTHOR_NAME, AUTHOR_EMAIL);
pub(crate) use extract::extract_in;
pub use extract::{extract, ExtractOptions};
pub(crate) use fetch::fetch_in;
pub use fetch::{fetch, FetchOptions};
pub use location::parse_binding;
pub(crate) use reconcile::reconcile_in;
pub use reconcile::{reconcile, ReconcileOptions};
