//! The lint ratchet — which error-severity violations are already known.
//!
//! Checks report; this module decides what is acceptable. It exists because the two
//! obvious alternatives both fail, and fail quietly.
//!
//! A gate that fails on every error wedges a repository shut the moment it inherits debt
//! it cannot pay down today — so people reach for `--warn-only`, which switches off every
//! check at once and stays switched off. A gate run under `continue-on-error` reports
//! success whatever the exit code, so it goes green on every commit and the count it
//! prints is read by nobody.
//!
//! A baseline separates the two questions that single exit code was conflating. *Is the
//! corpus clean?* — no, and it says so, in a file anyone can read. *Did this commit make
//! it less clean?* — that can be gated, because inherited debt is enumerable and new debt
//! is not inherited.
//!
//! # What identifies a violation
//!
//! The pair `(check id, node)`, compared as a multiset so a node tripping the same check
//! twice is not silently reduced to once. [`Entry::detail`] is carried so the file reads
//! as a record rather than a list of identifiers, but it is deliberately **not** compared:
//! detail is prose, and rewording a sentence is not a corpus change.
//!
//! # Why a stale baseline fails
//!
//! A baselined violation that no longer occurs fails the gate exactly as an introduced one
//! does. Blocking on good news looks perverse for about as long as it takes to notice that
//! a baseline permitted to be wrong drifts, and a baseline that over-lists silently permits
//! re-introduction of whatever it over-lists. The fix is one command, and the failure
//! message names it.
//!
//! # A check nobody asked is carried, not resolved
//!
//! Some checks run only when asked for — `unrecognized-verb` under `--commits`, and the two
//! typed calculator checks only in a build with a typechecker (#1099). A run that did not
//! ask one has no news about its entries, good or bad, so [`diff`] takes the ids the roster
//! holds but this invocation left out and **carries** their entries: neither compared nor
//! reported stale. The same list keeps [`Baseline::from_checks`] from dropping them on a
//! blessing, so a baseline written by one build survives being read, and re-written, by
//! the other (#1114).
//!
//! That is distinct from an id the roster no longer has at all. Such an entry describes a
//! question no build asks any more, its entries really are stale, and they still gate.
//! The caller draws the line by passing only ids it knows and did not ask; `diff` never
//! guesses which of the two an unfamiliar id is.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::model::Check;

const HEADER: &str = "\
# Known error-severity lint violations — this corpus's inherited debt.
#
# GENERATED. Rewritten by `yidam lint --bless`. A person writes four things here, and each
# is kept: `expire_after`, `expire_record`, each entry's `note`, and the argument block
# below this header's last line. Everything else is generated.
#
# `yidam lint` fails on any error-severity violation NOT listed here, and also when
# something listed here no longer occurs — an exact baseline is the only kind that keeps
# its meaning. Neither failure means the corpus got worse on its own; both mean this file
# and the corpus disagree.
#
# Listing a violation here is not absolution. It records that the corpus was already in
# this state when the ratchet was installed, so that the next one is attributable to the
# commit that introduced it.
#
# `since` is the corpus commit at which an entry was first recorded, and it is CARRIED
# FORWARD by later blessings rather than restamped — otherwise re-blessing would forgive
# the debt again and the clock below would never run out.
#
# `expire_after` is how many corpus-touching commits an entry may survive before it stops
# forgiving and the violation gates again. Absent means never, which is where a repository
# starts. `expire_record` names the decision record that argues the number, and the record
# states the same `expire_after`: a raise changes the record too, so the argument lands in
# a file somebody reviews rather than in a commit message.
#
# `detail` is informational. Violations are identified by check id and node, so rewording
# a message does not make the baseline stale. `note` is a person's, matched to its entry by
# check and node, and never compared.
#
# ── this repository's argument: kept by --bless, written by people ──
";

/// The last line of [`HEADER`]. Every comment line after it and before the first key is the
/// repository's argument, kept verbatim by a bless (RFC-0066 §5.1).
const ARGUMENT_LINE: &str =
    "# ── this repository's argument: kept by --bless, written by people ──";

/// Every [`HEADER`] a release shipped before [`ARGUMENT_LINE`] existed, newest first. A file
/// one of these wrote has no line to split on, so its argument is whatever follows the match.
const PREVIOUS_HEADERS: &[&str] = &[
    "\
# Known error-severity lint violations — this corpus's inherited debt.
#
# GENERATED. Rewritten by `yidam lint --bless`; do not hand-edit, except for
# `expire_after`, which is this repository's own argument and is preserved.
#
# `yidam lint` fails on any error-severity violation NOT listed here, and also when
# something listed here no longer occurs — an exact baseline is the only kind that keeps
# its meaning. Neither failure means the corpus got worse on its own; both mean this file
# and the corpus disagree.
#
# Listing a violation here is not absolution. It records that the corpus was already in
# this state when the ratchet was installed, so that the next one is attributable to the
# commit that introduced it.
#
# `since` is the corpus commit at which an entry was first recorded, and it is CARRIED
# FORWARD by later blessings rather than restamped — otherwise re-blessing would forgive
# the debt again and the clock below would never run out.
#
# `expire_after` is how many corpus-touching commits an entry may survive before it stops
# forgiving and the violation gates again. Absent means never, which is where a repository
# starts. Raising it is the supported way to argue for more time — and the argument is then
# in the repository, in a diff somebody reviews, rather than in a build nobody can see.
#
# `detail` is informational. Violations are identified by check id and node, so rewording
# a message does not make the baseline stale.
",
    "\
# Known error-severity lint violations — this corpus's inherited debt.
#
# GENERATED. Rewritten wholesale by `yidam lint --bless`; do not hand-edit.
#
# `yidam lint` fails on any error-severity violation NOT listed here, and also when
# something listed here no longer occurs — an exact baseline is the only kind that keeps
# its meaning. Neither failure means the corpus got worse on its own; both mean this file
# and the corpus disagree.
#
# Listing a violation here is not absolution. It records that the corpus was already in
# this state when the ratchet was installed, so that the next one is attributable to the
# commit that introduced it.
#
# `detail` is informational. Violations are identified by check id and node, so rewording
# a message does not make the baseline stale.
",
];

