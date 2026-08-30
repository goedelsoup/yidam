//! Resolving `cites:` against the dependencies actually on disk — RFC-0019, #266.
//!
//! # Why this is the check that makes composition safe
//!
//! The local gate's whole value is that a link cannot rot silently. Without this, a citation
//! into a dependency is unverifiable in exactly the way a local link is not — which inverts
//! the property that makes the local graph trustworthy, at the boundary where it is hardest
//! to recover.
//!
//! # What can and cannot be checked here
//!
//! Not "the node exists at the pinned commit". There is nothing to resolve that against: a
//! `.yiz` bundle is a tarball of `corpus/`, `skills/` and `decisions/` — no history, no
//! object store. The only commit identity a fetched dependency carries is `manifest.yml`'s,
//! which is `head_commit_short` in the *producing* repository, whose length git chooses from
//! that repository's object count.
//!
//! So what is checked is what is knowable: **the node is in the bundle installed here, the
//! span still appears in it, and the bundle is the one the citation says it read.** Those are
//! three findings, not one, and their severities differ because their repairs do.
//!
//! # The light build, deliberately
//!
//! `--features tonpa` buys the network — resolving a source, fetching an archive, writing a
//! lock. None of that is wanted here, and derived-repo CI downloads a binary rather than
//! compiling one, so a check that needed the feature would be a check that never ran where it
//! matters. Everything below reads `.yidam/tonpa/<pkg>/` and `.yidam/tonpa.toml` off disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::checks::Node;
use super::model::{Check, Severity, Violation};
use crate::deps::{DependencyKind, ResolvedDependency};
use crate::parse::ExternalCitation;

/// A dependency as a citation needs to see it: where its corpus is, and what pin it carries.
pub struct Installed {
    pub corpus_dir: PathBuf,
    pub kind: DependencyKind,
    /// `manifest.yml`'s `commit`, for a fetched dependency. `None` for a path dependency,
    /// which is not fetched, not hashed and not locked — hashing a working tree that changes
    /// under you records nothing.
    pub pin: Option<String>,
    /// Where this dependency's node text is read from — see [`Nodes`].
    pub nodes: Nodes,
}

/// Where a dependency's node text comes from, which is a question about *when* it was read.
///
/// `lint` reads the file. It runs once against the tree as it is, and a dependency that
/// changed a second ago is the one it should judge.
///
/// The MCP server answers from the corpus it walked at startup — the same snapshot
/// `retrieve`, `get_node` and `query --across` answer from. Contract 0.9.1 states that every
/// read comes from the corpus built on disk when the server started, and that freshness is a
/// restart; #324 is what happens when one field forgets. `query --select body` reached for
/// the working tree and was, alone among the fields beside it, answering about a different
/// corpus. A span check that read through would be that defect with worse consequences: it
/// would report a citation sound against text no other tool on that server can show you.
pub enum Nodes {
    /// Read `<corpus_dir>/<node>.yml` when asked.
    OnDisk,
    /// `<class>/<name>` → that file's text, as walked at some earlier moment.
    Snapshot(BTreeMap<String, String>),
}

impl Installed {
    /// The cited node's text, or `None` when this corpus holds no such node.
    ///
    /// The one reader of a dependency's content. Both the presence test and the span
    /// comparison go through it, so a snapshot cannot answer one while the disk answers the
    /// other — which would let a citation resolve and drift at the same time.
    pub fn text(&self, node: &str) -> Option<String> {
        match &self.nodes {
            Nodes::OnDisk => std::fs::read_to_string(node_path(self, node)).ok(),
            Nodes::Snapshot(nodes) => nodes.get(node).cloned(),
        }
    }

    /// Whether this corpus holds the cited node at all.
    pub fn has(&self, node: &str) -> bool {
        match &self.nodes {
            Nodes::OnDisk => node_path(self, node).is_file(),
            Nodes::Snapshot(nodes) => nodes.contains_key(node),
        }
    }
}

/// Every dependency whose corpus can be read, keyed by the name a citation would use.
pub fn installed(root: &Path) -> BTreeMap<String, Installed> {
    crate::deps::resolved(root)
        .into_iter()
        .map(|dep: ResolvedDependency| {
            let pin = match dep.kind {
                DependencyKind::Fetched => manifest_commit(dep.corpus_dir.parent()),
                DependencyKind::Path => None,
            };
            (
                dep.name,
                Installed {
                    corpus_dir: dep.corpus_dir,
                    kind: dep.kind,
                    pin,
                    // The gate judges the tree as it is. A snapshot here would mean `lint`
                    // reporting on a dependency the working copy no longer holds.
                    nodes: Nodes::OnDisk,
                },
            )
        })
        .collect()
}

/// `commit:` from an unpacked bundle's `manifest.yml`.
///
/// Parsed with the same `serde_yaml` shape `tonpa install` decodes, and tolerant of a
/// manifest that does not have the field: an older bundle format is a reason to report the
/// citation unpinned, not a reason to fail reading the dependency.
fn manifest_commit(dir: Option<&Path>) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        commit: Option<String>,
    }
    let text = std::fs::read_to_string(dir?.join("manifest.yml")).ok()?;
    serde_yaml::from_str::<Manifest>(&text).ok()?.commit
}

/// Where a cited node's file would be.
fn node_path(dep: &Installed, node: &str) -> PathBuf {
    dep.corpus_dir.join(format!("{node}.yml"))
}

