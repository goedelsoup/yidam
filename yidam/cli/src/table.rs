//! A declared table (RFC-0059): a tracked TSV or CSV that a `dataset` catalog entry names by a
//! `file` location and describes with `columns:` and `key:`.
//!
//! **One reader.** `lint`, `yidam run` and the `#row=` citation all read a table through
//! [`read`] and hold it to its declaration through [`shape`], so a file `lint` passes is a file
//! the other two read the same way (RFC-0059 §2.2).
//!
//! **Read only on a dataset.** `columns:` and `key:` sit in the frontmatter every markdown kind
//! shares, typed as YAML values. [`declaration`] interprets them only on an entry whose `type:`
//! is `dataset`, and on no other file.

use std::collections::HashMap;

use crate::corpus::Source;

/// The types a column may declare. A cell is one line and holds no standing of its own, so
/// `text`, `claim`, `quotation` and `list` are refused.
pub(crate) const COLUMN_TYPES: [&str; 4] = ["string", "number", "date", "ref"];

/// The property types a column may not take, named apart so the finding can say why.
const REFUSED_TYPES: [&str; 4] = ["text", "claim", "quotation", "list"];

/// How many shape findings one file reports before it reports a count instead.
pub(crate) const SHAPE_CAP: usize = 20;

/// The two file formats a table may be written in, chosen by extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Format {
    Tsv,
    Csv,
}

impl Format {
    pub(crate) fn of(path: &str) -> Option<Self> {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".tsv") {
            Some(Self::Tsv)
        } else if lower.ends_with(".csv") {
            Some(Self::Csv)
        } else {
            None
        }
    }
}

/// One declared column.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(crate) struct Column {
    pub name: String,
    /// One of [`COLUMN_TYPES`].
    pub r#type: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// A table as its catalog entry declares it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(crate) struct Declaration {
    /// The entry's stem, the name a node links to.
    pub entry: String,
    /// The entry's repository-relative path.
    #[serde(skip)]
    pub entry_rel: String,
    /// The table file, as the `file` location writes it: relative to the repository root.
    pub path: String,
    pub format: Format,
    pub columns: Vec<Column>,
    /// The key columns, by name, in `key:` order. Empty when the entry declares no key.
    pub key: Vec<String>,
}

impl Declaration {
    fn key_indices(&self) -> Vec<usize> {
        self.key
            .iter()
            .filter_map(|k| self.columns.iter().position(|c| &c.name == k))
            .collect()
    }
}

/// The entry's declaration, if it declares a table.
///
/// `None` for every entry that is not a `dataset` carrying `columns:`, which is every entry
/// written before RFC-0059. `Some(Err)` lists everything wrong with the declaration, and a
/// malformed declaration is not used to check the file. The file's existence and tracking are
/// the caller's to check, because they need the repository.
pub(crate) fn declaration(source: &Source) -> Option<Result<Declaration, Vec<String>>> {
    if source.r#type.as_deref() != Some("dataset") {
        return None;
    }
    let columns = source.columns.as_ref()?;
    let mut why = Vec::new();

    let mut parsed: Vec<Column> = Vec::new();
    match columns {
        serde_yaml::Value::Sequence(items) if !items.is_empty() => {
            for (i, item) in items.iter().enumerate() {
                match column(item) {
                    Ok(c) => {
                        if parsed.iter().any(|p| p.name == c.name) {
                            why.push(format!("declares the column `{}` twice", c.name));
                        }
                        parsed.push(c);
                    }
                    Err(e) => why.push(format!("column {}: {e}", i + 1)),
                }
            }
        }
        _ => why.push("`columns:` is not a non-empty list of columns".to_string()),
    }

    let key: Vec<String> = match &source.key {
        None => Vec::new(),
        Some(serde_yaml::Value::String(s)) => vec![s.clone()],
        Some(serde_yaml::Value::Sequence(items)) => {
            let names: Vec<String> = items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if names.len() != items.len() || names.is_empty() {
                why.push("`key:` is not a column name or a non-empty list of them".to_string());
            }
            names
        }
        Some(_) => {
            why.push("`key:` is not a column name or a non-empty list of them".to_string());
            Vec::new()
        }
    };
    for k in &key {
        if !parsed.iter().any(|c| &c.name == k) && why.iter().all(|w| !w.starts_with("column")) {
            why.push(format!(
                "`key:` names `{k}`, which is not a declared column"
            ));
        }
    }
    for (i, k) in key.iter().enumerate() {
        if key[..i].contains(k) {
            why.push(format!("`key:` names `{k}` twice"));
        }
    }

    let files: Vec<&str> = source
        .locations
        .iter()
        .filter(|l| l.kind.as_deref() == Some("file"))
        .filter_map(|l| l.value.as_deref())
        .collect();
    let path = match files.as_slice() {
        [one] => Some(one.trim().to_string()),
        [] => {
            why.push("has no `file` location naming the table".to_string());
            None
        }
        many => {
            why.push(format!(
                "has {} `file` locations, and a table is one file",
                many.len()
            ));
            None
        }
    };
    let format = path.as_deref().and_then(|p| {
        let f = Format::of(p);
        if f.is_none() {
            why.push(format!("`{p}` does not end `.tsv` or `.csv`"));
        }
        f
    });

    if !why.is_empty() {
        return Some(Err(why));
    }
    let (Some(path), Some(format)) = (path, format) else {
        return Some(Err(why));
    };
    for c in &mut parsed {
        if key.contains(&c.name) {
            c.required = true;
        }
    }
    Some(Ok(Declaration {
        entry: stem(&source.rel),
        entry_rel: source.rel.clone(),
        path,
        format,
        columns: parsed,
        key,
    }))
}

