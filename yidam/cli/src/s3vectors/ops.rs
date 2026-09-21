//! The loops: paging a query, mirroring a push, fetching the witness.
//!
//! # Why these are not in `transport`
//!
//! There is no emulator for S3 Vectors. No MinIO, no localstack path this crate can rely on —
//! which means a round trip cannot be exercised in CI at all, and anything sharing a module
//! with one cannot either. Paging, batching, retrying and the mirror diff are where the bugs
//! that matter live, so they take the call as an argument ([`Api`]) and are tested against a
//! recorded one. [`super::transport`] is left as a shim thin enough to read.
//!
//! # The mirror only ever deletes its own corpus's keys
//!
//! A push makes the remote index equal to the local one, which means deleting what is no
//! longer there — the lesson #807/#808 recorded, where an additive writer left entries nothing
//! could see. It also means a diff, and a diff against the *whole* index would delete the
//! witness and, once an index holds more than one corpus, another corpus's vectors.
//!
//! So [`plan_mirror`] considers only keys under this corpus's prefix. That is a safety
//! property rather than a nicety and it is asserted directly, because the cost of getting it
//! wrong is someone else's corpus.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;

use super::response::{self, Failure, Fetched, Page};
use super::{filter, request, Api, RemoteIndex, MAX_LIST_PAGE, WITNESS_KEY};
use crate::retrieval::{Filter, Hit};

/// One conversation with the service: a transport, and what to do when it says "later".
pub struct Session<'a> {
    api: &'a dyn Api,
    /// How to wait between attempts. Injected so a retry can be exercised without a test that
    /// sleeps.
    pause: fn(Duration),
    /// Total attempts per request, so `2` is one retry.
    attempts: u32,
}

/// How long to wait before the one retry.
///
/// A fixed pause and not a backoff schedule: there is exactly one retry, so a schedule would
/// have one entry. What it is for is a `TooManyRequestsException` on a burst — a push writes
/// up to 1,000 requests per second per index by AWS's own limit, and being told to slow down
/// is an expected part of that rather than a failure.
const RETRY_PAUSE: Duration = Duration::from_millis(500);

impl<'a> Session<'a> {
    pub fn new(api: &'a dyn Api) -> Self {
        Self {
            api,
            pause: std::thread::sleep,
            attempts: 2,
        }
    }

    /// One attempt only — for a caller that is already inside its own loop.
    pub fn without_retry(api: &'a dyn Api) -> Self {
        Self {
            api,
            pause: |_| {},
            attempts: 1,
        }
    }

    /// Send, retrying once on a transient fault.
    ///
    /// Only transient ones: a 403 retried is a 403, and a retry against a `ValidationException`
    /// is two identical rejections and twice the latency before the same message.
    pub fn call(&self, req: &request::Request) -> Result<Value, Failure> {
        // Written so the last attempt's failure is returned from where it happened. The
        // alternative — accumulating into an `Option` and unwrapping after the loop — needs a
        // panic path to express "a loop that ran at least once recorded something", and
        // `panic_paths.rs` ratchets those.
        let mut spent = 0;
        loop {
            match self.api.call(req) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    spent += 1;
                    if spent >= self.attempts || !e.retryable() {
                        return Err(e);
                    }
                    (self.pause)(RETRY_PAUSE);
                }
            }
        }
    }
}

