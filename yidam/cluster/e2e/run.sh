#!/usr/bin/env bash
# cluster-e2e: run the streamflow example on a real cluster, and check the remote (#1237, #1235).
#
#   mise run cluster-e2e
#
# Everything else that tests `yidam cluster` calls the library a pod calls, against a bare repo
# on disk. Nothing there shows that Argo accepts the YAML, that each mount lands where its
# command reads it, that the image holds what a pod needs, that the run account's RBAC is
# enough, or that a restricted namespace and the egress policies leave the pods working. This
# script runs the whole thing on kind and finds out.
#
# What it builds:
#
#   - a kind cluster with Calico in place of kindnet, which enforces no NetworkPolicy
#   - Argo Workflows at a pinned version, its executor settings fit for `restricted`
#   - a git server in its own namespace, holding streamflow, with a read key and a write key
#   - the streamflow overlay, as `yidam/cluster/overlays/streamflow` ships it, with a freshly
#     generated CronWorkflow and NetworkPolicy, in a namespace that enforces `restricted`
#   - Argo Events at a pinned version, a JetStream EventBus, the `streamflow-on-push` overlay
#     with a freshly generated `--on-push webhook`, and a hook on the remote that posts each push
#
# streamflow is changed in one place before it is pushed: `disclosure-envelope` establishes
# rather than computes, so the run has an epistemic step to propose (as cluster_run.rs does).
#
# Every check reads the remote or the cluster's record of what ran, never a pod's log. A pod's
# log is not provenance (docs/cluster-runs.md).
#
# Environment:
#
#   YIDAM_E2E_IMAGE      an image already built from yidam/cluster/Dockerfile; built if unset
#   YIDAM_E2E_KEEP=1     leave the cluster up afterwards, to look around
#   YIDAM_E2E_ARTIFACTS  where to write diagnostics on failure (default: a temp dir)
#   YIDAM_E2E_BREAK      a deliberate defect, to show the job can go red:
#                          drop-git-read  submit the run without the git-read mount
set -euo pipefail

ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
HERE="$ROOT/yidam/cluster/e2e"

# Pinned. Argo 3.6 is the floor the generated `synchronization.mutexes` needs.
ARGO_VERSION=v4.1.4
CALICO_VERSION=v3.32.2
ARGO_EVENTS_VERSION=v1.9.11
# A JetStream version that Argo Events release lists in its controller config.
NATS_VERSION=2.10.29

CLUSTER=yidam-e2e
NS=yidam-e2e
CORPUS=streamflow
GIT_HOST=git.git-server.svc.cluster.local
REMOTE="git@$GIT_HOST:/srv/git/$CORPUS.git"
VAULT_URL=file:///var/yidam/vault
BREAK=${YIDAM_E2E_BREAK:-}
RUN_TIMEOUT=${YIDAM_E2E_RUN_TIMEOUT:-1800}
# Longer than the controller takes to start the next pod, which is seconds.
STALL_SECONDS=${YIDAM_E2E_STALL_SECONDS:-180}

WORK=$(mktemp -d "${TMPDIR:-/tmp}/yidam-e2e.XXXXXX")
ARTIFACTS=${YIDAM_E2E_ARTIFACTS:-$WORK/artifacts}
mkdir -p "$ARTIFACTS"

failures=0
log() { printf '\n== %s\n' "$*"; }
pass() { printf 'ok    %s\n' "$*"; }
fail() {
  printf 'FAIL  %s\n' "$*"
  failures=$((failures + 1))
}
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

case "$BREAK" in
  "" | drop-git-read) ;;
  *) die "YIDAM_E2E_BREAK=$BREAK is not a break this script knows" ;;
esac

for tool in docker kind kubectl jq git ssh-keygen tar openssl; do
  command -v "$tool" >/dev/null || die "$tool is not on PATH"
done

# ── teardown ──────────────────────────────────────────────────────────────────

diagnose() {
  log "diagnostics in $ARTIFACTS"
  kubectl get workflows -n "$NS" -o yaml >"$ARTIFACTS/workflows.yml" 2>&1 || true
  kubectl get pods -A -o wide >"$ARTIFACTS/pods.txt" 2>&1 || true
  kubectl get events -A --sort-by=.lastTimestamp >"$ARTIFACTS/events.txt" 2>&1 || true
  kubectl logs -n argo deploy/workflow-controller >"$ARTIFACTS/workflow-controller.log" 2>&1 || true
  kind export logs "$ARTIFACTS/kind" --name "$CLUSTER" >/dev/null 2>&1 || true
}

