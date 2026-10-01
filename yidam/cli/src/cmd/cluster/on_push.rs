//! `cluster workflow --on-push` — an Argo Events `EventSource` and `Sensor` that submit a run
//! when the branch moves (#1235).
//!
//! A cron is the wrong clock for half of what makes a run owed. An edited calculator input
//! or a new catalog entry is stale the moment it is pushed, and it waits for the next tick: a
//! short cron spends admissions on corpora with nothing owed, and a long one leaves stale
//! derivations on the branch for hours.
//!
//! # A push is a reason to ask, not a reason to run
//!
//! #460 decision 8. The Sensor submits the same workflow the cron does, `admit` first, and a
//! push that made nothing stale ends at `admit`. Nothing about the push is handed to the
//! workflow: not the sha, not the pusher. `admit` clones the tip it finds, so a burst of
//! pushes asks one question several times, about the branch as it is.
//!
//! # The lander's own push
//!
//! The lander pushes to the branch, and that push is a push. It triggers a run, and that run
//! is not admitted, because the run that landed it left nothing stale. That answer is what
//! stops the loop. The Sensor deliberately does not filter out the lander's commits by
//! committer: `GIT_COMMITTER_NAME` is a deployment's to set, so a filter on `yidam cluster`
//! is a list that rots silently, and a filter that dropped a push admission would have
//! admitted would leave a stale derivation on the branch until the cron.
//!
//! One run at a time is what makes that answer true. A chain of N steps lands N times, and a
//! run submitted by the first landing would find the later steps stale and race the run that
//! is about to land them. Every generated workflow holds a mutex named for its corpus, so the
//! runs a burst of pushes submits queue behind the one running and ask `admit` after it has
//! finished. The cron form's `concurrencyPolicy: Forbid` only keeps its own ticks apart; the
//! mutex keeps a cron run and a push run apart too.
//!
//! # What these pods hold
//!
//! No git credential and no vault credential. The EventSource holds the secret it checks a
//! push against, and the Sensor's account can create a `Workflow` and nothing else. Neither
//! pod carries the label `yidam cluster network-policy` selects, so neither is fenced by the
//! corpus's policies, and neither can move a ref: the run it submits is the one the
//! generator wrote, and only its `land` mounts the write key.

use std::fmt::Write as _;

use anyhow::Result;

use super::workflow::quote;

/// Where a push is heard from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Source {
    /// GitHub's push webhook, checked against its HMAC signature.
    Github,
    /// Any JSON POST with a bearer token and a top-level `ref`: a GitLab or Gitea push
    /// webhook, or a `post-receive` hook on a remote of your own.
    Webhook,
}

/// The port the EventSource listens on and its Service exposes.
pub(super) const PORT: u16 = 12000;
/// The path a push is posted to.
pub(super) const ENDPOINT: &str = "/push";
/// The key in the webhook secret that holds the shared secret or the token.
pub(super) const SECRET_KEY: &str = "secret";
/// The user the two Argo Events pods run as. Their image is `FROM scratch` with no `USER`, so
/// it runs as root unless the pod says otherwise, which a `restricted` namespace refuses.
const UID: u32 = 1000;

/// What the two objects are generated from.
pub(super) struct OnPush<'a> {
    pub source: Source,
    pub slug: &'a str,
    pub namespace: Option<&'a str>,
    pub remote: &'a str,
    pub branch: &'a str,
    pub events_account: &'a str,
    pub webhook_secret: &'a str,
    /// The `Workflow` the Sensor creates, as YAML: the admitted form, with `generateName`.
    pub workflow: &'a str,
}

