//! What the reactive-strength family has to hold once four of its entries compute.
//!
//! Every rule here is a division, and the whole content of each one is which two quantities
//! it divided. A ratio reported without the pair named is the shape this project exists to
//! refuse, so what is measured below is that the choices move the number, and by how much on
//! a real recording rather than in prose.
//!
//! Five of the six committed trials were trimmed before the athlete came back down, so the
//! two rules that read past takeoff have a denominator of one here and this file says so
//! wherever it reads past takeoff.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures"
);

/// The founding corpus samples at 1200 Hz. Reading these traces at 1000 corrupts every
/// velocity, displacement and interval measured across one by 20 percent.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

const RSI_KEY: &str = "reactive_strength_index_modified";
const RATIO_KEY: &str = "reactive_strength_ratio";
const RPEM_KEY: &str = "rpem_index_meters_per_second";
const STABILISATION_KEY: &str = "time_to_stabilisation_seconds";

const RSI_CONSTRUCT: &str = "reactive_strength_index";
const RATIO_CONSTRUCT: &str = "reactive_strength_ratio";
const RPEM_CONSTRUCT: &str = "reactive_strength_index.rpem";
const STABILISATION_CONSTRUCT: &str = "time_to_stabilisation";

fn corpus_trial(name: &str) -> Trial {
    let path = format!("{FIXTURE_ROOT}/{name}.force.txt");
    let (trial, _) = read_trial_from_path(&path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

/// The one committed trial that returns to the plate.
fn subject01_trial1() -> Trial {
    corpus_trial("subject01_trial1")
}

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

/// A construct, the rule to bind there, and the options stated on it.
type BoundSlot<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

/// A request naming one rule per construct, with the names and numbers each rule states
/// required and publishes no value for.
fn naming(pairs: &[BoundSlot]) -> AnalysisRequest {
    let mut request = base();
    for (construct, method_id, options) in pairs {
        request.derived.insert(
            (*construct).to_string(),
            MethodChoice {
                method_id: (*method_id).to_string(),
                options: options
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                    .collect(),
                ..Default::default()
            },
        );
    }
    request
}

/// The landmarks every rule in this file divides, named once.
const LANDMARKS: &[BoundSlot<'static>] = &[
    (
        "propulsion_phase_start",
        "phase.propulsion_start.zero_velocity",
        &[],
    ),
    ("landing", "landing.threshold.tied_to_takeoff", &[]),
];

fn with_landmarks(extra: &[BoundSlot]) -> AnalysisRequest {
    let mut pairs = LANDMARKS.to_vec();
    pairs.extend_from_slice(extra);
    naming(&pairs)
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// The reason a rule gave for reporting nothing, as the sentence a reader meets.
fn declined_because(response: &AnalysisResponse, method_id: &str) -> Option<String> {
    response
        .refusals
        .iter()
        .map(|declined| plateforce_core::Refusal::from(declined.refusal.clone()))
        .find(|refusal| refusal.method_id == method_id)
        .map(|refusal| refusal.message().to_string())
}

fn rpem_under(trial: &Trial, term: &str) -> Option<f64> {
    let request = with_landmarks(&[(
        RPEM_CONSTRUCT,
        "index.rpem.riosgallardo",
        &[("velocity_term", term)],
    )]);
    // The slice above cannot borrow `term`, so the option is rebuilt on the request itself.
    let mut request = request;
    request
        .derived
        .get_mut(RPEM_CONSTRUCT)
        .expect("the request names the rule")
        .options
        .insert("velocity_term".to_string(), term.to_string());
    let response = run(trial, &request).expect("the request is answerable");
    value(&response, RPEM_KEY)
}

/// The three velocities a plate can hand this rule are three numbers from one recording, and
/// two of them stand in a fixed relation the arithmetic guarantees.
///
/// The source states none of the three. Its own analysis divides by a vendor column that is
/// the takeoff velocity halved, so a reader who took the word at face value and used the
/// takeoff velocity would report exactly twice the source's figure under the source's name.
#[test]
fn the_three_velocity_terms_are_three_numbers_from_one_recording() {
    let trial = subject01_trial1();
    let modelled = rpem_under(&trial, "mean_from_takeoff_velocity").expect("the modelled mean");
    let measured = rpem_under(&trial, "mean_over_push_off").expect("the measured mean");
    let at_takeoff = rpem_under(&trial, "takeoff").expect("the takeoff velocity");

    println!(
        "subject 01 trial 1: modelled {modelled:.4}, measured {measured:.4}, at takeoff \
         {at_takeoff:.4} m/s"
    );
    assert!(
        (at_takeoff - 2.0 * modelled).abs() < 1e-9,
        "the takeoff velocity is not twice the modelled mean: {at_takeoff} against {modelled}"
    );
    // The measured mean against the modelled one is the uniform-acceleration assumption, and
    // it is the one difference here that is a fact about the athlete rather than about the
    // arithmetic. Bounded rather than asserted equal, because a jump whose push-off really
    // was uniform would put them together and that is not this recording.
    assert!(
        measured > modelled,
        "the measured push-off mean is not above the modelled one: {measured} against {modelled}"
    );
    assert!(
        (measured - modelled).abs() > 1e-6,
        "two velocity terms produced the same number, so the choice moves nothing"
    );
}

/// The size of that assumption across every trial the corpus holds, which is six here
/// because nothing in this rule reads past takeoff.
#[test]
fn the_uniform_acceleration_assumption_is_measured_over_every_committed_trial() {
    let mut ratios = Vec::new();
    for index in 1..=6 {
        let trial = corpus_trial(&format!("subject01_trial{index}"));
        let modelled = rpem_under(&trial, "mean_from_takeoff_velocity")
            .unwrap_or_else(|| panic!("trial {index} produced no modelled figure"));
        let measured = rpem_under(&trial, "mean_over_push_off")
            .unwrap_or_else(|| panic!("trial {index} produced no measured figure"));
        ratios.push((index, measured / modelled));
    }
    println!(
        "measured over modelled, by trial: {:?}",
        ratios
            .iter()
            .map(|(index, ratio)| format!("trial{index} {ratio:.4}"))
            .collect::<Vec<_>>()
    );
    // Six, and the denominator is stated because every other figure in this file is over one.
    assert_eq!(
        ratios.len(),
        6,
        "this rule reads nothing past takeoff, so every committed trial should answer"
    );
    assert!(
        ratios.iter().all(|(_, ratio)| *ratio > 1.0),
        "the measured push-off mean did not exceed the modelled one on every trial: {ratios:?}"
    );
    let widest = ratios
        .iter()
        .fold(1.0f64, |widest, (_, ratio)| widest.max(*ratio));
    assert!(
        widest > 1.05,
        "the widest gap between the two means is {widest}, so the choice between them moves \
         less than one percent and the parameter is decoration"
    );
}

/// Why the push-off index is a construct of its own rather than a third rule beside the two
/// modified indices.
///
/// A request carries one rule per construct. Filed together, a caller reaching for this one
/// would lose the index and get a number three times the size under an unchanged heading.
#[test]
fn the_push_off_index_is_not_the_index_it_would_have_replaced() {
    let trial = subject01_trial1();
    let response = run(
        &trial,
        &with_landmarks(&[
            (RSI_CONSTRUCT, "rsimod.jh_tov_over_ttt", &[]),
            (
                RPEM_CONSTRUCT,
                "index.rpem.riosgallardo",
                &[("velocity_term", "mean_from_takeoff_velocity")],
            ),
        ]),
    )
    .expect("the request is answerable");

    let index = value(&response, RSI_KEY).expect("the modified index");
    let push_off = value(&response, RPEM_KEY).expect("the push-off index");
    println!("subject 01 trial 1: index {index:.4} m/s, push-off index {push_off:.4} m/s");
    assert!(
        push_off / index > 2.0,
        "the push-off index is {push_off} against the index's {index}, a factor of {:.2}, so \
         the two denominators are close enough that one construct could hold both",
        push_off / index
    );
    // Two keys, because two constructs. One key would put an unrelated denominator under a
    // heading a reader takes for the other.
    assert_ne!(RSI_KEY, RPEM_KEY);
}

/// The two numerators the modified index takes are two numbers over one denominator, on the
/// one committed trial that returns to the plate.
///
/// One key and two entry ids, which is what makes them two answers to one question rather
/// than two quantities. The registry records the gap as small on a countermovement jump and
/// 25.7 percent between vendors on a drop jump; this measures the countermovement half.
#[test]
fn the_two_numerators_are_two_numbers_over_one_denominator() {
    let trial = subject01_trial1();
    let mut measured = Vec::new();
    for method_id in ["rsimod.jh_tov_over_ttt", "rsimod.jh_ft_over_ttt"] {
        let response = run(&trial, &with_landmarks(&[(RSI_CONSTRUCT, method_id, &[])]))
            .expect("the request is answerable");
        let value = value(&response, RSI_KEY)
            .unwrap_or_else(|| panic!("{method_id} reported nothing under {RSI_KEY}"));
        measured.push((method_id, value));
    }
    println!("subject 01 trial 1, one trial of the six that returns to the plate: {measured:?}");
    let (_, takeoff_velocity_numerator) = measured[0];
    let (_, flight_time_numerator) = measured[1];
    assert!(
        (flight_time_numerator - takeoff_velocity_numerator).abs() > 1e-6,
        "the two numerators returned the same index, so the entry id on a result says nothing"
    );
    assert!(
        flight_time_numerator > takeoff_velocity_numerator,
        "the flight-time numerator is not the larger of the two, against nine studies putting \
         it 0.021 m high: {flight_time_numerator} against {takeoff_velocity_numerator}"
    );
}

/// A figure nobody attributed is refused rather than filed under no convention.
///
/// The vendor label was the only thing separating the rows this entry was collapsed from, so
/// a result that does not carry one has lost what the collapse existed to keep.
#[test]
fn the_ratio_refuses_a_figure_nobody_attributed() {
    let trial = subject01_trial1();
    let unattributed = run(
        &trial,
        &with_landmarks(&[(RATIO_CONSTRUCT, "ratio.ft_over_ttt.cmj", &[])]),
    )
    .expect("the request is answerable");
    assert_eq!(
        value(&unattributed, RATIO_KEY),
        None,
        "a ratio came back under no vendor convention at all"
    );
    let refusal = declined_because(&unattributed, "ratio.ft_over_ttt.cmj")
        .expect("the rule that reported nothing said why");
    println!("unattributed: {refusal}");
    assert!(
        refusal.contains("vendor_name"),
        "the refusal does not name the parameter that stopped it: {refusal}"
    );

    let attributed = run(
        &trial,
        &with_landmarks(&[(
            RATIO_CONSTRUCT,
            "ratio.ft_over_ttt.cmj",
            &[("vendor_name", "hawkin")],
        )]),
    )
    .expect("the request is answerable");
    let ratio = value(&attributed, RATIO_KEY).expect("a stated convention produces a ratio");
    println!("subject 01 trial 1, attributed: {ratio:.4}");
    assert!(
        ratio > 0.0,
        "an attributed ratio came back at {ratio}, which no flight over a takeoff time is"
    );
}

/// A recording that stops before the settling says how much of it there was, and names the
/// dwell it was measured against.
///
/// Every committed trial fails this rule, and for two different reasons that the refusal
/// tells apart: five never return to the plate at all, and the sixth holds 0.22 s after
/// landing against a one-second dwell. That is a fact about the recordings rather than about
/// the rule, and the recording that would settle it is a landing captured for at least the
/// dwell after force comes back inside the band.
#[test]
fn a_recording_that_ends_before_the_dwell_says_how_much_it_holds() {
    let trial = subject01_trial1();
    let response = run(
        &trial,
        &with_landmarks(&[(STABILISATION_CONSTRUCT, "tts.band_and_dwell.hawkin", &[])]),
    )
    .expect("the request is answerable");

    assert_eq!(
        value(&response, STABILISATION_KEY),
        None,
        "a trace holding a fifth of the dwell reported a settling"
    );
    let refusal = declined_because(&response, "tts.band_and_dwell.hawkin")
        .expect("the rule that reported nothing said why");
    println!("subject 01 trial 1: {refusal}");
    assert!(
        refusal.contains("seconds_recorded_after_landing"),
        "the refusal does not say how much recording there was: {refusal}"
    );
    assert!(
        refusal.contains("dwell_seconds"),
        "the refusal does not name the dwell it fell short of: {refusal}"
    );

    // Shortening the dwell alone changes which of the two shortfalls the rule reports and
    // produces nothing, because this athlete is still landing when the recording stops. The
    // longest run inside the band is zero samples, so the trace never comes back within five
    // percent of system weight at any dwell at all.
    let seconds_at_a_short_dwell = stabilisation_under(&trial, &[("dwell_seconds", 0.02)]);
    let never_entered = declined_because(
        &run(&trial, &stating(&[("dwell_seconds", 0.02)])).expect("the request is answerable"),
        "tts.band_and_dwell.hawkin",
    )
    .expect("the rule that reported nothing said why");
    println!("subject 01 trial 1 at a 0.02 s dwell: {seconds_at_a_short_dwell:?}, {never_entered}");
    assert_eq!(seconds_at_a_short_dwell, None);
    assert!(
        never_entered.contains("longest_quiet_run_seconds = 0"),
        "the trace held a quiet run inside the band and the refusal above blamed the dwell: \
         {never_entered}"
    );

    // The control, and it is what makes both refusals evidence about the recording rather
    // than about the rule. A band this landing does fall inside produces a settling on the
    // same trace through the same request, so the search reached those samples and read them.
    let settled = stabilisation_under(&trial, &[("band_pct", 100.0), ("dwell_seconds", 0.02)]);
    println!("subject 01 trial 1 at a 100 percent band and a 0.02 s dwell: {settled:?}");
    assert!(
        settled.is_some(),
        "no band or dwell this trace can hold produces a settling, so the refusals above say \
         nothing about the recording"
    );
}

/// One request naming the stabilisation rule, carrying the numbers a probe states on it.
fn stating(parameters: &[(&str, f64)]) -> AnalysisRequest {
    let mut request =
        with_landmarks(&[(STABILISATION_CONSTRUCT, "tts.band_and_dwell.hawkin", &[])]);
    let choice = request
        .derived
        .get_mut(STABILISATION_CONSTRUCT)
        .expect("the request names the rule");
    for (name, value) in parameters {
        choice.parameters.insert((*name).to_string(), *value);
    }
    request
}

fn stabilisation_under(trial: &Trial, parameters: &[(&str, f64)]) -> Option<f64> {
    let response = run(trial, &stating(parameters)).expect("the request is answerable");
    value(&response, STABILISATION_KEY)
}
