use super::*;

/// Are the corpora this repository depends on actually unpacked, and are they the ones
/// `tonpa.lock` pins?
///
/// Nothing asked this before. It did not matter much while `yidam tonpa install` was a step
/// somebody ran deliberately — they saw it succeed or fail. #397 made it a `postinstall`
/// hook, and mise treats a failing postinstall as a *warning*:
///
/// ```text
/// [tonpa-install] ERROR task failed
/// mise WARN  Postinstall hook in <dir> failed: … exited with code 1
/// $ echo $?
/// 0
/// ```
///
/// So `mise install` goes green, the toolchains are fine, the binary is fine, and every
/// command that reads a dependency's corpus reads nothing. The one line that said otherwise
/// scrolled past. This is the check that can still say so afterwards.
///
/// **No network, and none is possible from here.** It compares what is on disk against a
/// lock file, which is the whole correctness story for a fetched corpus. `doctor` is exactly
/// the command someone runs when they suspect the network, so it must not need it.
///
/// **Read-only.** `cmd_install` writes `tonpa.lock` when anything changed; this shares the
/// *verification* and not the command, the way [`check_regen`] borrows `stale_blocks` rather
/// than reimplementing the generator list.
pub(super) fn check_corpora(root: &Path) -> Answer {
    const REMEDY: &str = "mise run tonpa-install";
    // A shadow is not fixed by installing anything — the unpacked copy is a *different*
    // corpus from the checkout being read under its name, so the repair is to stop one of
    // them answering to that name.
    const SHADOW_REMEDY: &str = "rm -r .yidam/tonpa/<name>, or rename one of the two";

    // A file that does not parse declares nothing anyone can read, and every reader of the
    // dependencies degrades to none. Answering "none declared" over it was this check
    // agreeing with them (#1616).
    let config = match crate::deps::load_config(&crate::paths::tonpa_config_path(root)) {
        Ok(config) => config,
        Err(e) => return Answer::fail(format!("{e:#}"), None),
    };
    if config.dependencies.is_empty() {
        // Not a warning, and not "0 missing". A repository that depends on nothing is not
        // half-provisioned, and a line reporting a count here would read as a verdict on a
        // question nobody put — the same reason check_catalog stays quiet without a TTL.
        return Answer::ok("none declared");
    }

    let tonpa_dir = crate::paths::tonpa_dir(root);
    // Likewise the lock (#1657). Read as empty, it sent every fetched dependency to
    // "declared but never pinned", the line a repository with no lock gets, and the
    // `tonpa install` that line implies then fails on the parse error this check hid.
    let lock = match crate::deps::load_lock(&crate::paths::tonpa_lock_path(root)) {
        Ok(lock) => lock,
        Err(e) => return Answer::fail(format!("{e:#}"), None),
    };

    let (mut missing, mut corrupt, mut unlocked, mut ok, mut local) =
        (Vec::new(), Vec::new(), Vec::new(), 0usize, 0usize);

    // Two corpora claiming one name, which `deps::resolved` resolves silently in favour of
    // the checkout. Silence is right when they are the same corpus in two forms and wrong
    // when they are not, and only the genesis digest in the unpacked manifest can tell those
    // apart — RFC-0032 §4.5, and #781, which is that digest arriving.
    let shadowed = crate::deps::shadowed(root);

    // An unpacked bundle whose `manifest.yml` does not parse. Read here rather than inferred
    // from a missing field, because absence is legitimate in this format and unreadable is not
    // — and the check just above is one of the things it silently switches off: `shadowed`
    // compares `genesis_hash` and is documented to stay quiet when either side is unknown, so a
    // manifest nobody could read reads as a bundle that declined to say which corpus it is.
    let unreadable = crate::deps::unreadable_manifests(root);

    for (name, dep) in &config.dependencies {
        // A path dependency is read where it sits and has nothing to fetch, which is exactly
        // what `cmd_install` decides about it. Counted, not graded.
        //
        // A *pinned* path (#1466) is installed and locked like a fetched one, and graded
        // like one below.
        if dep.is_live_path() {
            local += 1;
            continue;
        }
        // A pinned path whose lock entry is at another commit than the declaration is not
        // installed at the declared one — the same state as no entry, and the same remedy.
        let Some(locked) = lock
            .packages
            .iter()
            .find(|p| &p.name == name && (dep.rev.is_none() || p.rev == dep.rev))
        else {
            // Declared and never pinned. The lock is the correctness story for a fetched
            // corpus, so a dependency outside it is not verifiable — but it is also the
            // normal state between `tonpa add` and the first install, so it is not a failure.
            unlocked.push(name.clone());
            continue;
        };
        match crate::deps::verify_installed(name, &tonpa_dir, locked) {
            Ok(true) => ok += 1,
            // `verify_installed` answers one question — is the bundle the pinned one — and
            // returns false for both "absent" and "different". A caller that can re-fetch
            // does not care which; this one cannot fetch anything, so the remedy it prints
            // is the same but the sentence it writes is not.
            Ok(false) if !crate::deps::installed_marker(name, &tonpa_dir, locked).exists() => {
                missing.push(name.clone())
            }
            Ok(false) => corrupt.push(name.clone()),
            // Unreadable is not intact. Reporting it as present would be the one wrong
            // answer here.
            Err(_) => corrupt.push(name.clone()),
        }
    }

    let mut detail = Vec::new();
    if ok > 0 {
        detail.push(format!("{ok} installed"));
    }
    if local > 0 {
        detail.push(format!("{local} path"));
    }
    if !missing.is_empty() {
        detail.push(format!("not installed: {}", missing.join(", ")));
    }
    if !corrupt.is_empty() {
        detail.push(format!("does not match tonpa.lock: {}", corrupt.join(", ")));
    }
    if !unlocked.is_empty() {
        detail.push(format!(
            "declared but never pinned: {}",
            unlocked.join(", ")
        ));
    }
    for s in &shadowed {
        detail.push(format!(
            "{}: the path checkout (corpus {}) and the unpacked bundle (corpus {}) are \
             different corpora under one name — the checkout is what gets read",
            s.name,
            short(&s.path_genesis),
            short(&s.fetched_genesis),
        ));
    }
    for (name, why) in &unreadable {
        detail.push(format!(
            "{name}: the unpacked bundle's manifest.yml does not parse ({why}) — its \
             commit, genesis and index model are read as absent, and absent is what an \
             older bundle format looks like"
        ));
    }
    let detail = detail.join("; ");

    // A shadow outranks a missing install: one is the wrong corpus being read, the other is
    // no corpus being read, and the second is visible the moment anything asks for it. An
    // unreadable manifest ranks with the reinstall cases rather than with the shadow: the
    // repair is to fetch the bundle again, and `tonpa-install` is already the remedy printed.
    if !shadowed.is_empty() {
        Answer::fail(detail, Some(SHADOW_REMEDY))
    } else if !missing.is_empty() || !corrupt.is_empty() || !unreadable.is_empty() {
        Answer::fail(detail, Some(REMEDY))
    } else if !unlocked.is_empty() {
        Answer::warn(detail, Some(REMEDY))
    } else {
        Answer::ok(detail)
    }
}

