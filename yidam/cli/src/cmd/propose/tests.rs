//! The planner against a real corpus on disk, and the constitutional rule as an assertion.
//!
//! [`super::draft`]'s own tests hold the pure functions. What is left for here is the part
//! that needs a repository: which findings turn into which proposals, that a re-run does not
//! ask the same question twice, and that a question is retired when its finding goes.

use std::path::Path;
use std::process::Command;

use super::draft::Verb;
use super::*;

/// A corpus shaped like the worked example: one class points at another.
///
/// `gage` declares `sources-from -> concept, direction: out`, and that is what makes
/// `concept` a class something is meant to point at — so an uncited `concept` is a finding,
/// while a `gage` is exempt because nothing is declared to point at gages.
///
/// Self-edges do not count. `concept.refines -> concept` says concepts relate to each other,
/// not that every concept is cited: any acyclic self-relation has an endpoint that is not.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let corpus = root.join(".yidam/corpus");
    std::fs::create_dir_all(corpus.join("concept")).unwrap();
    std::fs::create_dir_all(corpus.join("gage")).unwrap();
    std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();

    std::fs::write(
        corpus.join("concept.ont.yml"),
        "class: concept\nlabel: Concept\ndescription: |\n  A notion used to describe flow.\n\
         properties:\n  - name: claim_tag\n    type: claim\n    description: The standing.\n\
         edges:\n  - relationship: refines\n    target: concept\n    direction: out\n    \
         description: A concept this one narrows.\n",
    )
    .unwrap();
    std::fs::write(
        corpus.join("gage.ont.yml"),
        "class: gage\nlabel: Gage\ndescription: |\n  A station on a channel.\n\
         properties:\n  - name: claim_tag\n    type: claim\n    description: The standing.\n\
         edges:\n  - relationship: sources-from\n    target: concept\n    direction: out\n    \
         description: A concept this record computes.\n",
    )
    .unwrap();
    node(
        root,
        "concept",
        "cited",
        "  A gage points at this one. [verified]",
        &[],
    );
    node(
        root,
        "concept",
        "lonely",
        "  Nothing points at this one. [verified]",
        &[],
    );
    node(
        root,
        "gage",
        "probe",
        "  It sources from a concept. [verified]",
        &["../concept/cited.yml"],
    );

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t"]);
    git(root, &["config", "user.name", "Tester"]);
    commit(root, "genesis: corpus");
    dir
}

/// Write an instance whose description is a block scalar, pointing at `links`.
fn node(root: &Path, class: &str, name: &str, prose: &str, links: &[&str]) {
    let edges: String = links
        .iter()
        .map(|t| format!("  - target: {t}\n    relationship: sources-from\n"))
        .collect();
    std::fs::write(
        root.join(format!(".yidam/corpus/{class}/{name}.yml")),
        format!(
            "class: {class}\nlabel: {name}\ndescription: |\n{prose}\nproperties:\n  \
             claim_tag: verified\nlinks:\n  - target: ../{class}.ont.yml\n    \
             relationship: instance-of\n{edges}"
        ),
    )
    .unwrap();
}

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

/// Point `cited` at `lonely`, which is what resolves the orphan finding.
fn link_lonely(root: &Path) {
    let p = ".yidam/corpus/concept/cited.yml";
    let text = read(root, p).replace(
        "    relationship: instance-of\n",
        "    relationship: instance-of\n  - target: ./lonely.yml\n    relationship: refines\n",
    );
    std::fs::write(root.join(p), text).unwrap();
}

fn checks_at(root: &Path) -> Vec<lint::model::Check> {
    lint::run_checks(
        root,
        &lint::Options {
            warn_only: true,
            explain: false,
            commits: false,
            range: None,
            bless: false,
            init_baseline: false,
            format: crate::report::Format::Text,
        },
    )
}

/// The planner, with `orphan-in` escalated so its findings gate.
fn plan_at(root: &Path, threshold: Option<usize>) -> (Vec<Proposal>, Vec<Skipped>) {
    let mut checks = checks_at(root);
    for c in &mut checks {
        if c.id == "orphan-in" {
            *c = c.clone().escalating_after(Some(1));
        }
    }
    let head = write::head(root).unwrap().1;
    plan(root, &checks, threshold, &head).unwrap()
}

