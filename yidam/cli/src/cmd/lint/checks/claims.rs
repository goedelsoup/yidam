use super::*;

/// One corpus file's prose and where it lives — all [`claim_tag_malformed`] reads.
///
/// A view rather than `&[Node]` for the reason [`crate::corpus::EdgeView`] is one: the derivation has two
/// callers holding different things. A malformed evidence tag is a defect of *prose*, and a
/// class file's prose is prose a reader treats exactly as an instance's — so the check has to
/// see both, and there is no shape either side already has that the other can produce.
///
/// **A [`Class`] is deliberately not made into a [`Node`].** Two derivations decide what class
/// a node belongs to, and a class file borrowed into the node list is read wrongly by both, in
/// two different directions. [`unknown_class`] reads `inst.class`, which on an `.ont.yml` is
/// the class's *own* name — so the file reads as an instance of itself and passes.
/// [`class_of`] reads the parent **directory**, which is `corpus` for
/// `.yidam/corpus/gage.ont.yml` — so `missing-property`, `property-type`, `node-too-long`,
/// `unlicensed-edge` and `edge-target-class` would all look the file up under a class no
/// ontology declares, find nothing, and skip it while appearing to have checked it. Sharing
/// the one field two checks need costs nothing; sharing the type re-answers every question
/// `&[Node]` is asked, and answers several of them silently.
pub struct ProseView<'a> {
    pub rel: &'a str,
    pub text: &'a str,
}

/// Every file whose prose may carry an evidence tag: the corpus's instances, then its classes.
///
/// Instances first, so widening the scope did not reorder the findings a report already had.
///
/// `universal.yml` is **not** here, and that is a scope decision rather than an omission. It
/// declares properties every class inherits; it is not a class, [`crate::corpus::load_classes`] does not
/// produce one for it, and no measured corpus writes prose in it at all. A file whose contents
/// are a property list has no argument in it to tag.
pub fn prose_views<'a>(nodes: &'a [Node], classes: &'a [Class]) -> Vec<ProseView<'a>> {
    nodes
        .iter()
        .map(|n| ProseView {
            rel: &n.rel,
            text: n.prose(),
        })
        .chain(classes.iter().map(|c| ProseView {
            rel: &c.rel,
            text: &c.text,
        }))
        .collect()
}

/// Separators that turn a tag into a tag-plus-something.
///
/// A plain space is not among them, deliberately. `[open questions]` is ordinary prose and
/// reference-style link text; flagging it would put a permanent finding in front of every
/// reader who writes the phrase, and a permanently non-empty report is where a real finding
/// gets lost. What is reported is a tag followed by punctuation that reads as *and here is
/// the rest of it*.
///
/// **This is no longer the only way in**, and the reason is measured. Punctuation admits a
/// detail cheaply, but the details it admits are overwhelmingly *citations* — across thirteen
/// derived corpora, of 1,024 findings the separator rule produced, only 46 narrowed the claim
/// rather than sourcing it. The narrowing details were mostly on the other side of the rule:
/// **59 of 105 were introduced by a plain space** and so were invisible, and they are the ones
/// the model cannot express at all — `[verified for the ratio; inference for the conclusion]`
/// is one tag carrying two standings for two spans of one sentence.
///
/// So [`crate::claims::narrowing_detail`] is a second admission path, keyed on the *content* rather than the
/// punctuation. It admits those 59 and none of the prose this list exists to protect: `open` in
/// `[open questions]` is followed by a word that is neither a narrowing head nor a second
/// standing, so it stays unreported, which is the whole point of the paragraph above.
/// The inner text of a reported near-miss form — `[verified — x]` → `verified — x`.
///
/// One `strip_prefix`/`strip_suffix` pair rather than `trim_matches`, which would eat the inner
/// bracket of `[verified for the composition; the basis effect is [open]]` and change the
/// answer.
fn reported_inner(found: &str) -> &str {
    found
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(found)
}

pub const CLAIM_TAG_MALFORMED: CheckSpec = CheckSpec {
    id: "claim-tag-malformed",
    title: "Evidence tag that counts as nothing",
    severity: Severity::Warn,
    rationale: "The three tokens are matched exactly, which is what keeps `[open questions](…)` \
         from reading as an open claim. The cost is that a tag with its citation folded \
         inside the brackets matches nothing and is silently counted as untagged — a node \
         that looks tagged to a reader and reads as bare assertion to every counter. This \
         reports the near miss rather than guessing at it. Brackets are matched by depth \
         over the block rather than to the first `]` on the line, because a citation that is \
         itself a link both nests a `]` and wraps: measured on one derived corpus, the line \
         scan found 234 of 445 such tags and none of the 179 that wrapped. The block is the \
         bound, so an unbalanced `[` in quoted source costs one bracket rather than every \
         tag below it.\n\n\
         Scope: every instance file and every class file, read as bytes — so a tag in a \
         class's `description`, in a `properties[]` or `edges[]` entry, or in its \
         `analytic_note` is reported the same way one in a node is. Measured over 17 corpora, \
         all four carry tags. Reading a class's parsed fields instead would miss two of the \
         four outright, because the property and edge structs declare no `description` at \
         all. `universal.yml` is not read: it is a property list, not an argument.\n\n\
         One check, two pieces of advice. A detail naming a source is told to put the \
         citation beside the tag. A detail that *narrows* the claim is not, because moving \
         `as proposed` out of the brackets leaves a bare `[verified]` asserting more than the \
         sentence does — the advice would make the corpus less accurate and the counter more \
         confident. Measured over thirteen derived corpora, ~89% of details name a source, so \
         the citation advice is the default and a detail diverts only on an unambiguous \
         narrowing tell: a second standing, or a narrowing head as its first word. That tell \
         also admits a space-introduced detail the separator list cannot see, which is where \
         59 of the 105 narrowing details in the population were sitting.",
};

