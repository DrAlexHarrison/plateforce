//! A reader who opens the folder is told what every trial in it was analysed under.
//!
//! A folder run states its gravity and its athlete's mass once, as it states the plate once,
//! and those values belong to the run rather than to any rule's row. Before this the run
//! record carried them only inside `request_digest`, so two runs under different masses were
//! visibly different documents and a reader could not recover either mass from either one.
//!
//! The population is more than one on purpose: a record holding a single row proves nothing
//! about the shape that holds the next one.

mod common;

use plateforce_batch::{analyse, TrialIdentity, TrialSet};

use common::{
    analysis_request, bound_request, committed_format, registry, tempdir, FIXTURES,
};

const TRIAL_FILE: &str = "subject01_trial1.force.txt";
const STATED_MASS_KILOGRAMS: f64 = 61.5;
const STATED_GRAVITY: f64 = 9.70;
const STANDARD_GRAVITY: f64 = 9.80665;

/// One folder holding one committed trace, run under whatever the caller bound.
fn run_bound(
    body_mass_kilograms: Option<f64>,
    gravity: Option<f64>,
) -> plateforce_batch::BatchResult {
    let directory = tempdir("bound-globals");
    std::fs::copy(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        directory.join(TRIAL_FILE),
    )
    .expect("the fixture copies");

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem)
        .expect("the folder walks");
    let mut request = bound_request();
    request.analysis = analysis_request(1.0);
    request.analysis.body_mass_kilograms = body_mass_kilograms;
    if let Some(gravity) = gravity {
        request.analysis.state_gravity(Some(gravity));
    }
    analyse(&set, &request, &registry()).expect("the run produces a result")
}

/// Every value the run was bound to, against the name the record reports it by.
fn named(
    result: &plateforce_batch::BatchResult,
) -> std::collections::BTreeMap<String, (f64, String)> {
    result
        .run
        .bound_globals
        .iter()
        .map(|bound| {
            (
                bound.name.clone(),
                (bound.value, bound.source.wire_name().to_string()),
            )
        })
        .collect()
}

/// A mass and a gravity the operator stated reach the run record under the operator's own
/// claim, and a run that states neither still says what gravity it ran at and that nobody
/// chose it.
#[test]
fn the_run_record_names_every_value_the_folder_was_bound_to_and_who_chose_it() {
    let spoken = named(&run_bound(Some(STATED_MASS_KILOGRAMS), Some(STATED_GRAVITY)));
    println!("{spoken:?}");
    assert_eq!(
        spoken["body_mass_kilograms"],
        (STATED_MASS_KILOGRAMS, "stated".to_string())
    );
    assert_eq!(
        spoken["gravity_meters_per_second_squared"],
        (STATED_GRAVITY, "stated".to_string())
    );

    let quiet = named(&run_bound(None, None));
    assert_eq!(
        quiet["gravity_meters_per_second_squared"],
        (STANDARD_GRAVITY, "assumed".to_string()),
        "a folder nobody stated a gravity for still ran at one, and says nobody chose it"
    );
    assert!(
        !quiet.contains_key("body_mass_kilograms"),
        "a run nobody stated a mass for carried a row for one: {quiet:?}"
    );
}

/// The record is read back off disk rather than off the value in memory, because the run row
/// is what a reader opening the folder holds and a field that serialises and does not
/// deserialise reads as an absent value rather than as a broken one.
#[test]
fn the_bound_values_survive_the_round_trip_through_the_written_record() {
    let result = run_bound(Some(STATED_MASS_KILOGRAMS), Some(STATED_GRAVITY));
    let written = serde_json::to_string(&result.run).expect("the run row writes");
    let read: plateforce_batch::RunRow =
        serde_json::from_str(&written).expect("the run row reads back");
    assert_eq!(read.bound_globals, result.run.bound_globals);
    assert_eq!(read.bound_globals.len(), 2, "{:?}", read.bound_globals);
}

/// A run under one mass and a run under another are two results, so the identity the record
/// publishes has to separate them. The digest is what a reader compares two folders by.
#[test]
fn two_folders_bound_to_different_masses_do_not_share_one_request_digest() {
    let lighter = run_bound(Some(52.0), None);
    let heavier = run_bound(Some(87.5), None);
    let unstated = run_bound(None, None);

    assert_ne!(
        lighter.run.request_digest, heavier.run.request_digest,
        "two masses digested alike, so the record calls two results one"
    );
    assert_ne!(
        lighter.run.request_digest, unstated.run.request_digest,
        "a stated mass and none digested alike"
    );
}
