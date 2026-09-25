use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct YidamConfig {
    /// Read only by `index-build`, which the light `reports` binary does not carry. The
    /// field is still parsed there so a config naming a model is not rejected by a binary
    /// that simply cannot act on it.
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub propose: ProposeConfig,
    #[serde(default)]
    pub catalog: CatalogConfig,
    #[serde(default)]
    pub due: DueConfig,
    #[serde(default)]
    pub object: ObjectConfig,
    #[serde(default)]
    pub serve: ServeConfig,
    /// The stores this corpus keeps artifacts in, by name.
    ///
    /// Plural before it needs to be, and the reasoning is
    /// [`crate::vault::resolve`]'s: `[vault]` and `[vault.default]` are different config
    /// shapes, this file is committed, and a corpus that wrote the singular form is one a
    /// later release breaks. Exactly one entry is honoured today and a second is refused
    /// rather than resolved to the first.
    ///
    /// Never a credential. Those come from the environment, because this file is committed
    /// and a repository has already been found carrying an untracked `.env` that its own
    /// prescribed `git add -A` would have staged.
    #[serde(default)]
    pub vault: BTreeMap<String, crate::vault::VaultConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct IndexConfig {
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    pub model: Option<String>,
    /// The vector index this corpus publishes to and can be queried out of (RFC-0033).
    ///
    /// **Not feature-gated, and for the reason the `[lint]` note above gives about its own
    /// field**: the section is a corpus's declaration about itself, it lives in a committed
    /// file, and a build that cannot act on it should still be able to read it and say so.
    /// `doctor` reports a configured remote index in every build; only querying one needs
    /// `vector-read`.
    #[serde(default)]
    pub remote: Option<crate::s3vectors::RemoteIndexConfig>,
}

/// What this corpus has decided about its own gate.
#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// Corpus-touching commits a dated finding may hold before it escalates to an error.
    ///
    /// Absent means no finding ever escalates, and that is the right default rather than a
    /// timid one. The number is a judgement about how fast *this* corpus is meant to
    /// consume what it collects — a breadth sweep landing twelve nodes it will link over
    /// the next eighty commits is healthy in one repository and over-collection in another
    /// — so a value compiled into the binary would be one corpus's answer imposed on every
    /// other, arriving as a build failure in a repository that never agreed to it.
    ///
    /// Declared here so the argument for the number lives in the repository that has to
    /// live with it:
    ///
    /// ```toml
    /// [lint]
    /// escalate_after = 100
    /// ```
    pub escalate_after: Option<usize>,
}

/// What this corpus has licensed `yidam propose` to draft.
///
/// Empty by default, and the default is the design rather than caution. `propose` drafts a
/// question from any finding, because recording a question asserts nothing the finding did
/// not already assert. Drafting a *deletion* asserts that the node should go, and no finding
/// says that — so it is licensed only by a corpus that says so here, about itself.
#[derive(Debug, Default, Deserialize)]
pub struct ProposeConfig {
    /// Corpus-touching commits an uncited node may hold before `propose` drafts its
    /// withdrawal.
    ///
    /// Absent means no withdrawal is ever drafted, which is every corpus until someone turns
    /// it on. The reasoning is [`LintConfig::escalate_after`]'s and is not repeated: a number
    /// compiled into the binary would be one repository's judgement arriving as a proposed
    /// deletion in another that never agreed to it.
    ///
    /// ```toml
    /// [propose]
    /// withdraw_uncited_after = 400
    /// ```
    ///
    /// **Not `escalate_after` under another name.** That declares when a finding becomes a
    /// build failure, which is a statement about the gate. This declares when an uncited node
    /// stops being a sweep in progress and becomes over-collection, which is a statement
    /// about the corpus. A repository may reasonably hold the first and not the second, and
    /// most will: failing the build asks a person to look, and deleting the node decides what
    /// they would have concluded.
    pub withdraw_uncited_after: Option<usize>,
}

/// What this corpus has decided about how its sources age.
///
/// Not `.yidam.toml`. That file is the *template provenance pin* — `origin`, `commit`,
/// `template`, `committed` — and records which yidam governs a corpus. This one records what
/// the corpus decided about itself, which is where `escalate_after` and
/// `withdraw_uncited_after` already live.
#[derive(Debug, Default, Deserialize)]
pub struct CatalogConfig {
    /// Days a catalog entry may stand before it is worth looking at again, when the entry
    /// does not declare its own.
    ///
    /// A default rather than the mechanism: the per-entry `ttl_days:` is the primary form,
    /// because a gauge record and a statute do not age at the same rate. This exists for the
    /// common case of a corpus whose sources mostly do age alike, so that adopting a TTL is
    /// one line rather than one line per entry.
    ///
    /// Absent means **no entry expires unless it says so itself**, which is every corpus
    /// until someone turns it on. The reasoning is [`LintConfig::escalate_after`]'s and is
    /// not repeated.
    ///
    /// ```toml
    /// [catalog]
    /// ttl_days = 180
    /// ```
    pub ttl_days: Option<u32>,
}

