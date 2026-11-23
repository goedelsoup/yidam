//! yidam_kit: read the graph `yidam run` hands a step, and write a file it will read back.
//!
//! RFC-0056 §2.2. A calculator is invoked with the resolved corpus at `$YIDAM_GRAPH`, in the
//! tab-separated records `yidam run` writes, and is expected to write YAML that `yidam` reads.
//! This module is both halves, once. `yidam_kit.py` is the same kit in Python, and the
//! template's tests hold the two to one another's output, byte for byte on the writer's side.
//!
//! `std` only, so a calculator crate gains no dependency. Include it by path from the vendored
//! prelude, and list `.yidam/.vendor/prelude/kit/**` in the step's `reads` so a kit update is a
//! reason to re-run:
//!
//! ```ignore
//! #[path = "../.yidam/.vendor/prelude/kit/yidam_kit.rs"]
//! mod yidam_kit;
//! use yidam_kit::{signals, Value};
//!
//! let graph = yidam_kit::read()?;
//! let rows = graph.nodes.iter().map(|n| Value::map([
//!     ("node", Value::from(n.reference.as_str())),
//!     ("links", Value::from(n.links.len() as i64)),
//! ])).collect();
//! let text = signals(rows, Some(Value::map([("rule", "count outgoing links".into())])), vec![])?;
//! ```
//!
//! The kit does not know `yidam`'s rules for a signal table: which names are reserved, and which
//! nodes exist. `yidam run` checks those against the corpus before it commits.

use std::fmt;
use std::fmt::Write as _;

/// The `$YIDAM_GRAPH` record grammar this kit reads. Another version is refused.
pub const GRAPH_FORMAT_VERSION: u32 = 1;

/// The signal-table format [`signals`] writes.
pub const SIGNALS_FORMAT_VERSION: i64 = 1;

/// A graph this kit cannot read, or a value it will not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitError(pub String);

impl fmt::Display for KitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KitError {}

fn fail<T>(message: String) -> Result<T, KitError> {
    Err(KitError(message))
}

/// A value read from the graph, or one to write.
///
/// `Int` is wider than either reader's integer so that a `u64` and an `i64` both fit; the
/// writer refuses anything outside them. `Map` keeps its keys in the order given.
/// `Opaque` is a property the graph states the kind of and gives no value for: a mapping at
/// the top level, a list inside a list, a record field that is itself nested. The writer
/// refuses it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
    Opaque(String),
}

impl Value {
    pub fn map<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Self)>) -> Self {
        Self::Map(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// A mapping's value for `key`.
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::Int(i128::from(i))
    }
}
impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Self::Float(x)
    }
}
impl From<Vec<Self>> for Value {
    fn from(v: Vec<Self>) -> Self {
        Self::List(v)
    }
}

// ── reading ─────────────────────────────────────────────────────────────────────────────

/// One `prop` record as the graph carried it: the key, the kind, and the decoded value. A
/// `seq` row's value is the item count, as the graph writes it; the list is the property.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub kind: String,
    pub value: Value,
}

/// One outgoing link, resolved by `yidam`. `exists` is whether the target is a file at the
/// commit, not whether it is in this step's view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub index: usize,
    pub relationship: String,
    pub target: String,
    pub resolved: String,
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassProp {
    pub name: String,
    pub r#type: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    pub name: String,
    pub properties: Vec<ClassProp>,
}

/// One corpus node.
///
/// `id` is repository-relative, the spelling `reads` uses. `reference` is the RFC-0032
/// reference a signal row is keyed on: `gage/canyon-outlet` for
/// `.yidam/corpus/gage/canyon-outlet.yml`. `malformed` is the parser's reason when the file did
/// not parse. A malformed node is still a node, and has no properties because none could be read.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub reference: String,
    pub class: String,
    pub label: String,
    pub malformed: Option<String>,
    pub properties: Vec<(String, Value)>,
    pub rows: Vec<Row>,
    pub links: Vec<Link>,
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    pub corpus_dir: String,
    pub classes: Vec<Class>,
    pub nodes: Vec<Node>,
}

impl Graph {
    /// A node by its id or its reference.
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes
            .iter()
            .find(|n| n.id == name || n.reference == name)
    }

    fn by_id(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}

fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mapped = match chars.peek() {
            Some('\\') => Some('\\'),
            Some('t') => Some('\t'),
            Some('n') => Some('\n'),
            Some('r') => Some('\r'),
            _ => None,
        };
        match mapped {
            Some(m) => {
                out.push(m);
                chars.next();
            }
            None => out.push('\\'),
        }
    }
    out
}

fn scalar(kind: &str, raw: &str, at: &str) -> Result<Value, KitError> {
    Ok(match kind {
        "null" => Value::Null,
        "bool" => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return fail(format!("{at}: a bool row holds {raw:?}")),
        },
        "int" => match raw.parse::<i128>() {
            Ok(i) => Value::Int(i),
            Err(_) => return fail(format!("{at}: an int row holds {raw:?}")),
        },
        "float" => match raw {
            ".inf" => Value::Float(f64::INFINITY),
            "-.inf" => Value::Float(f64::NEG_INFINITY),
            ".nan" => Value::Float(f64::NAN),
            _ => match raw.parse::<f64>() {
                Ok(x) => Value::Float(x),
                Err(_) => return fail(format!("{at}: a float row holds {raw:?}")),
            },
        },
        "string" => Value::Str(raw.to_string()),
        other => Value::Opaque(other.to_string()),
    })
}

fn reference(corpus_dir: &str, id: &str) -> String {
    let prefix = format!("{corpus_dir}/");
    let rest = if corpus_dir.is_empty() {
        id
    } else {
        id.strip_prefix(prefix.as_str()).unwrap_or(id)
    };
    rest.strip_suffix(".yml").unwrap_or(rest).to_string()
}

/// `[<i>]` or `[<i>].<field>` after a list's name.
fn item(rest: &str) -> Option<(usize, Option<&str>)> {
    let inner = rest.strip_prefix('[')?;
    let close = inner.find(']')?;
    let digits = &inner[..close];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let index = digits.parse().ok()?;
    let after = &inner[close + 1..];
    if after.is_empty() {
        Some((index, None))
    } else {
        after.strip_prefix('.').map(|f| (index, Some(f)))
    }
}

/// Fold one `prop` row into the node's properties. A sequence row is followed by its items,
/// `<key>[<i>]`, and an item that is a record by its fields, `<key>[<i>].<field>`.
fn add_prop(node: &mut Node, key: &str, kind: &str, raw: &str) -> Result<(), KitError> {
    let at = format!("{} {key}", node.id);
    // A sequence's row holds its item count; its property is the list the item rows fill.
    let (row, value) = match kind {
        "seq" if !raw.is_empty() => (scalar("int", raw, &at)?, Value::List(Vec::new())),
        "seq" | "map" => (Value::Opaque(kind.into()), Value::Opaque(kind.into())),
        _ => {
            let v = scalar(kind, raw, &at)?;
            (v.clone(), v)
        }
    };
    node.rows.push(Row {
        key: key.to_string(),
        kind: kind.to_string(),
        value: row,
    });

    for (name, held) in node.properties.iter_mut().rev() {
        let Value::List(items) = held else { continue };
        let Some(rest) = key.strip_prefix(name.as_str()) else {
            continue;
        };
        if !rest.starts_with('[') {
            continue;
        }
        let Some((index, field)) = item(rest) else {
            break;
        };
        match field {
            None => {
                if index != items.len() {
                    return fail(format!(
                        "{at}: item {index} arrived after {} items",
                        items.len()
                    ));
                }
                items.push(if kind == "map" {
                    Value::Map(Vec::new())
                } else {
                    value
                });
            }
            Some(f) => match (index + 1 == items.len(), items.last_mut()) {
                (true, Some(Value::Map(fields))) => {
                    fields.push((f.to_string(), value));
                }
                _ => return fail(format!("{at}: a field of an item that is not a record")),
            },
        }
        return Ok(());
    }
    node.properties.push((key.to_string(), value));
    Ok(())
}

