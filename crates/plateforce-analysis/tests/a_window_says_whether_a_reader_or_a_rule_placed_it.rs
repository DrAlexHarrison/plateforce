//! Two windows over the same samples, and the record that has to tell them apart.
//!
//! A reader who drags a span on the trace and a reader who double-clicks the phase that covers
//! the same span have made two different claims. The first is theirs and rests on no published
//! rule; the second is the boundary rules' and carries their citations. A build whose record
//! could not separate the two would have reproduced, inside this software, the defect it was
//! written to expose.
//!
//! So these run both rules against one trace, force them onto the same samples, and assert the
//! records differ in the one way that matters.

use std::collections::BTreeMap;

use plateforce_analysis::document::{ResultDocument, TrialSource};
use plateforce_analysis::{
    markdown, run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice,
};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::Trial;

mod common;

const STATED: &str = "window.stated.by_caller";
const NAMED_PHASE: &str = "window.from_named_phase";
const WINDOW: &str = "analysis_window";
const START_KEY: &str = "analysis_window_start_seconds";
const END_KEY: &str = "analysis_window_end_seconds";

/// A countermovement jump with a landing larger than anything in the jump, which is the shape
/// that makes the choice of window decide a peak.
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
    common::prepared(AnalysisRequest {
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
    })
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
    common::prepared(request)
}

fn phase_boundaries() -> Vec<(&'static str, &'static str)> {
    vec![
        ("braking_phase_start", "phase.braking_start.min_force"),
        (
            "propulsion_phase_start",
            "phase.propulsion_start.zero_velocity",
        ),
        ("propulsion_phase_end", "phase.propulsion_end.takeoff"),
    ]
}

fn answered(request: AnalysisRequest) -> AnalysisResponse {
    run(&a_jump_that_lands(), &request).expect("the request runs")
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

fn row<'a>(response: &'a AnalysisResponse, id: &str) -> &'a plateforce_analysis::BoundMethod {
    response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == id)
        .unwrap_or_else(|| panic!("{id} is on the record"))
}

fn document(response: &AnalysisResponse, rows: usize) -> ResultDocument {
    ResultDocument::of(
        "0.1.0",
        TrialSource {
            name: "selected jump".into(),
            rows_read: rows,
            samples_matching_the_convention: 0,
            sample_rate_hz: 1200.0,
        },
        &RegistryStamp {
            version: Some("fixture-pin".to_string()),
            declared_version: Some("fixture-declares".to_string()),
            digest: Some("content-fixture".to_string()),
        },
        &plateforce_core::Capture::default(),
        response,
        None,
    )
}

/// The one property the pair exists for. Both windows are made to cover the same samples, and
/// the record still separates them: the stated one carries the reader's signature on both ends
/// and names no rule behind them, the phase one names the two rules that placed its ends and
/// carries the reader's signature on the phase alone.
#[test]
fn one_span_two_records_and_the_record_says_which_hand_drew_it() {
    let mut with_phase = naming(&[
        (WINDOW, NAMED_PHASE),
        ("braking_phase_start", "phase.braking_start.min_force"),
        (
            "propulsion_phase_start",
            "phase.propulsion_start.zero_velocity",
        ),
    ]);
    with_phase.derived.get_mut(WINDOW).unwrap().options.insert(
        "phase".to_string(),
        "braking_start_to_propulsion_start".to_string(),
    );
    let placed = answered(common::prepared(with_phase));

    let (from, to) = (
        value(&placed, START_KEY).expect("the phase window has a start"),
        value(&placed, END_KEY).expect("the phase window has an end"),
    );

    let mut hand = naming(&[(WINDOW, STATED)]);
    let choice = hand.derived.get_mut(WINDOW).unwrap();
    choice.parameters.insert("start_seconds".to_string(), from);
    choice.parameters.insert("end_seconds".to_string(), to);
    let drawn = answered(common::prepared(hand));

    assert_eq!(
        (value(&drawn, START_KEY), value(&drawn, END_KEY)),
        (Some(from), Some(to)),
        "the two windows have to cover the same samples for this comparison to mean anything",
    );

    let stated_row = row(&drawn, STATED);
    let placed_row = row(&placed, NAMED_PHASE);
    assert_eq!(
        stated_row.parameter_sources.get("start_seconds"),
        Some(&ParameterSource::Stated),
        "a window a reader drew records both of its ends as theirs",
    );
    assert_eq!(
        stated_row.parameter_sources.get("end_seconds"),
        Some(&ParameterSource::Stated),
    );
    assert!(
        !placed_row
            .bound_parameters
            .iter()
            .any(|(name, _)| name.ends_with("_seconds")),
        "a window taken from a phase records no instant of the reader's: {:?}",
        placed_row.bound_parameters,
    );

    let behind_the_placed_one = &placed.regions;
    let named = behind_the_placed_one
        .iter()
        .find(|region| region.phase == "braking_start_to_propulsion_start")
        .expect("the run reports the interval it settled");
    assert!(
        named
            .placed_by
            .contains(&"phase.braking_start.min_force".to_string()),
        "the interval names the rules that placed its ends: {:?}",
        named.placed_by,
    );
    assert!(
        !drawn
            .regions
            .iter()
            .any(|region| region.placed_by.is_empty()),
        "a reported interval with no rule behind it is an interval nobody placed",
    );
}

