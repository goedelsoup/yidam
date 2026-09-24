//! `yidam --help`, grouped — and honest about which commands write.
//!
//! Thirty-five subcommands printed in one undifferentiated list is the least structured
//! view of this tool that exists, and it was the first one a new user saw. Giving the blank
//! ones descriptions fixed legibility per line and did nothing about the wall. The README
//! has grouped them for a long time, and that grouping is genuinely useful; `--help` simply
//! did not carry it.
//!
//! # Why a table and not `help_heading`
//!
//! clap 4 groups *arguments* under headings and does not group subcommands. The two ways
//! out are a custom help template, or arranging subcommands into nested parents — which
//! would change the command line itself to fix its documentation. So: a template that omits
//! `{subcommands}`, and [`render`] supplying the grouped listing in its place.
//!
//! The listing is rendered **from clap's own metadata** — the names and `about` strings of
//! whatever subcommands this binary actually compiled — so a description is never
//! duplicated here and cannot drift from the one `yidam <cmd> --help` prints. [`GROUPS`]
//! carries only what clap does not know: which group a command belongs to, and whether it
//! writes.
//!
//! # The list that must not grow silently
//!
//! Thirty-five commands accumulated without anyone deciding they should, which is what
//! happens when adding one costs nothing and shows up nowhere. A command missing from
//! [`GROUPS`] now fails [`tests::every_subcommand_is_grouped`], so adding one means
//! deciding where it goes and whether it writes. That is the maintainable part — not the
//! rendering.
//!
//! # Two renderings, one list
//!
//! Grouping the fifty-eight did not make them fifty-eight fewer. `--help` ran to ninety
//! lines, and the eleven `<!-- REGEN -->` generators — commands a first reader will never
//! type — printed above `graph`, `retrieve` and `query`, which are the product (#921). A
//! reader opening `--help` to find out what this tool does read a maintenance surface
//! first.
//!
//! So [`render_short`] prints the commands a session usually needs, flat and in reading
//! order, and [`render`] — the grouped ninety lines, unchanged — moves to `--help-all`.
//! The short list is **not a second roster**: it is the [`Entry::short`] flag on entries
//! already in [`GROUPS`], so a command can be on it without existing twice, and the
//! existence and no-duplicate gates already cover it. Its order is [`GROUPS`]' own, which
//! is why that order is by what a reader wants first.
//!
//! A short list is only worth having while it is short, and nothing about printing thirteen
//! rows resists a fourteenth. [`tests::the_short_help_stays_short`] is the resistance.

use std::fmt::Write as _;

/// One command's placement.
pub struct Entry {
    /// The subcommand name, exactly as clap knows it.
    pub name: &'static str,
    /// Whether running it rewrites files in the repository it is pointed at.
    ///
    /// The test is **what it does by default**, not what any flag can make it do. `lint`
    /// reads; `lint --bless` rewrites the baseline, and that belongs in `lint`'s long help
    /// rather than in a marker on every invocation. Marking every command that *could*
    /// write would mark nearly all of them and tell a reader nothing.
    pub writes: bool,
    /// Whether it prints on the short `--help`, rather than only on `--help-all`.
    ///
    /// The test is **what a reader needs before they know the tool**, which is not the same
    /// as what anyone uses most. `regen` is on it and the eleven generators it runs are not:
    /// one row saying the blocks can be refreshed is the whole of what a first reader needs
    /// to know about that family, and eleven rows above `query` is what #921 measured.
    pub short: bool,
}

const fn r(name: &'static str) -> Entry {
    Entry {
        name,
        writes: false,
        short: false,
    }
}

const fn w(name: &'static str) -> Entry {
    Entry {
        name,
        writes: true,
        short: false,
    }
}

impl Entry {
    /// Put this command on the short `--help`.
    ///
    /// A method rather than a third and fourth constructor, so the two facts stay separable
    /// at the call site: `w("regen").short()` says it writes *and* that it leads, and
    /// neither reading depends on remembering which of four one-letter names means which
    /// pair.
    const fn short(self) -> Self {
        Self {
            name: self.name,
            writes: self.writes,
            short: true,
        }
    }
}

