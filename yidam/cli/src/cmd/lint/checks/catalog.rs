use super::*;

// ── catalog ───────────────────────────────────────────────────────────────────

/// Every repository path a node points at: `links:` targets and markdown links alike, each
/// resolved from the node's own directory and normalized.
///
/// **The one answer to "what does this node link to?"** There were three matchers asking it,
/// and two of them asked it by searching the node's bytes for a slug: `lint`'s citation
/// counter and `catalog-audit`'s. Both reported a node that merely *named* a source as one
/// that cites it, which the conventions do not say and a reader chasing the finding does not
/// find. A third copy is where the next divergence goes.
///
/// **The `unwrap_or_default` below stays, and #676 is why it can.** This answers a question
/// about bytes — *what does this text point at?* — and half of the answer, the markdown links,
/// does not need the parse at all. A node whose YAML is broken still cites what its prose
/// cites, and the `links:` half being empty is the most this function can honestly say. What
/// used to be wrong was that nobody was told; [`malformed_yaml`] reports the file now, so the
/// degradation here is a stated consequence rather than a silent one. Reporting it a second
/// time would put a finding about a corpus node on the catalog entry that happens to be asking.
///
/// `catalog` is the catalog directory, and it is here because the third form a citation takes
/// is not a path. An edge's `source:` names a catalog entry by file stem (#1158), and a stem
/// resolves only against the directory the entries live in — see [`source_targets`].
///
/// `quotations` is here for the fourth form. A quotation's `of:` names an entry the same way
/// (#1174), but only in a property the ontology declares `type: quotation`, and the bytes
/// alone cannot say which properties those are.
pub fn linked_paths(
    node_path: &Path,
    rel: &str,
    text: &str,
    catalog: &Path,
    quotations: &Declared,
) -> HashSet<PathBuf> {
    let inst: crate::parse::CorpusInstance = serde_yaml::from_str(text).unwrap_or_default();
    linked_paths_from(&inst, node_path, rel, text, catalog, quotations)
}

/// Where an edge's `source:` may point, as node directory `dir` wrote it.
///
/// Two readings, tried together: a catalog entry by file stem — `pearl-2009` for
/// `.yidam/catalog/pearl-2009.md` — and a path written the way a `links:` target is. Both are
/// returned rather than one chosen, because a bare stem tried as a path lands in the node's
/// own class directory, which is never a catalog entry, and a path tried as a stem lands under
/// `catalog/` with a `.md` appended to a `.md`. Neither reading can admit the other's mistake,
/// so a caller testing membership against the catalog gets exactly the entries the value
/// names. The third spelling an edge source admits, a decision record's `id:`, is not a path
/// and is not resolved here: `edge-source-unresolved` reads it, and a citation is a thing that
/// resolves to the catalog.
///
/// **This is the one place every direction resolves a stem.** `edge-source-unresolved` asks
/// whether a `source:` names something held; `catalog-uncited` asks whether an entry is named
/// by anything; `rename` asks which edges name the entry it is moving (#1159). Until #1158 the
/// second read only links, so an entry cited from edges alone was reported uncited while every
/// edge citing it resolved. Sharing the readings is what makes that impossible to reintroduce.
/// A quotation's `of:` is resolved here too, by `quotation-unresolved` and by the citation
/// count (RFC-0046, #1174).
pub(crate) fn source_targets(dir: &Path, catalog: &Path, written: &str) -> [PathBuf; 2] {
    [
        normalize(&catalog.join(format!("{written}.md"))),
        normalize(&dir.join(written)),
    ]
}

/// [`linked_paths`] for a caller that has already parsed the node.
///
/// **The matching lives here and only here**, so the invariant above survives the split:
/// [`linked_paths`] is this function plus the parse, not a second implementation of it. The
/// `&str` entry point stays because two callers reach this question without a [`Node`] in
/// hand — `serve`'s node view and `catalog audit`, both holding only a path and bytes.
///
/// The lint checks do hold one. [`Node::inst`] is the same parse this would redo, produced by
/// [`Node::parse`] from the same [`Node::text`] — including the broken-YAML case, where
/// [`crate::corpus::parse_or_default`] leaves a default instance exactly as the `unwrap_or_default` above
/// does. Two checks ask this per node, so re-parsing here meant a lint run parsed every
/// node's YAML three times.
///
/// Five things go in: every `links:` target, every markdown link in the prose, every
/// non-blank `source:` an edge writes, every `of:` a declared quotation writes, and every
/// `source` an evidenced list entry writes (RFC-0050). The last three are read through
/// [`source_targets`]. An edge source is what an edge rests on, and it is a
/// citation for the same reason a prose link is — the node has named a catalog entry as the
/// thing a claim draws from. A quotation names the entry its words were taken from, which is
/// the same act (#1174). A blank `source:` names nothing and is skipped, as
/// `edge-source-unresolved` skips it. A quotation without an `of:` is not one, and
/// [`Declared::named_entries`] drops it.
///
/// The directory and the class both come from `node_path`, here, so the two entry points
/// cannot derive them differently.
fn linked_paths_from(
    inst: &crate::parse::CorpusInstance,
    node_path: &Path,
    rel: &str,
    text: &str,
    catalog: &Path,
    quotations: &Declared,
) -> HashSet<PathBuf> {
    let dir = node_path.parent().unwrap_or(node_path);
    let links = inst.links.as_deref().unwrap_or(&[]);
    let mut out: HashSet<PathBuf> = links
        .iter()
        .filter_map(|l| l.target.as_ref())
        .map(|t| normalize(&dir.join(t)))
        .collect();
    out.extend(
        links
            .iter()
            .filter_map(|l| l.source.as_deref())
            .filter(|s| !s.trim().is_empty())
            .flat_map(|s| source_targets(dir, catalog, s)),
    );
    out.extend(
        quotations
            .named_entries(&crate::paths::class_of_path(node_path), inst)
            .iter()
            .flat_map(|of| source_targets(dir, catalog, of)),
    );
    // An evidenced entry's `source`, which RFC-0050 resolves as an edge's.
    out.extend(
        quotations
            .entry_sources(&crate::paths::class_of_path(node_path), inst)
            .iter()
            .flat_map(|s| source_targets(dir, catalog, s)),
    );
    out.extend(
        prose_links(rel, dir, text)
            .into_iter()
            .map(|l| normalize(&l.resolved)),
    );
    out
}

/// [`linked_paths_from`] for a [`Node`], which carries everything it needs but the catalog
/// and the declarations.
fn linked_paths_of(node: &Node, catalog: &Path, quotations: &Declared) -> HashSet<PathBuf> {
    linked_paths_from(
        &node.inst, &node.path, &node.rel, &node.text, catalog, quotations,
    )
}

/// Corpus nodes citing each source, by repo-relative path.
///
/// **A citation is a link that resolves to the catalog file.** That is what the conventions
/// describe — "corpus nodes link to catalog nodes as edges […] writes
/// `[Pearl 2009](../../catalog/pearl-2009.md)`" — and it used to be a substring search for
/// the slug anywhere in the node's bytes, which admits every sentence that happens to
/// contain the word.
///
/// The reported case was a catalog entry carrying `obtained: false` whose slug named a
/// connector crate — the directory conventions recommend naming connectors after what they
/// fetch (`nwis`, `echo`, `census`), so a catalog entry for the source those connectors
/// fetch from naturally collides. A node mentioning the crate in backticks, linking nothing,
/// failed the build under `catalog-unobtained-but-cited`, which is Error severity and gates.
/// The finding named a file whose text contains no citation, so the diagnosis ran through
/// "which node cites this?" before arriving at "none of them do".
///
/// Four forms count: a `links:` entry, a markdown link in the prose, an edge's `source:`
/// naming the entry by stem or by path (#1158), and a declared quotation's `of:` naming it the
/// same way (#1174). The prose scan is [`prose_links`], the same function `broken-prose-link`
/// uses, so a link shown as an example in code is not read as a citation here either. The edge
/// source and the quotation are resolved by [`source_targets`], the same function
/// `edge-source-unresolved` and `quotation-unresolved` resolve them with, so an entry every
/// edge or quotation citing it resolves to cannot be reported as cited by nothing.
pub fn citations(
    sources: &[Source],
    nodes: &[Node],
    catalog: &Path,
    quotations: &Declared,
) -> Vec<Vec<String>> {
    // Resolved once per node rather than once per (node, source) pair, and from [`Node::text`]
    // rather than from a parallel `Vec<String>` the caller built by re-reading every instance.
    // The gate's hot path read the corpus twice and held two copies of it to run one check.
    let linked: Vec<HashSet<PathBuf>> = nodes
        .iter()
        .map(|n| linked_paths_of(n, catalog, quotations))
        .collect();

    sources
        .iter()
        .map(|s| {
            let target = normalize(&s.path);
            linked
                .iter()
                .zip(nodes)
                .filter(|(links, _)| links.contains(&target))
                .map(|(_, n)| n.rel.clone())
                .collect()
        })
        .collect()
}

pub const VERIFIED_UNSOURCED: CheckSpec = CheckSpec {
    id: "verified-unsourced",
    title: "A claim asserted at `[verified]` that rests on nothing",
    severity: Severity::Warn,
    rationale: "`[verified]` means supported by a committed primary source. A node asserting one \
         while linking to no catalog entry has claimed a standing it cannot demonstrate. \
         This is the one direction the claim vocabulary exists to prevent: over-counting \
         evidence is the flattering error, and a mature corpus measured eight miscounts of \
         which all eight promoted. The catalog checks beside this one verify the catalog's \
         own bookkeeping; this is the only one that asks whether a claim rests on anything. \
         The fix is a citation or a demotion, and which of the two is the author's call — \
         nothing here proposes a promotion, ever.",
};

