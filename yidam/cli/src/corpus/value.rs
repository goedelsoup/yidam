//! What a value may hold, by the type its property declares.
//!
//! The one reading of `type:` that every reader shares: `lint`'s `property-type`, `migrate`'s
//! retype, `query`'s orderings, the warehouse's typed columns and the claim counter's `list`
//! reading. It lived in `cmd/lint/checks/properties.rs`, where it was first needed, and three
//! modules outside the command layer reached up into `lint` for it (#1635). Nothing here knows
//! a check exists; `lint::checks` re-exports it under the names it had.

/// The declared types this check knows how to test.
///
/// `pub(crate)` for `migrate`, which tests an instance's existing value against a *proposed*
/// type before performing a retype. That has to be this predicate rather than a second
/// reading of it: a migration disagreeing with the gate about what a valid value is would
/// be a migration into a failing build.
///
/// Anything else is left alone rather than reported: a corpus is free to coin a type, and a
/// check that failed on every type it had not heard of would make coining one impossible.
pub(crate) fn property_type_violation(declared: &str, value: &serde_yaml::Value) -> Option<String> {
    let scalar = |v: &serde_yaml::Value| match v {
        serde_yaml::Value::String(s) => Ok(s.clone()),
        serde_yaml::Value::Null => Err("is empty".to_string()),
        serde_yaml::Value::Sequence(_) => Err("is a list".to_string()),
        serde_yaml::Value::Mapping(_) => Err("is a mapping".to_string()),
        // The classic YAML surprise: `parameter: 00060` is the number 60, and the code the
        // corpus meant is gone by the time anything reads it.
        other => Err(format!(
            "is {}, not text — quote it",
            match other {
                serde_yaml::Value::Bool(_) => "a boolean",
                serde_yaml::Value::Number(_) => "a number",
                _ => "not text",
            }
        )),
    };
    let not_a_tag = |s: &str| {
        format!(
            "`{}` is not an evidence tag — write `verified`, `inference`, or `open`",
            s.trim()
        )
    };
    match declared {
        // A sequence is legal here and nowhere else: `count_structural` reads a list of
        // tags as one claim each, and a corpus writing `claim_tag: [open]` unquoted has
        // written a one-element list without meaning to. Rejecting it would put this check
        // in disagreement with the counter about the same bytes.
        crate::claims::CLAIM_PROPERTY_TYPE => match value {
            serde_yaml::Value::Sequence(items) => items.iter().find_map(|i| match i.as_str() {
                Some(s) if crate::claims::tag_of(s).is_some() => None,
                Some(s) => Some(not_a_tag(s)),
                None => Some("holds a value that is not text".to_string()),
            }),
            other => match scalar(other) {
                Err(why) => Some(why),
                Ok(s) if crate::claims::tag_of(&s).is_some() => None,
                Ok(s) => Some(not_a_tag(&s)),
            },
        },
        "date" => match scalar(value) {
            Err(why) => Some(why),
            Ok(s) if is_iso_date(s.trim()) => None,
            Ok(s) => Some(format!(
                "`{}` is not a date — write `YYYY-MM-DD`, or `YYYY-MM` or `YYYY` if that is \
                 the precision it is known to",
                s.trim()
            )),
        },
        "string" | "text" | "ref" => match scalar(value) {
            Err(why) => Some(why),
            Ok(s) if s.trim().is_empty() => Some("is empty".to_string()),
            Ok(_) => None,
        },
        // **A number is a YAML number, unquoted** (RFC-0040). The mirror of the `string`
        // arm's "quote it": a corpus that writes `length_km: "24"` has stored text that
        // happens to spell a number, and the compiled schema — `{"type": "number"}` — would
        // refuse the file while this gate waved it through. The one reading a number gets is
        // [`numeric_value`], which `query`'s ordering shares for the reason `iso_date_parts`
        // gives: a second parser is a corpus where `lint` accepts what `query` will not order.
        "number" => match value {
            serde_yaml::Value::Number(n) => match numeric_value(&n.to_string()) {
                Some(_) => None,
                None => Some(format!("`{n}` is not a finite number")),
            },
            serde_yaml::Value::String(s) => match numeric_value(s) {
                Some(_) => Some(format!("`{}` is text, not a number — unquote it", s.trim())),
                None => Some(format!("`{}` is not a number", s.trim())),
            },
            serde_yaml::Value::Bool(_) => Some("is a boolean, not a number".to_string()),
            other => scalar(other).err(),
        },
        // **A value that names its document** (RFC-0046). The shape is all this can test —
        // whether the words are in the bytes is `quotation-span-drift`'s question, and needs
        // the catalog and the vault cache — and it is read by the one parser those checks
        // read with, so a value this admits is one they can resolve.
        super::quotation::QUOTATION_PROPERTY_TYPE => super::quotation::read(value).err(),
        // **A list, as a bare `list` declares it** (RFC-0050): a non-empty sequence of
        // anything. What `items:` or `fields:` add is [`declared_violation`]'s, because they
        // live on the declaration and this predicate is handed only its type.
        LIST_PROPERTY_TYPE => match value {
            serde_yaml::Value::Sequence(items) if items.is_empty() => {
                Some("is an empty list — leave the property out instead".to_string())
            }
            serde_yaml::Value::Sequence(_) => None,
            serde_yaml::Value::Null => Some("is empty".to_string()),
            serde_yaml::Value::Mapping(_) => {
                Some("is a mapping, not a list — write it as a one-item list".to_string())
            }
            _ => Some("is not a list — write it as a one-item list".to_string()),
        },
        _ => None,
    }
}

