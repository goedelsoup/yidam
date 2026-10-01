use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct YidamConfig {
    /// Read only by `index-build`, which the light `reports` binary does not carry. The
    /// field is still parsed there so a config naming a model is not rejected by a binary
    /// that simply cannot act on it.
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub propose: ProposeConfig,
    #[serde(default)]
    pub catalog: CatalogConfig,
    #[serde(default)]
    pub due: DueConfig,
    #[serde(default)]
    pub object: ObjectConfig,
    #[serde(default)]
    pub derive: DeriveConfig,
    #[serde(default)]
    pub serve: ServeConfig,
    /// The stores this corpus keeps artifacts in, by name.
    ///
    /// Plural before it needs to be, and the reasoning is
    /// [`crate::vault::resolve`]'s: `[vault]` and `[vault.default]` are different config
    /// shapes, this file is committed, and a corpus that wrote the singular form is one a
    /// later release breaks. Exactly one entry is honoured today and a second is refused
    /// rather than resolved to the first.
    ///
    /// Never a credential. Those come from the environment, because this file is committed
    /// and a repository has already been found carrying an untracked `.env` that its own
    /// prescribed `git add -A` would have staged.
    #[serde(default)]
    pub vault: BTreeMap<String, crate::vault::VaultConfig>,
    /// Where this corpus's runs execute when they do not execute here (RFC-0026 §7).
    ///
    /// Everything in it is about **whether and where** a run happens — the remote, the
    /// image, the branch, the cap on open proposals. Nothing in it is about what a run may
    /// author, and `deny_unknown_fields` is what makes an attempt to add such a key a parse
    /// error rather than a line somebody reads as effective. §3's distinction, kept where it
    /// can be checked: *policy decides whether a run happens; it does not decide what a run
    /// may say.*
    #[serde(default)]
    pub cluster: ClusterConfig,
}

/// The `[cluster]` section: the execution plane a run is generated for.
///
/// No default for `remote` or `image`, because a value compiled into the binary would be one
/// corpus's answer imposed on every other — `config.rs:52`'s argument, which is the same one
/// that leaves `max_open_proposals` unset until a corpus sets it. `branch` and `vault` have
/// defaults because they name conventions this repository already holds elsewhere: `main` is
/// what `clone` leaves a derived repository on, and `default` is the vault name whose
/// credentials the ambient `AWS_*` variables are honoured for.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterConfig {
    /// The git remote the lander writes and the pin reads. An SSH or HTTPS URL; what
    /// credential each pod holds for it is the manifest's concern, never this file's.
    #[serde(default)]
    pub remote: Option<String>,
    /// The branch a run advances. Operational commits land here; epistemic ones land on
    /// `propose/<input>` and this branch does not move.
    #[serde(default = "default_branch")]
    pub branch: String,
    /// The container image every pod runs — a build of this CLI with `git` and `sh` beside it.
    #[serde(default)]
    pub image: Option<String>,
    /// Which `[vault.<name>]` carries bundles between pods.
    #[serde(default = "default_vault")]
    pub vault: String,
    /// The Kubernetes namespace the generated manifest names. Unset leaves it to `kubectl`.
    #[serde(default)]
    pub namespace: Option<String>,
    /// How many `propose/*` branches may stand open before admission refuses to submit a
    /// workflow (#460 decision 8). Unset means the cap is undeclared, and `admit` says so.
    #[serde(default)]
    pub max_open_proposals: Option<usize>,
    /// The Kubernetes objects the manifest names. Each defaults to one derived from this
    /// corpus, so two corpora in one namespace share no credential (#1228).
    #[serde(default)]
    pub names: ClusterNamesConfig,
    /// What every pod requests, may use and may run for, unless its capability says otherwise
    /// in `.yidam/capabilities.toml` (#1231). Unset keys take the compiled-in sizes in
    /// `cmd/cluster/workflow.rs`, so a namespace with a `ResourceQuota` admits every pod.
    #[serde(default)]
    pub pod: ClusterPodConfig,
    /// How long a finished run's pods and workflow stay on the cluster (#1231).
    #[serde(default)]
    pub cleanup: ClusterCleanupConfig,
    /// What a pod may reach, as `yidam cluster network-policy` writes it (#1232). A
    /// NetworkPolicy matches addresses, not hostnames, so each is a list of CIDRs.
    #[serde(default)]
    pub egress: ClusterEgressConfig,
    /// How `land` authenticates its push (#1233). `deploy-key`, the default, is an SSH key in
    /// the `git_write` secret. `github-app` is a GitHub App's private key there instead, from
    /// which the lander mints an installation token that expires in an hour.
    #[serde(default)]
    pub git_auth: GitAuth,
    /// The App `git_auth = "github-app"` mints from. Refused under any other `git_auth`.
    #[serde(default)]
    pub github_app: Option<ClusterGithubAppConfig>,
}

