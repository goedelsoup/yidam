//! `mise run verify` checks every formal spec, and something runs it.
//!
//! The task named three files and there are four. That is survivable only while the task
//! runs; #461 found it had never run at all — no `dir`, so `lake build` looked for a lakefile
//! four directories above the one that has it; a lakefile that did not elaborate; and neither
//! tool installed by anything. A spec added tomorrow would have been verified by nobody, in
//! silence, exactly as these were.
//!
//! **Both sides are discovered.** The specs come from walking the directory and the checked
//! set comes from parsing the task, and neither is a list in this file — the rule
//! `cli_reference.rs` states one level up:
//!
//! > A hardcoded roster is the same rot one level up: it would stop covering new commands
//! > without ever going red.
//!
//! The two languages need different questions asked of them. Dafny takes a file per
//! invocation, so the task must name each `.dfy`. Lake takes a library and follows imports,
//! so naming each `.lean` would be wrong — the property there is *reachability*: every source
//! is a declared root or is imported, transitively, from one. A `.lean` that is neither still
//! compiles for whoever opens it in an editor and is checked by no build.
//!
//! What the tests above do not check is that the specs say anything true. Two of the six
//! claims their headers carried were an axiom or a `True` — a green check standing for a
//! claim nobody had made. #499 discharged both, and the census at the bottom of this file is
//! what keeps them discharged: it counts the constructs that assume rather than prove, over
//! text with the comments and string literals stripped out, and the count is zero.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SPEC_DIR: &str = "yidam/prelude/sdks/spec";

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// The `[tasks.verify]` table, parsed rather than grepped.
///
/// Parsed, because the three things asserted below are structure: which directory the task
/// runs in, which files its command lines name, and that CI invokes the task rather than
/// reproducing it. A regex over the file answers none of those and passes on a task that has
/// been commented out.
fn verify_task() -> toml::Table {
    let mise: toml::Table = read("mise.toml")
        .parse()
        .expect("mise.toml does not parse as TOML");
    mise.get("tasks")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("verify"))
        .and_then(|t| t.as_table())
        .cloned()
        .expect("mise.toml declares no [tasks.verify]")
}

fn verify_run_lines() -> Vec<String> {
    match verify_task().get("run") {
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("a non-string in verify's run list")
                    .to_string()
            })
            .collect(),
        Some(toml::Value::String(s)) => vec![s.clone()],
        other => panic!("[tasks.verify] has no usable `run` ({other:?})"),
    }
}