finish() {
  status=$?
  if [ "$status" -ne 0 ]; then
    diagnose
  fi
  if [ "${YIDAM_E2E_KEEP:-}" = 1 ]; then
    echo "kept: kind cluster $CLUSTER, work dir $WORK"
  else
    kind delete cluster --name "$CLUSTER" >/dev/null 2>&1 || true
    rm -rf "$WORK"
  fi
  exit "$status"
}
trap finish EXIT

# ── images ────────────────────────────────────────────────────────────────────

if [ -n "${YIDAM_E2E_IMAGE:-}" ]; then
  IMAGE=$YIDAM_E2E_IMAGE
else
  # Not `latest`: a pod pulls `latest` on every start, and kind's node has no registry to
  # pull from. Any other tag is used as loaded.
  IMAGE=yidam-cluster:e2e
  log "building $IMAGE"
  docker build -f "$ROOT/yidam/cluster/Dockerfile" -t "$IMAGE" "$ROOT"
fi
log "building the git server"
docker build -q -t yidam-e2e-git:local "$HERE/git-server" >/dev/null

# The generator, run from the image under test rather than a host build: one binary, one
# build, and it is the binary the pods run.
yidam() {
  docker run --rm -v "$WORK/$CORPUS:/$CORPUS:ro" -w "/$CORPUS" \
    -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0='*' \
    "$IMAGE" "$@"
}

# ── cluster ───────────────────────────────────────────────────────────────────

log "creating kind cluster $CLUSTER"
kind delete cluster --name "$CLUSTER" >/dev/null 2>&1 || true
kind create cluster --name "$CLUSTER" --config "$HERE/kind.yaml"
kubectl config use-context "kind-$CLUSTER" >/dev/null

log "installing Calico $CALICO_VERSION"
kubectl apply -f "https://raw.githubusercontent.com/projectcalico/calico/$CALICO_VERSION/manifests/calico.yaml" >/dev/null
kubectl -n kube-system rollout status daemonset/calico-node --timeout=10m
kubectl wait --for=condition=Ready node --all --timeout=5m

kind load docker-image --name "$CLUSTER" "$IMAGE" yidam-e2e-git:local

log "installing Argo Workflows $ARGO_VERSION"
kubectl create namespace argo
kubectl apply -n argo --server-side \
  -f "https://github.com/argoproj/argo-workflows/releases/download/$ARGO_VERSION/install.yaml" >/dev/null
# Argo adds its own `init` and `wait` containers to every pod, and takes their settings from
# here rather than from the workflow. `restricted` requires them too (docs/cluster-runs.md).
kubectl -n argo patch configmap workflow-controller-configmap --type merge -p "$(
  jq -n '{data: {executor: "securityContext:\n  runAsNonRoot: true\n  allowPrivilegeEscalation: false\n  capabilities:\n    drop: [ALL]\n  seccompProfile:\n    type: RuntimeDefault\n"}}'
)"
kubectl -n argo rollout restart deployment/workflow-controller
kubectl -n argo rollout status deployment/workflow-controller --timeout=5m

# ── the remote ────────────────────────────────────────────────────────────────

log "starting the git server"
mkdir -p "$WORK/keys"
ssh-keygen -q -t ed25519 -N '' -C host -f "$WORK/keys/host"
ssh-keygen -q -t ed25519 -N '' -C read -f "$WORK/keys/read"
ssh-keygen -q -t ed25519 -N '' -C write -f "$WORK/keys/write"
kubectl create namespace git-server
kubectl -n git-server create secret generic git-server-keys \
  --from-file=ssh_host_ed25519_key="$WORK/keys/host" \
  --from-file=read.pub="$WORK/keys/read.pub" \
  --from-file=write.pub="$WORK/keys/write.pub"
