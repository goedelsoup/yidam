//! Two records nothing in the corpus refers to (#1068).
//!
//! `catalog-uncited` asks the question for a catalog entry. These ask it for the other two
//! things a corpus holds beside its nodes: a decision record nothing refers to, and a source
//! the prose uses but the catalog never registered. Both are Info. An uncited record is
//! something to look at, and a person decides whether it was ever meant to be cited.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::checks::{prose_lines, ProseLink};
use super::model::{Check, Severity, Violation};
use crate::corpus::{normalize, DecisionRecord, Node, Source};

// ── decision-uncited ────────────────────────────────────────────────────────────

/// The 1-based lines of each file that a REGEN block's body covers.
///
/// A generator's link is not a person's citation. `yidam decisions-log` writes a row linking
/// every record into `.yidam/decisions/README.md`. Counting those rows would cite every record
/// in a corpus that runs it, and the check would never report anything there.
fn generated_lines(files: &[(String, String)]) -> HashMap<&str, Vec<(usize, usize)>> {
    let line_of = |text: &str, at: usize| text[..at].matches('\n').count() + 1;
    files
        .iter()
        .map(|(rel, text)| {
            let spans = yidam_core::markers::scan_markers(text)
                .regen
                .iter()
                .map(|s| (line_of(text, s.body), line_of(text, s.close)))
                .collect();
            (rel.as_str(), spans)
        })
        .collect()
}

/// Each record a file names by path, as `decisions/<stem>`, outside a REGEN block's body.
///
/// This is how a corpus most often cites a decision. The form is `decisions/<stem>` in a code
/// span in a class description, `.yidam/decisions/<stem>.yml` in a comment in the code that
/// implements it, or a link from a crate's README, which carries the same text. What follows
/// the stem may be `.yml`, `.yaml` or anything that cannot continue a name. A name that only
/// begins with a record's stem does not name it.
fn mentioned(texts: &[(String, String)], name: impl Fn(&str) -> Vec<usize>) -> Vec<(usize, &str)> {
    const AT: &str = "decisions/";
    let mut out = Vec::new();
    for (rel, text) in texts {
        if !text.contains(AT) {
            continue;
        }
        let blocks: Vec<(usize, usize)> = yidam_core::markers::scan_markers(text)
            .regen
            .iter()
            .map(|s| (s.body, s.close))
            .collect();
        for (at, _) in text.match_indices(AT) {
            if blocks.iter().any(|&(a, b)| (a..b).contains(&at)) {
                continue;
            }
            let rest = &text[at + AT.len()..];
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')))
                .unwrap_or(rest.len());
            let word = rest[..end].trim_end_matches('.');
            let stem = word
                .strip_suffix(".yml")
                .or_else(|| word.strip_suffix(".yaml"))
                .unwrap_or(word);
            out.extend(name(stem).into_iter().map(|i| (i, rel.as_str())));
        }
    }
    out
}

