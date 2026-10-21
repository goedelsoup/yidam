//! `yidam export --format warehouse` end to end (RFC-0049 P1, #1396).
//!
//! The derivation and the ownership rules are unit-tested in `src/warehouse/`. What is held
//! here is what only the binary shows: every shipped example's ontology places in a contract,
//! an edit at the other end survives a re-export unless `--force` is given, and a feed file
//! on disk becomes load SQL (#1397).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn export(root: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["export", "--format", "warehouse", "--root"])
        .arg(root)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()
        .expect("running yidam export")
}

fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(repo_root().join("examples"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(".yidam/corpus").is_dir())
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn every_example_places_in_a_contract() {
    let examples = examples();
    assert!(
        examples.len() >= 4,
        "found {} examples; the scan is looking in the wrong place",
        examples.len()
    );
    for root in examples {
        let out = tempfile::tempdir().unwrap();
        let run = export(&root, out.path(), &[]);
        assert!(
            run.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&run.stderr)
        );
        let ddl = std::fs::read_to_string(out.path().join("ddl/bigquery/0001_init.sql")).unwrap();
        assert!(
            ddl.contains("CREATE TABLE IF NOT EXISTS"),
            "{}",
            root.display()
        );
        assert!(out.path().join("contract.json").is_file());
        assert!(out.path().join(".yidam-export.json").is_file());
    }
}

#[test]
fn an_edit_at_the_other_end_survives_a_re_export_unless_forced() {
    let root = repo_root().join("examples/streamflow");
    let out = tempfile::tempdir().unwrap();
    assert!(export(&root, out.path(), &[]).status.success());

    let ddl = out.path().join("ddl/bigquery/0001_init.sql");
    let mut edited = std::fs::read_to_string(&ddl).unwrap();
    edited.push_str("\n-- a dataset owner's note\n");
    std::fs::write(&ddl, &edited).unwrap();

    let refused = export(&root, out.path(), &[]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("ddl/bigquery/0001_init.sql was edited"),
        "{stderr}"
    );
    assert_eq!(std::fs::read_to_string(&ddl).unwrap(), edited);

    assert!(export(&root, out.path(), &["--force"]).status.success());
    assert_ne!(std::fs::read_to_string(&ddl).unwrap(), edited);
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dst = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dst);
        } else {
            std::fs::copy(entry.path(), dst).unwrap();
        }
    }
}

const GAUGES: &str = "\
feed: nwis
mode: append
parameters:
  landing: The dataset the NWIS site service lands in.
classes:
  gage:
    from: \"`${landing}.sites`\"
    id: site_no
    source: usgs-nwis
    columns:
      parameter: parm_cd
";

#[test]
fn a_feed_on_disk_becomes_load_sql_and_coverage() {
    let root = tempfile::tempdir().unwrap();
    copy_tree(&repo_root().join("examples/streamflow"), root.path());
    let feeds = root.path().join(".yidam/warehouse/feeds");
    std::fs::create_dir_all(&feeds).unwrap();
    std::fs::write(feeds.join("nwis.yml"), GAUGES).unwrap();

    let out = tempfile::tempdir().unwrap();
    let run = export(root.path(), out.path(), &[]);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("feeds: nwis"), "{stdout}");
    assert!(
        stdout.contains("classes no feed maps: concept, reach"),
        "{stdout}"
    );
    let sql = std::fs::read_to_string(out.path().join("feeds/nwis/gage.sql")).unwrap();
    assert!(sql.contains("FROM `${landing}.sites`"), "{sql}");
    assert!(
        !sql.contains("NOT MATCHED BY SOURCE"),
        "an append feed deletes nothing"
    );

    std::fs::write(feeds.join("nwis.yml"), GAUGES.replace("usgs-nwis", "usgs")).unwrap();
    let refused = export(root.path(), out.path(), &[]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains(".yidam/warehouse/feeds/nwis.yml: classes.gage.source"),
        "{stderr}"
    );
}
