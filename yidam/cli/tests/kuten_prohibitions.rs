//! A kuten may not widen the model — RFC-0028 §7, guarded, one guard per prohibition.
//!
//! Five prohibitions, transcribed from the epic that settled them: a kuten may not add a
//! commit verb, add or alter a claim standing, contradict Articles I–VI, change the graph
//! encoding, or loosen a gate quietly. Each has its own check below, and each check reads a
//! **parsed profile** rather than a file.
//!
//! # Why parsed and never grepped
//!
//! *"A guard that greps a whole file is satisfied by that file's own comments."* A profile
//! that added a verb would very plausibly explain itself in a comment beside the addition —
//! and a text scan looking for the words would then find the explanation and call the file
//! clean, or find the explanation and call a clean file dirty. Both failures are silent.
//!
//! So every check here takes `serde_yaml::Value`, which drops comments before the check ever
//! runs, and [`a_comment_can_neither_trip_nor_satisfy_a_guard`] holds that property by
//! mutation rather than by this paragraph.
//!
//! # Both sets are discovered
//!
//! The **profiles** are whatever is on disk under `yidam/prelude/kuten/`, so a profile added
//! tomorrow is guarded by default. The **slot allowlist** is read out of RFC-0028's own slot
//! table, so widening the model means editing the specification that forbids it rather than
//! a list in a test file. A hardcoded roster on either side stops covering new material
//! without ever going red.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_yaml::Value;
use yidam_core::git::is_recognized_verb;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn kuten_dir() -> PathBuf {
    repo_root().join("yidam/prelude/kuten")
}

/// Every profile this repository ships: its name, and its parsed declaration.
///
/// Discovered from the directory. A profile is a subdirectory holding `kuten.yml`; the
/// layer's own `README.md` is not one and is skipped by that rule rather than by name.
fn profiles() -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let entries = std::fs::read_dir(kuten_dir()).expect("yidam/prelude/kuten/ exists");
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path().join("kuten.yml");
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
        let doc = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} is not YAML ({e})", path.display()));
        out.insert(name, doc);
    }
    out
}

/// The floor. A scan that finds nothing satisfies every question asked of it, which is how a
/// guard goes green while guarding none of its subject.
#[test]
fn there_is_something_to_guard() {
    let found = profiles();
    assert!(
        !found.is_empty(),
        "no profile found under {} — every check in this file is passing vacuously",
        kuten_dir().display()
    );
    for name in found.keys() {
        let doc = kuten_dir().join(name).join("KUTEN.md");
        assert!(
            doc.is_file(),
            "`{name}` declares a profile and ships no document a person can read: {}",
            doc.display()
        );
    }
}

// ── walking a parsed profile ──────────────────────────────────────────────────

