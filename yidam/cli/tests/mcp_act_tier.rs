//! The write tier, end to end — RFC-0029, #474.
//!
//! `mcp_serve.rs` runs the frozen conformance cases, which say what a *response* must carry.
//! This file asserts the things a case file cannot reach: that a call actually wrote commits
//! into git, where those commits landed, that the server's own snapshot moved with them, and
//! that a server told to write and unable to fails at the command rather than at the call.
//!
//! # The one demonstration the definition of done has
//!
//! #474 asked that an agent *"read `due`, run what discharges a clock, and read the receipt"*,
//! against `examples/streamflow`. `cmd/due.rs`'s own table narrows that to one of four clocks:
//! a catalog entry past its TTL is discharged by `propose`, which drafts an `open:` per
//! expired source. The index clock wants a build (*"not a commit `propose` can draft"*) and
//! the question and phase clocks want, in the module's own word, *"a person."* So one
//! demonstration exists and three do not, and the definition of done was narrowed on the
//! record to the one that does — [`an_agent_reads_a_clock_discharges_it_and_reads_the_receipt`]
//! is it.
//!
//! # The invariant, over MCP exactly as over the CLI
//!
//! RFC-0026: *a run authors operational commits directly; every epistemic commit it produces
//! goes to a proposal branch, and nothing merges itself.* That is what makes a write tier
//! arguable at all, and #474's definition of done asks for it held over this transport. The
//! check is mechanical and is in
//! [`an_agent_triggered_write_puts_no_epistemic_commit_outside_propose`]: classify every
//! commit reachable from every ref by its leading verb, and assert the epistemic ones are all
//! on a `propose/*` ref.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

mod common;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

use common::git::{git, out as git_out};

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

/// `examples/streamflow`, staged as a git repository that declares `act`.
///
/// **The worked example and not a fixture written here.** #474 names it, and the point of
/// demonstrating against it is that it is a corpus somebody would actually copy: eight
/// instances across three classes, a real catalog entry, and an ontology doing work. A
/// purpose-built tree would prove the tool runs and not that it runs on a corpus.
///
/// Two things are added to the copy, and both are stated rather than quietly arranged. The
/// `[serve] act = true` key, because the whole tier is opt-in and the shipped example does not
/// opt in — an example that declared it would be handing every reader a writable server. And a
/// `retrieved:`/`ttl_days:` pair on its one catalog entry that puts the source past its TTL,
/// because the shipped entry is not expired and a demonstration of discharging a clock needs a
/// clock that is due. Neither is a change to the example on disk.
fn stage_streamflow() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    copy_dir(&repo_root().join("examples/streamflow"), root);

    // Opt in to the write tier. Expressed as the file a repository would commit, because that
    // is the whole of what the declaration is.
    std::fs::write(root.join(".yidam/config.toml"), "[serve]\nact = true\n").unwrap();

    expire_the_catalog_entry(root);

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.co"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "genesis: streamflow"]);
    tmp
}

