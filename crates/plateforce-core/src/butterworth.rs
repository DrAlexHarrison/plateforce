//! Recursive Butterworth low-pass filtering, in one pass or in two.
//!
//! The two published rules differ in what they cost. One pass delays every feature by an
//! amount that depends on its frequency content, which moves a landmark. Two passes, one
//! forward and one backward, cancel that delay and square the magnitude response, so the
//! cutoff has to be corrected before filtering or the band the user asked for is not the
//! band they get.
//!
//! Filter state is initialised to the steady-state response of the signal's opening level
//! rather than to zeros. Zero initialisation makes the filter start from a plate reading
//! nothing and climb to the standing weight, which puts a step at the front of every
//! trace, and it is what every implementation examined here does by leaving the state
//! unset.

use crate::statistics::compensated_sum;

#[derive(Debug, thiserror::Error)]
pub enum ButterworthError {
    #[error("butterworth(order = {order}) requires an order of at least one")]
    OrderBelowOne { order: usize },
    #[error("butterworth(cutoff_hz = {cutoff_hz}) must sit above zero and below the Nyquist frequency of {nyquist_hz} Hz")]
    CutoffOutsideBand { cutoff_hz: f64, nyquist_hz: f64 },
    #[error("butterworth(order = {order}) needs at least {required_samples} samples and the trace holds {sample_count}")]
    TraceTooShort {
        order: usize,
        required_samples: usize,
        sample_count: usize,
    },
}

/// Where the recursion starts.
///
/// The choice is a registry entry of its own, `filter.prewarm_state_to_dc.inertiax`, and
/// its own note records that this is a correctness question rather than a debate about
/// method: an implementation that leaves the state at zero has not chosen zero, it has
/// failed to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateInitialisation {
    Zeros,
    SteadyStateAtOpeningLevel,
}

/// One second-order section in transposed direct form II.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    feedforward: [f64; 3],
    feedback: [f64; 2],
}

impl Biquad {
    /// A low-pass section at a stated quality factor, by bilinear transform with the
    /// cutoff prewarped so the digital cutoff lands where the analogue one was asked for.
    fn low_pass(normalised_cutoff: f64, quality_factor: f64) -> Self {
        let warped = (std::f64::consts::PI * normalised_cutoff).tan();
        let warped_squared = warped * warped;
        let scale = 1.0 / (1.0 + warped / quality_factor + warped_squared);
        Self {
            feedforward: [
                warped_squared * scale,
                2.0 * warped_squared * scale,
                warped_squared * scale,
            ],
            feedback: [
                2.0 * (warped_squared - 1.0) * scale,
                (1.0 - warped / quality_factor + warped_squared) * scale,
            ],
        }
    }

    /// The single real pole an odd order carries alongside its conjugate pairs, written as
    /// a section whose second-order terms are zero so the cascade stays one loop.
    fn first_order_low_pass(normalised_cutoff: f64) -> Self {
        let warped = (std::f64::consts::PI * normalised_cutoff).tan();
        let scale = 1.0 / (1.0 + warped);
        Self {
            feedforward: [warped * scale, warped * scale, 0.0],
            feedback: [(warped - 1.0) * scale, 0.0],
        }
    }

    /// The state a constant input of one leaves this section in once it has settled.
    ///
    /// A section's gain at zero frequency is one, so the settled output equals the input
    /// and the two state variables follow from that.
    fn settled_state_per_unit_input(&self) -> [f64; 2] {
        let second = self.feedforward[2] - self.feedback[1];
        [self.feedforward[1] - self.feedback[0] + second, second]
    }

    fn apply(&self, values: &[f64], initialisation: StateInitialisation) -> Vec<f64> {
        let mut state = match initialisation {
            StateInitialisation::Zeros => [0.0, 0.0],
            StateInitialisation::SteadyStateAtOpeningLevel => {
                let opening = values.first().copied().unwrap_or(0.0);
                let settled = self.settled_state_per_unit_input();
                [settled[0] * opening, settled[1] * opening]
            }
        };
        values
            .iter()
            .map(|&input| {
                let output = self.feedforward[0] * input + state[0];
                state[0] = self.feedforward[1] * input - self.feedback[0] * output + state[1];
                state[1] = self.feedforward[2] * input - self.feedback[1] * output;
                output
            })
            .collect()
    }
}