/// The four check ids, named once. A filter keyed on a literal would drift from the id the
/// baseline records the moment either was reworded.
pub const UNRESOLVED: &str = "external-citation-unresolved";
pub const SPAN_DRIFT: &str = "external-citation-span-drift";
pub const PIN_MOVED: &str = "external-citation-pin-moved";
pub const UNPINNED: &str = "external-citation-unpinned";

/// One check's verdict on one citation.
#[derive(Debug, Clone)]
pub struct Finding {
    /// The check id that would fire — the same string `lint` prints and a baseline keys on.
    pub check: &'static str,
    pub severity: Severity,
    /// That check's own message, unchanged. It names the repair, and a caller asking
    /// *before* writing needs the repair more than a caller reading a report afterwards.
    pub message: String,
}

/// Everything the four checks would say about one citation, in check order.
///
/// # One predicate, four readings
///
/// Each check used to walk the corpus itself. That was fine while `lint` was the only caller
/// and became a trap the moment a second surface wanted the same verdict about a citation
/// that **is not in the corpus yet** — which is the whole of #357: all four checks run after
/// the citation has been written into a file and committed to, so an agent about to write one
/// had no way to ask whether it would hold.
///
/// Re-deriving the predicate beside the checks would have been a fifth answer to what a
/// citation is, and `retrieve`'s history is the argument against that. So the predicate is
/// here, the checks are filters over it, and the MCP tool calls the same function they do.
/// A check whose rule changes changes for both surfaces or for neither.
///
/// # It takes a citation, not a node
///
/// Nothing here reads the citing node. That is what makes the question askable in advance:
/// the four rules are all about the far side, and the local node contributes only the path a
/// [`Violation`] is filed under.
pub fn findings(cite: &ExternalCitation, deps: &BTreeMap<String, Installed>) -> Vec<Finding> {
    let mut out = Vec::new();
    let package = cite.package.as_deref();
    let target = cite.node.as_deref();
    let mut report = |check, severity, message| {
        out.push(Finding {
            check,
            severity,
            message,
        })
    };

    // ── unresolved: the citation names something that is not there ───────────
    match (package, target) {
        (Some(package), Some(target)) => match deps.get(package) {
            None => report(
                UNRESOLVED,
                Severity::Error,
                format!(
                    "cites `{package}::{target}` and `{package}` is not installed — \
                     declare it in `.yidam/tonpa.toml` and run `yidam tonpa install`"
                ),
            ),
            Some(dep) if !dep.has(target) => report(
                UNRESOLVED,
                Severity::Error,
                format!(
                    "cites `{package}::{target}` and `{package}` has no such node — \
                     it was removed or renamed on the far side"
                ),
            ),
            Some(_) => {}
        },
        _ => report(
            UNRESOLVED,
            Severity::Error,
            format!(
                "citation with no `package:` or no `node:` — {}",
                describe(cite)
            ),
        ),
    }

    // ── span drift: it is there and says something else ──────────────────────
    //
    // A citation that named neither package nor node is `unresolved`'s finding and not a
    // second one here: there is no far side to have drifted.
    if let (Some(package), Some(target)) = (package, target) {
        match cite
            .span
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            None => report(
                SPAN_DRIFT,
                Severity::Error,
                format!(
                    "cites `{package}::{target}` with no `span:` — a node reference alone \
                     rots invisibly, because the node keeps its name while its content is \
                     rewritten"
                ),
            ),
            // A package that is not installed, or a node it does not hold, is reported by
            // `unresolved` and is not drift — there is no text to have moved.
            Some(span) => match deps.get(package).and_then(|dep| dep.text(target)) {
                // Whitespace between words is normalized on both sides and nothing else is.
                // A YAML folded scalar (`>-`) is the natural way to write a span and it
                // rewraps the text on read, so comparing raw bytes would fail every citation
                // written the readable way — against a node whose own prose is also wrapped.
                // What is *not* normalized is case, punctuation, or wording: those are the
                // changes a span exists to catch.
                Some(text) if !flatten(&text).contains(&flatten(span)) => report(
                    SPAN_DRIFT,
                    Severity::Error,
                    format!(
                        "cites `{package}::{target}`{} for a span that is no longer in it — \
                         the far side revised the text this claim rests on. Read the node and \
                         decide whether the claim still holds; do not re-quote it to make the \
                         check pass. Span: {:?}",
                        // The standing it was recorded at, where one was. A foreign tag never
                        // transfers and is not computed into anything — it is here because
                        // the person deciding whether the local claim survives wants to know
                        // what they thought they were leaning on. Its other consumer is #267.
                        match cite.tag.as_deref() {
                            Some(tag) => format!(", recorded at [{tag}],"),
                            None => String::new(),
                        },
                        truncate(span)
                    ),
                ),
                _ => {}
            },
        }
    }

    // ── pin moved: honest about a state that is no longer the installed one ──
    if let Some((package, cited)) = package.zip(cite.commit.as_deref()) {
        // A dependency that cannot be pinned is `unpinned`'s finding, not a move.
        if let Some(pin) = deps.get(package).and_then(|dep| dep.pin.as_deref()) {
            // Prefix either way. A manifest commit is `git rev-parse --short` in the
            // producing repository, and git chooses that abbreviation's length from the
            // repository's object count — so the same commit is legitimately spelled with
            // more characters in a later bundle from a larger repository. Comparing for
            // equality would report a move that did not happen.
            if !pin.starts_with(cited) && !cited.starts_with(pin) {
                report(
                    PIN_MOVED,
                    Severity::Warn,
                    format!(
                        "cites `{package}` at {cited} and {package} is installed at {pin} — \
                         the claim was written against a different state of that corpus"
                    ),
                );
            }
        }
    }

    // ── unpinned: it cannot be pinned, and says so rather than implying it was ──
    if let Some((package, dep)) = package.and_then(|name| deps.get(name).map(|dep| (name, dep))) {
        if dep.kind == DependencyKind::Path {
            report(
                UNPINNED,
                Severity::Info,
                format!(
                    "cites `{package}`, a path dependency, which carries no pin — this \
                     citation records what was read and not which state it was read from"
                ),
            );
        } else if dep.pin.is_none() {
            report(
                UNPINNED,
                Severity::Info,
                format!("cites `{package}`, whose `manifest.yml` states no commit"),
            );
        }
    }

    out
}

