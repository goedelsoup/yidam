//! `.yidam/capabilities.toml` — what may run, what it may read, and what it may write.
//!
//! RFC-0026 §4 specifies the file and RFC-0028 §4 constrains it. Both constraints are
//! checked here, at load, rather than after a step has already produced a tree: *"`writes` is
//! load-bearing rather than documentation. It is what lets the executor refuse a step that
//! wrote outside its declaration, and what makes the operational/epistemic classification
//! decidable **before** the step runs rather than after."*
//!
//! # Rust only
//!
//! Not a parity function, and that is decided rather than deferred — #460 decision 3,
//! reversed 2026-08-31 and restated in RFC-0026 §4. No `.yidam/` config file has ever been
//! one; the ten are document and graph parsers. A fixture directory here with no runner
//! reading it is the shape `parity-check` refuses.
//!
//! # Unknown fields are refused
//!
//! `deny_unknown_fields`, and the reason is not tidiness. #472 added the declared dependencies
//! and the ageing rule this file now carries. A binary that silently ignored an `after = [...]`
//! it did not implement would run a dependent step before the step it depends on and report
//! success — the failure would be in the corpus, not in the exit code. Refusing to parse says
//! which field and which binary, which is recoverable.
//!
//! # The whole file is validated, not the entry being asked for
//!
//! [`Manifest::parse`] checks every declaration and then the graph they form, so a manifest
//! holding a cycle or a dangling `after` does not load **at all** — not even to run a step that
//! is nowhere near the defect. The alternative, checking a step's own closure when it is asked
//! for, would let a corpus carry a broken manifest indefinitely as long as nobody ran the step
//! that touched the broken part, which is the shape of a gate that is green because it is not
//! looking.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::kuten::{Register, Registers};

/// Where the manifest lives, relative to the corpus root.
pub const MANIFEST: &str = ".yidam/capabilities.toml";

/// What a capability is — the three the published taxonomy names, and no fourth.
///
/// `guidelines/directories.md` and `docs/domain-computer.md` both type the domain computer as
/// three kinds. For as long as this enum had two, the third was a vocabulary a corpus could
/// read and could not write. `cmd/build.rs` exists because a crate index that cannot say which
/// capability a crate implements "is describing the scaffold rather than the thing it scaffolds",
/// and it reads that from the manifest — so a crate implementing feature engineering had two
/// options, an em dash forever or a declaration that said `calculator` and was false.
///
/// All three are declarable and only one is executable — see [`Kind::unrunnable_because`], which
/// answers both halves at once. That split
/// is `Connector`'s precedent, and its doc comment states the reason it is the right shape:
/// *"a refusal naming the issue is a better answer than a manifest that cannot express the
/// other kind at all."* A corpus that declares a featurizer gets a manifest that parses, a row
/// in the crate index that names the capability, and a step that refuses by name.
///
/// # The spelling is frozen
///
/// `kind = "featurizer"` — the agent noun, which is the form the other two arms already take, so
/// all three inherit `rename_all` and no arm is a special case. This is a value a corpus commits,
/// so the alternatives are worth recording as rejected rather than unconsidered. `"feature"` and
/// `"features"` name the output rather than the thing that produces it, and a manifest entry is a
/// thing that runs. `"feature-engineering"` is the taxonomy's own words and was the first choice
/// for that reason; what it costs is a value that no longer lowercases from its own variant, an
/// arm carrying an explicit rename, and a hyphen a corpus author has to remember in a field where
/// the neighbouring values have none.
///
/// What that choice costs instead is a value a reader cannot derive from the document: the
/// taxonomy's section is headed *Feature engineering* and the value is `featurizer`. Nothing in
/// the document bridges that on its own, so `guidelines/directories.md` names the value beside the
/// definition — the same sentence a corpus author is reading when they need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Connector,
    Calculator,
    Featurizer,
}

