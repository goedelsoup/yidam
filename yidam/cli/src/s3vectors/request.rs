//! The request bodies, as pure functions.
//!
//! Nothing here performs I/O, reads a clock or touches the environment — the same discipline
//! `vault::sigv4` keeps, and for the same payoff: every body a real send would put on the wire
//! can be asserted exactly, and `--dry-run` prints the true thing rather than an approximation
//! of it.
//!
//! # Limits are refused here, not discovered at the service
//!
//! A `ValidationException` names a field and a constraint; it does not name the node whose text
//! was too long or the batch that was too big. So each limit AWS publishes is checked here,
//! where the message can say which node and what to do.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{
    Operation, RemoteIndex, DATA_TYPE, DISTANCE_METRIC, MAX_DIMENSION,
    MAX_FILTERABLE_METADATA_BYTES, MAX_KEYS_PER_GET, MAX_KEY_LEN, MAX_LIST_PAGE,
    MAX_METADATA_BYTES, MAX_PAYLOAD_BYTES, MAX_TOP_K, MAX_VECTORS_PER_WRITE, META_CORPUS,
    META_KEY_CLASS, META_KEY_COMMIT, META_KEY_CORPUS, META_KEY_EMBED_CONFIG, META_KEY_LABEL,
    META_KEY_TEXT, META_KEY_TEXT_TRUNCATED, NON_FILTERABLE_KEYS, WITNESS_KEY,
};

/// One request, ready to be signed.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub op: Operation,
    pub body: Value,
}

impl Request {
    /// The body as it goes on the wire. The digest signed as `x-amz-content-sha256` is this
    /// string's, so it is produced once and both signed and sent — a body serialized twice is
    /// a body that can differ twice.
    ///
    /// `Result` rather than an `expect`, though serializing a `Value` this module built cannot
    /// realistically fail. The reason is not defensive: `panic_paths.rs` ratchets production
    /// panic paths, and the alternatives to a panic here are all worse than a `?` — an empty
    /// body would be *sent*, and a default one would be signed and rejected with a message
    /// about a field rather than about this.
    pub fn bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(&self.body)
            .with_context(|| format!("rendering the {} body", self.op.path()))
    }
}

/// One vector on its way to the index.
#[derive(Debug, Clone, PartialEq)]
pub struct OutVector {
    pub key: String,
    pub data: Vec<f32>,
    pub metadata: Value,
}

/// How many characters of the genesis hash a corpus is identified by.
///
/// Twelve characters of a SHA-1, which is what the rest of this repository uses when it needs
/// a short commit and is comfortably past the point where two corpora collide.
///
/// It lives here rather than in `cmd::index_push` because the push is behind two features and
/// this is not: anything that wants to know what a row would cost — `yidam embed --dry-run`
/// measuring against the ceiling, say — needs the same twelve characters in the same `corpus`
/// field, and a second copy of `take(12)` would be a second answer waiting to disagree.
pub const CORPUS_ID_LEN: usize = 12;

/// The corpus identity a key and a row carry, from the genesis hash.
pub fn corpus_id(genesis_hash: &str) -> String {
    genesis_hash.chars().take(CORPUS_ID_LEN).collect()
}

/// The key a node's row is stored under: `<corpus>/<repo-relative path>`.
///
/// Prefixed by the corpus from the first push, which costs nothing now and is what lets an
/// index hold more than one corpus later without every key having to move.
pub fn vector_key(corpus: &str, path: &str) -> Result<String> {
    let key = format!("{corpus}/{path}");
    if key.chars().count() > MAX_KEY_LEN {
        bail!(
            "the key for {path} is {} characters — S3 Vectors allows {MAX_KEY_LEN}. \
             Shorten the path; truncating it here would make two nodes share one key.",
            key.chars().count()
        );
    }
    if key.is_empty() {
        bail!("a vector key cannot be empty");
    }
    Ok(key)
}

/// The path back out of a key, or `None` when the key belongs to another corpus.
///
/// Returns `None` rather than the whole key for a foreign one: a path that is silently a key
/// resolves to no node and looks like an index built before a file moved, which is a different
/// diagnosis with a different repair.
pub fn path_from_key<'a>(corpus: &str, key: &'a str) -> Option<&'a str> {
    key.strip_prefix(corpus)?.strip_prefix('/')
}

