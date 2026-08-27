//! Prelude and scaffold links must resolve in the tree they are *installed* into.
//!
//! These files are written for where they land, not where they sit. `yidam/prelude/`
//! installs to `.yidam/.vendor/prelude/`; `sadhana/root/AGENTS.md` installs to the
//! repository root; `samudaya/` is deleted at genesis. So a relative link here is
//! checkable only against the layout bootstrap produces, and checking it against *this*
//! repository gets the answer backwards in both directions:
//!
//! | link | here | installed |
//! |---|---|---|
//! | `directories.md` → `../.vendor/prelude/CONSTITUTION.md` | absent | present — correct |
//! | `directories.md` → `../../../samudaya/README.md` | present | `.yidam/samudaya/` — absent |
//!
//! One is right where it looks wrong; the other is wrong where it looks right. A lexical
//! rule ("never escape the prelude") flags both identically and was tried and discarded
//! for that reason. What works is modelling the destination.
//!
//! # A destination can be conditional
//!
//! Modelling the destination is not enough on its own, because not every destination is
//! installed. `sadhana/sangha/` is copied only under `governance: collective`; single-elector
//! is the documented default and the case bootstrap takes when the user is unsure. This test
//! shipped listing `sangha` as an unconditional row and its own header table cited
//! `GRAPH.md → ../../sangha/README.md` as the worked example of a link that is *right where
//! it looks wrong*. It was wrong in the majority of derived repositories, and the test held
//! the answer key that said otherwise — so the link stayed broken and green.
//!
//! Conditional rows carry [`Install::when`]. Links are resolved against the tree a repository
//! has when it satisfies only the conditions the linking file itself implies: a file inside
//! `sadhana/sangha/` may link within the sangha, and a file outside it may not link in.
//!
//! The mapping below is the one `bootstrap.md` prescribes, and
//! [`the_mapping_agrees_with_bootstrap`] holds the two together: bootstrap's own
//! `source → destination` table is parsed and every row must appear here. A test checking
//! a layout nobody installs would be worse than no test.

use std::collections::BTreeSet;
use std::path::Path;

use walkdir::WalkDir;

mod common;

use common::{install_of, repo_root, ALWAYS_PRESENT, COLLECTIVE, DOMAIN_SELECTED, MAPPING};

/// Sections of an otherwise unconditional file that bootstrap deletes unless a condition
/// holds: `(source file, opening heading, condition)`.
///
/// `sadhana/root/AGENTS.md` installs into every derived repository, but its governance
/// section carries a `<!-- TEMPLATE -->` instruction to delete the whole thing in a
/// single-elector repository. Its links into `.yidam/sangha/` are correct — they are removed
/// by the same answer that removes their targets.
///
/// Declared here rather than read out of the marker. The `<!-- TEMPLATE … -->` syntax is a
/// frozen parity contract (`parse_markers`, implemented in three languages against shared
/// fixtures), so teaching it to carry a condition is a change to that contract and not
/// something a link checker gets to make on its own.
const CONDITIONAL_REGIONS: &[(&str, &str, &str)] =
    &[("sadhana/root/AGENTS.md", "## Governance", COLLECTIVE)];

/// Line numbers of `text` that fall inside a region conditional on something.
///
/// A region runs from its heading to the next heading of the same or higher level, which is
/// the extent "delete this whole section" means to the agent reading it.
fn conditional_lines(src: &str, text: &str) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for (file, heading, _) in CONDITIONAL_REGIONS {
        if src != *file {
            continue;
        }
        let depth = heading.chars().take_while(|c| *c == '#').count();
        let mut inside = false;
        for (n, line) in text.lines().enumerate() {
            if line.starts_with('#') {
                let d = line.chars().take_while(|c| *c == '#').count();
                inside = if line.trim_end() == *heading {
                    true
                } else if d <= depth {
                    false
                } else {
                    inside
                };
            }
            if inside {
                out.insert(n + 1);
            }
        }
    }
    out
}