impl Kind {
    /// Every kind, so a test that must cover the taxonomy is held to the taxonomy rather than to
    /// a list somebody remembered to extend — the guard-list shape #448 found, and the shape the
    /// two-arm enum #1027 reported was an instance of.
    ///
    /// `#[cfg(test)]` because nothing in the binary enumerates the kinds: a declaration names
    /// one, and serde's own "unknown variant" message is what lists them to a corpus that named
    /// none of them. A `pub const` no shipped path reads would be a surface with no consumer,
    /// which `-D warnings` says out loud and this repository has filed against itself before.
    #[cfg(test)]
    pub const ALL: [Self; 3] = [Self::Connector, Self::Calculator, Self::Featurizer];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Calculator => "calculator",
            Self::Featurizer => "featurizer",
        }
    }

    /// Why this binary does not invoke it — `None` for the one it does.
    ///
    /// This is also the predicate: `None` *is* "this binary invokes it", and #471 is the
    /// calculator slice that makes that one kind. A separate `executable()` alongside it read
    /// well and could disagree with it, and the caller that wants the reason wants the predicate
    /// in the same breath — [`super::plan_and_write`] collects each refused step with the
    /// sentence that refused it, so the two cannot come apart and neither can be `None` where
    /// the other says it should not be.
    ///
    /// Per kind rather than one sentence, because the two unrunnable kinds are unrunnable for
    /// unrelated reasons, and the reasons are not even the same *sort* of reason. A connector's
    /// is a decision: it reaches a network and a credential path, and `vault/mod.rs` already set
    /// the rule those inherit. A featurizer's is an absence: nobody has built the slice. Saying
    /// so is the point — a refusal that gave the connector's sentence for a featurizer would
    /// send its author to read about credentials it does not need.
    ///
    /// The one thing the two share is the shape of the answer: a named refusal, not a shell
    /// failure.
    pub fn unrunnable_because(self) -> Option<&'static str> {
        match self {
            Self::Calculator => None,
            Self::Connector => Some(
                "A connector re-runs against an external source, which is a network capability \
                 and a credential path, and `vault/mod.rs` sets the rule those inherit",
            ),
            Self::Featurizer => Some(
                "Feature engineering is declarable and nothing invokes it yet: #471 built the \
                 calculator slice, and epic #1025 records the third kind as having no executor at \
                 all, so there is nothing for this manifest to withhold a permission from",
            ),
        }
    }
}

/// Where a run's commit goes, which is a function of the verb and of nothing else.
///
/// **This is the second structural route, not a permission that can be granted.** RFC-0026 §2
/// states the invariant in one sentence — *"A run authors operational commits directly. Every
/// epistemic commit it produces goes to a proposal branch, and nothing merges itself."* — and
/// for as long as only the first half existed, the second was enforced by refusing the verb.
/// That was the right holding action and it made the whole epistemic half of the vocabulary
/// undeclarable: `yidam run` could compute a number and could not open a question about one.
///
/// What replaces the refusal has to be at least as strong, and the property to preserve is
/// exact: **there is no path, and no config value, by which a run advances the current branch
/// with an epistemic commit.** So there is no field. `route` takes `&self` and reads one thing,
/// `verb`, through the parity function that already decides the families. A manifest cannot
/// declare a route, a policy cannot override one, and the two facts a reader might otherwise
/// have to reconcile — the verb and the destination — are one fact.
///
/// The temptation this forecloses, stated so the next reader does not have to rediscover it: a
/// `route = "branch"` field, or an `allow_direct = true`, would make the safety argument a
/// config value, which is exactly the contradiction RFC-0024 named and RFC-0026 §3.1 answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Advances the branch the run was invoked from. Operational verbs only.
    Branch,
    /// Lands on `propose/<head>`, leaving the current branch where it was. A person merges.
    Proposal,
}

impl Route {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Proposal => "proposal",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub kind: Kind,
    /// argv, invoked directly — not through a shell. A corpus that wants a shell says `sh`.
    pub run: Vec<String>,
    /// Globs, relative to the corpus root. Exactly what the step is given, and nothing else
    /// from the repository reaches it.
    #[serde(default)]
    pub reads: Vec<String>,
    /// Globs, relative to the corpus root. What the step may land.
    pub writes: Vec<String>,
    /// The commit verb a run of this capability authors.
    pub verb: String,
    /// Steps that must be up to date before this one is invoked.
    ///
    /// Named `after` rather than `depends_on` because that is what it means operationally:
    /// this step is invoked after those, against the commit they landed. A dependency here is
    /// not advisory — [`Manifest::plan`] resolves the transitive closure and the executor runs
    /// it, so a step declaring one is never invoked against an upstream that had not run.
    #[serde(default)]
    pub after: Vec<String>,
    /// How many days this step's result stands before it is re-run, however little moved.
    ///
    /// **The interval is declared here and is never compiled in.** That is
    /// [`crate::config::LintConfig::escalate_after`]'s argument, which `cmd/due.rs` repeats for
    /// all four of its clocks: a number in the binary is one corpus's judgement arriving in
    /// another that never agreed to it. Absent by default, and a step with no ageing rule is
    /// decided by its input state alone.
    ///
    /// What it is *for* is the case an input state cannot see. A calculator is a function of
    /// what it reads, so an unchanged corpus computes an unchanged answer and re-running it
    /// says nothing. A connector reads a world that is not in the repository, and its input
    /// state can sit unchanged across a year in which everything it describes moved. This is
    /// the catalog clock's distinction exactly — *"An expiry does not claim the upstream
    /// changed. It claims nobody has looked."*
    #[serde(default)]
    pub ageing_days: Option<u32>,
}

