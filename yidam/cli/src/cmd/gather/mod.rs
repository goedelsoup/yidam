//! `yidam gather` — ask pinned peers one question, and land what they said as a question (#476).
//!
//! `tonpa` pins a corpus and `query --across` can read one, but nothing could *ask* a peer
//! anything: `--across` matches classes by name, which is an alignment nobody authored, and it
//! returns rows rather than landing anything a corpus keeps. A gather is the other half.
//!
//! # What it does
//!
//! A person commits `.yidam/gathers/<name>.toml`: a question in words, an RFC-0018 query in
//! **this corpus's own classes**, the property whose value is the answer, and — per peer — a
//! correspondence saying which of the peer's classes and properties mean the same thing. The
//! gather translates the query through each correspondence, runs it once per peer through the
//! same check and executor `query --across` uses, and lands one `? …` question node (RFC-0037)
//! citing every answer verbatim (RFC-0019), on a `propose/*` branch.
//!
//! # What it refuses to do
//!
//! **Nothing is imported and no claim is merged.** A peer's answer is a `cites:` entry — a
//! pointer with a quoted span — and never a `links:` edge, a copied node, or a tag. Where two
//! peers give different answers for the same key, the gather does not pick one: it lands a
//! second question citing both, because deciding between them is the resolution event
//! Article V confines to a sangha.
//!
//! **Alignment is authored, never inferred.** A class or property the correspondence does not
//! name is not guessed at by spelling; the peer is reported `unaligned` and not asked.
//!
//! **Every peer is reported.** A peer that answered nothing, could not be aligned, was
//! refused, is declared but not installed, or is installed but not named — each has an
//! outcome in the report and in the landed node's description. A gather that silently asked
//! fewer peers than it names would read as a consensus it does not have.
//!
//! **A path dependency is refused.** It is a working tree, not a pinned commit, so a citation
//! into it could not say what it quoted.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::cmd::lint::citations;
use crate::cmd::propose::write::{self, TempIndex};
use crate::cmd::query::{check, exec, lang, Foreign, Graph};
use crate::cmd::run::manifest::{Capability, Kind, Run};
use crate::cmd::run::receipt::{self, File, Input, Receipt};
use crate::deps::DependencyKind;
use crate::paths::{repo_root, require_yidam_repo};

#[cfg(test)]
mod tests;

/// The author every gathered commit carries — the tool drafted it; whoever ran it commits.
const AUTHOR_NAME: &str = "yidam gather";
const AUTHOR_EMAIL: &str = "gather@yidam";

pub struct Options {
    /// The gather to run: `.yidam/gathers/<name>.toml`.
    pub name: String,
    /// Ask every peer and report; write no branch.
    pub dry_run: bool,
    /// Replace an existing gather branch at this HEAD whose content differs.
    pub force: bool,
    pub format: crate::report::Format,
}

// ── the spec ──────────────────────────────────────────────────────────────────

/// `.yidam/gathers/<name>.toml`, as a person wrote it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The question in words. The landed node's label is this, prefixed `? `.
    pub question: String,
    /// One step of RFC-0018, in this corpus's classes. No hop and no anchor: a hop would need
    /// a relationship correspondence nobody has authored, and an anchor ranks by a model the
    /// peer did not embed with.
    pub query: String,
    /// The local property whose value is the answer. Its peer counterpart's value is the span.
    pub answer: String,
    /// The local property two answers must share to be about the same thing. Without it, no
    /// disagreement can be detected, and none is claimed.
    #[serde(default)]
    pub key: Option<String>,
    /// The local class the question node is written as. Must be declared here.
    pub lands_as: String,
    /// One correspondence per peer, keyed by the dependency name `tonpa.toml` uses.
    #[serde(default)]
    pub peers: BTreeMap<String, Correspondence>,
    /// The sha256 of the file as committed, which every peer's receipt records.
    #[serde(skip)]
    pub digest: String,
}

/// Which of a peer's classes and properties mean the same as this corpus's. Local name on the
/// left, the peer's on the right; a name missing here is a name this gather cannot ask about.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correspondence {
    #[serde(default)]
    pub classes: BTreeMap<String, String>,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
}

pub fn gathers_dir(root: &Path) -> std::path::PathBuf {
    root.join(".yidam").join("gathers")
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
}

