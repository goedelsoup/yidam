//! The Claude Code plugin is two halves in one install, and nothing else holds them together.
//!
//! `/plugin install yidam@yidam` is meant to leave a person holding the MCP server *and* the
//! practice it enforces. That is a claim about three files nobody compiles — a marketplace
//! manifest, a plugin manifest, and a `.mcp.json` naming a shell script — plus a directory of
//! skills whose whole content is *which tool to call*. Every one of them can rot without a build
//! ever going red.
//!
//! The failure this file is built around is the specific one: **a skill that tells an agent to
//! call a tool the contract no longer has.** RFC-0005 froze the tool names; the skills spend
//! those names in prose; and prose is not linked against anything. So the central assertion
//! runs the other way round from a name list — it derives the vocabulary from
//! `mcp-contract.json` and requires the skills to stay inside it.
//!
//! **Nothing here is a list of skills, tools or plugins.** The marketplace names its plugins,
//! the plugin directory names its skills, and the contract names its tools. A new skill is
//! covered the day it is written, and a new tool fails this file until some skill says when to
//! reach for it — which is the conversation that should happen. The one place a number is
//! written down is the prose about the plugin, and the last two tests hold it to these same
//! sources rather than to somebody's memory.
//!
//! What is *not* checked here: that `/plugin marketplace add` succeeds. That needs Claude
//! Code, and `claude plugin validate --strict` is the tool for it. What is checked is every
//! precondition of it that lives in this repository.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

mod common;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root is readable")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

fn json(rel: &str) -> serde_json::Value {
    serde_json::from_str(&read(rel)).unwrap_or_else(|e| panic!("{rel} is not valid JSON ({e})"))
}

/// The marketplace manifest at the repository root — the file `/plugin marketplace add
/// goedelsoup/yidam` reads.
const MARKETPLACE: &str = ".claude-plugin/marketplace.json";

/// The page that carries the `/plugin marketplace add` instruction, and so the one a reader
/// meets the plugin on. Named once because three tests below read it.
const PLUGIN_DOC: &str = "docs/mcp-server.md";

/// Every `<dir>/SKILL.md` under a plugin's `skills/`, as (directory name, text).
///
/// Discovered rather than listed, for the reason in the module doc. Depth is fixed at one
/// because that is where Claude Code looks: a `SKILL.md` nested deeper is not loaded, and
/// finding one here would be a finding rather than a skill.
fn skills(plugin: &Path) -> Vec<(String, String)> {
    let dir = plugin.join("skills");
    let mut found: Vec<(String, String)> = common::repo_walk(&dir)
        .filter(|e| e.depth() == 1)
        .filter(|e| e.file_type().is_dir())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path().join("SKILL.md");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                panic!(
                    "{} is a skill directory with no SKILL.md — Claude Code will not load it",
                    e.path().display()
                )
            });
            (name, text)
        })
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "{} holds no skills; the plugin is half of what it claims to be",
        dir.display()
    );
    found
}

/// The `key: value` pairs of a leading `---` YAML block. Flat by construction — a SKILL.md
/// frontmatter has no nesting, and reaching for a YAML parser here would accept documents
/// Claude Code does not.
fn frontmatter(text: &str) -> Vec<(String, String)> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Vec::new();
    };
    let Some(end) = rest.find("\n---") else {
        return Vec::new();
    };
    rest[..end]
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn field<'a>(fm: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fm.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Every backticked run in `text` that is a bare lowercase identifier.
///
/// The filter is what keeps this from being a general prose scan: `` `--select body` ``,
/// `` `class-unpopulated` `` and `` `.yidam/tonpa.toml` `` are not identifiers and are not
/// claims about the contract's vocabulary. What is left — `retrieve`, `absence`,
/// `declares_edges` — is exactly the set of names a skill is asserting the server uses.
fn code_identifiers(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut parts = text.split('`');
    // Odd-indexed parts are inside backticks; even-indexed are outside.
    let _ = parts.next();
    while let Some(inside) = parts.next() {
        if !inside.contains('\n')
            && !inside.is_empty()
            && inside.starts_with(|c: char| c.is_ascii_lowercase())
            && inside
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            out.insert(inside.to_string());
        }
        let _ = parts.next();
    }
    out
}

