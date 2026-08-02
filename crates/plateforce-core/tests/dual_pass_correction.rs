//! Where two passes actually put the half-power point, measured by sine sweep.
//!
//! The dual-pass entry says its correction makes the combined minus 3 dB point equal the
//! nominal cutoff, and it publishes orders two and four. Winter derives the correction for
//! a second-order filter. This finds the half-power frequency under both readings so the
//! difference between them is a number somebody can take again rather than a claim.
//!
//! Run it with
//! `cargo test -p plateforce-core --test dual_pass_correction -- --nocapture`.

use plateforce_core::butterworth::{
    dual_pass_cutoff_correction, low_pass_single_pass, StateInitialisation,
};
use plateforce_core::statistics::compensated_sum;

const SAMPLE_RATE_HZ: f64 = 1200.0;
const NOMINAL_CUTOFF_HZ: f64 = 50.0;

/// Amplitude out of two passes of an order-N filter run at a stated per-pass cutoff.
fn gain_after_two_passes(frequency_hz: f64, per_pass_cutoff_hz: f64, order: usize) -> f64 {
    let sample_count = (SAMPLE_RATE_HZ * 4.0) as usize;
    let input: Vec<f64> = (0..sample_count)
        .map(|index| {
            (2.0 * std::f64::consts::PI * frequency_hz * index as f64 / SAMPLE_RATE_HZ).sin()
        })
        .collect();
    let once = low_pass_single_pass(
        &input,
        per_pass_cutoff_hz,
        order,
        SAMPLE_RATE_HZ,
        StateInitialisation::SteadyStateAtOpeningLevel,
    )
    .unwrap();
    let twice = low_pass_single_pass(
        &once,
        per_pass_cutoff_hz,
        order,
        SAMPLE_RATE_HZ,
        StateInitialisation::SteadyStateAtOpeningLevel,
    )
    .unwrap();
    let settled = &twice[sample_count / 4..3 * sample_count / 4];
    let squares: Vec<f64> = settled.iter().map(|value| value * value).collect();
    (compensated_sum(&squares) / settled.len() as f64).sqrt() * std::f64::consts::SQRT_2
}

/// The frequency at which the pair passes half power, found by bisection on the sweep.
fn half_power_frequency_hz(per_pass_cutoff_hz: f64, order: usize) -> f64 {
    let target = 1.0 / std::f64::consts::SQRT_2;
    let (mut low, mut high) = (1.0f64, SAMPLE_RATE_HZ / 2.0 - 1.0);
    for _ in 0..60 {
        let middle = (low + high) / 2.0;
        if gain_after_two_passes(middle, per_pass_cutoff_hz, order) > target {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

#[test]
fn the_correction_that_carries_the_order_and_the_one_that_does_not() {
    let winter_factor = dual_pass_cutoff_correction(2, 2);
    println!(
        "nominal cutoff {NOMINAL_CUTOFF_HZ} Hz, sample rate {SAMPLE_RATE_HZ} Hz, two passes"
    );
    for order in [2usize, 4] {
        let carried = NOMINAL_CUTOFF_HZ / dual_pass_cutoff_correction(order, 2);
        println!(
            "  order {order}, correction carries the order: half power at {:.2} Hz",
            half_power_frequency_hz(carried, order)
        );
    }
    let flat = NOMINAL_CUTOFF_HZ / winter_factor;
    println!(
        "  order 4, Winter's second-order factor: half power at {:.2} Hz",
        half_power_frequency_hz(flat, 4)
    );
}
