//! JSON span records, read through a declared mapping — RFC-0063 §3.
//!
//! A producer that computes over its corpus writes what it computed to JSON, and each value it
//! quotes carries a node and a span. `[[derive.records]]` says where those values are, by RFC
//! 6901 pointer, and each one is checked as a citation: the node resolves, the span is in its
//! file, and the governing quote sits in one paragraph that carries a standing.
//!
//! # The file text, not the prose
//!
//! An artifact's span is matched against a node's prose strings ([`super::prose_fields`]). A
//! record's is matched against the node's **file text**, and its paragraph is a blank-line block
//! of that text, because a record quotes typed properties and YAML's own line folding, which no
//! parsed string holds (RFC-0063 §3.2).
//!
//! # What a record does not get
//!
//! A record has no `claim` and no `reach`, so no reach, refusal or answer check applies, and
//! `derive-no-reach` is never reported for one.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::Value;

use super::{
    finding, CitedSpan, Derivation, CROSSES_PARAGRAPH, RECORD_UNMATCHED, SPAN_DRIFT, TAG_DRIFT,
    UNRESOLVED,
};
use crate::claims::{bare_standing, parse_tag, standing_rank};
use crate::cmd::lint::citations::truncate;
use crate::cmd::lint::local_citations::key_of;
use crate::cmd::lint::Severity;
use crate::config::DeriveRecords;

/// How many records the mappings matched, and their derivations: one per record, one per
/// pointer that matched nothing in a file, and one per mapping that could not be read at all.
///
/// A mapping is refused, as a `derive-record-unmatched` finding against `.yidam/config.toml`,
/// when it declares both `export` and `paths` or neither, no `records`, a pointer it cannot
/// read, globs that match no JSON file, or an export whose dependency is not installed at its
/// declared `rev`. Emitted rather than propagated, so a `--format json` consumer still reads a
/// report that names why.
pub(crate) fn check_records(root: &Path, mappings: &[DeriveRecords]) -> (usize, Vec<Derivation>) {
    let mut out = Vec::new();
    let mut matched = 0;
    for (i, mapping) in mappings.iter().enumerate() {
        let n = i + 1;
        if let Err(e) = check_mapping(root, mapping, &mut matched, &mut out) {
            out.push(unmatched(
                CONFIG,
                None,
                format!("[[derive.records]] #{n}: {e:#}"),
            ));
        }
    }
    (matched, out)
}

/// Where a mapping that cannot be read is reported: the file that declares it.
const CONFIG: &str = ".yidam/config.toml";

/// One mapping's records, pushed onto `out`. An `Err` is the mapping's own refusal; anything a
/// file or record does wrong is a finding in `out` instead.
fn check_mapping(
    root: &Path,
    mapping: &DeriveRecords,
    matched: &mut usize,
    out: &mut Vec<Derivation>,
) -> Result<()> {
    let parts = Parts::of(mapping)?;
    if mapping.records.is_empty() {
        bail!("declares no `records` pointers, so it reads nothing");
    }
    for pointer in &mapping.records {
        if !pointer.is_empty() && !pointer.starts_with('/') {
            bail!("`{pointer}` is not a JSON pointer: it does not open with `/`");
        }
    }
    let (files, corpus) = match (&mapping.export, &mapping.paths) {
        (Some(name), None) => {
            let (file, corpus) = installed_export(root, name)?;
            (vec![file], corpus)
        }
        (None, Some(globs)) => {
            let files = super::files_under(root, globs, &[".json"]);
            if files.is_empty() {
                bail!(
                    "`paths` {} match no .json file, so no record is read",
                    globs.join(", ")
                );
            }
            (files, root.join(".yidam/corpus"))
        }
        _ => bail!(
            "declares {} — give `export` for a dependency's export, or `paths` for this \
             repository's own JSON files",
            if mapping.export.is_some() {
                "both `export` and `paths`"
            } else {
                "neither `export` nor `paths`"
            }
        ),
    };
    for rel in files {
        let text = match std::fs::read_to_string(root.join(&rel)) {
            Ok(t) => t,
            Err(e) => {
                out.push(unmatched(
                    &rel,
                    None,
                    format!("cannot be read ({e}), so no record in it can be"),
                ));
                continue;
            }
        };
        let json: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                out.push(unmatched(
                    &rel,
                    None,
                    format!("is not JSON ({e}), so no record in it can be read"),
                ));
                continue;
            }
        };
        for pointer in &mapping.records {
            let matches = matching(&json, pointer)?;
            if matches.is_empty() {
                out.push(unmatched(
                    &rel,
                    Some(pointer),
                    format!(
                        "pointer {pointer} matches nothing in this file — a key the \
                         producer renamed or dropped; re-pin and update the pointer together"
                    ),
                ));
            }
            *matched += matches.len();
            for m in matches {
                out.push(check_record(&rel, &corpus, &m, &parts));
            }
        }
    }
    Ok(())
}

