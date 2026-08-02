//! Conformance against the frozen reference run.
//!
//! The committed fixture runs everywhere and a breach fails the build. The full corpus
//! runs where it is present, which is one machine, because 40 of its 41 subjects may
//! not be redistributed.

use plateforce_conformance::bindings::{analyse, ReferenceBindings};
use plateforce_conformance::corpus::{Corpus, CorpusFormat};
use plateforce_conformance::{compare, parse_reference, Agreement, Tolerance};
use plateforce_core::onset::{
    onset_final_departure_from_band, onset_noise_relative, BandSides, CrossingSearch,
    CrossingSelection, DegenerateBandPolicy,
};
use plateforce_core::statistics::{mean, standard_deviation, DispersionEstimator};
use plateforce_core::{read_trial_from_path, Trial};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CORPUS_VARIABLE: &str = "PLATEFORCE_CONFORMANCE_CORPUS";
const REFERENCE_VARIABLE: &str = "PLATEFORCE_CONFORMANCE_REFERENCE";
/// Prefixes every line that states what this run did and did not check. A conformance
/// run that quietly covered six trials instead of 244 is the failure this project
/// documents, so the coverage is printed rather than inferred from a green result.
const COVERAGE_MARKER: &str = "CONFORMANCE COVERAGE:";

fn fixtures() -> PathBuf {
    plateforce_conformance::fixtures_directory()
}

/// The committed traces are one column of numbers per file, named by subject and trial
/// number only, so the loader never sees a name to leak.
fn fixture_traces() -> BTreeMap<(u32, u32), PathBuf> {
    let mut found = BTreeMap::new();
    for entry in std::fs::read_dir(fixtures()).expect("fixtures directory") {
        let path = entry.expect("fixture entry").path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".force.txt") else {
            continue;
        };
        let Some((subject, trial)) = stem
            .strip_prefix("subject")
            .and_then(|rest| rest.split_once("_trial"))
        else {
            continue;
        };
        if let (Ok(subject), Ok(trial)) = (subject.parse(), trial.parse()) {
            found.insert((subject, trial), path);
        }
    }
    found
}

fn fixture_trial(paths: &BTreeMap<(u32, u32), PathBuf>, subject: u32, trial: u32) -> Option<Trial> {
    let path = paths.get(&(subject, trial))?;
    read_trial_from_path(path, '\t', 0, 1200.0)
        .ok()
        .map(|(trace, _)| trace)
}

fn assert_clean(report: &plateforce_conformance::ConformanceReport, what: &str) {
    for column in report.breaches() {
        for item in &column.disagreements {
            eprintln!(
                "{}: {} subject {} trial {}: reference {} against computed {}",
                what, column.name, item.subject, item.trial, item.reference, item.computed
            );
        }
    }
    assert!(
        report.breaches().is_empty(),
        "{what}: {} column(s) disagree with the frozen reference",
        report.breaches().len()
    );
    assert!(
        report.missing_from_corpus.is_empty(),
        "{what}: reference rows with no trace: {:?}",
        report.missing_from_corpus
    );
    assert!(report.trials_compared > 0, "{what}: nothing was compared");
}

/// Names what this run covered, on every run. A conformance suite that reported
/// success while checking six trials of 244 would be the defect it exists to catch.
#[test]
fn the_coverage_of_this_run_is_stated() {
    let full = std::env::var(CORPUS_VARIABLE).is_ok() && std::env::var(REFERENCE_VARIABLE).is_ok();
    if full {
        println!("{COVERAGE_MARKER} committed fixture (6 trials) AND full corpus (244 trials)");
    } else {
        println!("{COVERAGE_MARKER} committed fixture only, 6 trials of 244");
        println!(
            "{COVERAGE_MARKER} the other 238 were NOT checked. Set {CORPUS_VARIABLE} and {REFERENCE_VARIABLE} to check them"
        );
    }
}

#[test]
fn the_committed_fixture_reproduces_the_reference() {
    let text = std::fs::read_to_string(fixtures().join("reference_subject01.csv"))
        .expect("committed reference rows");
    let reference = parse_reference(&text).expect("reference parses");
    let paths = fixture_traces();
    let report = compare(
        &reference,
        &|subject, trial| fixture_trial(&paths, subject, trial),
        &ReferenceBindings::default(),
        Tolerance::default(),
    );
    assert_eq!(report.trials_compared, 6);
    assert_clean(&report, "committed fixture");
}

/// The full run is the evidence. The fixture is the guard that keeps it true.
#[test]
fn the_full_corpus_reproduces_the_reference_where_the_corpus_is_present() {
    let (Ok(corpus_root), Ok(reference_path)) = (
        std::env::var(CORPUS_VARIABLE),
        std::env::var(REFERENCE_VARIABLE),
    ) else {
        println!("{COVERAGE_MARKER} full corpus NOT run, 238 trials unchecked");
        return;
    };
    let text = std::fs::read_to_string(&reference_path).expect("reference output");
    let reference = parse_reference(&text).expect("reference parses");
    let corpus =
        Corpus::open(Path::new(&corpus_root), CorpusFormat::default()).expect("corpus opens");
    let report = compare(
        &reference,
        &|subject, trial| corpus.trial(subject, trial),
        &ReferenceBindings::default(),
        Tolerance::default(),
    );
    assert_eq!(report.trials_compared, 244);
    assert_clean(&report, "full corpus");
}

