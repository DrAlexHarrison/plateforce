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

const TAKEOFF_FRAME_HEIGHT: &str = "jump_height_from_takeoff_meters";
const FLIGHT_TIME_HEIGHT: &str = "jump_height_from_flight_time_meters";

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
    if let Some(signal) = jump_height_routes_disagree(response) {
        found.push(signal);
    }
    found
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
pub fn distrusted(signals: &[QualitySignal]) -> bool {
    signals
        .iter()
        .any(|signal| signal.status == QualityStatus::Disagrees)
}

fn metric(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metric(key)
        .and_then(|entry| entry.value)
        .filter(|value| value.is_finite())
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
