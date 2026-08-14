//! The record a reader pastes names the rate the analysis ran at, and it is the rate the
//! request stated.
//!
//! Every velocity, displacement, impulse and rate of force development is that number times a
//! count of samples, so it moves every one of them. The same trace declared at 1000 Hz where
//! the plate recorded 1200 answers a jump height a fifth of the way off, and nothing in the
//! trace says which declaration is right. A record naming the registry down to its content
//! digest and not naming this is a record a reader cannot repeat the analysis from.
//!
//! Broken by changing the stated rate rather than by deleting the line. A test that removes
//! the line and watches it disappear proves the line exists; a test that states two rates and
//! requires the printed one to follow fails on a surface printing a constant, and on one
//! reading a field that is not the rate.

use std::collections::BTreeMap;

use plateforce_analysis::document::{ResultDocument, TrialSource};
use plateforce_analysis::{
    markdown, recorded_number_text, run, AnalysisRequest, MethodChoice, WeighingChoice,
};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::{read_trial_from_path, Trial};

mod common;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

/// The rate the plate recorded at, and one a reader might declare by mistake. Both are rates a
/// caller can state, and the recording cannot tell them apart.
const RECORDED_HZ: f64 = 1200.0;
const MISDECLARED_HZ: f64 = 1000.0;

fn trial_declared_at(sample_rate_hz: f64) -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, sample_rate_hz)
        .unwrap_or_else(|error| panic!("{FIXTURE} did not read: {error}"));
    trial
}

fn request() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    })
}

/// The document a surface hands a reader, for a trace declared at one rate.
///
/// The rate travels on the trial block rather than being restated here, so what the record
/// prints is what the caller declared the recording at and not a number this test wrote.
fn record_at(sample_rate_hz: f64) -> (String, Option<f64>) {
    let trial = trial_declared_at(sample_rate_hz);
    let response = run(&trial, &request()).expect("the request is well formed");
    let height = response
        .metric("jump_height_from_takeoff_meters")
        .and_then(|metric| metric.value);
    let document = ResultDocument::of(
        "0.1.0",
        TrialSource {
            name: "subject01_trial1".into(),
            rows_read: trial.len(),
            samples_matching_the_convention: 0,
            sample_rate_hz,
        },
        &RegistryStamp {
            version: Some("fixture-pin".to_string()),
            declared_version: Some("fixture-declares".to_string()),
            digest: Some("content-fixture".to_string()),
        },
        &plateforce_core::Capture::default(),
        &response,
        None,
    );
    (markdown::result(&document), height)
}

/// The rate the record prints is the rate the recording was declared at.
///
/// Two declarations of one trace, so a record printing a constant and a record reading some
/// other field both fail. The line is read off the record and compared against the number the
/// caller stated, spelled the way every other number in this product is spelled.
#[test]
fn the_pasted_record_names_the_rate_the_caller_declared() {
    for stated in [RECORDED_HZ, MISDECLARED_HZ] {
        let (record, _) = record_at(stated);
        let line = record
            .lines()
            .find(|line| line.starts_with("sampled at"))
            .unwrap_or_else(|| {
                panic!(
                    "the record names no rate, so a reader cannot repeat the analysis from it:\n{}",
                    record.lines().take(6).collect::<Vec<&str>>().join("\n")
                )
            });
        println!("{line}");
        assert_eq!(
            line,
            format!("sampled at {} Hz", recorded_number_text(stated)),
            "the record names a rate the caller did not state"
        );
    }
}

/// And the rate is load-bearing, which is why the record has to carry it.
///
/// The control on the guard above. A rate the numbers did not depend on would be a line worth
/// nothing to a reader repeating the analysis, and a test asserting the line follows the
/// declaration would be asserting the faithful printing of a decoration. One trace, two
/// declarations, two different heights.
#[test]
fn the_rate_the_record_names_is_one_every_number_in_it_rests_on() {
    let (_, recorded) = record_at(RECORDED_HZ);
    let (_, misdeclared) = record_at(MISDECLARED_HZ);
    let recorded = recorded.expect("the trial answers a height at the rate it was recorded at");
    let misdeclared = misdeclared.expect("the trial answers a height at the declared rate");
    println!("{recorded} m declared at {RECORDED_HZ} Hz, {misdeclared} m declared at {MISDECLARED_HZ} Hz");
    assert!(
        (recorded - misdeclared).abs() > 0.01,
        "the same trace answers {recorded} m and {misdeclared} m under two declared rates, which \
         is close enough that the rate is not moving the number this test says it moves"
    );
}