/// A genesis hash abbreviated for a diagnostic line, where the whole of it earns nothing.
///
/// Twelve, matching `export_rdf`'s subject component, so the two places a person sees this
/// value abbreviated spell it the same way. Shorter than the full hash and long past the
/// point where two corpora in one tree could collide.
pub(super) fn short(hash: &str) -> &str {
    // `get` rather than an index: a slice of a value read off disk is a panic path.
    hash.get(..12).unwrap_or(hash)
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use tempfile::TempDir;

    /// A repository declaring corpora, with control over what is on disk and in the lock.
    fn repo_with_corpora(deps: &str, lock: &str, bundles: &[(&str, &[u8])]) -> TempDir {
        let tmp = derived_repo();
        std::fs::write(crate::paths::tonpa_config_path(tmp.path()), deps).unwrap();
        let dir = crate::paths::tonpa_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        if !lock.is_empty() {
            std::fs::write(crate::paths::tonpa_lock_path(tmp.path()), lock).unwrap();
        }
        for (name, bytes) in bundles {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join("bundle.yiz"), bytes).unwrap();
        }
        tmp
    }

    fn locked(name: &str, bytes: &[u8]) -> String {
        format!(
            "[[package]]\nname = \"{name}\"\nurl = \"https://example.com/{name}.yiz\"\nsha256 = \"{}\"\n",
            crate::deps::sha256_hex(bytes)
        )
    }

    /// A repository that depends on nothing is not half-provisioned.
    ///
    /// It must not read as "0 corpora missing" either — a verdict on a question nobody put is
    /// the failure `check_catalog` avoids by staying quiet where no TTL applies.
    #[test]
    fn declaring_no_corpora_is_not_a_finding() {
        let tmp = derived_repo();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert_eq!(c.detail, "none declared");
        assert!(c.remedy.is_none(), "nothing to do, so nothing to suggest");
    }

    /// A `tonpa.toml` that does not parse declares nothing anyone can read, and is not a
    /// repository that depends on nothing (#1616). The control is the same file before the
    /// typo, so the failure is the typo and not the fixture.
    #[test]
    fn a_tonpa_toml_that_does_not_parse_fails_rather_than_declaring_none() {
        let declared = "[dependencies.hydrology]\npath = \"../hydrology\"\n";
        let tmp = repo_with_corpora(declared, "", &[]);
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert_ne!(c.detail, "none declared");

        let config = crate::paths::tonpa_config_path(tmp.path());
        std::fs::write(&config, format!("{declared}\n[broken\n")).unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("tonpa.toml") && c.detail.contains("line 4"),
            "names the file and the line: {}",
            c.detail
        );
        assert!(!c.detail.contains('\n'), "one line: {}", c.detail);
    }

    /// The state #397's hook can leave behind: mise logged a WARN, `mise install` exited 0,
    /// and the corpus is not there. Nothing else in this repository reports it.
    #[test]
    fn a_declared_corpus_that_never_arrived_fails() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(c.detail.contains("not installed") && c.detail.contains("hydrology"));
        assert_eq!(c.remedy.as_deref(), Some("mise run tonpa-install"));
    }

    /// Present but not the pinned bytes. Distinct from absent, and said differently — the
    /// shared `verify_installed` returns false for both, because the caller that can re-fetch
    /// does not care which.
    #[test]
    fn a_corpus_that_does_not_match_the_lock_fails_and_says_so() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"what we pinned"),
            &[("hydrology", b"something else entirely")],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(
            c.detail.contains("does not match tonpa.lock"),
            "a corrupt corpus must not be reported as a missing one: {}",
            c.detail
        );
        assert!(!c.detail.contains("not installed"), "{}", c.detail);
    }

    /// Unpacked and matching the lock. The whole point is that this is answerable offline.
    #[test]
    fn a_corpus_matching_its_lock_entry_passes() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[("hydrology", b"bundle bytes")],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert!(c.detail.contains("1 installed"), "{}", c.detail);
    }

    /// A `tonpa.lock` that does not parse pins nothing anyone can read, and is not a lock that
    /// was never written (#1657). The control is the same lock before the typo.
    #[test]
    fn a_tonpa_lock_that_does_not_parse_fails_rather_than_reading_as_unpinned() {
        let lock = locked("hydrology", b"bundle bytes");
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &lock,
            &[("hydrology", b"bundle bytes")],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);

        let path = crate::paths::tonpa_lock_path(tmp.path());
        std::fs::write(&path, format!("{lock}\n[[package\n")).unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("tonpa.lock") && c.detail.contains("line 6"),
            "names the file and the line: {}",
            c.detail
        );
        assert!(!c.detail.contains("never pinned"), "{}", c.detail);
        assert!(!c.detail.contains('\n'), "one line: {}", c.detail);
    }

    /// Declared, never pinned. Normal between `tonpa add` and the first install, so a warning
    /// rather than a failure — but not silence: the lock is the entire correctness story for
    /// a fetched corpus, and a dependency outside it is not verifiable at all.
    #[test]
    fn a_dependency_with_no_lock_entry_warns_rather_than_fails() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            "",
            &[],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Warn, "detail was: {}", c.detail);
        assert!(c.detail.contains("never pinned"), "{}", c.detail);
    }

    /// A path dependency has nothing to fetch, which is exactly what `cmd_install` decides
    /// about it. Counted, not graded — and specifically not reported as missing, since there
    /// is no bundle under `.yidam/tonpa/` for one and never will be.
    #[test]
    fn a_path_dependency_is_counted_and_not_graded() {
        let tmp = repo_with_corpora("[dependencies.sibling]\npath = \"../sibling\"\n", "", &[]);
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert!(c.detail.contains("1 path"), "{}", c.detail);
    }

    /// Two corpora under one name, which `deps::resolved` resolves silently in favour of the
    /// checkout — right when they are the same corpus in two forms, and a wrong answer when
    /// they are not. Nothing could tell those apart until the manifest carried a genesis
    /// digest (#781): the declared names agree, and `genesis` is a date, which two corpora
    /// created on one day also agree on.
    ///
    /// Graded above a missing install deliberately. A corpus that is not there is visible the
    /// moment anything asks for it; a corpus that is the wrong one answers every question.
    #[test]
    fn a_checkout_shadowing_a_different_corpus_fails_and_names_both() {
        let tmp = repo_with_corpora("[dependencies.dep]\npath = \"sibling\"\n", "", &[]);
        let sibling = tmp.path().join("sibling");
        let sibling_hash = nested_corpus(&sibling);
        let unpacked = crate::paths::tonpa_dir(tmp.path()).join("dep");
        std::fs::create_dir_all(&unpacked).unwrap();
        std::fs::write(
            unpacked.join("manifest.yml"),
            "bundle_version: \"1\"\ngenesis_hash: \"0000111122223333444455556666777788889999\"\n",
        )
        .unwrap();

        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("under one name") && c.detail.contains("dep:"),
            "{}",
            c.detail
        );
        assert!(
            c.detail.contains(&sibling_hash[..12]) && c.detail.contains("000011112222"),
            "both identities have to be in the line, or it names no evidence: {}",
            c.detail
        );
        assert_eq!(
            c.remedy.as_deref(),
            Some("rm -r .yidam/tonpa/<name>, or rename one of the two"),
            "installing again cannot fix a name claimed by two corpora"
        );
    }

    /// The same fixture where the two *are* one corpus. This is the development loop — fetch
    /// a dependency, then point at a checkout of it to edit — and it must stay quiet, or the
    /// check reports the normal case forever.
    #[test]
    fn a_checkout_of_the_same_corpus_is_not_a_shadow() {
        let tmp = repo_with_corpora("[dependencies.dep]\npath = \"sibling\"\n", "", &[]);
        let sibling_hash = nested_corpus(&tmp.path().join("sibling"));
        let unpacked = crate::paths::tonpa_dir(tmp.path()).join("dep");
        std::fs::create_dir_all(&unpacked).unwrap();
        std::fs::write(
            unpacked.join("manifest.yml"),
            format!("bundle_version: \"1\"\ngenesis_hash: \"{sibling_hash}\"\n"),
        )
        .unwrap();

        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert!(c.detail.contains("1 path"), "{}", c.detail);
    }

    /// Twelve, matching the component `export_rdf` builds a subject from, so the two places a
    /// person meets an abbreviated genesis hash spell it the same way — and a value shorter
    /// than the window comes back whole rather than panicking on a slice.
    #[test]
    fn a_digest_is_abbreviated_to_the_length_rdf_subjects_use() {
        assert_eq!(
            short("da4eeb36530f1111222233334444555566667777"),
            "da4eeb36530f"
        );
        assert_eq!(short("da4eeb"), "da4eeb");
        assert_eq!(short(""), "");
    }

    /// A corpus checkout with one commit of its own, so it has a genesis hash to compare.
    fn nested_corpus(dir: &Path) -> String {
        std::fs::create_dir_all(crate::paths::yidam_corpus_dir(dir)).unwrap();
        std::fs::write(
            crate::paths::yidam_corpus_dir(dir).join("a.ont.yml"),
            "x: 1\n",
        )
        .unwrap();
        crate::git::fixture::repo(dir, "genesis: a corpus");
        crate::git::genesis_hash(dir).expect("the fixture checkout has a root commit")
    }

    /// An unreadable bundle is not an intact one.
    ///
    /// `verify_installed` returns `Err` rather than `false` when the file is there and cannot
    /// be read, and the one wrong answer here is to let that count as installed.
    #[test]
    fn an_unreadable_bundle_is_not_reported_as_present() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[("hydrology", b"bundle bytes")],
        );
        // A directory where the bundle should be: exists, and cannot be read as a file.
        let bundle = crate::paths::tonpa_dir(tmp.path())
            .join("hydrology")
            .join("bundle.yiz");
        std::fs::remove_file(&bundle).unwrap();
        std::fs::create_dir(&bundle).unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
    }

    /// The #1081 arm. An installed corpus that matches `tonpa.lock` is otherwise entirely
    /// healthy here — the digest check passes, because the digest is over `bundle.yiz` and the
    /// manifest is what was written *out of* it. So the one thing that reads the manifest
    /// silently reads every field as absent, and `shadowed` — documented to stay quiet when
    /// either genesis is unknown — is switched off by it without saying so.
    #[test]
    fn an_unpacked_manifest_that_does_not_parse_is_reported() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[("hydrology", b"bundle bytes")],
        );
        let unpacked = crate::paths::tonpa_dir(tmp.path()).join("hydrology");

        // The control first: the same tree with a manifest that reads is clean, so the
        // failure below is the manifest and not the fixture.
        std::fs::write(
            unpacked.join("manifest.yml"),
            "bundle_version: \"1\"\ncommit: \"abc1234\"\n",
        )
        .unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);

        std::fs::write(unpacked.join("manifest.yml"), "commit: \"unclosed\n").unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("hydrology") && c.detail.contains("manifest.yml does not parse"),
            "{}",
            c.detail
        );
        assert_eq!(c.remedy.as_deref(), Some("mise run tonpa-install"));
    }

    /// And a dependency with nothing unpacked keeps the sentence it had. The failure is
    /// already reported as not installed, and a second line saying its manifest could not be
    /// read would be describing the same absence twice.
    #[test]
    fn a_corpus_that_is_not_installed_is_not_also_reported_as_unreadable() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(c.detail.contains("not installed"), "{}", c.detail);
        assert!(!c.detail.contains("manifest.yml"), "{}", c.detail);
    }
}
