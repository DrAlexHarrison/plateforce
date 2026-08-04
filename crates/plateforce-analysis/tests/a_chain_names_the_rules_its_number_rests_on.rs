//! A number names the rules it rests on, and no others.
//!
//! Every chain used to open with the same prefix: what conditioned the signal, the weighing
//! rule, every onset id and every takeoff id, whatever the number was. With no record of what
//! a rule read, naming everything was the only shape that could not omit a rule that had
//! contributed, and it named several that had not. Flight time is measured from takeoff to the
//! return to the plate and carried the weighing rule and six onset entries; the height taken
//! from it carried the same six.
//!
//! A chain naming rules that did not contribute and a chain omitting rules that did are one
//! defect, because both make the record say something untrue about how a number was produced,
//! and this tool's whole claim is that the record can be trusted.

use std::collections::BTreeMap;

use plateforce_analysis::binding::Dispatch;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// A countermovement jump that leaves the plate and lands back on it, so every landmark is
/// placed and no rule below declines for want of one.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 811));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn request(onset_id: &str, takeoff_id: &str) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: onset_id.into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: takeoff_id.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn chain<'a>(response: &'a AnalysisResponse, key: &str) -> &'a [String] {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .map(|metric| metric.contributing_method_ids.as_slice())
        .unwrap_or_else(|| panic!("{key} is absent, so there is no chain to read"))
}

/// Every rule the request can name for a landmark, read off the binding table rather than
/// listed, so a rule added under one of these constructs is covered without an edit here.
fn landmark_rules(slot: &str) -> Vec<&'static str> {
    plateforce_analysis::BINDINGS
        .iter()
        .filter(|binding| binding.slot == slot && matches!(binding.dispatch, Dispatch::Spine))
        .map(|binding| binding.id)
        .collect()
}

/// Flight time is bounded by takeoff and the return to the plate, so its chain names no
/// onset rule, on every onset rule this build runs.
///
/// Over every onset rule rather than one, because the rules differ in what they read and a
/// pass on one of them is not a statement about the others. `onset.threshold.last_within_band`
/// is the one that would have gone unnoticed: it searches back from a bound the takeoff rule
/// settles, so it is the only onset rule a takeoff-bounded number could plausibly have
/// reached, and it is the one an assertion written against the default rule never runs.
#[test]
fn flight_time_names_no_onset_rule_under_any_onset_rule() {
    let trial = a_jump_that_lands();
    let mut checked = 0usize;

    for onset_id in landmark_rules("onset") {
        let response = run(
            &trial,
            &request(onset_id, "takeoff.threshold.absolute_force"),
        )
        .expect("well formed");

        // The onset rule ran and left its mark on the result, or this pair proves nothing:
        // a chain naming no onset rule is trivially satisfied where no onset rule ran.
        let onset_ran = response
            .bound_methods
            .iter()
            .any(|row| row.method_id.starts_with("onset."));
        assert!(onset_ran, "{onset_id} left no record, so it did not run");

        for key in ["flight_time_seconds", "jump_height_from_flight_time_meters"] {
            let named: Vec<&String> = chain(&response, key)
                .iter()
                .filter(|id| id.starts_with("onset."))
                .collect();
            assert!(
                named.is_empty(),
                "{key} is measured from takeoff to the return to the plate and names {named:?} under {onset_id}"
            );
        }

        // And the other half of the same fact, so this is not passing by naming nothing at
        // all: the interval that is bounded by onset does name the onset rule.
        let interval: Vec<&String> = chain(&response, "time_to_takeoff_seconds")
            .iter()
            .filter(|id| id.starts_with("onset."))
            .collect();
        assert!(
            !interval.is_empty(),
            "time to takeoff is bounded by onset and names no onset rule under {onset_id}"
        );
        checked += 1;
    }

    println!("{checked} onset rules checked");
    assert!(checked >= 5, "only {checked} onset rules were reached");
}

/// A chain names the weighing rule when the rule that placed its landmark read the weighing
/// epoch, and does not when it did not.
///
/// Both directions, on the two rules that differ. `takeoff.threshold.absolute_force` floors
/// its search at the epoch's end and `takeoff.threshold.descending_crossing` floors at the
/// start of the recording and reads no epoch at all, so one names the weighing rule and one
/// must not. An assertion in one direction only is satisfied by a chain that names the
/// weighing rule always, which is the prefix this replaced.
#[test]
fn the_weighing_rule_is_named_by_the_landmarks_that_read_it_and_not_by_the_ones_that_did_not() {
    let trial = a_jump_that_lands();
    const WEIGHING: &str = "bwepoch.fixed_window";

    let reads_the_epoch = run(
        &trial,
        &request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.absolute_force",
        ),
    )
    .expect("well formed");
    assert!(
        chain(&reads_the_epoch, "takeoff_time_seconds").contains(&WEIGHING.to_string()),
        "the takeoff search floored at the weighing epoch's end and the chain does not name the rule that placed it: {:?}",
        chain(&reads_the_epoch, "takeoff_time_seconds")
    );

    let reads_no_epoch = run(
        &trial,
        &request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.descending_crossing",
        ),
    )
    .expect("well formed");
    assert!(
        !chain(&reads_no_epoch, "takeoff_time_seconds").contains(&WEIGHING.to_string()),
        "this takeoff rule reads no epoch and its chain names the weighing rule: {:?}",
        chain(&reads_no_epoch, "takeoff_time_seconds")
    );

    // The weighing rule still ran on both, so the difference above is the chain and not the
    // analysis: a run with no weighing rule would satisfy the second assertion for the wrong
    // reason.
    for response in [&reads_the_epoch, &reads_no_epoch] {
        assert!(
            response
                .bound_methods
                .iter()
                .any(|row| row.method_id == WEIGHING),
            "the weighing rule did not run, so the pair above is not a comparison"
        );
    }
}

/// Every rule the request can name for a landmark says which landmarks it reads.
///
/// The pipeline ends the analysis rather than assuming an empty list for a rule that has not
/// answered, because an empty list publishes a chain claiming the number rests on no landmark,
/// which reads as a finished answer and is not one. This is what catches a rule added with a
/// dispatch arm and no reading arm, in the suite rather than on a caller's trace.
#[test]
fn every_landmark_rule_says_which_landmarks_it_reads() {
    let trial = a_jump_that_lands();
    let mut checked = 0usize;

    for onset_id in landmark_rules("onset") {
        run(
            &trial,
            &request(onset_id, "takeoff.threshold.absolute_force"),
        )
        .unwrap_or_else(|error| panic!("{onset_id} could not be reached: {error}"));
        checked += 1;
    }
    for takeoff_id in landmark_rules("takeoff") {
        run(
            &trial,
            &request("onset.threshold.noise_relative", takeoff_id),
        )
        .unwrap_or_else(|error| panic!("{takeoff_id} could not be reached: {error}"));
        checked += 1;
    }

    println!("{checked} landmark rules ran and each said what it reads");
    // Five onset rules and five takeoff rules. A count below this is a subject that shrank,
    // and a guard reading fewer rules than the build ships passes by looking at less.
    assert!(checked >= 10, "only {checked} landmark rules were reached");
}
