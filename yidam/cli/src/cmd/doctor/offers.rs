use super::*;

/// A feature a release made available, which a repository may adopt or decline in writing
/// (RFC-0066 §1). Every other warning is a defect, and no decline reaches it.
///
/// Finer than the check ids, because one check can carry two offers from two releases, and a
/// decline that named the check would silence an offer added after its record was written.
#[derive(Debug)]
pub(super) struct Offer {
    /// The key `[doctor.declined]` takes. Stable: a repository writes it in a committed file.
    pub(super) id: &'static str,
    /// The [`ROSTER`] check whose warning this is.
    pub(super) check: &'static str,
    /// The release that first made it, as a `cli/v*` tag.
    pub(super) since: &'static str,
}

/// Every offer `doctor` raises. A gate holds each `check` to [`ROSTER`] and each `since` to a
/// release tag.
pub(super) const OFFERS: &[Offer] = &[
    Offer {
        id: "routes",
        check: Check::ROUTES,
        since: "cli/v0.16.0",
    },
    Offer {
        id: "scaffold-regions",
        check: Check::SCAFFOLD,
        since: "cli/v0.17.0",
    },
    Offer {
        id: "scaffold-hooks",
        check: Check::SCAFFOLD,
        since: "cli/v0.24.0",
    },
    Offer {
        id: "ci",
        check: Check::CI,
        since: "cli/v0.17.0",
    },
    Offer {
        id: "contract-required",
        check: Check::CONTRACT,
        since: "cli/v0.17.0",
    },
    Offer {
        id: "contract-edge-policy",
        check: Check::CONTRACT,
        since: "cli/v0.17.0",
    },
];

impl Offer {
    pub(super) fn named(id: &str) -> Option<&'static Self> {
        OFFERS.iter().find(|o| o.id == id)
    }
}

/// One offer as a check found it: open, with what is missing and what adopts it, or adopted.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Offered {
    pub id: &'static str,
    /// The release that first made it.
    pub since: &'static str,
    pub state: OfferState,
    /// The decision record a decline resolved to. Set only on `declined`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    #[serde(skip)]
    pub(super) open: Option<Finding>,
}

impl Offered {
    pub(super) fn adopted(id: &'static str) -> Self {
        Self::new(id, None)
    }

