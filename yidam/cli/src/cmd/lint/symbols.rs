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
//! `.`, against the name of a definition. When several definitions share that name, the one
//! inside a definition named by the previous segment is taken (`MicroDefaults::load` is the
//! `load` in `impl MicroDefaults`), and the first in the file otherwise.

/// What the pinned symbol did between the read and the reference revision.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
pub(crate) enum Grain {
    /// The item's span is the same read, up to leading whitespace. Not due.
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
        let Some(read) = grammar::span(&lang, symbol, was) else {
            return Grain::File("symbol not found at the read".into());
        };
        match grammar::span(&lang, symbol, now) {
            Some(current) if same_read(&read, &current) => Grain::Unchanged,
            Some(_) => Grain::Changed,
            None => Grain::Gone,
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

/// Two spans are the same read when they differ only in leading whitespace on each line, so
/// re-indenting an item (moving it into a module, say) does not make its read due.
#[cfg_attr(not(feature = "symbol-pins"), allow(dead_code))]
fn same_read(a: &str, b: &str) -> bool {
    let lines = |s: &str| {
        s.lines()
            .map(|l| l.trim_start().to_string())
            .collect::<Vec<_>>()
    };
    lines(a) == lines(b)
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

    /// The text of the item `symbol` names in `text`, or `None` when nothing defines it.
    pub(super) fn span(lang: &Language, symbol: &str, text: &str) -> Option<String> {
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
        let pick = parents
            .last()
            .and_then(|parent| found.iter().find(|n| inside(n, parent)))
            .or_else(|| found.first())?;
        // A decorator is part of what a Python definition says.
        let node = match pick.parent() {
            Some(p) if p.kind() == "decorated_definition" => p,
            _ => *pick,
        };
        text.get(node.byte_range()).map(str::to_string)
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
                "with no parent the first `load` in the file is taken"
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
