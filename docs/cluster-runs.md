# Running a corpus on a cluster

`yidam run` invokes the capability manifest on one machine and lands each result as a commit.
`yidam cluster` is the same run, split into pods on Argo Workflows. Each pod does one act. The
acts pass records between them, never a checkout. This page is how to deploy it.

The design is RFC-0026 §7 and the parent epic, #460. Two sentences carry it. **A run authors
operational commits and only proposes epistemic ones. No pod can break that rule, because the
pod that computes holds no credential that could.**

## What runs where

A workflow is a pin, then a step and a landing for each capability, in dependency order.
The catalog's own steps come first in every workflow: `catalog-fetch`, `catalog-extract` and
`catalog-reconcile`.

Every object a workflow names is its corpus's own. The table shortens `yidam-<corpus>-git-read`
to `…-git-read`.

| Task | Command | Reads | Writes | Git secret |
|---|---|---|---|---|
| `admit` | `yidam cluster admit` | the remote | a record | `…-git-read` |
| `pin` | `yidam cluster pin` | the remote | a bundle, to the vault | `…-git-read` |
| `step-<name>` | `yidam cluster step <name>` | a bundle, from the vault | a bundle, to the vault | none |
| `land-<name>` | `yidam cluster land` | a bundle, from the vault | one ref, on the remote | `…-git-write` |
| `survey-<g>` | `yidam cluster survey <g>` | the pin, from the vault | a plan: one ask per peer | none |
| `ask-<g>` | `yidam cluster ask` | a peer's bundle, from the vault or its lock url | a record, to the vault | none |
| `gather-<g>` | `yidam cluster gather <g>` | the pin and the records | a bundle, to the vault | none |
| `land-gather-<g>` | `yidam cluster land` | a bundle, from the vault | `propose/gather/<g>/<pin>` | `…-git-write` |

The step pod has no `--remote` flag and no `--branch` flag. It fetches a bundle, clones it into
scratch, runs the capability there and bundles what it built. Its record names the commit's
sha, its class, its verb and its receipt path. A sha is not a permission. The pod cannot land
it, and the manifest cannot give it a field that would.

The lander is the one component that may update a ref. It reads the class off the commit, not
off the record. An operational commit goes to the branch. An epistemic one goes to
`propose/<input>`, and the branch stays where it was. The push uses `--force-with-lease`. If
the branch moved, the lander rebuilds the commit on the new tip. That holds only when nothing
the step reads moved. Otherwise it refuses.

Each file in `.yidam/gathers/` adds the last four rows, after the last capability. One `ask` pod
runs per peer, and a failed one does not stop the rest. Its peer is reported `refused`. The
lander refuses a gather commit that is not `open:`. It also refuses one that touches anything
but the gather's own nodes and receipts.

## Generate the manifest

Run it in a checkout. The output is a complete Argo `Workflow`.

```sh
yidam cluster workflow --remote git@github.com:you/corpus.git \
  --image ghcr.io/goedelsoup/yidam-cluster@sha256:<hex> \
  --vault-url file:///var/yidam/vault > yidam.workflow.yml
```

