//! The typed calculator arm (RFC-0042), and the row of its own table it has to survive.
//!
//! RFC-0042 made its exclusion from the default build a kill criterion: *"71 marginal packages
//! and +6.8 MB are acceptable **only** outside the default set … if anything in the default build
//! ends up linking it, the proposal fails on the first row of the table and should be closed."*
//! The first two tests here are that row, and they are the reason this file is not entirely
//! behind `#[cfg(feature = "calculators-gluon")]` — a gate that only runs in the build it is
//! about would be asserting the wrong thing in the build that matters.
//!
//! Everything after them is the engine, and one of those tests is a regression guard on a
//! measurement that contradicts the RFC: `std.debug.trace` is declared `a -> ()`, is registered
//! unconditionally, and prints to stdout, so the entry point's type is necessary and not
//! sufficient. `src/gluon_arm/mod.rs` carries the argument; `the_one_pure_typed_effect_is_out_of
//! _reach` carries the check.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The one line of `Cargo.toml` that begins with `key`.
fn manifest_line(key: &str) -> String {
    let toml = manifest();
    let mut found: Vec<&str> = toml
        .lines()
        .filter(|l| l.trim_start().starts_with(key))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "Cargo.toml has {} line(s) starting with {key:?}; this test grades exactly one",
        found.len()
    );
    found.pop().unwrap_or_default().to_string()
}

// ── the first row of RFC-0042's table ─────────────────────────────────────────

/// The arm is behind a feature, that feature is not in `default`, and `full` has it.
///
/// Derived from the manifest text rather than from `cfg!`, so it is the same assertion in every
/// build. `cfg!(not(feature = …))` would pass vacuously in exactly the build that could be wrong.
#[test]
fn the_arm_is_outside_the_default_set() {
    let default = manifest_line("default = [");
    assert!(
        !default.contains("calculators-gluon"),
        "`calculators-gluon` is in the default set:\n  {default}\n\n\
         That is RFC-0042's stated kill criterion. The arm costs 71 marginal packages and \
         +6.8 MB against the 8 RFC-0024 accepted for regorus, and the whole proposal rests on \
         the light binary — what `install.sh` downloads and release.yml cross-compiles — not \
         paying for it. If the default set needs it, the RFC fails on the first row of its own \
         table and should be closed rather than quietly widened."
    );
    let toml = manifest();
    assert!(
        toml.contains("calculators-gluon = [\"dep:gluon\", \"dep:gluon_codegen\"]"),
        "the `calculators-gluon` feature no longer names exactly the two optional dependencies \
         it gates; a feature that enables nothing gates nothing"
    );
    for dep in ["gluon = {", "gluon_codegen = {"] {
        let line = manifest_line(dep);
        assert!(
            line.contains("optional = true"),
            "{dep} is not optional, so the default build links it whatever the feature says:\n  \
             {line}"
        );
    }
    // `full` is what `cargo install --features full` and `--all-features` reproduce. A feature
    // missing from it is one only `--all-features` ever compiles, which is the hole every note
    // in `[features]` is written against.
    let full_start = toml.find("\nfull = [").expect("no `full` feature");
    let full_end = toml[full_start..].find(']').expect("unterminated `full`") + full_start;
    assert!(
        toml[full_start..full_end].contains("calculators-gluon"),
        "`full` does not name `calculators-gluon`"
    );
}

