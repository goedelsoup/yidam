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
}

const fn r(name: &'static str) -> Entry {
    Entry {
        name,
        writes: false,
    }
}

const fn w(name: &'static str) -> Entry {
    Entry { name, writes: true }
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
            r("doctor"),
            r("graph-check"),
            r("lint"),
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
        commands: &[r("due")],
    },
    Group {
        title: "README blocks — each rewrites its <!-- REGEN --> block where it is run",
        commands: &[
            w("regen"),
            w("status"),
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
            r("graph"),
            r("neighbors"),
            r("query"),
            r("pack"),
            r("estimate"),
            r("diff"),
            // Beside `diff` rather than with the gates: it reads a code diff the way `diff`
            // reads a corpus one, and it cannot fail. A command filed under "exit nonzero on
            // a problem" would be read as one more thing that can break the build, and every
            // finding it has is a question somebody has to answer rather than a defect.
            r("check-diff"),
            w("rename"),
            // Beside `rename` and not with the gates: both move something and rewrite every
            // reference to it. `rename` moves one node; this moves a class, a property, or
            // the target of a relationship, which is the same operation one level up.
            w("migrate"),
            // Beside the corpus commands rather than with the gates: it reads the gate's
            // findings and answers none of them. A command filed under "exit nonzero on a
            // problem" would be read as one more thing that can fail the build, and this
            // one cannot — it drafts commits and leaves.
            w("propose"),
            r("log"),
            r("phases"),
            r("replay"),
            r("decisions-log"),
            r("sangha"),
            r("vocabulary"),
        ],
    },
    Group {
        title: "Index and embeddings",
        commands: &[w("embed"), w("index-build")],
    },
    Group {
        // Its own group rather than beside `tonpa`, which is the nearest neighbour and is
        // still a different thing: `tonpa` installs a corpus somebody else published, and
        // this keeps the bytes a corpus rests on or produces. Nor with the index commands,
        // which build an artifact rather than store one.
        title: "Artifacts — bytes kept outside git, addressed by content",
        commands: &[w("vault")],
    },
    Group {
        // Its own group rather than beside the gates. Every command there answers *is this
        // corpus in the state it claims*; this one answers *what did this repository decide
        // the rule was*, which is a question about the gate rather than a use of it.
        title: "The rules this repository writes about itself",
        commands: &[w("policy")],
    },
    Group {
        title: "Export",
        commands: &[w("export"), w("bundle"), w("schema")],
    },
    Group {
        title: "Serving the domain computer",
        commands: &[r("serve")],
    },
    Group {
        // Its own group rather than beside the gates: `bench` measures and does not gate,
        // and a measurement filed under "exit nonzero on a problem" would be read as one.
        title: "Measuring the corpus",
        commands: &[r("bench")],
    },
    Group {
        title: "Deriving and maintaining a repository",
        commands: &[w("clone"), w("overlay"), w("backfill"), w("tonpa")],
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
const FEATURE_GATED: &[&str] = &["index-build"];

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
        out.push_str(&format!("{}:\n", group.title));
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

    out.push_str(&format!(
        "  {WRITES} rewrites files in the repository it is run against. Everything else\n\
         \x20   only reads — `yidam <command> --help` says exactly what.\n"
    ));
    out
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

    /// The eleven REGEN generators are the reason the write marker exists: they are the
    /// commands that look like reads. `regen` runs all eleven, so it writes too.
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
