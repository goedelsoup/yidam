//! The rule a repository writes about itself — RFC-0024.
//!
//! # Rust computes facts; Rego decides
//!
//! [`crate::cmd::lint::model`] already draws this line one level down — *"A check reports; it
//! does not decide what is acceptable"* — and this is the same split for the decisions a gate
//! makes. Walking the tree, parsing frontmatter, resolving routes and knowing which
//! directories a bundle carries are all facts, and none of them belongs in a policy. What a
//! policy receives is a finished description of one situation; what it returns is a verdict.
//!
//! The line is not always where it first looks. `holds_content` reads a directory, so it is a
//! fact and the binary computes it — but *whether a placeholder counts as material* is a
//! judgement and lives in `lib.rego`. RFC-0024 named it as a policy function; implementing it
//! moved the half of it that touches a filesystem.
//!
//! # Nothing above this module sees `regorus`
//!
//! Callers get [`Decision`] and [`Denial`]. That is deliberate rather than tidy: the engine is
//! an implementation choice made by measurement (#438), and a caller that pattern-matched on
//! `regorus::Value` would make it a contract.
//!
//! # Hermetic by dependency, not by review
//!
//! `Cargo.toml` takes `regorus` with `default-features = false`, so the `http` and `time`
//! builtin families are not compiled in and a committed policy cannot make a network call or
//! read a clock. CI stays hermetic and goldens stay stable, and the way to break that is a
//! diff in `Cargo.toml` rather than a diff in a `.rego` file.
//!
//! The catch, found by probing rather than by reading: an absent builtin is refused at
//! **evaluation**, not at parse. A policy calling `http.send` compiles clean and dies at the
//! moment a decision is needed, which for a gate is the worst available time. That is what
//! [`Policies::disallowed_builtins`] exists for — it is not the mechanism, it is *when you
//! find out*.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::Value as Json;

/// The default policy, compiled in.
///
/// These are the same bytes `yidam/prelude/policy/` holds, so the copy a reader inspects in
/// `.yidam/.vendor/prelude/policy/` after bootstrap is the rule that ran — one file, not two.
///
/// A hardcoded roster is exactly the shape that stops covering new files without ever going
/// red, so `tests` discovers the directory and asserts every `.rego` in it appears here.
const DEFAULT_POLICIES: &[(&str, &str)] = &[
    (
        "disclose/lib.rego",
        include_str!("../../../prelude/policy/disclose/lib.rego"),
    ),
    (
        "disclose/at_rest.rego",
        include_str!("../../../prelude/policy/disclose/at_rest.rego"),
    ),
    (
        "disclose/record.rego",
        include_str!("../../../prelude/policy/disclose/record.rego"),
    ),
    (
        "disclose/derived.rego",
        include_str!("../../../prelude/policy/disclose/derived.rego"),
    ),
];

/// The decisions this binary asks about, by name.
///
/// A decision is addressed as `family/name` and evaluates `data.yidam.<family>.<name>.decision`.
pub const DECISIONS: &[&str] = &["disclose/at_rest", "disclose/record", "disclose/derived"];

/// Builtin families this build does not carry, and must not appear to.
///
/// **Every Rego builtin that reads the world is namespaced.** `http.send`, `net.lookup_ip_addr`,
/// `time.now_ns`, `rand.intn`, `uuid.rfc4122`, `crypto.*`, `io.jwt.*`, `opa.runtime` — the
/// unqualified builtins (`count`, `sprintf`, `startswith`, `concat`) are pure by construction.
/// That is the assumption this list rests on, stated so it can be argued with: if a future Rego
/// grows an unqualified impure builtin, this check stops being sufficient and the feature
/// resolution in `Cargo.toml` is still what actually refuses it.
const DENIED_NAMESPACES: &[&str] = &[
    "crypto", "graphql", "http", "io", "net", "opa", "rand", "time", "urlquery", "uuid",
];

