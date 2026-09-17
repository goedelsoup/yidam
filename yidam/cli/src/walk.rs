use std::path::{Path, PathBuf};
use walkdir::DirEntry;

/// `max_depth` for a walk that is not depth-limited.
const ANY_DEPTH: usize = usize::MAX;

/// Every entry under `dir` that `keep` accepts, in sorted order.
///
/// **The sort is here, and that is the reason this helper exists.** All six walkers below
/// sorted; a seventh that forgot would be non-deterministic in a way no fixture with one
/// entry can show, because a `Vec` collected from a filtered walk carries whatever order the
/// filesystem handed back. Having one place to sort means a new walk cannot miss it.
///
/// `descend` is walkdir's `filter_entry`: it decides which directories are entered at all,
/// rather than which entries survive. Only [`walk_rust_files`] needs it; the rest pass
/// `|_| true`, which is a no-op.
///
/// Returns empty rather than erroring when `dir` is absent. Every caller here asks about a
/// directory a repository may legitimately not have yet.
fn walk_files(
    dir: &Path,
    max_depth: usize,
    descend: impl FnMut(&DirEntry) -> bool,
    keep: impl Fn(&DirEntry) -> bool,
) -> Vec<PathBuf> {
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(descend)
        .filter_map(|e| e.ok())
        .filter(|e| keep(e))
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

pub fn walk_md_files(dir: &Path) -> Vec<PathBuf> {
    walk_files(
        dir,
        1,
        |_| true,
        |e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_str() != Some("README.md")
        },
    )
}

/// Every file under `dir` that can carry a markdown link in prose: `.md` at any depth,
/// and `.yml`, because a corpus node's description is markdown inside YAML and is where
/// this repository's citations actually live.
///
/// Deliberately unlike [`walk_md_files`], which is `max_depth(1)` and skips `README.md`
/// because it feeds the table generators. Reusing it for links scanned almost nothing:
/// every README, every nested document, and every corpus node was invisible — including
/// the node whose broken citation motivated the check.
pub fn walk_linkable_files(dir: &Path) -> Vec<PathBuf> {
    walk_files(
        dir,
        ANY_DEPTH,
        |_| true,
        |e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|x| x == "md" || x == "yml")
        },
    )
}

/// Every `.rs` file under `dir`, at any depth.
///
/// `target/` is skipped rather than filtered afterwards: a built repository holds tens of
/// thousands of generated files there, and `unimplemented-class` would otherwise resolve a
/// class against a type in a build artifact of a dependency.
pub fn walk_rust_files(dir: &Path) -> Vec<PathBuf> {
    walk_files(
        dir,
        ANY_DEPTH,
        |e| e.file_name() != "target",
        |e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "rs"),
    )
}

// Instance .yml files live at depth >= 2 inside the corpus dir (inside class subdirs).
// Depth 1 files ending in .ont.yml are class schema files, not instances.
pub fn walk_corpus_instances(corpus: &Path) -> Vec<PathBuf> {
    walk_files(
        corpus,
        ANY_DEPTH,
        |_| true,
        |e| {
            e.file_type().is_file()
                && e.depth() >= 2
                && e.path().extension().is_some_and(|x| x == "yml")
                && !e.file_name().to_string_lossy().ends_with(".ont.yml")
        },
    )
}

// Class schema files live directly in the corpus dir and end in .ont.yml.
pub fn walk_ont_files(corpus: &Path) -> Vec<PathBuf> {
    walk_files(
        corpus,
        1,
        |_| true,
        |e| e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".ont.yml"),
    )
}

pub fn walk_decision_files(decisions_dir: &Path) -> Vec<PathBuf> {
    walk_files(
        decisions_dir,
        1,
        |_| true,
        |e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "yml"),
    )
}

