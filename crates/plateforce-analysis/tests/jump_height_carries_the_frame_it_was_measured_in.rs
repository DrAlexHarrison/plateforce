//! What the jump-height rules have to hold, on a trace whose flight phase has a closed form.
//!
//! The frame is the whole reason the published methods disagree as widely as they do, so the
//! properties here are mostly about the gaps between rules rather than about any one number:
//! two rules that agreed on every trial would not need to be two entries.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{RefusalCode, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
/// Twice the takeoff velocity over gravity, in samples. The flight this trace spends off the
/// plate is the flight the impulse it recorded pays for, so the two routes to the takeoff
/// frame have to meet on it.
const FLIGHT_SAMPLES: usize = 811;
const GRAVITY: f64 = plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

const TAKEOFF_FRAME: &str = "jump_height.takeoff_frame";
const STANDING_FRAME: &str = "jump_height.standing_frame";
const UNDECLARED: &str = "jump_height.undeclared";

const TAKEOFF_KEY: &str = "jump_height_from_takeoff_meters";
const FLIGHT_KEY: &str = "jump_height_from_flight_time_meters";
const STANDING_KEY: &str = "jump_height_from_standing_meters";
const UNDECLARED_KEY: &str = "jump_height_undeclared_frame_meters";

/// A countermovement jump that is consistent with itself.
///
/// The countermovement is deep enough and the propulsion long enough that the centre of mass
/// is above standing at takeoff, which is the ordering between the two frames the registry
/// states and the thing a force trace assembled at random does not give. The flight is then
/// the flight that takeoff velocity pays for, so the projectile equation and the impulse meet
/// rather than disagreeing by an amount this file would have to hard-code.
///
/// Quiet standing carries a sawtooth, because the noise-relative onset rule reads a band of
/// k standard deviations and a perfectly flat epoch collapses it to nothing.
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

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// The one number a rule reports, run on the trace that lands.
fn height(construct: &str, method_id: &str, key: &str) -> f64 {
    let response = run(&a_jump_that_lands(), &naming(&[(construct, method_id)]))
        .expect("the request is well formed");
    value(&response, key).unwrap_or_else(|| {
        panic!(
            "{method_id} reported no {key}: {:?}",
            response
                .refusals
                .iter()
                .map(|rule| rule.refusal.to_string())
                .collect::<Vec<_>>()
        )
    })
}

/// The projectile equation on a flight phase of stated length, which is the one jump-height
/// answer that has a closed form. Nothing about the integrator can move it.
///
/// A reader who states nothing gets the value the entry declares, 9.81, and not the constant
/// the request carries. The entry publishes four values because the tools disagree on this one
/// and a rule running on whichever number a struct initialiser held would be the silent
/// default with the paperwork of a declared one.
#[test]
fn the_flight_time_height_is_the_closed_form_at_the_gravity_its_entry_declares() {
    use plateforce_analysis::slots::jh_takeoff_frame::flight_time::{
        GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED as DECLARED, GRAVITY_PARAMETER,
    };

    let seconds = FLIGHT_SAMPLES as f64 / SAMPLE_RATE_HZ;
    let stated_nothing = height(TAKEOFF_FRAME, "jumpheight.takeoff.flight_time", FLIGHT_KEY);
    let closed_form = DECLARED * seconds * seconds / 8.0;
    println!("flight {seconds:.4} s, height {stated_nothing:.6} m, closed form at g = {DECLARED} is {closed_form:.6} m");
    assert!((stated_nothing - closed_form).abs() < 1e-9);
    assert_ne!(
        DECLARED, GRAVITY,
        "the entry's value and the request's constant are the same number here, so this could not tell them apart"
    );

    // And a reader who does state one gets theirs. A rule that took its entry's default over a
    // stated value would be the same fault pointing the other way.
    let mut request = naming(&[(TAKEOFF_FRAME, "jumpheight.takeoff.flight_time")]);
    request
        .derived
        .get_mut(TAKEOFF_FRAME)
        .unwrap()
        .parameters
        .insert(GRAVITY_PARAMETER.to_string(), 9.8);
    let response = run(&a_jump_that_lands(), &request).expect("the request is well formed");
    let stated = value(&response, FLIGHT_KEY).expect("a height");
    println!("stated g = 9.8 gives {stated:.6} m");
    assert!((stated - 9.8 * seconds * seconds / 8.0).abs() < 1e-9);
}

