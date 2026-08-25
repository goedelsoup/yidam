//! The query language: text to a path, per RFC-0018.
//!
//! # Lexing is one rule, and it is the reason the grammar works
//!
//! > **Split on whitespace that is not inside `"…"` or `[…]`. Classify each token by its own
//! > shape. Never re-lex a token.**
//!
//! Relationship names contain hyphens — four of `examples/streamflow`'s six do, and 173 of
//! the 275 relationships authored across this repository's example and test corpora — and so
//! do the hop arrows. A character-level grammar in which `-` is both an ident continuation
//! and an arrow affix has *no* parse of `reach -measured-by-> gage`, and two parses of
//! `assumption <-supports- study-design` (`supports`/`study-design`, and
//! `supports-study`/`design`), both halves of which are real names in the test corpora.
//!
//! So a hop is one whitespace-delimited token, the relationship is whatever sits inside its
//! fixed affixes, taken verbatim, and whitespace around a hop is **required**. That is the
//! only syntactic obligation this surface imposes, and it buys an unambiguous grammar with
//! no backtracking and no lookahead.
//!
//! The alternative — lexing against the corpus's declared names, so the longest known
//! relationship wins — was rejected in the RFC: it makes the same string parse differently
//! in two repositories.

use std::fmt;

/// A predicate's comparison.
///
/// `Contains` is **contiguous, case-insensitive substring containment over the value's
/// serialized text**. Deliberately *not* `keyword_retrieve`'s rule, which splits on
/// whitespace, matches any term at any distance, scores the result, and searches three
/// concatenated fields. Only the single-word case coincides, and a surface that borrowed the
/// name without the semantics would be the worse kind of consistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Contains,
}

impl Op {
    pub fn as_str(&self) -> &'static str {
        match self {
            Op::Eq => "=",
            Op::Ne => "!=",
            Op::Contains => "~",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pred {
    pub prop: String,
    pub op: Op,
    pub value: String,
}

/// One step: a class pattern, optionally entered by similarity, optionally filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// A declared class name, or `*`.
    pub class: String,
    /// The anchor text of a similarity entry, if this step is one.
    ///
    /// The class is **required** even here. A bare `~"…"` cannot be typechecked before it
    /// runs: the first hop's verdict depends on the source class's `edge_policy`, and the
    /// source class would be whatever retrieval happened to return.
    pub anchor: Option<String>,
    pub filter: Vec<Pred>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// `-rel->` — follow the edge in its authoring direction.
    Out,
    /// `<-rel-` — follow it backwards.
    In,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    pub relationship: String,
    pub direction: Dir,
}

/// A path: `steps.len() == hops.len() + 1`, always.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub steps: Vec<Step>,
    pub hops: Vec<Hop>,
}

