//! #1217's definition of done, for the three pods a cluster gather is.

use std::cell::Cell;
use std::io::Write as _;
use std::path::Path;

use super::*;
use crate::cmd::gather::{self as g, Outcome};
use crate::git::fixture;

const GAGE: &str = "class: gage\nproperties:\n  - name: station\n    type: string\n  \
                    - name: peak_cfs\n    type: number\n";

const SPEC: &str = r#"
question = "What peak discharge has each gage recorded?"
query = "gage"
answer = "peak_cfs"
key = "station"
lands_as = "question"

[peers.alpha]
classes = { gage = "station" }
properties = { station = "usgs_id", peak_cfs = "flood_peak" }

[peers.beta]
classes = { gage = "site" }
properties = { station = "id", peak_cfs = "peak" }
"#;

/// A `.yiz` as `tonpa publish` would serve it: a manifest pinned to `commit`, and one node.
fn yiz(commit: &str, class: &str, props: [&str; 2], id: &str, peak: &str) -> Vec<u8> {
    let files = [
        (
            "manifest.yml".to_string(),
            format!("commit: \"{commit}\"\n"),
        ),
        (
            format!("corpus/{class}.ont.yml"),
            format!(
                "class: {class}\nproperties:\n  - name: {}\n    type: string\n  - name: {}\n    \
                 type: number\n",
                props[0], props[1]
            ),
        ),
        (
            format!("corpus/{class}/one.yml"),
            format!(
                "class: {class}\nlabel: one\nproperties:\n  {}: \"{id}\"\n  {}: {peak}\n",
                props[0], props[1]
            ),
        ),
    ];
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (path, body) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, path, body.as_bytes()).unwrap();
    }
    let mut gz = tar.into_inner().unwrap();
    gz.flush().unwrap();
    gz.finish().unwrap()
}

struct World {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    vault: std::path::PathBuf,
    /// `url → bytes`, what a fetch would find.
    served: BTreeMap<String, Vec<u8>>,
}

impl World {
    /// A corpus with two peers installed from real bundles and locked by url, sha and commit —
    /// the state `tonpa install` leaves, committed.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&vault).unwrap();
        fixture::init(&root);
        fixture::write(&root, ".yidam/corpus/gage.ont.yml", GAGE);
        fixture::write(&root, ".yidam/corpus/question.ont.yml", "class: question\n");
        fixture::write(&root, ".yidam/gathers/peaks.toml", SPEC);
        let peers = [
            (
                "alpha",
                "aaa1111",
                yiz(
                    "aaa1111",
                    "station",
                    ["usgs_id", "flood_peak"],
                    "01646500",
                    "12000",
                ),
            ),
            (
                "beta",
                "bbb2222",
                yiz("bbb2222", "site", ["id", "peak"], "01646500", "15500"),
            ),
        ];
        let mut lock = String::new();
        let mut served = BTreeMap::new();
        for (name, commit, bytes) in peers {
            let url = format!("https://peers.invalid/{name}.yiz");
            crate::deps::extract_bundle(&bytes, &crate::paths::tonpa_dir(&root).join(name))
                .unwrap();
            std::fmt::Write::write_fmt(
                &mut lock,
                format_args!(
                    "[[package]]\nname = \"{name}\"\nurl = \"{url}\"\nsha256 = \"{}\"\n\
                     commit = \"{commit}\"\n\n",
                    crate::deps::sha256_hex(&bytes)
                ),
            )
            .unwrap();
            served.insert(url, bytes);
        }
        fixture::write(&root, ".yidam/tonpa/tonpa.lock", &lock);
        fixture::commit(&root, "tonpa: install alpha and beta");
        Self {
            _dir: dir,
            root,
            vault,
            served,
        }
    }

    fn store(&self) -> Box<dyn Store> {
        VaultArgs {
            vault: "default".into(),
            vault_url: format!("file://{}", self.vault.display()),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
        }
        .open()
        .unwrap()
    }

    fn head(&self) -> String {
        fixture::git_out(&self.root, &["rev-parse", "HEAD"])
    }

    fn plan(&self) -> Survey {
        survey_in(&self.root, "peaks", &self.head(), "unused").unwrap()
    }

    /// Ask one planned peer in its own scratch directory, as its own pod would.
    fn ask(&self, ask: &Ask, fetch: Fetch) -> AskOutput {
        let scratch = tempfile::tempdir().unwrap();
        ask_in(ask, None, self.store().as_ref(), scratch.path(), fetch).unwrap()
    }

    fn fetcher(&self) -> impl Fn(&str) -> Result<Vec<u8>> + '_ {
        |url: &str| {
            self.served
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("404 for {url}"))
        }
    }

    fn gather(&self, input: &str, outputs: &[AskOutput]) -> (StepOutput, GatherReport) {
        let scratch = tempfile::tempdir().unwrap();
        gather_in(
            &self.root,
            "peaks",
            input,
            outputs,
            self.store().as_ref(),
            scratch.path(),
        )
        .unwrap()
    }
}

