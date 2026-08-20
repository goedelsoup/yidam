use anyhow::Result;
use std::path::Path;

use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::{line_count, walk_corpus_instances, walk_ont_files};

use super::has_open_claim;

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
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
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
    let mut items = Vec::new();
    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let label = inst.label.clone().unwrap_or_default();
        if label.starts_with('?') || has_open_claim(&text) {
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

    let mut nodes_with_issues: Vec<NodeIssues> = Vec::new();

    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let mut node_issues = Vec::new();

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

        let links = inst.links.unwrap_or_default();
        if links.is_empty() {
            node_issues.push("orphan node: no outgoing links".to_string());
        } else {
            let dir = path.parent().unwrap_or(path);
            for link in &links {
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
        passed: issue_count == 0,
        corpus_empty: false,
        total_instances: total,
        clean_instances: total - issue_count,
        classes_defined: defined_classes.len(),
        nodes_with_issues,
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
        out.push_str(&format!(
            "Checked {} instances across {} classes — all clean.",
            r.total_instances, r.classes_defined
        ));
    } else {
        out.push_str(&format!(
            "Checked {} instances across {} classes — {} clean, {} with issues:\n",
            r.total_instances,
            r.classes_defined,
            r.clean_instances,
            r.nodes_with_issues.len()
        ));
        for n in &r.nodes_with_issues {
            out.push_str(&format!("\n  {}", n.node));
            for issue in &n.issues {
                out.push_str(&format!("\n    - {issue}"));
            }
        }
    }

    if !r.classes_without_instances.is_empty() {
        out.push_str(&format!(
            "\n\nClasses with schema but no instances: {}",
            r.classes_without_instances.join(", ")
        ));
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
    let nodes = walk_corpus_instances(corpus)
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let claims = crate::claims::count_in_source(&text);
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
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
    let open_questions = walk_corpus_instances(corpus)
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
            let label = inst.label.clone().unwrap_or_default();
            if !(label.starts_with('?') || has_open_claim(&text)) {
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
    println!("{content}");
    update_file_regen(&corpus.join("README.md"), "yidam corpus-index", &content)
}

pub fn open_questions(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    if format.is_json() {
        return crate::report::emit(&root, open_questions_data(&root, &corpus));
    }
    let content = render_open_questions(&root, &corpus);
    println!("{content}");
    update_file_regen(&root.join("README.md"), "yidam open-questions", &content)
}

pub fn graph_check(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let data = graph_check_data(&root, &corpus);
    let issue_count = data.nodes_with_issues.len();

    if format.is_json() {
        crate::report::emit(&root, data)?;
    } else {
        println!("{}", render_graph_check_text(&data, &corpus));
    }

    // The gate, unchanged and shared: the verdict cannot depend on the rendering.
    if issue_count > 0 {
        anyhow::bail!("{issue_count} instance(s) have issues")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
