use anyhow::Result;
use serde::Serialize;

use crate::git::head_commit_short;
use crate::parse::{frontmatter_body, parse_frontmatter};
use crate::paths::{
    class_of_path, repo_root, yidam_catalog_dir, yidam_corpus_dir, yidam_embeddings_dir,
};
use crate::s3vectors::request;
use crate::walk::walk_md_files;

/// The `corpus` a dry run measures with when the repository cannot see its own root commit.
///
/// A real push refuses that repository outright — its keys would not be stable — but a
/// measurement can still be made, and the field's *length* is what the ceiling counts. Twelve
/// characters, the same as a real one, so the number does not move.
const UNKNOWN_CORPUS: &str = "000000000000";

/// The two values [`EmbedRecord::kind`] takes. Named because a dry run partitions its report
/// on them and a typo would silently file every source under `node`.
const KIND_NODE: &str = "node";
const KIND_SOURCE: &str = "source";

/// What `yidam embed` reads.
#[derive(Debug, Clone)]
pub struct EmbedOptions {
    /// Walk `.yidam/catalog/` as well as the corpus. On by default.
    ///
    /// It is on by default because leaving it off was a silent scope decision. A derived
    /// repository measured what this command would index for it and found **41.9%** — the
    /// corpus nodes — against **51.3%** sitting in the catalog and never walked at all. It
    /// declined to route through `yidam embed` for exactly that reason: the cut would have
    /// happened as a side effect of a tool boundary rather than as anyone's decision.
    ///
    /// The flag exists because a repository whose catalog holds material it does not want
    /// retrievable should be able to say so, once, rather than by not knowing.
    pub catalog: bool,
    /// Compose every record, write none of them, and report what the composed text would
    /// cost as a remote vector's metadata.
    ///
    /// It exists because the question RFC-0033 §8 left open — whether the 40 KB metadata
    /// ceiling is comfortable for real corpora — can only be answered by the thing that
    /// composes the text, and the corpora worth asking it of are not ours to write into.
    /// `text` is not a field: it is [`compose_text`] over a label, every declared prose
    /// field and the edge names, or [`compose_source_text`] over a catalog entry's whole
    /// markdown body. A grep under-measures both.
    pub dry_run: bool,
}

impl Default for EmbedOptions {
    fn default() -> Self {
        Self {
            catalog: true,
            dry_run: false,
        }
    }
}