fn tree(root: &Path, rev: &str) -> String {
    fixture::git_out(root, &["rev-parse", &format!("{rev}^{{tree}}")])
}

fn outcomes(peers: &[PeerReport]) -> BTreeMap<String, Outcome> {
    peers
        .iter()
        .map(|p| (p.package.clone(), p.outcome))
        .collect()
}

#[test]
fn the_plan_is_translated_on_the_gathering_side_and_pinned_to_the_lock() {
    let w = World::new();
    let s = w.plan();
    assert_eq!(s.asking, 2);
    let alpha = &s.asks[0];
    assert_eq!(alpha.peer, "alpha");
    assert_eq!(alpha.pin, "aaa1111", "the lock's commit, not a manifest's");
    // The asker is handed the peer's words and never the correspondence.
    assert_eq!(alpha.class, "station");
    assert_eq!(alpha.answer, "flood_peak");
    assert_eq!(alpha.key.as_deref(), Some("usgs_id"));
    assert!(alpha.query.contains("station"), "{}", alpha.query);
    assert!(!alpha.query.contains("gage"), "{}", alpha.query);
    // What `show` writes, `parse` reads back to the same query.
    let q = crate::cmd::query::lang::parse(&alpha.query).unwrap();
    assert_eq!(g::show(&q), alpha.query);
    // And the plan round-trips as the JSON a pod hands the next.
    let back: Survey = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.asks, s.asks);
}

#[test]
fn a_bundle_already_in_the_vault_is_not_fetched_again() {
    let w = World::new();
    let s = w.plan();
    let fetched = Cell::new(0);
    let serve = w.fetcher();
    let counting = |url: &str| {
        fetched.set(fetched.get() + 1);
        serve(url)
    };
    let first = w.ask(&s.asks[0], &counting);
    assert_eq!(first.source, Some(Source::Fetched));
    assert_eq!(first.outcome, Outcome::Answered);

    // `has` answers before any transfer: the second asker of this pin fetches nothing, and
    // answers even with a url nothing is served at.
    let unreachable = |_: &str| -> Result<Vec<u8>> { panic!("fetched a bundle the vault holds") };
    let second = w.ask(&s.asks[0], &unreachable);
    assert_eq!(second.source, Some(Source::Vault));
    assert_eq!(second.outcome, Outcome::Answered);
    assert_eq!(fetched.get(), 1);
    assert_eq!(
        second.record, first.record,
        "the same bytes answer the same way"
    );
}

