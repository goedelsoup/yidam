//! Typechecking a declared calculator without invoking one (#1099, RFC-0042 open question 5).
//!
//! A `.glu` script that does not typecheck is a **manifest** failure, not a step failure. It is
//! decidable before anything runs — the same way a cycle in `after` is — so a corpus should find
//! out its calculator is broken from `yidam lint`, on the commit that broke it, rather than from
//! the step that was supposed to produce a signal.
//!
//! # One prelude, or no gate at all
//!
//! The constraint that makes this worth its own module is *whose* vocabulary the script is held
//! to. RFC-0042: "A second opinion about a script's vocabulary is worse than no gate — it either
//! passes something `run` refuses, or refuses something `run` would have accepted." So nothing
//! here builds a prelude, injects a type or decides what a calculator is: it calls
//! [`entry::admit`], which is the function [`crate::gluon_arm::evaluate`] opens with, and
//! `tests/gluon_arm.rs` says so. The same shape #1080 arrived at, for the same reason.
//!
//! # Three refusals, three checks
//!
//! The arm's purity is a closed prelude *and* a typecheck (#1087), and those refuse for three
//! different reasons a reader has to be able to tell apart:
//!
//! | check | refusal | needs |
//! |---|---|---|
//! | `calculator-script` | the path does not read, or the script invokes a macro | text |
//! | `calculator-scope` | a name the prelude does not bind | the typechecker |
//! | `calculator-type` | not `Corpus -> Computed` | the typechecker |
//!
//! The split is by **what each build can answer**. `refuse_macros` and the prelude are text and a
//! list of names, so `calculator-script` gates identically in the light binary `install.sh`
//! downloads and in one built with `--features calculators-gluon`. The other two need a VM, and
//! are left out of a light build's report rather than reported empty — see
//! [`super::Asked::WithTypechecker`] for why a build that held nothing to the prelude should not
//! report that it did, and #1114 for what absence still does not fix.
//!
//! # A manifest that does not parse yields nothing here
//!
//! Deliberately. `yidam run` and `yidam doctor` report an unparseable manifest as the failure it
//! is; reporting *every* declared calculator as broken because the file above them has a typo
//! would bury the one finding anybody can act on under a list of consequences of it. The same
//! judgement [`super::input::Input::config`] records about a malformed config.

use std::path::Path;

use crate::corpus::Overlay;
use crate::gluon_arm::entry::{self, Refused};

use super::{Check, Severity, Violation};

/// One declared calculator, and what could be decided about it.
pub(crate) struct Script {
    /// The capability that declared it — what a reader greps `capabilities.toml` for.
    pub step: String,
    /// The script's path, repo-relative: the node every finding here is filed against.
    pub rel: String,
    /// Its text, or `None` where the declared path did not resolve.
    pub text: Option<String>,
    /// What [`entry::admit`] made of it, or `None` in a build with no typechecker.
    pub admitted: Option<Admission>,
}

/// The verdict of the one admission, as a value a pure check can read.
///
/// Declared in every build although a light one constructs none of them — [`admit`] there
/// returns `None` and the two checks that read them are out of the report. Kept unconditional
/// deliberately: [`scope`] and [`shape`] are then the same code in both builds and their unit
/// tests run in both, so the reading that sorts a scope failure from a shape failure is held to
/// something in the build that cannot produce one.
#[cfg_attr(not(feature = "calculators-gluon"), allow(dead_code))]
pub(crate) enum Admission {
    /// A calculator. Whether it *computes* the right thing is a step's business, not lint's.
    Ok,
    Refused(entry::Refusal),
    /// `gluon_base`'s `ice!` is a `panic!` and a script can reach one, so
    /// `gluon_arm::ice::contain` catches it. Kept apart from a refusal because it is not the
    /// corpus's defect: charging it to the repository as a gating error would fail a build over
    /// somebody else's bug.
    Ice(String),
}

