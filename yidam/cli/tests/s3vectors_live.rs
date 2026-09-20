//! The S3 Vectors transport against the real service.
//!
//! # Why this is `#[ignore]`d
//!
//! CI is hermetic and must stay that way. **There is no emulator for S3 Vectors** — unlike S3,
//! where a MinIO in a container is enough — so these tests need an AWS account, a vector
//! bucket, and credentials. They do not run by default and they do not run on a pull request.
//!
//! They are committed because they are the **authority** for four claims the rest of the
//! suite can only approximate, and RFC-0033 §8 names each of them. All four were settled
//! against a live index on 2026-09-20 — §8.2 records the run, and records that each was
//! settled by breaking the code and watching a test here fail, not by watching one pass:
//!
//! 1. **A request signed for `s3vectors` is accepted.** The unit tests pin that the credential
//!    scope differs from `s3` and that the payload digest covers the bytes sent. Only a server
//!    that recomputes the signature independently settles whether the whole is right.
//! 2. **`score = 1 - distance` for a cosine index.** This is the one number the design assumes
//!    rather than derives. The unit tests pin that the conversion is monotone — a nearer vector
//!    always scores higher — which is the property ranking needs and which holds under any
//!    affine reading of the metric. `the_score_is_the_cosine_similarity_to_five_decimals` is
//!    what pins the values, against vectors whose cosine is known exactly by hand.
//! 3. **The mirror's delete reaches the service.** `ops` tests assert the request is built and
//!    ordered correctly against a recorded transport. Whether a deleted key stops coming back
//!    is a question only the index can answer.
//! 4. **`GetIndex`'s response shape.** `response::disagreement` reads `/index/dimension` and
//!    `/index/distanceMetric`. A unit test checks those pointers against the shape as
//!    *documented*; only the service settles the shape as *served*, and a pointer into the
//!    wrong path does not fail loudly — it finds no dimension, concludes nothing, and lets a
//!    push proceed into an index it does not fit.
//!
//! # Running them
//!
//! ```sh
//! aws s3vectors create-vector-bucket --vector-bucket-name my-yidam-test
//!
//! YIDAM_S3VECTORS_TEST=1 \
//! YIDAM_S3VECTORS_BUCKET=my-yidam-test \
//! YIDAM_INDEX_ACCESS_KEY_ID=… YIDAM_INDEX_SECRET_ACCESS_KEY=… \
//!   cargo test --test s3vectors_live -- --ignored --test-threads=1
//! ```
//!
//! `--test-threads=1` is not optional: every test here writes to one index.
//!
//! Without `YIDAM_S3VECTORS_TEST` they skip rather than failing, so `-- --ignored` on a machine
//! with no account reports a skip instead of a red suite.
//!
//! **That skip is quiet, and it reads as success.** cargo captures stdout for a passing test, so
//! an unarmed run prints `6 passed` and the notice is visible only under `--nocapture`. The tell
//! is the duration: an unarmed run finishes in 0.00s and a real one takes about ten seconds.
//! Do not take a green run here as evidence that anything was checked without looking at that.

#![cfg(feature = "s3-vectors")]

use serde_json::json;

use yidam::retrieval::Filter;
use yidam::s3vectors::{
    ops::{self, Session},
    request::{self, OutVector},
    response::{self, Fault},
    transport::Client,
    RemoteIndex, RemoteIndexConfig, KIND, WITNESS_KEY,
};

/// One index, reused. Creating one per run would leave them behind — there is no `DeleteIndex`
/// on this crate's surface, and 10,000 per bucket is a ceiling a test suite can reach.
const INDEX: &str = "yidam-live-test";
/// Four dimensions, so every vector in this file has a cosine that can be checked by hand.
const DIMENSION: u32 = 4;
/// The corpus these tests write under. Not a real genesis hash; the point is that it is *a*
/// prefix and that nothing outside it is touched.
const CORPUS: &str = "livetest0001";

fn enabled() -> bool {
    if std::env::var("YIDAM_S3VECTORS_TEST").is_err() {
        ci_report::skipped(
            "set YIDAM_S3VECTORS_TEST=1, YIDAM_S3VECTORS_BUCKET=<a vector bucket> and \
             credentials — there is no S3 Vectors emulator, so this needs a real account",
        );
        return false;
    }
    true
}

