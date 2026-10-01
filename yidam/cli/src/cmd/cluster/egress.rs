//! `cluster network-policy`: what each of a corpus's pods may reach, as NetworkPolicy (#1232).
//!
//! RFC-0026 §7 holds the invariant by mount: only `land` mounts the write key. That holds
//! until a key reaches another pod some other way, such as a webhook that projects the wrong
//! secret, or an overlay that adds the wrong volume. Then nothing but the network stops that
//! pod from pushing. So every template carries a label naming its [`Egress`] kind, and this
//! writes one policy per kind, over a corpus-wide policy that denies everything else.
//!
//! # Addresses, not hosts
//!
//! A NetworkPolicy matches an address. The remote, an `s3://` vault and the API server are
//! hostnames, so a corpus declares each as CIDRs in `[cluster.egress]`. The property that
//! matters does not depend on the remote's: a `step` pod runs a calculator, which is arbitrary
//! code, and it reaches DNS, the API server and the vault, whether the remote is declared or
//! not. Declaring the remote narrows the remote pods to it and takes it out of the internet pods'
//! reach. Both of those run only this binary's own code. A CNI that matches hostnames would
//! need no CIDRs at all; that is #1255.
//!
//! # Separate from the workflow
//!
//! A one-shot `Workflow` is submitted with `kubectl create -f`, once per run. Policies in the
//! same file would fail the second run with `AlreadyExists`. They are the corpus's standing
//! objects, applied with its overlay, like its secrets.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Result};

use crate::config::{parse_cidr, ClusterEgressConfig};
use crate::paths::{repo_root, require_yidam_repo};

/// The label every pod of a corpus carries, whatever its kind. The corpus-wide policy selects
/// on it alone, so a pod whose kind label is missing or misspelt still reaches nothing.
pub(super) const CORPUS_LABEL: &str = "yidam.dev/corpus";
/// The label naming a pod's [`Egress`] kind, which selects the one policy that widens it.
pub(super) const EGRESS_LABEL: &str = "yidam.dev/egress";

/// What a pod needs to reach, beyond DNS and the API server every pod reaches.
///
/// The table in `docs/cluster-runs.md`, as a type. A template names its kind, and the kind is
/// its whole network: nothing in a template adds a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Egress {
    /// `admit`, `pin` and `land`: the git remote, and the vault.
    Remote,
    /// `step`, `survey` and `gather`: the vault only. A calculator runs here.
    Vault,
    /// `catalog-fetch` and `ask`: the vault, and the internet but the remote. A source, or a
    /// peer's lock url, can be anywhere.
    Internet,
}

impl Egress {
    pub(super) const ALL: &[Self] = &[Self::Remote, Self::Vault, Self::Internet];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Remote => "remote",
            Self::Vault => "vault",
            Self::Internet => "internet",
        }
    }
}

/// Flags that override `[cluster.egress]` and `[vault.<name>] url`. A list given on the
/// command line replaces the configured one rather than adding to it.
pub(super) struct Overrides {
    pub vault_url: Option<String>,
    pub executor: Vec<String>,
    pub remote: Vec<String>,
    pub vault: Vec<String>,
    pub sts: Vec<String>,
}

pub(super) fn run(o: &Overrides) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    print!("{}", generate(&root, o)?);
    Ok(())
}

/// What the policies are written from, every key resolved and checked.
pub(super) struct Settings {
    pub slug: String,
    pub namespace: Option<String>,
    /// `None` for a `file://` vault, which is a volume and no address.
    pub vault: Option<Vec<String>>,
    pub executor: Vec<String>,
    pub remote: Vec<String>,
    /// STS, beside the vault, for pods that assume a role. Empty unless the vault is `s3://`.
    pub sts: Vec<String>,
}

fn resolve(root: &Path, o: &Overrides) -> Result<Settings> {
    let cfg = crate::config::load_yidam_config(root)?;
    let pick = |flag: &[String], set: &[String]| match flag.is_empty() {
        true => set.to_vec(),
        false => flag.to_vec(),
    };
    let e = &cfg.cluster.egress;
    let egress = ClusterEgressConfig {
        executor: pick(&o.executor, &e.executor),
        remote: pick(&o.remote, &e.remote),
        vault: pick(&o.vault, &e.vault),
        sts: pick(&o.sts, &e.sts),
    };
    egress.check()?;
    let vault_name = &cfg.cluster.vault;
    let vault_url = o
        .vault_url
        .clone()
        .or_else(|| cfg.vault.get(vault_name).map(|v| v.url.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no vault url: pass `--vault-url` or set `[vault.{vault_name}] url` in \
                 .yidam/config.toml. A file:// vault reaches no network and an s3:// one does, \
                 so the policies differ"
            )
        })?;
    Settings::new(
        super::workflow::slug(root),
        cfg.cluster.namespace.clone(),
        &vault_url,
        egress,
    )
}