/// The lander's push credential (#1233).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum GitAuth {
    /// An SSH deploy key, which has no expiry.
    #[default]
    DeployKey,
    /// A GitHub App installation token, minted in the lander pod, which expires in an hour.
    GithubApp,
}

impl GitAuth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeployKey => "deploy-key",
            Self::GithubApp => "github-app",
        }
    }
}

/// `[cluster.github_app]`: the GitHub App whose private key is in the `git_write` secret.
///
/// No installation id: the lander asks GitHub for the installation on the remote's
/// repository, so this table holds nothing that changes when the App is reinstalled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterGithubAppConfig {
    /// The App's id, from its settings page. Not a secret: it is in every JWT the App signs.
    pub app_id: u64,
    /// GitHub's REST API. Unset is github.com's; GitHub Enterprise Server's is
    /// `https://<host>/api/v3`.
    #[serde(default)]
    pub api_url: Option<String>,
}

/// `[cluster.names]`: an override for each object a generated workflow refers to by name.
///
/// Unset is the per-corpus default, `yidam-<corpus>-<role>`. There is no shared default to
/// fall back to: a constant name here is one write key for every corpus in the namespace,
/// which is the one-credential invariant (#460 decision 7) undone by a `kubectl` habit.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterNamesConfig {
    /// The service account the pods and the executor run as.
    #[serde(default)]
    pub service_account: Option<String>,
    /// The secret `pin` and `admit` mount to read the remote.
    #[serde(default)]
    pub git_read: Option<String>,
    /// The secret `land` mounts to move a ref: the only credential that can.
    #[serde(default)]
    pub git_write: Option<String>,
    /// The secret every pod reads vault credentials from, for an `s3://` vault.
    #[serde(default)]
    pub vault_secret: Option<String>,
    /// The `PersistentVolumeClaim` a `file://` vault is mounted from.
    #[serde(default)]
    pub vault_claim: Option<String>,
    /// The service account the `--on-push` Sensor creates workflows as.
    #[serde(default)]
    pub events_account: Option<String>,
    /// The secret an `--on-push` EventSource checks each push against: GitHub's webhook
    /// secret, or a generic webhook's bearer token.
    #[serde(default)]
    pub webhook_secret: Option<String>,
}

/// `[cluster.pod]`, and the same keys under `[capability.<name>.cluster]`: one pod's bounds.
///
/// Each key is optional at both levels. A capability's key wins over `[cluster.pod]`'s, which
/// wins over the compiled-in size, key by key — so a calculator that needs more memory says so
/// and keeps the corpus's CPU request. A quantity is Kubernetes's own spelling (`"250m"`,
/// `"2Gi"`), checked when the manifest is generated rather than when the cluster refuses it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterPodConfig {
    /// The CPU the scheduler reserves for the pod.
    #[serde(default)]
    pub cpu_request: Option<String>,
    /// The memory the scheduler reserves for the pod.
    #[serde(default)]
    pub memory_request: Option<String>,
    /// The CPU the pod is throttled at.
    #[serde(default)]
    pub cpu_limit: Option<String>,
    /// The memory the pod is killed at.
    #[serde(default)]
    pub memory_limit: Option<String>,
    /// How long the pod may run before Argo fails it. A hung calculator is stopped here,
    /// rather than holding the corpus's only run slot until someone deletes it.
    #[serde(default)]
    pub deadline_seconds: Option<u64>,
}

impl ClusterPodConfig {
    /// Every key set is one Kubernetes would accept, or an error naming `table` and the key.
    pub fn check(&self, table: &str) -> Result<()> {
        for (key, value) in [
            ("cpu_request", &self.cpu_request),
            ("memory_request", &self.memory_request),
            ("cpu_limit", &self.cpu_limit),
            ("memory_limit", &self.memory_limit),
        ] {
            if let Some(v) = value {
                if !is_quantity(v) {
                    anyhow::bail!(
                        "{table} {key} = {v:?} is not a Kubernetes quantity: a number, then \
                         optionally one of m, k, M, G, T, P, E, Ki, Mi, Gi, Ti, Pi, Ei"
                    );
                }
            }
        }
        if self.deadline_seconds == Some(0) {
            anyhow::bail!("{table} deadline_seconds = 0 would fail every pod before it starts");
        }
        Ok(())
    }
}