/// The stated rule publishes no default for either end, so a caller who names it and states
/// nothing is refused by name. Serving the recording's own extent would be the silent default
/// this entry exists to refuse.
#[test]
fn a_stated_window_with_no_instants_declines_and_names_what_it_wanted() {
    let response = answered(naming(&[(WINDOW, STATED)]));
    let declined = response
        .refusals
        .iter()
        .find(|refusal| refusal.method_id == STATED)
        .expect("the rule declines rather than filling in an interval");
    let said = declined.refusal.to_string();
    assert!(
        said.contains("start_seconds") || said.contains("end_seconds"),
        "the refusal names the instant it wanted: {said}",
    );
    assert_eq!(value(&response, START_KEY), None);
}

/// Both ends inside the recording, in order, or the rule declines. A window clamped to fit
/// answers a question about a different window from the one that was asked.
#[test]
fn a_stated_window_outside_the_recording_or_out_of_order_is_refused_rather_than_clamped() {
    for (start, end, what) in [
        (0.5f64, 0.5f64, "a window of no width"),
        (1.2, 0.4, "a window that ends before it starts"),
        (0.5, 99.0, "an end past the last sample"),
        (-1.0, 0.5, "a start before the first sample"),
    ] {
        let mut request = naming(&[(WINDOW, STATED)]);
        let choice = request.derived.get_mut(WINDOW).unwrap();
        choice.parameters.insert("start_seconds".to_string(), start);
        choice.parameters.insert("end_seconds".to_string(), end);
        let response = answered(common::prepared(request));
        assert!(
            response
                .refusals
                .iter()
                .any(|refusal| refusal.method_id == STATED),
            "{what} is refused rather than run: {start} to {end}",
        );
        assert_eq!(
            value(&response, START_KEY),
            None,
            "{what} publishes no window",
        );
    }
}

/// A peak is taken over the window, so the two windows have to give different peaks on a trace
/// whose landing is bigger than its jump. A figure that does not move when the window does is
/// a window nothing reads.
#[test]
fn the_window_a_reader_states_moves_the_peak_it_is_taken_over() {
    let mut over_the_jump = naming(&[(WINDOW, STATED), ("peak_force", "force.peak.gross")]);
    let choice = over_the_jump.derived.get_mut(WINDOW).unwrap();
    choice.parameters.insert("start_seconds".to_string(), 1.0);
    choice.parameters.insert("end_seconds".to_string(), 1.6);
    let jump = answered(common::prepared(over_the_jump));

    let mut over_the_landing = naming(&[(WINDOW, STATED), ("peak_force", "force.peak.gross")]);
    let choice = over_the_landing.derived.get_mut(WINDOW).unwrap();
    choice.parameters.insert("start_seconds".to_string(), 2.2);
    choice.parameters.insert("end_seconds".to_string(), 2.4);
    let landing = answered(common::prepared(over_the_landing));

    let (over_jump, over_landing) = (
        value(&jump, "peak_force_newtons").expect("a peak over the jump"),
        value(&landing, "peak_force_newtons").expect("a peak over the landing"),
    );
    assert!(
        (over_landing - over_jump).abs() > 100.0,
        "the same rule over two stated windows gave {over_jump} N and {over_landing} N, which is \
         one window doing nothing",
    );
}