/// Bracketed forms that open with an evidence tag and are not one.
///
/// The counter matches exact tokens, so `[verified — Pearl 2009]` matches nothing and is
/// counted as **no claim at all** — the opposite of the mention-counted-as-use failure and
/// just as silent. A reader who writes that has plainly intended a tag, and the count
/// absorbs the authoring error rather than reporting it. Counting it would mean guessing
/// what was meant; saying so does not.
///
/// Warn rather than Error: the node is still readable and the fix is the author's to make.
///
/// **Class files are in scope, and were not until #603.** The check took `&[Node]`, and a
/// `*.ont.yml` is not a node, so a class's own prose was never scanned — a boundary nobody had
/// decided and nobody could see. It was found by porting the matcher downstream: excluding
/// `*.ont.yml` was exactly what made the port reproduce lint's output, 234/234, and including
/// them reported two findings lint did not. Both were real, and both in one class explaining
/// why a property has to exist.
pub fn claim_tag_malformed(prose: &[ProseView<'_>]) -> Check {
    let mut violations = Vec::new();
    for n in prose {
        // Masked, so a file explaining the vocabulary is not reported for naming it — the
        // same reason the counter masks.
        // Spans, not just lines, because the suppression below has to ask the counter about the
        // *same tag* over the text the counter reads — and the two mask differently.
        for tag in crate::claims::detail_tags(&crate::markdown::mask_code(n.text)) {
            let (line, found) = (tag.line, format!("[{}]", tag.inner));
            let inner = reported_inner(&found);
            // The terminal form written wrongly (RFC-0061) is reported by its own pass below,
            // never here: an unknown reason is a scoped tag and neither form should be told to
            // move a citation out.
            if crate::claims::unanswerable_misuse(&tag.inner).is_some() {
                continue;
            }
            // A **scoped tag is a claim** and is not reported (#789). This check used to tell
            // the author of `[verified — as proposed]` that it counted as nothing, and the
            // advice it gave was that there was nothing to do about it — a finding whose own
            // remedy is "leave it" is not a finding. `parse_tag` now reads it, the counter
            // counts it, and the only thing left to say is nothing.
            //
            // The refusals stay reported, and they are the two this cannot fix: a citation
            // folded into the bracket (move it to `references:`) and two standings in one
            // bracket (RFC-0031's, not a qualifier's).
            if crate::claims::is_scoped_claim(n.text, tag.start, tag.end) {
                continue;
            }
            // One finding, two pieces of advice. Telling the author of a detail that narrows
            // to "put the citation beside it" asks them to delete a qualifier and leaves a
            // bare `[verified]` that overstates the claim — the advice makes the corpus less
            // accurate and the counter more confident.
            let message = if crate::claims::narrowing_detail(inner) {
                format!(
                    "`{found}` opens with an evidence tag and is not one, so it is counted \
                     as no claim at all — it asserts two things at two standings, which one \
                     qualifier cannot carry; write them as two claims"
                )
            } else {
                format!(
                    "`{found}` opens with an evidence tag and is not one, so it is counted \
                     as no claim at all — write the tag alone and put the citation beside it"
                )
            };
            violations.push(Violation::new(format!("{}:{}", n.rel, line), message));
        }
        // Over the text the counter reads, not the code-masked text above (RFC-0061): a
        // backticked `[open — unanswerable: no filing exists]` is counted as owed, so it is
        // named, and one the sentence only reports is counted as nothing, so it is not.
        for (tag, misuse) in crate::claims::unanswerable_misuses(n.text) {
            let found = format!("[{}]", tag.inner);
            let message = match misuse {
                crate::claims::UnanswerableMisuse::UnknownReason => format!(
                    "`{found}` gives a reason that is not one of {}, so it still counts as a \
                     plain `[open]` and stays owed — use one of the three, and put anything \
                     more beside the tag",
                    crate::claims::UNANSWERABLE_REASONS
                        .iter()
                        .map(|r| format!("`{r}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                crate::claims::UnanswerableMisuse::NotOpen => format!(
                    "`{found}` marks a claim unanswerable that is not open, so it is counted \
                     as no claim at all — only an `[open]` claim can be unanswerable"
                ),
            };
            violations.push(Violation::new(format!("{}:{}", n.rel, tag.line), message));
        }
    }
    Check::new(&CLAIM_TAG_MALFORMED, violations)
}

pub const CLASS_CLAIM_UNCOUNTED: CheckSpec = CheckSpec {
    id: "class-claim-uncounted",
    title: "Evidence tag asserted in class prose, which no counter reads",
    severity: Severity::Info,
    rationale:
        "A claim is \"a statement in a node\" — CONSTITUTION.md, Article V — and the counters \
         hold to that. A class file may still write one, because a class is where a corpus \
         argues for its own shape and that argument is evidenced like any other. This reports \
         the gap rather than closing it, and the reason it is not closed is measured: taking \
         the counter's own rule to class files finds 6 tags across 2 of the 4 corpora this \
         repository ships, and every one of the six is a class explaining its own vocabulary — \
         \"`[verified]` means somebody confirmed the change shipped and held\". They survive \
         the narration rule because it is past-tense-only and the word after every one of them \
         is \"means\". Telling those six from a real assertion is a judgement about wording, \
         which is the same reason Article V gives for refusing to delegate claim identity to a \
         checker at all. So the count is shown and not spent, and only what is asserted \
         outside backticks is shown at all. If a corpus finds real claims here, the fix is to \
         move them into a node — where they are counted, cited, and reachable by every other \
         check.",
};

/// Evidence tags in a class file's prose, which nothing counts.
///
/// **Reported, and does not gate** — the shape [`EdgePolicy::Unstated`] and `policy-override`
/// already have, and for the same reason: the state is worth seeing and there is no defect to
/// fix. A class arguing for its own shape in tagged prose is doing something legitimate.
///
/// It exists because the boundary was invisible from inside a derived repository. `yidam
/// status` counts claims in nodes and reads no class file, which is a decision (see
/// [`crate::claims::count_in_source`]) rather than an accident — but nothing said so, and an
/// author moving an argument out of a node and into the class it belongs to would watch its
/// claims silently stop being counted. #603 was filed by somebody who could only find this
/// out by porting the matcher and noticing which files had to be excluded for the numbers to
/// agree.
///
/// # What is counted, and the one place it diverges from the counter
///
/// The grammar is [`crate::claims::count_in_source`]'s, so the number here and the number a
/// widened counter would add come from one rule and move together. The text handed to it is
/// masked by [`crate::markdown::mask_code`] rather than `mask_fenced`, which is the divergence
/// and the only one — a tag inside backticks is not reported.
///
/// **That is measured, not inherited.** Over the 26 class files this repository tracks, the
/// counter's own rule finds 6 tags in 2 files and the span mask finds 0, and the 6 are the
/// same 6 the counter is not widened for: a class explaining its own vocabulary, every one of
/// them backticked. Reporting them would put a permanent Info row in front of every reader of
/// two teaching corpora, which is the state `TAG_SEPARATORS` refuses for the same vocabulary
/// one screen up — a permanently non-empty report is where a real finding gets lost.
///
/// It is the opposite call from the one [`crate::markdown::mask_fenced`] records for the
/// counter, and the difference is the subject rather than the rule. There, 80% of a mature
/// corpus's `[open]` tags are backticked *node* prose and are claims, so the typographic rule
/// under-counted a corpus's own front page fivefold. Here the subject is a class file, the
/// measured population is the other way round, and under-reporting costs an Info row rather
/// than a published number. Should a corpus turn up asserting backticked claims in class
/// prose, this is the line to revisit, and the measurement above is what to re-run.
///
/// **Never baselined**, by construction: [`super::super::baseline::Baseline::from_checks`] records
/// only what gates, and nothing here does. So this cannot be blessed into silence, which is
/// the point — it is the trigger for a later decision, not debt.
pub fn class_claim_uncounted(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let n = crate::claims::count_in_source(&crate::markdown::mask_code(&c.text)).total();
            (n > 0).then(|| {
                Violation::new(
                    &c.rel,
                    format!(
                        "class prose asserts {n} evidence tag(s). `yidam status` counts claims \
                         in nodes, so these are counted nowhere"
                    ),
                )
                .at(Severity::Info)
            })
        })
        .collect();
    Check::new(&CLASS_CLAIM_UNCOUNTED, violations)
}

pub const CLAIM_PROPERTY_UNDECLARED: CheckSpec = CheckSpec {
    id: "claim-property-undeclared",
    title: "Property spelling a standing that its class did not declare `type: claim`",
    severity: Severity::Info,
    rationale: "A claim in a node's field is counted when the class declares that field \
         `type: claim`, and is read by nothing when it does not — not by `status`, not by \
         `open-questions`, not by the `due` questions clock, not by the MCP `claims` \
         resource. The corpus wrote the standing; the declaration is what makes it \
         reachable. This reports bare spellings only: a bracketed `[open]` is already \
         counted wherever it sits, because the prose scan reads the file's bytes, and every \
         bracketed value measured in an undeclared field is a placeholder for a value the \
         corpus does not have yet rather than a claim. It is Info because the right repair \
         is not always the declaration: a `status` holding `open`, `narrowing` and \
         `answered` is a lifecycle and not an evidence standing, and declaring that one \
         `type: claim` would make the corpus assert something it did not. The finding names \
         both fixes and imposes neither.",
};

/// A property whose value spells a standing, in a class that did not declare it `type: claim`.
///
/// **The gap #1069 arrived through.** A derived corpus reported 56 open questions as nodes
/// against a `due` questions clock that saw 8, and the issue asked for the counter to match a
/// bare `open` under any key. The frozen `open_questions` contract refuses that by name, and for
/// a reason a fifth key in the same corpus demonstrates: `status: open` on a ballot measure is a
/// legislative stage, and a scan that read it as an unsettled claim would make every corpus in
/// the population assert things it did not. **The ontology declares the field or the arm reads
/// nothing** — so what was missing is not a wider scan. It is that a corpus which wrote a
/// standing into an undeclared field was told by nothing at all.
///
/// The finding is one declaration away from being counted, and says so:
/// [`crate::claims::undeclared_structural_tags`] is [`crate::claims::count_structural`]'s own
/// walk inverted, so every pair reported here is a claim the counters would pick up the moment
/// the class declared the field — in `status`, in `open-questions`, in the questions clock, in
/// the MCP `claims` resource, at once.
///
/// **Info, and not a gate, because the fix is not always the declaration.** Run by this binary
/// over the sixteen corpora in eighteen derived repositories — 2,770 tracked instance nodes —
/// it reports **18 findings in 3 corpora**, and those 18 are three `(class, property)` pairs
/// that split three ways. One is unambiguous: `bitrecover-bitwipe`'s `question` class carries
/// `claim_tag` on 11 nodes holding exactly `verified` (5), `inference` (5) and `open` (1), and
/// its `question.ont.yml` declares no such property — a claim field by name, by vocabulary and
/// by intent, undeclared, counted nowhere. One is a corpus that grades its own transcription:
/// `allen-recorder`'s `parcel.status` holds a bare `verified` on 3 nodes. And one is neither:
/// `hegeomai`'s `inquiry.status` holds `open` on 4 nodes and `narrowing` and `answered` on
/// others, which is a question's lifecycle and not an evidence standing. Declaring that one
/// `type: claim` would be wrong; renaming it is the repair. A check that gated could not tell
/// the three apart, and deciding which a word is remains the judgement Article V refuses to
/// delegate — so both fixes are named and neither is imposed.
///
/// Bracketed values are not reported; [`crate::claims::undeclared_structural_tags`] says why, and
/// the reason is that they are already counted.
pub fn claim_property_undeclared(nodes: &[Node], classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let class_name = class_of(n);
        let Some(class) = by_name.get(class_name.as_str()) else {
            continue;
        };
        // The counters' own list, entry paths included, so a `claim` field a `list`'s
        // record declares is skipped exactly where it is counted.
        let declared = crate::claims::declared_claim_fields(class);
        for (key, value, holder) in crate::claims::undeclared_structural_tags(&n.text, &declared) {
            let detail = match holder {
                // Nested under a property: the standing of an entry, which a top-level
                // declaration would misdescribe as the node's (#1382).
                Some(holder) => format!(
                    "`{key}: {}` under `{holder}` spells a standing, and `{}` does not declare \
                     it — so it is counted as no claim at all, and is not the node's. Declare \
                     `{holder}` as `type: list` with a `{key}` field of `type: claim`, so each \
                     entry carries its own standing, or rename the key if the word is a \
                     workflow state rather than a standing",
                    elide_value(&value),
                    class.rel
                ),
                None => format!(
                    "`{key}: {}` spells a standing, and `{}` does not declare `{key}` \
                     as `type: claim` — so it is counted as no claim at all. Declare it, \
                     or rename the property if the word is a workflow state rather than a \
                     standing",
                    elide_value(&value),
                    class.rel
                ),
            };
            violations.push(Violation::new(&n.rel, detail).at(Severity::Info));
        }
    }
    Check::new(&CLAIM_PROPERTY_UNDECLARED, violations)
}

