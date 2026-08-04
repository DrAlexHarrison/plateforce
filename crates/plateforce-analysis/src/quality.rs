//! What the software already knows about a number it is showing, said beside that number.
//!
//! A signal is read off values this response already carries. Nothing here computes a
//! quantity, because a second derivation of a quantity is the failure this project
//! documents, and a signal that disagreed with the number it qualifies would be worse than
//! no signal at all.
//!
//! Every signal carries an action rather than a verdict. A rate stated without one leaves
//! the reader holding a diagnosis they cannot act on, which is the half of the in-line
//! quality pattern that does the work.

use serde::Serialize;

use crate::response::AnalysisResponse;
use crate::slots::movement_onset::{
    BACKWARD_OFFSET_FIXED, FLOOR_SECONDS, OFFSET_MILLISECONDS, SEARCH_FLOOR,
    SEARCH_FLOOR_AT_WEIGHING_EPOCH_END, WEIGHING_EPOCH_END_SECONDS,
};
use crate::slots::takeoff::{
    TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END, TAKEOFF_WEIGHING_EPOCH_END_SECONDS,
};

const TAKEOFF_FRAME_HEIGHT: &str = "jump_height_from_takeoff_meters";
const FLIGHT_TIME_HEIGHT: &str = "jump_height_from_flight_time_meters";
const ONSET_TIME: &str = "onset_time_seconds";
const TAKEOFF_TIME: &str = "takeoff_time_seconds";
const TIME_TO_TAKEOFF: &str = "time_to_takeoff_seconds";
const FLIGHT_TIME: &str = "flight_time_seconds";
const TAKEOFF_VELOCITY: &str = "takeoff_velocity_meters_per_second";
const NET_IMPULSE: &str = "net_impulse_newton_seconds";
const REACTIVE_STRENGTH_INDEX_MODIFIED: &str = "reactive_strength_index_modified";

/// Every quantity measured across the interval between the two landmarks, which is every
/// quantity that goes without a value when the interval runs backwards.
///
/// Listed rather than derived because the pipeline decides it by declining to build a
/// `Landmarks`, and a signal that named fewer keys than that would leave a reader holding a
/// blank cell with nothing pointing at it.
///
/// It held seven while flight time and the height taken from it read the three landmarks as a
/// bundle. They are measured from takeoff to the return to the plate, they answer whatever
/// onset did, and a signal claiming to account for them would tell a reader that a number in
/// front of them is absent.
const MEASURED_ACROSS_THE_INTERVAL: [&str; 5] = [
    TIME_TO_TAKEOFF,
    TAKEOFF_VELOCITY,
    NET_IMPULSE,
    TAKEOFF_FRAME_HEIGHT,
    REACTIVE_STRENGTH_INDEX_MODIFIED,
];

/// How far the two routes to a jump height may differ before the difference is no longer
/// the difference between the routes.
///
/// Measured on this project's corpus: observed flight runs 7.6 percent longer than the
/// flight implied by takeoff velocity, so the flight-time route overestimates the
/// takeoff-frame route by roughly 16 percent. A disagreement above that is not the known
/// bias between two correct answers. The number is a choice, so it rides on the signal
/// rather than sitting inside the comparison where a reader cannot see it.
pub const JUMP_HEIGHT_DISAGREEMENT_THRESHOLD_PERCENT: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    /// Two routes to one quantity differ by more than the published difference between
    /// the routes accounts for.
    Disagrees,
    /// One route produced no value, so the check could not run. Silence here would read
    /// exactly like a check that ran and found nothing wrong.
    Incomparable,
    /// A rule returned the first instant it was permitted to examine, so the landmark it
    /// reports is the boundary of its own search rather than anything it found in the trace.
    ///
    /// Not a fault in the rule and not a fault in the recording. The arithmetic is right and
    /// the rule did what it publishes, which is why this reports rather than refuses.
    AtSearchFloor,
    /// The jump's start was placed at or after its takeoff, so the interval between them runs
    /// backwards and every quantity measured across it goes without a value.
    ///
    /// Reported rather than refused, and the distinction is which numbers exist. Both rules
    /// ran and both answered; the trial still carries its system weight and both instants,
    /// and it is their order that no quantity can be measured across. A refusal names one
    /// rule and one parameter, and neither rule here is at fault on its own.
    OnsetNotBeforeTakeoff,
}