/// The bytes the service counts against the *filterable* ceiling: every key not named in
/// [`NON_FILTERABLE_KEYS`].
///
/// A separate figure from the whole row's, and not derivable from it, which is why it is a
/// function rather than a subtraction: `text` and `embed_config` are the two large fields and
/// both are excluded, so a row at 39 KB of metadata may be 200 bytes of filterable metadata.
///
/// **This counts the filterable keys as a JSON object**, braces, quotes and key names
/// included. AWS publishes the 2 KB limit and not the accounting behind it, so this is the
/// conservative reading — it can only over-count, which makes the local refusal stricter than
/// the service rather than looser. Over the sixteen corpora RFC-0033 §8.3 measured, the
/// largest figure it returns anywhere is 237 bytes, so the difference between readings is
/// nowhere near load-bearing.
pub fn filterable_len(metadata: &Value) -> usize {
    let filterable: serde_json::Map<String, Value> = metadata
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| !NON_FILTERABLE_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    json_len(&Value::Object(filterable))
}

/// Refuse a row whose filterable half exceeds [`MAX_FILTERABLE_METADATA_BYTES`].
///
/// **Cutting `text` cannot fix this**, which is the whole reason it is checked separately.
/// `text` is non-filterable, so the shrink loop below moves this number not at all; a row that
/// broke the 2 KB ceiling would pass the 40 KB check, be truncated or not, go on the wire, and
/// come back a `ValidationException` naming a constraint rather than a node.
///
/// `label` is the only one of the four filterable keys a corpus controls the size of —
/// `corpus` is twelve characters, `commit` is seven, `class` is a directory name — so the
/// message names it.
fn refuse_if_unfilterable(m: &Value, class: &str, label: &str) -> Result<()> {
    let len = filterable_len(m);
    if len > MAX_FILTERABLE_METADATA_BYTES {
        bail!(
            "the filterable metadata for a {class} node is {len} bytes — S3 Vectors allows \
             {MAX_FILTERABLE_METADATA_BYTES}, and `text` is not in that budget, so truncating \
             it would not help. Its label is {} bytes.",
            label.len()
        );
    }
    Ok(())
}

/// The metadata for one row, and whether its text had to be cut to fit.
///
/// The ceiling is AWS's 40 KB per vector, and `text` is the only field that can approach it.
/// Cutting is done on a character boundary and announced with [`META_KEY_TEXT_TRUNCATED`], so a
/// consumer reading `text` and finding no such flag is reading all of it.
///
/// The 2 KB filterable ceiling is checked too, and separately — see [`refuse_if_unfilterable`]
/// for why one check cannot stand in for the other.
pub fn metadata(
    corpus: &str,
    class: &str,
    label: &str,
    commit: &str,
    text: &str,
) -> Result<(Value, bool)> {
    let build = |text: &str, truncated: bool| {
        let mut m = json!({
            META_KEY_CORPUS: corpus,
            META_KEY_CLASS: class,
            META_KEY_LABEL: label,
            META_KEY_COMMIT: commit,
            META_KEY_TEXT: text,
        });
        if truncated {
            m[META_KEY_TEXT_TRUNCATED] = json!(true);
        }
        m
    };

    let full = build(text, false);
    if json_len(&full) <= MAX_METADATA_BYTES {
        refuse_if_unfilterable(&full, class, label)?;
        return Ok((full, false));
    }

    // Everything but `text` must fit on its own, or there is nothing to cut. A label that large
    // is a data error, and truncating it would be inventing a shorter title for a node.
    let without_text = json_len(&build("", true));
    if without_text >= MAX_METADATA_BYTES {
        bail!(
            "the metadata for a {class} node exceeds the {MAX_METADATA_BYTES}-byte ceiling \
             before its text is counted — its label is {} bytes",
            label.len()
        );
    }

    // Shrink to fit. JSON escaping means a byte of text is not always a byte of body, so the
    // budget is found by measuring rather than by subtracting — one loop, halving the overshoot,
    // over a string that is at most tens of kilobytes.
    let mut cut = text.len();
    loop {
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let candidate = build(&text[..cut], true);
        if json_len(&candidate) <= MAX_METADATA_BYTES {
            refuse_if_unfilterable(&candidate, class, label)?;
            return Ok((candidate, true));
        }
        if cut == 0 {
            bail!("the metadata for a {class} node cannot be made to fit its ceiling");
        }
        cut = cut.saturating_sub((cut / 8).max(1));
    }
}

fn json_len(v: &Value) -> usize {
    serde_json::to_vec(v).map(|b| b.len()).unwrap_or(usize::MAX)
}