kubectl -n git-server apply -f "$HERE/git-server.yml"
kubectl -n git-server rollout status deployment/git --timeout=5m
GIT_POD=$(kubectl -n git-server get pod -l app=git -o jsonpath='{.items[0].metadata.name}')
GIT_IP=$(kubectl -n git-server get pod "$GIT_POD" -o jsonpath='{.status.podIP}')
[ -n "$GIT_IP" ] || die "the git server has no pod IP"

# git on the remote, as the remote sees itself. Every assertion below goes through this.
rgit() {
  kubectl -n git-server exec "$GIT_POD" -- \
    git -c safe.directory='*' -C "/srv/git/$CORPUS.git" "$@"
}

log "pushing $CORPUS to the remote"
# The tracked files only, as the test suites materialize an example.
git -C "$ROOT" ls-files -z "examples/$CORPUS" | tar --null -cf - -C "$ROOT" -T - | tar -xf - -C "$WORK"
mv "$WORK/examples/$CORPUS" "$WORK/$CORPUS"
manifest="$WORK/$CORPUS/.yidam/capabilities.toml"
[ -f "$manifest" ] || die "$CORPUS was not copied: no $manifest"
awk '
  /^\[/ { inside = ($0 == "[capability.disclosure-envelope]") }
  inside && !done && /^verb *= *"compute"/ { sub(/"compute"/, "\"establish\""); done = 1 }
  { print }
' "$manifest" >"$manifest.new"
mv "$manifest.new" "$manifest"
# An edit that matched nothing would leave a run with no epistemic step, and the proposal
# assertions would fail for a reason that is not the cluster's.
awk '/^\[/ { inside = ($0 == "[capability.disclosure-envelope]") } inside && /^verb *= *"establish"/ { found = 1 } END { exit !found }' "$manifest" \
  || die "disclosure-envelope was not switched to establish"
(
  cd "$WORK/$CORPUS"
  git init -q -b main
  git add -A
  git -c user.name=Example -c user.email=example@yidam.test commit -qm "genesis: the $CORPUS example"
  git bundle create -q "$WORK/seed.bundle" main
)
kubectl cp "$WORK/seed.bundle" "git-server/$GIT_POD:/tmp/seed.bundle"
kubectl -n git-server exec "$GIT_POD" -- sh -c "
  git clone -q --bare /tmp/seed.bundle /srv/git/$CORPUS.git &&
  git -C /srv/git/$CORPUS.git remote remove origin &&
  chown -R git:git /srv/git/$CORPUS.git"
SEED=$(rgit rev-parse refs/heads/main)
pass "remote seeded at $SEED"

# ── the corpus's namespace ────────────────────────────────────────────────────

log "deploying the $CORPUS overlay into $NS (restricted)"
kubectl create namespace "$NS"
kubectl label namespace "$NS" \
  pod-security.kubernetes.io/enforce=restricted \
  pod-security.kubernetes.io/warn=restricted

# The file-vault component claims `ReadWriteMany` with no storage class, which is the cluster's
# to provide. kind's local-path provisioner offers only `ReadWriteOnce`, so the claim is bound to
# a volume on the one node. The directory belongs to the pods' uid: a hostPath ignores fsGroup.
docker exec "$CLUSTER-control-plane" sh -c 'mkdir -p /var/local/yidam-vault && chown 1000:1000 /var/local/yidam-vault'
DEFAULT_CLASS=$(kubectl get storageclass -o json \
  | jq -r '.items[] | select(.metadata.annotations["storageclass.kubernetes.io/is-default-class"] == "true") | .metadata.name')
kubectl apply -f - <<EOF
apiVersion: v1
kind: PersistentVolume
metadata:
  name: yidam-e2e-vault
spec:
  capacity:
    storage: 10Gi
  accessModes: ["ReadWriteMany"]
  storageClassName: "$DEFAULT_CLASS"
  persistentVolumeReclaimPolicy: Delete
  claimRef:
    namespace: $NS
    name: yidam-$CORPUS-vault
  hostPath:
    path: /var/local/yidam-vault
    type: Directory
EOF

EXECUTOR=$(kubectl get endpointslice -n default -l kubernetes.io/service-name=kubernetes \
  -o jsonpath='{.items[0].endpoints[0].addresses[0]}')
[ -n "$EXECUTOR" ] || die "could not read the API server's endpoint address"

# The shipped overlay, with the two generated files it lists regenerated for this cluster. The
# schedule never comes round in a run; the CronWorkflow is also suspended once applied, and each
# run below is submitted from it.
cp -R "$ROOT/yidam/cluster" "$WORK/cluster"
overlay="$WORK/cluster/overlays/$CORPUS"
yidam cluster workflow --cron "0 0 1 1 *" --image "$IMAGE" --remote "$REMOTE" --vault-url "$VAULT_URL" \
  >"$overlay/$CORPUS.cronworkflow.yml"
yidam cluster network-policy --vault-url "$VAULT_URL" --executor "$EXECUTOR/32" --remote-cidr "$GIT_IP/32" \
  >"$overlay/$CORPUS.netpol.yml"
mkdir -p "$overlay/objects/secrets"
cp "$WORK/keys/read" "$overlay/objects/secrets/git-read.key"
cp "$WORK/keys/write" "$overlay/objects/secrets/git-write.key"
printf '%s %s\n' "$GIT_HOST" "$(cut -d' ' -f1,2 "$WORK/keys/host.pub")" >"$overlay/objects/secrets/known_hosts"
cp "$overlay/$CORPUS.cronworkflow.yml" "$overlay/$CORPUS.netpol.yml" "$ARTIFACTS/"

kubectl apply -k "$overlay" -n "$NS"
kubectl -n "$NS" patch cronworkflow "yidam-$CORPUS" --type merge -p '{"spec":{"suspend":true}}'
kubectl -n "$NS" wait --for=jsonpath='{.status.phase}'=Bound "pvc/yidam-$CORPUS-vault" --timeout=2m

# ── runs ──────────────────────────────────────────────────────────────────────

# A Workflow made from the applied CronWorkflow's spec, which is what `argo submit --from
# cronwf/…` does: the run admits first, exactly as the schedule's would.
submit() {
  kubectl -n "$NS" get cronworkflow "yidam-$CORPUS" -o json \
    | jq --arg brk "$BREAK" '
        {apiVersion, kind: "Workflow",
         metadata: {generateName: "yidam-\(.metadata.labels["yidam.dev/corpus"])-", labels: .metadata.labels},
         spec: .spec.workflowSpec}
        | if $brk == "drop-git-read" then
            .spec.templates |= map(if .container then
              .container.volumeMounts |= map(select(.name != "git-read")) else . end)
          else . end' \
    | kubectl -n "$NS" create -f - -o jsonpath='{.metadata.name}'
}

# Wait for a workflow to end. Prints its phase. A run that never ends is reported as that,
# so a red job says whether it failed or timed out.
#
# A workflow Running with no pod Pending or Running is waiting on nothing: a `when` that never
# resolves leaves it so (#1237). That ends as `Stalled` after STALL_SECONDS, so a broken DAG
# fails on what it is rather than on RUN_TIMEOUT.
await() {
  local wf=$1 phase="" waited=0 idle=0 live
  while [ "$waited" -lt "$RUN_TIMEOUT" ]; do
    phase=$(kubectl -n "$NS" get workflow "$wf" -o jsonpath='{.status.phase}')
    case "$phase" in
      Succeeded | Failed | Error)
        echo "$phase"
        return
        ;;
    esac
    live=$(kubectl -n "$NS" get pods -l "workflows.argoproj.io/workflow=$wf" \
      --field-selector=status.phase!=Succeeded,status.phase!=Failed -o name | wc -l)
    if [ "$phase" = Running ] && [ "$live" -eq 0 ]; then
      idle=$((idle + 5))
      if [ "$idle" -ge "$STALL_SECONDS" ]; then
        echo "Stalled"
        return
      fi
    else
      idle=0
    fi
    sleep 5
    waited=$((waited + 5))
  done
  echo "TimedOut"
}

