//! The design system's inventory agrees with itself.
//!
//! There are four statements of what the system contains, and until #467 nothing compared
//! them: the files under `components/`, the `components` list in `_ds_manifest.json`, the
//! prop contracts in `_adherence.oxlintrc.json`, and — new here — the exports in `index.js`.
//!
//! `index.js` is the one that had never existed. `_adherence.oxlintrc.json` has forbidden
//! importing `components/<group>/**` since it was written, with the message "Import
//! design-system components from 'index.js', not component internals", and there was no such
//! file. Nothing imported anything either, so the rule had never fired and the file it named
//! had never been missed. #465 found the same shape twice — a lint nobody invoked, and a
//! config that could not have loaded — and this is the third: a rule that could not fire,
//! naming a door that was not there.
//!
//! The quality pages are the first consumer, so the door now matters. A component missing
//! from the barrel is an import that resolves to `undefined` and a React render of nothing;
//! a barrel entry for a file that moved is a build error one workspace away from here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

const DESIGN: &str = "yidam/design";

/// `components/<group>/<Name>.jsx`, from the filesystem.
fn component_files() -> BTreeSet<String> {
    let root = repo_root().join(DESIGN).join("components");
    let mut out = BTreeSet::new();
    for group in std::fs::read_dir(&root).expect("yidam/design/components is readable") {
        let group = group.expect("entry").path();
        if !group.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&group).expect("component group is readable") {
            let path = file.expect("entry").path();
            if path.extension().is_some_and(|e| e == "jsx") {
                out.insert(path.file_stem().unwrap().to_string_lossy().to_string());
            }
        }
    }
    assert!(
        out.len() > 15,
        "only {} components found on disk; the walk is looking at the wrong tree",
        out.len()
    );
    out
}

fn manifest_components() -> BTreeSet<String> {
    let manifest: serde_json::Value =
        serde_json::from_str(&read(&format!("{DESIGN}/_ds_manifest.json")))
            .expect("_ds_manifest.json parses");
    manifest["components"]
        .as_array()
        .expect("the manifest declares no components")
        .iter()
        .filter_map(|c| c["name"].as_str())
        .map(str::to_string)
        .collect()
}

/// `export { Name } from './…'` in the barrel, comments stripped.
fn barrel_exports() -> BTreeSet<String> {
    read(&format!("{DESIGN}/index.js"))
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim().to_string())
        .filter_map(|l| {
            let rest = l.strip_prefix("export {")?;
            Some(rest.split('}').next()?.trim().to_string())
        })
        .collect()
}

/// The three inventories name the same components.
#[test]
fn the_files_the_manifest_and_the_barrel_agree() {
    let files = component_files();
    let manifest = manifest_components();
    let barrel = barrel_exports();

    let missing_from_manifest: Vec<&String> = files.difference(&manifest).collect();
    assert!(
        missing_from_manifest.is_empty(),
        "these components exist and `_ds_manifest.json` does not list them: \
         {missing_from_manifest:?}"
    );
    let stale_in_manifest: Vec<&String> = manifest.difference(&files).collect();
    assert!(
        stale_in_manifest.is_empty(),
        "`_ds_manifest.json` lists components with no file: {stale_in_manifest:?}"
    );

    let missing_from_barrel: Vec<&String> = files.difference(&barrel).collect();
    assert!(
        missing_from_barrel.is_empty(),
        "these components are not exported from `index.js`, so an importer that follows the \
         adherence lint's own instruction gets `undefined` and renders nothing: \
         {missing_from_barrel:?}"
    );
    let stale_in_barrel: Vec<&String> = barrel.difference(&files).collect();
    assert!(
        stale_in_barrel.is_empty(),
        "`index.js` exports components with no file: {stale_in_barrel:?}"
    );
}

