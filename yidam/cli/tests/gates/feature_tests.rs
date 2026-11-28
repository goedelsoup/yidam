//! The tests a non-default feature set adds are run on the pull request (#1639).
//!
//! `check-features` clippies the two non-default builds, and before #1639 nothing on a pull
//! request ran a test under either. `cli-full` runs the tests of one of them, after the merge.
//! So a test that only exists, or is only un-ignored, in one of those builds went red on
//! `main`, which is where #922's three outages were found too.
//!
//! The pull request does not run all of each build's tests. Compiling the test binaries is most
//! of a leg's cost, and that part cannot be avoided. Running them is about 90s a leg on the CI
//! runner, and almost all of those tests run the same code they run in the default build, which
//! `ci (cli)` has already tested. What a leg adds is the tests whose own attributes change with
//! the features: a `#[cfg(...)]` or `#[cfg_attr(..., ignore)]` on the test or on a module it is
//! in. `test-features-<leg>` in mise.toml runs exactly those, by name.
//!
//! **The names are derived, not listed.** This file walks the module tree of each test binary
//! from its root, reads the attributes on every module and test, and evaluates them under the
//! default set and under each leg. A test that runs under a leg and does not run under the
//! default set is selected. The result is a nextest filterset, committed at
//! `tests/goldens/feature-tests/<leg>.filter`, and the first test below fails when the
//! committed file is not what the attributes derive. `UPDATE_GOLDENS=1` rewrites it.
//!
//! The filter is committed rather than computed in CI so that the selection is in the diff,
//! where a reviewer reads it. The leg's task also counts what nextest lists against the
//! number of names in the filter, so a name this walk gets wrong fails the CI job instead of
//! matching nothing.
//!
//! What this does **not** see is a test whose behaviour changes with a feature without an
//! attribute saying so: a `cfg!(...)` in its body, or a CLI test whose binary was built with
//! different features. Those run only under `cli-full`, as before. It also refuses, rather than
//! skips, what it cannot read. It panics on a `cfg` predicate it does not know, on a `#[test]`
//! it did not reach through the module tree, and on a file with a `#[test]` the walk never
//! opened.
//!
//! The third clippy leg, `--no-default-features --features reports`, has no test leg. Its
//! tests fail on purpose (#482; the note on `reports` in yidam/cli/Cargo.toml).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::common;

/// The non-default builds a pull request tests, by the name their task and filter carry.
///
/// The flags are what `check-features` passes, with `$CLI_FEATURES_WITHOUT_VECTOR_READ` as
/// mise.toml spells it. [`every_compiled_leg_is_tested_or_excused`] holds this against
/// `check-features`, so a clippy leg added there without a test leg fails.
const LEGS: &[(&str, &str)] = &[
    (
        "without-vector-read",
        "--no-default-features --features $CLI_FEATURES_WITHOUT_VECTOR_READ",
    ),
    ("all-features", "--all-features"),
];

/// The one compiled leg without a test leg, and why.
const UNTESTED_LEGS: &[(&str, &str)] = &[(
    "--no-default-features --features reports",
    "its tests fail on purpose (#482): the note on `reports` in yidam/cli/Cargo.toml",
)];

/// The test binaries, as nextest names them, and the file each is rooted at.
const BINARIES: &[(&str, &str)] = &[
    ("yidam", "src/lib.rs"),
    ("yidam::bin/yidam", "src/main.rs"),
    ("yidam::suite", "tests/suite/main.rs"),
    ("yidam::gates", "tests/gates/main.rs"),
];

/// The platform `cfg` predicates are evaluated for: the CI runner's.
const TARGET_OS: &str = "linux";
const TARGET_ARCH: &str = "x86_64";

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()))
}