fn stem(rel: &str) -> String {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    base.strip_suffix(".md").unwrap_or(base).to_string()
}

fn column(item: &serde_yaml::Value) -> Result<Column, String> {
    let serde_yaml::Value::Mapping(m) = item else {
        return Err("is not a mapping with a `name` and a `type`".to_string());
    };
    let get = |k: &str| m.get(serde_yaml::Value::String(k.to_string()));
    let name = match get("name") {
        Some(serde_yaml::Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Err("has no `name`".to_string()),
    };
    let ty = match get("type") {
        Some(serde_yaml::Value::String(s)) => s.trim().to_string(),
        _ => return Err(format!("`{name}` has no `type`")),
    };
    if REFUSED_TYPES.contains(&ty.as_str()) {
        return Err(format!(
            "`{name}` is `{ty}`, which a cell cannot hold — a column is one of {}",
            COLUMN_TYPES.join(", ")
        ));
    }
    if !COLUMN_TYPES.contains(&ty.as_str()) {
        return Err(format!(
            "`{name}` is `{ty}`, which is not a column type — the set is {}",
            COLUMN_TYPES.join(", ")
        ));
    }
    let required = match get("required") {
        None => false,
        Some(serde_yaml::Value::Bool(b)) => *b,
        Some(_) => return Err(format!("`{name}`'s `required` is not true or false")),
    };
    let unit = match get("unit") {
        None => None,
        Some(serde_yaml::Value::String(s)) => Some(s.clone()),
        Some(_) => return Err(format!("`{name}`'s `unit` is not text")),
    };
    let values = match get("values") {
        None => None,
        Some(serde_yaml::Value::Sequence(vs)) if ty == "string" => {
            let out: Vec<String> = vs
                .iter()
                .filter_map(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.clone()),
                    serde_yaml::Value::Number(n) => Some(n.to_string()),
                    serde_yaml::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            if out.len() != vs.len() || out.is_empty() {
                return Err(format!("`{name}`'s `values` is not a list of scalars"));
            }
            Some(out)
        }
        Some(_) if ty != "string" => {
            return Err(format!(
                "`{name}` declares `values`, which only a `string` column takes"
            ));
        }
        Some(_) => return Err(format!("`{name}`'s `values` is not a list")),
    };
    Ok(Column {
        name,
        r#type: ty,
        required,
        unit,
        values,
    })
}

/// One row of a table file: its 1-based line, and its cells as text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub line: usize,
    pub cells: Vec<String>,
}

/// A table file, read without its declaration.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Rows {
    pub header: Vec<String>,
    pub rows: Vec<Row>,
}

/// Read `text` as a table in `format`.
///
/// TSV is plain tab-separated text with no quoting: a line is a row and a tab ends a cell. CSV
/// is RFC 4180, so a quoted cell may hold a comma. A trailing newline ends the last row rather
/// than starting an empty one. `Err` names the line a CSV reader could not read past.
pub(crate) fn read(text: &str, format: Format) -> Result<Rows, String> {
    let mut lines: Vec<(usize, Vec<String>)> = Vec::new();
    match format {
        Format::Tsv => {
            for (i, raw) in text.split('\n').enumerate() {
                let line = raw.strip_suffix('\r').unwrap_or(raw);
                lines.push((i + 1, line.split('\t').map(str::to_string).collect()));
            }
            if text.ends_with('\n') {
                lines.pop();
            }
        }
        Format::Csv => {
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .flexible(true)
                .from_reader(text.as_bytes());
            for record in reader.records() {
                let record = record.map_err(|e| {
                    let at = e
                        .position()
                        .map(|p| format!("line {}: ", p.line()))
                        .unwrap_or_default();
                    format!("{at}{e}")
                })?;
                let line = record.position().map(|p| p.line() as usize).unwrap_or(0);
                lines.push((line, record.iter().map(str::to_string).collect()));
            }
        }
    }
    let mut it = lines.into_iter();
    let Some((_, header)) = it.next() else {
        return Ok(Rows::default());
    };
    Ok(Rows {
        header,
        rows: it.map(|(line, cells)| Row { line, cells }).collect(),
    })
}

