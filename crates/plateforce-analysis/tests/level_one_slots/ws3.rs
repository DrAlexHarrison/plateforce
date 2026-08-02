//! What conditioning the signal does to where onset lands.
//!
//! The filter is applied inside onset detection rather than as a global preprocessing
//! step, so the rule that finds onset and the rule that conditions the signal are one
//! decision, and the shift between them is a property of the pair.

use plateforce_analysis::run;
use plateforce_core::smoothing::savitzky_golay_interpolated_edges;
use plateforce_core::Trial;

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS, CORPUS_SAMPLE_RATE_HZ};

/// JumpMetrics applies a 0.2 s window at order 3 inside its onset detection.
const WINDOW_SECONDS: f64 = 0.2;
const POLYNOMIAL_ORDER: usize = 3;

/// How far onset moves when the signal is conditioned, trial by trial.
///
/// Six trials cannot settle a mean shift of under a millisecond. What they can show is the
/// scale the shift is drawn from and whether it takes one sign, which is what decides
/// whether a systematic figure is separable from the scatter around it.
#[test]
fn conditioning_moves_onset_by_far_more_than_any_mean_shift_between_the_two() {
    let window_samples = (WINDOW_SECONDS * CORPUS_SAMPLE_RATE_HZ).round() as usize | 1;
    let mut shifts_milliseconds = Vec::new();

    for name in COMMITTED_TRIALS {
        let raw = committed_trial(name);
        let conditioned = Trial::new(
            savitzky_golay_interpolated_edges(raw.force(), window_samples, POLYNOMIAL_ORDER)
                .expect("the window fits the trace"),
            CORPUS_SAMPLE_RATE_HZ,
        )
        .expect("a conditioned trace is a trace");

        let onset_of = |trial: &Trial| {
            run(trial, &default_request())
                .unwrap_or_else(|error| panic!("{name} did not run: {error}"))
                .onset_index
                .unwrap_or_else(|| panic!("{name} placed no onset"))
        };

        let raw_onset = onset_of(&raw);
        let conditioned_onset = onset_of(&conditioned);
        let shift = (conditioned_onset as f64 - raw_onset as f64) / CORPUS_SAMPLE_RATE_HZ * 1000.0;
        shifts_milliseconds.push(shift);
        println!("{name}: onset {raw_onset} raw, {conditioned_onset} conditioned, {shift:+.1} ms");
    }

    assert_eq!(
        shifts_milliseconds.len(),
        COMMITTED_TRIALS.len(),
        "the check read fewer trials than are committed"
    );

    let mean = shifts_milliseconds.iter().sum::<f64>() / shifts_milliseconds.len() as f64;
    let spread = shifts_milliseconds.iter().copied().fold(f64::MIN, f64::max)
        - shifts_milliseconds.iter().copied().fold(f64::MAX, f64::min);
    println!(
        "over {} trials: mean {mean:+.1} ms, spread {spread:.1} ms, signs {}",
        shifts_milliseconds.len(),
        if shifts_milliseconds.iter().all(|s| *s >= 0.0)
            || shifts_milliseconds.iter().all(|s| *s <= 0.0)
        {
            "all one way"
        } else {
            "both ways"
        }
    );

    assert!(
        spread > 1.0,
        "conditioning moved onset by a spread of {spread} ms, so this check saw nothing to compare"
    );
}
