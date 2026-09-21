//! Decoding what came back, and turning it into the ordering the local scan produces.
//!
//! # One `score`, or no answer
//!
//! The local path scores a row as the dot product of two L2-normalized vectors, which is the
//! cosine similarity. S3 Vectors returns a *distance*, and which distance depends on how the
//! index was created. A number in the wrong units would not look wrong — it would rank
//! plausibly and differently, which is the failure mode `embed.config.json` exists for on the
//! other axis. So the metric is checked on every response and a non-cosine index is refused
//! rather than converted.
//!
//! # Order is part of the answer
//!
//! `retrieval::vector::search` breaks ties on the path, deliberately: *"two rows at the same
//! score must come back in the same order on every run, or a golden that pins an entry node is
//! pinning the Arrow file's row layout."* A remote backend has no row layout at all, and the
//! service does not promise an order among equals either. [`finish`] applies the same rule, so
//! the two backends answer one question one way.

use serde_json::Value;

use super::{request::path_from_key, DISTANCE_METRIC, MAX_TOP_K};
use crate::retrieval::Hit;

/// How many rows to ask for per row wanted, when a residual filter will discard some locally.
///
/// The pushable half of a filter — the class set — is applied by the service *during* the
/// search, so what the residual discards is only what the class test could not express: an
/// anchored step's *"and this path resolves to a node this repository owns"*. That rejects
/// rarely, because the classes have already been narrowed.
///
/// Eight, which keeps the default `k` of 5 inside one 100-row page. It is a headroom figure
/// and not a measurement, and it is honest about that: when it is not enough, the read path
/// pages rather than returning short.
pub const OVERFETCH: usize = 8;

/// One page of query results.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub hits: Vec<Hit>,
    /// Present when the service has more to give.
    pub next: Option<String>,
}

/// What went wrong, in the only distinction the read path acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Worth one retry — throttling, a timeout, an unavailable service.
    Transient,
    /// Credentials or permissions. Retrying cannot help, and the repair is a human one.
    Denied,
    /// The bucket or index is not there.
    Missing,
    /// The request was wrong, or the answer was not what this code expects.
    Invalid,
}

/// A failed call: what kind, and what to tell a person.
///
/// Carried as one type so a caller can branch on [`Fault`] without discarding the service's
/// own words — an `AccessDeniedException` and a `NotFoundException` are both "it did not
/// answer", and only the message says which bucket or which permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub fault: Fault,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

impl Failure {
    /// A failure that never reached the service — a DNS failure, a refused connection, a body
    /// that did not parse. Transient by default, because the alternative reading is that the
    /// network is permanently broken.
    pub fn local(message: impl Into<String>) -> Self {
        Self {
            fault: Fault::Transient,
            message: message.into(),
        }
    }

    /// A malformed answer: the call succeeded and the response was not what this code reads.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            fault: Fault::Invalid,
            message: message.into(),
        }
    }

    /// Whether one retry is worth attempting.
    pub fn retryable(&self) -> bool {
        self.fault == Fault::Transient
    }
}

/// Classify an HTTP status the way the read path needs to branch on it.
///
/// The status codes are S3 Vectors': 403 `AccessDeniedException`, 404 `NotFoundException`,
/// 408 `RequestTimeoutException`, 429 `TooManyRequestsException`, 500
/// `InternalServerException`, 503 `ServiceUnavailableException`.
pub fn classify(status: u16) -> Fault {
    match status {
        403 => Fault::Denied,
        404 => Fault::Missing,
        408 | 429 | 500 | 503 => Fault::Transient,
        _ => Fault::Invalid,
    }
}

