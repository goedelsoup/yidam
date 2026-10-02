//! `search_sources` and `resolve_source`: the read half of `yidam source` over MCP (#1319).
//!
//! RFC-0048 §8. The measured workflow is an agent adding sources, so the two questions it asks
//! before `source add` are offered to it directly: what a pack's search endpoint holds under a
//! name, and what entry an identifier would draft. Both answer from the functions the command
//! runs, so an MCP answer and a `--dry-run` cannot disagree about what would be written.
//!
//! # Neither writes
//!
//! Adding an entry is a commit, and stays `source add` on the CLI. `resolve_source` returns the
//! frontmatter and body that command would write, and the path it would write them to.
//!
//! # `answered` is the offline marker
//!
//! A server whose corpus declares `[serve] offline_sources` asks no publisher and answers from
//! the packs' recorded fixtures. Every answer says which it gave: `network` or `fixture`, and
//! null where nothing was asked. That settles RFC-0048's open question 1. An identifier being a
//! fixture's own does not tell a caller the list was not the publisher's live answer, and the
//! CLI already reports the same field.

use serde_json::{json, Value};

use super::ServerState;
use crate::cmd::source;
use crate::sources::draft::{self, Needs, Resolved};
use crate::sources::transform::{self, Draft};

/// The default `limit`, the same as `source search`'s.
const LIMIT: u64 = 10;

/// Whether this corpus enables a pack: the `sources` capability.
pub(crate) fn backed(state: &ServerState) -> bool {
    !transform::enabled(&state.packs).is_empty()
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{key} is required"))
}

pub(crate) fn search_sources(state: &ServerState, args: &Value) -> Result<Value, String> {
    let pack = required(args, "pack")?;
    let query = required(args, "query")?;
    let limit = args["limit"].as_u64().unwrap_or(LIMIT).max(1) as usize;
    let offline = state.offline_sources;
    let searched = source::search(&state.root, &state.packs, pack, query, limit, offline)
        .map_err(|e| format!("{e:#}"))?;

    // A summary per candidate where the build runs transforms and the scheme declares a
    // describe. Said once at the top when it cannot be, rather than as the same null on each.
    let undescribed = summaries_unavailable(state, &searched.scheme);
    let mut candidates = Vec::new();
    source::with_asker(&state.root, &state.packs, offline, |asker| {
        for c in &searched.candidates {
            let summary = match undescribed {
                Some(_) => None,
                None => draft::describe_only(&state.root, &state.packs, &c.identifier, asker)
                    .ok()
                    .filter(|(d, _)| d.by.is_some())
                    .map(|(d, _)| summary(&d)),
            };
            candidates.push(json!({
                "identifier": c.identifier,
                "title": c.title,
                "summary": summary,
            }));
        }
    })
    .map_err(|e| format!("{e:#}"))?;

    Ok(json!({
        "pack": searched.pack,
        "query": searched.query,
        "scheme": searched.scheme,
        "url": searched.url,
        // The search's own answer. Each summary is asked the same way, so it cannot differ.
        "answered": searched.answered,
        "candidates": candidates,
        "undescribed": undescribed,
    }))
}

/// Why no candidate in `scheme` can carry a summary, or `None` when each may.
fn summaries_unavailable(state: &ServerState, scheme: &str) -> Option<&'static str> {
    if !transform::AVAILABLE {
        return Some(transform::UNAVAILABLE);
    }
    match transform::scheme(&state.packs, scheme) {
        Ok((_, _, s)) if s.describe.is_some() => None,
        _ => Some("the scheme declares no describe transform"),
    }
}

/// What a describe filled, in the catalog's own field names.
fn summary(d: &Draft) -> Value {
    json!({
        "name": d.name,
        "type": d.kind,
        "date": d.date,
        "description": d.description,
        "by": d.by,
    })
}

pub(crate) fn resolve_source(state: &ServerState, args: &Value) -> Result<Value, String> {
    let identifier = required(args, "identifier")?;
    let root = &state.root;
    let resolved = source::with_asker(root, &state.packs, state.offline_sources, |asker| {
        draft::resolve(root, &state.packs, identifier, asker)
    })
    .map_err(|e| format!("{e:#}"))??;

    let catalog_dir = crate::paths::yidam_catalog_dir(root);
    let catalogued = source::catalogued(&catalog_dir);
    let already = resolved
        .locations
        .iter()
        .find_map(|l| catalogued.get(&l.identifier).cloned());
    let entry = source::free_slug(&catalog_dir, &source::slug_for(&resolved), &[]);
    let needs = needs(state, &resolved);

    Ok(json!({
        "identifier": resolved.identifier,
        "pack": resolved.pack,
        "entry": entry,
        "path": format!(".yidam/catalog/{entry}.md"),
        "catalogued": already,
        "frontmatter": source::frontmatter(&entry, &resolved),
        "body": source::body(&resolved),
        "locations": resolved.locations.iter().map(|l| json!({
            "identifier": l.identifier,
            "url": l.url,
            "via": l.via,
            "description": l.description(),
        })).collect::<Vec<_>>(),
        "draft": {
            "name": resolved.draft.name,
            "type": resolved.draft.kind,
            "date": resolved.draft.date,
            "description": resolved.draft.description,
            "unfilled": resolved.draft.unfilled,
            "why": resolved.draft.why,
            "by": resolved.draft.by,
        },
        "ttl_days": resolved.ttl_days,
        "answered": resolved.answered,
        "unfollowed": resolved.unfollowed,
        "needs": needs.as_ref().map(|n| json!({
            "contact": n.contact,
            "contact_set": n.contact_set,
            "auth": n.auth,
            "blocked": n.blocked,
        })),
        "unmet": needs.as_ref().and_then(Needs::unmet),
    }))
}

/// What the resolving pack's transport asks of this server's environment.
fn needs(state: &ServerState, r: &Resolved) -> Option<Needs> {
    transform::enabled(&state.packs)
        .into_iter()
        .find(|(p, _)| p.name == r.pack)
        .map(|(_, m)| Needs::of(m, &env))
}
