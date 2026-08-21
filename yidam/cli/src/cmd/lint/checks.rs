//! The invariants `yidam lint` enforces.
//!
//! Each function returns a [`Check`] whether or not it found anything — a check that
//! reports nothing when it passes cannot be distinguished from a check that did not run,
//! and the difference matters when someone is deciding whether the gate covers a case.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::parse::{parse_frontmatter, CorpusInstance, CATALOG_LOCATION_KINDS};

use super::model::{Check, Severity, Violation};

/// A corpus instance parsed once, with the paths needed to talk about it.
pub struct Node {
    pub path: PathBuf,
    pub rel: String,
    pub inst: CorpusInstance,
}

/// A class definition parsed once — `<class>.ont.yml`.
pub struct Class {
    pub rel: String,
    /// The `description` field: the text that says what kind of thing an instance is.
    /// Deliberately the only field [`class_asserts_purpose`] reads; see there.
    pub description: String,
}

#[derive(Default, serde::Deserialize)]
struct ClassFields {
    #[serde(default)]
    description: Option<String>,
}

pub fn load_classes(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Class> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            let fields: ClassFields = serde_yaml::from_str(&text).unwrap_or_default();
            Class {
                rel: rel_of(root, p),
                description: fields.description.unwrap_or_default(),
            }
        })
        .collect()
}

/// A catalog entry parsed once.
pub struct Source {
    pub rel: String,
    /// Absolute path on disk. A citation is a link that resolves to *this file*, which is
    /// what replaced the slug this struct used to carry — the slug existed only to be
    /// searched for in a node's bytes, and nothing else ever needed it.
    pub path: PathBuf,
    pub obtained: bool,
    pub used_by: Vec<String>,
    pub locations: Vec<crate::parse::CatalogLocation>,
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub fn load_nodes(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Node> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            Node {
                path: p.clone(),
                rel: rel_of(root, p),
                inst: serde_yaml::from_str(&text).unwrap_or_default(),
            }
        })
        .collect()
}

pub fn load_sources(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Source> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            let fm = parse_frontmatter(&text);
            Source {
                rel: rel_of(root, p),
                path: p.clone(),
                // Absent means obtained. Only an explicit `false` claims otherwise.
                obtained: fm.obtained.unwrap_or(true),
                used_by: fm.used_by.unwrap_or_default(),
                locations: fm.location.unwrap_or_default(),
            }
        })
        .collect()
}

// ── class definitions ─────────────────────────────────────────────────────────

/// Phrasings that assert a reason rather than describe a kind.
///
/// Each is a purpose construction: it says the thing was done *in order to* bring something
/// about, which is a claim about somebody's intent. Kept small on purpose — a longer list
/// buys recall at the cost of the check being switched off.
const PURPOSE_PHRASES: &[&str] = &[
    "designed to",
    "deployed to",
    "intended to",
    "calculated to",
    "engineered to",
    "contrived to",
    "orchestrated to",
    "in order to",
    "so as to",
    "for the purpose of",
    "with the aim of",
    "with the intent",
];

/// Class definitions whose `description` asserts a purpose.
///
/// **Why this check exists where no instance-level check could.** Every other rule in a
/// yidam corpus runs on claim tags, and a tag attaches to a claim somebody makes on a node.
/// A class definition is not that: it is the meaning every instance takes on by being filed
/// under the class — asserted identically, silently, untagged, and for each. So a class
/// defined as "a procedural mechanism *deployed to obtain* an outcome the ordinary path
/// would not yield" makes every one of its instances assert a purpose, and no amount of
/// tagging on the instances can reach it. That definition is real: it survived five
/// resolutions and three separate arguments about its instances in a derived repository,
/// because every safeguard that repository had was pointed at instances.
///
/// **Only `description` is read.** That field says what kind of thing an instance is, and
/// is what an instance silently asserts. A class may perfectly well *discuss* purpose in
/// `analytic_note` — the corrected version of the class above does exactly that, quoting
/// "designed to produce this outcome" in order to forbid it — and reading commentary would
/// flag the discussion of the rule as a violation of it.
///
/// **Warn, and a prompt rather than a proof.** This is a check over wording. It cannot see
/// a purpose asserted in words that are not on the list, and a description can name a
/// purpose someone else attributed without asserting it. Read the finding; do not clear it.
pub fn class_asserts_purpose(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let lowered = c.description.to_lowercase();
            let hit = PURPOSE_PHRASES.iter().find(|p| lowered.contains(**p))?;
            Some(Violation::new(
                &c.rel,
                format!("`description` says \"{hit}\" — a purpose, not a kind"),
            ))
        })
        .collect();
    Check::new(
        "class-asserts-purpose",
        "Class definition asserts a purpose rather than describing a kind",
        Severity::Warn,
        "A claim tag attaches to a claim somebody makes on a node. A class definition is \
         not that — it is the meaning every instance takes on by being filed under the \
         class, asserted identically and untagged for each. So a class whose description \
         states a reason ('deployed to obtain an outcome the ordinary path would not \
         yield') puts that assertion beyond the reach of every other check here, which all \
         run on tags. The case this was written from survived five resolutions and three \
         arguments about its instances. Rewrite the description to say what an instance IS \
         — the observable shape, the admission conditions — and move any characterization \
         of why to a field that attributes it, or to `analytic_note`, which this check does \
         not read.",
        violations,
    )
}

// ── corpus structure ──────────────────────────────────────────────────────────

pub fn missing_class(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.class.is_none())
        .map(|n| Violation::new(&n.rel, "no `class:` field"))
        .collect();
    Check::new(
        "missing-class",
        "Instance declaring no class",
        Severity::Error,
        "An instance's class is what binds it to a schema. Without one there is nothing to \
         validate the node against and nothing to render it as — it is a YAML file in a \
         corpus directory, not a node.",
        violations,
    )
}

