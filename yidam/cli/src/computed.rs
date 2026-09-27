//! `.yidam/computed/**` — what a calculator worked out, read back.
//!
//! A calculator commits its answer to a file rather than onto the nodes it is about, and
//! `guidelines/directories.md` gives the reason: *"a property on a node reads as a fact about
//! the subject when it is a fact about a calculation."* That decision is right and it left the
//! file with nowhere to go. Every occurrence of `computed` across the CLI was the English word
//! in a doc comment until this module — no path helper, no reader, no retrieval surface — so a
//! calculator could compute a derived quantity under the orchestrator's guarantees and no
//! command in the toolkit could see it (#1028).
//!
//! # One question has to have one answer: which node does this row describe
//!
//! A file of numbers reaches a retrieval surface only if something can say what each number is
//! *about*, and nothing constrained that. So a row is keyed by `node:`, holding a reference in
//! RFC-0032's grammar — `gage/canyon-outlet`, or the full `yidam://<corpus>/node/<class>/<name>`
//! — and it is read by [`yidam_core::uri::parse_reference`] rather than by a rule invented here.
//! That module exists because a corpus node had eleven string forms across this repository
//! before it, and a twelfth written for this directory is the defect it was filed to end.
//!
//! Two consequences of using the grammar rather than a path. A reference naming another corpus
//! is refused: a signal lands on a node in *this* corpus's embedding, and a foreign name has no
//! node here to land on. And a reference whose node this corpus does not hold is reported rather
//! than dropped, because a table keyed on nodes that are not there is the one failure a reader
//! cannot distinguish from an empty answer.
//!
//! # A signal name is corpus-wide, and a collision is refused rather than resolved
//!
//! Signals from every computed file merge onto one node, so two files declaring `tier` would be
//! one key with two meanings. The alternative to refusing was a prefix — `travel-tier.tier` —
//! and it was declined: it invents a second name for every signal, it is the name nobody wrote
//! in their calculator, and the collision it hides is a real disagreement between two
//! capabilities about what a word means. Refusing says which two files and which name, which is
//! recoverable; a prefix makes both readings permanent.
//!
//! [`RESERVED`] is the same rule against the names a record already carries.
//!
//! # A file with no table is still a computed artifact
//!
//! `signals:` is optional. A calculator whose output is an operator's report — a partition, a
//! narrative, a summary — is not obliged to key it by node, and a reader that demanded a table
//! would be telling corpora what their calculators are for. Such a file is found, attributed to
//! the capability that declared it, and carries no signals. What is *not* optional is
//! `format_version` on a file that does have a table: `receipt.rs` states the argument for
//! writing one from the first commit rather than adding it later, and a signal table is the
//! same kind of committed record.
//!
//! # What this module does not decide
//!
//! Whether a signal becomes an index column, and under which type. That is #1029's question and
//! it is a schema decision about the index rather than about the corpus; this module reads what
//! was committed and says which node it is about. [`crate::cmd::embed`] is the first consumer.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::cmd::run::manifest::{Manifest, MANIFEST};
use crate::kuten::glob_covers;
use crate::paths::yidam_computed_dir;

/// The signal table format this binary reads.
///
/// Bumped when a consumer that understood the previous version would mis-read this one. A file
/// declaring a version this binary does not know is reported and not read, rather than read as
/// though the fields meant what they used to — see [`Signals::load`].
pub const FORMAT_VERSION: u32 = 1;

/// The key a row is addressed by. Not a signal name.
const NODE_KEY: &str = "node";

