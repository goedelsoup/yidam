//! The contract version moves when the contract does.
//!
//! Four copies of the MCP contract version are held to each other: `tools.json` declares it,
//! `mcp/VERSION` states it, `mcp/README.md`'s capability block prints it, and
//! `docs/mcp-server.md`'s handshake example shows it. `versioning_layers.rs` fails when any of
//! the four disagrees — and none of them is required to *move*. Two branches can each add a
//! tool, each ship `"contract": "0.24.0"`, and every gate stays green on both branches and on
//! the merge, because agreeing is all four copies do (#940). #906 was the prose copy of the
//! same hole and is closed; this is the mechanical one.
//!
//! `CONTRACT_SHA` is the missing half. One line per contract version, carrying a digest of the
//! document that version names, appended in the order the versions shipped. The number then
//! cannot be reused — reusing it means writing a second line for a version already in the file
//! — and two branches that each claim one number collide in that file at merge instead of
//! shipping.
//!
//! ## What the digest covers, and why it is the whole document
//!
//! #940 proposed hashing "tool names, tiers, param schemas, refusal tokens". That was measured
//! against the contract's own history before being implemented, and it is the wrong surface:
//! `tools.json` has 27 commits carrying 25 version bumps, and a digest over the structured
//! fields alone would have been **silent on 11 of the 25**. Adding the frozen vocabularies
//! parsed out of the prose brings it to silent on 8.
//!
//! The bumps it cannot see are not corner cases. `0.16.0 → 0.17.0` specified what an ordering
//! does when two dates disagree about precision — the difference between a server that refuses
//! the right queries and answers the right ones differently — and the entire change is two
//! `notes` paragraphs. No name, tier, schema or code string moved. `0.14.0 → 0.15.0` rewrote
//! three of nine frozen rejection codes, which a vocabulary parser catches only for as long as
//! the clause it keys on keeps its wording.
//!
//! So the digest is over the **whole document minus the version itself**: the contract is the
//! file, its prose is load-bearing (`tools.json` says so about its own `notes` — the frozen
//! enumerations live there deliberately, and a structured field beside them would be a second
//! freeze), and a projection that drops prose drops half of what the version is announcing.
//!
//! Its cost is measured too, and it is one commit in 27: `948c316` reworded a `notes`
//! paragraph at 0.16.0 without moving the version — which is this hole, once, in the history.
//! A document that changes under a version a client has already read is what the ledger exists
//! to make impossible, so the honest reading of that commit is that it owed a patch bump.
//!
//! ## Mutations it was checked against
//!
//! A digest is a number, and a number computed over the wrong bytes agrees with itself
//! forever. [`the_digest_moves_when_any_part_of_the_contract_does`] applies each of these to
//! the parsed document in memory and fails if the digest does not move, and
//! [`the_digest_is_blind_to_formatting_and_to_the_version_itself`] holds the two that must not
//! move it. Both run every time, so this is not a claim about an afternoon's hand-testing.
//!
//! The two tests that read the ledger have no in-suite negative control — one asserts a file on
//! disk and the other a history — so each was broken by hand and watched go red:
//!
//! | mutation | caught by |
//! |---|---|
//! | a sixteenth tool added to `tools.json`, version left at 0.24.0 — #940's scenario exactly | `the_ledger_records_the_digest_of_the_contract_it_ships`, which printed the line to append |
//! | one hex digit changed in the `0.17.0` record, twenty-four commits back | `the_ledger_is_the_history_the_repository_records` |
//!
//! The second matters more than it looks: 25 of the 26 records were measured rather than
//! written, and without a check that recomputes them from the history they would be 25
//! unfalsifiable claims sitting in a file that says it is append-only.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::{json, Value};
use yidam::git::Git;

/// `tools.json`, relative to the repository root — the spelling git wants for `show`.
const CONTRACT: &str = "yidam/prelude/sdks/parity/mcp/tools.json";

/// The ledger, beside it.
const LEDGER: &str = "yidam/prelude/sdks/parity/mcp/CONTRACT_SHA";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above the crate")
}

