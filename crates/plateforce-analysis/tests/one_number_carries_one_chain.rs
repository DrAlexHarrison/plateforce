//! The tree a response describes is derived in one place, and it loses nothing the response
//! said.
//!
//! Four surfaces read this tree, and a surface that rebuilds it for itself can drop the
//! arithmetic rule's own bound values: the gravity behind the flight-time height and the four
//! integration choices behind every impulse figure would reach a folder run's record and no
//! notebook's, no R session's and no account a reader is shown. A number that moves when a
//! value moves, with nothing in the record naming the value, is a number nobody can reproduce.

use std::collections::BTreeMap;

use plateforce_analysis::chain::{chain_names, chain_of, chains_of, metrics_resting_on};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::reporting::fingerprint;
use plateforce_core::{Acquisition, ProvenanceChain, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// A countermovement jump that leaves the plate and lands back on it, so every landmark is
/// placed and every quantity below reports a number.
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

fn request_with_onset_k(k: f64) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), k)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
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

/// A plate whose settings were all recorded, so a fingerprint taken over it is one that
/// publishes. An unfilled block matches nothing, itself included, so a comparison over two
/// incomplete fingerprints passes whatever the digests are.
fn a_recorded_plate() -> Acquisition {
    Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}

fn analysed(k: f64) -> AnalysisResponse {
    run(&a_jump_that_lands(), &request_with_onset_k(k)).expect("the request is well formed")
}

fn chain_for(response: &AnalysisResponse, key: &str) -> ProvenanceChain {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .unwrap_or_else(|| panic!("{key} is not reported, so there is no chain to read"));
    chain_of(response, metric, &stamp(), true)
}

/// The rule named in `computed_by` roots the chain and carries the values it read.
///
/// `jumpheight.takeoff.flight_time` publishes a gravity of its own, 9.81, which is not the
/// 9.80665 the request carries. A root carrying no parameters would report a number produced
/// by a value that appears nowhere in the record beside it.
#[test]
fn the_arithmetic_roots_the_chain_and_carries_the_values_it_read() {
    let response = analysed(5.0);
    let chain = chain_for(&response, "jump_height_from_flight_time_meters");

    assert_eq!(chain.provenance.method_id, "jumpheight.takeoff.flight_time");
    let gravity = chain
        .provenance
        .parameters
        .iter()
        .find(|parameter| parameter.name == "gravity")
        .expect("the rule that computed this height read a gravity and the root does not name it");
    assert_eq!(gravity.value, 9.81);

    // And the other half, so this cannot pass by giving every root every value: the request's
    // own gravity is not what this rule ran at, and the root must not claim it was.
    assert_ne!(
        gravity.value, response.bound_globals[0].value,
        "the root reports the request's gravity rather than the one its rule published"
    );
}

/// The four integration choices behind an impulse figure reach the record, as the choices the
/// arithmetic rule recorded.
#[test]
fn the_integration_choices_behind_an_impulse_reach_the_record() {
    let response = analysed(5.0);
    let chain = chain_for(&response, "net_impulse_newton_seconds");

    let named: Vec<&str> = chain
        .provenance
        .choices
        .iter()
        .map(|choice| choice.name.as_str())
        .collect();
    for expected in [
        "integration_rule",
        "integration_direction",
        "integration_start",
        "integration_anchor",
    ] {
        assert!(
            named.contains(&expected),
            "the impulse rule read {expected} and the record does not name it: {named:?}"
        );
    }
}

