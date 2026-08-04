//! What the phase boundaries have to hold, on one countermovement jump with a landing.
//!
//! Every per-phase quantity downstream of these rests on where the boundary went, and two
//! published phase models disagree about how many phases a countermovement jump has. So the
//! properties here are about which instant each rule placed and whose name is on it, not
//! about whether a rule ran.

use std::collections::BTreeMap;

use plateforce_analysis::binding::{derived_bindings, Binding};
use plateforce_analysis::slots::phase_model::CONSTRUCT as PHASE_MODEL;
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

fn keys_reported_by(response: &AnalysisResponse, rule: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.computed_by.as_deref() == Some(rule))
        .map(|metric| metric.key.clone())
        .collect()
}

/// Every quantity a phase model publishes carries a number on a recording the model places on,
/// and every key it reports is one its row publishes.
///
/// The population is read off the binding table rather than written here, so a third phase
/// model arrives inside this guard rather than beside it.
///
/// Asserted on the values rather than on the keys, and the difference is the whole guard. The
/// keys now come off the same quantities the row publishes, so a key set compared against that
/// row is a set compared with itself and cannot fail. The boundaries are the independent
/// population: they come back from `plateforce_core::phases`, one index per boundary the model
/// found, and a model whose row publishes six while its search returns five leaves a column the
/// registry promises, the interface draws, and no recording ever fills. On this trace all three
/// models place, so a quantity with no number here is that mismatch and nothing else.
///
/// The other direction, a number reported under a key the row does not publish, is asserted for
/// every derived rule where the metrics are built, in `pipeline.rs`, and repeating it here
/// would be an assertion that cannot fail.
#[test]
fn every_phase_model_fills_every_quantity_its_row_publishes() {
    let trial = a_jump_that_lands();
    let models: Vec<&Binding> = derived_bindings()
        .filter(|binding| binding.construct == PHASE_MODEL)
        .collect();
    // The control. Every assertion below holds over an empty population, and a construct
    // renamed out from under this filter would leave one.
    assert!(
        !models.is_empty(),
        "no row of the binding table is filed under {PHASE_MODEL}, so this guard walked nothing"
    );

    let mut faults = Vec::new();
    let mut valued = 0;
    let mut published = 0;
    for model in &models {
        let response = run(&trial, &naming(&[(model.construct, model.id)]))
            .unwrap_or_else(|error| panic!("{} could not run: {error}", model.id));
        published += model.quantities.len();

        let empty: Vec<&str> = model
            .quantities
            .iter()
            .map(|quantity| quantity.key)
            .filter(|key| value(&response, key).is_none())
            .collect();
        valued += model.quantities.len() - empty.len();
        if !empty.is_empty() {
            faults.push(format!(
                "{} publishes {} quantities and places {} boundaries on this recording, so \
                 {empty:?} reach a reader as columns nothing fills",
                model.id,
                model.quantities.len(),
                model.quantities.len() - empty.len()
            ));
        }
    }

    // What this guard covers, as a query rather than a figure written down. The population and
    // what it filled, counted apart, because a guard reporting one of them says nothing about
    // a model that ran and placed nothing.
    println!(
        "{} phase models publish {published} quantities, {valued} of them filled on this recording",
        models.len()
    );
    assert!(
        faults.is_empty(),
        "{} of {} phase models place something other than what they publish:\n  {}",
        faults.len(),
        models.len(),
        faults.join("\n  ")
    );
}

/// The registry's own claim about this pair, and the reason a phase model is a decision a user
/// makes rather than a default: the two models produce different sets of metrics, not
/// different values for one metric. So the keys change when the model changes, and a reader
/// comparing two results sees that rather than two numbers under one name.
#[test]
fn the_two_phase_models_report_two_different_sets_of_keys() {
    let trial = a_jump_that_lands();
    let single = run(
        &trial,
        &naming(&[("phase_model", "phase.model.unweighting_single.mcmahon2018")]),
    )
    .unwrap();
    let split = run(
        &trial,
        &naming(&[(
            "phase_model",
            "phase.model.unloading_yielding_split.harry2020",
        )]),
    )
    .unwrap();

    let single_keys = keys_reported_by(&single, "phase.model.unweighting_single.mcmahon2018");
    let split_keys = keys_reported_by(&split, "phase.model.unloading_yielding_split.harry2020");
    println!("single {single_keys:?}\nsplit {split_keys:?}");
    assert_eq!(single_keys.len(), 2, "{single_keys:?}");
    assert_eq!(split_keys.len(), 5, "{split_keys:?}");
    assert!(
        single_keys.iter().all(|key| !split_keys.contains(key)),
        "the two models shared a key, so a reader could take one model's number for the other"
    );
}