/// The export file `export = "<name>"` reads, relative to `root`, and the corpus it cites —
/// both only when the dependency is installed at its declared `rev`.
fn installed_export(root: &Path, name: &str) -> Result<(String, PathBuf)> {
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root))?;
    let Some(dep) = config.dependencies.get(name) else {
        bail!("no dependency named '{name}' in .yidam/tonpa.toml");
    };
    let (Some((_, rev)), Some(export)) = (dep.pinned_path(), dep.export.as_deref()) else {
        bail!(
            "'{name}' declares no export at a pin\n  `export = \"{name}\"` reads a pinned path \
             dependency's export; add one with `yidam tonpa add <path>@<commit> --export \
             <file>`"
        );
    };
    let lock = crate::deps::load_lock(&crate::paths::tonpa_lock_path(root))?;
    let locked = lock.packages.iter().find(|p| p.name == name);
    let tonpa = crate::paths::tonpa_dir(root);
    let installed = match locked {
        Some(p) if p.rev.as_deref() == Some(rev) && p.export.as_deref() == Some(export) => {
            p.export_sha256.is_some() && crate::deps::verify_installed(name, &tonpa, p)?
        }
        _ => false,
    };
    let file = crate::deps::export_file(name, &tonpa, export);
    let (true, Some(file)) = (installed, file) else {
        bail!(
            "'{name}' is not installed at its declared rev {rev} with its export {export}\n  \
             run `yidam tonpa install`"
        );
    };
    let rel = crate::paths::slash(
        &file
            .strip_prefix(root)
            .map_or_else(|_| file.clone(), Path::to_path_buf),
    );
    Ok((rel, tonpa.join(name).join("corpus")))
}

// ── pointers ──────────────────────────────────────────────────────────────────

/// A relative JSON pointer: up `levels` from a record, then down `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Relative {
    levels: usize,
    path: Vec<String>,
    written: String,
}

impl Relative {
    fn parse(written: &str) -> Result<Self> {
        let digits = written.bytes().take_while(u8::is_ascii_digit).count();
        let (n, rest) = written.split_at(digits);
        let Ok(levels) = n.parse::<usize>() else {
            bail!("`{written}` is not a relative JSON pointer: it opens with no level count");
        };
        if !(rest.is_empty() || rest.starts_with('/')) {
            bail!("`{written}` is not a relative JSON pointer: `{rest}` follows the level count");
        }
        Ok(Self {
            levels,
            path: segments(rest),
            written: written.to_string(),
        })
    }

    /// The value this pointer names from the record at the end of `chain`.
    fn resolve<'a>(&self, chain: &[&'a Value]) -> Option<&'a Value> {
        let at = chain.len().checked_sub(self.levels + 1)?;
        self.path.iter().try_fold(chain[at], |v, seg| child(v, seg))
    }
}

/// The four relative pointers a mapping declares, defaulted.
struct Parts {
    node: Relative,
    span: Relative,
    governing: Option<Relative>,
    tier: Option<Relative>,
}

impl Parts {
    fn of(m: &DeriveRecords) -> Result<Self> {
        let read = |p: Option<&str>| p.map(Relative::parse).transpose();
        Ok(Self {
            node: Relative::parse(m.node.as_deref().unwrap_or("0/node"))?,
            span: Relative::parse(m.span.as_deref().unwrap_or("0/span"))?,
            governing: read(m.governing.as_deref())?,
            tier: read(m.tier.as_deref())?,
        })
    }
}

/// RFC 6901 reference tokens, unescaped: `~1` is `/` and `~0` is `~`.
fn segments(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn escape(seg: &str) -> String {
    seg.replace('~', "~0").replace('/', "~1")
}

fn child<'a>(v: &'a Value, seg: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    }
}