impl Capability {
    /// Where a commit this capability authors goes — see [`Route`].
    ///
    /// Total on the verb, and the verb is closed by [`validate`], so there is no third answer
    /// and no unreachable arm to get wrong. `classify_commit` is the parity function that draws
    /// the same line over a subject line; this draws it over the declaration, before anything
    /// has been written, which is what RFC-0026 §4 means by the classification being decidable
    /// before the step runs.
    pub fn route(&self) -> Route {
        match yidam_core::git::OPERATIONAL_VERBS.contains(&self.verb.as_str()) {
            true => Route::Branch,
            false => Route::Proposal,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub capability: BTreeMap<String, Capability>,
}

impl Manifest {
    /// Parse and validate the manifest at `root`, or report that there is none.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no capability manifest at {MANIFEST} — a run is one declared capability \
                 invoked, and nothing here declares one"
            )
        })?;
        Self::parse(&text, &Registers::of_repo(root))
    }

    /// The same, against a given object register, so the rules can be tested without a repo.
    pub fn parse(text: &str, registers: &Registers) -> Result<Self> {
        let m: Self = toml::from_str(text).context("parsing .yidam/capabilities.toml")?;
        for (name, cap) in &m.capability {
            validate(name, cap, registers)?;
        }
        m.check_graph()?;
        Ok(m)
    }

    pub fn get<'a>(&'a self, step: &str) -> Result<&'a Capability> {
        self.capability.get(step).ok_or_else(|| self.no_such(step))
    }

    /// What to say about a step this manifest does not declare.
    ///
    /// One sentence for two callers — [`Self::get`] and [`Self::plan`] — because the second
    /// reaches it through an `after` naming nothing, and a reader who mistyped a dependency
    /// needs exactly what a reader who mistyped an argument needs: the list.
    fn no_such(&self, step: &str) -> anyhow::Error {
        let declared: Vec<&str> = self.capability.keys().map(String::as_str).collect();
        if declared.is_empty() {
            anyhow::anyhow!("{MANIFEST} declares no capabilities, so `{step}` is not one")
        } else {
            anyhow::anyhow!(
                "no capability named `{step}` — {MANIFEST} declares: {}",
                declared.join(", ")
            )
        }
    }

    /// The steps to run, every dependency before the step that declares it.
    ///
    /// `Some(step)` plans that step's transitive `after` closure and nothing else; `None` plans
    /// the whole manifest. A step is in the plan exactly once however many dependents name it.
    ///
    /// **The order is total, not merely valid.** Independent steps are visited in the
    /// manifest's own key order, which `BTreeMap` makes the file's alphabetical order rather
    /// than its line order — so the same manifest plans the same sequence on every machine and
    /// in every process. A planner that was free between independent steps would make a run's
    /// commit sequence unreproducible, and the input state exists to make a run's result an
    /// equality check.
    pub fn plan(&self, step: Option<&str>) -> Result<Vec<&str>> {
        let roots: Vec<&str> = match step {
            Some(s) => vec![self.key(s)?],
            None => self.capability.keys().map(String::as_str).collect(),
        };
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        let mut path = Vec::new();
        for root in roots {
            self.visit(root, &mut order, &mut done, &mut path)?;
        }
        Ok(order)
    }

    /// The manifest's own copy of a step's name, so a plan borrows from the map rather than
    /// from the argument that asked for it.
    fn key(&self, step: &str) -> Result<&str> {
        match self.capability.get_key_value(step) {
            Some((name, _)) => Ok(name.as_str()),
            None => Err(self.no_such(step)),
        }
    }

    /// Depth-first, dependencies first, with the stack kept so a cycle can be named.
    ///
    /// `path` is the chain currently being resolved and `done` is everything already ordered.
    /// Meeting a step that is on `path` is a cycle; meeting one in `done` is a diamond, which
    /// is ordinary and is why the two sets are not one.
    fn visit<'a>(
        &'a self,
        step: &'a str,
        order: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
        path: &mut Vec<&'a str>,
    ) -> Result<()> {
        if done.contains(step) {
            return Ok(());
        }
        if let Some(at) = path.iter().position(|s| *s == step) {
            let cycle: Vec<&str> = path[at..]
                .iter()
                .copied()
                .chain(std::iter::once(step))
                .collect();
            bail!(
                "`after` forms a cycle, so there is no order to run these in:\n  {}\n  \
                 Every step in it waits for one that is waiting for it.",
                cycle.join(" → ")
            );
        }
        let cap = self.get(step)?;
        path.push(step);
        for dep in &cap.after {
            self.visit(self.key(dep)?, order, done, path)?;
        }
        path.pop();
        done.insert(step);
        order.push(step);
        Ok(())
    }

    /// Every rule about the graph the declarations form, rather than about one of them.
    ///
    /// Checked over the whole manifest at load — see the module doc. The cycle check is
    /// [`Self::plan`] itself rather than a second traversal agreeing with it: a manifest that
    /// loaded and then could not be planned would be a file this module called valid and the
    /// executor could not use.
    fn check_graph(&self) -> Result<()> {
        for (name, cap) in &self.capability {
            for dep in &cap.after {
                let upstream = self.capability.get(dep).ok_or_else(|| self.no_such(dep))?;
                // The rule that is not about the graph's shape but about what a run lands.
                // An epistemic step's output goes to `propose/<head>` and the branch does not
                // move, so it is not in the tree a dependent would be materialized from — and
                // waiting for it would mean waiting for a person to merge, which is an act no
                // plan can contain.
                if upstream.route() == Route::Proposal {
                    bail!(
                        "capability `{name}` declares `after = [\"{dep}\"]`, and `{dep}` \
                         authors `{}`, which is epistemic.\n  \
                         An epistemic run lands on `propose/<head>` and leaves the branch where \
                         it was, so what `{dep}` writes is not in the tree `{name}` would be \
                         materialized from. A step cannot wait for output a run does not land, \
                         and nothing merges itself.",
                        upstream.verb
                    );
                }
            }
        }
        self.plan(None).map(|_| ())
    }
}

