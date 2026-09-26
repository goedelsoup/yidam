//! `retrieve`'s `where` (#1029): `query`'s predicate, asked of a ranked answer.
//!
//! # One predicate, two readers
//!
//! `query 'tenure[began<=1893,ended>?1893]'` and `retrieve "…" --where 'began<=1893,ended>?1893'`
//! are the same question asked two ways, and the whole of this module is keeping them the
//! same question. The text inside `[...]` is parsed by the query grammar
//! ([`lang::parse_where`]), typechecked by the query checker over one synthetic step
//! ([`check::check_where`]) — so an undeclared property, an ordering on a string, or an
//! operand the type cannot hold is rejected here with the code `query` would give — and
//! evaluated by the query evaluator's comparison half ([`exec::pred_holds_over`]). What
//! differs is only where the values come from: `query` reads the node's YAML, the vector arm
//! reads the row's `properties` column, and the keyword arm reads the node the server loaded.
//! Three readers into one comparison, rather than three comparisons.
//!
//! # Typechecked against the corpus, applied to the row
//!
//! A `where` with no `--class` narrows the way `*` does — to the classes that declare each
//! property with an ordered type — and a row of another class is out before its properties
//! are read. That is the class half of the predicate and it is pushable: the vector arm hands
//! it to [`crate::retrieval::Filter::classes`], the same way `--class` is handed to
//! [`crate::retrieval::Filter::class`], and the residual is the comparison alone.
//!
//! # A dependency's node is checked against the dependency's ontology
//!
//! The keyword arm searches every installed dependency's nodes, and a foreign node's
//! `began` is typed by the foreign corpus's `tenure.ont.yml` and nothing else. So the
//! predicate is typechecked once per corpus it may be asked of, and a dependency whose own
//! ontology rejects it — no such property, or the property is a string there — contributes
//! no rows rather than mis-typed ones. That is `query --across`'s `corpus-excluded` rule,
//! applied silently: `retrieve` has no `diagnostics` to say it in, and the alternative of
//! comparing a foreign string as a date is the wrong answer the whole check exists to refuse.

use std::collections::BTreeMap;

use super::absence::Rejection;
use super::ServerState;
use crate::cmd::query::check;
use crate::cmd::query::exec;
use crate::cmd::query::lang::{self, Pred};

/// What one corpus's ontology says about the predicate: which classes it may be asked of and
/// what type each (class, property) pair is declared as.
#[derive(Debug)]
struct Scope {
    classes: Vec<String>,
    declared: BTreeMap<(String, String), String>,
}

impl Scope {
    fn check(
        preds: &[Pred],
        class: Option<&str>,
        classes: &[crate::corpus::Class],
        universal: &crate::universal::Universal,
        nodes: &[crate::corpus::Node],
    ) -> Result<Self, check::Rejection> {
        let schema = check::Schema {
            classes,
            universal,
            authored: exec::authored(nodes),
        };
        let checked = check::check_where(preds, class, &schema)?;
        Ok(Self {
            classes: checked.narrowed.into_iter().next().unwrap_or_default(),
            declared: checked.declared,
        })
    }

    /// `None` when the class is one the predicate cannot be asked of — the row or node was
    /// never a candidate, and does not count toward the denominator an empty answer reports.
    /// `Some(holds)` when it was asked.
    fn admits(
        &self,
        class: &str,
        preds: &[Pred],
        values: impl Fn(&str) -> Vec<String>,
    ) -> Option<bool> {
        if !self.classes.iter().any(|c| c == class) {
            return None;
        }
        Some(preds.iter().all(|p| {
            let declared = self
                .declared
                .get(&(class.to_string(), p.prop.clone()))
                .map(String::as_str);
            exec::pred_holds_over(&values(&p.prop), p, declared)
        }))
    }
}

/// A `where`, parsed and typechecked, ready to be asked of rows and nodes.
#[derive(Debug)]
pub(crate) struct Bound {
    preds: Vec<Pred>,
    /// This corpus's verdict — the one the vector arm's rows are all subject to, since a
    /// local index holds one corpus by construction.
    local: Scope,
    /// Each installed dependency's verdict, by package; `None` where its ontology rejected
    /// the predicate. Only the keyword arm has foreign nodes to ask.
    foreign: BTreeMap<String, Option<Scope>>,
}

