use super::*;

/// One instance's place in time under its class's [`crate::corpus::Interval`]: the start it wrote, and the
/// end, or `None` for an interval still open.
struct Placed {
    node: usize,
    start: String,
    end: Option<String>,
}

/// A date property's value as text, or `None` when it is absent or not a scalar.
///
/// A number is text here because YAML reads an unquoted `1893` as one, and `property-type`
/// reports that as a malformed date without gating — so the interval should still be read.
fn date_scalar(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.trim().to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Whether `a` begins strictly before `b` ends. An open end is after every start.
fn starts_before_end(a: &Placed, b: &Placed) -> bool {
    match &b.end {
        None => true,
        Some(end) => compare_dates(&a.start, end) == Some(std::cmp::Ordering::Less),
    }
}

/// The later of two dates by [`compare_dates`], and at a tie the more precise one — the tie
/// is two readings of one moment, and the finer one says more about where the span begins.
fn later<'a>(a: &'a str, b: &'a str) -> &'a str {
    match compare_dates(a, b) {
        Some(std::cmp::Ordering::Less) => b,
        Some(std::cmp::Ordering::Equal) if b.len() > a.len() => b,
        _ => a,
    }
}

/// The earlier of two ends, where `None` is open and so later than any date.
fn earlier_end<'a>(a: Option<&'a str>, b: Option<&'a str>) -> Option<&'a str> {
    match (a, b) {
        (None, e) | (e, None) => e,
        (Some(a), Some(b)) => match compare_dates(a, b) {
            Some(std::cmp::Ordering::Greater) => Some(b),
            Some(std::cmp::Ordering::Equal) if b.len() > a.len() => Some(b),
            _ => Some(a),
        },
    }
}

/// Whether `a` began before `b`: by [`compare_dates`], and where that cannot tell them apart,
/// by path — so of two holders, exactly one began first.
fn began_first(nodes: &[Node], a: &Placed, b: &Placed) -> bool {
    match compare_dates(&a.start, &b.start) {
        Some(std::cmp::Ordering::Less) => true,
        Some(std::cmp::Ordering::Greater) => false,
        _ => nodes[a.node].rel < nodes[b.node].rel,
    }
}

/// How many instances may hold `target` at once: its `capacity` property, or one where the
/// class names none or the target omits it. `None` when the value is not a whole number of at
/// least one. Digits in quotes count, because a corpus that declared the property `string`
/// before this check existed writes `seats: "3"`, and it is still a count.
fn capacity_of(target: &Node, capacity: Option<&str>) -> Option<usize> {
    let Some(cap) = capacity else {
        return Some(1);
    };
    let value = target.inst.properties.as_ref().and_then(|m| m.get(cap));
    let n = match value {
        None => return Some(1),
        Some(serde_yaml::Value::Number(n)) => n.as_u64(),
        Some(serde_yaml::Value::String(s)) => s.trim().parse::<u64>().ok(),
        Some(_) => None,
    };
    n.filter(|&n| n >= 1).map(|n| n as usize)
}

pub const INTERVAL_OVERLAP: CheckSpec = CheckSpec {
    id: "interval-overlap",
    title:
        "More holders of one target at once than it seats, or an interval ending before it starts",
    severity: Severity::Error,
    rationale:
        "A class that declares `interval:` has said which of its properties are an instance's \
         start and end, and one that also declares `exclusive_over:` has said no two instances \
         may hold the same target by that relationship at once — one sheriff at a time. Two \
         that overlap give *who held it then* two answers. A `capacity:` names the target's \
         own count, so three commissioners at once is not a finding. Intervals are half-open and \
         compared as `query` orders dates: at the precision both sides share, so `1893` and \
         `1893-06-01` are adjacent rather than overlapping, and an absent end is still open. \
         A class that declares no `interval:` is not checked.",
};

/// More instances of an interval class holding one target at once than it holds, and
/// intervals that end before they start (#1201).
///
/// **The question a `tenure` class exists to answer is the one this protects.** A corpus
/// models tenures so it can say who was sheriff in 1893, and two tenures of one office that
/// overlap give that question two answers while `lint`, `graph-check` and `query` all report
/// success. The constraint is what `interval.exclusive_over` declares: no two instances linking
/// the same target by that relationship may overlap.
///
/// **Overlap is decided by [`compare_dates`], the rule `query`'s orderings use.** An interval
/// is half-open — its start inclusive, its end exclusive, as `tenure`'s own actions define
/// *open at a date* — so two intervals overlap when each starts strictly before the other
/// ends. Where the two sides disagree about precision both drop to the coarser, and equality
/// there is not overlap: a term ending `1893` and one beginning `1893-06-01` may have met in
/// June, and reporting it would be the check inventing a day the corpus does not know. What
/// is certain at the shared precision is reported; nothing else is.
///
/// **An absent end is open, and two open intervals over one target always overlap.** That is
/// the reading `tenure` writes — *absent means still serving* — and it is the case this check
/// most needs to see: two holders both still serving is the corpus contradicting itself today.
/// An instance with no start, or a start or end that is not a date, is not placed at all:
/// `missing-property` and `property-type` report those, and guessing a date to compare would
/// be a finding the corpus did not make.
///
/// **A target may hold more than one, and says how many itself (#1205).** A board of three
/// commissioners with staggered terms is three tenures of one office open at once, and correct.
/// `interval.capacity` names the target's property that counts its seats; a holder is reported
/// only when that many others already held the target at its start. The count is the target's,
/// not the class's, because one `office` class holds both the sheriff and the board. A target
/// that omits it holds one — the calculator the corpus already had read it the same way — and
/// one whose value is not a whole number is reported on the target and left unchecked.
///
/// **A declaration naming what the class does not declare is reported on the class.** A
/// `start` that is not a `date` property of the class, or an `exclusive_over` naming no
/// relationship in its `edges:`, would otherwise read every instance as unplaceable and report
/// nothing — the check switched off by the typo it most needs to catch. So would a `capacity`
/// no target class declares, or one with no `exclusive_over` to bound.
///
/// Error, because the population is empty by construction: nothing is checked until a class
/// declares `interval:`, and no class could before this check existed. Gaps in coverage — a
/// span with no holder — are [`interval_gap`]'s, and only where a target asks for them.
pub fn interval_overlap(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    let mut violations = Vec::new();
    // A target two classes both bound by capacity is reported unreadable once.
    let mut unreadable: HashSet<usize> = HashSet::new();
    let by_class = members_by_class(nodes);
    for class in classes {
        let Some(interval) = &class.interval else {
            continue;
        };
        let faults = interval_faults(class, interval, classes);
        if !faults.is_empty() {
            violations.extend(faults.into_iter().map(|f| Violation::new(&class.rel, f)));
            continue;
        }
        let (placed, reversed) = place(nodes, &by_class, class, interval);
        violations.extend(reversed);
        let Some(rel) = interval.exclusive_over.as_deref() else {
            continue;
        };
        for (target, held) in holders_by_target(nodes, edges, &placed, rel) {
            let Some(seats) = capacity_of(&nodes[target], interval.capacity.as_deref()) else {
                if unreadable.insert(target) {
                    let cap = interval.capacity.as_deref().unwrap_or_default();
                    violations.push(Violation::new(
                        &nodes[target].rel,
                        format!(
                            "`{cap}` is not a whole number of at least one, so \
                             `{}` cannot say how many of its holders overlap",
                            class.name
                        ),
                    ));
                }
                continue;
            };
            // Reported on the holder that took the target while `seats` others already had
            // it. Every holder that began first and overlaps it is open at its start, so
            // those are the ones holding at once — exact for intervals on one line, and no
            // sort, because `compare_dates` is not a total order across precisions.
            for b in &held {
                let before: Vec<&&Placed> = held
                    .iter()
                    .filter(|a| a.node != b.node)
                    .filter(|a| began_first(nodes, a, b))
                    .filter(|a| starts_before_end(a, b) && starts_before_end(b, a))
                    .collect();
                if before.len() < seats {
                    continue;
                }
                let from = before
                    .iter()
                    .fold(b.start.as_str(), |f, a| later(f, &a.start));
                let to = before
                    .iter()
                    .fold(b.end.as_deref(), |t, a| earlier_end(t, a.end.as_deref()))
                    .unwrap_or("open");
                let others = before
                    .iter()
                    .map(|a| format!("`{}`", nodes[a.node].rel))
                    .collect::<Vec<_>>()
                    .join(", ");
                let beyond = match interval.capacity.as_deref() {
                    Some(cap) => format!(", beyond its `{cap}: {seats}`"),
                    None => String::new(),
                };
                violations.push(Violation::new(
                    &nodes[b.node].rel,
                    format!(
                        "overlaps {others} over `{rel}` `{}`{beyond}, from {from} to {to}",
                        nodes[target].rel,
                    ),
                ));
            }
        }
    }
    Check::new(&INTERVAL_OVERLAP, violations)
}

/// Each class's instances, by index into `nodes`.
fn members_by_class(nodes: &[Node]) -> HashMap<String, Vec<usize>> {
    let mut by_class: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        by_class.entry(class_of(n)).or_default().push(i);
    }
    by_class
}

