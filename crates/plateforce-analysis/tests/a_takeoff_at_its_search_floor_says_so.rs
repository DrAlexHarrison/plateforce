//! What a takeoff rule that returned its own boundary has to say about it.
//!
//! Two of the five published takeoff rules begin searching where the weighing window ended.
//! Where that window ends inside a stretch of low force, the first sample they are permitted
//! to examine already satisfies them, so the takeoff they report is the boundary of their own
//! search rather than a flight phase found in the recording.
//!
//! The property is not that a signal exists. It is that the two rules which returned their
//! floor say so and the three which searched the whole recording stay quiet, on one recording,
//! under one weighing window, with only the takeoff rule changing between runs.

use std::collections::BTreeMap;

use plateforce_analysis::quality::{QualitySignal, QualityStatus};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

/// The founding corpus samples at 1200 Hz. Read at 1000 every landmark index below moves.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The weighing window this whole file runs under. 4.4 s at this rate ends the window at
/// sample 5280, which sits inside the flight phase of this jump, and that is what puts the
/// two flooring rules at their own boundary.
const WEIGHING_WINDOW_SECONDS: f64 = 4.4;
const EXPECTED_FLOOR_INDEX: usize = 5280;

/// Where the three rules that search the whole recording place takeoff on this trial, which
/// is 266 samples before the floor. Named so the guard states what the flooring rules missed
/// rather than only that they differed.
const TAKEOFF_FOUND_BY_A_WHOLE_RECORDING_SEARCH: usize = 5014;

/// The onset rule held fixed across every run below. It reads its own trailing window rather
/// than the weighing epoch, so it records no onset search floor and raises no floor signal of
/// its own, which keeps every `AtSearchFloor` signal in this file a takeoff one.
fn onset_choice() -> MethodChoice {
    MethodChoice {
        method_id: "onset.threshold.adaptive_trailing_window".into(),
        parameters: BTreeMap::from([("k".to_string(), 5.0), ("window_seconds".to_string(), 1.0)]),
        ..Default::default()
    }
}

/// The five takeoff rules this build ships.
fn takeoff_rules() -> Vec<&'static str> {
    vec![
        "takeoff.threshold.absolute_force",
        "takeoff.threshold.flight_noise_k_sd",
        "takeoff.threshold.longest_run",
        "takeoff.threshold.descending_crossing",
        "takeoff.threshold.landing_shape",
    ]
}

/// The two rules that begin searching where the weighing window ends, so on this trial the
/// first sample they may examine is already inside the flight phase.
const RULES_THAT_RETURN_THEIR_FLOOR: &[&str] = &[
    "takeoff.threshold.absolute_force",
    "takeoff.threshold.flight_noise_k_sd",
];

fn trial() -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{FIXTURE} did not read: {error}"));
    trial
}

fn analyse(trial: &Trial, takeoff_id: &str) -> AnalysisResponse {
    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), WEIGHING_WINDOW_SECONDS)]),
            ..Default::default()
        },
        onset: onset_choice(),
        takeoff: MethodChoice {
            method_id: takeoff_id.to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    run(trial, &request).unwrap_or_else(|refusal| panic!("{takeoff_id} did not run: {refusal}"))
}

fn floor_signals(response: &AnalysisResponse) -> Vec<&QualitySignal> {
    response
        .signals
        .iter()
        .filter(|signal| signal.status == QualityStatus::AtSearchFloor)
        .collect()
}

/// A recorded value read back at the precision the rule recorded it.
fn recorded(response: &AnalysisResponse, method_id: &str, name: &str) -> Option<f64> {
    response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == method_id)
        .and_then(|bound| bound.numeric_values.get(name).copied())
}

