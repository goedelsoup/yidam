//! Amazon S3 Vectors — a vector index a corpus can be queried out of instead of carried.
//!
//! # What this is for
//!
//! `.yidam/index/` is built only by a binary compiled `--features index` — protoc 31 plus an
//! ONNX runtime — so the index exists on whichever machine could build it. RFC-0023 gave it a
//! way to travel as a tarball a reader fetches whole. This is the other answer: the vectors
//! live in a vector bucket, and a reader queries them.
//!
//! **It does not put semantic search in the light build, and that is the assumption worth
//! killing up front.** Answering a query still means embedding the query text, which is
//! `fastembed`, which is `vector-read`. What stops being local is the corpus's vectors, not
//! the model. The light build's `no_vector_support` answer is unchanged by every line here.
//!
//! # Why it costs no dependency
//!
//! `reqwest`, `tokio`, `hmac`, `sha2`, `hex` and `serde_json` are all in the `default` feature
//! set already — through `tonpa`, `vault-s3` and `catalog-fetch`. S3 Vectors is JSON over
//! POST with SigV4, so there is nothing else to buy. That is what lets this module sit in the
//! default build, where **every pull request compiles it**: `Cargo.toml` records four separate
//! occasions on which gated code shipped without a single PR having built it.
//!
//! # The split, and what it is a mitigation for
//!
//! Everything that decides *what would be sent* and *what came back* is here, pure, ungated
//! and unit-tested: [`request`] builds bodies, [`filter`] translates a [`crate::retrieval::Filter`],
//! [`response`] decodes and orders. [`transport`] is the only module that performs I/O.
//!
//! So the logic is testable from the build CI actually compiles on a pull request, which is
//! `cmd/catalog/location.rs`'s shape — anything gated takes gated facts as arguments.
//!
//! # Every limit here is AWS's, and is named rather than discovered
//!
//! The constants below are quoted from the S3 Vectors documentation, not inferred from a
//! failure. A request that would exceed one is refused here with a message that says which,
//! rather than at the service with a `ValidationException` a caller cannot act on.

pub mod filter;
pub mod ops;
pub mod request;
pub mod response;
#[cfg(feature = "s3-vectors")]
pub mod transport;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// One round trip to the service.
///
/// A trait with one method, and the reason is testability rather than choice: there is no
/// emulator for S3 Vectors — no MinIO, no localstack path this crate can rely on — so a loop
/// that pages, batches, retries or diffs cannot be exercised against anything real in CI.
/// Taking the call as an argument is what lets [`ops`] be tested exhaustively while
/// [`transport`] stays a thin, obvious shim over `reqwest`.
///
/// It returns [`response::Failure`] rather than `anyhow::Error` because the caller branches on
/// the kind: the read path degrades on a [`response::Fault::Transient`] and reports a
/// [`response::Fault::Denied`], and an opaque error would collapse the two.
pub trait Api {
    fn call(&self, request: &request::Request) -> Result<serde_json::Value, response::Failure>;
}

/// The `kind` an `[index.remote]` section must name to mean this backend.
///
/// A closed vocabulary of one. The point is not the choice — there is nothing else to choose
/// — it is that `kind = "s3vectors"` is refused rather than parsed into a default, which is
/// the argument `vault::config::ARTIFACT_KINDS` makes about a misspelled `holds` entry.
pub const KIND: &str = "s3-vectors";

/// The reserved key the embedding contract is stored under.
///
/// `embed.config.json` is what stops a binary querying an index built in another vector space
/// (#536), and a remote index has no file to carry it. So it is carried as a vector: this key,
/// with the verification probe as its data and the contract as its metadata.
///
/// Under `__yidam__/` rather than a bare name so it cannot collide with a repository path.
pub const WITNESS_KEY: &str = "__yidam__/embed-config";

