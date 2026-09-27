//! The resolved corpus a step is handed beside its bytes (#1080).
//!
//! # Why a step needs this at all
//!
//! `reads` gets a step the bytes of what it declared and nothing else, so a calculator does
//! the CLI's job again before it can do its own. The worked example measured it: of 190 lines
//! of `sh` and `awk` in `travel-tier.sh`, lines 74–82 parsed node YAML by regex and lines
//! 100–116 re-implemented link resolution as an awk `function resolve(base, t)` — a second
//! answer to the question [`crate::corpus::edges::resolve_target`] already answers, whose own
//! header states the *one resolver, one answer* rule the script was quietly breaking. Nothing
//! compared the two, so a `links:` entry the regex did not anticipate resolved one way in the
//! calculator and another way in `yidam graph`.
//!
//! This is the CLI's answer, materialized, so that the parse, the path resolution and the
//! malformed-file decision happen once and a calculator is only the rule it computes.
//!
//! # Tab-separated records, and **not** `yidam graph --json`
//!
//! #1080 proposed the shape `yidam graph --json` emits. That was the wrong shape for this
//! surface, and the reason is the whole point of the change: the contract is *"deliberately
//! small enough to implement in a shell script"*
//! ([`invoke`](../exec.rs)), the consumers are shell scripts, and JSON in `awk` is a worse
//! parser than the YAML regex it would replace. A form that cannot be read by the thing it is
//! for would have moved the duplication rather than removed it.
//!
//! So: one record per line, fields separated by a tab, each line beginning with its kind. That
//! is `IFS=$'\t'` in `sh` and `-F'\t'` in `awk`, and a consumer that wants only one kind of
//! record greps for it. Two further properties fall out of the format and are worth naming,
//! because scripts will depend on both:
//!
//! - **The order is the corpus's.** Nodes arrive in [`crate::corpus::Corpus`]'s walk order and
//!   properties in the order the file wrote them, so a script needs no `sort` and no `LC_ALL=C`
//!   preamble to make its output a property of the corpus rather than of the locale.
//! - **Every field is escaped**, so a record is exactly one line and a tab inside a label
//!   cannot become a field boundary. See [`escape`].
//!
//! # What it does not carry
//!
//! Prose. A node's `description` and its body text are not here: those are what the bytes are
//! for, and a step computing over prose has the file. Nor is a non-scalar property's value — a
//! `prop` row for a mapping states its `kind` and stops, because rendering arbitrary nested
//! YAML into a flat record would be inventing a second encoding of the file that sits next to
//! it in the same tree.

use std::fmt::Write as _;

use serde_yaml::Value;

use crate::corpus::{resolve_target, Class, Node};

/// The record grammar's version, not the corpus's.
///
/// Bumped when a record kind changes shape or a field moves. A new *kind* of record is
/// additive and does not bump it: a consumer reads the kinds it knows and a line beginning
/// with a word it has not heard of is one it skips, which is the rule the file's own header
/// states so that nobody has to guess it.
pub const FORMAT_VERSION: u32 = 1;

/// Make a field safe to sit between tabs on one line.
///
/// Backslash first, or the escapes would not be reversible. A consumer that does not unescape
/// is still correct about field *boundaries*, which is the property the format exists to give
/// `awk` — it only sees `\t` where a label really held a tab.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// What kind of thing a property's value is, for a consumer deciding whether to read it.
///
/// Named rather than inferred from the rendering, because `1` and `"1"` are a different
/// answer to *is this a count* and a script that guessed would get it right until the day a
/// corpus quoted a number.
fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_i64() || n.is_u64() => "int",
        Value::Number(_) => "float",
        Value::String(_) => "string",
        Value::Sequence(_) => "seq",
        Value::Mapping(_) => "map",
        Value::Tagged(_) => "tagged",
    }
}