/// The EventSource, then the Sensor, as two YAML documents.
pub(super) fn generate(o: &OnPush) -> Result<String> {
    let name = format!("yidam-{}", o.slug);
    let mut y = String::new();

    // ── the EventSource ──
    y.push_str("apiVersion: argoproj.io/v1alpha1\nkind: EventSource\n");
    metadata(&mut y, o, &name);
    y.push_str("spec:\n");
    template(&mut y, None);
    let _ = writeln!(
        y,
        "  # A Service on this port, which an Ingress or a remote's hook reaches.\n  \
         service:\n    ports:\n      - port: {PORT}\n        targetPort: {PORT}"
    );
    // The listener: GitHub's source nests it under `webhook`, the generic source inlines it.
    let listener = |indent: &str| {
        format!("{indent}endpoint: {ENDPOINT}\n{indent}port: \"{PORT}\"\n{indent}method: POST\n")
    };
    match o.source {
        Source::Github => {
            let (owner, repo) = github_repository(o.remote)?;
            let _ = writeln!(
                y,
                "  # No `apiToken`, so Argo Events registers no hook and holds no GitHub \
                 credential.\n  # Add the hook in the repository's settings: the Service's \
                 url, content type\n  # JSON, the push event, and the secret below.\n  \
                 github:\n    push:\n      repositories:\n        - owner: {}\n          \
                 names:\n            - {}",
                quote(&owner),
                quote(&repo)
            );
            y.push_str("      webhook:\n");
            y.push_str(&listener("        "));
            y.push_str(
                "      events:\n        - push\n      contentType: json\n      active: true\n      \
                 insecure: false\n",
            );
            let _ = writeln!(
                y,
                "      # A push whose signature this secret does not verify is dropped.\n      \
                 webhookSecret:\n        name: {}\n        key: {SECRET_KEY}",
                quote(o.webhook_secret)
            );
        }
        Source::Webhook => {
            y.push_str("  webhook:\n    push:\n");
            y.push_str(&listener("      "));
            let _ = writeln!(
                y,
                "      # A request without `Authorization: Bearer <this>` is refused.\n      \
                 authSecret:\n        name: {}\n        key: {SECRET_KEY}",
                quote(o.webhook_secret)
            );
        }
    }

    // ── the Sensor ──
    y.push_str("---\napiVersion: argoproj.io/v1alpha1\nkind: Sensor\n");
    metadata(&mut y, o, &name);
    y.push_str("spec:\n");
    template(&mut y, Some(o.events_account));
    let _ = writeln!(
        y,
        "  dependencies:\n    - name: push\n      eventSourceName: {}\n      eventName: push\n      \
         # This branch only. A push to `propose/*` is the lander proposing, and leaves the\n      \
         # branch where admission reads it.\n      \
         filters:\n        data:\n          - path: body.ref\n            type: string\n            \
         value:\n              - {}",
        quote(&name),
        quote(&branch_pattern(o.branch))
    );
    y.push_str(
        "  triggers:\n    - template:\n        name: admit\n        \
         # The run below starts with `admit`, and a push that made nothing stale ends there.\n        \
         # Nothing from the push is passed in: admission reads the tip it finds.\n        \
         k8s:\n          operation: create\n          source:\n            resource:\n",
    );
    y.push_str(&super::workflow::indent(o.workflow, 14));
    Ok(y)
}

fn metadata(y: &mut String, o: &OnPush, name: &str) {
    let _ = writeln!(y, "metadata:\n  name: {name}");
    if let Some(ns) = o.namespace {
        let _ = writeln!(y, "  namespace: {}", quote(ns));
    }
    let _ = writeln!(y, "  labels:\n    yidam.dev/corpus: {}", quote(o.slug));
}

/// The pod template both objects share: `restricted`, and no label a corpus policy selects.
///
/// No `readOnlyRootFilesystem`: `restricted` does not ask for it, and this is Argo Events'
/// image, not ours, so whether it writes is not ours to promise.
fn template(y: &mut String, account: Option<&str>) {
    y.push_str("  template:\n");
    if let Some(account) = account {
        let _ = writeln!(y, "    serviceAccountName: {}", quote(account));
    }
    let _ = writeln!(
        y,
        "    securityContext:\n      runAsNonRoot: true\n      runAsUser: {UID}\n      \
         runAsGroup: {UID}\n      seccompProfile:\n        type: RuntimeDefault\n    \
         container:\n      securityContext:\n        allowPrivilegeEscalation: false\n        \
         capabilities:\n          drop: [\"ALL\"]"
    );
}

/// The filter value matching `refs/heads/<branch>` exactly. Argo Events reads a string filter
/// as an unanchored regular expression, so `main` alone would match `main-backup` too.
pub(super) fn branch_pattern(branch: &str) -> String {
    let mut p = String::from("^refs/heads/");
    for c in branch.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            p.push('\\');
        }
        p.push(c);
    }
    p.push('$');
    p
}

