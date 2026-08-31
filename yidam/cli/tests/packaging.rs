//! What `cargo package` copies, and what it silently leaves behind.
//!
//! `cargo package` copies only what lives under the crate root. An `include_str!` whose path
//! escapes it — `../../../prelude/…` from `yidam/cli/src/` — resolves in the working tree and
//! in every CI job, because the tree is right there, and is simply **absent from the
//! tarball**. Nothing builds that tarball until `cargo publish` does, which is after the tag
//! is pushed and the binaries are built.
//!
//! This has now happened twice. `cli/v0.3.0` was one command from shipping five platform
//! binaries, a GitHub release and a Homebrew formula all naming a version `cargo install`
//! could not install; `cli/v0.7.0` would have done the same over seven `.rego` files. Both
//! were caught by `release.sh`'s `cargo publish --dry-run`, at the last moment at which
//! catching it is still free — and a check that only ever runs during a release is a check
//! whose failures all arrive at the worst time.
//!
//! So the rule moves into the test suite, where the diff that breaks it is the diff that
//! goes red. The fix in both cases was the same and is the one this repository has settled
//! on: a **symlink at the crate root** (git mode `120000`), which cargo dereferences when
//! packaging, so the crate carries the bytes and the repository keeps one copy.

use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

/// `yidam/cli/`, the crate root — the boundary `cargo package` will not reach past.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Resolve a path lexically, without touching the filesystem.
///
/// `Path::canonicalize` is the wrong tool here twice over: it resolves symlinks, which would
/// turn `policy/disclose/lib.rego` back into `yidam/prelude/…` and report the very escape
/// this test exists to permit; and it fails outright on a path that does not exist, which is
/// the *other* thing worth reporting. Lexical resolution answers the question actually being
/// asked — where does this path point, as written.
fn resolve_lexically(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in Path::new(rel).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Every `include_str!` / `include_bytes!` argument in the CLI's sources, with the file it
/// appears in.
///
/// Comment lines are stripped first. Two doc comments in this crate discuss `include_str!`
/// by name, and a guard that reads prose is a guard prose can satisfy — the same shape as a
/// job name answering for a job. Only real invocations count.
fn compiled_in_paths() -> Vec<(PathBuf, String)> {
    let src = crate_root().join("src");
    let mut found = Vec::new();

    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).expect("a source file is readable");

        for line in text.lines() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for macro_name in ["include_str!(", "include_bytes!("] {
                let mut rest = code;
                while let Some(at) = rest.find(macro_name) {
                    rest = &rest[at + macro_name.len()..];
                    let Some(open) = rest.find('"') else { break };
                    let Some(close) = rest[open + 1..].find('"') else {
                        break;
                    };
                    found.push((
                        entry.path().to_path_buf(),
                        rest[open + 1..open + 1 + close].to_string(),
                    ));
                    rest = &rest[open + 1 + close..];
                }
            }
        }
    }

    assert!(
        !found.is_empty(),
        "no include_str!/include_bytes! was found anywhere under {}; this test is scanning \
         the wrong tree and its assertion is vacuous",
        src.display()
    );
    found
}

/// Nothing compiled into the binary may live outside the crate root.
///
/// The failure this prevents is not a build error. It is four green jobs and one red one,
/// after the tag, naming a version nobody can install.
#[test]
fn every_compiled_in_file_is_inside_the_crate_root() {
    let root = crate_root();

    for (file, rel) in compiled_in_paths() {
        let dir = file.parent().expect("a source file has a directory");
        let target = resolve_lexically(dir, &rel);

        assert!(
            target.starts_with(&root),
            "{} compiles in `{rel}`, which resolves to {} — outside the crate root {}.\n\n\
             `cargo package` copies only what lives under the crate root, so this file would \
             be ABSENT from the published tarball. It builds here and in every CI job because \
             the working tree has it; it fails only in the packaged build, which nothing runs \
             until `cargo publish` does.\n\n\
             The fix this repository uses: a symlink at the crate root pointing at the real \
             file or directory, committed with git mode 120000 — see `yidam/cli/policy` and \
             `yidam/cli/mcp-contract.json` — and an include path that goes through it.",
            file.strip_prefix(&root).unwrap_or(&file).display(),
            target.display(),
            root.display(),
        );

        assert!(
            target.exists(),
            "{} compiles in `{rel}`, which resolves to {} — and nothing is there",
            file.strip_prefix(&root).unwrap_or(&file).display(),
            target.display(),
        );
    }
}

/// The crate-root symlinks are committed AS symlinks, not as copies of their targets.
///
/// A symlink replaced by a real file is invisible in every build and in this test's other
/// assertion — the bytes are right there, the include resolves, packaging works. What is
/// lost is the single copy: the file at the crate root and the one under `yidam/prelude/`
/// become two files free to drift, and the drift shows up as a binary enforcing a rule that
/// no longer matches the one a reader inspects after bootstrap.
#[test]
fn the_crate_root_indirections_are_symlinks() {
    let root = crate_root();
    let mut checked = 0;

    for name in ["policy", "mcp-contract.json"] {
        let path = root.join(name);
        let meta = std::fs::symlink_metadata(&path)
            .unwrap_or_else(|e| panic!("yidam/cli/{name} is not present ({e})"));
        assert!(
            meta.file_type().is_symlink(),
            "yidam/cli/{name} is a real file, not a symlink. It exists so the crate can \
             package bytes that live under yidam/prelude/ while the repository keeps one \
             copy of them; a copy here is a second copy, and the two are free to drift."
        );
        checked += 1;
    }

    assert_eq!(checked, 2, "the symlink roster was not walked");
}