/// Names a signal may not take, because a record already carries them.
///
/// The row key above, the fields [`crate::cmd::embed::EmbedRecord`] writes, and the metadata
/// keys a remote vector carries (RFC-0033). The last group is here although nothing pushes a
/// signal yet: a corpus that adopts a signal called `commit` and finds out when its index is
/// built has committed a calculator's output under a name it now has to change, and the
/// declaration that would have caught it costs one list.
const RESERVED: &[&str] = &[
    NODE_KEY,
    // `EmbedRecord`'s own fields. Held to the struct by `reserved_covers_every_record_field`.
    "path",
    "class",
    "label",
    "text",
    "commit",
    "kind",
    "signals",
    "properties",
    // A remote vector's metadata, and the column an index writes beside them.
    crate::s3vectors::META_KEY_CORPUS,
    crate::s3vectors::META_KEY_TEXT_TRUNCATED,
    crate::s3vectors::META_KEY_EMBED_CONFIG,
    "vector",
];

/// One computed file, and what it turned out to be.
#[derive(Debug, Clone)]
pub struct ComputedFile {
    /// Repository-relative path, so it can be compared with a receipt's `outputs` and with a
    /// capability's `writes` without either side normalizing.
    pub path: String,
    /// The capabilities whose `writes` declaration covers this path.
    ///
    /// A list rather than one name because a manifest may legitimately have two steps writing
    /// into one directory — `examples/streamflow` does — and narrowing that to a single owner
    /// would mean guessing. **Empty is the interesting case**: a file no capability declares is
    /// a file nothing in the manifest is accountable for, which is what
    /// [`Signals::problems`] reports and what `doctor` asks about.
    pub declared_by: Vec<String>,
    /// The signal names this file carries, in byte order. Empty for a file with no table.
    pub names: Vec<String>,
    /// How many rows keyed a node.
    ///
    /// No digest beside it, deliberately: *are these the bytes the run wrote* is a question the
    /// receipt already answers, over the whole of a step's `outputs` rather than over the files
    /// that happen to be under this directory. A second copy of that comparison here would be a
    /// second opinion about freshness — see `cmd::run::standing`.
    pub rows: usize,
}

/// Every computed file in a corpus, and every signal they key onto a node.
#[derive(Debug, Default, Clone)]
pub struct Signals {
    /// Every `*.yml` under `.yidam/computed/`, in byte order.
    pub files: Vec<ComputedFile>,
    /// Signals by the reference of the node they describe — `<class>/<name>`.
    ///
    /// One map, keyed the way the *file* writes it, and the two consumers take the two halves of
    /// that one parse. [`crate::cmd::embed`] holds [`crate::corpus::Node::rel`] and asks through
    /// [`Self::for_node`], which converts. The typed calculator arm's projection reads the
    /// references straight, because `marshal::NodeView::id` is one. Keeping the path form here
    /// instead would send that projection back through RFC-0032's grammar to recover a string
    /// this module had already parsed out.
    by_reference: BTreeMap<String, BTreeMap<String, Value>>,
    /// What could not be read, one sentence each, in the order the files were read.
    ///
    /// Accumulated rather than returned as an error. A malformed computed file is not a reason
    /// for `embed` to refuse to embed a corpus — the same argument
    /// [`crate::cmd::run::receipt::Receipt::committed_state`] makes about a receipt that does
    /// not parse — and it is exactly what `doctor` exists to say out loud.
    pub problems: Vec<String>,
}

/// One file, parsed as much as this module reads of it.
///
/// Unknown fields are *allowed*, which is the opposite of the manifest's rule and for the
/// opposite reason. A manifest field this binary ignored would silently not be enforced; a
/// computed file's other keys are a calculator's own account of its method and its summary,
/// which this module has no business having an opinion about. `travel-tier.yml` carries a
/// `method:` block naming the guideline it implements, and reading it would be inventing a
/// second contract over prose written for a person.
#[derive(serde::Deserialize)]
struct Table {
    #[serde(default)]
    format_version: Option<u32>,
    #[serde(default)]
    signals: Option<Vec<serde_yaml::Mapping>>,
}