#[test]
fn an_escalated_orphan_becomes_one_open_proposal_against_the_node_itself() {
    let dir = repo();
    let root = dir.path();
    let (proposals, skipped) = plan_at(root, None);

    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!(proposals.len(), 1, "{:?}", subjects(&proposals));
    let p = &proposals[0];
    assert_eq!(p.verb, Verb::Open);
    assert_eq!(p.node, ".yidam/corpus/concept/lonely.yml");
    assert_eq!(
        p.changes.len(),
        1,
        "one file, and it is the node the finding is about"
    );
    assert_eq!(p.changes[0].path(), ".yidam/corpus/concept/lonely.yml");
}

/// The constitutional rule, as an assertion. A proposal that paraphrased its finding would
/// be composing, and RFC-0020's whole design rests on it not doing that.
#[test]
fn every_proposal_carries_its_finding_verbatim() {
    let dir = repo();
    let root = dir.path();
    let (proposals, _) = plan_at(root, Some(1));
    assert!(!proposals.is_empty());
    for p in &proposals {
        assert!(
            p.carries(),
            "{} does not quote its finding:\n{}",
            p.subject,
            p.body
        );
    }
}

/// Every proposal's verb is one `lint --commits` recognizes, which is what keeps a branch of
/// generated commits from failing the check that makes the log legible.
#[test]
fn every_subject_is_in_the_closed_vocabulary() {
    let dir = repo();
    let root = dir.path();
    let (proposals, _) = plan_at(root, Some(1));
    assert!(!proposals.is_empty());
    for p in &proposals {
        assert!(
            yidam_core::git::is_recognized_verb(p.verb.as_str()),
            "`{}` is not in GRAPH.md's vocabulary",
            p.verb.as_str()
        );
        let subject = p.message("abc1234").lines().next().unwrap().to_string();
        assert!(
            subject.starts_with(&format!("{}: ", p.verb.as_str())),
            "{subject}"
        );
    }
}

/// A second run must not ask the same question again. Without this the branch grows one copy
/// of every outstanding finding per HEAD, which is a report that writes itself repeatedly —
/// the failure this command exists to fix, wearing a different hat.
#[test]
fn a_question_already_asked_is_not_asked_twice() {
    let dir = repo();
    let root = dir.path();
    let (first, _) = plan_at(root, None);
    assert_eq!(first.len(), 1);

    // Apply it, as a merge would.
    let Change::Write { path, content } = &first[0].changes[0] else {
        panic!("an open proposal writes")
    };
    std::fs::write(root.join(path), content).unwrap();
    commit(root, "open: lonely — nothing links to this node");

    let (second, _) = plan_at(root, None);
    assert!(
        second.iter().all(|p| p.verb != Verb::Open),
        "asked again: {:?}",
        subjects(&second)
    );
}

/// The closing half. The finding goes away, so the question this command opened is retired —
/// and the node comes back to exactly what it was.
#[test]
fn a_question_is_closed_when_its_finding_is_gone_and_the_node_is_restored() {
    let dir = repo();
    let root = dir.path();
    let before = read(root, ".yidam/corpus/concept/lonely.yml");

    let (first, _) = plan_at(root, None);
    let Change::Write { path, content } = &first[0].changes[0] else {
        panic!("an open proposal writes")
    };
    std::fs::write(root.join(path), content).unwrap();
    commit(root, "open: lonely — nothing links to this node");
    assert_ne!(read(root, ".yidam/corpus/concept/lonely.yml"), before);

    // Somebody answers it: the node gets cited.
    link_lonely(root);
    commit(root, "establish: cited refines lonely");

    let (second, _) = plan_at(root, None);
    let closes: Vec<&Proposal> = second.iter().filter(|p| p.verb == Verb::Close).collect();
    assert_eq!(closes.len(), 1, "{:?}", subjects(&second));
    let Change::Write { content, .. } = &closes[0].changes[0] else {
        panic!("a close proposal writes")
    };
    assert_eq!(
        *content, before,
        "the node is exactly what it was before the question"
    );
}

