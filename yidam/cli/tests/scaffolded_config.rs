//! `sadhana/config.toml` must offer every key the binary reads, and no others.
//!
//! The file was read, documented and never written (#916): `config.rs` parses
//! `.yidam/config.toml`, `sadhana/root/README.md` tells a reader the `due` intervals come
//! from it, `.github/workflows/index.yml` hard-errors when it holds no `[vault.*]` — and
//! nothing put one in a derived repository. The template now ships, and a template that
//! ships is a template that rots: a field added to `DueConfig` next quarter is a key no
//! corpus scaffolded after today has ever seen offered to it.
//!
//! So this checks the correspondence in **both directions**, as set equality over dotted
//! paths:
//!
//! - a key the binary reads and the template does not offer is a key a corpus has to learn
//!   about from the source;
//! - a key the template offers and the binary does not read is a key someone will set and
//!   watch do nothing — which is the shape a typo takes here, since every line in the file
//!   is a comment and no parser sees it until the day it is uncommented.
//!
//! The typed half of that — right section, right spelling, right *type* — is
//! `config::the_scaffolded_config_deserializes`, a unit test, because `mod config` is
//! private and an integration test cannot name `YidamConfig`. This test is the structural
//! half: it reads the struct definitions as text and needs no access to them.
//!
//! [`every_sadhana_file_has_a_mapping_row`] guards the other way the file could go quiet:
//! a template nothing installs is a template nobody gets.

use std::collections::BTreeSet;

mod common;

use common::{repo_root, tracked_under, MAPPING};

/// The scaffolded template, repo-relative.
const TEMPLATE: &str = "sadhana/config.toml";

/// Config structs defined outside `src/config.rs`, by the file that defines them.
///
/// Not a list of every config struct — [`definition_of`] defaults to `src/config.rs`, so a
/// new section struct beside `YidamConfig` needs no entry here. Only a type that moved out
/// of it does, and [`struct_fields`] panics naming this constant when it cannot find one,
/// so the omission fails rather than silently skipping a subtree.
const ELSEWHERE: &[(&str, &str)] = &[
    ("VaultConfig", "src/vault/config.rs"),
    ("RemoteIndexConfig", "src/s3vectors/mod.rs"),
];

/// What a field's type means for the shape of the TOML underneath it.
#[derive(Debug)]
enum Shape {
    /// A scalar or a list: one key, nothing below it.
    Key,
    /// A sub-table whose keys are the named struct's fields.
    Table(String),
    /// A sub-table whose keys the corpus chooses — `[due.declined]`, `corpora`. The
    /// template must show the table and an example key; what the key *is* is not ours.
    OpenTable,
    /// A table of named instances, each shaped like the named struct — `[vault.<name>]`.
    Instances(String),
}

fn last_segment(s: &str) -> &str {
    s.rsplit("::").next().unwrap_or(s).trim()
}

/// The argument of `ctor<…>`, when `ctor` is the type's outermost constructor.
fn generic_arg<'a>(ty: &'a str, ctor: &str) -> Option<&'a str> {
    let head = format!("{ctor}<");
    let at = ty.find(&head)?;
    // `crate::vault::VaultConfig` may precede it; `Vec<Option<T>>` may not.
    let before = &ty[..at];
    if !before.is_empty() && !before.ends_with("::") {
        return None;
    }
    ty[at + head.len()..].trim_end().strip_suffix('>')
}

fn shape(ty: &str) -> Shape {
    let ty = ty.trim();
    if let Some(inner) = generic_arg(ty, "Option") {
        return shape(inner);
    }
    if let Some(args) = generic_arg(ty, "BTreeMap") {
        let value = args.split_once(',').map(|(_, v)| v).unwrap_or(args);
        let name = last_segment(value);
        return if name.ends_with("Config") {
            Shape::Instances(name.to_string())
        } else {
            Shape::OpenTable
        };
    }
    let name = last_segment(ty);
    if name.ends_with("Config") {
        Shape::Table(name.to_string())
    } else {
        Shape::Key
    }
}

fn definition_of(struct_name: &str) -> String {
    let file = ELSEWHERE
        .iter()
        .find(|(n, _)| *n == struct_name)
        .map(|(_, f)| *f)
        .unwrap_or("src/config.rs");
    let path = repo_root().join("yidam/cli").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// `(field, type)` for every `pub` field of `pub struct <name>`.
///
/// Read as text rather than through the type system on purpose: `mod config` is private, so
/// an integration test cannot reflect over it, and the alternative — a hand-written list of
/// the fields — is the guard list that rots into a hole this test exists to prevent.
fn struct_fields(struct_name: &str) -> Vec<(String, String)> {
    let text = definition_of(struct_name);
    let head = format!("pub struct {struct_name} {{");
    let start = text.find(&head).unwrap_or_else(|| {
        panic!(
            "no `{head}` in the file this test looks for it in. If the type moved, add it to \
             ELSEWHERE in this test — a config struct it cannot find is a subtree it does not \
             check."
        )
    });

    let mut fields = Vec::new();
    let mut closed = false;
    for line in text[start..].lines().skip(1) {
        if line == "}" {
            closed = true;
            break;
        }
        let Some(rest) = line.trim().strip_prefix("pub ") else {
            continue;
        };
        let Some((field, ty)) = rest.split_once(':') else {
            continue;
        };
        let field = field.trim();
        if field.is_empty()
            || !field
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        fields.push((
            field.to_string(),
            ty.trim().trim_end_matches(',').trim().to_string(),
        ));
    }

    assert!(
        closed,
        "`pub struct {struct_name}` has no closing brace at column zero — this test's parse \
         of it read to the end of the file and its field list means nothing."
    );
    assert!(
        !fields.is_empty(),
        "parsed no fields out of `pub struct {struct_name}`. Either the struct is empty or \
         this test's parse broke; both make the check below vacuous."
    );
    fields
}

/// The template as it reads once every offered line is uncommented.
///
/// Everything in the delivered file is a comment — that is the point of it, since an
/// interval compiled into a scaffold is one corpus's judgement arriving in another that
/// never agreed to it. So the only way to check that what is offered is well-formed is to
/// take the offer up.
fn uncommented(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let bare = line
            .strip_prefix('#')
            .map(str::trim_start)
            .unwrap_or(line)
            .trim();
        let is_header = bare.starts_with('[') && bare.ends_with(']') && !bare.contains(' ');
        let is_assignment = bare
            .split_once('=')
            .map(|(k, _)| {
                let k = k.trim();
                !k.is_empty()
                    && k.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                    })
            })
            .unwrap_or(false);
        if is_header || is_assignment {
            out.push_str(bare);
            out.push('\n');
        }
    }
    out
}

