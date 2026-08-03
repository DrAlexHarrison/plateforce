//! What the phase boundaries have to hold, on one countermovement jump with a landing.
//!
//! Every per-phase quantity downstream of these rests on where the boundary went, and two
//! published phase models disagree about how many phases a countermovement jump has. So the
//! properties here are about which instant each rule placed and whose name is on it, not
//! about whether a rule ran.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

/// A countermovement jump with a landing: quiet stance, an unweighting dip, a braking rise
/// through system weight, a propulsive peak, flight, and a landing larger than anything in
/// the jump.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, 1200.0).unwrap()
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

/// A request naming one rule per construct, with no parameters stated.
fn naming(pairs: &[(&str, &str)]) -> AnalysisRequest {
    let mut request = base();
    for (construct, method_id) in pairs {
        request.derived.insert(
            (*construct).to_string(),
            MethodChoice {
                method_id: (*method_id).to_string(),
                ..Default::default()
            },
        );
    }
    request
}

fn with_option(
    pairs: &[(&str, &str)],
    construct: &str,
    name: &str,
    value: &str,
) -> AnalysisRequest {
    let mut request = naming(pairs);
    if let Some(choice) = request.derived.get_mut(construct) {
        choice.options.insert(name.to_string(), value.to_string());
    }
    request
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

fn chain(response: &AnalysisResponse, key: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .map(|metric| metric.contributing_method_ids.clone())
        .unwrap_or_default()
}

const BRAKING_START: &str = "braking_phase_start_seconds";

/// The registry's own directional claim about this pair, which is why they are two entries
/// and not one: the force nadir strictly precedes the return through system weight, because
/// velocity keeps becoming more negative while force sits below it. Every braking-phase mean
/// is therefore taken over a longer and earlier window under the first rule.
#[test]
fn the_force_nadir_precedes_the_return_through_system_weight() {
    let trial = a_jump_that_lands();
    let nadir = value(
        &run(
            &trial,
            &naming(&[("braking_phase_start", "phase.braking_start.min_force")]),
        )
        .unwrap(),
        BRAKING_START,
    )
    .expect("the nadir is on this trace");
    let crossing = value(
        &run(
            &trial,
            &with_option(
                &[("braking_phase_start", "phase.braking_start.zero_net_force")],
                "braking_phase_start",
                "search_signal",
                "force_bw_crossing",
            ),
        )
        .unwrap(),
        BRAKING_START,
    )
    .expect("the crossing is on this trace");

    println!("nadir {nadir:.4} s, crossing {crossing:.4} s");
    assert!(
        nadir < crossing,
        "the nadir at {nadir:.4} s did not precede the crossing at {crossing:.4} s"
    );
    // Stated as a size as well as an order: a pair separated by one sample would satisfy the
    // ordering while telling a reader the two names are interchangeable, and they are not.
    assert!(
        crossing - nadir > 0.02,
        "the two rules landed {:.4} s apart, so this trace does not tell the names apart",
        crossing - nadir
    );
}

/// The two search signals of one entry are definitionally the same instant and numerically
/// different, which is what makes the signal a bound choice rather than a detail. Both land
/// inside the countermovement, which is what stops this passing on a rule that returned any
/// two different numbers.
#[test]
fn the_two_search_signals_of_one_entry_land_at_two_instants_inside_the_countermovement() {
    let trial = a_jump_that_lands();
    let named = [("braking_phase_start", "phase.braking_start.zero_net_force")];
    let mut placed = Vec::new();
    for signal in ["velocity_argmin", "force_bw_crossing"] {
        let response = run(
            &trial,
            &with_option(&named, "braking_phase_start", "search_signal", signal),
        )
        .unwrap();
        let seconds = value(&response, BRAKING_START)
            .unwrap_or_else(|| panic!("{signal} placed no boundary"));
        // The recorded choice is what a reader compares two results by, so it has to be in
        // the fingerprint under the registry's own parameter name.
        let bound = response
            .bound_methods
            .iter()
            .find(|method| method.method_id == "phase.braking_start.zero_net_force")
            .expect("the rule was bound");
        assert!(
            bound
                .bound_parameters
                .contains(&("search_signal".to_string(), signal.to_string())),
            "{signal} was not recorded: {:?}",
            bound.bound_parameters
        );
        placed.push(seconds);
    }

    let onset = value(&run(&trial, &base()).unwrap(), "onset_time_seconds").unwrap();
    let takeoff = value(&run(&trial, &base()).unwrap(), "takeoff_time_seconds").unwrap();
    for seconds in &placed {
        assert!(
            *seconds > onset && *seconds < takeoff,
            "a boundary at {seconds:.4} s sits outside onset {onset:.4} s to takeoff \
             {takeoff:.4} s"
        );
    }
    println!("velocity {:.4} s, force {:.4} s", placed[0], placed[1]);
    assert_ne!(
        placed[0], placed[1],
        "the search signal reached the same arithmetic under both names"
    );
}

/// A boundary is only comparable against another boundary when both name the rules they rest
/// on. The chain carries the three landmark rules because every phase boundary is measured
/// from them.
#[test]
fn a_placed_boundary_names_the_landmark_rules_it_rests_on() {
    let trial = a_jump_that_lands();
    for id in [
        "phase.braking_start.zero_net_force",
        "phase.braking_start.min_force",
    ] {
        let response = run(&trial, &naming(&[("braking_phase_start", id)])).unwrap();
        let metric = response
            .metrics
            .iter()
            .find(|metric| metric.key == BRAKING_START)
            .unwrap_or_else(|| panic!("{id} reported no boundary"));
        assert_eq!(metric.computed_by.as_deref(), Some(id));
        for landmark in [
            "bwepoch.fixed_window",
            "onset.threshold.noise_relative",
            "takeoff.threshold.absolute_force",
        ] {
            assert!(
                chain(&response, BRAKING_START).contains(&landmark.to_string()),
                "{id} did not name {landmark}: {:?}",
                metric.contributing_method_ids
            );
        }
    }
}

/// The velocity a boundary is read off carries four choices no rule in this build makes, and
/// a boundary that moved with a setting the fingerprint did not carry would be the defect
/// this registry documents. They are recorded as assumed, which is what they are.
#[test]
fn a_boundary_read_off_a_velocity_records_the_four_integration_choices() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[("braking_phase_start", "phase.braking_start.zero_net_force")]),
    )
    .unwrap();
    let bound = response
        .bound_methods
        .iter()
        .find(|method| method.method_id == "phase.braking_start.zero_net_force")
        .expect("the rule was bound");

    for (name, id) in [
        ("integration_rule", "integration.rule.trapezoid"),
        ("integration_direction", "integration.direction.forward"),
        ("integration_start", "integration.start.detected_onset"),
        ("integration_anchor", "integration.anchor.single_point"),
    ] {
        assert!(
            bound
                .bound_parameters
                .contains(&(name.to_string(), id.to_string())),
            "{name} did not reach the record: {:?}",
            bound.bound_parameters
        );
        assert_eq!(
            bound
                .parameter_sources
                .get(name)
                .map(|source| format!("{source:?}")),
            Some("Assumed".to_string()),
            "{name} was recorded as something other than assumed"
        );
    }
}