/// A `[verified]` claim in a node that draws on no registered source.
///
/// The claim vocabulary is the epistemic core of the system and, until this check, its
/// correctness was unenforced in the one direction that matters. `claims.rs` records which
/// direction that is — it was measured against a mature corpus and every one of the eight
/// miscounts *promoted*:
///
/// > That is the flattering direction, and it is the one direction this vocabulary exists to
/// > prevent.
///
/// `catalog/` exists to record provenance, and the three checks that read it
/// (`catalog-uncited`, `catalog-unobtained-but-cited`, `catalog-used-by-declared`) check the
/// catalog's own bookkeeping — whether entries and citations agree with each other. None of
/// them asks the question the vocabulary is *for*: whether a claim asserted at the strongest
/// standing rests on anything at all.
///
/// `[verified]` means **supported by a committed primary source**. A node making one while
/// linking to no catalog entry has asserted a standing it cannot demonstrate from inside the
/// repository.
///
/// # What counts as resting on something
///
/// A link that resolves to a file under `.yidam/catalog/`, or an edge `source:` or a declared
/// quotation's `of:` that names one. That is the same resolution [`citations`] performs, so a
/// node this check calls unsourced is exactly a node absent from every entry's citation list —
/// the two cannot come to disagree about what a citation is.
///
/// **A `cites:` into a dependency does not count**, and that is RFC-0019's rule rather than a
/// simplification. A foreign tag is the producer's tag: it records what *that* corpus's
/// electors accepted, travels with none of the apparatus that made it accountable, and is
/// recorded as an observation rather than folded into a local standing. `agent-conduct.md`
/// prescribes what to do instead — put what you took into a local node, at this corpus's
/// standard — and a local `[verified]` still owes a local source.
///
/// # Warn, not Error
///
/// The corpus is not malformed and traversal is unaffected; what is wrong is a claim about
/// evidence, and the fix is either a citation or a demotion — both of which are an author's
/// judgement rather than a mechanical repair. Gating would also make adopting this check a
/// build break in every corpus that predates it, which is the failure
/// `docs/post-genesis-measurement.md` recorded for a ratchet nobody could satisfy.
///
/// Measured across three instrumented derived repositories before choosing the level; the
/// figures are in the pull request that added it.
pub fn verified_unsourced(
    nodes: &[Node],
    sources: &[Source],
    fields: &crate::claims::ClaimFields,
    catalog: &Path,
    quotations: &Declared,
) -> Check {
    let registered: HashSet<PathBuf> = sources.iter().map(|s| normalize(&s.path)).collect();
    let violations = nodes
        .iter()
        .filter_map(|n| {
            // Through the counter rather than by filtering `claims_in_node` on a spelling.
            // `ServedClaim::standing` is the bare word and `claims::VERIFIED` is the
            // bracketed marker, so comparing the two compares nothing — which is how the
            // first draft of this check reported zero findings against a corpus holding 456
            // verified claims, and looked exactly like a clean bill of health. The counter
            // is already held to agree with the served list tag for tag.
            let verified =
                crate::claims::count_in_node(&n.text, fields.for_class(&class_of(n))).verified;
            if verified == 0 {
                return None;
            }
            let links = linked_paths_of(n, catalog, quotations);
            if links.iter().any(|p| registered.contains(p)) {
                return None;
            }
            // Named separately because the fix differs: a node already citing a dependency
            // has *some* provenance recorded and needs it localized, and one citing nothing
            // has none.
            let external = n.inst.cites.as_deref().is_some_and(|c| !c.is_empty());
            let detail = match (verified, external) {
                (1, false) => "1 `[verified]` claim, and this node draws on no source".to_string(),
                (n, false) => format!("{n} `[verified]` claims, and this node draws on no source"),
                (1, true) => "1 `[verified]` claim resting only on a `cites:` into a dependency — \
                              a foreign tag is the producer's, and does not transfer"
                    .to_string(),
                (n, true) => format!(
                    "{n} `[verified]` claims resting only on a `cites:` into a dependency — a \
                     foreign tag is the producer's, and does not transfer"
                ),
            };
            Some(Violation::new(&n.rel, detail))
        })
        .collect();
    Check::new(&VERIFIED_UNSOURCED, violations)
}

/// How many corpus nodes rest on a source, and which of them.
///
/// #270 asked for the expiry question to reach *"claims citing it"*, and the honest way to do
/// that is not to write the question into each of them. Measured across the three
/// instrumented repositories, one entry is cited by a median of 3, 5 and 3 nodes — and by a
/// maximum of **84**, which is a web service, exactly the kind of record a `ttl_days` is
/// declared on. `propose` writes one commit per proposal, so propagation would mean 84
/// commits for one aged file and 84 more to retire them. The finding names them instead.
///
/// It is also what E3 already does for the analogous case: `citations::Movement` carries the
/// **local** node whose claim is affected, *"because this report is about my graph and not
/// theirs"*.
///
/// Three names, then a count. The median entry in all three corpora is listed in full, and
/// the tail says how much more there is rather than printing it.
fn resting_clause(nodes: &[String]) -> String {
    if nodes.is_empty() {
        // Worth saying, and it changes what the reader does: an aged record nothing rests on
        // may want deleting rather than refreshing.
        return " Nothing cites it.".to_string();
    }
    let mut named: Vec<&str> = nodes.iter().map(|n| corpus_address(n)).collect();
    named.sort_unstable();
    let listed = named
        .iter()
        .take(RESTING_NAMED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    match named.len().saturating_sub(RESTING_NAMED) {
        0 => format!(" {} node(s) rest on it: {listed}.", named.len()),
        more => format!(
            " {} node(s) rest on it: {listed}, and {more} more.",
            named.len()
        ),
    }
}

/// How many citing nodes a finding names before it starts counting.
const RESTING_NAMED: usize = 3;

/// `.yidam/corpus/gage/canyon-outlet.yml` → `gage/canyon-outlet.yml`.
///
/// The address `rename` and `neighbors` take, rather than [`basename`]: two classes may hold
/// a node of the same name, and a list of bare stems would not say which one aged.
fn corpus_address(rel: &str) -> &str {
    rel.strip_prefix(".yidam/corpus/").unwrap_or(rel)
}

pub const CATALOG_EXPIRED: CheckSpec = CheckSpec {
    id: "catalog-expired",
    title: "A source record older than the corpus said it may be",
    severity: Severity::Warn,
    rationale: "A catalog entry records where something came from; until it could also record how \
         long that record may stand, a source fetched a year ago read exactly like one \
         fetched today. Declare `ttl_days:` on the entry — a gauge record and a statute do \
         not age at the same rate — or a corpus-wide default under `[catalog]` in \
         `.yidam/config.toml`. Absent both, nothing expires. This reports rather than gates \
         because an aged record is a thing to look at, and refreshing it is a knowledge event \
         a person owns. It does not claim the source changed: nothing here reads upstream, \
         and `doctor` does no network. It claims nobody has looked. The finding names the \
         nodes resting on the record, because the reader who has to act is the one who wrote \
         them — and naming them here is what keeps the question in one file instead of \
         copied into every node that cites it.",
};

/// A catalog record that has stood longer than the corpus said it may.
///
/// `docs/domain-computer.md` specified "refreshed on TTL or on demand" and there was no TTL:
/// an entry recorded where something came from and never that the record had aged, so a
/// corpus resting on a source fetched a year ago read exactly like one fetched today.
///
/// **An aged source is a thing to look at, not an error.** Refreshing it is a knowledge
/// event a person should own, so this reports and does not gate — and it says only what it
/// knows. An expiry does not claim the upstream changed; nothing here can see upstream, and
/// `doctor` must keep doing no network. It claims that nobody has looked.
///
/// Two silences are kept apart, because the fix differs. An entry with no applicable TTL is
/// not reported at all: absent a declaration the corpus never asked to be told, which is
/// every corpus until someone turns it on. An entry that *does* carry a TTL and has no date
/// to measure against is reported as **undatable** rather than as expired — a gap in the
/// bookkeeping, not a stale source, and calling it stale would assert something nobody knows.
pub fn catalog_expired(
    ages: &[super::super::ttl::Age],
    sources: &[Source],
    cites: &[Vec<String>],
) -> Check {
    let resting: HashMap<&str, &Vec<String>> = sources
        .iter()
        .map(|s| s.rel.as_str())
        .zip(cites.iter())
        .collect();
    let empty: Vec<String> = Vec::new();
    let violations = ages
        .iter()
        .filter_map(|a| {
            let on = resting_clause(resting.get(a.entry.as_str()).copied().unwrap_or(&empty));
            if a.undatable() {
                return Some(Violation::new(
                    &a.entry,
                    format!(
                        "declares a {}-day TTL and nothing records when it was fetched — add \
                         `retrieved:`, or commit the entry so its date can be read.{on}",
                        a.ttl_days.unwrap_or(0)
                    ),
                ));
            }
            let over = a.overdue_days()?;
            Some(Violation::new(
                &a.entry,
                format!(
                    "retrieved {} ({}), {} day(s) ago against a {}-day TTL — {} day(s) past.{on}",
                    a.retrieved.as_deref().unwrap_or("?"),
                    a.dated.map(|d| d.as_str()).unwrap_or("?"),
                    a.age_days.unwrap_or(0),
                    a.ttl_days.unwrap_or(0),
                    over
                ),
            ))
        })
        .collect();
    Check::new(&CATALOG_EXPIRED, violations)
}

pub const CATALOG_UNCITED: CheckSpec = CheckSpec {
    id: "catalog-uncited",
    title: "Registered source nothing draws from",
    severity: Severity::Info,
    rationale: "A catalog entry describes a source, not knowledge, so an uncited one is not an \
         error. It is either a source registered ahead of the extraction that will use it, \
         or a claim resting on evidence that never became a node. The two look identical \
         from here and are worth telling apart — which is what `obtained: false` declares. \
         Entries that have said they are the former are not reported.",
};

pub fn catalog_uncited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| s.obtained && c.is_empty())
        .map(|(s, _)| Violation::new(&s.rel, "no corpus node draws on this source"))
        .collect();
    Check::new(&CATALOG_UNCITED, violations)
}

pub const CATALOG_UNOBTAINED_BUT_CITED: CheckSpec = CheckSpec {
    id: "catalog-unobtained-but-cited",
    title: "Source nobody has fetched, cited anyway",
    severity: Severity::Error,
    rationale: "This is the price of the `obtained: false` exemption, which is otherwise the \
         cheapest way to silence `catalog-uncited`. A node drawing on a source the catalog \
         says was never retrieved is one of two things and both are defects: the flag is \
         stale and was not cleared when the source arrived, or the citation rests on \
         something nobody has read.",
};

pub fn catalog_unobtained_but_cited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| !s.obtained && !c.is_empty())
        .map(|(s, c)| {
            Violation::new(
                &s.rel,
                format!(
                    "`obtained: false` but {} node(s) cite it, e.g. {}",
                    c.len(),
                    c[0]
                ),
            )
        })
        .collect();
    Check::new(&CATALOG_UNOBTAINED_BUT_CITED, violations)
}

/// How a declared `used-by` list disagrees with the citations, by basename.
///
/// Basenames rather than paths because the list is hand-written and a person writing one
/// writes `tailwater.yml`, not `.yidam/corpus/concept/tailwater.yml`. Comparing full paths
/// would report drift on every entry whose author spelled the node the way the convention
/// asks them to.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct UsedByDrift {
    /// Names the list claims, that no node citing this entry carries.
    pub claimed_not_citing: Vec<String>,
    /// Nodes that cite this entry, that the list omits.
    pub citing_not_claimed: Vec<String>,
}

