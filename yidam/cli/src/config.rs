use anyhow::{Context, Result};
use serde::Deserialize;
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
}

#[derive(Debug, Default, Deserialize)]
pub struct IndexConfig {
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    pub model: Option<String>,
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
