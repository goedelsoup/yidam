//! The machine-readable report contract — RFC-0016 Phase 0, RFC-0001's `reports/` family.
//!
//! Every report in this CLI renders prose to stdout and nothing else. A consumer that
//! wants a *verdict* has two options: scrape the prose, or re-implement the checks. One
//! downstream project chose re-implement — ~1,600 lines of Python whose own docstrings
//! claim faithfulness to Rust symbols it has already drifted from.
//!
//! So this module exists before any consumer does, and the rule it encodes is the one
//! RFC-0016 makes absolute: **the CLI computes verdicts; a client computes affordances.**
//! Anything a consumer would otherwise have to decide for itself — whether the gate passed,
//! whether a violation is inherited debt or a regression — is answered here.
//!
//! # The handshake
//!
//! [`FORMAT_VERSION`] and the `yidam` block are what let a consumer versioned independently
//! of the binary detect skew and degrade loudly. A consumer reading an unknown major version
//! must say so and disable verdict features; it must never mis-parse.
//!
//! # Text is the default and is byte-identical
//!
//! `--format text` is what every command printed before this module existed, unchanged. The
//! JSON path is additive: no existing output moved, and no exit code changed. A gate that
//! gates differently depending on how you asked for the answer is not a gate.

use serde::Serialize;

/// The contract's major version.
///
/// Bumped only when a consumer that understood the previous version would mis-read this
/// one — a removed field, a changed meaning, a narrowed type. Adding a field is not a
/// break: consumers must ignore what they do not know.
pub const FORMAT_VERSION: &str = "1";

/// Which yidam produced a report.
#[derive(Debug, Clone, Serialize)]
pub struct YidamBlock {
    /// Crate version, e.g. `0.1.0`.
    pub version: String,
    /// Short commit of the build, or `unknown` — see `build.rs`. Never guessed.
    pub commit: String,
    /// Cargo features compiled in. `reports` names the base and is always present; the rest
    /// gate whole subcommands, so a consumer can tell "this binary cannot do that" from
    /// "that failed".
    pub features: Vec<String>,
}

impl YidamBlock {
    pub fn current() -> Self {
        // Unconditional, and not an oversight: `reports` gates no code, so every build has
        // the capability it names whether or not the feature was spelled. A `cfg!` here
        // would report a *flag* where the rest of the list reports a capability. See the
        // note on the feature in Cargo.toml.
        let mut features = vec!["reports".to_string()];
        // Two entries, not one, and `index` implies the other. A client needs to tell three
        // builds apart: one that cannot read an index, one that can read but not build, and
        // one that can do both. The middle build is the point of the split — it needs no
        // protoc — and collapsing it into `index` would make it indistinguishable from the
        // build that carries lancedb.
        if cfg!(feature = "vector-read") {
            features.push("vector-read".to_string());
        }
        if cfg!(feature = "index") {
            features.push("index".to_string());
        }
        if cfg!(feature = "export-sqlite") {
            features.push("export-sqlite".to_string());
        }
        if cfg!(feature = "export-graph") {
            features.push("export-graph".to_string());
        }
        if cfg!(feature = "tonpa") {
            features.push("tonpa".to_string());
        }
        // Not a subcommand gate either: `serve --mcp` exists in every build, and this says
        // whether it can be reached by a URL rather than only by a subprocess. A report that
        // omitted it would call a stdio-only binary and a remotely reachable one the same
        // build, which is the difference #420 is entirely about.
        if cfg!(feature = "serve-http") {
            features.push("serve-http".to_string());
        }
        // Not a subcommand gate like the others: `yidam vault` exists in every build, and
        // this says whether it can reach an `s3://` store. `store.rs` refuses such a url by
        // telling the reader that this list is where to look, so the list has to carry it.
        if cfg!(feature = "vault-s3") {
            features.push("vault-s3".to_string());
        }
        // Not a subcommand gate either: `index-push` is, but what this says is broader. A
        // build carrying `s3-vectors` can *reach* a vector bucket — sign for it, query it,
        // mirror into it — and one without it answers a corpus that declares `[index.remote]`
        // by ignoring the declaration. That is exactly the difference a reader of this list
        // is trying to see.
        if cfg!(feature = "s3-vectors") {
            features.push("s3-vectors".to_string());
        }
        // Not a subcommand gate: `catalog-fetch` exists in every build and follows a
        // `kind: file` location in all of them. This says whether it can follow a `url` or a
        // `url_template` — which is the difference between a corpus that can re-fetch its
        // sources and one that can only re-read what it already holds.
        if cfg!(feature = "catalog-fetch") {
            features.push("catalog-fetch".to_string());
        }
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: env!("YIDAM_BUILD_COMMIT").to_string(),
            features,
        }
    }
}