/// Read and validate a gather against the local corpus.
///
/// Everything that can be wrong with the *spec* is refused here, before any peer is read, so
/// a peer outcome is always about the peer.
pub fn load(root: &Path, name: &str, local: &Graph) -> Result<(Spec, lang::Query)> {
    if !valid_name(name) {
        bail!("`{name}` is not a gather name — lowercase letters, digits and `-` only");
    }
    let path = gathers_dir(root).join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no gather named `{name}` — expected {}", path.display()))?;
    let mut spec: Spec =
        toml::from_str(&text).with_context(|| format!("{} does not parse", path.display()))?;
    spec.digest = receipt::sha256(text.as_bytes());

    let question = spec.question.trim();
    if question.is_empty() {
        bail!("{}: `question` is empty", path.display());
    }
    if question.starts_with('?') {
        bail!(
            "{}: write `question` without the leading `?` — the gather adds it",
            path.display()
        );
    }
    if spec.peers.is_empty() {
        bail!(
            "{}: names no `[peers.<name>]` — there is nobody to ask",
            path.display()
        );
    }

    let query = lang::parse(&spec.query)
        .map_err(|e| anyhow::anyhow!("{}: `query` does not parse: {e}", path.display()))?;
    if query.steps.len() != 1 || !query.hops.is_empty() {
        bail!(
            "{}: a gather's query is one step — a hop would need a relationship \
             correspondence, and none is authored",
            path.display()
        );
    }
    let step = &query.steps[0];
    if step.anchor.is_some() {
        bail!(
            "{}: a gather's query takes no `~\"…\"` anchor — similarity ranks by an \
             embedding the peer did not compute",
            path.display()
        );
    }
    if step.class == "*" {
        bail!(
            "{}: a gather's query names one class — `*` has no single correspondence",
            path.display()
        );
    }

    if local.classes.is_empty() {
        bail!("this corpus declares no classes, so a gather has nothing to write its question in");
    }
    let schema = check::Schema {
        classes: &local.classes,
        universal: &local.universal,
        authored: exec::authored(&local.nodes),
    };
    if let Err(r) = check::check(&query, &schema) {
        bail!(
            "{}: `query` is not a question this corpus can ask: {}",
            path.display(),
            r.message
        );
    }
    let class = local.classes.iter().find(|c| c.name == step.class);
    for (field, prop) in
        std::iter::once(("answer", &spec.answer)).chain(spec.key.iter().map(|k| ("key", k)))
    {
        if local.universal.declared_type_for(class, prop).is_none() {
            bail!(
                "{}: `{field} = \"{prop}\"` is not a property `{}` declares",
                path.display(),
                step.class
            );
        }
    }
    if !local.classes.iter().any(|c| c.name == spec.lands_as) {
        bail!(
            "{}: `lands_as = \"{}\"` is not a class this corpus declares",
            path.display(),
            spec.lands_as
        );
    }
    Ok((spec, query))
}

// ── the report ────────────────────────────────────────────────────────────────

/// What happened when a peer was asked — or why it was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Asked, and at least one answer was cited.
    Answered,
    /// Asked, and nothing matched — or nothing that matched could be quoted.
    Empty,
    /// Not asked: the correspondence does not reach a class or property the query needs, or
    /// the peer's own schema rejected the translated query.
    Unaligned,
    /// Not asked: a path dependency, a bundle with no commit, or one that is not what
    /// `tonpa.lock` pins.
    Refused,
    /// Named by the gather, not installed.
    Missing,
    /// Installed, not named by the gather, so not asked.
    Undeclared,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::Empty => "empty",
            Self::Unaligned => "unaligned",
            Self::Refused => "refused",
            Self::Missing => "missing",
            Self::Undeclared => "undeclared",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReport {
    pub package: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The bundle's `commit`, which every citation into this peer carries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// The query as the peer was asked it, after translation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    pub matched: usize,
    pub cited: usize,
    /// Matched nodes whose answer could not be quoted verbatim, and so were not cited.
    pub unquotable: Vec<String>,
}

impl PeerReport {
    fn not_asked(package: &str, outcome: Outcome, reason: String) -> Self {
        Self {
            package: package.to_string(),
            outcome,
            reason: Some(reason),
            pin: None,
            asked: None,
            matched: 0,
            cited: 0,
            unquotable: vec![],
        }
    }
}

/// One `cites:` entry, in RFC-0019's shape. No `tag`: a peer's standing is recorded by the
/// peer, and the weakest-claim rule cannot be computed across the boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cite {
    pub package: String,
    pub node: String,
    pub commit: String,
    pub span: String,
}