pub fn unknown_class(nodes: &[Node], defined: &HashSet<String>) -> Check {
    // With no .ont.yml files at all, every class is "unknown" and the check would report
    // the whole corpus. That is a repository without a schema layer, which is a different
    // problem than a mistyped class name.
    let violations = if defined.is_empty() {
        Vec::new()
    } else {
        nodes
            .iter()
            .filter_map(|n| {
                let class = n.inst.class.as_ref()?;
                (!defined.contains(class)).then(|| {
                    Violation::new(&n.rel, format!("class `{class}` has no {class}.ont.yml"))
                })
            })
            .collect()
    };
    Check::new(
        "unknown-class",
        "Instance of a class that has no schema",
        Severity::Error,
        "Either the class file was never written or the instance misspells it. Both leave \
         the node unvalidatable, and a misspelling additionally splits one class into two \
         that no query will join.",
        violations,
    )
}

pub fn missing_label(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.label.is_none())
        .map(|n| Violation::new(&n.rel, "no `label:` field"))
        .collect();
    Check::new(
        "missing-label",
        "Instance with no human-readable label",
        Severity::Warn,
        "The label is what every index, export, and graph rendering shows. Without it a \
         reader gets a filename, which is an identifier chosen for stability rather than \
         for saying what the thing is.",
        violations,
    )
}

pub fn missing_description(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.description.is_none())
        .map(|n| Violation::new(&n.rel, "no `description:` field"))
        .collect();
    Check::new(
        "missing-description",
        "Instance with no description",
        Severity::Warn,
        "The description is the node's content. An instance with properties and links but \
         nothing said about it records that something exists without recording what is \
         known about it.",
        violations,
    )
}

pub fn orphan_out(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.links.as_deref().unwrap_or(&[]).is_empty())
        .map(|n| Violation::new(&n.rel, "no outgoing links"))
        .collect();
    Check::new(
        "orphan-out",
        "Node connected to nothing",
        Severity::Error,
        "Every node must carry at least one outgoing edge. A node with none is unreachable \
         by traversal from anywhere, which in a graph whose value is its connectivity means \
         it is not in the graph at all.",
        violations,
    )
}

pub fn dangling_edge(nodes: &[Node]) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            match &link.target {
                None => violations.push(Violation::new(&n.rel, "link entry with no `target:`")),
                Some(target) => {
                    if !dir.join(target).exists() {
                        violations.push(Violation::new(
                            &n.rel,
                            format!("target does not exist: {target}"),
                        ));
                    }
                }
            }
        }
    }
    Check::new(
        "dangling-edge",
        "Edge pointing at nothing",
        Severity::Error,
        "A broken edge is worse than a missing one: it asserts a relationship to something \
         that is not there, so a reader following it learns nothing and a traversal counting \
         it overstates the graph's connectivity. Renaming a node severs every edge into it, \
         which is why node filenames are meant to be stable.",
        violations,
    )
}

pub fn orphan_in(nodes: &[Node]) -> Check {
    let mut targeted: HashSet<PathBuf> = HashSet::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            if let Some(t) = &link.target {
                // Normalize so `../class/x.yml` and `class/x.yml` compare equal.
                targeted.insert(normalize(&dir.join(t)));
            }
        }
    }
    let violations = nodes
        .iter()
        .filter(|n| !targeted.contains(&normalize(&n.path)))
        .map(|n| Violation::new(&n.rel, "nothing links to this node"))
        .collect();
    Check::new(
        "orphan-in",
        "Node nothing points to",
        Severity::Info,
        "Reported rather than gated: a node authored this morning legitimately has no \
         inbound edges yet, and the hub nodes of a young corpus often point outward at \
         everything while nothing points back. Worth seeing, not worth blocking on.",
        violations,
    )
}

/// Resolve `.` and `..` without touching the filesystem.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Separators that turn a tag into a tag-plus-something.
///
/// A plain space is not among them, deliberately. `[open questions]` is ordinary prose and
/// reference-style link text; flagging it would put a permanent finding in front of every
/// reader who writes the phrase, and a permanently non-empty report is where a real finding
/// gets lost. What is reported is a tag followed by punctuation that reads as *and here is
/// the rest of it*.
const TAG_SEPARATORS: &[char] = &['—', '–', '-', ':', ';', ',', '|', '/', '='];

/// Bracketed forms that open with an evidence tag and are not one.
///
/// The counter matches exact tokens, so `[verified — Pearl 2009]` matches nothing and is
/// counted as **no claim at all** — the opposite of the mention-counted-as-use failure and
/// just as silent. A reader who writes that has plainly intended a tag, and the count
/// absorbs the authoring error rather than reporting it. Counting it would mean guessing
/// what was meant; saying so does not.
///
/// Warn rather than Error: the node is still readable and the fix is the author's to make.
pub fn claim_tag_malformed(nodes: &[Node], texts: &[String]) -> Check {
    let mut violations = Vec::new();
    for (n, text) in nodes.iter().zip(texts) {
        // Masked, so a node explaining the vocabulary is not reported for naming it — the
        // same reason the counter masks.
        for (line, found) in near_miss_tags(&crate::markdown::mask_code(text)) {
            violations.push(Violation::new(
                format!("{}:{}", n.rel, line),
                format!(
                    "`{found}` opens with an evidence tag and is not one, so it is counted \
                     as no claim at all — write the tag alone and put the citation beside it"
                ),
            ));
        }
    }
    Check::new(
        "claim-tag-malformed",
        "Evidence tag that counts as nothing",
        Severity::Warn,
        "The three tokens are matched exactly, which is what keeps `[open questions](…)` \
         from reading as an open claim. The cost is that a tag with its citation folded \
         inside the brackets matches nothing and is silently counted as untagged — a node \
         that looks tagged to a reader and reads as bare assertion to every counter. This \
         reports the near miss rather than guessing at it.",
        violations,
    )
}

