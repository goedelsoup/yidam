//! `prelude_sources` — which packs a corpus vendors, and at what version (RFC-0048 §3).
//!
//! ```yaml
//! # .yidam/decisions/proposals.yml
//! prelude_sources:
//!   - scholarly@^0.1
//!   - us-oh-legislature@^0.3 from github.com/<owner>/<corpus>@<commit>
//! ```
//!
//! The first form names a pack the template ships. The second names a pack another corpus
//! wrote, at one commit of its repository.
//!
//! # A range at a commit is checked, not resolved
//!
//! A commit holds exactly one `pack.toml`, so a `from` pin has exactly one candidate version,
//! just as a template pin does. Its range is an assertion the vendor step checks: `^0.3`
//! refuses a commit whose pack has moved to `0.4.0`. Nothing walks the other repository's
//! history looking for a commit that would satisfy it. RFC-0048 records this as its sixth
//! decision, settled in #1315.
//!
//! The commit is a full 40-character sha. A short one names a different commit once the
//! other repository grows another with the same prefix, and the pin is the record of which
//! bytes were reviewed.

use std::fmt;

use super::version::Range;

/// One entry of `prelude_sources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub pack: String,
    pub range: Range,
    pub from: Option<Peer>,
    raw: String,
}

/// Another corpus's repository, at one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub repo: String,
    pub commit: String,
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl Pin {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let words: Vec<&str> = raw.split_whitespace().collect();
        let (head, from) = match words.as_slice() {
            [head] => (*head, None),
            [head, "from", peer] => (*head, Some(*peer)),
            _ => {
                return Err(format!(
                    "`{raw}` is neither `<pack>@<range>` nor `<pack>@<range> from <repo>@<commit>`"
                ))
            }
        };
        let (pack, range) = head
            .split_once('@')
            .ok_or_else(|| format!("`{head}` names no range — write `{head}@^<major>.<minor>`"))?;
        if !is_pack_name(pack) {
            return Err(format!(
                "`{pack}` is not a pack name: lowercase letters, digits and `-`, starting with a letter"
            ));
        }
        let range = Range::parse(range).ok_or_else(|| {
            format!(
                "`{range}` is not a range this binary reads: `^X.Y`, `~X.Y.Z`, `=X.Y.Z` or `X.Y`"
            )
        })?;
        let from = from
            .map(|peer| {
                // The last `@`: an ssh remote (`git@github.com:o/r`) carries one of its own.
                let (repo, commit) = peer
                    .rsplit_once('@')
                    .filter(|(repo, _)| !repo.is_empty())
                    .ok_or_else(|| format!("`{peer}` is not `<repo>@<commit>`"))?;
                if commit.len() != 40 || !commit.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                    return Err(format!(
                        "`{commit}` is not a full commit sha; a `from` pin names 40 lowercase hex characters"
                    ));
                }
                Ok(Peer {
                    repo: repo.to_string(),
                    commit: commit.to_string(),
                })
            })
            .transpose()?;
        Ok(Self {
            pack: pack.to_string(),
            range,
            from,
            raw: raw.to_string(),
        })
    }
}

/// Lowercase letters, digits and `-`, starting with a letter. A pack name is a directory name
/// and a word in a pin, so it has neither a space nor an `@`.
pub fn is_pack_name(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The pins a decision record declares, each parsed or refused on its own.
///
/// `None` when the record has no `prelude_sources` key at all. The vendor step reads that the
/// same as an empty list.
pub fn declared(proposals: &str) -> Result<Option<Vec<Result<Pin, String>>>, String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(proposals)
        .map_err(|e| format!("proposals.yml does not parse: {e}"))?;
    let Some(list) = doc.get("prelude_sources") else {
        return Ok(None);
    };
    match list {
        serde_yaml::Value::Null => Ok(Some(Vec::new())),
        serde_yaml::Value::Sequence(items) => Ok(Some(
            items
                .iter()
                .map(|item| match item.as_str() {
                    Some(s) => Pin::parse(s),
                    None => Err(format!("`{item:?}` is not a string")),
                })
                .collect(),
        )),
        other => Err(format!("prelude_sources is not a list: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn a_template_pin_names_a_pack_and_a_range() {
        let pin = Pin::parse("scholarly@^0.1").unwrap();
        assert_eq!(pin.pack, "scholarly");
        assert_eq!(pin.range.to_string(), "^0.1");
        assert!(pin.from.is_none());
    }

    #[test]
    fn a_peer_pin_splits_on_the_last_at() {
        let pin = Pin::parse(&format!("oh-leg@^0.3 from git@github.com:o/corpus@{SHA}")).unwrap();
        let peer = pin.from.unwrap();
        assert_eq!(peer.repo, "git@github.com:o/corpus");
        assert_eq!(peer.commit, SHA);
    }

    #[test]
    fn a_short_or_missing_commit_is_refused() {
        for raw in [
            "oh-leg@^0.3 from github.com/o/c@0123abc",
            "oh-leg@^0.3 from github.com/o/c",
            &format!("oh-leg@^0.3 from @{SHA}"),
            &format!("oh-leg@^0.3 from github.com/o/c@{}", SHA.to_uppercase()),
        ] {
            assert!(Pin::parse(raw).is_err(), "{raw} parsed");
        }
    }

    #[test]
    fn a_pin_with_no_range_or_a_bad_name_is_refused() {
        for raw in [
            "scholarly",
            "Scholarly@^0.1",
            "-x@^0.1",
            "scholarly@>=0.1",
            "a@^0.1 to b",
        ] {
            assert!(Pin::parse(raw).is_err(), "{raw} parsed");
        }
    }

    #[test]
    fn both_list_forms_are_read_and_an_absent_key_is_distinguished() {
        let flow = declared("prelude_sources: [scholarly@^0.1, archive@^0.1]\n")
            .unwrap()
            .unwrap();
        assert_eq!(flow.len(), 2);
        let block = declared(&format!(
            "id: proposals\nprelude_sources:\n  - scholarly@^0.1  # comment\n  - oh@^0.3 from r@{SHA}\nrationale: x\n"
        ))
        .unwrap()
        .unwrap();
        assert!(block.iter().all(Result::is_ok), "{block:?}");
        assert!(declared("prelude_sources: []\n")
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(declared("prelude_sources:\n").unwrap().unwrap().is_empty());
        assert!(declared("prelude_domains: []\n").unwrap().is_none());
    }
}
