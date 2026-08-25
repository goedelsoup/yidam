//! `yidam bench --scaling` — the arms that are functions of N, over generated corpora.
//!
//! `examples/streamflow` is 8 nodes with a class-narrowing ceiling of 4x against a claimed
//! 10-100x. A run over it is a regression guard and cannot be evidence, whatever the
//! traversal does. Evidence needs corpora large enough for the claim to be visible, and
//! nobody has four thousand hand-authored nodes to hand. So they are generated, from
//! parameters derived from a real corpus and committed in [`scaling.toml`] beside this file.
//!
//! # Which arms scale, and which do not
//!
//! Only two of the three arms are functions of N, and this is the amendment that reshaped
//! #264 rather than an implementation convenience:
//!
//! - **full-scan** grows with N. It reads everything, so its cost is the corpus and its
//!   precision is `|expect| / N`. **This is the arm the paper's O(*n*) claim is about.**
//! - **anchored** is bounded by depth and branching, not by N — the claim under test.
//! - **flat** is `k` candidates whatever N is. It is *analytically* constant, so measuring
//!   it across sizes would spend an embedding run to rediscover the `k` we chose.
//!
//! The flat arm is therefore excluded here by argument, and the report says so rather than
//! omitting it silently. One consequence is worth stating plainly: because neither scaling
//! arm needs retrieval, **`--scaling` needs no vector index and does not refuse on a light
//! build.** That is not a loosening of the rule the single-corpus run enforces; it is that
//! the rule's reason — a keyword baseline is not RAG — does not arise where there is no
//! retrieval arm at all.
//!
//! # The corpora are in memory
//!
//! Nothing is written to disk. Both scaling arms are structural, so a generated corpus is a
//! graph and a per-node size, not a directory of YAML nobody reads.

use anyhow::{bail, Result};

use super::{ArmReport, CorpusShape, CLAIMED_NARROWING_FLOOR};

/// The committed generator parameters, compiled in.
///
/// `include_str!` rather than a runtime read: the config is not the *repository's* to vary
/// — a benchmark whose generator can be retuned per corpus is measuring the tuner. A repo
/// that wants different parameters edits this file and says why, in the file.
const CONFIG_TOML: &str = include_str!("scaling.toml");

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ScalingConfig {
    pub version: u32,
    /// Where the numbers came from, printed in every report.
    pub source: String,
    pub mean_out_degree: f64,
    pub node_chars: usize,
    pub class_shares: Vec<f64>,
    pub sizes: Vec<usize>,
    pub goals_per_size: usize,
    pub max_hops: usize,
    pub seed: u64,
}

pub fn config() -> Result<ScalingConfig> {
    let config: ScalingConfig = toml::from_str(CONFIG_TOML)?;
    if config.version != CONFIG_VERSION {
        bail!(
            "scaling config declares version {} and this build reads {CONFIG_VERSION}",
            config.version
        );
    }
    if config.class_shares.is_empty() || config.sizes.is_empty() {
        bail!("scaling config declares no classes or no sizes");
    }
    Ok(config)
}

// ── determinism ───────────────────────────────────────────────────────────────

/// xorshift64*, seeded from the config.
///
/// Written out rather than taken as a dependency, and fixed rather than seeded from the
/// clock, because a benchmark whose corpora move between runs cannot be a regression guard.
/// The generator is the input; an input that changes when nothing else did makes every
/// comparison across commits meaningless.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // A zero state is a fixed point of xorshift; the constant is arbitrary and only has
        // to be non-zero.
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            n => (self.next() % n as u64) as usize,
        }
    }
}

// ── the generated corpus ──────────────────────────────────────────────────────

pub struct SynthNode {
    pub class: usize,
    /// `(target index, relationship index)`, in generation order.
    pub out: Vec<(usize, usize)>,
}

pub struct SynthCorpus {
    /// Node counts per class, index-aligned with [`SynthNode::class`].
    pub class_sizes: Vec<usize>,
    pub nodes: Vec<SynthNode>,
}

