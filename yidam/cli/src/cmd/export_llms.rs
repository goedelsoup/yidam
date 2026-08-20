use super::export::unix_to_iso;
use crate::model::{corpus_nodes, DomainModel, NodeView};

/// Render the corpus as an llms.txt context pack: a flat plaintext file where
/// each node is a short named section, ordered to maximize information density
/// (open-question nodes first, then by outgoing link count) so that truncation
/// under a token budget keeps the most useful context.
///
/// `token_budget` caps the output at approximately `budget * 4` characters
/// (1 token ≈ 4 chars — an approximation, deliberately not a real tokenizer).
/// When the budget is exceeded mid-node the node is emitted with its
/// description cut at `[truncated]`, and a trailing `# Omitted: N nodes` line
/// accounts for everything that didn't fit.
pub fn render_llms(model: &DomainModel, token_budget: Option<usize>) -> String {
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

    let mut out = format!(
        "# {}\n# Generated: {} | Commit: {} | Nodes: {}\n\n",
        model.provenance.domain,
        unix_to_iso(model.provenance.generated_at),
        model.provenance.commit,
        nodes.len(),
    );

    let char_budget = token_budget.map(|t| t.saturating_mul(4));
    let mut omitted = 0usize;
    let mut exhausted = false;

    for node in &nodes {
        if exhausted {
            omitted += 1;
            continue;
        }
        let section = render_section(node);
        match char_budget {
            Some(budget) if out.len() + section.len() > budget => {
                // First node that overflows: emit header + label with the
                // description cut down to the remaining budget, then stop.
                out.push_str(&render_truncated(node, budget.saturating_sub(out.len())));
                exhausted = true;
            }
            _ => out.push_str(&section),
        }
    }

    if omitted > 0 {
        out.push_str(&format!("# Omitted: {omitted} nodes\n"));
    }
    out
}

fn render_section(node: &NodeView) -> String {
    let mut s = format!("## {}\n**{}**\n", node.id, node.label);
    if !node.description.is_empty() {
        s.push_str(&node.description);
        s.push('\n');
    }
    if !node.links.is_empty() {
        let targets: Vec<String> = node
            .links
            .iter()
            .map(|(target, _)| format!("[[{target}]]"))
            .collect();
        s.push_str(&format!("Links: {}\n", targets.join(", ")));
    }
    s.push_str("\n---\n\n");
    s
}

fn render_truncated(node: &NodeView, remaining: usize) -> String {
    let head = format!("## {}\n**{}**\n", node.id, node.label);
    const MARKER: &str = " [truncated]\n\n---\n\n";
    let room = remaining
        .saturating_sub(head.len())
        .saturating_sub(MARKER.len());
    let mut s = head;
    s.push_str(truncate_at_char_boundary(&node.description, room));
    s.push_str(MARKER);
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

    fn section_ids(text: &str) -> Vec<&str> {
        text.lines().filter_map(|l| l.strip_prefix("## ")).collect()
    }

    #[test]
    fn one_section_per_node_and_header_count_matches() {
        let text = render_llms(&test_model(), None);
        let ids = section_ids(&text);
        assert_eq!(ids.len(), 3);
        assert!(text.contains("# test-domain\n"));
        assert!(text.contains("| Commit: abc1234 | Nodes: 3\n"));
        assert!(text.contains("**Beta**\n"));
        assert!(text.contains("Links: [[concept/alpha]], [[concept/gamma]]\n"));
    }

    #[test]
    fn open_question_nodes_first_then_by_link_count() {
        let text = render_llms(&test_model(), None);
        assert_eq!(
            section_ids(&text),
            vec!["concept/gamma", "concept/beta", "concept/alpha"],
        );
    }

    #[test]
    fn token_budget_truncates_and_reports_omitted() {
        let model = test_model();
        let full = render_llms(&model, None);
        // A budget that admits the first section but not the second.
        let budget = (full.len() / 2) / 4;
        let text = render_llms(&model, Some(budget));

        // The budget may be exceeded by the truncated node's fixed header +
        // label + marker and the trailing omitted line — bounded, small tail.
        assert!(
            text.len() <= budget * 4 + 96,
            "output stays within budget plus the fixed truncation tail"
        );
        assert!(text.len() < full.len(), "budgeted output is shorter");
        assert!(text.contains("[truncated]"));
        let ids = section_ids(&text);
        let omitted: usize = text
            .lines()
            .find_map(|l| l.strip_prefix("# Omitted: "))
            .and_then(|rest| rest.strip_suffix(" nodes"))
            .expect("omitted line present")
            .parse()
            .unwrap();
        assert_eq!(ids.len() + omitted, 3, "sections + omitted == total nodes");
        assert_eq!(ids[0], "concept/gamma", "open node survives truncation");
    }

    #[test]
    fn generous_budget_emits_everything_without_omitted_line() {
        let text = render_llms(&test_model(), Some(100_000));
        assert_eq!(section_ids(&text).len(), 3);
        assert!(!text.contains("[truncated]"));
        assert!(!text.contains("# Omitted:"));
    }

    #[test]
    fn empty_corpus_renders_header_only() {
        let text = render_llms(&model_with(vec![]), None);
        assert!(text.contains("Nodes: 0\n"));
        assert!(section_ids(&text).is_empty());
        // Budget on an empty corpus must not panic or emit an omitted line.
        let budgeted = render_llms(&model_with(vec![]), Some(10));
        assert!(!budgeted.contains("# Omitted:"));
    }

    #[test]
    fn truncation_respects_utf8_boundaries() {
        let model = model_with(vec![instance(
            "concept",
            "uni.yml",
            "class: concept\nlabel: Uni\ndescription: \"ééééééééééééééééééééééééééééééé\"\n",
        )]);
        // Sweep budgets so the cut lands on every possible byte offset.
        for budget in 1..40 {
            let text = render_llms(&model, Some(budget));
            assert!(text.is_char_boundary(text.len()));
        }
    }
}
