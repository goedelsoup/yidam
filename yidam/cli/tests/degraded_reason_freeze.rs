//! The reasons this server produces are the reasons the contract freezes.
//!
//! `retrieval/mod.rs` has always pinned the spellings — against constants in the same file.
//! That holds a rename to a deliberate act and proves nothing about the document a client
//! actually reads: `prelude/sdks/parity/mcp/tools.json` is the freeze, and it says so itself —
//! *"a value outside this set is a divergence; a server needing one should add it here first."*
//!
//! Adding `remote_unavailable` is the first time that instruction has been followed, and the
//! only thing that made it visible was reading the freeze by hand. So the freeze is read here
//! instead.
//!
//! **Both directions.** A value produced and not frozen is a divergence; a value frozen and
//! produced by nothing is a `stale_contract` — which sat in the contract unimplemented from
//! the start until #536, described a state nobody could reach, and would have been reinvented
//! under a second name by anyone who needed it. Asking only the first question is how that
//! lasted.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// The values the CLI can put in a `degraded_reason`.
///
/// Written out rather than imported: `retrieval`'s constants are `pub(crate)`, and a test that
/// imported them would be asserting that two spellings of the same constant agree. What this
/// file is for is holding the *implementation* to the *contract*, so one side of the comparison
/// has to be independent of the other. `retrieval/mod.rs`'s own unit test pins these against
/// the constants; the pair is what closes the loop.
const PRODUCED: &[&str] = &[
    "no_index",
    "no_vector_support",
    "stale_contract",
    "remote_unavailable",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above the crate")
}

fn tools_json() -> String {
    let p = repo_root().join("yidam/prelude/sdks/parity/mcp/tools.json");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The passage listing the frozen values, and the values in it.
///
/// Parsed from the prose block rather than from a JSON array, because that is the shape the
/// contract uses — the values are listed in a `notes` string with their explanations beside
/// them, and an explanation is most of what makes the vocabulary usable. A parser looking for
/// a key that does not exist would pass vacuously, so the population is asserted below.
fn frozen() -> BTreeSet<String> {
    let text = tools_json();
    let start = text
        .find("The frozen values:")
        .expect("tools.json no longer contains the frozen-value block — has the freeze moved?");
    // The block ends where the precedence discussion begins.
    let end = text[start..]
        .find("Precedence is by what must be fixed FIRST")
        .expect("the frozen-value block no longer ends where this parser expects");
    let block = &text[start..start + end];

    let mut out = BTreeSet::new();
    for line in block.split("\\n") {
        // `  <name>  — <explanation>`; the em dash is what marks a definition rather than
        // prose that happens to mention a value.
        let Some((name, rest)) = line.trim().split_once('—') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(' ') || rest.trim().is_empty() {
            continue;
        }
        out.insert(name.to_string());
    }
    assert!(
        out.len() >= 3,
        "only {} frozen value(s) parsed ({out:?}); if the block's shape changed, every \
         assertion built on this is vacuous",
        out.len()
    );
    out
}

#[test]
fn every_reason_this_server_produces_is_in_the_freeze() {
    let frozen = frozen();
    let missing: Vec<&str> = PRODUCED
        .iter()
        .copied()
        .filter(|r| !frozen.contains(*r))
        .collect();
    assert!(
        missing.is_empty(),
        "these degraded reasons are produced and not frozen: {missing:?}.\n\
         The contract says a value outside its set is a divergence and that a server needing \
         one should add it to `prelude/sdks/parity/mcp/tools.json` FIRST. Add it there, and \
         bump the contract version in all three places that carry it."
    );
}

#[test]
fn every_frozen_reason_is_produced_by_something() {
    let produced: BTreeSet<&str> = PRODUCED.iter().copied().collect();
    let orphans: Vec<String> = frozen()
        .into_iter()
        .filter(|r| !produced.contains(r.as_str()))
        .collect();
    assert!(
        orphans.is_empty(),
        "these values are frozen and nothing produces them: {orphans:?}.\n\
         `stale_contract` was in exactly this position from the contract's first version \
         until #536 — a named state no code could reach, which the next person to need it \
         would have reinvented under a second name."
    );
}

/// The three copies of the contract version agree — which is all this establishes.
///
/// `tools.json` carries it, `VERSION` carries it, and the README example prints it. A client
/// reading the handshake and a client reading the file must not be told different numbers.
///
/// It does **not** establish that the version moved when the surface did, and it used to claim
/// it did (#940). Copies held to copies are green on a branch that adds a tool and reuses the
/// number, and on a second branch doing the same, and on the merge of both. What the version
/// names is settled one file over, in `mcp/CONTRACT_SHA` and `tests/mcp_contract_digest.rs`: a
/// digest of the document is recorded beside each version, so the number cannot be reused and
/// two branches claiming one collide in that ledger.
///
/// A fourth copy exists — `docs/mcp-server.md`'s handshake example, which went stale across
/// eleven bumps — and is held by `versioning_layers.rs::the_mcp_contract_states_one_version`
/// along with these three. This test is the narrower duplicate, and it stays here because the
/// reasons frozen above are part of what that version names: changing one of them is a bump,
/// and this file is where it gets changed.
#[test]
fn the_three_copies_of_the_contract_version_agree() {
    let root = repo_root().join("yidam/prelude/sdks/parity/mcp");
    let declared: serde_json::Value =
        serde_json::from_str(&tools_json()).expect("tools.json is JSON");
    let in_json = declared["contract"]
        .as_str()
        .expect("tools.json carries a contract version")
        .to_string();

    let in_version = std::fs::read_to_string(root.join("VERSION"))
        .expect("VERSION is readable")
        .trim()
        .to_string();
    assert_eq!(
        in_json, in_version,
        "tools.json and VERSION disagree. A bump touches five things: those two, the README \
         capability block, the handshake example in `docs/mcp-server.md`, and a new record in \
         `mcp/CONTRACT_SHA` — `mcp_contract_digest.rs` prints the line to append."
    );

    let readme = std::fs::read_to_string(root.join("README.md")).expect("README is readable");
    assert!(
        readme.contains(&format!("\"contract\": \"{in_json}\"")),
        "the README example prints a contract version other than {in_json} — an implementer \
         copying that block conforms to a version the file no longer declares"
    );
}