/// What `class`'s `interval:` names that the class, or its `exclusive_over` targets, do not
/// declare. Empty when every instance can be read by it.
fn interval_faults(
    class: &Class,
    interval: &crate::corpus::Interval,
    classes: &[Class],
) -> Vec<String> {
    let mut faults = Vec::new();
    let is_date = |name: &str| {
        class
            .properties
            .iter()
            .any(|p| p.name == name && p.r#type == "date")
    };
    for (key, name) in [("start", &interval.start), ("end", &interval.end)] {
        if !is_date(name) {
            faults.push(format!(
                "`interval.{key}` names `{name}`, which `{}` does not declare as a `date` \
                 property",
                class.name
            ));
        }
    }
    let exclusive = interval.exclusive_over.as_deref();
    if let Some(rel) = exclusive {
        if !class.edges.iter().any(|e| e.relationship == rel) {
            faults.push(format!(
                "`interval.exclusive_over` names `{rel}`, which `{}` does not declare in \
                 `edges:`",
                class.name
            ));
        }
    }
    if let Some(cap) = interval.capacity.as_deref() {
        faults.extend(target_property_faults(
            class, classes, exclusive, "capacity", cap,
        ));
    }
    faults
}

/// Why `interval.{key}: {prop}` cannot be read: no `exclusive_over` to name a target, or a
/// target class that does not declare `prop`.
fn target_property_faults(
    class: &Class,
    classes: &[Class],
    exclusive: Option<&str>,
    key: &str,
    prop: &str,
) -> Vec<String> {
    let Some(rel) = exclusive else {
        return vec![format!(
            "`interval.{key}` names `{prop}`, but `{}` declares no `exclusive_over` for it to \
             bound",
            class.name
        )];
    };
    let mut targets: BTreeSet<&str> = class
        .edges
        .iter()
        .filter(|e| e.relationship == rel)
        .map(|e| e.target.as_str())
        .collect();
    // A target class that does not exist is `edge-target-class`'s finding.
    targets.retain(|t| classes.iter().any(|c| c.name == *t));
    targets
        .into_iter()
        .filter(|t| {
            !classes
                .iter()
                .filter(|c| c.name == *t)
                .any(|c| c.properties.iter().any(|p| p.name == prop))
        })
        .map(|t| {
            format!(
                "`interval.{key}` names `{prop}`, which `{t}` — a `{rel}` target — does not \
                 declare"
            )
        })
        .collect()
}

/// `class`'s instances placed in time by `interval`, and a violation for each whose end
/// precedes its start. An instance with no start, or a start or end that is not a date, is
/// not placed: `missing-property` and `property-type` report those.
fn place(
    nodes: &[Node],
    by_class: &HashMap<String, Vec<usize>>,
    class: &Class,
    interval: &crate::corpus::Interval,
) -> (Vec<Placed>, Vec<Violation>) {
    let mut placed = Vec::new();
    let mut reversed = Vec::new();
    let members = by_class.get(&class.name).map(Vec::as_slice);
    for &i in members.unwrap_or_default() {
        let props = &nodes[i].inst.properties;
        let get = |name: &str| {
            props
                .as_ref()
                .and_then(|m| m.get(name))
                .and_then(date_scalar)
        };
        let Some(start) = get(&interval.start).filter(|s| iso_date_parts(s).is_some()) else {
            continue;
        };
        let end = match get(&interval.end) {
            None => None,
            Some(e) if iso_date_parts(&e).is_some() => Some(e),
            Some(_) => continue,
        };
        if let Some(e) = &end {
            if compare_dates(e, &start) == Some(std::cmp::Ordering::Less) {
                reversed.push(Violation::new(
                    &nodes[i].rel,
                    format!(
                        "`{}: {e}` precedes `{}: {start}`",
                        interval.end, interval.start
                    ),
                ));
                continue;
            }
        }
        placed.push(Placed {
            node: i,
            start,
            end,
        });
    }
    (placed, reversed)
}

/// Each target the `placed` instances link by `rel`, with the instances holding it.
fn holders_by_target<'a>(
    nodes: &[Node],
    edges: &Edges,
    placed: &'a [Placed],
    rel: &str,
) -> BTreeMap<usize, Vec<&'a Placed>> {
    let mut holders: BTreeMap<usize, Vec<&Placed>> = BTreeMap::new();
    for p in placed {
        let n = &nodes[p.node];
        let targets: BTreeSet<usize> = edges
            .instance_links(p.node)
            .filter(|(e, _)| e.written_as(n).relationship.as_deref() == Some(rel))
            .map(|(_, t)| t)
            .collect();
        for t in targets {
            holders.entry(t).or_default().push(p);
        }
    }
    holders
}

