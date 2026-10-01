//! `cluster workflow` — the Argo manifest, from the capability manifest.
//!
//! #460 decision 5: Argo Workflows, and no operator. The DAG is Argo's; what this writes is
//! the arrangement of four container templates over it, and the arrangement is the whole of
//! the invariant's deployment:
//!
//! - `pin` and `admit` mount the read credential.
//! - `step` mounts no git credential and is handed no remote.
//! - `land` mounts the write credential, and is the only template that does.
//!
//! A capability that declares `[capability.<name>.cluster]` gets its own copy of `step`,
//! `step-<name>`, carrying its bounds, so its override reaches no other step's pod (#1231).
//!
//! The steps are [`super::builtin::BUILTINS`] — the catalog's connectors, compiled in — and
//! then the manifest's plan. Every corpus gets the first three whatever it declares, and a
//! corpus whose catalog has nothing to do gets three steps that build nothing.
//!
//! The DAG is a chain, because a run is a chain: `run` invokes each step against the commit
//! the one before it landed, and so does this — `pin`, then `step-a`, `land-a`, `step-b`,
//! `land-b`. A wider DAG that ran independent steps in parallel would have two landers
//! racing one ref, which the lander's lease handles, at the cost of one of them re-parenting
//! on every run. That is a change to make when a corpus has a plan wide enough to want it,
//! and none does.
//!
//! # Hand-formatted rather than serialized
//!
//! A YAML serializer would write a correct manifest with no comments in it, and a manifest a
//! person reads before applying to a cluster is one where the comment beside the write
//! credential's mount is worth more than the guarantee that a value is quoted. The output
//! is parse-checked in the tests, which is the guarantee that matters.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Result};

use crate::cmd::run::manifest::{Manifest, Run, MANIFEST};
use crate::config::{ClusterCleanupConfig, ClusterPodConfig};
use crate::paths::{repo_root, require_yidam_repo};

/// Flags that override `[cluster]` and `[vault.<name>]`.
pub(super) struct Overrides {
    pub cron: Option<String>,
    pub image: Option<String>,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub vault_url: Option<String>,
}

/// Where every pod writes its record, and where Argo reads the output parameter from.
const OUT: &str = "/tmp/yidam/out.json";
/// The scratch every pod clones into: an `emptyDir`, and the pod's `TMPDIR`, since the root
/// filesystem is read-only (#1230). [`OUT`] is inside it.
const SCRATCH: &str = "/tmp/yidam";
/// The image's `yidam` user's home, also an `emptyDir`: git and ssh read config from it, and
/// the vault cache a catalog step fetches into is `$HOME/.cache`.
const HOME: &str = "/home/yidam";
/// The image's `yidam` user. The pod asks the cluster to hold it to this rather than trusting
/// the image's `USER`, which a namespace enforcing `restricted` cannot see.
const UID: u32 = 1000;
/// Where a git credential secret is mounted: `key` and `known_hosts` inside it.
const GIT_DIR: &str = "/etc/yidam/git";
/// How often Argo runs a pod again after it fails, on the templates that may be repeated.
const RETRIES: u32 = 2;
const GIT_SSH: &str =
    "ssh -i /etc/yidam/git/key -o UserKnownHostsFile=/etc/yidam/git/known_hosts -o IdentitiesOnly=yes";

pub(super) fn run(o: &Overrides) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    print!("{}", generate(&root, o)?);
    Ok(())
}

struct Settings {
    slug: String,
    image: String,
    remote: String,
    branch: String,
    vault_name: String,
    vault_url: String,
    vault_region: Option<String>,
    vault_endpoint: Option<String>,
    vault_path_style: bool,
    namespace: Option<String>,
    cron: Option<String>,
    steps: Vec<String>,
    /// `.yidam/gathers/<name>.toml`, by name — each a fan-out after the chain (#1217).
    gathers: Vec<String>,
    names: Names,
    /// Every pod's bounds: `[cluster.pod]` over the compiled-in sizes.
    pod: Bounds,
    /// The bounds of each capability that declares its own, by capability name. A step named
    /// here runs from its own template; every other step shares `step`.
    step_pods: BTreeMap<String, Bounds>,
    cleanup: Cleanup,
}

/// One pod's resources and deadline, every key resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bounds {
    cpu_request: String,
    memory_request: String,
    cpu_limit: String,
    memory_limit: String,
    deadline_seconds: u64,
}

impl Bounds {
    /// The sizes a corpus gets when it sets none (#1231).
    ///
    /// Compiled in, unlike a clock, because unset is not a state a cluster has: a namespace
    /// with a `ResourceQuota` refuses a pod that requests nothing, and a pod with no deadline
    /// is a hung calculator holding the corpus's one `concurrencyPolicy: Forbid` slot until a
    /// person deletes it. These are sized for what most pods do — clone, run a shell
    /// calculator, bundle — with headroom, and a corpus that knows better says so.
    fn builtin() -> Self {
        Self {
            cpu_request: "250m".into(),
            memory_request: "512Mi".into(),
            cpu_limit: "2".into(),
            memory_limit: "2Gi".into(),
            deadline_seconds: 3600,
        }
    }

    /// These bounds with each key `o` sets replaced, and every other kept.
    fn over(&self, o: &ClusterPodConfig) -> Self {
        let pick = |set: &Option<String>, kept: &String| set.clone().unwrap_or(kept.clone());
        Self {
            cpu_request: pick(&o.cpu_request, &self.cpu_request),
            memory_request: pick(&o.memory_request, &self.memory_request),
            cpu_limit: pick(&o.cpu_limit, &self.cpu_limit),
            memory_limit: pick(&o.memory_limit, &self.memory_limit),
            deadline_seconds: o.deadline_seconds.unwrap_or(self.deadline_seconds),
        }
    }
}

/// When Argo deletes a finished run's pods and the workflow, every key resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cleanup {
    pod_gc: String,
    seconds_after_success: u64,
    seconds_after_failure: u64,
}

impl Cleanup {
    /// A pod that succeeded goes at once: its record is in the commit, and its logs are not
    /// provenance. A failed one stays with its workflow, a week, for whoever debugs it.
    fn builtin() -> Self {
        Self {
            pod_gc: "OnPodSuccess".into(),
            seconds_after_success: 86_400,
            seconds_after_failure: 604_800,
        }
    }

    fn over(&self, o: &ClusterCleanupConfig) -> Self {
        Self {
            pod_gc: o.pod_gc.clone().unwrap_or(self.pod_gc.clone()),
            seconds_after_success: o
                .seconds_after_success
                .unwrap_or(self.seconds_after_success),
            seconds_after_failure: o
                .seconds_after_failure
                .unwrap_or(self.seconds_after_failure),
        }
    }
}

