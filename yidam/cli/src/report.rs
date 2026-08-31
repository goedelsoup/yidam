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
        // Not a subcommand gate like the others: `yidam vault` exists in every build, and
        // this says whether it can reach an `s3://` store. `store.rs` refuses such a url by
        // telling the reader that this list is where to look, so the list has to carry it.
        if cfg!(feature = "vault-s3") {
            features.push("vault-s3".to_string());
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
        matches!(self, Format::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Payload {
        answer: u8,
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