/// One accepted violation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub node: String,
    /// The message the check emitted when this baseline was written. Carried so the file
    /// explains itself; never compared.
    #[serde(default)]
    pub detail: String,
    /// The corpus commit at which this entry was **first** recorded.
    ///
    /// Carried forward by every later blessing rather than restamped, which is the whole
    /// of what makes the clock mean anything: if `--bless` reset it, the expiry below
    /// would be cleared by the same command that is supposed to be constrained by it, and
    /// the baseline would be a permanent exemption wearing a deadline.
    ///
    /// Absent on an entry written before this field existed, and on a hand-written one.
    /// Such an entry never expires — it has no clock, and inventing one by stamping it
    /// with today would restart every clock in the file the first time anyone blessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// A person's words about this entry, carried across blessings the way `since` is and
    /// matched to it by check and node. Nothing compares it (RFC-0066 §5.1).
    ///
    /// A field rather than a YAML comment because a comment inside the mapping does not
    /// survive a parse and a re-serialize, and a bless is exactly that.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The committed set of violations the gate accepts, keyed by check id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// Corpus-touching commits an entry may survive before it stops forgiving.
    ///
    /// **Declared here rather than in the binary**, and for the same reason the residence
    /// threshold is declared in `.yidam/config.toml`: how long is too long is a judgement
    /// about this corpus, and a number compiled in would be one repository's answer
    /// arriving as a build failure in every other. It sits in *this* file rather than in
    /// the config because what it governs is this file — how long its entries stand.
    ///
    /// `None` — the default, and the state of every baseline until somebody argues about
    /// a number — means entries never expire. That is the ratchet as it has always
    /// behaved, and adopting the clock is opt-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_after: Option<usize>,
    /// The decision record that argues [`Self::expire_after`], by id (RFC-0066 §5.2).
    ///
    /// The record states the same number, so a raise made in this file alone disagrees
    /// with the record and fails: the argument has to land in a file somebody reviews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_record: Option<String>,
    #[serde(default)]
    pub violations: BTreeMap<String, Vec<Entry>>,
    /// The comment lines below the generated header and above the first key, verbatim.
    ///
    /// This repository's argument about its debt. Not a YAML value, so it is split off the
    /// text on [`Self::load`] and written back between the header and the body.
    #[serde(skip)]
    pub argument: String,
    /// The lines of [`Self::argument`] kept from a leading block that matched no header any
    /// release wrote. A bless lists them once, so a person can check that what it kept is
    /// theirs; after that the file carries [`ARGUMENT_LINE`] and nothing is unmatched.
    #[serde(skip)]
    pub kept_unmatched: Vec<String>,
}

/// The leading comment block of a baseline, split into the repository's argument and the
/// lines of it that no known header accounted for.
///
/// Three cases, in order (RFC-0066 §5.1):
///
/// - the block holds [`ARGUMENT_LINE`]: the argument is everything after it;
/// - the block starts with a header a release wrote ([`PREVIOUS_HEADERS`]): the argument is
///   everything after that prefix;
/// - neither: every leading comment line is kept, less any line that matches a line of a
///   known header, so a bless never deletes a person's words and never carries a lightly
///   edited old header forward as one.
fn split_argument(text: &str) -> (String, Vec<String>) {
    let lead_len: usize = text
        .split_inclusive('\n')
        .take_while(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('#')
        })
        .map(str::len)
        .sum();
    let lead = &text[..lead_len];
    if let Some(at) = lead.find(ARGUMENT_LINE) {
        let rest = &lead[at + ARGUMENT_LINE.len()..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        return (terminated(rest), Vec::new());
    }
    if let Some(h) = PREVIOUS_HEADERS.iter().find(|h| lead.starts_with(**h)) {
        return (terminated(&lead[h.len()..]), Vec::new());
    }
    let known: std::collections::HashSet<&str> = std::iter::once(HEADER)
        .chain(PREVIOUS_HEADERS.iter().copied())
        .flat_map(str::lines)
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && *l != "#")
        .collect();
    let kept: Vec<String> = lead
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty() && *l != "#" && !known.contains(l))
        .map(str::to_string)
        .collect();
    let argument = kept.iter().map(|l| format!("{l}\n")).collect();
    (argument, kept)
}

