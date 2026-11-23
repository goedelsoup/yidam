//! `yidam signal` — a computed value quoted in prose, refreshed rather than copied (RFC-0058).
//!
//! A calculator writes its answer to `.yidam/computed/`, and a person copies the number into a
//! node's prose. Nothing ties the copy to the file. This is `count`'s shape with a different
//! source: a block whose command names a signal, and a generator that writes the signal's
//! value into it.
//!
//! Two forms, and the bare one is the generator:
//!
//! - `yidam signal <node-ref> <name> [modifiers]` prints the value. Reads.
//! - `yidam signal` refreshes every `<!-- REGEN: yidam signal … -->` block in the tracked
//!   markdown and node files. Writes, and is what `yidam regen` runs.
//!
//! No `--format`. RFC-0058 changes no report schema, and the value on stdout is the whole
//! answer.

use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
use std::path::Path;

use crate::computed::Signals;

/// The literal every `signal` block's command begins with, argument excluded.
///
/// The trailing space is load-bearing, for the reason [`super::count::COMMAND_PREFIX`] gives:
/// without it `yidam signals` would read as a `signal` block whose argument is `s`.
pub const COMMAND_PREFIX: &str = "yidam signal ";

/// The generator's name, as [`crate::cmd::regen::GENERATORS`] and `--help` spell it.
pub const NAME: &str = "signal";

/// The three modifiers of RFC-0058 §2.2.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Modifiers {
    percent: bool,
    places: Option<usize>,
    grouped: bool,
}

impl Modifiers {
    fn any(&self) -> bool {
        self.percent || self.places.is_some() || self.grouped
    }
}

/// What a block's argument asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Citation {
    /// The node reference as written, or `None` for the own-row form.
    node: Option<String>,
    name: String,
    modifiers: Modifiers,
}

/// Read `[<node-ref>] <name> [modifiers]`.
///
/// Positional words come first and the modifiers follow them, so a word after a modifier is
/// refused rather than guessed at. One positional word is the own-row form; two are a node
/// reference and a name.
fn parse(argument: &str) -> std::result::Result<Citation, String> {
    let mut positional: Vec<&str> = Vec::new();
    let mut modifiers = Modifiers::default();
    let mut seen_modifier = false;
    for word in argument.split_whitespace() {
        let repeated = |what: &str| format!("`{what}` is given twice");
        if word == "percent" {
            if modifiers.percent {
                return Err(repeated("percent"));
            }
            modifiers.percent = true;
        } else if word == "grouped" {
            if modifiers.grouped {
                return Err(repeated("grouped"));
            }
            modifiers.grouped = true;
        } else if let Some(n) = word.strip_prefix("places=") {
            if modifiers.places.is_some() {
                return Err(repeated("places"));
            }
            let n: usize = n
                .parse()
                .map_err(|_| format!("`{word}` needs a whole number of places"))?;
            modifiers.places = Some(n);
        } else {
            if seen_modifier {
                return Err(format!(
                    "`{word}` follows a modifier; the node and the name come first"
                ));
            }
            positional.push(word);
            continue;
        }
        seen_modifier = true;
    }
    let (node, name) = match positional.as_slice() {
        [name] => (None, *name),
        [node, name] => (Some(node.to_string()), *name),
        [] => return Err("names no signal".to_string()),
        _ => {
            return Err(format!(
                "`{}` is more than a node and a name",
                positional.join(" ")
            ))
        }
    };
    Ok(Citation {
        node,
        name: name.to_string(),
        modifiers,
    })
}