/// Search the remote index, paging until `k` rows survive the residual filter.
///
/// `residual` is the half of a caller's filter that could not be pushed — see
/// [`crate::retrieval::Filter`]. Everything else was applied by the service during the search.
pub fn query(
    session: &Session,
    idx: &RemoteIndex,
    corpus: &str,
    query_vector: &[f32],
    filter: &Filter,
    residual: impl Fn(&Hit) -> bool,
    k: usize,
) -> Result<Vec<Hit>, Failure> {
    let pushed = filter::to_json(corpus, filter).map_err(|e| Failure::invalid(e.to_string()))?;
    let top_k = response::top_k_request(k, true);

    let mut collected: Vec<Hit> = Vec::new();
    let mut token: Option<String> = None;
    let mut seen_tokens: BTreeSet<String> = BTreeSet::new();

    loop {
        let req = request::query_vectors(idx, query_vector, top_k, &pushed, token.as_deref())
            .map_err(|e| Failure::invalid(e.to_string()))?;
        let body = session.call(&req)?;
        let Page { hits, next } =
            response::decode_query(corpus, &body).map_err(Failure::invalid)?;

        let empty = hits.is_empty();
        collected.extend(hits);

        // Enough surviving rows, or nothing more to ask for.
        let kept = collected.iter().filter(|h| residual(h)).count();
        if !response::wants_more(kept, k, &next) {
            break;
        }
        let Some(t) = next else { break };
        // A service that hands back a token it has already given would loop here forever, and
        // a page that is empty behind a token would do the same more slowly. Neither should
        // happen; both are cheap to refuse, and an infinite loop inside an MCP request handler
        // is not a failure anybody gets to see.
        if empty || !seen_tokens.insert(t.clone()) {
            break;
        }
        token = Some(t);
    }

    Ok(response::finish(collected, residual, k))
}

/// Fetch the embedding witness, or `None` when the index carries none.
///
/// `None` is the honest answer for an index pushed before the witness existed, and it maps
/// onto the same [`crate::embed_config::Verdict::Unverifiable`] a local index with no
/// `embed.config.json` produces. Failing closed here would break every such index at once.
pub fn fetch_witness(session: &Session, idx: &RemoteIndex) -> Result<Option<Fetched>, Failure> {
    let req = request::get_vectors(idx, &[WITNESS_KEY.to_string()], true, true)
        .map_err(|e| Failure::invalid(e.to_string()))?;
    let body = session.call(&req)?;
    let mut got = response::decode_get(&body).map_err(Failure::invalid)?;
    Ok(if got.is_empty() {
        None
    } else {
        Some(got.remove(0))
    })
}

/// Every key currently in the index, across all pages.
///
/// Includes the witness and any other corpus's keys — filtering is [`plan_mirror`]'s job, and
/// doing it here would make this function's name a lie about what it returned.
pub fn list_keys(session: &Session, idx: &RemoteIndex) -> Result<Vec<String>, Failure> {
    let mut keys = Vec::new();
    let mut token: Option<String> = None;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    loop {
        let req = request::list_vectors(idx, token.as_deref());
        let body = session.call(&req)?;
        let (page, next) = response::decode_list(&body).map_err(Failure::invalid)?;
        let empty = page.is_empty();
        keys.extend(page);
        let Some(t) = next else { break };
        if empty || !seen.insert(t.clone()) {
            break;
        }
        token = Some(t);
        // A page holds at most `MAX_LIST_PAGE`; an index at AWS's 2-billion ceiling would be
        // two million round trips. Nothing stops that here — listing an index is proportional
        // to the index — but the constant is named so the cost is visible at the call site.
        let _ = MAX_LIST_PAGE;
    }
    Ok(keys)
}

/// What a push would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorPlan {
    /// Keys this push will write. Every local row, since a put overwrites by key and the
    /// listing cannot say whether a vector's contents changed.
    pub put: usize,
    /// How many of those are not in the index yet. Reported because *"0 new, 412 written"* and
    /// *"412 new"* are very different sentences about the same push.
    pub new: usize,
    /// Keys under this corpus's prefix that the local index no longer has. These are deleted.
    pub delete: Vec<String>,
    /// Keys the push leaves alone: the witness, and anything belonging to another corpus.
    pub untouched: usize,
}