/// `(line, text)` for each bracketed near miss. Links are not near misses.
fn near_miss_tags(text: &str) -> Vec<(usize, String)> {
    let tags = [
        crate::claims::VERIFIED,
        crate::claims::INFERENCE,
        crate::claims::OPEN,
    ];
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            let Some(open) = line[j..].find('[') else {
                break;
            };
            let at = j + open;
            let Some(close_rel) = line[at..].find(']') else {
                break;
            };
            let close = at + close_rel;
            j = close + 1;
            // `[text](target)` and `[text][ref]` are links; their label is not a claim.
            if matches!(line[j..].chars().next(), Some('(') | Some('[')) {
                continue;
            }
            let inner = &line[at + 1..close];
            for tag in tags {
                let word = tag.trim_matches(|c| c == '[' || c == ']');
                if inner == word {
                    break; // the tag itself, exactly as intended
                }
                let Some(rest) = inner.strip_prefix(word) else {
                    continue;
                };
                if rest
                    .trim_start()
                    .starts_with(|c: char| TAG_SEPARATORS.contains(&c))
                {
                    out.push((i + 1, line[at..=close].to_string()));
                    break;
                }
            }
        }
    }
    out
}

// ── catalog ───────────────────────────────────────────────────────────────────

/// Every repository path a node points at: `links:` targets and markdown links alike, each
/// resolved from the node's own directory and normalized.
///
/// **The one answer to "what does this node link to?"** There were three matchers asking it,
/// and two of them asked it by searching the node's bytes for a slug: `lint`'s citation
/// counter and `catalog-audit`'s. Both reported a node that merely *named* a source as one
/// that cites it, which the conventions do not say and a reader chasing the finding does not
/// find. A third copy is where the next divergence goes.
pub fn linked_paths(node_path: &Path, rel: &str, text: &str) -> HashSet<PathBuf> {
    let dir = node_path.parent().unwrap_or(node_path);
    let inst: crate::parse::CorpusInstance = serde_yaml::from_str(text).unwrap_or_default();
    let mut out: HashSet<PathBuf> = inst
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|l| l.target.as_ref())
        .map(|t| normalize(&dir.join(t)))
        .collect();
    out.extend(
        prose_links(rel, dir, text)
            .into_iter()
            .map(|l| normalize(&l.resolved)),
    );
    out
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
/// Both edge forms count: a `links:` entry and a markdown link in the prose. The prose scan
/// is [`prose_links`], the same function `broken-prose-link` uses, so a link shown as an
/// example in code is not read as a citation here either.
pub fn citations(sources: &[Source], nodes: &[Node], texts: &[String]) -> Vec<Vec<String>> {
    // Resolved once per node rather than once per (node, source) pair.
    let linked: Vec<HashSet<PathBuf>> = nodes
        .iter()
        .zip(texts)
        .map(|(n, text)| linked_paths(&n.path, &n.rel, text))
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

pub fn catalog_uncited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| s.obtained && c.is_empty())
        .map(|(s, _)| Violation::new(&s.rel, "no corpus node draws on this source"))
        .collect();
    Check::new(
        "catalog-uncited",
        "Registered source nothing draws from",
        Severity::Info,
        "A catalog entry describes a source, not knowledge, so an uncited one is not an \
         error. It is either a source registered ahead of the extraction that will use it, \
         or a claim resting on evidence that never became a node. The two look identical \
         from here and are worth telling apart — which is what `obtained: false` declares. \
         Entries that have said they are the former are not reported.",
        violations,
    )
}

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
    Check::new(
        "catalog-unobtained-but-cited",
        "Source nobody has fetched, cited anyway",
        Severity::Error,
        "This is the price of the `obtained: false` exemption, which is otherwise the \
         cheapest way to silence `catalog-uncited`. A node drawing on a source the catalog \
         says was never retrieved is one of two things and both are defects: the flag is \
         stale and was not cleared when the source arrived, or the citation rests on \
         something nobody has read.",
        violations,
    )
}

pub fn catalog_used_by_drift(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let mut violations = Vec::new();
    for (s, actual) in sources.iter().zip(cites) {
        if s.used_by.is_empty() {
            continue; // the list is optional; absent is not drift
        }
        let claimed: HashSet<&str> = s.used_by.iter().map(|u| basename(u)).collect();
        let found: HashSet<&str> = actual.iter().map(|a| basename(a)).collect();
        let mut detail = Vec::new();
        let mut missing: Vec<&str> = claimed.difference(&found).copied().collect();
        let mut extra: Vec<&str> = found.difference(&claimed).copied().collect();
        missing.sort_unstable();
        extra.sort_unstable();
        if !missing.is_empty() {
            detail.push(format!("claims {} that do not cite it", missing.join(", ")));
        }
        if !extra.is_empty() {
            detail.push(format!("omits {} that do", extra.join(", ")));
        }
        if !detail.is_empty() {
            violations.push(Violation::new(&s.rel, detail.join("; ")));
        }
    }
    Check::new(
        "catalog-used-by-drift",
        "`used-by` list disagreeing with the citations",
        Severity::Warn,
        "The citations are authoritative — they cannot drift from the corpus, and a \
         hand-maintained list can. Both are kept so the disagreement is visible rather than \
         averaged away. The list is optional; an entry that declares one is asserting it is \
         current.",
        violations,
    )
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn catalog_location_malformed(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, loc) in s.locations.iter().enumerate() {
            let mut problems = Vec::new();
            match loc.kind.as_deref() {
                None => problems.push("no `kind`".to_string()),
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
                    if k == "url_template" && !v.contains('{') {
                        problems
                            .push("kind `url_template` but value has no `{…}` placeholder".into());
                    }
                }
            }
            if loc.value.as_deref().unwrap_or("").trim().is_empty() {
                problems.push("no `value`".to_string());
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
    Check::new(
        "catalog-location-malformed",
        "Catalog location missing or mistyped",
        Severity::Warn,
        "A `location` is a list of typed places. The type decides whether a reader is \
         offered a link, so a value whose shape contradicts its type renders wrongly; and \
         where an entry has several locations, the description is the only thing \
         distinguishing them.",
        violations,
    )
}

// ── presentation ──────────────────────────────────────────────────────────────

/// Cells of a markdown table row, ignoring the leading and trailing pipes.
fn table_cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').collect()
}

