//! The steps every corpus's workflow runs and no manifest declares: the catalog's connectors.
//!
//! `catalog-fetch`, `catalog-extract` and `catalog-reconcile` are operational — `refresh:`,
//! `extract:`, `reconcile:` — and until now they ran only on a laptop, committing on the
//! current branch. #475's 2026-09-30 amendment puts them in the workflow, so a catalog clock
//! that `due` finds overdue is one the cluster that admitted the run also acts on.
//!
//! # Why a frozen set, and not manifest entries
//!
//! #460 decision 9: connectors stay unrunnable in `run`. A manifest that declared
//! `kind = "connector"` would be asking this binary to invoke an argv that reads the world,
//! and `Kind::unrunnable_because` refuses that everywhere it is asked. These three are not
//! declared, they are compiled in: the binary invokes its own code, in-process, through
//! [`crate::cmd::catalog`]'s detached writer. So the refusal stands for every connector a
//! corpus could write, and the only connectors a pod runs are the ones this constant names.
//!
//! # One constant, read by the pod and by the generator
//!
//! [`BUILTINS`] is the whole set. `cluster workflow` emits a step for each, in this order,
//! ahead of the manifest's plan; `cluster step` and `cluster land` resolve a name here before
//! they ask the manifest. The tests derive from it too, so a fourth built-in added here is a
//! step generated, runnable and landable without a list elsewhere to extend.
//!
//! # A built-in is a capability like any other, to everything after the invocation
//!
//! [`Builtin::capability`] is the declaration it would have if a manifest could write it.
//! The receipt's input state, the lander's read set and its re-parent check all take a
//! [`Capability`] and need nothing else, so a built-in's commit is landed by the code that
//! lands a calculator's — the lander does not know which kind of step built it.

use std::path::Path;

use anyhow::{bail, Result};

use crate::cmd::catalog::{self, Detached, Writer};
use crate::cmd::run::manifest::{Capability, Kind, Manifest, Run};

/// What a built-in runs: the command's library entry, not its argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Fetch,
    Extract,
    Reconcile,
}

/// One compiled-in step.
#[derive(Debug)]
pub struct Builtin {
    /// The step's name in the workflow, and the command a person runs for the same thing.
    pub name: &'static str,
    /// The verb its commits carry. Operational, every one: a built-in that authored an
    /// epistemic commit would be the invariant's own exception, compiled in.
    pub verb: &'static str,
    /// What its input state is computed over, beside the manifest and config.
    pub reads: &'static [&'static str],
    pub op: Op,
}

/// Everything a catalog command writes.
const CATALOG: &str = ".yidam/catalog/**";

/// The built-in steps, in the order a workflow runs them.
///
/// Fetch first, because the other two read what it records. Extract reads the PDFs a fetch
/// recorded — from this machine's vault cache, which on a pod is only what that pod fetched,
/// so on a cluster it records what it can and skips the rest, saying why. Reconcile last:
/// it compares `used-by` against the corpus's citations, and nothing before it writes those.
pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "catalog-fetch",
        verb: "refresh",
        reads: &[CATALOG],
        op: Op::Fetch,
    },
    Builtin {
        name: "catalog-extract",
        verb: "extract",
        reads: &[CATALOG],
        op: Op::Extract,
    },
    Builtin {
        name: "catalog-reconcile",
        verb: "reconcile",
        reads: &[CATALOG, ".yidam/corpus/**"],
        op: Op::Reconcile,
    },
];

impl Op {
    /// Whether the step reads from outside the corpus, so its pod needs the internet (#1232).
    ///
    /// Fetch reads each entry's source wherever it is. Extract reads only the vault cache
    /// on its own pod, and reconcile reads the clone, so both reach the vault and no further.
    pub fn reaches_the_world(self) -> bool {
        match self {
            Self::Fetch => true,
            Self::Extract | Self::Reconcile => false,
        }
    }
}

/// The built-in named `name`, if there is one.
pub(crate) fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

