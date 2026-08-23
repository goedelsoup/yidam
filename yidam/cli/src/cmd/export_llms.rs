use std::collections::BTreeMap;

use super::export::unix_to_iso;
use crate::model::{corpus_nodes, DomainModel, NodeView};

/// Result of rendering an llms.txt context pack: the text plus an honest account
/// of what it contains, so a caller can report what it wrote rather than what it
/// was given.
pub struct LlmsPack {
    pub text: String,
    /// Nodes in the corpus.
    pub total: usize,
    /// Nodes present in `text` as a section (whether full prose or label-only).
    pub written: usize,
    /// Written nodes whose description was dropped to fit the budget.
    pub elided: usize,
    /// Nodes with no section at all, by class.
    pub omitted_by_class: BTreeMap<String, usize>,
}

impl LlmsPack {
    /// Nodes that did not make it into the pack at all.
    pub fn omitted(&self) -> usize {
        self.omitted_by_class.values().sum()
    }
}

/// Render the corpus as an llms.txt context pack: a flat plaintext file where
/// each node is a short named section, ordered to maximize information density
/// (open-question nodes first, then by outgoing link count).
///
/// `token_budget` caps the output at approximately `budget * 4` characters
/// (1 token ≈ 4 chars — an approximation, deliberately not a real tokenizer).
/// A budget degrades **coverage before membership**: prose is dropped first, and
/// membership — when it has to give — is spent round-robin across classes, so a
/// pack keeps at least one node of every class for as long as the budget admits
/// one. Whatever is still lost is named in the trailer, per class, rather than
/// vanishing.
pub fn render_llms(model: &DomainModel, token_budget: Option<usize>) -> LlmsPack {
    let nodes = sorted_nodes(model);
    let total = nodes.len();

    // The unbudgeted pack, which is also the answer whenever a budget is generous
    // enough to hold it — no reserve arithmetic, no trailer.
    let full_text = render_all(model, &nodes, total, None);
    let char_budget = match token_budget {
        None => {
            return pack(full_text, total, total, 0, BTreeMap::new());
        }
        Some(t) => t.saturating_mul(4),
    };
    if full_text.len() <= char_budget {
        return pack(full_text, total, total, 0, BTreeMap::new());
    }

    // Everything the trailer and header could cost at their longest, held back so
    // the accounting itself cannot push the pack over the budget it reports.
    let class_totals = class_totals(&nodes);
    let reserve =
        header(model, total, total, token_budget).len() + trailer(&class_totals, total).len();
    let room = char_budget.saturating_sub(reserve);

    let compact: Vec<usize> = nodes.iter().map(|n| render_compact(n).len()).collect();
    let (selected, mut used) = select_membership(&nodes, &compact, room);
    let (detail, elided) = select_detail(&nodes, &compact, &selected, room, &mut used);

    let written = selected.iter().filter(|s| **s).count();
    let mut omitted_by_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut body = String::new();
    for (i, node) in nodes.iter().enumerate() {
        if !selected[i] {
            *omitted_by_class.entry(node.class.clone()).or_default() += 1;
            continue;
        }
        match detail[i] {
            Detail::Full => body.push_str(&render_section(node)),
            Detail::Truncated(room) => body.push_str(&render_truncated(node, room)),
            Detail::LabelOnly => body.push_str(&render_compact(node)),
        }
    }

    let mut text = header(model, written, total, token_budget);
    text.push_str(&body);
    text.push_str(&trailer(&omitted_by_class, elided));
    pack(text, total, written, elided, omitted_by_class)
}

fn pack(
    text: String,
    total: usize,
    written: usize,
    elided: usize,
    omitted_by_class: BTreeMap<String, usize>,
) -> LlmsPack {
    LlmsPack {
        text,
        total,
        written,
        elided,
        omitted_by_class,
    }
}

fn sorted_nodes(model: &DomainModel) -> Vec<NodeView> {
    let mut nodes = corpus_nodes(model);
    // Open questions sort first, so the fields that decide "open" have to be loaded before
    // the comparison rather than inside it.
    let fields = crate::paths::repo_root()
        .map(|r| crate::claims::ClaimFields::load(&crate::paths::yidam_corpus_dir(&r)))
        .unwrap_or_default();
    nodes.sort_by(|a, b| {
        let open_a =
            crate::claims::is_open_question(&a.label, &a.content, fields.for_class(&a.class));
        let open_b =
            crate::claims::is_open_question(&b.label, &b.content, fields.for_class(&b.class));
        open_b
            .cmp(&open_a)
            .then_with(|| b.links.len().cmp(&a.links.len()))
            .then_with(|| a.id.cmp(&b.id))
    });
    nodes
}