/// Every index column is held to equality, so a rule that moves by one sample fails
/// rather than being absorbed by a tolerance meant for arithmetic noise.
#[test]
fn index_columns_are_compared_exactly_and_measurements_are_not() {
    let text = std::fs::read_to_string(fixtures().join("reference_subject01.csv")).unwrap();
    let reference = parse_reference(&text).unwrap();
    let paths = fixture_traces();
    let report = compare(
        &reference,
        &|subject, trial| fixture_trial(&paths, subject, trial),
        &ReferenceBindings::default(),
        Tolerance::default(),
    );
    let exact: Vec<&str> = report
        .columns
        .iter()
        .filter(|column| column.agreement == Agreement::Exact)
        .map(|column| column.name.as_str())
        .collect();
    assert!(exact.contains(&"onset_jm"), "{exact:?}");
    assert!(exact.contains(&"takeoff_sams"), "{exact:?}");
    assert!(exact.contains(&"velocity_zero"), "{exact:?}");
    assert_eq!(exact.len(), 23, "{exact:?}");
    for column in &report.columns {
        if column.agreement == Agreement::Exact {
            assert_eq!(
                column.worst_absolute_difference, 0.0,
                "{} is not bit-exact",
                column.name
            );
        }
    }
}

/// Two readings of one published onset rule, measured on the same trials with the same
/// band, so the only difference between them is the direction of the search.
#[test]
fn the_two_readings_of_the_onset_rule_are_measured_against_each_other() {
    let (Ok(corpus_root), Ok(reference_path)) = (
        std::env::var(CORPUS_VARIABLE),
        std::env::var(REFERENCE_VARIABLE),
    ) else {
        println!("{COVERAGE_MARKER} onset-direction comparison NOT run");
        return;
    };
    let text = std::fs::read_to_string(&reference_path).expect("reference output");
    let reference = parse_reference(&text).expect("reference parses");
    let corpus =
        Corpus::open(Path::new(&corpus_root), CorpusFormat::default()).expect("corpus opens");
    let bindings = ReferenceBindings::default();
    let rate = bindings.sample_rate_hz;
    let quiet_samples = (0.5 * rate) as usize;
    let persistence = (0.175 * rate) as usize;

    let (mut identical, mut compared, mut worst_gap_samples) = (0usize, 0usize, 0usize);
    let (mut forward_far, mut backward_far) = (0usize, 0usize);
    for row in &reference {
        let Some(trial) = corpus.trial(row.subject, row.trial) else {
            continue;
        };
        let Ok(analysis) = analyse(&trial, &bindings) else {
            continue;
        };
        let (Some(takeoff), Some(trough)) = (
            analysis.takeoff_index[0],
            analysis.force_minimum_before_peak,
        ) else {
            continue;
        };
        let force = trial.force();
        let reference_newtons = mean(&force[..quiet_samples]).unwrap();
        let dispersion =
            standard_deviation(&force[..quiet_samples], DispersionEstimator::Population).unwrap();

        let forward = onset_noise_relative(
            force,
            reference_newtons,
            dispersion,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            &CrossingSearch {
                start_index: quiet_samples,
                end_index: force.len().saturating_sub(persistence),
                persistence_samples: persistence,
                selection: CrossingSelection::First,
            },
            rate,
        );
        let backward = onset_final_departure_from_band(
            force,
            reference_newtons,
            dispersion,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            trough,
            rate,
        );
        let (Ok(forward), Ok(backward)) = (forward, backward) else {
            continue;
        };
        compared += 1;
        if forward == backward {
            identical += 1;
        }
        worst_gap_samples = worst_gap_samples.max(forward.abs_diff(backward));
        let two_seconds = (2.0 * rate) as usize;
        if takeoff.saturating_sub(forward) > two_seconds {
            forward_far += 1;
        }
        if takeoff.saturating_sub(backward) > two_seconds {
            backward_far += 1;
        }
    }
    println!(
        "onset direction over {compared} trials: identical on {identical}, worst gap {worst_gap_samples} samples ({:.0} ms)",
        worst_gap_samples as f64 / rate * 1000.0
    );
    println!(
        "  onset placed more than 2 s before takeoff: forward {forward_far}, backward {backward_far}"
    );
    assert_eq!(compared, 244);
    assert_eq!(identical, 195);
    assert_eq!(worst_gap_samples, 813);
    assert_eq!(forward_far, 2);
    assert_eq!(backward_far, 0);
}

/// A commercial export writes 0.00 to jump height, time to takeoff and reactive
/// strength index together when it has no measurement, and a 0.00 cm countermovement
/// jump is not a small jump. The harness must name the trial rather than compare it.
#[test]
fn a_vendor_sentinel_row_is_excluded_and_named() {
    let header = "subject,trial,ttt_accupower,jh_accupower_cm,rsi_accupower";
    let text = format!("{header}\n7,3,0.0,0.0,0.0\n7,4,0.74,37.92,0.51\n");
    let reference = parse_reference(&text).unwrap();
    let report = compare(
        &reference,
        &|_, _| None,
        &ReferenceBindings::default(),
        Tolerance::default(),
    );
    let sentinel_rows: Vec<&plateforce_conformance::Exclusion> = report
        .exclusions
        .iter()
        .filter(|exclusion| exclusion.reason.contains("sentinel"))
        .collect();
    assert_eq!(sentinel_rows.len(), 1);
    assert_eq!((sentinel_rows[0].subject, sentinel_rows[0].trial), (7, 3));
    assert!(sentinel_rows[0].reason.contains("jh_accupower_cm"));
}

/// A reference row whose trace is absent is a gap in the evidence, not a pass.
#[test]
fn a_reference_row_with_no_trace_is_reported_rather_than_skipped() {
    let text = "subject,trial,bw_025\n9,1,600.0\n";
    let reference = parse_reference(text).unwrap();
    let report = compare(
        &reference,
        &|_, _| None,
        &ReferenceBindings::default(),
        Tolerance::default(),
    );
    assert_eq!(report.missing_from_corpus, vec![(9, 1)]);
    assert!(!report.is_clean());
}
