//! The code-artifact pin (RFC-0051 §2–3): a `file` location that names the `commit` it was
//! read at, the `repo` that commit is in, and optionally a `symbol` inside it.
//!
//! **One process per repository.** Every pin in a repository goes to a single
//! `git cat-file --batch-command`, two lines per pin, so 244 pins cost one git process and
//! not 244. [`resolve`] is the resolver; the `due` clock and the coverage report reuse it, so
//! it hands back each pin's object id and never stores one.
//!
//! **Unchecked is not unresolved.** A shallow clone that cannot see a commit has not shown the
//! commit is missing, and a `url`/`github` dependency has no checkout to look in. Both are
//! `catalog-pin-unchecked`, apart from the error, for the reason the superseded clock counts
//! `unread` apart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::model::{Check, Severity, Violation};
use crate::corpus::Source;
use crate::parse::CatalogLocation;

pub(crate) const UNRESOLVED: &str = "catalog-pin-unresolved";
pub(crate) const AMBIGUOUS: &str = "catalog-pin-ambiguous";
pub(crate) const ABBREVIATED: &str = "catalog-pin-abbreviated";
pub(crate) const SYMBOL_MISSING: &str = "catalog-pin-symbol-missing";
pub(crate) const UNCHECKED: &str = "catalog-pin-unchecked";

// ── the resolver ────────────────────────────────────────────────────────────────

/// One pin to look up: `path` at `commit`, with the file's text when `contents` is set.
pub(crate) struct Query {
    pub commit: String,
    pub path: String,
    pub contents: bool,
}

/// What git said about one object name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lookup {
    Found { oid: String, kind: String },
    Missing,
    Ambiguous,
}

/// One [`Query`] answered: the commit peeled, the path at it, and the text when asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub commit: Lookup,
    pub object: Lookup,
    pub text: Option<String>,
}

/// Look every query up in the repository containing `dir`, with one git process.
///
/// `None` when git could not run there at all (no git, not a repository), which a caller
/// reports as unchecked rather than as every pin missing. A path is read from the
/// repository's top level, whichever directory inside it `dir` is.
pub(crate) fn resolve(dir: &Path, queries: &[Query]) -> Option<Vec<Answer>> {
    if queries.is_empty() {
        return Some(Vec::new());
    }
    let input: String = queries
        .iter()
        .map(|q| {
            let verb = if q.contents { "contents" } else { "info" };
            format!("info {c}^{{commit}}\n{verb} {c}:{}\n", q.path, c = q.commit)
        })
        .collect();
    let out = crate::git::Git::new(dir)
        .args(["cat-file", "--batch-command"])
        .stdin(input)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&out.stdout, queries)
}

/// Read `--batch-command` output back into one [`Answer`] per query, in order.
///
/// Each line is `<oid> <type> <size>`, `<name> missing` or `<name> ambiguous`; a found
/// `contents` request is followed by `<size>` bytes and a newline.
fn parse(mut out: &[u8], queries: &[Query]) -> Option<Vec<Answer>> {
    let mut answers = Vec::with_capacity(queries.len());
    for q in queries {
        let (commit, _) = header(&mut out)?;
        let (object, size) = header(&mut out)?;
        let mut text = None;
        if q.contents && matches!(object, Lookup::Found { .. }) {
            let body = out.get(..size)?;
            text = Some(String::from_utf8_lossy(body).into_owned());
            out = out.get(size + 1..)?;
        }
        answers.push(Answer {
            commit,
            object,
            text,
        });
    }
    Some(answers)
}

/// Read one `--batch-command` header line off the front of `out`, with the size of the body
/// that follows it when the request was `contents`.
fn header(out: &mut &[u8]) -> Option<(Lookup, usize)> {
    let end = out.iter().position(|b| *b == b'\n')?;
    let line = String::from_utf8_lossy(&out[..end]).into_owned();
    *out = &out[end + 1..];
    if line.ends_with(" missing") {
        return Some((Lookup::Missing, 0));
    }
    if line.ends_with(" ambiguous") {
        return Some((Lookup::Ambiguous, 0));
    }
    let mut parts = line.split(' ');
    let oid = parts.next()?.to_string();
    let kind = parts.next()?.to_string();
    let size = parts.next()?.parse().ok()?;
    Some((Lookup::Found { oid, kind }, size))
}