fn render_all(
    model: &DomainModel,
    nodes: &[NodeView],
    total: usize,
    budget: Option<usize>,
) -> String {
    let mut out = header(model, total, total, budget);
    for node in nodes {
        out.push_str(&render_section(node));
    }
    out
}

fn class_totals(nodes: &[NodeView]) -> BTreeMap<String, usize> {
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for node in nodes {
        *totals.entry(node.class.clone()).or_default() += 1;
    }
    totals
}

/// Which nodes get a section at all. Classes take turns — every class places its
/// first node before any class places its second — so a budget that admits `n`
/// nodes spreads them over the ontology instead of handing all `n` to whichever
/// class happens to sort first.
fn select_membership(nodes: &[NodeView], compact: &[usize], room: usize) -> (Vec<bool>, usize) {
    // Classes in order of their highest-priority node, each holding its own node
    // indices in that same priority order.
    let mut order: Vec<String> = Vec::new();
    let mut by_class: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        let entry = by_class.entry(node.class.clone()).or_default();
        if entry.is_empty() {
            order.push(node.class.clone());
        }
        entry.push(i);
    }

    let mut selected = vec![false; nodes.len()];
    let mut used = 0usize;
    let mut round = 0usize;
    loop {
        let mut any = false;
        for class in &order {
            let Some(&i) = by_class[class].get(round) else {
                continue;
            };
            any = true;
            if used + compact[i] <= room {
                selected[i] = true;
                used += compact[i];
            }
        }
        if !any {
            break;
        }
        round += 1;
    }
    (selected, used)
}

enum Detail {
    Full,
    /// Description cut to this many bytes, marked `[truncated]`.
    Truncated(usize),
    LabelOnly,
}

/// How much prose each selected node gets. Priority order spends what is left
/// after membership: full descriptions until one does not fit, one truncated
/// description at the boundary if there is meaningful room, label-only after.
fn select_detail(
    nodes: &[NodeView],
    compact: &[usize],
    selected: &[bool],
    room: usize,
    used: &mut usize,
) -> (Vec<Detail>, usize) {
    /// Below this there is no prose worth the `[truncated]` marker.
    const MIN_PROSE: usize = 64;

    let mut detail: Vec<Detail> = nodes.iter().map(|_| Detail::LabelOnly).collect();
    let mut spending = true;
    for (i, node) in nodes.iter().enumerate() {
        if !selected[i] {
            continue;
        }
        if !spending || node.description.is_empty() {
            continue;
        }
        let extra = render_section(node).len().saturating_sub(compact[i]);
        if *used + extra <= room {
            detail[i] = Detail::Full;
            *used += extra;
            continue;
        }
        // The boundary node: give it whatever prose is left, then stop upgrading
        // so the spend stays in priority order rather than skipping to the next
        // description that happens to be short.
        spending = false;
        let left = room.saturating_sub(*used);
        if left >= MIN_PROSE {
            detail[i] = Detail::Truncated(compact[i] + left);
            *used += left;
        }
    }
    let elided = nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| {
            selected[*i] && !n.description.is_empty() && matches!(detail[*i], Detail::LabelOnly)
        })
        .count();
    (detail, elided)
}

fn header(model: &DomainModel, written: usize, total: usize, budget: Option<usize>) -> String {
    let count = if written == total {
        format!("Nodes: {total}")
    } else {
        format!("Nodes: {written} of {total}")
    };
    let budget_note = match budget {
        Some(b) => format!(" | Token budget: {b}"),
        None => String::new(),
    };
    format!(
        "# {}\n# Generated: {} | Commit: {} | {}{}\n\n",
        model.provenance.domain,
        unix_to_iso(model.provenance.generated_at),
        model.provenance.commit,
        count,
        budget_note,
    )
}

/// The account of what the budget cost, per class. A pack that silently holds a
/// slice is indistinguishable from one that holds the corpus; this is the line
/// that distinguishes them.
fn trailer(omitted_by_class: &BTreeMap<String, usize>, elided: usize) -> String {
    let mut s = String::new();
    let omitted: usize = omitted_by_class.values().sum();
    if omitted > 0 {
        let breakdown: Vec<String> = omitted_by_class
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(class, n)| format!("{class}: {n}"))
            .collect();
        s.push_str(&format!(
            "# Omitted: {omitted} nodes ({})\n",
            breakdown.join(", ")
        ));
    }
    if elided > 0 {
        s.push_str(&format!(
            "# Elided: {elided} descriptions (label and links kept)\n"
        ));
    }
    s
}