/// Every path a derived repository holds, files and directories alike, given the bootstrap
/// answers it gave.
///
/// `conditions` is what the repository said yes to. A row whose [`Install::when`] is not in
/// it contributes nothing, because bootstrap never copied it.
fn installed_tree(root: &Path, conditions: &BTreeSet<&str>) -> BTreeSet<String> {
    let mut tree: BTreeSet<String> = ALWAYS_PRESENT.iter().map(|s| s.to_string()).collect();
    for e in MAPPING {
        if e.when.is_some_and(|w| !conditions.contains(w)) {
            continue;
        }
        for entry in WalkDir::new(root.join(e.src))
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let s = entry.path().to_string_lossy();
            if s.contains("/node_modules/")
                || s.contains("/.venv/")
                || s.contains("/target/")
                || s.contains("/__pycache__/")
                || s.contains("/.pytest_cache/")
            {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let Some((owner, Some(dest))) = install_of(&rel.to_string_lossy()) else {
                continue;
            };
            // The condition of the row that *claims* this path, which is not always the row
            // being walked. `yidam/prelude` is unconditional and `yidam/prelude/domains` is
            // not, so consulting only the outer row installs the domains under the prelude's
            // answer and the nested condition withholds nothing. The sangha row never showed
            // this: it is a top-level directory no other row walks over.
            if owner.when.is_some_and(|w| !conditions.contains(w)) {
                continue;
            }
            // Every ancestor is a directory that exists.
            let mut acc = std::path::PathBuf::new();
            for part in Path::new(&dest).components() {
                acc.push(part);
                tree.insert(acc.to_string_lossy().to_string());
            }
        }
    }
    tree
}

/// Replace the contents of `` `…` `` spans with spaces, preserving byte offsets.
fn blank_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(c);
        } else if in_code {
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Markdown link targets worth resolving, skipping fenced and inline code.
fn targets(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        // Blank inline code so `[label](path)` shown AS an example is not read as a link.
        // Four of this test's first six findings were this, and every one of them was a
        // document explaining what an edge looks like.
        let line = blank_code_spans(raw);
        let mut j = 0;
        while let Some(open) = line[j..].find("](") {
            let at = j + open + 2;
            let Some(close) = line[at..].find(')') else {
                break;
            };
            let t = line[at..at + close].trim();
            j = at + close + 1;
            let t = t.split_whitespace().next().unwrap_or(t);
            let t = t.split('#').next().unwrap_or(t);
            if t.is_empty()
                || t.starts_with('#')
                || t.contains("://")
                || t.starts_with("mailto:")
                || t.starts_with('/')
            {
                continue;
            }
            out.push((n + 1, t.to_string()));
        }
    }
    out
}

/// Resolve `target` from `from_dir` lexically. `None` if it climbs above the root.
fn resolve(from_dir: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if from_dir.is_empty() {
        vec![]
    } else {
        from_dir.split('/').collect()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

#[test]
fn every_template_link_resolves_in_the_installed_layout() {
    let root = repo_root();
    // The default repository, and the one a collective repository has. A link is checked
    // against the first unless the file carrying it is itself conditional.
    let default_tree = installed_tree(&root, &BTreeSet::new());
    let collective_tree = installed_tree(&root, &BTreeSet::from([COLLECTIVE]));
    assert!(
        default_tree.contains(".yidam/.vendor/prelude/GRAPH.md"),
        "the installed tree is not being built — mapping or walk is wrong"
    );

    // One tree per condition a row can carry, plus the unconditional default. A file is
    // checked against the repository that has it — see the `tree` binding in the walk.
    let trees: std::collections::BTreeMap<Option<&str>, BTreeSet<String>> = MAPPING
        .iter()
        .map(|e| e.when)
        .chain(std::iter::once(None))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|w| {
            let conds: BTreeSet<&str> = w.into_iter().collect();
            (w, installed_tree(&root, &conds))
        })
        .collect();

    let mut bad = Vec::new();
    let mut checked = 0usize;

    for e in MAPPING {
        if e.dst.is_none() {
            continue; // consumed; its links go nowhere because it goes nowhere
        }
        for entry in WalkDir::new(root.join(e.src))
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let s = path.to_string_lossy();
            if s.contains("/node_modules/") || s.contains("/.venv/") || s.contains("/target/") {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&root) else {
                continue;
            };
            let Some((owner, Some(dest))) = install_of(&rel.to_string_lossy()) else {
                continue;
            };
            // Resolved against the repository that has this file: one satisfying the
            // condition of the row that claims it, and no others. Picking the tree from the
            // walked row assumed there was only ever one condition, which stopped being true
            // the moment a second row carried one.
            let tree = trees.get(&owner.when).unwrap_or(&default_tree);
            let dest_dir = Path::new(&dest)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let conditional = conditional_lines(&rel.to_string_lossy(), &text);
            for (line, target) in targets(&text) {
                checked += 1;
                // A link inside a section bootstrap may delete is checked against the
                // repository that keeps the section.
                let tree = if conditional.contains(&line) {
                    &collective_tree
                } else {
                    tree
                };
                match resolve(&dest_dir, &target) {
                    Some(r) if tree.contains(&r) => {}
                    Some(r) => bad.push(format!(
                        "  {}:{} — `{}`\n      installs to {}\n      resolves to {} — absent",
                        rel.display(),
                        line,
                        target,
                        dest,
                        r
                    )),
                    None => bad.push(format!(
                        "  {}:{} — `{}` climbs above the repository root from {}",
                        rel.display(),
                        line,
                        target,
                        dest
                    )),
                }
            }
        }
    }

    assert!(checked > 0, "no relative links found — the scan is broken");
    assert!(
        bad.is_empty(),
        "{} link(s) do not resolve in the layout bootstrap installs:\n{}\n\n\
         These files are written for where they LAND, not where they sit. Count `../` from \
         the installed path, or use an absolute https:// URL to reach outside the derived \
         repository entirely — which is also the answer for a destination bootstrap only \
         installs conditionally, such as `.yidam/sangha/`.",
        bad.len(),
        bad.join("\n")
    );
}