/// Prose a person wrote is never this command's to retire, however much it looks like one of
/// its own questions.
#[test]
fn a_question_a_person_wrote_is_never_closed() {
    let dir = repo();
    let root = dir.path();
    let p = ".yidam/corpus/concept/cited.yml";
    let text = read(root, p).replace(
        "  A gage points at this one. [verified]\n",
        "  A gage points at this one. [verified]\n\n  Whether it should is unresolved. \
         [open]\n",
    );
    std::fs::write(root.join(p), text).unwrap();
    commit(root, "open: cited — whether it should be pointed at");

    let (proposals, _) = plan_at(root, None);
    assert!(
        proposals.iter().all(|x| x.verb != Verb::Close),
        "{:?}",
        subjects(&proposals)
    );
}

/// Absent the declaration, no withdrawal is ever drafted — which is every corpus by default.
#[test]
fn no_declaration_means_no_withdrawal() {
    let dir = repo();
    let root = dir.path();
    let (proposals, _) = plan_at(root, None);
    assert!(proposals.iter().all(|p| p.verb != Verb::Withdraw));
}

/// With it, the same finding is withdrawn instead of asked about — and only once. A node
/// being deleted must not also be handed a question.
#[test]
fn a_declared_threshold_withdraws_and_does_not_also_ask() {
    let dir = repo();
    let root = dir.path();
    let (proposals, _) = plan_at(root, Some(1));

    let by_node: Vec<&Proposal> = proposals
        .iter()
        .filter(|p| p.node == ".yidam/corpus/concept/lonely.yml")
        .collect();
    assert_eq!(by_node.len(), 1, "{:?}", subjects(&proposals));
    assert_eq!(by_node[0].verb, Verb::Withdraw);
    assert!(matches!(by_node[0].changes[0], Change::Remove { .. }));
}

/// A threshold the finding has not reached is not a licence.
#[test]
fn a_threshold_above_the_residence_time_withdraws_nothing() {
    let dir = repo();
    let root = dir.path();
    let (proposals, _) = plan_at(root, Some(10_000));
    assert!(proposals.iter().all(|p| p.verb != Verb::Withdraw));
    assert!(
        proposals.iter().any(|p| p.verb == Verb::Open),
        "the question still stands"
    );
}

/// A withdrawal takes the node out of any catalog that listed it, in the same commit —
/// otherwise it trades one finding for a `catalog-used-by-drift`.
#[test]
fn a_withdrawal_also_edits_the_catalog_that_listed_the_node() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join(".yidam/catalog/source.md"),
        "---\nname: source\ndescription: A source.\ntype: api\nobtained: true\nlocation:\n  \
         - kind: url\n    value: https://example.org\nused-by:\n  \
         - ../corpus/concept/lonely.yml\n  - ../corpus/concept/cited.yml\n---\n\n# Source\n\nProse.\n",
    )
    .unwrap();
    commit(root, "catalog: a source");

    let (proposals, _) = plan_at(root, Some(1));
    let w = proposals
        .iter()
        .find(|p| p.verb == Verb::Withdraw)
        .expect("a withdrawal");
    let paths: Vec<&str> = w.changes.iter().map(|c| c.path()).collect();
    assert!(paths.contains(&".yidam/catalog/source.md"), "{paths:?}");
    let Some(Change::Write { content, .. }) = w
        .changes
        .iter()
        .find(|c| c.path() == ".yidam/catalog/source.md")
    else {
        panic!("the catalog is rewritten")
    };
    assert!(!content.contains("lonely.yml"));
    assert!(content.contains("cited.yml"), "the other citation stays");
}

/// A finding this command cannot draft for is reported, not dropped. A finding silently not
/// proposed about is the failure mode the whole command exists to remove.
#[test]
fn a_node_with_a_plain_scalar_description_is_reported_as_skipped() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join(".yidam/corpus/concept/lonely.yml"),
        "class: concept\nlabel: lonely\ndescription: One line, not a block.\nproperties:\n  \
         claim_tag: verified\nlinks:\n  - target: ../concept.ont.yml\n    \
         relationship: instance-of\n",
    )
    .unwrap();
    commit(root, "revise: lonely says it on one line");

    let (proposals, skipped) = plan_at(root, None);
    assert!(proposals.is_empty(), "{:?}", subjects(&proposals));
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].node, ".yidam/corpus/concept/lonely.yml");
    assert!(
        skipped[0].reason.contains("block scalar"),
        "{}",
        skipped[0].reason
    );
}

