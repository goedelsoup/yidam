//! `yidam index-push` — make a vector bucket's index equal to `.yidam/index/`.
//!
//! # A mirror, not an append
//!
//! The push writes every local row and then deletes the keys under this corpus's prefix that
//! the local index no longer has. That second half is the whole difference between this and an
//! uploader, and it is the failure #807/#808 recorded in another part of this repository: a
//! writer that only adds leaves entries nothing can see, and a node deleted from the corpus
//! goes on being findable indefinitely.
//!
//! What it never deletes is anything that is not this corpus's — the witness record, and
//! another corpus's vectors once an index holds more than one. [`crate::s3vectors::ops::plan_mirror`]
//! owns that rule and asserts it.
//!
//! # What it needs, and what it does not
//!
//! `vector-read`, because it decodes `index/corpus.arrow`. **Not `index`**: building an index
//! wants lancedb and protoc 31, and reading one it has already built wants neither. The
//! machine that can push is therefore not necessarily the machine that built — which is the
//! same split `vector-read` exists for on the query side.
//!
//! It does not embed anything, so it never loads a model. See
//! [`crate::s3vectors::request::witness`] for what that costs and why it is the right trade.

use anyhow::{bail, Context, Result};

use crate::config::load_yidam_config;
use crate::model::{index_rows, load_domain_model};
use crate::paths::repo_root;
use crate::s3vectors::{
    ops::{self, MirrorPlan, Session},
    request::{self, OutVector},
    transport::Client,
    RemoteIndex,
};
use crate::vault::creds;

/// The genesis hash, shortened to the length a key carries.
///
/// Twelve characters of a SHA-1, which is what the rest of this repository uses when it needs
/// a short commit and is comfortably past the point where two corpora collide.
const CORPUS_ID_LEN: usize = 12;

pub fn index_push(dry_run: bool, create: bool) -> Result<()> {
    let root = repo_root()?;

    let cfg = load_yidam_config(&root)?;
    let declared = cfg.index.remote.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "this corpus declares no remote index.\n  \
             Add one to .yidam/config.toml:\n\n  \
             [index.remote]\n  \
             kind   = \"s3-vectors\"\n  \
             bucket = \"<vector bucket>\"\n  \
             index  = \"<vector index>\"\n  \
             region = \"<aws region>\""
        )
    })?;
    let remote = RemoteIndex::resolve(declared)?;

    let model = load_domain_model(&root)?;
    let index = model.index.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "this corpus has no index to push.\n  \
             Run `yidam embed && yidam index-build` first."
        )
    })?;

    // The contract travels with the vectors or the push is sending rows nothing can verify.
    let contract = index.embed_config.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "this index carries no {} — it was built before the embedding contract existed.\n  \
             Rebuild it with `yidam index-build`: pushing it would put rows in a vector bucket \
             that no reader can check its own embedder against.",
            crate::embed_config::EMBED_CONFIG_FILENAME
        )
    })?;

    // **A push cannot proceed without the genesis hash**, and refusing is not pedantry.
    // Every key this push writes is prefixed with it, so a repository that cannot see its own
    // root commit would write one set of keys and a full clone of the same corpus would write
    // another — and the mirror's delete step, which only touches its own prefix, would leave
    // the first set behind forever with nothing able to name it. A shallow clone is exactly
    // the case: `git log` in one does not reach the root, and the hash is `None`.
    let corpus = model
        .provenance
        .genesis_hash
        .as_deref()
        .map(corpus_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "this repository cannot see its own genesis commit, so a push has no corpus \
                 identity to key its vectors under.\n  \
                 A shallow clone is the usual cause — `git fetch --unshallow` and try again."
            )
        })?;
    let commit = model.provenance.commit.clone();
    let rows = index_rows(index)?;
    if rows.is_empty() {
        bail!("the index holds no rows — there is nothing to push");
    }

    let (vectors, truncated) = to_vectors(&rows, &corpus, &commit)?;
    let witness = request::witness(contract)?;

    let creds = creds::resolve_scope(&creds::index_scope(), |k| std::env::var(k).ok())?;
    let client = Client::new(remote.clone(), creds)?;
    let session = Session::new(&client);

    if create {
        ensure_index(&session, &remote, contract.embedding_dim)?;
    }

    let keys: Vec<String> = vectors.iter().map(|v| v.key.clone()).collect();
    let plan = ops::plan_mirror(&session, &remote, &corpus, &keys).map_err(|e| {
        // The first request a push makes is a listing, so "there is no such index" arrives as
        // a 404 on `ListVectors` — a message about the wrong operation for a state that has an
        // obvious remedy.
        if e.fault == crate::s3vectors::response::Fault::Missing && !create {
            anyhow::anyhow!(
                "the vector index {} does not exist in bucket {}.\n  \
                 Run `yidam index-push --create` to create it. Its dimension, distance metric \
                 and non-filterable metadata keys are fixed at creation and cannot be changed \
                 afterwards, which is why creating one is not the default.",
                remote.index,
                remote.bucket
            )
        } else {
            e.into()
        }
    })?;

    println!("{}", remote.describe());
    println!("  corpus:  {corpus} at {commit}");
    report_plan(&plan, truncated);

    if dry_run {
        // The canonical request, because it is the only artifact of a signing bug a person can
        // inspect — `vault push --dry-run` prints the same thing for the same reason.
        let sample = request::put_vectors(&remote, &vectors[..1])?;
        println!("\n--- canonical request (first PutVectors) ---");
        println!("{}", client.sign(&sample)?.canonical_request);
        println!("\nNothing was written. Drop --dry-run to apply.");
        return Ok(());
    }

    ops::apply_mirror(&session, &remote, &vectors, Some(&witness), &plan)?;
    println!(
        "\nPushed {} row(s); deleted {}.",
        plan.put,
        plan.delete.len()
    );
    Ok(())
}