/// Which kind of tree a read is over, and therefore which questions it may ask of it.
///
/// Two of this module's refusals are about the *surroundings* of a computed file rather than
/// about the file: whether some capability's `writes` covers it, and whether the nodes its rows
/// key on are there. Both are questions about a corpus, and both are the right questions when a
/// corpus is what is in front of you — this module's note argues each at length, and `doctor`
/// exists to ask them.
///
/// A step of a run does not stand in a corpus. It stands in a scratch tree holding exactly what
/// its `reads` resolve to — the mechanism RFC-0026 refuses an undeclared read with.
/// `examples/streamflow`'s `disclosure-envelope` declares one computed file, no corpus path and
/// no manifest, so in that tree both questions have the same answer for every row, and it is an
/// answer about the declaration doing its job rather than about anything being wrong. Asked
/// there, they would report a broken corpus and carry no signals — which is how a typed second
/// stage would come to compute over an empty table and commit a plausible, wrong file (#1105).
///
/// So this is not a strictness dial. It is which of two trees is being read, and each question
/// below is asked of the tree it is a question about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tree {
    /// A corpus, with its manifest and its nodes. What `doctor`, `embed` and `status` read.
    Corpus,
    /// The `reads`-bounded tree a step of a run stands in.
    Reads,
}

impl Signals {
    /// Read every computed file at `root`, resolving each against the capability manifest.
    ///
    /// Never fails. A corpus with no `.yidam/computed/` is the overwhelmingly common case and
    /// answers empty; everything else that can go wrong is a [`Signals::problems`] entry, so a
    /// caller gets both the signals that *did* read and the account of the ones that did not.
    pub fn load(root: &Path) -> Self {
        Self::read(root, Tree::Corpus)
    }

    /// Read the computed files a step's `reads` put in the tree it stands in.
    ///
    /// Every refusal that is about a computed file, and none of the two that are about a corpus
    /// it is not in — this module's `Tree` carries the argument.
    /// `gluon_arm::marshal::project` is the caller, which is how a pipeline's second stage can be
    /// typed at all (#1105).
    pub fn in_reads(root: &Path) -> Self {
        Self::read(root, Tree::Reads)
    }