/// When this corpus considers each of its clocks due.
///
/// Read by `yidam due`, and by nothing else. The keys here are the intervals the clocks it
/// reads had none of; the fourth interval it reads is [`CatalogConfig::ttl_days`] and is
/// deliberately **not** repeated here. A source's TTL is a statement about the source and
/// belongs where a source is configured — restating it under `[due]` would create two places
/// to change it and one of them would be wrong.
///
/// Every key absent is the default, and it means `yidam due` reports what it measured and
/// calls nothing due. That is not a degraded mode: a clock with no interval is a number
/// nobody has yet decided the meaning of, and inventing one in the binary would be the
/// failure [`LintConfig::escalate_after`] describes at greater length.
#[derive(Debug, Default, Deserialize)]
pub struct DueConfig {
    /// Corpus-touching commits an open question may stand before it is due a look.
    ///
    /// Commits, not days, and the reasoning is `history::Age`'s: how long a question has gone
    /// unanswered is a fact about the repository, and the repository's clock is `HEAD`. A
    /// corpus that has not committed has not ignored anything.
    ///
    /// ```toml
    /// [due]
    /// questions_after = 100
    /// ```
    pub questions_after: Option<usize>,
    /// Days a bounded inquiry ref may be in flight before it is due a look.
    ///
    /// **Days, and this is the second clock that counts them.** A phase is work somebody is
    /// doing in the world, and it does not stop having been open for four months because
    /// nobody committed to the corpus. That is the same argument
    /// [`crate::cmd::lint::ttl`] makes for a source's TTL, applied to the other quantity
    /// here that is not a fact about the repository.
    ///
    /// ```toml
    /// [due]
    /// phases_after = 60
    /// ```
    pub phases_after: Option<u32>,
    /// Corpus files that may change after the index was built before a rebuild is due.
    ///
    /// `1` means any change at all makes it due, which is what a repository that keeps
    /// semantic search sharp will want. A larger number is a corpus saying it is content for
    /// retrieval to lag its own edits by that much.
    ///
    /// ```toml
    /// [due]
    /// index_after = 25
    /// ```
    pub index_after: Option<usize>,
    /// Clocks this corpus decided it does not want, each naming the record that argues it.
    ///
    /// **Unset and declined are different states, and only one of them was a choice.** A
    /// corpus with no `index_after` and a corpus that examined a vector index and did not
    /// choose it report identically without this key, and the only remedy `due` offers the
    /// second is to declare an interval for work nobody intends — a clock that is
    /// permanently due, which is a clock a reader learns to skip.
    ///
    /// The key is the clock's `id` — `index`, `catalog`, `questions`, `phases` — and the
    /// value is a decision record in `.yidam/decisions/`, by its `id:` or its file stem.
    /// **The record is required**, and that is the whole of what makes this a declaration
    /// rather than a mute button: a decline naming a record this repository does not hold
    /// is not honoured, and `due` says so. It is `.yidam/lint-baseline.yml`'s property —
    /// an accepted finding and an unnoticed one are different objects, and a decline that
    /// no longer has anything behind it goes red rather than quiet.
    ///
    /// ```toml
    /// [due.declined]
    /// index = "due-clocks"
    /// ```
    #[serde(default)]
    pub declined: BTreeMap<String, String>,
}

/// Where the artifact this corpus is about lives, in this repository.
///
/// **The paths are here and not in the kuten, and that is settled** — RFC-0028 §4, Erratum 5.
/// A kuten is an upstream-authored profile vendored unchanged, `inquiry` is one profile
/// serving six repositories with six object shapes, and the only thing a corpus writes about
/// it is `{kuten, revision}`. There is no channel by which a corpus supplies paths to a
/// profile, and paths are a fact about a repository rather than about a practice. The kuten
/// declares the *direction* of the arrow; this declares where the arrow points. It is the
/// `clocks` precedent: the kuten proposes values and never holds live ones.
///
/// Absent means the repository has **one** register and the corpus vocabulary governs every
/// commit, which is what every repository did before this key existed.
#[derive(Debug, Default, Deserialize)]
pub struct ObjectConfig {
    /// Globs naming the artifact register, relative to the repository root.
    ///
    /// `**` spans any number of path segments and `*` any run within one. A glob also claims
    /// everything beneath what it names, so `"web"` and `"web/**"` say the same thing.
    ///
    /// Read by `lint --commits`, which declines to report a commit touching **only** these
    /// paths against the corpus vocabulary. `feat:` on the artifact is not a corpus-vocabulary
    /// violation; measured across the population, 40 off-vocabulary commits in one derived
    /// repository and 75 in another touch no corpus file at all.
    ///
    /// A commit touching both registers is governed by the corpus register, and a commit
    /// listing no paths — the authored merge — is too.
    ///
    /// ```toml
    /// [object]
    /// paths = ["web/**", "crates/**", "package.json"]
    /// ```
    #[serde(default)]
    pub paths: Vec<String>,
}