/// A scalar's value as a consumer should read it, or `None` where it has none to give.
///
/// A `bool` is `true`/`false` and never `yes`: the corpus may have written either, and a
/// script comparing against one spelling would be reading the file's typography rather than
/// its value.
fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some(String::new()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// One node's properties as `prop` records, flattening a sequence of scalars and stopping at
/// anything deeper.
///
/// A sequence element is keyed `<key>[<i>]`, which keeps a list readable without a second
/// record kind and keeps the index in the key where a script can match on it. A nested
/// sequence or mapping is one row stating its kind: the value is in the file, the step has the
/// file, and a flat rendering of it would be a second encoding of the same bytes.
fn props(out: &mut String, id: &str, v: &Value) {
    let Value::Mapping(m) = v else { return };
    for (k, val) in m {
        let Some(key) = k.as_str() else { continue };
        match val {
            Value::Sequence(items) => {
                let _ = writeln!(
                    out,
                    "prop\t{}\t{}\tseq\t{}",
                    escape(id),
                    escape(key),
                    items.len()
                );
                for (i, item) in items.iter().enumerate() {
                    let value = scalar(item).map(|s| escape(&s)).unwrap_or_default();
                    let _ = writeln!(
                        out,
                        "prop\t{}\t{}[{i}]\t{}\t{value}",
                        escape(id),
                        escape(key),
                        kind_of(item)
                    );
                }
            }
            other => {
                let value = scalar(other).map(|s| escape(&s)).unwrap_or_default();
                let _ = writeln!(
                    out,
                    "prop\t{}\t{}\t{}\t{value}",
                    escape(id),
                    escape(key),
                    kind_of(other)
                );
            }
        }
    }
}

/// Render the resolved corpus.
///
/// **Ids are repository-relative**, which is where this deliberately differs from
/// `yidam graph --json`'s corpus-relative `node`. A step stands in a tree rooted at the
/// repository root, so a repository-relative id is a path the script can open — and it is the
/// same spelling `reads` is written in and the receipt lists its inputs in, so a consumer
/// comparing an id against either is comparing like with like. `corpus_dir` is still stated,
/// for a consumer that wants to know which prefix is the corpus.
///
/// `exists` is asked about a repository-relative path and is the reason this takes a closure
/// rather than touching the filesystem. The tree a step stands in holds only what it declared,
/// so a link pointing at a node outside `reads` is not a file *there* — and answering from that
/// tree would tell a calculator an edge is broken when `yidam graph` and `lint` call it fine.
/// The caller passes the commit's own file list, so `exists` means what it means everywhere
/// else, and *is this node in my view* is a question the consumer answers for itself by looking
/// for a `node` record with that id.
fn render(
    corpus_dir: &str,
    nodes: &[&Node],
    classes: &[&Class],
    exists: &dyn Fn(&str) -> bool,
) -> String {
    let mut out = String::new();
    out.push_str(
        "# The resolved corpus, as the CLI read it. Tab-separated; one record per line; the\n\
         # first field is the record kind, and a kind you do not know is a line you skip.\n\
         # Fields are escaped: \\\\ \\t \\n \\r. Ids are repository-relative.\n\
         # Written by `yidam run` (#1080); see RFC-0026.\n",
    );
    let _ = writeln!(out, "format_version\t{FORMAT_VERSION}");
    let _ = writeln!(out, "corpus_dir\t{}", escape(corpus_dir));

    for c in classes {
        let _ = writeln!(out, "class\t{}", escape(&c.name));
        for p in &c.properties {
            let _ = writeln!(
                out,
                "class_prop\t{}\t{}\t{}\t{}",
                escape(&c.name),
                escape(&p.name),
                escape(&p.r#type),
                p.required
            );
        }
    }

    for n in nodes {
        let id = &n.rel;
        let _ = writeln!(
            out,
            "node\t{}\t{}\t{}",
            escape(id),
            escape(n.inst.class.as_deref().unwrap_or_default()),
            escape(n.inst.label.as_deref().unwrap_or_default())
        );
        // A node that does not parse is stated as one rather than left to look empty. The
        // parser's rule is that unparseable is the default instance, so a consumer given no
        // marker would read a broken file as a node with no properties and no links — and
        // would compute over it as though the corpus said nothing, which is the one reading
        // that is certainly wrong.
        if let Some(why) = &n.malformed {
            let _ = writeln!(out, "malformed\t{}\t{}", escape(id), escape(why));
        }
        if let Some(p) = &n.inst.properties {
            props(&mut out, id, &Value::Mapping(p.clone()));
        }
        for (i, l) in n.inst.links.iter().flatten().enumerate() {
            let target = l.target.clone().unwrap_or_default();
            // One resolver, one answer: the same call `lint`'s `dangling_edge` and
            // `yidam graph` make, against the node's own path, so a target written `../a/b.yml`
            // lands where every other reader of this corpus puts it.
            let resolved = if target.is_empty() {
                String::new()
            } else {
                resolve_target(std::path::Path::new(id), &target)
                    .to_string_lossy()
                    .replace('\\', "/")
            };
            let _ = writeln!(
                out,
                "link\t{}\t{i}\t{}\t{}\t{}\t{}",
                escape(id),
                escape(l.relationship.as_deref().unwrap_or_default()),
                escape(&target),
                escape(&resolved),
                if !resolved.is_empty() && exists(&resolved) {
                    "true"
                } else {
                    "false"
                }
            );
        }
    }
    out
}

/// The resolved corpus for one step, or `None` where its `reads` admit no node.
///
/// **The one builder, called from both sides.** `yidam run` calls it over a materialized
/// scratch tree, where `keep` is *everything* because the tree already holds exactly what the
/// step declared; `doctor` calls it over the working tree, where `keep` is the declaration
/// applied to a corpus that holds more than the step may see. Two builders would be two
/// answers, and since this document's digest is part of the input state a run and a `doctor`
/// that disagreed about it would report every step stale forever — the failure
/// [`super::resolve_reads`] already carries a note about, one field further in.
///
/// `keep` is asked about a repository-relative path, which is the spelling `reads` is written
/// in and the spelling [`crate::corpus::Node::rel`] carries.
pub fn build(
    read: &crate::corpus::Corpus,
    keep: &dyn Fn(&str) -> bool,
    exists: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let nodes: Vec<&Node> = read.nodes().iter().filter(|n| keep(&n.rel)).collect();
    if nodes.is_empty() {
        return None;
    }
    let classes: Vec<&Class> = read.classes().iter().filter(|c| keep(&c.rel)).collect();
    let corpus_dir = read
        .dir()
        .strip_prefix(read.root())
        .unwrap_or(read.dir())
        .to_string_lossy()
        .replace('\\', "/");
    Some(render(&corpus_dir, &nodes, &classes, exists))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(rel: &str, text: &str) -> Node {
        Node::parse(std::path::PathBuf::from(rel), rel, text)
    }

    fn render_one(n: &Node, exists: &dyn Fn(&str) -> bool) -> String {
        render(".yidam/corpus", &[n], &[], exists)
    }

    fn rows(out: &str, kind: &str) -> Vec<Vec<String>> {
        out.lines()
            .filter(|l| l.split('\t').next() == Some(kind))
            .map(|l| l.split('\t').map(str::to_string).collect())
            .collect()
    }

    /// The duplication #1080 was filed about, asserted at the boundary: a target written with
    /// `..` resolves the way `resolve_target` resolves it, and the consumer is handed the
    /// answer rather than the ingredients.
    #[test]
    fn a_relative_target_arrives_resolved() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nlinks:\n  - target: ../station/b.yml\n    relationship: supports\n",
        );
        let out = render_one(&n, &|_| true);
        let link = &rows(&out, "link")[0];
        assert_eq!(link[4], "../station/b.yml", "the target as written: {out}");
        assert_eq!(link[5], ".yidam/corpus/station/b.yml", "resolved: {out}");
    }

    /// The reason `exists` is a closure and not a `is_file` call.
    ///
    /// The step's tree holds only what it declared, so the file a link points at is routinely
    /// not there — and a document that answered from that tree would tell a calculator an edge
    /// is broken when `yidam graph` and `lint` call it fine. Asserted as the pair, because
    /// either answer alone is consistent with the wrong implementation.
    #[test]
    fn a_target_outside_the_step_s_view_is_not_reported_broken() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nlinks:\n  - target: ../station/b.yml\n    relationship: supports\n",
        );
        let held = |p: &str| p == ".yidam/corpus/station/b.yml";
        assert_eq!(rows(&render_one(&n, &held), "link")[0][6], "true");
        assert_eq!(rows(&render_one(&n, &|_| false), "link")[0][6], "false");
    }

    /// An empty `target:` is not an edge to anywhere, and must not be reported as one that
    /// resolves to the node's own directory — which is what the lexical resolver would say if
    /// it were asked.
    #[test]
    fn an_empty_target_resolves_to_nothing_and_exists_nowhere() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nlinks:\n  - relationship: supports\n",
        );
        let link = &rows(&render_one(&n, &|_| true), "link")[0];
        assert_eq!(link[5], "", "no resolved path");
        assert_eq!(link[6], "false", "and it exists nowhere");
    }

    /// The property that makes the format readable by `awk -F'\t'` at all: a field holding a
    /// tab or a newline must not become a field or a record boundary.
    #[test]
    fn a_label_holding_a_tab_or_a_newline_is_still_one_record() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nlabel: \"one\\ttwo\\nthree\"\n",
        );
        let out = render_one(&n, &|_| true);
        let node_rows = rows(&out, "node");
        assert_eq!(node_rows.len(), 1, "one record: {out}");
        assert_eq!(node_rows[0][3], "one\\ttwo\\nthree", "{out}");
    }

    /// `1` and `"1"` are a different answer to *is this a count*, so the kind is stated and
    /// not left to be guessed from the rendering.
    #[test]
    fn a_quoted_number_is_not_reported_as_a_number() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nproperties:\n  a: 1\n  b: \"1\"\n  c: 1.5\n  d: true\n  e: null\n",
        );
        let kinds: Vec<(String, String, String)> = rows(&render_one(&n, &|_| true), "prop")
            .into_iter()
            .map(|r| (r[2].clone(), r[3].clone(), r[4].clone()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("a".into(), "int".into(), "1".into()),
                ("b".into(), "string".into(), "1".into()),
                ("c".into(), "float".into(), "1.5".into()),
                ("d".into(), "bool".into(), "true".into()),
                ("e".into(), "null".into(), String::new()),
            ]
        );
    }

    /// A list is flattened with its index in the key, and its length is stated first so a
    /// consumer knows how many rows to expect rather than counting them.
    #[test]
    fn a_list_of_scalars_is_flattened_with_its_length_stated() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nproperties:\n  tags: [x, y]\n",
        );
        let props = rows(&render_one(&n, &|_| true), "prop");
        assert_eq!(props[0][2..5], ["tags", "seq", "2"]);
        assert_eq!(props[1][2..5], ["tags[0]", "string", "x"]);
        assert_eq!(props[2][2..5], ["tags[1]", "string", "y"]);
    }

    /// Anything deeper states its kind and stops. The value is in the file, the step has the
    /// file, and a flat rendering of nested YAML would be a second encoding of the same bytes
    /// sitting beside them in the same tree.
    #[test]
    fn a_nested_property_states_its_kind_and_no_value() {
        let n = node(
            ".yidam/corpus/claim/a.yml",
            "class: claim\nproperties:\n  m: {a: 1}\n  s: [[1]]\n",
        );
        let props = rows(&render_one(&n, &|_| true), "prop");
        assert_eq!(props[0][2..5], ["m", "map", ""]);
        assert_eq!(props[2][2..5], ["s[0]", "seq", ""]);
    }

    /// A node that will not parse is the default instance, so without a marker a consumer
    /// reads a broken file as a node that said nothing — and computes over it as though the
    /// corpus had.
    #[test]
    fn a_malformed_node_says_so_rather_than_looking_empty() {
        let n = node(".yidam/corpus/claim/a.yml", "class: claim\n  bad: [\n");
        let out = render_one(&n, &|_| true);
        assert_eq!(rows(&out, "malformed").len(), 1, "{out}");
        assert_eq!(rows(&out, "node").len(), 1, "and it is still a node: {out}");
    }

    /// The header states the format's own version, and it is the first record so a consumer
    /// can refuse a document it does not understand before reading any of it.
    #[test]
    fn the_first_record_is_the_format_version() {
        let n = node(".yidam/corpus/claim/a.yml", "class: claim\n");
        let out = render_one(&n, &|_| true);
        let first = out.lines().find(|l| !l.starts_with('#')).expect("a record");
        assert_eq!(first, format!("format_version\t{FORMAT_VERSION}"));
    }

    /// Escaping is reversible, which a consumer that *does* unescape depends on. Backslash
    /// first or it would not be.
    #[test]
    fn the_escape_is_reversible() {
        let raw = "a\\tb\tc\nd\\\\e\r";
        let e = escape(raw);
        assert!(
            !e.contains('\t') && !e.contains('\n') && !e.contains('\r'),
            "{e}"
        );
        let mut back = String::new();
        let mut chars = e.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                back.push(c);
                continue;
            }
            match chars.next() {
                Some('\\') => back.push('\\'),
                Some('t') => back.push('\t'),
                Some('n') => back.push('\n'),
                Some('r') => back.push('\r'),
                other => {
                    back.push('\\');
                    back.extend(other);
                }
            }
        }
        assert_eq!(back, raw);
    }
}
