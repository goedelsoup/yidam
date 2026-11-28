use super::*;

// ── class definitions ─────────────────────────────────────────────────────────

/// Phrasings that assert a reason rather than describe a kind.
///
/// Each is a purpose construction: it says the thing was done *in order to* bring something
/// about, which is a claim about somebody's intent. Kept small on purpose — a longer list
/// buys recall at the cost of the check being switched off.
const PURPOSE_PHRASES: &[&str] = &[
    "designed to",
    "deployed to",
    "intended to",
    "calculated to",
    "engineered to",
    "contrived to",
    "orchestrated to",
    "in order to",
    "so as to",
    "for the purpose of",
    "with the aim of",
    "with the intent",
];

pub const POLICY_OVERRIDE: CheckSpec = CheckSpec {
    id: "policy-override",
    title: "A disclosure decision is this repository's own rule",
    severity: Severity::Info,
    rationale:
        "A policy in `.yidam/policy/` supersedes the default `yidam` carries, and may permit \
         what the default refused — that is the authoritative model RFC-0024 settled on, and a \
         repository is entitled to use it. This check does not object to that. It exists so \
         that the choice cannot be made quietly: a guard loosened in a file nobody reads is the \
         shape `.yidam/private-paths` was built to end, where access control looked enforced \
         and was not. Record why in `.yidam/decisions/`, and use `yidam policy test` to see \
         exactly which inherited expectations your rule changed.",
};

/// Class definitions whose `description` asserts a purpose.
///
/// **Why this check exists where no instance-level check could.** Every other rule in a
/// yidam corpus runs on claim tags, and a tag attaches to a claim somebody makes on a node.
/// A class definition is not that: it is the meaning every instance takes on by being filed
/// under the class — asserted identically, silently, untagged, and for each. So a class
/// defined as "a procedural mechanism *deployed to obtain* an outcome the ordinary path
/// would not yield" makes every one of its instances assert a purpose, and no amount of
/// tagging on the instances can reach it. That definition is real: it survived five
/// resolutions and three separate arguments about its instances in a derived repository,
/// because every safeguard that repository had was pointed at instances.
///
/// **Only `description` is read.** That field says what kind of thing an instance is, and
/// is what an instance silently asserts. A class may perfectly well *discuss* purpose in
/// `analytic_note` — the corrected version of the class above does exactly that, quoting
/// "designed to produce this outcome" in order to forbid it — and reading commentary would
/// flag the discussion of the rule as a violation of it.
///
/// **Warn, and a prompt rather than a proof.** This is a check over wording. It cannot see
/// a purpose asserted in words that are not on the list, and a description can name a
/// purpose someone else attributed without asserting it. Read the finding; do not clear it.
///
/// **The finding quotes the sentence, not just the phrase.** The commonest false positive
/// is a description that carries the phrase inside a clause *disclaiming* purpose — "does
/// not record that a project was pursued in order to obtain anything" — which is the same
/// shape `analytic_note` is exempted for, one field over. Deciding automatically which
/// negations defuse a purpose claim is a judgement this check should not make, so it makes
/// the judgement cheap for the reader instead: the clause is printed with the finding, and
/// a case like that resolves at a glance without opening the file.
/// A decision this repository decided for itself.
///
/// **Not a defect, and reported anyway.** RFC-0024 settled that a repository's policy is
/// authoritative — a local rule decides, including by permitting what the default refused.
/// That is a decision the repository is entitled to make, so this never gates and is `Info`:
/// reporting it as probably-wrong would train people to ignore the check, which is the failure
/// mode `Severity` already argues about one file over.
///
/// What it exists to prevent is the *silent* version. `.yidam/private-paths` was built because
/// a repository's privacy was an assumption — true, load-bearing, and unenforced — and the
/// guideline states the rule this inherits:
///
/// > An assumption about access control that looks enforced and is not is worse than one
/// > everybody knows is manual, because nobody checks the second kind by hand.
///
/// The remedy there was not prohibition; it was making the declaration explicit. So an
/// override appears in `lint --format json`, in `serve --lsp`, and therefore in the editor —
/// where somebody reviewing a diff will actually meet it.
///
/// **It reports that the rule is local, and does not claim to know which way it moved.** That
/// is a question about every possible input; `yidam policy test` answers the part of it that
/// can be answered, by running the inherited cases against the local rule.
pub fn policy_override(overrides: &[(String, String)]) -> Check {
    let violations = overrides
        .iter()
        .map(|(decision, source)| {
            Violation::new(
                source,
                format!(
                    "`{decision}` is decided by this repository's own rule rather than the \
                     one `yidam` ships. Run `yidam policy test` to see which inherited \
                     expectations it no longer meets"
                ),
            )
            .at(Severity::Info)
        })
        .collect();
    Check::new(&POLICY_OVERRIDE, violations)
}