/// Whether `target`'s line of holders is marked complete by its `complete` property: absent
/// is not. `None` when the value is neither `true` nor `false`; quoted, it still counts.
fn line_complete(target: &Node, complete: &str) -> Option<bool> {
    let value = target
        .inst
        .properties
        .as_ref()
        .and_then(|m| m.get(complete));
    match value {
        None => Some(false),
        Some(serde_yaml::Value::Bool(b)) => Some(*b),
        Some(serde_yaml::Value::String(s)) => s.trim().parse::<bool>().ok(),
        Some(_) => None,
    }
}

/// Whether `h` has certainly ended by `at`: its end is earlier, or the same date written to
/// the same precision. An end equal to `at` only at a coarser precision may be after it.
fn ended_by(h: &Placed, at: &str) -> bool {
    let Some(end) = &h.end else {
        return false;
    };
    match compare_dates(end, at) {
        Some(std::cmp::Ordering::Less) => true,
        Some(std::cmp::Ordering::Equal) => {
            iso_date_parts(end).map(|p| p.len()) == iso_date_parts(at).map(|p| p.len())
        }
        _ => false,
    }
}

pub const INTERVAL_GAP: CheckSpec = CheckSpec {
    id: "interval-gap",
    title: "A span inside a finished line of holders that fewer hold than the target seats",
    severity: Severity::Warn,
    rationale:
        "A target whose `interval.complete:` property is `true` has said its line of holders is \
         finished, so a span inside it that fewer instances hold than it seats is a missing \
         tenure or a wrong date. Only interior spans count: before the first holder the line \
         had not started, and after the last it may have ended. A target that declares nothing \
         is not checked, because most corpora are incomplete by design. Intervals are half-open \
         and compared at the precision both sides share, so a handover on one date is not a \
         gap, and neither is `1893` followed by `1893-06-01`. Warn, because a line marked \
         complete may already hold real gaps.",
};

