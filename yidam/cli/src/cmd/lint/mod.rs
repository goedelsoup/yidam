//! `yidam lint` — run every check, compare against the ratchet, report.
//!
//! The gate answers one question: *did this commit make the corpus less clean?* It does
//! not answer *is the corpus clean?*, which for any repository with history is usually no.
//! Conflating the two is what produces a gate that is either permanently red or
//! permanently ignored; see [`baseline`].

pub(crate) mod attest;
pub(crate) mod baseline;
pub(crate) mod checks;
pub(crate) mod citations;
pub(crate) mod commitments;
pub(crate) mod commits;
pub(crate) mod edge_claims;
pub(crate) mod history;
pub(crate) mod independence;
pub mod json;
pub(crate) mod line_citations;
pub(crate) mod lineage;
pub(crate) mod local_citations;
pub(crate) mod model;
pub(crate) mod scope;
pub(crate) mod ttl;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

pub use line_citations::{
    citation_label_not_cited, citation_range_stated_twice, dead_line_citation, label_range,
    label_symbols, relocate, slid_line_citation, unverified_line_citation, LineCitation,
    LineFragment, Relocation,
};
pub use model::{Check, Severity, Violation, LINT_SEVERITIES};

/// Every line-anchored citation in `root`'s prose surfaces (`docs/`, `.yidam/`), read
/// from disk.
///
/// Exposed so this repository can hold its own documentation to the line-citation checks
/// from a test: the lint gate walks a *corpus*, the template repository is not one, and
/// that gap is exactly how twelve citations rotted with every build green (#563).
/// Authorship regions are honoured the way the gate honours them — a file in a declared
/// region is not plainly this repository's, and its citations are not held here.
pub fn collect_line_citations(root: &Path) -> Vec<LineCitation> {
    let overlay = Overlay::default();
    let authorship = crate::authorship::Authorship::load_or_default(root);
    let mut paths = walk_linkable_files(&root.join(".yidam"));
    paths.extend(walk_linkable_files(&root.join("docs")));
    let mut links = Vec::new();
    for p in &paths {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        if authorship.covering(&rel).is_some() {
            continue;
        }
        let dir = p.parent().unwrap_or(root);
        links.extend(checks::prose_links(&rel, dir, &overlay.read(p)));
    }
    line_citations::collect(root, &links, &|p| overlay.read(p))
}

/// The severity `unrecognized-verb` reports at.
///
/// Exposed so that a surface rendering the same rule somewhere else — `yidam vocabulary
/// --check`, and through it the SCM input box — reads its severity from the check rather
/// than restating it. A squiggle stricter than the gate is an editor asserting a verdict
/// nobody agreed to.
/// The committed baseline, or an empty one.
///
/// Exposed for `serve --lsp`, which needs the same debt accounting the gate uses: a finding
/// the baseline already records is inherited and must not be rendered as a regression.
pub(crate) fn load_baseline(root: &Path) -> baseline::Baseline {
    baseline::Baseline::load(root).unwrap_or_default()
}

/// The wire report, built exactly as `--format json` builds it.
pub(crate) fn build_report(
    root: &Path,
    checks: &[Check],
    base: &baseline::Baseline,
) -> json::LintReport {
    let commits = history::corpus_commits(root);
    json::build(root, checks, base, &baseline::diff(checks, base, &commits))
}

pub(crate) fn commit_verb_severity() -> Severity {
    commits::unrecognized_verb(&[], &crate::kuten::Registers::corpus_only()).severity
}

use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::{
    walk_corpus_instances, walk_linkable_files, walk_md_files, walk_ont_files, walk_rust_files,
};

/// Unsaved editor buffers, keyed by absolute path.
///
/// Every check in this module reads the working tree, which is exactly right for a gate and
/// exactly wrong for an editor: the file you are typing into is the one whose findings you
/// want, and it is the one on disk that is stale. An overlay lets `serve --lsp` answer about
/// the buffer without any check knowing that is what it is doing.
///
/// Empty for every other caller, and `Overlay::read` is then a plain `read_to_string`.
#[derive(Debug, Default, Clone)]
pub struct Overlay(HashMap<PathBuf, String>);

impl Overlay {
    pub fn set(&mut self, path: PathBuf, text: String) {
        self.0.insert(path, text);
    }

    pub fn clear(&mut self, path: &Path) {
        self.0.remove(path);
    }

    /// The buffer if one is open, otherwise the file.
    pub fn read(&self, path: &Path) -> String {
        match self.0.get(path) {
            Some(text) => text.clone(),
            None => std::fs::read_to_string(path).unwrap_or_default(),
        }
    }

    /// Instance buffers the walker cannot see: open under `corpus`, and not yet on disk.
    ///
    /// Every path the checks read comes from a directory walk, so a buffer for a file that
    /// has not been saved once was read by nobody — an editor's `:e concept/new.yml` got no
    /// verdict until the first `:w`, and the web editor's node form (#607) is a buffer that
    /// by design is *never* written, so it got none at all. Same shape as `read`: the walk
    /// answers for what is on disk, and the overlay answers for what is not, with the same
    /// predicate `walk_corpus_instances` applies — under the corpus, at least a class
    /// directory deep, `.yml`, and not a class file.
    ///
    /// A buffer whose file *does* exist is the walk's already and is not repeated here.
    pub fn unsaved_instances(&self, corpus: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = self
            .0
            .keys()
            .filter(|p| !p.exists())
            .filter(|p| {
                let Ok(rel) = p.strip_prefix(corpus) else {
                    return false;
                };
                let name = rel.file_name().map(|n| n.to_string_lossy());
                rel.components().count() >= 2
                    && p.extension().is_some_and(|x| x == "yml")
                    && name.is_some_and(|n| !n.ends_with(".ont.yml"))
            })
            .cloned()
            .collect();
        found.sort();
        found
    }
}

/// How `lint` was invoked.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Report everything, gate on nothing. Escape hatch, not a mode of operation.
    pub warn_only: bool,
    /// Print each check's rationale alongside its findings.
    pub explain: bool,
    /// Also check the git log against the commit vocabulary.
    pub commits: bool,
    /// Restrict the commit check to a revision range (e.g. `main..HEAD`).
    pub range: Option<String>,
    /// Rewrite the baseline from this run instead of gating on it.
    pub bless: bool,
    /// Write a baseline only if there is not one already, then exit.
    ///
    /// The adoption path, and safe to run unconditionally — which is the whole point, so
    /// that re-vendoring can call it without first asking whether this repository has ever
    /// blessed anything. A repository that has never created a baseline is the case this
    /// exists for, not the exception: the measured corpus with a third of its nodes
    /// unreachable had no `lint-baseline.yml` at all, so a ratchet had nothing to ratchet
    /// against and reported clean.
    pub init_baseline: bool,
    /// Output format. `text` is what this command has always printed.
    pub format: crate::report::Format,
}

/// Run every check against the repository at `root`.
pub fn run_checks(root: &Path, opts: &Options) -> Vec<Check> {
    run_checks_with(root, opts, &Overlay::default())
}

