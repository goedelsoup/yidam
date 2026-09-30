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

| Task | Command | Reads | Writes | Git secret |
|---|---|---|---|---|
| `admit` | `yidam cluster admit` | the remote | a record | `yidam-git-read` |
| `pin` | `yidam cluster pin` | the remote | a bundle, to the vault | `yidam-git-read` |
| `step-<name>` | `yidam cluster step <name>` | a bundle, from the vault | a bundle, to the vault | none |
| `land-<name>` | `yidam cluster land` | a bundle, from the vault | one ref, on the remote | `yidam-git-write` |
| `survey-<g>` | `yidam cluster survey <g>` | the pin, from the vault | a plan: one ask per peer | none |
| `ask-<g>` | `yidam cluster ask` | a peer's bundle, from the vault or its lock url | a record, to the vault | none |
| `gather-<g>` | `yidam cluster gather <g>` | the pin and the records | a bundle, to the vault | none |
| `land-gather-<g>` | `yidam cluster land` | a bundle, from the vault | `propose/gather/<g>/<pin>` | `yidam-git-write` |

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

The admission task runs `yidam cluster admit`. Its record says `admitted`. The pin task and
everything after it carry a `when` on that field. A clock that is not owed submits no step.
`concurrencyPolicy: Forbid` keeps two runs of one corpus apart.

The worked example is [cluster/streamflow.workflow.yml](cluster/streamflow.workflow.yml),
generated from `examples/streamflow` and pinned by a test. The cron form is beside it.

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

Build from [cluster/Dockerfile](cluster/Dockerfile) to run a commit that is not released, or to
change the features the binary carries. Build from the repository root, because the CLI crate
has two path dependencies beside it.

```sh
docker build -f docs/cluster/Dockerfile -t ghcr.io/you/yidam-cluster:<version> .
```

A digest of your own build names an image only you can pull. A receipt that records it tells
a reader which image ran, but not how to get it.

## Secrets and the service account

Apply [cluster/argo-rbac.yml](cluster/argo-rbac.yml). It creates the `yidam-run` service account
and the one role Argo's executor needs, which is to record task results. Pods do not mount the
account's token. The executor sidecar uses it, the main container never sees it.

Two git secrets, each holding a deploy key and a known-hosts file:

```sh
kubectl create secret generic yidam-git-read \
  --from-file=key=./read-only-deploy-key --from-file=known_hosts=./known_hosts
kubectl create secret generic yidam-git-write \
  --from-file=key=./read-write-deploy-key --from-file=known_hosts=./known_hosts
```

The read key is a deploy key without write access. The write key is a deploy key with it.
Only the lander mounts the second. That mount is the invariant. A test tries to break it. It
hands a valid commit to a process that cannot write, and checks that the remote refuses.

## Two vault backends

The vault carries every bundle between pods. Either backend works, and the workflow differs by
one volume.

**`file://`, no credentials at all.** Declare a URL under a mount. The generated workflow
mounts a `PersistentVolumeClaim` named `yidam-vault` at that path on every pod. Create the
claim with `ReadWriteMany` access, since pin, step and land run on any node. Nothing in it is a
working tree. Pods write new digests and read old ones. This is not a shared checkout.

```toml
[cluster]
vault = "default"

[vault.default]
url = "file:///var/yidam/vault"
```

**`s3://`.** Declare the bucket the way [artifact-vaults.md](artifact-vaults.md) declares one.
Put the credentials in a secret named `yidam-vault`. Every pod reads it as environment. The
variable names are the ones the vault already reads, prefixed by the vault's name:

```sh
kubectl create secret generic yidam-vault \
  --from-literal=YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID=... \
  --from-literal=YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY=...
```

The secret is optional in the manifest, so a `file://` deployment creates none.

## Pod logs are not provenance

Argo keeps a log per pod. Read it when a step fails. Do not cite it. A calculator's stderr is
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
  exits. The clone is gone with it.

## When something looks wrong

Run the same commands by hand against the same remote and vault. Each one is a pod, and each
one works from a shell.

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