/// The body a value is written as (RFC-0058 §2.2).
///
/// With no modifier, an integer in decimal, a float the way `serde_json` writes it, a string as
/// it is and a boolean as `true` or `false`. A null, list or map is not a figure. The modifiers
/// apply to numbers only, in one fixed order: `percent` multiplies by 100, `places` rounds,
/// `grouped` separates thousands, and `percent` appends `%`.
fn render(value: &serde_json::Value, m: &Modifiers) -> std::result::Result<String, String> {
    use serde_json::Value;
    let number = match value {
        Value::Number(n) => n,
        Value::String(s) if !m.any() => return Ok(s.clone()),
        Value::Bool(b) if !m.any() => return Ok(b.to_string()),
        Value::String(_) | Value::Bool(_) => {
            return Err(format!(
                "a modifier applies to a number, and this value is `{value}`"
            ))
        }
        Value::Null | Value::Array(_) | Value::Object(_) => {
            return Err(format!("`{value}` is not a figure"))
        }
    };
    let mut body = match (number.as_i64(), m.places) {
        (Some(i), None) if m.percent => i
            .checked_mul(100)
            .map(|p| p.to_string())
            .ok_or_else(|| format!("{i} as a percentage overflows"))?,
        (Some(i), None) => i.to_string(),
        _ => {
            let mut f = number
                .as_f64()
                .ok_or_else(|| format!("`{number}` is not a number this can write"))?;
            if m.percent {
                f *= 100.0;
            }
            match m.places {
                Some(p) => format!("{f:.p$}"),
                // `serde_json` writes a very large or very small float with an exponent, which
                // `grouped` would put commas into. Grouping asks for positional digits, and
                // `Display` writes the same shortest value without one.
                None if m.grouped && f.is_finite() => format!("{f}"),
                None => serde_json::Number::from_f64(f)
                    .map(|n| n.to_string())
                    .ok_or_else(|| format!("`{number}` is not a finite number"))?,
            }
        }
    };
    if m.grouped {
        body = grouped(&body);
    }
    if m.percent {
        body.push('%');
    }
    Ok(body)
}

/// Thousands separated with `,` in the integer part of a decimal number.
fn grouped(decimal: &str) -> String {
    let (sign, rest) = match decimal.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", decimal),
    };
    let (whole, fraction) = match rest.find('.') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    let mut out = String::with_capacity(decimal.len() + whole.len() / 3);
    out.push_str(sign);
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.push_str(fraction);
    out
}

/// Why a body may not be written into a node file, or `None` where it may (RFC-0058 §2.3).
///
/// A block in a node sits inside a YAML scalar. In a quoted scalar, a `"`, `'` or `\` would
/// change what the scalar means, and a newline would end the line the block has to sit on. In a
/// plain scalar, `: ` makes a mapping and ` #` starts a comment — the second still parses, and
/// silently drops the rest of the line. Every number qualifies. The write point re-parses a node
/// as well, for whatever this list does not name.
fn unsafe_in_yaml(body: &str) -> Option<&'static str> {
    if body.contains('\n') {
        Some("it is more than one line")
    } else if body.contains(['"', '\'', '\\']) {
        Some("it holds a quote or a backslash, which would change the YAML scalar it sits in")
    } else if body.contains(": ") || body.contains(" #") {
        Some("it holds `: ` or ` #`, which would change a plain YAML scalar it sits in")
    } else {
        None
    }
}

/// The value one citation resolves to, rendered, or why it does not resolve.
///
/// `own` is the node the block sits in — `None` outside a node file.
fn resolve(
    root: &Path,
    signals: &Signals,
    citation: &Citation,
    own: Option<&str>,
) -> std::result::Result<String, String> {
    let reference = match (&citation.node, own) {
        (Some(written), _) => crate::computed::node_reference(written)
            .ok_or_else(|| format!("`{written}` is not a reference to a node in this corpus"))?,
        (None, Some(own)) => own.to_string(),
        (None, None) => {
            return Err(format!(
                "`{}` names no node, and only a node file has a row of its own; write \
                 `yidam signal <node-ref> {}`",
                citation.name, citation.name
            ))
        }
    };
    let value = signals.value(root, &reference, &citation.name)?;
    render(value, &citation.modifiers)
}

/// `yidam signal [<node-ref> <name> [modifiers]]` — the value, or the refresh.
pub fn signal(root: Option<&Path>, words: &[String]) -> Result<()> {
    if words.is_empty() {
        return block(root);
    }
    let root = crate::paths::resolve_root(root)?;
    let argument = words.join(" ");
    let citation =
        parse(&argument).map_err(|why| anyhow::anyhow!("yidam signal {argument}: {why}"))?;
    if citation.node.is_none() {
        bail!(
            "yidam signal {argument}: name the node too — `yidam signal <node-ref> {}`. The bare \
             name is the form a node file uses for its own row.",
            citation.name
        );
    }
    let signals = Signals::load(&root);
    match resolve(&root, &signals, &citation, None) {
        Ok(body) => {
            println!("{body}");
            Ok(())
        }
        Err(why) => bail!("yidam signal {argument} in {}: {why}", root.display()),
    }
}

/// One block this generator answers for.
struct Block {
    /// Repository-relative, as `tracked::list` gives it.
    file: String,
    /// The command's argument, **verbatim** — see [`super::count`]'s `Block::argument` for why
    /// the rebuild needs it untrimmed.
    argument: String,
}