pub struct Group {
    pub title: &'static str,
    pub commands: &'static [Entry],
}

/// The groups, in the order they print.
///
/// Ordering is by what a reader is likely to want first, not alphabetically: the checks
/// answer "is something wrong", which is why anyone opens `--help` under pressure. The
/// README-block generators come second because they are ten of the thirty-five and are the
/// ten that write — putting them together is most of what this grouping buys.
pub const GROUPS: &[Group] = &[
    Group {
        title: "Checks and gates — read-only, and exit nonzero on a problem",
        commands: &[
            r("doctor").short(),
            r("graph-check").short(),
            r("lint").short(),
            r("index-verify"),
            r("samudaya-audit"),
        ],
    },
    Group {
        // Its own group, and beside the gates rather than in them. `due` reads clocks and
        // exits zero however much is owed: a corpus with three expired sources is doing
        // exactly what it is meant to do and is simply owed a look. Filing it under "exit
        // nonzero on a problem" would teach a reader that being owed is a defect, which is
        // the one reading this report exists to prevent.
        title: "The practice — what is owed, which is not what is wrong",
        // `kuten` belongs here and not with the README generators, though it writes one of
        // their blocks. What it reports is the same kind of thing `due` reports: a question
        // for a person about how the practice is going. `kuten check` exits zero however far
        // a corpus has drifted, and filing it under the gates would teach a reader that
        // having drifted is a defect — which is the one reading it exists to prevent.
        // `score` belongs here for the same reason, one unit of work in. It reads a range
        // against declared criteria and reports a row each; it has no verdict to give and
        // exits zero however it reads, so filing it under the gates would teach a reader
        // that a low reading is a defect — which is the one thing it must not say.
        // `cycle` is here and not with the gates for the reason the group title gives. It
        // composes `due`, `phases`, `lint` and `graph-check` into one reading and exits zero
        // however much is owed; two of the four surfaces it reads *are* gates, and filing the
        // composition under them would make the whole report inherit a verdict that only a
        // quarter of it has.
        // `practice` writes a REGEN block and still belongs here, beside `kuten`, for the
        // same reason: what it writes is a reading of how the practice has gone, in `kuten
        // check`'s verdict vocabulary, and divergence in it is a question and not a defect.
        commands: &[
            r("due").short(),
            r("cycle"),
            w("kuten"),
            w("practice"),
            r("score"),
        ],
    },
    Group {
        title: "README blocks — each rewrites its <!-- REGEN --> block where it is run",
        commands: &[
            w("regen").short(),
            w("status").short(),
            w("open-questions"),
            w("corpus-index"),
            w("catalog-audit"),
            w("index-status"),
            w("agents-index"),
            w("skills-index"),
            w("crates-index"),
            w("packages-index"),
            w("bundle-status"),
            w("vault-status"),
        ],
    },
    Group {
        title: "The corpus and its history",
        commands: &[
            r("graph").short(),
            r("neighbors"),
            // Before `query` because it is the one a reader reaches for first: `retrieve`
            // finds where a subject is written about, and `query` walks from there. It had
            // no terminal route at all until #835 — the tool an agent uses before it knows
            // enough to write a query was the one surface a person could not try.
            r("retrieve").short(),
            r("query").short(),
            r("pack"),
            r("estimate"),
            r("diff"),
            // Beside `diff` rather than with the gates: it reads a code diff the way `diff`
            // reads a corpus one, and it cannot fail. A command filed under "exit nonzero on
            // a problem" would be read as one more thing that can break the build, and every
            // finding it has is a question somebody has to answer rather than a defect.
            r("check-diff"),
            w("rename").short(),
            // Beside `rename` and not with the gates: both move something and rewrite every
            // reference to it. `rename` moves one node; this moves a class, a property, or
            // the target of a relationship, which is the same operation one level up.
            w("migrate"),
            // Beside the corpus commands rather than with the gates: it reads the gate's
            // findings and answers none of them. A command filed under "exit nonzero on a
            // problem" would be read as one more thing that can fail the build, and this
            // one cannot — it drafts commits and leaves.
            w("propose").short(),
            // Beside `propose` rather than under the README generators, though both write.
            // What the generators rewrite is a block; what these two write is history, and
            // they are the only two commands here that do. The `*` says so for the same
            // reason `propose` carries it: a run does write to the repository, and its long
            // help says which part and what it leaves alone.
            w("run"),
            // Beside `run` and `propose`, the other two that write history. `phase` writes a
            // `scaffold:` commit per act and is the surface `phases` reads, so the pair sits
            // together: one opens and records the unit of inquiry, the other lists it.
            w("phase"),
            r("log"),
            r("phases"),
            r("replay"),
            // Beside `replay` because it is the same reading over a set rather than over
            // one repository — and in this group rather than under the gates because it
            // exits zero however much lost. A norm its derivations do not keep is a
            // question for whoever wrote it, and filing this under "exit nonzero on a
            // problem" would teach a reader that a repository diverging is a defect.
            r("cohort"),
            // Beside the history commands rather than under the README generators, for the
            // reason `kuten` gives one group up: what it lists is a practice's record, not an
            // index of files. But it is `w`, and was `r` until #831 — it rewrites the REGEN
            // block in `.yidam/decisions/README.md` whenever that file carries one, and no
            // corpus the prelude creates does, so the marker was wrong in the direction
            // nothing would notice.
            w("decisions-log"),
            r("sangha"),
            r("vocabulary"),
        ],
    },
    Group {
        title: "Index and embeddings",
        // `index-push` writes, and what it writes is not in this repository — it makes a
        // vector bucket equal to `.yidam/index/`, deletions included. The marker says a
        // command rewrites files in the repository it is pointed at, which this does not, so
        // it is `r`; the group title and its own help are where the writing is stated.
        commands: &[w("embed"), w("index-build"), r("index-push")],
    },
    Group {
        // Its own group rather than beside `tonpa`, which is the nearest neighbour and is
        // still a different thing: `tonpa` installs a corpus somebody else published, and
        // this keeps the bytes a corpus rests on or produces. Nor with the index commands,
        // which build an artifact rather than store one.
        title: "Artifacts — bytes kept outside git, addressed by content",
        // `catalog-fetch` sits here and not with the REGEN blocks beside `catalog-audit`,
        // because what it produces is bytes and a commit rather than a table: it follows an
        // entry's address, files what comes back under its digest, and records that. Its
        // nearest neighbour is `vault`, which is where those bytes then live.
        //
        // `catalog-reconcile` sits beside it because the pair is the point — one keeps what a
        // source gave, the other keeps what the corpus says about it — and separating them
        // would put the two halves of `catalog:`'s row in #460's table in different groups.
        commands: &[w("vault"), w("catalog-fetch"), w("catalog-reconcile")],
    },
    Group {
        // Its own group rather than beside the gates. Every command there answers *is this
        // corpus in the state it claims*; this one answers *what did this repository decide
        // the rule was*, which is a question about the gate rather than a use of it.
        title: "The rules this repository writes about itself",
        // `r`, and it was `w` until #831 found it — the group's title has the verb in it, and
        // what writes the rules is a person. `check`, `eval`, `gate` and `test` compile and
        // ask; not one of them opens a file for writing. A marker on a command that reads is
        // not a harmless surplus: it is the same claim as the marker on `run`, and it makes
        // the one that means something cheaper to ignore.
        commands: &[r("policy")],
    },
    Group {
        title: "Export",
        commands: &[w("export"), w("bundle"), w("schema")],
    },
    Group {
        title: "Serving the domain computer",
        commands: &[r("serve").short()],
    },
    Group {
        // Its own group rather than beside the gates: `bench` measures and does not gate,
        // and a measurement filed under "exit nonzero on a problem" would be read as one.
        title: "Measuring the corpus",
        commands: &[r("bench")],
    },
    Group {
        title: "Deriving and maintaining a repository",
        commands: &[w("clone").short(), w("overlay"), w("backfill"), w("tonpa")],
    },
];