/// Look every commit up in the repository containing `dir`, with one git process and no
/// path: the lookup `prose-commit-unpinned` needs, where prose names a commit and no file.
///
/// `None` when git could not run there at all, as for [`resolve`].
pub(crate) fn resolve_commits(dir: &Path, commits: &[String]) -> Option<Vec<Lookup>> {
    if commits.is_empty() {
        return Some(Vec::new());
    }
    let input: String = commits
        .iter()
        .map(|c| format!("info {c}^{{commit}}\n"))
        .collect();
    let out = crate::git::Git::new(dir)
        .args(["cat-file", "--batch-command"])
        .stdin(input)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut rest = out.stdout.as_slice();
    commits
        .iter()
        .map(|_| header(&mut rest).map(|(lookup, _)| lookup))
        .collect()
}

// ── the checks ──────────────────────────────────────────────────────────────────

/// Where a pin's repository is, or why it cannot be looked in.
pub(crate) enum Place {
    Dir(PathBuf),
    Unchecked(String),
    Unresolved(String),
}

/// A pin that passed the shape checks, waiting for its repository's answer.
struct Pending<'a> {
    rel: &'a str,
    at: String,
    commit: String,
    path: String,
    symbol: Option<String>,
}

/// The five pin checks, in roster order: unresolved, ambiguous, abbreviated, symbol-missing,
/// unchecked.
pub(crate) fn checks(root: &Path, sources: &[Source]) -> [Check; 5] {
    let mut found: [Vec<Violation>; 5] = Default::default();
    let [unresolved, ambiguous, abbreviated, symbol_missing, unchecked] = &mut found;

    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    let mut by_repo: BTreeMap<Option<String>, Vec<Pending>> = BTreeMap::new();
    for s in sources {
        for (i, loc) in s.locations.iter().enumerate() {
            if !loc.is_pinned() {
                continue;
            }
            let at = format!("location[{i}]");
            match shape(loc) {
                Err(why) => unresolved.push(Violation::new(&s.rel, format!("{at}: {why}"))),
                Ok((commit, path)) => by_repo.entry(loc.repo.clone()).or_default().push(Pending {
                    rel: &s.rel,
                    at,
                    commit,
                    path,
                    symbol: loc.symbol.clone(),
                }),
            }
        }
    }

    for (repo, pins) in by_repo {
        let named = repo
            .as_deref()
            .map_or("this repository".to_string(), |r| format!("`{r}`"));
        let dir = match place(root, repo.as_deref(), &config) {
            Place::Dir(d) => d,
            Place::Unchecked(why) => {
                for p in &pins {
                    unchecked.push(Violation::new(p.rel, format!("{}: {why}", p.at)));
                }
                continue;
            }
            Place::Unresolved(why) => {
                for p in &pins {
                    unresolved.push(Violation::new(p.rel, format!("{}: {why}", p.at)));
                }
                continue;
            }
        };
        let queries: Vec<Query> = pins
            .iter()
            .map(|p| Query {
                commit: p.commit.clone(),
                path: p.path.clone(),
                contents: p.symbol.is_some(),
            })
            .collect();
        let Some(answers) = resolve(&dir, &queries) else {
            for p in &pins {
                unchecked.push(Violation::new(
                    p.rel,
                    format!("{}: git could not read {named} at {}", p.at, dir.display()),
                ));
            }
            continue;
        };
        let mut shallow = None;
        for (p, a) in pins.iter().zip(answers) {
            let say = |detail: String| Violation::new(p.rel, format!("{}: {detail}", p.at));
            let full = match a.commit {
                Lookup::Ambiguous => {
                    ambiguous.push(say(format!(
                        "commit `{}` matches more than one commit in {named}; write all 40 digits",
                        p.commit
                    )));
                    continue;
                }
                Lookup::Missing => {
                    if *shallow.get_or_insert_with(|| crate::git::is_shallow(&dir)) {
                        unchecked.push(say(format!(
                            "commit `{}` is not in this shallow clone of {named}; fetch more \
                             history to check it",
                            p.commit
                        )));
                    } else {
                        unresolved.push(say(format!(
                            "commit `{}` is not a commit in {named}",
                            p.commit
                        )));
                    }
                    continue;
                }
                Lookup::Found { oid, .. } => oid,
            };
            if p.commit.len() < 40 {
                abbreviated.push(say(format!(
                    "commit `{}` is abbreviated; write it in full: `commit: {full}`",
                    p.commit
                )));
            }
            let kind = match a.object {
                Lookup::Found { kind, .. } => kind,
                _ => {
                    unresolved.push(say(format!(
                        "`{}` does not exist at commit `{}` in {named}",
                        p.path, p.commit
                    )));
                    continue;
                }
            };
            let Some(symbol) = &p.symbol else { continue };
            let word = last_segment(symbol);
            if kind != "blob" {
                symbol_missing.push(say(format!(
                    "`symbol: {symbol}` needs a file, and `{}` is a {kind} at that commit",
                    p.path
                )));
            } else if !a.text.as_deref().is_some_and(|t| has_word(t, word)) {
                symbol_missing.push(say(format!(
                    "`{word}` (from `symbol: {symbol}`) does not occur as a word in `{}` at \
                     commit `{}`",
                    p.path, p.commit
                )));
            }
        }
    }

    let [unresolved, ambiguous, abbreviated, symbol_missing, unchecked] = found;
    [
        Check::new(
            UNRESOLVED,
            "Pinned read names a commit, repository or path that is not there",
            Severity::Error,
            "A `file` location with a `commit` is a read of that file at that commit, in the \
             repository `repo` names, or in this repository when `repo` is absent. A commit \
             the repository does not have, a `repo` that `.yidam/tonpa.toml` does not \
             declare, or a path that does not exist at the commit is a read of nothing, and \
             every claim resting on it rests on nothing. The pin's fields belong to a `file` \
             location only, `repo` and `symbol` need a `commit`, and the path is read from \
             the repository's top level, so it cannot be absolute or climb out with `..`.",
            unresolved,
        ),
        Check::new(
            AMBIGUOUS,
            "Pinned commit is an abbreviation that matches more than one commit",
            Severity::Error,
            "An abbreviated commit names a read only while its prefix is unique. Once two \
             commits share it, the pin names neither, and which one a reader lands on depends \
             on the clone. Write the commit in full.",
            ambiguous,
        ),
        Check::new(
            ABBREVIATED,
            "Pinned commit is abbreviated",
            Severity::Info,
            "A prefix unique today can stop being unique as history grows, and the pin then \
             names nothing. An abbreviation of at least 7 digits still resolves; the finding \
             gives the full 40-digit commit to write in its place.",
            abbreviated,
        ),
        Check::new(
            SYMBOL_MISSING,
            "Pinned symbol does not occur in the file at the commit",
            Severity::Warn,
            "`symbol` says which part of the file was read. Its last segment, split on `::` \
             and `.`, should occur as a whole word in the file at `commit`; when it does not, \
             the symbol was renamed before the read, misspelled, or belongs to another file. \
             The check is a presence check and needs no parser.",
            symbol_missing,
        ),
        Check::new(
            UNCHECKED,
            "Pinned read cannot be checked in this clone",
            Severity::Info,
            "A shallow clone that cannot see a commit has not shown that the commit is \
             missing, and a `url` or `github` dependency has no checkout here to look in. \
             These pins are reported apart from the error, so a CI checkout with shallow \
             history does not fail on reads that a full clone would resolve.",
            unchecked,
        ),
    ]
}

