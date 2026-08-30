//! Counting the evidence markers in a node.
//!
//! Distinct from `yidam_core::corpus::extract_claims`, which parses whole *claims* —
//! line-oriented, one tag per line, returning the claim text. This counts *markers*
//! wherever they appear, which is a different question with a different answer.
//!
//! Properties are included because the corpus routinely records absence there rather than
//! in prose: `estimate: "[open] — not computed"` is a live open claim that a
//! description-only count misses entirely.
//!
//! Markers are matched as exact bracketed tokens. That is deliberately narrow — corpus
//! prose is dense with markdown links, and a looser match would read `[open questions](…)`
//! as an open claim. The three tokens never appear as link text, so exact matching has no
//! false positives to trade against.
//!
//! # A mention is not a claim
//!
//! Exact was not narrow enough. A node that *discusses* the evidence vocabulary — a sentence
//! saying a claim is not verified, with the token in backticks as code — was counted as
//! carrying that claim. The count was correct about the bytes and wrong about the corpus.
//!
//! This is not a hypothetical for any corpus whose subject touches its own evidentiary
//! apparatus, and such a node has good reason to name the tags and no way to do so without
//! being counted. Reported from a derived repository where `yidam status` published **1
//! verified claim against a true 0** for four commits, inside a `REGEN` block in `README.md`
//! — which no human writes and everyone therefore trusts. It was found because a reader
//! happened to hold an expectation about the number, not by any check.
//!
//! The frozen MCP contract already described the predicate this way — "the body contains an
//! `[open]` **claim**" — so telling a mention from a claim is agreeing with words the
//! contract already promised rather than changing them. What it did **not** say was how, and
//! that gap is where this went wrong once already.
//!
//! ## The typographic rule, and why it is gone
//!
//! The first answer was that a tag in backticks is a mention. Inline code is markdown's
//! conventional signal for mention-rather-than-use, it costs a corpus nothing to say what it
//! means, and it is wrong.
//!
//! Measured against a mature derived corpus — 99 nodes, ~800 tagged claims — it moved that
//! repository's own generated self-description:
//!
//! | | before | after |
//! |---|---:|---:|
//! | open questions, on the README | 72 | **26** |
//! | counted `[open]` claims | **227** | **43** |
//!
//! Because of how tags are actually written: of that corpus's 230 `[open]` tags, **184 are
//! backticked and they are claims**. An open question is written mid-sentence with the token
//! set off from the prose around it — *"Whether the money and the vote are connected is
//! `[open]`"* — while `[verified]` terminates a sentence of fact and reads fine bare, at 4%
//! backticked. The asymmetry is not one repository's house style; it follows from what each
//! tag is for.
//!
//! So the rule made a corpus **understate its open questions fivefold**, in a generated
//! block, on its front page, with no diagnostic. That is the flattering direction, and it is
//! the one direction this vocabulary exists to prevent: anything computing a publication
//! permission from the weakest tag in a supporting chain would promote blocks that should not
//! travel. A derived repository implemented the same rule, measured it, and threw it out.
//!
//! ## Grammar instead
//!
//! [`is_narrated`] decides, on three shapes and no others — a plural or possessive, a
//! past-tense reporting verb, or a negation. Two candidate arms were cut *by measurement*
//! downstream (copulas, and the present tense), and the reasons are recorded there.
//!
//! The rule is written into `prelude/sdks/parity/mcp/tools.json` with the three arms it
//! serves, because a contract that freezes which arms exist and leaves *what counts as a
//! claim* unsaid lets two conforming implementations disagree fivefold — which is what
//! happened.
//!
//! The opposite failure is left to `lint`. A tag with its citation folded inside the
//! brackets — `[verified — <source>]` — matches no token and is counted as *nothing*, just
//! as silently; a reader who writes that has plainly intended a tag. Counting it would
//! guess, so `claim-tag-malformed` reports it instead.
//!
//! # And a node may say so structurally
//!
//! A bracketed token in serialized text is a property of a node's *serialization*, not of
//! the node. A corpus whose evidence markers are structured — `claim_tag: open` in a
//! property, because it has a typed claim vocabulary and stores the tag rather than prose
//! about the tag — was invisible to every count here.
//!
//! Measured, by a consumer running the real binary over its own mirror: `open-questions`
//! returned **2 nodes against that repository's own count of 26**. Not a rounding
//! difference. The other 24 were open, said so in a machine-readable field, and were
//! counted as nothing. It closed the gap by serializing `claim_tag: "[open]"` — a data-model
//! decision made to satisfy a `contains()` call, with every reader normalizing it back.
//!
//! So a class may now declare which of its properties carry an evidence tag, and a value in
//! one of those is read as the tag it is. The text scan stays, for prose-authored corpora
//! and for the markers that legitimately live in a sentence.
//!
//! **The ontology declares it and no key name is blessed.** Matching a bare `open` under any
//! key would read `status: open` on a ballot measure as an open claim, which is a false
//! positive a corpus cannot opt out of. A declared field has none: the corpus said which
//! field it is.

/// How much of a node is measured against how much is supposed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClaimCounts {
    pub verified: usize,
    pub inference: usize,
    pub open: usize,
}

impl ClaimCounts {
    pub fn total(&self) -> usize {
        self.verified + self.inference + self.open
    }

    pub fn add(&mut self, other: ClaimCounts) {
        self.verified += other.verified;
        self.inference += other.inference;
        self.open += other.open;
    }

    /// Compact rendering for an index cell: `12v / 3i / 1o`, or `—` when untagged.
    pub fn cell(&self) -> String {
        if self.total() == 0 {
            return "—".to_string();
        }
        format!("{}v / {}i / {}o", self.verified, self.inference, self.open)
    }
}