pub const CLASS_ASSERTS_PURPOSE: CheckSpec = CheckSpec {
    id: "class-asserts-purpose",
    title: "Class definition asserts a purpose rather than describing a kind",
    severity: Severity::Warn,
    rationale:
        "A claim tag attaches to a claim somebody makes on a node. A class definition is read \
         differently — it is the meaning every instance takes on by being filed under the \
         class, asserted identically and untagged for each. So a class whose description \
         states a reason ('deployed to obtain an outcome the ordinary path would not yield') \
         states it where no tag attributes it, and the checks that run on tags read past it. \
         The case this was written from survived five resolutions and three arguments about \
         its instances. Rewrite the description to say what an instance IS — the observable \
         shape, the admission conditions — and move any characterization of why to a field \
         that attributes it, or to `analytic_note`, which this check does not read.\n\n\
         That last clause is a rule about what THIS check reads, not a claim about what \
         corpora contain. Class prose does carry evidence tags — measured over 17 corpora, 3 \
         classes in 3 of them do — and #603 was filed because that fact was inferable only by \
         porting the matcher downstream. Two other checks read what this one does not: \
         `claim-tag-malformed` scans every field of the class file, `analytic_note` included, \
         and `class-claim-uncounted` reports the tags there at `Info`. Neither contradicts the \
         escape hatch above. The first reports a tag that is MALFORMED, so a well-formed tag \
         in `analytic_note` stays exactly as fine as this sentence offers; the second reports \
         a count and gates on nothing.",
};

pub fn class_asserts_purpose(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let (hit, sentence) = first_purpose_phrase(&c.description)?;
            Some(Violation::new(
                &c.rel,
                format!("`description` says \"{hit}\" — a purpose, not a kind: \"{sentence}\""),
            ))
        })
        .collect();
    Check::new(&CLASS_ASSERTS_PURPOSE, violations)
}

/// The longest a quoted sentence gets before the middle is elided.
///
/// Long enough for the negating clause that motivates the quoting to survive on either
/// side of a phrase sitting mid-sentence; short enough to stay one line of terminal.
const SENTENCE_BUDGET: usize = 160;

/// The first purpose phrase in `description`, with the sentence carrying it.
///
/// Matching runs per sentence rather than over the whole description, so the phrase and
/// the clause reported for it cannot come from different places, and no index has to be
/// mapped back from the lowercased copy — `to_lowercase` is not length-preserving.
fn first_purpose_phrase(description: &str) -> Option<(&'static str, String)> {
    sentences(description).into_iter().find_map(|s| {
        let lowered = s.to_lowercase();
        let hit = PURPOSE_PHRASES.iter().find(|p| lowered.contains(**p))?;
        Some((*hit, elide(&s, hit)))
    })
}

/// Splits prose into whitespace-collapsed sentences on terminal punctuation.
///
/// Collapsing first means a phrase broken across lines by a literal YAML block still
/// matches, which the previous whole-description `contains` did not manage. Splitting
/// after cannot lose a match either: no phrase on the list contains a `.`, `!`, or `?`,
/// so a break — including a spurious one after an abbreviation — always falls outside a
/// phrase and at worst costs the reader some context in the quote.
fn sentences(text: &str) -> Vec<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = flat.as_bytes();
    for (i, ch) in flat.char_indices() {
        let ends = matches!(ch, '.' | '!' | '?')
            && bytes.get(i + 1).is_none_or(|b| b.is_ascii_whitespace());
        if ends {
            let end = i + 1;
            if !flat[start..end].trim().is_empty() {
                out.push(flat[start..end].trim().to_string());
            }
            start = end;
        }
    }
    if !flat[start..].trim().is_empty() {
        out.push(flat[start..].trim().to_string());
    }
    out
}