/// `text` ending in a newline, unless it is empty, so the body never starts mid-line.
fn terminated(text: &str) -> String {
    if text.is_empty() || text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

pub fn path(root: &Path) -> PathBuf {
    root.join(".yidam").join("lint-baseline.yml")
}

impl Baseline {
    /// Read the baseline, or an empty one if the file is absent.
    ///
    /// Absent is not an error: a repository that has never blessed anything has no
    /// inherited debt, and every error-severity violation is therefore new.
    pub fn load(root: &Path) -> Result<Self> {
        let p = path(root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let text =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        let mut b: Self =
            serde_yaml::from_str(&text).with_context(|| format!("parsing {}", p.display()))?;
        (b.argument, b.kept_unmatched) = split_argument(&text);
        Ok(b)
    }

    pub fn write(&self, root: &Path) -> Result<()> {
        let p = path(root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_yaml::to_string(self).context("serializing baseline")?;
        std::fs::write(&p, format!("{HEADER}{}{body}", self.argument))
            .with_context(|| format!("writing {}", p.display()))?;
        Ok(())
    }

    /// Build a baseline from the violations of a run that gate.
    ///
    /// `previous` is the baseline being replaced and `head` the corpus commit this run
    /// stands at. An entry that already existed keeps the `since` it already had; only a
    /// genuinely new one is stamped with `head`. That asymmetry is the mechanism: a
    /// blessing records new debt without forgiving old debt a second time, so the clock on
    /// an entry runs from the first time anyone accepted it and not from the last.
    ///
    /// `head` empty — a repository with no commits, or a git call that failed — stamps
    /// nothing. An entry with no clock never expires, which fails toward the ratchet's
    /// existing behaviour rather than toward a deadline nobody set.
    ///
    /// `unasked` is every check id the roster holds that this run did not ask (#1114).
    /// Entries `previous` carries under one of those ids are copied forward as they stand,
    /// clock included: a blessing from a light build has no view of the typed calculator
    /// checks, and one without `--commits` none of the log, and neither is grounds to
    /// forget what a fuller run recorded. An id in neither `checks` nor `unasked` is one
    /// the roster no longer has, and its entries are dropped as they always were.
    pub fn from_checks(checks: &[Check], previous: &Self, head: &str, unasked: &[&str]) -> Self {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for (id, entries) in &previous.violations {
            if unasked.contains(&id.as_str()) {
                violations.insert(id.clone(), entries.clone());
            }
        }
        for check in checks {
            // Per violation, not per check: residence time can escalate one finding of an
            // Info check without escalating its siblings, and the baseline records what
            // gates. Reading `check.severity` here would have quietly recorded none of
            // them — or, for a check whose base severity is Error, all of them.
            //
            // `inherited` is drained as it matches, so a node tripping one check twice
            // takes two distinct stamps rather than both taking the first one's.
            let mut inherited: Vec<&Entry> = previous
                .violations
                .get(check.id)
                .map(|entries| entries.iter().collect())
                .unwrap_or_default();
            let mut entries: Vec<Entry> = check
                .violations
                .iter()
                .filter(|v| check.gates(v))
                .map(|v| {
                    let carried = inherited
                        .iter()
                        .position(|e| e.node == v.node)
                        .map(|i| inherited.remove(i));
                    Entry {
                        node: v.node.clone(),
                        detail: v.detail.clone(),
                        since: carried
                            .and_then(|e| e.since.clone())
                            .or_else(|| (!head.is_empty()).then(|| head.to_string())),
                        note: carried.and_then(|e| e.note.clone()),
                    }
                })
                .collect();
            if entries.is_empty() {
                continue;
            }
            // Sorted so the file is stable across runs — a baseline that reorders itself
            // produces a diff on every bless and teaches people to ignore the diff.
            entries.sort_by(|a, b| a.node.cmp(&b.node));
            violations.insert(check.id.to_string(), entries);
        }
        Self {
            // The corpus's own argument about how long is long enough. Preserved across
            // blessings: it is the one thing in this generated file a human wrote.
            expire_after: previous.expire_after,
            // The rest a person wrote, kept for the same reason (RFC-0066 §5.1).
            expire_record: previous.expire_record.clone(),
            violations,
            argument: previous.argument.clone(),
            kept_unmatched: previous.kept_unmatched.clone(),
        }
    }
}

/// One baseline entry that has outlived the expiry the corpus declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expired {
    pub check: String,
    pub node: String,
    /// Corpus-touching commits it has stood for.
    pub commits: usize,
}

/// What a run and the committed baseline disagree about.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Error-severity violations present in the run and absent from the baseline.
    pub introduced: Vec<(String, String)>,
    /// Baseline entries with no corresponding violation in the run.
    pub resolved: Vec<(String, String)>,
    /// Baseline entries under a check this run did not ask, left as they stand.
    ///
    /// Not a disagreement and not a verdict: the run has no news about them either way.
    /// Listed so a report can say which entries it did not compare, and which check would
    /// have to be asked to compare them (#1114).
    pub carried: Vec<(String, String)>,
    /// Entries that have outlived the declared expiry and no longer forgive.
    ///
    /// Reported separately from [`Self::introduced`] rather than folded into it, though
    /// both fail the gate. They are different events and the fix for each is different: an
    /// introduced violation is something this change did, and an expired one is something
    /// the repository agreed to deal with and has not. Telling somebody they introduced a
    /// finding that has been sitting in their baseline for two hundred commits would be a
    /// lie, and a confusing one.
    pub expired: Vec<Expired>,
    /// What `expire_record` says about `expire_after`. [`diff`] leaves it
    /// [`Argument::Unset`]; the caller, which has the repository, reads it with
    /// [`argument`].
    pub argument: Argument,
}

impl Diff {
    pub fn is_clean(&self) -> bool {
        self.introduced.is_empty()
            && self.resolved.is_empty()
            && self.expired.is_empty()
            && !self.argument.fails()
    }
}

/// Whether the baseline's `expire_after` is argued by the record it names (RFC-0066 §5.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Argument {
    /// No `expire_after`: entries never expire, and there is nothing to argue.
    #[default]
    Unset,
    /// The named record states the same number. Honoured.
    Argued { record: String, expire_after: usize },
    /// A number and no record. Honoured, with one warning line — for one minor release,
    /// then a failure (RFC-0066 Migration).
    Unargued { expire_after: usize },
    /// The record states another number, or none. Fails.
    Disagrees {
        record: String,
        expire_after: usize,
        states: Option<usize>,
    },
    /// The record is not held. Fails, as any baseline that disagrees with the repository
    /// does.
    NotHeld { record: String },
}

