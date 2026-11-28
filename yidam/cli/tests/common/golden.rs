//! One golden comparison for every test that holds a committed file to what code produces
//! (#1634).
//!
//! Before this there were three variables (`UPDATE_GOLDENS`, `UPDATE_TRANSCRIPTS`,
//! `YIDAM_BLESS`), a hand-written write-or-compare branch at every site, and as many failure
//! messages, some of which printed both texts whole and none of which printed a diff.
//! Now there is one variable, read in one place; one diff format; and one refusal rule:
//!
//! - **`UPDATE_GOLDENS=1` is the only switch.** [`updating`] is the one reading of it, so a
//!   test that rewrites something other than a whole file (a fix mode, a baseline compared
//!   within a tolerance) still asks here rather than reading the environment itself.
//! - **It is refused where `CI` is set.** A gate that may rewrite what it compares against
//!   passes by construction, and a runner is the one place nobody reviews the rewrite.
//! - **The retired names are refused, not ignored.** `UPDATE_TRANSCRIPTS=1` or
//!   `YIDAM_BLESS=1` set by habit would otherwise do nothing and look like a passing run.
//!
//! The file is one source compiled into three targets: the `gates` and `suite` test binaries
//! reach it through `common` (which both include by `#[path]`), and the library's unit tests
//! through a `#[cfg(test)]` `#[path]` module in `src/lib.rs`. A shared crate would have been a
//! new dependency for two hundred lines. A `#[cfg(test)]` module in the library alone is
//! invisible to the test binaries, which link the library built without `cfg(test)`, and a
//! `#[doc(hidden)]` public one would ship a test helper in every build of the crate.

#![allow(dead_code)] // each target that includes this uses a different part of it

use std::ffi::OsString;
use std::path::Path;

/// The one variable. Set to anything, it rewrites goldens instead of comparing against them.
pub const VARIABLE: &str = "UPDATE_GOLDENS";

/// Names that once meant what [`VARIABLE`] means, refused so that setting one by habit fails
/// loudly instead of running a comparison the person believed was a rewrite.
pub const RETIRED: &[&str] = &["UPDATE_TRANSCRIPTS", "YIDAM_BLESS"];

/// Diff lines printed before the rest are counted rather than shown.
const SHOWN: usize = 120;

/// Unchanged lines kept around each change, so a hunk can be found in the file.
const CONTEXT: usize = 2;

/// Whether this run rewrites goldens. Panics, naming the rule, when the environment asks for
/// something the rule refuses.
pub fn updating() -> bool {
    decide(|name| std::env::var_os(name)).unwrap_or_else(|refusal| panic!("{refusal}"))
}

/// The rule, over an environment passed in, so it can be tested without mutating the
/// process's own.
pub fn decide(env: impl Fn(&str) -> Option<OsString>) -> Result<bool, String> {
    if let Some(old) = RETIRED.iter().find(|name| env(name).is_some()) {
        return Err(format!(
            "{old} is no longer read (#1634): every golden is rewritten by {VARIABLE}=1. \
             Unset {old} and re-run with {VARIABLE}=1."
        ));
    }
    let update = env(VARIABLE).is_some();
    if update && env("CI").is_some() {
        return Err(format!(
            "{VARIABLE} is set and so is CI. A run that may rewrite what it compares against \
             passes by construction, so goldens are only rewritten off CI, where the diff is \
             reviewed before it is committed."
        ));
    }
    Ok(update)
}

/// `actual` is what `path` holds, or, under [`VARIABLE`], becomes it.
pub fn assert(path: &Path, actual: &str) {
    if let Err(failure) = check(path, actual) {
        panic!("\n{failure}");
    }
}

/// [`assert`] for a test that gathers every drifted golden before it fails: the failure is
/// returned rather than raised.
pub fn check(path: &Path, actual: &str) -> Result<(), String> {
    compare(path, actual, updating())
}

