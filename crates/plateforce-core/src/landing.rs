//! Landing, the instant the athlete is back on the plate, and the drop-jump arrival that is
//! the same event read on a recording that opens with the plate empty.
//!
//! Two registered rules answer this question and they differ only in which threshold they
//! compare against and how long the return has to hold, so both come through one search here.
//! Tying the rising edge to the threshold that placed takeoff makes a threshold error compound
//! across the flight phase, because a higher threshold places takeoff earlier and landing
//! later; stating the rising edge separately lets the two errors stay where they were made.
//!
//! A return is only a return if the plate was unloaded first. Without that the search answers
//! the first sample of a recording that opens with the athlete standing on the plate, which is
//! quiet stance rather than a landing. With it, the same search reads a countermovement jump's
//! post-flight landing and a drop jump's arrival off the box, which is the one event the
//! registry's construct names: the instant the first foot returns to the plate.

use crate::signal::TrialError;

/// The first sample beginning a run at or above `threshold_newtons` that holds for
/// `minimum_contact_samples`, searched forward from `search_from`.
///
/// The run has to be preceded by a sample below the threshold, at or after `search_from`, so
/// what is found is a rising crossing rather than whatever the search happened to start on.
/// A tied rule searching from the takeoff it was handed gets that for nothing, because takeoff
/// is the start of a run below the threshold. A rule searching the whole recording needs it:
/// on a trace that opens at standing weight every sample is already above the threshold, and
/// without the crossing requirement the rule would answer sample zero and call quiet stance a
/// landing.
///
/// `minimum_contact_samples` of 1 asks for no span at all, which is what the tied rule states.
pub fn landing_first_sustained_run(
    signal: &[f64],
    threshold_newtons: f64,
    minimum_contact_samples: usize,
    search_from: usize,
    sample_rate_hz: f64,
    method_id: &str,
    parameter: &str,
) -> Result<usize, TrialError> {
    let mut plate_has_been_unloaded = false;
    let mut run_start: Option<usize> = None;
    for (index, &value) in signal.iter().enumerate().skip(search_from) {
        if value < threshold_newtons {
            plate_has_been_unloaded = true;
            run_start = None;
        } else if plate_has_been_unloaded {
            let start = *run_start.get_or_insert(index);
            if index - start + 1 >= minimum_contact_samples.max(1) {
                return Ok(start);
            }
        }
    }
    Err(TrialError::NoCrossing {
        method_id: method_id.to_string(),
        parameter: parameter.to_string(),
        value: threshold_newtons,
        search_bound_seconds: signal.len() as f64 / sample_rate_hz,
    })
}