/// `owner` and `repo` from a GitHub remote, in any of the forms git accepts.
pub(super) fn github_repository(remote: &str) -> Result<(String, String)> {
    let not_hosted = || {
        anyhow::anyhow!(
            "--on-push github reads the repository from the remote, and {remote:?} names no \
             `owner/repo` on a host. Use `--on-push webhook` for a remote GitHub does not host"
        )
    };
    if remote.starts_with(['/', '.']) || remote.starts_with("file://") {
        return Err(not_hosted());
    }
    let path = remote
        .split_once("://")
        .map_or(remote, |(_, rest)| rest)
        .trim_end_matches('/');
    // `git@host:owner/repo` names the path after the colon; a url after the host.
    let path = match path.split_once(':') {
        Some((host, rest)) if !host.contains('/') => rest,
        _ => path.split_once('/').map_or("", |(_, rest)| rest),
    };
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.rsplit('/');
    match (parts.next(), parts.next()) {
        (Some(repo), Some(owner)) if !repo.is_empty() && !owner.is_empty() => {
            Ok((owner.to_string(), repo.to_string()))
        }
        _ => Err(not_hosted()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_github_repository_is_read_from_every_remote_form() {
        for remote in [
            "git@github.com:goedelsoup/streamflow.git",
            "git@github.com:goedelsoup/streamflow",
            "https://github.com/goedelsoup/streamflow.git",
            "https://github.com/goedelsoup/streamflow/",
            "ssh://git@github.com/goedelsoup/streamflow.git",
            "ssh://git@github.com:22/goedelsoup/streamflow.git",
        ] {
            assert_eq!(
                github_repository(remote).unwrap(),
                ("goedelsoup".to_string(), "streamflow".to_string()),
                "{remote}"
            );
        }
        for remote in ["/srv/git/corpus.git", "corpus", "git@github.com:corpus.git"] {
            assert!(github_repository(remote).is_err(), "{remote}");
        }
    }

    fn objects(source: Source) -> Vec<serde_yaml::Value> {
        use serde::Deserialize as _;
        let text = generate(&OnPush {
            source,
            slug: "corpus",
            namespace: Some("yidam"),
            remote: "git@github.com:you/corpus.git",
            branch: "main",
            events_account: "yidam-corpus-events",
            webhook_secret: "yidam-corpus-webhook",
            workflow: "apiVersion: argoproj.io/v1alpha1\nkind: Workflow\nmetadata:\n  \
                       generateName: yidam-corpus-\n",
        })
        .unwrap();
        serde_yaml::Deserializer::from_str(&text)
            .map(|d| serde_yaml::Value::deserialize(d).expect("the output parses"))
            .collect()
    }

    /// #1230 holds for the two pods this adds: a `restricted` namespace admits them.
    #[test]
    fn both_pods_satisfy_the_restricted_profile_and_carry_no_corpus_label() {
        for source in [Source::Github, Source::Webhook] {
            let docs = objects(source);
            let kinds: Vec<&str> = docs.iter().filter_map(|d| d["kind"].as_str()).collect();
            assert_eq!(kinds, ["EventSource", "Sensor"], "{source:?}");
            for d in &docs {
                let t = &d["spec"]["template"];
                let pod = &t["securityContext"];
                assert_eq!(pod["runAsNonRoot"].as_bool(), Some(true));
                assert_ne!(pod["runAsUser"].as_u64(), Some(0));
                assert!(pod["runAsUser"].is_u64());
                assert_eq!(
                    pod["seccompProfile"]["type"].as_str(),
                    Some("RuntimeDefault")
                );
                let c = &t["container"]["securityContext"];
                assert_eq!(c["allowPrivilegeEscalation"].as_bool(), Some(false));
                assert_eq!(
                    c["capabilities"]["drop"],
                    serde_yaml::from_str::<serde_yaml::Value>("[ALL]").unwrap()
                );
                // A corpus label on the pod would put it under the corpus's NetworkPolicies,
                // which allow DNS and the API server and would cut it off from the event bus.
                assert!(t.get("metadata").is_none(), "{:?}", d["kind"]);
                assert_eq!(d["metadata"]["namespace"].as_str(), Some("yidam"));
            }
        }
    }

    #[test]
    fn the_sensor_creates_the_workflow_on_a_push_to_the_branch_alone() {
        let docs = objects(Source::Github);
        let sensor = &docs[1];
        assert_eq!(
            sensor["spec"]["template"]["serviceAccountName"].as_str(),
            Some("yidam-corpus-events")
        );
        let dep = &sensor["spec"]["dependencies"][0];
        assert_eq!(dep["eventSourceName"], docs[0]["metadata"]["name"]);
        assert!(docs[0]["spec"]["github"][dep["eventName"].as_str().unwrap()].is_mapping());
        let filter = &dep["filters"]["data"][0];
        assert_eq!(filter["path"].as_str(), Some("body.ref"));
        assert_eq!(filter["value"][0].as_str(), Some("^refs/heads/main$"));
        let k8s = &sensor["spec"]["triggers"][0]["template"]["k8s"];
        assert_eq!(k8s["operation"].as_str(), Some("create"));
        assert_eq!(k8s["source"]["resource"]["kind"].as_str(), Some("Workflow"));
        assert!(
            sensor["spec"]["triggers"][0]["template"]
                .get("parameters")
                .is_none()
                && sensor["spec"]["triggers"][0].get("parameters").is_none(),
            "nothing from the push reaches the run: admission reads the tip itself"
        );
    }

    #[test]
    fn each_source_checks_the_push_against_the_webhook_secret() {
        let gh = &objects(Source::Github)[0]["spec"]["github"]["push"];
        assert_eq!(
            gh["webhookSecret"]["name"].as_str(),
            Some("yidam-corpus-webhook")
        );
        assert_eq!(gh["webhookSecret"]["key"].as_str(), Some(SECRET_KEY));
        assert_eq!(gh["repositories"][0]["owner"].as_str(), Some("you"));
        assert!(gh.get("apiToken").is_none(), "no GitHub credential is held");
        let wh = &objects(Source::Webhook)[0]["spec"]["webhook"]["push"];
        assert_eq!(
            wh["authSecret"]["name"].as_str(),
            Some("yidam-corpus-webhook")
        );
        assert_eq!(wh["endpoint"].as_str(), Some(ENDPOINT));
        assert!(
            wh.get("webhook").is_none(),
            "the generic source inlines its listener"
        );
        assert_eq!(gh["webhook"]["endpoint"].as_str(), Some(ENDPOINT));
    }

    #[test]
    fn the_branch_filter_matches_its_branch_and_nothing_else() {
        let p = branch_pattern("release/1.x");
        assert_eq!(p, r"^refs/heads/release/1\.x$");
        assert_eq!(branch_pattern("main"), "^refs/heads/main$");
    }
}
