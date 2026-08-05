//! Five of the eleven numbers this request reports move when a stated gravity moves, and four
//! when one nobody stated moves. The difference is the one entry that declares a gravity of its
//! own, and it is the whole of the difference, which the third test below holds.
//!
//! Both counts are printed by the tests that take them rather than trusted from here. The
//! request binds the spine and nothing else, so eleven is this request's population and not the
//! build's: `the_chain_behind_a_number_names_the_gravity_that_moved_it.rs` runs one rule for
//! every construct instead and reaches twenty-one, which is where a sixth number resting on the
//! gravity was found.
//!
//! Rules read the analysis gravity and record nothing about it: their registry entries declare
//! no such parameter and a rule may not record one its entry does not carry. So the value
//! belongs to the analysis rather than to any rule, and the analysis is where the record carries
//! it.
//!
//! Every set below is computed by moving the gravity and reading which numbers followed.
//! A list of four keys written here would go stale the day a fifth arrived, and would pass
//! while doing it.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, BODY_MASS_GLOBAL,
    GRAVITY_GLOBAL, TOUCHDOWN_GLOBAL,
};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::Trial;

const SAMPLE_RATE_HZ: f64 = 1200.0;
const FLIGHT_SAMPLES: usize = 811;
const FLIGHT_TIME_HEIGHT: &str = "jump_height_from_flight_time_meters";

/// The two constants the tools argue over, which is the smallest disagreement any of this has
/// to survive. Gravity varies by half a percent across the Earth's surface, fifteen times
/// this gap, so a guard that passes here passes on anything a plate owner would state.
const STANDARD: f64 = 9.80665;
const PUBLISHED: f64 = 9.81;

/// A countermovement jump that leaves the plate and lands back on it, so every rule has the
/// three landmarks and the return it needs and none declines for want of one. Without the
/// landing there is no flight time, and the one rule this file turns on would decline rather
/// than answering with the gravity its entry declares.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, FLIGHT_SAMPLES));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn base() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// An analysis at one gravity under one claim. The claim is set directly rather than through
/// `state_gravity`, which writes the value and the claim together, because these guards need
/// to write the two apart.
fn at(gravity: f64, source: ParameterSource) -> AnalysisResponse {
    let mut request = base();
    request.gravity_meters_per_second_squared = gravity;
    request.gravity_source = source;
    run(&a_jump_that_lands(), &request).expect("the trace carries every landmark")
}

fn values(response: &AnalysisResponse) -> BTreeMap<String, Option<f64>> {
    response
        .metrics
        .iter()
        .map(|metric| (metric.key.clone(), metric.value))
        .collect()
}

/// The keys whose number is not the same at the two gravities. Computed, never listed.
fn moved_between(one: &AnalysisResponse, other: &AnalysisResponse) -> BTreeSet<String> {
    let (before, after) = (values(one), values(other));
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the two analyses reported different quantities, so nothing below compares numbers"
    );
    before
        .iter()
        .filter(|(key, value)| after[*key] != **value)
        .map(|(key, _)| key.clone())
        .collect()
}

fn gravity_on_the_record(response: &AnalysisResponse) -> &plateforce_analysis::BoundGlobal {
    response
        .bound_globals
        .iter()
        .find(|bound| bound.name == GRAVITY_GLOBAL)
        .expect("every result names the gravity it ran under")
}

/// A number that moves with the analysis gravity runs in an analysis whose record names that
/// gravity.
///
/// The moving set is measured rather than written down, and the guard first requires it to be
/// non-empty. Without that control a build where gravity moved nothing at all would report
/// every assertion below as satisfied while proving none of them.
#[test]
fn every_number_the_analysis_gravity_moves_runs_in_an_analysis_whose_record_names_that_gravity() {
    let standard = at(STANDARD, ParameterSource::Stated);
    let published = at(PUBLISHED, ParameterSource::Stated);

    let moved = moved_between(&standard, &published);
    println!(
        "{} of {} numbers moved: {moved:?}",
        moved.len(),
        standard.metrics.len()
    );
    assert!(
        !moved.is_empty(),
        "no number moved between {STANDARD} and {PUBLISHED}, so nothing here is being tested"
    );

    for (response, requested) in [(&standard, STANDARD), (&published, PUBLISHED)] {
        let recorded = gravity_on_the_record(response);
        assert_eq!(
            recorded.value, requested,
            "the record names {} where the numbers ran at {requested}",
            recorded.value
        );
        assert_eq!(recorded.source, ParameterSource::Stated);
        assert_eq!(recorded.unit, "meters_per_second_squared");
    }
}