impl QualityStatus {
    /// The word this status travels under, wherever a surface writes it as text.
    ///
    /// Matched exhaustively, so a status added to the vocabulary is ruled on here rather
    /// than reaching a reader unnamed. Two surfaces had begun answering this question for
    /// themselves, one by matching and one by round-tripping through the serialiser with the
    /// variant's debug form as a fallback, which would have put `AtSearchFloor` into a
    /// spreadsheet cell the day the serialiser stopped answering.
    pub fn wire_name(self) -> &'static str {
        match self {
            QualityStatus::Disagrees => "disagrees",
            QualityStatus::Incomparable => "incomparable",
            QualityStatus::AtSearchFloor => "at_search_floor",
            QualityStatus::OnsetNotBeforeTakeoff => "onset_not_before_takeoff",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QualitySignal {
    /// What was compared, in the reader's words.
    pub label: String,
    /// The computed value the threshold is applied to. `None` when the comparison could
    /// not be made at all.
    pub value: Option<f64>,
    pub unit: &'static str,
    pub threshold: f64,
    pub status: QualityStatus,
    /// An action, never a verdict.
    pub remedy: String,
    /// The construct whose bound rule the reader would change, so a surface can put its
    /// published alternatives one interaction away rather than parsing the sentence.
    ///
    /// A construct rather than a single rule id: naming one rule would resolve a live
    /// methodological debate on the reader's behalf, at the moment they are most likely
    /// to accept whatever is suggested.
    pub remedy_construct: &'static str,
    /// The metric keys this signal qualifies, so a surface places it beside the value it
    /// is about without a second lookup table.
    pub qualifies: Vec<&'static str>,
}

/// `Serialize` on both types above is load-bearing rather than decorative: the browser's
/// only analysis path returns a serialised response, so a signal that does not travel in
/// it reaches no reader.
pub fn signals(response: &AnalysisResponse) -> Vec<QualitySignal> {
    let mut found = Vec::new();
    // First, because it accounts for absences rather than qualifying values, and a reader
    // meeting seven blank cells needs the reason before anything said about a number.
    if let Some(signal) = onset_placed_at_or_after_takeoff(response) {
        found.push(signal);
    }
    if let Some(signal) = jump_height_routes_disagree(response) {
        found.push(signal);
    }
    if let Some(signal) = onset_placed_at_its_search_floor(response) {
        found.push(signal);
    }
    if let Some(signal) = takeoff_placed_at_its_search_floor(response) {
        found.push(signal);
    }
    found
}

/// Whether the jump's start was placed at or after its takeoff.
///
/// Both landmarks are already on the response and both rules answered, so nothing here is
/// computed twice: the test is the comparison of two sample indices, which is the same
/// comparison the pipeline makes when it declines to assemble the landmarks at all.
///
/// A takeoff found before an onset is not a near miss. Three of the five onset rules search
/// forward from the weighing window, so under those the order inverts only where the plate is
/// unloaded before the trace has left quiet standing.
/// `onset.threshold.adaptive_trailing_window` inverts it from the other end: it keeps the last
/// departure from quiet before the recording's force extremum, and on a trace still running
/// after the athlete steps off the plate that extremum is the step-off, not the flight.
///
/// Measured on `synthetic_untrimmed_step_off_after_jump` under
/// `bwepoch.adaptive_lowest_variance`, `onset.threshold.adaptive_trailing_window` and
/// `takeoff.threshold.flight_noise_k_sd`: onset 3.5433 s against takeoff 1.8567 s, with seven
/// quantities left without a value. The 246-trial corpus raises it on 0 of the 12,300
/// combinations it offers, every trial in it having been trimmed to one jump before archiving.
fn onset_placed_at_or_after_takeoff(response: &AnalysisResponse) -> Option<QualitySignal> {
    let onset_index = response.onset_index?;
    let takeoff_index = response.takeoff_index?;
    if onset_index < takeoff_index {
        return None;
    }
    let onset_seconds = metric(response, ONSET_TIME)?;
    let takeoff_seconds = metric(response, TAKEOFF_TIME)?;

    Some(QualitySignal {
        label: "The start of the jump, against the instant of takeoff".to_string(),
        value: Some(onset_seconds),
        unit: "seconds",
        threshold: takeoff_seconds,
        status: QualityStatus::OnsetNotBeforeTakeoff,
        remedy: format!(
            "Takeoff is placed at {takeoff_seconds:.4} s and the start of the jump at \
             {onset_seconds:.4} s, so the interval between them runs backwards and the five \
             quantities measured across it have no value on this trial. A stretch where the \
             plate carries almost no force before the jump begins, a step onto it or a \
             recording started early, satisfies a takeoff rule before the trace has left \
             quiet standing. Compare the other published rules for takeoff, or analyse the \
             span of the recording holding the jump, and watch both instants move."
        ),
        remedy_construct: crate::TAKEOFF_CONSTRUCT,
        qualifies: MEASURED_ACROSS_THE_INTERVAL.to_vec(),
    })
}

/// Whether these signals give the software reason to leave a value out of a figure taken
/// over rules that agree with one another.
///
/// `Disagrees` does. Two routes to one quantity that disagree past the published
/// difference between them means at least one of the two is wrong, and a spread that
/// counted it would report the wrong one as the cost of choosing a method.
///
/// `Incomparable` does not. A check that could not run is not evidence against the value
/// it could not check, and treating it as evidence would drop every truncated trace from
/// its own spread without anything having gone wrong.
///
/// `AtSearchFloor` does not either, and this one is a decision rather than a consequence. A
/// rule that returns its own boundary has still done what it publishes, on a recording where
/// that is what it does. The distance between it and the rules that searched somewhere else is
/// the disagreement between published methods, measured, and a spread that dropped the rule
/// would report the methods as closer together than they are. The ruling covers the onset half
/// and the takeoff half alike: on subject 01's first trial dropping the two takeoff rules that
/// return their floor would hide jump heights of 1.4 cm against 37.0 cm.
///
/// `OnsetNotBeforeTakeoff` does not, and here the reason is that there is nothing to drop.
/// Every quantity it qualifies already has no value, so a spread over them never saw one. What
/// the trial does carry is its system weight and its two instants, which are correct, and
/// distrusting the response would take those out of figures they belong in on the strength of
/// a condition that says nothing about them.
pub fn distrusted(signals: &[QualitySignal]) -> bool {
    signals
        .iter()
        .any(|signal| signal.status == QualityStatus::Disagrees)
}

/// The number a metric reported, and nothing where it reported none.
///
/// No second non-finite test here. `Metric::from_declaration` is the one place a metric is
/// built and it never puts a value that is not finite into `value`, so a filter here would be
/// the same policy spelled a second time, and a reader could not tell which of the two was
/// the one that decides.
fn metric(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response.metric(key).and_then(|entry| entry.value)
}

/// A value one bound rule read, at the precision it read it rather than the precision it is
/// printed at.
fn bound_value(response: &AnalysisResponse, method_id: &str, name: &str) -> Option<f64> {
    response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == method_id)
        .and_then(|bound| bound.numeric_values.get(name).copied())
        .filter(|value| value.is_finite())
}

/// The interval between two samples of this recording, in seconds.
///
/// A landmark's time is its index times this interval and nothing else, so any landmark the
/// response carries returns the interval it was read at. Two are tried because either index
/// can be zero, and the one that is would return an interval no recording has.
fn sample_interval_seconds(response: &AnalysisResponse) -> Option<f64> {
    [
        (response.onset_index, ONSET_TIME),
        (response.takeoff_index, TAKEOFF_TIME),
    ]
    .into_iter()
    .find_map(|(index, key)| match index {
        Some(index) if index > 0 => metric(response, key).map(|seconds| seconds / index as f64),
        _ => None,
    })
    .filter(|interval| interval.is_finite() && *interval > 0.0)
}

/// The first sample an onset rule was permitted to examine, and which operator put it there.
struct SearchFloor {
    index: usize,
    seconds: f64,
    /// Whether the weighing rule settled this floor, which decides what the reader can move.
    set_by_the_weighing_window: bool,
}

/// Two operators place this floor and the registry files them as separate entries, so which
/// of the two ran is read off the record rather than assumed.
///
/// The weighing epoch's own end index is taken as the index rather than converted back from
/// the seconds beside it, because that index is the one the search was given.
fn onset_search_floor(
    response: &AnalysisResponse,
    sample_interval_seconds: f64,
) -> Option<SearchFloor> {
    if let Some(seconds) = bound_value(
        response,
        SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
        WEIGHING_EPOCH_END_SECONDS,
    ) {
        return Some(SearchFloor {
            index: response.weighing_end_index,
            seconds,
            set_by_the_weighing_window: true,
        });
    }
    let seconds = bound_value(response, SEARCH_FLOOR, FLOOR_SECONDS)?;
    Some(SearchFloor {
        index: (seconds / sample_interval_seconds).round().max(0.0) as usize,
        seconds,
        set_by_the_weighing_window: false,
    })
}

/// Whether the onset a rule reported is the first instant it was allowed to look at.
///
/// A forward search starts at its floor, so the crossing it returns is at or after that
/// floor and the two are equal exactly when the search found nothing before returning. That
/// is a comparison of two sample indices, both integers, so there is no tolerance to choose
/// and no float to compare: the crossing is the reported onset with the composed backward
/// offset put back, and the floor is the index the weighing rule or the caller set.
///
/// Measured on subject 01's first trial, where `onset.threshold.absolute_force` and
/// `onset.threshold.relative_to_system_weight` both report index 1164 against a floor of
/// 1200 and an offset of 36 samples, while the three rules that found a departure report
/// 4090, 4091 and 4097.
fn onset_placed_at_its_search_floor(response: &AnalysisResponse) -> Option<QualitySignal> {
    let onset_index = response.onset_index?;
    let interval_seconds = sample_interval_seconds(response)?;
    let floor = onset_search_floor(response, interval_seconds)?;
    // A floor at the first sample of the recording forbids nothing, so a crossing there is
    // the rule reading the whole trace rather than the rule reading its own boundary.
    if floor.index == 0 {
        return None;
    }
    let offset_samples = bound_value(response, BACKWARD_OFFSET_FIXED, OFFSET_MILLISECONDS)
        .map(|milliseconds| (milliseconds / 1000.0 / interval_seconds).round().max(0.0) as usize)
        .unwrap_or(0);
    if onset_index + offset_samples != floor.index {
        return None;
    }

    let lever = if floor.set_by_the_weighing_window {
        "This rule begins searching where the weighing window ends"
    } else {
        "This rule begins searching at the floor this analysis set"
    };
    let second_lever = if floor.set_by_the_weighing_window {
        "move the weighing window"
    } else {
        "lower the floor"
    };
    Some(QualitySignal {
        label: "Movement onset, against the instant this rule's search begins".to_string(),
        value: Some(floor.seconds),
        unit: "seconds",
        threshold: 0.0,
        status: QualityStatus::AtSearchFloor,
        remedy: format!(
            "{lever}, at {:.4} s, and the trace was already outside its threshold there, so \
             the onset it reports is the start of its own search rather than a departure it \
             found in the recording. Time to takeoff, the impulse and everything drawn from \
             them run from that instant. Compare the other published rules for the start of \
             the jump, or {second_lever}, and watch this time change.",
            floor.seconds
        ),
        remedy_construct: crate::ONSET_CONSTRUCT,
        qualifies: vec![ONSET_TIME, TIME_TO_TAKEOFF],
    })
}

/// Whether the takeoff a rule reported is the first instant it was allowed to look at.
///
/// The same comparison as the onset half and for the same reason: a forward search starts at
/// its floor, so the sample it returns is at or after that floor and the two are equal exactly
/// when the search returned without finding anything later. Both sides are sample indices, so
/// there is no tolerance to choose and no float compared. Simpler than the onset half by one
/// step, because no takeoff rule composes a backward offset, so the index the rule returned is
/// the index the search reached.
///
/// Two of the five shipped takeoff rules take this floor. The other three record their floor
/// under `takeoff.op.search_floor_at_trial_start` instead, so the lookup below finds nothing for
/// them and the comparison is never reached. That exclusion is structural rather than
/// arithmetic, which is why the guard holds the comparison against a rule that records this
/// floor and searches past it: without that case a comparison firing on every rule that records
/// a floor would still leave the other three quiet.
///
/// Measured on subject 01's first trial under a weighing window of 4.4 s, which ends at sample
/// 5280 inside the flight phase: `takeoff.threshold.absolute_force` and
/// `takeoff.threshold.flight_noise_k_sd` both report 5280, while the three that search the whole
/// recording report 5014. Jump height from the impulse reads 1.4 cm against 37.0 cm.
fn takeoff_placed_at_its_search_floor(response: &AnalysisResponse) -> Option<QualitySignal> {
    let takeoff_index = response.takeoff_index?;
    let floor_seconds = bound_value(
        response,
        TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
        TAKEOFF_WEIGHING_EPOCH_END_SECONDS,
    )?;
    let floor_index = response.weighing_end_index;
    // A floor at the first sample forbids nothing, so a takeoff there was found rather than
    // returned. No shipped weighing rule reaches it: all three refuse a window of zero length.
    if floor_index == 0 || takeoff_index != floor_index {
        return None;
    }

    Some(QualitySignal {
        label: "Takeoff, against the instant this rule's search begins".to_string(),
        value: Some(floor_seconds),
        unit: "seconds",
        threshold: 0.0,
        status: QualityStatus::AtSearchFloor,
        remedy: format!(
            "This rule begins searching for the flight phase where the weighing window ends, at \
             {floor_seconds:.4} s, and the plate was already below its threshold there, so the \
             takeoff it reports is the start of its own search rather than a flight phase it \
             found in the recording. Flight time, the impulse and everything drawn from them \
             run to that instant. Compare the other published rules for takeoff, or move the \
             weighing window, and watch this time change."
        ),
        remedy_construct: crate::TAKEOFF_CONSTRUCT,
        qualifies: vec![TAKEOFF_TIME, TIME_TO_TAKEOFF, FLIGHT_TIME],
    })
}

/// The impulse route and the flight-time route answer the same question from different
/// halves of the trace, so they check each other, and which of the two reads higher says
/// which landmark to look at.
///
/// The impulse route counts every newton between the two landmarks. Reading high means it
/// counted too much, and the way to count too much is to start after the unweighting has
/// begun, missing the negative impulse. Reading low means it counted too little, and the
/// ways to do that both sit at the other end: stopping before the propulsion finishes, or
/// running past takeoff into a flight phase carrying almost no force.
///
/// The direction is evidence rather than proof. A system weight that is itself wrong moves
/// the impulse route either way, so the remedy names the landmark the evidence points at
/// and leaves both heights on screen for the reader to check.
fn jump_height_routes_disagree(response: &AnalysisResponse) -> Option<QualitySignal> {
    let from_takeoff = metric(response, TAKEOFF_FRAME_HEIGHT)?;
    let qualifies = vec![TAKEOFF_FRAME_HEIGHT, FLIGHT_TIME_HEIGHT];
    let label = "Jump height from the impulse against jump height from the flight time".to_string();

    let Some(from_flight) = metric(response, FLIGHT_TIME_HEIGHT) else {
        return Some(QualitySignal {
            label,
            value: None,
            unit: "percent",
            threshold: JUMP_HEIGHT_DISAGREEMENT_THRESHOLD_PERCENT,
            status: QualityStatus::Incomparable,
            remedy: "This trace carries no flight time, so there is no second route to \
                     check this height against. A recording that runs past the landing \
                     gives it one."
                .to_string(),
            remedy_construct: crate::TAKEOFF_CONSTRUCT,
            qualifies,
        });
    };

    if from_flight.abs() <= f64::EPSILON {
        return None;
    }
    let disagreement_percent = 100.0 * (from_takeoff - from_flight).abs() / from_flight.abs();
    if disagreement_percent <= JUMP_HEIGHT_DISAGREEMENT_THRESHOLD_PERCENT {
        return None;
    }

    let centimetres_apart = 100.0 * (from_takeoff - from_flight).abs();
    let counted_too_much = from_takeoff > from_flight;
    let remedy = if counted_too_much {
        format!(
            "These two heights are {centimetres_apart:.0} cm apart, and the impulse route \
             reads higher. It counts every newton from the start of the jump, so a start \
             placed after the unweighting has begun inflates it. Compare the rules for the \
             start of the jump and watch both numbers."
        )
    } else {
        format!(
            "These two heights are {centimetres_apart:.0} cm apart, and the impulse route \
             reads lower. It stops counting at takeoff, so a takeoff placed anywhere but \
             the last foot leaving the plate cuts the count short. Compare the rules for \
             takeoff and watch both numbers."
        )
    };

    Some(QualitySignal {
        label,
        value: Some(disagreement_percent),
        unit: "percent",
        threshold: JUMP_HEIGHT_DISAGREEMENT_THRESHOLD_PERCENT,
        status: QualityStatus::Disagrees,
        remedy,
        remedy_construct: if counted_too_much {
            crate::ONSET_CONSTRUCT
        } else {
            crate::TAKEOFF_CONSTRUCT
        },
        qualifies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The statuses this file checks the spelling of, and its own position in the list.
    ///
    /// The match is what carries the list forward: a variant added to the vocabulary makes it
    /// non-exhaustive and stops this file compiling, in the same file the list lives in. The
    /// assertion below is what makes the two agree, so a variant given a position outside the
    /// list turns it red rather than passing unnoticed.
    fn position_in_the_list(status: QualityStatus) -> usize {
        match status {
            QualityStatus::Disagrees => 0,
            QualityStatus::Incomparable => 1,
            QualityStatus::AtSearchFloor => 2,
            QualityStatus::OnsetNotBeforeTakeoff => 3,
        }
    }

    const EVERY_STATUS: [QualityStatus; 4] = [
        QualityStatus::Disagrees,
        QualityStatus::Incomparable,
        QualityStatus::AtSearchFloor,
        QualityStatus::OnsetNotBeforeTakeoff,
    ];

    /// The word a surface prints and the word the JSON carries are one word, proved against
    /// the serialiser rather than against a second list written here.
    #[test]
    fn every_status_prints_the_word_the_wire_carries() {
        for (index, status) in EVERY_STATUS.into_iter().enumerate() {
            assert_eq!(
                index,
                position_in_the_list(status),
                "{status:?} sits at a different place in the list than the match gives it"
            );
            let serialised = serde_json::to_value(status).expect("a status serialises");
            assert_eq!(
                serde_json::Value::String(status.wire_name().to_string()),
                serialised,
                "{status:?} prints one word and serialises as another"
            );
        }
    }

    /// Four words, four statuses. A variant sharing another's word would read to every surface
    /// as the status it borrowed from.
    #[test]
    fn no_two_statuses_travel_under_one_word() {
        let mut words: Vec<&str> = EVERY_STATUS.iter().map(|s| s.wire_name()).collect();
        words.sort_unstable();
        let spoken = words.len();
        words.dedup();
        assert_eq!(spoken, words.len(), "two statuses share a word: {words:?}");
    }
}