/// Every name RFC-0005's frozen contract defines: capabilities, tool names, the arguments
/// each takes, the fields each answers with, and the names its own prose writes in code font.
///
/// The last of those is deliberate rather than lax. `origin` and `target` are contract
/// concepts that appear only in the notes — `every result attributed by `origin`` — and a
/// vocabulary that admitted the schema keys but not the notes would fail a skill for using
/// the contract's own word for the thing.
fn contract_vocabulary() -> BTreeSet<String> {
    let contract = json("yidam/cli/mcp-contract.json");
    let mut names = BTreeSet::new();

    let obj = contract.as_object().expect("the contract is an object");
    for c in obj["capabilities"]
        .as_object()
        .expect("capabilities is an object")
        .keys()
    {
        names.insert(c.clone());
    }
    if let Some(d) = obj.get("description").and_then(|d| d.as_str()) {
        names.extend(code_identifiers(d));
    }

    for tool in obj["tools"].as_array().expect("tools is an array") {
        names.insert(
            tool["name"]
                .as_str()
                .expect("a tool has a name")
                .to_string(),
        );
        if let Some(d) = tool.get("description").and_then(|d| d.as_str()) {
            names.extend(code_identifiers(d));
        }
        if let Some(props) = tool
            .pointer("/inputSchema/properties")
            .and_then(|p| p.as_object())
        {
            for (key, prop) in props {
                names.insert(key.clone());
                if let Some(d) = prop.get("description").and_then(|d| d.as_str()) {
                    names.extend(code_identifiers(d));
                }
            }
        }
        if let Some(req) = tool
            .pointer("/response/required")
            .and_then(|r| r.as_array())
        {
            names.extend(req.iter().filter_map(|r| r.as_str()).map(str::to_string));
        }
        if let Some(notes) = tool.pointer("/response/notes").and_then(|n| n.as_str()) {
            names.extend(code_identifiers(notes));
        }
    }
    names
}

/// Every plugin the marketplace offers, as (entry name, plugin directory).
fn marketplace_plugins() -> Vec<(String, PathBuf)> {
    let manifest = json(MARKETPLACE);
    let entries = manifest["plugins"]
        .as_array()
        .expect("the marketplace lists plugins");
    assert!(
        !entries.is_empty(),
        "{MARKETPLACE} lists no plugins, so `/plugin install` from it can install nothing"
    );
    entries
        .iter()
        .map(|e| {
            let name = e["name"].as_str().expect("an entry has a name").to_string();
            let source = e["source"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}'s source is not a path in this repository"));
            (name, repo_root().join(source.trim_start_matches("./")))
        })
        .collect()
}

/// A marketplace entry that points at nothing installs nothing, and says so only to whoever
/// runs `/plugin install`.
#[test]
fn every_marketplace_entry_resolves_to_a_plugin() {
    for (name, dir) in marketplace_plugins() {
        let manifest = dir.join(".claude-plugin/plugin.json");
        assert!(
            manifest.is_file(),
            "the marketplace offers `{name}` from {}, which has no .claude-plugin/plugin.json",
            dir.display()
        );
        let text = std::fs::read_to_string(&manifest).expect("plugin manifest is readable");
        let plugin: serde_json::Value =
            serde_json::from_str(&text).expect("the plugin manifest is valid JSON");
        assert_eq!(
            plugin["name"].as_str(),
            Some(name.as_str()),
            "the marketplace calls it `{name}` and {} calls itself `{}` — `/plugin install` \
             resolves by the marketplace's name and the runtime loads by the manifest's",
            manifest.display(),
            plugin["name"]
        );
    }
}

