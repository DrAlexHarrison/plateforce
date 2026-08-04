//! Three jump-height rules whose number rests on something no force trace carries.
//!
//! A box height, a foot length, an ankle angle: each is a scalar the operator measures once and
//! states, and each moves the height it feeds. The registry filed all three behind a barrier,
//! which is honest about the operator's recordings and says nothing about whether this build
//! can run the rule. So what has to hold is the pair: the rule computes when the reader states
//! the measurement, and refuses **by name** when they do not, rather than filling one in.
//!
//! Run on subject 01 trial 1, the committed corpus trial, because a rule that only ever ran on
//! a trace written to suit it has not been shown to compose with the spine.
//!
//! `cargo test -p plateforce-analysis --test a_height_that_needs_a_measurement_asks_for_it_by_name`

use std::collections::BTreeMap;

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, RefusalCode, Trial};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

/// The founding corpus samples at 1200 Hz. Read at 1000 every height below moves by a fifth.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

const TAKEOFF_FRAME: &str = "jump_height.takeoff_frame";
const STANDING_FRAME: &str = "jump_height.standing_frame";
const TAKEOFF_KEY: &str = "jump_height_from_takeoff_meters";
const STANDING_KEY: &str = "jump_height_from_standing_meters";

const DROP_FROM_BOX: &str = "jumpheight.dj.box_height_as_drop_height";
const ANKLE_CORRECTED: &str = "jumpheight.flight_time.ankle_angle_corrected";
const HEEL_RISE: &str = "jumpheight.standing.flight_time_anthropometric_correction.wade2020";
const IMPULSE_MOMENTUM: &str = "jumpheight.takeoff.impulse_momentum";

/// A 1.71 m subject in a 0.02 m sole, taking off plantarflexed and landing flat. The middle of
/// the range the ankle-angle source simulates.
const STATURE_METERS: f64 = 1.71;
const TAKEOFF_ANKLE_DEGREES: f64 = 40.0;
const LANDING_ANKLE_DEGREES: f64 = 5.0;

fn corpus_trial() -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .expect("the committed corpus trial reads");
    trial
}

/// The spine every case below runs over, with the landing stated so the flight-time routes have
/// two bounds rather than the end of the recording.
fn base() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
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

fn asking(construct: &str, method_id: &str, stated: &[(&str, f64)]) -> AnalysisRequest {
    let mut request = base();
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            parameters: stated
                .iter()
                .map(|(name, value)| ((*name).to_string(), *value))
                .collect(),
            ..Default::default()
        },
    );
    request
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// What the rule refused over, or `None` where it produced a number.
fn refusal_of(response: &AnalysisResponse, method_id: &str) -> Option<(RefusalCode, String)> {
    response
        .refusals
        .iter()
        .find(|rule| rule.method_id == method_id)
        .map(|rule| {
            let refusal = refusal_from_rule(rule);
            (refusal.code, refusal.message().to_string())
        })
}

/// The lengths the heel-rise constant reads, which the entry states required and publishes no
/// value for.
fn heel_rise_measurements() -> Vec<(&'static str, f64)> {
    vec![
        ("foot_length_m", 0.26),
        ("sole_thickness_m", 0.02),
        ("ankle_height_m", 0.067),
    ]
}

fn ankle_measurements() -> Vec<(&'static str, f64)> {
    vec![
        ("stature_m", STATURE_METERS),
        ("ankle_angle_at_takeoff_degrees", TAKEOFF_ANKLE_DEGREES),
        ("ankle_angle_at_landing_degrees", LANDING_ANKLE_DEGREES),
    ]
}

/// Each rule computes on the corpus trial once its measurement is stated, and each records the
/// entry that produced the number.
///
/// The provenance assertion is the half that matters here: a height that arrived without an id
/// a stranger can look up is the defect this project exists to remove, and it would otherwise
/// be indistinguishable from a height that arrived correctly.
#[test]
fn each_rule_computes_on_the_corpus_trial_and_names_the_entry_that_did_it() {
    let trial = corpus_trial();
    let cases: [(&str, &str, &str, Vec<(&str, f64)>); 3] = [
        (
            TAKEOFF_FRAME,
            DROP_FROM_BOX,
            TAKEOFF_KEY,
            vec![("box_height_m", 0.30)],
        ),
        (
            TAKEOFF_FRAME,
            ANKLE_CORRECTED,
            TAKEOFF_KEY,
            ankle_measurements(),
        ),
        (
            STANDING_FRAME,
            HEEL_RISE,
            STANDING_KEY,
            heel_rise_measurements(),
        ),
    ];

    for (construct, method_id, key, stated) in cases {
        let response = run(&trial, &asking(construct, method_id, &stated))
            .expect("the request is well formed");
        let height = value(&response, key).unwrap_or_else(|| {
            panic!(
                "{method_id} reported no {key}: {:?}",
                refusal_of(&response, method_id)
            )
        });
        let named = response
            .metrics
            .iter()
            .find(|metric| metric.key == key)
            .and_then(|metric| metric.computed_by.clone());
        println!("{method_id} on subject 01 trial 1: {height:.4} m, recorded under {named:?}");
        assert_eq!(
            named.as_deref(),
            Some(method_id),
            "{method_id} produced a number under another entry's name"
        );
        assert!(
            height.is_finite(),
            "{method_id} reported {height}, which is not a height"
        );
        // Every name the request carried was read. A rule that recorded a measurement it did
        // not consult would answer the reader's word with its own.
        let unread = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == method_id)
            .map(|bound| bound.unread_parameters.clone())
            .unwrap_or_else(|| panic!("{method_id} bound nothing"));
        assert!(
            unread.is_empty(),
            "{method_id} left {unread:?} unread while the request carried them"
        );
    }
}