/// A rule whose landmark is missing declines by name, and names only the landmark it did not
/// get. A refusal listing every input a rule reads sends a reader to repair a rule that
/// answered.
#[test]
fn a_boundary_without_its_landmark_declines_naming_only_what_is_missing() {
    // A countermovement that never leaves the ground: the onset rule places a sample and the
    // takeoff rule finds no flight, so exactly one of the two landmarks is absent.
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 900.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 1200.0 - 600.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(600.0, 600));
    let trial = Trial::new(force, 1200.0).unwrap();
    let response = run(
        &trial,
        &naming(&[("braking_phase_start", "phase.braking_start.min_force")]),
    )
    .unwrap();

    let declined = response
        .refusals
        .iter()
        .find(|refusal| refusal.method_id == "phase.braking_start.min_force")
        .expect("the rule declined");
    let message = format!("{}", declined.refusal);
    assert!(
        message.contains("takeoff"),
        "the refusal did not name takeoff: {message}"
    );
    assert!(
        !message.contains("movement_onset"),
        "the refusal named an onset the rule was given: {message}"
    );
    assert!(value(&response, BRAKING_START).is_none());
}

const PROPULSION_START: &str = "propulsion_phase_start_seconds";
const PROPULSION_END: &str = "propulsion_phase_end_seconds";

/// Three rules under one construct, three instants, in the order their entries predict: the
/// velocity crossing, then the threshold that guards it against jitter, then the force
/// maximum, which has no mechanical reason to be the transition at all.
#[test]
fn the_three_propulsion_start_rules_place_three_instants_in_the_order_their_entries_predict() {
    let trial = a_jump_that_lands();
    let at = |id: &str| {
        value(
            &run(&trial, &naming(&[("propulsion_phase_start", id)])).unwrap(),
            PROPULSION_START,
        )
        .unwrap_or_else(|| panic!("{id} placed no boundary"))
    };
    let zero = at("phase.propulsion_start.zero_velocity");
    let threshold = at("phase.propulsion_start.velocity_threshold");
    let peak = at("phase.propulsion_start.peak_grf");

    println!("zero {zero:.4} s, threshold {threshold:.4} s, peak force {peak:.4} s");
    assert!(
        zero < threshold,
        "the threshold at {threshold:.4} s did not follow the zero crossing at {zero:.4} s"
    );
    assert!(
        threshold < peak,
        "peak force at {peak:.4} s did not follow the velocity crossing at {threshold:.4} s"
    );
    // The deprecated rule is carried to show a disagreement, so it has to be a real one.
    assert!(
        peak - zero > 0.02,
        "the legacy rule landed {:.4} s from the recommended one, which is not the gap its \
         entry records",
        peak - zero
    );
}

