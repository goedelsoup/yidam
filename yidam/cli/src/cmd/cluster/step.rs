//! `cluster step` — one capability, against one pinned bundle, producing one unlanded commit.
//!
//! This is `cmd/run`'s loop body with the ref write taken out of it. Everything up to the
//! commit object is the same code — `materialize`, `invoke`, `check_declared`, the receipt,
//! `build_commit` — because RFC-0026 §5 names the four properties a run inherits from the
//! proposal machinery and two implementations of them would be two answers to whether a
//! run is safe. What differs is where the commit goes: nowhere. It is written to the
//! object store of a clone that lives in a scratch directory, pointed at by a ref that is
//! not a branch, bundled, and put in the vault. The pod exits, the scratch is gone, and the
//! commit exists in exactly one place that outlives it: a content-addressed file whose name
//! is its digest.
//!
//! # The step does not know about proposal branches
//!
//! `cmd/run` decides, before an epistemic step is invoked, whether `propose/<head>` already
//! exists and builds on its tip if so. This does not, and cannot: it has no remote to ask.
//! Every commit a step builds has the pinned input as its parent, and the lander — which
//! does have the remote — re-parents it onto the proposal branch's tip if there is one. That
//! is the same split the module doc describes: the step computes, the lander decides where.

use std::path::Path;

use anyhow::{bail, Result};

use super::{
    bundle, class_of, deliver, StepOutcome, StepOutput, VaultArgs, CONTRACT_VERSION, OUT_REF,
};
use crate::cmd::propose::write::{git, short_of};
use crate::cmd::run::exec::{self, Scratch};
use crate::cmd::run::manifest::Manifest;
use crate::cmd::run::receipt::{self, sha256, File, Input, Receipt};
use crate::cmd::run::{build_commit, digest_of, plan_and_write, Freshness};
use crate::report::Format;
use crate::vault::Store;

pub(super) fn run(
    name: &str,
    digest: &str,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let store = vault.open()?;
    let scratch = Scratch::new("step")?;
    let file = scratch.path().join("in.bundle");
    bundle::fetch(store.as_ref(), digest, &file)?;
    let (input, branch) = bundle::pinned_branch(scratch.path(), &file)?;
    let root = scratch.path().join("corpus");
    bundle::clone(&file, &branch, &root)?;
    super::commits_as_the_pod(&root)?;
    let record = step_in(&root, name, &input, store.as_ref(), scratch.path())?;
    deliver(&root, format, out, record, render)
}

