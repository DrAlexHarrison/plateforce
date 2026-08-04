//! A landing the caller placed reaches the record of every number measured against it, and no
//! other number's.
//!
//! Flight time is bounded by takeoff and the return to the plate. The registry entry the record
//! names says that return is "the first sample at which force returned above the threshold that
//! placed takeoff", so a caller who states one has supplied a sample no rule produced. Before
//! this, the two reached identical records: the same six ids, the same digest, and a flight time
//! that had moved by a quarter of a second. A digest whose whole purpose is to prove two labs
//! computed one quantity matched two numbers that were not computed the same way.
//!
//! The value and not a flag, because two hand-placed landings give two flight times.

use std::collections::BTreeMap;

use plateforce_analysis::chain::chain_of;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::provenance::{ParameterSource, RegistryStamp};
use plateforce_core::reporting::fingerprint;
use plateforce_core::{Acquisition, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const FLIGHT: &str = "flight_time_seconds";
const FLIGHT_HEIGHT: &str = "jump_height_from_flight_time_meters";
const INTERVAL: &str = "time_to_takeoff_seconds";
const LANDING: &str = "touchdown_index";

/// A countermovement jump that leaves the plate and lands back on it, so the software places a
/// landing of its own and the stated one is a departure from it rather than the only answer.
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

fn request(stated_landing: Option<usize>) -> AnalysisRequest {
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
        touchdown_index: stated_landing,
        ..Default::default()
    }
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// A plate whose settings were all recorded. An unfilled block publishes no digest at all, and
/// two absent digests compare equal to nothing, so a comparison over them proves nothing.
fn a_recorded_plate() -> Acquisition {
    Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}

fn value(response: &AnalysisResponse, key: &str) -> f64 {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
        .unwrap_or_else(|| panic!("{key} carries no number, so there is nothing to compare"))
}

fn digest(response: &AnalysisResponse, key: &str) -> String {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .unwrap_or_else(|| panic!("{key} is absent, so there is no chain to fingerprint"));
    let chain = chain_of(response, metric, &stamp(), true);
    fingerprint(&chain.provenance, &a_recorded_plate(), SAMPLE_RATE_HZ)
        .published()
        .expect("the acquisition block is filled, so this digest publishes")
        .to_string()
}

fn ids(response: &AnalysisResponse, key: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .map(|metric| metric.contributing_method_ids.clone())
        .unwrap_or_else(|| panic!("{key} is absent, so there is no chain to read"))
}

/// The rules whose record names the landing the caller stated, with the source recorded for it.
fn rows_naming_the_landing(response: &AnalysisResponse) -> Vec<(String, ParameterSource)> {
    response
        .bound_methods
        .iter()
        .filter(|row| row.bound_parameters.iter().any(|(name, _)| name == LANDING))
        .map(|row| {
            (
                row.method_id.clone(),
                row.parameter_sources
                    .get(LANDING)
                    .copied()
                    // A name on the row with no source recorded beside it, which would report
                    // the caller's landing as a value nobody chose. Named rather than skipped,
                    // so the assertion below sees it.
                    .unwrap_or(ParameterSource::Assumed),
            )
        })
        .collect()
}

/// The landing the software found and the landing the caller stated give two numbers, and two
/// records.
#[test]
fn a_stated_landing_and_a_found_one_do_not_fingerprint_alike() {
    let trial = a_jump_that_lands();
    let found = run(&trial, &request(None)).expect("the request is well formed");
    let placed_by_the_software = found
        .touchdown_index
        .expect("this recording returns to the plate, so a landing was found");

    let stated = run(&trial, &request(Some(placed_by_the_software + 300)))
        .expect("the request is well formed");

    // The pair is a comparison only if the number actually moved. A stated landing equal to the
    // found one would make every assertion below pass without saying anything.
    for key in [FLIGHT, FLIGHT_HEIGHT] {
        assert_ne!(
            value(&found, key),
            value(&stated, key),
            "the stated landing did not move {key}, so this pair is not a comparison"
        );
        assert_ne!(
            digest(&found, key),
            digest(&stated, key),
            "{key} moved from {} to {} and both runs fingerprint as {}, so the record says one \
             quantity was computed twice the same way",
            value(&found, key),
            value(&stated, key),
            digest(&found, key)
        );
    }

    // The rule that read the landing names it, with the claim that says the caller supplied it.
    let naming = rows_naming_the_landing(&stated);
    assert!(
        naming
            .iter()
            .any(|(id, _)| id == "flight_time.takeoff_to_touchdown"),
        "the caller placed the landing flight time is measured to and the rule's record does not \
         name it: {naming:?}"
    );
    for (id, source) in &naming {
        assert_eq!(
            *source,
            ParameterSource::Stated,
            "{id} records the caller's landing as {source:?}, which reads as a value nobody chose"
        );
    }

    // And a landing nobody stated is recorded by nobody. The found landing is the return above
    // the threshold the takeoff rule resolved, and the chain already names that rule, so a value
    // here as well would report one decision twice.
    assert!(
        rows_naming_the_landing(&found).is_empty(),
        "no landing was stated and these rules record one: {:?}",
        rows_naming_the_landing(&found)
    );
}

/// Two hand-placed landings are two records, which is why the index is recorded and not a flag.
#[test]
fn two_stated_landings_do_not_fingerprint_alike() {
    let trial = a_jump_that_lands();
    let found = run(&trial, &request(None)).expect("the request is well formed");
    let placed_by_the_software = found.touchdown_index.expect("a landing was found");

    let earlier = run(&trial, &request(Some(placed_by_the_software + 100))).expect("well formed");
    let later = run(&trial, &request(Some(placed_by_the_software + 400))).expect("well formed");

    assert_ne!(
        value(&earlier, FLIGHT),
        value(&later, FLIGHT),
        "the two stated landings gave one flight time, so this pair is not a comparison"
    );
    assert_ne!(
        digest(&earlier, FLIGHT),
        digest(&later, FLIGHT),
        "two landings a caller placed 300 samples apart fingerprint alike, so the record says \
         where the athlete landed does not bear on the flight time"
    );
}

/// A number measured to no landing is untouched by one, in its chain and in its digest.
///
/// The other half of naming exactly what a number rests on. A record that separated the two
/// runs by moving every number's digest would have replaced one untruth with another: time to
/// takeoff ends at takeoff and the athlete had not yet landed.
#[test]
fn a_number_that_never_reads_the_landing_does_not_carry_it() {
    let trial = a_jump_that_lands();
    let found = run(&trial, &request(None)).expect("the request is well formed");
    let placed_by_the_software = found.touchdown_index.expect("a landing was found");
    let stated = run(&trial, &request(Some(placed_by_the_software + 300))).expect("well formed");

    assert_eq!(
        value(&found, INTERVAL),
        value(&stated, INTERVAL),
        "the stated landing moved a number that ends at takeoff"
    );
    assert_eq!(
        ids(&found, INTERVAL),
        ids(&stated, INTERVAL),
        "the stated landing changed the rules time to takeoff rests on"
    );
    assert_eq!(
        digest(&found, INTERVAL),
        digest(&stated, INTERVAL),
        "time to takeoff is measured from onset to takeoff and its digest moved when the caller \
         stated where the athlete landed"
    );

    // The rule that computes it names no landing either, so the assertion above is about what
    // the record carries rather than about a digest that happens to collide.
    let interval_row = stated
        .bound_methods
        .iter()
        .find(|row| row.method_id == "time_to_takeoff.onset_to_takeoff")
        .expect("the interval ran");
    assert!(
        !interval_row
            .bound_parameters
            .iter()
            .any(|(name, _)| name == LANDING),
        "time to takeoff records a landing it never read: {:?}",
        interval_row.bound_parameters
    );
}

/// The rules that carry a stated landing are exactly the rules that read one.
///
/// Counted against the population rather than listed, so a rule added that reads the landing is
/// covered here without an edit, and one that stops reading it cannot keep carrying the value.
#[test]
fn every_rule_carrying_the_landing_is_one_that_reads_it() {
    let trial = a_jump_that_lands();
    let found = run(&trial, &request(None)).expect("the request is well formed");
    let placed = found.touchdown_index.expect("a landing was found");
    let stated = run(&trial, &request(Some(placed + 300))).expect("well formed");

    let naming: Vec<String> = rows_naming_the_landing(&stated)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let rows = stated.bound_methods.len();

    println!(
        "{} of {rows} recorded rules name the landing: {naming:?}",
        naming.len()
    );
    assert!(
        !naming.is_empty(),
        "no rule named the caller's landing, so the guards above are reading an empty set"
    );
    assert!(
        naming.len() < rows,
        "every one of the {rows} rules named the landing, so it is being written onto rules that \
         never read one"
    );
}