/// The witness record: the embedding contract, stored as a vector because that is the only
/// kind of thing a vector index holds.
///
/// **The `data` is a placeholder and the contract is the metadata.** A vector index requires
/// every record to carry a vector of the index's dimension, and what needs to travel here is a
/// JSON document. So the data is the first basis vector — `[1, 0, 0, …]` — and the reason it
/// is not zeros is that a zero vector has no direction: a cosine index asked to compare
/// against one has nothing to compute, and what it does about that is unspecified.
///
/// Nothing reads the data. A consumer verifies the way it does against a local index: it
/// embeds [`crate::embed_config::VERIFICATION_PROBE`] with its own model and compares the
/// result against the prefix recorded in the contract. Storing the full probe embedding would
/// let that comparison use every dimension rather than the first eight, and it would also mean
/// a push had to load the model — which `index-build` has already done, on a machine this push
/// may not be running on.
///
/// `corpus` is [`META_CORPUS`], which is how every real query excludes it.
pub fn witness(config: &crate::embed_config::EmbedConfig) -> Result<OutVector> {
    let dimension = usize::try_from(config.embedding_dim)
        .map_err(|_| anyhow::anyhow!("the contract records a negative embedding_dim"))?;
    if dimension == 0 || dimension > MAX_DIMENSION as usize {
        bail!(
            "the contract records an embedding_dim of {dimension}, outside the \
             1..={MAX_DIMENSION} S3 Vectors allows"
        );
    }
    let mut data = vec![0.0_f32; dimension];
    data[0] = 1.0;

    let encoded = serde_json::to_string(config)
        .map_err(|e| anyhow::anyhow!("rendering the embedding contract: {e}"))?;
    let metadata = json!({
        META_KEY_CORPUS: META_CORPUS,
        META_KEY_EMBED_CONFIG: encoded,
    });
    if json_len(&metadata) > MAX_METADATA_BYTES {
        bail!("the embedding contract does not fit the {MAX_METADATA_BYTES}-byte metadata ceiling");
    }

    Ok(OutVector {
        key: WITNESS_KEY.to_string(),
        data,
        metadata,
    })
}

/// `CreateIndex`, with every choice this crate makes about a new index.
///
/// The `metadataConfiguration` is the irreversible half — see [`NON_FILTERABLE_KEYS`].
pub fn create_index(idx: &RemoteIndex, dimension: u32) -> Result<Request> {
    if dimension == 0 || dimension > MAX_DIMENSION {
        bail!("dimension {dimension} is outside the 1..={MAX_DIMENSION} S3 Vectors allows");
    }
    Ok(Request {
        op: Operation::CreateIndex,
        body: json!({
            "vectorBucketName": idx.bucket,
            "indexName": idx.index,
            "dataType": DATA_TYPE,
            "dimension": dimension,
            "distanceMetric": DISTANCE_METRIC,
            "metadataConfiguration": { "nonFilterableMetadataKeys": NON_FILTERABLE_KEYS },
        }),
    })
}

/// `GetIndex` — what an existing index says it is, so a push can refuse to write into one
/// whose dimension or metric disagrees with what it holds.
pub fn get_index(idx: &RemoteIndex) -> Request {
    Request {
        op: Operation::GetIndex,
        body: json!({ "vectorBucketName": idx.bucket, "indexName": idx.index }),
    }
}

/// `PutVectors` for one batch.
///
/// Callers should reach this through [`put_batches`], which is what enforces the two limits a
/// batch can break.
pub fn put_vectors(idx: &RemoteIndex, vectors: &[OutVector]) -> Result<Request> {
    if vectors.is_empty() {
        bail!("a PutVectors request must carry at least one vector");
    }
    if vectors.len() > MAX_VECTORS_PER_WRITE {
        bail!(
            "{} vectors in one PutVectors — S3 Vectors allows {MAX_VECTORS_PER_WRITE}",
            vectors.len()
        );
    }
    let encoded: Result<Vec<Value>> = vectors.iter().map(encode_vector).collect();
    Ok(Request {
        op: Operation::PutVectors,
        body: json!({
            "vectorBucketName": idx.bucket,
            "indexName": idx.index,
            "vectors": encoded?,
        }),
    })
}

/// Split `vectors` into requests none of which breaks the count or payload limit.
///
/// Both limits, because either can bind first: 500 vectors of a 384-dimension model is about
/// 3 MB, but the same 500 carrying 40 KB of text each is 20 MB.
pub fn put_batches(idx: &RemoteIndex, vectors: &[OutVector]) -> Result<Vec<Request>> {
    let mut out = Vec::new();
    let mut batch: Vec<OutVector> = Vec::new();
    // The envelope — bucket, index, the `vectors` key — costs a little, so a batch is closed
    // before the limit rather than at it.
    let overhead = 512;

    let mut size = overhead;
    for v in vectors {
        let each = json_len(&encode_vector(v)?) + 1;
        if each + overhead > MAX_PAYLOAD_BYTES {
            bail!(
                "the row for {} is {each} bytes on its own, over the {MAX_PAYLOAD_BYTES}-byte \
                 request ceiling",
                v.key
            );
        }
        if !batch.is_empty()
            && (batch.len() == MAX_VECTORS_PER_WRITE || size + each > MAX_PAYLOAD_BYTES)
        {
            out.push(put_vectors(idx, &batch)?);
            batch.clear();
            size = overhead;
        }
        size += each;
        batch.push(v.clone());
    }
    if !batch.is_empty() {
        out.push(put_vectors(idx, &batch)?);
    }
    Ok(out)
}

