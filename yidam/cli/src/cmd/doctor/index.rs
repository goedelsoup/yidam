use super::*;

/// Is the index built, and is it stale against the corpus?
///
/// Never a failure. A light `reports` build cannot build one, and a repository that has no
/// use for semantic search is not misconfigured for lacking it.
pub(super) fn check_index(root: &Path) -> Answer {
    const BUILD: &str = "yidam index-build (needs the `index` feature)";
    let resolution = crate::index_source::resolve(root);
    let data = crate::cmd::index_status_from(root, &resolution);
    if !data.index_present {
        // A linked worktree whose main worktree has an index this branch does not contain
        // (RFC-0069 §2). Saying only "no index here" would name one repair of two, and the
        // one nobody takes for a checkout that lives a day.
        if let crate::index_source::Resolution::Refused {
            dir,
            indexed_commit,
        } = &resolution
        {
            return Answer::warn(
                format!(
                    "no {}, and the main worktree's index at {} was built at {indexed_commit}, \
                     which this branch does not contain",
                    yidam_index_dir(root).display(),
                    dir.display()
                ),
                Some(
                    "merge the baseline into this branch, or run yidam index-build here (needs \
                     the `index` feature)",
                ),
            );
        }
        return Answer::warn(
            format!("no {}", yidam_index_dir(root).display()),
            Some(BUILD),
        );
    }
    let index_dir = data
        .index_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| yidam_index_dir(root));
    // Named on a pass as on a warning: an index this checkout did not build is a fact a
    // reader should not have to infer from a path that is not theirs.
    let shared = match data.index_source {
        Some("shared") => format!("; the main worktree's, at {}", index_dir.display()),
        _ => String::new(),
    };
    if !data.meta_present {
        return Answer::warn(
            "index present, but it carries no readable meta.json",
            Some(BUILD),
        );
    }
    // An index of whole nodes (RFC-0060): still read, as passage 0 of each node, and blind
    // past the embedding window on every node longer than it. A rebuild is all it takes.
    let contract = index_dir.join(crate::embed_config::EMBED_CONFIG_FILENAME);
    if let Some(parsed) = std::fs::read_to_string(&contract)
        .ok()
        .map(|t| crate::embed_config::EmbedConfig::from_json(&t))
    {
        // A contract every reader on this binary refuses — a later yidam's, or one that does
        // not parse — is not an index that is fine. Saying nothing here reported Ok over an
        // index `retrieve` will not read.
        let config = match parsed {
            Ok(config) => config,
            Err(e) => {
                return Answer::warn(
                    format!("{} is refused: {e:#}", contract.display()),
                    Some("move this pin to the release that built it, or yidam index-build"),
                );
            }
        };
        if config.is_whole_nodes() {
            return Answer::warn(
                format!(
                    "built {} with format_version \"1\", one row per node: a long node is \
                     embedded only as far as the model's window reaches",
                    data.built.clone().unwrap_or_default()
                ),
                Some(BUILD),
            );
        }
    }
    if data.stale_nodes > 0 {
        return Answer::warn(
            format!(
                "built {}, and {} corpus file(s) have changed since{shared}",
                data.built.clone().unwrap_or_default(),
                data.stale_nodes
            ),
            Some(BUILD),
        );
    }
    Answer::ok(format!(
        "built {}, {} node(s), model {}{shared}",
        data.built.clone().unwrap_or_default(),
        data.node_count.unwrap_or(0),
        data.model.clone().unwrap_or_default()
    ))
}

