//! `yidam export --format warehouse` end to end (RFC-0049 P1, #1396).
//!
//! The derivation and the ownership rules are unit-tested in `src/warehouse/`. What is held
//! here is what only the binary shows: every shipped example's ontology places in a contract,
//! an edit at the other end survives a re-export unless `--force` is given, a feed file on
//! disk becomes load SQL (#1397), the `textql` folder keeps the same rules (#1398), a
//! changed ontology becomes a migration or a refusal (#1401), and `--with-corpus` writes the
//! nodes as one more feed only when asked (#1400).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

use common::git::git;

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
        stderr.contains("ddl/bigquery/0001_init.sql is a migration an earlier export wrote"),
        "{stderr}"
    );
    assert_eq!(std::fs::read_to_string(&ddl).unwrap(), edited);

    // A migration is history: --force records the edit and does not rewrite the file,
    // since the edited script may be the one that ran.
    assert!(export(&root, out.path(), &["--force"]).status.success());
    assert_eq!(std::fs::read_to_string(&ddl).unwrap(), edited);
    assert!(export(&root, out.path(), &[]).status.success());
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

fn export_textql(root: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["export", "--format", "textql", "--root"])
        .arg(root)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()
        .expect("running yidam export")
}

/// #1398: the TextQL folder keeps the same ownership rules as the warehouse tree.
#[test]
fn an_edited_textql_page_is_refused_unless_forced() {
    let root = repo_root().join("examples/streamflow");
    let out = tempfile::tempdir().unwrap();
    let first = export_textql(&root, out.path(), &["--prefix", "ontology/streamflow"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        stdout.contains("| Concept, Gage, Reach | `ontology/streamflow/README.md` |"),
        "{stdout}"
    );
    assert!(!out.path().join("ANA.md").exists());

    let page = out.path().join("classes/gage.md");
    let mut edited = std::fs::read_to_string(&page).unwrap();
    edited.push_str("\nA TextQL user's note.\n");
    std::fs::write(&page, &edited).unwrap();

    let refused = export_textql(&root, out.path(), &[]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("classes/gage.md was edited"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&page).unwrap(), edited);

    assert!(export_textql(&root, out.path(), &["--force"])
        .status
        .success());
    assert_ne!(std::fs::read_to_string(&page).unwrap(), edited);
}

#[test]
fn a_renamed_property_is_refused_until_allowed_and_then_written_for_review() {
    let root = tempfile::tempdir().unwrap();
    copy_tree(&repo_root().join("examples/streamflow"), root.path());
    let out = tempfile::tempdir().unwrap();
    let first = export(root.path(), out.path(), &[]);
    assert!(
        String::from_utf8_lossy(&first.stdout).contains("migration: ddl/bigquery/0001_init.sql")
    );
    let same = export(root.path(), out.path(), &[]);
    assert!(
        String::from_utf8_lossy(&same.stdout).contains("migration: none"),
        "{}",
        String::from_utf8_lossy(&same.stdout)
    );

    let gage = root.path().join(".yidam/corpus/gage.ont.yml");
    let text = std::fs::read_to_string(&gage).unwrap();
    std::fs::write(&gage, text.replace("name: units", "name: unit")).unwrap();

    let refused = export(root.path(), out.path(), &[]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("column `gage.units` removed"), "{stderr}");
    assert!(stderr.contains("column `gage.unit` added"), "{stderr}");
    assert!(!out.path().join("ddl/bigquery/breaking").exists());

    let allowed = export(root.path(), out.path(), &["--allow-breaking"]);
    let stdout = String::from_utf8_lossy(&allowed.stdout);
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
    assert!(
        stdout.contains("migration: ddl/bigquery/0002_add_gage__unit.sql"),
        "{stdout}"
    );
    assert!(stdout.contains("column `gage.units` removed"), "{stdout}");
    let breaking =
        std::fs::read_to_string(out.path().join("ddl/bigquery/breaking/0002.sql")).unwrap();
    assert!(
        breaking.contains("ALTER TABLE `gage` DROP COLUMN IF EXISTS `units`;"),
        "{breaking}"
    );
    assert!(out.path().join("ddl/bigquery/0001_init.sql").is_file());
}

#[test]
fn the_corpus_travels_only_with_the_flag_and_only_from_a_corpus_with_its_own_genesis() {
    let root = tempfile::tempdir().unwrap();
    copy_tree(&repo_root().join("examples/streamflow"), root.path());
    let out = tempfile::tempdir().unwrap();

    // A tempdir is no repository, so there is no genesis to name the rows' corpus by.
    let refused = export(root.path(), out.path(), &["--with-corpus"]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("genesis commit"), "{stderr}");
    assert!(!out.path().join("contract.json").exists());

    git(root.path(), &["init", "-q"]);
    git(root.path(), &["add", "-A"]);
    git(root.path(), &["commit", "-qm", "genesis: streamflow"]);
    let run = export(root.path(), out.path(), &["--with-corpus"]);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("corpus: "), "{stdout}");
    let corpus = out.path().join("feeds/corpus");
    let gage = std::fs::read_to_string(corpus.join("gage.ndjson")).unwrap();
    assert_eq!(gage.lines().count(), 2, "{gage}");
    assert!(gage.contains("\"id\":\"yidam://"), "{gage}");
    assert!(gage.contains("\"_source\":\"usgs-nwis\""), "{gage}");
    assert!(corpus.join("gage.schema.json").is_file());
    let runner = std::fs::read_to_string(corpus.join("load.sh")).unwrap();
    assert!(runner.contains("gage.ndjson gage.schema.json"), "{runner}");

    // Without the flag, no instance data: what the last export wrote is removed.
    let again = export(root.path(), out.path(), &[]);
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert!(
        !corpus.exists(),
        "feeds/corpus survived an export without --with-corpus"
    );
}