/// A finding the baseline forgives is debt the corpus already dispositioned, and re-raising
/// it as a question would be arguing with a decision somebody recorded.
#[test]
fn a_baselined_finding_is_not_proposed_about() {
    let dir = repo();
    let root = dir.path();
    assert_eq!(plan_at(root, None).0.len(), 1);

    let mut checks = checks_at(root);
    for c in &mut checks {
        if c.id == "orphan-in" {
            *c = c.clone().escalating_after(Some(1));
        }
    }
    let head = crate::cmd::lint::history::corpus_commits(root)
        .last()
        .cloned()
        .unwrap_or_default();
    let b = crate::cmd::lint::baseline::Baseline::from_checks(
        &checks,
        &crate::cmd::lint::baseline::Baseline::default(),
        &head,
    );
    b.write(root).unwrap();
    commit(root, "fix: bless the orphan");

    assert!(plan_at(root, None).0.is_empty(), "the baseline forgives it");
}

/// The report says what did not happen. A reader who assumes a proposal landed is the failure
/// mode of a command that writes epistemic commits.
#[test]
fn the_report_says_nothing_was_merged() {
    let r = ProposeReport {
        branch: "propose/abc1234".into(),
        head: "abc1234".into(),
        proposals: vec![Drafted {
            verb: "open",
            subject: "lonely — nothing links to this node".into(),
            check: "orphan-in".into(),
            node: ".yidam/corpus/concept/lonely.yml".into(),
            detail: "nothing links to this node".into(),
            paths: vec![".yidam/corpus/concept/lonely.yml".into()],
        }],
        skipped: vec![],
        written: Some(write::Written {
            branch: "propose/abc1234".into(),
            commits: vec!["deadbee".into()],
        }),
        withdraw_uncited_after: None,
    };
    let text = render(&r, false);
    assert!(
        text.contains("nothing was merged, and no claim was re-tagged"),
        "{text}"
    );
    assert!(
        text.contains("git log --reverse"),
        "review is one command away: {text}"
    );
    assert!(
        text.contains("git branch -D"),
        "and so is rejection: {text}"
    );
}

/// A quiet run says the threshold is off rather than printing a bare "nothing", which reads
/// as a clean bill of health on a corpus that simply never licensed the act.
#[test]
fn a_quiet_run_says_the_withdrawal_threshold_is_undeclared() {
    let r = ProposeReport {
        branch: "propose/abc1234".into(),
        head: "abc1234".into(),
        proposals: vec![],
        skipped: vec![],
        written: None,
        withdraw_uncited_after: None,
    };
    let text = render(&r, false);
    assert!(text.contains("withdraw_uncited_after"), "{text}");
    assert!(text.contains("That is the default"), "{text}");
}

/// An expired source record, licensed by a TTL this corpus declared.
fn with_expired_source(root: &Path) {
    std::fs::write(
        root.join(".yidam/config.toml"),
        "[catalog]\nttl_days = 30\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/catalog/source.md"),
        "---\nname: source\ndescription: A source.\ntype: api\nobtained: true\n\
         retrieved: 2000-01-01\nlocation:\n  - kind: url\n    \
         value: https://example.org\n---\n\n# Source\n\nProse about the source.\n",
    )
    .unwrap();
    commit(root, "catalog: a source with a declared ttl");
}

/// The second `open:` target. A source record's expiry is a finding about a file that is not
/// a node, and the question belongs on the source rather than on any one node citing it.
#[test]
fn an_expired_source_is_proposed_as_a_question_on_the_source_itself() {
    let dir = repo();
    let root = dir.path();
    with_expired_source(root);

    let (proposals, skipped) = plan_at(root, None);
    assert!(skipped.is_empty(), "{skipped:?}");
    let p = proposals
        .iter()
        .find(|p| p.check == "catalog-expired")
        .unwrap_or_else(|| panic!("no expiry proposal: {:?}", subjects(&proposals)));
    assert_eq!(p.verb, Verb::Open);
    assert_eq!(p.node, ".yidam/catalog/source.md");
    assert_eq!(p.changes[0].path(), ".yidam/catalog/source.md");
    assert!(p.carries(), "{}", p.body);
}

