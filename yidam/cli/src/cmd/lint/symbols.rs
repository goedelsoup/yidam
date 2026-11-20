//! Whether a pinned symbol changed (RFC-0051 §4.1): the grain a pin with a `symbol` is
//! measured at, once its file is known to differ.
//!
//! **The fallback only ever reports more reads due.** [`compare`] is asked only about a file
//! whose object moved. With the `symbol-pins` feature and a grammar for the file's extension,
//! it parses both revisions and compares the byte span of the named item, so an edit elsewhere
//! in the file makes nothing due. In every other case (no feature, no grammar, a symbol not
//! found at the read) it answers [`Grain::File`], which is due, as the file grain was.
//!
//! The grammar is picked by extension: `.rs` is Rust, `.py`/`.pyi` Python, `.ts`/`.mts`/`.cts`
//! TypeScript and `.tsx` TSX. A symbol path is matched by its last segment, split on `::` or
//! `.`, against the name of a definition. When the previous segment names a definition that
//! holds one of them, the read is every such one inside it (`MicroDefaults::load` is the `load`
//! in `impl MicroDefaults`); otherwise it is every definition of that name in the file, in
//! order, so a field and a function sharing a name are read together. A read found inside its
//! parent is gone when nothing of that name is inside the parent any longer, whatever else in
//! the file shares the name.

/// What the pinned symbol did between the read and the reference revision.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
pub(crate) enum Grain {
    /// The item's span is the same read, up to re-indenting the whole item. Not due.
    Unchanged,
    /// The item's span changed.
    Changed,
    /// The item was found at the read and is not at the reference revision.
    Gone,
    /// The symbol could not be compared, so the file grain stands; the reason is said.
    File(String),
}

impl Grain {
    /// Whether the read is due at this grain.
    pub(crate) fn due(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    /// The clock line's name for the grain.
    pub(crate) fn said(&self) -> String {
        match self {
            Self::Unchanged => "symbol unchanged".into(),
            Self::Changed => "symbol changed".into(),
            Self::Gone => "symbol gone".into(),
            Self::File(why) => format!("file changed ({why})"),
        }
    }
}

/// Compare `symbol` in `path` between the text it was read at and the text now. The caller
/// has already seen the two file objects differ; `None` is a text that could not be read.
#[cfg_attr(not(feature = "symbol-pins"), allow(unused_variables))]
pub(crate) fn compare(path: &str, symbol: &str, was: Option<&str>, now: Option<&str>) -> Grain {
    #[cfg(feature = "symbol-pins")]
    {
        let Some(lang) = grammar::for_path(path) else {
            return Grain::File(format!("no parser for {}", extension(path)));
        };
        let (Some(was), Some(now)) = (was, now) else {
            return Grain::File("the file could not be read".into());
        };
        use grammar::Found;
        let Some(read) = grammar::find(&lang, symbol, was) else {
            return Grain::File("symbol not found at the read".into());
        };
        match (read, grammar::find(&lang, symbol, now)) {
            (Found::Scoped(a), Some(Found::Scoped(b)))
            | (Found::Named(a), Some(Found::Named(b))) => {
                if same_read(&a, &b) {
                    Grain::Unchanged
                } else {
                    Grain::Changed
                }
            }
            // Read inside its parent, and nothing of that name is inside it now: another
            // definition elsewhere sharing the name is not the item that was read.
            (Found::Scoped(_), _) | (Found::Named(_), None) => Grain::Gone,
            // Its parent now holds one, so the read now resolves to another item.
            (Found::Named(_), Some(Found::Scoped(_))) => Grain::Changed,
        }
    }
    #[cfg(not(feature = "symbol-pins"))]
    Grain::File("built without `symbol-pins`".into())
}

/// `.toml` for `Cargo.toml`, and the file name for a file with no extension.
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
fn extension(path: &str) -> String {
    let p = std::path::Path::new(path);
    match p.extension() {
        Some(e) => format!(".{}", e.to_string_lossy()),
        None => format!(
            "`{}`",
            p.file_name()
                .map_or_else(|| path.into(), |n| n.to_string_lossy().into_owned())
        ),
    }
}

/// Two spans are the same read when they differ only by re-indenting the whole item, so
/// moving it into a module, say, does not make its read due. Only the indentation every line
/// shares is set aside: a line indented apart from its neighbours is a change, which in Python
/// moves a statement into or out of a block.
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
fn same_read(a: &str, b: &str) -> bool {
    dedent(a) == dedent(b)
}

/// The span's lines with the indentation its later lines share removed. The first line starts
/// at the item, so its own indentation is not in the span; blank lines are empty.
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
fn dedent(s: &str) -> Vec<&str> {
    let mut lines = s.lines();
    let first = lines.next().map(str::trim_start);
    let rest: Vec<&str> = lines.collect();
    let indent = rest
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    first
        .into_iter()
        .chain(rest.iter().map(|l| {
            if l.trim().is_empty() {
                ""
            } else {
                l.get(indent..).unwrap_or(l)
            }
        }))
        .collect()
}

#[cfg(feature = "symbol-pins")]
mod grammar {
    use tree_sitter::{Language, Node, Parser};