impl Query {
    /// The step a hop leaves from, and the one it lands on.
    pub fn ends(&self, hop: usize) -> (&Step, &Step) {
        (&self.steps[hop], &self.steps[hop + 1])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// Token index, where the failure is attributable to one.
    pub token: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

fn err(message: impl Into<String>, token: Option<usize>) -> ParseError {
    ParseError {
        message: message.into(),
        token,
    }
}

// ── lexing ────────────────────────────────────────────────────────────────────

/// Split on whitespace that is not inside `"…"` or `[…]`.
///
/// A backslash escapes the next character inside a quoted span, so `\"` does not close it.
pub fn tokens(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut depth = 0usize;
    let mut escaped = false;
    for ch in query.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => {
                cur.push(ch);
                escaped = true;
                continue;
            }
            '"' => quoted = !quoted,
            '[' if !quoted => depth += 1,
            ']' if !quoted => depth = depth.saturating_sub(1),
            _ => {}
        }
        if ch.is_whitespace() && !quoted && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// What a token is, by its own shape and nothing else.
enum Shape<'a> {
    Forward(&'a str),
    Backward(&'a str),
    Step(&'a str),
}

fn shape(token: &str) -> Shape<'_> {
    // Length guards keep `->` and `<--` out: an arrow with no relationship inside it names
    // no relationship, and is a step token that will fail to be a class.
    if let Some(rest) = token.strip_prefix("<-").and_then(|r| r.strip_suffix('-')) {
        if !rest.is_empty() {
            return Shape::Backward(rest);
        }
    }
    if let Some(rest) = token.strip_prefix('-').and_then(|r| r.strip_suffix("->")) {
        if !rest.is_empty() {
            return Shape::Forward(rest);
        }
    }
    Shape::Step(token)
}

// ── parsing ───────────────────────────────────────────────────────────────────

fn ident_ok(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A relationship name this surface cannot express.
///
/// Reported by name rather than mis-lexed. The affix rule means a name that begins or ends
/// with `-` would make `<-rel-` ambiguous again, and one containing `>` or whitespace could
/// not survive tokenizing.
fn relationship_ok(name: &str) -> bool {
    !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains(['>', '<', '[', ']', '"', '~'])
        && !name.chars().any(char::is_whitespace)
}

/// Unescape a quoted span's body: `\"` and `\\` only.
fn unquote(raw: &str, token: usize) -> Result<String, ParseError> {
    let body = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .ok_or_else(|| err(format!("unterminated quoted value in `{raw}`"), Some(token)))?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some(next @ ('"' | '\\')) => out.push(next),
            Some(other) => {
                return Err(err(
                    format!("`\\{other}` is not an escape — only `\\\"` and `\\\\` are"),
                    Some(token),
                ))
            }
            None => return Err(err("a value ends in a lone backslash", Some(token))),
        }
    }
    Ok(out)
}

fn parse_value(raw: &str, token: usize) -> Result<String, ParseError> {
    match raw.starts_with('"') {
        true => unquote(raw, token),
        false => match raw.is_empty() {
            true => Err(err("a predicate has no value", Some(token))),
            false => Ok(raw.to_string()),
        },
    }
}

fn parse_filter(raw: &str, token: usize) -> Result<Vec<Pred>, ParseError> {
    let mut out = Vec::new();
    // Split on commas outside quotes; a value may legitimately contain one.
    let mut parts = Vec::new();
    let (mut cur, mut quoted, mut escaped) = (String::new(), false, false);
    for ch in raw.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => {
                cur.push(ch);
                escaped = true;
                continue;
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    parts.push(cur);

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            return Err(err("an empty predicate between commas", Some(token)));
        }
        // First operator character wins, so a value may contain `=` and `~`.
        let at = part.find(['=', '!', '~']).ok_or_else(|| {
            err(
                format!("`{part}` is not a predicate — it has no operator"),
                Some(token),
            )
        })?;
        let (prop, rest) = part.split_at(at);
        let (op, value) = match rest.as_bytes() {
            [b'!', b'=', ..] => (Op::Ne, &rest[2..]),
            [b'!', ..] => {
                return Err(err(
                    format!("`!` is not an operator — write `!=` (in `{part}`)"),
                    Some(token),
                ))
            }
            [b'=', ..] => (Op::Eq, &rest[1..]),
            _ => (Op::Contains, &rest[1..]),
        };
        let prop = prop.trim();
        if !ident_ok(prop) {
            return Err(err(format!("`{prop}` is not a property name"), Some(token)));
        }
        out.push(Pred {
            prop: prop.to_string(),
            op,
            value: parse_value(value.trim(), token)?,
        });
    }
    Ok(out)
}

fn parse_step(raw: &str, token: usize) -> Result<Step, ParseError> {
    // `[` before `~` would put the filter inside the anchor text, which the tokenizer's
    // bracket depth already forbids; splitting on the first of either is enough.
    let (head, filter) = match raw.split_once('[') {
        Some((head, rest)) => {
            let body = rest
                .strip_suffix(']')
                .ok_or_else(|| err(format!("unclosed `[` in `{raw}`"), Some(token)))?;
            (head, parse_filter(body, token)?)
        }
        None => (raw, Vec::new()),
    };
    let (class, anchor) = match head.split_once('~') {
        Some((class, text)) => (class, Some(parse_value(text, token)?)),
        None => (head, None),
    };
    if class == "*" {
        return Ok(Step {
            class: class.to_string(),
            anchor,
            filter,
        });
    }
    if !ident_ok(class) {
        return Err(err(
            match class.is_empty() {
                true => "a step names no class — an anchor must say which class it enters \
                         (`concept~\"…\"`), because a hop out of an unknown class cannot be \
                         checked before it runs"
                    .to_string(),
                false => format!("`{class}` is not a class name"),
            },
            Some(token),
        ));
    }
    Ok(Step {
        class: class.to_string(),
        anchor,
        filter,
    })
}

pub fn parse(query: &str) -> Result<Query, ParseError> {
    let tokens = tokens(query);
    if tokens.is_empty() {
        return Err(err("empty query", None));
    }
    let mut steps = Vec::new();
    let mut hops = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match shape(token) {
            // The invariant, stated once: a step is expected exactly when the counts are
            // equal, and a hop exactly when there is one more step than hop. Anything else
            // is a shape error, and saying which one is most of the diagnosis.
            Shape::Step(raw) => {
                if steps.len() != hops.len() {
                    return Err(err(
                        format!("two steps in a row — `{token}` needs a hop before it"),
                        Some(index),
                    ));
                }
                steps.push(parse_step(raw, index)?);
            }
            Shape::Forward(rel) | Shape::Backward(rel) => {
                if steps.len() != hops.len() + 1 {
                    return Err(err(
                        format!("`{token}` has no step to leave from"),
                        Some(index),
                    ));
                }
                if !relationship_ok(rel) {
                    return Err(err(
                        format!(
                            "`{rel}` cannot be written as a hop: a relationship name may not \
                             begin or end with `-`, or contain whitespace, `<`, `>`, `[`, \
                             `]`, `\"` or `~`"
                        ),
                        Some(index),
                    ));
                }
                hops.push(Hop {
                    relationship: rel.to_string(),
                    direction: match shape(token) {
                        Shape::Backward(_) => Dir::In,
                        _ => Dir::Out,
                    },
                });
            }
        }
    }
    if steps.len() != hops.len() + 1 {
        return Err(err(
            "the query ends on a hop — every hop has to land on a class",
            None,
        ));
    }
    Ok(Query { steps, hops })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(class: &str) -> Step {
        Step {
            class: class.to_string(),
            anchor: None,
            filter: Vec::new(),
        }
    }

