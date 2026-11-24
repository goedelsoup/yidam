//! A corpus written by a program rather than by people (RFC-0064).
//!
//! `[corpus] mode = "projection"` in `.yidam/config.toml` says the tree under `.yidam/corpus/`
//! is overwritten by `generated_by` on every run. Four things key off it, and this module is
//! where each one asks:
//!
//! - [`corpus_id`] — the name the corpus is published under. An authored corpus is named by
//!   twelve characters of its genesis commit; a projection by the id its producer writes in
//!   `.yidam/corpus/corpus.toml`, because every site a producer projects shares one host
//!   repository and so one genesis (§2).
//! - [`refuse_if_projection`] — a command that writes the corpus, or commits on its behalf,
//!   stops and names the producer, since anything it wrote would be gone on the next run (§3).
//! - [`CorpusMode::generated_by`] — the region `.yidam/corpus` is generated, attributed to the
//!   producer, without an `authorship.yml` entry (§4).
//! - `doctor` skips the template-vendoring checks a projection never has (§5).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

/// The file a projection's producer writes its id into, relative to the corpus root.
pub const CORPUS_TOML: &str = ".yidam/corpus/corpus.toml";

/// Whether a corpus was written by people or projected by a program.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawCorpusConfig")]
pub enum CorpusMode {
    /// Every corpus before RFC-0064, and every corpus with no `[corpus]` table.
    #[default]
    Authored,
    /// Overwritten by `generated_by` on every run.
    Projection { generated_by: String },
}

impl CorpusMode {
    /// The producer, when this is a projection.
    pub fn generated_by(&self) -> Option<&str> {
        match self {
            Self::Authored => None,
            Self::Projection { generated_by } => Some(generated_by),
        }
    }

    /// The word a report prints: `authored` or `projection`.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Projection { .. } => "projection",
        }
    }
}

/// The `[corpus]` table as written. `deny_unknown_fields` for RFC-0063 §3.5's reason: a
/// mistyped key is otherwise read as absent, and an absent `mode` is a projection read as
/// authored without a word.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCorpusConfig {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    generated_by: Option<String>,
}

impl TryFrom<RawCorpusConfig> for CorpusMode {
    type Error = String;

    /// Both keys or neither (§1.1). `generated_by` beside an authored mode is refused rather
    /// than ignored: it would put a producer's name on findings for files people wrote.
    fn try_from(raw: RawCorpusConfig) -> std::result::Result<Self, String> {
        let generated_by = raw
            .generated_by
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        match (raw.mode.as_deref(), generated_by) {
            (None | Some("authored"), None) => Ok(Self::Authored),
            (None | Some("authored"), Some(by)) => Err(format!(
                "`[corpus] generated_by = \"{by}\"` names a producer, but `mode` is not \
                 \"projection\". Set `mode = \"projection\"`, or remove `generated_by`."
            )),
            (Some("projection"), Some(generated_by)) => Ok(Self::Projection { generated_by }),
            (Some("projection"), None) => Err(
                "`[corpus] mode = \"projection\"` needs `generated_by`, the command that writes \
                 the corpus: every refusal and every finding names it."
                    .to_string(),
            ),
            (Some(other), _) => Err(format!(
                "`[corpus] mode = \"{other}\"`: expected \"authored\" or \"projection\""
            )),
        }
    }
}

/// The mode `.yidam/config.toml` declares. A config that does not parse is an error here, as
/// it is for every other reader of that file.
pub fn mode(root: &Path) -> Result<CorpusMode> {
    Ok(crate::config::load_yidam_config(root)?.corpus)
}

/// The mode a config file's text declares, read from its `[corpus]` table alone.
///
/// For a config read out of a pinned commit: every other table is the shape that commit's
/// binary accepted, not this one's, so a key since renamed or added elsewhere must not stop a
/// dependency from being locked. The `[corpus]` table itself is still read strictly.
#[cfg(any(feature = "tonpa", test))]
pub fn parse_mode(text: &str) -> Result<CorpusMode> {
    let table: toml::Table = toml::from_str(text).context("parsing .yidam/config.toml")?;
    match table.get("corpus") {
        None => Ok(CorpusMode::Authored),
        Some(corpus) => corpus
            .clone()
            .try_into()
            .context("parsing the [corpus] table of .yidam/config.toml"),
    }
}