/// The tokens defined by `prelude/guidelines/agent-conduct.md`.
pub const VERIFIED: &str = "[verified]";
pub const INFERENCE: &str = "[inference]";
pub const OPEN: &str = "[open]";

/// The ontology property type that marks a field as carrying an evidence tag.
pub const CLAIM_PROPERTY_TYPE: &str = "claim";

/// The three tags, their bare spellings, and what each means.
///
/// **The prose stays.** `prelude/guidelines/agent-conduct.md` carries the *reasoning* —
/// which is what makes the vocabulary arguable and revisable — and this carries the
/// *content*, which is what makes it cheap for an agent to obey without holding a document
/// in context. Neither is a substitute for the other.
///
/// The glosses are the one piece of prose duplicated from that file, so they are pinned to
/// it by test rather than trusted: `agent_conduct_defines_exactly_these_tags` asserts the
/// document names these three and no fourth. Parsing it at runtime was the alternative and
/// is worse — it would make a tool's answer depend on a vendored file that may be absent.
pub const TAG_MEANINGS: [(&str, &str, &str); 3] = [
    (
        "verified",
        VERIFIED,
        "supported by a committed primary source linked from this node or its catalog entry",
    ),
    (
        "inference",
        INFERENCE,
        "a reasonable conclusion drawn from verified facts; not directly witnessed",
    ),
    (
        "open",
        OPEN,
        "a live question; the answer is unknown, contested, or under investigation",
    ),
];

/// Which properties of each class carry an evidence tag.
///
/// Built once from the `.ont.yml` files and passed to the counters, because the answer is a
/// property of the class rather than of the file being read.
#[derive(Debug, Default, Clone)]
pub struct ClaimFields(std::collections::BTreeMap<String, Vec<String>>);

#[derive(Default, serde::Deserialize)]
struct OntologyProperties {
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    properties: Vec<OntologyProperty>,
}

#[derive(serde::Deserialize)]
struct OntologyProperty {
    name: String,
    #[serde(default)]
    r#type: String,
}

/// What one class definition declares: the name it calls itself, and its claim-typed
/// properties.
///
/// **One parser, two readers.** [`ClaimFields::load`] reads the ontology on disk;
/// [`crate::cmd::lint::history`] reads it from a blob at a past commit, where there is no
/// file to walk. A second copy of this parse is how a corpus and its own history come to
/// disagree about which fields carry a tag — and the disagreement would surface as a
/// question whose age is measured against an ontology that never applied to it.
///
/// The class name is `None` when the file does not declare one; the caller supplies the
/// file stem, which is the only thing it can fall back to and which the blob reader does not
/// have a path for.
pub(crate) fn declared_claim_fields(text: &str) -> (Option<String>, Vec<String>) {
    let ont: OntologyProperties = serde_yaml::from_str(text).unwrap_or_default();
    let fields = ont
        .properties
        .into_iter()
        .filter(|p| p.r#type == CLAIM_PROPERTY_TYPE)
        .map(|p| p.name)
        .collect();
    (ont.class, fields)
}

impl ClaimFields {
    /// Read every class definition in a corpus.
    pub fn load(corpus: &std::path::Path) -> Self {
        let mut map = std::collections::BTreeMap::new();
        for path in crate::walk::walk_ont_files(corpus) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (declared, fields) = declared_claim_fields(&text);
            let class = declared.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".ont.yml"))
                    .unwrap_or_default()
                    .to_string()
            });
            if !fields.is_empty() {
                map.insert(class, fields);
            }
        }
        Self(map)
    }

    /// The same declaration, read from an ontology already parsed.
    ///
    /// [`Self::load`] walks `.ont.yml` from disk. A caller holding the classes already —
    /// `query::Graph` does — would otherwise read every one of them a second time, and a
    /// graph reconstructed at a past commit holds an ontology that is not on disk at all,
    /// where the second read would answer with today's declaration about another year's
    /// corpus.
    ///
    /// **Key by whatever the caller will look a class up by.** `load` keys by the file's
    /// `class:` field where it has one; the gate keys by the `.ont.yml` stem, which is also
    /// the directory an instance must live in. Where the two disagree the stem is the one a
    /// node's class resolves to, so a caller passing stems gets the answer the gate would.
    pub fn from_declarations(
        declarations: impl IntoIterator<Item = (String, Vec<String>)>,
    ) -> Self {
        Self(
            declarations
                .into_iter()
                .filter(|(_, fields)| !fields.is_empty())
                .collect(),
        )
    }

    pub fn for_class(&self, class: &str) -> &[String] {
        self.0.get(class).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// The tag a declared field's value carries, if any.
///
/// Both spellings are read: `open` is what a typed vocabulary stores, and `[open]` is what a
/// corpus writes after being told the scan needs brackets. Accepting both means nobody has
/// to reshape their data a second time when this lands.
pub fn tag_of(value: &str) -> Option<&'static str> {
    match value.trim().trim_matches(|c| c == '[' || c == ']') {
        "verified" => Some(VERIFIED),
        "inference" => Some(INFERENCE),
        "open" => Some(OPEN),
        _ => None,
    }
}

/// Every value in the document held under one of `fields`, at any depth.
///
/// Depth-first over the whole document rather than only `properties`, because a projected
/// corpus may put its typed fields anywhere and the key name was declared by the ontology —
/// there is no false positive to guard against once the corpus has named the field.
fn structural_values(value: &serde_yaml::Value, fields: &[String], out: &mut Vec<String>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    if fields.iter().any(|f| f == key) {
                        match v {
                            serde_yaml::Value::String(s) => out.push(s.clone()),
                            // A list of tags is one claim each — a node carrying two is
                            // making two statements about itself.
                            serde_yaml::Value::Sequence(items) => out.extend(
                                items.iter().filter_map(|i| i.as_str().map(str::to_string)),
                            ),
                            _ => {}
                        }
                        continue;
                    }
                }
                structural_values(v, fields, out);
            }
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                structural_values(item, fields, out);
            }
        }
        _ => {}
    }
}

