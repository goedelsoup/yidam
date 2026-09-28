//! `yidam routes` — the reading routes, by occasion, from the vendored prelude (RFC-0039).
//!
//! `AGENTS.md` used to hand every session one list of whole files: about 18,000 words before
//! any substantive action. Most of it is reference that a given act never touches. An agent
//! about to write a node needs the class contract and the corpus conventions, and has no use
//! for the catalog frontmatter or the resolution protocol. So the route names the **occasion**
//! by the commit verb it ends in, and names **sections** rather than files. #967 measured that
//! an agent handed a section link reads the section and stops.
//!
//! **The routes are data, and this renders them.** They live in `routes.yml` in the vendored
//! prelude, and this writes them into the `<!-- REGEN: yidam routes -->` block of `AGENTS.md`.
//! A derivation's `AGENTS.md` is installed once, at genesis. Of sixteen route lines the
//! template later added, two reached a derivation by hand and nothing reported the other
//! fourteen (#969). A block is re-rendered on every re-vendor, and a stale one fails
//! `regen --check` in the derivation's own CI.
//!
//! **It reads the vendored prelude and nothing else**, so a fresh clone and a working copy
//! render the same bytes (the #647 contract). The block owns the list under *Before taking
//! substantive action* and not the heading or the prose after it: across nine derivations,
//! that list held no owner edits and the rest of each file was heavily edited.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::Path;

use crate::regen::update_file_regen;

/// Where the routes are read from, relative to the repository root.
pub(crate) const VENDORED: &str = ".yidam/.vendor/prelude/routes.yml";

/// The link prefix a derived repository's `AGENTS.md` reaches the prelude by.
const VENDORED_PREFIX: &str = ".yidam/.vendor/prelude/";

/// The routes file: what every occasion reads, each occasion, and the reference index.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Routes {
    always: Vec<Entry>,
    occasions: Vec<Occasion>,
    reference: Vec<Entry>,
}

/// One occasion: the heading an agent finds it by, and the sections it reads.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Occasion {
    heading: String,
    /// The commit verbs that end this occasion, from GRAPH.md's closed vocabulary.
    #[serde(default)]
    verbs: Vec<String>,
    /// When the verbs alone do not say it, or there is no verb.
    #[serde(default)]
    note: Option<String>,
    read: Vec<Entry>,
}

/// One link: a prelude path with an optional `#fragment`, what to call it, and why to read it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    read: String,
    label: String,
    #[serde(default)]
    gloss: Option<String>,
}

pub fn routes(root: Option<&Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let content = block(&root)?;
    crate::regen::emit(&content);
    // A literal: `every_generator_in_the_crate_is_listed` reads call sites for the name they
    // write, and a constant is invisible to it.
    update_file_regen(&root.join("AGENTS.md"), "yidam routes", &content)
}