/// A skill whose frontmatter `name` is not its directory name is loaded under one name and
/// referred to by the other.
#[test]
fn every_skill_declares_the_name_of_its_own_directory() {
    for (_, dir) in marketplace_plugins() {
        for (name, text) in skills(&dir) {
            let fm = frontmatter(&text);
            assert!(
                !fm.is_empty(),
                "skills/{name}/SKILL.md has no YAML frontmatter; Claude Code will not load it"
            );
            assert_eq!(
                field(&fm, "name"),
                Some(name.as_str()),
                "skills/{name}/SKILL.md declares name `{}`",
                field(&fm, "name").unwrap_or("<missing>")
            );
            let description = field(&fm, "description").unwrap_or_default();
            assert!(
                description.len() > 40,
                "skills/{name}/SKILL.md has no usable `description`. The description is the \
                 whole trigger — a skill nothing fires is a skill nobody installed."
            );
        }
    }
}

/// The plugin's skills are authored, not copied out of the prelude.
///
/// `yidam/prelude/skills/` holds `bootstrap.md`, an agent prompt for an **empty** repository
/// that is actively wrong to load into a corpus that already exists — `.claude/CLAUDE.md`
/// routes to it on an empty `git log` for exactly that reason. Shipping the prelude's skills
/// directory into a plugin that installs into working repositories would ship that.
///
/// The assertion is disjointness rather than a ban on one filename, so a second prelude skill
/// cannot be swept in by having a name nobody thought to forbid. If one should travel, it is
/// a decision to write down, and this is where it gets made.
#[test]
fn no_plugin_skill_shares_a_name_with_a_prelude_skill() {
    let prelude: BTreeSet<String> = common::repo_walk(&repo_root().join("yidam/prelude/skills"))
        .filter(|e| e.depth() == 1)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    assert!(
        prelude.contains("bootstrap"),
        "the prelude skills directory no longer holds `bootstrap` — this test is looking in \
         the wrong place and asserting nothing"
    );

    for (_, dir) in marketplace_plugins() {
        for (name, _) in skills(&dir) {
            assert!(
                !prelude.contains(&name),
                "the plugin ships a skill named `{name}`, which is also a prelude skill. The \
                 prelude's are written for a repository being bootstrapped; the plugin installs \
                 into one that already exists."
            );
        }
    }
}