/// #270's gap, and the reason the carriage rule pays for itself here.
///
/// The question lands on the source, so a reader of a node resting on it would not otherwise
/// learn that its evidence aged. `catalog-expired` names those nodes in its finding, and
/// because a proposal must quote its finding verbatim, the names arrive in the commit body
/// with no code in `propose` that knows anything about citations.
#[test]
fn the_expiry_proposal_names_the_nodes_resting_on_the_source() {
    let dir = repo();
    let root = dir.path();
    with_expired_source(root);
    // A node that draws on it, which is the case the naming exists for.
    let cited = root.join(".yidam/corpus/concept/cited.yml");
    let text = std::fs::read_to_string(&cited).unwrap();
    std::fs::write(
        &cited,
        format!(
            "{text}
  Drawn from [the source](../../catalog/source.md).
"
        ),
    )
    .unwrap();
    commit(root, "revise: the concept draws on the source");

    let (proposals, _) = plan_at(root, None);
    let p = proposals
        .iter()
        .find(|p| p.check == "catalog-expired")
        .expect("an expiry proposal");
    assert!(
        p.detail.contains("concept/cited.yml"),
        "the finding names what rests on it: {}",
        p.detail
    );
    assert!(
        p.body.contains("concept/cited.yml"),
        "and carriage puts it in the message: {}",
        p.body
    );
    assert!(p.carries(), "{}", p.body);
}

/// Severity is not the licence — `catalog-expired` is Warn and never gates. What licenses the
/// proposal is the `ttl_days` this corpus declared, the same shape as
/// `withdraw_uncited_after` licensing a deletion.
#[test]
fn an_expired_source_is_proposed_although_it_never_gates() {
    let dir = repo();
    let root = dir.path();
    with_expired_source(root);

    let checks = checks_at(root);
    let expired = checks.iter().find(|c| c.id == "catalog-expired").unwrap();
    assert_eq!(expired.violations.len(), 1);
    assert!(
        !expired.gates(&expired.violations[0]),
        "the check must not gate; the declaration is what licenses the proposal"
    );
    assert!(plan_at(root, None)
        .0
        .iter()
        .any(|p| p.check == "catalog-expired"));
}

/// Refreshing the source retires the question, and the entry comes back to exactly what it
/// was — the same round trip the node case makes.
#[test]
fn refreshing_a_source_closes_the_question_and_restores_the_entry() {
    let dir = repo();
    let root = dir.path();
    with_expired_source(root);
    let before = read(root, ".yidam/catalog/source.md");

    let (first, _) = plan_at(root, None);
    let p = first.iter().find(|p| p.check == "catalog-expired").unwrap();
    let Change::Write { path, content } = &p.changes[0] else {
        panic!("an open proposal writes")
    };
    std::fs::write(root.join(path), content).unwrap();
    commit(root, "open: source — the record has aged");

    // Somebody re-fetches it. Today rather than a date far in the future: a future
    // `retrieved:` is read as *undatable* and not as fresh — deliberately, since treating it
    // as fresh would silence the entry forever — so it would leave the finding standing.
    let today = crate::cmd::export::unix_to_iso(crate::dates::today_days() as u64 * 86_400)
        .split('T')
        .next()
        .unwrap_or_default()
        .to_string();
    let refreshed = read(root, ".yidam/catalog/source.md").replace("2000-01-01", &today);
    std::fs::write(root.join(".yidam/catalog/source.md"), refreshed).unwrap();
    commit(root, "refresh: source re-fetched");

    let (second, _) = plan_at(root, None);
    let close = second
        .iter()
        .find(|p| p.verb == Verb::Close)
        .unwrap_or_else(|| panic!("no close: {:?}", subjects(&second)));
    let Change::Write { content, .. } = &close.changes[0] else {
        panic!("a close proposal writes")
    };
    assert_eq!(
        *content,
        before.replace("2000-01-01", &today),
        "closing the question did not restore the entry"
    );
}

/// A corpus that declared no TTL is asked nothing about its sources.
#[test]
fn without_a_declared_ttl_no_source_is_proposed_about() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join(".yidam/catalog/source.md"),
        "---\nname: source\ndescription: A source.\ntype: api\nobtained: true\n\
         retrieved: 2000-01-01\nlocation:\n  - kind: url\n    \
         value: https://example.org\n---\n\n# Source\n\nProse.\n",
    )
    .unwrap();
    commit(root, "catalog: a source with no ttl");
    assert!(plan_at(root, None)
        .0
        .iter()
        .all(|p| p.check != "catalog-expired"));
}

fn subjects(p: &[Proposal]) -> Vec<String> {
    p.iter()
        .map(|x| format!("{}: {}", x.verb.as_str(), x.subject))
        .collect()
}
