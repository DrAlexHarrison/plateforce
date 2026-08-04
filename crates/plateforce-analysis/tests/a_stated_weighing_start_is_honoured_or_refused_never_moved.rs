//! What happens to a weighing window a caller placed where no window fits.
//!
//! The weighing epoch sets system weight, the onset band, every threshold scaled by the quiet
//! spread, and the search floor for three of the five onset rules and two of the five takeoff
//! rules. A window moved from where the caller put it therefore moves five landmark rules by
//! its end and every impulse by its level, and on this recording moving it to the last two
//! samples reads system weight 48 percent high, because those samples are landing impact.
//!
//! Two properties, on one real recording, with only the stated start moving between runs. A
//! stated start is read where it was stated or refused, never reshaped into one that fits.
//! And the refusal names the start the caller stated and the recording the caller holds,
//! rather than the shifted frame the arithmetic ran in.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Refusal, RefusalCode, Trial};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

/// The founding corpus samples at 1200 Hz. Read at 1000 every index and second below moves.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;
const TRIAL_SAMPLES: usize = 6000;
const TRIAL_DURATION_SECONDS: f64 = 5.0;

/// A start one sample past the last, and a start sixteen times the recording's length. Both
/// used to be moved to sample 5998, and neither said so.
const ONE_PAST_THE_END: usize = TRIAL_SAMPLES;
const FAR_PAST_THE_END: usize = 100_000;

/// The window this recording's quiet stance holds, and the window short enough to have fitted
/// in the two samples a moved start left behind. The second is what made the move produce a
/// number rather than a refusal, so it is the case that has to be reachable here.
const WINDOW_THAT_FITS_SECONDS: f64 = 1.0;
const TWO_SAMPLE_WINDOW_SECONDS: f64 = 0.002;

/// System weight over the quiet second at sample 1200, and over the two samples that end the
/// recording. The second is the landing impact, and the distance between them is what a moved
/// window costs every quantity below.
const QUIET_STANCE_NEWTONS: f64 = 585.879;
const LAST_TWO_SAMPLES_NEWTONS: f64 = 870.93085;

/// The three weighing rules this build ships, each with the registry's own name for the
/// window's length on its row. A stated start reaches one placement for all three: the
/// searching rule searches only where nobody has said the window goes, so a start states it
/// onto the placed path beside the other two.
const WEIGHING_RULES: &[(&str, &str)] = &[
    ("bwepoch.fixed_window", "duration"),
    ("bwepoch.manual_placement", "span_seconds"),
    ("bwepoch.adaptive_lowest_variance", "window_seconds"),
];

fn trial() -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{FIXTURE} did not read: {error}"));
    trial
}

fn request(
    weighing_id: &str,
    window_length_parameter: &str,
    start_index: usize,
    window_seconds: f64,
) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: weighing_id.into(),
            start_index: Some(start_index),
            parameters: BTreeMap::from([(window_length_parameter.to_string(), window_seconds)]),
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

fn placed(trial: &Trial, start_index: usize, window_seconds: f64) -> AnalysisResponse {
    let (id, parameter) = WEIGHING_RULES[0];
    run(trial, &request(id, parameter, start_index, window_seconds))
        .unwrap_or_else(|refusal| panic!("start {start_index} was refused: {refusal}"))
}

fn declined(trial: &Trial, start_index: usize, window_seconds: f64) -> Refusal {
    let (id, parameter) = WEIGHING_RULES[0];
    *run(trial, &request(id, parameter, start_index, window_seconds))
        .expect_err(&format!("start {start_index} produced a number"))
}

/// Every number a reader could act on, asserted against the start the caller stated.
fn names_the_callers_own_numbers(trial: &Trial, refusal: &Refusal, start_index: usize) {
    assert_eq!(refusal.code, RefusalCode::TraceTooShort);
    assert_eq!(
        refusal.detail.get("start_seconds"),
        Some(&trial.time_at(start_index)),
        "the refusal names a start the caller did not state"
    );
    assert_eq!(
        refusal.detail.get("available_seconds"),
        Some(&TRIAL_DURATION_SECONDS),
        "the refusal names a recording the caller does not have"
    );
    // The sentence and the fields are read by different callers, and both are the record.
    assert!(
        refusal
            .message()
            .contains(&format!("{}", trial.time_at(start_index))),
        "{}",
        refusal.message()
    );
}