fn rel(path: &Path) -> String {
    path.strip_prefix(crate_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn golden(leg: &str) -> PathBuf {
    crate_root().join(format!("tests/goldens/feature-tests/{leg}.filter"))
}

// ── reading Rust ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Lit,
    Punct(char),
}

#[derive(Clone, Debug)]
enum Tree {
    Tok(Tok),
    Group(char, Vec<Tree>),
}

/// `r"…"`, `r#"…"#`, `br"…"` and `cr"…"` starting at `i`: its length and contents.
fn raw_string(c: &[char], i: usize) -> Option<(usize, String)> {
    let mut j = i;
    if matches!(c.get(j), Some('b' | 'c')) {
        j += 1;
    }
    if c.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while c.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if c.get(j) != Some(&'"') {
        return None;
    }
    let start = j + 1;
    let mut k = start;
    while k < c.len() {
        if c[k] == '"' && (1..=hashes).all(|h| c.get(k + h) == Some(&'#')) {
            return Some((k + 1 + hashes - i, c[start..k].iter().collect()));
        }
        k += 1;
    }
    None
}

/// Tokens, with comments dropped and literals kept whole, so that a `#[cfg(...)]` in a doc
/// comment or a `{` in a string is not read as code.
fn lex(src: &str) -> Vec<Tok> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && next == Some('*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if let Some((len, body)) = raw_string(&c, i) {
            out.push(Tok::Str(body));
            i += len;
        } else if ch == '"' || (matches!(ch, 'b' | 'c') && next == Some('"')) {
            let mut j = if ch == '"' { i + 1 } else { i + 2 };
            let mut body = String::new();
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' {
                    body.push(c[j]);
                    j += 1;
                }
                if let Some(&d) = c.get(j) {
                    body.push(d);
                }
                j += 1;
            }
            out.push(Tok::Str(body));
            i = j + 1;
        } else if ch == '\'' || (ch == 'b' && next == Some('\'')) {
            let q = if ch == '\'' { i } else { i + 1 };
            if c.get(q + 1) == Some(&'\\') {
                let mut j = q + 3;
                while j < c.len() && c[j] != '\'' {
                    j += 1;
                }
                out.push(Tok::Lit);
                i = j + 1;
            } else if c.get(q + 2) == Some(&'\'') {
                out.push(Tok::Lit);
                i = q + 3;
            } else {
                // A lifetime or a label.
                out.push(Tok::Punct('\''));
                i = q + 1;
            }
        } else if ch.is_alphabetic() || ch == '_' {
            let mut j = i;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_') {
                j += 1;
            }
            out.push(Tok::Ident(c[i..j].iter().collect()));
            i = j;
        } else if ch.is_ascii_digit() {
            let mut j = i;
            while j < c.len()
                && (c[j].is_alphanumeric()
                    || c[j] == '_'
                    || (c[j] == '.' && c.get(j + 1).is_some_and(char::is_ascii_digit)))
            {
                j += 1;
            }
            out.push(Tok::Lit);
            i = j;
        } else {
            out.push(Tok::Punct(ch));
            i += 1;
        }
    }
    out
}

fn trees(toks: Vec<Tok>, file: &str) -> Vec<Tree> {
    let mut stack: Vec<(char, Vec<Tree>)> = vec![(' ', Vec::new())];
    for tok in toks {
        match tok {
            Tok::Punct(open @ ('(' | '[' | '{')) => stack.push((open, Vec::new())),
            Tok::Punct(')' | ']' | '}') => {
                let (open, body) = stack.pop().expect("a group was opened");
                stack
                    .last_mut()
                    .unwrap_or_else(|| panic!("{file}: a closing delimiter with nothing open"))
                    .1
                    .push(Tree::Group(open, body));
            }
            other => stack
                .last_mut()
                .expect("the root group is never popped")
                .1
                .push(Tree::Tok(other)),
        }
    }
    assert_eq!(
        stack.len(),
        1,
        "{file}: delimiters do not balance as this reader lexes it. Teach `lex` whatever \
         literal it misread; skipping the file would drop its tests from the selection."
    );
    stack.pop().map(|(_, body)| body).unwrap_or_default()
}

