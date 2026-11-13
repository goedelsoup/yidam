//! `.yidam.toml`'s `[build] features`, and the three shipped shell paths that read it.
//!
//! A derived repository that needs a feature the released binary was not built with had no
//! way to say so. Its build compiled with `--features …` by hand; the moment its pin reached
//! a `cli/v*` tag, both download paths would replace that binary with the released one and
//! every gated command would start answering "needs --features …". Commands do not *fail* on
//! that — they decline — so a test written to skip when a capability is absent goes on
//! passing while asserting nothing. #532, reported by a derived repository that spotted it
//! before adopting.
//!
//! One declaration, three readers, and they must agree:
//!
//! | Path | What it does with it |
//! |---|---|
//! | `yidam-build-source` | passes it to `cargo install --features` |
//! | `yidam-build` | asks the downloaded binary's `--version` and compiles instead if short |
//! | `yidam-vendor-update` | withholds the `[tools]` entry, so `mise install` cannot swap |
//!
//! **The shipped shell is what runs here.** These tests pull the exact lines out of
//! `mise.yidam.toml` and execute them under `sh` against fixtures. Restating the logic in
//! Rust would grade a second implementation and let the real one drift — and the real one is
//! three escaping layers deep (a TOML multi-line string, read back by a test, run through
//! sh), which is where the two bugs this file found actually lived: a stray double space that
//! made the write-back emit `["a", "", "b"]`.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One task's `run` body, out of the file that ships.
fn task_body(task: &str) -> String {
    let path = repo_root().join("mise.yidam.toml");
    let text = std::fs::read_to_string(&path).expect("mise.yidam.toml is readable");
    let doc: toml::Table = text.parse().expect("mise.yidam.toml parses");
    doc.get(task)
        .and_then(|t| t.get("run"))
        .and_then(|r| r.as_str())
        .unwrap_or_else(|| panic!("mise.yidam.toml has no [{task}] with a string `run`"))
        .to_string()
}

/// The one line of `task`'s body that contains `needle`.
///
/// By content rather than by line number: an anchor that moves is a test that fails saying
/// so, which is the right failure. A line number that moves is a test grading something else.
fn line_of(task: &str, needle: &str) -> String {
    let body = task_body(task);
    let mut found: Vec<&str> = body.lines().filter(|l| l.contains(needle)).collect();
    assert_eq!(
        found.len(),
        1,
        "[{task}] has {} line(s) containing {needle:?}; this test grades exactly one",
        found.len()
    );
    found.pop().expect("one line").trim().to_string()
}