fn contract() -> Value {
    let path = repo_root().join(CONTRACT);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// The version `tools.json` declares.
fn declared_version(doc: &Value) -> String {
    doc["contract"]
        .as_str()
        .expect("tools.json declares a contract version")
        .to_string()
}

// ── the projection ────────────────────────────────────────────────────────────

/// One JSON value, written the same way whatever the file looked like.
///
/// Object keys sorted by code point, arrays left in document order, no insignificant
/// whitespace. Reindenting `tools.json` or moving a key is therefore free; reordering the
/// `tools` array is not, and that is deliberate — the order a client reads tools in is part of
/// what the file publishes, and a reorder is one line in the ledger.
fn canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{}:", Value::String((*k).to_string()));
                canonical(&map[*k], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        scalar => {
            let _ = write!(out, "{scalar}");
        }
    }
}

/// The document the version names, canonically written, with the version removed.
///
/// Removed rather than included, because a digest over the version too could never fail: every
/// bump would move it by construction and the check would pass while saying nothing.
fn projection(doc: &Value) -> String {
    let mut doc = doc.clone();
    doc.as_object_mut()
        .expect("the contract is a JSON object")
        .remove("contract");
    let mut out = String::new();
    canonical(&doc, &mut out);
    out
}

fn digest(doc: &Value) -> String {
    yidam::deps::sha256_hex(projection(doc).as_bytes())
}

// ── the ledger ────────────────────────────────────────────────────────────────

/// One recorded version and the digest of the document it named.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recorded {
    version: String,
    digest: String,
}

impl std::fmt::Display for Recorded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<8} sha256:{}", self.version, self.digest)
    }
}

/// `(major, minor, patch)`, for comparing two recorded versions.
fn semver(v: &str) -> (u64, u64, u64) {
    let mut parts = v.split('.').map(|p| {
        p.parse::<u64>()
            .unwrap_or_else(|e| panic!("`{v}` is not a version this ledger can order: {e}"))
    });
    let triple = (
        parts.next().expect("a version has a major"),
        parts.next().expect("a version has a minor"),
        parts.next().expect("a version has a patch"),
    );
    assert!(
        parts.next().is_none(),
        "`{v}` has a fourth component; the contract is versioned major.minor.patch"
    );
    triple
}

/// Every line of the ledger that carries a record, in file order.
///
/// Each line is checked as it is read rather than the count being checked at the end: a
/// scanner that skipped every malformed line would report a shorter ledger and nothing else,
/// and the shape of that failure is a hole that grows one line at a time.
fn ledger() -> Vec<Recorded> {
    let path = repo_root().join(LEDGER);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()));

    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let version = fields.next().expect("a non-empty line has a first field");
        let digest = fields
            .next()
            .unwrap_or_else(|| panic!("{LEDGER}:{} records `{version}` and no digest", n + 1));
        assert!(
            fields.next().is_none(),
            "{LEDGER}:{} has a third field; a record is `<version>  sha256:<hex>`",
            n + 1
        );
        let digest = digest.strip_prefix("sha256:").unwrap_or_else(|| {
            panic!(
                "{LEDGER}:{} carries `{digest}`, which does not name its algorithm. A record \
                 is `<version>  sha256:<hex>`.",
                n + 1
            )
        });
        assert!(
            digest.len() == 64
                && digest
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "{LEDGER}:{} carries `{digest}`, which is not 64 lower-case hex characters",
            n + 1
        );
        semver(version);
        out.push(Recorded {
            version: version.to_string(),
            digest: digest.to_string(),
        });
    }

    // The ledger is append-only and was seeded from the whole of the contract's history, so a
    // shorter one means a truncation rather than a beginning.
    assert!(
        out.len() >= 26,
        "{LEDGER} records {} version(s). It was seeded with 26 — every contract version from \
         0.1.0 — and is only ever appended to, so a shorter file has lost records rather than \
         not yet gained them.",
        out.len()
    );
    assert_eq!(
        out.first().map(|r| r.version.as_str()),
        Some("0.1.0"),
        "{LEDGER} no longer starts at the version the contract was frozen at"
    );
    out
}

/// What to do when the document has moved and the version has not.
fn how_to_bump(current: &str, digest: &str) -> String {
    format!(
        "`tools.json` is not the document contract {current} names, and {current} has already \
         shipped under that number. Bump it, in the four places that carry it and the ledger \
         that dates it:\n\
         \n\
         \x20 1. `{CONTRACT}` — the `contract` field\n\
         \x20 2. `yidam/prelude/sdks/parity/mcp/VERSION`\n\
         \x20 3. `yidam/prelude/sdks/parity/mcp/README.md` — the capability-block example\n\
         \x20 4. `docs/mcp-server.md` — the `initialize` handshake example\n\
         \x20 5. `{LEDGER}` — append `<the new version>  sha256:{digest}`\n\
         \n\
         If the edit was to the document as {current} stands — a typo, a reflow — it is still a \
         bump: a client that read {current} yesterday and reads it today must not be looking at \
         two different documents. A patch bump is what that costs."
    )
}