/// Markers a node declares structurally, in fields its class named.
pub fn count_structural(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    if fields.is_empty() {
        return counts;
    }
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return counts;
    };
    let mut values = Vec::new();
    structural_values(&doc, fields, &mut values);
    for v in values {
        match tag_of(&v) {
            Some(VERIFIED) => counts.verified += 1,
            Some(INFERENCE) => counts.inference += 1,
            Some(OPEN) => counts.open += 1,
            _ => {}
        }
    }
    counts
}

// ── serving claims, not counting them ─────────────────────────────────────────
//
// The counter answers *how many*. An agent needs *which*, with the standing attached, and
// that is a different return type over the same discrimination. Everything below reuses
// `is_narrated`, `mask_fenced` and `tag_of` rather than re-deriving them: a second reading
// of what counts as a claim is how a corpus comes to be described two ways at once, and
// this module exists because that already happened once.
//
// The invariant is asserted rather than assumed — see `served_claims_match_the_count`.

/// Where a served claim was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimScope {
    /// A tagged statement in the node's prose or in a property value.
    Statement,
    /// A property the class declared `type: claim`, whose value is the tag itself. The
    /// standing is the *node's*, not one sentence's.
    Node,
}

/// One assertion a node makes, with the standing it makes it at.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ServedClaim {
    /// The statement, tag removed. For a node-level standing, the property that carries it.
    pub text: String,
    /// `verified`, `inference`, or `open` — never inferred, only read.
    pub standing: &'static str,
    pub scope: ClaimScope,
    /// The declared `type: claim` property this came from, when it came from one.
    pub property: Option<String>,
    /// Byte offset of the marker, so two passes can tell one occurrence from another.
    /// Not part of the served contract; used to dedupe.
    #[serde(skip)]
    at: usize,
}

/// The sentence a marker sits in, with the marker removed.
///
/// Two shapes occur and both are in the corpus. A tag usually *follows* a completed
/// sentence — `"…converted by a rating curve. [verified]"` — and sometimes sits inside one:
/// `"Whether the money and the vote are connected is `[open]`"`. Reading only the first
/// would truncate every mid-sentence claim to its opening clause.
fn statement_around(text: &str, start: usize, end: usize) -> String {
    let before = text[..start].trim_end();
    // A terminator immediately before the marker belongs to the claim's own sentence, so the
    // search for where that sentence began has to start behind it.
    let terminated = before.ends_with(['.', '!', '?']);
    let head_end = if terminated {
        before.len() - 1
    } else {
        before.len()
    };
    let head_start = text[..head_end]
        .rfind(['.', '!', '?', '\n'])
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut out = text[head_start..before.len()].trim().to_string();
    if !terminated {
        let after = &text[end..];
        let stop = after
            .find(['.', '!', '?', '\n'])
            .map(|i| i + 1)
            .unwrap_or(after.len());
        out.push_str(&after[..stop]);
    }
    // Backticks that wrapped the marker are left behind by removing it from the middle.
    let out = out.trim().trim_end_matches('`').trim();
    // A claim written on the same line as its YAML key — `description: Finding a node. [open]`
    // — starts at the line boundary, so the key comes with it. The key is not part of what
    // the corpus asserted. Stripped only for a real key: one token, no spaces, then `: `.
    let key_len = out
        .find(": ")
        .filter(|i| !out[..*i].contains(char::is_whitespace) && *i > 0);
    match key_len {
        Some(i) => out[i + 2..].trim().to_string(),
        None => out.to_string(),
    }
}

/// Every asserted marker in `text`, as a claim.
///
/// Masked exactly as the counter masks — fenced blocks only. Inline code is **not** a
/// mention signal: 80% of a mature corpus's `[open]` tags are backticked and are claims, and
/// treating backticks as narration understated that repository's open questions fivefold.
fn prose_claims(text: &str) -> Vec<ServedClaim> {
    let masked = crate::markdown::mask_fenced(text);
    let mut out = Vec::new();
    for (tag, standing) in [
        (VERIFIED, "verified"),
        (INFERENCE, "inference"),
        (OPEN, "open"),
    ] {
        for (i, _) in masked.match_indices(tag) {
            if is_narrated(&masked, i, i + tag.len()) {
                continue;
            }
            out.push(ServedClaim {
                text: statement_around(&masked, i, i + tag.len()),
                standing,
                scope: ClaimScope::Statement,
                property: None,
                at: i,
            });
        }
    }
    out.sort_by_key(|c| c.at);
    out
}

/// The standings a node declares structurally, in properties its class typed `claim`.
///
/// A value that is *already bracketed* is visible to the prose pass too. The counter
/// subtracts a tally to correct for that; a list cannot, because it has to know *which*
/// occurrence is the duplicate — so this pass reports the byte offset of the value and the
/// caller drops the prose claim that sits inside it.
fn structural_claims(text: &str, fields: &[String]) -> Vec<ServedClaim> {
    if fields.is_empty() {
        return Vec::new();
    }
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return Vec::new();
    };
    let mut named: Vec<(String, String)> = Vec::new();
    named_values(&doc, fields, &mut named);

    named
        .into_iter()
        .filter_map(|(field, value)| {
            let standing = match tag_of(&value)? {
                VERIFIED => "verified",
                INFERENCE => "inference",
                OPEN => "open",
                _ => return None,
            };
            // Where the value sits in the raw bytes, so a bracketed one can be matched
            // against the prose pass's finding. `find` is enough: the value is a short
            // scalar and a false match would have to be the same string in the same file,
            // which dedupes to the same claim either way.
            let at = text.find(value.trim()).unwrap_or(0);
            Some(ServedClaim {
                text: field.clone(),
                standing,
                scope: ClaimScope::Node,
                property: Some(field),
                at,
            })
        })
        .collect()
}