fn ident(tree: &Tree) -> Option<&str> {
    match tree {
        Tree::Tok(Tok::Ident(s)) => Some(s),
        _ => None,
    }
}

fn is_punct(tree: &Tree, ch: char) -> bool {
    matches!(tree, Tree::Tok(Tok::Punct(p)) if *p == ch)
}

fn split_commas(trees: &[Tree]) -> Vec<&[Tree]> {
    trees
        .split(|t| is_punct(t, ','))
        .filter(|part| !part.is_empty())
        .collect()
}

/// An attribute's path (`cfg`, `tokio::test`) and what follows it.
fn attr_path(attr: &[Tree]) -> (String, &[Tree]) {
    let mut path = String::new();
    let mut i = 0;
    while i < attr.len() {
        if let Some(s) = ident(&attr[i]) {
            path.push_str(s);
            i += 1;
        } else if is_punct(&attr[i], ':') && attr.get(i + 1).is_some_and(|t| is_punct(t, ':')) {
            path.push_str("::");
            i += 2;
        } else {
            break;
        }
    }
    (path, &attr[i..])
}

enum Kind {
    Mod {
        name: String,
        body: Option<Vec<Tree>>,
    },
    Fn {
        name: String,
    },
    Other,
}

struct Item {
    attrs: Vec<Vec<Tree>>,
    kind: Kind,
}

const MODIFIERS: &[&str] = &[
    "pub", "crate", "async", "unsafe", "extern", "default", "safe",
];

/// The items of one module body, and the inner attributes it opens with.
fn items(trees: &[Tree]) -> (Vec<Vec<Tree>>, Vec<Item>) {
    let mut inner = Vec::new();
    let mut out = Vec::new();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < trees.len() {
        if is_punct(&trees[i], '#') {
            let bang = trees.get(i + 1).is_some_and(|t| is_punct(t, '!'));
            let at = if bang { i + 2 } else { i + 1 };
            if let Some(Tree::Group('[', body)) = trees.get(at) {
                if bang {
                    inner.push(body.clone());
                } else {
                    attrs.push(body.clone());
                }
                i = at + 1;
                continue;
            }
        }

        // One item: up to its `;`, or up to its body for anything that ends in one.
        let mut keyword: Option<&str> = None;
        let mut name: Option<String> = None;
        let mut body: Option<Vec<Tree>> = None;
        let mut j = i;
        while j < trees.len() {
            let tree = &trees[j];
            j += 1;
            if is_punct(tree, ';') {
                break;
            }
            if let Tree::Group('{', inside) = tree {
                if matches!(keyword, Some("const" | "static" | "use" | "type" | "crate")) {
                    continue;
                }
                body = Some(inside.clone());
                break;
            }
            let Some(word) = ident(tree) else { continue };
            match keyword {
                None if word == "const"
                    && trees
                        .get(j)
                        .and_then(ident)
                        .is_some_and(|n| matches!(n, "fn" | "unsafe" | "async" | "extern")) => {}
                None if MODIFIERS.contains(&word) => {}
                None => keyword = Some(word),
                Some("fn" | "mod") if name.is_none() => name = Some(word.to_string()),
                Some(_) => {}
            }
        }
        let kind = match (keyword, name) {
            (Some("mod"), Some(name)) => Kind::Mod { name, body },
            (Some("fn"), Some(name)) => Kind::Fn { name },
            _ => Kind::Other,
        };
        out.push(Item {
            attrs: std::mem::take(&mut attrs),
            kind,
        });
        i = j;
    }
    (inner, out)
}

/// How many `#[test]`s a file holds at any depth: the count the walk must reach.
fn test_attrs_anywhere(trees: &[Tree]) -> usize {
    let mut n = 0;
    for (i, tree) in trees.iter().enumerate() {
        if let Tree::Group(_, inside) = tree {
            if i > 0 && is_punct(&trees[i - 1], '#') && is_test_attr(inside) {
                n += 1;
            }
            n += test_attrs_anywhere(inside);
        }
    }
    n
}