/// One value a pointer matched: the pointer with its wildcards filled, and every value from
/// the document root down to it.
struct Match<'a> {
    pointer: String,
    chain: Vec<&'a Value>,
}

/// Every value `pointer` names in `doc`, a `*` segment standing for every member.
fn matching<'a>(doc: &'a Value, pointer: &str) -> Result<Vec<Match<'a>>> {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        bail!("`{pointer}` is not a JSON pointer: it does not open with `/`");
    }
    let mut found = vec![Match {
        pointer: String::new(),
        chain: vec![doc],
    }];
    for seg in segments(pointer) {
        let mut next = Vec::new();
        for m in found {
            let Some(v) = m.chain.last().copied() else {
                continue;
            };
            let members: Vec<(String, &Value)> = match (seg.as_str(), v) {
                ("*", Value::Object(o)) => o.iter().map(|(k, c)| (k.clone(), c)).collect(),
                ("*", Value::Array(a)) => a
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i.to_string(), c))
                    .collect(),
                _ => child(v, &seg)
                    .map(|c| (seg.clone(), c))
                    .into_iter()
                    .collect(),
            };
            for (key, c) in members {
                let mut chain = m.chain.clone();
                chain.push(c);
                next.push(Match {
                    pointer: format!("{}/{}", m.pointer, escape(&key)),
                    chain,
                });
            }
        }
        found = next;
    }
    Ok(found)
}

// ── one record ────────────────────────────────────────────────────────────────

fn unmatched(rel: &str, record: Option<&str>, message: String) -> Derivation {
    Derivation {
        path: rel.to_string(),
        record: record.map(str::to_string),
        claim: None,
        reach: None,
        tier: None,
        spans: Vec::new(),
        findings: vec![finding(RECORD_UNMATCHED, Severity::Error, None, message)],
    }
}

/// The byte ranges of `text`'s blank-line paragraphs: runs of lines with no blank line inside.
fn paragraphs(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let blank = line.trim().is_empty();
        match (blank, start) {
            (true, Some(s)) => {
                out.push((s, at));
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
        at += line.len();
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

/// The paragraph one occurrence of `quote` sits wholly inside, if any does.
fn paragraph_holding<'t>(text: &'t str, quote: &str) -> Option<&'t str> {
    let paras = paragraphs(text);
    text.match_indices(quote).find_map(|(i, _)| {
        paras
            .iter()
            .find(|(s, e)| *s <= i && i + quote.len() <= *e)
            .map(|(s, e)| &text[*s..*e])
    })
}

/// Every standing a paragraph carries, bare: each bracketed tag, and each `tag:` value.
fn standings_in(paragraph: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut rest = paragraph;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find([']', '[']) else {
            break;
        };
        if after.as_bytes()[close] == b']' {
            if let Some(tag) = parse_tag(&after[..close]) {
                out.extend(bare_standing(tag.standing));
            }
        }
        rest = &after[close..];
    }
    for line in paragraph.lines() {
        let line = line.trim().trim_start_matches("- ").trim_start();
        if let Some(value) = line.strip_prefix("tag:") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !value.starts_with('[') {
                if let Some(tag) = parse_tag(value) {
                    out.extend(bare_standing(tag.standing));
                }
            }
        }
    }
    out
}

/// A node key that stays inside the corpus directory.
fn node_file(corpus: &Path, key: &str) -> Option<PathBuf> {
    let rel = Path::new(key);
    let inside = !key.is_empty() && rel.components().all(|c| matches!(c, Component::Normal(_)));
    inside.then(|| corpus.join(format!("{key}.yml")))
}