fn is_delimiter_row(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| ch == '-' || ch == ':'))
}

/// Ragged markdown tables in any committed prose.
///
/// Applied to catalog entries and the corpus's own READMEs — anywhere a reader meets a
/// table. A table whose rows disagree on width does not render as a table; it renders as
/// a paragraph of pipes.
pub fn malformed_table(files: &[(String, String)]) -> Check {
    let mut violations = Vec::new();
    for (rel, text) in files {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            // A table is a header row followed by a delimiter row.
            if !lines[i].trim_start().starts_with('|') || i + 1 >= lines.len() {
                i += 1;
                continue;
            }
            if !is_delimiter_row(lines[i + 1]) {
                i += 1;
                continue;
            }
            let width = table_cells(lines[i]).len();
            if table_cells(lines[i + 1]).len() != width {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "line {}: header has {width} cells, delimiter has {} — renders as a \
                         paragraph of pipes",
                        i + 1,
                        table_cells(lines[i + 1]).len()
                    ),
                ));
            }
            let mut j = i + 2;
            let mut ragged = Vec::new();
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                if table_cells(lines[j]).len() != width {
                    ragged.push(j + 1);
                }
                j += 1;
            }
            if !ragged.is_empty() {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "row(s) at line {} have a different width than the {width}-cell header",
                        ragged
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            i = j;
        }
    }
    Check::new(
        "malformed-table",
        "Markdown table whose rows disagree on width",
        Severity::Warn,
        "A ragged table does not render as a table. Every reader of that file sees a \
         paragraph of pipes instead of the thing the author was trying to show, and the \
         author usually never looks at the rendered output again.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(rel: &str, yaml: &str) -> Node {
        Node {
            path: PathBuf::from(rel),
            rel: rel.to_string(),
            inst: serde_yaml::from_str(yaml).unwrap(),
        }
    }

    fn source(slug: &str, obtained: bool, used_by: &[&str]) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            used_by: used_by.iter().map(|s| s.to_string()).collect(),
            locations: vec![],
        }
    }

    #[test]
    fn orphan_out_flags_a_node_with_no_links() {
        let nodes = vec![
            node("a.yml", "class: c\nlinks: []\n"),
            node("b.yml", "class: c\nlinks:\n  - target: a.yml\n"),
        ];
        let c = orphan_out(&nodes);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "a.yml");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn unknown_class_is_silent_when_no_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: whatever\n")];
        assert!(unknown_class(&nodes, &HashSet::new()).passed());
    }

    #[test]
    fn unknown_class_fires_once_a_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: typo\n")];
        let defined: HashSet<String> = ["real".to_string()].into_iter().collect();
        assert_eq!(unknown_class(&nodes, &defined).violations.len(), 1);
    }

    #[test]
    fn orphan_in_sees_through_relative_traversal() {
        // `../reach/a.yml` from reach/ must match `reach/a.yml`, or every cross-class
        // edge would look like it points somewhere else.
        let a = Node {
            path: PathBuf::from("corpus/reach/a.yml"),
            rel: "corpus/reach/a.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks: []\n").unwrap(),
        };
        let b = Node {
            path: PathBuf::from("corpus/other/b.yml"),
            rel: "corpus/other/b.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks:\n  - target: ../reach/a.yml\n").unwrap(),
        };
        let c = orphan_in(&[a, b]);
        let flagged: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            flagged,
            vec!["corpus/other/b.yml"],
            "only b is unpointed-at"
        );
    }

    /// The reported shape: a tag with its citation folded inside the brackets.
    #[test]
    fn a_tag_with_its_citation_inside_is_a_near_miss() {
        let found = near_miss_tags("The estimate is [verified — Pearl 2009].");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], (1, "[verified — Pearl 2009]".to_string()));
        // And it is counted as nothing, which is why it is worth reporting at all.
        assert_eq!(
            crate::claims::count_in_source("The estimate is [verified — Pearl 2009].").total(),
            0
        );
    }

    #[test]
    fn several_separators_all_read_as_a_folded_citation() {
        for inner in [
            "[open: pending review]",
            "[inference - weak]",
            "[verified, 2026]",
            "[open — nobody has looked]",
            "[verified/partial]",
        ] {
            assert_eq!(
                near_miss_tags(inner).len(),
                1,
                "{inner} should read as a near miss"
            );
        }
    }

    /// The forms that must stay silent. A warn nobody can act on is worse than no warn.
    #[test]
    fn the_tag_itself_and_ordinary_prose_are_not_reported() {
        for quiet in [
            "[verified]",
            "[open]",
            "[inference]",
            // A plain space is not a separator: this is prose, and link text.
            "See [open questions] below.",
            "See [open questions](../q/index.md) for more.",
            "A reference [open questions][q] link.",
            "[opening the valve] is not a tag",
            "[verification] is a different word",
            "nothing bracketed at all",
        ] {
            assert!(
                near_miss_tags(quiet).is_empty(),
                "{quiet} must not be reported"
            );
        }
    }

    /// A node that names the malformed shape in order to warn about it is not committing it.
    #[test]
    fn a_masked_mention_is_not_a_near_miss() {
        let node = Node {
            path: PathBuf::from("/tmp/x.yml"),
            rel: "x.yml".to_string(),
            inst: Default::default(),
        };
        let text = "description: Never write `[verified — source]`; the counter reads it as \
                    untagged.\n";
        let c = claim_tag_malformed(std::slice::from_ref(&node), &[text.to_string()]);
        assert_eq!(c.violations.len(), 0, "{:?}", c.violations);
    }

    /// The violation names the line, because the point is to go and fix that line.
    #[test]
    fn a_near_miss_is_reported_against_its_line() {
        let node = Node {
            path: PathBuf::from("/tmp/x.yml"),
            rel: ".yidam/corpus/c/x.yml".to_string(),
            inst: Default::default(),
        };
        let text = "class: c\nlabel: X\ndescription: |\n  Settled [verified — Pearl 2009].\n";
        let c = claim_tag_malformed(std::slice::from_ref(&node), &[text.to_string()]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].node.ends_with(":4"),
            "{}",
            c.violations[0].node
        );
    }

    /// A source at a known path, and a node in a directory that can reach it.
    fn catalog_source(slug: &str, obtained: bool) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            used_by: vec![],
            locations: vec![],
        }
    }

    fn corpus_node(name: &str, yaml: &str) -> (Node, String) {
        (
            Node {
                path: PathBuf::from(format!("/repo/.yidam/corpus/concept/{name}.yml")),
                rel: format!(".yidam/corpus/concept/{name}.yml"),
                inst: serde_yaml::from_str(yaml).unwrap_or_default(),
            },
            yaml.to_string(),
        )
    }

    /// The reported case. A catalog entry whose slug collides with a connector crate — the
    /// conventions recommend naming connectors after what they fetch — and a node that
    /// mentions the crate in prose and links nothing. It failed a build at Error severity on
    /// a node that cites nothing.
    #[test]
    fn naming_a_slug_in_prose_is_not_a_citation() {
        let sources = vec![catalog_source("nwis", false)];
        let (node, text) = corpus_node(
            "gauge-ingest",
            "class: concept\nlabel: Gauge ingest\ndescription: The `nwis` crate fetches the \
             series; nwis is also the source name.\nlinks:\n  - target: ../concept/other.yml\n",
        );
        let cites = citations(&sources, &[node], &[text]);
        assert_eq!(cites, vec![Vec::<String>::new()], "no link resolves to it");
        assert!(catalog_unobtained_but_cited(&sources, &cites).passed());
    }

    /// And a real citation still is one. Without this the fix is indistinguishable from
    /// deleting the check.
    #[test]
    fn a_markdown_link_that_resolves_to_the_entry_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let (node, text) = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on \
             [Pearl 2009](../../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node], &[text]);
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
        let (node, text) = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../../catalog/pearl-2009.md\n    relationship: cites\n",
        );
        let cites = citations(&sources, &[node], &[text]);
        assert_eq!(cites[0].len(), 1);
        assert!(catalog_uncited(&sources, &cites).passed());
    }

    /// A link shown as an example is not a citation, for the same reason it is not a link.
    #[test]
    fn a_citation_shown_in_code_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let (node, text) = corpus_node(
            "conventions",
            "class: concept\nlabel: How to cite\ndescription: Write \
             `[Pearl 2009](../../catalog/pearl-2009.md)` rather than a full \
             citation.\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node], &[text]);
        assert_eq!(cites, vec![Vec::<String>::new()]);
    }

    /// `..` in a target must not make two spellings of one file look like two files.
    #[test]
    fn a_citation_is_matched_through_a_normalized_path() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let (node, text) = corpus_node(
            "confounding",
            "class: concept\nlabel: C\ndescription: See \
             [P](../../corpus/../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        assert_eq!(citations(&sources, &[node], &[text])[0].len(), 1);
    }

    #[test]
    fn an_unobtained_source_is_not_reported_as_uncited() {
        let s = vec![source("not-yet-fetched", false, &[])];
        assert!(catalog_uncited(&s, &[vec![]]).passed());
    }

    #[test]
    fn an_obtained_source_nothing_cites_is_reported() {
        let s = vec![source("fetched", true, &[])];
        assert_eq!(catalog_uncited(&s, &[vec![]]).violations.len(), 1);
    }

    #[test]
    fn citing_an_unfetched_source_is_an_error() {
        let s = vec![source("not-yet-fetched", false, &[])];
        let c = catalog_unobtained_but_cited(&s, &[vec!["corpus/x/y.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn used_by_drift_reports_both_directions() {
        let s = vec![source("src", true, &["../corpus/a/one.yml"])];
        let c = catalog_used_by_drift(&s, &[vec!["corpus/a/two.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("one.yml"), "should name the stale claim: {d}");
        assert!(d.contains("two.yml"), "should name the omitted citer: {d}");
    }

    #[test]
    fn an_absent_used_by_list_is_not_drift() {
        let s = vec![source("src", true, &[])];
        assert!(catalog_used_by_drift(&s, &[vec!["corpus/a/x.yml".to_string()]]).passed());
    }

    #[test]
    fn a_url_location_must_be_a_url() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("see the reading room".into()),
            description: None,
        }];
        assert_eq!(catalog_location_malformed(&[s]).violations.len(), 1);
    }

    #[test]
    fn a_well_formed_location_passes() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("https://example.org/x".into()),
            description: None,
        }];
        assert!(catalog_location_malformed(&[s]).passed());
    }

    #[test]
    fn a_well_formed_table_passes() {
        let text = "intro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn a_ragged_body_row_is_flagged_with_its_line_number() {
        let text = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 |\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains('4'),
            "should name line 4: {}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_header_and_delimiter_of_different_widths_are_flagged() {
        let text = "| A | B | C |\n|---|---|\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert!(c.violations[0].detail.contains("paragraph of pipes"));
    }

    #[test]
    fn a_line_of_pipes_that_is_not_a_table_is_ignored() {
        // No delimiter row, so this is prose that happens to contain pipes.
        let text = "the grammar is `a | b | c` in this notation\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    // ── class-asserts-purpose ─────────────────────────────────────────────────

    fn class(rel: &str, description: &str) -> Class {
        Class {
            rel: rel.into(),
            description: description.into(),
        }
    }

    #[test]
    fn the_definition_that_motivated_the_check_is_caught() {
        // Verbatim from a derived repository's genesis. Every instance of this class
        // asserted a purpose by existing, and no instance-level tag could reach it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A procedural mechanism deployed to obtain an outcome the ordinary path \
             would not yield.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("deployed to"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn the_documentary_rewrite_of_that_definition_passes() {
        // The same class after it was redefined: an observable shape and its admission
        // conditions, with no claim about anyone's reason.
        assert!(class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A dated procedural sequence that departed from a baseline the record fixed \
             before the sequence and independently of it. The departure is the whole of \
             the class's content, and it is a comparison rather than a motive.",
        )])
        .passed());
    }

    #[test]
    fn commentary_outside_description_is_not_read() {
        // `analytic_note` is where a class discusses the purpose rule — often by quoting
        // the forbidden phrasing in order to forbid it. Reading it would flag the
        // discussion of the rule as a violation of the rule.
        let text = "class: maneuver\ndescription: A dated procedural sequence.\n\
                    analytic_note: |\n  \"designed to produce this outcome\" is an \
                    allegation, not a record.\n";
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("maneuver.ont.yml");
        std::fs::write(&p, text).unwrap();
        let loaded = load_classes(dir.path(), &[p], &crate::cmd::lint::Overlay::default());
        assert_eq!(loaded[0].description.trim(), "A dated procedural sequence.");
        assert!(class_asserts_purpose(&loaded).passed());
    }

    #[test]
    fn the_match_is_case_insensitive() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument Designed To pass.")]);
        assert_eq!(c.violations.len(), 1);
    }

    #[test]
    fn a_class_with_no_description_passes() {
        assert!(class_asserts_purpose(&[class("x.ont.yml", "")]).passed());
    }

    #[test]
    fn the_check_warns_rather_than_gates() {
        // A heuristic over wording. Gating on it would make every false positive a
        // blocked commit, and the check would be switched off within a week.
        assert_eq!(class_asserts_purpose(&[]).severity, Severity::Warn);
    }
}