/// A Kubernetes resource quantity in the forms a person writes: `"500m"`, `"2"`, `"1.5Gi"`.
///
/// Narrower than the API's grammar, which also takes a sign and an exponent (`"1e3"`). Neither
/// is a size anyone sets for a pod, and a value refused here is one the corpus can respell; a
/// value passed here that the cluster refuses is a run that never starts.
fn is_quantity(s: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "m", "k", "M", "G", "T", "P", "E", "",
    ];
    let digits = s.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &s[digits.len()..];
    let mut parts = digits.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    SUFFIXES.contains(&suffix)
        && !whole.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()))
}

/// `[cluster.cleanup]`: when Argo deletes a finished run's pods and the workflow itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterCleanupConfig {
    /// Argo's `podGC.strategy`: `OnPodSuccess`, `OnPodCompletion`, `OnWorkflowSuccess` or
    /// `OnWorkflowCompletion`.
    #[serde(default)]
    pub pod_gc: Option<String>,
    /// How long a workflow that succeeded is kept, in seconds.
    #[serde(default)]
    pub seconds_after_success: Option<u64>,
    /// How long a workflow that failed is kept, in seconds — longer, for whoever reads its logs.
    #[serde(default)]
    pub seconds_after_failure: Option<u64>,
}

impl ClusterCleanupConfig {
    /// The strategies Argo's `podGC` takes. A misspelt one is refused by the cluster when the
    /// workflow is submitted, which for a `CronWorkflow` is the next morning.
    pub const POD_GC: &[&str] = &[
        "OnPodSuccess",
        "OnPodCompletion",
        "OnWorkflowSuccess",
        "OnWorkflowCompletion",
    ];

    pub fn check(&self) -> Result<()> {
        if let Some(gc) = &self.pod_gc {
            if !Self::POD_GC.contains(&gc.as_str()) {
                anyhow::bail!(
                    "[cluster.cleanup] pod_gc = {gc:?} is not an Argo podGC strategy: one of {}",
                    Self::POD_GC.join(", ")
                );
            }
        }
        Ok(())
    }
}

/// `[cluster.egress]`: the addresses a generated NetworkPolicy lets a pod reach (#1232).
///
/// The hostnames a pod needs, given as CIDRs, because a NetworkPolicy cannot name a host.
/// What each pod may reach is the generator's (`cmd/cluster/egress.rs`); this says only where
/// those things are on this cluster's network.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterEgressConfig {
    /// Where Argo's executor sidecar reaches the Kubernetes API from inside every pod: the
    /// API server's endpoint addresses, after DNAT, not the `kubernetes` service's cluster IP.
    /// Required, since without it no pod can report its output.
    #[serde(default)]
    pub executor: Vec<String>,
    /// Where the git remote is. Optional: unset, the pods that hold a git credential reach
    /// everywhere, and the pods that need the internet can reach the remote too.
    #[serde(default)]
    pub remote: Vec<String>,
    /// Where an `s3://` vault's endpoint is. Required for one, refused for a `file://` vault,
    /// which is a volume and reaches no network.
    #[serde(default)]
    pub vault: Vec<String>,
    /// Where STS is, for an `s3://` vault whose pods assume a role by web identity (#1234):
    /// the regional STS interface endpoint's addresses. Optional, since keys need no STS, and
    /// refused for a `file://` vault, like `vault`.
    #[serde(default)]
    pub sts: Vec<String>,
}

impl ClusterEgressConfig {
    /// Every address is a CIDR Kubernetes would accept, or an error naming the key.
    pub fn check(&self) -> Result<()> {
        for (key, list) in [
            ("executor", &self.executor),
            ("remote", &self.remote),
            ("vault", &self.vault),
            ("sts", &self.sts),
        ] {
            for cidr in list {
                if let Err(why) = parse_cidr(cidr) {
                    anyhow::bail!("[cluster.egress] {key} holds {cidr:?}, which {why}");
                }
            }
        }
        Ok(())
    }
}