/// The declared type a value may hold several of (RFC-0050).
pub(crate) const LIST_PROPERTY_TYPE: &str = "list";

/// Every `type:` a reader grades: the scalar item types, and the two that are not one.
/// [`crate::cmd::lint::checks::property_type_unknown`] reports any other; the test
/// `known_types_are_exactly_the_graded_ones` holds this list to [`property_type_violation`].
pub(crate) const KNOWN_PROPERTY_TYPES: [&str; 8] = [
    "string",
    "text",
    "ref",
    "date",
    "number",
    crate::claims::CLAIM_PROPERTY_TYPE,
    super::quotation::QUOTATION_PROPERTY_TYPE,
    LIST_PROPERTY_TYPE,
];

/// Why a `list` declaration constrains nothing, or `None` when it is well formed.
///
/// `items:` and `fields:` together say two things about one item; an `items:` outside
/// [`yidam_core::ontology::LIST_ITEM_TYPES`] names a list, a quotation or a coined type; a
/// field typed `list` or `quotation` is one the readers that walk records do not reach. Each
/// is reported once, on the class, by [`crate::cmd::lint::checks::list_declaration_invalid`]
/// — and an instance under it is held only to the bare `list` shape, because a declaration
/// that cannot be read cannot be held to either half of what it says. The compiled schema
/// says the same: `true`.
pub(crate) fn list_declaration_problem(p: &crate::corpus::ClassProperty) -> Option<String> {
    if p.r#type != LIST_PROPERTY_TYPE {
        return None;
    }
    let item_type = |t: &str| yidam_core::ontology::LIST_ITEM_TYPES.contains(&t);
    let kinds = yidam_core::ontology::LIST_ITEM_TYPES.join("`, `");
    if !p.items.is_empty() && !p.fields.is_empty() {
        return Some(
            "declares both `items:` and `fields:` — an item is a scalar or a record, not both"
                .into(),
        );
    }
    if !p.items.is_empty() && !item_type(&p.items) {
        return Some(format!(
            "declares `items: {}` — an item is one of `{kinds}`, or a record declared with `fields:`",
            p.items
        ));
    }
    p.fields
        .iter()
        .find(|f| matches!(f.r#type.as_str(), LIST_PROPERTY_TYPE | super::quotation::QUOTATION_PROPERTY_TYPE))
        .map(|f| {
            format!(
                "declares the field `{}` as `{}` — a record field is one of `{kinds}`, or a type the corpus coined",
                f.name, f.r#type
            )
        })
}

/// What an instance value violates in its full declaration, or `None`.
///
/// [`property_type_violation`] reads a type; this reads the declaration, which is what a
/// `string`'s `values:` and a `list`'s `items:` and `fields:` need. The shape first, then the
/// set: a value that is not text is reported for what it is, not also for being outside a
/// list of text.
pub(crate) fn declared_violation(
    p: &crate::corpus::ClassProperty,
    value: &serde_yaml::Value,
) -> Option<String> {
    let why = property_type_violation(&p.r#type, value);
    if why.is_some() {
        return why;
    }
    match p.r#type.as_str() {
        "string" => declared_value_violation(&p.values, value),
        LIST_PROPERTY_TYPE if list_declaration_problem(p).is_none() => {
            let serde_yaml::Value::Sequence(entries) = value else {
                return None;
            };
            entries.iter().enumerate().find_map(|(i, entry)| {
                let at = i + 1;
                if !p.items.is_empty() {
                    item_violation(p, entry).map(|why| format!("item {at} {why}"))
                } else if !p.fields.is_empty() {
                    record_violation(&p.fields, entry).map(|why| format!("item {at} {why}"))
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

/// One item of an `items:` list, held to the scalar arm its type names and to the list's set.
fn item_violation(p: &crate::corpus::ClassProperty, item: &serde_yaml::Value) -> Option<String> {
    // The one-or-a-list tolerance is the property's: an item is one tag, and a list inside
    // a list is the shape being wrong, whatever the claim arm would accept on its own.
    if let serde_yaml::Value::Sequence(_) = item {
        return Some("is a list — an item is one value".into());
    }
    property_type_violation(&p.items, item).or_else(|| {
        (p.items == "string")
            .then(|| declared_value_violation(&p.values, item))
            .flatten()
    })
}

/// One item of a `fields:` list: a mapping, closed against keys no field names, carrying
/// every field declared `required: true`, and each field held to its own declaration.
fn record_violation(
    fields: &[crate::corpus::ClassProperty],
    entry: &serde_yaml::Value,
) -> Option<String> {
    let serde_yaml::Value::Mapping(map) = entry else {
        return Some("is not a record — each item is a mapping of the declared fields".into());
    };
    for key in map.keys() {
        let name = key.as_str().unwrap_or_default();
        if !fields.iter().any(|f| f.name == name) {
            let named = fields
                .iter()
                .map(|f| format!("`{}`", f.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "has `{}`, which no field declares — the fields are {named}",
                serde_yaml::to_string(key).unwrap_or_default().trim()
            ));
        }
    }
    fields.iter().find_map(|f| match map.get(f.name.as_str()) {
        None if f.required() => Some(format!("has no `{}`, which every item carries", f.name)),
        None => None,
        Some(v) => declared_violation(f, v)
            .map(|why| format!("has `{}` declared `{}`, which {why}", f.name, f.r#type)),
    })
}

/// A `string` outside the closed set its class declares (RFC-0044), or `None`.
///
/// `pub(crate)` for `migrate`, for the reason [`property_type_violation`] is: a retype into
/// `string` on a property that declares a set is a retype into this predicate, and a
/// migration that did not consult it would be a migration into a failing build.
///
/// Only text is tested. A list, a mapping or a bare number is the *shape* being wrong, and
/// [`property_type_violation`] has already said so — reporting it a second time for being
/// outside a list of text would be two findings for one mistake. An empty set is no set at
/// all: a closed set of nothing would refuse every value, which no class means by
/// `values: []`. The match is exact, as JSON Schema's `enum` is: the compiled schema refuses
/// `In operation` against `in operation`, and a gate that admitted it would be looser than
/// the schema it exists to be no stricter than.
pub(crate) fn declared_value_violation(
    values: &[String],
    value: &serde_yaml::Value,
) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let serde_yaml::Value::String(s) = value else {
        return None;
    };
    if values.iter().any(|v| v == s) {
        return None;
    }
    let set = values
        .iter()
        .map(|v| format!("`{v}`"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "`{s}` is not one of the values the class declares — {set}"
    ))
}

/// The value of a `number`, or `None` when the text is not one.
///
/// The whole of the `number` arm's rule, returning what it read, so that an ordering over
/// numbers and the check that admits them cannot disagree about which values are numbers —
/// [`iso_date_parts`]'s reason, one type over. Finite only: YAML spells `.inf` and `.nan`,
/// and neither is a quantity a corpus measured. There is no precision rule here and none is
/// wanted — `1893` denotes an interval, which is why dates have one; `7` and `7.0` denote
/// the same point, which is why `=` on a number compares numerically (RFC-0040).
pub fn numeric_value(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

/// An ISO-8601 calendar date, structurally, **at whatever precision the corpus knows**:
/// `YYYY`, `YYYY-MM`, or `YYYY-MM-DD`.
///
/// Not a calendar — `2024-02-31` passes and is somebody else's finding. What this catches is
/// a date field carrying prose, and reduced precision is not prose. Requiring a full day
/// rejected `formed: "1985"` 71 times in one derived corpus and `stated: "1991-06"` in
/// another: a band formed in 1985, and the year is the precision the fact is actually known
/// to. Demanding a month and a day there does not make the record more accurate, it makes it
/// invented — and the check's own rationale had already said prose was the target.
///
/// The 45 findings that remain in that corpus are the ones worth having: `[open] No date.`,
/// `1993. [inference], via [musicbrainz](../../catalog/musicbrainz.md) life-span.`
fn is_iso_date(s: &str) -> bool {
    iso_date_parts(s).is_some()
}

/// The components of an ISO date, in order, or `None` when it is not one.
///
/// The whole of [`is_iso_date`]'s rule, returning what it read instead of discarding it, so
/// that a comparison over dates and the check that admits them cannot come to disagree about
/// which strings are dates. `query`'s ordering operators are the second reader (#725), and a
/// second parser there would be a corpus where `lint` accepts a value that `query` will not
/// order — the quietest possible split.
///
/// Every component is fixed-width and zero-padded, so a lexical comparison of the parts is a
/// numeric one, and the caller needs no integers.
pub fn iso_date_parts(s: &str) -> Option<Vec<&str>> {
    let mut parts = s.split('-');
    // Year, then optionally month, then optionally day. A trailing separator leaves an
    // empty part, which fails the width test rather than passing as absent — `1985-` is
    // not a year.
    let widths = [4, 2, 2];
    let mut seen = Vec::with_capacity(3);
    for w in widths {
        match parts.next() {
            None => break,
            Some(p) if p.len() == w && p.chars().all(|c| c.is_ascii_digit()) => seen.push(p),
            Some(_) => return None,
        }
    }
    match !seen.is_empty() && parts.next().is_none() {
        true => Some(seen),
        false => None,
    }
}

/// Order two ISO dates **at the precision they share**, or `None` if either is not a date.
///
/// The corpus writes a date to whatever precision it knows it — `is_iso_date`'s own rationale
/// records `formed: "1985"` occurring 71 times in one derived corpus — so a comparison has to
/// say what it does when the two sides disagree about precision. This one drops both to the
/// coarser: `1893-04-01 < 1900` is decided on the years alone.
///
/// **This is deliberately not `=`'s rule, and the divergence is the point.** `=` compares at
/// the precision the *query* wrote, so `born=1893-04-01` does not match `born: 1893` — a
/// day-precision question the corpus cannot answer. Ordering at the query's precision would
/// make `born<=1900-01-01` skip every person whose birth year alone is known, silently, in
/// the direction of a smaller answer. An equality asked more precisely than the corpus knows
/// is genuinely unanswerable; an ordering usually is not — `1893` is before `1900-01-01`
/// whatever day it fell on — and answering it is the reason the operator exists.
///
/// The consequence to know: where the corpus is coarser than the query, `<=` and `>=` can
/// both hold for a value `=` rejects. When the two sides carry the same precision the three
/// are trichotomous, which is the case a caller reasons about.
///
/// **Here rather than in `query`, because it has a second reader.**
/// [`crate::cmd::lint::checks::interval_overlap`] orders a class's declared start and end by
/// it (#1201), and a second rule there would be a corpus where `query` says a holder was in
/// office on a day and `lint` says nobody was.
/// Equality at the shared precision is what an overlap check must not guess past: `1893`
/// and `1893-06-01` compare equal, and so a term ending `1893` and one beginning
/// `1893-06-01` are adjacent rather than overlapping.
pub fn compare_dates(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let (left, right) = (iso_date_parts(left.trim())?, iso_date_parts(right.trim())?);
    let shared = left.len().min(right.len());
    Some(left[..shared].cmp(&right[..shared]))
}