/// Bootstrap's own `source → destination` table must be a subset of [`MAPPING`].
///
/// The mapping is only worth having if it is the one that gets installed. If bootstrap
/// grows a row and this does not, the test starts checking a layout nobody produces —
/// which is the failure mode the issue behind this test named specifically.
#[test]
fn the_mapping_agrees_with_bootstrap() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("yidam/prelude/skills/bootstrap.md")).unwrap();

    let mut rows = 0;
    for line in text.lines() {
        let Some((lhs, rhs)) = line.split_once('→') else {
            continue;
        };
        let src = lhs.trim();
        if !src.starts_with("sadhana/") {
            continue;
        }
        // `dest (overwrites yidam's)` — the parenthetical is commentary.
        let dst = rhs.split_whitespace().next().unwrap_or("");
        if dst.is_empty() {
            continue;
        }
        rows += 1;
        let found = MAPPING
            .iter()
            .any(|e| e.src == src && e.dst.map(|d| d == dst).unwrap_or(false));
        assert!(
            found,
            "bootstrap.md maps `{src}` → `{dst}`, and MAPPING in this test does not. \
             Update the table here, or the layout being checked is not the one installed."
        );
    }
    assert!(
        rows >= 6,
        "found only {rows} `→` rows in bootstrap.md — the parse is broken, not the doc"
    );
}

/// The conditional row must actually withhold something.
///
/// This is the guard that was missing. `sadhana/sangha/` sat in the mapping as an ordinary
/// row, so the tree the checker consulted always contained `.yidam/sangha/README.md` and
/// `GRAPH.md`'s relative link to it passed in a test whose own header called that link the
/// worked example of correctness. A single-elector repository — the default — never has the
/// file, and reported `unauthored-prose-link` against it on every run, forever.
#[test]
fn the_sangha_is_absent_by_default_and_present_when_elected() {
    let root = repo_root();
    let default_tree = installed_tree(&root, &BTreeSet::new());
    let collective_tree = installed_tree(&root, &BTreeSet::from([COLLECTIVE]));

    let target = resolve(".yidam/.vendor/prelude", "../../sangha/README.md").unwrap();
    assert_eq!(target, ".yidam/sangha/README.md");
    assert!(
        !default_tree.contains(&target),
        "single-elector is the default and installs no sangha; a tree that contains one \
         cannot fail the link that motivated this test"
    );
    assert!(
        collective_tree.contains(&target),
        "electing collective governance does install it — a condition that withholds \
         unconditionally is just a deletion"
    );
    // The condition is scoped to the sangha and nothing else moves with it.
    assert!(default_tree.contains(".yidam/corpus"));
    assert_eq!(
        collective_tree.difference(&default_tree).count(),
        collective_tree.len() - default_tree.len(),
        "the two trees differ only by additions"
    );
}

/// A sangha document may link within the sangha; its own presence proves the condition.
#[test]
fn a_conditional_file_is_checked_against_the_repository_that_has_it() {
    let sangha = MAPPING
        .iter()
        .find(|e| e.src == "sadhana/sangha")
        .expect("the sangha row");
    assert_eq!(sangha.when, Some(COLLECTIVE));

    let (entry, dest) = install_of("sadhana/sangha/PROTOCOL.md").expect("covered by the row");
    assert_eq!(dest.as_deref(), Some(".yidam/sangha/PROTOCOL.md"));
    assert_eq!(
        entry.when,
        Some(COLLECTIVE),
        "a file inherits its row's condition, which is what entitles it to link inward"
    );

    // Everything else is unconditional, including the prelude that must not link inward.
    let (prelude, _) = install_of("yidam/prelude/GRAPH.md").expect("covered by the row");
    assert_eq!(prelude.when, None);
}