/// The difference between the two is the force minimum, and it is a boundary in one and not
/// the other. Measured on one trace: the split model's minimum lies strictly inside the single
/// model's one unweighting phase, which is what makes the second model a split of the first
/// rather than a different interval.
#[test]
fn the_split_model_puts_a_boundary_inside_the_single_models_unweighting_phase() {
    let trial = a_jump_that_lands();
    let single = run(
        &trial,
        &naming(&[("phase_model", "phase.model.unweighting_single.mcmahon2018")]),
    )
    .unwrap();
    let split = run(
        &trial,
        &naming(&[(
            "phase_model",
            "phase.model.unloading_yielding_split.harry2020",
        )]),
    )
    .unwrap();

    let start = value(&single, "unweighting_phase_start_seconds").expect("a start");
    let end = value(&single, "unweighting_phase_end_seconds").expect("an end");
    let minimum = value(&split, "force_minimum_seconds").expect("a minimum");
    println!("unweighting {start:.4} s to {end:.4} s, minimum {minimum:.4} s");
    assert!(
        start < minimum && minimum < end,
        "the minimum at {minimum:.4} s is not inside {start:.4} s to {end:.4} s"
    );
    assert!(
        value(&single, "force_minimum_seconds").is_none(),
        "the single-phase model reported the force minimum, which is the other model"
    );
}

/// The split model reads its own definition of where movement began, so a caller changing the
/// onset rule cannot move a boundary this model never asked that rule for.
///
/// The two threshold rules both land ahead of this model's own start on this trace, so they
/// cannot tell the two readings apart and a test built on them would pass either way. A
/// dragged marker placed deliberately after the model's own start can: under a reading that
/// searched from the bound onset, the unloading start would follow the marker.
#[test]
fn the_split_models_unloading_start_does_not_follow_the_bound_onset_rule() {
    let trial = a_jump_that_lands();
    let model = (
        "phase_model",
        "phase.model.unloading_yielding_split.harry2020",
    );
    let under = |onset_rule: &str, manual: Option<usize>| {
        let mut request = naming(&[model]);
        request.onset.method_id = onset_rule.to_string();
        request.onset.manual_index = manual;
        let response = run(&trial, &request).unwrap();
        (
            value(&response, "onset_time_seconds").expect("an onset"),
            value(&response, "unloading_phase_start_seconds").expect("an unloading start"),
        )
    };

    let (detected_onset, from_detected) = under("onset.threshold.noise_relative", None);
    let (other_onset, from_other) = under("onset.threshold.relative_to_system_weight", None);
    assert_eq!(
        from_detected, from_other,
        "the model's own start moved between two onset rules"
    );

    // A marker dragged well past the model's own start, so a rule that searched from the
    // bound onset could not return the same instant.
    let (dragged_onset, from_dragged) = under("onset.threshold.noise_relative", Some(1300));
    println!(
        "onsets {detected_onset:.4} s, {other_onset:.4} s, dragged {dragged_onset:.4} s; \
         unloading start {from_detected:.4} s and {from_dragged:.4} s"
    );
    assert!(
        dragged_onset > from_detected,
        "the marker at {dragged_onset:.4} s did not land after the model's own start at \
         {from_detected:.4} s, so this trace cannot tell the two readings apart"
    );
    assert_eq!(
        from_dragged, from_detected,
        "the model's own unloading start followed a dragged onset marker"
    );
}

/// The model's own drop is a bound value that does move it, which is what separates a
/// parameter this model owns from a choice it declines to inherit.
#[test]
fn a_deeper_unloading_drop_starts_the_split_model_later() {
    let trial = a_jump_that_lands();
    let at = |percent: f64| {
        let mut request = naming(&[(
            "phase_model",
            "phase.model.unloading_yielding_split.harry2020",
        )]);
        request
            .derived
            .get_mut("phase_model")
            .unwrap()
            .parameters
            .insert(
                "unloading_drop_percent_of_system_weight".to_string(),
                percent,
            );
        value(
            &run(&trial, &request).unwrap(),
            "unloading_phase_start_seconds",
        )
        .expect("an unloading start")
    };
    let published = at(2.5);
    let deeper = at(15.0);
    println!("2.5 percent at {published:.4} s, 15 percent at {deeper:.4} s");
    assert!(
        deeper > published,
        "a deeper drop started no later: {deeper:.4} s against {published:.4} s"
    );
}

