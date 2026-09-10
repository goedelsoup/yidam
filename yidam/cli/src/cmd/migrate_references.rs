//! `yidam migrate references` — lifting a reference out of an evidence tag into a field.
//!
//! An evidence tag's detail is prose: `[verified — #362]`, `[verified —
//! `crates/project/tests/the_statute_behind_the_weights.rs`]`, `[inference —
//! decisions/building-typology]`. Every one of those names something the repository addresses,
//! and none of them is addressable — a consumer reading the node gets a sentence, and the only
//! way to follow the citation is to read it the way a person does.
//!
//! RFC-0032 gave those things a grammar and #786 gave a node somewhere to write them
//! ([`CorpusInstance::references`](yidam_core::corpus::CorpusInstance)). Shipping the
//! destination without the mover would leave the decision to lift them as the one the corpus
//! already declined: two corpora alone write over a thousand of these details, and nobody is
//! going to convert them by hand.
//!
//! # What it does, measured
//!
//! Every number here is what `yidam migrate references --dry-run` reports, run against the
//! corpora on 2026-09-09 — not a separate scan agreeing with it.
//!
//! | | what happens |
//! |---|---|
//! | the detail **is** the reference | the reference is written to `references:` and the tag collapses to `[verified]` |
//! | the reference sits **inside** prose | the reference is written to `references:`; **the detail is left exactly as written** |
//! | the detail names nothing addressable | nothing is written |
//!
//! Across the seven corpora that write any details: **555 references across 241 nodes, 211 tags
//! collapsed, 614 details left as prose.** By kind: 244 issues, 171 crate items, 51 nodes, 44
//! decisions, 43 catalog entries, 2 skills. Five of the seven yield no references at all and 114
//! prose details — their evidence is attestation and reading, not a file.
//!
//! # Why the prose is not edited
//!
//! The settled shape is *move the reference, leave the clause in the tag*, and in 211 details
//! there is no clause: the detail is the reference and nothing else, so removing it leaves the
//! bare standing the tag vocabulary already defines. Elsewhere the reference is a grammatical
//! part of a sentence — `#42 (agglomeration), #43 (the congestion feedback), and #122 (the
//! network-distance base)` — and excising the three numbers yields `(agglomeration), (the
//! congestion feedback), and (…)`. So the entry is **added** and the sentence is left alone.
//! That is a reference written twice, which is a cost; the alternative is a migration that
//! mangles prose, which is not a cost this can pay. The report counts the two outcomes
//! separately so the split is visible rather than implied.
//!
//! [`crate::cmd::lint::checks`] already recorded the sharper version of this argument, about
//! the case where collapsing would change what is claimed: a narrowing detail —
//! `[verified as proposed]` — must never become a bare `[verified]`, because that asserts the
//! proposal was adopted. **The rule that protects those is the span rule and not the resolver.**
//! It is tempting to say a narrowing detail can never *be* a reference, since its first word is
//! `as`; that is true and it is not what saves `[verified as proposed in #362]`, which names an
//! issue. What saves it is that the issue is *part* of the detail rather than all of it, so
//! `whole` is false and the detail stays. Mutating `whole` to `true` left the first version of
//! that test green, which is how the distinction was found.
//!
//! # Nothing is resolved that does not exist
//!
//! Every reference but an issue number is written only when the thing it names is on disk. That
//! is the whole defence against guessing, and it is what lets the resolver read shapes it would
//! otherwise have to refuse: `Auglaize/Mercer`, `land-use/transport` and `6a/6b` all have a
//! path's shape and are prose using a slash for *or*. They resolve to no file, so they are
//! prose, and no rule about English had to be written to decide it. Every local markdown link
//! inside a measured detail resolves, so a link that does not is not a category this reports.
//!
//! An issue number cannot be checked — the tracker is not on disk — but `#362` has no second
//! reading. An issue named by *full URL* is a different matter, because which tracker the URL
//! names is exactly what the grammar's corpus slot would have to say (#779). Not one of the
//! details measured here does it, so no rule was written for a case that does not arise.
//!
//! Nothing is written that [`reference_conforms`] rejects, either. `reference-not-in-the-grammar`
//! gates the field this writes into (#786), and a migration whose output reddened the check that
//! reads it would be a migration into a failing build — the argument
//! [`crate::cmd::migrate`] already makes about a retype.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use yidam_core::uri::{parse_reference, reference_conforms, render_reference, Kind};