/// The common envelope every report shares.
///
/// `report` is flattened, so a payload's own fields sit beside `format_version` rather than
/// under a wrapper key — which is what the RFC's example shows and what keeps a consumer
/// from having to know whether it is reading a lint or a status before it can find `root`.
#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub format_version: &'static str,
    pub yidam: YidamBlock,
    /// Absolute path to the repository the report was computed over.
    pub root: String,
    #[serde(flatten)]
    pub report: T,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(root: &std::path::Path, report: T) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            yidam: YidamBlock::current(),
            root: root.display().to_string(),
            report,
        }
    }
}

/// Print a report as JSON on stdout.
///
/// Pretty-printed with a trailing newline: these are read by people at least as often as by
/// programs, and `jq` does not care either way.
pub fn emit<T: Serialize>(root: &std::path::Path, report: T) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&Envelope::new(root, report))?
    );
    Ok(())
}

/// A gate that ran, reported its verdict, and failed (#926).
///
/// The thirteen commands that gate used to call `std::process::exit(1)` from inside this
/// library, which meant a caller that was not `main.rs` did not get its process back. The
/// verdict now travels as an error and the binary owns the exit, so the library's contract
/// is the one every other Rust library has: it returns.
///
/// It carries no message, and that is the point. The report is the description of what
/// failed — it has already been printed, in the format the caller asked for — so `main.rs`
/// exits 1 without writing anything further. An `anyhow::Error` that printed
/// `Error: gate failed` above a JSON report would be the binary answering a second time,
/// on the wrong stream, in a format no consumer parses.
///
/// A caller embedding this library distinguishes it from a real failure by downcasting:
///
/// ```no_run
/// # fn run() -> anyhow::Result<()> { Ok(()) }
/// match run() {
///     Ok(()) => println!("passed"),
///     Err(e) if e.downcast_ref::<yidam::report::GateFailed>().is_some() => println!("failed"),
///     Err(e) => return Err(e),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateFailed;

impl std::fmt::Display for GateFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Reached only by a caller that chose to print it; `main.rs` does not.
        f.write_str("gate failed")
    }
}

impl std::error::Error for GateFailed {}

/// `Ok(())` when a gate passed, [`GateFailed`] when it did not.
///
/// The epilogue of every gating command, so that the verdict-to-error translation exists
/// once rather than thirteen times. See [`GateFailed`] for why it is an error at all.
///
/// `pub(crate)`, where [`GateFailed`] is public: a caller embedding this crate has to be able
/// to *recognise* a failed gate, which means naming the type. It has no use for the helper
/// that builds one — that is this crate's own epilogue.
///
/// Most callers reach it through [`gate`], which is the epilogue with the format branch in
/// front of it (#927). The two that call it directly — `vault`'s two checks — print neither
/// a report nor a rendering, so there is nothing for [`gate`] to do for them.
pub(crate) fn verdict(passed: bool) -> anyhow::Result<()> {
    if passed {
        Ok(())
    } else {
        Err(GateFailed.into())
    }
}

/// The report epilogue: the one branch on `--format`, in one place (#927).
///
/// Every command that takes `--format` ends the same way — emit the envelope, or print the
/// prose — and it was written out at each of them. Thirty-odd copies of a three-line branch
/// is not expensive to read individually; what it costs is that a change to the contract has
/// thirty-odd sites to reach, and the ones it misses look exactly like the ones it did not
/// need to. `emit` gaining a stream, a pager, or a `--format yaml` would be that change.
///
/// **The text arm prints rather than returning a string.** Four commands end their rendering
/// with a newline and print it with `print!`; the rest do not and use `println!`. A helper
/// that took a `String` would have to pick one and silently move a byte under the other four,
/// and `--format text` being byte-identical to what these commands have always printed is
/// `report_goldens.rs`'s first obligation. So the caller keeps its own `print!`, and what
/// moves here is the branch and the order.
pub(crate) fn finish<R: Serialize>(
    root: &std::path::Path,
    format: Format,
    report: R,
    text: impl FnOnce(&R),
) -> anyhow::Result<()> {
    if format.is_json() {
        emit(root, report)
    } else {
        text(&report);
        Ok(())
    }
}

/// [`finish`], and then the gate's verdict (#926).
///
/// `passed` is the caller's, not read off the report: the commands that gate disagree about
/// where the verdict lives — `report.passed` for four of them, `rejected.is_some()` for
/// three, an empty `blocked` list for two — and a trait to paper over that would be three
/// implementations of one field access.
///
/// The order is the load-bearing part. The report is printed **before** the verdict is
/// returned, in both formats, so a failing gate still says what failed. See [`GateFailed`]
/// for why the verdict carries no message of its own.
pub(crate) fn gate<R: Serialize>(
    root: &std::path::Path,
    format: Format,
    report: R,
    passed: bool,
    text: impl FnOnce(&R),
) -> anyhow::Result<()> {
    finish(root, format, report, text)?;
    verdict(passed)
}

