use anyhow::Result;
use std::collections::HashMap;

use crate::parse::parse_samudaya_seed_reporting;
use crate::paths::samudaya_dir;

/// The `kind` values a seed file may declare.
///
/// `pub` so `tests/samudaya_examples.rs` can hold the domain seed sets under
/// `samudaya/examples/` to this list. That directory is skipped by the walk below — which is
/// what keeps those sets from being live seeds of this repository — so nothing else would
/// check them, and a second copy of these four names is how the check and the command would
/// come to disagree.
pub const VALID_KINDS: &[&str] = &["axiom", "hint", "constraint", "augmentation"];

fn extract_body(text: &str) -> &str {
    let body = text.trim_start();
    let Some(rest) = body.strip_prefix("---\n") else {
        return text;
    };
    let Some(end) = rest.find("\n---") else {
        return text;
    };
    &rest[end + 4..]
}

fn has_title(text: &str) -> bool {
    extract_body(text)
        .lines()
        .any(|l| l.starts_with("# ") && l.len() > 2)
}

/// What a constitutional augmentation at `path` lacks to be checked once it is sealed.
///
/// `yidam lint` evaluates `<stem>.rego` from the genesis commit and runs `<stem>_test.rego`
/// beside it (RFC-0047). Without the rule the article is sealed as prose: it still binds, and
/// nothing checks it. Without the cases the rule is evaluated and reported unproven on every
/// lint. Both are warnings, because a norm with no machine form is still a norm.
fn unruled(path: &std::path::Path, rel: &str) -> Vec<String> {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let rule = path.with_file_name(format!("{stem}.rego"));
    let cases = path.with_file_name(format!("{stem}_test.rego"));
    if !rule.is_file() {
        return vec![format!(
            "{rel}: constitutional augmentation has no {stem}.rego beside it — \
             it will be sealed as prose, and nothing will check it"
        )];
    }
    if !cases.is_file() {
        return vec![format!(
            "{rel}: {stem}.rego has no {stem}_test.rego beside it — \
             `yidam lint` will report the article unproven"
        )];
    }
    Vec::new()
}