use super::lint::checks::detail_tags;
use super::migrate::MigrateReport;
use super::rename::Edit;

/// One reference lifted out of one tag.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Lift {
    /// The node, repository-relative.
    pub node: String,
    /// 1-based line of the tag it came out of.
    pub line: usize,
    /// The reference, as written into `references:`.
    pub reference: String,
    /// Whether the tag's detail was the reference alone, so the tag collapsed to its bare
    /// standing. False when the reference sits inside a sentence, which is left as written.
    pub detail_cleared: bool,
}

/// What one node's rewrite consists of. The single decision both `plan` and `apply` read.
///
/// Two reads of the same file rather than a recorded span carried between them — the property
/// [`crate::cmd::migrate`]'s `apply` states and the reason it re-locates every edit. Here the
/// spans are byte offsets into a specific string, so carrying them across a second read of the
/// file would be worse than useless; recomputing costs one pass over a few kilobytes.
#[derive(Debug, Default)]
struct NodePlan {
    /// `(start, end, replacement)` per tag to collapse, ascending, `end` inclusive.
    rewrites: Vec<(usize, usize, String)>,
    /// References to write, in first-seen order, none of them already present.
    append: Vec<String>,
    lifts: Vec<Lift>,
    /// Details naming nothing addressable.
    prose: usize,
}

// ── resolving ─────────────────────────────────────────────────────────────────

/// A reference found in a detail, and the span of the detail it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    start: usize,
    end: usize,
    reference: String,
}

/// The tokens a detail can carry that might name something.
///
/// Four shapes, in the order they are tried: an issue number, a markdown link's target, a code
/// span, and a bare slash-joined path. A code span is tried before a bare path because a bare
/// path inside one would otherwise match the tail of it.
fn tokens(detail: &str) -> Vec<(usize, usize, &str)> {
    let b = detail.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        // `#362`
        if b[i] == b'#' {
            let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                out.push((i, i + 1 + digits, &detail[i..i + 1 + digits]));
                i += 1 + digits;
                continue;
            }
        }
        // `[label](target)` — the target, not the label.
        if b[i] == b'[' {
            if let Some(close) = detail[i..].find("](").map(|k| i + k) {
                if let Some(end) = detail[close + 2..].find(')').map(|k| close + 2 + k) {
                    out.push((i, end + 1, &detail[close + 2..end]));
                    i = end + 1;
                    continue;
                }
            }
        }
        // `` `crates/project` ``
        if b[i] == b'`' {
            if let Some(close) = detail[i + 1..].find('`').map(|k| i + 1 + k) {
                out.push((i, close + 1, &detail[i + 1..close]));
                i = close + 1;
                continue;
            }
        }
        // A bare `a/b`, which is the form a corpus writes for `decisions/x` and `class/name`.
        if is_path_byte(b[i]) && (i == 0 || !is_path_byte(b[i - 1])) {
            let len = b[i..].iter().take_while(|c| is_path_byte(**c)).count();
            let word = &detail[i..i + len];
            if word.contains('/') || word.contains("::") {
                out.push((i, i + len, word.trim_end_matches(['.', ',', ';', ':'])));
            }
            i += len.max(1);
            continue;
        }
        i += 1;
    }
    out
}

fn is_path_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'-' | b'_' | b':')
}

/// Every reference a detail names, with the span each occupies.
fn resolve_all(root: &Path, node_dir: &Path, detail: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (start, end, raw) in tokens(detail) {
        let Some(reference) = resolve(root, node_dir, raw) else {
            continue;
        };
        out.push(Found {
            start,
            end,
            reference,
        });
    }
    out
}

/// One token as a reference, or `None` when it names nothing this may write.
///
/// The grammar's own gate, not a second reading of it: a class named `NonConformance` is a name
/// two of the sixteen corpora write and [`reference_conforms`] rejects (#777). The file exists,
/// so existence is not what refuses it — writing it would redden
/// `reference-not-in-the-grammar` on the next lint of the corpus this just migrated.
fn resolve(root: &Path, node_dir: &Path, raw: &str) -> Option<String> {
    let reference = locate_token(root, node_dir, raw)?;
    parse_reference(&reference)
        .filter(reference_conforms)
        .map(|_| reference)
}