fn render_section(node: &NodeView) -> String {
    let mut s = format!("## {}\n**{}**\n", node.id, node.label);
    if !node.description.is_empty() {
        s.push_str(&node.description);
        s.push('\n');
    }
    s.push_str(&render_links(node));
    s.push_str("\n---\n\n");
    s
}

/// A node with its prose dropped: still names itself, still shows its edges.
fn render_compact(node: &NodeView) -> String {
    let mut s = format!("## {}\n**{}**\n", node.id, node.label);
    s.push_str(&render_links(node));
    s.push_str("\n---\n\n");
    s
}

fn render_links(node: &NodeView) -> String {
    if node.links.is_empty() {
        return String::new();
    }
    let targets: Vec<String> = node
        .links
        .iter()
        .map(|(target, _)| format!("[[{target}]]"))
        .collect();
    format!("Links: {}\n", targets.join(", "))
}

fn render_truncated(node: &NodeView, budget: usize) -> String {
    const MARKER: &str = " [truncated]\n";
    let head = format!("## {}\n**{}**\n", node.id, node.label);
    let tail = format!("{}\n---\n\n", render_links(node));
    let room = budget
        .saturating_sub(head.len())
        .saturating_sub(tail.len())
        .saturating_sub(MARKER.len());
    let mut s = head;
    s.push_str(truncate_at_char_boundary(&node.description, room));
    s.push_str(MARKER);
    s.push_str(&tail);
    s
}