/// The disagreement between a declared `used-by` list and the citations, or `None` when the
/// entry declares no list — **absent is not drift**, and that distinction is the whole
/// reason this takes an `Option` rather than a slice.
///
/// **An empty list is a declaration, not an absence.** `used-by: []` is a value an author
/// typed, or one the retired `catalog-reconcile` wrote on purpose so that an entry which
/// claimed something and now claims nothing still said so; either way it asserts that nothing
/// cites the entry, and an assertion is checkable. It was collapsed
/// into absence for as long as this took a slice, which made `[]` a silent opt-out of the
/// check — and the default state of a hand-authored entry, so the check was weakest exactly
/// where new material enters.
///
/// `yidam migrate used-by` asks it what a deleted list claimed that nothing cites
/// (RFC-0053 §2.2). It used to be shared by the retired drift check, `catalog-audit`'s `drift`
/// field and `catalog-reconcile`; one function is what kept them from answering differently.
pub fn used_by_drift(used_by: Option<&[String]>, citing: &[String]) -> Option<UsedByDrift> {
    let used_by = used_by?;
    let claimed: HashSet<&str> = used_by.iter().map(|u| basename(u)).collect();
    let found: HashSet<&str> = citing.iter().map(|a| basename(a)).collect();
    let mut claimed_not_citing: Vec<String> = claimed
        .difference(&found)
        .map(|s| (*s).to_string())
        .collect();
    let mut citing_not_claimed: Vec<String> = found
        .difference(&claimed)
        .map(|s| (*s).to_string())
        .collect();
    claimed_not_citing.sort_unstable();
    citing_not_claimed.sort_unstable();
    Some(UsedByDrift {
        claimed_not_citing,
        citing_not_claimed,
    })
}

pub const CATALOG_USED_BY_DECLARED: CheckSpec = CheckSpec {
    id: "catalog-used-by-declared",
    title: "Catalog entry storing what cites it",
    severity: Severity::Warn,
    rationale: "The citations are the record of what draws on an entry, and `yidam catalog-audit` \
         reports them as `cited_by`. A `used-by:` list stored beside them can only drift from \
         them, and keeping it current cost a commit per entry per new citation. An entry does \
         not list what cites it.",
};

/// A catalog entry that still stores what cites it (RFC-0053 §2.3).
///
/// The citations are the record, and `yidam catalog-audit` reports them as `cited_by`. A stored
/// list beside them could only drift, and every repair of it was a commit. Any `used-by:` key
/// is reported, whatever its value — `used-by: []` too, since that was a claim as well.
///
/// Warn and not Error, so upgrading does not fail a repository's gate before it has run the
/// migration. A lint check and not a parse failure, so the entry's other fields are still read.
pub fn catalog_used_by_declared(sources: &[Source]) -> Check {
    let violations = sources
        .iter()
        .filter(|s| s.used_by.is_some())
        .map(|s| {
            Violation::new(
                &s.rel,
                "declares `used-by:`; `yidam migrate used-by` deletes it and lists any file it \
                 named that does not cite the entry"
                    .to_string(),
            )
        })
        .collect();
    Check::new(&CATALOG_USED_BY_DECLARED, violations)
}

pub(crate) fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub const CATALOG_ARTIFACT_MALFORMED: CheckSpec = CheckSpec {
    id: "catalog-artifact-malformed",
    title: "Obtained-artifact record is missing or mistyped",
    severity: Severity::Error,
    rationale:
        "An artifact record is what makes `obtained: true` demonstrable rather than asserted \
         — the digest names the bytes the entry claims to have. A record without a well-formed \
         digest names nothing, so it is a claim of retrieval with no more standing than the \
         flag it was added to support. Error, because the failure is in the record itself and \
         is settled entirely by files in this repository.",
};

/// An artifact record that cannot be read as one.
///
/// # Why this and not a parse failure
///
/// [`crate::parse::CatalogArtifact`] takes every field as `Option` so that one bad line does
/// not make the whole entry unreadable — an entry that fails to load takes its citations, its
/// TTL and its `used-by` down with it, which is a large penalty for a mistyped digest. The
/// cost of that choice is that malformedness has to be *reported*, and this is the report.
///
/// # What it does not check
///
/// **Whether the bytes exist, and whether they hash correctly.** Both are facts about the
/// machine running the check rather than about `HEAD`, and no check in this file has ever read
/// machine-local state — `lint` reads the working tree and nothing else. A gate whose verdict
/// depends on which machine ran it is one a corpus cannot reason about, so the cache lives
/// behind `yidam vault verify` where a per-machine answer is the point.
///
/// So this reports only what the repository can settle from its own files, and it fires only
/// on entries that declare `artifacts:` at all. A corpus that has not adopted the field gets
/// no findings from it.
pub fn catalog_artifact_malformed(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, a) in s.artifacts.iter().enumerate() {
            let mut problems = Vec::new();
            problems.extend(digest_problem("sha256", a.sha256.as_deref()));
            // A text reading (#1172) is a second digest, and the extractor it names is what
            // lets a person reproduce it — a reading without one is text of unknown origin.
            if let Some(t) = &a.text {
                problems.extend(digest_problem("text.sha256", t.sha256.as_deref()));
                if t.extractor.as_deref().is_none_or(|e| e.trim().is_empty()) {
                    problems.push("`text` names no `extractor`".to_string());
                }
            }
            // `readings:` is `text:` generalised (#1318), held to the same two things and to a
            // third: a reading of unknown media type is bytes nothing says how to read.
            for (k, r) in a.readings.iter().flatten().enumerate() {
                problems.extend(digest_problem(
                    &format!("readings[{k}].sha256"),
                    r.sha256.as_deref(),
                ));
                if r.media_type.as_deref().is_none_or(|m| m.trim().is_empty()) {
                    problems.push(format!("`readings[{k}]` names no `media_type`"));
                }
                if r.by.as_deref().is_none_or(|b| b.trim().is_empty()) {
                    problems.push(format!(
                        "`readings[{k}]` names no `by`, so nothing says what took it"
                    ));
                }
            }
            // A `from:` index naming no location is the one cross-field error worth having:
            // it means the record cites a provenance the entry does not carry.
            if let Some(crate::parse::ArtifactOrigin::Location(n)) = &a.from {
                if *n >= s.locations.len() {
                    problems.push(format!(
                        "`from: {n}` names location {n} and this entry declares {}",
                        s.locations.len()
                    ));
                }
            }
            if !problems.is_empty() {
                violations.push(Violation::new(
                    &s.rel,
                    format!("artifacts[{i}]: {}", problems.join("; ")),
                ));
            }
        }
    }
    Check::new(&CATALOG_ARTIFACT_MALFORMED, violations)
}

/// What is wrong with a recorded digest, if anything. `field` is how the finding names it.
fn digest_problem(field: &str, digest: Option<&str>) -> Option<String> {
    match digest.map(str::trim) {
        None | Some("") => Some(format!("no `{field}`")),
        Some(h) if h.len() != 64 => Some(format!(
            "`{field}` is {} characters, not 64: {h:?}",
            h.len()
        )),
        // Uppercase named separately: hex is case-insensitive and a store is not, so the two
        // spellings would be two keys for one artifact.
        Some(h) if !h.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) => {
            Some(if h.chars().any(|c| c.is_ascii_uppercase()) {
                format!(
                    "`{field}` is uppercase; vault keys are lowercase hex — use {}",
                    h.to_ascii_lowercase()
                )
            } else {
                format!("`{field}` is not hex: {h:?}")
            })
        }
        Some(_) => None,
    }
}

pub const CATALOG_ARTIFACT_UNROUTABLE: CheckSpec = CheckSpec {
    id: "catalog-artifact-unroutable",
    title: "Artifact routed to a vault nothing declares",
    severity: Severity::Error,
    rationale: "A record naming a vault that `.yidam/config.toml` does not declare is a route to \
         nowhere: nothing will ever store or fetch those bytes, and the entry reads as though \
         something will. Both sides are committed, so this check answers identically in every \
         clone — it reports a defect in the corpus, never a fact about your credentials. \
         `vault: none` is a route and means the local cache and nowhere else.",
};

/// An artifact routed to a vault this repository does not declare.
///
/// # Why this may gate
///
/// **Both sides are committed.** The record is in `.yidam/catalog/`, the vault is in
/// `.yidam/config.toml`, and the check compares one against the other — so it answers
/// identically in every clone, and a finding is a defect in the corpus rather than a fact
/// about your credentials or your network. Whether a *declared* vault can actually be reached
/// is a different question and belongs to `yidam vault status`.
///
/// `vault: none` is a route, not an absence: it says these bytes stay in the local cache. It
/// is spelled rather than omitted so that "nobody has decided" and "decided to keep it here"
/// are different states.
pub fn catalog_artifact_unroutable(sources: &[Source], declared: &[String]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, a) in s.artifacts.iter().enumerate() {
            let Some(name) = a.vault.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
                continue;
            };
            if name == "none" || declared.iter().any(|d| d == name) {
                continue;
            }
            let known = if declared.is_empty() {
                "this repository declares no vault".to_string()
            } else {
                format!(
                    "declared: {}",
                    declared
                        .iter()
                        .map(|d| format!("`{d}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            violations.push(Violation::new(
                &s.rel,
                format!("artifacts[{i}]: `vault: {name}` names no declared vault ({known})"),
            ));
        }
    }
    Check::new(&CATALOG_ARTIFACT_UNROUTABLE, violations)
}

pub const CATALOG_LOCATION_MALFORMED: CheckSpec = CheckSpec {
    id: "catalog-location-malformed",
    title: "Catalog location missing or mistyped",
    severity: Severity::Warn,
    rationale: "A `location` is a list of typed places. The type decides whether a reader is \
         offered a link, so a value whose shape contradicts its type renders wrongly; and \
         where an entry has several locations, the description is the only thing \
         distinguishing them. An `identifier` names a source by what it is rather than where \
         it is, as `scheme:local-id` — `doi:10.1167/tvst.8.5.14` — and one kind serves every \
         scheme so that the set of kinds stays closed. Once the repository enables a source \
         pack, the scheme must be one an enabled pack declares and the local id must match \
         its pattern, since that pack is what resolves it; a repository that enables none \
         is held to the shape alone. A file inside a fetched zip is named under `members:` \
         beside the value, never in it, and each must be a path that stays inside the \
         archive. A `url` carrying a `#` fragment is reported: the fragment is never sent, \
         so the fetch takes the whole file while the entry reads as holding the part.",
};