/// Trims a sentence to `SENTENCE_BUDGET` characters, keeping `hit` inside the window.
///
/// A long sentence is elided around the phrase rather than from the end, because the
/// words that decide whether the finding is real — a negation, an attribution — are
/// usually the ones next to it.
fn elide(sentence: &str, hit: &str) -> String {
    let chars: Vec<char> = sentence.chars().collect();
    if chars.len() <= SENTENCE_BUDGET {
        return sentence.to_string();
    }
    let at = sentence.to_lowercase().find(hit).unwrap_or(0);
    let at_char = sentence[..at].chars().count();
    let half = (SENTENCE_BUDGET - hit.chars().count().min(SENTENCE_BUDGET)) / 2;
    let hi = (at_char + hit.chars().count() + half).clamp(SENTENCE_BUDGET, chars.len());
    let lo = hi - SENTENCE_BUDGET;
    let mut out = String::new();
    if lo > 0 {
        out.push('…');
    }
    out.extend(&chars[lo..hi]);
    if hi < chars.len() {
        out.push('…');
    }
    out
}

// ── corpus structure ──────────────────────────────────────────────────────────

pub const FOUNDATIONAL_FIELD_MISSPELLED: CheckSpec = CheckSpec {
    id: "foundational-field-misspelled",
    title: "Class declares foundational alignment in a field nothing reads",
    severity: Severity::Error,
    rationale: "Foundational alignment is `foundational_type:`, carrying `ontology: bfo|ufo`, a \
         `type:`, and optionally the `iri:` that type has in that ontology. `bfo_type:` and \
         `ufo_type:` were never read by anything — they are what bootstrap's prose asked for \
         while its own template asked for `foundational_type:`. `bfo_anchor:` was read by \
         `export-rdf` and written by nothing, and was BFO-only besides, so a UFO-aligned \
         corpus could not use it. A class carrying one of these has stated an alignment that \
         no gate, query, or export can see, which is worse than stating none: the corpus \
         believes it is aligned. RFC-0013 is the record.",
};

/// A class file naming foundational alignment in a spelling nothing reads.
///
/// `foundational_type:` is the field: `yidam schema` validates it, the bootstrap template
/// writes it, and `export-rdf` reads it. Three other spellings existed. `bfo_type:` and
/// `ufo_type:` were what `bootstrap.md` told authors to write, in prose, one hundred and
/// ninety lines above its own template writing the correct one — so an agent following the
/// dialogue produced a field no part of the toolchain had ever read. `bfo_anchor:` was read
/// by `export-rdf` and written by nothing, which is the same bug from the other end: the RDF
/// export looked for a field bootstrap never produced, so a corpus that declared an alignment
/// exported no `skos:exactMatch` at all.
///
/// None of it went red. The class body is `additionalProperties: true`, so an undeclared key
/// validates, and no corpus this repository ships declares an alignment — the path was never
/// exercised. This check is the thing that would have said so. It gates rather than warns,
/// because a silently-ignored alignment field is indistinguishable from an alignment.
pub fn foundational_field_misspelled(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .flat_map(|c| {
            c.dead_alignment_fields.iter().map(move |f| {
                let fix = if *f == "bfo_anchor" {
                    "move the URI to `foundational_type.iri`"
                } else {
                    "write `foundational_type:` with `ontology:` and `type:`"
                };
                Violation::new(&c.rel, format!("`{f}:` is read by nothing — {fix}"))
            })
        })
        .collect();
    Check::new(&FOUNDATIONAL_FIELD_MISSPELLED, violations)
}

pub const FOUNDATIONAL_TYPE_MALFORMED: CheckSpec = CheckSpec {
    id: "foundational-type-malformed",
    title: "Class declares a foundational alignment that cannot be resolved",
    severity: Severity::Error,
    rationale: "`foundational_type:` carries `ontology: bfo` or `ontology: ufo`, a non-empty \
         `type:`, and optionally an `iri:` beginning `http://` or `https://`. An alignment \
         outside that shape reaches RDF as a literal no consumer can follow, which leaves \
         the corpus asserting an alignment nothing can check — the state #613 found across \
         four field names. The `type:` itself is not checked against BFO or UFO: their term \
         sets are theirs to revise, and a copy of them in this binary would start refusing \
         terms a corpus is right to use.",
};