// ── Resolution annotations ───────────────────────────────────────────────────
//
// A resolution record is dated, and its `What remains open` describes the world on that
// date. The world moves, so PROTOCOL allows a dated annotation to be appended beneath an
// item that has moved — never an edit to the original.
//
// The risk that convention carries is not that an annotation is wrong. It is what an
// annotation structurally *is*: a place where one elector, in one commit, having read no
// `ma/*` tip and transported nothing, could perform a resolution in a file the protocol
// never routes through one. Article V puts synthesis in resolution events, so an
// annotation may record movement and never outcome.

/// One `> **Moved …**` annotation found in a resolution record.
pub struct Annotation {
    pub file: String,
    pub line: usize,
    /// The `## ` heading it sits under, or empty if it precedes the first one.
    pub section: String,
    pub text: String,
}

/// Find every annotation in a resolution record, with the section it sits under.
pub fn annotations_in(file: &str, text: &str) -> Vec<Annotation> {
    let mut section = String::new();
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("## ") {
            section = rest.trim().to_string();
            continue;
        }
        // The annotation form PROTOCOL prescribes. Matched on the block-quote marker plus
        // the bold `Moved` opener, so ordinary prose mentioning the word is not a finding.
        if line.starts_with("> **Moved") {
            out.push(Annotation {
                file: file.to_string(),
                line: i + 1,
                section: section.clone(),
                text: line.to_string(),
            });
        }
    }
    out
}

