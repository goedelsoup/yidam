//! Loading the shared parity fixtures, once.
//!
//! Every domain's `rust/tests/parity.rs` opened with the same `fixture_dir` + `load_fixtures`
//! pair — one hash across all fourteen crates with comments stripped, ~30 lines apiece. The
//! copies agreed, which is not why this exists. This is why:
//!
//! ```ignore
//! let fixtures = load_fixtures("similarity.cosine");
//! assert!(!fixtures.is_empty(), "no similarity.cosine fixtures found");
//! ```
//!
//! That `assert!` is the whole distance between this suite and fourteen tests that iterate an
//! empty vector and pass. It lived at the call site, so a fifteenth domain written by copying
//! a neighbour could lose it — and **a parity test that loads no fixtures reports success
//! identically to one that loads twelve**. Nothing downstream can tell those apart: the run is
//! green, the count is not printed, and the function under test was never called.
//!
//! `domain-parity-check` covers the neighbouring case from the other side — it fails on a
//! fixture *directory* that is empty — and cannot see a test that never asks for a directory
//! at all. The two are complements, not a duplicate pair.
//!
//! So the assertion moved inside [`load_fixtures`], where it is not a line anyone has to
//! remember to write. Sharing the code is the smaller half of that; making the guard
//! unskippable is the point, and `yidam/cli/tests/domain_parity_testkit.rs` is the other half
//! — it walks every `domains/*/rust/` and fails one that loads fixtures by hand instead.
//!
//! The fixture root is resolved from **this** crate's `CARGO_MANIFEST_DIR`, not the caller's,
//! which is why the `../../parity/fixtures` hop each domain used to spell out is gone. A
//! domain crate no longer knows where the fixtures live, so it cannot be moved to a depth that
//! makes its copy of that path wrong.

use std::path::{Path, PathBuf};

/// The `toml` a fixture was parsed with.
///
/// Re-exported so a domain crate's only dev-dependency is this one. A suite that named `toml`
/// itself could resolve a different 0.8.x than the testkit did, and then `toml::Value` in a
/// helper signature would be a *different type* from the `toml::Value` it is handed — a type
/// error whose message names one crate twice.
pub use toml;

/// Where `<function>`'s fixtures live: `prelude/domains/parity/fixtures/<function>`.
///
/// Exposed because a test that wants to name the directory in its own failure message should
/// ask rather than reconstruct it.
pub fn fixture_dir(function: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/parity/testkit/
    // ../fixtures/<function>  →  prelude/domains/parity/fixtures/<function>
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(function)
}

/// Every fixture for `function`, in filename order.
///
/// # Panics
///
/// If the directory is missing, unreadable, or holds no `.toml` fixture. That is the guard
/// this crate exists for and it is deliberately not a `Result`: a caller handed an `Err` can
/// decide to carry on, and "carry on with nothing to check" is the failure being prevented.
///
/// Filename order, not directory order, because `read_dir` yields whatever the filesystem
/// hands back. A fixture set that runs in one order on a laptop and another on a runner makes
/// the first failure's identity depend on the machine.
pub fn load_fixtures(function: &str) -> Vec<toml::Value> {
    let dir = fixture_dir(function);
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            panic!(
                "no fixture directory for {function} at {}: {e}",
                dir.display()
            )
        })
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort();

    let out: Vec<toml::Value> = entries
        .iter()
        .map(|path| {
            let raw = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("unreadable fixture {}: {e}", path.display()));
            toml::from_str::<toml::Value>(&raw)
                .unwrap_or_else(|e| panic!("malformed fixture {}: {e}", path.display()))
        })
        .collect();

    assert!(
        !out.is_empty(),
        "no {function} fixtures found in {} — a parity test with nothing to iterate passes, \
         which is what this assertion exists to prevent",
        dir.display()
    );
    out
}