    pub(super) fn open(
        id: &'static str,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            Some(Finding {
                detail: detail.into(),
                remedy: remedy.into(),
            }),
        )
    }

    fn new(id: &'static str, open: Option<Finding>) -> Self {
        let offer = Offer::named(id);
        debug_assert!(offer.is_some(), "`{id}` is not in OFFERS");
        Self {
            id,
            since: offer.map_or("", |o| o.since),
            state: if open.is_some() {
                OfferState::Open
            } else {
                OfferState::Adopted
            },
            record: None,
            open,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Finding {
    pub(super) detail: String,
    pub(super) remedy: String,
}

/// Where one offer stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OfferState {
    /// Neither adopted nor declined.
    Open,
    /// Declined against a decision record this repository holds.
    Declined,
    /// Declined against a record this repository does not hold, so not honoured.
    Unhonoured,
    /// What the offer asks for is in place.
    Adopted,
}

/// What `[doctor.declined]` says, with what it needs to judge each decline.
#[derive(Debug, Default)]
pub(super) struct Declines {
    pub(super) declined: std::collections::BTreeMap<String, String>,
    pub(super) decisions: PathBuf,
    /// `.yidam.toml`'s `release`, without its `+build` suffix (RFC-0065 §1).
    pub(super) pin: Option<String>,
}

impl Declines {
    pub(super) fn read(root: &Path) -> Self {
        let declined = crate::config::load_yidam_config(root)
            .map(|c| c.doctor.declined)
            .unwrap_or_default();
        let pin = std::fs::read_to_string(root.join(crate::provenance::MANIFEST))
            .ok()
            .and_then(|t| ManifestPin::parse(&t).release)
            .map(|r| crate::provenance::base_release(&r).to_string());
        Self {
            declined,
            decisions: crate::paths::yidam_decisions_dir(root),
            pin,
        }
    }

    pub(super) fn find(&self, record: &str) -> Option<String> {
        crate::cmd::decisions::find_decision(&self.decisions, record)
    }

    /// How old an undecided offer is, read offline from the pin (RFC-0066 §3).
    pub(super) fn dated(&self, since: &str) -> String {
        match &self.pin {
            Some(pin) => format!("offered since {since}; this pin is {pin}"),
            None => format!("offered since {since}"),
        }
    }

    /// Keys naming no offer, sorted (the config is read into a sorted map): a typo, or an
    /// attempt to decline a defect.
    pub(super) fn unknown(&self) -> Vec<String> {
        self.declined
            .keys()
            .filter(|k| Offer::named(k).is_none())
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use tempfile::TempDir;

    // ── offers and declines (RFC-0066) ───────────────────────────────────────

    /// Each offer names a check doctor asks and a release that could have shipped it.
    #[test]
    fn every_offer_names_a_roster_check_and_a_release() {
        for o in OFFERS {
            assert!(
                ROSTER.iter().any(|q| q.id == o.check),
                "`{}` names `{}`, which no question asks",
                o.id,
                o.check
            );
            let version = o
                .since
                .strip_prefix("cli/v")
                .unwrap_or_else(|| panic!("`{}`'s since `{}` is not a cli/v* tag", o.id, o.since));
            let parts: Vec<&str> = version.split('.').collect();
            assert!(
                parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
                "`{}`'s since `{}` is not a release",
                o.id,
                o.since
            );
        }
        let mut ids: Vec<&str> = OFFERS.iter().map(|o| o.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), OFFERS.len(), "an offer id is listed twice");
    }

    /// Regions and hooks both missing: the regions offer and the hooks offer, in that order.
    fn scaffold_without_regions_or_hooks() -> TempDir {
        let tmp = derived_repo();
        std::fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join(".github/workflows/ci.yml"), "jobs:\n").unwrap();
        std::fs::write(tmp.path().join(".claude/CLAUDE.md"), "# Ours\n").unwrap();
        tmp
    }

    /// Every warn arm of an offer-carrying check is an open offer of that check's own.
    #[test]
    fn each_offer_check_warns_only_through_its_offers() {
        let bare = derived_repo();
        std::fs::write(bare.path().join("AGENTS.md"), "# Agents\n").unwrap();
        let scaffold = scaffold_without_regions_or_hooks();
        let ci_without_gate = derived_repo();
        let workflows = ci_without_gate.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows).unwrap();
        std::fs::write(workflows.join("build.yml"), "jobs:\n  b:\n    steps: []\n").unwrap();
        let contract = [class("metric", &format!("{}{EDGE}", property("")))];
        let cases = [
            (Check::ROUTES, check_routes(bare.path())),
            (Check::SCAFFOLD, check_scaffold(scaffold.path())),
            (Check::CI, check_ci(bare.path())),
            (Check::CI, check_ci(ci_without_gate.path())),
            (Check::CONTRACT, contract_answer(&contract)),
        ];
        for (check, a) in cases {
            assert_eq!(a.verdict, Verdict::Warn, "{check}: {}", a.detail);
            assert!(!a.offers.is_empty(), "{check} warned without an offer");
            for o in &a.offers {
                assert_eq!(o.state, OfferState::Open, "{check}: {}", o.id);
                assert_eq!(Offer::named(o.id).unwrap().check, check, "{}", o.id);
            }
        }
    }

    /// The regions declined, the hooks still missing: the check warns, and names the hooks.
    #[test]
    fn a_declined_offer_does_not_hide_another_on_its_check() {
        let tmp = scaffold_without_regions_or_hooks();
        let (_d, declines) = declining(
            &["0001-our-ci"],
            &[("scaffold-regions", "0001-our-ci")],
            None,
        );
        let a = check_scaffold(tmp.path()).settle(&declines);
        assert_eq!(a.verdict, Verdict::Warn, "{}", a.detail);
        assert!(a.detail.contains("`yidam hook`"), "{}", a.detail);
        assert!(!a.detail.contains("no YIDAM region"), "{}", a.detail);
        assert!(
            a.detail
                .contains("`scaffold-regions` declined in .yidam/decisions/0001-our-ci.yml"),
            "{}",
            a.detail
        );
        assert_eq!(a.remedy.as_deref(), Some("yidam migrate scaffold"));
        let states: Vec<_> = a.offers.iter().map(|o| (o.id, o.state)).collect();
        assert_eq!(
            states,
            [
                ("scaffold-regions", OfferState::Declined),
                ("scaffold-hooks", OfferState::Open)
            ]
        );
    }

    /// Every open offer declined against a held record: `declined`, with the record named.
    #[test]
    fn an_offer_declined_against_a_held_record_reads_declined() {
        let tmp = derived_repo();
        let (_d, declines) =
            declining(&["0002-ci-elsewhere"], &[("ci", "0002-ci-elsewhere")], None);
        let a = check_ci(tmp.path()).settle(&declines);
        assert_eq!(a.verdict, Verdict::Declined, "{}", a.detail);
        assert_eq!(a.remedy, None);
        assert_eq!(
            a.offers[0].record.as_deref(),
            Some(".yidam/decisions/0002-ci-elsewhere.yml")
        );
    }

    /// A decline against a record that is not there is not honoured, and says so.
    #[test]
    fn a_decline_without_its_record_is_not_honoured() {
        let tmp = derived_repo();
        let (_d, declines) = declining(&[], &[("ci", "0009-missing")], Some("cli/v0.30.0"));
        let a = check_ci(tmp.path()).settle(&declines);
        assert_eq!(a.verdict, Verdict::Warn, "{}", a.detail);
        assert!(
            a.detail
                .contains("declined against `0009-missing`, which this repository does not hold"),
            "{}",
            a.detail
        );
        assert!(
            a.detail
                .contains("offered since cli/v0.17.0; this pin is cli/v0.30.0"),
            "{}",
            a.detail
        );
        let remedy = a.remedy.unwrap();
        assert!(
            remedy.starts_with("record `0009-missing` in .yidam/decisions/"),
            "{remedy}"
        );
        assert_eq!(a.offers[0].state, OfferState::Unhonoured);
    }

    /// Unknown keys are listed, and fail only under `--strict`; a declined check never does.
    #[test]
    fn strict_fails_on_an_unknown_key_and_not_on_a_decline() {
        let (_d, declines) = declining(&[], &[("graph-check", "0001"), ("ci", "0002")], None);
        assert_eq!(declines.unknown(), ["graph-check"]);

        let declined = DoctorReport::judged(checks_with(&[Verdict::Declined]), true, vec![]);
        assert!(declined.passed);
        assert_eq!(declined.declined, 1);
        let text = render(&declined, Path::new("/r"));
        assert!(text.contains("1 declined"), "{text}");

        let unknown = DoctorReport::judged(
            checks_with(&[Verdict::Ok]),
            true,
            vec!["graph-check".into()],
        );
        assert!(!unknown.passed);
        let text = render(&unknown, Path::new("/r"));
        assert!(
            text.contains("names no offer called `graph-check`"),
            "{text}"
        );
        assert!(
            !text.contains("Everything checks out"),
            "a failing --strict run does not say all is well: {text}"
        );
        assert!(DoctorReport::judged(checks_with(&[Verdict::Ok]), false, vec!["x".into()]).passed);
    }

    /// `[doctor.declined]` read from the repository's config, with the pin from `.yidam.toml`.
    #[test]
    fn declines_are_read_from_config_and_the_pin() {
        let tmp = derived_repo();
        std::fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[doctor.declined]\nci = \"0002-ci-elsewhere\"\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join(crate::provenance::MANIFEST),
            "[yidam]\nrelease = \"cli/v0.31.0+abc\"\n",
        )
        .unwrap();
        let d = Declines::read(tmp.path());
        assert_eq!(
            d.declined.get("ci").map(String::as_str),
            Some("0002-ci-elsewhere")
        );
        assert_eq!(d.pin.as_deref(), Some("cli/v0.31.0"));
    }
}