fn is_test_attr(attr: &[Tree]) -> bool {
    let (path, _) = attr_path(attr);
    path == "test" || path.ends_with("::test")
}

// ── evaluating cfg ───────────────────────────────────────────────────────────

struct Features {
    /// Every feature `[features]` declares.
    known: BTreeSet<String>,
    /// The default set, then each leg in [`LEGS`] order.
    builds: Vec<BTreeSet<String>>,
}

fn features() -> Features {
    let manifest: toml::Table = read(&crate_root().join("Cargo.toml"))
        .parse()
        .expect("yidam/cli/Cargo.toml parses");
    let table: BTreeMap<String, Vec<String>> = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("yidam/cli/Cargo.toml has a [features] table")
        .iter()
        .map(|(k, v)| {
            let entries = v
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            (k.clone(), entries)
        })
        .collect();
    let closure = |roots: Vec<String>| -> BTreeSet<String> {
        let mut on = BTreeSet::new();
        let mut todo = roots;
        while let Some(f) = todo.pop() {
            if table.contains_key(&f) && on.insert(f.clone()) {
                todo.extend(table[&f].iter().cloned());
            }
        }
        on
    };

    let mut builds = vec![closure(vec!["default".to_string()])];
    for (leg, flags) in LEGS {
        let flags = crate::local_ci::render_mise_env(flags);
        let tokens: Vec<&str> = flags.split_whitespace().collect();
        let build = if tokens == ["--all-features"] {
            table.keys().cloned().collect()
        } else if let ["--no-default-features", "--features", list] = tokens.as_slice() {
            closure(list.split(',').map(str::to_string).collect())
        } else {
            panic!("leg `{leg}` has flags `{flags}`, which this file does not know how to resolve")
        };
        builds.push(build);
    }
    Features {
        known: table.keys().cloned().collect(),
        builds,
    }
}

fn cfg_holds(pred: &[Tree], on: &BTreeSet<String>, known: &BTreeSet<String>, at: &str) -> bool {
    match pred {
        [Tree::Tok(Tok::Ident(name))] => match name.as_str() {
            "test" | "unix" | "debug_assertions" => true,
            "windows" | "miri" | "doc" | "doctest" => false,
            other => panic!("{at}: `cfg({other})` is a predicate this file does not evaluate"),
        },
        [Tree::Tok(Tok::Ident(name)), eq, Tree::Tok(Tok::Str(value))] if is_punct(eq, '=') => {
            match name.as_str() {
                "feature" => {
                    assert!(
                        known.contains(value),
                        "{at}: `feature = \"{value}\"` names no feature in [features]"
                    );
                    on.contains(value)
                }
                "target_os" => value == TARGET_OS,
                "target_arch" => value == TARGET_ARCH,
                "target_family" => value == "unix",
                other => {
                    panic!("{at}: `cfg({other} = …)` is a predicate this file does not evaluate")
                }
            }
        }
        [Tree::Tok(Tok::Ident(name)), Tree::Group('(', args)] => {
            let parts = split_commas(args);
            match name.as_str() {
                "not" => !cfg_holds(parts.first().copied().unwrap_or(&[]), on, known, at),
                "all" => parts.iter().all(|p| cfg_holds(p, on, known, at)),
                "any" => parts.iter().any(|p| cfg_holds(p, on, known, at)),
                other => {
                    panic!("{at}: `cfg({other}(…))` is a predicate this file does not evaluate")
                }
            }
        }
        _ => panic!("{at}: a `cfg` predicate this file cannot read"),
    }
}

