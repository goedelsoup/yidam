//! The policy says what the Rust guard says.
//!
//! This is the reason RFC-0024 does disclosure first. `vault::may_push` and
//! `vault::derived_may_push` are tested, shipped, and the only thing standing between a corpus
//! and publishing somebody else's paper — so the policy that will replace them in #440 has to
//! be shown equivalent *before* anything calls it, not argued to be.
//!
//! Both matrices compare the verdict always, and the reason wherever the reason is the
//! contract. Where they deliberately differ — `derived` reports every overlapping path and the
//! Rust returns the first — the test pins the difference rather than papering over it: the
//! Rust's message must be *among* the policy's.
//!
//! **Mutate this before trusting it.** Breaking a rule in `disclose/lib.rego` — dropping the
//! slash from `prefix_match`, or ignoring `holds_content` — must turn it red. A guard that
//! looks at nothing passes.

use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;
use yidam::policy::{input, Decision, Policies};
use yidam::vault::{derived_may_push, may_push, ContentHash, Derived, Disposition, Named};

fn named(rel: &str, redistributable: Option<bool>) -> Named {
    Named {
        hash: ContentHash::of_bytes(rel.as_bytes()),
        kind: "catalog".to_string(),
        rel: rel.to_string(),
        vault: None,
        redistributable,
        bytes: None,
        media_type: None,
    }
}

/// Assert the two agree, and say which case failed when they do not.
fn agree(case: &str, rust: &Disposition, policy: &Decision) {
    assert_eq!(
        rust.is_push(),
        policy.allow,
        "{case}: rust={rust:?} policy={policy:?}"
    );
    if let Disposition::Refused(why) = rust {
        assert!(
            policy.deny.iter().any(|d| d.msg == *why),
            "{case}: the Rust reason is not among the policy's.\n  rust:   {why}\n  policy: {:#?}",
            policy.deny
        );
    }
}

fn decide(root: &Path, decision: &str, input: &Value) -> Decision {
    Policies::load(root)
        .expect("the compiled-in defaults load")
        .decide(decision, input)
        .unwrap_or_else(|e| panic!("{decision} could not answer: {e:?}"))
}

/// `disclose/record` ↔ `vault::may_push`.
///
/// Three axes: what the record licenses, where the record sits, and — the trap — a directory
/// whose name merely starts like a declared one.
#[test]
fn the_record_decision_says_what_may_push_says() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // `dossier` holds material; `empty` holds only a placeholder. Neither should matter to this
    // decision, and the matrix is what proves it.
    std::fs::create_dir_all(root.join("dossier/deep")).unwrap();
    std::fs::write(root.join("dossier/deep/evidence.md"), "x").unwrap();
    std::fs::create_dir_all(root.join("empty")).unwrap();
    std::fs::write(root.join("empty/README.md"), "placeholder").unwrap();
    std::fs::create_dir_all(root.join("dossiers")).unwrap();
    std::fs::write(root.join("dossiers/a.md"), "x").unwrap();

    let declared = vec!["dossier".to_string(), "empty".to_string()];

    let rels = [
        "dossier",
        "dossier/deep/evidence.md",
        "dossiers/a.md",
        "empty/x.md",
        ".yidam/catalog/pearl-2009.md",
        "./dossier/deep/evidence.md",
    ];
    let licences = [None, Some(true), Some(false)];

    let mut checked = 0;
    for rel in rels {
        for licence in licences {
            let a = named(rel, licence);
            let rust = may_push(&a, &declared);
            let policy = decide(root, "disclose/record", &input::record(root, &a, &declared));
            agree(
                &format!("record rel={rel} licence={licence:?}"),
                &rust,
                &policy,
            );
            checked += 1;
        }
    }
    assert_eq!(checked, rels.len() * licences.len());
}