/// The velocity the athlete arrived on the plate with, recovered from the standing period the
/// recording ends in rather than from the height of the box they stepped off.
///
/// `zero_anchored_velocity` is the centre-of-mass velocity integrated forward from the arrival
/// with zero written at the arrival, and `final_period` is the half-open sample range the
/// athlete stands still over at the end of the recording. Standing still means the true
/// velocity there is zero, so whatever the zero-anchored series reads over that period is the
/// arrival velocity the integration was missing, with its sign reversed.
///
/// The box height cancels, which is the method's whole point rather than an omission. Seeding
/// the integration with `v` shifts every sample of the series by `v`, so the correction the
/// source applies, `v` less the mean of the seeded series over the final period, comes to
/// minus the mean of the zero-anchored series whatever `v` was. A stated box height would
/// therefore be a name on the record that cannot move the number, so this rule takes none, and
/// `jumpheight.dj.box_height_as_drop_height` is the entry that does.
///
/// Returns nothing where the final period is empty or reaches past the series, which is a
/// recording that does not end in the standing period this rests on.
pub fn arrival_velocity_from_final_standing_period_meters_per_second(
    zero_anchored_velocity: &[f64],
    final_period: std::ops::Range<usize>,
) -> Option<f64> {
    if final_period.is_empty() || final_period.end > zero_anchored_velocity.len() {
        return None;
    }
    let samples = final_period.len() as f64;
    let mean = zero_anchored_velocity[final_period].iter().sum::<f64>() / samples;
    Some(-mean)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE_HZ: f64 = 1000.0;
    const THRESHOLD_NEWTONS: f64 = 20.0;

    /// Quiet standing, flight, then a landing that stays loaded.
    fn stand_then_fly_then_land(
        standing_samples: usize,
        flight_samples: usize,
        landing_samples: usize,
    ) -> Vec<f64> {
        let mut signal = vec![700.0; standing_samples];
        signal.extend(std::iter::repeat_n(2.0, flight_samples));
        signal.extend(std::iter::repeat_n(2100.0, landing_samples));
        signal
    }

    fn first_run(
        signal: &[f64],
        minimum_contact_samples: usize,
        search_from: usize,
    ) -> Option<usize> {
        landing_first_sustained_run(
            signal,
            THRESHOLD_NEWTONS,
            minimum_contact_samples,
            search_from,
            RATE_HZ,
            "landing.threshold.absolute_force",
            "threshold_n",
        )
        .ok()
    }

    #[test]
    fn a_recording_that_opens_at_standing_weight_is_not_landed_on_at_its_first_sample() {
        let signal = stand_then_fly_then_land(500, 400, 600);
        assert_eq!(
            first_run(&signal, 1, 0),
            Some(900),
            "the search from index zero has to reach the flight phase before anything counts"
        );
    }

    /// The arrival off a box, which is the same search on a recording that opens unloaded.
    #[test]
    fn a_recording_that_opens_with_an_empty_plate_is_landed_on_at_the_arrival() {
        let mut signal = vec![0.5; 300];
        signal.extend(std::iter::repeat_n(1800.0, 400));
        signal.extend(std::iter::repeat_n(2.0, 350));
        signal.extend(std::iter::repeat_n(1900.0, 500));
        assert_eq!(
            first_run(&signal, 1, 0),
            Some(300),
            "the first foot reaches the plate at the end of the empty stretch"
        );
    }

    #[test]
    fn a_brief_return_across_the_threshold_is_skipped_once_a_span_is_required() {
        let mut signal = vec![700.0; 500];
        signal.extend(std::iter::repeat_n(2.0, 100));
        signal.extend(std::iter::repeat_n(25.0, 5));
        signal.extend(std::iter::repeat_n(2.0, 300));
        signal.extend(std::iter::repeat_n(2100.0, 400));
        assert_eq!(
            first_run(&signal, 1, 0),
            Some(600),
            "with no span required the chatter is the landing"
        );
        assert_eq!(
            first_run(&signal, 15, 0),
            Some(905),
            "a 15 sample span passes over a 5 sample excursion"
        );
    }

    #[test]
    fn a_search_that_never_leaves_the_plate_refuses_rather_than_answering_its_first_sample() {
        let signal = vec![700.0; 800];
        assert!(first_run(&signal, 1, 0).is_none());
    }

    #[test]
    fn the_arrival_velocity_reverses_the_sign_of_what_the_final_period_reads() {
        let velocity = vec![0.0, -1.0, 2.4, 2.4, 2.4, 2.4];
        assert_eq!(
            arrival_velocity_from_final_standing_period_meters_per_second(&velocity, 2..6),
            Some(-2.4)
        );
    }

    /// The seed cancelling is what lets this rule run without a stated box height, so it is
    /// asserted rather than described: two seeds shift the series and leave the answer alone.
    #[test]
    fn the_arrival_velocity_does_not_depend_on_the_seed_the_integration_carried() {
        let zero_anchored = [0.0, -0.4, -1.1, -1.9, -2.35, -2.35, -2.35];
        let recovered =
            arrival_velocity_from_final_standing_period_meters_per_second(&zero_anchored, 4..7)
                .expect("the final period sits inside the series");
        for seed in [-2.43, -1.0, -8.0] {
            let seeded: Vec<f64> = zero_anchored.iter().map(|value| value + seed).collect();
            let mean_of_final = seeded[4..7].iter().sum::<f64>() / 3.0;
            let corrected = seed - mean_of_final;
            assert!(
                (corrected - recovered).abs() < 1e-12,
                "a seed of {seed} gave {corrected} against {recovered}"
            );
        }
    }
}
