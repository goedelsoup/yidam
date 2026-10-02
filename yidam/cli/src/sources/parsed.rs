//! A fetched response, parsed by the host into the value a transform is handed (#1318).
//!
//! RFC-0048 §6: *the host parses and marshals; the script never sees bytes*. A transform that
//! parsed for itself would need a parser in the closed prelude, and every parser it could be
//! handed is one more way for two runs of the same pack over the same bytes to disagree. So
//! the parse happens here, once per format, and [`Parsed`] is its whole answer — the flat,
//! document-ordered shape [`crate::gluon_arm::transform`] describes.
//!
//! Three formats, chosen by the media type the scheme's `resolve` declares: JSON (`serde_json`,
//! already in the closure), XML (`quick-xml`) and CSV (`csv`). A media type none of them reads
//! is refused by name, not guessed at from the bytes, for the reason `catalog-extract` will not
//! guess that bytes are a PDF: which format a response is in is the pack's to say.
//!
//! JSON object keys arrive in the order `serde_json` keeps them, which in this build is sorted
//! by key. A transform that wants a field selects it by path, so no transform can depend on it.

use crate::gluon_arm::marshal::Value;
use crate::gluon_arm::transform::{Field, Parsed};

/// Which of the three formats a media type names, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Xml,
    Csv,
}

impl Format {
    /// `application/json`, `application/vnd.citationstyles.csl+json; charset=utf-8` → JSON.
    pub fn of(media_type: &str) -> Option<Self> {
        let essence = media_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match essence.as_str() {
            "application/json" | "text/json" => Some(Self::Json),
            "application/xml" | "text/xml" => Some(Self::Xml),
            "text/csv" => Some(Self::Csv),
            e if e.ends_with("+json") => Some(Self::Json),
            e if e.ends_with("+xml") => Some(Self::Xml),
            _ => None,
        }
    }
}

/// Parse `bytes` as the format `media_type` names, or say why not.
pub fn parse(media_type: &str, bytes: &[u8]) -> Result<Parsed, String> {
    let format = Format::of(media_type).ok_or_else(|| {
        format!("`{media_type}` is not a media type a transform can be handed: JSON, XML or CSV")
    })?;
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("is not UTF-8 ({e}), which every format here is read as"))?;
    let mut out = Tree::default();
    match format {
        Format::Json => {
            let v: serde_json::Value =
                serde_json::from_str(text).map_err(|e| format!("is not JSON: {e}"))?;
            out.json(-1, "", &v);
        }
        Format::Xml => out.xml(text).map_err(|e| format!("is not XML: {e}"))?,
        Format::Csv => out.csv(text).map_err(|e| format!("is not CSV: {e}"))?,
    }
    Ok(Parsed {
        media_type: media_type.to_string(),
        fields: out.fields,
    })
}

#[derive(Default)]
struct Tree {
    fields: Vec<Field>,
    /// Whitespace read where no text has started yet: kept until the next text under the same
    /// element, which it then begins, and dropped at the next tag. So the space in
    /// `<i>a</i> b` is kept and the indentation between two elements is not.
    blank: Option<(i64, String)>,
}

impl Tree {
    /// Append a field under `parent`, and return its index.
    fn push(&mut self, parent: i64, name: &str, value: Value) -> i64 {
        let path = match usize::try_from(parent) {
            Ok(p) => format!("{}/{name}", self.fields[p].path),
            Err(_) => name.to_string(),
        };
        self.fields.push(Field {
            parent,
            name: name.to_string(),
            path,
            value,
        });
        (self.fields.len() - 1) as i64
    }