// ── the gate ──────────────────────────────────────────────────────────────────

/// The last line of the ledger is about the contract that is in the tree.
#[test]
fn the_ledger_ends_at_the_version_the_contract_declares() {
    let doc = contract();
    let declared = declared_version(&doc);
    let last = ledger().pop().expect("the ledger has records");
    assert_eq!(
        last.version, declared,
        "`tools.json` declares contract {declared} and the last record in {LEDGER} is \
         {}. The ledger is appended to in the same commit as the bump — a version that \
         reaches the file without reaching the ledger is a version with no digest, which is \
         the state #940 is about.",
        last.version
    );
}

/// And the document it is about hashes to what the ledger says.
///
/// This is the ratchet. Every other test here is about the ledger being well formed.
#[test]
fn the_ledger_records_the_digest_of_the_contract_it_ships() {
    let doc = contract();
    let declared = declared_version(&doc);
    let computed = digest(&doc);
    let last = ledger().pop().expect("the ledger has records");
    assert_eq!(
        last.digest,
        computed,
        "{LEDGER} records sha256:{} for contract {}, and the document in the tree hashes to \
         sha256:{computed}.\n\n{}",
        last.digest,
        last.version,
        how_to_bump(&declared, &computed)
    );
}

/// A number, once shipped, is that document's name forever.
///
/// The merge case is what this is for: two branches each adding a tool under one version each
/// write a line, and the file conflicts. If a merge resolves them both in — or a hand-edit
/// reuses a number — the duplicate is here rather than in a client's `match`.
#[test]
fn no_contract_version_is_recorded_twice() {
    let mut seen = BTreeSet::new();
    let mut repeats = Vec::new();
    for r in ledger() {
        if !seen.insert(r.version.clone()) {
            repeats.push(r.version);
        }
    }
    assert!(
        repeats.is_empty(),
        "{LEDGER} records {repeats:?} more than once. Either two changes claimed one contract \
         version — which is the collision #940 asks this file to make loud — or a merge kept \
         both sides of one line. The repair is a new version for the later document, not a \
         second digest for the same number."
    );
}

/// And the ledger reads forward, so a reused number cannot arrive as a rollback.
#[test]
fn the_ledger_only_ever_moves_forward() {
    let records = ledger();
    for pair in records.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        assert!(
            semver(&before.version) < semver(&after.version),
            "{LEDGER} records {} after {}. The file is the order the versions shipped in; a \
             record out of order means a number was reused or a line was inserted rather than \
             appended.",
            after.version,
            before.version
        );
    }
}

/// The converse, and it was the unasked direction: a bump means the document moved.
///
/// Two versions with one digest would be a number announcing a change that is not in the file
/// — the same defect as a change with no number, seen from the other side, and the one a
/// client cannot detect at all: it compares versions, finds them different, and re-reads a
/// document that is byte-for-byte what it already had. True across all 26 seeded versions.
#[test]
fn no_two_versions_record_one_document() {
    let mut seen: Vec<Recorded> = Vec::new();
    for r in ledger() {
        if let Some(earlier) = seen.iter().find(|e| e.digest == r.digest) {
            panic!(
                "{LEDGER} records one document under both {} and {}. A bump whose document is \
                 unchanged tells a client to re-read bytes it already has; if the version moved \
                 for a reason outside `tools.json`, that reason belongs in the file the version \
                 is about.",
                earlier.version, r.version
            );
        }
        seen.push(r);
    }
}

// ── the digest's own guards ───────────────────────────────────────────────────