/// Every violation one check reports over the corpus, from the shared predicate.
///
/// Takes the findings already computed rather than computing them, so the four checks are
/// four filters over **one** walk. Recomputing per check would read every cited dependency
/// node four times to answer four questions about the same bytes.
fn violations(found: &[(&Node, Vec<Finding>)], id: &str) -> Vec<Violation> {
    found
        .iter()
        .flat_map(|(node, findings)| {
            findings
                .iter()
                .filter(|f| f.check == id)
                .map(move |f| Violation::new(&node.rel, f.message.clone()))
        })
        .collect()
}

/// The four checks, over one walk of the corpus.
///
/// Returned together and destructured at the call site rather than exposed as four public
/// functions, because four functions is what made four walks look free.
pub fn checks(nodes: &[Node], deps: &BTreeMap<String, Installed>) -> [Check; 4] {
    let found: Vec<(&Node, Vec<Finding>)> = all(nodes)
        .into_iter()
        .map(|(node, cite)| (node, findings(cite, deps)))
        .collect();
    [
        external_citation_unresolved(violations(&found, UNRESOLVED)),
        external_citation_span_drift(violations(&found, SPAN_DRIFT)),
        external_citation_pin_moved(violations(&found, PIN_MOVED)),
        external_citation_unpinned(violations(&found, UNPINNED)),
    ]
}

/// Every `(node, citation)` pair in the corpus, in corpus order.
fn all(nodes: &[Node]) -> Vec<(&Node, &ExternalCitation)> {
    nodes
        .iter()
        .flat_map(|n| {
            n.inst
                .cites
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(move |c| (n, c))
        })
        .collect()
}

/// How a citation reads, for a finding that has to name it.
fn describe(cite: &ExternalCitation) -> String {
    format!(
        "{}::{}",
        cite.package.as_deref().unwrap_or("<no package>"),
        cite.node.as_deref().unwrap_or("<no node>")
    )
}

/// A citation naming something that is not there.
///
/// **Error, against the instinct that a dependency problem should not gate a corpus.** A
/// citation that does not resolve is worse than no citation: it asserts a provenance that is
/// not there, which is `dangling-edge`'s own argument carried across the boundary.
///
/// A missing package and a missing node inside a present package are two messages and one
/// check id. The messages differ because the repairs do — `tonpa install` against a name the
/// dependency does not carry is wasted time. The id is shared because the baseline ratchet
/// keys on `(check id, node)`, and splitting them would let a repository bless the absence of
/// a whole dependency and inherit a free pass on every node inside it.
fn external_citation_unresolved(violations: Vec<Violation>) -> Check {
    Check::new(
        UNRESOLVED,
        "Citation into a dependency that is not there",
        Severity::Error,
        "A citation that does not resolve asserts a provenance that is not there, which is \
         strictly worse than claiming none. The local graph is trustworthy because a link \
         cannot rot silently; a corpus that cites across a boundary without this check has \
         inverted that property at exactly the point where it is hardest to recover. \
         A missing package and a missing node inside a present one are different findings \
         with different repairs, and the message says which.",
        violations,
    )
}

/// A citation naming something that is there and says something else.
///
/// The only check of the four that can catch a dependency **revising a node out from under a
/// claim**, and it catches it without cooperation from the producer, without history, and
/// without the sangha a bundle does not carry.
fn external_citation_span_drift(violations: Vec<Violation>) -> Check {
    Check::new(
        SPAN_DRIFT,
        "Cited span no longer appears in the dependency",
        Severity::Error,
        "A node reference alone rots invisibly: the node keeps its name while its content is \
         rewritten, and the citation still resolves. The span is what makes the rot visible, \
         and it is the only check available across this boundary that needs nothing from the \
         producer — a bundle carries no sangha, no elector register and no resolution \
         history, so the text is the whole of the evidence. Re-quoting to clear the finding \
         is the one repair that is always wrong: the far side changed its mind, and the \
         question is whether the local claim survives it.",
        violations,
    )
}