pub fn samudaya_audit(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let samudaya = samudaya_dir(&root);

    if !samudaya.exists() {
        println!("samudaya/ not found at {}.", samudaya.display());
        println!(
            "This repository has no samudaya seeds (or they were already consumed at genesis)."
        );
        return Ok(());
    }

    // Walk samudaya/ for .md files, skipping examples/ and README.md
    let mut seed_files: Vec<_> = walkdir::WalkDir::new(&samudaya)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_str() != Some("README.md")
                && !e.path().components().any(|c| c.as_os_str() == "examples")
        })
        .map(|e| e.path().to_owned())
        .collect();
    seed_files.sort();

    if seed_files.is_empty() {
        println!("samudaya/ contains no seed files (only examples/ templates).");
        println!("A derived repo will inherit 0 seeds.");
        return Ok(());
    }

    let mut issues: Vec<String> = Vec::new();
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    let mut constitutional_count = 0usize;

    for path in &seed_files {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let (seed, malformed) = parse_samudaya_seed_reporting(&text);
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Said first, and said instead of the field complaints below (#1081). A seed whose
        // header the parser rejected declares nothing, so every `missing 'x:'` line after this
        // one is about a key that may well be on line 2 — the wrong sentence about the right
        // file, which sends the author to the wrong place. `_unknown` still counts the file:
        // the summary's job is to say how many seeds a derived repo inherits, and this is one.
        if let Some(why) = &malformed {
            issues.push(format!("{rel}: frontmatter does not parse: {why}"));
            *kind_counts.entry("_unknown".to_string()).or_insert(0) += 1;
            continue;
        }

        let kind = match &seed.kind {
            None => {
                issues.push(format!("{rel}: missing 'kind:' field in frontmatter"));
                "_unknown".to_string()
            }
            Some(k) => {
                if !VALID_KINDS.contains(&k.as_str()) {
                    issues.push(format!(
                        "{rel}: unknown kind '{k}' — expected one of: {}",
                        VALID_KINDS.join(", ")
                    ));
                }
                *kind_counts.entry(k.clone()).or_insert(0) += 1;
                k.clone()
            }
        };

        if !has_title(&text) {
            issues.push(format!("{rel}: missing title (H1 heading in body)"));
        }

        if kind == "augmentation" {
            match seed.constitutional {
                None => issues.push(format!(
                    "{rel}: augmentation missing 'constitutional: true|false' — \
                     set true to seal it into .yidam/constitution/ at genesis, false otherwise"
                )),
                Some(true) => {
                    constitutional_count += 1;
                    issues.extend(unruled(path, &rel));
                    println!(
                        "[review] {rel}: constitutional augmentation — \
                         will be sealed into the derived repo's .yidam/constitution/ at genesis"
                    );
                    println!(
                        "         Verify it does not contradict Articles I–VI \
                         (prelude/CONSTITUTION.md)"
                    );
                }
                Some(false) => {}
            }
        }
    }

    // Print issues
    for issue in &issues {
        eprintln!("[warn]  {issue}");
    }

    // Summary
    let n = seed_files.len();
    let m = kind_counts.len();

    println!();
    println!("Seeds found: {n} (across {m} kind(s))");
    let mut kinds: Vec<_> = kind_counts.iter().collect();
    kinds.sort_by(|a, b| a.0.cmp(b.0));
    for (kind, count) in &kinds {
        println!("  {kind}: {count}");
    }
    if constitutional_count > 0 {
        println!(
            "  ({constitutional_count} constitutional augmentation(s) flagged for review above)"
        );
    }
    println!();
    println!("A derived repo will inherit {n} seeds covering {m} kind(s).");

    if !issues.is_empty() {
        let ni = issues.len();
        eprintln!();
        eprintln!("{ni} issue(s) found in samudaya/ seeds.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constitutional_augmentation_is_asked_for_its_rule_and_its_cases() {
        let dir = tempfile::tempdir().unwrap();
        let seed = dir.path().join("augmentation-x.md");
        std::fs::write(&seed, "").unwrap();

        let got = unruled(&seed, "samudaya/augmentation-x.md");
        assert_eq!(got.len(), 1);
        assert!(
            got[0].contains("no augmentation-x.rego beside it"),
            "{got:?}"
        );

        std::fs::write(dir.path().join("augmentation-x.rego"), "").unwrap();
        let got = unruled(&seed, "samudaya/augmentation-x.md");
        assert_eq!(got.len(), 1);
        assert!(got[0].contains("no augmentation-x_test.rego"), "{got:?}");

        std::fs::write(dir.path().join("augmentation-x_test.rego"), "").unwrap();
        assert!(unruled(&seed, "samudaya/augmentation-x.md").is_empty());
    }

    #[test]
    fn has_title_detects_h1() {
        assert!(has_title("---\nkind: axiom\n---\n# My Title\nBody text.\n"));
        assert!(!has_title("---\nkind: axiom\n---\nNo heading here.\n"));
        assert!(!has_title("---\nkind: axiom\n---\n## Only h2\n"));
    }

    #[test]
    fn extract_body_strips_frontmatter() {
        let text = "---\nkind: axiom\n---\n# Title\nBody.\n";
        let body = extract_body(text);
        assert!(body.contains("# Title"));
        assert!(!body.contains("kind:"));
    }

    #[test]
    fn parse_seed_constitutional_field() {
        let text = "---\nkind: augmentation\nconstitutional: true\n---\n# Aug\nContent.\n";
        let (seed, malformed) = parse_samudaya_seed_reporting(text);
        assert_eq!(seed.kind.as_deref(), Some("augmentation"));
        assert_eq!(seed.constitutional, Some(true));
        assert_eq!(malformed, None);
    }

    /// The #1081 arm, asserted as the *sentence* rather than as a count: the old reader lost the
    /// header and the audit then said `missing 'kind:' field` about a file whose `kind:` is on
    /// line 2. A person reading that goes and looks at a line that is already correct.
    #[test]
    fn a_seed_whose_header_does_not_parse_says_that_and_not_which_field_is_missing() {
        // `kind:` and `constitutional:` are both there. The `title:` value is an unclosed quote.
        let text = "---\nkind: augmentation\nconstitutional: true\ntitle: \"unclosed\n---\n# Aug\n";
        let (seed, malformed) = parse_samudaya_seed_reporting(text);
        assert!(malformed.is_some(), "the fixture has to be a parse failure");
        assert_eq!(
            seed.kind, None,
            "and the record it falls back to declares nothing"
        );
        assert_eq!(seed.constitutional, None);
    }

    /// A document with no frontmatter at all has contradicted nothing, and must not be reported
    /// as unreadable — the distinction `parse_header` is written around.
    #[test]
    fn a_seed_with_no_frontmatter_is_not_a_parse_failure() {
        for text in ["", "# Just a title\n", "Body with no header.\n"] {
            assert_eq!(parse_samudaya_seed_reporting(text).1, None, "{text:?}");
        }
    }
}