/// The two frames are different quantities and the standing frame is the larger, by exactly
/// the rise the centre of mass made while the foot was still on the plate.
///
/// The sign is the property. A test on the size alone would pass with the two subtracted the
/// wrong way round, and that is the error that would file a standing-frame number under the
/// takeoff frame.
#[test]
fn the_standing_frame_is_the_takeoff_frame_plus_the_rise_to_takeoff() {
    let from_takeoff = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.impulse_momentum",
        TAKEOFF_KEY,
    );
    let from_standing = height(
        STANDING_FRAME,
        "jumpheight.standing.tov_plus_displacement",
        STANDING_KEY,
    );
    let rise_to_takeoff = from_standing - from_takeoff;
    let percent = rise_to_takeoff / from_takeoff * 100.0;
    println!(
        "takeoff frame {from_takeoff:.6} m, standing frame {from_standing:.6} m, rise to takeoff {rise_to_takeoff:.6} m, {percent:.1} percent"
    );
    assert!(
        from_standing > from_takeoff,
        "the standing frame has to be the larger of the two"
    );
    // The published separation between the frames is 26 to 45 percent on average. A trace
    // outside it would still exercise the arithmetic and would stop standing for the jump the
    // registry's figures were measured on.
    assert!(
        (26.0..=45.0).contains(&percent),
        "the two frames are {percent:.1} percent apart on this trace"
    );
}

/// On a jump whose flight is the flight its impulse pays for, the projectile equation and the
/// impulse have to meet. They are two estimators of one construct and the registry files them
/// under one, so a trace where the assumption behind the first holds is where they agree.
#[test]
fn the_two_takeoff_frame_routes_meet_on_a_jump_that_is_consistent_with_itself() {
    let from_impulse = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.impulse_momentum",
        TAKEOFF_KEY,
    );
    let from_flight = height(TAKEOFF_FRAME, "jumpheight.takeoff.flight_time", FLIGHT_KEY);
    let gap_centimetres = (from_flight - from_impulse).abs() * 100.0;
    println!(
        "impulse {from_impulse:.6} m, flight time {from_flight:.6} m, gap {gap_centimetres:.4} cm"
    );
    assert!(
        gap_centimetres < 0.2,
        "the two routes are {gap_centimetres:.4} cm apart on a trace built so they agree"
    );
}

/// Work through the displacement and impulse through the time are the same theorem, so these
/// two agree to the quadrature and not further. A gap of any real size means one of them is
/// integrating something else.
#[test]
fn the_work_energy_route_and_the_impulse_route_are_one_theorem() {
    let from_impulse = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.impulse_momentum",
        TAKEOFF_KEY,
    );
    let from_work = height(TAKEOFF_FRAME, "jumpheight.takeoff.work_energy", TAKEOFF_KEY);
    let gap = (from_work - from_impulse).abs();
    println!("impulse {from_impulse:.6} m, work energy {from_work:.6} m, gap {gap:.8} m");
    assert!(
        gap < 1e-3,
        "the two routes differ by {gap:.8} m, which is more than a quadrature apart"
    );
}

/// Peak centre-of-mass velocity precedes takeoff, because force falls below system weight
/// while the foot is still down. So this route reads at or above the takeoff-velocity route,
/// and the direction is fixed by the mechanism rather than by the trace.
#[test]
fn the_peak_velocity_route_never_reads_below_the_takeoff_velocity_route() {
    let at_takeoff = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.impulse_momentum",
        TAKEOFF_KEY,
    );
    let at_peak = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.peak_velocity.chavda2018",
        TAKEOFF_KEY,
    );
    println!("at takeoff {at_takeoff:.6} m, at peak velocity {at_peak:.6} m");
    assert!(at_peak >= at_takeoff);
}