/// Files directly under the spec directory with the given extension, sorted.
fn spec_files(ext: &str) -> Vec<String> {
    let dir = repo_root().join(SPEC_DIR);
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext))
        .map(|p| {
            p.file_name()
                .expect("a file with no name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    out.sort();
    assert!(
        !out.is_empty(),
        "no *.{ext} under {SPEC_DIR} — this test is asserting nothing"
    );
    out
}

/// The task runs where the specs are, so its command lines can name them plainly.
///
/// Without this the task ran from the repository root, `lake` found no lakefile, and the
/// error was about a missing configuration file rather than about a spec.
#[test]
fn the_verify_task_runs_in_the_directory_that_holds_the_specs() {
    let dir = verify_task()
        .get("dir")
        .and_then(|d| d.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            panic!(
                "[tasks.verify] declares no `dir`, so mise runs it from the repository root \
                 where there is no lakefile — the #461 failure exactly"
            )
        });

    assert_eq!(
        dir.trim_end_matches('/'),
        SPEC_DIR,
        "[tasks.verify] runs in `{dir}`, which is not where the specs live"
    );
}

/// Every Dafny spec is named by the task. Dafny verifies one file per invocation, so a spec
/// the task does not name is a spec nothing checks.
#[test]
fn every_dafny_spec_is_verified_by_the_task() {
    let run = verify_run_lines();
    let dafny_args: BTreeSet<String> = run
        .iter()
        .filter(|line| line.split_whitespace().next() == Some("dafny"))
        .flat_map(|line| {
            line.split_whitespace()
                .skip(2) // `dafny verify`
                .filter(|a| !a.starts_with('-'))
                .map(str::to_string)
        })
        .collect();

    let present: BTreeSet<String> = spec_files("dfy").into_iter().collect();

    let unchecked: Vec<&String> = present.difference(&dafny_args).collect();
    assert!(
        unchecked.is_empty(),
        "{SPEC_DIR} holds {} Dafny spec(s) that [tasks.verify] does not name: {:?}\n\
         `mise run verify` is green without ever reading them.",
        unchecked.len(),
        unchecked
    );

    // And the other direction: a name the task carries for a file that is gone verifies
    // nothing and fails loudly only when somebody runs it.
    let stale: Vec<&String> = dafny_args.difference(&present).collect();
    assert!(
        stale.is_empty(),
        "[tasks.verify] names {stale:?}, which {SPEC_DIR} does not have"
    );
}

/// Every LEAN source is reached by the build.
///
/// Lake compiles a library from its declared `roots` and whatever they import, so the
/// question is not whether the task names each file — it names none of them — but whether
/// each file is reachable. An unreachable `.lean` type-checks in an editor and is compiled by
/// no CI, which is the same silence as an unnamed `.dfy` wearing different clothes.
#[test]
fn every_lean_source_is_reachable_from_a_declared_root() {
    let lakefile = read(&format!("{SPEC_DIR}/lakefile.lean"));

    // `roots := #[`Core]` — module names, backtick-quoted, in an array literal.
    let roots: BTreeSet<String> = lakefile
        .lines()
        .find(|l| l.trim_start().starts_with("roots"))
        .map(|l| {
            l.split('`')
                .skip(1)
                .step_by(2)
                .map(|m| m.trim_end_matches(']').trim().to_string())
                .filter(|m| !m.is_empty())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !roots.is_empty(),
        "lakefile.lean declares no `roots`, so `lake build Yidam` compiles nothing"
    );

    // Walk imports from the roots. A module name maps to `<name>.lean` in this flat package
    // (`srcDir := "."`), which is the only layout this directory has ever had.
    let module_path = |m: &str| repo_root().join(SPEC_DIR).join(format!("{m}.lean"));
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = roots.iter().cloned().collect();
    while let Some(m) = queue.pop_front() {
        if !reached.insert(m.clone()) {
            continue;
        }
        let p = module_path(&m);
        assert!(
            p.exists(),
            "lakefile.lean reaches module `{m}`, and {SPEC_DIR}/{m}.lean does not exist"
        );
        let text = std::fs::read_to_string(&p).expect("a module that exists is readable");
        for line in text.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("import ") {
                if let Some(name) = rest.split_whitespace().next() {
                    queue.push_back(name.to_string());
                }
            }
        }
    }

    let orphans: Vec<String> = spec_files("lean")
        .into_iter()
        .filter(|f| f != "lakefile.lean")
        .filter(|f| !reached.contains(f.trim_end_matches(".lean")))
        .collect();

    assert!(
        orphans.is_empty(),
        "{SPEC_DIR} holds {} LEAN source(s) no root reaches: {:?}\n\
         `lake build Yidam` never compiles them.",
        orphans.len(),
        orphans
    );
}

/// The toolchain the specs are checked under is written down.
///
/// The lakefile was written against a Lake that accepted `name := \"Yidam\"` as a String and
/// stopped elaborating under one that wants a `Name`. Nothing recorded which, so the file
/// could not even be dated. `lean-toolchain` is elan's own pin and the only one — see the
/// note beside `[tasks.verify]` for why there is not a second.
#[test]
fn the_lean_toolchain_is_pinned() {
    let pin = read(&format!("{SPEC_DIR}/lean-toolchain"));
    let pin = pin.trim();
    assert!(
        pin.starts_with("leanprover/lean4:"),
        "{SPEC_DIR}/lean-toolchain reads `{pin}`, which names no toolchain"
    );
}

/// Workflow text with its comments removed.
///
/// Necessary, and found the way these things are found: the first version of the test below
/// scanned the whole file, and deleting the `- run: mise run verify` step left it green —
/// because the job's own comment says the words "mise run verify" while explaining why the
/// step is there. A guard satisfied by prose about the code is not reading the code.
fn code_only(yaml: &str) -> String {
    yaml.lines()
        .map(|line| match line.split_once('#') {
            // A `#` at the start of the content, or after a space, opens a comment. Inside a
            // quoted value it would not — no workflow here has one, and the assertions below
            // are about step commands, which are the lines least likely to grow one.
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => {
                before.to_string()
            }
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A workflow runs the task, and runs *the task* rather than a copy of it.
///
/// This is the whole of #461 in one assertion. `verify` was declared, was documented in
/// `prelude/sdks/README.md`, and no workflow mentioned dafny, lake, or lean — so the
/// specification had a checker on paper and none in fact.
///
/// It must be `mise run verify` and not the three commands spelled out again, because a
/// workflow that reproduces the task is a second answer to "what is verified" and drifts from
/// the first without either going red. `ci.yml` already carries that argument about the
/// cross-compile matrix mirroring `release.yml`.
#[test]
fn a_workflow_runs_the_verification_task() {
    let workflows = repo_root().join(".github/workflows");
    let mut runners: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&workflows).expect("no .github/workflows") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|e| e != "yml") {
            continue;
        }
        let text = code_only(&std::fs::read_to_string(&path).expect("an unreadable workflow"));
        let name = path
            .file_name()
            .expect("a file with no name")
            .to_string_lossy()
            .into_owned();
        if text.contains("mise run verify") {
            runners.push((name, text));
        }
    }

    assert!(
        !runners.is_empty(),
        "no workflow runs `mise run verify`. The specs are checked by nothing, which is \
         the state #461 found and the state this test exists to keep the repository out of."
    );

    // The Lean version is pinned once, in `lean-toolchain`. A workflow naming its own is a
    // second pin, and two pins for one toolchain drift silently — the shape #397 cost a
    // release when `releases/latest` answered for four layers at once.
    for (name, text) in &runners {
        let hardcoded: Vec<&str> = text
            .lines()
            .filter(|l| l.contains("leanprover/lean4:"))
            .map(str::trim)
            .collect();
        assert!(
            hardcoded.is_empty(),
            "{name} names a Lean toolchain of its own ({hardcoded:?}); `lean-toolchain` is \
             the pin, and a second one drifts from it in silence"
        );
    }
}

/// The prose that documents the task documents the task that exists.
///
/// `prelude/sdks/README.md` carries a copy of `[tasks.verify]` as an example. It carried the
/// broken one — same missing `dir`, same three-of-four file list — which is how a reader
/// checked the documentation against the config and found them in agreement, both wrong.
///
/// Both extensions, and case-sensitively. The Lean half was added after the README's file
/// tree was found naming `core.lean` for a file that has been `Core.lean` since it was
/// written: on a case-insensitive filesystem nothing notices, and on CI's nothing looked.
#[test]
fn the_documented_verify_task_names_the_specs_the_real_one_does() {
    let readme = read("yidam/prelude/sdks/README.md");
    for ext in ["dfy", "lean"] {
        for spec in spec_files(ext) {
            assert!(
                readme.contains(&spec),
                "prelude/sdks/README.md never mentions {spec}, which {SPEC_DIR} holds"
            );
        }
    }
}

// ── The census (#499) ────────────────────────────────────────────────────────
//
// `mise run verify` running is one thing; what a green run stands for is another. Two of the
// six claims the specs are headed with used to be a `{:axiom}` or a conclusion of `True`, and
// `dafny verify` counted an axiom toward "13 verified" exactly as it counts a proof. #499
// discharged both. This is what keeps them discharged.
//
// A budget, not a roster. The count is asserted to be zero and the failure message says what
// to do if an assumption is ever genuinely wanted — state it in the file's header claim table
// and change the number here on purpose. A list of *which* axioms are permitted is the thing
// that rots: it stops covering new declarations without ever going red, which is the same
// fault one level up that made this test's neighbours discover both of their sides.

/// The line-comment token and block-comment delimiters of a spec language.
struct Syntax {
    line: &'static str,
    block_open: &'static str,
    block_close: &'static str,
}

fn syntax(ext: &str) -> Syntax {
    match ext {
        "dfy" => Syntax {
            line: "//",
            block_open: "/*",
            block_close: "*/",
        },
        "lean" => Syntax {
            line: "--",
            block_open: "/-",
            block_close: "-/",
        },
        other => panic!("no comment syntax recorded for *.{other} — the census cannot read it"),
    }
}

/// Comment bodies and string contents blanked, everything else left where it was.
///
/// Both halves are load-bearing, and both are the reason the self-test below exists.
///
/// Comments, because `graph.dfy`'s own header names `{:axiom}` twice while explaining that
/// the axioms are gone: a census that reads prose reports the file that was written to clear
/// it. `Core.lean` says `True` four times for the same reason.
///
/// Strings, because the same file holds `"https://"`, `"-->"` and `"<!-- /REGEN -->"`.
/// Blanking from a `//` or a `--` without tracking string state deletes real code, and a
/// census over a file with holes in it is a census that finds nothing.
fn spec_code_only(text: &str, syn: &Syntax) -> String {
    let chars: Vec<char> = text.chars().collect();
    let starts = |i: usize, pat: &str| {
        pat.chars()
            .enumerate()
            .all(|(k, c)| chars.get(i + k) == Some(&c))
    };
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let blank = |out: &mut Vec<char>, c: char| out.push(if c == '\n' { '\n' } else { ' ' });

    let mut i = 0;
    while i < chars.len() {
        if starts(i, syn.block_open) {
            let opener = syn.block_open.chars().count();
            let mut j = i + opener;
            while j < chars.len() && !starts(j, syn.block_close) {
                j += 1;
            }
            let end = (j + syn.block_close.chars().count()).min(chars.len());
            for c in chars.iter().take(end).skip(i) {
                blank(&mut out, *c);
            }
            i = end;
        } else if starts(i, syn.line) {
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut out, chars[i]);
                i += 1;
            }
        } else if chars[i] == '"' {
            blank(&mut out, chars[i]);
            i += 1;
            // A string literal ends at its closing quote or, if it is unterminated, at the end
            // of the line — running to the end of the file would blank everything after a
            // stray quote and leave the census reading nothing.
            while i < chars.len() && chars[i] != '"' && chars[i] != '\n' {
                let escape = chars[i] == '\\';
                blank(&mut out, chars[i]);
                i += 1;
                if escape && i < chars.len() {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
            if i < chars.len() && chars[i] == '"' {
                blank(&mut out, chars[i]);
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// Whether a character can sit inside an identifier in either language. Lean's `huv'` and
/// Dafny's `nodes'` both carry a prime, which is why `'` is here and why neither language's
/// char literals are treated as strings above: `'` is far more often part of a name.
fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\'' || c == '?' || c == '!'
}

/// Occurrences of `needle`, ignoring ones glued to a longer identifier.
///
/// Without the boundary check `True` matches `TrueName` and `axiom` matches `axiomatised`,
/// and a census that fires on a word inside another word gets switched off.
fn occurrences(haystack: &str, needle: &str) -> usize {
    let hay: Vec<char> = haystack.chars().collect();
    let pat: Vec<char> = needle.chars().collect();
    let lead = pat.first().copied().is_some_and(ident_char);
    let trail = pat.last().copied().is_some_and(ident_char);
    let mut n = 0;
    let mut i = 0;
    while i + pat.len() <= hay.len() {
        if hay[i..i + pat.len()] == pat[..]
            && !(lead && i > 0 && ident_char(hay[i - 1]))
            && !(trail && hay.get(i + pat.len()).copied().is_some_and(ident_char))
        {
            n += 1;
        }
        i += 1;
    }
    n
}

/// Everything a spec can say instead of proving something, and why each one is that.
///
/// This roster is inverted on purpose: it names the *constructs*, not the declarations that
/// are allowed to use them. It grows only when a language grows a new way to skip work, which
/// is rare — an exemption list would need an entry every time a spec did.
fn assumption_markers(ext: &str) -> &'static [(&'static str, &'static str)] {
    match ext {
        "dfy" => &[
            (
                "{:axiom}",
                "Dafny assumes the declaration and counts it toward `n verified` as if it \
                 had proved it — the #499 fault exactly",
            ),
            (
                "{:verify false}",
                "verification switched off for this declaration",
            ),
            ("{:extern}", "the body is elsewhere and Dafny cannot see it"),
            ("assume", "an assumed statement inside a proof"),
        ],
        "lean" => &[
            ("sorry", "an unfinished proof that still elaborates"),
            ("axiom", "an assumed declaration"),
            (
                "native_decide",
                "the kernel is asked to trust a compiled evaluation it did not perform",
            ),
            (
                "True",
                "a conclusion, hypothesis or field of `True` discharges nothing; \
                 `additive_augmentations_do_not_contradict` was `True` for as long as it existed",
            ),
        ],
        _ => &[],
    }
}

/// Nothing in the specs is assumed, and nothing concludes `True`.
#[test]
fn no_spec_claims_more_than_it_proves() {
    let mut checked = 0usize;
    let mut findings: Vec<String> = Vec::new();

    for ext in ["dfy", "lean"] {
        let syn = syntax(ext);
        for name in spec_files(ext) {
            let raw = read(&format!("{SPEC_DIR}/{name}"));
            let code = spec_code_only(&raw, &syn);

            // A stripper that blanked the file would make every count zero and every
            // assertion below vacuous. The specs all declare something.
            assert!(
                code.contains("lemma") || code.contains("theorem") || code.contains("lean_lib"),
                "{name}: after stripping comments and strings there is no declaration left, \
                 so the census below is reading an empty file"
            );
            checked += 1;

            for (marker, why) in assumption_markers(ext) {
                let n = occurrences(&code, marker);
                if n > 0 {
                    findings.push(format!("  {name}: {n}× `{marker}` — {why}"));
                }
            }
        }
    }

    assert!(
        checked >= 3,
        "the census read {checked} spec file(s); {SPEC_DIR} has more"
    );
    assert!(
        findings.is_empty(),
        "{SPEC_DIR} carries {} assumed or vacuous construct(s):\n{}\n\n\
         Every claim these files are headed with is supposed to be one `mise run verify` \
         establishes. If an assumption is genuinely wanted, say so in the file's header claim \
         table — the way graph.dfy and Core.lean now say what they prove — and change this \
         test deliberately rather than around it.",
        findings.len(),
        findings.join("\n")
    );
}

/// The census reads code, and not the prose about it.
///
/// The whole test above is its stripper, and a stripper is exactly the kind of thing that
/// passes by doing nothing. Both directions are checked here: a marker in a comment is not
/// counted, a marker in code is, and the code around a string holding `//` or `-->` survives.
#[test]
fn the_census_reads_code_and_not_the_prose_about_it() {
    let dfy = syntax("dfy");
    let lean = syntax("lean");

    // Comments are not code — the case the real files depend on.
    let commented = "// This file used to carry {:axiom} and no longer does.\nlemma L() {}\n";
    let code = spec_code_only(commented, &dfy);
    assert_eq!(
        occurrences(&code, "{:axiom}"),
        0,
        "a marker in a comment was counted"
    );
    assert!(
        code.contains("lemma L"),
        "the code beside the comment was blanked: {code:?}"
    );

    // …and code is.
    let real = "lemma {:axiom} L()\n  ensures true\n";
    assert_eq!(
        occurrences(&spec_code_only(real, &dfy), "{:axiom}"),
        1,
        "a marker in code was not counted — the census cannot fail"
    );

    // A `//` inside a string does not open a comment.
    let url = "predicate P(t: string) { HasPrefix(t, \"https://\") || Q(t) }\n";
    let stripped = spec_code_only(url, &dfy);
    assert!(
        stripped.contains("|| Q(t) }"),
        "a `//` inside a string blanked the rest of the line: {stripped:?}"
    );

    // A `--` inside a string does not open a Lean comment, and `/-- … -/` is a comment.
    let arrow = "def a := \"-->\"\ndef b := 1\n";
    assert!(
        spec_code_only(arrow, &lean).contains("def b := 1"),
        "a `--` inside a string blanked the rest of the file"
    );
    let doc = "/-- A doc comment mentioning sorry. -/\ntheorem T : 1 = 1 := rfl\n";
    let stripped = spec_code_only(doc, &lean);
    assert_eq!(
        occurrences(&stripped, "sorry"),
        0,
        "a doc comment was read as code"
    );
    assert!(
        stripped.contains("theorem T"),
        "the declaration after a doc comment was blanked"
    );

    // Word boundaries: a marker inside a longer name is not a marker.
    assert_eq!(occurrences("theorem TrueName : P", "True"), 0);
    assert_eq!(occurrences("theorem T : True := trivial", "True"), 1);
    assert_eq!(occurrences("lemma Assumed()", "assume"), 0);
}
