//! What every check is answered about.
//!
//! One value, built once per run, carrying the corpus and everything derived from it. A
//! check is a function of this and nothing else — no root, no options, no second walk — so
//! [`super::ROSTER`] can be a list of `fn(&Input) -> Check` rather than four hundred lines
//! of assembly (#928). `doctor`'s [`Subject`](crate::cmd::doctor) is the same shape for the
//! same reason.
//!
//! **Derived lazily, and shared.** Every field below is a [`OnceLock`] filled on first ask,
//! which is what lets the expensive readings — the git replays, the prose walk, the
//! `crates/` scan — be named once and reached from the several checks that need them
//! without anybody re-deriving one. It is the pattern [`Corpus`] already uses for nodes,
//! classes and edges, and the reason a check may be written without first asking who else
//! is going to want its inputs.
//!
//! **The reads are still where the repository is.** A check function stays pure — it takes
//! records and returns findings — and the git, disk and network readings happen here. That
//! split is what makes the arms that have never fired in a real corpus testable at all, and
//! it is unchanged by this bundle: it is exactly which side of the line each reading was on
//! before.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::authorship::{Authorship, Region};
use crate::corpus::{Class, Corpus, Edges, Node, Overlay, Source};

use super::{
    attest, checks, citations, commitments, edge_claims, independence, line_citations, lineage,
    local_citations, scope, ttl, Check, Options,
};

/// The corpus, the options, and every reading the checks are answered from.
///
/// Borrowed rather than owned for the four bases: several derived values hold references
/// into the corpus and the authorship manifest ([`checks::ProseView`] and
/// [`checks::UnauthoredLink`] both do), and a struct that owned what its own cached values
/// borrow from could not be written at all.
pub(crate) struct Input<'a> {
    root: &'a Path,
    opts: &'a Options,
    overlay: &'a Overlay,
    corpus: &'a Corpus,
    authorship: &'a Authorship,

    defined: OnceLock<HashSet<String>>,
    universal: OnceLock<crate::universal::Universal>,
    config: OnceLock<crate::config::YidamConfig>,
    policy_overrides: OnceLock<Vec<(String, String)>>,
    deps: OnceLock<std::collections::BTreeMap<String, citations::Installed>>,
    cites: OnceLock<Vec<Vec<String>>>,
    claim_fields: OnceLock<crate::claims::ClaimFields>,
    prose_fields: OnceLock<crate::prose::ProseFields>,
    catalog_ages: OnceLock<Vec<ttl::Age>>,
    tables: OnceLock<Vec<(String, String)>>,
    annotations: OnceLock<Vec<checks::Annotation>>,
    sangha: OnceLock<crate::cmd::sangha::SanghaReport>,
    registered: OnceLock<Vec<String>>,
    attestations: OnceLock<Vec<attest::Attestation>>,
    scope_audits: OnceLock<Vec<scope::ScopeAudit>>,
    independence_audits: OnceLock<Vec<independence::IndependenceAudit>>,
    standings: OnceLock<Vec<lineage::Standing>>,
    commitments: OnceLock<Vec<commitments::Commitments>>,
    prose_paths: OnceLock<Vec<PathBuf>>,
    regen_files: OnceLock<Vec<(String, String)>>,
    links: OnceLock<(Vec<checks::ProseLink>, Vec<checks::UnauthoredLink<'a>>)>,
    stale_regions: OnceLock<Vec<&'a Region>>,
    line_citations: OnceLock<Vec<line_citations::LineCitation>>,
    types: OnceLock<checks::TypeIndex>,
    tag_prose: OnceLock<Vec<checks::ProseView<'a>>>,

    // ── whole groups ────────────────────────────────────────────────────────────
    //
    // Six producers answer several checks from one walk, and the roster reaches into the
    // array they return. Cached here rather than at the call site so the walk happens once
    // however many of its checks are asked for — and so that a roster entry is still one
    // expression. Which index is which id is not left to the reader: every entry declares
    // its id and `the_roster_declares_the_id_each_entry_produces` compares the two.
    citation_checks: OnceLock<[Check; 4]>,
    local_citation_checks: OnceLock<[Check; 4]>,
    edge_claim_checks: OnceLock<[Check; 3]>,
    scope_checks: OnceLock<[Check; 2]>,
    lineage_checks: OnceLock<[Check; 3]>,
    commitment_checks: OnceLock<[Check; 4]>,
}