# Each node of a workflow that did not succeed or skip, and why, from Argo's status.
failed_nodes() {
  kubectl -n "$NS" get workflow "$1" -o json \
    | jq -r '.status.nodes // {} | .[] | select(.type == "Pod" and (.phase | IN("Succeeded", "Skipped", "Omitted") | not))
             | "      \(.displayName): \(.phase): \(.message // "")"'
}

node_phase() {
  kubectl -n "$NS" get workflow "$1" -o json \
    | jq -r --arg n "$2" '[.status.nodes // {} | .[] | select(.displayName == $n)][0].phase // "absent"'
}

refs() { rgit for-each-ref --format='%(refname) %(objectname)'; }

# ── the first run: admitted, lands, proposes ──

log "run 1: admitted on a corpus nothing has run in"
wf=$(submit)
phase=$(await "$wf")
if [ "$phase" = Succeeded ]; then
  pass "$wf succeeded"
else
  fail "$wf ended $phase"
  failed_nodes "$wf"
fi
admit=$(node_phase "$wf" admit)
if [ "$admit" = Succeeded ]; then pass "admit ran"; else fail "admit did not succeed: $admit"; fi

MAIN=$(rgit rev-parse refs/heads/main)
if [ "$MAIN" != "$SEED" ]; then
  pass "main moved: $SEED -> $MAIN"