/// The `corpus` value the witness carries, and the reason every query filters on `corpus`.
///
/// The witness is a vector in the same index as the corpus's own, so an unfiltered query could
/// return it. The exclusion is not a `$ne` — it is that every real query pushes
/// `corpus = <genesis>` as a positive predicate, which the witness fails by construction.
///
/// A positive predicate rather than a negative one on purpose: `$ne` against a vector that
/// lacks the key has semantics this code would be guessing at, and a guess that is wrong
/// leaks an internal record into a user's search results.
pub const META_CORPUS: &str = "__yidam_meta";

/// Metadata key names. Filterable unless named in [`NON_FILTERABLE_KEYS`].
pub const META_KEY_CORPUS: &str = "corpus";
pub const META_KEY_CLASS: &str = "class";
pub const META_KEY_LABEL: &str = "label";
pub const META_KEY_COMMIT: &str = "commit";
pub const META_KEY_TEXT: &str = "text";
/// The embedding contract, as a JSON string, on the witness record.
pub const META_KEY_EMBED_CONFIG: &str = "embed_config";
/// Set when [`META_KEY_TEXT`] was cut to fit the per-vector metadata ceiling.
///
/// Present so a short answer is never silently a whole one. A consumer reading `text` and
/// finding no such key is reading all of it.
pub const META_KEY_TEXT_TRUNCATED: &str = "text_truncated";

/// The metadata keys declared non-filterable when an index is created.
///
/// **This is the one irreversible decision in the whole design.** AWS: *"Once a metadata key
/// is designated as non-filterable during index creation, it can't be changed to filterable
/// later."* The set is fixed at `CreateIndex` and an index that wants a different one is a
/// different index.
///
/// `text` is here because it is the large field and filtering on it is meaningless, and
/// `embed_config` because the witness carries a whole JSON document — filterable metadata is
/// capped at 2 KB and typed as string, number, boolean or list, so a nested object has no
/// business there.
///
/// **`AMAZON_BEDROCK_TEXT` and `AMAZON_BEDROCK_METADATA` are here for a phase that has not
/// been built**, which normally would not justify a line of code. It justifies these two: they
/// are what Bedrock Knowledge Bases requires to be non-filterable, declaring them is free, and
/// declaring them *later* is impossible. An index created without them can only be replaced.
/// Three of the ten keys an index is allowed.
pub const NON_FILTERABLE_KEYS: &[&str] = &[
    "text",
    "embed_config",
    "AMAZON_BEDROCK_TEXT",
    "AMAZON_BEDROCK_METADATA",
];

/// The only element type S3 Vectors stores.
pub const DATA_TYPE: &str = "float32";

/// The distance metric this crate creates indexes with, and the only one it will read.
///
/// S3 Vectors offers `euclidean` too. Reading one would mean returning numbers that are not
/// the cosine similarity every other path in this crate calls a `score` — `retrieve`'s
/// results, an anchored step's ranking, `bench`'s comparisons — so a query against a euclidean
/// index is refused rather than answered in different units. See
/// [`response::score_from_distance`].
pub const DISTANCE_METRIC: &str = "cosine";

// ── AWS's limits, quoted ──────────────────────────────────────────────────────
/// Max vectors in one `PutVectors` or `DeleteVectors` call.
pub const MAX_VECTORS_PER_WRITE: usize = 500;
/// Max keys in one `GetVectors` call.
pub const MAX_KEYS_PER_GET: usize = 100;
/// Max request payload, any operation.
pub const MAX_PAYLOAD_BYTES: usize = 20 * 1024 * 1024;
/// Max `topK` on a `QueryVectors` request.
pub const MAX_TOP_K: usize = 10_000;
/// Max results in one `QueryVectors` response page — **two orders of magnitude below
/// [`MAX_TOP_K`]**, which is why the read path pages rather than assuming one round trip.
pub const MAX_RESULTS_PER_PAGE: usize = 100;
/// Max vectors in one `ListVectors` response page.
pub const MAX_LIST_PAGE: usize = 1000;
/// Max length of a vector key, in characters.
pub const MAX_KEY_LEN: usize = 1024;
/// Max total metadata per vector, filterable and non-filterable together.
pub const MAX_METADATA_BYTES: usize = 40 * 1024;
/// Max filterable metadata per vector.
pub const MAX_FILTERABLE_METADATA_BYTES: usize = 2 * 1024;
/// Max dimension of a stored vector.
pub const MAX_DIMENSION: u32 = 4096;