fn client() -> (Client, RemoteIndex) {
    let bucket = std::env::var("YIDAM_S3VECTORS_BUCKET")
        .expect("YIDAM_S3VECTORS_BUCKET names the vector bucket to test against");
    let region = std::env::var("YIDAM_S3VECTORS_REGION").unwrap_or_else(|_| "us-east-1".into());
    let index = RemoteIndex::resolve(&RemoteIndexConfig {
        kind: KIND.to_string(),
        bucket,
        index: INDEX.to_string(),
        region,
        endpoint: None,
    })
    .expect("the test configuration resolves");
    let creds = yidam::vault::sigv4::Credentials {
        access_key_id: env_any(&["YIDAM_INDEX_ACCESS_KEY_ID", "AWS_ACCESS_KEY_ID"]),
        secret_access_key: env_any(&["YIDAM_INDEX_SECRET_ACCESS_KEY", "AWS_SECRET_ACCESS_KEY"]),
        session_token: ["YIDAM_INDEX_SESSION_TOKEN", "AWS_SESSION_TOKEN"]
            .iter()
            .find_map(|k| std::env::var(k).ok()),
    };
    let client = Client::new(index.clone(), creds).expect("building the client");
    (client, index)
}

fn env_any(keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| std::env::var(k).ok())
        .unwrap_or_else(|| panic!("one of {keys:?} must be set"))
}

/// Create the index if it is not there. Idempotent, because these tests share one.
fn ensure_index(session: &Session, index: &RemoteIndex) {
    match session.call(&request::get_index(index)) {
        Ok(_) => {}
        Err(e) if e.fault == Fault::Missing => {
            session
                .call(&request::create_index(index, DIMENSION).unwrap())
                .expect("creating the test index");
        }
        // A 403 here is the signing failure, arriving before the test that is *about* signing
        // gets to say so — `ensure_index` is the first call every test in this file makes. It
        // is named rather than folded into the general case, because "GetIndex failed" sends a
        // reader to look at GetIndex.
        Err(e) if e.fault == Fault::Denied => panic!(
            "the service rejected the signature or the permissions on GetIndex — every test \
             in this file will fail the same way until that is fixed: {e}"
        ),
        Err(e) => panic!("GetIndex failed for a reason that is not absence: {e}"),
    }
}

fn unit(v: [f32; 4]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter().map(|x| x / norm).collect()
}

fn row(path: &str, data: Vec<f32>) -> OutVector {
    let (metadata, _) = request::metadata(
        CORPUS,
        "concept",
        "A label",
        "deadbeef",
        "the embedded text",
    )
    .unwrap();
    OutVector {
        key: request::vector_key(CORPUS, path).unwrap(),
        data,
        metadata,
    }
}

/// Everything under this test's prefix, gone — so each test starts from a known index.
fn clear(session: &Session, index: &RemoteIndex) {
    let keys: Vec<String> = ops::list_keys(session, index)
        .expect("listing")
        .into_iter()
        .filter(|k| k.starts_with(&format!("{CORPUS}/")))
        .collect();
    for batch in request::delete_batches(index, &keys).unwrap() {
        session.call(&batch).expect("deleting");
    }
}

/// The signature is accepted, and the service answers each of the six operations.
///
/// The least specific test here and the one that fails first when signing is wrong: a bad
/// signature is a 403 on every operation, whatever else is true.
#[test]
#[ignore]
fn the_service_accepts_a_request_signed_for_s3vectors() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);

    // A 403 here is the signing failure this test exists to catch. Anything else — a missing
    // bucket, a throttle — is a different problem and says so.
    match session.call(&request::list_vectors(&index, None)) {
        Ok(_) => {}
        Err(e) if e.fault == Fault::Denied => {
            panic!("the service rejected the signature or the permissions: {e}")
        }
        Err(e) => panic!("ListVectors failed: {e}"),
    }
}