/// A value somebody chose against the same value nobody chose.
///
/// Two analyses at the identical gravity, differing only in whether anybody chose it. Ten of
/// the eleven numbers are the same, and for those ten the claim is visible nowhere but in the
/// record.
///
/// The eleventh is the flight-time height, and it differs on purpose: a reader who measured
/// 9.80665 at their own plate gets their measurement, where a reader who stated nothing gets
/// the 9.81 that entry publishes. Stating the standard value is an act, and this is where the
/// act is worth something.
#[test]
fn one_gravity_under_two_claims_gives_two_records_and_moves_only_the_rule_that_reads_the_claim() {
    let chosen = at(STANDARD, ParameterSource::Stated);
    let filled_in = at(STANDARD, ParameterSource::Assumed);

    assert_eq!(
        moved_between(&chosen, &filled_in),
        BTreeSet::from([FLIGHT_TIME_HEIGHT.to_string()]),
        "one gravity under two claims moved a number no rule reads the claim in"
    );
    assert_eq!(gravity_on_the_record(&chosen).value, STANDARD);
    assert_eq!(gravity_on_the_record(&filled_in).value, STANDARD);
    assert_eq!(
        gravity_on_the_record(&chosen).source,
        ParameterSource::Stated
    );
    assert_eq!(
        gravity_on_the_record(&filled_in).source,
        ParameterSource::Assumed
    );
}

/// The resolution order, held by measurement rather than by reading the code.
///
/// A rule whose entry publishes its own gravity takes that gravity while nobody has chosen
/// one, and takes the caller's the moment somebody has. So the set of numbers a moving gravity
/// moves is strictly larger under a stated claim than under a filled-in one, and the
/// difference is exactly the numbers those rules produce.
///
/// Both sets are measured. The one name written down is the key the difference is expected to
/// contain, and it is asserted as the whole difference rather than as a member of it, so a
/// second rule publishing its own gravity fails this rather than slipping in beside the first.
#[test]
fn a_gravity_nobody_chose_leaves_exactly_the_numbers_whose_rules_publish_their_own() {
    let moved_when_stated = moved_between(
        &at(STANDARD, ParameterSource::Stated),
        &at(PUBLISHED, ParameterSource::Stated),
    );
    let moved_when_assumed = moved_between(
        &at(STANDARD, ParameterSource::Assumed),
        &at(PUBLISHED, ParameterSource::Assumed),
    );
    println!("stated moved {moved_when_stated:?}\nassumed moved {moved_when_assumed:?}");

    assert!(
        !moved_when_assumed.is_empty(),
        "a gravity nobody chose moved nothing, so the two sets below are not being compared"
    );
    assert!(
        moved_when_assumed.is_subset(&moved_when_stated),
        "a number moved under a filled-in gravity and held still under a chosen one"
    );

    let only_when_stated: BTreeSet<String> = moved_when_stated
        .difference(&moved_when_assumed)
        .cloned()
        .collect();
    assert_eq!(
        only_when_stated,
        BTreeSet::from([FLIGHT_TIME_HEIGHT.to_string()]),
        "the numbers a chosen gravity reaches and a filled-in one does not"
    );
}

/// The population, which is every value the request binds that no rule's row can carry.
///
/// More than one, on purpose. A record holding one row proves nothing about the shape that
/// holds the next one, and the next one is already here: a body mass three level-one entries
/// divide by, and a touchdown a reader placed by hand which no rule owns.
#[test]
fn the_record_names_every_value_the_request_binds_and_no_rule_records() {
    let quiet = run(&a_jump_that_lands(), &base()).expect("the trace carries every landmark");
    let named: Vec<&str> = quiet.bound_globals.iter().map(|bound| bound.name).collect();
    assert_eq!(
        named,
        vec![GRAVITY_GLOBAL],
        "a request stating none of these still ran at a gravity, and says which"
    );
    assert_eq!(
        quiet.bound_globals[0].source,
        ParameterSource::Assumed,
        "nobody was asked, and the record says so rather than reading as a choice"
    );

    let mut stating = base();
    stating.state_gravity(Some(PUBLISHED));
    stating.body_mass_kilograms = Some(61.5);
    stating.touchdown_index = Some(3000);
    let spoken = run(&a_jump_that_lands(), &stating).expect("the trace carries every landmark");

    let recorded: BTreeMap<&str, (f64, ParameterSource)> = spoken
        .bound_globals
        .iter()
        .map(|bound| (bound.name, (bound.value, bound.source)))
        .collect();
    println!("{recorded:?}");
    assert_eq!(
        recorded.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([GRAVITY_GLOBAL, BODY_MASS_GLOBAL, TOUCHDOWN_GLOBAL])
    );
    assert_eq!(
        recorded[GRAVITY_GLOBAL],
        (PUBLISHED, ParameterSource::Stated)
    );
    assert_eq!(recorded[BODY_MASS_GLOBAL], (61.5, ParameterSource::Stated));
    assert_eq!(
        recorded[TOUCHDOWN_GLOBAL],
        (3000.0, ParameterSource::Stated)
    );
    assert_eq!(spoken.touchdown_index, Some(3000));
}

/// The one routine every surface writes a gravity through, held to the two claims it exists
/// to keep apart. A caller that states the constant itself has still stated it.
#[test]
fn stating_the_standard_value_is_an_act_and_stating_nothing_is_not() {
    assert_eq!(
        plateforce_analysis::gravity_stated(Some(STANDARD)),
        (STANDARD, ParameterSource::Stated)
    );
    assert_eq!(
        plateforce_analysis::gravity_stated(None),
        (STANDARD, ParameterSource::Assumed)
    );
}