/// Every calculator `.yidam/capabilities.toml` declares, read through the overlay.
///
/// Through the overlay like every other reading in this module's neighbour, so the editor holds
/// the buffer somebody is typing a calculator into to the same verdict the gate will reach.
pub(crate) fn read(root: &Path, overlay: &Overlay) -> Vec<Script> {
    use crate::cmd::run::manifest::{Manifest, MANIFEST};

    let text = overlay.read(&root.join(MANIFEST));
    let Ok(manifest) = Manifest::parse(&text, &crate::kuten::Registers::of_repo(root)) else {
        return Vec::new();
    };

    manifest
        .capability
        .iter()
        .filter_map(|(step, cap)| cap.run.gluon().map(|(rel, _)| (step, rel)))
        .map(|(step, rel)| {
            let text = overlay.try_read(&root.join(rel));
            Script {
                admitted: text.as_deref().and_then(|t| admit(step, t)),
                step: step.clone(),
                rel: rel.to_string(),
                text,
            }
        })
        .collect()
}

/// The one admission, where this build has it.
#[cfg(feature = "calculators-gluon")]
fn admit(step: &str, text: &str) -> Option<Admission> {
    Some(match entry::admit(step, text) {
        Ok(_) => Admission::Ok,
        Err(entry::Rejected::Refused(r)) => Admission::Refused(r),
        Err(entry::Rejected::Ice(e)) => Admission::Ice(e.to_string()),
    })
}

/// No typechecker in this build, so nothing is decided — and the two checks that would have
/// read the answer are not in the report either.
#[cfg(not(feature = "calculators-gluon"))]
fn admit(_step: &str, _text: &str) -> Option<Admission> {
    None
}

/// All three, so the one admission per script is performed once however many of its readings
/// the roster asks for.
pub(crate) fn checks(scripts: &[Script]) -> [Check; 3] {
    [script(scripts), scope(scripts), shape(scripts)]
}

/// The refusal every build can reach: the declaration names a file that reads, and what it
/// names is not reaching for a macro.
fn script(scripts: &[Script]) -> Check {
    let mut violations = Vec::new();
    for s in scripts {
        match &s.text {
            None => violations.push(Violation::new(
                s.rel.clone(),
                format!(
                    "`{}` declares `run = {{ gluon = \"{}\" }}` and that path does not read — \
                     a calculator is a file in this repository, and the plan names it before \
                     anything runs",
                    s.step, s.rel
                ),
            )),
            Some(text) => {
                if let Err(r) = entry::refuse_macros(text) {
                    violations.push(Violation::new(s.rel.clone(), refusal_detail(s, &r)));
                }
            }
        }
    }
    Check::new(
        "calculator-script",
        "a declared calculator reads, and invokes no macro",
        Severity::Error,
        "RFC-0042's arm is pure because its prelude is closed, and a macro is how a script \
         reopens it: `import!` reaches any module on disk and `io.println` is one call away. \
         Both halves of this check are decidable from the text, so the binary `install.sh` \
         downloads refuses exactly what a binary built with the engine refuses — a gate whose \
         verdict depended on which build the reader was holding would be the divergence the \
         RFC warns about, arriving through the gate instead of through the vocabulary.",
        violations,
    )
}

/// A name the prelude does not bind. The typechecker reports it, and it is not a type failure.
fn scope(scripts: &[Script]) -> Check {
    Check::new(
        "calculator-scope",
        "a calculator names only what the prelude binds",
        Severity::Error,
        "The commonest way to write a calculator that cannot run is to reach for a module the \
         prelude does not inject — the arm has no `import!`, so the names a script has are \
         exactly the ones it was given. The typechecker is what notices, which is why this is \
         reported as scope and not as type: the script did not get its shape wrong, it named \
         something that is not there, and the fix is a different arm rather than a different \
         signature.",
        refused(scripts, Refused::Scope),
    )
}

/// The shape. A calculator is `Corpus -> Computed` and nothing else is one.
fn shape(scripts: &[Script]) -> Check {
    let mut violations = refused(scripts, Refused::Shape);
    // Reported here rather than as its own check, and as a warning rather than an error: an
    // internal compiler error is gluon's defect and not this repository's, so it belongs in
    // the report — somebody has to know the script got no verdict — without gating a build on
    // a panic nobody in this corpus can fix.
    for s in scripts {
        if let Some(Admission::Ice(e)) = &s.admitted {
            violations.push(
                Violation::new(
                    s.rel.clone(),
                    format!(
                        "the typechecker panicked on `{}`, so this script got no verdict: {e}",
                        s.step
                    ),
                )
                .at(Severity::Warn),
            );
        }
    }
    Check::new(
        "calculator-type",
        "a declared calculator is `Corpus -> Computed`",
        Severity::Error,
        "A typed calculator is a function from the corpus to the signals it computes, and a \
         script of any other type is not one — including the near miss the type system is there \
         to catch, a script that performs an effect and so has type `IO _` in a position where \
         only a value belongs. Refused here rather than at the step, because a corpus should not \
         have to invoke a calculator to find out it never could have.",
        violations,
    )
}

