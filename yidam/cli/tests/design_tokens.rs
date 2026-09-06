//! One palette, and every surface reads it rather than a copy of it.
//!
//! `yidam/design/tokens/*.css` had two consumers and neither imported it (#465). The docs
//! site retyped the hex values into Starlight's variable slots with the token name in a
//! trailing comment; the CLI's web agent retyped a subset into a `:root` block whose own
//! first line called it a subset. Three copies, kept in step by nothing.
//!
//! They had not stayed in step. Ten of the CLI copy's twenty-four values no longer matched
//! the source, and `--inference-*` and `--open-*` had **swapped colour families**: an
//! exported corpus showed inference in blue and open questions in amber while every other
//! surface said the reverse. The web agent's own `DESIGN.md` had specified
//! `--status-{verified,inference,open}-*` from the start. Nothing made it so.
//!
//! What this file checks is that the collapse holds. The lint that reads JSX is a separate
//! job — oxlint parses JavaScript and the two drifted consumers are CSS, which it cannot
//! read at all, so the CSS side is guarded here instead.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

const DESIGN: &str = "yidam/design";

/// CSS with `/* … */` blocks blanked, line structure preserved.
///
/// Necessary, and found the way these are found: the first version of this file scanned raw
/// text and failed on its own prose. `#465` in a header comment read as a colour, and a
/// `var(--…)` in a sentence explaining the rule read as a dangling token. A guard that
/// grades comments is the mirror of #461's guard that was *satisfied* by one — the same
/// mistake seen from the other side.
///
/// Newlines survive so reported line numbers stay true.
fn css_code_only(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.char_indices().peekable();
    let mut in_comment = false;
    let mut in_line_comment = false;
    while let Some((i, c)) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push('\n');
            }
            continue;
        }
        if !in_comment && c == '/' && css[i..].starts_with("/*") {
            in_comment = true;
            chars.next();
            continue;
        }
        // `//` too: the scanned set is no longer only CSS, and `.astro` frontmatter and
        // `.jsx` both comment with it. The `:` guard keeps `https://` out of it — a comment
        // stripper that ate the rest of a line after a url would blank real declarations.
        if !in_comment && c == '/' && css[i..].starts_with("//") && !out.ends_with(':') {
            in_line_comment = true;
            chars.next();
            continue;
        }
        if in_comment {
            if c == '*' && css[i..].starts_with("*/") {
                in_comment = false;
                chars.next();
            } else if c == '\n' {
                out.push('\n');
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// `--name: value` declarations in a stylesheet, first definition winning.
fn declarations(css: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in css_code_only(css).lines() {
        let line = line.trim();
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !name.starts_with("--") {
            continue;
        }
        out.entry(name.to_string())
            .or_insert_with(|| value.trim().trim_end_matches(';').trim().to_string());
    }
    out
}

/// Every token the design system declares.
fn source_tokens() -> BTreeMap<String, String> {
    let dir = repo_root().join(DESIGN).join("tokens");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("yidam/design/tokens is readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "css"))
        .collect();
    files.sort();
    let mut out = BTreeMap::new();
    for f in files {
        for (k, v) in declarations(&std::fs::read_to_string(&f).expect("token file")) {
            out.entry(k).or_insert(v);
        }
    }
    assert!(
        out.len() > 100,
        "only {} tokens parsed from yidam/design/tokens; every assertion below would be \
         vacuous",
        out.len()
    );
    out
}

/// File types that can hold a token reference and are worth reading for one.
///
/// `.css` was the whole list, and the comment below it said P6's quality pages would be
/// "covered the moment they use the palette". They were not: those pages are `.astro` and the
/// components they render are `.jsx`, and neither was ever opened here. The adherence lint
/// reads the JSX — but only for a raw hex, never for a token that does not exist, since the
/// roster it once used for that was the fourth copy of the palette and #465 deleted it. So a
/// `var(--ink-90)` in a component resolved to nothing, rendered unstyled, and passed both
/// gates. A discovered set that discovers the wrong file types is a roster with extra steps.
const CONSUMER_EXTENSIONS: &[&str] = &["css", "astro", "jsx"];

/// Surfaces outside the design system that consume its tokens — discovered, not listed.
///
/// A consumer is any file of a [`CONSUMER_EXTENSIONS`] type outside `yidam/design/` that
/// references a token the system declares. The next UI kit is covered the moment it uses the
/// palette, which is the point: a roster here would stop covering whatever came next.
///
/// The test used to be `--ink-` or `--gold-`, and that is a roster too — of two families out
/// of nine. `semantic.css` exists precisely so a surface says `--text-primary` rather than
/// naming an ink, and the design system's own guidance is to use it; so the recommended way
/// to consume the palette was the way that made a file invisible here. Every quality page
/// #467 added was a consumer this function did not return.
fn consumer_stylesheets() -> Vec<(String, String)> {
    let declared = source_tokens();
    let mut out = Vec::new();
    for entry in WalkDir::new(repo_root())
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "node_modules" && n != "target" && n != ".git" && n != "dist" && n != ".claude"
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let is_consumer_type = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| CONSUMER_EXTENSIONS.contains(&e));
        if !entry.file_type().is_file() || !is_consumer_type {
            continue;
        }
        let rel = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with(DESIGN) {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if references(&text).any(|name| declared.contains_key(&name)) {
            out.push((rel, text));
        }
    }
    out.sort();
    out
}

/// Every `var(--name)` in a file, comments included — detection is deliberately broad, and
/// the assertions below strip comments before grading anything.
fn references(text: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = text;
    std::iter::from_fn(move || loop {
        let at = rest.find("var(--")?;
        let after = &rest[at + 4..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-')
            .collect();
        rest = &after[name.len()..];
        if !name.is_empty() {
            return Some(name);
        }
    })
}

/// No surface spells a colour, inside the design system or outside it.
///
/// This is the copy-prevention, and it is deliberately about *hex* rather than about every
/// literal: a stylesheet may reasonably say `3px` or `50%`, and may not reasonably decide
/// what colour gold is. Every copy this phase collapsed was a colour copy.
///
/// It covered only the consumers until #512, and the system's own components held twenty
/// hand-spelled colours — eleven copies of one red. The exclusion that let them through was
/// right for `tokens/`, which legitimately holds values, and wrong for everything else in
/// the directory; `system_surfaces()` already draws that line for the dangling check, so
/// this uses the same set rather than a second opinion about where the palette lives.
///
/// The comment stripping is load-bearing here in a way it was not before. `#464` and `#467`
/// are issue numbers, this repository's prose is full of them, and one sits in a
/// `CoverageBar.jsx` comment. A scan that read them would report four false colours in the
/// files it was newly pointed at, and a check whose first run is mostly noise is a check
/// somebody turns off.
#[test]
fn no_surface_declares_a_raw_colour() {
    let mut surfaces = consumer_stylesheets();
    assert!(
        !surfaces.is_empty(),
        "no stylesheet outside yidam/design references the palette. Either both consumers \
         stopped using the design system, or this test is looking at the wrong tree — and \
         either way it is now asserting nothing."
    );
    surfaces.extend(system_surfaces());

    let mut offenders = Vec::new();
    for (rel, text) in &surfaces {
        // Comments may name a colour — this file's own header does. Declarations may not.
        let stripped = css_code_only(text);
        for (i, code) in stripped.lines().enumerate() {
            if !code.contains(':') {
                continue;
            }
            for (idx, c) in code.char_indices() {
                if c != '#' {
                    continue;
                }
                let hex: String = code[idx + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect();
                if (3..=8).contains(&hex.len()) {
                    offenders.push(format!("  {rel}:{}: #{hex}", i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these declare a colour instead of referencing one. That is how the palette came to \
         have three copies, how two evidence-tag colour families ended up transposed between \
         them, and how the design system's own danger red came to be spelled eleven times by \
         hand:\n{}\n\nIf the value is genuinely new, it belongs in `tokens/colors.css` as a \
         family and in `tokens/semantic.css` as a role — which is what a component then asks \
         for by name.",
        offenders.join("\n")
    );
}

/// The design system's own surfaces, which the consumer scan deliberately excludes.
///
/// `consumer_stylesheets` skips `yidam/design/` because the token files legitimately *hold*
/// colour values and the copy checks would grade the source as a copy of itself. The dangling
/// check has no such reason, and skipping these left the system's own components unguarded: a
/// `var(--run-pased-fill)` in a component under `components/` rendered unstyled and passed
/// every gate, which is the same failure as a consumer's dangling token seen from inside.
///
/// Components and `styles.css` only. The `.card.html` previews and the two interface
/// prototypes beside them are design-tool output rather than shipped surfaces, and a roster
/// of dangling references in a prototype is noise this check would then be read past.
fn system_surfaces() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let root = repo_root().join(DESIGN);
    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !entry.file_type().is_file() || !CONSUMER_EXTENSIONS.contains(&ext) {
            continue;
        }
        let rel = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let inside = rel
            .strip_prefix(DESIGN)
            .unwrap_or("")
            .trim_start_matches('/');
        if !(inside.starts_with("components/") || inside == "styles.css") {
            continue;
        }
        out.push((rel, std::fs::read_to_string(path).unwrap_or_default()));
    }
    assert!(
        out.len() > 15,
        "only {} design-system surfaces found; the walk is looking at the wrong tree",
        out.len()
    );
    out.sort();
    out
}

/// Every token a consumer references exists at the source.
///
/// The other direction, and the one that fails on a rename: `var(--ink-900)` against a
/// palette that no longer declares `--ink-900` resolves to nothing, and CSS says nothing
/// about it. The page renders unstyled and the build stays green.
#[test]
fn every_token_a_consumer_uses_is_declared_at_the_source() {
    let source = source_tokens();
    let mut dangling = Vec::new();
    let mut surfaces = consumer_stylesheets();
    surfaces.extend(system_surfaces());
    for (rel, text) in surfaces {
        let stripped = css_code_only(&text);
        for (i, line) in stripped.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find("var(--") {
                let after = &rest[at + 4..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-')
                    .collect();
                // A consumer may define and use its own local variables (Starlight's `--sl-*`).
                let local = declarations(&text).contains_key(&name);
                if !local && !source.contains_key(&name) {
                    dangling.push(format!("  {rel}:{}: var({name})", i + 1));
                }
                rest = &after[name.len()..];
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "these reference tokens the design system does not declare; each resolves to nothing \
         and renders as an unstyled element under a green build:\n{}",
        dangling.join("\n")
    );
}

/// The bundle imports what the manifest declares, in the order it declares.
///
/// `_ds_manifest.json` has listed the token files since the system was synced and nothing
/// read the list. Order is load-bearing — `semantic.css` resolves `var(--ink-…)` from the
/// files above it — so an entry point that reordered them would produce a stylesheet whose
/// variables silently resolve to nothing.
#[test]
fn the_entry_point_imports_the_manifests_files_in_order() {
    let manifest: serde_json::Value =
        serde_json::from_str(&read(&format!("{DESIGN}/_ds_manifest.json")))
            .expect("_ds_manifest.json parses");
    let declared: Vec<String> = manifest["globalCssPaths"]
        .as_array()
        .expect("globalCssPaths is an array")
        .iter()
        .map(|v| v.as_str().expect("a string").to_string())
        .collect();
    assert!(!declared.is_empty(), "the manifest names no CSS");

    let entry = read(&format!("{DESIGN}/tokens.css"));
    let imported: Vec<String> = entry
        .lines()
        .filter_map(|l| l.trim().strip_prefix("@import \"./"))
        .filter_map(|r| r.split('"').next())
        .map(str::to_string)
        .collect();

    assert_eq!(
        imported, declared,
        "yidam/design/tokens.css and _ds_manifest.json disagree about what the system is or \
         in what order it loads"
    );
}

/// The binary embeds the same set, and by reading the manifest rather than a list of its own.
#[test]
fn the_web_exporter_embeds_the_design_system() {
    let build_rs = read("yidam/cli/build.rs");
    assert!(
        build_rs.contains("_ds_manifest.json") && build_rs.contains("globalCssPaths"),
        "build.rs no longer reads the manifest; the exporter's token set is a second opinion \
         about what the design system contains"
    );
    let export = read("yidam/cli/src/cmd/export_web.rs");
    assert!(
        export.contains("DESIGN_TOKENS"),
        "the web exporter no longer embeds the design bundle — an exported page would carry \
         `var(--…)` references and nothing to resolve them"
    );
    let main_css = read("yidam/cli/assets/web/main.css");
    let declared: BTreeSet<String> = declarations(&main_css).into_keys().collect();
    assert!(
        declared.is_empty(),
        "assets/web/main.css declares {} token(s) of its own again; that is the copy #465 \
         removed, and the copy had transposed two colour families: {declared:?}",
        declared.len()
    );
}

/// Changing a token at the source moves every surface. The deliverable, stated as a test.
///
/// Not a simulation: it reads the real token values and the real consumers, and asserts that
/// no consumer holds a literal copy of one. A surface holding a copy would be unaffected by a
/// change at the source — which is exactly the state #465 found, in which ten of the CLI
/// copy's twenty-four values no longer matched and two colour families had swapped.
///
/// Every colour-valued token, not a probe. The probe was `--gold-400`, and no consumer
/// referenced it: the assertion beside it passed on an `|| contains("var(--ink-")` fallback,
/// so the named token was checking nothing. A derived set cannot go quietly vacuous that way.
#[test]
fn one_change_at_the_source_reaches_every_surface() {
    let source = source_tokens();
    let literals: BTreeMap<&String, &String> = source
        .iter()
        .filter(|(_, v)| v.starts_with('#') || v.starts_with("oklch("))
        .collect();
    assert!(
        literals.len() > 50,
        "only {} tokens hold a literal colour; the palette is not where this thinks it is",
        literals.len()
    );

    let consumers = consumer_stylesheets();
    assert!(
        !consumers.is_empty(),
        "nothing outside the design system reads a token"
    );

    let mut copies = Vec::new();
    let mut referenced = 0usize;
    for (rel, text) in &consumers {
        let code = css_code_only(text);
        for (name, value) in &literals {
            if code.contains(value.as_str()) {
                copies.push(format!("  {rel}: holds the value of {name} ({value})"));
            }
        }
        referenced += references(&code).filter(|n| source.contains_key(n)).count();
    }

    assert!(
        copies.is_empty(),
        "these hold a colour instead of referencing it. A change at the source would leave \
         them behind, silently, which is how the CLI's copy came to disagree with the \
         palette in ten places:\n{}",
        copies.join("\n")
    );
    assert!(
        referenced > 0,
        "no consumer references a declared token outside a comment, so every assertion here \
         is about files that no longer read the design system"
    );
}

/// Every token a theme declares, the default also declares.
///
/// `semantic.css` is a `:root` block and four `[data-theme="…"]` blocks. A token that appears
/// only in a theme resolves to nothing for every reader who has not chosen that theme — the
/// component renders unstyled, CSS says nothing about it, and the build is green. That is the
/// dangling-reference failure with the reference intact and the *declaration* conditional.
///
/// The near-miss that prompted it is one step sideways: #512 replaced a hand-spelled `#fff`
/// in `Toast` with `var(--action-fg)`, on the reasoning that it is the token for text on a
/// saturated surface. It is not — it is the text on the action *button*, and under the
/// default `sid` theme that button is light gold, so `--action-fg` is near-black ink. The
/// substitution would have put black text on a dark green toast in the default theme and
/// white text in two of the other three. No guard can catch a token whose value is wrong for
/// a use; this catches the neighbouring case, where the value is absent entirely.
#[test]
fn every_themed_token_is_also_declared_by_the_default_theme() {
    let css = css_code_only(&read(&format!("{DESIGN}/tokens/semantic.css")));

    // Blocks, by their selector. A brace-depth scan rather than a regex: the values contain
    // parentheses and commas, and a line-based split would lose a nested `oklch(…)`.
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut rest = css.as_str();
    while let Some(open) = rest.find('{') {
        let selector = rest[..open].trim().rsplit('}').next().unwrap_or("").trim();
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in rest[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = end else { break };
        blocks.push((selector.to_string(), rest[open + 1..close].to_string()));
        rest = &rest[close + 1..];
    }

    let names = |body: &str| -> BTreeSet<String> {
        body.lines()
            .filter_map(|l| l.trim().split_once(':'))
            .map(|(n, _)| n.trim().to_string())
            .filter(|n| n.starts_with("--"))
            .collect()
    };

    let root: BTreeSet<String> = blocks
        .iter()
        .filter(|(sel, _)| sel == ":root")
        .flat_map(|(_, body)| names(body))
        .collect();
    assert!(
        root.len() > 40,
        "only {} tokens parsed from the :root block; the scan has lost the file's shape and \
         everything below is vacuous",
        root.len()
    );

    let themes: Vec<&(String, String)> = blocks
        .iter()
        .filter(|(sel, _)| sel.starts_with("[data-theme"))
        .collect();
    assert!(
        themes.len() >= 3,
        "found {} [data-theme] blocks; semantic.css declares four themes",
        themes.len()
    );

    let mut orphans = Vec::new();
    for (sel, body) in themes {
        for name in names(body).difference(&root) {
            orphans.push(format!("  {sel} declares {name}, and :root does not"));
        }
    }
    assert!(
        orphans.is_empty(),
        "a token only a theme declares is undefined for every reader who has not chosen that \
         theme. The component referencing it renders unstyled and nothing goes red:\n{}\n\n\
         Declare it at :root with the default's value, then override it per theme.",
        orphans.join("\n")
    );
}