    /// The grammar for a path's extension, or `None` when there is none.
    pub(super) fn for_path(path: &str) -> Option<Language> {
        let ext = std::path::Path::new(path).extension()?.to_str()?;
        Some(match ext {
            "rs" => tree_sitter_rust::LANGUAGE.into(),
            "py" | "pyi" => tree_sitter_python::LANGUAGE.into(),
            "ts" | "mts" | "cts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => return None,
        })
    }

    /// A node that defines something by name: a Rust `*_item`, a Python `*_definition`, a
    /// TypeScript `*_declaration`/`*_declarator`/`*_signature`/`*_definition`, or an enum
    /// variant. A call or a keyword argument names nothing it defines and is not one.
    fn defines(kind: &str) -> bool {
        [
            "_item",
            "_definition",
            "_declaration",
            "_declarator",
            "_signature",
        ]
        .iter()
        .any(|s| kind.ends_with(s))
            || kind == "enum_variant"
    }

    /// The name a definition is known by: its `name`, or for a Rust `impl` the type it is
    /// for, without generic arguments.
    fn name<'t>(node: Node<'_>, text: &'t str) -> Option<&'t str> {
        let n = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("type"))?;
        let s = text.get(n.byte_range())?;
        Some(s.split('<').next().unwrap_or(s).trim())
    }

    /// What `symbol` names in one revision's text: every definition of its last segment, in
    /// file order, joined as one read.
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Found {
        /// Inside a definition named by the symbol's previous segment.
        Scoped(String),
        /// By its last segment alone: it has no previous segment, or no definition of that
        /// name holds one.
        Named(String),
    }