    fn read(root: &Path, tree: Tree) -> Self {
        let mut out = Self::default();
        let dir = yidam_computed_dir(root);
        if !dir.is_dir() {
            return out;
        }

        // The manifest is read for attribution and is not required. A corpus holding computed
        // files and no manifest is a corpus whose calculators ran under some other tool, or
        // whose manifest was deleted after they did; either way the files are still readable
        // and the account of them says nobody declares them.
        let declarations = match root.join(MANIFEST).exists() {
            false => BTreeMap::new(),
            true => match Manifest::load(root) {
                Ok(m) => m
                    .capability
                    .iter()
                    .map(|(name, cap)| (name.clone(), cap.writes.clone()))
                    .collect(),
                Err(e) => {
                    out.problems.push(format!(
                        "{MANIFEST} does not parse, so no computed file can be attributed to \
                         the capability that wrote it: {e}"
                    ));
                    BTreeMap::new()
                }
            },
        };

        // Where each signal name was first seen, for the collision refusal. One map across
        // every file, because the collision this prevents is between files.
        let mut claimed: BTreeMap<String, String> = BTreeMap::new();

        for (path, bytes) in read_dir_sorted(&dir) {
            let rel = format!(".yidam/computed/{path}");
            let declared_by: Vec<String> = declarations
                .iter()
                .filter(|(_, writes)| writes.iter().any(|g| glob_covers(g, &rel)))
                .map(|(name, _)| name.clone())
                .collect();
            if declared_by.is_empty() && tree == Tree::Corpus {
                out.problems.push(format!(
                    "{rel} is not covered by any capability's `writes`, so nothing in \
                     {MANIFEST} is accountable for it"
                ));
            }

            let mut file = ComputedFile {
                path: rel.clone(),
                declared_by,
                names: Vec::new(),
                rows: 0,
            };

            let text = match String::from_utf8(bytes) {
                Ok(t) => t,
                Err(_) => {
                    out.problems.push(format!("{rel} is not UTF-8"));
                    out.files.push(file);
                    continue;
                }
            };
            let table: Table = match serde_yaml::from_str(&text) {
                Ok(t) => t,
                Err(e) => {
                    out.problems
                        .push(format!("{rel} does not parse as YAML: {e}"));
                    out.files.push(file);
                    continue;
                }
            };
            let Some(rows) = table.signals else {
                // A computed artifact with no keyed table. Found, attributed, no signals.
                out.files.push(file);
                continue;
            };
            match table.format_version {
                Some(FORMAT_VERSION) => {}
                Some(v) => {
                    out.problems.push(format!(
                        "{rel} declares `format_version: {v}` and this yidam reads \
                         {FORMAT_VERSION}, so its signals were not read"
                    ));
                    out.files.push(file);
                    continue;
                }
                None => {
                    out.problems.push(format!(
                        "{rel} carries `signals:` and no `format_version`, so a later reader \
                         could not tell which contract it was written against"
                    ));
                    out.files.push(file);
                    continue;
                }
            }

            // Names seen in this file, and names a problem has already been reported for.
            // Two sets rather than one: a defect is worth saying once however many rows carry
            // it, and a name that reached a node is what the file *carries*.
            let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let mut reported: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for (i, row) in rows.iter().enumerate() {
                let Some(node) = row.get(NODE_KEY).and_then(scalar_str) else {
                    out.problems.push(format!(
                        "{rel} row {} declares no `{NODE_KEY}`, so there is no node it is \
                         about",
                        i + 1
                    ));
                    continue;
                };
                let Some(reference) = node_reference(&node) else {
                    out.problems.push(format!(
                        "{rel} row {} keys on `{node}`, which is not a node in this corpus in \
                         RFC-0032's grammar",
                        i + 1
                    ));
                    continue;
                };
                let node_path = node_file(&reference);
                if tree == Tree::Corpus && !root.join(&node_path).is_file() {
                    out.problems.push(format!(
                        "{rel} row {} keys on `{node}`, and this corpus holds no {node_path}",
                        i + 1
                    ));
                    continue;
                }
                file.rows += 1;

                for (k, v) in row {
                    let Some(name) = k.as_str() else { continue };
                    if name == NODE_KEY {
                        continue;
                    }
                    names.insert(name.to_string());
                    if RESERVED.contains(&name) {
                        if reported.insert(name.to_string()) {
                            out.problems.push(format!(
                                "{rel} declares a signal called `{name}`, which is a field a \
                                 record already carries"
                            ));
                        }
                        continue;
                    }
                    match claimed.get(name) {
                        Some(first) if first != &rel => {
                            if reported.insert(name.to_string()) {
                                out.problems.push(format!(
                                    "`{name}` is declared by both {first} and {rel}; a signal \
                                     name is corpus-wide, so one of the two has to change"
                                ));
                            }
                            continue;
                        }
                        _ => {}
                    }
                    let Some(value) = scalar(v) else {
                        if reported.insert(name.to_string()) {
                            out.problems.push(format!(
                                "{rel} declares `{name}` as a list or a mapping; a signal is \
                                 one value about one node"
                            ));
                        }
                        continue;
                    };
                    claimed.insert(name.to_string(), rel.clone());
                    out.by_reference
                        .entry(reference.clone())
                        .or_default()
                        .insert(name.to_string(), value);
                }
            }
            // Only the names that reached a node. A name that was refused is in `problems`
            // and is not something this file carries.
            file.names = names
                .into_iter()
                .filter(|n| claimed.get(n) == Some(&rel))
                .collect();
            out.files.push(file);
        }
        out
    }

    /// The signals about one node, by its repository-relative path. Empty where there are none.
    pub fn for_node(&self, rel: &str) -> BTreeMap<String, Value> {
        node_id(rel)
            .and_then(|id| self.by_reference.get(id))
            .cloned()
            .unwrap_or_default()
    }