/// What has this corpus computed about itself, and does it still stand?
///
/// The question #1028 was filed about. `.yidam/computed/` was written from the moment a
/// calculator was declared and nothing ever read it, so a corpus could hold a stale answer,
/// an answer no capability admits to writing, or two files quarrelling over one signal name,
/// and the only symptom was a feature silently missing from a search.
///
/// It asks two separate things and reports the worse of them, in this order:
///
/// 1. **Can the files be read as signal tables at all** — [`crate::computed::Signals`]'s
///    problems. These are `fail` rather than `warn` because each one means a computed answer
///    is not reaching the embedding record: a name refused for colliding is dropped, and a row
///    keyed on a node this corpus does not hold attaches to nothing.
/// 2. **Does the answer still stand** — [`crate::cmd::run::standing`], which is the run's own
///    input-state equality asked of the working tree. Two shapes of drift, and they are
///    different findings: a computed file whose bytes are not the ones its receipt recorded was
///    edited by hand, and a step whose inputs moved is owed a run.
///
/// `ok` on a corpus that computes nothing, and it says so rather than reporting the absence as
/// a thing to fix: fifteen of the sixteen corpora derived from this template declare no
/// calculator, and a check that found that remarkable would be noise in all fifteen.
pub(super) fn check_computed(root: &Path) -> Answer {
    const RUN: &str = "yidam run";
    let signals = crate::computed::Signals::load(root);
    let steps = match crate::cmd::run::standing(root) {
        Ok(steps) => steps,
        // The manifest is the run's own contract and `yidam run` is what reports a broken one.
        // Saying so at `warn` here rather than `fail` keeps one finding in one place: a corpus
        // whose manifest does not parse has a `run` problem, not a `computed` problem.
        Err(e) => {
            return Answer::warn(
                format!(
                    "the capability manifest could not be read, so freshness is unknown: {e:#}"
                ),
                Some(RUN),
            )
        }
    };

    if !signals.problems.is_empty() {
        return Answer::fail(
            enumerate(&signals.problems, "problem"),
            Some("fix the capability that writes the file, then `yidam run`"),
        );
    }

    if signals.files.is_empty() && steps.is_empty() {
        return Answer::ok("this corpus computes nothing about itself, and declares no calculator");
    }

    let edited: Vec<String> = steps
        .iter()
        .flat_map(|s| &s.outputs)
        .filter(|o| o.unchanged == Some(false))
        .map(|o| o.path.clone())
        .collect();
    if !edited.is_empty() {
        return Answer::warn(
            format!(
                "{} — edited since the run that wrote them, so the receipt no longer describes \
                 what is on disk",
                enumerate(&edited, "file")
            ),
            Some(RUN),
        );
    }

    let gone: Vec<String> = steps
        .iter()
        .flat_map(|s| &s.outputs)
        .filter(|o| o.unchanged.is_none())
        .map(|o| o.path.clone())
        .collect();
    if !gone.is_empty() {
        // Not `yidam run`, and this is the case the remedy matters most for: a run commits and
        // leaves the checkout alone, so immediately after one these files are at HEAD and not
        // on disk. Another run would find the input state unchanged and write nothing, which
        // would read as the command not working. The one that syncs a checkout is `git
        // restore`, and `cmd::run`'s own report names it for the same reason.
        return Answer::warn(
            format!(
                "{} — at HEAD and not in this checkout, which is where a run leaves them",
                enumerate(&gone, "file")
            ),
            Some("git restore --source=HEAD --worktree --staged -- .yidam/"),
        );
    }

    let stale: Vec<String> = steps
        .iter()
        .filter(|s| s.freshness == crate::cmd::run::Freshness::Stale)
        .map(|s| format!("{} ({})", s.step, s.because))
        .collect();
    if !stale.is_empty() {
        return Answer::warn(enumerate(&stale, "step"), Some(RUN));
    }

    // Last, so it never hides a finding about the answers themselves: it is about whether
    // another clone can see that they were re-checked (RFC-0052 §2.4).
    if crate::cmd::run::notes::unshared(root) {
        return Answer::warn(
            format!(
                "{} holds re-runs that reproduced their receipts, and no remote's fetch refspec \
                 reads it, so another clone re-runs those steps",
                crate::cmd::run::notes::REF
            ),
            Some(
                "git fetch origin refs/notes/yidam-runs:refs/notes-remote/origin/yidam-runs, \
                 then git notes --ref=yidam-runs merge -s cat_sort_uniq \
                 refs/notes-remote/origin/yidam-runs, then git push origin refs/notes/yidam-runs",
            ),
        );
    }

    Answer::ok(format!(
        "{} file(s) carry {} signal(s) on {} node(s); {} step(s) fresh{}",
        signals.files.len(),
        signals.names().len(),
        signals.nodes(),
        steps.len(),
        match signals.names().is_empty() {
            true => String::new(),
            false => format!(" — {}", signals.names().join(", ")),
        }
    ))
}