#[derive(Serialize)]
pub struct EmbedRecord {
    pub path: String,
    /// For a node, its ontology class. For a source, the catalog `type`
    /// (`paper`/`dataset`/`api`/`database`), or `source` when it declares none.
    pub class: String,
    pub label: String,
    pub text: String,
    pub commit: String,
    /// `node` or `source`. Consumers that only want the corpus can filter on it; older
    /// ones ignore it, which is why the split is a new field rather than a changed `class`.
    pub kind: String,
    /// What this corpus's calculators worked out about this node — see [`crate::computed`].
    ///
    /// **Additive, and absent where there is none.** `skip_serializing_if` is not tidiness: a
    /// corpus with no `.yidam/computed/` writes byte-identical records to the ones it wrote
    /// before this field existed, so adopting the reader does not invalidate an index it has
    /// already built. A corpus that *does* hold computed files sees its records change, which
    /// is the change rather than a side effect of it — the same rule `retrievable` states about
    /// a corpus that flags a property.
    ///
    /// Not in `text`, and that is the decision. Concatenating `verified` into the embedded
    /// prose would make a tier a thing a sentence embedding is nudged by rather than a thing a
    /// query can *filter on*, and it would move every existing node's vector. A signal is
    /// structured metadata about the node; it is carried as structure.
    ///
    /// A source record never carries one: a signal is keyed to a corpus node, and a catalog
    /// entry is not one.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub signals: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Compose the embedding target text for a catalog entry.
///
/// The body carries the substance — what the source holds, what was retrieved from it,
/// what it does not answer — so it is the bulk of what is worth retrieving on. Location
/// *descriptions* are included and location URLs are not: a URL contributes no meaning to
/// a sentence embedding and dilutes the vector it sits in.
fn compose_source_text(
    name: &str,
    description: &str,
    kind: &str,
    locations: &[crate::parse::CatalogLocation],
    body: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !name.is_empty() {
        parts.push(name.to_string());
    }
    if !kind.is_empty() {
        parts.push(format!("A {kind} source."));
    }
    if !description.is_empty() {
        parts.push(description.to_string());
    }
    for loc in locations {
        if let Some(d) = loc.description.as_deref() {
            let d = d.trim();
            if !d.is_empty() {
                parts.push(d.to_string());
            }
        }
    }
    let body = body.trim();
    if !body.is_empty() {
        parts.push(body.to_string());
    }
    parts.join(" ")
}

/// Compose the embedding target text for a corpus instance.
/// Combines label, description, and relationship hints into a single string.
fn compose_text(label: &str, description: &str, links: &[crate::parse::CorpusLink]) -> String {
    let relationships: Vec<String> = links
        .iter()
        .filter_map(|l| l.target.as_deref())
        .filter(|t| !t.ends_with(".ont.yml"))
        .filter_map(|t| {
            std::path::Path::new(t)
                .file_stem()
                .map(|n| n.to_string_lossy().replace('-', " "))
        })
        .collect();

    let mut parts: Vec<String> = Vec::new();
    if !label.is_empty() {
        parts.push(label.to_string());
    }
    if !description.is_empty() {
        parts.push(description.to_string());
    }
    if !relationships.is_empty() {
        parts.push(format!("Related: {}.", relationships.join(", ")));
    }
    parts.join(" ")
}

/// What the composed records would cost as one remote vector's metadata each.
///
/// Measured with [`request::metadata`] itself and not by adding up field
/// lengths: the ceiling is on a JSON body, and a body's length is its escaping as much as its
/// content — a node whose prose is full of quotes is larger on the wire than `text.len()`.
#[derive(Default)]
struct Footprints {
    /// Composed `text` bytes, before any cut, and whether the record was a node or a source.
    ///
    /// Split because the two are composed by different functions over different material —
    /// a node's declared prose against a catalog entry's whole markdown body — and the
    /// question of whether a ceiling binds is answered differently for each. A merged figure
    /// would hide which half is near it.
    text: Vec<(usize, bool)>,
    /// Whole-row metadata bytes, as the request renders them.
    row: Vec<usize>,
    /// The largest whole row seen, and whose it was. The figure the 40 KB ceiling is on —
    /// larger than any `text` by the other four fields and by JSON escaping.
    row_max: (usize, String),
    /// The largest filterable half seen, and whose it was.
    filterable_max: (usize, String),
    /// Rows whose text did not fit: path, composed bytes, bytes kept.
    truncated: Vec<(String, usize, usize)>,
    /// Rows a push would refuse outright, and why.
    refused: Vec<(String, String)>,
}

/// The bucket edges the row histogram counts into, in bytes.
///
/// Counts rather than percentiles because counts **merge**: a figure computed per repository
/// can be added across a set of them, and a median cannot. The cross-corpus number RFC-0033
/// §8 wanted is the sum of these columns.
const ROW_BUCKETS: &[usize] = &[
    256,
    1024,
    4 * 1024,
    16 * 1024,
    crate::s3vectors::MAX_METADATA_BYTES,
];

impl Footprints {
    fn observe(&mut self, corpus: &str, commit: &str, rec: &EmbedRecord) {
        let text_bytes = rec.text.len();
        self.text.push((text_bytes, rec.kind == KIND_NODE));
        match request::metadata(corpus, &rec.class, &rec.label, commit, &rec.text) {
            Ok((m, cut)) => {
                let bytes = serde_json::to_vec(&m).map_or(0, |b| b.len());
                self.row.push(bytes);
                if bytes > self.row_max.0 {
                    self.row_max = (bytes, rec.path.clone());
                }
                let filterable = request::filterable_len(&m);
                if filterable > self.filterable_max.0 {
                    self.filterable_max = (filterable, rec.path.clone());
                }
                if cut {
                    let kept = m[crate::s3vectors::META_KEY_TEXT]
                        .as_str()
                        .map_or(0, str::len);
                    self.truncated.push((rec.path.clone(), text_bytes, kept));
                }
            }
            // Recorded rather than propagated: a push refuses the whole run on the first bad
            // row, which is right for a push and wrong for a measurement — the point of
            // running this is to find out how many there are.
            Err(e) => self.refused.push((rec.path.clone(), e.to_string())),
        }
    }

