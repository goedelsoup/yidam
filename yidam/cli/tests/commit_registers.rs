//! `lint --commits` honours the path registers — RFC-0028 §4, #575.
//!
//! The unit tests in `cmd/lint/commits.rs` hold the rules. This one holds the *wiring*: that
//! `[object] paths` in `.yidam/config.toml` reaches the check through a real binary reading a
//! real `git log`, and that the only thing separating the two runs below is that file.
//!
//! # The shape is measured, not invented
//!
//! #575's definition of done asks for a register split proved "against a real object-coupled
//! shape rather than a fixture invented for it". The tree built here is grindcore's, read
//! from disk during A0 and again on 2026-09-06: a corpus under `.yidam/`, a `web/` export and
//! a `crates/` workspace beside it. The two object-coupled repositories in the population have
//! the same shape with different names — matt-huffman adds `dossier/` and `public/`,
//! ohio-education-funding adds `design/` — and it is the shape rather than the names that the
//! register split has to handle.
//!
//! # Why the same repository twice
//!
//! One tree, two configs. Anything that differs between the runs is the declaration's doing,
//! because nothing else differs — same commits, same hashes, same binary. A pair of fixtures
//! could not say that.

use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git runs")
        .success();
    assert!(ok, "git {args:?} failed");
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn commit(root: &Path, subject: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--no-gpg-sign", "-m", subject]);
}

/// The `unrecognized-verb` findings `lint --commits` reports, by short hash.
fn findings(root: &Path) -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["lint", "--commits", "--format", "json"])
        .output()
        .expect("yidam runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let report: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("lint json ({e}): {text}"));
    let checks = report
        .get("checks")
        .and_then(|c| c.as_array())
        .expect("the report lists checks");
    let check = checks
        .iter()
        .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("unrecognized-verb"))
        .expect("`lint --commits` runs the vocabulary check");
    check
        .get("violations")
        .and_then(|v| v.as_array())
        .map(|vs| {
            vs.iter()
                .filter_map(|v| v.get("node")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A repository shaped like the object-coupled corpora in the population, with one commit of
/// each register. Returns the tempdir and the short hashes, oldest first.
fn object_coupled_repo() -> (tempfile::TempDir, Vec<String>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "register@yidam.test"]);
    git(root, &["config", "user.name", "Register"]);
    git(root, &["config", "commit.gpgsign", "false"]);

    // 1 — corpus only, in the vocabulary. Silent under either declaration.
    write(root, ".yidam/corpus/funding.md", "# funding\n");
    commit(root, "establish: school funding as a corpus concept");
    // 2 — object only, outside the vocabulary. The defect: reported as a corpus-vocabulary
    // violation today, and it is not one.
    write(root, "web/src/app.tsx", "export const App = () => null;\n");
    commit(root, "feat: add dark mode to the site");
    // 3 — object only again, in a second declared tree.
    write(root, "crates/mirror/src/lib.rs", "pub fn mirror() {}\n");
    commit(root, "test: cover the mirror's pagination edge");
    // 4 — both registers, outside the vocabulary. The corpus register governs it, and no
    // conduct finding is raised: that is #643's, struck on a measurement. (`regen` is
    // itself in the closed vocabulary, which is why the export act needs another verb to
    // make the point.)
    write(root, ".yidam/corpus/theme.md", "# theme\n");
    write(root, "web/src/theme.css", ":root{}\n");
    commit(root, "chore: regenerate the site from the corpus");
    // 5 — corpus only, outside the vocabulary. Reported under either declaration.
    write(root, ".yidam/corpus/charts.md", "# charts\n");
    commit(root, "viewport: charts that cannot draw a number");

    let out = Command::new("git")
        .current_dir(root)
        .args(["log", "--reverse", "--format=%H"])
        .output()
        .expect("git log");
    let hashes: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim()[..8].to_string())
        .collect();
    assert_eq!(hashes.len(), 5, "five commits");
    (dir, hashes)
}

/// Declaring no object reports every off-vocabulary commit, wherever it landed.
///
/// This is the behaviour every repository has today, and the one the register split may not
/// change for a repository that has said nothing.
#[test]
fn with_no_declaration_every_off_vocabulary_commit_is_reported() {
    let (dir, hashes) = object_coupled_repo();
    let got = findings(dir.path());
    let want: Vec<&String> = vec![&hashes[1], &hashes[2], &hashes[3], &hashes[4]];
    assert_eq!(
        got.len(),
        want.len(),
        "expected {want:?} in some order, got {got:?}"
    );
    for h in want {
        assert!(got.iter().any(|g| g == h), "{h} missing from {got:?}");
    }
}

/// The short hash of `HEAD`, in the form the report writes.
fn head(root: &Path) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&out.stdout).trim()[..8].to_string()
}

/// A node moved *out* of the corpus register is corpus work, and the register split may not
/// read it as the artifact's (#697).
///
/// Rename detection is on by default and `--name-only` prints a rename's destination alone,
/// so this commit presented as touching `web/` and nothing else: `Touch::ObjectOnly`,
/// declined without a word. Git holds the pre-image either way — the invocation was throwing
/// it away. The failing case is precisely a corpus node leaving the corpus, which is the
/// commit a vocabulary check most wants to see.
///
/// A pure `git mv` is the strongest form of the defect: at 100% similarity git detects the
/// rename under any threshold, so the test cannot pass by the pre-image surviving detection.
#[test]
fn a_node_moved_out_of_the_corpus_is_still_corpus_work() {
    let (dir, _) = object_coupled_repo();
    let root = dir.path();
    write(
        root,
        ".yidam/config.toml",
        "[object]\npaths = [\"web/**\", \"crates/**\"]\n",
    );
    commit(root, "resolve: the object register, declared");

    git(root, &["mv", ".yidam/corpus/funding.md", "web/funding.md"]);
    commit(root, "feat: move the node into the artifact");
    let moved = head(root);

    // The declaration is what makes this reachable: without `[object] paths` every commit is
    // corpus work and `ObjectOnly` never arises.
    assert!(
        findings(root).contains(&moved),
        "the commit that moved a node out of the corpus went unchecked — {:?}",
        findings(root)
    );
}

/// Declaring the object silences the two artifact commits and nothing else.
#[test]
fn declaring_the_object_silences_the_artifact_register_and_nothing_else() {
    let (dir, hashes) = object_coupled_repo();
    let root = dir.path();
    write(
        root,
        ".yidam/config.toml",
        "[object]\npaths = [\"web/**\", \"crates/**\"]\n",
    );
    commit(root, "resolve: the object register, declared");

    let got = findings(root);
    // The `feat:` and `test:` commits touched only the object. Gone.
    assert!(!got.contains(&hashes[1]), "feat: on web/ still reported");
    assert!(!got.contains(&hashes[2]), "test: on crates/ still reported");
    // The mixed commit and the corpus-only `viewport:` commit are untouched.
    assert!(
        got.contains(&hashes[3]),
        "the mixed commit spans both registers and the corpus governs it — {got:?}"
    );
    assert!(
        got.contains(&hashes[4]),
        "viewport: is corpus work — {got:?}"
    );
    assert_eq!(got.len(), 2, "exactly two survive — {got:?}");
}