/// `disagreement`'s pointers reach into the body the service actually sends.
///
/// The unit test beside the function pins it against `GetIndex`'s *documented* response shape.
/// This runs the same function against a *served* one, which is the only thing that can catch
/// the shape having been documented wrong or changed under us.
///
/// The load-bearing assertion is the second one. A check that reads the wrong path returns
/// `None` — agreement — for every input, so asserting that a matching index agrees would pass
/// just as happily with both pointers broken. Asking it about a dimension the index does *not*
/// have is what requires the pointer to have found something.
#[test]
#[ignore]
fn the_served_get_index_body_has_the_shape_the_push_check_reads() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);

    let body = session
        .call(&request::get_index(&index))
        .expect("GetIndex against the live index");

    assert_eq!(
        response::disagreement(&body, DIMENSION, INDEX),
        None,
        "the live index is {DIMENSION}-dimensional and cosine, and the check disagreed: {body}"
    );

    let wrong = response::disagreement(&body, DIMENSION + 1, INDEX)
        .expect("a dimension clash the service's own body should expose");
    assert!(
        wrong.contains(&DIMENSION.to_string()),
        "the clash did not name the dimension the index reports: {wrong}"
    );

    // The metric arm defaults to agreement when its pointer finds nothing, so a broken pointer
    // there is invisible to `disagreement` alone. Read it directly.
    assert_eq!(
        body.pointer("/index/distanceMetric")
            .and_then(|v| v.as_str()),
        Some("cosine"),
        "/index/distanceMetric is not where the push check looks for it: {body}"
    );
    assert_eq!(
        body.pointer("/index/dimension").and_then(|v| v.as_u64()),
        Some(u64::from(DIMENSION)),
        "/index/dimension is not where the push check looks for it: {body}"
    );
}

/// **The acceptance criterion of RFC-0033.**
///
/// Three vectors whose cosine against the query is known exactly, and the scores that come back
/// are those cosines. If `1 - distance` is not the cosine similarity, this is where it shows —
/// and the design's claim that a remote score and a local score are the same quantity fails
/// with it.
#[test]
#[ignore]
fn the_score_is_the_cosine_similarity_to_five_decimals() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);
    clear(&session, &index);

    let query = unit([1.0, 0.0, 0.0, 0.0]);
    // Cosine against the query, by construction: 1, 1/√2 ≈ 0.7071068, 0.
    let rows = vec![
        row("same.yml", unit([1.0, 0.0, 0.0, 0.0])),
        row("half.yml", unit([1.0, 1.0, 0.0, 0.0])),
        row("orthogonal.yml", unit([0.0, 1.0, 0.0, 0.0])),
    ];
    session
        .call(&request::put_vectors(&index, &rows).unwrap())
        .expect("putting the vectors");

    let hits = ops::query(
        &session,
        &index,
        CORPUS,
        &query,
        &Filter::any(),
        |_| true,
        3,
    )
    .expect("querying");

    let got: Vec<(&str, f32)> = hits.iter().map(|h| (h.path.as_str(), h.score)).collect();
    assert_eq!(
        got.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec!["same.yml", "half.yml", "orthogonal.yml"],
        "the ranking is not by decreasing similarity: {got:?}"
    );

    for (path, expected) in [
        ("same.yml", 1.0_f32),
        ("half.yml", std::f32::consts::FRAC_1_SQRT_2),
        ("orthogonal.yml", 0.0),
    ] {
        let (_, score) = got.iter().find(|(p, _)| *p == path).unwrap();
        assert!(
            (score - expected).abs() < 1e-5,
            "{path} scored {score}, and its cosine against the query is {expected}. \
             `score = 1 - distance` is the one assumption RFC-0033 could not derive."
        );
    }

    // And the metadata came back, which is what `retrieve` renders.
    let first = &hits[0];
    assert_eq!(first.class, "concept");
    assert_eq!(first.label, "A label");
    assert_eq!(first.text, "the embedded text");
}