#[test]
fn a_start_past_the_end_is_refused_rather_than_moved_to_where_a_window_fits() {
    let trial = trial();
    assert_eq!(trial.len(), TRIAL_SAMPLES, "the fixture changed length");
    let the_start_a_moved_window_lands_on = trial.time_at(trial.len() - 2);

    for start_index in [ONE_PAST_THE_END, FAR_PAST_THE_END] {
        // The short window is the case that used to come back as a number: two samples fit in
        // what a moved start left, so nothing downstream refused and nothing said anything.
        for window_seconds in [WINDOW_THAT_FITS_SECONDS, TWO_SAMPLE_WINDOW_SECONDS] {
            let refusal = declined(&trial, start_index, window_seconds);
            println!(
                "start {start_index}, window {window_seconds} s: {}",
                refusal.message()
            );

            names_the_callers_own_numbers(&trial, &refusal, start_index);
            assert_eq!(
                refusal.detail.get("requested_seconds"),
                Some(&window_seconds)
            );
            assert_ne!(
                refusal.detail.get("start_seconds"),
                Some(&the_start_a_moved_window_lands_on),
                "the refusal reports the start the window was moved to"
            );
        }
    }
}

/// The boundary from both sides, at the sample every over-far start used to be moved to.
///
/// Sample 5998 is not a forbidden place to weigh. It is where the last two samples of this
/// recording are, and a caller who states it is answered with what they hold there. What was
/// wrong was arriving at it without being asked, which is why the assertion is a pair: the
/// last start a window fits at is read, and the next one along is refused, under the same
/// window on the same trace.
#[test]
fn the_last_start_a_window_fits_at_is_read_and_the_next_one_along_is_refused() {
    let trial = trial();
    let last_start_that_fits = trial.len() - 2;

    let response = placed(&trial, last_start_that_fits, TWO_SAMPLE_WINDOW_SECONDS);
    assert_eq!(response.weighing_start_index, last_start_that_fits);
    assert_eq!(response.weighing_end_index, trial.len());
    let system_weight_newtons = response
        .levels
        .system_weight_newtons
        .expect("a window over real samples has a weight");
    println!("start {last_start_that_fits}: {system_weight_newtons:.5} N");
    assert!(
        (system_weight_newtons - LAST_TWO_SAMPLES_NEWTONS).abs() < 0.001,
        "{system_weight_newtons}"
    );

    let refusal = declined(&trial, last_start_that_fits + 1, TWO_SAMPLE_WINDOW_SECONDS);
    println!("start {}: {}", last_start_that_fits + 1, refusal.message());
    names_the_callers_own_numbers(&trial, &refusal, last_start_that_fits + 1);
}

/// The half that fires far from the end of the recording. A stated start with four fifths of
/// the trace in front of it, and a window longer than what remains, used to be refused in the
/// arithmetic's own frame: start 0 s, recording 0.4167 s, neither of them the caller's.
#[test]
fn a_window_that_overruns_the_end_is_refused_in_the_recording_the_caller_has() {
    let trial = trial();
    let start_index = 5500;
    let refusal = declined(&trial, start_index, WINDOW_THAT_FITS_SECONDS);
    println!("start {start_index}: {}", refusal.message());
    names_the_callers_own_numbers(&trial, &refusal, start_index);
}

/// Which rule declined and which construct it filled. A weighing refusal is the whole analysis
/// declining rather than a row in `refusals`, so nothing downstream names it, and all three
/// weighing rules reach one placement whenever a start is stated.
#[test]
fn every_weighing_rule_refuses_a_stated_start_under_its_own_name() {
    let trial = trial();
    let mut refused: Vec<&str> = Vec::new();

    for (weighing_id, window_length_parameter) in WEIGHING_RULES {
        let refusal = *run(
            &trial,
            &request(
                weighing_id,
                window_length_parameter,
                FAR_PAST_THE_END,
                WINDOW_THAT_FITS_SECONDS,
            ),
        )
        .expect_err("a start sixteen times the recording produced a number");
        println!("{weighing_id}: {}", refusal.message());

        assert_eq!(refusal.method_id, *weighing_id);
        assert_eq!(refusal.slot.as_deref(), Some("system_weight"));
        names_the_callers_own_numbers(&trial, &refusal, FAR_PAST_THE_END);
        refused.push(weighing_id);
    }

    assert_eq!(refused.len(), WEIGHING_RULES.len());
}

/// The control. Every assertion above is satisfied by software that refuses everything, and
/// this recording holds a quiet second at sample 1200 that a stated start must still read.
#[test]
fn a_stated_start_a_window_fits_at_is_read_where_it_was_stated() {
    let trial = trial();
    let start_index = 1200;
    let response = placed(&trial, start_index, WINDOW_THAT_FITS_SECONDS);

    assert_eq!(response.weighing_start_index, start_index);
    assert_eq!(
        response.weighing_end_index,
        start_index + (WINDOW_THAT_FITS_SECONDS * CORPUS_SAMPLE_RATE_HZ) as usize
    );
    // Quiet stance, not the landing impact a moved window reads. The two differ by 285 N here.
    let system_weight_newtons = response
        .levels
        .system_weight_newtons
        .expect("a window over real samples has a weight");
    println!("system weight {system_weight_newtons:.3} N");
    assert!(
        (system_weight_newtons - QUIET_STANCE_NEWTONS).abs() < 0.001,
        "{system_weight_newtons}"
    );
}
