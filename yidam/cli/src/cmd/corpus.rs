use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::lint::checks::{load_classes, load_nodes};
use crate::cmd::lint::Overlay;
use crate::paths::{repo_root, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::{line_count, walk_corpus_instances, walk_ont_files};

/// A path as a markdown link target: `/`-separated on every platform.
///
/// `Path::display` emits the host separator. This table is committed and then compared
/// byte-for-byte by CI, and a generator whose output depends on the machine it ran on
/// cannot be committed in a form CI reproduces.
fn slash_path(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Render the corpus index table.
///
/// `link_prefix` is prepended to each row's corpus-relative path. It exists because a
/// markdown link resolves against the directory of the file it sits in, not against the
/// repository root, and this table is written to two destinations at different depths:
/// the README inside the corpus directory (prefix `""`), and `index/corpus.md` in the
/// export bundle, whose corpus sits one directory over (prefix `"../corpus/"`).
///
/// It used to strip the repository root and hand the same string to both. In a derived
/// repository every row of the committed index resolved to `.yidam/corpus/.yidam/corpus/…`
/// — 84 dead links, and that repo's own link checker had to carry an exemption for them.
/// [`render_open_questions`] below *does* strip the root and is correct, because the file
/// it writes is the root README. One renderer serving two destinations is what let this sit.
pub(crate) fn render_corpus_index(link_prefix: &str, corpus: &Path) -> String {
    let instances = walk_corpus_instances(corpus);
    if instances.is_empty() {
        return "_No corpus instances yet._".to_string();
    }
    // `Claims` is verified / inference / open. It tells a reader how much of a node is
    // measured against how much is supposed, without opening the file.
    let mut rows = vec![
        "| Instance | Class | Label | Links out | Claims | Lines |".to_string(),
        "|---|---|---|---|---|---|".to_string(),
    ];
    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let claims = crate::claims::count_in_source(&text).cell();
        let inst = crate::parse::parse_instance(&text);
        let class = inst.class.unwrap_or_else(|| "—".to_string());
        let label = inst.label.unwrap_or_else(|| "—".to_string());
        let links = inst.links.unwrap_or_default().len();
        let lines = line_count(path);
        let rel = path.strip_prefix(corpus).unwrap_or(path);
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        rows.push(format!(
            "| [{filename}]({link_prefix}{}) | {class} | {label} | {links} | {claims} | {lines} |",
            slash_path(rel)
        ));
    }
    rows.join("\n")
}

/// Render the open-questions list.
///
/// Root-relative, and correct: [`open_questions`] writes it into the root README, so the
/// directory a link resolves against *is* the root. The asymmetry with
/// [`render_corpus_index`] is the point — each renderer is relative to where its output
/// lands, and neither may assume the other's depth.
pub(crate) fn render_open_questions(root: &Path, corpus: &Path) -> String {
    let instances = walk_corpus_instances(corpus);
    let fields = crate::claims::ClaimFields::load(corpus);
    let mut items = Vec::new();
    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let inst = crate::parse::parse_instance(&text);
        let label = inst.label.clone().unwrap_or_default();
        let class = inst.class.clone().unwrap_or_default();
        if crate::claims::is_open_question(&label, &text, fields.for_class(&class)) {
            let rel = path.strip_prefix(root).unwrap_or(path);
            items.push(format!("- [{label}]({})", slash_path(rel)));
        }
    }
    if items.is_empty() {
        "_No open questions._".to_string()
    } else {
        items.join("\n")
    }
}

/// Returns (report_text, issue_count). issue_count > 0 means the graph has problems.
/// One node's integrity findings.
#[derive(Debug, serde::Serialize)]
pub struct NodeIssues {
    /// Repo-relative path.
    pub node: String,
    pub issues: Vec<String>,
}

