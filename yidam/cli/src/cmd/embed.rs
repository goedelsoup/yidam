use anyhow::Result;
use serde::Serialize;

use crate::git::head_commit_short;
use crate::parse::{frontmatter_body, parse_frontmatter, CorpusInstance};
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir, yidam_embeddings_dir};
use crate::walk::{walk_corpus_instances, walk_md_files};

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
}

impl Default for EmbedOptions {
    fn default() -> Self {
        Self { catalog: true }
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

    let instances = walk_corpus_instances(&corpus_dir);
    if instances.is_empty() && sources.is_empty() {
        println!("No corpus instances found in {}.", corpus_dir.display());
        return Ok(());
    }

    std::fs::create_dir_all(&embeddings_dir)?;

    let mut count = 0;
    let mut skipped = 0usize;
    for path in &instances {
        let yaml = std::fs::read_to_string(path)?;
        let inst: CorpusInstance = match serde_yaml::from_str(&yaml) {
            Ok(v) => v,
            Err(e) => {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                eprintln!("[warn] skipping {}: {e}", rel.display());
                skipped += 1;
                continue;
            }
        };

        let class = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let label = inst.label.as_deref().unwrap_or("").to_string();
        let description = inst.description.as_deref().unwrap_or("").to_string();
        let links = inst.links.as_deref().unwrap_or(&[]);
        let text = compose_text(&label, &description, links);

        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let record = EmbedRecord {
            path: rel_path.to_string_lossy().to_string(),
            class: class.clone(),
            label,
            text,
            commit: commit.clone(),
            kind: "node".to_string(),
        };

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
            kind: "source".to_string(),
        };
        let out_dir = embeddings_dir.join("_catalog");
        std::fs::create_dir_all(&out_dir)?;
        std::fs::write(
            out_dir.join(format!("{stem}.json")),
            serde_json::to_string_pretty(&record)?,
        )?;
        source_count += 1;
    }

    if source_count > 0 {
        println!("  {source_count} catalog source(s)");
    } else if !opts.catalog {
        println!("  catalog skipped (--no-catalog)");
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
        assert!(!EmbedOptions { catalog: false }.catalog);
    }

    /// Node composition is unchanged — this is a scope change, not a re-embedding of the
    /// corpus, and an altered node text would silently invalidate every existing index.
    #[test]
    fn node_text_composition_is_unchanged() {
        let links = vec![crate::parse::CorpusLink {
            target: Some("person/matt-huffman.yml".into()),
            relationship: Some("mentions".into()),
        }];
        assert_eq!(
            compose_text("A label", "A description.", &links),
            "A label A description. Related: matt huffman."
        );
    }
}