impl Argument {
    pub fn fails(&self) -> bool {
        matches!(self, Self::Disagrees { .. } | Self::NotHeld { .. })
    }

    /// The line the ratchet prints about it, or none when there is nothing to say.
    pub fn line(&self) -> Option<String> {
        Some(match self {
            Self::Unset => return None,
            Self::Argued {
                record,
                expire_after,
            } => format!("lint: expire_after {expire_after} is argued in {record}"),
            Self::Unargued { expire_after } => format!(
                "lint: warning: no decision record argues expire_after {expire_after} — write \
                 one that states `expire_after: {expire_after}` and name it in the baseline's \
                 `expire_record`; a later release fails the ratchet without it"
            ),
            Self::Disagrees {
                record,
                expire_after,
                states,
            } => format!(
                "lint: the baseline's expire_after is {expire_after} and {record} states {} — \
                 a raise changes the record too",
                states.map_or_else(|| "no expire_after".to_string(), |n| n.to_string())
            ),
            Self::NotHeld { record } => {
                format!("lint: expire_record names {record}, and no decision record has that id")
            }
        })
    }
}

/// Read the record the baseline's `expire_record` names and compare its `expire_after`.
///
/// The record's key is read as an untyped value: the record is shared with every other
/// reader of decision records, and one without the key must not fail to parse.
pub fn argument(root: &Path, baseline: &Baseline) -> Argument {
    let decisions = root.join(".yidam").join("decisions");
    let held = |id: &str| crate::cmd::decisions::find_decision(&decisions, id);
    match (baseline.expire_after, baseline.expire_record.as_deref()) {
        (_, Some(record)) if held(record).is_none() => Argument::NotHeld {
            record: record.to_string(),
        },
        (None, _) => Argument::Unset,
        (Some(expire_after), None) => Argument::Unargued { expire_after },
        (Some(expire_after), Some(record)) => {
            let states = held(record)
                .and_then(|rel| std::fs::read_to_string(root.join(rel)).ok())
                .and_then(|text| stated_expiry(&text));
            if states == Some(expire_after) {
                Argument::Argued {
                    record: record.to_string(),
                    expire_after,
                }
            } else {
                Argument::Disagrees {
                    record: record.to_string(),
                    expire_after,
                    states,
                }
            }
        }
    }
}

/// The top-level `expire_after` a record states, from a YAML record or the frontmatter of a
/// Markdown one.
fn stated_expiry(text: &str) -> Option<usize> {
    let yaml = match text.strip_prefix("---\n") {
        Some(rest) => rest.split("\n---").next().unwrap_or(rest),
        None => text,
    };
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    value
        .get("expire_after")?
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
}