impl Settings {
    /// The settings for a corpus named `slug` whose vault is at `vault_url`, or why its policies
    /// would stop a pod doing its work.
    pub(super) fn new(
        slug: String,
        namespace: Option<String>,
        vault_url: &str,
        e: ClusterEgressConfig,
    ) -> Result<Self> {
        if e.executor.is_empty() {
            bail!(
                "[cluster.egress] executor is empty. Argo's executor sidecar reports each pod's \
                 output through the Kubernetes API from inside the pod, so a policy without it \
                 fails every task. Set it to the API server's endpoint addresses, as \
                 `kubectl get endpoints kubernetes -n default` lists them, each as a /32, or \
                 pass `--executor`"
            );
        }
        if let Some(whole) = e
            .remote
            .iter()
            .find(|c| parse_cidr(c).is_ok_and(|(_, l)| l == 0))
        {
            bail!(
                "[cluster.egress] remote holds {whole:?}, the whole address space, which would \
                 leave the internet pods nothing. Leave `remote` unset to mean \"not declared\""
            );
        }
        let file = vault_url.trim().starts_with("file://");
        if file && !e.sts.is_empty() {
            bail!(
                "[cluster.egress] sts is set, and the vault is {vault_url}, a volume that needs \
                 no credentials. Remove it, so no pod is let reach an address nothing uses"
            );
        }
        let vault = match file {
            true if !e.vault.is_empty() => bail!(
                "[cluster.egress] vault is set, and the vault is {vault_url}, a volume that \
                 reaches no network. Remove it, so no pod is let reach an address nothing uses"
            ),
            true => None,
            false if e.vault.is_empty() => bail!(
                "[cluster.egress] vault is empty, and the vault is {vault_url}, which every pod \
                 reaches over the network. Set it to the CIDRs its endpoint resolves into, or \
                 pass `--vault-cidr`. On AWS, an S3 interface endpoint gives the bucket private \
                 addresses in your own subnets, and those subnets are the CIDRs. Without one, \
                 the region's S3 prefixes are in ip-ranges.json, under service \"S3\". A gateway \
                 endpoint has no addresses of its own, so it does not help here. A policy that \
                 let every pod reach port 443 anywhere would let a step pod reach the remote"
            ),
            false => Some(e.vault),
        };
        Ok(Self {
            slug,
            namespace,
            vault,
            executor: e.executor,
            remote: e.remote,
            sts: e.sts,
        })
    }

    /// The corpus-wide policy's name, and each kind's after it.
    pub(super) fn name(&self, kind: Option<Egress>) -> String {
        match kind {
            None => format!("yidam-{}-egress", self.slug),
            Some(k) => format!("yidam-{}-egress-{}", self.slug, k.label()),
        }
    }
}

pub(super) fn generate(root: &Path, o: &Overrides) -> Result<String> {
    Ok(policies(&resolve(root, o)?))
}

