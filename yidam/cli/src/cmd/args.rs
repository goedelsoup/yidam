//! The flags most commands share, declared once for every command that takes them.
//!
//! These lived in `main.rs` while every command's arguments did. A command whose arguments
//! are a `clap::Args` struct in its own module (#1629) flattens them from here, and the
//! binary imports the same two structs for the variants that are still declared inline, so
//! there is one declaration of each flag whichever side of that move a command is on.

use std::path::PathBuf;

/// `--format`, declared once for the thirty-two commands that take it (#927).
///
/// The flag is one contract — RFC-0016's report envelope, or the prose this CLI has always
/// printed — and it was written out thirty-two times, which is thirty-two chances for the
/// spelling, the default, or the sentence describing it to drift apart. They had already
/// begun to: three copies described the same flag in three different wordings.
///
/// `#[command(flatten)]` puts the declaration in one place and leaves `<command> --help`
/// unchanged, which matters more than the line count. `report_goldens.rs` builds its roster
/// by scanning each subcommand's help for a `--format` line; a refactor that moved the flag
/// out of that output would empty the roster without failing anything.
///
/// Two commands keep their own declaration because they say something the shared sentence
/// cannot: `pack`, whose `text` is the artefact rather than a rendering of it, and `replay`,
/// whose `json` carries rows the prose summarises. A flattened struct has no per-command
/// override, and inventing one to absorb two exceptions would cost more than the exceptions.
#[derive(clap::Args, Debug, Clone, Copy)]
pub struct FormatArg {
    /// Output format. `json` emits the machine-readable report contract
    /// (RFC-0016); `text` is unchanged and remains the default.
    //
    // `long` and `value_name` are spelled out because the field is not called `format`:
    // clap derives both from the field name, and `format.format` at thirty-two call sites
    // is the duplication this struct exists to remove said once more. A `///` here would
    // be a second paragraph of help, which promotes the whole thing to clap's long form and
    // prints this note to every reader of `yidam <command> --help`.
    #[arg(
        long = "format",
        value_name = "FORMAT",
        value_enum,
        default_value_t = crate::report::Format::Text
    )]
    pub value: crate::report::Format,
}

/// `--root`, declared once for every command that names a corpus (#918).
///
/// `serve` took this flag first (#421) and `export` second (#236, #428), and both wrote out
/// their own declaration with a paragraph of reasoning attached. The reasoning generalised
/// long before the flag did: *every* command resolved its corpus from the working directory,
/// so a corpus that lives inside another git repository — `examples/streamflow` in this one,
/// or any checkout inside a larger workspace — could be read only by copying it somewhere
/// else and running `git init` on the copy. `docs/quickstart.md` spent a paragraph of a
/// reader's first twenty minutes on exactly that, and the paragraph was about the tool rather
/// than about corpora.
///
/// What the flag *means* is [`crate::paths::resolve_root`]'s and is documented there — one
/// place, so the next command to take it cannot quietly disagree with the rest. What this
/// struct adds is that the flag is now *spelled* once: a copy of the `#[arg]` and its help
/// sentence per command is a chance per command for the value name or the wording to drift,
/// and the two copies that already existed had already begun to.
///
/// The commands that do **not** take it are the ones for which a corpus you are not standing
/// in is not a coherent thing to act on. Three kinds: those that author a commit into the
/// corpus, those that write a build artefact into its `.yidam/`, and those that derive or
/// maintain a repository and already name their target positionally. They refuse `--root` as
/// an unknown argument, which is clap's own answer and needs no code here.
///
/// **The REGEN generators are on this side of the line, not that one**, although every one of
/// them writes a tracked file. `yidam regen` runs the whole list in one pass, so a generator
/// that refused the flag would be a block `regen --root X` refreshes and `<generator> --root X`
/// will not — and `regen` would then be writing into a corpus through commands that deny they
/// can. The write is a block regenerated from what the corpus already says, which is a report
/// that happens to be stored; that is a different act from authoring a commit.
///
/// `root_flag.rs` is what keeps that division honest, and it checks the half a reviewer
/// cannot: for every command whose `--help` lists the flag, that passing it actually reads
/// the named corpus. A declaration that is never threaded through to `resolve_root` compiles,
/// accepts the flag, and reports on the wrong repository — which is the silent failure
/// `export` was measured against in #428 and the only one worth a gate.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct RootArg {
    /// The corpus to act on. Defaults to `YIDAM_ROOT`, then to wherever this process started.
    //
    // `long` and `value_name` are spelled out for the reason `FormatArg` gives: the field is
    // not called `root`, because `root.root` at thirty-nine call sites is the duplication
    // this struct exists to remove, said once more. A `///` here would be a second paragraph
    // of help, which promotes the whole thing to clap's long form and prints this note to
    // every reader of `yidam <command> --help`.
    #[arg(long = "root", value_name = "DIR")]
    dir: Option<PathBuf>,
}

impl RootArg {
    /// The flag as every command function takes it, and as `resolve_root` reads it.
    pub fn as_deref(&self) -> Option<&std::path::Path> {
        self.dir.as_deref()
    }
}
