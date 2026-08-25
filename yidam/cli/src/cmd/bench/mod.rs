//! `yidam bench` — the paper's cost claim, measured against a goal set the repository owns.
//!
//! `docs/research/system` argues that ontological anchoring converts O(*n*) scan into
//! O(depth) lookup. This command is the executable for that claim: for each goal in
//! `.yidam/bench/goals.yml` it resolves the same question twice — once by **flat
//! retrieval**, once by **anchored traversal** — and reports precision and recall at a
//! fixed budget beside what each arm cost.
//!
//! # Why precision, and not cost alone
//!
//! The measurement this command was first specified to make would have been vacuous. Under
//! top-*k* retrieval the blind cost is `k × tokens-per-candidate` — **constant in N** — so
//! both arms plot flat against corpus size and the ratio reports the `k` we chose rather
//! than anything about the ontology. O(*n*) is real against a *full-scan* arm and against
//! nothing else.
//!
//! So against a fixed-*k* baseline the honest claim is **precision at fixed budget**: does
//! anchored traversal reach the *right* nodes where top-*k* reaches merely five? That is
//! why every goal carries an exhaustive `expect` set, and why this command scores rather
//! than only counting tokens.
//!
//! # Why the flat arm is not re-implemented here
//!
//! #264 requires the flat arm to be *"the retrieval `serve --mcp` performs today,
//! unmodified"*. So it is dispatched through [`crate::cmd::serve::tools::call`] by name,
//! envelope and all, rather than by calling anything this module owns. Unwrapping the MCP
//! content envelope costs five lines and buys the guarantee: if the server's retrieval
//! changes, this arm changes with it, and there is no second implementation to drift.
//!
//! # Why it refuses instead of degrading
//!
//! `retrieve` has two bodies. With `--features index` it is vector cosine top-*k*; without,
//! it falls through to keyword matching and reports `degraded: true`. **Beating keyword
//! search proves nothing about RAG**, and a bench summary printing a token count would not
//! surface the flag. `yidam query` degrades and says so; a measurement refuses. The test is
//! on the retrieval state rather than on the compiled feature, because a full build with no
//! index built is in exactly the same position.

pub mod scaling;

use anyhow::{bail, Result};
use std::collections::BTreeMap;

use crate::cmd::serve::{tools, ServerState};
use crate::paths::{repo_root, yidam_bench_dir};

pub const GOALS_FILE: &str = "goals.yml";

/// The goal-set format this build understands.
///
/// Checked rather than ignored, for the reason `report.rs` gives about `FORMAT_VERSION`: a
/// benchmark that mis-reads a newer goal set produces numbers rather than an error, and
/// numbers are believed.
pub const GOALS_VERSION: u32 = 1;

/// The lower end of the range `docs/research/system` claims for focused scan: "one to two
/// orders of magnitude cheaper than blind scan".
///
/// Used only to tell a reader whether the corpus in front of them could show that effect at
/// all. A corpus whose class-narrowing ceiling is below this cannot reach the claim by
/// arithmetic, whatever the traversal does.
pub(crate) const CLAIMED_NARROWING_FLOOR: f64 = 10.0;

// ── the goal set ──────────────────────────────────────────────────────────────

/// One benchmark goal, as the repository committed it.
///
/// Deserialized permissively — an unknown field is ignored rather than fatal, because the
/// goal set is authored by hand and a benchmark that refuses to run over a file with a
/// typo'd comment key is a benchmark nobody runs.
#[derive(Debug, serde::Deserialize)]
pub struct Goal {
    pub id: String,
    #[serde(default)]
    pub question: String,
    /// `traversal`, `filter`, or `unanchorable`. Read for the report; not branched on, so a
    /// corpus may coin another without this command rejecting its goal set.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub hops: Option<usize>,
    /// The query the anchored arm runs, or `None` when the ontology cannot express the goal.
    #[serde(default)]
    pub anchored: Option<String>,
    #[serde(default)]
    pub anchored_omitted_because: Option<String>,
    /// The text handed to `retrieve`, or `None` when retrieval cannot pose the question.
    #[serde(default)]
    pub flat: Option<String>,
    #[serde(default)]
    pub flat_omitted_because: Option<String>,
    /// **Exhaustive**: the complete set of nodes a correct answer contains, so recall is
    /// computable and an arm cannot score well by returning everything.
    pub expect: Vec<String>,
    #[serde(default)]
    pub why: String,
}