/// The findings of one kind, filed against the script rather than the manifest.
///
/// Against the script deliberately: the baseline compares on the path, and the failure is in the
/// file somebody has to open. The detail names the step, which is what makes the manifest
/// findable from the finding.
fn refused(scripts: &[Script], kind: Refused) -> Vec<Violation> {
    scripts
        .iter()
        .filter_map(|s| match &s.admitted {
            Some(Admission::Refused(r)) if r.refused == kind => {
                Some(Violation::new(s.rel.clone(), refusal_detail(s, r)))
            }
            _ => None,
        })
        .collect()
}

/// A refusal as a finding reads it: the step it belongs to, then the refusal's own words.
///
/// The refusal's prose is `entry`'s and is not restated here. It is the sentence `yidam run`
/// prints, and two wordings of one refusal is how a reader comes to believe the gate and the
/// runner are checking different things.
fn refusal_detail(s: &Script, r: &entry::Refusal) -> String {
    format!("`{}`: {r}", s.step)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(rel: &str, text: Option<&str>, admitted: Option<Admission>) -> Script {
        Script {
            step: "signals".to_string(),
            rel: rel.to_string(),
            text: text.map(str::to_string),
            admitted,
        }
    }

    #[test]
    fn a_declared_script_that_does_not_read_is_reported() {
        let c = super::script(&[script("calc/x.glu", None, None)]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "calc/x.glu");
        assert!(c.violations[0].detail.contains("does not read"), "{c:?}");
    }

    /// The refusal every build reaches, and the reason this check is not behind the feature.
    #[test]
    fn a_macro_is_refused_from_the_text_alone() {
        let c = super::script(&[script(
            "calc/x.glu",
            Some("let x = import! std.io\n\\corpus -> { rows = [] }"),
            None,
        )]);
        assert_eq!(c.violations.len(), 1, "{c:?}");
        assert!(c.violations[0].detail.contains("signals"), "{c:?}");
    }

    #[test]
    fn a_calculator_that_admits_is_not_a_finding() {
        let scripts = [script("calc/x.glu", Some("\\c -> c"), Some(Admission::Ok))];
        for c in checks(&scripts) {
            assert!(c.passed(), "{c:?}");
        }
    }

    /// The whole point of telling the two typechecker refusals apart: each is reported by its
    /// own check, so a baseline entry says which kind of mistake was accepted.
    #[test]
    fn scope_and_shape_are_reported_by_different_checks() {
        let scripts = [
            script(
                "calc/a.glu",
                Some("\\c -> c"),
                Some(Admission::Refused(entry::Refusal::new(
                    Refused::Scope,
                    "names `io`",
                ))),
            ),
            script(
                "calc/b.glu",
                Some("\\c -> c"),
                Some(Admission::Refused(entry::Refusal::new(
                    Refused::Shape,
                    "is not a calculator",
                ))),
            ),
        ];
        let [_, scope, shape] = checks(&scripts);
        assert_eq!(scope.violations.len(), 1, "{scope:?}");
        assert_eq!(scope.violations[0].node, "calc/a.glu");
        assert_eq!(shape.violations.len(), 1, "{shape:?}");
        assert_eq!(shape.violations[0].node, "calc/b.glu");
    }

    /// An ICE is reported and does not gate: the panic is gluon's and the corpus cannot fix it.
    #[test]
    fn an_ice_is_a_warning_on_the_type_check() {
        let scripts = [script(
            "calc/x.glu",
            Some("\\c -> c"),
            Some(Admission::Ice("panicked at ice.rs".to_string())),
        )];
        let [_, scope, shape] = checks(&scripts);
        assert!(scope.passed(), "{scope:?}");
        assert_eq!(shape.violations.len(), 1, "{shape:?}");
        assert_eq!(
            shape.severity_of(&shape.violations[0]),
            Severity::Warn,
            "an ICE must not gate a build"
        );
    }

    /// Nothing declared, nothing read, nothing reported — the state of every corpus today.
    #[test]
    fn a_corpus_declaring_no_calculator_reports_nothing() {
        for c in checks(&[]) {
            assert!(c.passed(), "{c:?}");
        }
    }
}