/// Invoke `name` in the clone at `root`, whose branch tip is `input`, and bundle what it built.
pub(super) fn step_in(
    root: &Path,
    name: &str,
    input: &str,
    store: &dyn Store,
    scratch: &Path,
) -> Result<StepOutput> {
    let m = Manifest::load(root)?;
    let cap = m.get(name)?;
    if let Some(reason) = cap.kind.unrunnable_because() {
        bail!(
            "`{name}` declares `kind = \"{}\"` and this binary invokes calculators only.\n  {reason}.",
            cap.kind.as_str()
        );
    }
    let route = cap.route();
    let receipt_path = Receipt::path(name);
    let short = short_of(root, input);

    // The plan this step is the end of, resolved as a dry run: every step it waits for must
    // be up to date at this input, and this step itself may already be.
    //
    // On one machine `run` invokes the upstream first and this step reads what it landed.
    // On a cluster the upstream ran in another pod and its output reached this one only if
    // the lander landed it and a new pin was taken — which is the order the generated
    // workflow imposes. This is the check that the order held: a step invoked against a pin
    // its upstream had not landed on would compute from stale inputs and record a receipt
    // saying so, and refusing is cheaper than landing that.
    let dry = plan_and_write(root, Some(name), true)?;
    for s in &dry.steps {
        if s.step == name {
            if s.freshness == Freshness::Fresh {
                return Ok(StepOutput {
                    format_version: CONTRACT_VERSION,
                    step: name.to_string(),
                    outcome: StepOutcome::Fresh,
                    sha: None,
                    class: class_of(route).to_string(),
                    verb: cap.verb.clone(),
                    receipt: receipt_path,
                    input: input.to_string(),
                    bundle: None,
                });
            }
            continue;
        }
        if s.freshness != Freshness::Fresh {
            bail!(
                "`{}` is not up to date at {short}: {}.\n  \
                 `{name}` waits for it, so this pin is not one `{name}` can run against. The \
                 workflow lands `{}` and pins again before this step; a bundle handed to it \
                 out of that order is refused here rather than computed from.",
                s.step,
                s.because,
                s.step
            );
        }
    }

    let manifest_sha256 = digest_of(root, crate::cmd::run::manifest::MANIFEST);
    let config_sha256 = digest_of(root, ".yidam/config.toml");
    let inputs = exec::materialize(root, input, cap)?;
    let resolved_digest = inputs.resolved.as_ref().map(|r| r.sha256.clone());
    let script_digest = Receipt::script_sha256(cap, &inputs.files);
    let input_state = Receipt::input_state(
        cap,
        &manifest_sha256,
        &config_sha256,
        &inputs.files,
        resolved_digest.as_deref(),
        script_digest.as_deref(),
    )?;

    let produced = exec::invoke(cap, &inputs, name, input)?;
    exec::check_declared(cap, &produced.outputs)?;
    if !produced.stderr.is_empty() {
        // Passed through, not recorded: a pod's log is not provenance, and the receipt is.
        eprint!("{}", produced.stderr);
    }

    let receipt = Receipt {
        format_version: receipt::FORMAT_VERSION,
        step: name.to_string(),
        kind: cap.kind.as_str(),
        verb: cap.verb.clone(),
        run: cap.run.clone(),
        input_state,
        input: Input {
            commit: input.to_string(),
            manifest_sha256,
            config_sha256,
            reads: cap.reads.clone(),
            files: inputs.files.clone(),
            resolved_graph_sha256: resolved_digest,
            script_sha256: script_digest,
        },
        writes: cap.writes.clone(),
        outputs: produced
            .outputs
            .iter()
            .map(|(path, bytes)| File {
                path: path.clone(),
                sha256: sha256(bytes),
            })
            .collect(),
    };
    let mut landing: Vec<(String, Vec<u8>)> = produced.outputs.clone();
    landing.push((receipt_path.clone(), receipt.to_yaml()?.into_bytes()));

    let Some(built) = build_commit(root, input, cap, name, &landing, &short, route)? else {
        return Ok(StepOutput {
            format_version: CONTRACT_VERSION,
            step: name.to_string(),
            outcome: StepOutcome::Unchanged,
            sha: None,
            class: class_of(route).to_string(),
            verb: cap.verb.clone(),
            receipt: receipt_path,
            input: input.to_string(),
            bundle: None,
        });
    };

    // A ref that is not a branch, so the bundle has a name to carry the commit under. The
    // prerequisite is the input: the lander already holds it, and a bundle without one would
    // carry the corpus's whole history back through the vault on every step.
    git(root, None, &["update-ref", OUT_REF, &built.sha], None)?;
    let file = scratch.join("out.bundle");
    bundle::create(root, &file, &[&format!("{input}..{OUT_REF}")])?;
    let digest = bundle::put(store, &file)?;

    Ok(StepOutput {
        format_version: CONTRACT_VERSION,
        step: name.to_string(),
        outcome: StepOutcome::Ran,
        sha: Some(built.sha),
        class: class_of(route).to_string(),
        verb: cap.verb.clone(),
        receipt: receipt_path,
        input: input.to_string(),
        bundle: Some(digest),
    })
}

pub(super) fn render(s: &StepOutput) -> String {
    let input = &s.input[..s.input.len().min(12)];
    match s.outcome {
        StepOutcome::Fresh => format!(
            "{}: fresh at {input} — the committed receipt matches this input state; nothing built\n",
            s.step
        ),
        StepOutcome::Unchanged => format!(
            "{}: ran at {input} and produced the tree already there; nothing built\n",
            s.step
        ),
        StepOutcome::Ran => format!(
            "{}: built {} ({} `{}:`) on {input}, unlanded\n  receipt {}\n  bundle {}\n",
            s.step,
            s.sha.as_deref().map_or("?", |x| &x[..x.len().min(12)]),
            s.class,
            s.verb,
            s.receipt,
            s.bundle.as_deref().unwrap_or("?")
        ),
    }
}