/// Two published partitions of one interval, landing at two instants. Both bounded by the
/// same propulsion boundaries, so the difference is the partition and nothing else.
#[test]
fn the_two_propulsion_subdivisions_split_one_interval_at_two_instants() {
    let trial = a_jump_that_lands();
    let both = |model: &str| {
        with_option(
            &[
                (
                    "propulsion_phase_start",
                    "phase.propulsion_start.zero_velocity",
                ),
                (
                    "propulsion_phase_end",
                    "phase.propulsion_end.peak_com_velocity",
                ),
                ("propulsion_subdivision", model),
            ],
            "propulsion_phase_end",
            "search_signal",
            "velocity_argmax",
        )
    };
    let by_time = run(&trial, &both("phase.propulsion_subdivision.by_time")).unwrap();
    let by_force = run(
        &trial,
        &both("phase.propulsion_subdivision.by_force_crossing"),
    )
    .unwrap();

    let key = "propulsion_subdivision_seconds";
    let time_split = value(&by_time, key).expect("a split by time");
    let start = value(&by_time, "propulsion_phase_start_seconds").expect("a start");
    let end = value(&by_time, "propulsion_phase_end_seconds").expect("an end");
    println!("propulsion {start:.4} s to {end:.4} s, split by time {time_split:.4} s");
    assert!(
        start < time_split && time_split < end,
        "the split at {time_split:.4} s is outside {start:.4} s to {end:.4} s"
    );
    // Halfway by construction, which is the whole of what the arbitrary rule claims.
    assert!(
        ((time_split - start) - (end - time_split)).abs() < 0.002,
        "a 50 percent split was not halfway: {start:.4}, {time_split:.4}, {end:.4}"
    );

    // The event-anchored rule either lands somewhere else on the same interval, or says the
    // recording carries no such crossing. Both are answers; agreeing silently is not.
    match value(&by_force, key) {
        Some(force_split) => {
            println!("split by force crossing {force_split:.4} s");
            assert_ne!(
                force_split, time_split,
                "the two partitions returned one instant, so the pair proves nothing here"
            );
        }
        None => assert!(
            by_force
                .refusals
                .iter()
                .any(|rule| rule.method_id == "phase.propulsion_subdivision.by_force_crossing"),
            "the force-crossing rule reported nothing and said nothing about why"
        ),
    }
}

/// A subdivision is bounded by whatever the propulsion rules placed, so it names both of them
/// and moves when either does.
#[test]
fn a_propulsion_subdivision_names_both_boundaries_it_splits_between() {
    let trial = a_jump_that_lands();
    let response = run(
        &trial,
        &with_option(
            &[
                (
                    "propulsion_phase_start",
                    "phase.propulsion_start.zero_velocity",
                ),
                (
                    "propulsion_phase_end",
                    "phase.propulsion_end.peak_com_velocity",
                ),
                (
                    "propulsion_subdivision",
                    "phase.propulsion_subdivision.by_time",
                ),
            ],
            "propulsion_phase_end",
            "search_signal",
            "velocity_argmax",
        ),
    )
    .unwrap();
    let named = chain(&response, "propulsion_subdivision_seconds");
    for rule in [
        "phase.propulsion_start.zero_velocity",
        "phase.propulsion_end.peak_com_velocity",
    ] {
        assert!(
            named.contains(&rule.to_string()),
            "the split did not name {rule}: {named:?}"
        );
    }
}

/// The time-anchored model measures from onset rather than from an event on the trace, so its
/// boundary moves with the stated epoch and with nothing else on the far side of onset.
#[test]
fn a_longer_time_epoch_ends_later_and_lands_the_stated_distance_from_onset() {
    let trial = a_jump_that_lands();
    let at = |milliseconds: f64| {
        let mut request = naming(&[("phase_model", "phase.anchor.time_epochs.schmidtbleicher")]);
        request
            .derived
            .get_mut("phase_model")
            .unwrap()
            .parameters
            .insert("epoch_ms".to_string(), milliseconds);
        let response = run(&trial, &request).unwrap();
        let onset = value(&response, "onset_time_seconds").expect("an onset");
        value(&response, "time_epoch_end_seconds").expect("an epoch end") - onset
    };
    for milliseconds in [30.0, 50.0, 100.0, 200.0, 250.0] {
        let measured = at(milliseconds);
        println!(
            "{milliseconds} ms epoch measured {:.4} s from onset",
            measured
        );
        assert!(
            (measured - milliseconds / 1000.0).abs() < 0.002,
            "a {milliseconds} ms epoch ended {measured:.4} s after onset"
        );
    }
}

