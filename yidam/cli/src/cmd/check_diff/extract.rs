//! Type names out of a diff, and the ontology's vocabulary out of `.ont.yml`.
//!
//! Both halves are pure: text in, names out. The git subprocess and the repository live in
//! [`super`], so everything that can be wrong about *reading a declaration* is testable
//! against a string literal.
//!
//! # Line-based, and staying that way
//!
//! No Rust parser. The light `reports` build carries `regex`, `walkdir`, `serde` and
//! `pulldown-cmark` and no native libraries; adding `syn` to read names out of a diff would
//! buy precision this does not need at a cost `--features index` already shows the shape of.
//! It is the idiom the repository already uses for this exact job — `rename.rs`'s
//! `target_on` reads a link target out of a line rather than parsing YAML, for the same
//! reason.

use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

use crate::cmd::lint::checks::Class;

/// One type declaration, and where the diff introduced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    /// As written: `AgendaItem`.
    pub name: String,
    /// Repo-relative path in the *new* tree.
    pub file: String,
    /// Line in the new file. Best-effort and output-only — nothing keys on it, for the same
    /// reason `report::Span` is never part of a violation's identity.
    pub line: usize,
}

/// `struct` and `enum`, and deliberately not `trait` or `type`.
///
/// A trait names a capability and a type alias names a spelling; neither is a thing the
/// domain has. Measured across the three instrumented repositories, widening to both adds
/// 14, 3 and 13 names and not one of them is a concept — `Connector`, `Result`, `BoxFut`.
/// The narrow reading is also what the RFC's counts were taken against.
fn declaration() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^[ \t]*(?:pub[ \t]*(?:\([^)]*\))?[ \t]+)?(?:struct|enum)[ \t]+([A-Za-z_][A-Za-z0-9_]*)",
        )
        .expect("the declaration pattern compiles")
    })
}

/// Every type declaration in a file of Rust, by line.
///
/// The tree-side reading of [`declaration`], and it exists so there is exactly one answer
/// to *what is a type declaration* in this repository. `check-diff` asks the question of a
/// hunk and `unimplemented-class` asks it of a whole file; if the two ever disagreed, a
/// corpus could declare an implementation that one of them could see and the other could
/// not, and the class contract would be enforced against a different tree than the one the
/// author was asked about.
///
/// No `removed` bookkeeping and no deduplication, because a tree has neither: what is here
/// is here. The caller indexes by name.
pub fn declared_in(text: &str) -> Vec<(String, usize)> {
    let re = declaration();
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| re.captures(l).map(|c| (c[1].to_string(), i + 1)))
        .collect()
}

/// The `+N` of a `@@ -a,b +c,d @@` header — where the hunk's added lines begin.
fn hunk_start(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("@@ ")?;
    let plus = rest.split(' ').find(|s| s.starts_with('+'))?;
    plus[1..].split(',').next()?.parse().ok()
}

/// Every type declaration a unified diff introduces, deduplicated by name.
///
/// Three properties, each of which is the difference between a question worth asking and a
/// permanently non-empty report:
///
/// **A name the diff also removes is not introduced.** A struct moved between files appears
/// as an addition and a deletion; so does one that was renamed back. Both would otherwise be
/// asked about at every reshuffle, and neither is new.
///
/// **`.rs` only.** Measured, and not hypothetical: `crates/README.md` in one repository
/// contains the words *"rather struct rather"* in prose, which this pattern matches happily.
/// A markdown file is not Rust and its sentences are not declarations.
///
/// **One question per name.** A concept is asked about once, at the commit that introduces
/// it, wherever it first appears — which is the property that makes diff-scoping work at
/// all. Five crates each defining `Error` is one question, not five.
pub fn introduced(diff: &str) -> Vec<Decl> {
    let re = declaration();
    let mut added: Vec<Decl> = Vec::new();
    let mut removed: HashSet<String> = HashSet::new();
    let mut file = String::new();
    let mut rust = false;
    let mut line = 0usize;

    for raw in diff.lines() {
        // The two file headers are tested before the `+`/`-` arms below, and must stay that
        // way: `+++ b/x.rs` starts with `+`.
        if let Some(path) = raw.strip_prefix("+++ ") {
            file = path.strip_prefix("b/").unwrap_or(path).to_string();
            rust = file.ends_with(".rs");
            continue;
        }
        if raw.starts_with("--- ") {
            continue;
        }
        if let Some(start) = hunk_start(raw) {
            line = start;
            continue;
        }
        // `\ No newline at end of file` — a note about the line above, not a line.
        if raw.starts_with('\\') {
            continue;
        }
        match raw.as_bytes().first() {
            Some(b'+') => {
                if rust {
                    if let Some(c) = re.captures(&raw[1..]) {
                        added.push(Decl {
                            name: c[1].to_string(),
                            file: file.clone(),
                            line,
                        });
                    }
                }
                line += 1;
            }
            Some(b'-') => {
                if rust {
                    if let Some(c) = re.captures(&raw[1..]) {
                        removed.insert(c[1].to_string());
                    }
                }
            }
            // Context, and anything else this walker does not model. Advancing on the
            // unknown keeps a line number roughly right rather than silently drifting to
            // zero, and a line number is output-only anyway.
            _ => line += 1,
        }
    }

    added.retain(|d| !removed.contains(&d.name));
    let mut seen: HashSet<String> = HashSet::new();
    added.retain(|d| seen.insert(d.name.clone()));
    added.sort_by(|a, b| (&a.file, a.line, &a.name).cmp(&(&b.file, b.line, &b.name)));
    added
}