/// The attributes that apply in one build: every `cfg_attr` whose predicate holds, expanded.
fn effective(
    attrs: &[Vec<Tree>],
    on: &BTreeSet<String>,
    known: &BTreeSet<String>,
    at: &str,
) -> Vec<Vec<Tree>> {
    let mut out = Vec::new();
    let mut todo: Vec<Vec<Tree>> = attrs.iter().rev().cloned().collect();
    while let Some(attr) = todo.pop() {
        let (path, rest) = attr_path(&attr);
        if path == "cfg_attr" {
            if let [Tree::Group('(', args)] = rest {
                let parts = split_commas(args);
                if cfg_holds(parts[0], on, known, at) {
                    todo.extend(parts[1..].iter().rev().map(|p| p.to_vec()));
                }
            }
        } else {
            out.push(attr);
        }
    }
    out
}

fn cfgs_hold(
    attrs: &[Vec<Tree>],
    on: &BTreeSet<String>,
    known: &BTreeSet<String>,
    at: &str,
) -> bool {
    attrs.iter().all(|attr| match attr_path(attr) {
        (path, [Tree::Group('(', pred)]) if path == "cfg" => cfg_holds(pred, on, known, at),
        _ => true,
    })
}

// ── walking the module tree ──────────────────────────────────────────────────

struct Test {
    binary: &'static str,
    name: String,
    /// Whether it runs, in the default build and then in each leg.
    runs: Vec<bool>,
}

struct Walk {
    features: Features,
    tests: Vec<Test>,
    /// Every file opened, with the `#[test]`s found in it and the ones reached as items.
    files: BTreeMap<PathBuf, (usize, usize)>,
}

impl Walk {
    fn file(&mut self, binary: &'static str, file: &Path, prefix: &[String], present: &[bool]) {
        let file = file.canonicalize().unwrap_or_else(|e| {
            panic!(
                "{}: a module declared here has no file: {e}",
                file.display()
            )
        });
        let at = rel(&file);
        let trees = trees(lex(&read(&file)), &at);
        let (inner, items) = items(&trees);
        let present: Vec<bool> = present
            .iter()
            .zip(&self.features.builds)
            .map(|(p, on)| {
                *p && cfgs_hold(
                    &effective(&inner, on, &self.features.known, &at),
                    on,
                    &self.features.known,
                    &at,
                )
            })
            .collect();
        let anywhere = test_attrs_anywhere(&trees);
        self.files.entry(file.clone()).or_insert((anywhere, 0));

        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let parent = file.parent().expect("a file has a directory").to_path_buf();
        let children = if matches!(stem, "mod" | "lib" | "main") {
            parent.clone()
        } else {
            parent.join(stem)
        };
        let reached = self.items(binary, &file, &items, prefix, &present, &parent, &children);
        if let Some(entry) = self.files.get_mut(&file) {
            entry.1 = entry.1.max(reached);
        }
    }