/// Every rule a declaration must satisfy, checked before anything runs.
fn validate(name: &str, cap: &Capability, registers: &Registers) -> Result<()> {
    if cap.run.is_empty() {
        bail!("capability `{name}` declares an empty `run`, so there is nothing to invoke");
    }

    // The closed vocabulary, and nothing outside it. Which *family* the verb falls in is not
    // checked here any more and is not a permission — see [`Capability::route`], which is the
    // whole of what an epistemic verb buys and the reason it no longer needs refusing.
    if !yidam_core::git::is_recognized_verb(&cap.verb) {
        bail!(
            "capability `{name}` declares `verb = \"{}\"`, which is not in the commit \
             vocabulary at all.\n  \
             Operational verbs: {}\n  \
             Epistemic verbs: {}",
            cap.verb,
            yidam_core::git::OPERATIONAL_VERBS.join(", "),
            yidam_core::git::EPISTEMIC_VERBS.join(", ")
        );
    }

    if cap.writes.is_empty() {
        bail!(
            "capability `{name}` declares no `writes` — a step whose outputs are undeclared \
             cannot be refused for writing outside them"
        );
    }
    for glob in cap.writes.iter().chain(cap.reads.iter()) {
        check_glob(name, glob)?;
    }
    // RFC-0028 §4 arm (b): the invariant test's population is kept register-pure by refusing
    // a declaration that reaches the object register, checked at declaration time.
    for glob in &cap.writes {
        let prefix = literal_prefix(glob);
        if registers.register_of(&prefix) == Register::Object {
            bail!(
                "capability `{name}` declares `writes = [\"{glob}\"]`, which lies in this \
                 repository's object register (`[object] paths` in .yidam/config.toml).\n  \
                 A run writes corpus commits; the artifact register is where a person's \
                 `feat:` and `fix:` live, and the commit vocabulary does not govern it."
            );
        }
    }
    Ok(())
}

/// A glob must be repository-relative, must escape nothing, and must start with a literal.
///
/// The last rule is what makes the register check above decidable at declaration time: a
/// pattern beginning `**/` names a prefix only once a file exists to match it, and a rule
/// checked "before the step runs" cannot be written against a set that does not exist yet.
fn check_glob(name: &str, glob: &str) -> Result<()> {
    let g = glob.trim();
    if g.is_empty() {
        bail!("capability `{name}` declares an empty glob");
    }
    if g.starts_with('/') || g.starts_with("~/") {
        bail!("capability `{name}` declares an absolute glob `{glob}` — paths are relative to the corpus root");
    }
    if g.split('/').any(|s| s == "..") {
        bail!("capability `{name}` declares `{glob}`, which leaves the corpus");
    }
    if literal_prefix(g).is_empty() {
        bail!(
            "capability `{name}` declares `{glob}`, whose first segment is a wildcard — a \
             glob must begin with a literal directory so which register it falls in is \
             decidable before the step runs"
        );
    }
    Ok(())
}