/// `> **Moved YYYY-MM-DD.**` — an ISO date immediately after the word.
fn carries_iso_date(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("> **Moved") else {
        return false;
    };
    let rest = rest.trim_start();
    let date: String = rest.chars().take(10).collect();
    if date.len() != 10 {
        return false;
    }
    let b = date.as_bytes();
    b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&i| b[i].is_ascii_digit())
}

pub fn resolution_annotation_malformed(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let node = format!("{}:{}", a.file, a.line);
            if !carries_iso_date(&a.text) {
                return Some(Violation::new(
                    node,
                    "no ISO date after `Moved` — an annotation carries the date of the \
                     movement, not of the resolution",
                ));
            }
            // Annotating anything but the open items edits the record rather than
            // extending it.
            if a.section != "What remains open" {
                return Some(Violation::new(
                    node,
                    format!(
                        "sits under `{}` — annotate `What remains open` and nowhere else",
                        if a.section.is_empty() {
                            "(no section)"
                        } else {
                            &a.section
                        }
                    ),
                ));
            }
            None
        })
        .collect();
    Check::new(
        "resolution-annotation-malformed",
        "Resolution annotation is malformed or in the wrong section",
        Severity::Error,
        "Both halves are structural rather than a judgement about wording, which is why \
         this gates where its sibling does not. An annotation with no date cannot be read \
         against the record it extends — the whole point is that the record speaks for its \
         own date and the annotation for a later one. An annotation under `What was \
         resolved` or `What changed` edits history instead of extending it, and those \
         sections are the ones a reader trusts to describe a settled past.",
        violations,
    )
}

/// Words that announce a decision rather than report a movement.
const DECIDING_WORDS: &[&str] = &[
    "resolved",
    "settled",
    "decided",
    "concluded",
    "agreed",
    "closes",
    "closed",
    "adopted",
    "rejected",
    "supersedes",
    "superseded",
];

pub fn resolution_annotation_decides(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let lower = a.text.to_lowercase();
            let hit = DECIDING_WORDS.iter().find(|w| {
                // Word-boundary-ish: avoid matching inside a longer word.
                lower
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|t| t == **w)
            })?;
            Some(Violation::new(
                format!("{}:{}", a.file, a.line),
                format!(
                    "`{hit}` announces an outcome — an annotation records movement only; \
                     an open item is closed by a later resolution"
                ),
            ))
        })
        .collect();
    Check::new(
        "resolution-annotation-decides",
        "Resolution annotation announces an outcome",
        Severity::Warn,
        "An annotation may say a question was proposed, retrieved, measured or superseded \
         in fact; it may not say it was settled. Closing an open item is synthesis, and \
         Article V puts synthesis in resolution events — so an annotation that decides \
         something is a resolution performed by one elector, in one commit, in a file the \
         protocol never routes through a resolution. Warn rather than Error because this \
         is a heuristic over wording: it cannot see an outcome announced in words that are \
         not on the list, and gating on it would make every false positive a blocked \
         commit and the check would be switched off within a week.",
        violations,
    )
}