/// The projection is the document, and it is not the version.
#[test]
fn the_projection_is_the_document_minus_its_version() {
    let doc = contract();
    let declared = declared_version(&doc);
    let projected = projection(&doc);

    // `contract` is the only key of that name in the file, so this is exact.
    assert!(
        !projected.contains("\"contract\":"),
        "the projection still carries the `contract` key, so every bump moves the digest by \
         itself and the ledger can never disagree with the tree"
    );
    // The version does appear in the prose, and it is content rather than a declaration: the
    // file dates a clause by writing `(contract 0.24.0)` beside it, twice at the version in the
    // tree. So the assertion is about the *form* — every occurrence is an arrival marker — and
    // what proves the declaration itself is out is
    // `the_digest_is_blind_to_formatting_and_to_the_version_itself`, which renumbers the
    // contract to 99.99.99 and gets the same digest.
    let marked = projected.match_indices(&declared).all(|(at, _)| {
        at >= "contract ".len() && &projected[at - "contract ".len()..at] == "contract "
    });
    assert!(
        marked,
        "the projection carries the version string {declared} somewhere that is not the \
         `(contract {declared})` marker the file dates its clauses with. Find it before \
         trusting this digest: a digest the version reaches is a digest every bump moves by \
         itself, and a ratchet that cannot fail is not one."
    );

    // And it is the document: every tool the file lists, by name, and the whole of the prose
    // the bumps above are mostly made of. A projection that had quietly become a summary would
    // satisfy the ledger tests forever.
    let names: Vec<&str> = doc["tools"]
        .as_array()
        .expect("the contract lists tools")
        .iter()
        .map(|t| t["name"].as_str().expect("a tool has a name"))
        .collect();
    assert!(
        names.len() >= 15,
        "{} tool(s) in the contract; the projection assertions below would be about almost \
         nothing",
        names.len()
    );
    for name in &names {
        assert!(
            projected.contains(&format!("\"{name}\"")),
            "the projection does not mention the tool `{name}`"
        );
    }
    assert!(
        projected.len() > 60_000,
        "the projection is {} bytes against a contract file of {} — it has stopped being the \
         document",
        projected.len(),
        std::fs::read_to_string(repo_root().join(CONTRACT))
            .expect("the contract is readable")
            .len()
    );

    // Sorted keys, checked on the keys the file happens to carry in another order.
    let (schema, tools) = (
        projected
            .find("\"$schema\"")
            .expect("the projection carries $schema"),
        projected
            .find("\"tools\"")
            .expect("the projection carries tools"),
    );
    assert!(
        schema < tools,
        "the projection's top-level keys are not in sorted order, so it is the document's \
         formatting and not its content"
    );
}