    #[test]
    fn a_bare_class_is_a_one_step_path() {
        let q = parse("reach").unwrap();
        assert_eq!(q.steps, vec![step("reach")]);
        assert!(q.hops.is_empty());
    }

    /// The case a character-level grammar cannot parse: `-` is the arrow's tail and a legal
    /// character inside the relationship it wraps.
    #[test]
    fn a_hyphenated_relationship_lexes_because_the_hop_is_one_token() {
        let q = parse("reach -measured-by-> gage").unwrap();
        assert_eq!(q.steps, vec![step("reach"), step("gage")]);
        assert_eq!(
            q.hops,
            vec![Hop {
                relationship: "measured-by".to_string(),
                direction: Dir::Out
            }]
        );
    }

    /// The two-parse case, from the causal-inference test corpus: `supports` on
    /// `study-design`, against `supports-study` on `design`. Only one of those is a token.
    #[test]
    fn a_backward_hop_takes_its_relationship_verbatim() {
        let q = parse("assumption <-supports- study-design").unwrap();
        assert_eq!(q.steps[1].class, "study-design");
        assert_eq!(q.hops[0].relationship, "supports");
        assert_eq!(q.hops[0].direction, Dir::In);

        let q = parse("intervention <-contrasts-with- intervention").unwrap();
        assert_eq!(q.hops[0].relationship, "contrasts-with");
    }

    #[test]
    fn a_path_alternates_steps_and_hops() {
        let q = parse("reach -measured-by-> gage -sources-from-> concept").unwrap();
        assert_eq!(q.steps.len(), 3);
        assert_eq!(q.hops.len(), 2);
        let (from, to) = q.ends(1);
        assert_eq!(from.class, "gage");
        assert_eq!(to.class, "concept");
    }

    #[test]
    fn a_query_that_ends_on_a_hop_is_refused() {
        let e = parse("reach -measured-by->").unwrap_err();
        assert!(e.message.contains("ends on a hop"), "{e}");
    }

    #[test]
    fn two_steps_in_a_row_are_refused() {
        let e = parse("reach gage").unwrap_err();
        assert!(e.message.contains("two steps in a row"), "{e}");
    }

    #[test]
    fn a_hop_with_no_step_to_leave_from_is_refused() {
        let e = parse("-measured-by-> gage").unwrap_err();
        assert!(e.message.contains("no step to leave from"), "{e}");
    }

    /// Whitespace around a hop is the one syntactic obligation, so its absence has to be a
    /// diagnosis rather than a surprise: the whole string is then one step token.
    #[test]
    fn a_hop_without_whitespace_is_not_a_hop() {
        let e = parse("reach-measured-by->gage").unwrap_err();
        assert!(e.message.contains("is not a class name"), "{e}");
    }