/// A deleted key stops coming back.
///
/// The mirror's second half, against the index rather than against a recorded transport. Writes
/// are strongly consistent, so there is no wait here and a failure is a real one.
#[test]
#[ignore]
fn a_deleted_key_stops_being_findable() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);
    clear(&session, &index);

    let keep = row("keep.yml", unit([1.0, 0.0, 0.0, 0.0]));
    let gone = row("gone.yml", unit([1.0, 0.1, 0.0, 0.0]));
    session
        .call(&request::put_vectors(&index, &[keep.clone(), gone.clone()]).unwrap())
        .expect("putting");

    let plan = ops::plan_mirror(&session, &index, CORPUS, &[keep.key.clone()])
        .expect("planning the mirror");
    assert_eq!(plan.delete, vec![gone.key.clone()]);

    ops::apply_mirror(&session, &index, &[keep.clone()], None, &plan).expect("applying");

    let hits = ops::query(
        &session,
        &index,
        CORPUS,
        &unit([1.0, 0.0, 0.0, 0.0]),
        &Filter::any(),
        |_| true,
        10,
    )
    .expect("querying");
    assert!(
        hits.iter().all(|h| h.path != "gone.yml"),
        "a deleted node is still findable: {hits:?}"
    );
    assert!(hits.iter().any(|h| h.path == "keep.yml"));
}

/// The witness is in the index and is never a result.
///
/// The exclusion is a positive `corpus` predicate on every query rather than a `$ne`, and the
/// unit tests assert that against an evaluator written in this repository. This asserts it
/// against the one that actually decides.
#[test]
#[ignore]
fn the_witness_is_stored_and_never_returned() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);
    clear(&session, &index);

    let contract = yidam::embed_config::EmbedConfig::for_fastembed_model(
        "AllMiniLML6V2Q",
        DIMENSION as i32,
        "onnx/model_quantized.onnx",
        "AllMiniLML6V2Q",
    )
    .with_verification(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
    let witness = request::witness(&contract).unwrap();

    session
        .call(&request::put_vectors(&index, &[witness.clone()]).unwrap())
        .expect("putting the witness");
    session
        .call(&request::put_vectors(&index, &[row("a.yml", unit([1.0, 0.0, 0.0, 0.0]))]).unwrap())
        .expect("putting a row");

    // The witness's own vector is `[1, 0, 0, …]`, so this query points straight at it. If the
    // corpus predicate were dropped it would come back first.
    let hits = ops::query(
        &session,
        &index,
        CORPUS,
        &unit([1.0, 0.0, 0.0, 0.0]),
        &Filter::any(),
        |_| true,
        10,
    )
    .expect("querying");
    assert!(
        hits.iter().all(|h| h.path != WITNESS_KEY),
        "the witness was returned as a search result: {hits:?}"
    );

    // And it is fetchable by key, with the contract it was given.
    let got = ops::fetch_witness(&session, &index)
        .expect("fetching the witness")
        .expect("the witness is there");
    let carried: serde_json::Value =
        serde_json::from_str(got.metadata["embed_config"].as_str().unwrap()).unwrap();
    assert_eq!(carried["embedding_dim"], json!(DIMENSION));
}

/// A class filter is applied by the service, not after the fact.
#[test]
#[ignore]
fn a_pushed_class_filter_narrows_the_result() {
    if !enabled() {
        return;
    }
    let (client, index) = client();
    let session = Session::new(&client);
    ensure_index(&session, &index);
    clear(&session, &index);

    let mut other = row("other.yml", unit([1.0, 0.0, 0.0, 0.0]));
    other.metadata = request::metadata(CORPUS, "source", "Other", "deadbeef", "text")
        .unwrap()
        .0;
    session
        .call(
            &request::put_vectors(
                &index,
                &[row("concept.yml", unit([1.0, 0.05, 0.0, 0.0])), other],
            )
            .unwrap(),
        )
        .expect("putting");

    let hits = ops::query(
        &session,
        &index,
        CORPUS,
        &unit([1.0, 0.0, 0.0, 0.0]),
        &Filter::class(Some("concept")),
        |_| true,
        10,
    )
    .expect("querying");
    assert_eq!(
        hits.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
        vec!["concept.yml"],
        "the class filter did not narrow the result — `other.yml` is the nearer vector, so it \
         would come first if the filter were not applied"
    );
}