/// Every check, reading through `overlay` rather than straight from disk.
pub fn run_checks_with(root: &Path, opts: &Options, overlay: &Overlay) -> Vec<Check> {
    let corpus_dir = yidam_corpus_dir(root);
    let catalog_dir = yidam_catalog_dir(root);

    let mut instance_paths = walk_corpus_instances(&corpus_dir);
    // Plus the buffers that are not files yet — see `Overlay::unsaved_instances`. Empty for
    // every caller but the language server.
    instance_paths.extend(overlay.unsaved_instances(&corpus_dir));
    // Which disclosure decisions this repository decided for itself. Read here rather than in
    // the check, which stays pure — the same split every other check in this module keeps.
    //
    // A policy that does not compile is not reported as an override: it is a failure, and
    // `yidam policy check` and `yidam doctor` are where it is reported as one. Swallowing it
    // into an empty list here would turn a broken rule into a clean gate.
    let policy_overrides: Vec<(String, String)> = crate::policy::Policies::load(root)
        .map(|p| {
            p.origins()
                .filter_map(|(d, o)| match o {
                    crate::policy::Origin::Local(path) => Some((
                        d.to_string(),
                        path.strip_prefix(root)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string(),
                    )),
                    crate::policy::Origin::Inherited => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let nodes = checks::load_nodes(root, &instance_paths, overlay);

    let ont_paths = walk_ont_files(&corpus_dir);
    let classes = checks::load_classes(root, &ont_paths, overlay);
    // Read through the overlay like every class, so the editor lints an unsaved
    // `universal.yml` against the buffer rather than against the file on disk.
    let universal =
        crate::universal::Universal::parse(&overlay.read(&crate::universal::Universal::path(root)));
    let defined: HashSet<String> = ont_paths
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(str::to_string)
        })
        .collect();

    // What this repository depends on and can actually read. Off disk, in the light build:
    // `--features tonpa` buys the network, and derived-repo CI downloads a binary rather than
    // compiling one — so a citation check behind that feature would never run where it counts.
    let deps = citations::installed(root);

    let catalog_paths = walk_md_files(&catalog_dir);
    let sources = checks::load_sources(root, &catalog_paths, overlay);
    // `Node` carries the text `load_nodes` already read, so nothing here re-reads the corpus
    // to hand the same bytes to a check a second time.
    let cites = checks::citations(&sources, &nodes);
    // The `type: claim` properties each class declared, so the structural arm of the claim
    // reader sees anything at all. Loaded once and shared: it walks the ontology.
    let claim_fields = crate::claims::ClaimFields::load(&corpus_dir);
    // Built from the classes already parsed above rather than re-read from disk, and through
    // the overlay for `universal.yml`, so the editor measures an unsaved declaration. Keyed
    // by the `.ont.yml` stem, which is the directory an instance's class resolves to — the
    // same keying `ClaimFields` documents.
    let prose_fields = crate::prose::ProseFields::from_declarations(
        universal.prose().to_vec(),
        classes
            .iter()
            .map(|c| crate::prose::Declaration {
                class: c.name.clone(),
                keys: c.prose.clone(),
                properties: c
                    .properties
                    .iter()
                    .filter(|p| p.prose)
                    .map(|p| p.name.clone())
                    .collect(),
            })
            .collect::<Vec<_>>(),
    );
    // How old each source record is, and whether this corpus asked to be told. `today` is
    // resolved once here rather than inside the check, so the one wall-clock report in the
    // tool has a single place its clock enters.
    let catalog_ages = ttl::ages(
        &sources,
        &ttl::committed_dates(root, &catalog_dir),
        crate::config::load_yidam_config(root)
            .map(|c| c.catalog.ttl_days)
            .unwrap_or_default(),
        &today_iso(),
    );

    // Tables are checked wherever a reader meets one: catalog entries and the READMEs
    // that carry REGEN blocks.
    let mut prose: Vec<(String, String)> = Vec::new();
    for p in catalog_paths.iter().chain(
        [corpus_dir.join("README.md"), catalog_dir.join("README.md")]
            .iter()
            .filter(|p| p.exists()),
    ) {
        prose.push((
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string(),
            overlay.read(p),
        ));
    }

    // Resolution records, when this repository runs a sangha at all. Collective mode is
    // opt-in, so an absent directory is the common case and walks to nothing.
    let resolutions_dir = crate::paths::yidam_sangha_dir(root).join("resolutions");
    let mut annotations: Vec<checks::Annotation> = Vec::new();
    for p in walk_md_files(&resolutions_dir) {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        annotations.extend(checks::annotations_in(&rel, &overlay.read(&p)));
    }

    // The seats the records name, and the seats the registry carries. Read through the
    // sangha report rather than re-parsed here: a second reading of "who is an elector" is
    // how a repository comes to be described two ways at once, and `electors.md` is a table
    // whose column order is the kind of thing that drifts.
    let sangha = crate::cmd::sangha::sangha_data(root);
    let registered: Vec<String> = sangha.electors.iter().map(|e| e.branch.clone()).collect();

    // RFC-0012's verification, and its condition is the registry's own declaration: a seat's
    // tip is verified when, and only when, its row binds a key. Both are empty in a corpus
    // that binds none — every corpus today — and neither touches git there.
    let attestations = attest::attest(root, &sangha.electors);
    let keys_bind_seats = attest::binds_distinct_key_per_seat(&sangha.electors);

    // Article V's node and edge clauses, decided against the tips each record names. The git
    // reading happens here, where there is a repository; the two checks that consume it are
    // pure, which is what lets the arm that has never fired in a real corpus be tested at all.
    // A repository with no resolutions — every corpus not running a sangha — spawns nothing.
    let scope_audits = scope::audit(root, &sangha.resolutions);

    // What `electors.md` said about each participating seat at that seat's own tip (#823).
    // Read here and not at HEAD: a seat's row is mutable and a model upgrade is material, so
    // HEAD would re-judge every past resolution the day somebody bumps a model. Like the
    // scope audit, the git reading happens where there is a repository and the check stays
    // pure. A tip this clone does not carry reads as `unrecorded`, which is already the
    // vocabulary's word for *the registry does not say*.
    let independence_audits = independence::audit(root, &sangha.resolutions);

    // Where each elector branch stands in the settled line, and what it says about where it
    // stands. Read here for the same reason the scope audit is: the checks stay pure, and the
    // refs are the one thing they cannot be handed off disk.
    let standings = lineage::standings(root, &sangha.resolutions);

    // What each seat's own branch says it is standing on (#294). Read from the branch and not
    // from the baseline, because a commitments file is never transported — it is an index of one
    // seat's own grounds, and a resolution that could reach for it would be synthesizing from
    // something no elector filed as a position. The git reading happens here for the same reason
    // the two above do: the checks stay pure over what was read.
    let commitments = commitments::read(root);

    // ── Prose links ─────────────────────────────────────────────────────────────
    //
    // Authored markdown, and what counts as authored is declared rather than hard-coded:
    // see [`crate::authorship`]. `.yidam/.vendor/` used to be named here as the single
    // exception, on a rationale that generalizes — a defect in the prelude is fixed
    // upstream and adopted by re-vendoring, so reporting one to a derived repo hands it a
    // finding it cannot act on. It is now the built-in instance of the general mechanism,
    // and a repository that is not a vendoring repository can say the same about a
    // generated directory or a frozen import of its own.
    //
    // `docs/` is included — documentation about the repository is authored, and its links
    // rot the same way. Not `crates/` or `web/`, whose READMEs carry illustrative targets
    // rather than references to files that are supposed to exist.
    // ── REGEN blocks (#524) ─────────────────────────────────────────────────────
    //
    // Every authored file the generators can write into. The walk is the prose-link walk
    // plus the repository README, which `yidam status` and `yidam vault-status` write and
    // which nothing else in this function reads. A file with no markers contributes nothing,
    // so a walk wider than the generators' own list costs a read and cannot miss a target —
    // which a list copied from the ten `update_file_regen` call sites would.
    let authorship = crate::authorship::Authorship::load_or_default(root);
    let mut prose_link_paths: Vec<std::path::PathBuf> = walk_linkable_files(&root.join(".yidam"));
    prose_link_paths.extend(walk_linkable_files(&root.join("docs")));

    let mut regen_files: Vec<(String, String)> = Vec::new();
    for p in prose_link_paths
        .iter()
        .chain([root.join("README.md")].iter().filter(|p| p.exists()))
    {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        // The same authorship rule the prose-link check applies: a finding in vendored
        // prelude content is one the derived repository cannot act on.
        if authorship
            .covering(&rel)
            .is_some_and(|r| !r.kind.reportable())
        {
            continue;
        }
        regen_files.push((rel, overlay.read(p)));
    }

    let mut prose_links: Vec<checks::ProseLink> = Vec::new();
    let mut unauthored: Vec<checks::UnauthoredLink> = Vec::new();
    for p in &prose_link_paths {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        let region = authorship.covering(&rel);
        // `excluded` is the one kind that means *do not look*; the file is not even read.
        if region.is_some_and(|r| !r.kind.reportable()) {
            continue;
        }
        let dir = p.parent().unwrap_or(root);
        let links = checks::prose_links(&rel, dir, &overlay.read(p));
        match region {
            Some(region) => unauthored.extend(
                links
                    .into_iter()
                    .map(|link| checks::UnauthoredLink { region, link }),
            ),
            None => prose_links.extend(links),
        }
    }
    let stale_regions = crate::authorship::stale(root, &authorship);

    // The links that also name a line, decided against the cited files — through the
    // overlay, so the buffer someone is editing a passage out of is the one the citation
    // is held to. Authored links only: a line citation in vendored or generated prose is
    // somebody else's to fix, the same judgement `unauthored-prose-link` records.
    let line_citations = line_citations::collect(root, &prose_links, &|p| overlay.read(p));

    // What this corpus has declared about its own gate. Absent — the common case, and the
    // case for every repository that has not yet argued about a number — escalates nothing.
    //
    // Read leniently: a malformed config must not take the checks down. The gate reports
    // the file as its own finding elsewhere; here, degrading to "no escalation" fails in
    // the direction of reporting rather than of failing a build on a number nobody set.
    let config = crate::config::load_yidam_config(root).unwrap_or_default();
    let escalate_after = config.lint.escalate_after;
    // The vault names an artifact record is allowed to route to. Read straight from the
    // config rather than through `vault::resolve`, deliberately: `resolve` enforces the
    // one-vault rule, and a corpus that has declared two has a configuration problem rather
    // than a *catalog* problem. Reporting every artifact as unroutable because a second
    // vault exists would blame the records for something they did not do.
    let declared_vaults: Vec<String> = config.vault.keys().cloned().collect();

    // The types `crates/` defines, for the one check whose subject is the ontology and whose
    // evidence is the code. Read through the overlay like everything else, so the editor
    // resolves a class against the buffer somebody is deleting a struct out of.
    //
    // **Only when a class asked.** The walk is skipped entirely where no class declares
    // `implemented_by:`, which is every corpus measured and every corpus that predates the
    // field — so a repository that never opted in pays nothing for a check that would report
    // nothing.
    //
    // Authorship regions are deliberately *not* consulted. Elsewhere a region says whose
    // finding a file's contents are; here the finding's subject is a class in this
    // repository's own ontology, and a type is evidence that it exists wherever it lives. A
    // generated implementation is still an implementation.
    let types = match classes.iter().any(|c| c.implemented_by.is_some()) {
        false => checks::TypeIndex::build([]),
        true => {
            let paths = walk_rust_files(&root.join("crates"));
            let texts: Vec<(String, String)> = paths
                .iter()
                .map(|p| {
                    (
                        p.strip_prefix(root)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .to_string(),
                        overlay.read(p),
                    )
                })
                .collect();
            checks::TypeIndex::build(texts.iter().map(|(r, t)| (r.as_str(), t.as_str())))
        }
    };

    // Nodes and classes both: a malformed evidence tag is a defect of prose, and a class file
    // carries prose. Bound here rather than inline because the view borrows from both.
    let tag_prose = checks::prose_views(&nodes, &classes);

    // One walk of the citations, four readings of it — the same predicate `check_citation`
    // answers from over MCP (#357). Destructured here rather than pushed after the vec, so
    // the four keep their place in the report's order.
    let [unresolved, span_drift, pin_moved, unpinned] = citations::checks(&nodes, &deps);
    // The other direction of the same join: a node resting on a verbatim span of another
    // node in this corpus (RFC-0034). No dependency, no network, no pin — which is why it
    // is the arm every corpus can actually use, and the external four have never had a
    // subject in any measured corpus.
    let [local_unresolved, local_span_drift, local_tag_drift, local_untagged] =
        local_citations::checks(&nodes, &claim_fields);
    // The graph's own half of the same discipline (#587): an edge is a claim written as
    // structure, and these are the checks that ask it what it rests on. The third compares
    // that standing to the ones its own endpoints declare (#858), which is why the claim
    // fields go in — a node's standing is a property its class declared `type: claim`.
    let [edge_untagged, edge_verified_unsourced, edge_standing_unheld] =
        edge_claims::checks(&nodes, &universal, &claim_fields);
    let [scope_unheld, scope_unverifiable] = scope::checks(&scope_audits);
    let [baseline_unmet, baseline_undeclared, holds_unadopted] = lineage::checks(&standings);
    let [commitments_absent, commitments_malformed, position_unindexed, commitment_vanished] =
        commitments::checks(&commitments);

    // ── this list is complete, and the compiler is what says so (#680) ────────────
    //
    // It is hand-written, which looks like the classic hole: a `pub fn … -> Check` added to
    // `checks.rs` and not added here would compile, pass its own unit tests, and never run,
    // and the corpus would report clean.
    //
    // It cannot. `mod cmd;` is private (`lib.rs:3`), `lint` and `checks` are `pub(crate)`, and
    // nothing re-exports them — so a check function no registry calls is unreachable from
    // outside the crate, and `dead_code` is an **error** under `ci-cli`'s `-D warnings`. Its
    // own unit test does not save it: the `lib` target is built without `cfg(test)`.
    //
    // That guarantee is a property of the privacy, not of this file, and nothing stated it
    // until `tests/lint_registry.rs` — which asserts each link of that chain, because the day
    // someone writes `pub use cmd::lint;` the hole opens with nothing going red.
    let mut all = vec![
        // First, because it is the finding that says whether the rest of the report is about
        // the corpus or about what serde made of a file it could not read.
        checks::malformed_yaml(&nodes, &classes),
        checks::missing_class(&nodes),
        checks::unknown_class(&nodes, &defined),
        checks::orphan_out(&nodes),
        checks::dangling_edge(&nodes),
        checks::undeclared_property(&nodes, &classes, &universal),
        checks::missing_property(&nodes, &classes),
        checks::node_too_long(&nodes, &classes, &prose_fields),
        checks::property_type(&nodes, &classes, &universal),
        checks::unimplemented_class(&classes, &types),
        checks::unlicensed_edge(&nodes, &classes),
        checks::edge_target_class(&nodes, &classes),
        edge_untagged,
        edge_verified_unsourced,
        edge_standing_unheld,
        unresolved,
        span_drift,
        pin_moved,
        unpinned,
        local_unresolved,
        local_span_drift,
        local_tag_drift,
        local_untagged,
        checks::verified_unsourced(&nodes, &sources, &claim_fields),
        checks::catalog_expired(&catalog_ages, &sources, &cites),
        checks::catalog_unobtained_but_cited(&sources, &cites),
        checks::name_not_a_slug(&nodes, &classes),
        checks::reference_not_in_the_grammar(&nodes),
        checks::missing_label(&nodes),
        checks::missing_description(&nodes, &prose_fields),
        checks::claim_tag_malformed(&tag_prose),
        checks::catalog_used_by_drift(&sources, &cites),
        checks::catalog_location_malformed(&sources),
        checks::catalog_artifact_malformed(&sources),
        checks::catalog_artifact_unroutable(&sources, &declared_vaults),
        checks::malformed_table(&prose),
        checks::malformed_regen_block(&regen_files),
        orphan_in_dated(root, &nodes, &classes),
        checks::catalog_uncited(&sources, &cites),
        checks::class_asserts_purpose(&classes),
        checks::class_claim_uncounted(&classes),
        checks::foundational_field_misspelled(&classes),
        checks::foundational_type_malformed(&classes),
        checks::resolution_annotation_malformed(&annotations),
        checks::resolution_annotation_decides(&annotations),
        checks::resolution_elector_unregistered(&sangha.resolutions, &registered),
        checks::resolution_executor_unrecorded(&sangha.resolutions, keys_bind_seats),
        independence::independence_mismatch(&independence_audits),
        attest::elector_signature_unverified(&attestations),
        scope_unheld,
        scope_unverifiable,
        baseline_unmet,
        baseline_undeclared,
        holds_unadopted,
        commitments_absent,
        commitments_malformed,
        position_unindexed,
        commitment_vanished,
        checks::broken_prose_link(&prose_links),
        line_citations::dead_line_citation(&line_citations),
        line_citations::slid_line_citation(&line_citations),
        line_citations::citation_label_not_cited(&line_citations),
        line_citations::unverified_line_citation(&line_citations),
        line_citations::citation_range_stated_twice(&line_citations),
        checks::unauthored_prose_link(&unauthored),
        checks::authorship_region_stale(&stale_regions),
        checks::policy_override(&policy_overrides),
    ];

    if opts.commits {
        let subjects = commits::read_subjects(root, opts.range.as_deref());
        // `[object] paths`, or one register if the repository declares no object — which is
        // every repository that has not written the key, and every one that ran this before
        // the key existed.
        let registers = crate::kuten::Registers::of_repo(root);
        all.push(commits::unrecognized_verb(&subjects, &registers));
    }

    // The corpus's threshold, handed to every check rather than to the one that dates.
    //
    // That reads like over-reach and is the opposite. [`Check::severity_of`] escalates only a
    // finding carrying an age, so a threshold on a check that dates nothing is inert — the
    // two forms are behaviourally identical, and this one has no second half to forget. The
    // call site used to read `orphan_in_dated(…).escalating_after(escalate_after)`, which made
    // opting in a separate act from dating: a check that grew a clock and not that suffix
    // would have reported itself escalation-eligible and escalated nothing, which is #774's
    // defect with the halves swapped. Eligibility is now declared once, by `Check::dated`, at
    // the point the ages are attached.
    for check in &mut all {
        check.escalate_after = escalate_after;
    }

    suppress_unparsed(&mut all, &nodes, &classes);
    all
}

/// Drop every finding about a file whose bytes did not parse, except the one that says so.
///
/// **The second half of [`checks::malformed_yaml`], and the half without which it makes the
/// report worse.** A file the parser rejected reached every other check as an empty record,
/// and what those checks reported is what they made of the emptiness rather than of the file:
/// one unclosed quote produced eight findings across six checks, opening with `missing-class`
/// against a file whose first line is a `class:` field. Adding a ninth finding and leaving the
/// eight in place would bury the only one that can be acted on.
///
/// **Applied to the report rather than to the inputs**, and that is the design decision. The
/// obvious alternative — keep the unparsed files out of the slices the checks are handed —
/// changes what those checks conclude about *other* files. `unlicensed-edge` and
/// `edge-target-class` resolve link targets through an index built from the node list, so
/// withholding one node would silently stop checking every well-formed node that points at
/// it. `citations` reads a node's markdown links straight out of its bytes and does not need
/// the parse at all, so withholding one would throw away citations that are still perfectly
/// legible. Both are new mistakes about files that parse, made in order to hide findings about
/// one that does not. Filtering the findings themselves cannot do that: nothing but the
/// unreadable file's own entries changes.
///
/// **What still gets through, and should.** A check whose subject is a different file is left
/// alone even when the unreadable one is why it fired. Breaking one instance in
/// `examples/streamflow` leaves `catalog-used-by-drift` reporting that `usgs-nwis.md` claims a
/// node that does not cite it — because the citation is a `links:` entry, and while the file
/// does not parse there is no reading under which it does cite. That is a true statement about
/// what the corpus can be shown to say, it is a Warn and does not gate, and it goes away when
/// the quote is closed. Suppressing it would mean deciding that an unreadable file is evidence
/// for the claims made about it, which is the whole mistake #676 is about.
///
/// It also cannot fall behind the registry. A check added next year is covered without knowing
/// this function exists, which a per-check `if malformed { continue }` in thirty-seven places
/// could not promise for long.
fn suppress_unparsed(all: &mut [Check], nodes: &[checks::Node], classes: &[checks::Class]) {
    let unparsed: HashSet<&str> = nodes
        .iter()
        .filter(|n| n.malformed.is_some())
        .map(|n| n.rel.as_str())
        .chain(
            classes
                .iter()
                .filter(|c| c.malformed.is_some())
                .map(|c| c.rel.as_str()),
        )
        .collect();
    if unparsed.is_empty() {
        return;
    }
    for check in all.iter_mut().filter(|c| c.id != checks::MALFORMED_YAML) {
        check
            .violations
            .retain(|v| !unparsed.contains(file_of(&v.node)));
    }
}

/// The file a finding is about, where the finding names a line inside it.
///
/// `claim-tag-malformed` and the line-citation checks report against `path:line`, because the
/// point of those findings is to go and fix that line. Comparing the whole string against a
/// path would leave exactly those findings behind on an unreadable file — the ones scanning
/// its prose, which is the part of it that still reads.
/// Through [`json::node_line`], because the report already had to answer this: a second copy
/// of what a `path:line` identity means is how the suppression and the span come to disagree
/// about which file a finding names.
fn file_of(node: &str) -> &str {
    json::node_line(node).map_or(node, |(file, _)| file)
}

/// [`checks::orphan_in`], with each finding dated and aged.
///
/// The check is pure and stays that way; the history is read here, where there is a
/// repository to read it from. Three properties are deliberate:
///
/// **The replay runs only when there is something to date.** A corpus with no orphans has
/// nothing to explain, and the common case should not pay for the uncommon one.
///
/// **A date, not a day count.** An age in days is a function of when you ask, so the same
/// corpus would render differently every day and no golden could pin it — the same reason
/// `index-status` reports `built_at` and lets its client do the arithmetic.
///
/// **A commit count, which is not the same thing.** It is a function of HEAD rather than of
/// the wall clock, so it is reproducible from the repository alone, and it is the unit the
/// distinction is actually drawn in: a node uncited for five commits is a sweep in
/// progress, one uncited for two hundred is over-collection, and a percentage cannot tell
/// them apart. That count is what [`Check::severity_of`] escalates on.
fn orphan_in_dated(root: &Path, nodes: &[checks::Node], classes: &[checks::Class]) -> Check {
    // Declared before the early return, because eligibility is a property of the check and
    // not of what this run happened to find. A corpus with no orphans still wants to be told
    // that arming `escalate_after` would reach this check and nothing else (#774).
    let mut check = checks::orphan_in(nodes, classes).dated();
    if check.violations.is_empty() {
        return check;
    }
    let ages = history::uncited_age(root);
    for v in &mut check.violations {
        let Some(age) = ages.get(&v.node).filter(|a| a.ts > 0) else {
            continue;
        };
        // Reuses the exporters' civil-date conversion rather than adding a second one; the
        // calendar arithmetic is the kind that is wrong in one copy and right in the other.
        // The clock half is dropped — a day is the resolution anyone reads an orphan's age
        // at.
        let iso = crate::cmd::export::unix_to_iso(age.ts as u64);
        let day = iso.split('T').next().unwrap_or(&iso);
        v.detail = format!(
            "{} — uncited since {day}, {} commit(s)",
            v.detail, age.commits
        );
        v.age = Some(age.clone());
    }
    check
}

/// Today as `YYYY-MM-DD`.
///
/// The single point at which the wall clock enters this command. Everything downstream takes
/// the date as an argument, so a report is testable and a golden is stable — the one thing a
/// wall-clock feature must not do is make its own tests depend on the day they run.
fn today_iso() -> String {
    let iso = crate::cmd::export::unix_to_iso(crate::dates::today_days() as u64 * 86_400);
    iso.split('T').next().unwrap_or_default().to_string()
}

/// Write the baseline for this run, carrying forward the clock on entries that already
/// stood.
///
/// One place, so `--bless` and `--init-baseline` cannot come to disagree about what a
/// blessing preserves — and what it preserves is the part that constrains it.
fn bless(root: &Path, all: &[Check]) -> Result<baseline::Baseline> {
    let previous = baseline::Baseline::load(root)?;
    let head = history::corpus_commits(root)
        .last()
        .cloned()
        .unwrap_or_default();
    let b = baseline::Baseline::from_checks(all, &previous, &head);
    b.write(root)?;
    Ok(b)
}

pub fn lint(opts: Options) -> Result<()> {
    let root = repo_root()?;
    // Same reason as `graph_check`: `lint` reported "0 finding(s), no errors" from an empty
    // directory that was not a repository at all.
    crate::paths::require_yidam_repo(&root)?;
    // Read the manifest here, where there is an error channel. `run_checks` degrades to the
    // built-ins so the editor keeps answering mid-edit; a gate that did the same would
    // re-scan every region the file declares and report the flood as a corpus that got
    // worse, rather than as a file with a typo in it.
    crate::authorship::Authorship::load(&root)?;
    let all = run_checks(&root, &opts);

    // JSON short-circuits the prose path entirely rather than interleaving with it: the
    // text output is a contract of its own (byte-identical to what it has always been),
    // and a report that half-prints is worse than either.
    //
    // The EXIT CODE is deliberately shared. A gate that gates differently depending on how
    // you asked for the answer is not a gate, so the verdict below is computed the same way
    // and returned the same way in both modes — only the rendering differs.
    if opts.format.is_json() {
        return lint_json(&root, &all, &opts);
    }

    if opts.init_baseline {
        if baseline::path(&root).exists() {
            println!(
                "{} already exists — left alone",
                baseline::path(&root).display()
            );
            return Ok(());
        }
        let b = bless(&root, &all)?;
        let count: usize = b.violations.values().map(|v| v.len()).sum();
        println!(
            "wrote {count} inherited violation(s) into {}",
            baseline::path(&root).display()
        );
        println!(
            "this records what was already true today, so that the next one is attributable\n\
             to the commit that introduced it — it does not fix anything"
        );
        return Ok(());
    }

    if opts.bless {
        let b = bless(&root, &all)?;
        let count: usize = b.violations.values().map(|v| v.len()).sum();
        println!(
            "blessed {count} error-severity violation(s) into {}",
            baseline::path(&root).display()
        );
        println!(
            "this records the corpus's current state as its inherited debt — it does not fix it"
        );
        return Ok(());
    }

    report(&all, &opts);

    let committed = baseline::Baseline::load(&root)?;
    let corpus_commits = history::corpus_commits(&root);
    let d = baseline::diff(&all, &committed, &corpus_commits);

    if opts.warn_only {
        let n: usize = all.iter().map(|c| c.violations.len()).sum();
        eprintln!("lint: {n} finding(s) (reported, not failing)");
        return Ok(());
    }

    if d.is_clean() {
        let n: usize = all.iter().map(|c| c.violations.len()).sum();
        let errs: usize = all
            .iter()
            .map(|c| c.violations.iter().filter(|v| c.gates(v)).count())
            .sum();
        if errs > 0 {
            println!("lint: {n} finding(s); {errs} error(s), all baselined — no regression");
        } else {
            println!("lint: {n} finding(s), no errors");
        }
        return Ok(());
    }

    if !d.introduced.is_empty() {
        eprintln!("\nnot in the baseline — introduced by this change:");
        for (check, node) in &d.introduced {
            eprintln!("  [{check}] {node}");
        }
    }
    if !d.expired.is_empty() {
        eprintln!("\nbaselined, and out of time — the corpus agreed to deal with these:");
        for e in &d.expired {
            eprintln!(
                "  [{}] {} — baselined {} commit(s) ago",
                e.check, e.node, e.commits
            );
        }
        // Deliberately does not offer `--bless`. Blessing carries the original `since`
        // forward rather than restamping it, so it would print a reassuring line and
        // change nothing — the two ways out are to fix the finding or to argue, in the
        // file, for more time.
        eprintln!(
            "\nA baseline is a scheduled repayment, not a permanent exemption, and blessing\n\
             again will not clear these — the clock runs from when the debt was first\n\
             accepted. Fix them, or raise `expire_after` in the baseline and say in the\n\
             commit message why this corpus needs longer than it said it did."
        );
    }
    if !d.resolved.is_empty() {
        eprintln!("\nin the baseline but no longer occurring — the baseline is stale:");
        for (check, node) in &d.resolved {
            eprintln!("  [{check}] {node}");
        }
        eprintln!(
            "\nfixing a violation is good; leaving it listed is not. A baseline permitted to be\n\
             wrong drifts, and one that over-lists silently re-permits what it over-lists."
        );
    }
    // Only when blessing would actually do something. An expired entry is not fixed by
    // re-recording it, and telling somebody otherwise sends them round a loop.
    if !d.introduced.is_empty() || !d.resolved.is_empty() {
        eprintln!("\nrun `yidam lint --bless` to record the current state as the new baseline.");
    }
    anyhow::bail!(
        "lint: {} introduced, {} expired, {} stale",
        d.introduced.len(),
        d.expired.len(),
        d.resolved.len()
    )
}

/// The JSON path: same checks, same baseline, same verdict, same exit code.
fn lint_json(root: &Path, all: &[Check], opts: &Options) -> Result<()> {
    if opts.init_baseline && baseline::path(root).exists() {
        return crate::report::emit(
            root,
            serde_json::json!({
                "blessed": { "recorded_violations": 0, "wrote": false,
                             "path": baseline::path(root).display().to_string() }
            }),
        );
    }

    if opts.bless || opts.init_baseline {
        // Blessing writes a file and reports what it recorded; there is no gate to
        // report on, and pretending otherwise would put `passed: true` on a run that
        // checked nothing.
        let b = bless(root, all)?;
        let recorded: usize = b.violations.values().map(|v| v.len()).sum();
        return crate::report::emit(
            root,
            serde_json::json!({
                "blessed": { "recorded_violations": recorded, "wrote": true,
                             "path": baseline::path(root).display().to_string() }
            }),
        );
    }

    let committed = baseline::Baseline::load(root)?;
    let corpus_commits = history::corpus_commits(root);
    let d = baseline::diff(all, &committed, &corpus_commits);
    crate::report::emit(root, json::build(root, all, &committed, &d))?;

    // Same verdict as the text path, and the same silence about it on success.
    if opts.warn_only || d.is_clean() {
        return Ok(());
    }
    anyhow::bail!(
        "lint: {} introduced, {} expired, {} stale",
        d.introduced.len(),
        d.expired.len(),
        d.resolved.len()
    )
}

fn report(all: &[Check], opts: &Options) {
    for check in all {
        if check.passed() {
            continue;
        }
        // The block is headed at the *highest* severity it contains, not at the check's
        // declared one. A check that is Info because a young finding is usually fine must
        // not print INFO above one that has aged into failing the build.
        println!(
            "\n{} [{}] {} — {} finding(s)",
            check.effective_severity().as_str().to_uppercase(),
            check.id,
            check.title,
            check.violations.len()
        );
        if opts.explain {
            println!("  {}", check.rationale);
            // Only where it is true, and only under `--explain`: a line on every block saying
            // a check *cannot* escalate would be forty-odd lines of nothing. The whole
            // population is readable from `--format json`, which reports passing checks too;
            // this is the answer for the check somebody is already looking at (#774).
            match (check.escalation_eligible, check.escalate_after) {
                (true, Some(n)) => println!(
                    "  these findings carry a clock: past {n} corpus commit(s) one escalates \
                     to an error"
                ),
                (true, None) => println!(
                    "  these findings carry a clock, so `[lint] escalate_after` would reach \
                     them; unset, nothing escalates"
                ),
                (false, _) => {}
            }
        }
        for v in &check.violations {
            // Marked per finding, because within one escalated block the escalated
            // findings and their younger siblings are printed side by side and the header
            // can no longer distinguish them.
            let escalated = if check.severity_of(v) != check.severity {
                format!(" [{}]", check.severity_of(v).as_str().to_uppercase())
            } else {
                String::new()
            };
            println!("  {}: {}{escalated}", v.node, v.detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A minimal well-formed corpus: two nodes pointing at each other.
    fn clean_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        let class = corpus.join("reach");
        fs::create_dir_all(&class).unwrap();
        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        fs::write(
            class.join("alpha.yml"),
            "class: reach\nlabel: Alpha\ndescription: A.\nlinks:\n  - target: beta.yml\n    relationship: refines\n",
        )
        .unwrap();
        fs::write(
            class.join("beta.yml"),
            "class: reach\nlabel: Beta\ndescription: B.\nlinks:\n  - target: alpha.yml\n    relationship: refines\n",
        )
        .unwrap();
        tmp
    }

    /// A buffer for a file that is not on disk is linted as a node.
    ///
    /// The walk is what every check reads its paths from, so before #607 a buffer that had
    /// never been saved was a buffer no check saw: its findings were exactly none, which is
    /// what a clean node's are. The dangling edge here is the difference made visible — it
    /// exists only in the overlay, and only a node the checks enumerated could have raised it.
    #[test]
    fn a_buffer_that_is_not_a_file_yet_is_still_a_node() {
        let tmp = clean_repo();
        let corpus = tmp.path().join(".yidam/corpus");
        let unsaved = corpus.join("reach/gamma.yml");
        assert!(!unsaved.exists());

        let mut overlay = Overlay::default();
        overlay.set(
            unsaved.clone(),
            "class: reach\nlabel: Gamma\ndescription: G.\nlinks:\n  - target: gone.yml\n    relationship: refines\n"
                .to_string(),
        );
        assert_eq!(overlay.unsaved_instances(&corpus), vec![unsaved.clone()]);

        let all = run_checks_with(tmp.path(), &Options::default(), &overlay);
        let dangling = all.iter().find(|c| c.id == "dangling-edge").unwrap();
        assert!(
            dangling
                .violations
                .iter()
                .any(|v| v.node.contains("reach/gamma.yml")),
            "{:?}",
            dangling.violations
        );
        // Nothing was written: the verdict is about a buffer and the tree is as it was.
        assert!(!unsaved.exists());
    }

    /// The predicate is the walker's, not a looser one.
    ///
    /// A class file, a buffer outside the corpus, a `.md` beside the nodes, and a buffer at
    /// the corpus root are each things the walk would not return, so the overlay must not
    /// return them either — or an editor with a `README.md` open under `.yidam/corpus/`
    /// would lint it as a node. A saved file is the walk's and is not repeated.
    #[test]
    fn unsaved_instances_apply_the_walkers_predicate() {
        let tmp = clean_repo();
        let corpus = tmp.path().join(".yidam/corpus");
        let mut overlay = Overlay::default();
        for p in [
            corpus.join("new.ont.yml"),
            corpus.join("root-level.yml"),
            corpus.join("reach/notes.md"),
            tmp.path().join("elsewhere/reach/x.yml"),
            corpus.join("reach/alpha.yml"),
        ] {
            overlay.set(p, String::new());
        }
        assert!(overlay.unsaved_instances(&corpus).is_empty());
    }

    /// A repository that has overridden nothing reports the check and no findings.
    #[test]
    fn a_corpus_with_no_policy_of_its_own_reports_no_override() {
        let tmp = clean_repo();
        let all = run_checks(tmp.path(), &Options::default());
        let c = all
            .iter()
            .find(|c| c.id == "policy-override")
            .expect("the check must report even when it passes");
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// **An override is visible and does not gate.** RFC-0024 settled that a local rule
    /// decides; this exists so that deciding cannot be done quietly, which is the remedy
    /// `.yidam/private-paths` applied to itself.
    #[test]
    fn an_overridden_decision_is_reported_at_info_and_gates_nothing() {
        let tmp = clean_repo();
        let dir = tmp.path().join(".yidam/policy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("record.rego"),
            r#"package yidam.disclose.record

decision := {"allow": true, "deny": []}
"#,
        )
        .unwrap();

        let all = run_checks(tmp.path(), &Options::default());
        let c = all.iter().find(|c| c.id == "policy-override").unwrap();
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(c.violations[0].detail.contains("disclose/record"));
        assert!(c.violations[0].node.contains("record.rego"));
        assert_eq!(c.severity, Severity::Info);
        assert_eq!(c.violations[0].severity, Some(Severity::Info));
        // The whole point: it does not gate.
        assert_eq!(errors(&all), 0, "an override must not fail the build");
    }

    /// A policy that does not compile is **not** an empty override list.
    ///
    /// Swallowing the error here would turn a rule nobody can evaluate into a clean gate.
    /// `doctor` is where that failure is reported, and it reports it as a failure.
    #[test]
    fn a_policy_that_does_not_compile_is_not_reported_as_having_no_overrides() {
        let tmp = clean_repo();
        let dir = tmp.path().join(".yidam/policy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("broken.rego"),
            "package yidam.disclose.record
{{{
",
        )
        .unwrap();

        // The gate still runs and still reports every check — lint's subject is the corpus,
        // and a broken policy is not a corpus defect. What must NOT happen is the broken rule
        // being reported as "no overrides", which reads as a clean repository.
        let all = run_checks(tmp.path(), &Options::default());
        let c = all.iter().find(|c| c.id == "policy-override").unwrap();
        assert!(c.violations.is_empty());
        // `doctor` is the surface that calls it a failure; see `tests/doctor.rs`.
    }

    /// Findings that gate — per violation, because residence time can escalate one
    /// finding of an Info check without escalating the check. Counting by `c.severity`
    /// here reported zero errors on a corpus the gate was failing.
    fn errors(checks: &[Check]) -> usize {
        checks
            .iter()
            .map(|c| c.violations.iter().filter(|v| c.gates(v)).count())
            .sum()
    }

    #[test]
    fn a_clean_corpus_produces_no_errors() {
        let tmp = clean_repo();
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(errors(&all), 0, "{all:#?}");
    }

    #[test]
    fn every_check_reports_even_when_it_passes() {
        // A check that vanishes when it passes cannot be told from one that did not run.
        let tmp = clean_repo();
        let all = run_checks(tmp.path(), &Options::default());

        // Against a repository that *has* findings, rather than against a number written
        // here. The invariant is that a passing run reports the same checks a failing one
        // does; the literal was 46 and only ever recorded how many existed the day it was
        // typed.
        let dirty = repo_with_an_aged_orphan(6);
        let dirty_ids: HashSet<&str> = run_checks(dirty.path(), &Options::default())
            .iter()
            .map(|c| c.id)
            .collect();
        let clean_ids: HashSet<&str> = all.iter().map(|c| c.id).collect();
        assert_eq!(
            clean_ids, dirty_ids,
            "a passing run reports fewer checks than a failing one"
        );
        assert_eq!(all.len(), clean_ids.len(), "a check id was reported twice");
        let ids: HashSet<&str> = all.iter().map(|c| c.id).collect();
        assert!(ids.contains("dangling-edge"));
        assert!(ids.contains("catalog-used-by-drift"));
        assert!(ids.contains("class-asserts-purpose"));
        // Reported in a repository that has overridden nothing, which is every corpus until
        // somebody writes a rule — the same reason every other check here reports when it
        // passes: a check that vanishes cannot be told from one that did not run.
        assert!(ids.contains("policy-override"));
        // Reported in a repository with no sangha at all, which is the common case: a
        // check that disappears when there is nothing to check cannot be told from one
        // that was never wired in.
        assert!(ids.contains("resolution-annotation-malformed"));
        // Same reason, and the pair that reads the record's frontmatter rather than its
        // prose: a repository with no `electors.md` at all still hears both answer.
        assert!(ids.contains("resolution-elector-unregistered"));
        assert!(ids.contains("resolution-executor-unrecorded"));
        // Same reason again, and this one is the most silent of all: RFC-0012's verification
        // is vacuous until a registry row binds a signing key, so a check that vanished when
        // it found no keys would be indistinguishable from one nobody wired in.
        assert!(ids.contains("elector-signature-unverified"));
        assert!(ids.contains("resolution-annotation-decides"));
        assert!(ids.contains("broken-prose-link"));
        assert!(ids.contains("unauthored-prose-link"));
        assert!(ids.contains("claim-tag-malformed"));
        // Reported in a repository whose REGEN blocks are all well formed, which is the
        // state every corpus is in until one is edited by hand.
        assert!(ids.contains("malformed-regen-block"));
        assert!(ids.contains("authorship-region-stale"));
        // The class contract. `clean_repo`'s ontology declares neither properties nor
        // edges, so all five pass here — which is the case worth pinning: silence is not a
        // contract, and a corpus whose ontology is not filled in must not be flooded.
        assert!(ids.contains("undeclared-property"));
        assert!(ids.contains("missing-property"));
        assert!(ids.contains("property-type"));
        assert!(ids.contains("unlicensed-edge"));
        assert!(ids.contains("edge-target-class"));
        // And the sixth, which reports in a repository with no `crates/` at all — the
        // common case, and the one where a check that vanished when it had nothing to read
        // could not be told from one that was never wired in.
        assert!(ids.contains("unimplemented-class"));
    }

    /// **Every rationale is prose, and the prose ships.** `--explain` prints it, `--format
    /// json` emits it as `checks[].rationale`, and the report golden the SDKs test against
    /// carries it verbatim — so a slip in a string literal here is a slip in a contract that
    /// is versioned separately from this binary. `unimplemented-class` shipped two lines of a
    /// `Class` struct literal spliced into the middle of its opening sentence (#709) and
    /// nothing complained: the literal is not raw, so an unindented newline and a
    /// `foundational_type: None,` are as valid to the compiler as any other bytes.
    ///
    /// Two shapes, both cheap. A **lone newline** is what unindented source pasted into a
    /// flowed paragraph looks like — the only break any rationale wants is the blank line
    /// between paragraphs, and two of them use it. And **Rust tokens outside backticks**: a
    /// rationale names `implemented_by:` and `vec![]` freely inside a code span, so the spans
    /// come out before the scan rather than the tokens being banned outright.
    #[test]
    fn no_rationale_carries_source_that_leaked_out_of_a_code_span() {
        let tmp = clean_repo();
        let opts = Options {
            // The commit check is the one not in the vec unconditionally, and its rationale
            // ships like every other.
            commits: true,
            ..Options::default()
        };
        for c in run_checks(tmp.path(), &opts) {
            let r = c.rationale;
            assert!(
                !r.replace("\n\n", "").contains('\n'),
                "{}: the rationale breaks a line outside a paragraph break, which is what a \
                 pasted-in fragment looks like:\n{r}",
                c.id
            );
            assert_eq!(
                r.matches('`').count() % 2,
                0,
                "{}: an unbalanced backtick — the code spans cannot be told from the prose",
                c.id
            );
            // Even segments are outside the spans; odd ones are the spans themselves.
            let prose = r.split('`').step_by(2).collect::<Vec<_>>().join(" ");
            for token in ["vec!", ": None", ": Some", "::", "->", "=>"] {
                assert!(
                    !prose.contains(token),
                    "{}: `{token}` is Rust, and it is in the rationale outside a code \
                     span:\n{prose}",
                    c.id
                );
            }
        }
    }

    fn check<'a>(all: &'a [Check], id: &str) -> &'a Check {
        all.iter().find(|c| c.id == id).expect(id)
    }

    // ── #676: a file that does not parse ────────────────────────────────────────

    /// A corpus whose class declares a property its instances do not carry.
    ///
    /// `schema` is the whole class file, so an arm can hand over a sound one or one with a
    /// typo in it and change *nothing else*. The instance defect is identical either way,
    /// which is what makes the pair a measurement rather than two fixtures.
    fn repo_with_a_declared_property(schema: &str) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        let class = corpus.join("gage");
        fs::create_dir_all(&class).unwrap();
        fs::write(corpus.join("gage.ont.yml"), schema).unwrap();
        for (name, other) in [
            ("canyon-outlet", "valley-bridge"),
            ("valley-bridge", "canyon-outlet"),
        ] {
            fs::write(
                class.join(format!("{name}.yml")),
                format!(
                    "class: gage\nlabel: {name}\ndescription: A gage.\nlinks:\n  \
                     - target: {other}.yml\n    relationship: refines\n"
                ),
            )
            .unwrap();
        }
        tmp
    }

    /// Declares one property, so an instance without it is a finding.
    const SOUND_SCHEMA: &str =
        "class: gage\nlabel: Gage\nproperties:\n  - name: parameter\n    type: string\n";
    /// The same file, with one unclosed quote in `label:`. Nothing else differs.
    const BROKEN_SCHEMA: &str =
        "class: gage\nlabel: \"Gage\nproperties:\n  - name: parameter\n    type: string\n";

    /// **The controlled pair from #676, and the arm that reported nothing at all.**
    ///
    /// A one-character typo in a class file dropped every declared property, which switched
    /// off each check that reads them — `missing-property`, `undeclared-property`,
    /// `property-type`, `edge-target-class`, `unlicensed-edge` — and left `lint` printing
    /// `0 finding(s), no errors` over a corpus carrying the defect the sound arm reports. A
    /// false negative in a gate, produced by a typo, and nothing in the report could say it
    /// had happened.
    ///
    /// The assertion is deliberately about the *dependent* check rather than about the parse:
    /// a check that grepped the bytes for a broken quote would satisfy half this test while
    /// reading nothing, so both arms are measured through `missing-property`.
    #[test]
    fn a_class_file_that_does_not_parse_gates_instead_of_declaring_nothing() {
        let sound = repo_with_a_declared_property(SOUND_SCHEMA);
        let all = run_checks(sound.path(), &Options::default());
        assert_eq!(
            check(&all, "missing-property").violations.len(),
            2,
            "the sound arm must have a finding to lose"
        );
        assert!(check(&all, "malformed-yaml").passed());

        let broken = repo_with_a_declared_property(BROKEN_SCHEMA);
        let all = run_checks(broken.path(), &Options::default());
        let c = check(&all, "malformed-yaml");
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(
            c.violations[0].node.ends_with("gage.ont.yml"),
            "{}",
            c.violations[0].node
        );
        // What the unreadable schema actually cost, stated rather than inferred: the class
        // declares nothing the tool can see, so the check that reads the declaration has
        // nothing to report and is right not to.
        assert!(check(&all, "missing-property").passed());
        // And the whole point — this run used to exit 0.
        assert!(errors(&all) > 0, "a schema nobody can read must gate");
    }

    /// **The instance arm: one finding, and the rest of the report stays quiet about it.**
    ///
    /// One unclosed quote used to produce eight findings across six checks, opening with
    /// `missing-class` against a file whose first line is a `class:` field. Every one of them
    /// described the empty record `serde_yaml` returned rather than the file. Adding a ninth
    /// finding and leaving the eight would have buried the only one worth reading.
    ///
    /// The prose carries a near-miss evidence tag, and that is the load-bearing part of the
    /// fixture rather than decoration. `claim-tag-malformed` reads the *bytes*, so it is the
    /// one check with something to say about a file the parser rejected — and it reports
    /// against `alpha.yml:3`. The sound arm pins that it fires at all; without it this case
    /// would pass against a suppression that never handled a finding naming a line.
    #[test]
    fn a_node_that_does_not_parse_gets_one_finding_and_no_others() {
        let rel = ".yidam/corpus/reach/alpha.yml";
        let tagged = |label: &str| {
            format!("class: reach\nlabel: {label}\ndescription: Settled [verified — Pearl 2009].\n")
        };

        let sound = clean_repo();
        fs::write(sound.path().join(rel), tagged("Alpha")).unwrap();
        let tag = check(
            &run_checks(sound.path(), &Options::default()),
            "claim-tag-malformed",
        )
        .violations
        .iter()
        .map(|v| v.node.clone())
        .collect::<Vec<_>>();
        assert_eq!(tag, vec![format!("{rel}:3")], "the finding this suppresses");

        // The same file with one unclosed quote in `label:`.
        let tmp = clean_repo();
        fs::write(tmp.path().join(rel), tagged("\"Alpha")).unwrap();
        let all = run_checks(tmp.path(), &Options::default());

        let c = check(&all, "malformed-yaml");
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert_eq!(c.violations[0].node, rel);
        // The parser's own reason, which is what makes the finding actionable: it names
        // line 2 column 8, where the quote was opened.
        assert!(
            c.violations[0].detail.contains("line 2 column 8"),
            "{}",
            c.violations[0].detail
        );

        // `starts_with` and deliberately not `file_of`: an assertion written through the
        // function under test agrees with it by construction, and a `path:line` finding
        // slipping past the suppression is exactly what it would then fail to see.
        let elsewhere: Vec<(&str, &str)> = all
            .iter()
            .filter(|c| c.id != checks::MALFORMED_YAML)
            .flat_map(|c| c.violations.iter().map(move |v| (c.id, v.node.as_str())))
            .filter(|(_, node)| node.starts_with(rel))
            .collect();
        assert!(
            elsewhere.is_empty(),
            "a file nobody could read is described by these too: {elsewhere:?}"
        );
    }

    /// The suppression is about one file, not about the run.
    ///
    /// The guard on the case above: a filter that reached one finding too far would make an
    /// unreadable node into a way of quieting the gate about everything beside it, which is
    /// the defect #676 reports rather than a fix for it.
    #[test]
    fn a_neighbour_that_does_not_parse_does_not_quiet_the_findings_about_a_file_that_does() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: \"Alpha\n",
        )
        .unwrap();
        // A real, unrelated defect on a file that parses perfectly.
        fs::write(
            tmp.path().join(".yidam/corpus/reach/beta.yml"),
            "class: reach\nlabel: Beta\ndescription: B.\nlinks:\n  \
             - target: nowhere.yml\n    relationship: refines\n",
        )
        .unwrap();

        let all = run_checks(tmp.path(), &Options::default());
        let dangling = check(&all, "dangling-edge");
        assert_eq!(dangling.violations.len(), 1, "{:?}", dangling.violations);
        assert!(dangling.violations[0].node.ends_with("beta.yml"));
    }

    /// A file with nothing in it has not contradicted anything.
    ///
    /// `serde_yaml` reads no bytes as the absent value rather than as a failure, and this pins
    /// that: `missing-class` and its siblings already describe an empty file correctly, and
    /// reporting it as unparseable would additionally suppress them. It is also what keeps
    /// `Overlay::read`'s unreadable-file-as-`""` out of this check's scope.
    #[test]
    fn an_empty_node_file_is_described_by_the_ordinary_checks_and_not_by_this_one() {
        let tmp = clean_repo();
        fs::write(tmp.path().join(".yidam/corpus/reach/alpha.yml"), "").unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "malformed-yaml").passed());
        assert!(check(&all, "missing-class")
            .violations
            .iter()
            .any(|v| v.node.ends_with("alpha.yml")));
    }

    /// A file with one link that goes nowhere, at `rel` under the repo.
    fn broken_link_at(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "See [the thing](./nowhere.md).\n").unwrap();
    }

    fn declare(root: &Path, body: &str) {
        fs::create_dir_all(root.join(".yidam")).unwrap();
        fs::write(root.join(crate::authorship::MANIFEST), body).unwrap();
    }

    /// The measured case: a directory whose own README says it is a frozen copy of an
    /// upstream project. The link is broken, it is real, and it is not this repo's to fix.
    #[test]
    fn a_broken_link_in_an_imported_region_is_reported_but_does_not_gate() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/reference/upstream/notes.md");
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/upstream/\n    from: acme/gis at the fork point\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());

        let scoped = check(&all, "unauthored-prose-link");
        assert_eq!(scoped.violations.len(), 1);
        assert_eq!(scoped.severity, Severity::Info);
        let detail = &scoped.violations[0].detail;
        assert!(detail.contains("acme/gis at the fork point"), "{detail}");
        assert!(detail.contains("falsify"), "{detail}");
        assert_eq!(errors(&all), 0);
    }

    /// A declared region is not a blanket finding: only links that actually fail to
    /// resolve are reported, exactly as in authored material.
    #[test]
    fn a_resolving_link_in_a_declared_region_is_not_a_finding() {
        let tmp = clean_repo();
        let dir = tmp.path().join("docs/reference/upstream");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("target.md"), "# There\n").unwrap();
        fs::write(dir.join("notes.md"), "See [there](./target.md).\n").unwrap();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/upstream/\n    from: acme/gis\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "unauthored-prose-link").passed());
    }

    /// The mutation. Delete the declaration and the same link is an error that gates —
    /// which is what makes the declaration, rather than the path, the thing doing the work.
    #[test]
    fn without_the_declaration_the_same_link_gates() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/reference/upstream/notes.md");
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(check(&all, "broken-prose-link").violations.len(), 1);
        assert!(check(&all, "unauthored-prose-link").passed());
        assert_eq!(errors(&all), 1);
    }

    /// `generated` names the generator, so the finding arrives addressed to it.
    #[test]
    fn a_generated_region_reports_against_the_generator() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), ".yidam/reports/coverage.md");
        declare(
            tmp.path(),
            "generated:\n  - path: .yidam/reports/\n    by: yidam report\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        let detail = &check(&all, "unauthored-prose-link").violations[0].detail;
        assert!(detail.contains("yidam report"), "{detail}");
        assert!(detail.contains("the generator's"), "{detail}");
    }

    /// The escape hatch, and the only kind that produces silence.
    #[test]
    fn an_excluded_region_is_not_read_at_all() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/scratch/notes.md");
        declare(
            tmp.path(),
            "excluded:\n  - path: docs/scratch/\n    why: working notes\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        assert!(check(&all, "unauthored-prose-link").passed());
    }

    /// The special case became an instance: no manifest, same treatment.
    #[test]
    fn the_vendored_prelude_needs_no_declaration() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), ".yidam/.vendor/prelude/GRAPH.md");
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        assert_eq!(check(&all, "unauthored-prose-link").violations.len(), 1);
    }

    #[test]
    fn a_declaration_that_matches_nothing_is_reported_and_does_not_gate() {
        let tmp = clean_repo();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/gone/\n    from: acme/gis\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        let stale = check(&all, "authorship-region-stale");
        assert_eq!(stale.violations.len(), 1);
        assert_eq!(stale.severity, Severity::Warn);
        // The manifest is the offending file, so an editor squiggles the entry itself.
        assert_eq!(stale.violations[0].node, crate::authorship::MANIFEST);
        assert_eq!(errors(&all), 0);
    }

    /// A manifest that exists and cannot be read must be reported as that, not absorbed
    /// into the flood of findings it was written to scope.
    #[test]
    fn an_unreadable_manifest_fails_the_command() {
        let tmp = clean_repo();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/x\n    from: a\n    why: b\n",
        );
        assert!(crate::authorship::Authorship::load(tmp.path()).is_err());
        // …while the checks themselves keep answering, for the editor's sake. Compared
        // against a repository whose manifest reads, so the number is derived rather than
        // written: the literal here was 46 and said nothing about the manifest.
        let all = run_checks(tmp.path(), &Options::default());
        let sound = clean_repo();
        assert_eq!(
            all.len(),
            run_checks(sound.path(), &Options::default()).len()
        );
    }

    /// A class declaring an implementation, and a `crates/` tree that may or may not hold it.
    fn with_implementation(root: &Path, declares: &str, defines: Option<&str>) {
        fs::write(
            root.join(".yidam/corpus/reach.ont.yml"),
            format!("class: reach\nimplemented_by: {declares}\n"),
        )
        .unwrap();
        if let Some(ty) = defines {
            let src = root.join("crates/domain/src");
            fs::create_dir_all(&src).unwrap();
            fs::write(src.join("lib.rs"), format!("pub struct {ty};\n")).unwrap();
        }
    }

    /// The half a unit test cannot see: `run_checks` must actually walk `crates/`. The check
    /// itself is pure and takes its index in, so it passes happily against an index nothing
    /// ever filled — which is what an unwired walk produces and what a green unit suite
    /// would keep reporting.
    #[test]
    fn a_declared_implementation_is_resolved_against_the_tree_on_disk() {
        let tmp = clean_repo();
        with_implementation(tmp.path(), "Reach", Some("Reach"));
        let all = run_checks(tmp.path(), &Options::default());
        assert!(
            check(&all, "unimplemented-class").passed(),
            "the type is on disk; a walk that read nothing would report it missing"
        );
        assert_eq!(errors(&all), 0, "{all:#?}");
    }

    #[test]
    fn a_declared_implementation_the_tree_lacks_gates_the_run() {
        let tmp = clean_repo();
        with_implementation(tmp.path(), "Reach", Some("SomethingElse"));
        let all = run_checks(tmp.path(), &Options::default());
        let c = check(&all, "unimplemented-class");
        assert_eq!(c.violations.len(), 1, "{:#?}", c.violations);
        assert_eq!(c.violations[0].node, ".yidam/corpus/reach.ont.yml");
        assert_eq!(errors(&all), 1, "the class contract is contradicted");
    }

    /// `target/` holds tens of thousands of generated files in a built repository, including
    /// the sources of every dependency. Resolving a class against one would let a corpus
    /// claim an implementation it does not have.
    #[test]
    fn a_type_in_a_build_directory_is_not_an_implementation() {
        let tmp = clean_repo();
        with_implementation(tmp.path(), "Reach", None);
        let dir = tmp.path().join("crates/target/debug/build/dep/src");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("lib.rs"), "pub struct Reach;\n").unwrap();
        assert!(
            !check(
                &run_checks(tmp.path(), &Options::default()),
                "unimplemented-class"
            )
            .passed(),
            "a build artifact is not this repository's implementation"
        );
    }

    /// The measured default, at the wiring level: no class declares the field, so the walk
    /// never happens and nothing is reported — which is every corpus that predates it.
    #[test]
    fn a_corpus_that_declares_no_implementation_reads_no_code() {
        let tmp = clean_repo();
        let src = tmp.path().join("crates/domain/src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "pub struct Unrelated;\n").unwrap();
        assert!(
            check(
                &run_checks(tmp.path(), &Options::default()),
                "unimplemented-class"
            )
            .passed(),
            "silence is not a contract; 129 of 157 measured classes name no type"
        );
    }

    /// The four citation checks are registered, and reach the reporting path a unit test
    /// cannot see. The unit tests in `citations` prove each one decides correctly; this
    /// proves `run_checks` actually calls them — the half that fails silently, because a
    /// check nobody runs looks exactly like a check that found nothing.
    #[test]
    fn an_unresolvable_citation_gates() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\ncites:\n  - package: nowhere\n    \
             node: concept/x\n    span: y\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        let cite = all
            .iter()
            .find(|c| c.id == "external-citation-unresolved")
            .expect("registered");
        assert_eq!(cite.violations.len(), 1);
        assert!(errors(&all) > 0, "an unresolvable citation must gate");
    }

    #[test]
    fn a_dangling_edge_is_an_error() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert!(errors(&all) > 0);
    }

    #[test]
    fn the_commit_check_runs_only_when_asked() {
        let tmp = clean_repo();
        let without = run_checks(tmp.path(), &Options::default());
        let with = run_checks(
            tmp.path(),
            &Options {
                commits: true,
                ..Default::default()
            },
        );
        assert_eq!(with.len(), without.len() + 1);
    }

    #[test]
    fn blessing_then_running_again_is_clean() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert!(errors(&all) > 0);

        baseline::Baseline::from_checks(&all, &super::baseline::Baseline::default(), "")
            .write(tmp.path())
            .unwrap();
        let again = run_checks(tmp.path(), &Options::default());
        let loaded = baseline::Baseline::load(tmp.path()).unwrap();
        assert!(baseline::diff(&again, &loaded, &[]).is_clean());
    }

    #[test]
    fn fixing_a_baselined_violation_makes_the_baseline_stale() {
        let tmp = clean_repo();
        let broken = tmp.path().join(".yidam/corpus/reach/alpha.yml");
        fs::write(
            &broken,
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        baseline::Baseline::from_checks(&all, &super::baseline::Baseline::default(), "")
            .write(tmp.path())
            .unwrap();

        // Repair it.
        fs::write(
            &broken,
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: beta.yml\n",
        )
        .unwrap();
        let after = run_checks(tmp.path(), &Options::default());
        let loaded = baseline::Baseline::load(tmp.path()).unwrap();
        let d = baseline::diff(&after, &loaded, &[]);
        assert!(!d.resolved.is_empty(), "the fix must show as stale");
        assert!(!d.is_clean());
    }

    // ── residence time, end to end ────────────────────────────────────────────

    /// A repository with one orphan and a history to age it against.
    ///
    /// `git` for real rather than a stubbed replay: the count comes from the commit graph,
    /// and a test that fabricated it would be asserting the arithmetic rather than the
    /// walk.
    fn repo_with_an_aged_orphan(commits: usize) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "T"]);

        let corpus = root.join(".yidam/corpus/reach");
        fs::create_dir_all(&corpus).unwrap();
        fs::write(root.join(".yidam/corpus/reach.ont.yml"), "class: reach\n").unwrap();
        let cited = "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: beta.yml\n    relationship: refines\n";
        fs::write(corpus.join("alpha.yml"), cited).unwrap();
        fs::write(
            corpus.join("beta.yml"),
            "class: reach\nlabel: B\ndescription: B.\nlinks:\n  - target: alpha.yml\n    relationship: refines\n",
        )
        .unwrap();
        // The orphan: it points out, and nothing ever points at it.
        fs::write(
            corpus.join("lonely.yml"),
            "class: reach\nlabel: L\ndescription: L.\nlinks:\n  - target: alpha.yml\n    relationship: refines\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "genesis: corpus"]);

        for i in 1..commits {
            fs::write(corpus.join("alpha.yml"), format!("{cited}# pass {i}\n")).unwrap();
            git(&["add", "-A"]);
            git(&["commit", "-q", "-m", &format!("scope: pass {i}")]);
        }
        tmp
    }

    fn orphan(all: &[Check]) -> &Check {
        check(all, "orphan-in")
    }

    /// The finding carries its clock, and the clock is in commits.
    #[test]
    fn an_orphan_finding_carries_its_residence_time() {
        let tmp = repo_with_an_aged_orphan(6);
        let all = run_checks(tmp.path(), &Options::default());
        let c = orphan(&all);
        let v = c
            .violations
            .iter()
            .find(|v| v.node.ends_with("lonely.yml"))
            .expect("the orphan is reported");
        assert_eq!(v.age.as_ref().map(|a| a.commits), Some(6));
        assert!(v.detail.contains("6 commit(s)"), "{}", v.detail);
    }

    /// The default. Six commits of neglect and the gate stays quiet, because nobody
    /// declared how long is too long.
    #[test]
    fn without_a_declared_threshold_an_aged_orphan_does_not_gate() {
        let tmp = repo_with_an_aged_orphan(6);
        let all = run_checks(tmp.path(), &Options::default());
        let c = orphan(&all);
        assert!(c.violations.iter().all(|v| !c.gates(v)));
        assert_eq!(errors(&all), 0, "{all:#?}");
    }

    /// Declared, and the same corpus now fails — on the finding that has outlived the
    /// number this repository chose for itself.
    #[test]
    fn a_declared_threshold_escalates_the_finding_that_outlived_it() {
        let tmp = repo_with_an_aged_orphan(6);
        fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[lint]\nescalate_after = 5\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        let c = orphan(&all);
        let v = c
            .violations
            .iter()
            .find(|v| v.node.ends_with("lonely.yml"))
            .unwrap();
        assert!(c.gates(v), "6 commits is past the declared 5");
        assert_eq!(c.severity, Severity::Info, "the check itself is unchanged");
        assert_eq!(errors(&all), 1);
    }

    /// A threshold the corpus has not reached leaves the gate exactly where it was.
    #[test]
    fn a_threshold_above_the_finding_leaves_it_alone() {
        let tmp = repo_with_an_aged_orphan(6);
        fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[lint]\nescalate_after = 500\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(errors(&all), 0, "{all:#?}");
    }

    /// An escalated finding is ordinary inherited debt: blessable, and quiet afterwards.
    /// This is what makes the mechanism usable on a corpus that adopts it mid-life —
    /// the case the sibling issue turns into a generated baseline.
    #[test]
    fn an_escalated_finding_can_be_blessed_like_any_other() {
        let tmp = repo_with_an_aged_orphan(6);
        fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[lint]\nescalate_after = 5\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        let base =
            super::baseline::Baseline::from_checks(&all, &super::baseline::Baseline::default(), "");
        assert_eq!(
            base.violations.get("orphan-in").map(Vec::len),
            Some(1),
            "only the escalated finding is recorded, not its younger siblings"
        );
        assert!(super::baseline::diff(&all, &base, &[]).is_clean());
    }

    /// A config that does not parse must not take the checks down with it. The gate loses
    /// escalation, which fails toward reporting rather than toward failing a build on a
    /// number nobody set.
    #[test]
    fn an_unparseable_config_degrades_to_no_escalation() {
        let tmp = repo_with_an_aged_orphan(6);
        fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[lint\nescalate_after =\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());

        // Compared against the same repository with a config that parses, rather than against
        // a number written here. The literal was 46 and went stale the next time a check was
        // added — which says nothing about escalation, and is the one thing this test is for.
        let sound = repo_with_an_aged_orphan(6);
        let expected = run_checks(sound.path(), &Options::default()).len();
        assert_eq!(all.len(), expected, "every check still ran");
        assert_eq!(errors(&all), 0);
    }

    /// A corpus that trips a broad set of checks at once.
    ///
    /// The two direction guards below are only as exhaustive as the population they see, and
    /// `repo_with_an_aged_orphan` trips exactly one check — under which a check dating its
    /// findings without declaring it would go unseen, which is the whole thing they are for.
    /// This one is deliberately wrong in several unrelated ways, so that a broad slice of the
    /// report is non-empty and the guards have something to be exhaustive over.
    fn repo_that_trips_many_checks() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "T"]);

        let corpus = root.join(".yidam/corpus/reach");
        fs::create_dir_all(&corpus).unwrap();
        fs::write(
            root.join(".yidam/corpus/reach.ont.yml"),
            "class: reach\nlabel: Reach\ndescription: A reach.\nproperties:\n  - name: depth\n    type: number\n    required: true\n",
        )
        .unwrap();
        // Points at a file that does not exist, and is pointed at by nothing.
        fs::write(
            corpus.join("alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: gone.yml\n    relationship: refines\n",
        )
        .unwrap();
        // No label, no description, no required property, and no links in either direction.
        fs::write(corpus.join("bare.yml"), "class: reach\n").unwrap();
        // A class nothing defines.
        fs::write(
            corpus.join("stray.yml"),
            "class: nowhere\nlabel: S\ndescription: S.\n",
        )
        .unwrap();
        // A name the grammar does not admit.
        fs::write(
            corpus.join("Not A Slug.yml"),
            "class: reach\nlabel: N\ndescription: N.\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "genesis: corpus"]);
        tmp
    }

    // ── which findings are dated (#774) ───────────────────────────────────────
    //
    // `escalate_after` is documented as a property of findings and reaches one check.
    // Nothing said which, so a derived corpus read the general prose, assumed the shape
    // filling its report was at risk, and declined the mechanism on a danger that did not
    // exist for it. `Check::escalation_eligible` is that population, and these three hold
    // the declaration to what the checks actually do — read off a real run rather than off a
    // list, because a list of eligible checks written here is a list that stops covering the
    // next one without ever going red.

    /// The checks that say they age.
    fn declared_eligible(all: &[Check]) -> std::collections::BTreeSet<&str> {
        all.iter()
            .filter(|c| c.escalation_eligible)
            .map(|c| c.id)
            .collect()
    }

    /// The checks that actually dated a finding on this run.
    fn observed_dating(all: &[Check]) -> std::collections::BTreeSet<&str> {
        all.iter()
            .filter(|c| c.violations.iter().any(|v| v.age.is_some()))
            .map(|c| c.id)
            .collect()
    }

    /// The direction that would be a lie about the gate: a check writing ages without
    /// declaring it escalates findings the report contract says it cannot reach.
    ///
    /// Exhaustive over whatever this corpus trips, which is what makes it worth more than the
    /// pin below — a check that grows a clock next year is caught here without anyone
    /// remembering this file exists, provided the fixture trips it.
    #[test]
    fn no_check_dates_a_finding_without_declaring_itself_eligible() {
        let aged = repo_with_an_aged_orphan(6);
        let broken = repo_that_trips_many_checks();
        let all: Vec<Check> = [aged.path(), broken.path()]
            .into_iter()
            .flat_map(|r| run_checks(r, &Options::default()))
            .collect();
        let undeclared: Vec<&str> = observed_dating(&all)
            .difference(&declared_eligible(&all))
            .copied()
            .collect();
        assert!(
            undeclared.is_empty(),
            "{undeclared:?} attach an age to their findings — so `escalate_after` escalates \
             them — while reporting `escalation_eligible: false`. Call `Check::dated()` where \
             the ages are attached."
        );
    }

    /// The other direction: a check promising a gate it cannot raise. Asked only of the
    /// checks that fired, because a check with no findings this run has dated nothing and
    /// that says nothing about it.
    #[test]
    fn an_eligible_check_that_fired_dated_at_least_one_finding() {
        let aged = repo_with_an_aged_orphan(6);
        let broken = repo_that_trips_many_checks();
        let all: Vec<Check> = [aged.path(), broken.path()]
            .into_iter()
            .flat_map(|r| run_checks(r, &Options::default()))
            .collect();
        let empty_promises: Vec<&str> = all
            .iter()
            .filter(|c| c.escalation_eligible && !c.passed())
            .filter(|c| c.violations.iter().all(|v| v.age.is_none()))
            .map(|c| c.id)
            .collect();
        assert!(
            empty_promises.is_empty(),
            "{empty_promises:?} report `escalation_eligible: true` and dated none of the \
             findings they raised, so no value of `escalate_after` can act on them"
        );
    }

    /// The population itself, pinned — which is the fact #774 says a corpus has no way to
    /// learn.
    ///
    /// A pin rather than a filter: a second dated check is *supposed* to fail this line, and
    /// updating it is how the docs that quote the population get updated with it.
    #[test]
    fn orphan_in_is_the_whole_escalation_eligible_population() {
        let tmp = repo_with_an_aged_orphan(6);
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(
            declared_eligible(&all),
            std::collections::BTreeSet::from(["orphan-in"]),
            "if a check was added or lost here, say so in docs/configuration.md and \
             prelude/GRAPH.md, which both name this set"
        );
    }

    /// The threshold now reaches every check rather than the one that dates, and that must
    /// not widen the gate by a single finding.
    ///
    /// Compared against the same repository unarmed rather than against severities written
    /// here: the interesting claim is that nothing outside the eligible set moved, and a
    /// literal would be asserting today's severities instead.
    #[test]
    fn arming_the_threshold_moves_only_the_eligible_checks() {
        // The broad corpus, not the aged orphan: the claim is about the checks that *cannot*
        // age, so the run has to contain some of them with findings to move.
        let tmp = repo_that_trips_many_checks();
        let severities = |all: &[Check]| -> Vec<(&'static str, Vec<Severity>)> {
            all.iter()
                .map(|c| {
                    (
                        c.id,
                        c.violations.iter().map(|v| c.severity_of(v)).collect(),
                    )
                })
                .collect()
        };

        let quiet = run_checks(tmp.path(), &Options::default());
        let before = severities(&quiet);

        fs::write(
            tmp.path().join(".yidam/config.toml"),
            // 1, so every dated finding in the corpus is past it. Anything that moves under
            // this moves under some threshold.
            "[lint]\nescalate_after = 1\n",
        )
        .unwrap();
        let armed = run_checks(tmp.path(), &Options::default());
        let after = severities(&armed);

        assert_eq!(before.len(), after.len(), "the same checks ran");
        let moved: Vec<&str> = before
            .iter()
            .zip(&after)
            .filter(|((_, b), (_, a))| b != a)
            .map(|((id, _), _)| *id)
            .collect();
        assert_eq!(
            moved,
            vec!["orphan-in"],
            "a threshold handed to a check that dates nothing must be inert"
        );
        assert_eq!(
            moved
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            declared_eligible(&armed),
            "what moved is exactly what the report says can move"
        );
    }

    // ── adoption ──────────────────────────────────────────────────────────────

    /// The case this exists for. A repository that has never blessed anything has no
    /// `lint-baseline.yml`, so the ratchet has nothing to ratchet against and reports
    /// clean forever — which is what a measured derived repository did while a third of
    /// its corpus was unreachable.
    #[test]
    fn adoption_writes_a_baseline_where_there_was_none() {
        let tmp = repo_with_an_aged_orphan(2);
        // Give it something that gates.
        fs::write(
            tmp.path().join(".yidam/corpus/reach/broken.yml"),
            "class: reach\nlabel: X\ndescription: X.\nlinks:\n  - target: gone.yml\n    relationship: refines\n",
        )
        .unwrap();
        commit_all(tmp.path(), "establish: a node with a broken edge");

        assert!(!super::baseline::path(tmp.path()).exists());
        lint_at(
            tmp.path(),
            Options {
                init_baseline: true,
                ..Default::default()
            },
        )
        .unwrap();

        let b = super::baseline::Baseline::load(tmp.path()).unwrap();
        assert_eq!(b.violations["dangling-edge"].len(), 1);
        assert!(
            b.violations["dangling-edge"][0].since.is_some(),
            "the entry starts its clock at adoption"
        );
    }

    /// Safe to run unconditionally, which is the whole point — the re-vendor task calls it
    /// without asking whether this repository has ever blessed anything.
    #[test]
    fn adoption_leaves_an_existing_baseline_alone() {
        let tmp = repo_with_an_aged_orphan(2);
        fs::write(
            tmp.path().join(".yidam/corpus/reach/broken.yml"),
            "class: reach\nlabel: X\ndescription: X.\nlinks:\n  - target: gone.yml\n    relationship: refines\n",
        )
        .unwrap();
        commit_all(tmp.path(), "establish: a node with a broken edge");

        let hand_written = super::baseline::Baseline {
            expire_after: Some(42),
            ..Default::default()
        };
        hand_written.write(tmp.path()).unwrap();

        lint_at(
            tmp.path(),
            Options {
                init_baseline: true,
                ..Default::default()
            },
        )
        .unwrap();

        let b = super::baseline::Baseline::load(tmp.path()).unwrap();
        assert_eq!(b.expire_after, Some(42), "untouched");
        assert!(
            b.violations.is_empty(),
            "adoption did not overwrite an existing file: {b:?}"
        );
    }

    /// The gate an adopted baseline installs: quiet on the debt it recorded, loud on the
    /// next thing.
    #[test]
    fn an_adopted_baseline_gates_the_next_violation_and_not_the_inherited_one() {
        let tmp = repo_with_an_aged_orphan(2);
        fs::write(
            tmp.path().join(".yidam/corpus/reach/broken.yml"),
            "class: reach\nlabel: X\ndescription: X.\nlinks:\n  - target: gone.yml\n    relationship: refines\n",
        )
        .unwrap();
        commit_all(tmp.path(), "establish: a node with a broken edge");
        lint_at(
            tmp.path(),
            Options {
                init_baseline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            lint_at(tmp.path(), Options::default()).is_ok(),
            "inherited debt is quiet"
        );

        fs::write(
            tmp.path().join(".yidam/corpus/reach/second.yml"),
            "class: reach\nlabel: Y\ndescription: Y.\nlinks:\n  - target: also-gone.yml\n    relationship: refines\n",
        )
        .unwrap();
        commit_all(tmp.path(), "establish: a second broken edge");
        assert!(
            lint_at(tmp.path(), Options::default()).is_err(),
            "the next one is attributable to the commit that introduced it"
        );
    }

    /// Run `lint` against a directory that is not the process's cwd.
    ///
    /// `lint()` resolves the repository itself, so the tests that need the whole command —
    /// rather than `run_checks` — set the cwd. Serialized behind a mutex because the cwd
    /// is process-global and the test runner is threaded.
    fn lint_at(root: &Path, opts: Options) -> Result<()> {
        static CWD: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _held = CWD.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(root).unwrap();
        let out = lint(opts);
        std::env::set_current_dir(previous).unwrap();
        out
    }

    fn commit_all(root: &Path, message: &str) {
        for args in [vec!["add", "-A"], vec!["commit", "-q", "-m", message]] {
            let ok = std::process::Command::new("git")
                .current_dir(root)
                .args(&args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }
    }
}