/// A citation honest about a state that is no longer the installed one.
///
/// **Warn, not Error, and the difference is a design decision rather than a severity
/// preference.** A stale dependency is a normal state — it is pinned deliberately — and
/// escalating this would mean a producer cutting a release could turn a stranger's CI red
/// without the stranger changing anything, which is the exact failure a pin exists to
/// prevent.
fn external_citation_pin_moved(violations: Vec<Violation>) -> Check {
    Check::new(
        PIN_MOVED,
        "Citation written against a pin that has moved",
        Severity::Warn,
        "A stale dependency is a normal state: it is pinned deliberately, and where its \
         currency bears on a conclusion the thing to do is say which pin was read. This \
         reports rather than gates because escalating it would let a producer cutting a \
         release turn a stranger's CI red without the stranger changing anything — the exact \
         failure a pin exists to prevent. Where the text this claim rests on actually moved, \
         `external-citation-span-drift` is the finding that gates.",
        violations,
    )
}

/// A citation that cannot be pinned, saying so rather than implying it was.
fn external_citation_unpinned(violations: Vec<Violation>) -> Check {
    Check::new(
        UNPINNED,
        "Citation into a dependency that carries no pin",
        Severity::Info,
        "A path dependency is read where it sits and is not fetched, not hashed and not \
         locked, because hashing a working tree that changes under you records nothing. It \
         is also the only dependency form that supports a development loop, so refusing to \
         cite one would exclude the case people actually work in. Reported rather than \
         refused, and reported every time: a citation whose pin is silently absent is \
         indistinguishable from one whose pin is current.",
        violations,
    )
}

