//! A rule whose search interval the recording did not supply declines, rather than reporting a
//! quantity with no value and no reason.
//!
//! Three returns produced that silence, and a reader met each of them as a row with the rule
//! column and the account column both empty. Which is worse than a row that is not there: a
//! reader filtering the table receives it and learns nothing.
//!
//! What each of them is, measured on this tree rather than argued for. Sweeping
//! `phase.braking_start.min_force` over the three weighing rules by the five onset rules by the
//! five takeoff rules on each of the nine committed recordings, 675 analyses, **27** came back
//! silent, in two conditions that take different repairs:
//!
//! | condition | recording | count |
//! |---|---|---|
//! | the two landmarks enclose no samples | `synthetic_untrimmed_step_off_after_jump` | 12 of 675 |
//! | the propulsive peak stands at the onset | both synthetic recordings | 15 of 675 |
//!
//! The third was `jumpheight.dj.mcmahon_correction_factor`, and what looked like a third silence
//! was the visible edge of an arithmetic fault. Its standing period was the weighing window plus
//! one sample, so the height rested on a sample outside the window the record names and the rule
//! ran out of series on a window ending at the recording's last sample. The period is the
//! window's own half-open span now, and the two tests below hold the number to it: it moves with
//! every sample the window holds and with none outside it, and the whole standing period is a
//! window the rule answers.
//!
//! The corpus holds no drop jump, so the drop-jump case is built here as it is in
//! `a_drop_jump_height_names_the_landing_it_rested_on`. A population of countermovement jumps
//! cannot reach a rule that integrates from an arrival before takeoff, and would report the
//! whole class clean.

use std::collections::BTreeMap;

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{
    accounts_of, chain_names, run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice,
};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::{
    read_trial_from_path, Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED as GRAVITY,
};

mod common;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/"
);
const INTERRUPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt"
);
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

const NADIR: &str = "phase.braking_start.min_force";
const NADIR_KEY: &str = "braking_phase_start_seconds";
const MCMAHON: &str = "jumpheight.dj.mcmahon_correction_factor";
const HEIGHT_KEY: &str = "jump_height_from_takeoff_meters";

/// The onset rule that reads the step-off as the start of the jump, which is what puts the two
/// landmarks in an order the recording did not happen in.
const INVERTING_ONSET: &str = "onset.threshold.adaptive_trailing_window";

