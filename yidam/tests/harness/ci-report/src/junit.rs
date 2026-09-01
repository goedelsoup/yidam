//! Reading what nextest wrote.
//!
//! nextest's JUnit is regular and this reader is narrow: `<testsuites>` holding `<testsuite>`
//! holding `<testcase>`, each case optionally carrying `<failure>` and — with
//! `store-success-output` on, which `.config/nextest.toml` sets — `<system-out>` and
//! `<system-err>`.
//!
//! **What is not in this file is the point of it.** A `#[ignore]`d test appears nowhere in
//! nextest's JUnit: run `--test vault_s3` with its three ignored tests and the document is
//! `tests="0" skipped="0"` with no children, while the human summary says "3 skipped". A
//! census built on this file alone would report zero skipped for a suite that ran nothing,
//! which is the failure #462 exists to remove, arrived at from the inside. The ignored set
//! comes from `nextest list --message-format json` instead — see `census.rs`.

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub suite: String,
    pub name: String,
    /// Seconds, as nextest reports them. `None` when the attribute is absent rather than
    /// zero: a case with no timing and a case that took no time are different facts, and
    /// the slowest-tests table must not invent a `0.000` for the first.
    pub time: Option<f64>,
    pub failure: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

impl Case {
    pub fn id(&self) -> String {
        format!("{} {}", self.suite, self.name)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Run {
    pub cases: Vec<Case>,
}

impl Run {
    pub fn failed(&self) -> impl Iterator<Item = &Case> {
        self.cases.iter().filter(|c| c.failure.is_some())
    }

    pub fn passed(&self) -> usize {
        self.cases.len() - self.failed().count()
    }
}

/// One attribute's value, with entities resolved.
///
/// `normalized_value` rather than the `unescape_value` this used at quick-xml 0.37: the two
/// advisories that forced the upgrade (RUSTSEC-2026-0194/0195) came with a rename, and the
/// old name is deprecated rather than gone. Following the rename is what keeps the next
/// upgrade from being a second decision.
fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == key.as_bytes())
            .then(|| {
                a.normalized_value(XmlVersion::Implicit1_0)
                    .ok()
                    .map(|v| v.into_owned())
            })
            .flatten()
    })
}