/// Does any committed output stamp the commit its inputs came from? (RFC-0052 §2.6)
///
/// `warn` and never `fail`: the stamp is not wrong, it is costly. An output that contains its
/// receipt's `input.commit` is new bytes at every run, so a re-run can never reproduce its
/// receipt, every re-check lands a commit, and a reader cannot tell from the log whether the
/// answer moved. The receipt already records the commit. `YIDAM_INPUT_COMMIT` is exported for a
/// step that needs to read history or name what it measured, not to be echoed into an output.
pub(super) fn check_stamps(root: &Path) -> Answer {
    let found = match crate::cmd::run::stamped(root) {
        Ok(found) => found,
        Err(e) => {
            return Answer::skipped(format!(
                "the capability manifest could not be read, so no receipt was found to compare: \
                 {e:#}"
            ))
        }
    };
    // Both findings in one warning: returning on the first would hide the second until it was
    // fixed.
    let mut found_wrong = Vec::new();
    let mut fixes = Vec::new();
    if !found.undeclared.is_empty() {
        found_wrong.push(format!(
            "{} — each contains its receipt's `input.commit`, so every run of it is new bytes \
             and none can reproduce its receipt",
            enumerate(&found.undeclared, "output")
        ));
        fixes.push(
            "stop writing YIDAM_INPUT_COMMIT into the output, since the receipt already records \
             it; or, if a consumer requires it, name the output under `stamps_input_commit`",
        );
    }
    if !found.stale.is_empty() {
        found_wrong.push(format!(
            "{} declared under `stamps_input_commit` but not containing its receipt's full \
             `input.commit`, so the declaration masks nothing",
            enumerate(&found.stale, "output")
        ));
        fixes.push(
            "remove the path from `stamps_input_commit` if its consumer no longer requires the \
             commit; only the full id is masked, so a step that writes an abbreviation should \
             write the whole of YIDAM_INPUT_COMMIT",
        );
    }
    if !found_wrong.is_empty() {
        let fix = fixes.join("; ");
        return Answer::warn(found_wrong.join("; "), Some(&fix));
    }
    match found.declared.is_empty() {
        true => Answer::ok("no committed output contains the commit its receipt was computed from"),
        false => Answer::ok(format!(
            "{} declared under `stamps_input_commit`, so a run masks the commit before \
             judging reproduction; no undeclared output contains it",
            enumerate(&found.declared, "output")
        )),
    }
}

/// `n thing(s): a, b, c` with the tail elided, for a detail line that has to stay one line.
///
/// Three rather than all of them, and the count first: `check_regen` joins its whole list and
/// gets away with it because a repository has a dozen REGEN blocks, while a corpus can key a
/// computed file per node and a detail naming four hundred of them is a report nobody reads.
fn enumerate(items: &[String], noun: &str) -> String {
    let head = items.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
    match items.len() > 3 {
        true => format!(
            "{} {noun}(s): {head}; and {} more",
            items.len(),
            items.len() - 3
        ),
        false => format!("{} {noun}(s): {head}", items.len()),
    }
}

