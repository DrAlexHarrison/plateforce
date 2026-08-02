//! The Rust landing-shape rule places takeoff where the reference implementation did.
//!
//! The rule was ruled and measured in a Python harness and never reached the software, so
//! the two placed takeoff by different rules. A port that merely looks equivalent leaves two
//! implementations of one quantity standing, which is the duplication this project exists to
//! publish about. Equality is asserted sample for sample: both read integer sample indices
//! off the same trace under the same threshold, so anything less than identical hides a
//! divergence.
//!
//! Two tiers, because only subject 01 is ever public. The committed fixture carries subject
//! 01 and runs everywhere. The full corpus runs when a reader has it on disk, named by
//! `PLATEFORCE_CORPUS` and `PLATEFORCE_REFERENCE_PLACEMENTS`, and reports its own
//! denominator rather than passing quietly when it finds nothing.
//!
//! To watch this fail, move `landing_rise_rate_floor_bodyweights_per_second` far enough to
//! cross a real rise rate. Measured against the 246 trials: 19.0 moves nothing, because no
//! run on this corpus rises at between 19 and 20 bodyweights per second, so a small nudge
//! proves the gate works only if you already knew where the values were. 25.0 moves one
//! placement, 12.0 moves two, and 0.0 moves four.

use std::collections::BTreeMap;
use std::path::PathBuf;

use plateforce_conformance::corpus::{index_corpus, CorpusFormat};
use plateforce_core::read_trial_from_path;
use plateforce_core::takeoff::landing_shape::{takeoff_by_landing_shape, LandingShapeSpec};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const THRESHOLD_NEWTONS: f64 = 20.0;

/// One reference placement: the weighing figure the reference used, the sample it placed
/// takeoff on, and how many landings it found.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placement {
    system_weight_newtons: f64,
    takeoff_index: Option<usize>,
    flight_count: usize,
}

fn parse_placements(text: &str) -> BTreeMap<(u32, u32), Placement> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let key = (
                fields[0].parse().expect("subject number"),
                fields[1].parse().expect("trial number"),
            );
            (
                key,
                Placement {
                    system_weight_newtons: fields[2].parse().expect("weighing figure"),
                    takeoff_index: fields[3].parse().ok(),
                    flight_count: fields[4].parse().expect("landing count"),
                },
            )
        })
        .collect()
}

/// What this build places, run under exactly the weighing figure the reference used, so a
/// difference in weighing cannot be mistaken for a difference in the rule.
///
/// The committed fixtures carry one column and the corpus export carries the vertical
/// channel third, which is a difference in the file rather than in the trace: the six
/// fixtures are sample-for-sample the six corpus traces.
fn placed_here(path: &PathBuf, reference: &Placement) -> (Option<usize>, usize) {
    let single_column = path.to_string_lossy().ends_with(".force.txt");
    let (delimiter, column) = if single_column { (',', 0) } else { ('\t', 2) };
    let (trial, _report) = read_trial_from_path(path, delimiter, column, SAMPLE_RATE_HZ)
        .expect("the corpus reader stopped reading its own corpus");
    takeoff_by_landing_shape(
        trial.force(),
        reference.system_weight_newtons,
        THRESHOLD_NEWTONS,
        SAMPLE_RATE_HZ,
        &LandingShapeSpec::default(),
    )
}

fn compare(
    trials: &BTreeMap<(u32, u32), PathBuf>,
    reference: &BTreeMap<(u32, u32), Placement>,
    label: &str,
) {
    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut differences: Vec<String> = Vec::new();

    for (key, expected) in reference {
        let Some(path) = trials.get(key) else {
            continue;
        };
        compared += 1;
        let (index, count) = placed_here(path, expected);
        if index == expected.takeoff_index && count == expected.flight_count {
            identical += 1;
        } else if differences.len() < 5 {
            differences.push(format!(
                "  subject {} trial {}: reference {:?} over {} landings, this build {:?} over {}",
                key.0, key.1, expected.takeoff_index, expected.flight_count, index, count
            ));
        }
    }

    println!("{label}: {identical} of {compared} placements identical");
    assert!(
        compared > 0,
        "{label}: nothing was compared, so this proves nothing"
    );
    assert_eq!(
        identical,
        compared,
        "{} of {compared} placements differ:\n{}",
        compared - identical,
        differences.join("\n")
    );
}

#[test]
fn the_rust_landing_shape_rule_places_takeoff_where_the_reference_did_on_subject_01() {
    let fixtures = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures"));
    let reference = parse_placements(
        &std::fs::read_to_string(fixtures.join("landing_shape_placements_subject01.tsv"))
            .expect("the committed reference placements"),
    );
    let trials: BTreeMap<(u32, u32), PathBuf> = (1..=6)
        .map(|trial| {
            (
                (1, trial),
                fixtures.join(format!("subject01_trial{trial}.force.txt")),
            )
        })
        .filter(|(_, path)| path.exists())
        .collect();
    compare(&trials, &reference, "subject 01");
}

/// The full corpus, for a reader who holds it. It is re-identifiable, so neither the traces
/// nor their per-trial placements are committed here.
#[test]
fn the_two_rules_agree_across_the_whole_corpus_when_it_is_on_disk() {
    let (Ok(corpus), Ok(placements)) = (
        std::env::var("PLATEFORCE_CORPUS"),
        std::env::var("PLATEFORCE_REFERENCE_PLACEMENTS"),
    ) else {
        println!("the corpus was not named, so the committed subject-01 tier is the whole check");
        return;
    };
    let reference = parse_placements(
        &std::fs::read_to_string(&placements).expect("the reference placements named"),
    );
    let trials =
        index_corpus(&PathBuf::from(&corpus), &CorpusFormat::default()).expect("the corpus named");
    compare(&trials, &reference, "the whole corpus");
}