/// Whitespace-flattened, for a containment test that survives YAML folding.
fn flatten(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A span short enough to sit in a finding, with the elision visible.
fn truncate(span: &str) -> String {
    let flat = flatten(span);
    match flat.chars().count() > 60 {
        true => format!("{}…", flat.chars().take(60).collect::<String>()),
        false => flat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::Overlay;
    use crate::walk::walk_corpus_instances;

    /// A repository with one node, and one installed dependency holding one node.
    fn fixture(cites: &str, foreign: &str, manifest: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus/reach");
        std::fs::create_dir_all(&corpus).unwrap();
        std::fs::write(
            corpus.join("tailwater.yml"),
            format!("class: reach\nlabel: Tailwater\n{cites}"),
        )
        .unwrap();

        let pkg = dir.path().join(".yidam/tonpa/upstream");
        std::fs::create_dir_all(pkg.join("corpus/concept")).unwrap();
        std::fs::write(pkg.join("corpus/concept/base-flow.yml"), foreign).unwrap();
        if let Some(manifest) = manifest {
            std::fs::write(pkg.join("manifest.yml"), manifest).unwrap();
        }
        dir
    }

    fn nodes(root: &Path) -> Vec<Node> {
        super::super::checks::load_nodes(
            root,
            &walk_corpus_instances(&crate::paths::yidam_corpus_dir(root)),
            &Overlay::default(),
        )
    }

    const FOREIGN: &str = "class: concept\nlabel: Base-flow separation\ndescription: |\n  \
                           Partitioning a hydrograph into base flow — the slowly varying\n  \
                           component sustained by groundwater discharge — and quickflow.\n";
    const MANIFEST: &str = "bundle_version: \"1\"\ncommit: \"8d35441\"\n";

    fn cites(body: &str) -> String {
        format!("cites:\n  - package: upstream\n    node: concept/base-flow\n{body}")
    }

    fn run(dir: &tempfile::TempDir) -> BTreeMap<&'static str, Check> {
        let root = dir.path();
        let [unresolved, drift, pin, unpinned] = checks(&nodes(root), &installed(root));
        [
            ("unresolved", unresolved),
            ("drift", drift),
            ("pin", pin),
            ("unpinned", unpinned),
        ]
        .into_iter()
        .collect()
    }

    /// The happy path: everything resolves and nothing is reported.
    #[test]
    fn a_citation_that_resolves_reports_nothing() {
        let dir = fixture(
            &cites(
                "    commit: 8d35441\n    span: >-\n      the slowly varying component \
                 sustained by groundwater discharge\n",
            ),
            FOREIGN,
            Some(MANIFEST),
        );
        for (name, check) in run(&dir) {
            assert!(
                check.violations.is_empty(),
                "{name}: {:?}",
                check.violations
            );
        }
    }

    /// A YAML folded scalar rewraps on read, and the cited node's own prose is wrapped too.
    /// Comparing raw bytes would fail every citation written the readable way.
    #[test]
    fn a_span_matches_across_line_wrapping_on_both_sides() {
        let dir = fixture(
            &cites(
                "    span: \"base flow — the slowly varying component sustained by groundwater\"\n",
            ),
            FOREIGN,
            Some(MANIFEST),
        );
        assert!(run(&dir)["drift"].violations.is_empty());
    }

    /// What flattening must not swallow: the far side changed the words.
    #[test]
    fn a_reworded_span_is_drift() {
        let dir = fixture(
            &cites("    span: \"sustained by groundwater discharge\"\n"),
            &FOREIGN.replace("groundwater discharge", "subsurface storage"),
            Some(MANIFEST),
        );
        let violations = &run(&dir)["drift"].violations;
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("revised the text"));
    }

    /// The repair that is always wrong is named in the rationale, because it is the one a
    /// reader reaches for first.
    #[test]
    fn drift_says_not_to_requote() {
        let dir = fixture(&cites("    span: \"gone\"\n"), FOREIGN, Some(MANIFEST));
        assert!(run(&dir)["drift"].rationale.contains("Re-quoting"));
    }

    /// A citation with no span is drift's finding, not unresolved's: the node is there.
    #[test]
    fn a_citation_with_no_span_is_reported() {
        let dir = fixture(&cites(""), FOREIGN, Some(MANIFEST));
        assert!(run(&dir)["unresolved"].violations.is_empty());
        assert_eq!(run(&dir)["drift"].violations.len(), 1);
    }

    /// Two messages, one check id.
    #[test]
    fn a_missing_package_and_a_missing_node_say_which() {
        let missing_pkg = fixture(
            "cites:\n  - package: nowhere\n    node: concept/x\n    span: y\n",
            FOREIGN,
            Some(MANIFEST),
        );
        let v = &run(&missing_pkg)["unresolved"].violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("is not installed"), "{}", v[0].detail);

        let missing_node = fixture(
            "cites:\n  - package: upstream\n    node: concept/gone\n    span: y\n",
            FOREIGN,
            Some(MANIFEST),
        );
        let v = &run(&missing_node)["unresolved"].violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("no such node"), "{}", v[0].detail);
    }

    /// A moved pin reports and does not gate.
    #[test]
    fn a_moved_pin_is_a_warning_naming_both_states() {
        let dir = fixture(
            &cites("    commit: 0000000\n    span: \"slowly varying component\"\n"),
            FOREIGN,
            Some(MANIFEST),
        );
        let check = &run(&dir)["pin"];
        assert_eq!(check.severity, Severity::Warn);
        assert_eq!(check.violations.len(), 1);
        assert!(check.violations[0].detail.contains("8d35441"));
        assert!(check.violations[0].detail.contains("0000000"));
    }

    /// Git chooses an abbreviation's length from the producing repository's object count, so
    /// the same commit is legitimately spelled longer in a later bundle. Comparing for
    /// equality reports a move that did not happen.
    #[test]
    fn a_lengthened_abbreviation_of_the_same_commit_is_not_a_move() {
        let dir = fixture(
            &cites("    commit: 8d35441\n    span: \"slowly varying component\"\n"),
            FOREIGN,
            Some("bundle_version: \"1\"\ncommit: \"8d354419ab\"\n"),
        );
        assert!(run(&dir)["pin"].violations.is_empty());
    }

    // ── the predicate, asked directly (#357) ─────────────────────────────────

    /// A citation nothing in the corpus has written yet, judged.
    ///
    /// The whole point of lifting the predicate: `findings` takes a citation and a dependency
    /// set, and nothing else. There is no citing node here and there does not need to be —
    /// all four rules are about the far side.
    #[test]
    fn a_citation_is_judged_without_a_node_to_have_written_it_on() {
        let dir = fixture("", FOREIGN, Some(MANIFEST));
        let deps = installed(dir.path());
        let cite = ExternalCitation {
            package: Some("upstream".into()),
            node: Some("concept/base-flow".into()),
            commit: Some("8d35441".into()),
            span: Some("the slowly varying component sustained by groundwater discharge".into()),
            tag: None,
        };
        assert!(findings(&cite, &deps).is_empty());

        let drifted = ExternalCitation {
            span: Some("sustained by a reservoir release".into()),
            ..cite
        };
        let found = findings(&drifted, &deps);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].check, SPAN_DRIFT);
        assert_eq!(found[0].severity, Severity::Error);
    }

    /// The four checks and the predicate cannot disagree, because there is one of them.
    ///
    /// A citation wrong in two independent ways: the pin has moved *and* the span is gone.
    /// Both are reported, at their own severities, and the checks report exactly what the
    /// predicate does — which is the property that lets `check_citation` promise an agent the
    /// verdict its gate will give.
    #[test]
    fn the_checks_report_exactly_what_the_predicate_found() {
        let dir = fixture(
            &cites("    commit: 0000000\n    span: \"a reservoir release\"\n"),
            FOREIGN,
            Some(MANIFEST),
        );
        let deps = installed(dir.path());
        let cite = ExternalCitation {
            package: Some("upstream".into()),
            node: Some("concept/base-flow".into()),
            commit: Some("0000000".into()),
            span: Some("a reservoir release".into()),
            tag: None,
        };

        let found = findings(&cite, &deps);
        let ids: Vec<&str> = found.iter().map(|f| f.check).collect();
        assert_eq!(ids, vec![SPAN_DRIFT, PIN_MOVED]);

        let checks = run(&dir);
        assert_eq!(checks["drift"].violations.len(), 1);
        assert_eq!(checks["pin"].violations.len(), 1);
        assert!(checks["unresolved"].violations.is_empty());
        assert!(checks["unpinned"].violations.is_empty());
        for (check, finding) in [(&checks["drift"], &found[0]), (&checks["pin"], &found[1])] {
            assert_eq!(check.violations[0].detail, finding.message);
            assert_eq!(check.severity, finding.severity);
        }
    }

    /// A snapshot answers, and the disk is never reached.
    ///
    /// `corpus_dir` names a path that does not exist, so a read that fell through would
    /// report the node missing. This is the arm the MCP server runs on: it must judge a span
    /// against the corpus it walked at startup, which is the one every other tool answers
    /// from.
    #[test]
    fn a_snapshot_answers_a_span_without_reading_the_disk() {
        let deps: BTreeMap<String, Installed> = [(
            "upstream".to_string(),
            Installed {
                corpus_dir: PathBuf::from("/nonexistent/upstream/corpus"),
                kind: DependencyKind::Fetched,
                pin: Some("8d35441".into()),
                nodes: Nodes::Snapshot(
                    [("concept/base-flow".to_string(), FOREIGN.to_string())].into(),
                ),
            },
        )]
        .into();

        let held = ExternalCitation {
            package: Some("upstream".into()),
            node: Some("concept/base-flow".into()),
            commit: Some("8d35441".into()),
            span: Some("sustained by groundwater discharge".into()),
            tag: None,
        };
        assert!(findings(&held, &deps).is_empty());

        // And the snapshot bounds what exists: a node it does not carry is unresolved, not a
        // stat against a directory that is not there.
        let absent = ExternalCitation {
            node: Some("concept/quickflow".into()),
            ..held
        };
        let found = findings(&absent, &deps);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].check, UNRESOLVED);
        assert!(found[0].message.contains("no such node"));
    }

    /// A manifest with no commit is unpinned, not moved.
    #[test]
    fn a_manifest_with_no_commit_is_unpinned_rather_than_moved() {
        let dir = fixture(
            &cites("    commit: 8d35441\n    span: \"slowly varying component\"\n"),
            FOREIGN,
            Some("bundle_version: \"1\"\n"),
        );
        assert!(run(&dir)["pin"].violations.is_empty());
        assert_eq!(run(&dir)["unpinned"].violations.len(), 1);
        assert_eq!(run(&dir)["unpinned"].severity, Severity::Info);
    }
}

