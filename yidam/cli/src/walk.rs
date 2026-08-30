use std::path::{Path, PathBuf};

pub fn walk_md_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_str() != Some("README.md")
        })
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
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
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|x| x == "md" || x == "yml")
        })
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

/// Every `.rs` file under `dir`, at any depth.
///
/// `target/` is skipped rather than filtered afterwards: a built repository holds tens of
/// thousands of generated files there, and `unimplemented-class` would otherwise resolve a
/// class against a type in a build artifact of a dependency.
pub fn walk_rust_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.file_name() != "target")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

// Instance .yml files live at depth >= 2 inside the corpus dir (inside class subdirs).
// Depth 1 files ending in .ont.yml are class schema files, not instances.
pub fn walk_corpus_instances(corpus: &Path) -> Vec<PathBuf> {
    if !corpus.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(corpus)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.depth() >= 2
                && e.path().extension().is_some_and(|x| x == "yml")
                && !e.file_name().to_string_lossy().ends_with(".ont.yml")
        })
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

// Class schema files live directly in the corpus dir and end in .ont.yml.
pub fn walk_ont_files(corpus: &Path) -> Vec<PathBuf> {
    if !corpus.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(corpus)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".ont.yml")
        })
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

pub fn walk_decision_files(decisions_dir: &Path) -> Vec<PathBuf> {
    if !decisions_dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(decisions_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "yml"))
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
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
}