impl SynthCorpus {
    fn shape(&self) -> CorpusShape {
        let smallest = self
            .class_sizes
            .iter()
            .enumerate()
            .min_by_key(|(index, size)| (**size, *index));
        let (smallest_index, smallest_class_size) = match smallest {
            Some((index, size)) => (index, *size),
            None => (0, 0),
        };
        let narrowing_ceiling = match smallest_class_size {
            0 => 0.0,
            size => self.nodes.len() as f64 / size as f64,
        };
        CorpusShape {
            nodes: self.nodes.len(),
            classes: self.class_sizes.len(),
            smallest_class: format!("c{smallest_index}"),
            smallest_class_size,
            narrowing_ceiling,
            ceiling_reaches_claim: narrowing_ceiling >= CLAIMED_NARROWING_FLOOR,
        }
    }
}

/// Allocate `total` nodes across the configured class shares, largest remainder first.
///
/// Classes allocated nothing are **dropped rather than floored to one**. Flooring would
/// give a small corpus more classes than it has room for and, worse, would manufacture a
/// singleton class — which is the numerator of the narrowing ceiling, the one number this
/// whole exercise is trying not to fake. At N = 8 the shares support six classes; the
/// report says six, and the ceiling it prints is the one that corpus actually has.
pub fn allocate(shares: &[f64], total: usize) -> Vec<usize> {
    let sum: f64 = shares.iter().sum();
    let exact: Vec<f64> = shares.iter().map(|s| s / sum * total as f64).collect();
    let mut sizes: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    let mut short = total - sizes.iter().sum::<usize>();

    // Largest remainder, ties by index so the result does not depend on sort stability.
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|a, b| {
        let (ra, rb) = (exact[*a] - exact[*a].floor(), exact[*b] - exact[*b].floor());
        rb.total_cmp(&ra).then(a.cmp(b))
    });
    for index in order.iter().cycle().take(short.max(0)) {
        if short == 0 {
            break;
        }
        sizes[*index] += 1;
        short -= 1;
    }
    sizes.retain(|size| *size > 0);
    sizes
}

/// Build one corpus of `size` nodes from the committed parameters.
pub fn generate(config: &ScalingConfig, size: usize) -> SynthCorpus {
    let mut rng = Rng::new(config.seed ^ size as u64);
    let class_sizes = allocate(&config.class_shares, size);
    let class_count = class_sizes.len();

    // Each class licenses up to three relationships, each toward one class. Three because
    // it is what the two real ontologies on hand declare per class (streamflow's `reach`
    // declares three, `gage` and `concept` one each), and because a class licensing every
    // other class would make a typed hop no narrower than an untyped one.
    let licensed: Vec<Vec<(usize, usize)>> = (0..class_count)
        .map(|from| {
            (0..3)
                .map(|slot| (from * 3 + slot, rng.below(class_count)))
                .collect()
        })
        .collect();

    let mut class_of = Vec::with_capacity(size);
    for (class, count) in class_sizes.iter().enumerate() {
        class_of.extend(std::iter::repeat_n(class, *count));
    }

    // Out-degree averages `mean_out_degree` by spending its fractional part as a
    // probability rather than by rounding, which would quantise 4.18 to 4 and lose 4% of
    // the edges the real corpus has.
    let floor = config.mean_out_degree.floor() as usize;
    let fraction = config.mean_out_degree - config.mean_out_degree.floor();
    let mut nodes: Vec<SynthNode> = Vec::with_capacity(size);
    for index in 0..size {
        let class = class_of[index];
        let extra = usize::from((rng.below(10_000) as f64) < fraction * 10_000.0);
        let degree = floor + extra;
        let mut out = Vec::with_capacity(degree);
        for _ in 0..degree {
            let Some((relationship, target_class)) = licensed
                .get(class)
                .and_then(|edges| edges.get(rng.below(edges.len().max(1))))
                .copied()
            else {
                continue;
            };
            // Uniform over the target class's members, and a self-edge is dropped rather
            // than retried: retrying would bias degree upward on the smallest classes,
            // which are the ones the ceiling is computed from.
            let candidates: Vec<usize> = (0..size)
                .filter(|other| class_of[*other] == target_class && *other != index)
                .collect();
            if candidates.is_empty() {
                continue;
            }
            out.push((candidates[rng.below(candidates.len())], relationship));
        }
        nodes.push(SynthNode { class, out });
    }
    SynthCorpus { class_sizes, nodes }
}

// ── generated goals ───────────────────────────────────────────────────────────