#[test]
fn a_peer_that_cannot_be_fetched_is_refused_with_the_reason() {
    let w = World::new();
    let s = w.plan();
    let down = |url: &str| -> Result<Vec<u8>> { anyhow::bail!("connection refused by {url}") };
    let out = w.ask(&s.asks[0], &down);
    assert_eq!(out.outcome, Outcome::Refused);
    assert_eq!(out.source, None);
    let scratch = tempfile::tempdir().unwrap();
    let rec = fetch_record(w.store().as_ref(), &out.record, scratch.path()).unwrap();
    let why = rec.report.reason.unwrap();
    assert!(why.contains("https://peers.invalid/alpha.yiz"), "{why}");
    assert!(why.contains("connection refused"), "{why}");
}

#[test]
fn bytes_that_are_not_the_locked_ones_are_refused_and_kept_out_of_the_vault() {
    let w = World::new();
    let s = w.plan();
    let wrong = |_: &str| {
        Ok(yiz(
            "aaa1111",
            "station",
            ["usgs_id", "flood_peak"],
            "x",
            "1",
        ))
    };
    let out = w.ask(&s.asks[0], &wrong);
    assert_eq!(out.outcome, Outcome::Refused);
    let hash = ContentHash::parse(&s.asks[0].lock.as_ref().unwrap().sha256).unwrap();
    assert!(!w.store().has(&hash).unwrap());
}

#[test]
fn a_bundle_with_an_entry_outside_itself_is_refused() {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    let body = b"x";
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    // `append_data` refuses `..`, so the name is written into the header by hand.
    header.as_gnu_mut().unwrap().name[..9].copy_from_slice(b"../escape");
    header.set_cksum();
    tar.append(&header, &body[..]).unwrap();
    let bytes = tar.into_inner().unwrap().finish().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("peer");
    let err = crate::deps::extract_bundle(&bytes, &dest).unwrap_err();
    assert!(format!("{err:#}").contains("outside itself"), "{err:#}");
    assert!(!dir.path().join("escape").exists());
}