/// The Kubernetes objects a workflow refers to by name, one set per corpus (#1228).
///
/// These were constants, so a namespace running two corpora held one `yidam-git-write`: either
/// one corpus's lander could not push, or one key could push to both and either lander could
/// land the other's refs. The invariant is which pod holds the credential, and it only holds
/// if the credential is this corpus's alone. The vault's claim and secret are split for the
/// disclosure half of the same argument: RFC-0023 draws a boundary between vaults, and a
/// shared volume erases it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Names {
    service_account: String,
    git_read: String,
    git_write: String,
    vault_secret: String,
    vault_claim: String,
}

impl Names {
    /// `yidam-<slug>-<role>`: the `metadata.name` prefix the workflow already carries.
    fn derived(slug: &str) -> Self {
        let n = |role: &str| format!("yidam-{slug}-{role}");
        Self {
            service_account: n("run"),
            git_read: n("git-read"),
            git_write: n("git-write"),
            vault_secret: n("vault"),
            vault_claim: n("vault"),
        }
    }

    /// The derived set with each `[cluster.names]` override applied, every name checked.
    fn resolve(slug: &str, o: &crate::config::ClusterNamesConfig) -> Result<Self> {
        let d = Self::derived(slug);
        let pick = |key: &str, set: &Option<String>, default: String| -> Result<String> {
            let Some(name) = set else {
                return Ok(default);
            };
            if !is_object_name(name) {
                bail!(
                    "[cluster.names] {key} = {name:?} is not a Kubernetes object name: lowercase \
                     letters, digits, `-` and `.`, starting and ending with a letter or digit, \
                     at most 253 characters"
                );
            }
            Ok(name.clone())
        };
        Ok(Self {
            service_account: pick("service_account", &o.service_account, d.service_account)?,
            git_read: pick("git_read", &o.git_read, d.git_read)?,
            git_write: pick("git_write", &o.git_write, d.git_write)?,
            vault_secret: pick("vault_secret", &o.vault_secret, d.vault_secret)?,
            vault_claim: pick("vault_claim", &o.vault_claim, d.vault_claim)?,
        })
    }
}

/// A DNS-1123 subdomain, which is what a secret, service account or claim name must be.
fn is_object_name(name: &str) -> bool {
    let ok_end = |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    name.len() <= 253
        && ok_end(name.chars().next())
        && ok_end(name.chars().last())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

fn resolve(root: &Path, o: &Overrides) -> Result<Settings> {
    let cfg = crate::config::load_yidam_config(root)?;
    let m = Manifest::load(root)?;
    let plan = m.plan(None)?;
    let gathers = gathers(root);
    if plan.is_empty() && gathers.is_empty() {
        bail!(
            "{MANIFEST} declares no capabilities and .yidam/gathers/ holds no gather, so there \
             is no workflow to write"
        );
    }
    super::builtin::refuse_shadowing(&plan)?;
    cfg.cluster.pod.check("[cluster.pod]")?;
    cfg.cluster.cleanup.check()?;
    let pod = Bounds::builtin().over(&cfg.cluster.pod);
    let mut step_pods = BTreeMap::new();
    for name in &plan {
        let cap = m.get(name)?;
        if let Some(o) = &cap.cluster {
            step_pods.insert(name.to_string(), pod.over(o));
        }
        if let Some(reason) = cap.kind.unrunnable_because() {
            bail!(
                "`{name}` declares `kind = \"{}\"` and a cluster step invokes calculators \
                 only, so this manifest has no workflow.\n  {reason}.",
                cap.kind.as_str()
            );
        }
        // Not [`Capability::unrunnable_because`]: that also asks about *this* build's features,
        // and the pods run an image this binary is not. An unbuilt step is unrunnable in every
        // image, so it is the one build-independent refusal beside the kind's (#1184).
        if let (Run::Unbuilt, Some(reason)) = (&cap.run, cap.run.unrunnable_because()) {
            bail!("`{name}` declares no `run`, so this manifest has no workflow.\n  {reason}.");
        }
    }

    let need = |what: &str, flag: &str, key: &str| {
        anyhow::anyhow!("no {what}: pass `{flag}` or set `{key}` in .yidam/config.toml")
    };
    let image = o
        .image
        .clone()
        .or(cfg.cluster.image.clone())
        .ok_or_else(|| need("image", "--image", "[cluster] image"))?;
    let remote = o
        .remote
        .clone()
        .or(cfg.cluster.remote.clone())
        .ok_or_else(|| need("remote", "--remote", "[cluster] remote"))?;
    let branch = o
        .branch
        .clone()
        .unwrap_or_else(|| cfg.cluster.branch.clone());
    let vault_name = cfg.cluster.vault.clone();
    let vault_cfg = cfg.vault.get(&vault_name);
    let vault_url = o
        .vault_url
        .clone()
        .or_else(|| vault_cfg.map(|v| v.url.clone()))
        .ok_or_else(|| {
            need(
                "vault url",
                "--vault-url",
                &format!("[vault.{vault_name}] url"),
            )
        })?;

    let slug: String = root
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "corpus".to_string())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    let names = Names::resolve(&slug, &cfg.cluster.names)?;

    Ok(Settings {
        names,
        slug,
        image,
        remote,
        branch,
        vault_name,
        vault_url,
        vault_region: vault_cfg.and_then(|v| v.region.clone()),
        vault_endpoint: vault_cfg.and_then(|v| v.endpoint.clone()),
        vault_path_style: vault_cfg.and_then(|v| v.path_style).unwrap_or(false),
        namespace: cfg.cluster.namespace.clone(),
        cron: o.cron.clone(),
        steps: super::builtin::BUILTINS
            .iter()
            .map(|b| b.name)
            .chain(plan.iter().copied())
            .map(str::to_string)
            .collect(),
        gathers,
        pod,
        step_pods,
        cleanup: Cleanup::builtin().over(&cfg.cluster.cleanup),
    })
}

/// Every gather `.yidam/gathers/` declares, by a name `yidam gather` would accept, in order.
fn gathers(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(crate::cmd::gather::gathers_dir(root)) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().to_string();
            let name = name.strip_suffix(".toml")?.to_string();
            crate::cmd::gather::valid_name(&name).then_some(name)
        })
        .collect();
    names.sort();
    names
}