/// Bucket and index names: 3–63 characters, per `CreateIndex`'s length constraints.
const MIN_NAME_LEN: usize = 3;
const MAX_NAME_LEN: usize = 63;

/// What a repository declares about the vector index it publishes to.
///
/// ```toml
/// [index.remote]
/// kind   = "s3-vectors"
/// bucket = "yidam-corpora"
/// index  = "yidam-main"
/// region = "us-east-1"
/// ```
///
/// Never a credential, for the reason `vault::config` gives at length: `.yidam/config.toml` is
/// committed, and this repository has already found an untracked `.env` that its own prescribed
/// `git add -A` would have staged.
///
/// `deny_unknown_fields`, so `regoin = "us-east-1"` is refused rather than silently leaving the
/// region unset.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RemoteIndexConfig {
    /// Must be [`KIND`].
    pub kind: String,
    /// The vector bucket's name.
    pub bucket: String,
    /// The vector index's name, within that bucket.
    pub index: String,
    /// The signing region, which also derives the endpoint.
    ///
    /// **Required, with no default** — unlike `vault::VaultConfig::region`, which defaults to
    /// `us-east-1` because a MinIO on a laptop has no meaningful region to declare. There is
    /// no S3 Vectors emulator and no S3-Vectors-compatible store: the region is always a real
    /// AWS region, it selects the endpoint as well as the scope, and defaulting it would point
    /// a misconfigured repository at a bucket in Virginia it does not own.
    pub region: String,
    /// An explicit endpoint, for a PrivateLink or dual-stack address. Absent means the
    /// regional public endpoint.
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// The six operations, each its own path. There are no query strings and no path parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    CreateIndex,
    GetIndex,
    PutVectors,
    GetVectors,
    ListVectors,
    QueryVectors,
    DeleteVectors,
}

impl Operation {
    /// The request path, with its leading slash.
    ///
    /// Every one of these is unreserved characters, which is the property
    /// `vault::sigv4::canonical_uri` relies on: SigV4 requires non-S3 services to double-encode
    /// the canonical URI and that signer single-encodes, and the two agree exactly on strings
    /// that need no encoding at all. `a_path_needs_no_encoding` asserts it for all seven, so a
    /// future operation whose name is not unreserved goes red here rather than producing a
    /// valid-looking rejected signature.
    pub fn path(self) -> &'static str {
        match self {
            Self::CreateIndex => "/CreateIndex",
            Self::GetIndex => "/GetIndex",
            Self::PutVectors => "/PutVectors",
            Self::GetVectors => "/GetVectors",
            Self::ListVectors => "/ListVectors",
            Self::QueryVectors => "/QueryVectors",
            Self::DeleteVectors => "/DeleteVectors",
        }
    }

    /// All of them, so a test over the set cannot go stale as one is added.
    pub const ALL: [Self; 7] = [
        Self::CreateIndex,
        Self::GetIndex,
        Self::PutVectors,
        Self::GetVectors,
        Self::ListVectors,
        Self::QueryVectors,
        Self::DeleteVectors,
    ];
}

/// A validated remote index: where it is, and what to sign for it.
///
/// Separate from [`RemoteIndexConfig`] because that is whatever the committed file said and
/// this is what survived being checked. Constructing one is the only way to reach [`transport`],
/// so a malformed endpoint is a parse error at startup rather than a 403 at first query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteIndex {
    pub bucket: String,
    pub index: String,
    pub region: String,
    /// `https`, or whatever an explicit endpoint named.
    scheme: String,
    /// Host and port — the value signed as, and sent as, `host`.
    host: String,
}