/// A goal whose expected answer is read off the graph rather than authored.
///
/// This is the one thing a synthetic corpus is unambiguously better at: the answer is
/// derived from the structure, so it cannot be chosen to favour an arm. The corpus is still
/// circular — that is what the config file is for — but the *scoring* is not.
pub struct SynthGoal {
    pub hops: usize,
    pub expect: Vec<usize>,
}

pub fn goals(config: &ScalingConfig, corpus: &SynthCorpus, size: usize) -> Vec<SynthGoal> {
    let mut rng = Rng::new(config.seed.wrapping_add(1) ^ size as u64);
    let mut out = Vec::new();
    // Bounded rather than `while out.len() < n`: a corpus can be too sparse to yield the
    // requested number of goals, and a benchmark that spins looking for one is worse than a
    // benchmark that reports it found fewer.
    for _ in 0..config.goals_per_size * 8 {
        if out.len() >= config.goals_per_size {
            break;
        }
        let want = 1 + rng.below(config.max_hops.max(1));
        let mut frontier = vec![rng.below(corpus.nodes.len().max(1))];
        let mut walked = 0;
        for _ in 0..want {
            // A relationship is chosen per hop, from what the frontier actually authors.
            // Reusing one name across hops looks natural and is wrong here: relationship
            // identity is per-class, so a second hop keyed on the first hop's relationship
            // can only match when the path stays inside one class. That bug made 2-hop
            // goals die on the vine — the generated set came out at a mean of 1.0 hops, and
            // a benchmark whose goals are all single-hop understates the one thing typed
            // traversal is for.
            let choices: Vec<usize> = {
                let mut seen: Vec<usize> = frontier
                    .iter()
                    .flat_map(|node| corpus.nodes[*node].out.iter())
                    .map(|(_, relationship)| *relationship)
                    .collect();
                seen.sort_unstable();
                seen.dedup();
                seen
            };
            if choices.is_empty() {
                break;
            }
            let relationship = choices[rng.below(choices.len())];
            let mut next: Vec<usize> = frontier
                .iter()
                .flat_map(|node| corpus.nodes[*node].out.iter())
                .filter(|(_, r)| *r == relationship)
                .map(|(target, _)| *target)
                .collect();
            next.sort_unstable();
            next.dedup();
            if next.is_empty() {
                break;
            }
            frontier = next;
            walked += 1;
        }
        if walked == 0 || frontier.is_empty() {
            continue;
        }
        out.push(SynthGoal {
            hops: walked,
            expect: frontier,
        });
    }
    out
}

/// Candidates left after narrowing to the classes the answer lives in.
///
/// This is §3's *focused scan*: "if a corpus has N nodes and an ontological class covers C
/// nodes, focused scan is C/N of blind scan's cost." It is measurable here without a query
/// executor, because it is arithmetic over the class sizes rather than a traversal.
///
/// **What it assumes, stated rather than buried:** that the goal's class is already known.
/// That is exactly what anchoring is supposed to provide, so this is the *upper bound* of
/// what class narrowing alone can buy — not a substitute for the anchored arm, which still
/// has to find the right nodes inside the class.
fn focused_candidates(corpus: &SynthCorpus, goal: &SynthGoal) -> usize {
    let mut classes: Vec<usize> = goal
        .expect
        .iter()
        .map(|node| corpus.nodes[*node].class)
        .collect();
    classes.sort_unstable();
    classes.dedup();
    classes
        .iter()
        .map(|class| corpus.class_sizes.get(*class).copied().unwrap_or_default())
        .sum()
}

// ── the report ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct ScalingRow {
    pub corpus: CorpusShape,
    pub goals: usize,
    /// Mean expected-answer size, the numerator of full-scan precision.
    pub mean_expected: f64,
    /// Mean generated path length, so a reader can see the goals are not all one hop.
    pub mean_hops: f64,
    pub full_scan: ArmReport,
    pub focused_scan: ArmReport,
    pub anchored: ArmReport,
}

#[derive(Debug, serde::Serialize)]
pub struct ScalingReport {
    pub config: ScalingConfig,
    pub standing: &'static str,
    pub standing_reason: String,
    pub flat_excluded_because: String,
    pub rows: Vec<ScalingRow>,
}