/// An answer with the key it was given under, for disagreement detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub cite: Cite,
    pub value: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Disagreement {
    pub key: String,
    /// Every cited answer given under this key, across the peers that disagree.
    pub answers: Vec<Cite>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Landed {
    pub branch: String,
    /// Short shas, oldest first. Empty when `unchanged`.
    pub commits: Vec<String>,
    /// Repo-relative paths of the question nodes.
    pub nodes: Vec<String>,
    /// Repo-relative paths of the receipts, one per peer that answered or said nothing (#1217).
    pub receipts: Vec<String>,
    /// The branch already held exactly this — or HEAD does — so nothing was written.
    pub unchanged: bool,
}

#[derive(Debug, Serialize)]
pub struct GatherReport {
    /// Always `gather`.
    pub kind: &'static str,
    pub gather: String,
    pub question: String,
    pub query: String,
    pub peers: Vec<PeerReport>,
    pub cites: Vec<Cite>,
    pub disagreements: Vec<Disagreement>,
    /// `null` on `--dry-run`, and when no peer answered.
    pub landed: Option<Landed>,
    pub dry_run: bool,
}

// ── asking ────────────────────────────────────────────────────────────────────

/// A quote of `value` from the node's own bytes, or `None` when it is not there verbatim.
///
/// `peer_prop: value` first, because it says what the number is; the bare value when YAML
/// quoting or folding kept that form out of the text. The test is `citations::flatten`'s —
/// the one `external-citation-span-drift` applies — so a span chosen here is one the gate
/// will hold to.
fn quote(text: &str, peer_prop: &str, value: &str) -> Option<String> {
    let flat = citations::flatten(text);
    let labelled = format!("{peer_prop}: {value}");
    [labelled, value.to_string()]
        .into_iter()
        .find(|s| !citations::flatten(s).is_empty() && flat.contains(&citations::flatten(s)))
}

/// Translate the local query into one the peer can answer, or say what is missing.
fn translate(
    query: &lang::Query,
    corr: &Correspondence,
    spec: &Spec,
) -> std::result::Result<(lang::Query, String, String, Option<String>), String> {
    let mut q = query.clone();
    let step = &mut q.steps[0];
    let Some(class) = corr.classes.get(&step.class) else {
        return Err(format!("no class corresponds to `{}`", step.class));
    };
    step.class = class.clone();
    let map = |local: &str| {
        corr.properties
            .get(local)
            .cloned()
            .ok_or_else(|| format!("no property corresponds to `{local}`"))
    };
    for pred in &mut step.filter {
        pred.prop = map(&pred.prop)?;
    }
    let answer = map(&spec.answer)?;
    let key = spec.key.as_deref().map(map).transpose()?;
    Ok((q, class.clone(), answer, key))
}

/// The query as text, for the report — rebuilt rather than rewritten, so what is shown is what
/// ran.
pub(crate) fn show(q: &lang::Query) -> String {
    let step = &q.steps[0];
    let preds: Vec<String> = step.filter.iter().map(lang::Pred::spelled).collect();
    match preds.is_empty() {
        true => step.class.clone(),
        false => format!("{}[{}]", step.class, preds.join(",")),
    }
}

/// Ask one installed, pinned peer the query `q`, already in its own words.
#[allow(clippy::too_many_arguments)]
fn ask(
    foreign: &Foreign,
    pin: &str,
    q: &lang::Query,
    class: &str,
    answer: &str,
    key: Option<&str>,
    deps: &BTreeMap<String, citations::Installed>,
) -> (PeerReport, Vec<Answer>) {
    let package = foreign.package.as_str();
    let mut report = PeerReport {
        pin: Some(pin.to_string()),
        asked: Some(show(q)),
        ..PeerReport::not_asked(package, Outcome::Unaligned, String::new())
    };
    let declared = foreign.classes.iter().find(|c| c.name == class);
    if declared.is_none() {
        report.reason = Some(format!("`{package}` declares no class `{class}`"));
        return (report, vec![]);
    }
    for prop in std::iter::once(answer).chain(key) {
        if foreign
            .universal
            .declared_type_for(declared, prop)
            .is_none()
        {
            report.reason = Some(format!(
                "`{package}`'s `{class}` declares no property `{prop}`"
            ));
            return (report, vec![]);
        }
    }
    let schema = check::Schema {
        classes: &foreign.classes,
        universal: &foreign.universal,
        authored: exec::authored(&foreign.nodes),
    };
    let checked = match check::check(q, &schema) {
        Ok(c) => c,
        Err(r) => {
            report.reason = Some(format!(
                "`{package}` rejects the translated query: {}",
                r.message
            ));
            return (report, vec![]);
        }
    };
    let outcome = exec::execute(
        q,
        &checked,
        &foreign.nodes,
        &foreign.edges,
        &foreign.corpus_dir,
        None,
    );
    report.matched = outcome.matched.len();

    let by_id: BTreeMap<String, &crate::corpus::Node> = foreign
        .nodes
        .iter()
        .map(|n| (exec::id_of(n, &foreign.corpus_dir), n))
        .collect();
    let mut answers = Vec::new();
    for id in &outcome.matched {
        let Some(node) = by_id.get(id) else { continue };
        let node_ref = id.strip_suffix(".yml").unwrap_or(id).to_string();
        let keys = key.map(|k| exec::node_scalars(node, k)).unwrap_or_default();
        let mut quoted = false;
        for value in exec::node_scalars(node, answer) {
            let Some(span) = quote(&node.text, answer, &value) else {
                continue;
            };
            let cite = Cite {
                package: package.to_string(),
                node: node_ref.clone(),
                commit: pin.to_string(),
                span,
            };
            // The gate's own verdict, asked before writing. A citation it would fault is not
            // landed — failing closed costs a report line; landing it costs a red gate.
            let ext = crate::parse::ExternalCitation {
                package: Some(cite.package.clone()),
                node: Some(cite.node.clone()),
                commit: Some(cite.commit.clone()),
                tag: None,
                span: Some(cite.span.clone()),
            };
            if !citations::findings(&ext, deps).is_empty() {
                continue;
            }
            quoted = true;
            answers.push(Answer {
                cite,
                value,
                keys: keys.clone(),
            });
        }
        if !quoted {
            report.unquotable.push(node_ref);
        }
    }
    report.cited = answers.len();
    report.outcome = match answers.is_empty() {
        true => Outcome::Empty,
        false => Outcome::Answered,
    };
    report.reason = match (report.outcome, report.matched) {
        (Outcome::Empty, 0) => Some("nothing matched".into()),
        (Outcome::Empty, n) => Some(format!("{n} matched and none could be quoted verbatim")),
        _ => None,
    };
    (report, answers)
}

/// Where peers answered the same key differently.
///
/// Per peer, the *set* of values given under a key: a peer that says one thing twice agrees
/// with itself, and two peers disagree when their sets differ. Only peers are compared — a
/// peer inconsistent with itself is that peer's question, not this corpus's.
fn disagreements(answers: &[Answer]) -> Vec<Disagreement> {
    let mut by_key: BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> = BTreeMap::new();
    for a in answers {
        for k in &a.keys {
            by_key
                .entry(k.as_str())
                .or_default()
                .entry(a.cite.package.as_str())
                .or_default()
                .insert(a.value.as_str());
        }
    }
    let mut out = Vec::new();
    for (key, peers) in by_key {
        let distinct: BTreeSet<&BTreeSet<&str>> = peers.values().collect();
        if peers.len() < 2 || distinct.len() < 2 {
            continue;
        }
        let mut cites: Vec<Cite> = answers
            .iter()
            .filter(|a| a.keys.iter().any(|k| k == key))
            .map(|a| a.cite.clone())
            .collect();
        cites.sort();
        cites.dedup();
        out.push(Disagreement {
            key: key.to_string(),
            answers: cites,
        });
    }
    out
}

// ── the plan, the asking, and the roll call ─────────────────────────────────────
//
// A gather is three phases, and they are three functions because a cluster runs them in three
// kinds of pod (#1217). The gathering side *plans*: it decides who is asked and translates the
// query, so the correspondence never leaves the one place that knows both vocabularies. Each
// peer's asker *answers* from the peer's own bytes, in the peer's own words. The gathering side
// then *settles*: every plan entry gets exactly one outcome, whether or not its asker came back.
// On one machine the three run in one process, and the result is the same tree.

/// Where the gathering side learns which peers exist and what they are pinned at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The bundles `tonpa install` unpacked under `.yidam/tonpa/`, pinned by their manifests.
    Local,
    /// `tonpa.lock`, pinned by its `commit`. A cluster's pin is a git bundle of the corpus,
    /// which carries the lock and not what an install unpacked beside it; each asker fetches
    /// its peer by the lock entry.
    Cluster,
}

