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

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Result};

use crate::cmd::run::manifest::{Manifest, Run, MANIFEST};
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
/// Where a git credential secret is mounted: `key` and `known_hosts` inside it.
const GIT_DIR: &str = "/etc/yidam/git";
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
}

fn resolve(root: &Path, o: &Overrides) -> Result<Settings> {
    let cfg = crate::config::load_yidam_config(root)?;
    let m = Manifest::load(root)?;
    let plan = m.plan(None)?;
    if plan.is_empty() {
        bail!("{MANIFEST} declares no capabilities, so there is no workflow to write");
    }
    super::builtin::refuse_shadowing(&plan)?;
    for name in &plan {
        let cap = m.get(name)?;
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

    Ok(Settings {
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
    })
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
         #\n\
         #   pin, admit  mount yidam-git-read   and can read the remote\n\
         #   step        mounts no git secret   and has no flag that names a ref\n\
         #   land        mounts yidam-git-write and is the only pod that can move one\n\
         #\n\
         # What crosses between tasks is the vault digest of a git bundle, never a shared\n\
         # volume holding a checkout. Pod logs are not provenance: everything that matters is\n\
         # in the receipt, in the commit, on the ref.\n",
    );
    let _ = writeln!(h, "#\n# steps: {}", s.steps.join(" → "));
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
    y.push_str("serviceAccountName: yidam-run\n");
    y.push_str(
        "# No pod gets a Kubernetes token in its main container; the executor sidecar has \
         its own.\n",
    );
    y.push_str("automountServiceAccountToken: false\n");
    y.push_str("executor:\n  serviceAccountName: yidam-run\n");
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
    y.push_str(
        "  - name: git-read\n    secret:\n      secretName: yidam-git-read\n      \
         defaultMode: 256\n",
    );
    y.push_str(
        "  # The one credential that can move a ref. Mounted by `land` and by nothing \
         else.\n  - name: git-write\n    secret:\n      secretName: yidam-git-write\n      \
         defaultMode: 256\n",
    );
    if let Some(mount) = file_vault_mount(&s.vault_url) {
        let _ = writeln!(
            y,
            "  # A file:// vault: a content-addressed store of immutable bundles, mounted at\n  \
             # {mount} on every pod. This is not a corpus checkout on a shared volume — nothing\n  \
             # in it is a working tree, and no pod writes anything but a new digest into it.\n  \
             - name: vault\n    persistentVolumeClaim:\n      claimName: yidam-vault"
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
        let _ = writeln!(
            y,
            "        - name: step-{task}\n          template: step\n          depends: {previous}\n          \
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

    // ── the four container templates ──────────────────────────────────────────
    y.push_str(&template(
        s,
        "pin",
        &[],
        &["cluster", "pin"],
        &remote_args(),
        Credential::Read,
        true,
    ));
    y.push_str(&template(
        s,
        "step",
        &["step", "bundle"],
        &["cluster", "step", "{{inputs.parameters.step}}"],
        &[
            "--bundle",
            "{{inputs.parameters.bundle}}",
            "--image",
            "{{workflow.parameters.image}}",
        ],
        Credential::None,
        true,
    ));
    y.push_str(&template(
        s,
        "land",
        &["step-output"],
        &["cluster", "land"],
        &[
            &["--step-output", "{{inputs.parameters.step-output}}"][..],
            &remote_args(),
        ]
        .concat(),
        Credential::Write,
        true,
    ));
    y.push_str(&template(
        s,
        "admit",
        &[],
        &["cluster", "admit"],
        &remote_args(),
        Credential::Read,
        false,
    ));
    y
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

fn template(
    s: &Settings,
    name: &str,
    inputs: &[&str],
    command: &[&str],
    args: &[&str],
    cred: Credential,
    vault: bool,
) -> String {
    let mut y = String::new();
    let _ = writeln!(y, "  - name: {name}");
    match cred {
        Credential::None => y.push_str(
            "    # No git secret and no remote: this pod computes a commit it cannot land.\n",
        ),
        Credential::Write => y.push_str(
            "    # The lander. It classifies the commit by its own subject line, chooses the ref\n    \
             # the class permits, and writes it with a compare-and-swap.\n",
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
    if cred != Credential::None {
        let _ = writeln!(
            y,
            "      env:\n        - name: GIT_SSH_COMMAND\n          value: {}",
            quote(GIT_SSH)
        );
    }
    if vault {
        y.push_str(
            "      # Vault credentials, for an s3:// vault; absent for file://, which needs none.\n      \
             envFrom:\n        - secretRef:\n            name: yidam-vault\n            optional: true\n",
        );
    }
    let mut mounts: Vec<String> = Vec::new();
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
    if !mounts.is_empty() {
        y.push_str("      volumeMounts:\n");
        for m in mounts {
            let _ = writeln!(y, "{m}");
        }
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
                "step" => assert!(
                    !mounts.iter().any(|m| m.starts_with("git-")),
                    "{name}: {mounts:?}"
                ),
                "run" => {}
                _ => assert!(
                    mounts.contains(&"git-read") && !mounts.contains(&"git-write"),
                    "{name}: {mounts:?}"
                ),
            }
            if name == "step" {
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
}