/// The block of `task`'s body from the line containing `start` through the next line whose
/// trimmed content is `end`, inclusive.
///
/// For the multi-line fragments a single `line_of` cannot reach. The first version of the
/// shortfall test below did not have this and inlined the loop into the Rust instead — which
/// this file's own header forbids, and which mutation testing caught: breaking the shipped
/// loop left the test green.
fn block_of(task: &str, start: &str, end: &str) -> String {
    let body = task_body(task);
    let lines: Vec<&str> = body.lines().collect();
    let from = lines
        .iter()
        .position(|l| l.contains(start))
        .unwrap_or_else(|| panic!("[{task}] has no line containing {start:?}"));
    let to = lines[from..]
        .iter()
        .position(|l| l.trim() == end)
        .unwrap_or_else(|| panic!("[{task}] has no {end:?} after {start:?}"))
        + from;
    lines[from..=to]
        .iter()
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a script in a scratch directory, returning its stdout.
fn sh(dir: &std::path::Path, script: &str) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .expect("sh");
    assert!(
        out.status.success(),
        "script failed ({}):\n{script}\n--- stderr ---\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn scratch(yidam_toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(".yidam.toml"), yidam_toml).expect("write .yidam.toml");
    dir
}

const DECLARED: &str = r#"[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "abc123"

[build]
features = ["export-sqlite", "index"]
"#;

const NOTHING_DECLARED: &str = r#"[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "abc123"
"#;

/// All three readers get the same list out of the same file.
///
/// They are three separate lines of shell in three tasks, which is three chances to normalise
/// differently — and two of them did, leaving a double space that only showed up in the one
/// place the value is re-serialised rather than word-split.
#[test]
fn every_reader_of_the_declaration_agrees() {
    let dir = scratch(DECLARED);
    let readers = [
        ("yidam-build", "required=$(sed -n"),
        ("yidam-build-source", "want=$(sed -n"),
        ("yidam-vendor-update", "required=$(sed -n"),
    ];
    for (task, needle) in readers {
        let line = line_of(task, needle);
        let var = line.split('=').next().expect("an assignment").to_string();
        let got = sh(dir.path(), &format!("{line}\nprintf '%s' \"${var}\""));
        assert_eq!(
            got, "export-sqlite index",
            "[{task}] reads the declaration as {got:?}"
        );
    }
}

/// A repository that declares nothing gets an empty list, not a parse error or a stray value.
#[test]
fn no_declaration_reads_as_no_requirement() {
    let dir = scratch(NOTHING_DECLARED);
    let line = line_of("yidam-build", "required=$(sed -n");
    assert_eq!(
        sh(dir.path(), &format!("{line}\nprintf '%s' \"$required\"")),
        ""
    );
}

/// The source build turns the declaration into a cargo flag.
#[test]
fn the_source_build_passes_the_declared_features_to_cargo() {
    let dir = scratch(DECLARED);
    let read = line_of("yidam-build-source", "want=$(sed -n");
    let flag = line_of("yidam-build-source", "features=\"--features");
    let got = sh(
        dir.path(),
        &format!("{read}\n{flag}\nprintf '%s' \"$features\""),
    );
    assert_eq!(got, "--features export-sqlite,index");
}

/// The write-back round-trips: what `yidam-vendor-update` re-emits parses, and parses back to
/// the list it was given.
///
/// This is the assertion that caught the real bug. With a double space in `required`, the
/// `sed` that quotes it produced `["export-sqlite", "", "index"]` — valid TOML naming a
/// feature that is the empty string, which no release will ever carry, so the repository
/// would have compiled forever and never been told why.
#[test]
fn the_declaration_survives_a_re_vendor() {
    let dir = scratch(DECLARED);
    let read = line_of("yidam-vendor-update", "required=$(sed -n");
    let quote = line_of("yidam-vendor-update", "quoted=$(echo $required");
    let got = sh(
        dir.path(),
        &format!("{read}\n{quote}\nprintf 'features = [\"%s\"]' \"$quoted\""),
    );
    let back: toml::Table = got
        .parse()
        .unwrap_or_else(|e| panic!("{got:?} is not TOML: {e}"));
    let list: Vec<String> = back["features"]
        .as_array()
        .expect("an array")
        .iter()
        .map(|v| v.as_str().expect("a string").to_string())
        .collect();
    assert_eq!(
        list,
        vec!["export-sqlite", "index"],
        "round-tripped as {got:?}"
    );
}

/// The comparison a refusal runs on: what the release carries, against what is required.
///
/// The loop is **lifted out of the task**, not written here. `haystack` is the variable that
/// task matches against — `released` for the pinned tree's default set, `have` for the
/// downloaded binary's own report — and the two loops are otherwise the same shape.
///
/// Returned trimmed by `sh`, so the leading space the shell accumulator carries is not part
/// of what is asserted; the message the tasks print pads it back.
fn shortfall(task: &str, haystack: &str, required: &str, carried: &str) -> String {
    let loop_body = block_of(task, "for f in $required; do", "done");
    let dir = tempfile::tempdir().expect("tempdir");
    sh(
        dir.path(),
        &format!(
            "required='{required}'\n{haystack}=' {carried} '\nshort=''\n{loop_body}\n\
             printf '%s' \"$short\""
        ),
    )
}

/// A release short of a required feature is refused; one that carries them is adopted.
///
/// The loop here is the shape both refusals use. `export-graph` is the case #532 was reported
/// on and the case this repository fixed by moving it into `default` — so with the current
/// default set that repository is adopted, and the mechanism is what protects the next one.
#[test]
fn a_release_is_adopted_only_when_it_carries_what_was_asked_for() {
    let default_now = "reports tonpa vault-s3 export-graph";
    // Both refusals, because they are two separate loops in two tasks: `mise install` reaches
    // only the vendor-update one, `mise run yidam-build` only the other.
    for (task, haystack) in [("yidam-vendor-update", "released"), ("yidam-build", "have")] {
        assert_eq!(
            shortfall(task, haystack, "export-graph", default_now),
            "",
            "export-graph is in the default set now; a repository needing it takes the download"
        );
        assert_eq!(
            shortfall(task, haystack, "export-sqlite index", default_now),
            "export-sqlite index",
            "a release without them must be refused, and must name what is missing"
        );
        assert_eq!(
            shortfall(task, haystack, "export-graph export-sqlite", default_now),
            "export-sqlite",
            "one missing feature is enough, and only the missing one is named"
        );
        // The pre-#532 default set: the exact situation the derived repository reported.
        assert_eq!(
            shortfall(task, haystack, "export-graph", "reports tonpa vault-s3"),
            "export-graph",
            "this is the swap that would have deleted a conformance gate"
        );
    }
}