/// Parse one nextest JUnit document.
///
/// Empty input is an error rather than an empty run. A report of nothing is the thing a
/// summary step degrades into when its input path is wrong, and it looks exactly like a
/// suite that passed — see `main.rs`, which refuses to emit one.
pub fn parse(xml: &str) -> Result<Run> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut run = Run::default();
    let mut suite = String::new();
    let mut current: Option<Case> = None;
    // Which child of a <testcase> we are inside, so text lands in the right field.
    let mut sink: Option<&'static str> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => bail!(
                "malformed JUnit XML at byte {}: {e}",
                reader.buffer_position()
            ),
            Ok(Event::Eof) => break,
            // `Start` and `Empty` carry the same attributes and differ only in whether an
            // `End` will follow. That difference is load-bearing: a self-closing
            // `<testcase … />` — which is every passing test in node's reporter — produces
            // no `End`, so a reader that only pushes a case on `End` silently returns an
            // empty run for a suite of 183 passing tests. `closed` is what keeps the two
            // element forms from being two different parsers.
            Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
                let closed = matches!(event, Event::Empty(_));
                let e = match &event {
                    Event::Start(e) | Event::Empty(e) => e.clone(),
                    _ => unreachable!("the match arm admits only these two"),
                };
                match e.name().as_ref() {
                    b"testsuite" => suite = attr(&e, "name").unwrap_or_default(),
                    b"testcase" => {
                        let case = Case {
                            suite: attr(&e, "classname").unwrap_or_else(|| suite.clone()),
                            name: attr(&e, "name").unwrap_or_default(),
                            time: attr(&e, "time").and_then(|t| t.parse().ok()),
                            // node's reporter also puts the message on the testcase itself.
                            failure: attr(&e, "failure"),
                            stdout: String::new(),
                            stderr: String::new(),
                        };
                        if closed {
                            run.cases.push(case);
                        } else {
                            current = Some(case);
                        }
                    }
                    b"failure" | b"error" => {
                        if let Some(c) = current.as_mut() {
                            // The message attribute is the one-line reason; the element body
                            // is the panic output. Seed with the attribute so a `<failure/>`
                            // with no body still names something.
                            c.failure = Some(attr(&e, "message").unwrap_or_default());
                            if !closed {
                                sink = Some("failure");
                            }
                        }
                    }
                    b"system-out" if !closed => sink = Some("stdout"),
                    b"system-err" if !closed => sink = Some("stderr"),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(c), Some(field)) = (current.as_mut(), sink) {
                    // `xml_content` resolves entities in character data, the counterpart to
                    // `normalized_value` for attributes. Test output is full of `&lt;` and
                    // `&amp;`, and handing those to a reader is the reason this crate parses
                    // XML rather than scanning it.
                    let text = t
                        .xml_content(XmlVersion::Implicit1_0)
                        .unwrap_or_default()
                        .into_owned();
                    match field {
                        "stdout" => c.stdout.push_str(&text),
                        "stderr" => c.stderr.push_str(&text),
                        "failure" => {
                            let body = c.failure.get_or_insert_with(String::new);
                            if !text.trim().is_empty() {
                                body.push('\n');
                                body.push_str(&text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::CData(t)) => {
                if let (Some(c), Some(field)) = (current.as_mut(), sink) {
                    let text = String::from_utf8_lossy(t.as_ref()).into_owned();
                    match field {
                        "stdout" => c.stdout.push_str(&text),
                        "stderr" => c.stderr.push_str(&text),
                        "failure" => c.failure.get_or_insert_with(String::new).push_str(&text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"testcase" => {
                    if let Some(c) = current.take() {
                        run.cases.push(c);
                    }
                    sink = None;
                }
                b"system-out" | b"system-err" | b"failure" | b"error" => sink = None,
                _ => {}
            },
            Ok(_) => {}
        }
        buf.clear();
    }

    Ok(run)
}

/// Read and parse a JUnit file, naming the path when it cannot be read.
pub fn read(path: &std::path::Path) -> Result<Run> {
    let xml = std::fs::read_to_string(path)
        .with_context(|| format!("reading JUnit XML at {}", path.display()))?;
    parse(&xml).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim from `cargo nextest run --profile ci`, trimmed to two cases. Committed shape
    /// rather than a shape this file imagines: the reader's job is to read what nextest
    /// writes, and a fixture written from the parser's own assumptions tests nothing.
    const REAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="2" skipped="0" failures="1" errors="0" uuid="d0" timestamp="2026-08-31T23:20:34.598-04:00" time="0.013">
    <testsuite name="yidam::formal_specs" tests="2" skipped="0" errors="0" failures="1">
        <testcase name="the_lean_toolchain_is_pinned" classname="yidam::formal_specs" timestamp="2026-08-31T23:20:34.598-04:00" time="0.010">
            <system-out>
running 1 test
test the_lean_toolchain_is_pinned ... ok
</system-out>
            <system-err></system-err>
        </testcase>
        <testcase name="a_workflow_runs_the_verification_task" classname="yidam::formal_specs" timestamp="2026-08-31T23:20:34.598-04:00" time="1.250">
            <failure message="assertion failed: 3 &lt; 2 &amp;&amp; x != &quot;y&quot;" type="test failure">thread panicked</failure>
            <system-out>running 1 test</system-out>
        </testcase>
    </testsuite>
</testsuites>"#;

    #[test]
    fn it_reads_cases_suites_and_timings() {
        let run = parse(REAL).expect("the committed nextest output should parse");
        assert_eq!(run.cases.len(), 2);
        assert_eq!(run.cases[0].suite, "yidam::formal_specs");
        assert_eq!(run.cases[0].name, "the_lean_toolchain_is_pinned");
        assert_eq!(run.cases[0].time, Some(0.010));
        assert_eq!(run.cases[1].time, Some(1.250));
        assert_eq!(run.passed(), 1);
        assert_eq!(run.failed().count(), 1);
    }

    /// The reason quick-xml is a dependency and this is not a `grep`.
    ///
    /// A failure message is test output, and test output contains the characters XML escapes.
    /// A reader that hands back `3 &lt; 2` makes the summary a worse place to read a failure
    /// than the log it was meant to replace.
    #[test]
    fn it_unescapes_entities_in_a_failure_message() {
        let run = parse(REAL).unwrap();
        let failure = run.cases[1].failure.as_deref().unwrap();
        assert!(
            failure.contains(r#"assertion failed: 3 < 2 && x != "y""#),
            "entities survived into the summary: {failure}"
        );
    }

    #[test]
    fn it_keeps_captured_stdout_because_the_skip_census_reads_it() {
        let run = parse(REAL).unwrap();
        assert!(run.cases[0].stdout.contains("running 1 test"));
    }

    /// nextest writes this document for a run whose every test was `#[ignore]`d. It is well
    /// formed, it is not an error, and it says nothing — which is why `main.rs` and not this
    /// function decides that a report of nothing must not be published.
    #[test]
    fn a_run_with_no_cases_parses_and_is_empty() {
        let empty = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="0" skipped="0" failures="0" errors="0" time="0.000">
</testsuites>"#;
        assert_eq!(parse(empty).unwrap().cases.len(), 0);
    }

    /// Verbatim from `node --test --test-reporter=junit`, which the VS Code gate runs.
    ///
    /// A second producer, and it writes a shape nextest never does: every passing test is a
    /// **self-closing** `<testcase … />` directly under `<testsuites>`, with no `<testsuite>`
    /// wrapper and no `<system-out>`. The first version of this reader pushed a case only on
    /// the closing tag, so it read 183 passing tests as an empty run — and `main.rs`, which
    /// refuses to publish a summary of nothing, is what turned that into an error message
    /// instead of a blank summary under a green check.
    const NODE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuites>
	<testcase name="an explicit setting outranks discovery" time="0.000480" classname="test" file="/x/test/binary.test.ts"/>
	<testcase name="a setting pointing at nothing fails" time="0.011432" classname="test" file="/x/test/binary.test.ts"/>
	<testcase name="fails" time="0.000425" classname="test" failure="Expected values to be strictly equal:1 !== 2">
		<failure type="testCodeFailure" message="Expected values to be strictly equal:1 !== 2">stack</failure>
	</testcase>
</testsuites>"#;

    #[test]
    fn it_reads_the_other_runners_self_closing_cases() {
        let run = parse(NODE).expect("node's reporter output should parse");
        assert_eq!(run.cases.len(), 3, "self-closing cases were dropped");
        assert_eq!(run.cases[0].name, "an explicit setting outranks discovery");
        assert_eq!(run.cases[0].suite, "test");
        assert_eq!(run.passed(), 2);
        assert_eq!(run.failed().count(), 1);
    }

    #[test]
    fn malformed_input_is_an_error_rather_than_an_empty_run() {
        assert!(parse("<testsuites><testcase name=").is_err());
    }
}