/// The lock entry an asker fetches its peer by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locked {
    pub url: String,
    /// The `.yiz`'s sha256, which is also the key a vault stores those bytes under.
    pub sha256: String,
}

/// One peer the gathering side decided to ask, and everything its asker needs.
///
/// The query is **already translated**. An asker is handed the peer's own class and property
/// names and never the correspondence, so there is one place that maps one vocabulary onto the
/// other, and it is `.yidam/gathers/<name>.toml` on the gathering side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub gather: String,
    pub peer: String,
    /// The commit every citation into this peer carries.
    pub pin: String,
    /// Where the peer's bundle can be fetched, and what it must hash to. `None` for a peer
    /// installed without a lock entry, which only a local gather asks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<Locked>,
    /// RFC-0018, in the peer's classes and properties.
    pub query: String,
    pub class: String,
    pub answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// The version of [`Asked`]. An asker and its gatherer run one image, so a mismatch is a
/// workflow assembled from two releases, and the peer is refused rather than half-read.
pub const ASKED_VERSION: u32 = 1;

/// What asking one peer produced: the record an asker hands back to the gathering side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asked {
    pub format_version: u32,
    /// The ask this answers, whole. The gathering side holds it to the plan it derives itself,
    /// so a record answering some other question is refused and not cited.
    pub ask: Ask,
    pub report: PeerReport,
    pub answers: Vec<Answer>,
    /// The sha256 of the `.yiz` the answer was read from, where there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_sha256: Option<String>,
    /// The version of the binary that answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The image it ran in, where the asker's pod was started from a digest reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_digest: Option<String>,
}

impl Asked {
    /// A peer that was not asked after all. It keeps its place in the roll call, with the reason.
    pub(crate) fn refused(ask: &Ask, reason: String, image_digest: Option<String>) -> Self {
        Self {
            format_version: ASKED_VERSION,
            ask: ask.clone(),
            report: PeerReport::not_asked(&ask.peer, Outcome::Refused, reason),
            answers: vec![],
            bundle_sha256: None,
            version: receipt::this_version(),
            image_digest,
        }
    }
}

const PATH_DEPENDENCY: &str = "a path dependency is a working tree, not a pinned commit — a \
                               citation into it could not say what it quoted; install it as a \
                               fetched bundle";
const NOT_NAMED: &str = "installed, and the gather names no correspondence for it — not asked";
const NOT_INSTALLED: &str = "named by the gather and not installed — declare it in \
                             .yidam/tonpa.toml and run `yidam tonpa install`";
const NO_MANIFEST_COMMIT: &str =
    "its manifest.yml records no commit, so a citation could not be pinned";
const NO_LOCKED_COMMIT: &str =
    "tonpa.lock records no commit for it, so a citation could not be pinned";
const NOT_THE_LOCKED_BUNDLE: &str =
    "the unpacked bundle is not the one tonpa.lock pins — run `yidam tonpa install`";

/// Every peer's name, and for each either an [`Ask`] or the outcome that means it is not asked.
///
/// Names come from the gather, from what is installed (or locked, on a cluster), and from the
/// path dependencies. Refused, undeclared, missing and unaligned are all decided here, before
/// any peer's bytes are read — so the same pin plans the same roll call wherever it is planned.
pub(crate) fn plan(
    root: &Path,
    name: &str,
    spec: &Spec,
    query: &lang::Query,
    mode: Mode,
) -> (Vec<PeerReport>, Vec<Ask>) {
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    let deps = citations::installed(root);
    let lock = crate::deps::load_lock(&crate::paths::tonpa_lock_path(root)).unwrap_or_default();
    let locked = |n: &str| lock.packages.iter().find(|p| p.name == n);

    let mut names: BTreeSet<String> = spec.peers.keys().cloned().collect();
    match mode {
        Mode::Local => names.extend(deps.keys().cloned()),
        Mode::Cluster => names.extend(lock.packages.iter().map(|p| p.name.clone())),
    }
    names.extend(
        config
            .dependencies
            .iter()
            .filter(|(_, d)| d.path.is_some())
            .map(|(n, _)| n.clone()),
    );

    let mut peers = Vec::new();
    let mut asks = Vec::new();
    for peer in names {
        let is_path = config
            .dependencies
            .get(&peer)
            .is_some_and(|d| d.path.is_some())
            || deps
                .get(&peer)
                .is_some_and(|d| d.kind == DependencyKind::Path);
        let not = |outcome, why: &str| PeerReport::not_asked(&peer, outcome, why.to_string());
        if is_path {
            peers.push(not(Outcome::Refused, PATH_DEPENDENCY));
            continue;
        }
        if !spec.peers.contains_key(&peer) {
            peers.push(not(Outcome::Undeclared, NOT_NAMED));
            continue;
        }
        // `None`: not there. `Some(None)`: there, and pinned to nothing.
        let pin: Option<Option<String>> = match mode {
            Mode::Local => deps.get(&peer).map(|d| d.pin.clone()),
            Mode::Cluster => locked(&peer).map(|l| l.commit.clone()),
        };
        let pin = match pin {
            None => {
                peers.push(not(Outcome::Missing, NOT_INSTALLED));
                continue;
            }
            Some(None) => {
                let why = match mode {
                    Mode::Local => NO_MANIFEST_COMMIT,
                    Mode::Cluster => NO_LOCKED_COMMIT,
                };
                peers.push(not(Outcome::Refused, why));
                continue;
            }
            Some(Some(pin)) => pin,
        };
        match translate(query, &spec.peers[&peer], spec) {
            Err(why) => peers.push(not(Outcome::Unaligned, &why)),
            Ok((q, class, answer, key)) => asks.push(Ask {
                gather: name.to_string(),
                pin,
                lock: locked(&peer).map(|l| Locked {
                    url: l.url.clone(),
                    sha256: l.sha256.clone(),
                }),
                query: show(&q),
                class,
                answer,
                key,
                peer,
            }),
        }
    }
    (peers, asks)
}