impl<'a> Input<'a> {
    pub(crate) fn new(
        root: &'a Path,
        opts: &'a Options,
        overlay: &'a Overlay,
        corpus: &'a Corpus,
        authorship: &'a Authorship,
    ) -> Self {
        Self {
            root,
            opts,
            overlay,
            corpus,
            authorship,
            defined: OnceLock::new(),
            universal: OnceLock::new(),
            config: OnceLock::new(),
            policy_overrides: OnceLock::new(),
            deps: OnceLock::new(),
            cites: OnceLock::new(),
            claim_fields: OnceLock::new(),
            prose_fields: OnceLock::new(),
            catalog_ages: OnceLock::new(),
            tables: OnceLock::new(),
            annotations: OnceLock::new(),
            sangha: OnceLock::new(),
            registered: OnceLock::new(),
            attestations: OnceLock::new(),
            scope_audits: OnceLock::new(),
            independence_audits: OnceLock::new(),
            standings: OnceLock::new(),
            commitments: OnceLock::new(),
            prose_paths: OnceLock::new(),
            regen_files: OnceLock::new(),
            links: OnceLock::new(),
            stale_regions: OnceLock::new(),
            line_citations: OnceLock::new(),
            types: OnceLock::new(),
            tag_prose: OnceLock::new(),
            citation_checks: OnceLock::new(),
            local_citation_checks: OnceLock::new(),
            edge_claim_checks: OnceLock::new(),
            scope_checks: OnceLock::new(),
            lineage_checks: OnceLock::new(),
            commitment_checks: OnceLock::new(),
        }
    }

    // ── the bases ───────────────────────────────────────────────────────────────