/// A declared region must name a heading that exists and must actually cover links.
///
/// An anchor one character off matches nothing, excuses nothing, and passes — the region
/// silently stops existing and the links inside it go back to being checked against a tree
/// that does not have their targets. Assert in both directions: the heading is real, and
/// something is being excused by it.
#[test]
fn every_conditional_region_is_real_and_covers_something() {
    let root = repo_root();
    for (file, heading, condition) in CONDITIONAL_REGIONS {
        let text = std::fs::read_to_string(root.join(file)).expect(file);
        assert!(
            text.lines().any(|l| l.trim_end() == *heading),
            "{file} has no heading `{heading}` — the region matches nothing and excuses nothing"
        );
        let lines = conditional_lines(file, &text);
        assert!(
            !lines.is_empty(),
            "{file}: `{heading}` resolved to no lines"
        );

        let covered: Vec<_> = targets(&text)
            .into_iter()
            .filter(|(n, _)| lines.contains(n))
            .collect();
        assert!(
            !covered.is_empty(),
            "{file}: `{heading}` covers no links, so the exemption is doing nothing and \
             should be deleted rather than maintained"
        );
        assert_eq!(*condition, COLLECTIVE, "the only condition modelled so far");
    }
}

#[test]
fn resolve_is_lexical_and_bounded() {
    assert_eq!(
        resolve(".yidam/.vendor/prelude", "GRAPH.md").unwrap(),
        ".yidam/.vendor/prelude/GRAPH.md"
    );
    // Resolution is lexical and says nothing about whether the target exists — that the
    // sangha is conditional is the tree's business, not this function's.
    assert_eq!(
        resolve(".yidam/.vendor/prelude", "../../sangha/README.md").unwrap(),
        ".yidam/sangha/README.md"
    );
    assert_eq!(resolve("", "AGENTS.md").unwrap(), "AGENTS.md");
    assert_eq!(
        resolve(".yidam/sangha", "../.vendor/prelude/CONSTITUTION.md").unwrap(),
        ".yidam/.vendor/prelude/CONSTITUTION.md"
    );
    // Above the root is not a path.
    assert!(resolve("", "../outside.md").is_none());
}

/// The domain libraries are absent by default and present when a calculator names one.
///
/// `prelude/domains/` is fifteen libraries in three languages — about 320 of the ~540 files
/// the vendor step moves, and the majority of its bytes. Every derived repository received
/// all fifteen, and none could build any of them: there is no mise task, no workspace
/// membership, and no CI job, and `domain-parity` — the gate that keeps them honest — is
/// yidam's and does not travel. That is precisely the stale-fork outcome the vendor step
/// argues against, arriving through the one directory it allows.
///
/// The same shape as the sangha row, and the same guard for the same reason: a condition
/// that withholds unconditionally is a deletion, and one that withholds nothing is decoration.
#[test]
fn the_domain_libraries_are_absent_by_default_and_present_when_named() {
    let root = repo_root();
    let default_tree = installed_tree(&root, &BTreeSet::new());
    let with_domain = installed_tree(&root, &BTreeSet::from([DOMAIN_SELECTED]));

    let index = ".yidam/.vendor/prelude/domains/README.md";
    assert!(
        !default_tree.contains(index),
        "naming no prelude domain is the common case and must vendor none of them"
    );
    assert!(
        with_domain.contains(index),
        "a condition that withholds unconditionally is just a deletion"
    );

    // The prelude itself is unaffected — this withholds a subtree, not the layer above it.
    for kept in [
        ".yidam/.vendor/prelude/GRAPH.md",
        ".yidam/.vendor/prelude/guidelines/agent-conduct.md",
    ] {
        assert!(
            default_tree.contains(kept),
            "{kept} is doctrine and travels regardless"
        );
    }

    // And it withholds enough to be worth doing. The number is deliberately far below the
    // ~320 actually withheld: this asserts the row reaches the subtree, not a file count.
    let withheld = with_domain.difference(&default_tree).count();
    assert!(
        withheld > 100,
        "the domains row withholds only {withheld} path(s) — it is matching a corner of the \
         subtree rather than the subtree"
    );
    assert_eq!(
        with_domain.len() - default_tree.len(),
        withheld,
        "the two trees differ only by additions"
    );
}
