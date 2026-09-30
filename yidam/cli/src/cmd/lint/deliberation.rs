//! Which resolution records were written after this repository's PROTOCOL.md asked for
//! `rounds:` and `positions:` (#592).
//!
//! `resolution-deliberation-unrecorded` warns on a record missing either field, and gates on
//! one that could have carried them. This module decides *could have*, and only this module
//! touches git for it; the check stays a pure function of the records and this set.
//!
//! # Decided by ancestry, not by date or by the record's other fields
//!
//! A derived repository vendors PROTOCOL.md at a vintage, and the one that has run this
//! protocol still vendors a copy without the two fields. So the template's own date for them
//! (2026-08-20) says nothing about any given repository. What does is this repository's
//! history: the commit that first put a `rounds:` line into its own PROTOCOL.md. A record
//! added in that commit or in one descending from it was written from a format that asked for
//! the fields.
//!
//! **Ancestry rather than dates.** A `rigpa/*` branch cut before the protocol was upgraded and
//! merged after it was written from the old format, and its commit date says otherwise. The
//! record's own `date:` is prose the author typed. The record's other fields are no better:
//! `synthesized-by:` and `independence:` both entered the format later, but both can be added
//! to an old record, and `resolution-independence-mismatch` asks for exactly that.
//!
//! **A shallow clone decides nothing.** Its boundary commit appears to add the whole
//! PROTOCOL.md, and every record with it, so every record would read as written after the
//! ask. The set is empty there instead, and every finding warns. That is the lenient
//! direction: a thin CI checkout under-reports and never gates on a debt that cannot be paid.

use std::collections::BTreeSet;
use std::path::Path;

use crate::cmd::sangha::Resolution;
use crate::git::Git;

/// The records, by repository-relative path, that were added at or after the commit that put
/// `rounds:` into this repository's PROTOCOL.md record format.
///
/// Empty when there are no records (nothing is spawned), when the protocol has never asked,
/// and in a shallow clone.
pub(crate) fn asked(root: &Path, records: &[Resolution]) -> BTreeSet<String> {
    if records.is_empty() || crate::git::is_shallow(root) {
        return BTreeSet::new();
    }
    let sangha = crate::paths::yidam_sangha_dir(root);
    let rel = |p: &Path| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let protocol = rel(&sangha.join("PROTOCOL.md"));
    let resolutions = rel(&sangha.join("resolutions"));

    // Oldest first, so the first line is the commit that introduced the field.
    let Some(ask) = Git::new(root)
        .args(["log", "--reverse", "--format=%H", "-G^rounds:"])
        .rev("HEAD")
        .paths([&protocol])
        .lines()
        .and_then(|l| l.into_iter().find(|h| !h.is_empty()))
    else {
        return BTreeSet::new();
    };

    let added = |g: Git| -> Vec<String> {
        g.args(["--format=", "--name-only", "--diff-filter=A", "--relative"])
            .paths([&resolutions])
            .lines()
            .unwrap_or_default()
    };
    // The commit that asked, then everything descending from it.
    let at = added(Git::new(root).arg("show").rev(&ask));
    let after = added(
        Git::new(root)
            .args(["log", "--ancestry-path"])
            .rev(format!("{ask}..HEAD")),
    );
    at.into_iter()
        .chain(after)
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git, init, write};

    const OLD: &str = "# Protocol\n\n```\nevolution: <name>\ntips:\n  - ma/<elector>@<hash>\n```\n";
    const NEW: &str = "# Protocol\n\n```\nevolution: <name>\nrounds: <n>\ntips:\n  - ma/<elector>@<hash>\npositions:\n  - positions/<p>.md\n```\n";
    const PROTOCOL: &str = ".yidam/sangha/PROTOCOL.md";

    fn rec(name: &str) -> String {
        format!(".yidam/sangha/resolutions/{name}.md")
    }

    fn records(names: &[&str]) -> Vec<Resolution> {
        names
            .iter()
            .map(|n| Resolution {
                file: rec(n),
                evolution: n.to_string(),
                date: String::new(),
                tips: vec![],
                synthesized_by: vec![],
                independence: String::new(),
                rounds: String::new(),
                positions: vec![],
                branch_present: false,
            })
            .collect()
    }

    fn record(dir: &Path, name: &str) {
        write(dir, &rec(name), "---\nevolution: e\n---\n");
    }

    /// Before the ask, the ask itself, after it, and a branch cut before it and merged after.
    /// Only the second and third were written from a format that asked.
    #[test]
    fn records_descending_from_the_ask_are_asked_and_no_others() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);
        write(root, PROTOCOL, OLD);
        record(root, "before");
        commit(root, "genesis");

        git(root, &["switch", "-q", "-c", "rigpa/stale"]);
        record(root, "stale-branch");
        commit(root, "resolve: cut before the ask");
        git(root, &["switch", "-q", "main"]);

        write(root, PROTOCOL, NEW);
        record(root, "same-commit");
        commit(root, "revise: the format asks for rounds and positions");
        record(root, "after");
        commit(root, "resolve: after the ask");
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "--no-gpg-sign",
                "rigpa/stale",
            ],
        );

        let all = records(&["before", "stale-branch", "same-commit", "after"]);
        let got = asked(root, &all);
        let want: BTreeSet<String> = [rec("same-commit"), rec("after")].into();
        assert_eq!(got, want);
    }

    /// The measured case: a repository whose vendored protocol never asked. Every record
    /// warns, however recent.
    #[test]
    fn a_protocol_that_never_asked_asks_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);
        write(root, PROTOCOL, OLD);
        record(root, "a");
        commit(root, "genesis");
        record(root, "b");
        commit(root, "resolve: b");

        assert!(asked(root, &records(&["a", "b"])).is_empty());
    }

    /// A shallow clone's boundary commit appears to add PROTOCOL.md whole, and every record
    /// with it. Reading that as the ask would gate on every record in the clone.
    #[test]
    fn a_shallow_clone_decides_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let origin = tmp.path().join("origin");
        std::fs::create_dir(&origin).unwrap();
        init(&origin);
        write(&origin, PROTOCOL, OLD);
        record(&origin, "old");
        commit(&origin, "genesis");
        write(&origin, PROTOCOL, NEW);
        commit(&origin, "revise: ask");
        record(&origin, "new");
        commit(&origin, "resolve: new");

        // The full history decides: `new` was asked, `old` was not.
        let all = records(&["old", "new"]);
        assert_eq!(asked(&origin, &all), BTreeSet::from([rec("new")]));

        let url = format!("file://{}", origin.display());
        git(
            tmp.path(),
            &["clone", "-q", "--depth", "1", &url, "shallow"],
        );
        let shallow = tmp.path().join("shallow");
        assert!(crate::git::is_shallow(&shallow));
        assert!(asked(&shallow, &all).is_empty());
    }
}