Each flag overrides one `[cluster]` key in `.yidam/config.toml`, so a corpus that declares them
runs `yidam cluster workflow` alone. See [configuration.md](configuration.md#cluster).

With `--cron` the output is a `CronWorkflow` that admits itself first:

```sh
yidam cluster workflow --cron "0 6 * * *" > yidam.cronworkflow.yml
```

The admission task runs `yidam cluster admit`. Its record says `admitted`. The pin task carries a
`when` on that field, and every task after it waits for the one before to succeed. A clock that is
not owed runs no step, and the workflow still ends `Succeeded`.

Every form holds one mutex per corpus, `yidam-<corpus>`. Two runs of one corpus never land at
once. A second run waits, then asks `admit` about the tip the first one left. The field is
`synchronization.mutexes`, which needs Argo Workflows 3.6 or later. The cron form lists its time under
`schedules`, since Argo Workflows 4 refuses the older `schedule` field.

With `--on-push` the output adds an Argo Events `EventSource` and `Sensor`. A push to the
branch submits the same run, which admits first. See [Run on push](#run-on-push).

The worked example is [streamflow.workflow.yml](../yidam/cluster/overlays/streamflow/streamflow.workflow.yml),
generated from `examples/streamflow` and pinned by a test. The cron form is beside it.
The push form is in [streamflow-on-push](../yidam/cluster/overlays/streamflow-on-push/streamflow.onpush.yml).

## What admission reads

`admit` clones the branch and asks three questions.

- **Is a clock due?** `due`'s clocks, exactly as `yidam due` reads them. `Undeclared` is not
  owed.
- **Is a step stale?** `yidam run --dry-run`'s answer, over the manifest at the tip. `due` has
  no clock for a receipt that no longer matches, so admission asks here.
- **How many proposals are open?** The count of `propose/*` branches on the remote, against
  `[cluster] max_open_proposals`. A run that would propose while the last proposal stands
  unmerged is not admitted. No cap declared means no cap, and the record says so.

The exit code is zero either way. A corpus with nothing owed is its ordinary state.

## The image

Each `cli/v*` release publishes `ghcr.io/goedelsoup/yidam-cluster:<version>` for `linux/amd64`
and `linux/arm64`. The image is the `yidam` binary of that release, git and an SSH client on a
slim Debian base. Nothing else runs in a pod.

Pass `--image` by digest to record which image ran. Each step receipt then carries
`image_digest`. A tag can move, so a tag records nothing. Read the digest from the registry:

```sh
docker buildx imagetools inspect ghcr.io/goedelsoup/yidam-cluster:<version> \
  --format '{{json .Manifest}}' | jq -r .digest
yidam cluster workflow --image ghcr.io/goedelsoup/yidam-cluster@sha256:<hex> > yidam.workflow.yml
```

`--image` has no default, so the generator refuses without it or `[cluster] image`.

Every published image carries an SBOM for each platform and a signed build-provenance
attestation. Check where an image came from before a cluster runs it:

```sh
gh attestation verify oci://ghcr.io/goedelsoup/yidam-cluster@sha256:<hex> --repo goedelsoup/yidam
```

A pod holds no git identity. The commits it builds carry `yidam run` as author and
`yidam cluster <cluster@yidam>` as committer. Set `GIT_COMMITTER_NAME` and
`GIT_COMMITTER_EMAIL` on the containers to name your own.

### Building your own

Build from [yidam/cluster/Dockerfile](../yidam/cluster/Dockerfile) to run a commit that is not released, or to
change the features the binary carries. Build from the repository root, because the CLI crate
has two path dependencies beside it.

```sh
docker build -f yidam/cluster/Dockerfile -t ghcr.io/you/yidam-cluster:<version> .
```

A digest of your own build names an image only you can pull. A receipt that records it tells
a reader which image ran, but not how to get it.

## Deploy a corpus

Each name below carries the corpus's name, so two corpora can share a namespace. The corpus
name is its root directory's, lowercased. The manifest's header names both git secrets.

| Object | Kind | Name |
|---|---|---|
| Run account | ServiceAccount | `yidam-<corpus>-run` |
| Read key | Secret | `yidam-<corpus>-git-read` |
| Write key | Secret | `yidam-<corpus>-git-write` |
| S3 vault keys | Secret | `yidam-<corpus>-vault` |
| File vault | PersistentVolumeClaim | `yidam-<corpus>-vault` |
| Sensor account (on push) | ServiceAccount | `yidam-<corpus>-events` |
| Webhook secret (on push) | Secret | `yidam-<corpus>-webhook` |

One Kustomize overlay creates all of them, beside the workflow that names them. The worked
example is [yidam/cluster/overlays/streamflow](../yidam/cluster/overlays/streamflow/kustomization.yaml).
A test builds it and checks that it creates exactly what the workflow names.

### Make the overlay

Copy the example into your corpus's repository. It has two layers:

```text
deploy/
  kustomization.yaml         # resources: objects, the workflow and the policies
  <corpus>.cronworkflow.yml  # yidam cluster workflow --cron output
  <corpus>.netpol.yml        # yidam cluster network-policy output
  objects/
    kustomization.yaml       # the prefix, the base, the vault, the server and the git secrets
    serve.patch.yml          # which bundle the server serves
    .gitignore               # secrets/
    secrets/                 # your keys, never committed
```

In `objects/kustomization.yaml`, set the prefix to your corpus's: `namePrefix: yidam-<corpus>-`.
Set the `yidam.dev/corpus` label beside it to `<corpus>`.

Your repository has no copy of the base, so name it by URL. Pin it to the release your image runs:

```yaml
resources:
  - https://github.com/goedelsoup/yidam//yidam/cluster/base?ref=cli/v<version>
components:
  - https://github.com/goedelsoup/yidam//yidam/cluster/components/file-vault?ref=cli/v<version>
  - https://github.com/goedelsoup/yidam//yidam/cluster/components/serve?ref=cli/v<version>
```

Leave `serve` out if nothing will read the corpus over MCP.

The first release to carry the base is the one after `cli/v0.17.0`.

Then write the workflow into the overlay:

```sh
yidam cluster workflow --cron "0 6 * * *" > deploy/<corpus>.cronworkflow.yml
```

### Add the keys

Put three files in `objects/secrets/`:

- `git-read.key` is a deploy key without write access.
- `git-write.key` is a deploy key with write access.
- `known_hosts` holds the git host's key.

Only the lander mounts the write key. That mount is the invariant. A test tries to break it. It
hands a valid commit to a process that cannot write, and checks that the remote refuses.

### Push as a GitHub App

A deploy key never expires. On GitHub, the lander can push with a token that lasts an hour
instead. Deploy keys stay the default until this has run on a real cluster (#1237).

Create a GitHub App with one permission, **Contents: read and write**. Install it on the
corpus's repository, and generate a private key. Then set in `.yidam/config.toml`:

```toml
[cluster]
git_auth = "github-app"

[cluster.github_app]
app_id = 123456
```

Put the App's key in `objects/secrets/` as `private-key.pem`, in place of `git-write.key`. The
write secret then holds `private-key.pem` and `known_hosts`. Regenerate the workflow.

The lander reads the key and signs a JWT. It asks GitHub for a token scoped to this one
repository. Then it pushes over HTTPS. The token is never a workflow parameter, so Argo never
records it. Git gets it through its environment, never its argv or the clone's config. A test
lands a commit this way and finds the token in no output, record, receipt or file.

The read pods still clone over SSH with `git-read.key`. Only `land` changes.

### Apply it

```sh
kubectl apply -k deploy -n <namespace>
```

A missing key file fails the build and names the file.

Two lines in the example matter, so keep them:

- `disableNameSuffixHash: true`. Otherwise Kustomize adds a hash to each secret's name. The
  workflow mounts the plain name, so no pod would start.
- The workflow is listed above `objects/`, not inside it. The prefix would rename it too.

The run account can do one thing: record task results for Argo's executor. Pods do not mount its
token. The executor sidecar uses it, and the main container never sees it.

The base also creates that token, as the secret `yidam-<corpus>-run.service-account-token`.
Argo mounts it into the executor alone. Kubernetes 1.24 and later create no such secret, so
without it every pod waits in `Init`.

### Run once

A one-shot `Workflow` cannot go in the overlay. It has `generateName` and no name, and Kustomize
refuses it. Apply the overlay first, then create the run:

```sh
yidam cluster workflow > yidam.workflow.yml
kubectl create -f yidam.workflow.yml -n <namespace>
```

A corpus with no schedule lists `objects` alone in its top `kustomization.yaml`.

### Run on push

A cron runs on the clock. A corpus edited by people wants a run when they push. With
`--on-push`, a push to the branch submits the run, and the run asks `admit` first.

```sh
yidam cluster workflow --on-push github > deploy/<corpus>.onpush.yml
```

The output is two objects. The `EventSource` listens for the push on port 12000 at `/push`. The
`Sensor` creates the run when the push's `ref` is `refs/heads/<branch>`. A push to `propose/*`
creates nothing.

The source is one of two:

- `github` checks GitHub's signature against the webhook secret. It reads the owner and the
  repository from `--remote`.
- `webhook` takes any `POST` with `Authorization: Bearer <secret>`. Its body must carry
  `{"ref": "refs/heads/<branch>"}`. Use it for a host other than GitHub.

The cluster supplies three things this does not generate:

- Argo Events, installed.
- An `EventBus` named `default` in the corpus's namespace.
- A route from the git host to the `EventSource`'s Service, such as an Ingress.

Argo Events registers no hook here, so it holds no GitHub token. Add the hook yourself: the
route's URL, content type JSON, the push event, and the secret.

The worked example is [streamflow-on-push](../yidam/cluster/overlays/streamflow-on-push/kustomization.yaml).
It is the streamflow overlay plus three things: the Sensor's account, its role, and the
webhook secret. Put the secret in `objects/secrets/webhook.secret`. It is a separate overlay
because the two kinds exist only where Argo Events is installed.

The Sensor's account may create a `Workflow` and nothing else. The run it creates runs as
`yidam-<corpus>-run`, like the cron's. A forged push could submit a run, and the run would
find nothing owed.

#### The lander's own push

The lander pushes to the branch, so the lander's push is a push too. It submits a run. That run
asks `admit`, finds every receipt matching the tip, and stops. A test drives this through.
A person's push is admitted, the chain lands, and the lander's last push is not.

The Sensor does not filter on the committer. The committer is a name a deployment sets. A
filter on it breaks silently when the name changes. Admission reads what is owed, which is
the actual question.

A chain of several steps lands several times. Each landing is a push, and each push submits a
run. The mutex queues those runs behind the one landing. Each asks `admit` after it finishes,
and finds nothing owed. Without the mutex, a run submitted mid-chain would be admitted and
race the first.

### Serve what a run landed

The `serve` component adds a Deployment and a Service. The pod runs `yidam serve --mcp --http
--bundle <digest>` over the vault, on port 8787. It is
[read-only by construction](mcp-server.md#serving-a-bundle-read-only-by-construction). It mounts
no git key and the vault read-only. Its security context is a step pod's.

`/healthz` is its liveness and startup probe, and `/readyz` its readiness probe. Neither needs a
token, and neither reads the corpus.

The example starts it at zero replicas, because no bundle exists before the first run. The last
`land` step's output names the landed bundle as `next.bundle`. Set it, then scale up:

```sh
kubectl set env deployment/yidam-streamflow-serve YIDAM_SERVE_BUNDLE=sha256:<digest> -n <namespace>
kubectl scale deployment/yidam-streamflow-serve --replicas=1 -n <namespace>
```

The pod carries the corpus label, so the corpus's egress policy holds it to DNS. Put TLS and a
token in front of it before it leaves the cluster; see
[A bearer token](mcp-server.md#a-bearer-token). An `s3://` corpus patches its `--vault-url` and
vault keys in, as it does for a step.

### A restricted namespace

Every generated pod meets the `restricted` Pod Security Standard. A namespace may enforce it:

```sh
kubectl label namespace <namespace> pod-security.kubernetes.io/enforce=restricted
```

Each pod runs as uid 1000. Its root filesystem is read-only, and it holds no capabilities.
It writes to three places only:

- `/tmp/yidam`, an `emptyDir` and the pod's `TMPDIR`. The clone and the pod's record go there.
- `/home/yidam`, an `emptyDir` and the pod's `HOME`. Git, ssh and the vault cache write there.
- A `file://` vault's claim. The pod's `fsGroup` makes it writable.

A test checks each template against the profile's fields. No pod has yet run in an enforcing
namespace, so a missing writable path would show only on a cluster.

Argo adds its own `init` and `wait` containers to each pod. Their container settings come from
the controller's `executor` config, not from this manifest. They must also drop all
capabilities and forbid privilege escalation.

### Resources, deadlines and cleanup

Every pod declares CPU and memory requests and limits, so a namespace with a `ResourceQuota`
admits it. Every pod has a deadline. A hung calculator fails at it rather than holding the
corpus's one run slot. The sizes are set in `[cluster.pod]`. See
[configuration.md](configuration.md#cluster).

A calculator that knows its own cost sets the same keys in `.yidam/capabilities.toml`:

```toml
[capability.travel-tier.cluster]
memory_limit     = "8Gi"
deadline_seconds = 7200
```

That step then runs from its own template, `step-travel-tier`. No other pod changes. A key it
leaves unset comes from `[cluster.pod]`.

Argo retries `pin`, `survey` and `step` twice on failure. Each is safe to repeat: it reads a
pinned bundle and moves no ref. It never retries `land`. The lander retries its own
compare-and-swap. When it refuses, the refusal is the answer, and the next admission runs the
step again.

Argo deletes a pod that succeeded at once. A failed pod stays until its workflow is deleted,
a week after it ends. A succeeded workflow is deleted after a day. `[cluster.cleanup]` sets
all three.

### Limit what each pod reaches

Only the lander mounts the write key. A NetworkPolicy makes that a network fact too. Say a key
reached a step pod by some other route. It still could not push, since a step pod reaches the
vault and nothing else.

The workflow labels every pod with its corpus, `yidam.dev/corpus`, and its kind,
`yidam.dev/egress`:

| Kind | Pods | Reaches |
|---|---|---|
| `remote` | `admit`, `pin`, `land-*` | the remote |
| `vault` | `step-*`, `survey-*`, `gather-*`, `catalog-extract`, `catalog-reconcile` | the vault |
| `internet` | `catalog-fetch`, `ask-*` | anywhere but the remote |

Every pod also reaches DNS and the API server. Argo's `wait` sidecar shares the pod's network
and records task results through the API. `catalog-fetch` runs from its own template,
`step-catalog-fetch`, since it alone among the steps reads the internet.

A policy matches addresses, not hosts. Set them in `[cluster.egress]`. See
[configuration.md](configuration.md#cluster). Then write the policies into the overlay:

```sh
kubectl get endpoints kubernetes -n default   # the executor's addresses
yidam cluster network-policy > deploy/<corpus>.netpol.yml
```

List the file in the top `kustomization.yaml`, beside the workflow. Policies are standing
objects, so a one-shot run uses them too.

- **`executor`** is the API server's endpoint addresses, not the `kubernetes` service's.
  Policies match after the service is resolved. The generator refuses without them.
- **`remote`** is optional. Unset, the `remote` pods reach anywhere, and `catalog-fetch` and
  `ask` can reach the remote. They hold no key, so that is a weaker fence, not a hole. The
  generated file says which you chose. A lander pushing as a GitHub App also calls the API.
  Include its addresses too.
- **`vault`** is for an `s3://` vault. Every kind reaches it. The generator refuses an
  `s3://` vault without it. On AWS, use an S3 interface endpoint's subnet CIDRs, or the S3
  prefixes for your region from `ip-ranges.json`. A gateway endpoint does not help, since it
  gives S3 no address of its own. A `file://` vault is a mount, so it needs none.
- **`sts`** is for pods that assume a role by web identity. Every kind reaches it with the
  vault. Use the regional STS interface endpoint's subnet CIDRs. Pods on keys need none.

Only a CNI that enforces NetworkPolicy enforces them, such as Calico or Cilium. kind's default
one does not. A cluster without one applies the policies and enforces nothing.

### Names you set yourself

To keep names you already have, set them under `[cluster.names]`. See
[configuration.md](configuration.md#cluster). A prefix cannot produce those names, so patch
them in `objects/` instead. The test covers the derived names only.

## Two vault backends

The vault carries every bundle between pods. Either backend works, and the workflow differs by
one volume.

**`file://`, no credentials at all.** Declare a URL under a mount. The generated workflow
mounts a `PersistentVolumeClaim` named `yidam-<corpus>-vault` at that path on every pod. The
`file-vault` component creates it with `ReadWriteMany` access, since pin, step and land run on
any node. Nothing in it is a working tree. Pods write new digests and read old ones.

```toml
[cluster]
vault = "default"

[vault.default]
url = "file:///var/yidam/vault"
```

The claim names no storage class. Patch one in if your default class cannot share a volume
across nodes.

**`s3://`.** Declare the bucket the way [artifact-vaults.md](artifact-vaults.md) declares one.
Drop the `file-vault` component, and generate the vault's secret in `objects/` instead:

```yaml
secretGenerator:
  - name: vault
    envs: [secrets/vault.env]
```

Every pod reads it as environment. The variable names are the ones the vault already reads,
prefixed by the vault's name:

```sh
YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID=...
YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY=...
```

The secret is optional in the manifest, so a `file://` deployment creates none.

**On EKS, a role instead of keys.** With IRSA, annotate the workflow's service account,
`yidam-<corpus>-run`, with the role, as a patch in `objects/`:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: run
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/yidam-corpus
```

EKS then projects a token into every pod and sets `AWS_ROLE_ARN`. The `default` vault
assumes that role, and no secret holds a key. A second vault never inherits it. Give it
`YIDAM_VAULT_<NAME>_ROLE_ARN` and `…_WEB_IDENTITY_TOKEN_FILE`, or keys, in the secret. Every
pod calls STS before the vault, so a fenced cluster also needs `[cluster.egress] sts`.

## Pod logs are not provenance

Argo keeps a log per pod until the pod is deleted. Read it when a step fails. Do not cite it.
[`cluster status`](#when-something-looks-wrong) reads the records instead. A calculator's stderr is
passed through to the pod's own and recorded nowhere. Everything that matters is in the commit
the lander pushed. That is the outputs, and the receipt at `.yidam/runs/<step>.yml`. The
receipt names the input state it was computed from. A log can be rotated, truncated or lost.
A receipt is on the ref.

## Three things this does not build

- **No shared volume holding a corpus.** Pods exchange bundles through a content-addressed
  store. A `file://` vault on a PVC holds digests, never a working tree.
- **No operator.** Argo's DAG is the dependency graph. The manifest is generated, and a corpus
  regenerates it when its capabilities change.
- **No long-lived pod holding a checkout.** Each pod clones into scratch, does one act, and
  exits. The clone is gone with it. The server pod lives long, but it holds a bundle at one
  digest, not a checkout.

## Checked on a real cluster

`mise run cluster-e2e` runs streamflow on a kind cluster, under Calico and Argo Workflows 4.1.
It needs Docker, and builds the image from the checkout. The script is
[run.sh](../yidam/cluster/e2e/run.sh).

It applies the overlay in a namespace that enforces `restricted`. Then it reads the remote, never
a log:

- Each `compute:` commit is on the branch, with its receipt.
- The epistemic step is on `propose/*`, and the branch did not move for it.
- A second submission is not admitted, and moves no ref.
- A step pod holding the write key cannot reach the remote. A lander pod can push with it.

It runs nightly, on a pull request labelled `cluster`, and on any change to the generator or
`yidam/cluster/`. `YIDAM_E2E_KEEP=1` keeps the cluster after the run.

## When something looks wrong

Start from `cluster status`, in a checkout of the corpus.

```sh
yidam cluster status --remote git@host:corpus.git
```

It lists the recent runs, newest first. For each run it says whether admission let it through,
and why. For each step it says where the result went:

- `landed`: on the branch, at the commit it names.
- `proposed`: on `propose/*`, and whether that branch is still open.
- `refused`: the lander declined, and the line gives its reason.
- `fresh` or `unchanged`: there was nothing to land.
- `not reached`: an earlier step stopped the run.

Each gather lists every peer and its outcome, `refused` included.

It reads the `Workflow` objects through `kubectl`, with your current context. `--namespace` and
`--context` pick another. `--workflows <file>` reads saved `kubectl get -o json` output instead.

It never reads a log. Every pod writes a record, and Argo keeps it on the `Workflow`. A refused
landing writes one too. A node with no record is `failed` or `unknown`, and the line says
which record is missing. Argo deletes a finished run after `[cluster.cleanup]`'s delay. Such a
run shows as `unknown`, from its `CronWorkflow`'s last tick. Nothing `status` prints is
committed, and nothing in it is provenance.

Then run the failing act by hand against the same remote and vault. Each one is a pod, and
each one works from a shell.

```sh
yidam cluster admit --remote git@host:corpus.git
yidam cluster pin --remote git@host:corpus.git --vault-url file:///var/yidam/vault --out pin.json
yidam cluster step travel-tier --bundle <digest> --vault-url file:///var/yidam/vault --out step.json
yidam cluster land --step-output @step.json --remote git@host:corpus.git --vault-url file:///var/yidam/vault
```

A step refused with *is not up to date* was handed a pin its upstream had not landed on. The
workflow lands the upstream and pins again before that step, so the order was broken outside
it. A landing refused with *touched what it reads* lost a race to an edit of its inputs. The
next admission sees the step stale at the new tip and runs it there. A landing refused in
git's own words could not write the ref. Check which secret the pod mounted.