#[cfg(test)]
mod annotation_tests {
    use super::*;

    const WELL_FORMED: &str = "\
# local-infrastructure

## What was resolved

The project and site classes are adopted.

## What remains open

Whether `body` splits into fields.

> **Moved 2026-08-18.** A budget proposal now exists on `ma/goedelsoup`, measured over
> six commits. One branch holds it, so it is a proposal rather than a tension.

Whether block-quoted host error text should be indexed.
";

    fn annos(text: &str) -> Vec<Annotation> {
        annotations_in("resolutions/x.md", text)
    }

    #[test]
    fn a_well_formed_annotation_passes_both_checks() {
        let a = annos(WELL_FORMED);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].section, "What remains open");
        assert!(resolution_annotation_malformed(&a).passed());
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// The date is what lets a reader tell the record's own date from the movement's.
    #[test]
    fn an_annotation_without_a_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved.** Somebody proposed it.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("ISO date"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_non_iso_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved 18/08/2026.** Proposed.\n");
        assert_eq!(resolution_annotation_malformed(&a).violations.len(), 1);
    }

    /// Annotating a settled section edits history rather than extending it.
    #[test]
    fn an_annotation_under_what_was_resolved_is_an_error() {
        let a = annos("## What was resolved\n\n> **Moved 2026-08-18.** It was reopened.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("What was resolved"),
            "{:?}",
            c.violations[0].detail
        );
    }

    /// The one that matters: an annotation performing a resolution.
    #[test]
    fn an_annotation_announcing_an_outcome_is_warned() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-18.** This is now settled: the \
             bodies stay indexed.\n",
        );
        let c = resolution_annotation_decides(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("settled"),
            "{:?}",
            c.violations[0].detail
        );
        // Structurally fine — it is the wording that offends.
        assert!(resolution_annotation_malformed(&a).passed());
    }

    /// Reporting a movement in fact is exactly what the convention is for, even when the
    /// movement happens to be a supersession that occurred elsewhere.
    #[test]
    fn reporting_a_movement_is_not_announcing_an_outcome() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-19.** Measured at 714 bytes per \
             document and recorded on `ma/auditor`.\n",
        );
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// A deciding word inside a longer word is not a hit.
    #[test]
    fn the_word_list_respects_boundaries() {
        let a = annos("## What remains open\n\n> **Moved 2026-08-19.** Unresolvedness aside.\n");
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// Prose that merely mentions the word is not an annotation.
    #[test]
    fn only_the_prescribed_form_is_read_as_an_annotation() {
        let a = annos("## What remains open\n\nNothing has moved since this was written.\n");
        assert!(a.is_empty());
    }

    /// A repository with no sangha has no annotations and no findings.
    #[test]
    fn no_annotations_is_clean() {
        assert!(resolution_annotation_malformed(&[]).passed());
        assert!(resolution_annotation_decides(&[]).passed());
    }
}

// ── Prose links ──────────────────────────────────────────────────────────────
//
// `graph-check` validates the corpus's own `links:` edges. Nothing read the markdown
// links in the prose beside them, and they break silently: a node cites a resolution, a
// resolution cites a position, a convention cites a guideline, and any of those can be
// moved or removed by a commit that never touches the citing file.
//
// In a derived repository the gap had eleven live instances — four citations to position
// files that had never left their author's branch, and seven naming files that are not
// there, including two pointing one directory short at the code behind a published figure.

/// One markdown link found in prose.
pub struct ProseLink {
    pub file: String,
    pub line: usize,
    /// The target exactly as written, for the message.
    pub target: String,
    /// Where it resolves from the directory of the file it sits in — which is what a
    /// reader's markdown client does, and the whole reason a repo-root-relative link in
    /// a nested file is broken.
    pub resolved: PathBuf,
}

/// Whether a link target points outside the filesystem and is therefore not ours to check.
fn is_external(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        // Root-relative. In a repository these are ambiguous — repo root or site root —
        // and resolving them either way produces findings nobody can act on.
        || target.starts_with('/')
}

/// Strip a markdown title and any angle-bracket wrapper: `<a b> "title"` → `a b`.
fn link_path(raw: &str) -> String {
    let t = raw.trim();
    let t = match (t.strip_prefix('<'), t.find('>')) {
        (Some(_), Some(end)) => &t[1..end],
        _ => t.split_whitespace().next().unwrap_or(t),
    };
    // Drop an anchor: the file is what exists, the heading is not ours to verify.
    t.split('#').next().unwrap_or(t).trim().to_string()
}

/// Every checkable markdown link in one file.
///
/// `dir` is the directory the file sits in — links resolve against it, not against the
/// repository root.
///
/// **Fenced code blocks are skipped.** A path inside a fence is an example, a shell
/// command, or a directory tree drawing; these documents are full of them and every one
/// would be a false positive. Inline code spans are skipped for the same reason.
pub fn prose_links(file: &str, dir: &Path, text: &str) -> Vec<ProseLink> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        // Fence open/close. The marker is repeated to allow ``` inside a ~~~ block.
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        // Blank out inline code spans so a `[a](b)` shown as code is not read as a link.
        let line = crate::markdown::mask_code_spans(raw);
        let bytes = line.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            let Some(open) = line[j..].find("](") else {
                break;
            };
            let at = j + open;
            j = at + 2;
            let Some(close_rel) = line[j..].find(')') else {
                break;
            };
            let target_raw = &line[j..j + close_rel];
            j += close_rel + 1;
            let target = link_path(target_raw);
            if is_external(&target) {
                continue;
            }
            out.push(ProseLink {
                file: file.to_string(),
                line: i + 1,
                target: target.clone(),
                resolved: dir.join(&target),
            });
        }
    }
    out
}