fn check_record(rel: &str, corpus: &Path, m: &Match, parts: &Parts) -> Derivation {
    let at = &m.pointer;
    let string = |p: &Relative| p.resolve(&m.chain).and_then(Value::as_str);
    let (Some(node), Some(span)) = (string(&parts.node), string(&parts.span)) else {
        let missing = if string(&parts.node).is_none() {
            &parts.node.written
        } else {
            &parts.span.written
        };
        return unmatched(
            rel,
            Some(at),
            format!("names no string at {missing} — a record that names nothing cites nothing"),
        );
    };
    let governing = match &parts.governing {
        None => span,
        Some(p) => match string(p) {
            Some(g) => g,
            None => {
                return unmatched(
                    rel,
                    Some(at),
                    format!("names no governing quote at {}", p.written),
                );
            }
        },
    };
    // A declared tier that this record leaves out is not compared; one that is there must read.
    let declared = match parts.tier.as_ref().and_then(|p| p.resolve(&m.chain)) {
        None | Some(Value::Null) => None,
        Some(v) => match v
            .as_str()
            .and_then(parse_tag)
            .and_then(|t| bare_standing(t.standing))
        {
            Some(t) => Some(t),
            None => {
                let written = parts.tier.as_ref().map_or("", |p| p.written.as_str());
                return unmatched(
                    rel,
                    Some(at),
                    format!("holds {v} at {written}, which names no standing"),
                );
            }
        },
    };

    let key = key_of(node);
    let mut out = Derivation {
        path: rel.to_string(),
        record: Some(at.clone()),
        claim: None,
        reach: None,
        tier: None,
        spans: Vec::new(),
        findings: Vec::new(),
    };
    let mut entry = CitedSpan {
        node: Some(key.clone()),
        span: Some(span.to_string()),
        standing: None,
    };
    let Some(text) = node_file(corpus, &key).and_then(|f| std::fs::read_to_string(f).ok()) else {
        out.findings.push(finding(
            UNRESOLVED,
            Severity::Error,
            Some(&key),
            format!("names `{key}`, and the cited corpus holds no such node"),
        ));
        out.spans.push(entry);
        return out;
    };
    let drift = |what: &str, quote: &str| {
        finding(
            SPAN_DRIFT,
            Severity::Error,
            Some(&key),
            format!(
                "quotes {what} that is not in `{key}`'s file — the node was edited after the \
                 export was computed. Quote: {}",
                truncate(quote)
            ),
        )
    };
    if span.trim().is_empty() || !text.contains(span) {
        out.findings.push(drift("a span", span));
    }
    if governing.trim().is_empty() || !text.contains(governing) {
        if governing != span {
            out.findings.push(drift("a governing quote", governing));
        }
        out.spans.push(entry);
        return out;
    }
    let Some(paragraph) = paragraph_holding(&text, governing) else {
        out.findings.push(finding(
            CROSSES_PARAGRAPH,
            Severity::Error,
            Some(&key),
            format!(
                "quotes `{key}` across a blank line, so no one paragraph governs it. Quote: {}",
                truncate(governing)
            ),
        ));
        out.spans.push(entry);
        return out;
    };
    let carried = standings_in(paragraph);
    let standing = carried
        .iter()
        .copied()
        .min_by_key(|s| standing_rank(s))
        .unwrap_or("open");
    if carried.is_empty() {
        out.findings.push(finding(
            TAG_DRIFT,
            Severity::Error,
            Some(&key),
            format!(
                "rests on a paragraph of `{key}` that carries no standing tag, so nothing \
                 licenses more than [open]. Quote: {}",
                truncate(governing)
            ),
        ));
    }
    if let Some(declared) = declared {
        if !carried.contains(&declared) {
            out.findings.push(finding(
                TAG_DRIFT,
                Severity::Error,
                Some(&key),
                format!(
                    "declares [{declared}], and the paragraph of `{key}` it rests on carries {} \
                     — the node's standing changed after the export was computed. Quote: {}",
                    if carried.is_empty() {
                        "none".to_string()
                    } else {
                        carried
                            .iter()
                            .map(|s| format!("[{s}]"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                    truncate(governing)
                ),
            ));
        }
    }
    entry.standing = Some(standing);
    out.spans.push(entry);
    out.tier = Some(standing);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = "class: model\nlabel: Signal timing\n\
                        description: >\n  Cycle length is 90 s at\n  the arterial [verified].\n\n\
                        notes: The offset is a guess [inference].\n\
                        cycle_s: 90\n\nunit: seconds\n";

    fn corpus() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("model")).unwrap();
        std::fs::write(d.path().join("model/timing.yml"), NODE).unwrap();
        d
    }

    fn mapping(records: &[&str]) -> DeriveRecords {
        DeriveRecords {
            records: records.iter().map(|s| (*s).to_string()).collect(),
            ..DeriveRecords::default()
        }
    }

    fn run(doc: &Value, m: &DeriveRecords) -> Vec<Derivation> {
        let c = corpus();
        let parts = Parts::of(m).unwrap();
        let mut out = Vec::new();
        for p in &m.records {
            let found = matching(doc, p).unwrap();
            if found.is_empty() {
                out.push(unmatched("pack.json", Some(p), "none".into()));
            }
            for f in found {
                out.push(check_record("pack.json", c.path(), &f, &parts));
            }
        }
        out
    }

    fn ids(d: &Derivation) -> Vec<&str> {
        d.findings.iter().map(|f| f.check).collect()
    }

    #[test]
    fn a_wildcard_fills_the_pointer_and_escapes_its_keys() {
        let doc = serde_json::json!({"m": [{"a/b": {"x": 1}}, {"a/b": {"x": 2}}]});
        let found = matching(&doc, "/m/*/a~1b").unwrap();
        let pointers: Vec<_> = found.iter().map(|m| m.pointer.as_str()).collect();
        assert_eq!(pointers, ["/m/0/a~1b", "/m/1/a~1b"]);
        assert_eq!(found[1].chain.len(), 4);
    }

    #[test]
    fn a_relative_pointer_climbs_then_descends() {
        let doc = serde_json::json!({"p": {"tier": "verified", "src": {"node": "n"}}});
        let m = &matching(&doc, "/p/src").unwrap()[0];
        let up = Relative::parse("1/tier").unwrap();
        assert_eq!(up.resolve(&m.chain), Some(&Value::from("verified")));
        assert_eq!(Relative::parse("9/tier").unwrap().resolve(&m.chain), None);
        assert!(Relative::parse("node").is_err());
        assert!(Relative::parse("0node").is_err());
    }

    #[test]
    fn a_record_quoting_a_typed_property_holds_at_its_paragraph() {
        let doc = serde_json::json!({"r": [{"node": "model/timing", "span": "cycle_s: 90",
                                           "tier_span": "The offset is a guess"}]});
        let mut m = mapping(&["/r/*"]);
        m.governing = Some("0/tier_span".into());
        let d = &run(&doc, &m)[0];
        assert_eq!(ids(d), Vec::<&str>::new());
        assert_eq!(d.tier, Some("inference"));
        assert_eq!(d.record.as_deref(), Some("/r/0"));
    }

    #[test]
    fn a_folded_quote_matches_the_file_text_and_its_tier_is_compared() {
        let doc = serde_json::json!({"r": {"tier": "[inference]",
            "s": {"node": "model/timing.yml", "span": "Cycle length is 90 s at\n  the arterial"}}});
        let mut m = mapping(&["/r/s"]);
        m.tier = Some("1/tier".into());
        let d = &run(&doc, &m)[0];
        assert_eq!(ids(d), [TAG_DRIFT]);
        assert_eq!(d.tier, Some("verified"));
    }

    #[test]
    fn each_way_a_record_fails_has_its_id() {
        let doc = serde_json::json!({"r": [
            {"node": "model/gone", "span": "x"},
            {"node": "model/timing", "span": "not in the file"},
            {"node": "model/timing", "span": "the arterial [verified].\n\nnotes"},
            {"node": "model/timing", "span": "unit: seconds"},
            {"node": "../timing", "span": "x"},
            {"node": 3, "span": "x"},
        ]});
        let out = run(&doc, &mapping(&["/r/*", "/missing/*"]));
        let got: Vec<Vec<&str>> = out.iter().map(ids).collect();
        assert_eq!(
            got,
            [
                vec![UNRESOLVED],
                vec![SPAN_DRIFT],
                vec![CROSSES_PARAGRAPH],
                vec![TAG_DRIFT],
                vec![UNRESOLVED],
                vec![RECORD_UNMATCHED],
                vec![RECORD_UNMATCHED],
            ]
        );
        assert_eq!(out[3].tier, Some("open"));
        assert_eq!(out[0].tier, None);
    }

    #[test]
    fn a_tag_value_is_a_standing_and_a_citing_bracket_is_not() {
        assert_eq!(standings_in("- claim: x\n  tag: verified\n"), ["verified"]);
        assert_eq!(
            standings_in("see [Pearl 2009] and [open] [inference — as proposed]"),
            ["open", "inference"]
        );
        assert!(standings_in("[[verified").is_empty());
    }

    #[test]
    fn a_paragraph_is_a_blank_line_block_of_the_file() {
        let text = "a\nb\n\n  \nc\n";
        assert_eq!(paragraphs(text), [(0, 4), (8, 10)]);
        assert_eq!(paragraph_holding(text, "a\nb"), Some("a\nb\n"));
        assert_eq!(paragraph_holding(text, "b\n\n  \nc"), None);
    }
}
