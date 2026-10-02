//! A source pack's two transforms, as typed gluon values (RFC-0048 §6, #1318).
//!
//! # The same arm, a second pair of types
//!
//! A pack's `describe` turns a parsed response into the fields of a catalog entry, and its
//! `extract` turns the same response into a reading of the artifact. Both are pure for the
//! reason a calculator is: a pack is a file somebody else wrote, and the only promise this
//! binary can make about running it is the one RFC-0042's closed prelude already makes. So
//! neither gets a prelude, a budget or a containment of its own — [`entry::admit_as`] and
//! [`super::run`] are the calculator's, with the type and the words changed.
//!
//! # A parsed document is a flat list, not a tree
//!
//! JSON, XML and CSV are three shapes of tree, and a gluon record cannot be recursive through a
//! derive. [`Parsed`] is therefore the tree written out in document order: every node is a
//! [`Field`] that names its parent by index and carries its whole path, so a script that wants
//! `message/title/0` compares one string, and a script that wants the children of a node filters
//! on one integer. A container's value is [`Value::Empty`]; its children follow it.
//!
//! What a format cannot say is not invented. XML text is its own `#text` field rather than the
//! element's value, because an element can hold text, children and more text in that order, and
//! a value that concatenated them would lose the order a citation's title depends on. An
//! attribute is a child named `@name`. A CSV row is a field named by its index whose children are
//! named by the header.

use gluon_codegen::{Getable, Pushable, VmType};

use super::entry::{self, Role};
use super::marshal::Value;

/// One node of a parsed document. See the module note for the shape.
#[derive(Debug, Clone, PartialEq, Getable, Pushable, VmType)]
pub struct Field {
    /// The index in [`Parsed::fields`] of this node's parent, or `-1` at the root.
    pub parent: i64,
    pub name: String,
    /// Every name from the root down, joined by `/`.
    pub path: String,
    pub value: Value,
}

/// What a transform is applied to: a fetched response, parsed by the host.
#[derive(Debug, Clone, PartialEq, Getable, Pushable, VmType)]
pub struct Parsed {
    /// The response's media type as the scheme's `resolve` declared it.
    pub media_type: String,
    pub fields: Vec<Field>,
}

/// One key and its value: an identifier the draft names, under the key a `then.from` reads.
#[derive(Debug, Clone, PartialEq, Getable, Pushable, VmType)]
pub struct Named {
    pub key: String,
    pub value: String,
}

/// What `describe` returns: as much of a catalog entry as the response says.
///
/// Every field is optional because a response is allowed to be silent, and the draft says which
/// fields it could not fill rather than filling them with a guess.
#[derive(Debug, Clone, PartialEq, Default, Getable, Pushable, VmType)]
pub struct EntryDraft {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    /// Further identifiers the response names, which a scheme's `then` resolves next.
    pub identifiers: Vec<Named>,
}

/// What `extract` returns: one reading of the artifact, which the host hashes and records.
#[derive(Debug, Clone, PartialEq, Getable, Pushable, VmType)]
pub struct Reading {
    pub media_type: String,
    pub text: String,
}

/// A pack's `describe` transform.
pub const DESCRIBE: Role = Role {
    noun: "a describe transform",
    signature: "Parsed -> EntryDraft",
    otherwise: "A pack that needs anything else leaves the field to the entry's author.",
};

/// A pack's `extract` transform.
pub const EXTRACT: Role = Role {
    noun: "an extract transform",
    signature: "Parsed -> Reading",
    otherwise:
        "A pack that needs anything else leaves the reading to `catalog-extract`'s own extractors.",
};

/// Admit `script` as a describe transform, without running it.
pub fn admit_describe(name: &str, script: &str) -> Result<entry::Admitted, entry::Rejected> {
    entry::admit_as::<fn(Parsed) -> EntryDraft>(name, script, &DESCRIBE)
}