    fn report(&self, corpus: &str, nodes: usize, sources: usize, skipped: usize) {
        let ceiling = crate::s3vectors::MAX_METADATA_BYTES;
        let filterable_ceiling = crate::s3vectors::MAX_FILTERABLE_METADATA_BYTES;
        println!("\nmetadata footprint — what `yidam index-push` would put on the wire");
        println!("  corpus:      {corpus}");
        println!(
            "  rows:        {} ({nodes} node, {sources} source{})",
            self.text.len(),
            if skipped > 0 {
                format!("; {skipped} unparseable and skipped")
            } else {
                String::new()
            }
        );
        if self.text.is_empty() {
            return;
        }
        for (label, is_node) in [("node ", true), ("source", false)] {
            let mut text: Vec<usize> = self
                .text
                .iter()
                .filter(|(_, n)| *n == is_node)
                .map(|(b, _)| *b)
                .collect();
            if text.is_empty() {
                continue;
            }
            text.sort_unstable();
            println!(
                "  {label} text: min {}  p50 {}  p90 {}  p99 {}  max {}  total {}",
                text[0],
                quantile(&text, 50),
                quantile(&text, 90),
                quantile(&text, 99),
                text[text.len() - 1],
                text.iter().sum::<usize>(),
            );
        }

        println!("  row bytes against the {ceiling}-byte ceiling:");
        let total = self.row.len().max(1);
        let mut lower = 0usize;
        for &edge in ROW_BUCKETS {
            let n = self.row.iter().filter(|&&b| b > lower && b <= edge).count();
            println!(
                "    {:>6} < b ≤ {:>6}   {:>6}  {:>5.1}%",
                lower,
                edge,
                n,
                100.0 * n as f64 / total as f64
            );
            lower = edge;
        }
        let over = self.row.iter().filter(|&&b| b > ceiling).count();
        println!(
            "    {:>6} < b            {:>6}  {:>5.1}%",
            ceiling,
            over,
            100.0 * over as f64 / total as f64
        );
        // Zero by construction — `metadata` cuts until the row fits — and printed anyway: a
        // reader should be able to see that the bucket was counted rather than assumed, and
        // `embed_dry_run.rs` holds it to zero over every example corpus.

        println!(
            "  largest row: {} of {ceiling} ({})",
            self.row_max.0,
            if self.row_max.1.is_empty() {
                "—"
            } else {
                &self.row_max.1
            }
        );
        println!(
            "  filterable:  max {} of {filterable_ceiling} ({})",
            self.filterable_max.0,
            if self.filterable_max.1.is_empty() {
                "—"
            } else {
                &self.filterable_max.1
            }
        );
        println!(
            "  truncated:   {} row(s) would have their text cut",
            self.truncated.len()
        );
        for (path, composed, kept) in self.truncated.iter().take(10) {
            println!("    - {path}: {composed} composed, {kept} kept");
        }
        if self.truncated.len() > 10 {
            println!("    … and {} more", self.truncated.len() - 10);
        }
        if !self.refused.is_empty() {
            println!(
                "  refused:     {} row(s) a push would not send",
                self.refused.len()
            );
            for (path, why) in self.refused.iter().take(10) {
                println!("    - {path}: {why}");
            }
        }
    }
}

/// The `p`th percentile of a sorted slice, by nearest rank. Empty is not a case: every caller
/// has returned already.
fn quantile(sorted: &[usize], p: usize) -> usize {
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

pub fn embed(opts: EmbedOptions) -> Result<()> {
    let root = repo_root()?;
    let corpus_dir = yidam_corpus_dir(&root);
    let embeddings_dir = yidam_embeddings_dir(&root);
    let commit = head_commit_short(&root);

    let catalog_dir = yidam_catalog_dir(&root);
    let sources = if opts.catalog {
        walk_md_files(&catalog_dir)
    } else {
        vec![]
    };

    let prose_fields = crate::prose::ProseFields::load(&corpus_dir);
    let retrievable = crate::retrievable::Retrievable::load(&corpus_dir);
    // What this corpus's calculators worked out, read once for the whole walk (#1028). Before
    // this there was no reader at all: a calculator could commit a derived quantity under the
    // orchestrator's guarantees and nothing in the toolkit could see the file it landed in.
    //
    // Its `problems` are printed here rather than raised, because a malformed computed file is
    // not a reason to refuse to embed a corpus, and `yidam doctor` is where the question is
    // asked as a question.
    let signals = crate::computed::Signals::load(&root);
    for problem in &signals.problems {
        eprintln!("[warn] {problem}");
    }
    // One read of the corpus, where this loop used to be the sixth place with its own copy
    // of read-then-`serde_yaml::from_str` (#925). A malformed file is still skipped with a
    // warning: `Node::malformed` carries the same `serde_yaml` message this printed.
    let read = crate::corpus::Corpus::open(&root);
    let instances = read.nodes();
    if instances.is_empty() && sources.is_empty() {
        println!("No corpus instances found in {}.", corpus_dir.display());
        return Ok(());
    }

    // A dry run composes everything and writes nothing — not the directory either. These
    // corpora are read, not ours to leave an empty `.yidam/embeddings/` behind in.
    if !opts.dry_run {
        std::fs::create_dir_all(&embeddings_dir)?;
    }
    // Only a dry run asks: `genesis_hash` shells out to git, and a plain `embed` has never
    // needed the corpus identity to write a record.
    let corpus = if opts.dry_run {
        crate::git::genesis_hash(&root)
            .as_deref()
            .map_or_else(|| UNKNOWN_CORPUS.to_string(), request::corpus_id)
    } else {
        String::new()
    };
    let mut footprints = Footprints::default();

    let mut count = 0;
    let mut skipped = 0usize;
    for node in instances {
        let path = &node.path;
        if let Some(e) = &node.malformed {
            eprintln!("[warn] skipping {}: {e}", node.rel);
            skipped += 1;
            continue;
        }
        let inst = &node.inst;

        // `crate::paths::class_of_path` and not a copy of its body: this value is written
        // into every index row and every remote vector, and the query path recomputes the
        // same rule from the file on disk. Two functions were what RFC-0033 §4.5 declined to
        // push a class filter on.
        let class = class_of_path(path);

        let label = inst.label.as_deref().unwrap_or("").to_string();
        // Every declared prose field, and since #746 the properties a class flagged too. A
        // node whose substance is in `summary` and `findings` was embedded on its label and
        // its edge names alone — retrievable by title and by nothing it says. The same was
        // true of `properties.method`, a block scalar on 341 nodes across 214 KB in the
        // measured corpora, none of which was in any embedding.
        //
        // Since #717 it is `retrievable::text` and not `prose::text`, because this is the one
        // reader for which prose was nearly the right question and not the same one. A gage's
        // `parameter: "00060"` and `units: cubic feet per second` are not prose — flagging
        // them as prose would make `node-too-long` count the code and `missing-description`
        // accept it — and they are exactly the strings a query is typed in. `prose::text` is
        // still what the two checks read, and the union of the two axes is taken there rather
        // than here, so this callsite cannot ask for one and silently miss the other.
        //
        // Node text is what an index is built from, so a corpus that flags a property must
        // re-embed. That is the change rather than a side effect of it.
        let description = crate::retrievable::text(inst, &prose_fields, &retrievable, &class);
        let links = inst.links.as_deref().unwrap_or(&[]);
        let text = compose_text(&label, &description, links);

        let record = EmbedRecord {
            path: node.rel.clone(),
            class: class.clone(),
            label,
            text,
            commit: commit.clone(),
            kind: KIND_NODE.to_string(),
            signals: signals.for_node(&node.rel),
        };

        if opts.dry_run {
            footprints.observe(&corpus, &commit, &record);
        } else {
            let out_dir = embeddings_dir.join(&class);
            std::fs::create_dir_all(&out_dir)?;
            let json_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
                + ".json";
            std::fs::write(
                out_dir.join(&json_name),
                serde_json::to_string_pretty(&record)?,
            )?;
        }
        count += 1;
    }

    // ── Catalog ──────────────────────────────────────────────────────────────
    //
    // Written under `_catalog/` rather than `catalog/`: the sibling directories here are
    // named after ontology classes, and the leading underscore is what keeps a domain that
    // happens to define a `catalog` class from colliding with this.
    let mut source_count = 0usize;
    for path in &sources {
        let text = std::fs::read_to_string(path)?;
        let fm = parse_frontmatter(&text);
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let name = fm.name.clone().unwrap_or_else(|| stem.replace('-', " "));
        let source_type = fm.r#type.clone().unwrap_or_default();
        let composed = compose_source_text(
            &name,
            fm.description.as_deref().unwrap_or(""),
            &source_type,
            fm.location.as_deref().unwrap_or(&[]),
            frontmatter_body(&text),
        );
        if composed.trim().is_empty() {
            continue;
        }
        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let record = EmbedRecord {
            path: rel_path.to_string_lossy().to_string(),
            class: if source_type.is_empty() {
                "source".to_string()
            } else {
                source_type
            },
            label: name,
            text: composed,
            commit: commit.clone(),
            kind: KIND_SOURCE.to_string(),
            signals: Default::default(),
        };
        if opts.dry_run {
            footprints.observe(&corpus, &commit, &record);
        } else {
            let out_dir = embeddings_dir.join("_catalog");
            std::fs::create_dir_all(&out_dir)?;
            std::fs::write(
                out_dir.join(format!("{stem}.json")),
                serde_json::to_string_pretty(&record)?,
            )?;
        }
        source_count += 1;
    }

    if signals.nodes() > 0 {
        println!(
            "  {} node(s) carry computed signal(s): {}",
            signals.nodes(),
            signals.names().join(", ")
        );
    }
    if source_count > 0 {
        println!("  {source_count} catalog source(s)");
    } else if !opts.catalog {
        println!("  catalog skipped (--no-catalog)");
    }

    if opts.dry_run {
        footprints.report(&corpus, count, source_count, skipped);
        println!(
            "\nNothing was written. Drop --dry-run to write {}.",
            embeddings_dir.display()
        );
        return Ok(());
    }

    if skipped > 0 {
        println!(
            "embedded {count} instance(s) → {} ({skipped} skipped)",
            embeddings_dir.display()
        );
    } else {
        println!(
            "embedded {count} instance(s) → {}",
            embeddings_dir.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::CatalogLocation;

    fn loc(desc: Option<&str>) -> CatalogLocation {
        CatalogLocation {
            kind: Some("url".into()),
            value: Some("https://example.com/x".into()),
            description: desc.map(str::to_string),
        }
    }

    #[test]
    fn a_source_composes_name_type_description_and_body() {
        let t = compose_source_text(
            "Allen County Canvasses",
            "Certified county canvasses.",
            "dataset",
            &[],
            "The board posts three elections.",
        );
        assert!(t.contains("Allen County Canvasses"));
        assert!(t.contains("A dataset source."));
        assert!(t.contains("Certified county canvasses."));
        assert!(t.contains("The board posts three elections."));
    }

    /// A URL contributes no meaning to a sentence embedding and dilutes the vector it
    /// sits in; the prose beside it is what a reader was told the source holds.
    #[test]
    fn location_descriptions_are_kept_and_urls_are_not() {
        let t = compose_source_text(
            "S",
            "",
            "",
            &[loc(Some("Precinct-level SOVC for 2024.")), loc(None)],
            "",
        );
        assert!(t.contains("Precinct-level SOVC for 2024."));
        assert!(!t.contains("https://"), "{t}");
    }

    #[test]
    fn an_empty_source_composes_to_nothing() {
        assert!(compose_source_text("", "", "", &[], "").trim().is_empty());
    }

    #[test]
    fn the_body_is_everything_under_the_frontmatter() {
        let doc = "---\nname: X\ntype: dataset\n---\n\nThe body.\nTwo lines.\n";
        assert_eq!(frontmatter_body(doc).trim(), "The body.\nTwo lines.");
    }

    #[test]
    fn a_file_with_no_frontmatter_is_all_body() {
        assert_eq!(frontmatter_body("Just prose.\n").trim(), "Just prose.");
    }

    /// `---` inside the body must not be read as the end of a frontmatter that never
    /// opened.
    #[test]
    fn an_unopened_frontmatter_is_not_split() {
        let doc = "Prose first.\n\n---\n\nA horizontal rule.\n";
        assert!(frontmatter_body(doc).starts_with("Prose first."));
    }

    #[test]
    fn the_catalog_is_on_by_default_and_the_flag_turns_it_off() {
        assert!(EmbedOptions::default().catalog);
        assert!(
            !EmbedOptions {
                catalog: false,
                ..Default::default()
            }
            .catalog
        );
    }

    /// A dry run is off by default. The flag is what a measurement over a corpus that is not
    /// ours to write into asks for, and nothing else should get it by accident.
    #[test]
    fn a_dry_run_is_off_by_default() {
        assert!(!EmbedOptions::default().dry_run);
    }

    /// Node composition is unchanged — this is a scope change, not a re-embedding of the
    /// corpus, and an altered node text would silently invalidate every existing index.
    #[test]
    fn node_text_composition_is_unchanged() {
        let links = vec![crate::parse::CorpusLink {
            target: Some("person/matt-huffman.yml".into()),
            relationship: Some("mentions".into()),
            ..Default::default()
        }];
        assert_eq!(
            compose_text("A label", "A description.", &links),
            "A label A description. Related: matt huffman."
        );
    }
}