/// The manifest for the corpus at `root`, as text.
pub(super) fn generate(root: &Path, o: &Overrides) -> Result<String> {
    let s = resolve(root, o)?;
    let mut y = String::new();
    y.push_str(&header(&s));
    y.push_str("apiVersion: argoproj.io/v1alpha1\n");
    match &s.cron {
        None => {
            y.push_str("kind: Workflow\nmetadata:\n");
            let _ = writeln!(y, "  generateName: yidam-{}-", s.slug);
            metadata(&mut y, &s);
            y.push_str("spec:\n");
            y.push_str(&indent(&spec(&s), 2));
        }
        Some(schedule) => {
            y.push_str("kind: CronWorkflow\nmetadata:\n");
            let _ = writeln!(y, "  name: yidam-{}", s.slug);
            metadata(&mut y, &s);
            y.push_str("spec:\n");
            let _ = writeln!(y, "  schedule: {}", quote(schedule));
            y.push_str(
                "  # One run at a time per corpus: a second admission while one is landing \
                 would\n  # pin a tip the first is about to move.\n",
            );
            y.push_str("  concurrencyPolicy: Forbid\n");
            y.push_str("  workflowSpec:\n");
            y.push_str(&indent(&spec(&s), 4));
        }
    }
    Ok(y)
}

fn header(s: &Settings) -> String {
    let mut h = String::new();
    let _ = writeln!(
        h,
        "# Generated by `yidam cluster workflow` from {MANIFEST}. Regenerate rather than edit."
    );
    h.push_str("#\n");
    h.push_str(
        "# One run of this corpus's capability manifest on Argo Workflows (RFC-0026 §7). A pin,\n\
         # then for each step in dependency order a `step` and a `land`. The invariant that a\n\
         # run authors operational commits and only proposes epistemic ones is not a check any\n\
         # of these pods performs; it is which pod holds the credential:\n\
         #\n",
    );
    let n = &s.names;
    // Padded to one width, so the three rows read as a table whatever the corpus is called.
    let w = n.git_read.len().max(n.git_write.len());
    let _ = writeln!(
        h,
        "#   pin, admit  mount  {:w$}  and can read the remote",
        n.git_read
    );
    let _ = writeln!(
        h,
        "#   step        mounts {:w$}  and has no flag that names a ref",
        "no git secret"
    );
    let _ = writeln!(
        h,
        "#   land        mounts {:w$}  and is the only pod that can move one",
        n.git_write
    );
    h.push_str(
        "#\n\
         # Every name is this corpus's own, so no other corpus's lander holds its write key.\n\
         # What crosses between tasks is the vault digest of a git bundle, never a shared\n\
         # volume holding a checkout. Pod logs are not provenance: everything that matters is\n\
         # in the receipt, in the commit, on the ref.\n",
    );
    let _ = writeln!(h, "#\n# steps: {}", s.steps.join(" → "));
    if !s.gathers.is_empty() {
        let _ = writeln!(
            h,
            "# gathers: {} — each a survey, one ask per peer, a gather and a land, after the\n\
             # last step and all reading the pin it landed. survey, ask and gather mount no git\n\
             # secret either; an asker reads its peer's bundle from the vault, or its lock url.",
            s.gathers.join(", ")
        );
    }
    h
}

fn metadata(y: &mut String, s: &Settings) {
    if let Some(ns) = &s.namespace {
        let _ = writeln!(y, "  namespace: {}", quote(ns));
    }
    let _ = writeln!(y, "  labels:\n    yidam.dev/corpus: {}", quote(&s.slug));
}

fn spec(s: &Settings) -> String {
    let mut y = String::new();
    y.push_str("entrypoint: run\n");
    let n = &s.names;
    let _ = writeln!(y, "serviceAccountName: {}", quote(&n.service_account));
    y.push_str(
        "# No pod gets a Kubernetes token in its main container; the executor sidecar has \
         its own.\n",
    );
    y.push_str("automountServiceAccountToken: false\n");
    let c = &s.cleanup;
    let _ = writeln!(
        y,
        "# A pod that succeeded is deleted at once; a failed one stays with its workflow.\n\
         podGC:\n  strategy: {}\nttlStrategy:\n  secondsAfterSuccess: {}\n  \
         secondsAfterFailure: {}",
        c.pod_gc, c.seconds_after_success, c.seconds_after_failure
    );
    let _ = writeln!(
        y,
        "executor:\n  serviceAccountName: {}",
        quote(&n.service_account)
    );
    y.push_str("arguments:\n  parameters:\n");
    for (k, v) in [
        ("image", &s.image),
        ("remote", &s.remote),
        ("branch", &s.branch),
        ("vault-url", &s.vault_url),
    ] {
        let _ = writeln!(y, "    - name: {k}\n      value: {}", quote(v));
    }
    y.push_str("volumes:\n");
    let _ = writeln!(
        y,
        "  - name: git-read\n    secret:\n      secretName: {}\n      defaultMode: 256",
        quote(&n.git_read)
    );
    let _ = writeln!(
        y,
        "  # The one credential that can move a ref. Mounted by `land` and by nothing \
         else.\n  - name: git-write\n    secret:\n      secretName: {}\n      \
         defaultMode: 256",
        quote(&n.git_write)
    );
    y.push_str(
        "  # Every root filesystem is read-only, so these are the only places a pod writes.\n  \
         - name: scratch\n    emptyDir: {}\n  - name: home\n    emptyDir: {}\n",
    );
    if let Some(mount) = file_vault_mount(&s.vault_url) {
        let _ = writeln!(
            y,
            "  # A file:// vault: a content-addressed store of immutable bundles, mounted at\n  \
             # {mount} on every pod. This is not a corpus checkout on a shared volume — nothing\n  \
             # in it is a working tree, and no pod writes anything but a new digest into it.\n  \
             - name: vault\n    persistentVolumeClaim:\n      claimName: {}",
            quote(&n.vault_claim)
        );
    }

    y.push_str("templates:\n");
    y.push_str("  - name: run\n    dag:\n      tasks:\n");
    let mut previous = String::from("pin");
    let mut previous_bundle = "$.bundle";
    if s.cron.is_some() {
        y.push_str("        - name: admit\n          template: admit\n");
        y.push_str(
            "        # Nothing below runs unless admission says so. A corpus with nothing owed\n        \
             # submits no run — this task is the whole workflow that ran.\n",
        );
        y.push_str(
            "        - name: pin\n          template: pin\n          depends: admit\n          \
             when: \"{{=jsonpath(tasks.admit.outputs.parameters.output, '$.admitted') == true}}\"\n",
        );
    } else {
        y.push_str("        - name: pin\n          template: pin\n");
    }
    for step in &s.steps {
        let task = task_name(step);
        let template = match s.step_pods.contains_key(step) {
            true => format!("step-{task}"),
            false => "step".to_string(),
        };
        let _ = writeln!(
            y,
            "        - name: step-{task}\n          template: {template}\n          depends: {previous}\n          \
             arguments:\n            parameters:\n              - name: step\n                \
             value: {}\n              - name: bundle\n                \
             value: \"{{{{=jsonpath(tasks.{previous}.outputs.parameters.output, '{previous_bundle}')}}}}\"",
            quote(step)
        );
        let _ = writeln!(
            y,
            "        - name: land-{task}\n          template: land\n          depends: step-{task}\n          \
             arguments:\n            parameters:\n              - name: step-output\n                \
             value: \"{{{{tasks.step-{task}.outputs.parameters.output}}}}\""
        );
        previous = format!("land-{task}");
        previous_bundle = "$.next.bundle";
    }
    for g in &s.gathers {
        gather_tasks(&mut y, g, &previous, previous_bundle);
    }

    // ── the four container templates ──────────────────────────────────────────
    y.push_str(&template(
        s,
        &Pod {
            name: "pin",
            inputs: &[],
            command: &["cluster", "pin"],
            args: &remote_args(),
            cred: Credential::Read,
            vault: true,
            retry: true,
            bounds: &s.pod,
        },
    ));
    y.push_str(&template(s, &step_pod("step", &s.pod)));
    for (step, bounds) in &s.step_pods {
        let name = format!("step-{}", task_name(step));
        y.push_str(&template(s, &step_pod(&name, bounds)));
    }
    y.push_str(&template(
        s,
        &Pod {
            name: "land",
            inputs: &["step-output"],
            command: &["cluster", "land"],
            args: &[
                &["--step-output", "{{inputs.parameters.step-output}}"][..],
                &remote_args(),
            ]
            .concat(),
            cred: Credential::Write,
            vault: true,
            // Never. The lander retries its own compare-and-swap, and refuses when it loses a
            // race it cannot rebuild over; an Argo retry would re-attempt that deliberate
            // refusal and hide it.
            retry: false,
            bounds: &s.pod,
        },
    ));
    if !s.gathers.is_empty() {
        gather_templates(&mut y, s);
    }
    y.push_str(&template(
        s,
        &Pod {
            name: "admit",
            inputs: &[],
            command: &["cluster", "admit"],
            args: &remote_args(),
            cred: Credential::Read,
            vault: false,
            retry: false,
            bounds: &s.pod,
        },
    ));
    y
}