// ── whether a read is still current (RFC-0051 §4) ──────────────────────────────

/// One pinned read whose file at the reference revision is not the object it was read at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Changed {
    /// Repo-relative path of the catalog entry.
    pub entry: String,
    /// The dependency the pin is in, or `None` for this repository.
    pub repo: Option<String>,
    pub path: String,
    /// The commit the read names, as written.
    pub commit: String,
    /// The reference revision's commit, in full.
    pub now: String,
    /// Commits between the two that touched `path`, when git could count them.
    pub commits: Option<u64>,
    /// The file is absent at the reference revision.
    pub gone: bool,
    /// The pinned symbol and what it did, when the pin names one and its file is still there.
    pub symbol: Option<(String, super::symbols::Grain)>,
}

impl Changed {
    /// The catalog clock's line: what changed and since when, never how long ago.
    pub(crate) fn line(&self) -> String {
        let stem = Path::new(&self.entry)
            .file_stem()
            .map_or_else(|| self.entry.clone(), |s| s.to_string_lossy().into_owned());
        let path = match &self.repo {
            Some(r) => format!("{r}:{}", self.path),
            None => self.path.clone(),
        };
        let since = self.commit.get(..7).unwrap_or(&self.commit);
        let now = self.now.get(..8).unwrap_or(&self.now);
        let commits = |n: u64| format!("{n} commit{}", if n == 1 { "" } else { "s" });
        let what = match (self.gone, &self.symbol, self.commits) {
            (true, _, _) => format!("{path} is gone"),
            (false, Some((symbol, grain)), Some(n)) => {
                format!("{path} `{symbol}`: {}, in {}", grain.said(), commits(n))
            }
            (false, Some((symbol, grain)), None) => format!("{path} `{symbol}`: {}", grain.said()),
            (false, None, Some(n)) => format!("{path} changed in {}", commits(n)),
            (false, None, None) => format!("{path} changed"),
        };
        format!("{stem} — {what} since {since} (now {now})")
    }
}