const FLAT_EXCLUDED: &str = "top-k retrieval considers k candidates whatever N is, so its \
     cost and precision are constant in N by construction. Measuring it across sizes would \
     spend an embedding run to rediscover the k we chose. O(n) lives on the full-scan arm.";

const ANCHORED_PENDING: &str = "the query executor does not exist yet (#261). The corpora \
     and goals here are deterministic, so the anchored arm can be run over exactly these \
     inputs when it lands, and compared with this run.";

pub fn run() -> Result<ScalingReport> {
    let config = config()?;
    let mut rows = Vec::new();
    for size in config.sizes.clone() {
        let corpus = generate(&config, size);
        let generated = goals(&config, &corpus, size);
        let shape = corpus.shape();

        // The full-scan arm reads every node for every goal. Aggregated rather than
        // per-goal: the cost is identical across goals by definition, and only precision
        // varies, with the size of the expected answer.
        let total = corpus.nodes.len();
        let mean_expected = match generated.is_empty() {
            true => 0.0,
            false => {
                generated.iter().map(|g| g.expect.len()).sum::<usize>() as f64
                    / generated.len() as f64
            }
        };
        let mut full_scan = ArmReport::empty("full-scan");
        full_scan.ran = !generated.is_empty();
        full_scan.candidates = Some(total);
        full_scan.nodes_read = Some(total);
        full_scan.tokens = Some(total * config.node_chars / 4);
        full_scan.recall = generated.is_empty().then_some(0.0).or(Some(1.0));
        full_scan.precision = match total {
            0 => None,
            n => Some(mean_expected / n as f64),
        };

        // Focused scan: §3's C/N, averaged over goals.
        let mut focused = ArmReport::empty("focused-scan");
        if !generated.is_empty() {
            let candidates: Vec<usize> = generated
                .iter()
                .map(|goal| focused_candidates(&corpus, goal))
                .collect();
            let mean_candidates = candidates.iter().sum::<usize>() as f64 / candidates.len() as f64;
            let mean_precision = generated
                .iter()
                .zip(&candidates)
                .map(|(goal, seen)| goal.expect.len() as f64 / (*seen).max(1) as f64)
                .sum::<f64>()
                / generated.len() as f64;
            focused.ran = true;
            focused.candidates = Some(mean_candidates.round() as usize);
            focused.nodes_read = Some(mean_candidates.round() as usize);
            focused.tokens = Some((mean_candidates * config.node_chars as f64 / 4.0) as usize);
            // 1 by construction, as for full scan: every expected node is inside its own
            // class, so narrowing to that class cannot miss one.
            focused.recall = Some(1.0);
            focused.precision = Some(mean_precision);
        }

        let mean_hops = match generated.is_empty() {
            true => 0.0,
            false => {
                generated.iter().map(|g| g.hops).sum::<usize>() as f64 / generated.len() as f64
            }
        };

        rows.push(ScalingRow {
            corpus: shape,
            goals: generated.len(),
            mean_expected,
            mean_hops,
            full_scan,
            focused_scan: focused,
            anchored: ArmReport::unavailable("anchored", ANCHORED_PENDING),
        });
    }

    Ok(ScalingReport {
        standing: "baseline",
        standing_reason: format!(
            "this run establishes the O(n) baseline and the corpora; it is not yet a \
             comparison. {ANCHORED_PENDING}"
        ),
        flat_excluded_because: FLAT_EXCLUDED.to_string(),
        config,
        rows,
    })
}