/// How long one decision may take before it is abandoned.
///
/// A committed policy is code a gate runs, and a gate does not get to hang CI. The number is
/// generous by three orders of magnitude for the rules here — the point is that a runaway
/// terminates, not that it terminates fast.
const EVAL_TIME_LIMIT: Duration = Duration::from_secs(5);

/// Work units between clock checks. Small enough that the limit is real, large enough that the
/// check is not the cost.
const EVAL_CHECK_INTERVAL: u32 = 10_000;

/// Where the rule that answered came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The default compiled into this binary.
    Inherited,
    /// A file in `.yidam/policy/`, which under RFC-0024's authoritative model supersedes the
    /// default — including by being more permissive.
    Local(PathBuf),
}

impl Origin {
    pub fn is_local(&self) -> bool {
        matches!(self, Origin::Local(_))
    }
}

/// One reason a decision refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    /// Stable identifier for the rule that fired. Renaming one changes what a consumer keys on.
    pub rule: String,
    /// Written for somebody reading a terminal. `Permission denied` is not a sentence anybody
    /// can act on; naming the path and the reason is.
    pub msg: String,
}

/// A verdict, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub allow: bool,
    /// Every reason, not the first one. `derived_may_push` returns the first intersecting path
    /// and this reports all of them, which is what somebody about to fix it needs.
    pub deny: Vec<Denial>,
}

/// The loaded rule set.
pub struct Policies {
    engine: regorus::Engine,
    origins: BTreeMap<String, Origin>,
    /// Every module in the engine, as (source path, text), for the builtin scan and for
    /// `policy check`'s override diff.
    sources: Vec<(String, String)>,
}

