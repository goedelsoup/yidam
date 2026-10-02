//! The two age-invariant candidates RFC-0028 names for the retired `classes` slot, fitted and
//! then replayed against the histories they were fitted from (#1303).
//!
//! Revision 2 of `inquiry` retired `nodes_per_commit` because it measured how old a repository
//! is: every member passed through the band and then left it, without changing anything it
//! did. RFC-0028 named two replacements that might not:
//!
//! - **a trailing window**: nodes added per authored commit over the last *N*, not over the
//!   whole history. It is age-invariant if the rate within a window is stable at a stage.
//! - **a decay curve**: `nodes / authored = a · authored^b`, fitted across the members, with the
//!   distance from the curve reported instead of membership of a box.
//!
//! Arguing over which one is age-invariant is what produced the retired bands, so this runs
//! the test instead. Each candidate's band is fitted the way the profile fits every band, from
//! one value per member at the head of its history, by `measured.estimator`. Then every
//! member's own history is read back through that band. **A candidate that any member leaves
//! while it ages is rejected.** That is #1210's ambition test, and it is the reading
//! `allen-county-ohio` failed six hours after the `classes` fit.
//!
//! # The maturity control is the replay
//!
//! Members are not compared at their heads, where one is 73 commits old and another 1,300.
//! Each one is read at every authored index it passed through, inside the age range the fit
//! spans, so at each index the band is asked about every member that has reached it. A point
//! outside that range is not evidence either way: the band was never fitted there, and a member
//! leaving it at commit 30 says nothing about a range that begins at 73.
//!
//! Spearman's ρ between age and value is printed beside each member's replay. It is not part
//! of the verdict. It is what shows whether the exits are ageing or noise, and a reader who
//! wants to dispute a rejection should start there.
//!
//! Pure: trajectories in, verdicts out. `cohort` reads the histories.

/// One point on a member's history: the corpus as it stood after `authored` authored commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub authored: usize,
    pub nodes: usize,
}

/// One member's history, oldest first. The last point is the member's head, and the fit
/// reads that.
#[derive(Debug, Clone)]
pub struct Trajectory {
    pub letter: String,
    pub points: Vec<Point>,
}

/// The windows the trailing-window candidate is read at. RFC-0028 names no *N*, and choosing
/// one after seeing which one passes would be a number chosen to fit, so the candidate is
/// rejected only if it fails at all three. 100 is deliberately longer than the two youngest
/// members of the cluster: a window that cannot be measured in a young corpus is part of the
/// answer, not a reason to leave it out.
pub const WINDOWS: [usize; 3] = [25, 50, 100];

/// The whole reading.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Accretion {
    /// Members whose heads the bands were fitted from, by letter.
    pub fitted_from: Vec<String>,
    /// Members left out of the fit, and why. Reported, not dropped.
    pub excluded: Vec<Excluded>,
    /// `[youngest head, oldest head]` in authored commits: the range each replay is read over.
    pub ages: Option<(usize, usize)>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Excluded {
    pub letter: String,
    pub reason: String,
}