/// Decision records nothing refers to.
///
/// Six things count as a reference, and each is a form a corpus already writes:
///
/// - a mention of the record's path in any file this repository authors, from any file but
///   the record itself: see [`mentioned`];
/// - a markdown link in authored prose that resolves to the record, from any file but the
///   record itself;
/// - a node `links:` target that resolves to it;
/// - an edge `source:` naming it by `id:` or file stem, the reading `edge-source-unresolved`
///   accepts;
/// - a node `references:` entry `decision/<id>` in this corpus;
/// - another record's `supersedes:` naming it.
///
/// A link or a mention inside a REGEN block does not count: see [`generated_lines`].
///
/// Code counts. The first version of this check read links from `.yidam/` and `docs/` only,
/// on the reasoning that a record cited from code is out of a corpus reader's sight. Across
/// thirteen derived corpora it reported 61 of 346 records, and a file in the same repository
/// named 32 of those 61 by path. A crate's README citing the decision it implements is the
/// consequence written where it applies, which is what this check asks for.
pub fn decision_uncited(
    decisions: &[DecisionRecord],
    nodes: &[Node],
    links: &[ProseLink],
    texts: &[(String, String)],
) -> Check {
    let by_path: HashMap<PathBuf, usize> = decisions
        .iter()
        .enumerate()
        .map(|(i, d)| (normalize(&d.path), i))
        .collect();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, d) in decisions.iter().enumerate() {
        let stem = d.path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let names: HashSet<String> = d.decision.id.clone().into_iter().chain(stem).collect();
        for name in names {
            by_name.entry(name).or_default().push(i);
        }
    }
    let named = |name: &str| by_name.get(name.trim()).cloned().unwrap_or_default();

    let generated = generated_lines(texts);
    let mut cited = vec![false; decisions.len()];

    for (i, file) in mentioned(texts, named) {
        if decisions[i].rel != file {
            cited[i] = true;
        }
    }

    for l in links {
        let in_block = generated
            .get(l.file.as_str())
            .is_some_and(|spans| spans.iter().any(|&(a, b)| (a..=b).contains(&l.line)));
        if in_block {
            continue;
        }
        if let Some(&i) = by_path.get(&normalize(&l.resolved)) {
            if decisions[i].rel != l.file {
                cited[i] = true;
            }
        }
    }

    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for l in n.inst.links.as_deref().unwrap_or(&[]) {
            if let Some(t) = &l.target {
                if let Some(&i) = by_path.get(&normalize(&dir.join(t))) {
                    cited[i] = true;
                }
            }
            for i in l.source.as_deref().map(named).unwrap_or_default() {
                cited[i] = true;
            }
        }
        for r in n.inst.references.as_deref().unwrap_or(&[]) {
            let Some(r) = yidam_core::uri::parse_reference(r) else {
                continue;
            };
            if r.corpus.is_none() && r.kind == yidam_core::uri::Kind::Decision {
                for i in named(&r.path) {
                    cited[i] = true;
                }
            }
        }
    }

    for (from, d) in decisions.iter().enumerate() {
        for i in d.decision.superseded().into_iter().flat_map(named) {
            if i != from {
                cited[i] = true;
            }
        }
    }

    let violations = decisions
        .iter()
        .zip(&cited)
        .filter(|(_, c)| !**c)
        .map(|(d, _)| {
            Violation::new(
                &d.rel,
                "no file in the repository names this record, rests an edge on it, or \
                 supersedes it",
            )
        })
        .collect();
    Check::new(
        "decision-uncited",
        "Decision record nothing refers to",
        Severity::Info,
        "A decision record says why the corpus is shaped as it is. One that no file, edge \
         or later record refers to cannot be reached by a reader of the corpus: it is found \
         only by listing the directory. It may be a record of the tooling rather than \
         of the knowledge, which is fine, or a decision whose consequence was never written \
         where it applies. Name it, as `decisions/<stem>`, from the node, page or code it \
         shaped. A REGEN block a generator wrote does not count.",
        violations,
    )
}

// ── source-unregistered ─────────────────────────────────────────────────────────

/// Characters that end a URL in prose: whitespace, and what wraps one in markdown or YAML.
fn ends_url(c: char) -> bool {
    c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`' | ')' | ']' | '|')
}

/// Every `http://` or `https://` URL on one line, with trailing sentence punctuation and
/// markdown emphasis removed.
///
/// A host without a dot is not read as a URL. Prose about URLs writes `http://www` to name a
/// scheme and a host form, and one corpus that studies a website does exactly that.
fn urls_in(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find("http").map(|i| from + i) {
        let rest = &line[at..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            from = at + 4;
            continue;
        }
        let end = rest.find(ends_url).unwrap_or(rest.len());
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '*', '_']);
        if url_key(url).0.contains('.') {
            out.push(url);
        }
        from = at + end.max(4);
    }
    out
}

/// A URL reduced to what registration is about: its host and its path.
///
/// The scheme, a `www.`, the query, the fragment and a trailing `/` are dropped. The host is
/// lowercased, since hostnames are case-insensitive.
fn url_key(url: &str) -> (String, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = host.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (host, path.trim_end_matches('/').to_string())
}