/// A value shortened to its first line and 60 characters, so a finding stays one line.
///
/// A claim-spelling value can be a qualified tag running to a paragraph — `verified for the
/// counts and the two roll joins; …` — and a finding that reproduced one would push the
/// declaration it is asking for off the reader's screen.
fn elide_value(value: &str) -> String {
    let first = value.lines().next().unwrap_or_default().trim();
    match first.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &first[..i]),
        None => first.to_string(),
    }
}

/// `(line, text)` for each bracketed near miss, in the form `claim-tag-malformed` reports.
///
/// **A test helper now, and only that.** The check iterates
/// [`crate::claims::detail_tags`] directly, because it needs each tag's *span* to ask the counter
/// whether that tag is a claim. This keeps the shorter view the scan's own tests are written
/// against; the formatting is the check's, so the two cannot drift on what a finding looks like.
#[cfg(test)]
fn near_miss_tags(text: &str) -> Vec<(usize, String)> {
    crate::claims::detail_tags(text)
        .into_iter()
        .map(|t| (t.line, format!("[{}]", t.inner)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    /// The reported shape: a tag with its citation folded inside the brackets.
    #[test]
    fn a_tag_with_its_citation_inside_is_a_near_miss() {
        let found = near_miss_tags("The estimate is [verified — Pearl 2009].");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], (1, "[verified — Pearl 2009]".to_string()));
        // And it is counted as nothing, which is why it is worth reporting at all.
        assert_eq!(
            crate::claims::count_in_source("The estimate is [verified — Pearl 2009].").total(),
            0
        );
    }

    /// The shape that made 179 of 445 tags invisible: the citation is a link, so it is long,
    /// so the tag wraps and its `]` is never on the line that holds its `[`.
    #[test]
    fn a_tag_that_wraps_a_line_is_still_a_near_miss() {
        let text = "  A resolution stands. [verified —\n    \
                    [the audit reports](../catalog/auditor-district-audits.md)]\n";
        let found = near_miss_tags(text);
        assert_eq!(found.len(), 1, "a wrapped tag is one tag: {found:?}");
        assert_eq!(found[0].0, 1, "reported where the tag opens");
        assert_eq!(
            found[0].1, "[verified — [the audit reports](../catalog/auditor-district-audits.md)]",
            "the wrap is the file's, not the tag's, so the finding does not carry it"
        );
    }

    /// The link guard, tested against the outer `]` rather than the inner one. Single-line,
    /// and reported by nothing before this: `find(']')` returned the *link's* bracket, the
    /// next character was `(`, and the guard fired on the tag it exists to protect.
    #[test]
    fn a_tag_whose_citation_is_a_link_is_a_near_miss() {
        let found = near_miss_tags(
            "The share follows [verified — [`bridge-formula`](bridge-formula.yml)].",
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].1,
            "[verified — [`bridge-formula`](bridge-formula.yml)]"
        );
    }

    /// The bound is the point of the bound. An unbalanced `[` in quoted source — this one is
    /// verbatim from a derived corpus — must cost the check that bracket and nothing else; an
    /// unbounded depth scan swallows every tag below it and the finding count *falls* as the
    /// corpus grows.
    #[test]
    fn an_unbalanced_bracket_does_not_blind_the_rest_of_the_file() {
        let text = "properties:\n  as_written: \"against 37 on the [Cupp\"\n  \
                    note: A later count. [verified — Cupp 2019]\n";
        let found = near_miss_tags(text);
        assert_eq!(
            found.len(),
            1,
            "the tag below the stray `[` survives: {found:?}"
        );
        assert_eq!(found[0], (3, "[verified — Cupp 2019]".to_string()));
    }

    /// A tag may cross a soft wrap and may not cross into the next YAML key, list item or
    /// paragraph — the same boundary `statement_around` is measured against, for the same
    /// reason.
    ///
    /// Each case here is written so the *separator* is inside the unclosed bracket: without
    /// the bound the scan runs on to a `]` in the next block, folds two blocks into one
    /// "tag", and reports it. That is the false positive an unbounded depth scan buys, and
    /// it is why these are three cases and not one.
    #[test]
    fn the_scan_stops_at_the_block_it_started_in() {
        for (label, text) in [
            (
                "a new YAML key",
                "description: The rate rose. [verified —\nsource: ../s.md]\n",
            ),
            ("a new list item", "- first [verified —\n- second later]\n"),
            (
                "a blank line",
                "The rate rose. [verified —\n\nA later paragraph entirely]\n",
            ),
        ] {
            assert!(
                near_miss_tags(text).is_empty(),
                "{label} ends the block, so nothing here is one tag: {:?}",
                near_miss_tags(text)
            );
        }
    }

    #[test]
    fn several_separators_all_read_as_a_folded_citation() {
        for inner in [
            "[open: pending review]",
            "[inference - weak]",
            "[verified, 2026]",
            "[open — nobody has looked]",
            "[verified/partial]",
        ] {
            assert_eq!(
                near_miss_tags(inner).len(),
                1,
                "{inner} should read as a near miss"
            );
        }
    }

    /// The forms that must stay silent. A warn nobody can act on is worse than no warn.
    #[test]
    fn the_tag_itself_and_ordinary_prose_are_not_reported() {
        for quiet in [
            "[verified]",
            "[open]",
            "[inference]",
            // A plain space is not a separator: this is prose, and link text.
            "See [open questions] below.",
            "See [open questions](../q/index.md) for more.",
            "A reference [open questions][q] link.",
            "[opening the valve] is not a tag",
            "[verification] is a different word",
            "nothing bracketed at all",
        ] {
            assert!(
                near_miss_tags(quiet).is_empty(),
                "{quiet} must not be reported"
            );
        }
    }

    /// A node that names the malformed shape in order to warn about it is not committing it.
    #[test]
    fn a_masked_mention_is_not_a_near_miss() {
        let node = Node::parse(
            PathBuf::from("/tmp/x.yml"),
            "x.yml",
            "description: Never write `[verified — source]`; the counter reads it as \
             untagged.\n",
        );
        let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
        assert_eq!(c.violations.len(), 0, "{:?}", c.violations);
    }

    /// The violation names the line, because the point is to go and fix that line.
    #[test]
    fn a_near_miss_is_reported_against_its_line() {
        let node = Node::parse(
            PathBuf::from("/tmp/x.yml"),
            ".yidam/corpus/c/x.yml",
            "class: c\nlabel: X\ndescription: |\n  Settled [verified — Pearl 2009].\n",
        );
        let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].node.ends_with(":4"),
            "{}",
            c.violations[0].node
        );
    }

    /// The terminal form (RFC-0061): the valid one is a claim and not reported; the two wrong
    /// ones get their own advice, never the citation advice.
    #[test]
    fn the_unanswerable_form_is_read_and_its_misuses_are_named() {
        let node = Node::parse(
            PathBuf::from("/tmp/x.yml"),
            ".yidam/corpus/c/x.yml",
            "class: c\ndescription: |\n  Who paid is [open — unanswerable: not in the public record].\n",
        );
        let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
        assert_eq!(c.violations.len(), 0, "{:?}", c.violations);

        let unknown = advice_for("Who paid is [open — unanswerable: no filing exists].");
        assert!(
            unknown.contains("`not in the public record`, `out of scope`, `superseded`"),
            "{unknown}"
        );
        assert!(unknown.contains("plain `[open]`"), "{unknown}");
        assert!(!unknown.contains("citation beside it"), "{unknown}");

        let not_open = advice_for("It held [verified — unanswerable: superseded].");
        assert!(not_open.contains("only an `[open]` claim"), "{not_open}");
        assert!(!not_open.contains("citation beside it"), "{not_open}");

        // Backticked, as an open claim is usually written: still counted, so still named.
        let ticked = advice_for("Who paid is `[open — unanswerable: no filing exists]`.");
        assert!(ticked.contains("plain `[open]`"), "{ticked}");

        // Only reported, so counted as nothing and not named.
        let node = Node::parse(
            PathBuf::from("/tmp/x.yml"),
            ".yidam/corpus/c/x.yml",
            "class: c\ndescription: |\n  An earlier draft said [open — unanswerable: no filing exists].\n",
        );
        let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
        assert_eq!(c.violations.len(), 0, "{:?}", c.violations);
    }

    fn advice_for(body: &str) -> String {
        let node = Node::parse(
            PathBuf::from("/tmp/x.yml"),
            ".yidam/corpus/c/x.yml",
            format!("class: c\ndescription: |\n  {body}\n"),
        );
        let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
        assert_eq!(
            c.violations.len(),
            1,
            "expected one finding: {:?}",
            c.violations
        );
        c.violations[0].detail.clone()
    }

    /// The case the advice used to fork for, now not reported at all.
    ///
    /// `[verified as proposed]` says *what* is verified — that the corpus holds the text of a
    /// proposal, not evidence it was adopted. Told to "put the citation beside it", an author
    /// deletes the qualifier and leaves a bare `[verified]` that the counter records as the
    /// stronger claim. That was the reason for a second piece of advice; #789 went further and
    /// made the tag a **claim**, so there is nothing left to advise. A finding whose own remedy
    /// is "leave it" is not a finding.
    #[test]
    fn a_qualified_tag_is_not_reported_at_all() {
        for body in [
            "The amendment lands [verified as proposed].",
            "The row holds [verified — for the snapshot].",
            "Whether it is one of six is [open — which of the six, not whether].",
        ] {
            let node = Node::parse(
                PathBuf::from("/tmp/x.yml"),
                ".yidam/corpus/c/x.yml",
                format!("class: c\ndescription: |\n  {body}\n"),
            );
            let c = claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]));
            assert!(c.violations.is_empty(), "{body:?} → {:?}", c.violations);
            // …and the other half of the same change: it is counted, so it did not merely stop
            // being reported. These two live in different modules and have to move together.
            assert_eq!(crate::claims::count_in_source(body).total(), 1, "{body:?}");
        }
    }

    /// **Either counted or reported, never neither and never both.**
    ///
    /// The property that failed first. The check masks inline code and the counter does not, so
    /// ``[verified — `crates/x`, as introduced]`` read as a qualifier to one and a citation to
    /// the other: the check suppressed the finding, the counter declined the claim, and the tag
    /// went **silently missing** — three of them across two real corpora, which is the failure
    /// `claim-tag-malformed` exists to prevent. It was found by predicting the per-corpus deltas
    /// and measuring, not by a test, so this is the test.
    ///
    /// Measured on the six corpora that write these details, every one now satisfies
    /// Δfindings == −Δclaims: −29/+29, −7/+7, −2/+2 and three unchanged.
    #[test]
    fn a_tag_is_either_counted_or_reported_and_never_neither() {
        for body in [
            // qualified: counted, not reported
            "It lands [verified as proposed].",
            "The row holds [verified — for the snapshot].",
            "Whether it is one of six is [open — which of the six, not whether].",
            // a citation: reported, not counted
            "The estimate is [verified — Pearl 2009].",
            // two standings: reported, not counted
            "It holds [verified — for the ratio; inference for the conclusion].",
            // a qualifier whose detail opens with inline code — the case that broke
            "The factor holds [verified — `crates/project`, as introduced].",
            "Held [verified for the office; `oh-sos` and unasserted as to any act].",
        ] {
            let node = Node::parse(
                PathBuf::from("/tmp/x.yml"),
                ".yidam/corpus/c/x.yml",
                format!("class: c\ndescription: |\n  {body}\n"),
            );
            let reported = !claim_tag_malformed(&prose_views(std::slice::from_ref(&node), &[]))
                .violations
                .is_empty();
            let counted = crate::claims::count_in_source(body).total() > 0;
            assert!(
                reported != counted,
                "{body:?} — reported {reported}, counted {counted}"
            );
        }
    }

    /// And the default is untouched, because ~89% of details across thirteen corpora name a
    /// source. A detail diverts on evidence, never on the absence of it.
    #[test]
    fn a_source_detail_keeps_the_citation_advice() {
        let m = advice_for("The estimate holds [verified — Pearl 2009].");
        assert!(m.contains("put the citation beside it"), "{m}");
        assert!(!m.contains("narrows what is asserted"), "{m}");
    }

    /// A narrowing head is a *word*, not a prefix. `Fordham` is a source and starts with `For`.
    #[test]
    fn a_head_matches_a_word_and_not_a_prefix() {
        let m = advice_for("The framing is [verified — Fordham].");
        assert!(m.contains("put the citation beside it"), "{m}");
    }

    /// 59 of the 105 narrowing details across the population were introduced by a plain space
    /// and so were invisible to the separator rule. They are also the ones the model cannot
    /// express at all: one tag, two standings, two spans of one sentence.
    #[test]
    fn two_standings_are_admitted_without_any_separator() {
        let text = "The share follows [verified for the ratio; inference for the conclusion].";
        let found = near_miss_tags(text);
        assert_eq!(
            found.len(),
            1,
            "a space-introduced narrowing detail: {found:?}"
        );
        assert!(advice_for(text).contains("asserts two things at two standings"));
    }

    /// A second standing counts wherever it sits, not only as the first word.
    #[test]
    fn a_second_standing_later_in_the_detail_is_reported_as_two_claims() {
        let m = advice_for("It holds [inference on the mechanism, [verified] on the measurement].");
        assert!(m.contains("asserts two things at two standings"), "{m}");
        // The advice a qualifier would have got is wrong here and must not appear: one scope
        // field cannot carry two standings, which is why these stay with RFC-0031.
        assert!(!m.contains("put the citation beside it"), "{m}");
    }

    /// The guard on the new admission path, and the reason `TAG_SEPARATORS` excludes a space.
    /// `[open questions]` is ordinary prose and reference-style link text; admitting it would
    /// put a permanent finding in front of every reader who writes the phrase.
    #[test]
    fn ordinary_prose_after_a_standing_word_stays_unreported() {
        for text in [
            "See the [open questions] below.",
            "The [open items] are listed.",
            "An [inference engine] is not a claim.",
        ] {
            assert!(
                near_miss_tags(text).is_empty(),
                "admitted ordinary prose: {text:?} -> {:?}",
                near_miss_tags(text)
            );
        }
    }

    /// `reported_inner` strips one bracket from each end and no more — `trim_matches` would eat
    /// the inner `]` here and lose the second standing that makes this a narrowing detail.
    #[test]
    fn a_nested_bracket_survives_the_inner_strip() {
        assert_eq!(
            reported_inner("[verified for the composition; the basis effect is [open]]"),
            "verified for the composition; the basis effect is [open]"
        );
        assert!(crate::claims::narrowing_detail(reported_inner(
            "[verified for the composition; the basis effect is [open]]"
        )));
    }

    /// A bare tag is the claim, and the split does not touch that rule.
    #[test]
    fn a_bare_tag_is_still_not_a_finding() {
        for text in ["Settled [verified].", "Open [open].", "Read [inference]."] {
            assert!(near_miss_tags(text).is_empty(), "{text:?}");
        }
    }

    /// The two shapes #603 was filed with, verbatim, in the file they were found in.
    ///
    /// The first wraps across three lines and folds two markdown links inside the brackets;
    /// the second folds a backticked crate path. Neither was reported before this check read
    /// class files, and both are exactly what it exists to catch.
    #[test]
    fn the_reported_shapes_are_caught_in_a_class_file() {
        let (_d, classes) = loaded_class(
            "education-agency",
            "class: education-agency\ndescription: |\n  A body a state creates. [verified —\n  \
             [the audit reports that recite each resolution](../catalog/auditor-district-audits.md), and\n  \
             [R.C. 3311.22](../catalog/ohio-revised-code.md)]\n  \
             The register files it. [verified — `dispersion::lea_directory`]\n",
        );
        let c = claim_tag_malformed(&prose_views(&[], &classes));
        assert_eq!(c.violations.len(), 2, "{:?}", c.violations);
        assert!(
            c.violations[0].node.ends_with(":3"),
            "{}",
            c.violations[0].node
        );
        assert!(
            c.violations[1].node.ends_with(":6"),
            "{}",
            c.violations[1].node
        );
    }

    /// All four places a class writes prose, and the reason the scan reads bytes.
    ///
    /// Measured over 17 corpora, the 21 tag sites in class files fall in all four —
    /// `description`, `properties[].description`, `edges[].description` and `analytic_note`.
    /// `ClassProperty` and `ClassEdge` declare no `description` field, so a scan over the
    /// parsed struct would report two of these four and silently miss the rest.
    #[test]
    fn every_prose_field_of_a_class_is_read() {
        let (_d, classes) = loaded_class(
            "agency",
            "class: agency\n\
             description: A body a state creates. [verified — R.C. 3311.22]\n\
             properties:\n  - name: dispersion\n    type: string\n    \
             description: Where its pupils went. [verified — `dispersion::lea_directory`]\n\
             edges:\n  - relationship: succeeds\n    target: agency\n    \
             description: The body it replaced. [inference — the register]\n\
             analytic_note: The closure had no effect on boundaries. [open — whether it held]\n",
        );
        let c = claim_tag_malformed(&prose_views(&[], &classes));
        let lines: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(c.violations.len(), 4, "{lines:?}");
        // One per field, so no two findings share a line.
        let distinct: HashSet<&&str> = lines.iter().collect();
        assert_eq!(distinct.len(), 4, "{lines:?}");
    }

    /// The widening reports nothing against anything this repository ships.
    ///
    /// Paired with the two tests above deliberately: a zero assertion on its own would pass
    /// against a check that read nothing at all. The set is walked rather than listed — a
    /// hardcoded file list stops covering new corpora without ever going red — and an empty
    /// walk fails, because a discovery that finds nothing also passes everything.
    #[test]
    fn no_class_file_in_this_repository_carries_a_near_miss() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let mut paths = Vec::new();
        collect_ont_files(&root, &mut paths);
        paths.sort();
        assert!(
            !paths.is_empty(),
            "no class files found under {} — the walk, not the corpus, is what broke",
            root.display()
        );
        let classes =
            crate::corpus::load_classes(&root, &paths, &crate::corpus::Overlay::default());
        let malformed = claim_tag_malformed(&prose_views(&[], &classes));
        assert!(
            malformed.passed(),
            "{} class file(s) scanned: {:?}",
            paths.len(),
            malformed.violations
        );
        // And the Info companion, whose measurement is the other half of the same claim.
        let uncounted = class_claim_uncounted(&classes);
        assert!(
            uncounted.passed(),
            "{} class file(s) scanned: {:?}",
            paths.len(),
            uncounted.violations
        );
    }

    /// Every `*.ont.yml` this repository **ships**, from git.
    ///
    /// Was a directory walk pruned by a four-name list — `.git`, `target`, `node_modules`,
    /// `dist` — and that list is the #900 defect: it is written twice, here and in
    /// `.gitignore`, and the copies disagree. It reached
    /// `.local/ext-fixture/.yidam/corpus/{concept,gauge}.ont.yml` — a whole staged foreign
    /// repository, git-ignored, which `copy.rs` already refuses to clone for exactly this
    /// reason — so a test whose name says *in this repository* was grading two class files
    /// this repository does not have.
    ///
    /// `ls-files` rather than a pruned walk, because "ships" is a question about the tree
    /// git tracks, and asking git is the only form of the answer that cannot drift from it.
    /// An uncommitted new corpus is covered too: `ls-files` reads the index, not HEAD.
    fn collect_ont_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let output = crate::git::Git::new(dir)
            .args(["ls-files", "-z"])
            .paths(["*.ont.yml"])
            .output()
            .expect("git ls-files");
        assert!(
            output.status.success(),
            "git could not list class files under {} — without it this test grades whatever \
             the working directory happens to contain",
            dir.display()
        );
        out.extend(
            String::from_utf8_lossy(&output.stdout)
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| dir.join(s)),
        );
    }

    // ── class-claim-uncounted (#603) ──────────────────────────────────────────

    /// A class asserting a tag outside backticks is reported, with the number.
    #[test]
    fn a_tag_asserted_in_class_prose_is_reported_with_its_count() {
        let (_d, classes) = loaded_class(
            "remediation",
            "class: remediation\ndescription: |\n  A change made after an incident.\n  \
             It shipped and held. [verified]\nanalytic_note: |\n  Whether it held is [open].\n",
        );
        let c = class_claim_uncounted(&classes);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(
            c.violations[0].detail.contains("asserts 2 evidence tag(s)"),
            "{}",
            c.violations[0].detail
        );
    }

    /// A class explaining its own vocabulary is not reported for naming it.
    ///
    /// This is the shape both `examples/incidents` and `examples/journalism` write, and the
    /// only shape either of them writes: 6 tags across the 26 class files this repository
    /// tracks, every one backticked. Reporting them would put a permanent Info row in front of
    /// every reader of two teaching corpora.
    #[test]
    fn a_class_naming_its_vocabulary_is_not_reported() {
        let (_d, classes) = loaded_class(
            "finding",
            "class: finding\ndescription: |\n  The tiers map onto newsroom standards: \
             `[verified]` means two independent documents support it, `[inference]` means it \
             follows from verified facts, `[open]` is still being chased.\n",
        );
        assert!(class_claim_uncounted(&classes).passed());
    }

    /// Reported, and gates on nothing — the shape `EdgePolicy::Unstated` and `policy-override`
    /// already have. Gating would make a legitimate class file a blocked commit.
    #[test]
    fn the_class_claim_count_never_gates() {
        let (_d, classes) = loaded_class(
            "remediation",
            "class: remediation\ndescription: It shipped and held. [verified]\n",
        );
        let c = class_claim_uncounted(&classes);
        assert_eq!(c.severity, Severity::Info);
        assert!(!c.violations.iter().any(|v| c.gates(v)));
    }

    // ── claim-property-undeclared (#1069) ─────────────────────────────────────

    /// The shape #1069 is really about, from the corpus that has it.
    ///
    /// `bitrecover-bitwipe` carries `claim_tag` on 11 `question` nodes holding exactly the three
    /// standings, and its `question.ont.yml` declares no such property. Every counter in this
    /// repository reads that as no claim.
    #[test]
    fn a_standing_in_a_property_no_class_declared_as_a_claim_is_reported() {
        let (_d, classes) = loaded_class(
            "question",
            "class: question\ndescription: A thing asked.\nproperties:\n  - name: claim_tag\n    type: string\n",
        );
        let nodes = vec![node(
            ".yidam/corpus/question/whether-it-wipes.yml",
            "class: question\nlabel: Whether it wipes\nclaim_tag: open\n",
        )];
        let c = claim_property_undeclared(&nodes, &classes);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let d = &c.violations[0].detail;
        assert!(d.contains("`claim_tag: open`"), "{d}");
        assert!(d.contains("type: claim"), "{d}");
        // And the other repair, because for one of the three measured rows it is the right one.
        assert!(d.contains("rename"), "{d}");
    }

    /// #1382: a standing nested in a list is an entry's, and the repair named is a `list`
    /// with a `claim` field — not the top-level declaration that would make it the node's.
    /// Declaring the record silences it, because the counter then reads it per entry.
    #[test]
    fn a_nested_standing_is_told_to_declare_a_list() {
        let body = "class: model\nlabel: M\nproperties:\n  claim_tag: inference\n  parameters:\n    - name: k\n      claim_tag: open\n";
        let nodes = vec![node(".yidam/corpus/model/m.yml", body)];
        let (_d, top_only) = loaded_class(
            "model",
            "class: model\ndescription: A model.\nproperties:\n  - name: claim_tag\n    type: claim\n",
        );
        let c = claim_property_undeclared(&nodes, &top_only);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(
            c.violations[0].detail.contains(
                "Declare `parameters` as `type: list` with a `claim_tag` field of `type: claim`"
            ),
            "{c:#?}"
        );
        let (_d2, records) = loaded_class(
            "model",
            "class: model\ndescription: A model.\nproperties:\n  - name: claim_tag\n    type: claim\n  - name: parameters\n    type: list\n    fields:\n      - { name: name, type: string }\n      - { name: claim_tag, type: claim }\n",
        );
        assert!(claim_property_undeclared(&nodes, &records).passed());
    }

    /// The declaration is what silences it, and silencing it is the counter starting to read.
    ///
    /// The two halves of the same fact: this check's population is exactly
    /// `count_structural`'s blind spot, so a class that declares the field moves the node out of
    /// here and into the count in one edit. Asserted together, because a check whose finding
    /// could be cleared without the counter gaining anything would be advice for its own sake.
    #[test]
    fn declaring_the_property_clears_the_finding_and_starts_the_count() {
        let body = "class: question\nlabel: Whether it wipes\nclaim_tag: open\n";
        let nodes = vec![node(".yidam/corpus/question/w.yml", body)];

        let (_d, undeclared) = loaded_class(
            "question",
            "class: question\ndescription: A thing asked.\nproperties:\n  - name: claim_tag\n    type: string\n",
        );
        assert_eq!(
            claim_property_undeclared(&nodes, &undeclared)
                .violations
                .len(),
            1
        );
        assert_eq!(
            crate::claims::count_structural(body, &["claim_tag".to_string()]).open,
            1,
            "the counter must be the thing the declaration turns on"
        );

        let (_d2, declared) = loaded_class(
            "question",
            "class: question\ndescription: A thing asked.\nproperties:\n  - name: claim_tag\n    type: claim\n",
        );
        assert!(claim_property_undeclared(&nodes, &declared).passed());
    }

    /// A bracketed value is not reported, because it is already counted.
    ///
    /// Both halves asserted in one test, since the second is the whole reason for the first.
    /// `registered_voters: "[open]"` is the measured shape — a placeholder for a value the
    /// corpus does not have — and telling `demi-moore` to declare a voter count as a claim
    /// field would be this check's only false positive across the population.
    #[test]
    fn a_bracketed_placeholder_is_left_to_the_prose_scan_that_already_counts_it() {
        let (_d, classes) = loaded_class(
            "precinct",
            "class: precinct\ndescription: A voting precinct.\nproperties:\n  - name: registered_voters\n    type: string\n",
        );
        let body = "class: precinct\nlabel: Bath 1\nregistered_voters: \"[open]\"\n";
        let nodes = vec![node(".yidam/corpus/precinct/bath-1.yml", body)];
        assert!(
            claim_property_undeclared(&nodes, &classes).passed(),
            "{:?}",
            claim_property_undeclared(&nodes, &classes).violations
        );
        // Visible already, which is what makes the exclusion safe rather than a gap. No
        // declared field is passed because the class declares none — the shape this check
        // reports — and the text scan reaches the bracketed bytes regardless.
        assert_eq!(crate::claims::count_in_node(body, &[]).open, 1);
    }

    /// A link's standing belongs to its edge, and `edge-claims` grades it there.
    #[test]
    fn a_links_claim_tag_is_not_this_checks_finding() {
        let (_d, classes) = loaded_class("concept", "class: concept\ndescription: An idea.\n");
        let nodes = vec![node(
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlabel: A\nlinks:\n  - target: ../concept/b.yml\n    relationship: reads\n    claim_tag: inference\n",
        )];
        assert!(claim_property_undeclared(&nodes, &classes).passed());
    }

    /// A carried finding's `standing: open` was written by the tool, not the corpus.
    ///
    /// `yidam propose` writes `yidam.findings[].standing: open` into a node, and the
    /// `carried-findings` section of `GRAPH.evidence.md` is explicit that the block is counted
    /// as nothing the corpus claims. A `migrate` test caught this walk reporting it before the
    /// skip existed: telling a corpus to declare `standing` as a claim property because a tool
    /// wrote one into its node would be this check's worst finding.
    #[test]
    fn a_carried_finding_the_tool_wrote_is_not_this_checks_finding() {
        let (_d, classes) = loaded_class("concept", "class: concept\ndescription: An idea.\n");
        let nodes = vec![node(
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlabel: A\nyidam:\n  findings:\n    - id: 7f3a1c94b2e1\n      check: orphan-in\n      opened_at: 4f2a1c9\n      detail: nothing links to this node\n      standing: open\n",
        )];
        let c = claim_property_undeclared(&nodes, &classes);
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// A word that merely starts with a standing is not a standing.
    ///
    /// Measured, and not hypothetical: `allen-county-ohio` writes `event_type: opening` and
    /// `parameter: open highway-rail grade crossings`, and a check that read `starts_with` would
    /// report both. It does not read `starts_with` — it asks `claims::parse_tag`, the same reader
    /// the counter uses — and this holds it to that.
    #[test]
    fn a_word_that_only_begins_with_a_standing_is_not_one() {
        let (_d, classes) = loaded_class("event", "class: event\ndescription: A thing done.\n");
        for value in [
            "opening",
            "open highway-rail grade crossings",
            "verified — all three rows checked against the image by the repository owner",
            "verified for the counts; the code expansions are inference from behaviour",
        ] {
            let nodes = vec![node(
                ".yidam/corpus/event/e.yml",
                &format!("class: event\nlabel: E\nevent_type: \"{value}\"\n"),
            )];
            let c = claim_property_undeclared(&nodes, &classes);
            assert!(c.passed(), "reported {value:?}: {:?}", c.violations);
        }
    }

    /// It reaches a nested key, because the counter it inverts does.
    #[test]
    fn a_standing_nested_under_properties_is_reached() {
        let (_d, classes) = loaded_class("parcel", "class: parcel\ndescription: A parcel.\n");
        let nodes = vec![node(
            ".yidam/corpus/parcel/p.yml",
            "class: parcel\nlabel: P\nproperties:\n  status: verified\n",
        )];
        let c = claim_property_undeclared(&nodes, &classes);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(c.violations[0].detail.contains("`status: verified`"));
    }

    /// A class this corpus does not define is not reported against.
    ///
    /// `unknown-class` is that finding, and reading a missing declaration as *declares no claim
    /// field* would report every node of an undefined class here as well.
    #[test]
    fn a_node_of_an_undefined_class_is_left_to_unknown_class() {
        let (_d, classes) = loaded_class("concept", "class: concept\ndescription: An idea.\n");
        let nodes = vec![node(
            ".yidam/corpus/mystery/m.yml",
            "class: mystery\nlabel: M\nstatus: open\n",
        )];
        assert!(claim_property_undeclared(&nodes, &classes).passed());
    }

    /// A paragraph-long qualified standing is reported on one line.
    #[test]
    fn a_long_value_is_elided_rather_than_reproduced() {
        let (_d, classes) = loaded_class("parcel", "class: parcel\ndescription: A parcel.\n");
        let nodes = vec![node(
            ".yidam/corpus/parcel/p.yml",
            "class: parcel\nlabel: P\nstatus: >-\n  verified for the host-parcel majority \
             and the two roll joins and the clock comparison and the ground filtering\n",
        )];
        let c = claim_property_undeclared(&nodes, &classes);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let d = &c.violations[0].detail;
        assert!(d.lines().count() == 1, "{d}");
        assert!(d.contains('\u{2026}'), "{d}");
    }

    /// Info, and gates on nothing. The measured population includes one row whose right repair
    /// is a rename, and a gate could not tell it from the two whose repair is the declaration.
    #[test]
    fn the_undeclared_claim_property_never_gates() {
        let (_d, classes) =
            loaded_class("question", "class: question\ndescription: A thing asked.\n");
        let nodes = vec![node(
            ".yidam/corpus/question/w.yml",
            "class: question\nlabel: W\nclaim_tag: open\n",
        )];
        let c = claim_property_undeclared(&nodes, &classes);
        assert_eq!(c.severity, Severity::Info);
        assert!(!c.violations.is_empty());
        assert!(!c.violations.iter().any(|v| c.gates(v)));
    }
}