/// One way a table file departs from its declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Departure {
    pub line: usize,
    pub column: Option<String>,
    pub detail: String,
}

impl std::fmt::Display for Departure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.column {
            Some(c) => write!(f, "line {}, column `{c}`: {}", self.line, self.detail),
            None => write!(f, "line {}: {}", self.line, self.detail),
        }
    }
}

/// Every way `rows` departs from `decl`: the header, a row's width, a required cell, a cell's
/// type, a closed value and a repeated key. A header that does not match is the only finding,
/// because no row can be read against columns the file does not have.
pub(crate) fn shape(decl: &Declaration, rows: &Rows) -> Vec<Departure> {
    let names: Vec<&str> = decl.columns.iter().map(|c| c.name.as_str()).collect();
    let header: Vec<&str> = rows.header.iter().map(|h| h.trim()).collect();
    if header != names {
        let detail = if rows.header.is_empty() {
            format!(
                "the file is empty, and the header should read `{}`",
                names.join(" ")
            )
        } else {
            format!(
                "the header reads `{}`, and the declaration lists `{}` in that order",
                header.join(" "),
                names.join(" ")
            )
        };
        return vec![Departure {
            line: 1,
            column: None,
            detail,
        }];
    }
    let mut out = Vec::new();
    let key = decl.key_indices();
    let mut seen: HashMap<Vec<&str>, usize> = HashMap::new();
    for row in &rows.rows {
        if row.cells.len() != decl.columns.len() {
            out.push(Departure {
                line: row.line,
                column: None,
                detail: format!(
                    "has {} cells, and the table has {} columns",
                    row.cells.len(),
                    decl.columns.len()
                ),
            });
            continue;
        }
        for (col, cell) in decl.columns.iter().zip(&row.cells) {
            if let Some(detail) = cell_departure(col, cell) {
                out.push(Departure {
                    line: row.line,
                    column: Some(col.name.clone()),
                    detail,
                });
            }
        }
        if !key.is_empty() {
            let k: Vec<&str> = key.iter().map(|&i| row.cells[i].as_str()).collect();
            if k.iter().all(|v| !v.is_empty()) {
                if let Some(first) = seen.get(&k) {
                    out.push(Departure {
                        line: row.line,
                        column: None,
                        detail: format!(
                            "repeats the key `{}` first held on line {first}",
                            k.join(",")
                        ),
                    });
                } else {
                    seen.insert(k, row.line);
                }
            }
        }
    }
    out
}