    /// Walk one module body; returns how many `#[test]` items it holds in this file.
    #[allow(clippy::too_many_arguments)]
    fn items(
        &mut self,
        binary: &'static str,
        file: &Path,
        items: &[Item],
        prefix: &[String],
        present: &[bool],
        path_base: &Path,
        children: &Path,
    ) -> usize {
        let at = rel(file);
        let mut reached = 0;
        for item in items {
            let per_build: Vec<Vec<Vec<Tree>>> = self
                .features
                .builds
                .iter()
                .map(|on| effective(&item.attrs, on, &self.features.known, &at))
                .collect();
            let here: Vec<bool> = present
                .iter()
                .zip(&per_build)
                .zip(&self.features.builds)
                .map(|((p, attrs), on)| *p && cfgs_hold(attrs, on, &self.features.known, &at))
                .collect();
            match &item.kind {
                Kind::Mod { name, body } => {
                    if !here.iter().any(|p| *p) {
                        continue;
                    }
                    let mut inner_prefix = prefix.to_vec();
                    inner_prefix.push(name.clone());
                    if let Some(body) = body {
                        let (inner, inner_items) = items_of(body);
                        let present: Vec<bool> = here
                            .iter()
                            .zip(&self.features.builds)
                            .map(|(p, on)| {
                                *p && cfgs_hold(
                                    &effective(&inner, on, &self.features.known, &at),
                                    on,
                                    &self.features.known,
                                    &at,
                                )
                            })
                            .collect();
                        reached += self.items(
                            binary,
                            file,
                            &inner_items,
                            &inner_prefix,
                            &present,
                            &children.join(name),
                            &children.join(name),
                        );
                    } else {
                        let path_attr = item.attrs.iter().find_map(|a| match attr_path(a) {
                            (p, [eq, Tree::Tok(Tok::Str(target))])
                                if p == "path" && is_punct(eq, '=') =>
                            {
                                Some(target.clone())
                            }
                            _ => None,
                        });
                        let target = match path_attr {
                            Some(target) => path_base.join(target),
                            None => {
                                let flat = children.join(format!("{name}.rs"));
                                if flat.exists() {
                                    flat
                                } else {
                                    children.join(name).join("mod.rs")
                                }
                            }
                        };
                        self.file(binary, &target, &inner_prefix, &here);
                    }
                }
                Kind::Fn { name } => {
                    if !item.attrs.iter().any(|a| is_test_attr(a)) {
                        continue;
                    }
                    reached += 1;
                    let runs = here
                        .iter()
                        .zip(&per_build)
                        .map(|(p, attrs)| {
                            *p && attrs.iter().any(|a| is_test_attr(a))
                                && !attrs.iter().any(|a| attr_path(a).0 == "ignore")
                        })
                        .collect();
                    let mut full = prefix.to_vec();
                    full.push(name.clone());
                    self.tests.push(Test {
                        binary,
                        name: full.join("::"),
                        runs,
                    });
                }
                Kind::Other => {}
            }
        }
        reached
    }
}

fn items_of(body: &[Tree]) -> (Vec<Vec<Tree>>, Vec<Item>) {
    items(body)
}

fn walk() -> Walk {
    let features = features();
    let builds = features.builds.len();
    let mut walk = Walk {
        features,
        tests: Vec::new(),
        files: BTreeMap::new(),
    };
    for (binary, root) in BINARIES {
        walk.file(binary, &crate_root().join(root), &[], &vec![true; builds]);
    }
    walk
}

/// The filterset one leg runs: what runs there and does not run in the default build.
fn derived_filter(walk: &Walk, leg: usize) -> (String, usize) {
    let mut selected: Vec<(&str, &str)> = walk
        .tests
        .iter()
        .filter(|t| t.runs[leg + 1] && !t.runs[0])
        .map(|t| (t.binary, t.name.as_str()))
        .collect();
    selected.sort_unstable();
    selected.dedup();
    let lines: Vec<String> = selected
        .iter()
        .enumerate()
        .map(|(i, (binary, name))| {
            let or = if i == 0 { "" } else { "| " };
            format!("{or}(binary_id({binary}) & test(={name}))")
        })
        .collect();
    (lines.join("\n") + "\n", selected.len())
}

// ── the gates ────────────────────────────────────────────────────────────────