/// Which thing a token names, before the grammar is asked whether it may be written.
fn locate_token(root: &Path, node_dir: &Path, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(number) = raw.strip_prefix('#') {
        return number
            .chars()
            .all(|c| c.is_ascii_digit())
            .then(|| format!("issue/{number}"))
            .filter(|_| !number.is_empty());
    }
    // A Rust item path, `dispersion::ohio_panel`. The crate is a directory; the rest is the
    // fragment #786 widened the grammar to hold. `_` for `-` is cargo's own mapping between a
    // crate name and its module name, not a guess about this repository.
    if raw.contains("::") && !raw.contains('/') {
        let (head, rest) = raw.split_once("::")?;
        for name in [head.to_string(), head.replace('_', "-")] {
            if root.join("crates").join(&name).is_dir() {
                return Some(reference_string(
                    Kind::Crate,
                    &name,
                    (!rest.is_empty()).then_some(rest),
                ));
            }
        }
        return None;
    }
    // A path. `#` and `?` are a fragment and a query on a markdown target, not part of it.
    let path = raw.split(['#', '?']).next().unwrap_or(raw).trim();
    if path.is_empty() {
        return None;
    }
    // The bases a corpus writes paths against: its own directory, the repository root, and the
    // `.yidam/` conventions it names without the prefix — `decisions/x`, `catalog/x`,
    // `class/name`, and the bare slug of a catalog entry or a decision.
    let yidam = root.join(".yidam");
    let bases = [
        node_dir.to_path_buf(),
        root.to_path_buf(),
        yidam.clone(),
        yidam.join("corpus"),
        yidam.join("catalog"),
        yidam.join("decisions"),
        yidam.join("skills"),
    ];
    for base in bases {
        for suffix in ["", ".yml", ".md"] {
            let candidate = normalise(&base.join(format!("{path}{suffix}")))?;
            if !candidate.starts_with(root) {
                continue;
            }
            if candidate.is_file() || (suffix.is_empty() && candidate.is_dir()) {
                if let Some(r) = locate(root, &candidate) {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// A path with `.` and `..` resolved textually, so a traversal out of the repository is
/// visible before the filesystem is asked. `None` when it climbs above the root.
fn normalise(path: &Path) -> Option<std::path::PathBuf> {
    let mut out = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Which kind a path that exists is, by where it sits.
fn locate(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    fn stem(s: &str) -> &str {
        s.strip_suffix(".yml")
            .or_else(|| s.strip_suffix(".md"))
            .unwrap_or(s)
    }
    match parts.as_slice() {
        [".yidam", "corpus", class, name] if !name.ends_with(".ont.yml") => Some(reference_string(
            Kind::Node,
            &format!("{class}/{}", stem(name)),
            None,
        )),
        [".yidam", "catalog", name] => Some(reference_string(Kind::Catalog, stem(name), None)),
        [".yidam", "decisions", name] => Some(reference_string(Kind::Decision, stem(name), None)),
        [".yidam", "skills", name] => Some(reference_string(Kind::Skill, stem(name), None)),
        ["crates", name] => Some(reference_string(Kind::Crate, name, None)),
        ["crates", name, rest @ ..] => {
            Some(reference_string(Kind::Crate, name, Some(&rest.join("/"))))
        }
        _ => None,
    }
}

/// A reference, rendered by the grammar's own renderer rather than by formatting a string.
fn reference_string(kind: Kind, path: &str, fragment: Option<&str>) -> String {
    render_reference(&yidam_core::uri::Reference {
        corpus: None,
        kind,
        path: path.to_string(),
        rev: None,
        fragment: fragment.map(str::to_string),
    })
}

// ── the `references:` block ───────────────────────────────────────────────────

/// The `references:` block's extent, as `(key line index, first line index after it)`.
///
/// Not a YAML parse, for the reason `migrate`'s every other rewrite is not one: re-emitting the
/// document would reformat every file it touched. A top-level key sits at column 0 and a block's
/// content does not, so the boundary is unambiguous without one.
fn references_block(lines: &[&str]) -> Option<(usize, usize)> {
    let key = lines.iter().position(|l| *l == "references:")?;
    let end = lines[key + 1..]
        .iter()
        .position(|l| l.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '"'))
        .map(|k| key + 1 + k)
        .unwrap_or(lines.len());
    Some((key, end))
}

/// The references a node already declares. Read so a second run writes nothing.
fn declared(text: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some((key, end)) = references_block(&lines) else {
        return BTreeSet::new();
    };
    lines[key + 1..end]
        .iter()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
        .collect()
}

/// A reference as one YAML sequence item.
///
/// Quoted when it carries a `#`. YAML only starts a comment at a `#` preceded by whitespace, so
/// `crate/project#tests/x.rs` is safe unquoted — but the corpus that writes the most of these
/// carries a warning in its own `.yidam/corpus/README.md` about exactly this character, and a
/// migration is the wrong place to be subtle about it.
fn item(reference: &str) -> String {
    if reference.contains('#') {
        format!("\"{reference}\"")
    } else {
        reference.to_string()
    }
}

/// The node's text with its tags collapsed and its new references written.
fn rewrite(text: &str, plan: &NodePlan) -> String {
    let mut out = text.to_string();
    // Back to front: an earlier offset is still valid after a later span is replaced.
    for (start, end, to) in plan.rewrites.iter().rev() {
        out.replace_range(*start..=*end, to);
    }
    if plan.append.is_empty() {
        return out;
    }
    let items: Vec<String> = plan.append.iter().map(|r| item(r)).collect();
    let lines: Vec<&str> = out.lines().collect();
    match references_block(&lines) {
        Some((key, end)) => {
            // Match the indentation the block already uses, so the diff is the new lines only.
            let indent = lines[key + 1..end]
                .iter()
                .find_map(|l| l.find("- ").map(|k| &l[..k]))
                .unwrap_or("  ")
                .to_string();
            let mut all: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            let added: Vec<String> = items.iter().map(|i| format!("{indent}- {i}")).collect();
            all.splice(end..end, added);
            let mut joined = all.join("\n");
            if out.ends_with('\n') {
                joined.push('\n');
            }
            joined
        }
        None => {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("references:\n");
            for i in &items {
                let _ = writeln!(out, "  - {i}");
            }
            out
        }
    }
}

// ── planning ──────────────────────────────────────────────────────────────────

/// What this migration does to one node. The one decision; `plan` reports it and `apply` writes it.
fn plan_node(root: &Path, node_dir: &Path, file: &str, text: &str) -> NodePlan {
    let mut plan = NodePlan::default();
    let existing = declared(text);
    let mut written: BTreeSet<String> = BTreeSet::new();
    // `mask_fenced` and not `mask_code`: a fenced block is shown rather than said, so a tag
    // inside one is an example and must not be rewritten — but an inline code span is where the
    // citation *is*. Both maskers preserve byte offsets, so the spans index the original text.
    let masked = crate::markdown::mask_fenced(text);
    for tag in detail_tags(&masked) {
        let detail = tag.detail().to_string();
        if detail.is_empty() {
            continue;
        }
        let found = resolve_all(root, node_dir, &detail);
        if found.is_empty() {
            plan.prose += 1;
            continue;
        }
        // The detail *is* the reference: one of them, spanning all of it. Only then does
        // removing it leave the bare standing the tag vocabulary defines.
        let whole = found.len() == 1 && found[0].start == 0 && found[0].end == detail.len();
        for f in &found {
            if existing.contains(&f.reference) || !written.insert(f.reference.clone()) {
                continue;
            }
            plan.append.push(f.reference.clone());
            plan.lifts.push(Lift {
                node: file.to_string(),
                line: tag.line,
                reference: f.reference.clone(),
                detail_cleared: whole,
            });
        }
        if whole {
            plan.rewrites
                .push((tag.start, tag.end, format!("[{}]", tag.standing)));
        }
    }
    plan
}

/// Every reference in every evidence tag in the corpus, and where it would go.
pub(crate) fn plan(root: &Path, corpus: &Path, report: &mut MigrateReport) {
    for (path, file, text) in nodes(root, corpus) {
        let node_dir = path.parent().unwrap_or(root).to_path_buf();
        let node = plan_node(root, &node_dir, &file, &text);
        report.prose_details += node.prose;
        for (start, end, to) in &node.rewrites {
            report.edits.push(Edit {
                file: file.clone(),
                line: line_of(&text, *start),
                from: crate::claims::collapse_whitespace(&text[*start..=*end]),
                to: to.clone(),
            });
        }
        report.lifted.extend(node.lifts);
    }
}

/// Write it. Re-derives the plan, for the reason [`NodePlan`] gives.
pub(crate) fn apply(root: &Path, corpus: &Path, report: &mut MigrateReport) -> anyhow::Result<()> {
    for (path, file, text) in nodes(root, corpus) {
        let node_dir = path.parent().unwrap_or(root).to_path_buf();
        let node = plan_node(root, &node_dir, &file, &text);
        if node.rewrites.is_empty() && node.append.is_empty() {
            continue;
        }
        let out = rewrite(&text, &node);
        // The migration must not produce a state the corpus's own parser cannot read. A node
        // whose YAML this broke would fail `malformed-yaml` on the next lint, and the reader of
        // that finding would have no way to know a migration caused it.
        if serde_yaml::from_str::<serde_yaml::Value>(&out).is_err() {
            report
                .blocked
                .push(format!("{file}: the rewrite would not parse as YAML"));
            continue;
        }
        std::fs::write(&path, out)?;
    }
    Ok(())
}

/// `(path, repository-relative path, text)` for every instance node that reads.
fn nodes(root: &Path, corpus: &Path) -> Vec<(std::path::PathBuf, String, String)> {
    crate::walk::walk_corpus_instances(corpus)
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let file = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            Some((p, file, text))
        })
        .collect()
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// One line, for the report and the commit subject.
///
/// **The collapse count is `edits` and not the `detail_cleared` lifts.** A node citing `#362` in
/// two tags collapses both and writes the reference once, so counting the lifts reported 60
/// collapses where the migration performed 75.
pub(crate) fn summary(report: &MigrateReport) -> String {
    format!(
        "{} reference(s) out of evidence tags — {} tag(s) collapse to a bare standing",
        report.lifted.len(),
        report.edits.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corpus on disk, because every rule here is "does this exist?".
    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            for d in [
                ".yidam/corpus/concept",
                ".yidam/catalog",
                ".yidam/decisions",
                ".yidam/skills",
                "crates/project/tests",
            ] {
                std::fs::create_dir_all(dir.path().join(d)).unwrap();
            }
            for f in [
                ".yidam/corpus/concept/other.yml",
                ".yidam/catalog/a-source.md",
                ".yidam/decisions/building-typology.yml",
                ".yidam/skills/a-skill.md",
                "crates/project/tests/the_statute.rs",
            ] {
                std::fs::write(dir.path().join(f), "x").unwrap();
            }
            Self { dir }
        }
        fn root(&self) -> &Path {
            self.dir.path()
        }
        fn node_dir(&self) -> std::path::PathBuf {
            self.dir.path().join(".yidam/corpus/concept")
        }
        fn resolve(&self, raw: &str) -> Option<String> {
            resolve(self.root(), &self.node_dir(), raw)
        }
        fn plan(&self, text: &str) -> NodePlan {
            plan_node(
                self.root(),
                &self.node_dir(),
                ".yidam/corpus/concept/a.yml",
                text,
            )
        }
    }

    #[test]
    fn every_kind_the_corpora_write_resolves_to_its_own_form() {
        let f = Fixture::new();
        assert_eq!(f.resolve("#362").as_deref(), Some("issue/362"));
        assert_eq!(
            f.resolve("crates/project/tests/the_statute.rs").as_deref(),
            Some("crate/project#tests/the_statute.rs")
        );
        assert_eq!(
            f.resolve("crates/project").as_deref(),
            Some("crate/project")
        );
        assert_eq!(
            f.resolve("../../catalog/a-source.md").as_deref(),
            Some("catalog/a-source")
        );
        assert_eq!(
            f.resolve("decisions/building-typology").as_deref(),
            Some("decision/building-typology")
        );
        assert_eq!(f.resolve("a-skill").as_deref(), Some("skill/a-skill"));
        // `concept/other` and not `node/concept/other`: the bare path *is* the node form, and
        // `render_reference` spells the kind only where a class name would shadow it.
        assert_eq!(f.resolve("other.yml").as_deref(), Some("concept/other"));
        assert_eq!(
            f.resolve("project::a_module").as_deref(),
            Some("crate/project#a_module")
        );
    }

    /// The rule that lets a slash mean *or*.
    #[test]
    fn a_slash_in_prose_resolves_to_nothing_and_needs_no_rule_about_english() {
        let f = Fixture::new();
        for prose in [
            "Auglaize/Mercer",
            "land-use/transport",
            "6a/6b",
            "he-sim/he-platform",
        ] {
            assert_eq!(f.resolve(prose), None, "{prose}");
        }
    }

    #[test]
    fn a_path_that_climbs_out_of_the_repository_resolves_to_nothing() {
        let f = Fixture::new();
        assert_eq!(f.resolve("../../../../../../etc/passwd"), None);
    }

    #[test]
    fn a_detail_that_is_only_the_reference_collapses_the_tag() {
        let f = Fixture::new();
        let plan = f.plan("description: |\n  A claim [verified — #362].\n");
        assert_eq!(plan.append, vec!["issue/362"]);
        assert_eq!(plan.rewrites.len(), 1);
        assert_eq!(plan.rewrites[0].2, "[verified]");
        assert!(plan.lifts[0].detail_cleared);
    }

    /// 57 of the 211 collapsible tags wrap across two lines, which is why the span is bytes.
    #[test]
    fn a_tag_wrapped_across_two_lines_collapses_as_one_span() {
        let f = Fixture::new();
        let text =
            "description: |\n  A claim [verified —\n  `crates/project/tests/the_statute.rs`].\n";
        let plan = f.plan(text);
        assert_eq!(plan.append, vec!["crate/project#tests/the_statute.rs"]);
        assert_eq!(
            rewrite(text, &plan),
            "description: |\n  A claim [verified].\nreferences:\n  - \"crate/project#tests/the_statute.rs\"\n"
        );
    }

    #[test]
    fn a_reference_inside_a_sentence_is_written_and_the_sentence_is_not_touched() {
        let f = Fixture::new();
        let text =
            "description: |\n  Held [verified — #42 (agglomeration) and #43 (the feedback)].\n";
        let plan = f.plan(text);
        assert_eq!(plan.append, vec!["issue/42", "issue/43"]);
        assert!(plan.rewrites.is_empty());
        assert!(plan.lifts.iter().all(|l| !l.detail_cleared));
        assert!(rewrite(text, &plan).starts_with(
            "description: |\n  Held [verified — #42 (agglomeration) and #43 (the feedback)].\n"
        ));
    }

    /// The failure mode `checks.rs` named before this tool existed.
    ///
    /// **Each case here names something**, deliberately. A narrowing detail that resolves to
    /// nothing is protected by the resolver and would pass this test with the collapse rule
    /// deleted — mutating `whole` to `true` left the earlier version of it green. What has to
    /// hold is that the *span* rule refuses these: the reference is part of the detail and not
    /// all of it, so removing the tag's detail would drop the narrowing with it.
    #[test]
    fn a_narrowing_detail_never_collapses_to_a_bare_standing() {
        let f = Fixture::new();
        for text in [
            "description: |\n  It holds [verified as proposed in #362].\n",
            "description: |\n  It holds [verified for FY2025, see `crates/project`].\n",
            "description: |\n  It holds [inference on the mechanism, [verified] in #42].\n",
            "description: |\n  It holds [verified which #362 revised].\n",
        ] {
            let plan = f.plan(text);
            assert!(
                !plan.append.is_empty(),
                "{text:?} resolved nothing, so this proves nothing"
            );
            assert!(plan.rewrites.is_empty(), "{text:?} collapsed");
        }
    }

    /// And a narrowing detail naming nothing is prose, which is the other half of the same rule.
    #[test]
    fn a_narrowing_detail_naming_nothing_is_prose() {
        let f = Fixture::new();
        for text in [
            "description: |\n  It holds [verified as proposed].\n",
            "description: |\n  It holds [verified for FY2025].\n",
        ] {
            let plan = f.plan(text);
            assert!(plan.append.is_empty(), "{text:?}");
            assert!(plan.rewrites.is_empty(), "{text:?}");
            assert_eq!(plan.prose, 1, "{text:?}");
        }
    }

    /// The half that remains when the write is already done.
    #[test]
    fn a_tag_is_collapsed_even_when_its_reference_is_already_declared() {
        let f = Fixture::new();
        let text = "description: |\n  A claim [verified — #362].\nreferences:\n  - issue/362\n";
        let plan = f.plan(text);
        assert!(plan.append.is_empty(), "nothing left to write");
        assert!(plan.lifts.is_empty(), "and so nothing to report as lifted");
        assert_eq!(plan.rewrites.len(), 1, "but the tag still says it in prose");
        assert_eq!(
            rewrite(text, &plan),
            "description: |\n  A claim [verified].\nreferences:\n  - issue/362\n"
        );
    }

    /// Two tags, one reference: both collapse and the reference is written once.
    #[test]
    fn a_reference_cited_twice_is_written_once_and_collapses_both_tags() {
        let f = Fixture::new();
        let text = "description: |\n  One [verified — #362]. Two [inference — #362].\n";
        let plan = f.plan(text);
        assert_eq!(plan.append, vec!["issue/362"]);
        assert_eq!(plan.rewrites.len(), 2);
        assert_eq!(
            rewrite(text, &plan),
            "description: |\n  One [verified]. Two [inference].\nreferences:\n  - issue/362\n"
        );
    }

    #[test]
    fn a_second_run_writes_nothing() {
        let f = Fixture::new();
        let text = "description: |\n  A claim [verified — #362].\n";
        let once = rewrite(text, &f.plan(text));
        assert_ne!(
            once, text,
            "the first run has to do something to be idempotent about"
        );
        let twice = rewrite(&once, &f.plan(&once));
        assert_eq!(once, twice);
        assert!(f.plan(&once).append.is_empty());
        assert!(f.plan(&once).rewrites.is_empty());
    }

    #[test]
    fn an_existing_references_block_gains_the_new_entries_at_its_own_indent() {
        let f = Fixture::new();
        let text = "description: |\n  A claim [verified — #362].\nreferences:\n    - issue/1\nlinks:\n  - target: x\n";
        let plan = f.plan(text);
        assert_eq!(plan.append, vec!["issue/362"]);
        assert_eq!(
            rewrite(text, &plan),
            "description: |\n  A claim [verified].\nreferences:\n    - issue/1\n    - issue/362\nlinks:\n  - target: x\n"
        );
    }

    #[test]
    fn a_tag_inside_a_fenced_block_is_an_example_and_is_left_alone() {
        let f = Fixture::new();
        let text = "description: |\n  ```\n  A claim [verified — #362].\n  ```\n";
        let plan = f.plan(text);
        assert!(plan.append.is_empty());
        assert!(plan.rewrites.is_empty());
    }

    /// The gate on the field this writes into. Nothing goes in that the check would report.
    #[test]
    fn nothing_is_written_that_the_grammar_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".yidam/corpus/NonConformance")).unwrap();
        std::fs::write(root.join(".yidam/corpus/NonConformance/DIS-2024.yml"), "x").unwrap();
        let node_dir = root.join(".yidam/corpus/concept");
        std::fs::create_dir_all(&node_dir).unwrap();
        // The file exists, so existence is not what refuses it.
        assert!(root
            .join(".yidam/corpus/NonConformance/DIS-2024.yml")
            .is_file());
        assert_eq!(
            locate_token(root, &node_dir, "../NonConformance/DIS-2024.yml").as_deref(),
            Some("NonConformance/DIS-2024")
        );
        // …and `resolve` refuses it, because the grammar does.
        assert_eq!(
            resolve(root, &node_dir, "../NonConformance/DIS-2024.yml"),
            None
        );
        let plan = plan_node(
            root,
            &node_dir,
            "a.yml",
            "description: |\n  A claim [verified — ../NonConformance/DIS-2024.yml].\n",
        );
        assert!(plan.append.is_empty(), "{:?}", plan.append);
        assert_eq!(plan.prose, 1);
    }

    #[test]
    fn a_detail_naming_nothing_is_counted_and_not_written() {
        let f = Fixture::new();
        let plan = f.plan(
            "description: |\n  A claim [verified — owner attestation, no document in hand].\n",
        );
        assert!(plan.append.is_empty());
        assert!(plan.rewrites.is_empty());
        assert_eq!(plan.prose, 1);
    }

    #[test]
    fn what_is_written_reads_back_as_the_references_it_lifted() {
        let f = Fixture::new();
        let text = "class: concept\nproperties:\n  name: a\ndescription: |\n  A claim [verified — #362, `crates/project`].\n";
        let out = rewrite(text, &f.plan(text));
        let parsed = yidam_core::corpus::parse_instance(&out);
        assert_eq!(
            parsed.references,
            Some(vec!["issue/362".into(), "crate/project".into()])
        );
        assert_eq!(parsed.class.as_deref(), Some("concept"));
    }
}