/// `disclose/derived` ↔ `vault::derived_may_push`.
///
/// Three axes: which artifact, where the declared path sits relative to what that artifact is
/// built from, and whether the declared path holds anything at all.
///
/// The third is the one a naive transcription drops, and dropping it passes every other case
/// here while making the feature unusable for a repository that declared its intent before it
/// had anything to protect.
#[test]
fn the_derived_decision_says_what_derived_may_push_says() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Inside a source directory, containing every source directory, and disjoint from all of
    // them — each in a real-material and a placeholder-only variant.
    let layout: [(&str, bool); 6] = [
        (".yidam/corpus/secret", true),
        (".yidam/corpus/hollow", false),
        (".yidam", true),
        ("dossier", true),
        (".yidam/catalog/private", true),
        (".yidam/skills/hollow", false),
    ];
    for (p, material) in layout {
        std::fs::create_dir_all(root.join(p)).unwrap();
        if material {
            std::fs::write(root.join(p).join("note.md"), "x").unwrap();
        } else {
            std::fs::write(root.join(p).join("README.md"), "placeholder").unwrap();
            std::fs::write(root.join(p).join(".gitkeep"), "").unwrap();
        }
    }

    let mut checked = 0;
    for (p, _) in layout {
        // One declared path at a time, so a disagreement names the path that caused it.
        let declared = vec![p.to_string()];
        for d in Derived::ALL {
            let rust = derived_may_push(root, d, &declared);
            let policy = decide(
                root,
                "disclose/derived",
                &input::derived(root, d, &declared),
            );
            agree(
                &format!("derived kind={} declared={p}", d.kind()),
                &rust,
                &policy,
            );
            checked += 1;
        }
    }
    assert_eq!(checked, layout.len() * Derived::ALL.len());

    // And all of them together, which is the shape a real repository has.
    let declared: Vec<String> = layout.iter().map(|(p, _)| p.to_string()).collect();
    for d in Derived::ALL {
        let rust = derived_may_push(root, d, &declared);
        let policy = decide(
            root,
            "disclose/derived",
            &input::derived(root, d, &declared),
        );
        agree(
            &format!("derived kind={} declared=all", d.kind()),
            &rust,
            &policy,
        );
    }
}

/// A repository that declared nothing is the common case, and both must permit everything.
#[test]
fn nothing_declared_private_refuses_nothing_on_either_side() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let declared: Vec<String> = Vec::new();

    let a = named(".yidam/catalog/x.md", Some(true));
    agree(
        "record, nothing declared",
        &may_push(&a, &declared),
        &decide(root, "disclose/record", &input::record(root, &a, &declared)),
    );

    for d in Derived::ALL {
        agree(
            &format!("derived {}, nothing declared", d.kind()),
            &derived_may_push(root, d, &declared),
            &decide(
                root,
                "disclose/derived",
                &input::derived(root, d, &declared),
            ),
        );
    }
}

/// The deliberate difference, pinned so nobody "fixes" it into agreement.
///
/// `derived_may_push` returns on the first intersecting path; the policy reports all of them.
/// Somebody about to fix a refusal wants every path, not the alphabetically first — but the
/// verdicts still have to match, which is what the matrices above check.
#[test]
fn the_derived_policy_names_every_overlapping_path_where_the_rust_names_one() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    for p in [".yidam/corpus/a", ".yidam/catalog/b"] {
        std::fs::create_dir_all(root.join(p)).unwrap();
        std::fs::write(root.join(p).join("note.md"), "x").unwrap();
    }
    let declared = vec![
        ".yidam/corpus/a".to_string(),
        ".yidam/catalog/b".to_string(),
    ];

    let rust = derived_may_push(root, Derived::Bundle, &declared);
    let policy = decide(
        root,
        "disclose/derived",
        &input::derived(root, Derived::Bundle, &declared),
    );
    assert!(!rust.is_push());
    assert!(!policy.allow);
    assert_eq!(
        policy.deny.len(),
        2,
        "both private paths must be named: {:#?}",
        policy.deny
    );
    agree("derived, two overlaps", &rust, &policy);
}