/// Every policy for one corpus, as YAML documents.
pub(super) fn policies(s: &Settings) -> String {
    let mut y = String::new();
    y.push_str(
        "# Generated by `yidam cluster network-policy`. Regenerate rather than edit.\n\
         #\n\
         # What each of this corpus's pods may reach (#1232). The workflow labels every pod\n\
         # with its corpus and its kind. The first policy selects the corpus alone: DNS and the\n\
         # API server, which Argo's executor sidecar reports through, and nothing else. Each\n\
         # policy after it selects one kind and adds what that kind needs. Policies add up, so\n\
         # a pod reaches what the first allows and what its own kind's allows.\n\
         #\n\
         # Only the lander mounts the write key, and that mount is the invariant. These keep a\n\
         # key that reached another pod by some other route from pushing anywhere: a step pod,\n\
         # where a calculator runs, reaches the vault and nothing else.\n\
         #\n\
         # A CNI that enforces NetworkPolicy is required. kind's default one does not.\n",
    );
    document(&mut y, s, None);
    y.push_str("  egress:\n");
    y.push_str(
        "    # DNS, wherever the cluster serves it from.\n    \
         - ports:\n        - protocol: UDP\n          port: 53\n        \
         - protocol: TCP\n          port: 53\n",
    );
    y.push_str("    # The API server, for the executor sidecar. [cluster.egress] executor.\n");
    to(&mut y, &s.executor);

    for &kind in Egress::ALL {
        y.push_str("---\n");
        document(&mut y, s, Some(kind));
        let mut rules = String::new();
        match kind {
            Egress::Remote if s.remote.is_empty() => rules.push_str(
                "    # The remote, which [cluster.egress] remote does not place, so anywhere.\n    \
                 - {}\n",
            ),
            Egress::Remote => {
                rules.push_str("    # The remote. [cluster.egress] remote.\n");
                to(&mut rules, &s.remote);
            }
            Egress::Vault => {}
            Egress::Internet => {
                rules.push_str(match s.remote.is_empty() {
                    true => {
                        "    # Anywhere. [cluster.egress] remote is not declared, so the remote \
                         is not\n    # excepted, and this pod could reach it with a key.\n"
                    }
                    false => "    # Anywhere but the remote.\n",
                });
                rules.push_str("    - to:\n");
                for (all, v4) in [("0.0.0.0/0", true), ("::/0", false)] {
                    let _ = writeln!(rules, "        - ipBlock:\n            cidr: {}", quote(all));
                    let except: Vec<&String> = s
                        .remote
                        .iter()
                        .filter(|c| parse_cidr(c).is_ok_and(|(a, _)| a.is_ipv4() == v4))
                        .collect();
                    if !except.is_empty() {
                        rules.push_str("            except:\n");
                        for c in except {
                            let _ = writeln!(rules, "              - {}", quote(c));
                        }
                    }
                }
            }
        }
        if let Some(vault) = &s.vault {
            rules.push_str("    # The vault. [cluster.egress] vault.\n");
            to(&mut rules, vault);
            if !s.sts.is_empty() {
                rules.push_str(
                    "    # STS, where a pod exchanges its web identity token for the vault's \
                     credentials.\n    # [cluster.egress] sts.\n",
                );
                to(&mut rules, &s.sts);
            }
        }
        if rules.is_empty() {
            y.push_str(
                "  # Nothing past what every pod reaches: the vault is a volume.\n  egress: []\n",
            );
        } else {
            y.push_str("  egress:\n");
            y.push_str(&rules);
        }
    }
    y
}

/// One policy's header and selector: the corpus's, or one kind's within it.
fn document(y: &mut String, s: &Settings, kind: Option<Egress>) {
    y.push_str("apiVersion: networking.k8s.io/v1\nkind: NetworkPolicy\nmetadata:\n");
    let _ = writeln!(y, "  name: {}", s.name(kind));
    if let Some(ns) = &s.namespace {
        let _ = writeln!(y, "  namespace: {}", quote(ns));
    }
    let _ = writeln!(y, "  labels:\n    {CORPUS_LABEL}: {}", quote(&s.slug));
    y.push_str("spec:\n");
    match kind {
        None => y.push_str("  # Every pod of this corpus.\n"),
        Some(k) => {
            let _ = writeln!(y, "  # This corpus's {} pods.", k.label());
        }
    }
    let _ = writeln!(
        y,
        "  podSelector:\n    matchLabels:\n      {CORPUS_LABEL}: {}",
        quote(&s.slug)
    );
    if let Some(k) = kind {
        let _ = writeln!(y, "      {EGRESS_LABEL}: {}", quote(k.label()));
    }
    y.push_str("  policyTypes: [\"Egress\"]\n");
}

/// One egress rule reaching every CIDR in `cidrs`, on any port.
fn to(y: &mut String, cidrs: &[String]) {
    y.push_str("    - to:\n");
    for c in cidrs {
        let _ = writeln!(y, "        - ipBlock:\n            cidr: {}", quote(c));
    }
}