/// Every pinned read in `sources`, measured against its reference revision.
#[derive(Debug, Default)]
pub(crate) struct Staleness {
    /// Pins whose object was compared.
    pub pinned: usize,
    pub changed: Vec<Changed>,
    /// Symbol pins whose file changed and whose symbol did not, so they are not due.
    pub unchanged: usize,
    /// Pins that could not be compared: a bad shape, a missing commit or path, a shallow
    /// clone, or a dependency with no checkout. Lint names each one; this only counts them.
    pub unread: usize,
}

/// The revision a pin in `repo` is read against: `HEAD` for this repository, the tonpa `rev`
/// for a dependency, and that checkout's `HEAD` when the dependency pins no `rev`.
fn reference(repo: Option<&str>, config: &crate::deps::TonpaConfig) -> String {
    repo.and_then(|r| config.dependencies.get(r))
        .and_then(|d| d.rev.as_deref())
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .unwrap_or("HEAD")
        .to_string()
}

/// One well-shaped pin, waiting for its repository's batch.
struct Pin<'a> {
    rel: &'a str,
    commit: String,
    path: String,
    symbol: Option<&'a str>,
}

/// Compare every pinned read with its file at the reference revision, with one git process
/// per repository and one more per changed pin to count the commits. A pin naming a `symbol`
/// is then measured at the grain [`super::symbols::compare`] decides.
pub(crate) fn staleness(root: &Path, sources: &[Source]) -> Staleness {
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    let mut out = Staleness::default();
    let mut by_repo: BTreeMap<Option<String>, Vec<Pin<'_>>> = BTreeMap::new();
    for s in sources {
        for loc in s.locations.iter().filter(|l| l.is_pinned()) {
            match shape(loc) {
                Ok((commit, path)) => by_repo.entry(loc.repo.clone()).or_default().push(Pin {
                    rel: &s.rel,
                    commit,
                    path,
                    symbol: loc.symbol.as_deref(),
                }),
                Err(_) => out.unread += 1,
            }
        }
    }
    for (repo, pins) in by_repo {
        let Place::Dir(dir) = place(root, repo.as_deref(), &config) else {
            out.unread += pins.len();
            continue;
        };
        let at = reference(repo.as_deref(), &config);
        let queries: Vec<Query> = pins
            .iter()
            .flat_map(|p| {
                [p.commit.clone(), at.clone()].map(|commit| Query {
                    commit,
                    path: p.path.clone(),
                    contents: p.symbol.is_some(),
                })
            })
            .collect();
        let Some(answers) = resolve(&dir, &queries) else {
            out.unread += pins.len();
            continue;
        };
        for (pin, pair) in pins.iter().zip(answers.chunks(2)) {
            let Pin {
                rel,
                commit,
                path,
                symbol,
            } = pin;
            let [read, current] = pair else {
                out.unread += 1;
                continue;
            };
            let (Lookup::Found { oid: was, .. }, Lookup::Found { oid: now, .. }) =
                (&read.object, &current.commit)
            else {
                out.unread += 1;
                continue;
            };
            out.pinned += 1;
            let gone = !matches!(current.object, Lookup::Found { .. });
            if let Lookup::Found { oid, .. } = &current.object {
                if oid == was {
                    continue;
                }
            }
            let symbol = match symbol {
                Some(symbol) if !gone => {
                    let grain = super::symbols::compare(
                        path,
                        symbol,
                        read.text.as_deref(),
                        current.text.as_deref(),
                    );
                    if !grain.due() {
                        out.unchanged += 1;
                        continue;
                    }
                    Some((symbol.to_string(), grain))
                }
                _ => None,
            };
            let commits = crate::git::Git::new(&dir)
                .args(["rev-list", "--count"])
                .rev(format!("{commit}..{now}"))
                .paths([path])
                .try_run()
                .and_then(|n| n.trim().parse().ok());
            out.changed.push(Changed {
                entry: rel.to_string(),
                repo: repo.clone(),
                path: path.clone(),
                commit: commit.clone(),
                now: now.clone(),
                commits,
                gone,
                symbol,
            });
        }
    }
    out.changed
        .sort_by(|a, b| (&a.entry, &a.repo, &a.path).cmp(&(&b.entry, &b.repo, &b.path)));
    out
}