/// Diff the remote index against the keys a push is about to write.
///
/// Read-only — this is what `--dry-run` runs, and what the real push runs before writing.
pub fn plan_mirror(
    session: &Session,
    idx: &RemoteIndex,
    corpus: &str,
    local_keys: &[String],
) -> Result<MirrorPlan, Failure> {
    let local: BTreeSet<&str> = local_keys.iter().map(String::as_str).collect();
    let remote = list_keys(session, idx)?;

    let prefix = format!("{corpus}/");
    let mut delete = Vec::new();
    let mut untouched = 0;
    let mut present = 0;
    for key in &remote {
        // **The safety property.** Anything that is not this corpus's is not this push's to
        // remove: the witness lives beside the rows, and an index may hold another corpus.
        if key == WITNESS_KEY || !key.starts_with(&prefix) {
            untouched += 1;
            continue;
        }
        if local.contains(key.as_str()) {
            present += 1;
        } else {
            delete.push(key.clone());
        }
    }
    delete.sort();

    Ok(MirrorPlan {
        put: local_keys.len(),
        new: local_keys.len().saturating_sub(present),
        delete,
        untouched,
    })
}

/// Write the vectors, the witness, and the deletions the plan named.
///
/// Order matters and is not arbitrary: the puts go first, so a push interrupted between the
/// two halves leaves an index that is a superset of the truth rather than one missing rows
/// that exist. Writes are strongly consistent, so there is no window in which a just-written
/// row is invisible to the listing the next push makes.
pub fn apply_mirror(
    session: &Session,
    idx: &RemoteIndex,
    vectors: &[request::OutVector],
    witness: Option<&request::OutVector>,
    plan: &MirrorPlan,
) -> Result<(), Failure> {
    let batches =
        request::put_batches(idx, vectors).map_err(|e| Failure::invalid(e.to_string()))?;
    for batch in &batches {
        session.call(batch)?;
    }

    if let Some(w) = witness {
        let req = request::put_vectors(idx, std::slice::from_ref(w))
            .map_err(|e| Failure::invalid(e.to_string()))?;
        session.call(&req)?;
    }

    for batch in
        request::delete_batches(idx, &plan.delete).map_err(|e| Failure::invalid(e.to_string()))?
    {
        session.call(&batch)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3vectors::{Operation, RemoteIndexConfig, KIND, MAX_METADATA_BYTES};
    use serde_json::json;
    use std::cell::RefCell;

    fn idx() -> RemoteIndex {
        RemoteIndex::resolve(&RemoteIndexConfig {
            kind: KIND.to_string(),
            bucket: "yidam-corpora".to_string(),
            index: "yidam-main".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
        })
        .unwrap()
    }

    /// A transport that answers from a script and records what it was asked.
    struct Fake {
        replies: RefCell<Vec<Result<Value, Failure>>>,
        seen: RefCell<Vec<request::Request>>,
    }

    impl Fake {
        fn new(replies: Vec<Result<Value, Failure>>) -> Self {
            Self {
                replies: RefCell::new(replies),
                seen: RefCell::new(Vec::new()),
            }
        }
        fn ok(replies: Vec<Value>) -> Self {
            Self::new(replies.into_iter().map(Ok).collect())
        }
        fn calls(&self) -> Vec<request::Request> {
            self.seen.borrow().clone()
        }
    }

    impl Api for Fake {
        fn call(&self, request: &request::Request) -> Result<Value, Failure> {
            self.seen.borrow_mut().push(request.clone());
            let mut r = self.replies.borrow_mut();
            if r.is_empty() {
                return Err(Failure::invalid("the fake ran out of scripted replies"));
            }
            r.remove(0)
        }
    }

    /// A session that retries without sleeping.
    fn session(api: &dyn Api) -> Session<'_> {
        Session {
            api,
            pause: |_| {},
            attempts: 2,
        }
    }

    fn page(rows: Value, next: Option<&str>) -> Value {
        let mut v = json!({"distanceMetric": "cosine", "vectors": rows});
        if let Some(t) = next {
            v["nextToken"] = json!(t);
        }
        v
    }

    fn row(key: &str, distance: f64) -> Value {
        json!({"key": key, "distance": distance, "metadata": {"class": "concept", "label": "l", "text": "t"}})
    }

    /// The push cut a row, said so in its metadata, and the read path has to say so too.
    ///
    /// **The metadata is built by the push rather than written by hand**, which is the point of
    /// the test: the flag is a contract between two modules, and a fixture that spells the key
    /// itself would pass while `request::metadata` renamed it. So the row that arrives here is
    /// the row `index-push` would have sent for a catalog source too large to carry whole —
    /// which is not hypothetical, one such source exists across the sixteen corpora #848
    /// measured (RFC-0033 §8.3).
    ///
    /// The two rows are the same in everything but size, so what the assertion holds is that
    /// they come back **distinguishable**. A test asserting only that the field exists would
    /// pass against a decoder that hard-coded `false`.
    #[test]
    fn a_row_the_push_had_to_cut_comes_back_saying_so() {
        let whole = "x".repeat(64);
        let over = "y".repeat(MAX_METADATA_BYTES * 2);
        let meta = |text: &str| request::metadata("c1", "concept", "l", "dead", text).unwrap();
        let (whole_meta, whole_cut) = meta(&whole);
        let (over_meta, over_cut) = meta(&over);
        assert!(!whole_cut, "the short row was not the whole-text case");
        assert!(over_cut, "the long row was not the truncated case");

        let api = Fake::ok(vec![page(
            json!([
                {"key": "c1/whole.yml", "distance": 0.1, "metadata": whole_meta},
                {"key": "c1/cut.yml", "distance": 0.2, "metadata": over_meta},
            ]),
            None,
        )]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1, 0.2],
            &Filter::any(),
            |_| true,
            2,
        )
        .unwrap();

        assert_eq!(
            hits.iter()
                .map(|h| (h.path.as_str(), h.truncated))
                .collect::<Vec<_>>(),
            vec![("whole.yml", false), ("cut.yml", true)]
        );
        // And the flag is about something: the cut row really is short of what was pushed.
        assert_eq!(hits[0].text, whole);
        assert!(hits[1].text.len() < over.len());
    }

    #[test]
    fn a_transient_failure_is_retried_once_and_a_denial_is_not() {
        let api = Fake::new(vec![
            Err(Failure {
                fault: response::Fault::Transient,
                message: "429".into(),
            }),
            Ok(json!({"ok": true})),
        ]);
        assert!(session(&api).call(&request::get_index(&idx())).is_ok());
        assert_eq!(api.calls().len(), 2);

        let api = Fake::new(vec![
            Err(Failure {
                fault: response::Fault::Denied,
                message: "403".into(),
            }),
            Ok(json!({"ok": true})),
        ]);
        assert!(session(&api).call(&request::get_index(&idx())).is_err());
        assert_eq!(api.calls().len(), 1, "a denial was retried");
    }

    #[test]
    fn a_transient_failure_that_persists_reports_the_services_message() {
        let fail = || {
            Err(Failure {
                fault: response::Fault::Transient,
                message: "503 ServiceUnavailableException".into(),
            })
        };
        let api = Fake::new(vec![fail(), fail()]);
        let e = session(&api).call(&request::get_index(&idx())).unwrap_err();
        assert_eq!(e.fault, response::Fault::Transient);
        assert!(e.message.contains("503"));
        assert_eq!(api.calls().len(), 2);
    }

    #[test]
    fn a_query_that_is_satisfied_by_one_page_makes_one_call() {
        let api = Fake::ok(vec![page(
            json!([row("c1/a.yml", 0.1), row("c1/b.yml", 0.2)]),
            None,
        )]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1, 0.2],
            &Filter::any(),
            |_| true,
            2,
        )
        .unwrap();
        assert_eq!(api.calls().len(), 1);
        assert_eq!(
            hits.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
            vec!["a.yml", "b.yml"]
        );
        assert_eq!(api.calls()[0].op, Operation::QueryVectors);
    }

    /// The residual filter is what makes a second page necessary — the service already applied
    /// everything that could be pushed.
    #[test]
    fn a_query_pages_until_enough_rows_survive_the_residual() {
        let api = Fake::ok(vec![
            page(
                json!([row("c1/skip1.yml", 0.1), row("c1/keep1.yml", 0.2)]),
                Some("t1"),
            ),
            page(
                json!([row("c1/skip2.yml", 0.3), row("c1/keep2.yml", 0.4)]),
                Some("t2"),
            ),
            page(json!([row("c1/keep3.yml", 0.5)]), None),
        ]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1],
            &Filter::any(),
            |h| h.path.starts_with("keep"),
            3,
        )
        .unwrap();
        assert_eq!(api.calls().len(), 3);
        assert_eq!(
            hits.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
            vec!["keep1.yml", "keep2.yml", "keep3.yml"]
        );
        // The second and third requests carried the tokens the previous page handed back.
        assert_eq!(api.calls()[1].body["nextToken"], "t1");
        assert_eq!(api.calls()[2].body["nextToken"], "t2");
    }

    #[test]
    fn a_query_stops_paging_once_it_has_what_it_asked_for() {
        let api = Fake::ok(vec![page(
            json!([row("c1/a.yml", 0.1), row("c1/b.yml", 0.2)]),
            Some("t1"),
        )]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1],
            &Filter::any(),
            |_| true,
            2,
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(api.calls().len(), 1, "it paged past what it needed");
    }

    /// A service that repeated a token, or answered a token with nothing, would otherwise spin
    /// forever — inside an MCP request handler, where nobody sees it.
    #[test]
    fn a_repeated_or_empty_page_ends_the_paging_rather_than_looping() {
        let api = Fake::ok(vec![
            page(json!([row("c1/a.yml", 0.1)]), Some("same")),
            page(json!([row("c1/b.yml", 0.2)]), Some("same")),
            page(json!([row("c1/c.yml", 0.3)]), Some("same")),
        ]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1],
            &Filter::any(),
            |_| true,
            99,
        )
        .unwrap();
        assert_eq!(api.calls().len(), 2, "a repeated token was followed twice");
        assert_eq!(hits.len(), 2);

        let api = Fake::ok(vec![page(json!([]), Some("t"))]);
        let hits = query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1],
            &Filter::any(),
            |_| true,
            5,
        )
        .unwrap();
        assert!(hits.is_empty());
        assert_eq!(api.calls().len(), 1);
    }

    #[test]
    fn a_query_pushes_its_class_filter_and_over_fetches() {
        let api = Fake::ok(vec![page(json!([]), None)]);
        query(
            &session(&api),
            &idx(),
            "c1",
            &[0.1],
            &Filter::class(Some("concept")),
            |_| true,
            5,
        )
        .unwrap();
        let body = &api.calls()[0].body;
        assert_eq!(body["topK"], 40, "k was not over-fetched");
        assert_eq!(
            body["filter"],
            json!({"$and": [{"corpus": {"$eq": "c1"}}, {"class": {"$eq": "concept"}}]})
        );
    }

    #[test]
    fn the_witness_is_fetched_by_key_and_its_absence_is_not_an_error() {
        let api = Fake::ok(vec![json!({"vectors": [{
            "key": WITNESS_KEY,
            "data": {"float32": [0.25]},
            "metadata": {"model_id": "m"},
        }]})]);
        let w = fetch_witness(&session(&api), &idx()).unwrap().unwrap();
        assert_eq!(w.data, vec![0.25]);
        assert_eq!(api.calls()[0].body["keys"], json!([WITNESS_KEY]));
        assert_eq!(api.calls()[0].body["returnData"], json!(true));

        let api = Fake::ok(vec![json!({"vectors": []})]);
        assert!(fetch_witness(&session(&api), &idx()).unwrap().is_none());
    }

    #[test]
    fn listing_follows_every_page() {
        let api = Fake::ok(vec![
            json!({"vectors": [{"key": "c1/a"}], "nextToken": "t1"}),
            json!({"vectors": [{"key": "c1/b"}], "nextToken": "t2"}),
            json!({"vectors": [{"key": "c1/c"}]}),
        ]);
        let keys = list_keys(&session(&api), &idx()).unwrap();
        assert_eq!(keys, vec!["c1/a", "c1/b", "c1/c"]);
        assert_eq!(api.calls().len(), 3);
        assert_eq!(api.calls()[2].body["nextToken"], "t2");
    }

    fn listing(keys: &[&str]) -> Value {
        json!({"vectors": keys.iter().map(|k| json!({"key": k})).collect::<Vec<_>>()})
    }

    #[test]
    fn the_plan_deletes_what_the_local_index_no_longer_has() {
        let api = Fake::ok(vec![listing(&["c1/a.yml", "c1/gone.yml", "c1/b.yml"])]);
        let local = vec![
            "c1/a.yml".to_string(),
            "c1/b.yml".to_string(),
            "c1/new.yml".to_string(),
        ];
        let plan = plan_mirror(&session(&api), &idx(), "c1", &local).unwrap();
        assert_eq!(plan.put, 3);
        assert_eq!(plan.new, 1);
        assert_eq!(plan.delete, vec!["c1/gone.yml"]);
        assert_eq!(plan.untouched, 0);
    }

    /// The safety property: a push touches its own corpus and nothing else.
    ///
    /// Both halves matter and neither is hypothetical — the witness is in every index this
    /// crate writes, and a second corpus is the next phase. A diff taken against the whole
    /// listing would delete both.
    #[test]
    fn the_plan_never_deletes_the_witness_or_another_corpus() {
        let api = Fake::ok(vec![listing(&[
            WITNESS_KEY,
            "c1/a.yml",
            "c1/gone.yml",
            "other9/theirs.yml",
            "other9/also-theirs.yml",
        ])]);
        let plan = plan_mirror(&session(&api), &idx(), "c1", &["c1/a.yml".to_string()]).unwrap();
        assert_eq!(plan.delete, vec!["c1/gone.yml"]);
        assert_eq!(plan.untouched, 3, "the witness and two foreign keys");
        assert!(!plan.delete.iter().any(|k| k == WITNESS_KEY));
        assert!(!plan.delete.iter().any(|k| k.starts_with("other9/")));
    }

    /// A corpus whose short hash is a prefix of another's must not reach into it. `c1` and
    /// `c12` differ by one character and a `starts_with` on the bare hash would match both.
    #[test]
    fn a_corpus_whose_prefix_starts_another_does_not_touch_it() {
        let api = Fake::ok(vec![listing(&["c1/a.yml", "c12/theirs.yml"])]);
        let plan = plan_mirror(&session(&api), &idx(), "c1", &[]).unwrap();
        assert_eq!(plan.delete, vec!["c1/a.yml"]);
        assert_eq!(plan.untouched, 1);
    }

    fn out(key: &str) -> request::OutVector {
        request::OutVector {
            key: key.to_string(),
            data: vec![0.1, 0.2],
            metadata: json!({"class": "concept"}),
        }
    }

    #[test]
    fn applying_a_plan_puts_before_it_deletes() {
        let api = Fake::ok(vec![json!({}), json!({}), json!({})]);
        let plan = MirrorPlan {
            put: 1,
            new: 1,
            delete: vec!["c1/gone.yml".to_string()],
            untouched: 0,
        };
        let witness = out(WITNESS_KEY);
        apply_mirror(
            &session(&api),
            &idx(),
            &[out("c1/a.yml")],
            Some(&witness),
            &plan,
        )
        .unwrap();
        let ops: Vec<Operation> = api.calls().iter().map(|c| c.op).collect();
        assert_eq!(
            ops,
            vec![
                Operation::PutVectors,
                Operation::PutVectors,
                Operation::DeleteVectors
            ],
            "an interrupted push must leave a superset of the truth, not a gap"
        );
        assert_eq!(api.calls()[1].body["vectors"][0]["key"], WITNESS_KEY);
        assert_eq!(api.calls()[2].body["keys"], json!(["c1/gone.yml"]));
    }

    #[test]
    fn a_plan_with_nothing_to_delete_sends_no_delete() {
        let api = Fake::ok(vec![json!({})]);
        let plan = MirrorPlan {
            put: 1,
            new: 0,
            delete: Vec::new(),
            untouched: 0,
        };
        apply_mirror(&session(&api), &idx(), &[out("c1/a.yml")], None, &plan).unwrap();
        assert_eq!(api.calls().len(), 1);
        assert_eq!(api.calls()[0].op, Operation::PutVectors);
    }
}