/// Walk the template against a struct, collecting what each side has that the other does not.
fn compare(
    prefix: &str,
    struct_name: &str,
    table: &toml::Table,
    missing: &mut Vec<String>,
    unknown: &mut Vec<String>,
) {
    let fields = struct_fields(struct_name);

    for (field, ty) in &fields {
        let path = format!("{prefix}{field}");
        let Some(value) = table.get(field) else {
            missing.push(path);
            continue;
        };
        match shape(ty) {
            Shape::Key => {}
            Shape::Table(inner) => {
                let Some(sub) = value.as_table() else {
                    missing.push(format!("{path} (offered as a value; it is a table)"));
                    continue;
                };
                compare(&format!("{path}."), &inner, sub, missing, unknown);
            }
            Shape::OpenTable => {
                let empty = value.as_table().map(|t| t.is_empty()).unwrap_or(true);
                if empty {
                    missing.push(format!("{path}.<a key of this corpus's choosing>"));
                }
            }
            Shape::Instances(inner) => {
                let Some(sub) = value.as_table() else {
                    missing.push(format!(
                        "{path} (offered as a value; it is a table of tables)"
                    ));
                    continue;
                };
                if sub.is_empty() {
                    missing.push(format!("{path}.<name>"));
                }
                for (name, instance) in sub {
                    let Some(instance) = instance.as_table() else {
                        unknown.push(format!("{path}.{name} (not a table)"));
                        continue;
                    };
                    compare(&format!("{path}.*."), &inner, instance, missing, unknown);
                }
            }
        }
    }

    let known: BTreeSet<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
    for key in table.keys() {
        if !known.contains(key.as_str()) {
            unknown.push(format!("{prefix}{key}"));
        }
    }
}

#[test]
fn the_scaffolded_config_offers_every_key_the_binary_reads() {
    let path = repo_root().join(TEMPLATE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

    let live = uncommented(&text);
    let parsed: toml::Table = toml::from_str(&live).unwrap_or_else(|e| {
        panic!(
            "{TEMPLATE} does not parse once uncommented: {e}\n\n\
             Every key in it is delivered commented out, so nothing catches a malformed \
             line until a corpus takes the offer up — which is exactly the day it is least \
             welcome.\n\n--- uncommented ---\n{live}"
        )
    });

    assert!(
        parsed.len() > 1,
        "uncommenting {TEMPLATE} yielded {} top-level key(s). This test's uncommenter has \
         stopped recognising the file's shape, and everything below it would pass by \
         finding nothing.",
        parsed.len()
    );

    let mut missing = Vec::new();
    let mut unknown = Vec::new();
    compare("", "YidamConfig", &parsed, &mut missing, &mut unknown);

    assert!(
        missing.is_empty(),
        "the binary reads these and {TEMPLATE} does not offer them:\n  {}\n\n\
         A corpus scaffolded today never meets the key, and learns it exists from the \
         source or not at all. Add each one commented out, with a line saying what absent \
         means.",
        missing.join("\n  ")
    );
    assert!(
        unknown.is_empty(),
        "{TEMPLATE} offers these and the binary reads none of them:\n  {}\n\n\
         Someone will uncomment one and watch it do nothing. A misspelling is the usual \
         cause, and a commented misspelling has no parser to catch it.",
        unknown.join("\n  ")
    );
}

#[test]
fn every_sadhana_file_has_a_mapping_row() {
    let files = tracked_under(&repo_root(), "sadhana/");
    assert!(
        files.len() > 10,
        "{} tracked file(s) under sadhana/. That is not the scaffold, and this test would \
         pass by checking nothing.",
        files.len()
    );

    for file in &files {
        let covered = MAPPING.iter().any(|e| {
            e.src == file.as_str()
                || (file.starts_with(e.src) && file.as_bytes()[e.src.len()] == b'/')
        });
        assert!(
            covered,
            "`{file}` is tracked under sadhana/ and no MAPPING row installs or consumes it. \
             Bootstrap's scaffold is built from that table, so a template with no row is a \
             file that ships here and reaches no derived repository — and nothing else goes \
             red, because the derived-repo test constructs the tree it expects."
        );
    }
}