/// Every index row as a vector on its way to the bucket, and how many had their text cut.
///
/// Split out so the assembly is testable without a transport. It is the part of this command
/// that decides *what would be written* — the key each node lands under and the metadata that
/// travels with it — and everything after it is I/O.
fn to_vectors(
    rows: &[crate::model::VectorRow],
    corpus: &str,
    commit: &str,
) -> Result<(Vec<OutVector>, usize)> {
    let mut vectors = Vec::with_capacity(rows.len());
    let mut truncated = 0usize;
    for row in rows {
        let key = request::vector_key(corpus, &row.path)?;
        let (metadata, cut) = request::metadata(corpus, &row.class, &row.label, commit, &row.text)
            .with_context(|| format!("building the metadata for {}", row.path))?;
        if cut {
            truncated += 1;
        }
        vectors.push(OutVector {
            key,
            data: row.vector.clone(),
            metadata,
        });
    }
    Ok((vectors, truncated))
}

/// The corpus identity a key carries.
fn corpus_id(genesis_hash: &str) -> String {
    genesis_hash.chars().take(CORPUS_ID_LEN).collect()
}

/// Create the index if it is not there, and refuse to write into one that disagrees.
///
/// The disagreement that matters is dimension: a `PutVectors` into an index of another
/// dimension fails per request, and a *query* against one silently cannot happen at all. The
/// metric matters for the same reason `response::score_from_distance` refuses a euclidean one.
fn ensure_index(session: &Session, remote: &RemoteIndex, dimension: i32) -> Result<()> {
    let dimension = u32::try_from(dimension)
        .map_err(|_| anyhow::anyhow!("the contract records a negative embedding_dim"))?;

    match session.call(&request::get_index(remote)) {
        Ok(body) => disagreement(&body, dimension, &remote.index).map_or_else(
            || {
                println!("  index exists; not recreating it.");
                Ok(())
            },
            |why| bail!(why),
        ),
        Err(e) if e.fault == crate::s3vectors::response::Fault::Missing => {
            session.call(&request::create_index(remote, dimension)?)?;
            println!("  created index {} ({dimension} dimensions).", remote.index);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Why an existing index cannot hold this corpus, or `None` when it can.
///
/// Pure, and separate from the call that produces the body, so the field names are checked
/// against `GetIndex`'s documented response shape by a test rather than by a reader. A pointer
/// into the wrong path would make this check silently unfireable — it would find no dimension,
/// conclude nothing, and let a push proceed into an index it does not fit.
fn disagreement(body: &serde_json::Value, dimension: u32, name: &str) -> Option<String> {
    let got = body.pointer("/index/dimension").and_then(|v| v.as_u64());
    if let Some(d) = got {
        if d != u64::from(dimension) {
            return Some(format!(
                "the index {name} already exists with dimension {d}, and this corpus embeds \
                 into {dimension}.\n  \
                 A vector index's dimension is fixed when it is created — push to a \
                 different index name, or delete that one."
            ));
        }
    }
    let metric = body
        .pointer("/index/distanceMetric")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::s3vectors::DISTANCE_METRIC);
    if metric != crate::s3vectors::DISTANCE_METRIC {
        return Some(format!(
            "the index {name} uses the {metric} distance metric and this build reads only {}.\n  \
             The metric is fixed when an index is created — push to a different index name.",
            crate::s3vectors::DISTANCE_METRIC
        ));
    }
    None
}

fn report_plan(plan: &MirrorPlan, truncated: usize) {
    println!("  write:   {} row(s) ({} new)", plan.put, plan.new);
    println!(
        "  delete:  {} row(s) no longer in the corpus",
        plan.delete.len()
    );
    if plan.untouched > 0 {
        println!(
            "  leave:   {} record(s) that are not this corpus's",
            plan.untouched
        );
    }
    if truncated > 0 {
        println!(
            "  note:    {truncated} row(s) had their text cut to fit the 40 KB metadata ceiling \
             (marked text_truncated)"
        );
    }
    for key in plan.delete.iter().take(10) {
        println!("    - {key}");
    }
    if plan.delete.len() > 10 {
        println!("    … and {} more", plan.delete.len() - 10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::VectorRow;
    use crate::s3vectors::{MAX_METADATA_BYTES, META_KEY_CLASS, META_KEY_COMMIT, META_KEY_CORPUS};

    fn row(path: &str, text: &str) -> VectorRow {
        VectorRow {
            path: path.to_string(),
            class: "concept".to_string(),
            label: "A label".to_string(),
            text: text.to_string(),
            vector: vec![0.5, -0.5],
        }
    }

    /// Every row becomes one vector, keyed under the corpus, carrying the commit.
    ///
    /// Two rows and not one: a loop that wrote the first row's key for every vector would pass
    /// a single-row fixture, and the key is what the mirror's delete step diffs against.
    #[test]
    fn each_row_becomes_a_vector_keyed_under_the_corpus() {
        let rows = [
            row("corpus/concept/a.yml", "text a"),
            row("corpus/concept/b.yml", "text b"),
        ];
        let (vectors, truncated) = to_vectors(&rows, "abc123", "deadbee").unwrap();
        assert_eq!(truncated, 0);
        assert_eq!(
            vectors.iter().map(|v| v.key.as_str()).collect::<Vec<_>>(),
            vec!["abc123/corpus/concept/a.yml", "abc123/corpus/concept/b.yml"]
        );
        assert_eq!(vectors[0].data, vec![0.5, -0.5]);
        assert_eq!(vectors[0].metadata[META_KEY_CORPUS], "abc123");
        assert_eq!(vectors[0].metadata[META_KEY_CLASS], "concept");
        assert_eq!(vectors[0].metadata[META_KEY_COMMIT], "deadbee");
        assert_eq!(vectors[1].metadata["text"], "text b");
    }

    /// A row too large for the metadata ceiling is cut and counted, not dropped and not fatal.
    #[test]
    fn an_oversize_row_is_counted_as_truncated() {
        let rows = [
            row("a.yml", "short"),
            row("b.yml", &"x".repeat(MAX_METADATA_BYTES * 2)),
        ];
        let (vectors, truncated) = to_vectors(&rows, "abc123", "deadbee").unwrap();
        assert_eq!(truncated, 1, "the cut row was not counted");
        assert_eq!(vectors.len(), 2, "a row was dropped rather than cut");
        assert_eq!(
            vectors[1].metadata["text_truncated"],
            serde_json::json!(true)
        );
    }

    /// A path that cannot be keyed stops the push rather than being silently skipped: a node
    /// missing from the remote index is indistinguishable from one that was never there.
    #[test]
    fn a_path_that_cannot_be_keyed_fails_the_push() {
        let rows = [row(&"a/".repeat(600), "text")];
        assert!(to_vectors(&rows, "abc123", "deadbee").is_err());
    }

    /// The response shape is `GetIndex`'s, and the pointers must reach into it.
    ///
    /// A check that reads the wrong path finds no dimension and concludes nothing, which looks
    /// exactly like agreement. The body here is the documented response shape, so a pointer
    /// typo fails rather than passing quietly.
    #[test]
    fn an_index_of_another_dimension_or_metric_is_refused() {
        let body = |dim: u64, metric: &str| {
            serde_json::json!({"index": {
                "indexName": "yidam-main",
                "dataType": "float32",
                "dimension": dim,
                "distanceMetric": metric,
                "metadataConfiguration": {"nonFilterableMetadataKeys": ["text"]},
            }})
        };
        assert_eq!(disagreement(&body(384, "cosine"), 384, "yidam-main"), None);

        let d = disagreement(&body(768, "cosine"), 384, "yidam-main").expect("a dimension clash");
        assert!(d.contains("768") && d.contains("384"), "{d}");

        let m = disagreement(&body(384, "euclidean"), 384, "yidam-main").expect("a metric clash");
        assert!(m.contains("euclidean") && m.contains("cosine"), "{m}");
    }

    /// A body this code cannot read concludes nothing — and that is the failure mode the test
    /// above exists to prevent, so it is named here rather than left implicit.
    #[test]
    fn a_body_with_no_index_object_concludes_nothing() {
        assert_eq!(disagreement(&serde_json::json!({}), 384, "x"), None);
    }

    #[test]
    fn a_corpus_id_is_the_short_genesis_hash() {
        assert_eq!(corpus_id("0123456789abcdef0123"), "0123456789ab");
        // A hash shorter than the window is used whole rather than padded — a test fixture
        // repository has one, and panicking on it would make the command untestable.
        assert_eq!(corpus_id("abc"), "abc");
        assert_eq!(corpus_id(""), "");
    }
}