/// `(field, value)` for every value held under one of `fields`, at any depth.
///
/// The same walk `structural_values` performs, keeping the key so a served claim can say
/// which property gave it its standing.
fn named_values(value: &serde_yaml::Value, fields: &[String], out: &mut Vec<(String, String)>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    if fields.iter().any(|f| f == key) {
                        match v {
                            serde_yaml::Value::String(s) => out.push((key.to_string(), s.clone())),
                            serde_yaml::Value::Sequence(items) => out.extend(
                                items
                                    .iter()
                                    .filter_map(|i| i.as_str())
                                    .map(|s| (key.to_string(), s.to_string())),
                            ),
                            _ => {}
                        }
                        continue;
                    }
                }
                named_values(v, fields, out);
            }
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                named_values(item, fields, out);
            }
        }
        _ => {}
    }
}

/// Every claim a node makes, prose and structure both.
///
/// The list form of [`count_in_node`], and held to it: the two must agree tag for tag, or
/// the corpus is described one way by `status` and another way by the agent surface.
pub fn claims_in_node(text: &str, fields: &[String]) -> Vec<ServedClaim> {
    let structural = structural_claims(text, fields);
    let mut prose = prose_claims(text);

    // Drop the prose sighting of a structural value that was written bracketed. Matched by
    // position rather than by count: `claim_tag: "[open]"` and a genuine `[open]` elsewhere
    // in the same node are two claims, and subtracting one from a tally cannot tell them
    // apart.
    for s in &structural {
        if let Some(i) = prose
            .iter()
            .position(|p| p.standing == s.standing && p.at >= s.at && p.at < s.at + 64)
        {
            prose.remove(i);
        }
    }

    let mut all = structural;
    all.extend(prose);
    all.sort_by_key(|c| c.at);
    all
}

/// Every marker in a node: prose and structure both.
///
/// A structural tag whose value already carries brackets is counted once, not twice — the
/// text scan would otherwise see the same tag in the serialized bytes. That is the case a
/// corpus lands in after reshaping its data to satisfy the old scan, which is most of the
/// corpora this feature exists for.
pub fn count_in_node(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = count_in_source(text);
    let structural = count_structural(text, fields);
    let bracketed = count_bracketed_structural(text, fields);
    counts.add(structural);
    // Subtract what both passes saw.
    counts.verified -= bracketed.verified.min(counts.verified);
    counts.inference -= bracketed.inference.min(counts.inference);
    counts.open -= bracketed.open.min(counts.open);
    counts
}

/// Structural values that are *also* visible to the text scan, i.e. already bracketed.
fn count_bracketed_structural(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    if fields.is_empty() {
        return counts;
    }
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return counts;
    };
    let mut values = Vec::new();
    structural_values(&doc, fields, &mut values);
    for v in values {
        let trimmed = v.trim();
        if !trimmed.starts_with('[') {
            continue;
        }
        match tag_of(trimmed) {
            Some(VERIFIED) => counts.verified += 1,
            Some(INFERENCE) => counts.inference += 1,
            Some(OPEN) => counts.open += 1,
            _ => {}
        }
    }
    counts
}

/// **The** open-question predicate.
///
/// One function, because it was three: inlined at three call sites in `cmd/` and again in
/// the MCP server, each spelling `label.starts_with('?') || text.contains("[open]")` for
/// itself. RFC-0006 names the copies, and a fourth is where the next divergence goes.
///
/// # Three arms, and the contract says so
///
/// The arms are frozen in `prelude/sdks/parity/mcp/tools.json` (`open_questions`), because
/// this function and a conforming MCP server elsewhere must answer identically over the same
/// corpus. When the structural arm landed here it did not land there: the contract still
/// said two and forbade a third by name, so `yidam open-questions` and a server that
/// implemented the contract *correctly* disagreed — which is the exact failure the freeze
/// exists to prevent, with the CLI as the one that moved. Reported from a repository holding
/// both.
///
/// A change to the arms is a change to that file and a bump of its `contract` version. It is
/// not a local decision, and the compiler will not tell you.
pub fn is_open_question(label: &str, text: &str, fields: &[String]) -> bool {
    label.trim_start().starts_with('?')
        // Masked, for the same reason the counter is: a node explaining what `[open]` means
        // is not thereby an open question.
        || count_tag(&crate::markdown::mask_fenced(text), OPEN) > 0
        || count_structural(text, fields).open > 0
}

/// Past-tense transitive reporting verbs. A tag that is the object of one is being reported,
/// not made: *"an earlier version said they were `[open]`"*.
///
/// **Past tense only, and no copulas.** Both cuts are measurements, not taste. `carries` and
/// `records` are the corpus *applying* a tag — "this node now carries `[open]`" asserts it —
/// and reading the present tense as narration produced three false positives. `was`, `were`,
/// `remains` and `held` produced five more, every one a live claim: *"Why the appointment was
/// made is `[open]`"*. All eight were in the promoting direction.
const REPORTING_VERBS: &[&str] = &[
    "carried", "said", "stated", "called", "wrote", "written", "tagged", "marked", "read",
];

/// Negations. The sentence denies the tag rather than applying it.
///
/// `no`, `nobody` and `nothing` are here for the same reason `written` is on the verb list:
/// English negates with more than the word `not`, and a rule that reads *"this claim is not
/// `[verified]`"* but not *"no claim here is `[verified]`"* has not implemented the arm it
/// claims to. They are completions of one shape, not extra shapes.
const NEGATIONS: &[&str] = &["not", "never", "no", "nobody", "nothing", "none", "neither"];

/// How far back a narrating construction may sit. Measured downstream at ~18 characters;
/// beyond that the verb belongs to a different clause.
const NARRATION_WINDOW: usize = 18;

