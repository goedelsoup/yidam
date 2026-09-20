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
//! `deny_unknown_fields`, and the reason is not tidiness. The generalisation in #472 adds
//! declared dependencies and an ageing rule to this file. A binary that silently ignored an
//! `after = [...]` it did not implement would run a dependent step before the step it
//! depends on and report success — the failure would be in the corpus, not in the exit code.
//! Refusing to parse says which field and which binary, which is recoverable.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::kuten::{Register, Registers};

/// Where the manifest lives, relative to the corpus root.
pub const MANIFEST: &str = ".yidam/capabilities.toml";

/// What a capability is.
///
/// Both arms are declarable today and only one is executable — see [`Kind::executable`]. A
/// corpus that declares a connector gets a manifest that parses and a step that refuses by
/// name, rather than a parse error blaming the wrong thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Connector,
    Calculator,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Calculator => "calculator",
        }
    }

    /// Whether this binary can invoke it.
    ///
    /// A connector re-runs against an external source, which is a network capability and a
    /// credential path; `vault/mod.rs` already sets the rule those inherit. #471 is the
    /// calculator slice and says so, and a refusal naming the issue is a better answer than
    /// a manifest that cannot express the other kind at all.
    pub fn executable(self) -> bool {
        matches!(self, Self::Calculator)
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
        Ok(m)
    }

    pub fn get<'a>(&'a self, step: &str) -> Result<&'a Capability> {
        self.capability.get(step).ok_or_else(|| {
            let declared: Vec<&str> = self.capability.keys().map(String::as_str).collect();
            if declared.is_empty() {
                anyhow::anyhow!("{MANIFEST} declares no capabilities, so `{step}` is not one")
            } else {
                anyhow::anyhow!(
                    "no capability named `{step}` — {MANIFEST} declares: {}",
                    declared.join(", ")
                )
            }
        })
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
        assert!(c.kind.executable());
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

    /// #472 adds fields to this file. An old binary must say so rather than ignore them.
    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let text = format!("{CALC}after  = [\"upstream\"]\n");
        // `{:#}` rather than `{}`: serde names the field in the *source* of the error, and
        // the outer context is this module's own sentence — which is identical for every
        // malformed manifest, so a bare `to_string` would assert nothing about this one.
        let err = format!("{:#}", Manifest::parse(&text, &corpus_only()).unwrap_err());
        assert!(err.contains("after"), "{err}");
    }

    #[test]
    fn a_missing_step_names_what_is_declared() {
        let m = Manifest::parse(CALC, &corpus_only()).unwrap();
        let err = m.get("nope").unwrap_err().to_string();
        assert!(err.contains("low-flow"), "{err}");
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
