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
//!
//! A step that reads a table a catalog entry declares reads it with [`table`], which types its
//! cells by the declaration `yidam run` names in `$YIDAM_TABLES` (RFC-0059 §2.4).

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

// ── tables ──────────────────────────────────────────────────────────────────────────────

/// The `$YIDAM_TABLES` document, read as far as [`parse_table`] needs it.
///
/// A JSON reader of its own because the kit takes no dependency. It reads any JSON, and keeps an
/// object's keys in the order written.
struct Json<'a> {
    s: &'a [u8],
    at: usize,
}

impl Json<'_> {
    fn ws(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn fail<T>(&self, what: &str) -> Result<T, KitError> {
        fail(format!("$YIDAM_TABLES, byte {}: {what}", self.at))
    }

    fn eat(&mut self, b: u8) -> Result<(), KitError> {
        self.ws();
        if self.s.get(self.at) == Some(&b) {
            self.at += 1;
            Ok(())
        } else {
            self.fail(&format!("expected `{}`", b as char))
        }
    }

    fn value(&mut self) -> Result<Value, KitError> {
        self.ws();
        match self.s.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut m = Vec::new();
                self.ws();
                if self.s.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Ok(Value::Map(m));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    m.push((k, self.value()?));
                    self.ws();
                    match self.s.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Value::Map(m));
                        }
                        _ => return self.fail("expected `,` or `}`"),
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut l = Vec::new();
                self.ws();
                if self.s.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Ok(Value::List(l));
                }
                loop {
                    l.push(self.value()?);
                    self.ws();
                    match self.s.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Value::List(l));
                        }
                        _ => return self.fail("expected `,` or `]`"),
                    }
                }
            }
            Some(b'"') => self.string().map(Value::Str),
            Some(_) => {
                let start = self.at;
                while self.at < self.s.len() && !b",]} \t\r\n".contains(&self.s[self.at]) {
                    self.at += 1;
                }
                match &self.s[start..self.at] {
                    b"null" => Ok(Value::Null),
                    b"true" => Ok(Value::Bool(true)),
                    b"false" => Ok(Value::Bool(false)),
                    word => match std::str::from_utf8(word).ok().and_then(|w| w.parse().ok()) {
                        Some(x) => Ok(Value::Float(x)),
                        None => self.fail("expected a value"),
                    },
                }
            }
            None => self.fail("expected a value"),
        }
    }

    fn hex4(&mut self) -> Result<u32, KitError> {
        let digits = self
            .s
            .get(self.at..self.at + 4)
            .and_then(|h| std::str::from_utf8(h).ok());
        match digits.and_then(|h| u32::from_str_radix(h, 16).ok()) {
            Some(u) => {
                self.at += 4;
                Ok(u)
            }
            None => self.fail("a `\\u` escape needs four hex digits"),
        }
    }

    fn string(&mut self) -> Result<String, KitError> {
        if self.s.get(self.at) != Some(&b'"') {
            return self.fail("expected a string");
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            match self.s.get(self.at) {
                None => return self.fail("an unterminated string"),
                Some(b'"') => {
                    self.at += 1;
                    break;
                }
                Some(b'\\') => {
                    self.at += 1;
                    let c = match self.s.get(self.at) {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            self.at += 1;
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi)
                                && self.s.get(self.at..self.at + 2) == Some(b"\\u")
                            {
                                self.at += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF)
                            } else {
                                hi
                            };
                            let c = char::from_u32(code).unwrap_or('\u{FFFD}');
                            out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                            continue;
                        }
                        _ => return self.fail("an unknown escape"),
                    };
                    self.at += 1;
                    out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
                Some(&b) => {
                    self.at += 1;
                    out.push(b);
                }
            }
        }
        String::from_utf8(out).or_else(|_| self.fail("a string that is not UTF-8"))
    }
}

/// The rows of a CSV file, as RFC 4180 reads them: a quoted cell may hold a comma, a newline, or
/// a doubled quote. A blank line is no record, as it is to `yidam`'s reader.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut touched = false;
    let mut chars = text.chars().peekable();
    let mut end = |row: &mut Vec<String>, cell: &mut String, touched: &mut bool| {
        row.push(std::mem::take(cell));
        if *touched || row.len() > 1 || !row[0].is_empty() {
            rows.push(std::mem::take(row));
        } else {
            row.clear();
        }
        *touched = false;
    };
    while let Some(c) = chars.next() {
        if quoted {
            if c != '"' {
                cell.push(c);
            } else if chars.peek() == Some(&'"') {
                cell.push('"');
                chars.next();
            } else {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => {
                quoted = true;
                touched = true;
            }
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' | '\n' => end(&mut row, &mut cell, &mut touched),
            _ => cell.push(c),
        }
    }
    if touched || !row.is_empty() || !cell.is_empty() {
        end(&mut row, &mut cell, &mut touched);
    }
    rows
}