fn encode_vector(v: &OutVector) -> Result<Value> {
    // **Non-finite floats become `null` in JSON**, silently, because that is what
    // `serde_json::Number::from_f64` does with them. A vector with a `null` element is not a
    // vector, and the service would reject the request with a message about a type rather than
    // about a NaN. An embedder producing one has already gone wrong; this is where it is said.
    if let Some(bad) = v.data.iter().position(|f| !f.is_finite()) {
        bail!(
            "the vector for {} has a non-finite value at element {bad} — JSON cannot carry one, \
             and it would be sent as null",
            v.key
        );
    }
    Ok(json!({
        "key": v.key,
        "data": { DATA_TYPE: v.data },
        "metadata": v.metadata,
    }))
}

/// `QueryVectors`. `filter` comes from [`super::filter::to_json`].
pub fn query_vectors(
    idx: &RemoteIndex,
    query: &[f32],
    top_k: usize,
    filter: &Value,
    next_token: Option<&str>,
) -> Result<Request> {
    if query.is_empty() {
        bail!("a query vector cannot be empty");
    }
    if let Some(bad) = query.iter().position(|f| !f.is_finite()) {
        bail!("the query vector has a non-finite value at element {bad}");
    }
    if top_k == 0 || top_k > MAX_TOP_K {
        bail!("topK {top_k} is outside the 1..={MAX_TOP_K} S3 Vectors allows");
    }
    let mut body = json!({
        "vectorBucketName": idx.bucket,
        "indexName": idx.index,
        "queryVector": { DATA_TYPE: query },
        "topK": top_k,
        "filter": filter,
        "returnDistance": true,
        // Both of these, and each costs a permission: `returnMetadata` — and any filter at all
        // — makes the request need `s3vectors:GetVectors` as well as `s3vectors:QueryVectors`,
        // and without it the service answers 403. Named here because the 403 does not say so.
        "returnMetadata": true,
    });
    if let Some(t) = next_token {
        body["nextToken"] = json!(t);
    }
    Ok(Request {
        op: Operation::QueryVectors,
        body,
    })
}

/// `GetVectors` — used for the witness, and for nothing else so far.
pub fn get_vectors(
    idx: &RemoteIndex,
    keys: &[String],
    data: bool,
    metadata: bool,
) -> Result<Request> {
    if keys.is_empty() {
        bail!("a GetVectors request must name at least one key");
    }
    if keys.len() > MAX_KEYS_PER_GET {
        bail!(
            "{} keys in one GetVectors — S3 Vectors allows {MAX_KEYS_PER_GET}",
            keys.len()
        );
    }
    Ok(Request {
        op: Operation::GetVectors,
        body: json!({
            "vectorBucketName": idx.bucket,
            "indexName": idx.index,
            "keys": keys,
            "returnData": data,
            "returnMetadata": metadata,
        }),
    })
}

/// `ListVectors`, one page. Keys only.
///
/// `returnData` and `returnMetadata` stay false deliberately: the mirror needs the set of keys
/// that exist, nothing more, and asking for either would make the request require
/// `s3vectors:GetVectors` on top of `s3vectors:ListVectors`. A push that only ever writes
/// should not need read permission on vector contents.
pub fn list_vectors(idx: &RemoteIndex, next_token: Option<&str>) -> Request {
    let mut body = json!({
        "vectorBucketName": idx.bucket,
        "indexName": idx.index,
        "maxResults": MAX_LIST_PAGE,
        "returnData": false,
        "returnMetadata": false,
    });
    if let Some(t) = next_token {
        body["nextToken"] = json!(t);
    }
    Request {
        op: Operation::ListVectors,
        body,
    }
}