fn trial_at(path: &str) -> Trial {
    let (trial, _) = read_trial_from_path(path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

fn fixture(name: &str) -> Trial {
    trial_at(&format!("{FIXTURES}{name}.force.txt"))
}

/// Every recording this repository commits, the damaged one beside them.
fn every_committed_recording() -> Vec<(String, Trial)> {
    let mut recordings: Vec<(String, Trial)> = (1..=6)
        .map(|number| {
            let name = format!("subject01_trial{number}");
            let trial = fixture(&name);
            (name, trial)
        })
        .collect();
    for name in [
        "synthetic_untrimmed_step_off",
        "synthetic_untrimmed_step_off_after_jump",
    ] {
        recordings.push((name.to_string(), fixture(name)));
    }
    recordings.push((
        "subject01_trial1_interrupted".to_string(),
        trial_at(INTERRUPTED),
    ));
    recordings
}

fn rules_for(construct: &str) -> Vec<String> {
    plateforce_analysis::BINDINGS
        .iter()
        .filter(|binding| binding.construct == construct)
        .map(|binding| binding.id.to_string())
        .collect()
}

fn spine_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([
                ("duration".to_string(), 1.0),
                ("window_seconds".to_string(), 1.0),
                ("span_seconds".to_string(), 1.0),
            ]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn binding(construct: &str, method_id: &str) -> AnalysisRequest {
    let mut request = spine_request();
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            ..Default::default()
        },
    );
    request
}

fn analysed(trial: &Trial, request: AnalysisRequest) -> AnalysisResponse {
    run(trial, &common::prepared(request))
        .unwrap_or_else(|refusal| panic!("the request ran: {refusal}"))
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// The sentence the named rule wrote on this response, refused rather than defaulted so a rule
/// that did not decline cannot read as one that declined saying nothing.
fn sentence_from(response: &AnalysisResponse, method_id: &str) -> String {
    let declined = response
        .refusals
        .iter()
        .find(|declined| declined.method_id == method_id)
        .unwrap_or_else(|| panic!("{method_id} did not decline: {:#?}", response.refusals));
    refusal_from_rule(declined).message().to_string()
}

/// The account a reader meets against one quantity, which has to be the sentence its own
/// producer wrote and nothing built from it.
fn account_is_the_rules_own(response: &AnalysisResponse, key: &str, method_id: &str) -> String {
    let metric = response
        .metric(key)
        .unwrap_or_else(|| panic!("{key} is absent from the response the caller asked for it in"));
    assert!(metric.value.is_none(), "{key} answers: {:?}", metric.value);
    assert!(
        chain_names(metric, method_id),
        "{method_id} is not on the chain of {key}: {:?} {:?}",
        metric.contributing_method_ids,
        metric.computed_by
    );

    let sentence = sentence_from(response, method_id);
    let block = accounts_of(response, &stamp(), false);
    let account = block
        .get(key)
        .unwrap_or_else(|| panic!("the block holds no entry for {key}"));
    assert_eq!(
        account, &sentence,
        "the account of {key} is not the refusal's own sentence, so something composed one"
    );
    sentence
}

/// The propulsive peak stands at the onset, so the nadir search is handed no samples.
///
/// The landmarks are in order on this recording, which is what makes this the second of the two
/// conditions rather than the first: takeoff sits after onset and the peak between them is the
/// onset itself.
#[test]
fn the_braking_nadir_declines_where_the_peak_it_searches_to_stands_at_the_onset() {
    let mut request = binding("braking_phase_start", NADIR);
    request.onset.method_id = INVERTING_ONSET.to_string();
    let response = analysed(&fixture("synthetic_untrimmed_step_off"), request);

    let (Some(onset), Some(takeoff)) = (response.onset_index, response.takeoff_index) else {
        panic!("both landmarks place on this recording, or this is the other condition");
    };
    assert!(
        takeoff > onset,
        "the landmarks came back out of order, so this is the condition the sibling guard covers"
    );

    let sentence = account_is_the_rules_own(&response, NADIR_KEY, NADIR);
    println!("onset {onset}, takeoff {takeoff}: {sentence}");
    assert!(
        sentence.contains(&format!("samples {onset} to {onset}")),
        "the refusal names an interval other than the one the search was handed: {sentence}"
    );
}

/// The two landmarks enclose no samples, so no peak can be taken between them.
///
/// The onset rule reads the step off the plate as the start of the jump, so it lands after the
/// takeoff. `ORIENTATION.md` records this recording as the one built for landmarks out of order.
#[test]
fn the_braking_nadir_declines_where_its_two_landmarks_enclose_no_samples() {
    let mut request = binding("braking_phase_start", NADIR);
    request.onset.method_id = INVERTING_ONSET.to_string();
    let response = analysed(&fixture("synthetic_untrimmed_step_off_after_jump"), request);

    let (Some(onset), Some(takeoff)) = (response.onset_index, response.takeoff_index) else {
        panic!("both landmarks place on this recording");
    };
    assert!(
        onset > takeoff,
        "the landmarks are in order now, so this is no longer the condition this guard is about"
    );

    let sentence = account_is_the_rules_own(&response, NADIR_KEY, NADIR);
    println!("onset {onset}, takeoff {takeoff}: {sentence}");
    assert!(
        sentence.contains(&format!("samples {onset} to {takeoff}")),
        "the refusal names an interval other than the one the two landmarks enclose: {sentence}"
    );
}

/// The control for both guards above: the recording where the rule answers.
///
/// Without it a build declining on every recording satisfies the two guards completely, and a
/// reader would meet a reason under a number sitting in front of them.
#[test]
fn the_braking_nadir_answers_on_the_recording_it_can_read_and_writes_no_reason_there() {
    let response = analysed(
        &fixture("subject01_trial1"),
        binding("braking_phase_start", NADIR),
    );
    let metric = response
        .metric(NADIR_KEY)
        .expect("the quantity is reported");
    assert!(
        metric.value.is_some(),
        "the rule no longer answers on this recording, so the pair proves nothing"
    );
    assert!(
        !response
            .refusals
            .iter()
            .any(|declined| declined.method_id == NADIR),
        "the rule declined on the recording it can read: {:#?}",
        response.refusals
    );
    println!(
        "{NADIR_KEY} reads {:?} s and carries no reason",
        metric.value
    );
}

// ------------------------------------------------ the drop jump the corpus does not hold

const DROP_RATE_HZ: f64 = 1000.0;
const MASS_KILOGRAMS: f64 = 70.0;
const ARRIVAL_VELOCITY: f64 = -2.31;
const TAKEOFF_VELOCITY: f64 = 2.60;
const ON_THE_BOX_SAMPLES: usize = 250;
const CONTACT_SAMPLES: usize = 250;
const LANDING_SAMPLES: usize = 250;
const STANDING_SAMPLES: usize = 1200;

/// A drop jump written as the force each phase needs to be worth the velocity change it makes,
/// ending in the standing period this method reads its velocity reference over.
fn synthetic_drop_jump() -> Trial {
    let weight = MASS_KILOGRAMS * GRAVITY;
    let interval = 1.0 / DROP_RATE_HZ;
    let flight_samples = ((2.0 * TAKEOFF_VELOCITY / GRAVITY) * DROP_RATE_HZ).round() as usize;
    let net_newtons = |velocity_change: f64, samples: usize| {
        MASS_KILOGRAMS * velocity_change / (samples as f64 * interval)
    };
    let contact = weight + net_newtons(TAKEOFF_VELOCITY - ARRIVAL_VELOCITY, CONTACT_SAMPLES);
    let landing = weight + net_newtons(TAKEOFF_VELOCITY, LANDING_SAMPLES);

    let mut force = Vec::new();
    force.extend(std::iter::repeat_n(0.0, ON_THE_BOX_SAMPLES));
    force.extend(std::iter::repeat_n(contact, CONTACT_SAMPLES));
    force.extend(std::iter::repeat_n(0.0, flight_samples));
    force.extend(std::iter::repeat_n(landing, LANDING_SAMPLES));
    force.extend(std::iter::repeat_n(weight, STANDING_SAMPLES));
    Trial::new(force, DROP_RATE_HZ).expect("the assembled drop jump is a trial")
}

/// The drop jump weighed over a window of `samples`, anchored where the athlete comes to rest.
fn drop_jump_weighed_over(trial: &Trial, samples: usize) -> AnalysisResponse {
    let mut request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: Some(trial.len() - STANDING_SAMPLES),
            parameters: BTreeMap::from([("duration".to_string(), samples as f64 / DROP_RATE_HZ)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.longest_run".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    for (construct, method_id) in [
        ("landing", "landing.threshold.absolute_force"),
        ("jump_height.takeoff_frame", MCMAHON),
    ] {
        request.derived.insert(
            construct.to_string(),
            MethodChoice {
                method_id: method_id.to_string(),
                ..Default::default()
            },
        );
    }
    analysed(trial, request)
}

/// A standing period that wanders, which is every recording a plate actually produces.
///
/// On a period that is exactly flat every sample equals the mean, so widening or narrowing the
/// span by one changes nothing and no assertion about the span can fail. The sway is what makes
/// the span observable: 1.4 N is one converter step on the plate this corpus came from.
fn drop_jump_with_a_standing_period_that_wanders() -> Trial {
    let flat = synthetic_drop_jump();
    let standing_start = flat.len() - STANDING_SAMPLES;
    let mut force = flat.force().to_vec();
    for (offset, sample) in force[standing_start..].iter_mut().enumerate() {
        let seconds = offset as f64 / DROP_RATE_HZ;
        *sample += 5.0
            * ((2.0 * std::f64::consts::PI * 0.37 * seconds).sin()
                + 0.4 * (2.0 * std::f64::consts::PI * 7.3 * seconds).sin());
    }
    Trial::new(force, DROP_RATE_HZ).expect("the swaying drop jump is a trial")
}

fn height_over(trial: &Trial, window_samples: usize) -> f64 {
    drop_jump_weighed_over(trial, window_samples)
        .metric(HEIGHT_KEY)
        .expect("the quantity is reported")
        .value
        .expect("the rule answers on this window")
}

/// The height rests on no sample outside the weighing window the record names.
///
/// Two recordings identical everywhere except the single sample at the window's exclusive end,
/// which is the first sample the window does not hold. A period taken a sample wider reads it
/// and reports a different height, while the record names the same window both times, so the
/// number would move under a name that cannot account for it.
///
/// The span is pinned by value as well, on the swaying trace, because the assertion above is
/// one-sided: it catches a period reaching past the window and says nothing about one stopping
/// short. Narrowing the span leaves the sample at the end untouched and passes the first half.
/// The perturbation control cannot close that gap, because a sample inside the window moves the
/// system weight too, so it moves the height whether the standing period reads it or not.
#[test]
fn the_drop_jump_height_reads_the_weighing_window_and_no_sample_outside_it() {
    let window_samples = STANDING_SAMPLES - 200;
    let swaying = drop_jump_with_a_standing_period_that_wanders();
    let end_index = swaying.len() - STANDING_SAMPLES + window_samples;

    let untouched = height_over(&swaying, window_samples);
    let moved_outside = {
        let mut force = swaying.force().to_vec();
        force[end_index] += 90.0;
        let trial = Trial::new(force, DROP_RATE_HZ).expect("the perturbed drop jump is a trial");
        let response = drop_jump_weighed_over(&trial, window_samples);
        assert_eq!(response.weighing_end_index, end_index);
        response
            .metric(HEIGHT_KEY)
            .expect("the quantity is reported")
            .value
            .expect("the rule answers on this window")
    };

    println!("window ends at {end_index}: {untouched:.10} m, sample {end_index} moved {moved_outside:.10} m");
    assert_eq!(
        untouched, moved_outside,
        "the height moved with the sample at {end_index}, which the weighing window does not \
         hold, so it rests on a sample the record does not name"
    );

    // The measured value of the span itself, so a period that stops short reddens here even
    // though it leaves the sample at the end untouched and passes the assertion above. Seven
    // places rather than the ten the run prints: a span one sample narrower reads 0.3510861,
    // which differs in the sixth, so the digits below that carry no signal and would only pin
    // this to the machine that measured it.
    assert_eq!(
        format!("{untouched:.7}"),
        "0.3510849",
        "the height over this window changed, so the span the velocity is averaged over moved"
    );
}

/// A window ending at the recording's last sample is one the rule answers.
///
/// The paired half of the property above. A period taken as the window plus one sample runs off
/// the end of a recording that ends exactly where the window does, and the rule declined on a
/// request that is the ordinary way to weigh a drop jump: over the whole standing period the
/// athlete holds at the end.
#[test]
fn a_weighing_window_ending_at_the_last_sample_of_the_recording_is_answered() {
    let trial = synthetic_drop_jump();
    let response = drop_jump_weighed_over(&trial, STANDING_SAMPLES);
    assert_eq!(
        response.weighing_end_index,
        trial.len(),
        "the window no longer ends at the recording's last sample, so this is not the case"
    );
    assert!(
        !response
            .refusals
            .iter()
            .any(|declined| declined.method_id == MCMAHON),
        "the rule declined on a window that sits inside the recording: {:#?}",
        response.refusals
    );
    let answered = response
        .metric(HEIGHT_KEY)
        .expect("the quantity is reported")
        .value
        .expect("the rule answers over the whole standing period");
    println!("the whole standing period reads {answered:.10} m");

    // The control: one sample shorter answers too, so the assertion above is about the window
    // reaching the end rather than about the rule answering whatever it is given.
    let one_shorter = drop_jump_weighed_over(&trial, STANDING_SAMPLES - 1);
    assert_eq!(one_shorter.weighing_end_index, trial.len() - 1);
    assert!(one_shorter
        .metric(HEIGHT_KEY)
        .expect("the quantity is reported")
        .value
        .is_some());
}

// ------------------------------------------------------------------ the swept population

/// Whether a quantity came back with no number, no refusal on its chain, no signal naming it,
/// and no non-number recorded: the state this file exists to empty.
fn silent(response: &AnalysisResponse, key: &str) -> bool {
    let Some(metric) = response.metric(key) else {
        return false;
    };
    metric.value.is_none()
        && !metric.carried_no_number
        && !response
            .refusals
            .iter()
            .any(|declined| chain_names(metric, &declined.method_id))
        && !response
            .signals
            .iter()
            .any(|signal| signal.qualifies.iter().any(|named| named == key))
}

/// No combination this build can be given leaves the braking nadir silent.
///
/// The denominator is read off the binding table, so a rule added to the build widens this
/// sweep on the day it is added, and the count of quantities that came back without a number is
/// printed beside the count of silent ones: zero silent out of zero absent is what a build that
/// answered everything reports, and it would satisfy this guard phrased any other way.
#[test]
fn no_combination_leaves_the_braking_nadir_with_neither_a_number_nor_a_reason() {
    let mut ran = 0usize;
    let mut absent = 0usize;
    let mut unexplained: Vec<String> = Vec::new();

    for (name, trial) in every_committed_recording() {
        for weighing in rules_for(plateforce_analysis::WEIGHING_CONSTRUCT) {
            for onset in rules_for(plateforce_analysis::ONSET_CONSTRUCT) {
                for takeoff in rules_for(plateforce_analysis::TAKEOFF_CONSTRUCT) {
                    let mut request = binding("braking_phase_start", NADIR);
                    request.weighing.method_id = weighing.clone();
                    request.onset.method_id = onset.clone();
                    request.takeoff.method_id = takeoff.clone();
                    let Ok(response) = run(&trial, &common::prepared(request)) else {
                        continue;
                    };
                    ran += 1;
                    let metric = response
                        .metric(NADIR_KEY)
                        .expect("the caller bound the rule, so the quantity is reported");
                    if metric.value.is_none() {
                        absent += 1;
                    }
                    if silent(&response, NADIR_KEY) {
                        unexplained.push(format!("{name} | {weighing} | {onset} | {takeoff}"));
                    }
                }
            }
        }
    }

    println!(
        "{ran} analyses, {absent} with no number, {} of those with no reason",
        unexplained.len()
    );
    assert_eq!(ran, 675, "the binding table changed shape");
    assert!(
        absent >= 27,
        "only {absent} combinations leave the quantity without a number, and 27 of them used to \
         come back silent, so this population no longer reaches the case"
    );
    assert!(
        unexplained.is_empty(),
        "{} of {absent} report no value and no reason: {unexplained:?}",
        unexplained.len()
    );
}

/// The control for the sweep, and the one that has to discriminate.
///
/// A predicate that could not see silence reports every population clean, and clean is what the
/// guard above is looking for. `phase.propulsion_start.zero_velocity` on the step-off recording
/// is still in that state on purpose: its search runs the interval it was given and finds no
/// crossing in it, which is a rule reporting no such instant rather than a rule handed no
/// interval. So the same predicate reads it silent here, and the sweep above is a measurement.
#[test]
fn the_predicate_still_reads_a_rule_that_searched_and_found_nothing_as_silent() {
    let response = analysed(
        &fixture("synthetic_untrimmed_step_off"),
        binding(
            "propulsion_phase_start",
            "phase.propulsion_start.zero_velocity",
        ),
    );
    assert!(
        silent(&response, "propulsion_phase_start_seconds"),
        "the predicate no longer reads any quantity as silent, so the sweep proves nothing"
    );
    println!("propulsion_phase_start_seconds is still silent, so the predicate discriminates");
}