/// The commit and the normalized path of a pin, or why its shape is wrong.
pub(crate) fn shape(loc: &CatalogLocation) -> Result<(String, String), String> {
    let kind = loc.kind.as_deref().unwrap_or("");
    if kind != "file" {
        return Err(format!(
            "`commit`, `repo` and `symbol` belong to a `file` location, and this one is \
             `{kind}`"
        ));
    }
    let Some(commit) = loc.commit.as_deref().map(str::trim) else {
        return Err("`repo` or `symbol` without a `commit` pins nothing; add the commit".into());
    };
    if !(7..=40).contains(&commit.len())
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "`commit: {commit}` is not 7 to 40 lowercase hex digits"
        ));
    }
    for (field, v) in [("repo", &loc.repo), ("symbol", &loc.symbol)] {
        if v.as_deref().is_some_and(|v| v.trim().is_empty()) {
            return Err(format!("`{field}` is empty"));
        }
    }
    let raw = loc.value.as_deref().unwrap_or("").trim();
    let path = raw.strip_prefix("./").unwrap_or(raw).trim_end_matches('/');
    if path.is_empty() {
        return Err("a pinned `file` location needs a `value`".into());
    }
    if path.starts_with('/') || path.split('/').any(|seg| seg == "..") || path.contains('\n') {
        return Err(format!(
            "`value: {raw}` must be a path from the repository's top level, not absolute and \
             without `..`"
        ));
    }
    Ok((commit.to_string(), path.to_string()))
}