impl RemoteIndex {
    /// Check a declaration and resolve its endpoint.
    pub fn resolve(cfg: &RemoteIndexConfig) -> Result<Self> {
        if cfg.kind != KIND {
            bail!(
                "[index.remote] kind {:?} is not a backend this build knows — expected {KIND:?}",
                cfg.kind
            );
        }
        check_name("bucket", &cfg.bucket)?;
        check_name("index", &cfg.index)?;
        if cfg.region.trim().is_empty() {
            bail!("[index.remote] names no region — it selects the endpoint as well as the signing scope, so there is nothing sensible to default it to");
        }

        let (scheme, host) = match &cfg.endpoint {
            Some(e) => split_endpoint(e.trim_end_matches('/'))
                .with_context(|| format!("[index.remote] endpoint {e:?}"))?,
            // The regional public endpoint. `.api.aws`, not `.amazonaws.com` — S3 Vectors is a
            // dual-stack-only service and its endpoints are published under the newer suffix.
            None => (
                "https".to_string(),
                format!("s3vectors.{}.api.aws", cfg.region),
            ),
        };

        Ok(Self {
            bucket: cfg.bucket.clone(),
            index: cfg.index.clone(),
            region: cfg.region.clone(),
            scheme,
            host,
        })
    }

    /// The host header — signed verbatim, so this is also what the request must be sent to.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The absolute URL for one operation.
    pub fn url(&self, op: Operation) -> String {
        format!("{}://{}{}", self.scheme, self.host, op.path())
    }

    /// What `doctor` and `--dry-run` print. Never a credential.
    pub fn describe(&self) -> String {
        format!(
            "s3 vectors: bucket {} index {} in {} ({})",
            self.bucket, self.index, self.region, self.host
        )
    }
}

fn check_name(what: &str, name: &str) -> Result<()> {
    if name.len() < MIN_NAME_LEN || name.len() > MAX_NAME_LEN {
        bail!(
            "[index.remote] {what} {name:?} is {} characters — S3 Vectors allows {MIN_NAME_LEN} to {MAX_NAME_LEN}",
            name.len()
        );
    }
    Ok(())
}