/// Shows what was loaded and from where, and not the engine.
///
/// Hand-written because `regorus::Engine` is an implementation choice this module exists to
/// keep private — a derived `Debug` would print the parse tree into a test failure and make
/// the engine part of what a reader depends on.
impl std::fmt::Debug for Policies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Policies")
            .field("origins", &self.origins)
            .field(
                "modules",
                &self.sources.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Policies {
    /// The default policy, then whatever `.yidam/policy/` overrides.
    ///
    /// **Local files are added first and the default fills in what they did not define.** Rego
    /// merges same-package files, so adding both copies of a package would not override — it
    /// would make `decision` a complete rule with two bodies and fail at evaluation with a
    /// conflict. Overriding means *not adding* the default for a package the repository has
    /// claimed, and the package name is what `add_policy` returns.
    pub fn load(root: &Path) -> Result<Self> {
        let mut engine = regorus::Engine::new();
        engine.set_execution_timer_config(regorus::utils::limits::ExecutionTimerConfig {
            limit: EVAL_TIME_LIMIT,
            check_interval: std::num::NonZeroU32::new(EVAL_CHECK_INTERVAL)
                .expect("EVAL_CHECK_INTERVAL is nonzero"),
        });

        let mut origins = BTreeMap::new();
        let mut sources = Vec::new();
        let mut claimed: BTreeMap<String, PathBuf> = BTreeMap::new();

        for path in local_policy_files(root) {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let name = path.to_string_lossy().to_string();
            let package = engine
                .add_policy(name.clone(), text.clone())
                .with_context(|| format!("{} does not compile", path.display()))?;
            claimed.insert(package, path.clone());
            sources.push((name, text));
        }

        for (name, text) in DEFAULT_POLICIES {
            let package =
                package_of(text).with_context(|| format!("{name} declares no package"))?;
            if claimed.contains_key(&package) {
                continue;
            }
            engine
                .add_policy(format!("<default>/{name}"), text.to_string())
                .with_context(|| format!("the compiled-in default {name} does not compile"))?;
            sources.push((format!("<default>/{name}"), text.to_string()));
        }

        for decision in DECISIONS {
            let package = format!("data.yidam.{}", decision.replace('/', "."));
            origins.insert(
                decision.to_string(),
                match claimed.get(&package) {
                    Some(p) => Origin::Local(p.clone()),
                    None => Origin::Inherited,
                },
            );
        }

        Ok(Self {
            engine,
            origins,
            sources,
        })
    }

    /// Where each decision's rule came from, in decision order.
    pub fn origins(&self) -> impl Iterator<Item = (&str, &Origin)> {
        self.origins.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn origin(&self, decision: &str) -> Option<&Origin> {
        self.origins.get(decision)
    }

    /// The compiled-in text of a default policy, for `policy check`'s override diff.
    pub fn default_source(name: &str) -> Option<&'static str> {
        DEFAULT_POLICIES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, t)| *t)
    }

    /// Every default policy, for a caller that wants to show what was superseded.
    pub fn defaults() -> &'static [(&'static str, &'static str)] {
        DEFAULT_POLICIES
    }

    /// Ask one decision about one situation.
    ///
    /// The three engine outcomes are not interchangeable and are not collapsed:
    ///
    /// - an object carrying `allow` and `deny` is an answer;
    /// - `Undefined` means the rule exists and did not fire, which for a `:=` rule with no body
    ///   means the policy is malformed;
    /// - an error means the policy could not answer at all — a missing package, a builtin this
    ///   build does not carry, or the execution timer.
    ///
    /// The last two are failures rather than permits. **Authoritative does not mean absence
    /// permits**: the policy text decides, and a policy that fails to answer is a failure.
    pub fn decide(&mut self, decision: &str, input: &Json) -> Result<Decision> {
        if !DECISIONS.contains(&decision) {
            bail!(
                "`{decision}` is not a decision this binary asks about ({})",
                DECISIONS.join(", ")
            );
        }
        let rule = format!("data.yidam.{}.decision", decision.replace('/', "."));
        self.engine.set_input(
            regorus::Value::from_json_str(&input.to_string())
                .context("the decision input is not valid JSON")?,
        );
        let value = self
            .engine
            .eval_rule(rule.clone())
            .with_context(|| format!("evaluating {rule}"))?;
        if value == regorus::Value::Undefined {
            bail!(
                "`{decision}` is undefined — the policy defines {rule} and it did not produce a \
                 value. A decision that cannot answer is a refusal to answer, not a permit"
            );
        }
        let json: Json = serde_json::to_value(&value)
            .with_context(|| format!("{rule} returned something that is not JSON"))?;
        parse_decision(decision, &json)
    }

    /// Calls to builtin families this build does not carry.
    ///
    /// Returns `(policy source path, dotted call target)`. Walks the parse tree rather than the
    /// text, because a policy's own comment explaining why `http.send` is forbidden would answer
    /// a grep exactly as well as a call would.
    pub fn disallowed_builtins(&self) -> Result<Vec<(String, String)>> {
        let ast: Json = serde_json::from_str(
            &self
                .engine
                .get_ast_as_json()
                .context("reading the policy parse tree")?,
        )
        .context("the policy parse tree is not valid JSON")?;

        let mut out = Vec::new();
        for module in ast.as_array().into_iter().flatten() {
            let path = module
                .get("source")
                .and_then(|s| s.get("file"))
                .and_then(Json::as_str)
                .unwrap_or("<unknown>")
                .to_string();
            // Only the parsed tree. `source.contents` is the file verbatim, and scanning it
            // would rediscover every comment.
            let Some(tree) = module.get("ast") else {
                continue;
            };
            let mut calls = Vec::new();
            collect_calls(tree, &mut calls);
            for target in calls {
                let namespace = target.split('.').next().unwrap_or_default();
                if DENIED_NAMESPACES.contains(&namespace) {
                    out.push((path.clone(), target));
                }
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Every loaded module, as (source path, text).
    pub fn sources(&self) -> &[(String, String)] {
        &self.sources
    }
}

/// `.yidam/policy/**/*.rego`, sorted, so a load is reproducible across machines.
fn local_policy_files(root: &Path) -> Vec<PathBuf> {
    let dir = crate::paths::yidam_policy_dir(root);
    let mut out: Vec<PathBuf> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "rego"))
        .collect();
    out.sort();
    out
}