/// The segments of a glob before the first wildcard, joined.
pub(crate) fn literal_prefix(glob: &str) -> String {
    glob.trim_start_matches("./")
        .split('/')
        .take_while(|s| !s.contains('*') && !s.contains('?'))
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    const CALC: &str = r#"
[capability.low-flow]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/low-flow.sh"]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "compute"
"#;

    fn corpus_only() -> Registers {
        Registers::corpus_only()
    }

    #[test]
    fn a_calculator_declaration_parses() {
        let m = Manifest::parse(CALC, &corpus_only()).unwrap();
        let c = m.get("low-flow").unwrap();
        assert_eq!(c.kind, Kind::Calculator);
        assert_eq!(c.verb, "compute");
        assert!(c.kind.unrunnable_because().is_none());
    }

    /// Every kind the taxonomy names parses, and each is spelled the way `as_str` says it is.
    ///
    /// Driven from [`Kind::ALL`] rather than from three literals, so a fourth arm is covered the
    /// day it is added instead of the day somebody remembers this test. The round trip is the
    /// assertion that matters: `as_str` is what the crate index prints and what a receipt
    /// carries, and serde is what a corpus's manifest is read with, so the two disagreeing
    /// would mean a corpus could declare a value no report could name.
    #[test]
    fn every_kind_parses_under_its_own_spelling() {
        for kind in Kind::ALL {
            let text = CALC.replace(
                r#"kind   = "calculator""#,
                &format!(r#"kind   = "{}""#, kind.as_str()),
            );
            assert!(
                text != CALC || kind == Kind::Calculator,
                "`{}` did not substitute, so this asserts nothing",
                kind.as_str()
            );
            let m = Manifest::parse(&text, &corpus_only())
                .unwrap_or_else(|e| panic!("`{}` does not parse: {e}", kind.as_str()));
            assert_eq!(m.get("low-flow").unwrap().kind, kind);
        }
    }

    /// Feature engineering is declarable, it is not executable, and it says which it is.
    ///
    /// The whole of #1027: the arm parses, so a corpus can write the third kind the published
    /// taxonomy names and `crates-index` can name it; and it refuses by name rather than
    /// failing in a shell, which is `Connector`'s precedent applied unchanged.
    #[test]
    fn a_featurizer_is_declarable_and_not_executable() {
        let text = CALC.replace(r#"kind   = "calculator""#, r#"kind   = "featurizer""#);
        let m = Manifest::parse(&text, &corpus_only()).unwrap();
        let c = m.get("low-flow").unwrap();
        assert_eq!(c.kind, Kind::Featurizer);
        assert!(c.kind.unrunnable_because().is_some());
        // The kind does not touch the route. A featurizer declaring `compute` advances the
        // branch exactly as a calculator does, because `route` reads the verb and nothing else
        // — see [`routes_are_a_total_function_of_the_verb`].
        assert_eq!(c.route(), Route::Branch);
    }

    /// The frozen spelling, asserted as the literal a corpus commits.
    ///
    /// `Kind::ALL` covers the round trip; this covers the *choice*. A rename here is a migration
    /// of every committed manifest, so it should cost a deliberate edit to this line rather than
    /// riding along with a refactor of the variant's name.
    #[test]
    fn the_third_kind_is_spelled_featurizer() {
        assert_eq!(Kind::Featurizer.as_str(), "featurizer");
        for rejected in [
            "feature",
            "features",
            "feature-engineering",
            "featureengineering",
        ] {
            let text = CALC.replace(
                r#"kind   = "calculator""#,
                &format!(r#"kind   = "{rejected}""#),
            );
            assert!(
                Manifest::parse(&text, &corpus_only()).is_err(),
                "`{rejected}` parses, so the spelling is not frozen"
            );
        }
    }

    /// Every kind this binary cannot invoke carries its own reason, and exactly one carries none.
    ///
    /// Two assertions in one because the defect they are against is one defect: a refusal that
    /// hands a featurizer's author the connector's sentence about credentials. Distinctness is
    /// asserted per kind rather than as a count of reasons, because two kinds sharing a sentence
    /// and a third going quiet both leave the same total.
    #[test]
    fn each_unrunnable_kind_has_its_own_reason() {
        let mut reasons: Vec<&str> = Vec::new();
        for kind in Kind::ALL {
            if let Some(why) = kind.unrunnable_because() {
                assert!(
                    !reasons.contains(&why),
                    "`{}` repeats another kind's reason",
                    kind.as_str()
                );
                reasons.push(why);
            }
        }
        assert_eq!(
            reasons.len() + 1,
            Kind::ALL.len(),
            "more or fewer than one kind is invoked, so the refusal pre-pass is not the whole rule"
        );
    }

    /// An epistemic verb loads, and what it buys is a destination rather than a permission.
    ///
    /// This replaces `an_epistemic_verb_is_refused_by_name`, which asserted the holding action.
    /// The invariant it was protecting is now [`routes_are_a_total_function_of_the_verb`] and
    /// the executor test that no epistemic run touches the branch.
    #[test]
    fn an_epistemic_verb_loads_and_routes_to_a_proposal() {
        let text = CALC.replace(r#"verb   = "compute""#, r#"verb   = "establish""#);
        let m = Manifest::parse(&text, &corpus_only()).unwrap();
        let c = m.get("low-flow").unwrap();
        assert_eq!(c.verb, "establish");
        assert_eq!(c.route(), Route::Proposal);
    }

    /// The property that replaced the refusal: every verb the vocabulary admits has exactly one
    /// destination, it is decided by the verb, and the two families do not overlap.
    ///
    /// Written over the whole vocabulary rather than over a sample, because the claim is about
    /// the partition and a sample would be about two strings.
    #[test]
    fn routes_are_a_total_function_of_the_verb() {
        let cap = |verb: &str| Capability {
            kind: Kind::Calculator,
            run: vec!["true".into()],
            reads: vec![],
            writes: vec![".yidam/computed/**".into()],
            verb: verb.to_string(),
            after: vec![],
            ageing_days: None,
        };
        for verb in yidam_core::git::OPERATIONAL_VERBS {
            assert_eq!(cap(verb).route(), Route::Branch, "{verb}");
        }
        for verb in yidam_core::git::EPISTEMIC_VERBS {
            assert_eq!(cap(verb).route(), Route::Proposal, "{verb}");
            assert!(
                !yidam_core::git::OPERATIONAL_VERBS.contains(verb),
                "{verb} is in both families, so `route` is deciding by which list was consulted \
                 first rather than by the vocabulary"
            );
        }
    }

    /// A verb in neither family is still refused as such, and the message now names both.
    #[test]
    fn a_verb_outside_the_vocabulary_is_refused_as_such() {
        let text = CALC.replace(r#"verb   = "compute""#, r#"verb   = "lift""#);
        let err = Manifest::parse(&text, &corpus_only())
            .unwrap_err()
            .to_string();
        assert!(err.contains("not in the commit vocabulary"), "{err}");
        assert!(err.contains("establish"), "{err}");
    }

    /// The field that must never exist. A manifest cannot say where its output goes, and
    /// `deny_unknown_fields` is what makes an attempt to say so a parse error rather than a
    /// line somebody reads as effective.
    #[test]
    fn a_manifest_cannot_declare_a_route() {
        for line in [
            "route  = \"branch\"\n",
            "allow_direct = true\n",
            "epistemic = false\n",
        ] {
            let text = format!("{CALC}{line}");
            assert!(
                Manifest::parse(&text, &corpus_only()).is_err(),
                "`{}` was accepted, so a corpus can state a destination the executor does not \
                 read — which is the shape of a permission",
                line.trim()
            );
        }
    }

    /// RFC-0028 §4 arm (b), at declaration time.
    #[test]
    fn a_write_into_the_object_register_is_refused() {
        let text = CALC.replace(
            r#"writes = [".yidam/computed/**"]"#,
            r#"writes = ["web/data/**"]"#,
        );
        let registers = Registers::of_globs(vec!["web/**".into()]);
        let err = Manifest::parse(&text, &registers).unwrap_err().to_string();
        assert!(err.contains("object register"), "{err}");
        // And the same declaration is fine in a repository that declares no object.
        Manifest::parse(&text, &corpus_only()).unwrap();
    }

    #[test]
    fn a_glob_that_leaves_the_corpus_is_refused() {
        for bad in ["../elsewhere/**", "/etc/**", "**/everything"] {
            let text = CALC.replace(".yidam/computed/**", bad);
            assert!(
                Manifest::parse(&text, &corpus_only()).is_err(),
                "{bad} was accepted"
            );
        }
    }

    /// A field this binary does not implement is refused rather than ignored.
    ///
    /// Written against `after` until #472 implemented it, which is the point the test was
    /// making: the field it names is whatever the *next* generalisation will add, and the
    /// property is that a corpus declaring one gets a refusal naming the field rather than a
    /// run that silently did something else. `ageing_hours` is the plausible near-miss for the
    /// key that now exists, which is the shape a reader actually mistypes.
    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        for field in [
            "ageing_hours  = 3",
            "needs  = [\"upstream\"]",
            "retries  = 2",
        ] {
            let text = format!("{CALC}{field}\n");
            // `{:#}` rather than `{}`: serde names the field in the *source* of the error, and
            // the outer context is this module's own sentence — which is identical for every
            // malformed manifest, so a bare `to_string` would assert nothing about this one.
            let err = format!("{:#}", Manifest::parse(&text, &corpus_only()).unwrap_err());
            let name = field.split_whitespace().next().unwrap();
            assert!(
                err.contains(name),
                "`{field}` was accepted or misreported: {err}"
            );
        }
    }

    #[test]
    fn a_missing_step_names_what_is_declared() {
        let m = Manifest::parse(CALC, &corpus_only()).unwrap();
        let err = m.get("nope").unwrap_err().to_string();
        assert!(err.contains("low-flow"), "{err}");
    }

    // ── the graph the declarations form ───────────────────────────────────────

    /// A manifest of `name -> after`, with everything else held constant.
    ///
    /// Built rather than written out so the tests below differ in the one thing they are
    /// about. A declaration's other fields have their own tests above and none of them
    /// participate in an order.
    fn graph(steps: &[(&str, &[&str])]) -> String {
        let mut text = String::new();
        for (name, after) in steps {
            let deps: Vec<String> = after.iter().map(|d| format!("\"{d}\"")).collect();
            let _ = write!(
                text,
                "[capability.{name}]\nkind   = \"calculator\"\nrun    = [\"true\"]\n\
                 reads  = [\".yidam/corpus/**\"]\nwrites = [\".yidam/computed/**\"]\n\
                 verb   = \"compute\"\nafter  = [{}]\n\n",
                deps.join(", ")
            );
        }
        text
    }

    fn plan_of(text: &str, step: Option<&str>) -> Vec<String> {
        Manifest::parse(text, &corpus_only())
            .unwrap()
            .plan(step)
            .unwrap()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// The whole manifest, with every dependency ahead of what declares it.
    ///
    /// Asserted as a property over the result rather than as one expected sequence: the
    /// contract is *dependencies first*, and a literal expectation would also be pinning the
    /// tie-break between independent steps, which [`the_order_is_the_same_on_every_run`] is
    /// the test for.
    #[test]
    fn a_plan_puts_every_dependency_before_the_step_that_declares_it() {
        let text = graph(&[
            ("envelope", &["tier"]),
            ("tier", &["gather"]),
            ("gather", &[]),
            ("unrelated", &[]),
        ]);
        let order = plan_of(&text, None);
        assert_eq!(order.len(), 4, "{order:?}");
        let at = |s: &str| order.iter().position(|o| o == s).unwrap();
        assert!(at("gather") < at("tier"), "{order:?}");
        assert!(at("tier") < at("envelope"), "{order:?}");
    }

    /// Asking for one step plans its closure and nothing beside it.
    #[test]
    fn planning_one_step_plans_what_it_waits_for_and_nothing_else() {
        let text = graph(&[
            ("envelope", &["tier"]),
            ("tier", &["gather"]),
            ("gather", &[]),
            ("unrelated", &[]),
        ]);
        assert_eq!(plan_of(&text, Some("tier")), ["gather", "tier"]);
        assert_eq!(plan_of(&text, Some("gather")), ["gather"]);
        assert_eq!(
            plan_of(&text, Some("envelope")),
            ["gather", "tier", "envelope"]
        );
    }

    /// A step two dependents both wait for is in the plan once, not twice.
    ///
    /// The diamond is what separates *visited* from *ordered* in [`Manifest::visit`]. Run
    /// twice, `gather` would land two commits for one act and the second would be the no-op
    /// path reporting that nothing happened — a run that has to be explained rather than read.
    #[test]
    fn a_step_two_dependents_share_is_planned_once() {
        let text = graph(&[
            ("left", &["gather"]),
            ("right", &["gather"]),
            ("gather", &[]),
        ]);
        let order = plan_of(&text, None);
        assert_eq!(
            order.iter().filter(|s| *s == "gather").count(),
            1,
            "{order:?}"
        );
        assert_eq!(order.len(), 3, "{order:?}");
    }

    /// Independent steps are ordered by the manifest's keys, so two runs plan one sequence.
    #[test]
    fn the_order_is_the_same_on_every_run() {
        let text = graph(&[("c", &[]), ("a", &[]), ("b", &["a"])]);
        assert_eq!(plan_of(&text, None), ["a", "b", "c"]);
        for _ in 0..8 {
            assert_eq!(plan_of(&text, None), ["a", "b", "c"]);
        }
    }

    /// The definition of done's second bullet: refused, and the cycle is named.
    #[test]
    fn a_cycle_is_refused_and_the_cycle_is_named() {
        let text = graph(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]);
        let err = Manifest::parse(&text, &corpus_only())
            .unwrap_err()
            .to_string();
        assert!(err.contains("cycle"), "{err}");
        for step in ["a", "b", "c"] {
            assert!(
                err.contains(step),
                "the cycle does not name `{step}`: {err}"
            );
        }
        assert!(
            err.contains('→'),
            "the cycle is not shown as a chain: {err}"
        );
    }

    /// A step waiting for itself is a cycle of one and is named as one.
    #[test]
    fn a_step_that_waits_for_itself_is_refused() {
        let err = Manifest::parse(&graph(&[("a", &["a"])]), &corpus_only())
            .unwrap_err()
            .to_string();
        assert!(err.contains("cycle") && err.contains("a → a"), "{err}");
    }

    /// An `after` naming nothing is refused at load, with the declared set named.
    ///
    /// At load rather than when the step is reached, which is the module doc's rule: a corpus
    /// whose manifest names a step that does not exist has a broken manifest whether or not
    /// anybody ran the part that is broken.
    #[test]
    fn an_after_naming_no_capability_is_refused_at_load() {
        let err = Manifest::parse(&graph(&[("a", &["ghost"])]), &corpus_only())
            .unwrap_err()
            .to_string();
        assert!(err.contains("ghost"), "{err}");
        assert!(err.contains('a'), "the declared set is not named: {err}");
    }

    /// A step cannot wait for one whose output never lands on the branch it would read.
    ///
    /// The rule is about the route and therefore about the verb, so it is asserted over the
    /// whole epistemic family rather than over `establish`: a family member added tomorrow is
    /// covered the day it is added.
    #[test]
    fn a_step_cannot_wait_for_one_whose_commit_goes_to_a_proposal() {
        for verb in yidam_core::git::EPISTEMIC_VERBS {
            let text = format!(
                "[capability.downstream]\nkind   = \"calculator\"\nrun    = [\"true\"]\n\
                 reads  = [\".yidam/computed/**\"]\nwrites = [\".yidam/computed/**\"]\n\
                 verb   = \"compute\"\nafter  = [\"upstream\"]\n\n\
                 [capability.upstream]\nkind   = \"calculator\"\nrun    = [\"true\"]\n\
                 reads  = [\".yidam/corpus/**\"]\nwrites = [\".yidam/computed/**\"]\n\
                 verb   = \"{verb}\"\n"
            );
            let err = Manifest::parse(&text, &corpus_only())
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("epistemic") && err.contains("upstream"),
                "`after` a `{verb}` step was accepted or misreported: {err}"
            );
        }
    }

    /// And the same shape with an operational upstream loads, so the test above is about the
    /// family and not about the manifest it happens to be written against.
    #[test]
    fn the_same_declaration_with_an_operational_upstream_loads() {
        let text = "[capability.downstream]\nkind   = \"calculator\"\nrun    = [\"true\"]\n\
                    reads  = [\".yidam/computed/**\"]\nwrites = [\".yidam/computed/**\"]\n\
                    verb   = \"compute\"\nafter  = [\"upstream\"]\n\n\
                    [capability.upstream]\nkind   = \"calculator\"\nrun    = [\"true\"]\n\
                    reads  = [\".yidam/corpus/**\"]\nwrites = [\".yidam/computed/**\"]\n\
                    verb   = \"compute\"\n";
        assert_eq!(plan_of(text, None), ["upstream", "downstream"]);
    }

    /// The ageing rule parses, is absent by default, and is never a number in this binary.
    #[test]
    fn ageing_is_declared_per_capability_and_absent_by_default() {
        let m = Manifest::parse(CALC, &corpus_only()).unwrap();
        assert_eq!(m.get("low-flow").unwrap().ageing_days, None);

        let text = format!("{CALC}ageing_days = 30\n");
        let m = Manifest::parse(&text, &corpus_only()).unwrap();
        assert_eq!(m.get("low-flow").unwrap().ageing_days, Some(30));
    }

    #[test]
    fn the_literal_prefix_stops_at_the_first_wildcard() {
        assert_eq!(
            literal_prefix(".yidam/corpus/gage/**"),
            ".yidam/corpus/gage"
        );
        assert_eq!(literal_prefix("web/*.json"), "web");
        assert_eq!(literal_prefix("**/x"), "");
    }
}