/// Where the repository a pin names lives.
pub(crate) fn place(root: &Path, repo: Option<&str>, config: &crate::deps::TonpaConfig) -> Place {
    let Some(name) = repo else {
        return Place::Dir(root.to_path_buf());
    };
    let Some(dep) = config.dependencies.get(name) else {
        let declared = if config.dependencies.is_empty() {
            "`.yidam/tonpa.toml` declares no dependencies".to_string()
        } else {
            format!(
                "declared: {}",
                config
                    .dependencies
                    .keys()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Place::Unresolved(format!("`repo: {name}` is not a dependency ({declared})"));
    };
    match &dep.path {
        Some(p) => Place::Dir(root.join(p)),
        None => Place::Unchecked(format!(
            "`repo: {name}` is a `url`/`github` dependency with no local checkout to look in"
        )),
    }
}

/// The part of a symbol path a file has to contain: the last `::`- or `.`-separated segment,
/// without an argument list.
fn last_segment(symbol: &str) -> &str {
    let s = symbol.trim();
    let s = s.split('(').next().unwrap_or(s);
    let s = s.rsplit("::").next().unwrap_or(s);
    s.rsplit('.').next().unwrap_or(s).trim()
}

/// Whether `word` occurs in `text` with no identifier character on either side.
fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git, git_out, init, write};

    fn pinned(value: &str, commit: Option<&str>, symbol: Option<&str>) -> CatalogLocation {
        CatalogLocation {
            kind: Some("file".into()),
            value: Some(value.into()),
            commit: commit.map(String::from),
            symbol: symbol.map(String::from),
            ..Default::default()
        }
    }

    fn source(locations: Vec<CatalogLocation>) -> Source {
        Source {
            rel: ".yidam/catalog/s.md".into(),
            path: PathBuf::from("/repo/.yidam/catalog/s.md"),
            obtained: true,
            r#type: None,
            used_by: None,
            locations,
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
            text: String::new(),
            malformed: None,
        }
    }

    /// A repository with one commit holding `src/lib.rs`, and that commit's full id.
    fn fixture(dir: &Path) -> String {
        init(dir);
        write(
            dir,
            "src/lib.rs",
            "pub struct MicroDefaults;\nimpl MicroDefaults { fn load() {} }\n",
        );
        write(dir, ".yidam/README.md", "corpus\n");
        commit(dir, "genesis");
        git_out(dir, &["rev-parse", "HEAD"])
    }

    /// Which checks fired, by index, and how often.
    fn fired(root: &Path, locs: Vec<CatalogLocation>) -> [usize; 5] {
        checks(root, &[source(locs)]).map(|c| c.violations.len())
    }

    #[test]
    fn a_pin_that_resolves_raises_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let head = fixture(tmp.path());
        let locs = vec![
            pinned("src/lib.rs", Some(&head), Some("game::MicroDefaults::load")),
            pinned("./src/", Some(&head), None),
        ];
        assert_eq!(fired(tmp.path(), locs), [0; 5]);
    }

    #[test]
    fn each_failure_raises_exactly_its_own_check() {
        let tmp = tempfile::tempdir().unwrap();
        let head = fixture(tmp.path());
        let missing = "0123456789abcdef0123456789abcdef01234567";
        let cases: [(CatalogLocation, [usize; 5]); 6] = [
            (pinned("src/lib.rs", Some(missing), None), [1, 0, 0, 0, 0]),
            (pinned("src/nope.rs", Some(&head), None), [1, 0, 0, 0, 0]),
            (
                pinned("src/lib.rs", Some(&head[..7]), None),
                [0, 0, 1, 0, 0],
            ),
            (
                pinned("src/lib.rs", Some(&head), Some("a::Gone")),
                [0, 0, 0, 1, 0],
            ),
            (pinned("src", Some(&head), Some("load")), [0, 0, 0, 1, 0]),
            (
                CatalogLocation {
                    repo: Some("remote".into()),
                    ..pinned("x.rs", Some(&head), None)
                },
                [0, 0, 0, 0, 1],
            ),
        ];
        write(
            tmp.path(),
            ".yidam/tonpa.toml",
            "[dependencies.remote]\ngithub = \"o/r\"\n",
        );
        for (loc, want) in cases {
            let label = format!("{:?} {:?}", loc.value, loc.symbol);
            assert_eq!(fired(tmp.path(), vec![loc]), want, "{label}");
        }
    }

    #[test]
    fn the_abbreviation_finding_gives_the_full_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let head = fixture(tmp.path());
        let c = &checks(
            tmp.path(),
            &[source(vec![pinned("src/lib.rs", Some(&head[..9]), None)])],
        )[2];
        assert!(
            c.violations[0].detail.contains(&head),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_malformed_pin_is_unresolved_without_asking_git() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = [
            CatalogLocation {
                kind: Some("url".into()),
                ..pinned("https://x", Some("abcdef0"), None)
            },
            pinned("src/lib.rs", None, Some("x")),
            pinned("src/lib.rs", Some("ABCDEF0"), None),
            pinned("src/lib.rs", Some("abc"), None),
            pinned("/etc/passwd", Some("abcdef0"), None),
            pinned("../sibling/x", Some("abcdef0"), None),
        ];
        for loc in bad {
            let label = format!("{:?} {:?}", loc.value, loc.commit);
            assert_eq!(fired(tmp.path(), vec![loc]), [1, 0, 0, 0, 0], "{label}");
        }
    }

    #[test]
    fn an_undeclared_repo_is_unresolved_and_names_the_declared() {
        let tmp = tempfile::tempdir().unwrap();
        let head = fixture(tmp.path());
        write(
            tmp.path(),
            ".yidam/tonpa.toml",
            "[dependencies.sib]\npath = \"../sib\"\n",
        );
        let loc = CatalogLocation {
            repo: Some("other".into()),
            ..pinned("x.rs", Some(&head), None)
        };
        let c = &checks(tmp.path(), &[source(vec![loc])])[0];
        assert!(
            c.violations[0].detail.contains("`sib`"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_path_dependency_is_read_in_its_own_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let here = tmp.path().join("here");
        let sib = tmp.path().join("sib");
        std::fs::create_dir_all(&here).unwrap();
        std::fs::create_dir_all(&sib).unwrap();
        fixture(&here);
        let theirs = fixture(&sib);
        write(
            &here,
            ".yidam/tonpa.toml",
            "[dependencies.sib]\npath = \"../sib\"\n",
        );
        let loc = CatalogLocation {
            repo: Some("sib".into()),
            ..pinned("src/lib.rs", Some(&theirs), Some("MicroDefaults"))
        };
        assert_eq!(fired(&here, vec![loc]), [0; 5]);
    }

    #[test]
    fn outside_a_repository_every_pin_is_unchecked() {
        let tmp = tempfile::tempdir().unwrap();
        let loc = pinned("src/lib.rs", Some("abcdef0"), None);
        assert_eq!(fired(tmp.path(), vec![loc]), [0, 0, 0, 0, 1]);
    }

    #[test]
    fn a_shallow_clone_without_the_commit_is_unchecked() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin");
        std::fs::create_dir_all(&origin).unwrap();
        let first = fixture(&origin);
        write(&origin, "later.txt", "x\n");
        commit(&origin, "second");
        let clone = tmp.path().join("clone");
        let url = format!("file://{}", origin.display());
        git(
            tmp.path(),
            &["clone", "-q", "--depth", "1", &url, clone.to_str().unwrap()],
        );
        let loc = pinned("src/lib.rs", Some(&first), None);
        assert_eq!(fired(&clone, vec![loc]), [0, 0, 0, 0, 1]);
    }

    #[test]
    fn many_pins_are_one_batch() {
        let tmp = tempfile::tempdir().unwrap();
        let head = fixture(tmp.path());
        let queries: Vec<Query> = (0..244)
            .map(|i| Query {
                commit: head.clone(),
                path: if i % 2 == 0 { "src/lib.rs" } else { "src" }.into(),
                contents: i % 3 == 0,
            })
            .collect();
        let answers = resolve(tmp.path(), &queries).expect("git runs");
        assert_eq!(answers.len(), 244);
        assert!(answers
            .iter()
            .all(|a| matches!(a.object, Lookup::Found { .. })));
        assert!(answers[0]
            .text
            .as_deref()
            .unwrap()
            .contains("MicroDefaults"));
    }

    #[test]
    fn an_ambiguous_name_parses_as_ambiguous() {
        let q = [Query {
            commit: "abcdef0".into(),
            path: "x".into(),
            contents: true,
        }];
        let out = b"abcdef0^{commit} ambiguous\nabcdef0:x ambiguous\n";
        assert_eq!(
            parse(out, &q).unwrap(),
            vec![Answer {
                commit: Lookup::Ambiguous,
                object: Lookup::Ambiguous,
                text: None
            }]
        );
    }

    #[test]
    fn a_symbol_is_matched_by_its_last_segment_as_a_word() {
        assert_eq!(last_segment("game::MicroDefaults::load"), "load");
        assert_eq!(last_segment("pkg.mod.Klass.method(x, y)"), "method");
        assert!(has_word("fn load() {}", "load"));
        assert!(!has_word("fn loader() {}", "load"));
        assert!(!has_word("fn reload() {}", "load"));
    }
}