/// The clause immediately before `start`, lowercased, at most [`NARRATION_WINDOW`] chars.
///
/// Stops at a clause boundary, which is what keeps *"does not assert either figure as the
/// reconciled one. `[open]`"* from reading as a negation: the `not` is in the sentence
/// before, and narration does not reach across a full stop.
fn preceding_clause(text: &str, start: usize) -> String {
    let head = &text[..start];
    let from = head
        .char_indices()
        .rev()
        .take(NARRATION_WINDOW)
        .last()
        .map_or(0, |(i, _)| i);
    let tail = &head[from..];
    match tail.rfind(['.', ';', ':', '\n', '!', '?', ',']) {
        Some(i) => tail[i + 1..].to_lowercase(),
        None => tail.to_lowercase(),
    }
}

fn has_word(hay: &str, word: &str) -> bool {
    hay.split(|c: char| !c.is_alphanumeric()).any(|w| w == word)
}

/// Metalinguistic nouns. A tag immediately followed by one is being named: *"the `[open]`
/// tag marks an unanswered claim"*, *"a `[verified]` claim about provenance"*.
///
/// A determiner *before* the tag was tried first and cut. It reads correctly on the corpus
/// that motivated this — seven occurrences, seven mentions — but only because that corpus
/// happens never to write *"A [verified] fact"*, which is an ordinary assertion the arm
/// killed. Being right on one corpus and wrong on plausible prose is how a word list starts
/// getting tuned; the noun is the part that actually carries the naming.
const NAMING_NOUNS: &[&str] = &[
    "tag", "tags", "claim", "claims", "marker", "markers", "token", "tokens",
];

/// Whether the word immediately after the tag names it rather than being described by it.
fn followed_by_naming_noun(after: &str) -> bool {
    let word: String = after
        .trim_start_matches('`')
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .collect();
    NAMING_NOUNS.contains(&word.to_lowercase().as_str())
}

/// Whether the tag occupying `start..end` is being named rather than asserted.
///
/// Grammar, not typography. The typographic rule this replaced — a tag in backticks is a
/// mention — was measured against a mature corpus and failed in the one direction the
/// evidence vocabulary exists to prevent: 80% of that corpus's `[open]` tags are backticked
/// and are claims, because an open question is usually written mid-sentence with the token
/// set off from the prose (*"Whether the money and the vote are connected is `[open]`"*),
/// while `[verified]` terminates a sentence of fact and reads fine bare. Applying it made a
/// repository understate its open questions fivefold, on its front page, with no diagnostic.
///
/// Four shapes and no others:
fn is_narrated(text: &str, start: usize, end: usize) -> bool {
    // 1. Pluralised — the tag is a noun being counted, not a tag being made.
    //    "Two of those three `[open]`s are closed."
    let after = text[end..].trim_start_matches('`');
    if after.starts_with('s') || after.starts_with("'s") || after.starts_with('\u{2019}') {
        return true;
    }

    // 2. Named by the noun that follows it — "the `[open]` tag", "a `[verified]` claim".
    if followed_by_naming_noun(after) {
        return true;
    }

    let clause = preceding_clause(text, start);

    // 3. Object of a past-tense reporting verb.
    if REPORTING_VERBS.iter().any(|v| has_word(&clause, v)) {
        return true;
    }

    // 4. Negated — the sentence denies the tag rather than applying it. "This claim is not
    //    `[verified]`" and "The three-fifths requirement is no longer `[open]`" are the two
    //    forms seen in a real corpus. This arm is not in the downstream rule; without it,
    //    dropping the typographic rule reintroduces the defect that motivated it, and does so
    //    on `[verified]`, where over-counting is the unsafe direction.
    NEGATIONS.iter().any(|n| has_word(&clause, n)) || clause.contains("no longer")
}

/// Occurrences of `tag` in `text` that are asserted rather than named.
fn count_tag(text: &str, tag: &str) -> usize {
    text.match_indices(tag)
        .filter(|(i, _)| !is_narrated(text, *i, i + tag.len()))
        .count()
}

fn tally(text: &str, counts: &mut ClaimCounts) {
    counts.verified += count_tag(text, VERIFIED);
    counts.inference += count_tag(text, INFERENCE);
    counts.open += count_tag(text, OPEN);
}

