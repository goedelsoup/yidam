use super::*;

/// Which declaration this repository adopted, and whether the vendored profile still matches
/// it — RFC-0028 §9.
///
/// **Holding none is `Ok`.** That was every one of the eighteen corpora A0 measured, and a
/// state eighteen repositories are in is not a fault to be listed under things to fix.
///
/// What warns is a declaration that cannot be honoured: a record naming a profile nothing
/// vendored, or one whose revision has moved out from under it. Without the revision, `score`
/// would score a repository against a kuten it may not hold and `fit` would compare two
/// holding different ones — A0's own confound designed into A0's deliverable.
pub(super) fn check_kuten(root: &Path) -> Answer {
    let declaration = match crate::kuten::read_declaration(root) {
        Ok(d) => d,
        Err(e) => {
            return Answer::warn(
                e.to_string(),
                Some("fix the decision record, or delete it to hold no kuten"),
            )
        }
    };
    let Some(declaration) = declaration else {
        return Answer::ok("none — the loop runs on the template's defaults");
    };
    match crate::kuten::read_profile(root, &declaration.name) {
        Err(e) => Answer::warn(e.to_string(), Some("re-vendor the prelude")),
        Ok(None) => Answer::warn(
            format!(
                "`{}` is declared and no profile is vendored for it",
                declaration.name
            ),
            Some("mise run yidam-vendor-update"),
        ),
        Ok(Some(profile)) if profile.revision != declaration.revision => Answer::warn(
            format!(
                "`{}` at revision {}, vendored at revision {}",
                declaration.name, declaration.revision, profile.revision
            ),
            Some("re-vendor, or record a superseding `decide:` decision"),
        ),
        Ok(Some(profile)) => Answer::ok(format!(
            "`{}`, revision {} — {}",
            profile.name,
            profile.revision,
            object_state(profile.object.as_ref())
        )),
    }
}

/// Is the declaration somewhere an agent will meet it? — #694.
///
/// A separate question from [`check_kuten`], and separate because it has a different subject:
/// that one answers *which kuten does this repository hold*, from the decision record and the
/// vendored profile. This one asks whether anything in the loop **reads** it. A repository can
/// answer the first perfectly and the second not at all, and one of the two repositories that
/// adopted on day one did exactly that — `adopt` skipped the `AGENTS.md` write because there
/// was no `AGENTS.md`, and `regen --check`, `doctor` and `kuten` all reported it as fine.
///
/// The module `adopt` lives in names this outcome as the failure the layer exists to prevent:
/// *"A declaration nothing in the loop reads is this epic's own diagnosed failure aimed at its
/// centrepiece."*
///
/// **Warn, never fail.** The remedy is a document a person writes, the declaration is still
/// true, and nothing about the corpus is broken — the same reasoning `kuten check` runs on.
/// Holding no kuten skips the question entirely rather than passing it: there is nothing to
/// carry, and an `ok` there would read as *something checked and found present*.
pub(super) fn check_kuten_read(root: &Path) -> Answer {
    let held = match crate::kuten::read_declaration(root) {
        Ok(Some(d)) => d,
        // Unreadable or absent: `kuten` above is the check that says so, and two verdicts on
        // one record would be two answers to one question.
        Ok(None) | Err(_) => {
            return Answer::skipped("no kuten is declared, so nothing carries one")
        }
    };
    match carriers(root).as_slice() {
        [] => Answer::warn(
            format!(
                "`{}` is declared and no file carries the `yidam kuten` block, so nothing an \
                 agent reads mentions it",
                held.name
            ),
            Some("add the `yidam kuten` REGEN section to AGENTS.md, then run `yidam kuten`"),
        ),
        [one] => Answer::ok(format!("`{}` — {one} carries the block", held.name)),
        many => Answer::ok(format!(
            "`{}` — {} files carry the block",
            held.name,
            many.len()
        )),
    }
}

/// Repository-relative paths carrying the `yidam kuten` REGEN marker.
///
/// `AGENTS.md` is where `adopt` and the scaffold put it and where `write_block` fills it, so
/// it is checked first and by name. The wider scan exists because the marker is what actually
/// decides: a corpus that briefs its agents in `CLAUDE.md` and pasted the section there has
/// not failed at anything, and a check that only knew one filename would tell it that it had.
fn carriers(root: &Path) -> Vec<String> {
    const MARKER: &str = "<!-- REGEN: yidam kuten";
    let mut found = Vec::new();
    for rel in ["AGENTS.md", "CLAUDE.md", "README.md", ".yidam/AGENTS.md"] {
        if std::fs::read_to_string(root.join(rel)).is_ok_and(|t| t.contains(MARKER)) {
            found.push(rel.to_string());
        }
    }
    found
}

/// Which way the arrow between corpus and object runs — RFC-0028 §6, and #582's acceptance
/// criterion.
///
/// **This line is why #582 closes.** An undeclared untracked corpus is today
/// indistinguishable from no corpus at all, and #582's third question was precisely whether
/// anything says which state a repository is in. It is on the existing `kuten` check's text
/// rather than a check of its own: the direction is part of the answer to *which kuten does
/// this repository hold*, not a second question, and every consumer of `doctor` keys on the
/// check id.
///
/// A profile that leaves the slot unpopulated says nothing here — the difference between
/// *this practice makes no claim* and *this repository was not read*, kept the same way
/// [`crate::kuten::compare`] keeps it.
fn object_state(object: Option<&crate::kuten::Object>) -> String {
    match object {
        None => "the object slot is unpopulated, so nothing is declared about direction".into(),
        Some(o) => format!("{} ({})", o.direction.describe(), o.direction.name()),
    }
}
