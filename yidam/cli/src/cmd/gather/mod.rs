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

fn valid_name(name: &str) -> bool {
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
    let spec: Spec =
        toml::from_str(&text).with_context(|| format!("{} does not parse", path.display()))?;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Cite {
    pub package: String,
    pub node: String,
    pub commit: String,
    pub span: String,
}

/// An answer with the key it was given under, for disagreement detection.
#[derive(Debug, Clone)]
struct Answer {
    cite: Cite,
    value: String,
    keys: Vec<String>,
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
fn show(q: &lang::Query) -> String {
    let step = &q.steps[0];
    let preds: Vec<String> = step.filter.iter().map(lang::Pred::spelled).collect();
    match preds.is_empty() {
        true => step.class.clone(),
        false => format!("{}[{}]", step.class, preds.join(",")),
    }
}

/// Ask one installed, pinned, aligned peer.
fn ask(
    foreign: &Foreign,
    pin: &str,
    query: &lang::Query,
    corr: &Correspondence,
    spec: &Spec,
    deps: &BTreeMap<String, citations::Installed>,
) -> (PeerReport, Vec<Answer>) {
    let package = foreign.package.as_str();
    let (q, class, answer, key) = match translate(query, corr, spec) {
        Ok(t) => t,
        Err(why) => {
            return (
                PeerReport::not_asked(package, Outcome::Unaligned, why),
                vec![],
            )
        }
    };
    let mut report = PeerReport {
        pin: Some(pin.to_string()),
        asked: Some(show(&q)),
        ..PeerReport::not_asked(package, Outcome::Unaligned, String::new())
    };
    let declared = foreign.classes.iter().find(|c| c.name == class);
    if declared.is_none() {
        report.reason = Some(format!("`{package}` declares no class `{class}`"));
        return (report, vec![]);
    }
    for prop in std::iter::once(&answer).chain(key.as_ref()) {
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
    let checked = match check::check(&q, &schema) {
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
        &q,
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
        let keys = key
            .as_deref()
            .map(|k| exec::node_scalars(node, k))
            .unwrap_or_default();
        let mut quoted = false;
        for value in exec::node_scalars(node, &answer) {
            let Some(span) = quote(&node.text, &answer, &value) else {
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

/// Every peer's outcome, in name order: the named ones asked or refused, and the installed
/// ones nobody named.
pub(crate) fn survey(
    root: &Path,
    spec: &Spec,
    query: &lang::Query,
) -> (Vec<PeerReport>, Vec<Cite>, Vec<Disagreement>) {
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    let deps = citations::installed(root);
    let lock = crate::deps::load_lock(&crate::paths::tonpa_lock_path(root)).unwrap_or_default();
    let tonpa = crate::paths::tonpa_dir(root);
    let foreign: BTreeMap<String, Foreign> = Graph::foreign(root)
        .into_iter()
        .map(|f| (f.package.clone(), f))
        .collect();

    let mut names: BTreeSet<String> = spec.peers.keys().cloned().collect();
    names.extend(deps.keys().cloned());
    names.extend(
        config
            .dependencies
            .iter()
            .filter(|(_, d)| d.path.is_some())
            .map(|(n, _)| n.clone()),
    );

    let mut peers = Vec::new();
    let mut answers = Vec::new();
    for name in names {
        let is_path = config
            .dependencies
            .get(&name)
            .is_some_and(|d| d.path.is_some())
            || deps
                .get(&name)
                .is_some_and(|d| d.kind == DependencyKind::Path);
        let report = if is_path {
            PeerReport::not_asked(
                &name,
                Outcome::Refused,
                "a path dependency is a working tree, not a pinned commit — a citation into \
                 it could not say what it quoted; install it as a fetched bundle"
                    .into(),
            )
        } else if !spec.peers.contains_key(&name) {
            PeerReport::not_asked(
                &name,
                Outcome::Undeclared,
                "installed, and the gather names no correspondence for it — not asked".into(),
            )
        } else if let (Some(dep), Some(f)) = (deps.get(&name), foreign.get(&name)) {
            let locked = lock.packages.iter().find(|p| p.name == name);
            match (&dep.pin, locked) {
                (None, _) => PeerReport::not_asked(
                    &name,
                    Outcome::Refused,
                    "its manifest.yml records no commit, so a citation could not be pinned".into(),
                ),
                (Some(_), Some(l))
                    if !crate::deps::verify_installed(&name, &tonpa, l).unwrap_or(false) =>
                {
                    PeerReport::not_asked(
                        &name,
                        Outcome::Refused,
                        "the unpacked bundle is not the one tonpa.lock pins — run \
                         `yidam tonpa install`"
                            .into(),
                    )
                }
                (Some(pin), _) => {
                    let (r, a) = ask(f, pin, query, &spec.peers[&name], spec, &deps);
                    answers.extend(a);
                    r
                }
            }
        } else {
            PeerReport::not_asked(
                &name,
                Outcome::Missing,
                "named by the gather and not installed — declare it in .yidam/tonpa.toml \
                 and run `yidam tonpa install`"
                    .into(),
            )
        };
        peers.push(report);
    }
    let found = disagreements(&answers);
    let mut cites: Vec<Cite> = answers.into_iter().map(|a| a.cite).collect();
    cites.sort();
    (peers, cites, found)
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

/// `(repo-relative path, file content, commit subject)` for every node this gather lands.
fn draft(
    name: &str,
    spec: &Spec,
    peers: &[PeerReport],
    cites: &[Cite],
    found: &[Disagreement],
) -> Result<Vec<(String, String, String)>> {
    let dir = format!(".yidam/corpus/{}", spec.lands_as);
    let asked = format!(
        "Gathered by `yidam gather {name}`. Each answer below is cited, not imported: nothing \
         was merged and no claim was re-tagged. Peers:{}",
        roll_call(peers)
    );
    let mut out = Vec::new();
    let label = format!("? {}", spec.question.trim());
    let node = QuestionNode {
        class: &spec.lands_as,
        label: label.clone(),
        description: asked.clone(),
        cites,
    };
    out.push((
        format!("{dir}/gather-{name}.yml"),
        serde_yaml::to_string(&node)?,
        label,
    ));
    for d in found {
        let packages: BTreeSet<&str> = d.answers.iter().map(|c| c.package.as_str()).collect();
        let label = format!(
            "? {} disagree on {} for {} = {}",
            packages.into_iter().collect::<Vec<_>>().join(", "),
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
        out.push((
            format!("{dir}/gather-{name}-{}.yml", slug(&d.key)),
            serde_yaml::to_string(&node)?,
            label,
        ));
    }
    Ok(out)
}

pub fn branch_for(name: &str, short_head: &str) -> String {
    format!("propose/gather/{name}/{short_head}")
}

fn tree_of(root: &Path, rev: &str) -> Option<String> {
    write::git(root, None, &["rev-parse", &format!("{rev}^{{tree}}")], None).ok()
}

/// One `open:` commit per question node, on `propose/gather/<name>/<head>`.
///
/// **Unchanged is a result, not a refusal.** The tree is built first; if HEAD or the branch
/// already holds exactly it, nothing is committed and no ref moves — a repeat gather over
/// peers that have not moved writes nothing. A branch holding something *else* is refused
/// unless `force`, for the reason `propose` refuses: it may be somebody's review in progress.
fn land(
    root: &Path,
    name: &str,
    nodes: &[(String, String, String)],
    force: bool,
) -> Result<Landed> {
    let (full, short) = write::head(root)?;
    let branch = branch_for(name, &short);
    let paths: Vec<String> = nodes.iter().map(|(p, _, _)| p.clone()).collect();

    let scratch = TempIndex::new(root, "gather")?;
    let index = scratch.path().to_path_buf();
    write::git(root, Some(&index), &["read-tree", "HEAD"], None)?;
    let mut trees = Vec::new();
    for (path, content, _) in nodes {
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
        trees.push(write::git(root, Some(&index), &["write-tree"], None)?);
    }
    let last = trees.last().cloned().unwrap_or_default();
    let existing = tree_of(root, &format!("refs/heads/{branch}"));
    if tree_of(root, "HEAD").as_deref() == Some(&last) || existing.as_deref() == Some(&last) {
        return Ok(Landed {
            branch,
            commits: vec![],
            nodes: paths,
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

    let mut parent = full;
    let mut commits = Vec::new();
    for ((path, _, subject), tree) in nodes.iter().zip(&trees) {
        let message = format!(
            "open: {subject}\n\nGathered by `yidam gather {name}`: {path}\n\nNothing was \
             imported and no claim was merged — every answer is a `cites:` entry pinned to \
             the peer's commit.\n\nProposed-from: {short}\n"
        );
        let sha = write::commit_tree(root, tree, &parent, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;
        commits.push(write::short_of(root, &sha));
        parent = sha;
    }
    write::git(
        root,
        None,
        &["update-ref", &format!("refs/heads/{branch}"), &parent],
        None,
    )?;
    Ok(Landed {
        branch,
        commits,
        nodes: paths,
        unchanged: false,
    })
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
    let (peers, cites, found) = survey(root, &spec, &query);

    let landed = match opts.dry_run || cites.is_empty() {
        true => None,
        false => {
            let nodes = draft(&opts.name, &spec, &peers, &cites, &found)?;
            Some(land(root, &opts.name, &nodes, opts.force)?)
        }
    };
    Ok(GatherReport {
        kind: "gather",
        gather: opts.name.clone(),
        question: spec.question.trim().to_string(),
        query: spec.query.clone(),
        peers,
        cites,
        disagreements: found,
        landed,
        dry_run: opts.dry_run,
    })
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