/// A CIDR's address and prefix length, or why it is not one Kubernetes accepts as an `ipBlock`.
///
/// Host bits must be zero. Kubernetes warns on `10.0.0.1/8` and reads it as `10.0.0.0/8`, so
/// a corpus that wrote the first meant something narrower than it gets.
pub fn parse_cidr(cidr: &str) -> std::result::Result<(std::net::IpAddr, u8), String> {
    let (addr, len) = cidr
        .split_once('/')
        .ok_or("has no `/<prefix length>`".to_string())?;
    let addr: std::net::IpAddr = addr
        .parse()
        .map_err(|_| "does not begin with an IP address".to_string())?;
    let max = if addr.is_ipv4() { 32 } else { 128 };
    let len: u8 = len
        .parse()
        .ok()
        .filter(|l| *l <= max)
        .ok_or(format!("has a prefix length that is not 0 to {max}"))?;
    let bits: u128 = match addr {
        std::net::IpAddr::V4(a) => u32::from(a).into(),
        std::net::IpAddr::V6(a) => a.into(),
    };
    let host = u32::from(max - len);
    let mask = 1u128.checked_shl(host).map_or(u128::MAX, |b| b - 1);
    if bits & mask != 0 {
        return Err("sets bits past its prefix length; write the network address".to_string());
    }
    Ok((addr, len))
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            remote: None,
            branch: default_branch(),
            image: None,
            vault: default_vault(),
            namespace: None,
            max_open_proposals: None,
            names: ClusterNamesConfig::default(),
            pod: ClusterPodConfig::default(),
            cleanup: ClusterCleanupConfig::default(),
            egress: ClusterEgressConfig::default(),
            git_auth: GitAuth::default(),
            github_app: None,
        }
    }
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_vault() -> String {
    "default".to_string()
}

#[derive(Debug, Default, Deserialize)]
pub struct IndexConfig {
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    pub model: Option<String>,
    /// The vector index this corpus publishes to and can be queried out of (RFC-0033).
    ///
    /// **Not feature-gated, and for the reason the `[lint]` note above gives about its own
    /// field**: the section is a corpus's declaration about itself, it lives in a committed
    /// file, and a build that cannot act on it should still be able to read it and say so.
    /// `doctor` reports a configured remote index in every build; only querying one needs
    /// `vector-read`.
    #[serde(default)]
    pub remote: Option<crate::s3vectors::RemoteIndexConfig>,
}

/// What this corpus has decided about its own gate.
#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// Corpus-touching commits a dated finding may hold before it escalates to an error.
    ///
    /// Absent means no finding ever escalates, and that is the right default rather than a
    /// timid one. The number is a judgement about how fast *this* corpus is meant to
    /// consume what it collects — a breadth sweep landing twelve nodes it will link over
    /// the next eighty commits is healthy in one repository and over-collection in another
    /// — so a value compiled into the binary would be one corpus's answer imposed on every
    /// other, arriving as a build failure in a repository that never agreed to it.
    ///
    /// Declared here so the argument for the number lives in the repository that has to
    /// live with it:
    ///
    /// ```toml
    /// [lint]
    /// escalate_after = 100
    /// ```
    pub escalate_after: Option<usize>,
}

/// What this corpus has licensed `yidam propose` to draft.
///
/// Empty by default, and the default is the design rather than caution. `propose` drafts a
/// question from any finding, because recording a question asserts nothing the finding did
/// not already assert. Drafting a *deletion* asserts that the node should go, and no finding
/// says that — so it is licensed only by a corpus that says so here, about itself.
#[derive(Debug, Default, Deserialize)]
pub struct ProposeConfig {
    /// Corpus-touching commits an uncited node may hold before `propose` drafts its
    /// withdrawal.
    ///
    /// Absent means no withdrawal is ever drafted, which is every corpus until someone turns
    /// it on. The reasoning is [`LintConfig::escalate_after`]'s and is not repeated: a number
    /// compiled into the binary would be one repository's judgement arriving as a proposed
    /// deletion in another that never agreed to it.
    ///
    /// ```toml
    /// [propose]
    /// withdraw_uncited_after = 400
    /// ```
    ///
    /// **Not `escalate_after` under another name.** That declares when a finding becomes a
    /// build failure, which is a statement about the gate. This declares when an uncited node
    /// stops being a sweep in progress and becomes over-collection, which is a statement
    /// about the corpus. A repository may reasonably hold the first and not the second, and
    /// most will: failing the build asks a person to look, and deleting the node decides what
    /// they would have concluded.
    pub withdraw_uncited_after: Option<usize>,
}