/// Every `(dotted path, key)` in the document, at any depth.
fn keys(doc: &Value) -> Vec<(String, String)> {
    fn walk(v: &Value, at: &str, out: &mut Vec<(String, String)>) {
        match v {
            Value::Mapping(m) => {
                for (k, child) in m {
                    let Some(name) = k.as_str() else { continue };
                    let path = if at.is_empty() {
                        name.to_string()
                    } else {
                        format!("{at}.{name}")
                    };
                    out.push((path.clone(), name.to_string()));
                    walk(child, &path, out);
                }
            }
            Value::Sequence(items) => {
                for item in items {
                    walk(item, at, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(doc, "", &mut out);
    out
}

/// Every `(dotted path, scalar)` in the document, at any depth.
fn scalars(doc: &Value) -> Vec<(String, String)> {
    fn walk(v: &Value, at: &str, out: &mut Vec<(String, String)>) {
        match v {
            Value::Mapping(m) => {
                for (k, child) in m {
                    let Some(name) = k.as_str() else { continue };
                    let path = if at.is_empty() {
                        name.to_string()
                    } else {
                        format!("{at}.{name}")
                    };
                    walk(child, &path, out);
                }
            }
            Value::Sequence(items) => {
                for item in items {
                    walk(item, at, out);
                }
            }
            Value::String(s) => out.push((at.to_string(), s.clone())),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(doc, "", &mut out);
    out
}

fn parse(yaml: &str) -> Value {
    serde_yaml::from_str(yaml).expect("the test document parses")
}

// ── prohibition 1: add a commit verb ──────────────────────────────────────────

/// Verbs a profile declares that the closed vocabulary does not carry.
///
/// The list it is held to is `yidam_core`'s, which is the same list `lint --commits` and the
/// parity fixtures in three SDKs are held to. Reading GRAPH.md's table here would be a second
/// opinion about what the vocabulary is.
fn added_verbs(doc: &Value) -> Vec<String> {
    doc.get("vocabulary")
        .and_then(|v| v.get("verbs"))
        .and_then(Value::as_sequence)
        .map(|verbs| {
            verbs
                .iter()
                .filter_map(Value::as_str)
                .filter(|v| !is_recognized_verb(v))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn no_profile_adds_a_commit_verb() {
    for (name, doc) in profiles() {
        assert!(
            added_verbs(&doc).is_empty(),
            "`{name}` declares {:?}, which the closed vocabulary does not carry. A kuten may \
             declare a subset and gloss it; a needed-and-absent verb is evidence for the \
             forum, not a patch.",
            added_verbs(&doc)
        );
    }
}

#[test]
fn the_added_verb_guard_catches_one() {
    let doc = parse("kuten: x\nrevision: 1\nvocabulary:\n  verbs: [establish, curate]\n");
    assert_eq!(added_verbs(&doc), vec!["curate".to_string()]);
}

// ── prohibition 2: add or alter a claim standing ──────────────────────────────

/// The three standings, in the bracketed form the model writes them in.
///
/// The bracket is what makes this checkable at all: `open` is also a *verb* in the closed
/// vocabulary, so a bare scalar cannot tell a standing from a legitimate declaration. The
/// syntactic form is unambiguous, and it is the form a profile would have to use to redefine
/// one.
const STANDINGS: &[&str] = &["[verified]", "[inference]", "[open]"];

fn standing_declarations(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (path, key) in keys(doc) {
        if key == "standing" || key == "standings" {
            out.push(path);
        }
    }
    for (path, text) in scalars(doc) {
        if STANDINGS.iter().any(|s| text.contains(s)) {
            out.push(path);
        }
    }
    out
}

#[test]
fn no_profile_names_a_claim_standing() {
    for (name, doc) in profiles() {
        assert!(
            standing_declarations(&doc).is_empty(),
            "`{name}` names a claim standing at {:?}. Article V reads the standings as a total \
             order when it licenses lowering a claim at resolution; a kuten's answer here is \
             nothing, and that is constitutional rather than conservative.",
            standing_declarations(&doc)
        );
    }
}

#[test]
fn the_standing_guard_catches_both_forms() {
    let by_key = parse("kuten: x\nrevision: 1\nclasses:\n  standings: [a, b]\n");
    assert_eq!(standing_declarations(&by_key), vec!["classes.standings"]);
    let by_value = parse("kuten: x\nrevision: 1\ngloss: prefers [verified] over [open]\n");
    assert_eq!(standing_declarations(&by_value), vec!["gloss"]);
    // And a bare `open` is the commit verb, not a standing.
    let verb = parse("kuten: x\nrevision: 1\nvocabulary:\n  verbs: [open, close]\n");
    assert!(standing_declarations(&verb).is_empty());
}

// ── prohibition 3: contradict Articles I–VI ───────────────────────────────────

/// The header keys every profile carries, which are not slots.
///
/// A profile names itself, says which revision it is, glosses itself in one line, and records
/// where its numbers came from. None of those parameterizes the loop, so none is in the slot
/// table — and each has to be licensed here rather than by widening the table.
const PROFILE_HEADER: &[&str] = &["kuten", "revision", "gloss", "measured"];

/// The slots RFC-0028 §1 names, read out of its own table.
///
/// **Discovered from the specification, not listed here.** A profile that declared something
/// the layer does not name would be a kuten deciding what a kuten may decide, and the check
/// that catches it must be anchored to the document that settles the question — otherwise
/// widening the model costs one line in a test file.
fn declared_slots() -> BTreeSet<String> {
    let path = repo_root().join("docs/rfcs/0028-kuten-layer.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let slots = slot_table(&text);
    assert!(
        slots.len() >= 8,
        "read only {} slot(s) out of RFC-0028's table — its shape changed and this is no \
         longer reading it: {slots:?}",
        slots.len()
    );
    slots
}

/// Slot names from a markdown table whose first column header is `Slot`.
///
/// The RFC writes a cell as `**phases** — the valid phase types` and the vendored layer
/// document writes it as `` `phases` ``, so the name is read out of whichever run opens the
/// cell. `question-pressure` is hyphenated in prose and underscored as a key, and the two are
/// reconciled here rather than in every caller.
fn slot_table(text: &str) -> BTreeSet<String> {
    first_column(text, "Slot")
}

/// The first column of every body row of the markdown table whose first header cell is
/// `header`, unwrapped from whichever of `**bold**` or `` `code` `` opens the cell.
///
/// One parser, because the layer names three sets this way — the slots, their readers, and
/// the question-pressure kinds — and three copies of "read a markdown table" is how two of
/// them come to disagree about what a row is.
fn first_column(text: &str, header: &str) -> BTreeSet<String> {
    table_rows(text, header)
        .into_iter()
        .filter_map(|cells| cell_name(cells.first().copied().unwrap_or_default()))
        .collect()
}

/// The body rows of the table whose first header cell is `header`, as cell slices.
fn table_rows<'a>(text: &'a str, header: &str) -> Vec<Vec<&'a str>> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            inside = false;
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.first().copied() == Some(header) {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        out.push(cells);
    }
    out
}

/// The name a table cell opens with, unwrapped from `**` or a backtick. `None` for the
/// `|---|` separator row and for any cell that does not open with a delimited name.
fn cell_name(cell: &str) -> Option<String> {
    ["**", "`"].iter().find_map(|delim| {
        cell.strip_prefix(delim)
            .and_then(|rest| rest.split_once(delim))
            .map(|(name, _)| name.trim().replace('-', "_"))
    })
}

fn unknown_slots(doc: &Value, allowed: &BTreeSet<String>) -> Vec<String> {
    let Some(map) = doc.as_mapping() else {
        return vec!["the profile is not a mapping".to_string()];
    };
    map.keys()
        .filter_map(Value::as_str)
        .filter(|k| !PROFILE_HEADER.contains(k) && !allowed.contains(*k))
        .map(str::to_string)
        .collect()
}

#[test]
fn no_profile_declares_a_slot_the_layer_does_not_name() {
    let allowed = declared_slots();
    for (name, doc) in profiles() {
        let unknown = unknown_slots(&doc, &allowed);
        assert!(
            unknown.is_empty(),
            "`{name}` declares {unknown:?}, which RFC-0028's slot table does not name. A kuten \
             narrows and parameterizes the loop; a slot the layer never licensed is the loop \
             being widened from inside a profile."
        );
    }
}

/// The layer's own document must describe the same slots the RFC settles, or a reader of the
/// vendored text is being told about a different model than the one guarded here.
#[test]
fn the_vendored_layer_document_names_the_same_slots() {
    let readme = std::fs::read_to_string(kuten_dir().join("README.md")).expect("kuten/README.md");
    assert_eq!(
        slot_table(&readme),
        declared_slots(),
        "the vendored layer document and RFC-0028 name different slots"
    );
}

#[test]
fn the_unknown_slot_guard_catches_one() {
    let allowed: BTreeSet<String> = ["phases".to_string()].into_iter().collect();
    let doc = parse("kuten: x\nrevision: 1\nphases: {}\narticles: {V: relaxed}\n");
    assert_eq!(unknown_slots(&doc, &allowed), vec!["articles".to_string()]);
}

// ── prohibition 4: change the graph encoding ──────────────────────────────────

/// The premise, not a policy: files are nodes, links are edges, commits are events.
///
/// Matched on exact key names at any depth, so `classes.nodes_per_commit` — a band about how
/// fast a corpus accretes — is untouched while a `nodes:` that redefined what one is fails.
const ENCODING_KEYS: &[&str] = &[
    "graph", "encoding", "node", "nodes", "edge", "edges", "commit", "commits", "event", "events",
];

fn encoding_declarations(doc: &Value) -> Vec<String> {
    keys(doc)
        .into_iter()
        .filter(|(_, key)| ENCODING_KEYS.contains(&key.as_str()))
        .map(|(path, _)| path)
        .collect()
}

#[test]
fn no_profile_changes_the_graph_encoding() {
    for (name, doc) in profiles() {
        assert!(
            encoding_declarations(&doc).is_empty(),
            "`{name}` declares {:?}. Files are nodes, links are edges, commits are events — \
             that is the premise the model rests on, and a kuten's answer to it is nothing.",
            encoding_declarations(&doc)
        );
    }
}

#[test]
fn the_encoding_guard_catches_one_and_spares_a_band() {
    let bad = parse("kuten: x\nrevision: 1\nclasses:\n  nodes: [concept, question]\n");
    assert_eq!(encoding_declarations(&bad), vec!["classes.nodes"]);
    let fine =
        parse("kuten: x\nrevision: 1\nclasses:\n  nodes_per_commit: {low: 0.5, high: 1.0}\n");
    assert!(encoding_declarations(&fine).is_empty());
}

// ── prohibition 5: loosen a gate quietly ──────────────────────────────────────

/// The severities a finding can carry, plus the two words that switch one off.
const SEVERITIES: &[&str] = &["error", "warn", "warning", "info", "off", "allow", "deny"];

/// Severities declared anywhere but the `policy` slot.
///
/// A local rule may be more permissive and may not be *silent*. The policy slot is where a
/// proposed severity is visible three ways — `policy check`, an `Info` lint finding, and
/// `doctor` — so a severity anywhere else in a profile is a gate loosened where nothing
/// surfaces it.
fn severities_outside_policy(doc: &Value) -> Vec<String> {
    let outside = |path: &str| !path.starts_with("policy");
    let mut out: Vec<String> = keys(doc)
        .into_iter()
        .filter(|(path, key)| (key == "severity" || key == "severities") && outside(path))
        .map(|(path, _)| path)
        .collect();
    out.extend(
        scalars(doc)
            .into_iter()
            .filter(|(path, text)| {
                outside(path) && SEVERITIES.contains(&text.trim().to_ascii_lowercase().as_str())
            })
            .map(|(path, _)| path),
    );
    // `severity: off` trips both halves — the key and the value — and reporting one place
    // twice reads as two problems.
    out.sort();
    out.dedup();
    out
}

#[test]
fn no_profile_loosens_a_gate_outside_the_policy_slot() {
    for (name, doc) in profiles() {
        assert!(
            severities_outside_policy(&doc).is_empty(),
            "`{name}` sets a severity at {:?}, outside the policy slot. A rule that is more \
             permissive is allowed; a rule that is silently more permissive is not.",
            severities_outside_policy(&doc)
        );
    }
}

#[test]
fn the_severity_guard_catches_one_and_spares_the_policy_slot() {
    let bad = parse("kuten: x\nrevision: 1\nvocabulary:\n  severity: off\n");
    assert_eq!(severities_outside_policy(&bad), vec!["vocabulary.severity"]);
    let declared = parse(
        "kuten: x\nrevision: 1\npolicy:\n  proposes_overrides:\n    - check: unrecognized-verb\n      severity: warn\n",
    );
    assert!(severities_outside_policy(&declared).is_empty());
}

// ── the property every guard above depends on ────────────────────────────────

/// A comment can neither trip a guard nor satisfy one.
///
/// The mutation this file's design turns on, run rather than asserted in prose. A profile
/// explaining a prohibition in a comment must not be reported as breaking it, and a profile
/// that breaks one must not be excused by a comment that says it does not.
#[test]
fn a_comment_can_neither_trip_nor_satisfy_a_guard() {
    let commented = parse(
        "# This profile may not add a verb like `curate`, may not set severity: off, and may\n\
         # not declare nodes: or standings: or a slot named articles:.\n\
         kuten: x\nrevision: 1\n\
         vocabulary:\n  verbs: [establish]\n",
    );
    assert!(
        added_verbs(&commented).is_empty(),
        "a comment tripped a guard"
    );
    assert!(standing_declarations(&commented).is_empty());
    assert!(encoding_declarations(&commented).is_empty());
    assert!(severities_outside_policy(&commented).is_empty());

    let excused = parse(
        "kuten: x\nrevision: 1\n\
         # This adds no verb; `curate` below is only an illustration.\n\
         vocabulary:\n  verbs: [establish, curate]\n",
    );
    assert_eq!(
        added_verbs(&excused),
        vec!["curate".to_string()],
        "a comment excused a verb the profile actually declares"
    );
}

// ── the binding rule travels with the profile ─────────────────────────────────

/// Lines of the first blockquote in `text`, with HTML comments removed first.
///
/// Stripping the comments is what stops a profile from satisfying this by *quoting the rule
/// in a comment about the rule* — the same failure the YAML guards avoid by parsing.
fn first_blockquote(text: &str) -> String {
    let mut stripped = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        stripped.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    stripped.push_str(rest);

    let mut quote: Vec<String> = Vec::new();
    for line in stripped.lines() {
        let trimmed = line.trim();
        if let Some(body) = trimmed.strip_prefix('>') {
            quote.push(body.trim().to_string());
        } else if !quote.is_empty() && trimmed.is_empty() {
            break;
        }
    }
    quote.join(" ")
}

/// Every profile document opens with the binding rule, verbatim.
///
/// One canonical text — the layer README's — and each profile carries it rather than a
/// paraphrase. The rule lands in vendored text and not only in the RFC because it binds a
/// repository that will never read an RFC, and a paraphrase is how the two come to say
/// different things.
#[test]
fn every_profile_document_opens_with_the_binding_rule() {
    let readme = std::fs::read_to_string(kuten_dir().join("README.md")).expect("kuten/README.md");
    let rule = first_blockquote(&readme);
    assert!(
        rule.contains("may not widen the model") && rule.contains("binds nobody"),
        "the layer README no longer opens its binding rule with the rule: {rule:?}"
    );

    for name in profiles().keys() {
        let path = kuten_dir().join(name).join("KUTEN.md");
        let text = std::fs::read_to_string(&path).expect("a profile document");
        assert_eq!(
            first_blockquote(&text),
            rule,
            "`{name}`'s document does not open with the binding rule verbatim"
        );
    }
}

#[test]
fn the_binding_rule_check_reads_the_document_and_not_its_comments() {
    let quoted_in_a_comment = "# x\n\n<!--\n> the rule\n-->\n\nsome prose\n";
    assert_eq!(first_blockquote(quoted_in_a_comment), "");
    let real = "# x\n\n> the rule\n> continues\n\nsome prose\n";
    assert_eq!(first_blockquote(real), "the rule continues");
}

// ── the profile is what the binary reads ──────────────────────────────────────

/// The four `clocks` scalars are pinned, and the set of them is closed.
///
/// The bands next door are pinned by [`the_inquiry_profile_is_readable_by_the_binary`], which
/// parses through [`yidam::kuten::Profile`]. The scalars cannot be: `Profile` does not parse the
/// `clocks` slot at all, deliberately — `due` reads only `[due]` keys, and a second live home for
/// an interval is what RFC-0028 §9 forbids. The cost was that all four could be edited with
/// nothing going red, which is half of why #633 could say the proposal was unfalsifiable.
///
/// So this reads the document rather than the model. Changing a value here is allowed and is
/// what #633's retirement rule describes — three cluster members holding a key replaces the
/// scalar with their median — but it is not allowed to happen quietly, and adding a fifth
/// proposed key is a new interval the layer never argued for.
#[test]
fn the_proposed_clock_intervals_are_pinned_and_the_set_is_closed() {
    let text = std::fs::read_to_string(kuten_dir().join("inquiry/kuten.yml"))
        .expect("the inquiry profile");
    let doc: Value = serde_yaml::from_str(&text).expect("the profile is YAML");

    let proposed = doc
        .get("clocks")
        .and_then(|c| c.get("proposed"))
        .and_then(Value::as_mapping)
        .expect("`clocks.proposed` is a mapping");

    let mut got: Vec<(String, i64)> = proposed
        .iter()
        .map(|(k, v)| {
            let key = k.as_str().expect("a scalar key").to_string();
            let n = v
                .as_i64()
                .unwrap_or_else(|| panic!("`{key}` proposes {v:?}, which is not a whole interval"));
            (key, n)
        })
        .collect();
    got.sort();

    assert_eq!(
        got,
        vec![
            ("catalog.ttl_days".to_string(), 180),
            ("due.index_after".to_string(), 25),
            ("due.phases_after".to_string(), 60),
            ("due.questions_after".to_string(), 100),
        ],
        "the proposed intervals moved. That is a real change and may be the right one — but it \
         is #633's, and it lands with the measurement that retires the old value, the RFC-0028 \
         §1 row, and this assertion updated together"
    );
}

/// The shipped `inquiry` profile parses as the binary parses it, and carries the bands A0
/// measured. A profile the CLI cannot read is a declaration nothing consumes.
#[test]
fn the_inquiry_profile_is_readable_by_the_binary() {
    let path = kuten_dir().join("inquiry/kuten.yml");
    let text = std::fs::read_to_string(&path).expect("the inquiry profile");
    let profile = yidam::kuten::Profile::parse(&text).expect("the binary can read it");

    assert_eq!(profile.name, "inquiry");
    assert!(profile.revision >= 1);

    let phases = profile.phases.expect("the phases slot is populated");
    assert_eq!(phases.commit_share.low, 0.13);
    assert_eq!(phases.commit_share.high, 0.26);

    let classes = profile.classes.expect("the classes slot is populated");
    assert_eq!(classes.nodes_per_commit.low, 0.50);
    assert_eq!(classes.nodes_per_commit.high, 1.11);
    assert_eq!(classes.median_node_lines.low, 35.0);
    assert_eq!(classes.median_node_lines.high, 62.0);

    // The phase *types* are not counted here. A `len() == 4` was the only thing catching a
    // dropped type, and it caught it by knowing the answer — so it went on passing while
    // `PHASES.md` and the profile drifted in every other direction.
    // [`the_phase_type_enumeration_is_one_set_in_two_documents`] holds the set instead.

    let vocabulary = profile
        .vocabulary
        .expect("the vocabulary slot is populated");
    assert_eq!(
        vocabulary.off_vocabulary_share,
        yidam::kuten::Band {
            low: 0.0,
            high: 0.0
        },
        "exactly 0% in all six, and the two that were not are the object-coupled pair"
    );
}

// ── the phase-type enumeration ────────────────────────────────────────────────

/// The bold lead-ins inside one `##` section of a markdown document.
///
/// **A section, and never the document.** `PHASES.md` bolds a dozen phrases — *One phase, one
/// branch*, *Bound phases*, *Settle with a merge commit* — and a scan over the whole file
/// reads every one of them as a phase type. Restricting to the section that enumerates them,
/// and to the lead-in form the document actually uses (a bold run opening a paragraph), is
/// what makes this read the enumeration rather than the prose around it.
fn bold_lead_ins(text: &str, heading: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            inside = rest.trim() == heading;
            continue;
        }
        if !inside {
            continue;
        }
        let Some(name) = line
            .strip_prefix("**")
            .and_then(|rest| rest.split_once("**"))
            .map(|(name, _)| name.trim().to_string())
        else {
            continue;
        };
        out.insert(name);
    }
    out
}

/// **The phase types are one set written in two documents, and the two must be equal.**
///
/// This replaces a check that ran in one direction — every declared type appears somewhere in
/// `PHASES.md` — and a `len() == 4` beside it that caught a dropped type by knowing the
/// answer. Three ways of breaking the pair were green under that arrangement: adding a fifth
/// type to `PHASES.md`, deleting the section that enumerates them, and any bold sentence
/// elsewhere in the file standing in for a type that no longer exists. Set equality against
/// the *section* closes all three.
#[test]
fn the_phase_type_enumeration_is_one_set_in_two_documents() {
    let path = repo_root().join("yidam/prelude/PHASES.md");
    let phases = std::fs::read_to_string(&path).expect("PHASES.md");
    let described = bold_lead_ins(&phases, "Phase types");
    assert!(
        !described.is_empty(),
        "{} has no `## Phase types` section with bold lead-ins in it — this check is reading \
         nothing, and would pass against any profile at all",
        path.display()
    );

    let text = std::fs::read_to_string(kuten_dir().join("inquiry/kuten.yml")).unwrap();
    let profile = yidam::kuten::Profile::parse(&text).unwrap();
    let declared: BTreeSet<String> = profile
        .phases
        .expect("the phases slot is populated")
        .types
        .into_iter()
        .collect();

    assert_eq!(
        declared, described,
        "the profile declares {declared:?} and PHASES.md describes {described:?}. The \
         enumeration lives in both documents and a reader is entitled to find the same set in \
         either; a type in one and not the other is a practice nobody can run or a phase type \
         nobody can declare."
    );
}

/// The section reader ignores the document around the section it was asked for.
#[test]
fn the_lead_in_reader_reads_one_section_and_not_the_file() {
    let doc = "# Title\n\n**Preamble** — not a type.\n\n## Phase types\n\n\
               **Investigation** — a type.\n\n**Extraction** — a type.\n\n\
               ## Phase discipline\n\n**One phase, one branch** — not a type.\n";
    assert_eq!(
        bold_lead_ins(doc, "Phase types"),
        ["Investigation".to_string(), "Extraction".to_string()]
            .into_iter()
            .collect()
    );
    // A document with no such section yields nothing, so the guard above can tell "read the
    // section and found none" from "found the four".
    assert!(bold_lead_ins(doc, "Phase kinds").is_empty());
}

// ── the question-pressure kinds ───────────────────────────────────────────────

/// **The layer document names every kind the binary parses, and no others.**
///
/// `coverage` is reserved and unimplemented, which is exactly the state a reader will not
/// infer from a parser: a kind that exists in the model and in no document is a blank for
/// somebody to invent a meaning for, and a kind in the document that does not parse is a
/// declaration that fails at the profile it was written for.
#[test]
fn the_layer_document_names_every_question_pressure_kind() {
    let readme = std::fs::read_to_string(kuten_dir().join("README.md")).expect("kuten/README.md");
    let documented = first_column(&readme, "Kind");
    let modelled: BTreeSet<String> = yidam::kuten::PressureKind::ALL
        .iter()
        .map(|k| k.name().to_string())
        .collect();
    assert_eq!(
        documented, modelled,
        "the layer document and the parser name different question-pressure kinds"
    );

    // And each documented name is the name that parses — the table is the reader's spelling
    // guide, not a gloss beside one.
    for name in &documented {
        let doc = format!("kuten: x\nrevision: 1\nquestion_pressure:\n  kind: {name}\n");
        yidam::kuten::Profile::parse(&doc)
            .unwrap_or_else(|e| panic!("the document names `{name}` and it does not parse ({e})"));
    }
}

// ── every populated slot has a consumer ───────────────────────────────────────

/// Slot → the surfaces the layer document says read it, from the table's `Read by` column.
///
/// The tokens are a closed set — `check`, `block`, `none` — and an unknown one fails here
/// rather than being silently treated as no claim.
fn declared_readers() -> BTreeMap<String, BTreeSet<String>> {
    let readme = std::fs::read_to_string(kuten_dir().join("README.md")).expect("kuten/README.md");
    let column = read_by_column(&readme).expect("the slot table has a `Read by` column");

    let mut out = BTreeMap::new();
    for cells in table_rows(&readme, "Slot") {
        let Some(slot) = cells.first().copied().and_then(cell_name) else {
            continue;
        };
        let cell = cells.get(column).copied().unwrap_or_default();
        let mut tokens = BTreeSet::new();
        for piece in cell.split(',') {
            let Some(token) = cell_name(piece.trim()) else {
                continue;
            };
            assert!(
                ["check", "block", "none"].contains(&token.as_str()),
                "`{slot}` names `{token}` as a reader, which is not one of the surfaces a slot \
                 can reach: `check`, `block`, `none`"
            );
            tokens.insert(token);
        }
        assert!(
            !tokens.is_empty(),
            "`{slot}` names no reader at all. `none` is the way to say nothing reads it, and \
             saying it is the point"
        );
        assert!(
            !(tokens.contains("none") && tokens.len() > 1),
            "`{slot}` claims `none` and a reader in the same breath"
        );
        out.insert(slot, tokens);
    }
    out
}

/// The index of the `Read by` column in the slot table's header row.
fn read_by_column(text: &str) -> Option<usize> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .find(|l| {
            l.trim_start_matches('|')
                .split('|')
                .next()
                .is_some_and(|c| c.trim() == "Slot")
        })
        .and_then(|header| {
            header
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .position(|c| c.trim() == "Read by")
        })
}

/// **Every populated slot reaches a reader, and every claimed reader actually reads it.**
///
/// This is the mechanical form of the failure RFC-0028 names by name and this repository
/// keeps finding in itself: a surface with no consumer. A slot is populated in a vendored
/// profile, vendored into every derived corpus, and read by nothing — and nothing goes red,
/// because there is no assertion anywhere that a declaration has to arrive somewhere.
///
/// It is guarded in **both** directions, which is what makes it more than a spelling check.
/// Dropping a slot from the profile must change exactly the surfaces the layer document says
/// read it: a slot claiming `check` whose removal leaves `kuten check` identical is a
/// documented consumer that does not exist, and a slot claiming `none` whose removal changes
/// a report is a consumer nobody documented.
#[test]
fn every_populated_slot_has_a_consumer() {
    use yidam::kuten::{compare, render_block, Declaration, Measurement, Profile, Vintage};

    let readers = declared_readers();
    let text = std::fs::read_to_string(kuten_dir().join("inquiry/kuten.yml")).expect("inquiry");
    let doc: Value = serde_yaml::from_str(&text).expect("the profile is YAML");
    let full = Profile::parse(&text).expect("the binary can read it");
    let declaration = Declaration::parse("kuten: inquiry\nrevision: 1\n").unwrap();

    // A repository in the middle of every band, holding open questions, at a vintage that
    // gates nothing off. A probe that was vintage-exempt or empty would make two slots
    // report identically whatever they declare.
    let probe = Measurement {
        commits: 100,
        phase_commits: 20,
        off_vocabulary_commits: 0,
        nodes: 80,
        median_node_lines: Some(48.0),
        open_questions: 6,
    };
    let vintage = Vintage::read("| `phase` | settled |\n\nThis list is closed:\n");
    let check_of =
        |p: &Profile| serde_json::to_string(&compare(p, &probe, &vintage)).expect("findings");

    let slots: Vec<String> = doc
        .as_mapping()
        .expect("a mapping")
        .keys()
        .filter_map(Value::as_str)
        .filter(|k| !PROFILE_HEADER.contains(k))
        .map(str::to_string)
        .collect();
    assert!(
        slots.len() > 3,
        "read {} slot(s) out of the shipped profile — this is guarding almost nothing: {slots:?}",
        slots.len()
    );

    for slot in slots {
        let reduced: serde_yaml::Mapping = doc
            .as_mapping()
            .expect("a mapping")
            .iter()
            .filter(|(k, _)| k.as_str() != Some(slot.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let reduced = Value::Mapping(reduced);
        let without = Profile::parse(&serde_yaml::to_string(&reduced).expect("re-serialised"))
            .unwrap_or_else(|e| panic!("the profile without `{slot}` does not parse ({e})"));

        let mut observed = BTreeSet::new();
        if check_of(&full) != check_of(&without) {
            observed.insert("check".to_string());
        }
        if render_block(Some(&declaration), Some(&full))
            != render_block(Some(&declaration), Some(&without))
        {
            observed.insert("block".to_string());
        }
        if observed.is_empty() {
            observed.insert("none".to_string());
        }

        let declared = readers.get(&slot).unwrap_or_else(|| {
            panic!(
                "`{slot}` is populated in the profile and the layer document's slot table has \
                 no row for it, so nothing says whether anything reads it"
            )
        });
        assert_eq!(
            &observed, declared,
            "`{slot}` is documented as read by {declared:?} and is actually read by \
             {observed:?}. A slot populated in a vendored profile that reaches no report is a \
             declaration nothing consumes — the failure this layer exists to stop — and a \
             documented reader that does not read it is the same failure with a paper trail."
        );
    }
}