/// A reader who states nothing gets a refusal naming the measurement, never a number.
///
/// The name in the refusal is the assertion. A rule that declined for any other reason, or that
/// declined with a sentence naming no field, sends the reader nowhere.
#[test]
fn a_rule_whose_measurement_is_unstated_refuses_over_that_name() {
    let trial = corpus_trial();
    let cases = [
        (TAKEOFF_FRAME, DROP_FROM_BOX, TAKEOFF_KEY, "box_height_m"),
        (
            TAKEOFF_FRAME,
            ANKLE_CORRECTED,
            TAKEOFF_KEY,
            "ankle_angle_at_takeoff_degrees",
        ),
        (STANDING_FRAME, HEEL_RISE, STANDING_KEY, "foot_length_m"),
    ];

    for (construct, method_id, key, expected_name) in cases {
        let response =
            run(&trial, &asking(construct, method_id, &[])).expect("the request is well formed");
        assert_eq!(
            value(&response, key),
            None,
            "{method_id} produced a height with {expected_name} unstated"
        );
        let (code, message) = refusal_of(&response, method_id)
            .unwrap_or_else(|| panic!("{method_id} neither produced a number nor refused"));
        println!("{method_id} with nothing stated: {code:?}, {message}");
        assert_eq!(
            code,
            RefusalCode::RequiredParameterUnstated,
            "{method_id} declined over {code:?} rather than over the measurement it needs"
        );
        assert!(
            message.contains(expected_name),
            "{method_id} refused without naming {expected_name}: {message}"
        );
    }
}

/// Stating a drop takes the height down, and the amount is the arrival velocity and nothing
/// else.
///
/// The check against impulse-momentum is what makes this more than a direction. Both rules
/// integrate the same trace between the same two landmarks, so the whole difference between
/// them is the boundary condition, and it has a closed form: the drop rule's velocity is the
/// other's less `sqrt(2 g h)`.
///
/// Backing the velocity out of the height loses its sign, so the boxes swept here are the ones
/// this corpus trial still leaves the athlete rising at takeoff under. The one that does not is
/// the case below.
#[test]
fn a_stated_box_height_moves_the_height_by_the_arrival_it_implies() {
    let trial = corpus_trial();
    let gravity = plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

    let from_rest = value(
        &run(&trial, &asking(TAKEOFF_FRAME, IMPULSE_MOMENTUM, &[]))
            .expect("the request is well formed"),
        TAKEOFF_KEY,
    )
    .expect("the impulse-momentum height");

    for box_height in [0.10, 0.20, 0.30] {
        let dropped = value(
            &run(
                &trial,
                &asking(
                    TAKEOFF_FRAME,
                    DROP_FROM_BOX,
                    &[("box_height_m", box_height)],
                ),
            )
            .expect("the request is well formed"),
            TAKEOFF_KEY,
        )
        .expect("the drop-jump height");

        // Back out each rule's takeoff velocity from the height it reported, which is the one
        // quantity the two rules differ in.
        let velocity_of = |height: f64| (2.0 * gravity * height).sqrt();
        let gap = velocity_of(from_rest) - velocity_of(dropped);
        let expected = (2.0 * gravity * box_height).sqrt();
        println!(
            "box {box_height:.2} m: {from_rest:.4} m from rest, {dropped:.4} m dropped, \
             velocity gap {gap:.4} m/s against the {expected:.4} m/s the drop implies"
        );
        assert!(
            (gap - expected).abs() < 1e-9,
            "a {box_height} m box moved takeoff velocity by {gap} m/s, not the {expected} m/s \
             free fall through it gives"
        );
    }
}

