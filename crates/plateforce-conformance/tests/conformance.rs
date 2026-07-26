//! Conformance against the frozen reference run.
//!
//! The committed fixture runs everywhere and a breach fails the build. The full corpus
//! runs where it is present, which is one machine, because 40 of its 41 subjects may
//! not be redistributed.

use plateforce_conformance::bindings::ReferenceBindings;
use plateforce_conformance::corpus::{Corpus, CorpusFormat};
use plateforce_conformance::{compare, parse_reference, Agreement, Tolerance};
use plateforce_core::{read_trial_from_path, Trial};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CORPUS_VARIABLE: &str = "PLATEFORCE_CONFORMANCE_CORPUS";
const REFERENCE_VARIABLE: &str = "PLATEFORCE_CONFORMANCE_REFERENCE";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
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
fn the_full_corpus_reproduces_the_reference() {
    let (Ok(corpus_root), Ok(reference_path)) = (
        std::env::var(CORPUS_VARIABLE),
        std::env::var(REFERENCE_VARIABLE),
    ) else {
        eprintln!(
            "skipped: set {CORPUS_VARIABLE} and {REFERENCE_VARIABLE} to run against the full corpus"
        );
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