/// Both sides of the same run. A guard that only asserted the two rules speak would pass on a
/// signal attached to every result, and one that only asserted the three stay quiet would pass
/// on a signal that never fires at all.
#[test]
fn the_rules_that_returned_their_floor_say_so_and_the_rules_that_searched_on_do_not() {
    let trial = trial();
    let mut spoke: Vec<&str> = Vec::new();
    let mut quiet: Vec<&str> = Vec::new();

    for takeoff_id in takeoff_rules() {
        let response = analyse(&trial, takeoff_id);
        let raised = floor_signals(&response);
        println!(
            "{takeoff_id}: takeoff index {:?}, floor-landing signals {}",
            response.takeoff_index,
            raised.len()
        );
        if raised.is_empty() {
            quiet.push(takeoff_id);
        } else {
            assert_eq!(raised.len(), 1, "{takeoff_id} raised the same signal twice");
            spoke.push(takeoff_id);
        }
    }

    assert_eq!(
        spoke, RULES_THAT_RETURN_THEIR_FLOOR,
        "the rules that returned their own boundary are not the ones that said so"
    );
    assert_eq!(
        quiet.len(),
        3,
        "3 of the 5 rules search the whole recording and find a takeoff before the floor: {quiet:?}"
    );
}

/// The arithmetic, stated in samples rather than in seconds, because two times agreeing to
/// four decimals could be two numbers that round together and two integer indices cannot.
#[test]
fn the_signal_fires_on_an_index_equal_to_the_recorded_floor() {
    let trial = trial();

    for takeoff_id in RULES_THAT_RETURN_THEIR_FLOOR {
        let response = analyse(&trial, takeoff_id);
        assert_eq!(
            response.weighing_end_index, EXPECTED_FLOOR_INDEX,
            "{takeoff_id} ran under a different weighing window than this guard describes"
        );
        assert_eq!(
            response.takeoff_index,
            Some(EXPECTED_FLOOR_INDEX),
            "{takeoff_id} did not return its floor, so it is the wrong rule for this assertion"
        );

        let recorded_floor_seconds = recorded(
            &response,
            "takeoff.op.search_floor_at_weighing_epoch_end",
            "weighing_epoch_end_seconds",
        )
        .unwrap_or_else(|| panic!("{takeoff_id} recorded no search floor"));
        assert_eq!(
            recorded_floor_seconds,
            trial.time_at(EXPECTED_FLOOR_INDEX),
            "{takeoff_id} recorded a floor that is not the index its search was given"
        );
    }
}

/// Three surfaces print a signal holding no value as "not comparable", which is a false
/// sentence about this one. So it carries the instant the search begins against the start of
/// the recording, and the gap between them is the span the rule was forbidden to examine.
#[test]
fn the_signal_carries_a_value_every_surface_can_render_truthfully() {
    let trial = trial();
    let response = analyse(&trial, RULES_THAT_RETURN_THEIR_FLOOR[0]);
    let signals = floor_signals(&response);
    let signal = signals
        .first()
        .expect("the flooring rule raised its signal");

    let value = signal.value.expect("a rendered sentence needs a number");
    assert!(
        value > signal.threshold,
        "the sentence reads '{value} {} past {}', which has to stay true",
        signal.unit,
        signal.threshold
    );
    assert_eq!(signal.unit, "seconds");
    assert_eq!(value, trial.time_at(EXPECTED_FLOOR_INDEX));
    assert_eq!(signal.remedy_construct, "takeoff");
    assert!(
        !signal.remedy.is_empty(),
        "a signal without an action leaves the reader holding a diagnosis"
    );
}

/// The keys the signal places itself beside, and the sentence's own claim that a reader can
/// act on it. Every key named has a value on this trial, so the signal qualifies numbers
/// rather than accounting for absences, which is a different signal's job.
#[test]
fn the_signal_names_the_quantities_the_takeoff_index_defines() {
    let trial = trial();
    let response = analyse(&trial, RULES_THAT_RETURN_THEIR_FLOOR[0]);
    let signals = floor_signals(&response);
    let signal = signals
        .first()
        .expect("the flooring rule raised its signal");

    assert_eq!(
        signal.qualifies,
        vec![
            "takeoff_time_seconds",
            "time_to_takeoff_seconds",
            "flight_time_seconds"
        ],
        "the keys a surface places this signal beside"
    );
    for key in &signal.qualifies {
        let entry = response
            .metric(key)
            .unwrap_or_else(|| panic!("{key} is not a key this response carries"));
        assert!(
            entry.value.is_some(),
            "{key} has no value, so this signal is qualifying an absence"
        );
    }
}