/// Split `scheme://host[:port]`, refusing anything that is not that.
///
/// `vault::s3` does the same thing for the same reason and the two are deliberately not shared:
/// that one is reached through a `VaultConfig` and this one through an `[index.remote]`, and a
/// single function would have to produce an error message naming one of the two sections.
fn split_endpoint(endpoint: &str) -> Result<(String, String)> {
    let (scheme, authority) = endpoint
        .split_once("://")
        .filter(|(s, a)| !s.is_empty() && !a.is_empty())
        .context("has no scheme — expected `https://host[:port]`")?;
    if authority.contains('/') {
        bail!("has a path — expected `https://host[:port]` and nothing after the host");
    }
    Ok((scheme.to_string(), authority.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RemoteIndexConfig {
        RemoteIndexConfig {
            kind: KIND.to_string(),
            bucket: "yidam-corpora".to_string(),
            index: "yidam-main".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
        }
    }

    #[test]
    fn the_regional_endpoint_is_derived_from_the_region() {
        let r = RemoteIndex::resolve(&cfg()).unwrap();
        assert_eq!(r.host(), "s3vectors.us-east-1.api.aws");
        assert_eq!(
            r.url(Operation::QueryVectors),
            "https://s3vectors.us-east-1.api.aws/QueryVectors"
        );

        // A different region is a different endpoint, not just a different scope. Without
        // this, a resolver that hardcoded the host would pass the assertion above.
        let mut eu = cfg();
        eu.region = "eu-west-1".to_string();
        assert_eq!(
            RemoteIndex::resolve(&eu).unwrap().host(),
            "s3vectors.eu-west-1.api.aws"
        );
    }

    #[test]
    fn an_explicit_endpoint_replaces_the_derived_one_and_keeps_its_scheme() {
        let mut c = cfg();
        c.endpoint = Some("https://vpce-0abc.s3vectors.us-east-1.vpce.amazonaws.com/".to_string());
        let r = RemoteIndex::resolve(&c).unwrap();
        assert_eq!(r.host(), "vpce-0abc.s3vectors.us-east-1.vpce.amazonaws.com");
        assert!(r.url(Operation::PutVectors).ends_with("/PutVectors"));
    }

    #[test]
    fn an_endpoint_without_a_scheme_or_with_a_path_is_refused_rather_than_guessed_at() {
        let mut c = cfg();
        c.endpoint = Some("s3vectors.us-east-1.api.aws".to_string());
        let e = RemoteIndex::resolve(&c).unwrap_err().to_string();
        assert!(e.contains("endpoint"), "{e}");

        let mut c = cfg();
        c.endpoint = Some("https://host/v1".to_string());
        assert!(RemoteIndex::resolve(&c).is_err());
    }

    #[test]
    fn a_kind_this_build_does_not_know_is_refused() {
        let mut c = cfg();
        c.kind = "s3vectors".to_string();
        let e = RemoteIndex::resolve(&c).unwrap_err().to_string();
        assert!(e.contains("s3-vectors"), "{e}");
    }

    #[test]
    fn a_name_outside_the_services_length_constraints_is_refused_here() {
        let mut c = cfg();
        c.bucket = "ab".to_string();
        let e = RemoteIndex::resolve(&c).unwrap_err().to_string();
        assert!(e.contains("bucket") && e.contains("3"), "{e}");

        let mut c = cfg();
        c.index = "i".repeat(64);
        assert!(RemoteIndex::resolve(&c)
            .unwrap_err()
            .to_string()
            .contains("index"));
    }

    #[test]
    fn a_missing_region_is_refused_rather_than_defaulted() {
        let mut c = cfg();
        c.region = "  ".to_string();
        assert!(RemoteIndex::resolve(&c)
            .unwrap_err()
            .to_string()
            .contains("region"));
    }

    /// Every operation path is unreserved characters, which is what makes
    /// `vault::sigv4::canonical_uri` — an S3-style single encoder — correct for a service that
    /// is not S3. The assertion is on the paths, because that is where the property lives.
    #[test]
    fn a_path_needs_no_encoding() {
        for op in Operation::ALL {
            let p = op.path();
            assert!(p.starts_with('/'), "{p} has no leading slash");
            assert!(
                p[1..]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')),
                "{p} contains a character SigV4's two encodings disagree about"
            );
        }
    }

    #[test]
    fn describe_names_the_index_and_never_a_credential() {
        let d = RemoteIndex::resolve(&cfg()).unwrap().describe();
        assert!(d.contains("yidam-corpora") && d.contains("yidam-main"));
        assert!(d.contains("us-east-1"));
    }

    /// The non-filterable set is fixed at CreateIndex and cannot be changed afterwards, so it
    /// is pinned here: changing it is changing what a new index is, and every index already
    /// created keeps the old set forever.
    #[test]
    fn the_non_filterable_keys_are_pinned_and_within_the_services_ceiling() {
        assert_eq!(
            NON_FILTERABLE_KEYS,
            &[
                "text",
                "embed_config",
                "AMAZON_BEDROCK_TEXT",
                "AMAZON_BEDROCK_METADATA"
            ]
        );
        assert!(NON_FILTERABLE_KEYS.len() <= 10);
        assert!(NON_FILTERABLE_KEYS.contains(&META_KEY_TEXT));
        // The filterable keys must NOT be in it — a key named here cannot be filtered on, and
        // `class` is what every pushed filter is written in terms of.
        for k in [
            META_KEY_CORPUS,
            META_KEY_CLASS,
            META_KEY_LABEL,
            META_KEY_COMMIT,
        ] {
            assert!(
                !NON_FILTERABLE_KEYS.contains(&k),
                "{k} would be unfilterable"
            );
        }
    }
}