    /// A JSON value under `parent`. The document itself is not a field: its members are the
    /// roots, so `message/title` is a path and not `/message/title`.
    fn json(&mut self, parent: i64, name: &str, v: &serde_json::Value) {
        use serde_json::Value as J;
        let at = |t: &mut Self, value| {
            if parent < 0 && name.is_empty() {
                -1
            } else {
                t.push(parent, name, value)
            }
        };
        match v {
            J::Null => {
                at(self, Value::Empty);
            }
            J::Bool(b) => {
                at(self, Value::Flag(*b));
            }
            J::Number(n) => {
                let value = match n.as_i64() {
                    Some(i) => Value::Int(i),
                    // A u64 past i64 and every float: a number, not a narrowed integer.
                    None => n.as_f64().map_or(Value::Unrepresentable, Value::Number),
                };
                at(self, value);
            }
            J::String(s) => {
                at(self, Value::Text(s.clone()));
            }
            J::Array(items) => {
                let me = at(self, Value::Empty);
                for (i, item) in items.iter().enumerate() {
                    self.json(me, &i.to_string(), item);
                }
            }
            J::Object(map) => {
                let me = at(self, Value::Empty);
                for (k, item) in map {
                    self.json(me, k, item);
                }
            }
        }
    }

    /// An XML document: elements as fields, attributes as `@name` children, text as `#text`.
    fn xml(&mut self, text: &str) -> Result<(), String> {
        use quick_xml::events::Event;
        use quick_xml::XmlVersion;

        let mut reader = quick_xml::Reader::from_str(text);
        let mut open: Vec<i64> = Vec::new();
        loop {
            let event = reader.read_event().map_err(|e| e.to_string())?;
            let parent = open.last().copied().unwrap_or(-1);
            if matches!(event, Event::Start(_) | Event::Empty(_) | Event::End(_)) {
                self.blank = None;
            }
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let empty = matches!(event, Event::Empty(_));
                    let me = self.push(parent, e.name().as_ref(), Value::Empty);
                    for a in e.attributes() {
                        let a = a.map_err(|e| e.to_string())?;
                        let v = a
                            .normalized_value(XmlVersion::default())
                            .map_err(|e| e.to_string())?;
                        self.push(
                            me,
                            &format!("@{}", a.key.as_ref()),
                            Value::Text(v.into_owned()),
                        );
                    }
                    if !empty {
                        open.push(me);
                    }
                }
                Event::End(_) => {
                    open.pop();
                }
                Event::Text(t) => self.text(parent, &t.xml10_content()),
                Event::CData(t) => self.text(parent, &t.xml10_content()),
                Event::GeneralRef(r) => {
                    let resolved = match r.resolve_char_ref().map_err(|e| e.to_string())? {
                        Some(c) => c.to_string(),
                        None => quick_xml::escape::resolve_predefined_entity(&r)
                            .ok_or_else(|| format!("`&{};` is not an entity XML defines", &*r))?
                            .to_string(),
                    };
                    self.text(parent, &resolved);
                }
                Event::Eof => break,
                Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
            }
        }
        if !open.is_empty() {
            return Err("the document ends inside an element".into());
        }
        Ok(())
    }

    /// Text under `parent`, joined onto the `#text` field just before it if there is one, so
    /// `a &amp; b` is one field and not three. Whitespace between elements is not text.
    fn text(&mut self, parent: i64, s: &str) {
        if parent < 0 {
            return;
        }
        if let Some(last) = self.fields.last_mut() {
            if last.parent == parent && last.name == "#text" {
                if let Value::Text(t) = &mut last.value {
                    t.push_str(s);
                    return;
                }
            }
        }
        let mut joined = match self.blank.take() {
            Some((p, ws)) if p == parent => ws,
            _ => String::new(),
        };
        joined.push_str(s);
        if joined.trim().is_empty() {
            self.blank = Some((parent, joined));
            return;
        }
        self.push(parent, "#text", Value::Text(joined));
    }

    /// A CSV table with a header row: each row a field named by its index, each cell a child
    /// named by its column. An empty cell is [`Value::Empty`], as an empty JSON value is.
    fn csv(&mut self, text: &str) -> Result<(), String> {
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        let headers = reader.headers().map_err(|e| e.to_string())?.clone();
        for (i, record) in reader.records().enumerate() {
            let record = record.map_err(|e| e.to_string())?;
            let row = self.push(-1, &i.to_string(), Value::Empty);
            for (h, cell) in headers.iter().zip(record.iter()) {
                let value = if cell.is_empty() {
                    Value::Empty
                } else {
                    Value::Text(cell.to_string())
                };
                self.push(row, h, value);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(p: &Parsed) -> Vec<(String, Value)> {
        p.fields
            .iter()
            .map(|f| (f.path.clone(), f.value.clone()))
            .collect()
    }

    fn text(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn a_media_type_names_its_format_or_is_refused() {
        assert_eq!(Format::of("application/json"), Some(Format::Json));
        assert_eq!(
            Format::of("application/vnd.citationstyles.csl+json; charset=utf-8"),
            Some(Format::Json)
        );
        assert_eq!(Format::of("Application/XML"), Some(Format::Xml));
        assert_eq!(Format::of("application/jats+xml"), Some(Format::Xml));
        assert_eq!(Format::of("text/csv"), Some(Format::Csv));
        assert_eq!(Format::of("application/pdf"), None);
        let e = parse("application/pdf", b"%PDF").unwrap_err();
        assert!(e.contains("JSON, XML or CSV"), "{e}");
    }

    #[test]
    fn json_is_flattened_with_paths_and_typed_scalars() {
        let p = parse(
            "application/json",
            br#"{"message": {"title": ["On Bridges"], "year": 1931, "score": 0.5, "oa": true, "pmcid": null}}"#,
        )
        .unwrap();
        let got = paths(&p);
        for want in [
            ("message".to_string(), Value::Empty),
            ("message/title".into(), Value::Empty),
            ("message/title/0".into(), text("On Bridges")),
            ("message/year".into(), Value::Int(1931)),
            ("message/score".into(), Value::Number(0.5)),
            ("message/oa".into(), Value::Flag(true)),
            ("message/pmcid".into(), Value::Empty),
        ] {
            assert!(got.contains(&want), "{want:?} not in {got:?}");
        }
        let title = p
            .fields
            .iter()
            .position(|f| f.path == "message/title")
            .unwrap();
        let first = p
            .fields
            .iter()
            .find(|f| f.path == "message/title/0")
            .unwrap();
        assert_eq!(first.parent, title as i64);
        assert_eq!(p.fields[0].parent, -1);
    }

    #[test]
    fn xml_keeps_attributes_and_mixed_text_in_order() {
        let p = parse(
            "application/xml",
            br#"<?xml version="1.0"?>
<article id="PMC1">
  <title>Bridges &amp; <i>rivers</i> &#x41;fter</title>
  <empty kind="x"/>
  <note><![CDATA[a < b]]></note>
</article>"#,
        )
        .unwrap();
        assert_eq!(
            paths(&p),
            vec![
                ("article".to_string(), Value::Empty),
                ("article/@id".into(), text("PMC1")),
                ("article/title".into(), Value::Empty),
                ("article/title/#text".into(), text("Bridges & ")),
                ("article/title/i".into(), Value::Empty),
                ("article/title/i/#text".into(), text("rivers")),
                ("article/title/#text".into(), text(" After")),
                ("article/empty".into(), Value::Empty),
                ("article/empty/@kind".into(), text("x")),
                ("article/note".into(), Value::Empty),
                ("article/note/#text".into(), text("a < b")),
            ]
        );
    }

    #[test]
    fn xml_that_is_not_well_formed_is_refused() {
        let e = parse("text/xml", b"<a><b></a>").unwrap_err();
        assert!(e.starts_with("is not XML"), "{e}");
        let e = parse("text/xml", b"<a>&nbsp;</a>").unwrap_err();
        assert!(e.contains("nbsp"), "{e}");
    }

    #[test]
    fn csv_rows_are_fields_and_cells_are_named_by_header() {
        let p = parse(
            "text/csv",
            b"title,year\n\"On Bridges, Again\",1931\nUntitled,\n",
        )
        .unwrap();
        assert_eq!(
            paths(&p),
            vec![
                ("0".to_string(), Value::Empty),
                ("0/title".into(), text("On Bridges, Again")),
                ("0/year".into(), text("1931")),
                ("1".into(), Value::Empty),
                ("1/title".into(), text("Untitled")),
                ("1/year".into(), Value::Empty),
            ]
        );
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused_rather_than_transcoded() {
        let e = parse("application/json", &[0xff, 0xfe]).unwrap_err();
        assert!(e.contains("UTF-8"), "{e}");
    }
}