impl Goal {
    /// Whether this goal's numbers may be folded into a headline ratio.
    ///
    /// A goal one arm cannot express is a category difference, not a cost difference.
    /// Averaging it in would be the strawman comparison #264 forbids; dropping it would
    /// hide a real finding. So it is reported, and excluded from the mean.
    pub fn counts_toward_ratio(&self) -> bool {
        self.anchored.is_some() && self.flat.is_some()
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct GoalSet {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub corpus: String,
    pub goals: Vec<Goal>,
}

/// Parse a goal set, rejecting the shapes that would make a result meaningless.
///
/// The validation is deliberately narrow. It does not check that `expect` names nodes that
/// exist — that is the corpus's business and `lint`'s — but it does refuse a goal with an
/// empty `expect`, because recall against an empty set is either 0/0 or 1 depending on which
/// arm you ask, and a benchmark that silently scores it is publishing an artifact.
pub fn parse_goals(text: &str) -> Result<GoalSet> {
    let set: GoalSet = serde_yaml::from_str(text)?;
    if set.version != GOALS_VERSION {
        bail!(
            "goal set declares version {} and this build reads {GOALS_VERSION} — refusing \
             rather than scoring a file it may be mis-reading",
            set.version
        );
    }
    if set.goals.is_empty() {
        bail!("the goal set declares no goals");
    }
    let mut seen = std::collections::BTreeSet::new();
    for goal in &set.goals {
        if goal.expect.is_empty() {
            bail!(
                "goal `{}` has an empty `expect` — recall against an empty answer set is \
                 not a number",
                goal.id
            );
        }
        if !seen.insert(goal.id.clone()) {
            bail!(
                "goal id `{}` appears twice; ids identify a row in the report",
                goal.id
            );
        }
        if goal.anchored.is_none() && goal.anchored_omitted_because.is_none() {
            bail!(
                "goal `{}` has no anchored query and no `anchored_omitted_because` — an arm \
                 omitted without a stated reason reads as an arm that lost",
                goal.id
            );
        }
        if goal.flat.is_none() && goal.flat_omitted_because.is_none() {
            bail!(
                "goal `{}` has no flat query and no `flat_omitted_because` — an arm omitted \
                 without a stated reason reads as an arm that lost",
                goal.id
            );
        }
    }
    Ok(set)
}

// ── identity ──────────────────────────────────────────────────────────────────

/// The corpus-relative id of a node, from whatever path an arm reported.
///
/// Both retrieval bodies emit `path`, and both emit it repo-relative
/// (`.yidam/corpus/<class>/<name>.yml`) — the vector rows because `embed` strips the root
/// before writing the record, the keyword body because it formats the same shape. Anything
/// that is *not* under the corpus — a catalog entry, a dependency's node — has no corpus id
/// and keeps its path, so it is visibly foreign in the report and scores as a miss. That is
/// correct rather than unfortunate: the flat arm searches a wider set than the corpus, and
/// hiding the difference would flatter it.
pub fn node_id(path: &str) -> String {
    let path = path.replace('\\', "/");
    match path.split_once(".yidam/corpus/") {
        Some((_, rest)) => rest.to_string(),
        None => path,
    }
}

/// An `expect` entry, tolerant of the ways it is written by hand.
fn expected_id(raw: &str) -> String {
    let id = raw.trim().trim_start_matches('/');
    let id = id.strip_prefix(".yidam/corpus/").unwrap_or(id);
    match id.ends_with(".yml") {
        true => id.to_string(),
        false => format!("{id}.yml"),
    }
}

// ── the report ────────────────────────────────────────────────────────────────

/// What the corpus can show, independent of what the arms did.
///
/// Carried on every report because #264's decision 3 requires it: *"Every report prints N,
/// class count, and `N/min|C|` beside the measured ratio, so a reader can see when the
/// corpus is the binding constraint."*
#[derive(Debug, serde::Serialize)]
pub struct CorpusShape {
    pub nodes: usize,
    pub classes: usize,
    pub smallest_class: String,
    pub smallest_class_size: usize,
    /// `N / min|C|` — the most a class predicate alone could ever narrow this corpus.
    pub narrowing_ceiling: f64,
    /// Whether that ceiling reaches the low end of the range the paper claims.
    pub ceiling_reaches_claim: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct ArmReport {
    pub arm: &'static str,
    pub ran: bool,
    /// Why this arm did not run: the goal set's stated reason, or the tooling's.
    pub unavailable_reason: Option<String>,
    /// Node ids the arm put in front of the agent, in rank order.
    ///
    /// Capped by [`RETURNED_SHOWN`] — the full-scan arm's candidate set is the corpus, and a
    /// four-thousand-entry array in a report is not a result, it is a copy of the input.
    /// `candidates` is the number that matters and is never capped.
    pub returned: Vec<String>,
    pub hits: Vec<String>,
    /// How many nodes the agent had to consider. **Precision's denominator**, and not
    /// `returned.len()`: the full-scan arm considers every node and returns the ones that
    /// answer, so scoring it on what it returned would give it perfect precision for
    /// reading the whole corpus.
    pub candidates: Option<usize>,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    /// Nodes an agent consuming this answer would have to read — the same number as
    /// `candidates`, named for what it costs rather than for what it scores.
    pub nodes_read: Option<usize>,
    /// `chars / 4` over those nodes' instance text, the approximation `export_llms`
    /// documents. The arms are charged the same per-node rate, so they differ only in
    /// *which* nodes and *how many* — which is the whole comparison.
    pub tokens: Option<usize>,
}

/// How many ranked ids a report shows per arm before it is summarising rather than listing.
const RETURNED_SHOWN: usize = 20;

impl ArmReport {
    /// An arm with nothing filled in, for a caller that computes its own aggregate.
    ///
    /// `--scaling`'s full-scan arm reads every node for every goal, so its cost is one
    /// number for the whole size rather than one per goal, and going through [`Self::scored`]
    /// would mean materialising four thousand ids to count them.
    pub(crate) fn empty(arm: &'static str) -> Self {
        Self {
            arm,
            ran: false,
            unavailable_reason: None,
            returned: Vec::new(),
            hits: Vec::new(),
            candidates: None,
            precision: None,
            recall: None,
            nodes_read: None,
            tokens: None,
        }
    }

    pub(crate) fn unavailable(arm: &'static str, reason: impl Into<String>) -> Self {
        Self {
            arm,
            ran: false,
            unavailable_reason: Some(reason.into()),
            returned: Vec::new(),
            hits: Vec::new(),
            candidates: None,
            precision: None,
            recall: None,
            nodes_read: None,
            tokens: None,
        }
    }

    /// Score a candidate set against the goal's exhaustive expected set.
    ///
    /// Precision over an empty candidate set is `None`, not 0. An arm that considered
    /// nothing has no precision — 0/0 is undefined, and recording it as zero would drag a
    /// mean downward on a goal where the arm made no claim at all.
    fn scored(
        arm: &'static str,
        candidates: Vec<String>,
        expect: &[String],
        chars: &BTreeMap<String, usize>,
    ) -> Self {
        let hits: Vec<String> = candidates
            .iter()
            .filter(|id| expect.contains(id))
            .cloned()
            .collect();
        let considered = candidates.len();
        let precision = match considered {
            0 => None,
            n => Some(hits.len() as f64 / n as f64),
        };
        let recall = match expect.is_empty() {
            true => None,
            false => Some(hits.len() as f64 / expect.len() as f64),
        };
        let read: usize = candidates
            .iter()
            .map(|id| chars.get(id).copied().unwrap_or_default())
            .sum();
        let mut returned = candidates;
        returned.truncate(RETURNED_SHOWN);
        Self {
            arm,
            ran: true,
            unavailable_reason: None,
            candidates: Some(considered),
            nodes_read: Some(considered),
            tokens: Some(read / 4),
            returned,
            hits,
            precision,
            recall,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GoalReport {
    pub id: String,
    pub question: String,
    pub kind: String,
    /// Path length the goal set declares, `None` where no path expresses the goal.
    pub hops: Option<usize>,
    /// The goal set's own argument for including this goal.
    ///
    /// Carried into the report rather than left in the source file, because #264's standard
    /// is that the benchmark be *arguable*: a reader handed a number should not have to open
    /// another file to find out why the goal is in the set.
    pub why: String,
    pub expect: Vec<String>,
    pub counts_toward_ratio: bool,
    pub flat: ArmReport,
    pub full_scan: ArmReport,
    pub anchored: ArmReport,
}

#[derive(Debug, serde::Serialize)]
pub struct Summary {
    /// Goals both arms could express, and which therefore carry the comparison.
    pub compared: usize,
    pub reported_only: usize,
    pub flat_mean_precision: Option<f64>,
    pub flat_mean_recall: Option<f64>,
    pub full_scan_mean_precision: Option<f64>,
    pub full_scan_mean_recall: Option<f64>,
    pub anchored_mean_precision: Option<f64>,
    pub anchored_mean_recall: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct BenchReport {
    /// The goal set's own name for the corpus it was written against.
    pub goal_set: String,
    /// `regression-guard` — what a single-corpus run is worth. Never `evidence`.
    pub standing: &'static str,
    pub standing_reason: String,
    pub budget: usize,
    pub corpus: CorpusShape,
    pub goals: Vec<GoalReport>,
    pub summary: Summary,
}

// ── running ───────────────────────────────────────────────────────────────────

/// The flat arm: `retrieve`, exactly as `serve --mcp` dispatches it.
///
/// Routed through `tools::call` by tool name rather than through the retrieval function, so
/// that this is provably the same path an agent takes — envelope, error convention and all.
/// The envelope carries the payload as pretty-printed JSON in `content[0].text`.
fn flat_arm(
    state: &ServerState,
    query: &str,
    budget: usize,
    expect: &[String],
    chars: &BTreeMap<String, usize>,
) -> ArmReport {
    let args = serde_json::json!({ "query": query, "k": budget });
    let envelope = tools::call(state, "retrieve", &args);
    if envelope["isError"].as_bool().unwrap_or(false) {
        let message = envelope["content"][0]["text"]
            .as_str()
            .unwrap_or("retrieve failed")
            .to_string();
        return ArmReport::unavailable("flat", message);
    }
    let payload: serde_json::Value = envelope["content"][0]["text"]
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();
    let returned = payload["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r["path"].as_str())
                .map(node_id)
                .collect()
        })
        .unwrap_or_default();
    ArmReport::scored("flat", returned, expect, chars)
}

/// The full-scan arm: read the whole corpus and find the answer inside it.
///
/// This is the long-context regime, and **it is the only arm the paper's O(*n*) claim is
/// actually about**. `outline.md` §5 says candidates evaluated is "≈ k (top-k retrieval) *or*
/// document count (full scan)" and then that blind cost grows without bound as N grows. Only
/// the second half of that disjunction does. Against top-*k*, blind cost is `k ×
/// tokens-per-candidate` — constant in N — so a benchmark that offered only the flat arm
/// would be reporting the `k` we chose.
///
/// Its recall is 1 by construction: everything is read, so nothing is missed. That is not a
/// win, it is the definition of the baseline — the arm pays for it in `candidates`, which is
/// N, and in `tokens`, which is the whole corpus. Precision is `|expect| / N`, which is the
/// quantity that decays as the corpus grows.
fn full_scan_arm(ids: &[String], expect: &[String], chars: &BTreeMap<String, usize>) -> ArmReport {
    ArmReport::scored("full-scan", ids.to_vec(), expect, chars)
}

/// Every local node's id, in corpus order, and the size of its instance text.
///
/// One pass, because both arms are charged the same per-node rate: they differ in which
/// nodes and how many, and nothing else. Charging them differently is how a benchmark
/// arrives at the answer it wanted.
fn corpus_text(state: &ServerState) -> (Vec<String>, BTreeMap<String, usize>) {
    let mut ids = Vec::new();
    let mut chars = BTreeMap::new();
    for node in state.nodes.iter().filter(|n| n.is_local()) {
        let id = format!("{}.yml", node.id);
        chars.insert(id.clone(), node.content.len());
        ids.push(id);
    }
    (ids, chars)
}

/// What the corpus is, measured rather than declared.
fn corpus_shape(state: &ServerState) -> CorpusShape {
    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for node in state.nodes.iter().filter(|n| n.is_local()) {
        *sizes.entry(node.class.as_str()).or_default() += 1;
    }
    let nodes = sizes.values().sum();
    let smallest = sizes.iter().min_by_key(|(name, size)| (**size, **name));
    let (smallest_class, smallest_class_size) = match smallest {
        Some((name, size)) => (name.to_string(), *size),
        None => (String::new(), 0),
    };
    let narrowing_ceiling = match smallest_class_size {
        0 => 0.0,
        size => nodes as f64 / size as f64,
    };
    CorpusShape {
        nodes,
        classes: sizes.len(),
        smallest_class,
        smallest_class_size,
        narrowing_ceiling,
        ceiling_reaches_claim: narrowing_ceiling >= CLAIMED_NARROWING_FLOOR,
    }
}

/// Why a run over this corpus is a regression guard, said in the report rather than only in
/// the goal set's header — the two must not be able to disagree.
fn standing_reason(shape: &CorpusShape) -> String {
    let base = "a single-corpus run is a regression guard, not evidence: it catches the \
                traversal path getting more expensive between commits and cannot separate the \
                arms on cost. Evidence comes from `bench --scaling` over generated corpora.";
    match shape.ceiling_reaches_claim {
        true => base.to_string(),
        false => format!(
            "{base} This corpus additionally cannot show the claimed effect at all: its \
             class-narrowing ceiling is {:.1}x ({} nodes / {} in `{}`, the smallest class), \
             against a claimed {CLAIMED_NARROWING_FLOOR:.0}-100x. That is unreachable \
             arithmetic, not an unmet target.",
            shape.narrowing_ceiling, shape.nodes, shape.smallest_class_size, shape.smallest_class
        ),
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    match values.is_empty() {
        true => None,
        false => Some(values.iter().sum::<f64>() / values.len() as f64),
    }
}

fn summarize(goals: &[GoalReport]) -> Summary {
    let compared: Vec<&GoalReport> = goals.iter().filter(|g| g.counts_toward_ratio).collect();
    let collect = |pick: fn(&GoalReport) -> &ArmReport, field: fn(&ArmReport) -> Option<f64>| {
        compared
            .iter()
            .filter_map(|g| field(pick(g)))
            .collect::<Vec<f64>>()
    };
    Summary {
        compared: compared.len(),
        reported_only: goals.len() - compared.len(),
        flat_mean_precision: mean(&collect(|g| &g.flat, |a| a.precision)),
        flat_mean_recall: mean(&collect(|g| &g.flat, |a| a.recall)),
        full_scan_mean_precision: mean(&collect(|g| &g.full_scan, |a| a.precision)),
        full_scan_mean_recall: mean(&collect(|g| &g.full_scan, |a| a.recall)),
        anchored_mean_precision: mean(&collect(|g| &g.anchored, |a| a.precision)),
        anchored_mean_recall: mean(&collect(|g| &g.anchored, |a| a.recall)),
    }
}

/// The anchored arm's standing until `yidam query` exists.
///
/// Reported as unavailable on every goal rather than quietly skipped: a benchmark that
/// prints one arm and says nothing about the other reads as a benchmark with one arm.
const ANCHORED_PENDING: &str =
    "the query executor does not exist yet (#261); no anchored result is available";

pub fn run(root: &std::path::Path, budget: usize) -> Result<BenchReport> {
    let goals_path = yidam_bench_dir(root).join(GOALS_FILE);
    if !goals_path.is_file() {
        bail!(
            "no goal set at {} — `bench` does not invent one, because a benchmark that \
             supplies its own goals measures whoever wrote the fallback",
            goals_path.display()
        );
    }
    let set = parse_goals(&std::fs::read_to_string(&goals_path)?)?;

    let state = ServerState::load(root)?;
    if let Some(reason) = state.retrieval.degraded_reason() {
        bail!(
            "the flat arm would be keyword search ({reason}), and beating keyword search \
             proves nothing about retrieval. Build the index and run a binary that can read \
             it: `yidam embed && yidam index-build`, with `--features index`."
        );
    }

    let shape = corpus_shape(&state);
    let (all_ids, chars) = corpus_text(&state);
    let goals = set
        .goals
        .iter()
        .map(|goal| {
            let expect: Vec<String> = goal.expect.iter().map(|e| expected_id(e)).collect();
            let flat = match &goal.flat {
                Some(query) => flat_arm(&state, query, budget, &expect, &chars),
                None => ArmReport::unavailable(
                    "flat",
                    goal.flat_omitted_because.clone().unwrap_or_default(),
                ),
            };
            let anchored = match &goal.anchored {
                Some(_) => ArmReport::unavailable("anchored", ANCHORED_PENDING),
                None => ArmReport::unavailable(
                    "anchored",
                    goal.anchored_omitted_because.clone().unwrap_or_default(),
                ),
            };
            // The full-scan arm has no query to omit: reading everything is available for
            // every goal, which is exactly why it is the baseline the claim is measured
            // against rather than a competitor.
            let full_scan = full_scan_arm(&all_ids, &expect, &chars);
            GoalReport {
                id: goal.id.clone(),
                question: goal.question.clone(),
                kind: goal.kind.clone(),
                hops: goal.hops,
                why: goal.why.clone(),
                counts_toward_ratio: goal.counts_toward_ratio(),
                expect,
                flat,
                full_scan,
                anchored,
            }
        })
        .collect::<Vec<_>>();

    let summary = summarize(&goals);
    Ok(BenchReport {
        goal_set: set.corpus.clone(),
        standing: "regression-guard",
        standing_reason: standing_reason(&shape),
        budget,
        corpus: shape,
        goals,
        summary,
    })
}

// ── rendering ─────────────────────────────────────────────────────────────────

fn pct(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{:.0}%", v * 100.0),
        None => "—".to_string(),
    }
}

/// A goal set's stated reason, on one line.
///
/// The reasons are authored as YAML folded scalars and arrive with their newlines intact.
/// They are prose worth reading in full, so this reflows rather than truncating — it only
/// stops a reason from breaking the two-lines-per-goal shape the rest of the report has.
pub(crate) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn render_arm(arm: &ArmReport) -> String {
    if !arm.ran {
        let reason = arm.unavailable_reason.as_deref().map(one_line);
        return format!(
            "    {:<10} not run — {}\n",
            arm.arm,
            match reason.as_deref() {
                Some("") | None => "no reason given",
                Some(text) => text,
            }
        );
    }
    format!(
        "    {:<10} precision {}  recall {}  read {} node(s) / {} token(s), {} correct\n",
        arm.arm,
        pct(arm.precision),
        pct(arm.recall),
        arm.nodes_read.unwrap_or(0),
        arm.tokens.unwrap_or(0),
        arm.hits.len(),
    )
}

pub fn render(report: &BenchReport) -> String {
    let c = &report.corpus;
    let mut out = format!(
        "{} node(s) across {} class(es) — class-narrowing ceiling {:.1}x \
         (smallest class `{}`, {} node(s))\nbudget k={}\n\n{}\n\n",
        c.nodes,
        c.classes,
        c.narrowing_ceiling,
        c.smallest_class,
        c.smallest_class_size,
        report.budget,
        report.standing_reason,
    );
    for goal in &report.goals {
        out.push_str(&format!(
            "  {}{}\n",
            goal.id,
            match goal.counts_toward_ratio {
                true => String::new(),
                false => "  (reported, not compared)".to_string(),
            }
        ));
        out.push_str(&render_arm(&goal.flat));
        out.push_str(&render_arm(&goal.full_scan));
        out.push_str(&render_arm(&goal.anchored));
    }
    let s = &report.summary;
    out.push_str(&format!(
        "\n{} goal(s) compared, {} reported only\n  flat       mean precision {}  mean recall {}\n  full-scan  mean precision {}  mean recall {}\n  anchored   mean precision {}  mean recall {}\n",
        s.compared,
        s.reported_only,
        pct(s.flat_mean_precision),
        pct(s.flat_mean_recall),
        pct(s.full_scan_mean_precision),
        pct(s.full_scan_mean_recall),
        pct(s.anchored_mean_precision),
        pct(s.anchored_mean_recall),
    ));
    out.trim_end().to_string()
}

/// Measure the goal set against every arm.
pub fn bench(budget: usize, scaling: bool, format: crate::report::Format) -> Result<()> {
    if scaling {
        let report = scaling::run()?;
        if format.is_json() {
            return crate::report::emit(&repo_root()?, report);
        }
        println!("{}", scaling::render(&report));
        return Ok(());
    }
    let root = repo_root()?;
    let report = run(&root, budget)?;
    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render(&report));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
version: 1
corpus: test
goals:
  - id: one
    question: q
    kind: traversal
    hops: 1
    anchored: 'reach -measured-by-> gage'
    flat: which gage
    expect:
      - gage/canyon-outlet.yml
"#;

    #[test]
    fn a_goal_set_parses_and_both_arms_count() {
        let set = parse_goals(MINIMAL).unwrap();
        assert_eq!(set.version, 1);
        assert_eq!(set.goals.len(), 1);
        assert!(set.goals[0].counts_toward_ratio());
    }

    /// Recall against an empty answer set is 0/0 for one arm and 1 for another depending on
    /// how you squint. A benchmark that scores it is publishing an artifact.
    #[test]
    fn a_goal_with_no_expected_answer_is_refused() {
        let text = MINIMAL.replace("      - gage/canyon-outlet.yml\n", "");
        let err = parse_goals(&text).unwrap_err().to_string();
        assert!(err.contains("empty `expect`"), "{err}");
    }

    #[test]
    fn a_duplicate_goal_id_is_refused() {
        let doubled = format!("{MINIMAL}{}", MINIMAL.split_once("goals:\n").unwrap().1);
        let err = parse_goals(&doubled).unwrap_err().to_string();
        assert!(err.contains("appears twice"), "{err}");
    }

    /// An arm dropped without a stated reason is indistinguishable in the report from an arm
    /// that ran and lost, which is the direction the whole file is arranged against.
    #[test]
    fn an_omitted_arm_must_say_why() {
        let text = MINIMAL.replace("    flat: which gage\n", "");
        let err = parse_goals(&text).unwrap_err().to_string();
        assert!(err.contains("flat_omitted_because"), "{err}");

        let text = MINIMAL.replace("    anchored: 'reach -measured-by-> gage'\n", "");
        let err = parse_goals(&text).unwrap_err().to_string();
        assert!(err.contains("anchored_omitted_because"), "{err}");
    }

    #[test]
    fn a_goal_only_one_arm_can_express_is_reported_and_not_compared() {
        let text = MINIMAL.replace(
            "    flat: which gage\n",
            "    flat: null\n    flat_omitted_because: retrieval cannot filter on standing\n",
        );
        let set = parse_goals(&text).unwrap();
        assert!(!set.goals[0].counts_toward_ratio());
    }

    // ── identity ──────────────────────────────────────────────────────────────

    #[test]
    fn both_retrieval_bodies_report_paths_this_reduces_to_a_node_id() {
        assert_eq!(
            node_id(".yidam/corpus/gage/canyon-outlet.yml"),
            "gage/canyon-outlet.yml"
        );
        assert_eq!(
            node_id(".yidam\\corpus\\gage\\canyon-outlet.yml"),
            "gage/canyon-outlet.yml"
        );
    }

    /// A catalog entry and a dependency's node are both indexed and both retrievable.
    /// Neither is a node of this corpus, so neither may quietly acquire an id that could
    /// match an `expect` entry — they score as misses, which is what they are.
    #[test]
    fn a_result_outside_the_corpus_keeps_its_path_and_cannot_match() {
        assert_eq!(
            node_id(".yidam/catalog/usgs-nwis.md"),
            ".yidam/catalog/usgs-nwis.md"
        );
        let dep = node_id(".yidam/tonpa/other/corpus/gage/canyon-outlet.yml");
        assert_ne!(dep, "gage/canyon-outlet.yml");
    }

    #[test]
    fn an_expect_entry_is_read_however_it_was_written() {
        for written in [
            "gage/canyon-outlet.yml",
            "gage/canyon-outlet",
            "/gage/canyon-outlet.yml",
            ".yidam/corpus/gage/canyon-outlet.yml",
            "  gage/canyon-outlet.yml  ",
        ] {
            assert_eq!(expected_id(written), "gage/canyon-outlet.yml", "{written}");
        }
    }

    // ── scoring ───────────────────────────────────────────────────────────────

    /// Four nodes of 40 characters each, so token arithmetic in these tests is checkable by
    /// hand: 40 chars is 10 tokens under the `chars / 4` approximation.
    fn chars_fixture() -> BTreeMap<String, usize> {
        ["a/one.yml", "a/two.yml", "b/three.yml", "b/four.yml"]
            .into_iter()
            .map(|id| (id.to_string(), 40usize))
            .collect()
    }

    #[test]
    fn precision_and_recall_are_computed_against_the_exhaustive_expected_set() {
        let expect = vec!["a/one.yml".to_string(), "a/two.yml".to_string()];
        let arm = ArmReport::scored(
            "flat",
            vec![
                "a/one.yml".to_string(),
                "b/three.yml".to_string(),
                "b/four.yml".to_string(),
            ],
            &expect,
            &chars_fixture(),
        );
        assert_eq!(arm.hits, vec!["a/one.yml"]);
        assert_eq!(arm.precision, Some(1.0 / 3.0));
        assert_eq!(arm.recall, Some(0.5));
        assert_eq!(arm.nodes_read, Some(3));
        assert_eq!(arm.candidates, Some(3));
        // Three nodes of 40 characters, charged at the one rate both arms pay.
        assert_eq!(arm.tokens, Some(30));
    }

    /// The full-scan arm's recall is 1 by construction — it reads everything, so it misses
    /// nothing. That is the definition of the baseline, not a win, and it pays for it in
    /// candidates and tokens. Precision is `|expect| / N`, the quantity that decays as the
    /// corpus grows, and the reason O(*n*) lives on this arm and not on top-*k*.
    #[test]
    fn the_full_scan_arm_has_perfect_recall_and_pays_the_whole_corpus_for_it() {
        let chars = chars_fixture();
        let all: Vec<String> = chars.keys().cloned().collect();
        let expect = vec!["a/one.yml".to_string()];
        let arm = full_scan_arm(&all, &expect, &chars);
        assert_eq!(arm.recall, Some(1.0));
        assert_eq!(arm.precision, Some(0.25));
        assert_eq!(arm.candidates, Some(4));
        assert_eq!(arm.tokens, Some(40));
    }

    /// Precision's denominator is the candidate set, not what came back. Scoring the
    /// full-scan arm on what it "returned" would hand it perfect precision for reading the
    /// entire corpus, which is the exact inversion of what it costs.
    #[test]
    fn precision_is_scored_on_candidates_rather_than_on_the_listed_result() {
        let chars: BTreeMap<String, usize> =
            (0..50).map(|i| (format!("a/n{i}.yml"), 4usize)).collect();
        let all: Vec<String> = chars.keys().cloned().collect();
        let expect = vec!["a/n0.yml".to_string()];
        let arm = full_scan_arm(&all, &expect, &chars);
        assert_eq!(arm.candidates, Some(50));
        assert_eq!(arm.precision, Some(1.0 / 50.0));
        assert_eq!(
            arm.returned.len(),
            RETURNED_SHOWN,
            "the listing is capped; the score is not"
        );
    }

    /// 0/0 is undefined, and recording it as zero drags the mean down on a goal where the
    /// arm made no claim at all.
    #[test]
    fn an_arm_that_returned_nothing_has_no_precision_rather_than_zero() {
        let expect = vec!["a/one.yml".to_string()];
        let arm = ArmReport::scored("flat", vec![], &expect, &chars_fixture());
        assert_eq!(arm.precision, None);
        assert_eq!(arm.recall, Some(0.0));
    }

    // ── corpus shape ──────────────────────────────────────────────────────────

    /// The numbers #264's decision 3 requires beside every ratio, on the corpus that
    /// prompted the requirement: 8 nodes, 3 classes, smallest class 2, ceiling 4x against a
    /// claimed 10-100x.
    #[test]
    fn the_streamflow_ceiling_is_reported_as_unreachable() {
        let shape = CorpusShape {
            nodes: 8,
            classes: 3,
            smallest_class: "gage".to_string(),
            smallest_class_size: 2,
            narrowing_ceiling: 4.0,
            ceiling_reaches_claim: false,
        };
        let reason = standing_reason(&shape);
        assert!(reason.contains("regression guard"), "{reason}");
        assert!(reason.contains("4.0x"), "{reason}");
        assert!(reason.contains("unreachable arithmetic"), "{reason}");
    }

    /// A corpus that *can* show the effect still only gets a regression guard from a
    /// single-corpus run — the ceiling and the standing are two different facts.
    #[test]
    fn clearing_the_ceiling_does_not_promote_a_single_corpus_run_to_evidence() {
        let shape = CorpusShape {
            nodes: 102,
            classes: 12,
            smallest_class: "boundary-case".to_string(),
            smallest_class_size: 1,
            narrowing_ceiling: 102.0,
            ceiling_reaches_claim: true,
        };
        let reason = standing_reason(&shape);
        assert!(reason.contains("regression guard"), "{reason}");
        assert!(!reason.contains("unreachable"), "{reason}");
    }

    // ── summary ───────────────────────────────────────────────────────────────

    fn goal_report(id: &str, counts: bool, flat_precision: Option<f64>) -> GoalReport {
        GoalReport {
            id: id.to_string(),
            question: String::new(),
            kind: "traversal".to_string(),
            hops: Some(1),
            why: String::new(),
            expect: vec!["a/one.yml".to_string()],
            counts_toward_ratio: counts,
            flat: ArmReport {
                arm: "flat",
                ran: flat_precision.is_some(),
                unavailable_reason: None,
                returned: vec![],
                hits: vec![],
                candidates: Some(0),
                precision: flat_precision,
                recall: flat_precision,
                nodes_read: Some(0),
                tokens: Some(0),
            },
            full_scan: ArmReport::scored(
                "full-scan",
                vec!["a/one.yml".to_string()],
                &["a/one.yml".to_string()],
                &chars_fixture(),
            ),
            anchored: ArmReport::unavailable("anchored", ANCHORED_PENDING),
        }
    }

    /// The rule the goal set states and this has to enforce: a goal only one arm can
    /// express is reported and kept out of the mean.
    #[test]
    fn the_mean_excludes_goals_only_one_arm_can_express() {
        let goals = vec![
            goal_report("compared", true, Some(1.0)),
            goal_report("uncomparable", false, Some(0.0)),
        ];
        let summary = summarize(&goals);
        assert_eq!(summary.compared, 1);
        assert_eq!(summary.reported_only, 1);
        assert_eq!(summary.flat_mean_precision, Some(1.0));
    }

    #[test]
    fn the_anchored_arm_reports_its_absence_rather_than_scoring_zero() {
        let goals = vec![goal_report("one", true, Some(1.0))];
        let summary = summarize(&goals);
        assert_eq!(summary.anchored_mean_precision, None);
        assert!(!goals[0].anchored.ran);
        assert_eq!(
            goals[0].anchored.unavailable_reason.as_deref(),
            Some(ANCHORED_PENDING)
        );
    }

    #[test]
    fn the_text_report_states_the_standing_and_the_ceiling() {
        let report = BenchReport {
            goal_set: "test".to_string(),
            standing: "regression-guard",
            standing_reason: standing_reason(&CorpusShape {
                nodes: 8,
                classes: 3,
                smallest_class: "gage".to_string(),
                smallest_class_size: 2,
                narrowing_ceiling: 4.0,
                ceiling_reaches_claim: false,
            }),
            budget: 5,
            corpus: CorpusShape {
                nodes: 8,
                classes: 3,
                smallest_class: "gage".to_string(),
                smallest_class_size: 2,
                narrowing_ceiling: 4.0,
                ceiling_reaches_claim: false,
            },
            goals: vec![goal_report("one", true, Some(1.0))],
            summary: Summary {
                compared: 1,
                reported_only: 0,
                flat_mean_precision: Some(1.0),
                flat_mean_recall: Some(1.0),
                full_scan_mean_precision: Some(1.0),
                full_scan_mean_recall: Some(1.0),
                anchored_mean_precision: None,
                anchored_mean_recall: None,
            },
        };
        let text = render(&report);
        assert!(text.contains("ceiling 4.0x"), "{text}");
        assert!(text.contains("regression guard"), "{text}");
        assert!(text.contains("budget k=5"), "{text}");
        assert!(text.contains("not run"), "{text}");
    }
}