/// Every component carries the two documents that make it usable without reading the JSX.
#[test]
fn every_component_declares_its_props_and_says_what_it_is_for() {
    let root = repo_root().join(DESIGN).join("components");
    let mut missing = Vec::new();
    for group in std::fs::read_dir(&root).expect("components dir") {
        let group = group.expect("entry").path();
        if !group.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&group).expect("group dir") {
            let path = file.expect("entry").path();
            if path.extension().is_none_or(|e| e != "jsx") {
                continue;
            }
            for companion in ["d.ts", "prompt.md"] {
                let sibling = path.with_extension(companion);
                if !sibling.exists() {
                    missing.push(format!(
                        "  {}",
                        sibling
                            .strip_prefix(repo_root())
                            .unwrap_or(&sibling)
                            .display()
                    ));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these components have no type declaration or no usage note:\n{}",
        missing.join("\n")
    );
}

/// Every prop a component is passed is one it declares.
///
/// This was 40 selectors in `_adherence.oxlintrc.json`, each transcribing a component's
/// `.d.ts` into an esquery regex, and every one of them was inert: oxlint does not implement
/// `no-restricted-syntax` — the rule is absent from `oxlint --rules` — and an unknown rule
/// key is accepted at load and ignored at run. Forty transcriptions of a file that was right
/// there, enforcing nothing.
///
/// So this reads the `.d.ts` instead. Both sides are derived: the declared props from the
/// type file, the passed props from every JSX and Astro usage in the repository. A component
/// added tomorrow is covered by the file it already has to have.
#[test]
fn every_prop_passed_to_a_component_is_declared() {
    let components = component_files();
    let declared: BTreeMap<String, BTreeSet<String>> = components
        .iter()
        .map(|name| (name.clone(), declared_props(name)))
        .collect();

    let mut undeclared = Vec::new();
    let mut checked = 0usize;
    for (rel, text) in jsx_surfaces() {
        // Only names this file imported. Without that, `Card.jsx`'s `as: Tag = 'div'` — a
        // local rename of the element type, a plain React idiom — reads as three violations
        // on the design system's own `Tag`. A guard that invents a violation gets edited
        // around, and then it is not a guard.
        let in_scope: BTreeSet<String> = imported_names(&text)
            .intersection(&components)
            .cloned()
            .collect();
        for (component, prop, line) in props_passed(&text, &in_scope) {
            checked += 1;
            let props = &declared[&component];
            if !props.contains(&prop) && !UNIVERSAL.contains(&prop.as_str()) {
                undeclared.push(format!(
                    "  {rel}:{line}: <{component} {prop}=…> — declared: {}",
                    props.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }

    assert!(
        checked > 0,
        "no component usage was found anywhere in the repository, so this asserts nothing. \
         Either the design system has no consumer again — which is the state #465 left it in \
         and #467's quality pages were the first thing to change — or the scan is looking at \
         the wrong tree."
    );
    assert!(
        undeclared.is_empty(),
        "these pass a prop the component does not declare. It is dropped silently: React \
         ignores what a function component does not read, so the element renders without \
         it:\n{}",
        undeclared.join("\n")
    );
}

/// Identifiers a file imports, from `import { A, B } from '…'` and `import A from '…'`.
fn imported_names(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
    {
        let Some(rest) = line.strip_prefix("import ") else {
            continue;
        };
        let clause = rest.split(" from ").next().unwrap_or("");
        if let Some(braced) = clause.split('{').nth(1).and_then(|c| c.split('}').next()) {
            for name in braced.split(',') {
                // `X as Y` binds Y; the prop contract belongs to whatever the JSX names.
                let bound = name.rsplit(" as ").next().unwrap_or("").trim();
                if !bound.is_empty() {
                    out.insert(bound.to_string());
                }
            }
        }
        let default = clause.split(['{', ',']).next().unwrap_or("").trim();
        if !default.is_empty() && default.chars().all(|c| c.is_alphanumeric() || c == '_') {
            out.insert(default.to_string());
        }
    }
    out
}

/// Props React and Astro accept on anything, which no component has to declare.
const UNIVERSAL: &[&str] = &["key", "ref", "className", "class", "style", "children"];

/// The members of `interface <Name>Props { … }`, from the component's own type declaration.
fn declared_props(component: &str) -> BTreeSet<String> {
    let root = repo_root().join(DESIGN).join("components");
    let path = WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n == format!("{component}.d.ts").as_str())
        })
        .unwrap_or_else(|| panic!("{component} has no .d.ts"));
    let text = std::fs::read_to_string(&path).expect("readable");

    let body = text
        .split(&format!("interface {component}Props {{"))
        .nth(1)
        .unwrap_or_else(|| panic!("{}: no `interface {component}Props`", path.display()))
        .split("\n}")
        .next()
        .unwrap_or_default();

    let out: BTreeSet<String> = body
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
        .filter_map(|l| {
            let name = l.split(['?', ':']).next()?.trim();
            let is_member = l.contains(':')
                && !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_');
            is_member.then(|| name.to_string())
        })
        .collect();
    assert!(
        !out.is_empty(),
        "{}: `{component}Props` declares no members, so every prop would be a violation",
        path.display()
    );
    out
}

/// Every `.jsx` and `.astro` in the repository — the places a component can be used.
fn jsx_surfaces() -> Vec<(String, String)> {
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
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !entry.file_type().is_file() || !(ext == "jsx" || ext == "astro") {
            continue;
        }
        let rel = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((rel, std::fs::read_to_string(path).unwrap_or_default()));
    }
    out.sort();
    out
}

/// `(component, prop, line)` for every attribute passed to a known component.
///
/// A hand-rolled scan rather than a JSX parser, and narrow on purpose: it reads attribute
/// *names* between `<Name` and the end of the opening tag, tracking brace depth so a
/// `{expression > 0}` does not end the tag early. What it can miss is a spread; what it must
/// not do is invent a violation, which is why anything it cannot resolve is skipped.
fn props_passed(text: &str, components: &BTreeSet<String>) -> Vec<(String, String, usize)> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == '\n' {
            line += 1;
        }
        if bytes[i] != '<' {
            i += 1;
            continue;
        }
        let rest: String = bytes[i + 1..].iter().take(40).collect();
        let Some(name) = components.iter().find(|c| {
            rest.starts_with(c.as_str()) && !rest[c.len()..].starts_with(char::is_alphanumeric)
        }) else {
            i += 1;
            continue;
        };

        // Walk the opening tag, counting braces so an expression cannot end it early.
        let mut j = i + 1 + name.len();
        let mut depth = 0i32;
        let mut word = String::new();
        let start_line = line;
        while j < bytes.len() {
            let c = bytes[j];
            match c {
                '\n' => line += 1,
                '{' => depth += 1,
                '}' => depth -= 1,
                '>' if depth == 0 => break,
                _ => {}
            }
            if depth == 0 {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    word.push(c);
                } else {
                    if c == '=' && !word.is_empty() {
                        out.push((name.clone(), std::mem::take(&mut word), start_line));
                    }
                    word.clear();
                }
            } else {
                word.clear();
            }
            j += 1;
        }
        i = j;
    }
    out
}