else
  fail "main did not move from the seed"
fi

# Every operational commit carries its receipt, and every manifest step that computes landed one.
computes=0
while read -r sha subject; do
  [ -n "$sha" ] || continue
  case "$subject" in compute:*) ;; *) continue ;; esac
  computes=$((computes + 1))
  receipts=$(rgit diff-tree --no-commit-id --name-only -r "$sha" | grep -c '^\.yidam/runs/.*\.yml$' || true)
  if [ "$receipts" -ge 1 ]; then
    pass "$sha '$subject' carries a receipt"
  else
    fail "$sha '$subject' carries no receipt under .yidam/runs/"
  fi
  committer=$(rgit log -1 --format='%an / %cn' "$sha")
  [ "$committer" = "yidam run / yidam cluster" ] || fail "$sha was authored/committed as '$committer'"
done < <(rgit log --format='%H %s' "$SEED..refs/heads/main" 2>/dev/null || true)
if [ "$computes" -ge 2 ]; then
  pass "$computes compute: commits on main"
else
  fail "expected compute: commits from travel-tier and travel-tier-typed on main, found $computes"
fi
for step in travel-tier travel-tier-typed; do
  if rgit cat-file -e "refs/heads/main:.yidam/runs/$step.yml" 2>/dev/null; then
    pass "main holds $step's receipt"
  else
    fail "main holds no receipt for $step"
  fi
done

# The epistemic step proposed, and main does not hold it.
proposals=$(rgit for-each-ref --format='%(refname:short)' refs/heads/propose/)
count=$(printf '%s' "$proposals" | grep -c . || true)
if [ "$count" = 1 ]; then
  proposal=$proposals
  tip=$(rgit rev-parse "refs/heads/$proposal")
  subject=$(rgit log -1 --format=%s "$tip")
  case "$subject" in
    establish:*) pass "$proposal holds '$subject'" ;;
    *) fail "$proposal's tip is '$subject', not an establish: commit" ;;
  esac
  if rgit cat-file -e "$tip:.yidam/runs/disclosure-envelope.yml" 2>/dev/null; then
    pass "the proposal carries disclosure-envelope's receipt"
  else
    fail "the proposal carries no receipt for disclosure-envelope"
  fi
  if rgit merge-base --is-ancestor "$tip" refs/heads/main 2>/dev/null; then
    fail "main contains the proposed commit $tip"
  else
    pass "main does not contain the proposal"
  fi
  if rgit merge-base --is-ancestor "$tip~1" refs/heads/main 2>/dev/null; then
    pass "the proposal sits on a commit main holds"
  else
    fail "the proposal's parent is not on main"
  fi
else
  fail "expected one propose/* branch, found $count: $proposals"
fi
if rgit log --format=%s "$SEED..refs/heads/main" | grep -q '^establish:'; then
  fail "an establish: commit landed on main"
else
  pass "no establish: commit on main"
fi

# ── the second run: nothing is owed ──

log "run 2: submitted again, owes nothing"
before=$(refs)
wf=$(submit)
phase=$(await "$wf")
if [ "$phase" = Succeeded ]; then
  pass "$wf succeeded"
else
  fail "$wf ended $phase"
  failed_nodes "$wf"