/// The rows of a table file, the header first, read as `yidam`'s own reader reads them.
fn table_lines(text: &str, format: &str) -> Vec<Vec<String>> {
    // A leading byte-order mark is not part of the first cell, to `yidam` either.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if format == "csv" {
        return csv_rows(text);
    }
    let mut lines: Vec<Vec<String>> = text
        .split('\n')
        .map(|r| {
            let r = r.strip_suffix('\r').unwrap_or(r);
            r.split('\t').map(str::to_string).collect()
        })
        .collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn field<'v>(decl: &'v Value, key: &str) -> Result<&'v Value, KitError> {
    match decl.get(key) {
        Some(v) => Ok(v),
        None => fail(format!("$YIDAM_TABLES: a declaration has no `{key}`")),
    }
}

fn text_field<'v>(decl: &'v Value, key: &str) -> Result<&'v str, KitError> {
    match field(decl, key)?.as_str() {
        Some(s) => Ok(s),
        None => fail(format!(
            "$YIDAM_TABLES: a declaration's `{key}` is not a string"
        )),
    }
}

/// Read one table's text by its declaration in `declarations`, the `$YIDAM_TABLES` document.
///
/// One [`Value::Map`] per row, in the declaration's column order. A `number` cell is a
/// [`Value::Float`], an empty cell is [`Value::Null`], and every other cell is its text, trimmed.
/// A path no declaration names is refused, so an undeclared file is not read as if it were typed.
pub fn parse_table(declarations: &str, path: &str, text: &str) -> Result<Vec<Value>, KitError> {
    let doc = Json {
        s: declarations.as_bytes(),
        at: 0,
    }
    .value()?;
    let Value::List(decls) = doc else {
        return fail("$YIDAM_TABLES does not hold a list of declarations".into());
    };
    let mut decl = None;
    for d in &decls {
        if text_field(d, "path")? == path {
            decl = Some(d);
            break;
        }
    }
    let Some(decl) = decl else {
        return fail(format!(
            "{path}: no table declaration in $YIDAM_TABLES names it. Does a catalog entry declare \
             its `columns:`, and does this step's `reads` cover the file?"
        ));
    };
    let entry = text_field(decl, "entry")?;
    let Value::List(columns) = field(decl, "columns")? else {
        return fail(format!(
            "$YIDAM_TABLES: `{entry}`'s `columns` is not a list"
        ));
    };
    let mut names = Vec::new();
    let mut numeric = Vec::new();
    for c in columns {
        names.push(text_field(c, "name")?.to_string());
        numeric.push(text_field(c, "type")? == "number");
    }
    let lines = table_lines(text, text_field(decl, "format")?);
    let header: Vec<String> = lines
        .first()
        .map(|h| h.iter().map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();
    if header != names {
        return fail(format!(
            "{path}: the header reads `{}`, and `{entry}` declares `{}`",
            header.join(" "),
            names.join(" ")
        ));
    }
    let mut rows = Vec::new();
    for (i, cells) in lines.iter().enumerate().skip(1) {
        let number = i + 1;
        if cells.len() != names.len() {
            return fail(format!(
                "{path}: row {number} has {} cells, and `{entry}` declares {} columns",
                cells.len(),
                names.len()
            ));
        }
        let mut row = Vec::new();
        for ((name, &is_number), cell) in names.iter().zip(&numeric).zip(cells) {
            let v = cell.trim();
            let value = if v.is_empty() {
                Value::Null
            } else if is_number {
                // `yidam`'s rule: a finite decimal.
                match v.parse::<f64>().ok().filter(|x| x.is_finite()) {
                    Some(x) => Value::Float(x),
                    None => {
                        return fail(format!(
                            "{path}: row {number}, column `{name}`: `{v}` is not a number"
                        ))
                    }
                }
            } else {
                Value::Str(v.to_string())
            };
            row.push((name.clone(), value));
        }
        rows.push(Value::Map(row));
    }
    Ok(rows)
}

/// Read the declared table at `path`, repository-relative as its entry's `file` location.
///
/// The declarations are read from `$YIDAM_TABLES` and the file from under `$YIDAM_IN`, both of
/// which `yidam run` sets. See [`parse_table`] for what a row is.
pub fn table(path: &str) -> Result<Vec<Value>, KitError> {
    let tables =
        match std::env::var("YIDAM_TABLES") {
            Ok(p) if !p.is_empty() => p,
            _ => return fail(
                "no table declarations: $YIDAM_TABLES is unset. Does this step's `reads` cover \
                 a table a catalog entry declares?"
                    .into(),
            ),
        };
    let root = std::env::var("YIDAM_IN").unwrap_or_else(|_| ".".into());
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p).or_else(|e| fail(format!("{}: {e}", p.display())))
    };
    let declarations = read(std::path::Path::new(&tables))?;
    let text = read(&std::path::Path::new(&root).join(path))?;
    parse_table(&declarations, path, &text)
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