/// What `.yidam/corpus/corpus.toml` holds. Only `id`, and nothing else is accepted, so a
/// producer that misspells it is told rather than named after nothing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusToml {
    id: Option<String>,
}

/// The twelve lowercase hex characters a declared id must be, which is the length RFC-0033
/// already cuts a genesis hash to.
fn well_formed(id: &str) -> bool {
    id.len() == crate::s3vectors::request::CORPUS_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn remedy(generated_by: &str) -> String {
    format!(
        "`{generated_by}` writes it on every run: `id = \"<twelve lowercase hex characters>\"`, \
         derived from something stable the producer already names the corpus by"
    )
}

/// The id a projection's producer declared, refused when it is missing or malformed.
///
/// No fallback to the host's genesis: that fallback is the collision RFC-0064 removes, since
/// every site a producer projects into one repository would share it.
pub fn declared_id(root: &Path, generated_by: &str) -> Result<String> {
    let path = root.join(CORPUS_TOML);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_declared_id(&text, generated_by),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "this corpus is a projection and {CORPUS_TOML} is missing, so it has no name to be \
             published under.\n  {}.",
            remedy(generated_by)
        ),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// [`declared_id`] for text already read — `tonpa` reads the file out of a pinned commit
/// rather than a working tree.
pub fn parse_declared_id(text: &str, generated_by: &str) -> Result<String> {
    let remedy = remedy(generated_by);
    let parsed: CorpusToml =
        toml::from_str(text).with_context(|| format!("parsing {CORPUS_TOML}. {remedy}"))?;
    let Some(id) = parsed.id else {
        bail!("{CORPUS_TOML} has no `id`.\n  {remedy}.");
    };
    if !well_formed(&id) {
        bail!(
            "{CORPUS_TOML} declares `id = \"{id}\"`, which is not twelve lowercase hex \
             characters.\n  {remedy}."
        );
    }
    Ok(id)
}

/// The name a corpus is published under: twelve characters, in either mode (RFC-0064 §2.2).
///
/// `Ok(None)` only for an authored corpus with no genesis of its own — no commits, a shallow
/// clone, or a directory inside a larger repository — which every consumer already refuses by
/// name. A projection never answers `None`: its id is declared or the call fails.
pub fn corpus_id(root: &Path) -> Result<Option<String>> {
    match mode(root)? {
        CorpusMode::Authored => Ok(crate::git::genesis_hash(root)
            .as_deref()
            .map(crate::s3vectors::request::corpus_id)),
        CorpusMode::Projection { generated_by } => declared_id(root, &generated_by).map(Some),
    }
}

/// The directory a projection's producer overwrites, relative to the corpus root.
pub const CORPUS_DIR: &str = ".yidam/corpus";

/// Whether a repo-relative path is the corpus directory or under it.
pub fn under_corpus(rel: &str) -> bool {
    let rel = rel.trim_start_matches("./");
    rel == CORPUS_DIR
        || rel
            .strip_prefix(CORPUS_DIR)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// What a finding under a projection's corpus adds, so the reader is sent to the producer
/// rather than to the YAML (§4). The severity is not touched: a dangling edge a producer wrote
/// is still a defect, and only who can fix it changes.
pub fn attribution(generated_by: &str) -> String {
    format!("written by `{generated_by}`; fix it there and re-run the projection")
}

/// The producer, when the corpus at `root` is a projection and its config can be read.
///
/// For the reports that attribute findings: a config that does not parse is reported by every
/// command that needs it, so here it reads as authored rather than failing a report about
/// something else.
pub fn producer(root: &Path) -> Option<String> {
    mode(root).ok()?.generated_by().map(str::to_string)
}

/// Stop a command that writes the corpus, or commits on its behalf, in a projection (§3).
///
/// Called once the command has resolved its root and before it reads or writes the corpus.
/// `verb` is what the person typed after `yidam`.
pub fn refuse_if_projection(root: &Path, verb: &str) -> Result<()> {
    if let CorpusMode::Projection { generated_by } = mode(root)? {
        bail!(
            "This corpus is a projection written by `{generated_by}`. Change it there and \
             re-run the projection; what `yidam {verb}` wrote would be overwritten on the next \
             run."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(t: &str) -> std::result::Result<CorpusMode, String> {
        toml::from_str::<crate::config::YidamConfig>(t)
            .map(|c| c.corpus)
            .map_err(|e| e.to_string())
    }

    #[test]
    fn a_pinned_config_is_read_for_its_corpus_table_alone() {
        let text = "[cluster]\nkey_since_removed = 1\n[corpus]\nmode = \"projection\"\n\
                    generated_by = \"mirror\"\n";
        assert!(
            parse(text).is_err(),
            "today's YidamConfig refuses the stray [cluster] key"
        );
        assert_eq!(
            parse_mode(text).unwrap(),
            CorpusMode::Projection {
                generated_by: "mirror".into()
            }
        );
        assert_eq!(
            parse_mode("[other]\nx = 1\n").unwrap(),
            CorpusMode::Authored
        );
        assert!(parse_mode("[corpus]\nmode = \"projection\"\ntypo = 1\n").is_err());
    }

    #[test]
    fn absent_is_authored_and_both_keys_or_neither() {
        assert_eq!(parse("").unwrap(), CorpusMode::Authored);
        assert_eq!(
            parse("[corpus]\nmode = \"authored\"").unwrap(),
            CorpusMode::Authored
        );
        assert_eq!(
            parse("[corpus]\nmode = \"projection\"\ngenerated_by = \"mirror\"").unwrap(),
            CorpusMode::Projection {
                generated_by: "mirror".into()
            }
        );
        for (bad, says) in [
            ("[corpus]\nmode = \"projection\"", "needs `generated_by`"),
            ("[corpus]\ngenerated_by = \"mirror\"", "names a producer"),
            (
                "[corpus]\nmode = \"projected\"\ngenerated_by = \"m\"",
                "expected",
            ),
            (
                "[corpus]\nmode = \"projection\"\ngenerated_by = \"  \"",
                "needs",
            ),
            ("[corpus]\nmod = \"projection\"", "unknown field"),
        ] {
            let err = parse(bad).unwrap_err();
            assert!(err.contains(says), "{bad}: {err}");
        }
    }

    fn projection(id: Option<&str>) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam/corpus")).unwrap();
        std::fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[corpus]\nmode = \"projection\"\ngenerated_by = \"site mirror\"\n",
        )
        .unwrap();
        if let Some(body) = id {
            std::fs::write(tmp.path().join(CORPUS_TOML), body).unwrap();
        }
        tmp
    }

    #[test]
    fn a_projection_is_named_by_its_declared_id_or_refused() {
        let ok = projection(Some("id = \"3f9c2a71b0de\"\n"));
        assert_eq!(
            corpus_id(ok.path()).unwrap().as_deref(),
            Some("3f9c2a71b0de")
        );

        for (body, says) in [
            (None, "is missing"),
            (Some(""), "has no `id`"),
            (Some("id = \"3F9C2A71B0DE\"\n"), "not twelve lowercase hex"),
            (Some("id = \"3f9c2a71b0\"\n"), "not twelve lowercase hex"),
            (
                Some("id = \"3f9c2a71b0de\"\nname = \"x\"\n"),
                "unknown field",
            ),
        ] {
            let tmp = projection(body);
            let err = format!("{:#}", corpus_id(tmp.path()).unwrap_err());
            assert!(err.contains(says), "{body:?}: {err}");
            assert!(err.contains("site mirror"), "names the producer: {err}");
        }
    }

    #[test]
    fn the_corpus_directory_is_matched_by_component() {
        assert!(under_corpus(".yidam/corpus"));
        assert!(under_corpus(".yidam/corpus/site/a.yml"));
        assert!(under_corpus("./.yidam/corpus/site/a.yml"));
        assert!(!under_corpus(".yidam/corpus-old/a.yml"));
        assert!(!under_corpus(".yidam/exports/a.ttl"));
    }

    #[test]
    fn a_writer_is_refused_by_the_producer_s_name() {
        let tmp = projection(Some("id = \"3f9c2a71b0de\"\n"));
        let err = refuse_if_projection(tmp.path(), "propose")
            .unwrap_err()
            .to_string();
        assert!(err.contains("`site mirror`"), "{err}");
        assert!(err.contains("`yidam propose`"), "{err}");

        let authored = tempfile::tempdir().unwrap();
        refuse_if_projection(authored.path(), "propose").unwrap();
    }
}