/// The step template, under `name` and with `bounds`: `step` itself, or a capability's own.
///
/// Repeatable, because a step is a function of its input state: a second run against the same
/// bundle builds the same commit, and nothing it does is visible until a lander moves a ref.
fn step_pod<'a>(name: &'a str, bounds: &'a Bounds) -> Pod<'a> {
    Pod {
        name,
        inputs: &["step", "bundle"],
        command: &["cluster", "step", "{{inputs.parameters.step}}"],
        args: &[
            "--bundle",
            "{{inputs.parameters.bundle}}",
            "--image",
            "{{workflow.parameters.image}}",
        ],
        cred: Credential::None,
        vault: true,
        retry: true,
        bounds,
    }
}

/// One gather's four tasks: plan it, ask every planned peer in its own pod, settle, land.
///
/// The askers are a `withParam` fan-out over the survey's `asks`, and `continueOn: failed` so
/// one peer's pod dying does not stop the others: `gather` runs whichever way they ended, and a
/// peer whose asker left no record is `refused` in the roll call, never missing from it.
fn gather_tasks(y: &mut String, g: &str, previous: &str, previous_bundle: &str) {
    let task = task_name(g);
    let survey = format!("tasks.survey-{task}.outputs.parameters.output");
    let _ = writeln!(
        y,
        "        - name: survey-{task}\n          template: survey\n          depends: {previous}\n          \
         arguments:\n            parameters:\n              - name: gather\n                \
         value: {}\n              - name: bundle\n                \
         value: \"{{{{=jsonpath(tasks.{previous}.outputs.parameters.output, '{previous_bundle}')}}}}\"",
        quote(g)
    );
    let _ = writeln!(
        y,
        "        - name: ask-{task}\n          template: ask\n          depends: survey-{task}\n          \
         when: \"{{{{=jsonpath({survey}, '$.asking') > 0}}}}\"\n          \
         withParam: \"{{{{=toJson(jsonpath({survey}, '$.asks'))}}}}\"\n          \
         continueOn:\n            failed: true\n          \
         arguments:\n            parameters:\n              - name: ask\n                \
         value: \"{{{{item}}}}\""
    );
    let _ = writeln!(
        y,
        "        - name: gather-{task}\n          template: gather\n          \
         depends: \"ask-{task}.Succeeded || ask-{task}.Failed\"\n          \
         when: \"{{{{=jsonpath({survey}, '$.asking') > 0}}}}\"\n          \
         arguments:\n            parameters:\n              - name: gather\n                \
         value: {}\n              - name: bundle\n                \
         value: \"{{{{=jsonpath({survey}, '$.bundle')}}}}\"\n              - name: asked\n                \
         value: \"{{{{tasks.ask-{task}.outputs.parameters.output}}}}\"",
        quote(g)
    );
    let _ = writeln!(
        y,
        "        - name: land-gather-{task}\n          template: land\n          depends: gather-{task}\n          \
         arguments:\n            parameters:\n              - name: step-output\n                \
         value: \"{{{{tasks.gather-{task}.outputs.parameters.output}}}}\""
    );
}

/// The three gather templates, written only for a corpus that declares a gather.
fn gather_templates(y: &mut String, s: &Settings) {
    y.push_str(&template(
        s,
        &Pod {
            name: "survey",
            inputs: &["gather", "bundle"],
            command: &["cluster", "survey", "{{inputs.parameters.gather}}"],
            args: &["--bundle", "{{inputs.parameters.bundle}}"],
            cred: Credential::None,
            vault: true,
            // A plan read from the pin: the same bundle plans the same asks.
            retry: true,
            bounds: &s.pod,
        },
    ));
    y.push_str(&template(
        s,
        &Pod {
            name: "ask",
            inputs: &["ask"],
            command: &["cluster", "ask"],
            args: &[
                "--ask",
                "{{inputs.parameters.ask}}",
                "--image",
                "{{workflow.parameters.image}}",
            ],
            cred: Credential::None,
            vault: true,
            retry: false,
            bounds: &s.pod,
        },
    ));
    y.push_str(&template(
        s,
        &Pod {
            name: "gather",
            inputs: &["gather", "bundle", "asked"],
            command: &["cluster", "gather", "{{inputs.parameters.gather}}"],
            args: &[
                "--bundle",
                "{{inputs.parameters.bundle}}",
                "--asked",
                "{{inputs.parameters.asked}}",
            ],
            cred: Credential::None,
            vault: true,
            retry: false,
            bounds: &s.pod,
        },
    ));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Credential {
    None,
    Read,
    Write,
}

fn remote_args() -> Vec<&'static str> {
    vec![
        "--remote",
        "{{workflow.parameters.remote}}",
        "--branch",
        "{{workflow.parameters.branch}}",
    ]
}