/// The service's own words about a failure, or a fallback naming the status.
///
/// S3 Vectors reports errors as JSON with a `message`, and the exception type in either a
/// `__type` field or the `x-amzn-errortype` header. Reading the body is worth the trouble: the
/// difference between *"the specified resource can't be found"* for a bucket and for an index
/// is the whole of what a person needs to fix it.
pub fn fault_message(status: u16, body: &[u8]) -> String {
    let parsed: Option<Value> = serde_json::from_slice(body).ok();
    let field = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let (kind, message) = match &parsed {
        Some(v) => (
            field(v, "__type").or_else(|| field(v, "code")),
            field(v, "message").or_else(|| field(v, "Message")),
        ),
        None => (None, None),
    };
    match (kind, message) {
        (Some(k), Some(m)) => format!("{status} {k}: {m}"),
        (None, Some(m)) => format!("{status}: {m}"),
        (Some(k), None) => format!("{status} {k}"),
        (None, None) => format!("HTTP {status}"),
    }
}

/// Cosine similarity from the distance the service returned.
///
/// Refuses any metric but cosine. An index created as `euclidean` is answerable — it just
/// answers in units nothing else in this crate calls a score, and silently mixing the two
/// would put incomparable numbers in `retrieve`'s results and in `bench`'s comparisons.
pub fn score_from_distance(metric: &str, distance: f64) -> Result<f32, String> {
    if metric != DISTANCE_METRIC {
        return Err(format!(
            "this index was created with the {metric} distance metric and this build reads \
             only {DISTANCE_METRIC} — its scores would not be the cosine similarity every \
             other path here calls a score. Recreate the index with {DISTANCE_METRIC}."
        ));
    }
    if !distance.is_finite() {
        return Err(format!(
            "the service returned a non-finite distance: {distance}"
        ));
    }
    Ok((1.0 - distance) as f32)
}

/// Why an existing index cannot hold this corpus, or `None` when it can.
///
/// Pure, and separate from the call that produces the body, so the field names can be checked
/// against `GetIndex`'s response shape by a test rather than by a reader. A pointer into the
/// wrong path would make this check silently unfireable — it would find no dimension, conclude
/// nothing, and let a push proceed into an index it does not fit.
///
/// It lives here rather than beside its one caller because a *live* test has to be able to run
/// it against a body the service actually sent. A unit test can only confirm the pointers match
/// the shape as documented; `tests/s3vectors_live.rs` confirms they match the shape as served,
/// which is the claim RFC-0033 §8 records.
pub fn disagreement(body: &serde_json::Value, dimension: u32, name: &str) -> Option<String> {
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
        .unwrap_or(DISTANCE_METRIC);
    if metric != DISTANCE_METRIC {
        return Some(format!(
            "the index {name} uses the {metric} distance metric and this build reads only \
             {DISTANCE_METRIC}.\n  \
             The metric is fixed when an index is created — push to a different index name."
        ));
    }
    None
}

/// Decode one `QueryVectors` response.
///
/// Rows whose key belongs to another corpus are skipped rather than returned: the caller asked
/// about one corpus, and a foreign row is not an answer to that question. With the filter this
/// crate sends there should be none — the skip is what keeps a filter bug from becoming a
/// result a person acts on.
pub fn decode_query(corpus: &str, body: &Value) -> Result<Page, String> {
    let metric = body
        .get("distanceMetric")
        .and_then(Value::as_str)
        .ok_or("the query response carries no distanceMetric")?;

    let rows = body
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or("the query response carries no vectors array")?;

    let mut hits = Vec::with_capacity(rows.len());
    for row in rows {
        let key = row
            .get("key")
            .and_then(Value::as_str)
            .ok_or("a returned vector has no key")?;
        let Some(path) = path_from_key(corpus, key) else {
            continue;
        };
        let distance = row
            .get("distance")
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("the vector {key} came back without a distance"))?;
        let meta = row.get("metadata").unwrap_or(&Value::Null);
        let text_field = |k: &str| {
            meta.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        hits.push(Hit {
            path: path.to_string(),
            class: text_field(super::META_KEY_CLASS),
            label: text_field(super::META_KEY_LABEL),
            text: text_field(super::META_KEY_TEXT),
            score: score_from_distance(metric, distance)?,
            // The push writes this key only on a row it had to cut, so its absence is the
            // statement that `text` is whole — see `request::metadata`. Read as a bool and
            // not merely tested for presence: a non-boolean under this key is a row this
            // crate did not write, and reading it as `true` would report a cut on the word
            // of something else.
            truncated: meta
                .get(super::META_KEY_TEXT_TRUNCATED)
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }

    Ok(Page {
        hits,
        next: body
            .get("nextToken")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string),
    })
}

/// Decode one `ListVectors` response into its keys and the token for the next page.
pub fn decode_list(body: &Value) -> Result<(Vec<String>, Option<String>), String> {
    let rows = body
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or("the list response carries no vectors array")?;
    let mut keys = Vec::with_capacity(rows.len());
    for row in rows {
        keys.push(
            row.get("key")
                .and_then(Value::as_str)
                .ok_or("a listed vector has no key")?
                .to_string(),
        );
    }
    Ok((
        keys,
        body.get("nextToken")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string),
    ))
}