// ── what moved under a claim (#267) ───────────────────────────────────────────
//
// `tonpa update` moves a pin. Nothing said what that did to the claims resting on the other
// side of it: a dependency can revise a node out from under a citation, and the citing corpus
// learns nothing until someone happens to run `lint`.
//
// **This opens a question; it does not resolve one.** A tool that decided a local claim was no
// longer warranted because a foreign one moved would be performing synthesis, and that is the
// line `cmd/sangha.rs` draws and the constitution draws. Every movement below is phrased as a
// question for a person, and nothing here writes to the corpus.
//
// The survey is in the light build on purpose, and `tonpa update` is not. The feature buys the
// network; comparing two states of an installed dependency needs none of it — and keeping the
// comparison out of the gated half is what makes it testable without one.

/// What one citation resolved to, at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// The citing local node, repo-relative.
    pub node: String,
    pub package: String,
    /// The cited node's id inside that corpus.
    pub target: String,
    pub resolved: bool,
    pub span_present: bool,
    /// The standings the cited node carries **now** — not the one the citation recorded.
    /// Sorted, so two surveys compare without ordering noise.
    pub standings: Vec<&'static str>,
    /// The standing the citing author recorded having read.
    pub recorded: Option<String>,
    pub pin: Option<String>,
}

/// Resolve every citation in the corpus against the dependencies currently on disk.
///
/// Called twice by `tonpa update` — once before the fetch and once after — because the
/// question is not what the dependency says, it is *what changed about what I was leaning on*.
pub fn survey(root: &Path) -> Vec<Standing> {
    let corpus_dir = crate::paths::yidam_corpus_dir(root);
    let nodes = super::checks::load_nodes(
        root,
        &crate::walk::walk_corpus_instances(&corpus_dir),
        &super::Overlay::default(),
    );
    let deps = installed(root);
    // One `ClaimFields` per dependency, not per citation: it is a property of that corpus's
    // ontology, and loading it per citation would re-read every `.ont.yml` in the bundle for
    // every claim that leans on it.
    let fields: BTreeMap<&String, crate::claims::ClaimFields> = deps
        .iter()
        .map(|(name, dep)| (name, crate::claims::ClaimFields::load(&dep.corpus_dir)))
        .collect();

    let mut out = Vec::new();
    for (node, cite) in all(&nodes) {
        let (Some(package), Some(target)) = (cite.package.as_deref(), cite.node.as_deref()) else {
            continue;
        };
        let dep = deps.get(package);
        let text = dep.and_then(|d| d.text(target));
        let standings = match (
            &text,
            deps.get_key_value(package).and_then(|(k, _)| fields.get(k)),
        ) {
            (Some(text), Some(fields)) => {
                let class = target.split('/').next().unwrap_or_default();
                let mut seen: Vec<&'static str> =
                    crate::claims::claims_in_node(text, fields.for_class(class))
                        .into_iter()
                        .map(|c| c.standing)
                        .collect();
                seen.sort_unstable();
                seen.dedup();
                seen
            }
            _ => Vec::new(),
        };
        out.push(Standing {
            node: node.rel.clone(),
            package: package.to_string(),
            target: target.to_string(),
            resolved: text.is_some(),
            span_present: match (&text, cite.span.as_deref()) {
                (Some(text), Some(span)) => flatten(text).contains(&flatten(span)),
                _ => false,
            },
            standings,
            recorded: cite.tag.clone(),
            pin: dep.and_then(|d| d.pin.clone()),
        });
    }
    out
}

/// One thing that moved beneath a local claim.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Movement {
    /// The **local** node, because this report is about my graph and not theirs.
    pub node: String,
    pub package: String,
    pub target: String,
    /// From a closed set, so a consumer branches without matching prose:
    /// `vanished`, `span-drifted`, `standing-withdrawn`, `standing-changed`, `pin-moved`.
    pub kind: &'static str,
    /// Phrased as a question, deliberately. The answer is a person's.
    pub question: String,
}