/// The minimum of two estimators returns one of them, and which one is not fixed across
/// trials. Both halves are checked, because a rule returning either one unconditionally would
/// satisfy a test that only asked for the smaller.
///
/// Its own entry declares no gravity, so both of its terms run under the one the request
/// carries. That is not the value the flight-time entry declares for its own route, so this
/// compares against the terms this rule computed rather than against the other rule's number.
#[test]
fn the_minimum_route_returns_the_smaller_of_the_two_it_computed() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[
            ("flight_time", "flight_time.takeoff_to_touchdown"),
            (UNDECLARED, "jumpheight.min_of_ft_and_tov.labanalysis"),
        ]),
    )
    .expect("the request is well formed");
    let seconds = value(&response, "flight_time_seconds").expect("a flight time");
    let minimum = value(&response, UNDECLARED_KEY).expect("a height");

    // The same core functions this rule calls, at the gravity this rule ran under.
    let flight_term = plateforce_core::jump_height_from_flight_time(seconds, GRAVITY);
    let impulse_term = height(
        TAKEOFF_FRAME,
        "jumpheight.takeoff.impulse_momentum",
        TAKEOFF_KEY,
    );
    println!(
        "flight term {flight_term:.6} m, impulse term {impulse_term:.6} m, minimum {minimum:.6} m"
    );
    assert_eq!(minimum, flight_term.min(impulse_term));
    assert!(minimum <= flight_term && minimum <= impulse_term);
}

/// The apex searched over the flight and the apex searched from standing are the same instant
/// on a jump that goes up once, and the two entries differ in what they say the number means
/// rather than in what they compute here.
#[test]
fn the_flight_phase_apex_and_the_whole_curve_apex_agree_on_a_single_jump() {
    let over_flight = height(
        UNDECLARED,
        "jumpheight.flight_phase_displacement.vald_impdis",
        UNDECLARED_KEY,
    );
    let from_standing = height(
        STANDING_FRAME,
        "jumpheight.standing.double_integration",
        STANDING_KEY,
    );
    println!("flight-phase apex {over_flight:.6} m, whole-curve apex {from_standing:.6} m");
    assert!((over_flight - from_standing).abs() < 1e-9);
}

/// The two standing-frame routes answer the same question and are not the same arithmetic, so
/// they agree to the integrator rather than exactly.
#[test]
fn the_two_standing_frame_routes_agree_on_the_apex_they_both_measure() {
    let integrated = height(
        STANDING_FRAME,
        "jumpheight.standing.double_integration",
        STANDING_KEY,
    );
    let summed = height(
        STANDING_FRAME,
        "jumpheight.standing.tov_plus_displacement",
        STANDING_KEY,
    );
    let gap = (integrated - summed).abs();
    println!("double integration {integrated:.6} m, takeoff velocity plus rise {summed:.6} m, gap {gap:.6} m");
    assert!(gap < 0.02, "the two routes differ by {gap:.6} m");
}

/// The interval rules report the samples between the landmarks they were given, and the
/// closed form is the sample count over the rate.
#[test]
fn the_intervals_report_the_samples_between_the_landmarks_that_bound_them() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[
            ("flight_time", "flight_time.takeoff_to_touchdown"),
            ("time_to_takeoff", "time_to_takeoff.onset_to_takeoff"),
        ]),
    )
    .expect("the request is well formed");

    let flight = value(&response, "flight_time_seconds").expect("a flight time");
    let to_takeoff = value(&response, "time_to_takeoff_seconds").expect("an interval to takeoff");
    let takeoff_index = response.takeoff_index.expect("takeoff was placed");
    let onset_index = response.onset_index.expect("onset was placed");
    let touchdown_index = response.touchdown_index.expect("the landing was found");
    println!(
        "onset {onset_index}, takeoff {takeoff_index}, touchdown {touchdown_index}, to takeoff {to_takeoff:.6} s, flight {flight:.6} s"
    );
    assert!(
        (flight - (touchdown_index - takeoff_index) as f64 / SAMPLE_RATE_HZ).abs() < 1e-12,
        "flight time is not the interval between the two samples it names"
    );
    assert!((to_takeoff - (takeoff_index - onset_index) as f64 / SAMPLE_RATE_HZ).abs() < 1e-12);
}

/// The frame declaration refuses rather than picking one. It is the one jump-height choice the
/// registry says a reader must not be allowed to default through, so a rule that supplied a
/// frame would be answering the question the entry exists to ask.
#[test]
fn the_frame_declaration_refuses_until_a_reader_states_which_rise_they_mean() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[(UNDECLARED, "jumpheight.frame")]),
    )
    .expect("the request is well formed");
    let declined = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "jumpheight.frame")
        .expect("the declaration declined");
    let refusal = plateforce_analysis::document::refusal_from_rule(declined);
    println!("{}", refusal.message());
    assert_eq!(refusal.code, RefusalCode::RequiredParameterUnstated);
    assert_eq!(refusal.parameter.as_deref(), Some("frame"));
}