pub fn broken_prose_link(links: &[ProseLink]) -> Check {
    let violations = links
        .iter()
        .filter(|l| !l.resolved.exists())
        .map(|l| {
            Violation::new(
                format!("{}:{}", l.file, l.line),
                format!("`{}` does not resolve from this file's directory", l.target),
            )
        })
        .collect();
    Check::new(
        "broken-prose-link",
        "A markdown link in prose does not resolve",
        Severity::Error,
        "A link resolves against the directory of the file it sits in — that is what a \
         reader's client does — so a path spelled from the repository root is broken in \
         every file that is not at the root. Error severity, which puts it under the \
         baseline ratchet: a link listed there and since repaired fails the build exactly \
         as a new break does, because a list permitted to be wrong drifts and one that \
         over-lists silently re-permits whatever it over-lists. Fenced and inline code are \
         not read, so an example path in a shell command is not a finding.",
        violations,
    )
}

/// A broken prose link inside a region the repository declared it did not author, paired
/// with the declaration that explains it.
pub struct UnauthoredLink<'a> {
    pub region: &'a crate::authorship::Region,
    pub link: ProseLink,
}

pub fn unauthored_prose_link(links: &[UnauthoredLink<'_>]) -> Check {
    let violations = links
        .iter()
        .filter(|u| !u.link.resolved.exists())
        .map(|u| {
            Violation::new(
                format!("{}:{}", u.link.file, u.link.line),
                format!(
                    "`{}` does not resolve — {}",
                    u.link.target,
                    u.region.explain()
                ),
            )
        })
        .collect();
    Check::new(
        "unauthored-prose-link",
        "A broken prose link in material this repository did not author",
        Severity::Info,
        "Error severity is the right verdict for a link somebody here can go and fix. It is \
         the wrong one for generated output, whose defect belongs to the generator and whose \
         file is rewritten by the next build, and for an unmodified import, where editing the \
         file to satisfy a linter falsifies the record it exists to keep. A consumer that met \
         43 broken links under `docs/` could act on 28: fifteen were inside a directory whose \
         own README says it is a frozen copy of an upstream project at a fork point. It \
         baselined them, which records `we accept this violation` when the truth is `this \
         file is not ours` — and a baselined entry that is later repaired fails the build, so \
         the gate came to depend on nobody ever re-syncing the import. Declared in \
         `.yidam/authorship.yml`. Reported rather than silenced, with the reason and whoever \
         can act on it, because these are real defects addressed to somebody else. Info \
         severity: never gates, never baselined.",
        violations,
    )
}

pub fn authorship_region_stale(stale: &[&crate::authorship::Region]) -> Check {
    let violations = stale
        .iter()
        .map(|r| {
            Violation::new(
                crate::authorship::MANIFEST,
                format!(
                    "`{}` is declared `{}` but matches nothing on disk",
                    r.path,
                    r.kind.as_str()
                ),
            )
        })
        .collect();
    Check::new(
        "authorship-region-stale",
        "A declared authorship region matches nothing on disk",
        Severity::Warn,
        "A manifest permitted to be wrong drifts, exactly as a lint baseline does, and the \
         entry that outlives the directory it describes is the one that quietly excuses a \
         path somebody later creates under the same name. `generated` regions are exempt: \
         they are written by a build and are frequently git-ignored, so absence on a fresh \
         clone carries no information. Warn rather than Error, unlike the baseline's own \
         staleness rule, and for the reason this check exists — an imported region is \
         re-synced by somebody upstream on their schedule, and gating on it would make the \
         build's colour depend on that.",
        violations,
    )
}

#[cfg(test)]
mod prose_link_tests {
    use super::*;

    fn scan(dir: &Path, text: &str) -> Vec<ProseLink> {
        prose_links("docs/x.md", dir, text)
    }

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("there.md"), "x").unwrap();
        std::fs::write(tmp.path().join("sub/deep.md"), "x").unwrap();
        tmp
    }

    /// The defect this exists for: a path spelled from the repository root, sitting in a
    /// file that is not at the root.
    #[test]
    fn a_root_relative_link_in_a_nested_file_is_broken() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](docs/there.md).");
        assert_eq!(links.len(), 1);
        let c = broken_prose_link(&links);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("docs/there.md"));
    }

    #[test]
    fn a_link_relative_to_its_own_file_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](there.md) and [deep](sub/deep.md).");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    /// Every document here is full of example paths in shell blocks and tree drawings.
    /// Reading them would make the check unusable on its first run.
    #[test]
    fn fenced_code_is_not_read() {
        let tmp = fixture();
        let text = "\
Before.

```
git checkout ma/x -- [a](does/not/exist.md)
```

After [it](there.md).
";
        let links = scan(tmp.path(), text);
        assert_eq!(
            links.len(),
            1,
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn tilde_fences_and_nested_backticks_are_handled() {
        let tmp = fixture();
        let text = "~~~\n[a](nope.md)\n```\n[b](nope2.md)\n~~~\n[c](there.md)\n";
        let links = scan(tmp.path(), text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn inline_code_is_not_read() {
        let tmp = fixture();
        let links = scan(tmp.path(), "Write `[label](path/to/thing.md)` in the file.");
        assert!(
            links.is_empty(),
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
    }

    #[test]
    fn external_and_anchor_targets_are_skipped() {
        let tmp = fixture();
        let text = "[a](https://example.com/x) [b](#section) [c](mailto:x@y.z) [d](/abs/path)";
        assert!(scan(tmp.path(), text).is_empty());
    }

    /// The file is what exists; the heading is not ours to verify.
    #[test]
    fn an_anchor_is_stripped_before_resolving() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md#a-heading)");
        assert_eq!(links[0].target, "there.md");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn titles_and_angle_brackets_are_stripped() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md \"Title\") and [b](<there.md>)");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn two_links_on_one_line_are_both_found() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md) then [b](gone.md)");
        assert_eq!(links.len(), 2);
        assert_eq!(broken_prose_link(&links).violations.len(), 1);
    }

    #[test]
    fn a_link_to_a_directory_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[the subdir](sub/)");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn no_links_is_clean() {
        assert!(broken_prose_link(&[]).passed());
    }
}