/// A span inside a complete line of holders during which fewer instances hold the target than
/// it seats (#1213).
///
/// **The opt-in is the target's, as the count is.** Most corpora are incomplete by design, and
/// reporting every gap would put a permanent finding on every office whose early history nobody
/// has transcribed. A corpus that has finished a line is in a different position: there a gap
/// is a missing tenure or a wrong date, and nothing else reports either. `interval.complete`
/// names the target's property that says so — `line_complete: true` on the offices someone has
/// actually finished — for the same reason `capacity` is the target's: one `office` class holds
/// both a transcribed sheriff and a board whose 1850s nobody has read.
///
/// **Only interior holes count, and only certain ones.** A gap opens where a holder ends and
/// fewer than the target's seats remain open, and it is reported only if some holder certainly
/// begins after it: the span before the first holder is the line not having started, and the
/// span after the last is a line that ended or a holder not yet recorded. A target with no
/// holders reports nothing. The same half-open, shared-precision reading as
/// [`interval_overlap`] decides it, so a handover on the same date is not a gap, and neither is
/// a term ending `1893` followed by one beginning `1893-06-01` — the gap cannot be shown at the
/// precision they share, and a holder the dates cannot place is counted as still holding.
///
/// **Fewer than capacity, not none at all**, because a missing commissioner on a board of three
/// is exactly the tenure a finished line should not lack. A vacancy the record really shows is
/// the corpus's to note; the finding is the question.
///
/// Warn, because an opt-in does not keep the population empty: a corpus marking a line complete
/// may already hold real gaps, and the check exists to find them rather than to fail the gate
/// on the day it is asked.
pub fn interval_gap(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    let mut violations = Vec::new();
    let mut unreadable: HashSet<usize> = HashSet::new();
    let by_class = members_by_class(nodes);
    for class in classes {
        let Some(interval) = &class.interval else {
            continue;
        };
        let Some(complete) = interval.complete.as_deref() else {
            continue;
        };
        let exclusive = interval.exclusive_over.as_deref();
        let faults = target_property_faults(class, classes, exclusive, "complete", complete);
        if !faults.is_empty() {
            violations.extend(faults.into_iter().map(|f| Violation::new(&class.rel, f)));
            continue;
        }
        // The rest of the declaration is `interval-overlap`'s to report.
        let Some(rel) = exclusive.filter(|_| interval_faults(class, interval, classes).is_empty())
        else {
            continue;
        };
        let (placed, _) = place(nodes, &by_class, class, interval);
        for (target, held) in holders_by_target(nodes, edges, &placed, rel) {
            match line_complete(&nodes[target], complete) {
                Some(true) => {}
                Some(false) => continue,
                None => {
                    if unreadable.insert(target) {
                        violations.push(Violation::new(
                            &nodes[target].rel,
                            format!(
                                "`{complete}` is neither `true` nor `false`, so `{}` cannot \
                                 say whether its line of holders is complete",
                                class.name
                            ),
                        ));
                    }
                    continue;
                }
            }
            // An unreadable count is `interval-overlap`'s finding.
            let Some(seats) = capacity_of(&nodes[target], interval.capacity.as_deref()) else {
                continue;
            };
            let mut opened: BTreeSet<&str> = BTreeSet::new();
            for a in &held {
                let Some(at) = a.end.as_deref() else {
                    continue;
                };
                if !opened.insert(at) {
                    continue;
                }
                let after =
                    |h: &&&Placed| compare_dates(&h.start, at) == Some(std::cmp::Ordering::Greater);
                let Some(next) =
                    held.iter()
                        .filter(after)
                        .reduce(|x, y| if began_first(nodes, y, x) { y } else { x })
                else {
                    continue;
                };
                let open = held
                    .iter()
                    .filter(|h| !after(h) && !ended_by(h, at))
                    .count();
                if open >= seats {
                    continue;
                }
                let holding = match (open, interval.capacity.as_deref()) {
                    (0, _) => format!("no `{rel}` holder"),
                    (n, Some(cap)) => format!("at most {n} of its `{cap}: {seats}` held"),
                    (n, None) => format!("at most {n} of {seats} held"),
                };
                violations.push(Violation::new(
                    &nodes[target].rel,
                    format!(
                        "{holding} from {at} to {}, between `{}` and `{}`",
                        next.start, nodes[a.node].rel, nodes[next.node].rel,
                    ),
                ));
            }
        }
    }
    Check::new(&INTERVAL_GAP, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    const EXCLUSIVE: &str =
        "interval:\n  start: began\n  end: ended\n  exclusive_over: of-office\n";

    /// `interval-overlap` over the given tenures, with a `sheriff` and an `auditor` office.
    fn overlaps(class: Class, terms: Vec<Node>) -> Check {
        let mut nodes = vec![office("sheriff"), office("auditor")];
        nodes.extend(terms);
        let edges = Edges::build(&nodes);
        interval_overlap(&nodes, &edges, &[class])
    }

    /// The reported case: two sheriffs for the same years. Reported once, on the holder who
    /// began later, naming the other and the span they share.
    #[test]
    fn two_holders_of_one_office_over_the_same_years_overlap() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1891-06-01", Some("1895")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let v = &c.violations[0];
        assert_eq!(v.node, ".yidam/corpus/tenure/b.yml");
        assert!(v.detail.contains("tenure/a.yml"), "{}", v.detail);
        assert!(v.detail.contains("office/sheriff.yml"), "{}", v.detail);
        assert!(v.detail.contains("from 1891-06-01 to 1893"), "{}", v.detail);
        assert_eq!(c.severity, Severity::Error);
    }

    /// Exactly the same interval twice is the plainest overlap there is.
    #[test]
    fn an_exact_overlap_is_reported() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889-01-01", Some("1893-01-01")),
                term("b", "sheriff", "1889-01-01", Some("1893-01-01")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
    }

    /// Mixed precision decided at the precision both sides share: `1893-06-01` begins before
    /// a term ending `1894` ends, whatever day in 1894 that was.
    #[test]
    fn an_overlap_under_mixed_precision_is_reported() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889", Some("1894")),
                term("b", "sheriff", "1893-06-01", Some("1897")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
    }

    /// And equality at that precision is not overlap. A term ending `1893` and one beginning
    /// `1893-06-01` may have met in June; reporting them would invent a day nobody recorded.
    #[test]
    fn a_boundary_equal_only_at_the_coarser_precision_is_adjacent() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1893-06-01", Some("1897")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// A handover: one ends the day the next begins. Half-open, so they do not overlap.
    #[test]
    fn adjacent_intervals_do_not_overlap() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889-01-07", Some("1893-01-02")),
                term("b", "sheriff", "1893-01-02", Some("1897-01-04")),
                term("c", "sheriff", "1897-01-04", None),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// Two holders both still serving: open intervals over one target always overlap.
    #[test]
    fn two_open_intervals_over_one_target_overlap() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "2017", None),
                term("b", "sheriff", "2021-01-04", None),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(
            c.violations[0].detail.ends_with("to open"),
            "{:?}",
            c.violations
        );
    }

    /// Exclusivity is per target: a sheriff and an auditor over the same years is two offices.
    #[test]
    fn holders_of_different_targets_do_not_overlap() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "auditor", "1889", Some("1893")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// An interval with no `exclusive_over:` checks order and nothing else.
    #[test]
    fn an_end_before_its_start_is_reported_without_exclusivity() {
        let c = overlaps(
            tenure("interval:\n  start: began\n  end: ended\n"),
            vec![
                term("a", "sheriff", "1893", Some("1889")),
                term("b", "sheriff", "1889", Some("1893")),
                term("c", "sheriff", "1889", Some("1893")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert_eq!(c.violations[0].node, ".yidam/corpus/tenure/a.yml");
        assert!(c.violations[0]
            .detail
            .contains("`ended: 1889` precedes `began: 1893`"));
    }

    /// Silent for a class that declares nothing, which is every class that predates the field.
    #[test]
    fn a_class_declaring_no_interval_is_not_checked() {
        let c = overlaps(
            tenure(""),
            vec![
                term("a", "sheriff", "1893", Some("1889")),
                term("b", "sheriff", "1880", Some("1899")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// A declaration naming what the class does not declare is reported on the class, rather
    /// than reading every instance as unplaceable and reporting nothing.
    #[test]
    fn a_declaration_naming_undeclared_fields_is_reported_on_the_class() {
        let c = overlaps(
            tenure("interval:\n  start: begun\n  end: ended\n  exclusive_over: holds\n"),
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1889", Some("1893")),
            ],
        );
        let details: Vec<&str> = c.violations.iter().map(|v| v.detail.as_str()).collect();
        assert_eq!(details.len(), 2, "{details:?}");
        assert!(c
            .violations
            .iter()
            .all(|v| v.node == ".yidam/corpus/tenure.ont.yml"));
        assert!(
            details[0].contains("`interval.start` names `begun`"),
            "{details:?}"
        );
        assert!(
            details[1].contains("`interval.exclusive_over` names `holds`"),
            "{details:?}"
        );
    }

    /// A half-written declaration does not parse, so `malformed-yaml` says so rather than the
    /// class quietly declaring no interval — and a misspelled key is the same.
    #[test]
    fn a_declaration_missing_its_end_or_misspelling_a_key_is_malformed() {
        for bad in [
            "interval:\n  start: began\n",
            "interval:\n  start: began\n  end: ended\n  exclusive: of-office\n",
        ] {
            let c = tenure(bad);
            assert!(c.malformed.is_some(), "{bad}");
        }
        assert!(tenure(EXCLUSIVE).malformed.is_none());
    }

    /// The case #1205 measured: a board of three with staggered terms is three at once, and
    /// correct. A holder who has left does not count against the one who arrives after.
    #[test]
    fn three_holders_of_three_seats_are_not_reported() {
        let c = seated(
            SEATED,
            vec![seated_office("board", Some("\"3\""))],
            vec![
                term("a", "board", "1880", Some("1890")),
                term("b", "board", "1882", Some("1892")),
                term("c", "board", "1884", Some("1894")),
                term("d", "board", "1890", Some("1900")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// A fourth on a board of three is reported on the one who arrived fourth, naming the
    /// three already there and the count it exceeds.
    #[test]
    fn a_fourth_holder_of_three_seats_is_reported() {
        let c = seated(
            SEATED,
            vec![seated_office("board", Some("\"3\""))],
            vec![
                term("a", "board", "1880", Some("1890")),
                term("b", "board", "1882", Some("1892")),
                term("c", "board", "1884", Some("1894")),
                term("d", "board", "1886", Some("1896")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let v = &c.violations[0];
        assert_eq!(v.node, ".yidam/corpus/tenure/d.yml");
        for other in ["tenure/a.yml", "tenure/b.yml", "tenure/c.yml"] {
            assert!(v.detail.contains(other), "{}", v.detail);
        }
        assert!(v.detail.contains("beyond its `seats: 3`"), "{}", v.detail);
        assert!(v.detail.ends_with("from 1886 to 1890"), "{}", v.detail);
    }

    /// A target that omits its count holds one, as the corpus's own calculator read it; and
    /// an unquoted count is a count too.
    #[test]
    fn a_target_omitting_its_capacity_holds_one_and_a_number_counts() {
        let c = seated(
            SEATED,
            vec![
                seated_office("sheriff", None),
                seated_office("bench", Some("2")),
            ],
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1891", Some("1895")),
                term("c", "bench", "1889", Some("1893")),
                term("d", "bench", "1891", Some("1895")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert_eq!(c.violations[0].node, ".yidam/corpus/tenure/b.yml");
        assert!(
            c.violations[0].detail.contains("beyond its `seats: 1`"),
            "{}",
            c.violations[0].detail
        );
    }

    /// A count that is not a whole number of at least one is reported on the target, once,
    /// and its holders are left unchecked rather than checked against a guess.
    #[test]
    fn an_unreadable_capacity_is_reported_on_the_target() {
        for bad in ["three", "\"0\"", "1.5"] {
            let c = seated(
                SEATED,
                vec![seated_office("board", Some(bad))],
                vec![
                    term("a", "board", "1880", Some("1890")),
                    term("b", "board", "1882", Some("1892")),
                ],
            );
            assert_eq!(c.violations.len(), 1, "{bad}: {:?}", c.violations);
            assert_eq!(c.violations[0].node, ".yidam/corpus/office/board.yml");
            assert!(
                c.violations[0]
                    .detail
                    .contains("`seats` is not a whole number"),
                "{}",
                c.violations[0].detail
            );
        }
    }

    /// With one seat, a holder overlapping two earlier ones is one finding naming both.
    #[test]
    fn a_holder_overlapping_two_is_one_finding_naming_both() {
        let c = overlaps(
            tenure(EXCLUSIVE),
            vec![
                term("a", "sheriff", "1880", Some("1895")),
                term("b", "sheriff", "1885", Some("1895")),
                term("c", "sheriff", "1890", Some("1895")),
            ],
        );
        let nodes: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            nodes,
            [".yidam/corpus/tenure/b.yml", ".yidam/corpus/tenure/c.yml"],
            "{:?}",
            c.violations
        );
        let c_detail = &c.violations[1].detail;
        assert!(c_detail.contains("`.yidam/corpus/tenure/a.yml`, `.yidam/corpus/tenure/b.yml`"));
        assert!(c_detail.ends_with("from 1890 to 1895"), "{c_detail}");
    }

    /// The reported case: a year nobody held a complete line's one seat. Reported on the
    /// target, naming the span and the holders on either side.
    #[test]
    fn a_hole_in_a_complete_line_is_reported_on_the_target() {
        let c = gaps(
            COMPLETE,
            vec![office_with("sheriff", &["line_complete: true"])],
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1894", Some("1897")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let v = &c.violations[0];
        assert_eq!(v.node, ".yidam/corpus/office/sheriff.yml");
        assert_eq!(
            v.detail,
            "no `of-office` holder from 1893 to 1894, between \
             `.yidam/corpus/tenure/a.yml` and `.yidam/corpus/tenure/b.yml`"
        );
        assert_eq!(c.severity, Severity::Warn);
    }

    /// The same hole in a line nobody marked complete is the line being incomplete, which is
    /// what most corpora are — and `false` says so as plainly as saying nothing.
    #[test]
    fn a_line_not_marked_complete_is_not_checked() {
        for props in [&[][..], &["line_complete: false"][..]] {
            let c = gaps(
                COMPLETE,
                vec![office_with("sheriff", props)],
                vec![
                    term("a", "sheriff", "1889", Some("1893")),
                    term("b", "sheriff", "1894", Some("1897")),
                ],
            );
            assert!(c.violations.is_empty(), "{props:?}: {:?}", c.violations);
        }
    }

    /// Before the first holder the line had not started, after the last it may have ended, a
    /// handover on one date is no gap, and a target with no holders reports nothing.
    #[test]
    fn only_interior_holes_are_gaps() {
        let c = gaps(
            COMPLETE,
            vec![
                office_with("sheriff", &["line_complete: true"]),
                office_with("auditor", &["line_complete: \"true\""]),
            ],
            vec![
                term("a", "sheriff", "1889-01-07", Some("1893-01-02")),
                term("b", "sheriff", "1893-01-02", Some("1897-01-04")),
                term("c", "sheriff", "1897-01-04", Some("1901")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// `1893` and `1893-06-01` may have met in June, so the gap cannot be shown at the
    /// precision they share. `1893-03` and `1893-06-01` cannot have.
    #[test]
    fn a_gap_is_reported_only_where_the_shared_precision_shows_it() {
        let terms = |ended: &str| {
            vec![
                term("a", "sheriff", "1889", Some(ended)),
                term("b", "sheriff", "1893-06-01", Some("1897")),
            ]
        };
        let office = || vec![office_with("sheriff", &["line_complete: true"])];
        let coarse = gaps(COMPLETE, office(), terms("1893"));
        assert!(coarse.violations.is_empty(), "{:?}", coarse.violations);
        let fine = gaps(COMPLETE, office(), terms("1893-03"));
        assert_eq!(fine.violations.len(), 1, "{:?}", fine.violations);
        assert!(fine.violations[0]
            .detail
            .contains("from 1893-03 to 1893-06-01"));
    }

    /// A still-open holder covers everything after it, and an overlap is not a gap.
    #[test]
    fn an_open_or_overlapping_holder_covers_the_line() {
        let c = gaps(
            COMPLETE,
            vec![office_with("sheriff", &["line_complete: true"])],
            vec![
                term("a", "sheriff", "1880", None),
                term("b", "sheriff", "1885", Some("1890")),
                term("c", "sheriff", "1895", Some("1899")),
            ],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// A board of three with one seat empty for a year is fewer than it seats, and the count is
    /// named; three seats held throughout is not reported.
    #[test]
    fn a_vacant_seat_of_three_is_fewer_than_capacity() {
        let board = || vec![office_with("board", &["seats: 3", "line_complete: true"])];
        let full = gaps(
            COMPLETE,
            board(),
            vec![
                term("a", "board", "1880", Some("1890")),
                term("b", "board", "1880", Some("1892")),
                term("c", "board", "1880", Some("1894")),
                term("d", "board", "1890", Some("1900")),
                term("e", "board", "1892", Some("1902")),
                term("f", "board", "1894", Some("1904")),
            ],
        );
        assert!(full.violations.is_empty(), "{:?}", full.violations);

        let vacant = gaps(
            COMPLETE,
            board(),
            vec![
                term("a", "board", "1880", Some("1890")),
                term("b", "board", "1880", Some("1892")),
                term("c", "board", "1880", Some("1894")),
                term("d", "board", "1891", Some("1900")),
                term("e", "board", "1892", Some("1902")),
                term("f", "board", "1894", Some("1904")),
            ],
        );
        assert_eq!(vacant.violations.len(), 1, "{:?}", vacant.violations);
        assert_eq!(
            vacant.violations[0].detail,
            "at most 2 of its `seats: 3` held from 1890 to 1891, between \
             `.yidam/corpus/tenure/a.yml` and `.yidam/corpus/tenure/d.yml`"
        );
    }

    /// A mark that is neither `true` nor `false` is reported on the target once, and its line
    /// is left unchecked rather than read as either.
    #[test]
    fn an_unreadable_mark_is_reported_on_the_target() {
        let c = gaps(
            COMPLETE,
            vec![office_with("sheriff", &["line_complete: yes please"])],
            vec![
                term("a", "sheriff", "1889", Some("1893")),
                term("b", "sheriff", "1894", Some("1897")),
            ],
        );
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert_eq!(c.violations[0].node, ".yidam/corpus/office/sheriff.yml");
        assert!(c.violations[0]
            .detail
            .contains("`line_complete` is neither `true` nor `false`"));
    }
}
