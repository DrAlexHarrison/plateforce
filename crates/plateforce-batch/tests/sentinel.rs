//! What the run treated as missing, and what it would have read had nobody said.
//!
//! A vendor export writes a missing sample as 0, -1 or 9999. Reading one as a real force is
//! the same fault as dropping a real one, so a run states which value means missing and
//! records both the statement and what it removed.

mod common;

use common::{bound_request_describing_the_plate, registry, tempdir};
use plateforce_batch::{analyse, SourceFormat, TrialIdentity, TrialSet};

const MISSING: f64 = 0.0;
const STANDING_NEWTONS: f64 = 700.0;
const SAMPLES: usize = 3600;

/// A trace that stands still, with one sample in sixty written as the missing marker.
fn trace_with_missing_samples(directory: &std::path::Path) -> usize {
    let mut written = 0;
    let body: String = (0..SAMPLES)
        .map(|sample| {
            if sample % 60 == 59 {
                written += 1;
                format!("{MISSING}\n")
            } else {
                format!("{}\n", STANDING_NEWTONS + ((sample % 7) as f64) * 0.05)
            }
        })
        .collect();
    std::fs::write(directory.join("standing.force.txt"), body).unwrap();
    written
}

fn format_declaring(sentinel: Option<f64>) -> SourceFormat {
    SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["force.txt".to_string()],
        sentinel,
    }
}

fn weight_of(
    directory: &std::path::Path,
    sentinel: Option<f64>,
) -> (f64, plateforce_batch::RunRow, Vec<String>) {
    let set = TrialSet::walk(
        directory,
        &format_declaring(sentinel),
        &TrialIdentity::FileStem,
    )
    .unwrap();
    // The plate is described so the two runs below have fingerprints to differ by. Left
    // unstated, both publish none and the guard compares two absences.
    let result = analyse(&set, &bound_request_describing_the_plate(), &registry())
        .expect("every choice was made");
    let weight = result.results[0]
        .values
        .get("system_weight_newtons")
        .and_then(|value| *value)
        .expect("a standing trace weighs");
    let messages = result
        .warnings
        .iter()
        .map(|row| row.message.clone())
        .collect();
    (weight, result.run.clone(), messages)
}

#[test]
fn a_declared_missing_value_is_not_read_as_force() {
    let directory = tempdir("sentinel-declared");
    let planted = trace_with_missing_samples(&directory);

    let (declared, run, warnings) = weight_of(&directory, Some(MISSING));
    let (undeclared, _, _) = weight_of(&directory, None);

    println!(
        "planted {planted} missing samples; weight declaring the marker {declared:.4} N, \
         reading it as force {undeclared:.4} N"
    );
    println!(
        "run records sentinel {:?}, dropped {}",
        run.sentinel, run.sentinel_rows_dropped
    );
    for message in &warnings {
        println!("warning: {message}");
    }

    assert_eq!(
        run.sentinel_rows_dropped, planted,
        "every planted marker was removed"
    );
    assert!(
        (declared - STANDING_NEWTONS).abs() < 1.0,
        "the declared run weighs the athlete, not the gaps: {declared}"
    );
    assert!(
        undeclared < declared - 5.0,
        "and reading the marker as force pulls the weight down, so the field does something: \
         {undeclared} against {declared}"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_run_records_which_convention_it_applied_and_what_it_removed() {
    let directory = tempdir("sentinel-recorded");
    let planted = trace_with_missing_samples(&directory);

    let (_, declared, warnings) = weight_of(&directory, Some(MISSING));
    assert_eq!(
        declared.sentinel.parse::<f64>().ok(),
        Some(MISSING),
        "the record names the value it read as missing: {:?}",
        declared.sentinel
    );
    assert_eq!(declared.sentinel_rows_dropped, planted);
    assert!(
        warnings
            .iter()
            .any(|line| line.contains(&planted.to_string())),
        "and the trial says how many it lost: {warnings:?}"
    );

    // Declaring none is a statement, so the record shows a run that removed nothing rather
    // than a run that was never asked.
    let (_, none_declared, quiet) = weight_of(&directory, None);
    assert_eq!(none_declared.sentinel, "");
    assert_eq!(none_declared.sentinel_rows_dropped, 0);
    assert!(
        !quiet
            .iter()
            .any(|line| line.contains("matched the declared")),
        "and nothing is reported missing, though a rule may still warn: {quiet:?}"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// Two runs that read one folder differently must not share a fingerprint, because the record
/// is what tells a reader whether two tables can be compared.
#[test]
fn reading_the_same_folder_two_ways_fingerprints_two_ways() {
    let directory = tempdir("sentinel-fingerprint");
    trace_with_missing_samples(&directory);

    let (_, declared, _) = weight_of(&directory, Some(MISSING));
    let (_, undeclared, _) = weight_of(&directory, None);
    println!(
        "declared {:?} against undeclared {:?}",
        declared.run_fingerprint, undeclared.run_fingerprint
    );
    // Both published one, so the inequality below is between two digests rather than between
    // two runs that withheld theirs.
    assert!(declared.run_fingerprint.is_some());
    assert!(undeclared.run_fingerprint.is_some());
    assert_ne!(declared.run_fingerprint, undeclared.run_fingerprint);
    std::fs::remove_dir_all(&directory).ok();
}
