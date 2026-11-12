//! The two checks over `.yidam/asks/` (#1467): an ask addressed to nothing, and an ask whose
//! header cannot say what state it is in.
//!
//! Both gate. Neither reads the far side, so neither can turn a repository red because a
//! producer moved — that is the `asks` clock in `due`, which reports and never fails.

use std::path::Path;

use super::model::{Check, Severity, Violation};
use crate::asks::{Ask, STATUSES};

pub const UNRESOLVED: &str = "ask-unresolved";
pub const MALFORMED: &str = "ask-malformed";

/// Both checks over one read of the directory, in registry order.
pub fn checks(root: &Path) -> [Check; 2] {
    let asks = crate::asks::read(root);
    let declared = crate::deps::load_config(&crate::paths::tonpa_config_path(root)).dependencies;
    let unresolved = asks
        .iter()
        .filter_map(|a| match a.package.as_deref() {
            None => Some(Violation::new(
                &a.rel,
                "an ask directly under .yidam/asks/ names no dependency — move it into \
                 .yidam/asks/<package>/, named as .yidam/tonpa.toml names the corpus it asks",
            )),
            Some(p) if !declared.contains_key(p) => Some(Violation::new(
                &a.rel,
                format!(
                    "asks `{p}`, which .yidam/tonpa.toml does not declare — an ask is addressed \
                     to a corpus this repository depends on, and its directory names which"
                ),
            )),
            Some(_) => None,
        })
        .collect();
    [
        Check::new(
            UNRESOLVED,
            "Ask addressed to a corpus this repository does not depend on",
            Severity::Error,
            "An ask's directory is the corpus it is addressed to, and the pin it was read \
             against means something only against a dependency this repository declares. An \
             ask to an undeclared corpus can never come due, so it would sit in the outbox \
             unread — the failure the directory exists to end.",
            unresolved,
        ),
        Check::new(
            MALFORMED,
            "Ask whose header cannot say what state it is in",
            Severity::Error,
            "An ask records the far-side commit it was read against and where it stands. \
             Without a pin nothing can tell when the far side has moved past it, and with a \
             status outside the four, or an answer with no commit, a reader is back to \
             maintaining its state in prose — which is how drafts went stale before this \
             directory existed.",
            asks.iter().flat_map(|a| malformed(a, &asks)).collect(),
        ),
    ]
}

fn malformed(ask: &Ask, all: &[Ask]) -> Vec<Violation> {
    let at = |detail: String| Violation::new(&ask.rel, detail);
    if let Some(why) = &ask.unreadable {
        return vec![at(format!("header does not parse: {why}"))];
    }
    let h = &ask.header;
    let mut out = Vec::new();
    if h.pin.as_deref().is_none_or(|p| p.trim().is_empty()) {
        out.push(at(
            "no `pin:` — name the far side's commit this question was read against".into(),
        ));
    }
    match h.status.as_deref() {
        None => out.push(at(format!("no `status:` — one of {}", STATUSES.join(", ")))),
        Some(s) if !STATUSES.contains(&s) => out.push(at(format!(
            "status `{s}` is not one of {}",
            STATUSES.join(", ")
        ))),
        Some("answered") if h.answered_at.as_deref().is_none_or(|c| c.trim().is_empty()) => out
            .push(at(
                "`status: answered` with no `answered_at:` — name the far side's commit that \
                 answered it"
                    .into(),
            )),
        Some("superseded") => match h.superseded_by.as_deref() {
            None => out.push(at(
                "`status: superseded` with no `superseded_by:` — name the ask that replaced it"
                    .into(),
            )),
            Some(by) if !all.iter().any(|o| o.package == ask.package && o.slug == by) => {
                out.push(at(format!(
                    "superseded by `{by}`, which is not an ask in the same directory"
                )))
            }
            Some(_) => {}
        },
        Some(_) => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let p = tmp.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        tmp
    }

    const DECLARED: (&str, &str) = (".yidam/tonpa.toml", "[dependencies.up]\npath = \"../up\"\n");

    fn found(tmp: &tempfile::TempDir, id: &str) -> Vec<String> {
        checks(tmp.path())
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .violations
            .iter()
            .map(|v| v.detail.clone())
            .collect()
    }

    #[test]
    fn a_sound_ask_to_a_declared_dependency_is_clean() {
        let tmp = repo(&[
            DECLARED,
            (".yidam/asks/up/q.md", "---\npin: abc\nstatus: open\n---\n"),
        ]);
        assert!(found(&tmp, UNRESOLVED).is_empty());
        assert!(found(&tmp, MALFORMED).is_empty());
    }

    #[test]
    fn an_ask_to_an_undeclared_corpus_or_to_none_is_unresolved() {
        let tmp = repo(&[
            DECLARED,
            (
                ".yidam/asks/elsewhere/q.md",
                "---\npin: a\nstatus: open\n---\n",
            ),
            (".yidam/asks/loose.md", "---\npin: a\nstatus: open\n---\n"),
        ]);
        let v = found(&tmp, UNRESOLVED);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v.iter().any(|d| d.contains("`elsewhere`")), "{v:?}");
        assert!(v.iter().any(|d| d.contains("names no dependency")), "{v:?}");
    }

    #[test]
    fn each_malformed_header_is_reported_for_its_own_reason() {
        let tmp = repo(&[
            DECLARED,
            (".yidam/asks/up/no-pin.md", "---\nstatus: open\n---\n"),
            (".yidam/asks/up/no-status.md", "---\npin: a\n---\n"),
            (
                ".yidam/asks/up/bad-status.md",
                "---\npin: a\nstatus: sent\n---\n",
            ),
            (
                ".yidam/asks/up/unanswered.md",
                "---\npin: a\nstatus: answered\n---\n",
            ),
            (
                ".yidam/asks/up/orphan.md",
                "---\npin: a\nstatus: superseded\nsuperseded_by: nobody\n---\n",
            ),
            (".yidam/asks/up/broken.md", "---\npin: [\n---\n"),
        ]);
        let v = found(&tmp, MALFORMED);
        for needle in [
            "no `pin:`",
            "no `status:`",
            "status `sent`",
            "no `answered_at:`",
            "superseded by `nobody`",
            "does not parse",
        ] {
            assert!(v.iter().any(|d| d.contains(needle)), "{needle}: {v:?}");
        }
        assert_eq!(v.len(), 6, "{v:?}");
    }

    #[test]
    fn a_supersession_resolves_within_its_own_directory() {
        let tmp = repo(&[
            DECLARED,
            (
                ".yidam/asks/up/old.md",
                "---\npin: a\nstatus: superseded\nsuperseded_by: new\n---\n",
            ),
            (".yidam/asks/up/new.md", "---\npin: b\nstatus: open\n---\n"),
        ]);
        assert!(found(&tmp, MALFORMED).is_empty());
    }
}