fi
admit=$(node_phase "$wf" admit)
if [ "$admit" = Succeeded ]; then pass "admit ran"; else fail "admit did not succeed: $admit"; fi
pin=$(node_phase "$wf" pin)
case "$pin" in
  Skipped | Omitted | absent) pass "pin did not run ($pin): the run was not admitted" ;;
  *) fail "pin ran ($pin): the second submission was admitted" ;;
esac
after=$(refs)
if [ "$before" = "$after" ]; then
  pass "no ref on the remote moved"
else
  fail "the remote's refs moved:"
  diff <(echo "$before") <(echo "$after") || true
fi

# ── the write key, from the wrong pod ──
#
# The egress policy is what stops a write key that reached a step pod from pushing. Each probe
# is a pod labelled as one of the workflow's kinds, mounting a key, pushing a commit to a ref of
# its own. Its exit code says how far it got: 10 if it could not open a connection to the
# remote, 20 if it connected and the push was refused, 0 if the push landed.

probe() {
  local name=$1 egress=$2 key=$3
  kubectl -n "$NS" apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: $name
  labels:
    yidam.dev/corpus: "$CORPUS"
    yidam.dev/egress: "$egress"
spec:
  restartPolicy: Never
  automountServiceAccountToken: false
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
    runAsGroup: 1000
    fsGroup: 1000
    seccompProfile:
      type: RuntimeDefault
  volumes:
    - name: key
      secret:
        secretName: yidam-$CORPUS-$key
        defaultMode: 256
    - name: scratch
      emptyDir: {}
    - name: home
      emptyDir: {}
  containers:
    - name: probe
      image: $IMAGE
      command: ["bash", "-c"]
      args:
        - |
          timeout 15 bash -c 'exec 3<>/dev/tcp/$GIT_HOST/22' || exit 10
          git init -q /tmp/yidam/probe && cd /tmp/yidam/probe || exit 1
          git -c user.name=probe -c user.email=probe@yidam.test commit -q --allow-empty -m probe || exit 1
          git push -q "$REMOTE" HEAD:refs/e2e/$name || exit 20
      env:
        - name: HOME
          value: /home/yidam
        - name: TMPDIR
          value: /tmp/yidam
        - name: GIT_SSH_COMMAND
          value: "ssh -i /etc/yidam/git/key -o UserKnownHostsFile=/etc/yidam/git/known_hosts -o IdentitiesOnly=yes -o ConnectTimeout=15"
      securityContext:
        allowPrivilegeEscalation: false
        readOnlyRootFilesystem: true
        capabilities:
          drop: ["ALL"]
      volumeMounts:
        - name: key
          mountPath: /etc/yidam/git
          readOnly: true
        - name: scratch
          mountPath: /tmp/yidam
        - name: home
          mountPath: /home/yidam
EOF
  # Either end, since `kubectl wait` takes one: most probes are meant to fail.
  local phase deadline=$((SECONDS + 120))
  while [ "$SECONDS" -lt "$deadline" ]; do
    phase=$(kubectl -n "$NS" get pod "$name" -o jsonpath='{.status.phase}')
    case "$phase" in Succeeded | Failed) break ;; esac
    sleep 2
  done
  kubectl -n "$NS" get pod "$name" -o jsonpath='{.status.containerStatuses[0].state.terminated.exitCode}'
}

pushed() { rgit rev-parse -q --verify "refs/e2e/$1" >/dev/null 2>&1; }

log "probes: who can push with which key"
code=$(probe step-with-write-key vault git-write)
if [ "$code" = 10 ] && ! pushed step-with-write-key; then
  pass "a step pod holding the write key cannot reach the remote"
else
  fail "a step pod holding the write key exited '$code' (want 10, a connection that never opened); pushed: $(pushed step-with-write-key && echo yes || echo no)"
fi
code=$(probe land-with-write-key remote git-write)
if [ "$code" = 0 ] && pushed land-with-write-key; then
  pass "a land pod holding the write key pushes"
else
  fail "a land pod holding the write key exited '$code' (want 0, a push); pushed: $(pushed land-with-write-key && echo yes || echo no)"
fi
code=$(probe pin-with-read-key remote git-read)
if [ "$code" = 20 ] && ! pushed pin-with-read-key; then
  pass "a pin pod holding the read key reaches the remote and is refused"