    pub(crate) fn root(&self) -> &'a Path {
        self.root
    }

    pub(crate) fn opts(&self) -> &'a Options {
        self.opts
    }

    pub(crate) fn nodes(&self) -> &'a [Node] {
        self.corpus.nodes()
    }

    pub(crate) fn classes(&self) -> &'a [Class] {
        self.corpus.classes()
    }

    pub(crate) fn sources(&self) -> &'a [Source] {
        self.corpus.sources()
    }

    pub(crate) fn edges(&self) -> &'a Edges {
        self.corpus.edges()
    }

    // ── derived ─────────────────────────────────────────────────────────────────

    /// The class names this corpus defines, as a set.
    pub(crate) fn defined(&self) -> &HashSet<String> {
        self.defined
            .get_or_init(|| self.corpus.defined_classes().map(str::to_string).collect())
    }

    /// The universal declarations, read through the overlay like every class — so the editor
    /// lints an unsaved `universal.yml` against the buffer rather than against the file on
    /// disk.
    pub(crate) fn universal(&self) -> &crate::universal::Universal {
        self.universal.get_or_init(|| {
            crate::universal::Universal::parse(
                &self
                    .overlay
                    .read(&crate::universal::Universal::path(self.root)),
            )
        })
    }

    /// What this corpus has declared about itself.
    ///
    /// Read leniently: a malformed config must not take the checks down. The gate reports the
    /// file as its own finding elsewhere; here, degrading to the defaults fails in the
    /// direction of reporting rather than of failing a build on a number nobody set.
    pub(crate) fn config(&self) -> &crate::config::YidamConfig {
        self.config
            .get_or_init(|| crate::config::load_yidam_config(self.root).unwrap_or_default())
    }

    /// Commits a dated finding may hold before it escalates — absent, and escalating nothing,
    /// for every corpus that has not argued about a number.
    pub(crate) fn escalate_after(&self) -> Option<usize> {
        self.config().lint.escalate_after
    }

    /// The vault names an artifact record is allowed to route to.
    ///
    /// Read straight from the config rather than through `vault::resolve`, deliberately:
    /// `resolve` enforces the one-vault rule, and a corpus that has declared two has a
    /// configuration problem rather than a *catalog* problem. Reporting every artifact as
    /// unroutable because a second vault exists would blame the records for something they
    /// did not do.
    pub(crate) fn declared_vaults(&self) -> Vec<String> {
        self.config().vault.keys().cloned().collect()
    }

    /// Which disclosure decisions this repository decided for itself.
    ///
    /// Read here rather than in the check, which stays pure — the same split every other
    /// check in this module keeps.
    ///
    /// A policy that does not compile is not reported as an override: it is a failure, and
    /// `yidam policy check` and `yidam doctor` are where it is reported as one. Swallowing it
    /// into an empty list here would turn a broken rule into a clean gate.
    pub(crate) fn policy_overrides(&self) -> &[(String, String)] {
        self.policy_overrides.get_or_init(|| {
            crate::policy::Policies::load(self.root)
                .map(|p| {
                    p.origins()
                        .filter_map(|(d, o)| match o {
                            crate::policy::Origin::Local(path) => {
                                Some((d.to_string(), self.rel(path)))
                            }
                            crate::policy::Origin::Inherited => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// What this repository depends on and can actually read.
    ///
    /// Off disk, in the light build: `--features tonpa` buys the network, and derived-repo CI
    /// downloads a binary rather than compiling one — so a citation check behind that feature
    /// would never run where it counts.
    pub(crate) fn deps(&self) -> &std::collections::BTreeMap<String, citations::Installed> {
        self.deps.get_or_init(|| citations::installed(self.root))
    }

    /// Which nodes cite each catalog source.
    ///
    /// [`Node`] carries the text `load_nodes` already read, so nothing here re-reads the
    /// corpus to hand the same bytes to a check a second time.
    pub(crate) fn cites(&self) -> &[Vec<String>] {
        self.cites
            .get_or_init(|| checks::citations(self.sources(), self.nodes()))
    }

    /// The `type: claim` properties each class declared, so the structural arm of the claim
    /// reader sees anything at all. Loaded once and shared: it walks the ontology.
    pub(crate) fn claim_fields(&self) -> &crate::claims::ClaimFields {
        self.claim_fields
            .get_or_init(|| crate::claims::ClaimFields::load(self.corpus.dir()))
    }

    /// Which keys on each class carry prose.
    ///
    /// Built from the classes already parsed rather than re-read from disk, and through the
    /// overlay for `universal.yml`, so the editor measures an unsaved declaration. Keyed by
    /// the `.ont.yml` stem, which is the directory an instance's class resolves to — the same
    /// keying [`crate::claims::ClaimFields`] documents.
    pub(crate) fn prose_fields(&self) -> &crate::prose::ProseFields {
        self.prose_fields.get_or_init(|| {
            crate::prose::ProseFields::from_declarations(
                self.universal().prose().to_vec(),
                self.classes()
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
            )
        })
    }

    /// How old each source record is, and whether this corpus asked to be told.
    ///
    /// `today` is resolved once here rather than inside the check, so the one wall-clock
    /// report in the tool has a single place its clock enters.
    pub(crate) fn catalog_ages(&self) -> &[ttl::Age] {
        self.catalog_ages.get_or_init(|| {
            ttl::ages(
                self.sources(),
                &ttl::committed_dates(self.root, self.corpus.catalog_dir()),
                self.config().catalog.ttl_days,
                &super::today_iso(),
            )
        })
    }

    /// The prose a reader meets a table in: catalog entries, and the READMEs that carry REGEN
    /// blocks.
    pub(crate) fn tables(&self) -> &[(String, String)] {
        self.tables.get_or_init(|| {
            let readmes = [
                self.corpus.dir().join("README.md"),
                self.corpus.catalog_dir().join("README.md"),
            ];
            self.corpus
                .catalog_paths()
                .iter()
                .chain(readmes.iter().filter(|p| p.exists()))
                .map(|p| (self.rel(p), self.overlay.read(p)))
                .collect()
        })
    }

    /// Resolution records, when this repository runs a sangha at all.
    ///
    /// Collective mode is opt-in, so an absent directory is the common case and walks to
    /// nothing.
    pub(crate) fn annotations(&self) -> &[checks::Annotation] {
        self.annotations.get_or_init(|| {
            let dir = crate::paths::yidam_sangha_dir(self.root).join("resolutions");
            crate::walk::walk_md_files(&dir)
                .iter()
                .flat_map(|p| checks::annotations_in(&self.rel(p), &self.overlay.read(p)))
                .collect()
        })
    }

    /// The seats the records name, and the seats the registry carries.
    ///
    /// Read through the sangha report rather than re-parsed here: a second reading of "who is
    /// an elector" is how a repository comes to be described two ways at once, and
    /// `electors.md` is a table whose column order is the kind of thing that drifts.
    pub(crate) fn sangha(&self) -> &crate::cmd::sangha::SanghaReport {
        self.sangha
            .get_or_init(|| crate::cmd::sangha::sangha_data(self.root))
    }

    /// The elector branches the registry carries.
    pub(crate) fn registered(&self) -> &[String] {
        self.registered.get_or_init(|| {
            self.sangha()
                .electors
                .iter()
                .map(|e| e.branch.clone())
                .collect()
        })
    }

    /// RFC-0012's verification, and its condition is the registry's own declaration: a seat's
    /// tip is verified when, and only when, its row binds a key. Empty in a corpus that binds
    /// none — every corpus today — and it does not touch git there.
    pub(crate) fn attestations(&self) -> &[attest::Attestation] {
        self.attestations
            .get_or_init(|| attest::attest(self.root, &self.sangha().electors))
    }

    /// Whether the registry binds a distinct signing key to every seat.
    pub(crate) fn keys_bind_seats(&self) -> bool {
        attest::binds_distinct_key_per_seat(&self.sangha().electors)
    }

    /// Article V's node and edge clauses, decided against the tips each record names.
    ///
    /// The git reading happens here, where there is a repository; the two checks that consume
    /// it are pure, which is what lets the arm that has never fired in a real corpus be tested
    /// at all. A repository with no resolutions — every corpus not running a sangha — spawns
    /// nothing.
    pub(crate) fn scope_audits(&self) -> &[scope::ScopeAudit] {
        self.scope_audits
            .get_or_init(|| scope::audit(self.root, &self.sangha().resolutions))
    }

    /// What `electors.md` said about each participating seat at that seat's own tip (#823).
    ///
    /// Read here and not at HEAD: a seat's row is mutable and a model upgrade is material, so
    /// HEAD would re-judge every past resolution the day somebody bumps a model. Like the
    /// scope audit, the git reading happens where there is a repository and the check stays
    /// pure. A tip this clone does not carry reads as `unrecorded`, which is already the
    /// vocabulary's word for *the registry does not say*.
    pub(crate) fn independence_audits(&self) -> &[independence::IndependenceAudit] {
        self.independence_audits
            .get_or_init(|| independence::audit(self.root, &self.sangha().resolutions))
    }

    /// Where each elector branch stands in the settled line, and what it says about where it
    /// stands. Read here for the same reason the scope audit is: the checks stay pure, and the
    /// refs are the one thing they cannot be handed off disk.
    pub(crate) fn standings(&self) -> &[lineage::Standing] {
        self.standings
            .get_or_init(|| lineage::standings(self.root, &self.sangha().resolutions))
    }

    /// What each seat's own branch says it is standing on (#294).
    ///
    /// Read from the branch and not from the baseline, because a commitments file is never
    /// transported — it is an index of one seat's own grounds, and a resolution that could
    /// reach for it would be synthesizing from something no elector filed as a position. The
    /// git reading happens here for the same reason the three above do: the checks stay pure
    /// over what was read.
    pub(crate) fn commitments(&self) -> &[commitments::Commitments] {
        self.commitments
            .get_or_init(|| commitments::read(self.root))
    }

    /// Every authored prose surface the link checks walk: `.yidam/` and `docs/`.
    ///
    /// What counts as authored is declared rather than hard-coded; see
    /// [`crate::authorship`]. `.yidam/.vendor/` used to be named here as the single
    /// exception, on a rationale that generalizes — a defect in the prelude is fixed upstream
    /// and adopted by re-vendoring, so reporting one to a derived repo hands it a finding it
    /// cannot act on. It is now the built-in instance of the general mechanism, and a
    /// repository that is not a vendoring repository can say the same about a generated
    /// directory or a frozen import of its own.
    ///
    /// `docs/` is included — documentation about the repository is authored, and its links rot
    /// the same way. Not `crates/` or `web/`, whose READMEs carry illustrative targets rather
    /// than references to files that are supposed to exist.
    pub(crate) fn prose_paths(&self) -> &[PathBuf] {
        self.prose_paths.get_or_init(|| {
            let mut paths = crate::walk::walk_linkable_files(&self.root.join(".yidam"));
            paths.extend(crate::walk::walk_linkable_files(&self.root.join("docs")));
            paths
        })
    }

    /// Every authored file the generators can write into (#524).
    ///
    /// The walk is the prose-link walk plus the repository README, which `yidam status` and
    /// `yidam vault-status` write and which nothing else reads. A file with no markers
    /// contributes nothing, so a walk wider than the generators' own list costs a read and
    /// cannot miss a target — which a list copied from the ten `update_file_regen` call sites
    /// would.
    pub(crate) fn regen_files(&self) -> &[(String, String)] {
        self.regen_files.get_or_init(|| {
            let readme = [self.root.join("README.md")];
            self.prose_paths()
                .iter()
                .chain(readme.iter().filter(|p| p.exists()))
                .map(|p| (self.rel(p), p))
                // The same authorship rule the prose-link check applies: a finding in
                // vendored prelude content is one the derived repository cannot act on.
                .filter(|(rel, _)| {
                    self.authorship
                        .covering(rel)
                        .is_none_or(|r| r.kind.reportable())
                })
                .map(|(rel, p)| (rel, self.overlay.read(p)))
                .collect()
        })
    }

    /// The markdown links in this repository's own prose.
    pub(crate) fn prose_links(&self) -> &[checks::ProseLink] {
        &self.links().0
    }

    /// The markdown links in prose some other repository answers for.
    pub(crate) fn unauthored(&self) -> &[checks::UnauthoredLink<'a>] {
        &self.links().1
    }

    /// Both halves of one walk: a link is this repository's or a declared region's, never
    /// both, and deciding that twice is how the two checks would come to disagree about which
    /// file a finding belongs to.
    fn links(&self) -> &(Vec<checks::ProseLink>, Vec<checks::UnauthoredLink<'a>>) {
        self.links.get_or_init(|| {
            let mut mine: Vec<checks::ProseLink> = Vec::new();
            let mut theirs: Vec<checks::UnauthoredLink<'a>> = Vec::new();
            for p in self.prose_paths() {
                let rel = self.rel(p);
                let region = self.authorship.covering(&rel);
                // `excluded` is the one kind that means *do not look*; the file is not even
                // read.
                if region.is_some_and(|r| !r.kind.reportable()) {
                    continue;
                }
                let dir = p.parent().unwrap_or(self.root);
                let links = checks::prose_links(&rel, dir, &self.overlay.read(p));
                match region {
                    Some(region) => theirs.extend(
                        links
                            .into_iter()
                            .map(|link| checks::UnauthoredLink { region, link }),
                    ),
                    None => mine.extend(links),
                }
            }
            (mine, theirs)
        })
    }

    /// Declared regions that no longer describe anything on disk.
    pub(crate) fn stale_regions(&self) -> &[&'a Region] {
        self.stale_regions
            .get_or_init(|| crate::authorship::stale(self.root, self.authorship))
    }

    /// The links that also name a line, decided against the cited files.
    ///
    /// Through the overlay, so the buffer someone is editing a passage out of is the one the
    /// citation is held to. Authored links only: a line citation in vendored or generated
    /// prose is somebody else's to fix, the same judgement `unauthored-prose-link` records.
    pub(crate) fn line_citations(&self) -> &[line_citations::LineCitation] {
        self.line_citations.get_or_init(|| {
            line_citations::collect(self.root, self.prose_links(), &|p| self.overlay.read(p))
        })
    }

    /// The types `crates/` defines, for the one check whose subject is the ontology and whose
    /// evidence is the code. Read through the overlay like everything else, so the editor
    /// resolves a class against the buffer somebody is deleting a struct out of.
    ///
    /// **Only when a class asked.** The walk is skipped entirely where no class declares
    /// `implemented_by:`, which is every corpus measured and every corpus that predates the
    /// field — so a repository that never opted in pays nothing for a check that would report
    /// nothing.
    ///
    /// Authorship regions are deliberately *not* consulted. Elsewhere a region says whose
    /// finding a file's contents are; here the finding's subject is a class in this
    /// repository's own ontology, and a type is evidence that it exists wherever it lives. A
    /// generated implementation is still an implementation.
    pub(crate) fn types(&self) -> &checks::TypeIndex {
        self.types.get_or_init(|| {
            if !self.classes().iter().any(|c| c.implemented_by.is_some()) {
                return checks::TypeIndex::build([]);
            }
            let texts: Vec<(String, String)> =
                crate::walk::walk_rust_files(&self.root.join("crates"))
                    .iter()
                    .map(|p| (self.rel(p), self.overlay.read(p)))
                    .collect();
            checks::TypeIndex::build(texts.iter().map(|(r, t)| (r.as_str(), t.as_str())))
        })
    }

    /// Nodes and classes both: a malformed evidence tag is a defect of prose, and a class file
    /// carries prose.
    pub(crate) fn tag_prose(&self) -> &[checks::ProseView<'a>] {
        self.tag_prose
            .get_or_init(|| checks::prose_views(self.nodes(), self.classes()))
    }

    // ── groups ──────────────────────────────────────────────────────────────────

    /// One walk of the citations, four readings of it — the same predicate `check_citation`
    /// answers from over MCP (#357).
    pub(crate) fn citation_checks(&self) -> &[Check; 4] {
        self.citation_checks
            .get_or_init(|| citations::checks(self.nodes(), self.deps()))
    }

    /// The other direction of the same join: a node resting on a verbatim span of another node
    /// in this corpus (RFC-0034). No dependency, no network, no pin — which is why it is the
    /// arm every corpus can actually use, and the external four have never had a subject in
    /// any measured corpus.
    pub(crate) fn local_citation_checks(&self) -> &[Check; 4] {
        self.local_citation_checks
            .get_or_init(|| local_citations::checks(self.nodes(), self.claim_fields()))
    }

    /// The graph's own half of the same discipline (#587): an edge is a claim written as
    /// structure, and these are the checks that ask it what it rests on. The third compares
    /// that standing to the ones its own endpoints declare (#858), which is why the claim
    /// fields go in — a node's standing is a property its class declared `type: claim`.
    pub(crate) fn edge_claim_checks(&self) -> &[Check; 3] {
        self.edge_claim_checks.get_or_init(|| {
            edge_claims::checks(
                self.nodes(),
                self.edges(),
                self.universal(),
                self.claim_fields(),
            )
        })
    }

    pub(crate) fn scope_checks(&self) -> &[Check; 2] {
        self.scope_checks
            .get_or_init(|| scope::checks(self.scope_audits()))
    }

    pub(crate) fn lineage_checks(&self) -> &[Check; 3] {
        self.lineage_checks
            .get_or_init(|| lineage::checks(self.standings()))
    }

    pub(crate) fn commitment_checks(&self) -> &[Check; 4] {
        self.commitment_checks
            .get_or_init(|| commitments::checks(self.commitments()))
    }

    /// `path`, relative to the repository root — the form every finding names a file in.
    ///
    /// One helper rather than the six copies of `strip_prefix(root).unwrap_or(p)` this
    /// assembly used to carry: a finding's path is half the baseline's identity, and two
    /// spellings of it are two ways to write an entry nothing can match.
    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }
}