/// The assertion this file exists for: a skill may not name a thing the server does not have.
///
/// These skills are deliberately thin — they carry *when to ask*, and the tools carry the
/// answer — which means almost every identifier in them is a claim about the frozen contract.
/// A tool renamed in RFC-0005 leaves the old name sitting in prose that still reads perfectly,
/// and an agent following it gets `unknown tool` at the moment it was trying to comply.
#[test]
fn every_name_a_skill_writes_in_code_font_is_one_the_contract_defines() {
    let vocabulary = contract_vocabulary();
    assert!(
        vocabulary.contains("check_subject"),
        "the contract vocabulary came back without the tools in it — the parse is wrong and \
         every assertion below is vacuous"
    );

    let mut unknown = Vec::new();
    for (_, dir) in marketplace_plugins() {
        for (name, text) in skills(&dir) {
            for ident in code_identifiers(&text) {
                if !vocabulary.contains(&ident) {
                    unknown.push(format!("  skills/{name}/SKILL.md — `{ident}`"));
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "a skill writes a name the MCP contract does not define:\n{}\n\
         Either the contract moved and the skill did not, or the name is a typo. Check it \
         against yidam/cli/mcp-contract.json.",
        unknown.join("\n")
    );
}

/// The other direction: a tool nothing tells an agent when to reach for.
///
/// The plugin's premise is that the practice is callable *at the point the decision is made*,
/// and a tool no skill mentions is one an agent reaches by having remembered — which is the
/// state #422 was filed about. This is why a new tool fails here: adding one to RFC-0005
/// without deciding where in the loop it belongs is the omission, not the test.
#[test]
fn every_contract_tool_is_named_by_some_skill() {
    let contract = json("yidam/cli/mcp-contract.json");
    let tools: Vec<String> = contract["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .map(|t| t["name"].as_str().expect("a tool has a name").to_string())
        .collect();
    assert!(
        tools.len() > 5,
        "the contract parse found {} tools",
        tools.len()
    );

    let mut mentioned = BTreeSet::new();
    for (_, dir) in marketplace_plugins() {
        for (_, text) in skills(&dir) {
            mentioned.extend(code_identifiers(&text));
        }
    }

    let missing: Vec<&String> = tools.iter().filter(|t| !mentioned.contains(*t)).collect();
    assert!(
        missing.is_empty(),
        "no skill says when to reach for: {missing:?}\n\
         The plugin exists to put the tools at the point of decision. A tool no skill names \
         is one an agent has to remember."
    );
}

/// `.mcp.json` names a launcher, and a launcher that is not there is a server that never
/// starts — reported to the user as a failed MCP server and nothing more.
#[test]
fn the_mcp_manifest_names_an_executable_launcher() {
    for (name, dir) in marketplace_plugins() {
        let manifest = dir.join(".mcp.json");
        assert!(
            manifest.is_file(),
            "`{name}` declares an MCP server nowhere; the plugin installs skills and no server"
        );
        let text = std::fs::read_to_string(&manifest).expect(".mcp.json is readable");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect(".mcp.json is valid JSON");
        // Both shapes are in the wild — a bare map of servers, and one under `mcpServers`.
        let servers = value
            .get("mcpServers")
            .unwrap_or(&value)
            .as_object()
            .expect(".mcp.json holds an object of servers");
        assert!(
            !servers.is_empty(),
            "`{name}`'s .mcp.json declares no server"
        );

        for (server, config) in servers {
            let command = config["command"]
                .as_str()
                .unwrap_or_else(|| panic!("server `{server}` has no command"));
            let Some(rel) = command.strip_prefix("${CLAUDE_PLUGIN_ROOT}/") else {
                // A server naming a bare command is depending on a PATH this repository
                // cannot check, which is the choice #422 decided against.
                panic!(
                    "server `{server}` runs `{command}`, which is not inside the plugin. A \
                     command resolved from PATH fails silently when it is absent."
                );
            };
            let launcher = dir.join(rel);
            assert!(
                launcher.is_file(),
                "server `{server}` runs {}, which does not exist",
                launcher.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = launcher
                    .metadata()
                    .expect("launcher metadata")
                    .permissions()
                    .mode();
                assert!(
                    mode & 0o111 != 0,
                    "{} is not executable ({mode:o}); the client cannot spawn it",
                    launcher.display()
                );
            }
        }
    }
}

/// The launcher's whole value is what it says when it refuses, so what it says has to be true.
///
/// It names an install line and two subcommands. An install line that has moved on is worse
/// than no message — the person follows it and it fails — and a subcommand that was renamed
/// sends them to a `yidam --help-all` that does not list it.
///
/// `--help-all` is the whole surface; `--help` has been the short listing since #921, and
/// `overlay` — one of the three names checked below — is not on it.
#[test]
fn the_launcher_prescribes_commands_that_exist() {
    let installation = read("docs/installation.md");
    let help = {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
            .arg("--help-all")
            .output()
            .expect("running yidam --help-all");
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    for (name, dir) in marketplace_plugins() {
        for entry in common::repo_walk(&dir.join("scripts")).filter(|e| e.file_type().is_file()) {
            let text = std::fs::read_to_string(entry.path()).expect("a script is readable");
            for line in text.lines().map(str::trim) {
                if let Some(install) = line.strip_prefix("\"    ") {
                    let install = install.trim_end_matches("\" \\").trim_end_matches('"');
                    if install.starts_with("curl ") || install.starts_with("brew ") {
                        assert!(
                            installation.contains(install),
                            "`{name}`'s launcher tells a reader to run\n    {install}\n\
                             which does not appear in docs/installation.md"
                        );
                    }
                }
            }
            for sub in ["clone", "overlay", "serve"] {
                if text.contains(&format!("yidam {sub}")) {
                    assert!(
                        help.contains(sub),
                        "`{name}`'s launcher names `yidam {sub}`, which the binary does not list"
                    );
                }
            }
        }
    }
}

/// A distribution channel nothing documents is one nobody finds.
///
/// This is `install_channels.rs`'s thesis one channel later. That file cannot cover this one —
/// its channels all fetch a binary and are probed by running them in a clean container, and
/// `/plugin marketplace add` needs Claude Code — so the part of it that *is* checkable here is
/// that the instruction exists and names the marketplace this repository actually publishes.
#[test]
fn the_plugin_is_reachable_from_the_documentation() {
    let marketplace = json(MARKETPLACE);
    let name = marketplace["name"]
        .as_str()
        .expect("the marketplace is named");

    let docs = read(PLUGIN_DOC);
    assert!(
        docs.contains("/plugin marketplace add"),
        "docs/mcp-server.md is the document about connecting an agent and does not mention \
         the plugin. Installing it is then something a reader has to already know about."
    );
    for (plugin, _) in marketplace_plugins() {
        assert!(
            docs.contains(&format!("{plugin}@{name}")),
            "docs/mcp-server.md never writes `{plugin}@{name}`, which is the id \
             `/plugin install` takes"
        );
    }
}

// ── what the prose counts ─────────────────────────────────────────────────────
//
// Everything above derives its vocabulary from the contract and the skills directory, which
// is what lets a sixth skill be covered on the day it is written. The prose that *describes*
// the plugin was not derived from anything, and #943 is what that cost: the two manifest
// descriptions read "thirteen MCP tools over the corpus, and five skills", the skills
// directory held six, and `starting-a-session` — the one it shipped without counting — was
// also the one the README table omitted. A marketplace description is the only thing a
// reader sees before installing, and it was the last thing anybody checked.
//
// The two tests below are the same trick as the rest of the file applied to sentences: the
// numbers are computed, and the prose is held to them.

/// The English numerals this repository's prose reaches for.
///
/// Digits are deliberately absent. Nothing here writes "13 skills", and a scanner that took
/// both would be reading version numbers, node counts and issue numbers as claims about what
/// the plugin ships.
const NUMERALS: &[(&str, usize)] = &[
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
];

/// What the repository can actually produce a count of.
struct Shipped {
    skills: usize,
    /// Every name RFC-0005 froze, act tier included.
    tools: usize,
    /// The tools a server serves without being told it may write — what an install gets.
    read: usize,
    /// How many tools sit at each tier, keyed by the tier's own name.
    ///
    /// Prose about this server counts by tier as often as it counts the whole surface — *"the
    /// four tools at that tier"* — so a rule that knew only the total and the read set would
    /// be wrong about sentences that are right.
    tiers: BTreeMap<String, usize>,
}

fn shipped() -> Shipped {
    let contract = json("yidam/cli/mcp-contract.json");
    let tools = contract["tools"].as_array().expect("tools is an array");
    let mut tiers: BTreeMap<String, usize> = BTreeMap::new();
    for tool in tools {
        let tier = tool["tier"].as_str().expect("a tool declares a tier");
        *tiers.entry(tier.to_string()).or_default() += 1;
    }
    let act = tiers.get("act").copied().unwrap_or(0);
    let skills = marketplace_plugins()
        .iter()
        .map(|(_, dir)| skills(dir).len())
        .sum();
    Shipped {
        skills,
        tools: tools.len(),
        read: tools.len() - act,
        tiers,
    }
}

/// Every `<numeral> skills` and `<numeral> [qualifier] tools` phrase in `text`, as
/// (phrase, the number written, the thing counted).
///
/// `qualifiers` is the set of words allowed between the numeral and the noun — `read`, `mcp`,
/// and every tier name the contract declares. It is passed in rather than written here so
/// that a tier added to RFC-0005 becomes a qualifier this scanner reads, rather than a
/// sentence it stops seeing.
///
/// The lookahead stops at one qualifier on purpose. *"Four of the thirteen read tools"* is one
/// claim — that there are thirteen read tools — and not a second one about four of anything.
/// A singular noun is not matched at all: *"one field, on one tool"* counts nothing.
fn counted_claims(text: &str, qualifiers: &BTreeSet<String>) -> Vec<(String, usize, String)> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect();

    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let Some(&(_, written)) = NUMERALS.iter().find(|(n, _)| n == word) else {
            continue;
        };
        let next = words.get(i + 1).map(String::as_str);
        let after = words.get(i + 2).map(String::as_str);
        let (modifier, noun) = match (next, after) {
            (Some(n @ ("tools" | "skills")), _) => ("", n),
            (Some(m), Some(n @ ("tools" | "skills"))) if qualifiers.contains(m) => (m, n),
            _ => continue,
        };
        let counted = format!("{modifier} {noun}").trim().to_string();
        let phrase = format!("{word} {counted}");
        out.push((phrase, written, counted));
    }
    out
}

/// The words `counted_claims` will read as a qualifier.
fn qualifiers(s: &Shipped) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = s.tiers.keys().cloned().collect();
    out.insert("read".into());
    out.insert("mcp".into());
    out
}

/// The numbers a phrase is allowed to be, or `None` if this file has no opinion about it.
///
/// A bare *"tools"* admits every count the contract can produce — the frozen total, the set an
/// install serves, and each tier — because all of them are things prose about this server
/// legitimately means, and a sentence naming a tier two clauses earlier reads perfectly
/// without repeating it. That is looser than pinning one number, and it is still not nothing:
/// the day a tool is added to RFC-0005, the stale count falls out of the set that admitted it.
/// A qualified phrase is pinned, because the qualifier says which set is meant.
fn permitted(counted: &str, s: &Shipped) -> Option<Vec<usize>> {
    if let Some(tier) = counted.strip_suffix(" tools") {
        // `MCP tools` is the manifests' phrase, and a manifest describes an install. An
        // install serves the read tools; `act` is a key the *corpus* writes in its own
        // config, so a description promising that tier promises what the plugin cannot give.
        if tier == "read" || tier == "mcp" {
            return Some(vec![s.read]);
        }
        return s.tiers.get(tier).map(|n| vec![*n]);
    }
    Some(match counted {
        "skills" => vec![s.skills],
        "tools" => [s.tools, s.read]
            .into_iter()
            .chain(s.tiers.values().copied())
            .collect(),
        // A qualifier on `skills` — there is one set of skills and no tiers over it — is a
        // sentence this file has nothing to say about.
        _ => return None,
    })
}

/// The prose that describes the plugin may only count to numbers this repository produces.
///
/// The scanned surface is the plugin as a reader meets it: the marketplace entry, the plugin
/// manifest, the plugin's own README, and the page the marketplace instruction lives on. All
/// four are discovered from the marketplace rather than listed, except the last, which
/// [`the_plugin_is_reachable_from_the_documentation`] already pins by name.
#[test]
fn every_count_the_plugin_prose_writes_is_one_the_repository_produces() {
    let s = shipped();
    assert!(
        s.skills > 1 && s.read > 1 && s.tiers.len() > 1,
        "the counts came back as skills={}, read={}, tiers={:?} — the derivation is wrong and \
         every assertion below is meaningless",
        s.skills,
        s.read,
        s.tiers
    );
    let qualifiers = qualifiers(&s);
    // The scanner, not the prose, is what goes vacuous silently: prose that stops counting is
    // a decision somebody made and can be read in a diff, and a parse that stops matching is
    // nothing at all.
    let probe = counted_claims(
        "four of the thirteen read tools, six skills, and two act tools",
        &qualifiers,
    );
    assert_eq!(
        probe.len(),
        3,
        "counted_claims no longer reads the phrases it was written for: {probe:?}"
    );

    let marketplace = json(MARKETPLACE);
    let mut prose: Vec<(String, String)> = Vec::new();
    for entry in marketplace["plugins"]
        .as_array()
        .expect("plugins is an array")
    {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        if let Some(d) = entry.get("description").and_then(|d| d.as_str()) {
            prose.push((format!("{MARKETPLACE} ({name})"), d.to_string()));
        }
    }
    for (name, dir) in marketplace_plugins() {
        let manifest = dir.join(".claude-plugin/plugin.json");
        let plugin: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest).expect("manifest readable"))
                .expect("the plugin manifest is valid JSON");
        if let Some(d) = plugin.get("description").and_then(|d| d.as_str()) {
            prose.push((format!("{name}'s plugin.json"), d.to_string()));
        }
        let readme = dir.join("README.md");
        if let Ok(text) = std::fs::read_to_string(&readme) {
            prose.push((format!("{name}'s README.md"), text));
        }
    }
    prose.push((PLUGIN_DOC.into(), read(PLUGIN_DOC)));

    let mut wrong = Vec::new();
    for (where_, text) in &prose {
        for (phrase, written, counted) in counted_claims(text, &qualifiers) {
            let Some(allowed) = permitted(&counted, &s) else {
                continue;
            };
            if !allowed.contains(&written) {
                let numeral = NUMERALS
                    .iter()
                    .find(|(_, n)| Some(n) == allowed.first())
                    .map(|(w, _)| *w)
                    .unwrap_or("<out of range>");
                wrong.push(format!(
                    "  {where_} — `{phrase}`, and there are {allowed:?} ({numeral})"
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the plugin's prose counts something that is no longer true:\n{}\n\
         The skills directory and yidam/cli/mcp-contract.json are what these numbers are \
         read from. Correct the sentence, or drop the number from it — a description that \
         counts nothing is not checked here.",
        wrong.join("\n")
    );
}

/// A skill that neither document mentions is one only its own directory knows about.
///
/// This is #943's other half, and the more expensive one. The counts were wrong *because*
/// `starting-a-session` was written, shipped and never written down: it was absent from the
/// README's table of what the plugin installs *and* from the page that tells a reader to
/// install it, so the only way to learn the plugin had it was to list the directory. A skill
/// nobody documents is a skill nobody reviews.
///
/// Both documents, because they answer to different readers and neither substitutes for the
/// other: [`PLUGIN_DOC`] is where somebody decides whether to install this at all, and the
/// README is what they find afterwards. #943 left `docs/mcp-server.md` counting *"six
/// skills"* and then enumerating them by position — *"the fifth fires… the sixth fires…"* —
/// which the count gate cannot read. A seventh skill would have left that sentence wrong
/// with every test green. Naming the skills is what makes the claim checkable at all.
///
/// These are documents, not manifests, so this asks only that the name appear. Where it
/// appears — a table row, a sentence, a heading — is an editorial choice, and one a test has
/// no business making.
#[test]
fn every_skill_the_plugin_ships_is_named_where_the_plugin_is_documented() {
    let doc = read(PLUGIN_DOC);
    for (name, dir) in marketplace_plugins() {
        let readme = dir.join("README.md");
        let text = std::fs::read_to_string(&readme).unwrap_or_else(|e| {
            panic!(
                "`{name}` ships skills and {} is unreadable ({e})",
                readme.display()
            )
        });
        for (where_, prose) in [
            (readme.display().to_string(), &text),
            (PLUGIN_DOC.into(), &doc),
        ] {
            let missing: Vec<String> = skills(&dir)
                .into_iter()
                .map(|(skill, _)| skill)
                .filter(|skill| !prose.contains(skill.as_str()))
                .collect();
            assert!(
                missing.is_empty(),
                "`{name}` installs {missing:?}, which {where_} never names. A skill a document \
                 omits is one a reader finds by listing the directory.",
            );
        }
    }
}