/// The generated browser bundle is behind the source, and this says by how much.
///
/// `_ds_bundle.js` is design-tool output: an IIFE that hangs the components off a window
/// namespace, carrying source hashes, and the `.card.html` previews render out of it. Only
/// the tool can rewrite it, so a component added by hand has no card until the next sync.
///
/// That is a real gap and it is stated here rather than left to be discovered by someone
/// opening a preview and finding it blank. What must not happen is the *other* direction —
/// a bundle naming a component the source no longer has, which would be a preview rendering
/// something nobody can edit.
#[test]
fn the_generated_bundle_is_a_subset_of_the_source() {
    let bundle = read(&format!("{DESIGN}/_ds_bundle.js"));
    let header = bundle
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("/* @ds-bundle: "))
        .and_then(|l| l.strip_suffix(" */"))
        .expect("_ds_bundle.js has no @ds-bundle header");
    let meta: serde_json::Value = serde_json::from_str(header).expect("the bundle header parses");
    let bundled: BTreeSet<String> = meta["components"]
        .as_array()
        .expect("the bundle header lists no components")
        .iter()
        .filter_map(|c| c["name"].as_str())
        .map(str::to_string)
        .collect();

    let files = component_files();
    let phantom: Vec<&String> = bundled.difference(&files).collect();
    assert!(
        phantom.is_empty(),
        "`_ds_bundle.js` carries {phantom:?}, which no longer exist in the source. A card \
         preview would render a component nobody can edit."
    );

    // The other direction is expected and is recorded, not asserted away. If it ever reaches
    // zero the design tool has been re-run and this note can go.
    let unpreviewed: Vec<&String> = files.difference(&bundled).collect();
    assert!(
        unpreviewed.len() <= 2,
        "{} components are missing from the generated bundle and therefore have no card \
         preview: {unpreviewed:?}. Two is the number #467 added by hand; more than that \
         means the bundle has fallen further behind than anyone decided it should.",
        unpreviewed.len()
    );
}