/// Each leg's committed filter is the one the attributes derive.
#[test]
fn the_feature_test_filters_are_the_ones_the_attributes_derive() {
    let walk = walk();
    let update = std::env::var("UPDATE_GOLDENS").is_ok();
    let mut stale = Vec::new();
    for (i, (leg, flags)) in LEGS.iter().enumerate() {
        let (derived, count) = derived_filter(&walk, i);
        assert!(
            count > 0,
            "`{flags}` adds no test the default build does not run, so `test-features-{leg}` \
             would compile a build to run nothing. Drop the leg from LEGS and from mise.toml."
        );
        let path = golden(leg);
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        if committed == derived {
            continue;
        }
        if update {
            std::fs::create_dir_all(path.parent().expect("a golden has a directory"))
                .expect("the goldens directory can be made");
            std::fs::write(&path, &derived).expect("the filter can be written");
        } else {
            let old: BTreeSet<&str> = committed.lines().collect();
            let new: BTreeSet<&str> = derived.lines().collect();
            let added: Vec<&&str> = new.difference(&old).collect();
            let removed: Vec<&&str> = old.difference(&new).collect();
            stale.push(format!(
                "  {}: {count} tests derived\n    missing: {added:?}\n    no longer derived: \
                 {removed:?}",
                rel(&path)
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "a test's feature attributes changed, so what a pull request runs under a non-default \
         build changed with them, and the committed filter did not. Re-run with \
         UPDATE_GOLDENS=1 and review the diff:\n{}",
        stale.join("\n")
    );
}

/// The walk reached every `#[test]` there is, so the derivation is not silently partial.
#[test]
fn the_walk_reaches_every_test_it_could_select() {
    let walk = walk();

    let mut short = Vec::new();
    for (file, (anywhere, reached)) in &walk.files {
        if anywhere != reached {
            short.push(format!(
                "  {}: {anywhere} `#[test]`s, {reached} reached as test functions",
                rel(file)
            ));
        }
    }
    assert!(
        short.is_empty(),
        "these files hold a `#[test]` the walk did not reach as an item: inside a macro, a \
         function body, or a shape `items` does not read. Its attributes are then never \
         evaluated, so a feature-conditional test there would never be selected:\n{}",
        short.join("\n")
    );

    let mut unopened = Vec::new();
    for dir in ["src", "tests/suite", "tests/gates", "tests/common"] {
        for entry in common::repo_walk(&crate_root().join(dir)) {
            let path = entry.path();
            if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let canonical = path.canonicalize().expect("a walked file exists");
            let holds = test_attrs_anywhere(&trees(lex(&read(path)), &rel(path))) > 0;
            if holds && !walk.files.contains_key(&canonical) {
                unopened.push(format!("  {}", rel(path)));
            }
        }
    }
    assert!(
        unopened.is_empty(),
        "these files hold tests and no module declaration this walk followed reaches them. \
         Either they are not compiled, or the walk resolved a `mod` to the wrong file:\n{}",
        unopened.join("\n")
    );

    let total = walk.tests.iter().filter(|t| t.runs[0]).count();
    assert!(
        total > 4000,
        "only {total} tests run in the default build by this walk's count, against more than \
         4,600 nextest lists. It is reading the wrong tree."
    );
}

/// The legs a pull request compiles are the legs it tests, less the one that cannot be.
#[test]
fn every_compiled_leg_is_tested_or_excused() {
    let mise: toml::Table = read(&common::repo_root().join("mise.toml"))
        .parse()
        .expect("mise.toml parses");
    let task_lines = |name: &str| -> Vec<String> {
        let run = mise
            .get("tasks")
            .and_then(|t| t.get(name))
            .and_then(|t| t.get("run"))
            .unwrap_or_else(|| panic!("mise.toml has no `[tasks.{name}]` with a `run`"));
        match run {
            toml::Value::String(one) => one.lines().map(str::to_string).collect(),
            toml::Value::Array(many) => many
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        }
    };

    let compiled: Vec<String> = task_lines("check-features")
        .iter()
        .filter(|l| l.contains("cargo clippy"))
        .map(|l| {
            let words: Vec<&str> = l.split_whitespace().collect();
            let mut flags = Vec::new();
            let mut i = 0;
            while i < words.len() {
                match words[i] {
                    "--all-features" | "--no-default-features" => flags.push(words[i]),
                    "--features" => {
                        flags.push(words[i]);
                        flags.extend(words.get(i + 1));
                        i += 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            flags.join(" ")
        })
        .collect();
    assert!(
        compiled.len() >= 2,
        "`check-features` parsed to {compiled:?}; this test is reading the wrong thing"
    );

    for flags in &compiled {
        let tested = LEGS.iter().any(|(_, f)| f == flags);
        let excused = UNTESTED_LEGS.iter().any(|(f, _)| f == flags);
        assert!(
            tested || excused,
            "`check-features` compiles `{flags}` and no pull-request task tests it. Add it to \
             LEGS here with a task `test-features-<leg>`, or to UNTESTED_LEGS with the reason."
        );
    }
    for (flags, _) in UNTESTED_LEGS {
        assert!(
            compiled.iter().any(|f| f == flags),
            "UNTESTED_LEGS excuses `{flags}`, which `check-features` no longer compiles"
        );
    }
    let manifest = read(&crate_root().join("Cargo.toml"));
    assert!(
        manifest.contains("#482") && manifest.contains("compiled, and not tested"),
        "the `reports` note in yidam/cli/Cargo.toml no longer says why that build is not \
         tested, which is the reason UNTESTED_LEGS gives"
    );

    for (leg, flags) in LEGS {
        assert!(
            compiled.iter().any(|f| f == flags),
            "leg `{leg}` tests `{flags}`, which `check-features` does not compile"
        );
        let task = format!("test-features-{leg}");
        let lines = task_lines(&task);
        let filter = format!("tests/goldens/feature-tests/{leg}.filter");
        let run = lines
            .iter()
            .find(|l| l.contains("cargo nextest run"))
            .unwrap_or_else(|| panic!("`{task}` runs no `cargo nextest run`"));
        let list = lines
            .iter()
            .find(|l| l.contains("cargo nextest list"))
            .unwrap_or_else(|| panic!("`{task}` does not list what its filter matches"));
        for (what, line) in [("run", run), ("list", list)] {
            assert!(
                line.contains(&format!(" {flags} ")) && line.contains(&filter),
                "`{task}`'s nextest {what} is not `{flags}` with `{filter}`:\n  {line}"
            );
        }
        assert!(
            run.contains("--no-tests=fail"),
            "`{task}` would pass with a filter that matched nothing:\n  {run}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("grep -c") && l.contains(&filter)),
            "`{task}` does not compare what nextest lists with the names in `{filter}`, so a \
             name this file derives wrongly would match nothing and pass"
        );
    }
}

/// The CI job that runs the legs caps each of them and the job, and reports each.
#[test]
fn the_pull_request_job_runs_each_leg_capped_and_reported() {
    let yml = read(&common::repo_root().join(".github/workflows/ci.yml"));
    let job: Vec<&str> = yml
        .lines()
        .skip_while(|l| *l != "  cli-features:")
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with("   "))
        .collect();
    assert!(!job.is_empty(), "ci.yml has no `cli-features` job");
    assert!(
        job.iter().any(|l| l.starts_with("    timeout-minutes:")),
        "`cli-features` has no job-level `timeout-minutes`, so a hung leg runs for six hours"
    );
    assert!(
        job.iter()
            .any(|l| l.trim() == "if: github.event_name == 'pull_request'"),
        "`cli-features` is not the pull-request job any more"
    );

    for (leg, _) in LEGS {
        let task = format!("test-features-{leg}");
        let run = job
            .iter()
            .position(|l| {
                l.trim() == format!("run: .github/scripts/with-sampler.sh mise run {task}")
            })
            .unwrap_or_else(|| panic!("`cli-features` does not run `mise run {task}`"));
        // The step's own keys sit between its `- name:` and its `run:`.
        let step: Vec<&str> = job[..run]
            .iter()
            .rev()
            .take_while(|l| !l.trim_start().starts_with("- "))
            .copied()
            .collect();
        assert!(
            step.iter()
                .any(|l| l.trim().starts_with("timeout-minutes:")),
            "the step running `{task}` has no `timeout-minutes` of its own"
        );
        let summary = job[run..]
            .iter()
            .skip_while(|l| !l.contains("./.github/actions/test-summary"))
            .take_while(|l| !l.contains("mise run test-features-") || l.contains(&task))
            .any(|l| l.trim() == format!("artifact: test-results-cli-features-{leg}"));
        assert!(
            summary,
            "no test summary follows `{task}` with `artifact: test-results-cli-features-{leg}`, \
             so its results reach a reader only as a colored dot"
        );
    }
}