impl Builtin {
    /// The declaration this step would have, if a manifest could declare it.
    pub(crate) fn capability(&self) -> Capability {
        Capability {
            kind: Kind::Connector,
            run: Run::Argv(vec!["yidam".to_string(), self.name.to_string()]),
            reads: self.reads.iter().map(|g| g.to_string()).collect(),
            writes: vec![CATALOG.to_string()],
            verb: self.verb.to_string(),
            after: Vec::new(),
            ageing_days: None,
            cluster: None,
        }
    }

    /// Run the command against the clone at `root`, committing through `writer`.
    pub(crate) fn invoke(&self, root: &Path, writer: &mut Writer) -> Result<()> {
        let format = crate::report::Format::Text;
        match self.op {
            Op::Fetch => catalog::fetch_in(
                root,
                &catalog::FetchOptions {
                    entry: None,
                    location: None,
                    bind: Vec::new(),
                    dry_run: false,
                    format,
                },
                writer,
            )
            .map(drop),
            Op::Extract => catalog::extract_in(
                root,
                &catalog::ExtractOptions {
                    entry: None,
                    dry_run: false,
                    format,
                },
                writer,
            )
            .map(drop),
            Op::Reconcile => catalog::reconcile_in(
                root,
                &catalog::ReconcileOptions {
                    entry: None,
                    dry_run: false,
                    format,
                },
                writer,
            )
            .map(drop),
        }
    }

    /// Run it detached on `input`, and return the writer for its chain.
    pub(crate) fn invoke_detached(&self, root: &Path, input: &str) -> Result<Detached> {
        let mut writer = Writer::Detached(Detached::new(root, input)?);
        self.invoke(root, &mut writer)?;
        let Writer::Detached(d) = writer else {
            unreachable!("constructed detached above")
        };
        Ok(d)
    }
}

/// The capability `name` stands for: a built-in's, or the manifest's declaration.
///
/// A built-in first, so a step and its lander agree on what a name means without either
/// reading a manifest that has no say in it. A manifest declaring a built-in's name is
/// refused where the workflow is generated — see [`refuse_shadowing`].
pub(crate) fn capability(root: &Path, name: &str) -> Result<Capability> {
    match find(name) {
        Some(b) => Ok(b.capability()),
        None => Ok(Manifest::load(root)?.get(name)?.clone()),
    }
}

/// Refuse a manifest that declares a capability under a built-in's name.
///
/// Either reading of such a manifest is wrong: running the built-in ignores a declaration
/// the corpus wrote, and running the declaration would run a connector argv.
pub(crate) fn refuse_shadowing(declared: &[&str]) -> Result<()> {
    if let Some(b) = BUILTINS.iter().find(|b| declared.contains(&b.name)) {
        bail!(
            "the manifest declares `{}`, which is the name of a step every workflow runs \
             already ({} `{}:` commits).\n  Rename the capability.",
            b.name,
            b.verb,
            b.verb
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in authors operational commits: the lander would land an epistemic one on a
    /// proposal branch, but a compiled-in step that proposed would be the invariant's
    /// exception written into the binary.
    #[test]
    fn every_builtin_is_operational_and_a_connector() {
        for b in BUILTINS {
            let cap = b.capability();
            assert!(
                yidam_core::git::OPERATIONAL_VERBS.contains(&b.verb),
                "{} authors `{}:`",
                b.name,
                b.verb
            );
            assert_eq!(super::super::class_of(cap.route()), "operational");
            assert_eq!(cap.kind, Kind::Connector, "{}", b.name);
            assert!(
                cap.kind.unrunnable_because().is_some(),
                "a connector is still refused by `run`; the built-in path is what runs these"
            );
            assert_eq!(cap.writes, [CATALOG]);
        }
    }

    #[test]
    fn names_are_unique_and_found() {
        for b in BUILTINS {
            assert_eq!(find(b.name).map(|f| f.op), Some(b.op));
        }
        let mut names: Vec<&str> = BUILTINS.iter().map(|b| b.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), BUILTINS.len());
        assert!(find("travel-tier").is_none());
    }

    #[test]
    fn a_manifest_shadowing_a_builtin_is_refused() {
        for b in BUILTINS {
            let err = refuse_shadowing(&["travel-tier", b.name]).unwrap_err();
            assert!(err.to_string().contains(b.name), "{err}");
        }
        refuse_shadowing(&["travel-tier"]).unwrap();
    }
}