/// The block's content for this repository.
fn block(root: &Path) -> Result<String> {
    let path = root.join(VENDORED);
    if !path.exists() {
        return Ok(format!(
            "_No `{VENDORED}` in this repository's vendored prelude. Re-vendor \
             (`mise run yidam-vendor-update`), then run `yidam routes`._"
        ));
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {VENDORED}"))?;
    let routes = parse(&text).with_context(|| format!("parsing {VENDORED}"))?;
    Ok(render(&routes, VENDORED_PREFIX))
}

pub(crate) fn parse(text: &str) -> Result<Routes> {
    Ok(serde_yaml::from_str(text)?)
}

/// The block's text, with every link reaching the prelude through `prefix`.
///
/// The prefix is the one thing the template's own `AGENTS.md` and a derivation's differ by:
/// the template links `yidam/prelude/`, a derivation `.yidam/.vendor/prelude/`.
pub(crate) fn render(routes: &Routes, prefix: &str) -> String {
    let mut out = String::from(
        "Read by occasion. Find the occasion your next commit belongs to, then read *On every \
         occasion* and that occasion's list, in order. A link with a `#` names one section: \
         read from its heading and stop at the next heading of the same level or higher. \
         *Reference* is every file whole, for an occasion not named here.\n",
    );
    out.push_str("\n### On every occasion\n\n");
    list(&mut out, &routes.always, prefix);
    for occasion in &routes.occasions {
        let _ = write!(out, "\n### {}\n\n", occasion.heading);
        let verbs: Vec<String> = occasion.verbs.iter().map(|v| format!("`{v}`")).collect();
        let lead = match (verbs.is_empty(), &occasion.note) {
            (false, Some(note)) => format!("Verbs: {}. {note}", verbs.join(", ")),
            (false, None) => format!("Verbs: {}.", verbs.join(", ")),
            (true, Some(note)) => note.clone(),
            (true, None) => String::new(),
        };
        if !lead.is_empty() {
            let _ = write!(out, "{lead}\n\n");
        }
        list(&mut out, &occasion.read, prefix);
    }
    out.push_str("\n### Reference\n\n");
    list(&mut out, &routes.reference, prefix);
    out.truncate(out.trim_end().len());
    out
}

fn list(out: &mut String, entries: &[Entry], prefix: &str) {
    for e in entries {
        let _ = write!(out, "- [{}]({prefix}{})", e.label, e.read);
        if let Some(gloss) = &e.gloss {
            let _ = write!(out, " — {gloss}");
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn template(rel: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()))
    }

    fn template_routes() -> Routes {
        parse(&template("yidam/prelude/routes.yml")).expect("yidam/prelude/routes.yml parses")
    }

    /// Both `AGENTS.md` files hold exactly what the generator writes.
    ///
    /// `sadhana/root/AGENTS.md` is a derivation's at genesis, so a block that differed would
    /// be stale in the new derivation's first `regen --check`. The template's own `AGENTS.md`
    /// has no `.yidam/`, so `regen` never runs on it. This test is the only thing that keeps
    /// its route equal to the one every derivation reads (#969 left the choice to #972).
    #[test]
    fn both_agents_md_hold_the_rendered_block() {
        let routes = template_routes();
        for (file, prefix) in [
            ("sadhana/root/AGENTS.md", VENDORED_PREFIX),
            ("AGENTS.md", "yidam/prelude/"),
        ] {
            let text = template(file);
            assert!(
                text.contains("<!-- REGEN: yidam routes\n"),
                "{file} has no `yidam routes` block; the routes reach it by nothing"
            );
            let rendered =
                crate::regen::update_regen(&text, "yidam routes", &render(&routes, prefix));
            assert!(
                rendered == text,
                "{file}'s `yidam routes` block is not what `yidam/prelude/routes.yml` renders. \
                 Edit the routes in routes.yml, then copy the rendering in; the block is \
                 generated and a hand edit is overwritten at the next `yidam regen`."
            );
        }
    }

    /// An occasion is named by a verb the commit message can actually carry.
    #[test]
    fn every_verb_is_in_the_closed_vocabulary() {
        let closed: BTreeSet<&str> = super::super::vocabulary::vocabulary_verbs()
            .into_iter()
            .collect();
        let mut seen = BTreeSet::new();
        for occasion in template_routes().occasions {
            for verb in &occasion.verbs {
                assert!(
                    closed.contains(verb.as_str()),
                    "`{}` names `{verb}`, which is not in GRAPH.md's commit vocabulary",
                    occasion.heading
                );
                assert!(
                    seen.insert(verb.clone()),
                    "`{verb}` names two occasions; an agent holding it cannot tell which to read"
                );
            }
        }
    }

    /// Every occasion's heading is one the ceiling gate can find, and says when.
    #[test]
    fn every_occasion_is_headed_and_reads_something() {
        let routes = template_routes();
        assert!(!routes.occasions.is_empty(), "routes.yml names no occasion");
        for occasion in &routes.occasions {
            assert!(
                occasion.heading.starts_with("Before "),
                "`{}` does not start `Before `, which is how the ceiling gate finds an occasion",
                occasion.heading
            );
            assert!(
                !occasion.verbs.is_empty() || occasion.note.is_some(),
                "`{}` has no verb and no note; nothing says when it applies",
                occasion.heading
            );
            assert!(
                !occasion.read.is_empty(),
                "`{}` reads nothing",
                occasion.heading
            );
        }
    }

    /// A link to a missing file is the one failure the fragment gate skips.
    #[test]
    fn every_read_is_a_prelude_file() {
        let routes = template_routes();
        let entries = routes
            .always
            .iter()
            .chain(routes.occasions.iter().flat_map(|o| &o.read))
            .chain(&routes.reference);
        for e in entries {
            let file = e.read.split('#').next().unwrap_or_default();
            assert!(
                !template(&format!("yidam/prelude/{file}")).is_empty(),
                "routes.yml reads `{}`, and yidam/prelude/{file} is empty",
                e.read
            );
        }
    }

    #[test]
    fn an_absent_routes_file_says_what_to_do() {
        let tmp = tempfile::tempdir().unwrap();
        let text = block(tmp.path()).unwrap();
        assert!(text.contains("yidam-vendor-update"), "{text}");
    }
}