else
  fail "a pin pod holding the read key exited '$code' (want 20, a refused push); pushed: $(pushed pin-with-read-key && echo yes || echo no)"
fi

# ── on push ───────────────────────────────────────────────────────────────────
#
# The runs above were submitted by hand. Here the remote submits them (#1235): its hook posts
# each pushed ref to the EventSource `--on-push webhook` generates, and the Sensor creates a run
# for a push to main. A person's push owes a run. The lander's pushes to main owe nothing, and
# each submits a run that `admit` turns away; its push to `propose/*` submits none.

log "installing Argo Events $ARGO_EVENTS_VERSION"
kubectl create namespace argo-events
kubectl apply -n argo-events --server-side \
  -f "https://github.com/argoproj/argo-events/releases/download/$ARGO_EVENTS_VERSION/install.yaml" >/dev/null
kubectl -n argo-events rollout status deployment/controller-manager --timeout=5m

# The bus the EventSource publishes to and the Sensor reads, in the corpus's namespace and so
# under `restricted`. One server: the stream's replicas follow it.
kubectl -n "$NS" apply -f - <<EOF
apiVersion: argoproj.io/v1alpha1
kind: EventBus
metadata:
  name: default
spec:
  jetstream:
    version: "$NATS_VERSION"
    replicas: 1
    streamConfig: |
      replicas: 1
    securityContext:
      runAsNonRoot: true
      runAsUser: 1000
      runAsGroup: 1000
      fsGroup: 1000
      seccompProfile:
        type: RuntimeDefault
$(for t in containerTemplate reloaderContainerTemplate metricsContainerTemplate; do
  printf '    %s:\n      securityContext:\n        allowPrivilegeEscalation: false\n        capabilities:\n          drop: ["ALL"]\n' "$t"
done)
EOF
kubectl -n "$NS" wait --for=condition=Deployed eventbus/default --timeout=5m
# What Argo Events' controller makes from an object, once it has made it.
rolled_out() {
  local kind=$1 selector=$2 deadline=$((SECONDS + 120))
  until [ -n "$(kubectl -n "$NS" get "$kind" -l "$selector" -o name)" ]; do
    [ "$SECONDS" -lt "$deadline" ] || die "no $kind labelled $selector was created"
    sleep 2
  done
  kubectl -n "$NS" rollout status "$kind" -l "$selector" --timeout=5m
}
rolled_out statefulset eventbus-name=default

on_push="$WORK/cluster/overlays/$CORPUS-on-push"
yidam cluster workflow --on-push webhook --image "$IMAGE" --remote "$REMOTE" --vault-url "$VAULT_URL" \
  >"$on_push/$CORPUS.onpush.yml"
cp "$on_push/$CORPUS.onpush.yml" "$ARTIFACTS/"
mkdir -p "$on_push/objects/secrets"
TOKEN=$(openssl rand -hex 32)
printf '%s' "$TOKEN" >"$on_push/objects/secrets/webhook.secret"
kubectl apply -k "$on_push" -n "$NS"
rolled_out deployment eventsource-name="yidam-$CORPUS"
rolled_out deployment sensor-name="yidam-$CORPUS"
# A Ready Sensor is not yet a listening one: it subscribes to the bus once it is leader, and a
# new subscription gets only what is published after it. A push before then is published to no
# one (#1235's first CI run lost it by a third of a second). Argo Events reports the
# subscription only in the Sensor's log, so this waits on that line. It is a wait, not a check.
deadline=$((SECONDS + 120))
# A count, not `grep -q`: under pipefail, kubectl killed by a grep that stopped reading fails it.
until [ "$(kubectl -n "$NS" logs -l sensor-name="yidam-$CORPUS" --tail=-1 | grep -c 'Subscribing to subject')" -gt 0 ]; do
  [ "$SECONDS" -lt "$deadline" ] || die "the Sensor never subscribed to the event bus"
  sleep 2
done

# The remote's half: a post-receive hook posting each updated ref, as a git host's webhook does.
# It logs every post and its outcome, so a red run can tell an unsent push from an unrun one.
HOOK_URL="http://yidam-$CORPUS-eventsource-svc.$NS.svc.cluster.local:12000/push"
kubectl -n git-server exec -i "$GIT_POD" -- sh -c "
  cat >/srv/git/$CORPUS.git/hooks/post-receive &&
  chmod 755 /srv/git/$CORPUS.git/hooks/post-receive &&
  : >/tmp/hook.log && chown git:git /tmp/hook.log" <<EOF