/// A class that declared an alignment the vocabulary does not have.
///
/// `foundational_type` is validated by `yidam schema` for shape — that `ontology` and `type`
/// are present strings — but a schema cannot say that `bfo-2` is not an ontology or that
/// `BFO_0000002` is not an IRI. Both produce an alignment that exports as a literal nobody
/// can resolve, which is the failure `foundational_field_misspelled` exists for wearing a
/// valid field name.
///
/// The `type` is deliberately unchecked. BFO and UFO both have large term sets that their
/// maintainers revise, and a list of them compiled into this binary would be a second copy of
/// somebody else's vocabulary — wrong the moment they revise it, and refusing terms a corpus
/// is right to use. `ontology` is checkable because it is two values and they are ours.
pub fn foundational_type_malformed(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let ft = c.foundational_type.as_ref()?;
            let problem = if !matches!(ft.ontology.as_str(), "bfo" | "ufo") {
                format!(
                    "`ontology: {}` — the values are `bfo` and `ufo`",
                    ft.ontology
                )
            } else if ft.ty.trim().is_empty() {
                format!("`ontology: {}` with no `type:`", ft.ontology)
            } else if ft
                .iri
                .as_deref()
                .is_some_and(|i| !i.starts_with("http://") && !i.starts_with("https://"))
            {
                format!(
                    "`iri: {}` is not an absolute IRI",
                    ft.iri.as_deref().unwrap_or_default()
                )
            } else {
                return None;
            };
            Some(Violation::new(&c.rel, problem))
        })
        .collect();
    Check::new(&FOUNDATIONAL_TYPE_MALFORMED, violations)
}

pub const LIST_DECLARATION_INVALID: CheckSpec = CheckSpec {
    id: "list-declaration-invalid",
    title: "Class declares a list whose items cannot be read",
    severity: Severity::Error,
    rationale: "A `list` declares its items with `items:`, naming one scalar type, or with \
         `fields:`, declaring a record — or with neither, and then holds any non-empty list. \
         Both at once, an `items:` naming a list, a quotation or a coined type, or a record \
         field typed `list` or `quotation` is a declaration nothing can hold an instance to, \
         and the gate falls back to checking only that the value is a list. Reported here so \
         the fallback is not silent.",
};

/// A `list` declaration that cannot be read as either form (RFC-0050).
///
/// The instance side is [`property_type`]'s; this is the class side, reported once on the
/// class file rather than once per instance under it, for the reason
/// [`foundational_type_malformed`] reports an alignment there. Under a declaration this
/// reports, instances are held only to the bare `list` shape: neither half of a
/// contradictory declaration is the one the class meant.
pub fn list_declaration_invalid(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .flat_map(|c| {
            c.properties.iter().filter_map(|p| {
                list_declaration_problem(p)
                    .map(|why| Violation::new(&c.rel, format!("`{}` {why}", p.name)))
            })
        })
        .collect();
    Check::new(&LIST_DECLARATION_INVALID, violations)
}

pub const PROPERTY_TYPE_UNKNOWN: CheckSpec = CheckSpec {
    id: "property-type-unknown",
    title: "Class declares a property type nothing reads",
    severity: Severity::Warn,
    rationale:
        "A `type:` the tooling does not define is read by nothing: `property-type` leaves its \
         values alone, the compiled schema admits anything, and a standing or a `source` \
         inside it is neither graded nor resolved. A corpus that coins `type: effect` for a \
         list of evidenced entries gets no finding, and its entries no checks. Reported so \
         the choice is made knowingly: a known type, or a `list` with `fields:` for a value \
         that holds records. Not a gate, because a coined type is unread, not wrong.",
};