/// `DeleteVectors` for one batch of keys.
pub fn delete_vectors(idx: &RemoteIndex, keys: &[String]) -> Result<Request> {
    if keys.is_empty() {
        bail!("a DeleteVectors request must name at least one key");
    }
    if keys.len() > MAX_VECTORS_PER_WRITE {
        bail!(
            "{} keys in one DeleteVectors — S3 Vectors allows {MAX_VECTORS_PER_WRITE}",
            keys.len()
        );
    }
    Ok(Request {
        op: Operation::DeleteVectors,
        body: json!({
            "vectorBucketName": idx.bucket,
            "indexName": idx.index,
            "keys": keys,
        }),
    })
}

/// Split `keys` into `DeleteVectors` requests that respect the batch limit.
pub fn delete_batches(idx: &RemoteIndex, keys: &[String]) -> Result<Vec<Request>> {
    keys.chunks(MAX_VECTORS_PER_WRITE)
        .map(|c| delete_vectors(idx, c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx() -> RemoteIndex {
        RemoteIndex::resolve(&super::super::RemoteIndexConfig {
            kind: super::super::KIND.to_string(),
            bucket: "yidam-corpora".to_string(),
            index: "yidam-main".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
        })
        .unwrap()
    }

    fn vector(key: &str, text: &str) -> OutVector {
        OutVector {
            key: key.to_string(),
            data: vec![0.5, -0.5],
            metadata: metadata("abc123", "concept", "A label", "deadbee", text)
                .unwrap()
                .0,
        }
    }

    #[test]
    fn create_index_declares_every_irreversible_choice() {
        let r = create_index(&idx(), 384).unwrap();
        assert_eq!(r.op, Operation::CreateIndex);
        assert_eq!(r.body["dataType"], "float32");
        assert_eq!(r.body["dimension"], 384);
        assert_eq!(r.body["distanceMetric"], "cosine");
        assert_eq!(
            r.body["metadataConfiguration"]["nonFilterableMetadataKeys"],
            json!([
                "text",
                "embed_config",
                "AMAZON_BEDROCK_TEXT",
                "AMAZON_BEDROCK_METADATA"
            ])
        );
        assert_eq!(r.body["vectorBucketName"], "yidam-corpora");
        assert_eq!(r.body["indexName"], "yidam-main");
    }

    fn contract(dim: i32) -> crate::embed_config::EmbedConfig {
        crate::embed_config::EmbedConfig::for_fastembed_model(
            "AllMiniLML6V2Q",
            dim,
            "onnx/model_quantized.onnx",
            "AllMiniLML6V2Q",
        )
        .with_verification(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8])
    }

    /// The witness is the contract, carried by a vector that means nothing.
    #[test]
    fn the_witness_carries_the_contract_and_a_vector_of_the_indexs_dimension() {
        let w = witness(&contract(384)).unwrap();
        assert_eq!(w.key, WITNESS_KEY);
        assert_eq!(w.data.len(), 384);
        assert_eq!(w.metadata[META_KEY_CORPUS], META_CORPUS);

        let round: crate::embed_config::EmbedConfig =
            serde_json::from_str(w.metadata[META_KEY_EMBED_CONFIG].as_str().unwrap()).unwrap();
        assert_eq!(round.embedding_dim, 384);
        assert_eq!(
            round.verification.unwrap().prefix,
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
        );
    }

    /// Not a zero vector: a cosine index has nothing to compute against one, and what it does
    /// about that is unspecified rather than documented.
    #[test]
    fn the_witness_vector_has_a_direction() {
        let w = witness(&contract(8)).unwrap();
        let norm: f32 = w.data.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm > 0.0, "the witness vector is all zeros");
        assert!(w.data.iter().all(|x| x.is_finite()));
        // And it is a vector a put would accept.
        assert!(put_vectors(&idx(), &[w]).is_ok());
    }

    /// Every real query filters on its own corpus, and the witness fails that filter. Asserted
    /// here as well as in `filter`, because this is where the value is written.
    #[test]
    fn the_witness_is_not_in_any_corpus() {
        let w = witness(&contract(8)).unwrap();
        assert_ne!(w.metadata[META_KEY_CORPUS], json!("abc123"));
        assert_eq!(w.metadata[META_KEY_CORPUS], json!(META_CORPUS));
        assert_eq!(path_from_key("abc123", &w.key), None);
    }

    #[test]
    fn a_contract_with_an_impossible_dimension_is_refused() {
        assert!(witness(&contract(0)).is_err());
        assert!(witness(&contract(-1)).is_err());
        assert!(witness(&contract(MAX_DIMENSION as i32 + 1)).is_err());
    }

    #[test]
    fn a_dimension_outside_the_services_range_is_refused() {
        assert!(create_index(&idx(), 0).is_err());
        assert!(create_index(&idx(), MAX_DIMENSION + 1).is_err());
        assert!(create_index(&idx(), MAX_DIMENSION).is_ok());
    }

    #[test]
    fn a_key_carries_the_corpus_and_round_trips_back_to_a_path() {
        let k = vector_key("abc123", "corpus/concept/funding.yml").unwrap();
        assert_eq!(k, "abc123/corpus/concept/funding.yml");
        assert_eq!(
            path_from_key("abc123", &k),
            Some("corpus/concept/funding.yml")
        );
    }

    /// A key from another corpus resolves to no path — the property the cross-corpus phase
    /// rests on, asserted now so that phase does not have to discover it.
    #[test]
    fn a_key_from_another_corpus_does_not_resolve_here() {
        assert_eq!(path_from_key("abc123", "other9/corpus/x.yml"), None);
        // And a prefix that merely starts the same is not a match either.
        assert_eq!(path_from_key("abc", "abc123/corpus/x.yml"), None);
    }

    #[test]
    fn a_key_longer_than_the_service_allows_is_refused_rather_than_truncated() {
        let long = "a/".repeat(600);
        let e = vector_key("abc123", &long).unwrap_err().to_string();
        assert!(e.contains("1024"), "{e}");
        assert!(e.contains("share one key"), "{e}");
    }

    #[test]
    fn short_text_is_carried_whole_and_unflagged() {
        let (m, truncated) = metadata("abc123", "concept", "L", "dead", "hello").unwrap();
        assert!(!truncated);
        assert_eq!(m[META_KEY_TEXT], "hello");
        assert!(m.get(META_KEY_TEXT_TRUNCATED).is_none());
        assert_eq!(m[META_KEY_CORPUS], "abc123");
        assert_eq!(m[META_KEY_CLASS], "concept");
        assert_eq!(m[META_KEY_COMMIT], "dead");
    }

    #[test]
    fn oversize_text_is_cut_to_fit_and_says_so() {
        let text = "x".repeat(MAX_METADATA_BYTES * 2);
        let (m, truncated) = metadata("abc123", "concept", "L", "dead", &text).unwrap();
        assert!(truncated);
        assert_eq!(m[META_KEY_TEXT_TRUNCATED], json!(true));
        assert!(json_len(&m) <= MAX_METADATA_BYTES);
        // Cut, not emptied: the answer is still most of the text.
        assert!(m[META_KEY_TEXT].as_str().unwrap().len() > MAX_METADATA_BYTES / 2);
    }

    /// Text that escapes to more bytes than it occupies. A budget computed by subtracting
    /// `text.len()` from the ceiling would overshoot here and the service would reject it.
    #[test]
    fn text_that_expands_under_json_escaping_still_fits() {
        for filler in ["\"", "\\", "\n", "\u{7f}", "é"] {
            let text = filler.repeat(MAX_METADATA_BYTES);
            let (m, truncated) = metadata("abc123", "concept", "L", "dead", &text).unwrap();
            assert!(truncated, "{filler:?} was not truncated");
            assert!(
                json_len(&m) <= MAX_METADATA_BYTES,
                "{filler:?} produced {} bytes",
                json_len(&m)
            );
            // And the cut landed on a character boundary — `as_str` would not be reachable
            // otherwise, but the round trip is what proves the string is intact.
            assert!(m[META_KEY_TEXT].as_str().is_some());
        }
    }

    /// The gap #848 found: `MAX_FILTERABLE_METADATA_BYTES` was declared and never read.
    ///
    /// A 3 KB label is nowhere near the 40 KB row ceiling, so the check above passes it and
    /// the shrink loop never runs. Before this guard the row went on the wire and the service
    /// refused it, naming a constraint rather than a node.
    #[test]
    fn a_label_over_the_filterable_ceiling_is_refused_though_the_row_fits() {
        let label = "L".repeat(MAX_FILTERABLE_METADATA_BYTES + 1);
        let m = metadata("abc123", "concept", &label, "dead", "short");
        let e = m.unwrap_err().to_string();
        assert!(e.contains("filterable"), "{e}");
        assert!(e.contains("2048"), "{e}");
    }

    /// And the check is not made redundant by truncation, which is the reason it is separate:
    /// `text` is non-filterable, so cutting it moves the filterable figure by nothing. This
    /// row takes the shrink path and must still be refused.
    #[test]
    fn truncating_text_does_not_rescue_an_oversize_filterable_half() {
        let label = "L".repeat(MAX_FILTERABLE_METADATA_BYTES + 1);
        let text = "x".repeat(MAX_METADATA_BYTES * 2);
        assert!(metadata("abc123", "concept", &label, "dead", &text).is_err());
    }

    /// The four filterable keys, and nothing else. A row whose `text` is at the 40 KB ceiling
    /// is a few hundred bytes of filterable metadata — the two budgets do not track.
    #[test]
    fn filterable_len_excludes_the_large_fields() {
        let text = "x".repeat(MAX_METADATA_BYTES * 2);
        let (m, truncated) = metadata("abc123", "concept", "A label", "dead", &text).unwrap();
        assert!(truncated);
        assert!(json_len(&m) > MAX_METADATA_BYTES / 2);
        assert!(
            filterable_len(&m) < 128,
            "{} bytes filterable on a maximal row",
            filterable_len(&m)
        );
        // `text_truncated` is filterable and is counted; `text` is not and is not.
        assert!(
            filterable_len(&m)
                > filterable_len(
                    &metadata("abc123", "concept", "A label", "dead", "x")
                        .unwrap()
                        .0
                )
        );
    }

    /// Twelve characters, and the same twelve wherever a corpus is named — the push's keys
    /// and a dry run's measurement of what those keys would carry.
    #[test]
    fn a_corpus_is_identified_by_twelve_characters_of_its_genesis_hash() {
        assert_eq!(
            corpus_id("da4eeb36530f1111222233334444555566667777"),
            "da4eeb36530f"
        );
        // A hash shorter than the window is used whole rather than padded — a test fixture
        // repository has one, and panicking on it would make the command untestable.
        assert_eq!(corpus_id("abc"), "abc");
        assert_eq!(corpus_id(""), "");
    }

    #[test]
    fn a_label_too_large_to_leave_room_for_any_text_is_refused() {
        let label = "L".repeat(MAX_METADATA_BYTES + 1);
        let e = metadata("abc123", "concept", &label, "dead", "x")
            .unwrap_err()
            .to_string();
        assert!(e.contains("label"), "{e}");
    }

    #[test]
    fn a_non_finite_element_is_refused_rather_than_sent_as_null() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let v = OutVector {
                key: "abc123/x".to_string(),
                data: vec![0.1, bad],
                metadata: json!({}),
            };
            let e = put_vectors(&idx(), &[v]).unwrap_err().to_string();
            assert!(e.contains("non-finite") && e.contains("element 1"), "{e}");
        }
        // And the query side, which embeds rather than reads and can go wrong the same way.
        assert!(query_vectors(&idx(), &[0.1, f32::NAN], 5, &json!({}), None).is_err());
    }

    #[test]
    fn a_vector_is_encoded_as_the_float32_arm_of_the_union() {
        let r = put_vectors(&idx(), &[vector("abc123/a.yml", "t")]).unwrap();
        assert_eq!(r.op, Operation::PutVectors);
        assert_eq!(r.body["vectors"][0]["key"], "abc123/a.yml");
        assert_eq!(r.body["vectors"][0]["data"]["float32"], json!([0.5, -0.5]));
        assert_eq!(r.body["vectors"][0]["metadata"]["class"], "concept");
    }

    #[test]
    fn batches_respect_the_count_limit() {
        let vs: Vec<OutVector> = (0..MAX_VECTORS_PER_WRITE * 2 + 3)
            .map(|i| vector(&format!("abc123/{i}.yml"), "t"))
            .collect();
        let rs = put_batches(&idx(), &vs).unwrap();
        assert_eq!(rs.len(), 3);
        let counts: Vec<usize> = rs
            .iter()
            .map(|r| r.body["vectors"].as_array().unwrap().len())
            .collect();
        assert_eq!(counts, vec![500, 500, 3]);
        // Every vector went exactly once, in order.
        let sent: Vec<String> = rs
            .iter()
            .flat_map(|r| r.body["vectors"].as_array().unwrap().clone())
            .map(|v| v["key"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(sent.len(), vs.len());
        assert_eq!(sent[0], "abc123/0.yml");
        assert_eq!(sent[sent.len() - 1], format!("abc123/{}.yml", vs.len() - 1));
    }

    /// The payload limit binds before the count limit when rows are large, and a batch that
    /// broke it would be rejected whole.
    ///
    /// The numbers are why this case exists rather than being hypothetical. A row is capped at
    /// 40 KB of metadata plus its vector — about 7.7 KB for the 384 dimensions of the default
    /// model, rendered as JSON — so a full row runs to roughly 48 KB. Five hundred of those is
    /// 24 MB, over the 20 MiB ceiling: **the count limit alone is not enough**, and a batcher
    /// that only counted would send a request the service rejects whole.
    #[test]
    fn batches_respect_the_payload_limit_even_below_the_count_limit() {
        let fat = "x".repeat(MAX_METADATA_BYTES - 1024);
        let dims: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let vs: Vec<OutVector> = (0..MAX_VECTORS_PER_WRITE + 20)
            .map(|i| OutVector {
                data: dims.clone(),
                ..vector(&format!("abc123/{i}.yml"), &fat)
            })
            .collect();
        let rs = put_batches(&idx(), &vs).unwrap();
        assert!(
            rs.iter()
                .all(|r| r.body["vectors"].as_array().unwrap().len() < MAX_VECTORS_PER_WRITE),
            "a batch reached the count limit, so the payload limit was not what closed it"
        );
        for r in &rs {
            assert!(
                r.bytes().unwrap().len() <= MAX_PAYLOAD_BYTES,
                "a batch is {} bytes",
                r.bytes().unwrap().len()
            );
            assert!(r.body["vectors"].as_array().unwrap().len() <= MAX_VECTORS_PER_WRITE);
        }
        let total: usize = rs
            .iter()
            .map(|r| r.body["vectors"].as_array().unwrap().len())
            .sum();
        assert_eq!(total, vs.len(), "a row was dropped between batches");
    }

    #[test]
    fn deletes_are_batched_and_an_empty_one_is_refused() {
        let keys: Vec<String> = (0..1001).map(|i| format!("abc123/{i}")).collect();
        let rs = delete_batches(&idx(), &keys).unwrap();
        assert_eq!(rs.len(), 3);
        assert_eq!(rs[0].body["keys"].as_array().unwrap().len(), 500);
        assert_eq!(rs[2].body["keys"].as_array().unwrap().len(), 1);
        assert!(delete_vectors(&idx(), &[]).is_err());
        assert_eq!(delete_batches(&idx(), &[]).unwrap().len(), 0);
    }

    #[test]
    fn a_query_carries_its_filter_and_asks_for_what_the_read_path_needs() {
        let f = json!({"corpus": {"$eq": "abc123"}});
        let r = query_vectors(&idx(), &[0.1, 0.2], 25, &f, None).unwrap();
        assert_eq!(r.op, Operation::QueryVectors);
        assert_eq!(r.body["topK"], 25);
        assert_eq!(r.body["filter"], f);
        assert_eq!(r.body["returnDistance"], json!(true));
        assert_eq!(r.body["returnMetadata"], json!(true));
        assert!(r.body.get("nextToken").is_none());

        let paged = query_vectors(&idx(), &[0.1, 0.2], 25, &f, Some("tok")).unwrap();
        assert_eq!(paged.body["nextToken"], "tok");
    }

    #[test]
    fn a_top_k_outside_the_services_range_is_refused() {
        let f = json!({});
        assert!(query_vectors(&idx(), &[0.1], 0, &f, None).is_err());
        assert!(query_vectors(&idx(), &[0.1], MAX_TOP_K + 1, &f, None).is_err());
        assert!(query_vectors(&idx(), &[0.1], MAX_TOP_K, &f, None).is_ok());
    }

    /// `ListVectors` asks for keys only, which is what keeps a push from needing read
    /// permission on vector contents.
    #[test]
    fn listing_asks_for_no_data_and_no_metadata() {
        let r = list_vectors(&idx(), None);
        assert_eq!(r.body["returnData"], json!(false));
        assert_eq!(r.body["returnMetadata"], json!(false));
        assert_eq!(r.body["maxResults"], MAX_LIST_PAGE);
        assert_eq!(list_vectors(&idx(), Some("t")).body["nextToken"], "t");
    }

    #[test]
    fn get_vectors_refuses_more_keys_than_one_call_takes() {
        let keys: Vec<String> = (0..MAX_KEYS_PER_GET + 1).map(|i| i.to_string()).collect();
        assert!(get_vectors(&idx(), &keys, true, true).is_err());
        let one = get_vectors(&idx(), &["k".to_string()], true, true).unwrap();
        assert_eq!(one.body["returnData"], json!(true));
        assert_eq!(one.body["keys"], json!(["k"]));
    }

    /// The bytes signed and the bytes sent are one string. Two serializations of one `Value`
    /// agreeing is not guaranteed by anything in serde_json's contract about map order, and a
    /// body whose digest does not match what arrived is a 403 that names nothing.
    #[test]
    fn a_requests_bytes_are_stable_across_calls() {
        let r = put_vectors(&idx(), &[vector("abc123/a.yml", "t")]).unwrap();
        assert_eq!(r.bytes().unwrap(), r.bytes().unwrap());
        assert_eq!(
            serde_json::from_slice::<Value>(&r.bytes().unwrap()).unwrap(),
            r.body
        );
    }
}