pub fn render(report: &ScalingReport) -> String {
    let c = &report.config;
    let mut out = format!(
        "generated corpora — parameters from {}\n  mean out-degree {:.2}  node size {} char(s)  \
         {} class share(s)  seed {}\n\n{}\n\nflat arm excluded: {}\n\n",
        c.source,
        c.mean_out_degree,
        c.node_chars,
        c.class_shares.len(),
        c.seed,
        report.standing_reason,
        report.flat_excluded_because,
    );
    out.push_str(&format!(
        "{:>6}  {:>7}  {:>7}  {:>5}  {:>12}  {:>10}  {:>13}  {:>10}  {:>9}\n",
        "N",
        "classes",
        "ceiling",
        "hops",
        "full-scan tok",
        "precision",
        "focused tok",
        "precision",
        "narrowing"
    ));
    for row in &report.rows {
        let full = row.full_scan.tokens.unwrap_or(0);
        let focused = row.focused_scan.tokens.unwrap_or(0);
        out.push_str(&format!(
            "{:>6}  {:>7}  {:>6.1}x  {:>5.1}  {:>12}  {:>9.2}%  {:>13}  {:>9.2}%  {:>8.1}x\n",
            row.corpus.nodes,
            row.corpus.classes,
            row.corpus.narrowing_ceiling,
            row.mean_hops,
            full,
            row.full_scan.precision.unwrap_or(0.0) * 100.0,
            focused,
            row.focused_scan.precision.unwrap_or(0.0) * 100.0,
            match focused {
                0 => 0.0,
                f => full as f64 / f as f64,
            },
        ));
    }
    out.push_str("\nanchored arm: not run — ");
    out.push_str(&super::one_line(ANCHORED_PENDING));
    out.push('\n');
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_config_parses_and_its_shares_sum_to_one() {
        let config = config().unwrap();
        let sum: f64 = config.class_shares.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9, "shares sum to {sum}");
        assert_eq!(config.sizes, vec![8, 64, 512, 4096]);
    }

    /// The two order statistics the config claims to reproduce, checked against the record
    /// on #264: a smallest class of ~0.98% (ceiling 102x) and a median of ~3.9% (median
    /// narrowing 25.5x). If someone retunes the vector, this is what stops the provenance
    /// comment from quietly becoming false.
    #[test]
    fn the_class_shares_reproduce_the_measured_order_statistics() {
        let config = config().unwrap();
        let mut shares = config.class_shares.clone();
        shares.sort_by(f64::total_cmp);
        let ceiling = 1.0 / shares[0];
        assert!(
            (90.0..=110.0).contains(&ceiling),
            "ceiling {ceiling:.1}x is not the measured 102x"
        );
        let mid = shares.len() / 2;
        let median = (shares[mid - 1] + shares[mid]) / 2.0;
        let narrowing = 1.0 / median;
        assert!(
            (23.0..=28.0).contains(&narrowing),
            "median narrowing {narrowing:.1}x is not the measured 25.5x"
        );
    }

    #[test]
    fn allocation_spends_every_node_and_drops_the_classes_it_cannot_fill() {
        let config = config().unwrap();
        for size in [8usize, 64, 512, 4096] {
            let sizes = allocate(&config.class_shares, size);
            assert_eq!(sizes.iter().sum::<usize>(), size, "N={size}");
            assert!(
                sizes.iter().all(|s| *s > 0),
                "a zero-sized class at N={size}"
            );
            assert!(sizes.len() <= config.class_shares.len());
        }
    }

    /// The finding the ceiling column exists to show: a small corpus cannot express a small
    /// class share, so its ceiling is low for arithmetic reasons and not because traversal
    /// failed. This is streamflow's problem, generated.
    #[test]
    fn the_ceiling_grows_with_n_and_is_the_reason_small_corpora_prove_nothing() {
        let config = config().unwrap();
        let ceiling = |size: usize| {
            let sizes = allocate(&config.class_shares, size);
            size as f64 / *sizes.iter().min().unwrap() as f64
        };
        assert!(ceiling(8) < CLAIMED_NARROWING_FLOOR, "{}", ceiling(8));
        assert!(
            ceiling(4096) >= CLAIMED_NARROWING_FLOOR,
            "{}",
            ceiling(4096)
        );
        assert!(ceiling(8) < ceiling(512));
    }

    /// A benchmark whose inputs move between runs cannot be a regression guard.
    #[test]
    fn generation_is_deterministic() {
        let config = config().unwrap();
        let (a, b) = (generate(&config, 64), generate(&config, 64));
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (left, right) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!(left.class, right.class);
            assert_eq!(left.out, right.out);
        }
        let (ga, gb) = (goals(&config, &a, 64), goals(&config, &b, 64));
        assert_eq!(ga.len(), gb.len());
        for (left, right) in ga.iter().zip(&gb) {
            assert_eq!(left.expect, right.expect);
        }
    }

    #[test]
    fn the_generated_degree_tracks_the_configured_mean() {
        let config = config().unwrap();
        let corpus = generate(&config, 4096);
        let edges: usize = corpus.nodes.iter().map(|n| n.out.len()).sum();
        let mean = edges as f64 / corpus.nodes.len() as f64;
        // Loose: a target class can be empty at small N and a self-edge is dropped rather
        // than retried, so the realised mean sits at or just below the configured one.
        assert!(
            (config.mean_out_degree - 0.6..=config.mean_out_degree + 0.1).contains(&mean),
            "realised mean degree {mean:.2} against configured {:.2}",
            config.mean_out_degree
        );
    }

    #[test]
    fn every_generated_goal_has_a_non_empty_answer_read_off_the_graph() {
        let config = config().unwrap();
        let corpus = generate(&config, 512);
        let generated = goals(&config, &corpus, 512);
        assert!(!generated.is_empty());
        for goal in &generated {
            assert!(!goal.expect.is_empty());
            assert!(goal.hops >= 1 && goal.hops <= config.max_hops);
            assert!(goal.expect.iter().all(|n| *n < corpus.nodes.len()));
        }
    }

    /// The whole point of the exercise: full-scan cost is linear in N, and its precision
    /// decays as 1/N. Both are what O(*n*) means, and neither is true of top-*k*.
    #[test]
    fn full_scan_cost_grows_with_n_while_its_precision_decays() {
        let report = run().unwrap();
        assert_eq!(report.rows.len(), 4);
        for pair in report.rows.windows(2) {
            let (small, large) = (&pair[0], &pair[1]);
            assert!(
                large.full_scan.tokens > small.full_scan.tokens,
                "full-scan cost must grow with N"
            );
            assert!(
                large.full_scan.precision <= small.full_scan.precision,
                "full-scan precision must not improve as the corpus grows"
            );
        }
        let biggest = report.rows.last().unwrap();
        assert!(!biggest.anchored.ran, "the anchored arm cannot have run");
    }

    /// Relationship identity is per-class, so a walk that keys every hop on the first hop's
    /// relationship can only stay inside one class — and the generated set came out at a
    /// mean of 1.0 hops, all of them single. A benchmark whose goals are all one hop
    /// understates the one thing typed traversal is for, so the mean is pinned here.
    #[test]
    fn the_generated_goals_are_not_all_single_hop() {
        let config = config().unwrap();
        let corpus = generate(&config, 512);
        let generated = goals(&config, &corpus, 512);
        let multi = generated.iter().filter(|g| g.hops > 1).count();
        assert!(
            multi * 4 >= generated.len(),
            "only {multi} of {} goals walk more than one hop",
            generated.len()
        );
    }

    /// §3's regime, and the row the paper's 10-100x claim actually lives on. The narrowing
    /// is *not* asserted to be monotonic: a multi-hop goal can land in more classes than a
    /// single-hop one, so focused scan reads more at some sizes than at the size below.
    /// Smoothing that out would be choosing a curve.
    #[test]
    fn focused_scan_reads_less_than_full_scan_and_crosses_the_claimed_floor() {
        let report = run().unwrap();
        for row in &report.rows {
            let full = row.full_scan.tokens.unwrap();
            let focused = row.focused_scan.tokens.unwrap();
            assert!(
                focused < full,
                "focused scan must read less than everything"
            );
            assert_eq!(row.focused_scan.recall, Some(1.0));
        }
        let smallest = report.rows.first().unwrap();
        let largest = report.rows.last().unwrap();
        let narrowing = |row: &ScalingRow| {
            row.full_scan.tokens.unwrap() as f64 / row.focused_scan.tokens.unwrap() as f64
        };
        assert!(
            narrowing(smallest) < CLAIMED_NARROWING_FLOOR,
            "an 8-node corpus cannot reach the claimed range: {:.1}x",
            narrowing(smallest)
        );
        assert!(
            narrowing(largest) >= CLAIMED_NARROWING_FLOOR,
            "the largest corpus should reach it: {:.1}x",
            narrowing(largest)
        );
    }

    #[test]
    fn the_report_prints_the_parameters_it_assumed() {
        let report = run().unwrap();
        let text = render(&report);
        assert!(text.contains("102-node derived corpus"), "{text}");
        assert!(text.contains("mean out-degree 4.18"), "{text}");
        assert!(text.contains("seed 20260823"), "{text}");
        assert!(text.contains("flat arm excluded"), "{text}");
        assert!(text.contains("not run"), "{text}");
    }
}