/// One container template, as [`template`] writes it.
struct Pod<'a> {
    name: &'a str,
    inputs: &'a [&'a str],
    command: &'a [&'a str],
    args: &'a [&'a str],
    cred: Credential,
    vault: bool,
    /// Whether Argo runs the pod again when it fails. Only for a pod whose second run computes
    /// what its first would have and moves nothing: `pin`, `survey` and `step` (#1231).
    retry: bool,
    bounds: &'a Bounds,
}

fn template(s: &Settings, pod: &Pod) -> String {
    let Pod {
        name,
        inputs,
        command,
        args,
        cred,
        vault,
        retry,
        bounds,
    } = *pod;
    let mut y = String::new();
    let _ = writeln!(y, "  - name: {name}");
    match cred {
        Credential::None => y.push_str(
            "    # No git secret and no remote: this pod computes a commit it cannot land.\n",
        ),
        Credential::Write => y.push_str(
            "    # The lander. It classifies the commit by its own subject line, chooses the ref\n    \
             # the class permits, and writes it with a compare-and-swap. No retryStrategy: it\n    \
             # retries its own race, and a refusal it reaches is the answer, not a fault.\n",
        ),
        Credential::Read => {}
    }
    if !inputs.is_empty() {
        y.push_str("    inputs:\n      parameters:\n");
        for i in inputs {
            let _ = writeln!(y, "        - name: {i}");
        }
    }
    let _ = writeln!(
        y,
        "    outputs:\n      parameters:\n        - name: output\n          valueFrom:\n            path: {OUT}"
    );
    // Per attempt: a retried pod gets its own deadline, so a hang costs at most one each.
    let _ = writeln!(y, "    activeDeadlineSeconds: {}", bounds.deadline_seconds);
    if retry {
        let _ = writeln!(
            y,
            "    retryStrategy:\n      limit: {RETRIES}\n      retryPolicy: Always\n      \
             backoff:\n        duration: \"30s\"\n        factor: 2"
        );
    }
    // The `restricted` Pod Security Standard, so a namespace enforcing it admits the pod
    // (#1230). `fsGroup` is what makes a `file://` vault's claim writable by the user.
    let _ = writeln!(
        y,
        "    securityContext:\n      runAsNonRoot: true\n      runAsUser: {UID}\n      \
         runAsGroup: {UID}\n      fsGroup: {UID}\n      seccompProfile:\n        type: RuntimeDefault"
    );
    y.push_str("    container:\n      image: \"{{workflow.parameters.image}}\"\n");
    let _ = writeln!(
        y,
        "      command: [{}]",
        std::iter::once("yidam")
            .chain(command.iter().copied())
            .map(quote)
            .collect::<Vec<_>>()
            .join(", ")
    );
    y.push_str("      args:\n");
    let mut all: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    if vault {
        if s.vault_name != "default" {
            all.extend(["--vault".to_string(), s.vault_name.clone()]);
        }
        all.extend([
            "--vault-url".to_string(),
            "{{workflow.parameters.vault-url}}".to_string(),
        ]);
        if let Some(r) = &s.vault_region {
            all.extend(["--vault-region".to_string(), r.clone()]);
        }
        if let Some(e) = &s.vault_endpoint {
            all.extend(["--vault-endpoint".to_string(), e.clone()]);
        }
        if s.vault_path_style {
            all.push("--vault-path-style".to_string());
        }
    }
    all.extend(["--out".to_string(), OUT.to_string()]);
    for a in &all {
        let _ = writeln!(y, "        - {}", quote(a));
    }
    let _ = writeln!(
        y,
        "      securityContext:\n        allowPrivilegeEscalation: false\n        \
         readOnlyRootFilesystem: true\n        capabilities:\n          drop: [\"ALL\"]"
    );
    let _ = writeln!(
        y,
        "      resources:\n        requests:\n          cpu: {}\n          memory: {}\n        \
         limits:\n          cpu: {}\n          memory: {}",
        quote(&bounds.cpu_request),
        quote(&bounds.memory_request),
        quote(&bounds.cpu_limit),
        quote(&bounds.memory_limit)
    );
    let _ = writeln!(
        y,
        "      env:\n        - name: TMPDIR\n          value: {}\n        - name: HOME\n          \
         value: {}",
        quote(SCRATCH),
        quote(HOME)
    );
    if cred != Credential::None {
        let _ = writeln!(
            y,
            "        - name: GIT_SSH_COMMAND\n          value: {}",
            quote(GIT_SSH)
        );
    }
    if vault {
        let _ = writeln!(
            y,
            "      # Vault credentials, for an s3:// vault; absent for file://, which needs none.\n      \
             envFrom:\n        - secretRef:\n            name: {}\n            optional: true",
            quote(&s.names.vault_secret)
        );
    }
    let mut mounts: Vec<String> = vec![
        format!("        - name: scratch\n          mountPath: {SCRATCH}"),
        format!("        - name: home\n          mountPath: {HOME}"),
    ];
    match cred {
        Credential::Read => mounts.push(format!(
            "        - name: git-read\n          mountPath: {GIT_DIR}\n          readOnly: true"
        )),
        Credential::Write => mounts.push(format!(
            "        - name: git-write\n          mountPath: {GIT_DIR}\n          readOnly: true"
        )),
        Credential::None => {}
    }
    if vault {
        if let Some(mount) = file_vault_mount(&s.vault_url) {
            mounts.push(format!(
                "        - name: vault\n          mountPath: {}",
                quote(&mount)
            ));
        }
    }
    y.push_str("      volumeMounts:\n");
    for m in mounts {
        let _ = writeln!(y, "{m}");
    }
    y
}