fn quote(s: &str) -> String {
    super::workflow::quote(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn egress(executor: &[&str], remote: &[&str], vault: &[&str]) -> ClusterEgressConfig {
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect();
        ClusterEgressConfig {
            executor: v(executor),
            remote: v(remote),
            vault: v(vault),
            sts: vec![],
        }
    }

    fn refusal(vault_url: &str, e: ClusterEgressConfig) -> String {
        match Settings::new("c".into(), None, vault_url, e) {
            Ok(_) => panic!("{vault_url} was not refused"),
            Err(e) => e.to_string(),
        }
    }

    /// Each refusal is a policy that would stop a pod doing its work, or let one reach what
    /// nothing in it uses. None of them is a warning, because the cluster would not warn.
    #[test]
    fn a_policy_that_would_break_a_pod_is_refused() {
        let err = refusal("file:///v", egress(&[], &[], &[]));
        assert!(
            err.contains("executor") && err.contains("endpoints"),
            "{err}"
        );

        let err = refusal("s3://bucket/p", egress(&["192.0.2.1/32"], &[], &[]));
        assert!(
            err.contains("interface endpoint") && err.contains("ip-ranges.json"),
            "{err}"
        );

        let err = refusal("file:///v", egress(&["192.0.2.1/32"], &[], &["10.0.0.0/8"]));
        assert!(err.contains("reaches no network"), "{err}");

        let err = refusal("file:///v", egress(&["192.0.2.1/32"], &["0.0.0.0/0"], &[]));
        assert!(err.contains("whole address space"), "{err}");

        let mut e = egress(&["192.0.2.1/32"], &[], &[]);
        e.sts = vec!["10.30.0.0/24".into()];
        let err = refusal("file:///v", e);
        assert!(
            err.contains("sts is set") && err.contains("no credentials"),
            "{err}"
        );
    }

    /// Under a web identity every pod that reads the vault first asks STS for credentials, so
    /// STS rides with the vault into every kind's policy (#1234). Unset, nothing changes: a
    /// corpus on keys gets the policies it had.
    #[test]
    fn sts_is_reached_wherever_the_vault_is() {
        let mut e = egress(&["192.0.2.1/32"], &[], &["10.20.0.0/16"]);
        let without = policies(&Settings::new("c".into(), None, "s3://b/p", e.clone()).unwrap());
        assert!(!without.contains("STS"), "{without}");

        e.sts = vec!["10.30.0.0/24".into()];
        let text = policies(&Settings::new("c".into(), None, "s3://b/p", e).unwrap());
        let docs: Vec<&str> = text.split("\n---\n").collect();
        for doc in &docs[1..] {
            assert!(doc.contains("cidr: \"10.30.0.0/24\""), "{doc}");
        }
        assert!(
            !docs[0].contains("10.30.0.0/24"),
            "the corpus-wide policy widened"
        );
    }

    /// An `s3://` vault's CIDRs reach every kind, since every pod but `admit` reads the vault.
    #[test]
    fn an_s3_vault_is_reached_by_every_kind() {
        let s = Settings::new(
            "c".into(),
            Some("ns".into()),
            "s3://bucket/p",
            egress(&["192.0.2.1/32"], &[], &["10.20.0.0/16"]),
        )
        .unwrap();
        let text = policies(&s);
        let docs: Vec<&str> = text.split("\n---\n").collect();
        assert_eq!(docs.len(), 1 + Egress::ALL.len());
        for doc in &docs[1..] {
            assert!(doc.contains("cidr: \"10.20.0.0/16\""), "{doc}");
            assert!(doc.contains("namespace: \"ns\""), "{doc}");
        }
        assert!(
            !docs[0].contains("10.20.0.0/16"),
            "the corpus-wide policy widened"
        );
    }

    /// With the remote undeclared, a remote pod reaches anywhere, and the internet pods' rule says
    /// it does not except the remote.
    #[test]
    fn an_undeclared_remote_leaves_the_git_pods_open_and_says_so() {
        let s = Settings::new(
            "c".into(),
            None,
            "file:///v",
            egress(&["192.0.2.1/32"], &[], &[]),
        )
        .unwrap();
        let text = policies(&s);
        let git = text
            .split("\n---\n")
            .find(|d| d.contains("egress-remote\n"))
            .unwrap();
        assert!(git.ends_with("    - {}"), "{git}");
        assert!(!text.contains("except:"), "{text}");
        assert!(text.contains("is not\n    # excepted"), "{text}");
    }
}