/// Commands only some builds carry.
///
/// [`GROUPS`] is unconditional — a `#[cfg]` per entry would put the feature matrix in two
/// places — so the coverage test needs to know which absences are legitimate. Under
/// `--features full` every one of these is present and this list buys nothing; under the
/// light default it is the difference between a passing test and a false alarm.
///
/// `tonpa` was on this list until it joined the default set. It is off it now on purpose:
/// an entry here is a licence for a command to be missing, and `tonpa` is no longer
/// allowed to be. Anything that drops it from the build should fail this test.
#[cfg(test)]
const FEATURE_GATED: &[&str] = &["index-build", "index-push"];

/// The marker on a command that writes.
///
/// One character, because it has to survive being read at a glance in a list of thirty-five
/// — and ASCII, because this is the output most likely to be piped somewhere with an
/// opinion about encodings.
const WRITES: &str = "*";

/// Render the grouped listing for the subcommands this binary actually has.
///
/// `available` is `(name, about)` straight from clap. A command present in `available` and
/// absent from [`GROUPS`] is printed under a trailing group rather than dropped: a
/// subcommand invisible in `--help` is worse than an ugly one, and the test is what keeps
/// that path unreachable.
pub fn render(available: &[(String, String)]) -> String {
    let width = available
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    let about = |name: &str| {
        available
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, about)| about.as_str())
    };

    let mut out = String::new();
    let mut placed: Vec<&str> = Vec::new();
    for group in GROUPS {
        let rows: Vec<&Entry> = group
            .commands
            .iter()
            .filter(|e| about(e.name).is_some())
            .collect();
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}:", group.title);
        for entry in rows {
            placed.push(entry.name);
            out.push_str(&row(entry.name, entry.writes, about(entry.name), width));
        }
        out.push('\n');
    }

    let ungrouped: Vec<&(String, String)> = available
        .iter()
        .filter(|(n, _)| !placed.contains(&n.as_str()))
        .collect();
    if !ungrouped.is_empty() {
        out.push_str("Ungrouped:\n");
        for (name, about) in ungrouped {
            out.push_str(&row(name, false, Some(about), width));
        }
        out.push('\n');
    }

    out.push_str(&legend());
    out
}