/// Ask one peer from a root where it is unpacked under `.yidam/tonpa/<peer>/`.
///
/// **Never fails.** Every way asking can go wrong is an outcome for this peer; an asker that
/// errored out of the roll call would leave a gather reporting fewer peers than it named, which
/// reads as a consensus it does not have.
pub(crate) fn answer(root: &Path, ask: &Ask, image_digest: Option<String>) -> Asked {
    let foreign: Vec<Foreign> = Graph::foreign(root)
        .into_iter()
        .filter(|f| f.package == ask.peer)
        .collect();
    answer_from(
        root,
        ask,
        foreign.first(),
        &citations::installed(root),
        image_digest,
    )
}

fn answer_from(
    root: &Path,
    ask: &Ask,
    foreign: Option<&Foreign>,
    deps: &BTreeMap<String, citations::Installed>,
    image_digest: Option<String>,
) -> Asked {
    let refuse = |why: String| Asked::refused(ask, why, image_digest.clone());
    let bundle_sha256 = std::fs::read(
        crate::paths::tonpa_dir(root)
            .join(&ask.peer)
            .join("bundle.yiz"),
    )
    .ok()
    .map(|b| crate::deps::sha256_hex(&b));
    if let Some(l) = &ask.lock {
        if bundle_sha256.as_deref() != Some(l.sha256.as_str()) {
            return refuse(NOT_THE_LOCKED_BUNDLE.into());
        }
    }
    let Some(foreign) = foreign else {
        return refuse(NOT_INSTALLED.into());
    };
    match deps.get(&ask.peer).and_then(|d| d.pin.as_deref()) {
        None => return refuse(NO_MANIFEST_COMMIT.into()),
        Some(p) if p != ask.pin => {
            return refuse(format!(
                "its manifest.yml records commit {p}, and the gather was planned against {}",
                ask.pin
            ))
        }
        Some(_) => {}
    }
    let q = match lang::parse(&ask.query) {
        Ok(q) => q,
        Err(e) => {
            return refuse(format!(
                "the translated query `{}` does not parse: {e}",
                ask.query
            ))
        }
    };
    let (report, answers) = self::ask(
        foreign,
        &ask.pin,
        &q,
        &ask.class,
        &ask.answer,
        ask.key.as_deref(),
        deps,
    );
    Asked {
        format_version: ASKED_VERSION,
        ask: ask.clone(),
        report,
        answers,
        bundle_sha256,
        version: receipt::this_version(),
        image_digest,
    }
}

/// The plan and every asker's record, as one roll call.
pub(crate) struct Settled {
    pub peers: Vec<PeerReport>,
    pub cites: Vec<Cite>,
    pub found: Vec<Disagreement>,
    /// The records of peers that were asked and answered — or said nothing — in name order.
    /// Each gets a receipt.
    pub asked: Vec<Asked>,
}

/// Give every planned ask exactly one outcome, whatever came back for it.
///
/// A plan entry with no record is `refused`, never dropped: on a cluster that is a pod that
/// failed before it could say why, and it stays in the roll call saying so. A record is held
/// to the ask the gathering side planned, and to the peer and pin it was asked about; one that
/// is not is refused rather than cited.
pub(crate) fn settle(
    mut peers: Vec<PeerReport>,
    asks: &[Ask],
    mut records: BTreeMap<String, Asked>,
) -> Settled {
    let mut answers = Vec::new();
    let mut asked = Vec::new();
    for ask in asks {
        let refused = |why: String| Asked::refused(ask, why, None);
        let record = match records.remove(&ask.peer) {
            None => {
                refused("its asker returned no record — it failed before it could say why".into())
            }
            Some(r) if r.format_version != ASKED_VERSION => refused(format!(
                "its asker's record is format_version {}, and this binary reads {ASKED_VERSION}",
                r.format_version
            )),
            Some(r) if r.ask != *ask => {
                refused("its asker's record answers a different ask than this pin plans".into())
            }
            Some(r)
                if r.report.package != ask.peer
                    || r.answers
                        .iter()
                        .any(|a| a.cite.package != ask.peer || a.cite.commit != ask.pin) =>
            {
                refused("its asker's record cites a peer or a commit it was not asked".into())
            }
            Some(r) => r,
        };
        peers.push(record.report.clone());
        if matches!(record.report.outcome, Outcome::Answered | Outcome::Empty) {
            answers.extend(record.answers.iter().cloned());
            asked.push(record);
        }
    }
    peers.sort_by(|a, b| a.package.cmp(&b.package));
    let found = disagreements(&answers);
    let mut cites: Vec<Cite> = answers.into_iter().map(|a| a.cite).collect();
    cites.sort();
    Settled {
        peers,
        cites,
        found,
        asked,
    }
}