/// The threshold is a bound value that moves the boundary, not a formality.
#[test]
fn raising_the_propulsion_threshold_moves_the_boundary_later() {
    let trial = a_jump_that_lands();
    let at = |threshold: f64| {
        let mut request = naming(&[(
            "propulsion_phase_start",
            "phase.propulsion_start.velocity_threshold",
        )]);
        request
            .derived
            .get_mut("propulsion_phase_start")
            .unwrap()
            .parameters
            .insert("threshold_mps".to_string(), threshold);
        value(&run(&trial, &request).unwrap(), PROPULSION_START).expect("a boundary")
    };
    let published = at(0.01);
    let higher = at(0.30);
    println!("0.01 m/s at {published:.4} s, 0.30 m/s at {higher:.4} s");
    assert!(
        higher > published,
        "raising the threshold left the boundary at {published:.4} s against {higher:.4} s"
    );
}

/// The entry states its signal required with no default, so an unstated one is refused under
/// the code for a required parameter nobody stated rather than filled from a neighbour.
#[test]
fn the_propulsion_end_signal_is_refused_when_the_request_states_none() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[(
            "propulsion_phase_end",
            "phase.propulsion_end.peak_com_velocity",
        )]),
    )
    .unwrap();
    let declined = response
        .refusals
        .iter()
        .find(|refusal| refusal.method_id == "phase.propulsion_end.peak_com_velocity")
        .expect("the rule declined");
    let message = format!("{}", declined.refusal);
    assert!(
        message.contains("search_signal"),
        "the refusal did not name the parameter: {message}"
    );
    assert!(value(&response, PROPULSION_END).is_none());
}

/// Under the force signal the boundary is searched from the braking start, so it rests on
/// whichever braking rule ran and its chain says so. Asked without one it declines naming the
/// construct, rather than searching from somewhere convenient.
#[test]
fn the_force_signal_for_propulsion_end_rests_on_the_braking_rule_and_names_it() {
    let trial = a_jump_that_lands();
    let end_rule = (
        "propulsion_phase_end",
        "phase.propulsion_end.peak_com_velocity",
    );
    let braking = ("braking_phase_start", "phase.braking_start.zero_net_force");
    let signal = |pairs: &[(&str, &str)], value: &str| {
        with_option(pairs, "propulsion_phase_end", "search_signal", value)
    };

    let alone = run(&trial, &signal(&[end_rule], "force_bw_crossing")).unwrap();
    let declined = alone
        .refusals
        .iter()
        .find(|refusal| refusal.method_id == "phase.propulsion_end.peak_com_velocity")
        .expect("the rule declined without a braking start");
    assert!(
        format!("{}", declined.refusal).contains("braking_phase_start"),
        "the refusal did not name the construct it needed: {}",
        declined.refusal
    );

    let together = run(&trial, &signal(&[braking, end_rule], "force_bw_crossing")).unwrap();
    let named = chain(&together, PROPULSION_END);
    assert!(
        named.contains(&"phase.braking_start.zero_net_force".to_string()),
        "the boundary did not name the braking rule it was searched from: {named:?}"
    );

    // The velocity signal does not read the braking start, so it must not name it either. A
    // chain claiming more than the number rests on is as wrong as one claiming less.
    let velocity = run(&trial, &signal(&[braking, end_rule], "velocity_argmax")).unwrap();
    let velocity_chain = chain(&velocity, PROPULSION_END);
    assert!(
        !velocity_chain.contains(&"phase.braking_start.zero_net_force".to_string()),
        "the velocity signal named a rule it never read: {velocity_chain:?}"
    );
}

/// The registry's directional claim about this entry: peak velocity necessarily precedes
/// takeoff, because velocity peaks where net force crosses zero and takeoff is declared later.
/// Propulsion duration is therefore shorter, and mean propulsion force higher, than under the
/// reading that propulsion ends when the athlete leaves the plate.
#[test]
fn propulsion_ends_before_takeoff_under_this_entry() {
    let trial = a_jump_that_lands();
    let response = run(
        &trial,
        &with_option(
            &[(
                "propulsion_phase_end",
                "phase.propulsion_end.peak_com_velocity",
            )],
            "propulsion_phase_end",
            "search_signal",
            "velocity_argmax",
        ),
    )
    .unwrap();
    let end = value(&response, PROPULSION_END).expect("a boundary");
    let takeoff = value(&response, "takeoff_time_seconds").expect("a takeoff");
    println!("propulsion ends {end:.4} s, takeoff {takeoff:.4} s");
    assert!(
        end < takeoff,
        "propulsion ended at {end:.4} s, at or after takeoff at {takeoff:.4} s"
    );
}