/// What this repository permits a server it starts to do — RFC-0029.
///
/// # Why a repository's own file and not a flag
///
/// RFC-0029 §2.1 decides that an `act` declaration is **configuration, never inference**: a
/// server that satisfies every condition for writing and was not told to write declares
/// false. A flag would put the declaration in the argv of whatever launcher happened to spawn
/// the process — an agent's client config, a desktop app's manifest, a shell alias — so the
/// answer to *may this corpus be written to by a tool* would be a property of who started the
/// server rather than of the corpus. This file is committed, and the repository is the thing
/// that has to live with what a tool wrote into it.
///
/// RFC-0029's open question offered `.yidam/capabilities.toml` as a third option. **It does
/// not exist** — the name appears in RFC-0026 and RFC-0028 prose and nothing reads it — so the
/// choice was this key or a flag.
#[derive(Debug, Default, Deserialize)]
pub struct ServeConfig {
    /// Whether a server started against this corpus may declare the `act` capability.
    ///
    /// **`false` is the default and a server that satisfies every other condition still
    /// declares false without this key.** That is §2.1's rule and it is the whole difference
    /// between the write tier and every read tier: a read tier's absence says the server
    /// *cannot*, and is therefore discovered; this says the deployment *will not*, and a
    /// policy that a server can discover about itself is not a policy.
    ///
    /// Declaring it true is not sufficient either. RFC-0029 §2.2's clause 1 requires that a
    /// git author identity resolve in the corpus being served — `user.name` and `user.email`,
    /// the values the commit would actually take — and clause 3 requires every listening
    /// socket to be loopback. Both are checked at startup, and both fail the server rather
    /// than quietly downgrading it: a server that was told to write and serves reads instead
    /// is a deployment that believes something false about itself.
    ///
    /// ```toml
    /// [serve]
    /// act = true
    /// ```
    #[serde(default)]
    pub act: bool,
}

pub fn load_yidam_config(root: &Path) -> Result<YidamConfig> {
    let path = root.join(".yidam").join("config.toml");
    if !path.exists() {
        return Ok(YidamConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sadhana/config.toml`, the scaffold a derived repository gets, deserializes into
    /// [`YidamConfig`] once every offered line is uncommented.
    ///
    /// The file ships with every key commented out, because an interval compiled into a
    /// scaffold is one corpus's judgement arriving in another that never agreed to it. That
    /// is the right delivered state and it is also why nothing checks the file: a comment
    /// has no parser, so a key in the wrong section, a misspelling, or a string where a
    /// number belongs sits there until the day a corpus takes the offer up.
    ///
    /// This is the typed half of that check — right section, right spelling, right type,
    /// through the binary's own deserializer, which is why it lives here and not in
    /// `tests/scaffolded_config.rs`: `mod config` is private. `VaultConfig` and
    /// `RemoteIndexConfig` are `deny_unknown_fields`, so a leaf typo fails here rather than
    /// parsing into nothing.
    #[test]
    fn the_scaffolded_config_deserializes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("sadhana/config.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

        // As delivered: no live keys at all, so a derived repository's config parses to
        // exactly what an absent file gives it.
        let delivered: YidamConfig = toml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} does not parse as delivered: {e}", path.display()));
        assert!(
            delivered.vault.is_empty()
                && delivered.due.questions_after.is_none()
                && delivered.lint.escalate_after.is_none()
                && !delivered.serve.act,
            "the scaffolded config sets a key. Every one of them is meant to be commented \
             out: a number shipped here is a judgement this corpus never made."
        );

        // And as taken up.
        let live: String = text
            .lines()
            .filter_map(|line| {
                let bare = line
                    .strip_prefix('#')
                    .map(str::trim_start)
                    .unwrap_or(line)
                    .trim();
                let header = bare.starts_with('[') && bare.ends_with(']') && !bare.contains(' ');
                let assignment = bare.split_once('=').is_some_and(|(k, _)| {
                    let k = k.trim();
                    !k.is_empty()
                        && k.chars().all(|c| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                        })
                });
                (header || assignment).then(|| bare.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");

        let config: YidamConfig = toml::from_str(&live).unwrap_or_else(|e| {
            panic!(
                "{} does not deserialize once uncommented: {e}\n\n--- uncommented ---\n{live}",
                path.display()
            )
        });

        // Spot-check the ends of the shape, so a parse that silently read nothing is not
        // mistaken for one that read everything.
        assert_eq!(config.due.questions_after, Some(100));
        assert_eq!(config.vault.len(), 1, "the example vault did not survive");
        let vault = config.vault.values().next().unwrap();
        assert!(
            vault.audience.is_some(),
            "the example vault states no audience"
        );
        let remote = config
            .index
            .remote
            .as_ref()
            .expect("the example remote index did not survive");
        assert!(
            !remote.corpora.is_empty(),
            "the example `[index.remote.corpora]` nickname did not survive — a key with a \
             hyphen in it is how that reads, and a parse that drops it would pass every \
             other assertion here"
        );
    }
}