/// Nothing outside `src/gluon_arm/` reaches into the gluon crate.
///
/// The feature can only be honest if the code behind it is in one place. A `use gluon::…` in a
/// module that is not gated compiles in the default build or does not compile at all, and the
/// second is how a feature stops being optional. Held over the tree rather than over a list.
///
/// # The engine and the arm's name are different things (#1091)
///
/// This scanned for the *word* until the manifest could declare the arm. A corpus now writes
/// `run = { gluon = "…" }`, so `gluon` is a key in `.yidam/capabilities.toml` and appears in
/// `cmd/run/manifest.rs` — in the variant, in its field, and in every refusal that quotes the
/// declaration back. None of that links a package: the shape and its validation are ungated
/// deliberately, because the released binary is the one that reads a manifest declaring an arm it
/// cannot run, and a build that could not parse the declaration would refuse the whole manifest
/// over a step it was never going to invoke.
///
/// So the scan asks about a path into the crate — `gluon::`, `use gluon`, `gluon_codegen` — which is
/// what the paragraph above was always arguing about. It is still over the tree and not over a list,
/// and it is narrower in exactly one way: a manifest keyword is no longer an engine reference.
#[test]
fn only_the_arm_names_the_engine() {
    let src = repo_root().join("yidam/cli/src");
    let mut strays: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(&src).into_iter().flatten() {
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&src)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("gluon_arm/") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            // Code only: `lib.rs` has to be able to say the word in the doc comment that
            // explains why the module is gated.
            // The module declaration `lib.rs` makes is `gluon_arm`, which is a path into this
            // crate and not into the engine, so it is masked before the question is asked.
            let code = line
                .split("//")
                .next()
                .unwrap_or("")
                .replace("gluon_arm", "<module>");
            if ["gluon::", "use gluon", "gluon_codegen"]
                .iter()
                .any(|p| code.contains(p))
            {
                strays.push(format!("  src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        strays.is_empty(),
        "the gluon crate is reached from code outside `src/gluon_arm/`:\n{}\n\n\
         Every use of the engine belongs behind `calculators-gluon` in one module, or the \
         feature is not what decides whether the default build links 71 packages.\n\
         Declaring the arm is not using it: `run = {{ gluon = … }}` is manifest vocabulary and is \
         parsed in every build.",
        strays.join("\n")
    );
}

// ── the engine ────────────────────────────────────────────────────────────────

#[cfg(feature = "calculators-gluon")]
mod engine {
    use gluon::vm::api::VmType;
    use gluon::{new_vm, ThreadExt};
    use yidam::gluon_arm::marshal::{Computed, Corpus, Link, NodeView, Property, Value};
    use yidam::gluon_arm::{self, budget, entry, EXCLUDED, PRELUDE_MODULES, PRELUDE_TYPES};

    /// Whitespace-insensitive, so a rendered type's line breaks do not decide a comparison.
    fn flat(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The same type with every module path dropped: `std.types.Bool` as `Bool`.
    ///
    /// `make_type` renders a bool as `std.types.Bool`, and the prelude's `type Value = …` cannot
    /// spell it that way — a path is not a type name a script can write. They unify because they
    /// are the same type; what differs is only how each side is allowed to say it.
    fn no_paths(s: &str) -> String {
        let mut out = String::new();
        let mut word = String::new();
        for c in s.chars() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                word.push(c);
            } else {
                out.push_str(word.rsplit('.').next().unwrap_or(&word));
                word.clear();
                out.push(c);
            }
        }
        out.push_str(word.rsplit('.').next().unwrap_or(&word));
        out
    }

    fn corpus(miles: &[(&str, f64)]) -> Corpus {
        Corpus {
            nodes: miles
                .iter()
                .map(|(id, m)| NodeView {
                    id: (*id).to_string(),
                    class: "gage".to_string(),
                    label: String::new(),
                    description: String::new(),
                    properties: vec![Property {
                        key: "miles".to_string(),
                        value: Value::Number(*m),
                    }],
                    links: Vec::new(),
                    references: Vec::new(),
                    malformed: false,
                })
                .collect(),
            classes: Vec::new(),
        }
    }

    /// Typecheck a script body the way [`gluon_arm::evaluate`] would, and say only whether it
    /// was accepted.
    fn accepts(script: &str) -> Result<(), String> {
        let source = gluon_arm::prepare(script).map_err(|e| e.to_string())?;
        let vm = new_vm();
        entry::typecheck(&vm, "t", &source).map_err(|e| e.to_string())
    }

    // ── the two lists ─────────────────────────────────────────────────────────

    /// A module cannot be both allowed and excluded, and no entry is a duplicate.
    #[test]
    fn the_two_module_lists_are_disjoint() {
        let excluded: Vec<&str> = EXCLUDED.iter().map(|(m, _)| *m).collect();
        for m in PRELUDE_MODULES {
            assert!(
                !excluded.contains(m),
                "`std.{m}` is in the prelude and in the excluded list"
            );
        }
        for list in [PRELUDE_MODULES.to_vec(), excluded.clone()] {
            let mut sorted = list.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                list.len(),
                "a module is listed twice: {list:?}"
            );
        }
        assert!(
            EXCLUDED.iter().all(|(_, why)| !why.is_empty()),
            "an excluded module carries no reason, and the reason is what the next reader needs"
        );
    }

    /// Every module the prelude names resolves, and binds what a script would reach for.
    ///
    /// A typo in that list is a `Could not find module` at the first calculator anyone writes,
    /// in a build almost nobody has.
    #[test]
    fn every_prelude_module_resolves() {
        for m in PRELUDE_MODULES {
            let vm = new_vm();
            let src = format!("import! std.{m}");
            assert!(
                vm.typecheck_str("p", &src, None).is_ok(),
                "`std.{m}` is in the prelude and does not resolve"
            );
        }
    }

    // ── the injected types mirror the derived ones ────────────────────────────

    /// The textual `type Value = …` in the prelude is the type `#[derive(VmType)]` produced.
    ///
    /// Gluon's variants are structural, so these unify only while the constructors and their
    /// argument types match exactly — and a mismatch does not fail loudly, it fails as a type
    /// error in whichever calculator happens to use the changed constructor. Adding a case to
    /// the Rust enum and not to the prelude is what this catches.
    #[test]
    fn the_injected_types_match_the_derived_ones() {
        let vm = new_vm();
        let derived = no_paths(&flat(&<Value as VmType>::make_type(&vm).to_string()));
        let declared = PRELUDE_TYPES
            .iter()
            .find_map(|t| t.split_once(" = "))
            .map(|(_, rhs)| no_paths(&flat(rhs)))
            .expect("no `type Value = …` in PRELUDE_TYPES");
        assert_eq!(
            declared, derived,
            "the prelude declares `Value` as one type and `marshal::Value` derives another"
        );

        // And the comparison is not the only check, because it is a comparison of renderings.
        // Each constructor is used in a real calculator, which is what a mismatch would break.
        for ctor in [
            "Text \"x\"",
            "Int 1",
            "Number 1.0",
            "Flag True",
            "Empty",
            "Unrepresentable",
        ] {
            let script = format!(
                "\\c -> {{ signals = [{{ node = \"a/b\", values = [{{ name = \"n\", value = {ctor} }}] }}], \
                 summary = [] }}"
            );
            accepts(&script)
                .unwrap_or_else(|e| panic!("the prelude's `{ctor}` is not `marshal::Value`: {e}"));
        }
    }

    // ── what the entry point's type does and does not refuse ──────────────────

    /// A calculator is accepted, and every shape of performed effect is refused at typecheck.
    #[test]
    fn the_entry_type_refuses_an_effect_in_a_pure_position() {
        accepts("\\c -> { signals = [], summary = [] }")
            .expect("a pure calculator is a calculator");
        accepts(
            "\\c -> { signals = array.functor.map \
             (\\n -> { node = n.id, values = [{ name = \"class\", value = Text n.class }] }) c.nodes, \
             summary = [] }",
        )
        .expect("reading the corpus is what a calculator is for");

        // No `io` is in scope, so these are refused twice over — once for the name and once for
        // the type. The name is the mechanism that works; the type is the one RFC-0042 argued
        // for, and `the_one_pure_typed_effect_is_out_of_reach` below is why it needed help.
        for bad in [
            "\\c -> { signals = [], summary = [], extra = 1 }",
            "\\c -> { signals = 1, summary = [] }",
            "\\c -> 1",
            "\\c -> { signals = [{ node = 1, values = [] }], summary = [] }",
        ] {
            assert!(
                accepts(bad).is_err(),
                "this is not a `Corpus -> Computed` and was accepted: {bad}"
            );
        }
    }

    /// The measurement that contradicts RFC-0042, kept as a check.
    ///
    /// `std.debug.trace` is declared `a -> ()`, is registered unconditionally, and is `println!`
    /// to stdout — so a function whose declared type is exactly a calculator's can perform an
    /// effect and typecheck. For this CLI stdout is where `--format json` goes. What puts it out
    /// of reach is the closed prelude and the macro refusal, and this asserts both arms of that.
    #[test]
    fn the_one_pure_typed_effect_is_out_of_reach() {
        // The name is not bound.
        let err = accepts("\\c -> { signals = [], summary = [], x = debug.trace 1 }")
            .expect_err("`debug` must not be in a calculator's scope");
        assert!(err.contains("debug"), "{err}");

        // And it cannot be brought into scope, because that takes a macro.
        for reach in [
            "let debug = import! std.debug in \\c -> { signals = [], summary = [] }",
            "let d = import! std.debug.prim in \\c -> { signals = [], summary = [] }",
            "let io = import! std.io in \\c -> { signals = [], summary = [] }",
        ] {
            let err = accepts(reach).expect_err("a calculator may not import");
            assert!(
                err.contains("macro"),
                "an import was refused for the wrong reason: {err}"
            );
        }

        // Nor reached as a variable path: a registered global is not a name in scope. Measured
        // on gluon 0.18.4 — `std.debug.prim.trace "x"` is *Undefined variable `std`*.
        let err = accepts("\\c -> { signals = [], summary = [], x = std.debug.prim.trace 1 }")
            .expect_err("a registered global is not a variable");
        assert!(err.contains('`'), "{err}");
    }

    /// Every excluded module is unreachable, by name and by import.
    #[test]
    fn no_excluded_module_can_be_reached() {
        for (m, why) in EXCLUDED {
            let bound = accepts(&format!("\\c -> {{ signals = [], summary = [], x = {m} }}"));
            assert!(
                bound.is_err(),
                "`{m}` is bound in a calculator's scope ({why})"
            );
            let imported = accepts(&format!(
                "let {m} = import! std.{m} in \\c -> {{ signals = [], summary = [] }}"
            ));
            let err = imported.expect_err(&format!("`std.{m}` was importable ({why})"));
            assert!(err.contains("macro"), "{err}");
        }
    }

    // ── the macro refusal ─────────────────────────────────────────────────────

    /// A macro invocation is refused, and the refusal says which one and where.
    #[test]
    fn a_macro_invocation_is_refused_with_its_line() {
        let err = entry::refuse_macros(
            "\\c ->\n  let d = import! std.debug\n  { signals = [], summary = [] }",
        )
        .expect_err("an import is a macro invocation");
        let msg = err.to_string();
        assert!(
            msg.contains("line 2"),
            "the refusal does not name the line: {msg}"
        );
        assert!(msg.contains("`import!`"), "{msg}");
        // `lift_io!` is the other macro the default VM registers, and the rule is over macro
        // invocations rather than over today's two names.
        assert!(
            entry::refuse_macros("lift_io! lift (1)").is_err(),
            "only `import!` is refused, so the rule is a denylist after all"
        );
        assert!(
            entry::refuse_macros("\\c -> { signals = [], summary = [], x = 1 != 2 }").is_ok(),
            "`!=` is not a macro invocation"
        );
        assert!(
            entry::refuse_macros("\\c -> { signals = [], summary = [] }").is_ok(),
            "a calculator with no macro must pass"
        );
    }

    /// The scan walks characters, and a multi-byte one before a `!` does not take the process.
    ///
    /// `refuse_macros` is the first thing `prepare` does, so a byte index landing inside a
    /// character here aborts on script *text* — a refusal scan that panics has refused nothing
    /// and told nobody. A calculator may hold any text in a string literal.
    #[test]
    fn a_multibyte_character_before_a_bang_is_read_and_not_sliced() {
        // Refused — a false positive, and the safe direction: `a!` is the macro shape, and the
        // scan does not know it is inside a string. What matters is that it *returns*.
        assert!(
            entry::refuse_macros("\\c -> { signals = [], summary = [], m = \"¡Hola!\" }").is_err()
        );
        // Not refused, and the character before the `!` is multi-byte in both.
        assert!(
            entry::refuse_macros("\\c -> { signals = [], summary = [], m = \"café — !\" }").is_ok()
        );
        assert!(
            entry::refuse_macros("\\c -> { signals = [], summary = [], m = \"日本!\" }").is_ok()
        );
        // And a name reached past a multi-byte delimiter is still named in full.
        let err = entry::refuse_macros("-- ¡\nlet d = import! std.debug in d")
            .expect_err("an import is an import whatever precedes it");
        assert!(err.to_string().contains("`import!`"), "{err}");
    }

    /// The scan has no false negatives: every spelling that actually imports contains `import!`.
    ///
    /// This is the soundness of a lexical rule, and it is checked against the engine rather than
    /// argued. Anything gluon accepts as an import must be something the scan refuses; a
    /// spelling the scan misses must be one gluon does not import for.
    #[test]
    fn no_spelling_imports_past_the_scan() {
        for spelling in [
            "let d = import! std.debug in d.show 1",
            "let d = import ! std.debug in d.show 1",
            "let d = import/*x*/! std.debug in d.show 1",
            "let d = import\t! std.debug in d.show 1",
            "let d = import\n! std.debug in d.show 1",
        ] {
            let refused = entry::refuse_macros(spelling).is_err();
            let vm = new_vm();
            let imported = vm
                .typecheck_str("i", spelling, None)
                .map_err(|e| e.to_string());
            assert!(
                refused || imported.is_err(),
                "gluon imported a module the scan let through:\n  {spelling:?}"
            );
        }
    }

    // ── the budget ────────────────────────────────────────────────────────────

    /// A script that does not finish is refused, and the refusal names the budget.
    #[test]
    fn the_budget_stops_a_calculator_that_does_not_finish() {
        // One line and explicit `in`: this test is about the budget, and a script whose
        // parse depends on layout would fail it for the wrong reason. A gluon script cannot
        // loop without calling something, which is why counting calls bounds it at all.
        let looping = "\\c -> rec let go n = go (n + 1) in { signals = go 0, summary = [] }";
        let err = gluon_arm::evaluate("loop", looping, corpus(&[]), 5_000)
            .expect_err("an endless calculator must be refused rather than waited for");
        let msg = err.to_string();
        assert!(
            msg.contains("5000"),
            "the refusal does not name the budget: {msg}"
        );
        assert!(msg.contains("budget"), "{msg}");
    }

    /// The budget also stops a script that loops before the entry point is ever applied.
    ///
    /// `run_expr` does not only compile: it evaluates the module's top level. A script that
    /// loops there never reaches `f.call`, and reporting that as a compile failure would tell a
    /// calculator author the wrong thing about their own script — and hide the one refusal in
    /// this arm a corpus can act on.
    #[test]
    fn the_budget_stops_a_calculator_that_loops_before_it_is_called() {
        let looping =
            "rec let go n = go (n + 1) in let x = go 0 in \\c -> { signals = [], summary = [] }";
        let err = gluon_arm::evaluate("toploop", looping, corpus(&[]), 5_000)
            .expect_err("a top-level loop must be refused rather than waited for");
        let msg = err.to_string();
        assert!(
            msg.contains("5000"),
            "the refusal does not name the budget: {msg}"
        );
        assert!(msg.contains("budget"), "{msg}");
    }

    /// An integer the script returns comes back at the width it was written.
    ///
    /// `gluon_vm` converts every integer width through `VmInt` with a cast, so a narrower Rust
    /// type here would take `Int 3000000000` back as `705032704` — silently, in the direction a
    /// receipt cannot check, and against `computed::scalar`, which keeps signal integers wide.
    #[test]
    fn an_integer_wider_than_i32_survives_the_round_trip() {
        let out = gluon_arm::evaluate(
            "wide",
            "\\c -> { signals = [{ node = \"gage/one\", values = \
             [{ name = \"n\", value = Int 3000000000 }] }], summary = [] }",
            corpus(&[]),
            budget::DEFAULT_CALLS,
        )
        .expect("a calculator returning a wide integer runs");
        assert_eq!(
            out.computed.signals[0].values[0].value,
            Value::Int(3_000_000_000)
        );
    }

    /// A count about the whole run comes back beside the table, and the field is not optional.
    ///
    /// `summary` is a field of the entry point's return type, so a script that omits it does not
    /// typecheck — gluon's records are exact, which is the same property
    /// [`the_entry_type_refuses_an_effect_in_a_pure_position`] relies on to refuse an `extra` one.
    /// That is a cost worth stating: a calculator with nothing to count writes `summary = []`
    /// rather than leaving the field out, and there is no shape of optional record field in gluon
    /// that would let it. What it buys is that the block cannot be forgotten silently.
    #[test]
    fn a_count_about_the_run_comes_back_and_cannot_be_left_out() {
        let out = gluon_arm::evaluate(
            "counted",
            "\\c -> { signals = [], summary = [{ name = \"nodes\", \
             value = Int (array.len c.nodes) }] }",
            corpus(&[("gage/one", 1.0), ("gage/two", 2.0)]),
            budget::DEFAULT_CALLS,
        )
        .expect("a calculator counting its own input runs");
        assert_eq!(out.computed.summary.len(), 1);
        assert_eq!(out.computed.summary[0].name, "nodes");
        assert_eq!(out.computed.summary[0].value, Value::Int(2));

        let err = gluon_arm::evaluate(
            "no-summary",
            "\\c -> { signals = [] }",
            corpus(&[]),
            budget::DEFAULT_CALLS,
        )
        .expect_err("a record missing a field is not the entry point's type");
        assert!(
            err.to_string().contains("is not a calculator"),
            "a script omitting `summary` was refused for some other reason: {err}"
        );
    }

    /// A script reads a link's resolved target by matching on it, with no import to do it.
    ///
    /// The field is an `Option String` (see [`Link::resolved`]), so a calculator that
    /// wants the chain rule has to be able to spell `Some` and `None`. Neither is in
    /// [`PRELUDE_MODULES`] — they come from gluon's own implicit prelude — and this is the check
    /// that the closed scope leaves them reachable. If a gluon upgrade ever stops binding them,
    /// the answer is a `type Option` in [`PRELUDE_TYPES`] and not a script that imports.
    #[test]
    fn a_script_matches_a_resolved_link_with_no_import() {
        let mut c = corpus(&[("reach/lower-canyon", 1.0)]);
        c.nodes[0].links = vec![
            Link {
                target: "../reach.ont.yml".into(),
                relationship: "instance-of".into(),
                resolved: None,
            },
            Link {
                target: "../gage/valley-bridge.yml".into(),
                relationship: "measured-by".into(),
                resolved: Some("gage/valley-bridge".into()),
            },
        ];
        let out = gluon_arm::evaluate(
            "reach",
            "\\c ->\n  \
             let landed l =\n    \
             match l.resolved with\n    \
             | Some id -> 1\n    \
             | None -> 0\n  \
             let count n = foldable.foldl (\\acc l -> acc + landed l) 0 n.links\n  \
             { signals = array.functor.map (\\n -> { node = n.id, \
             values = [{ name = \"links_in_corpus\", value = Int (count n) }] }) c.nodes, \
             summary = [] }",
            c,
            budget::DEFAULT_CALLS,
        )
        .expect("matching on a resolved link is what the chain rule is written with");
        assert_eq!(out.computed.signals[0].values[0].value, Value::Int(1));
    }

    /// The default budget is headroom and not a limit anything real meets.
    #[test]
    fn a_real_calculator_costs_a_fraction_of_the_budget() {
        let script = "\\c ->\n    let miles p =\n        match p.value with\n        \
                      | Number x -> x\n        | _ -> 0.0\n    \
                      let total n = foldable.foldl (\\acc p -> acc + miles p) 0.0 n.properties\n    \
                      { signals = array.functor.map (\\n -> { node = n.id, values = \
                      [{ name = \"tier\", value = Text (if total n > 10.0 then \"far\" else \"near\") }] }) c.nodes, \
                      summary = [] }";
        let out = gluon_arm::evaluate(
            "tier",
            script,
            corpus(&[("gage/canyon-outlet", 12.5), ("gage/high-bridge", 3.0)]),
            budget::DEFAULT_CALLS,
        )
        .expect("a worked calculator runs");
        let rows: Vec<(&str, &str)> = out
            .computed
            .signals
            .iter()
            .map(|r| {
                let v = match &r.values[0].value {
                    Value::Text(s) => s.as_str(),
                    other => panic!("expected Text, got {other:?}"),
                };
                (r.node.as_str(), v)
            })
            .collect();
        assert_eq!(
            rows,
            vec![("gage/canyon-outlet", "far"), ("gage/high-bridge", "near")]
        );
        // Exact, because `budget::DEFAULT_CALLS`'s note cites this figure to argue the default
        // is headroom rather than a limit. A gluon upgrade that changes how many calls a fold
        // costs should fail here and have the note corrected, not leave it citing a measurement
        // nobody took again.
        assert_eq!(
            out.calls, 75,
            "the worked example now costs {} calls, and `budget.rs` cites 75",
            out.calls
        );
        assert!(
            out.calls < budget::DEFAULT_CALLS / 1_000,
            "a two-node calculator spent {} of a {} call budget, so the budget is not the \
             headroom its note claims",
            out.calls,
            budget::DEFAULT_CALLS
        );
    }

    /// The prelude is one physical line, so a type error's line number is the script's own.
    ///
    /// Not a style point. A prelude on its own lines would shift every number a calculator
    /// author reads by exactly that many, quietly and in the wrong direction.
    #[test]
    fn a_type_error_is_reported_at_the_script_s_own_line() {
        let p = gluon_arm::prelude();
        assert!(!p.contains('\n'), "the prelude spans lines:\n{p}");
        let err =
            accepts("\\c ->\n  { signals = nope, summary = [] }").expect_err("`nope` is undefined");
        let msg = err.to_string();
        assert!(
            msg.contains(":2:"),
            "the error is not reported on the script's line 2:\n{msg}"
        );
    }

    /// `Computed` carries no `format_version`.
    ///
    /// The signal-table contract version belongs to the binary that writes the file. A script
    /// that could declare one could declare a version this binary does not read, which
    /// `Signals::load` would then refuse for a reason the calculator author never chose.
    #[test]
    fn a_script_cannot_declare_the_signal_table_version() {
        let vm = new_vm();
        let rendered = flat(&<Computed as VmType>::make_type(&vm).to_string());
        assert!(
            !rendered.contains("format_version"),
            "`Computed` exposes the table's contract version to the script: {rendered}"
        );
    }
}