/// What changed between two surveys, in terms of the citing corpus.
///
/// Matched on `(node, package, target)`. A citation present in one survey and not the other is
/// not a movement — the corpus itself changed between the two, which cannot happen inside an
/// update and would be a different report if it could.
pub fn moved(before: &[Standing], after: &[Standing]) -> Vec<Movement> {
    let mut out = Vec::new();
    for old in before {
        let key = |s: &&Standing| {
            s.node == old.node && s.package == old.package && s.target == old.target
        };
        let Some(new) = after.iter().find(key) else {
            continue;
        };
        let at = |kind, question| Movement {
            node: old.node.clone(),
            package: old.package.clone(),
            target: old.target.clone(),
            kind,
            question,
        };

        if old.resolved && !new.resolved {
            out.push(at(
                "vanished",
                format!(
                    "`{}::{}` is gone. Does the claim in `{}` still stand without it, and if \
                     so on what?",
                    old.package, old.target, old.node
                ),
            ));
            // Everything downstream of a node that is not there is not a second finding.
            continue;
        }
        if old.span_present && !new.span_present {
            out.push(at(
                "span-drifted",
                format!(
                    "the text `{}` quoted from `{}::{}` is no longer there. Read what replaced \
                     it: does it still support the claim, or did the far side change its mind?",
                    old.node, old.package, old.target
                ),
            ));
        }
        // The precise form, available only when the citation recorded what it read. This is
        // the case #267 names: a foreign `[verified]` demoted to `[open]`.
        match old.recorded.as_deref() {
            Some(recorded) => {
                if old.standings.contains(&recorded) && !new.standings.contains(&recorded) {
                    out.push(at(
                        "standing-withdrawn",
                        format!(
                            "`{}::{}` no longer carries [{recorded}] — it now carries {}. Your \
                             claim in `{}` was written against the stronger reading.",
                            old.package,
                            old.target,
                            describe_standings(&new.standings),
                            old.node
                        ),
                    ));
                }
            }
            // The coarse form, and it says it is coarse. A citation that recorded no `tag:`
            // cannot be asked the precise question, and reporting nothing would make the
            // missing field look like a clean bill of health.
            None if old.standings != new.standings => out.push(at(
                "standing-changed",
                format!(
                    "`{}::{}` carried {} and now carries {}. This citation recorded no `tag:`, \
                     so which of those you were leaning on is not written down — check `{}`.",
                    old.package,
                    old.target,
                    describe_standings(&old.standings),
                    describe_standings(&new.standings),
                    old.node
                ),
            )),
            None => {}
        }
        if old.pin != new.pin {
            out.push(at(
                "pin-moved",
                format!(
                    "`{}` moved from {} to {}. `{}` records the older one; is the citation \
                     still describing what you read?",
                    old.package,
                    old.pin.as_deref().unwrap_or("no pin"),
                    new.pin.as_deref().unwrap_or("no pin"),
                    old.node
                ),
            ));
        }
    }
    out
}