pub fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn corpus_instances_excludes_ont_and_shallow_files() {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join("corpus");
        let class_dir = corpus.join("reach");
        fs::create_dir_all(&class_dir).unwrap();

        // .ont.yml at depth 1 — should NOT appear
        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        // instance .yml at depth 2 — should appear
        fs::write(class_dir.join("instance-01.yml"), "class: reach\n").unwrap();

        let found = walk_corpus_instances(&corpus);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("instance-01.yml"));
    }

    #[test]
    fn ont_files_finds_schema_only() {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join("corpus");
        let class_dir = corpus.join("reach");
        fs::create_dir_all(&class_dir).unwrap();

        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        fs::write(class_dir.join("instance-01.yml"), "class: reach\n").unwrap();

        let found = walk_ont_files(&corpus);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("reach.ont.yml"));
    }

    /// Every walker returns sorted paths, and does so on every run.
    ///
    /// Each fixture carries **at least two** matching entries, created in an order that is not
    /// their sorted order. One entry cannot fail this test however the walk is written: a
    /// single-element `Vec` is sorted by construction, which is exactly how an unsorted walk
    /// ships green. Repeated because the order a directory read hands back is not promised to
    /// be stable between calls, so a single agreeing run is not evidence.
    #[test]
    fn every_walk_is_sorted_on_every_run() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let corpus = root.join("corpus");
        let class_dir = corpus.join("reach");
        let nested = root.join("nested");
        let target = root.join("target");
        fs::create_dir_all(&class_dir).unwrap();
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Written last-first, so insertion order is the reverse of sorted order.
        for name in ["zebra", "middle", "alpha"] {
            fs::write(root.join(format!("{name}.md")), "x").unwrap();
            fs::write(root.join(format!("{name}.yml")), "x").unwrap();
            fs::write(root.join(format!("{name}.rs")), "x").unwrap();
            fs::write(nested.join(format!("{name}.md")), "x").unwrap();
            fs::write(class_dir.join(format!("{name}.yml")), "class: reach\n").unwrap();
            fs::write(corpus.join(format!("{name}.ont.yml")), "class: x\n").unwrap();
            // Must never be walked: proves `walk_rust_files` still skips `target/`.
            fs::write(target.join(format!("{name}.rs")), "x").unwrap();
        }
        fs::write(root.join("README.md"), "x").unwrap();

        /// A walker, by name, with the directory to point it at.
        type Walk<'a> = (&'a str, fn(&Path) -> Vec<PathBuf>, &'a Path);

        let walks: [Walk; 6] = [
            ("walk_md_files", walk_md_files, root),
            ("walk_linkable_files", walk_linkable_files, root),
            ("walk_rust_files", walk_rust_files, root),
            ("walk_corpus_instances", walk_corpus_instances, &corpus),
            ("walk_ont_files", walk_ont_files, &corpus),
            ("walk_decision_files", walk_decision_files, root),
        ];

        for (name, walk, dir) in walks {
            let first = walk(dir);
            assert!(
                first.len() >= 2,
                "{name} matched {} entries — a fixture this small cannot show an unsorted walk",
                first.len()
            );
            let mut want = first.clone();
            want.sort();
            assert_eq!(first, want, "{name} returned paths out of order");
            for run in 1..8 {
                assert_eq!(
                    walk(dir),
                    first,
                    "{name} disagreed with itself on run {run}"
                );
            }
        }

        assert!(
            walk_rust_files(root)
                .iter()
                .all(|p| !p.starts_with(&target)),
            "walk_rust_files descended into target/"
        );
    }

    /// An absent directory is empty, not an error — and still empty per walker.
    #[test]
    fn a_missing_directory_walks_to_nothing() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("not-here");
        assert!(walk_md_files(&missing).is_empty());
        assert!(walk_linkable_files(&missing).is_empty());
        assert!(walk_rust_files(&missing).is_empty());
        assert!(walk_corpus_instances(&missing).is_empty());
        assert!(walk_ont_files(&missing).is_empty());
        assert!(walk_decision_files(&missing).is_empty());
    }
}