/// One vector fetched by key.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    pub key: String,
    pub data: Vec<f32>,
    pub metadata: Value,
}

/// Decode a `GetVectors` response. A key that does not come back is simply absent from the
/// result — the service returns what it found and says nothing about what it did not.
pub fn decode_get(body: &Value) -> Result<Vec<Fetched>, String> {
    let rows = body
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or("the get response carries no vectors array")?;
    rows.iter()
        .map(|row| {
            let key = row
                .get("key")
                .and_then(Value::as_str)
                .ok_or("a fetched vector has no key")?
                .to_string();
            let data = match row.get("data").and_then(|d| d.get(super::DATA_TYPE)) {
                Some(Value::Array(xs)) => xs
                    .iter()
                    .map(|x| {
                        x.as_f64()
                            .map(|f| f as f32)
                            .ok_or_else(|| format!("the vector {key} has a non-numeric element"))
                    })
                    .collect::<Result<Vec<f32>, String>>()?,
                _ => Vec::new(),
            };
            Ok(Fetched {
                key,
                data,
                metadata: row.get("metadata").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

/// How many rows to ask the service for, to end up with `k` after a residual filter.
///
/// Clamped to what `topK` allows. The page size is a separate ceiling the caller pages
/// against — asking for 400 does not mean receiving 400 in one response.
pub fn top_k_request(k: usize, has_residual: bool) -> usize {
    let want = if has_residual {
        k.saturating_mul(OVERFETCH)
    } else {
        k
    };
    want.clamp(1, MAX_TOP_K)
}

/// Whether a caller should ask for another page: it still wants rows and the service has more.
pub fn wants_more(kept: usize, k: usize, next: &Option<String>) -> bool {
    kept < k && next.is_some()
}

/// Apply the residual filter, order, and cut to `k`.
///
/// The ordering is `retrieval::vector::search`'s, verbatim: score descending, ties broken on
/// the path ascending. Both backends sort here so neither can drift.
pub fn finish(mut hits: Vec<Hit>, residual: impl Fn(&Hit) -> bool, k: usize) -> Vec<Hit> {
    hits.retain(|h| residual(h));
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.path.cmp(&b.path))
    });
    hits.truncate(k);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(rows: Value, metric: &str) -> Value {
        json!({ "distanceMetric": metric, "vectors": rows })
    }

    #[test]
    fn a_cosine_distance_becomes_the_similarity_the_local_scan_produces() {
        // Identical vectors: distance 0, similarity 1.
        assert_eq!(score_from_distance("cosine", 0.0).unwrap(), 1.0);
        // Orthogonal: distance 1, similarity 0.
        assert_eq!(score_from_distance("cosine", 1.0).unwrap(), 0.0);
        // Opposed: distance 2, similarity -1.
        assert_eq!(score_from_distance("cosine", 2.0).unwrap(), -1.0);
    }

    /// The identity `score = 1 - distance` is an assumption about what AWS means by "cosine
    /// distance", and it is recorded as one. What this test pins is that the conversion is
    /// *monotone decreasing* — a nearer vector always scores higher — which is the property
    /// ranking depends on and which holds under any affine reading of the metric.
    ///
    /// The absolute values are checked against a live index by `tests/s3vectors_live.rs`.
    #[test]
    fn a_nearer_vector_always_scores_higher() {
        let mut last = f32::INFINITY;
        for d in [0.0, 0.05, 0.25, 0.5, 1.0, 1.5, 2.0] {
            let s = score_from_distance("cosine", d).unwrap();
            assert!(s < last, "distance {d} did not score below the previous");
            last = s;
        }
    }

    #[test]
    fn a_euclidean_index_is_refused_rather_than_converted() {
        let e = score_from_distance("euclidean", 0.4).unwrap_err();
        assert!(e.contains("euclidean") && e.contains("cosine"), "{e}");
        let body = response(json!([{"key": "c/a.yml", "distance": 0.1}]), "euclidean");
        assert!(decode_query("c", &body).is_err());
    }

    #[test]
    fn a_query_response_decodes_into_hits_with_their_metadata() {
        let body = response(
            json!([{
                "key": "abc123/corpus/concept/a.yml",
                "distance": 0.25,
                "metadata": {"class": "concept", "label": "A", "text": "some text"},
            }]),
            "cosine",
        );
        let page = decode_query("abc123", &body).unwrap();
        assert_eq!(page.next, None);
        assert_eq!(page.hits.len(), 1);
        let h = &page.hits[0];
        assert_eq!(h.path, "corpus/concept/a.yml");
        assert_eq!(h.class, "concept");
        assert_eq!(h.label, "A");
        assert_eq!(h.text, "some text");
        assert_eq!(h.score, 0.75);
    }

    #[test]
    fn a_row_from_another_corpus_is_skipped_rather_than_returned() {
        let body = response(
            json!([
                {"key": "other9/x.yml", "distance": 0.0, "metadata": {"class": "concept"}},
                {"key": "abc123/y.yml", "distance": 0.5, "metadata": {"class": "concept"}},
            ]),
            "cosine",
        );
        let page = decode_query("abc123", &body).unwrap();
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].path, "y.yml");
    }

    /// Missing metadata is a thin answer, not a failure: an index written by an older push, or
    /// by something other than this crate, still resolves to a node through its key.
    #[test]
    fn a_row_without_metadata_still_decodes() {
        let body = response(json!([{"key": "abc123/y.yml", "distance": 0.5}]), "cosine");
        let h = &decode_query("abc123", &body).unwrap().hits[0];
        assert_eq!(h.path, "y.yml");
        assert_eq!(h.class, "");
        assert_eq!(h.text, "");
    }

    /// A row without a distance is a failure, though — `returnDistance` was asked for, and a
    /// row scored zero by default would rank last while looking like a real answer.
    #[test]
    fn a_row_without_a_distance_is_an_error_rather_than_a_zero() {
        let body = response(json!([{"key": "abc123/y.yml"}]), "cosine");
        let e = decode_query("abc123", &body).unwrap_err();
        assert!(e.contains("distance"), "{e}");
    }

    #[test]
    fn a_response_missing_its_shape_says_which_part() {
        assert!(decode_query("c", &json!({"vectors": []}))
            .unwrap_err()
            .contains("distanceMetric"));
        assert!(decode_query("c", &json!({"distanceMetric": "cosine"}))
            .unwrap_err()
            .contains("vectors"));
    }

    #[test]
    fn an_empty_next_token_means_no_next_page() {
        let mut body = response(json!([]), "cosine");
        body["nextToken"] = json!("");
        assert_eq!(decode_query("c", &body).unwrap().next, None);
        body["nextToken"] = json!("t");
        assert_eq!(decode_query("c", &body).unwrap().next, Some("t".into()));
    }

    fn hit(path: &str, score: f32) -> Hit {
        Hit {
            path: path.to_string(),
            class: "concept".into(),
            label: String::new(),
            text: String::new(),
            score,
            truncated: false,
        }
    }

    /// Ties break on the path, and the input order must not show through.
    ///
    /// Two elements at equal score and both input orders, because a one-element fixture proves
    /// nothing about order and a single ordering can pass by accident.
    #[test]
    fn ties_break_on_the_path_whichever_order_they_arrive_in() {
        for input in [
            vec![hit("b.yml", 0.5), hit("a.yml", 0.5)],
            vec![hit("a.yml", 0.5), hit("b.yml", 0.5)],
        ] {
            let out = finish(input, |_| true, 10);
            assert_eq!(
                out.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
                vec!["a.yml", "b.yml"]
            );
        }
    }

    #[test]
    fn finishing_filters_then_orders_then_cuts() {
        let hits = vec![
            hit("low.yml", 0.1),
            hit("high.yml", 0.9),
            hit("dropped.yml", 0.95),
            hit("mid.yml", 0.5),
        ];
        let out = finish(hits, |h| h.path != "dropped.yml", 2);
        assert_eq!(
            out.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
            vec!["high.yml", "mid.yml"],
            "the residual must run before the cut, or `k` counts rows the caller cannot use"
        );
    }

    #[test]
    fn over_fetching_only_happens_when_something_will_be_discarded() {
        assert_eq!(top_k_request(5, false), 5);
        assert_eq!(top_k_request(5, true), 40);
        // Clamped to the service's ceiling rather than sent and rejected.
        assert_eq!(top_k_request(MAX_TOP_K, true), MAX_TOP_K);
        assert_eq!(top_k_request(0, false), 1);
    }

    #[test]
    fn another_page_is_wanted_only_while_rows_are_short_and_more_exist() {
        let some = Some("t".to_string());
        assert!(wants_more(3, 5, &some));
        assert!(!wants_more(5, 5, &some));
        assert!(!wants_more(3, 5, &None));
    }

    #[test]
    fn statuses_classify_into_what_the_read_path_branches_on() {
        assert_eq!(classify(403), Fault::Denied);
        assert_eq!(classify(404), Fault::Missing);
        for s in [408, 429, 500, 503] {
            assert_eq!(classify(s), Fault::Transient, "{s}");
        }
        assert_eq!(classify(400), Fault::Invalid);
        assert_eq!(classify(402), Fault::Invalid);
    }

    #[test]
    fn a_fault_message_prefers_the_services_own_words() {
        let body = br#"{"__type":"NotFoundException","message":"index not found"}"#;
        assert_eq!(
            fault_message(404, body),
            "404 NotFoundException: index not found"
        );
        assert_eq!(fault_message(503, b"not json"), "HTTP 503");
        assert_eq!(fault_message(400, br#"{"message":"bad"}"#), "400: bad");
    }

    #[test]
    fn listing_decodes_keys_and_its_token() {
        let body = json!({"vectors": [{"key": "a"}, {"key": "b"}], "nextToken": "t"});
        let (keys, next) = decode_list(&body).unwrap();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(next, Some("t".into()));
        assert!(decode_list(&json!({})).is_err());
    }

    #[test]
    fn a_fetched_vector_decodes_its_float32_arm() {
        let body = json!({"vectors": [{
            "key": "__yidam__/embed-config",
            "data": {"float32": [0.25, -0.5]},
            "metadata": {"model_id": "m"},
        }]});
        let got = decode_get(&body).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, "__yidam__/embed-config");
        assert_eq!(got[0].data, vec![0.25, -0.5]);
        assert_eq!(got[0].metadata["model_id"], "m");
        // A key that was asked for and does not exist is simply not in the answer.
        assert!(decode_get(&json!({"vectors": []})).unwrap().is_empty());
    }

    /// The page ceiling is the service's, and the read path pages against it rather than
    /// assuming `topK` arrives at once. Pinned so the two constants cannot be confused: a
    /// reader who takes `topK: 400` to mean four hundred rows in one response has written a
    /// loop that silently returns a quarter of what it was asked for.
    #[test]
    fn the_page_ceiling_is_far_below_the_top_k_ceiling() {
        assert_eq!(super::super::MAX_RESULTS_PER_PAGE, 100);
        assert_eq!(MAX_TOP_K, 10_000);
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
}