/// The comparison itself, with the decision passed in.
pub fn compare(path: &Path, actual: &str, update: bool) -> Result<(), String> {
    let committed = std::fs::read_to_string(path);
    if update {
        if committed.as_deref().ok() != Some(actual) {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .unwrap_or_else(|e| panic!("creating {}: {e}", dir.display()));
            }
            std::fs::write(path, actual)
                .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        }
        return Ok(());
    }
    let committed = committed.map_err(|e| {
        format!(
            "{} is missing or unreadable ({e}). Run with {VARIABLE}=1 to write it.",
            path.display()
        )
    })?;
    if committed == actual {
        return Ok(());
    }
    Err(format!(
        "{} drifted from what the test produces. If the change is intended, re-run with \
         {VARIABLE}=1 and review the diff.\n--- {} (committed)\n+++ produced\n{}",
        path.display(),
        path.display(),
        diff(&committed, actual)
    ))
}

/// A line diff of `old` against `new`: `-` lines only in `old`, `+` lines only in `new`, and
/// a few unchanged lines around each change, under an `@@` header naming both line numbers.
pub fn diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let ops = edit_script(&a, &b);

    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, Op::Same(..)))
        .map(|(i, _)| i)
        .collect();
    let keep = |i: usize| {
        changed
            .iter()
            .any(|&c| i + CONTEXT >= c && i <= c + CONTEXT)
    };

    let mut lines = Vec::new();
    let mut previous: Option<usize> = None;
    for (i, op) in ops.iter().enumerate() {
        if !keep(i) {
            continue;
        }
        if previous.is_none_or(|p| p + 1 != i) {
            let (x, y) = op.position();
            lines.push(format!(
                "@@ committed line {}, produced line {} @@",
                x + 1,
                y + 1
            ));
        }
        previous = Some(i);
        let (mark, text) = match op {
            Op::Same(_, _, t) => (' ', *t),
            Op::Gone(_, _, t) => ('-', *t),
            Op::Added(_, _, t) => ('+', *t),
        };
        match text.strip_suffix('\n') {
            Some(body) => lines.push(format!("{mark} {body}")),
            None => lines.push(format!("{mark} {text}\n  (no newline at end of file)")),
        }
    }
    if lines.is_empty() {
        return "  (the texts differ in no line)\n".to_string();
    }
    let hidden = lines.len().saturating_sub(SHOWN);
    lines.truncate(SHOWN);
    if hidden > 0 {
        lines.push(format!("… and {hidden} more diff lines"));
    }
    lines.join("\n") + "\n"
}

/// One step of the edit script, with its position in `old` and in `new`.
enum Op<'a> {
    Same(usize, usize, &'a str),
    Gone(usize, usize, &'a str),
    Added(usize, usize, &'a str),
}

impl Op<'_> {
    fn position(&self) -> (usize, usize) {
        match self {
            Op::Same(x, y, _) | Op::Gone(x, y, _) | Op::Added(x, y, _) => (*x, *y),
        }
    }
}

/// The longest common subsequence of the lines between the common prefix and suffix. Past
/// four million cells the middle is shown as removed and re-added whole, which is still a
/// correct diff, only a longer one.
fn edit_script<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Op<'a>> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);

    let mut ops: Vec<Op<'a>> = (0..prefix).map(|i| Op::Same(i, i, a[i])).collect();
    let (n, m) = (am.len(), bm.len());
    if n.saturating_mul(m) > 4_000_000 {
        ops.extend(
            am.iter()
                .enumerate()
                .map(|(i, t)| Op::Gone(prefix + i, prefix, t)),
        );
        ops.extend(
            bm.iter()
                .enumerate()
                .map(|(j, t)| Op::Added(prefix + n, prefix + j, t)),
        );
    } else {
        // lcs[i][j]: the longest common subsequence of am[i..] and bm[j..].
        let mut lcs = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i][j] = if am[i] == bm[j] {
                    lcs[i + 1][j + 1] + 1
                } else {
                    lcs[i + 1][j].max(lcs[i][j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            let (x, y) = (prefix + i, prefix + j);
            if i < n && j < m && am[i] == bm[j] {
                ops.push(Op::Same(x, y, am[i]));
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
                // Removals before additions, so a changed line reads `-` then `+`.
                ops.push(Op::Gone(x, y, am[i]));
                i += 1;
            } else {
                ops.push(Op::Added(x, y, bm[j]));
                j += 1;
            }
        }
    }
    let (ta, tb) = (a.len() - suffix, b.len() - suffix);
    ops.extend((0..suffix).map(|k| Op::Same(ta + k, tb + k, a[ta + k])));
    ops
}