#[test]
fn the_cluster_builds_the_tree_a_local_gather_builds_over_the_same_pins() {
    let w = World::new();
    let input = w.head();
    let s = w.plan();
    let serve = w.fetcher();
    let outputs: Vec<AskOutput> = s.asks.iter().map(|a| w.ask(a, &serve)).collect();
    let (step, cluster) = w.gather(&input, &outputs);
    assert_eq!(step.outcome, StepOutcome::Ran);
    assert_eq!(step.step, "gather/peaks");
    assert_eq!(step.class, "epistemic");
    let sha = step.sha.clone().unwrap();
    assert!(step.bundle.is_some());

    let local = g::run(
        &w.root,
        &g::Options {
            name: "peaks".into(),
            dry_run: false,
            force: false,
            format: Format::Text,
        },
    )
    .unwrap();
    let branch = local.landed.as_ref().unwrap().branch.clone();
    assert_eq!(tree(&w.root, &sha), tree(&w.root, &branch));
    assert_eq!(outcomes(&cluster.peers), outcomes(&local.peers));
    assert_eq!(cluster.cites, local.cites);
    // No ref moved on the gathering side: the commits are objects, shipped through the vault.
    assert_eq!(w.head(), input);
    assert_eq!(
        fixture::git_out(
            &w.root,
            &["for-each-ref", "--format=%(refname)", "refs/heads/propose"]
        ),
        format!("refs/heads/{branch}")
    );

    // Each peer's receipt records what produced its answer.
    let receipt = fixture::git_out(
        &w.root,
        &["show", &format!("{sha}:.yidam/runs/gather/peaks/alpha.yml")],
    );
    let r: serde_yaml::Value = serde_yaml::from_str(&receipt).unwrap();
    assert_eq!(r["format_version"].as_u64(), Some(2));
    assert_eq!(r["kind"].as_str(), Some("gather"));
    assert_eq!(r["input"]["commit"].as_str(), Some("aaa1111"));
    assert_eq!(r["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
    let files: Vec<&str> = r["input"]["files"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [".yidam/gathers/peaks.toml", ".yidam/tonpa/alpha/bundle.yiz"]
    );

    // A repeat run over pins that have not moved writes nothing.
    let (again, _) = w.gather(&sha, &outputs);
    assert_eq!(again.outcome, StepOutcome::Unchanged);
    assert!(again.sha.is_none() && again.bundle.is_none());
}

#[test]
fn a_peer_whose_asker_returned_nothing_stays_in_the_roll_call_as_refused() {
    let w = World::new();
    let input = w.head();
    let s = w.plan();
    let serve = w.fetcher();
    let alpha = w.ask(&s.asks[0], &serve);
    // beta's pod died: Argo aggregates its missing output as nothing at all.
    let outputs = read_asked(
        &serde_json::to_string(&vec![
            serde_json::to_string(&alpha).unwrap(),
            "".to_string(),
        ])
        .unwrap(),
    )
    .unwrap();
    assert_eq!(outputs.len(), 1);
    let (step, report) = w.gather(&input, &outputs);
    assert_eq!(step.outcome, StepOutcome::Ran);
    let beta = report.peers.iter().find(|p| p.package == "beta").unwrap();
    assert_eq!(beta.outcome, Outcome::Refused);
    assert!(
        beta.reason
            .as_deref()
            .unwrap()
            .contains("returned no record"),
        "{beta:?}"
    );
    assert_eq!(report.peers.len(), 2);
}

#[test]
fn a_record_answering_a_different_ask_is_refused_not_cited() {
    let w = World::new();
    let input = w.head();
    let s = w.plan();
    let serve = w.fetcher();
    let honest = w.ask(&s.asks[0], &serve);
    // A record whose answers are real and whose ask is not the one this pin plans — the asker
    // was handed some other question, or the record was edited on the way.
    let scratch = tempfile::tempdir().unwrap();
    let mut rec = fetch_record(w.store().as_ref(), &honest.record, scratch.path()).unwrap();
    assert_eq!(rec.report.outcome, Outcome::Answered);
    rec.ask.query = "station where usgs_id = \"x\"".into();
    let file = scratch.path().join("forged.json");
    std::fs::write(&file, serde_json::to_vec(&rec).unwrap()).unwrap();
    let forged = AskOutput {
        record: bundle::put(w.store().as_ref(), &file).unwrap(),
        ..honest
    };
    let (_, report) = w.gather(&input, &[forged]);
    let alpha = report.peers.iter().find(|p| p.package == "alpha").unwrap();
    assert_eq!(alpha.outcome, Outcome::Refused, "{alpha:?}");
    assert!(
        alpha.reason.as_deref().unwrap().contains("different ask"),
        "{alpha:?}"
    );
    assert!(report.cites.iter().all(|c| c.package != "alpha"));
}

#[test]
fn a_path_dependency_stays_refused_on_the_cluster() {
    let w = World::new();
    fixture::write(
        &w.root,
        ".yidam/tonpa.toml",
        "[dependencies]\nsibling = { path = \"../sibling\" }\n",
    );
    fixture::commit(&w.root, "tonpa: a path dependency");
    let s = w.plan();
    assert!(s.asks.iter().all(|a| a.peer != "sibling"));
    let sibling = s.peers.iter().find(|p| p.package == "sibling").unwrap();
    assert_eq!(sibling.outcome, Outcome::Refused);
    assert!(sibling
        .reason
        .as_deref()
        .unwrap()
        .contains("path dependency"));
}

#[test]
fn a_locked_peer_with_no_commit_is_refused_before_it_is_asked() {
    let w = World::new();
    let lock = std::fs::read_to_string(w.root.join(".yidam/tonpa/tonpa.lock")).unwrap();
    fixture::write(
        &w.root,
        ".yidam/tonpa/tonpa.lock",
        &lock.replace("commit = \"bbb2222\"\n", ""),
    );
    fixture::commit(&w.root, "tonpa: an unpinned lock entry");
    let s = w.plan();
    assert_eq!(s.asking, 1);
    assert_eq!(outcomes(&s.peers)["beta"], Outcome::Refused);
}