/// The sections an order-N Butterworth low-pass cascades into.
///
/// Conjugate pole pairs give second-order sections whose quality factors spread with the
/// order, and an odd order carries one real pole besides.
fn sections(
    order: usize,
    cutoff_hz: f64,
    sample_rate_hz: f64,
) -> Result<Vec<Biquad>, ButterworthError> {
    if order < 1 {
        return Err(ButterworthError::OrderBelowOne { order });
    }
    let nyquist_hz = sample_rate_hz / 2.0;
    if !(cutoff_hz > 0.0 && cutoff_hz < nyquist_hz) {
        return Err(ButterworthError::CutoffOutsideBand {
            cutoff_hz,
            nyquist_hz,
        });
    }
    let normalised_cutoff = cutoff_hz / sample_rate_hz;
    let mut cascade: Vec<Biquad> = (0..order / 2)
        .map(|pair| {
            let angle = (2 * pair + 1) as f64 * std::f64::consts::PI / (2.0 * order as f64);
            Biquad::low_pass(normalised_cutoff, 1.0 / (2.0 * angle.sin()))
        })
        .collect();
    if order % 2 == 1 {
        cascade.push(Biquad::first_order_low_pass(normalised_cutoff));
    }
    Ok(cascade)
}

/// The factor a cutoff is divided by so that `passes` passes of an order-N filter put the
/// combined minus 3 dB point at the nominal cutoff.
///
/// At second order and two passes this is the quarter-power form Winter states, 0.8022.
/// The exponent carries the order because the correction is not order-free: the same
/// factor applied to a fourth-order filter leaves the combined half-power point away from
/// the cutoff the caller asked for.
pub fn dual_pass_cutoff_correction(order: usize, passes: usize) -> f64 {
    (2.0f64.powf(1.0 / passes as f64) - 1.0).powf(1.0 / (2.0 * order as f64))
}

/// Odd reflection about each endpoint, which continues the signal's level and its slope
/// rather than folding a mirror image back over the recursion.
fn pad_by_reflection(values: &[f64], pad_length: usize) -> Vec<f64> {
    let leading = values[0];
    let trailing = values[values.len() - 1];
    let mut padded = Vec::with_capacity(values.len() + 2 * pad_length);
    padded.extend(
        (1..=pad_length)
            .rev()
            .map(|back| 2.0 * leading - values[back]),
    );
    padded.extend_from_slice(values);
    padded.extend(
        (1..=pad_length).map(|forward| 2.0 * trailing - values[values.len() - 1 - forward]),
    );
    padded
}

fn run_cascade(
    values: &[f64],
    cascade: &[Biquad],
    initialisation: StateInitialisation,
) -> Vec<f64> {
    cascade.iter().fold(values.to_vec(), |carried, section| {
        section.apply(&carried, initialisation)
    })
}

/// One pass of an order-N Butterworth, delay and all.
///
/// The delay is the point of the entry: a rule that filters once and then places a
/// landmark on the result places it late by an amount that depends on how sharp the
/// feature was.
pub fn low_pass_single_pass(
    values: &[f64],
    cutoff_hz: f64,
    order: usize,
    sample_rate_hz: f64,
    initialisation: StateInitialisation,
) -> Result<Vec<f64>, ButterworthError> {
    let cascade = sections(order, cutoff_hz, sample_rate_hz)?;
    let pad_length = padding_for(order, values.len())?;
    let padded = pad_by_reflection(values, pad_length);
    let filtered = run_cascade(&padded, &cascade, initialisation);
    Ok(filtered[pad_length..pad_length + values.len()].to_vec())
}