#!/bin/sh
while read -r old new ref; do
  if wget -q -O /dev/null -T 10 --header 'Authorization: Bearer $TOKEN' \\
    --header 'Content-Type: application/json' --post-data "{\"ref\":\"\$ref\"}" '$HOOK_URL'; then
    echo "sent \$ref" >>/tmp/hook.log
  else
    echo "failed \$ref" >>/tmp/hook.log
  fi
done
EOF

# A person's push to main: one corpus file changed, which every corpus-reading step reads.
log "on push: a person pushes to main"
existing=$(kubectl -n "$NS" get workflows -l "yidam.dev/corpus=$CORPUS" -o name | sort)
kubectl -n git-server exec "$GIT_POD" -- su git -c "
  set -e
  rm -rf /tmp/human && git clone -q -b main /srv/git/$CORPUS.git /tmp/human && cd /tmp/human
  printf '\n# Edited by a person, to make a run owed.\n' >>.yidam/corpus/concept.ont.yml
  git -c user.name=Person -c user.email=person@yidam.test commit -qam 'chore: a person edits the corpus'
  git push -q origin HEAD:main"
HUMAN=$(rgit rev-parse refs/heads/main)

# The runs that push brought, and those the lander's pushes brought after it.
pushed_runs() {
  comm -13 <(echo "$existing") <(kubectl -n "$NS" get workflows -l "yidam.dev/corpus=$CORPUS" -o name | sort) \
    | sed 's|^workflow[^/]*/||' | grep . || true
}
deadline=$((SECONDS + 120))
while [ -z "$(pushed_runs)" ] && [ "$SECONDS" -lt "$deadline" ]; do sleep 2; done
first=$(pushed_runs | head -1)
if [ -z "$first" ]; then
  fail "no run was created for the push to main"
else
  # Every run on push, in the order the mutex lets them through, until none is left running.
  # Then a wait as long as the controller takes to start one, in case a push's run is late.
  for settle in 1 2; do
    for wf in $(pushed_runs); do await "$wf" >/dev/null; done
    [ "$settle" = 2 ] || sleep 60
  done
  lands=$(rgit rev-list --count "$HUMAN..refs/heads/main")
  admitted=0
  turned_away=0
  for wf in $(pushed_runs); do
    phase=$(kubectl -n "$NS" get workflow "$wf" -o jsonpath='{.status.phase}')
    [ "$phase" = Succeeded ] || {
      fail "$wf ended $phase"
      failed_nodes "$wf"
    }
    case "$(node_phase "$wf" pin)" in
      Succeeded) admitted=$((admitted + 1)) ;;
      *) turned_away=$((turned_away + 1)) ;;
    esac
  done
  if [ "$admitted" = 1 ]; then
    pass "the person's push brought one admitted run"
  else
    fail "the push to main brought $admitted admitted runs, not 1"
  fi
  if [ "$lands" -ge 1 ]; then
    pass "the admitted run landed $lands commit(s) on main"
  else
    fail "the admitted run landed nothing on main, so no push of the lander's was tested"
  fi
  if [ "$turned_away" = "$lands" ]; then
    pass "each of the lander's $lands push(es) to main brought one run, and admit turned it away"
  else
    fail "the lander pushed to main $lands time(s) and $turned_away run(s) were turned away"
  fi
  # The hook posted the proposal as well, and the Sensor's filter, not a missing post, is why
  # no run came of it.
  if kubectl -n git-server exec "$GIT_POD" -- grep -q '^sent refs/heads/propose/' /tmp/hook.log; then
    pass "the lander's push to propose/* was posted and brought no run"
  else
    fail "the hook posted no push to propose/*"
  fi
fi
kubectl -n git-server exec "$GIT_POD" -- cat /tmp/hook.log >"$ARTIFACTS/hook.log" || true

# ── verdict ───────────────────────────────────────────────────────────────────

if [ "$failures" -gt 0 ]; then
  log "$failures assertion(s) failed"
  exit 1
fi
log "cluster-e2e passed"