/// What this corpus has decided about how its sources age.
///
/// Not `.yidam.toml`. That file is the *template provenance pin* — `origin`, `commit`,
/// `template`, `committed` — and records which yidam governs a corpus. This one records what
/// the corpus decided about itself, which is where `escalate_after` and
/// `withdraw_uncited_after` already live.
#[derive(Debug, Default, Deserialize)]
pub struct CatalogConfig {
    /// Days a catalog entry may stand before it is worth looking at again, when the entry
    /// does not declare its own.
    ///
    /// A default rather than the mechanism: the per-entry `ttl_days:` is the primary form,
    /// because a gauge record and a statute do not age at the same rate. This exists for the
    /// common case of a corpus whose sources mostly do age alike, so that adopting a TTL is
    /// one line rather than one line per entry.
    ///
    /// Absent means **no entry expires unless it says so itself**, which is every corpus
    /// until someone turns it on. The reasoning is [`LintConfig::escalate_after`]'s and is
    /// not repeated.
    ///
    /// ```toml
    /// [catalog]
    /// ttl_days = 180
    /// ```
    pub ttl_days: Option<u32>,
}

/// When this corpus considers each of its clocks due.
///
/// Read by `yidam due`, and by nothing else. The keys here are the intervals the clocks it
/// reads had none of; the fourth interval it reads is [`CatalogConfig::ttl_days`] and is
/// deliberately **not** repeated here. A source's TTL is a statement about the source and
/// belongs where a source is configured — restating it under `[due]` would create two places
/// to change it and one of them would be wrong.
///
/// Every key absent is the default, and it means `yidam due` reports what it measured and
/// calls nothing due. That is not a degraded mode: a clock with no interval is a number
/// nobody has yet decided the meaning of, and inventing one in the binary would be the
/// failure [`LintConfig::escalate_after`] describes at greater length.
#[derive(Debug, Default, Deserialize)]
pub struct DueConfig {
    /// Corpus-touching commits an open question may stand before it is due a look.
    ///
    /// Commits, not days, and the reasoning is `history::Age`'s: how long a question has gone
    /// unanswered is a fact about the repository, and the repository's clock is `HEAD`. A
    /// corpus that has not committed has not ignored anything.
    ///
    /// ```toml
    /// [due]
    /// questions_after = 100
    /// ```
    pub questions_after: Option<usize>,
    /// Days a bounded inquiry ref may be in flight before it is due a look.
    ///
    /// **Days, and this is the second clock that counts them.** A phase is work somebody is
    /// doing in the world, and it does not stop having been open for four months because
    /// nobody committed to the corpus. That is the same argument
    /// [`crate::cmd::lint::ttl`] makes for a source's TTL, applied to the other quantity
    /// here that is not a fact about the repository.
    ///
    /// ```toml
    /// [due]
    /// phases_after = 60
    /// ```
    pub phases_after: Option<u32>,
    /// Corpus files that may change after the index was built before a rebuild is due.
    ///
    /// `1` means any change at all makes it due, which is what a repository that keeps
    /// semantic search sharp will want. A larger number is a corpus saying it is content for
    /// retrieval to lag its own edits by that much.
    ///
    /// ```toml
    /// [due]
    /// index_after = 25
    /// ```
    ///
    /// **Declarable in every build, and acted on by one.** `index-build` is behind the `index`
    /// feature and the released binary does not carry it, so a corpus that declares this and
    /// installs the default build cannot discharge the clock. `due` says so on the row rather
    /// than reporting it as owed — see [`crate::cmd::due::State::Unbuildable`] — because the
    /// alternative is what #1061 found: a permanently red clock, and two repositories that
    /// silenced it rather than repaired anything. The key is still read in every build, for
    /// [`IndexConfig::remote`]'s reason: this is a corpus's declaration about itself, it lives
    /// in a committed file, and a build that cannot act on one should read it and say so.
    pub index_after: Option<usize>,
    /// Days a node may go on citing a source from before its latest version before it is due
    /// a re-read (#1200).
    ///
    /// **Days, counted from the commit that recorded the version**, for the TTL's reason: a
    /// source changed in the world, and a node resting on the old bytes does not rest on them
    /// any less for nobody having committed. `0` makes every such node due the day the version
    /// arrives, which is what a corpus that means to re-read on every change will want.
    ///
    /// ```toml
    /// [due]
    /// superseded_after = 14
    /// ```
    pub superseded_after: Option<u32>,
    /// Clocks this corpus decided it does not want, each naming the record that argues it.
    ///
    /// **Unset and declined are different states, and only one of them was a choice.** A
    /// corpus with no `index_after` and a corpus that examined a vector index and did not
    /// choose it report identically without this key, and the only remedy `due` offers the
    /// second is to declare an interval for work nobody intends — a clock that is
    /// permanently due, which is a clock a reader learns to skip.
    ///
    /// The key is the clock's `id` — `index`, `catalog`, `questions`, `phases`, `superseded` —
    /// and the value is a decision record in `.yidam/decisions/`, by its `id:` or its file stem.
    /// **The record is required**, and that is the whole of what makes this a declaration
    /// rather than a mute button: a decline naming a record this repository does not hold
    /// is not honoured, and `due` says so. It is `.yidam/lint-baseline.yml`'s property —
    /// an accepted finding and an unnoticed one are different objects, and a decline that
    /// no longer has anything behind it goes red rather than quiet.
    ///
    /// ```toml
    /// [due.declined]
    /// index = "due-clocks"
    /// ```
    #[serde(default)]
    pub declined: BTreeMap<String, String>,
}

