//! The gates that read this repository, linked as one test binary (#1010).
//!
//! Every module here was once a file of its own under `tests/`, and so a crate of its own:
//! a separate link of everything it reaches, three feature sets per main run. None of them
//! needed what that bought. A gate that reads a file cannot be disturbed by another gate
//! reading a file, so the process isolation nextest gives each binary was isolating nothing.
//!
//! Measured locally (macOS, not the CI runner) on the `--all-features` tree at `-j 2`: the
//! 53 files that start no `yidam` process took 16.5s to build as separate binaries and 4.2s
//! as one, out of 31s for every test binary in the crate. Run together in one process they
//! passed unchanged; what the move needed was the path strings that named `tests/<gate>.rs`.
//!
//! What is here is the 47 of those 53 that are gates. The six left behind start the binary
//! through `common::Example::run`, or talk to a live service, and belong with the suites
//! that do.
//!
//! One module per former file, and each keeps its own `//!` doc, because that doc is where the
//! argument for its gate lives. Merging the bodies would have traded that for CI minutes.
//! A new gate that only reads files belongs here as a new `mod`, not as a new binary.

#[path = "../common/mod.rs"]
mod common;

mod bootstrap_kuten;
mod bootstrap_skill;
mod build_profile;
mod capability_discovery;
mod ci_reporting;
mod class_schemas;
mod cluster_image;
mod coverage_reporting;
mod degraded_reason_freeze;
mod deny_policy;
mod dependency_holds;
mod design_system;
mod design_tokens;
mod docs_site;
mod domain_parity_testkit;
mod edit_publish;
mod embed_parity;
mod feature_split;
mod formal_specs;
mod gluon_arm;
mod hook_claims;
mod index_workflow;
mod install_script;
mod installed_layout_links;
mod kuten_prohibitions;
mod light_build;
mod line_citations;
mod lint_levels;
mod lint_registry;
mod mcp_contract_digest;
mod mutation_residue;
mod packaging;
mod panic_paths;
mod parity_implementations;
mod policy_equivalence;
mod prelude_commit_vocabulary;
mod prelude_glossary;
mod prelude_rules_and_evidence;
mod publish_guard;
mod quality_surface;
mod recorded_output;
mod repo_walks;
mod scaffolded_config;
mod template_pin;
mod toolchain_pins;
mod versioning_layers;
mod withheld_pointers;