/// What the catalog registers: one prefix per `url` or `url_template` location.
struct Registered {
    /// `(host, path, is_template)`. A template's path is its text before the first `{`.
    prefixes: Vec<(String, String, bool)>,
}

impl Registered {
    fn build(sources: &[Source]) -> Self {
        let mut prefixes = Vec::new();
        for s in sources {
            for l in &s.locations {
                let (Some(kind), Some(value)) = (l.kind.as_deref(), l.value.as_deref()) else {
                    continue;
                };
                let template = match kind {
                    "url" => false,
                    "url_template" => true,
                    _ => continue,
                };
                let value = value.split('{').next().unwrap_or(value).trim();
                let (host, path) = url_key(value);
                prefixes.push((host, path, template));
            }
        }
        Self { prefixes }
    }

    /// Whether some location covers `url`: the same host, and a path equal to the location's
    /// or beneath it. Beneath a `url` means at a `/`. Beneath a template means after its
    /// fixed text, since the template's variable may begin mid-segment.
    fn covers(&self, url: &str) -> bool {
        let (host, path) = url_key(url);
        self.prefixes.iter().any(|(h, p, template)| {
            *h == host
                && path
                    .strip_prefix(p.as_str())
                    .is_some_and(|rest| *template || rest.is_empty() || rest.starts_with('/'))
        })
    }
}