/// A declared `type:` that no reader grades (#1391).
///
/// [`property_type`] leaves a coined type alone, and so does every other reader: the compiled
/// schema gives it `true`, the claim reader skips it, the warehouse gives it no column. So a
/// corpus that writes `type: effect` hears nothing, and its values are held to nothing.
/// `travel-demand` found that by probing: a `source` naming no entry, a `claim_tag` of
/// `definitely-true` and a text `value` all passed. Reported once on the class, for a property
/// or a record field, as [`list_declaration_invalid`] reports a declaration. A warning, not an
/// error: coining was tolerated, and a coined type is not a wrong value, only an unread one.
pub fn property_type_unknown(classes: &[Class]) -> Check {
    let kinds = KNOWN_PROPERTY_TYPES.join("`, `");
    let unknown = |t: &str| !t.is_empty() && !KNOWN_PROPERTY_TYPES.contains(&t);
    let violations = classes
        .iter()
        .flat_map(|c| {
            c.properties.iter().flat_map(move |p| {
                let own = unknown(&p.r#type).then(|| (p.name.clone(), p.r#type.clone()));
                let fields = p
                    .fields
                    .iter()
                    .filter(|f| unknown(&f.r#type))
                    .map(|f| (format!("{}[].{}", p.name, f.name), f.r#type.clone()));
                own.into_iter()
                    .chain(fields)
                    .map(|(name, t)| (c.rel.clone(), name, t))
                    .collect::<Vec<_>>()
            })
        })
        .map(|(rel, name, t)| {
            Violation::new(
                &rel,
                format!(
                    "`{name}` is declared `type: {t}`, which nothing reads, so its values pass \
                     unchecked — use one of `{kinds}`; a value holding records is a `list` with \
                     `fields:`"
                ),
            )
        })
        .collect();
    Check::new(&PROPERTY_TYPE_UNKNOWN, violations)
}

/// #613 — foundational alignment had four field names and the export read the one nothing
/// wrote. These cover the two checks that make that state visible.
#[cfg(test)]
mod foundational_alignment_tests {
    use super::*;

    /// Build a class from its file's actual bytes, so these exercise the deserialization the
    /// bug lived in rather than a struct literal that cannot reproduce it.
    fn ont(rel: &str, yaml: &str) -> Class {
        Class::parse(rel, yaml)
    }

    #[test]
    fn the_field_bootstrap_asked_for_is_read_by_nothing_and_says_so() {
        // `bootstrap.md` told authors to write this for the whole life of the field, in prose
        // 190 lines above its own template writing the correct one. #613.
        for dead in ["bfo_type: continuant", "ufo_type: relator"] {
            let c =
                foundational_field_misspelled(&[ont("x.ont.yml", &format!("class: x\n{dead}\n"))]);
            assert!(!c.passed(), "{dead} must not pass");
            assert!(
                c.violations[0].detail.contains("foundational_type"),
                "the finding must name the field that works, got: {}",
                c.violations[0].detail
            );
        }
    }

    #[test]
    fn the_field_the_rdf_export_used_to_read_is_named_too() {
        let c = foundational_field_misspelled(&[ont(
            "x.ont.yml",
            "class: x\nbfo_anchor: http://purl.obolibrary.org/obo/BFO_0000002\n",
        )]);
        assert!(!c.passed());
        assert!(
            c.violations[0].detail.contains("iri"),
            "a bfo_anchor holds a URI, so the repair is where the URI now goes: {}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_correct_alignment_passes_both_checks() {
        let good = ont(
            "x.ont.yml",
            "class: x\nfoundational_type:\n  ontology: ufo\n  type: relator\n  iri: https://purl.org/nemo/gufo#Relator\n",
        );
        assert!(foundational_field_misspelled(std::slice::from_ref(&good)).passed());
        assert!(foundational_type_malformed(&[good]).passed());
    }

    #[test]
    fn an_alignment_with_no_iri_is_complete() {
        // The IRI is optional on purpose: the alignment is worth stating without one, and
        // requiring it would mean shipping a BFO/gUFO term table to derive it.
        let c = ont(
            "x.ont.yml",
            "class: x\nfoundational_type:\n  ontology: bfo\n  type: continuant\n",
        );
        assert!(foundational_type_malformed(&[c]).passed());
    }

    #[test]
    fn a_class_with_no_alignment_at_all_passes() {
        // Every corpus this repository ships is here, and "none" is an answer the bootstrap
        // dialogue offers by name.
        let c = ont("x.ont.yml", "class: x\ndescription: A thing.\n");
        assert!(foundational_field_misspelled(std::slice::from_ref(&c)).passed());
        assert!(foundational_type_malformed(&[c]).passed());
    }

    #[test]
    fn an_ontology_outside_the_two_is_malformed() {
        for bad in [
            "  ontology: bfo-2\n  type: continuant",
            "  ontology: bfo\n  type: \"\"",
            "  ontology: bfo\n  type: continuant\n  iri: BFO_0000002",
        ] {
            let c = foundational_type_malformed(&[ont(
                "x.ont.yml",
                &format!("class: x\nfoundational_type:\n{bad}\n"),
            )]);
            assert!(!c.passed(), "must not pass: {bad}");
        }
    }

    #[test]
    fn the_type_itself_is_not_checked_against_a_term_list() {
        // A list of BFO/UFO terms in this binary would start refusing terms a corpus is right
        // to use the first time either ontology is revised.
        let c = ont(
            "x.ont.yml",
            "class: x\nfoundational_type:\n  ontology: bfo\n  type: a-term-invented-today\n",
        );
        assert!(foundational_type_malformed(&[c]).passed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    #[test]
    fn the_definition_that_motivated_the_check_is_caught() {
        // Verbatim from a derived repository's genesis. Every instance of this class
        // asserted a purpose by existing, and no instance-level tag could reach it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A procedural mechanism deployed to obtain an outcome the ordinary path \
             would not yield.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("deployed to"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn the_documentary_rewrite_of_that_definition_passes() {
        // The same class after it was redefined: an observable shape and its admission
        // conditions, with no claim about anyone's reason.
        assert!(class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A dated procedural sequence that departed from a baseline the record fixed \
             before the sequence and independently of it. The departure is the whole of \
             the class's content, and it is a comparison rather than a motive.",
        )])
        .passed());
    }

    #[test]
    fn commentary_outside_description_is_not_read() {
        // `analytic_note` is where a class discusses the purpose rule — often by quoting
        // the forbidden phrasing in order to forbid it. Reading it would flag the
        // discussion of the rule as a violation of the rule.
        let text = "class: maneuver\ndescription: A dated procedural sequence.\n\
                    analytic_note: |\n  \"designed to produce this outcome\" is an \
                    allegation, not a record.\n";
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("maneuver.ont.yml");
        std::fs::write(&p, text).unwrap();
        let loaded =
            crate::corpus::load_classes(dir.path(), &[p], &crate::corpus::Overlay::default());
        assert_eq!(loaded[0].description.trim(), "A dated procedural sequence.");
        assert!(class_asserts_purpose(&loaded).passed());
    }

    #[test]
    fn the_match_is_case_insensitive() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument Designed To pass.")]);
        assert_eq!(c.violations.len(), 1);
    }

    #[test]
    fn the_finding_quotes_the_sentence_the_phrase_sits_in() {
        // Direction (3) of the report: the reader decides whether a hit is real, so the
        // clause is put in front of them rather than left in the file.
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A dated procedural sequence. It was designed to obtain an outcome.",
        )]);
        assert_eq!(
            c.violations[0].detail,
            "`description` says \"designed to\" — a purpose, not a kind: \
             \"It was designed to obtain an outcome.\""
        );
    }

    #[test]
    fn the_disclaiming_sentence_is_quoted_with_its_negation() {
        // Verbatim shape from the report: the phrase occurs inside the clause refusing
        // to assert a purpose. Still a finding — but one a reader can resolve from the
        // message, because the words that defuse it are printed with it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/project.ont.yml",
            "It records what a body minuted, on what date, for how much. It does not \
             record that anyone intended an outcome, that a project was pursued in order \
             to obtain anything, or that a named person caused it to exist.",
        )]);
        assert_eq!(c.violations.len(), 1);
        let detail = &c.violations[0].detail;
        assert!(detail.contains("does not record"), "{detail}");
        assert!(detail.contains("in order to obtain anything"), "{detail}");
    }

    #[test]
    fn only_the_sentence_carrying_the_phrase_is_quoted() {
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A minuted decision. An instrument intended to bind. Filed by date.",
        )]);
        assert!(
            c.violations[0]
                .detail
                .ends_with("\"An instrument intended to bind.\""),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_phrase_broken_across_lines_is_still_found() {
        // A literal YAML block keeps its newlines. The old whole-description `contains`
        // missed a phrase straddling one; collapsing whitespace before matching does not.
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A mechanism deployed\n   to obtain an outcome.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.ends_with(
                "\"A mechanism deployed to obtain an \
                 outcome.\""
            ),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_long_sentence_is_elided_around_the_phrase() {
        let pad = "word ".repeat(60);
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            &format!("{pad}not designed to bind {pad}"),
        )]);
        let detail = &c.violations[0].detail;
        let quoted = detail.split_once(": \"").unwrap().1.trim_end_matches('"');
        assert!(quoted.chars().count() <= SENTENCE_BUDGET + 2, "{quoted}");
        assert!(quoted.starts_with('…') && quoted.ends_with('…'), "{quoted}");
        // The negation next to the phrase is what makes the quote worth printing.
        assert!(quoted.contains("not designed to bind"), "{quoted}");
    }

    #[test]
    fn a_short_sentence_is_quoted_whole_and_unelided() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument intended to bind.")]);
        assert!(!c.violations[0].detail.contains('…'));
    }

    #[test]
    fn a_class_with_no_description_passes() {
        assert!(class_asserts_purpose(&[class("x.ont.yml", "")]).passed());
    }

    #[test]
    fn the_check_warns_rather_than_gates() {
        // A heuristic over wording. Gating on it would make every false positive a
        // blocked commit, and the check would be switched off within a week.
        assert_eq!(class_asserts_purpose(&[]).severity, Severity::Warn);
    }

    /// A declaration nothing can hold an instance to is reported once, on the class — and
    /// an instance under it is held only to the bare shape, not to either half.
    #[test]
    fn an_unreadable_list_declaration_is_reported_on_the_class() {
        let reported = [
            "properties:\n  - name: xs\n    type: list\n    items: string\n    fields:\n      - { name: a, type: string }\n",
            "properties:\n  - name: xs\n    type: list\n    items: quotation\n",
            "properties:\n  - name: xs\n    type: list\n    items: list\n",
            "properties:\n  - name: xs\n    type: list\n    items: effect\n",
            "properties:\n  - name: xs\n    type: list\n    fields:\n      - { name: a, type: list }\n",
            "properties:\n  - name: xs\n    type: list\n    fields:\n      - { name: a, type: quotation }\n",
        ];
        let xs = |v: &str| {
            vec![node(
                ".yidam/corpus/c/alpha.yml",
                &format!("class: c\nproperties:\n  xs: {v}\nlinks: []\n"),
            )]
        };
        for declaration in reported {
            let classes = vec![class_from("c", declaration)];
            let c = list_declaration_invalid(&classes);
            assert_eq!(c.violations.len(), 1, "{declaration}: {c:#?}");
            assert!(c.violations[0].detail.starts_with("`xs` "), "{c:#?}");
            assert!(
                property_type(&xs("[7, {z: 1}]"), &classes, &NONE).passed(),
                "{declaration}"
            );
            assert_eq!(
                property_type(&xs("seven"), &classes, &NONE)
                    .violations
                    .len(),
                1
            );
        }
        for declaration in [
            "properties:\n  - name: xs\n    type: list\n",
            "properties:\n  - name: xs\n    type: list\n    items: claim\n",
            "properties:\n  - name: xs\n    type: list\n    fields:\n      - { name: a, type: span }\n",
            // `items:` off a list is carried and ignored, as `values:` is off a string.
            "properties:\n  - name: xs\n    type: string\n    items: quotation\n",
        ] {
            let classes = vec![class_from("c", declaration)];
            assert!(list_declaration_invalid(&classes).passed(), "{declaration}");
        }
    }

    // ── property-type-unknown (#1391) ─────────────────────────────────────────

    /// `travel-demand`'s `type: effect`, and a coined field inside a record, are each reported
    /// once on the class; every type a reader grades, and a property with no `type:`, is not.
    #[test]
    fn a_coined_type_is_reported_on_the_class_and_a_known_one_is_not() {
        let classes = vec![class_from(
            "pathway",
            "properties:\n  - name: effect\n    type: effect\n  \
             - name: effects\n    type: list\n    fields:\n      \
             - { name: span, type: interval }\n      - { name: claim_tag, type: claim }\n  \
             - name: untyped\n",
        )];
        let c = property_type_unknown(&classes);
        assert_eq!(c.severity, Severity::Warn);
        let details: Vec<_> = c.violations.iter().map(|v| v.detail.as_str()).collect();
        assert_eq!(details.len(), 2, "{c:#?}");
        assert!(
            details[0].starts_with("`effect` is declared `type: effect`"),
            "{details:?}"
        );
        assert!(
            details[1].starts_with("`effects[].span` is declared `type: interval`"),
            "{details:?}"
        );
        assert!(
            c.violations
                .iter()
                .all(|v| v.node.ends_with("pathway.ont.yml")),
            "{c:#?}"
        );

        let known = KNOWN_PROPERTY_TYPES
            .iter()
            .map(|t| format!("  - name: p_{t}\n    type: {t}\n"))
            .collect::<String>();
        let classes = vec![class_from("c", &format!("properties:\n{known}"))];
        assert!(property_type_unknown(&classes).passed(), "{known}");
    }
}
