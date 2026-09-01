//! An advisories file of undated ignores is a check that has stopped checking.
//!
//! `cargo deny` found three advisories on the day it was installed, and the cheap way to a
//! green gate was three lines in `[advisories] ignore`. Nobody would have noticed; the check
//! would still run, still pass, and still be pointed at the same dependencies. That is the
//! shape #461 found in `verify` — a check nothing executes — arrived at from the other side:
//! a check that executes and has been told to say yes.
//!
//! So an ignore is allowed and it is not free. It carries **a reason and an expiry**, and
//! this file is what makes that a rule rather than a convention. The expiry is the important
//! half: a reason ages into a sentence nobody re-reads, and a date fails a build.
//!
//! What this cannot check is whether a reason is a *good* one. That is review's job. What it
//! catches is the shape every rushed ignore has — bare, undated, and added at 6pm.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn deny_toml() -> toml::Table {
    let p = repo_root().join("deny.toml");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("deny.toml unreadable: {e}"));
    text.parse().expect("deny.toml does not parse as TOML")
}

/// The `[advisories] ignore` list, whatever shape its entries take.
fn ignores() -> Vec<toml::Value> {
    deny_toml()
        .get("advisories")
        .and_then(|a| a.as_table())
        .and_then(|a| a.get("ignore"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Every ignored advisory has a reason and a date it stops being ignored.
///
/// cargo-deny takes an ignore as a bare string (`"RUSTSEC-2026-0195"`) or as a table with
/// `id`, `reason` and `expiry`. Only the second form is allowed here: the bare string is the
/// one that gets added in a hurry, and it is indistinguishable a year later from a decision
/// somebody thought about.
#[test]
fn every_ignored_advisory_carries_a_reason_and_an_expiry() {
    let mut bare = Vec::new();
    for entry in ignores() {
        match &entry {
            toml::Value::String(id) => bare.push(format!(
                "  {id} — a bare id. Give it `reason` and `expiry`, or fix the advisory."
            )),
            toml::Value::Table(t) => {
                let id = t
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<no id>")
                    .to_string();
                let reason = t.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                let expiry = t.get("expiry").and_then(|v| v.as_str()).unwrap_or("");
                if reason.trim().len() < 20 {
                    bare.push(format!(
                        "  {id} — `reason` is {} characters. Say what makes this one \
                         acceptable, for whoever reads it after you.",
                        reason.trim().len()
                    ));
                }
                if expiry.trim().is_empty() {
                    bare.push(format!(
                        "  {id} — no `expiry`. An ignore without one never comes back, and \
                         the gate quietly stops covering it."
                    ));
                }
            }
            other => bare.push(format!("  unreadable ignore entry: {other:?}")),
        }
    }
    assert!(
        bare.is_empty(),
        "deny.toml ignores advisories without saying why or until when:\n{}",
        bare.join("\n")
    );
}

/// The checks that would silently stop checking are all switched on.
///
/// Each of these has a default that is *not* what this repository wants, and each failure
/// mode is silence rather than noise: a yanked crate resolving happily, an unknown registry
/// serving a dependency, a wildcard version whose meaning changes without the manifest
/// changing. Derived from the file rather than assumed, because a `deny.toml` that has been
/// quietly relaxed looks exactly like one that has not.
#[test]
fn the_checks_that_would_fail_silently_are_switched_on() {
    let t = deny_toml();
    let get = |section: &str, key: &str| -> String {
        t.get(section)
            .and_then(|s| s.as_table())
            .and_then(|s| s.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("<unset>")
            .to_string()
    };
    assert_eq!(
        get("advisories", "yanked"),
        "deny",
        "a yanked crate is one the author withdrew; resolving one must not be a warning"
    );
    assert_eq!(
        get("bans", "wildcards"),
        "deny",
        "a wildcard version is a build whose input changes without this repository changing"
    );
    assert_eq!(
        get("sources", "unknown-registry"),
        "deny",
        "a dependency from an unnamed registry is a dependency nobody reviewed"
    );
    assert_eq!(
        get("sources", "unknown-git"),
        "deny",
        "a git dependency has no version to pin and no registry to yank it"
    );
}

/// The licence policy is a list, not an "allow everything".
///
/// `allow = []` with no other setting, or a `[licenses]` table that has been removed, both
/// leave a check that runs and permits anything. This asserts the shape rather than the
/// contents: which licences are acceptable is a decision for a person, and the file says so.
#[test]
fn the_licence_policy_names_licences() {
    let t = deny_toml();
    let allow = t
        .get("licenses")
        .and_then(|l| l.as_table())
        .and_then(|l| l.get("allow"))
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        allow.len() >= 4,
        "deny.toml allows {} licence(s). An empty or near-empty allow list is either a gate \
         nothing can pass or one that was switched off.",
        allow.len()
    );
    let names: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();
    for copyleft in ["GPL-2.0", "GPL-3.0", "AGPL-3.0", "LGPL-3.0"] {
        assert!(
            !names.iter().any(|n| n.contains(copyleft)),
            "deny.toml allows {copyleft}, which this repository cannot ship under MIT. If \
             that changed, it changed in a licence file first."
        );
    }
}