/// Admit `script` as an extract transform, without running it.
pub fn admit_extract(name: &str, script: &str) -> Result<entry::Admitted, entry::Rejected> {
    entry::admit_as::<fn(Parsed) -> Reading>(name, script, &EXTRACT)
}

/// Apply a describe transform to a parsed response under a budget of `calls`.
pub fn describe(
    name: &str,
    script: &str,
    parsed: Parsed,
    calls: usize,
) -> anyhow::Result<EntryDraft> {
    let admitted = admit_describe(name, script)?;
    Ok(super::run::<Parsed, EntryDraft>(admitted, name, parsed, calls)?.0)
}

/// Apply an extract transform to a parsed response under a budget of `calls`.
pub fn extract(name: &str, script: &str, parsed: Parsed, calls: usize) -> anyhow::Result<Reading> {
    let admitted = admit_extract(name, script)?;
    Ok(super::run::<Parsed, Reading>(admitted, name, parsed, calls)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> Parsed {
        let f = |parent: i64, name: &str, path: &str, value: Value| Field {
            parent,
            name: name.into(),
            path: path.into(),
            value,
        };
        Parsed {
            media_type: "application/json".into(),
            fields: vec![
                f(-1, "title", "title", Value::Text("On Bridges".into())),
                f(-1, "year", "year", Value::Int(1931)),
                f(-1, "doi", "doi", Value::Text("10.1/x".into())),
            ],
        }
    }

    const DESCRIBE_SCRIPT: &str = "\\p ->
    let same : String -> String -> Bool = \\a b -> a == b
    let pick name acc f =
        match acc with
        | Some found -> Some found
        | None -> if same f.path name then Some f.value else None
    let text : String -> Option String = \\name ->
        match array.foldable.foldl (pick name) None p.fields with
        | Some (Text s) -> Some s
        | Some (Int i) -> Some (show.show i)
        | _ -> None
    let ids =
        match text \"doi\" with
        | Some d -> [{ key = \"doi\", value = d }]
        | None -> []
    { name = text \"title\", kind = Some \"article\", date = text \"year\", description = None, identifiers = ids }
";

    #[test]
    fn a_describe_transform_fills_what_the_response_says() {
        let draft = describe("describe.glu", DESCRIBE_SCRIPT, parsed(), 100_000).unwrap();
        assert_eq!(draft.name.as_deref(), Some("On Bridges"));
        assert_eq!(draft.kind.as_deref(), Some("article"));
        assert_eq!(draft.date.as_deref(), Some("1931"));
        assert_eq!(draft.description, None);
        assert_eq!(
            draft.identifiers,
            vec![Named {
                key: "doi".into(),
                value: "10.1/x".into()
            }]
        );
    }

    #[test]
    fn an_extract_transform_returns_a_reading() {
        let script = "\\p -> { media_type = \"text/plain\", text = p.media_type }\n";
        let reading = extract("extract.glu", script, parsed(), 10_000).unwrap();
        assert_eq!(
            reading,
            Reading {
                media_type: "text/plain".into(),
                text: "application/json".into()
            }
        );
    }

    #[test]
    fn a_calculator_is_not_a_transform_and_the_refusal_says_which_it_was_asked_to_be() {
        let calculator = "\\c -> { signals = [], summary = [] }\n";
        let e = admit_describe("describe.glu", calculator).err().unwrap();
        let entry::Rejected::Refused(r) = e else {
            panic!("expected a refusal")
        };
        assert!(r.to_string().contains("is not a describe transform"), "{r}");
        assert!(r.to_string().contains("Parsed -> EntryDraft"), "{r}");
    }

    #[test]
    fn a_transform_that_loops_is_stopped_by_the_budget() {
        let script = "\\p ->
    rec let go n = go (n + 1)
    in
    let n : Int = go 0
    { media_type = \"text/plain\", text = if n == 0 then \"\" else \"x\" }
";
        let e = extract("extract.glu", script, parsed(), 1_000).unwrap_err();
        assert!(e.to_string().contains("which is its budget"), "{e}");
    }
}