/// Both frames are accepted, and a third word is not. Substituting a frame for one the reader
/// wrote would record their word beside a number the other frame produced.
#[test]
fn the_frame_declaration_takes_the_two_frames_and_refuses_a_third_word() {
    for frame in ["takeoff", "standing"] {
        let mut request = naming(&[(UNDECLARED, "jumpheight.frame")]);
        request
            .derived
            .get_mut(UNDECLARED)
            .unwrap()
            .options
            .insert("frame".to_string(), frame.to_string());
        let response = run(&a_jump_that_lands(), &request).expect("the request is well formed");
        assert!(
            response.refusals.is_empty(),
            "{frame} was refused: {:?}",
            response
                .refusals
                .iter()
                .map(|rule| rule.refusal.to_string())
                .collect::<Vec<_>>()
        );
        let declared = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == "jumpheight.frame")
            .expect("the declaration was recorded");
        assert!(
            declared
                .bound_parameters
                .iter()
                .any(|(name, shown)| name == "frame" && shown == frame),
            "the record does not carry the frame the reader stated"
        );
    }

    let mut request = naming(&[(UNDECLARED, "jumpheight.frame")]);
    request
        .derived
        .get_mut(UNDECLARED)
        .unwrap()
        .options
        .insert("frame".to_string(), "apex".to_string());
    let response = run(&a_jump_that_lands(), &request).expect("the request is well formed");
    let declined = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "jumpheight.frame")
        .expect("a word that is not a frame is refused");
    let refusal = plateforce_analysis::document::refusal_from_rule(declined);
    println!("{}", refusal.message());
    assert_eq!(refusal.code, RefusalCode::ValueNotAccepted);
    assert_eq!(refusal.named_value.as_deref(), Some("apex"));
}

/// A height whose flight the recording never closed is refused by name, rather than reading
/// the tail of an untrimmed recording as flight.
#[test]
fn a_flight_time_height_on_a_recording_that_never_lands_is_refused_by_name() {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 1200));
    let never_lands = Trial::new(force, SAMPLE_RATE_HZ).unwrap();

    let response = run(
        &never_lands,
        &naming(&[(TAKEOFF_FRAME, "jumpheight.takeoff.flight_time")]),
    )
    .expect("the request is well formed");
    let declined = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "jumpheight.takeoff.flight_time")
        .expect("the rule declined");
    let refusal = plateforce_analysis::document::refusal_from_rule(declined);
    println!("{}", refusal.message());
    assert_eq!(refusal.code, RefusalCode::RequiredParameterUnstated);
    assert!(value(&response, FLIGHT_KEY).is_none());
}

/// Every rule names itself on the number it produced, and reports it once. Three of these
/// report one key, so a shared declaration would put the first rule's name on all three.
#[test]
fn each_height_rule_reports_its_own_arithmetic_exactly_once() {
    for (construct, id, key) in [
        (
            TAKEOFF_FRAME,
            "jumpheight.takeoff.impulse_momentum",
            TAKEOFF_KEY,
        ),
        (TAKEOFF_FRAME, "jumpheight.takeoff.flight_time", FLIGHT_KEY),
        (
            TAKEOFF_FRAME,
            "jumpheight.takeoff.peak_velocity.chavda2018",
            TAKEOFF_KEY,
        ),
        (TAKEOFF_FRAME, "jumpheight.takeoff.work_energy", TAKEOFF_KEY),
        (
            STANDING_FRAME,
            "jumpheight.standing.double_integration",
            STANDING_KEY,
        ),
        (
            STANDING_FRAME,
            "jumpheight.standing.tov_plus_displacement",
            STANDING_KEY,
        ),
        (
            UNDECLARED,
            "jumpheight.flight_phase_displacement.vald_impdis",
            UNDECLARED_KEY,
        ),
        (
            UNDECLARED,
            "jumpheight.min_of_ft_and_tov.labanalysis",
            UNDECLARED_KEY,
        ),
        (
            "flight_time",
            "flight_time.takeoff_to_touchdown",
            "flight_time_seconds",
        ),
        (
            "time_to_takeoff",
            "time_to_takeoff.onset_to_takeoff",
            "time_to_takeoff_seconds",
        ),
    ] {
        let response = run(&a_jump_that_lands(), &naming(&[(construct, id)]))
            .expect("the request is well formed");
        let carrying: Vec<&plateforce_analysis::Metric> = response
            .metrics
            .iter()
            .filter(|metric| metric.key == key)
            .collect();
        assert_eq!(
            carrying.len(),
            1,
            "{id} reported {key} {} times",
            carrying.len()
        );
        assert_eq!(carrying[0].computed_by.as_deref(), Some(id));
    }
}