/// Where the artifact this corpus is about lives, in this repository.
///
/// **The paths are here and not in the kuten, and that is settled** — RFC-0028 §4, Erratum 5.
/// A kuten is an upstream-authored profile vendored unchanged, `inquiry` is one profile
/// serving six repositories with six object shapes, and the only thing a corpus writes about
/// it is `{kuten, revision}`. There is no channel by which a corpus supplies paths to a
/// profile, and paths are a fact about a repository rather than about a practice. The kuten
/// declares the *direction* of the arrow; this declares where the arrow points. It is the
/// `clocks` precedent: the kuten proposes values and never holds live ones.
///
/// Absent means the repository has **one** register and the corpus vocabulary governs every
/// commit, which is what every repository did before this key existed.
#[derive(Debug, Default, Deserialize)]
pub struct ObjectConfig {
    /// Globs naming the artifact register, relative to the repository root.
    ///
    /// `**` spans any number of path segments and `*` any run within one. A glob also claims
    /// everything beneath what it names, so `"web"` and `"web/**"` say the same thing.
    ///
    /// Read by `lint --commits`, which declines to report a commit touching **only** these
    /// paths against the corpus vocabulary. `feat:` on the artifact is not a corpus-vocabulary
    /// violation; measured across the population, 40 off-vocabulary commits in one derived
    /// repository and 75 in another touch no corpus file at all.
    ///
    /// A commit touching both registers is governed by the corpus register, and a commit
    /// listing no paths — the authored merge — is too.
    ///
    /// ```toml
    /// [object]
    /// paths = ["web/**", "crates/**", "package.json"]
    /// ```
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Where this repository keeps the arguments it derives from its corpus (RFC-0045).
///
/// A derived repository writes its memos, dossiers and findings where its own practice puts them
/// — `dossier/`, `briefs/`, `analysis/` in the three that built a gate for this — and no
/// directory is compiled in for the same reason `[object]` has none: where an argument lives is a
/// fact about a repository and not about the practice.
///
/// Absent means `derive check` has nothing to read, reports nothing, and passes. That is every
/// repository before this key existed.
#[derive(Debug, Default, Deserialize)]
pub struct DeriveConfig {
    /// Globs naming the artifacts `derive check` reads, relative to the repository root.
    ///
    /// The `[object] paths` grammar: `**` spans any number of segments, `*` any run within one,
    /// and a glob claims everything beneath what it names. Under them, a `.yml` or `.yaml` file
    /// is an artifact, and so is a `.md` file that opens with YAML frontmatter; a `.md` without
    /// frontmatter is prose beside the artifacts and is skipped.
    ///
    /// ```toml
    /// [derive]
    /// paths = ["dossier/**"]
    /// ```
    #[serde(default)]
    pub paths: Vec<String>,
}

/// What this repository permits a server it starts to do — RFC-0029.
///
/// # Why a repository's own file and not a flag
///
/// RFC-0029 §2.1 decides that an `act` declaration is **configuration, never inference**: a
/// server that satisfies every condition for writing and was not told to write declares
/// false. A flag would put the declaration in the argv of whatever launcher happened to spawn
/// the process — an agent's client config, a desktop app's manifest, a shell alias — so the
/// answer to *may this corpus be written to by a tool* would be a property of who started the
/// server rather than of the corpus. This file is committed, and the repository is the thing
/// that has to live with what a tool wrote into it.
///
/// RFC-0029's open question offered `.yidam/capabilities.toml` as a third option. **It does
/// not exist** — the name appears in RFC-0026 and RFC-0028 prose and nothing reads it — so the
/// choice was this key or a flag.
#[derive(Debug, Default, Deserialize)]
pub struct ServeConfig {
    /// Whether a server started against this corpus may declare the `act` capability.
    ///
    /// **`false` is the default and a server that satisfies every other condition still
    /// declares false without this key.** That is §2.1's rule and it is the whole difference
    /// between the write tier and every read tier: a read tier's absence says the server
    /// *cannot*, and is therefore discovered; this says the deployment *will not*, and a
    /// policy that a server can discover about itself is not a policy.
    ///
    /// Declaring it true is not sufficient either. RFC-0029 §2.2's clause 1 requires that a
    /// git author identity resolve in the corpus being served — `user.name` and `user.email`,
    /// the values the commit would actually take — and clause 3 requires every listening
    /// socket to be loopback. Both are checked at startup, and both fail the server rather
    /// than quietly downgrading it: a server that was told to write and serves reads instead
    /// is a deployment that believes something false about itself.
    ///
    /// ```toml
    /// [serve]
    /// act = true
    /// ```
    #[serde(default)]
    pub act: bool,
    /// Whether a server started against this corpus appends a line per `tools/call` to
    /// `.yidam/record/calls.jsonl` — [`crate::cmd::serve::record`].
    ///
    /// **`false` is the default, on the precedent `[due]` and `ttl_days` set**: the corpus
    /// declares, nothing is compiled in. A number or a policy compiled into the binary would be
    /// one repository's judgement arriving in another that never agreed to it, and that argument
    /// is sharper here than for `escalate_after`, because over `--http` the thing being recorded
    /// is traffic from callers this server cannot authenticate.
    ///
    /// **Not `act` under another name.** `act` declares what the server may write *into the
    /// graph*, and carries an identity gate because the history it writes is the knowledge
    /// graph. This declares that the server keeps an operational record of what it was asked —
    /// RFC-0026's permitted side, closest to `index:` in the commit vocabulary's own table — and
    /// needs no author, because nothing it writes is testimony.
    ///
    /// ```toml
    /// [serve]
    /// record = true
    /// ```
    #[serde(default)]
    pub record: bool,
}

pub fn load_yidam_config(root: &Path) -> Result<YidamConfig> {
    let path = root.join(".yidam").join("config.toml");
    if !path.exists() {
        return Ok(YidamConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CIDR is the network address and a length its family allows. `10.0.0.1/8` is refused
    /// rather than read as `10.0.0.0/8`, which is wider than whoever wrote it meant.
    #[test]
    fn a_cidr_is_a_network_address_and_a_length() {
        for ok in [
            "10.0.0.0/8",
            "192.0.2.1/32",
            "0.0.0.0/0",
            "::/0",
            "2a0a:a440::/29",
            "::1/128",
        ] {
            assert!(parse_cidr(ok).is_ok(), "{ok}");
        }
        for (bad, why) in [
            ("10.0.0.0", "prefix length"),
            ("host/24", "IP address"),
            ("10.0.0.0/33", "0 to 32"),
            ("::/129", "0 to 128"),
            ("10.0.0.1/8", "network address"),
            ("2a0a:a440::1/29", "network address"),
        ] {
            let err = parse_cidr(bad).unwrap_err();
            assert!(err.contains(why), "{bad}: {err}");
        }
        let e = ClusterEgressConfig {
            remote: vec!["10.0.0.1/8".into()],
            ..Default::default()
        };
        let err = e.check().unwrap_err().to_string();
        assert!(
            err.contains("[cluster.egress] remote") && err.contains("10.0.0.1/8"),
            "{err}"
        );
    }

    /// `sadhana/config.toml`, the scaffold a derived repository gets, deserializes into
    /// [`YidamConfig`] once every offered line is uncommented.
    ///
    /// The file ships with every key commented out, because an interval compiled into a
    /// scaffold is one corpus's judgement arriving in another that never agreed to it. That
    /// is the right delivered state and it is also why nothing checks the file: a comment
    /// has no parser, so a key in the wrong section, a misspelling, or a string where a
    /// number belongs sits there until the day a corpus takes the offer up.
    ///
    /// This is the typed half of that check — right section, right spelling, right type,
    /// through the binary's own deserializer, which is why it lives here and not in
    /// `tests/gates/scaffolded_config.rs`: `mod config` is private. `VaultConfig` and
    /// `RemoteIndexConfig` are `deny_unknown_fields`, so a leaf typo fails here rather than
    /// parsing into nothing.
    #[test]
    fn the_scaffolded_config_deserializes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("sadhana/config.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

        // As delivered: no live keys at all, so a derived repository's config parses to
        // exactly what an absent file gives it.
        let delivered: YidamConfig = toml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} does not parse as delivered: {e}", path.display()));
        assert!(
            delivered.vault.is_empty()
                && delivered.due.questions_after.is_none()
                && delivered.lint.escalate_after.is_none()
                && !delivered.serve.act,
            "the scaffolded config sets a key. Every one of them is meant to be commented \
             out: a number shipped here is a judgement this corpus never made."
        );

        // And as taken up.
        let live: String = text
            .lines()
            .filter_map(|line| {
                let bare = line
                    .strip_prefix('#')
                    .map(str::trim_start)
                    .unwrap_or(line)
                    .trim();
                let header = bare.starts_with('[') && bare.ends_with(']') && !bare.contains(' ');
                let assignment = bare.split_once('=').is_some_and(|(k, _)| {
                    let k = k.trim();
                    !k.is_empty()
                        && k.chars().all(|c| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                        })
                });
                (header || assignment).then(|| bare.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");

        let config: YidamConfig = toml::from_str(&live).unwrap_or_else(|e| {
            panic!(
                "{} does not deserialize once uncommented: {e}\n\n--- uncommented ---\n{live}",
                path.display()
            )
        });

        // Spot-check the ends of the shape, so a parse that silently read nothing is not
        // mistaken for one that read everything.
        assert_eq!(config.due.questions_after, Some(100));
        assert_eq!(
            config.derive.paths,
            ["dossier/**"],
            "the [derive] offer did not survive"
        );
        assert_eq!(config.vault.len(), 1, "the example vault did not survive");
        let vault = config.vault.values().next().unwrap();
        assert!(
            vault.audience.is_some(),
            "the example vault states no audience"
        );
        let remote = config
            .index
            .remote
            .as_ref()
            .expect("the example remote index did not survive");
        assert!(
            !remote.corpora.is_empty(),
            "the example `[index.remote.corpora]` nickname did not survive — a key with a \
             hyphen in it is how that reads, and a parse that drops it would pass every \
             other assertion here"
        );
    }

    #[test]
    fn a_quantity_is_spelled_as_kubernetes_spells_it() {
        for ok in ["250m", "2", "0.5", "1.5Gi", "512Mi", "1k", "3E"] {
            assert!(is_quantity(ok), "{ok}");
        }
        for bad in [
            "", "Gi", "2 Gi", "2GB", "-1", "1.", ".5", "1e3", "2gi", "1.2.3",
        ] {
            assert!(!is_quantity(bad), "{bad}");
        }
    }

    #[test]
    fn a_pod_table_names_its_bad_key() {
        let pod = ClusterPodConfig {
            cpu_limit: Some("two".into()),
            ..Default::default()
        };
        let err = pod.check("[cluster.pod]").unwrap_err().to_string();
        assert!(err.contains("[cluster.pod] cpu_limit = \"two\""), "{err}");

        let pod = ClusterPodConfig {
            deadline_seconds: Some(0),
            ..Default::default()
        };
        assert!(pod.check("[cluster.pod]").is_err());
        assert!(ClusterPodConfig::default().check("[cluster.pod]").is_ok());
    }

    #[test]
    fn pod_gc_is_one_of_argos_strategies() {
        for gc in ClusterCleanupConfig::POD_GC {
            let c = ClusterCleanupConfig {
                pod_gc: Some(gc.to_string()),
                ..Default::default()
            };
            assert!(c.check().is_ok(), "{gc}");
        }
        let c = ClusterCleanupConfig {
            pod_gc: Some("OnSuccess".into()),
            ..Default::default()
        };
        let err = c.check().unwrap_err().to_string();
        assert!(
            err.contains("\"OnSuccess\"") && err.contains("OnPodSuccess"),
            "{err}"
        );
    }
}