    /// Every node that carries a signal, by reference and in byte order, with its signals.
    ///
    /// Byte order over both, which is what makes a projection of this a function of the files
    /// and not of the run: a `BTreeMap` all the way down.
    pub fn by_reference(&self) -> impl Iterator<Item = (&str, &BTreeMap<String, Value>)> {
        self.by_reference.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// How many nodes carry at least one signal.
    pub fn nodes(&self) -> usize {
        self.by_reference.len()
    }

    /// Every signal name in the corpus, in byte order.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .by_reference
            .values()
            .flat_map(|m| m.keys().cloned())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// The node reference a row's `node:` names, or `None` where it names no node in this corpus.
///
/// Refuses three things, each for its own reason. A kind other than `Node`: a signal is about a
/// node, and a catalog entry or a crate has no embedding for one to reach. A naming corpus: the
/// reference would be about a node in another repository, which this one cannot key onto. A
/// revision pin: `x@abc` and `x` denote the same node in two states (RFC-0032 §4.3), and a
/// signal keyed at a past commit would be attached to the node as it stands now — which is the
/// one reading that is certainly wrong.
fn node_reference(reference: &str) -> Option<String> {
    let r = yidam_core::uri::parse_reference(reference)?;
    if r.kind != yidam_core::uri::Kind::Node || r.corpus.is_some() || r.rev.is_some() {
        return None;
    }
    if r.fragment.is_some() || r.segments().len() != 2 {
        return None;
    }
    Some(r.path.to_string())
}

/// The repository-relative file a node reference names.
///
/// The inverse of [`node_id`], and the two are a pair on purpose: `<class>/<name>` and
/// `.yidam/corpus/<class>/<name>.yml` are one fact in two spellings, and every other module in
/// this CLI holds one or the other. Spelling the conversion once here is what keeps a third
/// reading of it from appearing in a consumer.
fn node_file(reference: &str) -> String {
    format!(".yidam/corpus/{reference}.yml")
}

/// The node reference a repository-relative corpus file is written as, or `None` for a path that
/// is not one.
fn node_id(rel: &str) -> Option<&str> {
    rel.strip_prefix(".yidam/corpus/")?.strip_suffix(".yml")
}

/// A YAML scalar as a JSON one, or `None` for a sequence or a mapping.
///
/// Numbers stay numbers and booleans stay booleans. #1030 is about a corpus having no way to
/// declare a numeric *property*; a signal is not a property, so nothing here has to flatten a
/// number into a string to carry it.
fn scalar(v: &serde_yaml::Value) -> Option<Value> {
    match v {
        serde_yaml::Value::Bool(b) => Some(Value::Bool(*b)),
        serde_yaml::Value::String(s) => Some(Value::String(s.clone())),
        serde_yaml::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Some(Value::from(i)),
            (None, Some(f)) => serde_json::Number::from_f64(f).map(Value::Number),
            _ => None,
        },
        _ => None,
    }
}

fn scalar_str(v: &serde_yaml::Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// Every `*.yml` directly under `dir`, by name in byte order, with its bytes.
///
/// Byte order rather than directory order, for the reason the example's own calculators export
/// `LC_ALL=C` before they sort: the account of a corpus is a property of the corpus and not of
/// the filesystem that happened to enumerate it. Not recursive — a nested directory here would
/// be a second organizing idea nobody has asked for, and one that a `writes` glob of
/// `.yidam/computed/**` would silently admit.
fn read_dir_sorted(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "yml"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
        .into_iter()
        .filter_map(|n| std::fs::read(dir.join(&n)).ok().map(|b| (n, b)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corpus with one class, one node, and whatever computed files a test writes.
    fn corpus(files: &[(&str, &str)], manifest: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".yidam/corpus/gage")).unwrap();
        std::fs::write(
            root.join(".yidam/corpus/gage/canyon-outlet.yml"),
            "label: Canyon Outlet\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".yidam/computed")).unwrap();
        for (name, text) in files {
            std::fs::write(root.join(".yidam/computed").join(name), text).unwrap();
        }
        if let Some(m) = manifest {
            std::fs::write(root.join(MANIFEST), m).unwrap();
        }
        dir
    }

    const MANIFEST_ONE_STEP: &str = r#"
[capability.travel-tier]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/travel-tier.sh"]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "compute"
"#;

    const TRAVEL_TIER: &str = "\
format_version: 1
method:
  rule: the weakest claim beneath it
signals:
  - node: gage/canyon-outlet
    travels_as: verified
    downgraded: false
";

    #[test]
    fn a_row_keyed_by_a_relative_reference_reaches_the_node_it_names() {
        let dir = corpus(&[("travel-tier.yml", TRAVEL_TIER)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert_eq!(s.problems, Vec::<String>::new());
        let signals = s.for_node(".yidam/corpus/gage/canyon-outlet.yml");
        assert_eq!(signals["travels_as"], Value::String("verified".into()));
        assert_eq!(signals["downgraded"], Value::Bool(false));
        assert_eq!(s.nodes(), 1);
    }

    /// A step's own tree is not a corpus, so `in_reads` carries a row the corpus check drops.
    ///
    /// This is the pipeline case, and the reason [`Tree`] has two variants (#1105).
    /// `examples/streamflow`'s `disclosure-envelope` declares
    /// `reads = [".yidam/computed/travel-tier.yml", …]` and no corpus path, so the tree it is
    /// invoked in holds the answer it computes from and no node at all. Under [`Signals::load`]
    /// every row of that answer keys on a node that is not there: one problem per row about a
    /// corpus nobody asked for, and no signals. A typed second stage would compute over an empty
    /// table and commit a plausible, wrong file.
    ///
    /// Both halves are asserted, because each is the other's control: the two questions have to
    /// still be asked of the tree they are questions about.
    #[test]
    fn a_reads_bounded_tree_carries_its_signals_and_a_corpus_is_still_checked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".yidam/computed")).unwrap();
        std::fs::write(root.join(".yidam/computed/travel-tier.yml"), TRAVEL_TIER).unwrap();

        let told = Signals::in_reads(root);
        assert_eq!(told.problems, Vec::<String>::new());
        let rows: Vec<(&str, Vec<&str>)> = told
            .by_reference()
            .map(|(node, values)| (node, values.keys().map(String::as_str).collect()))
            .collect();
        assert_eq!(
            rows,
            vec![("gage/canyon-outlet", vec!["downgraded", "travels_as"])],
            "a second stage is handed the signals its `reads` put in the tree"
        );

        let read = Signals::load(root);
        assert_eq!(read.nodes(), 0);
        let p = read.problems.join("\n");
        assert!(
            p.contains("this corpus holds no .yidam/corpus/gage/canyon-outlet.yml"),
            "the corpus-membership check is still the reading `doctor` gets: {p}"
        );
    }

    /// The full grammar, not a second form invented here.
    #[test]
    fn the_absolute_form_of_the_grammar_reaches_the_same_node() {
        for written in [
            "node/gage/canyon-outlet",
            "yidam://corpus/node/gage/canyon-outlet",
        ] {
            assert_eq!(
                node_reference(written).as_deref(),
                Some("gage/canyon-outlet"),
                "`{written}` is the grammar's own form of one node"
            );
        }
    }

    /// The two spellings of one node are each other's inverse, over the form a file writes.
    ///
    /// The pair is what lets one map serve both consumers — see [`Signals::by_reference`] — so a
    /// change to either half that did not round-trip would make `for_node` answer empty for
    /// every node, which is indistinguishable from a corpus that computed nothing.
    #[test]
    fn a_reference_and_the_file_it_names_round_trip() {
        let reference = node_reference("gage/canyon-outlet").expect("a node reference");
        let file = node_file(&reference);
        assert_eq!(file, ".yidam/corpus/gage/canyon-outlet.yml");
        assert_eq!(node_id(&file), Some("gage/canyon-outlet"));
        assert_eq!(node_id("docs/cli-reference.md"), None);
    }

    /// A signal lands on a node in *this* corpus's embedding, and a foreign name has none.
    #[test]
    fn a_reference_naming_another_corpus_is_not_a_node_here() {
        assert!(node_reference("yidam://streamflow/node/gage/canyon-outlet").is_none());
    }

    /// `x@abc` and `x` are the same node in two states; a signal attached at a past commit
    /// would be read as a signal about the node as it stands.
    #[test]
    fn a_revision_pin_is_refused_rather_than_ignored() {
        assert!(node_reference("gage/canyon-outlet@abc1234").is_none());
    }

    #[test]
    fn a_catalog_reference_is_not_a_node() {
        assert!(node_reference("catalog/usgs-nwis").is_none());
    }

    /// The collision names both files, because either one of them is the one to change and a
    /// reader has to be able to see which.
    #[test]
    fn two_files_declaring_one_signal_name_is_refused_naming_both() {
        let other = "\
format_version: 1
signals:
  - node: gage/canyon-outlet
    travels_as: open
";
        let dir = corpus(
            &[("travel-tier.yml", TRAVEL_TIER), ("other.yml", other)],
            Some(MANIFEST_ONE_STEP),
        );
        let s = Signals::load(dir.path());
        let p = s.problems.join("\n");
        assert!(p.contains("travel-tier.yml"), "{p}");
        assert!(p.contains("other.yml"), "{p}");
        // The first reading stands and the second is not merged over it — and *first* is the
        // byte order the directory is read in, so `other.yml` precedes `travel-tier.yml`. That
        // the survivor is the alphabetically-first file is not a rule anybody should rely on;
        // what the test pins is that one of the two wins, always the same one, and that the
        // loser is reported rather than applied.
        assert_eq!(
            s.for_node(".yidam/corpus/gage/canyon-outlet.yml")["travels_as"],
            Value::String("open".into())
        );
    }

    #[test]
    fn a_signal_named_after_a_record_field_is_refused() {
        let text = "\
format_version: 1
signals:
  - node: gage/canyon-outlet
    label: Something Else
";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert!(
            s.problems.join("\n").contains("`label`"),
            "{:?}",
            s.problems
        );
        assert!(s
            .for_node(".yidam/corpus/gage/canyon-outlet.yml")
            .is_empty());
    }

    /// The argument `receipt.rs` makes for its own version field, applied to this record.
    #[test]
    fn a_table_with_no_format_version_is_not_read() {
        let text = "signals:\n  - node: gage/canyon-outlet\n    travels_as: verified\n";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert!(
            s.problems.join("\n").contains("format_version"),
            "{:?}",
            s.problems
        );
        assert_eq!(s.nodes(), 0);
    }

    #[test]
    fn a_future_format_version_is_reported_rather_than_read_as_this_one() {
        let text = "format_version: 99\nsignals:\n  - node: gage/canyon-outlet\n    t: v\n";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert!(s.problems.join("\n").contains("99"), "{:?}", s.problems);
        assert_eq!(s.nodes(), 0);
    }

    /// A calculator whose output is a report for a person is not obliged to key it by node.
    #[test]
    fn a_file_with_no_table_is_found_attributed_and_carries_no_signals() {
        let text = "method:\n  rule: a partition\ntiers:\n  - tier: verified\n    nodes: 1\n";
        let dir = corpus(&[("envelope.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert_eq!(s.problems, Vec::<String>::new());
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].declared_by, vec!["travel-tier".to_string()]);
        assert_eq!(s.files[0].rows, 0);
        assert_eq!(s.nodes(), 0);
    }

    /// The case the issue was filed about, one level down: a file nothing is accountable for.
    #[test]
    fn a_file_no_capability_declares_is_reported() {
        let manifest = r#"
[capability.travel-tier]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/travel-tier.sh"]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/travel-tier.yml"]
verb   = "compute"
"#;
        let dir = corpus(
            &[("travel-tier.yml", TRAVEL_TIER), ("stray.yml", TRAVEL_TIER)],
            Some(manifest),
        );
        let s = Signals::load(dir.path());
        let p = s.problems.join("\n");
        assert!(p.contains("stray.yml"), "{p}");
        assert!(!p.contains("travel-tier.yml is not covered"), "{p}");
    }

    #[test]
    fn a_corpus_with_no_manifest_still_reads_its_files_and_says_nobody_declares_them() {
        let dir = corpus(&[("travel-tier.yml", TRAVEL_TIER)], None);
        let s = Signals::load(dir.path());
        assert_eq!(s.nodes(), 1);
        assert!(s.files[0].declared_by.is_empty());
        assert!(s.problems.join("\n").contains("not covered"));
    }

    /// A table keyed on nodes that are not there is indistinguishable from an empty answer
    /// unless something says so.
    #[test]
    fn a_row_keyed_on_a_node_this_corpus_does_not_hold_is_reported() {
        let text = "format_version: 1\nsignals:\n  - node: gage/missing\n    travels_as: open\n";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert!(
            s.problems.join("\n").contains("gage/missing"),
            "{:?}",
            s.problems
        );
        assert_eq!(s.nodes(), 0);
    }

    #[test]
    fn a_list_valued_signal_is_refused() {
        let text =
            "format_version: 1\nsignals:\n  - node: gage/canyon-outlet\n    members: [a, b]\n";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        assert!(
            s.problems.join("\n").contains("`members`"),
            "{:?}",
            s.problems
        );
        assert_eq!(s.nodes(), 0);
    }

    /// #1030 is about a numeric *property*. A signal is not one, so a number stays a number.
    #[test]
    fn a_numeric_signal_stays_numeric() {
        let text = "format_version: 1\nsignals:\n  - node: gage/canyon-outlet\n    q95: 12.5\n    links: 3\n";
        let dir = corpus(&[("c.yml", text)], Some(MANIFEST_ONE_STEP));
        let s = Signals::load(dir.path());
        let signals = s.for_node(".yidam/corpus/gage/canyon-outlet.yml");
        assert_eq!(signals["q95"].as_f64(), Some(12.5));
        assert_eq!(signals["links"].as_i64(), Some(3));
    }

    /// A corpus with nothing computed is the common case and answers empty rather than
    /// reporting an absence nobody asked about.
    #[test]
    fn a_corpus_with_no_computed_directory_answers_empty() {
        let dir = tempfile::tempdir().unwrap();
        let s = Signals::load(dir.path());
        assert!(s.files.is_empty());
        assert!(s.problems.is_empty());
        assert_eq!(s.nodes(), 0);
    }

    /// [`RESERVED`] is a hand-written list and `EmbedRecord` is the struct it is about. The
    /// list is held to the struct by serializing one and reading its keys back, so a field
    /// added there fails here rather than becoming a signal name a corpus may take.
    #[test]
    fn reserved_covers_every_record_field() {
        let record = crate::cmd::embed::EmbedRecord {
            path: "p".into(),
            class: "c".into(),
            label: "l".into(),
            text: "t".into(),
            commit: "abc".into(),
            kind: "node".into(),
            signals: BTreeMap::from([("travels_as".to_string(), Value::Bool(true))]),
            properties: BTreeMap::from([("began".to_string(), Value::from("1893"))]),
        };
        let json = serde_json::to_value(&record).unwrap();
        for key in json.as_object().unwrap().keys() {
            assert!(
                RESERVED.contains(&key.as_str()),
                "`{key}` is a field of an embedding record and is not in RESERVED, so a \
                 corpus may declare a signal that collides with it"
            );
        }
    }
}