/// Where a violation sits in its file. Best-effort, output-only.
///
/// **Never part of a violation's identity.** The baseline compares on `(check id, node)`
/// deliberately; a line number in that comparison would make the baseline churn on every
/// edit above a violation, which is how a ratchet becomes noise and then gets deleted.
/// A violation with no span anchors at the file's first line, and that is a normal outcome
/// rather than a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub line: usize,
}

/// Output format for a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Format {
    /// Human-readable prose, byte-identical to what this CLI has always printed.
    #[default]
    Text,
    /// The machine-readable contract in this module.
    Json,
}

impl Format {
    pub fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Payload {
        answer: u8,
    }

    /// Exactly one arm of [`finish`] runs, whichever format was asked for.
    ///
    /// Both arms running is the failure the goldens cannot see: they capture one format per
    /// case, so an envelope with the prose printed underneath it would match the JSON golden
    /// on its first line and the text golden on none, and only the second is compared.
    #[test]
    fn finish_renders_under_text_and_emits_under_json() {
        let root = std::path::Path::new("/r");
        let rendered = std::cell::Cell::new(0u8);

        finish(root, Format::Text, Payload { answer: 1 }, |r| {
            assert_eq!(
                r.answer, 1,
                "the renderer is handed the report that was built"
            );
            rendered.set(rendered.get() + 1);
        })
        .unwrap();
        assert_eq!(rendered.get(), 1, "`--format text` renders, once");

        finish(root, Format::Json, Payload { answer: 1 }, |_| {
            rendered.set(rendered.get() + 1);
        })
        .unwrap();
        assert_eq!(
            rendered.get(),
            1,
            "`--format json` emitted the envelope and rendered nothing"
        );
    }

    /// A failing gate has already reported by the time it fails — in both formats.
    ///
    /// This is the order the thirty-odd hand-written epilogues had, and the one thing about
    /// them that was never stated anywhere. A gate that returned its verdict first would exit
    /// 1 with an empty stdout, which a `--format json` consumer cannot tell from a truncated
    /// pipe — `cmd::retrieve` writes that argument out at length.
    #[test]
    fn a_failing_gate_reports_before_it_returns_the_verdict() {
        let root = std::path::Path::new("/r");

        let reported = std::cell::Cell::new(false);
        let err = gate(root, Format::Text, Payload { answer: 0 }, false, |_| {
            reported.set(true);
        })
        .unwrap_err();
        assert!(
            reported.get(),
            "the report was printed before the gate failed"
        );
        assert!(
            err.downcast_ref::<GateFailed>().is_some(),
            "a failed gate is `GateFailed`, so `main` prints nothing above the report"
        );

        // The JSON arm reaches the same verdict, and the renderer is the negative control:
        // it would panic if the format branch let both arms through.
        let err = gate(root, Format::Json, Payload { answer: 0 }, false, |_| {
            panic!("the JSON arm must not render")
        })
        .unwrap_err();
        assert!(err.downcast_ref::<GateFailed>().is_some());

        gate(root, Format::Text, Payload { answer: 0 }, true, |_| {})
            .expect("a gate that passed returns Ok");
    }

    #[test]
    fn the_envelope_flattens_its_payload_beside_the_handshake() {
        let e = Envelope::new(std::path::Path::new("/r"), Payload { answer: 42 });
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(v["format_version"], FORMAT_VERSION);
        assert_eq!(v["root"], "/r");
        // Flattened, not nested under `report`.
        assert_eq!(v["answer"], 42);
        assert!(v.get("report").is_none());
    }

    #[test]
    fn a_verdict_is_ok_when_the_gate_passed_and_an_error_when_it_did_not() {
        assert!(verdict(true).is_ok());
        let e = verdict(false).unwrap_err();
        assert!(e.downcast_ref::<GateFailed>().is_some());
    }

    #[test]
    fn a_gate_failure_is_still_recognisable_under_a_context_layer() {
        // `main.rs` decides whether to print by downcasting, and two of the thirteen sites
        // (`vault::pull`, `vault::pull_derived`) return through a caller that could grow a
        // `.context(..)` at any time. If anyhow stopped carrying the concrete type through
        // its context chain, the binary would silently start printing `Error: gate failed`
        // above every JSON report — visible only to whoever was parsing the stream.
        use anyhow::Context;
        let wrapped = verdict(false)
            .context("while pulling the vault")
            .unwrap_err();
        assert!(wrapped.downcast_ref::<GateFailed>().is_some());
    }

    #[test]
    fn the_yidam_block_always_carries_a_commit_field() {
        let b = YidamBlock::current();
        assert!(
            !b.commit.is_empty(),
            "commit is never omitted, only `unknown`"
        );
        assert!(b.features.contains(&"reports".to_string()));
        assert_eq!(b.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn text_is_the_default_format() {
        assert_eq!(Format::default(), Format::Text);
        assert!(!Format::default().is_json());
    }
}
