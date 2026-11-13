//! The tests that start the `yidam` binary, linked as one test binary (#1488).
//!
//! Every module here was once a file of its own under `tests/`, and so a crate of its own:
//! a separate link of the whole library, after every edit to it, three feature sets per
//! `ci-cli-full`. [`gates`](../gates/main.rs) did this for the files that only read the
//! repository (#1010) and left these behind because they spawn the binary. That turned out
//! not to be a reason: nextest runs every test in a process of its own whichever binary holds
//! it, and none of these sets an environment variable, changes directory or keeps state in a
//! `static` that another module reads.
//!
//! Measured on macOS, one edit to a core source file then `cargo test --no-run`: 18–21s wall
//! and 114–133s of CPU across 83 units before, 7.8s and 22s across 7 after.
//!
//! Under plain `cargo test` these do share a process, as threads. A test that needs process
//! state of its own belongs in `tests/gates/test_binaries.rs`'s `OWN_BINARY`, with the
//! reason, rather than back at the top of `tests/`.
//!
//! One module per former file, each keeping its `//!` doc, which is where its argument lives.
//! A new test that starts the binary belongs here as a new `mod`. To run one module's tests:
//! `cargo test --test suite <module>::`, or `cargo nextest run -E 'binary(suite)'`.

#[path = "../common/mod.rs"]
mod common;

mod asks;
mod bench;
mod bench_baseline;
mod binary_pin;
mod build_features;
mod capability_run;
mod catalog_extract;
mod catalog_extract_members;
mod catalog_fetch;
mod class_derivation;
mod claude_plugin;
mod cli_reference;
mod cluster_run;
mod cohort;
mod commit_registers;
mod computed_signals;
mod count;
mod crates_index;
mod cycle_acts;
mod cycle_rounds;
mod dependency_corpora;
mod derive_check;
mod derived_ci_binary;
mod derived_repo_smoke;
mod design_lint;
mod dispatch;
mod doctor;
mod domain_articles;
mod due;
mod editor_configs;
mod elector_commitments;
mod embed_dry_run;
mod example_corpus;
mod first_corpus;
mod gather;
mod git_hygiene;
mod git_spawns;
mod homebrew_formula;
mod index_verify;
mod init_corpus;
mod install_channels;
mod kuten_cluster;
mod library_surface;
mod mcp_act_tier;
mod mcp_serve;
mod mcpb_bundle;
mod migrate;
mod object_coupling;
mod phase_record;
mod policy;
mod propose;
mod query_goldens;
mod query_history;
mod reading_surface;
mod receipt_compat;
mod regen_check;
mod release_script;
mod repo_detection;
mod report_goldens;
mod retrieve_cli;
mod rfc_status;
mod root_flag;
mod run_route_claims;
mod s3vectors_live;
mod samudaya_examples;
mod schema_ownership;
mod score_range;
mod serve_http;
mod source_add;
mod source_check;
mod task_layer;
mod template_root;
mod template_source_packs;
mod vault;
mod vault_s3;
mod walkthrough_transcripts;
mod warehouse_export;
mod watch_workflow;