/// Read the graph from its text. See [`read`].
pub fn parse(text: &str) -> Result<Graph, KitError> {
    let mut graph = Graph::default();
    let mut version = false;
    for (number, line) in text.split('\n').enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<String> = line.split('\t').map(unescape).collect();
        let kind = f[0].as_str();
        if !version && kind != "format_version" {
            return fail(format!(
                "line {}: the graph must open with format_version",
                number + 1
            ));
        }
        match (kind, f.len()) {
            ("format_version", _) => {
                if f[1..] != [GRAPH_FORMAT_VERSION.to_string()] {
                    return fail(format!(
                        "the graph is format_version {} and this kit reads \
                         {GRAPH_FORMAT_VERSION}. Update the kit",
                        f[1..].join("\t")
                    ));
                }
                version = true;
            }
            ("corpus_dir", 2..) => graph.corpus_dir = f[1].clone(),
            ("class", 2..) => graph.classes.push(Class {
                name: f[1].clone(),
                properties: Vec::new(),
            }),
            ("class_prop", 5..) => {
                for c in graph.classes.iter_mut().filter(|c| c.name == f[1]) {
                    c.properties.push(ClassProp {
                        name: f[2].clone(),
                        r#type: f[3].clone(),
                        required: f[4] == "true",
                    });
                }
            }
            ("node", 4..) => {
                let reference = reference(&graph.corpus_dir, &f[1]);
                graph.nodes.push(Node {
                    id: f[1].clone(),
                    reference,
                    class: f[2].clone(),
                    label: f[3].clone(),
                    malformed: None,
                    properties: Vec::new(),
                    rows: Vec::new(),
                    links: Vec::new(),
                });
            }
            ("malformed", 3..) => {
                if let Some(n) = graph.by_id(&f[1]) {
                    n.malformed = Some(f[2].clone());
                }
            }
            ("prop", 5..) => {
                if let Some(n) = graph.by_id(&f[1]) {
                    add_prop(n, &f[2], &f[3], &f[4])?;
                }
            }
            ("link", 7..) => {
                let Ok(index) = f[2].parse() else {
                    return fail(format!("line {}: a link index is {:?}", number + 1, f[2]));
                };
                if let Some(n) = graph.by_id(&f[1]) {
                    n.links.push(Link {
                        index,
                        relationship: f[3].clone(),
                        target: f[4].clone(),
                        resolved: f[5].clone(),
                        exists: f[6] == "true",
                    });
                }
            }
            // Any other kind is one this kit has not heard of, and the format says to skip it.
            _ => {}
        }
    }
    if !version {
        return fail("the graph carries no format_version".into());
    }
    Ok(graph)
}

/// Read the resolved corpus `yidam run` handed this step, from `$YIDAM_GRAPH`.
///
/// A step whose `reads` admit no corpus node is handed no graph, and this says so rather than
/// returning an empty one.
pub fn read() -> Result<Graph, KitError> {
    let path = match std::env::var("YIDAM_GRAPH") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return fail(
                "no resolved corpus: $YIDAM_GRAPH is unset. Does this step's `reads` cover \
                 .yidam/corpus?"
                    .into(),
            )
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(e) => fail(format!("{path}: {e}")),
    }
}

// ── writing ─────────────────────────────────────────────────────────────────────────────

const WORDS: &[&str] = &["true", "false", "null", "yes", "no", "on", "off", "y", "n"];

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        let o = u32::from(c);
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ if o < 0x20 || (0x7F..=0x9F).contains(&o) => {
                let _ = write!(out, "\\x{o:02x}");
            }
            // Not controls, and escaped all the same: no YAML reader accepts U+FFFE or U+FFFF
            // raw, and YAML 1.1 reads U+2028 and U+2029 as line breaks, which fold to a space
            // inside a quoted string.
            '\u{2028}' | '\u{2029}' | '\u{fffe}' | '\u{ffff}' => {
                let _ = write!(out, "\\u{o:04x}");
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The shortest digits that round-trip, laid out the way Python's `repr` lays them out, with a
/// `.` always present. So `12.0`, `0.0001`, `1.5e-05` and `1.0e+16`.
fn float(x: f64) -> Result<String, KitError> {
    if !x.is_finite() {
        return fail(format!(
            "{x} is not a finite float. JSON cannot carry it; write a string if you mean an \
             unbounded value"
        ));
    }
    let sci = format!("{x:e}");
    let (sign, sci) = match sci.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", sci.as_str()),
    };
    let Some((mantissa, exp)) = sci.split_once('e') else {
        return fail(format!("{x} did not format with an exponent"));
    };
    let Ok(exp) = exp.parse::<i32>() else {
        return fail(format!("{x} formatted with exponent {exp:?}"));
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if x == 0.0 {
        return Ok(format!("{sign}0.0"));
    }
    Ok(if !(-4..16).contains(&exp) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        let e_sign = if exp < 0 { '-' } else { '+' };
        format!("{sign}{first}.{rest}e{e_sign}{:02}", exp.abs())
    } else if exp < 0 {
        let zeros = "0".repeat(exp.unsigned_abs() as usize - 1);
        format!("{sign}0.{zeros}{digits}")
    } else {
        let whole = exp as usize + 1;
        if digits.len() <= whole {
            let zeros = "0".repeat(whole - digits.len());
            format!("{sign}{digits}{zeros}.0")
        } else {
            format!("{sign}{}.{}", &digits[..whole], &digits[whole..])
        }
    })
}