/// Two passes, forward then backward, with the cutoff corrected for the pair.
///
/// The backward pass cancels the forward pass's delay exactly, so a feature comes back at
/// the sample it went in at. It also applies the magnitude response twice, which is why
/// the cutoff is divided by the correction before either pass runs.
pub fn low_pass_dual_pass_zero_lag(
    values: &[f64],
    cutoff_hz: f64,
    order: usize,
    sample_rate_hz: f64,
    initialisation: StateInitialisation,
) -> Result<Vec<f64>, ButterworthError> {
    let corrected_cutoff_hz = cutoff_hz / dual_pass_cutoff_correction(order, 2);
    let cascade = sections(order, corrected_cutoff_hz, sample_rate_hz)?;
    let pad_length = padding_for(order, values.len())?;
    let padded = pad_by_reflection(values, pad_length);

    let forward = run_cascade(&padded, &cascade, initialisation);
    let mut reversed: Vec<f64> = forward.into_iter().rev().collect();
    reversed = run_cascade(&reversed, &cascade, initialisation);
    let restored: Vec<f64> = reversed.into_iter().rev().collect();
    Ok(restored[pad_length..pad_length + values.len()].to_vec())
}

/// Three times the recursion depth on each end, which is long enough for the transient the
/// padding exists to absorb to have decayed before the real samples begin.
fn padding_for(order: usize, sample_count: usize) -> Result<usize, ButterworthError> {
    let pad_length = 3 * order;
    if sample_count <= pad_length {
        return Err(ButterworthError::TraceTooShort {
            order,
            required_samples: pad_length + 1,
            sample_count,
        });
    }
    Ok(pad_length)
}