/// `AppointingAuthority` → `appointing-authority`, `claim_tag` → `claim-tag`.
///
/// The one join between two naming conventions that never meet anywhere else: Rust spells a
/// type in `UpperCamel`, an ontology spells a class in `kebab-case` and a property in either
/// `kebab` or `snake`. Acronyms break before the last capital — `HTTPServer` is
/// `http-server` and not `h-t-t-p-server` — because the alternative matches nothing.
pub fn kebab(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in chars.iter().enumerate() {
        if *c == '_' || *c == '-' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            continue;
        }
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = prev.is_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_uppercase() && next.is_some_and(char::is_lowercase));
            if boundary && !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Every name the ontology declares, kebab-cased: classes, properties, relationships.
///
/// All three, and not classes alone. A domain concept can perfectly well arrive in the
/// ontology as a property of something else or as the name of a relationship, and a check
/// that read only class names would ask about `sponsorship` in a corpus that declares
/// `sponsored-by`. Across the three instrumented repositories the wider vocabulary is 152,
/// 84 and 189 names against 15, 12 and 18 classes.
///
/// An edge's `target` is not read here: it names a class, which is counted from that class's
/// own declaration, and a target naming a class that does not exist is `edge-target-class`'s
/// finding rather than this one's.
pub fn declared(classes: &[Class]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for c in classes {
        if !c.name.is_empty() {
            out.insert(kebab(&c.name));
        }
        for p in &c.properties {
            if !p.name.is_empty() {
                out.insert(kebab(&p.name));
            }
        }
        for e in &c.edges {
            if !e.relationship.is_empty() {
                out.insert(kebab(&e.relationship));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(diff: &str) -> Vec<String> {
        introduced(diff).into_iter().map(|d| d.name).collect()
    }

    #[test]
    fn a_struct_and_an_enum_are_read_off_an_added_line() {
        let diff = "\
--- a/crates/a/src/lib.rs
+++ b/crates/a/src/lib.rs
@@ -0,0 +1,3 @@
+pub struct AgendaItem;
+enum Chamber { House, Senate }
+pub(crate) struct Canvass {
";
        assert_eq!(names(diff), ["AgendaItem", "Chamber", "Canvass"]);
    }

    /// A trait names a capability and an alias names a spelling. Neither is a concept.
    #[test]
    fn a_trait_and_a_type_alias_are_not_declarations() {
        let diff = "\
+++ b/crates/a/src/lib.rs
@@ -0,0 +1,2 @@
+pub trait Connector {}
+pub type Bill = u32;
";
        assert!(names(diff).is_empty());
    }

    /// The measured case, and the reason this filter exists: `crates/README.md` in one
    /// repository contains the words "rather struct rather" in a sentence.
    #[test]
    fn prose_in_a_markdown_file_is_not_a_declaration() {
        let diff = "\
+++ b/crates/README.md
@@ -0,0 +1,1 @@
+A domain crate is a struct Rather than a service.
";
        assert!(names(diff).is_empty());
    }

    /// A moved or renamed-back struct is not a new concept, and would otherwise be asked
    /// about at every reshuffle.
    #[test]
    fn a_name_the_diff_also_removes_is_not_introduced() {
        let diff = "\
+++ b/crates/a/src/new.rs
@@ -0,0 +1,1 @@
+pub struct Canvass;
--- a/crates/a/src/old.rs
@@ -1 +0,0 @@
-pub struct Canvass;
";
        assert!(names(diff).is_empty());
    }

    #[test]
    fn one_name_is_asked_about_once_however_many_crates_define_it() {
        let diff = "\
+++ b/crates/a/src/lib.rs
@@ -0,0 +1,1 @@
+pub struct Error;
+++ b/crates/b/src/lib.rs
@@ -0,0 +1,1 @@
+pub struct Error;
";
        assert_eq!(names(diff), ["Error"]);
    }

    /// `+++ b/…` starts with `+`, so header handling has to precede the added-line arm.
    #[test]
    fn a_file_header_is_not_an_added_line() {
        let diff = "\
+++ b/crates/a/src/lib.rs
@@ -0,0 +1,1 @@
+pub struct Amendment;
";
        let d = introduced(diff);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].file, "crates/a/src/lib.rs");
    }

    #[test]
    fn a_line_number_counts_context_and_additions_and_not_deletions() {
        let diff = "\
+++ b/crates/a/src/lib.rs
@@ -10,4 +10,5 @@ impl Bill {
 fn a() {}
-fn gone() {}
 fn b() {}
+pub struct Sponsorship;
";
        let d = introduced(diff);
        assert_eq!(d[0].line, 12, "10 and 11 are context, 12 is the addition");
    }

    #[test]
    fn a_deleted_file_contributes_nothing() {
        let diff = "\
--- a/crates/a/src/lib.rs
+++ /dev/null
@@ -1,1 +0,0 @@
-pub struct Chamber;
";
        assert!(names(diff).is_empty());
    }

    /// The two directions must agree about what a declaration is. A hunk of added lines and
    /// the file it lands in hold the same declarations, and a corpus could otherwise declare
    /// an implementation that one reader sees and the other does not.
    #[test]
    fn a_tree_and_a_diff_read_the_same_declarations() {
        let file = "\
pub struct AgendaItem;
enum Chamber { House, Senate }
pub trait Connector {}
pub type Bill = u32;
pub(crate) struct Canvass {
";
        let diff = format!(
            "+++ b/crates/a/src/lib.rs\n@@ -0,0 +1,5 @@\n{}",
            file.lines().map(|l| format!("+{l}\n")).collect::<String>()
        );
        let from_tree: Vec<String> = declared_in(file).into_iter().map(|(n, _)| n).collect();
        assert_eq!(from_tree, ["AgendaItem", "Chamber", "Canvass"]);
        assert_eq!(from_tree, names(&diff));
    }

    #[test]
    fn a_tree_declaration_carries_its_one_based_line() {
        let d = declared_in("// a comment\n\npub struct Amendment;\n");
        assert_eq!(d, [("Amendment".to_string(), 3)]);
    }

    #[test]
    fn camel_case_becomes_kebab() {
        assert_eq!(kebab("AppointingAuthority"), "appointing-authority");
        assert_eq!(kebab("BillVotes"), "bill-votes");
        assert_eq!(kebab("Bill"), "bill");
        assert_eq!(kebab("claim_tag"), "claim-tag");
        assert_eq!(kebab("session-law"), "session-law");
    }

    /// Acronyms break before the last capital, because the alternative matches nothing.
    #[test]
    fn an_acronym_does_not_become_one_letter_per_word() {
        assert_eq!(kebab("HTTPServer"), "http-server");
        assert_eq!(kebab("ZCTA"), "zcta");
    }

    #[test]
    fn a_digit_is_a_word_boundary_before_a_capital_and_not_after_one() {
        assert_eq!(kebab("Zip5"), "zip5");
        assert_eq!(kebab("Zip5Reading"), "zip5-reading");
    }

    #[test]
    fn a_leading_underscore_does_not_produce_a_leading_dash() {
        assert_eq!(kebab("_Private"), "private");
        assert_eq!(kebab(""), "");
    }
}
