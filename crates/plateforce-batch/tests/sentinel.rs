//! What the run was told means missing, what it found, and what it did about it.
//!
//! A vendor export writes a missing sample as 0, -1 or 9999. A run states which value means
//! missing, counts what matched, and leaves the trace exactly as the file wrote it.
//!
//! Leaving it alone is the policy every surface applies, and this reader used to be the one
//! that did not. It removed the matches, which closes the gap and shifts every timestamp after
//! it. On `subject01_trial1` the zero convention matches the whole flight phase, so declaring
//! it deleted 157 samples of flight and moved jump height from flight time by 17.13 cm, from
//! 0.44022460156250015 m to 0.2689609062500001 m, under a warning saying the samples were not
//! read as force. `plateforce_core::signal::trial_from_column` is the one home now, and
//! `crates/plateforce-wasm/tests/no_reader_repairs_the_recording_it_was_handed.rs` holds this
//! reader to it on the recording that measurement came from.
//!
//! So what a declaration does here is report. It never moves a number, which is the invariant
//! the parity gate's `sentinel` row states for the other four surfaces.

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

/// Declaring a convention reports what matched and moves no number.
///
/// The two runs read one file and differ only in what the caller said about it, so a weight
/// that moved between them would be the declaration deciding a number. That is what the
/// removing reader did, and on a jump trace the samples it removed were the flight.
#[test]
fn declaring_a_missing_value_reports_it_and_moves_no_number() {
    let directory = tempdir("sentinel-declared");
    let planted = trace_with_missing_samples(&directory);

    let (declared, run, warnings) = weight_of(&directory, Some(MISSING));
    let (undeclared, quiet_run, _) = weight_of(&directory, None);

    println!(
        "planted {planted} missing samples; weight declaring the marker {declared:.6} N, \
         declaring none {undeclared:.6} N"
    );
    println!(
        "run records sentinel {:?}, matching {} and carrying no number {}",
        run.sentinel, run.samples_matching_the_convention, run.samples_carrying_no_number
    );
    for message in &warnings {
        println!("warning: {message}");
    }

    // The convention matches something, or the equality below holds for the reason that
    // nothing was declared rather than for the reason this test is about.
    assert_eq!(
        run.samples_matching_the_convention, planted,
        "every planted marker was counted"
    );
    assert_eq!(quiet_run.samples_matching_the_convention, 0);
    assert_eq!(
        declared, undeclared,
        "declaring the convention moved the weight from {undeclared} to {declared}"
    );
}

/// The run record carries the two reasons apart, and says nothing was removed.
#[test]
fn the_run_records_which_convention_it_applied_and_what_matched() {
    let directory = tempdir("sentinel-recorded");
    let planted = trace_with_missing_samples(&directory);

    let (_, declared, warnings) = weight_of(&directory, Some(MISSING));
    assert_eq!(
        declared.sentinel.parse::<f64>().ok(),
        Some(MISSING),
        "the record names the value it read as missing: {:?}",
        declared.sentinel
    );
    assert_eq!(declared.samples_matching_the_convention, planted);
    // Nothing in this trace carries no number, so the second count is the one that separates
    // a caller's declaration from a gap in the recording and it reads zero here.
    assert_eq!(declared.samples_carrying_no_number, 0);
    assert!(
        warnings
            .iter()
            .any(|line| line.contains(&planted.to_string())),
        "and the trial says how many matched: {warnings:?}"
    );

    // Declaring none is a statement, so the record shows a run that matched nothing rather
    // than a run that was never asked.
    let (_, none_declared, quiet) = weight_of(&directory, None);
    assert_eq!(none_declared.sentinel, "");
    assert_eq!(none_declared.samples_matching_the_convention, 0);
    assert_eq!(none_declared.samples_carrying_no_number, 0);
    assert!(
        !quiet
            .iter()
            .any(|line| line.contains("match the declared")),
        "and nothing is reported missing, though a rule may still warn: {quiet:?}"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// The trace the run analysed is the trace the file wrote, sample for sample.
///
/// The property the removed samples used to break. Held on the time base rather than on the
/// count, because a reader that removed 60 samples reported 60 correctly and analysed a
/// recording 50 ms shorter than the one on disk.
#[test]
fn declaring_a_convention_leaves_the_time_base_alone() {
    let directory = tempdir("sentinel-time-base");
    trace_with_missing_samples(&directory);

    for declared in [None, Some(MISSING)] {
        let format = format_declaring(declared);
        let set = TrialSet::walk(&directory, &format, &TrialIdentity::FileStem).unwrap();
        let (trial, _report, reported) = set
            .iter()
            .next()
            .expect("one trial was walked")
            .1
            .source
            .read(&format)
            .expect("the trace reads");
        assert_eq!(
            trial.len(),
            SAMPLES,
            "declaring {declared:?} changed how many samples the run analysed"
        );
        assert_eq!(trial.duration_seconds(), SAMPLES as f64 / 1200.0);
        assert_eq!(
            reported.matched_the_convention,
            declared.map_or(0, |_| SAMPLES / 60)
        );
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// Two runs that read one folder differently must not share a fingerprint, because the record
/// is what tells a reader whether two tables can be compared.
///
/// It matters more now than it did, not less: the declaration no longer moves a number, so the
/// record is the only place the two runs differ at all.
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