/// Count markers in a whole instance file.
///
/// Reads the raw YAML rather than the parsed struct so that markers in property values are
/// seen regardless of how the property is shaped — the corpus puts them in scalars, lists,
/// and nested maps, and a typed walk would have to anticipate each.
pub fn count_in_source(text: &str) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    tally(&crate::markdown::mask_fenced(text), &mut counts);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_marker_in_prose() {
        let c = count_in_source("A [verified] and a [inference] and an [open].");
        assert_eq!(c.verified, 1);
        assert_eq!(c.inference, 1);
        assert_eq!(c.open, 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn counts_markers_in_property_values() {
        // The single most load-bearing open claim in a real corpus lives here, not in prose.
        let yaml =
            "description: no markers here\nproperties:\n  estimate: \"[open] — not computed\"\n";
        assert_eq!(count_in_source(yaml).open, 1);
    }

    #[test]
    fn a_markdown_link_is_not_an_open_claim() {
        let c = count_in_source("see [open questions](../q/index.md) for more");
        assert_eq!(c.open, 0, "matching must be exact-token, not substring");
    }

    /// The reported case: a node that names the vocabulary rather than using it.
    ///
    /// `yidam status` published 1 verified claim against a true 0 for four commits on a
    /// sentence shaped like this one. The negation arm is what keeps it fixed — this is a
    /// denial, not a plural, a naming noun, or a reporting verb, so dropping the typographic
    /// rule without it would have reinstated the defect on `[verified]`, where over-counting
    /// is the unsafe direction.
    #[test]
    fn a_negated_token_is_a_mention_and_is_not_counted() {
        let c = count_in_source("This claim is not `[verified]`; nobody has checked it.");
        assert_eq!(c.verified, 0, "a denial is not an assertion");
        assert_eq!(c.total(), 0);
    }

    /// Backticks decide nothing. This is the whole change: the corpus that reported the
    /// defect writes 80% of its `[open]` claims in inline code, because an open question is
    /// written mid-sentence with the token set off from the prose around it.
    #[test]
    fn a_backticked_tag_is_counted_when_it_is_asserted() {
        let c = count_in_source("Whether the money and the vote are connected is `[open]`.");
        assert_eq!(c.open, 1, "typography is not the signal");
    }

    /// Narration: the tag is the object of a past-tense reporting verb.
    #[test]
    fn a_reported_tag_is_not_counted() {
        assert_eq!(
            count_in_source("An earlier version said they were `[open]` in the same breath.").open,
            0
        );
    }

    /// The present tense is the corpus *applying* a tag, not reporting one. Cut from the verb
    /// list by measurement downstream — three false positives, all promoting.
    #[test]
    fn the_present_tense_asserts_and_is_counted() {
        assert_eq!(count_in_source("This node now carries [open].").open, 1);
        assert_eq!(count_in_source("The node carried [open].").open, 0);
    }

    /// Copulas were cut for the same reason — five false positives, every one a live claim.
    #[test]
    fn a_copula_is_not_narration() {
        assert_eq!(
            count_in_source("Why the appointment was made is `[open]`.").open,
            1,
            "`was` earlier in the clause must not silence a live claim"
        );
    }

    /// Narration does not reach across a full stop, which is what keeps a negation in the
    /// previous sentence from silencing the next one's claim.
    #[test]
    fn narration_stops_at_a_clause_boundary() {
        let c = count_in_source("This corpus does not assert either figure. `[open]`");
        assert_eq!(c.open, 1, "the `not` belongs to the sentence before");
    }

    /// A tag being counted as a noun is a tag being discussed.
    #[test]
    fn a_pluralised_tag_is_not_counted() {
        assert_eq!(
            count_in_source("Two of those three `[open]`s are now closed.").open,
            0
        );
    }

    /// Both halves, in one sentence, because a corpus that discusses its own vocabulary
    /// writes both — and silencing the wrong one is how a fix becomes the next defect.
    #[test]
    fn a_mention_and_a_use_on_one_line_are_told_apart() {
        let c = count_in_source("The `[open]` tag is named here. This one is settled. [verified]");
        assert_eq!((c.verified, c.open), (1, 0));
    }

    /// A mention no arm reaches, recorded rather than legislated for.
    ///
    /// *"Unlike `[open]`, …"* names the tag and is counted as making it. Adding a
    /// comparative-preposition arm would fix this sentence and nothing else — it appears
    /// nowhere in any corpus measured, and a rule extended to satisfy a test it was not
    /// derived from is how a word list starts being tuned until the build goes green.
    ///
    /// It is left because the direction is right: an uncaught mention **over**-counts
    /// `[open]`, which overstates how much is unsettled. The failure this replaced ran the
    /// other way, and that is the one the vocabulary exists to prevent.
    #[test]
    fn an_uncaught_mention_fails_toward_caution() {
        let c = count_in_source("Unlike `[open]`, this one is settled.");
        assert_eq!(c.open, 1, "counted — and counting open is the safe error");
    }

    /// A fenced example is shown, not said.
    #[test]
    fn a_fenced_example_is_not_counted() {
        let yaml = "description: |\n  Write it like this:\n  ```\n  estimate: \"[open]\"\n  ```\n";
        assert_eq!(count_in_source(yaml).open, 0);
    }

    /// A node explaining the vocabulary is not thereby an instance of it.
    #[test]
    fn a_node_naming_open_in_code_is_not_an_open_question() {
        let text = "class: c\ndescription: The `[open]` tag marks an unanswered claim.\n";
        assert!(!is_open_question("Evidence tags", text, &[]));
        // And the used form still is.
        assert!(is_open_question(
            "Evidence tags",
            "description: Unresolved. [open]\n",
            &[]
        ));
    }

    #[test]
    fn repeated_markers_all_count() {
        let c = count_in_source("[verified] one. [verified] two.");
        assert_eq!(c.verified, 2);
    }

    #[test]
    fn an_untagged_node_renders_as_a_dash() {
        assert_eq!(ClaimCounts::default().cell(), "—");
    }

    #[test]
    fn a_tagged_node_renders_compactly() {
        let c = ClaimCounts {
            verified: 12,
            inference: 3,
            open: 1,
        };
        assert_eq!(c.cell(), "12v / 3i / 1o");
    }

    #[test]
    fn source_counting_sees_markers_anywhere_in_the_file() {
        let yaml = "class: c\ndescription: |\n  A [verified] fact.\nproperties:\n  estimate: \"[open] — not computed\"\n";
        let c = count_in_source(yaml);
        assert_eq!(c.verified, 1);
        assert_eq!(c.open, 1);
    }

    #[test]
    fn totals_accumulate() {
        let mut a = ClaimCounts::default();
        a.add(count_in_source("[verified]"));
        a.add(count_in_source("[open] [open]"));
        assert_eq!(a.verified, 1);
        assert_eq!(a.open, 2);
    }

    // ── serving claims ────────────────────────────────────────────────────────

    /// **The invariant.** The list and the tally are one extraction seen two ways, and a
    /// corpus described one way by `status` and another by the agent surface is the exact
    /// failure this module was written to end.
    ///
    /// Asserted per tag, not on the total: two errors in opposite directions cancel in a
    /// sum, and cancelling is how a disagreement survives a test.
    fn assert_served_matches_counted(text: &str, fields: &[String]) {
        let served = claims_in_node(text, fields);
        let counted = count_in_node(text, fields);
        for (standing, n) in [
            ("verified", counted.verified),
            ("inference", counted.inference),
            ("open", counted.open),
        ] {
            assert_eq!(
                served.iter().filter(|c| c.standing == standing).count(),
                n,
                "{standing}: served {served:#?} against count {counted:?} for:\n{text}"
            );
        }
    }

    #[test]
    fn served_claims_match_the_count() {
        let fields = vec!["claim_tag".to_string()];
        for text in [
            "description: |\n  A fact. [verified]\n  A guess. [inference]\n  A question. [open]\n",
            "description: plain prose with no markers\n",
            "description: |\n  Whether it holds is `[open]` — nobody has checked.\n",
            // Structural, bare: one claim, from the property.
            "class: gage\nproperties:\n  claim_tag: inference\n",
            // Structural, bracketed: seen by both passes, and still ONE claim.
            "class: gage\nproperties:\n  claim_tag: \"[open]\"\n",
            // Both arms at once.
            "class: gage\ndescription: |\n  A fact. [verified]\nproperties:\n  claim_tag: open\n",
            // A node discussing the vocabulary rather than using it.
            "description: |\n  The `[open]` tag marks an unanswered claim.\n  This claim is not [verified]\n",
            // A fenced block is masked in both.
            "description: |\n  ```\n  [verified]\n  ```\n  A real one. [verified]\n",
        ] {
            assert_served_matches_counted(text, &fields);
        }
    }

    /// The text is the statement, not the tag and not the whole node.
    #[test]
    fn a_served_claim_carries_the_statement_it_tags() {
        let c = claims_in_node(
            "description: |\n  Stage is converted by a rating curve. [verified]\n",
            &[],
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].standing, "verified");
        assert_eq!(c[0].text, "Stage is converted by a rating curve.");
    }

    /// A tag written mid-sentence carries the whole sentence, not the half before it.
    #[test]
    fn a_mid_sentence_tag_keeps_the_rest_of_the_sentence() {
        let c = claims_in_node(
            "description: |\n  Whether the two are connected is `[open]` and nobody has checked.\n",
            &[],
        );
        assert_eq!(c.len(), 1, "{c:#?}");
        assert!(
            c[0].text.contains("nobody has checked"),
            "truncated at the tag: {:?}",
            c[0].text
        );
    }

    /// Narration is not assertion, and the four shapes are the counter's — not a fifth
    /// invented to make a case pass.
    #[test]
    fn a_named_tag_is_not_served_as_a_claim() {
        for text in [
            "description: The `[open]` tag marks an unanswered claim.\n",
            "description: An earlier version said they were [open]\n",
            "description: This claim is not [verified]\n",
        ] {
            assert!(
                claims_in_node(text, &[]).is_empty(),
                "narrated tag served as a claim: {text}"
            );
        }
    }

    /// **Backticks are not a mention signal.** Measured: 80% of a mature corpus's `[open]`
    /// tags are backticked and are claims. A tool that filtered them would pass its own
    /// tests and understate that corpus fivefold.
    #[test]
    fn a_backticked_tag_is_still_a_claim() {
        let c = claims_in_node("description: The reach is regulated. `[verified]`\n", &[]);
        assert_eq!(c.len(), 1, "{c:#?}");
        assert_eq!(c[0].standing, "verified");
    }

    /// A declared `type: claim` property gives the NODE its standing, and says so.
    #[test]
    fn a_structural_tag_is_scoped_to_the_node() {
        let c = claims_in_node(
            "class: gage\nproperties:\n  claim_tag: inference\n",
            &["claim_tag".to_string()],
        );
        assert_eq!(c.len(), 1, "{c:#?}");
        assert_eq!(c[0].scope, ClaimScope::Node);
        assert_eq!(c[0].property.as_deref(), Some("claim_tag"));
        assert_eq!(c[0].standing, "inference");
    }

    /// Only a property the class DECLARED as a claim field counts. A bare `open` under an
    /// undeclared key is a word.
    #[test]
    fn an_undeclared_property_holding_a_bare_token_is_not_a_claim() {
        assert!(claims_in_node("properties:\n  status: open\n", &[]).is_empty());
    }

    /// Both spellings, because the counter accepts both.
    #[test]
    fn both_spellings_of_a_structural_tag_are_served_once() {
        for value in ["open", "\"[open]\""] {
            let c = claims_in_node(
                &format!("class: g\nproperties:\n  claim_tag: {value}\n"),
                &["claim_tag".to_string()],
            );
            assert_eq!(c.len(), 1, "{value}: {c:#?}");
            assert_eq!(c[0].standing, "open");
        }
    }

    /// There is no untagged arm. An unmarked sentence is prose, and prose is `get_node`'s
    /// job — inventing a fourth standing for it would turn every aside in the corpus into a
    /// weakly-evidenced claim.
    #[test]
    fn untagged_prose_is_not_served_as_a_claim() {
        assert!(claims_in_node(
            "description: |\n  Randomization eliminates confounding by design.\n",
            &[]
        )
        .is_empty());
    }

    /// The glosses `claim_tags` serves are copied from the guidelines, so they are pinned
    /// to them. Not the wording — that would make the document unrevisable — but the SET:
    /// this asserts the guidelines define exactly these three tokens and no fourth, which
    /// is the fact a tool serving them would otherwise silently outlive.
    #[test]
    fn agent_conduct_defines_exactly_these_tags() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../prelude/guidelines/agent-conduct.md");
        let text = std::fs::read_to_string(&path).expect("agent-conduct.md");

        // The definition list: a bullet opening with the bracketed token and an em dash.
        let defined: Vec<String> = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("- `["))
            .filter(|r| r.contains("]` — "))
            .filter_map(|r| r.split("]`").next().map(str::to_string))
            .collect();

        assert_eq!(
            defined,
            TAG_MEANINGS.iter().map(|(b, _, _)| *b).collect::<Vec<_>>(),
            "the guidelines and the served tags have come apart: {defined:?}"
        );
    }
}