/// A quantity no rule computed is rooted at the rule whose answer it is, not at a step naming
/// nothing.
///
/// Both directions on the two that differ: the system weight is the weighing rule's own answer
/// and movement onset is the onset rule's, so a root chosen by position in a list rather than
/// by the order rules run would put one of them under the other.
#[test]
fn a_quantity_no_rule_computed_is_rooted_at_the_rule_that_produced_it() {
    let response = analysed(5.0);

    let weight = chain_for(&response, "system_weight_newtons");
    assert_eq!(weight.provenance.method_id, "bwepoch.fixed_window");
    assert!(
        weight
            .provenance
            .parameters
            .iter()
            .any(|parameter| parameter.name == "duration"),
        "the weighing rule's own window is not on the root: {:?}",
        weight.provenance.parameters
    );

    let onset = chain_for(&response, "onset_time_seconds");
    assert_eq!(onset.provenance.method_id, "onset.threshold.noise_relative");

    let takeoff = chain_for(&response, "takeoff_time_seconds");
    assert_eq!(
        takeoff.provenance.method_id,
        "takeoff.threshold.absolute_force"
    );
}

/// An operator sits under the landmark rule it composes onto, never beside it.
///
/// An operator is a registry entry with its own citation and its own default, so it stands in
/// the chain under the rule it modified. Flattened onto the root it would read as a rule that
/// contributed to the quantity directly.
#[test]
fn an_operator_sits_under_the_landmark_rule_it_composes_onto() {
    let response = analysed(5.0);
    let chain = chain_for(&response, "time_to_takeoff_seconds");

    let onset = chain
        .depends_on
        .iter()
        .find(|input| input.provenance.method_id == "onset.threshold.noise_relative")
        .expect("time to takeoff rests on the onset rule");
    let under_onset: Vec<&str> = onset
        .depends_on
        .iter()
        .map(|input| input.provenance.method_id.as_str())
        .collect();
    assert!(
        under_onset.contains(&"onset.op.persistence"),
        "the onset operators are not under the onset rule: {under_onset:?}"
    );

    // The same operator must not also sit at the root's own level.
    let beside_the_root: Vec<&str> = chain
        .depends_on
        .iter()
        .map(|input| input.provenance.method_id.as_str())
        .collect();
    assert!(
        !beside_the_root.contains(&"onset.op.persistence"),
        "an onset operator is reported as contributing to the quantity directly: {beside_the_root:?}"
    );

    // And no takeoff operator sits under the onset rule, which is the failure a single list of
    // operators would produce.
    assert!(
        !under_onset.iter().any(|id| id.starts_with("takeoff.op.")),
        "a takeoff operator is filed under the onset rule: {under_onset:?}"
    );
}

/// Every rule the response says contributed is somewhere in the chain, either as a step or as
/// the value of a choice a step recorded.
///
/// The four integration ids have no bound record of their own: they are values of the
/// arithmetic rule's own choices. A chain that made a step for each would report one decision
/// twice, and one that dropped them would lose four choices that move the number.
#[test]
fn every_contributing_rule_is_somewhere_in_the_chain() {
    let response = analysed(5.0);
    let mut checked = 0usize;
    let mut ids = 0usize;

    for metric in &response.metrics {
        let chain = chain_of(&response, metric, &stamp(), true);
        let steps: Vec<&str> = chain
            .flattened()
            .iter()
            .map(|step| step.provenance.method_id.as_str())
            .collect();
        let recorded_values: Vec<String> = chain
            .flattened()
            .iter()
            .flat_map(|step| step.provenance.choices.iter().map(|c| c.value.clone()))
            .collect();

        for id in &metric.contributing_method_ids {
            ids += 1;
            assert!(
                steps.contains(&id.as_str()) || recorded_values.contains(id),
                "{} names {id} and the chain neither runs it nor records it as a choice: steps {steps:?}",
                metric.key
            );
        }
        checked += 1;
    }

    println!("{checked} metrics checked over {ids} contributing ids");
    // Eleven quantities on this trial. A count below that is a subject that shrank, and a guard
    // reading fewer than the build reports passes by looking at less.
    assert!(checked >= 11, "only {checked} metrics were reached");
    assert!(ids >= 100, "only {ids} contributing ids were reached");
}