fn bare(k: &str) -> bool {
    let mut bytes = k.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_');
    first_ok
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && !WORDS.contains(&k.to_ascii_lowercase().as_str())
}

fn key(k: &str) -> String {
    if bare(k) {
        k.to_string()
    } else {
        quote(k)
    }
}

/// A value's text when it fits on its key's line, or `None` for a non-empty map or list.
fn inline(v: &Value) -> Result<Option<String>, KitError> {
    Ok(Some(match v {
        Value::Null => "null".into(),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Int(i) => {
            if *i < i128::from(i64::MIN) || *i > i128::from(u64::MAX) {
                return fail(format!("{i} does not fit in 64 bits"));
            }
            i.to_string()
        }
        Value::Float(x) => float(*x)?,
        Value::Str(s) => quote(s),
        Value::Map(m) if m.is_empty() => "{}".into(),
        Value::List(l) if l.is_empty() => "[]".into(),
        Value::Map(_) | Value::List(_) => return Ok(None),
        Value::Opaque(kind) => {
            return fail(format!(
                "an opaque {kind} has no value to write. Read the node's file for it"
            ))
        }
    }))
}

fn block(v: &Value, indent: usize, out: &mut Vec<String>) -> Result<(), KitError> {
    let pad = " ".repeat(indent);
    match v {
        Value::Map(m) => {
            for (k, x) in m {
                match inline(x)? {
                    Some(s) => out.push(format!("{pad}{}: {s}", key(k))),
                    None => {
                        out.push(format!("{pad}{}:", key(k)));
                        block(x, indent + 2, out)?;
                    }
                }
            }
        }
        Value::List(l) => {
            for x in l {
                match inline(x)? {
                    Some(s) => out.push(format!("{pad}- {s}")),
                    None => {
                        let start = out.len();
                        block(x, indent + 2, out)?;
                        if let Some(first) = out.get_mut(start) {
                            *first = format!("{pad}- {}", &first[indent + 2..]);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Block YAML for a tree of maps, lists and scalars, ending in a newline.
///
/// Every string is double-quoted, so a string reads back as the string it was in every YAML
/// reader: `00:00`, `yes`, `a: b` and `#x` included. A float always carries a `.`. A key is bare
/// only when no YAML schema reads it as anything but a string.
pub fn dump(value: &Value) -> Result<String, KitError> {
    if let Some(s) = inline(value)? {
        return Ok(format!("{s}\n"));
    }
    let mut out = Vec::new();
    block(value, 0, &mut out)?;
    Ok(out.join("\n") + "\n")
}

/// A signal table: `format_version`, then `method` when given, `signals`, then `fields`.
///
/// Every row is a mapping with a string `node:`, the RFC-0032 reference of the node it is about
/// (a [`Node`]'s `reference`). The other keys of a row are the signals.
pub fn signals(
    rows: Vec<Value>,
    method: Option<Value>,
    fields: Vec<(String, Value)>,
) -> Result<String, KitError> {
    for (i, row) in rows.iter().enumerate() {
        if row.get("node").and_then(Value::as_str).is_none() {
            return fail(format!(
                "signal row {i} has no `node:` naming the node it is about"
            ));
        }
    }
    let mut table = vec![(
        "format_version".to_string(),
        Value::Int(i128::from(SIGNALS_FORMAT_VERSION)),
    )];
    if let Some(m) = method {
        table.push(("method".into(), m));
    }
    table.push(("signals".into(), Value::List(rows)));
    for (k, v) in fields {
        // Every name signals() owns, whether or not it wrote that one this time: a `method`
        // passed as a field would read as the method this table was computed by.
        let owned = ["format_version", "method", "signals"].contains(&k.as_str());
        if owned || table.iter().any(|(t, _)| *t == k) {
            return fail(format!("`{k}` is written by signals() itself"));
        }
        table.push((k, v));
    }
    dump(&Value::Map(table))
}