/// Which landmarks the countermovement promotes and where the propulsion phase divides are
/// two questions, and a caller may answer both on one analysis.
///
/// Written as the JSON a caller sends rather than as a request built field by field, because
/// the failure this guards against happens in the map before the engine is asked. A request
/// carries one rule per construct id, so two rules filed under one construct arrive as one
/// rule. A Python dict, a JavaScript object and a JSON document each keep one value per key,
/// so on those three surfaces the second rule is gone before anything can refuse it and the
/// engine is handed no evidence it was named. Measured on subject 01 trial 1: the model alone
/// adds two keys, the split alone adds one, and the three keys are disjoint, so a caller who
/// loses one loses a quantity rather than a spelling.
#[test]
fn a_phase_model_and_a_propulsion_split_are_two_answers_one_analysis_can_carry() {
    let trial = a_jump_that_lands();
    let request: AnalysisRequest = serde_json::from_str(
        r#"{
          "weighing": {
            "method_id": "bwepoch.fixed_window",
            "parameters": { "duration": 0.8 }
          },
          "onset": { "method_id": "onset.threshold.noise_relative" },
          "takeoff": { "method_id": "takeoff.threshold.absolute_force" },
          "derived": {
            "phase_model": {
              "method_id": "phase.model.unweighting_single.mcmahon2018"
            },
            "propulsion_phase_start": {
              "method_id": "phase.propulsion_start.zero_velocity"
            },
            "propulsion_phase_end": {
              "method_id": "phase.propulsion_end.peak_com_velocity",
              "options": { "search_signal": "velocity_argmax" }
            },
            "propulsion_subdivision": {
              "method_id": "phase.propulsion_subdivision.by_time"
            }
          }
        }"#,
    )
    .expect("the request a caller sends parses");

    // The map is the surface under test, so it is asserted before anything runs: two rules
    // named under two keys are two entries, and under one key they would be one.
    assert_eq!(
        request.derived.len(),
        4,
        "the request lost a rule before the engine saw it: {:?}",
        request.derived.keys().collect::<Vec<_>>()
    );

    let response = run(&trial, &request).expect("both rules run on one analysis");
    let ran: Vec<&str> = response
        .bound_methods
        .iter()
        .map(|bound| bound.method_id.as_str())
        .collect();
    println!("{ran:?}");

    for (id, key) in [
        (
            "phase.model.unweighting_single.mcmahon2018",
            "unweighting_phase_start_seconds",
        ),
        (
            "phase.model.unweighting_single.mcmahon2018",
            "unweighting_phase_end_seconds",
        ),
        (
            "phase.propulsion_subdivision.by_time",
            "propulsion_subdivision_seconds",
        ),
    ] {
        assert!(ran.contains(&id), "{id} is on no row: {ran:?}");
        assert!(
            value(&response, key).is_some(),
            "{id} ran and {key} carries no number"
        );
    }
}

/// Two rules named under one construct arrive as one rule, and the engine is handed nothing
/// that says a second was named.
///
/// The reason the test above has to state two construct keys rather than two rules under one.
/// A request holds a map, and a map keeps one value per key, so the loss happens in the reader
/// before `run` is called and no refusal inside the engine can reach it. The terminal is the
/// one surface that still sees two entries, because it parses repeated flags into its own map
/// and refuses on the repeat; Python, JavaScript and JSON each hand over the survivor.
///
/// Asserted against the same shape a caller sends rather than against `serde_json` in the
/// abstract, so this fails if `derived` ever stops being keyed by construct, which is the
/// change that would make the guard above unnecessary.
#[test]
fn two_rules_under_one_construct_reach_the_engine_as_one_with_no_word_of_the_other() {
    let request: AnalysisRequest = serde_json::from_str(
        r#"{
          "weighing": { "method_id": "bwepoch.fixed_window" },
          "onset": { "method_id": "onset.threshold.noise_relative" },
          "takeoff": { "method_id": "takeoff.threshold.absolute_force" },
          "derived": {
            "phase_model": {
              "method_id": "phase.model.unweighting_single.mcmahon2018"
            },
            "phase_model": {
              "method_id": "phase.propulsion_subdivision.by_time"
            }
          }
        }"#,
    )
    .expect("a repeated key is read rather than refused");

    assert_eq!(
        request.derived.len(),
        1,
        "two rules under one construct survived, so the map is no longer one rule per key"
    );
    assert_eq!(
        request.derived["phase_model"].method_id, "phase.propulsion_subdivision.by_time",
        "the survivor is the last one written, and the first is gone without a record"
    );
}