/// A rule the tree runs is a rule the number is said to rest on, so the two readings of one
/// chain cannot drift apart.
///
/// `chain_of` walks the response into a tree and `metrics_resting_on` reads the same two flat
/// lists without building one. A signal about a rule is placed by the second, and every surface
/// draws the record from the first, so a rule the tree runs and the flat reading does not know
/// would be a signal placed beside none of the numbers whose record shows it.
///
/// The counts below are the population, because a walk that reached one metric and one step
/// would satisfy every assertion in the loop.
#[test]
fn a_rule_the_chain_runs_is_a_rule_the_number_rests_on() {
    let response = analysed(5.0);
    let mut steps_walked = 0usize;
    let mut metrics_walked = 0usize;

    for metric in &response.metrics {
        let chain = chain_of(&response, metric, &stamp(), true);
        for step in chain.flattened() {
            let id = step.provenance.method_id.as_str();
            assert!(
                chain_names(metric, id),
                "{} carries a step for {id} and is not said to rest on it",
                metric.key
            );
            assert!(
                metrics_resting_on(&response, id).contains(&metric.key),
                "{id} runs inside {}'s chain and the numbers resting on it leave it out",
                metric.key
            );
            steps_walked += 1;
        }
        metrics_walked += 1;
    }

    println!("{steps_walked} steps walked over {metrics_walked} metrics");
    assert!(
        metrics_walked >= 11,
        "only {metrics_walked} metrics reached"
    );
    assert!(steps_walked >= 50, "only {steps_walked} steps reached");
}

/// The chain is the type the fingerprint takes, so a surface holding a response can identify
/// what produced each of its numbers.
///
/// `plateforce_core::reporting::fingerprint` takes that tree, and a surface that cannot build
/// it from a response cannot call the fingerprint at all.
#[test]
fn a_number_can_be_fingerprinted_from_the_response_that_reported_it() {
    let taken = |k: f64| {
        fingerprint(
            &chain_for(&analysed(k), "jump_height_from_takeoff_meters").provenance,
            &a_recorded_plate(),
            SAMPLE_RATE_HZ,
        )
    };

    let once = taken(5.0);
    assert!(once.complete, "a recorded plate publishes a digest");
    assert_eq!(once.published(), Some(once.digest.as_str()));

    // One analysis run twice is one result.
    assert_eq!(once, taken(5.0));

    // And a value moved upstream, on a rule the height does not name in `computed_by`, is a
    // different result. A fingerprint blind to this would call two heights one.
    let moved = taken(3.0);
    assert_ne!(
        once, moved,
        "the onset threshold moved and the height fingerprinted the same"
    );
}

/// Every metric gets a chain, including one that reported no number, so a surface reporting
/// what a rule would have produced carries the same record as one reporting what it did.
#[test]
fn every_reported_quantity_gets_a_chain_in_the_order_the_response_lists_them() {
    let response = analysed(5.0);
    let derived = chains_of(&response, &stamp(), true);

    let quantities: Vec<&str> = derived.iter().map(|one| one.quantity.as_str()).collect();
    let reported: Vec<&str> = response
        .metrics
        .iter()
        .map(|metric| metric.key.as_str())
        .collect();
    assert_eq!(quantities, reported);
}

/// Every step in every chain carries the registry the analysis was read out of.
///
/// A step that lost the stamp would publish a number whose method nobody can look up.
#[test]
fn every_step_names_the_registry_behind_it() {
    let response = analysed(5.0);
    let mut steps = 0usize;

    for one in chains_of(&response, &stamp(), true) {
        for step in one.chain.flattened() {
            assert_eq!(
                step.provenance.registry_version.as_deref(),
                Some("fixture-pin"),
                "{} in {} lost the caller's pin",
                step.provenance.method_id,
                one.quantity
            );
            assert_eq!(
                step.provenance.registry_declared_version.as_deref(),
                Some("fixture-declares"),
                "{} in {} lost the registry's own claim",
                step.provenance.method_id,
                one.quantity
            );
            assert!(step.provenance.acquisition_complete);
            steps += 1;
        }
    }

    println!("{steps} steps checked");
    assert!(steps >= 50, "only {steps} steps were reached");
}