/// One named change to the contract document, applied in memory.
type Mutation = (&'static str, Box<dyn Fn(&mut Value)>);

/// Every part of the contract is part of the digest.
#[test]
fn the_digest_moves_when_any_part_of_the_contract_does() {
    let doc = contract();
    let before = digest(&doc);

    let mutations: Vec<Mutation> = vec![
        (
            "a sixteenth tool",
            Box::new(|d: &mut Value| {
                d["tools"].as_array_mut().expect("tools").push(json!({
                    "name": "semantic_search",
                    "tier": "core",
                    "description": "The second name for `retrieve` the contract forbids.",
                    "inputSchema": {"type": "object", "properties": {}},
                    "response": {"required": ["results"]}
                }));
            }),
        ),
        (
            "a tool renamed",
            Box::new(|d: &mut Value| d["tools"][0]["name"] = json!("search")),
        ),
        (
            "a tool moved to another tier",
            Box::new(|d: &mut Value| d["tools"][0]["tier"] = json!("graph")),
        ),
        (
            "a required response key dropped",
            Box::new(|d: &mut Value| {
                d["tools"][0]["response"]["required"]
                    .as_array_mut()
                    .expect("retrieve's response names required keys")
                    .pop();
            }),
        ),
        (
            "an input property added",
            Box::new(|d: &mut Value| {
                d["tools"][0]["inputSchema"]["properties"]["threshold"] = json!({"type": "number"});
            }),
        ),
        (
            "a capability dropped from the block",
            Box::new(|d: &mut Value| {
                d["capabilities"]
                    .as_object_mut()
                    .expect("the contract describes its capabilities")
                    .remove("graph");
            }),
        ),
        (
            "a frozen degraded reason struck out of the prose that freezes it",
            Box::new(|d: &mut Value| {
                let notes = d["tools"][0]["response"]["notes"]
                    .as_str()
                    .expect("retrieve documents its response");
                assert!(
                    notes.contains("no_vector_support"),
                    "this mutation edits a value the freeze no longer names; the vocabulary \
                     moved and the mutation has to move with it"
                );
                let struck = notes.replace("no_vector_support", "index_unreadable");
                d["tools"][0]["response"]["notes"] = json!(struck);
            }),
        ),
        (
            "one word of a specified behaviour",
            Box::new(|d: &mut Value| {
                let notes = d["tools"][0]["description"]
                    .as_str()
                    .expect("retrieve is described");
                d["tools"][0]["description"] =
                    json!(notes.replace("degrades to", "may degrade to"));
            }),
        ),
    ];

    for (what, mutate) in mutations {
        let mut mutated = doc.clone();
        mutate(&mut mutated);
        assert_ne!(
            mutated, doc,
            "the `{what}` mutation left the document unchanged, so what it proves about the \
             digest is nothing"
        );
        assert_ne!(
            digest(&mutated),
            before,
            "`{what}` does not move the contract digest. Whatever the ledger is recording, it \
             is not the document."
        );
    }
}

/// And two things do not move it, because a gate nobody can live with gets worked around.
#[test]
fn the_digest_is_blind_to_formatting_and_to_the_version_itself() {
    let doc = contract();
    let before = digest(&doc);

    let mut renumbered = doc.clone();
    renumbered["contract"] = json!("99.99.99");
    assert_eq!(
        digest(&renumbered),
        before,
        "the version is inside the digest, so a bump moves it on its own and the ratchet can \
         never fail"
    );

    // Whitespace: the pretty-printed document is a different file and the same contract.
    let reflowed: Value = serde_json::from_str(
        &serde_json::to_string_pretty(&doc).expect("the contract re-serialises"),
    )
    .expect("and re-parses");
    assert_eq!(
        digest(&reflowed),
        before,
        "reindenting `tools.json` moves the digest, which would make every reflow a contract \
         bump"
    );
}

// ── the seeded history ────────────────────────────────────────────────────────

/// The ledger's 26 records are the 26 documents this repository has actually shipped.
///
/// Seeded by measurement rather than by hand: every line was computed from the `tools.json`
/// each commit left behind, and this is the test that says so — 25 of those lines are about
/// documents no longer in the tree, and a claim nothing can check is not evidence of anything.
///
/// **Consecutive states under one version collapse to the last.** `948c316` reworded a `notes`
/// paragraph without moving 0.16.0, so the document stood two ways under that number; the
/// ledger records the one it ended as. That commit is the hole this file closes, and it is
/// left visible here rather than smoothed over.
///
/// Skipped without the history to read: a shallow clone has the file and not its past, and a
/// released crate has neither. `ci (cli)` runs on `actions/checkout` at depth 1, so this
/// announces a skip there and runs for whoever has the repository.
#[test]
fn the_ledger_is_the_history_the_repository_records() {
    let root = repo_root();

    if Git::new(&root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .try_run()
        .is_none()
    {
        ci_report::skipped(&format!(
            "run inside a checkout of this repository: {LEDGER} is verified against the \
             history of {CONTRACT}, and there is none to read here"
        ));
        return;
    }
    if Git::new(&root)
        .args(["rev-parse", "--is-shallow-repository"])
        .try_run()
        .as_deref()
        == Some("true")
    {
        ci_report::skipped(&format!(
            "run `--unshallow` first: {LEDGER} is verified against the whole history of \
             {CONTRACT}, and a shallow clone has only its tip"
        ));
        return;
    }

    let commits = Git::new(&root)
        .args(["log", "--reverse", "--format=%H"])
        .paths([CONTRACT])
        .lines()
        .expect("the log of a tracked file is readable");
    assert!(
        commits.len() >= 27,
        "{} commit(s) touch {CONTRACT}; there were 27 when this was written and the file is \
         only ever added to. A history this short is a clone with a truncated graft, not a \
         contract with a shorter past.",
        commits.len()
    );

    let mut derived: Vec<Recorded> = Vec::new();
    for commit in &commits {
        let text = Git::new(&root)
            .arg("show")
            .rev(format!("{commit}:{CONTRACT}"))
            .run()
            .unwrap_or_else(|e| panic!("reading {CONTRACT} at {commit}: {e}"));
        let doc: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{CONTRACT} at {commit} is not JSON: {e}"));
        let record = Recorded {
            version: declared_version(&doc),
            digest: digest(&doc),
        };
        match derived.last_mut() {
            Some(last) if last.version == record.version => *last = record,
            _ => derived.push(record),
        }
    }

    if derived != ledger() {
        let shown: Vec<String> = derived.iter().map(ToString::to_string).collect();
        panic!(
            "{LEDGER} is not the history of {CONTRACT}. What the history says, in full:\n\n{}\n\n\
             A mismatch is one of three things: a record hand-written rather than measured, a \
             line edited after it shipped — the file is append-only, and an earlier record is \
             about a document a client has already read — or the projection in this file \
             changed, in which case every record moves at once and the ledger is reseeded from \
             this output in the same commit.",
            shown.join("\n")
        );
    }
}