#[cfg(test)]
mod structural_tests {
    use super::*;
    use tempfile::TempDir;

    fn corpus_with(ont: &str) -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("lead")).unwrap();
        std::fs::write(corpus.join("lead.ont.yml"), ont).unwrap();
        (tmp, corpus)
    }

    const TYPED: &str = "class: lead\nlabel: Lead\nproperties:\n  \
                         - name: claim_tag\n    type: claim\n    description: The tag.\n";

    /// The reported case, reproduced: a node whose tag is a structured value.
    ///
    /// `open-questions` returned 2 of 26 against a corpus shaped like this, because the
    /// bracketed token never appears in the bytes.
    #[test]
    fn a_structured_tag_is_seen() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        assert_eq!(fields.for_class("lead"), ["claim_tag"]);

        let node = "class: lead\nlabel: A lead\nproperties:\n  claim_tag: open\n";
        assert_eq!(count_in_source(node).open, 0, "the text scan sees nothing");
        assert!(is_open_question("A lead", node, fields.for_class("lead")));
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 1);
    }

    /// The workaround a consumer adopted to satisfy the old scan must not now count twice.
    ///
    /// `claim_tag: "[open]"` is visible to both passes. A corpus that reshaped its data to be
    /// seen at all should not be penalised for it the moment the fix lands.
    #[test]
    fn a_bracketed_structured_tag_counts_once() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: A lead\nproperties:\n  claim_tag: \"[open]\"\n";
        assert_eq!(count_in_source(node).open, 1, "the text scan sees it");
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 1);
    }

    /// Prose and structure both, on one node, are two claims.
    #[test]
    fn prose_and_structure_are_separate_claims() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: A lead\ndescription: Unresolved. [open]\n\
                    properties:\n  claim_tag: open\n";
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 2);
    }

    /// The reason the ontology declares *which* field, rather than any bare `open` counting.
    ///
    /// A ballot measure with `status: open` is not an open claim, and a corpus cannot opt out
    /// of a false positive it never asked for.
    ///
    /// The class here declares **both** kinds of property, which is the version that bites: a
    /// first draft of this test used a class declaring nothing, so removing the `type: claim`
    /// filter entirely still passed it. A guarantee about which fields are read cannot be
    /// tested by a corpus with no fields.
    #[test]
    fn a_declared_non_claim_field_holding_the_word_open_is_not_a_claim() {
        let (_t, corpus) = corpus_with(
            "class: lead\nlabel: Lead\nproperties:\n  \
             - name: status\n    type: string\n    description: Procedural state.\n  \
             - name: claim_tag\n    type: claim\n    description: The tag.\n",
        );
        let fields = ClaimFields::load(&corpus);
        assert_eq!(
            fields.for_class("lead"),
            ["claim_tag"],
            "only the claim-typed one"
        );

        let node = "class: lead\nlabel: A measure\nproperties:\n  status: open\n";
        assert!(!is_open_question(
            "A measure",
            node,
            fields.for_class("lead")
        ));
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 0);
    }

    /// A corpus declaring no claim field at all reads exactly as it did before.
    #[test]
    fn a_class_declaring_no_claim_field_contributes_none() {
        let (_t, corpus) = corpus_with("class: lead\nlabel: Lead\n");
        // Zero declared fields is the common case, and means this reads exactly as it did
        // before — which is what makes the feature additive rather than a migration.
        assert!(ClaimFields::load(&corpus).for_class("lead").is_empty());
    }

    /// A corpus that declares nothing behaves exactly as it did. That is what makes this
    /// additive rather than a migration.
    #[test]
    fn a_prose_corpus_is_unaffected() {
        let node = "class: lead\nlabel: A lead\ndescription: Unresolved. [open]\n";
        assert!(is_open_question("A lead", node, &[]));
        assert_eq!(count_in_node(node, &[]), count_in_source(node));
        assert!(is_open_question("? A question", "class: lead\n", &[]));
    }

    /// Both spellings, so nobody reshapes their data a second time.
    #[test]
    fn either_spelling_of_the_value_is_read() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        for value in ["open", "\"[open]\"", " open "] {
            let node = format!("class: lead\nlabel: L\nproperties:\n  claim_tag: {value}\n");
            assert!(
                is_open_question("L", &node, f.for_class("lead")),
                "{value:?}"
            );
        }
    }

    /// A node making two statements about itself is two claims.
    #[test]
    fn a_list_of_tags_is_one_claim_each() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: L\nproperties:\n  claim_tag:\n    - verified\n    - open\n";
        let counts = count_in_node(node, f.for_class("lead"));
        assert_eq!((counts.verified, counts.open), (1, 1));
    }

    /// The field is found wherever the corpus put it. The key name was declared, so depth
    /// costs nothing — and a projected corpus does not have to flatten itself first.
    #[test]
    fn a_nested_declared_field_is_found() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: L\nproperties:\n  evidence:\n    - claim_tag: open\n";
        assert!(is_open_question("L", node, f.for_class("lead")));
    }

    /// The class name comes from the filename when the field is absent, matching every other
    /// reader of an `.ont.yml`.
    #[test]
    fn a_class_is_named_by_its_file_when_it_does_not_say() {
        let (_t, corpus) = corpus_with(
            "label: Lead\nproperties:\n  - name: claim_tag\n    type: claim\n    description: x\n",
        );
        assert_eq!(ClaimFields::load(&corpus).for_class("lead"), ["claim_tag"]);
    }
}