fn describe_standings(standings: &[&'static str]) -> String {
    match standings.is_empty() {
        true => "no tagged claim".to_string(),
        false => standings
            .iter()
            .map(|s| format!("[{s}]"))
            .collect::<Vec<_>>()
            .join(" and "),
    }
}

/// The movements, as `tonpa update` prints them.
///
/// **Questions, and a sentence saying nothing was changed.** The temptation on a report like
/// this is a summary line that reads like a verdict — *3 claims weakened* — and that is
/// precisely the synthesis this must not perform.
pub fn render_movements(movements: &[Movement]) -> String {
    if movements.is_empty() {
        return "Nothing your corpus cites moved.".to_string();
    }
    let mut out = format!(
        "{} question(s) opened by this update — nothing was changed, and no claim was \
         re-tagged:\n",
        movements.len()
    );
    let mut current = String::new();
    for m in movements {
        if m.node != current {
            out.push_str(&format!("\n  {}\n", m.node));
            current = m.node.clone();
        }
        out.push_str(&format!("    [{}] {}\n", m.kind, m.question));
    }
    out.push_str(
        "\nThese are findings, not revisions. Answer them in the corpus — a claim that no \
         longer holds is rewritten by a person, and one whose support moved may need to be \
         re-tagged or made an open question.\n",
    );
    out
}

#[cfg(test)]
mod survey_tests {
    use super::*;

    /// A repository citing an installed dependency whose node carries a tagged claim.
    ///
    /// The dependency ships its own `.ont.yml` declaring a `type: claim` property, because
    /// that is what a real bundle carries and it is what decides whether the structural arm
    /// of the claim reader sees anything at all. A fixture without it would exercise the
    /// prose arm only and pass while the arm that matters read nothing.
    fn fixture(foreign: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus/reach");
        std::fs::create_dir_all(&corpus).unwrap();
        std::fs::write(
            corpus.join("tailwater.yml"),
            "class: reach\nlabel: Tailwater\ncites:\n  - package: upstream\n    \
             node: concept/base-flow\n    tag: verified\n    span: \"groundwater discharge\"\n",
        )
        .unwrap();

        let pkg = dir.path().join(".yidam/tonpa/upstream/corpus");
        std::fs::create_dir_all(pkg.join("concept")).unwrap();
        std::fs::write(
            pkg.join("concept.ont.yml"),
            "class: concept\nproperties:\n  - name: claim_tag\n    type: claim\n",
        )
        .unwrap();
        std::fs::write(pkg.join("concept/base-flow.yml"), foreign).unwrap();
        std::fs::write(
            dir.path().join(".yidam/tonpa/upstream/manifest.yml"),
            "bundle_version: \"1\"\ncommit: \"aaa1111\"\n",
        )
        .unwrap();
        dir
    }

    const VERIFIED: &str = "class: concept\nlabel: Base flow\ndescription: Sustained by \
                            groundwater discharge.\nproperties:\n  claim_tag: verified\n";

    /// The survey reads the *producer's current* standing, not the one the citation recorded.
    /// If this returns empty the whole of #267 reports nothing and looks healthy.
    #[test]
    fn a_survey_reads_the_standing_the_dependency_carries_now() {
        let dir = fixture(VERIFIED);
        let survey = survey(dir.path());
        assert_eq!(survey.len(), 1);
        assert!(survey[0].resolved);
        assert!(survey[0].span_present);
        assert_eq!(survey[0].standings, vec!["verified"]);
        assert_eq!(survey[0].recorded.as_deref(), Some("verified"));
        assert_eq!(survey[0].pin.as_deref(), Some("aaa1111"));
    }

    /// End to end, without a network: survey, revise the dependency the way `tonpa update`
    /// would by overwriting it, survey again, and ask what moved.
    #[test]
    fn a_demotion_on_the_far_side_opens_a_question_about_the_local_node() {
        let dir = fixture(VERIFIED);
        let before = survey(dir.path());

        std::fs::write(
            dir.path()
                .join(".yidam/tonpa/upstream/corpus/concept/base-flow.yml"),
            VERIFIED.replace("claim_tag: verified", "claim_tag: open"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".yidam/tonpa/upstream/manifest.yml"),
            "bundle_version: \"1\"\ncommit: \"bbb2222\"\n",
        )
        .unwrap();

        let movements = moved(&before, &survey(dir.path()));
        let kinds: Vec<&str> = movements.iter().map(|m| m.kind).collect();
        assert!(kinds.contains(&"standing-withdrawn"), "{kinds:?}");
        assert!(kinds.contains(&"pin-moved"), "{kinds:?}");
        assert!(
            movements.iter().all(|m| m.node.ends_with("tailwater.yml")),
            "the report is about my graph, not theirs"
        );
    }
}

#[cfg(test)]
mod movement_tests {
    use super::*;

    fn standing(recorded: Option<&str>, standings: &[&'static str], pin: &str) -> Standing {
        Standing {
            node: ".yidam/corpus/reach/tailwater.yml".into(),
            package: "upstream".into(),
            target: "concept/base-flow".into(),
            resolved: true,
            span_present: true,
            standings: standings.to_vec(),
            recorded: recorded.map(str::to_string),
            pin: Some(pin.into()),
        }
    }

    /// The case #267 names: a foreign `[verified]` demoted to `[open]`.
    #[test]
    fn a_withdrawn_standing_is_reported_against_the_local_node() {
        let before = vec![standing(Some("verified"), &["verified"], "aaa")];
        let after = vec![standing(Some("verified"), &["open"], "aaa")];
        let moved = moved(&before, &after);
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].kind, "standing-withdrawn");
        assert!(
            moved[0].question.contains("[open]"),
            "{}",
            moved[0].question
        );
        // In terms of *my* graph. The local node is the subject, not the foreign one.
        assert!(moved[0].node.ends_with("tailwater.yml"));
    }

    /// A node gaining a standing it did not have is not a withdrawal of the one I cited.
    #[test]
    fn a_standing_added_beside_the_one_i_cited_is_not_a_movement() {
        let before = vec![standing(Some("verified"), &["verified"], "aaa")];
        let after = vec![standing(Some("verified"), &["open", "verified"], "aaa")];
        assert!(moved(&before, &after).is_empty());
    }

    /// Without a recorded tag the precise question cannot be asked, and silence would make
    /// the missing field look like a clean bill of health.
    #[test]
    fn a_citation_with_no_recorded_tag_gets_the_coarse_question_and_is_told_so() {
        let before = vec![standing(None, &["verified"], "aaa")];
        let after = vec![standing(None, &["open"], "aaa")];
        let moved = moved(&before, &after);
        assert_eq!(moved[0].kind, "standing-changed");
        assert!(moved[0].question.contains("recorded no `tag:`"));
    }

    /// A node that is gone is one question, not four.
    #[test]
    fn a_vanished_node_does_not_also_report_its_span_and_standing() {
        let before = vec![standing(Some("verified"), &["verified"], "aaa")];
        let mut gone = standing(Some("verified"), &[], "bbb");
        gone.resolved = false;
        gone.span_present = false;
        let moved = moved(&before, &[gone]);
        assert_eq!(moved.len(), 1, "{moved:?}");
        assert_eq!(moved[0].kind, "vanished");
    }

    /// The output must not read as a verdict. This is the constitutional line, and it is the
    /// one a future edit is most likely to cross while making the summary friendlier.
    #[test]
    fn the_report_says_it_changed_nothing() {
        let before = vec![standing(Some("verified"), &["verified"], "aaa")];
        let after = vec![standing(Some("verified"), &["open"], "bbb")];
        let text = render_movements(&moved(&before, &after));
        assert!(text.contains("nothing was changed"));
        assert!(text.contains("findings, not revisions"));
        assert!(
            text.contains('?'),
            "every movement is phrased as a question"
        );
    }

    #[test]
    fn an_update_that_moved_nothing_says_so_rather_than_printing_a_heading() {
        assert_eq!(render_movements(&[]), "Nothing your corpus cites moved.");
    }
}