/// Cut `s` to at most `max_bytes`, backing off to a UTF-8 char boundary.
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DomainModel, InstanceFile, Provenance, RenderedViews};

    fn model_with(instances: Vec<InstanceFile>) -> DomainModel {
        DomainModel {
            classes: vec![],
            instances,
            skills: vec![],
            decisions: vec![],
            index: None,
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "test-domain".into(),
                generated_at: 0,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    fn instance(class: &str, filename: &str, yaml: &str) -> InstanceFile {
        InstanceFile {
            class: class.into(),
            filename: filename.into(),
            content: yaml.as_bytes().to_vec(),
        }
    }

    fn test_model() -> DomainModel {
        model_with(vec![
            // No links, no open claims — should sort last.
            instance(
                "concept",
                "alpha.yml",
                "class: concept\nlabel: Alpha\ndescription: First node\nlinks: []\n",
            ),
            // Two links — highest connectivity among the closed nodes.
            instance(
                "concept",
                "beta.yml",
                "class: concept\nlabel: Beta\ndescription: Second node\n\
                 links:\n  - target: alpha.yml\n    relationship: causes\n\
                 \x20 - target: gamma.yml\n    relationship: link\n",
            ),
            // One link but carries an open claim — must come first.
            instance(
                "concept",
                "gamma.yml",
                "class: concept\nlabel: Gamma\ndescription: Third node [open] unresolved\n\
                 links:\n  - target: alpha.yml\n    relationship: link\n",
            ),
        ])
    }

    /// One `question` node that sorts ahead of everything, then many `concept`
    /// nodes and a handful of `relation` nodes — the shape that used to hand a
    /// small budget an all-one-class prefix.
    fn multi_class_model() -> DomainModel {
        let mut instances = vec![instance(
            "question",
            "q.yml",
            "class: question\nlabel: Q\ndescription: An open question here [open]\n",
        )];
        for i in 0..12 {
            instances.push(instance(
                "concept",
                &format!("c{i}.yml"),
                &format!(
                    "class: concept\nlabel: C{i}\ndescription: {}\n",
                    "concept prose ".repeat(8)
                ),
            ));
        }
        for i in 0..3 {
            instances.push(instance(
                "relation",
                &format!("r{i}.yml"),
                &format!(
                    "class: relation\nlabel: R{i}\ndescription: {}\n",
                    "relation prose ".repeat(8)
                ),
            ));
        }
        model_with(instances)
    }

    fn section_ids(text: &str) -> Vec<&str> {
        text.lines().filter_map(|l| l.strip_prefix("## ")).collect()
    }

    fn classes_present(text: &str) -> std::collections::BTreeSet<&str> {
        section_ids(text)
            .into_iter()
            .filter_map(|id| id.split('/').next())
            .collect()
    }

    #[test]
    fn one_section_per_node_and_header_count_matches() {
        let pack = render_llms(&test_model(), None);
        let ids = section_ids(&pack.text);
        assert_eq!(ids.len(), 3);
        assert_eq!((pack.written, pack.total, pack.omitted()), (3, 3, 0));
        assert!(pack.text.contains("# test-domain\n"));
        assert!(pack.text.contains("| Commit: abc1234 | Nodes: 3\n"));
        assert!(pack.text.contains("**Beta**\n"));
        assert!(pack
            .text
            .contains("Links: [[concept/alpha]], [[concept/gamma]]\n"));
    }

    #[test]
    fn open_question_nodes_first_then_by_link_count() {
        let text = render_llms(&test_model(), None).text;
        assert_eq!(
            section_ids(&text),
            vec!["concept/gamma", "concept/beta", "concept/alpha"],
        );
    }

    #[test]
    fn budget_drops_prose_before_it_drops_nodes() {
        let model = multi_class_model();
        let full = render_llms(&model, None);
        // Half the full size: far too small for every description, roomy enough
        // for every label.
        let pack = render_llms(&model, Some(full.text.len() / 2 / 4));

        assert_eq!(pack.written, pack.total, "membership survives");
        assert_eq!(pack.omitted(), 0);
        assert!(pack.elided > 0, "prose is what paid for the budget");
        assert!(pack.text.contains("# Elided: "));
        assert!(!pack.text.contains("# Omitted: "));
        // Every node still names itself and its edges.
        assert_eq!(section_ids(&pack.text).len(), pack.total);
    }

    #[test]
    fn membership_when_it_must_give_is_spread_across_classes() {
        let model = multi_class_model();
        // Room for only a few nodes even label-only.
        let pack = render_llms(&model, Some(120));

        assert!(pack.written < pack.total, "this budget must drop nodes");
        assert!(pack.written >= 3, "but not below one node per class");
        assert_eq!(
            classes_present(&pack.text),
            ["concept", "question", "relation"].into_iter().collect(),
            "every class is represented",
        );
    }

    #[test]
    fn omitted_nodes_are_named_by_class_in_the_trailer() {
        let model = multi_class_model();
        let pack = render_llms(&model, Some(120));

        let line = pack
            .text
            .lines()
            .find(|l| l.starts_with("# Omitted: "))
            .expect("trailer accounts for the drop");
        assert!(line.contains(&format!("# Omitted: {} nodes", pack.omitted())));
        for (class, n) in &pack.omitted_by_class {
            assert!(line.contains(&format!("{class}: {n}")), "{line}");
        }
        assert_eq!(
            section_ids(&pack.text).len() + pack.omitted(),
            pack.total,
            "sections + omitted == total nodes",
        );
        assert!(pack
            .text
            .contains(&format!("Nodes: {} of {}", pack.written, pack.total)));
    }

    #[test]
    fn budgeted_output_stays_within_its_budget() {
        let model = multi_class_model();
        for budget in [60usize, 120, 400, 1_000, 2_000] {
            let pack = render_llms(&model, Some(budget));
            assert!(
                pack.text.len() <= budget * 4,
                "budget {budget}: {} chars",
                pack.text.len(),
            );
        }
    }

    #[test]
    fn the_boundary_node_keeps_truncated_prose() {
        let model = multi_class_model();
        let full = render_llms(&model, None);
        // Between "every label fits" and "every description fits" there is a node
        // whose prose only partly fits.
        let found = (1..=full.text.len() / 4)
            .step_by(7)
            .any(|b| render_llms(&model, Some(b)).text.contains("[truncated]"));
        assert!(found, "some budget lands mid-description");
    }

    #[test]
    fn generous_budget_emits_everything_without_a_trailer() {
        let pack = render_llms(&test_model(), Some(100_000));
        assert_eq!(section_ids(&pack.text).len(), 3);
        assert_eq!((pack.elided, pack.omitted()), (0, 0));
        assert!(!pack.text.contains("[truncated]"));
        assert!(!pack.text.contains("# Omitted:"));
        assert!(!pack.text.contains("# Elided:"));
        // A budget that fits is invisible: same bytes as no budget at all.
        assert_eq!(pack.text, render_llms(&test_model(), None).text);
    }

    #[test]
    fn empty_corpus_renders_header_only() {
        let pack = render_llms(&model_with(vec![]), None);
        assert!(pack.text.contains("Nodes: 0\n"));
        assert!(section_ids(&pack.text).is_empty());
        // Budget on an empty corpus must not panic or emit a trailer.
        let budgeted = render_llms(&model_with(vec![]), Some(10));
        assert!(!budgeted.text.contains("# Omitted:"));
        assert_eq!(budgeted.total, 0);
    }

    #[test]
    fn truncation_respects_utf8_boundaries() {
        let model = model_with(vec![instance(
            "concept",
            "uni.yml",
            "class: concept\nlabel: Uni\ndescription: \"ééééééééééééééééééééééééééééééé\"\n",
        )]);
        // Sweep budgets so the cut lands on every possible byte offset.
        for budget in 1..80 {
            let text = render_llms(&model, Some(budget)).text;
            assert!(text.is_char_boundary(text.len()));
        }
    }
}