/// The package a policy declares, as `add_policy` would report it.
///
/// Read from the text rather than by adding it to a throwaway engine, because this is asked
/// about the compiled-in defaults before anything has been added and the answer decides whether
/// to add them at all.
fn package_of(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("package "))
        .map(|p| format!("data.{}", p.trim()))
}

/// Turn `{allow, deny}` into a [`Decision`], refusing the shapes that are contradictions.
fn parse_decision(decision: &str, json: &Json) -> Result<Decision> {
    let allow = json
        .get("allow")
        .and_then(Json::as_bool)
        .with_context(|| format!("`{decision}` returned no boolean `allow`"))?;

    let mut deny = Vec::new();
    for d in json
        .get("deny")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let rule = d
            .get("rule")
            .and_then(Json::as_str)
            .with_context(|| format!("`{decision}` returned a denial with no `rule`"))?;
        let msg = d
            .get("msg")
            .and_then(Json::as_str)
            .with_context(|| format!("`{decision}` denial `{rule}` has no `msg`"))?;
        deny.push(Denial {
            rule: rule.to_string(),
            msg: msg.to_string(),
        });
    }
    // Sorted, because Rego sets have no order a caller may rely on and a golden needs one.
    deny.sort_by(|a, b| (&a.rule, &a.msg).cmp(&(&b.rule, &b.msg)));

    if allow && !deny.is_empty() {
        bail!(
            "`{decision}` allowed and refused at once ({} denial(s)). That is a contradiction in \
             the policy, and it is an error rather than a permit",
            deny.len()
        );
    }
    if !allow && deny.is_empty() {
        bail!(
            "`{decision}` refused and gave no reason. A refusal nobody can act on is not a \
             verdict this binary will render"
        );
    }
    Ok(Decision { allow, deny })
}

/// Every function call in a parse tree, as a dotted name.
///
/// A `Call`'s `fcn` is either a `Var` — an unqualified builtin like `startswith` — or a `RefDot`
/// chain, which is how `http.send` is represented.
fn collect_calls(v: &Json, out: &mut Vec<String>) {
    match v {
        Json::Object(map) => {
            if let Some(call) = map.get("Call") {
                if let Some(name) = call.get("fcn").and_then(flatten_ref) {
                    out.push(name);
                }
            }
            for child in map.values() {
                collect_calls(child, out);
            }
        }
        Json::Array(items) => {
            for child in items {
                collect_calls(child, out);
            }
        }
        _ => {}
    }
}