/// What `yidam graph-check` found.
///
/// The data is gathered once and the prose is rendered *from* it — see
/// [`render_graph_check`]. Computing the text separately would let the two answers drift,
/// and a gate whose JSON and prose disagree is worse than either alone.
#[derive(Debug, serde::Serialize)]
pub struct GraphCheckReport {
    /// Whether the gate passed. No issues, and it is a gate: `graph_check` exits nonzero.
    pub passed: bool,
    /// True when the corpus directory holds neither instances nor class definitions —
    /// a fresh repository, which is not a failure.
    pub corpus_empty: bool,
    pub total_instances: usize,
    pub clean_instances: usize,
    pub classes_defined: usize,
    pub nodes_with_issues: Vec<NodeIssues>,
    /// Class files whose bytes did not parse, and why — #721.
    ///
    /// Its own list rather than a row in [`Self::nodes_with_issues`], because that list is
    /// about instances and `clean_instances` is counted against it. A class file in there
    /// would be filed under a heading that says "instances" and would make the arithmetic
    /// wrong at the same time.
    ///
    /// **Reported here as well as by `lint`, deliberately.** The alternative — leave it to
    /// `malformed-yaml` and say nothing — is what left this gate green over a corpus with an
    /// unreadable class file, and `graph-check` is the one job a derived repository's CI runs
    /// unconditionally. Two gates disagreeing about whether a file is readable is the defect;
    /// both answering the same way is the fix.
    pub classes_with_issues: Vec<NodeIssues>,
    /// Classes with a schema and no instances. Reported, never gated.
    pub classes_without_instances: Vec<String>,
}