/// Every `signal` block in the tracked markdown and node files.
fn blocks(root: &Path) -> Result<Vec<Block>> {
    let mut found = Vec::new();
    for rel in super::tracked::list(root)? {
        if !crate::regen::reads_regen(&rel) {
            continue;
        }
        let path = root.join(&rel);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
        found.extend(
            in_document(&text, crate::regen::is_node_file(&rel))
                .into_iter()
                .map(|argument| Block {
                    file: rel.clone(),
                    argument,
                }),
        );
    }
    Ok(found)
}

/// The argument of every `signal` block in one file, verbatim. Masked first, and inline only in
/// a node file — [`crate::regen::block_commands`].
fn in_document(text: &str, node_file: bool) -> Vec<String> {
    crate::regen::block_commands(text, node_file)
        .into_iter()
        .filter_map(|command| command.strip_prefix(COMMAND_PREFIX).map(str::to_string))
        .collect()
}

/// Refresh every `signal` block in the repository.
///
/// **A block that does not resolve fails the command**, the way `count` fails on a query that
/// does not typecheck: an unknown name, a node with no row, a refused name, a modifier on a
/// string, or a value that cannot sit in YAML. It keeps whatever it holds. A figure that could
/// not be read has not been computed. Every block is visited first, so one bad block does not
/// hide the next or stop the others being refreshed.
pub fn block(root: Option<&Path>) -> Result<()> {
    let resolved = crate::paths::resolve_root(root)?;
    let blocks = blocks(&resolved)?;
    if blocks.is_empty() {
        return Ok(());
    }
    let signals = Signals::load(&resolved);

    let mut refused: Vec<(&Block, String)> = Vec::new();
    for b in &blocks {
        let node_file = crate::regen::is_node_file(&b.file);
        let own = if node_file {
            crate::computed::node_id(&b.file)
        } else {
            None
        };
        let body = parse(&b.argument).and_then(|citation| {
            let body = resolve(&resolved, &signals, &citation, own)?;
            match unsafe_in_yaml(&body) {
                Some(why) if node_file => Err(format!("`{body}` cannot be written here: {why}")),
                _ => Ok(body),
            }
        });
        let body = match body {
            Ok(body) => body,
            Err(why) => {
                refused.push((b, why));
                continue;
            }
        };
        // The command is rebuilt with the literal at the call site, so `generators_in_source`
        // can attribute it — see `count::block`.
        // A write the node's YAML would not survive is refused there, and is this block's
        // refusal rather than the end of the run.
        if let Err(e) = crate::regen::update_file_regen(
            &resolved.join(&b.file),
            &format!("yidam signal {}", b.argument),
            &body,
        ) {
            refused.push((b, format!("{e:#}")));
        }
    }

    if refused.is_empty() {
        return Ok(());
    }
    let mut detail = String::new();
    for (b, why) in &refused {
        let _ = writeln!(
            detail,
            "  {}  (yidam signal {})\n    {why}",
            b.file,
            b.argument.trim()
        );
    }
    bail!(
        "{} signal block(s) do not resolve:\n{}\n\n\
         Those blocks keep whatever they hold — a figure that could not be read has not been \
         computed. Correct the block, or the computed file it names.",
        refused.len(),
        detail.trim_end()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn m(percent: bool, places: Option<usize>, grouped: bool) -> Modifiers {
        Modifiers {
            percent,
            places,
            grouped,
        }
    }

    #[test]
    fn both_forms_and_the_modifiers_parse() {
        assert_eq!(
            parse("stable_max_s").unwrap(),
            Citation {
                node: None,
                name: "stable_max_s".into(),
                modifiers: Modifiers::default()
            }
        );
        assert_eq!(
            parse(" finding/x  share places=1 percent ").unwrap(),
            Citation {
                node: Some("finding/x".into()),
                name: "share".into(),
                modifiers: m(true, Some(1), false)
            }
        );
    }

    #[test]
    fn the_modifier_order_does_not_matter_and_a_repeat_is_refused() {
        let a = parse("share places=1 percent").unwrap();
        let b = parse("share percent places=1").unwrap();
        assert_eq!(a, b);
        assert_eq!(render(&json!(0.18234), &a.modifiers).unwrap(), "18.2%");
        for bad in [
            "share percent percent",
            "share places=1 places=2",
            "share grouped grouped",
        ] {
            assert!(parse(bad).unwrap_err().contains("twice"), "{bad}");
        }
    }

    #[test]
    fn a_malformed_argument_is_refused() {
        assert!(parse("").is_err());
        assert!(parse("percent").is_err());
        assert!(parse("share places=x").is_err());
        assert!(parse("share percent finding/x").is_err());
        assert!(parse("a b c").is_err());
    }

    #[test]
    fn each_value_kind_writes_its_documented_form() {
        let none = Modifiers::default();
        assert_eq!(render(&json!(569), &none).unwrap(), "569");
        assert_eq!(render(&json!(0.75), &none).unwrap(), "0.75");
        assert_eq!(render(&json!(7.0), &none).unwrap(), "7.0");
        assert_eq!(render(&json!("high"), &none).unwrap(), "high");
        assert_eq!(render(&json!(true), &none).unwrap(), "true");
        for v in [json!(null), json!([1]), json!({"a": 1})] {
            assert!(render(&v, &none).is_err(), "{v}");
        }
    }

    #[test]
    fn each_modifier_writes_its_documented_form() {
        assert_eq!(
            render(&json!(0.75), &m(false, Some(1), false)).unwrap(),
            "0.8"
        );
        assert_eq!(
            render(&json!(3), &m(false, Some(2), false)).unwrap(),
            "3.00"
        );
        assert_eq!(render(&json!(0.5), &m(true, None, false)).unwrap(), "50.0%");
        assert_eq!(render(&json!(2), &m(true, None, false)).unwrap(), "200%");
        assert_eq!(
            render(&json!(1234567), &m(false, None, true)).unwrap(),
            "1,234,567"
        );
        assert_eq!(
            render(&json!(-1234.5), &m(false, Some(2), true)).unwrap(),
            "-1,234.50"
        );
        assert_eq!(render(&json!(123), &m(false, None, true)).unwrap(), "123");
        assert_eq!(
            render(&json!(123.456), &m(true, Some(0), true)).unwrap(),
            "12,346%"
        );
        // Grouping never meets an exponent.
        assert_eq!(
            render(&json!(1.5e21), &m(false, None, true)).unwrap(),
            "1,500,000,000,000,000,000,000"
        );
        assert_eq!(
            render(&json!(1e-7), &m(false, None, true)).unwrap(),
            "0.0000001"
        );
    }

    #[test]
    fn a_modifier_on_a_string_is_refused() {
        for modifiers in [
            m(true, None, false),
            m(false, Some(1), false),
            m(false, None, true),
        ] {
            assert!(render(&json!("high"), &modifiers).is_err(), "{modifiers:?}");
        }
    }

    #[test]
    fn a_value_that_would_change_its_yaml_scalar_is_refused_in_a_node() {
        assert!(unsafe_in_yaml("0.75").is_none());
        assert!(unsafe_in_yaml("1,234.5%").is_none());
        for bad in [
            "say \"hi\"",
            "it's",
            "a\\b",
            "two\nlines",
            "high: see note",
            "high #1",
        ] {
            assert!(unsafe_in_yaml(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn a_block_form_in_a_node_file_is_not_read_and_an_inline_one_is() {
        let text = "description: |\n  Up to <!-- REGEN: yidam signal a -->1<!-- /REGEN --> s.\n  \
                    <!-- REGEN: yidam signal b -->\n  2\n  <!-- /REGEN -->\n";
        assert_eq!(in_document(text, true), ["a"]);
        assert_eq!(in_document(text, false), ["a", "b"]);
    }

    #[test]
    fn a_command_that_merely_begins_with_signal_is_not_one() {
        let text = "x <!-- REGEN: yidam signals -->1<!-- /REGEN --> y\n";
        assert!(in_document(text, false).is_empty());
        let shown = "See `<!-- REGEN: yidam signal a -->1<!-- /REGEN -->`.\n";
        assert!(in_document(shown, false).is_empty());
    }

    #[test]
    fn the_rebuilt_command_is_the_one_the_block_carries() {
        let text = "a <!-- REGEN: yidam signal  finding/x  share -->1<!-- /REGEN --> b\n";
        let found = in_document(text, false);
        assert_eq!(
            format!("yidam signal {}", found[0]),
            yidam_core::markers::scan_markers(text).regen[0].command
        );
        assert_eq!(format!("yidam signal {}", ""), COMMAND_PREFIX);
    }
}