impl Bound {
    /// Parse and typecheck `text` against this corpus (and each dependency's ontology).
    ///
    /// A rejection carries the code `query` would carry for the same text — `parse`,
    /// `undeclared-property`, `unordered-property`, `unsatisfiable-predicate` — so a client
    /// branching on `rejected.code` branches once for both tools.
    pub(crate) fn parse(
        state: &ServerState,
        text: &str,
        class: Option<&str>,
    ) -> Result<Self, Rejection> {
        let preds = lang::parse_where(text).map_err(|e| Rejection {
            code: check::code::PARSE.as_str(),
            message: format!("`where` did not parse: {}", e.message),
        })?;
        let graph = &state.graph;
        let local = Scope::check(&preds, class, &graph.classes, &graph.universal, &graph.nodes)
            .map_err(|r| Rejection {
                code: r.code.as_str(),
                message: r.message,
            })?;
        let foreign = state
            .graph_across
            .iter()
            .flat_map(|g| g.across.iter())
            .map(|f| {
                let scope =
                    Scope::check(&preds, class, &f.classes, &f.universal, &f.nodes).ok();
                (f.package.clone(), scope)
            })
            .collect();
        Ok(Self {
            preds,
            local,
            foreign,
        })
    }

    /// The classes this corpus's rows may belong to — the pushable half.
    #[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
    pub(crate) fn classes(&self) -> &[String] {
        &self.local.classes
    }

    /// Does a row of the local index satisfy the predicate?
    ///
    /// `properties` is the row's `properties` column: a JSON object of its declared date and
    /// number values, or `None` for a row that has none. Absence is absence — `began>?1893`
    /// admits a row with no `began`, `began>1893` does not — exactly as over the YAML. A row
    /// of a class outside [`Self::classes`] is refused; the vector arm pushes that half into
    /// the filter, so it never reaches here.
    #[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
    pub(crate) fn admits_row(&self, class: &str, properties: Option<&str>) -> bool {
        let object: serde_json::Value = properties
            .and_then(|p| serde_json::from_str(p).ok())
            .unwrap_or(serde_json::Value::Null);
        self.local
            .admits(class, &self.preds, |prop| {
                object
                    .get(prop)
                    .map(exec::json_scalars)
                    .unwrap_or_default()
            })
            .unwrap_or(false)
    }

    /// Does a node the server loaded satisfy the predicate?
    ///
    /// Read from the parsed instance the query evaluator reads — the same file, the same
    /// scalars — so that the keyword arm and `query` cannot disagree about one node. `None`
    /// for a node the predicate was never asked of: one outside the narrowed classes, one
    /// from a dependency whose ontology rejected the predicate, or one the server loaded
    /// that the graph does not hold.
    pub(crate) fn admits_node(
        &self,
        state: &ServerState,
        view: &crate::model::NodeView,
    ) -> Option<bool> {
        let (scope, nodes, corpus_dir) = match &view.origin {
            None => (Some(&self.local), &state.graph.nodes, &state.graph.corpus_dir),
            Some(pkg) => {
                let foreign = state
                    .graph_across
                    .iter()
                    .flat_map(|g| g.across.iter())
                    .find(|f| &f.package == pkg)?;
                (
                    self.foreign.get(pkg).and_then(Option::as_ref),
                    &foreign.nodes,
                    &foreign.corpus_dir,
                )
            }
        };
        let scope = scope?;
        let wanted = format!("{}.yml", view.id);
        let node = nodes
            .iter()
            .find(|n| exec::id_of(n, corpus_dir) == wanted)?;
        scope.admits(&view.class, &self.preds, |prop| {
            exec::node_scalars(node, prop)
        })
    }
}

/// The index predates the column: nothing to compare against.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
pub(crate) fn unindexed() -> Rejection {
    Rejection {
        code: "where-unindexed",
        message: "the vector index carries no `properties` column, so a `where` has nothing to \
                  compare against. It was built before typed property columns existed: run \
                  `yidam embed && yidam index-build` and ask again."
            .to_string(),
    }
}

/// A remote index carries no properties, by design — see `index_push`.
#[cfg(all(feature = "s3-vectors", feature = "vector-read"))]
pub(crate) fn remote() -> Rejection {
    Rejection {
        code: "where-remote",
        message: "this corpus is served out of a remote vector index, which carries no \
                  `properties` column — `yidam index-push` does not forward it — so a `where` \
                  cannot be answered here. Ask `query`, whose predicate reads the corpus."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::serve::tests::test_state;

    fn state_with_ordered() -> ServerState {
        test_state()
    }

    /// The test state declares no ordered property, so every `where` is `undeclared-property`
    /// there — with the code `query` gives for the same text, which is the whole claim.
    #[test]
    fn an_undeclared_property_is_rejected_with_the_query_code() {
        let state = state_with_ordered();
        let err = Bound::parse(&state, "began>1893", None).unwrap_err();
        assert_eq!(err.code, "undeclared-property");
    }

    #[test]
    fn a_parse_failure_is_the_parse_code() {
        let state = state_with_ordered();
        let err = Bound::parse(&state, "", None).unwrap_err();
        assert_eq!(err.code, "parse");
        let err = Bound::parse(&state, "began>1893,,ended", None).unwrap_err();
        assert_eq!(err.code, "parse");
    }
}