pub(crate) fn graph_check_data(root: &Path, corpus: &Path) -> GraphCheckReport {
    let instances = walk_corpus_instances(corpus);
    let ont_files = walk_ont_files(corpus);

    if instances.is_empty() && ont_files.is_empty() {
        return GraphCheckReport {
            passed: true,
            corpus_empty: true,
            total_instances: 0,
            clean_instances: 0,
            classes_defined: 0,
            nodes_with_issues: vec![],
            classes_with_issues: vec![],
            classes_without_instances: vec![],
        };
    }

    let defined_classes: std::collections::HashSet<String> = ont_files
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(|s| s.to_string())
        })
        .collect();

    // Class files, read for one question only: did they parse? Their *contents* are not
    // consulted — `defined_classes` above is derived from filenames, and deliberately stays
    // that way, so a typo inside `gage.ont.yml` does not make every instance of `gage`
    // report an unknown class. That would be the same contradiction one layer up.
    let mut classes_with_issues: Vec<NodeIssues> = Vec::new();
    for class in load_classes(root, &ont_files, &Overlay::default()) {
        if let Some(why) = &class.malformed {
            classes_with_issues.push(NodeIssues {
                node: slash_path(Path::new(&class.rel)),
                issues: vec![format!("unreadable: {why}")],
            });
        }
    }

    let mut nodes_with_issues: Vec<NodeIssues> = Vec::new();

    // Through the lint model's loader rather than this command's own read-and-deserialize.
    // The two were the same three lines, and this copy was the one that still ended in
    // `unwrap_or_default()` after #676 fixed the other: a file nobody could read arrived
    // here as an *empty record*, and the checks below then reported `missing 'class:'`
    // about a file whose first line is a `class:` field (#721). `Node::parse` records the
    // parse outcome where the parse happens, so going through it gets the answer for free.
    for node in load_nodes(root, &instances, &Overlay::default()) {
        let path = &node.path;
        let inst = &node.inst;
        let mut node_issues = Vec::new();

        // Everything below reads fields off a record. When the bytes did not parse, that
        // record is empty and describes nothing, so this is the whole finding for the file
        // — the same suppression `lint` applies, for the same reason: adding six findings
        // derived from the emptiness buries the one that can be acted on.
        if let Some(why) = &node.malformed {
            nodes_with_issues.push(NodeIssues {
                node: slash_path(path.strip_prefix(root).unwrap_or(path)),
                issues: vec![format!("unreadable: {why}")],
            });
            continue;
        }

        match &inst.class {
            None => node_issues.push("missing 'class:' field".to_string()),
            Some(class) if !defined_classes.is_empty() && !defined_classes.contains(class) => {
                node_issues.push(format!(
                    "unknown class '{class}': no matching {class}.ont.yml"
                ));
            }
            _ => {}
        }
        if inst.label.is_none() {
            node_issues.push("missing 'label:' field".to_string());
        }

        // Borrowed rather than moved: `inst` belongs to the loaded node now.
        let links = inst.links.as_deref().unwrap_or_default();
        if links.is_empty() {
            node_issues.push("orphan node: no outgoing links".to_string());
        } else {
            let dir = path.parent().unwrap_or(path);
            for link in links {
                match &link.target {
                    None => node_issues.push("link entry missing 'target:' field".to_string()),
                    Some(target) => {
                        let resolved = dir.join(target);
                        if !resolved.exists() {
                            node_issues.push(format!("broken link: {target}"));
                        }
                    }
                }
            }
        }

        if !node_issues.is_empty() {
            let rel = path.strip_prefix(root).unwrap_or(path);
            nodes_with_issues.push(NodeIssues {
                node: slash_path(rel),
                issues: node_issues,
            });
        }
    }

    let classes_with_instances: std::collections::HashSet<String> = instances
        .iter()
        .filter_map(|p| {
            p.parent()
                .and_then(|d| d.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .collect();
    let mut classes_without_instances: Vec<String> = defined_classes
        .iter()
        .filter(|c| !classes_with_instances.contains(*c))
        .cloned()
        .collect();
    classes_without_instances.sort();

    let total = instances.len();
    let issue_count = nodes_with_issues.len();
    GraphCheckReport {
        // An unreadable class file gates too. It is not an instance, so it is not in
        // `clean_instances` — but a corpus whose schema cannot be read is not a graph
        // anyone has checked, and returning `passed: true` over one is the false negative
        // this fix exists to remove.
        passed: issue_count == 0 && classes_with_issues.is_empty(),
        corpus_empty: false,
        total_instances: total,
        clean_instances: total - issue_count,
        classes_defined: defined_classes.len(),
        nodes_with_issues,
        classes_with_issues,
        classes_without_instances,
    }
}

/// Render [`graph_check_data`] as the prose this command has always printed.
///
/// Byte-identical to the pre-contract output, which the report goldens pin.
pub(crate) fn render_graph_check(root: &Path, corpus: &Path) -> (String, usize) {
    let r = graph_check_data(root, corpus);
    (
        render_graph_check_text(&r, corpus),
        r.nodes_with_issues.len(),
    )
}

pub(crate) fn render_graph_check_text(r: &GraphCheckReport, corpus: &Path) -> String {
    if r.corpus_empty {
        return format!("No corpus content found in {}.", corpus.display());
    }

    let mut out = String::new();
    if r.nodes_with_issues.is_empty() {
        let _ = write!(
            out,
            "Checked {} instances across {} classes — all clean.",
            r.total_instances, r.classes_defined
        );
    } else {
        let _ = writeln!(
            out,
            "Checked {} instances across {} classes — {} clean, {} with issues:",
            r.total_instances,
            r.classes_defined,
            r.clean_instances,
            r.nodes_with_issues.len()
        );
        for n in &r.nodes_with_issues {
            let _ = write!(out, "\n  {}", n.node);
            for issue in &n.issues {
                let _ = write!(out, "\n    - {issue}");
            }
        }
    }

    // Before the advisory line below, because this one gates and that one does not.
    if !r.classes_with_issues.is_empty() {
        let _ = write!(
            out,
            "\n\n{} class file(s) could not be read:",
            r.classes_with_issues.len()
        );
        for c in &r.classes_with_issues {
            let _ = write!(out, "\n\n  {}", c.node);
            for issue in &c.issues {
                let _ = write!(out, "\n    - {issue}");
            }
        }
    }

    if !r.classes_without_instances.is_empty() {
        let _ = write!(
            out,
            "\n\nClasses with schema but no instances: {}",
            r.classes_without_instances.join(", ")
        );
    }

    out
}

/// One row of the corpus index.
#[derive(Debug, serde::Serialize)]
pub struct IndexRow {
    pub node: String,
    pub class: String,
    pub label: String,
    pub links_out: usize,
    pub claims_verified: usize,
    pub claims_inference: usize,
    pub claims_open: usize,
    pub lines: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct CorpusIndexReport {
    /// Repository-relative corpus root, so a consumer building a path from a row does not
    /// have to hardcode `.yidam/corpus`. Rows are relative to *this*.
    pub corpus_dir: String,
    pub nodes: Vec<IndexRow>,
}

pub(crate) fn corpus_index_data(root: &Path, corpus: &Path) -> CorpusIndexReport {
    let fields = crate::claims::ClaimFields::load(corpus);
    let nodes = walk_corpus_instances(corpus)
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let inst = crate::parse::parse_instance(&text);
            let claims = crate::claims::count_in_node(
                &text,
                fields.for_class(inst.class.as_deref().unwrap_or_default()),
            );
            let rel = path.strip_prefix(corpus).unwrap_or(path);
            IndexRow {
                node: slash_path(rel),
                class: inst.class.unwrap_or_else(|| "—".to_string()),
                label: inst.label.unwrap_or_else(|| "—".to_string()),
                links_out: inst.links.unwrap_or_default().len(),
                claims_verified: claims.verified,
                claims_inference: claims.inference,
                claims_open: claims.open,
                lines: line_count(path),
            }
        })
        .collect();
    CorpusIndexReport {
        corpus_dir: slash_path(corpus.strip_prefix(root).unwrap_or(corpus)),
        nodes,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct OpenQuestion {
    pub node: String,
    pub label: String,
}

#[derive(Debug, serde::Serialize)]
pub struct OpenQuestionsReport {
    pub open_questions: Vec<OpenQuestion>,
}

pub(crate) fn open_questions_data(root: &Path, corpus: &Path) -> OpenQuestionsReport {
    let fields = crate::claims::ClaimFields::load(corpus);
    let open_questions = walk_corpus_instances(corpus)
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let inst = crate::parse::parse_instance(&text);
            let label = inst.label.clone().unwrap_or_default();
            let class = inst.class.clone().unwrap_or_default();
            if !crate::claims::is_open_question(&label, &text, fields.for_class(&class)) {
                return None;
            }
            let rel = path.strip_prefix(root).unwrap_or(path);
            Some(OpenQuestion {
                node: slash_path(rel),
                label,
            })
        })
        .collect();
    OpenQuestionsReport { open_questions }
}

pub fn corpus_index(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    if format.is_json() {
        return crate::report::emit(&root, corpus_index_data(&root, &corpus));
    }
    // Prefix "": the README this writes to sits in `corpus/`, so a row's path relative
    // to `corpus/` is already the link a reader's client resolves.
    let content = render_corpus_index("", &corpus);
    crate::regen::emit(&content);
    update_file_regen(&corpus.join("README.md"), "yidam corpus-index", &content)
}

pub fn open_questions(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    if format.is_json() {
        return crate::report::emit(&root, open_questions_data(&root, &corpus));
    }
    let content = render_open_questions(&root, &corpus);
    crate::regen::emit(&content);
    update_file_regen(&root.join("README.md"), "yidam open-questions", &content)
}

pub fn graph_check(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    // Before anything is counted: a gate that cannot see the repository must say so rather
    // than report the nothing it found as a clean bill of health. See `require_yidam_repo`.
    crate::paths::require_yidam_repo(&root)?;
    let corpus = yidam_corpus_dir(&root);
    let data = graph_check_data(&root, &corpus);
    let issue_count = data.nodes_with_issues.len();
    let unreadable_classes = data.classes_with_issues.len();

    if format.is_json() {
        crate::report::emit(&root, data)?;
    } else {
        println!("{}", render_graph_check_text(&data, &corpus));
    }

    // The gate, shared: the verdict cannot depend on the rendering. Both counts are named
    // in the message rather than summed, because they are findings about different things
    // and a single number would leave a reader guessing which.
    match (issue_count, unreadable_classes) {
        (0, 0) => Ok(()),
        (n, 0) => anyhow::bail!("{n} instance(s) have issues"),
        (0, c) => anyhow::bail!("{c} class file(s) could not be read"),
        (n, c) => {
            anyhow::bail!("{n} instance(s) have issues and {c} class file(s) could not be read")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── #721: a file that does not parse ────────────────────────────────────────

    /// A corpus of two linked instances and one class, with control over the bytes of one
    /// instance and of the class file.
    ///
    /// The pair is the measurement: an arm hands over sound bytes or the same bytes with
    /// one unclosed quote, and **nothing else differs**. A fixture per outcome would prove
    /// only that two different corpora produce two different reports.
    fn repo_with(instance: &str, schema: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        let class = corpus.join("gage");
        std::fs::create_dir_all(&class).unwrap();
        std::fs::write(corpus.join("gage.ont.yml"), schema).unwrap();
        std::fs::write(class.join("canyon-outlet.yml"), instance).unwrap();
        std::fs::write(
            class.join("valley-bridge.yml"),
            "class: gage\nlabel: valley-bridge\ndescription: A gage.\nlinks:\n  \
             - target: canyon-outlet.yml\n    relationship: refines\n",
        )
        .unwrap();
        tmp
    }

    const SOUND_INSTANCE: &str = "class: gage\nlabel: canyon-outlet\ndescription: A gage.\nlinks:\n  - target: valley-bridge.yml\n    relationship: refines\n";
    /// The same instance with one unclosed quote in `label:`. Nothing else differs.
    const BROKEN_INSTANCE: &str = "class: gage\nlabel: \"canyon-outlet\ndescription: A gage.\nlinks:\n  - target: valley-bridge.yml\n    relationship: refines\n";
    const SOUND_SCHEMA: &str = "class: gage\nlabel: Gage\n";
    /// The same class file with one unclosed quote in `label:`.
    const BROKEN_SCHEMA: &str = "class: gage\nlabel: \"Gage\n";

    fn check(root: &Path) -> GraphCheckReport {
        graph_check_data(root, &root.join(".yidam/corpus"))
    }

    /// **The report contradicted the file.** An instance nobody could read arrived here as
    /// an empty record, and the checks below then described the emptiness: `missing
    /// 'class:'` about a file whose first line is a `class:` field, plus `missing 'label:'`
    /// and `orphan node: no outgoing links` about a file that has both.
    ///
    /// Asserted as *suppression* rather than as "one issue is present", because a fix that
    /// added a fourth issue beside the three wrong ones would satisfy the weaker form while
    /// leaving the reader everything they had before.
    #[test]
    fn an_unreadable_instance_is_reported_once_and_not_described_from_its_emptiness() {
        let sound = repo_with(SOUND_INSTANCE, SOUND_SCHEMA);
        assert!(check(sound.path()).passed, "the sound arm must be clean");

        let broken = repo_with(BROKEN_INSTANCE, SOUND_SCHEMA);
        let r = check(broken.path());
        assert!(!r.passed);
        assert_eq!(r.nodes_with_issues.len(), 1, "{:?}", r.nodes_with_issues);
        let n = &r.nodes_with_issues[0];
        assert!(n.node.ends_with("canyon-outlet.yml"), "{}", n.node);
        assert_eq!(n.issues.len(), 1, "one finding, not four: {:?}", n.issues);
        assert!(n.issues[0].starts_with("unreadable: "), "{}", n.issues[0]);
        // The three the file itself refutes.
        let joined = n.issues.join(" ");
        for wrong in ["missing 'class:'", "missing 'label:'", "orphan node"] {
            assert!(!joined.contains(wrong), "{wrong} still reported: {joined}");
        }
    }

    /// A class file nobody can read gates, and does not take its instances down with it.
    ///
    /// `defined_classes` is derived from filenames, so the class is still *defined* and its
    /// instances still resolve. That is deliberate: making the class vanish would report
    /// `unknown class 'gage'` against every instance of it — the same contradiction one
    /// layer up, and the shape #676 found in `lint`.
    #[test]
    fn an_unreadable_class_file_gates_without_orphaning_its_instances() {
        let broken = repo_with(SOUND_INSTANCE, BROKEN_SCHEMA);
        let r = check(broken.path());
        assert!(
            !r.passed,
            "a schema nothing can read is not a checked graph"
        );
        assert_eq!(
            r.classes_with_issues.len(),
            1,
            "{:?}",
            r.classes_with_issues
        );
        assert!(r.classes_with_issues[0].node.ends_with("gage.ont.yml"));
        assert!(r.classes_with_issues[0].issues[0].starts_with("unreadable: "));
        assert!(
            r.nodes_with_issues.is_empty(),
            "the instances are readable and resolve: {:?}",
            r.nodes_with_issues
        );
    }

    /// **The sharper assertion from #721**: `lint` and `graph-check` on the identical
    /// corpus must not disagree about whether a file is readable.
    ///
    /// Two commands describing the same file two different ways is worse than either answer
    /// on its own, and it is the state this repository was in — `lint` reporting one
    /// `malformed-yaml` finding while `graph-check` reported three findings that the file
    /// refutes. Written against both reports rather than against either one's wording, so
    /// it stays true if either changes how it phrases the finding.
    #[test]
    fn lint_and_graph_check_agree_about_which_files_are_readable() {
        for (instance, schema) in [
            (SOUND_INSTANCE, SOUND_SCHEMA),
            (BROKEN_INSTANCE, SOUND_SCHEMA),
            (SOUND_INSTANCE, BROKEN_SCHEMA),
            (BROKEN_INSTANCE, BROKEN_SCHEMA),
        ] {
            let tmp = repo_with(instance, schema);
            let root = tmp.path();

            let lint_says: std::collections::BTreeSet<String> =
                crate::cmd::lint::run_checks(root, &crate::cmd::lint::Options::default())
                    .into_iter()
                    .find(|c| c.id == "malformed-yaml")
                    .expect("malformed-yaml reports even when it passes")
                    .violations
                    .iter()
                    .map(|v| v.node.replace('\\', "/"))
                    .collect();

            let r = check(root);
            let graph_says: std::collections::BTreeSet<String> = r
                .nodes_with_issues
                .iter()
                .chain(r.classes_with_issues.iter())
                .filter(|n| n.issues.iter().any(|i| i.starts_with("unreadable: ")))
                .map(|n| n.node.clone())
                .collect();

            assert_eq!(
                lint_says, graph_says,
                "lint and graph-check disagree about which files are readable"
            );
        }
    }

    fn node(dir: &Path, class: &str, name: &str) {
        let d = dir.join(class);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(format!("{name}.yml")),
            format!("class: {class}\nlabel: {name}\nlinks:\n  - target: other.yml\n"),
        )
        .unwrap();
    }

    /// Every link in a generated table is resolved against the directory the table is
    /// written into — which is what a reader's markdown client does.
    ///
    /// The defect this pins passed a prefix check: the rows *contained* the right path,
    /// spelled from the wrong place. Assert by resolving, never by string-matching.
    fn assert_links_resolve(rendered: &str, from_dir: &Path) {
        let mut checked = 0;
        for line in rendered.lines() {
            let Some(open) = line.find("](") else {
                continue;
            };
            let rest = &line[open + 2..];
            let Some(close) = rest.find(')') else {
                continue;
            };
            let target = from_dir.join(&rest[..close]);
            assert!(
                target.exists(),
                "link {:?} does not resolve from {:?} (tried {:?})",
                &rest[..close],
                from_dir,
                target
            );
            checked += 1;
        }
        assert!(checked > 0, "no links found in:\n{rendered}");
    }

    #[test]
    fn index_links_resolve_from_the_corpus_readme() {
        let tmp = tempfile::tempdir().unwrap();
        let corpus = tmp.path().join(".yidam").join("corpus");
        node(&corpus, "person", "alpha");
        node(&corpus, "event", "beta");

        // The README lives in `corpus/`, so that is the directory its links resolve from.
        assert_links_resolve(&render_corpus_index("", &corpus), &corpus);
    }

    /// The bundle lays the same table out at `index/corpus.md` with the nodes at
    /// `corpus/<class>/<file>`, so its links are one directory up and over.
    #[test]
    fn index_links_resolve_from_the_bundle_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let corpus = tmp.path().join(".yidam").join("corpus");
        node(&corpus, "person", "alpha");

        let bundle = tmp.path().join("bundle");
        std::fs::create_dir_all(bundle.join("index")).unwrap();
        std::fs::create_dir_all(bundle.join("corpus").join("person")).unwrap();
        std::fs::write(bundle.join("corpus").join("person").join("alpha.yml"), "x").unwrap();

        assert_links_resolve(
            &render_corpus_index("../corpus/", &corpus),
            &bundle.join("index"),
        );
    }

    /// The root README's list is root-relative, and that is correct rather than
    /// inconsistent: it is written to the root.
    #[test]
    fn open_question_links_resolve_from_the_root_readme() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let corpus = root.join(".yidam").join("corpus");
        let d = corpus.join("question");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("gamma.yml"),
            "class: question\nlabel: ?what is gamma\n",
        )
        .unwrap();

        assert_links_resolve(&render_open_questions(root, &corpus), root);
    }

    /// A row's link is `/`-separated whatever the host does, because the rendered table is
    /// committed and CI compares it byte-for-byte.
    #[test]
    fn link_targets_are_slash_separated() {
        let tmp = tempfile::tempdir().unwrap();
        let corpus = tmp.path().join(".yidam").join("corpus");
        node(&corpus, "person", "alpha");

        let rendered = render_corpus_index("", &corpus);
        assert!(rendered.contains("(person/alpha.yml)"), "{rendered}");
        assert!(!rendered.contains('\\'), "{rendered}");
    }
}