/// URLs in prose that no catalog `location:` covers.
///
/// Node files and the bodies of catalog entries are read. Decision records are not: their
/// URLs are mostly issue trackers, and a decision is not evidence. Fenced blocks and inline
/// code are skipped, by [`prose_lines`], the same reading `broken-prose-link` uses.
///
/// **Silent in a corpus that declares no URL location.** Such a corpus records where a source
/// lives some other way, in a body line or not at all. Every URL it cites would be reported,
/// and a check that reports everything tells the reader nothing.
///
/// **A malformed catalog entry's URLs are covered.** Its `location:` did not parse, so the
/// URLs it registers are unknown. Every URL written anywhere in its text stands in for them.
/// `malformed-yaml` already names that entry, and reporting its sources as unregistered would
/// send the reader to register them a second time.
///
/// Known source *names* are not matched. The only list of names is the catalog the source is
/// missing from, and substring matching of titles is what #172 removed.
pub fn source_unregistered(sources: &[Source], nodes: &[Node]) -> Check {
    let mut registered = Registered::build(sources);
    let active = !registered.prefixes.is_empty();
    for s in sources.iter().filter(|s| s.malformed.is_some()) {
        for (_, line) in prose_lines(&s.text) {
            for url in urls_in(&line) {
                let (host, path) = url_key(url);
                registered.prefixes.push((host, path, false));
            }
        }
    }

    let mut violations = Vec::new();
    if active {
        let files = nodes
            .iter()
            .map(|n| (n.rel.as_str(), n.text.as_str(), 0))
            .chain(sources.iter().map(|s| {
                let body = crate::parse::frontmatter_body(&s.text);
                let above = s.text[..s.text.len() - body.len()].matches('\n').count();
                (s.rel.as_str(), body, above)
            }));
        for (rel, text, above) in files {
            let mut seen = HashSet::new();
            for (number, line) in prose_lines(text) {
                for url in urls_in(&line) {
                    if registered.covers(url) || !seen.insert(url.to_string()) {
                        continue;
                    }
                    violations.push(Violation::new(
                        format!("{rel}:{}", number + above),
                        format!(
                            "`{url}` is not covered by any catalog entry's `location:`. \
                             Register the source it points at, or add the URL as a \
                             `location:` on the entry that describes it"
                        ),
                    ));
                }
            }
        }
    }
    Check::new(
        "source-unregistered",
        "URL in prose no catalog entry registers",
        Severity::Info,
        "The catalog is where a corpus says what it read. A URL cited in a node or a catalog \
         entry's body that no `location:` covers is a source the catalog does not know about: \
         nothing records when it was retrieved, and `catalog-expired` cannot age it. A location \
         covers a URL on the same host at its own path or beneath it; scheme, `www.`, query \
         and fragment are ignored. Info, because some URLs are a subject of study rather than \
         a source, and only a person can tell which. A corpus that declares no URL location \
         is not read.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::CatalogLocation;
    use std::path::Path;

    fn decision(stem: &str, yaml: &str) -> DecisionRecord {
        let rel = format!(".yidam/decisions/{stem}.yml");
        DecisionRecord::parse(PathBuf::from(format!("/repo/{rel}")), rel, yaml)
    }

    fn node(name: &str, yaml: &str) -> Node {
        let rel = format!(".yidam/corpus/concept/{name}.yml");
        let n = Node::parse(PathBuf::from(format!("/repo/{rel}")), rel, yaml);
        assert!(n.malformed.is_none(), "fixture: {:?}", n.malformed);
        n
    }

    /// The links `prose_links` finds in one authored file, as the driver would collect them.
    fn links_in(rel: &str, text: &str) -> Vec<ProseLink> {
        let path = Path::new("/repo").join(rel);
        super::super::checks::prose_links(rel, path.parent().unwrap(), text)
    }

    fn uncited(check: &Check) -> Vec<&str> {
        check.violations.iter().map(|v| v.node.as_str()).collect()
    }

    const A: &str = ".yidam/decisions/a.yml";
    const B: &str = ".yidam/decisions/b.yml";

    fn two() -> Vec<DecisionRecord> {
        vec![decision("a", "id: a\n"), decision("b", "id: b\n")]
    }

    #[test]
    fn a_record_nothing_refers_to_is_reported_and_one_referred_to_is_not() {
        let links = links_in(
            "docs/why.md",
            "See [the choice](../.yidam/decisions/a.yml).\n",
        );
        let check = decision_uncited(&two(), &[], &links, &[]);
        assert_eq!(uncited(&check), vec![B]);
        assert_eq!(check.severity, Severity::Info);
    }

    #[test]
    fn a_record_linking_itself_is_not_cited() {
        let links = links_in(A, "rationale: see [this](a.yml)\n");
        let check = decision_uncited(&two(), &[], &links, &[]);
        assert_eq!(uncited(&check), vec![A, B]);
    }

    #[test]
    fn a_record_linking_another_cites_it() {
        let links = links_in(A, "rationale: see [the earlier one](b.yml)\n");
        let check = decision_uncited(&two(), &[], &links, &[]);
        assert_eq!(uncited(&check), vec![A]);
    }

    #[test]
    fn a_link_in_a_generated_block_is_not_a_citation() {
        // The written links sit on the lines either side of the block, so a range one line
        // too wide at either end takes one of them.
        let rel = ".yidam/decisions/README.md";
        let text = "# Decisions\nWritten: [b](b.yml)\n<!-- REGEN: yidam decisions-log -->\n\
                    | [a](a.yml) | x |\n<!-- /REGEN -->\nWritten: [c](c.yml)\n";
        let links = links_in(rel, text);
        assert_eq!(links.len(), 3, "fixture: every link is read as prose");
        let files = vec![(rel.to_string(), text.to_string())];
        let decisions = vec![
            decision("a", "id: a\n"),
            decision("b", "id: b\n"),
            decision("c", "id: c\n"),
        ];
        let check = decision_uncited(&decisions, &[], &links, &files);
        assert_eq!(uncited(&check), vec![A]);
    }

    /// The files a repository authors, as [`decision_uncited`] is handed them.
    fn texts(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(rel, text)| (rel.to_string(), text.to_string()))
            .collect()
    }

    fn three() -> Vec<DecisionRecord> {
        vec![
            decision("a", "id: a\n"),
            decision("b", "id: b\n"),
            decision("c", "id: c\n"),
        ]
    }

    #[test]
    fn a_path_named_in_code_or_a_code_span_cites_the_record() {
        let files = texts(&[
            (
                "crates/x/src/lib.rs",
                "// See .yidam/decisions/a.yml for why.\n",
            ),
            (
                ".yidam/corpus/concept.ont.yml",
                "description: |\n  Not a root cause. See `decisions/b`.\n",
            ),
        ]);
        let check = decision_uncited(&three(), &[], &[], &files);
        assert_eq!(uncited(&check), vec![".yidam/decisions/c.yml"]);
    }

    #[test]
    fn what_follows_a_named_path_is_not_part_of_the_name() {
        let files = texts(&[
            ("AGENTS.md", "Read decisions/a.yaml first.\n"),
            ("README.md", "The record is decisions/b.\n"),
            ("web/README.md", "[why](../.yidam/decisions/c.yml)\n"),
        ]);
        let check = decision_uncited(&three(), &[], &[], &files);
        assert!(uncited(&check).is_empty(), "{:?}", uncited(&check));
    }

    #[test]
    fn a_name_that_only_begins_with_a_records_stem_does_not_name_it() {
        let files = texts(&[(
            "README.md",
            "decisions/ab.yml, decisions/a-b, decisions/a_b and decisions/a2 are other records.\n",
        )]);
        let check = decision_uncited(&two(), &[], &[], &files);
        assert_eq!(uncited(&check), vec![A, B]);
    }

    #[test]
    fn a_record_naming_itself_is_not_cited_and_naming_another_cites_it() {
        let files = texts(&[(
            A,
            "id: a\nrationale: unlike decisions/a, decisions/b held.\n",
        )]);
        let check = decision_uncited(&two(), &[], &[], &files);
        assert_eq!(uncited(&check), vec![A]);
    }

    #[test]
    fn a_path_named_in_a_generated_block_is_not_a_citation() {
        // A marker is a line of its own, so the written mentions sit on the lines either
        // side of the block, and the generated one starts at the body's first byte.
        let text = "decisions/b\n<!-- REGEN: yidam decisions-log -->\n\
                    decisions/a\n<!-- /REGEN -->\ndecisions/c\n";
        let files = texts(&[("README.md", text)]);
        let check = decision_uncited(&three(), &[], &[], &files);
        assert_eq!(uncited(&check), vec![A]);
    }

    #[test]
    fn a_path_at_the_first_byte_of_an_inline_block_is_not_a_citation() {
        let text = "Latest: <!-- REGEN: yidam decisions-log -->decisions/a<!-- /REGEN --> \
                    decisions/b\n";
        let files = texts(&[("README.md", text)]);
        let check = decision_uncited(&two(), &[], &[], &files);
        assert_eq!(uncited(&check), vec![A]);
    }

    #[test]
    fn a_links_target_cites_the_record() {
        let n = node(
            "x",
            "class: concept\nlinks:\n  - target: ../../decisions/b.yml\n    relationship: shaped_by\n",
        );
        let check = decision_uncited(&two(), &[n], &[], &[]);
        assert_eq!(uncited(&check), vec![A]);
    }

    #[test]
    fn an_edge_source_names_the_record_by_id_or_by_stem() {
        let decisions = vec![
            decision("a", "id: the-a-decision\n"),
            decision("b", "id: the-b-decision\n"),
            decision("c", "id: c\n"),
        ];
        let n = node(
            "x",
            "class: concept\nlinks:\n  - target: y.yml\n    relationship: r\n    source: the-a-decision\n  \
             - target: z.yml\n    relationship: r\n    source: ' b '\n",
        );
        let check = decision_uncited(&decisions, &[n], &[], &[]);
        assert_eq!(uncited(&check), vec![".yidam/decisions/c.yml"]);
    }

    #[test]
    fn a_decision_reference_in_this_corpus_cites_the_record_and_a_foreign_one_does_not() {
        let n = node(
            "x",
            "class: concept\nreferences:\n  - decision/a\n  - yidam://other/decision/b\n  - catalog/b\n",
        );
        let check = decision_uncited(&two(), &[n], &[], &[]);
        assert_eq!(uncited(&check), vec![B]);
    }

    #[test]
    fn supersedes_cites_the_record_it_replaces_as_a_string_or_a_list() {
        let decisions = vec![
            decision("a", "id: a\nsupersedes: b\n"),
            decision("b", "id: b\n"),
            decision("c", "id: c\nsupersedes: [d, a]\n"),
            decision("d", "id: d\n"),
        ];
        let check = decision_uncited(&decisions, &[], &[], &[]);
        assert_eq!(uncited(&check), vec![".yidam/decisions/c.yml"]);
    }

    #[test]
    fn a_record_superseding_itself_is_not_cited_by_it() {
        let decisions = vec![decision("a", "id: a\nsupersedes: a\n")];
        let check = decision_uncited(&decisions, &[], &[], &[]);
        assert_eq!(uncited(&check), vec![A]);
    }

    /// The field was untyped so a shape nobody anticipated is not a parse failure.
    #[test]
    fn a_supersedes_of_any_other_shape_names_nothing_and_still_parses() {
        let d = decision("a", "id: a\nsupersedes:\n  record: b\n");
        assert!(d.malformed.is_none(), "{:?}", d.malformed);
        assert!(d.decision.superseded().is_empty());
    }

    // ── source-unregistered ─────────────────────────────────────────────────────

    fn source(slug: &str, locations: &[(&str, &str)], text: &str) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained: true,
            r#type: None,
            used_by: None,
            locations: locations
                .iter()
                .map(|(kind, value)| CatalogLocation {
                    kind: Some(kind.to_string()),
                    value: Some(value.to_string()),
                    description: None,
                })
                .collect(),
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
            text: text.to_string(),
            malformed: None,
        }
    }

    fn census() -> Source {
        source(
            "census",
            &[("url", "https://www.census.gov/data/tables")],
            "",
        )
    }

    fn findings(check: &Check) -> Vec<(&str, String)> {
        check
            .violations
            .iter()
            .map(|v| {
                let url = v.detail.split('`').nth(1).unwrap_or_default().to_string();
                (v.node.as_str(), url)
            })
            .collect()
    }

    fn prose(name: &str, line: &str) -> Node {
        node(name, &format!("class: concept\ndescription: |\n  {line}\n"))
    }

    #[test]
    fn a_url_no_location_covers_is_reported_at_its_line() {
        let n = prose("x", "Per https://example.org/report.pdf, the rate rose.");
        let check = source_unregistered(&[census()], &[n]);
        assert_eq!(
            findings(&check),
            vec![(
                ".yidam/corpus/concept/x.yml:3",
                "https://example.org/report.pdf".into()
            )]
        );
        assert_eq!(check.severity, Severity::Info);
    }

    #[test]
    fn a_location_covers_its_own_path_and_beneath_it_on_any_scheme() {
        for url in [
            "https://www.census.gov/data/tables",
            "http://census.gov/data/tables/",
            "https://CENSUS.gov/data/tables/2020/x.csv?y=1#top",
            // A query or fragment on the location's own path: without them split off, the
            // rest after the path is neither empty nor at a `/`.
            "https://census.gov/data/tables?year=2020",
            "https://census.gov/data/tables#notes",
            "(https://census.gov/data/tables).",
        ] {
            let check = source_unregistered(&[census()], &[prose("x", url)]);
            assert!(check.violations.is_empty(), "{url}: {:?}", findings(&check));
        }
    }

    #[test]
    fn a_sibling_path_or_another_host_is_not_covered() {
        for (url, found) in [
            (
                "https://census.gov/data/tablesets",
                "https://census.gov/data/tablesets",
            ),
            ("https://census.gov/data", "https://census.gov/data"),
            (
                "https://census.gov.evil.org/data/tables",
                "https://census.gov.evil.org/data/tables",
            ),
        ] {
            let check = source_unregistered(&[census()], &[prose("x", url)]);
            assert_eq!(
                findings(&check),
                vec![(".yidam/corpus/concept/x.yml:3", found.into())]
            );
        }
    }

    #[test]
    fn a_location_with_no_path_covers_the_whole_host() {
        let s = source("bls", &[("url", "https://bls.gov/")], "");
        let check = source_unregistered(&[s], &[prose("x", "https://www.bls.gov/cpi/x.htm")]);
        assert!(check.violations.is_empty());
    }

    #[test]
    fn a_template_covers_what_follows_its_fixed_text() {
        let s = source(
            "fred",
            &[("url_template", "https://fred.org/series/CPI{id}.csv")],
            "",
        );
        let hit = source_unregistered(&[s], &[prose("x", "https://fred.org/series/CPIAUCSL.csv")]);
        assert!(hit.violations.is_empty());
        let s = source(
            "fred",
            &[("url_template", "https://fred.org/series/CPI{id}.csv")],
            "",
        );
        let miss = source_unregistered(&[s], &[prose("x", "https://fred.org/graph/CPI")]);
        assert_eq!(miss.violations.len(), 1);
    }

    #[test]
    fn an_address_or_file_location_registers_no_url() {
        let s = source(
            "a",
            &[
                ("url", "https://census.gov/"),
                ("address", "https://example.org/"),
            ],
            "",
        );
        let check = source_unregistered(&[s], &[prose("x", "https://example.org/")]);
        assert_eq!(check.violations.len(), 1);
    }

    #[test]
    fn a_corpus_declaring_no_url_location_is_not_read() {
        let s = source("a", &[("file", "data/a.csv")], "");
        let check = source_unregistered(&[s], &[prose("x", "https://example.org/")]);
        assert!(check.violations.is_empty());
    }

    #[test]
    fn a_url_in_code_is_not_read() {
        let n = node(
            "x",
            "class: concept\ndescription: |\n  Run `curl https://example.org/a`.\n  ```\n  \
             https://example.org/b\n  ```\n  Then https://example.org/c\n",
        );
        let check = source_unregistered(&[census()], &[n]);
        assert_eq!(
            findings(&check),
            vec![(
                ".yidam/corpus/concept/x.yml:7",
                "https://example.org/c".into()
            )]
        );
    }

    #[test]
    fn a_url_twice_in_one_file_is_one_finding_and_in_two_files_is_two() {
        let a = prose(
            "a",
            "https://example.org/r and again https://example.org/r.",
        );
        let b = prose("b", "https://example.org/r");
        let check = source_unregistered(&[census()], &[a, b]);
        let files: Vec<&str> = check.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            files,
            vec![
                ".yidam/corpus/concept/a.yml:3",
                ".yidam/corpus/concept/b.yml:3"
            ]
        );
    }

    #[test]
    fn a_catalog_body_is_read_below_its_frontmatter_and_numbered_from_the_file() {
        let text = "---\ntitle: Census\nlicense: https://creativecommons.org/by/4.0\nlocation:\n  - kind: url\n    value: https://census.gov/data/tables\n\
                    ---\n\n# Census\n\nSee also https://example.org/method.\n";
        let s = source("census", &[("url", "https://census.gov/data/tables")], text);
        let check = source_unregistered(&[s], &[]);
        assert_eq!(
            findings(&check),
            vec![(
                ".yidam/catalog/census.md:11",
                "https://example.org/method".into()
            )]
        );
    }

    #[test]
    fn a_malformed_entry_covers_every_url_it_mentions() {
        let mut broken = source(
            "broken",
            &[],
            "---\ntitle: a: b\n---\nhttps://example.org/tool\n",
        );
        broken.malformed = Some("mapping values are not allowed".into());
        let check = source_unregistered(
            &[census(), broken],
            &[prose(
                "x",
                "https://example.org/tool/v2 and https://other.org/",
            )],
        );
        assert_eq!(
            findings(&check),
            vec![(".yidam/corpus/concept/x.yml:3", "https://other.org/".into())]
        );
    }

    #[test]
    fn urls_are_cut_at_markdown_and_sentence_punctuation() {
        assert_eq!(
            urls_in("[a](https://a.org/x), <https://b.org/y>; **https://c.org/z**. httpd https:// http://www \"https://d.org\""),
            vec!["https://a.org/x", "https://b.org/y", "https://c.org/z", "https://d.org"]
        );
    }
}