/// One candidate statistic: the band it fitted, and every member's history read back through
/// it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Candidate {
    pub id: String,
    pub reads: String,
    /// Each fitted member's value at its head. A member the candidate cannot be measured on
    /// at its head (a window longer than its history) is absent here and listed in `unfitted`.
    pub fit: Vec<Value>,
    pub unfitted: Vec<String>,
    /// The observed range of `fit`, quoted to two decimal places and rounded outward. `None`
    /// when fewer than two members could be fitted.
    pub band: Option<(f64, f64)>,
    pub replay: Vec<Replayed>,
    /// `Some(true)` when no member left the band anywhere in the replayed range. `None` when
    /// there was no band to replay against.
    pub survives: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Value {
    pub letter: String,
    pub value: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Replayed {
    pub letter: String,
    /// Points inside the fitted age range at which the candidate could be measured.
    pub read: usize,
    pub outside: usize,
    /// The first point outside the band: `(authored, value)`.
    pub first_exit: Option<(usize, f64)>,
    /// Spearman's ρ between authored index and value over the points read. `None` under
    /// three points, or when either side is constant.
    pub rho: Option<f64>,
}

/// Fit and replay every candidate over `members`.
pub fn assess(members: &[Trajectory]) -> Accretion {
    let fitted: Vec<&Trajectory> = members.iter().filter(|t| !t.points.is_empty()).collect();
    let excluded = members
        .iter()
        .filter(|t| t.points.is_empty())
        .map(|t| Excluded {
            letter: t.letter.clone(),
            reason: "no corpus history to replay".into(),
        })
        .collect();
    let heads: Vec<usize> = fitted.iter().map(|t| head(t).authored).collect();
    let ages = heads
        .iter()
        .min()
        .zip(heads.iter().max())
        .map(|(a, b)| (*a, *b));

    let mut candidates = Vec::new();
    for n in WINDOWS {
        candidates.push(candidate(
            format!("trailing-window-{n}"),
            format!(
                "nodes added per authored commit over the last {n} or more, read at each \
                 point from the nearest earlier point at least {n} commits back"
            ),
            &fitted,
            ages,
            |t, i| window(&t.points, i, n),
        ));
    }

    let curve = decay_curve(&fitted);
    candidates.push(candidate(
        "decay-curve".into(),
        match curve {
            Some((a, b)) => format!(
                "distance from nodes/authored = {:.2} · authored^{b:.3}, fitted across the \
                 members' heads, in natural-log units",
                a.exp()
            ),
            None => "distance from a nodes/authored power curve fitted across the members' \
                     heads; fewer than three heads could be fitted"
                .into(),
        },
        &fitted,
        ages,
        |t, i| curve.and_then(|c| residual(c, t.points[i])),
    ));

    Accretion {
        fitted_from: fitted.iter().map(|t| t.letter.clone()).collect(),
        excluded,
        ages,
        candidates,
    }
}

/// The member's newest point. Only fitted trajectories reach here, and an empty one is
/// excluded before fitting, so the origin is never read; it keeps this total rather than
/// spending the panic budget on an invariant `assess` already holds.
fn head(t: &Trajectory) -> Point {
    t.points.last().copied().unwrap_or(Point {
        authored: 0,
        nodes: 0,
    })
}

fn candidate(
    id: String,
    reads: String,
    members: &[&Trajectory],
    ages: Option<(usize, usize)>,
    value: impl Fn(&Trajectory, usize) -> Option<f64>,
) -> Candidate {
    let mut fit = Vec::new();
    let mut unfitted = Vec::new();
    for t in members {
        match value(t, t.points.len() - 1) {
            Some(v) => fit.push(Value {
                letter: t.letter.clone(),
                value: v,
            }),
            None => unfitted.push(t.letter.clone()),
        }
    }
    let band = (fit.len() >= 2).then(|| {
        let lo = fit.iter().map(|v| v.value).fold(f64::INFINITY, f64::min);
        let hi = fit
            .iter()
            .map(|v| v.value)
            .fold(f64::NEG_INFINITY, f64::max);
        outward(lo, hi)
    });

    let mut replay = Vec::new();
    if let (Some((lo, hi)), Some((young, old))) = (band, ages) {
        for t in members {
            let series: Vec<(usize, f64)> = (0..t.points.len())
                .filter(|&i| (young..=old).contains(&t.points[i].authored))
                .filter_map(|i| value(t, i).map(|v| (t.points[i].authored, v)))
                .collect();
            let exits: Vec<&(usize, f64)> =
                series.iter().filter(|(_, v)| *v < lo || *v > hi).collect();
            replay.push(Replayed {
                letter: t.letter.clone(),
                read: series.len(),
                outside: exits.len(),
                first_exit: exits.first().map(|p| **p),
                rho: spearman(&series),
            });
        }
    }
    let survives = band.map(|_| replay.iter().all(|r| r.outside == 0));

    Candidate {
        id,
        reads,
        fit,
        unfitted,
        band,
        replay,
        survives,
    }
}

/// `measured.estimator`: the observed range, quoted to two decimal places, rounded outward.
///
/// Rounded to the nearest millionth first, so a value that *is* 0.23 and arrives as
/// 0.23000000000000001 is quoted 0.23 rather than ceilinged to 0.24.
pub fn outward(lo: f64, hi: f64) -> (f64, f64) {
    let hundredths = |v: f64| (v * 100.0 * 1e6).round() / 1e6;
    (
        hundredths(lo).floor() / 100.0,
        hundredths(hi).ceil() / 100.0,
    )
}

/// Nodes added per authored commit between point `i` and the nearest earlier point at least
/// `n` authored commits back. `None` when no point is that far back.
pub fn window(points: &[Point], i: usize, n: usize) -> Option<f64> {
    let at = points[i];
    let from = points[..i]
        .iter()
        .rev()
        .find(|p| p.authored + n <= at.authored)?;
    Some((at.nodes as f64 - from.nodes as f64) / (at.authored - from.authored) as f64)
}

/// `ln(nodes / authored) = a + b · ln(authored)`, least squares over the members' heads.
/// `None` under three usable heads.
pub fn decay_curve(members: &[&Trajectory]) -> Option<(f64, f64)> {
    let xy: Vec<(f64, f64)> = members
        .iter()
        .map(|t| head(t))
        .filter(|p| p.authored > 0 && p.nodes > 0)
        .map(|p| {
            let k = p.authored as f64;
            (k.ln(), (p.nodes as f64 / k).ln())
        })
        .collect();
    if xy.len() < 3 {
        return None;
    }
    let n = xy.len() as f64;
    let mx = xy.iter().map(|p| p.0).sum::<f64>() / n;
    let my = xy.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = xy.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let b = xy.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / sxx;
    Some((my - b * mx, b))
}

/// How far a point sits from the curve, in natural-log units of nodes per authored commit.
pub fn residual((a, b): (f64, f64), p: Point) -> Option<f64> {
    if p.authored == 0 || p.nodes == 0 {
        return None;
    }
    let k = p.authored as f64;
    Some((p.nodes as f64 / k).ln() - (a + b * k.ln()))
}

/// Spearman's ρ with ties given their average rank.
pub fn spearman(series: &[(usize, f64)]) -> Option<f64> {
    if series.len() < 3 {
        return None;
    }
    let xs = ranks(&series.iter().map(|p| p.0 as f64).collect::<Vec<_>>());
    let ys = ranks(&series.iter().map(|p| p.1).collect::<Vec<_>>());
    let n = xs.len() as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let vx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    let vy: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
    (vx > 0.0 && vy > 0.0).then(|| cov / (vx * vy).sqrt())
}

fn ranks(v: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..v.len()).collect();
    order.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
    let mut out = vec![0.0; v.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && v[order[j + 1]] == v[order[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0;
        for &o in &order[i..=j] {
            out[o] = avg;
        }
        i = j + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traj(
        letter: &str,
        f: impl Fn(usize) -> usize,
        ages: impl Iterator<Item = usize>,
    ) -> Trajectory {
        Trajectory {
            letter: letter.into(),
            points: ages
                .map(|k| Point {
                    authored: k,
                    nodes: f(k),
                })
                .collect(),
        }
    }

    #[test]
    fn the_estimator_rounds_outward_and_not_past_an_exact_endpoint() {
        assert_eq!(outward(0.1249, 0.2623), (0.12, 0.27));
        assert_eq!(outward(0.23, 0.75), (0.23, 0.75));
        assert_eq!(outward(-0.173, 0.407), (-0.18, 0.41));
    }

    #[test]
    fn a_window_reads_from_the_nearest_point_far_enough_back() {
        let p = |authored, nodes| Point { authored, nodes };
        let points = [p(10, 5), p(30, 15), p(40, 30), p(60, 40)];
        // From 60, the nearest point at least 25 back is 30: (40 - 15) / 30.
        assert_eq!(window(&points, 3, 25), Some(25.0 / 30.0));
        assert_eq!(window(&points, 1, 25), None);
    }

    #[test]
    fn the_decay_fit_recovers_the_curve_it_was_drawn_from() {
        let members: Vec<Trajectory> = [73usize, 122, 247, 1312]
            .iter()
            .enumerate()
            .map(|(i, &k)| Trajectory {
                letter: i.to_string(),
                points: vec![Point {
                    authored: k,
                    nodes: (2.0 * (k as f64).powf(0.7)).round() as usize,
                }],
            })
            .collect();
        let refs: Vec<&Trajectory> = members.iter().collect();
        let (a, b) = decay_curve(&refs).unwrap();
        assert!((a.exp() - 2.0).abs() < 0.05, "a = {}", a.exp());
        assert!((b + 0.3).abs() < 0.01, "b = {b}");
    }

    #[test]
    fn spearman_reads_a_monotone_series_as_one_and_ties_as_a_shared_rank() {
        let up: Vec<(usize, f64)> = (1..10).map(|k| (k, (k * k) as f64)).collect();
        assert!((spearman(&up).unwrap() - 1.0).abs() < 1e-12);
        let down: Vec<(usize, f64)> = (1..10).map(|k| (k, -(k as f64))).collect();
        assert!((spearman(&down).unwrap() + 1.0).abs() < 1e-12);
        assert_eq!(spearman(&[(1, 2.0), (2, 2.0), (3, 2.0)]), None);
    }

    /// The half that makes a rejection mean something: a statistic that does not age passes.
    /// Three members accreting at their own steady rates, at different ages, never leave a
    /// band fitted from their heads.
    #[test]
    fn a_steady_rate_survives_its_own_replay() {
        let members = vec![
            traj("A", |k| k, (1..=80).step_by(3)),
            traj("B", |k| 2 * k, (1..=400).step_by(7)),
            traj("C", |k| 3 * k, (1..=1300).step_by(11)),
        ];
        let a = assess(&members);
        let w25 = a
            .candidates
            .iter()
            .find(|c| c.id == "trailing-window-25")
            .unwrap();
        assert_eq!(w25.survives, Some(true), "{w25:#?}");
    }

    /// And the half #692 is the record of: accretion that slows with age, as every member of
    /// the cluster's does, is exited by ageing on the curve as well as on the window.
    #[test]
    fn a_decaying_rate_is_rejected_by_its_own_replay() {
        let decay = |scale: f64| move |k: usize| (scale * (k as f64).powf(0.5)) as usize;
        let members = vec![
            traj("A", decay(9.0), (1..=73).step_by(2)),
            traj("B", decay(4.0), (1..=250).step_by(3)),
            traj("C", decay(14.0), (1..=1300).step_by(5)),
        ];
        let a = assess(&members);
        assert_eq!(a.ages, Some((73, 1296)));
        for id in ["trailing-window-25", "decay-curve"] {
            let c = a.candidates.iter().find(|c| c.id == id).unwrap();
            assert_eq!(c.survives, Some(false), "{id}: {c:#?}");
        }
    }

    #[test]
    fn a_window_longer_than_a_history_leaves_that_member_unfitted() {
        let members = vec![
            traj("A", |k| k, (1..=73).step_by(4)),
            traj("B", |k| k, (1..=400).step_by(4)),
            traj("C", |k| k, (1..=900).step_by(4)),
        ];
        let a = assess(&members);
        let w100 = a
            .candidates
            .iter()
            .find(|c| c.id == "trailing-window-100")
            .unwrap();
        assert_eq!(w100.unfitted, vec!["A".to_string()]);
        assert_eq!(w100.fit.len(), 2);
    }
}