/// Render the short listing: the commands a session usually needs, flat and in order.
///
/// Flat on purpose. The grouping in [`render`] earns its headings over fifty-eight rows and
/// would cost more than it buys over thirteen — four of the groups would print a single row
/// under a heading longer than the row. What the order carries instead is the sequence: ask
/// whether something is wrong, then what is owed, then read the corpus, then write to it,
/// then serve or derive. That is [`GROUPS`]' own order, so there is nothing here to keep in
/// step with it.
///
/// The count in the heading is `available.len()`, not a number written down. A listing that
/// advertises how much it is hiding has to be right about it, and the only way to stay right
/// is to ask.
pub fn render_short(available: &[(String, String)]) -> String {
    let about = |name: &str| {
        available
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, about)| about.as_str())
    };
    let rows: Vec<&Entry> = GROUPS
        .iter()
        .flat_map(|g| g.commands)
        .filter(|e| e.short && about(e.name).is_some())
        .collect();
    let width = rows.iter().map(|e| e.name.len()).max().unwrap_or(0);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "Commands — what a session usually needs. `yidam --help-all` lists all {}, grouped:",
        available.len()
    );
    for entry in rows {
        out.push_str(&row(entry.name, entry.writes, about(entry.name), width));
    }
    out.push('\n');
    out.push_str(&legend());
    out
}

/// What the `*` column means, printed under both listings.
///
/// A function and not a `const`, so the marker in the prose is the same [`WRITES`] the
/// column is drawn with. Two listings spelling the legend out twice is two places for it to
/// stop matching the thing it explains.
fn legend() -> String {
    format!(
        "  {WRITES} rewrites files in the repository it is run against. Everything else\n\
         \x20   only reads — `yidam <command> --help` says exactly what.\n"
    )
}