/// Is `[index.remote]` well formed, and are the credentials for it in the environment —
/// **without asking the service**.
///
/// The same discipline `check_vault` states for itself: `doctor` answers what can be settled
/// locally, and whether the index actually answers is `yidam index-push --dry-run`'s question,
/// which is allowed to make a request. A `doctor` that reached the network would be a check
/// nobody can run on a plane and a check that fails for reasons that are not this repository's.
///
/// Ungated, and deliberately. A build that cannot query a remote index can still read the
/// declaration and say whether it is malformed — the same argument `config.rs` makes for
/// parsing the section in every build. A binary that silently ignored a section the corpus
/// committed would be the worse answer.
pub(super) fn check_remote_index(root: &Path) -> Answer {
    let config = crate::config::load_yidam_config(root).unwrap_or_default();
    let Some(declared) = config.index.remote.as_ref() else {
        return Answer::ok("no remote index declared");
    };

    let remote = match crate::s3vectors::RemoteIndex::resolve(declared) {
        Ok(r) => r,
        Err(e) => {
            return Answer::fail(
                first_line(&e.to_string()),
                Some("Fix `[index.remote]` in `.yidam/config.toml`."),
            )
        }
    };

    // The names this corpus declared for the others sharing that index (#835). Checked here
    // because a malformed table is a *startup* failure for `serve` and `retrieve` — the
    // corpus a person asks with the surface they ask through — and a `doctor` that passed on
    // a repository whose server will not start is answering a different question than the one
    // it was asked.
    let named = match crate::s3vectors::Aliases::resolve(&declared.corpora) {
        Ok(a) => a,
        Err(e) => {
            return Answer::fail(
                first_line(&e.to_string()),
                Some(
                    "A corpus is named by its corpus id: twelve characters of its genesis \
                      commit hash, or the id a projection declares in .yidam/corpus/corpus.toml. \
                      `yidam index-push --dry-run` lists the corpora an index holds.",
                ),
            )
        }
    };

    // Only a build that can send them asks whether they are there. Without `s3-vectors` the
    // index is unreachable whatever the environment holds, and the declaration above is
    // still checked (#1257).
    #[cfg(feature = "s3-vectors")]
    if let Err(e) = crate::vault::creds::resolve_scope(&crate::vault::creds::index_scope(), |k| {
        std::env::var(k).ok()
    }) {
        return Answer::warn(
            first_line(&e.to_string()),
            Some(
                "Credentials come from the environment only — `.yidam/config.toml` is \
                  committed and must never carry one.",
            ),
        );
    }

    let shared = named.declared().count();
    match shared {
        0 => Answer::ok(remote.describe()),
        n => Answer::ok(format!(
            "{} (+{n} corpus name(s) declared)",
            remote.describe()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// An index built before passages is read, and reported as owed a rebuild (RFC-0060).
    #[test]
    fn an_index_of_whole_nodes_is_reported_stale_by_its_format() {
        let tmp = TempDir::new().unwrap();
        let index = yidam_index_dir(tmp.path());
        std::fs::create_dir_all(&index).unwrap();
        std::fs::write(
            index.join("meta.json"),
            r#"{"generated_at": 1, "model_name": "m", "node_count": 1}"#,
        )
        .unwrap();
        let mut config = crate::embed_config::EmbedConfig::for_fastembed_model("m", 4, "f", "E");
        let contract = index.join(crate::embed_config::EMBED_CONFIG_FILENAME);

        std::fs::write(&contract, serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(check_index(tmp.path()).verdict, Verdict::Ok);

        config.format_version = crate::embed_config::FORMAT_VERSION_WHOLE_NODES.to_string();
        config.passages = None;
        std::fs::write(&contract, serde_json::to_string(&config).unwrap()).unwrap();
        let answer = check_index(tmp.path());
        assert_eq!(answer.verdict, Verdict::Warn);
        assert!(
            answer.detail.contains("one row per node"),
            "{}",
            answer.detail
        );
        assert!(answer.remedy.unwrap().contains("index-build"));
    }

    /// A contract this binary refuses is not reported Ok (RFC-0060 §2.5).
    #[test]
    fn an_index_from_a_later_yidam_is_not_reported_ok() {
        let tmp = TempDir::new().unwrap();
        let index = yidam_index_dir(tmp.path());
        std::fs::create_dir_all(&index).unwrap();
        std::fs::write(
            index.join("meta.json"),
            r#"{"generated_at": 1, "model_name": "m", "node_count": 1}"#,
        )
        .unwrap();
        let mut config = crate::embed_config::EmbedConfig::for_fastembed_model("m", 4, "f", "E");
        config.format_version = "3".to_string();
        std::fs::write(
            index.join(crate::embed_config::EMBED_CONFIG_FILENAME),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();
        let answer = check_index(tmp.path());
        assert_eq!(answer.verdict, Verdict::Warn);
        assert!(answer.detail.contains("\"3\""), "{}", answer.detail);
    }
}