/// Every interval the run reports is one whose two ends a rule placed, and a run that placed
/// none reports none. The empty case is the control: without it, a list that is always full
/// would read the same as a list that is right.
#[test]
fn the_intervals_a_run_offers_are_the_ones_its_own_rules_settled() {
    let with_boundaries = answered(naming(&phase_boundaries()));
    // The control names a derived rule and no boundary rule, rather than naming none at all. A
    // request with an empty derived map returns before the intervals are gathered, so a control
    // built that way is empty whatever the gathering does and cannot fail for the reason the
    // real query would. Caught by breaking the gathering and watching this test stay green.
    let without = answered(naming(&[("peak_force", "force.peak.gross")]));

    assert!(
        !with_boundaries.regions.is_empty(),
        "a run that placed three boundaries offers the intervals between them",
    );
    assert!(
        without.regions.is_empty(),
        "a run with no boundary rule on the path offers no interval: {:?}",
        without.regions.iter().map(|r| r.phase).collect::<Vec<_>>(),
    );
    for region in &with_boundaries.regions {
        assert!(
            region.end_index > region.start_index,
            "{} runs from {} to {}",
            region.phase,
            region.start_index,
            region.end_index,
        );
        assert!(
            !region.placed_by.is_empty(),
            "{} names no rule behind it",
            region.phase,
        );
    }
}

/// A copied selection is a description of the data in that selection, not only of the rules
/// that happened to compute a quantity over it. The three visible landmarks are the major events
/// a reader uses to orient the trace, so the block names exactly the ones inside the two stated
/// ends and keeps each event attached to the rule chain that placed it.
#[test]
fn a_copied_window_names_exactly_the_landmarks_inside_it_and_their_rules() {
    let trial = a_jump_that_lands();
    let landmarks = answered(base());
    let onset = value(&landmarks, "onset_time_seconds").expect("the jump has an onset");
    let takeoff = value(&landmarks, "takeoff_time_seconds").expect("the jump has a takeoff");
    let flight = value(&landmarks, "flight_time_seconds").expect("the jump has a landing");

    let mut request = naming(&[(WINDOW, STATED), ("peak_force", "force.peak.gross")]);
    let choice = request.derived.get_mut(WINDOW).unwrap();
    choice
        .parameters
        .insert("start_seconds".to_string(), onset - 0.01);
    choice
        .parameters
        .insert("end_seconds".to_string(), takeoff + 0.01);
    let response = answered(common::prepared(request));
    let copied = markdown::window(&document(&response, trial.len()), STATED);

    assert!(
        copied.contains("## Landmarks in this window"),
        "the copied window has no landmark ledger:\n{copied}",
    );
    assert!(
        copied.contains("Movement onset") && copied.contains("onset.threshold.noise_relative"),
        "the contained onset and its rule are absent:\n{copied}",
    );
    assert!(
        copied.contains("Takeoff") && copied.contains("takeoff.threshold.absolute_force"),
        "the contained takeoff and its rule are absent:\n{copied}",
    );
    assert!(
        !copied.contains("- Landing at"),
        "landing at {:.4} s is outside the window and was still reported:\n{copied}",
        takeoff + flight,
    );
}

/// The empty-landmark case is the one that looked like a successful copy while telling a reader
/// nothing about where their range sat. A range wholly inside flight contains no boundary, but it
/// still carries the takeoff and landing rules that define the phase around it.
#[test]
fn a_copied_window_with_no_landmark_says_it_is_inside_flight_under_both_boundary_rules() {
    let trial = a_jump_that_lands();
    let landmarks = answered(base());
    let takeoff = value(&landmarks, "takeoff_time_seconds").expect("the jump has a takeoff");
    let flight = value(&landmarks, "flight_time_seconds").expect("the jump has a landing");

    let mut request = naming(&[(WINDOW, STATED), ("peak_force", "force.peak.gross")]);
    let choice = request.derived.get_mut(WINDOW).unwrap();
    choice
        .parameters
        .insert("start_seconds".to_string(), takeoff + 0.05);
    choice
        .parameters
        .insert("end_seconds".to_string(), takeoff + flight - 0.05);
    let response = answered(common::prepared(request));
    let copied = markdown::window(&document(&response, trial.len()), STATED);

    assert!(
        copied.contains("No movement onset, takeoff, or landing falls inside this window."),
        "the no-landmark result is not stated:\n{copied}",
    );
    assert!(
        copied.contains("This window is inside flight")
            && copied.contains("takeoff.threshold.absolute_force")
            && copied.contains("flight_time.takeoff_to_touchdown"),
        "the surrounding phase and both of its rules are absent:\n{copied}",
    );
}
