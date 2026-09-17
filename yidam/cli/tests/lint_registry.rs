//! An unregistered check cannot exist, and this is what that rests on.
//!
//! #680 was filed against `lint/mod.rs`'s hand-written `vec![]` of checks: a `pub fn … ->
//! Check` added to `checks.rs` and not added there **compiles, passes its own unit tests, and
//! never runs**, and the corpus then reports clean. The proposed fix was a test that scans the
//! source for `-> Check` signatures and compares them to the `checks::name(` call sites.
//!
//! **The premise is false, and the scan would have been the weaker guard.** Measured at
//! `14c94b9` by adding exactly the function the issue describes — unregistered, with a passing
//! unit test of its own:
//!
//! ```text
//! $ cargo clippy --manifest-path yidam/cli/Cargo.toml --all-targets --locked -- -D warnings
//! error: function `zz_unregistered_probe` is never used
//! error: could not compile `yidam` (lib) due to 1 previous error
//! ```
//!
//! That is `ci-cli`'s own clippy line (mise.toml). `mod cmd;` is private (`lib.rs:3`),
//! `lint` is `pub(crate)` and so is `checks`, and nothing re-exports them — so a check
//! function nothing calls is unreachable from outside the crate, and `dead_code` says so.
//! Its own unit test does not rescue it: the `lib` target is compiled without `cfg(test)`,
//! and that is the build that fails.
//!
//! The compiler is exhaustive by construction, which no source scan is. The scan would also
//! have been the exact failure this tranche is about — the `-> Check` signatures in
//! `checks.rs` are multi-line, and a naive regex over them both **over**-counts (four helpers
//! returning `Option<Vec<&str>>`, `PathBuf`, `HashSet<PathBuf>` and `Option<UsedByDrift>`
//! matched, because `.*?` crossed function boundaries to a later `-> Check`) and
//! **under**-counts by the same four, landing on 40 by cancellation.
//!
//! So there is nothing here to discover. What there is, is a **precondition nothing states**:
//! the guarantee holds only while the check functions are unreachable from outside the crate.
//! Add `pub use cmd::lint;` to `lib.rs`, or promote either module to `pub mod`, and every
//! unregistered check becomes externally reachable, `dead_code` falls silent, and #680's hole
//! opens for real — with no test going red anywhere, because the thing that was guarding it
//! was never written down.
//!
//! This file writes it down.
//!
//! ## The exception, and why the guarantee is not universal
//!
//! It holds for a check function that is not re-exported, which is all forty in `checks.rs`
//! and every producer in `attest.rs`, `commits.rs`, `citations.rs`, `scope.rs` and
//! `lineage.rs`. It does **not** hold for five in `line_citations.rs`. `lib.rs` names
//! `dead_line_citation`, `slid_line_citation`, `citation_label_not_cited`,
//! `unverified_line_citation` and `citation_range_stated_twice` in its `pub use cmd::{…}`, so
//! they are reachable from outside the crate and `dead_code` cannot fire on them. Four are
//! reached by `tests/line_citations.rs`, which is why they were exported;
//! `unverified_line_citation` is exported and called by nothing but the registry — a check
//! that, had it been left out of the registry, nothing would have caught.
//!
//! That is not a bug today: all five are registered. It is the shape of the residual hole, and
//! it is why [`exported_checks_are_exactly_the_recorded_exceptions`] pins the set. A sixth
//! check exported the same way leaves the guarantee silently; the test makes it say so.
//!
//! ## What nothing here covers
//!
//! A check function that *is* called somewhere under `lint/` but never reaches the registry
//! vec — bound to a `let _`, or pushed into a vector that is dropped. It is "used", so the
//! compiler is content, and it still never reaches a report. Narrower than what #680
//! describes; worth a follow-up, not worth a signature scanner.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// Source with `//` comments removed.
///
/// Every assertion below asks what the code *declares*. This file's own doc comment quotes
/// `pub mod` and `pub use cmd::lint;` as the things that would break the guarantee — a scan
/// counting those would be reading its own prose, and this repository has shipped that
/// mistake. Strip first, then look.
fn code(rel: &str) -> String {
    read(rel)
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_check_functions_are_unreachable_from_outside_the_crate() {
    let lib = code("yidam/cli/src/lib.rs");
    let cmd = code("yidam/cli/src/cmd/mod.rs");
    let lint = code("yidam/cli/src/cmd/lint/mod.rs");

    // The chain, link by link. Each line is one way the `dead_code` guarantee is lost, and a
    // failure should name which link went rather than "something is public now".
    let mut broken: Vec<&str> = Vec::new();

    if !lib.contains("\nmod cmd;") {
        broken.push(
            "lib.rs no longer declares `mod cmd;` privately — if it is `pub mod cmd;`, every \
             check function is externally reachable and an unregistered one stops being dead",
        );
    }
    if !cmd.contains("pub(crate) mod lint;") {
        broken.push(
            "cmd/mod.rs no longer declares `pub(crate) mod lint;` — a `pub mod lint;` publishes \
             the checks through any public path into `cmd`",
        );
    }
    if !lint.contains("pub(crate) mod checks;") {
        broken.push(
            "lint/mod.rs no longer declares `pub(crate) mod checks;` — `pub mod checks;` makes \
             every `pub fn … -> Check` reachable and silences the lint that registers them",
        );
    }
    // The fourth link — no re-export routing around the three above — is not a substring
    // test and cannot be: `dead_line_citation` contains neither "lint" nor "checks". It is
    // `exported_checks_are_exactly_the_recorded_exceptions` below, which discovers both sides.

    assert!(
        broken.is_empty(),
        "#680's guard is the compiler's `dead_code` lint, and it only fires while the check \
         functions are unreachable from outside this crate. That stopped being true:\n  - {}\n\n\
         Either restore the privacy, or replace this file with a real registry-completeness \
         test — but do not leave both gone.",
        broken.join("\n  - ")
    );
}

/// The lint must actually be denied, not merely emitted.
///
/// `dead_code` is a warning by default. Everything above is worth nothing if the gate that runs
/// clippy stops passing `-D warnings`, or stops building the `lib` target — which is the one
/// compiled without `cfg(test)`, and therefore the one where a check with its own unit test is
/// still unused.
#[test]
fn ci_cli_denies_warnings_over_the_library() {
    let mise = code("mise.toml");
    assert!(
        mise.contains("cargo clippy --manifest-path yidam/cli/Cargo.toml --all-targets --locked -- -D warnings"),
        "ci-cli no longer runs clippy over yidam/cli with `-D warnings`. That line is what \
         turns `dead_code` into a failure, and it is the whole of #680's guard."
    );
}

/// The names in `lint/mod.rs`'s registry, as the registry calls them.
///
/// A token scan for `module::name(`, which needs no signature parsing — the thing that makes a
/// `-> Check` scan unreliable is the multi-line *signature*, and a call site is one token.
/// Every check the report carries is reached through one of these.
fn registered_check_names() -> BTreeSet<String> {
    let src = code("yidam/cli/src/cmd/lint/mod.rs");
    let mut out = BTreeSet::new();
    for module in [
        "checks",
        "line_citations",
        "attest",
        "commits",
        "citations",
        "scope",
        "lineage",
    ] {
        let needle = format!("{module}::");
        let mut from = 0;
        while let Some(i) = src[from..].find(&needle) {
            let start = from + i + needle.len();
            let name: String = src[start..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            if !name.is_empty() && src[start + name.len()..].starts_with('(') {
                out.insert(name);
            }
            from = start.max(from + i + 1);
        }
    }
    out
}

/// Every name `lib.rs` re-exports out of the private `cmd` module.
fn exported_from_cmd() -> BTreeSet<String> {
    let src = code("yidam/cli/src/lib.rs");
    let mut out = BTreeSet::new();
    let mut from = 0;
    while let Some(i) = src[from..].find("pub use cmd::") {
        let start = from + i + "pub use cmd::".len();
        let end = start
            + src[start..]
                .find(';')
                .expect("a `pub use` ends in a semicolon");
        for tok in src[start..end].split([',', '{', '}', '\n']) {
            // `as` renames: the exported name is on the right, the item on the left. Both
            // matter — the item is what stops being dead.
            let item = tok.trim().split(" as ").next().unwrap_or("").trim();
            if !item.is_empty() && item.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.insert(item.to_string());
            }
        }
        from = end;
    }
    out
}

/// Exactly these checks are reachable from outside the crate, and no others.
///
/// Recorded rather than derived, because each one is a deliberate export with a consumer —
/// `tests/line_citations.rs` for four of them, and nothing at all for
/// `unverified_line_citation`. An entry here is a check the compiler has stopped guarding, so
/// the list is the price, and it should get shorter rather than longer.
const EXPORTED_CHECKS: [&str; 5] = [
    "citation_label_not_cited",
    "citation_range_stated_twice",
    "dead_line_citation",
    "slid_line_citation",
    "unverified_line_citation",
];

#[test]
fn exported_checks_are_exactly_the_recorded_exceptions() {
    let registered = registered_check_names();
    let exported = exported_from_cmd();

    // Both sides must have found something. A scan that returns an empty set agrees with every
    // assertion below, which is the failure this whole file is about.
    assert!(
        registered.len() >= 40,
        "found {} registered check names in lint/mod.rs, expected at least 40 — the scan has \
         stopped finding call sites and everything below it is vacuous",
        registered.len()
    );
    assert!(
        exported.len() >= 40,
        "found {} names re-exported from `cmd` in lib.rs, expected at least 40 — the scan has \
         stopped parsing the `pub use` blocks",
        exported.len()
    );

    let actual: BTreeSet<String> = registered.intersection(&exported).cloned().collect();
    let recorded: BTreeSet<String> = EXPORTED_CHECKS.iter().map(|s| s.to_string()).collect();

    assert_eq!(
        actual, recorded,
        "the set of checks reachable from outside `yidam` has changed.\n\nA check on this list \
         is outside the `dead_code` guarantee #680's hole is closed by: export it and an \
         unregistered one compiles clean again. Adding a name here is a real decision — say \
         which consumer needs it. Removing one is the direction to want."
    );
}