/// The plan, asked in this process against what is unpacked here.
fn survey(root: &Path, name: &str, spec: &Spec, query: &lang::Query) -> Settled {
    let (peers, asks) = plan(root, name, spec, query, Mode::Local);
    let deps = citations::installed(root);
    let foreign: BTreeMap<String, Foreign> = Graph::foreign(root)
        .into_iter()
        .map(|f| (f.package.clone(), f))
        .collect();
    let records = asks
        .iter()
        .map(|a| {
            (
                a.peer.clone(),
                answer_from(root, a, foreign.get(&a.peer), &deps, None),
            )
        })
        .collect();
    settle(peers, &asks, records)
}

// ── landing ───────────────────────────────────────────────────────────────────

/// The node file a question is written as. Serialised through serde so a label with a colon
/// or a quote in it cannot break the YAML.
#[derive(Serialize)]
struct QuestionNode<'a> {
    class: &'a str,
    label: String,
    description: String,
    cites: &'a [Cite],
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c.is_ascii_alphanumeric() {
            true => out.push(c.to_ascii_lowercase()),
            false if !out.ends_with('-') => out.push('-'),
            false => {}
        }
    }
    let out = out.trim_matches('-').to_string();
    match out.is_empty() {
        true => "key".into(),
        false => out,
    }
}

/// The account of who was asked, written into every landed node: absence is part of the
/// answer, and a reader of the node alone must be able to see it.
fn roll_call(peers: &[PeerReport]) -> String {
    let mut out = String::new();
    for p in peers {
        let _ = write!(out, "\n- {}: {}", p.package, p.outcome.as_str());
        if p.outcome == Outcome::Answered {
            let _ = write!(out, " ({} cited)", p.cited);
        }
        if let Some(r) = &p.reason {
            let _ = write!(out, " — {r}");
        }
    }
    out
}

/// One commit's worth of a gather: the question node it opens, and every file it adds.
pub(crate) struct Draft {
    /// The node's repo-relative path.
    pub path: String,
    /// `(repo-relative path, content)`, the node first. The first draft also carries every
    /// peer's receipt, so the receipts land with the question that cites them.
    pub files: Vec<(String, String)>,
    pub subject: String,
}

/// Where a gather's receipt for one peer is written: `.yidam/runs/gather/<name>/<peer>.yml`.
pub fn receipt_step(name: &str, peer: &str) -> String {
    format!("gather/{name}/{peer}")
}

/// A receipt for one peer's answer (#1217): what it was asked from, and what answered.
///
/// A gather is a connector in RFC-0026's sense — its answer depends on bytes this repository
/// does not hold — so its receipt records the peer's bundle by digest beside the gather spec
/// that planned the ask. Two runs over the same pins give the same input state; a new binary
/// or a new image does not, by the rule [`Receipt`] states, change it.
fn peer_receipt(
    name: &str,
    spec: &Spec,
    asked: &Asked,
    outputs: &[(String, String)],
) -> Result<(String, String)> {
    let peer = &asked.ask.peer;
    let spec_path = format!(".yidam/gathers/{name}.toml");
    let cap = Capability {
        kind: Kind::Connector,
        run: Run::Argv(vec!["yidam".into(), "gather".into(), name.into()]),
        reads: vec![spec_path.clone(), format!(".yidam/tonpa/{peer}/**")],
        writes: vec![format!(
            ".yidam/corpus/{}/gather-{name}*.yml",
            spec.lands_as
        )],
        verb: "open".into(),
        after: vec![],
        ageing_days: None,
        cluster: None,
    };
    let mut files = vec![File {
        path: spec_path,
        sha256: spec.digest.clone(),
    }];
    if let Some(sha) = &asked.bundle_sha256 {
        files.push(File {
            path: format!(".yidam/tonpa/{peer}/bundle.yiz"),
            sha256: sha.clone(),
        });
    }
    let config_sha256 = receipt::sha256(b"");
    let input_state = Receipt::input_state(&cap, &spec.digest, &config_sha256, &files, None, None)?;
    let step = receipt_step(name, peer);
    let r = Receipt {
        format_version: receipt::FORMAT_VERSION,
        step: step.clone(),
        kind: "gather",
        verb: cap.verb.clone(),
        run: cap.run.clone(),
        input_state,
        input: Input {
            commit: asked.ask.pin.clone(),
            manifest_sha256: spec.digest.clone(),
            config_sha256,
            reads: cap.reads.clone(),
            files,
            resolved_graph_sha256: None,
            script_sha256: None,
        },
        writes: cap.writes.clone(),
        outputs: outputs
            .iter()
            .map(|(path, content)| File {
                path: path.clone(),
                sha256: receipt::sha256(content.as_bytes()),
            })
            .collect(),
        model: None,
        version: asked.version.clone(),
        config: None,
        image_digest: asked.image_digest.clone(),
    };
    Ok((Receipt::path(&step), r.to_yaml()?))
}

