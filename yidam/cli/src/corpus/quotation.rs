//! The shape of a `type: quotation` value — RFC-0046.
//!
//! The reading of the value alone: what a quotation is written as, and why a value is not one.
//! Holding a quotation to the bytes it names is `lint`'s, in `cmd/lint/quotations.rs`, which
//! re-exports this under the names it had. It moved beside [`super::value`] because that
//! module's `property_type_violation` dispatches on [`QUOTATION_PROPERTY_TYPE`] and reads the
//! shape with [`read`] (#1635).

use crate::vault::ContentHash;

/// The declared type, named once. `property_type_violation` and the schema compilers
/// dispatch on this string, and a second spelling would be a type one of them had not heard of.
pub const QUOTATION_PROPERTY_TYPE: &str = "quotation";

/// One quotation, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quotation {
    /// The catalog entry, by stem or by path from the node's directory — the two spellings
    /// an edge's `source:` admits, read by the same function.
    pub of: String,
    pub span: String,
    /// The artifact the span was read from.
    pub sha256: Option<String>,
}

/// Every quotation a value holds, or why it holds none.
///
/// **The one reading of the shape.** `property-type` reports the `Err` and `lint`'s quotation
/// checks read the `Ok`, so the two cannot disagree about what a quotation is — `iso_date_parts`'s
/// reason, one type over. A value is one mapping or a non-empty list of them, and a mapping
/// carries `of:` and `span:`, optionally `sha256:`, and nothing else: a fourth key is a typo
/// of one of those three more often than a field, and the compiled schema refuses it too.
pub(crate) fn read(value: &serde_yaml::Value) -> Result<Vec<Quotation>, String> {
    match value {
        serde_yaml::Value::Mapping(m) => one(m).map(|q| vec![q]),
        serde_yaml::Value::Sequence(items) if items.is_empty() => {
            Err("is an empty list — a quotation names what it quotes".to_string())
        }
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .enumerate()
            .map(|(i, item)| match item {
                serde_yaml::Value::Mapping(m) => {
                    one(m).map_err(|why| format!("entry {} {why}", i + 1))
                }
                _ => Err(format!(
                    "entry {} is not a quotation — write `of:` and `span:` beneath it",
                    i + 1
                )),
            })
            .collect(),
        serde_yaml::Value::Null => Err("is empty".to_string()),
        _ => Err("is not a quotation — write `of:` and `span:` beneath it".to_string()),
    }
}

fn one(m: &serde_yaml::Mapping) -> Result<Quotation, String> {
    let mut of = None;
    let mut span = None;
    let mut sha256 = None;
    for (k, v) in m {
        let key = k.as_str().unwrap_or_default();
        let slot = match key {
            "of" => &mut of,
            "span" => &mut span,
            "sha256" => &mut sha256,
            _ => {
                return Err(format!(
                    "has `{key}:`, which a quotation does not — write `of:`, `span:`, and \
                     optionally `sha256:`"
                ))
            }
        };
        match v {
            serde_yaml::Value::String(s) if !s.trim().is_empty() => *slot = Some(s.clone()),
            serde_yaml::Value::String(_) | serde_yaml::Value::Null => {}
            _ => return Err(format!("has `{key}:` holding something that is not text")),
        }
    }
    let of = of.ok_or("has no `of:` — name the catalog entry the span is quoted from")?;
    let span = span.ok_or("has no `span:` — the words, as the document has them")?;
    if let Some(pin) = &sha256 {
        ContentHash::parse(pin).map_err(|e| format!("has a `sha256:` that is {e}"))?;
    }
    Ok(Quotation {
        of: of.trim().to_string(),
        span,
        sha256: sha256.map(|s| s.trim().to_string()),
    })
}