fn row(name: &str, writes: bool, about: Option<&str>, width: usize) -> String {
    format!(
        "  {name:<width$} {:<1} {}\n",
        if writes { WRITES } else { "" },
        about.unwrap_or_default()
    )
}

/// The help layout, with clap's flat `{subcommands}` replaced by [`render`]'s grouping.
///
/// `{after-help}` is where the listing lands, so the options block stays clap's own.
/// No newline before `{after-help}`: clap emits its own blank line ahead of it, and a
/// second one here is a gap nobody asked for. The one after it separates the legend from
/// the options block.
pub const TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}{after-help}
Options:
{options}";

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The names this binary actually carries, minus clap's own `help`, which belongs
    /// beside `-h` rather than in a group of corpus commands.
    fn compiled() -> Vec<(String, String)> {
        crate::Cli::command()
            .get_subcommands()
            .filter(|c| c.get_name() != "help")
            .map(|c| {
                (
                    c.get_name().to_string(),
                    c.get_about().map(|a| a.to_string()).unwrap_or_default(),
                )
            })
            .collect()
    }

    fn grouped() -> Vec<&'static str> {
        GROUPS
            .iter()
            .flat_map(|g| g.commands.iter().map(|e| e.name))
            .collect()
    }

    /// The point of the whole module. A command added to the CLI and not to a group is a
    /// command nobody decided the shape of — which is how the flat list reached thirty-five.
    #[test]
    fn every_subcommand_is_grouped() {
        let grouped = grouped();
        let missing: Vec<String> = compiled()
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| !grouped.contains(&name.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "not in any --help group: {missing:?} — add them to help::GROUPS \
             and decide whether each writes"
        );
    }

    /// The other direction: a group naming a command that does not exist prints nothing and
    /// looks fine, so nothing would ever catch the typo.
    #[test]
    fn every_grouped_command_exists_in_this_build() {
        let compiled: Vec<String> = compiled().into_iter().map(|(n, _)| n).collect();
        let phantom: Vec<&str> = grouped()
            .into_iter()
            .filter(|name| !compiled.contains(&name.to_string()))
            .filter(|name| !FEATURE_GATED.contains(name))
            .collect();
        assert!(
            phantom.is_empty(),
            "grouped but not a subcommand of this build: {phantom:?}\n\
             If the build is `--no-default-features`, the build is what is wrong and not the \
             grouping: the light build is the `default` set, and the note on the `reports` \
             feature in Cargo.toml says why naming that feature alone does not produce one."
        );
    }

    #[test]
    fn no_command_is_in_two_groups() {
        let names = grouped();
        let mut seen = names.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), names.len(), "a command is grouped twice");
    }

    /// The REGEN generators are the reason the write marker exists: they are the commands
    /// that look like reads. `regen` runs all of them, so it writes too.
    ///
    /// **This group is not the set of generators**, and reading it as one is how two
    /// generators came to be marked read-only (#831). `kuten` and `decisions-log` are filed
    /// by what they report, for the reasons given beside them. The set itself is guarded in
    /// [`crate::cmd`]'s `regen` module, against the crate's own `update_file_regen` call
    /// sites rather than against another list — including the part this test cannot see,
    /// which is whether a generator is in any group at all.
    #[test]
    fn every_regen_generator_is_marked_as_writing() {
        let readme = GROUPS
            .iter()
            .find(|g| g.title.starts_with("README blocks"))
            .expect("the README-block group");
        assert_eq!(readme.commands.len(), 12, "eleven generators plus `regen`");
        for entry in readme.commands {
            assert!(entry.writes, "{} must be marked as writing", entry.name);
        }
    }

    /// Every generator, wherever it is filed, carries the marker.
    ///
    /// The test above looks at a *group* and asks whether its entries are marked; this looks
    /// at the generators and asks whether they are here at all. `decisions-log` passed the
    /// first for as long as it existed — it is filed with the history commands — while being
    /// marked read-only and documenting itself as *"Read-only."* (#831).
    ///
    /// The list comes from the library, where [`yidam::regen_generator_names`] is in turn held
    /// to the crate's own `update_file_regen` call sites. Neither end of that chain can name a
    /// generator the other has not got.
    #[test]
    fn every_generator_carries_the_write_marker_wherever_it_is_filed() {
        for name in yidam::regen_generator_names() {
            let entry = GROUPS
                .iter()
                .flat_map(|g| g.commands)
                .find(|e| e.name == name)
                .unwrap_or_else(|| panic!("`{name}` writes a REGEN block and is in no group"));
            assert!(
                entry.writes,
                "`{name}` rewrites a tracked file and `--help` marks it read-only"
            );
        }
    }

    /// `doctor` exists to be safe against a repository you only mean to inspect. If it ever
    /// appears with the marker, either the marker is wrong or the command is.
    #[test]
    fn the_read_only_checks_carry_no_marker() {
        let gates = GROUPS
            .iter()
            .find(|g| g.title.starts_with("Checks and gates"))
            .expect("the gates group");
        for entry in gates.commands {
            assert!(!entry.writes, "{} is in a read-only group", entry.name);
        }
    }

    // ── the short list ───────────────────────────────────────────────────────

    fn short() -> Vec<&'static str> {
        GROUPS
            .iter()
            .flat_map(|g| g.commands)
            .filter(|e| e.short)
            .map(|e| e.name)
            .collect()
    }

    /// The point of the short listing. Nothing about printing thirteen rows resists a
    /// fourteenth, and a short list that grows is the long one arriving by instalments —
    /// which is how `--help` reached ninety lines the first time (#921).
    ///
    /// A ceiling and not an exact count, so a command can leave the list without a test
    /// edit. It is also the shape that survives a merge: two branches each adding one entry
    /// both pass alone and the merge does not, which an `assert_eq!` on a number both
    /// branches moved would not catch.
    #[test]
    fn the_short_help_stays_short() {
        let short = short();
        assert!(
            short.len() <= 14,
            "{} commands on the short `--help`: {short:?}\nIf one of them belongs there more \
             than one already on it, say which comes off.",
            short.len()
        );
    }

    /// A reader who wants a command that is not on the short list has to be told where it is.
    #[test]
    fn the_short_help_says_where_the_rest_are() {
        let available: Vec<(String, String)> = GROUPS
            .iter()
            .flat_map(|g| g.commands)
            .map(|e| (e.name.to_string(), "a description".to_string()))
            .collect();
        let text = render_short(&available);
        assert!(text.contains("--help-all"), "{text}");
        assert!(
            text.contains(&available.len().to_string()),
            "the short listing does not say how many commands it is not showing:\n{text}"
        );
    }

    /// The short list is the same in every build, or it is not a promise about the tool.
    ///
    /// A `.short()` on a feature-gated command would render on a `--features full` build and
    /// vanish from the light one, and the vanishing is silent: the listing just has twelve
    /// rows. `--help-all` may legitimately differ between builds; the first screen may not.
    #[test]
    fn no_short_command_is_feature_gated() {
        let gated: Vec<&str> = short()
            .into_iter()
            .filter(|n| FEATURE_GATED.contains(n))
            .collect();
        assert!(
            gated.is_empty(),
            "on the short `--help` and absent from some builds: {gated:?}"
        );
    }

    /// #921's measurement, as a gate.
    ///
    /// Eleven `<!-- REGEN -->` generators printed above `graph`, `retrieve` and `query` —
    /// eleven rows of maintenance surface ahead of the commands a reader opened `--help` to
    /// find. What a first reader needs to know about that family is that it can be
    /// refreshed, which is one row: `regen`, which is not itself a generator and runs all
    /// eleven.
    ///
    /// `status` is the one generator on the list, and is there for what it reports rather
    /// than for what it writes — it is how a person sees the repository at all. It is a
    /// generator incidentally, which is why the ceiling is one and not zero.
    #[test]
    fn the_generators_do_not_lead_the_short_help() {
        let short = short();
        assert!(
            short.contains(&"regen"),
            "`regen` is the one row that stands for the eleven generators, and it is not on \
             the short `--help`: {short:?}"
        );
        let generators: Vec<&str> = yidam::regen_generator_names()
            .into_iter()
            .filter(|n| short.contains(n))
            .collect();
        assert!(
            generators.len() <= 1,
            "the REGEN generators on the short `--help` are {generators:?}\n`regen` already \
             stands for the family; the rest belong in `--help-all`."
        );
    }

    // ── rendering ────────────────────────────────────────────────────────────

    #[test]
    fn the_rendering_carries_every_compiled_command_under_a_heading() {
        let available = compiled();
        let text = render(&available);
        assert!(!text.contains("Ungrouped:"), "{text}");
        for (name, _) in &available {
            assert!(
                text.contains(name.as_str()),
                "{name} is missing from:\n{text}"
            );
        }
        for group in GROUPS {
            // Groups whose every command is feature-gated out are skipped, not printed empty.
            let any = group
                .commands
                .iter()
                .any(|e| available.iter().any(|(n, _)| n == e.name));
            assert_eq!(
                text.contains(group.title),
                any,
                "group heading {:?} printed without commands, or omitted with them",
                group.title
            );
        }
    }

    /// Descriptions come from clap, never from this module, so they cannot drift from what
    /// `yidam <command> --help` prints.
    #[test]
    fn descriptions_are_claps_own() {
        let available = vec![("doctor".to_string(), "a distinctive blurb".to_string())];
        assert!(render(&available).contains("a distinctive blurb"));
    }

    #[test]
    fn the_write_marker_is_on_the_writers_and_only_them() {
        let available = vec![
            ("status".to_string(), "writes a block".to_string()),
            ("doctor".to_string(), "reads only".to_string()),
        ];
        let text = render(&available);
        let line = |needle: &str| {
            text.lines()
                .find(|l| l.contains(needle))
                .unwrap_or_default()
                .to_string()
        };
        assert!(line("status").contains(WRITES), "{text}");
        assert!(!line("doctor").contains(WRITES), "{text}");
        assert!(text.contains("rewrites files in the repository"), "{text}");
    }

    /// The short rendering carries exactly the marked commands, and nothing else.
    ///
    /// Both directions, because each fails invisibly on its own: a marked command the
    /// renderer drops leaves a reader without it, and an unmarked one it keeps is how a
    /// thirteen-row listing becomes a fifty-eight-row one nobody decided on.
    #[test]
    fn the_short_rendering_carries_the_marked_commands_and_only_them() {
        let available = compiled();
        let text = render_short(&available);
        let rows: Vec<&str> = text
            .lines()
            .skip(1)
            .filter_map(|l| l.strip_prefix("  "))
            .filter(|l| !l.starts_with(' ') && !l.starts_with(WRITES))
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        assert_eq!(rows, short(), "rendered short listing:\n{text}");
    }

    /// The short listing is a view of the long one, not a second listing beside it. A
    /// description reaching a reader through `--help` and a different one through
    /// `--help-all` would be the drift this module exists to prevent, one level up.
    #[test]
    fn the_short_rendering_is_a_subset_of_the_grouped_one() {
        let available = compiled();
        let all = render(&available);
        for line in render_short(&available)
            .lines()
            .filter_map(|l| l.strip_prefix("  "))
            .filter(|l| !l.starts_with(' ') && !l.starts_with(WRITES))
        {
            let name = line.split_whitespace().next().unwrap_or_default();
            let about = line.split_once(name).map(|(_, r)| r.trim()).unwrap_or("");
            let about = about.strip_prefix(WRITES).unwrap_or(about).trim();
            assert!(
                all.lines().any(|l| l.contains(name) && l.contains(about)),
                "`{name}` reads differently on `--help` than on `--help-all`"
            );
        }
    }

    /// The fallback exists so a missed command is ugly rather than invisible. The coverage
    /// test above keeps it unreachable in practice; this one keeps it working.
    #[test]
    fn an_unknown_command_is_listed_rather_than_dropped() {
        let available = vec![("brand-new".to_string(), "not in any group".to_string())];
        let text = render(&available);
        assert!(text.contains("Ungrouped:"), "{text}");
        assert!(text.contains("brand-new"), "{text}");
    }
}