/// Every node this gather lands, one per commit, with the receipts in the first.
fn draft(name: &str, spec: &Spec, settled: &Settled) -> Result<Vec<Draft>> {
    let dir = format!(".yidam/corpus/{}", spec.lands_as);
    let asked = format!(
        "Gathered by `yidam gather {name}`. Each answer below is cited, not imported: nothing \
         was merged and no claim was re-tagged. Peers:{}",
        roll_call(&settled.peers)
    );
    // `(path, content, subject, the peers it cites)`
    let mut nodes: Vec<(String, String, String, BTreeSet<String>)> = Vec::new();
    let label = format!("? {}", spec.question.trim());
    let node = QuestionNode {
        class: &spec.lands_as,
        label: label.clone(),
        description: asked.clone(),
        cites: &settled.cites,
    };
    nodes.push((
        format!("{dir}/gather-{name}.yml"),
        serde_yaml::to_string(&node)?,
        label,
        settled.cites.iter().map(|c| c.package.clone()).collect(),
    ));
    for d in &settled.found {
        let packages: BTreeSet<&str> = d.answers.iter().map(|c| c.package.as_str()).collect();
        let label = format!(
            "? {} disagree on {} for {} = {}",
            packages.iter().copied().collect::<Vec<_>>().join(", "),
            spec.answer,
            spec.key.as_deref().unwrap_or("key"),
            d.key
        );
        let node = QuestionNode {
            class: &spec.lands_as,
            label: label.clone(),
            description: format!(
                "The peers cited here give different answers under one key, and this corpus \
                 does not decide between them. {asked}"
            ),
            cites: &d.answers,
        };
        nodes.push((
            format!("{dir}/gather-{name}-{}.yml", slug(&d.key)),
            serde_yaml::to_string(&node)?,
            label,
            packages.into_iter().map(str::to_string).collect(),
        ));
    }

    let mut receipts = Vec::new();
    for a in &settled.asked {
        let outputs: Vec<(String, String)> = nodes
            .iter()
            .filter(|(_, _, _, cited)| cited.contains(&a.ask.peer))
            .map(|(p, c, _, _)| (p.clone(), c.clone()))
            .collect();
        receipts.push(peer_receipt(name, spec, a, &outputs)?);
    }
    let mut out: Vec<Draft> = nodes
        .into_iter()
        .map(|(path, content, subject, _)| Draft {
            files: vec![(path.clone(), content)],
            path,
            subject,
        })
        .collect();
    if let Some(first) = out.first_mut() {
        first.files.extend(receipts);
    }
    Ok(out)
}

pub fn branch_for(name: &str, short_head: &str) -> String {
    format!("propose/gather/{name}/{short_head}")
}

fn tree_of(root: &Path, rev: &str) -> Option<String> {
    write::git(root, None, &["rev-parse", &format!("{rev}^{{tree}}")], None).ok()
}

/// The tree after each draft, built on `base` in a temporary index.
fn trees(root: &Path, base: &str, drafts: &[Draft]) -> Result<Vec<String>> {
    let scratch = TempIndex::new(root, "gather")?;
    let index = scratch.path().to_path_buf();
    write::git(root, Some(&index), &["read-tree", base], None)?;
    let mut trees = Vec::new();
    for d in drafts {
        for (path, content) in &d.files {
            let blob = write::git(
                root,
                Some(&index),
                &["hash-object", "-w", "--stdin"],
                Some(content),
            )?;
            write::git(
                root,
                Some(&index),
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("100644,{blob},{path}"),
                ],
                None,
            )?;
        }
        trees.push(write::git(root, Some(&index), &["write-tree"], None)?);
    }
    Ok(trees)
}