pub fn catalog_location_malformed(
    sources: &[Source],
    packs: &crate::sources::resolve::Enabled,
) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, loc) in s.locations.iter().enumerate() {
            let mut problems = Vec::new();
            match loc.kind.as_deref() {
                None => problems.push("no `kind`".to_string()),
                Some(k) if CATALOG_LOCATION_KINDS_RETIRED.contains(&k) => {
                    problems.push(format!(
                        "kind `{k}` is not one of {}; `yidam migrate locations` rewrites it as \
                         `kind: identifier, value: {k}:…`",
                        CATALOG_LOCATION_KINDS.join(", ")
                    ));
                }
                Some(k) if !CATALOG_LOCATION_KINDS.contains(&k) => {
                    problems.push(format!(
                        "kind `{k}` is not one of {}",
                        CATALOG_LOCATION_KINDS.join(", ")
                    ));
                }
                Some(k) => {
                    // The type decides whether a reader is offered a link, so a value
                    // whose shape contradicts its type renders wrongly.
                    let v = loc.value.as_deref().unwrap_or("");
                    if k == "url" && !v.starts_with("http://") && !v.starts_with("https://") {
                        problems.push(format!("kind `url` but value is not a URL: {v}"));
                    }
                    // The fragment never reaches the server, so the entry would read as
                    // holding what the fragment names while the fetch takes the whole file.
                    if let (Some((fetched, _)), "url") = (v.split_once('#'), k) {
                        problems.push(format!(
                            "kind `url` carries a `#` fragment, which is never sent: \
                             `catalog-fetch` fetches {fetched} whole; name a file inside a zip \
                             under `members:`"
                        ));
                    }
                    if k == "url_template" && !v.contains('{') {
                        problems
                            .push("kind `url_template` but value has no `{…}` placeholder".into());
                    }
                    if k == "identifier" && !v.trim().is_empty() {
                        if let Some(p) = identifier_problem(v).or_else(|| undeclared(v, packs)) {
                            problems.push(p);
                        }
                    }
                }
            }
            if loc.value.as_deref().unwrap_or("").trim().is_empty() {
                problems.push("no `value`".to_string());
            }
            for m in loc.members.iter().flatten() {
                if let Some(why) = crate::cmd::catalog::member::refusal(m) {
                    problems.push(why);
                }
            }
            if s.locations.len() > 1 && loc.description.is_none() {
                problems.push("several locations and no `description` to tell them apart".into());
            }
            if !problems.is_empty() {
                violations.push(Violation::new(
                    &s.rel,
                    format!("location[{i}]: {}", problems.join("; ")),
                ));
            }
        }
    }
    Check::new(&CATALOG_LOCATION_MALFORMED, violations)
}

/// What is wrong with an `identifier` location's value, if anything.
///
/// `scheme:local-id`, both halves non-empty. What is refused here is a value that is not the
/// shape at all, and a URL, whose `https:` would otherwise read as a scheme. Whether a pack
/// declares the scheme is [`undeclared`]'s.
fn identifier_problem(v: &str) -> Option<String> {
    let Some((scheme, local)) = v.split_once(':') else {
        return Some(format!(
            "kind `identifier` but value is not `scheme:local-id`: {v}"
        ));
    };
    if scheme.is_empty() || scheme.chars().any(char::is_whitespace) {
        return Some(format!("kind `identifier` but value names no scheme: {v}"));
    }
    if matches!(scheme, "http" | "https") {
        return Some(format!(
            "kind `identifier` but value is a URL, so kind `url`: {v}"
        ));
    }
    if local.trim().is_empty() {
        return Some(format!(
            "kind `identifier` but scheme `{scheme}` has no local id"
        ));
    }
    None
}

/// What the enabled packs say is wrong with a well-shaped `identifier` value (RFC-0048 §2).
///
/// Nothing when the repository enables no pack: a corpus that lists none sees no change
/// (RFC-0048, *Migration & compatibility*), and the `doi:` locations `yidam migrate locations`
/// wrote are not wrong for want of a pack. Once one is enabled, the packs are the scheme set.
fn undeclared(v: &str, packs: &crate::sources::resolve::Enabled) -> Option<String> {
    use crate::sources::resolve::{template_pack, Refusal};
    if packs.is_empty() {
        return None;
    }
    match packs.admits(v).err()? {
        Refusal::NoPack { scheme } => Some(match template_pack(&scheme) {
            Some(pack) => format!(
                "no enabled source pack declares scheme `{scheme}`; the template's `{pack}` \
                 pack does, so pin it under prelude_sources: {v}"
            ),
            None => format!(
                "no enabled source pack declares scheme `{scheme}`; write one in {}: {v}",
                crate::sources::AUTHORED
            ),
        }),
        Refusal::Malformed {
            scheme,
            local,
            pattern,
        } => Some(format!(
            "`{local}` does not match [scheme.{scheme}] pattern `{pattern}`: {v}"
        )),
        _ => None,
    }
}

pub const CATALOG_TYPE_UNKNOWN: CheckSpec = CheckSpec {
    id: "catalog-type-unknown",
    title: "Catalog type outside the closed set",
    severity: Severity::Warn,
    rationale:
        "A catalog `type` says what form a source takes, and the set is closed so that a reader \
         and the schema agree on what each word means. A word outside it is reported rather than \
         mapped: `primary` says what standing a source has, and a primary source can be a \
         statute, a dataset or a report, so only someone who read it can choose.",
};