    /// What `symbol` names in `text`, or `None` when nothing defines it.
    pub(super) fn find(lang: &Language, symbol: &str, text: &str) -> Option<Found> {
        let segments: Vec<&str> = {
            let s = symbol.trim();
            let s = s.split('(').next().unwrap_or(s);
            s.split("::")
                .flat_map(|p| p.split('.'))
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect()
        };
        let (last, parents) = segments.split_last()?;
        let mut parser = Parser::new();
        parser.set_language(lang).ok()?;
        let tree = parser.parse(text, None)?;
        let mut found: Vec<Node<'_>> = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if defines(node.kind())
                && node.child_by_field_name("name").is_some()
                && name(node, text) == Some(last)
            {
                found.push(node);
            }
            let mut cursor = node.walk();
            let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
        }
        let inside = |node: &Node<'_>, parent: &str| {
            let mut up = node.parent();
            while let Some(p) = up {
                if defines(p.kind()) && name(p, text) == Some(parent) {
                    return true;
                }
                up = p.parent();
            }
            false
        };
        let scoped: Vec<Node<'_>> = match parents.last() {
            Some(parent) => found
                .iter()
                .filter(|n| inside(n, parent))
                .copied()
                .collect(),
            None => Vec::new(),
        };
        let read = |nodes: &[Node<'_>]| -> Option<String> {
            let mut taken: Vec<std::ops::Range<usize>> = Vec::new();
            for n in nodes {
                // A decorator is part of what a Python definition says.
                let n = match n.parent() {
                    Some(p) if p.kind() == "decorated_definition" => p,
                    _ => *n,
                };
                let r = n.byte_range();
                if !taken.iter().any(|t| t.start <= r.start && r.end <= t.end) {
                    taken.push(r);
                }
            }
            let parts: Option<Vec<&str>> = taken.into_iter().map(|r| text.get(r)).collect();
            parts.filter(|p| !p.is_empty()).map(|p| p.join("\n"))
        };
        if scoped.is_empty() {
            read(&found).map(Found::Named)
        } else {
            read(&scoped).map(Found::Scoped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn re_indenting_is_the_same_read() {
        assert!(same_read(
            "fn a() {\n    x\n}",
            "    fn a() {\n        x\n    }"
        ));
        assert!(!same_read("fn a() {\n    x\n}", "fn a() {\n    y\n}"));
    }

    /// Moving one statement into a block changes what a Python function does, and only the
    /// indentation of that one line says so.
    #[test]
    fn re_indenting_one_line_is_a_change() {
        let was = "def f():\n    if x:\n        a()\n    b()";
        let now = "def f():\n    if x:\n        a()\n        b()";
        assert!(!same_read(was, now));
        let moved = "def f():\n        if x:\n            a()\n        b()";
        assert!(same_read(was, moved));
    }

    #[test]
    fn the_extension_is_named_or_the_file_is() {
        assert_eq!(extension("Cargo.toml"), ".toml");
        assert_eq!(extension("tools/Makefile"), "`Makefile`");
    }

    #[test]
    fn every_grain_but_unchanged_is_due() {
        assert!(!Grain::Unchanged.due());
        for g in [Grain::Changed, Grain::Gone, Grain::File("x".into())] {
            assert!(g.due(), "{g:?}");
        }
    }

    #[cfg(not(feature = "symbol-pins"))]
    #[test]
    fn without_the_feature_every_symbol_is_read_at_file_grain() {
        let g = compare(
            "src/lib.rs",
            "load",
            Some("fn load() {}"),
            Some("fn load() {}"),
        );
        assert_eq!(g, Grain::File("built without `symbol-pins`".into()));
        assert!(g.due());
    }

    #[cfg(feature = "symbol-pins")]
    mod parsed {
        use super::super::*;

        const RUST: &str = "pub struct MicroDefaults;\n\
            impl MicroDefaults {\n    fn load() -> u8 {\n        1\n    }\n}\n\
            fn load() -> u8 {\n    2\n}\n";

        #[test]
        fn an_edit_elsewhere_leaves_the_symbol_unchanged() {
            let now = format!("{RUST}\nfn unrelated() {{}}\n");
            let now = now.replace("    2\n", "    3\n");
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(RUST), Some(&now)),
                Grain::Unchanged
            );
        }

        #[test]
        fn an_edit_inside_the_symbol_changes_it() {
            let now = RUST.replace("        1\n", "        10\n");
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(RUST), Some(&now)),
                Grain::Changed
            );
        }

        #[test]
        fn the_parent_segment_picks_between_two_of_a_name() {
            let now = RUST.replace("        1\n", "        10\n");
            assert_eq!(
                compare("src/lib.rs", "load", Some(RUST), Some(&now)),
                Grain::Changed,
                "with no parent every `load` in the file is read"
            );
            let now = RUST.replace("    2\n", "    20\n");
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(RUST), Some(&now)),
                Grain::Unchanged
            );
        }

        #[test]
        fn a_symbol_that_disappears_is_gone() {
            let now = "pub struct MicroDefaults;\n";
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(RUST), Some(now)),
                Grain::Gone
            );
        }

        /// The method was read inside its `impl`. Deleting it leaves a free `fn load` with the
        /// same text, which is another item, not the read.
        #[test]
        fn a_method_gone_from_its_parent_is_gone_while_a_namesake_remains() {
            let was = "struct MicroDefaults;\nimpl MicroDefaults {\n    fn load() -> u8 { 2 }\n}\n\
                fn load() -> u8 { 2 }\n";
            let now = "struct MicroDefaults;\nimpl MicroDefaults {}\nfn load() -> u8 { 2 }\n";
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(was), Some(now)),
                Grain::Gone
            );
            let none = "fn load() -> u8 { 2 }\n";
            assert_eq!(
                compare("src/lib.rs", "MicroDefaults::load", Some(was), Some(none)),
                Grain::Gone
            );
        }

        /// A field declared above a function of the same name does not stand in for it.
        #[test]
        fn a_namesake_earlier_in_the_file_does_not_hide_an_edit() {
            let was = "struct Cfg {\n    load: u8,\n}\nfn load() -> u8 {\n    1\n}\n";
            let now = was.replace("    1\n", "    10\n");
            assert_eq!(
                compare("src/lib.rs", "load", Some(was), Some(&now)),
                Grain::Changed
            );
        }

        /// A path qualifier naming no definition in the file, such as a module, is read as
        /// the bare name.
        #[test]
        fn a_module_path_is_read_by_its_last_segment() {
            let now = RUST.replace("    2\n", "    20\n");
            assert_eq!(
                compare("src/lib.rs", "crate::game::load", Some(RUST), Some(&now)),
                Grain::Changed
            );
            let elsewhere = format!("{RUST}\nfn unrelated() {{}}\n");
            assert_eq!(
                compare(
                    "src/lib.rs",
                    "crate::game::load",
                    Some(RUST),
                    Some(&elsewhere)
                ),
                Grain::Unchanged
            );
        }

        #[test]
        fn a_symbol_not_found_at_the_read_falls_back_to_the_file() {
            assert_eq!(
                compare("src/lib.rs", "Nowhere::thing", Some(RUST), Some(RUST)),
                Grain::File("symbol not found at the read".into())
            );
        }

        #[test]
        fn a_file_with_no_grammar_falls_back_and_says_why() {
            assert_eq!(
                compare("Cargo.toml", "package", Some("a"), Some("b")),
                Grain::File("no parser for .toml".into())
            );
        }

        #[test]
        fn python_methods_and_decorators_are_spans() {
            let was = "class Grid:\n    @cached\n    def load(self):\n        return 1\n\n\
                def other():\n    return 2\n";
            let elsewhere = was.replace("return 2", "return 3");
            assert_eq!(
                compare("m.py", "Grid.load", Some(was), Some(&elsewhere)),
                Grain::Unchanged
            );
            let decorator = was.replace("@cached", "@lru_cache");
            assert_eq!(
                compare("m.py", "Grid.load", Some(was), Some(&decorator)),
                Grain::Changed
            );
        }

        #[test]
        fn typescript_and_tsx_are_parsed() {
            let was = "export class Grid {\n  load(): number { return 1; }\n}\n\
                export const other = () => 2;\n";
            let elsewhere = was.replace("=> 2", "=> 3");
            for path in ["a.ts", "a.tsx"] {
                assert_eq!(
                    compare(path, "Grid.load", Some(was), Some(&elsewhere)),
                    Grain::Unchanged,
                    "{path}"
                );
                assert_eq!(
                    compare(path, "other", Some(was), Some(&elsewhere)),
                    Grain::Changed,
                    "{path}"
                );
            }
        }
    }
}