/// The path a `file://` vault url names, which is where its volume is mounted, or `None`.
fn file_vault_mount(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest.to_string()
    } else {
        format!("/{rest}")
    };
    Some(path.trim_end_matches('/').to_string())
}

/// An Argo task name from a capability name: the characters a DNS label allows.
fn task_name(step: &str) -> String {
    step.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// A double-quoted YAML scalar.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn indent(text: &str, by: usize) -> String {
    let pad = " ".repeat(by);
    text.lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overrides() -> Overrides {
        Overrides {
            cron: None,
            image: Some("ghcr.io/example/yidam:test".into()),
            remote: Some("git@example.com:corpus.git".into()),
            branch: None,
            vault_url: Some("file:///vault".into()),
        }
    }

    #[test]
    fn a_file_vault_is_mounted_where_its_url_points() {
        assert_eq!(file_vault_mount("file:///vault").as_deref(), Some("/vault"));
        assert_eq!(
            file_vault_mount("file:///mnt/bundles/").as_deref(),
            Some("/mnt/bundles")
        );
        assert_eq!(file_vault_mount("s3://bucket/prefix"), None);
    }

    #[test]
    fn every_derived_name_is_the_corpus_own_and_an_object_name() {
        let a = Names::derived("streamflow");
        let b = Names::derived("rivergage");
        let all = |n: &Names| {
            [
                n.service_account.clone(),
                n.git_read.clone(),
                n.git_write.clone(),
                n.vault_secret.clone(),
                n.vault_claim.clone(),
            ]
        };
        for name in all(&a).iter().chain(all(&b).iter()) {
            assert!(is_object_name(name), "{name}");
        }
        for name in all(&a) {
            assert!(!all(&b).contains(&name), "{name} is both corpora's");
        }
        // A slug that begins or ends in `-` still derives a valid name, since the prefix and
        // the role bracket it.
        for name in all(&Names::derived("-odd-")) {
            assert!(is_object_name(&name), "{name}");
        }
    }

    #[test]
    fn object_names_follow_dns_1123() {
        for ok in ["a", "yidam-git-write", "a.b-c", "0x"] {
            assert!(is_object_name(ok), "{ok}");
        }
        let long = "a".repeat(254);
        for bad in [
            "",
            "Upper",
            "under_score",
            "-lead",
            "trail-",
            ".dot",
            long.as_str(),
        ] {
            assert!(!is_object_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_names_override_replaces_one_name_and_is_checked() {
        let o = crate::config::ClusterNamesConfig {
            git_write: Some("yidam-git-write".into()),
            ..Default::default()
        };
        let n = Names::resolve("streamflow", &o).unwrap();
        assert_eq!(n.git_write, "yidam-git-write");
        assert_eq!(n.git_read, "yidam-streamflow-git-read");

        let o = crate::config::ClusterNamesConfig {
            vault_claim: Some("Vault".into()),
            ..Default::default()
        };
        let err = Names::resolve("streamflow", &o).unwrap_err().to_string();
        assert!(
            err.contains("vault_claim") && err.contains("\"Vault\""),
            "{err}"
        );
    }

    #[test]
    fn task_names_are_dns_labels() {
        assert_eq!(task_name("travel-tier"), "travel-tier");
        assert_eq!(task_name("Rate_Card.v2"), "rate-card-v2");
    }

    #[test]
    fn indenting_leaves_blank_lines_blank() {
        assert_eq!(indent("a\n\nb\n", 2), "  a\n\n  b\n");
    }

    /// The generated YAML parses, and only `land` mounts the write credential.
    #[test]
    fn the_write_credential_is_mounted_by_the_lander_alone() {
        let s = Settings {
            slug: "corpus".into(),
            image: "img".into(),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: None,
            steps: vec!["a".into(), "b".into()],
            gathers: vec![],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::new(),
            cleanup: Cleanup::builtin(),
        };
        let text = spec(&s);
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("the spec parses");
        let templates = doc["templates"].as_sequence().expect("templates");
        for t in templates {
            let name = t["name"].as_str().unwrap();
            let mounts: Vec<&str> = t["container"]["volumeMounts"]
                .as_sequence()
                .map(|m| m.iter().filter_map(|v| v["name"].as_str()).collect())
                .unwrap_or_default();
            match name {
                "land" => assert!(mounts.contains(&"git-write"), "{name}: {mounts:?}"),
                "step" | "survey" | "ask" | "gather" => assert!(
                    !mounts.iter().any(|m| m.starts_with("git-")),
                    "{name}: {mounts:?}"
                ),
                "run" => {}
                _ => assert!(
                    mounts.contains(&"git-read") && !mounts.contains(&"git-write"),
                    "{name}: {mounts:?}"
                ),
            }
            if matches!(name, "step" | "survey" | "ask" | "gather") {
                let args = serde_yaml::to_string(&t["container"]).unwrap();
                assert!(!args.contains("--remote"), "step names a remote:\n{args}");
            }
        }
        let tasks = doc["templates"][0]["dag"]["tasks"].as_sequence().unwrap();
        let names: Vec<&str> = tasks.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["pin", "step-a", "land-a", "step-b", "land-b"]);
        assert_eq!(tasks[3]["depends"].as_str(), Some("land-a"));
        assert!(tasks[3]["arguments"]["parameters"][1]["value"]
            .as_str()
            .unwrap()
            .contains("$.next.bundle"));
        let _ = overrides();
    }

    /// A step is told the image it was started from by the parameter that started it, so the
    /// digest its receipt records is the one the pod ran and never a tag read from a registry.
    #[test]
    fn a_step_names_its_image_by_the_parameter_its_container_runs() {
        let s = Settings {
            slug: "corpus".into(),
            image: format!("img@sha256:{}", "a".repeat(64)),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: None,
            steps: vec!["a".into()],
            gathers: vec![],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::new(),
            cleanup: Cleanup::builtin(),
        };
        let doc: serde_yaml::Value = serde_yaml::from_str(&spec(&s)).unwrap();
        let step = doc["templates"]
            .as_sequence()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "step")
            .unwrap();
        let image = step["container"]["image"].as_str().unwrap();
        let args: Vec<&str> = step["container"]["args"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|a| a.as_str())
            .collect();
        let at = args.iter().position(|a| *a == "--image").expect("--image");
        assert_eq!(args[at + 1], image);
    }

    /// A gather runs after the chain, from the pin its last land moved: one survey, one asker
    /// per planned peer that is allowed to fail without stopping the rest, a gather that runs
    /// either way, and the same lander as a step. None of the three mounts a git secret.
    #[test]
    fn a_gather_fans_out_after_the_chain_and_mounts_no_git_secret() {
        let s = Settings {
            slug: "corpus".into(),
            image: "img".into(),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: None,
            steps: vec!["a".into()],
            gathers: vec!["units".into()],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::new(),
            cleanup: Cleanup::builtin(),
        };
        let text = spec(&s);
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("the spec parses");
        let tasks = doc["templates"][0]["dag"]["tasks"].as_sequence().unwrap();
        let names: Vec<&str> = tasks.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "pin",
                "step-a",
                "land-a",
                "survey-units",
                "ask-units",
                "gather-units",
                "land-gather-units"
            ]
        );
        let task = |n: &str| tasks.iter().find(|t| t["name"] == n).unwrap();
        assert_eq!(task("survey-units")["depends"].as_str(), Some("land-a"));
        assert!(task("survey-units")["arguments"]["parameters"][1]["value"]
            .as_str()
            .unwrap()
            .contains("tasks.land-a.outputs.parameters.output, '$.next.bundle'"));
        let ask = task("ask-units");
        assert!(ask["withParam"].as_str().unwrap().contains("'$.asks'"));
        assert_eq!(ask["continueOn"]["failed"].as_bool(), Some(true));
        assert_eq!(
            task("gather-units")["depends"].as_str(),
            Some("ask-units.Succeeded || ask-units.Failed")
        );
        assert_eq!(task("land-gather-units")["template"].as_str(), Some("land"));
        let templates: Vec<&str> = doc["templates"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for t in ["survey", "ask", "gather"] {
            assert!(templates.contains(&t), "{t} missing from {templates:?}");
        }
        assert!(header(&s).contains("# gathers: units"));
    }

    /// Every pod the workflow starts is one a namespace enforcing the `restricted` Pod Security
    /// Standard admits, and every path it writes is a volume, since its root is read-only
    /// (#1230). The templates are walked, not named, so one added later is held to this without
    /// joining a list; and the fixture must write as many as the generator has call sites, so a
    /// template behind a condition this fixture does not meet cannot pass unread.
    #[test]
    fn every_pod_satisfies_the_restricted_profile() {
        let s = Settings {
            slug: "corpus".into(),
            image: "img".into(),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: Some("0 6 * * *".into()),
            steps: vec!["a".into()],
            gathers: vec!["units".into()],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::from([("a".into(), Bounds::builtin())]),
            cleanup: Cleanup::builtin(),
        };
        let doc: serde_yaml::Value = serde_yaml::from_str(&spec(&s)).expect("the spec parses");
        // The volume types `restricted` allows; a hostPath, say, fails admission.
        const ALLOWED: &[&str] = &[
            "configMap",
            "csi",
            "downwardAPI",
            "emptyDir",
            "ephemeral",
            "persistentVolumeClaim",
            "projected",
            "secret",
        ];
        let mut volumes = std::collections::BTreeMap::new();
        for v in doc["volumes"].as_sequence().expect("volumes") {
            let name = v["name"].as_str().unwrap();
            let kind = v
                .as_mapping()
                .unwrap()
                .keys()
                .filter_map(|k| k.as_str())
                .find(|k| *k != "name")
                .unwrap();
            assert!(ALLOWED.contains(&kind), "{name} is a {kind} volume");
            volumes.insert(name, kind);
        }
        assert_eq!(volumes.get("scratch"), Some(&"emptyDir"));
        assert_eq!(volumes.get("home"), Some(&"emptyDir"));
        assert!(
            OUT.starts_with(&format!("{SCRATCH}/")),
            "{OUT} is not in scratch"
        );

        let mut checked = vec![];
        for t in doc["templates"].as_sequence().expect("templates") {
            let name = t["name"].as_str().unwrap();
            if t.get("dag").is_some() {
                continue;
            }
            let c = &t["container"];
            assert!(
                c.is_mapping(),
                "{name} is neither a dag nor a container template, so this cannot check it"
            );

            let pod = &t["securityContext"];
            assert_eq!(pod["runAsNonRoot"].as_bool(), Some(true), "{name}");
            assert_eq!(pod["runAsUser"].as_u64(), Some(UID.into()), "{name}");
            assert_eq!(pod["runAsGroup"].as_u64(), Some(UID.into()), "{name}");
            assert_eq!(pod["fsGroup"].as_u64(), Some(UID.into()), "{name}");
            assert_eq!(
                pod["seccompProfile"]["type"].as_str(),
                Some("RuntimeDefault"),
                "{name}"
            );
            assert_ne!(UID, 0);

            let sc = &c["securityContext"];
            assert_eq!(
                sc["allowPrivilegeEscalation"].as_bool(),
                Some(false),
                "{name}"
            );
            assert_eq!(sc["readOnlyRootFilesystem"].as_bool(), Some(true), "{name}");
            let drop: Vec<&str> = sc["capabilities"]["drop"]
                .as_sequence()
                .map(|d| d.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            assert_eq!(drop, ["ALL"], "{name}");
            assert!(sc["capabilities"].get("add").is_none(), "{name}");
            assert!(sc.get("privileged").is_none(), "{name}");

            let env = |k: &str| {
                c["env"]
                    .as_sequence()
                    .and_then(|e| e.iter().find(|v| v["name"] == k))
                    .and_then(|v| v["value"].as_str())
            };
            assert_eq!(env("TMPDIR"), Some(SCRATCH), "{name}");
            assert_eq!(env("HOME"), Some(HOME), "{name}");
            let mounts: Vec<(&str, &str)> = c["volumeMounts"]
                .as_sequence()
                .expect("volumeMounts")
                .iter()
                .map(|m| {
                    (
                        m["name"].as_str().unwrap(),
                        m["mountPath"].as_str().unwrap(),
                    )
                })
                .collect();
            for (vol, _) in &mounts {
                assert!(volumes.contains_key(vol), "{name} mounts undeclared {vol}");
            }
            assert!(mounts.contains(&("scratch", SCRATCH)), "{name}: {mounts:?}");
            assert!(mounts.contains(&("home", HOME)), "{name}: {mounts:?}");
            checked.push(name);
        }
        let call_sites = include_str!("workflow.rs")
            .matches(concat!("y.push_str(&", "template("))
            .count();
        assert_eq!(
            checked.len(),
            call_sites,
            "the fixture wrote {checked:?}, and the generator has {call_sites} templates"
        );
    }

    /// Every template the generator has: a cron, a gather and a capability with its own
    /// bounds, so no template sits behind a condition this does not meet.
    fn every_template() -> Settings {
        let mut wide = Bounds::builtin();
        wide.memory_limit = "8Gi".into();
        Settings {
            slug: "corpus".into(),
            image: "img".into(),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: Some("0 6 * * *".into()),
            steps: vec!["a".into(), "b".into()],
            gathers: vec!["units".into()],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::from([("b".into(), wide)]),
            cleanup: Cleanup::builtin(),
        }
    }

    /// The `yidam cluster` subcommand a container template runs: what it *is*, whatever it is
    /// called.
    fn subcommand(t: &serde_yaml::Value) -> Option<&str> {
        t["container"]["command"].as_sequence()?.get(2)?.as_str()
    }

    /// No pod that runs `cluster land` carries a `retryStrategy`, and nothing hands it one.
    ///
    /// The lander retries its own compare-and-swap and refuses when it loses a race it cannot
    /// rebuild over. That refusal is the answer — the next admission runs the step over — and
    /// an Argo retry on top would attempt it again and bury it. Landers are found by the
    /// command they run, so one under another name is still a lander; and the pods that may
    /// repeat are asserted to, so a walk that read nothing cannot pass.
    #[test]
    fn no_land_template_carries_a_retry_strategy() {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&spec(&every_template())).expect("the spec parses");
        assert!(
            doc.get("templateDefaults").is_none(),
            "templateDefaults would reach the lander too"
        );
        assert!(
            doc.get("retryStrategy").is_none(),
            "a workflow retryStrategy would reach the lander too"
        );
        let mut retried = vec![];
        let mut landers = 0;
        for t in doc["templates"].as_sequence().expect("templates") {
            let Some(cmd) = subcommand(t) else { continue };
            let name = t["name"].as_str().unwrap();
            if cmd == "land" {
                landers += 1;
                assert!(
                    t.get("retryStrategy").is_none(),
                    "{name} runs `cluster land` and carries a retryStrategy"
                );
            } else if let Some(r) = t.get("retryStrategy") {
                assert_eq!(r["limit"].as_u64(), Some(RETRIES.into()), "{name}");
                retried.push(cmd);
            }
        }
        assert_eq!(landers, 1);
        // The overridden step is a second `step` template, and it retries too.
        retried.sort();
        assert_eq!(retried, ["pin", "step", "step", "survey"]);
    }

    /// Every pod requests, is limited and has a deadline, and the workflow cleans up after
    /// itself. A namespace with a `ResourceQuota` refuses a pod that requests nothing.
    #[test]
    fn every_pod_is_bounded_and_every_run_is_collected() {
        let s = every_template();
        let doc: serde_yaml::Value = serde_yaml::from_str(&spec(&s)).expect("the spec parses");
        assert_eq!(doc["podGC"]["strategy"].as_str(), Some("OnPodSuccess"));
        assert_eq!(
            doc["ttlStrategy"]["secondsAfterSuccess"].as_u64(),
            Some(86_400)
        );
        assert_eq!(
            doc["ttlStrategy"]["secondsAfterFailure"].as_u64(),
            Some(604_800)
        );
        let mut bounded = 0;
        for t in doc["templates"].as_sequence().expect("templates") {
            if t.get("dag").is_some() {
                continue;
            }
            let name = t["name"].as_str().unwrap();
            let r = &t["container"]["resources"];
            for side in ["requests", "limits"] {
                for key in ["cpu", "memory"] {
                    assert!(
                        r[side][key].as_str().is_some(),
                        "{name} has no {side}.{key}"
                    );
                }
            }
            assert_eq!(t["activeDeadlineSeconds"].as_u64(), Some(3600), "{name}");
            bounded += 1;
        }
        let call_sites = include_str!("workflow.rs")
            .matches(concat!("y.push_str(&", "template("))
            .count();
        assert_eq!(
            bounded, call_sites,
            "a template this fixture does not write"
        );
    }

    /// A capability's bounds reach its own step's template and no other, and its task runs
    /// from it. Two steps, only the second overridden, so a template chosen by position or
    /// shared by all would both be wrong here.
    #[test]
    fn a_capability_bounds_reach_only_its_own_step() {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&spec(&every_template())).expect("the spec parses");
        let templates = doc["templates"].as_sequence().unwrap();
        let memory = |name: &str| {
            templates
                .iter()
                .find(|t| t["name"] == name)
                .unwrap_or_else(|| panic!("no template {name}"))["container"]["resources"]["limits"]
                ["memory"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(memory("step-b"), "8Gi");
        for other in ["step", "pin", "land"] {
            assert_eq!(memory(other), "2Gi", "{other}");
        }
        assert!(templates.iter().all(|t| t["name"] != "step-a"));
        let tasks = templates[0]["dag"]["tasks"].as_sequence().unwrap();
        let uses = |task: &str| {
            tasks.iter().find(|t| t["name"] == task).unwrap()["template"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(uses("step-a"), "step");
        assert_eq!(uses("step-b"), "step-b");
    }

    /// A capability's key replaces the corpus's, and every key it leaves unset is the
    /// corpus's, not the compiled-in one.
    #[test]
    fn bounds_layer_key_by_key() {
        let corpus = Bounds::builtin().over(&ClusterPodConfig {
            cpu_request: Some("1".into()),
            ..Default::default()
        });
        let cap = corpus.over(&ClusterPodConfig {
            memory_limit: Some("8Gi".into()),
            deadline_seconds: Some(600),
            ..Default::default()
        });
        assert_eq!(cap.cpu_request, "1");
        assert_eq!(cap.memory_limit, "8Gi");
        assert_eq!(cap.deadline_seconds, 600);
        assert_eq!(cap.memory_request, Bounds::builtin().memory_request);
        assert_eq!(corpus.deadline_seconds, 3600);
    }

    /// A corpus with no gather gets none of the gather templates, so its workflow is unchanged.
    #[test]
    fn no_gather_writes_no_gather_template() {
        let s = Settings {
            slug: "corpus".into(),
            image: "img".into(),
            remote: "r".into(),
            branch: "main".into(),
            vault_name: "default".into(),
            vault_url: "file:///vault".into(),
            vault_region: None,
            vault_endpoint: None,
            vault_path_style: false,
            namespace: None,
            cron: None,
            steps: vec!["a".into()],
            gathers: vec![],
            names: Names::derived("corpus"),
            pod: Bounds::builtin(),
            step_pods: BTreeMap::new(),
            cleanup: Cleanup::builtin(),
        };
        let text = spec(&s);
        for t in ["survey", "ask", "gather"] {
            assert!(!text.contains(&format!("- name: {t}\n")), "{t} written");
        }
    }
}