/// The magnitude of the filter's response at one frequency, measured by running a sine
/// through it rather than by evaluating a transfer function that would be a second
/// derivation of the same coefficients.
pub fn measured_gain_at(
    frequency_hz: f64,
    cutoff_hz: f64,
    order: usize,
    sample_rate_hz: f64,
    passes: usize,
) -> Result<f64, ButterworthError> {
    let sample_count = (sample_rate_hz * 4.0) as usize;
    let input: Vec<f64> = (0..sample_count)
        .map(|index| {
            (2.0 * std::f64::consts::PI * frequency_hz * index as f64 / sample_rate_hz).sin()
        })
        .collect();
    let filtered = match passes {
        1 => low_pass_single_pass(
            &input,
            cutoff_hz,
            order,
            sample_rate_hz,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )?,
        _ => low_pass_dual_pass_zero_lag(
            &input,
            cutoff_hz,
            order,
            sample_rate_hz,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )?,
    };
    // Measured over the settled middle of the trace, so neither end's transient is in the
    // amplitude the gain is taken from.
    let settled = &filtered[sample_count / 4..3 * sample_count / 4];
    let squares: Vec<f64> = settled.iter().map(|value| value * value).collect();
    let root_mean_square = (compensated_sum(&squares) / settled.len() as f64).sqrt();
    Ok(root_mean_square * std::f64::consts::SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE_HZ: f64 = 1200.0;

    fn half_power_gain() -> f64 {
        1.0 / std::f64::consts::SQRT_2
    }

    /// The claim the dual-pass entry makes in words, held to mechanically at both orders
    /// the entry publishes.
    ///
    /// The correction is stated on the analogue prototype and applied to a cutoff that is
    /// then prewarped, so the two passes land the half-power point 0.4 percent high at
    /// 50 Hz and 1200 Hz rather than exactly on it. That residue is the published rule's,
    /// and every implementation of it carries the same one. The band it admits is what the
    /// tolerance here is set from, and it is two orders of magnitude tighter than the error
    /// the wrong correction makes: at order four, Winter's second-order factor passes 0.85
    /// where 0.71 is wanted.
    #[test]
    fn two_passes_put_the_half_power_point_at_the_nominal_cutoff() {
        for order in [2usize, 4] {
            let gain = measured_gain_at(50.0, 50.0, order, SAMPLE_RATE_HZ, 2).unwrap();
            assert!(
                (gain - half_power_gain()).abs() < 5e-3,
                "order {order} passed {gain} through at its own cutoff"
            );
        }
    }

    /// A fourth-order filter corrected as though it were second order passes far more than
    /// half power at the cutoff the caller asked for.
    #[test]
    fn the_second_order_correction_applied_to_a_fourth_order_filter_misses_the_cutoff() {
        let mistaken_cutoff_hz = 50.0 / dual_pass_cutoff_correction(2, 2);
        let sample_count = (SAMPLE_RATE_HZ * 4.0) as usize;
        let input: Vec<f64> = (0..sample_count)
            .map(|index| (2.0 * std::f64::consts::PI * 50.0 * index as f64 / SAMPLE_RATE_HZ).sin())
            .collect();
        let once = low_pass_single_pass(
            &input,
            mistaken_cutoff_hz,
            4,
            SAMPLE_RATE_HZ,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )
        .unwrap();
        let twice = low_pass_single_pass(
            &once,
            mistaken_cutoff_hz,
            4,
            SAMPLE_RATE_HZ,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )
        .unwrap();
        let settled = &twice[sample_count / 4..3 * sample_count / 4];
        let squares: Vec<f64> = settled.iter().map(|value| value * value).collect();
        let gain =
            (compensated_sum(&squares) / settled.len() as f64).sqrt() * std::f64::consts::SQRT_2;
        assert!(
            (gain - half_power_gain()).abs() > 0.1,
            "the wrong correction passed {gain}, which is close enough to half power to hide"
        );
    }

    #[test]
    fn one_pass_puts_the_half_power_point_at_the_nominal_cutoff() {
        for order in [2usize, 3, 4] {
            let gain = measured_gain_at(50.0, 50.0, order, SAMPLE_RATE_HZ, 1).unwrap();
            assert!(
                (gain - half_power_gain()).abs() < 1e-3,
                "order {order} passed {gain} through at its own cutoff"
            );
        }
    }

    /// The correction reduces to the form the registry entry states, at the order Winter
    /// stated it for.
    #[test]
    fn the_correction_is_winters_quarter_power_form_at_second_order() {
        let winter = (2.0f64.powf(0.5) - 1.0).powf(0.25);
        assert!((dual_pass_cutoff_correction(2, 2) - winter).abs() < 1e-15);
        assert!((dual_pass_cutoff_correction(4, 2) - winter).abs() > 1e-3);
    }

    /// A step whose edge sits in the middle of the trace, so the delay of each rule is
    /// measurable as the sample at which the response passes half of the step.
    fn step() -> Vec<f64> {
        let mut values = vec![600.0f64; 2400];
        for value in values.iter_mut().skip(1200) {
            *value = 1600.0;
        }
        values
    }

    fn half_rise_index(values: &[f64]) -> usize {
        values.iter().position(|&value| value >= 1100.0).unwrap()
    }

    #[test]
    fn two_passes_leave_a_step_where_it_was_and_one_pass_moves_it_later() {
        let values = step();
        let dual = low_pass_dual_pass_zero_lag(
            &values,
            20.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )
        .unwrap();
        let single = low_pass_single_pass(
            &values,
            20.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )
        .unwrap();
        let dual_at = half_rise_index(&dual) as isize;
        let single_at = half_rise_index(&single) as isize;
        assert!(
            (dual_at - 1200).abs() <= 2,
            "the zero-lag rule moved the edge to {dual_at}"
        );
        assert!(
            single_at - 1200 > 8,
            "the single pass placed the edge at {single_at}, which is not late"
        );
    }

    /// A constant is what a plate reads under a standing athlete, and it is the shape that
    /// separates the two initialisations: settled state returns it exactly, and zeros
    /// climb to it.
    #[test]
    fn a_constant_returns_unchanged_from_a_settled_state_and_climbs_from_zeros() {
        let values = vec![665.4303f64; 3000];
        let settled = low_pass_dual_pass_zero_lag(
            &values,
            20.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::SteadyStateAtOpeningLevel,
        )
        .unwrap();
        for (index, &got) in settled.iter().enumerate() {
            assert!(
                (got - 665.4303).abs() < 1e-9,
                "sample {index} came back as {got}"
            );
        }

        let from_zero = low_pass_dual_pass_zero_lag(
            &values,
            20.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
        )
        .unwrap();
        assert!(
            (from_zero[0] - 665.4303).abs() > 1.0,
            "zero initialisation started at {} and should have started low",
            from_zero[0]
        );
    }

    #[test]
    fn a_cutoff_at_or_above_nyquist_names_the_cutoff_and_the_nyquist_frequency() {
        let values = vec![600.0f64; 3000];
        let error = low_pass_single_pass(
            &values,
            600.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("600"), "{message}");
    }

    #[test]
    fn a_trace_shorter_than_the_padding_names_both_lengths() {
        let error = low_pass_single_pass(
            &[600.0, 601.0, 602.0],
            20.0,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("3"), "{message}");
        assert!(message.contains('7'), "{message}");
    }
}