/// Compare a run against the baseline as a multiset of `(check id, node)`.
///
/// `corpus_commits` is every commit that touched the corpus, oldest first — the clock an
/// entry's `since` is read against. Empty disables expiry entirely, which is what happens
/// outside a git repository and in a corpus with no history: an entry whose age cannot be
/// established has not been shown to be old.
///
/// `unasked` is every check id the roster holds that this run did not ask. Its entries are
/// [`Diff::carried`] rather than [`Diff::resolved`]: absent from `checks` because nobody put
/// the question, not because the answer changed (#1114). Pass only ids the roster knows —
/// an id it no longer has is not on the list, and its entries resolve as stale.
pub fn diff(
    checks: &[Check],
    baseline: &Baseline,
    corpus_commits: &[String],
    unasked: &[&str],
) -> Diff {
    let mut out = Diff::default();

    // Which entries have run out of time. Computed up front so that the matching below can
    // simply decline to consume them: an expired entry is still *listed*, so it must not
    // also be reported as stale for the violation it does still describe.
    let expired: std::collections::HashSet<(&str, &str)> = match baseline.expire_after {
        None => Default::default(),
        Some(limit) => baseline
            .violations
            .iter()
            .flat_map(|(id, entries)| entries.iter().map(move |e| (id.as_str(), e)))
            .filter_map(|(id, e)| {
                let age = super::history::commits_since(corpus_commits, e.since.as_deref()?)?;
                (age >= limit).then_some((id, e.node.as_str()))
            })
            .collect(),
    };

    // Multiset accounting: for each check, walk the run's violations and consume a
    // matching baseline entry per occurrence. Whatever is left on either side is a
    // disagreement. A plain set would hide a node that starts tripping the same check
    // twice, which is precisely the regression the ratchet is for.
    let mut remaining: BTreeMap<&str, Vec<&Entry>> = baseline
        .violations
        .iter()
        .map(|(id, entries)| (id.as_str(), entries.iter().collect()))
        .collect();

    for check in checks {
        let pool = remaining.entry(check.id).or_default();
        for v in check.violations.iter().filter(|v| check.gates(v)) {
            let Some(i) = pool.iter().position(|e| e.node == v.node) else {
                out.introduced.push((check.id.to_string(), v.node.clone()));
                continue;
            };
            // Consumed either way: the entry describes this violation, and leaving it in
            // the pool would report it as stale on top of expired.
            let entry = pool.remove(i);
            if !expired.contains(&(check.id, v.node.as_str())) {
                continue;
            }
            let commits = entry
                .since
                .as_deref()
                .and_then(|sha| super::history::commits_since(corpus_commits, sha))
                .unwrap_or_default();
            out.expired.push(Expired {
                check: check.id.to_string(),
                node: v.node.clone(),
                commits,
            });
        }
    }

    for (id, leftover) in &remaining {
        // A check that ran consumes from its pool above, so an unasked id's pool is still
        // whole here — and it is whole because nothing looked, not because nothing matched.
        let side = if unasked.contains(id) {
            &mut out.carried
        } else {
            &mut out.resolved
        };
        for entry in leftover {
            side.push((id.to_string(), entry.node.clone()));
        }
    }

    out.introduced.sort();
    out.resolved.sort();
    out.carried.sort();
    out.expired
        .sort_by(|a, b| (&a.check, &a.node).cmp(&(&b.check, &b.node)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::model::Severity;
    use crate::cmd::lint::model::Violation;

    fn err_check(id: &'static str, nodes: &[&str]) -> Check {
        Check::new(
            id,
            "t",
            Severity::Error,
            "r",
            nodes.iter().map(|n| Violation::new(*n, "d")).collect(),
        )
    }

    fn baseline_of(pairs: &[(&str, &str)]) -> Baseline {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for (id, node) in pairs {
            violations.entry(id.to_string()).or_default().push(Entry {
                node: node.to_string(),
                detail: String::new(),
                since: None,
                note: None,
            });
        }
        Baseline {
            expire_after: None,
            violations,
            ..Default::default()
        }
    }

    #[test]
    fn a_baselined_violation_does_not_gate() {
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        let d = diff(
            &checks,
            &baseline_of(&[("dangling-edge", "a.yml")]),
            &[],
            &[],
        );
        assert!(d.is_clean(), "{d:?}");
    }

    #[test]
    fn a_new_violation_is_introduced() {
        let checks = vec![err_check("dangling-edge", &["a.yml", "b.yml"])];
        let d = diff(
            &checks,
            &baseline_of(&[("dangling-edge", "a.yml")]),
            &[],
            &[],
        );
        assert_eq!(d.introduced, vec![("dangling-edge".into(), "b.yml".into())]);
        assert!(d.resolved.is_empty());
    }

    #[test]
    fn a_fixed_violation_is_resolved_and_still_fails() {
        let checks = vec![err_check("dangling-edge", &[])];
        let d = diff(
            &checks,
            &baseline_of(&[("dangling-edge", "a.yml")]),
            &[],
            &[],
        );
        assert_eq!(d.resolved, vec![("dangling-edge".into(), "a.yml".into())]);
        assert!(!d.is_clean(), "a stale baseline must not pass");
    }

    /// A check nobody asked has no news about its entries. They are carried — listed, so the
    /// report can say what it did not compare — and neither stale nor a failure (#1114).
    ///
    /// Before this, `remaining` was seeded from every entry the file carried whichever checks
    /// ran, so a corpus that baselined a `--commits` finding failed as a *stale baseline* under
    /// every invocation without the flag.
    #[test]
    fn an_unasked_check_carries_its_entries() {
        let d = diff(
            &[],
            &baseline_of(&[("unrecognized-verb", "a.yml")]),
            &[],
            &["unrecognized-verb"],
        );
        assert_eq!(
            d.carried,
            vec![("unrecognized-verb".into(), "a.yml".into())]
        );
        assert!(d.resolved.is_empty(), "{d:?}");
        assert!(d.is_clean(), "carried is not a disagreement: {d:?}");
    }

    /// The case `resolved` exists for, kept distinct: an id the roster no longer has is not
    /// on the unasked list, and its entries are stale exactly as before.
    #[test]
    fn an_id_the_roster_no_longer_has_still_resolves() {
        let d = diff(
            &[],
            &baseline_of(&[("retired-check", "a.yml")]),
            &[],
            &["unrecognized-verb"],
        );
        assert_eq!(d.resolved, vec![("retired-check".into(), "a.yml".into())]);
        assert!(d.carried.is_empty(), "{d:?}");
        assert!(!d.is_clean());
    }

    /// A baseline written by one build and read by another — the supported pair, since
    /// `install.sh` downloads a light binary and CI runs a full one.
    ///
    /// Blessed where both checks ran; linted where one could not; blessed again there, which
    /// must not forget the entry the light run could not see; linted again where both run.
    #[test]
    fn a_baseline_written_by_one_build_survives_the_other() {
        let full = [
            err_check("dangling-edge", &["a.yml"]),
            err_check("calculator-type", &["c.yml"]),
        ];
        let light = [err_check("dangling-edge", &["a.yml"])];
        let unasked = ["calculator-type"];

        let blessed = Baseline::from_checks(&full, &Baseline::default(), "c1", &[]);
        assert!(diff(&light, &blessed, &[], &unasked).is_clean());

        let reblessed = Baseline::from_checks(&light, &blessed, "c2", &unasked);
        assert_eq!(
            reblessed.violations["calculator-type"], blessed.violations["calculator-type"],
            "a blessing from the light build carries the entry, clock and all"
        );
        assert!(diff(&full, &reblessed, &[], &[]).is_clean());

        // Without the list, a blessing drops what it did not see, and the full build
        // reports the entry as introduced — the failure the list exists to prevent.
        let forgetful = Baseline::from_checks(&light, &blessed, "c2", &[]);
        assert_eq!(
            diff(&full, &forgetful, &[], &[]).introduced,
            vec![("calculator-type".into(), "c.yml".into())]
        );
    }

    #[test]
    fn the_same_node_tripping_twice_is_not_collapsed() {
        // Multiset, not set: one baselined occurrence must not excuse two.
        let checks = vec![err_check("dangling-edge", &["a.yml", "a.yml"])];
        let d = diff(
            &checks,
            &baseline_of(&[("dangling-edge", "a.yml")]),
            &[],
            &[],
        );
        assert_eq!(
            d.introduced,
            vec![("dangling-edge".into(), "a.yml".into())],
            "the second occurrence is new"
        );
    }

    #[test]
    fn detail_is_not_part_of_identity() {
        let checks = vec![Check::new(
            "dangling-edge",
            "t",
            Severity::Error,
            "r",
            vec![Violation::new("a.yml", "completely rewritten wording")],
        )];
        let d = diff(
            &checks,
            &baseline_of(&[("dangling-edge", "a.yml")]),
            &[],
            &[],
        );
        assert!(d.is_clean(), "rewording is not a corpus change");
    }

    #[test]
    fn warn_severity_never_reaches_the_baseline() {
        let checks = vec![Check::new(
            "orphan-in",
            "t",
            Severity::Warn,
            "r",
            vec![Violation::new("a.yml", "d")],
        )];
        assert!(
            Baseline::from_checks(&checks, &Baseline::default(), "", &[])
                .violations
                .is_empty()
        );
        assert!(diff(&checks, &Baseline::default(), &[], &[]).is_clean());
    }

    #[test]
    fn blessing_a_run_makes_it_clean() {
        let checks = vec![err_check("dangling-edge", &["b.yml", "a.yml"])];
        let blessed = Baseline::from_checks(&checks, &Baseline::default(), "", &[]);
        assert!(diff(&checks, &blessed, &[], &[]).is_clean());
        // Stable ordering, so a re-bless produces no diff.
        assert_eq!(blessed.violations["dangling-edge"][0].node, "a.yml");
    }

    #[test]
    fn round_trips_through_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        Baseline::from_checks(&checks, &Baseline::default(), "", &[])
            .write(root)
            .unwrap();
        let loaded = Baseline::load(root).unwrap();
        assert!(diff(&checks, &loaded, &[], &[]).is_clean());
    }

    #[test]
    fn an_absent_baseline_loads_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(Baseline::load(tmp.path()).unwrap(), Baseline::default());
    }

    // ── the clock on an entry ─────────────────────────────────────────────────

    /// Six corpus commits, oldest first, as `history::corpus_commits` returns them.
    fn commits() -> Vec<String> {
        (1..=6).map(|i| format!("c{i}")).collect()
    }

    fn aged_baseline(expire_after: Option<usize>, since: &str) -> Baseline {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        violations.insert(
            "dangling-edge".into(),
            vec![Entry {
                node: "a.yml".into(),
                detail: String::new(),
                since: Some(since.into()),
                note: None,
            }],
        );
        Baseline {
            expire_after,
            violations,
            ..Default::default()
        }
    }

    /// The default. A baseline with no declared expiry is the ratchet as it has always
    /// behaved, however old its entries are.
    #[test]
    fn an_entry_never_expires_when_the_corpus_declared_no_number() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &aged_baseline(None, "c1"), &commits(), &[]);
        assert!(d.is_clean(), "{d:?}");
    }

    #[test]
    fn an_entry_that_outlived_the_declared_expiry_stops_forgiving() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        // Recorded at the first of six corpus commits: it has stood for six.
        let d = diff(&checks, &aged_baseline(Some(6), "c1"), &commits(), &[]);
        assert_eq!(d.expired.len(), 1, "{d:?}");
        assert_eq!(d.expired[0].commits, 6);
        assert!(!d.is_clean());
        // …and is not *also* reported as introduced or stale. It is one event.
        assert!(d.introduced.is_empty(), "{d:?}");
        assert!(d.resolved.is_empty(), "{d:?}");
    }

    #[test]
    fn an_entry_inside_the_declared_expiry_still_forgives() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &aged_baseline(Some(7), "c1"), &commits(), &[]);
        assert!(d.is_clean(), "six commits is inside seven: {d:?}");
    }

    /// A sha the corpus history does not contain — a baseline written before a rebase, or
    /// hand-edited. Unreadable is not expired: failing a build over a rewritten history
    /// would be failing over something the corpus never did.
    #[test]
    fn an_entry_whose_commit_is_gone_does_not_expire() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(
            &checks,
            &aged_baseline(Some(1), "rebased-away"),
            &commits(),
            &[],
        );
        assert!(d.is_clean(), "{d:?}");
    }

    /// An entry written before `since` existed. Stamping it with today would restart every
    /// clock in the file the first time anyone blessed.
    #[test]
    fn an_entry_with_no_clock_does_not_expire() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let mut b = baseline_of(&[("dangling-edge", "a.yml")]);
        b.expire_after = Some(1);
        assert!(diff(&checks, &b, &commits(), &[]).is_clean());
    }

    // ── what a blessing preserves ─────────────────────────────────────────────

    /// The mechanism. If `--bless` restamped `since`, the expiry would be cleared by the
    /// same command it is meant to constrain, and the baseline would be a permanent
    /// exemption wearing a deadline.
    #[test]
    fn blessing_again_carries_the_original_clock_forward() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&checks, &Baseline::default(), "c1", &[]);
        assert_eq!(
            first.violations["dangling-edge"][0].since.as_deref(),
            Some("c1")
        );

        let second = Baseline::from_checks(&checks, &first, "c6", &[]);
        assert_eq!(
            second.violations["dangling-edge"][0].since.as_deref(),
            Some("c1"),
            "re-blessing must not forgive the debt a second time"
        );
    }

    /// New debt is stamped with today even when it lands beside old debt. Otherwise a
    /// corpus could never accrue an entry with an honest start date.
    #[test]
    fn a_newly_blessed_entry_is_stamped_at_head() {
        let old = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&old, &Baseline::default(), "c1", &[]);

        let both = [err_check("dangling-edge", &["a.yml", "b.yml"])];
        let second = Baseline::from_checks(&both, &first, "c6", &[]);
        let entries = &second.violations["dangling-edge"];
        assert_eq!(entries[0].since.as_deref(), Some("c1"), "a.yml is old debt");
        assert_eq!(entries[1].since.as_deref(), Some("c6"), "b.yml is new");
    }

    /// A node tripping one check twice takes two distinct stamps rather than both taking
    /// the first one's — the same multiset care the diff takes.
    #[test]
    fn a_node_tripping_one_check_twice_keeps_one_clock_per_occurrence() {
        let once = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&once, &Baseline::default(), "c1", &[]);
        let twice = [err_check("dangling-edge", &["a.yml", "a.yml"])];
        let second = Baseline::from_checks(&twice, &first, "c6", &[]);
        let stamps: Vec<Option<&str>> = second.violations["dangling-edge"]
            .iter()
            .map(|e| e.since.as_deref())
            .collect();
        assert_eq!(stamps, vec![Some("c1"), Some("c6")]);
    }

    /// The one thing in this generated file a human wrote.
    #[test]
    fn blessing_preserves_the_corpus_own_argument_about_time() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let previous = Baseline {
            expire_after: Some(200),
            ..Default::default()
        };
        let blessed = Baseline::from_checks(&checks, &previous, "c1", &[]);
        assert_eq!(blessed.expire_after, Some(200));
    }

    /// A repository with no history stamps nothing rather than inventing a clock.
    #[test]
    fn a_repository_with_no_commits_stamps_no_clock() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let b = Baseline::from_checks(&checks, &Baseline::default(), "", &[]);
        assert_eq!(b.violations["dangling-edge"][0].since, None);
    }

    /// The file must round-trip: `expire_after` is read back, and an absent one stays
    /// absent rather than serializing as null.
    #[test]
    fn the_declared_expiry_survives_a_round_trip() {
        let mut b = aged_baseline(Some(150), "c1");
        let text = serde_yaml::to_string(&b).unwrap();
        assert!(text.contains("expire_after: 150"), "{text}");
        assert_eq!(serde_yaml::from_str::<Baseline>(&text).unwrap(), b);

        b.expire_after = None;
        let text = serde_yaml::to_string(&b).unwrap();
        assert!(
            !text.contains("expire_after"),
            "absent stays absent: {text}"
        );
    }

    fn written(b: &Baseline) -> (tempfile::TempDir, String) {
        let tmp = tempfile::TempDir::new().unwrap();
        b.write(tmp.path()).unwrap();
        let text = std::fs::read_to_string(path(tmp.path())).unwrap();
        (tmp, text)
    }

    fn rewrite(root: &Path, text: &str) {
        std::fs::write(path(root), text).unwrap();
    }

    const ARGUMENT: &str = "# We inherited 40 dangling edges from the 2024 import.\n#\n#   Indented, and kept as written.\n";

    /// RFC-0066 §5.1: a bless keeps every line of the argument block, byte for byte.
    #[test]
    fn a_bless_keeps_the_argument_block_verbatim() {
        let (tmp, text) = written(&Baseline::default());
        rewrite(
            tmp.path(),
            &text.replacen(
                &format!("{ARGUMENT_LINE}\n"),
                &format!("{ARGUMENT_LINE}\n{ARGUMENT}"),
                1,
            ),
        );
        let loaded = Baseline::load(tmp.path()).unwrap();
        assert_eq!(loaded.argument, ARGUMENT);
        assert!(loaded.kept_unmatched.is_empty());

        let checks = [err_check("dangling-edge", &["a.yml"])];
        Baseline::from_checks(&checks, &loaded, "c1", &[])
            .write(tmp.path())
            .unwrap();
        let again = std::fs::read_to_string(path(tmp.path())).unwrap();
        assert!(
            again.starts_with(&format!("{HEADER}{ARGUMENT}")),
            "header, then the argument, then the body: {again}"
        );
        assert_eq!(Baseline::load(tmp.path()).unwrap().argument, ARGUMENT);
    }

    /// A file a previous release wrote has no argument line; its header is matched whole and
    /// dropped, and what follows it is the argument.
    #[test]
    fn a_header_a_release_wrote_is_matched_and_dropped() {
        for old in PREVIOUS_HEADERS {
            let tmp = tempfile::TempDir::new().unwrap();
            std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
            rewrite(tmp.path(), &format!("{old}{ARGUMENT}violations: {{}}\n"));
            let loaded = Baseline::load(tmp.path()).unwrap();
            assert_eq!(loaded.argument, ARGUMENT, "after {old}");
            assert!(loaded.kept_unmatched.is_empty());

            loaded.write(tmp.path()).unwrap();
            let text = std::fs::read_to_string(path(tmp.path())).unwrap();
            assert_eq!(text.matches("GENERATED").count(), 1, "one header: {text}");
        }
    }

    /// A header no release wrote keeps every comment line that is not a known header line,
    /// and lists them — so a lightly edited old header is not carried forward as argument.
    #[test]
    fn an_unmatched_header_keeps_its_lines_and_lists_them() {
        let edited = PREVIOUS_HEADERS[0].replacen("# Known", "# Formerly known", 1);
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        rewrite(
            tmp.path(),
            &format!("{edited}# Ours: the import is being re-run.\nviolations: {{}}\n"),
        );
        let loaded = Baseline::load(tmp.path()).unwrap();
        assert_eq!(
            loaded.kept_unmatched,
            vec![
                "# Formerly known error-severity lint violations — this corpus's inherited debt."
                    .to_string(),
                "# Ours: the import is being re-run.".to_string(),
            ]
        );
        assert_eq!(
            loaded.argument,
            "# Formerly known error-severity lint violations — this corpus's inherited debt.\n\
             # Ours: the import is being re-run.\n"
        );
        // Written once, the file has the argument line and nothing is unmatched again.
        loaded.write(tmp.path()).unwrap();
        let reread = Baseline::load(tmp.path()).unwrap();
        assert!(reread.kept_unmatched.is_empty());
        assert_eq!(reread.argument, loaded.argument);
    }

    /// A bless carries each entry's note by check and node, and drops the note with its entry.
    #[test]
    fn a_bless_carries_each_note() {
        let mut previous = baseline_of(&[("dangling-edge", "a.yml"), ("dangling-edge", "b.yml")]);
        previous.violations.get_mut("dangling-edge").unwrap()[0].note =
            Some("Waiting on the 2024 import.".into());
        previous.violations.get_mut("dangling-edge").unwrap()[1].note = Some("Fixed.".into());
        let checks = [err_check("dangling-edge", &["a.yml", "c.yml"])];
        let b = Baseline::from_checks(&checks, &previous, "c1", &[]);
        let notes: Vec<_> = b.violations["dangling-edge"]
            .iter()
            .map(|e| (e.node.as_str(), e.note.as_deref()))
            .collect();
        assert_eq!(
            notes,
            [
                ("a.yml", Some("Waiting on the 2024 import.")),
                ("c.yml", None)
            ]
        );
        let text = serde_yaml::to_string(&b).unwrap();
        assert_eq!(
            text.matches("note:").count(),
            1,
            "absent stays absent: {text}"
        );
    }

    fn with_record(record: Option<&str>) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/decisions");
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(body) = record {
            std::fs::write(
                dir.join("baseline-expiry.yml"),
                format!("summary: Eighty commits.\nrationale: The import.\n{body}"),
            )
            .unwrap();
        }
        tmp
    }

    fn expiring(after: Option<usize>, record: Option<&str>) -> Baseline {
        Baseline {
            expire_after: after,
            expire_record: record.map(str::to_string),
            ..Default::default()
        }
    }

    /// RFC-0066 §5.2, one test per row of the table.
    #[test]
    fn the_expiry_argument_reads_each_state() {
        let held = with_record(Some("expire_after: 80\n"));
        let root = held.path();
        assert_eq!(argument(root, &expiring(None, None)), Argument::Unset);
        assert_eq!(
            argument(root, &expiring(Some(80), Some("baseline-expiry"))),
            Argument::Argued {
                record: "baseline-expiry".into(),
                expire_after: 80
            }
        );
        assert_eq!(
            argument(root, &expiring(Some(120), Some("baseline-expiry"))),
            Argument::Disagrees {
                record: "baseline-expiry".into(),
                expire_after: 120,
                states: Some(80)
            }
        );
        assert_eq!(
            argument(root, &expiring(Some(80), None)),
            Argument::Unargued { expire_after: 80 }
        );
        assert_eq!(
            argument(root, &expiring(Some(80), Some("gone"))),
            Argument::NotHeld {
                record: "gone".into()
            }
        );

        let silent = with_record(Some(""));
        assert_eq!(
            argument(silent.path(), &expiring(Some(80), Some("baseline-expiry"))),
            Argument::Disagrees {
                record: "baseline-expiry".into(),
                expire_after: 80,
                states: None
            },
            "a record that states no number does not argue one"
        );
    }

    /// Only the two disagreements fail the ratchet; a missing record is a warning for now.
    #[test]
    fn only_a_disagreeing_record_fails_the_ratchet() {
        let clean = |argument| {
            Diff {
                argument,
                ..Default::default()
            }
            .is_clean()
        };
        assert!(clean(Argument::Unset));
        assert!(clean(Argument::Unargued { expire_after: 80 }));
        assert!(clean(Argument::Argued {
            record: "r".into(),
            expire_after: 80
        }));
        assert!(!clean(Argument::NotHeld { record: "r".into() }));
        assert!(!clean(Argument::Disagrees {
            record: "r".into(),
            expire_after: 80,
            states: None
        }));
        assert!(Argument::Unargued { expire_after: 80 }
            .line()
            .unwrap()
            .contains("warning"));
    }

    /// A Markdown record states the number in its frontmatter.
    #[test]
    fn a_markdown_record_states_its_number_in_frontmatter() {
        assert_eq!(
            stated_expiry("---\nid: x\nexpire_after: 80\n---\n\n# Why\n"),
            Some(80)
        );
        assert_eq!(stated_expiry("summary: s\nexpire_after: 40\n"), Some(40));
        assert_eq!(stated_expiry("summary: s\n"), None);
    }
}