/// A catalog entry whose `type:` is outside the closed set.
///
/// The set grew by four in RFC-0048 §2.1 — `statute`, `report`, `standard`, `document` — which
/// covers 178 of the entries corpora had typed outside it. What it does not cover is reported
/// here and resolved by hand. `primary` is the large case: it says what standing a source has,
/// not what form it takes, and only the person who read the source knows which form that is.
/// Nothing maps it, here or in a migration.
///
/// Warn, because the population is not empty: every corpus that coined a type is in it.
pub fn catalog_type_unknown(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        let Some(t) = s.r#type.as_deref() else {
            continue;
        };
        if CATALOG_TYPES.contains(&t) {
            continue;
        }
        let standing = if t == "primary" {
            "`primary` is a standing, not a form; "
        } else {
            ""
        };
        violations.push(Violation::new(
            &s.rel,
            format!(
                "`type: {t}` is not a catalog type — {standing}is it a `statute`, `report`, \
                 `standard` or `document`? (all: {})",
                CATALOG_TYPES.join(", ")
            ),
        ));
    }
    Check::new(&CATALOG_TYPE_UNKNOWN, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    /// The catalog directory every staged node here resolves an edge `source:` against.
    /// `/repo/.yidam/catalog`, which is where `catalog_source` and `source` put their entries.
    fn catalog_dir() -> PathBuf {
        PathBuf::from("/repo/.yidam/catalog")
    }

    /// An ontology that declares no quotation, for every citation test that is not about one.
    fn no_quotations() -> Declared<'static> {
        static UNIVERSAL: std::sync::LazyLock<crate::universal::Universal> =
            std::sync::LazyLock::new(Default::default);
        Declared::new(&[], &UNIVERSAL)
    }

    /// The `concept` class the quotation tests read through (#1174). `anchors` is declared a
    /// quotation, and `note` is declared prose.
    fn quoting() -> Vec<Class> {
        vec![Class::parse(
            "concept.ont.yml",
            "properties:\n  - name: anchors\n    type: quotation\n  \
             - name: note\n    type: string\n",
        )]
    }

    /// `None` and an empty drift are different answers, and the report emits them as
    /// different values — `null` for an entry that never claimed anything, an object with two
    /// empty arrays for one whose claim holds. Collapsing them would make an entry that keeps
    /// its list current indistinguishable from one that has none.
    #[test]
    fn an_absent_used_by_list_is_not_drift_and_an_accurate_one_is_not_either() {
        assert_eq!(used_by_drift(None, &["a.yml".into()]), None);
        assert_eq!(
            used_by_drift(Some(&["a.yml".into()]), &[".yidam/corpus/c/a.yml".into()]),
            Some(UsedByDrift::default()),
        );
    }

    /// The three states the old slice signature could not tell apart, each asserted against
    /// the same citing node. This is the regression: for as long as an empty `Vec` and an
    /// absent key arrived here as the same value, `used-by: []` was a silent opt-out of the
    /// check — and it is the state a hand-authored entry starts in.
    ///
    /// Three cases and not two. The empty-declared arm has to be checked both ways round:
    /// firing when nodes cite the entry, *and* staying silent when none do. A fix that
    /// dropped the empty check entirely would pass the first assertion alone.
    #[test]
    fn an_empty_used_by_list_is_a_claim_and_an_absent_one_is_not() {
        let citing = [".yidam/corpus/concept/tailwater.yml".to_string()];

        // Absent: undeclared, and exempt. Unchanged.
        assert_eq!(used_by_drift(None, &citing), None);

        // Declared empty, and cited: a false claim, named in the direction it is false.
        let drift = used_by_drift(Some(&[]), &citing).expect("`[]` declares a list");
        assert_eq!(drift.citing_not_claimed, vec!["tailwater.yml"]);
        assert!(drift.claimed_not_citing.is_empty());

        // Declared empty, and cited by nothing: the sets agree, so no finding — which is
        // why this needs no special case.
        let agrees = used_by_drift(Some(&[]), &[]).expect("`[]` declares a list");
        assert_eq!(agrees, UsedByDrift::default());
    }

    /// Basenames, because the list is hand-written and a person writes `tailwater.yml`.
    /// Comparing full paths would report drift on every entry spelled the way the
    /// convention asks for.
    #[test]
    fn drift_is_reported_in_both_directions_and_compared_by_basename() {
        let drift = used_by_drift(
            Some(&["mixing-zone.yml".into(), "low-flow.yml".into()]),
            &[
                ".yidam/corpus/concept/low-flow.yml".into(),
                ".yidam/corpus/concept/tailwater.yml".into(),
            ],
        )
        .expect("a list was declared");
        assert_eq!(drift.claimed_not_citing, vec!["mixing-zone.yml"]);
        assert_eq!(drift.citing_not_claimed, vec!["tailwater.yml"]);
    }

    /// `used_by` is an `Option` and not a slice so that every fixture has to say which of the
    /// two states it means. `None` is a catalog entry with no `used-by:` key; `Some(&[])` is
    /// one that wrote `used-by: []`. While both spelled `&[]`, no fixture could reach the
    /// second state at all, and the tests below covered one case believing it was both.
    fn source(slug: &str, obtained: bool, used_by: Option<&[&str]>) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            r#type: None,
            used_by: used_by.map(|u| u.iter().map(|s| s.to_string()).collect()),
            locations: vec![],
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
            columns: None,
            key: None,
            text: String::new(),
            malformed: None,
        }
    }

    /// The `type: claim` field a class declared, which is what the structural arm reads.
    ///
    /// Keyed on `concept` rather than on the node's `class:` field, because `class_of`
    /// reads the *directory* — instances live in `corpus/<class>/` and the two agree by
    /// convention. A fixture keyed on the YAML field silently exercises the prose arm only.
    fn claim_fields() -> crate::claims::ClaimFields {
        crate::claims::ClaimFields::from_declarations([(
            "concept".to_string(),
            vec!["claim_tag".to_string()],
        )])
    }

    /// A `[verified]` claim whose only source is the one an edge names (#1158). The edge
    /// resolved under `edge-source-unresolved` and the node was still called unsourced, which
    /// is the disagreement [`citations`] and this check promise not to have.
    #[test]
    fn a_verified_claim_resting_on_an_edge_source_is_sourced() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks:\n  \
             - target: ../concept/b.yml\n    claim_tag: verified\n    source: nwis\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// The finding: a claim at the strongest standing, and nothing registered beneath it.
    #[test]
    fn a_verified_claim_with_no_citation_is_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks:\n  \
             - target: ../concept.ont.yml\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, ".yidam/corpus/concept/a.yml");
        assert!(
            c.violations[0].detail.contains("1 `[verified]` claim"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A link that resolves to a catalog entry is what "rests on something" means, and it is
    /// the same resolution `citations` performs.
    #[test]
    fn a_verified_claim_citing_a_source_is_not_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks:\n  \
             - target: ../../catalog/nwis.md\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// The vocabulary's other two standings assert nothing this check is about. A corpus
    /// that is honest about what it does not know must not be reported for it.
    #[test]
    fn an_inference_or_an_open_question_is_not_reported() {
        let nodes = vec![
            corpus_node(
                "a",
                "class: c\ndescription: |\n  A conclusion. [inference]\nlinks: []\n",
            ),
            corpus_node(
                "b",
                "class: c\ndescription: |\n  A question. [open]\nlinks: []\n",
            ),
        ];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// A node-level standing declared in a `type: claim` property is a claim too — the
    /// structural arm, which a text-only scan misses entirely.
    #[test]
    fn a_structural_claim_tag_counts() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  Prose with no marker.\nproperties:\n  \
             claim_tag: verified\nlinks: []\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.violations.len(), 1, "the structural arm read nothing");
    }

    /// A `cites:` into a dependency does not discharge a local `[verified]`. RFC-0019: a
    /// foreign tag is the producer's, records what *that* corpus's electors accepted, and
    /// travels without the apparatus that made it accountable.
    #[test]
    fn a_citation_into_a_dependency_does_not_count_as_a_source() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\ncites:\n  \
             - package: upstream\n    node: concept/x.yml\n    tag: verified\nlinks: []\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("does not transfer"),
            "the finding should say why a foreign tag is not enough: {:?}",
            c.violations[0]
        );
    }

    /// Several claims in one node are one finding. The baseline compares on `(check, node)`,
    /// so a node cannot carry two entries for the same check — and the count is what a
    /// reader needs, not five identical lines.
    #[test]
    fn several_verified_claims_in_one_node_are_one_finding() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  One. [verified] Two. [verified] Three. \
             [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.starts_with("3 `[verified]` claims"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A corpus with no catalog at all reports every verified claim, which is correct and is
    /// the state a corpus is in before it registers its first source.
    #[test]
    fn with_no_registered_sources_every_verified_claim_is_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.violations.len(), 1);
    }

    /// It reports and does not gate. Adopting it must not break the build of every corpus
    /// that predates it, and the fix — a citation or a demotion — is an author's judgement.
    #[test]
    fn the_check_reports_rather_than_gates() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(c.severity, Severity::Warn);
        assert!(!c.gates(&c.violations[0]));
    }

    fn aged(
        entry: &str,
        retrieved: Option<&str>,
        dated: Option<super::super::super::ttl::Dated>,
        age: Option<i64>,
        ttl: Option<u32>,
    ) -> super::super::super::ttl::Age {
        super::super::super::ttl::Age {
            entry: entry.to_string(),
            retrieved: retrieved.map(str::to_string),
            dated,
            age_days: age,
            ttl_days: ttl,
        }
    }

    /// [`catalog_expired`] against entries nothing cites — the shape most of these tests
    /// are about, where the citation list is not the thing under test.
    fn expired(ages: &[super::super::super::ttl::Age]) -> Check {
        let none: Vec<Vec<String>> = ages.iter().map(|_| Vec::new()).collect();
        expired_cited(ages, &none)
    }

    /// [`catalog_expired`] with a citation list per entry, in the same order.
    fn expired_cited(ages: &[super::super::super::ttl::Age], cites: &[Vec<String>]) -> Check {
        let sources: Vec<Source> = ages
            .iter()
            .map(|a| Source {
                rel: a.entry.clone(),
                path: std::path::PathBuf::from(&a.entry),
                obtained: true,
                r#type: None,
                used_by: None,
                locations: vec![],
                retrieved: a.retrieved.clone(),
                ttl_days: a.ttl_days,
                artifacts: Vec::new(),
                columns: None,
                key: None,
                text: String::new(),
                malformed: None,
            })
            .collect();
        catalog_expired(ages, &sources, cites)
    }

    fn resting(nodes: &[&str]) -> Vec<String> {
        nodes.iter().map(|n| format!(".yidam/corpus/{n}")).collect()
    }

    /// A corpus that declared no TTL asked nothing, and is told nothing.
    #[test]
    fn an_entry_with_no_ttl_is_not_reported() {
        let c = expired(&[aged("a.md", Some("1999-01-01"), None, Some(9_000), None)]);
        assert!(c.passed(), "{:?}", c.violations);
    }

    #[test]
    fn an_entry_inside_its_ttl_is_not_reported() {
        let c = expired(&[aged("a.md", Some("2026-08-01"), None, Some(25), Some(180))]);
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// The finding names the date, where the date came from, and how far past it is — the
    /// three things a maintainer needs before deciding whether to re-fetch.
    #[test]
    fn an_expired_entry_says_how_it_was_dated() {
        let c = expired(&[aged(
            "a.md",
            Some("2024-01-15"),
            Some(super::super::super::ttl::Dated::Declared),
            Some(954),
            Some(30),
        )]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("2024-01-15"), "{d}");
        assert!(d.contains("declared"), "{d}");
        assert!(d.contains("924 day(s) past"), "{d}");
    }

    /// A date read from git is labelled as such: it counts a typo fix as a refresh, so it
    /// errs in the flattering direction and a reader is owed the difference.
    #[test]
    fn a_git_dated_entry_says_so() {
        let c = expired(&[aged(
            "a.md",
            Some("2024-01-15"),
            Some(super::super::super::ttl::Dated::Committed),
            Some(954),
            Some(30),
        )]);
        assert!(
            c.violations[0].detail.contains("from git"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A TTL with nothing to measure against is a gap in the bookkeeping, not a stale
    /// source. Calling it expired would assert something nobody knows.
    #[test]
    fn an_undatable_entry_is_reported_as_undatable_and_not_as_expired() {
        let c = expired(&[aged("a.md", None, None, None, Some(30))]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("nothing records when it was fetched"), "{d}");
        assert!(!d.contains("past"), "it must not read as an expiry: {d}");
    }

    /// It reports and does not gate. An aged record is a thing to look at, and refreshing it
    /// is a knowledge event a person owns.
    #[test]
    fn an_expired_entry_reports_rather_than_gates() {
        let c = expired(&[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))]);
        assert_eq!(c.severity, Severity::Warn);
        assert!(!c.gates(&c.violations[0]));
    }

    /// #270's gap, closed the cheap way: the question stays on the source and the nodes
    /// resting on it are named, rather than a copy of it being written into each of them.
    #[test]
    fn an_expired_entry_names_the_nodes_resting_on_it() {
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&["gage/canyon-outlet.yml", "reach/tailwater.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("2 node(s) rest on it"), "{d}");
        assert!(d.contains("gage/canyon-outlet.yml"), "{d}");
        assert!(d.contains("reach/tailwater.yml"), "{d}");
        assert!(!d.contains("more"), "two fits without a tail: {d}");
    }

    /// The address `rename` takes, not a bare stem: two classes may hold a node of the same
    /// name, and a list of stems would not say which one aged.
    #[test]
    fn a_resting_node_is_named_by_class_and_stem() {
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&["gage/outlet.yml", "reach/outlet.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("gage/outlet.yml, reach/outlet.yml"), "{d}");
        assert!(
            !d.contains(".yidam/corpus/"),
            "the prefix is noise here: {d}"
        );
    }

    /// The tail is counted, not printed. Measured: one entry in a real corpus is cited by
    /// 84 nodes, and a finding that listed them would be unreadable — which is the same
    /// reason the question is not written into each of them.
    #[test]
    fn a_long_citation_list_is_named_three_deep_and_then_counted() {
        let many: Vec<&str> = vec![
            "a/1.yml", "a/2.yml", "a/3.yml", "a/4.yml", "a/5.yml", "a/6.yml",
        ];
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&many)],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("6 node(s) rest on it"), "{d}");
        assert!(d.contains("a/1.yml, a/2.yml, a/3.yml, and 3 more"), "{d}");
        assert!(!d.contains("a/4.yml"), "{d}");
    }

    /// Worth saying, and it changes what the reader does: an aged record nothing rests on
    /// may want deleting rather than refreshing.
    #[test]
    fn an_expired_entry_nothing_cites_says_so() {
        let c = expired(&[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))]);
        assert!(
            c.violations[0].detail.contains("Nothing cites it"),
            "{:?}",
            c.violations[0]
        );
    }

    /// The same stake, whichever silence it is. An entry whose bookkeeping is incomplete is
    /// one whose citing nodes cannot be told how old their evidence is either.
    #[test]
    fn an_undatable_entry_also_names_what_rests_on_it() {
        let c = expired_cited(
            &[aged("a.md", None, None, None, Some(30))],
            &[resting(&["gage/canyon-outlet.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("1 node(s) rest on it"), "{d}");
        assert!(!d.contains("past"), "still not an expiry: {d}");
    }

    /// RFC-0050: an evidenced entry's `source` cites, as an edge's does (#1174) — so a
    /// catalog entry only an entry names is not `catalog-uncited`.
    #[test]
    fn an_evidenced_entrys_source_is_a_citation() {
        let (_d, classes) = loaded_class(
            "pathway",
            "class: pathway\ndescription: A pathway.\nproperties:\n  - name: effects\n    type: list\n    fields:\n      - { name: claim_tag, type: claim }\n      - { name: source, type: ref }\n",
        );
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        let text = "class: pathway\nlabel: P\nproperties:\n  effects:\n    - claim_tag: verified\n      source: ridership-2019\nlinks: []\n";
        let cited = linked_paths(
            Path::new(".yidam/corpus/pathway/p.yml"),
            ".yidam/corpus/pathway/p.yml",
            text,
            Path::new(".yidam/catalog"),
            &quotations,
        );
        assert!(
            cited.contains(&normalize(Path::new(".yidam/catalog/ridership-2019.md"))),
            "{cited:?}"
        );
    }

    /// A source at a known path, and a node in a directory that can reach it.
    fn catalog_source(slug: &str, obtained: bool) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            r#type: None,
            used_by: None,
            locations: vec![],
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
            columns: None,
            key: None,
            text: String::new(),
            malformed: None,
        }
    }

    // ── obtained-artifact records ────────────────────────────────────────────

    const GOOD: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn with_artifacts(slug: &str, yaml: &str) -> Source {
        let mut s = catalog_source(slug, true);
        s.artifacts = serde_yaml::from_str(yaml).expect("test fixture parses");
        s
    }

    /// The upgrade story, and the reason both checks are written the way they are: a corpus
    /// that has not adopted `artifacts:` must see nothing new. Every entry in every existing
    /// corpus is this case on the day the field ships.
    #[test]
    fn an_entry_declaring_no_artifacts_produces_no_findings() {
        let sources = vec![catalog_source("pearl-2009", true)];
        assert!(catalog_artifact_malformed(&sources).violations.is_empty());
        assert!(catalog_artifact_unroutable(&sources, &[])
            .violations
            .is_empty());
    }

    #[test]
    fn a_well_formed_record_is_silent() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  bytes: 4194304\n  media_type: application/pdf\n"),
        )];
        assert!(catalog_artifact_malformed(&sources).violations.is_empty());
    }

    #[test]
    fn a_record_without_a_digest_names_nothing_and_is_reported() {
        let sources = vec![with_artifacts("pearl-2009", "- bytes: 10\n")];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("no `sha256`"), "{:?}", v[0].detail);
    }

    /// Hex is case-insensitive and a content-addressed store is not, so an uppercase digest
    /// is a second name for one artifact. The message carries the fix.
    #[test]
    fn an_uppercase_digest_is_reported_with_the_lowercase_form() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            &format!("- sha256: {}\n", GOOD.to_ascii_uppercase()),
        )];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains(GOOD),
            "carries the fix: {:?}",
            v[0].detail
        );
    }

    #[test]
    fn a_digest_of_the_wrong_length_or_alphabet_is_reported() {
        let sources = vec![
            with_artifacts("short", "- sha256: abc123\n"),
            with_artifacts("nonhex", &format!("- sha256: {}\n", "z".repeat(64))),
        ];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 2);
        assert!(v[0].detail.contains("not 64"), "{:?}", v[0].detail);
        assert!(v[1].detail.contains("not hex"), "{:?}", v[1].detail);
    }

    /// A text reading is a second digest held to the first one's form, and it names the
    /// extractor that took it (#1172).
    #[test]
    fn a_text_reading_is_held_to_a_digest_and_an_extractor() {
        let ok = with_artifacts(
            "ok",
            &format!("- sha256: {GOOD}\n  text:\n    sha256: {GOOD}\n    extractor: x 1\n"),
        );
        assert!(catalog_artifact_malformed(&[ok]).violations.is_empty());
        let sources = vec![
            with_artifacts(
                "short",
                &format!("- sha256: {GOOD}\n  text:\n    sha256: abc\n    extractor: x 1\n"),
            ),
            with_artifacts(
                "anonymous",
                &format!("- sha256: {GOOD}\n  text:\n    sha256: {GOOD}\n"),
            ),
        ];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v[0].detail.contains("`text.sha256` is 3"),
            "{:?}",
            v[0].detail
        );
        assert!(v[1].detail.contains("no `extractor`"), "{:?}", v[1].detail);
    }

    /// `readings:` is held to what `text:` is, and names its media type (#1318).
    #[test]
    fn a_reading_is_held_to_a_digest_a_media_type_and_what_took_it() {
        let ok = with_artifacts(
            "ok",
            &format!(
                "- sha256: {GOOD}\n  readings:\n    - sha256: {GOOD}\n      media_type: text/plain\n      by: x 1\n"
            ),
        );
        assert!(catalog_artifact_malformed(&[ok]).violations.is_empty());
        let bad = with_artifacts(
            "bad",
            &format!(
                "- sha256: {GOOD}\n  readings:\n    - sha256: {GOOD}\n      media_type: text/plain\n      by: x 1\n    - sha256: abc\n"
            ),
        );
        let v = &catalog_artifact_malformed(&[bad]).violations;
        assert_eq!(v.len(), 1, "{v:?}");
        for want in [
            "`readings[1].sha256` is 3",
            "`readings[1]` names no `media_type`",
            "`readings[1]` names no `by`",
        ] {
            assert!(v[0].detail.contains(want), "{want}: {:?}", v[0].detail);
        }
    }

    /// A `from:` index naming no location means the record cites a provenance the entry does
    /// not carry — the one cross-field error worth having here.
    #[test]
    fn a_from_index_naming_no_location_is_reported() {
        let mut s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n  from: 2\n"));
        s.locations = vec![crate::parse::CatalogLocation {
            members: None,
            commit: None,
            repo: None,
            symbol: None,
            kind: Some("url".into()),
            value: Some("https://example.org".into()),
            description: None,
        }];
        let v = &catalog_artifact_malformed(&[s]).violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("from: 2"), "{:?}", v[0].detail);
        assert!(v[0].detail.contains("declares 1"), "{:?}", v[0].detail);
    }

    #[test]
    fn a_from_url_is_not_an_index_and_is_never_out_of_range() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  from: https://example.org/paper.pdf\n"),
        );
        assert!(catalog_artifact_malformed(&[s]).violations.is_empty());
    }

    /// Each malformed record is reported once, indexed, so an entry with several says which.
    #[test]
    fn several_problems_in_one_record_are_reported_as_one_violation() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            "- sha256: abc\n  from: 5\n- bytes: 1\n",
        )];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 2, "one per record, not one per problem");
        assert!(v[0].detail.starts_with("artifacts[0]"), "{:?}", v[0].detail);
        assert!(v[1].detail.starts_with("artifacts[1]"), "{:?}", v[1].detail);
        assert!(
            v[0].detail.contains(';'),
            "both problems: {:?}",
            v[0].detail
        );
    }

    // ── routing ──────────────────────────────────────────────────────────────

    #[test]
    fn an_artifact_routed_to_a_declared_vault_is_silent() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: sources\n"),
        );
        let declared = vec!["default".to_string(), "sources".to_string()];
        assert!(catalog_artifact_unroutable(&[s], &declared)
            .violations
            .is_empty());
    }

    /// `none` is a route — the local cache and nowhere else — rather than an absence, so it
    /// resolves without any vault being declared at all.
    #[test]
    fn vault_none_is_a_route_and_needs_nothing_declared() {
        let s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n  vault: none\n"));
        assert!(catalog_artifact_unroutable(&[s], &[]).violations.is_empty());
    }

    #[test]
    fn an_artifact_naming_no_declared_vault_is_reported_with_what_is_declared() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: archive\n"),
        );
        let declared = vec!["default".to_string()];
        let v = &catalog_artifact_unroutable(&[s], &declared).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains("`vault: archive`"),
            "{:?}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("`default`"),
            "names what is declared: {:?}",
            v[0].detail
        );
    }

    /// A corpus with records and no vault at all is a common half-configured state, and the
    /// message has to say that rather than listing an empty set.
    #[test]
    fn with_no_vault_declared_the_message_says_so_rather_than_listing_nothing() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: default\n"),
        );
        let v = &catalog_artifact_unroutable(&[s], &[]).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains("declares no vault"),
            "{:?}",
            v[0].detail
        );
    }

    /// A record that does not name a vault has not been routed yet, which is not an error —
    /// the route falls back to what the vault itself declares it holds.
    #[test]
    fn a_record_naming_no_vault_is_not_unroutable() {
        let s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n"));
        assert!(catalog_artifact_unroutable(&[s], &[]).violations.is_empty());
    }

    fn corpus_node(name: &str, yaml: &str) -> Node {
        Node::parse(
            PathBuf::from(format!("/repo/.yidam/corpus/concept/{name}.yml")),
            format!(".yidam/corpus/concept/{name}.yml"),
            yaml,
        )
    }

    /// The reported case. A catalog entry whose slug collides with a connector crate — the
    /// conventions recommend naming connectors after what they fetch — and a node that
    /// mentions the crate in prose and links nothing. It failed a build at Error severity on
    /// a node that cites nothing.
    #[test]
    fn naming_a_slug_in_prose_is_not_a_citation() {
        let sources = vec![catalog_source("nwis", false)];
        let node = corpus_node(
            "gauge-ingest",
            "class: concept\nlabel: Gauge ingest\ndescription: The `nwis` crate fetches the \
             series; nwis is also the source name.\nlinks:\n  - target: ../concept/other.yml\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(cites, vec![Vec::<String>::new()], "no link resolves to it");
        assert!(catalog_unobtained_but_cited(&sources, &cites).passed());
    }

    /// And a real citation still is one. Without this the fix is indistinguishable from
    /// deleting the check.
    #[test]
    fn a_markdown_link_that_resolves_to_the_entry_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on \
             [Pearl 2009](../../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(
            cites[0],
            vec![".yidam/corpus/concept/confounding.yml".to_string()]
        );
        let c = catalog_unobtained_but_cited(&sources, &cites);
        assert_eq!(
            c.violations.len(),
            1,
            "an unfetched source, cited, still gates"
        );
    }

    /// A structured edge is a citation too — the corpus writes both forms.
    #[test]
    fn a_links_entry_pointing_at_the_entry_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../../catalog/pearl-2009.md\n    relationship: cites\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(cites[0].len(), 1);
        assert!(catalog_uncited(&sources, &cites).passed());
    }

    /// The reported case (#1158). A corpus that cites its sources from edges — 140 of them in
    /// one derived repository, every one a catalog stem — had every such entry reported
    /// uncited, while `edge-source-unresolved` resolved every one of the edges.
    #[test]
    fn an_edge_source_naming_the_entry_by_stem_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../concept/o.yml\n    relationship: refines\n    claim_tag: verified\n    \
             source: pearl-2009\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(
            cites[0],
            vec![".yidam/corpus/concept/confounding.yml".to_string()]
        );
        assert!(catalog_uncited(&sources, &cites).passed());
    }

    /// The second spelling `edge-source-unresolved` admits: a path written as a `links:`
    /// target is. It resolves against the node's own directory, as a target does.
    #[test]
    fn an_edge_source_naming_the_entry_by_path_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../concept/o.yml\n    source: ../../catalog/pearl-2009.md\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(cites[0].len(), 1);
        assert_eq!(
            catalog_unobtained_but_cited(&sources, &cites)
                .violations
                .len(),
            1,
            "an edge resting on an unfetched source gates as a prose citation of it does"
        );
    }

    /// An edge source that resolves to nothing — a renamed entry, a decision record, a blank
    /// — is not a citation of anything. A decision record in particular is not a catalog entry,
    /// and the bare stem tried as a path lands in the node's own class directory.
    #[test]
    fn an_edge_source_naming_nothing_in_the_catalog_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../concept/o.yml\n    source: pearl-2009-old\n  \
             - target: ../concept/p.yml\n    source: sampling-frame\n  \
             - target: ../concept/q.yml\n    source: ''\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(cites, vec![Vec::<String>::new()]);
        assert_eq!(catalog_uncited(&sources, &cites).violations.len(), 1);
    }

    /// A node quoting pearl-2009 through `key`, with `of:` written as `of`.
    fn quoting_node(key: &str, of: &str) -> Node {
        corpus_node(
            "confounding",
            &format!(
                "class: concept\nlabel: Confounding\ndescription: Quotes it.\nproperties:\n  \
                 {key}:\n    of: {of}\n    span: the words\nlinks:\n  - target: ../concept/o.yml\n"
            ),
        )
    }

    /// The reported case (#1174). A quotation's `of:` resolves through the reading an edge
    /// `source:` does, and `quotation-unresolved` resolved it, while the entry it named was
    /// still reported uncited.
    #[test]
    fn a_quotation_naming_the_entry_by_stem_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        let cites = citations(
            &sources,
            &[quoting_node("anchors", "pearl-2009")],
            &catalog_dir(),
            &quotations,
        );
        assert_eq!(
            cites[0],
            vec![".yidam/corpus/concept/confounding.yml".to_string()]
        );
        assert!(catalog_uncited(&sources, &cites).passed());
    }

    /// The path spelling counts too, and a quotation of an unfetched source is a citation of
    /// it, as an edge resting on one is.
    #[test]
    fn a_quotation_naming_the_entry_by_path_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        let cites = citations(
            &sources,
            &[quoting_node("anchors", "../../catalog/pearl-2009.md")],
            &catalog_dir(),
            &quotations,
        );
        assert_eq!(cites[0].len(), 1);
        assert_eq!(
            catalog_unobtained_but_cited(&sources, &cites)
                .violations
                .len(),
            1
        );
    }

    /// Only a declared quotation counts. The same mapping under a key the class declares as
    /// prose, or under one nothing declares, is a value no quotation check reads. A class the
    /// corpus does not define declares nothing.
    #[test]
    fn a_quotation_shape_under_an_undeclared_key_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        for node in [
            quoting_node("note", "pearl-2009"),
            quoting_node("stray", "pearl-2009"),
        ] {
            let cites = citations(&sources, &[node], &catalog_dir(), &quotations);
            assert_eq!(cites, vec![Vec::<String>::new()]);
        }
        let cites = citations(
            &sources,
            &[quoting_node("anchors", "pearl-2009")],
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(cites, vec![Vec::<String>::new()], "no class is defined");
    }

    /// A value lint cannot read names nothing, and neither does an `of:` that resolves to no
    /// entry. `property-type` and `quotation-unresolved` report those, so nothing can rest on
    /// them here.
    #[test]
    fn a_malformed_or_unresolved_quotation_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        let malformed = corpus_node(
            "confounding",
            "class: concept\nlabel: C\ndescription: Quotes it.\nproperties:\n  anchors:\n    \
             of: pearl-2009\nlinks:\n  - target: ../concept/o.yml\n",
        );
        for node in [malformed, quoting_node("anchors", "pearl-2009-old")] {
            let cites = citations(&sources, &[node], &catalog_dir(), &quotations);
            assert_eq!(cites, vec![Vec::<String>::new()]);
        }
    }

    /// `universal.yml` declares a quotation for every class, and a class's own declaration of
    /// the key wins over it. That is `property-type`'s order.
    #[test]
    fn a_universal_quotation_counts_unless_the_class_declares_the_key() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let universal = crate::universal::Universal::parse(
            "properties:\n  - name: anchors\n    type: quotation\n  \
             - name: note\n    type: quotation\n",
        );
        let classes = vec![Class::parse(
            "concept.ont.yml",
            "properties:\n  - name: note\n    type: string\n",
        )];
        let quotations = Declared::new(&classes, &universal);
        let cited = |key| {
            citations(
                &sources,
                &[quoting_node(key, "pearl-2009")],
                &catalog_dir(),
                &quotations,
            )[0]
            .len()
        };
        assert_eq!(cited("anchors"), 1, "declared in universal.yml");
        assert_eq!(cited("note"), 0, "the class declares it prose");
    }

    /// A `[verified]` claim that rests only on a quotation is not unsourced. It is the same
    /// count, so the two checks cannot disagree on it.
    #[test]
    fn a_verified_claim_resting_on_a_quotation_is_sourced() {
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nproperties:\n  anchors:\n    \
             of: nwis\n    span: the words\nlinks:\n  - target: ../concept.ont.yml\n",
        )];
        let c = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &quotations,
        );
        assert!(c.passed(), "{:?}", c.violations);
        let undeclared = verified_unsourced(
            &nodes,
            &[catalog_source("nwis", true)],
            &claim_fields(),
            &catalog_dir(),
            &no_quotations(),
        );
        assert_eq!(
            undeclared.violations.len(),
            1,
            "the quotation was all it had"
        );
    }

    /// A link shown as an example is not a citation, for the same reason it is not a link.
    #[test]
    fn a_citation_shown_in_code_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let node = corpus_node(
            "conventions",
            "class: concept\nlabel: How to cite\ndescription: Write \
             `[Pearl 2009](../../catalog/pearl-2009.md)` rather than a full \
             citation.\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node], &catalog_dir(), &no_quotations());
        assert_eq!(cites, vec![Vec::<String>::new()]);
    }

    /// `..` in a target must not make two spellings of one file look like two files.
    #[test]
    fn a_citation_is_matched_through_a_normalized_path() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: C\ndescription: See \
             [P](../../corpus/../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        assert_eq!(
            citations(&sources, &[node], &catalog_dir(), &no_quotations())[0].len(),
            1
        );
    }

    /// The two entry points are one implementation, and this is what says so. The `&str` form
    /// parses; the `Node` form reads [`Node::inst`]. Both must answer identically — including
    /// on bytes that do not parse, where each is left with a default instance and the prose
    /// half of the answer.
    #[test]
    fn the_parsed_and_unparsed_entry_points_agree() {
        let classes = quoting();
        let universal = crate::universal::Universal::default();
        let quotations = Declared::new(&classes, &universal);
        for text in [
            "class: concept\nlabel: C\ndescription: See \
             [P](../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
            "class: concept\nlabel: C\ndescription: no links here\n",
            "class: concept\n  label: broken\n\tdescription: [P](../catalog/pearl-2009.md)\n",
            "class: concept\nlabel: C\ndescription: Quotes it.\nproperties:\n  anchors:\n    \
             of: pearl-2009\n    span: the words\n",
        ] {
            let node = corpus_node("confounding", text);
            assert_eq!(
                linked_paths_of(&node, &catalog_dir(), &quotations),
                linked_paths(
                    &node.path,
                    &node.rel,
                    &node.text,
                    &catalog_dir(),
                    &quotations
                ),
                "diverged on {text:?}"
            );
        }
    }

    #[test]
    fn an_unobtained_source_is_not_reported_as_uncited() {
        let s = vec![source("not-yet-fetched", false, None)];
        assert!(catalog_uncited(&s, &[vec![]]).passed());
    }

    #[test]
    fn an_obtained_source_nothing_cites_is_reported() {
        let s = vec![source("fetched", true, None)];
        assert_eq!(catalog_uncited(&s, &[vec![]]).violations.len(), 1);
    }

    #[test]
    fn citing_an_unfetched_source_is_an_error() {
        let s = vec![source("not-yet-fetched", false, None)];
        let c = catalog_unobtained_but_cited(&s, &[vec!["corpus/x/y.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn a_declared_used_by_list_is_reported_whatever_its_value() {
        let s = vec![
            source("listed", true, Some(&["../corpus/a/one.yml"])),
            source("empty", true, Some(&[])),
            source("absent", true, None),
        ];
        let c = catalog_used_by_declared(&s);
        assert_eq!(c.severity, Severity::Warn);
        let named: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(named.len(), 2, "{named:?}");
        assert!(named.iter().any(|n| n.contains("listed")), "{named:?}");
        assert!(named.iter().any(|n| n.contains("empty")), "{named:?}");
        assert!(
            c.violations[0].detail.contains("yidam migrate used-by"),
            "the message names the fix: {}",
            c.violations[0].detail
        );
    }

    /// `used-by: []` is a key the check has to see, so the loader must not flatten it to an
    /// absent one. Through `Overlay` rather than a temp directory: the loader reads catalog
    /// text through it, so the two entries below are parsed by the same path a real file takes.
    #[test]
    fn the_loader_keeps_an_empty_used_by_apart_from_an_absent_one() {
        let root = Path::new("/repo");
        let declared = PathBuf::from("/repo/.yidam/catalog/declared-empty.md");
        let absent = PathBuf::from("/repo/.yidam/catalog/no-key.md");

        let mut overlay = crate::corpus::Overlay::default();
        overlay.set(
            declared.clone(),
            "---\nname: a\nused-by: []\n---\n\n# A\n".into(),
        );
        overlay.set(absent.clone(), "---\nname: b\n---\n\n# B\n".into());

        let sources = crate::corpus::load_sources(root, &[declared, absent], &overlay);
        assert_eq!(sources[0].used_by, Some(vec![]), "`[]` is a declared list");
        assert_eq!(sources[1].used_by, None, "no key is no list");

        let c = catalog_used_by_declared(&sources);
        assert_eq!(
            c.violations.len(),
            1,
            "only the declared-empty entry is reported"
        );
        assert!(c.violations[0].node.contains("declared-empty"));
    }

    #[test]
    fn a_url_location_must_be_a_url() {
        let mut s = source("src", true, None);
        s.locations = vec![crate::parse::CatalogLocation {
            members: None,
            commit: None,
            repo: None,
            symbol: None,
            kind: Some("url".into()),
            value: Some("see the reading room".into()),
            description: None,
        }];
        assert_eq!(
            catalog_location_malformed(&[s], &no_packs())
                .violations
                .len(),
            1
        );
    }

    #[test]
    fn a_well_formed_location_passes() {
        let mut s = source("src", true, None);
        s.locations = vec![crate::parse::CatalogLocation {
            members: None,
            commit: None,
            repo: None,
            symbol: None,
            kind: Some("url".into()),
            value: Some("https://example.org/x".into()),
            description: None,
        }];
        assert!(catalog_location_malformed(&[s], &no_packs()).passed());
    }

    /// A fragment is never sent, so a `url` naming a member with `#` fetches the whole zip
    /// (#1352). The finding names the bytes actually fetched and the field that reads one.
    #[test]
    fn a_url_with_a_fragment_names_what_is_fetched() {
        let zip = "https://example.org/oh2010.sf1.zip";
        let c = catalog_location_malformed(
            &[located(
                "url",
                &format!("{zip}#oh2010.sf1.prd.packinglist.txt"),
            )],
            &no_packs(),
        );
        assert_eq!(c.violations.len(), 1);
        let detail = &c.violations[0].detail;
        assert!(
            detail.contains("`#` fragment")
                && detail.contains(&format!("fetches {zip} whole"))
                && detail.contains("`members:`"),
            "{detail}"
        );
        assert!(catalog_location_malformed(&[located("url", zip)], &no_packs()).passed());
        // Only a `url`: a `#` elsewhere is another kind's business.
        assert!(
            catalog_location_malformed(&[located("address", "Box 4 #12")], &no_packs()).passed()
        );
    }

    /// A member is named beside the zip, and one whose path leaves the archive is refused.
    #[test]
    fn a_member_path_must_stay_inside_its_archive() {
        let mut s = located("url", "https://example.org/oh2010.sf1.zip");
        s.locations[0].members = Some(vec!["oh2010.sf1.prd.packinglist.txt".into()]);
        let c = catalog_location_malformed(std::slice::from_ref(&s), &no_packs());
        assert!(c.passed());
        s.locations[0].members = Some(vec!["../escape.txt".into(), "/abs".into()]);
        let c = catalog_location_malformed(&[s], &no_packs());
        assert_eq!(c.violations.len(), 1);
        let detail = &c.violations[0].detail;
        assert!(
            detail.contains("`..`") && detail.contains("absolute"),
            "{detail}"
        );
    }

    fn located(kind: &str, value: &str) -> Source {
        let mut s = source("src", true, None);
        s.locations = vec![crate::parse::CatalogLocation {
            members: None,
            commit: None,
            repo: None,
            symbol: None,
            kind: Some(kind.into()),
            value: Some(value.into()),
            description: None,
        }];
        s
    }

    fn no_packs() -> crate::sources::resolve::Enabled {
        crate::sources::resolve::Enabled::default()
    }

    fn enabled(packs: &[(&str, crate::sources::Origin, &str)]) -> crate::sources::resolve::Enabled {
        crate::sources::resolve::Enabled::from_packs(
            &packs
                .iter()
                .map(|(name, origin, manifest)| crate::sources::Pack {
                    name: name.to_string(),
                    dir: std::path::PathBuf::from(format!(".yidam/sources/{name}")),
                    origin: *origin,
                    manifest: toml::from_str(manifest).map_err(|e| e.to_string()),
                })
                .collect::<Vec<_>>(),
        )
    }

    const SCHOLARLY_DOI: &str = r#"
[pack]
name = "scholarly"
version = "0.1.0"

[scheme.doi]
pattern = '^10\.\d{4,9}/\S+$'
type = "paper"
resolve = { template = "https://api.crossref.org/works/{id}" }
"#;

    /// With no pack enabled, any scheme with a local id passes (RFC-0048, *Migration &
    /// compatibility*): a corpus that lists none sees no change.
    #[test]
    fn an_identifier_location_is_a_scheme_and_a_local_id() {
        for v in ["doi:10.1167/tvst.8.5.14", "pmc:PMC6753881", "orc:2921.13"] {
            assert!(
                catalog_location_malformed(&[located("identifier", v)], &no_packs()).passed(),
                "{v}"
            );
        }
        for v in [
            "10.1167/tvst.8.5.14",
            ":PMC6753881",
            "doi:",
            "https://doi.org/10.1/x",
        ] {
            assert_eq!(
                catalog_location_malformed(&[located("identifier", v)], &no_packs())
                    .violations
                    .len(),
                1,
                "{v}"
            );
        }
    }

    /// Once a pack is enabled, the packs are the scheme set (RFC-0048 §2, #1328).
    #[test]
    fn an_enabled_pack_holds_the_scheme_and_its_pattern() {
        use crate::sources::Origin;
        let packs = enabled(&[("scholarly", Origin::Authored, SCHOLARLY_DOI)]);
        let said = |v: &str| {
            let c = catalog_location_malformed(&[located("identifier", v)], &packs);
            c.violations
                .into_iter()
                .map(|v| v.detail)
                .collect::<Vec<_>>()
        };
        assert_eq!(said("doi:10.1167/tvst.8.5.14"), Vec::<String>::new());

        // The pattern refuses the local id.
        let found = said("doi:not-a-doi");
        assert_eq!(found.len(), 1);
        assert!(
            found[0].contains("does not match [scheme.doi] pattern"),
            "{found:?}"
        );

        // A scheme the template ships a pack for names that pack.
        let found = said("wayback:20240102030405/https://example.org/");
        assert_eq!(found.len(), 1);
        assert!(
            found[0].contains("no enabled source pack declares scheme `wayback`"),
            "{found:?}"
        );
        assert!(
            found[0].contains("the template's `archive` pack"),
            "{found:?}"
        );

        // One it ships none for says where to write one.
        let found = said("orc:2921.13");
        assert_eq!(found.len(), 1);
        assert!(
            found[0].contains("write one in .yidam/sources"),
            "{found:?}"
        );

        // The shape is still checked first, and once.
        assert_eq!(said("https://doi.org/10.1/x").len(), 1);
    }

    /// A vendored pack that an authored pack of the same name shadows declares nothing.
    #[test]
    fn a_shadowed_vendored_pack_declares_nothing() {
        use crate::sources::Origin;
        let vendored = SCHOLARLY_DOI.replace("[scheme.doi]", "[scheme.pmc]");
        let packs = enabled(&[
            ("scholarly", Origin::Authored, SCHOLARLY_DOI),
            ("scholarly", Origin::Vendored, &vendored),
        ]);
        let c = catalog_location_malformed(&[located("identifier", "pmc:PMC6753881")], &packs);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("scheme `pmc`"));
    }

    /// The two kinds corpora wrote before `identifier` existed are still refused, and the
    /// finding names the migration that fixes them.
    #[test]
    fn a_retired_kind_names_its_migration() {
        let c = catalog_location_malformed(&[located("doi", "10.1167/tvst.8.5.14")], &no_packs());
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("yidam migrate locations"),
            "{}",
            c.violations[0].detail
        );
    }

    fn typed(t: Option<&str>) -> Source {
        let mut s = source("src", true, None);
        s.r#type = t.map(str::to_string);
        s
    }

    #[test]
    fn every_catalog_type_passes_and_an_absent_one_is_not_this_check_s() {
        for t in crate::parse::CATALOG_TYPES {
            assert!(catalog_type_unknown(&[typed(Some(t))]).passed(), "{t}");
        }
        assert!(catalog_type_unknown(&[typed(None)]).passed());
    }

    /// `primary` and the long tail are reported with the four new words offered, and nothing
    /// is chosen for the author (RFC-0048 §2.1).
    #[test]
    fn a_type_outside_the_set_is_offered_the_four_new_words() {
        for t in ["primary", "publication", "thesis"] {
            let c = catalog_type_unknown(&[typed(Some(t))]);
            assert_eq!(c.violations.len(), 1, "{t}");
            let m = &c.violations[0].detail;
            for word in ["statute", "report", "standard", "document"] {
                assert!(m.contains(&format!("`{word}`")), "{t}: {m}");
            }
        }
        let c = catalog_type_unknown(&[typed(Some("primary"))]);
        assert!(c.violations[0].detail.contains("standing"));
    }

    /// A `capacity` with nothing to bound, or naming what no target class declares, is
    /// reported on the class rather than read as one seat everywhere.
    #[test]
    fn a_capacity_the_class_cannot_read_is_reported_on_the_class() {
        let terms = || vec![term("a", "board", "1880", Some("1890"))];
        let unbound = seated(
            "interval:\n  start: began\n  end: ended\n  capacity: seats\n",
            vec![seated_office("board", Some("\"3\""))],
            terms(),
        );
        assert_eq!(unbound.violations.len(), 1, "{:?}", unbound.violations);
        assert_eq!(unbound.violations[0].node, ".yidam/corpus/tenure.ont.yml");
        assert!(unbound.violations[0].detail.contains("no `exclusive_over`"));

        let undeclared = seated(
            &SEATED.replace("capacity: seats", "capacity: chairs"),
            vec![seated_office("board", Some("\"3\""))],
            terms(),
        );
        assert_eq!(
            undeclared.violations.len(),
            1,
            "{:?}",
            undeclared.violations
        );
        assert_eq!(
            undeclared.violations[0].node,
            ".yidam/corpus/tenure.ont.yml"
        );
        assert!(undeclared.violations[0].detail.contains("which `office`"));
    }

    /// A `complete` with no target to mark, or naming what no target class declares, is
    /// reported on the class rather than read as a line nobody finished.
    #[test]
    fn a_complete_the_class_cannot_read_is_reported_on_the_class() {
        let terms = || vec![term("a", "sheriff", "1889", Some("1893"))];
        let office = || vec![office_with("sheriff", &["line_complete: true"])];
        let unbound = gaps(
            "interval:\n  start: began\n  end: ended\n  complete: line_complete\n",
            office(),
            terms(),
        );
        assert_eq!(unbound.violations.len(), 1, "{:?}", unbound.violations);
        assert_eq!(unbound.violations[0].node, ".yidam/corpus/tenure.ont.yml");
        assert!(unbound.violations[0].detail.contains("no `exclusive_over`"));

        let undeclared = gaps(
            &COMPLETE.replace("complete: line_complete", "complete: finished"),
            office(),
            terms(),
        );
        assert_eq!(
            undeclared.violations.len(),
            1,
            "{:?}",
            undeclared.violations
        );
        assert!(undeclared.violations[0].detail.contains("which `office`"));
    }
}