/// A rule that returns its own boundary has still done what it publishes, and the distance
/// between it and the rules that searched on is the disagreement between published methods.
/// A spread that dropped it would report those methods as closer together than they are.
#[test]
fn a_takeoff_at_its_search_floor_is_not_distrusted_and_the_gap_is_why() {
    let trial = trial();
    let floored = analyse(&trial, RULES_THAT_RETURN_THEIR_FLOOR[0]);
    let searched_on = analyse(&trial, "takeoff.threshold.landing_shape");

    let floor_only: Vec<QualitySignal> = floored
        .signals
        .iter()
        .filter(|signal| signal.status == QualityStatus::AtSearchFloor)
        .cloned()
        .collect();
    assert_eq!(floor_only.len(), 1, "one floor signal to rule on");
    assert!(
        !plateforce_analysis::quality::distrusted(&floor_only),
        "a floor landing is a value a published rule produced, not one to drop from a spread"
    );
    // The control, because an assertion that a function returns false is worth nothing until
    // the function is shown returning true. This same response carries a jump-height
    // disagreement, which does distrust, so the ruling above is a ruling on the status rather
    // than a function that never fires.
    assert!(
        plateforce_analysis::quality::distrusted(&floored.signals),
        "nothing on this response distrusts it, so the exclusion above is vacuous"
    );
    assert_eq!(
        searched_on.takeoff_index,
        Some(TAKEOFF_FOUND_BY_A_WHOLE_RECORDING_SEARCH),
        "the rule this is measured against did not find the takeoff this guard describes"
    );

    let height = |response: &AnalysisResponse| {
        response
            .metric("jump_height_from_takeoff_meters")
            .and_then(|entry| entry.value)
            .expect("both runs produce a height from the impulse")
    };
    let (low, high) = (height(&floored), height(&searched_on));
    println!("jump height from the impulse: {low:.4} m against {high:.4} m");
    assert!(
        high > low * 10.0,
        "the two rules read {low:.4} m and {high:.4} m, which is not the spread this describes"
    );
}

/// The weighing half of the same question. The lowest-variance rule removes candidate windows
/// before it compares anything, and until it recorded the count a fifth of this recording was
/// ruled out of the search with nothing anywhere saying so.
#[test]
fn the_lowest_variance_rule_says_how_many_windows_it_ruled_out() {
    let trial = trial();
    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.adaptive_lowest_variance".into(),
            parameters: BTreeMap::from([("window_seconds".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: onset_choice(),
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let response = run(&trial, &request).expect("the searching weighing rule runs");

    let rejected = recorded(
        &response,
        "bwepoch.adaptive_lowest_variance",
        "rejected_window_count",
    )
    .expect("the rule records what its gate removed");
    let compared = recorded(
        &response,
        "bwepoch.adaptive_lowest_variance",
        "compared_window_count",
    )
    .expect("a rejection count without its population is not a proportion");

    // The window is 1200 samples of a 6000 sample recording, so the search has 4801 windows
    // to place, and the gate removes every one that touches the flight phase.
    println!(
        "{rejected} of {} windows removed by the gate",
        rejected + compared
    );
    assert_eq!(
        rejected + compared,
        4801.0,
        "the population is every window of the stated length"
    );
    assert!(
        rejected > 0.0,
        "the gate removed nothing, so this trial does not exercise it"
    );
    assert_eq!(rejected, 985.0, "the count this recording produces");
}