/// `Var{value}` → `value`; `RefDot{refr, field}` → `<flatten(refr)>.<field>`.
fn flatten_ref(v: &Json) -> Option<String> {
    if let Some(var) = v.get("Var") {
        return var.get("value").and_then(Json::as_str).map(str::to_string);
    }
    if let Some(dot) = v.get("RefDot") {
        let base = flatten_ref(dot.get("refr")?)?;
        // `field` is [span, name].
        let field = dot
            .get("field")?
            .as_array()?
            .iter()
            .find_map(Json::as_str)?;
        return Some(format!("{base}.{field}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn repo(private: &[(&str, bool)]) -> Json {
        json!({
            "is_private": false,
            "private_paths": private.iter()
                .map(|(p, holds)| json!({"path": p, "holds_content": holds}))
                .collect::<Vec<_>>(),
        })
    }

    fn record_input(rel: &str, redistributable: Option<bool>, private: &[(&str, bool)]) -> Json {
        let mut subject = json!({"rel": rel, "kind": "catalog"});
        if let Some(r) = redistributable {
            subject["redistributable"] = json!(r);
        }
        json!({"repo": repo(private), "subject": subject})
    }

    fn loaded() -> Policies {
        Policies::load(TempDir::new().unwrap().path()).expect("the compiled-in defaults load")
    }

    /// **The default is refusal.** A record that says nothing about redistribution is not a
    /// licence, and the first `vault push` anybody runs must not be one.
    #[test]
    fn a_record_that_says_nothing_is_not_pushed() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/record",
                &record_input(".yidam/catalog/pearl-2009.md", None, &[]),
            )
            .unwrap();
        assert!(!d.allow);
        assert_eq!(d.deny[0].rule, "unstated-redistribution");
        assert!(d.deny[0].msg.contains("does not say"), "{:?}", d.deny);
    }

    #[test]
    fn an_explicit_licence_is_what_permits_a_push() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/record",
                &record_input(".yidam/catalog/x.md", Some(true), &[]),
            )
            .unwrap();
        assert!(d.allow, "{d:?}");
        assert!(d.deny.is_empty());
    }

    #[test]
    fn an_explicit_refusal_is_reported_as_a_licence_and_not_as_an_omission() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/record",
                &record_input(".yidam/catalog/x.md", Some(false), &[]),
            )
            .unwrap();
        assert_eq!(d.deny[0].rule, "not-redistributable");
        assert!(d.deny[0].msg.contains("licensed to read, not to host"));
    }

    /// **The load-bearing guard.** A declared-private path refuses a push the licence would
    /// have allowed — and it is reported *instead of* the licence finding, not beside it,
    /// because that is the precedence `may_push` returns in.
    #[test]
    fn a_private_path_refuses_a_push_that_the_licence_would_have_allowed() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/record",
                &record_input("dossier/evidence.md", Some(true), &[("dossier", true)]),
            )
            .unwrap();
        assert!(!d.allow);
        assert_eq!(
            d.deny.len(),
            1,
            "one reason, the actionable one: {:?}",
            d.deny
        );
        assert_eq!(d.deny[0].rule, "private-path");
        assert!(d.deny[0].msg.contains("outlives the access"));
    }

    /// The trap: `dossiers` is a different directory and must not be swept in.
    #[test]
    fn privacy_matches_a_directory_prefix_and_not_a_name_prefix() {
        let mut p = loaded();
        for (rel, refused) in [
            ("dossier", true),
            ("dossier/a/b.md", true),
            ("dossiers/a.md", false),
            ("other/dossier.md", false),
        ] {
            let d = p
                .decide(
                    "disclose/record",
                    &record_input(rel, Some(true), &[("dossier", true)]),
                )
                .unwrap();
            assert_eq!(!d.allow, refused, "{rel}: {d:?}");
        }
    }

    /// A catalog record under a declared path is refused whether or not the directory holds
    /// anything else — `is_private` never consulted the filesystem, and this inherits that.
    #[test]
    fn a_record_is_judged_on_its_path_and_not_on_what_sits_beside_it() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/record",
                &record_input("dossier/x.md", Some(true), &[("dossier", false)]),
            )
            .unwrap();
        assert!(
            !d.allow,
            "a placeholder flag must not license the record: {d:?}"
        );
    }

    fn derived_input(kind: &str, sources: &[&str], private: &[(&str, bool)]) -> Json {
        json!({
            "repo": repo(private),
            "subject": {"kind": kind, "sources": sources},
        })
    }

    /// Intersection is tested both ways: a declared path inside a source directory, and a
    /// declared path that *contains* one.
    #[test]
    fn a_derived_artifact_is_refused_from_either_direction_of_overlap() {
        let mut p = loaded();
        let sources = [".yidam/corpus", ".yidam/catalog"];

        let inside = p
            .decide(
                "disclose/derived",
                &derived_input("index", &sources, &[(".yidam/corpus/secret", true)]),
            )
            .unwrap();
        assert!(!inside.allow, "a private path inside a source: {inside:?}");

        let containing = p
            .decide(
                "disclose/derived",
                &derived_input("index", &sources, &[(".yidam", true)]),
            )
            .unwrap();
        assert!(
            !containing.allow,
            "a private path containing a source: {containing:?}"
        );

        let disjoint = p
            .decide(
                "disclose/derived",
                &derived_input("index", &sources, &[("dossier", true)]),
            )
            .unwrap();
        assert!(disjoint.allow, "no overlap must not refuse: {disjoint:?}");
    }

    /// **The rule a naive transcription drops.** A declared directory holding nothing but a
    /// placeholder is not material, and refusing on it would make the feature unusable for a
    /// repository that declared its intent before it had anything to protect.
    #[test]
    fn a_placeholder_directory_does_not_refuse_a_derived_push() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/derived",
                &derived_input("bundle", &[".yidam/corpus"], &[(".yidam/corpus", false)]),
            )
            .unwrap();
        assert!(d.allow, "a placeholder must not refuse: {d:?}");
    }

    /// Every overlapping path is reported, not the first. Somebody about to fix this wants all
    /// of them, and `derived_may_push` could only ever name one.
    #[test]
    fn a_derived_refusal_names_every_private_path_it_encodes() {
        let mut p = loaded();
        let d = p
            .decide(
                "disclose/derived",
                &derived_input(
                    "bundle",
                    &[".yidam/corpus", ".yidam/catalog"],
                    &[(".yidam/corpus/a", true), (".yidam/catalog/b", true)],
                ),
            )
            .unwrap();
        assert!(!d.allow);
        assert_eq!(d.deny.len(), 2, "{:?}", d.deny);
    }

    #[test]
    fn at_rest_permits_declared_material_in_a_private_repository_and_refuses_it_in_a_public_one() {
        let mut p = loaded();
        let mut input = json!({"repo": repo(&[("dossier", true)])});

        let public = p.decide("disclose/at_rest", &input).unwrap();
        assert!(!public.allow, "{public:?}");
        assert_eq!(public.deny[0].rule, "private-material-in-public-repo");

        input["repo"]["is_private"] = json!(true);
        let private = p.decide("disclose/at_rest", &input).unwrap();
        assert!(private.allow, "{private:?}");
    }

    // ── the layer's own invariants ────────────────────────────────────────────

    /// A repository that has overridden nothing gets the compiled-in rule, and says so.
    #[test]
    fn a_repository_with_no_policy_directory_inherits_every_decision() {
        let p = loaded();
        assert_eq!(p.origins().count(), DECISIONS.len());
        assert!(p.origins().all(|(_, o)| *o == Origin::Inherited));
    }

    /// **Authoritative.** A local file supersedes the default for its package, including by
    /// being more permissive — and the origin records that it did.
    #[test]
    fn a_local_policy_supersedes_the_default_for_its_package() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("record.rego"),
            "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
        )
        .unwrap();

        let mut p = Policies::load(tmp.path()).unwrap();
        assert!(p.origin("disclose/record").unwrap().is_local());
        assert!(
            p.origin("disclose/derived").unwrap() == &Origin::Inherited,
            "overriding one decision must not disturb another"
        );

        // The record that the default refuses is now allowed. This is the authoritative model
        // working as decided, and #441 is what makes it visible.
        let d = p
            .decide(
                "disclose/record",
                &record_input("dossier/x.md", None, &[("dossier", true)]),
            )
            .unwrap();
        assert!(d.allow, "the local rule decides: {d:?}");
    }

    #[test]
    fn a_policy_that_does_not_compile_is_an_error_and_not_an_empty_rule_set() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("broken.rego"),
            "package yidam.disclose.record\n{{{\n",
        )
        .unwrap();
        let err = Policies::load(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("does not compile"), "{err}");
    }

    /// `Undefined` at a decision entrypoint is a refusal to answer, and refusing to answer is
    /// not a permit. This is the single sentence an implementer gets backwards.
    #[test]
    fn a_decision_that_does_not_fire_is_an_error_rather_than_a_permit() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("record.rego"),
            "package yidam.disclose.record\n\ndecision := x if { x := false }\n",
        )
        .unwrap();
        let mut p = Policies::load(tmp.path()).unwrap();
        let err = p
            .decide("disclose/record", &record_input("a.md", Some(true), &[]))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("undefined") || err.contains("no boolean `allow`"),
            "{err}"
        );
    }

    #[test]
    fn allowing_and_refusing_at_once_is_a_contradiction_and_not_a_permit() {
        let json = json!({"allow": true, "deny": [{"rule": "r", "msg": "m"}]});
        let err = parse_decision("disclose/record", &json)
            .unwrap_err()
            .to_string();
        assert!(err.contains("contradiction"), "{err}");
    }

    #[test]
    fn a_refusal_with_no_reason_is_refused_as_unrenderable() {
        let json = json!({"allow": false, "deny": []});
        let err = parse_decision("disclose/record", &json)
            .unwrap_err()
            .to_string();
        assert!(err.contains("gave no reason"), "{err}");
    }

    /// **The hermeticity claim, run rather than cited.** It is a claim about a cargo feature
    /// set, which is exactly the kind that rots silently when somebody enables a feature to fix
    /// something else.
    #[test]
    fn a_policy_cannot_reach_the_network_or_read_a_clock() {
        for (builtin, call) in [
            (
                "http.send",
                "http.send({\"method\": \"get\", \"url\": \"http://example.com\"})",
            ),
            ("time.now_ns", "time.now_ns()"),
        ] {
            let tmp = TempDir::new().unwrap();
            let dir = tmp.path().join(".yidam/policy");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("record.rego"),
                format!(
                    "package yidam.disclose.record\n\ndecision := {{\"allow\": true, \"deny\": \
                     [], \"x\": {call}}}\n"
                ),
            )
            .unwrap();
            let mut p = Policies::load(tmp.path()).unwrap();
            // `{:?}` and not `to_string()`: the engine's reason is the *cause*, and the
            // top-level context only says which rule was being evaluated.
            let err = format!(
                "{:?}",
                p.decide("disclose/record", &record_input("a.md", Some(true), &[]))
                    .unwrap_err()
            );
            assert!(
                err.contains("could not find function"),
                "{builtin} must not exist in this build: {err}"
            );
        }
    }

    /// And the same call is found at *check* time, which is the point of the scan: an absent
    /// builtin is otherwise refused at the moment a decision is needed.
    #[test]
    fn a_disallowed_builtin_is_found_before_any_decision_is_made() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("record.rego"),
            "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": [], \"x\": \
             http.send({})}\n",
        )
        .unwrap();
        let found = Policies::load(tmp.path())
            .unwrap()
            .disallowed_builtins()
            .unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].1, "http.send");
    }

    /// A comment naming a forbidden builtin is prose, not a call. The scan walks the parse tree
    /// precisely so that the default policy can explain itself without tripping its own guard.
    #[test]
    fn a_comment_mentioning_a_forbidden_builtin_is_not_a_call() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".yidam/policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("record.rego"),
            "package yidam.disclose.record\n\n# never call http.send or time.now_ns here\ndecision \
             := {\"allow\": true, \"deny\": []}\n",
        )
        .unwrap();
        let found = Policies::load(tmp.path())
            .unwrap()
            .disallowed_builtins()
            .unwrap();
        assert!(found.is_empty(), "{found:?}");
    }

    /// The shipped default must pass its own check.
    #[test]
    fn the_compiled_in_default_calls_nothing_this_build_forbids() {
        assert!(loaded().disallowed_builtins().unwrap().is_empty());
    }

    /// **Discovered, not transcribed.** A hardcoded roster stops covering new files without ever
    /// going red — a `.rego` added to the prelude and not embedded here is a rule that ships in
    /// the template and never runs.
    #[test]
    fn every_default_policy_on_disk_is_compiled_into_the_binary() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/policy");
        let mut on_disk: Vec<String> = walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| p.extension().is_some_and(|x| x == "rego"))
            .map(|p| {
                p.strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        on_disk.sort();
        assert!(!on_disk.is_empty(), "{} holds no policy", dir.display());

        let mut embedded: Vec<String> = DEFAULT_POLICIES
            .iter()
            .map(|(n, _)| n.to_string())
            .collect();
        embedded.sort();
        assert_eq!(on_disk, embedded, "the prelude and the binary disagree");
    }

    #[test]
    fn an_unknown_decision_is_named_rather_than_silently_allowed() {
        let err = loaded()
            .decide("disclose/nope", &json!({}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("is not a decision"), "{err}");
    }
}