/// No response carries one key twice, for any rule this build runs rather than for the ones
/// this file is about.
///
/// Two surfaces read a response by key and resolve a repeat in opposite directions, so a
/// repeated key is one trial reported as two different numbers depending on who asked. The
/// spine leaves out any key a bound rule will report.
#[test]
fn no_rule_this_build_runs_reports_a_key_a_second_time() {
    let trial = a_jump_that_lands();
    let mut checked = 0usize;
    for binding in plateforce_analysis::BINDINGS {
        // Conditioning runs before the spine rather than over what it placed, so it is
        // reached through its own map. Matched on the dispatch rather than on a construct
        // name, so a second conditioning rule is skipped here without an edit.
        if matches!(
            binding.dispatch,
            plateforce_analysis::binding::Dispatch::Conditioning(_)
        ) {
            continue;
        }
        let mut request = base();
        match binding.slot {
            "weighing" | "onset" | "takeoff" => continue,
            construct => {
                request.derived.insert(
                    construct.to_string(),
                    MethodChoice {
                        method_id: binding.id.to_string(),
                        ..Default::default()
                    },
                );
            }
        }
        // The window rules place what several later rules read, so a request naming only one
        // of those would decline rather than exercising the report this guard reads.
        request
            .derived
            .entry("analysis_window".to_string())
            .or_insert(MethodChoice {
                method_id: "window_end.takeoff.detected".to_string(),
                ..Default::default()
            });

        let response = run(&trial, &request).expect("the request is well formed");
        let mut seen: Vec<&str> = Vec::new();
        for metric in &response.metrics {
            assert!(
                !seen.contains(&metric.key.as_str()),
                "{} produced a response carrying {} twice",
                binding.id,
                metric.key
            );
            seen.push(metric.key.as_str());
        }
        checked += 1;
    }
    println!(
        "{checked} of {} rules checked for a repeated key",
        plateforce_analysis::BINDINGS.len()
    );
    assert!(checked >= 13, "only {checked} rules were reached");
}

/// The four integration choices behind every height reach the record. A velocity integrated
/// under a quadrature nobody stated is the silent default this registry documents, and three
/// of these rules never see a spec because the core function integrates for itself.
#[test]
fn every_height_records_the_integration_choices_its_series_was_built_under() {
    for (construct, id) in [
        (TAKEOFF_FRAME, "jumpheight.takeoff.impulse_momentum"),
        (TAKEOFF_FRAME, "jumpheight.takeoff.peak_velocity.chavda2018"),
        (TAKEOFF_FRAME, "jumpheight.takeoff.work_energy"),
        (STANDING_FRAME, "jumpheight.standing.double_integration"),
        (STANDING_FRAME, "jumpheight.standing.tov_plus_displacement"),
        (
            UNDECLARED,
            "jumpheight.flight_phase_displacement.vald_impdis",
        ),
        (UNDECLARED, "jumpheight.min_of_ft_and_tov.labanalysis"),
    ] {
        let response = run(&a_jump_that_lands(), &naming(&[(construct, id)]))
            .expect("the request is well formed");
        let bound = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == id)
            .unwrap_or_else(|| panic!("{id} recorded nothing"));
        let named: Vec<&str> = bound
            .bound_parameters
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        for choice in [
            "integration_rule",
            "integration_direction",
            "integration_start",
            "integration_anchor",
        ] {
            assert!(
                named.contains(&choice),
                "{id} recorded no {choice}, so the series it read was built on a choice nobody can see: {named:?}"
            );
        }
        // The values are registry ids, so a reader looks the choice up rather than reading a
        // word this crate invented.
        assert!(bound
            .bound_parameters
            .iter()
            .any(|(name, shown)| name == "integration_rule" && shown.starts_with("integration.")));
    }
}