/// Why `cell` is not a value of `col`, or `None` when it is one.
///
/// The property grammar applied to text: a `number` reads through
/// [`crate::cmd::lint::checks::numeric_value`] and a `date` through
/// [`crate::cmd::lint::checks::iso_date_parts`], so a cell and a property of the same type
/// admit the same values. The empty cell is the absence of a value.
fn cell_departure(col: &Column, cell: &str) -> Option<String> {
    let v = cell.trim();
    if v.is_empty() {
        return col
            .required
            .then(|| "is empty, and the column is required".to_string());
    }
    match col.r#type.as_str() {
        "number" if crate::cmd::lint::checks::numeric_value(v).is_none() => {
            Some(format!("`{v}` is not a number"))
        }
        "date" if crate::cmd::lint::checks::iso_date_parts(v).is_none() => Some(format!(
            "`{v}` is not a date — write `YYYY-MM-DD`, or `YYYY-MM` or `YYYY`"
        )),
        "string" => match &col.values {
            Some(vs) if !vs.iter().any(|x| x == v) => Some(format!(
                "`{v}` is not one of the column's values — {}",
                vs.iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Every `ref` cell, with its line and column: what `table-ref-unresolved` resolves.
pub(crate) fn refs<'d>(decl: &'d Declaration, rows: &'d Rows) -> Vec<(usize, &'d str, &'d str)> {
    let mut out = Vec::new();
    for r in rows
        .rows
        .iter()
        .filter(|r| r.cells.len() == decl.columns.len())
    {
        for (col, cell) in decl.columns.iter().zip(&r.cells) {
            let v = cell.trim();
            if col.r#type == "ref" && !v.is_empty() {
                out.push((r.line, col.name.as_str(), v));
            }
        }
    }
    out
}

/// Whether `rows` holds a row whose key cells read `values`, compared as raw text.
///
/// Raw text on purpose (RFC-0059 §2.1): a row is cited by what its cells say, so `3` and
/// `3.0` are two keys whatever their column's type.
pub(crate) fn has_row(decl: &Declaration, rows: &Rows, values: &[String]) -> bool {
    let key = decl.key_indices();
    rows.rows.iter().any(|r| {
        r.cells.len() == decl.columns.len()
            && key.len() == values.len()
            && key.iter().zip(values).all(|(&i, v)| r.cells[i].trim() == v)
    })
}

/// Every well-formed declaration `sources` make.
///
/// A malformed one is `lint`'s to report. A step reads its file as an undeclared one, which is
/// how every table was read before RFC-0059.
pub(crate) fn declarations(sources: &[Source]) -> Vec<Declaration> {
    sources
        .iter()
        .filter(|s| s.malformed.is_none())
        .filter_map(declaration)
        .filter_map(Result::ok)
        .collect()
}

/// What `decl`'s file departs from, read from its bytes.
///
/// The read `yidam run` makes on each side of a step. A file that is not UTF-8, or not readable
/// as its format, is one departure at line 0.
pub(crate) fn departures(decl: &Declaration, bytes: &[u8]) -> Vec<Departure> {
    let whole = |detail: String| {
        vec![Departure {
            line: 0,
            column: None,
            detail,
        }]
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return whole("is not UTF-8".to_string());
    };
    match read(text, decl.format) {
        Ok(rows) => shape(decl, &rows),
        Err(e) => whole(format!("is not readable as {:?}: {e}", decl.format)),
    }
}

/// The `$YIDAM_TABLES` document: the declarations whose file `covered` admits, in path order.
///
/// `None` where it admits none, so a step that reads no declared table is handed no document and
/// its input state does not move (RFC-0059 §2.4). The one function `yidam run` writes the file
/// with and `doctor` restates its digest with.
pub(crate) fn document(decls: &[Declaration], covered: &dyn Fn(&str) -> bool) -> Option<String> {
    let mut read: Vec<&Declaration> = decls.iter().filter(|d| covered(&d.path)).collect();
    if read.is_empty() {
        return None;
    }
    read.sort_by(|a, b| a.path.cmp(&b.path));
    serde_json::to_string_pretty(&read).ok().map(|s| s + "\n")
}

/// The key values a `#row=` fragment names, or `None` when the fragment is not one.
///
/// Split on `,` before any value is decoded, so a `,` inside a value is written `%2C`. A
/// value that does not decode as UTF-8 is kept as written.
pub(crate) fn row_fragment(fragment: &str) -> Option<Vec<String>> {
    let rest = fragment.strip_prefix("row=")?;
    Some(rest.split(',').map(percent_decode).collect())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(yaml_columns: &str, key: Option<&str>, file: &str) -> Source {
        Source {
            rel: ".yidam/catalog/receipts.md".to_string(),
            path: std::path::PathBuf::from(".yidam/catalog/receipts.md"),
            obtained: true,
            r#type: Some("dataset".to_string()),
            used_by: None,
            locations: vec![crate::parse::CatalogLocation {
                kind: Some("file".to_string()),
                value: Some(file.to_string()),
                ..Default::default()
            }],
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
            columns: Some(serde_yaml::from_str(yaml_columns).unwrap()),
            key: key.map(|k| serde_yaml::from_str(k).unwrap()),
            text: String::new(),
            malformed: None,
        }
    }

    const COLS: &str = "
- { name: filing_id, type: string, required: true }
- { name: line, type: number }
- { name: kind, type: string, values: [cash, inkind] }
- { name: received, type: date }
";

    fn decl() -> Declaration {
        declaration(&source(COLS, Some("[filing_id, line]"), "data/r.tsv"))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn a_dataset_with_columns_declares_a_table_and_its_key_is_required() {
        let d = decl();
        assert_eq!(d.entry, "receipts");
        assert_eq!(d.format, Format::Tsv);
        assert_eq!(d.key, vec!["filing_id", "line"]);
        assert!(d.columns[1].required, "a key column is required");
    }

    #[test]
    fn a_declaration_reports_each_refused_shape() {
        for (cols, key, file, want) in [
            ("[{ name: a, type: text }]", None, "t.tsv", "cannot hold"),
            ("[{ name: a, type: claim }]", None, "t.tsv", "cannot hold"),
            (
                "[{ name: a, type: blob }]",
                None,
                "t.tsv",
                "not a column type",
            ),
            (
                "[{ name: a, type: string }, { name: a, type: number }]",
                None,
                "t.tsv",
                "twice",
            ),
            (
                "[{ name: a, type: string }]",
                Some("b"),
                "t.tsv",
                "not a declared column",
            ),
            (
                "[{ name: a, type: number, values: [1] }]",
                None,
                "t.tsv",
                "only a `string`",
            ),
            (
                "[{ name: a, type: string }]",
                None,
                "t.xlsx",
                "does not end",
            ),
            ("3", None, "t.tsv", "non-empty list"),
        ] {
            let got = declaration(&source(cols, key, file)).unwrap().unwrap_err();
            assert!(got.iter().any(|w| w.contains(want)), "{cols}: {got:?}");
        }
    }

    #[test]
    fn two_file_locations_are_refused() {
        let mut s = source(COLS, None, "a.tsv");
        s.locations.push(s.locations[0].clone());
        let got = declaration(&s).unwrap().unwrap_err();
        assert!(
            got.iter().any(|w| w.contains("2 `file` locations")),
            "{got:?}"
        );
    }

    #[test]
    fn the_keys_are_read_only_on_a_dataset() {
        let mut s = source("3", Some("primary"), "a.tsv");
        s.r#type = Some("api".to_string());
        assert!(declaration(&s).is_none());
        s.r#type = Some("dataset".to_string());
        s.columns = None;
        assert!(declaration(&s).is_none());
    }

    #[test]
    fn tsv_and_csv_read_the_same_rows() {
        let tsv = read("a\tb\n1\tx y\n2\t\n", Format::Tsv).unwrap();
        let csv = read("a,b\n1,x y\n2,\n", Format::Csv).unwrap();
        assert_eq!(tsv, csv);
        assert_eq!(tsv.rows[1].line, 3);
        let quoted = read("a,b\n1,\"x, y\"\n", Format::Csv).unwrap();
        assert_eq!(quoted.rows[0].cells, vec!["1", "x, y"]);
    }

    #[test]
    fn shape_reports_header_width_type_value_required_and_key() {
        let d = decl();
        let bad = read("filing_id\tkind\tline\treceived\n", Format::Tsv).unwrap();
        let got = shape(&d, &bad);
        assert_eq!(got.len(), 1);
        assert!(got[0].detail.contains("header"));

        let text = "filing_id\tline\tkind\treceived\n\
                    A\t24\tcash\t2025-03\n\
                    A\t24\tcash\t2025\n\
                    A\t3\tcash\n\
                    \t4\tcash\t\n\
                    B\tx\tgift\tMarch\n\
                    C\t3\tcash\t\n\
                    C\t3.0\tcash\t\n";
        let got: Vec<String> = shape(&d, &read(text, Format::Tsv).unwrap())
            .iter()
            .map(|x| x.to_string())
            .collect();
        assert_eq!(
            got,
            vec![
                "line 3: repeats the key `A,24` first held on line 2",
                "line 4: has 3 cells, and the table has 4 columns",
                "line 5, column `filing_id`: is empty, and the column is required",
                "line 6, column `line`: `x` is not a number",
                "line 6, column `kind`: `gift` is not one of the column's values — `cash`, `inkind`",
                "line 6, column `received`: `March` is not a date — write `YYYY-MM-DD`, or `YYYY-MM` or `YYYY`",
            ],
            "`24` passes as a number, and `3` and `3.0` are two keys"
        );
    }

    #[test]
    fn a_row_fragment_splits_before_it_decodes() {
        assert_eq!(
            row_fragment("row=OH%2C1,3").unwrap(),
            vec!["OH,1".to_string(), "3".to_string()]
        );
        assert!(row_fragment("L3").is_none());
        let d = decl();
        let rows = read(
            "filing_id\tline\tkind\treceived\nOH,1\t3\tcash\t\n",
            Format::Tsv,
        )
        .unwrap();
        assert!(has_row(&d, &rows, &row_fragment("row=OH%2C1,3").unwrap()));
        assert!(!has_row(
            &d,
            &rows,
            &row_fragment("row=OH%2C1,3.0").unwrap()
        ));
        assert!(!has_row(&d, &rows, &row_fragment("row=OH%2C1").unwrap()));
    }
}