    // ── predicates ────────────────────────────────────────────────────────────

    #[test]
    fn a_filter_parses_its_operators() {
        let q = parse("reach[regulated~yes,length_km=24,claim_tag!=open]").unwrap();
        assert_eq!(
            q.steps[0].filter,
            vec![
                Pred {
                    prop: "regulated".into(),
                    op: Op::Contains,
                    value: "yes".into()
                },
                Pred {
                    prop: "length_km".into(),
                    op: Op::Eq,
                    value: "24".into()
                },
                Pred {
                    prop: "claim_tag".into(),
                    op: Op::Ne,
                    value: "open".into()
                },
            ]
        );
    }

    /// The value `examples/streamflow` actually holds. It has spaces, a comma-free em dash,
    /// and would end the token without quoting.
    #[test]
    fn a_quoted_value_survives_spaces_and_non_ascii() {
        let q = parse(r#"reach[regulated="yes — inherited from upstream"]"#).unwrap();
        assert_eq!(q.steps[0].filter[0].value, "yes — inherited from upstream");
    }

    #[test]
    fn a_quoted_value_escapes_only_quote_and_backslash() {
        let q = parse(r#"reach[label="a \"quoted\" thing"]"#).unwrap();
        assert_eq!(q.steps[0].filter[0].value, "a \"quoted\" thing");
        let e = parse(r#"reach[label="a \n thing"]"#).unwrap_err();
        assert!(e.message.contains("is not an escape"), "{e}");
    }

    /// A bare value may begin with a digit, which an ident may not — `observed_on=2026-08`
    /// is the case that proves `bare` is a wider class than `ident`.
    #[test]
    fn a_bare_value_may_begin_with_a_digit() {
        let q = parse("gage[parameter=00060,observed_on=2026-08]").unwrap();
        assert_eq!(q.steps[0].filter[0].value, "00060");
        assert_eq!(q.steps[0].filter[1].value, "2026-08");
    }

    /// First operator character wins, so a value containing `=` needs no escape.
    #[test]
    fn the_first_operator_character_wins() {
        let q = parse("gage[units=cfs=1]").unwrap();
        assert_eq!(q.steps[0].filter[0].op, Op::Eq);
        assert_eq!(q.steps[0].filter[0].value, "cfs=1");
    }

    #[test]
    fn a_lone_bang_is_diagnosed_rather_than_read_as_contains() {
        let e = parse("reach[claim_tag!open]").unwrap_err();
        assert!(e.message.contains("write `!=`"), "{e}");
    }

    // ── anchors ───────────────────────────────────────────────────────────────

    #[test]
    fn an_anchor_carries_its_class() {
        let q = parse(r#"concept~"splitting a hydrograph" -refines-> concept"#).unwrap();
        assert_eq!(q.steps[0].class, "concept");
        assert_eq!(q.steps[0].anchor.as_deref(), Some("splitting a hydrograph"));
    }

    /// A classless anchor cannot be typechecked before it runs, so it is a parse error
    /// rather than a silently deferred check.
    #[test]
    fn a_classless_anchor_is_refused_with_the_reason() {
        let e = parse(r#"~"splitting a hydrograph""#).unwrap_err();
        assert!(e.message.contains("names no class"), "{e}");
        assert!(
            e.message.contains("cannot be checked before it runs"),
            "{e}"
        );
    }

    #[test]
    fn an_anchor_may_carry_a_filter() {
        let q = parse(r#"*~"low flow"[claim_tag=open]"#).unwrap();
        assert_eq!(q.steps[0].class, "*");
        assert_eq!(q.steps[0].anchor.as_deref(), Some("low flow"));
        assert_eq!(q.steps[0].filter.len(), 1);
    }

    // ── tokenizing ────────────────────────────────────────────────────────────

    #[test]
    fn whitespace_inside_quotes_and_brackets_does_not_split() {
        assert_eq!(
            tokens(r#"reach[a = 1, b = 2] -x-> concept~"two words""#),
            vec!["reach[a = 1, b = 2]", "-x->", r#"concept~"two words""#]
        );
    }

    #[test]
    fn a_relationship_this_surface_cannot_write_is_named_rather_than_mis_lexed() {
        let e = parse("reach -a>b-> gage").unwrap_err();
        assert!(e.message.contains("cannot be written as a hop"), "{e}");
    }
}