/// Put the example's one catalog source past its TTL, in its own front matter.
///
/// `retrieved: 2020-01-01` with `ttl_days: 1` is expired on any day this runs and will stay
/// expired — the device `corpus-dated/` uses with nineteenth-century dates, for the reason
/// `lint::today_iso` gives: a wall-clock feature whose own tests depend on the day they run is
/// the failure the whole date-passing arrangement exists to prevent.
fn expire_the_catalog_entry(root: &Path) {
    let entry = root.join(".yidam/catalog/usgs-nwis.md");
    let text = std::fs::read_to_string(&entry).expect("streamflow's catalog entry");
    let (_, rest) = text
        .split_once("---\n")
        .expect("the entry opens with front matter");
    let (front, body) = rest
        .split_once("\n---")
        .expect("the entry closes its front matter");
    let front: String = front
        .lines()
        .filter(|l| !l.starts_with("retrieved:") && !l.starts_with("ttl_days:"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        &entry,
        format!("---\n{front}\nretrieved: 2020-01-01\nttl_days: 1\n---{body}"),
    )
    .unwrap();
}

struct McpClient {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args(["serve", "--mcp"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawning yidam serve --mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut client = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        client.initialize();
        client
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let resp: Value = serde_json::from_str(&line).unwrap_or_else(|e| {
            panic!("{method} answered something that is not JSON ({e}): {line}")
        });
        assert!(resp.get("error").is_none(), "{method}: {resp}");
        resp["result"].clone()
    }

    fn initialize(&mut self) -> Value {
        let init = self.request("initialize", json!({"protocolVersion": "2024-11-05"}));
        init["capabilities"]["yidam"].clone()
    }

    fn capabilities(&mut self) -> Value {
        self.initialize()
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    fn tool_json(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.call(name, arguments);
        assert!(
            result["isError"].as_bool() != Some(true),
            "{name} errored: {}",
            result["content"][0]["text"].as_str().unwrap_or_default()
        );
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn listed(&mut self) -> Vec<String> {
        self.request("tools/list", json!({}))["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── the demonstration ─────────────────────────────────────────────────────────

/// #474's definition of done, narrowed to the one clock a tool can discharge, run end to end.
///
/// Read `cycle` and see the clock is owed; call `propose` and discharge it; read the receipt.
/// Every step is a tool call on one connection, which is the thing that did not exist: before
/// this, an agent could be told a source had expired and could not do the one thing that
/// discharges it.
#[test]
fn an_agent_reads_a_clock_discharges_it_and_reads_the_receipt() {
    let repo = stage_streamflow();
    let root = repo.path();
    let mut client = McpClient::spawn(root);

    // The tier is declared, and both its tools are listed.
    let capabilities = client.capabilities();
    assert_eq!(capabilities["act"], true, "{capabilities}");
    let listed = client.listed();
    assert!(listed.contains(&"propose".to_string()), "{listed:?}");
    assert!(listed.contains(&"cycle".to_string()), "{listed:?}");

    // 1 — read. The catalog clock is owed, and the report names what discharges it.
    let cycle = client.tool_json("cycle", json!({}));
    let catalog = cycle["owed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "catalog")
        .expect("`cycle` reports the four clocks by id");
    assert_eq!(catalog["state"], "due", "{catalog}");
    assert_eq!(cycle["passed"], true, "being owed is not a failure");
    let act = cycle["next"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["source"] == "owed")
        .expect("an owed clock names an act");
    assert!(
        act["act"].as_str().unwrap().contains("propose"),
        "the act that discharges the catalog clock is `propose`: {act}"
    );

    // 2 — act. The tool the report just named, called over the same connection.
    let proposed = client.tool_json("propose", json!({}));
    let drafted = proposed["proposals"].as_array().unwrap();
    assert_eq!(drafted.len(), 1, "{proposed}");
    assert_eq!(drafted[0]["verb"], "open");
    assert_eq!(drafted[0]["check"], "catalog-expired");
    let branch = proposed["branch"].as_str().unwrap().to_string();
    assert!(branch.starts_with("propose/"), "{branch}");

    // 3 — the receipt, and it is a fact about git rather than about the report. `written`
    // carries the branch and the shas; the commits are on the ref, and the baseline is
    // untouched.
    let written = &proposed["written"];
    assert!(!written.is_null(), "a non-dry run writes: {proposed}");
    assert_eq!(written["branch"], branch);
    let commits = written["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 1, "{written}");

    let on_branch = git_out(root, &["rev-list", "--count", &branch, "^main"]);
    assert_eq!(
        on_branch.trim(),
        "1",
        "the commit is ahead of the baseline and not on it"
    );
    let subject = git_out(root, &["log", "-1", "--format=%s", &branch]);
    assert!(
        subject.starts_with("open:"),
        "the drafted verb is `open`, and the commit carries it: {subject}"
    );
    assert_eq!(
        git_out(root, &["rev-list", "--count", "main"]).trim(),
        "1",
        "main is still the genesis commit alone"
    );

    // 4 — and the server can see what it wrote. This is the half a snapshot taken only at
    // startup could not answer: before the reload, the connection that performed the write
    // kept reporting the clock as owed, so the loop did not close within a session.
    let after = client.tool_json("cycle", json!({}));
    assert_eq!(
        after["passed"], true,
        "a discharged clock is still not a failure"
    );
    let head_after = client.capabilities();
    assert_eq!(
        head_after["corpus"]["commit"], capabilities["corpus"]["commit"],
        "the proposal is on a branch, so HEAD did not move — the reload is not a checkout"
    );
}

/// The author/committer split RFC-0020 and RFC-0026 specify, on a commit a tool wrote over MCP.
///
/// *The tool drafted and a person ran it*, and git has two fields for exactly that. The
/// **author** is the tool — a proposal is not an elector's position and must not borrow one's
/// name — and the **committer** is whoever ran it. RFC-0029 §2.2 rests on this split: the
/// identity gate exists so that the committer field has something true to carry, and a commit
/// recording only what a process did is the thing it refuses to allow.
///
/// **Asserted in both directions.** Either half alone passes on a server that stamped one
/// identity into both fields, which would lose the distinction while looking correct from
/// whichever side the test happened to read.
#[test]
fn the_commit_a_tool_wrote_records_both_the_tool_and_the_person() {
    let repo = stage_streamflow();
    let root = repo.path();
    let mut client = McpClient::spawn(root);
    let proposed = client.tool_json("propose", json!({}));
    let branch = proposed["branch"].as_str().unwrap().to_string();

    let author = git_out(root, &["log", "-1", "--format=%ae", &branch]);
    let committer = git_out(root, &["log", "-1", "--format=%ce", &branch]);
    assert_eq!(
        author.trim(),
        "propose@yidam",
        "the tool drafted it, and says so rather than signing as a person"
    );
    assert_eq!(
        committer.trim(),
        "t@t.co",
        "and a person ran it — the repository's own configured identity, which is what the \
         `act` gate requires to exist before this call is served at all"
    );
    assert_ne!(
        author.trim(),
        committer.trim(),
        "one identity in both fields is the split collapsed, and it reads as correct from \
         either side alone"
    );
}

/// What the reload does and does not change, pinned — because today it changes nothing.
///
/// # The finding this test exists to carry
///
/// RFC-0029's snapshot question was decided as *a write invalidates and reloads*, on the
/// premise that *"after an `act` call writes a `propose/*` branch, every read tool on that
/// connection keeps answering from the pre-write corpus."* The first half of that is true and
/// the consequence does not follow, for the reason `propose/write.rs` states about itself:
/// the commits are built against a **temporary index** and the ref is created with
/// `update-ref`, so the working tree is untouched, HEAD does not move, and *"a run changes
/// nothing a person can see and is safe mid-edit."*
///
/// Everything `ServerState::load` reads — the corpus walk, the class files, the query graph,
/// the index metadata, `provenance.commit` — is identical before and after. **So the reload
/// is a correct no-op for the only write the tier has**, and no response observably differs.
///
/// The mechanism is implemented anyway and is the right shape: it costs a corpus re-walk on a
/// call that already wrote to git, and it becomes load-bearing the first time an act tool
/// touches the working tree — at which point that tool is not also a contract change. What it
/// must not do is buy a weakened promise for a change nobody can see, which is why the
/// contract states both halves.
///
/// **This assertion is the falsifier.** If a future write does change what a read tool
/// answers, this goes red, and that is the signal that the amendment has started earning its
/// keep rather than merely being on the record.
#[test]
fn a_write_changes_nothing_a_read_tool_answers_today() {
    let repo = stage_streamflow();
    let root = repo.path();
    let mut client = McpClient::spawn(root);
    let handshake = client.capabilities();

    let reads = [
        ("list_nodes", json!({})),
        ("open_questions", json!({})),
        ("claims", json!({})),
        ("get_node", json!({"id": "reach/tailwater"})),
        ("neighbors", json!({"id": "reach/tailwater"})),
    ];
    let before: Vec<Value> = reads
        .iter()
        .map(|(n, a)| client.tool_json(n, a.clone()))
        .collect();

    let proposed = client.tool_json("propose", json!({}));
    assert!(
        !proposed["written"].is_null(),
        "the write must actually have happened, or this asserts nothing: {proposed}"
    );

    for ((name, args), was) in reads.iter().zip(&before) {
        let now = client.tool_json(name, args.clone());
        assert_eq!(
            &now, was,
            "`{name}` answered differently after a `propose` — the proposal is a branch built \
             against a temporary index, so nothing a read tool reads should have moved. If \
             this is a new act tool that writes into the tree, the reload has stopped being a \
             no-op and the contract's amendment has started meaning something."
        );
    }

    // And the handshake agrees: the same corpus, the same node count, the same commit. This
    // is the block a client caches, and the one the amendment is about.
    assert_eq!(
        client.capabilities()["corpus"],
        handshake["corpus"],
        "the handshake's corpus block moved across a write that touches no file"
    );
}

// ── the invariant ─────────────────────────────────────────────────────────────

/// RFC-0026's invariant, over MCP — #474's definition of done.
///
/// *A run authors operational commits directly; every epistemic commit it produces goes to a
/// proposal branch, and nothing merges itself.* Every commit **the call produced** is
/// classified by its leading verb, and every epistemic one must be on a `propose/*` ref. The
/// classification is the vocabulary's own: operational verbs are the explicitly marked case
/// and everything else is epistemic, which is what makes the check total rather than a list of
/// things to look for.
///
/// # The invariant is about what a run wrote, and the first version of this test was not
///
/// Scoping it to every commit in the repository looks stricter and is simply a different
/// claim — one that is false of every corpus that exists. `genesis:` is an epistemic verb and
/// sits on `main` in every repository the bootstrap ever created, so the unscoped form failed
/// on a fixture nothing had written to yet. What the invariant governs is the delta: the
/// commits that were not there before the call and are there after.
///
/// **Read off git rather than off the report.** A report is what the tool says it did; the
/// refs are what it did. A tool that wrote an `establish:` onto `main` and reported a clean
/// proposal would pass every assertion made against its own output.
#[test]
fn an_agent_triggered_write_puts_no_epistemic_commit_outside_propose() {
    let repo = stage_streamflow();
    let root = repo.path();

    let before = commits_by_ref(root);
    let mut client = McpClient::spawn(root);
    client.tool_json("propose", json!({}));
    // A second call, because the first left a branch behind and the interesting question is
    // what a server does when asked again — it must not write a second time onto anything.
    let _ = client.call("propose", json!({}));
    let after = commits_by_ref(root);

    assert!(
        after.keys().any(|r| r.starts_with("propose/")),
        "the write produced no proposal ref at all: {:?}",
        after.keys().collect::<Vec<_>>()
    );
    assert!(
        after.len() > before.len(),
        "nothing was written, so this test asserted the invariant over an empty set — \
         before: {:?}, after: {:?}",
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );

    let known: std::collections::BTreeSet<&String> =
        before.values().flatten().map(|(hash, _)| hash).collect();
    let mut checked = 0usize;
    for (r, commits) in &after {
        for (hash, subject) in commits {
            if known.contains(hash) {
                continue; // it was here before the call; the invariant is about what a run wrote
            }
            checked += 1;
            if !is_epistemic(subject) {
                continue;
            }
            assert!(
                r.starts_with("propose/"),
                "epistemic commit {hash} (`{subject}`) was written to `{r}` — a run authors \
                 operational commits directly, and every epistemic one goes to a proposal \
                 branch (RFC-0026). This is the invariant #474 asks to hold over MCP exactly \
                 as over the CLI."
            );
        }
    }
    assert!(
        checked > 0,
        "the call wrote no new commit, so this asserted nothing"
    );
}

/// Every ref, and the `(hash, subject)` of every commit reachable from it.
///
/// Taken before and after the call so the invariant is asserted over the delta. Keyed by ref
/// because *where* a commit landed is half of what is being checked.
fn commits_by_ref(root: &Path) -> std::collections::BTreeMap<String, Vec<(String, String)>> {
    git_out(root, &["for-each-ref", "--format=%(refname:short)"])
        .lines()
        .map(|r| {
            let log = git_out(root, &["log", "--format=%H %s", r]);
            let commits = log
                .lines()
                .filter_map(|l| l.split_once(' '))
                .map(|(h, s)| (h.to_string(), s.to_string()))
                .collect();
            (r.to_string(), commits)
        })
        .collect()
}

/// Whether a commit subject is an epistemic event, by the vocabulary's own rule.
///
/// **Operational is the marked case and everything else is epistemic**, which is the totality
/// the Dafny spec proves and the reason this can be a check rather than a search. A
/// git-generated merge subject carries no verb at all and is exempt — it is not something a
/// tool authored, and the vocabulary says so.
///
/// The list is read out of the shipped SDK rather than restated, so a verb moving between the
/// two sets cannot leave this test asserting yesterday's rule.
fn is_epistemic(subject: &str) -> bool {
    if subject.starts_with("Merge ") {
        return false;
    }
    let Some(verb) = subject.split([':', '(']).next() else {
        return false;
    };
    let verb = verb.trim();
    if verb == subject {
        // No `verb:` at all. `genesis:` and the rest all carry one; a subject without is
        // outside the vocabulary and is not something this invariant governs.
        return false;
    }
    !operational_verbs().iter().any(|v| v == verb)
}

/// `OPERATIONAL_VERBS`, read from the shipped Rust SDK.
///
/// Parsed from the source rather than linked, because this test binary depends on the CLI and
/// not on the SDK crate. Discovered rather than listed for the reason the reference-page guard
/// gives: a roster written here would stop covering a verb the vocabulary gained without ever
/// going red.
fn operational_verbs() -> Vec<String> {
    let src = std::fs::read_to_string(repo_root().join("yidam/prelude/sdks/rust/src/git.rs"))
        .expect("the shipped Rust SDK");
    let (_, rest) = src
        .split_once("pub const OPERATIONAL_VERBS: &[&str] = &[")
        .expect("OPERATIONAL_VERBS is declared there");
    let (list, _) = rest.split_once("];").expect("and is closed");
    let verbs: Vec<String> = list
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    assert!(
        verbs.len() > 10 && verbs.iter().any(|v| v == "regen"),
        "the SDK's operational verb list did not parse — this check is reading nothing: \
         {verbs:?}"
    );
    verbs
}

// ── the declaration, and what refuses it ──────────────────────────────────────

/// A corpus that did not ask serves the thirteen read tools, and refuses these two by name.
///
/// The refusal token is frozen and is not `unknown tool`: the two are different repairs, and a
/// caller told the second goes hunting for a spelling mistake in a name the contract froze.
#[test]
fn a_corpus_that_did_not_ask_refuses_the_act_tools() {
    let repo = stage_streamflow();
    let root = repo.path();
    // The one difference from every test above.
    std::fs::remove_file(root.join(".yidam/config.toml")).unwrap();
    git(root, &["add", "-A"]);
    git(
        root,
        &["commit", "-q", "-m", "decide: this corpus does not write"],
    );

    let mut client = McpClient::spawn(root);
    let capabilities = client.capabilities();
    assert_eq!(capabilities["act"], false, "{capabilities}");

    let listed = client.listed();
    for name in ["propose", "cycle"] {
        assert!(
            !listed.contains(&name.to_string()),
            "`{name}` is listed on a server that declares `act: false`: {listed:?}"
        );
        let result = client.call(name, json!({}));
        assert_eq!(result["isError"], true, "{result}");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.starts_with("capability-not-supported"),
            "the refusal token is frozen: {text}"
        );
        assert!(text.contains("`act`"), "and it names the tier: {text}");
    }

    // And nothing was written by the attempt.
    let refs = git_out(root, &["for-each-ref", "--format=%(refname:short)"]);
    assert!(
        !refs.contains("propose/"),
        "a refused call wrote a branch anyway: {refs}"
    );
}

/// Clause 1 — a checkout with no git author identity cannot declare `act`, and the server says
/// so instead of starting.
///
/// **A refusal and not a downgrade**, which is the part worth a test of its own. Serving the
/// read tools and declaring `act: false` would leave an operator who wrote the key reading a
/// running server as the answer to the question they asked.
///
/// # Removing an identity takes three things, and the first two are not enough
///
/// Clearing the repository's own `user.email` proves nothing: git falls back to the global
/// config, and a developer machine has one — so the first version of this test started a
/// server happily and would have gone green on every laptop while asserting nothing.
/// `user.useConfigOnly` does not close it either: it disables *auto-detection* from the
/// system, not the global file, so `git var GIT_AUTHOR_IDENT` still answered with the
/// developer's own address.
///
/// What is actually needed is all three at once — the local keys unset, the global and system
/// files pointed at `/dev/null`, and `useConfigOnly` to stop git guessing from the hostname.
/// This is the check-under-the-varying-condition shape: run under the default and it proves
/// nothing about the configuration that differs, and the failure is silent either way.
#[test]
fn a_checkout_with_no_author_refuses_to_declare_act() {
    let repo = stage_streamflow();
    let root = repo.path();
    git(root, &["config", "--local", "--unset-all", "user.email"]);
    git(root, &["config", "--local", "--unset-all", "user.name"]);
    git(root, &["config", "--local", "user.useConfigOnly", "true"]);

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["serve", "--mcp"])
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("EMAIL")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "a server told to write with no author to write as must not start"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no git author identity"),
        "the refusal must say which clause failed: {err}"
    );
}

/// Clause 3 — an `act`-declaring server may not bind an address another machine can reach.
///
/// Checked before the socket is bound, so a server that will not be allowed to write never
/// holds the port. `--port 0` because nothing should be listening either way, and a fixed
/// port would make this test fight whatever else is running.
///
/// **Bounded rather than waited on.** The thing being asserted is that the process *exits*,
/// and the failure this catches is a server that binds and serves — which never exits, so a
/// plain `output()` hangs until CI kills the job with no message. Mutating the clause to
/// `if false` is what showed that: the guard was load-bearing and its failure was a timeout.
/// A bounded wait turns it back into an assertion with a sentence on it.
#[test]
#[cfg(feature = "serve-http")]
fn an_act_declaring_server_refuses_a_non_loopback_bind() {
    let repo = stage_streamflow();
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args([
            "serve", "--mcp", "--http", "--bind", "0.0.0.0", "--port", "0",
        ])
        .current_dir(repo.path())
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Generous for a process whose whole job here is to read one config file and refuse.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        match child.try_wait().unwrap() {
            Some(status) => break Some(status),
            None if std::time::Instant::now() >= deadline => break None,
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the server was still running after 20s with `act` declared and `--bind 0.0.0.0` \
             — it bound a socket another machine can reach and started serving a write tier"
        );
    };
    assert!(!status.success(), "it exited zero, so it did not refuse");

    let mut err = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        use std::io::Read as _;
        let _ = stderr.read_to_string(&mut err);
    }
    assert!(
        err.contains("0.0.0.0") && err.contains("act"),
        "the refusal must name the bind and the declaration: {err}"
    );
}

/// The frame `yidam-edit` sends — RFC-0030 Phase 3, #608.
///
/// The web editor reaches the act tier by spawning `yidam serve --mcp` once per request and
/// writing three lines at once — `initialize`, `notifications/initialized`, one `tools/call`
/// — then closing stdin, and reading everything back after the process exits. That is not how
/// [`McpClient`] speaks (one line, one answer, in turn), and nothing above would notice if
/// the server started needing the conversation to be interactive: a server that waited for
/// `initialized` to be acknowledged, or that flushed only on exit, or that wrote its banner
/// to stdout, would break the editor and pass every test here. This holds the three facts
/// the editor's `src/lib/act.ts` depends on: every stdout line is JSON-RPC, both ids come
/// back, and EOF on stdin is a clean exit.
#[test]
fn a_one_shot_connection_answers_both_ids_and_exits_at_eof() {
    let repo = stage_streamflow();
    let frame = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2024-11-05", "clientInfo": {"name": "yidam-edit"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
               "params": {"name": "propose", "arguments": {"dry_run": true}}}),
    ]
    .iter()
    .map(|m| format!("{m}\n"))
    .collect::<String>();

    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["serve", "--mcp"])
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        // All of it, then EOF — the editor never reads before it has finished writing.
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(frame.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "EOF on stdin is not a clean exit: {stderr}"
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut by_id = std::collections::BTreeMap::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let msg: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout carried a line that is not JSON-RPC ({e}): {line}"));
        if let Some(id) = msg.get("id").and_then(Value::as_u64) {
            by_id.insert(id, msg);
        }
    }
    let init = by_id.get(&1).expect("no answer to initialize");
    assert_eq!(init["result"]["capabilities"]["yidam"]["act"], json!(true));
    let call = by_id.get(&2).expect("no answer to tools/call");
    assert!(
        call.get("error").is_none(),
        "the call was not understood: {call}"
    );
    assert_ne!(call["result"]["isError"], json!(true), "{call}");
    let text = call["result"]["content"][0]["text"].as_str().unwrap();
    let report: Value = serde_json::from_str(text).expect("the tool's text is its JSON report");
    assert_eq!(report["written"], Value::Null, "dry_run wrote a branch");
    assert_eq!(
        git_out(repo.path(), &["branch", "--list", "propose/*"]).trim(),
        "",
        "a dry run left a propose/ branch behind"
    );
}