/// One `open:` commit per draft on top of `base`. Full shas, oldest first.
fn chain(
    root: &Path,
    name: &str,
    base: &str,
    short: &str,
    drafts: &[Draft],
    trees: &[String],
) -> Result<Vec<String>> {
    let mut parent = base.to_string();
    let mut commits = Vec::new();
    for (d, tree) in drafts.iter().zip(trees) {
        let message = format!(
            "open: {}\n\nGathered by `yidam gather {name}`: {}\n\nNothing was imported and no \
             claim was merged — every answer is a `cites:` entry pinned to the peer's \
             commit.\n\nProposed-from: {short}\n",
            d.subject, d.path
        );
        let sha = write::commit_tree(root, tree, &parent, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;
        commits.push(sha.clone());
        parent = sha;
    }
    Ok(commits)
}

fn receipts_of(drafts: &[Draft]) -> Vec<String> {
    drafts
        .iter()
        .flat_map(|d| d.files.iter().skip(1).map(|(p, _)| p.clone()))
        .collect()
}

/// One `open:` commit per question node, on `propose/gather/<name>/<head>`.
///
/// **Unchanged is a result, not a refusal.** The tree is built first; if HEAD or the branch
/// already holds exactly it, nothing is committed and no ref moves — a repeat gather over
/// peers that have not moved writes nothing. A branch holding something *else* is refused
/// unless `force`, for the reason `propose` refuses: it may be somebody's review in progress.
fn land(root: &Path, name: &str, drafts: &[Draft], force: bool) -> Result<Landed> {
    let (full, short) = write::head(root)?;
    let branch = branch_for(name, &short);
    let nodes: Vec<String> = drafts.iter().map(|d| d.path.clone()).collect();
    let receipts = receipts_of(drafts);

    let trees = trees(root, "HEAD", drafts)?;
    let last = trees.last().cloned().unwrap_or_default();
    let existing = tree_of(root, &format!("refs/heads/{branch}"));
    if tree_of(root, "HEAD").as_deref() == Some(&last) || existing.as_deref() == Some(&last) {
        return Ok(Landed {
            branch,
            commits: vec![],
            nodes,
            receipts,
            unchanged: true,
        });
    }
    if existing.is_some() && !force {
        bail!(
            "{branch} already exists and holds a different answer — the peers moved, or it \
             was edited.\n  Review it with `git log --reverse HEAD..{branch}`, delete it, or \
             re-run with --force to replace it."
        );
    }

    let commits = chain(root, name, &full, &short, drafts, &trees)?;
    let tip = commits.last().cloned().unwrap_or(full);
    write::git(
        root,
        None,
        &["update-ref", &format!("refs/heads/{branch}"), &tip],
        None,
    )?;
    Ok(Landed {
        branch,
        commits: commits.iter().map(|c| write::short_of(root, c)).collect(),
        nodes,
        receipts,
        unchanged: false,
    })
}

/// A gather on a cluster, from the gathering pod's clone at `input` (#1217).
///
/// Plans from `tonpa.lock` and settles against the askers' records, then commits on `input`
/// without moving any ref: the tip is shipped through the vault, and the lander alone pushes
/// it. `None` for the tip when nothing answered, or when `input` already holds exactly this
/// tree — the repeat run over pins that have not moved.
pub(crate) fn gather_at(
    root: &Path,
    name: &str,
    input: &str,
    records: BTreeMap<String, Asked>,
) -> Result<(GatherReport, Option<String>)> {
    let local = Graph::load(root);
    let (spec, query) = load(root, name, &local)?;
    let (peers, asks) = plan(root, name, &spec, &query, Mode::Cluster);
    let settled = settle(peers, &asks, records);
    let short = write::short_of(root, input);
    let mut tip = None;
    let landed = if settled.cites.is_empty() {
        None
    } else {
        let drafts = draft(name, &spec, &settled)?;
        let trees = trees(root, input, &drafts)?;
        let unchanged = trees.last() == tree_of(root, input).as_ref();
        let commits = if unchanged {
            vec![]
        } else {
            chain(root, name, input, &short, &drafts, &trees)?
        };
        tip = commits.last().cloned();
        Some(Landed {
            branch: branch_for(name, &short),
            commits: commits.iter().map(|c| write::short_of(root, c)).collect(),
            nodes: drafts.iter().map(|d| d.path.clone()).collect(),
            receipts: receipts_of(&drafts),
            unchanged,
        })
    };
    Ok((report(name, &spec, settled, landed, false), tip))
}

/// The plan alone, from the gathering pod's clone — who is asked, and what.
pub(crate) fn plan_at(root: &Path, name: &str) -> Result<(Vec<PeerReport>, Vec<Ask>)> {
    let local = Graph::load(root);
    let (spec, query) = load(root, name, &local)?;
    Ok(plan(root, name, &spec, &query, Mode::Cluster))
}

fn report(
    name: &str,
    spec: &Spec,
    settled: Settled,
    landed: Option<Landed>,
    dry_run: bool,
) -> GatherReport {
    GatherReport {
        kind: "gather",
        gather: name.to_string(),
        question: spec.question.trim().to_string(),
        query: spec.query.clone(),
        peers: settled.peers,
        cites: settled.cites,
        disagreements: settled.found,
        landed,
        dry_run,
    }
}

// ── the command ───────────────────────────────────────────────────────────────

pub fn gather(opts: Options) -> Result<()> {
    let root = repo_root()?;
    let report = run(&root, &opts)?;
    crate::report::finish(&root, opts.format, report, |r| println!("{}", render(r)))
}

pub(crate) fn run(root: &Path, opts: &Options) -> Result<GatherReport> {
    require_yidam_repo(root)?;
    write::require_committed_corpus(root)?;
    let local = Graph::load(root);
    let (spec, query) = load(root, &opts.name, &local)?;
    let settled = survey(root, &opts.name, &spec, &query);

    let landed = match opts.dry_run || settled.cites.is_empty() {
        true => None,
        false => {
            let drafts = draft(&opts.name, &spec, &settled)?;
            Some(land(root, &opts.name, &drafts, opts.force)?)
        }
    };
    Ok(report(&opts.name, &spec, settled, landed, opts.dry_run))
}

pub fn render(r: &GatherReport) -> String {
    let mut out = format!(
        "gather {} — ? {}\n  query: {}\n",
        r.gather, r.question, r.query
    );
    for p in &r.peers {
        let _ = write!(out, "\n  {:<10} {}", p.outcome.as_str(), p.package);
        if let Some(asked) = &p.asked {
            let _ = write!(out, "  asked {asked}");
        }
        if p.outcome == Outcome::Answered {
            let _ = write!(out, "  {} matched, {} cited", p.matched, p.cited);
        }
        if let Some(reason) = &p.reason {
            let _ = write!(out, "\n             {reason}");
        }
        if !p.unquotable.is_empty() {
            let _ = write!(
                out,
                "\n             not quotable verbatim: {}",
                p.unquotable.join(", ")
            );
        }
    }
    out.push('\n');
    for d in &r.disagreements {
        let packages: BTreeSet<&str> = d.answers.iter().map(|c| c.package.as_str()).collect();
        let _ = write!(
            out,
            "\n  disagree on {}: {}",
            d.key,
            packages.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    match (&r.landed, r.dry_run, r.cites.is_empty()) {
        (_, true, _) => out.push_str("\n  dry run — nothing written"),
        (None, _, true) => out.push_str("\n  no peer answered — nothing to land"),
        (Some(l), _, _) if l.unchanged => {
            let _ = write!(
                out,
                "\n  unchanged — {} already holds this answer",
                l.branch
            );
        }
        (Some(l), _, _) => {
            let _ = write!(
                out,
                "\n  {} commit(s) on {} — review with `git log --reverse HEAD..{}`",
                l.commits.len(),
                l.branch,
                l.branch
            );
        }
        (None, _, _) => {}
    }
    out
}