/// A drop the contact phase cannot pay for is refused, naming the height that was stated.
///
/// Without this the rule squares a negative takeoff velocity and reports the descent as a
/// height. It is the worst shape a number can have here: it stays positive, it stays small, and
/// it shrinks as the stated box gets further from the truth, so the reader sees a modest jump
/// exactly when the input is most wrong.
#[test]
fn a_drop_the_contact_phase_cannot_pay_for_is_refused_over_the_height_stated() {
    let trial = corpus_trial();
    let response = run(
        &trial,
        &asking(TAKEOFF_FRAME, DROP_FROM_BOX, &[("box_height_m", 0.45)]),
    )
    .expect("the request is well formed");

    assert_eq!(
        value(&response, TAKEOFF_KEY),
        None,
        "a 0.45 m drop this trial cannot pay for still reported a height"
    );
    let (code, message) = refusal_of(&response, DROP_FROM_BOX)
        .expect("the rule neither produced a number nor refused");
    println!("a 0.45 m box on subject 01 trial 1: {code:?}, {message}");
    assert_eq!(code, RefusalCode::ValueNotAccepted);
    assert!(
        message.contains("box_height_m"),
        "the refusal did not name the height that was stated: {message}"
    );

    // And the neighbouring height this trial does pay for still computes, so the refusal is
    // about the value rather than about the rule having stopped working.
    let smaller = run(
        &trial,
        &asking(TAKEOFF_FRAME, DROP_FROM_BOX, &[("box_height_m", 0.30)]),
    )
    .expect("the request is well formed");
    assert!(
        value(&smaller, TAKEOFF_KEY).is_some(),
        "a 0.30 m drop was refused too, so the refusal is not about the height stated"
    );
}

/// The two corrections move their heights in the direction their sources say, and by an amount
/// those sources would recognise.
///
/// A correction that computed and did not move the number would read as coverage while
/// delivering nothing, which is the failure mode this file is most exposed to: both rules add a
/// term to a number the build already produced.
#[test]
fn each_correction_moves_its_height_the_way_its_source_says() {
    let trial = corpus_trial();

    // The heel rise is added to a flight-time height, so the standing-frame number it gives
    // exceeds the takeoff-frame flight-time number by the constant, and the constant is 10 to
    // 12 cm on the source's own cohort.
    let with_heel_rise = value(
        &run(
            &trial,
            &asking(STANDING_FRAME, HEEL_RISE, &heel_rise_measurements()),
        )
        .expect("the request is well formed"),
        STANDING_KEY,
    )
    .expect("the heel-rise height");
    // The flight-time entry publishes a gravity of its own and declares 9.81, while both
    // corrections inherit the one the analysis chose. Stating it here holds the constant still
    // so what is measured below is the correction and not the 342 parts per million between
    // the two values of g.
    let flight_only = value(
        &run(
            &trial,
            &asking(
                TAKEOFF_FRAME,
                "jumpheight.takeoff.flight_time",
                &[(
                    "gravity",
                    plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
                )],
            ),
        )
        .expect("the request is well formed"),
        "jump_height_from_flight_time_meters",
    )
    .expect("the flight-time height");
    let constant = with_heel_rise - flight_only;
    println!(
        "heel rise on subject 01 trial 1: {flight_only:.4} m in the takeoff frame, \
         {with_heel_rise:.4} m in the standing frame, a {constant:.4} m constant"
    );
    assert!(
        (0.09..=0.14).contains(&constant),
        "the heel-rise constant came to {constant:.4} m, outside the 0.10 to 0.12 m its source \
         reports"
    );

    // The ankle correction takes height off when the athlete lands flatter than they took off,
    // and puts it back when they land more plantarflexed. Both directions, because a rule that
    // subtracted an absolute value would pass the first alone.
    let corrected = |takeoff_degrees: f64, landing_degrees: f64| {
        value(
            &run(
                &trial,
                &asking(
                    TAKEOFF_FRAME,
                    ANKLE_CORRECTED,
                    &[
                        ("stature_m", STATURE_METERS),
                        ("ankle_angle_at_takeoff_degrees", takeoff_degrees),
                        ("ankle_angle_at_landing_degrees", landing_degrees),
                    ],
                ),
            )
            .expect("the request is well formed"),
            TAKEOFF_KEY,
        )
        .expect("the ankle-corrected height")
    };

    let unchanged_posture = corrected(TAKEOFF_ANKLE_DEGREES, TAKEOFF_ANKLE_DEGREES);
    let landed_flat = corrected(TAKEOFF_ANKLE_DEGREES, LANDING_ANKLE_DEGREES);
    let landed_pointed = corrected(LANDING_ANKLE_DEGREES, TAKEOFF_ANKLE_DEGREES);
    println!(
        "ankle correction: {unchanged_posture:.4} m unchanged, {landed_flat:.4} m landing flat, \
         {landed_pointed:.4} m landing pointed"
    );
    assert!(
        (unchanged_posture - flight_only).abs() < 1e-12,
        "an unchanged ankle moved the projectile height from {flight_only} to {unchanged_posture}"
    );
    assert!(
        landed_flat < unchanged_posture,
        "landing flatter than takeoff did not take height off: {landed_flat} against \
         {unchanged_posture}"
    );
    assert!(
        landed_pointed > unchanged_posture,
        "landing more plantarflexed than takeoff did not add height: {landed_pointed} against \
         {unchanged_posture}"
    );

    // 35 degrees of lost plantarflexion on a 1.71 m subject is around 8 percent of a jump this
    // size, and the source puts an average subject at 8 to 13 percent.
    let shortfall = (unchanged_posture - landed_flat) / unchanged_posture;
    println!(
        "landing flat cost {:.1} percent of the height",
        shortfall * 100.0
    );
    assert!(
        (0.03..=0.30).contains(&shortfall),
        "the correction moved the number by {:.1} percent, which is not a posture change",
        shortfall * 100.0
    );
}
